// Run after cargo xtask build: node --test --test-isolation=none crates/worker/tests/github_app.mjs
// Node 22.15+ supplies module hooks, SQLite and WebCrypto. No sockets or GitHub calls.
import assert from 'node:assert/strict';
import { createHash, createHmac, createPublicKey, verify } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { DatabaseSync } from 'node:sqlite';
import test from 'node:test';

const root = new URL('../../../', import.meta.url);
const build = new URL('crates/worker/build/index.js', root);
const wasm = new URL('crates/worker/build/index_bg.wasm', root);
registerHooks({
  resolve(specifier, context, next) {
    if (specifier === 'cloudflare:workers') {
      return {
        url: 'data:text/javascript,export class WorkerEntrypoint { constructor(ctx,env) { this.ctx=ctx; this.env=env; } }',
        shortCircuit: true,
      };
    }
    return next(specifier, context);
  },
  load(url, context, next) {
    if (url === wasm.href) {
      return {
        format: 'module', shortCircuit: true,
        source: `import {readFileSync} from 'node:fs'; export default new WebAssembly.Module(readFileSync(new URL(${JSON.stringify(url)})));`,
      };
    }
    return next(url, context);
  },
});
globalThis.addEventListener = () => {};
// Block every real outbound request before loading the compiled production Worker.
globalThis.fetch = async () => { throw new Error('Network disabled in callback tests'); };
const { default: Worker } = await import(build.href);

class D1Database {
  constructor() {
    this.calls = 0;
    this.db = new DatabaseSync(':memory:');
    this.db.exec('PRAGMA foreign_keys=ON');
    const migrations = new URL('migrations/', root);
    for (const file of readdirSync(migrations).filter(f => f.endsWith('.sql')).sort()) {
      this.db.exec(readFileSync(new URL(file, migrations), 'utf8'));
    }
  }
  async batch(statements) {
    this.db.exec('BEGIN');
    try {
      const results=[];
      for (const statement of statements) results.push(await statement.all());
      this.db.exec('COMMIT');
      return results;
    } catch (error) { this.db.exec('ROLLBACK'); throw error; }
  }
  prepare(sql) {
    this.calls++;
    const statement = this.db.prepare(sql);
    return {
      bind(...params) {
        return { async all() { return { success: true, results: statement.all(...params) }; } };
      },
    };
  }
}

const fixture = {
  id: 42, slug: 'fixture-app', name: '<Fixture & App>', client_id: 'fixture-id',
  client_secret: 'fixture-only-client', webhook_secret: 'fixture-only-webhook',
  pem: 'fixture-only-pem', owner: { login: 'fixture-owner' },
  html_url: 'https://github.com/apps/fixture-app',
};
const path = '/api/v1/github/app/manifest-callback';
const query = '?state=fixture-state&code=fixture-code';

async function response(worker, suffix, status, method = 'GET') {
  const result = await worker.fetch(new Request(`https://fixture.invalid${path}${suffix}`, { method }));
  assert.equal(result.status, status);
  assert.equal(result.headers.get('Cache-Control'), 'no-store');
  assert.equal(result.headers.get('Referrer-Policy'), 'no-referrer');
  assert.match(result.headers.get('Content-Type'), /^text\/html/);
  const html = await result.text();
  const style = html.match(/<style>([\s\S]*?)<\/style>/)[1];
  const hash = createHash('sha256').update(style).digest('base64');
  const csp = result.headers.get('Content-Security-Policy');
  assert.ok(csp.includes(`style-src 'sha256-${hash}'`));
  for (const directive of ["default-src 'none'", "form-action 'none'", "frame-ancestors 'none'", "base-uri 'none'"]) {
    assert.ok(csp.includes(directive));
  }
  for (const sensitive of [fixture.pem, fixture.webhook_secret, fixture.client_secret, 'fixture-state', 'fixture-code', 'upstream-body-marker']) {
    assert.ok(!html.includes(sensitive));
  }
  return html;
}

