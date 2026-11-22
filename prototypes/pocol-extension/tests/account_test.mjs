// The PoCol wallet (account.*, net.status, send.*, txs.*, connections.*; 2026-11-22, EXT-01..05): the real built
// service worker with mocked Chrome and a mocked JSON-RPC endpoint (component evidence, not real-browser evidence; the
// browser run is tests/wallet_ui_browser.mjs and the devnet run tests/e2e_wallet_devnet.mjs). Public test keys only.
import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { Transaction, Wallet, HDNodeWallet } from 'ethers';
import { build } from '../build.mjs';
import { installChromeMock, installFetchMock, loadWorker } from './chrome_mock.mjs';
import { siteFixture } from './site_fixture.mjs';
import { devnetSpecV3Hex } from './rp_fixture.mjs';

const dist = build().dist;
// A copy of the build that carries the D1 placeholder mark whatever the tree carries (before or after the identity
// switch), so the placeholder refusal is tested in both states of the tree.
const phRoot = mkdtempSync(join(tmpdir(), 'pocol-account-ph-'));
const distPh = build({ out: join(phRoot, 'dist') }).dist;
{ const f = join(distPh, 'lib', 'studio-network.js'), s = readFileSync(f, 'utf8'); if (!s.includes('export const NETWORK_PLACEHOLDER = true;')) writeFileSync(f, s + '\nexport const NETWORK_PLACEHOLDER = true;\n'); }
after(() => rmSync(phRoot, { recursive: true, force: true }));
const L = (m) => import(pathToFileURL(join(dist, 'lib', m)).href);
const A = await L('account.js'), K = await L('studio-keyfile.js'), H = await L('hex.js'), C = await L('crypto.js'), RP = await L('rp.js'), N = await L('studio-network.js');
// RecordStore schedules a late sweep at worker boot; release long timers when the suite ends.
const later = [], realTimeout = globalThis.setTimeout;
globalThis.setTimeout = (fn, ms, ...args) => { const t = realTimeout(fn, ms, ...args); if (ms >= 10000) later.push(t); return t; };
const intervals = [], realInterval = globalThis.setInterval;
globalThis.setInterval = (fn, ms, ...args) => { const t = realInterval(fn, ms, ...args); intervals.push(t); return t; };
after(() => { globalThis.setTimeout = realTimeout; globalThis.setInterval = realInterval; for (const t of later) clearTimeout(t); for (const t of intervals) clearInterval(t); });

const PW = 'account-test-password';
// Public test vectors: Hardhat's well-known first key and its BIP-39 phrase (never a real wallet).
const PHRASE = 'test test test test test test test test test test test junk';
const PHRASE_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80', PHRASE_ADDRESS = '0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266';
const KEY2 = '0x' + '11'.repeat(32), ADDRESS2 = new Wallet(KEY2).address.toLowerCase();
const TO = '0x' + '22'.repeat(20);
const spec = (() => { const pre = '0x' + devnetSpecV3Hex(); const b = H.fromHex(pre); return { pre, hash: H.toHex(C.keccak(b)), chainId: Number(RP.decodeSpec(b).chainId) }; })();
const profile = (name = 'devnet', endpoint = 'http://127.0.0.1:8599') => ({ name, chainId: spec.chainId, genesisPre: spec.pre, genesisHash: spec.hash, endpoint, trustLevel: 'LN' });

