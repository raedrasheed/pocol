// The saved PoCol wallet (`studio:wallet`) seen from the Studio publisher and IDE contract pages (final review,
// 2026-11-22). Those pages decrypt the key themselves and once wrote the vault directly, so a Create, or an unlock with a
// wallet file, silently replaced a saved wallet of another address with no backup: the earlier key could then never be
// unlocked again in this browser. Now a page writes the vault only when none is saved. A different saved wallet is
// replaced only in the extension's settings (Settings, "PoCol wallet"), where the service worker asks the user to back
// it up first (sw.js saveAccount, replace:true). A file of another wallet is used in its tab only, in memory.
//
// The toolbar's Lock locks every page too: the worker writes LOCK_SIGNAL_KEY on account.lock, and each page that holds
// a decrypted key watches it (watchLockSignal) and locks at once.
export const VAULT_KEY = 'studio:wallet';
export const LOCK_SIGNAL_KEY = 'wallet:lockSignal';
/** The minimum wallet password length, as lib/account.js PASSWORD_MIN (a test checks they are equal). */
export const PASSWORD_MIN = 10;

const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
/** The lower-case address a saved vault names, or null. */
export const vaultAddressOf = (vault) => (vault && typeof vault.address === 'string' && ADDRESS.test(vault.address) ? vault.address.toLowerCase() : null);
const same = (a, b) => typeof a === 'string' && typeof b === 'string' && a.toLowerCase() === b.toLowerCase();

/** The vault saved in `storage` (chrome.storage.local), or null. */
export async function savedVault(storage) {
  return (await storage.get(VAULT_KEY))[VAULT_KEY] || null;
}

/** Saves `makeVault()` (the encrypted vault of `address`) only when no wallet is saved. Returns
 *  {state: 'saved'} when it was written, {state: 'same'} when that address is already saved (nothing written), and
 *  {state: 'other', saved} when another wallet is saved: then nothing is written and the key stays in the caller's
 *  memory only. The saved vault is read again right before the write. */
export async function saveVaultIfNone(storage, address, makeVault) {
  const before = await savedVault(storage);
  if (before) return same(vaultAddressOf(before), address) ? { state: 'same' } : { state: 'other', saved: vaultAddressOf(before) };
  const vault = await makeVault();
  const now = await savedVault(storage);
  if (now) return same(vaultAddressOf(now), address) ? { state: 'same' } : { state: 'other', saved: vaultAddressOf(now) };
  await storage.set({ [VAULT_KEY]: vault });
  return { state: 'saved' };
}

/** The refusal of a Create while a wallet is saved: a page never replaces it. */
export function createRefusal(saved) {
  return `A PoCol wallet is already saved in this browser${saved ? ' (' + saved + ')' : ''}. Unlock it with its password, ` +
    'or replace it in the extension settings (Settings, "PoCol wallet"), which asks you to back it up first. Nothing was changed.';
}
/** The note shown when a file of another wallet was unlocked in a page. */
export function otherWalletNote(address, saved) {
  return `This file's wallet (${address}) is unlocked in this tab only. The PoCol wallet saved in this browser` +
    `${saved ? ' (' + saved + ')' : ''} was not changed; to replace it, use Settings, "PoCol wallet".`;
}
/** A Create's password checks: at least PASSWORD_MIN characters, typed twice the same. */
export function checkNewPassword(password, confirmation) {
  if (typeof password !== 'string' || password.length < PASSWORD_MIN) throw new Error(`Use a password of at least ${PASSWORD_MIN} characters`);
  if (password !== confirmation) throw new Error('The two passwords differ. Type the same password twice.');
}

/** Calls `onLock` whenever the worker signals a lock (the toolbar's or the settings' Lock). Returns false when the
 *  storage events are unavailable. */
export function watchLockSignal(chromeApi, onLock) {
  const events = chromeApi?.storage?.onChanged;
  if (!events?.addListener) return false;
  events.addListener((changes, area) => { if (area === 'local' && changes && Object.hasOwn(changes, LOCK_SIGNAL_KEY)) onLock(); });
  return true;
}