test('compiled manifest callback validates, exchanges and encrypts without network', async () => {
  await response(new Worker({}, {}), query, 503);
  const db = new D1Database();
  try {
    const env = { DB: db };
    const worker = new Worker({}, env);
    let exchanges = 0;
    let upstream = () => Response.json(fixture, { status: 201 });
    globalThis.fetch = async request => {
      exchanges++;
      assert.equal(request.url, 'https://api.github.com/app-manifests/fixture-code/conversions');
      assert.equal(request.method, 'POST');
      assert.equal(request.redirect, 'manual');
      assert.equal(request.headers.get('Authorization'), null);
      assert.equal(request.headers.get('Accept'), 'application/vnd.github+json');
      assert.equal(request.headers.get('X-GitHub-Api-Version'), '2022-11-28');
      assert.equal(request.headers.get('User-Agent'), 'tickets-worldwidewebb');
      return upstream();
    };

    await response(worker, query, 503);
    env.GITHUB_APP_SETUP_STATE = 'fixture-state';
    await response(worker, query, 503);
    env.GITHUB_APP_STORE_KEY = '';
    await response(worker, query, 503);
    env.GITHUB_APP_STORE_KEY = 'invalid-key';
    await response(worker, query, 503);
    env.GITHUB_APP_STORE_KEY = Buffer.alloc(32, 7).toString('base64');
    env.GITHUB_APP_SETUP_STATE = '';
    await response(worker, query, 503);
    env.GITHUB_APP_SETUP_STATE = 'fixture-state';
    for (const suffix of ['', '?state=wrong&code=fixture-code', `${query}&state=fixture-state`]) {
      await response(worker, suffix, 403);
    }
    for (const code of ['', 'bad/code', 'x'.repeat(129), '%2f', 'é']) {
      await response(worker, `?state=fixture-state&code=${encodeURIComponent(code)}`, 400);
    }
    await response(worker, `${query}&code=fixture-code`, 400);
    await response(worker, query, 405, 'POST');
    assert.equal(db.calls, 0);
    assert.equal(exchanges, 0);

    for (const status of [200, 301, 400, 422, 500]) {
      upstream = () => new Response('upstream-body-marker', { status });
      assert.ok((await response(worker, query, 502)).includes(`HTTP ${status}`));
    }
    upstream = () => { throw new Error('upstream-body-marker'); };
    await response(worker, query, 502);
    for (const body of ['not json', '{}', JSON.stringify({ ...fixture, pem: '' })]) {
      upstream = () => new Response(body, { status: 201 });
      await response(worker, query, 502);
    }
    assert.equal(db.db.prepare('SELECT count(*) AS n FROM github_app').get().n, 0);

    upstream = () => Response.json(fixture, { status: 201 });
    const html = await response(worker, query, 200);
    assert.ok(html.includes('&lt;Fixture &amp; App&gt;'));
    assert.ok(html.includes('href="https://github.com/apps/fixture-app/installations/new"'));
    assert.ok(html.includes('Install the app'));
    assert.ok(html.includes('href="/settings"'));
    assert.ok(html.includes('href="https://github.com/apps/fixture-app"'));
    const saved = db.db.prepare('SELECT * FROM github_app').get();
    assert.equal(saved.app_id, 42);
    assert.equal(saved.owner_login, 'fixture-owner');
    const packed = Buffer.from(saved.secrets_ciphertext, 'base64');
    const key = await crypto.subtle.importKey('raw', Buffer.alloc(32, 7), 'AES-GCM', false, ['decrypt']);
    const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: packed.subarray(0, 12), tagLength: 128 }, key, packed.subarray(12));
    assert.deepEqual(JSON.parse(new TextDecoder().decode(plaintext)), {
      pem: fixture.pem, webhook_secret: fixture.webhook_secret, client_secret: fixture.client_secret,
    });
    const beforeConflict = exchanges;
    assert.ok((await response(worker, query, 409)).includes('already registered'));
    assert.equal(exchanges, beforeConflict);
    assert.deepEqual(db.db.prepare('SELECT * FROM github_app').get(), saved);

    // A fresh registration of identical fixture data must get a fresh IV/ciphertext.
    db.db.exec('DELETE FROM github_app');
    await response(worker, query, 200);
    const fresh = Buffer.from(db.db.prepare('SELECT secrets_ciphertext FROM github_app').get().secrets_ciphertext, 'base64');
    assert.notDeepEqual(fresh.subarray(0, 12), packed.subarray(0, 12));
  } finally {
    globalThis.fetch = async () => { throw new Error('Network disabled in callback tests'); };
    db.db.close();
  }
});