let n = 0;
/** A fresh worker with the devnet profile, an RPC endpoint mock and helpers. `env` is the node's state. */
async function world({ disclosure = true, extra = {}, from = dist } = {}) {
  const mock = installChromeMock(); const fx = await siteFixture(dist);
  const env = { chainId: spec.chainId, genesis: spec.hash, balance: 10n ** 21n, nonce: 0n, latestNonce: 0n, base: 10000000n, receipts: new Map(), sendError: null, height: 42, ts: Math.floor(Date.now() / 1000) - 5, down: false };
  const hex = (v) => '0x' + BigInt(v).toString(16);
  const log = installFetchMock({ ...fx.handlers,
    eth_chainId: () => (env.down ? { __raw: () => 'down' } : hex(env.chainId)), pocol_status: () => ({ genesisHash: env.genesis }),
    eth_getBlockByNumber: ([tag]) => (tag === 'latest' ? { number: hex(env.height), timestamp: hex(env.ts), baseFeePerGas: hex(env.base), gasUsed: '0x0', gasLimit: '0x7a1200', hash: '0x' + 'b1'.repeat(32) } : { number: tag, hash: '0x' + 'b2'.repeat(32) }),
    eth_getBalance: () => hex(env.balance), eth_getTransactionCount: ([, tag]) => hex(tag === 'pending' ? env.nonce : env.latestNonce),
    eth_maxPriorityFeePerGas: () => hex(1000000000n), eth_gasPrice: () => hex(env.base + 1000000000n), eth_feeHistory: () => ({ baseFeePerGas: [hex(env.base)] }),
    eth_estimateGas: () => '0x5208',
    eth_sendRawTransaction: ([raw]) => { if (env.sendError) return { __error: env.sendError }; env.sent = raw; return H.toHex(C.keccak(H.fromHex(raw))); },
    eth_getTransactionReceipt: ([h]) => env.receipts.get(h) || null,
    pocol_getLogs: () => [], ...extra });
  await loadWorker(from, 'acct' + ++n);
  const r = await mock.send({ type: 'profiles.add', profile: profile() }); assert.equal(r.ok, true, JSON.stringify(r));
  const page = (p, tab = 7) => (msg) => mock.send(msg, tab, p);
  const options = page('options.html'), popup = page('popup.html', null), send = page('send.html', 21);
  if (disclosure) assert.equal((await popup({ type: 'account.acceptDisclosure' })).ok, true);
  const count = (m) => log.filter((x) => x.method === m).length;
  return { mock, env, log, count, options, popup, send, fx };
}
const vaultOf = (w) => w.mock.storage.data.get('studio:wallet');

test('account helpers: keys, BIP-39 phrase at m/44\'/60\'/0\'/0/0, recipients and PCOL amounts', () => {
  assert.equal(A.PHRASE_PATH, "m/44'/60'/0'/0/0");
  assert.equal(H.toHex(A.phraseToSecret(PHRASE)), PHRASE_KEY);
  assert.equal(H.toHex(A.phraseToSecret('  Test TEST test test test test test test test test test   junk ')), PHRASE_KEY, 'case and spaces are normalised');
  assert.equal(HDNodeWallet.fromPhrase(PHRASE, '', A.PHRASE_PATH).address.toLowerCase(), PHRASE_ADDRESS);
  assert.throws(() => A.phraseToSecret(PHRASE.replace('junk', 'test')), /checksum/);
  assert.throws(() => A.phraseToSecret('test test test'), /12, 15, 18, 21 or 24 words/);
  assert.equal(H.toHex(A.parsePrivateKey(PHRASE_KEY.slice(2))), PHRASE_KEY);
  assert.throws(() => A.parsePrivateKey('0x' + '00'.repeat(32)), /Invalid wallet secret/);
  assert.throws(() => A.parsePrivateKey('0x1234'), /64 hexadecimal digits/);
  assert.equal(A.parseRecipient('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'), PHRASE_ADDRESS);
  assert.throws(() => A.parseRecipient('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92267'.replace('F4ce', 'f4ce')), /checksum/);
  assert.throws(() => A.parseRecipient('0x' + '00'.repeat(20)), /zero address/);
  assert.throws(() => A.parseRecipient('22'.repeat(20)), /0x and 40/);
  assert.equal(A.parsePcol('1.5'), 1500000000000000000n); assert.equal(A.parsePcol('0.000000000000000001'), 1n);
  assert.throws(() => A.parsePcol('0'), /more than 0/); assert.throws(() => A.parsePcol('1.0000000000000000001'), /18 decimals/); assert.throws(() => A.parsePcol('-1'), /PCOL/); assert.throws(() => A.parsePcol('1e3'), /PCOL/);
  assert.equal(A.formatPcol(1500000000000000000n), '1.5 PCOL'); assert.equal(A.formatPcol(1n), '0.000000000000000001 PCOL'); assert.equal(A.formatPcol(0n), '0 PCOL');
  assert.deepEqual(A.transferFees(10n, 3n, 100n), { maxPriority: 3n, maxFee: 23n }); assert.throws(() => A.transferFees(60n, 1n, 100n), /ceiling/);
  assert.equal(A.explorerAddressUrl('0xABCDEF0000000000000000000000000000000001'), 'https://explorer.pocol.net/#q=0xabcdef0000000000000000000000000000000001');
});

