// RP header window (browser.md D77, D80–D84): items 1, 1b, viewFuture, 2–9 and
// viewTargetCeil over n <= 13 headers. Items 10–17 are not evaluable by RP.
//
// v3 (G3a survey on the final v3 rules, pocold chain.rs validate/child_pre/membership/evidence_effects and fork choice):
// every v3 host rule is keyed on v3Active below. None of them changes an item RP evaluates: the header layout, linkage,
// the template signature, the attempt window and fallback timestamp, ASERT, PoW, winnerSig and the share list (items
// 1–9) are the same; nMax stays the fork-version/nonceMode rule (fullRange: C1 changes who owns which nonce range, not
// the domain). The v3 rules sit in items RP cannot evaluate without state: all-live membership and the nonce ranges it
// assigns (C1, items 12–14), the base-fee floor baseFeeMin (item 11), late shares as kind-3 evidence (E3, item 15), the
// evidence penalties (E2b), issuance, carry and the tip split (E2a, item 17), and registration by replacement of an
// inactive member at a full set (FN-2b registerReplacing, item 17; the seat contest, the probation pool and T_PROB are
// gone); timely-signed precedence (C2) is fork choice, which RP does not make (it verifies the one window an endpoint
// serves). So a spec-2 window is verified exactly as a spec-1 one, and the result says that the v3 rules were not
// evaluated.
import { keccak, sha256, recover } from './crypto.js';
import { decodeGenesisSpec, decodeHeaderRef, maxSpecBytes, cpIndexFor, DECODE_CEILING } from './refdec.js';
import { concat, eq, be, toBig, fromHex, utf8 } from './hex.js';
import { normalizeForkSchedule, activeVersion } from './forks.js';
export const VIEW_SLACK_S = 4n, PHI_VIEW = 10;
const MAX = (1n << 256n) - 1n;
/** GenesisSpec (A07): the full accepted schema and evaluation order (refdec.js, a port of the
 * reference decoder checked differentially), after the MAX_SPEC_BYTES input bound. The result carries its
 * specVersion. The default ceiling is the build's DECODE_CEILING: the release decoder's SPEC_VERSION, so a spec-2 (v3)
 * genesis is refused with gsVersion until activation, and 2 only in the test-only v3-preview build; an explicit
 * `maxVersion` is for tests and explicit preview callers only. */
export function decodeSpec(b, maxVersion = DECODE_CEILING) {
  if (b.length > maxSpecBytes(maxVersion)) throw Object.assign(new Error('InputTooLarge'), { gs: 'InputTooLarge', detail: String(b.length) });
  return decodeGenesisSpec(b, maxVersion);
}
/** A CP value by name in the schema of the spec's own version (GenesisSpec::cp_value). The v2 schema keeps the v1
 * prefix, so every field RP reads has the same index in both; a v2-only name on a v1 spec is an error. */
export const cpv = (spec, k) => {
  const i = cpIndexFor(spec.specVersion ?? 1, k);
  if (i < 0 || i >= spec.cp.length) throw new Error(`CP field ${k} is not in spec version ${spec.specVersion ?? 1}`);
  return spec.cp[i];
};
const floorDiv = (a, b) => { const q = a / b; return (a % b !== 0n && ((a < 0n) !== (b < 0n))) ? q - 1n : q; };
export function asert(targetG, gTs, tBlk, tau, pTs, pH) {
  const dt = (pTs - gTs) - tBlk * pH; const e = floorDiv(dt * 65536n, tau); const s = floorDiv(e, 65536n); const f = e - 65536n * s;
  if (s >= 256n) return MAX; if (s <= -257n) return 1n;
  const F = 65536n + floorDiv(195766423245049n * f + 971821376n * f * f + 5127n * f * f * f + (1n << 47n), 1n << 48n);
  const X = targetG * F; const k = s - 16n; const Y = k >= 0n ? X << k : X >> -k;
  return Y < 1n ? 1n : (Y > MAX ? MAX : Y);
}
export const work = (t) => (1n << 256n) / (t + 1n);
/** pocold chain::full_nonce_rule: v2 introduced the full u64 nonce domain and every later version keeps it. The rule
 * reads only the fork version and CP nonceMode, never the spec version: a v3 (spec-2) genesis has nonceMode 1. */
