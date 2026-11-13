// Embedded wallet (browser.md D14, signing eligibility): seed encrypted with
// PBKDF2-SHA256 (600k) + AES-GCM; per-network key = HKDF-SHA256(seed, salt=netKey,
// info='PoCol-wallet-v1'); auto-lock after 15 minutes. EIP-1559 signing only.
import { sign, addressOfSecret, keccak } from './crypto.js';
import * as rlp from './rlp.js';
import { toHex, fromHex, concat, eq } from './hex.js';
export const PBKDF2_ITER = 600000, LOCK_MS = 15 * 60 * 1000;
const enc = new TextEncoder();
const b64 = (b) => btoa(String.fromCharCode(...b)); const unb64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
async function aesKey(password, salt) {
  const base = await crypto.subtle.importKey('raw', enc.encode(password), 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey({ name: 'PBKDF2', salt, iterations: PBKDF2_ITER, hash: 'SHA-256' }, base, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
}
export async function encryptSeed(seed, password) {
  const salt = crypto.getRandomValues(new Uint8Array(16)), iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, await aesKey(password, salt), seed));
  return { salt: b64(salt), iv: b64(iv), ct: b64(ct), iter: PBKDF2_ITER };
}
export async function decryptSeed(box, password) {
  const pt = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: unb64(box.iv) }, await aesKey(password, unb64(box.salt)), unb64(box.ct));
  return new Uint8Array(pt);
}
export async function networkKey(seed, netKeyHex) {
  const k = await crypto.subtle.importKey('raw', seed, 'HKDF', false, ['deriveBits']);
  const bits = await crypto.subtle.deriveBits({ name: 'HKDF', hash: 'SHA-256', salt: fromHex(netKeyHex), info: enc.encode('PoCol-wallet-v1') }, k, 256);
  return new Uint8Array(bits);
}
export const addressOf = (secret) => toHex(addressOfSecret(secret));
/** Unsigned EIP-1559 payload 0x02 || RLP([chainId, nonce, tip, maxFee, gas, to, value, data, []]):
 *  the bytes the confirmation shows; keccak(payload) is the signing hash (A06). */
export function unsignedTx(tx) {
  const u = (v) => rlp.encUint(BigInt(v));
  const fields = [u(tx.chainId), u(tx.nonce), u(tx.maxPriority), u(tx.maxFee), u(tx.gas), tx.to ? fromHex(tx.to) : new Uint8Array(0), u(tx.value || 0), fromHex(tx.data || '0x'), []];
  const payload = concat(Uint8Array.of(2), rlp.encode(fields));
  return { fields, payload, signingHash: keccak(payload) };
}
/** Signs exactly an unsignedTx result: envelope 0x02 || RLP([...fields, y, r, s]). The signing
 *  hash is recomputed from the payload bytes, so the signature commits to what was shown. */
export function signUnsigned(secret, ut) {
  const hash = keccak(ut.payload);
  if (!eq(hash, ut.signingHash) || !eq(ut.payload, concat(Uint8Array.of(2), rlp.encode(ut.fields)))) throw new Error('payload changed after confirmation');
  const s = sign(hash, secret);
  const all = [...ut.fields, rlp.encUint(BigInt(s[64])), stripZeros(s.subarray(0, 32)), stripZeros(s.subarray(32, 64))];
  const raw = concat(Uint8Array.of(2), rlp.encode(all));
  return { raw: toHex(raw), hash: toHex(keccak(raw)) };
}
/** EIP-1559 envelope 0x02 || RLP([chainId, nonce, tip, maxFee, gas, to, value, data, [], y, r, s]). */
export const signTx = (secret, tx) => signUnsigned(secret, unsignedTx(tx));
const stripZeros = (b) => { let i = 0; while (i < b.length && b[i] === 0) i++; return b.subarray(i); };
