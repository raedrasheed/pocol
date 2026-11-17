// Sites W11: Studio -> publisher.html -> publish through a local Studio gateway -> view through the same gateway, on a
// v3 devnet, with the extension's release RN path (the `testGateway` fault build: the built-in RN profile is the devnet,
// served at http://127.0.0.1:<port>/studio/rpc; build.mjs, lib/studio-network.js).
//   node tests/e2e_publisher_gateway.mjs            (E2E_PROFILE=pr, the default: a 1.6 MB project)
//   E2E_PROFILE=nightly node tests/e2e_publisher_gateway.mjs   (adds a 4 MiB site and three concurrent viewers, and a
//                                                   publication in a background tab; run headed under xvfb-run with
//                                                   E2E_HEADED=1 to measure Chrome's background-tab timer throttling)
// What runs: a 3-node v3 devnet; deploy/studio/pocol_studio_gateway.py --network devnet.json in front of node a (a tap
// between them counts what the gateway asks the node); per-viewer relays in front of the gateway set distinct
// X-Pocol-Client-IP headers (as Caddy does), so the gateway sees separate clients; Chromium with the testGateway build;
// PoCol Studio (prototypes/pocol-studio/dist, its pinned identity replaced by the devnet's) served at
// https://studio.pocol.net and its gateway calls at https://explorer.pocol.net routed to the local gateway.
// Checks: the folder import and the handoff to publisher.html; fees read from the node through the gateway (W4), no
// "fee rose" pause, the spend within the approved budget; the publication outlasts the build's shortened LOCK_MS
// (the key lease, W4) and Lock aborts a running one; the site opens through the gateway (CSS, JS, ES modules, font, JSON,
// rpc_read); the IDE copy (W9 tool, rpcUrl at its own relay) loads the deployed contract and reads it in its own UI;
// three viewers with distinct client addresses open it at once within the owner's Q8 target (OPEN_TARGET_MS) and
// without an HTTP 429 from the gateway, and the gateway's chunk cache answers some of their reads (fewer eth_getCode
// reach the node than the viewers sent).
import { spawn, spawnSync } from 'node:child_process';
import { createServer, request as httpRequest } from 'node:http';
import { mkdtempSync, mkdirSync, cpSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, dirname } from 'node:path';
import { build } from '../build.mjs';
import { EXT, BIN, sleep, wait, checker, devnet, rpc, portFree, startTap, siteFrame, keepOnFailure, chromium } from './harness.mjs';
import { w10Site, writeSiteFiles, SITE_MAX } from './w10_site.mjs';

const PROFILE = process.env.E2E_PROFILE || 'pr'; if (!['pr', 'nightly'].includes(PROFILE)) throw new Error('E2E_PROFILE is pr or nightly');
const HEADED = process.env.E2E_HEADED === '1';
const GW = 8650, NODE_TAP = 8654, RELAYS = [8651, 8652, 8653], IDE_RELAY = 8655, LOCK_MS = 30000;
// The owner's Q8 answer (SITES-PLAN.md): a 4 MiB site opens in about 10 s with the gateway's budgets (W1/W2). The
// concurrent open below must stay within it, and within the gateway's request budget (no HTTP 429 to any viewer).
const OPEN_TARGET_MS = 10000;
const { ok, results, failed } = checker();
const work = mkdtempSync(join(tmpdir(), 'pocol-pubgw-'));
const REPO = join(EXT, '..', '..'), STUDIO = join(EXT, '..', 'pocol-studio', 'dist');
const procs = []; const servers = []; const contexts = [];
const q = (v) => BigInt(v);

/** A relay in front of the gateway for one client: sets X-Pocol-Client-IP (the gateway's per-client key) and records
 *  every JSON-RPC method with its time, the request's Origin (which page sent it) and the gateway's HTTP status (429:
 *  the client's request budget was exceeded). GET /studio/health is relayed too (Studio's connection check). */
