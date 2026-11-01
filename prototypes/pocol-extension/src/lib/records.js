// Site storage as non-replaceable versioned records with a worker-generation fence
// (browser.md D103, D104, D134; core subset, see development/lp9/LP9-EXTENSION.md).
//  * EpochFence: at boot list 'pocol:epoch:*', E = max+1; names gate EN+1 <= 8 (sweep after
//    T_LATE, retry once); generation window (< 4 epochs booted within T_LATE); write and confirm
//    'pocol:epoch:'+hex16(E) before any data write (<= 3 retries, same name).
//  * Records: 'site:'+netKey+':'+addr+':r:'+hex16(E)+':'+hex8(seq) = {fmt:2, epoch, seq, tomb, b64};
//    b64 = base64 of [be32 len k | k | be32 len v | v] in bytewise key order. Never overwritten;
//    one SeqAlloc per (key, generation); confirmed state = largest valid (epoch, seq).
//  * RecoveryGate per key: read the largest record, fall back on corrupt ones (<= 4 gets, 5 s);
//    then a checkpoint (E, s_c) with the recovered dictionary (one retry); then delete lower records.
//    A write from a dead generation has a version < (E, 0): it either became the recovered base
//    or stays below every later record (I57).
//  * Writes: per-key FIFO, <= 2 active sets globally, STORE_WRITE_DEADLINE 5 s (storeTimeout keeps
//    the key blocked until the promise settles), STORE_QUEUE_WAIT 10 s, disk/name admission (D104).
import { apply as quotaApply, total as quotaTotal, SITE_STORE_MAX, SITE_ENTRIES_MAX } from './sitestore.js';

const MiB = 1048576;
export const LIMITS = Object.freeze({
  EPOCH_NAMES_MAX: 8, GEN_WINDOW_MAX: 4, T_LATE_MS: 60000, STORE_EPOCH_RETRY: 3, RECOVERY_GETS_MAX: 4, RECOVERY_READ_DEADLINE_MS: 5000,
  STORE_RECOVERY_DEADLINE_MS: 5000, STORE_WRITE_DEADLINE_MS: 5000, STORE_QUEUE_WAIT_MS: 10000, STORE_ACTIVE_GLOBAL: 2,
  STORE_RECORDS_MAX: 1024, LATE_NAMES: 24, STORE_DISK_HARD: 256 * MiB, RECORD_BYTES_MAX: 1442048, META_RESERVE: MiB,
});
export const EPOCH_PREFIX = 'pocol:epoch:';
const hex = (n, w) => BigInt(n).toString(16).padStart(w, '0');
const te = new TextEncoder(), td = new TextDecoder('utf-8', { fatal: true });
const cmpBytes = (a, b) => { for (let i = 0; i < Math.min(a.length, b.length); i++) if (a[i] !== b[i]) return a[i] - b[i]; return a.length - b.length; };
const b64 = (u8) => { let s = ''; for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000)); return btoa(s); };
const unb64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

export function encodeDict(d) {
  const ks = Object.keys(d).map((k) => [te.encode(k), te.encode(d[k])]).sort((a, b) => cmpBytes(a[0], b[0]));
  const n = ks.reduce((a, [k, v]) => a + 8 + k.length + v.length, 0); const out = new Uint8Array(n); const dv = new DataView(out.buffer); let o = 0;
  for (const [k, v] of ks) { dv.setUint32(o, k.length); o += 4; out.set(k, o); o += k.length; dv.setUint32(o, v.length); o += 4; out.set(v, o); o += v.length; }
  return b64(out);
}
export function decodeDict(s) {
  const b = unb64(s); const dv = new DataView(b.buffer, b.byteOffset, b.byteLength); const d = Object.create(null); let o = 0, prev = null, n = 0;
  while (o < b.length) {
    if (o + 4 > b.length) throw new Error('trunc'); const kl = dv.getUint32(o); o += 4; if (o + kl + 4 > b.length) throw new Error('trunc');
    const k = b.subarray(o, o + kl); o += kl; const vl = dv.getUint32(o); o += 4; if (o + vl > b.length) throw new Error('trunc');
    const v = b.subarray(o, o + vl); o += vl;
    if (prev && cmpBytes(prev, k) >= 0) throw new Error('order'); prev = k; d[td.decode(k)] = td.decode(v); n++;
  }
  if (n > SITE_ENTRIES_MAX || quotaTotal(d) > SITE_STORE_MAX) throw new Error('bounds');
  return d;
}
const verCmp = (a, b) => (a.epoch !== b.epoch ? (a.epoch < b.epoch ? -1 : 1) : a.seq - b.seq);
function parseName(prefix, name) {
  const m = /^([0-9a-f]{16}):([0-9a-f]{8})$/.exec(name.slice(prefix.length)); if (!m) return null;
  return { name, epoch: BigInt('0x' + m[1]), seq: parseInt(m[2], 16) };
}
const deadline = (p, ms, timers) => new Promise((res) => { const t = timers.setTimeout(() => res({ timeout: true }), ms); p.then((v) => { timers.clearTimeout(t); res({ ok: true, v }); }, (e) => { timers.clearTimeout(t); res({ err: e }); }); });
const fail = (reason, extra = {}) => ({ error: { code: -32603, message: 'storeRecovery', data: { reason: 'storeRecovery', detail: reason, ...extra } } });

