// scripts/package-store.py, the Chrome Web Store upload (2026-11-22). The store refuses a manifest
// with "key"; the release manifest has the owner's key (the side-loaded release's fixed id). The store zip is the
// release zip with manifest.json minus "key", after every check of scripts/package-release.py, reproducible; the
// owner's first upload may add the matching private key as key.pem (checked against release/manifest-key.pub), which
// fixes the store item's id to the release's.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, cpSync, readFileSync, writeFileSync, rmSync, existsSync, readdirSync, statSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, extensionId, MANIFEST_KEY_FILE } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE);
const RETIRED = join(EXT, '..', '..', 'deploy', 'mainnet', 'retired-networks.json');
const root = mkdtempSync(join(tmpdir(), 'pocol-store-'));
test.after(() => rmSync(root, { recursive: true, force: true }));
const rel = build({ out: join(root, 'release') }).dist;
const tree = join(root, 'tree');
mkdirSync(join(tree, 'scripts'), { recursive: true });
for (const f of ['package-release.py', 'package-store.py']) cpSync(join(EXT, 'scripts', f), join(tree, 'scripts', f));
writeFileSync(join(tree, 'package.json'), readFileSync(join(EXT, 'package.json')));
// A throwaway RSA manifest key (Chrome's key type) in the tree's slot, with its private key for the first upload.
const pair = (bits) => generateKeyPairSync('rsa', { modulusLength: bits });
const { privateKey, publicKey } = pair(2048), other = pair(2048);
const spki = (k) => k.export({ format: 'der', type: 'spki' }).toString('base64');
const TEST_KEY = spki(publicKey);
const pem = (k, name) => { const p = join(root, name); writeFileSync(p, k.export({ format: 'pem', type: 'pkcs8' }), { mode: 0o600 }); return p; };
const KEY_PEM = pem(privateKey, 'owner.pem'), OTHER_PEM = pem(other.privateKey, 'other.pem');
const SLOT = readFileSync(MANIFEST_KEY_FILE, 'utf8').replace(/^(?!#)\S.*$/m, TEST_KEY);
const files = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? files(join(d, f)) : [join(d, f)]));
const names = (d) => files(d).map((f) => relative(d, f).split(sep).join('/')).sort();
const edit = (file, f) => { const p = join(tree, 'dist', file); writeFileSync(p, f(readFileSync(p, 'utf8'))); };
const RETIRED_ID = JSON.parse(readFileSync(RETIRED, 'utf8')).retired[0];
const MARK = /\/\/ PLACEHOLDER \(decision D1\)[^]*?export const NETWORK_PLACEHOLDER = true;\n/;
/** A packager tree with dist/ as the release would be after the identity switch (the D1 mark dropped, a release
 * identity pinned and shown), with the throwaway key in the slot and the manifest; asBuilt keeps the network as built and
 * adds the D1 placeholder mark if the tree no longer carries it (after the identity switch), as tests/account_test.mjs's
 * distPh does, so the placeholder refusal is tested in both states of the tree. */
