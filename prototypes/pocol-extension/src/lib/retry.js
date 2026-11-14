// Bounded retry of idempotent JSON-RPC reads (Sites W2). A gateway or node that is momentarily
// busy answers HTTP 429 (transport 'rate', with Retry-After), the error -32021 {busy|rate,
// retryAfterMs}, a proxy in front of it 502/503/504, or the connection drops or times out (transport
// 'transport'). A read is then retried with bounded exponential backoff and jitter: at most
// RETRY.attempts attempts and a total deadline of RETRY.deadlineMs, which also bounds the attempts
// themselves: no retry starts unless it can end, its own timeout included (`attemptMs`), by the
// deadline. A server hint (Retry-After, retryAfterMs) is honoured up to RETRY.hintMaxMs. Any other
// HTTP status (transport 'status': 400, 403, 404, 413, 500, ...) is the endpoint's final answer and
// is never retried. Only the reads in IDEMPOTENT_READS are retried; anything else, above all
// eth_sendRawTransaction, is passed through exactly once (a broadcast whose answer was lost is
// reconciled by its caller from the receipt, never re-sent here). The extension's own local
// back-pressure (transport 'busy': the receive pool or a site session's budget) is not retried
// unless the caller asks (`retryBusy`, the verified chunk fetch only).
export const IDEMPOTENT_READS = Object.freeze(new Set([
  'eth_chainId', 'net_version', 'web3_clientVersion', 'eth_blockNumber', 'eth_gasPrice', 'eth_maxPriorityFeePerGas',
  'eth_getBalance', 'eth_getStorageAt', 'eth_getTransactionCount', 'pocol_getCaps', 'eth_getCode', 'pocol_getHeaders',
  'pocol_status', 'eth_getBlockByNumber', 'eth_getBlockByHash', 'eth_feeHistory', 'eth_getTransactionByHash',
  'eth_getTransactionReceipt', 'eth_call', 'eth_estimateGas', 'pocol_getLogs',
]));
export const RETRY = Object.freeze({ attempts: 6, baseMs: 250, maxMs: 8000, deadlineMs: 60000, hintMaxMs: 10000, minAttemptMs: 1000 });
const RETRY_STATUS = new Set([502, 503, 504]);
const WRAPPED = Symbol('pocol.retrying');
const hint = (v) => (Number.isFinite(v) && v > 0 ? v : 0);

/** The server's suggested wait in ms (0 when none) for a retryable failure, or null when `error` is final. */
export function retryHint(error, retryBusy = false) {
  if (!error || typeof error !== 'object') return null;
  if (error.transport === 'rate') return hint(error.retryAfterMs);
  // 'transport' is a fetch rejection, a timeout or a dropped body, or (with its status) 502/503/504.
  if (error.transport === 'transport') return error.status === undefined || RETRY_STATUS.has(error.status) ? hint(error.retryAfterMs) : null;
  if (error.transport === 'busy') return retryBusy ? 0 : null;
  if (error.rpc && error.rpc.code === -32021) return hint(error.rpc.data && error.rpc.data.retryAfterMs);
  return null;
}

function gaveUp(error, method, attempts) {
  const reason = error.transport || (error.rpc && error.rpc.message) || 'error';
  const out = new Error(`${method}: the network endpoint is busy or unreachable (${reason}); gave up after ${attempts} attempt${attempts === 1 ? '' : 's'}`);
  if (error.transport) out.transport = error.transport;
  if (error.rpc) out.rpc = error.rpc;
  out.attempts = attempts; out.cause = error;
  return out;
}

/**
 * withRetry(rpc, opts) -> rpc'. `rpc(method, params, ...rest)` returns a promise; extra arguments pass through.
 * opts: attempts, baseMs, maxMs, deadlineMs, hintMaxMs, minAttemptMs (RETRY), retryBusy, aborted() (stops
 * retrying), onRetry({method, attempt, waitMs, error}), and the injectable clock(), sleep(ms) and random() (tests).
 * The deadline: `attemptMs` (a number, or (method, params, ...rest) -> ms; default 0) is the longest one attempt
 * may take (the caller's own timeout). A retry starts only if the time spent, the wait and attemptMs fit in
 * deadlineMs. When they do not, `shorten(rest, ms)` (optional) returns the extra arguments for an attempt bounded
 * to the `ms` left, which is used if at least minAttemptMs; otherwise the loop gives up. The first attempt is the
 * caller's as given, so a read takes at most max(first attempt, deadlineMs).
 * Wrapping an already wrapped function returns it unchanged, so nested wrappers never multiply attempts.
 */
export function withRetry(rpc, opts = {}) {
  if (rpc[WRAPPED]) return rpc;
  const p = { ...RETRY, ...opts };
  const clock = opts.clock || Date.now, random = opts.random || Math.random;
  const sleep = opts.sleep || ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
  const aborted = opts.aborted || (() => false), isRead = opts.isRead || ((m) => IDEMPOTENT_READS.has(m));
  const attemptMs = typeof opts.attemptMs === 'function' ? opts.attemptMs : () => opts.attemptMs || 0;
  const retrying = async (method, params, ...rest) => {
    if (!isRead(method)) return rpc(method, params, ...rest);
    const start = clock(); const need = attemptMs(method, params, ...rest); let args = rest;
    for (let attempt = 1; ; attempt++) {
      try { return await rpc(method, params, ...args); }
      catch (error) {
        const h = retryHint(error, !!p.retryBusy);
        if (h === null) throw error;
        if (attempt >= p.attempts || aborted()) throw gaveUp(error, method, attempt);
        const backoff = Math.min(p.maxMs, p.baseMs * 2 ** (attempt - 1));
        const waitMs = Math.max(Math.round(backoff * (0.5 + random() / 2)), Math.min(h, p.hintMaxMs));
        const left = p.deadlineMs - (clock() - start) - waitMs;
        const fits = left >= need, shortened = !fits && !!opts.shorten && left >= Math.max(p.minAttemptMs, 1);
        if (!fits && !shortened) throw gaveUp(error, method, attempt);
        args = fits ? rest : opts.shorten(rest, left);
        if (opts.onRetry) { try { opts.onRetry({ method, attempt, waitMs, error }); } catch { /* observer only */ } }
        await sleep(waitMs);
        if (aborted()) throw gaveUp(error, method, attempt);
      }
    }
  };
  Object.defineProperty(retrying, WRAPPED, { value: true });
  return retrying;
}