export class RecordStore {
  constructor(area, opts = {}) {
    this.area = area; this.L = { ...LIMITS, ...(opts.limits || {}) }; this.now = opts.now || (() => Date.now()); this.timers = opts.timers || globalThis;
    this.keys = new Map(); this.active = 0; this.activeQ = []; this.stats = { epochGateBlocks: 0, recoveryCorrupt: 0, lateRecordsSeen: 0 };
    this.epoch = null; this.bootFailed = null; this.bootMs = this.now();
    // DiskLedger (D105): measured {inUse, names} plus live reservations, checked and reserved synchronously.
    this.disk = { inUse: 0, names: 0, unres: 0, unresN: 0 };
  }
  async measure() {
    try { this.disk.names = (await this.area.getKeys()).length; this.disk.inUse = this.area.getBytesInUse ? await this.area.getBytesInUse(null) : 0; } catch { /* keep the last measurement */ }
  }
  sleep(ms) { return new Promise((r) => this.timers.setTimeout(r, ms)); }
  async listEpochs() { return (await this.area.getKeys()).filter((k) => k.startsWith(EPOCH_PREFIX)).map((k) => ({ name: k, e: BigInt('0x' + k.slice(EPOCH_PREFIX.length)) })); }
  async sweepEpochs() {
    const ep = await this.listEpochs(); if (!ep.length) return; const max = ep.reduce((a, x) => (x.e > a ? x.e : a), 0n);
    const old = ep.filter((x) => x.e < max).map((x) => x.name); if (old.length) await this.area.remove(old).catch(() => {});
  }
  /** EpochFence (D103 section 1, D134). Resolves with the generation's epoch or records bootFailed. */
  async boot() {
    let ep = await this.listEpochs();
    if (ep.length + 1 > this.L.EPOCH_NAMES_MAX) {
      this.stats.epochGateBlocks++; await this.sleep(Math.max(0, this.bootMs + this.L.T_LATE_MS - this.now())); await this.sweepEpochs(); ep = await this.listEpochs();
      if (ep.length + 1 > this.L.EPOCH_NAMES_MAX) { this.bootFailed = 'epochNames'; return null; }
    }
    for (let tries = 0; ; tries++) { // generation window: < GEN_WINDOW_MAX epochs booted within T_LATE
      const vals = ep.length ? await this.area.get(ep.map((x) => x.name)) : {};
      const recent = ep.filter((x) => vals[x.name] && typeof vals[x.name].bootMs === 'number' && vals[x.name].bootMs >= this.now() - this.L.T_LATE_MS).length;
      if (recent < this.L.GEN_WINDOW_MAX) break;
      if (tries > 0) { this.bootFailed = 'genWindow'; return null; }
      await this.sleep(this.L.T_LATE_MS); ep = await this.listEpochs();
    }
    const E = ep.reduce((a, x) => (x.e > a ? x.e : a), 0n) + 1n; const name = EPOCH_PREFIX + hex(E, 16);
    await this.measure();
    for (let i = 0; i <= this.L.STORE_EPOCH_RETRY; i++) {
      try { await this.area.set({ [name]: { bootMs: this.bootMs } }); this.epoch = E; break; } catch { /* retry with the same name */ }
    }
    if (this.epoch === null) { this.bootFailed = 'epochWrite'; return null; }
    this.timers.setTimeout(() => { this.sweepEpochs().catch(() => {}); }, this.L.T_LATE_MS);
    return E;
  }
  keyOf(netKey, site) { return 'site:' + netKey + ':' + site.toLowerCase(); }
  st(k) { let s = this.keys.get(k); if (!s) { s = { k, prefix: k + ':r:', state: 'unrecovered', dict: Object.create(null), seq: 0, names: new Set(), chain: Promise.resolve(), gate: null }; this.keys.set(k, s); } return s; }
  alloc(s) { if (s.seq > 0xffffffff) return null; return s.seq++; }
  recordName(s, seq) { return s.prefix + hex(this.epoch, 16) + ':' + hex(seq, 8); }
  async slot() { if (this.active < this.L.STORE_ACTIVE_GLOBAL) { this.active++; return; } await new Promise((r) => this.activeQ.push(r)); }
  unslot() { const n = this.activeQ.shift(); if (n) n(); else this.active--; }
  /** D104 admission for one record of encoded size enc; on success the reservation is taken in the same
   * synchronous step (no await between check and add), and released after the settle is re-measured. */
  admitDisk(enc) {
    const d = this.disk;
    if (d.names + d.unresN + 1 + this.L.LATE_NAMES + this.L.EPOCH_NAMES_MAX > this.L.STORE_RECORDS_MAX) return { refuse: { code: 4300, message: 'sites', data: { reason: 'sites', limit: this.L.STORE_RECORDS_MAX } } };
    if (d.inUse + d.unres + enc + 4 * 4 * this.L.RECORD_BYTES_MAX + this.L.META_RESERVE > this.L.STORE_DISK_HARD) return { refuse: { code: 4300, message: 'disk', data: { reason: 'disk', limit: this.L.STORE_DISK_HARD } } };
    d.unres += enc; d.unresN += 1;
    return { release: async () => { await this.measure(); d.unres -= enc; d.unresN -= 1; } };
  }
  /** Writes one record; resolves {ok}, {timeout, settled: Promise}, or {err}. */
  async putRecord(s, dict) {
    const seq = this.alloc(s); if (seq === null) return { err: 'seq' };
    const name = this.recordName(s, seq); const value = { fmt: 2, epoch: hex(this.epoch, 16), seq, tomb: false, b64: encodeDict(dict) };
    const enc = name.length + JSON.stringify(value).length; const adm = this.admitDisk(enc); if (adm.refuse) return { disk: adm.refuse };
    s.names.add(name); await this.slot();
    const p = Promise.resolve().then(() => this.area.set({ [name]: value })); const settled = p.finally(async () => { this.unslot(); await adm.release(); });
    const r = await deadline(p, this.L.STORE_WRITE_DEADLINE_MS, this.timers);
    if (r.ok) { await settled.catch(() => {}); return { ok: true, name, seq }; }
    if (r.timeout) return { timeout: true, name, seq, settled: settled.then(() => true, () => false) };
    await settled.catch(() => {}); s.names.delete(name); return { err: 'transport' };
  }
  async pruneBelow(s, keepName) {
    const keep = parseName(s.prefix, keepName); const drop = [...s.names].filter((n) => { const v = parseName(s.prefix, n); return v && verCmp(v, keep) < 0; });
    if (drop.length) { try { await this.area.remove(drop); for (const n of drop) s.names.delete(n); } catch { /* best effort; still counted on disk */ } await this.measure(); }
  }
  /** RecoveryGate (D104 section 3). */
  recover(s) {
    if (s.state === 'ready' || s.state === 'failed') return Promise.resolve();
    if (s.gate) return s.gate;
    s.state = 'recovering';
    s.gate = (async () => {
      const read = (async () => {
        const all = (await this.area.getKeys()).filter((n) => n.startsWith(s.prefix)).map((n) => parseName(s.prefix, n)).filter(Boolean).sort((a, b) => -verCmp(a, b));
        for (const x of all) s.names.add(x.name);
        let corrupt = 0;
        for (const x of all.slice(0, this.L.RECOVERY_GETS_MAX)) {
          const v = (await this.area.get(x.name))[x.name];
          try {
            if (!v || v.fmt !== 2 || v.epoch !== hex(x.epoch, 16) || v.seq !== x.seq || v.tomb !== false) throw new Error('shape');
            if (this.epoch !== null && x.epoch >= this.epoch) this.stats.lateRecordsSeen++;
            return { dict: decodeDict(v.b64) };
          } catch { corrupt++; this.stats.recoveryCorrupt++; }
        }
        return corrupt ? { corrupt: true } : { dict: Object.create(null) };
      })();
      const r = await deadline(read, this.L.RECOVERY_READ_DEADLINE_MS, this.timers);
      if (r.timeout || r.err) { s.state = 'unrecovered'; s.gate = null; throw fail(r.timeout ? 'readTimeout' : 'read'); }
      if (r.v.corrupt) { s.state = 'failed'; s.failReason = 'corrupt'; return; }
      // Checkpoint (E, s_c) with the recovered dictionary; one retry with a fresh seq.
      let why = 'checkpoint';
      for (let attempt = 0; attempt < 2; attempt++) {
        const w = await this.putRecord(s, r.v.dict);
        if (w.ok) { s.dict = r.v.dict; s.state = 'ready'; await this.pruneBelow(s, w.name); return; }
        if (w.disk) { why = w.disk.data.reason; continue; } // disk refusal: a failed attempt with nothing pending
        if (w.timeout) { const late = await w.settled; if (late) { s.dict = r.v.dict; s.state = 'ready'; await this.pruneBelow(s, w.name); return; } }
      }
      s.state = 'failed'; s.failReason = why;
    })().catch((e) => { if (e && e.error) throw e; s.state = 'unrecovered'; s.gate = null; throw fail('recovery'); });
    return s.gate;
  }
  async ready(s) {
    if (this.epoch === null) return fail(this.bootFailed || 'boot');
    try { await this.recover(s); } catch (e) { return e && e.error ? e : fail('recovery'); }
    if (s.state === 'failed') return fail(s.failReason || 'failed');
    return null;
  }
  /** Snapshot for a new frame: the confirmed dictionary after recovery. */
  async load(netKey, site) { const s = this.st(this.keyOf(netKey, site)); const e = await this.ready(s); return e ? { error: e.error, dict: Object.create(null) } : { dict: Object.assign(Object.create(null), s.dict) }; }
  /** storage_set after BridgeAuth and StoreQueue admission; FIFO per key. The reply can be sent
   * (storeTimeout) while the key's chain still waits for Chrome to settle the write. */
  write(netKey, site, method, params) { return this.submit(netKey, site, method, params).reply; }
  /** A12: {reply, settled}. `reply` answers the frame (possibly storeTimeout); `settled` resolves
   * only when the underlying storage operation has settled (never, for a write that never
   * settles), so ownership of admission follows the operation and not the reply. */
  submit(netKey, site, method, params) {
    const s = this.st(this.keyOf(netKey, site)); const accepted = this.now();
    let reply; const replied = new Promise((r) => { reply = r; });
    const job = s.chain.then(async () => {
      if (this.now() - accepted > this.L.STORE_QUEUE_WAIT_MS) return reply({ error: { code: -32005, message: 'store', data: { reason: 'store' } } });
      const e = await this.ready(s); if (e) return reply(e);
      const d = quotaApply(s.dict, method, params); if (d.error) return reply({ error: d.error });
      const w = await this.putRecord(s, d.next);
      if (w.disk) return reply({ error: w.disk });
      if (w.err) return reply({ error: { code: -32603, message: 'transport', data: { reason: w.err === 'seq' ? 'storeRecovery' : 'transport' } } });
      if (w.timeout) {
        reply({ error: { code: -32603, message: 'storeTimeout', data: { reason: 'storeTimeout' } } });
        if (await w.settled) { s.dict = d.next; await this.pruneBelow(s, w.name); }
        return;
      }
      s.dict = d.next; reply({ result: null }); await this.pruneBelow(s, w.name);
    }).catch(() => reply({ error: { code: -32603, message: 'transport', data: { reason: 'transport' } } }));
    s.chain = job;
    return { reply: replied, settled: job.then(() => true, () => true) };
  }
}
