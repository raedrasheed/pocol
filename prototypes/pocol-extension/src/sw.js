// PoCol extension service worker: network profiles, verified site loading
// (RP window or LN trust), BridgeAuth for site messages, DNR isolation, wallet.
import { decodeSpec, verifyWindow } from './lib/rp.js';
import { fetchSite, normalize } from './lib/site.js';
import * as bridge from './lib/bridge.js';
import { StoreAdmission, qbytes } from './lib/sitestore.js';
import { RecordStore } from './lib/records.js';
import * as http from './lib/transport.js';
import { encryptSeed, decryptSeed, networkKey, addressOf, unsignedTx, signUnsigned, LOCK_MS } from './lib/wallet.js';
import { keccak } from './lib/crypto.js';
import * as rlp from './lib/rlp.js';
import { toHex, fromHex, eq } from './lib/hex.js';

const store = chrome.storage.local;
// Site storage records with a worker-generation fence (D103); no data write before the fence confirms.
const records = new RecordStore(store);
const booted = records.boot().catch(() => null);
const genId = crypto.randomUUID();
const admission = new StoreAdmission();
const sessions = new Map(); // sessionId -> SiteSession (D99)
let globalPending = 0;
let unlocked = null; // {seed, until}
const RPC_TIMEOUT_MS = 10000, LOGS_TIMEOUT_MS = 30000, LOG_RANGE_MAX = 1024;
const WALLET_PER_MIN = 5;
const IDENTITY_CHECK_MS = 30000; // browser.md: identity at open, every 30 s and at signing
const REVOKE_DNR_GRACE_MS = 2000;

// ---------------- profiles ----------------
export function netKeyOf(chainId, genesisHash) {
  return toHex(keccak(rlp.encode([new TextEncoder().encode('PoCol-net-v1'), rlp.encUint(BigInt(chainId)), genesisHash])));
}
const storedProfiles = async () => (await store.get('profiles')).profiles || [];
/** Stored profiles re-validated on every read (A06/A07): a profile that no longer passes
 *  validation is listed with `invalid` and is unusable; `conflict` is recomputed over the whole
 *  set, so both profiles of a chainId with two genesis hashes are read-only (D-A06-CONFLICT). */
async function profiles() {
  const out = [];
  for (const p of await storedProfiles()) { try { out.push(validateProfile(p)); } catch (e) { out.push({ name: String(p && p.name), invalid: String(e.message || e) }); } }
  for (const p of out) if (!p.invalid) p.conflict = out.some((x) => x !== p && !x.invalid && x.chainId === p.chainId && x.genesisHash !== p.genesisHash);
  return out;
}
/** Profile acceptance (browser.md D58/D77/D79): decode genesisPre, hash, chainId. */
function validateProfile(p) {
  if (!/^[A-Za-z0-9_-]{1,32}$/.test(p.name)) throw new Error('name');
  if (!['DEV', 'LN', 'RP'].includes(p.trustLevel)) throw new Error('trustLevel');
  if (!/^http:\/\/127\.0\.0\.1:\d{1,5}\/?$/.test(p.endpoint)) throw new Error('endpoint must be http://127.0.0.1:<port> (optional remote hosts are not enabled)');
  const pre = fromHex(p.genesisPre); const spec = decodeSpec(pre);
  if (!eq(spec.genesisHash, fromHex(p.genesisHash))) throw new Error('genesisHash mismatch');
  if (spec.chainId !== BigInt(p.chainId)) throw new Error('chainId mismatch');
  return { name: p.name, chainId: Number(p.chainId), genesisPre: p.genesisPre, genesisHash: p.genesisHash.toLowerCase(), endpoint: p.endpoint, trustLevel: p.trustLevel, netKey: netKeyOf(p.chainId, spec.genesisHash) };
}
// HttpTransport pools (D98): trust checks never starve behind sites; chunk fetches have their own pool.
const pools = { site: new http.Pool(http.SITE_POOL), trust: new http.Pool(http.TRUST_POOL), content: new http.Pool(http.CONTENT_POOL) };
let reqId = 0;
/** rpc for one pool; session (optional) carries the per-session SESSION_RECV_MAX account. */
function rpcFor(profile, pool, session = null, shared = http.inflight) {
  return (method, params, timeoutMs = RPC_TIMEOUT_MS, stages = {}) => http.request(profile.endpoint, method, params, { ...stages, pool, session, deadline: timeoutMs, id: ++reqId, shared: pool === pools.trust ? null : shared });
}
const contentRpc = (profile) => { const c = rpcFor(profile, pools.content), s = rpcFor(profile, pools.site); return (m, p) => (m === 'eth_getCode' ? c(m, p) : s(m, p)); };
/** netKey check against the endpoint: chainId and genesisHash must equal the profile. */
async function checkNetwork(profile, rpc) {
  const cid = BigInt(await rpc('eth_chainId', []));
  if (cid !== BigInt(profile.chainId)) throw Object.assign(new Error('endpoint chainId differs from profile'), { code: 4901 });
  const st = await rpc('pocol_status', []);
  if (st.genesisHash.toLowerCase() !== profile.genesisHash.toLowerCase()) throw Object.assign(new Error('endpoint genesis differs from profile'), { code: 4901 });
}

