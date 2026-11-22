// Production-readiness edits of the explorer bundle (tools/explorer-prod-edits.json, applied after W7 and the site-truth
// edits by tools/site_identity.py --edits; the bundle has no sources, owner answer Q1).
//   node --test deploy/websites/tests/explorer_prod.test.mjs
// 1. The edits file is what the served bundle carries: each `new` once and each `old` not at all; undone in reverse they
//    give a bundle without any PocolP1 name, and redone they give the served bundle byte for byte. English only, no
//    chain ID or genesis literal.
// 2. The relay read `it` (every /api/rpc.php request of the bundle) retries a transient refusal twice, 1.5 s then 3 s
//    later: the relay's busy or failure answer (HTTP 429, 502, 503) and a read the node or gateway answered busy or
//    temporarily unavailable (-32021, -32000). Anything else (a refused method, a moved chain, a timeout, a 400) is
//    reported at once, as before. The transaction view (explorer_smoke.mjs: registerReplacing) needs it: its
//    receipt read is heavy on v3, and the node answers -32021 busy when its heavy lane is full. A 429 is retried no
//    sooner than its Retry-After (at most 30 s), as the gateway asks (Retry-After: 3).
// 3. The texts: the transaction index's first kept block, and miner geography (the network records no miner location;
//    peers are not placed by IP address). Without geography data (this gateway never serves any) the Miner map is only
//    that notice: none of its GeoIP texts (map, counts, filters, attribution) is shown beside it.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const HERE = dirname(fileURLToPath(import.meta.url)), ROOT = join(HERE, '..', '..', '..');
const SITE = join(ROOT, 'deploy', 'websites', 'explorer.pocol.net');
const page = readFileSync(join(SITE, 'index.html'), 'utf8');
const served = [...page.matchAll(/<script\b[^>]*\bsrc="\/(assets\/index-[A-Za-z0-9_-]+\.js)"/g)].map((m) => m[1]);
assert.equal(served.length, 1, 'index.html loads one explorer bundle');
const BUNDLE = readFileSync(join(SITE, served[0]), 'utf8');
const EDITS = JSON.parse(readFileSync(join(ROOT, 'deploy', 'websites', 'tools', 'explorer-prod-edits.json'), 'utf8'));
const DECLARED = JSON.parse(readFileSync(join(ROOT, 'deploy', 'websites', 'site-identity.json'), 'utf8'));
const count = (text, part) => text.split(part).length - 1;

test('the edits file is exactly what the served bundle carries, and undone it leaves no PocolP1 name', () => {
  assert.ok(Array.isArray(EDITS) && EDITS.length >= 9);
  for (const [i, e] of EDITS.entries()) {
    assert.deepEqual(Object.keys(e).sort(), ['count', 'file', 'new', 'old'], `edit ${i}`);
    assert.equal(e.file, 'explorer-bundle'); assert.equal(e.count, 1);
    assert.equal(count(BUNDLE, e.new), 1, `edit ${i}: its new text occurs once: ${e.new.slice(0, 80)}`);
    assert.equal(count(BUNDLE, e.old), 0, `edit ${i}: its old text is gone: ${e.old.slice(0, 80)}`);
  }
  let base = BUNDLE;
  for (const e of [...EDITS].reverse()) base = base.replace(e.new, () => e.old);
  assert.equal(count(base, 'PocolP1'), 0, 'the prefix had no hit before the edits');
  let redone = base;
  for (const [i, e] of EDITS.entries()) { assert.equal(count(redone, e.old), 1, `edit ${i}: its old text was unique`); redone = redone.replace(e.old, () => e.new); }
  assert.equal(createHash('sha256').update(redone).digest('hex'), createHash('sha256').update(BUNDLE).digest('hex'));
  const added = EDITS.map((e) => e.new).join('\n');
  assert.doesNotMatch(added, /[֐-ࣿיִ-﷿ﹰ-﻿]/, 'English only (the explorer stays English at launch)');
  for (const literal of [String(DECLARED.chainId), DECLARED.chainId.toString(16), DECLARED.genesisHash.slice(2)]) assert.ok(!added.toLowerCase().includes(literal.toLowerCase()), literal);
});

