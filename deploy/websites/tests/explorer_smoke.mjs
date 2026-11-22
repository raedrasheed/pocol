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
// Sites W7 (the bundle's v3 membership, light refresh and Registry decoding): a throwaway devnet account registers with
// less than B_reg and another sends registerReplacing naming a miner (pocol-sign with devnet-only --account keys).
// The refresh asks for one pocol_getMembers page and only full=false block lists, never pocol_getMiners; the account
// view shows the newcomer as "Earning bond" with its bond as x/B PCOL (both from its pocol_getMember record); the
// transaction view decodes the registerReplacing call and shows the miner's record read at the head; the Miners tab and the
// Overview count the page's Held and Earning bond members, and the Overview lists the latest transactions by hash. A devnet of three miners cannot fill the member set, so the
// replacement itself reverts (the set is not full): what is checked is the decoding of the included call and the
// victim read, the same for a successful one (the vm test covers an exited victim's label).
// Production readiness (2026-11-22): the gateway runs with the transaction index (POCOL_TX_DB, pocol_transactions.py):
// /api/transactions.php lists the devnet's transactions newest first and the Transactions tab shows them in that order
// with the index's status; the Miner map says miner locations are not available (no GeoIP module: the network records
// none, and peers are not placed by IP address). At 390 and 1366 px the Overview, Blocks, Transactions, Miners and
// Miner map tabs and the block, transaction and account views have no horizontal scroll (screenshots in the work
// directory); the Overview's head moves without a reload (automatic refresh); with the gateway stopped the page says
// "Data delayed" and returns to "Connected" when it is back; no page shows the test-fixture banner.
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
const sign = (args) => { const r = spawnSync(join(BIN, 'pocol-sign'), args, { encoding: 'utf8' }); if (r.status) throw new Error(r.stderr); return r.stdout.trim(); };
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
  devnet(['init', join(work, 'dn'), '--miners', 'a,b,c', '--accounts', 'e,f', '--t-blk', '2', '--tau', '20', '--c', '8', '--d-att', '41', '--fb-wait', '3', '--work-bits', '15']);
  const info = JSON.parse(readFileSync(join(work, 'dn', 'net', 'devnet.json'), 'utf8'));
  const spec = readFileSync(join(work, 'dn', 'net', 'genesis.spec.hex'), 'utf8').trim();
  devnet(['start', join(work, 'dn'), '--log']);
  try {
    await wait(async () => Number(await rpc(8540, 'eth_blockNumber')) >= 6, 180000, 'devnet height >= 6');
    ok('the devnet is v3 (specVersion 2)', info.specVersion === 2 && info.v3 === true);
    // ---- a temporary copy switched to the devnet with the W9 tool ----
    const tree = join(work, 'tree');
    for (const top of ['deploy/websites', 'prototypes/pocol-studio/dist']) cpSync(join(ROOT, top), join(tree, top), { recursive: true, filter: (s) => !/__pycache__|[\\/]config\.php$/.test(s) });
    for (const f of ['deploy/mainnet/retired-networks.json', 'prototypes/pocol-extension/package.json', 'deploy/standalone/package.py']) { mkdirSync(dirname(join(tree, f)), { recursive: true }); cpSync(join(ROOT, f), join(tree, f)); }
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
    const gatewayArgs = ['-B', join(ROOT, 'deploy', 'rewards', 'gateway.py'), '--port', String(EXPLORER_GW), '--rpc-port', '8540', '--v3-network', join(work, 'v3-network.json'), '--site-origin', `http://127.0.0.1:${FRONT}`];
    const gatewayEnv = { env: { ...process.env, POCOL_TX_DB: join(work, 'transactions.sqlite3') } };
    let egw = spawnLogged('explorer-gateway', 'python3', gatewayArgs, gatewayEnv);
    await wait(async () => (await rpc(EXPLORER_GW, 'eth_blockNumber', [], '/rpc')) !== undefined, 30000, 'explorer gateway').catch((e) => { throw new Error(e.message + ' ' + egw.out.slice(-300)); });
    // ---- W7: an ACCRUING newcomer (e, with a quarter of B_reg) and a registerReplacing call naming a miner (f) ----
    // B_reg is the Registry's (the gateway's pocol_getMember reads it from the CAPS contract), never assumed here; the
    // named miner is the first record of the active list (a miner's id is its miner key, not a devnet account).
    const REGISTRY = '0x0000000000000000000000000000000000c0c001';
    const addressOf = (label) => sign(['--chain-id', String(info.chainId), '--account', label, '--address']).toLowerCase();
    const [newcomer, replacer] = ['e', 'f'].map(addressOf);
    const victim = (await rpc(EXPLORER_GW, 'pocol_getMembers', [0, 256], '/rpc')).members[0].id.toLowerCase();
    const word = (a) => '0'.repeat(24) + a.slice(2);
    const before = await rpc(EXPLORER_GW, 'pocol_getMember', [newcomer], '/rpc');
    const bReg = BigInt(before.bondTargetWei), bond = bReg / 4n;
    const send = (label, data, value, gas) => rpc(8540, 'eth_sendRawTransaction', [sign(['--chain-id', String(info.chainId), '--account', label, '--to', REGISTRY, '--value', String(value), '--gas', String(gas), '--data', data])]);
    const registered = await send('e', '0x4420e486' + word(newcomer), bond, 400000);
    const replacing = await send('f', '0x5326ca0b' + word(replacer) + word(victim), 0, 500000);
    const receipts = await wait(async () => { const a = await rpc(8540, 'eth_getTransactionReceipt', [registered]), b = await rpc(8540, 'eth_getTransactionReceipt', [replacing]); return a && b && [a, b]; }, 120000, 'the two Registry transactions are included');
    const record = await wait(async () => { const r = await rpc(EXPLORER_GW, 'pocol_getMember', [newcomer], '/rpc'); return r.exists && r; }, 30000, 'the newcomer\'s record');
    ok('a throwaway account registers with a quarter of B_reg (ACCRUING) and a registerReplacing call naming a miner is included (it reverts: the set is not full)',
      before.exists === false && bReg > 0n && receipts[0].status === '0x1' && receipts[1].status === '0x0' && record.status === 1 && record.bondState === 4 && BigInt(record.bondWei) === bond,
      JSON.stringify({ receipts: receipts.map((r) => r.status), status: record.status, bondState: record.bondState, bondWei: record.bondWei, bondTargetWei: record.bondTargetWei }));
    const pcol = (wei) => { const n = 10n ** 18n, f = (wei % n).toString().padStart(18, '0').replace(/0+$/, ''); return (wei / n).toString() + (f ? '.' + f : ''); };
    const bondText = `${pcol(bond)}/${pcol(bReg)} PCOL`;
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
    const geo = await fetch(`${base}/api/miner-locations.php`), badTx = await fetch(`${base}/api/transactions.php?page=0`);
    ok('without a GeoIP module miner-locations answers 503 with its message; a bad transaction query is 400', geo.status === 503 && /GeoIP/.test((await geo.json()).error) && badTx.status === 400, `${geo.status} ${badTx.status}`);
    // The transaction index: the devnet's only transactions are the two Registry calls, newest (block, position) first.
    const newestFirst = [[registered, receipts[0]], [replacing, receipts[1]]].sort(([, a], [, b]) => Number(b.blockNumber) - Number(a.blockNumber) || Number(b.transactionIndex) - Number(a.transactionIndex)).map(([h]) => h);
    const txPage = await wait(async () => { const r = await fetch(`${base}/api/transactions.php?page=1&size=25&order=newest&q=`); const v = r.status === 200 && (await r.json()).data;
      return v && v.index.height >= Math.max(...receipts.map((x) => Number(x.blockNumber))) && v; }, 60000, 'the transaction index').catch((e) => ({ error: e.message }));
    ok('/api/transactions.php lists the two Registry calls newest first from the transaction index (schema pocol-transactions-v1, a snapshot, the index state)',
      txPage.schema === 'pocol-transactions-v1' && JSON.stringify(txPage.rows.map((r) => r.hash)) === JSON.stringify(newestFirst) && txPage.rows.every((r) => r.kind === 'Contract call') && txPage.snapshot && ['ready', 'syncing'].includes(txPage.index.state),
      JSON.stringify(txPage).slice(0, 300));
    const hidden = {}, bare = {};
    for (const path of ['/api/config.php', '/api/lib.php', '/api/', '/.htaccess', '/SHA256SUMS.txt', '/site-identity.json', '/index.php', '/assets/index.js.map', '/assets/', '/missing.html']) {
      const r = await fetch(base + path); hidden[path] = r.status; await r.arrayBuffer();
      // The 404s Caddy writes itself (a missing file, a directory without index.html) come from handle_errors, the
      // others from the routes: every one with the explorer's headers, no-store, nosniff and no Server header.
      const lost = missing(r); if (lost.length || r.headers.get('server')) bare[path] = lost.concat(r.headers.get('server') ? ['server: ' + r.headers.get('server')] : []);
    }
    ok('server, hidden and missing paths are 404 (no directory listing, no PHP)', Object.values(hidden).every((s) => s === 404), JSON.stringify(hidden));
    ok('every 404, those of a missing file and of a directory included, carries the explorer\'s headers, no-store and nosniff and no Server header', Object.keys(bare).length === 0, JSON.stringify(bare));

    const browser = await chromium.launch({ executablePath: EXE, headless: true });
    try {
      // Every request outside the page's origin is aborted and counted. The bundle asks Google Fonts for its type faces
      // (index.html); that is the only outside request allowed (aborted all the same), anything else fails the check.
      const ctx = await browser.newContext(); const outside = []; const answers = [];
      await ctx.route('**/*', (route) => { const u = new URL(route.request().url()); if (u.origin === `http://127.0.0.1:${FRONT}`) return route.fallback(); outside.push(u.href); return route.abort(); });
      const page = await ctx.newPage(); const errors = []; page.on('pageerror', (e) => errors.push(e.message));
      const asked = []; page.on('request', (r) => { if (r.url().includes('/api/')) asked.push(r.method() + ' ' + r.url().replace(/^.*\/api\//, '')); });
      const bodies = []; page.on('request', (r) => { if (r.url().includes('/api/rpc.php')) { try { bodies.push(JSON.parse(r.postData() || 'null')); } catch { bodies.push(null); } } });
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
      // ---- W7 in the page: the Overview counts, the account view, the transaction view, the Miners tab ----
      const text = () => page.evaluate(() => document.body.innerText);
      const dialog = () => page.evaluate(() => document.querySelector('dialog.detail-dialog')?.innerText || '');
      const short = (h) => h.slice(0, 9) + '…' + h.slice(-7);
      const overview = await wait(async () => { const t = await text(); return /Active members\s+4\s+3 held · 1 earning bond · 0 tombed/.test(t) && t; }, 60000, 'the Overview counts the members page').catch((e) => e.message);
      ok('the Overview shows "Active members 4" with "3 held · 1 earning bond · 0 tombed" for the loaded page', /Active members/.test(overview), String(overview).slice(0, 300));
      // The Overview's latest transactions: eth_getTransactionByHash for the newest hashes of the light blocks. The
      // devnet's only transactions are the two Registry calls, each listed exactly when its block is in the window.
      const short6 = (h) => h.slice(0, 8) + '…' + h.slice(-6);
      const latest = await wait(async () => { const t = await text(); const head = Number((/Latest block\s+([\d,]+)/.exec(t) || [0, '0'])[1].replace(/,/g, ''));
        const due = [[registered, receipts[0]], [replacing, receipts[1]]].filter(([, r]) => Number(r.blockNumber) > head - 20).map(([h]) => h);
        return head > 0 && (due.length ? due.every((h) => t.includes(short6(h))) : t.includes('No transactions in this window')) && { head, due }; }, 40000, 'the latest transactions').catch((e) => e.message);
      ok('the Overview lists the latest transactions read by hash from the light blocks (the two Registry calls while their blocks are loaded)', typeof latest === 'object', JSON.stringify(latest));
      const search = async (q) => { await page.fill('input[aria-label="Search block, transaction or address"]', q); await page.press('input[aria-label="Search block, transaction or address"]', 'Enter'); };
      const close = async () => { await page.click('button[aria-label="Close details"]'); await wait(async () => !(await page.evaluate(() => document.querySelector('dialog.detail-dialog')?.open)), 10000, 'the dialog closes'); };
      const loadedHead = Number((/Latest block\s+([\d,]+)/.exec(await text()) || [0, '0'])[1].replace(/,/g, ''));
      await search(newcomer);
      // The activity is final once it lists a transaction of the account or says the loaded window has none: the
      // section heading can show before the activity read starts, and an empty section is not yet an answer.
      const account = await wait(async () => { const t = await dialog(); return /Recent account activity/.test(t) && !/Reading the latest blocks with transactions/.test(t)
        && (t.includes(short(registered)) || t.includes('No matching transactions in the loaded window.')) && t; }, 60000, 'the account view and its activity').catch((e) => e.message);
      ok(`the account view shows the newcomer as "Earning bond" with its bond ${bondText} and the proposer flag, from its pocol_getMember record`,
        /Mining membership\s+Earning bond/.test(account) && new RegExp(`Bond\\s+${bondText.replace(/[.]/g, '\\.')}`).test(account) && /Proposer eligible \(next block\)\s+(Yes|No|Undetermined)/.test(account) && /Reward address\s+0x/i.test(account), String(account).slice(String(account).indexOf('Mining membership')).replace(/\s+/g, ' '));
      // The activity reads the loaded window's blocks that have transactions, one full block per request: the
      // registration is listed exactly when its block is in that window.
      const inWindow = Number(receipts[0].blockNumber) > loadedHead - 20;
      ok(`the account activity ${inWindow ? 'lists the registration (its block is in the loaded window)' : 'reports no match (the registration\'s block left the loaded window)'}`,
        inWindow ? account.includes(short(registered)) : account.includes('No matching transactions in the loaded window.'), `block ${Number(receipts[0].blockNumber)} head ${loadedHead}`);
      await close();
      await search(replacing);
      // On a timeout the detail is what the dialog showed (an error answer, a loading state), not only the timeout.
      const decoded = await wait(async () => { const t = await dialog(); return /Registry call\s+registerReplacing/.test(t) && /Named member now/.test(t) && t; }, 60000, 'the decoded registerReplacing').catch(async (e) => e.message + ' | dialog: ' + (await dialog()));
      ok('the transaction view decodes registerReplacing (reward address, the replaced miner) and shows that miner\'s record ("Held") read at the head, not at the transaction\'s height',
        /Reward address\s+0x/i.test(decoded) && new RegExp(`replaced member\\s+${victim}`).test(decoded.toLowerCase()) && /Named member now\s+Held · read at the head \(block \d+\), not at this transaction's height/.test(decoded) && /Reverted/.test(decoded), String(decoded).slice(Math.max(0, String(decoded).indexOf('Registry call'))).replace(/\s+/g, ' '));
      await close();
      await page.click('nav button:has-text("Miners")');
      const rowsOf = () => page.evaluate(() => [...document.querySelectorAll('table tbody tr')].map((r) => r.innerText.replace(/\s+/g, ' ')));
      const miners = await wait(async () => { const rows = await rowsOf(); return rows.length === 4 && rows; }, 30000, 'the Miners table').catch(() => []);
      const mine = miners.find((r) => r.includes(short(newcomer))) || '';
      ok('the Miners tab lists the page with State, Last share and Registered at: the newcomer "Earning bond", the three miners "Held", "Members 1–4 of 4"',
        mine.includes('Earning bond') && miners.filter((r) => r.includes('Held')).length === 3 && /Members 1–4 of 4 · 256 per page/.test(await text()) && /Last share/.test(await text()), JSON.stringify(miners).slice(0, 600));
      await page.selectOption('select[aria-label="Member state"]', 'Earning bond');
      const filtered = await wait(async () => { const rows = await rowsOf(); return rows.length === 1 && rows; }, 10000, 'the filter').catch(() => []);
      ok('the Miners filter "Earning bond" keeps only the newcomer', filtered.length === 1 && filtered[0].includes(short(newcomer)), JSON.stringify(filtered));
      // ---- the Transactions tab (the transaction index) and the Miner map (no locations) ----
      await page.click('nav button:has-text("Transactions")');
      const history = await wait(async () => { const t = await text(); const rows = await rowsOf(); return /History indexed through block #|Indexing chain history/.test(t) && rows.length >= 2 && { t, rows }; }, 60000, 'the transaction history').catch((e) => ({ t: e.message, rows: [] }));
      ok('the Transactions tab lists the Registry calls newest first, with the index status and "Contract call"',
        history.rows.length === 2 && history.rows[0].includes(newestFirst[0].slice(0, 8)) && history.rows[1].includes(newestFirst[1].slice(0, 8)) && history.rows.every((r) => r.includes('Contract call')),
        JSON.stringify(history.rows).slice(0, 400) + ' ' + String(history.t).slice(String(history.t).indexOf('History'), String(history.t).indexOf('History') + 120));
      await page.click('nav button:has-text("Miner map")');
      const map = await wait(async () => { const t = await text(); return /Miner locations unavailable/.test(t) && t; }, 30000, 'the Miner map').catch((e) => e.message);
      ok('the Miner map says miner locations are unavailable and that peers are not placed by IP address, with no GeoIP installer named',
        /Miner locations are not published: the network does not record them\./.test(map) && /not used to place them/.test(map) && !/GeoIP\.sh|GeoIP gateway update/.test(map), String(map).slice(String(map).indexOf('Miner locations'), String(map).indexOf('Miner locations') + 300));
      await page.click('nav button:has-text("Overview")');
      await sleep(16000); // past one 15 s refresh of the overview and the transaction history
      const biggest = Math.max(0, ...answers.map((a) => a.bytes));
      ok(`every relay answer stays under 4 MiB (largest ${biggest} B over ${answers.length} answers)`, answers.length > 0 && biggest < 4194304);
      const calls = bodies.flatMap((b) => (Array.isArray(b) ? b : [b])).filter(Boolean);
      const blockLists = bodies.filter((b) => Array.isArray(b) && b.length > 1 && b.every((c) => c.method === 'eth_getBlockByNumber'));
      ok(`the refresh reads one pocol_getMembers page [0,256] and light blocks only (${blockLists.length} block lists, all full=false), never pocol_getMiners`,
        !calls.some((c) => c.method === 'pocol_getMiners') && calls.some((c) => c.method === 'pocol_getMembers' && JSON.stringify(c.params) === '[0,256]')
        && blockLists.length >= 2 && blockLists.every((b) => b.every((c) => c.params[1] === false)), JSON.stringify(calls.map((c) => c.method + JSON.stringify(c.params))).slice(0, 400));
      const full = calls.filter((c) => c.method === 'eth_getBlockByNumber' && c.params[1] === true);
      ok(`full blocks are read one per request only (${full.length}: the account activity's blocks with transactions)`, full.length > 0 && bodies.every((b) => !Array.isArray(b) || b.length === 1 || b.every((c) => !(c.method === 'eth_getBlockByNumber' && c.params[1] === true))), '');
      const unexpected = outside.filter((u) => !/^https:\/\/fonts\.(googleapis|gstatic)\.com\//.test(u));
      ok(`no request left the page's origin except the font stylesheet (${outside.length} aborted)`, unexpected.length === 0, unexpected.slice(0, 5).join(' '));
      ok('the explorer raised no uncaught error', errors.length === 0, errors.join(' | '));
      ok('the explorer ran under its CSP without a violation', csp.length === 0, csp.join(' | '));
      writeFileSync(join(work, 'explorer-text.txt'), String(shown).slice(0, 20000));
      ok('no page shows the test-fixture banner or synthetic data', !/Test fixture|TEST FIXTURE|Synthetic data/i.test(await text()));
      await page.close();

      // ---- 390 and 1366 px: no horizontal scroll on any tab or view (screenshots kept with the work directory) ----
      const overflow = {}, lateErrors = [];
      for (const [width, height] of [[390, 844], [1366, 900]]) {
        const p2 = await ctx.newPage(); await p2.setViewportSize({ width, height }); p2.on('pageerror', (e) => lateErrors.push(`${width}px: ${e.message}`));
        await p2.goto(`http://127.0.0.1:${FRONT}/`);
        await wait(async () => /Latest block\s+[\d,]+/.test(await p2.evaluate(() => document.body.innerText)), 60000, `the page at ${width} px`);
        const wide = async (what) => { const w = await p2.evaluate(() => [document.documentElement.scrollWidth, window.innerWidth]); if (w[0] > w[1] + 1) overflow[`${width}px ${what}`] = w; await p2.screenshot({ path: join(work, `explorer-${width}-${what.replace(/\W+/g, '-')}.png`), fullPage: false }); };
        const menu = async (name) => { const nav = p2.locator(`nav button:has-text("${name}")`); if (!(await nav.first().isVisible())) { const toggle = p2.locator('button[aria-label="Open navigation"]'); if (await toggle.count()) await toggle.first().click(); } await nav.first().click(); await sleep(1500); };
        await wide('overview');
        for (const tab of ['Blocks', 'Transactions', 'Miners', 'Miner map']) { await menu(tab); await wide(tab); }
        await menu('Overview');
        for (const [what, q] of [['block', String(Number(receipts[1].blockNumber))], ['transaction', replacing], ['account', newcomer]]) {
          await p2.fill('input[aria-label="Search block, transaction or address"]', q); await p2.press('input[aria-label="Search block, transaction or address"]', 'Enter');
          await wait(async () => { const t = await p2.evaluate(() => document.querySelector('dialog.detail-dialog')?.innerText || ''); return t && !/Looking up network data/.test(t) && t; }, 60000, `the ${what} view at ${width} px`);
          const inner = await p2.evaluate(() => { const d = document.querySelector('dialog.detail-dialog'); return d ? [d.scrollWidth, d.clientWidth] : [0, 0]; });
          if (inner[0] > inner[1] + 1) overflow[`${width}px ${what} view`] = inner;
          await wide(what + '-view');
          await p2.click('button[aria-label="Close details"]'); await sleep(500);
        }
        await p2.close();
      }
      ok('at 390 and 1366 px no tab and no block, transaction or account view scrolls horizontally', Object.keys(overflow).length === 0, JSON.stringify(overflow));

      // ---- automatic refresh, and the error state while the gateway is stopped ----
      const p3 = await ctx.newPage(); await p3.goto(`http://127.0.0.1:${FRONT}/`);
      const latestOf = async () => Number((/Latest block\s+([\d,]+)/.exec(await p3.evaluate(() => document.body.innerText)) || [0, '-1'])[1].replace(/,/g, ''));
      const h0 = await wait(async () => { const h = await latestOf(); return h >= 0 && h + 1; }, 60000, 'the head') - 1;
      const h1 = await wait(async () => { const h = await latestOf(); return h > h0 && h; }, 40000, 'the head moves without a reload').catch(() => h0);
      ok('the Overview\'s head moves without a reload (automatic refresh)', h1 > h0, `${h0} -> ${h1}`);
      egw.kill('SIGTERM'); await sleep(1000);
      const delayed = await wait(async () => /Data delayed/.test(await p3.evaluate(() => document.body.innerText)), 60000, '"Data delayed"').catch(() => false);
      egw = spawnLogged('explorer-gateway-2', 'python3', gatewayArgs, gatewayEnv);
      const back = await wait(async () => /\bConnected\b/.test(await p3.evaluate(() => document.body.innerText)) && (await latestOf()) > h1, 90000, 'Connected again').catch(() => false);
      ok('with the gateway stopped the page says "Data delayed", and "Connected" with a newer head once it is back', delayed && back, `delayed=${delayed} back=${back}`);
      await p3.close();
      ok('the pages at 390 and 1366 px raised no uncaught error', lateErrors.length === 0, lateErrors.join(' | '));

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
