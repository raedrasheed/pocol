// The wallet pages in the build (2026-11-22, EXT-02/06): the toolbar popup is the manifest's action popup, the popup,
// Send and Settings pages load only packaged scripts, never touch the vault or the network themselves (only the worker
// does), no extension code starts a Web Worker, and the extension-page CSP is the string the release packager requires.
// The real-browser evidence for these pages is tests/wallet_ui_browser.mjs.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync, mkdtempSync, rmSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { build, EXTENSION_PAGES_CSP } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE);
const out = mkdtempSync(join(tmpdir(), 'pocol-wallet-pages-'));
test.after(() => rmSync(out, { recursive: true, force: true }));
const { dist, manifest } = build({ out: join(out, 'dist') });
const read = (f) => readFileSync(join(dist, f), 'utf8');

test('the toolbar button opens the wallet popup, and every wallet page is in the package', () => {
  assert.equal(manifest.action.default_popup, 'popup.html');
  for (const f of ['popup.html', 'popup.js', 'send.html', 'send.js', 'wallet.css', 'options.html', 'options.js', 'lib/account.js', 'lib/identity.js']) assert.ok(existsSync(join(dist, f)), f);
  // No permission was added for the wallet (store/listing.md justifies exactly these).
  assert.deepEqual(manifest.permissions, ['storage', 'unlimitedStorage', 'declarativeNetRequest']);
  assert.deepEqual(manifest.host_permissions, ['http://127.0.0.1/*', 'https://explorer.pocol.net/*']);
  assert.equal(manifest.optional_permissions, undefined); assert.equal(manifest.web_accessible_resources, undefined); assert.equal(manifest.externally_connectable, undefined);
});

test('EXT-06: no extension page starts a worker, and the packager requires the build\'s extension-page CSP', () => {
  // worker-src 'none' is not used: Chrome does not apply it to extension pages (build.mjs, tests/wallet_ui_browser.mjs).
  assert.equal(manifest.content_security_policy.extension_pages, "script-src 'self'; object-src 'self'");
  assert.equal(EXTENSION_PAGES_CSP, manifest.content_security_policy.extension_pages);
  const py = spawnSync('python3', ['-B', '-c', `import importlib.util,sys;s=importlib.util.spec_from_file_location('p',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(m.EXTENSION_PAGES_CSP,end='')`, join(EXT, 'scripts', 'package-release.py')], { encoding: 'utf8' });
  assert.equal(py.status, 0, py.stderr); assert.equal(py.stdout, EXTENSION_PAGES_CSP);
  // No extension page, library or vendored module starts a worker.
  const walk = (d) => readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(d, e.name)) : [join(d, e.name)]));
  for (const f of walk(dist).filter((f) => f.endsWith('.js'))) assert.doesNotMatch(readFileSync(f, 'utf8'), /new (Shared)?Worker\(|serviceWorker\.register\(/, f);
});

