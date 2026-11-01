// Root-run helper for the 0.33 coordinator patch bundle. Node built-ins only: no network, no shell,
// no child process. It never reads or writes coordination/ui-control state, connection files,
// capabilities or the ledger; --ui-control is only checked for a running writer or service.
//
//   node bundle.mjs assemble --live <tool dir> --out <new dir>
//       Copies every live file byte for byte (node_modules/.git skipped), overlays the changed files,
//       and writes <out>/APPLY-MANIFEST.json with oldSha256/newSha256 per changed file.
//   node bundle.mjs verify   --live <tool dir> --manifest <out>/APPLY-MANIFEST.json
//   node bundle.mjs apply    --live <tool dir> --manifest <out>/APPLY-MANIFEST.json --backup <new dir> --ui-control <dir>
//   node bundle.mjs rollback --live <tool dir> --backup <dir> --ui-control <dir>
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BUNDLE = path.resolve(HERE, '..');
const FILES = path.join(BUNDLE, 'files');
const STATIC = JSON.parse(readFileSync(path.join(BUNDLE, 'apply-manifest.json'), 'utf8'));
const SKIP_DIRS = new Set(['node_modules', '.git']);

const sha = (f) => createHash('sha256').update(readFileSync(f)).digest('hex');
const shaOrNull = (f) => (existsSync(f) ? sha(f) : null);
function refuse(msg) { console.error('REFUSED: ' + msg); process.exit(2); }
function arg(name, required = true) {
  const i = process.argv.indexOf(name);
  const v = i > 0 ? process.argv[i + 1] : undefined;
  if (required && !v) refuse(`missing ${name}`);
  return v ? path.resolve(v) : null;
}
function safeRel(p) {
  if (typeof p !== 'string' || path.isAbsolute(p) || p.split('/').includes('..') || !/^[A-Za-z0-9._/-]+$/.test(p)) refuse(`unsafe path ${p}`);
  return p;
}
const abs = (root, p) => path.join(root, ...safeRel(p).split('/'));
function walk(dir, base = dir, out = []) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) { if (!SKIP_DIRS.has(e.name)) walk(path.join(dir, e.name), base, out); }
    else if (e.isFile()) out.push(path.relative(base, path.join(dir, e.name)).split(path.sep).join('/'));
  }
  return out.sort();
}
function copyExact(from, to) {
  mkdirSync(path.dirname(to), { recursive: true });
  copyFileSync(from, to);
  if (sha(from) !== sha(to)) refuse(`copy mismatch ${to}`);
}
function noWriter(uiDir) {
  if (!existsSync(uiDir)) refuse(`ui-control directory not found: ${uiDir}`);
  if (existsSync(path.join(uiDir, 'worker.lease'))) refuse('an author worker lease exists: wait until the author receipt has ended and been reviewed');
  if (existsSync(path.join(uiDir, 'service.lock'))) refuse('service.lock exists: stop the running coordinator service first (Ctrl+C); a stale lock must be handled manually');
}
function liveTool(live) {
  if (!existsSync(path.join(live, 'package.json')) || !existsSync(path.join(live, 'src', 'broker.mjs'))) refuse(`not the local-coordinator tool directory: ${live}`);
}

