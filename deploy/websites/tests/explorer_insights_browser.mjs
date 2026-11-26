// The explorer's Holders page and Miners map (assets/pocol-insights.js, development/explorer/MINERS-MAP-HOLDERS.md §6)
// in Chromium, offline. The snapshot (deploy/websites/explorer.pocol.net) is served from a loopback server with the
// gateway's Content-Security-Policy (deploy/rewards/explorer_api.py); /api/rpc.php answers from a small fake chain of
// the declared network, and /api/holders.php, /api/holders-stats.php and /api/miner-geography.php answer contract-shaped
// fixtures computed here with BigInt (shares, sums and buckets exact). Every request outside that server is aborted.
//   node deploy/websites/tests/explorer_insights_browser.mjs     (Playwright/Chromium: PLAYWRIGHT_MODULE, POCOL_CHROMIUM)
//   POCOL_INSIGHTS_SHOTS=<dir> also writes screenshots (desktop and phone, English and Arabic).
// 1. Holders: the statistics (8 metrics), the three charts with their table views, the table (rank, address, type,
//    total, available, bonded, % of supply, transactions, blocks mined, mining rewards), server paging (page 2 pinned
//    to the snapshot's height and hash), the order toggle, the type filter, the address search (an invalid prefix is
//    not sent), rows 25/50/100, copy, and an address opening the explorer's own account dialog.
// 2. Holders states: "Newer data available" on a pinned page; a 409 reset goes back to page 1 unpinned; a 429 with
//    Retry-After is retried; an answer of another chain is refused with nothing shown; complete:false shows the
//    completeness notice; one history point shows the empty state naming its block.
// 3. Arabic: the switch sets lang="ar" dir="rtl" on the new views only, keeps Western digits, and is remembered
//    (localStorage pocol-explorer-language) across a reload.
// 4. Miners map: 240 country shapes, a choropleth, markers clustered at the world view and split after zooming into
//    the cluster, a country panel on click, the Miners / Network nodes layers, the country table with Unknown as its own
//    row and percentages of registered miners, the sources, the attribution and the limits; without operator reports
//    the page says every miner is Unknown and opens on the nodes layer. The Overview card links to the page.
// 5. No horizontal page scroll at 360, 768, 1024 and 1440 px; no uncaught error, no CSP violation, no request outside
//    the origin (the font stylesheet is aborted like any other), and the old v1 map endpoint is never asked.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, stat, mkdir } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { dirname, extname, resolve, sep, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const SITE = resolve(ROOT, 'deploy/websites/explorer.pocol.net');
const DECLARED = JSON.parse(readFileSync(resolve(ROOT, 'deploy/websites/site-identity.json'), 'utf8'));
const CSP = /CONTENT_SECURITY_POLICY = \(([\s\S]*?)\)\n/.exec(readFileSync(resolve(ROOT, 'deploy/rewards/explorer_api.py'), 'utf8'))[1]
  .split('\n').map((l) => /"([^"]*)"/.exec(l)[1]).join('');
const SHOTS = process.env.POCOL_INSIGHTS_SHOTS || '';
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.ico': 'image/x-icon', '.webmanifest': 'application/manifest+json', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' };
const WEI = 10n ** 18n, E12 = 10n ** 12n, NOW = Math.floor(Date.now() / 1000), HEAD = 40;
const hex = (n) => '0x' + n.toString(16);
const hash = (tag, n) => '0x' + tag + n.toString(16).padStart(64 - tag.length, '0');
const address = (n) => '0x' + n.toString(16).padStart(40, '0');
let checks = 0;
const check = (name, condition, detail = '') => { assert.ok(condition, name + (detail ? ' — ' + String(detail).slice(0, 600) : '')); checks++; console.log('PASS ' + name); };