// ---- the relay read, run in node:vm with a scripted fetch ----
const start = BUNDLE.indexOf('var rt=0;async function it(e){'), end = BUNDLE.indexOf('var ot=e=>`0x`+e.toString(16)');
assert.ok(start > 0 && end > start, 'the relay read region');
const REGION = BUNDLE.slice(start, end);
function relay(respond) {
  // respond(request body, n): the n-th fetch's answer {status, body} (n from 1), or it throws to reject the fetch.
  const asked = [], delays = [];
  const fetch = async (url, options) => {
    const body = JSON.parse(options.body); asked.push(body);
    const { status, text, headers = {} } = respond(body, asked.length);
    const get = (name) => Object.entries(headers).find(([k]) => k.toLowerCase() === name.toLowerCase())?.[1] ?? null;
    return { status, ok: status >= 200 && status < 300, headers: { get }, text: async () => text };
  };
  const context = Object.assign(Object.create(null), { fetch, AbortSignal: { timeout: () => undefined }, setTimeout: (f, ms) => { delays.push(ms); f(); } });
  return { it: vm.runInNewContext(`${REGION};it`, context, { timeout: 2000 }), asked, delays };
}
const answer = (asked, fields) => JSON.stringify(asked.map((c) => ({ jsonrpc: '2.0', id: c.id, ...fields(c) })));
const plain = (x) => JSON.parse(JSON.stringify(x));
const BUSY = 'The data service is busy. Please retry shortly.'; // deploy/rewards/explorer_api.py BUSY
const NODE_BUSY = { code: -32021, message: 'busy', data: { reason: 'busy', retryAfterMs: 250 } }; // pocold rpc.rs busy()

test('a read the node answers busy is retried 1.5 s later; then the batch succeeds', async () => {
  // The transaction view's batch: the receipt (heavy on v3) answered busy by the node, relayed as a member error.
  const r = relay((body, n) => ({ status: 200, text: answer(body, (c) => (c.method === 'eth_getTransactionReceipt' && n === 1 ? { error: NODE_BUSY } : { result: c.method === 'eth_blockNumber' ? '0x10' : { status: '0x0' } })) }));
  const result = await r.it([['eth_getTransactionReceipt', ['0x' + '11'.repeat(32)]], ['eth_blockNumber', []]]);
  assert.deepEqual(plain(result), [{ status: '0x0' }, '0x10']);
  assert.equal(r.asked.length, 2); assert.deepEqual(r.delays, [1500]);
  // The gateway's "Node data temporarily unavailable" (-32000) twice, then the answer: 1.5 s then 3 s.
  const g = relay((body, n) => ({ status: 200, text: answer(body, () => (n < 3 ? { error: { code: -32000, message: 'Node data temporarily unavailable' } } : { result: '0x2a' })) }));
  assert.deepEqual(plain(await g.it([['eth_blockNumber', []]])), ['0x2a']);
  assert.equal(g.asked.length, 3); assert.deepEqual(g.delays, [1500, 3000]);
});

test('the relay\'s busy and failure answers are retried; three in a row report the relay\'s message', async () => {
  for (const status of [429, 502, 503]) {
    const r = relay(() => ({ status, text: status === 429 ? JSON.stringify({ error: BUSY }) : '{}' }));
    await assert.rejects(r.it([['eth_blockNumber', []]]), (e) => e.message === (status === 429 ? BUSY : 'Data source unavailable'));
    assert.equal(r.asked.length, 3, `HTTP ${status}: asked three times`); assert.deepEqual(r.delays, [1500, 3000]);
  }
  // An empty 502 (Caddy with the gateway down) is retried too, and its message kept.
  const empty = relay(() => ({ status: 502, text: '' }));
  await assert.rejects(empty.it([['eth_blockNumber', []]]), /empty response \(HTTP 502\)/);
  assert.equal(empty.asked.length, 3);
  // A busy relay that then answers.
  const later = relay((body, n) => (n === 1 ? { status: 429, text: JSON.stringify({ error: BUSY }) } : { status: 200, text: answer(body, () => ({ result: '0x2a' })) }));
  assert.deepEqual(plain(await later.it([['eth_blockNumber', []]])), ['0x2a']);
  assert.equal(later.asked.length, 2);
});

