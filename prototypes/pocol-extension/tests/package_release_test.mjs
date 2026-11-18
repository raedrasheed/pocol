// Sites W3: scripts/package-release.py refuses every build that is not the release, independently of the tests on
// dist/: fault and rehearsal names, widened permissions or content scripts, a manifest key the release does not have,
// another sandbox page or background, an open or stale sandbox CSP, another endpoint or trust level, a chatgpt.site
// origin, the D1 placeholder (Sites W9: its mark or its identity), a retired identity (in the built-in profile or shown
// by any file), or a missing or other manifest key (W14: release/manifest-key.pub, the owner key of the fixed extension
// id). The tree as it is carries both placeholders until the identity switch, so it is never packaged. The release zip
// is reproducible (W14): it depends only on the bytes and relative paths of dist/.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, cpSync, readFileSync, writeFileSync, rmSync, existsSync, readdirSync, statSync, chmodSync, utimesSync, symlinkSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createHash, createPrivateKey, createPublicKey } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, SANDBOX_CSP, readManifestKey, extensionId, MANIFEST_KEY_FILE } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE);
const RETIRED = join(EXT, '..', '..', 'deploy', 'mainnet', 'retired-networks.json');
const root = mkdtempSync(join(tmpdir(), 'pocol-package-'));
const rel = build({ out: join(root, 'release') }).dist, faultCsp = build({ fault: 'cspOpen', out: join(root, 'fault') }).dist;
test.after(() => rmSync(root, { recursive: true, force: true }));
/** A packager tree: a copy of scripts/package-release.py and package.json, with dist/ and the key slot filled by pack(). */
const packagerTree = (dir) => {
  mkdirSync(join(dir, 'scripts'), { recursive: true });
  cpSync(join(EXT, 'scripts', 'package-release.py'), join(dir, 'scripts', 'package-release.py'));
  writeFileSync(join(dir, 'package.json'), readFileSync(join(EXT, 'package.json')));
  return dir;
};
const tree = packagerTree(join(root, 'tree'));
// A throwaway manifest key (an Ed25519 SubjectPublicKeyInfo from a fixed seed) in the tree's key slot; the real slot is
// the owner's (release/manifest-key.pub, committed 2026-11-18). Its extension id is pinned below.
const spkiOf = (seedByte) => createPublicKey(createPrivateKey({ key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), Buffer.alloc(32, seedByte)]), format: 'der', type: 'pkcs8' }))
  .export({ format: 'der', type: 'spki' }).toString('base64');
const TEST_KEY = spkiOf(7), OTHER_KEY = spkiOf(8);
const PLACEHOLDER_SLOT = readFileSync(MANIFEST_KEY_FILE, 'utf8').replace(/^(?!#)\S.*$/m, 'PLACEHOLDER: the owner\'s manifest public key (base64 DER SubjectPublicKeyInfo)');
const slotWith = (line) => PLACEHOLDER_SLOT.replace(/^PLACEHOLDER.*$/m, line);
const setSlot = (text, at = tree) => { mkdirSync(join(at, 'release'), { recursive: true }); writeFileSync(join(at, 'release', 'manifest-key.pub'), text); };
setSlot(slotWith(TEST_KEY));
const edit = (file, f, at = tree) => { const p = join(at, 'dist', file); writeFileSync(p, f(readFileSync(p, 'utf8'))); };
const RETIRED_ID = JSON.parse(readFileSync(RETIRED, 'utf8')).retired[0];
// The identity switch (D1) removes the NETWORK_PLACEHOLDER mark and pins the network in place of the placeholder.
const MARK = /\/\/ PLACEHOLDER \(decision D1\)[^]*?export const NETWORK_PLACEHOLDER = true;\n/;
const dropMark = (t) => t.replace(MARK, '');
const releaseIdentity = (t) => dropMark(t).replace(/"chainId": \d+/, '"chainId": 4242').replace(/"genesisHash": "0x[0-9a-f]{64}"/, `"genesisHash": "0x${'42'.repeat(32)}"`);
// As deploy/mainnet/set_network_identity.py does, every page and README that shows the network is repinned too.
const shownIdentity = (t) => t.replaceAll(String(RETIRED_ID.chainId), '4242').replaceAll(RETIRED_ID.genesisHash.slice(2), '42'.repeat(32));
const TEXT = /\.(js|html|md|json|txt)$/;
const files = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? files(join(d, f)) : [join(d, f)]));
/** Packages a copy of `from` after `edits`, with a release identity unless `asBuilt` keeps the tree's own; `at` is the
 * packager tree, `prepare(dist)` runs on its dist/ last and `env` is the packager's environment. */