function prepare({ asBuilt = false } = {}) {
  rmSync(join(tree, 'dist'), { recursive: true, force: true }); cpSync(rel, join(tree, 'dist'), { recursive: true });
  mkdirSync(join(tree, 'release'), { recursive: true }); writeFileSync(join(tree, 'release', 'manifest-key.pub'), SLOT);
  edit('manifest.json', (t) => { const { key: _, ...m } = JSON.parse(t); return JSON.stringify({ key: TEST_KEY, ...m }, null, 2) + '\n'; });
  if (asBuilt) { edit('lib/studio-network.js', (t) => (t.includes('export const NETWORK_PLACEHOLDER = true;') ? t : t + '\nexport const NETWORK_PLACEHOLDER = true;\n')); return; }
  edit('lib/studio-network.js', (t) => t.replace(MARK, '').replace(/"chainId": \d+/, '"chainId": 4242').replace(/"genesisHash": "0x[0-9a-f]{64}"/, `"genesisHash": "0x${'42'.repeat(32)}"`));
  for (const f of files(join(tree, 'dist')).filter((f) => /\.(js|html|md|json|txt)$/.test(f))) {
    const t = readFileSync(f, 'utf8'), u = t.replaceAll(String(RETIRED_ID.chainId), '4242').replaceAll(RETIRED_ID.genesisHash.slice(2), '42'.repeat(32));
    if (u !== t) writeFileSync(f, u);
  }
}
const store = (args = []) => {
  const r = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-store.py'), '--retired', RETIRED, ...args], { encoding: 'utf8' });
  return r;
};
const sha = (b) => createHash('sha256').update(b).digest('hex');
/** {name: sha256} of a zip's members, and its manifest.json text, as Python's zipfile reads them. */
function unzip(path) {
  const r = spawnSync('python3', ['-B', '-c', `import hashlib,json,sys,zipfile
z=zipfile.ZipFile(sys.argv[1])
print(json.dumps({'members':{i.filename:hashlib.sha256(z.read(i)).hexdigest() for i in z.infolist()},'modes':{i.filename:oct(i.external_attr>>16) for i in z.infolist()},'manifest':z.read('manifest.json').decode()}))`, path], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  return JSON.parse(r.stdout);
}

test('the placeholder tree is refused for the store as it is for a release (network placeholder)', () => {
  prepare({ asBuilt: true });
  const out = join(root, 'placeholder.zip'), r = store([out]);
  assert.notEqual(r.status, 0); assert.match(r.stderr, /network placeholder/); assert.equal(existsSync(out), false);
});

test('the store zip is the release build with manifest.json without "key", reproducibly', () => {
  prepare();
  const a = join(root, 'a', 'store.zip'), b = join(root, 'b', 'store.zip');
  const ra = store([a]); assert.equal(ra.status, 0, ra.stderr);
  assert.match(ra.stdout, new RegExp(`^extension id ${extensionId(TEST_KEY)} \\(the side-loaded release's; the store item has it only with the key\\.pem method, otherwise note the id the dashboard assigns\\)$`, 'm'));
  const rb = store([b]); assert.equal(rb.status, 0, rb.stderr);
  assert.ok(readFileSync(a).equals(readFileSync(b)), 'two store zips of the same dist/ differ');
  assert.equal(readFileSync(a + '.sha256', 'utf8'), `${sha(readFileSync(a))}  store.zip\n`);
  const { members, manifest } = unzip(a), dist = join(tree, 'dist');
  assert.deepEqual(Object.keys(members).sort(), names(dist), 'the store zip lists exactly the dist/ files');
  for (const n of names(dist)) if (n !== 'manifest.json') assert.equal(members[n], sha(readFileSync(join(dist, n))), n);
  const released = JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8'));
  assert.equal(released.key, TEST_KEY);
  const { key: _, ...expected } = released;
  assert.equal(manifest, JSON.stringify(expected, null, 2) + '\n', 'manifest.json is the release manifest without "key", in build.mjs form');
  assert.equal('key' in JSON.parse(manifest), false);
  // And the release packager still packages the same dist/ with its key (side-loaded release unchanged).
  const r = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-release.py'), join(root, 'release.zip'), '--retired', RETIRED], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  assert.equal(JSON.parse(unzip(join(root, 'release.zip')).manifest).key, TEST_KEY);
});

test('a build that the release packager refuses is refused for the store too', () => {
  prepare(); edit('manifest.json', (t) => JSON.stringify({ ...JSON.parse(t), permissions: ['storage', 'unlimitedStorage', 'declarativeNetRequest', 'tabs'] }, null, 2) + '\n');
  let r = store([join(root, 'widened.zip')]); assert.notEqual(r.status, 0); assert.match(r.stderr, /permissions are/);
  prepare(); edit('manifest.json', (t) => JSON.stringify({ ...JSON.parse(t), key: spki(other.publicKey) }, null, 2) + '\n');
  r = store([join(root, 'otherkey.zip')]); assert.notEqual(r.status, 0); assert.match(r.stderr, /not the committed owner key/);
  assert.equal(existsSync(join(root, 'widened.zip')) || existsSync(join(root, 'otherkey.zip')), false);
});

test('the unofficial key.pem method is never the default: --first-upload-key alone is refused and writes nothing', () => {
  prepare();
  const out = join(root, 'unacknowledged.zip'), r = store(['--first-upload-key', KEY_PEM, out]);
  assert.notEqual(r.status, 0); assert.match(r.stderr, /Refusing --first-upload-key without --unofficial-key-pem/); assert.equal(existsSync(out), false);
  const plain = store([join(root, 'plain.zip')]); assert.equal(plain.status, 0, plain.stderr);
  assert.equal('key.pem' in unzip(join(root, 'plain.zip')).members, false, 'the default store zip holds no key');
});

test('the first upload adds the matching private key as key.pem, and only the matching one', () => {
  prepare();
  const wrong = join(root, 'wrong.zip');
  let r = store(['--first-upload-key', OTHER_PEM, '--unofficial-key-pem', wrong]);
  assert.notEqual(r.status, 0); assert.match(r.stderr, /not the private key of release\/manifest-key\.pub/); assert.equal(existsSync(wrong), false);
  const first = join(root, 'first.zip');
  r = store(['--first-upload-key', KEY_PEM, '--unofficial-key-pem', first]); assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /CONTAINS THE PRIVATE KEY \(key\.pem\)/);
  assert.equal(existsSync(first + '.sha256'), false, 'no .sha256 for a zip that holds a private key');
  const { members, modes, manifest } = unzip(first);
  assert.deepEqual(Object.keys(members).sort(), [...names(join(tree, 'dist')), 'key.pem'].sort());
  assert.equal(members['key.pem'], sha(readFileSync(KEY_PEM)));
  assert.equal(modes['key.pem'], '0o100600');
  assert.equal('key' in JSON.parse(manifest), false);
  // The key in key.pem is the slot's key: Chrome derives the item id from it, the same id as the release.
  assert.equal(spki(createPublicKey(createPrivateKey(readFileSync(KEY_PEM)))), TEST_KEY);
});