test('the popup, Send and Settings pages load only their own packaged module, and hold no key, vault or network access', () => {
  for (const [page, script] of [['popup.html', 'popup.js'], ['send.html', 'send.js'], ['options.html', 'options.js']]) {
    const html = read(page);
    const scripts = [...html.matchAll(/<script\b([^>]*)>([^]*?)<\/script>/g)];
    assert.equal(scripts.length, 1, page); assert.equal(scripts[0][2].trim(), '', `${page} has no inline script`);
    assert.match(scripts[0][1], new RegExp(`type="module" src="${script}"`));
    assert.doesNotMatch(html, /https?:\/\/(?!(studio|ide|explorer)\.pocol\.net)[a-z]/i, `${page} links no other host`);
  }
  for (const f of ['popup.js', 'send.js']) {
    const js = read(f);
    assert.doesNotMatch(js, /\bfetch\(|XMLHttpRequest|WebSocket|chrome\.storage|studio:wallet|importKeyfile|signTx|^\s*import\b|\bimport\(/m, `${f} talks to the worker only`);
  }
  // The Settings page reads job records from storage (as before) but never the vault; its wallet actions are messages.
  const options = read('options.js');
  assert.doesNotMatch(options, /\bfetch\(|studio:wallet|importKeyfile|exportKeyfile/);
  for (const type of ['account.create', 'account.import', 'account.export', 'connections.list', 'connections.revoke']) assert.ok(options.includes(`'${type}'`), type);
});

test('the Send page signs only after its consent box and the user\'s own click, and shows every field of the review', () => {
  const html = read('send.html'), js = read('send.js');
  for (const id of ['rNetwork', 'rChain', 'rGenesis', 'rEndpoint', 'rFrom', 'rTo', 'rAmount', 'rFee', 'rTotal', 'rNonce', 'rHash', 'consent']) assert.match(html, new RegExp(`id="${id}"`), id);
  assert.match(html, /<button id="confirm" type="button" class="primary" disabled>/);
  assert.match(js, /if \(!e\.isTrusted \|\| !review \|\| !\$\('consent'\)\.checked\) return;/);
  assert.ok(js.indexOf("type: 'send.confirm'") > js.indexOf('e.isTrusted'), 'send.confirm only after the trusted-click check');
});

// EXT-07 (2026-11-22): the vendored ethers is shipped whole, byte-identical to ethers 6.17.0's dist/ethers.min.js from
// the npm registry (package-lock.json pins the tarball's integrity); store/listing.md states the hash for the reviewer.
test('EXT-07: vendor/ethers.js is the unmodified ethers 6.17.0 dist/ethers.min.js, with the hash the listing states', async () => {
  const { createHash } = await import('node:crypto');
  const ETHERS_MIN_SHA256 = 'b016b0c3898c78fd8156466eb1ff1f42c9df951c2f0d64c9bdf799fe745b0a6c';
  const lock = JSON.parse(readFileSync(join(EXT, 'package-lock.json'), 'utf8')).packages['node_modules/ethers'];
  assert.equal(lock.version, '6.17.0'); assert.equal(lock.resolved, 'https://registry.npmjs.org/ethers/-/ethers-6.17.0.tgz');
  assert.equal(createHash('sha256').update(readFileSync(join(dist, 'vendor', 'ethers.js'))).digest('hex'), ETHERS_MIN_SHA256);
  assert.ok(readFileSync(join(EXT, 'store', 'listing.md'), 'utf8').includes(ETHERS_MIN_SHA256), 'store/listing.md names the hash');
});

test('the vendored @noble modules are their npm files as store/listing.md says: secp256k1 unchanged, hashes with only the import made relative and the source-map comment removed', () => {
  const nm = join(EXT, 'node_modules', '@noble');
  assert.ok(readFileSync(join(dist, 'vendor', 'secp256k1', 'index.js')).equals(readFileSync(join(nm, 'secp256k1', 'index.js'))), 'vendor/secp256k1/index.js is byte-identical');
  const hashes = readdirSync(join(dist, 'vendor', 'hashes')).filter((f) => f.endsWith('.js')).sort();
  assert.deepEqual(hashes, ['_md.js', '_u64.js', 'crypto.js', 'hmac.js', 'sha2.js', 'sha3.js', 'utils.js']);
  for (const f of hashes) {
    const npm = readFileSync(join(nm, 'hashes', 'esm', f), 'utf8'), shipped = read(join('vendor', 'hashes', f));
    assert.equal(shipped, npm.replace(/from '@noble\/hashes\/crypto'/g, "from './crypto.js'").replace(/\n\/\/# sourceMappingURL=.*$/m, ''), f);
    assert.equal(shipped.split('\n').length >= npm.split('\n').length - 1, true, f + ': at most the source-map line is removed');
  }
  const listing = readFileSync(join(EXT, 'store', 'listing.md'), 'utf8').replace(/\s+/g, ' ');
  assert.match(listing, /`vendor\/ethers\.js` and `vendor\/secp256k1\/index\.js` are byte-identical to their npm files; the `vendor\/hashes\/` modules of @noble\/hashes 1\.8\.0 are its `esm\/` files with only the `@noble\/hashes\/crypto` import made relative/);
  assert.equal(JSON.parse(readFileSync(join(EXT, 'package-lock.json'), 'utf8')).packages['node_modules/@noble/hashes'].version, '1.8.0');
});

// Final review (2026-11-22): the popup's refreshes overlap; the agreement's refresh of the built-in profile used to answer
// after the refresh of the profile the user picked next, and overwrote it ("Endpoint unreachable" for a profile no longer
// shown; the cause of tests/wallet_ui_browser.mjs's timeouts at "popup connected"). Only the latest refresh may write.
test('final review: a late answer for the profile no longer selected never overwrites the popup', async () => {
  const vm = await import('node:vm');
  const html = read('popup.html'), source = read('popup.js');
  const make = (id) => ({ id, textContent: '', hidden: false, disabled: false, value: '', className: '', href: '', children: [], replaceChildren(...c) { this.children = c; } });
  const elements = new Map([...html.matchAll(/id="([^"]+)"/g)].map((m) => [m[1], make(m[1])]));
  let release; const late = new Promise((r) => { release = r; });
  const answers = {
    'account.status': () => ({ ok: true, exists: true, unlocked: true, address: '0x' + '11'.repeat(20), disclosureAccepted: true }),
    'profiles.list': () => ({ ok: true, profiles: [{ name: 'pocol-public', chainId: 1 }, { name: 'devnet', chainId: 2 }] }),
    'net.status': (m) => (m.name === 'devnet' ? { ok: true, chainId: 2, endpoint: 'http://127.0.0.1:1', genesisHash: '0x' + '22'.repeat(32), reachable: true, chainOk: true, genesisOk: true, head: { height: 7, ageSec: 3 } }
      : late.then(() => ({ ok: true, chainId: 1, endpoint: 'https://x', genesisHash: '0x' + '33'.repeat(32), reachable: false, error: 'unreachable' }))),
    'account.balance': (m) => ({ ok: true, pcol: m.name === 'devnet' ? '2 PCOL' : '0 PCOL' }),
    'txs.list': () => ({ ok: true, transactions: [] }), 'txs.refresh': () => ({ ok: true, transactions: [] }),
  };
  const context = vm.createContext({ console, Object, Promise, BigInt, String, Number,
    document: { getElementById: (id) => elements.get(id), createElement: () => make('x') },
    localStorage: { getItem: () => null, setItem() {} },
    chrome: { runtime: { sendMessage: async (m) => answers[m.type](m), getURL: (p) => p, openOptionsPage() {} }, tabs: { create() {} } } });
  vm.runInContext(source, context);
  const settle = async () => { for (let i = 0; i < 30; i++) await new Promise((r) => setImmediate(r)); };
  await settle();
  assert.equal(elements.get('netText').textContent, 'Checking…', 'the first refresh still waits for the built-in profile');
  elements.get('network').value = 'devnet'; elements.get('network').onchange(); await settle();
  assert.match(elements.get('netText').textContent, /^Connected · chain ID and genesis match · block 7/);
  release(); await settle();
  assert.match(elements.get('netText').textContent, /^Connected/, 'the late answer for pocol-public did not overwrite the devnet status');
  assert.equal(elements.get('balance').textContent, '2 PCOL');
});