function startRelay(port, clientIp) {
  const relay = { log: [], clientIp };
  const server = createServer((req, res) => {
    const parts = []; req.on('data', (c) => parts.push(c)); req.on('end', () => {
      const body = Buffer.concat(parts); let m = null; try { m = JSON.parse(body); } catch {}
      const entries = [];
      for (const x of (Array.isArray(m) ? m : [m])) if (x && x.method) entries.push({ method: x.method, tag: Array.isArray(x.params) ? x.params[1] : null, t: Date.now(), origin: req.headers.origin || null, status: null });
      relay.log.push(...entries);
      const headers = { 'X-Pocol-Client-IP': clientIp }; if (req.headers['content-type']) headers['content-type'] = req.headers['content-type']; if (body.length) headers['content-length'] = body.length;
      const up = httpRequest({ host: '127.0.0.1', port: GW, method: req.method, path: req.url, headers }, (r) => { for (const e of entries) e.status = r.statusCode; res.writeHead(r.statusCode, { ...r.headers, 'access-control-allow-origin': '*' }); r.pipe(res); });
      up.on('error', () => { for (const e of entries) e.status = 502; res.writeHead(502, { 'access-control-allow-origin': '*' }); res.end(); }); up.end(body);
    });
  });
  relay.count = (mm, tag) => relay.log.filter((x) => x.method === mm && (tag === undefined || x.tag === tag)).length;
  servers.push(server);
  return new Promise((r) => server.listen(port, '127.0.0.1', () => r(relay)));
}
/** Studio's static files, with its pinned network identity (chain id and genesis hash, app.js) replaced by the devnet's. */
function studioRoute(info) {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.ico': 'image/x-icon', '.webmanifest': 'application/manifest+json' };
  const app = readFileSync(join(STUDIO, 'app.js'), 'utf8');
  const pinned = { chain: (/Number\(cid\)!==(\d+)/.exec(app) || [])[1], genesis: (/toLowerCase\(\)!=='(0x[0-9a-f]{64})'/.exec(app) || [])[1] };
  if (!pinned.chain || !pinned.genesis) throw new Error('Studio app.js: the pinned identity was not found');
  const patched = app.replaceAll(pinned.chain, String(info.chainId)).replaceAll(pinned.genesis, info.genesisHash.toLowerCase());
  return async (route) => {
    const u = new URL(route.request().url()); let p = u.pathname === '/' ? '/index.html' : u.pathname;
    const f = join(STUDIO, p.replace(/^\/+/, ''));
    if (p.includes('..') || !existsSync(f)) return route.fulfill({ status: 404, body: 'not found' });
    return route.fulfill({ status: 200, contentType: types[extname(f)] || 'application/octet-stream', body: p === '/app.js' ? patched : readFileSync(f) });
  };
}
/** A browser profile with one testGateway build (its built-in RN profile reaches the gateway through `relayPort`). */
async function openBrowser(testNetwork, relayPort, tag, { headless = !HEADED } = {}) {
  const dist = build({ fault: 'testGateway', out: join(work, 'dist-' + tag), lockMs: LOCK_MS, testNetwork: { ...testNetwork, endpoint: `http://127.0.0.1:${relayPort}/studio/rpc` } }).dist;
  const userDir = mkdtempSync(join(work, 'chrome-' + tag + '-'));
  // Headed (the nightly background-tab measurement): Playwright's default flags that turn Chrome's background timer
  // throttling and renderer backgrounding off are dropped, so a tab behind another one is throttled as in Chrome.
  const ignoreDefaultArgs = headless ? [] : ['--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows'];
  const ctx = await chromium.launchPersistentContext(userDir, { channel: 'chromium', headless, acceptDownloads: true, ignoreDefaultArgs, args: [`--disable-extensions-except=${dist}`, `--load-extension=${dist}`] });
  contexts.push(ctx);
  let [sw] = ctx.serviceWorkers(); if (!sw) sw = await ctx.waitForEvent('serviceworker', { timeout: 20000 });
  const id = new URL(sw.url()).host;
  // Studio and its gateway calls: Studio's fixed https://explorer.pocol.net endpoints go to this viewer's relay.
  await ctx.route('https://explorer.pocol.net/**', async (route) => {
    const r = route.request(); const u = new URL(r.url());
    const res = await fetch(`http://127.0.0.1:${relayPort}${u.pathname}`, { method: r.method(), headers: { 'content-type': 'application/json' }, body: r.method() === 'POST' ? r.postDataBuffer() : undefined });
    return route.fulfill({ status: res.status, headers: { 'access-control-allow-origin': '*', 'content-type': 'application/json' }, body: Buffer.from(await res.arrayBuffer()) });
  });
  return { ctx, id, dist, tag };
}
/** Reads fresh fees, approves and starts the publication on publisher page `pub`; returns once #publish was pressed. */
async function approveAndPublish(pub) {
  await pub.click('#refreshFees');
  await pub.waitForFunction(() => /PCOL/.test(document.getElementById('fee').textContent) && !document.getElementById('refreshFees').disabled, null, { timeout: 60000 });
  await pub.check('#consent');
  await pub.waitForFunction(() => !document.getElementById('publish').disabled, null, { timeout: 30000 });
  const before = (await pub.textContent('#status')) || '';
  await pub.click('#publish');
  await pub.waitForFunction((s) => document.getElementById('status').textContent !== s, before, { timeout: 30000 }).catch(() => {});
}
/** Waits until `done(page)` holds, logging every new #status text; fails with the last status after `ms`. */
async function follow(pub, done, ms, what) {
  const seen = []; const t0 = Date.now();
  for (;;) {
    const s = (await pub.textContent('#status').catch(() => '')) || '';
    if (s && s !== seen[seen.length - 1]) { seen.push(s); console.log(`STATUS ${what} +${((Date.now() - t0) / 1000).toFixed(0)}s: ${s.replace(/\s+/g, ' ').slice(0, 160)}`); }
    if (await done(pub).catch(() => false)) return seen;
    // The page's terminal error states (publisher-ui.js): a stopped publication says how to resume it.
    if (/Reopen this job and confirm again|Review the new budget/.test(s)) throw new Error(`${what}: the publication stopped: ${s.slice(0, 300)}`);
    if (Date.now() - t0 > ms) throw new Error(`${what}: not done after ${ms} ms; last status: ${s.slice(0, 300)}`);
    await sleep(500);
  }
}
/** page.waitForFunction that, on a timeout, reports the page's #status text. */
async function waitStatus(page, fn, ms, what) {
  try { await page.waitForFunction(fn, null, { timeout: ms }); }
  catch (e) { throw new Error(`${what}: ${e.message.split('\n')[0]}; status: ${((await page.textContent('#status').catch(() => '')) || '').slice(0, 300)}`); }
}
async function viewSite(b, site, name) {
  const t0 = Date.now(); const page = await b.ctx.newPage(); const errors = []; page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`chrome-extension://${b.id}/viewer.html#${name}/${site}/`);
  const { fr } = await siteFrame(page); const openMs = Date.now() - t0;
  const v = await fr.evaluate(async () => {
    const until = Date.now() + 8000; while (Date.now() < until && !(document.body.dataset.dyn && document.body.dataset.font && document.body.dataset.rows)) await new Promise((r) => setTimeout(r, 100));
    const h = document.getElementById('t');
    return { h1: h.textContent, color: getComputedStyle(h).color, loaded: h.dataset.loaded, mod: document.body.dataset.mod, dyn: document.body.dataset.dyn, font: document.body.dataset.font, rows: document.body.dataset.rows,
      fontFamily: getComputedStyle(document.getElementById('font')).fontFamily, png: document.getElementById('png').naturalWidth, svg: document.getElementById('svg').naturalWidth };
  });
  // eth_call: the site's own Website current() (0x9fa6a6e3), whose first word is the current version.
  const read = await fr.evaluate(async (to) => { try { return { block: await pocol.request('rpc_read', 'eth_blockNumber', []), call: await pocol.request('rpc_read', 'eth_call', [{ to, data: '0x9fa6a6e3' }, 'latest']) }; } catch (e) { return { error: e.code + ' ' + e.message }; } }, site);
  const trust = await page.getAttribute('#trust', 'data-level');
  return { page, v, read, trust, errors, openMs };
}