test('signed-in registration page supplies manifest and fails closed without assets or network', async () => {
  const db = new D1Database();
  const token = 'ab'.repeat(32);
  db.db.prepare("INSERT INTO users(id,username) VALUES(1,'fixture-owner')").run();
  db.db.prepare("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES(?,1,'2999-01-01T00:00:00.000Z')")
    .run(createHash('sha256').update(token).digest('hex'));
  const env = { DB: db };
  const worker = new Worker({}, env);
  const route = '/github/app/register';
  const request = (suffix = '', method = 'GET', signedIn = true, origin = 'https://fixture.invalid') =>
    worker.fetch(new Request(`${origin}${route}${suffix}`, {
      method, headers: signedIn ? { Cookie: `tickets_session=${token}` } : {},
    }));
  try {
    const anonymous = await request('', 'GET', false);
    assert.equal(anonymous.status, 302);
    assert.equal(anonymous.headers.get('Location'), '/login?next=%2Fgithub%2Fapp%2Fregister');
    const bearer = await worker.fetch(new Request(`https://fixture.invalid${route}`, {
      headers: { Authorization: `Bearer tk_${token}` },
    }));
    assert.equal(bearer.status, 302);
    for (const method of ['POST', 'PUT', 'DELETE', 'HEAD', 'OPTIONS']) {
      const result = await request('', method);
      assert.equal(result.status, 405);
      assert.equal(result.headers.get('Allow'), 'GET');
    }
    for (const config of [
      {}, { GITHUB_APP_SETUP_STATE: 'fixture-state' },
      { GITHUB_APP_STORE_KEY: Buffer.alloc(32, 7).toString('base64') },
      { GITHUB_APP_SETUP_STATE: '', GITHUB_APP_STORE_KEY: '' },
      { GITHUB_APP_SETUP_STATE: 'fixture-state', GITHUB_APP_STORE_KEY: 'invalid' },
    ]) {
      delete env.GITHUB_APP_SETUP_STATE;
      delete env.GITHUB_APP_STORE_KEY;
      Object.assign(env, config);
      const result = await request();
      assert.equal(result.status, 503);
      const html = await result.text();
      assert.ok(html.includes('Registration unavailable'));
      assert.ok(!html.includes('<form'));
    }
    env.GITHUB_APP_SETUP_STATE = 'fixture&state=+ /é';
    env.GITHUB_APP_STORE_KEY = Buffer.alloc(32, 7).toString('base64');
    const unsupportedOrigin = await request('', 'GET', true, 'ftp://fixture.invalid');
    assert.equal(unsupportedOrigin.status, 503);
    assert.ok(!(await unsupportedOrigin.text()).includes('<form'));
    for (const [suffix, name, origin] of [
      ['', 'Tickets by World Wide Webb', 'https://fixture.invalid'],
      ['?name=alt', 'Tickets WWW', 'http://localhost:8787'],
      ['?name=%3Cscript%3Eevil', 'Tickets by World Wide Webb', 'https://fixture.invalid'],
    ]) {
      const result = await request(suffix, 'GET', true, origin);
      assert.equal(result.status, 200);
      for (const [header, value] of Object.entries({
        'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer',
        'X-Content-Type-Options': 'nosniff', 'X-Frame-Options': 'DENY',
      })) assert.equal(result.headers.get(header), value);
      const html = await result.text();
      const csp = result.headers.get('Content-Security-Policy');
      for (const tag of ['style', 'script']) {
        const content = html.match(new RegExp(`<${tag}>([\\s\\S]*?)</${tag}>`))[1];
        const hash = createHash('sha256').update(content).digest('base64');
        assert.ok(csp.includes(`${tag}-src 'sha256-${hash}'`));
      }
      assert.ok(csp.includes('form-action https://github.com;'));
      assert.ok(csp.includes("default-src 'none'"));
      assert.ok(csp.includes("frame-ancestors 'none'"));
      assert.ok(!csp.includes('unsafe-inline'));
      const action = html.match(/action="([^"]+)"/)[1];
      assert.equal(new URL(action).searchParams.get('state'), env.GITHUB_APP_SETUP_STATE);
      const manifest = JSON.parse(html.match(/name="manifest" value="([^"]+)"/)[1]
        .replaceAll('&quot;', '"').replaceAll('&#39;', "'")
        .replaceAll('&lt;', '<').replaceAll('&gt;', '>').replaceAll('&amp;', '&'));
      assert.equal(manifest.name, name);
      assert.equal(manifest.url, origin);
      assert.equal(manifest.redirect_url, `${origin}/api/v1/github/app/manifest-callback`);
      assert.ok(!html.includes('evil'));
      assert.ok(!html.includes(env.GITHUB_APP_STORE_KEY));
    }
    db.db.prepare(`INSERT INTO github_app(id,app_id,slug,name,owner_login,client_id,html_url,secrets_ciphertext)
      VALUES(1,42,'fixture-app','Fixture App','fixture-owner','fixture-id',?,'fixture-ciphertext')`).run(fixture.html_url);
    const result = await request();
    assert.equal(result.status, 200);
    const html = await result.text();
    assert.ok(html.includes('already registered'));
    assert.ok(html.includes(`href="${fixture.html_url}"`));
    for (const forbidden of ['<form', '<script', 'fixture&state', 'fixture-ciphertext']) {
      assert.ok(!html.includes(forbidden));
    }
  } finally {
    db.db.close();
  }
});