test('a 429 is retried no sooner than its Retry-After, at most 30 s; other answers keep 1.5 s and 3 s', async () => {
  const busy = (after) => () => ({ status: 429, text: JSON.stringify({ error: BUSY }), headers: after === undefined ? {} : { 'Retry-After': after } });
  for (const [after, delays] of [['3', [3000, 3000]], ['10', [10000, 10000]], ['1', [1500, 3000]], ['3600', [30000, 30000]],
    ['0', [1500, 3000]], ['soon', [1500, 3000]], [undefined, [1500, 3000]]]) {
    const r = relay(busy(after));
    await assert.rejects(r.it([['eth_blockNumber', []]]), (e) => e.message === BUSY);
    assert.deepEqual(r.delays, delays, `Retry-After ${after}`);
  }
  // Retry-After on a 502 or 503 (not the gateway's busy answer) is not used.
  const r = relay(() => ({ status: 503, text: '{}', headers: { 'Retry-After': '20' } }));
  await assert.rejects(r.it([['eth_blockNumber', []]]));
  assert.deepEqual(r.delays, [1500, 3000]);
  // A busy relay that then answers, waited for as asked.
  const later = relay((body, n) => (n === 1 ? busy('3')() : { status: 200, text: answer(body, () => ({ result: '0x2a' })) }));
  assert.deepEqual(plain(await later.it([['eth_blockNumber', []]])), ['0x2a']);
  assert.deepEqual(later.delays, [3000]);
});

test('a final answer is reported at once: a refused method, a moved chain, a 400, a timeout, an unreadable body', async () => {
  const once = async (respond, pattern) => {
    const r = relay(respond);
    await assert.rejects(r.it([['eth_blockNumber', []]]), pattern);
    assert.equal(r.asked.length, 1, String(pattern)); assert.deepEqual(r.delays, []);
  };
  await once((b) => ({ status: 200, text: answer(b, () => ({ error: { code: -32601, message: 'Method not found' } })) }), /Method not found/);
  await once((b) => ({ status: 200, text: answer(b, () => ({ error: { code: -32022, message: 'Chain changed. Retry the membership read.' } })) }), /Chain changed/);
  await once(() => ({ status: 400, text: JSON.stringify({ error: 'Only supported, bounded explorer reads are accepted.' }) }), /Only supported/);
  await once(() => { const e = new Error('The operation was aborted due to timeout'); e.name = 'TimeoutError'; throw e; }, /timeout/);
  await once(() => ({ status: 200, text: 'not json' }), /unreadable response/);
});

test('the texts: the first kept block of the transaction index, and miner geography without IP placement', () => {
  assert.equal(count(BUNDLE, 'Number.isSafeInteger(E?.from)&&E.from>0?`Transactions from block #${E.from.toLocaleString()} onward; this gateway no longer keeps older ones.`:`Every included transaction, from the first block onward.`'), 1);
  for (const gone of ['Update-PoCol-Explorer-GeoIP.sh', 'Check the GeoIP gateway update', 'Automatic country estimates from observed network connections.']) assert.equal(count(BUNDLE, gone), 0, gone);
  assert.equal(count(BUNDLE, 'Miner locations are unavailable: the PoCol network does not record where miners are, and this explorer does not place peers by their IP addresses.'), 1);
  assert.equal(count(BUNDLE, 'Miner locations are not published: the network does not record them.'), 1);
  // Without geography data the panel returns only its notice (before the map, its counts and its GeoIP texts).
  const notice = BUNDLE.indexOf('if(!r.data)return(0,N.jsx)(`section`,{className:`panel miner-map-panel');
  const panel = BUNDLE.indexOf('"aria-label":`Automatic network geography`');
  assert.ok(notice > 0 && panel > notice && panel - notice < 600, 'the notice returns right before the geography panel');
  const early = BUNDLE.slice(notice, panel);
  assert.match(early, /children:r\.error\|\|\(r\.loading\?`Checking for miner location data\.`:`Miner locations are unavailable\.`\)/);
  for (const gone of ['GeoIP', 'IP', 'Countries', 'country', 'estimated']) assert.ok(!early.includes(gone), gone);
});
