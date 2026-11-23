// PoCol extension service worker: network profiles, verified site loading
// (RP window or LN trust), BridgeAuth for site messages, DNR isolation, wallet.
import { decodeSpec, verifyWindow, cpv } from './lib/rp.js';
import { normalizeForkSchedule } from './lib/forks.js';
import { fetchSite, normalize, chunkWidth } from './lib/site.js';
import { withRetry, IDEMPOTENT_READS } from './lib/retry.js';
import * as bridge from './lib/bridge.js';
import { StoreAdmission, qbytes } from './lib/sitestore.js';
import { RecordStore } from './lib/records.js';
import * as http from './lib/transport.js';
import { encryptSeed, decryptSeed, networkKey, addressOf, unsignedTx, signUnsigned, LOCK_MS } from './lib/wallet.js';
import { keccak } from './lib/crypto.js';
import * as rlp from './lib/rlp.js';
import { toHex, fromHex, eq } from './lib/hex.js';
import { NETWORK, isStudioOrigin, validateEndpoint, trustMessage } from './lib/studio-network.js';
// The D1 mark is read through the namespace: the identity switch removes that export (set_network_identity.py).
import * as studioNetwork from './lib/studio-network.js';
import { importKeyfile, exportKeyfile } from './lib/studio-keyfile.js';
import * as acct from './lib/account.js';
import { identityDifference, describeDifference } from './lib/identity.js';
import { readNetworkFees, ABSOLUTE_CEILING } from './lib/fees.js';
import { JOB_PREFIX, JOB_TTL_MS, validatePublishRequest, validateSiteTarget, publicJob } from './lib/studio-bridge.js';
import { IDE_JOB_PREFIX, isIdeOrigin, validateContractRequest, publicContractJob } from './lib/contracts.js';
import { networkOf, blocksAccount, archiveForeignJobs, removeSideKeys } from './lib/jobs.js';
import { LOCK_SIGNAL_KEY, withVaultLock } from './lib/vault-guard.js';

const store = chrome.storage.local;
// Final review (2026-11-22): Chrome lets content scripts use storage.local by default, and the extension's content
// scripts on studio.pocol.net and ide.pocol.net (studio-content.js, ide-content.js) never need it. Only the extension's
// own pages and this worker may read or write the vault, the history, the profiles and the jobs. It is set on every
// worker start (an install, an update, a browser restart), before any listener is registered.
export const STORAGE_ACCESS = Object.freeze({ accessLevel: 'TRUSTED_CONTEXTS' });
export const storageRestricted = (async () => { try { await store.setAccessLevel(STORAGE_ACCESS); return true; } catch { return false; } })();
// F11: minimum seed-encryption password length, enforced in the worker (the security boundary), to
// match the Studio key-file export. The viewer wallet cannot export its seed, so a weak password or a
// silent replace is unrecoverable.
const WALLET_PW_MIN = 10;
// Site storage records with a worker-generation fence (D103); no data write before the fence confirms.
const records = new RecordStore(store);
const booted = records.boot().catch(() => null);
const genId = crypto.randomUUID();
const admission = new StoreAdmission();
const sessions = new Map(); // sessionId -> SiteSession (D99)
let globalPending = 0;
let unlocked = null; // {seed, until}
let walletCreating = false;
const RPC_TIMEOUT_MS = 10000, LOGS_TIMEOUT_MS = 30000, LOG_RANGE_MAX = 1024;
const WALLET_PER_MIN = 5;
const IDENTITY_CHECK_MS = 30000; // browser.md: identity at open, every 30 s and at signing
const REVOKE_DNR_GRACE_MS = 2000;

// ---------------- profiles ----------------
export function netKeyOf(chainId, genesisHash) {
  return toHex(keccak(rlp.encode([new TextEncoder().encode('PoCol-net-v1'), rlp.encUint(BigInt(chainId)), genesisHash])));
}
/** The salt of a profile's wallet key (D14): its netKey, for the built-in profile too, so a wallet's address
 *  belongs to one network. A custom profile always keeps its own netKey, so no extension update moves its address. */
export const walletSalt = (p) => p.netKey;
// The built-in profile's fork schedule is pinned by this release: [[1,"0"]], what the fresh network's genesis metadata
// carries (pocold testnet.rs and main.rs write "forks":[[1,0]]; lib.rs maps a missing forks key to [(1,0)]). Neither
// storage nor RPC can change it. A cap raise ([[1,"0"],[2,H'],[3,H]]) ships as an extension release with the node release.
// EXT-01 (2026-11-22): the viewer seed wallet (`wallet.*`, D14) has no export and no import, and chrome.storage.local is
// deleted with the extension, so a key it holds is lost on uninstall or an extension-id change. It therefore serves local
// networks only: never the built-in public profile (trust level RN) nor a custom profile of the public network's
// identity (the same netKey), so it never shows or signs for an address on the public PoCol network. The PoCol wallet
// (`account.*`, the exportable `studio:wallet` vault) is the wallet of the public network.
export const PUBLIC_NET_KEY = netKeyOf(NETWORK.chainId, fromHex(NETWORK.genesisHash));
export const seedWalletAllowed = (p) => !!p && p.trustLevel !== 'RN' && p.netKey !== PUBLIC_NET_KEY;
const SEED_LOCAL_ONLY = 'The viewer wallet is for local networks only: it cannot be exported, so it never holds an address on the public PoCol network';
export const PUBLIC_FORK_SCHEDULE = Object.freeze(normalizeForkSchedule([[1, '0']]).map((entry) => Object.freeze(entry)));
const publicProfile = () => ({ ...NETWORK, forkSchedule: PUBLIC_FORK_SCHEDULE });
const storedProfiles = async () => {
  const existing = (await store.get('profiles')).profiles || [];
  // The reserved built-in name cannot be shadowed by an imported profile.
  return [...existing.filter((p) => p.name !== NETWORK.name), publicProfile()];
};
/** Stored profiles re-validated on every read (A06/A07): a profile that no longer passes
 *  validation is listed with `invalid` and is unusable; `conflict` is recomputed over the whole
 *  set, so conflicting genesis identities or activation schedules are read-only. */
