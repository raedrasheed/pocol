// Browser publisher for PoCol Website v1. Protocol parity: pocold/src/site.rs @50c2f27.
// Runs in the extension's trusted page. No private key is accepted or retained here.
// Sites W4: fees from the node (fees.js), one approved budget and ceiling, just-in-time signing, checkpoint schema 2,
// the safe sweep before createVersion, economy pacing and a read-back of the published site.
import * as rlp from './rlp.js';
import { keccak, recover } from './crypto.js';
import { concat, eq, toHex, fromHex } from './hex.js';
import { unsignedTx } from './wallet.js';
import { validPath, chunkAddress, chunkPayload, decodeManifest, decodeCurrent, fetchSite, CHUNK_MAX, FILE_MAX, FILES_MAX, SITE_MAX } from './site.js';
import { WEBSITE_INITCODE, WEBSITE_RUNTIME } from './website-bytecode.js';
import { feeSchedule, capFor, nextBaseFee, chunkGasUsed, ABSOLUTE_CEILING, TIP_CAP, BASE_FEE_FLOOR, MARGIN_PER_MILLE } from './fees.js';
import { denyReason, DENY_REASONS } from './publish-input.js';
export { readNetworkFees } from './fees.js';

export const FACTORY = '0x0000000000000000000000000000000000c0c005';
// The fixed caps every publication used before Sites W4; kept only to recognise (and migrate) schema-1 checkpoints.
export const LEGACY_MAX_FEE = '200000000000';
export const LEGACY_PRIORITY_FEE = '1000000000';
const enc = new TextEncoder();
// Manifest MIME ids (site.js MIME, pocold site.rs MIME). Id 11 stays unassigned: the M1 manifest vectors use it as the
// unknown MIME. Sites W5 (owner answer Q3): 12 favicon.ico, 13 site.webmanifest.
const EXT_MIME = {html:1,htm:1,css:2,js:3,mjs:3,json:4,png:5,jpg:6,jpeg:6,webp:7,svg:8,woff2:9,txt:10,ico:12,webmanifest:13};
export const mimeIdFor = (path) => EXT_MIME[String(path).split('.').pop().toLowerCase()] ?? null;
const addressRx = /^0x[0-9a-fA-F]{40}$/;
const hexRx = /^0x(?:[0-9a-fA-F]{2})*$/;
const assert = (yes, message) => { if (!yes) throw new Error(message); };
const address = (value) => { assert(typeof value === 'string' && addressRx.test(value), 'Invalid address'); return value.toLowerCase(); };
const uint = (value, name = 'integer') => { let n; try { n = BigInt(value); } catch { throw new Error('Invalid ' + name); } assert(n >= 0n && n < (1n << 256n), 'Invalid ' + name); return n; };
const qty = (value) => '0x' + uint(value).toString(16);
const jsonClone = (value) => JSON.parse(JSON.stringify(value));
const selBytes = (sig) => keccak(enc.encode(sig)).subarray(0, 4);
export const selector = (sig) => toHex(selBytes(sig));
export const word = (value) => fromHex('0x' + uint(value).toString(16).padStart(64, '0'));
export const abiUint = (sig, value) => toHex(concat(selBytes(sig), word(value)));
export const abiAddress = (sig, value) => toHex(concat(selBytes(sig), new Uint8Array(12), fromHex(address(value))));
export function abiBytes(sig, data) {
  return toHex(concat(selBytes(sig), word(32), word(data.length), data, new Uint8Array((32 - data.length % 32) % 32)));
}
export function abiCreateVersion(hash, size, chunks) {
  return toHex(concat(selBytes('createVersion(bytes32,uint32,address[])'), fromHex(hash), word(size), word(96), word(chunks.length), ...chunks.map((c) => concat(new Uint8Array(12), fromHex(address(c.address))))));
}
function chunksOf(data, kind) {
  const out = [];
  for (let i = 0; i < data.length; i += CHUNK_MAX) { const bytes = data.slice(i, i + CHUNK_MAX); out.push({address:toHex(chunkAddress(bytes)), data:bytes, kind}); }
  return out;
}
export function prepareSite(input, {entry = '/index.html'} = {}) {
  assert(Array.isArray(input) && input.length > 0 && input.length <= FILES_MAX, 'A site needs 1–256 files');
  const seen = new Set(); let totalBytes = 0;
  const files = input.map((f) => {
    assert(f && validPath(f.path), 'Invalid site file path');
    assert(!seen.has(f.path), 'Duplicate path: ' + f.path); seen.add(f.path);
    const denied = denyReason(f.path); assert(!denied, 'Refused file ' + f.path + ': ' + (DENY_REASONS[denied] || denied));
    assert(f.data instanceof Uint8Array, 'File data must be Uint8Array');
    assert(f.data.length <= FILE_MAX, 'File exceeds 1 MiB: ' + f.path);
    totalBytes += f.data.length; assert(totalBytes <= SITE_MAX, 'Site exceeds 4 MiB');
    const mime = mimeIdFor(f.path); assert(mime, 'Unsupported file extension: ' + f.path);
    const data = f.data.slice();
    return {path:f.path, data, mime, size:data.length, hash:toHex(keccak(data)), chunks:chunksOf(data, 'file')};
  }).sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  const entryIndex = files.findIndex((f) => f.path === entry);
  assert(entryIndex >= 0 && files[entryIndex].mime === 1, 'Entry must be an included HTML file');
  const u = (v) => rlp.encUint(BigInt(v));
  const manifest = rlp.encode([u(1), u(entryIndex), files.map((f) => [enc.encode(f.path), u(f.mime), u(f.size), fromHex(f.hash), f.chunks.map((c) => [fromHex(c.address),u(c.data.length)])])]);
  decodeManifest(manifest); // Independent existing viewer validation, including T16 limits.
  const manifestChunks = chunksOf(manifest, 'manifest');
  assert(manifestChunks.length > 0 && manifestChunks.length <= 3, 'Manifest needs 1–3 chunks');
  const unique = new Map();
  for (const c of [...files.flatMap((f) => f.chunks), ...manifestChunks]) unique.set(c.address, c);
  const manifestHash = toHex(keccak(manifest));
  return {entry, files, totalBytes, manifest, manifestHash, planHash:manifestHash, chunks:[...unique.values()], manifestChunks};
}
// A caller's plan is re-prepared from its files (so a mutation after preparation is detected); the private copy made
// here is never handed out, so this module reuses it without preparing it again.
const prepared = new WeakSet();
function snapshot(plan) {
  if (prepared.has(plan)) return plan;
  assert(plan && Array.isArray(plan.files), 'Invalid publish plan');
  const copy = prepareSite(plan.files, {entry:plan.entry});
  assert(copy.manifestHash === plan.manifestHash, 'Publish plan changed after preparation');
  prepared.add(copy);
  return copy;
}
const chunkGas = (size) => BigInt(Math.min(200000 + size * 260, 7900000));
export const DEPLOY_GAS = 1500000n, CREATE_VERSION_GAS = 600000n, PUBLISH_GAS = 200000n, SET_CURRENT_GAS = 200000n;
/** The planned transactions in signing order: each chunk, the Website deploy (new site), createVersion, publish,
 *  setCurrent. A chunk the chain already holds is skipped at publication, so this is an upper bound. */
