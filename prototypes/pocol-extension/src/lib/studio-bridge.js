// Public site data only. The web page has no RPC, signing or wallet-secret method.
import { validPath, normalize, FILE_MAX, SITE_MAX, FILES_MAX } from './site.js';
export const JOB_PREFIX = 'studio:job:';
export const JOB_TTL_MS = 24 * 60 * 60 * 1000;
export function base64Bytes(text) {
  if (typeof text !== 'string' || text.length > Math.ceil(FILE_MAX / 3) * 4 || text.length % 4 || /[^A-Za-z0-9+/=]/.test(text)) throw new Error('Invalid or oversized file data');
  let raw; try { raw = atob(text); } catch { throw new Error('Invalid file encoding'); }
  if (btoa(raw) !== text) throw new Error('Noncanonical file encoding');
  const bytes = Uint8Array.from(raw, (c) => c.charCodeAt(0));
  if (bytes.length > FILE_MAX) throw new Error('A file exceeds 1 MiB');
  return bytes;
}
export function validatePublishRequest(input) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new Error('Invalid publish request');
  const { files, entry = '/index.html', site = null } = input;
  if (!Array.isArray(files) || files.length < 1 || files.length > FILES_MAX) throw new Error('Choose 1–256 files');
  if (!validPath(entry)) throw new Error('Invalid entry path');
  if (site !== null && !/^0x[0-9a-fA-F]{40}$/.test(site)) throw new Error('Invalid site address');
  let total = 0; const paths = new Set();
  const out = files.map((f) => {
    if (!f || !validPath(f.path) || paths.has(f.path)) throw new Error('Invalid or duplicate file path');
    paths.add(f.path); total += base64Bytes(f.base64).length;
    if (total > SITE_MAX) throw new Error('Site exceeds 4 MiB');
    return { path: f.path, base64: f.base64 };
  });
  if (!paths.has(entry)) throw new Error('Entry file not found');
  return { title: typeof input.title === 'string' ? input.title.slice(0, 160) : 'PoCol site', files: out, entry, site: site?.toLowerCase() || null, totalBytes: total };
}
export function validateSiteTarget(input) {
  if (!input || !/^0x[0-9a-fA-F]{40}$/.test(input.site)) throw new Error('Invalid site address');
  const path = input.path || '/'; if (normalize(path) === null) throw new Error('Invalid site path');
  return { site: input.site.toLowerCase(), path };
}
export function publicJob(job) {
  return { jobId: job.id, status: job.status, progress: job.progress || null, result: job.result || null, error: job.error || null };
}