async function profiles() {
  const out = [];
  for (const p of await storedProfiles()) { try { out.push(validateProfile(p)); } catch (e) { out.push({ name: String(p && p.name), invalid: String(e.message || e) }); } }
  for (const p of out) if (!p.invalid) p.conflict = out.some((x) => x !== p && !x.invalid && x.chainId === p.chainId && (x.genesisHash !== p.genesisHash || JSON.stringify(x.forkSchedule) !== JSON.stringify(p.forkSchedule)));
  return out;
}
/** Profile acceptance (browser.md D58/D77/D79): decode genesisPre, hash, chainId. */
function validateProfile(p) {
  if (!/^[A-Za-z0-9_-]{1,32}$/.test(p.name)) throw new Error('name');
  if (!['DEV', 'LN', 'RP', 'RN'].includes(p.trustLevel)) throw new Error('trustLevel');
  validateEndpoint(p.endpoint, p.trustLevel);
  const pre = fromHex(p.genesisPre); const spec = decodeSpec(pre);
  // As pocold's admission (R24), with its reasons: a spec-1 genesis and a bounded nonce range are the retired rule set,
  // which no node of this release runs. Such a GenesisSpec still decodes (refdec.js), but is no profile.
  if (spec.specVersion < 2) throw new Error(`GenesisSpec version ${spec.specVersion}: the spec-1 rule set is retired; this release admits only a specVersion-2 (v3) genesis`);
  if (cpv(spec, 'nonceMode') !== 1n) throw new Error(`nonceMode ${cpv(spec, 'nonceMode')}: bounded nonce ranges are a retired spec-1 rule; a v3 genesis has nonceMode 1, the full 64-bit nonce domain`);
  if (!eq(spec.genesisHash, fromHex(p.genesisHash))) throw new Error('genesisHash mismatch');
  if (spec.chainId !== BigInt(p.chainId)) throw new Error('chainId mismatch');
  return { name: p.name, chainId: Number(p.chainId), genesisPre: p.genesisPre, genesisHash: p.genesisHash.toLowerCase(), endpoint: p.endpoint, trustLevel: p.trustLevel, forkSchedule: normalizeForkSchedule(p.forkSchedule), netKey: netKeyOf(p.chainId, spec.genesisHash) };
}
// HttpTransport pools (D98): trust checks never starve behind sites; chunk fetches have their own pool.
const pools = { site: new http.Pool(http.SITE_POOL), trust: new http.Pool(http.TRUST_POOL), content: new http.Pool(http.CONTENT_POOL) };
let reqId = 0;
// One attempt of rpcFor's `once(method, params, timeoutMs, stages)` takes at most the pool reservation wait plus its
// timeout; the retry deadline counts that, and an attempt that would end past it is shortened (lib/retry.js).
const RETRY_BUDGET = Object.freeze({
  attemptMs: (method, params, timeoutMs = RPC_TIMEOUT_MS) => http.RESERVE_WAIT_MS + timeoutMs,
  shorten: ([, stages], ms) => [ms - http.RESERVE_WAIT_MS, stages],
  minAttemptMs: http.RESERVE_WAIT_MS + 1000,
});
// Reads a site asks for (rpc_read, and the reads of its wallet requests) have a tighter budget than the extension's
// own, so a busy gateway cannot let one tab hold the global pending-read slots for long (bridge.js
// GLOBAL_PENDING_READS_MAX), and they stop when the session is closed or revoked (`aborted`).
const SITE_READ_RETRY = Object.freeze({ attempts: 4, deadlineMs: 20000 });
/** rpc for one pool; session (optional) carries the per-session SESSION_RECV_MAX account. Idempotent reads are
 *  retried on 429, -32021, 502/503/504, timeouts and dropped connections (W2, lib/retry.js); eth_sendRawTransaction
 *  never is. `retry`: true (RETRY), false (no retry) or withRetry options; an `aborted()` among them also cancels a
 *  read still waiting for its pool reservation, so nothing is sent once it is true. */
function rpcFor(profile, pool, session = null, shared = http.inflight, retry = true) {
  const opts = retry && typeof retry === 'object' ? retry : {}, aborted = opts.aborted || null;
  const once = (method, params, timeoutMs = RPC_TIMEOUT_MS, stages = {}) => {
    const cancel = aborted && IDEMPOTENT_READS.has(method) ? { beforeSend: () => aborted() || !!(stages.beforeSend && stages.beforeSend()) } : {};
    return http.request(profile.endpoint, method, params, { ...stages, ...cancel, pool, session, deadline: timeoutMs, id: ++reqId, shared: pool === pools.trust ? null : shared });
  };
  return retry ? withRetry(once, { ...RETRY_BUDGET, ...opts }) : once;
}
// The verified site fetch retries its own reads (fetchSite, with the chunk-level pool), so it gets them unwrapped.
const contentRpc = (profile) => { const c = rpcFor(profile, pools.content, null, http.inflight, false), s = rpcFor(profile, pools.site, null, http.inflight, false); return (m, p, ...r) => (m === 'eth_getCode' ? c(m, p, ...r) : s(m, p, ...r)); };
/** netKey check against the endpoint: chainId and genesisHash must equal the profile. */
// The refusal names the difference (lib/identity.js): what the profile pins and what the endpoint answered.
async function checkNetwork(profile, rpc) {
  const cid = await rpc('eth_chainId', []);
  let diff = identityDifference(profile, cid, profile.genesisHash);
  if (diff) throw Object.assign(new Error(`endpoint chainId differs from profile (${describeDifference(diff)})`), { code: 4901, difference: diff });
  const st = await rpc('pocol_status', []);
  diff = identityDifference(profile, cid, st?.genesisHash);
  if (diff) throw Object.assign(new Error(`endpoint genesis differs from profile (${describeDifference(diff)})`), { code: 4901, difference: diff });
}

// ---------------- signing eligibility (browser.md "Signing eligibility", A06) ----------------
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
// The PoCol wallet's signed transfers not yet sent (confirmTransfer): a lock or a profile change aborts the queued request.
const transfersQueued = new Set(); // AbortController
const abortTransfers = () => { for (const c of [...transfersQueued]) c.abort(); };
function walletChanged(message) { walletEpoch++; cancelApprovals(() => true, 4100, message); }
const lockWallet = (message) => { unlocked = null; walletChanged(message); };
const isUnlocked = () => { if (unlocked && Date.now() > unlocked.until) lockWallet('wallet locked (timeout)'); return !!unlocked; };
const PROFILE_FIELDS = ['name', 'chainId', 'genesisPre', 'genesisHash', 'endpoint', 'trustLevel', 'netKey', 'forkSchedule'];
const sameProfile = (a, b) => PROFILE_FIELDS.every((k) => k === 'forkSchedule' ? JSON.stringify(a[k]) === JSON.stringify(b[k]) : a[k] === b[k]);
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
  if (!seedWalletAllowed(cur)) return E(4100, SEED_LOCAL_ONLY);
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
function profilesChanging() { eligibilityEpoch++; cancelApprovals(() => true, 4901, 'network profiles changing'); abortTransfers(); }
/** After a profile change (under ProfileMutex): sessions whose profile was removed, changed or
 *  invalidated end with 4901; a session whose profile became conflicting stays read-only. */