export function planTransactions(plan, {site = null} = {}) {
  const p = snapshot(plan); if (site) address(site);
  return [...p.chunks.map((c) => ({id:'chunk:' + c.address, gas:chunkGas(c.data.length), gasUsed:chunkGasUsed(c.data.length) < chunkGas(c.data.length) ? chunkGasUsed(c.data.length) : chunkGas(c.data.length)})), ...(site ? [] : [{id:'deploy', gas:DEPLOY_GAS}]),
    {id:'create-version', gas:CREATE_VERSION_GAS}, {id:'publish-version', gas:PUBLISH_GAS}, {id:'set-current', gas:SET_CURRENT_GAS}];
}
/** The legacy maximum (schema 1): every transaction at 200 gwei. Used only to recognise schema-1 checkpoints. */
function legacyBudget(p, site) {
  const maxGas = p.chunks.reduce((sum, c) => sum + chunkGas(c.data.length), 0n) + (site ? 0n : 1500000n) + 1000000n;
  return maxGas * BigInt(LEGACY_MAX_FEE);
}
/**
 * Sites W4 quote. `fees` ({baseNext, tip}, from readNetworkFees) is required for the fee part: the projected London
 * schedule over the planned transactions, one budget and one per-gas ceiling (see fees.js). `economy` paces one
 * publication transaction every second block. `txs` restricts the quote to the remaining transactions (re-approval).
 * Returns the approval the user signs off: {budgetWei, ceilingWei, tipWei, economy, schedule:{id:maxFee}}.
 */