test('wallet operations come only from the extension\'s wallet pages; content scripts never reach them', async () => {
  const w = await world();
  for (const [msg, page] of [[{ type: 'account.create', password: PW }, 'popup.html'], [{ type: 'account.create', password: PW }, 'publisher.html'], [{ type: 'account.export', password: PW }, 'viewer.html'],
    [{ type: 'send.prepare', name: 'devnet', to: TO, amount: '1' }, 'options.html'], [{ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW }, undefined]]) {
    const r = await w.mock.send(msg, 7, page); assert.equal(r.ok, false, msg.type + ' from ' + page); assert.equal(r.error.code, 4100);
  }
  assert.equal(vaultOf(w), undefined, 'nothing was saved');
  // A content script (a page URL outside the extension) is routed to the Studio bridge, which has no wallet action.
  const listener = w.mock.listeners.message.at(-1);
  const r = await new Promise((resolve) => listener({ type: 'account.export', password: PW }, { id: chrome.runtime.id, url: 'https://studio.pocol.net/', tab: { id: 3 }, frameId: 0 }, resolve));
  assert.equal(r.ok, false); assert.match(r.error, /Unsupported Studio action/);
});

test('create saves the Studio vault format, returns its encrypted backup, and never replaces another wallet silently', async () => {
  const w = await world();
  assert.equal((await w.options({ type: 'account.create', password: 'short' })).ok, false);
  const c = await w.options({ type: 'account.create', password: PW }); assert.equal(c.ok, true, JSON.stringify(c));
  const v = vaultOf(w);
  assert.equal(v.schema, 'pocol-studio-vault-v1'); assert.equal(v.iter, 600000); assert.equal(v.address, c.address); assert.deepEqual(c.backup, v);
  assert.equal(JSON.stringify(c).includes('"secret"'), false);
  const key = await K.importKeyfile(new TextEncoder().encode(JSON.stringify(c.backup)), PW); assert.equal(H.toHex(C.addressOfSecret(key)), c.address);
  const st = await w.popup({ type: 'account.status' }); assert.equal(st.exists, true); assert.equal(st.unlocked, true); assert.equal(st.address, c.address);
  const again = await w.options({ type: 'account.create', password: PW }); assert.equal(again.ok, false); assert.equal(again.error.code, 'exists'); assert.equal(again.error.saved, c.address);
  assert.deepEqual(vaultOf(w), v, 'the saved wallet is unchanged');
  const replaced = await w.options({ type: 'account.create', password: PW, replace: true }); assert.equal(replaced.ok, true); assert.notEqual(replaced.address, c.address);
});

test('import from a private key, a recovery phrase, an encrypted wallet file and a raw 32-byte key file', async () => {
  const w = await world();
  const k = await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW }); assert.equal(k.ok, true, JSON.stringify(k)); assert.equal(k.address, ADDRESS2);
  const p = await w.options({ type: 'account.import', kind: 'phrase', phrase: PHRASE, password: PW, replace: true }); assert.equal(p.address, PHRASE_ADDRESS);
  assert.equal(JSON.stringify([...w.mock.storage.data]).includes('junk'), false, 'the phrase is not stored');
  assert.equal((await w.options({ type: 'account.import', kind: 'phrase', phrase: PHRASE.replace('junk', 'test'), password: PW, replace: true })).ok, false);
  // The Studio export file (pocol-studio-vault-v1) under its own password, saved again under the new password.
  const file = await K.exportKeyfile(H.fromHex(KEY2), 'file-password-1');
  const f = await w.options({ type: 'account.import', kind: 'file', fileBase64: btoa(JSON.stringify(file)), filePassword: 'file-password-1', password: PW, replace: true });
  assert.equal(f.ok, true, JSON.stringify(f)); assert.equal(f.address, ADDRESS2);
  await K.importKeyfile(new TextEncoder().encode(JSON.stringify(vaultOf(w))), PW);
  const wrong = await w.options({ type: 'account.import', kind: 'file', fileBase64: btoa(JSON.stringify(file)), filePassword: 'not-the-password', password: PW, replace: true }); assert.equal(wrong.ok, false); assert.match(wrong.error.message, /Could not unlock/);
  const raw = await w.options({ type: 'account.import', kind: 'file', fileBase64: btoa(String.fromCharCode(...H.fromHex(PHRASE_KEY))), password: PW, replace: true }); assert.equal(raw.address, PHRASE_ADDRESS);
});