function assemble() {
  const live = arg('--live'), out = arg('--out');
  liveTool(live);
  if (existsSync(out)) refuse(`--out must be a new directory: ${out}`);
  const changed = new Map(STATIC.files.map((f) => [safeRel(f.path), f]));
  const liveFiles = walk(live);
  for (const p of STATIC.unchangedRequired) if (!liveFiles.includes(safeRel(p))) refuse(`required unchanged file missing in live tree: ${p}`);
  mkdirSync(out, { recursive: true });
  const unchanged = [];
  for (const p of liveFiles) {
    if (changed.has(p)) continue;
    copyExact(abs(live, p), abs(out, p));
    unchanged.push({ path: p, sha256: sha(abs(out, p)) });
  }
  const files = [];
  for (const [p, f] of changed) {
    const src = abs(FILES, p);
    if (!existsSync(src)) refuse(`bundle file missing: ${p}`);
    const old = shaOrNull(abs(live, p));
    if (f.change === 'added' && old !== null) refuse(`${p} is marked added but exists in the live tree`);
    if (f.change === 'modified' && old === null) refuse(`${p} is marked modified but is missing in the live tree`);
    copyExact(src, abs(out, p));
    files.push({ path: p, change: f.change, oldSha256: old, newSha256: sha(abs(out, p)) });
  }
  const manifest = { schema: 'pocol-coordinator-applied-manifest/0.33', live, assembled: out, bundle: BUNDLE, files, unchanged };
  writeFileSync(path.join(out, 'APPLY-MANIFEST.json'), JSON.stringify(manifest, null, 2) + '\n');
  console.log(`assembled ${files.length} changed + ${unchanged.length} unchanged files into ${out}`);
  console.log('run the tests there:');
  console.log(`  cd "${out}" && node --test test/broker.test.mjs test/history.test.mjs test/server.test.mjs test/worker.test.mjs test/notifier.test.mjs test/continuation.test.mjs test/connection.test.mjs`);
}

function loadManifest() {
  const m = JSON.parse(readFileSync(arg('--manifest'), 'utf8'));
  if (m.schema !== 'pocol-coordinator-applied-manifest/0.33') refuse('unknown manifest schema');
  return m;
}
function drift(live, m) {
  const bad = [];
  for (const f of m.files) if (shaOrNull(abs(live, f.path)) !== f.oldSha256) bad.push(f.path);
  for (const u of m.unchanged) if (shaOrNull(abs(live, u.path)) !== u.sha256) bad.push(u.path);
  return bad;
}

function verify() {
  const live = arg('--live'), m = loadManifest();
  const bad = drift(live, m);
  if (bad.length) { console.error('live tree changed since assembly: ' + bad.join(', ')); process.exit(1); }
  console.log('live tree matches the assembled manifest');
}

function apply() {
  const live = arg('--live'), m = loadManifest(), backup = arg('--backup');
  liveTool(live); noWriter(arg('--ui-control'));
  if (existsSync(backup)) refuse(`--backup must be a new directory: ${backup}`);
  const bad = drift(live, m);
  if (bad.length) refuse('live tree changed since assembly: ' + bad.join(', '));
  for (const f of m.files) if (sha(abs(m.assembled, f.path)) !== f.newSha256) refuse(`assembled file changed: ${f.path}`);
  mkdirSync(backup, { recursive: true });
  for (const f of m.files) if (f.oldSha256) copyExact(abs(live, f.path), abs(path.join(backup, 'files'), f.path));
  writeFileSync(path.join(backup, 'ROLLBACK.json'), JSON.stringify({ schema: 'pocol-coordinator-rollback/0.33', live, files: m.files }, null, 2) + '\n');
  for (const f of m.files) {
    copyExact(abs(m.assembled, f.path), abs(live, f.path));
    if (sha(abs(live, f.path)) !== f.newSha256) refuse(`applied file mismatch: ${f.path}`);
  }
  console.log(`applied ${m.files.length} files; old copies in ${backup}. Restart the service with the same command line (the saved local URL is kept when valid).`);
}

function rollback() {
  const live = arg('--live'), backup = arg('--backup');
  liveTool(live); noWriter(arg('--ui-control'));
  const r = JSON.parse(readFileSync(path.join(backup, 'ROLLBACK.json'), 'utf8'));
  for (const f of r.files) {
    if (f.oldSha256) {
      const saved = abs(path.join(backup, 'files'), f.path);
      if (sha(saved) !== f.oldSha256) refuse(`backup copy changed: ${f.path}`);
      copyExact(saved, abs(live, f.path));
    } else {
      const cur = shaOrNull(abs(live, f.path));
      if (cur === f.newSha256) unlinkSync(abs(live, f.path));
      else if (cur !== null) refuse(`added file ${f.path} was modified after apply; not removed`);
    }
  }
  console.log('rolled back; saved state under ui-control was never touched. Restart the service with the same command line.');
}

const cmd = process.argv[2];
({ assemble, verify, apply, rollback }[cmd] ?? (() => refuse('command must be assemble | verify | apply | rollback')))();