function pack({ from = rel, edits = [], asBuilt = false, args = [], out = 'out.zip', key = TEST_KEY, slot = slotWith(TEST_KEY), at = tree, prepare = null, env = process.env } = {}) {
  rmSync(join(at, 'dist'), { recursive: true, force: true }); rmSync(join(at, out), { force: true }); rmSync(join(at, out + '.sha256'), { force: true });
  cpSync(from, join(at, 'dist'), { recursive: true });
  setSlot(slot, at);
  // As build.mjs writes it for a release once the slot holds the key (null: no key in the manifest).
  edit('manifest.json', (t) => { const { key: _, ...m } = JSON.parse(t); return JSON.stringify(key === null ? m : { key, ...m }, null, 2) + '\n'; }, at);
  if (!asBuilt) {
    edit('lib/studio-network.js', releaseIdentity, at);
    for (const f of files(join(at, 'dist')).filter((f) => TEXT.test(f))) { const t = readFileSync(f, 'utf8'), u = shownIdentity(t); if (u !== t) writeFileSync(f, u); }
  }
  for (const [file, f] of edits) edit(file, f, at);
  if (prepare) prepare(join(at, 'dist'));
  const r = spawnSync('python3', ['-B', join(at, 'scripts', 'package-release.py'), join(at, out), '--retired', RETIRED, ...args], { encoding: 'utf8', env });
  return { ...r, zip: existsSync(join(at, out)) };
}
const refused = (r, rx) => { assert.notEqual(r.status, 0, r.stdout); assert.equal(r.zip, false); assert.match(r.stderr, rx); };
const manifest = (f) => ['manifest.json', (t) => JSON.stringify(f(JSON.parse(t)), null, 2) + '\n'];