export const fullRange = (v, m) => v >= 2n || m === 1n;
/** The one v3 predicate (V3-DESIGN §3.3; pocold Spec::v3_active, lp1-node ProfileIdentity::v3_active,
 * pocol_rewards.v3_active): active_version(h) >= 3 || genesis specVersion >= 2. `forks` is a normalized local schedule
 * (forks.js; MAX_FORK_VERSION is still 2, so today only the spec-2 route can be true). */
export const v3Active = (spec, forks, h) => (spec.specVersion ?? 1) >= 2 || activeVersion(forks, h) >= 3n;
/** Header (A08): every field, width and canonical encoding of the reference decoder before any
 * trust result (refdec.js). RP still does not establish execution or membership validity. */
export const decodeHeader = decodeHeaderRef;
const sigMsg = (tid) => keccak(concat(Uint8Array.of(0x19), utf8('PoCol template'), Uint8Array.of(0x0a), tid));
const winMsg = (x) => keccak(concat(Uint8Array.of(0x19), utf8('PoCol winner'), Uint8Array.of(0x0a), x.tid, x.nonceB, x.shareRoot));
/** Verifies the window; returns {ok, n, toGenesis, message} or throws {flag}.
 * forkSchedule comes only from the saved local profile; omitted means v1 only. */
export async function verifyWindow(rpc, spec, clockS = Math.floor(Date.now() / 1000), forkSchedule = undefined) {
  const forks = normalizeForkSchedule(forkSchedule);
  const h = BigInt(await rpc('eth_blockNumber', []));
  if (h === 0n) return { ok: false, flag: 'viewNoBlocks', message: 'The network has not produced block 1 yet; no header is available for RP verification' };
  const b = h - 12n > 1n ? h - 12n : 1n; const from = b - 1n > 1n ? b - 1n : 1n; const count = h - from + 1n;
  const raw = await rpc('pocol_getHeaders', ['0x' + from.toString(16), '0x' + count.toString(16)]);
  if (!Array.isArray(raw) || BigInt(raw.length) !== count) throw Object.assign(new Error('viewIncomplete'), { flag: 'viewIncomplete' });
  const hs = raw.map((x) => decodeHeader(fromHex(x)));
  const tG = cpv(spec, 'target_g'), ceil = (tG << VIEW_SLACK_S) > MAX ? MAX : tG << VIEW_SLACK_S;
  const fail = (flag) => { throw Object.assign(new Error(flag), { flag }); };
  const netid = (x) => { if (x.chainId !== spec.chainId) fail('netChain'); if (!eq(x.genesisHash, spec.genesisHash)) fail('netGenesis'); if (x.version !== activeVersion(forks, x.h)) fail('netVersion'); if (x.ts > BigInt(clockS + PHI_VIEW)) fail('viewFuture'); };
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
    const cw = cpv(spec, 'c') * work(x.target); const nMax = fullRange(x.version, cpv(spec, 'nonceMode')) ? 1n << 64n : (cw < (1n << 64n) ? cw : 1n << 64n);
    if (x.nonce >= nMax) fail('6');
    if (toBig(sha256(concat(x.tid, x.nonceB))) > x.target) fail('7');
    try { recover(winMsg(x), x.winnerSig); } catch { fail('8'); }
    const tShare = x.target * cpv(spec, 'm') > MAX ? MAX : x.target * cpv(spec, 'm');
    for (let i = 0; i < x.shares.length; i++) { const n = x.shares[i]; if ((i && n <= x.shares[i - 1]) || n === x.nonce || n >= nMax || toBig(sha256(concat(x.tid, be(n, 8)))) > tShare) fail('9'); }
    parent = x;
  }
  const n = h - b + 1n; const minWork = work(ceil);
  // A v3 window (spec 2, or a future fork >= 3 at the head) says so; a spec-1 result is unchanged.
  const v3 = v3Active(spec, forks, h);
  const message = `RP: work and identity bound for ${n} headers${b === 1n ? ' back to genesis' : ''}, with computed work of at least ${minWork.toString(10)} per header (work(ceilTarget)), without execution or membership verification` + (minWork === 1n ? '. The work bound is ineffective for this network' : '')
    + (v3 ? '. Consensus v3 is active: its membership, fork-choice, evidence and reward rules are not evaluated by RP' : '');
  return { ok: true, n: Number(n), toGenesis: b === 1n, minWork: minWork.toString(), head: Number(h), message, ...(v3 ? { v3: true } : {}) };
}