const rsaPem = readFileSync(new URL('crates/core/tests/fixtures/github-app-test-rsa.pem', root), 'utf8');
const pkcs8Pem = readFileSync(new URL('crates/core/tests/fixtures/github-app-test-pkcs8.pem', root), 'utf8');
const appPath = '/api/v1/github/app/webhook';
async function signed(worker, event, payload, expected, options = {}) {
  const body = options.body ?? JSON.stringify(payload);
  const signature = options.signature ?? `sha256=${createHmac('sha256', fixture.webhook_secret).update(body).digest('hex')}`;
  const result = await worker.fetch(new Request(`https://fixture.invalid${options.path ?? appPath}`, {
    method: 'POST', body, headers: { 'X-GitHub-Event': event, 'X-Hub-Signature-256': signature, ...options.headers },
  }));
  assert.equal(result.status, expected, await result.text());
}
async function registerFixture(env, pem = rsaPem) {
  globalThis.fetch = async request => {
    assert.ok(request.url.includes('/app-manifests/'));
    return Response.json({...fixture, pem}, {status: 201});
  };
  await response(new Worker({}, env), query, 200);
}
function testEnv(db) {
  return {DB: db, GITHUB_APP_STORE_KEY: Buffer.alloc(32, 7).toString('base64'),
    GITHUB_APP_SETUP_STATE: 'fixture-state', GITHUB_PR_OWNERS: 'fixture-owner',
    GITHUB_WEBHOOK_SECRET: fixture.webhook_secret, GITHUB_API_BASE_URL: 'https://github-api.fixture.invalid'};
}
const install = {id:7, account:{login:'fixture-owner'}, repository_selection:'all', suspended_at:null};
function prPayload(action = 'opened', overrides = {}) {
  return {action, repository:{full_name:'fixture-owner/repo',owner:{login:'fixture-owner'}}, installation:install,
    pull_request:{number:1,html_url:'https://github.com/fixture-owner/repo/pull/1',title:'Feature [TIK-1]',body:'Closes TIK-1',state:'open',draft:false,merged:false,head:{sha:'a'.repeat(40)},updated_at:'2026-10-30T12:00:00Z',...overrides}};
}
function seed(db) {
  db.db.exec("INSERT INTO projects(name,key_prefix,color) VALUES('Example','TIK','#5e6ad2'); INSERT INTO tickets(project_id,number,title,status) VALUES(1,1,'Example ticket','todo');");
}

