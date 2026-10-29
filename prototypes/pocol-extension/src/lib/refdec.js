// Reference-equivalent decoders (A07, A08): ports of the accepted Rust decoders
// lp1-node/src/genesis_spec.rs (GenesisSpec v1: framing L0, then gsStructure, gsVersion,
// gsCount, gsInt, gsLen, gsRange, gsOrder, gsSys, each stage over the whole input) and
// lp1-node/src/header.rs (size, strict RLP, then every header field). Error codes and details
// equal the reference's; tests/decoder_diff.mjs checks this against verdicts produced by the
// Rust decoders on the same corpus (pocold/tests/decoder_reference.rs).
import { keccak, sha256 } from './crypto.js';
import { concat, toBig } from './hex.js';

// ------------------------------------------------------------------ GenesisSpec
const gsErr = (gs, detail) => Object.assign(new Error(gs + ' ' + detail), { gs, detail });
export const SPEC_VERSION = 1, CP_LEN = 52;
const N = null;
/** CP version 1: name, width in bits, lower bound, upper bound (null: none; 'gamma': 10^4 - alpha_bp). */
export const CP_SCHEMA = [
  ['g_ts', 64, 0, N], ['target_g', 256, 1, N], ['T_blk', 32, 1, N], ['tau', 32, 1, N], ['nonceMode', 8, 0, 1], ['c', 8, 1, N],
  ['D_att', 32, 0, N], ['dFbWait', 32, 0, N], ['m', 16, 1, N], ['kappa', 8, 1, N], ['S_max', 16, 0, 256], ['D', 16, 0, N], ['W_w', 16, 0, N],
  ['alpha_bp', 16, 0, 10000], ['gamma_bp', 16, 0, 'gamma'], ['subsidy', 256, 0, N], ['B_reg', 256, 0, N], ['R_max', 8, 0, N], ['M_max', 16, 1, N],
  ['M_min', 16, 1, N], ['REG_RECORDS_MAX', 32, 0, N], ['SWEEP_MAX', 16, 0, N], ['SWEEP_SCAN', 16, 0, N], ['REG_OPS', 16, 0, N], ['W_act', 32, 0, N],
  ['Theta_inact', 32, 0, N], ['E_win', 32, 0, N], ['U', 32, 0, N], ['E_max', 8, 0, N], ['EVID_BYTES_MAX', 32, 0, N], ['GAS_LIMIT', 64, 0, N],
  ['baseFee0', 256, 0, N], ['TX_MAX', 32, 0, N], ['BODY_MAX', 32, 0, N], ['B_code_max', 32, 0, N], ['CODE_CAP', 64, 0, N], ['TXBYTES_CAP', 64, 0, N],
  ['TX_CAP', 64, 0, N], ['SLOT_CAP', 64, 0, N], ['ACCT_CAP', 64, 0, N], ['SYS_SLOT_MAX', 32, 0, N], ['SYS_KEYS_BLOCK_MAX', 32, 0, N],
  ['SYS_KEYS_USER_MAX', 32, 0, N], ['SYS_ACCT_BLOCK_MAX', 32, 0, N], ['G_SYS', 64, 0, N], ['H_END', 64, 0, N], ['T_END', 64, 0, N], ['H_CLOSE', 64, 0, N],
  ['T_CLOSE', 64, 0, N], ['Z_close', 32, 0, N], ['RET', 32, 1, N], ['P2', 32, 0, N],
];
const ALPHA_BP = 13;
export const MAX_MEMBERS = 65535;
const beLen = (v) => { let n = 0; while (v > 0) { n++; v = Math.floor(v / 256); } return n; };
const uintItemLen = (v) => (v < 0x80 ? 1 : 1 + beLen(v));
const listLen = (p) => (p < 56 ? 1 + p : 1 + beLen(p) + p);
const maxCpPayload = CP_SCHEMA.reduce((a, [, w, , hi]) => a + (hi === N ? 1 + w / 8 : uintItemLen(hi === 'gamma' ? 10000 : hi)), 0);
/** Largest encoding that can pass decode and R12 (genesis_spec.rs MAX_SPEC_BYTES). */
export const MAX_SPEC_BYTES = listLen(1 + 9 + listLen(maxCpPayload) + 33 + listLen(MAX_MEMBERS * listLen(21 + 21)) + 33);

