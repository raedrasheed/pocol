// Synthetic, fully verifiable on-chain site for mocked RPC endpoints: manifest (T16),
// ChunkFactory CREATE2 chunk addresses and the Website current() ABI output.
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

export async function siteFixture(dist, files = { '/index.html': '<!doctype html><h1 id="t">fixture</h1>' }) {
  const L = async (m) => import(pathToFileURL(join(dist, 'lib', m)).href);
  const R = await L('rlp.js'), H = await L('hex.js'), C = await L('crypto.js'), S = await L('site.js');
  const enc = new TextEncoder(); const code = new Map(); const MIME = { html: 1, css: 2, js: 3, json: 4, png: 5, txt: 10 };
  const paths = Object.keys(files).sort();
  const entries = paths.map((p) => {
    const data = typeof files[p] === 'string' ? enc.encode(files[p]) : files[p];
    const chunks = []; for (let i = 0; i < data.length; i += S.CHUNK_MAX) { const c = data.subarray(i, i + S.CHUNK_MAX); const a = S.chunkAddress(c); code.set(H.toHex(a), H.concat(Uint8Array.of(0), c)); chunks.push([a, R.encUint(BigInt(c.length))]); }
    return [enc.encode(p), R.encUint(BigInt(MIME[p.split('.').pop()] || 10)), R.encUint(BigInt(data.length)), C.keccak(data), chunks];
  });
  const entry = paths.indexOf('/index.html');
  const manifest = R.encode([R.encUint(1n), R.encUint(BigInt(entry)), entries]);
  const ma = S.chunkAddress(manifest); code.set(H.toHex(ma), H.concat(Uint8Array.of(0), manifest));
  const word = (v) => H.be(BigInt(v), 32); const addrWord = (a) => H.concat(new Uint8Array(12), a);
  const current = H.toHex(H.concat(word(0), C.keccak(manifest), word(manifest.length), word(1), word(160), word(1), addrWord(ma)));
  const site = '0x' + '5a'.repeat(20);
  return {
    site, manifest, current,
    handlers: {
      eth_call: ([o]) => (o.to.toLowerCase() === site && o.data === S.SELECTORS.current ? current : '0x'),
      eth_getCode: ([a]) => { const c = code.get(a.toLowerCase()); return c ? H.toHex(c) : '0x'; },
    },
  };
}
