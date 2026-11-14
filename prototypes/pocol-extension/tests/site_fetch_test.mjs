// Sites W2: the verified site fetch reads chunks through a chunk-level pool and retries idempotent reads
// (HTTP 429, -32021, timeouts, dropped connections) with bounded backoff; eth_sendRawTransaction is never
// retried. The real built transport, retry and site modules run against a mocked fetch; the clock is virtual
// (sleeping advances it), so no assertion depends on wall-clock timing.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { build } from '../build.mjs';
import { siteFixture } from './site_fixture.mjs';

// A private build: node --test runs suites in parallel, and others read dist/.
const { dist } = build({ out: mkdtempSync(join(tmpdir(), 'pocol-site-fetch-')) });
test.after(() => rmSync(dist, { recursive: true, force: true }));
const L = (m) => import(pathToFileURL(join(dist, 'lib', m)).href);
const T = await L('transport.js'), R = await L('retry.js'), S = await L('site.js'), H = await L('hex.js');

/** A virtual clock: sleep(ms) advances it and resolves on the next macrotask. */
function virtualClock() {
  const c = { now: 0, slept: [] };
  c.clock = () => c.now;
  c.sleep = (ms) => { c.slept.push(ms); c.now += ms; return new Promise((r) => setImmediate(r)); };
  return c;
}
const BIG = new Uint8Array(300000).map((_, i) => (i * 7 + 3) & 255); // 13 chunks
const FILES = { '/index.html': '<!doctype html><link rel="stylesheet" href="style.css"><h1>multi-chunk</h1>', '/style.css': 'body{color:red}', '/big.txt': BIG, '/same.txt': BIG.subarray(0, S.CHUNK_MAX) };

/** fetch mock over the fixture. `fault(n, method)` may return a Response/'drop'/null for the n-th eth_getCode. */
function endpoint(fx, fault = () => null) {
  const st = { inflight: 0, peak: 0, calls: [], getCode: 0 };
  st.fetchImpl = async (url, init) => {
    const req = JSON.parse(init.body); st.calls.push(req.method);
    st.inflight++; st.peak = Math.max(st.peak, st.inflight);
    try {
      await new Promise((r) => setImmediate(r));
      const n = req.method === 'eth_getCode' ? ++st.getCode : 0;
      const f = fault(n, req.method, req.params);
      if (f === 'drop') throw new TypeError('connection reset');
      if (f instanceof Response) return f;
      if (f && f.code) return new Response(JSON.stringify({ jsonrpc: '2.0', id: req.id, error: f }), { status: 200 });
      if (f && f.result !== undefined) return new Response(JSON.stringify({ jsonrpc: '2.0', id: req.id, result: f.result }), { status: 200 });
      const h = fx.handlers[req.method]; if (!h) return new Response(JSON.stringify({ jsonrpc: '2.0', id: req.id, error: { code: -32601, message: 'no' } }), { status: 200 });
      return new Response(JSON.stringify({ jsonrpc: '2.0', id: req.id, result: h(req.params) }), { status: 200 });
    } finally { st.inflight--; }
  };
  return st;
}
function rpcOver(st) {
  // As the worker's contentRpc: chunks from the content pool, everything else from the site pool.
  const content = new T.Pool(T.CONTENT_POOL), sitePool = new T.Pool(T.SITE_POOL); let id = 0;
  return (method, params) => T.request('http://gateway.test/studio/rpc', method, params, { pool: method === 'eth_getCode' ? content : sitePool, shared: T.inflight, id: ++id, fetchImpl: st.fetchImpl });
}
const busy429 = (after = '1') => new Response('{"error":"Request budget exceeded; retry shortly"}', { status: 429, headers: { 'retry-after': after } });
const busy32021 = { code: -32021, message: 'busy', data: { reason: 'busy', retryAfterMs: 250 } };
const open = async (st, fx, width, clock = virtualClock(), retry = {}) => S.fetchSite(rpcOver(st), fx.site, { parallel: width, retry: { clock: clock.clock, sleep: clock.sleep, random: () => 0.5, ...retry } });
const fixture = () => siteFixture(dist, FILES);