// ---- fixtures ----
const SYSTEM = [['0x0000000000000000000000000000000000c0c002', 'RewardEscrow', 5000n * WEI], ['0x0000000000000000000000000000000000c0c001', 'MinerRegistry', 3n * WEI]];
function holderSet(seed = 0) {
  // 4 reward addresses of the founders, 6 miner IDs with a 20 PCOL bond, 2 contracts, wallets, 2 system accounts.
  const rows = [];
  for (let i = 0; i < 4; i++) rows.push({ address: address(0xa000 + i), type: 'reward', label: null, minerIds: [address(0xb000 + i)], rewardAddress: null, available: 999980n * WEI + BigInt(i) * 7n * WEI + BigInt(seed) * WEI, bonded: 0n, transactions: 3 + i, blocksMined: null, rewards: 1234n * WEI + BigInt(i) * 10n ** 15n });
  for (let i = 0; i < 6; i++) rows.push({ address: address(0xb000 + i), type: 'miner', label: null, minerIds: [], rewardAddress: i < 4 ? address(0xa000 + i) : null, available: BigInt(i) * 10n ** 17n, bonded: 20n * WEI, transactions: 1, blocksMined: 600 + i * 11, rewards: 40n * WEI });
  for (let i = 0; i < 2; i++) rows.push({ address: address(0xc000 + i), type: 'contract', label: null, minerIds: [], rewardAddress: null, available: 150n * WEI + BigInt(i), bonded: 0n, transactions: 12, blocksMined: null, rewards: 0n });
  for (let i = 0; i < 126; i++) rows.push({ address: address(0xd000 + i * 3), type: 'wallet', label: null, minerIds: [], rewardAddress: null, available: (BigInt(i % 9) + 1n) * 10n ** BigInt(14 + (i % 10)) + BigInt(i), bonded: 0n, transactions: i % 7, blocksMined: null, rewards: 0n });
  // RewardEscrow's Total is its balance minus the bonds it holds (6 × 20 PCOL here); the other system accounts' is their balance.
  for (const [a, label, wei] of SYSTEM) rows.push({ address: a, type: 'system', label, minerIds: [], rewardAddress: null, available: label === 'RewardEscrow' ? wei + 20n * 6n * WEI : wei, bonded: 0n, total: wei, transactions: 0, blocksMined: null, rewards: 0n });
  for (const r of rows) r.total ??= r.available + r.bonded;
  rows.sort((a, b) => (b.total > a.total ? 1 : b.total < a.total ? -1 : a.address < b.address ? -1 : 1));
  rows.forEach((r, i) => { r.rank = i + 1; });
  return rows;
}
function snapshotOf(height, seed) {
  const rows = holderSet(seed), supply = rows.reduce((a, r) => a + r.total, 0n);
  return { height, hash: hash('5a', height), timestamp: NOW - (HEAD - height) * 2, rows, supply };
}
const share = (part, whole) => ((part * E12) / whole).toString();
function rowJson(r, supply, rewardsKnown) {
  return { rank: r.rank, address: r.address, type: r.type, label: r.label, minerIds: r.minerIds, rewardAddress: r.rewardAddress, totalWei: r.total.toString(),
    availableWei: r.type === 'system' ? r.available.toString() : r.available.toString(), bondedWei: r.bonded.toString(), sharePercentE10: share(r.total, supply),
    transactions: r.transactions, blocksMined: r.blocksMined, rewardsWei: rewardsKnown ? r.rewards.toString() : null };
}
const identity = { chainId: DECLARED.chainId, genesisHash: DECLARED.genesisHash };
const indexOf = (snap) => ({ state: 'ready', head: snap.height, historyHeight: snap.height, updatedAt: NOW });
function holdersPage(state, q) {
  const page = Number(q.get('page') || 1), size = Number(q.get('size') || 25), order = q.get('order') || 'balance_desc', type = q.get('type') || 'all', prefix = q.get('q') || '';
  let snap = state.snapshots.at(-1);
  if (q.has('height')) {
    snap = state.snapshots.find((x) => x.height === Number(q.get('height')) && x.hash === q.get('hash'));
    if (!snap) return { status: 409, body: { error: 'The holder snapshot changed. Refreshing.', reset: true } };
  }
  let rows = snap.rows.filter((r) => (type === 'all' || r.type === type) && r.address.startsWith(prefix));
  if (order === 'balance_asc') rows = [...rows].reverse();
  const out = rows.slice((page - 1) * size, page * size).map((r) => rowJson(r, snap.supply, true));
  const data = { schema: 'pocol-holders-v1', ...identity, page, size, order, type, q: prefix, total: rows.length,
    snapshot: { height: snap.height, hash: snap.hash, timestamp: snap.timestamp }, index: indexOf(snap), supply: { currentSupplyWei: snap.supply.toString() },
    verification: { complete: state.complete }, rows: out };
  if (state.wrongChain) data.chainId = DECLARED.chainId + 1;
  return { status: 200, body: { data } };
}
function statsOf(state) {
  const snap = state.snapshots.at(-1), S = snap.supply, holders = snap.rows.filter((r) => r.type !== 'system');
  const base = 7n * 10n ** 15n, protocol = 2n * 10n ** 16n, genesis = 4000000n * WEI, minted = S + base + protocol;
  const top = (n) => { const t = holders.slice(0, n).reduce((a, r) => a + r.total, 0n); return { totalWei: t.toString(), sharePercentE10: share(t, S) }; };
  const bounds = [0n, 1n, 10n, 100n, 1000n, 10000n, 100000n, 1000000n].map((x) => x * WEI);
  const distribution = bounds.map((lo, i) => {
    const hi = bounds[i + 1] ?? null, inside = holders.filter((r) => r.total >= lo && (hi === null || r.total < hi));
    return { minWei: lo.toString(), maxWei: hi === null ? null : hi.toString(), accounts: inside.length, totalWei: inside.reduce((a, r) => a + r.total, 0n).toString() };
  });
  const points = Array.from({ length: state.historyPoints }, (_, i) => ({ height: 8 + i, timestamp: NOW - (state.historyPoints - i) * 3600, holders: 120 + Math.round(10 * Math.sin(i / 3)) + i,
    currentSupplyWei: (S - BigInt(state.historyPoints - i) * WEI).toString(), top10SharePercentE10: share(holders.slice(0, 10).reduce((a, r) => a + r.total, 0n), S) }));
  const sum = snap.rows.reduce((a, r) => a + r.available, 0n);
  const unlocated = state.complete ? 0n : 3n * WEI;
  return { schema: 'pocol-holders-stats-v1', ...identity, snapshot: { height: snap.height, hash: snap.hash, timestamp: snap.timestamp }, index: indexOf(snap),
    supply: { genesisWei: genesis.toString(), issuedWei: (minted - genesis).toString(), mintedWei: minted.toString(), baseFeeBurnedWei: base.toString(), protocolBurnedWei: protocol.toString(),
      totalBurnedWei: (base + protocol).toString(), currentSupplyWei: S.toString(), circulatingWei: holders.reduce((a, r) => a + r.available, 0n).toString(), availableWei: sum.toString(),
      bondedWei: (120n * WEI).toString(), systemWei: snap.rows.filter((r) => r.type === 'system').reduce((a, r) => a + r.total, 0n).toString(), pendingRewardsWei: (5000n * WEI).toString() },
    verification: { complete: state.complete, balancesSumWei: sum.toString(), expectedSumWei: (sum + unlocated).toString(), unlocatedWei: unlocated.toString(), accountsIndexed: snap.rows.length,
      accountsOnChain: snap.rows.length + (state.complete ? 0 : 2), accountsMissing: state.complete ? 0 : 2 },
    holders: { count: holders.length, withAvailable: holders.filter((r) => r.available > 0n).length, withBonded: 6, new24h: 3, new24hFrom: 1 },
    concentration: { top1: top(1), top10: top(10), top50: top(50), top100: top(100) }, distribution,
    history: { from: points.length ? points[0].height : null, points }, rewards: { from: 5, to: snap.height } };
}
const COUNTRIES = [
  // code, located, active, inactive, connected, nodes, blocks
  ['US', 3, 3, 1, 2, 5, 3000], ['DE', 2, 2, 0, 1, 3, 2000], ['FR', 1, 1, 0, 1, 1, 500], ['JP', 1, 0, 0, 0, 1, 400], ['NL', 0, 0, 0, 0, 2, 0], ['GI', 0, 0, 0, 0, 1, 0],
];
function geography(state) {
  // state.extra: countries a test adds to the fixture (same columns as COUNTRIES), e.g. between two refreshes.
  const window = 8640, rows = (state.reporters ? COUNTRIES : COUNTRIES.map(([c, , , , , n]) => [c, 0, 0, 0, 0, n, 0])).concat(state.extra || []);
  const located = rows.reduce((a, r) => a + r[1], 0), blocks = rows.reduce((a, r) => a + r[6], 0), nodes = rows.reduce((a, r) => a + r[5], 0);
  return { schema: 'pocol-miners-geography-v2', ...identity, generatedAt: NOW, height: window, blockHash: hash('b', window),
    database: { provider: 'DB-IP Lite', release: '2026-11', license: 'CC BY 4.0' },
    miners: { registered: 10, active: 8, inactive: 2, exited: 1, punished: 1, located, unknown: 10 - located, connected: state.reporters ? 4 : 0, thetaInact: 360 },
    nodes: { observed: nodes + 2, located: nodes, unlocated: 2 },
    sources: [{ kind: 'observer', name: 'explorer', fresh: true, reportedAt: NOW - 4 },
      ...(state.reporters ? [{ kind: 'operator', name: 'founder-1', fresh: true, reportedAt: NOW - 20 }, { kind: 'operator', name: 'founder-2', fresh: false, reportedAt: NOW - 900 }] : []),
      { kind: 'operator', name: 'founder-3', fresh: false, reportedAt: null }],
    blocksWindow: { from: 1, to: window, blocks: window },
    countries: rows.map(([code, l, a, i, c, n, b]) => ({ code, miners: { located: l, active: a, inactive: i, connected: c }, nodes: n, blocks: b, blocksSharePercentE10: share(BigInt(b), BigInt(window)), rewardsWei: (BigInt(b) * 2n * WEI).toString() })),
    unknown: { miners: 10 - located, blocks: window - blocks } };
}

