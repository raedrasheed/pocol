// Shared harness for the browser suites: devnet control, RpcTap, canaries, site
// publishing, and Chromium with the extension loaded.
import { spawnSync, execFileSync } from 'node:child_process';
import { createServer, request as httpRequest } from 'node:http';
import { createSocket } from 'node:dgram';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync, readdirSync, readFileSync, copyFileSync, statSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';
import { connect } from 'node:net';

const HERE = dirname(fileURLToPath(import.meta.url));
export const EXT = dirname(HERE), POCOLD = join(EXT, '..', 'pocold');
// POCOLD_BIN_DIR: another build's binaries (seeded-fault controls); devnet.py is given the same pocold.
export const BIN = (n) => join(process.env.POCOLD_BIN_DIR || join(POCOLD, 'target', 'release'), n);
const PW = process.env.PLAYWRIGHT_MODULE || '/opt/node-tools/node_modules/playwright/index.mjs';
export const { chromium } = await import(PW);
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export async function wait(pred, ms, what) {
  const t = Date.now(); let last;
  while (Date.now() - t < ms) { try { if ((last = await pred())) return last; } catch (e) { last = e; } await sleep(300); }
  throw new Error('timeout: ' + what + (last instanceof Error ? ' (' + last.message + ')' : ''));
}
export function checker() {
  const results = []; let failed = 0;
  const ok = (name, cond, detail = '') => { results.push({ name, pass: !!cond, detail: String(detail).slice(0, 300) }); console.log((cond ? 'PASS ' : 'FAIL ') + name + (detail !== '' ? ' — ' + String(detail).slice(0, 300) : '')); if (!cond) failed++; };
  return { ok, results, failed: () => failed };
}
export const devnet = (args) => { const r = spawnSync('python3', [join(POCOLD, 'scripts', 'devnet.py'), ...args], { encoding: 'utf8', env: { ...process.env, POCOLD: BIN('pocold') } }); if (r.status) throw new Error(r.stderr); return r.stdout; };
const sha256 = (f) => createHash('sha256').update(readFileSync(f)).digest('hex');
/** A failed run's evidence (2026-10-28), kept before anything cleans the work directory: every node log and
 * every input under `work` (the devnet's net/ directory with its genesis, profile and keys, site files, Playwright
 * traces), each node's blocks.journal (its blocks, from which replay rebuilds the derived stores, I1), and
 * `extra`, the run's own record (results, phases, and the values it drew: devnet.json holds the genesis time and
 * chain id). The derived stores (*.redb) are listed with their sizes, not copied. Also the commit, the pocold
 * binary's SHA-256 and the environment that changes a run. Into POCOL_E2E_KEEP or tests/failures/ (ignored by
 * git), with manifest.json (every file's size and SHA-256). Returns the directory. */
// The variables the e2e drivers read (review F9, 2026-10-28): named one by one, so nothing else in the environment is kept.
const KEPT_ENV = ['POCOLD_BIN_DIR', 'POCOLD_RUN_ARGS', 'POCOL_E2E_KEEP', 'PLAYWRIGHT_MODULE', 'E2E_KEEP', 'E2E_REUSE'];
export function keepOnFailure(suite, work, extra = {}) {
  const dir = join(process.env.POCOL_E2E_KEEP || join(HERE, 'failures'), `${suite}-${new Date().toISOString().replace(/[:.]/g, '')}-${process.pid}`);
  const files = [], skipped = [];
  const walk = (src, rel) => { for (const e of readdirSync(src, { withFileTypes: true })) { const s = join(src, e.name), r = rel ? join(rel, e.name) : e.name;
    if (e.isDirectory()) walk(s, r); else if (!e.isFile()) continue; else if (/\.redb(\.aside|\.damaged)?$/.test(e.name)) skipped.push({ path: r, bytes: statSync(s).size });
    else { const d = join(dir, 'work', r); mkdirSync(dirname(d), { recursive: true }); copyFileSync(s, d); files.push({ path: r, bytes: statSync(s).size, sha256: sha256(d) }); } } };
  mkdirSync(dir, { recursive: true }); if (existsSync(work)) walk(work, '');
  let commit = null; try { commit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: HERE, encoding: 'utf8' }).trim(); } catch {}
  const bin = BIN('pocold'); const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => KEPT_ENV.includes(k)));
  writeFileSync(join(dir, 'manifest.json'), JSON.stringify({ suite, work, keptAt: new Date().toISOString(), commit, pocold: { path: bin, sha256: existsSync(bin) ? sha256(bin) : null }, env, files, notCopied: skipped, extra },
    (k, v) => (typeof v === 'bigint' ? v.toString() : v), 1) + '\n');
  console.log(`kept the failed run's logs and inputs in ${dir}`); return dir;
}
export async function rpc(port, method, params = []) {
  const r = await fetch(`http://127.0.0.1:${port}/`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) });
  const v = await r.json(); if (v.error) throw Object.assign(new Error(v.error.message), { rpc: v.error }); return v.result;
}
export const portFree = (p) => new Promise((res) => { const s = connect(p, '127.0.0.1'); s.on('connect', () => { s.destroy(); res(false); }); s.on('error', () => res(true)); });