// ---------------- signing eligibility (browser.md "أهلية التوقيع", A06) ----------------
// Every profile change runs under ProfileMutex and bumps eligibilityEpoch; every wallet
// transition (create, unlock, lock, timeout) bumps walletEpoch. Either cancels pending
// approvals, and a wallet request re-checks both after every await, so an old request never
// resumes with a stale key or profile. Signing runs under ProfileMutex: eligibility, identity
// (endpoint chainId and genesis), a second eligibility check, then the shown bytes are signed.
let profileChain = Promise.resolve();
const profileLocked = (f) => { const r = profileChain.then(f); profileChain = r.catch(() => {}); return r; };
let eligibilityEpoch = 0, walletEpoch = 0;
const approvals = new Set(); // {sess, cancel(code, message)}
const epochs = () => ({ profile: eligibilityEpoch, wallet: walletEpoch });
function cancelApprovals(pred, code, message) { for (const a of [...approvals]) if (pred(a.sess)) a.cancel(code, message); }
function walletChanged(message) { walletEpoch++; cancelApprovals(() => true, 4100, message); }
const lockWallet = (message) => { unlocked = null; walletChanged(message); };
const isUnlocked = () => { if (unlocked && Date.now() > unlocked.until) lockWallet('wallet locked (timeout)'); return !!unlocked; };
const PROFILE_FIELDS = ['name', 'chainId', 'genesisPre', 'genesisHash', 'endpoint', 'trustLevel', 'netKey'];
const sameProfile = (a, b) => PROFILE_FIELDS.every((k) => a[k] === b[k]);
/** A transition since the request captured `ep` (checked after every await). */
function changed(sess, ep) {
  if (sess.closed) return E(4900, 'disconnected');
  if (ep.wallet !== walletEpoch) return E(4100, 'wallet locked or changed');
  if (ep.profile !== eligibilityEpoch) return E(4901, 'network profiles changed');
  return null;
}
/** signEligible: the profile equals the frozen one (netKey not revoked), no conflict, the session
 *  is connected, the wallet unlocked, the epochs unchanged, and LN, DEV or RP with h >= 1 and a
 *  successful window (established at open). */
async function signEligible(sess, ep) {
  const cur = (await profiles()).find((p) => p.name === sess.profile.name);
  const c = changed(sess, ep); if (c) return c;
  if (!cur || cur.invalid || !sameProfile(cur, sess.profile)) return E(4901, 'network profile changed');
  if (cur.conflict) return E(4100, 'conflicting network profile is read-only');
  if (!isUnlocked()) return E(4100, 'wallet locked');
  if (sess.trust.level === 'RP' && !(sess.trust.ok && sess.trust.head >= 1)) return E(4100, 'RP window not established');
  return null;
}
/** A confirmation that a wallet or profile transition cancels: resolves null (approved) or an error. */
function approval(sess, ask, q) {
  const p = ask(q);
  return new Promise((resolve) => {
    const a = { sess, cancel: (code, message) => { approvals.delete(a); resolve(E(code, message)); p.dismiss(); } };
    approvals.add(a);
    p.then((ok) => { if (approvals.delete(a)) resolve(ok ? null : E(4001, 'user rejected')); });
  });
}
/** Identity of the session's endpoint; a mismatch ends the session with 4901. */
async function identity(sess) {
  try { await checkNetwork(sess.profile, rpcFor(sess.profile, pools.trust)); return null; }
  catch (e) { if (e.code === 4901) { sess.revoke(4901, e.message); return E(4901, e.message); } return transportError(e); }
}
/** A profile change begins (under ProfileMutex, before its storage write): requests in flight
 *  can no longer rely on the old profile set, so approvals and queued submissions end (4901). */
