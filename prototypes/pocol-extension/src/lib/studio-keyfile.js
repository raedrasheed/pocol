// Import only within trusted extension UI. No web page or RPC receives these bytes.
import { addressOfSecret } from './crypto.js';
import { toHex, concat } from './hex.js';
import { encryptSeed, decryptSeed } from './wallet.js';
const MAGIC = 'POCOL-DPAPI-USER-V1\0';
const encoder = new TextEncoder();
const b64 = (s, n, label) => {
  if (typeof s !== 'string' || s.length > 4096) throw new Error('Invalid ' + label);
  let raw; try { raw = atob(s); } catch { throw new Error('Invalid ' + label); }
  if (btoa(raw) !== s || (n !== null && raw.length !== n)) throw new Error('Invalid ' + label);
  return Uint8Array.from(raw, (c) => c.charCodeAt(0));
};
export function validSecret(bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.length !== 32) throw new Error('A secret must contain exactly 32 bytes');
  try { addressOfSecret(bytes); } catch { throw new Error('Invalid wallet secret'); }
  return bytes;
}
export async function importKeyfile(bytes, password) {
  if (!(bytes instanceof Uint8Array) || bytes.length > 16384) throw new Error('Invalid wallet file');
  if (new TextDecoder().decode(bytes.slice(0, MAGIC.length)) === MAGIC) throw new Error('Windows protects this file with DPAPI. Run Export-Studio-Wallet.ps1 on the same Windows account, then import its encrypted JSON file here.');
  if (bytes.length === 32) return validSecret(bytes.slice());
  let box; try { box = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)); } catch { throw new Error('Choose a PoCol encrypted wallet export'); }
  let secret;
  try {
    if (box.schema === 'pocol-studio-vault-v1') {
      if (box.iter !== 600000) throw new Error('Unsupported KDF');
      b64(box.salt, 16, 'salt'); b64(box.iv, 12, 'IV'); b64(box.ct, 48, 'ciphertext');
      secret = await decryptSeed(box, password);
    } else if (box.schema === 'pocol-studio-key-v1') {
      if (box.kdf !== 'PBKDF2-SHA256' || box.iterations !== 600000 || box.cipher !== 'AES-256-CBC-HMAC-SHA256') throw new Error('Unsupported export format');
      const salt = b64(box.salt, 16, 'salt'), iv = b64(box.iv, 16, 'IV'), ct = b64(box.ciphertext, 48, 'ciphertext'), mac = b64(box.mac, 32, 'MAC');
      const material = await crypto.subtle.importKey('raw', encoder.encode(password), 'PBKDF2', false, ['deriveBits']);
      const derived = new Uint8Array(await crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt, iterations: 600000 }, material, 512));
      try {
        const hmac = await crypto.subtle.importKey('raw', derived.slice(32), { name: 'HMAC', hash: 'SHA-256' }, false, ['verify']);
        const ok = await crypto.subtle.verify('HMAC', hmac, mac, concat(encoder.encode('PoCol-Studio-export-v1\0'), salt, iv, ct));
        if (!ok) throw new Error('Authentication failed');
        const aes = await crypto.subtle.importKey('raw', derived.slice(0, 32), 'AES-CBC', false, ['decrypt']);
        secret = new Uint8Array(await crypto.subtle.decrypt({ name: 'AES-CBC', iv }, aes, ct));
      } finally { derived.fill(0); }
    } else throw new Error('Unsupported export schema');
    validSecret(secret);
    const address = toHex(addressOfSecret(secret));
    if (box.address && box.address.toLowerCase() !== address) throw new Error('Address mismatch');
    return secret;
  } catch { if (secret) secret.fill(0); throw new Error('Could not unlock this wallet. Check the password and file integrity.'); }
}
export async function exportKeyfile(secret, password) {
  validSecret(secret);
  if (typeof password !== 'string' || password.length < 10) throw new Error('Use a password with at least 10 characters');
  return { schema: 'pocol-studio-vault-v1', address: toHex(addressOfSecret(secret)), ...(await encryptSeed(secret, password)) };
}
