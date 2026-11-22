// The PoCol wallet in real Chromium against a local 3-node pocold devnet (2026-11-22, EXT-02/04/05/06/08):
//   POCOLD=<pocold> [POCOL_BIN_DIR=<dir with pocol-sign>] node tests/wallet_ui_browser.mjs
// (npm run test:wallet-browser; PLAYWRIGHT_MODULE as in tests/e2e_browser.mjs). It starts and stops its own devnet
// with scripts/devnet.py (ports 30300-30302 and 8540-8542, which must be free), and an RPC proxy at 127.0.0.1:8597 in
// front of node a that can rewrite pocol_status.genesisHash (an endpoint of another network) and logs every method.
// Every other host is unresolvable in the browser (--host-resolver-rules), so nothing reaches a PoCol or any other
// server. Keys: a fresh random key made here (never written to disk outside the temporary browser profile) and the
// devnet's built-in "alice" test account, which funds it.
//
// What it shows, through the extension's own pages (popup.html opened as a tab, options.html, send.html):
//  - the first-run disclosure blocks network reads until it is accepted;
//  - import from a private key in Settings; the popup's network status (connected, chain ID and genesis match, block
//    height and age) and balance from the devnet;
//  - the encrypted backup downloads after the password, and restores the same address in a fresh browser profile;
//  - Send: the review shows network, chain ID, genesis, endpoint, from, to, amount, fee and total; Confirm is disabled
//    until the consent box; after the click the transfer is included on the devnet as an EIP-1559 transaction with the
//    profile's chain ID, the recipient's balance is exactly the amount, and the popup lists it as included;
//  - an endpoint whose genesis differs: the popup says "Wrong network" with the difference, Send refuses with it, and
//    nothing more is submitted;
//  - the built-in profile of a placeholder build: "Not connected", no Send;
//  - EXT-06 (recorded only): whether worker-src 'none' would stop a blob: worker or a packaged worker script (w.js, in
//    the temporary test copies only) in an extension page of this Chrome;
//  - EXT-08: a viewer tab opened for a Studio request (viewer.html?confirm=studio) waits for the click and reads nothing.
import { spawnSync } from 'node:child_process';
import { createServer, request as httpRequest } from 'node:http';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, cpSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { connect } from 'node:net';
import { Wallet, Transaction } from 'ethers';
import { build } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE), POCOLD_DIR = join(EXT, '..', 'pocold');
const POCOLD = process.env.POCOLD || join(POCOLD_DIR, 'target', 'release', 'pocold');
const BIN_DIR = process.env.POCOL_BIN_DIR || dirname(POCOLD);
const PW_MODULE = process.env.PLAYWRIGHT_MODULE || '/opt/node-tools/node_modules/playwright/index.mjs';
const { chromium } = await import(PW_MODULE);
const PROXY = 8597, PASSWORD = 'wallet-browser-test-1';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0; const results = [];
const ok = (name, cond, detail = '') => { results.push({ name, pass: !!cond }); console.log((cond ? 'PASS ' : 'FAIL ') + name + (detail !== '' ? ' — ' + String(detail).slice(0, 400) : '')); if (!cond) failed++; };
const wait = async (pred, ms, what) => { const t = Date.now(); let last; while (Date.now() - t < ms) { try { if ((last = await pred())) return last; } catch (e) { last = e; } await sleep(300); } throw new Error('timeout: ' + what + (last instanceof Error ? ' (' + last.message + ')' : '')); };
const portFree = (p) => new Promise((res) => { const s = connect(p, '127.0.0.1'); s.on('connect', () => { s.destroy(); res(false); }); s.on('error', () => res(true)); });
const rpc = async (port, method, params = []) => {
  const r = await fetch(`http://127.0.0.1:${port}/`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) });
  const v = await r.json(); if (v.error) throw Object.assign(new Error(v.error.message), { rpc: v.error }); return v.result;
};
const py = (args) => { const r = spawnSync('python3', [join(POCOLD_DIR, 'scripts', 'devnet.py'), ...args], { encoding: 'utf8', env: { ...process.env, POCOLD }, timeout: 120000 }); if (r.status) throw new Error('devnet.py ' + args[0] + ': ' + r.stderr); return r.stdout; };