function profilesChanging() { eligibilityEpoch++; cancelApprovals(() => true, 4901, 'network profiles changing'); }
/** After a profile change (under ProfileMutex): sessions whose profile was removed, changed or
 *  invalidated end with 4901; a session whose profile became conflicting stays read-only. */
async function profilesChanged() {
  eligibilityEpoch++; cancelApprovals(() => true, 4901, 'network profiles changed');
  const list = await profiles();
  for (const s of [...sessions.values()]) { const cur = list.find((p) => p.name === s.profile.name); if (!cur || cur.invalid || !sameProfile(cur, s.profile)) s.revoke(4901, 'network profile changed'); }
}

// ---------------- DNR isolation (layer 2) ----------------
// Rule ids (amendment LP9-DNR-01): browser.md names {2·tabId, 2·tabId+1}; Chrome tab ids can
// exceed 2^30, so ids come from a per-tab slot k (block 2k prio 1, allow 2k+1 prio 2). The
// condition still names the tab. The slot map is rebuilt from getSessionRules() after a
// worker restart. main_frame is not blocked: the sandboxed frame cannot navigate the top.
const SUB_TYPES = ['sub_frame', 'stylesheet', 'script', 'image', 'font', 'object', 'xmlhttprequest', 'ping', 'csp_report', 'media', 'websocket', 'webtransport', 'webbundle', 'other'];
let dnrChain = Promise.resolve();
const dnrLocked = (f) => { const r = dnrChain.then(f); dnrChain = r.catch(() => {}); return r; };
const tabRules = (rules, tabId) => rules.filter((r) => r.condition && Array.isArray(r.condition.tabIds) && r.condition.tabIds.length === 1 && r.condition.tabIds[0] === tabId);
function installDnr(tabId) {
  return dnrLocked(async () => {
    const rules = await chrome.declarativeNetRequest.getSessionRules();
    const old = tabRules(rules, tabId).map((r) => r.id); const used = new Set(rules.map((r) => r.id));
    let k = 1; while (used.has(2 * k) || used.has(2 * k + 1)) k++;
    const block = 2 * k, allow = 2 * k + 1;
    await chrome.declarativeNetRequest.updateSessionRules({
      removeRuleIds: old,
      addRules: [
        { id: block, priority: 1, action: { type: 'block' }, condition: { tabIds: [tabId], resourceTypes: SUB_TYPES } },
        { id: allow, priority: 2, action: { type: 'allow' }, condition: { tabIds: [tabId], urlFilter: '|chrome-extension://' + chrome.runtime.id + '/', resourceTypes: SUB_TYPES } },
      ],
    });
    const now = tabRules(await chrome.declarativeNetRequest.getSessionRules(), tabId);
    if (!now.some((r) => r.id === block && r.action.type === 'block') || !now.some((r) => r.id === allow && r.action.type === 'allow')) throw Object.assign(new Error('DNR install failed (closed)'), { code: 4900 });
    return { block, allow };
  });
}
const opening = new Map(); // tabId -> opens in progress
function removeDnr(tabId) {
  return dnrLocked(async () => {
    if ((opening.get(tabId) || 0) > 0 || [...sessions.values()].some((s) => s.tabId === tabId)) return;
    const ids = tabRules(await chrome.declarativeNetRequest.getSessionRules(), tabId).map((r) => r.id);
    if (ids.length) await chrome.declarativeNetRequest.updateSessionRules({ removeRuleIds: ids });
  });
}
chrome.tabs.onRemoved.addListener((tabId) => { removeDnr(tabId).catch(() => {}); });

