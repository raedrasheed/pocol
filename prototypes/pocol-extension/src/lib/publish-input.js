// Sites W5: what may be published. Publishing is permanent, so nothing is dropped silently:
//  - an unsupported extension is refused loudly (prepareSite, "Unsupported file extension");
//  - the hard-deny list below is applied whatever the flags, and each refusal names its reason;
//  - only an explicit exclusion (a glob list, or an include list) leaves a file out, and every excluded path is listed.
// The same rules are implemented by PoCol Studio (dist/model.js), pocol-site (pocold/src/site.rs) and the S1 snapshot
// helper (deploy/websites/tools/site_snapshot.py); tests/data/publish-input-vectors.json is their shared vector file.
export const DENY_REASONS = {
  dotfile: 'a hidden file or directory (.htaccess, .git, .env …) is server or tool configuration',
  config: 'a config.* file can hold server credentials',
  'secret-key': 'a key or certificate file (*.key, *.pem) is secret material',
  database: 'a database dump (*.sql) is server data',
  backup: 'a backup file (*.bak*) can hold old server code or data',
  'server-api': 'the api/ directory is server-side code',
  checksums: 'a SHA256SUMS list names server-side files',
};
/** The hard-deny reason for a site path ('/a/b.txt'), or null. Case-insensitive. */
export function denyReason(path) {
  const segs = String(path).replace(/^\/+/, '').split('/');
  const base = segs[segs.length - 1].toLowerCase();
  if (segs.some((s) => s.startsWith('.'))) return 'dotfile';
  if (segs[0].toLowerCase() === 'api' && segs.length > 1) return 'server-api';
  if (base.startsWith('config.')) return 'config';
  if (/\.(key|pem)$/.test(base)) return 'secret-key';
  if (/\.sql$/.test(base)) return 'database';
  if (/\.bak[^/]*$/.test(base)) return 'backup';
  if (base.startsWith('sha256sums')) return 'checksums';
  return null;
}
/** A glob over site paths without the leading '/': `*` and `?` stay inside a segment, `**` spans segments, and a
 *  pattern ending in '/' matches everything below that directory. */
export function globRegex(pattern) {
  if (typeof pattern !== 'string' || !pattern.trim() || pattern.length > 256) throw new Error('Invalid exclusion pattern');
  let p = pattern.trim().replace(/^\/+/, ''); if (p.endsWith('/')) p += '**';
  let rx = '';
  for (let i = 0; i < p.length; i++) {
    const c = p[i];
    if (c === '*' && p[i + 1] === '*') { i++; if (p[i + 1] === '/') { i++; rx += '(?:.*/)?'; } else rx += '.*'; }
    else if (c === '*') rx += '[^/]*';
    else if (c === '?') rx += '[^/]';
    else rx += c.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  }
  return new RegExp('^' + rx + '$');
}
/** Parses an include list: one site path per line; blank lines and lines starting with '#' are ignored. */
export function parseIncludeList(text) {
  const out = [];
  for (const raw of String(text).split(/\r?\n/)) {
    const line = raw.trim(); if (!line || line.startsWith('#')) continue;
    const path = '/' + line.replace(/^\/+/, ''); if (out.includes(path)) throw new Error('Duplicate include-list path: ' + path);
    out.push(path);
  }
  if (!out.length) throw new Error('The include list is empty');
  return out;
}
/**
 * Applies the hard-deny list, then the explicit exclusions (`exclude` globs) or the include list (`include` paths), to
 * candidate site paths. Returns {kept, denied:[{path,reason}], excluded:[path], missing:[path]}: `missing` lists
 * include-list paths that are not candidates. The order of the input is kept.
 */
export function selectFiles(paths, {exclude = [], include = null} = {}) {
  const globs = exclude.map(globRegex), wanted = include ? new Set(include.map((p) => '/' + String(p).replace(/^\/+/, ''))) : null;
  const kept = [], denied = [], excluded = [];
  for (const path of paths) {
    const reason = denyReason(path);
    if (reason) { denied.push({path, reason}); continue; }
    const rel = path.replace(/^\/+/, '');
    if ((wanted && !wanted.has(path)) || globs.some((g) => g.test(rel))) { excluded.push(path); continue; }
    kept.push(path);
  }
  const present = new Set(paths);
  const missing = wanted ? [...wanted].filter((p) => !present.has(p)) : [];
  return {kept, denied, excluded, missing};
}
