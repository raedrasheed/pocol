// Sites W7: the explorer bundle's v3 membership, light refresh and Registry decoding, made through the W9 patcher's
// anchored edits (tools/explorer-w7-edits.json, applied by tools/site_identity.py --edits; the bundle has no sources,
// owner answer Q1).
//   node --test deploy/websites/tests/explorer_w7.test.mjs
// 1. The edits file is what the served bundle carries: each `new` occurs once and each `old` not at all; undone in
//    reverse order they give a bundle without any PocolW7 name (the prefix was free), and redone they give the served
//    bundle byte for byte. Their text keeps the rules: English only, no chain ID or genesis literal, B_reg never written
//    down (it is each record's bondTargetWei), every new top-level name prefixed PocolW7.
// 2. The extracted pure helpers (Pe and every PocolW7 function between it and the bundle's next function) run in node:vm
//    over every bondState 0-4 x status 0-2 x tomb, malformed records, the ACCRUING bond thresholds and a null
//    proposerEligible; the Registry decoder over real calldata of all 8 selectors (checked against keccak-256 of the
//    PoColSystemV3.sol signatures), bad lengths, dirty address words and a non-Registry `to`.
// 3. The refresh reads one pocol_getMembers page and light blocks: no pocol_getMiners and no full=true block list.
// The relay size bounds of that refresh are deploy/websites/tests/test_explorer_w7_size.py; the live check on a v3
// devnet is explorer_smoke.mjs.
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
const EDITS = JSON.parse(readFileSync(join(ROOT, 'deploy', 'websites', 'tools', 'explorer-w7-edits.json'), 'utf8'));
const DECLARED = JSON.parse(readFileSync(join(ROOT, 'deploy', 'websites', 'site-identity.json'), 'utf8'));
const SOLIDITY = readFileSync(join(ROOT, 'prototypes', 'lp4-execution', 'contracts', 'system', 'PoColSystemV3.sol'), 'utf8');
const count = (text, part) => text.split(part).length - 1;

// ---- keccak-256 (the original Keccak padding), for the selector cross-check ----
function keccak256(bytes) {
  const M = (1n << 64n) - 1n, rot = (x, n) => ((x << BigInt(n)) | (x >> BigInt(64 - n))) & M;
  const RC = [], R = [[0, 36, 3, 41, 18], [1, 44, 10, 45, 2], [62, 6, 43, 15, 61], [28, 55, 25, 21, 56], [27, 20, 39, 8, 14]];
  for (let i = 0, r = 1n; i < 24; i++) { let c = 0n; for (let j = 0; j < 7; j++) { if (r & 1n) c |= 1n << ((1n << BigInt(j)) - 1n); r = (r << 1n) ^ (r & 0x80n ? 0x71n : 0n); r &= 0xffn; } RC.push(c); }
  const rate = 136, msg = [...bytes, 0x01]; while (msg.length % rate) msg.push(0); msg[msg.length - 1] |= 0x80;
  const s = Array.from({ length: 5 }, () => Array(5).fill(0n));
  for (let off = 0; off < msg.length; off += rate) {
    for (let i = 0; i < rate / 8; i++) { let lane = 0n; for (let b = 7; b >= 0; b--) lane = (lane << 8n) | BigInt(msg[off + 8 * i + b]); s[i % 5][Math.floor(i / 5)] ^= lane; }
    for (let round = 0; round < 24; round++) {
      const C = s.map((col) => col.reduce((a, b) => a ^ b)), D = C.map((_, x) => C[(x + 4) % 5] ^ rot(C[(x + 1) % 5], 1));
      for (let x = 0; x < 5; x++) for (let y = 0; y < 5; y++) s[x][y] ^= D[x];
      const B = Array.from({ length: 5 }, () => Array(5).fill(0n));
      for (let x = 0; x < 5; x++) for (let y = 0; y < 5; y++) B[y][(2 * x + 3 * y) % 5] = R[x][y] ? rot(s[x][y], R[x][y]) : s[x][y];
      for (let x = 0; x < 5; x++) for (let y = 0; y < 5; y++) s[x][y] = B[x][y] ^ (~B[(x + 1) % 5][y] & M & B[(x + 2) % 5][y]);
      s[0][0] ^= RC[round];
    }
  }
  const out = []; for (let i = 0; i < 4; i++) { const lane = s[i % 5][Math.floor(i / 5)]; for (let b = 0; b < 8; b++) out.push(Number((lane >> BigInt(8 * b)) & 0xffn)); }
  return Buffer.from(out).toString('hex');
}
const selector = (signature) => '0x' + keccak256(Buffer.from(signature)).slice(0, 8);

