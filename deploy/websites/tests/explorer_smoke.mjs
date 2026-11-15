// Sites W11: the explorer snapshot against a live v3 devnet, served from a temporary copy (under $RUNNER_TEMP in CI,
// else the system temp directory) switched to the devnet's identity with the W9 tool (tools/site_identity.py --to: the
// anchored edits the release switch uses). The IDE smoke (its copy switched the same way, rpcUrl at a local Studio
// gateway, a contract compiled with its bundled compiler and deployed through the extension) is in
// prototypes/pocol-extension/tests/e2e_publisher_gateway.mjs, which has the extension and the gateway.
//   node deploy/websites/tests/explorer_smoke.mjs     (needs php, python3, Chromium/Playwright and pocold binaries)
// Explorer: `php -S` on the copy, its api/config.php written from deploy/websites/explorer-config.example.php for the
// devnet (the host's config.php is never read), api/rpc.php relaying to a local explorer gateway
// (deploy/rewards/gateway.py --v3-network) in front of node a. The bundle loads, reads the chain through the PHP relay
// and shows the devnet's live head; no relay answer reaches 4 MiB; no request leaves the page's origin.
// Not covered here (W7, the explorer's v3 membership semantics, has not landed: the bundle has no pocol_getMembers):
// an ACCRUING member shown as "Earning bond" and a registerReplacing transaction decoded with its victim.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, cpSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { connect } from 'node:net';
import { keepOnFailure } from '../../../prototypes/pocol-extension/tests/harness.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), ROOT = join(HERE, '..', '..', '..');
const POCOLD = join(ROOT, 'prototypes', 'pocold'), BIN = process.env.POCOLD_BIN_DIR || join(POCOLD, 'target', 'release');
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || '/opt/node-tools/node_modules/playwright/index.mjs');
const EXE = process.env.POCOL_CHROMIUM || ['/usr/bin/chromium', '/usr/bin/google-chrome'].find((p) => existsSync(p));
const PHP_PORT = 8661, EXPLORER_GW = 8662, STUDIO_GW = 8663; // STUDIO_GW: the rpcUrl the switched copy names (unused here)
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

