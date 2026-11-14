// Sites W3: scripts/package-release.py refuses every build that is not the release, independently of the tests on
// dist/: fault and rehearsal names, widened permissions or content scripts, a manifest key the release does not have,
// another sandbox page or background, an open or stale sandbox CSP, another endpoint or trust level, a chatgpt.site
// origin, the placeholder or a retired identity (in the built-in profile or shown by any file). --unreleased packages a CI
// artifact of the current tree (retired identity) under an UNRELEASED name, with every other refusal kept.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, cpSync, readFileSync, writeFileSync, rmSync, existsSync, readdirSync, statSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, SANDBOX_CSP } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE);
const RETIRED = join(EXT, '..', '..', 'deploy', 'mainnet', 'retired-networks.json');
const root = mkdtempSync(join(tmpdir(), 'pocol-package-'));
const rel = build({ out: join(root, 'release') }).dist, faultCsp = build({ fault: 'cspOpen', out: join(root, 'fault') }).dist;
test.after(() => rmSync(root, { recursive: true, force: true }));
const tree = join(root, 'tree');
mkdirSync(join(tree, 'scripts'), { recursive: true });
cpSync(join(EXT, 'scripts', 'package-release.py'), join(tree, 'scripts', 'package-release.py'));
writeFileSync(join(tree, 'package.json'), readFileSync(join(EXT, 'package.json')));
const edit = (file, f) => { const p = join(tree, 'dist', file); writeFileSync(p, f(readFileSync(p, 'utf8'))); };
const RETIRED_ID = JSON.parse(readFileSync(RETIRED, 'utf8')).retired[0];
const releaseIdentity = (t) => t.replace(/"chainId": \d+/, '"chainId": 4242').replace(/"genesisHash": "0x[0-9a-f]{64}"/, `"genesisHash": "0x${'42'.repeat(32)}"`);
// As deploy/mainnet/set_network_identity.py does, every page and README that shows the network is repinned too.
const shownIdentity = (t) => t.replaceAll(String(RETIRED_ID.chainId), '4242').replaceAll(RETIRED_ID.genesisHash.slice(2), '42'.repeat(32));
const TEXT = /\.(js|html|md|json|txt)$/;
const files = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? files(join(d, f)) : [join(d, f)]));
/** Packages a copy of `from` after `edits`, with a release identity unless `retired` keeps the tree's own. */
function pack({ from = rel, edits = [], retired = false, args = [], out = 'out.zip' } = {}) {
  rmSync(join(tree, 'dist'), { recursive: true, force: true }); rmSync(join(tree, out), { force: true });
  cpSync(from, join(tree, 'dist'), { recursive: true });
  if (!retired) {
    edit('lib/studio-network.js', releaseIdentity);
    for (const f of files(join(tree, 'dist')).filter((f) => TEXT.test(f))) { const t = readFileSync(f, 'utf8'), u = shownIdentity(t); if (u !== t) writeFileSync(f, u); }
  }
  for (const [file, f] of edits) edit(file, f);
  const r = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-release.py'), join(tree, out), '--retired', RETIRED, ...args], { encoding: 'utf8' });
  return { ...r, zip: existsSync(join(tree, out)) };
}
const refused = (r, rx) => { assert.notEqual(r.status, 0, r.stdout); assert.equal(r.zip, false); assert.match(r.stderr, rx); };
const manifest = (f) => ['manifest.json', (t) => JSON.stringify(f(JSON.parse(t)), null, 2) + '\n'];