/** RpcTap: HTTP proxy in front of a node's RPC that records every method it forwards. */
export function startTap(port, upstream = 8540) {
  const tap = { log: [], holdMs: {}, upstream };
  const server = createServer((req, res) => {
    let body = ''; req.on('data', (c) => { body += c; }); req.on('end', async () => {
      let m = null; try { m = JSON.parse(body); } catch {}
      const methods = Array.isArray(m) ? m.map((x) => x.method) : [m && m.method];
      for (const x of methods) tap.log.push({ method: x, params: m && !Array.isArray(m) ? m.params : null, origin: req.headers.origin || null, t: Date.now() });
      const hold = tap.holdMs[methods[0]]; if (hold) await sleep(hold);
      const up = httpRequest({ host: '127.0.0.1', port: tap.upstream, method: 'POST', path: '/', headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(body) } }, (r) => {
        res.writeHead(r.statusCode, { 'content-type': 'application/json', 'access-control-allow-origin': '*' }); r.pipe(res);
      });
      up.on('error', () => { res.writeHead(502); res.end(); }); up.end(body);
    });
  });
  tap.methods = () => tap.log.map((x) => x.method); tap.count = (mm) => tap.log.filter((x) => x.method === mm).length;
  tap.close = () => server.close();
  return new Promise((r) => server.listen(port, '127.0.0.1', () => r(tap)));
}
/** HTTP canary (all interfaces) and UDP canary for STUN. */
export async function startCanary(httpPort, udpPort) {
  const canary = { http: [], udp: 0 };
  const server = createServer((req, res) => { canary.http.push(req.method + ' ' + req.url); res.writeHead(200, { 'access-control-allow-origin': '*', 'content-type': 'text/plain' }); res.end('canary'); });
  server.on('upgrade', (req, sock) => { canary.http.push('UPGRADE ' + req.url); sock.destroy(); });
  const udp = createSocket('udp4'); udp.on('message', () => { canary.udp++; });
  await new Promise((r) => server.listen(httpPort, '0.0.0.0', r)); await new Promise((r) => udp.bind(udpPort, '0.0.0.0', r));
  canary.reset = () => { canary.http.length = 0; canary.udp = 0; };
  canary.close = () => { server.close(); udp.close(); };
  return canary;
}

export function png2x2() {
  const t = []; for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; }
  const crc = (b) => { let x = 0xffffffff; for (const v of b) x = t[(x ^ v) & 255] ^ (x >>> 8); return (x ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => { const l = Buffer.alloc(4); l.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
  const ihdr = Buffer.from([0, 0, 0, 2, 0, 0, 0, 2, 8, 2, 0, 0, 0]);
  const raw = Buffer.from([0, 255, 0, 0, 0, 255, 0, 0, 0, 0, 255, 255, 255, 0]);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
export function writeSite(dir, files) { for (const [p, c] of Object.entries(files)) { const f = join(dir, p); mkdirSync(dirname(f), { recursive: true }); writeFileSync(f, c); } }
export function publish(dir, port, cid, site) {
  const args = ['publish', '--rpc', `127.0.0.1:${port}`, '--chain-id', cid, '--account', 'alice', '--dir', dir]; if (site) args.push('--site', site);
  const r = spawnSync(BIN('pocol-site'), args, { encoding: 'utf8', timeout: 900000 }); if (r.status) throw new Error('publish: ' + (r.stderr || '').slice(-400));
  return JSON.parse(r.stdout);
}

export async function openBrowser(dist, tag) {
  const userDir = mkdtempSync(join(tmpdir(), 'pocol-chrome-' + tag + '-'));
  const ctx = await chromium.launchPersistentContext(userDir, { channel: 'chromium', headless: true, args: [`--disable-extensions-except=${dist}`, `--load-extension=${dist}`] });
  let [sw] = ctx.serviceWorkers(); if (!sw) sw = await ctx.waitForEvent('serviceworker', { timeout: 20000 });
  const id = new URL(sw.url()).host; const opt = await ctx.newPage(); await opt.goto(`chrome-extension://${id}/options.html`);
  const send = (m) => opt.evaluate((x) => chrome.runtime.sendMessage(x), m);
  return { ctx, id, opt, sw, send, close: async () => { await ctx.close(); rmSync(userDir, { recursive: true, force: true }); } };
}
export async function siteFrame(page, minFrame = 1) {
  await page.waitForFunction((n) => { const f = document.querySelector('#stage iframe#site'); return (f && Number(f.dataset.frame) >= n) || document.querySelector('#stage .err'); }, minFrame, { timeout: 90000 });
  const err = await page.$('#stage .err'); if (err) throw new Error('viewer: ' + (await err.textContent()));
  const el = await page.$('#stage iframe#site'); const n = Number(await el.getAttribute('data-frame')); const fr = await el.contentFrame();
  await fr.waitForFunction(() => document.documentElement.dataset.pocolReady === '1', null, { timeout: 30000 });
  return { fr, n };
}
export async function openSite(b, name, site, path = '/') {
  const page = await b.ctx.newPage(); const errors = []; page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`chrome-extension://${b.id}/viewer.html#${name}/${site}${path}`);
  return { page, errors };
}
export const raw = (fr, text) => fr.evaluate((t) => pocol.raw(t), text);
export const req = (fr, kind, method, params) => fr.evaluate(async ([k, m, p]) => { try { return { result: await pocol.request(k, m, p) }; } catch (e) { return { error: { code: e.code, message: e.message, data: e.data } }; } }, [kind, method, params]);
export const msg = (id, kind, method, params) => JSON.stringify({ id, kind, payload: { method, params } });
