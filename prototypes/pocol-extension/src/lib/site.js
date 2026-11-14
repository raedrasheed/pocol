// LP8/LP9 site verification (storage.md): manifest T16 rules, D27 chunk checks, D96 normalization.
import * as rlp from './rlp.js';
import { keccak } from './crypto.js';
import { concat, eq, toHex, fromHex } from './hex.js';
import { withRetry } from './retry.js';
export const CHUNK_MAX = 24575, FILE_MAX = 1 << 20, SITE_MAX = 4 << 20, FILES_MAX = 256, MANIFEST_MAX = 65536;
export const MIME = new Map([[1, 'text/html'], [2, 'text/css'], [3, 'application/javascript'], [4, 'application/json'], [5, 'image/png'], [6, 'image/jpeg'], [7, 'image/webp'], [8, 'image/svg+xml'], [9, 'font/woff2'], [10, 'text/plain']]);
const FACTORY = fromHex('0x0000000000000000000000000000000000c0c005');
const SEG = /^[A-Za-z0-9._~-]+$/;
export function validPath(p) {
  if (typeof p !== 'string' || p.length > 256 || p.length < 2 || p[0] !== '/') return false;
  return p.slice(1).split('/').every((s) => s.length > 0 && s !== '.' && s !== '..' && SEG.test(s));
}
/** null → 404; '' → entry; otherwise the normalized path. */
export function normalize(raw) {
  const p = String(raw).split(/[?#]/)[0];
  if (p === '' || p === '/') return '';
  if (p.includes('%') || p.includes('//') || p.split('/').some((s) => s === '.' || s === '..')) return null;
  const q = p.endsWith('/') ? p + 'index.html' : p;
  return validPath(q) ? q : null;
}
export function chunkInit(data) { const l = data.length + 1; return concat(Uint8Array.of(0x61, l >> 8, l & 255, 0x80, 0x60, 0x0a, 0x5f, 0x39, 0x5f, 0xf3, 0x00), data); }
export function chunkAddress(data) { return keccak(concat(Uint8Array.of(0xff), FACTORY, keccak(data), keccak(chunkInit(data)))).subarray(12); }
export function chunkPayload(addr, code, len) {
  if (code[0] !== 0) throw new Error('code[0] != 0x00');
  const data = code.subarray(1);
  if (data.length !== len) throw new Error('chunk length mismatch');
  if (!eq(chunkAddress(data), addr)) throw new Error('chunk not produced by the ChunkFactory');
  return data;
}
const dec = new TextDecoder('utf-8', { fatal: true });
export function decodeManifest(b) {
  if (b.length > MANIFEST_MAX) throw new Error('manifest > 64 KiB');
  const top = rlp.decode(b);
  if (!rlp.isList(top) || top.length !== 3) throw new Error('manifest shape');
  if (rlp.uint(top[0], 1, 'version') !== 1n) throw new Error('manifest version');
  const entry = Number(rlp.uint(top[1], 4, 'entry'));
  if (!rlp.isList(top[2])) throw new Error('files');
  const files = []; let total = 0;
  for (const f of top[2]) {
    if (!rlp.isList(f) || f.length !== 5) throw new Error('file arity');
    const pb = rlp.bytes(f[0], 'path'); if (!pb.every((c) => c < 128)) throw new Error('bad path (non-ASCII)');
    const path = dec.decode(pb); if (!validPath(path)) throw new Error('bad path ' + JSON.stringify(path));
    const mime = Number(rlp.uint(f[1], 1, 'mime')); if (!MIME.has(mime)) throw new Error('unknown mime ' + mime);
    const size = Number(rlp.uint(f[2], 4, 'size'));
    const hash = rlp.bytes(f[3], 'hash'); if (hash.length !== 32) throw new Error('hash len');
    if (!rlp.isList(f[4])) throw new Error('chunks'); // an empty file has size 0 and no chunks (pos-empty-file)
    let sum = 0; const chunks = [];
    for (const c of f[4]) { if (!rlp.isList(c) || c.length !== 2) throw new Error('chunk arity'); const a = rlp.bytes(c[0]); if (a.length !== 20) throw new Error('chunk addr'); const l = Number(rlp.uint(c[1], 4, 'len')); if (l < 1 || l > CHUNK_MAX) throw new Error('chunk len'); sum += l; chunks.push([a, l]); }
    if (sum !== size || size > FILE_MAX) throw new Error('size mismatch for ' + path);
    total += size; files.push({ path, mime, size, hash, chunks });
  }
  if (files.length === 0 || files.length > FILES_MAX || total > SITE_MAX) throw new Error('file count/site size');
  for (let i = 1; i < files.length; i++) if (cmpBytes(new TextEncoder().encode(files[i - 1].path), new TextEncoder().encode(files[i].path)) >= 0) throw new Error('paths not strictly sorted (or duplicate)');
  if (entry >= files.length || files[entry].mime !== 1) throw new Error('entry is not html');
  return { entry, files };
}
function cmpBytes(a, b) { for (let i = 0; i < Math.min(a.length, b.length); i++) if (a[i] !== b[i]) return a[i] - b[i]; return a.length - b.length; }
const SEL = (sig) => toHex(keccak(new TextEncoder().encode(sig)).subarray(0, 4));
export const SELECTORS = { current: SEL('current()') };
export function decodeCurrent(out) {
  if (out.length < 32 * 7) throw new Error('current() output');
  const w = (i) => BigInt(toHex(out.subarray(i * 32, i * 32 + 32)));
  const off = Number(w(4)); const n = Number(BigInt(toHex(out.subarray(off, off + 32))));
  if (n < 1 || n > 3) throw new Error('manifest chunks');
  const chunks = []; for (let i = 0; i < n; i++) chunks.push(out.subarray(off + 32 + 32 * i + 12, off + 64 + 32 * i));
  return { version: Number(w(0)), manifestHash: out.subarray(32, 64), manifestLen: Number(w(2)), publishedBlock: Number(w(3)), chunks };
}
/** Chunk-level fetch width (W2): at most CHUNK_WIDTH_MAX eth_getCode reads in flight, which CONTENT_POOL
 *  (8 x 68 KiB, transport.js) admits without queueing; 2 for an RN profile, whose gateway slots are shared by
 *  every viewer, 8 for LN/DEV/RP (a node the user runs or chose). */
export const CHUNK_WIDTH_MAX = 8, CHUNK_WIDTH_RN = 2;
export const chunkWidth = (trustLevel) => (trustLevel === 'RN' ? CHUNK_WIDTH_RN : CHUNK_WIDTH_MAX);
/** Run `tasks` (async thunks) with at most `width` in flight; the first failure stops new starts and is thrown. */
async function pooled(tasks, width) {
  let next = 0, failure = null;
  const worker = async () => { while (failure === null && next < tasks.length) { const t = tasks[next++]; try { await t(); } catch (e) { if (failure === null) failure = e; } } };
  await Promise.all(Array.from({ length: Math.max(1, Math.min(width, tasks.length)) }, worker));
  if (failure !== null) throw failure;
}
/** Fetch + fully verify the current version through `rpc(method, params)`.
 *  limits.parallel: chunk reads in flight (1..CHUNK_WIDTH_MAX, default CHUNK_WIDTH_MAX); limits.retry: options for
 *  lib/retry.js withRetry (attempts, deadlineMs, attemptMs, shorten, clock, sleep, random). Each read is retried on
 *  429, -32021, 502/503/504, a timeout or a dropped connection, never on another HTTP status; every chunk is verified
 *  (factory address, length) and every file by keccak, so a retry can never let tampered content through. */
export async function fetchSite(rpc, site, limits = {}) {
  const width = Math.max(1, Math.min(CHUNK_WIDTH_MAX, Math.floor(limits.parallel ?? CHUNK_WIDTH_MAX)));
  const read = withRetry(rpc, { retryBusy: true, ...(limits.retry || {}) });
  const cur = decodeCurrent(fromHex(await read('eth_call', [{ to: site, data: SELECTORS.current }, 'latest'])));
  const code = async (a) => {
    try { return fromHex(await read('eth_getCode', [toHex(a), 'latest'])); }
    catch (e) { throw Object.assign(new Error('chunk ' + toHex(a) + ' could not be read: ' + e.message), e.transport ? { transport: e.transport } : {}, e.rpc ? { rpc: e.rpc } : {}); }
  };
  const mcodes = new Array(cur.chunks.length);
  await pooled(cur.chunks.map((a, i) => async () => { mcodes[i] = await code(a); }), width);
  let mb = new Uint8Array(0); let left = cur.manifestLen;
  cur.chunks.forEach((a, i) => { const c = mcodes[i]; const l = Math.min(c.length - 1, left); const d = chunkPayload(a, c, l); left -= d.length; mb = concat(mb, d); });
  if (mb.length !== cur.manifestLen || !eq(keccak(mb), cur.manifestHash)) throw new Error('manifest hash/length mismatch');
  const m = decodeManifest(mb);
  // Every distinct (address, length) is read once, whichever files share it.
  const payloads = new Map(); const tasks = [];
  for (const f of m.files) for (const [a, l] of f.chunks) {
    const k = toHex(a) + ':' + l; if (payloads.has(k)) continue; payloads.set(k, null);
    tasks.push(async () => { payloads.set(k, chunkPayload(a, await code(a), l)); });
  }
  await pooled(tasks, width);
  const files = new Map();
  for (const f of m.files) {
    const data = concat(...f.chunks.map(([a, l]) => payloads.get(toHex(a) + ':' + l)));
    if (!eq(keccak(data), f.hash)) throw new Error('content hash mismatch for ' + f.path);
    files.set(f.path, { mime: MIME.get(f.mime), data });
  }
  return { version: cur.version, publishedBlock: cur.publishedBlock, entry: m.files[m.entry].path, files };
}
