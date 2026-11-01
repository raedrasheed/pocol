// Hex/bytes helpers (no dependencies).
export const toHex = (b) => '0x' + Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
export function fromHex(s) {
  if (typeof s !== 'string' || !/^0x([0-9a-fA-F]{2})*$/.test(s)) throw new Error('hex');
  const out = new Uint8Array((s.length - 2) / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(s.slice(2 + 2 * i, 4 + 2 * i), 16);
  return out;
}
export const concat = (...xs) => { const n = xs.reduce((a, x) => a + x.length, 0); const o = new Uint8Array(n); let i = 0; for (const x of xs) { o.set(x, i); i += x.length; } return o; };
export const eq = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);
export const be = (v, width) => { const o = new Uint8Array(width); let x = BigInt(v); for (let i = width - 1; i >= 0; i--) { o[i] = Number(x & 0xffn); x >>= 8n; } if (x) throw new Error('width'); return o; };
export const toBig = (b) => b.reduce((a, x) => (a << 8n) | BigInt(x), 0n);
export const utf8 = (s) => new TextEncoder().encode(s);
export const utf8len = (s) => new TextEncoder().encode(s).length;