// ---- the extracted helpers ----
const start = BUNDLE.indexOf('function Pe(e){'), end = BUNDLE.indexOf('function Fe(e){');
assert.ok(start > 0 && end > start && count(BUNDLE, 'function Pe(e){') === 1 && count(BUNDLE, 'function Fe(e){') === 1, 'the helper region');
const REGION = BUNDLE.slice(start, end);
const NAMES = ['Pe', 'PocolW7Label', 'PocolW7Bond', 'PocolW7Decode', 'PocolW7TxRows', 'PocolW7MemberRows', 'PocolW7Counts',
  'PocolW7Hashes', 'PocolW7Blocks', 'PocolW7LastShare', 'PocolW7Match', 'PocolW7Pcol', 'PocolW7Registry', 'PocolW7Calls'];
// A fresh context with nothing of the bundle in it (only the host's setTimeout, for the retry's pause): the helpers
// name no other bundle binding.
const CONTEXT = Object.assign(Object.create(null), { setTimeout });
const RAW = vm.runInNewContext(`${REGION};({${NAMES.join(',')},PocolW7Retry})`, CONTEXT, { timeout: 2000 });
// Values from the context are copied into this realm (JSON), so deepStrictEqual compares them as plain data.
const plain = (x) => (x === undefined ? x : JSON.parse(JSON.stringify(x)));
const W = Object.fromEntries(Object.entries(RAW).map(([k, v]) => [k, typeof v === 'function' ? (...a) => plain(v(...a)) : plain(v)]));
const B_REG = 20n * 10n ** 18n;

test('the edits file is exactly what the served bundle carries, and undone it leaves no PocolW7 name', () => {
  assert.ok(Array.isArray(EDITS) && EDITS.length >= 20);
  for (const [i, e] of EDITS.entries()) {
    assert.deepEqual(Object.keys(e).sort(), ['count', 'file', 'new', 'old'], `edit ${i}`);
    assert.equal(e.file, 'explorer-bundle'); assert.equal(e.count, 1);
    assert.equal(count(BUNDLE, e.new), 1, `edit ${i}: its new text occurs once: ${e.new.slice(0, 80)}`);
    assert.equal(count(BUNDLE, e.old), 0, `edit ${i}: its old text is gone: ${e.old.slice(0, 80)}`);
  }
  let base = BUNDLE;
  for (const e of [...EDITS].reverse()) { assert.equal(count(base, e.new), 1); base = base.replace(e.new, () => e.old); }
  assert.equal(count(base, 'PocolW7'), 0, 'the prefix had no hit before the edits');
  let redone = base;
  for (const [i, e] of EDITS.entries()) { assert.equal(count(redone, e.old), 1, `edit ${i}: its old text was unique`); redone = redone.replace(e.old, () => e.new); }
  assert.equal(createHash('sha256').update(redone).digest('hex'), createHash('sha256').update(BUNDLE).digest('hex'));
  assert.equal(count(BUNDLE, 'function Pe(e){return e.status===1&&e.tomb===!1&&e.bondState===1}'), 0, 'Pe no longer treats only HELD as eligible');
});

