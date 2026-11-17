// Sites W11 and W8: the explorer snapshot against a live v3 devnet, served from a temporary copy (under $RUNNER_TEMP in
// CI, else the system temp directory) switched to the devnet's identity with the W9 tool (tools/site_identity.py --to:
// the anchored edits the release switch uses), the way the gateway host serves it (owner decision 9). The IDE smoke
// (its copy switched the same way, rpcUrl at a local Studio gateway, a contract compiled with its bundled compiler and
// deployed through the extension) is in prototypes/pocol-extension/tests/e2e_publisher_gateway.mjs.
//   node deploy/websites/tests/explorer_smoke.mjs     (needs python3, Chromium/Playwright, pocold binaries, and caddy)
// The switched explorer's static files (every file but SHA256SUMS.txt, checked against it) are validated and given a
// loopback copy of the gateway host's Caddy site by deploy/explorer/explorer_site.py test-config (the production block
// with http://127.0.0.1:8661, the test gateway port and the copy's root), served by Caddy in front of the explorer
// gateway (deploy/rewards/gateway.py --v3-network --site-origin) on node a. POCOL_EXPLORER_FRONT=caddy (CI) requires
// Caddy; =node serves the same block's routes, headers and hidden-path rule from a small Node front (for a machine
// without Caddy; its header checks then test the block's values, not Caddy). The bundle loads under the explorer's
// CSP, reads the chain through /api/rpc.php and shows the devnet's live head; /api/health.php passes; the relay refuses
// a foreign Origin, a GET and an oversized body as PHP did; server and hidden paths are 404; no answer reaches 4 MiB;
// no request leaves the page's origin.
// Not covered here (W7, the explorer's v3 membership semantics, has not landed: the bundle has no pocol_getMembers):
// an ACCRUING member shown as "Earning bond" and a registerReplacing transaction decoded with its victim.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, cpSync, readFileSync, writeFileSync, rmSync, existsSync, statSync, createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { createServer, request as httpRequest } from 'node:http';
import { tmpdir } from 'node:os';
import { join, dirname, extname, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { connect } from 'node:net';
import { keepOnFailure } from '../../../prototypes/pocol-extension/tests/harness.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), ROOT = join(HERE, '..', '..', '..');
const POCOLD = join(ROOT, 'prototypes', 'pocold'), BIN = process.env.POCOLD_BIN_DIR || join(POCOLD, 'target', 'release');
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || '/opt/node-tools/node_modules/playwright/index.mjs');
const EXE = process.env.POCOL_CHROMIUM || ['/usr/bin/chromium', '/usr/bin/google-chrome'].find((p) => existsSync(p));
const FRONT = 8661, EXPLORER_GW = 8662, STUDIO_GW = 8663; // STUDIO_GW: the rpcUrl the switched copy names (unused here)
const CADDY = spawnSync('caddy', ['version'], { encoding: 'utf8' }).status === 0;
const MODE = process.env.POCOL_EXPLORER_FRONT || (CADDY ? 'caddy' : 'node');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = []; let failed = 0;
const ok = (name, cond, detail = '') => { results.push({ name, pass: !!cond, detail: String(detail).slice(0, 300) }); console.log((cond ? 'PASS ' : 'FAIL ') + name + (detail !== '' ? ' — ' + String(detail).slice(0, 300) : '')); if (!cond) failed++; };
const wait = async (pred, ms, what) => { const t = Date.now(); let last; while (Date.now() - t < ms) { try { if ((last = await pred())) return last; } catch (e) { last = e; } await sleep(300); } throw new Error('timeout: ' + what + (last instanceof Error ? ' (' + last.message + ')' : '')); };
const portFree = (p) => new Promise((res) => { const s = connect(p, '127.0.0.1'); s.on('connect', () => { s.destroy(); res(false); }); s.on('error', () => res(true)); });
const rpc = async (port, method, params = [], path = '/') => { const r = await fetch(`http://127.0.0.1:${port}${path}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) }); const v = await r.json(); if (v.error) throw new Error(JSON.stringify(v.error)); return v.result; };
const devnet = (args) => { const r = spawnSync('python3', [join(POCOLD, 'scripts', 'devnet.py'), ...args], { encoding: 'utf8', env: { ...process.env, POCOLD: join(BIN, 'pocold') } }); if (r.status) throw new Error(r.stderr); return r.stdout; };
const base = process.env.RUNNER_TEMP || tmpdir();
const work = mkdtempSync(join(base, 'pocol-explorer-smoke-'));
const procs = [];
const spawnLogged = (name, cmd, args, opts = {}) => { const p = spawn(cmd, args, { stdio: ['ignore', 'pipe', 'pipe'], ...opts }); p.out = ''; p.stdout.on('data', (c) => { p.out += c; }); p.stderr.on('data', (c) => { p.out += c; }); p.on('exit', () => writeFileSync(join(work, name + '.log'), p.out)); procs.push(p); return p; };

// The Node front (POCOL_EXPLORER_FRONT=node, a machine without Caddy): the routes of the rendered block, read from its
// text, so the values it sends are the block's. The site header block (no-store, nosniff, no Server) on every answer;
// /rpc and /health proxied; /api/* with the explorer headers and the 16 KiB Content-Length guard, proxied; everything
// else with the explorer headers, the block's hidden-path rule as 404, then the copy's regular files (index.html for
// /), no directory listing. Caddy's own behaviour (handle order, defer, path cleaning) is tested only in the caddy mode.
const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.png': 'image/png', '.ico': 'image/x-icon', '.webmanifest': 'application/manifest+json', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' };
function nodeFront(block, siteDir) {
  // Header names in lower case, so a set replaces the gateway's own copy of a header as Caddy's `header` does.
  const site = Object.fromEntries([...block.split('request_body')[0].matchAll(/^ {8}([A-Za-z-]+) "([^"]*)"$/gm)].map((m) => [m[1].toLowerCase(), m[2]]));
  const explorerHeaders = Object.fromEntries([...block.split('\n    handle {')[1].matchAll(/^ {12}([A-Za-z-]+) "([^"]*)"$/gm)].map((m) => [m[1].toLowerCase(), m[2]]));
  const hiddenText = /path_regexp explorer_hidden `([^`]+)`/.exec(block)[1];
  const hidden = new RegExp(hiddenText.replace(/^\(\?i\)/, ''), hiddenText.startsWith('(?i)') ? 'i' : '');
  const limit = Number(/int\(\{http\.request\.header\.Content-Length\}\) > ([0-9]+)/.exec(block)[1]);
  const apiTooLarge = /respond @api_oversized `([^`]*)` 413/.exec(block)[1], apiTooLargeType = /header @api_oversized Content-Type "([^"]*)"/.exec(block)[1];
  const root = /^ {8}root \* (.+)$/m.exec(block)[1];
  const send = (res, status, headers, body = '') => { res.writeHead(status, { ...headers, ...site }); res.end(body); };
  const proxy = (req, res, headers) => {
    const up = httpRequest({ host: '127.0.0.1', port: EXPLORER_GW, method: req.method, path: req.url, headers: { ...req.headers, host: `127.0.0.1:${EXPLORER_GW}` } }, (r) => {
      const h = { ...r.headers, ...headers }; delete h.server; res.writeHead(r.statusCode, { ...h, ...site }); r.pipe(res); });
    up.on('error', () => send(res, 502, headers)); req.pipe(up);
  };
  const server = createServer((req, res) => {
    const path = new URL(req.url, 'http://x').pathname;
    if (path === '/rpc' || path === '/health') return proxy(req, res, {});
    const size = Number(req.headers['content-length'] || 0);
    if (path.startsWith('/api/')) return size > limit ? send(res, 413, { ...explorerHeaders, 'content-type': apiTooLargeType }, apiTooLarge) : proxy(req, res, explorerHeaders);
    if (hidden.test(path)) return send(res, 404, explorerHeaders, 'Not found');
    let file; try { file = normalize(join(root, decodeURIComponent(path))); } catch { return send(res, 400, explorerHeaders); }
    if (!file.startsWith(normalize(root) + sep) && file !== normalize(root)) return send(res, 404, explorerHeaders);
    try { if (statSync(file).isDirectory()) file = join(file, 'index.html'); if (!statSync(file).isFile()) throw new Error(); } catch { return send(res, 404, explorerHeaders); }
    res.writeHead(200, { 'content-type': TYPES[extname(file)] || 'application/octet-stream', ...explorerHeaders, ...site });
    if (req.method === 'HEAD') return res.end();
    createReadStream(file).pipe(res);
  });
  server.listen(FRONT, '127.0.0.1');
  procs.push({ kill: () => server.close() });
}

async function main() {
  for (const p of [30300, 30301, 30302, 8540, 8541, 8542, FRONT, EXPLORER_GW, STUDIO_GW]) if (!(await portFree(p))) throw new Error('port busy ' + p);
  ok(`the front is ${MODE}` + (MODE === 'caddy' ? ' (' + spawnSync('caddy', ['version'], { encoding: 'utf8' }).stdout.trim().split(' ')[0] + ')' : ' (no Caddy: the Caddy block\'s routes and headers from a Node front)'), MODE === 'node' || (MODE === 'caddy' && CADDY));
  devnet(['init', join(work, 'dn'), '--miners', 'a,b,c', '--t-blk', '2', '--tau', '20', '--c', '8', '--d-att', '41', '--fb-wait', '3', '--work-bits', '15']);
  const info = JSON.parse(readFileSync(join(work, 'dn', 'net', 'devnet.json'), 'utf8'));
  const spec = readFileSync(join(work, 'dn', 'net', 'genesis.spec.hex'), 'utf8').trim();
  devnet(['start', join(work, 'dn'), '--log']);
  try {
    await wait(async () => Number(await rpc(8540, 'eth_blockNumber')) >= 6, 180000, 'devnet height >= 6');
    ok('the devnet is v3 (specVersion 2)', info.specVersion === 2 && info.v3 === true);
    // ---- a temporary copy switched to the devnet with the W9 tool ----
    const tree = join(work, 'tree');
    for (const top of ['deploy/websites', 'prototypes/pocol-studio/dist']) cpSync(join(ROOT, top), join(tree, top), { recursive: true, filter: (s) => !/__pycache__|[\\/]config\.php$/.test(s) });
    for (const f of ['deploy/mainnet/retired-networks.json', 'prototypes/pocol-extension/package.json']) { mkdirSync(dirname(join(tree, f)), { recursive: true }); cpSync(join(ROOT, f), join(tree, f)); }
    const genesisHash = info.genesisHash.toLowerCase();
    const invitation = { schema: 'pocol-network-v2', consensusRulesVersion: 2, seeds: ['192.0.2.10:30300', '192.0.2.11:30300', '192.0.2.12:30300'],
      files: { 'genesis.spec.hex': spec.startsWith('0x') ? spec : '0x' + spec, 'devnet.json': JSON.stringify({ chainId: info.chainId, genesisHash }) } };
    writeFileSync(join(work, 'network.pocol.json'), JSON.stringify(invitation));
    const sw = spawnSync('python3', ['-B', join(ROOT, 'deploy', 'websites', 'tools', 'site_identity.py'), '--root', tree, '--to', join(work, 'network.pocol.json'), '--gateway-host', '192.0.2.10', '--rpc-url', `http://127.0.0.1:${STUDIO_GW}/studio/rpc`], { encoding: 'utf8' });
    ok('the W9 tool switches a copy of the snapshots to the devnet (explorer bundle, IDE frozen network and rpcUrl)', sw.status === 0, (sw.stderr || sw.stdout).slice(-300));
    const explorer = join(tree, 'deploy', 'websites', 'explorer.pocol.net');
    const declared = JSON.parse(readFileSync(join(ROOT, 'deploy', 'websites', 'site-identity.json'), 'utf8'));
    const grepIn = (needle) => spawnSync('grep', ['-rlF', needle, explorer], { encoding: 'utf8' }).stdout.trim();
    ok('the explorer copy pins the devnet genesis and no longer the declared one', grepIn(genesisHash.slice(2)) !== '' && grepIn(declared.genesisHash.slice(2)) === '', grepIn(genesisHash.slice(2)).split('\n').join(' '));
    // ---- the static copy, its loopback Caddy site and the explorer gateway (--v3-network --site-origin) ----
    const siteDir = join(work, 'site');
    const sums = readFileSync(join(explorer, 'SHA256SUMS.txt'), 'utf8').trim().split('\n').map((l) => l.split('  '));
    for (const [digest, name] of sums) { const raw = readFileSync(join(explorer, name)); if (createHash('sha256').update(raw).digest('hex') !== digest) throw new Error('SHA256SUMS.txt: ' + name); mkdirSync(dirname(join(siteDir, name)), { recursive: true }); writeFileSync(join(siteDir, name), raw); }
    const caddyfile = join(work, 'Caddyfile');
    const conf = spawnSync('python3', ['-B', join(ROOT, 'deploy', 'explorer', 'explorer_site.py'), 'test-config', '--site', siteDir, '--listen', `127.0.0.1:${FRONT}`, '--gateway', `127.0.0.1:${EXPLORER_GW}`, '--out', caddyfile], { encoding: 'utf8' });
    ok('explorer_site.py validates the switched static copy (its files without SHA256SUMS.txt) and writes the loopback Caddy site', conf.status === 0 && existsSync(caddyfile), (conf.stderr || conf.stdout).slice(-300));
    const block = readFileSync(caddyfile, 'utf8');
    writeFileSync(join(work, 'v3-network.json'), JSON.stringify({ chainId: info.chainId, genesisHash, specVersion: 2, v3: true }));
    const egw = spawnLogged('explorer-gateway', 'python3', ['-B', join(ROOT, 'deploy', 'rewards', 'gateway.py'), '--port', String(EXPLORER_GW), '--rpc-port', '8540', '--v3-network', join(work, 'v3-network.json'), '--site-origin', `http://127.0.0.1:${FRONT}`]);
    await wait(async () => (await rpc(EXPLORER_GW, 'eth_blockNumber', [], '/rpc')) !== undefined, 30000, 'explorer gateway').catch((e) => { throw new Error(e.message + ' ' + egw.out.slice(-300)); });
    if (MODE === 'caddy') {
      const home = join(work, 'caddy-home'); mkdirSync(home);
      spawnLogged('caddy', 'caddy', ['run', '--config', caddyfile, '--adapter', 'caddyfile'], { env: { ...process.env, HOME: home, XDG_DATA_HOME: join(home, 'data'), XDG_CONFIG_HOME: join(home, 'config') } });
    } else nodeFront(block, siteDir);
    await wait(async () => (await fetch(`http://127.0.0.1:${FRONT}/index.html`)).ok, 30000, `the ${MODE} front`);
    const base = `http://127.0.0.1:${FRONT}`;
    const relayed = await rpc(FRONT, 'eth_blockNumber', [], '/api/rpc.php');
    ok('/api/rpc.php relays a read to the explorer gateway on the devnet (no Origin: a server-side caller such as pocol.net)', /^0x[0-9a-f]+$/.test(relayed), relayed);
    // health.php compares the gateway's pocol_getParams with its configured network: HTTP 200 with networkMatch and PASS
    // only when both match (503 NOT_CONNECTED otherwise).
    const hres = await fetch(`${base}/api/health.php`); const health = await hres.text();
    let hv = null; try { hv = JSON.parse(health); } catch {}
    ok('/api/health.php reports the devnet identity matched (the gateway\'s --v3-network against the node)', hres.status === 200 && hv && hv.gatewayReachable === true && hv.networkMatch === true && hv.result === 'PASS', `HTTP ${hres.status} ${health.slice(0, 200)}`);
    // The explorer's .htaccess headers, now the Caddy site's: on the page and on the /api reads.
    const expected = Object.fromEntries([...block.split('\n    handle {')[1].matchAll(/^ {12}([A-Za-z-]+) "([^"]*)"$/gm)].map((m) => [m[1].toLowerCase(), m[2]]));
    const home = await fetch(`${base}/`); const homeText = await home.text();
    const headersOf = (r) => Object.fromEntries([...r.headers.entries()]);
    const missing = (r, extra = {}) => Object.entries({ ...expected, 'x-content-type-options': 'nosniff', 'cache-control': 'no-store', ...extra }).filter(([k, v]) => r.headers.get(k) !== v).map(([k]) => k);
    ok('the page carries the explorer\'s CSP, X-Frame-Options, Referrer-Policy, Permissions-Policy, nosniff and no-store', home.status === 200 && homeText.includes('<div id="root">') && Object.keys(expected).length === 4 && missing(home).length === 0 && !home.headers.get('server'), `missing ${missing(home)} ${JSON.stringify(headersOf(home)).slice(0, 300)}`);
    ok('/api/health.php carries the same headers and a JSON charset', missing(hres, { 'content-type': 'application/json; charset=utf-8' }).length === 0, missing(hres).join(' '));
    const post = (body, headers = {}) => fetch(`${base}/api/rpc.php`, { method: 'POST', headers: { 'content-type': 'application/json', ...headers }, body });
    const one = JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_blockNumber', params: [] });
    const foreign = await post(one, { origin: 'https://evil.example' });
    ok('/api/rpc.php refuses a foreign Origin (403) and sends no CORS header', foreign.status === 403 && (await foreign.json()).error === 'Cross-origin requests are not enabled.' && !foreign.headers.get('access-control-allow-origin'));
    const own = await post(one, { origin: base });
    ok('/api/rpc.php accepts the site Origin', own.status === 200 && /^0x/.test((await own.json()).result));
    const getRpc = await fetch(`${base}/api/rpc.php`);
    ok('GET /api/rpc.php is 405 with Allow: POST', getRpc.status === 405 && getRpc.headers.get('allow') === 'POST');
    const big = await post(one + ' '.repeat(16385 - one.length)); const bigText = await big.text();
    ok('a 16385-byte body is 413 (the 16 KiB guard) with rpc.php\'s JSON error', big.status === 413 && big.headers.get('content-type') === 'application/json; charset=utf-8' && bigText === '{"error":"Request size limit exceeded."}' && missing(big).length === 0, `HTTP ${big.status} ${big.headers.get('content-type')} ${bigText.slice(0, 100)} missing ${missing(big)}`);
    const batch21 = await post(JSON.stringify(Array.from({ length: 21 }, (_, i) => ({ jsonrpc: '2.0', id: i + 1, method: 'eth_blockNumber', params: [] }))));
    ok('a batch of 21 is 400 (lib.php\'s bound)', batch21.status === 400);
    const reward = await post(JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'pocol_getBlockReward', params: [] }));
    ok('/api/rpc.php refuses a method outside the explorer allow-list that /rpc accepts (400)', reward.status === 400);
    const geo = await fetch(`${base}/api/miner-locations.php`), txs = await fetch(`${base}/api/transactions.php?page=1&size=25&order=newest&q=`), badTx = await fetch(`${base}/api/transactions.php?page=0`);
    ok('without the optional GeoIP and transaction index modules, miner-locations and transactions answer 503 with their messages; a bad query is 400', geo.status === 503 && /GeoIP/.test((await geo.json()).error) && txs.status === 503 && /Transaction history/.test((await txs.json()).error) && badTx.status === 400, `${geo.status} ${txs.status} ${badTx.status}`);
    const hidden = {};
    for (const path of ['/api/config.php', '/api/lib.php', '/api/', '/.htaccess', '/SHA256SUMS.txt', '/site-identity.json', '/index.php', '/assets/index.js.map', '/assets/', '/missing.html']) hidden[path] = (await fetch(base + path)).status;
    ok('server, hidden and missing paths are 404 (no directory listing, no PHP)', Object.values(hidden).every((s) => s === 404), JSON.stringify(hidden));

    const browser = await chromium.launch({ executablePath: EXE, headless: true });
    try {
      // Every request outside the page's origin is aborted and counted. The bundle asks Google Fonts for its type faces
      // (index.html); that is the only outside request allowed (aborted all the same), anything else fails the check.
      const ctx = await browser.newContext(); const outside = []; const answers = [];
      await ctx.route('**/*', (route) => { const u = new URL(route.request().url()); if (u.origin === `http://127.0.0.1:${FRONT}`) return route.fallback(); outside.push(u.href); return route.abort(); });
      const page = await ctx.newPage(); const errors = []; page.on('pageerror', (e) => errors.push(e.message));
      const asked = []; page.on('request', (r) => { if (r.url().includes('/api/')) asked.push(r.method() + ' ' + r.url().replace(/^.*\/api\//, '')); });
      page.on('response', async (r) => { if (r.url().includes('/api/')) { try { answers.push({ url: r.url(), status: r.status(), bytes: (await r.body()).length }); } catch (e) { answers.push({ url: r.url(), status: r.status(), bytes: 0, error: e.message }); } } });
      const csp = []; page.on('console', (m) => { if (/Content Security Policy/i.test(m.text())) csp.push(m.text()); });
      await page.goto(`http://127.0.0.1:${FRONT}/`);
      await wait(() => answers.some((a) => a.url.includes('/api/rpc.php') && a.status === 200), 60000, 'the bundle reads through api/rpc.php').catch(() => {});
      const headNow = Number(await rpc(8540, 'eth_blockNumber'));
      // The overview shows "Connected", the chain ID and "Latest block <n>" for the head it read through the relay.
      const shown = await wait(async () => { const t = await page.evaluate(() => document.body.innerText); const m = /Latest block\s+([\d,]+)/.exec(t); const n = m ? Number(m[1].replace(/,/g, '')) : -1;
        return /\bConnected\b/.test(t) && new RegExp(`Chain ID\\s+${info.chainId}\\b`).test(t) && n >= headNow - 5 && n <= Number(await rpc(8540, 'eth_blockNumber')) && t; }, 60000, 'the explorer shows Connected, the devnet chain ID and its live head');
      const rpcAnswers = answers.filter((a) => a.url.includes('/api/rpc.php'));
      ok('the explorer bundle loads from the copy and shows "Connected", the devnet\'s chain ID and its live head ("Latest block") read through api/rpc.php', rpcAnswers.some((a) => a.status === 200), `${rpcAnswers.length} relay answers; head ${headNow}; api: ${answers.map((a) => a.url.replace(/^.*\/api\//, '') + ':' + a.status).join(' ')}; asked: ${asked.slice(0, 6).join(', ')}`);
      await sleep(16000); // past one 15 s refresh of the overview and the transaction history
      const biggest = Math.max(0, ...answers.map((a) => a.bytes));
      ok(`every relay answer stays under 4 MiB (largest ${biggest} B over ${answers.length} answers)`, answers.length > 0 && biggest < 4194304);
      const unexpected = outside.filter((u) => !/^https:\/\/fonts\.(googleapis|gstatic)\.com\//.test(u));
      ok(`no request left the page's origin except the font stylesheet (${outside.length} aborted)`, unexpected.length === 0, unexpected.slice(0, 5).join(' '));
      ok('the explorer raised no uncaught error', errors.length === 0, errors.join(' | '));
      ok('the explorer ran under its CSP without a violation', csp.length === 0, csp.join(' | '));
      writeFileSync(join(work, 'explorer-text.txt'), String(shown).slice(0, 20000));
      await page.close();

      await ctx.close();
    } finally { await browser.close(); }
  } finally {
    for (const p of procs) { try { p.kill('SIGTERM'); } catch {} }
    try { devnet(['stop', join(work, 'dn')]); } catch {}
  }
}
try { await main(); } catch (e) { ok('explorer/IDE smoke completed', false, e.stack || e.message); }
await sleep(1500);
const summary = { result: failed ? 'FAIL' : 'PASS', checks: results.length, failed, work };
// A failed run keeps its evidence (POCOL_E2E_KEEP in CI, uploaded with evidence/): explorer-gateway.log, caddy.log,
// Caddyfile, explorer-text.txt and the devnet's node logs and inputs (dn/). The switched copy of the sites and the
// static copy are left out (the repository's files with the W9 tool's edits).
if (failed) {
  try { rmSync(join(work, 'tree'), { recursive: true, force: true }); rmSync(join(work, 'site'), { recursive: true, force: true }); } catch {}
  summary.kept = keepOnFailure('explorer_smoke', work, { results });
}
writeFileSync(join(HERE, 'explorer_smoke.last.json'), JSON.stringify({ ...summary, results }, null, 1) + '\n');
if (!failed && process.env.E2E_KEEP !== '1') rmSync(work, { recursive: true, force: true });
console.log(JSON.stringify(summary));
process.exit(failed ? 1 : 0);