test('EXT-04: a backup at any time after the password is entered again, restored into a fresh browser profile with the same address', async () => {
  const w = await world();
  const c = await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  await w.popup({ type: 'account.lock' });
  const bad = await w.options({ type: 'account.export', password: 'wrong-password-123' }); assert.equal(bad.ok, false); assert.equal(bad.error.code, 'password');
  const b = await w.options({ type: 'account.export', password: PW }); assert.equal(b.ok, true, JSON.stringify(b)); assert.equal(b.address, c.address); assert.deepEqual(b.backup, vaultOf(w));
  assert.equal((await w.popup({ type: 'account.status' })).unlocked, false, 'a backup does not unlock');
  // A new extension id starts with empty storage (Chrome Web Store move): restore the backup there.
  const fresh = await world();
  assert.equal((await fresh.popup({ type: 'account.status' })).exists, false);
  const r = await fresh.options({ type: 'account.import', kind: 'file', fileBase64: btoa(JSON.stringify(b.backup)), password: PW }); assert.equal(r.ok, true, JSON.stringify(r)); assert.equal(r.address, ADDRESS2);
});

test('unlock, lock, wrong passwords and the five-a-minute password gate', async () => {
  const w = await world();
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW }); await w.popup({ type: 'account.lock' });
  assert.equal((await w.popup({ type: 'account.status' })).unlocked, false);
  for (let i = 0; i < 5; i++) assert.equal((await w.popup({ type: 'account.unlock', password: 'wrong-' + i + '-password' })).error.code, 'password');
  const gated = await w.popup({ type: 'account.unlock', password: PW }); assert.equal(gated.ok, false); assert.equal(gated.error.code, 'rate');
  const realNow = Date.now; Date.now = () => realNow() + 61000;
  try { const u = await w.popup({ type: 'account.unlock', password: PW }); assert.equal(u.ok, true); assert.equal(u.address, ADDRESS2);
    assert.equal((await w.popup({ type: 'account.status' })).unlocked, true); } finally { Date.now = realNow; }
  await w.popup({ type: 'account.lock' }); assert.equal((await w.popup({ type: 'account.status' })).unlocked, false);
});

test('a vault written by a Studio or IDE page for another address locks the unlocked account', async () => {
  const w = await world();
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  await w.mock.storage.set({ 'studio:wallet': await K.exportKeyfile(H.fromHex(PHRASE_KEY), PW) });
  const st = await w.popup({ type: 'account.status' }); assert.equal(st.unlocked, false); assert.equal(st.address, PHRASE_ADDRESS);
});

test('the placeholder build makes no network request for the built-in profile and refuses to send', async () => {
  const w = await world({ from: distPh });
  assert.equal((await import(pathToFileURL(join(distPh, 'lib', 'studio-network.js')).href)).NETWORK_PLACEHOLDER, true, 'this build pins the D1 placeholder');
  assert.equal((await w.popup({ type: 'account.status' })).placeholderBuild, true);
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  const st = await w.popup({ type: 'net.status' }); assert.equal(st.ok, true); assert.equal(st.placeholder, true); assert.equal(st.reachable, null); assert.equal(st.name, N.NETWORK.name);
  const bal = await w.popup({ type: 'account.balance' }); assert.equal(bal.placeholder, true); assert.equal(bal.address, ADDRESS2);
  const s = await w.send({ type: 'send.prepare', to: TO, amount: '1' }); assert.equal(s.ok, false); assert.equal(s.error.code, 'placeholder');
  assert.equal(w.log.filter((x) => x.url === N.NETWORK.endpoint).length, 0, 'nothing went to the pinned endpoint');
});

