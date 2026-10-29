// SiteStorage (browser.md D34/D96/D104): per (netKey, site) dictionary, literal
// quota entry(k,v)=utf8len(k)+utf8len(v), SITE_STORE_MAX = 1048576 (equality ok),
// SITE_ENTRIES_MAX = 4096, the records themselves live in records.js (D103).
export const SITE_STORE_MAX = 1048576, SITE_ENTRIES_MAX = 4096;
const u8 = (s) => new TextEncoder().encode(s).length;
export const entry = (k, v) => u8(k) + u8(v);
export const total = (d) => Object.entries(d).reduce((a, [k, v]) => a + entry(k, v), 0);
/** Pure decision: returns {next} or {error:{code,message,data}}. */
export function apply(dict, method, params) {
  // Null-prototype dictionaries: '__proto__' is an ordinary key (review finding: it was silently dropped).
  if (method === 'site_storageClear') return { next: Object.create(null) };
  const [k, v] = params; const next = Object.assign(Object.create(null), dict); const t = total(dict);
  if (v === null) { delete next[k]; return { next }; }
  const has = Object.prototype.hasOwnProperty.call(dict, k);
  /*@fault quotaNoSubtract*/const newTotal = t - (has ? entry(k, dict[k]) : 0) + entry(k, v);/*@else*/const newTotal = t + entry(k, v);/*@end*/
  if (newTotal > SITE_STORE_MAX) return { error: { code: 4300, message: 'quota', data: { reason: 'quota', limit: SITE_STORE_MAX } } };
  if (!has && Object.keys(dict).length + 1 > SITE_ENTRIES_MAX) return { error: { code: 4300, message: 'entries', data: { reason: 'entries', limit: SITE_ENTRIES_MAX } } };
  next[k] = v; return { next };
}
// StoreQueue admission (D101): qbytes = 64 + utf8len(k) + utf8len(v) (null -> 0), clear = 64.
// Accepted iff session n+1 <= 8, session bytes <= 262144, global n+1 <= 64, global bytes <= 4194304
// (equality ok); otherwise -32005 {reason: store} with nothing appended. Counters cover every
// accepted message until release.
export const STORE_SESSION_MSGS_MAX = 8, STORE_SESSION_BYTES_MAX = 262144, STORE_GLOBAL_MSGS_MAX = 64, STORE_GLOBAL_BYTES_MAX = 4194304;
export function qbytes(method, params) { return method === 'site_storageClear' ? 64 : 64 + u8(params[0]) + (params[1] === null ? 0 : u8(params[1])); }
export class StoreAdmission {
  constructor(limits = {}) { this.l = { sm: STORE_SESSION_MSGS_MAX, sb: STORE_SESSION_BYTES_MAX, gm: STORE_GLOBAL_MSGS_MAX, gb: STORE_GLOBAL_BYTES_MAX, ...limits }; this.glob = { n: 0, bytes: 0 }; }
  session() { return { n: 0, bytes: 0 }; }
  admit(s, q) {
    if (s.n + 1 > this.l.sm || s.bytes + q > this.l.sb || this.glob.n + 1 > this.l.gm || this.glob.bytes + q > this.l.gb) return false;
    s.n++; s.bytes += q; this.glob.n++; this.glob.bytes += q; return true;
  }
  release(s, q) { s.n--; s.bytes -= q; this.glob.n--; this.glob.bytes -= q; }
}
