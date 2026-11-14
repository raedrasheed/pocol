// Sites W2b in Chromium: the built sandbox page (loader + inlined modscan) runs under the release sandbox CSP,
// served as a header by a local server (an opaque-origin sandboxed document, as the extension's sandbox page is),
// and receives sites the way viewer.js hands them over (pocol-init with a MessageChannel). No extension, wallet,
// network or node is used; every request outside the local server is aborted.
//   node tests/module_loader_browser.mjs   (PLAYWRIGHT_MODULE, POCOL_CHROMIUM select the browser)
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFileSync, mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { build } from '../build.mjs';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || '/opt/node-tools/node_modules/playwright/index.mjs');
const exe = process.env.POCOL_CHROMIUM || ['/usr/bin/chromium', '/usr/bin/google-chrome'].find((p) => existsSync(p));
const out = mkdtempSync(join(tmpdir(), 'pocol-modules-'));
const { dist, manifest } = build({ out });
const CSP = manifest.content_security_policy.sandbox;
let checks = 0;
const check = (name, cond, detail = '') => { assert.ok(cond, name + (detail ? ' — ' + detail : '')); checks++; console.log('PASS ' + name); };

const HOST = `<!doctype html><meta charset="utf-8"><body><script>
window.openSite = (files, path) => new Promise((resolve) => {
  document.querySelectorAll('iframe').forEach((f) => f.remove());
  const f = document.createElement('iframe'); f.src = '/sandbox.html';
  f.onload = () => {
    const ch = new MessageChannel(); window.bridge = [];
    ch.port1.onmessage = (e) => { window.bridge.push(e.data); ch.port1.postMessage({ seq: e.data.seq, reply: { id: 1, error: { code: 4200, message: 'unsupported in this test' } } }); };
    const fs = {}; for (const [p, v] of Object.entries(files)) fs[p] = { mime: v.mime, data: v.b64 ? Uint8Array.from(atob(v.b64), (c) => c.charCodeAt(0)) : new TextEncoder().encode(v.text) };
    f.contentWindow.postMessage({ type: 'pocol-init', files: fs, path, entry: path, storage: {}, notFound: false, version: 1 }, '*', [ch.port2]);
    resolve(true);
  };
  document.body.append(f);
});
</script>`;
const server = createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  if (u.pathname === '/sandbox.html') { res.writeHead(200, { 'content-type': 'text/html', 'content-security-policy': CSP }); res.end(readFileSync(join(dist, 'sandbox.html'))); return; }
  if (u.pathname === '/host.html') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(HOST); return; }
  res.writeHead(404); res.end('no');
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;

const html = (t) => ({ mime: 'text/html', text: t }), js = (t) => ({ mime: 'application/javascript', text: t }), css = (t) => ({ mime: 'text/css', text: t }), json = (t) => ({ mime: 'application/json', text: t });
const PNG_B64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
const SITES = {
  modules: {
    '/index.html': html(`<!doctype html><html><head><link rel="stylesheet" href="css/style.css"></head><body><h1 id="t">modules</h1><div id="bg"></div>
<script type="module">import { tag } from './js/lib.mjs'; document.body.dataset.inline = tag;</script>
<script src="js/classic.js"></script>
<script type="module" src="js/main.mjs"></script></body></html>`),
    '/css/style.css': css('#t { color: rgb(17, 170, 85); } #bg { width: 4px; height: 4px; background-image: url(../img/dot.png); }'),
    '/img/dot.png': { mime: 'image/png', b64: PNG_B64 },
    '/js/main.mjs': js(`import { lib, tag } from './lib.mjs';
import * as all from './reexport.mjs';
import data from '../data/config.json' with { type: 'json' };
const out = { lib: lib(2), tag, all: Object.keys(all).sort().join(','), json: data.name, meta: import.meta.url.slice(0, 5) };
const lazy = await import('./lazy.mjs');
out.lazy = lazy.default;
const which = 'computed';
out.computed = (await import('./' + which + '.mjs')).value;
try { await import('https://example.invalid/x.js'); out.external = 'loaded'; } catch (e) { out.external = e.message; }
try { await import('./missing.mjs'); out.missing = 'loaded'; } catch (e) { out.missing = e.message; }
try { await fetch('https://example.invalid/'); out.fetch = 'allowed'; } catch (e) { out.fetch = e.message; }
out.siteFetch = (await (await fetch('../data/config.json')).json()).name;
document.body.dataset.result = JSON.stringify(out); document.body.dataset.done = '1';`),
    '/js/lib.mjs': js("import { double } from '../util/util.js';\nexport const lib = (x) => double(x);\nexport const tag = 'lib-ok';"),
    '/js/reexport.mjs': js("export * from '../util/util.js';\nexport { tag as renamed } from './lib.mjs';"),
    '/util/util.js': js('export const double = (x) => x * 2;\nexport const triple = (x) => x * 3;'),
    '/js/lazy.mjs': js("import { tag } from './lib.mjs';\nexport default 'lazy:' + tag;"),
    '/js/computed.mjs': js("export const value = 'computed-ok';"),
    '/js/classic.js': js("import('./lazy.mjs').then((m) => { document.body.dataset.classic = m.default; });"),
    '/data/config.json': json('{"name":"cfg"}'),
  },
  split: {
    '/index.html': html('<!doctype html><body><script type="module" src="/assets/index-a1.js"></script></body>'),
    '/assets/index-a1.js': js(`import { v as vendor } from './vendor-c3.js';
window.entryEvaluations = (window.entryEvaluations || 0) + 1;
export const shared = { id: vendor() };
import('./page-b2.js').then((m) => { document.body.dataset.result = JSON.stringify(m.show()); document.body.dataset.done = '1'; }, (e) => { document.body.dataset.result = 'error ' + e.message; document.body.dataset.done = '1'; });`),
    '/assets/page-b2.js': js("import { shared } from './index-a1.js';\nimport { v } from './vendor-c3.js';\nexport const show = () => ({ same: shared.id === v(), entryEvaluations: window.entryEvaluations, page: 'b2' });"),
    '/assets/vendor-c3.js': js('export const v = () => 42;'),
  },
  cycle: {
    '/index.html': html('<!doctype html><body><p id="p">never</p><script type="module" src="a.js"></script></body>'),
    '/a.js': js("import { b } from './b.js';\nexport const a = 1;"),
    '/b.js': js("import { a } from './a.js';\nexport const b = 2;"),
  },
  lazyCycle: {
    '/index.html': html('<!doctype html><body><script type="module" src="main.js"></script></body>'),
    '/main.js': js("import('./x.js').then(() => { document.body.dataset.result = 'loaded'; }, (e) => { document.body.dataset.result = e.message; }).finally(() => { document.body.dataset.done = '1'; });"),
    '/x.js': js("import './y.js';"),
    '/y.js': js("import './x.js';"),
  },
};