test('app signatures, bounded body, installation lifecycle, and unchanged legacy endpoint', async () => {
  const db=new D1Database();
  const env=testEnv(db);
  const worker=new Worker({},env);
  try {
    await signed(worker,'ping',{},503);
    await registerFixture(env);
    globalThis.fetch=async()=>{throw new Error('Network forbidden');};
    for(const signature of ['', 'sha256='+'0'.repeat(64), 'sha256='+'g'.repeat(64)]) {
      await signed(worker,'installation',{action:'created',installation:install},401,{signature});
    }
    assert.equal(db.db.prepare('SELECT count(*) n FROM github_app_installations').get().n,0);
    await signed(worker,'ping',{},204);
    await signed(worker,'ping',{},400,{body:'not json'});
    await signed(worker,'ping',{},413,{body:'x'.repeat(1024*1024+1)});
    await signed(worker,'ping',{},413,{headers:{'Content-Length':String(1024*1024+1)}});
    await signed(worker,'installation',{action:'created',installation:install,repositories:[{full_name:'Fixture-Owner/Repo'}]},204);
    assert.deepEqual({...db.db.prepare('SELECT installation_id,account_login,repository_selection,suspended_at FROM github_app_installations').get()}, {installation_id:7,account_login:'fixture-owner',repository_selection:'all',suspended_at:null});
    await signed(worker,'installation_repositories',{action:'added',installation:install,repository_selection:'selected',repositories_added:[{full_name:'fixture-owner/second'}],repositories_removed:[]},204);
    assert.equal(db.db.prepare('SELECT count(*) n FROM github_app_installation_repos').get().n,2);
    assert.equal(db.db.prepare('SELECT repository_selection FROM github_app_installations').get().repository_selection,'selected');
    await signed(worker,'installation_repositories',{action:'removed',installation:install,repository_selection:'selected',repositories_added:[],repositories_removed:[{full_name:'fixture-owner/repo'}]},204);
    assert.equal(db.db.prepare('SELECT repo_full_name FROM github_app_installation_repos').get().repo_full_name,'fixture-owner/second');
    await signed(worker,'installation',{action:'suspend',installation:{...install,suspended_at:'2026-10-30T12:00:00Z'}},204);
    assert.equal(db.db.prepare('SELECT suspended_at FROM github_app_installations').get().suspended_at,'2026-10-30T12:00:00Z');
    await signed(worker,'installation',{action:'unsuspend',installation:install},204);
    assert.equal(db.db.prepare('SELECT suspended_at FROM github_app_installations').get().suspended_at,null);
    await signed(worker,'installation',{action:'new_permissions_accepted',installation:install,repositories:[]},204);
    assert.equal(db.db.prepare('SELECT count(*) n FROM github_app_installation_repos').get().n,0);
    const bearer = 'tk_'+'ab'.repeat(32);
    db.db.prepare("INSERT INTO users(id,username) VALUES(1,'fixture')").run();
    db.db.prepare("INSERT INTO api_tokens(user_id,name,token_hash) VALUES(1,'fixture',?)").run(createHash('sha256').update(bearer).digest('hex'));
    const overview=await worker.fetch(new Request('https://fixture.invalid/api/v1/github/app', {headers:{Authorization:`Bearer ${bearer}`}}));
    assert.equal(overview.status,200);
    assert.deepEqual(await overview.json(),{registered:true,app_slug:'fixture-app',app_name:fixture.name,html_url:fixture.html_url,installations:[{account_login:'fixture-owner',repository_selection:'all',repos:[]}]});
    assert.equal(overview.headers.get('Cache-Control'),'no-store');
    await signed(worker,'installation',{action:'deleted',installation:install},204);
    assert.equal(db.db.prepare('SELECT count(*) n FROM github_app_installations').get().n,0);
    await signed(worker,'push',{},202);
    await signed(worker,'ping',{},200,{path:'/api/v1/github/webhook'});
    await signed(worker,'push',{},202,{path:'/api/v1/github/webhook'});
    await signed(worker,'ping',{},401,{path:'/api/v1/github/webhook',signature:'sha256='+'0'.repeat(64)});
    const unauth=await worker.fetch(new Request('https://fixture.invalid/api/v1/github/app'));
    assert.equal(unauth.status,401);
  } finally {db.db.close();}
});

