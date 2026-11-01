// BR22-style unit gates for RecordStore (D103/D104/D134 core) with a FakeBackend that
// can hold set() promises, so a write from a dead worker generation lands late.
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

class FakeArea {
  constructor() { this.data = new Map(); this.holdNext = 0; this.held = []; this.sets = 0; this.failNext = 0; }
  async get(keys) { const ks = keys === null ? [...this.data.keys()] : Array.isArray(keys) ? keys : [keys]; const o = {}; for (const k of ks) if (this.data.has(k)) o[k] = structuredClone(this.data.get(k)); return o; }
  set(obj) {
    this.sets++;
    if (this.failNext > 0) { this.failNext--; return Promise.reject(new Error('io')); }
    const apply = () => { for (const [k, v] of Object.entries(obj)) this.data.set(k, structuredClone(v)); };
    if (this.holdNext > 0) { this.holdNext--; return new Promise((res) => this.held.push(() => { apply(); res(); })); }
    apply(); return Promise.resolve();
  }
  async remove(keys) { for (const k of Array.isArray(keys) ? keys : [keys]) this.data.delete(k); }
  async getKeys() { return [...this.data.keys()]; }
  async getBytesInUse() { let n = 0; for (const [k, v] of this.data) n += k.length + JSON.stringify(v).length; return n; }
  release() { const h = this.held.shift(); if (h) h(); }
}
const tick = (ms = 5) => new Promise((r) => setTimeout(r, ms));