test('the release build with a release identity is packaged, with the committed manifest key and its extension id', () => {
  const r = pack(); assert.equal(r.status, 0, r.stderr); assert.equal(r.zip, true);
  assert.match(r.stdout, new RegExp(`extension id ${extensionId(TEST_KEY)}$`, 'm'));
  // Owner decision 2026-11-16: the packager never signs; the release signature comes from deploy/release.
  assert.match(r.stdout, /^UNSIGNED: sign with deploy\/release \(ssh-keygen -Y sign -n pocol-release\) before publishing$/m);
  const listed = spawnSync('python3', ['-B', '-c', 'import sys,zipfile,json;print(json.loads(zipfile.ZipFile(sys.argv[1]).read("manifest.json"))["key"])', join(tree, 'out.zip')], { encoding: 'utf8' });
  assert.equal(listed.stdout.trim(), TEST_KEY);
});
// W14 (Q7): the release has the fixed extension id of the owner's manifest key.
const py = (expr, ...argv) => spawnSync('python3', ['-B', '-c', `import importlib.util,sys;s=importlib.util.spec_from_file_location('p',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(${expr},end='')`, join(EXT, 'scripts', 'package-release.py'), ...argv], { encoding: 'utf8' });
test('the extension id of a manifest key is Chrome\'s (sha256 of the DER, first 32 hex digits as a-p), the same in build.mjs and the packager', () => {
  assert.equal(TEST_KEY, 'MCowBQYDK2VwAyEA6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw=');
  assert.equal(extensionId(TEST_KEY), 'dcelocnokilmeeegblacddofbpkeijac');
  for (const k of [TEST_KEY, OTHER_KEY]) { assert.equal(py('m.extension_id(sys.argv[2])', k).stdout, extensionId(k)); assert.match(extensionId(k), /^[a-p]{32}$/); }
  assert.notEqual(extensionId(TEST_KEY), extensionId(OTHER_KEY));
  setSlot(slotWith(TEST_KEY)); const printed = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-release.py'), '--extension-id'], { encoding: 'utf8' });
  assert.equal(printed.status, 0, printed.stderr); assert.equal(printed.stdout.trim(), extensionId(TEST_KEY));
  setSlot(PLACEHOLDER_SLOT); const none = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-release.py'), '--extension-id'], { encoding: 'utf8' });
  assert.notEqual(none.status, 0); assert.match(none.stderr, /owner public key is not committed/);
});
// The owner's release manifest key (release/manifest-key.pub, committed 2026-11-18) and the extension id published on
// pocol.net.
const OWNER_MANIFEST_KEY = 'MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAqWQrqZhBbzcU1D+5dr+xs5kTZg4C84b6mGILnubAGjeU/JxC+Fg/F9fwE9b4e+ZTXgltm5AgIvhpwvzROtoQ1a6gpcwDLRf+TrhF6oMZwFhoMptBNLnJFsv/gg2LC56qmB7H0CWHi/2eQcabDrqSGBD0lxLKx65qES+1+YT/jT1SXoaDKWtSGMYadTgWZ7N3rXtmaMDJpJwGp4IcTt/eoiNeJKtZvDJSJY3SaWNJaAb/vBUeRvA2rH8BcoPAnrubjEPPHqdryMtKwDFxYlJnT8l25PyspO9nKB2mM8ROprSQjQmCd7U1liTq6KR99LBxDMuQnx9qHjOSGV3qsFLbtQIDAQAB';
const OWNER_EXTENSION_ID = 'gggcoflmedlhlkcdhebmoepmggdnafmp';
test('the committed slot is the owner key: the release build carries it, with the published extension id', () => {
  assert.equal(readManifestKey(), OWNER_MANIFEST_KEY);
  assert.doesNotMatch(readFileSync(MANIFEST_KEY_FILE, 'utf8'), /^PLACEHOLDER/m);
  assert.equal(extensionId(OWNER_MANIFEST_KEY), OWNER_EXTENSION_ID);
  assert.equal(JSON.parse(readFileSync(join(rel, 'manifest.json'), 'utf8')).key, OWNER_MANIFEST_KEY);
  const real = py('m.manifest_key(m.Path(sys.argv[2]))', EXT);
  assert.equal(real.status, 0, real.stderr); assert.equal(real.stdout, OWNER_MANIFEST_KEY);
  const printed = spawnSync('python3', ['-B', join(EXT, 'scripts', 'package-release.py'), '--extension-id'], { encoding: 'utf8' });
  assert.equal(printed.status, 0, printed.stderr); assert.equal(printed.stdout.trim(), OWNER_EXTENSION_ID);
});
test('a release without the committed owner key, or with another key, is refused', () => {
  refused(pack({ slot: PLACEHOLDER_SLOT }), /no manifest key: the owner public key is not committed \(release\/manifest-key\.pub is the PLACEHOLDER/);
  refused(pack({ slot: PLACEHOLDER_SLOT, key: null }), /no manifest key/);
  refused(pack({ key: null }), /manifest key is not the committed owner key/);
  refused(pack({ key: OTHER_KEY }), /manifest key is not the committed owner key/);
  rmSync(join(tree, 'release'), { recursive: true, force: true });
  const r = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-release.py'), join(tree, 'x.zip'), '--retired', RETIRED], { encoding: 'utf8' });
  assert.match(r.stderr, /manifest-key\.pub is missing/); assert.equal(existsSync(join(tree, 'x.zip')), false);
  for (const [line, rx] of [['MIIB', /not the canonical base64 of a DER SubjectPublicKeyInfo/], [TEST_KEY.replace(/=$/, ''), /not the canonical base64/],
                            [Buffer.from('not a key at all, but base64').toString('base64'), /not the canonical base64 of a DER SubjectPublicKeyInfo/],
                            [Buffer.concat([Buffer.from(TEST_KEY, 'base64'), Buffer.of(0)]).toString('base64'), /SubjectPublicKeyInfo/]]) {
    refused(pack({ slot: slotWith(line), key: line }), rx);
  }
  refused(pack({ slot: slotWith(TEST_KEY) + OTHER_KEY + '\n' }), /exactly one key line/);
});
test('the placeholder tree is refused as the network placeholder first, whatever the key slot holds', () => {
  for (const [slot, key] of [[PLACEHOLDER_SLOT, null], [slotWith(TEST_KEY), TEST_KEY], [slotWith(TEST_KEY), null]]) refused(pack({ asBuilt: true, slot, key }), /network placeholder \(NETWORK_PLACEHOLDER\)/);
});
test('build.mjs puts the slot key into the release manifest only: never into a fault or testGateway build; a malformed slot fails the build', () => {
  const slot = join(root, 'slot.pub'); writeFileSync(slot, slotWith(TEST_KEY));
  assert.equal(build({ out: join(root, 'keyed'), manifestKeyFile: slot }).manifest.key, TEST_KEY);
  for (const fault of ['noDnr', 'cspOpen']) assert.equal(build({ fault, out: join(root, 'keyed-' + fault), manifestKeyFile: slot }).manifest.key, undefined, fault);
  const tg = build({ fault: 'testGateway', out: join(root, 'keyed-tg'), manifestKeyFile: slot, testNetwork: { name: 'devnet-gw', chainId: 4242, genesisHash: '0x' + '42'.repeat(32), genesisPre: '0x01', endpoint: 'http://127.0.0.1:8650/studio/rpc' } });
  assert.equal(tg.manifest.key, undefined); assert.equal(JSON.parse(readFileSync(join(tg.dist, 'manifest.json'), 'utf8')).key, undefined);
  writeFileSync(slot, PLACEHOLDER_SLOT); assert.equal(build({ out: join(root, 'unkeyed'), manifestKeyFile: slot }).manifest.key, undefined);
  for (const bad of [slotWith('MIIB'), slotWith(Buffer.from('not a key').toString('base64')), slotWith(TEST_KEY) + OTHER_KEY + '\n']) {
    writeFileSync(slot, bad); assert.throws(() => build({ out: join(root, 'bad'), manifestKeyFile: slot }), /manifest-key|slot\.pub/);
  }
  assert.throws(() => build({ out: join(root, 'bad'), manifestKeyFile: join(root, 'missing.pub') }), /missing: it is the manifest key slot/);
  for (const d of ['keyed', 'keyed-noDnr', 'keyed-cspOpen', 'keyed-tg', 'unkeyed', 'bad']) rmSync(join(root, d), { recursive: true, force: true });
});
test('the packager\'s sandbox CSP is build.mjs SANDBOX_CSP(hash, false)', () => {
  const page = readFileSync(join(rel, 'sandbox.html'), 'utf8'); const loader = page.slice(page.indexOf('<script>') + 8, page.lastIndexOf('</script>'));
  const hash = createHash('sha256').update(loader, 'utf8').digest('base64');
  const py = spawnSync('python3', ['-B', '-c', `import importlib.util,sys;s=importlib.util.spec_from_file_location('p',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(m.sandbox_csp(sys.argv[2]),end='')`, join(EXT, 'scripts', 'package-release.py'), hash], { encoding: 'utf8' });
  assert.equal(py.stdout, SANDBOX_CSP(hash, false)); assert.equal(JSON.parse(readFileSync(join(rel, 'manifest.json'), 'utf8')).content_security_policy.sandbox, py.stdout);
});
test('the tree as it is (the D1 network placeholder) is refused as a release', () => {
  assert.match(readFileSync(join(rel, 'lib', 'studio-network.js'), 'utf8'), MARK);
  refused(pack({ asBuilt: true }), /network placeholder \(NETWORK_PLACEHOLDER\)/);
  // Every other refusal still applies to the tree as it is; the placeholder refusal does not hide them.
  refused(pack({ asBuilt: true, edits: [manifest((m) => ({ ...m, host_permissions: [...m.host_permissions, '<all_urls>'] }))] }), /host_permissions/);
  refused(pack({ asBuilt: true, edits: [manifest((m) => ({ ...m, name: '[REHEARSAL] ' + m.name }))] }), /not the release name/);
});
test('the placeholder identity without its mark is refused', () => {
  refused(pack({ asBuilt: true, edits: [['lib/studio-network.js', dropMark]] }), /network placeholder identity/);
});
test('a build repinned to a retired network is refused, with every file that shows it', () => {
  const back = (t) => t.replaceAll('42'.repeat(32), RETIRED_ID.genesisHash.slice(2)).replace('"chainId": 4242', `"chainId": ${RETIRED_ID.chainId}`)
    .replace('>4242<', `>${RETIRED_ID.chainId}<`);
  const r = pack({ edits: [['lib/studio-network.js', back], ['publisher.html', back], ['contract.html', back]] });
  refused(r, new RegExp(`pins the retired network ${RETIRED_ID.chainId}`));
  for (const f of ['publisher.html', 'contract.html', 'lib/studio-network.js']) assert.ok(r.stderr.includes(f + ' shows the retired network'), r.stderr);
});
test('a fault build and a rehearsal build are refused by name', () => {
  refused(pack({ from: faultCsp }), /TEST FAULT cspOpen.*not the release name/);
  refused(pack({ edits: [manifest((m) => ({ ...m, name: '[REHEARSAL] ' + m.name }))] }), /REHEARSAL.*not the release name/);
});
test('widened permissions, extra manifest keys, another sandbox page or background are refused', () => {
  for (const p of ['scripting', 'tabs', 'cookies', 'debugger']) refused(pack({ edits: [manifest((m) => ({ ...m, permissions: [...m.permissions, p] }))] }), /permissions are .* not exactly/);
  refused(pack({ edits: [manifest((m) => ({ ...m, permissions: m.permissions.slice(1) }))] }), /permissions are/);
  refused(pack({ edits: [manifest((m) => ({ ...m, optional_permissions: ['tabs'] }))] }), /no optional_permissions/);
  refused(pack({ edits: [manifest((m) => ({ ...m, web_accessible_resources: [{ resources: ['viewer.html'], matches: ['<all_urls>'] }] }))] }), /web_accessible_resources/);
  refused(pack({ edits: [manifest((m) => ({ ...m, sandbox: { pages: ['sandbox.html', 'viewer.html'] } }))] }), /sandbox pages/);
  refused(pack({ edits: [manifest((m) => { const { sandbox, ...rest } = m; return rest; })] }), /sandbox pages/);
  refused(pack({ edits: [manifest((m) => ({ ...m, background: { ...m.background, type: 'classic' } }))] }), /background/);
  refused(pack({ edits: [manifest((m) => ({ ...m, devtools_page: 'options.html' }))] }), /keys the release does not: devtools_page/);
  // `key` is a release key (W14), checked against the committed slot; any other new key is still refused.
  refused(pack({ edits: [manifest((m) => ({ ...m, chrome_url_overrides: { newtab: 'options.html' } }))] }), /keys the release does not: chrome_url_overrides$/m);
});
test('a page that still shows a retired network is refused, even with NETWORK repinned', () => {
  refused(pack({ edits: [['publisher.html', (t) => t.replace('<span class="chip">4242</span>', `<span class="chip">${RETIRED_ID.chainId}</span>`)]] }), new RegExp(`publisher\\.html shows the retired network chain ID ${RETIRED_ID.chainId}`));
  refused(pack({ edits: [['contract.html', (t) => t.replace('<span class="network-id">4242</span>', `<span class="network-id">${RETIRED_ID.chainId}</span>`)]] }), /contract\.html shows the retired network chain ID/);
  refused(pack({ edits: [['README-EN.md', (t) => t + '\ngenesis ' + RETIRED_ID.genesisHash.toUpperCase().replace('0X', '0x') + '\n']] }), new RegExp(`README-EN\\.md shows the retired network genesis ${RETIRED_ID.genesisHash.toLowerCase()}`));
  // A decimal word only: the digits inside a longer number or a hex string are not the chain ID.
  const r = pack({ edits: [['README-EN.md', (t) => t + `\nid 9${RETIRED_ID.chainId} hash a${RETIRED_ID.chainId}f\n`]] }); assert.equal(r.status, 0, r.stderr);
});
test('widened permissions and content scripts are refused', () => {
  refused(pack({ edits: [manifest((m) => ({ ...m, host_permissions: [...m.host_permissions, 'https://*/*'] }))] }), /host_permissions/);
  refused(pack({ edits: [manifest((m) => ({ ...m, optional_host_permissions: ['https://*/*'] }))] }), /host_permissions/);
  refused(pack({ edits: [manifest((m) => ({ ...m, content_scripts: [{ ...m.content_scripts[0], matches: [...m.content_scripts[0].matches, 'https://pocol-studio.example.org/*'] }, m.content_scripts[1]] }))] }), /content_scripts/);
  refused(pack({ edits: [manifest((m) => ({ ...m, content_scripts: [{ ...m.content_scripts[0], all_frames: true }, m.content_scripts[1]] }))] }), /content_scripts/);
  refused(pack({ edits: [manifest((m) => ({ ...m, externally_connectable: { matches: ['https://studio.pocol.net/*'] } }))] }), /externally_connectable/);
});
test('an open or stale sandbox CSP and a widened extension-page CSP are refused', () => {
  refused(pack({ edits: [['sandbox.html', (t) => t.replace("'use strict';", "'use strict'; ")]] }), /sandbox CSP is not the release policy/);
  refused(pack({ edits: [manifest((m) => ({ ...m, content_security_policy: { ...m.content_security_policy, sandbox: m.content_security_policy.sandbox.replace("connect-src 'none'", 'connect-src *') } }))] }), /sandbox CSP/);
  refused(pack({ edits: [manifest((m) => ({ ...m, content_security_policy: { ...m.content_security_policy, extension_pages: "script-src 'self' 'unsafe-eval'; object-src 'self'" } }))] }), /extension-page CSP/);
});
// Sites W11: the testGateway build (a devnet behind a loopback Studio gateway as the built-in RN profile) is a fault build:
// refused by its name, and, renamed to the release name, still refused by its built-in profile.
test('the testGateway build is refused, also under the release name', () => {
  const tg = build({ fault: 'testGateway', out: join(root, 'testGateway'), lockMs: 20000,
    testNetwork: { name: 'devnet-gw', chainId: 4242, genesisHash: '0x' + '42'.repeat(32), genesisPre: '0x01', endpoint: 'http://127.0.0.1:8650/studio/rpc' } }).dist;
  assert.match(JSON.parse(readFileSync(join(tg, 'manifest.json'), 'utf8')).name, /\[TEST FAULT testGateway\]$/);
  assert.match(readFileSync(join(tg, 'lib', 'wallet.js'), 'utf8'), /LOCK_MS = 20000;/);
  refused(pack({ from: tg }), /fault or rehearsal build/);
  refused(pack({ from: tg, edits: [manifest((m) => ({ ...m, name: 'PoCol Studio, IDE & Viewer' }))] }), /NETWORK is not plain JSON|built-in profile must use/);
  assert.throws(() => build({ fault: 'testGateway', out: join(root, 'tg2'), testNetwork: { name: 'x', chainId: 1, genesisHash: '0x' + '42'.repeat(32), genesisPre: '0x01', endpoint: 'http://10.0.0.1:8650/studio/rpc' } }), /endpoint must be/);
  assert.throws(() => build({ out: join(root, 'tg3'), lockMs: 1000 }), /testGateway build only/);
});
test('another endpoint or trust level for the built-in profile is refused', () => {
  refused(pack({ edits: [['lib/studio-network.js', (t) => t.replace('"endpoint": "https://explorer.pocol.net/studio/rpc"', '"endpoint": "http://127.0.0.1:8540/studio/rpc"')]] }), /built-in profile must use/);
  // The retired gateway host (owner decision 9, 2026-11-16: explorer.pocol.net) is refused too.
  refused(pack({ edits: [['lib/studio-network.js', (t) => t.replace('"endpoint": "https://explorer.pocol.net/studio/rpc"', '"endpoint": "https://gateway.pocol.net/studio/rpc"')]] }), /built-in profile must use/);
  refused(pack({ edits: [['lib/studio-network.js', (t) => t.replace('"trustLevel": "RN"', '"trustLevel": "LN"')]] }), /built-in profile must use/);
});
test('a chatgpt.site origin anywhere in the build is refused', () => {
  refused(pack({ edits: [['studio-content.js', (t) => t.replace("['https://studio.pocol.net']", "['https://studio.pocol.net', 'https://pocol-studio.mystic-gecko-9511.chatgpt.site']")]] }), /studio-content\.js names a retired chatgpt\.site/);
  refused(pack({ edits: [['README-STUDIO.md', (t) => t + '\nhttps://x.chatgpt.site\n']] }), /README-STUDIO\.md names a retired chatgpt\.site/);
});
test('an unreadable retired list fails closed for a release', () => {
  rmSync(join(tree, 'dist'), { recursive: true, force: true }); cpSync(rel, join(tree, 'dist'), { recursive: true }); edit('lib/studio-network.js', releaseIdentity);
  setSlot(slotWith(TEST_KEY)); edit('manifest.json', (t) => JSON.stringify({ ...JSON.parse(t), key: TEST_KEY }, null, 2) + '\n');
  const r = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-release.py'), join(tree, 'x.zip'), '--retired', join(root, 'missing.json')], { encoding: 'utf8' });
  assert.notEqual(r.status, 0); assert.match(r.stderr, /cannot read the retired network identities/); assert.equal(existsSync(join(tree, 'x.zip')), false);
});

// W14 reproducible zip (scripts/package-release.py write_release_zip): the zip and its .sha256 depend only on the bytes
// and relative paths of dist/, never on where it was built or packaged, the order its files were made in, their mtimes
// or modes, or the time zone. The CI double build (.github/workflows/full-nonce-upgrade.yml, extension-reproducible)
// compares two real checkouts the same way.
const sha = (b) => createHash('sha256').update(b).digest('hex');
const names = (d) => files(d).map((f) => relative(d, f).split(sep).join('/')).sort();
const MODES = [0o600, 0o755, 0o640, 0o444];
/** Recreates dist/ with its files made in reverse order of name, other modes and mtimes spread over decades. */
function scramble(dist) {
  const list = names(dist), bytes = new Map(list.map((n) => [n, readFileSync(join(dist, n))]));
  rmSync(dist, { recursive: true, force: true });
  list.slice().reverse().forEach((n, i) => {
    const p = join(dist, n); mkdirSync(dirname(p), { recursive: true }); writeFileSync(p, bytes.get(n));
    const t = new Date(Date.UTC(1999 + (i % 90), i % 12, 1 + (i % 28), i % 24, i % 60, i % 60)); utimesSync(p, t, t);
    chmodSync(p, MODES[i % MODES.length]);
  });
}
test('the release zip and its .sha256 are byte-identical from builds in other directories, file orders, mtimes and modes', () => {
  // A second release build, made later in another directory (a deeper path with a space).
  const other = build({ out: join(root, 'second build', 'nested', 'dist') }).dist;
  assert.deepEqual(names(other), names(rel));
  for (const n of names(rel)) assert.equal(sha(readFileSync(join(other, n))), sha(readFileSync(join(rel, n))), `dist/${n} differs between the two builds`);
  // Each is packaged from its own packager tree, at another path, as a release (throwaway key, release identity); the
  // second one's dist/ is scrambled and packaged under another time zone.
  const twin = packagerTree(join(root, 'second checkout', 'prototypes', 'pocol-extension'));
  const a = pack({ out: 'release.zip' });
  const b = pack({ from: other, at: twin, out: 'release.zip', prepare: scramble, env: { ...process.env, TZ: 'Pacific/Kiritimati' } });
  assert.equal(a.status, 0, a.stderr); assert.equal(b.status, 0, b.stderr);
  // Not vacuous: every file's mtime, and the modes, differ between the two dist/ trees that were packaged.
  const list = names(join(tree, 'dist'));
  assert.deepEqual(names(join(twin, 'dist')), list); assert.ok(list.length > 20, list.join(' '));
  const stats = list.map((n) => [statSync(join(tree, 'dist', n)), statSync(join(twin, 'dist', n))]);
  assert.ok(stats.every(([x, y]) => x.mtimeMs !== y.mtimeMs), 'the two dist/ trees have the same mtimes');
  assert.ok(stats.some(([x, y]) => (x.mode & 0o777) !== (y.mode & 0o777)), 'the two dist/ trees have the same modes');
  const za = readFileSync(join(tree, 'release.zip')), zb = readFileSync(join(twin, 'release.zip'));
  assert.ok(za.equals(zb), `the two release zips differ: ${sha(za)} and ${sha(zb)}`);
  const sa = readFileSync(join(tree, 'release.zip.sha256')), sb = readFileSync(join(twin, 'release.zip.sha256'));
  assert.ok(sa.equals(sb), `the two .sha256 files differ: ${sa} and ${sb}`);
  assert.equal(sa.toString('utf8'), `${sha(za)}  release.zip\n`);
  for (const r of [a, b]) assert.match(r.stdout, new RegExp(`^SHA-256 ${sha(za)}$`, 'm'));
});
test('the release zip holds exactly dist/, sorted by name, with fixed dates, modes and host system, DEFLATE level 9', () => {
  const r = pack({ out: 'layout.zip', prepare: scramble }); assert.equal(r.status, 0, r.stderr);
  // Each entry as Python reads it, and whether its raw stream is zlib's level-9 raw DEFLATE of the dist/ file.
  const read = spawnSync('python3', ['-B', '-c', `import json, struct, sys, zipfile, zlib
from pathlib import Path
dist, out = Path(sys.argv[2]), []
with zipfile.ZipFile(sys.argv[1]) as z, open(sys.argv[1], 'rb') as f:
    for i in z.infolist():
        f.seek(i.header_offset); n, m = struct.unpack('<HH', f.read(30)[26:30]); f.seek(i.header_offset + 30 + n + m)
        data, c = (dist / i.filename).read_bytes(), zlib.compressobj(9, zlib.DEFLATED, -15)
        out.append({'name': i.filename, 'date': list(i.date_time), 'mode': oct(i.external_attr), 'system': i.create_system,
                    'method': i.compress_type, 'level9': f.read(i.compress_size) == c.compress(data) + c.flush(), 'same': z.read(i) == data})
    print(json.dumps({'entries': out, 'comment': z.comment.hex()}))`, join(tree, 'layout.zip'), join(tree, 'dist')], { encoding: 'utf8' });
  assert.equal(read.status, 0, read.stderr);
  const { entries, comment } = JSON.parse(read.stdout);
  assert.equal(comment, '');
  assert.deepEqual(entries.map((e) => e.name), names(join(tree, 'dist')), 'the zip lists every dist/ file once, sorted by name, and nothing else');
  for (const e of entries) {
    assert.deepEqual({ ...e, name: undefined }, { name: undefined, date: [1980, 1, 1, 0, 0, 0], mode: '0o' + (0o100644 * 65536).toString(8), system: 3, method: 8, level9: true, same: true }, e.name);
  }
});
test('the zip writer refuses a symbolic link, a key file, node_modules or an output inside dist/, and writes no zip', () => {
  refused(pack({ prepare: (d) => symlinkSync('manifest.json', join(d, 'link.json')) }), /must not contain symbolic links: link\.json/);
  refused(pack({ prepare: (d) => writeFileSync(join(d, 'lib', 'miner.pocol-key'), 'throwaway') }), /Unexpected release member: lib\/miner\.pocol-key/);
  refused(pack({ prepare: (d) => { mkdirSync(join(d, 'node_modules', 'x'), { recursive: true }); writeFileSync(join(d, 'node_modules', 'x', 'i.js'), ''); } }), /Unexpected release member: node_modules\/x\/i\.js/);
  const inside = pack({ out: join('dist', 'inside.zip') });
  refused(inside, /the release zip must be written outside dist\//); assert.equal(existsSync(join(tree, 'dist', 'inside.zip.sha256')), false);
});