test('app PRs share status sync; JWT verifies; checks and CI persist per head; failures stay nonfatal', async () => {
  const db=new D1Database(); const env=testEnv(db); const worker=new Worker({},env); seed(db);
  try {
    await registerFixture(env);
    let runs=[]; let jwtCalls=0; let fetches=0; let runId=100;
    globalThis.fetch=async request=>{
      fetches++;
      assert.ok(request.url.startsWith(env.GITHUB_API_BASE_URL));
      assert.equal(request.headers.get('Accept'),'application/vnd.github+json');
      assert.equal(request.headers.get('User-Agent'),'tickets-worldwidewebb');
      assert.equal(request.headers.get('X-GitHub-Api-Version'),'2022-11-28');
      assert.equal(request.redirect,'manual');
      if(request.url.endsWith('/app/installations/7/access_tokens')) {
        jwtCalls++;
        const jwt=request.headers.get('Authorization').slice(7); const [head,claims,signature]=jwt.split('.');
        assert.deepEqual(JSON.parse(Buffer.from(head,'base64url')), {alg:'RS256',typ:'JWT'});
        const payload=JSON.parse(Buffer.from(claims,'base64url')); const now=Date.now()/1000;
        assert.equal(payload.iss,'42'); assert.ok(Math.abs(payload.iat-(now-60))<5); assert.equal(payload.exp-payload.iat,600);
        assert.ok(verify('RSA-SHA256', Buffer.from(`${head}.${claims}`), createPublicKey(rsaPem), Buffer.from(signature,'base64url')));
        return Response.json({token:'fixture-installation-token'}, {status:201});
      }
      assert.equal(request.headers.get('Authorization'),'Bearer fixture-installation-token');
      if(request.url.endsWith('/pulls/1')) return Response.json(prPayload().pull_request);
      assert.ok(request.url.endsWith('/repos/fixture-owner/repo/check-runs'));
      runs.push(await request.json());
      return Response.json({id:runId++},{status:201});
    };
    await signed(worker,'pull_request',prPayload(),204);
    assert.equal(db.db.prepare('SELECT status FROM tickets').get().status,'in_review');
    assert.equal(runs[0].conclusion,'success');
    assert.equal(runs[0].details_url,'https://tickets.worldwidewebb.co/t/TIK-1');
    assert.match(runs[0].output.summary,/Closes \[TIK-1\]/);
    assert.equal(db.db.prepare('SELECT tickets_check,check_run_id FROM github_pull_request_checks').get().check_run_id,100);
    // Rerequest only this app's Tickets check. No network for unrelated runs.
    const check={name:'Tickets',app:{id:42},pull_requests:[{number:1}]};
    await signed(worker,'check_run',{...prPayload(),action:'rerequested',check_run:{...check,app:{id:99}}},202);
    await signed(worker,'check_run',{...prPayload(),action:'rerequested',check_run:{...check,name:'Other'}},202);
    const prior=runs.length;
    await signed(worker,'check_run',{...prPayload(),action:'rerequested',check_run:check},204);
    assert.equal(runs.length,prior+1);
    for(const action of ['requested','rerequested']) await signed(worker,'check_suite',{...prPayload(),action,check_suite:{pull_requests:[{number:1}]}},204);
    let suite={head_sha:'a'.repeat(40),app:{id:99},conclusion:'success',updated_at:'2026-10-30T12:00:01Z',pull_requests:[{number:1}]};
    await signed(worker,'check_suite',{...prPayload(),action:'completed',check_suite:suite},204);
    assert.equal(db.db.prepare('SELECT ci_status FROM github_pull_request_checks').get().ci_status,'success');
    await signed(worker,'check_suite',{...prPayload(),action:'completed',check_suite:{...suite,conclusion:'failure',updated_at:'2026-10-30T12:00:00Z'}},204);
    assert.equal(db.db.prepare('SELECT ci_status FROM github_pull_request_checks').get().ci_status,'success');
    await signed(worker,'check_suite',{...prPayload(),action:'completed',check_suite:{...suite,app:{id:42}}},202);
    await signed(worker,'pull_request',prPayload('edited',{title:'Unknown [TIK-999]',body:null,updated_at:'2026-10-30T12:00:02Z'}),202);
    assert.equal(runs.at(-1).output.title,'Unknown ticket key');
    await signed(worker,'pull_request',prPayload('edited',{title:'No key',body:null,updated_at:'2026-10-30T12:00:03Z'}),202);
    assert.equal(runs.at(-1).output.title,'No ticket key');
    const before=fetches;
    env.GITHUB_PR_OWNERS='another-owner';
    await signed(worker,'pull_request',prPayload(),202);
    assert.equal(fetches,before);
    env.GITHUB_PR_OWNERS='fixture-owner';
    assert.ok(jwtCalls>=4);
    // Network failures after the status mutation cannot turn it into an error.
    globalThis.fetch=async()=>{throw new Error('Network disabled');};
    await signed(worker,'pull_request',prPayload('synchronize',{head:{sha:'b'.repeat(40)},updated_at:'2026-10-30T12:00:04Z'}),204);
    assert.equal(db.db.prepare('SELECT head_sha FROM github_pull_request_events').get().head_sha,'b'.repeat(40));
    await signed(worker,'pull_request',prPayload('closed',{state:'closed',merged:true,updated_at:'2026-10-30T12:00:05Z'}),204);
    assert.equal(db.db.prepare('SELECT status FROM tickets').get().status,'done');
    // Already-PKCS8 keys follow the same production signing path.
    db.db.exec('DELETE FROM github_app');
    await registerFixture(env,pkcs8Pem);
    let tokenRequest=false;
    globalThis.fetch=async request=>{ tokenRequest=true; return Response.json({token:'fixture-token'},{status:201}); };
    await signed(worker,'pull_request',prPayload('reopened',{updated_at:'2026-10-30T12:00:06Z'}),204);
    assert.ok(tokenRequest);
  } finally {db.db.close();globalThis.fetch=async()=>{throw new Error('Network disabled');};}
});

