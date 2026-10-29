// HttpTransport with RecvGuard (browser.md D98/D100): every network request of the
// extension reserves recvLimit bytes from a pool before sending (FIFO, busy after 2 s),
// refuses Content-Length > recvLimit before reading, counts streamed bytes after
// Chrome's decompression and aborts at the first chunk over the limit, then decodes
// UTF-8 fatally and parses strictly (depth <= 16, duplicate keys rejected). No partial
// content ever reaches a frame. Remote error text: message <= 256 UTF-16 units,
// data passed only when its encoding is <= 4 KiB.
import { parseStrict } from './bridge.js';

const KiB = 1024, MiB = 1024 * 1024;
// RECV_LIMIT per method (D100). exact: >= WorstLegit from pocold; capped: deliberately narrower.
export const RECV_LIMIT = Object.freeze(new Map([
  ...['eth_chainId', 'net_version', 'web3_clientVersion', 'eth_blockNumber', 'eth_gasPrice', 'eth_maxPriorityFeePerGas', 'eth_getBalance', 'eth_getStorageAt', 'eth_getTransactionCount', 'eth_sendRawTransaction'].map((m) => [m, 4096]),
  ['eth_getCode', 68 * KiB], ['pocol_getHeaders', 96 * KiB], ['pocol_status', 96 * KiB],
  ['eth_getBlockByNumber', 164 * KiB], ['eth_getBlockByHash', 164 * KiB], ['eth_feeHistory', 176 * KiB],
  ['eth_getTransactionByHash', 260 * KiB], ['eth_getTransactionReceipt', 340 * KiB],
  ['eth_call', 1 * MiB], ['eth_estimateGas', 64 * KiB], ['pocol_getLogs', 8 * MiB],
]));
export const SITE_POOL = 32 * MiB, TRUST_POOL = 2 * MiB, CONTENT_POOL = 8 * 68 * KiB, SESSION_RECV_MAX = 10 * MiB, RECV_INFLIGHT_MAX = 48, RESERVE_WAIT_MS = 2000;
export const RESP_DEPTH_MAX = 16, ERR_MESSAGE_MAX = 256, ERR_DATA_MAX = 4096;

/** Byte pool with FIFO reservation; reserve resolves true (granted), false after waitMs (busy),
 *  or 'aborted' when `signal` aborts while the job waits (it then leaves the queue unserved). */
export class Pool {
  constructor(bytes, inflight = Infinity) { this.free = bytes; this.cap = bytes; this.slots = inflight; this.q = []; Pool.all.push(this); }
  static all = [];
  reserve(n, waitMs = RESERVE_WAIT_MS, shared = null, signal = null) {
    return new Promise((resolve) => {
      if (signal && signal.aborted) { resolve('aborted'); return; }
      const job = { n, shared, resolve, timer: null, signal, onAbort: null };
      const drop = (v) => { const i = this.q.indexOf(job); if (i >= 0) { this.q.splice(i, 1); clearTimeout(job.timer); if (signal) signal.removeEventListener('abort', job.onAbort); resolve(v); this.pump(); } };
      job.timer = setTimeout(() => drop(false), waitMs);
      if (signal) { job.onAbort = () => drop('aborted'); signal.addEventListener('abort', job.onAbort, { once: true }); }
      this.q.push(job); this.pump();
    });
  }
  pump() {
    while (this.q.length) {
      const j = this.q[0]; const sh = j.shared;
      if (j.n > this.free || this.slots < 1 || (sh && (sh.slots < 1 || j.n > sh.free))) return; // FIFO: the head blocks the rest
      this.q.shift(); clearTimeout(j.timer); if (j.signal) j.signal.removeEventListener('abort', j.onAbort);
      this.free -= j.n; this.slots--; if (sh) { sh.free -= j.n; sh.slots--; } j.resolve(true);
    }
  }
  release(n, shared = null) { this.free += n; this.slots++; if (shared) { shared.free += n; shared.slots++; for (const p of Pool.all) p.pump(); } else this.pump(); }
}
/** Shared inflight counter (RECV_INFLIGHT_MAX across site + content pools). */
export const inflight = { free: Infinity, slots: RECV_INFLIGHT_MAX };

const fail = (reason) => Object.assign(new Error(reason), { transport: reason });
const clip = (s, n) => (typeof s === 'string' ? s.slice(0, n) : '');

