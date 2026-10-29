// LP9/LP10 browser end-to-end: real Chromium with the MV3 extension against a
// live 3-node pocold devnet and websites published into PoCol state.
//   node tests/e2e_browser.mjs            (needs pocold release binaries built)
// Evidence: RpcTap (HTTP proxy in front of node RPC, records every method the
// extension sends), an HTTP canary and a UDP (STUN) canary that must stay at zero
// for the release build, and fault builds that must FAIL (negative controls).
import { spawn, spawnSync } from 'node:child_process';
import { createServer, request as httpRequest } from 'node:http';
import { createSocket } from 'node:dgram';
import { mkdtempSync, writeFileSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { deflateSync } from 'node:zlib';
import { connect } from 'node:net';
import { build } from '../build.mjs';
import { runLiveRp } from './live_rp.mjs';
import { keepOnFailure } from './harness.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE), POCOLD = join(EXT, '..', 'pocold');
const BIN = (n) => join(POCOLD, 'target', 'release', n);
const PW = process.env.PLAYWRIGHT_MODULE || '/opt/node-tools/node_modules/playwright/index.mjs';
const { chromium } = await import(PW);
const TAP = 8600, CANARY = 8601, UDP = 8602, HOST_IP = process.env.CANARY_IP || '192.0.2.2';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = []; let failed = 0;
const ok = (name, cond, detail = '') => { results.push({ name, pass: !!cond, detail: String(detail).slice(0, 300) }); console.log((cond ? 'PASS ' : 'FAIL ') + name + (detail !== '' ? ' — ' + String(detail).slice(0, 300) : '')); if (!cond) failed++; };
const wait = async (pred, ms, what) => { const t = Date.now(); let last; while (Date.now() - t < ms) { try { if ((last = await pred())) return last; } catch (e) { last = e; } await sleep(300); } throw new Error('timeout: ' + what + (last instanceof Error ? ' (' + last.message + ')' : '')); };

// ---------------- devnet ----------------
// E2E_KEEP=1 leaves the devnet running and writes state.json; E2E_REUSE=<work> reuses it (debug only).
const REUSE = process.env.E2E_REUSE || null, KEEP = process.env.E2E_KEEP === '1';
const work = REUSE || mkdtempSync(join(tmpdir(), 'pocol-browser-'));
const py = (args) => { const r = spawnSync('python3', [join(POCOLD, 'scripts', 'devnet.py'), ...args], { encoding: 'utf8' }); if (r.status) throw new Error(r.stderr); return r.stdout; };
const rpc = async (port, method, params = []) => {
  const r = await fetch(`http://127.0.0.1:${port}/`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) });
  const v = await r.json(); if (v.error) throw Object.assign(new Error(v.error.message), { rpc: v.error }); return v.result;
};
const portFree = (p) => new Promise((res) => { const s = connect(p, '127.0.0.1'); s.on('connect', () => { s.destroy(); res(false); }); s.on('error', () => res(true)); });

// ---------------- RpcTap and canaries ----------------
// genesisOverride (A06): rewrites pocol_status.genesisHash, as an endpoint whose identity changed.
const tap = { log: [], holdMs: {}, genesisOverride: null };
const tapServer = createServer((req, res) => {
  let body = ''; req.on('data', (c) => { body += c; }); req.on('end', async () => {
    let m = null; try { m = JSON.parse(body); } catch {}
    const methods = Array.isArray(m) ? m.map((x) => x.method) : [m && m.method];
    for (const x of methods) tap.log.push({ method: x, params: m && !Array.isArray(m) ? m.params : null, origin: req.headers.origin || null, t: Date.now() });
    const hold = tap.holdMs[methods[0]]; if (hold) await sleep(hold);
    const up = httpRequest({ host: '127.0.0.1', port: 8540, method: 'POST', path: '/', headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(body) } }, (r) => {
      if (tap.genesisOverride && methods[0] === 'pocol_status') {
        let t = ''; r.on('data', (c) => { t += c; }); r.on('end', () => { const v = JSON.parse(t); if (v.result) v.result.genesisHash = tap.genesisOverride; res.writeHead(r.statusCode, { 'content-type': 'application/json', 'access-control-allow-origin': '*' }); res.end(JSON.stringify(v)); });
        return;
      }
      res.writeHead(r.statusCode, { 'content-type': 'application/json', 'access-control-allow-origin': '*' }); r.pipe(res);
    });
    up.on('error', () => { res.writeHead(502); res.end(); }); up.end(body);
  });
});
const canary = { http: [], udp: 0 };
const canaryServer = createServer((req, res) => { canary.http.push(req.method + ' ' + req.url); res.writeHead(200, { 'access-control-allow-origin': '*', 'content-type': 'text/plain' }); res.end('canary'); });
canaryServer.on('upgrade', (req, sock) => { canary.http.push('UPGRADE ' + req.url); sock.destroy(); });
const udp = createSocket('udp4'); udp.on('message', () => { canary.udp++; });
const tapMethods = () => tap.log.map((x) => x.method);
const tapCount = (m) => tap.log.filter((x) => x.method === m).length;