// ---- the fake chain the bundle reads (the Overview's refresh and the account dialog) ----
function rpc(call) {
  const ok = (result) => ({ jsonrpc: '2.0', id: call.id, result });
  const block = (n) => ({ number: hex(n), hash: hash('b', n), parentHash: n ? hash('b', n - 1) : '0x' + '00'.repeat(32), timestamp: hex(NOW - 2 * (HEAD - n)), miner: address(9), gasUsed: '0x0', gasLimit: '0x1c9c380', transactions: [] });
  switch (call.method) {
    case 'pocol_status': return ok({ ...identity, head: { height: HEAD, hash: hash('b', HEAD) }, pool: 0, peers: [{}], stats: { reorgs: 0, mined: 1, imported: 39 } });
    case 'pocol_getParams': return ok({ ...identity, protocolVersion: 2, compatibilitySha256: '0x' + '11'.repeat(32) });
    case 'pocol_getMembers': return ok({ height: HEAD, blockHash: hash('b', HEAD), active: 0, start: call.params[0], count: 0, members: [] });
    case 'eth_getBlockByNumber': return ok(Number(call.params[0]) <= HEAD ? block(Number(call.params[0])) : null);
    case 'eth_blockNumber': return ok(hex(HEAD));
    case 'eth_getBalance': return ok('0x0');
    case 'eth_getTransactionCount': return ok('0x0');
    case 'eth_getCode': return ok('0x');
    case 'pocol_getMember': return ok(null);
    default: return { jsonrpc: '2.0', id: call.id, error: { code: -32601, message: 'Method not found' } };
  }
}
async function serve(state) {
  const server = createServer(async (req, res) => {
    const url = new URL(req.url, 'http://localhost'), path = url.pathname;
    const json = (status, body, headers = {}) => { res.writeHead(status, { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store', ...headers }); res.end(JSON.stringify(body)); };
    if (path.startsWith('/api/')) state.asked.push(path + url.search);
    if (path === '/api/rpc.php' && req.method === 'POST') { let body = ''; for await (const c of req) body += c; return json(200, JSON.parse(body).map(rpc)); }
    if (path === '/api/holders.php') {
      if (state.busyHolders > 0) { state.busyHolders--; return json(429, { error: 'The data service is busy. Please retry shortly.' }, { 'retry-after': '1' }); }
      const r = holdersPage(state, url.searchParams); return json(r.status, r.body);
    }
    if (path === '/api/holders-stats.php') return json(200, { data: statsOf(state) });
    if (path === '/api/miner-geography.php') return state.geoDown ? json(503, { error: 'Miner geography is not available yet.' }) : json(200, { data: geography(state) });
    if (path.startsWith('/api/')) return json(503, { error: 'Unavailable in this test' });
    try {
      const file = resolve(SITE, '.' + decodeURIComponent(path));
      if (file !== SITE && !file.startsWith(SITE + sep)) throw Error('path');
      const real = (await stat(file)).isDirectory() ? resolve(file, 'index.html') : file;
      res.writeHead(200, { 'content-type': MIME[extname(real)] || 'application/octet-stream', 'content-security-policy': CSP, 'x-content-type-options': 'nosniff' });
      res.end(await readFile(real));
    } catch { res.writeHead(404); res.end('Not found'); }
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  return { origin: `http://127.0.0.1:${server.address().port}`, close: () => new Promise((r) => server.close(r)) };
}
function newState(extra = {}) {
  return { snapshots: [snapshotOf(HEAD - 2, 0)], complete: true, historyPoints: 30, reporters: true, asked: [], busyHolders: 0, ...extra };
}

const browser = await chromium.launch(process.env.POCOL_CHROMIUM ? { executablePath: process.env.POCOL_CHROMIUM } : {});
const problems = { errors: [], csp: [], outside: [] };
async function open(state, { width = 1440, height = 900, hash: at = '', lang = null, clock = false } = {}) {
  const server = await serve(state);
  const context = await browser.newContext({ viewport: { width, height }, locale: 'en-US' });
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: server.origin });
  await context.route('**/*', (route) => { const u = new URL(route.request().url()); if (u.origin === server.origin) return route.continue(); problems.outside.push(u.href); return route.abort(); });
  if (lang) await context.addInitScript((l) => { try { localStorage.setItem('pocol-explorer-language', l); } catch {} }, lang);
  const page = await context.newPage();
  if (clock) await page.clock.install();   // timers run as usual; clock.fastForward reaches the 30 s map refresh
  page.on('pageerror', (e) => problems.errors.push(e.message));
  page.on('console', (m) => { if (/Content Security Policy|Refused to/.test(m.text())) problems.csp.push(m.text()); });
  await page.goto(server.origin + '/' + at);
  return { page, state, server, close: async () => { await context.close(); await server.close(); } };
}
const until = async (page, fn, ms = 15000, what = 'condition', arg = undefined) => {
  const end = Date.now() + ms; let last;
  while (Date.now() < end) { last = await page.evaluate(fn, arg).catch(() => null); if (last) return last; await new Promise((r) => setTimeout(r, 120)); }
  const shown = await page.evaluate(() => document.querySelector('main')?.innerText.slice(0, 700)).catch(() => '');
  throw Error(`timed out waiting for ${what}: ${JSON.stringify(last)}; errors ${JSON.stringify(problems.errors)}; page: ${shown}`);
};
const focusOf = (page) => page.evaluate(() => { const a = document.activeElement; return a ? (a.getAttribute('data-focus') || a.tagName) : null; });
const rowsOf = (page) => page.evaluate(() => [...document.querySelectorAll('#pocol-holders table.pocol-holders-table tbody tr')].map((r) => r.innerText.replace(/\s+/g, ' ')));
const holdersAsked = (state) => state.asked.filter((a) => a.startsWith('/api/holders.php'));
const scroll = (page) => page.evaluate(() => [document.documentElement.scrollWidth, window.innerWidth]);
async function shot(page, name) {
  if (!SHOTS) return;
  await mkdir(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, name + '.png'), fullPage: true });
}

try {
  // ---- 1. Holders ----
  {
    const { page, state, close } = await open(newState(), { hash: '#page=Holders' });
    await until(page, () => document.querySelectorAll('#pocol-holders table.pocol-holders-table tbody tr').length === 25 && document.querySelectorAll('#pocol-holders .pocol-metrics .metric').length === 8, 20000, 'the holders page');
    const text = await page.evaluate(() => document.querySelector('#pocol-holders').innerText);
    check('the Holders tab is in the explorer navigation and its page has the bundle subtitle', await page.evaluate(() => [...document.querySelectorAll('.sidebar nav button')].some((b) => b.textContent.trim() === 'Holders'))
      && (await page.evaluate(() => document.querySelector('.page-heading p').textContent)).includes('PCOL balances of every discovered account'));
    check('the statistics show the current supply, holders, circulating, top 10 share, bonded, pending, burned and minted', ['Current supply', 'Holders', 'Circulating', 'Top 10 share', 'Bonded', 'Pending rewards (system)', 'Total burned', 'Minted (gross)'].every((t) => text.includes(t)), text.slice(0, 800));
    const snap = state.snapshots[0], holders = snap.rows.filter((r) => r.type !== 'system');
    check('the data line names the snapshot block', text.includes(`Data at block #${snap.height}`), text.slice(0, 400));
    check('the three charts render: the top-10 split, the balance columns and the holders line (30 points)', await page.evaluate(() => !!document.querySelector('.pocol-split .is-top') && !!document.querySelector('.pocol-split .is-rest')
      && document.querySelectorAll('[data-chart="distribution"] .pocol-column').length >= 3 && !!document.querySelector('[data-chart="history"] path.pocol-line')));
    const top10 = holders.slice(0, 10).reduce((a, r) => a + r.total, 0n), pct = (top10 * 10000n) / snap.supply;
    check('the top-10 share is the exact integer ratio of the fixture', text.includes(`${pct / 100n}.${String(pct % 100n).padStart(2, '0')}%`), `${pct}`);
    await page.click('[data-chart="distribution"] .pocol-view-toggle');
    const distTable = await page.evaluate(() => [...document.querySelectorAll('[data-chart="distribution"] tbody tr')].map((r) => r.innerText.replace(/\s+/g, ' ')));
    check('the balance chart has a table view with the eight ranges', distTable.length === 8 && distTable[0].startsWith('0 – 1') && distTable[7].startsWith('≥ 1,000,000'), JSON.stringify(distTable));
    await page.click('[data-chart="distribution"] .pocol-view-toggle');
    const rows = await rowsOf(page);
    check('the table lists rank 1 first with its type, total, share, transactions and rewards', rows[0].startsWith('1 ') && /Reward address/.test(rows[0]) && /1,000,001/.test(rows[0]) && /%/.test(rows[0]), rows[0]);
    check('the columns are rank, address, type, total, available, bonded, % of supply, transactions, blocks mined and mining rewards since the index start',
      JSON.stringify(await page.evaluate(() => [...document.querySelectorAll('#pocol-holders table.pocol-holders-table thead th')].slice(0, 10).map((t) => t.textContent)))
      === JSON.stringify(['Rank', 'Address', 'Type', 'Total', 'Available', 'Bonded', '% of supply', 'Transactions', 'Blocks mined', 'Mining rewards since #5']));
    check('a page footer says rows 1–25 of all holders and page 1 of 6', /1–25 of 140/.test(text) && /Page 1 of 6/.test(text), text.slice(-300));
    await shot(page, 'holders-en-1440');
    // Page 2 is pinned to the snapshot of page 1.
    await page.click('#pocol-holders button[aria-label="Next page"]');
    await until(page, () => /^26 /.test(document.querySelector('#pocol-holders table.pocol-holders-table tbody tr')?.innerText.replace(/\s+/g, ' ') || ''), 10000, 'page 2');
    const second = holdersAsked(state).at(-1);
    check('page 2 asks for page=2 pinned to the snapshot height and hash', second.includes('page=2') && second.includes(`height=${snap.height}`) && second.includes(`hash=${snap.hash}`), second);
    // A newer snapshot: the pinned page stays and offers the newest data.
    state.snapshots.push(snapshotOf(HEAD, 1));
    await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
    await until(page, () => /Newer data available \(block #40\)/.test(document.querySelector('#pocol-holders').innerText), 10000, 'the newer-data notice');
    check('a pinned page keeps its snapshot and offers "Newer data available"', (await rowsOf(page))[0].startsWith('26 '));
    // The pinned snapshot is dropped by the server: 409 reset to page 1, unpinned.
    state.snapshots.shift();
    await page.click('#pocol-holders button[aria-label="Next page"]');
    await until(page, () => /^1 /.test(document.querySelector('#pocol-holders table.pocol-holders-table tbody tr')?.innerText.replace(/\s+/g, ' ') || '') && /Page 1 of/.test(document.querySelector('#pocol-holders').innerText), 10000, 'the reset');
    const afterReset = holdersAsked(state).slice(-2);
    check('a 409 reset reloads page 1 without a pinned snapshot and says the snapshot changed', afterReset[0].includes('page=3') && afterReset[1].includes('page=1') && !afterReset[1].includes('height='), JSON.stringify(afterReset));
    // Order, type, search, size.
    await page.click('#pocol-holders button.pocol-order');
    await until(page, () => /Smallest first/.test(document.querySelector('#pocol-holders .pocol-order').innerText) && /^140 /.test(document.querySelector('#pocol-holders tbody tr')?.innerText.replace(/\s+/g, ' ') || ''), 10000, 'ascending order');
    check('the order toggle asks for balance_asc from page 1; the smallest holder (rank 140) comes first', holdersAsked(state).at(-1).includes('order=balance_asc') && holdersAsked(state).at(-1).includes('page=1'));
    await page.click('#pocol-holders button.pocol-order');
    await page.selectOption('#pocol-holders select[aria-label="Account type"]', 'miner');
    await until(page, () => document.querySelectorAll('#pocol-holders tbody tr').length === 6, 10000, 'the miner filter');
    const miners = await rowsOf(page);
    check('the type filter asks type=miner and shows the six miner IDs with their bond and blocks mined', holdersAsked(state).at(-1).includes('type=miner') && miners.every((r) => /Miner/.test(r) && /20/.test(r)) && /Rewards to 0x000000…00a00/.test(miners.join(' ')), JSON.stringify(miners.slice(0, 2)));
    await page.selectOption('#pocol-holders select[aria-label="Account type"]', 'all');
    await page.fill('#pocol-holders input[type="search"]', '0xzz');
    await until(page, () => /Enter 0x followed by 1 to 40 hexadecimal digits/.test(document.querySelector('#pocol-holders').innerText), 5000, 'the search error');
    check('an invalid address prefix is refused in the page and not sent', !holdersAsked(state).some((a) => a.includes('q=0xzz')));
    await page.fill('#pocol-holders input[type="search"]', '0x000000000000000000000000000000000000A00');
    await until(page, () => document.querySelectorAll('#pocol-holders tbody tr').length === 4, 10000, 'the search');
    check('the search sends the lower-cased prefix and lists its matches', holdersAsked(state).at(-1).includes('q=0x000000000000000000000000000000000000a00'));
    await page.fill('#pocol-holders input[type="search"]', '');
    await until(page, () => document.querySelectorAll('#pocol-holders tbody tr').length === 25, 10000, 'the cleared search');
    await page.selectOption('#pocol-holders .history-footer select', '50');
    await until(page, () => document.querySelectorAll('#pocol-holders tbody tr').length === 50, 10000, '50 rows');
    check('rows per page 50 asks size=50 and shows 50 rows', holdersAsked(state).at(-1).includes('size=50'));
    // Copy, and the account dialog.
    const firstAddress = holders[0].address;
    await page.click('#pocol-holders tbody tr:first-child button.pocol-copy');
    await until(page, () => /Address copied/.test(document.querySelector('.pocol-toast')?.textContent || ''), 5000, 'the copy toast');
    check('copy puts the full address on the clipboard', (await page.evaluate(() => navigator.clipboard.readText())) === firstAddress);
    await page.click('#pocol-holders tbody tr:first-child button.pocol-address');
    const dialog = await until(page, () => { const d = document.querySelector('dialog.detail-dialog'); return d?.open && /Account/.test(d.innerText) && d.innerText; }, 15000, 'the account dialog');
    check('an address opens the explorer\'s own account dialog for it', dialog.toLowerCase().includes(firstAddress), dialog.slice(0, 300));
    await page.click('button[aria-label="Close details"]');
    check('the Holders tab stays selected after the dialog', await page.evaluate(() => !!document.querySelector('#pocol-holders')));
    await close();
  }
  // ---- 2. Holders states ----
  {
    const { page, close } = await open(newState({ busyHolders: 1, complete: false, historyPoints: 1 }), { hash: '#page=Holders' });
    await until(page, () => /busy/.test(document.querySelector('#pocol-holders')?.innerText || '') || document.querySelectorAll('#pocol-holders tbody tr').length === 25, 10000, 'the busy answer');
    await until(page, () => document.querySelectorAll('#pocol-holders tbody tr').length === 25, 10000, 'the retry after Retry-After');
    const text = await page.evaluate(() => document.querySelector('#pocol-holders').innerText);
    check('a 429 with Retry-After is retried and then the page shows its rows', !/busy/.test(text));
    check('complete:false shows the completeness notice with the unlocated PCOL and the missing accounts', /Completeness check: a difference is published/.test(text) && /3 PCOL of the expected balance sum/.test(text) && /2 accounts on chain are not in the index/.test(text)
      && /the known balances are read again every minute, so one may be up to a minute old/.test(text), text.slice(0, 600));
    check('one history point is an empty state naming its block, never a one-point line', /One point so far \(block #8\)/.test(text) && !(await page.evaluate(() => !!document.querySelector('[data-chart="history"] path.pocol-line'))));
    await close();
    const wrong = await open(newState({ wrongChain: true }), { hash: '#page=Holders' });
    await until(wrong.page, () => /failed validation \(chain ID\)/.test(document.querySelector('#pocol-holders')?.innerText || ''), 10000, 'the refused answer');
    check('an answer of another chain is refused and no row of it is shown', (await rowsOf(wrong.page)).length === 0);
    await wrong.close();
  }
  // ---- 3. Arabic ----
  {
    const { page, close } = await open(newState(), { hash: '#page=Holders' });
    await until(page, () => document.querySelectorAll('#pocol-holders tbody tr').length === 25, 20000, 'the holders page');
    await page.click('#pocol-holders .pocol-lang button[data-lang="ar"]');
    const ar = await until(page, () => { const r = document.querySelector('#pocol-holders .pocol-insights'); return r?.getAttribute('dir') === 'rtl' && r.getAttribute('lang') === 'ar' && document.querySelectorAll('#pocol-holders tbody tr').length === 25 && r.innerText; }, 10000, 'Arabic');
    check('Arabic sets lang="ar" dir="rtl" on the new view only; the explorer stays English and left to right', /حائزو PCOL/.test(ar) && await page.evaluate(() => document.documentElement.lang === 'en' && !document.documentElement.dir && document.querySelector('.sidebar').innerText.includes('Holders')));
    check('Arabic keeps Western digits', /الصفحة 1 من 6/.test(ar) && !/[٠-٩]/.test(ar), ar.slice(-200));
    check('the choice is stored under pocol-explorer-language', (await page.evaluate(() => localStorage.getItem('pocol-explorer-language'))) === 'ar');
    await shot(page, 'holders-ar-1440');
    await page.reload();
    await until(page, () => document.querySelector('#pocol-holders .pocol-insights')?.getAttribute('dir') === 'rtl', 15000, 'Arabic after a reload');
    check('the language survives a reload', true);
    await close();
  }
  // ---- 3b. Keyboard focus through refreshes and keyboard actions (F1) ----
  {
    const { page, state, close } = await open(newState(), { hash: '#page=Holders' });
    await until(page, () => document.querySelectorAll('#pocol-holders tbody tr').length === 25, 20000, 'the holders page');
    const first = state.snapshots[0].rows[0].address;
    await page.focus('#pocol-holders tbody tr:first-child button.pocol-copy');
    const asked = holdersAsked(state).length;
    await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));   // an auto-refresh, same snapshot
    await new Promise((r) => setTimeout(r, 800));
    check('an auto-refresh with the same snapshot keeps the focus on the copy button', (await focusOf(page)) === 'copy:' + first && holdersAsked(state).length > asked, await focusOf(page));
    state.snapshots.push(snapshotOf(HEAD - 1, 0));                                     // a newer snapshot: page 1 is rebuilt
    await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
    await until(page, (h) => document.querySelector('#pocol-holders .pocol-data-at')?.textContent.includes('#' + h), 10000, 'the newer snapshot', HEAD - 1);
    await new Promise((r) => setTimeout(r, 500));
    check('an auto-refresh that rebuilds the table gives the focus back to the same copy button', (await focusOf(page)) === 'copy:' + first, await focusOf(page));
    await page.focus('#pocol-holders button[aria-label="Next page"]');
    await page.keyboard.press('Enter');
    await until(page, () => /^26 /.test(document.querySelector('#pocol-holders tbody tr')?.innerText.replace(/\s+/g, ' ') || ''), 10000, 'page 2 by keyboard');
    check('Enter on Next page keeps the focus on Next page', (await focusOf(page)) === 'page:next', await focusOf(page));
    await page.focus('#pocol-holders button[aria-label="Last page"]');
    await page.keyboard.press('Enter');
    await until(page, () => /Page 6 of 6/.test(document.querySelector('#pocol-holders').innerText), 10000, 'the last page by keyboard');
    check('on the last page the focus moves from the disabled Last page to the nearest enabled pager button', (await focusOf(page)) === 'page:prev', await focusOf(page));
    await page.focus('[data-chart="distribution"] .pocol-view-toggle');
    await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
    await new Promise((r) => setTimeout(r, 800));
    check('a statistics refresh keeps the focus on a chart toggle', (await focusOf(page)) === 'toggle:distribution', await focusOf(page));
    await close();
  }
  {
    const { page, state, close } = await open(newState(), { hash: '#page=Miner%20map', clock: true });
    await until(page, () => document.querySelectorAll('#pocol-miners-map .pocol-marker').length > 0, 20000, 'the map');
    const jp = await page.evaluate(() => document.querySelector('#pocol-miners-map .pocol-marker[data-codes="JP"] .pocol-marker-core').getAttribute('class'));
    check('a country whose located miner is not active is not drawn in the active blue (F2)', jp === 'pocol-marker-core is-other', jp);
    const us = await page.evaluate(() => document.querySelector('#pocol-miners-map .pocol-marker[data-codes="US"] .pocol-marker-core').getAttribute('class'));
    check('a country with active, sharing miners keeps the blue core', us === 'pocol-marker-core', us);
    await page.focus('#pocol-miners-map .pocol-marker[data-codes="JP"]');
    await page.keyboard.press('Enter');
    await until(page, () => document.querySelector('#pocol-miners-map .pocol-country-panel h3')?.textContent === 'Japan', 5000, 'the Japan panel');
    check('Enter on a marker selects its country and keeps the focus on that marker', (await focusOf(page)) === 'marker:JP', await focusOf(page));
    check('the country table highlights the selected country at once (F3)', await page.evaluate(() => document.querySelector('#pocol-miners-map tr[data-country="JP"]').classList.contains('is-selected')
      && document.querySelectorAll('#pocol-miners-map .pocol-country-table tr.is-selected').length === 1));
    await page.click('#pocol-miners-map .pocol-country-table button[aria-label="Show United States of America on the map"]');
    check('selecting another country in the table moves the highlight', await page.evaluate(() => document.querySelector('#pocol-miners-map tr[data-country="US"]').classList.contains('is-selected')
      && !document.querySelector('#pocol-miners-map tr[data-country="JP"]').classList.contains('is-selected')));
    await page.click('#pocol-miners-map .map-zoom button[aria-label="Show the whole world"]');
    await page.focus('#pocol-miners-map .pocol-marker.is-cluster');
    const cluster = (await focusOf(page)).slice(7).split(' ');
    await page.keyboard.press('Enter');
    await until(page, () => [...document.querySelectorAll('#pocol-miners-map .pocol-marker')].some((m) => m.getAttribute('data-codes') === 'DE'), 5000, 'the split cluster');
    const after = await focusOf(page);
    check('Enter on a cluster zooms in and the focus stays on a marker of its countries', after.startsWith('marker:') && after.slice(7).split(' ').every((c) => cluster.includes(c)), `${cluster} → ${after}`);
    await page.focus('#pocol-miners-map [data-zoom="in"]');
    const geographyAsked = () => state.asked.filter((a) => a.includes('miner-geography')).length, loads = geographyAsked();
    await page.clock.fastForward(31000);
    for (let i = 0; i < 80 && geographyAsked() <= loads; i++) await new Promise((r) => setTimeout(r, 120));
    check('the map refreshes after 30 s', geographyAsked() > loads, `${loads} → ${geographyAsked()}`);
    await new Promise((r) => setTimeout(r, 500));
    check('the 30 s map refresh keeps the focus on the zoom button', (await focusOf(page)) === 'zoom:in', await focusOf(page));
    await close();
    const ov = await open(newState());
    const legend = await until(ov.page, () => document.querySelector('#pocol-miners-map-compact .map-legend')?.innerText, 20000, 'the Overview legend');
    check('the Overview legend names the "Located, not active" key when a country has one', /Located, not active/.test(legend), legend);
    await ov.close();
  }
  {
    // A refresh that splits the focused cluster keeps the focus on the selected country, and Clear by keyboard keeps it
    // on the map.
    const { page, state, close } = await open(newState(), { hash: '#page=Miner%20map', clock: true });
    await until(page, () => document.querySelectorAll('#pocol-miners-map .pocol-marker').length > 0, 20000, 'the map');
    await page.click('#pocol-miners-map .pocol-country-table button[aria-label="Show France on the map"]');
    await page.click('#pocol-miners-map .map-zoom button[aria-label="Show the whole world"]');
    await until(page, () => document.querySelector('#pocol-miners-map .pocol-marker.is-selected')?.getAttribute('data-codes') === 'DE FR', 5000, 'France selected in the Germany and France cluster');
    await page.focus('#pocol-miners-map .pocol-marker[data-codes="DE FR"]');
    // Poland now has the most located miners: it seeds a cluster with Germany and France is left on its own.
    state.extra = [['PL', 3, 2, 0, 0, 1, 100]];
    const geographyAsked = () => state.asked.filter((a) => a.includes('miner-geography')).length, loads = geographyAsked();
    await page.clock.fastForward(31000);
    const split = await until(page, () => { const c = [...document.querySelectorAll('#pocol-miners-map .pocol-marker')].map((m) => m.getAttribute('data-codes')); return c.includes('FR') && c.some((x) => x.split(' ').includes('PL') && x.split(' ').includes('DE')) && c; }, 10000, 'the split cluster after the refresh');
    check('the 30 s refresh split the focused cluster: Poland with Germany, France alone', geographyAsked() > loads && Array.isArray(split), JSON.stringify(split));
    await new Promise((r) => setTimeout(r, 500));
    check('a refresh that splits the focused cluster keeps the focus on the selected country (France), not the first country of the old cluster', (await focusOf(page)) === 'marker:FR', await focusOf(page));
    await page.focus('#pocol-miners-map [data-focus="panel:clear"]');
    await page.keyboard.press('Enter');
    await until(page, () => !document.querySelector('#pocol-miners-map .pocol-country-panel h3'), 5000, 'the cleared panel');
    check('Enter on the panel\'s Clear clears the selection and keeps the focus on the cleared country\'s marker, not the page', (await focusOf(page)) === 'marker:FR'
      && !(await page.evaluate(() => document.querySelector('#pocol-miners-map .pocol-marker.is-selected'))), await focusOf(page));
    await close();
  }
  // ---- 4. Miners map ----
  {
    const { page, state, close } = await open(newState(), { hash: '#page=Miner%20map' });
    await until(page, () => document.querySelectorAll('#pocol-miners-map path.map-country').length === 240 && document.querySelectorAll('#pocol-miners-map .pocol-marker').length > 0, 20000, 'the map');
    const text = await page.evaluate(() => document.querySelector('#pocol-miners-map').innerText);
    check('the map page has the bundle subtitle, not the old notice', (await page.evaluate(() => document.querySelector('.page-heading p').textContent)) === 'Registered miners by country where an operator node reports it; every other miner is Unknown.'
      && !/Miner locations are unavailable/.test(await page.evaluate(() => document.body.innerText)));
    check('the statistics: 10 registered, 7 located (3 Unknown), 4 connected, network nodes', /Registered miners\s+10/.test(text) && /Located miners\s+7/.test(text) && /3 Unknown/.test(text) && /Connected miners\s+4/.test(text) && /Network nodes\s+15/.test(text), text.slice(0, 600));
    check('the choropleth colours the countries with located miners', await page.evaluate(() => ['US', 'DE', 'FR', 'JP'].every((c) => document.querySelector(`#pocol-miners-map path[data-country="${c}"]`).classList.contains('has-miners'))
      && !document.querySelector('#pocol-miners-map path[data-country="NL"]').classList.contains('has-miners')));
    const world = await page.evaluate(() => [...document.querySelectorAll('#pocol-miners-map .pocol-marker')].map((m) => m.getAttribute('data-codes')));
    check('at the world view Germany and France are one cluster with their count', world.some((c) => c.split(' ').includes('DE') && c.split(' ').includes('FR')) && world.includes('US'), JSON.stringify(world));
    check('the US marker has active and inactive ring segments', await page.evaluate(() => { const m = document.querySelector('#pocol-miners-map .pocol-marker[data-codes="US"]'); return !!m.querySelector('.pocol-ring.is-active') && !!m.querySelector('.pocol-ring.is-inactive') && /2 active and sharing, 1 inactive/.test(m.getAttribute('aria-label')); }));
    await page.focus('#pocol-miners-map .pocol-marker.is-cluster');
    await page.keyboard.press('Enter');
    const zoomed = await until(page, () => { const c = [...document.querySelectorAll('#pocol-miners-map .pocol-marker')].map((m) => m.getAttribute('data-codes')); return c.includes('DE') && c.includes('FR') && c; }, 5000, 'the split cluster');
    check('selecting the cluster zooms in and the clusters are recomputed: Germany and France apart', Array.isArray(zoomed), JSON.stringify(zoomed));
    const before = await page.evaluate(() => document.querySelector('#pocol-miners-map svg.world-map-svg').getAttribute('viewBox'));
    await page.click('#pocol-miners-map .map-zoom button[aria-label="Show the whole world"]');
    await page.focus('#pocol-miners-map svg.world-map-svg'); await page.keyboard.press('+');
    const after = await page.evaluate(() => document.querySelector('#pocol-miners-map svg.world-map-svg').getAttribute('viewBox'));
    check('the reset button and the + key change the view', before !== after && after !== '0.00 0.00 1000.00 500.00' && after.split(' ')[2] === '625.00', `${before} → ${after}`);
    await page.click('#pocol-miners-map .map-zoom button[aria-label="Show the whole world"]');
    await page.locator('#pocol-miners-map path[data-country="BR"]').click();
    const panel = await until(page, () => document.querySelector('#pocol-miners-map .pocol-country-panel h3')?.textContent, 5000, 'the country panel');
    check('a click on a country without miners opens its panel saying so', panel === 'Brazil' && /No miner is located in Brazil/.test(await page.evaluate(() => document.querySelector('#pocol-miners-map .pocol-country-panel').innerText)));
    await page.click('#pocol-miners-map .pocol-country-table button[aria-label="Show United States of America on the map"]');
    const us = await page.evaluate(() => document.querySelector('#pocol-miners-map .pocol-country-panel').innerText);
    check('the table selects a country: its panel gives located, share of registered, active, inactive, connected, nodes, blocks and rewards',
      /United States of America/.test(us) && /Share of registered miners\s+30.00%/.test(us) && /Active, sharing\s+2/.test(us) && /Inactive\s+1/.test(us) && /Network nodes\s+5/.test(us) && /Blocks mined \(last 8,640 blocks\)\s+3,000/.test(us), us);
    const table = await page.evaluate(() => [...document.querySelectorAll('#pocol-miners-map .pocol-country-table tbody tr')].map((r) => r.innerText.replace(/\s+/g, ' ')));
    check('the country table has every country and Unknown as its own row, with % of registered miners', table.length === COUNTRIES.length + 1 && /^US United States of America 3 30.00%/.test(table[0]) && /^Unknown \(no operator report\) 3 30.00%/.test(table.at(-1)), JSON.stringify(table));
    check('the sources list the observer and each operator with freshness; the database and window are named', /Observer node of this explorer/.test(text) && /founder-1/.test(text) && /Not fresh/.test(text) && /No report yet/.test(text) && /DB-IP Lite 2026-11 · CC BY 4.0/.test(text), text.slice(-900));
    check('the attribution and the stated limits are shown', /Map: Natural Earth · IP Geolocation by DB-IP/.test(text.replace(/\s+/g, ' ')) && /known only for miners whose node runs an operator reporter/.test(text) && /VPN exits, NAT and hosting providers/.test(text), text.slice(text.indexOf('Active, sharing'), text.indexOf('Active, sharing') + 700));
    await page.click('#pocol-miners-map .pocol-segmented button[data-layer="nodes"]');
    const nodes = await page.evaluate(() => [...document.querySelectorAll('#pocol-miners-map .pocol-marker')].map((m) => m.getAttribute('data-codes')).join(' '));
    check('the Network nodes layer places nodes (the Netherlands too) and leaves out a country without a shape', nodes.includes('NL') && !nodes.includes('GI'), nodes);
    check('the old v1 map endpoint is never asked', !state.asked.some((a) => a.includes('miner-locations')));
    await page.click('#pocol-miners-map .pocol-segmented button[data-layer="miners"]');
    await shot(page, 'map-en-1440');
    await page.click('#pocol-miners-map .pocol-lang button[data-lang="ar"]');
    await until(page, () => document.querySelector('#pocol-miners-map .pocol-insights')?.getAttribute('dir') === 'rtl' && document.querySelectorAll('#pocol-miners-map .pocol-marker').length > 0, 10000, 'the Arabic map');
    check('the Arabic map names countries in Arabic', /الولايات المتحدة/.test(await page.evaluate(() => document.querySelector('#pocol-miners-map').innerText)));
    await shot(page, 'map-ar-1440');
    await page.click('#pocol-miners-map .pocol-lang button[data-lang="en"]');
    await close();
    // Without operator reports.
    const none = await open(newState({ reporters: false }), { hash: '#page=Miner%20map' });
    const t2 = await until(none.page, () => document.querySelectorAll('#pocol-miners-map .pocol-marker').length > 0 && document.querySelector('#pocol-miners-map').innerText, 20000, 'the map without reporters');
    check('without operator reports every miner is Unknown, the page says so and opens on the nodes layer',
      /No operator node reports a miner yet, so every registered miner is Unknown/.test(t2) && (await none.page.evaluate(() => document.querySelector('#pocol-miners-map .pocol-segmented [aria-pressed="true"]').dataset.layer)) === 'nodes', t2.slice(0, 500));
    await none.close();
    // The Overview card.
    const ov = await open(newState());
    const card = await until(ov.page, () => document.querySelector('#pocol-miners-map-compact .pocol-compact')?.innerText.includes('10 registered miners') && document.querySelector('#pocol-miners-map-compact').innerText, 20000, 'the Overview card');
    check('the Overview card summarises the map', /7 located · 3 Unknown/.test(card), card.slice(0, 300));
    await shot(ov.page, 'overview-en-1440');
    await ov.page.click('#pocol-miners-map-compact button.secondary');
    await until(ov.page, () => document.querySelector('h1')?.textContent === 'Miner map' && !!document.querySelector('#pocol-miners-map'), 10000, 'the map page');
    check('the card opens the Miner map tab', true);
    await ov.close();
    // The geography service down.
    const down = await open(newState({ geoDown: true }), { hash: '#page=Miner%20map' });
    const t3 = await until(down.page, () => /Miner geography is unavailable/.test(document.querySelector('#pocol-miners-map')?.innerText || '') && document.querySelector('#pocol-miners-map').innerText, 15000, 'the unavailable map');
    check('a 503 says the geography is unavailable with the server\'s message', /Miner geography is not available yet\./.test(t3));
    await down.close();
  }
  // ---- 5. Widths ----
  for (const [width, lang] of [[360, 'en'], [360, 'ar'], [768, 'en'], [1024, 'en'], [1024, 'ar'], [1440, 'en']]) {
    for (const [tab, id] of [['Holders', 'pocol-holders'], ['Miner%20map', 'pocol-miners-map']]) {
      const { page, close } = await open(newState(), { width, height: 860, hash: '#page=' + tab, lang });
      await until(page, () => (document.querySelectorAll('#pocol-holders tbody tr').length === 25 || document.querySelectorAll('#pocol-miners-map .pocol-marker').length > 0), 20000, `${tab} at ${width}`);
      await new Promise((r) => setTimeout(r, 300));
      const [sw, iw] = await scroll(page);
      const wide = sw <= iw + 1 ? [] : await page.evaluate((w) => [...document.querySelectorAll('body *')].filter((e) => e.getBoundingClientRect().right > w + 1 && !e.parentElement.closest('.table-wrap'))
        .slice(0, 6).map((e) => `${e.tagName}.${e.getAttribute('class')} ${Math.round(e.getBoundingClientRect().right)}`), iw);
      check(`${tab.replace('%20', ' ')} at ${width} px (${lang}) has no horizontal page scroll`, sw <= iw + 1, `${sw} > ${iw}: ${wide.join(' | ')}`);
      if (width !== 1440) await shot(page, `${id.replace('pocol-', '')}-${lang}-${width}`);
      await close();
    }
  }
  check('no uncaught error in any page', problems.errors.length === 0, problems.errors.join(' | '));
  check('no Content-Security-Policy violation', problems.csp.length === 0, problems.csp.join(' | '));
  const unexpected = problems.outside.filter((u) => !/^https:\/\/fonts\.(googleapis|gstatic)\.com\//.test(u));
  check(`no request left the origin except the font stylesheet (${problems.outside.length} aborted)`, unexpected.length === 0, unexpected.join(' '));
} finally { await browser.close(); }
console.log(`explorer insights browser checks: ${checks} passed`);