/**
 * request(endpoint, method, params, {pool, session, deadline, fetchImpl, signal, beforeSend, onState}) -> result
 * Throws {rpc:{code,message,data?}} for a remote error object, {transport: reason}
 * for transport/recvLimit/recvParse/recvDepth/busy/unsupported/cancelled.
 * Submission stages (A06 round 2): onState('queued') before the reservation, onState('initiated')
 * immediately before fetch. `signal` aborts the request while it is queued; `beforeSend` runs
 * synchronously after the reservation and immediately before fetch (no await in between) and
 * cancels the request by returning a truthy value. Once initiated, nothing cancels it.
 */
export async function request(endpoint, method, params, opts) {
  const limit = RECV_LIMIT.get(method); if (limit === undefined) throw fail('unsupported');
  const { pool, session = null, deadline = 10000, id = 1, fetchImpl = fetch, signal = null, beforeSend = null, onState = null } = opts;
  const shared = opts.shared === undefined ? null : opts.shared;
  // A09: one ticket covers the session budget and the pool budget. The session share is taken
  // synchronously (no await between the check and the add), so concurrent requests can never
  // exceed SESSION_RECV_MAX; every path (busy, timeout, abort, error, success) releases once.
  if (session) { if (session.recv + limit > SESSION_RECV_MAX) throw fail('busy'); session.recv += limit; }
  let pooled = false, released = false;
  const release = () => { if (released) return; released = true; if (pooled) pool.release(limit, shared); if (session) session.recv -= limit; };
  let ctl = null, timer = null;
  try {
    if (onState) onState('queued');
    const got = await pool.reserve(limit, RESERVE_WAIT_MS, shared, signal);
    if (got === 'aborted') throw fail('cancelled');
    if (!got) throw fail('busy');
    pooled = true;
    if ((signal && signal.aborted) || (beforeSend && beforeSend())) throw fail('cancelled');
    if (onState) onState('initiated');
    ctl = new AbortController(); timer = setTimeout(() => ctl.abort(), deadline);
    let res;
    try { res = await fetchImpl(endpoint, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id, method, params }), signal: ctl.signal, credentials: 'omit', cache: 'no-store', redirect: 'error' }); }
    catch { throw fail('transport'); }
    const cl = res.headers.get('content-length');
    if (cl !== null && Number(cl) > limit) { ctl.abort(); throw fail('recvLimit'); }
    const chunks = []; let total = 0;
    if (!res.body) throw fail('transport');
    const reader = res.body.getReader();
    try {
      for (;;) {
        const { done, value } = await reader.read(); if (done) break;
        if (total + value.length > limit) { ctl.abort(); throw fail('recvLimit'); }
        chunks.push(value); total += value.length;
      }
    } catch (e) { if (e.transport) throw e; throw fail('transport'); }
    const buf = new Uint8Array(total); let o = 0; for (const c of chunks) { buf.set(c, o); o += c.length; }
    let text; try { text = new TextDecoder('utf-8', { fatal: true }).decode(buf); } catch { throw fail('recvParse'); }
    let v; try { v = parseStrict(text, RESP_DEPTH_MAX); } catch (e) { throw fail(e.message === 'depth' ? 'recvDepth' : 'recvParse'); }
    if (res.status !== 200) throw fail('transport');
    if (v === null || typeof v !== 'object' || Array.isArray(v)) throw fail('recvParse');
    // A14: a JSON-RPC 2.0 response to exactly this request: version, the request's id, and
    // exactly one of result and error; an error is {code: integer, message: string, data?}.
    const has = (k) => Object.prototype.hasOwnProperty.call(v, k);
    if (v.jsonrpc !== '2.0' || !has('id') || v.id !== id || has('result') === has('error')) throw fail('recvParse');
    if (has('error')) {
      const e = v.error;
      if (e === null || typeof e !== 'object' || Array.isArray(e) || !Number.isInteger(e.code) || typeof e.message !== 'string') throw fail('recvParse');
      const out = { code: e.code, message: clip(e.message, ERR_MESSAGE_MAX) };
      if (e.data !== undefined) { const d = JSON.stringify(e.data); if (new TextEncoder().encode(d).length <= ERR_DATA_MAX) out.data = e.data; }
      throw Object.assign(new Error(out.message), { rpc: out });
    }
    return v.result;
  } finally {
    if (timer !== null) clearTimeout(timer); release();
  }
}