async function main() {
  for (const p of [30300, 30301, 30302, 8540, 8541, 8542, PHP_PORT, EXPLORER_GW, STUDIO_GW]) if (!(await portFree(p))) throw new Error('port busy ' + p);
  ok('php is available', spawnSync('php', ['-v'], { encoding: 'utf8' }).status === 0);
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
    // ---- the explorer gateway (--v3-network) and the PHP relay ----
    writeFileSync(join(work, 'v3-network.json'), JSON.stringify({ chainId: info.chainId, genesisHash, specVersion: 2, v3: true }));
    const egw = spawnLogged('explorer-gateway', 'python3', ['-B', join(ROOT, 'deploy', 'rewards', 'gateway.py'), '--port', String(EXPLORER_GW), '--rpc-port', '8540', '--v3-network', join(work, 'v3-network.json')]);
    await wait(async () => (await rpc(EXPLORER_GW, 'eth_blockNumber', [], '/rpc')) !== undefined, 30000, 'explorer gateway').catch((e) => { throw new Error(e.message + ' ' + egw.out.slice(-300)); });
    const example = readFileSync(join(ROOT, 'deploy', 'websites', 'explorer-config.example.php'), 'utf8');
    const config = example.replace("'http://127.0.0.1:8088/rpc'", `'http://127.0.0.1:${EXPLORER_GW}/rpc'`).replace("'https://explorer.pocol.net'", `'http://127.0.0.1:${PHP_PORT}'`)
      .replace("'chain_id' => 0", `'chain_id' => ${info.chainId}`).replace(/'genesis_hash' => '0x0{64}'/, `'genesis_hash' => '${genesisHash}'`);
    ok('the test config.php is generated from the committed example (every key replaced)', [`'http://127.0.0.1:${EXPLORER_GW}/rpc'`, `'http://127.0.0.1:${PHP_PORT}'`, `'chain_id' => ${info.chainId},`, `'genesis_hash' => '${genesisHash}'`].every((s) => config.includes(s)));
    writeFileSync(join(explorer, 'api', 'config.php'), config);
    // Several PHP workers: the bundle sends its reads in parallel (php -S serves one request at a time otherwise).
    // api/lib.php keeps its request-slot lock files in the temp directory: the run's own (TMPDIR), removed with it.
    mkdirSync(join(work, 'php-tmp'));
    spawnLogged('php', 'php', ['-S', `127.0.0.1:${PHP_PORT}`, '-t', explorer], { env: { ...process.env, PHP_CLI_SERVER_WORKERS: '4', TMPDIR: join(work, 'php-tmp') } });
    await wait(async () => (await fetch(`http://127.0.0.1:${PHP_PORT}/index.html`)).ok, 20000, 'php -S');
    const relayed = await rpc(PHP_PORT, 'eth_blockNumber', [], '/api/rpc.php');
    ok('api/rpc.php relays a read to the explorer gateway on the devnet', /^0x[0-9a-f]+$/.test(relayed), relayed);
    // health.php compares the gateway's pocol_getParams with config.php's chain_id and genesis_hash: HTTP 200 with
    // networkMatch and PASS only when both match (503 NOT_CONNECTED otherwise, also for an unreachable gateway).
    const hres = await fetch(`http://127.0.0.1:${PHP_PORT}/api/health.php`); const health = await hres.text();
    let hv = null; try { hv = JSON.parse(health); } catch {}
    ok('api/health.php reports the devnet identity matched (config chain_id / genesis_hash against the gateway)', hres.status === 200 && hv && hv.gatewayReachable === true && hv.networkMatch === true && hv.result === 'PASS', `HTTP ${hres.status} ${health.slice(0, 200)}`);

    const browser = await chromium.launch({ executablePath: EXE, headless: true });
    try {
      // Every request outside the page's origin is aborted and counted. The bundle asks Google Fonts for its type faces
      // (index.html); that is the only outside request allowed (aborted all the same), anything else fails the check.
      const ctx = await browser.newContext(); const outside = []; const answers = [];
      await ctx.route('**/*', (route) => { const u = new URL(route.request().url()); if (u.origin === `http://127.0.0.1:${PHP_PORT}`) return route.fallback(); outside.push(u.href); return route.abort(); });
      const page = await ctx.newPage(); const errors = []; page.on('pageerror', (e) => errors.push(e.message));
      const asked = []; page.on('request', (r) => { if (r.url().includes('/api/')) asked.push(r.method() + ' ' + r.url().replace(/^.*\/api\//, '')); });
      page.on('response', async (r) => { if (r.url().includes('/api/')) { try { answers.push({ url: r.url(), status: r.status(), bytes: (await r.body()).length }); } catch (e) { answers.push({ url: r.url(), status: r.status(), bytes: 0, error: e.message }); } } });
      await page.goto(`http://127.0.0.1:${PHP_PORT}/`);
      await wait(() => answers.some((a) => a.url.includes('/api/rpc.php') && a.status === 200), 60000, 'the bundle reads through api/rpc.php').catch(() => {});
      const headNow = Number(await rpc(8540, 'eth_blockNumber'));
      // The overview shows "Connected", the chain ID and "Latest block <n>" for the head it read through the relay.
      const shown = await wait(async () => { const t = await page.evaluate(() => document.body.innerText); const m = /Latest block\s+([\d,]+)/.exec(t); const n = m ? Number(m[1].replace(/,/g, '')) : -1;
        return /\bConnected\b/.test(t) && new RegExp(`Chain ID\\s+${info.chainId}\\b`).test(t) && n >= headNow - 5 && n <= Number(await rpc(8540, 'eth_blockNumber')) && t; }, 60000, 'the explorer shows Connected, the devnet chain ID and its live head');
      const rpcAnswers = answers.filter((a) => a.url.includes('/api/rpc.php'));
      ok('the explorer bundle loads from the copy and shows "Connected", the devnet\'s chain ID and its live head ("Latest block") read through api/rpc.php', rpcAnswers.some((a) => a.status === 200), `${rpcAnswers.length} relay answers; head ${headNow}; api: ${answers.map((a) => a.url.replace(/^.*\/api\//, '') + ':' + a.status).join(' ')}; asked: ${asked.slice(0, 6).join(', ')}`);
      await sleep(7000); // a few 3 s refresh batches
      const biggest = Math.max(0, ...answers.map((a) => a.bytes));
      ok(`every relay answer stays under 4 MiB (largest ${biggest} B over ${answers.length} answers)`, answers.length > 0 && biggest < 4194304);
      const unexpected = outside.filter((u) => !/^https:\/\/fonts\.(googleapis|gstatic)\.com\//.test(u));
      ok(`no request left the page's origin except the font stylesheet (${outside.length} aborted)`, unexpected.length === 0, unexpected.slice(0, 5).join(' '));
      ok('the explorer raised no uncaught error', errors.length === 0, errors.join(' | '));
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
// A failed run keeps its evidence (POCOL_E2E_KEEP in CI, uploaded with evidence/): php.log, explorer-gateway.log,
// explorer-text.txt, the generated config.php, the devnet's node logs and inputs (dn/). The switched copy of the
// sites is left out except the explorer's api/ (the rest is the repository's files with the W9 tool's edits).
if (failed) {
  const tree = join(work, 'tree'), api = join(tree, 'deploy', 'websites', 'explorer.pocol.net', 'api');
  try { if (existsSync(api)) cpSync(api, join(work, 'explorer-api'), { recursive: true }); rmSync(tree, { recursive: true, force: true }); } catch {}
  summary.kept = keepOnFailure('explorer_smoke', work, { results });
}
writeFileSync(join(HERE, 'explorer_smoke.last.json'), JSON.stringify({ ...summary, results }, null, 1) + '\n');
if (!failed && process.env.E2E_KEEP !== '1') rmSync(work, { recursive: true, force: true });
console.log(JSON.stringify(summary));
process.exit(failed ? 1 : 0);
