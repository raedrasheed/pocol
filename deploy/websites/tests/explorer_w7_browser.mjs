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
// 3. The map's summary counts the loaded members page ("256 members on the loaded page"), not "registered miners",
//    beside "Active members 300".
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
    case 'pocol_getMember': return ok(null);
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
      if (state.busyTxRead && calls.every((c) => c.method === 'eth_getTransactionByHash')) {
        // 'down': a 503 without an error field, which the bundle reports as "Data source unavailable" (no period).
        if (state.busyTxRead === 'down') { res.writeHead(503, { 'content-type': 'application/json; charset=utf-8' }); return res.end('{}'); }
        res.writeHead(429, { 'content-type': 'application/json; charset=utf-8', 'retry-after': '3' }); return res.end(JSON.stringify({ error: BUSY }));
      }
      res.writeHead(200, { 'content-type': 'application/json; charset=utf-8' }); return res.end(JSON.stringify(calls.map((c) => answer(state, c))));
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
async function overview(txBlocks, busyTxRead) {
  const state = { chain: chain(txBlocks), busyTxRead, batches: [], head: HEAD, full: [] };
  const server = await serve(state), context = await browser.newContext();
  try {
    await context.route('**/*', (route) => (new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort()));
    const page = await context.newPage(), errors = []; page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(server.origin + '/');
    const latest = page.locator('section.panel', { has: page.locator('h2', { hasText: 'Latest transactions' }) });
    // The first refresh is done once its last request (the transaction read, when the window has transactions) is
    // answered and the page shows the head; then one render more.
    const requests = Object.keys(txBlocks).length ? 3 : 2, deadline = Date.now() + 30000; let text = '';
    while (Date.now() < deadline) {
      text = await page.evaluate(() => document.body.innerText);
      if (/Latest block\s+40\b/.test(text) && state.batches.length >= requests && !/Miner registry loading/.test(text)) break;
      await new Promise((r) => setTimeout(r, 200));
    }
    await new Promise((r) => setTimeout(r, 500)); text = await page.evaluate(() => document.body.innerText);
    const panel = (await latest.count()) ? await latest.first().innerText() : '';
    return { text, panel, errors, batches: state.batches, state };
  } finally { await context.close(); await server.close(); }
}

// 4. An account view left open follows the loaded window: when the head moves on, a new transaction of the account
//    appears, one that left the latest 20 blocks goes, and only the new block is read in full (the others come from
//    the activity's cache, keyed by block hash).
async function accountFollowsWindow(txBlocks) {
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
    state.chain.block(HEAD + 1); state.head = HEAD + 1; // block 41 holds one transaction of address(1)
    const second = activity(await until((t) => activity(t).includes(atx(HEAD + 1, 0)), 40000));
    return { first, second, fullFirst, fullAfter: state.full.slice(fullFirst.length), errors };
  } finally { await context.close(); await server.close(); }
}
const tx = (n, i) => { const h = hash('c' + i, n); return h.slice(0, 8) + '…' + h.slice(-6); }; // the Overview's short hash
const atx = (n, i) => { const h = hash('c' + i, n); return h.slice(0, 9) + '…' + h.slice(-7); }; // the account view's

try {
  // 1. The third request of the refresh refused (429, the gateway's busy answer).
  const busy = await overview({ [HEAD]: 2, [HEAD - 3]: 1 }, true);
  check('the fake relay serves the explorer: the Overview shows the head and "Active members 300"', /Latest block\s+40\b/.test(busy.text) && /Active members\s+300\b/.test(busy.text), busy.text.slice(0, 400));
  check('the refresh sent the status batch, the light blocks, then the transaction read (refused)',
    busy.batches.some((b) => b.includes('pocol_getMembers')) && busy.batches.some((b) => b.length === 3 && b.every((m) => m === 'eth_getTransactionByHash')), JSON.stringify(busy.batches.slice(0, 4)));
  check('a refused transaction read shows "Latest transactions unavailable" with the gateway\'s message, not an empty window',
    busy.panel.includes('Latest transactions unavailable') && busy.panel.includes(`reading them failed: ${BUSY} The next update reads them again.`) && !busy.panel.includes('No transactions in this window') && !busy.panel.includes('Empty blocks are valid'), busy.panel.replace(/\s+/g, ' '));
  check('the map\'s summary counts the loaded members page: "256 members on the loaded page", never "registered miners"',
    /\b256 members on the loaded page\b/.test(busy.text) && !/registered miners/.test(busy.text), (/[^\n]*members on the loaded page[^\n]*|[^\n]*registered miners[^\n]*/.exec(busy.text) || [''])[0]);
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
  check('no uncaught error in the other scenarios', down.errors.length === 0 && answered.errors.length === 0 && empty.errors.length === 0 && acct.errors.length === 0, [...down.errors, ...answered.errors, ...empty.errors, ...acct.errors].join(' | '));
} finally { await browser.close(); }
console.log(`explorer W7 browser checks: ${checks} passed`);