// ---------------- open a site (D26 step 2: the worker fetches and verifies) ----------------
const b64 = (u8) => { let s = ''; for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000)); return btoa(s); };
async function openSite({ name, site, path, tabId }) {
  const ep0 = eligibilityEpoch; // A06 review: a session opened across a profile change is refused
  const profile = (await profiles()).find((p) => p.name === name); if (!profile) throw new Error('unknown profile ' + name);
  if (profile.invalid) throw Object.assign(new Error('profile no longer valid: ' + profile.invalid), { code: 4901 });
  if (!/^0x[0-9a-fA-F]{40}$/.test(site)) throw new Error('site address');
  if (tabId === undefined) throw Object.assign(new Error('a site opens only in a viewer tab'), { code: 4900 });
  opening.set(tabId, (opening.get(tabId) || 0) + 1);
  // A refused or failed open leaves no rules behind (removeDnr keeps them while a session or another open holds the tab).
  try { return await openVerified(profile, site, path, tabId, ep0); } finally { opening.set(tabId, opening.get(tabId) - 1); if (!opening.get(tabId)) opening.delete(tabId); removeDnr(tabId).catch(() => {}); }
}
async function openVerified(profile, site, path, tabId, ep0) {
  /*@fault noDnr*/await installDnr(tabId);/*@else*//* TEST FAULT: DNR not installed *//*@end*/
  const rpc = rpcFor(profile, pools.trust); await checkNetwork(profile, rpc);
  let trust = { level: profile.trustLevel, message: profile.trustLevel === 'LN' ? 'LN: عقدة محلية تتحقق من التنفيذ كاملًا' : 'DEV: شبكة تطوير بلا تحقق' };
  if (profile.trustLevel === 'RP') {
    const spec = decodeSpec(fromHex(profile.genesisPre));
    try { trust = { level: 'RP', ...(await verifyWindow(rpc, spec)) }; }
    catch (e) { return { ok: false, error: { code: 4901, message: 'RP window rejected: ' + (e.flag || e.message) } }; }
    if (!trust.ok) return { ok: false, error: { code: 4901, message: trust.message } };
  }
  const s = await fetchSite(contentRpc(profile), site);
  const files = {}; for (const [p, f] of s.files) files[p] = { mime: f.mime, b64: b64(f.data) };
  const sessionId = crypto.randomUUID(); const now = Math.floor(performance.now());
  const recv = { recv: 0 };
  // A profile change during the awaits above was not seen by profilesChanged (the session did not
  // exist yet): refuse rather than register a session on a removed or changed profile.
  if (eligibilityEpoch !== ep0) return { ok: false, error: { code: 4901, message: 'network profiles changed while the site was opening; open it again' } };
  // The session freezes its profile (without the recomputed conflict flag) for signEligible.
  const frozen = Object.freeze(Object.fromEntries(PROFILE_FIELDS.map((k) => [k, profile[k]])));
  const sess = { profile: frozen, site: site.toLowerCase(), netKey: profile.netKey, bucket: bridge.newSession(now), approved: null, rpc: rpcFor(profile, pools.site, recv), tabId, trust,
    confirmPending: false, walletPending: false, walletTimes: [], storeQ: admission.session(), closed: false };
  // Until the viewer connects, ending the session only drops it (the port handler replaces this).
  sess.revoke = () => { sess.closed = true; sessions.delete(sessionId); removeDnr(tabId).catch(() => {}); };
  sessions.set(sessionId, sess);
  await booted; const snap = await records.load(profile.netKey, site); const storage = snap.dict;
  // Revoked while its storage loaded (e.g. a profile change): the viewer must not connect to it.
  if (sess.closed) return { ok: false, error: { code: 4901, message: 'network profile changed while the site was opening; open it again' } };
  const norm = normalize(path || '/'); const target = norm === '' ? s.entry : norm;
  return { ok: true, sessionId, version: s.version, publishedBlock: s.publishedBlock, entry: s.entry, path: target, notFound: norm === null || !s.files.has(target), files, storage, storageError: snap.error || null, trust, netKey: profile.netKey };
}

