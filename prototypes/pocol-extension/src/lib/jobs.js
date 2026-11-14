// Sites W4: Studio and IDE jobs carry the network they were created for. A job of another network (a retired chain, or
// one created before the identity changed) never blocks the wallet on this network: it is ignored by the unresolved-job
// checks, moved to the archive on install or update, listed read-only, and "Cancel" archives it without receipts.
export const STUDIO_JOB_PREFIX = 'studio:job:', IDE_JOB_PREFIX = 'ide:job:', ARCHIVE_PREFIX = 'archive:';
export const isJobKey = (key) => typeof key === 'string' && (key.startsWith(STUDIO_JOB_PREFIX) || key.startsWith(IDE_JOB_PREFIX));
export const networkOf = (network) => ({chainId: String(BigInt(network.chainId)), genesisHash: String(network.genesisHash).toLowerCase()});
const terminal = (job) => ['complete', 'cancelled'].includes(job?.status);
export const hasSignedTransactions = (job) => Object.keys(job?.checkpoint?.transactions || {}).length > 0;
/**
 * The network a job belongs to: its stored `network` ({chainId, genesisHash}); for a job stored before Sites W4, the
 * chain ids its signed transactions carry ({chainIds:[...], genesisHash:null}: their genesis is not recorded). Null when
 * nothing identifies it. A record without a readable chain id identifies nothing.
 */
export function jobNetwork(job) {
  if (job?.network && job.network.chainId !== undefined && typeof job.network.genesisHash === 'string') {
    try { const n = networkOf(job.network); return {chainIds: [n.chainId], genesisHash: n.genesisHash}; } catch { /* unreadable: fall through */ }
  }
  const ids = new Set();
  const add = (v) => { if (v === undefined || v === null) return; try { ids.add(String(BigInt(v))); } catch { /* unreadable */ } };
  for (const rec of Object.values(job?.checkpoint?.transactions || {})) add(rec?.tx?.chainId);
  add(job?.checkpoint?.review?.tx?.chainId);
  return ids.size ? {chainIds: [...ids].sort(), genesisHash: null} : null;
}
/** True only when the job is proven to belong to another network than `network`: its stored identity differs, or
 *  every chain id its signed transactions carry differs. An unidentified job is treated as this network's. */
export function isForeignJob(job, network) {
  const own = jobNetwork(job); if (!own) return false;
  const current = networkOf(network);
  if (own.genesisHash !== null) return own.chainIds[0] !== current.chainId || own.genesisHash !== current.genesisHash;
  return own.chainIds.every((id) => id !== current.chainId);
}
/** An unfinished job with signed transactions on THIS network, for `address` (lowercase compare). */
export function blocksAccount(job, address, network) {
  return !!job && !terminal(job) && hasSignedTransactions(job) && String(job.walletAddress || '').toLowerCase() === String(address).toLowerCase() && !isForeignJob(job, network);
}
// A Studio job's side keys (Sites W4 checkpoint size): its files, written once when the publisher page first opens the
// job, and one key per signed raw transaction, so a progress or checkpoint write no longer rewrites megabytes.
export const STUDIO_FILES_PREFIX = 'studio:files:', STUDIO_RAW_PREFIX = 'studio:raw:';
export const filesKey = (id) => STUDIO_FILES_PREFIX + id;
export const rawKey = (id, hash) => STUDIO_RAW_PREFIX + id + ':' + hash;
/** Removes a Studio job's side keys (its files and raw transactions). */
export async function removeSideKeys(storage, key, {files = true, raws = true} = {}) {
  if (!key.startsWith(STUDIO_JOB_PREFIX)) return;
  const id = key.slice(STUDIO_JOB_PREFIX.length), names = typeof storage.getKeys === 'function' ? await storage.getKeys() : Object.keys(await storage.get(null));
  const gone = names.filter((k) => (files && k === filesKey(id)) || (raws && k.startsWith(STUDIO_RAW_PREFIX + id + ':')));
  if (gone.length) await storage.remove(gone);
}
/** Moves `key`'s job to the archive (read-only) and drops its side keys. `storage` is chrome.storage.local or a
 *  compatible store. */
export async function archiveJob(storage, key, job, reason, now = Date.now()) {
  await storage.set({[ARCHIVE_PREFIX + key]: {...job, archivedAt: now, archivedReason: reason, archivedFrom: key}});
  await storage.remove(key);
  await removeSideKeys(storage, key);
}
/** Archives every job of another network. Returns the archived keys. */
export async function archiveForeignJobs(storage, network, now = Date.now()) {
  const all = await storage.get(null), moved = [];
  for (const [key, job] of Object.entries(all)) {
    if (!isJobKey(key) || !isForeignJob(job, network)) continue;
    await archiveJob(storage, key, job, 'foreign-network', now); moved.push(key);
  }
  return moved;
}
/** The archived jobs, newest first, for a read-only list. */
export async function archivedJobs(storage) {
  const all = await storage.get(null);
  return Object.entries(all).filter(([key]) => key.startsWith(ARCHIVE_PREFIX) && isJobKey(key.slice(ARCHIVE_PREFIX.length))).map(([key, job]) => ({key, job})).sort((a, b) => (b.job.archivedAt || 0) - (a.job.archivedAt || 0));
}
