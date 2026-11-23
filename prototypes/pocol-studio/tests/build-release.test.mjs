// Studio release builder (scripts/build-release.py main) before the signed release (owner policy 2026-11-22: no
// extension package link or listed digest until then). It runs on a copy of the served dist/ with a stub extension
// packager and a stub gateway installer builder, so neither the real extension build nor the network is needed:
// - the current dist/app.js passes the install-dialog check (it links pocol.net's release verification, no package);
// - the Chrome ZIP stays a release artifact for signing: it is not copied into dist/downloads nor into the cPanel ZIP;
// - a dist/app.js that links an extension package, or a dist/ that holds one, is refused.
import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const STUDIO = join(dirname(fileURLToPath(import.meta.url)), '..');
const SCRIPT = join(STUDIO, 'scripts', 'build-release.py');
const RELEASE_PAGE = 'https://pocol.net/chrome-extension.html#verify-release';
const root = mkdtempSync(join(tmpdir(), 'pocol-studio-build-'));
test.after(() => rmSync(root, { recursive: true, force: true }));

function write(path, text) { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, text); }

/** A release input set in `dir`: a copy of Studio (dist/, package.json, RELEASE-NOTES.md), a stub extension and gateway. */
function inputs(dir, version) {
  const studio = join(dir, 'studio'), extension = join(dir, 'extension'), gateway = join(dir, 'gateway');
  cpSync(join(STUDIO, 'dist'), join(studio, 'dist'), { recursive: true });
  for (const name of ['package.json', 'RELEASE-NOTES.md']) cpSync(join(STUDIO, name), join(studio, name));
  write(join(extension, 'package.json'), JSON.stringify({ version }));
  write(join(extension, 'dist', 'manifest.json'), JSON.stringify({ manifest_version: 3, name: 'PoCol Studio & Viewer', version }));
  write(join(extension, 'scripts', 'package-release.py'),
    'import sys, zipfile\nwith zipfile.ZipFile(sys.argv[1], "w") as z:\n    z.writestr("manifest.json", "{}")\n');
  write(join(gateway, 'build_installer.py'),
    'from pathlib import Path\n(Path(__file__).parent / "Install-PoCol-Studio-Gateway.sh").write_text("#!/bin/sh\\n")\n');
  for (const name of ['README-AR.txt', 'pocol_studio_gateway.py', 'install_studio_gateway.py', 'test_studio_gateway.py']) {
    write(join(gateway, name), `# ${name}\n`);
  }
  return { studio, extension, gateway, output: join(dir, 'out') };
}

function build({ studio, extension, gateway, output }) {
  return spawnSync('python3', ['-B', '-I', SCRIPT, '--studio-dir', studio, '--extension-dir', extension,
    '--gateway-dir', gateway, '--output-dir', output], { encoding: 'utf8', timeout: 120000 });
}

function zipNames(path) {
  const r = spawnSync('python3', ['-B', '-I', '-c', 'import json, sys, zipfile; print(json.dumps(zipfile.ZipFile(sys.argv[1]).namelist()))', path],
    { encoding: 'utf8', timeout: 60000 });
  assert.equal(r.status, 0, r.stderr);
  return JSON.parse(r.stdout);
}

const version = JSON.parse(readFileSync(join(STUDIO, '..', 'pocol-extension', 'package.json'), 'utf8')).version;
const zipName = `PoCol-Studio-IDE-Viewer-${version}.zip`;

test('the served Studio links the release verification page and no extension package', () => {
  const app = readFileSync(join(STUDIO, 'dist', 'app.js'), 'utf8');
  assert.ok(app.includes(RELEASE_PAGE), 'dist/app.js links ' + RELEASE_PAGE);
  assert.deepEqual(app.match(/PoCol-Studio-(?:IDE-Viewer-[^"'<>\s/]+?|Chrome)\.zip/g), null);
});

test('the builder keeps the Chrome ZIP out of dist/downloads and out of the cPanel ZIP', () => {
  const set = inputs(join(root, 'ok'), version);
  const r = build(set);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(existsSync(join(set.studio, 'dist', 'downloads')), false, 'the builder created dist/downloads');
  const cpanel = zipNames(join(set.output, 'PoCol-Studio-cPanel.zip'));
  assert.ok(cpanel.includes('index.html') && cpanel.includes('app.js'), cpanel.join(' '));
  assert.deepEqual(cpanel.filter((n) => n.startsWith('downloads/') || /\.zip$/.test(n)), []);
  // The Chrome ZIP is still a release artifact, for the owner to sign.
  const manifest = JSON.parse(readFileSync(join(set.output, 'release-manifest.json'), 'utf8'));
  assert.ok(existsSync(join(set.output, zipName)));
  assert.ok(manifest.artifacts[zipName], Object.keys(manifest.artifacts).join(' '));
  assert.deepEqual(Object.keys(manifest.sourceSha256).filter((n) => n.startsWith('studio/downloads/')), []);
});

test('the builder refuses a dist/app.js that links an extension package', () => {
  const set = inputs(join(root, 'link'), version);
  const app = join(set.studio, 'dist', 'app.js');
  const text = readFileSync(app, 'utf8');
  assert.ok(text.includes(RELEASE_PAGE));
  writeFileSync(app, text.replaceAll(RELEASE_PAGE, `downloads/${zipName}`));
  const r = build(set);
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, new RegExp(`names the extension package\\(s\\) \\['${zipName.replaceAll('.', '\\.')}'\\]`));
  assert.equal(existsSync(set.output), false);
});

test('the builder refuses a dist/ that holds an extension package', () => {
  const set = inputs(join(root, 'stale'), version);
  write(join(set.studio, 'dist', 'downloads', zipName), 'stale');
  const r = build(set);
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /dist\/ holds the extension package\(s\)/);
  assert.equal(existsSync(set.output), false);
});