// ---------------- B4 routing (state limits of the target component, D97) ----------------
const E = (code, message, data) => ({ error: data === undefined ? { code, message } : { code, message, data } });
/** Remote error objects pass through (already clipped by RecvGuard); local failures have fixed shapes. */
function transportError(e) {
  if (e.rpc) return { error: e.rpc };
  if (e.transport === 'busy') return E(-32005, 'busy', { reason: 'busy' });
  const reason = ['recvLimit', 'recvDepth', 'recvParse'].includes(e.transport) ? e.transport : 'transport';
  return E(-32603, reason, { reason });
}
async function rpcRead(sess, m) {
  // RpcReadClient re-checks the read allowlist independently of BridgeAuth B2 (defense in depth).
  /*@fault allowAll*/if (!bridge.ALLOW.get('rpc_read').has(m.method)) return E(4200, 'unsupported method', { method: m.method.slice(0, 64) });/*@else*//*@end*/
  if (sess.bucket.pendingReads >= bridge.PENDING_READS_MAX || globalPending >= bridge.GLOBAL_PENDING_READS_MAX) return E(-32005, 'pending', { reason: 'pending' });
  sess.bucket.pendingReads++; globalPending++;
  try {
    if (m.method === 'eth_getLogs') return await logClient(sess, m.params[0]);
    return { result: await sess.rpc(m.method, m.params) };
  } catch (e) { return transportError(e); }
  finally { sess.bucket.pendingReads--; globalPending--; }
}
/** LogClient.fetchAll: resolves tags, enforces the 1024-block range locally, then pocol_getLogs (never raw eth_getLogs). */
async function logClient(sess, f) {
  if (f.blockHash !== undefined) return { result: await sess.rpc('pocol_getLogs', [f], LOGS_TIMEOUT_MS) };
  const num = async (tag) => (tag.startsWith('0x') ? BigInt(tag) : BigInt((await sess.rpc('eth_getBlockByNumber', [tag, false])).number));
  const from = await num(f.fromBlock), to = await num(f.toBlock);
  if (to < from || to - from + 1n > BigInt(LOG_RANGE_MAX)) return E(-32020, 'range', { reason: 'range', limit: LOG_RANGE_MAX });
  return { result: await sess.rpc('pocol_getLogs', [{ ...f, fromBlock: '0x' + from.toString(16), toBlock: '0x' + to.toString(16) }], LOGS_TIMEOUT_MS) };
}
async function route(sess, m, ctl) {
  if (m.kind === 'rpc_read') return rpcRead(sess, m);
  if (m.kind === 'storage_set') {
    const q = qbytes(m.method, m.params);
    if (!admission.admit(sess.storeQ, q)) return E(-32005, 'store', { reason: 'store' });
    // A12: the admission slot is released when the storage operation settles, not when the frame
    // gets its reply (a storeTimeout reply can precede settlement; a never-settling write keeps it).
    let w;
    try { await booted; w = records.submit(sess.netKey, sess.site, m.method, m.params); } catch (e) { admission.release(sess.storeQ, q); throw e; }
    w.settled.then(() => admission.release(sess.storeQ, q));
    return await w.reply;
  }
  if (m.kind === 'nav') {
    if (m.method === 'site_navigate') {
      if (!bridge.navTake(sess.bucket, ctl.arrivalMs)) return E(-32005, 'nav', { reason: 'nav' });
      const n = normalize(m.params[0]);
      ctl.navigate(n === null ? { notFound: true, path: m.params[0] } : { path: n });
      return { suppressed: true }; // no reply to the torn-down frame
    }
    if (sess.confirmPending) return E(-32005, 'pending', { reason: 'pending' });
    sess.confirmPending = true;
    try {
      const okExt = await ctl.ask({ type: 'confirmExternal', url: m.params[0] });
      if (!okExt) return E(4001, 'user rejected');
      if (sess.closed) return E(4900, 'disconnected');
      await chrome.tabs.create({ url: m.params[0] }); return { result: null };
    } finally { sess.confirmPending = false; }
  }
  if (m.kind === 'wallet_req') {
    if (sess.walletPending) return E(-32005, 'pending', { reason: 'pending' });
    const now = Date.now(); sess.walletTimes = sess.walletTimes.filter((t) => now - t < 60000);
    if (sess.walletTimes.length >= WALLET_PER_MIN) return E(-32005, 'rate', { reason: 'wallet' });
    sess.walletTimes.push(now); sess.walletPending = true;
    try { return await wallet(sess, m, ctl.ask); } finally { sess.walletPending = false; }
  }
  return E(-32600, 'kind');
}
// The 15-minute lock is refreshed only by user actions (unlock, an approved grant or signature),
// never by site-driven calls (review finding: eth_accounts kept the seed decrypted).
async function walletKey(sess) { return isUnlocked() ? networkKey(unlocked.seed, sess.netKey) : null; }
const touchWallet = () => { if (unlocked) unlocked.until = Date.now() + LOCK_MS; };
async function wallet(sess, m, ask) {
  if (m.method === 'eth_chainId') return { result: '0x' + sess.profile.chainId.toString(16) };
  const ep = epochs();
  const key = await walletKey(sess);
  let err = changed(sess, ep); if (err) return err;
  if (!key) return E(4100, 'wallet locked');
  const address = addressOf(key);
  if (m.method === 'eth_accounts') return { result: sess.approved === address && !(await signEligible(sess, ep)) ? [address] : [] };
  if ((err = await signEligible(sess, ep))) return err;
  if (m.method === 'eth_requestAccounts') {
    if (sess.approved !== address) {
      if ((err = await approval(sess, ask, { type: 'confirmAccounts', site: sess.site, address, network: sess.profile.name }))) return err;
      if ((err = await signEligible(sess, ep))) return err;
      sess.approved = address; touchWallet();
    }
    return { result: [address] };
  }
  // eth_sendTransaction: build, show the unsigned bytes and signing hash bound to the network,
  // sign exactly those bytes only after approval and the checks under ProfileMutex, submit (WalletSubmit).
  if (sess.approved !== address) return E(4100, 'unauthorized');
  const o = m.params[0]; if (o.from && o.from.toLowerCase() !== address.toLowerCase()) return E(4100, 'from is not the granted account');
  const step = async (p) => { const v = await p; const c = changed(sess, ep); if (c) throw { reply: c }; return v; };
  // Endpoint quantities: hex strings of at most 256 bits (a negative or malformed value never reaches RLP).
  const qty = (v) => { if (typeof v !== 'string' || !/^0x[0-9a-fA-F]{1,64}$/.test(v)) throw { reply: E(-32603, 'endpoint returned a malformed quantity') }; return BigInt(v); };
  try {
    const nonce = o.nonce ? BigInt(o.nonce) : qty(await step(sess.rpc('eth_getTransactionCount', [address, 'pending'])));
    const blk = await step(sess.rpc('eth_getBlockByNumber', ['latest', false])); const base = qty(blk && blk.baseFeePerGas);
    const data = o.data || '0x', value = o.value ? BigInt(o.value) : 0n;
    const gas = o.gas ? BigInt(o.gas) : qty(await step(sess.rpc('eth_estimateGas', [{ from: address, to: o.to, data, value: '0x' + value.toString(16) }])));
    const maxPriority = o.maxPriorityFeePerGas ? BigInt(o.maxPriorityFeePerGas) : 1000000000n;
    const tx = { chainId: sess.profile.chainId, nonce, maxPriority, maxFee: o.maxFeePerGas ? BigInt(o.maxFeePerGas) : base * 2n + maxPriority, gas, to: o.to, value, data };
    const ut = unsignedTx(tx);
    // Every committed field is shown, with the worst-case cost gas·maxFee + value (review finding: fees were hidden).
    if ((err = await approval(sess, ask, { type: 'confirmTx', site: sess.site, network: sess.profile.name, chainId: tx.chainId, from: address, payload: toHex(ut.payload), signingHash: toHex(ut.signingHash), to: o.to, value: value.toString(), data, gas: gas.toString(), nonce: nonce.toString(),
      maxFeePerGas: tx.maxFee.toString(), maxPriorityFeePerGas: tx.maxPriority.toString(), maxCost: (gas * tx.maxFee + value).toString() }))) return err;
    const s = await profileLocked(async () => {
      if ((err = await signEligible(sess, ep))) return err;
      if ((err = await identity(sess))) return err;
      if ((err = await signEligible(sess, ep))) return err;
      const k = await walletKey(sess);
      if ((err = changed(sess, ep))) return err;
      if (!k || addressOf(k) !== address) return E(4100, 'wallet changed');
      const signed = signUnsigned(k, ut);
      touchWallet();
      return { signed };
    });
    // Submission runs outside ProfileMutex: a profile change requested meanwhile proceeds and
    // cancels the submission if it has not been initiated yet.
    return s.signed ? await submit(sess, ep, address, s.signed.raw) : s;
  } catch (e) { return e && e.reply ? e.reply : transportError(e); }
}
/** WalletSubmit (A06 round 2): signed -> queued for transport -> request initiated -> broadcast.
 *  - Until the request is initiated, a wallet lock or change (4100), a profile change or
 *    revocation (4901) or a disconnect (4900) cancels it: nothing is sent, and the error says so
 *    ({submission: 'cancelled', before: 'signed'|'queued'}).
 *  - After the transport reservation, eligibility is re-checked synchronously immediately before
 *    the request starts (the wallet lock timeout has no event of its own).
 *  - Once initiated, the transaction may reach the node and be broadcast: it is never reported as
 *    cancelled. A transport failure then says {submission: 'initiated'} (outcome unknown); a
 *    result is the broadcast transaction hash; a node error means the node did not accept it. */