test('the edits keep the W7 rules: English only, no identity literal, B_reg read from each record, PocolW7 names', () => {
  const added = EDITS.map((e) => e.new).join('\n');
  assert.doesNotMatch(added, /[֐-ࣿיִ-﷿ﹰ-﻿]/, 'English only (the explorer stays English at launch)');
  for (const literal of [String(DECLARED.chainId), DECLARED.chainId.toString(16), DECLARED.genesisHash.slice(2)]) assert.ok(!added.toLowerCase().includes(literal.toLowerCase()), literal);
  assert.doesNotMatch(added, /20000000000000000000|2e19|\/20 PCOL|`20`/, 'B_reg is the record\'s bondTargetWei, never a literal');
  assert.match(REGION, /PocolW7Pcol\(e\?\.bondTargetWei\)/);
  // Every top-level name the region declares besides Pe, and every new binding in the edits, carries the prefix.
  const declared = [...REGION.matchAll(/(?:^|[;}])(?:async\s+)?function\s+([A-Za-z_$][\w$]*)\(/g)].map((m) => m[1]);
  assert.deepEqual(declared.filter((n) => n !== 'Pe' && !n.startsWith('PocolW7')), []);
  const vars = [...REGION.matchAll(/(?:^|[;}])var\s+([^;]*)/g)].flatMap((m) => [...m[1].matchAll(/(?:^|,)([A-Za-z_$][\w$]*)=/g)].map((v) => v[1]));
  assert.ok(vars.length >= 3 && vars.every((n) => n.startsWith('PocolW7')), vars.join(' '));
  // In the App the edits add bindings with prefixed names only; a one-letter name is a parameter or local of a new
  // closure (minified style, shadowing nothing it reads), whose rendering explorer_smoke.mjs checks live.
  for (const e of EDITS.slice(1)) for (const m of e.new.matchAll(/(?:let|var|const)\s*(\[[^=]*?\]|[A-Za-z_$][\w$]*)\s*=/g)) {
    const names = m[1].replace(/[[\]]/g, ',').split(',').map((s) => s.trim()).filter(Boolean);
    const fresh = names.filter((n) => !e.old.includes(`${n}=`) && !/^[a-z]$/.test(n));
    assert.ok(fresh.every((n) => n.startsWith('PocolW7')), `a new binding without the prefix: ${fresh} in ${e.new.slice(0, 80)}`);
  }
});

