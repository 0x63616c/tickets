// Run after cargo xtask build: node --test crates/worker/tests/github_app.mjs
// Node 22.15+ supplies module hooks, SQLite and WebCrypto. No sockets or GitHub calls.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
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
    assert.ok(html.includes('Install on all repositories'));
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