test('the first-run disclosure must be accepted before any wallet network request', async () => {
  const w = await world({ disclosure: false });
  assert.equal((await w.popup({ type: 'account.status' })).disclosureAccepted, false);
  const st = await w.popup({ type: 'net.status', name: 'devnet' }); assert.equal(st.ok, false); assert.equal(st.error.code, 'disclosure');
  assert.equal(w.log.length, 0);
  await w.popup({ type: 'account.acceptDisclosure' });
  const ok = await w.popup({ type: 'net.status', name: 'devnet' }); assert.equal(ok.reachable, true);
});

test('network status: chain ID and genesis checked against the profile, head height and age, unreachable endpoint', async () => {
  const w = await world();
  const st = await w.popup({ type: 'net.status', name: 'devnet' });
  assert.equal(st.ok, true); assert.equal(st.chainId, spec.chainId); assert.equal(st.endpoint, 'http://127.0.0.1:8599');
  assert.equal(st.chainOk, true); assert.equal(st.genesisOk, true); assert.equal(st.head.height, 42); assert.ok(st.head.ageSec >= 4 && st.head.ageSec < 30, st.head.ageSec);
  w.env.genesis = '0x' + '99'.repeat(32);
  const bad = await w.popup({ type: 'net.status', name: 'devnet' }); assert.equal(bad.genesisOk, false); assert.equal(bad.head, null);
  w.env.genesis = spec.hash; w.env.chainId = spec.chainId + 1;
  const other = await w.popup({ type: 'net.status', name: 'devnet' }); assert.equal(other.chainOk, false);
  w.env.down = true;
  const down = await w.popup({ type: 'net.status', name: 'devnet' }); assert.equal(down.reachable, false);
});

async function prepared(w, extra = {}) {
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW, replace: true });
  const p = await w.send({ type: 'send.prepare', name: 'devnet', to: TO, amount: '1.25', ...extra }); assert.equal(p.ok, true, JSON.stringify(p));
  return p;
}
test('send: the review shows network, chain ID, recipient, amount, fee and total; confirming signs exactly that (EIP-1559, the profile chain ID) and tracks the receipt', async () => {
  const w = await world();
  const p = await prepared(w);
  assert.equal(p.network, 'devnet'); assert.equal(p.chainId, spec.chainId); assert.equal(p.from, ADDRESS2); assert.equal(p.to, TO); assert.equal(p.valuePcol, '1.25 PCOL');
  assert.equal(p.gas, '21000'); assert.equal(p.maxPriorityFeePerGas, '1000000000'); assert.equal(p.maxFeePerGas, String(2n * 10000000n + 1000000000n));
  assert.equal(p.maxFee, String(21000n * (2n * 10000000n + 1000000000n))); assert.equal(BigInt(p.total), 1250000000000000000n + BigInt(p.maxFee)); assert.equal(p.nonce, '0');
  assert.equal(w.count('eth_sendRawTransaction'), 0, 'nothing is signed or sent before the confirmation');
  const c = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(c.ok, true, JSON.stringify(c)); assert.equal(c.status, 'pending');
  assert.equal(w.count('eth_sendRawTransaction'), 1);
  const tx = Transaction.from(w.env.sent);
  assert.equal(tx.type, 2); assert.equal(Number(tx.chainId), spec.chainId); assert.equal(tx.from.toLowerCase(), ADDRESS2); assert.equal(tx.to.toLowerCase(), TO);
  assert.equal(tx.value, 1250000000000000000n); assert.equal(tx.nonce, 0); assert.equal(tx.gasLimit, 21000n); assert.equal(tx.maxFeePerGas, BigInt(p.maxFeePerGas)); assert.equal(tx.data, '0x');
  assert.equal(tx.unsignedHash, p.signingHash); assert.equal(tx.hash, c.hash);
  assert.equal((await w.send({ type: 'send.confirm', id: p.id })).error.code, 'expired', 'a review is used once');
  let list = (await w.popup({ type: 'txs.list' })).transactions; assert.equal(list.length, 1); assert.equal(list[0].status, 'pending'); assert.equal(list[0].hash, c.hash); assert.equal(list[0].explorer, null, 'no explorer link for a custom network');
  w.env.receipts.set(c.hash, { transactionHash: c.hash, status: '0x1', blockNumber: '0x2b', blockHash: '0x' + 'b2'.repeat(32), gasUsed: '0x5208' });
  list = (await w.popup({ type: 'txs.refresh' })).transactions; assert.equal(list[0].status, 'included'); assert.equal(list[0].blockNumber, 43);
});