async function submit(sess, ep, address, raw) {
  const sub = { state: 'signed', cancelled: null }; const abort = new AbortController();
  const cancel = (r) => { if (!sub.cancelled && (sub.state === 'signed' || sub.state === 'queued')) { sub.cancelled = E(r.error.code, r.error.message, { ...(r.error.data || {}), submission: 'cancelled', before: sub.state }); abort.abort(); } return !!sub.cancelled; };
  const hook = { sess, cancel: (code, message) => cancel(E(code, message)) };
  const ineligible = () => changed(sess, ep) || (!isUnlocked() ? E(4100, 'wallet locked') : null) || (sess.approved !== address ? E(4100, 'unauthorized') : null);
  const recheck = () => { const c = ineligible(); return c ? cancel(c) : !!sub.cancelled; };
  approvals.add(hook);
  try {
    if (recheck()) return sub.cancelled;
    const result = await sess.rpc('eth_sendRawTransaction', [raw], RPC_TIMEOUT_MS, { signal: abort.signal, beforeSend: recheck, onState: (st) => { sub.state = st; } });
    sub.state = 'broadcast';
    return { result };
  } catch (e) {
    if (sub.cancelled) return sub.cancelled;
    if (e.rpc) return { error: e.rpc };
    const t = transportError(e); t.error.data = { ...(t.error.data || {}), submission: sub.state === 'initiated' ? 'initiated' : 'notSent' };
    return t;
  } finally { approvals.delete(hook); }
}

