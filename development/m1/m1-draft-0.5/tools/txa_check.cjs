// Independent cross-check of the Python key/txA fixture (M1 draft 0.5, R5-08).
// Usage: node m1-draft-0.5/tools/txa_check.cjs   (after run_checks_05.py has written
// results/keys-txa-0.5.json). Writes results/txa-node-check.json.
// Independence: BIP-39/BIP-32 via node:crypto (PBKDF2, HMAC), public keys via
// OpenSSL ECDH secp256k1, keccak via @noble/hashes (coordination runtime copy),
// RLP and ECDSA verification re-implemented here with BigInt.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const pkg = path.resolve(__dirname, '..');
const outFile = path.join(pkg, 'results', 'txa-node-check.json');
const noblePath = path.resolve(pkg, '..', 'coordination', 'runtime', 'noble', 'node_modules', '@noble', 'hashes', 'sha3.js');
function done(o) { fs.mkdirSync(path.dirname(outFile), { recursive: true }); fs.writeFileSync(outFile, JSON.stringify(o, null, 2) + '\n'); process.stdout.write(JSON.stringify({ status: o.status, failures: (o.failures || []).length }) + '\n'); }
if (!fs.existsSync(noblePath)) { done({ status: 'notRun', reason: 'noble not found at ' + noblePath }); process.exit(0); }
const { keccak_256 } = require(noblePath);
const inFile = path.join(pkg, 'results', 'keys-txa-0.5.json');
if (!fs.existsSync(inFile)) { done({ status: 'notRun', reason: 'run run_checks_05.py first' }); process.exit(0); }
const fx = JSON.parse(fs.readFileSync(inFile, 'utf8'));

const P = 2n ** 256n - 2n ** 32n - 977n;
const N = BigInt('0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141');
const G = [BigInt('0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798'), BigInt('0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8')];
const mod = (a, m) => ((a % m) + m) % m;
function inv(a, m) { let [x0, x1, r0, r1] = [0n, 1n, m, mod(a, m)]; while (r1) { const q = r0 / r1; [x0, x1] = [x1, x0 - q * x1]; [r0, r1] = [r1, r0 - q * r1]; } return mod(x0, m); }
function add(p, q) {
  if (!p) return q; if (!q) return p;
  let l;
  if (p[0] === q[0]) { if (mod(p[1] + q[1], P) === 0n) return null; l = mod(3n * p[0] * p[0] * inv(2n * p[1], P), P); }
  else l = mod((q[1] - p[1]) * inv(q[0] - p[0], P), P);
  const x = mod(l * l - p[0] - q[0], P);
  return [x, mod(l * (p[0] - x) - p[1], P)];
}
function mul(k, pt) { let acc = null; while (k > 0n) { if (k & 1n) acc = add(acc, pt); pt = add(pt, pt); k >>= 1n; } return acc; }
const hex = (b) => Buffer.from(b).toString('hex');
const toBig = (b) => BigInt('0x' + (hex(b) || '0'));
const b32 = (n) => Buffer.from(n.toString(16).padStart(64, '0'), 'hex');
const kec = (b) => Buffer.from(keccak_256(b));

function pubFromPriv(k, format) { const e = crypto.createECDH('secp256k1'); e.setPrivateKey(b32(k)); return e.getPublicKey(null, format); }
function addr(k) { return '0x' + hex(kec(pubFromPriv(k, 'uncompressed').subarray(1)).subarray(12)); }

// BIP-39 / BIP-32
const seed = crypto.pbkdf2Sync(Buffer.from(fx.mnemonic, 'utf8'), Buffer.from('mnemonic', 'utf8'), 2048, 64, 'sha512');
function derive(pathArr) {
  let I = crypto.createHmac('sha512', 'Bitcoin seed').update(seed).digest();
  let k = toBig(I.subarray(0, 32)), c = I.subarray(32);
  for (const idx of pathArr) {
    const ib = Buffer.alloc(4); ib.writeUInt32BE(idx);
    const data = idx >= 0x80000000 ? Buffer.concat([Buffer.from([0]), b32(k), ib]) : Buffer.concat([pubFromPriv(k, 'compressed'), ib]);
    I = crypto.createHmac('sha512', c).update(data).digest();
    k = mod(toBig(I.subarray(0, 32)) + k, N); c = I.subarray(32);
  }
  return k;
}

// RLP (bytes and lists only)
function len(n, s, l) { if (n < 56) return Buffer.from([s + n]); let h = n.toString(16); if (h.length % 2) h = '0' + h; const lb = Buffer.from(h, 'hex'); return Buffer.concat([Buffer.from([l + lb.length]), lb]); }
function rlp(x) { if (Array.isArray(x)) { const b = Buffer.concat(x.map(rlp)); return Buffer.concat([len(b.length, 0xc0, 0xf7), b]); } if (x.length === 1 && x[0] < 0x80) return x; return Buffer.concat([len(x.length, 0x80, 0xb7), x]); }
function uint(n) { n = BigInt(n); if (n === 0n) return Buffer.alloc(0); let h = n.toString(16); if (h.length % 2) h = '0' + h; return Buffer.from(h, 'hex'); }

const failures = [];
const keys = {};
for (const k of fx.keys) {
  const d = derive([0x8000002c, 0x8000003c, 0x80000000, 0, k.index - 1]);
  const a = addr(d);
  keys[k.index] = { priv: '0x' + d.toString(16).padStart(64, '0'), address: a };
  if (keys[k.index].priv !== k.priv) failures.push('key ' + k.index + ' private key differs');
  if (a !== k.address) failures.push('key ' + k.index + ' address differs');
}
const t = fx.txA;
const fields = [uint(t.chainId), uint(t.nonce), uint(t.maxPriorityFeePerGas), uint(t.maxFeePerGas), uint(t.gas), Buffer.from(t.to.slice(2), 'hex'), uint(t.value), Buffer.alloc(0), []];
const sighash = kec(Buffer.concat([Buffer.from([2]), rlp(fields)]));
if ('0x' + hex(sighash) !== t.signingHash) failures.push('signing hash differs');
const r = BigInt(t.r), s = BigInt(t.s), z = toBig(sighash);
const signer = BigInt(keys[t.fromKey].priv);
const Q = mul(signer, G);
const w = inv(s, N);
const X = add(mul(mod(z * w, N), G), mul(mod(r * w, N), Q));
if (!X || mod(X[0], N) !== r) failures.push('ECDSA verification failed');
if (s > N / 2n) failures.push('high s');
const yPar = (() => { const y2 = mod(r ** 3n + 7n, P); let y = 1n, b = y2, e = (P + 1n) / 4n; while (e) { if (e & 1n) y = mod(y * b, P); b = mod(b * b, P); e >>= 1n; } if (mod(y, 2n) !== BigInt(t.yParity)) y = P - y; const Rp = [r, y]; const rinv = inv(r, N); return add(mul(mod(s * rinv, N), Rp), mul(mod(-z * rinv, N), G)); })();
if (!yPar || yPar[0] !== Q[0] || yPar[1] !== Q[1]) failures.push('public-key recovery with yParity does not give the signer');
const raw = Buffer.concat([Buffer.from([2]), rlp(fields.concat([uint(t.yParity), uint(r), uint(s)]))]);
if ('0x' + hex(raw) !== t.raw) failures.push('raw differs');
if ('0x' + hex(kec(raw)) !== t.txHash) failures.push('tx hash differs');
done({ status: failures.length ? 'FAIL' : 'pass', node: process.version, checks: ['bip32 keys', 'addresses', 'signing hash', 'ecdsa verify', 'yParity recovery', 'raw', 'txHash'], failures, keys });