test('credentials export enforces bearer auth and returns only stored encrypted credentials', async () => {
  const db = new D1Database();
  try {
    db.db.exec("INSERT INTO users(id,username) VALUES(1,'fixture')");
    const bearer = `tk_${'a'.repeat(64)}`;
    const session = 'b'.repeat(64);
    const hash = value => createHash('sha256').update(value).digest('hex');
    db.db.prepare("INSERT INTO api_tokens(user_id,name,token_hash) VALUES(1,'fixture',?)").run(hash(bearer));
    db.db.prepare("INSERT INTO sessions(user_id,token_hash,expires_at) VALUES(1,?,strftime('%Y-%m-%dT%H:%M:%fZ','now','+1 day'))").run(hash(session));
    // No store key is supplied: exporting must never decrypt the stored blob.
    const worker = new Worker({}, { DB: db });
    const url = 'https://fixture.invalid/api/v1/github/app/credentials';
    const request = async (headers, status) => {
      const result = await worker.fetch(new Request(url, { headers }));
      assert.equal(result.status, status);
      assert.equal(result.headers.get('Cache-Control'), 'no-store');
      return result.json();
    };
    const tokenHeaders = { Authorization: `Bearer ${bearer}` };
    const sessionHeaders = { Cookie: `tickets_session=${session}` };
    await request({}, 401);
    assert.equal((await request(tokenHeaders, 404)).error.code, 'not_found');
    assert.equal((await request(sessionHeaders, 403)).error.code, 'forbidden');
    const expected = {
      app_id: 42, slug: 'fixture-app', name: 'Fixture App', owner_login: 'fixture-owner',
      client_id: 'fixture-id', html_url: 'https://github.com/apps/fixture-app',
      secrets_ciphertext: Buffer.alloc(44, 7).toString('base64'),
    };
    db.db.prepare('INSERT INTO github_app(id,app_id,slug,name,owner_login,client_id,html_url,secrets_ciphertext) VALUES(1,?,?,?,?,?,?,?)')
      .run(...Object.values(expected));
    assert.deepEqual(await request(tokenHeaders, 200), expected);
    assert.equal((await request(sessionHeaders, 403)).error.code, 'forbidden');
    await request({ ...sessionHeaders, Authorization: 'Bearer invalid' }, 401);
    db.db.exec('UPDATE api_tokens SET revoked_at=created_at');
    await request(tokenHeaders, 401);
  } finally { db.db.close(); }
});

test('D1 availability errors return 503 and SQL failures stay 500 through Worker boundaries', async () => {
  const bearer = `tk_${'c'.repeat(64)}`;
  const session = 'd'.repeat(64);
  const details = [
    'D1_ERROR: Your account has exceeded D1\'s free tier daily row read limit.',
    'D1_ERROR: D1 DB is overloaded. Too many requests queued.',
    'D1_ERROR: Cannot resolve D1 DB due to transient issue on remote node.',
    'D1_ERROR: Network connection lost.',
    'D1_EXEC_ERROR: near LIMIT: syntax error',
    'D1_TYPE_ERROR: unavailable column',
  ];
  const originalConsole = console.error;
  const originalCaches = globalThis.caches;
  globalThis.caches = { default: { match: async () => undefined, put: async () => {} } };
  const logs = [];
  console.error = (...args) => logs.push(args.join(' '));
  try {
    for (const detail of details) {
      const expected = detail.startsWith('D1_EXEC_ERROR') || detail.startsWith('D1_TYPE_ERROR') ? 500 : 503;
      for (const mode of ['throw', 'cause', 'result']) {
        for (const stage of ['auth', 'cache', 'query', 'batch', 'mission', 'avatar', 'navigation']) {
          const db = new D1Database();
          try {
            db.db.exec("INSERT INTO users(id,username) VALUES(1,'fixture')");
            const hash = value => createHash('sha256').update(value).digest('hex');
            db.db.prepare("INSERT INTO api_tokens(user_id,name,token_hash) VALUES(1,'fixture',?)").run(hash(bearer));
            db.db.prepare("INSERT INTO sessions(user_id,token_hash,expires_at) VALUES(1,?,strftime('%Y-%m-%dT%H:%M:%fZ','now','+1 day'))").run(hash(session));
            const failure = () => {
              const message = `${detail} private-upstream-marker`;
              if (mode === 'throw') throw new Error(message);
              if (mode === 'cause') throw new Error('D1_ERROR', { cause: new Error(message) });
              return { success: false, error: message };
            };
            const prepare = db.prepare.bind(db);
            let calls = 0;
            db.prepare = sql => {
              calls++;
              const failAt = stage === 'query' ? 3 : stage === 'auth' || stage === 'navigation' ? 1 : 2;
              if (calls === failAt && stage !== 'batch') {
                return { bind() { return { async all() { return failure(); } }; } };
              }
              return prepare(sql);
            };
            if (stage === 'batch') db.batch = async () => mode !== 'result' ? failure() : [failure()];
            const worker = new Worker({}, { DB: db });
            const headers = stage === 'navigation' || stage === 'avatar'
              ? { Cookie: `tickets_session=${session}`, Origin: 'https://fixture.invalid' }
              : { Authorization: `Bearer ${bearer}` };
            let path = '/api/v1/projects';
            const options = { headers };
            if (stage === 'batch') {
              db.db.exec("INSERT INTO projects(id,name,key_prefix,color) VALUES(1,'Fixture','EX','#123456')");
              path = '/api/v1/tickets'; options.method = 'POST';
              options.body = JSON.stringify({project_id:1,title:'Fixture'});
            }
            if (stage === 'mission') { path = '/api/v1/mission/reports/fixture'; options.method = 'PUT'; options.body = '{}'; }
            if (stage === 'avatar') { path = '/api/v1/me/avatar'; options.method = 'PUT'; headers['Content-Type']='image/png'; options.body=Buffer.from([137,80,78,71,13,10,26,10]); }
            if (stage === 'navigation') path = '/board';
            const result = await worker.fetch(new Request(`https://fixture.invalid${path}`, options));
            assert.equal(result.status, expected, `${stage}/${mode}/${detail}`);
            assert.equal(result.headers.get('Cache-Control'), 'no-store');
            const body = await result.json();
            assert.equal(body.error.code, expected === 503 ? 'unavailable' : 'internal');
            if (expected === 503) assert.match(body.error.message, /Database temporarily unavailable/);
            assert.ok(!JSON.stringify(body).includes('private-upstream-marker'));
          } finally { db.db.close(); }
        }
      }
    }
    assert.ok(logs.some(line => line.includes('D1_ERROR')));
    assert.ok(logs.some(line => line.includes('D1_EXEC_ERROR')));
    assert.ok(logs.some(line => line.includes('D1_TYPE_ERROR')));
    assert.ok(logs.every(line => !line.includes('private-upstream-marker')));
  } finally { console.error = originalConsole; globalThis.caches = originalCaches; }
});