function longLen(b, i, ll, short) {
  if (b.length - (i + 1) < ll) throw gsErr('L0', 'truncated length');
  if (b[i + 1] === 0) throw gsErr('L0', 'leading zero in length');
  let n = 0; for (let k = 0; k < ll; k++) n = n * 256 + b[i + 1 + k];
  if (n < 56) throw gsErr('L0', short);
  return [n, i + 1 + ll];
}
function head(b, i) {
  if (i >= b.length) throw gsErr('L0', 'truncated');
  const p = b[i];
  if (p < 0x80) return { k: 'byte' };
  if (p <= 0xb7) return { k: 'str', n: p - 0x80, st: i + 1 };
  if (p <= 0xbf) { const [n, st] = longLen(b, i, p - 0xb7, 'long form for short string'); return { k: 'str', n, st }; }
  if (p <= 0xf7) return { k: 'list', n: p - 0xc0, st: i + 1 };
  const [n, st] = longLen(b, i, p - 0xf7, 'long form for short list'); return { k: 'list', n, st };
}
function checkFraming(b) {
  const ends = []; let i = 0;
  for (;;) {
    const h = head(b, i);
    if (h.k === 'byte') i += 1;
    else if (h.k === 'str') { if (h.n > b.length - h.st) throw gsErr('L0', 'truncated string'); i = h.st + h.n; }
    else { if (h.n > b.length - h.st) throw gsErr('L0', 'truncated list'); i = h.st; if (h.n > 0) { ends.push(h.st + h.n); continue; } }
    for (;;) {
      if (!ends.length) { if (i !== b.length) throw gsErr('L0', `${b.length - i} trailing bytes`); return; }
      const end = ends[ends.length - 1];
      if (i > end) throw gsErr('L0', 'item crosses list end');
      if (i < end) break;
      ends.pop();
    }
  }
}
function itemAt(b, i) {
  const h = head(b, i);
  if (h.k === 'byte') return { list: false, start: i, len: 1, wrapped: false };
  if (h.k === 'str') return { list: false, start: h.st, len: h.n, wrapped: h.n === 1 && b[h.st] < 0x80 };
  return { list: true, start: h.st, len: h.n, wrapped: false };
}
function children(b, it) { const out = []; if (!it.list) return out; let i = it.start; const end = it.start + it.len; while (i < end) { const c = itemAt(b, i); out.push(c); i = c.start + c.len; } return out; }
const bytesOf = (b, it) => b.subarray(it.start, it.start + it.len);
const significant = (v) => { let i = 0; while (i < v.length && v[i] === 0) i++; return v.subarray(i); };
const bitLen = (v) => (v.length === 0 ? 0 : 8 * (v.length - 1) + (32 - Math.clz32(v[0])));
const big = (v) => { let n = 0n; for (const x of v) n = (n << 8n) | BigInt(x); return n; };
// Linear-time parse for an unbounded item (the gsVersion detail can be ~MAX_SPEC_BYTES long).
const bigHex = (v) => (v.length ? BigInt('0x' + hex(v)) : 0n);
const cmp = (a, b) => { for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return a[i] - b[i]; return 0; };
const hex = (v) => Array.from(v, (x) => x.toString(16).padStart(2, '0')).join('');
function isReserved(id) {
  if (id.every((x) => x === 0) || (id.subarray(0, 19).every((x) => x === 0xff) && id[19] === 0xfe)) return true;
  const low = (id[17] << 16) | (id[18] << 8) | id[19];
  return id.subarray(0, 17).every((x) => x === 0) && low >= 0xc0c001 && low <= 0xc0c0ff;
}
/** GenesisSpec.decode; throws {gs, detail} exactly as the reference does. */
export function decodeGenesisSpec(b) {
  checkFraming(b);
  const top = itemAt(b, 0);
  if (!top.list) throw gsErr('gsStructure', 'top not a list');
  const kinds = [false, false, true, false, true, false]; const slot = [null, null, null, null, null, null]; let topLen = 0;
  for (const [idx, it] of children(b, top).entries()) { if (idx < 6) { if (it.list !== kinds[idx]) throw gsErr('gsStructure', `top[${idx}]`); slot[idx] = it; } topLen++; }
  const none = { list: true, start: 0, len: 0, wrapped: false };
  const cp = slot[2] || none, m0 = slot[4] || none;
  if (children(b, cp).some((x) => x.list)) throw gsErr('gsStructure', 'CP item is a list');
  for (const e of children(b, m0)) if (!e.list || children(b, e).some((x) => x.list)) throw gsErr('gsStructure', 'M_0List entry');
  if (slot[0]) { const raw = bytesOf(b, slot[0]); const sig = significant(raw); if (!(sig.length === 1 && sig[0] === SPEC_VERSION)) throw gsErr('gsVersion', bigHex(raw).toString(10)); }
  if (!(slot.every((x) => x) && topLen === 6)) throw gsErr('gsCount', `top ${topLen}`);
  const [version, chain, , allocRoot, , sysCodeHash] = slot;
  const cps = children(b, cp); if (cps.length !== CP_LEN) throw gsErr('gsCount', `CP ${cps.length}`);
  const ms = children(b, m0);
  for (const e of ms) { const n = children(b, e).length; if (n !== 2) throw gsErr('gsCount', `M_0List entry ${n}`); }
  if (ms.length === 0) throw gsErr('gsCount', 'M_0List empty');
  const ints = [['specVersion', version], ['chainId', chain], ...CP_SCHEMA.map((f, k) => [f[0], cps[k]])];
  for (const [name, it] of ints) if (it.wrapped || (it.len > 0 && b[it.start] === 0)) throw gsErr('gsInt', name);
  if (allocRoot.len !== 32 || sysCodeHash.len !== 32) throw gsErr('gsLen', 'root');
  const pair = (e) => { const c = children(b, e); return [c[0] || none, c[1] || none]; };
  for (const e of ms) { const [id, reward] = pair(e); if (id.len !== 20 || reward.len !== 20) throw gsErr('gsLen', 'M_0List address'); }
  const chainB = bytesOf(b, chain); if (chainB.length === 0 || chainB.length > 8) throw gsErr('gsRange', 'chainId');
  const vals = [];
  for (let k = 0; k < CP_LEN; k++) {
    const [name, width, lo, hi] = CP_SCHEMA[k]; const v = bytesOf(b, cps[k]);
    if (bitLen(v) > width || v.length > 32) throw gsErr('gsRange', name);
    const x = big(v); vals.push(x);
    if (x < BigInt(lo)) throw gsErr('gsRange', name);
    const h = hi === N ? null : hi === 'gamma' ? (vals[ALPHA_BP] >= 10000n ? 0n : 10000n - vals[ALPHA_BP]) : BigInt(hi);
    if (h !== null && x > h) throw gsErr('gsRange', name);
  }
  const ids = ms.map((e) => bytesOf(b, pair(e)[0]));
  for (let i = 1; i < ids.length; i++) if (cmp(ids[i - 1], ids[i]) >= 0) throw gsErr('gsOrder', 'ids');
  for (const id of ids) if (isReserved(id)) throw gsErr('gsSys', '0x' + hex(id));
  return { chainId: big(chainB), cp: vals, allocRoot: bytesOf(b, allocRoot).slice(), m0: ms.map((e) => { const [id, r] = pair(e); return { id: bytesOf(b, id).slice(), reward: bytesOf(b, r).slice() }; }),
    sysCodeHash: bytesOf(b, sysCodeHash).slice(), genesisHash: keccak(b) };
}