test('the width bound matches the content pool: CHUNK_WIDTH_MAX eth_getCode reservations fill CONTENT_POOL exactly', () => {
  assert.equal(S.CHUNK_WIDTH_MAX * T.RECV_LIMIT.get('eth_getCode'), T.CONTENT_POOL);
  assert.equal(S.chunkWidth('RN'), 2); for (const l of ['LN', 'DEV', 'RP']) assert.equal(S.chunkWidth(l), 8);
});
test('the retried reads are exactly the transport methods other than eth_sendRawTransaction', () => {
  assert.deepEqual([...R.IDEMPOTENT_READS].sort(), [...T.RECV_LIMIT.keys()].filter((m) => m !== 'eth_sendRawTransaction').sort());
});
test('a clean site opens with every file verified and each distinct chunk read once', async () => {
  const fx = await fixture(), st = endpoint(fx);
  const s = await open(st, fx, 8);
  assert.equal(s.entry, '/index.html'); assert.deepEqual([...s.files.keys()].sort(), Object.keys(FILES).sort());
  assert.deepEqual(s.files.get('/big.txt').data, BIG);
  assert.equal(st.getCode, 1 + 13 + 2); // manifest, 13 chunks of /big.txt, /index.html, /style.css; /same.txt reuses /big.txt's first chunk
});
for (const [width, label] of [[2, 'RN'], [8, 'LN/DEV']]) {
  test(`the chunk-level pool keeps at most ${width} reads in flight (${label}) and uses that width`, async () => {
    const fx = await fixture(), st = endpoint(fx);
    await open(st, fx, width);
    assert.equal(st.peak, width);
  });
}
test('the requested width is clamped to the content pool bound', async () => {
  const fx = await fixture(), st = endpoint(fx);
  await open(st, fx, 64); assert.ok(st.peak <= S.CHUNK_WIDTH_MAX, String(st.peak));
});
for (const [name, fault] of [
  ['HTTP 429 with Retry-After', () => busy429('2')],
  ['-32021 busy', () => busy32021],
  ['a dropped connection', () => 'drop'],
  ['HTTP 503 from the proxy', () => new Response('<html>bad gateway</html>', { status: 503 })],
]) {
  test(`the site opens when the first reads fail with ${name}`, async () => {
    const fx = await fixture(), c = virtualClock();
    const st = endpoint(fx, (n) => (n >= 1 && n <= 5 ? fault() : null));
    const s = await open(st, fx, 2, c);
    assert.deepEqual(s.files.get('/big.txt').data, BIG);
    assert.equal(st.getCode, 16 + 5);
    assert.ok(c.slept.length >= 1 && c.now < R.RETRY.deadlineMs, JSON.stringify(c.slept));
    if (name.startsWith('HTTP 429')) assert.ok(c.slept.every((ms) => ms >= 2000), 'Retry-After honoured: ' + c.slept);
  });
}
test('a timed-out read is retried (transport deadline) and the site opens', async () => {
  const fx = await fixture(); let first = true;
  const st = endpoint(fx);
  const inner = st.fetchImpl;
  st.fetchImpl = (u, init) => { if (first && JSON.parse(init.body).method === 'eth_getCode') { first = false; return new Promise((_, rej) => init.signal.addEventListener('abort', () => rej(new Error('aborted')))); } return inner(u, init); };
  const content = new T.Pool(T.CONTENT_POOL), sitePool = new T.Pool(T.SITE_POOL); let id = 0; const c = virtualClock();
  const rpc = (m, p) => T.request('http://gateway.test/', m, p, { pool: m === 'eth_getCode' ? content : sitePool, id: ++id, deadline: 20, fetchImpl: st.fetchImpl });
  const s = await S.fetchSite(rpc, fx.site, { parallel: 2, retry: { clock: c.clock, sleep: c.sleep, random: () => 0.5 } });
  assert.equal(s.files.size, 4); assert.equal(c.slept.length, 1);
});
test('a permanent 429 fails within the deadline with a clear error and bounded attempts', async () => {
  const fx = await fixture(), c = virtualClock();
  const st = endpoint(fx, (n, m) => (m === 'eth_getCode' ? busy429('10') : null));
  await assert.rejects(open(st, fx, 2, c), (e) => /could not be read: eth_getCode: the network endpoint is busy or unreachable \(rate\); gave up after \d attempts/.test(e.message) && e.transport === 'rate');
  assert.ok(c.now <= R.RETRY.deadlineMs, String(c.now));
  assert.ok(st.getCode <= 2 * R.RETRY.attempts, String(st.getCode)); // two workers, each gives up within RETRY.attempts
});
test('a tampered chunk served on the retry is still rejected', async () => {
  const fx = await fixture(), c = virtualClock();
  const evil = '0x00' + 'ee'.repeat(S.CHUNK_MAX);
  const st = endpoint(fx, (n) => (n === 2 ? busy429('0') : n === 3 ? { result: evil } : null));
  await assert.rejects(open(st, fx, 1, c), /not produced by the ChunkFactory|chunk length mismatch/);
});
test('a retried chunk read does not repeat the rest of the fetch (current() is read once)', async () => {
  const fx = await fixture(), c = virtualClock();
  const st = endpoint(fx, (n) => (n === 2 ? 'drop' : null));
  const real = fx.handlers.eth_call; let calls = 0;
  fx.handlers.eth_call = (p) => { calls++; return real(p); };
  const s = await open(st, fx, 1, c); assert.equal(calls, 1); assert.equal(s.files.size, 4);
});
test('a busy eth_sendRawTransaction is never retried; a busy read is', async () => {
  let sends = 0, reads = 0; const c = virtualClock();
  const base = async (m) => { if (m === 'eth_sendRawTransaction') { sends++; throw Object.assign(new Error('busy'), { rpc: busy32021 }); } reads++; if (reads < 3) throw Object.assign(new Error('rate'), { transport: 'rate', retryAfterMs: 0 }); return '0x1'; };
  const rpc = R.withRetry(base, { clock: c.clock, sleep: c.sleep, random: () => 0.5 });
  await assert.rejects(rpc('eth_sendRawTransaction', ['0x02']), (e) => e.rpc === busy32021 && !/gave up/.test(e.message));
  assert.equal(sends, 1);
  const timedOut = R.withRetry(async () => { sends++; throw Object.assign(new Error('transport'), { transport: 'transport' }); }, { clock: c.clock, sleep: c.sleep });
  await assert.rejects(timedOut('eth_sendRawTransaction', ['0x02']), /transport/); assert.equal(sends, 2);
  assert.equal(await rpc('eth_blockNumber', []), '0x1'); assert.equal(reads, 3);
});
test('final errors are not retried: remote errors other than -32021, response guard failures, cancellation, unsupported', async () => {
  for (const e of [{ rpc: { code: -32000, message: 'execution reverted' } }, { transport: 'recvLimit' }, { transport: 'recvParse' }, { transport: 'cancelled' }, { transport: 'unsupported' }, new Error('plain')]) {
    let n = 0; const rpc = R.withRetry(async () => { n++; throw Object.assign(new Error('x'), e); }, { sleep: async () => {} });
    await assert.rejects(rpc('eth_call', [])); assert.equal(n, 1, JSON.stringify(e));
  }
});
test('local back-pressure (busy) is retried only when asked (the chunk fetch), never for a site session', async () => {
  let n = 0; const busy = async () => { n++; throw Object.assign(new Error('busy'), { transport: 'busy' }); };
  await assert.rejects(R.withRetry(busy, { sleep: async () => {} })('eth_call', [])); assert.equal(n, 1);
  n = 0; const c = virtualClock();
  await assert.rejects(R.withRetry(busy, { retryBusy: true, clock: c.clock, sleep: c.sleep })('eth_getCode', []), /gave up after 6 attempts/); assert.equal(n, R.RETRY.attempts);
});
test('backoff is exponential with jitter, honours a server hint up to the cap, and stops at the total deadline', async () => {
  const c = virtualClock(); let n = 0;
  const rpc = R.withRetry(async () => { n++; throw Object.assign(new Error('x'), { rpc: { code: -32021, message: 'rate', data: { reason: 'rate', retryAfterMs: 999999 } } }); }, { clock: c.clock, sleep: c.sleep, random: () => 0 });
  await assert.rejects(rpc('eth_getBalance', []), (e) => e.rpc.code === -32021 && e.attempts === n);
  assert.ok(c.slept.every((ms) => ms === R.RETRY.hintMaxMs), JSON.stringify(c.slept)); // the 999999 ms hint is capped
  assert.ok(c.now <= R.RETRY.deadlineMs);
  const d = virtualClock(); n = 0;
  await assert.rejects(R.withRetry(async () => { n++; throw Object.assign(new Error('t'), { transport: 'transport' }); }, { clock: d.clock, sleep: d.sleep, random: () => 0 })('eth_call', []));
  assert.deepEqual(d.slept, [125, 250, 500, 1000, 2000]); assert.equal(n, 6);
  const e = virtualClock();
  await assert.rejects(R.withRetry(async () => { throw Object.assign(new Error('t'), { transport: 'transport' }); }, { clock: e.clock, sleep: e.sleep, random: () => 1 })('eth_call', []));
  assert.deepEqual(e.slept, [250, 500, 1000, 2000, 4000]);
});
test('an abort stops the retry loop', async () => {
  let n = 0, stop = false; const c = virtualClock();
  const rpc = R.withRetry(async () => { n++; stop = true; throw Object.assign(new Error('t'), { transport: 'rate' }); }, { clock: c.clock, sleep: c.sleep, aborted: () => stop });
  await assert.rejects(rpc('eth_call', []), /gave up after 1 attempt\b/); assert.equal(n, 1);
});
test('wrapping a retrying function again does not multiply attempts', async () => {
  let n = 0; const c = virtualClock();
  const once = R.withRetry(async () => { n++; throw Object.assign(new Error('t'), { transport: 'transport' }); }, { clock: c.clock, sleep: c.sleep });
  assert.equal(R.withRetry(once), once);
  await assert.rejects(R.withRetry(once)('eth_call', [])); assert.equal(n, R.RETRY.attempts);
});
test('transport classifies 429 as rate with Retry-After (delta-seconds, capped), 502/503/504 as transport, any other status as status', async () => {
  const pool = new T.Pool(T.TRUST_POOL); const run = (res) => T.request('http://x/', 'eth_blockNumber', [], { pool, id: 1, fetchImpl: async () => res }).then((v) => ({ v }), (e) => e);
  let e = await run(busy429('2')); assert.equal(e.transport, 'rate'); assert.equal(e.retryAfterMs, 2000); assert.equal(e.status, 429);
  e = await run(busy429('3600')); assert.equal(e.retryAfterMs, T.RETRY_AFTER_MAX_MS);
  e = await run(busy429('Wed, 21 Oct 2015 07:28:00 GMT')); assert.equal(e.transport, 'rate'); assert.equal(e.retryAfterMs, 0);
  e = await run(new Response('', { status: 429 })); assert.equal(e.retryAfterMs, 0);
  for (const s of [502, 503, 504]) { e = await run(new Response('<html></html>', { status: s })); assert.equal(e.transport, 'transport'); }
  for (const s of [201, 204, 301, 400, 403, 404, 413, 415, 500]) {
    e = await run(new Response(s === 204 ? null : '{"jsonrpc":"2.0","id":1,"result":"0x1"}', { status: s })); assert.equal(e.transport, 'status', String(s)); assert.equal(e.status, s);
  }
  e = await run(new Response('<html>not json</html>', { status: 404 })); assert.equal(e.transport, 'status'); // classified before the body is read
  assert.equal(R.retryHint({ transport: 'status', status: 400 }), null); assert.equal(R.retryHint({ transport: 'transport', status: 500 }), null);
  assert.equal(R.retryHint({ transport: 'transport', status: 503, retryAfterMs: 5 }), 5); assert.equal(R.retryHint({ transport: 'transport' }), 0);
  assert.equal(R.retryHint({ transport: 'rate', retryAfterMs: 2000 }), 2000); assert.equal(R.retryHint({ transport: 'recvLimit' }), null);
  assert.equal(pool.free, pool.cap);
});
// The Studio gateway answers 400 for a request it refuses, 404/415 for a wrong path or content type, and 413
// 'Response limit exceeded' only after it charged the client's budget and ran the call: a retry would only spend
// that budget again, so every status other than 429 and 502/503/504 is final.
test('HTTP 400, 403, 404, 413, 415 and 500 answers are final: exactly one attempt, through every retrying wrapper', async () => {
  for (const status of [400, 403, 404, 413, 415, 500]) {
    let n = 0; const c = virtualClock(); const pool = new T.Pool(T.SITE_POOL);
    const fetchImpl = async () => { n++; return new Response(JSON.stringify({ error: 'refused' }), { status, headers: { 'content-type': 'application/json' } }); };
    const rpc = R.withRetry((m, p) => T.request('http://gateway.test/studio/rpc', m, p, { pool, id: 1, fetchImpl }), { clock: c.clock, sleep: c.sleep });
    await assert.rejects(rpc('eth_call', [{}, 'latest']), (e) => e.transport === 'status' && e.status === status && !/gave up/.test(e.message));
    assert.equal(n, 1, String(status)); assert.deepEqual(c.slept, []); assert.equal(pool.free, pool.cap);
    // The verified site fetch (which also retries local back-pressure) gives up at once as well.
    const fx = await fixture(), st = endpoint(fx, (k, m) => (m === 'eth_getCode' ? new Response('{"error":"Response limit exceeded"}', { status }) : null));
    await assert.rejects(open(st, fx, 2, c), (e) => e.transport === 'status' && /could not be read/.test(e.message));
    assert.ok(st.getCode <= 2, `${status}: ${st.getCode}`); // at most one read per worker, none repeated
  }
});
test('the total deadline bounds the attempts too: a retry starts only if it can end, its own timeout included, by the deadline', async () => {
  // Each attempt runs to its timeout (10 s unless shortened) and then fails as a timeout ('transport').
  const slow = (c, seen) => async (m, p, timeoutMs) => { seen.push(timeoutMs ?? 'default'); c.now += timeoutMs ?? 10000; throw Object.assign(new Error('transport'), { transport: 'transport' }); };
  const budget = { attemptMs: (m, p, t = 10000) => 2000 + t, minAttemptMs: 3000 };
  // Without a budget, the check sees only the wait, and the last attempt ends past the deadline (the reported 67,750 ms).
  let c = virtualClock(), seen = [];
  await assert.rejects(R.withRetry(slow(c, seen), { clock: c.clock, sleep: c.sleep, random: () => 1 })('eth_call', []), /gave up after 6 attempts/);
  assert.equal(c.now, 67750);
  // With attemptMs, no retry starts that could end past 60 s.
  c = virtualClock(); seen = [];
  await assert.rejects(R.withRetry(slow(c, seen), { ...budget, clock: c.clock, sleep: c.sleep, random: () => 1 })('eth_call', []), /gave up after 5 attempts/);
  assert.equal(c.now, 53750); assert.ok(c.now <= R.RETRY.deadlineMs); assert.equal(seen.length, 5);
  // With shorten, the last attempt is bounded to the time left (6,125 ms minus the 2 s pool wait).
  c = virtualClock(); seen = [];
  await assert.rejects(R.withRetry(slow(c, seen), { ...budget, shorten: ([, stages], ms) => [ms - 2000, stages], clock: c.clock, sleep: c.sleep, random: () => 0 })('eth_call', []), /gave up after 6 attempts/);
  assert.deepEqual(seen, ['default', 'default', 'default', 'default', 'default', 4125]); assert.equal(c.now, 58000); assert.ok(c.now + 2000 <= R.RETRY.deadlineMs);
  // Too little left for even a shortened attempt (minAttemptMs): the loop gives up instead.
  c = virtualClock(); seen = [];
  await assert.rejects(R.withRetry(slow(c, seen), { ...budget, minAttemptMs: 7000, shorten: ([, stages], ms) => [ms - 2000, stages], clock: c.clock, sleep: c.sleep, random: () => 0 })('eth_call', []), /gave up after 5 attempts/);
  assert.equal(seen.length, 5);
});
test('the site fetch keeps the manifest checks: a wrong manifest hash is refused after retries', async () => {
  const fx = await fixture(), c = virtualClock();
  const cur = H.fromHex(fx.current); cur[40] ^= 1; const bad = H.toHex(cur);
  fx.handlers.eth_call = () => bad;
  const st = endpoint(fx, (n) => (n === 1 ? busy429('0') : null));
  await assert.rejects(open(st, fx, 2, c), /manifest hash\/length mismatch/);
});
test('the service worker opens a site whose chunk reads are first refused (-32021) and retries a busy site read', async () => {
  const { installChromeMock, installFetchMock, loadWorker } = await import('./chrome_mock.mjs');
  const { readFileSync } = await import('node:fs');
  const C = await L('crypto.js'), RP = await L('rp.js');
  const pre = '0x' + readFileSync(new URL('./data/devnet-spec.hex', import.meta.url), 'utf8').trim().replace(/^0x/, '');
  const spec = RP.decodeSpec(H.fromHex(pre)), hash = H.toHex(C.keccak(H.fromHex(pre)));
  const fx = await fixture(); let refusedCode = 0, refusedRead = 0;
  // The session's 30 s identity check is captured, not scheduled (as worker_test does), so the suite can exit.
  // The record store's 60 s epoch sweep is unref'd for the same reason.
  const realSI = globalThis.setInterval, realST = globalThis.setTimeout;
  globalThis.setInterval = (f, ms, ...a) => (ms === 30000 ? { captured: f } : realSI(f, ms, ...a));
  globalThis.setTimeout = (f, ms, ...a) => { const t = realST(f, ms, ...a); if (ms >= 30000) t.unref(); return t; };
  test.after(() => { globalThis.setInterval = realSI; globalThis.setTimeout = realST; });
  const mock = installChromeMock();
  const log = installFetchMock({ ...fx.handlers,
    eth_getCode: (p) => (refusedCode++ < 2 ? { __error: { code: -32021, message: 'busy', data: { reason: 'busy', retryAfterMs: 1 } } } : fx.handlers.eth_getCode(p)),
    eth_blockNumber: () => (refusedRead++ < 1 ? { __error: { code: -32021, message: 'rate', data: { reason: 'rate', retryAfterMs: 1 } } } : '0x2a'),
    eth_chainId: () => '0x' + spec.chainId.toString(16), pocol_status: () => ({ genesisHash: hash }) });
  await loadWorker(dist, 'site-fetch-retry');
  const added = await mock.send({ type: 'profiles.add', profile: { name: 'p1', chainId: Number(spec.chainId), genesisPre: pre, genesisHash: hash, endpoint: 'http://127.0.0.1:8599', trustLevel: 'LN' } });
  assert.equal(added.ok, true, JSON.stringify(added));
  const op = await mock.send({ type: 'open', name: 'p1', site: fx.site, path: '/' });
  assert.equal(op.ok, true, JSON.stringify(op));
  assert.equal(log.filter((x) => x.method === 'eth_getCode').length, 16 + 2);
  const port = mock.connect('bridge:' + op.sessionId);
  port.post({ seq: 1, frame: 1, text: JSON.stringify({ id: 1, kind: 'rpc_read', payload: { method: 'eth_blockNumber', params: [] } }) });
  const r = (await port.next((m) => m.seq === 1)).reply;
  assert.equal(r.result, '0x2a', JSON.stringify(r)); assert.equal(refusedRead, 2);
});
test('a closed or revoked site session stops retrying at once: no further read is sent and its pending-read slots return', async () => {
  const { installChromeMock, installFetchMock, loadWorker } = await import('./chrome_mock.mjs');
  const { readFileSync } = await import('node:fs');
  const C = await L('crypto.js'), RP = await L('rp.js'), B = await L('bridge.js');
  const pre = '0x' + readFileSync(new URL('./data/devnet-spec.hex', import.meta.url), 'utf8').trim().replace(/^0x/, '');
  const spec = RP.decodeSpec(H.fromHex(pre)), hash = H.toHex(C.keccak(H.fromHex(pre)));
  const fx = await fixture(); let busy = true;
  // The retry sleeps (the -32021 hint, HINT ms) are captured, not scheduled: the test decides when each one ends.
  // The 30 s identity checks are captured and long timers unref'd, as in the test above, so the suite can exit.
  const HINT = 4321, sleeps = [];
  const prevSI = globalThis.setInterval, prevST = globalThis.setTimeout;
  globalThis.setInterval = (f, ms, ...a) => (ms === 30000 ? { captured: f } : prevSI(f, ms, ...a));
  globalThis.setTimeout = (f, ms, ...a) => { if (ms === HINT) { sleeps.push(f); return { captured: f }; } const t = prevST(f, ms, ...a); if (ms >= 30000 && t.unref) t.unref(); return t; };
  try {
    const mock = installChromeMock();
    const log = installFetchMock({ ...fx.handlers,
      eth_blockNumber: () => (busy ? { __error: { code: -32021, message: 'busy', data: { reason: 'busy', retryAfterMs: HINT } } } : '0x2a'),
      eth_chainId: () => '0x' + spec.chainId.toString(16), pocol_status: () => ({ genesisHash: hash }) });
    await loadWorker(dist, 'site-session-abort');
    const EP = { p1: 'http://127.0.0.1:8597', p2: 'http://127.0.0.1:8596' };
    for (const name of ['p1', 'p2']) {
      const added = await mock.send({ type: 'profiles.add', profile: { name, chainId: Number(spec.chainId), genesisPre: pre, genesisHash: hash, endpoint: EP[name], trustLevel: 'LN' } });
      assert.equal(added.ok, true, JSON.stringify(added));
    }
    const sessions = [];
    for (let i = 0; i < 8; i++) {
      const tab = 40 + i, op = await mock.send({ type: 'open', name: i < 4 ? 'p1' : 'p2', site: fx.site, path: '/' }, tab);
      assert.equal(op.ok, true, JSON.stringify(op));
      sessions.push({ tab, port: mock.connect('bridge:' + op.sessionId, tab), seq: 0 });
    }
    const read = (s) => { const seq = ++s.seq; s.port.post({ seq, frame: 1, text: JSON.stringify({ id: seq, kind: 'rpc_read', payload: { method: 'eth_blockNumber', params: [] } }) }); return seq; };
    const reads = (url) => log.filter((x) => x.method === 'eth_blockNumber' && x.url === url).length;
    const settle = async (pred) => { for (let i = 0; i < 1000 && !pred(); i++) await new Promise((r) => setImmediate(r)); assert.ok(pred()); };
    // Sessions 1-4 (profile p1) each fill their PENDING_READS_MAX; together they hold every global slot.
    assert.equal(4 * B.PENDING_READS_MAX, B.GLOBAL_PENDING_READS_MAX);
    for (const s of sessions.slice(0, 4)) for (let k = 0; k < B.PENDING_READS_MAX; k++) read(s);
    await settle(() => sleeps.length === 16);
    assert.equal(reads(EP.p1), 16);
    const refused = read(sessions[4]);
    assert.equal((await sessions[4].port.next((m) => m.seq === refused)).reply.error.data.reason, 'pending');
    // Two tabs close; the profile of the other two is removed (they are revoked with 4901). All of it during the sleeps.
    sessions[0].port.close(); sessions[1].port.close();
    assert.equal((await mock.send({ type: 'profiles.remove', name: 'p1' })).ok, true);
    await sessions[2].port.next((m) => m.revoked && m.revoked.code === 4901); await sessions[3].port.next((m) => m.revoked && m.revoked.code === 4901);
    // The sleeps end: every one of the 16 loops stops without sending, and releases its slots.
    for (const f of sleeps.splice(0)) f();
    await settle(() => true); for (let i = 0; i < 20; i++) await new Promise((r) => setImmediate(r));
    assert.equal(reads(EP.p1), 16, 'nothing more is sent to the closed or revoked sessions\' endpoint');
    assert.equal(sleeps.length, 0);
    // The counters are back at 0: sessions 5-8 (profile p2) are again admitted for all 16 global slots.
    const later = sessions.slice(4).map((s) => Array.from({ length: B.PENDING_READS_MAX }, () => read(s)));
    await settle(() => sleeps.length === 16);
    assert.ok(sessions.slice(4).every((s) => !s.port.inbox.some((m) => m.reply && m.reply.error)), 'no read was refused as pending');
    // The gateway recovers: the retried reads of the live sessions now succeed.
    busy = false; for (const f of sleeps.splice(0)) f();
    for (const [i, s] of sessions.slice(4).entries()) for (const seq of later[i]) assert.equal((await s.port.next((m) => m.seq === seq)).reply.result, '0x2a');
    assert.equal(reads(EP.p2), 32);
  } finally { globalThis.setInterval = prevSI; globalThis.setTimeout = prevST; }
});
