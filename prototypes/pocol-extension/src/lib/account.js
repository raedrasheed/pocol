// The PoCol wallet (2026-11-22, EXT-02/04/05): one account, the `studio:wallet` vault that Studio and IDE already use
// (lib/studio-keyfile.js, schema pocol-studio-vault-v1: the 32-byte secp256k1 key under PBKDF2-SHA256 600000 +
// AES-256-GCM). The service worker holds it decrypted only while unlocked; these helpers are pure and never store or send
// anything. The key is used directly (no derivation), so the address is the same as in Studio, IDE and the native wallet.
import { HDNodeWallet, Mnemonic, getAddress } from '../vendor/ethers.js';
import { fromHex } from './hex.js';
import { validSecret } from './studio-keyfile.js';

/** The BIP-44 path of a recovery-phrase import: the first account of coin type 60, as standard Ethereum wallets use. */
export const PHRASE_PATH = "m/44'/60'/0'/0/0";
export const PHRASE_WORDS = Object.freeze([12, 15, 18, 21, 24]);
export const PASSWORD_MIN = 10;
export const WEI_PER_PCOL = 10n ** 18n;

/** A private key typed by the user: 64 hex digits, with or without 0x, and a valid secp256k1 secret. */
export function parsePrivateKey(text) {
  if (typeof text !== 'string') throw new Error('Enter a private key');
  const t = text.trim();
  if (!/^(0x)?[0-9a-fA-F]{64}$/.test(t)) throw new Error('A private key is 64 hexadecimal digits (32 bytes)');
  return validSecret(fromHex('0x' + t.replace(/^0x/, '').toLowerCase()));
}

/** A BIP-39 English recovery phrase (12-24 words, checksum checked) to the key at PHRASE_PATH, without a BIP-39
 *  passphrase. Only the derived key is kept; the phrase itself is never stored. */
export function phraseToSecret(text) {
  if (typeof text !== 'string') throw new Error('Enter the recovery phrase');
  const phrase = text.normalize('NFKD').trim().toLowerCase().split(/\s+/).join(' ');
  if (!PHRASE_WORDS.includes(phrase.split(' ').length)) throw new Error('A recovery phrase has 12, 15, 18, 21 or 24 words');
  if (!Mnemonic.isValidMnemonic(phrase)) throw new Error('This is not a valid English BIP-39 recovery phrase (a word or the checksum is wrong)');
  const node = HDNodeWallet.fromPhrase(phrase, '', PHRASE_PATH);
  return validSecret(fromHex(node.privateKey));
}

/** A recipient address: 0x and 40 hex digits; a mixed-case address must carry a valid EIP-55 checksum. The zero address
 *  is refused (nothing sent there can be spent). Returns the lowercase address. */
export function parseRecipient(text) {
  if (typeof text !== 'string' || !/^0x[0-9a-fA-F]{40}$/.test(text.trim())) throw new Error('Enter a recipient address: 0x and 40 hexadecimal digits');
  const a = text.trim(), body = a.slice(2);
  if (body !== body.toLowerCase() && body !== body.toUpperCase()) {
    let ok = false; try { ok = getAddress(a) === a; } catch { ok = false; }
    if (!ok) throw new Error('The recipient address has a wrong checksum (check every character)');
  }
  if (/^0x0{40}$/.test(a)) throw new Error('The zero address cannot receive a transfer');
  return a.toLowerCase();
}

/** A PCOL amount typed by the user (decimal, at most 18 decimals) to wei; must be positive. */
export function parsePcol(text) {
  if (typeof text !== 'string') throw new Error('Enter an amount');
  const t = text.trim();
  if (!/^(0|[1-9][0-9]{0,26})(\.[0-9]{1,18})?$/.test(t)) throw new Error('Enter the amount in PCOL, for example 1.5 (at most 18 decimals)');
  const [whole, frac = ''] = t.split('.');
  const wei = BigInt(whole) * WEI_PER_PCOL + BigInt(frac.padEnd(18, '0'));
  if (wei <= 0n) throw new Error('The amount must be more than 0 PCOL');
  return wei;
}

/** wei to an exact PCOL decimal string (no rounding, trailing zeros removed). */
export function formatPcol(wei) {
  const n = BigInt(wei), neg = n < 0n, a = neg ? -n : n;
  const frac = (a % WEI_PER_PCOL).toString().padStart(18, '0').replace(/0+$/, '');
  return (neg ? '-' : '') + (a / WEI_PER_PCOL).toString() + (frac ? '.' + frac : '') + ' PCOL';
}

/** The fee caps of a transfer: maxFee = 2 * next base fee + tip, as common wallets do, refused above the publisher's
 *  absolute ceiling. */
export function transferFees(baseNext, tip, ceiling) {
  const base = BigInt(baseNext), t = BigInt(tip), maxFee = base * 2n + t;
  if (maxFee > BigInt(ceiling)) throw new Error('The network fee is above the ceiling of this wallet; try again later');
  return { maxPriority: t, maxFee };
}

/** Whether a JSON-RPC error answer to eth_sendRawTransaction proves that the transaction was refused (not pooled, not
 *  forwarded), so it can never be included. Only those answers make a transfer "rejected"; every other error is
 *  ambiguous and the transfer stays "unknown" and is followed by its receipt. The PoCol gateway answers a broadcast it
 *  could not complete, which may already have reached the node, with -32000 "... check transaction hash before
 *  resubmitting" (deploy/studio/pocol_studio_gateway.py, call), and the node answers a transaction it parks for the
 *  next block with -32016 "... held for the next block" (pocold node.rs POOL_PARKED): neither is a refusal.
 *  Refusals: -32021 busy (the gateway before forwarding, or the node's "not pooled"), -32014 oversized, -32020 the
 *  node's admission deadline (nothing pooled), -32016 pool full without "held", the standard request errors
 *  -32600/-32601/-32602, and -32000 answers that name an admission check (nonce, funds, fee, caps, signature). */
const SEND_REFUSED_CODES = new Set([-32021, -32014, -32020, -32600, -32601, -32602]);
const SEND_REFUSED_32000 = /nonce too (low|high)|NonceToo(Low|High)|insufficient funds|LackOfFund|underpriced|below the next base fee|GasPriceLessThanBasefee|would exceed|intrinsic gas|IntrinsicGas|CallGasCostMoreThanGasLimit|InvalidChainId|invalid (chain id|signature|sender)/i;
const SEND_AMBIGUOUS = /unavailable|resubmit|check transaction hash|time(d)? ?out|may have/i;
export function sendRefused(error) {
  const code = error?.code, message = String(error?.message ?? '');
  if (!Number.isInteger(code) || SEND_AMBIGUOUS.test(message)) return false;
  if (SEND_REFUSED_CODES.has(code)) return true;
  if (code === -32016) return !/held/i.test(message);
  if (code === -32000) return SEND_REFUSED_32000.test(message);
  return false;
}

/** The explorer lookup of an address, for the pinned public network only. The explorer shows the balance, the nonce
 *  and the activity of recent blocks, not a complete history; callers say so. */
export const EXPLORER_ORIGIN = 'https://explorer.pocol.net';
export const explorerAddressUrl = (address) => `${EXPLORER_ORIGIN}/#q=${encodeURIComponent(address.toLowerCase())}`;