// ------------------------------------------------------------------ strict RLP (lp1-node rlp.rs)
export const RLP_MAX_DEPTH = 16;
const rlpErr = (code, detail) => Object.assign(new Error(code + ' ' + detail), { code, detail });
function prefix(b, i, end) {
  if (i >= end) throw rlpErr('decode.truncated', 'offset');
  const p = b[i];
  if (p < 0x80) return [false, i, 1];
  const list = p >= 0xc0; const shortBase = list ? 0xc0 : 0x80, longBase = list ? 0xf7 : 0xb7;
  if (p <= longBase) {
    const n = p - shortBase;
    if (n > end - (i + 1)) throw rlpErr('decode.truncated', list ? 'list body' : 'string body');
    if (!list && n === 1 && b[i + 1] < 0x80) throw rlpErr('decode.noncanonical', 'single byte wrapped');
    return [list, i + 1, n];
  }
  const ll = p - longBase;
  if (ll > end - (i + 1)) throw rlpErr('decode.truncated', 'length of length');
  if (b[i + 1] === 0) throw rlpErr('decode.noncanonical', 'leading zero in length');
  let n = 0; for (let k = 0; k < ll; k++) n = n * 256 + b[i + 1 + k];
  if (!Number.isSafeInteger(n)) throw rlpErr('decode.truncated', 'length of length');
  if (n < 56) throw rlpErr('decode.noncanonical', list ? 'long form for short list' : 'long form for short string');
  const s = i + 1 + ll;
  if (n > end - s) throw rlpErr('decode.truncated', list ? 'list body' : 'string body');
  return [list, s, n];
}
function rlpItem(b, i, end, depth, max) {
  const [list, s, n] = prefix(b, i, end); const stop = s + n;
  if (!list) return [{ raw: b.subarray(i, stop), bytes: b.subarray(s, stop) }, stop];
  if (depth + 1 > max) throw rlpErr('decode.depth', 'nesting');
  const items = []; let j = s;
  while (j < stop) { const [c, next] = rlpItem(b, j, stop, depth + 1, max); items.push(c); j = next; }
  return [{ raw: b.subarray(i, stop), list: items }, stop];
}
export function rlpStrict(b, max = RLP_MAX_DEPTH) {
  const [it, j] = rlpItem(b, 0, b.length, 0, max);
  if (j !== b.length) throw rlpErr('decode.trailing', 'trailing bytes');
  return it;
}

