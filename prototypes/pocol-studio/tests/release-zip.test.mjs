// W14 reproducible release: scripts/build-release.py write_zip (the cPanel and gateway-source zips) depends only on the
// bytes and archive names of its inputs, never on where they live, the order they were made in, their mtimes or modes,
// or the time zone. The full release (build-release.py main) needs the packaged extension, which the network
// placeholder blocks until the identity switch, so only its zip writer is checked here.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, chmodSync, utimesSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT = join(dirname(fileURLToPath(import.meta.url)), '..', 'scripts', 'build-release.py');
const root = mkdtempSync(join(tmpdir(), 'pocol-studio-zip-'));
test.after(() => rmSync(root, { recursive: true, force: true }));
const sha = (b) => createHash('sha256').update(b).digest('hex');
// A word stream long enough that zlib's default level and level 9 give different DEFLATE output.
const WORDS = ['pocol', 'block', 'miner', 'share', 'reward', 'chain', 'genesis', 'studio', 'site', 'chunk'];
let seed = 7; const words = Array.from({ length: 12000 }, () => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return WORDS[seed % 10] + ((seed >> 8) % 7 ? ' ' : '\n'); }).join('');
// A small site tree: nested paths, a name that sorts differently by string and by path parts ('a-b' / 'a/b'), a UTF-8
// name and compressible and incompressible content.
const FILES = {
  'app.js': words,
  'index.html': '<!doctype html><title>PoCol</title>\n'.repeat(50),
  '.htaccess': 'Options -Indexes\n',
  'a-b.css': 'body{margin:0}\n',
  'a/b.js': 'export const x = 1;\n',
  'assets/img/logo.bin': Buffer.from(Array.from({ length: 4096 }, (_, i) => (i * 2654435761) >>> 24)),
  'downloads/مرحبا.txt': 'مرحبا PoCol\n',
};
/** Writes FILES under `dir` in the given order of names, with mtimes and modes from `stamp(i)`. */
function tree(dir, order, stamp) {
  order.forEach((name, i) => {
    const p = join(dir, name); mkdirSync(dirname(p), { recursive: true }); writeFileSync(p, FILES[name]);
    const { time, mode } = stamp(i); utimesSync(p, time, time); chmodSync(p, mode);
  });
  return dir;
}
/** build-release.py write_zip over files_under(dir), as main() packs dist/; returns the zip bytes. */
function zip(dir, out, env = process.env) {
  const r = spawnSync('python3', ['-B', '-c', `import importlib.util, sys
from pathlib import Path
s = importlib.util.spec_from_file_location('b', sys.argv[1]); m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
src = Path(sys.argv[2])
m.write_zip(Path(sys.argv[3]), [(p.relative_to(src).as_posix(), p) for p in m.files_under(src)])`, SCRIPT, dir, out], { encoding: 'utf8', env });
  assert.equal(r.status, 0, r.stderr);
  return readFileSync(out);
}
const names = Object.keys(FILES);

test('write_zip gives byte-identical zips from trees in other places, file orders, mtimes and modes', () => {
  const now = new Date();
  const a = tree(join(root, 'a', 'dist'), names, () => ({ time: now, mode: 0o644 }));
  const b = tree(join(root, 'other place', 'deeper', 'dist'), names.slice().reverse(),
    (i) => ({ time: new Date(Date.UTC(1999 + 7 * i, i, 1 + i, i, i, i)), mode: [0o600, 0o755, 0o444][i % 3] }));
  const za = zip(a, join(root, 'a.zip')), zb = zip(b, join(root, 'b.zip'), { ...process.env, TZ: 'Pacific/Kiritimati' });
  assert.ok(za.equals(zb), `the two zips differ: ${sha(za)} and ${sha(zb)}`);
});

test('write_zip entries are sorted by name with fixed date, mode and host system, DEFLATE level 9', () => {
  const dir = tree(join(root, 'layout'), names, () => ({ time: new Date(), mode: 0o600 }));
  zip(dir, join(root, 'layout.zip'));
  const read = spawnSync('python3', ['-B', '-c', `import json, struct, sys, zipfile, zlib
from pathlib import Path
src, out = Path(sys.argv[2]), []
with zipfile.ZipFile(sys.argv[1]) as z, open(sys.argv[1], 'rb') as f:
    for i in z.infolist():
        f.seek(i.header_offset); n, m = struct.unpack('<HH', f.read(30)[26:30]); f.seek(i.header_offset + 30 + n + m)
        data, c = (src / i.filename).read_bytes(), zlib.compressobj(9, zlib.DEFLATED, -15)
        out.append({'name': i.filename, 'date': list(i.date_time), 'mode': i.external_attr, 'system': i.create_system,
                    'method': i.compress_type, 'level9': f.read(i.compress_size) == c.compress(data) + c.flush(), 'same': z.read(i) == data})
print(json.dumps(out))`, join(root, 'layout.zip'), dir], { encoding: 'utf8' });
  assert.equal(read.status, 0, read.stderr);
  const entries = JSON.parse(read.stdout);
  // Python's sorted() of the archive names: code point order, so 'a-b.css' before 'a/b.js'.
  assert.deepEqual(entries.map((e) => e.name), ['.htaccess', 'a-b.css', 'a/b.js', 'app.js', 'assets/img/logo.bin', 'downloads/مرحبا.txt', 'index.html']);
  for (const e of entries) {
    assert.deepEqual({ ...e, name: undefined }, { name: undefined, date: [1980, 1, 1, 0, 0, 0], mode: 0o100644 * 65536, system: 3, method: 8, level9: true, same: true }, e.name);
  }
});