export function quotePublish(plan, options = {}) {
  const p = snapshot(plan); if (options.site) address(options.site);
  const all = planTransactions(p, {site:options.site || null});
  const maxGas = all.reduce((n, t) => n + t.gas, 0n);
  const out = {planHash:p.planHash, maxGas:maxGas.toString(), transactionCount:all.length, bytes:p.totalBytes, files:p.files.length};
  if (!options.fees) return out;
  const wanted = options.txs ? new Set(options.txs) : null, txs = wanted ? all.filter((t) => wanted.has(t.id)) : all;
  const tip = uint(options.tip ?? options.fees.tip, 'tip');
  const s = feeSchedule({txs, baseNext:options.fees.baseNext, tip, economy:!!options.economy, floor:options.floor ?? BASE_FEE_FLOOR});
  const approval = {budgetWei:s.budgetWei, ceilingWei:s.ceilingWei, tipWei:tip.toString(), economy:!!options.economy, schedule:Object.fromEntries(s.schedule.map((x) => [x.id, x.maxFee]))};
  return {...out, quotedTransactions:txs.length, quotedGas:txs.reduce((n, t) => n + t.gas, 0n).toString(), expectedWei:s.expectedWei, budgetWei:s.budgetWei, ceilingWei:s.ceilingWei,
    baseNextWei:BigInt(options.fees.baseNext).toString(), tipWei:tip.toString(), economy:!!options.economy, schedule:s.schedule, approval};
}
/** The transactions a checkpoint has not signed yet, for a re-approval after a "fee rose" pause. */
export function remainingTransactions(plan, state, {site = null} = {}) {
  const done = new Set(Object.keys(state?.transactions || {}));
  return planTransactions(plan, {site:state?.site || site}).filter((t) => !done.has(t.id)).map((t) => t.id);
}
function checkedApproval(a) {
  assert(a && typeof a === 'object', 'An approved fee budget is required');
  const budget = uint(a.budgetWei, 'approved budget'), ceiling = uint(a.ceilingWei, 'approved fee ceiling'), tip = uint(a.tipWei, 'approved tip');
  assert(budget > 0n && ceiling > 0n && ceiling <= ABSOLUTE_CEILING, 'The approved fee ceiling must be between 1 wei and 200 gwei per gas');
  assert(tip <= TIP_CAP && tip <= ceiling, 'Invalid approved tip');
  assert(a.schedule && typeof a.schedule === 'object' && !Array.isArray(a.schedule), 'Invalid fee schedule');
  const schedule = new Map();
  for (const [id, v] of Object.entries(a.schedule)) { const fee = uint(v, 'scheduled fee'); assert(fee <= ceiling && fee >= tip, 'A scheduled fee is outside the approved ceiling'); schedule.set(id, fee); }
  const economy = a.economy === true;
  const digest = toHex(keccak(enc.encode(JSON.stringify([budget.toString(), ceiling.toString(), tip.toString(), economy, [...schedule].map(([k, v]) => [k, v.toString()]).sort()]))));
  return {budget, ceiling, tip, economy, schedule, digest};
}
export class PublishFeeRoseError extends Error {
  constructor(details) {
    super(details.reason === 'ceiling'
      ? `Network fees rose: the next transaction needs ${details.maxFee} wei per gas, above the approved ceiling of ${details.ceiling}. Confirmed chunks are kept; approve a new budget for the remaining transactions to continue.`
      : `Network fees rose: the remaining approved budget does not cover the next transaction. Confirmed chunks are kept; approve a new budget for the remaining transactions to continue.`);
    this.name = 'PublishFeeRoseError'; this.code = 'PUBLISH_FEE_ROSE'; Object.assign(this, details);
  }
}
function decodeWord(data, index, max = (1n << 256n) - 1n) {
  assert(typeof data === 'string' && hexRx.test(data), 'Invalid ABI response');
  const bytes = fromHex(data); assert(bytes.length >= (index + 1) * 32, 'Short ABI response');
  const n = BigInt(toHex(bytes.subarray(index * 32, (index + 1) * 32))); assert(n <= max, 'ABI integer out of range'); return n;
}
function versionMatches(data, plan) {
  const bytes = fromHex(data), offset = Number(decodeWord(data,4,BigInt(bytes.length)));
  assert(offset % 32 === 0 && offset >= 160 && offset + 32 <= bytes.length,'Invalid version chunk offset');
  const count = Number(decodeWord(data,offset / 32,3n));
  if (toHex(bytes.subarray(0,32)) !== plan.manifestHash || decodeWord(data,1,65536n) !== BigInt(plan.manifest.length) || count !== plan.manifestChunks.length) return false;
  for (let i = 0; i < count; i++) {
    const value = decodeWord(data,offset / 32 + 1 + i,(1n << 160n) - 1n);
    if ('0x' + value.toString(16).padStart(40,'0') !== plan.manifestChunks[i].address) return false;
  }
  return true;
}
function padded(n, size) { const b = rlp.bytes(n); assert(b.length <= size, 'Invalid signature integer'); const out = new Uint8Array(size); out.set(b, size - b.length); return out; }
function verifySigned(signed, tx, from) {
  assert(signed && typeof signed.raw === 'string' && hexRx.test(signed.raw), 'Signer returned invalid transaction');
  const raw = fromHex(signed.raw); assert(raw[0] === 2, 'Only EIP-1559 transactions are allowed');
  const fields = rlp.decode(raw.subarray(1)); assert(Array.isArray(fields) && fields.length === 12, 'Invalid signed transaction');
  const ut = unsignedTx(tx);
  assert(eq(rlp.encode(fields.slice(0, 9)), rlp.encode(ut.fields)), 'Signer changed transaction fields');
  const parity = rlp.uint(fields[9],1,'parity'); assert(parity <= 1n, 'Invalid parity');
  const signature = concat(padded(fields[10],32),padded(fields[11],32),Uint8Array.of(Number(parity)));
  assert(toHex(recover(ut.signingHash,signature)) === from, 'Signature does not match publishing account');
  const hash = toHex(keccak(raw)); if (signed.hash) assert(hash === signed.hash.toLowerCase(), 'Signed transaction hash mismatch');
  return {raw:toHex(raw), hash};
}
export class PublishPendingError extends Error {
  constructor(hash, message = 'Transaction confirmation is pending. Resume this publication; do not start a new one.') { super(message); this.name = 'PublishPendingError'; this.code = 'PUBLISH_PENDING'; this.transactionHash = hash; }
}
export class PublishRevertedError extends Error {
  constructor(hash) { super('Publication transaction reverted: ' + hash); this.name = 'PublishRevertedError'; this.code = 'PUBLISH_REVERTED'; this.transactionHash = hash; }
}
// FN-1 (C9): pocol_getCaps pre-check. What a publication adds to the chain caps, at most, given which chunks the chain
// already holds: each missing chunk a ChunkFactory account with its 0x00-prefixed runtime, a new site the Website
// account with its runtime and its constructor's three slots (owner, publishers[sender], publisherCount) plus the
// version counter, and the version record's manifestHash, meta, chunk-array length and one slot per manifest chunk
// (Website.sol; publish and setCurrent rewrite existing slots). A raw transaction is counted at its calldata plus
// TX_ENVELOPE_MAX bytes; Website and ChunkFactory emit no logs. pocold tests/fn1_caps.rs
// (a_website_publication_adds_what_the_publisher_pre_check_expects) executes a publication of the shipped Website
// contract and checks these slot, account and code counts. `largestTxBytes` is the largest single transaction: on a
// continuous chain (pocol_getCaps `cumulative` false, H_END = 0) item 16 counts each block from zero, and each of the
// publication's transactions is confirmed before the next is sent, so each must fit one block's room on its own.
export const CAPS_ALARM_PERCENT = 80;
export const TX_ENVELOPE_MAX = 160;
const CAP_FIELDS = [['accounts','accountsCap','accounts (ACCT_CAP)'],['storageSlots','storageSlotsCap','storage slots (SLOT_CAP)'],['codeBytes','codeBytesCap','code bytes (CODE_CAP)'],
  ['txCount','txCap','transactions (TX_CAP)'],['txBytes','txBytesCap','transaction bytes (TXBYTES_CAP)'],['logBytes','logCap','log bytes (LOG_CAP)']];