// ------------------------------------------------------------------ header (lp1-node header.rs)
export const TAG = 'PoCol-tpl-v1'; const TAG_BYTES = new TextEncoder().encode(TAG);
export const UT_MAX = 613, ST_MAX = 682, HDR_MAX = 3067, SHARES_MAX = 256, UT_FIELDS = 18;
const hFail = (detail) => Object.assign(new Error('1 ' + detail), { flag: '1', detail });
const uintBytes = (it, width, name) => { const v = it.bytes; if (v === undefined || v.length > width || (v.length && v[0] === 0)) throw hFail(name); return v; };
const fixed = (it, n, name) => { const v = it.bytes; if (v === undefined || v.length !== n) throw hFail(name); return v; };
/** header.rs decode: size, strict RLP, then parse_item with the reference's details. */
export function decodeHeaderRef(b) {
  if (b.length > HDR_MAX) throw hFail('size');
  let item; try { item = rlpStrict(b); } catch (e) { throw hFail(e.code); }
  const top = item.list; if (!top || top.length !== 4) throw hFail('header arity');
  const st = top[0].list; if (!st || st.length !== 2) throw hFail('template arity');
  const ut = st[0].list; if (!ut || ut.length !== UT_FIELDS) throw hFail('template arity');
  const tag = ut[0].bytes; if (!tag || tag.length !== TAG_BYTES.length || tag.some((x, i) => x !== TAG_BYTES[i])) throw hFail('tag');
  const chainId = big(uintBytes(ut[1], 8, 'integer'));
  const genesisHash = fixed(ut[2], 32, 'genesisHash'); const parentHash = fixed(ut[3], 32, 'parentHash');
  const h = big(uintBytes(ut[4], 8, 'integer')); const a = big(uintBytes(ut[5], 4, 'integer')); const version = big(uintBytes(ut[6], 2, 'integer'));
  const ts = big(uintBytes(ut[7], 8, 'integer')); const target = big(uintBytes(ut[8], 32, 'integer'));
  const stateRoot = fixed(ut[9], 32, 'stateRoot'); const txRoot = fixed(ut[10], 32, 'txRoot'); const receiptsRoot = fixed(ut[11], 32, 'receiptsRoot');
  const logsBloom = fixed(ut[12], 256, 'logsBloom');
  const gasLimit = big(uintBytes(ut[13], 8, 'integer')); const gasUsed = big(uintBytes(ut[14], 8, 'integer')); const baseFee = big(uintBytes(ut[15], 32, 'integer'));
  const evidenceRoot = fixed(ut[16], 32, 'evidenceRoot');
  const proposer = ut[17].bytes; if (proposer === undefined || !(proposer.length === 0 || proposer.length === 20)) throw hFail('proposer');
  if (target === 0n) throw hFail('target range');
  const sig = st[1].bytes; if (sig === undefined || !(sig.length === 0 || sig.length === 65)) throw hFail('sig');
  const nonceB = top[1].bytes; if (nonceB === undefined || nonceB.length !== 8) throw hFail('nonce');
  const shareItems = top[2].list; if (!shareItems || shareItems.length > SHARES_MAX) throw hFail('shareList');
  const shares = shareItems.map((s) => { if (s.bytes === undefined || s.bytes.length !== 8) throw hFail('shareList'); return toBig(s.bytes); });
  const winnerSig = top[3].bytes; if (winnerSig === undefined || winnerSig.length !== 65) throw hFail('winnerSig');
  if (st[0].raw.length > UT_MAX || top[0].raw.length > ST_MAX || item.raw.length > HDR_MAX) throw hFail('size');
  const tid = sha256(st[0].raw); const shareRoot = keccak(top[2].raw);
  return { chainId, genesisHash, parentHash, h, a, version, ts, target, stateRoot, txRoot, receiptsRoot, logsBloom, gasLimit, gasUsed, baseFee, evidenceRoot,
    proposer, sig, nonce: toBig(nonceB), nonceB, shares, winnerSig, tid, shareRoot, hash: keccak(concat(tid, nonceB, shareRoot)) };
}