// ---------------- RPC proxy (genesis override, method log) ----------------
const proxy = { log: [], genesisOverride: null };
const proxyServer = createServer((req, res) => {
  let body = ''; req.on('data', (c) => { body += c; }); req.on('end', () => {
    let m = null; try { m = JSON.parse(body); } catch { /* forwarded as is */ }
    const method = m && !Array.isArray(m) ? m.method : null; proxy.log.push(method);
    const up = httpRequest({ host: '127.0.0.1', port: 8540, method: 'POST', path: '/', headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(body) } }, (r) => {
      let t = ''; r.on('data', (c) => { t += c; }); r.on('end', () => {
        if (proxy.genesisOverride && method === 'pocol_status') { try { const v = JSON.parse(t); if (v.result) v.result.genesisHash = proxy.genesisOverride; t = JSON.stringify(v); } catch { /* unchanged */ } }
        res.writeHead(r.statusCode, { 'content-type': 'application/json', 'access-control-allow-origin': '*' }); res.end(t);
      });
    });
    up.on('error', () => { res.writeHead(502); res.end(); }); up.end(body);
  });
});

async function openBrowser(dist, tag) {
  const userDir = mkdtempSync(join(tmpdir(), 'pocol-wallet-ui-' + tag + '-'));
  const ctx = await chromium.launchPersistentContext(userDir, { channel: 'chromium', headless: true, acceptDownloads: true,
    args: [`--disable-extensions-except=${dist}`, `--load-extension=${dist}`, '--no-proxy-server', '--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1'] });
  let [sw] = ctx.serviceWorkers(); if (!sw) sw = await ctx.waitForEvent('serviceworker', { timeout: 20000 });
  const id = new URL(sw.url()).host;
  const page = async (path) => { const p = await ctx.newPage(); await p.goto(`chrome-extension://${id}/${path}`); return p; };
  return { ctx, id, page, close: async () => { await ctx.close(); rmSync(userDir, { recursive: true, force: true }); } };
}
/** Runs new Worker(blob:), or with `packaged` the extension's own file w.js, in an extension page; resolves 'ran' when
 *  the worker posts back, else what stopped it. */
const tryWorker = (p, packaged = false) => p.evaluate((packaged) => new Promise((resolve) => {
  try {
    const w = new Worker(packaged ? 'w.js' : URL.createObjectURL(new Blob(['postMessage(1)'], { type: 'text/javascript' })));
    w.onmessage = () => resolve('ran'); w.onerror = () => resolve('error event');
    setTimeout(() => resolve('no message'), 3000);
  } catch (e) { resolve('threw ' + e.name); }
}), packaged);
const text = (p, sel) => p.textContent(sel).then((t) => (t || '').trim());

