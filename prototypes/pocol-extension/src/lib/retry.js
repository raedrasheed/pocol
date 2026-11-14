// Bounded retry of idempotent JSON-RPC reads (Sites W2). A gateway or node that is momentarily
// busy answers HTTP 429 (transport 'rate', with Retry-After), the error -32021 {busy|rate,
// retryAfterMs}, or drops the connection or times out (transport 'transport'). A read is then
// retried with bounded exponential backoff and jitter: at most RETRY.attempts attempts and a total
// deadline of RETRY.deadlineMs. A server hint (Retry-After, retryAfterMs) is honoured up to
// RETRY.hintMaxMs. Only the reads in IDEMPOTENT_READS are retried; anything else, above all
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
export const RETRY = Object.freeze({ attempts: 6, baseMs: 250, maxMs: 8000, deadlineMs: 60000, hintMaxMs: 10000 });
const WRAPPED = Symbol('pocol.retrying');
const hint = (v) => (Number.isFinite(v) && v > 0 ? v : 0);

/** The server's suggested wait in ms (0 when none) for a retryable failure, or null when `error` is final. */
export function retryHint(error, retryBusy = false) {
  if (!error || typeof error !== 'object') return null;
  if (error.transport === 'rate' || error.transport === 'transport') return hint(error.retryAfterMs);
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
 * opts: attempts, baseMs, maxMs, deadlineMs, hintMaxMs (RETRY), retryBusy, aborted() (stops retrying),
 * onRetry({method, attempt, waitMs, error}), and the injectable clock(), sleep(ms) and random() (tests).
 * Wrapping an already wrapped function returns it unchanged, so nested wrappers never multiply attempts.
 */
export function withRetry(rpc, opts = {}) {
  if (rpc[WRAPPED]) return rpc;
  const p = { ...RETRY, ...opts };
  const clock = opts.clock || Date.now, random = opts.random || Math.random;
  const sleep = opts.sleep || ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
  const aborted = opts.aborted || (() => false), isRead = opts.isRead || ((m) => IDEMPOTENT_READS.has(m));
  const retrying = async (method, params, ...rest) => {
    if (!isRead(method)) return rpc(method, params, ...rest);
    const start = clock();
    for (let attempt = 1; ; attempt++) {
      try { return await rpc(method, params, ...rest); }
      catch (error) {
        const h = retryHint(error, !!p.retryBusy);
        if (h === null) throw error;
        if (attempt >= p.attempts || aborted()) throw gaveUp(error, method, attempt);
        const backoff = Math.min(p.maxMs, p.baseMs * 2 ** (attempt - 1));
        const waitMs = Math.max(Math.round(backoff * (0.5 + random() / 2)), Math.min(h, p.hintMaxMs));
        if (clock() - start + waitMs > p.deadlineMs) throw gaveUp(error, method, attempt);
        if (opts.onRetry) { try { opts.onRetry({ method, attempt, waitMs, error }); } catch { /* observer only */ } }
        await sleep(waitMs);
        if (aborted()) throw gaveUp(error, method, attempt);
      }
    }
  };
  Object.defineProperty(retrying, WRAPPED, { value: true });
  return retrying;
}