async function profilesChanged() {
  eligibilityEpoch++; cancelApprovals(() => true, 4901, 'network profiles changed'); abortTransfers();
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
  let trust = { level: profile.trustLevel, message: trustMessage(profile.trustLevel) };
  if (profile.trustLevel === 'RP') {
    const spec = decodeSpec(fromHex(profile.genesisPre));
    try { trust = { level: 'RP', ...(await verifyWindow(rpc, spec, undefined, profile.forkSchedule)) }; }
    catch (e) { return { ok: false, error: { code: 4901, message: 'RP window rejected: ' + (e.flag || e.message) } }; }
    if (!trust.ok) return { ok: false, error: { code: 4901, message: trust.message } };
  }
  const s = await fetchSite(contentRpc(profile), site, { parallel: chunkWidth(profile.trustLevel), retry: RETRY_BUDGET });
  const files = {}; for (const [p, f] of s.files) files[p] = { mime: f.mime, b64: b64(f.data) };
  const sessionId = crypto.randomUUID(); const now = Math.floor(performance.now());
  const recv = { recv: 0 };
  // A profile change during the awaits above was not seen by profilesChanged (the session did not
  // exist yet): refuse rather than register a session on a removed or changed profile.
  if (eligibilityEpoch !== ep0) return { ok: false, error: { code: 4901, message: 'network profiles changed while the site was opening; open it again' } };
  // The session freezes its profile (without the recomputed conflict flag) for signEligible.
  const frozen = Object.freeze(Object.fromEntries(PROFILE_FIELDS.map((k) => [k, profile[k]])));
  const sess = { profile: frozen, site: site.toLowerCase(), netKey: profile.netKey, bucket: bridge.newSession(now), approved: null, rpc: rpcFor(profile, pools.site, recv, http.inflight, { ...SITE_READ_RETRY, aborted: () => sess.closed }), tabId, trust,
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
  if (e.transport === 'rate') return E(-32005, 'rate', { reason: 'rate' });
  if (e.transport === 'status') return E(-32603, 'status', Number.isInteger(e.status) ? { reason: 'status', status: e.status } : { reason: 'status' });
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
async function walletKey(sess) { return isUnlocked() ? networkKey(unlocked.seed, walletSalt(sess.profile)) : null; }
const touchWallet = () => { if (unlocked) unlocked.until = Date.now() + LOCK_MS; };
async function wallet(sess, m, ask) {
  if (m.method === 'eth_chainId') return { result: '0x' + sess.profile.chainId.toString(16) };
  // EXT-01: no account, dialog or signature on the public network from the seed wallet.
  if (!seedWalletAllowed(sess.profile)) return m.method === 'eth_accounts' ? { result: [] } : E(4100, SEED_LOCAL_ONLY);
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
  if (!globalThis.navigator?.locks) return E(4200, 'Browser does not support safe wallet transaction coordination');
  return navigator.locks.request(`pocol:publish:${sess.profile.chainId}:${address}`, {mode:'exclusive',ifAvailable:true}, async (lease) => {
  if (!lease) return E(-32005, 'Another tab is publishing from this wallet');
  const allJobs = await store.get(null);
  // Sites W4: a job of another network (lib/jobs.js) never blocks this wallet.
  if (Object.entries(allJobs).some(([k,j]) => (k.startsWith(JOB_PREFIX)||k.startsWith(IDE_JOB_PREFIX)) && blocksAccount(j, address, NETWORK))) return E(-32005, 'Resume the unfinished PoCol operation before sending another transaction');
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
  });
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
const studioLimits = new Map();
let studioQueue = Promise.resolve();
const studioLocked = (fn) => { const p = studioQueue.then(fn); studioQueue = p.catch(() => {}); return p; };
async function collectStudioJob(key, previous, eligible) {
  // A publisher can persist its first signed transaction after a cleanup snapshot.
  // Use its job lock and re-read before deleting, so durable recovery state survives.
  if (!globalThis.navigator?.locks) return previous;
  return navigator.locks.request(`pocol:job:${key.slice(JOB_PREFIX.length)}`, {mode:'exclusive',ifAvailable:true}, async (lease) => {
    if (!lease) return previous;
    const current = (await store.get(key))[key];
    if (!current || eligible(current)) { await store.remove(key); await removeSideKeys(store, key); return null; }
    return current;
  });
}
async function studioRequest(msg, sender) {
  if (!sender.tab || !Number.isInteger(sender.tab.id) || sender.frameId !== 0 || !isStudioOrigin(sender.url)) throw new Error('Studio origin not allowed');
  const origin = new URL(sender.url).origin;
  const payload = msg.payload || {};
  if (msg.type === 'studio.status') {
    const all = await store.get(null);
    const jobs = Object.entries(all).filter(([k,j]) => k.startsWith(JOB_PREFIX) && j?.origin === origin && j.sourceTabId === sender.tab.id).map(([,j]) => ({...publicJob(j),title:j.request?.title,createdAt:j.createdAt,site:j.result?.site || j.request?.site || null})).sort((a,b)=>b.createdAt-a.createdAt).slice(0,20);
    return { version: chrome.runtime.getManifest().version, capabilities: ['publish', 'openSite', 'jobStatus'], network: publicProfile(), jobs };
  }
  if (msg.type === 'studio.jobStatus') {
    if (typeof payload.jobId !== 'string' || !/^[a-f0-9-]{36}$/.test(payload.jobId)) throw new Error('Invalid job');
    const job = (await store.get(JOB_PREFIX + payload.jobId))[JOB_PREFIX + payload.jobId];
    if (!job || job.origin !== origin || job.sourceTabId !== sender.tab.id) throw new Error('This publish job belongs to another tab');
    return publicJob(job);
  }
  if (!['studio.openPublisher', 'studio.openSite'].includes(msg.type)) throw new Error('Unsupported Studio action');
  const times = (studioLimits.get(sender.tab.id) || []).filter((t) => t > Date.now() - 60000);
  if (times.length >= 6) throw new Error('Please wait before opening another PoCol tab');
  times.push(Date.now()); studioLimits.set(sender.tab.id, times);
  if (msg.type === 'studio.openSite') {
    const target = validateSiteTarget(payload);
    // EXT-08: the tab opens on a confirmation (viewer.html?confirm=studio): the site is fetched and its code runs only
    // after the user's own click there, so a Studio page cannot start on-chain site code without the user.
    await chrome.tabs.create({ url: chrome.runtime.getURL('viewer.html') + '?confirm=studio#' + NETWORK.name + '/' + target.site + target.path });
    return { opened: true };
  }
  return studioLocked(async () => {
    const request = validatePublishRequest(payload);
    const all = await store.get(null); const active = [];
    for (const [k, v] of Object.entries(all)) if (k.startsWith(JOB_PREFIX)) {
      const current = await collectStudioJob(k, v, (job) => {
        const hasSigned = Object.keys(job.checkpoint?.transactions || {}).length > 0;
        return Date.now() - job.createdAt > JOB_TTL_MS && (!hasSigned || job.status === 'complete');
      });
      if (current) active.push([k, current]);
    }
    // Retain resumable operations. Never silently overwrite a pending publish.
    if (active.filter(([, j]) => !['complete', 'cancelled'].includes(j.status)).length >= 5) throw new Error('Close or finish existing publish requests before creating another');
    const completed = active.filter(([, j]) => ['complete', 'cancelled'].includes(j.status)).sort((a, b) => a[1].createdAt - b[1].createdAt);
    while (completed.length > 15) {
      const [key, previous] = completed.shift();
      await collectStudioJob(key, previous, (job) => ['complete', 'cancelled'].includes(job.status));
    }
    const id = crypto.randomUUID();
    const job = { id, origin, sourceTabId: sender.tab.id, createdAt: Date.now(), status: 'awaitingApproval', network: networkOf(NETWORK), request };
    await store.set({ [JOB_PREFIX + id]: job });
    try { await chrome.tabs.create({ url: chrome.runtime.getURL('publisher.html') + '#' + id }); }
    catch (e) { await store.remove(JOB_PREFIX + id); throw e; }
    return { jobId: id };
  });
}
async function ideRequest(msg,sender) {
  if(!sender.tab || !Number.isInteger(sender.tab.id) || sender.frameId!==0 || !isIdeOrigin(sender.url))throw new Error('IDE origin not allowed');
  const origin=new URL(sender.url).origin,payload=msg.payload||{};
  if(msg.type==='ide.status') {
    const all=await store.get(null);
    const jobs=Object.entries(all).filter(([k,j])=>k.startsWith(IDE_JOB_PREFIX)&&j?.origin===origin&&j.sourceTabId===sender.tab.id).map(([,j])=>({...publicContractJob(j),title:j.request?.title,createdAt:j.createdAt})).sort((a,b)=>b.createdAt-a.createdAt).slice(0,20);
    return {version:chrome.runtime.getManifest().version,capabilities:['contractTransactions','jobStatus'],network:publicProfile(),jobs};
  }
  if(msg.type==='ide.jobStatus') {
    if(typeof payload.jobId!=='string'||!/^[a-f0-9-]{36}$/.test(payload.jobId))throw new Error('Invalid contract job');
    const job=(await store.get(IDE_JOB_PREFIX+payload.jobId))[IDE_JOB_PREFIX+payload.jobId];
    if(!job||job.origin!==origin||job.sourceTabId!==sender.tab.id)throw new Error('This contract job belongs to another tab');
    return publicContractJob(job);
  }
  if(msg.type!=='ide.openContract')throw new Error('Unsupported IDE action');
  const times=(studioLimits.get(sender.tab.id)||[]).filter(t=>t>Date.now()-60000);
  if(times.length>=6)throw new Error('Please wait before opening another PoCol confirmation');
  times.push(Date.now());studioLimits.set(sender.tab.id,times);
  return studioLocked(async()=>{
    const request=validateContractRequest(payload),all=await store.get(null),active=[];
    for(const [key,previous]of Object.entries(all))if(key.startsWith(IDE_JOB_PREFIX)) {
      const current=await navigator.locks.request(`pocol:job:${key.slice(IDE_JOB_PREFIX.length)}`,{mode:'exclusive',ifAvailable:true},async lease=>{
        if(!lease)return previous;
        const job=(await store.get(key))[key];if(!job)return null;
        const signed=Object.keys(job.checkpoint?.transactions||{}).length>0;
        if(Date.now()-job.createdAt>JOB_TTL_MS&&(!signed||['complete','cancelled'].includes(job.status))){await store.remove(key);return null;}
        return job;
      });if(current)active.push(current);
    }
    if(active.filter(job=>!['complete','cancelled'].includes(job.status)).length>=5)throw new Error('Finish or cancel existing contract requests before opening another');
    const id=crypto.randomUUID(),job={id,kind:'contract',origin,sourceTabId:sender.tab.id,createdAt:Date.now(),status:'awaitingApproval',network:networkOf(NETWORK),request};
    await store.set({[IDE_JOB_PREFIX+id]:job});
    try{await chrome.tabs.create({url:chrome.runtime.getURL('contract.html')+'#'+id});}
    catch(error){await store.remove(IDE_JOB_PREFIX+id);throw error;}
    return {jobId:id};
  });
}
// ---------------- the PoCol wallet (account.*, EXT-02/04/05) ----------------
// One account: the `studio:wallet` vault (lib/studio-keyfile.js, the format Studio, IDE and the native-wallet export
// helpers use), decrypted only here, in the worker, while unlocked. Its key leaves the worker only inside the
// password-encrypted vault, when the user creates a wallet (the forced backup) or asks for a backup. Transfers are
// built here from the profile endpoint, shown in send.html, and signed here only after that page's explicit
// confirmation (send.prepare, then send.confirm with the review id), with the profile's chain ID in the signed
// EIP-1559 payload. The built-in profile is refused while this build pins the D1 placeholder (no network request then).
const VAULT_KEY = 'studio:wallet', HISTORY_KEY = 'wallet:history', DISCLOSURE_KEY = 'wallet:disclosure', DISCLOSURE_VERSION = 1;
const HISTORY_MAX = 100, REFRESH_MAX = 20, SEND_REVIEW_MS = 2 * 60 * 1000, TRANSFER_GAS_MAX = 1000000n;
const PW_FAIL_WINDOW_MS = 60000, PW_FAIL_MAX = 5;
// The placeholder refusal; the testGateway test build pins a devnet as the built-in profile, which is no placeholder.
export const PLACEHOLDER_BUILD = /*@fault testGateway*/studioNetwork.NETWORK_PLACEHOLDER === true/*@else*/false/*@end*/;
let account = null; // {secret, address, until}
let accountEpoch = 0, accountBusy = false, pwFails = [];
const reviews = new Map(); // review id -> a prepared transfer awaiting send.html's confirmation
const PAGES = Object.freeze({ manage: ['options.html'], unlock: ['options.html', 'popup.html', 'send.html'], read: ['options.html', 'popup.html', 'send.html'], send: ['send.html'] });
/** The extension page a message came from (its path), or null for anything else. */
function pageOf(sender) {
  const base = chrome.runtime.getURL('');
  return typeof sender?.url === 'string' && sender.url.startsWith(base) ? sender.url.slice(base.length).split(/[?#]/)[0] : null;
}
const fail = (message, code = 'refused', extra = {}) => Object.assign(new Error(message), { code, ...extra });
function fromPages(sender, pages) { const page = pageOf(sender); if (!page || !pages.includes(page)) throw fail('This wallet operation is available only in the extension’s own wallet pages', 4100); return page; }
function lockAccount(why) {
  if (account) account.secret.fill(0);
  account = null; accountEpoch++;
  for (const r of reviews.values()) r.cancelled = why;
  reviews.clear(); abortTransfers();
}
const accountUnlocked = () => { if (account && Date.now() > account.until) lockAccount('timeout'); return !!account; };
const touchAccount = () => { if (account) account.until = Date.now() + LOCK_MS; };
const storedVault = async () => (await store.get(VAULT_KEY))[VAULT_KEY] || null;
const vaultAddress = (v) => (v && typeof v.address === 'string' && /^0x[0-9a-f]{40}$/.test(v.address) ? v.address : null);
// A vault written by a Studio or IDE page (import, create) replaces the account: an unlocked copy of another key locks.
chrome.storage.onChanged?.addListener((changes, area) => {
  if (area === 'local' && changes[VAULT_KEY] && account && vaultAddress(changes[VAULT_KEY].newValue) !== account.address) lockAccount('wallet changed');
});
/** Failed password attempts (unlock, backup): at most PW_FAIL_MAX a minute; PBKDF2 600000 already costs each one. */
function passwordGate() { const now = Date.now(); pwFails = pwFails.filter((t) => now - t < PW_FAIL_WINDOW_MS); if (pwFails.length >= PW_FAIL_MAX) throw fail('Too many wrong passwords; wait a minute and try again', 'rate'); }
async function openVault(password) {
  const v = await storedVault(); if (!v) throw fail('No PoCol wallet is saved in this browser yet', 'none');
  passwordGate();
  if (typeof password !== 'string' || !password) throw fail('Enter the wallet password', 'password');
  try { return { vault: v, secret: await importKeyfile(new TextEncoder().encode(JSON.stringify(v)), password) }; }
  catch { pwFails.push(Date.now()); throw fail('Wrong password, or the saved wallet is damaged', 'password'); }
}
/** Saves `secret` as the vault under `password` and unlocks it. A different saved address is replaced only with
 *  replace:true (the settings page asks the user to back it up first), and replace:true covers only the wallet saved
 *  when this started. Returns the encrypted backup, and unlocked:false when the wallet was locked meanwhile.
 *  Codex review (PR #114): the saved vault is read again and written inside the vault lock that the Studio and IDE
 *  pages' saveVaultIfNone hold too, so a wallet a page saved while this encrypted is never overwritten; and a Lock
 *  (or any lock) during the encryption is kept: the new wallet is saved but stays locked.
 *  Codex review (PR #114, head 7f91bb9e): `replaceAddress` is the saved address the refusal reported, which the
 *  settings page's confirmation showed (and the user backed up); replace:true then covers only that wallet, compared
 *  with the saved vault again inside the vault lock, so a wallet another Settings tab saved meanwhile is refused
 *  ('exists', a new confirmation). Without it, replace:true covers the wallet saved when this started, as before. */
async function saveAccount(secret, password, replace, replaceAddress) {
  if (typeof password !== 'string' || password.length < acct.PASSWORD_MIN) throw fail(`Use a password of at least ${acct.PASSWORD_MIN} characters`, 'password');
  if (accountBusy) throw fail('Another wallet operation is in progress', 'busy');
  accountBusy = true;
  try {
    const e0 = accountEpoch, address = addressOf(secret), before = await storedVault();
    // The confirmed wallet: a lowercase address, or null for a saved vault without a readable one; anything else never matches.
    const confirmed = replaceAddress === undefined ? undefined : replaceAddress === null ? null : typeof replaceAddress === 'string' ? replaceAddress.toLowerCase() : false;
    const covered = (v) => (confirmed === undefined ? !!before && vaultAddress(v) === vaultAddress(before) : vaultAddress(v) === confirmed);
    const allowed = (v) => !v || vaultAddress(v) === address || (replace === true && covered(v));
    const refusal = (v) => fail('Another PoCol wallet is saved in this browser. Back it up first, then confirm that you replace it.', 'exists', { saved: vaultAddress(v) });
    if (!allowed(before)) throw refusal(before);
    const box = await exportKeyfile(secret, password);
    return await withVaultLock(async () => {
      const now = await storedVault();
      if (!allowed(now)) throw refusal(now);
      const lockedMeanwhile = e0 !== accountEpoch;
      lockAccount('wallet replaced'); const epoch = accountEpoch;
      await store.set({ [VAULT_KEY]: box });
      if (epoch !== accountEpoch) throw fail('The wallet changed while it was saved; unlock it again', 'changed');
      if (lockedMeanwhile) return { address, backup: box, unlocked: false };
      account = { secret: secret.slice(), address, until: Date.now() + LOCK_MS };
      return { address, backup: box, unlocked: true };
    });
  } finally { accountBusy = false; secret.fill(0); }
}
/** The profile a wallet operation uses (default: the built-in one): valid and not conflicting. */
async function walletProfile(name) {
  const p = (await profiles()).find((x) => x.name === (name || NETWORK.name));
  if (!p) throw fail('Unknown network profile', 'profile');
  if (p.invalid) throw fail('This network profile is no longer valid: ' + p.invalid, 'profile');
  if (p.conflict) throw fail('This network profile conflicts with another one (same chain ID, another genesis or schedule); it is read-only', 'profile');
  return p;
}
const isPlaceholder = (p) => PLACEHOLDER_BUILD && p.name === NETWORK.name;
const PLACEHOLDER_NOTE = 'This build pins a placeholder network, not PoCol Mainnet: the wallet makes no network request and cannot send until the extension release pins PoCol Mainnet.';
const explorerFor = (p, address) => (p.name === NETWORK.name && !isPlaceholder(p) && p.endpoint === acct.EXPLORER_ORIGIN + '/studio/rpc' ? acct.explorerAddressUrl(address) : null);
async function requireDisclosure() {
  const d = (await store.get(DISCLOSURE_KEY))[DISCLOSURE_KEY];
  if (!d || d.version !== DISCLOSURE_VERSION) throw fail('Read and accept what the wallet sends to the network first (open the PoCol wallet from the toolbar)', 'disclosure');
}
const quantity = (v, what) => { if (typeof v !== 'string' || !/^0x[0-9a-fA-F]{1,64}$/.test(v)) throw fail('The network returned a malformed ' + what, 'endpoint'); return BigInt(v); };
const frozenProfile = (p) => Object.freeze(Object.fromEntries(PROFILE_FIELDS.map((k) => [k, p[k]])));
async function netStatus(name) {
  const p = await walletProfile(name);
  const info = { name: p.name, chainId: p.chainId, genesisHash: p.genesisHash, endpoint: p.endpoint, trustLevel: p.trustLevel, builtIn: p.name === NETWORK.name, placeholder: isPlaceholder(p) };
  if (info.placeholder) return { ...info, reachable: null, note: PLACEHOLDER_NOTE };
  await requireDisclosure();
  const rpc = rpcFor(p, pools.trust, null, http.inflight, { attempts: 2, deadlineMs: 15000 });
  try {
    const cidHex = await rpc('eth_chainId', []), cid = quantity(cidHex, 'chain ID'), st = await rpc('pocol_status', []);
    const chainOk = cid === BigInt(p.chainId), genesisOk = typeof st?.genesisHash === 'string' && st.genesisHash.toLowerCase() === p.genesisHash;
    const diff = identityDifference(p, cidHex, st?.genesisHash);
    let head = null;
    if (chainOk && genesisOk) {
      const b = await rpc('eth_getBlockByNumber', ['latest', false]); const ts = Number(quantity(b?.timestamp, 'block time'));
      head = { height: Number(quantity(b?.number, 'block number')), timestamp: ts, ageSec: Math.max(0, Math.round(Date.now() / 1000 - ts)) };
    }
    return { ...info, reachable: true, chainOk, genesisOk, endpointChainId: cid.toString(), endpointGenesisHash: typeof st?.genesisHash === 'string' ? st.genesisHash.toLowerCase().slice(0, 66) : null,
      difference: diff ? describeDifference(diff) : null, head };
  } catch (e) { return { ...info, reachable: false, error: String(e?.rpc?.message || e?.message || e).slice(0, 200) }; }
}
async function accountBalance(name) {
  const v = await storedVault(), address = vaultAddress(v); if (!address) throw fail('No PoCol wallet is saved in this browser yet', 'none');
  const p = await walletProfile(name);
  if (isPlaceholder(p)) return { address, placeholder: true, note: PLACEHOLDER_NOTE };
  await requireDisclosure();
  const rpc = rpcFor(p, pools.trust); await checkNetwork(p, rpc);
  const wei = quantity(await rpc('eth_getBalance', [address, 'latest']), 'balance');
  return { address, network: p.name, chainId: p.chainId, wei: wei.toString(), pcol: acct.formatPcol(wei), explorer: explorerFor(p, address) };
}
let historyChain = Promise.resolve();
const historyLocked = (f) => { const r = historyChain.then(f); historyChain = r.catch(() => {}); return r; };
const readHistory = async () => { const h = (await store.get(HISTORY_KEY))[HISTORY_KEY]; return Array.isArray(h) ? h : []; };
const updateHistory = (hash, patch) => historyLocked(async () => {
  const h = await readHistory(), i = h.findIndex((x) => x.hash === hash);
  if (i >= 0) h[i] = { ...h[i], ...patch, updatedAt: Date.now() }; else h.unshift({ hash, ...patch, createdAt: Date.now(), updatedAt: Date.now() });
  const e = h[i >= 0 ? i : 0]; for (const k of Object.keys(e)) if (e[k] === undefined) delete e[k]; // a patch value undefined removes the field
  await store.set({ [HISTORY_KEY]: h.slice(0, HISTORY_MAX) });
});
async function prepareTransfer(msg, sender) {
  const tabId = sender.tab?.id; if (!Number.isInteger(tabId)) throw fail('Open the send page from the PoCol wallet', 4100);
  const p = await walletProfile(msg.name);
  if (isPlaceholder(p)) throw fail(PLACEHOLDER_NOTE, 'placeholder');
  await requireDisclosure();
  if (!accountUnlocked()) throw fail('Unlock the PoCol wallet first', 4100);
  const to = acct.parseRecipient(msg.to), value = acct.parsePcol(msg.amount);
  const address = account.address, ep = { account: accountEpoch, profile: eligibilityEpoch };
  const stale = () => { if (ep.account !== accountEpoch || ep.profile !== eligibilityEpoch || !accountUnlocked() || account.address !== address) throw fail('The wallet or the network profiles changed; review the transfer again', 'changed'); };
  if (p.name === NETWORK.name) { const all = await store.get(null); if (Object.entries(all).some(([k, j]) => (k.startsWith(JOB_PREFIX) || k.startsWith(IDE_JOB_PREFIX)) && blocksAccount(j, address, NETWORK))) throw fail('Resume or cancel the unfinished Studio or IDE operation of this wallet first (Settings, Publication history)', 'busy'); }
  const rpc = rpcFor(p, pools.trust);
  await checkNetwork(p, rpc); stale();
  const nonce = quantity(await rpc('eth_getTransactionCount', [address, 'pending']), 'nonce');
  const balance = quantity(await rpc('eth_getBalance', [address, 'latest']), 'balance');
  const fees = await readNetworkFees(rpc), { maxPriority, maxFee } = acct.transferFees(fees.baseNext, fees.tip, ABSOLUTE_CEILING);
  const gas = quantity(await rpc('eth_estimateGas', [{ from: address, to, value: '0x' + value.toString(16) }]), 'gas estimate');
  stale();
  if (gas < 21000n || gas > TRANSFER_GAS_MAX) throw fail(`The network estimates ${gas} gas for this transfer; this wallet sends plain transfers of at most ${TRANSFER_GAS_MAX} gas`, 'gas');
  const maxCost = gas * maxFee, total = value + maxCost;
  if (balance < total) throw fail(`Insufficient balance: ${acct.formatPcol(balance)} available, up to ${acct.formatPcol(total)} needed (amount plus maximum fee)`, 'funds');
  const tx = { chainId: p.chainId, nonce, maxPriority, maxFee, gas, to, value, data: '0x' }, ut = unsignedTx(tx);
  for (const [k, r] of reviews) if (r.tabId === tabId) reviews.delete(k);
  const id = crypto.randomUUID(), expiresAt = Date.now() + SEND_REVIEW_MS;
  reviews.set(id, { id, tabId, profile: frozenProfile(p), address, tx, ut, ep, expiresAt });
  return { id, expiresAt, network: p.name, chainId: p.chainId, genesisHash: p.genesisHash, endpoint: p.endpoint, trustLevel: p.trustLevel, builtIn: p.name === NETWORK.name,
    from: address, to, value: value.toString(), valuePcol: acct.formatPcol(value), nonce: nonce.toString(), gas: gas.toString(),
    maxFeePerGas: maxFee.toString(), maxPriorityFeePerGas: maxPriority.toString(), baseFeePerGas: fees.baseNext,
    maxFee: maxCost.toString(), maxFeePcol: acct.formatPcol(maxCost), expectedFeePcol: acct.formatPcol(gas * (BigInt(fees.baseNext) + maxPriority)),
    total: total.toString(), totalPcol: acct.formatPcol(total), balancePcol: acct.formatPcol(balance), signingHash: toHex(ut.signingHash), explorer: explorerFor(p, address) };
}
async function confirmTransfer(msg, sender) {
  const r = reviews.get(msg.id);
  if (!r || r.tabId !== sender.tab?.id) throw fail('This review is no longer valid; review the transfer again', 'expired');
  reviews.delete(msg.id); // one use
  if (r.cancelled) throw fail('The wallet was locked or changed; review the transfer again', 'changed');
  if (Date.now() > r.expiresAt) throw fail('The review expired after two minutes; review the transfer again', 'expired');
  if (!globalThis.navigator?.locks) throw fail('This browser cannot coordinate wallet transactions safely', 4200);
  // The publisher and contract pages hold this lock while they sign: a transfer never interleaves with their nonces.
  return navigator.locks.request(`pocol:publish:${r.tx.chainId}:${r.address}`, { mode: 'exclusive', ifAvailable: true }, async (lease) => {
    if (!lease) throw fail('Another tab is publishing from this wallet; finish or pause it first', 'busy');
    // Codex review (PR #114): a Studio or IDE job that signed after the review (and paused before its transaction reached
    // the node) holds the reviewed nonce. Checked again here, under the lock every job signs under, as they do.
    if (r.profile.name === NETWORK.name) { const all = await store.get(null); if (Object.entries(all).some(([k, j]) => (k.startsWith(JOB_PREFIX) || k.startsWith(IDE_JOB_PREFIX)) && blocksAccount(j, r.address, NETWORK))) throw fail('Resume or cancel the unfinished Studio or IDE operation of this wallet first (Settings, Publication history)', 'busy'); }
    const signed = await profileLocked(async () => {
      const cur = await walletProfile(r.profile.name);
      if (!sameProfile(cur, r.profile) || r.ep.profile !== eligibilityEpoch) throw fail('The network profile changed; review the transfer again', 4901);
      if (!accountUnlocked() || account.address !== r.address || r.ep.account !== accountEpoch) throw fail('The wallet was locked or changed; review the transfer again', 4100);
      const rpc = rpcFor(cur, pools.trust); await checkNetwork(cur, rpc);
      const nonce = quantity(await rpc('eth_getTransactionCount', [r.address, 'pending']), 'nonce');
      if (nonce !== r.tx.nonce) throw fail('Another transaction from this wallet was sent meanwhile; review the transfer again', 'nonce');
      if (!accountUnlocked() || account.address !== r.address || r.ep.account !== accountEpoch || r.ep.profile !== eligibilityEpoch) throw fail('The wallet was locked or changed; review the transfer again', 4100);
      const out = signUnsigned(account.secret, r.ut); touchAccount(); return out;
    });
    const entry = { network: r.profile.name, chainId: r.tx.chainId, genesisHash: r.profile.genesisHash, from: r.address, to: r.tx.to, value: r.tx.value.toString(), nonce: r.tx.nonce.toString(),
      gas: r.tx.gas.toString(), maxFeePerGas: r.tx.maxFee.toString(), status: 'sending' };
    // Codex review (PR #114): a Lock, a wallet change or a profile change until the request starts stops it, as the
    // viewer wallet's submit() does: the queued request is aborted (lockAccount, profilesChanging), and the account
    // epoch and the profiles are checked again after the history write and, synchronously after the transport
    // reservation, immediately before the request starts. A transfer stopped so was not sent, and its entry says so.
    const ineligible = () => (r.ep.profile !== eligibilityEpoch ? fail('The network profiles changed before the transfer was sent; it was not sent. Review it again', 4901)
      : !accountUnlocked() || account.address !== r.address || r.ep.account !== accountEpoch ? fail('The wallet was locked or changed before the transfer was sent; it was not sent. Review it again', 4100) : null);
    const notSent = async (e) => { await updateHistory(signed.hash, { status: 'cancelled', note: 'Not sent: ' + e.message }); throw e; };
    const queued = new AbortController(); transfersQueued.add(queued);
    try {
      await updateHistory(signed.hash, entry); // recorded before it is sent, so a lost answer is still tracked by its hash
      let stopped = ineligible();
      if (stopped || queued.signal.aborted) return await notSent(stopped || fail('The transfer was stopped before it was sent; it was not sent. Review it again', 'changed'));
      try {
        const result = await rpcFor(r.profile, pools.trust, null, http.inflight, false)('eth_sendRawTransaction', [signed.raw], RPC_TIMEOUT_MS,
          { signal: queued.signal, beforeSend: () => !!(stopped = ineligible()) });
        const ok = typeof result === 'string' && result.toLowerCase() === signed.hash;
        await updateHistory(signed.hash, ok ? { status: 'pending' } : { status: 'unknown', note: 'The node answered with another hash' });
        return { hash: signed.hash, status: ok ? 'pending' : 'unknown' };
      } catch (e) {
        // lib/transport.js answers 'cancelled' only before the request started (aborted or refused by beforeSend).
        if (e?.transport === 'cancelled') return await notSent(stopped || ineligible() || fail('The transfer was stopped before it was sent; it was not sent. Review it again', 'changed'));
        // Only an answer that proves a refusal is final (lib/account.js sendRefused). Any other error, such as the
        // gateway's "check transaction hash before resubmitting" after a broadcast it could not complete, may have
        // reached the node: the transfer stays "unknown" and refreshHistory follows its receipt, as the publisher does.
        const said = e?.rpc ? String(e.rpc.message || '').slice(0, 200) : null;
        if (e?.rpc && acct.sendRefused(e.rpc)) { await updateHistory(signed.hash, { status: 'rejected', note: said }); throw fail('The node did not accept the transaction: ' + said, 'rejected', { hash: signed.hash }); }
        const note = said === null ? 'No answer from the node; the transaction may have been sent'
          : `The node answered "${said}", which does not show that it refused the transaction; it may have been sent`;
        await updateHistory(signed.hash, { status: 'unknown', note });
        return { hash: signed.hash, status: 'unknown', note };
      }
    } finally { transfersQueued.delete(queued); }
  });
}
/** Receipts of this extension's open transfers (at most REFRESH_MAX): included (status and block, on the canonical
 *  chain), failed, or dropped when the account's nonce moved past it without a receipt. A receipt is not finality: an
 *  included or failed transfer is read again on every refresh until its block is SETTLE_DEPTH blocks below the head
 *  ("settled"), and goes back to pending when its receipt or its block leaves the canonical chain (a reorganization),
 *  as the publisher re-reads a saved receipt against the canonical block (lib/publisher.js). A dropped transfer is not
 *  final either (Codex review, PR #114): it is read again until the account's nonce at SETTLE_DEPTH blocks below the
 *  head is past its nonce, becomes included or failed when its receipt appears, and goes back to pending when the
 *  nonce is free again (a reorganization removed the other transaction). A node that cannot answer the nonce at that
 *  depth (its retained state window) bounds the watch to DROPPED_WATCH blocks after the drop was seen. */
const OPEN_STATES = ['sending', 'pending', 'unknown'], RECHECK_STATES = ['included', 'failed'];
// Deeper than pocold's reorganization alarm (node.rs K_ALARM = 60 blocks) and its `safe` depth (K_SAFE = 6).
export const SETTLE_DEPTH = 64n, DROPPED_WATCH = 4n * SETTLE_DEPTH;
async function refreshHistory() {
  await requireDisclosure();
  const list = await profiles(); const checked = new Map(), heads = new Map();
  const profileOf = (t) => list.find((x) => x.name === t.network && !x.invalid && x.chainId === t.chainId && x.genesisHash === t.genesisHash);
  // Entries whose profile is gone or is the placeholder are skipped before the bound, so they never crowd out the rest.
  // Codex review (PR #114, head 1e00c82d): the bound rotates through all eligible entries. Each refresh takes the
  // REFRESH_MAX least recently checked ones (never checked first, then newest first) and stamps them with the next
  // `checkSeq` numbers, under the history lock, before any request: entries that stay pending or unknown, or whose
  // profile is unreachable, move to the back, so older transfers are checked again in turn.
  const open = await historyLocked(async () => {
    const h = await readHistory();
    const picked = h.map((x, i) => [x, i]).filter(([x]) => (OPEN_STATES.includes(x.status) || ((RECHECK_STATES.includes(x.status) || x.status === 'dropped') && !x.settled)) && profileOf(x) && !isPlaceholder(profileOf(x)))
      .sort(([a, i], [b, j]) => (Number(a.checkSeq) || 0) - (Number(b.checkSeq) || 0) || i - j).slice(0, REFRESH_MAX);
    if (!picked.length) return [];
    let seq = h.reduce((m, x) => Math.max(m, Number(x.checkSeq) || 0), 0);
    for (const [, i] of picked) h[i] = { ...h[i], checkSeq: ++seq };
    await store.set({ [HISTORY_KEY]: h });
    return picked.map(([x]) => x);
  });
  for (const t of open) {
    const p = profileOf(t);
    const rpc = rpcFor(p, pools.trust, null, http.inflight, { attempts: 2, deadlineMs: 15000 });
    const headOf = () => { if (!heads.has(p.name)) heads.set(p.name, rpc('eth_getBlockByNumber', ['latest', false]).then((h) => quantity(h?.number, 'block number'))); return heads.get(p.name); };
    try {
      if (!checked.has(p.name)) checked.set(p.name, checkNetwork(p, rpc).then(() => true, () => false));
      if (!(await checked.get(p.name))) continue;
      const rc = await rpc('eth_getTransactionReceipt', [t.hash]);
      const mine = !!rc && String(rc.transactionHash).toLowerCase() === t.hash;
      if (mine) {
        const b = await rpc('eth_getBlockByNumber', [rc.blockNumber, false]);
        if (b && String(b.hash).toLowerCase() === String(rc.blockHash).toLowerCase()) {
          const at = quantity(rc.blockNumber, 'block number');
          const head = await headOf();
          await updateHistory(t.hash, { status: rc.status === '0x1' ? 'included' : 'failed', blockNumber: Number(at), blockHash: String(rc.blockHash).toLowerCase(), gasUsed: String(quantity(rc.gasUsed, 'gas used')),
            settled: head >= at && head - at >= SETTLE_DEPTH, droppedAt: undefined, note: undefined });
          continue;
        }
      }
      // No canonical receipt: an entry recorded as included or failed was reorganized out of the chain.
      if (RECHECK_STATES.includes(t.status))
        await updateHistory(t.hash, { status: 'pending', blockNumber: undefined, blockHash: undefined, gasUsed: undefined, settled: undefined, note: 'Its block is no longer on the canonical chain (a reorganization); waiting for it to be included again' });
      if (mine) continue; // a receipt in a block that is not canonical (yet): read it again next time
      const used = quantity(await rpc('eth_getTransactionCount', [t.from, 'latest']), 'nonce') > BigInt(t.nonce);
      if (t.status === 'dropped') {
        if (!used) { await updateHistory(t.hash, { status: 'pending', droppedAt: undefined, note: 'The transaction that used its nonce is no longer on the canonical chain (a reorganization); waiting for this one' }); continue; }
        const head = await headOf(), since = t.droppedAt === undefined ? null : BigInt(t.droppedAt);
        if (since === null) { await updateHistory(t.hash, { droppedAt: Number(head) }); continue; }
        if (head >= since + DROPPED_WATCH || (head >= SETTLE_DEPTH && quantity(await rpc('eth_getTransactionCount', [t.from, '0x' + (head - SETTLE_DEPTH).toString(16)]), 'nonce') > BigInt(t.nonce)))
          await updateHistory(t.hash, { settled: true });
        continue;
      }
      if (used && !(await rpc('eth_getTransactionReceipt', [t.hash]))) {
        // Asked again after the nonce read, so a transaction included between the two reads is not called dropped.
        await updateHistory(t.hash, { status: 'dropped', droppedAt: Number(await headOf()), note: 'Another transaction used this nonce; this one was not included' });
      }
    } catch { /* stays open; the next refresh tries again */ }
  }
  return listHistory();
}
async function listHistory() {
  const list = await profiles();
  return (await readHistory()).map((t) => { const p = list.find((x) => x.name === t.network && x.chainId === t.chainId); return { ...t, explorer: p ? explorerFor(p, t.from) : null }; });
}
async function accountRequest(msg, sender) {
  switch (msg.type) {
    case 'account.status': {
      const v = await storedVault();
      const disclosure = (await store.get(DISCLOSURE_KEY))[DISCLOSURE_KEY];
      return { exists: !!v, address: vaultAddress(v), unlocked: accountUnlocked(), lockAt: account ? account.until : null, disclosureAccepted: disclosure?.version === DISCLOSURE_VERSION, placeholderBuild: PLACEHOLDER_BUILD, network: NETWORK.name };
    }
    case 'account.acceptDisclosure': fromPages(sender, PAGES.read); await store.set({ [DISCLOSURE_KEY]: { version: DISCLOSURE_VERSION, at: Date.now() } }); return {};
    case 'account.create': {
      fromPages(sender, PAGES.manage);
      let key; do { key = crypto.getRandomValues(new Uint8Array(32)); try { addressOf(key); break; } catch { key.fill(0); } } while (true);
      return saveAccount(key, msg.password, msg.replace, msg.replaceAddress);
    }
    case 'account.import': {
      fromPages(sender, PAGES.manage);
      let secret;
      if (msg.kind === 'privateKey') secret = acct.parsePrivateKey(msg.privateKey);
      else if (msg.kind === 'phrase') secret = acct.phraseToSecret(msg.phrase);
      else if (msg.kind === 'file') {
        if (typeof msg.fileBase64 !== 'string' || msg.fileBase64.length > 22000) throw fail('Choose a PoCol wallet file', 'file');
        let bytes; try { bytes = Uint8Array.from(atob(msg.fileBase64), (c) => c.charCodeAt(0)); } catch { throw fail('Choose a PoCol wallet file', 'file'); }
        passwordGate();
        try { secret = await importKeyfile(bytes, msg.filePassword ?? msg.password); } catch (e) { pwFails.push(Date.now()); throw fail(e.message, 'file'); } finally { bytes.fill(0); }
      } else throw fail('Choose what to import', 'kind');
      return saveAccount(secret, msg.password, msg.replace, msg.replaceAddress);
    }
    case 'account.unlock': {
      fromPages(sender, PAGES.unlock);
      const e0 = accountEpoch, { secret } = await openVault(msg.password);
      if (e0 !== accountEpoch) { secret.fill(0); throw fail('The wallet changed during unlocking; try again', 'changed'); }
      lockAccount('unlocked again'); account = { secret, address: addressOf(secret), until: Date.now() + LOCK_MS };
      return { address: account.address, lockAt: account.until };
    }
    case 'account.lock': {
      // The user's Lock (toolbar or settings) locks every page that holds a decrypted key too: the Studio publisher and
      // IDE contract pages watch this key (lib/vault-guard.js watchLockSignal) and lock at once.
      lockAccount('locked');
      await store.set({ [LOCK_SIGNAL_KEY]: { at: Date.now(), id: crypto.randomUUID() } });
      return {};
    }
    case 'account.export': {
      // EXT-04: a backup at any time, after the password is entered again; the result is the stored encrypted vault.
      fromPages(sender, PAGES.manage);
      const { vault, secret } = await openVault(msg.password);
      const address = addressOf(secret); secret.fill(0);
      if (vaultAddress(vault) !== address) throw fail('The saved wallet does not match its address; it is damaged', 'damaged');
      return { address, backup: vault };
    }
    case 'net.status': fromPages(sender, PAGES.read); return netStatus(msg.name);
    case 'account.balance': fromPages(sender, PAGES.read); return accountBalance(msg.name);
    case 'send.prepare': fromPages(sender, PAGES.send); return prepareTransfer(msg, sender);
    case 'send.confirm': fromPages(sender, PAGES.send); return confirmTransfer(msg, sender);
    case 'send.cancel': fromPages(sender, PAGES.send); if (reviews.get(msg.id)?.tabId === sender.tab?.id) reviews.delete(msg.id); return {};
    case 'txs.list': fromPages(sender, PAGES.read); return { transactions: await listHistory() };
    case 'txs.refresh': fromPages(sender, PAGES.read); return { transactions: await refreshHistory() };
    case 'connections.list': fromPages(sender, PAGES.manage);
      return { connections: [...sessions.entries()].filter(([, s]) => !s.closed && s.approved).map(([id, s]) => ({ id, site: s.site, network: s.profile.name, address: s.approved, tabId: s.tabId })) };
    case 'connections.revoke': {
      // EXT-03: the user ends a site's account grant; its pending requests are cancelled and it must ask again.
      fromPages(sender, PAGES.manage);
      const s = sessions.get(msg.id); if (!s || s.closed) throw fail('This site is no longer connected', 'gone');
      s.approved = null; cancelApprovals((x) => x === s, 4100, 'disconnected by the user');
      return {};
    }
    default: return null;
  }
}

// Sites W4: on install or update, jobs of another network (a retired chain, or an identity change) move to the read-only
// archive, so they never block the wallet and need no receipts from this network.
// The schedule key of releases up to 0.4.1 (an operator-supplied schedule for the built-in profile) is never read; an
// update into the same storage (a development build without the release key) removes it.
chrome.runtime.onInstalled?.addListener(() => { archiveForeignJobs(store, NETWORK).catch(() => {}); store.remove('pocol:publicForkSchedule').catch(() => {}); });
chrome.runtime.onMessage.addListener((msg, sender, reply) => {
  if (sender.id !== chrome.runtime.id) return false;
  // Content scripts have our extension ID too. They must never reach wallet/profile APIs.
  if (sender.url && !sender.url.startsWith(chrome.runtime.getURL(''))) {
    const handler=typeof msg?.type==='string'&&msg.type.startsWith('ide.')?ideRequest:studioRequest;
    handler(msg || {}, sender).then((result) => reply({ ok: true, result }), (e) => reply({ ok: false, error: String(e.message || e).slice(0, 256) }));
    return true;
  }
  if (typeof msg?.type === 'string' && /^(account|net|send|txs|connections)\./.test(msg.type)) {
    accountRequest(msg, sender).then((r) => reply(r === null ? { ok: false, error: { code: -32601, message: 'unknown message' } } : { ok: true, ...r }),
      (e) => reply({ ok: false, error: { code: e.code ?? -32603, message: String(e.message || e).slice(0, 300), ...(e.saved ? { saved: e.saved } : {}), ...(e.hash ? { hash: e.hash } : {}) } }));
    return true;
  }
  (async () => {
    switch (msg.type) {
      case 'profiles.list': return { ok: true, profiles: await profiles() };
      case 'profiles.add': return profileLocked(async () => {
        if (msg.profile?.name === NETWORK.name) throw new Error('pocol-public is a built-in profile; choose a different name for custom networks');
        const p = validateProfile(msg.profile); profilesChanging(); const list = (await storedProfiles()).filter((x) => x.name !== p.name);
        list.push(p); try { await store.set({ profiles: list }); } finally { await profilesChanged(); }
        return { ok: true, profile: (await profiles()).find((x) => x.name === p.name) }; });
      case 'profiles.remove': return profileLocked(async () => {
        if (msg.name === NETWORK.name) throw new Error('The built-in PoCol public profile cannot be removed');
        profilesChanging();
        try { await store.set({ profiles: (await storedProfiles()).filter((x) => x.name !== msg.name) }); } finally { await profilesChanged(); }
        return { ok: true }; });
      case 'open': return await openSite({ ...msg, tabId: sender.tab ? sender.tab.id : undefined });
      case 'session.storage': { const s = sessions.get(msg.sessionId); if (!s || !sender.tab || sender.tab.id !== s.tabId) return { ok: false }; await booted; const snap = await records.load(s.netKey, s.site); return { ok: true, storage: snap.dict, storageError: snap.error || null }; }
      case 'wallet.create': {
        // F11: enforce the password policy and an explicit replace-confirmation here in the worker, not
        // only in the options page. Reject a missing/short password; and refuse to overwrite an existing
        // wallet unless the caller passes replace:true (a bare wallet.create cannot silently destroy the
        // current key, which the viewer wallet can never recover).
        if (typeof msg.password !== 'string' || msg.password.length < WALLET_PW_MIN) throw new Error(`password must be at least ${WALLET_PW_MIN} characters`);
        // Reserve synchronously: two settings tabs must not both observe an absent wallet
        // and silently replace each other's seed after the asynchronous encryption.
        if (walletCreating) throw new Error('wallet creation already in progress');
        walletCreating = true;
        try {
          const before = walletEpoch;
          if ((await store.get('wallet')).wallet && msg.replace !== true) throw new Error('a wallet already exists; pass replace:true to overwrite it (this permanently discards the current key)');
          if (walletEpoch !== before) throw new Error('wallet changed during creation');
          const seed = msg.seedHex ? fromHex(msg.seedHex) : crypto.getRandomValues(new Uint8Array(32)); if (seed.length !== 32) throw new Error('seed');
          lockWallet('wallet replaced'); const epoch = walletEpoch;
          const box = await encryptSeed(seed, msg.password);
          if (walletEpoch !== epoch) throw new Error('wallet changed during creation');
          await store.set({ wallet: box });
          // A lock cannot undo a storage write already in flight. Keep the persisted
          // wallet locked, and invalidate an unlock of the previous stored seed too.
          if (walletEpoch !== epoch) { lockWallet('wallet changed during creation'); throw new Error('wallet changed during creation; saved wallet remains locked'); }
          walletChanged('wallet replaced'); unlocked = { seed, until: Date.now() + LOCK_MS }; return { ok: true };
        } finally { walletCreating = false; }
      }
      case 'wallet.unlock': { const e0 = walletEpoch; const w = (await store.get('wallet')).wallet; if (!w) return { ok: false, error: 'no wallet' }; const seed = await decryptSeed(w, msg.password);
        // A wallet.create (or lock) during the decryption wins: the old seed is never reinstated.
        if (walletEpoch !== e0) return { ok: false, error: 'wallet changed during unlock' };
        walletChanged('wallet unlocked again'); unlocked = { seed, until: Date.now() + LOCK_MS }; return { ok: true }; }
      case 'wallet.lock': lockWallet('wallet locked'); return { ok: true };
      case 'wallet.status': return { ok: true, exists: !!(await store.get('wallet')).wallet, unlocked: isUnlocked() };
      case 'wallet.address': { const p = (await profiles()).find((x) => x.name === msg.name); if (!p || p.invalid || !isUnlocked()) return { ok: false }; if (!seedWalletAllowed(p)) return { ok: false, error: SEED_LOCAL_ONLY }; const seed = unlocked.seed; return { ok: true, address: addressOf(await networkKey(seed, walletSalt(p))) }; }
      default: return { ok: false, error: 'unknown message' };
    }
  })().then(reply, (e) => reply({ ok: false, error: { code: e.code || -32603, message: String(e.message || e) } }));
  return true;
});

// The toolbar button opens popup.html (manifest action.default_popup), so it has no click handler here.

// omnibox: 'pocol <profile>/<0xaddr>[/path]'
chrome.omnibox.onInputEntered.addListener((text) => {
  chrome.tabs.update({ url: chrome.runtime.getURL('viewer.html') + '#' + encodeURI(text.trim()) });
});
