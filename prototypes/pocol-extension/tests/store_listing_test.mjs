// store/: the Chrome Web Store listing materials (2026-11-22). The parts the code decides are
// checked against the code: every permission, host permission and content-script origin of the release manifest has a
// justification (so a new permission fails until it is justified), the summary is the manifest description within the
// store's 132 characters, the images have the store's sizes, the screenshots' provenance is recorded, and the privacy
// policy the listing links is in the pocol.net snapshot. The materials name no chain ID: the network is the release's.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE), STORE = join(EXT, 'store');
const SITE = join(EXT, '..', '..', 'deploy', 'websites', 'pocol.net');
const listing = readFileSync(join(STORE, 'listing.md'), 'utf8'), readme = readFileSync(join(STORE, 'README.md'), 'utf8');
const out = mkdtempSync(join(tmpdir(), 'pocol-store-listing-'));
test.after(() => rmSync(out, { recursive: true, force: true }));
const { manifest } = build({ out: join(out, 'dist') });
/** Width and height from a PNG's IHDR. */
const png = (file) => { const b = readFileSync(file); assert.equal(b.subarray(1, 4).toString(), 'PNG', file); return [b.readUInt32BE(16), b.readUInt32BE(20)]; };
const section = (title) => { const at = listing.indexOf(title); assert.ok(at >= 0, title); return listing.slice(at); };

test('every permission, host permission and content-script origin of the release manifest is justified', () => {
  const justified = section('### Permission justifications');
  const headings = [...justified.matchAll(/^#### (.+)$/gm)].map((m) => m[1]);
  for (const permission of manifest.permissions) assert.ok(headings.includes('`' + permission + '`'), `no justification for ${permission}`);
  for (const host of manifest.host_permissions) assert.ok(headings.includes('Host permission `' + host + '`'), `no justification for ${host}`);
  const matches = manifest.content_scripts.flatMap((c) => c.matches);
  assert.ok(headings.includes('Content scripts on ' + matches.map((m) => '`' + m + '`').join(' and ')), `content scripts ${matches}`);
  assert.equal(headings.length, manifest.permissions.length + manifest.host_permissions.length + 1, 'a justification for a permission the manifest does not have');
  assert.equal(manifest.optional_permissions, undefined); assert.equal(manifest.optional_host_permissions, undefined);
});

test('the summary is the manifest description, within the store limit, and the name is the manifest name', () => {
  assert.ok(manifest.description.length <= 132, manifest.description.length);
  const summary = listing.match(/\*\*Summary\*\* \(the manifest `description`, at most 132 characters\): ([^]*?)\n- \*\*/)[1].replace(/\s+/g, ' ').trim();
  assert.equal(summary, manifest.description);
  assert.match(listing, new RegExp(`^- \\*\\*Name\\*\\* \\(from the manifest\\): ${manifest.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`, 'm'));
});

test('the listing answers the privacy practices tab and links the privacy policy in the pocol.net snapshot', () => {
  for (const title of ['### Single purpose', '### Remote code', '### Data usage', 'I do not sell or transfer user data']) section(title);
  assert.match(listing, /^- \*\*Privacy policy URL:\*\* https:\/\/pocol\.net\/privacy\.html$/m);
  assert.ok(existsSync(join(SITE, 'privacy.html')), 'deploy/websites/pocol.net/privacy.html');
  const rows = [...section('### Data usage').matchAll(/^\| ([A-Z][^|]+?) \| (Yes|No) \|/gm)].map((m) => m[1]);
  assert.deepEqual(rows, ['Personally identifiable information', 'Health information', 'Financial and payment information',
    'Authentication information', 'Personal communications', 'Location', 'Web history', 'User activity', 'Website content']);
  // The hosts the listing says the extension contacts are the manifest's.
  assert.match(listing, /connects only to the PoCol network gateway \(https:\/\/explorer\.pocol\.net\) or to a node on your own computer/);
  assert.deepEqual(manifest.host_permissions, ['http://127.0.0.1/*', 'https://explorer.pocol.net/*']);
});

test('the store images have the store sizes and the screenshots record the build they show', () => {
  assert.deepEqual(png(join(STORE, 'icon-128.png')), [128, 128]);
  assert.deepEqual(png(join(STORE, 'promo-small-440x280.png')), [440, 280]);
  const provenance = JSON.parse(readFileSync(join(STORE, 'screenshots', 'RENDERED-FROM.json'), 'utf8'));
  assert.ok(provenance.screenshots.length >= 1 && provenance.screenshots.length <= 5);
  for (const name of provenance.screenshots) assert.deepEqual(png(join(STORE, 'screenshots', name)), [1280, 800], name);
  assert.equal(typeof provenance.placeholderBuild, 'boolean');
  assert.equal(provenance.name, manifest.name);
  // A placeholder render is never presented as ready: the README says to render again before uploading.
  if (provenance.placeholderBuild) assert.match(readme, /must be rendered again \(step 4\) before upload/);
});

test('the store materials name no chain ID or genesis hash (the network is the release build\'s)', () => {
  for (const [name, text] of [['listing.md', listing], ['README.md', readme], ['RENDERED-FROM.json', readFileSync(join(STORE, 'screenshots', 'RENDERED-FROM.json'), 'utf8')]]) {
    assert.doesNotMatch(text, /(?<![0-9A-Za-z])[0-9]{7,10}(?![0-9A-Za-z])/, `${name} names a chain ID`);
    assert.doesNotMatch(text, /0x[0-9a-fA-F]{64}/, `${name} names a hash`);
  }
});
