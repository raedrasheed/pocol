// Strict canonical RLP (consensus.md §1): shortest prefixes, no trailing bytes.
export function decode(b, maxDepth = 16) {
  const [item, end] = dec(b, 0, 0, maxDepth);
  if (end !== b.length) throw new Error('rlp trailing');
  return item;
}
function len(b, p, n) { if (n === 0 || p + n > b.length || b[p] === 0) throw new Error('rlp len'); let v = 0; for (let i = 0; i < n; i++) v = v * 256 + b[p + i]; if (v < 56) throw new Error('rlp nonminimal'); return v; }
function dec(b, p, depth, max) {
  if (p >= b.length) throw new Error('rlp truncated');
  const t = b[p];
  if (t < 0x80) return [b.subarray(p, p + 1), p + 1];
  if (t <= 0xb7) { const n = t - 0x80; if (p + 1 + n > b.length) throw new Error('rlp truncated'); if (n === 1 && b[p + 1] < 0x80) throw new Error('rlp single byte'); return [b.subarray(p + 1, p + 1 + n), p + 1 + n]; }
  if (t <= 0xbf) { const k = t - 0xb7; const n = len(b, p + 1, k); const s = p + 1 + k; if (s + n > b.length) throw new Error('rlp truncated'); return [b.subarray(s, s + n), s + n]; }
  if (depth >= max) throw new Error('rlp depth');
  let s, e;
  if (t <= 0xf7) { s = p + 1; e = s + (t - 0xc0); } else { const k = t - 0xf7; const n = len(b, p + 1, k); s = p + 1 + k; e = s + n; }
  if (e > b.length) throw new Error('rlp truncated');
  const items = []; let q = s;
  while (q < e) { const [it, nq] = dec(b, q, depth + 1, max); items.push(it); q = nq; }
  if (q !== e) throw new Error('rlp list');
  return [items, e];
}
export const isList = (x) => Array.isArray(x);
export function bytes(x, what = 'bytes') { if (Array.isArray(x)) throw new Error(what); return x; }
export function uint(x, width = 32, what = 'int') { const v = bytes(x, what); if (v.length > width || (v.length && v[0] === 0)) throw new Error(what + ' noncanonical'); let n = 0n; for (const c of v) n = (n << 8n) | BigInt(c); return n; }
export function encode(x) {
  if (Array.isArray(x)) { const p = concatAll(x.map(encode)); return concatAll([prefix(0xc0, p.length), p]); }
  if (x.length === 1 && x[0] < 0x80) return x;
  return concatAll([prefix(0x80, x.length), x]);
}
function prefix(base, n) { if (n < 56) return Uint8Array.of(base + n); const l = []; let v = n; while (v) { l.unshift(v & 255); v = Math.floor(v / 256); } return Uint8Array.of(base + 55 + l.length, ...l); }
function concatAll(xs) { const n = xs.reduce((a, x) => a + x.length, 0); const o = new Uint8Array(n); let i = 0; for (const x of xs) { o.set(x, i); i += x.length; } return o; }
export const encUint = (v) => { let x = BigInt(v); if (x < 0n) throw new Error('negative integer'); const o = []; while (x) { o.unshift(Number(x & 0xffn)); x >>= 8n; } return Uint8Array.from(o); };