test('the refresh reads one members page and light blocks; nothing reads every miner record', () => {
  assert.equal(count(BUNDLE, 'pocol_getMiners'), 0);
  assert.equal(count(BUNDLE, '[`pocol_getMembers`,[0,256]]'), 1);
  assert.equal(count(BUNDLE, '[`pocol_getMembers`,[PocolW7Start,256]]'), 1, 'the Miners tab pages by 256');
  assert.doesNotMatch(BUNDLE, /\[ot\([A-Za-z]+-t\),!0\]/, 'no block list asks for full blocks');
  assert.equal(count(BUNDLE, '[ot(r-t),!1]'), 1); assert.equal(count(BUNDLE, '[ot(M-t),!1]'), 1);
  // Full blocks only one at a time: the block view and the account activity (sequential, at most 20).
  assert.equal(count(BUNDLE, '`eth_getBlockByNumber`,[a,!0]'), 1);
  assert.match(BUNDLE, /PocolW7Blocks\(l\.blocks,20\);\(async\(\)=>\{let i=\[\];for\(let a of r\)\{let o=await at\(/);
  // Each beside its view's batch, retried while the shared gateway budget is busy (pocol_getMember costs 20 of its 80
  // tokens), its failure kept to its own rows (a busy gateway still shows the balance).
  assert.equal(count(BUNDLE, 'PocolW7Retry(()=>at(`pocol_getMember`,[n])).catch(e=>({PocolW7Error:e.message}))'), 1, 'the account view reads the member record');
  assert.equal(count(BUNDLE, 'PocolW7Retry(()=>at(`pocol_getMember`,[PocolW7D.victim])).catch(e=>({PocolW7Error:e.message}))'), 1, 'the transaction view reads the named member');
});

test('the map counts the loaded members page, and a failed transaction read is not shown as an empty window', () => {
  // The map gets one pocol_getMembers page (at most 256), not every record: its summary says so.
  assert.equal(count(BUNDLE, 'registered miners'), 0);
  assert.equal(count(BUNDLE, 'children:t?`${e.length} members on the loaded page`:`Miner registry loading`'), 1);
  // The Overview's eth_getTransactionByHash batch can be refused (429) after the status and block reads succeed: its
  // error is kept beside the empty list and the panel says the read failed (explorer_w7_browser.mjs checks it live).
  assert.equal(count(BUNDLE, '.catch(()=>[])'), 0, 'the transaction read\'s error is not dropped');
  assert.equal(count(BUNDLE, '.catch(e=>(PocolW7TE=e.message,[]))'), 1);
  assert.equal(count(BUNDLE, 'PocolW7Txs:PocolW7T,PocolW7TxError:PocolW7TE})'), 1);
  assert.equal(count(BUNDLE, ':l?.PocolW7TxError?I(`Latest transactions unavailable`,`${l.PocolW7TxError} '), 1);
});

test('a member read is tried up to 3 times with a growing pause, then its last error is shown', async () => {
  const retry = RAW.PocolW7Retry;
  let calls = 0;
  assert.equal(await retry(async () => { calls++; return 'ok'; }, 3, 1), 'ok'); assert.equal(calls, 1);
  calls = 0;
  assert.equal(await retry(async () => { if (++calls < 3) throw new Error('The data service is busy. Please retry shortly.'); return 'third'; }, 3, 1), 'third');
  assert.equal(calls, 3);
  calls = 0; const started = Date.now();
  await assert.rejects(retry(async () => { calls++; throw new Error(`busy ${calls}`); }, 3, 20), /busy 3/);
  assert.equal(calls, 3); assert.ok(Date.now() - started >= 55, 'pauses of 20 then 40 ms');
  assert.match(REGION, /async function PocolW7Retry\(e,t=3,n=1500\)/, 'the bundle tries 3 times, 1.5 s then 3 s apart');
});

// ---- labels ----
function expectedLabel(status, bondState, tomb, dueHeight) {
  if (status === 0) return 'Not a member';
  if (![1, 2, 3, 4].includes(bondState)) return 'Unknown';
  if (tomb) return 'Tombed';
  if (bondState === 2) return 'Burned';
  if (bondState === 3) return 'Paid';
  if (status === 1) return bondState === 1 ? 'Held' : 'Earning bond';
  return dueHeight === undefined ? 'Exited' : `Exited (refund at block ${dueHeight})`;
}

test('every status, bondState and tomb gets its label; Pe counts the live HELD and ACCRUING members', () => {
  let cases = 0;
  for (const status of [0, 1, 2]) for (const bondState of [0, 1, 2, 3, 4]) for (const tomb of [false, true]) for (const dueHeight of [undefined, 9000]) {
    const record = { status, bondState, tomb, exists: status !== 0, ...(dueHeight === undefined ? {} : { dueHeight }) };
    assert.equal(W.PocolW7Label(record), expectedLabel(status, bondState, tomb, dueHeight), JSON.stringify(record));
    // A pocol_getMembers row has no `exists` field.
    const row = { status, bondState, tomb };
    assert.equal(W.PocolW7Label(row), expectedLabel(status, bondState, tomb, undefined), JSON.stringify(row));
    assert.equal(W.Pe(record), status === 1 && !tomb && (bondState === 1 || bondState === 4), JSON.stringify(record));
    cases++;
  }
  assert.equal(cases, 60);
  assert.equal(W.PocolW7Label({ status: 1, bondState: 1, tomb: false, exists: false }), 'Not a member');
  assert.equal(W.PocolW7Match({ status: 1, bondState: 4, tomb: false }, 'Earning bond'), true);
  assert.equal(W.PocolW7Match({ status: 1, bondState: 4, tomb: true }, 'Earning bond'), false);
  assert.equal(W.PocolW7Match({ status: 1, bondState: 1, tomb: true }, 'Tombed'), true);
  assert.equal(W.PocolW7Match({ status: 9 }, 'All'), true);
});

test('a malformed or future record is "Unknown", never a crash', () => {
  for (const r of [null, undefined, 0, 'x', [], {}, { status: '1', bondState: 1, tomb: false }, { status: 1, bondState: 5, tomb: false },
    { status: 3, bondState: 1, tomb: false }, { status: 1, bondState: 1, tomb: 'no' }, { status: 1, bondState: 1 }, { status: 1, bondState: null, tomb: false },
    { status: 2, bondState: 1, tomb: false, dueHeight: -1 }, { status: 2, bondState: 1, tomb: false, dueHeight: 1.5 }]) {
    const label = W.PocolW7Label(r);
    assert.ok(['Unknown', 'Exited'].includes(label), `${JSON.stringify(r)} -> ${label}`);
    assert.doesNotThrow(() => { W.Pe(r || {}); W.PocolW7Bond(r); W.PocolW7MemberRows(r); W.PocolW7LastShare(r, 10); W.PocolW7Counts([r], { active: 1 }); });
  }
  assert.equal(W.PocolW7Label({ status: 2, bondState: 4, tomb: false, dueHeight: 1.5 }), 'Exited');
});

test('the bond reads "x/B PCOL" from each record, through the ACCRUING thresholds, and a null proposerEligible is undetermined', () => {
  const rec = (bond, eligible, target = B_REG) => ({ bondWei: String(bond), bondTargetWei: String(target), proposerEligible: eligible });
  const quarter = B_REG / 4n;
  assert.deepEqual([...W.PocolW7Bond(rec(0n, false))], ['0/20 PCOL', 'No']);
  assert.deepEqual([...W.PocolW7Bond(rec(quarter - 1n, null))], ['4.999999999999999999/20 PCOL', 'Undetermined']);
  assert.deepEqual([...W.PocolW7Bond(rec(quarter, true))], ['5/20 PCOL', 'Yes']);
  assert.deepEqual([...W.PocolW7Bond(rec(B_REG - 1n, true))], ['19.999999999999999999/20 PCOL', 'Yes']);
  assert.deepEqual([...W.PocolW7Bond(rec(B_REG, true))], ['20/20 PCOL', 'Yes']);
  assert.deepEqual([...W.PocolW7Bond(rec(4n * 10n ** 18n, undefined, 16n * 10n ** 18n))], ['4/16 PCOL', 'Undetermined'], 'B_reg is the record\'s');
  assert.deepEqual([...W.PocolW7Bond({ bondWei: 'x', bondTargetWei: '-1' })], ['—/— PCOL', 'Undetermined']);
  assert.deepEqual([...W.PocolW7Bond(null)], ['—/— PCOL', 'Undetermined']);
});

test('the account view rows: a member, an exited member, no member and an unavailable read', () => {
  const member = { height: 120, blockHash: '0x' + '11'.repeat(32), address: '0x' + 'aa'.repeat(20), exists: true, status: 1, tomb: false, bondState: 4,
    lastShare: 100, registeredAt: 7, rewardAddress: '0x' + 'bb'.repeat(20), bondWei: String(B_REG / 4n), bondTargetWei: String(B_REG),
    exitHeight: 0, dueHeight: 0, removal: 4, removalLabel: 'RECENTLY_ACTIVE', proposerEligible: true };
  assert.deepEqual(W.PocolW7MemberRows(member).map((r) => [...r]), [
    ['Mining membership', 'Earning bond'], ['Bond', '5/20 PCOL'], ['Proposer eligible (next block)', 'Yes'],
    ['Reward address', '0x' + 'bb'.repeat(20), true], ['Removal check', 'Not removable: recently active'], ['Registered at', 'Block 7'],
    ['Last share', 'Block 100 · inactive for 20 blocks'], ['Membership read at', 'Block 120 (the head)']]);
  const exited = W.PocolW7MemberRows({ ...member, status: 2, bondState: 1, dueHeight: 4000, removalLabel: 'NOT_LIVE_ACTIVE' }).map((r) => [...r]);
  assert.deepEqual(exited[0], ['Mining membership', 'Exited (refund at block 4000)']);
  assert.ok(exited.some((r) => r[0] === 'Refund at' && r[1] === 'Block 4000'));
  assert.ok(exited.some((r) => r[1] === 'Not removable: not a live active record'));
  assert.deepEqual(W.PocolW7MemberRows({ ...member, exists: false, status: 0, bondState: 0 }).map((r) => [...r]), [['Mining membership', 'Not a member']]);
  assert.deepEqual(W.PocolW7MemberRows({ PocolW7Error: 'Explorer busy; retry shortly' }).map((r) => [...r]), [['Mining membership', 'Unavailable: Explorer busy; retry shortly']]);
  assert.equal(W.PocolW7LastShare({ lastShare: 0 }, 50), 'None yet');
  assert.equal(W.PocolW7LastShare({ lastShare: 60 }, 50), 'Block 60');
});

test('counts are for the loaded page and say so when it is not the whole set', () => {
  const rows = [{ status: 1, bondState: 1, tomb: false }, { status: 1, bondState: 1, tomb: false }, { status: 1, bondState: 4, tomb: false }, { status: 1, bondState: 1, tomb: true }];
  assert.equal(W.PocolW7Counts(rows, { active: 4, start: 0 }), '2 held · 1 earning bond · 1 tombed');
  assert.equal(W.PocolW7Counts(rows, { active: 300, start: 256 }), '2 held · 1 earning bond · 1 tombed in members 257–260 of 300');
  assert.equal(W.PocolW7Counts([], { active: 0, start: 0 }), '0 held · 0 earning bond · 0 tombed');
  assert.equal(W.PocolW7Counts(undefined, undefined), '0 held · 0 earning bond · 0 tombed');
});

test('light blocks: the five newest transaction hashes and the blocks with transactions, at most 20', () => {
  const h = (n) => '0x' + n.toString(16).padStart(64, '0');
  const blocks = [{ number: '0x9', timestamp: '0x9', transactions: [h(1), h(2)] }, { number: '0x8', timestamp: '0x8', transactions: [] },
    { number: '0x7', timestamp: '0x7', transactions: [h(3), h(4), h(5), h(6)] }];
  assert.deepEqual(W.PocolW7Hashes(blocks, 5).map((r) => [...r]), [[h(1), '0x9'], [h(2), '0x9'], [h(3), '0x7'], [h(4), '0x7'], [h(5), '0x7']]);
  assert.deepEqual([...W.PocolW7Hashes([{ transactions: [{ hash: h(1) }] }], 5)], [], 'a full block\'s objects are not hashes');
  assert.deepEqual([...W.PocolW7Blocks(blocks, 20)], ['0x9', '0x7']);
  const many = Array.from({ length: 30 }, (_, i) => ({ number: '0x' + i.toString(16), transactions: [h(i)] }));
  assert.equal(W.PocolW7Blocks(many, 20).length, 20);
});

// ---- Registry decoding ----
const REGISTRY = '0x0000000000000000000000000000000000c0c001';
const word = (address) => '0'.repeat(24) + address.slice(2).toLowerCase();
const A1 = '0x' + '12'.repeat(20), A2 = '0x' + 'ab'.repeat(20);
const SIGNATURES = { register: 'register(address)', registerReplacing: 'registerReplacing(address,address)', reactivate: 'reactivate()',
  setRewardAddress: 'setRewardAddress(address)', removeInactive: 'removeInactive(address)', deregister: 'deregister()', claimBond: 'claimBond()', topUp: 'topUp()' };

test('the decoder\'s 8 selectors are keccak-256 of the Registry signatures in PoColSystemV3.sol', () => {
  assert.equal(W.PocolW7Registry, REGISTRY);
  const calls = Object.fromEntries(Object.entries(W.PocolW7Calls).map(([sel, [name]]) => [name, sel]));
  assert.deepEqual(Object.keys(calls).sort(), Object.keys(SIGNATURES).sort());
  for (const [name, signature] of Object.entries(SIGNATURES)) {
    assert.ok(SOLIDITY.includes(`keccak256("${signature}")`), `PoColSystemV3.sol dispatches ${signature}`);
    assert.equal(calls[name], selector(signature), signature);
  }
  assert.equal(selector('register(address)'), '0x4420e486');
});

test('real calldata of all 8 selectors decodes, with the value and the named member', () => {
  const sel = Object.fromEntries(Object.entries(SIGNATURES).map(([n, s]) => [n, selector(s)]));
  const tx = (input, value = '0x0', to = REGISTRY) => ({ to, input, value });
  const d = (t) => { const r = W.PocolW7Decode(t); return r && { ...r, args: r.args.map((a) => [...a]), value: r.value && [...r.value] }; };
  assert.deepEqual(d(tx(sel.register + word(A1), '0x' + (B_REG / 4n).toString(16))),
    { name: 'register', args: [['Reward address', A1]], value: ['Bond paid', '5 PCOL'], victim: null, undecoded: false });
  assert.deepEqual(d(tx(sel.registerReplacing + word(A1) + word(A2), '0x0')),
    { name: 'registerReplacing', args: [['Reward address', A1], ['Replaced member', A2]], value: ['Bond paid', '0 PCOL'], victim: A2, undecoded: false });
  assert.deepEqual(d(tx(sel.removeInactive + word(A2))), { name: 'removeInactive', args: [['Removed member', A2]], value: null, victim: A2, undecoded: false });
  assert.deepEqual(d(tx(sel.setRewardAddress + word(A1))), { name: 'setRewardAddress', args: [['New reward address', A1]], value: null, victim: null, undecoded: false });
  assert.deepEqual(d(tx(sel.topUp, '0x' + (15n * 10n ** 18n).toString(16))), { name: 'topUp', args: [], value: ['Top-up amount', '15 PCOL'], victim: null, undecoded: false });
  for (const name of ['reactivate', 'deregister', 'claimBond']) assert.deepEqual(d(tx(sel[name])), { name, args: [], value: null, victim: null, undecoded: false });
  // The Registry's address in any case, calldata in upper case.
  assert.equal(d(tx((sel.register + word(A2)).toUpperCase().replace('0X', '0x'), '0x0', REGISTRY.toUpperCase().replace('0X', '0x'))).args[0][1], A2);
  const rows = (decoded, victim) => W.PocolW7TxRows(W.PocolW7Decode(decoded), victim).map((r) => [...r]);
  const victim = { height: 77, exists: true, status: 2, bondState: 1, tomb: false, dueHeight: 500 };
  assert.deepEqual(rows(tx(sel.registerReplacing + word(A1) + word(A2)), victim), [['Registry call', 'registerReplacing'], ['Reward address', A1, true],
    ['Replaced member', A2, true], ['Bond paid', '0 PCOL'], ['Named member now', 'Exited (refund at block 500) · read at the head (block 77), not at this transaction\'s height']]);
  assert.deepEqual(rows(tx(sel.removeInactive + word(A2)), { PocolW7Error: 'Query unavailable' }).at(-1), ['Named member now', 'Unavailable: Query unavailable']);
  assert.deepEqual(rows(tx(sel.removeInactive + word(A2)), null).at(-1), ['Named member now', 'Unavailable: not read']);
  assert.deepEqual(W.PocolW7TxRows(null, null).length, 0);
});

test('bad calldata is "Registry call (undecoded)", and a call to another address is not a Registry call', () => {
  const sel = Object.fromEntries(Object.entries(SIGNATURES).map(([n, s]) => [n, selector(s)]));
  const bad = [sel.register + word(A1).slice(2), sel.register + word(A1) + '00', sel.register,
    sel.registerReplacing + word(A1) + word(A2).slice(2), sel.registerReplacing + word(A1) + word(A2) + '00', sel.registerReplacing + word(A1),
    sel.deregister + '00', sel.claimBond + word(A1), sel.topUp + '00', sel.reactivate + word(A1),
    sel.setRewardAddress + '01' + word(A1).slice(2), sel.removeInactive + 'f'.repeat(24) + A1.slice(2),
    sel.registerReplacing + word(A1) + '1' + word(A2).slice(1), '0x12345678', '0x1234', '0x', '', '0xzz' + word(A1), sel.register + word(A1) + '0',
    '0x' + 'ab'.repeat(36), null, 42];
  for (const input of bad) {
    const r = W.PocolW7Decode({ to: REGISTRY, input, value: '0x0' });
    assert.equal(r.name, 'Registry call (undecoded)', String(input));
    assert.equal(r.undecoded, true); assert.equal(r.victim, null); assert.equal(r.args.length, 0);
    assert.deepEqual(W.PocolW7TxRows(r, null).map((x) => [...x]), [['Registry call', 'Registry call (undecoded)']]);
  }
  for (const to of [null, undefined, '0x0000000000000000000000000000000000c0c002', '0x0000000000000000000000000000000000c0c004', A1, 'c0c001'])
    assert.equal(W.PocolW7Decode({ to, input: sel.register + word(A1), value: '0x0' }), null, String(to));
  assert.equal(W.PocolW7Decode(null), null);
});
