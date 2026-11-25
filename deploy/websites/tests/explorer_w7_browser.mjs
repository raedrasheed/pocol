// Sites W7: the explorer's Overview against a fake relay, offline. The snapshot (deploy/websites/explorer.pocol.net) is
// served from a loopback server whose /api/rpc.php answers from a fake chain of the declared network (site-identity.json):
// 40 light blocks, 300 active members (so one pocol_getMembers page of 256 is not the whole set), and transactions in
// the newest blocks. Every request outside that server is aborted. No node, gateway, wallet or key is used.
//   node deploy/websites/tests/explorer_w7_browser.mjs     (needs Playwright/Chromium: PLAYWRIGHT_MODULE, POCOL_CHROMIUM)
// 1. The gateway refuses the refresh's third request, the eth_getTransactionByHash batch, with its busy answer (429)
//    after the status and block reads succeed: the Latest transactions panel says the read failed and gives the
//    gateway's message, not "No transactions in this window". A refusal whose message has no closing period
//    (a 503 without an error field: "Data source unavailable") still reads as two sentences.
// 2. The same read answered: the panel lists the transactions. No transactions in the window: the empty state.
// 4. An open account view follows the loaded window (accountFollowsWindow).
// 3. The Overview never presents the loaded members page as registered miners: since the Miners map and Holders edits
//    (tools/explorer-insights-edits.json) the old one-node map and its "members on the loaded page" summary are no
//    longer rendered; the Overview's map is the insights module's card (div#pocol-miners-map-compact), whose numbers
//    come only from /api/miner-geography.php (unavailable in these runs). The one-peer v1 answer these runs still
//    serve at /api/miner-locations.php is never asked for.
// 5. Production readiness (tools/explorer-prod-edits.json): a refused read is retried twice before it is reported (the
//    refused transaction read of 1 is sent three times), and the transaction view of a registerReplacing call whose
//    receipt read the node first answers busy (-32021, as its heavy lane does under load) still shows the decoded
//    call and the named member: explorer_smoke.mjs's registerReplacing check on a v3 devnet failed that way.
// 6. The Transactions tab with rows at 390 px has no horizontal page scroll: the wide history table scrolls inside its
//    own box (its screen-reader "Details" label, absolutely positioned, made the page 727 px wide before the fix).
// 7. The Miner map without geography data (/api/miner-geography.php 503): on its tab and on the Overview the insights
//    module says miner geography is unavailable with the gateway's message, and shows no map, count or attribution
//    beside it; the old one-node notice is gone (MINERS-MAP-HOLDERS.md §6). The page has no horizontal scroll at 390 px.
//    The views with data are explorer_insights_browser.mjs.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { dirname, extname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const SITE = resolve(ROOT, 'deploy/websites/explorer.pocol.net');
const DECLARED = JSON.parse(readFileSync(resolve(ROOT, 'deploy/websites/site-identity.json'), 'utf8'));
const BUSY = 'The data service is busy. Please retry shortly.'; // deploy/rewards/explorer_api.py BUSY
const NODE_BUSY = { code: -32021, message: 'busy', data: { reason: 'busy', retryAfterMs: 250 } }; // pocold rpc.rs busy()
const REGISTRY = '0x0000000000000000000000000000000000c0c001', VICTIM = '0x' + '86'.repeat(20);
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.ico': 'image/x-icon', '.webmanifest': 'application/manifest+json' };
const HEAD = 40, ACTIVE = 300, NOW = Math.floor(Date.now() / 1000);
const hex = (n) => '0x' + n.toString(16);
const hash = (tag, n) => '0x' + tag + n.toString(16).padStart(64 - tag.length, '0');
const address = (n) => '0x' + n.toString(16).padStart(40, '0');
let checks = 0;
const check = (name, condition, detail = '') => { assert.ok(condition, name + (detail ? ' — ' + String(detail).slice(0, 400) : '')); checks++; console.log('PASS ' + name); };