test('compiled OpenAPI endpoint is static, public and makes zero binding/D1 calls', async () => {
  let reads = 0;
  const env = new Proxy({}, { get() { reads++; throw new Error('Spec touched a binding'); } });
  const worker = new Worker({}, env);
  for (const headers of [{}, { Authorization: 'Bearer invalid' }, { Cookie: 'tickets_session=invalid' }]) {
    const result = await worker.fetch(new Request('https://fixture.invalid/api/openapi.json', { headers }));
    assert.equal(result.status, 200);
    assert.match(result.headers.get('Content-Type'), /^application\/json/);
    assert.equal(result.headers.get('Cache-Control'), 'public, max-age=3600');
    const document = await result.json();
    assert.equal(document.openapi, '3.1.0');
    assert.deepEqual(document, JSON.parse(readFileSync(new URL('docs/openapi.json', root), 'utf8')));
  }
  assert.equal(reads, 0);
});

test('compiled auth routes in the spec reach their real handlers', async () => {
  const db = new D1Database();
  const session = 'ab'.repeat(32);
  db.db.prepare("INSERT INTO users(id,username) VALUES(1,'fixture')").run();
  db.db.prepare("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES(?,1,'2999-01-01T00:00:00.000Z')")
    .run(createHash('sha256').update(session).digest('hex'));
  db.db.prepare("INSERT INTO api_tokens(user_id,name,token_hash) VALUES(1,'fixture','fixture-hash')").run();
  const worker = new Worker({}, { DB: db });
  const spec = JSON.parse(readFileSync(new URL('docs/openapi.json', root), 'utf8'));
  const cases = [
    ['GET','/api/health',200], ['POST','/api/v1/login',403],
    ['GET','/api/v1/me',200], ['PATCH','/api/v1/me',200,{display_name:'Fixture'}],
    ['GET','/api/v1/users',200], ['GET','/api/v1/version',200],
    ['GET','/api/v1/tokens',200], ['POST','/api/v1/tokens',201,{name:'Temporary fixture'}],
    ['DELETE','/api/v1/tokens/1',200],
    ['PUT','/api/v1/me/avatar',415,'invalid-image'], ['POST','/api/v1/me/avatar',415,'invalid-image'],
    ['DELETE','/api/v1/me/avatar',200], ['GET','/api/v1/users/1/avatar',404],
    ['GET','/api/v1/live/1',404], ['POST','/api/v1/logout',200],
  ];
  try {
    for (const [method,path,status,body] of cases) {
      const template = path.replace('/tokens/1','/tokens/{id}').replace('/users/1/','/users/{id}/').replace('/live/1','/live/{project_id}');
      assert.ok(spec.paths[template]?.[method.toLowerCase()], `undocumented ${method} ${template}`);
      const headers = { Cookie: `tickets_session=${session}`, Origin:'https://fixture.invalid', 'Content-Type':'application/json' };
      if (path.endsWith('/login')) delete headers.Origin;
      const response = await worker.fetch(new Request(`https://fixture.invalid${path}`, {method,headers,body:body===undefined?undefined:typeof body==='string'?body:JSON.stringify(body)}));
      assert.equal(response.status,status,`${method} ${path}`);
      if (status===404) assert.ok(!(await response.text()).includes('Resource not found'), `missing handler ${path}`);
    }
  } finally { db.db.close(); }
});
