// WebKit smoke test: every route must boot the wasm app with no page or console errors.
// Signed-out routes redirect to /login, which serves the same index.html and module graph.
import { webkit } from 'playwright';

const base = process.env.SMOKE_URL ?? 'https://tickets.worldwidewebb.co';
const routes = ['/login', '/', '/board', '/mission'];
const browser = await webkit.launch();
let failed = false;
for (const route of routes) {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(`pageerror: ${error.message}`));
  page.on('console', message => { if (message.type() === 'error') errors.push(`console: ${message.text()}`); });
  await page.goto(base + route, { waitUntil: 'load' });
  const booted = await page.waitForFunction(() => typeof window.wasmBindings === 'object', null, { timeout: 15000 })
    .then(() => true, () => false);
  if (!booted) errors.push('wasm app did not start');
  console.log(`${errors.length ? 'FAIL' : 'PASS'} webkit ${route} -> ${new URL(page.url()).pathname}`);
  for (const error of errors) console.log(`  ${error}`);
  failed ||= errors.length > 0;
  await page.close();
}
await browser.close();
process.exit(failed ? 1 : 0);