async function main() {
  for (const p of [30300, 30301, 30302, 8540, 8541, 8542, PROXY]) if (!(await portFree(p))) throw new Error('port busy ' + p);
  if (!existsSync(POCOLD) || !existsSync(join(BIN_DIR, 'pocol-sign'))) throw new Error(`pocold or pocol-sign missing (POCOLD=${POCOLD}, POCOL_BIN_DIR=${BIN_DIR})`);
  const work = mkdtempSync(join(process.env.WALLET_E2E_WORK || tmpdir(), 'pocol-wallet-devnet-'));
  const out = mkdtempSync(join(tmpdir(), 'pocol-wallet-dist-'));
  const browsers = [];
  await new Promise((r) => proxyServer.listen(PROXY, '127.0.0.1', r));
  py(['init', work, '--miners', 'a,b,c', '--t-blk', '2', '--tau', '20', '--c', '8', '--d-att', '41', '--fb-wait', '3', '--work-bits', '15']);
  const info = JSON.parse(readFileSync(join(work, 'net', 'devnet.json'), 'utf8')), cid = Number(info.chainId);
  const base = JSON.parse(readFileSync(join(work, 'net', 'profile.json'), 'utf8'));
  py(['start', work]);
  try {
    await wait(async () => Number(await rpc(8540, 'eth_blockNumber')) >= 3, 180000, 'devnet height >= 3');
    const dist = build({ out: join(out, 'dist') }).dist;
    const placeholder = readFileSync(join(dist, 'lib', 'studio-network.js'), 'utf8').includes('export const NETWORK_PLACEHOLDER = true;');
    const b = await openBrowser(dist, 'a'); browsers.push(b);
    const popup = await b.page('popup.html');
    await popup.waitForSelector('#disclosure:not([hidden])', { timeout: 15000 });
    ok('first run: the popup asks for the disclosure before any wallet network read', /Waiting for your agreement/.test(await text(popup, '#netText')) && proxy.log.length === 0, await text(popup, '#netText'));

    // Settings: add the devnet profile (through the proxy) and import a fresh key.
    const options = await b.page('options.html');
    await options.fill('#profile', JSON.stringify({ ...base, endpoint: `http://127.0.0.1:${PROXY}`, trustLevel: 'LN' }));
    await options.click('#add');
    await wait(async () => (await text(options, '#profiles')).includes('devnet'), 15000, 'profile listed');
    const key = Wallet.createRandom(), address = key.address.toLowerCase();
    await options.selectOption('#importKind', 'privateKey');
    await options.fill('#newPw', PASSWORD); await options.fill('#newPw2', PASSWORD); await options.fill('#privateKey', key.privateKey);
    await options.click('#acctSave');
    await wait(async () => (await text(options, '#acctNote')).includes(address), 60000, 'imported');
    ok('import from a private key in Settings: the address, unlocked, and the key field cleared', (await text(options, '#acctState')).includes(address) && /unlocked/.test(await text(options, '#acctState')) && (await options.inputValue('#privateKey')) === '', await text(options, '#acctState'));

    // Fund it from the devnet's alice account.
    const alice = info.accounts.find((a) => a.label === 'alice').address;
    const aliceNonce = Number(await rpc(8540, 'eth_getTransactionCount', [alice, 'pending']));
    const fund = spawnSync(join(BIN_DIR, 'pocol-sign'), ['--chain-id', String(cid), '--account', 'alice', '--to', address, '--value', '2000000000000000000', '--nonce', String(aliceNonce), '--gas', '21000'], { encoding: 'utf8' }).stdout.trim();
    await rpc(8540, 'eth_sendRawTransaction', [fund]);
    await wait(async () => BigInt(await rpc(8540, 'eth_getBalance', [address, 'latest'])) === 2n * 10n ** 18n, 90000, 'funded');

    // Popup: accept the disclosure, choose the devnet profile, see status and balance.
    await popup.click('#accept');
    // The popup lists the profile added in Settings after it was opened (the list is read on every refresh).
    await wait(async () => (await popup.$$eval('#network option', (o) => o.map((x) => x.value))).includes('devnet'), 15000, 'devnet listed in the popup');
    await popup.selectOption('#network', 'devnet');
    await wait(async () => /^Connected/.test(await text(popup, '#netText')), 30000, 'popup connected');
    const status = await text(popup, '#netText'), detail = await text(popup, '#netDetail');
    ok('popup: connected, chain ID and genesis match, block height and age', /chain ID and genesis match · block \d+, \d+ s ago/.test(status) && detail.includes(`Chain ID ${cid}`) && detail.includes(`http://127.0.0.1:${PROXY}`), status + ' | ' + detail);
    await wait(async () => (await text(popup, '#balance')) === '2 PCOL', 30000, 'balance');
    ok('popup: address, unlocked, balance from the devnet', (await text(popup, '#address')) === address && (await text(popup, '#lockState')) === 'Unlocked' && (await text(popup, '#balance')) === '2 PCOL' && !(await popup.isDisabled('#send')));
    ok('popup: no explorer link for a custom network (the list is labelled as this extension\'s transfers only)', await popup.isHidden('#explorer') && /not the address's full history/.test(await text(popup, '#activityBox')));

    // Backup: password again, a downloaded encrypted file.
    await options.reload();
    await options.fill('#backupPw', 'not-the-password-1'); await options.click('#backup');
    await wait(async () => /Backup refused/.test(await text(options, '#acctNote')), 30000, 'wrong backup password');
    ok('backup with a wrong password is refused', /Wrong password/.test(await text(options, '#acctNote')), await text(options, '#acctNote'));
    await sleep(500);
    await options.fill('#backupPw', PASSWORD);
    const [download] = await Promise.all([options.waitForEvent('download', { timeout: 60000 }), options.click('#backup')]);
    const backupFile = join(out, 'backup.json'); await download.saveAs(backupFile);
    const backup = JSON.parse(readFileSync(backupFile, 'utf8'));
    ok('backup: an encrypted pocol-studio-vault-v1 file of this address, without the key', backup.schema === 'pocol-studio-vault-v1' && backup.address === address && backup.iter === 600000 && !readFileSync(backupFile, 'utf8').includes(key.privateKey.slice(2)), download.suggestedFilename());

    // Send PCOL.
    const to = Wallet.createRandom().address.toLowerCase();
    const send = await b.page('send.html#devnet');
    await wait(async () => /^Connected/.test(await text(send, '#netText')), 30000, 'send page connected');
    await send.fill('#to', to); await send.fill('#amount', '0.25'); await send.click('#review');
    await send.waitForSelector('#reviewBox:not([hidden])', { timeout: 30000 });
    const review = Object.fromEntries(await Promise.all(['rNetwork', 'rChain', 'rGenesis', 'rEndpoint', 'rFrom', 'rTo', 'rAmount', 'rFee', 'rTotal', 'rNonce', 'rHash'].map(async (id) => [id, await text(send, '#' + id)])));
    ok('Send review: network, chain ID, genesis, endpoint, from, to, amount, fee and total', review.rNetwork.startsWith('devnet') && review.rChain === String(cid) && review.rGenesis === base.genesisHash.toLowerCase() && review.rEndpoint === `http://127.0.0.1:${PROXY}`
      && review.rFrom === address && review.rTo === to && review.rAmount === '0.25 PCOL' && /^about .* PCOL, at most .* PCOL \(\d+ gas at most \d+ wei per gas\)$/.test(review.rFee) && /^0\.25\d* PCOL$/.test(review.rTotal), JSON.stringify(review));
    ok('Send: Confirm is disabled until the consent box, and nothing was submitted by the review', await send.isDisabled('#confirm') && !proxy.log.includes('eth_sendRawTransaction'));
    await send.check('#consent');
    await send.click('#confirm');
    await wait(async () => /^Included in block \d+/.test(await text(send, '#resultText')), 120000, 'included');
    const hash = await text(send, '#resultHash');
    const tx = await rpc(8540, 'eth_getTransactionByHash', [hash]), receipt = await rpc(8540, 'eth_getTransactionReceipt', [hash]);
    ok('Send: included on the devnet as an EIP-1559 transfer with the profile chain ID, signed over the reviewed hash', receipt?.status === '0x1' && tx.type === '0x2' && Number(tx.chainId) === cid && tx.to.toLowerCase() === to && BigInt(tx.value) === 25n * 10n ** 16n
      && proxy.log.filter((m) => m === 'eth_sendRawTransaction').length === 1, JSON.stringify({ type: tx.type, chainId: tx.chainId, status: receipt?.status }));
    ok('Send: the recipient holds exactly the amount', BigInt(await rpc(8540, 'eth_getBalance', [to, 'latest'])) === 25n * 10n ** 16n);
    const signed = Transaction.from({ type: 2, chainId: cid, nonce: Number(tx.nonce), maxPriorityFeePerGas: BigInt(tx.maxPriorityFeePerGas), maxFeePerGas: BigInt(tx.maxFeePerGas), gasLimit: BigInt(tx.gas), to: tx.to, value: BigInt(tx.value), data: '0x' });
    ok('Send: the signing hash shown in the review is the hash of the transaction that was included', signed.unsignedHash === review.rHash, review.rHash);
    await popup.click('#refresh');
    await wait(async () => /^included · devnet/.test(await text(popup, '#txs li')), 30000, 'popup list');
    ok('popup: the transfer is listed as included', (await text(popup, '#txs li')).includes('block'), await text(popup, '#txs li'));

    // Another network behind the same endpoint: refused, with the difference shown.
    proxy.genesisOverride = '0x' + '77'.repeat(32);
    const sentBefore = proxy.log.filter((m) => m === 'eth_sendRawTransaction').length;
    await popup.click('#refresh');
    await wait(async () => /^Wrong network/.test(await text(popup, '#netText')), 30000, 'popup wrong network');
    ok('popup: an endpoint with another genesis is "Wrong network" with the difference, and Send is disabled', (await text(popup, '#netDetail')).includes(`genesis hash: expected ${base.genesisHash.toLowerCase()}, the endpoint answered 0x${'77'.repeat(32)}`) && await popup.isDisabled('#send'), await text(popup, '#netDetail'));
    await send.click('#again'); await sleep(1500);
    await send.fill('#to', to); await send.fill('#amount', '0.1');
    await send.evaluate(() => { document.getElementById('review').disabled = false; }); // the page disables it; the worker is the boundary
    await send.click('#review');
    await wait(async () => /genesis/.test(await text(send, '#formNote')), 30000, 'send refused');
    ok('Send: refused on a genesis mismatch, with the difference, and nothing submitted', /endpoint genesis differs from profile \(genesis hash: expected 0x[0-9a-f]{64}, the endpoint answered 0x(77){32}\)/.test(await text(send, '#formNote')) && proxy.log.filter((m) => m === 'eth_sendRawTransaction').length === sentBefore, await text(send, '#formNote'));
    proxy.genesisOverride = null;

    // The built-in profile.
    await popup.selectOption('#network', 'pocol-public');
    await wait(async () => (await text(popup, '#netDetail')).includes('explorer.pocol.net'), 30000, 'built-in status');
    if (placeholder) { await wait(async () => /^Not connected/.test(await text(popup, '#netText')), 30000, 'placeholder status').catch(() => {});
      ok('popup: the built-in profile of a placeholder build is "Not connected", and Send is disabled', /^Not connected: this build pins a placeholder/.test(await text(popup, '#netText')) && await popup.isDisabled('#send'), await text(popup, '#netText')); }
    else ok('popup: the built-in profile is shown with its pinned endpoint (unreachable here: every host but 127.0.0.1 is blocked)', /explorer\.pocol\.net\/studio\/rpc/.test(await text(popup, '#netDetail')), await text(popup, '#netDetail'));

    // EXT-06, recorded, not asserted: does worker-src 'none' in the extension-page CSP stop a blob: worker or a packaged
    // worker script in an extension page of this Chrome? (Chrome for Testing 153.0.8010.12: no, so build.mjs does not add it.)
    // w.js, a packaged worker script, is written into these temporary test copies only (no release file starts a worker).
    writeFileSync(join(dist, 'w.js'), 'postMessage(1);\n');
    const csp = join(out, 'dist-worker-src'); cpSync(dist, csp, { recursive: true });
    const m = JSON.parse(readFileSync(join(csp, 'manifest.json'), 'utf8')); m.content_security_policy.extension_pages += "; worker-src 'none'"; writeFileSync(join(csp, 'manifest.json'), JSON.stringify(m, null, 2));
    const c = await openBrowser(csp, 'workersrc'); browsers.push(c);
    const withNone = await c.page('options.html');
    console.log(`INFO EXT-06 ${c.ctx.browser()?.version?.() ?? ''}: in options.html with the release CSP: blob: worker ${await tryWorker(options)}, packaged w.js ${await tryWorker(options, true)}; with worker-src 'none' added: blob: worker ${await tryWorker(withNone)}, packaged w.js ${await tryWorker(withNone, true)}`);
    await c.close(); browsers.pop();

    // EXT-08: the Studio-requested viewer tab waits for the click.
    proxy.log.length = 0;
    const viewer = await b.page(`viewer.html?confirm=studio#devnet/0x${'ab'.repeat(20)}/`);
    await viewer.waitForSelector('#gateOpen', { timeout: 15000 }); await sleep(1500);
    ok('EXT-08: a viewer tab opened for a Studio request waits for the click and reads nothing', proxy.log.length === 0 && /asked to open the website/.test(await text(viewer, '#gate')), JSON.stringify(proxy.log));
    await viewer.click('#gateOpen');
    await wait(async () => proxy.log.length > 0, 30000, 'viewer reads after the click');
    ok('EXT-08: after the click the viewer opens the site (it reads the network)', proxy.log.includes('eth_chainId'));

    // Restore the backup into a fresh browser profile: the same address.
    const d = await openBrowser(dist, 'restore'); browsers.push(d);
    const o2 = await d.page('options.html');
    ok('a fresh browser profile has no wallet', /No PoCol wallet/.test(await (async () => { await wait(async () => (await text(o2, '#acctState')) !== '…', 15000, 'state'); return text(o2, '#acctState'); })()));
    await o2.selectOption('#importKind', 'file'); await o2.setInputFiles('#walletFile', backupFile);
    await o2.fill('#filePw', PASSWORD); await o2.fill('#newPw', 'another-browser-pw'); await o2.fill('#newPw2', 'another-browser-pw');
    await o2.click('#acctSave');
    await wait(async () => (await text(o2, '#acctNote')).includes(address), 60000, 'restored');
    ok('EXT-04: the backup restores the same address in a fresh browser profile', (await text(o2, '#acctState')).includes(address), await text(o2, '#acctState'));
  } finally {
    for (const x of browsers) await x.close().catch(() => {});
    try { py(['stop', work]); } catch (e) { console.log('devnet stop: ' + e.message); }
    proxyServer.close();
    rmSync(out, { recursive: true, force: true });
    if (!failed) rmSync(work, { recursive: true, force: true }); else console.log('devnet kept at ' + work);
  }
}
main().then(() => { console.log(JSON.stringify({ result: failed ? 'FAIL' : 'PASS', passed: results.length - failed, failed })); process.exit(failed ? 1 : 0); },
  (e) => { console.log('ERROR ' + (e.stack || e)); process.exit(2); });