async function main() {
  for (const p of [30300, 30301, 30302, 8540, 8541, 8542, GW, NODE_TAP, ...RELAYS, IDE_RELAY]) if (!(await portFree(p))) throw new Error('port busy ' + p);
  // A devnet's CODE_CAP is 2 MiB (caps/64): the nightly 4 MiB site after the 1.6 MB project needs a larger one.
  devnet(['init', work, '--miners', 'a,b,c', '--t-blk', '2', '--tau', '20', '--c', '8', '--d-att', '41', '--fb-wait', '3', '--work-bits', '15', ...(PROFILE === 'nightly' ? ['--code-cap', String(16 << 20)] : [])]);
  const info = JSON.parse(readFileSync(join(work, 'net', 'devnet.json'), 'utf8')); const cid = String(info.chainId);
  const profile = JSON.parse(readFileSync(join(work, 'net', 'profile.json'), 'utf8'));
  devnet(['start', work, '--log']);
  try {
    await wait(async () => Math.min(...await Promise.all([0, 1, 2].map(async (i) => Number(await rpc(8540 + i, 'eth_blockNumber'))))) >= 8, 180000, 'devnet height >= 8');
    ok('the devnet is v3 (specVersion 2, V3 programs, nonceMode 1, continuous)', info.specVersion === 2 && info.v3 === true && (await rpc(8540, 'pocol_getCaps')).v3 === true && (await rpc(8540, 'pocol_getParams')).nonceMode === 1, JSON.stringify({ specVersion: info.specVersion, v3: info.v3 }));
    // ---- the Studio gateway with --network devnet.json, a tap between it and node a ----
    const nodeTap = await startTap(NODE_TAP, 8540); servers.push({ close: () => nodeTap.close() });
    writeFileSync(join(work, 'gateway-network.json'), JSON.stringify({ chainId: info.chainId, genesisHash: info.genesisHash, specVersion: 2, v3: true }));
    const gw = spawn('python3', ['-B', join(REPO, 'deploy', 'studio', 'pocol_studio_gateway.py'), '--port', String(GW), '--rpc-port', String(NODE_TAP), '--network', join(work, 'gateway-network.json')], { stdio: ['ignore', 'pipe', 'pipe'] });
    procs.push(gw); let gwOut = ''; gw.stdout.on('data', (c) => { gwOut += c; }); gw.stderr.on('data', (c) => { gwOut += c; });
    await wait(() => /ready on loopback/.test(gwOut), 30000, 'gateway ready');
    const relays = await Promise.all(RELAYS.map((p, i) => startRelay(p, `198.51.100.${i + 1}`)));
    // The IDE copy's rpcUrl (W9 tool --rpc-url) gets a relay of its own: only the IDE page's own reads reach it.
    const ideRelay = await startRelay(IDE_RELAY, '198.51.100.9');
    const health = await (await fetch(`http://127.0.0.1:${RELAYS[0]}/studio/health`)).json();
    ok('the Studio gateway serves the devnet identity (--network)', health.ready === true && health.chainId === info.chainId && health.genesisHash === info.genesisHash.toLowerCase(), JSON.stringify(health));
    const testNetwork = { name: 'devnet-gw', chainId: info.chainId, genesisHash: info.genesisHash.toLowerCase(), genesisPre: profile.genesisPre };

    // ---- Studio -> publisher.html -> publish a 1.6 MB project through the gateway ----
    const b = await openBrowser(testNetwork, RELAYS[0], 'pub');
    const manifest = JSON.parse(readFileSync(join(b.dist, 'manifest.json'), 'utf8'));
    ok('the browser runs the testGateway fault build (its built-in RN profile is the devnet behind the loopback gateway)', /\[TEST FAULT testGateway\]$/.test(manifest.name) && readFileSync(join(b.dist, 'lib', 'studio-network.js'), 'utf8').includes(`127.0.0.1:${RELAYS[0]}/studio/rpc`), manifest.name);
    await b.ctx.route('https://studio.pocol.net/**', studioRoute(info));
    const studioErrors = [];
    // Each publication starts from its own Studio tab (Studio follows one job per tab).
    async function openStudio() {
      const page = await b.ctx.newPage(); page.on('pageerror', (e) => studioErrors.push(e.message));
      await page.goto('https://studio.pocol.net/');
      await page.waitForFunction(() => /Direct connection available|اتصال مباشر متاح/.test(document.querySelector('#network-state')?.textContent || ''), null, { timeout: 30000 });
      return page;
    }
    let studio = await openStudio();
    ok('Studio (served at https://studio.pocol.net) sees the gateway as connected to the expected network', true);
    const projectBytes = 1600000;
    const base = Object.values(w10Site({ version: 1 })).reduce((a, c) => a + Buffer.byteLength(c), 0);
    const pdir = join(work, 'project'); const pfiles = w10Site({ version: 1, padBytes: projectBytes - base, seed: 7 }); const wrote = writeSiteFiles(pdir, pfiles);
    async function studioPublish(dir, fresh = false) {
      if (fresh) { await studio.close(); studio = await openStudio(); }
      await studio.setInputFiles('#folder-input', dir);
      await studio.waitForSelector('#import-confirm', { timeout: 20000 }); await studio.click('#import-confirm');
      await studio.waitForSelector('#modal', { state: 'hidden', timeout: 20000 }).catch(() => {});
      await studio.click('#publish-btn'); await studio.waitForSelector('#publish-confirm', { timeout: 20000 });
      const opened = b.ctx.waitForEvent('page', { timeout: 30000 }); await studio.click('#publish-confirm');
      const pub = await opened; await pub.waitForLoadState('domcontentloaded'); return pub;
    }
    const pub = await studioPublish(pdir);
    ok('Studio hands the imported folder to the extension: publisher.html opens with the job', /publisher\.html#[a-f0-9-]{36}$/.test(pub.url()), pub.url());
    await pub.waitForFunction(() => /PCOL/.test(document.getElementById('fee').textContent), null, { timeout: 60000 });
    const files = Number(await pub.textContent('#fileCount'));
    ok(`publisher.html lists the ${wrote.files} files (${(wrote.bytes / 1e6).toFixed(2)} MB) and quotes fees read from the node`, files === wrote.files, `${files} files; ${await pub.textContent('#feeDetails')}`);
    // A new wallet, funded from the devnet's alice.
    await pub.fill('#password', 'devnet-password-only'); await pub.click('details:has(#create) > summary'); await pub.click('#create');
    await pub.waitForFunction(() => /^0x[0-9a-fA-F]{40}$/.test(document.getElementById('walletAddress').textContent), null, { timeout: 60000 });
    const wallet = (await pub.textContent('#walletAddress')).trim();
    const aliceNonce = Number(await rpc(8540, 'eth_getTransactionCount', [info.accounts.find((a) => a.label === 'alice').address, 'pending']));
    const fund = spawnSync(BIN('pocol-sign'), ['--chain-id', cid, '--account', 'alice', '--to', wallet, '--value', '20000000000000000000', '--nonce', String(aliceNonce), '--gas', '21000'], { encoding: 'utf8' }).stdout.trim();
    await rpc(8540, 'eth_sendRawTransaction', [fund]); await wait(async () => BigInt(await rpc(8540, 'eth_getBalance', [wallet, 'latest'])) > 0n, 90000, 'wallet funded');
    const balance0 = BigInt(await rpc(8540, 'eth_getBalance', [wallet, 'latest']));
    const sent0 = relays[0].count('eth_sendRawTransaction'), t0 = Date.now();
    await approveAndPublish(pub);
    const statuses = await follow(pub, (p) => p.isVisible('#success'), 1800000, 'publish');
    const elapsed = Date.now() - t0;
    const site = (await pub.textContent('#resultAddress')).trim();
    const jobs = await pub.evaluate(() => chrome.storage.local.get(null));
    const job = Object.values(jobs).find((j) => j && j.result && j.result.site && j.result.site.toLowerCase() === site.toLowerCase());
    const balance1 = BigInt(await rpc(8540, 'eth_getBalance', [wallet, 'latest']));
    const spent = balance0 - balance1, sent = relays[0].count('eth_sendRawTransaction') - sent0;
    ok(`the 1.6 MB project published through the gateway in ${(elapsed / 1000).toFixed(0)} s (${sent} transactions)`, /^0x[0-9a-fA-F]{40}$/.test(site) && job && job.status === 'complete', site);
    ok('no "fee rose" pause during the publication', ![...statuses].some((s) => /fee rose|rose/i.test(s)) && !(job && job.error), [...statuses].filter(Boolean).slice(0, 8).join(' | ').slice(0, 300));
    ok('fees came from the node through the gateway (eth_gasPrice, eth_maxPriorityFeePerGas, eth_feeHistory, eth_getBlockByNumber)', ['eth_gasPrice', 'eth_maxPriorityFeePerGas', 'eth_feeHistory', 'eth_getBlockByNumber'].every((m) => relays[0].count(m) > 0), ['eth_gasPrice', 'eth_maxPriorityFeePerGas', 'eth_feeHistory'].map((m) => m + '=' + relays[0].count(m)).join(' '));
    ok(`the spend (${spent} wei) is within the approved budget (${job && job.approval && job.approval.budgetWei} wei)`, job && job.approval && spent > 0n && spent <= q(job.approval.budgetWei), JSON.stringify(job && job.approval && { budgetWei: job.approval.budgetWei, ceilingWei: job.approval.ceilingWei }));
    ok(`the publication outlasted the build's LOCK_MS (${LOCK_MS / 1000} s) by the key lease: ${(elapsed / 1000).toFixed(0)} s, the wallet still unlocked at the end`, elapsed > 2 * LOCK_MS && /^0x/.test((await pub.textContent('#walletAddress')).trim()), `${elapsed} ms`);
    // The publisher reads the site back (fetchSite) before it reports completion; the CLI reads it on every node too.
    const fetched = [0, 1, 2].map((i) => { const out = mkdtempSync(join(work, 'fetch-')); const r = spawnSync(BIN('pocol-site'), ['fetch', '--rpc', `127.0.0.1:${8540 + i}`, '--site', site, '--out', out], { encoding: 'utf8' }); return r.status === 0 ? JSON.parse(r.stdout).files.map((f) => f.path + ':' + f.keccak).join() : 'error ' + r.stderr; });
    ok('every node serves the published files (pocol-site fetch, identical)', fetched.every((x) => x === fetched[0] && !x.startsWith('error')) && fetched[0].split(',').length === wrote.files, fetched[0].slice(0, 120));

    // ---- view through the same gateway ----
    const v1 = await viewSite(b, site, 'devnet-gw');
    ok('the viewer opens the site through the gateway at trust level RN', v1.trust === 'RN', v1.trust);
    ok('CSS applied, the classic script ran, the module graph resolved (main -> lib -> util, literal import()), the woff2 font loaded, JSON, PNG and SVG',
      v1.v.h1 === 'PoCol W10 site v1' && v1.v.color === 'rgb(17, 170, 85)' && v1.v.loaded === 'yes' && v1.v.mod === 'lib>util v1' && v1.v.dyn === 'dynamic' && v1.v.font === '1' && v1.v.rows === '1400' && /PoColW10/.test(v1.v.fontFamily) && v1.v.png === 2 && v1.v.svg === 4, JSON.stringify(v1.v));
    ok('window.pocol rpc_read works through the gateway (eth_blockNumber, eth_call of the site\'s current())', /^0x[0-9a-f]+$/.test(v1.read.block || '') && /^0x[0-9a-f]{384,}$/.test(v1.read.call || '') && BigInt(v1.read.call.slice(0, 66)) === 0n, JSON.stringify(v1.read).slice(0, 200));
    ok('no uncaught errors in Studio, publisher or viewer', studioErrors.length === 0 && v1.errors.length === 0, [...studioErrors, ...v1.errors].join(' | '));
    await v1.page.close();

    // ---- IDE smoke: the IDE snapshot switched to the devnet (W9 tool: frozen network, rpcUrl at this gateway), a
    // contract compiled with the IDE's bundled compiler and deployed through ide-content.js -> contract.html, then
    // loaded and read in the IDE's own UI through its rpcUrl (its own relay, IDE_RELAY) ----
    {
      const tree = join(work, 'ide-tree');
      for (const top of ['deploy/websites', 'prototypes/pocol-studio/dist']) cpSync(join(REPO, top), join(tree, top), { recursive: true, filter: (s) => !/__pycache__|[\\/]config\.php$/.test(s) });
      for (const f of ['deploy/mainnet/retired-networks.json', 'prototypes/pocol-extension/package.json']) { mkdirSync(dirname(join(tree, f)), { recursive: true }); cpSync(join(REPO, f), join(tree, f)); }
      const spec = readFileSync(join(work, 'net', 'genesis.spec.hex'), 'utf8').trim();
      writeFileSync(join(work, 'network.pocol.json'), JSON.stringify({ schema: 'pocol-network-v2', consensusRulesVersion: 2, seeds: ['192.0.2.10:30300'],
        files: { 'genesis.spec.hex': spec.startsWith('0x') ? spec : '0x' + spec, 'devnet.json': JSON.stringify({ chainId: info.chainId, genesisHash: info.genesisHash.toLowerCase() }) } }));
      // TEST-NET seeds and gateway host (RFC 5737): the explorer bundle already holds 127.0.0.1 outside its anchors.
      const sw = spawnSync('python3', ['-B', join(REPO, 'deploy', 'websites', 'tools', 'site_identity.py'), '--root', tree, '--to', join(work, 'network.pocol.json'), '--gateway-host', '192.0.2.10', '--rpc-url', `http://127.0.0.1:${IDE_RELAY}/studio/rpc`], { encoding: 'utf8' });
      ok('IDE: the W9 tool switches a copy of the IDE to the devnet (frozen network object, rpcUrl at the local Studio gateway)', sw.status === 0, (sw.stderr || sw.stdout).slice(-300));
      const ide = join(tree, 'deploy', 'websites', 'ide.pocol.net');
      const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.ico': 'image/x-icon', '.json': 'application/json', '.wasm': 'application/wasm', '.ttf': 'font/ttf', '.webmanifest': 'application/manifest+json', '.txt': 'text/plain' };
      await b.ctx.route('https://ide.pocol.net/**', (route) => { const u = new URL(route.request().url()); const p = u.pathname === '/' ? '/index.html' : decodeURIComponent(u.pathname); const f = join(ide, p);
        if (p.includes('..') || !existsSync(f)) return route.fulfill({ status: 404, body: 'not found' }); return route.fulfill({ status: 200, contentType: types[extname(f)] || 'application/octet-stream', body: readFileSync(f) }); });
      // The test's gateway is on loopback, which Chrome's Local Network Access shields from a public https page until the
      // user allows it; the IDE's origin is allowed here (the release rpcUrl is the public https gateway).
      await b.ctx.grantPermissions(['local-network-access'], { origin: 'https://ide.pocol.net' });
      const idePage = await b.ctx.newPage(); const ideErrors = []; idePage.on('pageerror', (e) => ideErrors.push(e.message));
      // The context routes https://explorer.pocol.net (the release rpcUrl) to relay 0 for Studio; the IDE page may not
      // reach it: an IDE copy left on the release rpcUrl is caught here, not served.
      const ideToRelease = []; await idePage.route('https://explorer.pocol.net/**', (route) => { ideToRelease.push(route.request().url()); return route.abort(); });
      await idePage.goto('https://ide.pocol.net/', { waitUntil: 'load' }); await sleep(4000);
      const compiled = await idePage.evaluate(async () => {
        const worker = new Worker('/compiler/worker.js');
        try {
          const input = { language: 'Solidity', sources: { 'W11.sol': { content: 'pragma solidity ^0.8.38; contract W11 { uint256 public count = 7; function increment() external { count += 1; } }' } }, settings: { evmVersion: 'cancun', outputSelection: { '*': { '*': ['abi', 'evm.bytecode.object'] } } } };
          const out = await new Promise((resolve, reject) => { const t = setTimeout(() => reject(new Error('compiler deadline')), 60000); worker.onmessage = (e) => { clearTimeout(t); resolve(e.data); }; worker.postMessage({ id: 1, input }); });
          const c = out.result && out.result.contracts && out.result.contracts.find((x) => x.name === 'W11');
          return { compiler: out.result && out.result.compiler, abi: c && c.abi, bytecode: c && c.bytecode, error: out.error && out.error.message };
        } finally { worker.terminate(); }
      });
      ok('IDE: its bundled Solidity compiler compiles the contract', /^0\.8\.38/.test(compiled.compiler || '') && /^0x[0-9a-f]+$/.test(compiled.bytecode || '') && Array.isArray(compiled.abi), JSON.stringify({ compiler: compiled.compiler, error: compiled.error }));
      const opened = b.ctx.waitForEvent('page', { timeout: 30000 });
      const reply = await idePage.evaluate((req) => new Promise((resolve) => {
        const id = 'w11-' + Math.random().toString(16).slice(2);
        const on = (e) => { if (e.source === window && e.data && e.data.source === 'pocol-ide-extension' && e.data.id === id) { removeEventListener('message', on); resolve(e.data); } };
        addEventListener('message', on); postMessage({ source: 'pocol-ide', id, method: 'openContract', payload: req }, location.origin); setTimeout(() => resolve({ timeout: true }), 20000);
      }), { schema: 1, action: 'deploy', title: 'W11 IDE smoke', abi: compiled.abi, bytecode: compiled.bytecode, args: [], valueWei: '0' });
      ok('IDE: ide-content.js hands the deployment to the extension (a contract job)', reply.ok === true && /^[a-f0-9-]{36}$/.test((reply.result && reply.result.jobId) || ''), JSON.stringify(reply).slice(0, 200));
      const cpage = await opened; await cpage.waitForLoadState('domcontentloaded');
      await cpage.fill('#password', 'devnet-password-only'); await cpage.click('#unlock');
      await cpage.waitForFunction(() => /^0x[0-9a-fA-F]{40}$/.test(document.getElementById('walletAddress').textContent.trim()), null, { timeout: 60000 });
      await cpage.click('#prepare'); await waitStatus(cpage, () => !document.getElementById('consent').disabled && /ready/i.test(document.getElementById('status').textContent), 60000, 'IDE prepare');
      await cpage.check('#consent'); await cpage.click('#confirm');
      await waitStatus(cpage, () => /^0x[0-9a-fA-F]{40}$/.test(document.getElementById('resultAddress').textContent.trim()) && /included/i.test(document.getElementById('status').textContent), 180000, 'IDE deploy');
      const caddr = (await cpage.textContent('#resultAddress')).trim();
      const n = await rpc(8541, 'eth_call', [{ to: caddr, data: '0x06661abd' }, 'latest']);
      ok('IDE: the contract is deployed on the devnet through the extension and the gateway (code on node b, count() = 7)', (await rpc(8541, 'eth_getCode', [caddr, 'latest'])).length > 4 && BigInt(n) === 7n, `${caddr} count=${n}`);
      await cpage.close();
      // The IDE's own reads: compile its starter Counter (count() has the same selector), choose the public network
      // (the IDE checks eth_chainId and pocol_status against its frozen network), load the deployed contract at its
      // address (eth_getCode) and read count (eth_call). All of it must go through the switched rpcUrl.
      const ide0 = ideRelay.log.length;
      await idePage.click('button:has-text("Compile project")');
      await idePage.waitForFunction(() => /Compilation complete/.test(document.body.innerText), null, { timeout: 60000 });
      await idePage.click('button:has-text("02Deploy")'); await idePage.click('button:has-text("Public network")');
      await idePage.click('button:has-text("03Interact")'); await idePage.click('button:has-text("Load contract at address")');
      await idePage.fill('input[placeholder="0x…"]', caddr); await idePage.click('button:has-text("Load contract"):not(:has-text("at address"))');
      await idePage.click('button:has-text("READcount")', { timeout: 30000 }); await idePage.click('button:has-text("Read value")');
      const shown = await idePage.waitForFunction(() => { const m = /"values":\s*\[\s*"(\d+)"\s*\]/.exec(document.body.innerText); return m && m[1]; }, null, { timeout: 30000 }).then((h) => h.jsonValue()).catch(() => null);
      const ideLog = ideRelay.log.slice(ide0); const ideMethods = [...new Set(ideLog.map((x) => x.method))];
      ok('IDE: the IDE page reads the devnet through its switched rpcUrl (own relay, Origin https://ide.pocol.net): network check, the loaded contract\'s code and count() = 7 shown',
        shown === '7' && ['eth_chainId', 'pocol_status', 'eth_getCode', 'eth_call'].every((m) => ideMethods.includes(m)) && ideLog.every((x) => x.origin === 'https://ide.pocol.net' && x.status === 200) && /connected/i.test(await idePage.textContent('body')),
        `shown=${shown}; ${ideLog.map((x) => x.method + ':' + x.status + ':' + x.origin).join(' ').slice(0, 240)}`);
      ok('IDE: nothing from the IDE page went to the release rpcUrl (https://explorer.pocol.net) and it raised no uncaught error', ideToRelease.length === 0 && ideErrors.length === 0, [...ideToRelease, ...ideErrors].join(' | '));
      await idePage.close();
    }

    // ---- Lock aborts a running publication ----
    const ldir = join(work, 'project-lock'); writeSiteFiles(ldir, w10Site({ version: 2, padBytes: 10 * 24575, seed: 8 }));
    const pubL = await studioPublish(ldir, true);
    await pubL.waitForFunction(() => /PCOL/.test(document.getElementById('fee').textContent), null, { timeout: 60000 });
    await pubL.fill('#password', 'devnet-password-only'); await pubL.click('#unlock');
    await pubL.waitForFunction(() => /^0x/.test(document.getElementById('walletAddress').textContent), null, { timeout: 60000 });
    const l0 = relays[0].count('eth_sendRawTransaction');
    await approveAndPublish(pubL);
    await wait(() => relays[0].count('eth_sendRawTransaction') >= l0 + 3, 180000, 'the second publication is under way');
    await pubL.click('#lock'); const lockedAt = relays[0].count('eth_sendRawTransaction');
    await pubL.waitForFunction(() => document.getElementById('pause').disabled, null, { timeout: 60000 });
    await sleep(8000);
    const after = relays[0].count('eth_sendRawTransaction'); const lstatus = await pubL.textContent('#status');
    const ljobId = pubL.url().split('#')[1];
    const ljob = Object.values(await pubL.evaluate(() => chrome.storage.local.get(null))).find((j) => j && j.id === ljobId && j.request);
    ok('Lock during a publication stops it: no transaction is signed after the one in flight, the job is paused', after <= lockedAt + 1 && /locked|paused/i.test(lstatus) && ljob && ljob.status === 'paused' && (await pubL.textContent('#walletAddress')).trim() === 'Wallet locked', `${lockedAt} -> ${after}; ${lstatus.slice(0, 160)}; ${ljob && ljob.status}`);
    // Resume: unlock again and approve the remaining transactions; the saved signed transactions are reused.
    await pubL.fill('#password', 'devnet-password-only'); await pubL.click('#unlock');
    await pubL.waitForFunction(() => /^0x/.test(document.getElementById('walletAddress').textContent), null, { timeout: 60000 });
    await approveAndPublish(pubL); await follow(pubL, (p) => p.isVisible('#success'), 900000, 'resume');
    const ljob2 = Object.values(await pubL.evaluate(() => chrome.storage.local.get(null))).find((j) => j && j.id === ljobId && j.request);
    ok('the paused publication resumes after a new unlock and approval and completes', ljob2 && ljob2.status === 'complete', ljob2 && ljob2.status);
    await pubL.close();

    // ---- several viewers with distinct client addresses at once ----
    let viewSiteAddr = site, viewBytes = wrote.bytes;
    if (PROFILE === 'nightly') {
      // A 4 MiB site (published with the CLI: the browser publication is covered above) for the concurrent open.
      const big = join(work, 'big'); const bbase = Object.values(w10Site({ version: 3 })).reduce((a, c) => a + Buffer.byteLength(c), 0);
      viewBytes = writeSiteFiles(big, w10Site({ version: 3, padBytes: SITE_MAX - 4096 - bbase, seed: 9 })).bytes;
      const r = spawnSync(BIN('pocol-site'), ['publish', '--rpc', '127.0.0.1:8540', '--chain-id', cid, '--account', 'alice', '--dir', big], { encoding: 'utf8', timeout: 3600000 });
      ok(`nightly: a ${(viewBytes / 1048576).toFixed(2)} MiB site published for the concurrent viewers`, r.status === 0, (r.stderr || '').slice(-300));
      viewSiteAddr = JSON.parse(r.stdout).site;
    }
    const viewers = [b, await openBrowser(testNetwork, RELAYS[1], 'v2', { headless: true }), await openBrowser(testNetwork, RELAYS[2], 'v3', { headless: true })];
    const inBefore = relays.map((r) => r.count('eth_getCode')), upBefore = nodeTap.count('eth_getCode'), logBefore = relays.map((r) => r.log.length);
    const opened = await Promise.all(viewers.map((x) => viewSite(x, viewSiteAddr, 'devnet-gw')));
    const openLogs = relays.map((r, i) => r.log.slice(logBefore[i])), refused = openLogs.map((l) => l.filter((x) => x.status === 429).length);
    const sentIn = relays.reduce((a, r, i) => a + r.count('eth_getCode') - inBefore[i], 0), upstream = nodeTap.count('eth_getCode') - upBefore;
    ok(`${viewers.length} viewers with distinct client addresses open the ${(viewBytes / 1e6).toFixed(2)} MB site through the gateway at once (${opened.map((o) => (o.openMs / 1000).toFixed(1) + ' s').join(', ')})`,
      opened.every((o) => o.v.h1.startsWith('PoCol W10 site') && o.v.mod && o.v.mod.startsWith('lib>util') && o.trust === 'RN'), JSON.stringify(opened.map((o) => o.v.h1)));
    const slowest = Math.max(...opened.map((o) => o.openMs));
    ok(`the concurrent open stays within the Q8 target: the slowest of ${viewers.length} viewers opened the ${(viewBytes / 1e6).toFixed(2)} MB site in ${(slowest / 1000).toFixed(1)} s (target ${OPEN_TARGET_MS / 1000} s)`, slowest <= OPEN_TARGET_MS, opened.map((o) => o.openMs).join(' ms, ') + ' ms');
    ok(`the concurrent open stays within the gateway's request budget: no viewer was answered HTTP 429 (${openLogs.reduce((a, l) => a + l.length, 0)} requests)`, refused.every((n) => n === 0) && openLogs.every((l) => l.every((x) => x.status !== null)), relays.map((r, i) => `${r.clientIp}: ${openLogs[i].length} requests, ${refused[i]} refused`).join('; '));
    ok(`the gateway's chunk cache answered viewers' reads: ${sentIn} eth_getCode from the viewers, ${upstream} reached the node`, sentIn > 0 && upstream < sentIn, `${sentIn} in, ${upstream} upstream`);
    ok('each viewer reached the gateway under its own client address (relays)', relays.every((r, i) => r.count('eth_getCode') - inBefore[i] > 0), relays.map((r, i) => r.clientIp + ':' + (r.count('eth_getCode') - inBefore[i])).join(' '));
    writeFileSync(join(work, 'viewers.json'), JSON.stringify({ profile: PROFILE, bytes: viewBytes, openMs: opened.map((o) => o.openMs), targetMs: OPEN_TARGET_MS, refused429: refused, sentIn, upstream }, null, 1));
    console.log('MEASURE concurrent open', JSON.stringify({ bytes: viewBytes, openMs: opened.map((o) => o.openMs), sentIn, upstream }));
    for (const o of opened) await o.page.close();

    // ---- a publication in a background tab (measured, nightly) ----
    if (PROFILE === 'nightly' || process.env.E2E_BACKGROUND === '1') {
      const bdir = join(work, 'project-bg'); writeSiteFiles(bdir, w10Site({ version: 4, padBytes: 20 * 24575, seed: 10 }));
      const pubB = await studioPublish(bdir, true);
      await pubB.waitForFunction(() => /PCOL/.test(document.getElementById('fee').textContent), null, { timeout: 60000 });
      await pubB.fill('#password', 'devnet-password-only'); await pubB.click('#unlock');
      await pubB.waitForFunction(() => /^0x/.test(document.getElementById('walletAddress').textContent), null, { timeout: 60000 });
      const s0 = relays[0].log.length; await approveAndPublish(pubB);
      const front = await b.ctx.newPage(); await front.goto('about:blank'); await front.bringToFront();
      // Whether Chrome really treats the publisher tab as hidden: Playwright keeps its pages focused (focus emulation), and
      // under xvfb without a window manager the tab has been seen to stay 'visible'; the record says which it was.
      await pubB.waitForFunction(() => document.visibilityState === 'hidden', null, { timeout: 5000 }).catch(() => {});
      const hidden = await pubB.evaluate(() => document.visibilityState);
      await follow(pubB, (p) => p.isVisible('#success'), 3600000, 'background publish');
      const sends = relays[0].log.slice(s0).filter((x) => x.method === 'eth_sendRawTransaction').map((x) => x.t);
      const gaps = sends.slice(1).map((t, i) => t - sends[i]).sort((a, c) => a - c);
      const m = { headed: HEADED, visibility: hidden, transactions: sends.length, medianGapMs: gaps[gaps.length >> 1], maxGapMs: gaps[gaps.length - 1] };
      writeFileSync(join(work, 'background-tab.json'), JSON.stringify(m, null, 1));
      console.log('MEASURE background tab', JSON.stringify(m));
      ok(`a publication with another tab in front completes; per-transaction gaps recorded (tab ${hidden}: ${JSON.stringify(m)})`, sends.length > 10, JSON.stringify(m));
      await front.close();
    }
  } finally {
    for (const c of contexts) await c.close().catch(() => {});
    for (const p of procs) { try { p.kill('SIGTERM'); } catch {} }
    for (const s of servers) { try { s.close(); } catch {} }
    try { devnet(['stop', work]); } catch {}
  }
}

process.on('unhandledRejection', (e) => ok('no unhandled rejection', false, (e && e.stack) || String(e)));
try { await main(); } catch (e) { ok('e2e driver completed', false, e.stack || e.message); }
const summary = { result: failed() ? 'FAIL' : 'PASS', profile: PROFILE, checks: results.length, failed: failed(), work };
const measured = ['viewers.json', 'background-tab.json'].filter((f) => existsSync(join(work, f))).map((f) => JSON.parse(readFileSync(join(work, f), 'utf8')));
if (failed()) { await sleep(2000); summary.kept = keepOnFailure('e2e_publisher_gateway', work, { results }); }
writeFileSync(join(EXT, 'tests', 'e2e_publisher_gateway.last.json'), JSON.stringify({ ...summary, measured, results }, null, 1) + '\n');
if (!failed() && process.env.E2E_KEEP !== '1') rmSync(work, { recursive: true, force: true });
console.log(JSON.stringify(summary));
process.exit(failed() ? 1 : 0);