export async function recordChecks(dist) {
  const R = await import(pathToFileURL(join(dist, 'lib', 'records.js')).href);
  const checks = []; const ok = (n, c, d = '') => checks.push({ name: n, pass: !!c, detail: d });
  const NK = '0x' + 'ab'.repeat(32), SITE = '0x' + '12'.repeat(20), K = 'site:' + NK + ':' + SITE;
  const fast = { T_LATE_MS: 30, STORE_WRITE_DEADLINE_MS: 40, RECOVERY_READ_DEADLINE_MS: 200, STORE_QUEUE_WAIT_MS: 10000 };
  const recs = (area) => [...area.data.keys()].filter((k) => k.startsWith(K + ':r:')).sort();
  const set = (st, k, v) => st.write(NK, SITE, 'site_storageSet', [k, v]);

  // Format, single surviving record, roundtrip.
  { const area = new FakeArea(); const g = new R.RecordStore(area, { limits: fast }); ok('EpochFence: first generation is E=1 and its epoch item is confirmed', (await g.boot()) === 1n && area.data.has(R.EPOCH_PREFIX + '0000000000000001'));
    const r = await set(g, 'k', 'v'); const names = recs(area); const v = area.data.get(names[names.length - 1]);
    ok('record name and value format {fmt:2, epoch, seq, tomb, b64}; result null', r.result === null && /:r:0000000000000001:0000000[0-9a-f]$/.test(names[0]) && v.fmt === 2 && v.tomb === false && R.decodeDict(v.b64).k === 'v', names.join(','));
    ok('lower records deleted after a newer one commits (one record remains)', names.length === 1);
    const d = { b: '2', a: '1', é: 'x' }; ok('dict encoding: bytewise key order, strict decode', JSON.stringify(R.decodeDict(R.encodeDict(d))) === JSON.stringify({ a: '1', b: '2', é: 'x' })); }

  // I57: a write of a dead generation lands after the next generation's write and never wins.
  { const area = new FakeArea(); const g1 = new R.RecordStore(area, { limits: fast }); await g1.boot(); await set(g1, 'k', 'a');
    area.holdNext = 1; const late = set(g1, 'k', 'b'); await tick(60); // g1 dies with the set still pending in Chrome
    const g2 = new R.RecordStore(area, { limits: fast }); ok('next generation fences with E=2', (await g2.boot()) === 2n);
    const l2 = await g2.load(NK, SITE); ok('g2 recovers the last committed value (late write not yet applied)', l2.dict.k === 'a');
    await set(g2, 'k', 'c'); area.release(); await late.catch(() => {}); await tick(20);
    const g3 = new R.RecordStore(area, { limits: fast }); await g3.boot(); const l3 = await g3.load(NK, SITE);
    ok('I57: the dead generation\'s late write (1, s) stays below (2, s\'): g3 sees c', l3.dict.k === 'c', JSON.stringify(l3.dict)); }

  // Late write applied before the next generation's recovery becomes its base.
  { const area = new FakeArea(); const g1 = new R.RecordStore(area, { limits: fast }); await g1.boot(); await set(g1, 'k', 'a');
    area.holdNext = 1; const late = set(g1, 'k', 'b'); await tick(60); area.release(); await late.catch(() => {});
    const g2 = new R.RecordStore(area, { limits: fast }); await g2.boot(); ok('late write applied before recovery is the recovered base', (await g2.load(NK, SITE)).dict.k === 'b'); }

  // Corrupt newest record skipped; all-corrupt -> failed.
  { const area = new FakeArea(); const g1 = new R.RecordStore(area, { limits: fast }); await g1.boot(); await set(g1, 'k', 'a');
    area.data.set(K + ':r:0000000000000001:000000ff', { fmt: 2, epoch: '0000000000000001', seq: 255, tomb: false, b64: '!!!' });
    const g2 = new R.RecordStore(area, { limits: fast }); await g2.boot(); const l = await g2.load(NK, SITE);
    ok('corrupt newest record skipped, next valid recovered, counted', l.dict.k === 'a' && g2.stats.recoveryCorrupt === 1); }
  { const area = new FakeArea(); const g1 = new R.RecordStore(area, { limits: fast }); await g1.boot();
    for (let i = 0; i < 5; i++) area.data.set(K + ':r:0000000000000001:0000010' + i, { fmt: 2, epoch: '0000000000000001', seq: 256 + i, tomb: false, b64: 'x' });
    const r = await set(g1, 'k', 'v');
    ok('no valid record within 4 gets -> key failed, -32603 {storeRecovery, corrupt}, records kept', r.error && r.error.code === -32603 && r.error.data.detail === 'corrupt' && recs(area).length === 5); }

  // Epoch names gate and sweep (D134).
  { const area = new FakeArea(); for (let e = 1; e <= 8; e++) area.data.set(R.EPOCH_PREFIX + e.toString(16).padStart(16, '0'), { bootMs: 0 });
    const g = new R.RecordStore(area, { limits: fast }); const E = await g.boot(); const names = [...area.data.keys()].filter((k) => k.startsWith(R.EPOCH_PREFIX));
    ok('names gate: 8 epoch items block, sweep after T_LATE keeps the largest, E = 9', E === 9n && g.stats.epochGateBlocks === 1 && names.length === 2, names.join(',')); }
  { const area = new FakeArea(); const now = Date.now(); for (let e = 1; e <= 4; e++) area.data.set(R.EPOCH_PREFIX + e.toString(16).padStart(16, '0'), { bootMs: now + 3600000 });
    const g = new R.RecordStore(area, { limits: fast }); const E = await g.boot(); const r = await set(g, 'k', 'v');
    ok('generation window: 4 recent epochs (clock ahead) -> no fence, storage_set -32603 storeRecovery', E === null && g.bootFailed === 'genWindow' && r.error.data.reason === 'storeRecovery'); }
  { const area = new FakeArea(); area.failNext = 4; const g = new R.RecordStore(area, { limits: fast }); const E = await g.boot();
    ok('epoch write fails 1 + 3 retries -> no data writes in this generation', E === null && g.bootFailed === 'epochWrite'); }

  // STORE_WRITE_DEADLINE: storeTimeout keeps the key blocked; the next write waits for settle.
  { const area = new FakeArea(); const g = new R.RecordStore(area, { limits: fast }); await g.boot(); await g.load(NK, SITE);
    area.holdNext = 1; const w1 = set(g, 'k', 'x'); const w2 = set(g, 'k', 'y');
    const r1 = await w1; const setsAfterTimeout = area.sets; await tick(60);
    ok('storeTimeout reply after the deadline; the next write has not started', r1.error && r1.error.data.reason === 'storeTimeout' && area.sets === setsAfterTimeout);
    area.release(); const r2 = await w2; const g2 = new R.RecordStore(area, { limits: fast }); await g2.boot();
    ok('after settle the queued write runs; recovered state is the last write', r2.result === null && (await g2.load(NK, SITE)).dict.k === 'y'); }
  // STORE_QUEUE_WAIT and quota.
  { const area = new FakeArea(); const g = new R.RecordStore(area, { limits: { ...fast, STORE_QUEUE_WAIT_MS: 30 } }); await g.boot(); await g.load(NK, SITE);
    area.holdNext = 1; const w1 = set(g, 'k', 'x'); const w2 = set(g, 'k', 'y'); await w1; await tick(80); area.release(); const r2 = await w2;
    ok('STORE_QUEUE_WAIT: a message that waited past 10 s (scaled) -> -32005 {store}, no write', r2.error && r2.error.code === -32005 && r2.error.data.reason === 'store'); }
  { const area = new FakeArea(); const g = new R.RecordStore(area, { limits: fast }); await g.boot(); await set(g, 'k', 'a'); const before = area.sets;
    const r = await set(g, 'big', 'a'.repeat(61440)); const r2 = await g.write(NK, SITE, 'site_storageSet', ['k2', 'b']);
    ok('quota decision before any write (accepted writes call set once each)', r.result === null && r2.result === null && area.sets === before + 2);
    for (let i = 0; i < 16; i++) await set(g, 'f' + i, 'a'.repeat(61440)); const sets = area.sets; const q = await set(g, 'over', 'a'.repeat(61440));
    ok('4300 quota: zero set calls', q.error && q.error.code === 4300 && area.sets === sets); }
  { const area = new FakeArea(); const g = new R.RecordStore(area, { limits: { ...fast, STORE_RECORDS_MAX: 72 } }); await g.boot(); await set(g, 'k', 'v');
    for (let i = 0; i < 40; i++) area.data.set('other:' + i, 1); await g.measure(); const sets = area.sets; const r = await set(g, 'k', 'w');
    ok('D104 names admission: 4300 {sites} when names + 1 + 24 + 8 > STORE_RECORDS_MAX, no set', r.error && r.error.code === 4300 && r.error.data.reason === 'sites' && area.sets === sets, JSON.stringify(r)); }
  { const area = new FakeArea(); for (let i = 0; i < 40; i++) area.data.set('other:' + i, 1); const g = new R.RecordStore(area, { limits: { ...fast, STORE_RECORDS_MAX: 72 } }); await g.boot();
    const r = await set(g, 'k', 'v'); ok('checkpoint refused by the disk twice -> key failed, -32603 {storeRecovery}', r.error && r.error.code === -32603 && r.error.data.detail === 'sites', JSON.stringify(r)); }
  // D105: two writes in flight see each other's reservation (synchronous check-and-add).
  { const area = new FakeArea(); const g = new R.RecordStore(area, { limits: { ...fast, STORE_RECORDS_MAX: 36, STORE_WRITE_DEADLINE_MS: 2000 } }); await g.boot(); await g.load(NK, SITE);
    const SITE2 = '0x' + '34'.repeat(20); await g.load(NK, SITE2); await g.measure();
    area.holdNext = 2; const w1 = g.write(NK, SITE, 'site_storageSet', ['a', '1']); const w2 = g.write(NK, SITE2, 'site_storageSet', ['b', '2']);
    await tick(30); const inflight = g.disk.unresN; area.release(); area.release(); const r = [await w1, await w2];
    ok('D105 DiskLedger: concurrent writes reserve names synchronously; the one over the cap is refused', inflight === 1 && r.filter((x) => x.result === null).length === 1 && r.some((x) => x.error && x.error.data.reason === 'sites'), JSON.stringify({ inflight, r }));
    ok('D105 reservations released after settle', g.disk.unres === 0 && g.disk.unresN === 0); }
  // '__proto__' is an ordinary storage key (null-prototype dictionaries).
  { const area = new FakeArea(); const g = new R.RecordStore(area, { limits: fast }); await g.boot(); const r = await set(g, '__proto__', 'p');
    const g2 = new R.RecordStore(area, { limits: fast }); await g2.boot(); const d = (await g2.load(NK, SITE)).dict;
    ok("'__proto__' key stored, counted and recovered; no prototype pollution", r.result === null && Object.prototype.hasOwnProperty.call(d, '__proto__') && d['__proto__'] === 'p' && ({}).p === undefined); }
  return checks;
}