// ---------------- websites ----------------
function png2x2() {
  const crc = (b) => { let c, t = []; for (let n = 0; n < 256; n++) { c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } let x = 0xffffffff; for (const v of b) x = t[(x ^ v) & 255] ^ (x >>> 8); return (x ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => { const l = Buffer.alloc(4); l.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
  const ihdr = Buffer.from([0, 0, 0, 2, 0, 0, 0, 2, 8, 2, 0, 0, 0]);
  const raw = Buffer.from([0, 255, 0, 0, 0, 255, 0, 0, 0, 0, 255, 255, 255, 0]);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
function writeSite(dir, files) { for (const [p, c] of Object.entries(files)) { const f = join(dir, p); mkdirSync(dirname(f), { recursive: true }); writeFileSync(f, c); } }
const siteA = (v) => ({
  'index.html': `<!doctype html><html><head><meta charset="utf-8"><title>PoCol test site v${v}</title><link rel="stylesheet" href="style.css">
<link rel="prefetch" href="http://127.0.0.1:${CANARY}/prefetch"><meta http-equiv="refresh" content="0;url=http://127.0.0.1:${CANARY}/refresh"></head><body>
<h1 id="t">PoCol on-chain site v${v}</h1><img id="logo" src="img/logo.png" alt="logo"><div id="bg"></div>
<p><a id="docs" href="docs/">docs</a> <a id="about" href="about.html">about</a> <a id="ext" href="https://example.org/">external</a> <a id="js" href="javascript:alert(1)">js</a></p>
<script src="app.js"></script><script>document.getElementById('t').dataset.inline = 'yes';</script>
<script src="http://127.0.0.1:${CANARY}/evil.js"></script><iframe src="http://127.0.0.1:${CANARY}/frame"></iframe>
<img id="evilimg" src="http://127.0.0.1:${CANARY}/img" onerror="document.title='handler-ran'"><object data="http://127.0.0.1:${CANARY}/obj"></object>
</body></html>`,
  'style.css': `@import url("theme.css");\nh1{color:#1a5}\n#bg{width:4px;height:4px;background:url(img/logo.png)}\n`,
  'theme.css': 'body{background:rgb(244, 246, 248)}',
  'app.js': `document.getElementById('t').dataset.loaded='yes';
fetch('data.json').then(r=>r.json()).then(j=>{document.body.dataset.fetch=String(j.rows.length)});
const x=new XMLHttpRequest();x.open('GET','data.json');x.responseType='json';x.onload=()=>{document.body.dataset.xhr=String(x.response.rows.length)};x.send();
addEventListener('DOMContentLoaded',()=>{document.body.dataset.dcl='yes'});`,
  'data.json': JSON.stringify({ rows: Array.from({ length: 1400 }, (_, i) => ({ i, v: 'x'.repeat(40) })) }),
  'img/logo.png': png2x2(),
  'docs/index.html': '<!doctype html><h1 id="docs">Docs page from PoCol state</h1><a id="home" href="../">home</a>',
  'about.html': '<!doctype html><h1 id="about">About</h1>',
});
const siteB = { 'index.html': '<!doctype html><h1 id="b">Site B</h1>' };
function publish(dir, port, cid, site) {
  const args = ['publish', '--rpc', `127.0.0.1:${port}`, '--chain-id', cid, '--account', 'alice', '--dir', dir]; if (site) args.push('--site', site);
  const r = spawnSync(BIN('pocol-site'), args, { encoding: 'utf8', timeout: 600000 }); if (r.status) throw new Error('publish: ' + r.stderr.slice(-400));
  return JSON.parse(r.stdout);
}

// ---------------- browser helpers ----------------
// 2026-10-28: every browser context records a Playwright trace (DOM snapshots and screenshots). One closed after
// a check failed while it was open, or still open when the driver stops, saves its trace into WORK/traces/, which
// is kept with the nodes' logs and the inputs when the run fails (keepOnFailure); any other trace is dropped.
const contexts = new Set(); let traces = 0;
async function saveTrace(c, why) { try { mkdirSync(join(work, 'traces'), { recursive: true }); await c.ctx.tracing.stop({ path: join(work, 'traces', `${++traces}-${c.tag}-${why}.zip`) }); } catch {} }
async function openBrowser(dist, tag) {
  const userDir = mkdtempSync(join(tmpdir(), 'pocol-chrome-' + tag + '-'));
  const ctx = await chromium.launchPersistentContext(userDir, { channel: 'chromium', headless: true, args: [`--disable-extensions-except=${dist}`, `--load-extension=${dist}`] });
  const rec = { ctx, tag, failedAtOpen: failed }; contexts.add(rec); await ctx.tracing.start({ screenshots: true, snapshots: true });
  let [sw] = ctx.serviceWorkers(); if (!sw) sw = await ctx.waitForEvent('serviceworker', { timeout: 20000 });
  const id = new URL(sw.url()).host; const opt = await ctx.newPage(); await opt.goto(`chrome-extension://${id}/options.html`);
  const send = (m) => opt.evaluate((x) => chrome.runtime.sendMessage(x), m);
  return { ctx, id, opt, send, close: async () => { contexts.delete(rec); if (failed > rec.failedAtOpen) await saveTrace(rec, 'failed'); else await ctx.tracing.stop().catch(() => {});
    await ctx.close(); rmSync(userDir, { recursive: true, force: true }); } };
}
async function siteFrame(page, minFrame = 1) {
  await page.waitForFunction((n) => { const f = document.querySelector('#stage iframe#site'); return f && Number(f.dataset.frame) >= n; }, minFrame, { timeout: 60000 });
  const el = await page.$('#stage iframe#site'); const n = Number(await el.getAttribute('data-frame')); const fr = await el.contentFrame();
  await fr.waitForFunction(() => document.documentElement.dataset.pocolReady === '1', null, { timeout: 30000 });
  return { fr, n };
}
async function openSite(b, name, site, path = '/') {
  const page = await b.ctx.newPage(); const errors = []; page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`chrome-extension://${b.id}/viewer.html#${name}/${site}${path}`);
  return { page, errors };
}
const probeNetwork = async (fr) => fr.evaluate(async ({ CANARY, UDP, HOST_IP }) => {
  const out = {}; const C = `http://127.0.0.1:${CANARY}`;
  const t = async (k, f) => { try { out[k] = String(await f()); } catch (e) { out[k] = 'blocked:' + e.name; } };
  await t('fetchAbs', () => fetch(C + '/fetch').then((r) => r.status));
  await t('img', () => new Promise((r) => { const i = new Image(); i.onload = () => r('loaded'); i.onerror = () => r('error'); i.src = C + '/img2'; document.body.appendChild(i); setTimeout(() => r('timeout'), 3000); }));
  await t('ws', () => typeof WebSocket);
  await t('rtcType', () => typeof RTCPeerConnection);
  await t('beacon', () => navigator.sendBeacon(C + '/beacon', 'x'));
  await t('iframeAppend', () => { const f = document.createElement('iframe'); f.src = C + '/f2'; document.body.appendChild(f); return 'inserted'; });
  await t('iframeInnerHTML', () => { const d = document.createElement('div'); d.innerHTML = `<iframe src="${C}/f3"></iframe><b>x</b>`; document.body.appendChild(d); return 'iframes=' + document.querySelectorAll('iframe').length; });
  await t('cssImport', () => { const s = document.createElement('style'); s.textContent = `@import url(${C}/css); body{background-image:url(${C}/bg)}`; document.head.appendChild(s); return 'added'; });
  await t('linkCss', () => { const l = document.createElement('link'); l.rel = 'stylesheet'; l.href = C + '/link.css'; document.head.appendChild(l); return 'added'; });
  await t('xhrAbs', () => new Promise((res) => { const x = new XMLHttpRequest(); x.open('GET', C + '/xhr'); x.onerror = () => res('error'); x.onload = () => res('load ' + x.status); x.send(); }));
  await t('windowOpen', () => String(window.open(C + '/popup')));
  await t('rtc', async () => {
    if (typeof RTCPeerConnection !== 'function') return 'no RTCPeerConnection';
    const pc = new RTCPeerConnection({ iceServers: [{ urls: `stun:${HOST_IP}:${UDP}` }] }); pc.createDataChannel('x');
    await pc.setLocalDescription(await pc.createOffer()); await new Promise((r) => setTimeout(r, 3000)); return 'gathering=' + pc.iceGatheringState;
  });
  // Last: an unblocked form POST would navigate (and tear down) the frame.
  await t('formPost', () => { const f = document.createElement('form'); f.action = C + '/form'; f.method = 'POST'; document.body.appendChild(f); setTimeout(() => f.submit(), 0); return 'submitted'; });
  await new Promise((r) => setTimeout(r, 1500));
  return out;
}, { CANARY, UDP, HOST_IP }).catch((e) => ({ evaluateError: e.message.slice(0, 120) }));
const raw = (fr, text) => fr.evaluate((t) => pocol.raw(t), text);
// pocol.navigate tears the calling frame down. Called within the evaluated function, the teardown can land before
// Playwright receives the evaluate's answer ("Frame was detached"); scheduled for a later task, it cannot.
// The navigation replaces the frame it runs in, and can do so before Playwright has the evaluate's own result: a store
// run on 051dfaa failed 'Frame was detached' 24 ms into navigate(fr, '/') (evidence/r4/accept-051dfaa/). A detach is
// the navigation happening; the caller's siteFrame wait is the check. Any other error is rethrown.
const navigate = (fr, path) => fr.evaluate((p) => { setTimeout(() => { pocol.navigate(p); }, 0); }, path)
  .catch((e) => { if (!/Frame was detached/.test(String(e && e.message))) throw e; });
const req = (fr, kind, method, params) => fr.evaluate(async ([k, m, p]) => { try { return { result: await pocol.request(k, m, p) }; } catch (e) { return { error: { code: e.code, message: e.message, data: e.data } }; } }, [kind, method, params]);
const msg = (id, kind, method, params) => JSON.stringify({ id, kind, payload: { method, params } });
/** Confirmed site dictionary as stored (D103): the largest (epoch, seq) record under the key. */
async function storedDict(b, dist, stKey) {
  const all = await b.opt.evaluate(() => chrome.storage.local.get(null));
  const names = Object.keys(all).filter((k) => k.startsWith(stKey + ':r:')).sort();
  if (!names.length) return { dict: null, names, epochs: Object.keys(all).filter((k) => k.startsWith('pocol:epoch:')) };
  const R = await import(pathToFileURL(join(dist, 'lib', 'records.js')).href);
  return { dict: R.decodeDict(all[names[names.length - 1]].b64), names, epochs: Object.keys(all).filter((k) => k.startsWith('pocol:epoch:')).sort() };
}

async function main() {
  for (const p of [30300, 30301, 30302, 8540, 8541, 8542, TAP, CANARY]) if (!(await portFree(p))) throw new Error('port busy ' + p);
  await new Promise((r) => tapServer.listen(TAP, '127.0.0.1', r)); await new Promise((r) => canaryServer.listen(CANARY, '0.0.0.0', r)); await new Promise((r) => udp.bind(UDP, '0.0.0.0', r));
  if (!REUSE) py(['init', work, '--miners', 'a,b,c', '--t-blk', '2', '--tau', '20', '--c', '8', '--d-att', '41', '--fb-wait', '3', '--work-bits', '15']);
  const info = JSON.parse(readFileSync(join(work, 'net', 'devnet.json'), 'utf8')); const cid = String(info.chainId);
  const profileBase = JSON.parse(readFileSync(join(work, 'net', 'profile.json'), 'utf8'));
  if (!REUSE) py(['start', work, '--log']);
  try {
    await wait(async () => Math.min(...await Promise.all([0, 1, 2].map(async (i) => Number(await rpc(8540 + i, 'eth_blockNumber'))))) >= 16, 180000, 'devnet height >= 16');
    // The state mode every node reports (POCOLD_RUN_ARGS=--state-store runs the suite in store mode).
    const modes = await Promise.all([0, 1, 2].map(async (i) => (await rpc(8540 + i, 'pocol_status')).stateStore));
    ok('every node runs in the requested state mode', modes.every((m) => m === (process.env.POCOLD_RUN_ARGS || '').split(/\s+/).includes('--state-store')), `stateStore=${modes}`);
    // Publish S_A (multi-file, multi-chunk) and S_B.
    let S_A, S_B;
    if (REUSE) ({ S_A, S_B } = JSON.parse(readFileSync(join(work, 'state.json'), 'utf8')));
    else {
      const dA = join(work, 'siteA1'); writeSite(dA, siteA(1)); S_A = publish(dA, 8540, cid).site;
      const dB = join(work, 'siteB'); writeSite(dB, siteB); S_B = publish(dB, 8541, cid).site;
      writeFileSync(join(work, 'state.json'), JSON.stringify({ S_A, S_B }));
    }
    await wait(async () => (await Promise.all([0, 1, 2].map((i) => rpc(8540 + i, 'eth_getCode', [S_B, 'latest'])))).every((c) => c.length > 100), 120000, 'sites on all nodes');
    ok('LP8 sites published into PoCol state through nodes a and b', S_A && S_B, `S_A=${S_A} S_B=${S_B}`);
    // Live RP window over Rust-produced headers, in Node with the same library the extension ships.
    const dist = build().dist;
    for (const c of await runLiveRp(dist, 'http://127.0.0.1:8540/', profileBase)) ok(c.name, c.pass, c.detail);

    // ================= release build =================
    const b = await openBrowser(dist, 'rel');
    const prof = { ...profileBase, endpoint: `http://127.0.0.1:${TAP}` };
    const added = await b.send({ type: 'profiles.add', profile: prof }); ok('profile accepted (genesisPre hashes to genesisHash, chainId matches)', added.ok, added.ok ? added.profile.netKey : JSON.stringify(added.error));
    for (const [i, l] of ['a', 'b', 'c'].entries()) await b.send({ type: 'profiles.add', profile: { ...profileBase, name: 'node-' + l, endpoint: `http://127.0.0.1:${8540 + i}`, trustLevel: 'LN' } });
    const bad = await b.send({ type: 'profiles.add', profile: { ...prof, name: 'tampered', genesisHash: '0x' + '00'.repeat(32) } }); ok('profile with a genesisHash that genesisPre does not hash to is rejected', !bad.ok, JSON.stringify(bad.error));
    // A real other network (different genesis time) pointed at this endpoint must be refused (netKey binding).
    const other = join(work, 'other'); spawnSync(BIN('pocold'), ['init-devnet', '--out', other, '--miners', 'a,b,c', '--chain-id', cid, '--genesis-time', '1700000000'], { encoding: 'utf8' });
    const oHex = readFileSync(join(other, 'genesis.spec.hex'), 'utf8').trim(); const oInfo = JSON.parse(readFileSync(join(other, 'devnet.json'), 'utf8'));
    await b.send({ type: 'profiles.add', profile: { ...prof, name: 'othernet', genesisPre: oHex, genesisHash: oInfo.genesisHash } });

    // --- render (D26), RP trust banner ---
    tap.log.length = 0;
    const { page, errors } = await openSite(b, 'devnet', S_A); let { fr, n } = await siteFrame(page);
    const trust = await page.textContent('#trust');
    ok('RP trust banner shown outside the frame (13 headers, work bound)', /^RP: مرتبط بالعمل والهوية لـ13 رأسًا/.test(trust) && (await page.getAttribute('#trust', 'data-level')) === 'RP', trust.slice(0, 80));
    const view = await fr.evaluate(async () => { await new Promise((r) => setTimeout(r, 800)); const h = document.getElementById('t'); const img = document.getElementById('logo');
      return { h1: h.textContent, color: getComputedStyle(h).color, bg: getComputedStyle(document.body).backgroundColor, loaded: h.dataset.loaded, inline: h.dataset.inline || null, dcl: document.body.dataset.dcl, img: img.naturalWidth,
        cssBg: getComputedStyle(document.getElementById('bg')).backgroundImage.slice(0, 9), fetch: document.body.dataset.fetch, xhr: document.body.dataset.xhr, iframes: document.querySelectorAll('iframe,object,embed').length,
        title: document.title, jsHref: document.getElementById('js').getAttribute('href'), refresh: !!document.querySelector('meta[http-equiv]'), prefetch: !!document.querySelector('link[rel=prefetch]') }; });
    ok('site rendered from verified chunks: HTML + @import CSS + JS + PNG + url() in CSS', view.h1 === 'PoCol on-chain site v1' && view.color === 'rgb(17, 170, 85)' && view.bg === 'rgb(244, 246, 248)' && view.loaded === 'yes' && view.img === 2 && view.cssBg === 'url("blob', JSON.stringify(view));
    ok('scripts run in order from blob:, DOMContentLoaded delivered, fetch/XHR served from the file map', view.inline === 'yes' && view.dcl === 'yes' && view.fetch === '1400' && view.xhr === '1400', JSON.stringify(view));
    ok('sanitizer removed iframe/object/meta refresh/prefetch, on* handlers and javascript: URLs', view.iframes === 0 && !view.refresh && !view.prefetch && view.jsHref === null && view.title !== 'handler-ran', JSON.stringify(view));
    ok('RpcTap: site load used only read methods (pocol_status, pocol_getHeaders, eth_call, eth_getCode, eth_chainId, eth_blockNumber)', tapMethods().every((m) => ['eth_chainId', 'pocol_status', 'eth_blockNumber', 'pocol_getHeaders', 'eth_call', 'eth_getCode'].includes(m)), [...new Set(tapMethods())].join(','));
    ok('RpcTap: extension requests carry the extension Origin (ORIGIN_DENY applies)', tap.log.every((x) => x.origin === `chrome-extension://${b.id}`), tap.log[0] && tap.log[0].origin);

    // --- network isolation (layers 1-3): HTTP and UDP canaries stay at zero ---
    canary.http.length = 0; canary.udp = 0;
    const probe = await probeNetwork(fr); await sleep(1500);
    ok('isolation probes: WebSocket/RTCPeerConnection undefined, nested frames refused, fetch/XHR to network fail', probe.ws === 'undefined' && probe.rtcType === 'undefined' && probe.iframeAppend.startsWith('blocked:NotSupportedError') && probe.iframeInnerHTML === 'iframes=0' && probe.fetchAbs.startsWith('blocked:TypeError') && probe.xhrAbs === 'error' && probe.beacon === 'false', JSON.stringify(probe));
    ok('HTTP canary: zero requests from the site (img/css/@import/link/beacon/form/popup/frames)', canary.http.length === 0, canary.http.join(' | '));
    ok('UDP (STUN) canary: zero packets', canary.udp === 0, canary.udp);
    ok('frame alive after probes; legitimate read succeeds', /^0x[0-9a-f]+$/.test((await req(fr, 'rpc_read', 'eth_blockNumber', [])).result || ''));

    // --- BridgeAuth end-to-end (BR1-BR13 subset): zero forbidden requests reach the node ---
    tap.log.length = 0;
    const codes = {};
    for (const [k, text] of [['BR1', msg(1, 'rpc_read', 'eth_sendRawTransaction', ['0x02'])], ['BR2', msg(2, 'rpc_read', 'eth_sendTransaction', [{ to: S_B }])], ['BR3a', msg(3, 'rpc_read', 'pocol_submitShare', [])],
      ['BR3b', msg(4, 'rpc_read', 'pocol_getTemplate', [])], ['BR4', msg(5, 'rpc_read', 'ETH_SENDRAWTRANSACTION', [])], ['BR5', JSON.stringify({ id: 6, kind: 'rpc_read', payload: [{ method: 'eth_blockNumber', params: [] }] })],
      ['BR6', '{"id":7,"kind":"rpc_read","payload":{"method":"eth_blockNumber","method":"eth_sendRawTransaction","params":[]}}'], ['BR7b', msg(8, 'wallet_req', 'eth_sendRawTransaction', ['0x02'])],
      ['BR9', msg(9, 'rpc_read', 'eth_call', [{ to: S_A, data: '0x', input: '0x' }, 'latest'])], ['BR11', msg(10, 'nav', '__proto__', [])], ['BR12', msg(11, 'rpc_read', 'eth_getBlockByNumber', ['latest', true])],
      ['BR13', 'x'.repeat(65537)], ['debug', msg(12, 'rpc_read', 'debug_traceTransaction', [])], ['getProof', msg(13, 'rpc_read', 'eth_getProof', [S_A, [], 'latest'])]]) {
      const r = await raw(fr, text); codes[k] = r.error ? r.error.code : 'ok';
    }
    const want = { BR1: 4200, BR2: 4200, BR3a: 4200, BR3b: 4200, BR4: 4200, BR5: -32600, BR6: -32600, BR7b: 4200, BR9: -32602, BR11: 4200, BR12: -32602, BR13: -32600, debug: 4200, getProof: 4200 };
    ok('BridgeAuth replies in Chrome match BR1-BR13', JSON.stringify(codes) === JSON.stringify(want), JSON.stringify(codes));
    ok('RpcTap: zero requests for rejected messages', tap.log.length === 0, tapMethods().join(','));
    // BR10: eth_getLogs goes to LogClient (pocol_getLogs), never raw eth_getLogs; > 1024 blocks rejected locally.
    tap.log.length = 0;
    const lg = await req(fr, 'rpc_read', 'eth_getLogs', [{ fromBlock: '0x1', toBlock: '0x10' }]);
    const lgBig = await req(fr, 'rpc_read', 'eth_getLogs', [{ fromBlock: '0x1', toBlock: '0x401' }]);
    ok('BR10 eth_getLogs -> pocol_getLogs only; 1..1025 -> -32020 {range} without request', Array.isArray(lg.result) && tapCount('eth_getLogs') === 0 && tapCount('pocol_getLogs') === 1 && lgBig.error.code === -32020, JSON.stringify({ n: lg.result && lg.result.length, m: tapMethods(), big: lgBig.error }));
    // BR19b in Chrome: pendingReads <= 4 per session (eth_call held 2.5 s by the tap).
    tap.holdMs.eth_call = 2500;
    const five = await fr.evaluate(async (to) => Promise.all([0, 1, 2, 3, 4].map(() => pocol.request('rpc_read', 'eth_call', [{ to, data: '0x9fa6a6e3' }, 'latest']).then(() => 'ok', (e) => e.code + ':' + (e.data && e.data.reason)))), S_A);
    delete tap.holdMs.eth_call;
    ok('BR19b pendingReads: 4 forwarded, the 5th -32005 {pending}', five.filter((x) => x === 'ok').length === 4 && five.filter((x) => x === '-32005:pending').length === 1, five.join(','));
    // BR19c: 60 messages in one synchronous loop -> at least one rate rejection; the frame keeps working.
    const burst = await fr.evaluate(async () => { const ps = []; for (let i = 0; i < 60; i++) ps.push(pocol.request('rpc_read', 'eth_chainId', []).then(() => 'ok', (e) => e.code + ':' + (e.data && e.data.reason))); return Promise.all(ps); });
    await sleep(2100);
    ok('BR19c burst of 60: rate rejections occur, later read succeeds', burst.includes('-32005:rate') && burst.filter((x) => x === 'ok').length >= 1 && /^0x/.test((await req(fr, 'rpc_read', 'eth_chainId', [])).result), `ok=${burst.filter((x) => x === 'ok').length} rate=${burst.filter((x) => x === '-32005:rate').length} pending=${burst.filter((x) => x === '-32005:pending').length}`);

    // --- site storage (BR17) ---
    await sleep(1100);
    await fr.evaluate(async () => { localStorage.setItem('k', 'v'); localStorage.setItem('z', 'zz'); localStorage.removeItem('z'); await pocol.flushStorage(); });
    const stKey = 'site:' + added.profile.netKey + ':' + S_A.toLowerCase();
    const stored = await storedDict(b, dist, stKey);
    ok('BR17a localStorage write persisted through BridgeAuth as a versioned record (D103)', stored.dict && stored.dict.k === 'v' && !('z' in stored.dict) && stored.names.length === 1 && /:r:[0-9a-f]{16}:[0-9a-f]{8}$/.test(stored.names[0]), JSON.stringify({ d: stored.dict, n: stored.names.map((x) => x.slice(-26)) }));
    await page.reload(); ({ fr, n } = await siteFrame(page));
    ok('BR17a after reload localStorage.getItem(k) = v (new session, same netKey/site)', (await fr.evaluate(() => localStorage.getItem('k'))) === 'v');
    // BR17e quota table over the real bridge (24 messages, each awaited).
    await fr.evaluate(() => pocol.request('storage_set', 'site_storageClear', []));
    const tableCodes = await fr.evaluate(async () => {
      const A = 'a'.repeat(61440); const out = []; const s = (k, v) => pocol.request('storage_set', 'site_storageSet', [k, v]).then(() => 'null', (e) => String(e.code));
      for (let i = 0; i < 17; i++) { out.push(await s('k' + String(i).padStart(2, '0'), A)); await new Promise((r) => setTimeout(r, 25)); }
      for (const [k, v] of [['k17', A], ['k17', 'a'.repeat(4043)], ['k17', 'a'.repeat(4042)], ['k18', 'x'], ['k17', 'a'.repeat(4043)], ['k00', 'b'], ['k01', null]]) { out.push(await s(k, v)); await new Promise((r) => setTimeout(r, 25)); }
      return out;
    });
    const wantTable = [...Array(17).fill('null'), '4300', '4300', 'null', '4300', '4300', 'null', 'null'];
    ok('BR17e quota table (e1-e8) through Chrome', JSON.stringify(tableCodes) === JSON.stringify(wantTable), tableCodes.join(','));
    const dict = (await storedDict(b, dist, stKey)).dict;
    const tot = Object.entries(dict).reduce((a, [k, v]) => a + Buffer.byteLength(k) + Buffer.byteLength(v), 0);
    ok('BR17e(e9) stored dictionary total = 925694, k00=b, k01/k18 absent, k17 = 4042 bytes', tot === 925694 && dict.k00 === 'b' && !('k01' in dict) && !('k18' in dict) && dict.k17.length === 4042, tot);
    // BR17f isolation: S_B on the same network does not see S_A's keys and has its own quota.
    const pB = await openSite(b, 'devnet', S_B); const fB = (await siteFrame(pB.page)).fr;
    const iso = await fB.evaluate(async () => ({ k00: localStorage.getItem('k00'), w: await pocol.request('storage_set', 'site_storageSet', ['k', 'v']).then(() => 'null', (e) => e.code) }));
    ok('BR17f S_B sees none of S_A keys; its quota is independent', iso.k00 === null && iso.w === 'null', JSON.stringify(iso));
    await pB.page.close();
    // D103: the worker generation ends; frames are torn down with the loss notice; a user reload recovers in a new generation.
    const cdpS = await b.ctx.newCDPSession(b.opt); await cdpS.send('ServiceWorker.enable').catch(() => {}); await cdpS.send('ServiceWorker.stopAllWorkers');
    await page.waitForSelector('#reload', { timeout: 20000 }); const lost = await page.textContent('#stage');
    await page.click('#reload'); ({ fr, n } = await siteFrame(page));
    const rec = await fr.evaluate(() => ({ k00: localStorage.getItem('k00'), k17: (localStorage.getItem('k17') || '').length, k01: localStorage.getItem('k01') }));
    const st2 = await storedDict(b, dist, stKey);
    ok('D103 worker restart: frame torn down with the loss notice; reload recovers the committed dictionary under epoch 2',
      /انقطع عامل الإضافة/.test(lost) && rec.k00 === 'b' && rec.k17 === 4042 && rec.k01 === null && st2.epochs.some((e) => e.endsWith('0000000000000002')) && st2.names.every((x) => x.includes(':r:0000000000000002:')),
      JSON.stringify({ rec, epochs: st2.epochs.map((e) => e.slice(-4)), names: st2.names.map((x) => x.slice(-26)) }));

    // --- navigation (BR18, D26/D99) ---
    const pagesBefore = b.ctx.pages().length;
    await navigate(fr, '/about.html');
    ({ fr, n } = await siteFrame(page, n + 1));
    ok('BR18a site_navigate(/about.html): old frame torn down, new frame shows about.html, no new tab', (await fr.textContent('#about')) === 'About' && (await page.$$('#stage iframe')).length === 1 && b.ctx.pages().length === pagesBefore, `frame ${n}`);
    await navigate(fr, '/docs/'); ({ fr, n } = await siteFrame(page, n + 1));
    ok('BR18b /docs/ -> docs/index.html', (await fr.textContent('#docs')).startsWith('Docs page'));
    // A link in the site replaces the frame it was clicked in (BR18), so Playwright's click on that frame can stay
    // pending after the navigation is done: on 051dfaa the click on #docs timed out at 30 s while the trace shows the
    // viewer on /docs/index.html in a new frame (evidence/r4/accept-051dfaa/), as the #home click did on ad301e7.
    // The click is a real one; the check is the new frame, not the click's return.
    const clickAway = (f, sel) => { f.click(sel).catch(() => {}); };
    clickAway(fr, '#home'); ({ fr, n } = await siteFrame(page, n + 1));
    ok('link interception: relative ../ link -> site_navigate -> entry', (await fr.textContent('#t')) === 'PoCol on-chain site v1');
    // D99 nav bucket: three navigations in a row, then one per 2 s; the 4th is refused without teardown.
    const fourth = await req(fr, 'nav', 'site_navigate', ['/about.html']);
    ok('D99 4th rapid navigation -> -32005 {nav}, frame kept', fourth.error && fourth.error.code === -32005 && fourth.error.data.reason === 'nav' && (await fr.textContent('#t')) === 'PoCol on-chain site v1', JSON.stringify(fourth));
    await sleep(2100);
    clickAway(fr, '#docs'); ({ fr, n } = await siteFrame(page, n + 1));
    ok('link interception: <a href=docs/> navigates inside the site', (await fr.textContent('#docs')).startsWith('Docs page'));
    await sleep(2100); await navigate(fr, '/a/../b.html'); ({ fr, n } = await siteFrame(page, n + 1));
    ok('BR18c /a/../b.html -> viewer 404 page', (await fr.textContent('h1')) === '404' && (await page.textContent('#status')) === '404');
    await sleep(2100); await navigate(fr, '/'); ({ fr, n } = await siteFrame(page, n + 1));
    // External links: confirm outside the frame.
    const extP = fr.evaluate(() => pocol.openExternal('https://example.org/').then((r) => ({ r }), (e) => ({ code: e.code })));
    await page.waitForSelector('#modal:not([hidden])'); const shown = await page.textContent('#mtext');
    const second = await req(fr, 'nav', 'site_openExternal', ['https://example.org/second']);
    ok('BR18h second confirm while one is pending -> -32005 {pending}', second.error && second.error.code === -32005 && second.error.data.reason === 'pending', JSON.stringify(second));
    // The new tab (not the site frame) navigates; the container has no internet, so CDP history is the evidence.
    const newPage = b.ctx.waitForEvent('page', { timeout: 10000 });
    await page.click('#approve'); const extR = await extP; const opened = await newPage; await sleep(1000);
    const hist = (await (await b.ctx.newCDPSession(opened)).send('Page.getNavigationHistory')).entries.map((e) => e.url);
    ok('BR18e openExternal: confirm shows the full URL, approve opens one new tab that navigates to it, reply null',
      shown === 'https://example.org/' && extR.r === null && hist.includes('https://example.org/') && canary.http.length === 0, `${shown} ${JSON.stringify(extR)} history=${hist.join(',')}`);
    await opened.close();
    const rejP = fr.evaluate(() => pocol.openExternal('https://example.org/').then(() => 'ok', (e) => e.code));
    await page.waitForSelector('#modal:not([hidden])'); const pagesNow = b.ctx.pages().length; await page.click('#reject');
    ok('BR18f reject -> 4001 and no tab', (await rejP) === 4001 && b.ctx.pages().length === pagesNow);
    // D26 step 6: a second load event tears the frame down.
    await fr.evaluate(() => { setTimeout(() => { location.href = 'sandbox.html?second'; }, 50); }).catch(() => {});
    await page.waitForFunction(() => !document.querySelector('#stage iframe'), null, { timeout: 15000 });
    ok('D26 a self-navigating frame is torn down on its second load', /D26/.test(await page.textContent('#status')));

    // --- wallet: BR14 legitimate path, rejection and wrong from; A06 transitions in Chrome ---
    await b.send({ type: 'wallet.create', password: 'devnet-only', seedHex: '0x' + '42'.repeat(32) });
    const wAddr = (await b.send({ type: 'wallet.address', name: 'devnet' })).address;
    const aliceNonce = Number(await rpc(8540, 'eth_getTransactionCount', [info.accounts.find((a) => a.label === 'alice').address, 'pending']));
    const fund = spawnSync(BIN('pocol-sign'), ['--chain-id', cid, '--account', 'alice', '--to', wAddr, '--value', '1000000000000000000', '--nonce', String(aliceNonce), '--gas', '21000'], { encoding: 'utf8' }).stdout.trim();
    await rpc(8540, 'eth_sendRawTransaction', [fund]); await wait(async () => BigInt(await rpc(8540, 'eth_getBalance', [wAddr, 'latest'])) > 0n, 90000, 'wallet funded');
    await page.goto(`chrome-extension://${b.id}/viewer.html#devnet/${S_A}/`); await page.reload(); ({ fr, n } = await siteFrame(page));
    tap.log.length = 0;
    // A06: 'othernet' shares the chainId with another genesis, so both profiles are read-only until it is removed.
    const listed = (await b.send({ type: 'profiles.list' })).profiles;
    const conflictReq = await fr.evaluate(() => ethereum.request({ method: 'eth_requestAccounts' }).then(() => 'ok', (e) => e.code));
    ok('A06 Chrome: a profile with the same chainId and another genesis flags both as conflicting; eth_requestAccounts -> 4100 without a dialog',
      listed.find((x) => x.name === 'devnet').conflict === true && listed.find((x) => x.name === 'othernet').conflict === true && conflictReq === 4100 && (await page.isHidden('#modal')), `${conflictReq} ${JSON.stringify(listed.map((x) => [x.name, x.conflict]))}`);
    await b.send({ type: 'profiles.remove', name: 'othernet' });
    const accP = fr.evaluate(() => ethereum.request({ method: 'eth_requestAccounts' }));
    await page.waitForSelector('#modal[data-kind=confirmAccounts]:not([hidden])'); const armedAtOnce = await page.$eval('#approve', (b) => !b.disabled);
    ok('A06 Chrome: approve is not armed the moment a dialog appears (a double click cannot approve it)', armedAtOnce === false);
    await page.click('#approve');
    const accts = await accP; ok('BR14 eth_requestAccounts after click returns the per-network wallet address', accts.length === 1 && accts[0].toLowerCase() === wAddr.toLowerCase(), accts[0]);
    const s2 = info.accounts.find((a) => a.label === 'bob').address;
    const txP = fr.evaluate(([from, to]) => ethereum.request({ method: 'eth_sendTransaction', params: [{ from, to, value: '0x1' }] }), [accts[0], s2]);
    await page.waitForSelector('#modal[data-kind=confirmTx]:not([hidden])'); const txText = await page.textContent('#mtext'); const atModal = tap.log.length;
    // A06: the dialog shows the unsigned bytes and their signing hash; signing happens only after the click.
    const shownHash = /signingHash: (0x[0-9a-f]{64})/.exec(txText)[1], shownUnsigned = /unsigned: (0x[0-9a-f]+)/.exec(txText)[1];
    ok('BR14 no eth_sendRawTransaction before the click', tapCount('eth_sendRawTransaction') === 0);
    await page.click('#approve'); const txHash = await txP;
    const C = await import(pathToFileURL(join(dist, 'lib', 'crypto.js')).href), HX = await import(pathToFileURL(join(dist, 'lib', 'hex.js')).href), RL = await import(pathToFileURL(join(dist, 'lib', 'rlp.js')).href);
    const sent = tap.log.filter((x) => x.method === 'eth_sendRawTransaction');
    const sraw = sent.length ? HX.fromHex(sent[0].params[0]) : new Uint8Array(1); let spre = null, signer = null;
    try { const f = RL.decode(sraw.subarray(1)); spre = HX.toHex(HX.concat(Uint8Array.of(2), RL.encode(f.slice(0, 9)))); const pad = (x) => HX.concat(new Uint8Array(32 - x.length), x);
      signer = HX.toHex(C.recover(C.keccak(HX.fromHex(spre)), HX.concat(pad(f[10]), pad(f[11]), Uint8Array.of(f[9].length ? f[9][0] : 0)))); } catch {}
    ok('BR14 exactly one eth_sendRawTransaction; its signed preimage = displayed unsigned bytes, keccak of them = displayed signingHash, signer = wallet, result = keccak(raw)',
      sent.length === 1 && spre === shownUnsigned && HX.toHex(C.keccak(HX.fromHex(shownUnsigned))) === shownHash && signer === wAddr.toLowerCase() && txHash === HX.toHex(C.keccak(sraw)), `${sent.length} ${signer} ${txHash}`);
    const after = tap.log.slice(atModal).map((x) => x.method);
    ok('A06 Chrome: endpoint identity re-checked after the click, before the submission (eth_chainId, pocol_status, then eth_sendRawTransaction)',
      after.indexOf('pocol_status') >= 0 && after.indexOf('eth_chainId') >= 0 && after.indexOf('pocol_status') < after.indexOf('eth_sendRawTransaction'), after.join(','));
    // Every poll reads all three nodes: a receipt read once can name a block a short fork then abandons (a node's own
    // block at the inclusion height, replaced when the other branch is extended), and the others never report it.
    const rcs = await wait(async () => { const r = await Promise.all([8540, 8541, 8542].map((p) => rpc(p, 'eth_getTransactionReceipt', [txHash]))); return r.every((x) => x && x.status === '0x1' && x.blockHash === r[0].blockHash) && r; }, 150000, 'receipts');
    const rc = rcs[0];
    ok('BR14 transaction included by the devnet (status 1) and identical receipt on all nodes', rc.status === '0x1', `${rc.blockNumber} ${rc.blockHash}`);
    const rejTx = fr.evaluate(([from, to]) => ethereum.request({ method: 'eth_sendTransaction', params: [{ from, to, value: '0x1' }] }).then(() => 'ok', (e) => e.code), [accts[0], s2]);
    await page.waitForSelector('#modal[data-kind=confirmTx]:not([hidden])'); await page.click('#reject');
    ok('BR14 rejected signature -> 4001 and no further eth_sendRawTransaction', (await rejTx) === 4001 && tapCount('eth_sendRawTransaction') === 1);
    const wrongFrom = await fr.evaluate((to) => ethereum.request({ method: 'eth_sendTransaction', params: [{ from: '0x' + '99'.repeat(20), to, value: '0x1' }] }).then(() => 'ok', (e) => e.code), s2);
    ok('BR14 from not granted -> 4100', wrongFrom === 4100 && tapCount('eth_sendRawTransaction') === 1);
    // A06 in Chrome: transitions while the confirmation is open. A fresh session (reload) keeps the
    // wallet requests inside WALLET_PER_MIN = 5 per session and minute; the grant is per session.
    await page.reload(); ({ fr, n } = await siteFrame(page));
    const regrant = fr.evaluate(() => ethereum.request({ method: 'eth_requestAccounts' }));
    await page.waitForSelector('#modal[data-kind=confirmAccounts]:not([hidden])'); await page.click('#approve'); await regrant;
    const pendingTx = () => fr.evaluate(([from, to]) => ethereum.request({ method: 'eth_sendTransaction', params: [{ from, to, value: '0x1' }] }).then(() => 'ok', (e) => e.code), [accts[0], s2]);
    const modalTx = () => page.waitForSelector('#modal[data-kind=confirmTx]:not([hidden])');
    const modalGone = () => page.waitForSelector('#modal', { state: 'hidden', timeout: 5000 }).then(() => true, () => false);
    let pTx = pendingTx(); await modalTx(); await b.send({ type: 'wallet.lock' });
    const lockCode = await pTx;
    ok('A06 Chrome: wallet.lock while the confirmation is open -> 4100, dialog withdrawn, nothing sent', lockCode === 4100 && (await modalGone()) && tapCount('eth_sendRawTransaction') === 1, lockCode);
    await b.send({ type: 'wallet.unlock', password: 'devnet-only' });
    pTx = pendingTx(); await modalTx(); await b.send({ type: 'profiles.add', profile: { ...profileBase, name: 'node-x', endpoint: 'http://127.0.0.1:8541', trustLevel: 'LN' } });
    const profCode = await pTx;
    ok('A06 Chrome: a profile change while the confirmation is open -> 4901, dialog withdrawn, nothing sent; the unchanged session stays', profCode === 4901 && (await modalGone()) && tapCount('eth_sendRawTransaction') === 1 && (await page.$('#stage iframe#site')) !== null, profCode);
    // The frame is removed by the revocation, so the in-frame promise ends with a detached frame.
    pTx = pendingTx().catch(() => 'frame detached'); await modalTx(); tap.genesisOverride = '0x' + '77'.repeat(32); await page.click('#approve');
    await page.waitForSelector('#stage .err', { timeout: 20000 }).catch(() => {}); const revokedText = await page.textContent('#stage'); await pTx;
    tap.genesisOverride = null;
    ok('A06 Chrome: endpoint genesis changed before signing -> the frame is torn down with 4901, nothing sent', /4901/.test(revokedText) && tapCount('eth_sendRawTransaction') === 1 && !(await page.$('#stage iframe')), revokedText);
    // browser.md: the identity is also checked every 30 s; an idle session on a changed endpoint ends.
    const pI = await openSite(b, 'devnet', S_A); const fI = (await siteFrame(pI.page)).fr;
    // A dialog is open when the session ends: it must be withdrawn with the frame.
    fI.evaluate(() => pocol.openExternal('https://example.org/')).catch(() => {}); await pI.page.waitForSelector('#modal[data-kind=confirmExternal]:not([hidden])');
    tap.genesisOverride = '0x' + '77'.repeat(32); const tI = Date.now();
    await pI.page.waitForSelector('#stage .err', { timeout: 45000 }).catch(() => {}); const dI = Date.now() - tI; const idleText = await pI.page.textContent('#stage'); tap.genesisOverride = null;
    ok('A06 Chrome: periodic identity check ends an idle session whose endpoint genesis changed (4901 within 30 s + slack)', /4901/.test(idleText) && dI <= 40000, `${dI} ms: ${idleText}`);
    ok('A06 Chrome: the open dialog is withdrawn when the session is revoked', await pI.page.isHidden('#modal'));
    await pI.page.close();

    // --- A06 round 2 in Chrome: submission states over a held transport queue ---
    // Four sessions each hold an 8 MiB pocol_getLogs at the tap, so SITE_POOL (32 MiB) is full and the
    // approved, signed transaction waits for its transport reservation (queued, not yet sent). The
    // transaction is built and shown first (its reads use the same pool); the reads start while the
    // dialog is open. The pending nonce on all three nodes shows whether anything reached any node.
    await page.reload(); ({ fr, n } = await siteFrame(page));
    const regrant2 = fr.evaluate(() => ethereum.request({ method: 'eth_requestAccounts' }));
    await page.waitForSelector('#modal[data-kind=confirmAccounts]:not([hidden])'); await page.click('#approve'); await regrant2;
    const extra = []; for (let i = 0; i < 3; i++) { const o = await openSite(b, 'devnet', S_A); extra.push({ ...o, fr: (await siteFrame(o.page)).fr }); }
    const headBlk = await rpc(8540, 'eth_getBlockByNumber', ['latest', false]);
    const txState = () => fr.evaluate(([from, to]) => ethereum.request({ method: 'eth_sendTransaction', params: [{ from, to, value: '0x1' }] }).then((h) => ({ ok: h }), (e) => ({ code: e.code, data: e.data })), [accts[0], s2]).catch(() => ({ detached: true }));
    const holdQueue = async () => {
      tap.holdMs.pocol_getLogs = 6000; const l0 = tapCount('pocol_getLogs');
      for (const f of [fr, ...extra.map((x) => x.fr)]) f.evaluate((h) => ethereum.request({ method: 'eth_getLogs', params: [{ blockHash: h }] }), headBlk.hash).catch(() => {});
      await wait(() => tapCount('pocol_getLogs') >= l0 + 4, 15000, 'four held reads');
    };
    const approveQueued = async () => { await modalTx(); await holdQueue(); const st = tapCount('pocol_status'); await page.click('#approve'); await wait(() => tapCount('pocol_status') > st, 15000, 'identity at signing'); await sleep(300); };
    const drained = async () => { delete tap.holdMs.pocol_getLogs; await sleep(6500); };
    const nonces = async () => JSON.stringify(await Promise.all([8540, 8541, 8542].map((p) => rpc(p, 'eth_getTransactionCount', [wAddr, 'pending']))));
    const cancelledQ = (r, c) => r.code === c && r.data && r.data.submission === 'cancelled' && r.data.before === 'queued';
    let sent0 = tapCount('eth_sendRawTransaction'), n0 = await nonces();
    let pQ = txState(); await approveQueued(); await b.send({ type: 'wallet.lock' }); let rQ = await pQ; await drained();
    ok('A06-R2 Chrome: wallet.lock while the signed transaction waits for transport -> 4100 {submission: cancelled, before: queued}; never sent, also after the queue drains',
      cancelledQ(rQ, 4100) && tapCount('eth_sendRawTransaction') === sent0 && (await nonces()) === n0, JSON.stringify(rQ));
    await b.send({ type: 'wallet.unlock', password: 'devnet-only' });
    sent0 = tapCount('eth_sendRawTransaction'); n0 = await nonces();
    pQ = txState(); await approveQueued(); await b.send({ type: 'profiles.add', profile: { ...profileBase, name: 'node-y', endpoint: 'http://127.0.0.1:8542', trustLevel: 'LN' } }); rQ = await pQ; await drained();
    ok('A06-R2 Chrome: a profile change while queued -> 4901 cancelled before queued, never sent; the unchanged session stays',
      cancelledQ(rQ, 4901) && tapCount('eth_sendRawTransaction') === sent0 && (await nonces()) === n0 && (await page.$('#stage iframe#site')) !== null, JSON.stringify(rQ));
    // Initiated: the request has reached the tap (held there); a lock now cannot recall it.
    sent0 = tapCount('eth_sendRawTransaction'); tap.holdMs.eth_sendRawTransaction = 3000;
    pQ = txState(); await modalTx(); await page.click('#approve'); await wait(() => tapCount('eth_sendRawTransaction') > sent0, 15000, 'submission initiated');
    await b.send({ type: 'wallet.lock' }); rQ = await pQ; delete tap.holdMs.eth_sendRawTransaction;
    const rcI = typeof rQ.ok === 'string' ? await wait(() => rpc(8540, 'eth_getTransactionReceipt', [rQ.ok]), 90000, 'receipt') : null;
    ok('A06-R2 Chrome: wallet.lock after the request was initiated is not reported as a cancellation; the transaction is broadcast and included', typeof rQ.ok === 'string' && rcI && rcI.status === '0x1', JSON.stringify(rQ));
    await b.send({ type: 'wallet.unlock', password: 'devnet-only' });
    // The session's own endpoint changes while queued: the session is revoked and nothing is sent to either endpoint.
    sent0 = tapCount('eth_sendRawTransaction'); n0 = await nonces();
    pQ = txState(); await approveQueued(); await b.send({ type: 'profiles.add', profile: { ...prof, endpoint: 'http://127.0.0.1:8541' } });
    await page.waitForSelector('#stage .err', { timeout: 20000 }).catch(() => {}); const epText = await page.textContent('#stage'); await pQ; await drained();
    ok('A06-R2 Chrome: the session profile endpoint changed while queued -> session revoked (4901); nothing reached the old or the new endpoint (pending nonce unchanged on every node)',
      /4901/.test(epText) && tapCount('eth_sendRawTransaction') === sent0 && (await nonces()) === n0, epText);
    await b.send({ type: 'profiles.add', profile: prof }); await b.send({ type: 'profiles.remove', name: 'node-y' });
    for (const x of extra) await x.page.close();
    await page.reload(); ({ fr, n } = await siteFrame(page));

    // --- netKey binding: another network's profile against this endpoint is refused ---
    await b.send({ type: 'profiles.add', profile: { ...prof, name: 'othernet', genesisPre: oHex, genesisHash: oInfo.genesisHash } });
    const pO = await openSite(b, 'othernet', S_A); await pO.page.waitForSelector('#stage .err', { timeout: 30000 });
    ok('profile of another genesis pointed at this node is refused (netKey binding, 4901)', /genesis differs/.test(await pO.page.textContent('#stage')), await pO.page.textContent('#stage'));
    await pO.page.close();
    // --- update: v2 published through node c, read through every node in Chrome ---
    const dA2 = join(work, 'siteA2'); writeSite(dA2, siteA(2)); publish(dA2, 8542, cid, S_A);
    await wait(async () => (await Promise.all([0, 1, 2].map((i) => rpc(8540 + i, 'eth_call', [{ to: S_A, data: '0x9fa6a6e3' }, 'latest'])))).every((x) => BigInt('0x' + x.slice(2, 66)) === 1n), 120000, 'v2 on all nodes');
    const per = [];
    for (const l of ['a', 'b', 'c']) { const p = await openSite(b, 'node-' + l, S_A); const f = (await siteFrame(p.page)).fr; per.push([await f.textContent('#t'), await p.page.textContent('#ver'), await p.page.getAttribute('#trust', 'data-level')]); await p.page.close(); }
    ok('v2 update read in Chrome through each of the 3 nodes (LN profiles), identical content', per.every((x) => x[0] === 'PoCol on-chain site v2' && x[1].startsWith('version 1') && x[2] === 'LN'), JSON.stringify(per));
    ok('viewer pages raised no uncaught errors', errors.length === 0, errors.join(' | '));
    const allMethods = new Set(tapMethods());
    ok('RpcTap never saw pocol_submitShare/pocol_getTemplate/raw eth_getLogs', !allMethods.has('pocol_submitShare') && !allMethods.has('pocol_getTemplate') && !allMethods.has('eth_getLogs'), [...allMethods].join(','));
    await b.close();

    // ================= negative controls (fault builds) =================
    const runProbe = async (fault) => {
      const fb = await openBrowser(build({ fault }).dist, fault); await fb.send({ type: 'profiles.add', profile: prof });
      const p = await openSite(fb, 'devnet', S_A); const f = (await siteFrame(p.page)).fr;
      canary.http.length = 0; canary.udp = 0; const pr = await probeNetwork(f); await sleep(2000);
      const res = { probe: pr, http: canary.http.slice(), udp: canary.udp, f, fb, page: p.page }; return res;
    };
    let r = await runProbe('noDnr');
    ok('layer 1 alone (fault noDnr): CSP still blocks every HTTP probe', r.http.length === 0, r.http.join(' | ')); await r.fb.close();
    r = await runProbe('cspOpen');
    ok('layer 2 alone (fault cspOpen): DNR still blocks every HTTP probe', r.http.length === 0, r.http.join(' | ')); await r.fb.close();
    r = await runProbe('cspOpenNoDnr');
    ok('positive control (cspOpen+noDnr): the probes do reach the HTTP canary', r.http.length > 0, r.http.length + ': ' + r.http.slice(0, 6).join(' | ')); await r.fb.close();
    r = await runProbe('noRtc');
    ok('positive control (fault noRtc): RTCPeerConnection is available without RtcLockdown', r.probe.rtcType === 'function', JSON.stringify(r.probe.rtc));
    ok('X8 measurement: STUN packets from the sandbox without RtcLockdown', true, `udp=${r.udp} (reported, not a pass criterion: WebRTC is invisible to CSP and DNR, so layer 3 alone covers it)`); await r.fb.close();
    { const fb = await openBrowser(build({ fault: 'allowAll' }).dist, 'allowAll'); await fb.send({ type: 'profiles.add', profile: prof });
      const p = await openSite(fb, 'devnet', S_A); const f = (await siteFrame(p.page)).fr; tap.log.length = 0;
      await raw(f, msg(1, 'rpc_read', 'eth_sendRawTransaction', ['0x02']));
      ok('BR-neg allowAll: BR1 fails as required (eth_sendRawTransaction reaches RpcTap)', tapCount('eth_sendRawTransaction') === 1, tapMethods().join(',')); await fb.close(); }
  } finally {
    if (!KEEP && !REUSE) py(['stop', work]); tapServer.close(); canaryServer.close(); udp.close();
  }
}

// A stray rejection is a failed check, never a crash that skips the devnet shutdown in main's finally.
process.on('unhandledRejection', (e) => ok('no unhandled rejection', false, (e && e.stack) || String(e)));
try { await main(); } catch (e) { ok('e2e driver completed', false, e.stack || e.message); }
for (const c of contexts) await saveTrace(c, 'open-at-end');
const summary = { result: failed ? 'FAIL' : 'PASS', checks: results.length, failed, work };
if (failed) { await sleep(2000); summary.kept = keepOnFailure('e2e_browser', work, { results }); } // the nodes stop first
writeFileSync(join(EXT, 'tests', 'e2e_browser.last.json'), JSON.stringify({ ...summary, results }, null, 1) + '\n');
console.log(JSON.stringify(summary));
process.exit(failed ? 1 : 0);