// ---------------- bridge port: one per viewer tab session ----------------
chrome.runtime.onConnect.addListener((port) => {
  if (!port.name.startsWith('bridge:')) return;
  const sid = port.name.slice(7); const sess = sessions.get(sid);
  if (!sess || !port.sender || !port.sender.tab || port.sender.tab.id !== sess.tabId) { port.disconnect(); return; }
  let asks = 0; const waiting = new Map(); let alive = true;
  const post = (m) => { if (alive) port.postMessage(m); };
  post({ hello: genId }); // D103 section 5: the viewer tears frames down if the worker generation changes
  // ask(q) resolves true only on an explicit approval; dismiss() withdraws the dialog (A06 cancellation).
  const ask = (q) => {
    const id = ++asks; let resolve; const p = new Promise((r) => { resolve = r; }); waiting.set(id, resolve); post({ ask: id, q });
    p.dismiss = () => { if (waiting.delete(id)) { post({ dismiss: id }); resolve(false); } };
    return p;
  };
  // After a revocation the tab keeps its DNR rules for REVOKE_DNR_GRACE_MS while the viewer tears
  // the frame down (never CSP alone), then they go as on any session end.
  const end = (revoked = false) => {
    alive = false; sess.closed = true; clearInterval(tick); cancelApprovals((s) => s === sess, 4900, 'disconnected');
    for (const r of waiting.values()) r(false); waiting.clear(); sessions.delete(sid);
    if (revoked) setTimeout(() => { removeDnr(sess.tabId).catch(() => {}); }, REVOKE_DNR_GRACE_MS); else removeDnr(sess.tabId).catch(() => {});
  };
  // A changed identity or profile tears the frame down and cancels the session's requests (4901);
  // their replies are flushed before the port closes.
  sess.revoke = (code, message) => {
    if (!alive || sess.closed) return; sess.closed = true; post({ revoked: { code, message } }); cancelApprovals((s) => s === sess, code, message);
    setTimeout(() => { if (alive) { end(true); port.disconnect(); } }, 0);
  };
  const tick = setInterval(() => { if (!sess.checking) { sess.checking = true; identity(sess).finally(() => { sess.checking = false; }); } isUnlocked(); }, IDENTITY_CHECK_MS);
  port.onMessage.addListener((msg) => {
    if (msg && msg.answer !== undefined) { const r = waiting.get(msg.answer); if (r) { waiting.delete(msg.answer); r(msg.ok === true); } return; }
    const arrivalMs = Math.floor(performance.now()); // stamped on receipt, before any check (D97)
    if (!msg || typeof msg !== 'object' || !Number.isInteger(msg.seq) || !Number.isInteger(msg.frame)) return;
    // Bpre-B3 run synchronously in arrival order; routing never blocks the next message.
    const c = bridge.check(sess.bucket, msg.text, arrivalMs);
    if (!c.ok) { post({ seq: msg.seq, frame: msg.frame, reply: c.reply }); return; }
    const ctl = { arrivalMs, ask, navigate: (to) => post({ navigate: to, frame: msg.frame }) };
    route(sess, c, ctl).then((r) => { if (!r.suppressed) post({ seq: msg.seq, frame: msg.frame, reply: { id: c.id, ...r } }); },
      () => post({ seq: msg.seq, frame: msg.frame, reply: { id: c.id, error: { code: -32603, message: 'internal' } } }));
  });
  port.onDisconnect.addListener(() => { if (alive) end(); });
});