// Counted per block, not over the branch, when pocol_getCaps reports `cumulative` false.
const PER_BLOCK = new Set(['txCount','txBytes','logBytes']);
const hexLen = (h) => (h.length - 2) / 2;
export function publishNeeds(plan, {newSite, missing}) {
  const p = snapshot(plan); const absent = p.chunks.filter((c) => missing.has(c.address));
  const storeData = (n) => 4 + 64 + Math.ceil(n / 32) * 32;
  const versionData = 4 + 96 + 32 + 32 * p.manifestChunks.length;
  const txs = absent.length + (newSite ? 1 : 0) + 3;
  const largest = Math.max(versionData, 36, newSite ? hexLen(WEBSITE_INITCODE) : 0, ...absent.map((c) => storeData(c.data.length)));
  return {
    accounts: absent.length + (newSite ? 1 : 0),
    storageSlots: (newSite ? 4 : 0) + 3 + p.manifestChunks.length,
    codeBytes: absent.reduce((n, c) => n + 1 + c.data.length, 0) + (newSite ? hexLen(WEBSITE_RUNTIME) : 0),
    txCount: txs,
    txBytes: absent.reduce((n, c) => n + storeData(c.data.length), 0) + (newSite ? hexLen(WEBSITE_INITCODE) : 0) + versionData + 36 + 36 + txs * TX_ENVELOPE_MAX,
    logBytes: 0,
    largestTxBytes: largest + TX_ENVELOPE_MAX,
  };
}
/// What a cap's room must hold: the whole publication, or (the per-block rows of a continuous chain) one transaction,
/// its largest transaction's bytes, and the publication's log bytes (a bound on any one transaction's: 0 here).
function needOf(needs, name, perBlock) {
  if (!perBlock || !PER_BLOCK.has(name)) return BigInt(needs[name] ?? 0);
  if (name === 'txCount') return BigInt(needs.txCount ?? 0) > 0n ? 1n : 0n;
  if (name === 'txBytes') return BigInt(needs.largestTxBytes ?? needs.txBytes ?? 0);
  return BigInt(needs.logBytes ?? 0);
}
export class PublishCapsError extends Error {
  constructor(shortfalls) {
    super('This site cannot fit on the network: ' + shortfalls.map((x) => `it needs ${x.need} ${x.label} and ${x.remaining} remain`).join('; ') + '. Nothing was signed.');
    this.name = 'PublishCapsError'; this.code = 'PUBLISH_CAPS'; this.shortfalls = shortfalls;
  }
}
/// Reads pocol_getCaps and compares a publication's needs with each cap's room. Returns {checked:false, warning} when the
/// node predates the method (-32601: the pre-check is skipped), else {checked:true, caps, alarms}; throws
/// PublishCapsError when a cap cannot hold the site. With `cumulative` false (H_END = 0) the txCount, txBytes and
/// logBytes rows are a block's room, so each transaction is checked against it, not the whole publication; an answer
/// without the field is read as cumulative (the stricter check).
export async function capsPrecheck(rpc, needs) {
  let caps;
  try { caps = await rpc('pocol_getCaps', []); }
  catch (e) {
    // An older node (-32601), or a relay that does not forward the method (an older Studio gateway refuses it with an
    // HTTP error, a transport failure here): the pre-check is skipped. Every later request still checks the network.
    if (e?.rpc?.code === -32601) return {checked:false, warning:'This node does not report its chain caps (pocol_getCaps): the capacity pre-check was skipped.'};
    if (e?.transport) return {checked:false, warning:'The network endpoint did not answer pocol_getCaps (' + e.transport + '): the capacity pre-check was skipped.'};
    throw e;
  }
  assert(caps && typeof caps === 'object' && caps.occupancy && typeof caps.occupancy === 'object', 'Invalid pocol_getCaps response');
  assert(caps.cumulative === undefined || typeof caps.cumulative === 'boolean', 'Invalid pocol_getCaps response');
  const perBlock = caps.cumulative === false;
  const shortfalls = [], alarms = [];
  for (const [name, capName, label] of CAP_FIELDS) {
    const count = uint(caps[name], 'pocol_getCaps ' + name), cap = uint(caps[capName], 'pocol_getCaps ' + capName);
    const block = perBlock && PER_BLOCK.has(name);
    const remaining = cap > count ? cap - count : 0n, need = needOf(needs, name, perBlock);
    if (need > remaining) shortfalls.push({name, label:block ? label.replace(' (', ' in one block (') : label, need:need.toString(), remaining:remaining.toString()});
    if (count * 100n >= cap * BigInt(CAPS_ALARM_PERCENT)) alarms.push(name);
  }
  if (shortfalls.length) throw new PublishCapsError(shortfalls);
  return {checked:true, caps, alarms};
}
/// The chunk reads before and during the upload are paced on the client: a burst of READ_BURST, then READ_RATE per
/// second. The Studio gateway allows each client a burst of 40 and 10 requests per second (pocol_studio_gateway.py) and
/// answers 429 past that; the publication's other requests share that budget.
export const READ_BURST = 16, READ_RATE = 8;
/// The checkpoint (schema 2) binds the manifest, the publishing account, the target site and the network (chain id and
/// genesis). The fees are bound by the approvals: each records the budget, the ceiling, the tip and the pacing the user
/// approved, and each signed record names the approval it was signed under, so a resume after a re-approval keeps the
/// records signed with other fee caps (each must stay within its own approval's ceiling).
const bindingOf = ({manifestHash, from, chain, genesis, initialSite}) => toHex(keccak(enc.encode(JSON.stringify(['pocol-publish-2', manifestHash, from, chain.toString(), genesis, initialSite]))));
const legacyBindingOf = ({manifestHash, from, chain, genesis, initialSite, maxFee, priority, budget}) => toHex(keccak(enc.encode(JSON.stringify([manifestHash, from, chain.toString(), genesis, initialSite, maxFee, priority, budget]))));
/// A schema-1 checkpoint (fixed 200 gwei caps) is migrated only when its binding proves it belongs to this manifest,
/// account, site and network: its binding hash is recomputed from the current NETWORK and the fixed legacy fees.
/// Otherwise it belongs to another network (or was tampered with) and the caller archives the job.
export function migrateCheckpoint(state, {plan, address: from, chainId, genesisHash, site = null}) {
  if (!state || state.schema !== 1) return state;
  const p = snapshot(plan), chain = uint(chainId, 'chain ID'), genesis = String(genesisHash).toLowerCase(), initialSite = site ? address(site) : null;
  const recs = Object.values(state.transactions || {});
  const maxFee = recs[0]?.tx?.maxFee ?? LEGACY_MAX_FEE, priority = recs[0]?.tx?.maxPriority ?? LEGACY_PRIORITY_FEE;
  const budget = legacyBudget(p, initialSite).toString();
  const legacy = legacyBindingOf({manifestHash:p.manifestHash, from, chain, genesis, initialSite, maxFee:String(maxFee), priority:String(priority), budget});
  assert(state.binding === legacy, 'This saved publication belongs to another network or approval. Archive it; it cannot be resumed here.');
  const out = jsonClone(state);
  for (const rec of Object.values(out.transactions || {})) rec.approval = 0;
  return {...out, schema:2, migratedFrom:1, binding:bindingOf({manifestHash:p.manifestHash, from, chain, genesis, initialSite}), network:{chainId:chain.toString(), genesisHash:genesis},
    approvals:[{budgetWei:budget, ceilingWei:String(maxFee), tipWei:String(priority), economy:false, spentBeforeWei:'0', digest:'legacy-schema-1'}]};
}
const running = new Set();
const pauseError = () => Object.assign(new Error('Publication paused'), {code:'PUBLISH_PAUSED'});
/**
 * Publishes `options.plan` (prepareSite) and returns {site, version, manifestHash, publishedBlock, transactions,
 * maxFeeCommittedWei, verified}. Options: rpc, sign, onCheckpoint (durable), address, chainId, genesisHash, site,
 * approval (quotePublish(...).approval), reapprove (true appends a new approval to a saved checkpoint, after a "fee
 * rose" pause), state (the saved checkpoint), signal, onProgress, onWarning, readBurst/readRate, clock/sleep (tests),
 * pollIntervalMs, receiptTimeoutMs, sweepTimeoutMs, blockWaitMs, and rawStore.
 * `rawStore` ({put(hash, raw), get(hash)}, optional): each signed raw transaction is written there once, durably and
 * before the checkpoint that names it, instead of inside the checkpoint, so a checkpoint save stays small (a 4 MiB site
 * has ~175 raw transactions of up to 49 KB). Without it the raw transactions stay in the checkpoint.
 */