test('the release build with a release identity is packaged', () => { const r = pack(); assert.equal(r.status, 0, r.stderr); assert.equal(r.zip, true); });
test('the packager\'s sandbox CSP is build.mjs SANDBOX_CSP(hash, false)', () => {
  const page = readFileSync(join(rel, 'sandbox.html'), 'utf8'); const loader = page.slice(page.indexOf('<script>') + 8, page.lastIndexOf('</script>'));
  const hash = createHash('sha256').update(loader, 'utf8').digest('base64');
  const py = spawnSync('python3', ['-B', '-c', `import importlib.util,sys;s=importlib.util.spec_from_file_location('p',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(m.sandbox_csp(sys.argv[2]),end='')`, join(EXT, 'scripts', 'package-release.py'), hash], { encoding: 'utf8' });
  assert.equal(py.stdout, SANDBOX_CSP(hash, false)); assert.equal(JSON.parse(readFileSync(join(rel, 'manifest.json'), 'utf8')).content_security_policy.sandbox, py.stdout);
});
test('the tree as it is (pinned to the retired network) is refused as a release', () => refused(pack({ retired: true }), new RegExp(`pins the retired network ${RETIRED_ID.chainId}`)));
test('--unreleased packages the current tree under an UNRELEASED name only', () => {
  const r = pack({ retired: true, args: ['--unreleased'], out: 'PoCol-x-UNRELEASED.zip' }); assert.equal(r.status, 0, r.stderr); assert.equal(r.zip, true); assert.match(r.stderr, /UNRELEASED \(never publish\)/);
  refused(pack({ retired: true, args: ['--unreleased'], out: 'release.zip' }), /must have UNRELEASED in its name/);
  refused(pack({ retired: true, args: ['--unreleased'], out: 'x-UNRELEASED.zip', edits: [manifest((m) => ({ ...m, host_permissions: [...m.host_permissions, '<all_urls>'] }))] }), /host_permissions/);
});
test('the placeholder identity is refused', () => {
  refused(pack({ edits: [['lib/studio-network.js', (t) => t.replace('"name": "pocol-public"', '"name": "pocol-placeholder"')]] }), /placeholder identity/);
  refused(pack({ edits: [['lib/studio-network.js', (t) => '// @placeholder\n' + t]] }), /placeholder identity/);
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
  refused(pack({ edits: [manifest((m) => ({ ...m, chrome_url_overrides: { newtab: 'options.html' }, key: 'MIIB' }))] }), /keys the release does not: chrome_url_overrides, key/);
});
test('a page that still shows a retired network is refused, even with NETWORK repinned', () => {
  refused(pack({ edits: [['publisher.html', (t) => t.replace('<span class="chip">4242</span>', `<span class="chip">${RETIRED_ID.chainId}</span>`)]] }), new RegExp(`publisher\\.html shows the retired network chain ID ${RETIRED_ID.chainId}`));
  refused(pack({ edits: [['contract.html', (t) => t.replace('<span class="network-id">4242</span>', `<span class="network-id">${RETIRED_ID.chainId}</span>`)]] }), /contract\.html shows the retired network chain ID/);
  refused(pack({ edits: [['README-EN.md', (t) => t + '\ngenesis ' + RETIRED_ID.genesisHash.toUpperCase().replace('0X', '0x') + '\n']] }), new RegExp(`README-EN\\.md shows the retired network genesis ${RETIRED_ID.genesisHash.toLowerCase()}`));
  // A decimal word only: the digits inside a longer number or a hex string are not the chain ID.
  const r = pack({ edits: [['README-EN.md', (t) => t + `\nid 9${RETIRED_ID.chainId} hash a${RETIRED_ID.chainId}f\n`]] }); assert.equal(r.status, 0, r.stderr);
  // --unreleased reports every file that shows it and still packages the CI artifact.
  const u = pack({ retired: true, args: ['--unreleased'], out: 'y-UNRELEASED.zip' }); assert.equal(u.status, 0, u.stderr);
  for (const f of ['publisher.html', 'contract.html', 'lib/studio-network.js']) assert.ok(u.stderr.includes(f + ' shows the retired network'), u.stderr);
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
test('another endpoint or trust level for the built-in profile is refused', () => {
  refused(pack({ edits: [['lib/studio-network.js', (t) => t.replace('"endpoint": "https://gateway.pocol.net/studio/rpc"', '"endpoint": "http://127.0.0.1:8540/studio/rpc"')]] }), /built-in profile must use/);
  refused(pack({ edits: [['lib/studio-network.js', (t) => t.replace('"trustLevel": "RN"', '"trustLevel": "LN"')]] }), /built-in profile must use/);
});
test('a chatgpt.site origin anywhere in the build is refused', () => {
  refused(pack({ edits: [['studio-content.js', (t) => t.replace("['https://studio.pocol.net']", "['https://studio.pocol.net', 'https://pocol-studio.mystic-gecko-9511.chatgpt.site']")]] }), /studio-content\.js names a retired chatgpt\.site/);
  refused(pack({ edits: [['README-STUDIO.md', (t) => t + '\nhttps://x.chatgpt.site\n']] }), /README-STUDIO\.md names a retired chatgpt\.site/);
});
test('an unreadable retired list fails closed for a release', () => {
  rmSync(join(tree, 'dist'), { recursive: true, force: true }); cpSync(rel, join(tree, 'dist'), { recursive: true }); edit('lib/studio-network.js', releaseIdentity);
  const r = spawnSync('python3', ['-B', join(tree, 'scripts', 'package-release.py'), join(tree, 'x.zip'), '--retired', join(root, 'missing.json')], { encoding: 'utf8' });
  assert.notEqual(r.status, 0); assert.match(r.stderr, /cannot read the retired network identities/); assert.equal(existsSync(join(tree, 'x.zip')), false);
});
