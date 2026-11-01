// BridgeAuth (browser.md D95–D97): every site message is checked here before any
// network or local effect. Default deny. Check order: Bpre size, rate bucket,
// B0 strict structure, B1 kind, B2 (kind,method) allowlist, B3 params schema.
export const BRIDGE_MSG_MAX = 65536, BUCKET_CAP_MT = 50000, BUCKET_REFILL_MT_PER_MS = 50, MSG_COST_MT = 1000;
export const PENDING_READS_MAX = 4, GLOBAL_PENDING_READS_MAX = 16;
const KINDS = new Set(['rpc_read', 'wallet_req', 'storage_set', 'nav']);
const err = (id, code, message, data) => ({ id, error: data === undefined ? { code, message } : { code, message, data } });

/** Strict JSON in one linear pass: rejects duplicate keys (any level), depth > maxDepth
 * (error 'depth'), control characters in strings, invalid escapes, trailing data.
 * Objects have a null prototype, so no key reaches Object.prototype. */
export function parseStrict(text, maxDepth = 8) {
  const s = text, n = s.length; let i = 0;
  const fail = (m = 'json') => { throw new Error(m); };
  const ws = () => { while (i < n) { const c = s.charCodeAt(i); if (c === 32 || c === 9 || c === 10 || c === 13) i++; else break; } };
  const HEX = /^[0-9a-fA-F]{4}$/;
  function str() {
    if (s.charCodeAt(i) !== 34) fail(); i++; let out = '', start = i;
    for (;;) {
      if (i >= n) fail(); const c = s.charCodeAt(i);
      if (c === 34) { out += s.slice(start, i); i++; return out; }
      if (c < 0x20) fail();
      if (c === 92) {
        out += s.slice(start, i); const e = s[i + 1];
        if (e === 'u') { const h = s.slice(i + 2, i + 6); if (!HEX.test(h)) fail(); out += String.fromCharCode(parseInt(h, 16)); i += 6; }
        else { const m = { '"': '"', '\\': '\\', '/': '/', b: '\b', f: '\f', n: '\n', r: '\r', t: '\t' }[e]; if (m === undefined) fail(); out += m; i += 2; }
        start = i; continue;
      }
      i++;
    }
  }
  function num() {
    const st = i; if (s[i] === '-') i++;
    if (s[i] === '0') i++; else if (s[i] >= '1' && s[i] <= '9') { while (s[i] >= '0' && s[i] <= '9') i++; } else fail();
    if (s[i] === '.') { i++; if (!(s[i] >= '0' && s[i] <= '9')) fail(); while (s[i] >= '0' && s[i] <= '9') i++; }
    if (s[i] === 'e' || s[i] === 'E') { i++; if (s[i] === '+' || s[i] === '-') i++; if (!(s[i] >= '0' && s[i] <= '9')) fail(); while (s[i] >= '0' && s[i] <= '9') i++; }
    return Number(s.slice(st, i));
  }
  function val(depth) {
    ws(); const c = s[i];
    if (c === '{') {
      if (depth >= maxDepth) fail('depth'); i++; const o = Object.create(null); const seen = new Set(); ws();
      if (s[i] === '}') { i++; return o; }
      for (;;) { ws(); const k = str(); if (seen.has(k)) fail('duplicate'); seen.add(k); ws(); if (s[i++] !== ':') fail(); o[k] = val(depth + 1); ws(); if (s[i] === ',') { i++; continue; } if (s[i] === '}') { i++; return o; } fail(); }
    }
    if (c === '[') {
      if (depth >= maxDepth) fail('depth'); i++; const a = []; ws();
      if (s[i] === ']') { i++; return a; }
      for (;;) { a.push(val(depth + 1)); ws(); if (s[i] === ',') { i++; continue; } if (s[i] === ']') { i++; return a; } fail(); }
    }
    if (c === '"') return str();
    if (s.startsWith('true', i)) { i += 4; return true; }
    if (s.startsWith('false', i)) { i += 5; return false; }
    if (s.startsWith('null', i)) { i += 4; return null; }
    return num();
  }
  const v = val(0); ws(); if (i !== n) fail(); return v;
}
const isObj = (x) => x !== null && typeof x === 'object' && !Array.isArray(x);
const keysExactly = (o, ks) => { const k = Object.keys(o); return k.length === ks.length && ks.every((x) => Object.prototype.hasOwnProperty.call(o, x)); };
const lone = /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/;
const u8len = (s) => new TextEncoder().encode(s).length;
// Primitive types (browser.md).
const T = {
  addr: (x) => typeof x === 'string' && /^0x[0-9a-fA-F]{40}$/.test(x),
  hash32: (x) => typeof x === 'string' && /^0x[0-9a-fA-F]{64}$/.test(x),
  qty: (x) => typeof x === 'string' && /^0x(0|[1-9a-fA-F][0-9a-fA-F]{0,15})$/.test(x),
  tag: (x) => x === 'latest' || x === 'safe' || x === 'finalized' || T.qty(x),
  data: (x) => typeof x === 'string' && /^0x([0-9a-fA-F]{2})*$/.test(x) && x.length <= 2 + 2 * 65536,
  bool: (x) => typeof x === 'boolean',
  str: (n) => (x) => typeof x === 'string' && !lone.test(x) && u8len(x) >= 1 && u8len(x) <= n,
};
const has = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
const fieldsCheck = (allowed, x, path) => { for (const k of Object.keys(x)) if (!allowed.includes(k)) return path + '.' + k; return null; };
// callObj (browser.md): fields from {from,to,gas,gasPrice,maxFeePerGas,maxPriorityFeePerGas,value,data,input};
// `to` required (no deploy simulation); gas <= 30000000; data or input, not both.
const callObj = (x, path) => {
  if (!isObj(x)) return path;
  const f = fieldsCheck(['from', 'to', 'gas', 'gasPrice', 'maxFeePerGas', 'maxPriorityFeePerGas', 'value', 'data', 'input'], x, path); if (f) return f;
  if (!has(x, 'to') || !T.addr(x.to)) return path + '.to';
  if (has(x, 'from') && !T.addr(x.from)) return path + '.from';
  if (has(x, 'data') && has(x, 'input')) return path + '.input';
  for (const k of ['data', 'input']) if (has(x, k) && !T.data(x[k])) return path + '.' + k;
  for (const k of ['gas', 'gasPrice', 'maxFeePerGas', 'maxPriorityFeePerGas', 'value']) if (has(x, k) && !T.qty(x[k])) return path + '.' + k;
  if (has(x, 'gas') && BigInt(x.gas) > 30000000n) return path + '.gas';
  return null;
};
// txObj (wallet_req eth_sendTransaction): fields from {from,to,gas,value,data,maxFeePerGas,maxPriorityFeePerGas,nonce}.
const txObj = (x, path) => {
  if (!isObj(x)) return path;
  const f = fieldsCheck(['from', 'to', 'gas', 'value', 'data', 'maxFeePerGas', 'maxPriorityFeePerGas', 'nonce'], x, path); if (f) return f;
  if (!has(x, 'to') || !T.addr(x.to)) return path + '.to';
  if (has(x, 'from') && !T.addr(x.from)) return path + '.from';
  if (has(x, 'data') && !T.data(x.data)) return path + '.data';
  for (const k of ['gas', 'value', 'maxFeePerGas', 'maxPriorityFeePerGas', 'nonce']) if (has(x, k) && !T.qty(x[k])) return path + '.' + k;
  if (has(x, 'gas') && BigInt(x.gas) > 30000000n) return path + '.gas';
  return null;
};
const topicsOk = (t) => Array.isArray(t) && t.length <= 4 && t.every((x) => x === null || T.hash32(x) || (Array.isArray(x) && x.length <= 16 && x.every(T.hash32)));
const addrsOk = (a) => T.addr(a) || (Array.isArray(a) && a.length <= 16 && a.every(T.addr));
// filter: {fromBlock, toBlock, address?, topics?} or {blockHash, address?, topics?}.
const filterObj = (x, path) => {
  if (!isObj(x)) return path;
  const byHash = has(x, 'blockHash');
  const f = fieldsCheck(byHash ? ['blockHash', 'address', 'topics'] : ['fromBlock', 'toBlock', 'address', 'topics'], x, path); if (f) return f;
  if (byHash) { if (!T.hash32(x.blockHash)) return path + '.blockHash'; }
  else for (const k of ['fromBlock', 'toBlock']) if (!has(x, k) || !T.tag(x[k])) return path + '.' + k;
  if (has(x, 'address') && !addrsOk(x.address)) return path + '.address';
  if (has(x, 'topics') && !topicsOk(x.topics)) return path + '.topics';
  return null;
};
const P = (...checks) => (params) => {
  if (params.length !== checks.length) return 'params';
  for (let i = 0; i < checks.length; i++) { const c = checks[i]; const p = `params[${i}]`; const r = c.length === 2 ? c(params[i], p) : (c(params[i]) ? null : p); if (r) return r; }
  return null;
};
const oneOf = (...schemas) => (params) => { const m = schemas.find((s) => s.arity === params.length); return m ? m(params) : 'params'; };
const arity = (n, f) => Object.assign(f, { arity: n });
const nullable = (f) => (x) => x === null || f(x);
const isFalse = (x) => x === false;
const qty128 = (x) => T.qty(x) && BigInt(x) <= 128n;
const percentiles = (x) => Array.isArray(x) && x.length <= 16 && x.every((v, i) => typeof v === 'number' && Number.isFinite(v) && v >= 0 && v <= 100 && (i === 0 || v >= x[i - 1]));
// pathStr: printable ASCII 0x21-0x7e, length 1..2048, starting with '/'.
const pathStr = (x) => typeof x === 'string' && x.length >= 1 && x.length <= 2048 && /^[\x21-\x7e]+$/.test(x) && x[0] === '/';
// httpsUrl: printable ASCII <= 2048, literally 'https://', URL-parsed protocol https:, non-empty hostname, no credentials.
const httpsUrl = (x) => {
  if (typeof x !== 'string' || x.length > 2048 || !/^[\x21-\x7e]+$/.test(x) || !x.startsWith('https://')) return false;
  try { const u = new URL(x); return u.protocol === 'https:' && u.hostname !== '' && u.username === '' && u.password === ''; } catch { return false; }
};
// Frozen allowlist: Map of kind -> Map(method -> schema). No prototype lookups.
export const ALLOW = Object.freeze(new Map([
  ['rpc_read', Object.freeze(new Map([
    ['eth_chainId', P()], ['net_version', P()], ['web3_clientVersion', P()], ['eth_blockNumber', P()], ['eth_gasPrice', P()], ['eth_maxPriorityFeePerGas', P()],
    ['eth_getBalance', P(T.addr, T.tag)], ['eth_getCode', P(T.addr, T.tag)], ['eth_getTransactionCount', P(T.addr, T.tag)],
    ['eth_getStorageAt', P(T.addr, T.hash32, T.tag)],
    ['eth_getBlockByNumber', P(T.tag, isFalse)], ['eth_getBlockByHash', P(T.hash32, isFalse)],
    ['eth_getTransactionByHash', P(T.hash32)], ['eth_getTransactionReceipt', P(T.hash32)],
    ['eth_call', P(callObj, T.tag)], ['eth_estimateGas', oneOf(arity(1, P(callObj)), arity(2, P(callObj, T.tag)))],
    ['eth_feeHistory', P(qty128, T.tag, percentiles)], ['eth_getLogs', P(filterObj)],
  ]))],
  ['wallet_req', Object.freeze(new Map([['eth_accounts', P()], ['eth_requestAccounts', P()], ['eth_chainId', P()], ['eth_sendTransaction', P(txObj)]]))],
  ['storage_set', Object.freeze(new Map([['site_storageSet', P(T.str(256), nullable(T.str(61440)))], ['site_storageClear', P()]]))],
  ['nav', Object.freeze(new Map([['site_navigate', P(pathStr)], ['site_openExternal', P(httpsUrl)]]))],
]));
// Navigation bucket per site session (D99): cap 6000 mt, refill 1 mt/ms, cost 2000 mt.
export const NAV_CAP_MT = 6000, NAV_REFILL_MT_PER_MS = 1, NAV_COST_MT = 2000;
export function navTake(session, nowMs) {
  session.nav = Math.min(NAV_CAP_MT, session.nav + NAV_REFILL_MT_PER_MS * Math.max(0, nowMs - session.navLast)); session.navLast = nowMs;
  if (session.nav < NAV_COST_MT) return false; session.nav -= NAV_COST_MT; return true;
}
export function newSession(nowMs) { return { tokens: BUCKET_CAP_MT, last: nowMs, pendingReads: 0, nav: NAV_CAP_MT, navLast: nowMs }; }
/** Returns {ok:true, id, kind, method, params} or {ok:false, reply}. */
export function check(session, text, arrivalMs) {
  if (typeof text !== 'string' || u8len(text) > BRIDGE_MSG_MAX) return { ok: false, reply: err(null, -32600, 'size', { reason: 'size' }) };
  /*@fault noRefill*/session.tokens = Math.min(BUCKET_CAP_MT, session.tokens + BUCKET_REFILL_MT_PER_MS * Math.max(0, arrivalMs - session.last));/*@else*//*@end*/ session.last = arrivalMs;
  if (session.tokens < MSG_COST_MT) return { ok: false, reply: err(null, -32005, 'rate', { reason: 'rate' }) };
  session.tokens -= MSG_COST_MT;
  let v; try { v = parseStrict(text); } catch { return { ok: false, reply: err(null, -32600, 'invalid request') }; }
  const idOk = isObj(v) && Number.isInteger(v.id) && v.id >= 0 && v.id <= 0xffffffff; const id = idOk ? v.id : null;
  if (!isObj(v) || !keysExactly(v, ['id', 'kind', 'payload']) || !idOk || typeof v.kind !== 'string' || !isObj(v.payload) || !keysExactly(v.payload, ['method', 'params']) || typeof v.payload.method !== 'string' || !Array.isArray(v.payload.params))
    return { ok: false, reply: err(id, -32600, 'invalid request') };
  if (!KINDS.has(v.kind)) return { ok: false, reply: err(id, -32600, 'kind', { reason: 'kind' }) };
  const method = v.payload.method; const table = ALLOW.get(v.kind);
  const nameOk = u8len(method) <= 64 && /^[\x21-\x7e]+$/.test(method);
  const schema = nameOk ? table.get(method) : undefined;
  /*@fault allowAll*/if (!schema) return { ok: false, reply: err(id, 4200, 'unsupported method', { method: method.slice(0, 64) }) };/*@else*/if (!schema) return { ok: true, id, kind: v.kind, method, params: v.payload.params };/*@end*/
  const bad = schema(v.payload.params);
  if (bad) return { ok: false, reply: err(id, -32602, 'invalid params', { path: bad }) };
  return { ok: true, id, kind: v.kind, method, params: v.payload.params };
}