test('send refuses: a review from another tab, an expired or cancelled review, a moved nonce, a changed network, missing funds, a bad recipient', async () => {
  const w = await world();
  let p = await prepared(w);
  assert.equal((await w.mock.send({ type: 'send.confirm', id: p.id }, 22, 'send.html')).error.code, 'expired', 'another tab');
  p = await prepared(w); const realNow = Date.now; Date.now = () => realNow() + 121000;
  try { assert.equal((await w.send({ type: 'send.confirm', id: p.id })).error.code, 'expired'); } finally { Date.now = realNow; }
  p = await prepared(w); await w.popup({ type: 'account.lock' }); assert.equal((await w.send({ type: 'send.confirm', id: p.id })).error.code, 'expired', 'locking clears the reviews');
  p = await prepared(w); w.env.nonce = 1n; assert.equal((await w.send({ type: 'send.confirm', id: p.id })).error.code, 'nonce'); w.env.nonce = 0n;
  p = await prepared(w); w.env.genesis = '0x' + '99'.repeat(32); assert.equal((await w.send({ type: 'send.confirm', id: p.id })).error.code, 4901); w.env.genesis = spec.hash;
  p = await prepared(w); await w.options({ type: 'profiles.add', profile: profile('second', 'http://127.0.0.1:8598') });
  assert.equal((await w.send({ type: 'send.confirm', id: p.id })).ok, false, 'a profile change between review and confirmation');
  w.env.balance = 10n ** 18n; const poor = await w.send({ type: 'send.prepare', name: 'devnet', to: TO, amount: '1' }); assert.equal(poor.error.code, 'funds'); w.env.balance = 10n ** 21n;
  assert.match((await w.send({ type: 'send.prepare', name: 'devnet', to: '0x1234', amount: '1' })).error.message, /recipient/);
  await w.popup({ type: 'account.lock' }); assert.equal((await w.send({ type: 'send.prepare', name: 'devnet', to: TO, amount: '1' })).error.code, 4100);
  assert.equal(w.count('eth_sendRawTransaction'), 0, 'nothing was sent');
});

test('send: a node rejection, a lost answer and a dropped transaction are recorded as such, never as included', async () => {
  const w = await world();
  let p = await prepared(w); w.env.sendError = { code: -32000, message: 'nonce too low' };
  const r = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(r.ok, false); assert.equal(r.error.code, 'rejected');
  w.env.sendError = null;
  p = await prepared(w, { amount: '2' });
  const c = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(c.status, 'pending');
  w.env.latestNonce = 1n; // the nonce was used by another transaction, and no receipt names this one
  const list = (await w.popup({ type: 'txs.refresh' })).transactions;
  assert.deepEqual(list.map((x) => x.status), ['dropped', 'rejected']);
});

test('EXT-03: a site\'s account grant is listed and revocable in Settings; after revoking it gets no account until it asks again', async () => {
  const w = await world();
  await w.mock.send({ type: 'wallet.create', password: PW, seedHex: '0x' + '42'.repeat(32) });
  const op = await w.mock.send({ type: 'open', name: 'devnet', site: w.fx.site, path: '/' }); assert.equal(op.ok, true, JSON.stringify(op));
  const port = w.mock.connect('bridge:' + op.sessionId);
  const req = (id, method) => port.post({ seq: id, frame: 1, text: JSON.stringify({ id, kind: 'wallet_req', payload: { method, params: [] } }) });
  req(1, 'eth_requestAccounts'); const ask = await port.next((m) => m.ask !== undefined && m.q.type === 'confirmAccounts'); port.post({ answer: ask.ask, ok: true });
  const granted = (await port.next((m) => m.seq === 1)).reply.result[0];
  const list = await w.options({ type: 'connections.list' }); assert.equal(list.connections.length, 1); assert.equal(list.connections[0].site, w.fx.site); assert.equal(list.connections[0].address, granted);
  assert.equal((await w.popup({ type: 'connections.list' })).ok, false, 'only the settings page manages connections');
  assert.equal((await w.options({ type: 'connections.revoke', id: op.sessionId })).ok, true);
  req(2, 'eth_accounts'); assert.deepEqual((await port.next((m) => m.seq === 2)).reply.result, []);
  assert.equal((await w.options({ type: 'connections.list' })).connections.length, 0);
  port.close();
});