const browser = await chromium.launch({ executablePath: exe, headless: true });
let blocked = 0;
try {
  const context = await browser.newContext();
  await context.route('**/*', (route) => { const u = new URL(route.request().url()); if (u.origin === origin || u.protocol === 'blob:') return route.continue(); blocked++; return route.abort('blockedbyclient'); });
  const page = await context.newPage(); const errors = [];
  page.on('pageerror', (e) => errors.push(String(e))); const consoleLog = [];
  page.on('console', (m) => consoleLog.push(m.type() + ': ' + m.text()));
  await page.goto(origin + '/host.html');
  const open = async (name, path = '/index.html') => {
    await page.evaluate(([fs, p]) => window.openSite(fs, p), [SITES[name], path]);
    const frame = () => page.frames().find((f) => f.url().endsWith('/sandbox.html'));
    let st = null;
    for (let i = 0; i < 200; i++) {
      const f = frame();
      st = f && await f.evaluate(() => ({ done: document.body && document.body.dataset.done === '1', text: document.documentElement.textContent, ready: document.documentElement.getAttribute('data-pocol-ready') })).catch(() => null);
      if (st && (st.done || /PoCol loader error/.test(st.text))) return { f, st };
      await new Promise((r) => setTimeout(r, 50));
    }
    throw new Error('site ' + name + ' did not finish: ' + JSON.stringify(st) + ' console: ' + consoleLog.slice(-10).join(' | '));
  };

  {
    const { f } = await open('modules');
    const r = JSON.parse(await f.evaluate(() => document.body.dataset.result));
    check('module graph: two levels of static imports resolve (main -> lib -> util)', r.lib === 4 && r.tag === 'lib-ok', JSON.stringify(r));
    check('export * from and export {..} from resolve', r.all === 'double,renamed,triple', r.all);
    check('a JSON module import (with { type: "json" }) resolves', r.json === 'cfg');
    check('a literal import() resolves and shares the module instance graph', r.lazy === 'lazy:lib-ok');
    check('a computed import() resolves at run time', r.computed === 'computed-ok');
    check('import() of an absolute URL is refused with a clear error', /import\("https:\/\/example\.invalid\/x\.js"\) is blocked: only files of this site/.test(r.external), r.external);
    check('import() of a missing site file is refused with a clear error', /module \/js\/missing\.mjs is not in this site version/.test(r.missing), r.missing);
    check('fetch to the network stays blocked', /network is disabled/.test(r.fetch), r.fetch);
    check('fetch of a site file still works', r.siteFetch === 'cfg');
    check('import.meta.url is the blob: URL', r.meta === 'blob:');
    check('an inline module imports a site module', await f.evaluate(() => document.body.dataset.inline) === 'lib-ok');
    for (let i = 0; i < 100 && !(await f.evaluate(() => document.body.dataset.classic)); i++) await new Promise((res) => setTimeout(res, 50));
    check('a classic script\'s import() resolves', await f.evaluate(() => document.body.dataset.classic) === 'lazy:lib-ok');
    const st = await f.evaluate(() => ({ color: getComputedStyle(document.getElementById('t')).color, bg: getComputedStyle(document.getElementById('bg')).backgroundImage }));
    check('CSS with url() still applies next to the modules', st.color === 'rgb(17, 170, 85)' && st.bg.startsWith('url("blob:'), JSON.stringify(st));
  }
  {
    const { f } = await open('split');
    const r = JSON.parse(await f.evaluate(() => document.body.dataset.result));
    check('code-split shape: a lazy chunk importing back from its entry loads, with one entry instance', r.same === true && r.entryEvaluations === 1 && r.page === 'b2', JSON.stringify(r));
  }
  {
    const { st } = await open('cycle');
    check('a static import cycle is refused with the clear error shown in the viewer', /PoCol loader error: module import cycle: \/a\.js -> \/b\.js -> \/a\.js \(blob: URLs cannot express a cycle of static imports\)/.test(st.text) && !/never/.test(st.text), st.text.slice(0, 200));
  }
  {
    const { f } = await open('lazyCycle');
    const msg = await f.evaluate(() => document.body.dataset.result);
    check('a cycle reached through import() rejects that import with the cycle error', /module import cycle: \/x\.js -> \/y\.js -> \/x\.js/.test(msg), msg);
  }
  check('no request left the local server', blocked === 0, String(blocked));
  check('no uncaught page errors in the host', errors.length === 0, errors.join('; '));
} finally {
  await browser.close(); server.close(); rmSync(out, { recursive: true, force: true });
}
console.log(JSON.stringify({ result: 'PASS', checks }));