// ---------------- messages from extension pages ----------------
chrome.runtime.onMessage.addListener((msg, sender, reply) => {
  if (sender.id !== chrome.runtime.id) return false;
  (async () => {
    switch (msg.type) {
      case 'profiles.list': return { ok: true, profiles: await profiles() };
      case 'profiles.add': return profileLocked(async () => {
        const p = validateProfile(msg.profile); profilesChanging(); const list = (await storedProfiles()).filter((x) => x.name !== p.name);
        list.push(p); try { await store.set({ profiles: list }); } finally { await profilesChanged(); }
        return { ok: true, profile: (await profiles()).find((x) => x.name === p.name) }; });
      case 'profiles.remove': return profileLocked(async () => {
        profilesChanging();
        try { await store.set({ profiles: (await storedProfiles()).filter((x) => x.name !== msg.name) }); } finally { await profilesChanged(); }
        return { ok: true }; });
      case 'open': return await openSite({ ...msg, tabId: sender.tab ? sender.tab.id : undefined });
      case 'session.storage': { const s = sessions.get(msg.sessionId); if (!s || !sender.tab || sender.tab.id !== s.tabId) return { ok: false }; await booted; const snap = await records.load(s.netKey, s.site); return { ok: true, storage: snap.dict, storageError: snap.error || null }; }
      case 'wallet.create': { const seed = msg.seedHex ? fromHex(msg.seedHex) : crypto.getRandomValues(new Uint8Array(32)); if (seed.length !== 32) throw new Error('seed');
        lockWallet('wallet replaced'); await store.set({ wallet: await encryptSeed(seed, msg.password) }); walletChanged('wallet replaced'); unlocked = { seed, until: Date.now() + LOCK_MS }; return { ok: true }; }
      case 'wallet.unlock': { const e0 = walletEpoch; const w = (await store.get('wallet')).wallet; if (!w) return { ok: false, error: 'no wallet' }; const seed = await decryptSeed(w, msg.password);
        // A wallet.create (or lock) during the decryption wins: the old seed is never reinstated.
        if (walletEpoch !== e0) return { ok: false, error: 'wallet changed during unlock' };
        walletChanged('wallet unlocked again'); unlocked = { seed, until: Date.now() + LOCK_MS }; return { ok: true }; }
      case 'wallet.lock': lockWallet('wallet locked'); return { ok: true };
      case 'wallet.status': return { ok: true, exists: !!(await store.get('wallet')).wallet, unlocked: isUnlocked() };
      case 'wallet.address': { const p = (await profiles()).find((x) => x.name === msg.name); if (!p || p.invalid || !isUnlocked()) return { ok: false }; const seed = unlocked.seed; return { ok: true, address: addressOf(await networkKey(seed, p.netKey)) }; }
      default: return { ok: false, error: 'unknown message' };
    }
  })().then(reply, (e) => reply({ ok: false, error: { code: e.code || -32603, message: String(e.message || e) } }));
  return true;
});

chrome.action.onClicked.addListener(() => chrome.runtime.openOptionsPage());

// omnibox: 'pocol <profile>/<0xaddr>[/path]'
chrome.omnibox.onInputEntered.addListener((text) => {
  chrome.tabs.update({ url: chrome.runtime.getURL('viewer.html') + '#' + encodeURI(text.trim()) });
});
