// RP header window (browser.md D77, D80–D84): items 1, 1b, viewFuture, 2–9 and
// viewTargetCeil over n <= 13 headers. Items 10–17 are not evaluable by RP.
import { keccak, sha256, recover } from './crypto.js';
import { decodeGenesisSpec, decodeHeaderRef, MAX_SPEC_BYTES } from './refdec.js';
import { concat, eq, be, toBig, fromHex, utf8 } from './hex.js';
export const VIEW_SLACK_S = 4n, PHI_VIEW = 10;
const MAX = (1n << 256n) - 1n;
/** GenesisSpec (A07): the full accepted schema and evaluation order (refdec.js, a port of the
 * reference decoder checked differentially), after the MAX_SPEC_BYTES input bound. */
export function decodeSpec(b) {
  if (b.length > MAX_SPEC_BYTES) throw Object.assign(new Error('InputTooLarge'), { gs: 'InputTooLarge', detail: String(b.length) });
  return decodeGenesisSpec(b);
}
const CPI = { g_ts: 0, target_g: 1, T_blk: 2, tau: 3, nonceMode: 4, c: 5, D_att: 6, dFbWait: 7, m: 8, H_END: 45 };
export const cpv = (spec, k) => spec.cp[CPI[k]];
const floorDiv = (a, b) => { const q = a / b; return (a % b !== 0n && ((a < 0n) !== (b < 0n))) ? q - 1n : q; };
export function asert(targetG, gTs, tBlk, tau, pTs, pH) {
  const dt = (pTs - gTs) - tBlk * pH; const e = floorDiv(dt * 65536n, tau); const s = floorDiv(e, 65536n); const f = e - 65536n * s;
  if (s >= 256n) return MAX; if (s <= -257n) return 1n;
  const F = 65536n + floorDiv(195766423245049n * f + 971821376n * f * f + 5127n * f * f * f + (1n << 47n), 1n << 48n);
  const X = targetG * F; const k = s - 16n; const Y = k >= 0n ? X << k : X >> -k;
  return Y < 1n ? 1n : (Y > MAX ? MAX : Y);
}
export const work = (t) => (1n << 256n) / (t + 1n);
/** Header (A08): every field, width and canonical encoding of the reference decoder before any
 * trust result (refdec.js). RP still does not establish execution or membership validity. */
export const decodeHeader = decodeHeaderRef;
const sigMsg = (tid) => keccak(concat(Uint8Array.of(0x19), utf8('PoCol template'), Uint8Array.of(0x0a), tid));
const winMsg = (x) => keccak(concat(Uint8Array.of(0x19), utf8('PoCol winner'), Uint8Array.of(0x0a), x.tid, x.nonceB, x.shareRoot));
/** Verifies the window; returns {ok, n, toGenesis, message} or throws {flag}. */
export async function verifyWindow(rpc, spec, clockS = Math.floor(Date.now() / 1000)) {
  const h = BigInt(await rpc('eth_blockNumber', []));
  if (h === 0n) return { ok: false, flag: 'viewNoBlocks', message: 'الشبكة لم تنتج الكتلة 1 بعد؛ لا رأس للتحقق في RP' };
  const b = h - 12n > 1n ? h - 12n : 1n; const from = b - 1n > 1n ? b - 1n : 1n; const count = h - from + 1n;
  const raw = await rpc('pocol_getHeaders', ['0x' + from.toString(16), '0x' + count.toString(16)]);
  if (!Array.isArray(raw) || BigInt(raw.length) !== count) throw Object.assign(new Error('viewIncomplete'), { flag: 'viewIncomplete' });
  const hs = raw.map((x) => decodeHeader(fromHex(x)));
  const tG = cpv(spec, 'target_g'), ceil = (tG << VIEW_SLACK_S) > MAX ? MAX : tG << VIEW_SLACK_S;
  const fail = (flag) => { throw Object.assign(new Error(flag), { flag }); };
  const netid = (x) => { if (x.chainId !== spec.chainId) fail('netChain'); if (!eq(x.genesisHash, spec.genesisHash)) fail('netGenesis'); if (x.version !== 1n) fail('netVersion'); if (x.ts > BigInt(clockS + PHI_VIEW)) fail('viewFuture'); };
  hs.forEach((x, i) => { if (x.h !== from + BigInt(i)) fail('viewIncomplete'); netid(x); });
  let parent = b > 1n ? hs[0] : { hash: spec.genesisHash, ts: cpv(spec, 'g_ts'), h: 0n };
  for (const x of hs.slice(b > 1n ? 1 : 0)) {
    if (!eq(x.parentHash, parent.hash) || x.h !== parent.h + 1n) fail(x.h === 1n ? 'viewGenesis' : '2');
    const signed = x.proposer.length > 0 || x.sig.length > 0;
    if (signed) { if (x.proposer.length !== 20 || x.sig.length !== 65 || x.a !== 0n) fail('3'); let r; try { r = recover(sigMsg(x.tid), x.sig); } catch { fail('3'); } if (!eq(r, x.proposer)) fail('3'); }
    const dAtt = cpv(spec, 'D_att'); const L = parent.ts + x.a * dAtt + 1n, U = parent.ts + (x.a + 1n) * dAtt;
    if (x.ts < L || x.ts > U) fail('4');
    if (!signed && x.ts !== (x.a === 0n ? L + cpv(spec, 'dFbWait') : L)) fail('4');
    const tgt = asert(tG, cpv(spec, 'g_ts'), cpv(spec, 'T_blk'), cpv(spec, 'tau'), parent.ts, parent.h);
    if (x.target !== tgt) fail('5');
    if (x.target > ceil) fail('viewTargetCeil');
    const cw = cpv(spec, 'c') * work(x.target); const nMax = cpv(spec, 'nonceMode') === 0n ? (cw < (1n << 64n) ? cw : 1n << 64n) : 1n << 64n;
    if (x.nonce >= nMax) fail('6');
    if (toBig(sha256(concat(x.tid, x.nonceB))) > x.target) fail('7');
    try { recover(winMsg(x), x.winnerSig); } catch { fail('8'); }
    const tShare = x.target * cpv(spec, 'm') > MAX ? MAX : x.target * cpv(spec, 'm');
    for (let i = 0; i < x.shares.length; i++) { const n = x.shares[i]; if ((i && n <= x.shares[i - 1]) || n === x.nonce || n >= nMax || toBig(sha256(concat(x.tid, be(n, 8)))) > tShare) fail('9'); }
    parent = x;
  }
  const n = h - b + 1n; const minWork = work(ceil);
  const message = `RP: مرتبط بالعمل والهوية لـ${n} رأسًا${b === 1n ? ' حتى genesis' : ''}، بعمل محتسب لا يقل عن ${minWork.toString(10)} لكل رأس (work(ceilTarget))، دون تحقق تنفيذ أو عضوية` + (minWork === 1n ? '. حد العمل غير فعال لهذه الشبكة' : '');
  return { ok: true, n: Number(n), toGenesis: b === 1n, minWork: minWork.toString(), head: Number(h), message };
}
