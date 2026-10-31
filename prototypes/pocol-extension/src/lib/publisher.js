// Browser publisher for PoCol Website v1. Protocol parity: pocold/src/site.rs @50c2f27.
// Runs in the extension's trusted page. No private key is accepted or retained here.
import * as rlp from './rlp.js';
import { keccak, recover } from './crypto.js';
import { concat, eq, toHex, fromHex } from './hex.js';
import { unsignedTx } from './wallet.js';
import { validPath, chunkAddress, chunkPayload, decodeManifest, decodeCurrent, CHUNK_MAX, FILE_MAX, FILES_MAX, SITE_MAX } from './site.js';
import { WEBSITE_INITCODE, WEBSITE_RUNTIME } from './website-bytecode.js';

export const FACTORY = '0x0000000000000000000000000000000000c0c005';
export const DEFAULT_MAX_FEE = '200000000000';
export const DEFAULT_PRIORITY_FEE = '1000000000';
const enc = new TextEncoder();
const EXT_MIME = {html:1,htm:1,css:2,js:3,mjs:3,json:4,png:5,jpg:6,jpeg:6,webp:7,svg:8,woff2:9,txt:10};
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
    assert(f.data instanceof Uint8Array, 'File data must be Uint8Array');
    assert(f.data.length <= FILE_MAX, 'File exceeds 1 MiB: ' + f.path);
    totalBytes += f.data.length; assert(totalBytes <= SITE_MAX, 'Site exceeds 4 MiB');
    const mime = EXT_MIME[f.path.split('.').pop().toLowerCase()]; assert(mime, 'Unsupported file extension: ' + f.path);
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
function snapshot(plan) {
  assert(plan && Array.isArray(plan.files), 'Invalid publish plan');
  const copy = prepareSite(plan.files, {entry:plan.entry});
  assert(copy.manifestHash === plan.manifestHash, 'Publish plan changed after preparation');
  return copy;
}
function fees(options) {
  const maxFee = uint(options.maxFeePerGas ?? DEFAULT_MAX_FEE, 'maximum fee');
  const priority = uint(options.maxPriorityFeePerGas ?? DEFAULT_PRIORITY_FEE, 'priority fee');
  assert(maxFee > 0n && priority <= maxFee, 'Invalid fee caps');
  return {maxFee, priority};
}
const chunkGas = (size) => BigInt(Math.min(200000 + size * 260, 7900000));
export function quotePublish(plan, options = {}) {
  const p = snapshot(plan), f = fees(options); if (options.site) address(options.site);
  const maxGas = p.chunks.reduce((sum, c) => sum + chunkGas(c.data.length), 0n) + (options.site ? 0n : 1500000n) + 1000000n;
  return {planHash:p.planHash, maxGas:maxGas.toString(), maxFeeWei:(maxGas * f.maxFee).toString(), maxFeePerGas:f.maxFee.toString(), maxPriorityFeePerGas:f.priority.toString(), transactionCount:p.chunks.length + (options.site ? 3 : 4), bytes:p.totalBytes, files:p.files.length};
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
const running = new Set();
export async function publishSite(options) {
  const {rpc, sign, onCheckpoint, onProgress = () => {}, signal} = options;
  assert(typeof rpc === 'function' && typeof sign === 'function', 'RPC and signer are required');
  assert(typeof onCheckpoint === 'function', 'Durable publication checkpoint callback is required');
  const plan = snapshot(options.plan), from = address(options.address), chain = uint(options.chainId,'chain ID');
  assert(chain > 0n, 'Invalid chain ID');
  assert(/^0x[0-9a-fA-F]{64}$/.test(options.genesisHash || ''), 'Expected genesis hash is required');
  const genesis = options.genesisHash.toLowerCase(), initialSite = options.site ? address(options.site) : null;
  const f = fees(options), budget = uint(options.maxTotalFeeWei, 'approved total fee');
  const binding = toHex(keccak(enc.encode(JSON.stringify([plan.manifestHash,from,chain.toString(),genesis,initialSite,f.maxFee.toString(),f.priority.toString(),budget.toString()]))));
  const key = chain + ':' + genesis + ':' + from;
  assert(!running.has(key), 'Another publication is already using this account');
  running.add(key);
  try {
    let state = options.state ? jsonClone(options.state) : {schema:1,binding,site:initialSite,version:null,versionCountBefore:null,transactions:{},spentCapWei:'0'};
    assert(state.schema === 1 && state.binding === binding && state.transactions && typeof state.transactions === 'object', 'Checkpoint belongs to another publication or approval');
    assert(uint(state.spentCapWei) <= budget, 'Checkpoint exceeds approved fee');
    const recordedCost = Object.values(state.transactions).reduce((sum, rec) => sum + uint(rec.tx.gas) * uint(rec.tx.maxFee),0n);
    assert(recordedCost === uint(state.spentCapWei), 'Checkpoint fee accounting mismatch');
    const checkAbort = () => { if (signal?.aborted) throw Object.assign(new Error('Publication paused'),{code:'PUBLISH_PAUSED'}); };
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
    async function transaction(id, to, data, gas) {
      checkAbort();
      let rec = state.transactions[id];
      if (!rec) {
        await network();
        const cost = gas * f.maxFee, spent = uint(state.spentCapWei);
        assert(spent + cost <= budget, 'Publication would exceed the approved maximum fee');
        const balance = uint(await rpc('eth_getBalance',[from,'pending']));
        assert(balance >= cost, 'Insufficient balance for the next publication transaction');
        const nonce = uint(await rpc('eth_getTransactionCount',[from,'pending']));
        const tx = {chainId:chain.toString(),nonce:nonce.toString(),maxPriority:f.priority.toString(),maxFee:f.maxFee.toString(),gas:gas.toString(),to,value:'0',data};
        await network(); checkAbort();
        const signed = verifySigned(await sign(jsonClone(tx)),tx,from);
        rec = state.transactions[id] = {...signed,tx,receipt:null};
        state.spentCapWei = (spent + cost).toString();
        await save(); // Must commit the signed raw transaction BEFORE any broadcast.
      } else {
        assert(rec.tx.to === to && rec.tx.data === data && uint(rec.tx.gas) === gas && uint(rec.tx.chainId) === chain && uint(rec.tx.maxFee) === f.maxFee && uint(rec.tx.maxPriority) === f.priority && uint(rec.tx.value) === 0n, 'Checkpoint transaction does not match publication');
        verifySigned(rec,rec.tx,from);
      }
      await network();
      let receipt = await canonicalReceipt(rec.hash);
      if (!receipt) {
        checkAbort(); progress({phase:'broadcast',step:id,transactionHash:rec.hash});
        try { const h = await rpc('eth_sendRawTransaction',[rec.raw]); assert(typeof h === 'string' && h.toLowerCase() === rec.hash,'RPC returned a different transaction hash'); }
        catch (error) {
          receipt = await canonicalReceipt(rec.hash);
          if (!receipt) throw new PublishPendingError(rec.hash,'Submission could not be confirmed. Resume to check or retransmit the same signed transaction.');
        }
      }
      const deadline = Date.now() + (options.receiptTimeoutMs ?? 300000);
      while (!receipt) {
        checkAbort();
        if (Date.now() >= deadline) throw new PublishPendingError(rec.hash);
        await new Promise((resolve) => setTimeout(resolve,options.pollIntervalMs ?? 1000));
        receipt = await canonicalReceipt(rec.hash);
      }
      rec.receipt = receipt; await save();
      if (receipt.status === '0x0') throw new PublishRevertedError(rec.hash);
      progress({phase:'confirmed',step:id,transactionHash:rec.hash});
      return receipt;
    }
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
    await save();
    let completed = 0;
    for (const c of plan.chunks) {
      checkAbort();
      const code = await rpc('eth_getCode',[c.address,'latest']);
      if (code !== '0x') chunkPayload(fromHex(c.address),fromHex(code),c.data.length);
      // Another publisher may store this chunk while our signed transaction is
      // unresolved. Reconcile that exact transaction before using another nonce;
      // content availability alone does not settle the account's pending work.
      if (code === '0x' || state.transactions['chunk:'+c.address]) {
        await transaction('chunk:'+c.address,FACTORY,abiBytes('store(bytes)',c.data),chunkGas(c.data.length));
        chunkPayload(fromHex(c.address),fromHex(await rpc('eth_getCode',[c.address,'latest'])),c.data.length);
      }
      progress({phase:'upload',completed:++completed,total:plan.chunks.length});
    }
    if (!state.site) {
      const receipt = await transaction('deploy',null,WEBSITE_INITCODE,1500000n);
      const rec = state.transactions.deploy;
      const expected = toHex(keccak(rlp.encode([fromHex(from),rlp.encUint(BigInt(rec.tx.nonce))])).subarray(12));
      assert(address(receipt.contractAddress) === expected, 'Deployed contract address mismatch');
      state.site = expected; await save(); await authorize(state.site);
    }
    const target = state.site;
    if (state.versionCountBefore === null) {
      state.versionCountBefore = decodeWord(await call(target,selector('versionCount()')),0,BigInt(Number.MAX_SAFE_INTEGER)).toString(); await save();
    }
    if (state.version === null) {
      await transaction('create-version',target,abiCreateVersion(plan.manifestHash,plan.manifest.length,plan.manifestChunks),600000n);
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
    if (versionStatus === 0n || state.transactions['publish-version']) await transaction('publish-version',target,abiUint('publish(uint256)',version),200000n);
    await transaction('set-current',target,abiUint('setCurrent(uint256)',version),200000n);
    const current = decodeCurrent(fromHex(await call(target,selector('current()'))));
    assert(BigInt(current.version) === version && toHex(current.manifestHash) === plan.manifestHash && current.manifestLen === plan.manifest.length && current.chunks.length === plan.manifestChunks.length && current.chunks.every((c,i) => toHex(c) === plan.manifestChunks[i].address),'Published site verification failed');
    state.complete = true; await save();
    const result = {site:target,version:Number(version),manifestHash:plan.manifestHash,publishedBlock:current.publishedBlock,transactions:Object.values(state.transactions).map((tx) => tx.hash),maxFeeCommittedWei:state.spentCapWei};
    progress({phase:'complete',...result}); return result;
  } finally { running.delete(key); }
}