test('lib/identity.js: the refusal of another network names both values that differ', async () => {
  const I = await L('identity.js'), want = { chainId: 777, genesisHash: '0x' + 'ab'.repeat(32) };
  assert.equal(I.identityDifference(want, '0x309', '0x' + 'AB'.repeat(32)), null, 'same chain ID (hex answer) and genesis (any case)');
  assert.deepEqual(I.identityDifference(want, '0x30a', want.genesisHash), [{ field: 'chainId', expected: '777', got: '778' }]);
  assert.deepEqual(I.identityDifference(want, '0x309', '0x' + 'cd'.repeat(32)), [{ field: 'genesisHash', expected: want.genesisHash, got: '0x' + 'cd'.repeat(32) }]);
  assert.deepEqual(I.identityDifference(want, 'nonsense', undefined).map((d) => d.got), [null, null], 'a malformed answer never matches');
  assert.equal(I.describeDifference(I.identityDifference(want, '0x1', want.genesisHash)), 'chain ID: expected 777, the endpoint answered 1');
  assert.throws(() => I.assertIdentity(want, '0x1', '0x' + 'cd'.repeat(32), 'Blocked'), (e) => e.code === 4901 && /^Blocked \(chain ID: expected 777, the endpoint answered 1; genesis hash: expected 0xabab.*, the endpoint answered 0xcdcd/.test(e.message));
});

test('the wallet shows the difference when the endpoint is another network, and refuses to prepare a transfer', async () => {
  const w = await world();
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  w.env.chainId = spec.chainId + 5;
  const st = await w.popup({ type: 'net.status', name: 'devnet' });
  assert.equal(st.chainOk, false); assert.match(st.difference, new RegExp(`chain ID: expected ${spec.chainId}, the endpoint answered ${spec.chainId + 5}`));
  const p = await w.send({ type: 'send.prepare', name: 'devnet', to: TO, amount: '1' });
  assert.equal(p.ok, false); assert.equal(p.error.code, 4901); assert.match(p.error.message, new RegExp(`expected ${spec.chainId}, the endpoint answered ${spec.chainId + 5}`));
  w.env.chainId = spec.chainId; w.env.genesis = '0x' + '99'.repeat(32);
  const g = await w.send({ type: 'send.prepare', name: 'devnet', to: TO, amount: '1' });
  assert.equal(g.error.code, 4901); assert.match(g.error.message, new RegExp(`genesis hash: expected ${spec.hash}, the endpoint answered 0x${'99'.repeat(32)}`));
  assert.equal((await w.popup({ type: 'net.status', name: 'devnet' })).endpointGenesisHash, '0x' + '99'.repeat(32));
  assert.equal(w.count('eth_sendRawTransaction'), 0);
});

test('EXT-08: a Studio page\'s openSite opens the viewer on a confirmation, never on the site directly', async () => {
  const w = await world();
  const listener = w.mock.listeners.message.at(-1);
  const r = await new Promise((resolve) => listener({ type: 'studio.openSite', payload: { site: '0x' + 'ab'.repeat(20) } }, { id: chrome.runtime.id, url: 'https://studio.pocol.net/editor', tab: { id: 3 }, frameId: 0 }, resolve));
  assert.equal(r.ok, true, JSON.stringify(r));
  const url = w.mock.tabsCreated.at(-1).url;
  assert.equal(url, chrome.runtime.getURL('viewer.html') + '?confirm=studio#' + N.NETWORK.name + '/0x' + 'ab'.repeat(20) + '/');
  // The viewer waits for the user's own click before it fetches the site (src/viewer.js gate()).
  const viewer = readFileSync(join(dist, 'viewer.js'), 'utf8');
  assert.match(viewer, /get\('confirm'\) !== 'studio'/); assert.match(viewer, /if \(!e\.isTrusted\) return;/); assert.match(viewer, /gate\(\)\.then\(start\)/);
});