// The fake chain: block n has hash b..n and timestamp NOW-2*(HEAD-n); `txBlocks` maps a block number to its tx count.
function chain(txBlocks) {
  const txs = new Map();
  const block = (n) => {
    const hashes = Array.from({ length: txBlocks[n] || 0 }, (_, i) => hash('c' + i, n));
    for (const h of hashes) txs.set(h, { hash: h, blockNumber: hex(n), blockHash: hash('b', n), from: address(1), to: address(2), value: '0x0', nonce: '0x0', gas: '0x5208', gasPrice: '0x3b9aca00', input: '0x' });
    return { number: hex(n), hash: hash('b', n), parentHash: n ? hash('b', n - 1) : '0x' + '00'.repeat(32), timestamp: hex(NOW - 2 * (HEAD - n)), miner: address(9), gasUsed: '0x0', gasLimit: '0x1c9c380', transactions: hashes };
  };
  for (let n = 0; n <= HEAD; n++) block(n);
  return { block, txs };
}

function answer(state, call) {
  const { method, params } = call;
  const ok = (result) => ({ jsonrpc: '2.0', id: call.id, result });
  switch (method) {
    case 'pocol_status': return ok({ chainId: DECLARED.chainId, genesisHash: DECLARED.genesisHash, head: { height: state.head, hash: hash('b', state.head) }, pool: 0, peers: [{}, {}], stats: { reorgs: 0, mined: 3, imported: 37 } });
    case 'pocol_getParams': return ok({ chainId: DECLARED.chainId, genesisHash: DECLARED.genesisHash, protocolVersion: 2, compatibilitySha256: '0x' + '11'.repeat(32) });
    case 'pocol_getMembers': {
      const [start, count] = params, n = Math.max(0, Math.min(count, ACTIVE - start));
      const members = Array.from({ length: n }, (_, i) => ({ id: address(0x1000 + start + i), status: 1, tomb: false, bondState: 1, lastShare: HEAD - 1, registeredAt: 1 }));
      return ok({ height: state.head, blockHash: hash('b', state.head), active: ACTIVE, start, count: n, members });
    }
    case 'eth_getBlockByNumber': {
      if (params[1] === true) state.full.push(Number(params[0]));
      const b = Number(params[0]) <= state.head ? state.chain.block(Number(params[0])) : null;
      // A full block carries its transaction objects (what the account activity reads); a light one only the hashes.
      return ok(b && params[1] === true ? { ...b, transactions: b.transactions.map((h) => state.chain.txs.get(h)) } : b);
    }
    case 'eth_getTransactionByHash': return ok(state.chain.txs.get(params[0]) || null);
    case 'eth_blockNumber': return ok(hex(state.head));
    // The account view's batch (accountFollowsWindow): a plain account that is not a member.
    case 'eth_getBalance': return ok('0x0');
    case 'eth_getTransactionCount': return ok('0x0');
    case 'eth_getCode': return ok('0x');
    case 'pocol_getMember': return ok(params[0] === VICTIM ? { address: VICTIM, exists: true, status: 1, tomb: false, bondState: 1, bondWei: '20000000000000000000', bondTargetWei: '20000000000000000000', proposerEligible: true, rewardAddress: address(6), removalLabel: 'SET_NOT_FULL', registeredAt: 0, lastShare: HEAD - 1, height: state.head } : null);
    case 'eth_getTransactionReceipt': return ok(state.chain.txs.has(params[0]) ? { transactionHash: params[0], status: '0x0', gasUsed: '0x7530', effectiveGasPrice: '0x3b9aca00', logs: [], blockNumber: state.chain.txs.get(params[0]).blockNumber, blockHash: state.chain.txs.get(params[0]).blockHash } : null);
    default: return { jsonrpc: '2.0', id: call.id, error: { code: -32601, message: 'Method not found' } };
  }
}

