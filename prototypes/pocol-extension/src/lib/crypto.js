// Hashes and secp256k1 recovery via vendored @noble (MIT).
import { keccak_256 } from '../vendor/hashes/sha3.js';
import { sha256 as nsha256 } from '../vendor/hashes/sha2.js';
import { hmac } from '../vendor/hashes/hmac.js';
import * as secp from '../vendor/secp256k1/index.js';
import { concat, toBig } from './hex.js';
secp.etc.hmacSha256Sync = (k, ...m) => hmac(nsha256, k, secp.etc.concatBytes(...m));
export const keccak = (b) => keccak_256(b);
export const sha256 = (b) => nsha256(b);
const HALF_N = secp.CURVE.n >> 1n;
export function addressOfPub(pub65) { return keccak(pub65.subarray(1)).subarray(12); }
/** 65-byte r||s||v (v in {0,1}), low-s, over a 32-byte hash. Throws on any fault. */
export function recover(msg32, sig) {
  if (sig.length !== 65 || sig[64] > 1) throw new Error('sig shape');
  const r = toBig(sig.subarray(0, 32)), s = toBig(sig.subarray(32, 64));
  if (r === 0n || r >= secp.CURVE.n || s === 0n || s > HALF_N) throw new Error('sig range');
  const pub = new secp.Signature(r, s, sig[64]).recoverPublicKey(msg32).toRawBytes(false);
  return addressOfPub(pub);
}
export function sign(msg32, secret32) {
  const s = secp.sign(msg32, secret32, { lowS: true });
  return concat(secp.etc.numberToBytesBE(s.r, 32), secp.etc.numberToBytesBE(s.s, 32), Uint8Array.of(s.recovery));
}
export function addressOfSecret(secret32) { return addressOfPub(secp.getPublicKey(secret32, false)); }