export async function publishSite(options) {
  const {rpc, sign, onCheckpoint, onProgress = () => {}, signal} = options;
  assert(typeof rpc === 'function' && typeof sign === 'function', 'RPC and signer are required');
  assert(typeof onCheckpoint === 'function', 'Durable publication checkpoint callback is required');
  const plan = snapshot(options.plan), from = address(options.address), chain = uint(options.chainId,'chain ID');
  assert(chain > 0n, 'Invalid chain ID');
  assert(/^0x[0-9a-fA-F]{64}$/.test(options.genesisHash || ''), 'Expected genesis hash is required');
  const genesis = options.genesisHash.toLowerCase(), initialSite = options.site ? address(options.site) : null;
  const approval = checkedApproval(options.approval);
  const binding = bindingOf({manifestHash:plan.manifestHash, from, chain, genesis, initialSite});
  const key = chain + ':' + genesis + ':' + from;
  assert(!running.has(key), 'Another publication is already using this account');
  running.add(key);
  try {
    const approvalRecord = (spentBefore) => ({budgetWei:approval.budget.toString(), ceilingWei:approval.ceiling.toString(), tipWei:approval.tip.toString(), economy:approval.economy, spentBeforeWei:spentBefore, digest:approval.digest});
    let state = options.state ? jsonClone(options.state) : {schema:2,binding,network:{chainId:chain.toString(),genesisHash:genesis},site:initialSite,version:null,versionCountBefore:null,transactions:{},spentCapWei:'0',approvals:[approvalRecord('0')]};
    if (state.schema === 1) state = migrateCheckpoint(state, {plan, address:from, chainId:chain, genesisHash:genesis, site:initialSite});
    assert(state.schema === 2 && state.transactions && typeof state.transactions === 'object' && Array.isArray(state.approvals) && state.approvals.length > 0 && state.approvals.length <= 64, 'Checkpoint belongs to another publication or approval');
    assert(state.network?.chainId === chain.toString() && state.network?.genesisHash === genesis, 'This saved publication belongs to another network. Archive it; it cannot be resumed here.');
    assert(state.binding === binding, 'Checkpoint belongs to another publication or approval');
    if (state.approvals[state.approvals.length - 1].digest !== approval.digest) {
      assert(options.reapprove === true, 'Checkpoint belongs to another publication or approval');
      state.approvals.push(approvalRecord(String(state.spentCapWei)));
    }
    // Fee ledger: every record within its own approval's ceiling and the approval's budget, and the total exact.
    const spentBy = state.approvals.map(() => 0n); let recordedCost = 0n;
    for (const rec of Object.values(state.transactions)) {
      const a = state.approvals[rec.approval]; assert(Number.isInteger(rec.approval) && a, 'Checkpoint fee accounting mismatch');
      const fee = uint(rec.tx.maxFee), prio = uint(rec.tx.maxPriority), cost = uint(rec.tx.gas) * fee;
      assert(fee <= uint(a.ceilingWei) && prio <= fee, 'A saved transaction exceeds its approved fee ceiling');
      spentBy[rec.approval] += cost; recordedCost += cost;
    }
    assert(recordedCost === uint(state.spentCapWei), 'Checkpoint fee accounting mismatch');
    state.approvals.forEach((a, i) => assert(spentBy[i] <= uint(a.budgetWei), 'Checkpoint exceeds approved fee'));
    const current = state.approvals.length - 1, spentBefore = uint(state.approvals[current].spentBeforeWei);
    const checkAbort = () => { if (signal?.aborted) throw pauseError(); };
    const readBurst = options.readBurst ?? READ_BURST, readRate = options.readRate ?? READ_RATE;
    assert(Number.isFinite(readBurst) && readBurst >= 1 && Number.isFinite(readRate) && readRate > 0, 'Invalid chunk read pacing');
    // `clock` and `sleep` default to the wall clock and a timer; tests pass a virtual clock that only sleeping advances.
    const clock = options.clock ?? Date.now, sleep = options.sleep ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
    const pollMs = options.pollIntervalMs ?? 1000;
    let tokens = readBurst, refilled = clock();
    const pace = async () => {
      for (;;) {
        const now = clock(); tokens = Math.min(readBurst, tokens + Math.max(0, now - refilled) * readRate / 1000); refilled = now;
        if (tokens >= 1) { tokens -= 1; return; }
        checkAbort(); await sleep(Math.ceil((1 - tokens) * 1000 / readRate));
      }
    };
    const readCode = async (a, tag = 'latest') => { await pace(); checkAbort(); return rpc('eth_getCode',[a,tag]); };
    const save = async () => { await onCheckpoint(jsonClone(state)); };
    const progress = (data) => { try { onProgress({...data, site:state.site, manifestHash:plan.manifestHash}); } catch {} };
    const network = async () => {
      checkAbort();
      const cid = await rpc('eth_chainId', []), status = await rpc('pocol_status', []);
      assert(uint(cid) === chain && typeof status?.genesisHash === 'string' && status.genesisHash.toLowerCase() === genesis, 'RPC network identity does not match PoCol profile');
    };
    const call = (to, data) => rpc('eth_call',[{to,data},'latest']);
    const authorize = async (site) => {
      const code = await rpc('eth_getCode',[site,'latest']);
      assert(typeof code === 'string' && code.toLowerCase() === WEBSITE_RUNTIME.toLowerCase(), 'Target is not the supported PoCol Website contract');
      const allowed = await call(site,abiAddress('isPublisher(address)',from));
      assert(decodeWord(allowed,0,1n) === 1n, 'This wallet is not authorized to publish this site');
    };
    const checkedReceipt = (r, hash) => {
      if (!r) return null;
      assert(typeof r.transactionHash === 'string' && r.transactionHash.toLowerCase() === hash, 'Receipt hash mismatch');
      assert(r.status === '0x1' || r.status === '0x0', 'Invalid receipt status');
      return r;
    };
    const canonicalReceipt = async (hash) => {
      const receipt = checkedReceipt(await rpc('eth_getTransactionReceipt',[hash]),hash);
      if (!receipt) return null;
      assert(/^0x[0-9a-fA-F]{64}$/.test(receipt.blockHash || '') && /^0x[0-9a-fA-F]+$/.test(receipt.blockNumber || ''),'Receipt is missing block identity');
      const block = await rpc('eth_getBlockByNumber',[receipt.blockNumber,false]);
      return block && typeof block.hash === 'string' && block.hash.toLowerCase() === receipt.blockHash.toLowerCase() ? receipt : null;
    };
    const quantityOf = (v, name) => { assert(typeof v === 'string' && /^0x[0-9a-fA-F]{1,64}$/.test(v), 'Invalid ' + name); return BigInt(v); };
    // Just-in-time fees: the next block's base fee (the node's own eth_feeHistory figure, else the London step from the
    // latest block), read before each signature.
    const baseNext = async () => {
      try { const h = await rpc('eth_feeHistory',[1,'latest',[]]); const fees = h?.baseFeePerGas; assert(Array.isArray(fees) && fees.length > 0, 'Invalid fee history'); return quantityOf(fees[fees.length - 1], 'base fee'); }
      catch (e) { if (!e?.rpc && !e?.transport && !/fee history|base fee/.test(e?.message || '')) throw e; }
      const b = await rpc('eth_getBlockByNumber',['latest',false]);
      assert(b && b.baseFeePerGas !== undefined, 'The network did not report its base fee');
      return nextBaseFee(quantityOf(b.baseFeePerGas,'base fee'), quantityOf(b.gasUsed,'gas used'), quantityOf(b.gasLimit,'gas limit'), BASE_FEE_FLOOR);
    };
    const rawStore = options.rawStore || null;
    assert(!rawStore || (typeof rawStore.put === 'function' && typeof rawStore.get === 'function'), 'Invalid raw transaction store');
    const rawOf = async (rec) => {
      const raw = rec.raw ?? (rec.rawStored && rawStore ? await rawStore.get(rec.hash) : null);
      assert(typeof raw === 'string' && hexRx.test(raw), 'The saved signed transaction ' + rec.hash + ' is missing; it cannot be checked or resent');
      return raw;
    };
    const lastBlock = () => Object.values(state.transactions).reduce((m, r) => (r.receipt?.blockNumber ? (BigInt(r.receipt.blockNumber) > m ? BigInt(r.receipt.blockNumber) : m) : m), -1n);
    // Economy pacing: the next publication transaction is signed only once the block after the last one exists, so at
    // most every second block carries one (a full block raises the base fee by ~4.4 %, an empty one lowers it by 12.5 %).
    const economyWait = async () => {
      const last = lastBlock(); if (last < 0n) return;
      const deadline = clock() + (options.blockWaitMs ?? 600000);
      for (;;) {
        checkAbort();
        if (quantityOf(await rpc('eth_blockNumber', []), 'block number') > last) return;
        if (clock() >= deadline) throw new PublishPendingError(null, 'The network has not produced a new block. Resume this publication later.');
        await sleep(Math.max(1, pollMs));
      }
    };
    async function transaction(id, to, data, gas) {
      checkAbort();
      let rec = state.transactions[id];
      if (!rec) {
        await network();
        if (approval.economy) await economyWait();
        const base = await baseNext();
        let maxFee = capFor(base, approval.tip, MARGIN_PER_MILLE); const planned = approval.schedule.get(id);
        if (planned !== undefined && planned > maxFee) maxFee = planned;
        const spent = uint(state.spentCapWei), cost = gas * maxFee;
        if (maxFee > approval.ceiling) throw new PublishFeeRoseError({reason:'ceiling', step:id, baseNext:base.toString(), maxFee:maxFee.toString(), ceiling:approval.ceiling.toString()});
        if (spent - spentBefore + cost > approval.budget) throw new PublishFeeRoseError({reason:'budget', step:id, baseNext:base.toString(), maxFee:maxFee.toString(), ceiling:approval.ceiling.toString(), remainingWei:(approval.budget - (spent - spentBefore)).toString()});
        const balance = uint(await rpc('eth_getBalance',[from,'pending']));
        assert(balance >= cost, 'Insufficient balance for the next publication transaction');
        const nonce = uint(await rpc('eth_getTransactionCount',[from,'pending']));
        const tx = {chainId:chain.toString(),nonce:nonce.toString(),maxPriority:approval.tip.toString(),maxFee:maxFee.toString(),gas:gas.toString(),to,value:'0',data};
        await network(); checkAbort();
        const signed = verifySigned(await sign(jsonClone(tx)),tx,from);
        // The record keeps the calldata's hash, not the calldata: the plan rebuilds it (a chunk's calldata is its bytes).
        const {data:_, ...fields} = tx, saved = {...fields, dataHash:toHex(keccak(fromHex(data)))};
        if (rawStore) { await rawStore.put(signed.hash, signed.raw); rec = state.transactions[id] = {hash:signed.hash,rawStored:true,tx:saved,receipt:null,approval:current}; }
        else rec = state.transactions[id] = {...signed,tx:saved,receipt:null,approval:current};
        state.spentCapWei = (spent + cost).toString();
        await save(); // Must commit the signed raw transaction BEFORE any broadcast.
      } else {
        const a = state.approvals[rec.approval];
        const sameData = rec.tx.dataHash !== undefined ? rec.tx.dataHash === toHex(keccak(fromHex(data))) : rec.tx.data === data;
        assert(rec.tx.to === to && sameData && uint(rec.tx.gas) === gas && uint(rec.tx.chainId) === chain && uint(rec.tx.maxFee) <= uint(a.ceilingWei) && uint(rec.tx.maxPriority) <= uint(rec.tx.maxFee) && uint(rec.tx.value) === 0n, 'Checkpoint transaction does not match publication');
        const {dataHash:_, ...fields} = rec.tx;
        verifySigned({raw:await rawOf(rec), hash:rec.hash},{...fields, data},from);
      }
      await network();
      let receipt = await canonicalReceipt(rec.hash);
      if (!receipt) {
        checkAbort(); progress({phase:'broadcast',step:id,transactionHash:rec.hash});
        try { const h = await rpc('eth_sendRawTransaction',[await rawOf(rec)]); assert(typeof h === 'string' && h.toLowerCase() === rec.hash,'RPC returned a different transaction hash'); }
        catch (error) {
          receipt = await canonicalReceipt(rec.hash);
          if (!receipt) throw new PublishPendingError(rec.hash,'Submission could not be confirmed. Resume to check or retransmit the same signed transaction.');
        }
      }
      const deadline = clock() + (options.receiptTimeoutMs ?? 300000);
      while (!receipt) {
        checkAbort();
        if (clock() >= deadline) throw new PublishPendingError(rec.hash);
        await sleep(pollMs);
        receipt = await canonicalReceipt(rec.hash);
      }
      rec.receipt = receipt; await save();
      if (receipt.status === '0x0') throw new PublishRevertedError(rec.hash);
      progress({phase:'confirmed',step:id,transactionHash:rec.hash});
      return receipt;
    }
    const storeChunk = async (c) => {
      await transaction('chunk:'+c.address,FACTORY,abiBytes('store(bytes)',c.data),chunkGas(c.data.length));
      const code = await readCode(c.address); if (code !== '0x') chunkPayload(fromHex(c.address),fromHex(code),c.data.length);
      return code;
    };
    await network();
    // A persisted receipt is not finality: refresh it against the canonical block before resume.
    // Reuse saved raw transactions after a reorg; never silently sign replacements at new nonces.
    for (const [id, rec] of Object.entries(state.transactions)) {
      if (!rec.receipt) continue;
      const canonical = await canonicalReceipt(rec.hash);
      rec.receipt = canonical;
      if (!canonical && id === 'deploy') { state.site = initialSite; state.version = null; state.versionCountBefore = null; state.complete = false; }
      if (!canonical && id === 'create-version') { state.version = null; state.versionCountBefore = null; state.complete = false; }
      if (!canonical) state.complete = false;
    }
    if (state.site) await authorize(address(state.site));
    // FN-1 (C9): before the first signature of a publication, refuse a site the chain caps cannot hold. A resumed
    // publication with signed transactions is not re-checked: those are committed and are reconciled as they are.
    // The scan's stored chunks are reused by the upload below (a chunk's code never changes once stored), so each is
    // read once; a chunk the scan found missing is read again, as another publisher may store it meanwhile. A scan
    // read the network refuses (a 429 from the gateway is a transport failure here) or answers with an error skips the
    // pre-check, as an unanswered pocol_getCaps does.
    const scanned = new Map();
    if (!Object.keys(state.transactions).length) {
      const missing = new Set(); let pre = null;
      try { for (const c of plan.chunks) { checkAbort(); const code = await readCode(c.address); scanned.set(c.address, code); if (code === '0x') missing.add(c.address); } }
      catch (e) {
        if (!e?.transport && !e?.rpc) throw e;
        pre = {checked:false, warning:'The network endpoint did not answer the chunk scan (eth_getCode: ' + (e.transport || e.rpc?.message || 'error') + '): the capacity pre-check was skipped.'};
      }
      pre ??= await capsPrecheck(rpc, publishNeeds(plan, {newSite:!state.site, missing}));
      if (!pre.checked) { try { options.onWarning?.(pre.warning); } catch {} progress({phase:'warning', message:pre.warning}); }
      else if (pre.alarms.length) progress({phase:'capsAlarm', alarms:pre.alarms});
    }
    await save();
    let completed = 0;
    for (const c of plan.chunks) {
      checkAbort();
      const known = scanned.get(c.address);
      const code = typeof known === 'string' && known !== '0x' ? known : await readCode(c.address);
      if (code !== '0x') chunkPayload(fromHex(c.address),fromHex(code),c.data.length);
      // Another publisher may store this chunk while our signed transaction is
      // unresolved. Reconcile that exact transaction before using another nonce;
      // content availability alone does not settle the account's pending work.
      if (code === '0x' || state.transactions['chunk:'+c.address]) {
        const stored = await storeChunk(c); assert(stored !== '0x', 'chunk not stored');
      }
      progress({phase:'upload',completed:++completed,total:plan.chunks.length});
    }
    if (!state.site) {
      const receipt = await transaction('deploy',null,WEBSITE_INITCODE,DEPLOY_GAS);
      const rec = state.transactions.deploy;
      const expected = toHex(keccak(rlp.encode([fromHex(from),rlp.encUint(BigInt(rec.tx.nonce))])).subarray(12));
      assert(address(receipt.contractAddress) === expected, 'Deployed contract address mismatch');
      state.site = expected; await save(); await authorize(state.site);
    }
    const target = state.site;
    if (state.version === null) {
      // Sites W4: the safe sweep. Before createVersion references them, every chunk must exist at the `safe` tag: a
      // chunk seen at `latest` (stored by another publisher, or served from a cache) may still be reorged out. A chunk
      // missing at both `safe` and `latest` is stored (again); the sweep waits, bounded, for the rest to reach `safe`.
      const sweepEnd = clock() + (options.sweepTimeoutMs ?? 180000);
      for (;;) {
        checkAbort(); const missing = [];
        for (const c of plan.chunks) { const code = await readCode(c.address, 'safe'); if (code === '0x') missing.push(c); else chunkPayload(fromHex(c.address),fromHex(code),c.data.length); }
        progress({phase:'sweep', missing:missing.length, total:plan.chunks.length});
        if (!missing.length) break;
        for (const c of missing) if (await readCode(c.address) === '0x') { progress({phase:'restore', chunk:c.address}); await storeChunk(c); }
        if (clock() >= sweepEnd) throw new PublishPendingError(null, 'Some chunks have not reached the safe height yet. Resume this publication to check again.');
        await sleep(Math.max(1, pollMs));
      }
    }
    if (state.versionCountBefore === null) {
      state.versionCountBefore = decodeWord(await call(target,selector('versionCount()')),0,BigInt(Number.MAX_SAFE_INTEGER)).toString(); await save();
    }
    if (state.version === null) {
      await transaction('create-version',target,abiCreateVersion(plan.manifestHash,plan.manifest.length,plan.manifestChunks),CREATE_VERSION_GAS);
      const count = decodeWord(await call(target,selector('versionCount()')),0,BigInt(Number.MAX_SAFE_INTEGER));
      const before = uint(state.versionCountBefore); assert(count >= before && count - before <= 1024n,'Unexpected concurrent version count');
      for (let v = before; v < count; v++) {
        const out = await call(target,abiUint('version(uint256)',v));
        const status = decodeWord(out,2,255n);
        if (versionMatches(out,plan) && (status === 0n || status === 1n)) { state.version = v.toString(); await save(); break; }
      }
      assert(state.version !== null,'Created site version was not found');
    }
    const version = uint(state.version);
    const chosen = await call(target,abiUint('version(uint256)',version));
    assert(versionMatches(chosen,plan), 'Checkpoint version does not match publication');
    const versionStatus = decodeWord(chosen,2,255n); assert(versionStatus === 0n || versionStatus === 1n,'Publication version was revoked');
    if (versionStatus === 0n || state.transactions['publish-version']) await transaction('publish-version',target,abiUint('publish(uint256)',version),PUBLISH_GAS);
    const setReceipt = await transaction('set-current',target,abiUint('setCurrent(uint256)',version),SET_CURRENT_GAS);
    const cur = decodeCurrent(fromHex(await call(target,selector('current()'))));
    assert(BigInt(cur.version) === version && toHex(cur.manifestHash) === plan.manifestHash && cur.manifestLen === plan.manifest.length && cur.chunks.length === plan.manifestChunks.length && cur.chunks.every((c,i) => toHex(c) === plan.manifestChunks[i].address),'Published site verification failed');
    // Sites W4: once the version is at `safe`, read the site back as a viewer does (every chunk and hash verified).
    const safeEnd = clock() + (options.sweepTimeoutMs ?? 180000);
    for (;;) {
      checkAbort();
      const safe = await rpc('eth_getBlockByNumber',['safe',false]);
      if (safe && /^0x[0-9a-fA-F]+$/.test(safe.number || '') && BigInt(safe.number) >= BigInt(setReceipt.blockNumber)) break;
      if (clock() >= safeEnd) throw new PublishPendingError(state.transactions['set-current'].hash, 'The published version has not reached the safe height yet. Resume this publication to verify it.');
      await sleep(Math.max(1, pollMs));
    }
    const fetched = await fetchSite(async (method, params) => { if (method === 'eth_getCode') await pace(); checkAbort(); return rpc(method, params); }, target, {parallel:1});
    assert(fetched.files.size === plan.files.length && plan.files.every((f) => fetched.files.get(f.path) && toHex(keccak(fetched.files.get(f.path).data)) === f.hash), 'The published site read back differs from the plan');
    state.complete = true; await save();
    const result = {site:target,version:Number(version),manifestHash:plan.manifestHash,publishedBlock:cur.publishedBlock,transactions:Object.values(state.transactions).map((tx) => tx.hash),maxFeeCommittedWei:state.spentCapWei,verified:true};
    progress({phase:'complete',...result}); return result;
  } finally { running.delete(key); }
}