// One server per scenario: the snapshot's files, /api/rpc.php from the fake chain (a batch of transaction reads refused
// with the gateway's busy answer while `state.busyTxRead`), the optional /api modules unavailable as without them.
async function serve(state) {
  const server = createServer(async (req, res) => {
    const path = new URL(req.url, 'http://localhost').pathname;
    if (path === '/api/rpc.php' && req.method === 'POST') {
      let body = ''; for await (const c of req) body += c;
      const calls = JSON.parse(body); state.batches.push(calls.map((c) => c.method));
      // accountFollowsWindow(.., true): once set, the account activity's full block reads are refused (503, no message).
      if (state.failFull && calls.some((c) => c.method === 'eth_getBlockByNumber' && c.params?.[1] === true)) {
        state.full.push(...calls.map((c) => Number(c.params[0])));
        res.writeHead(503, { 'content-type': 'application/json; charset=utf-8' }); return res.end('{}');
      }
      // txView: the node answers the receipt read busy (relayed by the gateway as that member's error) `busyReceipt` times.
      if (state.busyReceipt > 0 && calls.some((c) => c.method === 'eth_getTransactionReceipt')) {
        state.busyReceipt--; state.receiptBusy = (state.receiptBusy || 0) + 1;
        res.writeHead(200, { 'content-type': 'application/json; charset=utf-8' });
        return res.end(JSON.stringify(calls.map((c) => (c.method === 'eth_getTransactionReceipt' ? { jsonrpc: '2.0', id: c.id, error: NODE_BUSY } : answer(state, c)))));
      }
      if (state.busyTxRead && calls.every((c) => c.method === 'eth_getTransactionByHash')) {
        // 'down': a 503 without an error field, which the bundle reports as "Data source unavailable" (no period).
        if (state.busyTxRead === 'down') { res.writeHead(503, { 'content-type': 'application/json; charset=utf-8' }); return res.end('{}'); }
        res.writeHead(429, { 'content-type': 'application/json; charset=utf-8', 'retry-after': '3' }); return res.end(JSON.stringify({ error: BUSY }));
      }
      res.writeHead(200, { 'content-type': 'application/json; charset=utf-8' }); return res.end(JSON.stringify(calls.map((c) => answer(state, c))));
    }
    if (path === '/api/miner-locations.php' && state.geography) {
      state.v1Asked = true;
      res.writeHead(200, { 'content-type': 'application/json; charset=utf-8' }); return res.end(JSON.stringify({ data: state.geography }));
    }
    if (path === '/api/transactions.php' && state.txPage) {
      const q = new URL(req.url, 'http://localhost').searchParams;
      res.writeHead(200, { 'content-type': 'application/json; charset=utf-8' });
      return res.end(JSON.stringify({ data: state.txPage(q) }));
    }
    if (path.startsWith('/api/')) { res.writeHead(503, { 'content-type': 'application/json' }); return res.end(JSON.stringify({ error: 'Unavailable in this test' })); }
    try {
      const file = resolve(SITE, '.' + decodeURIComponent(path));
      if (file !== SITE && !file.startsWith(SITE + sep)) throw Error('path');
      const real = (await stat(file)).isDirectory() ? resolve(file, 'index.html') : file;
      res.writeHead(200, { 'content-type': MIME[extname(real)] || 'application/octet-stream' }); res.end(await readFile(real));
    } catch { res.writeHead(404); res.end('Not found'); }
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  return { origin: `http://127.0.0.1:${server.address().port}`, close: () => new Promise((r) => server.close(r)) };
}

const browser = await chromium.launch({ executablePath: process.env.POCOL_CHROMIUM || '/usr/bin/chromium', headless: true });
// A geography answer of the declared network (pocol-network-geography-v1: the observer node alone, one connection).
const GEOGRAPHY = { schema: 'pocol-network-geography-v1', chainId: DECLARED.chainId, genesisHash: DECLARED.genesisHash, generatedAt: NOW, source: 'observed-peer-ip',
  scope: 'one-node', peerConnections: 1, excludedConnections: 0, database: { provider: 'DB-IP Lite', release: new Date(NOW * 1000).toISOString().slice(0, 7), updatedAt: NOW },
  nodes: [{ id: 'ip-' + 'a'.repeat(24), country: 'DE', connections: 1, observer: true, miner: null }] };
async function overview(txBlocks, busyTxRead) {
  const state = { chain: chain(txBlocks), busyTxRead, batches: [], head: HEAD, full: [], geography: GEOGRAPHY };
  const server = await serve(state), context = await browser.newContext();
  try {
    await context.route('**/*', (route) => (new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort()));
    const page = await context.newPage(), errors = []; page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(server.origin + '/');
    const latest = page.locator('section.panel', { has: page.locator('h2', { hasText: 'Latest transactions' }) });
    // The first refresh is done once its last request (the transaction read, when the window has transactions) is
    // answered and the page shows the head; then one render more.
    // A refused transaction read is sent three times (two retries, each after the 429's Retry-After: 3 s) before it
    // is reported.
    const requests = Object.keys(txBlocks).length ? (busyTxRead ? 5 : 3) : 2, deadline = Date.now() + 30000; let text = '';
    while (Date.now() < deadline) {
      text = await page.evaluate(() => document.body.innerText);
      if (/Latest block\s+40\b/.test(text) && state.batches.length >= requests && !/Miner registry loading/.test(text)) break;
      await new Promise((r) => setTimeout(r, 200));
    }
    await new Promise((r) => setTimeout(r, 500)); text = await page.evaluate(() => document.body.innerText);
    const panel = (await latest.count()) ? await latest.first().innerText() : '';
    const compact = await page.evaluate(() => !!document.querySelector('#pocol-miners-map-compact'));
    return { text, panel, compact, errors, batches: state.batches, state };
  } finally { await context.close(); await server.close(); }
}

// 4. An account view left open follows the loaded window: when the head moves on, a new transaction of the account
//    appears, one that left the latest 20 blocks goes, and only the new block is read in full (the others come from
//    the activity's cache, keyed by block hash).
async function accountFollowsWindow(txBlocks, failAfterMove = false) {
  const state = { chain: chain(txBlocks), busyTxRead: false, batches: [], head: HEAD, full: [] };
  const server = await serve(state), context = await browser.newContext();
  try {
    await context.route('**/*', (route) => (new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort()));
    const page = await context.newPage(), errors = []; page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(server.origin + '/');
    const until = async (pred, ms) => { const end = Date.now() + ms; let t = ''; while (Date.now() < end) { t = await page.evaluate(() => document.body.innerText); if (pred(t)) return t; await new Promise((r) => setTimeout(r, 250)); } return t; };
    // The account's own list: what follows its "Recent account activity" heading (the Overview lists transactions too).
    const activity = (t) => { const i = t.indexOf('Recent account activity'); return i < 0 ? '' : t.slice(i); };
    await until((t) => /Latest block\s+40\b/.test(t), 30000);
    const box = 'input[aria-label="Search block, transaction or address"]';
    await page.fill(box, address(1)); await page.press(box, 'Enter');
    const first = activity(await until((t) => activity(t) && !/Reading the latest blocks/.test(activity(t)), 30000));
    const fullFirst = [...state.full];
    state.chain.block(HEAD + 1); state.failFull = failAfterMove; state.head = HEAD + 1; // block 41 holds one transaction of address(1)
    const second = activity(await until((t) => failAfterMove ? /reading the latest blocks failed/.test(activity(t)) : activity(t).includes(atx(HEAD + 1, 0)), 40000));
    return { first, second, fullFirst, fullAfter: state.full.slice(fullFirst.length), errors };
  } finally { await context.close(); await server.close(); }
}
// 5. The transaction view of a registerReplacing call whose receipt read is answered busy `busy` times first.
async function txView(busy) {
  const state = { chain: chain({ [HEAD - 1]: 1 }), busyTxRead: false, batches: [], head: HEAD, full: [], busyReceipt: busy };
  const word = (a) => '0'.repeat(24) + a.slice(2);
  const rtx = '0x' + 'ee'.repeat(32);
  state.chain.txs.set(rtx, { hash: rtx, blockNumber: hex(HEAD - 1), blockHash: hash('b', HEAD - 1), from: address(5), to: REGISTRY, value: '0x0', nonce: '0x0', gas: '0x7a120', gasPrice: '0x3b9aca00', input: '0x5326ca0b' + word(address(5)) + word(VICTIM) });
  const server = await serve(state), context = await browser.newContext();
  try {
    await context.route('**/*', (route) => (new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort()));
    const page = await context.newPage(), errors = []; page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(server.origin + '/');
    const dialog = () => page.evaluate(() => document.querySelector('dialog.detail-dialog')?.innerText || '');
    const box = 'input[aria-label="Search block, transaction or address"]';
    for (let t = Date.now(); Date.now() - t < 30000 && !/Latest block\s+40\b/.test(await page.evaluate(() => document.body.innerText));) await new Promise((r) => setTimeout(r, 200));
    await page.fill(box, rtx); await page.press(box, 'Enter');
    let text = '';
    for (const end = Date.now() + 30000; Date.now() < end;) { text = await dialog(); if (/Registry call\s+registerReplacing/.test(text) || /Unable to load this result/.test(text)) break; await new Promise((r) => setTimeout(r, 200)); }
    await new Promise((r) => setTimeout(r, 300)); text = await dialog();
    return { text, receiptBusy: state.receiptBusy || 0, receiptReads: state.batches.filter((b) => b.includes('eth_getTransactionReceipt')).length, errors };
  } finally { await context.close(); await server.close(); }
}
// 6. The Transactions tab with two rows at `width` px: the page's scroll width and the rows shown.
async function historyAt(width) {
  const state = { chain: chain({}), busyTxRead: false, batches: [], head: HEAD, full: [] };
  state.txPage = (q) => ({ schema: 'pocol-transactions-v1', chainId: DECLARED.chainId, genesisHash: DECLARED.genesisHash, page: Number(q.get('page')), size: Number(q.get('size')),
    order: q.get('order'), q: q.get('q'), total: 2, index: { height: HEAD, head: HEAD, from: 0, updatedAt: NOW, state: 'ready' }, snapshot: { height: HEAD, hash: hash('b', HEAD) },
    rows: [1, 0].map((i) => ({ hash: hash('d' + i, HEAD - 1), blockHash: hash('b', HEAD - 1), blockNumber: hex(HEAD - 1), transactionIndex: hex(i), timestamp: hex(NOW), value: '0x4563918244f40000', kind: 'Contract call', from: address(5), to: REGISTRY })) });
  const server = await serve(state), context = await browser.newContext({ viewport: { width, height: 844 } });
  try {
    await context.route('**/*', (route) => (new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort()));
    const page = await context.newPage(), errors = []; page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(server.origin + '/#page=Transactions');
    let rows = [];
    for (const end = Date.now() + 20000; Date.now() < end && rows.length < 2;) { rows = await page.evaluate(() => [...document.querySelectorAll('table.history-table tbody tr')].map((r) => r.innerText.replace(/\s+/g, ' '))); await new Promise((r) => setTimeout(r, 200)); }
    const [scroll, inner] = await page.evaluate(() => [document.documentElement.scrollWidth, window.innerWidth]);
    return { rows, scroll, inner, errors };
  } finally { await context.close(); await server.close(); }
}
// 7. The Miner map tab (and the Overview's compact map) without geography data: the panel's text.
async function minerMap() {
  const state = { chain: chain({}), busyTxRead: false, batches: [], head: HEAD, full: [] };
  const server = await serve(state), context = await browser.newContext({ viewport: { width: 390, height: 844 } });
  try {
    await context.route('**/*', (route) => (new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort()));
    const page = await context.newPage(), errors = []; page.on('pageerror', (e) => errors.push(e.message));
    const panel = () => page.evaluate(() => [...document.querySelectorAll('#pocol-miners-map, #pocol-miners-map-compact')].map((e) => e.innerText).join(' | '));
    const settle = async () => { let t = ''; for (const end = Date.now() + 20000; Date.now() < end;) { t = await panel(); if (/Miner geography is unavailable/.test(t)) break; await new Promise((r) => setTimeout(r, 200)); } return t; };
    await page.goto(server.origin + '/');
    const overview = await settle();
    await page.goto(server.origin + '/#page=Miner%20map'); await page.reload();
    const tab = await settle();
    const [scroll, inner] = await page.evaluate(() => [document.documentElement.scrollWidth, window.innerWidth]);
    return { overview, tab, scroll, inner, body: await page.evaluate(() => document.body.innerText), errors };
  } finally { await context.close(); await server.close(); }
}
const tx = (n, i) => { const h = hash('c' + i, n); return h.slice(0, 8) + '…' + h.slice(-6); }; // the Overview's short hash
const atx = (n, i) => { const h = hash('c' + i, n); return h.slice(0, 9) + '…' + h.slice(-7); }; // the account view's

try {
  // 1. The third request of the refresh refused (429, the gateway's busy answer).
  const busy = await overview({ [HEAD]: 2, [HEAD - 3]: 1 }, true);
  check('the fake relay serves the explorer: the Overview shows the head and "Active members 300"', /Latest block\s+40\b/.test(busy.text) && /Active members\s+300\b/.test(busy.text), busy.text.slice(0, 400));
  check('the refresh sent the status batch, the light blocks, then the transaction read (refused), retried twice',
    busy.batches.some((b) => b.includes('pocol_getMembers')) && busy.batches.filter((b) => b.length === 3 && b.every((m) => m === 'eth_getTransactionByHash')).length >= 3, JSON.stringify(busy.batches.slice(0, 6)));
  check('a refused transaction read shows "Latest transactions unavailable" with the gateway\'s message, not an empty window',
    busy.panel.includes('Latest transactions unavailable') && busy.panel.includes(`reading them failed: ${BUSY} The next update reads them again.`) && !busy.panel.includes('No transactions in this window') && !busy.panel.includes('Empty blocks are valid'), busy.panel.replace(/\s+/g, ' '));
  check('the Overview shows the insights map card, never the loaded members page as "registered miners", and never asks the v1 map',
    busy.compact && !/members on the loaded page/.test(busy.text) && !/registered miners/.test(busy.text) && !busy.state.v1Asked, (/[^\n]*members on the loaded page[^\n]*|[^\n]*registered miners[^\n]*/.exec(busy.text) || [''])[0]);
  check('the page raised no uncaught error', busy.errors.length === 0, busy.errors.join(' | '));
  // 1b. The same read refused with a message that has no closing period: it is still set apart from the next sentence.
  const down = await overview({ [HEAD]: 2, [HEAD - 3]: 1 }, 'down');
  check('a transaction read refused without a message ends "Data source unavailable." and keeps the next sentence apart',
    down.panel.includes('reading them failed: Data source unavailable. The next update reads them again.') && !down.panel.includes('No transactions in this window'), down.panel.replace(/\s+/g, ' '));
  // 2. The same read answered: the transactions are listed; and a window without transactions is the empty state.
  const answered = await overview({ [HEAD]: 2, [HEAD - 3]: 1 }, false);
  check('an answered transaction read lists the newest transactions', [tx(HEAD, 0), tx(HEAD, 1), tx(HEAD - 3, 0)].every((s) => answered.panel.includes(s)) && !answered.panel.includes('unavailable'), answered.panel.replace(/\s+/g, ' '));
  const empty = await overview({}, true);
  check('a window without transactions sends no transaction read and shows "No transactions in this window"',
    empty.panel.includes('No transactions in this window') && !empty.panel.includes('unavailable') && !empty.batches.some((b) => b.includes('eth_getTransactionByHash')), empty.panel.replace(/\s+/g, ' '));
  // 4. The account activity follows the loaded window.
  const acct = await accountFollowsWindow({ [HEAD]: 2, [HEAD - 19]: 1, [HEAD + 1]: 1 });
  check('the account view lists the account\'s transactions of the loaded window (blocks 21..40)',
    [atx(HEAD, 0), atx(HEAD, 1), atx(HEAD - 19, 0)].every((s) => acct.first.includes(s)) && acct.fullFirst.length === 2, JSON.stringify(acct.fullFirst) + ' ' + acct.first.slice(0, 300).replace(/\s+/g, ' '));
  check('when the head moves on, the new transaction appears and the one that left the window goes, without reopening the account',
    acct.second.includes(atx(HEAD + 1, 0)) && acct.second.includes(atx(HEAD, 0)) && !acct.second.includes(atx(HEAD - 19, 0)), acct.second.slice(0, 400).replace(/\s+/g, ' '));
  check('only the new block is read in full after the move (the others come from the cache)', acct.fullAfter.length === 1 && acct.fullAfter[0] === HEAD + 1, JSON.stringify(acct.fullAfter));
  // 4b. The same move with the new block's read refused: the last list stays, and a warning above it says it may be
  // out of date (it is not presented as the latest window's matches).
  const stale = await accountFollowsWindow({ [HEAD]: 2, [HEAD - 19]: 1, [HEAD + 1]: 1 }, true);
  check('a refused activity read keeps the last list and says above it that reading the latest blocks failed',
    stale.second.includes('These are the last transactions read; reading the latest blocks failed: Data source unavailable. They may be out of date until the next update reads them again.')
      && [atx(HEAD, 0), atx(HEAD, 1)].every((s) => stale.second.includes(s)) && !stale.second.includes(atx(HEAD + 1, 0)) && stale.second.indexOf('These are the last') < stale.second.indexOf(atx(HEAD, 0)),
    stale.second.slice(0, 500).replace(/\s+/g, ' '));
  // 5. The transaction view with the receipt read answered busy once by the node, then answered.
  const view = await txView(1);
  check('a registerReplacing view whose receipt read is first answered busy shows the decoded call and the named member (read again 1.5 s later)',
    /Registry call\s+registerReplacing/.test(view.text) && view.text.toLowerCase().includes(VICTIM) && /Named member now\s+Held/.test(view.text) && /Reverted/.test(view.text)
      && !/Unable to load this result/.test(view.text) && view.receiptBusy === 1 && view.receiptReads === 2, `${view.receiptBusy} busy, ${view.receiptReads} reads: ` + view.text.replace(/\s+/g, ' ').slice(0, 400));
  // Busy three times in a row: the view reports the node's answer after the two retries.
  const stuck = await txView(3);
  check('a receipt read answered busy three times in a row is reported as unavailable (two retries, no more)',
    /Unable to load this result/.test(stuck.text) && /busy/.test(stuck.text) && stuck.receiptReads === 3, `${stuck.receiptReads} reads: ` + stuck.text.replace(/\s+/g, ' ').slice(0, 300));
  // 6. No horizontal page scroll with history rows on a phone.
  const phone = await historyAt(390);
  check('the Transactions tab with rows at 390 px has no horizontal page scroll (the table scrolls in its own box), rows newest first',
    phone.rows.length === 2 && phone.rows[0].includes('Position 2') && phone.rows[1].includes('Position 1') && phone.scroll <= phone.inner, `scroll ${phone.scroll} / ${phone.inner}: ${JSON.stringify(phone.rows)}`);
  // 7. The Miner map without geography data.
  const map = await minerMap();
  const OLD = ['Miner locations are unavailable', 'Miner locations are not published', 'members on the loaded page'];
  const CLAIMS = ['Registered miners', 'Located miners', 'Network nodes', 'DB-IP', 'Natural Earth', 'Miners around the world'];
  for (const [where, text] of [['Miner map tab', map.tab], ['Overview', map.overview]]) {
    check(`the ${where}'s map without geography data says it is unavailable with the gateway's message, and shows no map or count`,
      /Miner geography is unavailable/.test(text) && text.includes('Unavailable in this test') && CLAIMS.every((c) => !text.includes(c)) && OLD.every((c) => !text.includes(c)), text.replace(/\s+/g, ' ').slice(0, 500));
  }
  check('the Miner map tab has its new subtitle and none of the old one-node texts, no horizontal scroll at 390 px',
    map.body.includes('Registered miners by country where an operator node reports it; every other miner is Unknown.') && OLD.every((c) => !map.body.includes(c)) && map.scroll <= map.inner,
    `scroll ${map.scroll} / ${map.inner}: ` + OLD.filter((c) => map.body.includes(c)).join(', '));
  check('no uncaught error in the other scenarios', map.errors.length === 0 && view.errors.length === 0 && stuck.errors.length === 0 && phone.errors.length === 0 && down.errors.length === 0 && answered.errors.length === 0 && empty.errors.length === 0 && acct.errors.length === 0 && stale.errors.length === 0, [...down.errors, ...answered.errors, ...empty.errors, ...acct.errors, ...stale.errors].join(' | '));
} finally { await browser.close(); }
console.log(`explorer W7 browser checks: ${checks} passed`);
