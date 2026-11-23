// The PoCol wallet (account.*, net.status, send.*, txs.*, connections.*; 2026-11-22, EXT-01..05): the real built
// service worker with mocked Chrome and a mocked JSON-RPC endpoint (component evidence, not real-browser evidence; the
// real-browser run against a local pocold devnet is tests/wallet_ui_browser.mjs). Public test keys only.
import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import vm from 'node:vm';
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
    eth_getBalance: () => hex(env.balance),
    // env.nonceAt(height), when set, answers a read at a block number (the nonce at that depth); else the latest nonce.
    eth_getTransactionCount: ([, tag]) => { if (tag === 'pending') return hex(env.nonce); if (env.nonceAt && /^0x/.test(tag)) { const v = env.nonceAt(Number(tag)); return typeof v === 'object' ? v : hex(v); } return hex(env.latestNonce); },
    eth_maxPriorityFeePerGas: () => hex(1000000000n), eth_gasPrice: () => hex(env.base + 1000000000n), eth_feeHistory: () => ({ baseFeePerGas: [hex(env.base)] }),
    eth_estimateGas: () => '0x5208',
    // env.afterSend(), when set, runs once the node has the transaction (before its answer is returned).
    eth_sendRawTransaction: ([raw]) => { if (env.sendError) return { __error: env.sendError }; env.sent = raw; env.afterSend?.(); return H.toHex(C.keccak(H.fromHex(raw))); },
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

test('the first-run disclosure also gates the balance, Send prepare and the receipt refresh (each makes no request before it)', async () => {
  const w = await world({ disclosure: false });
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  // An open transfer in the history, so a refresh without the gate would read its receipt.
  await w.mock.storage.set({ 'wallet:history': [{ hash: '0x' + '33'.repeat(32), network: 'devnet', chainId: spec.chainId, genesisHash: spec.hash, from: ADDRESS2, to: TO, value: '1', nonce: '0', status: 'pending' }] });
  for (const [msg, page] of [[{ type: 'account.balance', name: 'devnet' }, w.popup], [{ type: 'send.prepare', name: 'devnet', to: TO, amount: '1' }, w.send], [{ type: 'txs.refresh' }, w.popup]]) {
    const r = await page(msg); assert.equal(r.ok, false, msg.type); assert.equal(r.error.code, 'disclosure', msg.type);
    assert.equal(w.log.length, 0, msg.type + ' sent nothing');
  }
  await w.popup({ type: 'account.acceptDisclosure' });
  assert.equal((await w.popup({ type: 'account.balance', name: 'devnet' })).pcol, '1000 PCOL');
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

test('lib/account.js sendRefused: only an answer that proves a refusal is final', () => {
  // The PoCol gateway's answer to a broadcast it could not complete (deploy/studio/pocol_studio_gateway.py call).
  assert.equal(A.sendRefused({ code: -32000, message: 'Node unavailable or network identity mismatch; check transaction hash before resubmitting' }), false);
  assert.equal(A.sendRefused({ code: -32000, message: 'Node unavailable or network identity mismatch' }), false);
  // pocold node.rs: POOL_PARKED holds a valid transaction for the next block; the other -32016 answers pool nothing.
  assert.equal(A.sendRefused({ code: -32016, message: '-32016 pool full (gas): held for the next block' }), false);
  assert.equal(A.sendRefused({ code: -32016, message: '-32016 pool full (sender gas)' }), true);
  assert.equal(A.sendRefused({ code: -32021, message: 'busy' }), true); assert.equal(A.sendRefused({ code: -32021, message: 'Studio gateway busy; retry shortly' }), true);
  assert.equal(A.sendRefused({ code: -32020, message: 'compute deadline' }), true); assert.equal(A.sendRefused({ code: -32014, message: '-32014 oversized' }), true);
  assert.equal(A.sendRefused({ code: -32000, message: 'nonce too low' }), true);
  assert.equal(A.sendRefused({ code: -32000, message: '-32000 Transaction(NonceTooLow { tx: 0, state: 1 })' }), true);
  assert.equal(A.sendRefused({ code: -32000, message: "-32000 insufficient funds for gas * price + value with the sender's pooled and parked transactions" }), true);
  assert.equal(A.sendRefused({ code: -32000, message: '-32000 replacement transaction underpriced' }), true);
  assert.equal(A.sendRefused({ code: -32000, message: 'something else went wrong' }), false, 'an unknown -32000 is ambiguous');
  assert.equal(A.sendRefused({ code: -32603, message: 'internal error' }), false); assert.equal(A.sendRefused({ code: '-32000', message: 'nonce too low' }), false); assert.equal(A.sendRefused(null), false);
});

/** Runs the built send.js against the worker with a minimal DOM (no browser) through one transfer. `relay(m, forward)`
 *  carries each page message to the worker (default: forward(m)), so a test can lose an answer or act before one.
 *  Returns the page's elements, a wait helper and stop() (the page's timers); sendPageResult gives its result text. */
async function sendPage(w, amount, { relay = (m, forward) => forward(m) } = {}) {
  const els = new Map(), el = (id) => { if (!els.has(id)) els.set(id, { id, textContent: '', hidden: false, disabled: false, checked: false, value: '', className: '', href: '' }); return els.get(id); };
  const timers = [], stop = () => { for (const t of timers) clearInterval(t); };
  const ctx = vm.createContext({ document: { getElementById: el }, location: { hash: '#devnet' }, window: { close() {} },
    chrome: { runtime: { sendMessage: (m) => relay(m, (x) => w.mock.send(x, 21, 'send.html')) } }, setInterval: (f, ms) => { const t = realInterval(f, ms); timers.push(t); return t; }, clearInterval });
  vm.runInContext(readFileSync(join(dist, 'send.js'), 'utf8'), ctx);
  const until = async (f, what) => { for (let i = 0; i < 300; i++) { if (f()) return; await new Promise((r) => realTimeout(r, 10)); } throw new Error('send page: ' + what); };
  try {
    await until(() => el('from').textContent !== '' && /^Connected/.test(el('netText').textContent), 'ready');
    assert.equal(el('review').disabled, false, el('netText').textContent);
    el('to').value = TO; el('amount').value = amount; await el('review').onclick();
    el('consent').checked = true; el('consent').onchange(); await el('confirm').onclick({ isTrusted: true });
    await new Promise((r) => realTimeout(r, 100)); // the first receipt check of track()
  } catch (e) { stop(); throw e; }
  return { el, until, stop };
}
async function sendPageResult(w, amount, opts) {
  const page = await sendPage(w, amount, opts);
  try { return page.el('resultText').textContent; } finally { page.stop(); }
}

test('send: the gateway\'s ambiguous error after a broadcast is "unknown", never "Not sent", and its receipt makes it included', async () => {
  const w = await world();
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  // The node got the transaction, but the gateway's answer is its timed-out broadcast error.
  w.env.sendError = { code: -32000, message: 'Node unavailable or network identity mismatch; check transaction hash before resubmitting' };
  const page = await sendPageResult(w, '1.5');
  assert.doesNotMatch(page, /Not sent/); assert.match(page, /may have been sent/); assert.match(page, /check transaction hash before resubmitting/);
  let list = (await w.popup({ type: 'txs.list' })).transactions; assert.equal(list.length, 1); assert.equal(list[0].status, 'unknown');
  const hash = list[0].hash;
  // Included after all: the receipt is read and the entry becomes included.
  w.env.receipts.set(hash, { transactionHash: hash, status: '0x1', blockNumber: '0x2b', blockHash: '0x' + 'b2'.repeat(32), gasUsed: '0x5208' }); w.env.nonce = 1n; w.env.latestNonce = 1n;
  const reads = w.count('eth_getTransactionReceipt');
  list = (await w.popup({ type: 'txs.refresh' })).transactions; assert.equal(list[0].status, 'included'); assert.equal(list[0].blockNumber, 43); assert.equal(list[0].note, undefined);
  assert.ok(w.count('eth_getTransactionReceipt') > reads, 'the receipt was read');
  // A refusal the node proves is still "Not sent".
  w.env.sendError = { code: -32000, message: 'nonce too low' };
  assert.match(await sendPageResult(w, '1'), /^Not sent: The node did not accept the transaction: nonce too low/);
});

// Codex review (PR #114, send.js "Not sent" branch): the page's answer can be lost after the worker broadcast the
// transfer (the message channel closes; chrome.runtime.sendMessage rejects), and a storage failure while the worker
// records the node's answer is an error after the broadcast. Neither proves that nothing was sent: the page shows an
// unknown outcome, offers no immediate resend, and reconciles with the worker's transfer history (txs.refresh).
// "Not sent" stays for refusals the worker proves before the broadcast and for refusals the node proves.
const LOST = 'The message port closed before a response was received.';
test('Codex review: a lost answer to send.confirm after the broadcast is an unknown outcome, reconciled with the history, never "Not sent"', async () => {
  const w = await world();
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  const relay = async (m, forward) => { const r = await forward(m); if (m.type === 'send.confirm') throw new Error(LOST); return r; };
  const page = await sendPage(w, '1.5', { relay });
  try {
    const el = page.el;
    assert.equal(w.count('eth_sendRawTransaction'), 1, 'the worker broadcast the transfer');
    const [entry] = (await w.popup({ type: 'txs.list' })).transactions; assert.equal(entry.status, 'pending');
    // The page found the transfer in the history by its sender, chain, nonce, recipient and amount.
    await page.until(() => el('resultHash').textContent === entry.hash, 'the hash from the history');
    assert.doesNotMatch(el('resultText').textContent, /Not sent/);
    assert.match(el('resultText').textContent, /^Sent to the network/, 'the history shows it was sent');
    // Its receipt arrives: the page says included.
    w.env.receipts.set(entry.hash, { transactionHash: entry.hash, status: '0x1', blockNumber: '0x2b', blockHash: '0x' + 'b2'.repeat(32), gasUsed: '0x5208' }); w.env.nonce = 1n; w.env.latestNonce = 1n;
    await page.until(() => /^Included in block 43/.test(el('resultText').textContent), 'included');
    assert.equal(el('again').disabled, false);
  } finally { page.stop(); }
  // While the history says nothing yet (the entry is still being recorded), the outcome stays unknown and no
  // immediate resend is offered.
  const w2 = await world();
  await w2.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  let quiet = false;
  const relay2 = async (m, forward) => { if (quiet && m.type === 'txs.refresh') return { ok: true, transactions: [] }; const r = await forward(m); if (m.type === 'send.confirm') { quiet = true; throw new Error(LOST); } return r; };
  const page2 = await sendPage(w2, '1.5', { relay: relay2 });
  try {
    const text = page2.el('resultText').textContent;
    assert.doesNotMatch(text, /Not sent/); assert.match(text, /may have been sent/); assert.match(text, /Do not send it again until its status below is known/);
    assert.equal(page2.el('again').disabled, true, 'no immediate resend while the outcome is unknown');
    quiet = false; // the history answers: it was sent
    await page2.until(() => /^Sent to the network/.test(page2.el('resultText').textContent), 'reconciled');
    assert.equal(page2.el('again').disabled, false);
  } finally { page2.stop(); }
});
test('Codex review: a history write that fails after the broadcast is "sent-unknown" from the worker and an unknown outcome on the page', async () => {
  const w = await world();
  const p = await prepared(w);
  w.env.afterSend = () => { w.env.afterSend = null; w.mock.storage.failSets = 1; }; // the write after the node's answer fails
  const c = await w.send({ type: 'send.confirm', id: p.id });
  assert.equal(w.count('eth_sendRawTransaction'), 1);
  assert.equal(c.ok, false, JSON.stringify(c)); assert.equal(c.error.code, 'sent-unknown'); assert.equal(c.error.outcome, 'unknown');
  const [entry] = (await w.popup({ type: 'txs.list' })).transactions;
  assert.equal(c.error.hash, entry.hash); assert.equal(entry.status, 'sending', 'the entry written before the broadcast stays open');
  w.env.receipts.set(entry.hash, { transactionHash: entry.hash, status: '0x1', blockNumber: '0x2b', blockHash: '0x' + 'b2'.repeat(32), gasUsed: '0x5208' });
  assert.equal((await w.popup({ type: 'txs.refresh' })).transactions[0].status, 'included', 'the refresh follows it');
  // The page: unknown, its hash shown, no immediate resend.
  const w2 = await world();
  await w2.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  w2.env.afterSend = () => { w2.env.afterSend = null; w2.mock.storage.failSets = 1; };
  let quiet = false;
  const relay = async (m, forward) => { if (quiet && m.type === 'txs.refresh') return { ok: true, transactions: [] }; const r = await forward(m); if (m.type === 'send.confirm') quiet = true; return r; };
  const page = await sendPage(w2, '1.5', { relay });
  try {
    const text = page.el('resultText').textContent;
    assert.doesNotMatch(text, /Not sent/); assert.match(text, /may have been sent/);
    assert.equal(page.el('again').disabled, true);
    const [e2] = (await w2.popup({ type: 'txs.list' })).transactions; assert.equal(page.el('resultHash').textContent, e2.hash);
  } finally { page.stop(); }
});
test('Codex review: refusals proven before the broadcast are still "Not sent", on the page and as the worker\'s outcome', async () => {
  for (const [what, before, code] of [['nonce', (w) => { w.env.nonce = 1n; }, 'nonce'], ['network', (w) => { w.env.genesis = '0x' + '99'.repeat(32); }, 4901],
    ['lock', (w) => w.popup({ type: 'account.lock' }), 'expired']]) {
    const w = await world();
    await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
    let answer = null;
    const relay = async (m, forward) => { if (m.type === 'send.confirm') { await before(w); answer = await forward(m); return answer; } return forward(m); };
    const text = await sendPageResult(w, '1', { relay });
    assert.equal(answer.ok, false, what); assert.equal(answer.error.code, code, what); assert.equal(answer.error.outcome, 'not-sent', what);
    assert.match(text, /^Not sent: /, what);
    assert.equal(w.count('eth_sendRawTransaction'), 0, what + ': nothing was sent');
  }
  // Stopped before the request started (a Lock while the entry is recorded): the worker's outcome is not-sent.
  const w = await world();
  const p = await prepared(w);
  w.mock.storage.holdSets = 1;
  const pending = w.send({ type: 'send.confirm', id: p.id });
  for (let i = 0; i < 1000 && !w.mock.storage.held.length; i++) await new Promise((r) => realTimeout(r, 2));
  assert.equal((await w.popup({ type: 'account.lock' })).ok, true); w.mock.storage.release();
  const c = await pending; assert.equal(c.error.code, 4100); assert.equal(c.error.outcome, 'not-sent');
  // A history write that fails before the broadcast: nothing was sent.
  const w3 = await world();
  const p3 = await prepared(w3); w3.mock.storage.failSets = 1;
  const c3 = await w3.send({ type: 'send.confirm', id: p3.id });
  assert.equal(c3.ok, false); assert.equal(c3.error.outcome, 'not-sent'); assert.equal(w3.count('eth_sendRawTransaction'), 0);
  // A refusal the node proves.
  const w4 = await world();
  const p4 = await prepared(w4); w4.env.sendError = { code: -32000, message: 'nonce too low' };
  const c4 = await w4.send({ type: 'send.confirm', id: p4.id }); assert.equal(c4.error.code, 'rejected'); assert.equal(c4.error.outcome, 'not-sent');
});
test('send: a transaction the node parks for the next block (-32016 held) is unknown, not rejected', async () => {
  const w = await world();
  const p = await prepared(w); w.env.sendError = { code: -32016, message: '-32016 pool full (gas): held for the next block' };
  const c = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(c.ok, true, JSON.stringify(c)); assert.equal(c.status, 'unknown');
  assert.equal((await w.popup({ type: 'txs.list' })).transactions[0].status, 'unknown');
});

test('history: an included transfer is re-read until it is settled, and goes back to pending when a reorganization removes it', async () => {
  const w = await world();
  const p = await prepared(w), c = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(c.status, 'pending');
  const rc = (block, hashByte = 'b2') => ({ transactionHash: c.hash, status: '0x1', blockNumber: block, blockHash: '0x' + hashByte.repeat(32), gasUsed: '0x5208' });
  w.env.receipts.set(c.hash, rc('0x2b'));
  let t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'included'); assert.equal(t.blockNumber, 43); assert.equal(t.settled, false);
  // A reorganization: the receipt is gone.
  w.env.receipts.delete(c.hash); let reads = w.count('eth_getTransactionReceipt');
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0];
  assert.ok(w.count('eth_getTransactionReceipt') > reads, 'an included entry is read again'); assert.equal(t.status, 'pending'); assert.equal(t.blockNumber, undefined); assert.match(t.note, /no longer on the canonical chain/);
  // A receipt whose block is not the canonical one at that height is not included either.
  w.env.receipts.set(c.hash, rc('0x2c', 'ee'));
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'pending');
  // Included again in another block; then SETTLE_DEPTH blocks deep it is settled and no longer read.
  w.env.receipts.set(c.hash, rc('0x2c'));
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'included'); assert.equal(t.blockNumber, 44); assert.equal(t.note, undefined);
  w.env.receipts.set(c.hash, { ...rc('0x2c'), status: '0x0' });
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'failed', 'a changed receipt is followed');
  w.env.receipts.set(c.hash, rc('0x2c')); w.env.height = 44 + 63;
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.settled, false, '63 blocks deep');
  w.env.height = 44 + 64;
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'included'); assert.equal(t.settled, true);
  reads = w.count('eth_getTransactionReceipt'); w.env.receipts.delete(c.hash);
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(w.count('eth_getTransactionReceipt'), reads, 'a settled entry is not read'); assert.equal(t.status, 'included');
});

// Codex review (PR #114): a transfer called dropped because another transaction used its nonce near the head stays
// under observation; a reorganization that removes the other transaction can still let this one be included.
test('history: a dropped transfer is read again, and goes back to pending, then included, when a reorganization removes the transaction that used its nonce', async () => {
  const w = await world();
  const p = await prepared(w), c = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(c.status, 'pending');
  w.env.latestNonce = 1n; // a same-nonce replacement is included at the head (block 42)
  let t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'dropped'); assert.notEqual(t.settled, true, 'not final near the head');
  // A reorganization removes the replacement: the nonce is free again.
  w.env.latestNonce = 0n; let reads = w.count('eth_getTransactionReceipt');
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0];
  assert.ok(w.count('eth_getTransactionReceipt') > reads, 'a dropped entry is read again');
  assert.equal(t.status, 'pending'); assert.match(t.note, /reorganization/);
  // The original is then included.
  w.env.receipts.set(c.hash, { transactionHash: c.hash, status: '0x1', blockNumber: '0x2b', blockHash: '0x' + 'b2'.repeat(32), gasUsed: '0x5208' }); w.env.latestNonce = 1n; w.env.height = 43;
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'included'); assert.equal(t.blockNumber, 43); assert.equal(t.note, undefined); assert.equal(t.droppedAt, undefined);
  // A dropped entry whose receipt appears directly (the replacement reorganized out between two refreshes) is included too.
  const p2 = await prepared(w, { amount: '2' }), c2 = await w.send({ type: 'send.confirm', id: p2.id }); w.env.latestNonce = 2n;
  t = (await w.popup({ type: 'txs.refresh' })).transactions.find((x) => x.hash === c2.hash); assert.equal(t.status, 'dropped');
  w.env.receipts.set(c2.hash, { transactionHash: c2.hash, status: '0x1', blockNumber: '0x2c', blockHash: '0x' + 'b2'.repeat(32), gasUsed: '0x5208' }); w.env.height = 44;
  t = (await w.popup({ type: 'txs.refresh' })).transactions.find((x) => x.hash === c2.hash); assert.equal(t.status, 'included'); assert.equal(t.blockNumber, 44);
});

test('history: a dropped transfer becomes final once the nonce it lost is used SETTLE_DEPTH blocks below the head, and is then no longer read', async () => {
  const w = await world();
  const p = await prepared(w), c = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(c.status, 'pending');
  w.env.latestNonce = 1n; w.env.nonceAt = (h) => (h >= 42 ? 1n : 0n); // the replacement is in block 42
  let t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'dropped');
  w.env.height = 42 + 63; let reads = w.count('eth_getTransactionReceipt');
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.ok(w.count('eth_getTransactionReceipt') > reads, 'read again while the replacement is 63 blocks deep');
  assert.equal(t.status, 'dropped'); assert.notEqual(t.settled, true);
  w.env.height = 42 + 64;
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'dropped'); assert.equal(t.settled, true);
  reads = w.count('eth_getTransactionReceipt'); const nonces = w.count('eth_getTransactionCount'); w.env.latestNonce = 0n;
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0];
  assert.equal(w.count('eth_getTransactionReceipt'), reads, 'a final dropped entry is not read'); assert.equal(w.count('eth_getTransactionCount'), nonces); assert.equal(t.status, 'dropped');
});

test('history: a dropped transfer is watched for a bounded window when the node cannot answer the nonce at settlement depth', async () => {
  const w = await world();
  const p = await prepared(w), c = await w.send({ type: 'send.confirm', id: p.id }); assert.equal(c.status, 'pending');
  w.env.latestNonce = 1n; w.env.nonceAt = () => ({ __error: { code: -32017, message: 'state outside the retained trie window (K_hist)' } });
  let t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'dropped');
  w.env.height = 42 + 255;
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'dropped'); assert.notEqual(t.settled, true);
  w.env.height = 42 + 256;
  t = (await w.popup({ type: 'txs.refresh' })).transactions[0]; assert.equal(t.status, 'dropped'); assert.equal(t.settled, true, 'final after the bounded window');
  const reads = w.count('eth_getTransactionReceipt');
  await w.popup({ type: 'txs.refresh' }); assert.equal(w.count('eth_getTransactionReceipt'), reads, 'no longer read');
});

// Codex review (PR #114, head 1e00c82d): with more than REFRESH_MAX (20) open transfers, the newest 20 staying pending
// or belonging to an unreachable profile never starved the older ones: a refresh still reads at most 20 entries, and
// rotates through all of them, least recently checked first.
test('Codex review: the bounded history refresh rotates through every open transfer; the newest 20 staying pending or unreachable do not starve an older one', async () => {
  const hex = (v) => '0x' + BigInt(v).toString(16);
  const w = await world({ extra: { eth_chainId: (_, { url }) => (String(url).includes(':8596') ? { __raw: () => 'down' } : hex(spec.chainId)) } });
  assert.equal((await w.options({ type: 'profiles.add', profile: profile('far', 'http://127.0.0.1:8596') })).ok, true);
  const entry = (i, network) => ({ hash: '0x' + i.toString(16).padStart(64, '0'), network, chainId: spec.chainId, genesisHash: spec.hash, from: ADDRESS2, to: TO, value: '1', nonce: String(i), status: 'pending', createdAt: 1000 - i, updatedAt: 1000 - i });
  // 25 open transfers, newest first: 10 on the unreachable profile, then 15 on the devnet that stay pending (nonce
  // unused, no receipt) except the oldest, whose receipt is now available.
  const history = Array.from({ length: 25 }, (_, i) => entry(i + 1, i < 10 ? 'far' : 'devnet'));
  await w.mock.storage.set({ 'wallet:history': history });
  const oldest = history[24].hash;
  w.env.receipts.set(oldest, { transactionHash: oldest, status: '0x1', blockNumber: '0x2b', blockHash: '0x' + 'b2'.repeat(32), gasUsed: '0x5208' });
  const readOf = (h) => w.log.filter((x) => x.method === 'eth_getTransactionReceipt' && x.params[0] === h).length;
  let t;
  for (let i = 0; i < 2; i++) {
    const before = w.count('eth_getTransactionReceipt');
    t = (await w.popup({ type: 'txs.refresh' })).transactions.find((x) => x.hash === oldest);
    assert.ok(w.count('eth_getTransactionReceipt') - before <= 20, 'at most REFRESH_MAX receipts a refresh');
  }
  assert.equal(t.status, 'included', 'the oldest open transfer is checked within two refreshes'); assert.equal(t.blockNumber, 43);
  for (const e of history.slice(10)) assert.ok(readOf(e.hash) >= 1, 'every reachable open transfer was read: ' + e.nonce);
  // The rotation goes on: the next refreshes read the entries least recently checked first, every one in turn.
  const counts = () => history.slice(10, 24).map((e) => readOf(e.hash));
  const c0 = counts();
  for (let i = 0; i < 2; i++) await w.popup({ type: 'txs.refresh' });
  assert.ok(counts().every((c, i) => c > c0[i]), 'each pending devnet entry is read again within two more refreshes');
  const list = (await w.popup({ type: 'txs.list' })).transactions;
  assert.equal(list.length, 25); assert.deepEqual(list.slice(0, 24).map((x) => x.status), Array(24).fill('pending'), 'the others stay pending');
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

// Final review (2026-11-22): the content scripts on studio.pocol.net and ide.pocol.net must not read or write
// chrome.storage.local (the vault, history, profiles, jobs); Chrome allows it unless the worker restricts the area.
test('final review: the worker restricts storage.local to the extension\'s own pages and itself on every start', async () => {
  const w = await world();
  assert.equal(w.mock.storage.accessLevel, 'TRUSTED_CONTEXTS');
  const src = readFileSync(join(dist, 'sw.js'), 'utf8');
  assert.match(src, /setAccessLevel\(STORAGE_ACCESS\)/); assert.match(src, /accessLevel: 'TRUSTED_CONTEXTS'/);
  for (const f of ['studio-content.js', 'ide-content.js']) assert.doesNotMatch(readFileSync(join(dist, f), 'utf8'), /chrome\.storage/, f + ' needs no storage');
});

// Final review: the toolbar's Lock locks every page holding a decrypted key, through a storage signal the pages watch.
test('final review: account.lock writes the lock signal the Studio and IDE pages watch; an unlock or a timeout does not', async () => {
  const VG = await L('vault-guard.js');
  assert.equal(VG.PASSWORD_MIN, A.PASSWORD_MIN, 'vault-guard.js and account.js agree on the password minimum');
  const w = await world();
  await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
  assert.equal(w.mock.storage.data.has(VG.LOCK_SIGNAL_KEY), false, 'an import signals no lock');
  const seen = []; VG.watchLockSignal(chrome, () => seen.push('lock'));
  await w.popup({ type: 'account.unlock', password: PW }); assert.deepEqual(seen, []);
  await w.popup({ type: 'account.lock' });
  const first = w.mock.storage.data.get(VG.LOCK_SIGNAL_KEY); assert.ok(first && Number.isInteger(first.at) && typeof first.id === 'string');
  assert.deepEqual(seen, ['lock']);
  await w.popup({ type: 'account.lock' }); assert.notEqual(w.mock.storage.data.get(VG.LOCK_SIGNAL_KEY).id, first.id, 'every Lock is a new signal');
  assert.deepEqual(seen, ['lock', 'lock']);
});

test('final review: vault-guard saves a page\'s vault only when none is saved, and never replaces another wallet', async () => {
  const VG = await L('vault-guard.js');
  const data = new Map(), storage = { get: async (k) => (data.has(k) ? { [k]: structuredClone(data.get(k)) } : {}), set: async (o) => { for (const [k, v] of Object.entries(o)) data.set(k, structuredClone(v)); } };
  const A1 = '0x' + 'a1'.repeat(20), B1 = '0x' + 'b1'.repeat(20);
  assert.deepEqual(await VG.saveVaultIfNone(storage, A1, async () => ({ address: A1, ct: 'one' })), { state: 'saved' });
  assert.deepEqual(await VG.saveVaultIfNone(storage, A1.toUpperCase().replace('0X', '0x'), async () => { throw new Error('not called'); }), { state: 'same' });
  let made = 0;
  assert.deepEqual(await VG.saveVaultIfNone(storage, B1, async () => { made++; return { address: B1, ct: 'two' }; }), { state: 'other', saved: A1 });
  assert.equal(made, 0); assert.equal(data.get(VG.VAULT_KEY).ct, 'one');
  // A wallet saved by another tab while this one encrypted is not overwritten either.
  data.clear();
  assert.deepEqual(await VG.saveVaultIfNone(storage, B1, async () => { data.set(VG.VAULT_KEY, { address: A1, ct: 'raced' }); return { address: B1, ct: 'two' }; }), { state: 'other', saved: A1 });
  assert.equal(data.get(VG.VAULT_KEY).ct, 'raced');
  assert.throws(() => VG.checkNewPassword('short', 'short'), /at least 10/);
  assert.throws(() => VG.checkNewPassword('long-enough-1', 'long-enough-2'), /differ/);
  VG.checkNewPassword('long-enough-1', 'long-enough-1');
  assert.match(VG.createRefusal(A1), new RegExp(`already saved in this browser \\(${A1}\\).*Nothing was changed`));
});

// Codex review (PR #114): every writer of the vault (the worker's saveAccount and the pages' saveVaultIfNone) reads
// the saved vault again and writes it inside one lock, and a Lock pressed while the worker encrypts is kept.
/** Holds the next PBKDF2 key derivation (the vault encryption) until open(): `entered` resolves once it started. */
function holdKdf() {
  const subtle = globalThis.crypto.subtle, real = subtle.deriveKey;
  let enter, open; const entered = new Promise((r) => (enter = r)), opened = new Promise((r) => (open = r));
  subtle.deriveKey = async function (...args) { delete subtle.deriveKey; enter(); await opened; return real.apply(subtle, args); };
  return { entered, open, restore: () => { if (Object.hasOwn(subtle, 'deriveKey')) delete subtle.deriveKey; } };
}
test('Codex review: a wallet a Studio or IDE page saves while the worker encrypts is never overwritten without replace:true', async () => {
  const VG = await L('vault-guard.js');
  for (const [replace, before] of [[undefined, null], [true, null], [true, KEY2]]) {
    const w = await world();
    if (before) assert.equal((await w.options({ type: 'account.import', kind: 'privateKey', privateKey: before, password: PW })).ok, true);
    const pageVault = await K.exportKeyfile(H.fromHex(PHRASE_KEY), PW);
    const kdf = holdKdf();
    try {
      // The settings page creates a wallet (replace:true covers only the wallet saved when it started); while the worker
      // encrypts it, a Studio or IDE tab saves another wallet (before: none) or replaces nothing (it never replaces).
      const pending = w.options({ type: 'account.create', password: PW, replace });
      await kdf.entered;
      if (before) w.mock.storage.data.delete('studio:wallet'); // the saved wallet is gone meanwhile (another tab's replace)
      assert.deepEqual(await VG.saveVaultIfNone(chrome.storage.local, PHRASE_ADDRESS, async () => pageVault), { state: 'saved' });
      kdf.open();
      const r = await pending;
      assert.equal(r.ok, false, `replace ${replace}, before ${!!before}: ${JSON.stringify(r)}`); assert.equal(r.error.code, 'exists'); assert.equal(r.error.saved, PHRASE_ADDRESS);
      assert.deepEqual(vaultOf(w), pageVault, 'the page\'s wallet is still saved');
      assert.equal((await w.popup({ type: 'account.status' })).unlocked, false, 'no key of another wallet is unlocked');
    } finally { kdf.restore(); }
  }
});
test('Codex review: two tabs saving a wallet at once: the second reads the first one\'s vault and writes nothing', async () => {
  const VG = await L('vault-guard.js');
  // A storage whose answers arrive a moment after the read (the storage IPC), so both tabs could read "none" first.
  const data = new Map(), storage = {
    get: (k) => { const o = data.has(k) ? { [k]: structuredClone(data.get(k)) } : {}; return new Promise((r) => setTimeout(() => r(o), 1)); },
    set: async (o) => { for (const [k, v] of Object.entries(o)) data.set(k, structuredClone(v)); },
  };
  const A1 = '0x' + 'a1'.repeat(20), B1 = '0x' + 'b1'.repeat(20);
  const out = await Promise.all([VG.saveVaultIfNone(storage, A1, async () => ({ address: A1, ct: 'one' })), VG.saveVaultIfNone(storage, B1, async () => ({ address: B1, ct: 'two' }))]);
  const kept = data.get(VG.VAULT_KEY);
  assert.deepEqual(out.map((x) => x.state).sort(), ['other', 'saved'], JSON.stringify(out));
  assert.equal(out.find((x) => x.state === 'other').saved, kept.address, 'the refused tab names the wallet that was saved');
  assert.equal(kept.ct, kept.address === A1 ? 'one' : 'two');
});
// Codex review (PR #114, head 7f91bb9e): the settings page's replacement confirmation names one wallet. The confirmed
// request carries that address, and the worker compares it with the saved vault inside the vault lock, so a wallet
// another Settings tab saved meanwhile is never overwritten by a confirmation (and a backup) that named another one.
test('Codex review: a confirmed replacement covers only the wallet its confirmation named, never one another tab saved meanwhile', async () => {
  const w = await world();
  assert.equal((await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW })).ok, true);
  // Tab 1 asks to create a wallet; the worker refuses and names the saved wallet A, which tab 1's confirmation shows.
  const first = await w.options({ type: 'account.create', password: PW }); assert.equal(first.error.code, 'exists'); assert.equal(first.error.saved, ADDRESS2);
  // Meanwhile tab 2 replaces A with B after its own confirmation of A.
  const other = await w.mock.send({ type: 'account.import', kind: 'phrase', phrase: PHRASE, password: PW, replace: true, replaceAddress: ADDRESS2 }, 8, 'options.html');
  assert.equal(other.ok, true, JSON.stringify(other)); assert.equal(other.address, PHRASE_ADDRESS);
  const b = vaultOf(w);
  // Tab 1's user confirms replacing A: B is not replaced, and the refusal names B for a new confirmation.
  const r = await w.options({ type: 'account.create', password: PW, replace: true, replaceAddress: first.error.saved });
  assert.equal(r.ok, false, JSON.stringify(r)); assert.equal(r.error.code, 'exists'); assert.equal(r.error.saved, PHRASE_ADDRESS);
  assert.deepEqual(vaultOf(w), b, 'B is still saved');
  const st = await w.popup({ type: 'account.status' }); assert.equal(st.address, PHRASE_ADDRESS); assert.equal(st.unlocked, true, 'B stays unlocked');
  // A replacement naming a malformed or missing address replaces nothing either.
  for (const replaceAddress of [ADDRESS2.toUpperCase(), 'nonsense', 42]) {
    const x = await w.options({ type: 'account.create', password: PW, replace: true, replaceAddress: replaceAddress === ADDRESS2.toUpperCase() ? '0x' + ADDRESS2.slice(2).toUpperCase() : replaceAddress });
    assert.equal(x.error.code, 'exists', String(replaceAddress)); assert.deepEqual(vaultOf(w), b);
  }
  // The settings page sends the address its confirmation showed; a confirmation naming B replaces B.
  assert.match(readFileSync(join(dist, 'options.js'), 'utf8'), /\{ \.\.\.msg, replace: true, replaceAddress: r\.error\.saved \?\? null \}/);
  const ok = await w.options({ type: 'account.create', password: PW, replace: true, replaceAddress: '0x' + PHRASE_ADDRESS.slice(2).toUpperCase() });
  assert.equal(ok.ok, true, JSON.stringify(ok)); assert.equal(vaultOf(w).address, ok.address); assert.notEqual(ok.address, PHRASE_ADDRESS);
});
// Codex review (PR #114, head 7f91bb9e): a Studio publication or an IDE contract request of this wallet that signs
// after the transfer was reviewed, and pauses before its transaction reached the node, holds the reviewed nonce. The
// unfinished-job check runs again under the transfer lock, before signing, as the publisher and contract flows do.
test('Codex review: an unfinished Studio or IDE job that signed after the review blocks the confirmation (checked under the transfer lock)', async () => {
  const w = await world();
  w.env.chainId = Number(N.NETWORK.chainId); w.env.genesis = N.NETWORK.genesisHash.toLowerCase();
  assert.equal((await w.options({ type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW })).ok, true);
  for (const key of ['studio:job:after-review', 'ide:job:after-review']) {
    const p = await w.send({ type: 'send.prepare', to: TO, amount: '1' }); assert.equal(p.ok, true, JSON.stringify(p)); assert.equal(p.network, N.NETWORK.name);
    await w.mock.storage.set({ [key]: { id: 'after-review', status: 'paused', createdAt: Date.now(), walletAddress: ADDRESS2,
      network: { chainId: String(N.NETWORK.chainId), genesisHash: N.NETWORK.genesisHash.toLowerCase() },
      checkpoint: { transactions: { ['0x' + 'cd'.repeat(32)]: { tx: { chainId: String(N.NETWORK.chainId), nonce: '0' } } } } } });
    const c = await w.send({ type: 'send.confirm', id: p.id });
    assert.equal(c.ok, false, key + ': ' + JSON.stringify(c)); assert.equal(c.error.code, 'busy'); assert.match(c.error.message, /unfinished Studio or IDE operation/);
    await w.mock.storage.remove(key);
  }
  assert.equal(w.count('eth_sendRawTransaction'), 0, 'nothing was sent'); assert.equal((await w.popup({ type: 'txs.list' })).transactions.length, 0, 'nothing was signed or recorded');
  const p = await w.send({ type: 'send.prepare', to: TO, amount: '1' }); assert.equal((await w.send({ type: 'send.confirm', id: p.id })).status, 'pending', 'without the job the transfer is sent');
});
// Codex review (PR #114, head 7f91bb9e): a Lock or a profile change while the transfer's history entry is written (or
// while the request waits for its transport reservation) stops it: the account epoch and the profiles are checked
// again immediately before the request starts, as the viewer wallet's submit() does, and its entry says "not sent".
test('Codex review: a Lock or a profile change while the transfer is recorded stops it before it is sent; its history entry says it was not sent', async () => {
  for (const [what, change, code] of [['lock', (w) => w.popup({ type: 'account.lock' }), 4100], ['profile', (w) => w.options({ type: 'profiles.add', profile: profile('third', 'http://127.0.0.1:8597') }), 4901]]) {
    const w = await world();
    const p = await prepared(w);
    w.mock.storage.holdSets = 1; // the history write made before the broadcast waits
    const pending = w.send({ type: 'send.confirm', id: p.id });
    for (let i = 0; i < 1000 && !w.mock.storage.held.length; i++) await new Promise((r) => realTimeout(r, 2));
    assert.equal(w.mock.storage.held.length, 1, what + ': the history write is pending');
    assert.equal((await change(w)).ok, true, what);
    w.mock.storage.release();
    const c = await pending;
    assert.equal(c.ok, false, what + ': ' + JSON.stringify(c)); assert.equal(c.error.code, code, what);
    assert.equal(w.count('eth_sendRawTransaction'), 0, what + ': nothing was sent');
    const list = (await w.popup({ type: 'txs.list' })).transactions; assert.equal(list.length, 1);
    assert.equal(list[0].status, 'cancelled', what); assert.match(list[0].note, /^Not sent/, what);
    const reads = w.count('eth_getTransactionReceipt'); await w.popup({ type: 'txs.refresh' });
    assert.equal(w.count('eth_getTransactionReceipt'), reads, what + ': a transfer that was not sent is not followed');
  }
});
test('Codex review: a Lock while the transfer waits for its transport reservation aborts the queued request at once; nothing is sent', async () => {
  let open; const gate = new Promise((r) => (open = r)); let holding = false;
  const w = await world({ extra: { eth_getBalance: async () => { if (holding) await gate; return '0x' + (10n ** 21n).toString(16); } } });
  const p = await prepared(w);
  w.mock.storage.holdSets = 1; // the history write waits while the trust pool is filled
  const pending = w.send({ type: 'send.confirm', id: p.id });
  for (let i = 0; i < 1000 && !w.mock.storage.held.length; i++) await new Promise((r) => realTimeout(r, 2));
  assert.equal(w.mock.storage.held.length, 1, 'the history write is pending');
  // 512 balance reads of 4 KiB each hold the whole 2 MiB trust pool (lib/transport.js TRUST_POOL), so the send queues.
  holding = true; const reads = w.count('eth_getBalance');
  const balances = Array.from({ length: 512 }, () => w.popup({ type: 'account.balance', name: 'devnet' }));
  for (let i = 0; i < 2000 && w.count('eth_getBalance') < reads + 512; i++) await new Promise((r) => realTimeout(r, 2));
  assert.equal(w.count('eth_getBalance'), reads + 512, 'the trust pool is full');
  w.mock.storage.release();
  await new Promise((r) => realTimeout(r, 50)); // the send request is now waiting for its reservation
  assert.equal(w.count('eth_sendRawTransaction'), 0);
  let settled = false; pending.then(() => { settled = true; });
  assert.equal((await w.popup({ type: 'account.lock' })).ok, true);
  for (let i = 0; i < 200 && !settled; i++) await new Promise((r) => realTimeout(r, 2));
  assert.equal(settled, true, 'the queued request ends at once, while the pool is still full');
  const c = await pending; assert.equal(c.ok, false, JSON.stringify(c)); assert.equal(c.error.code, 4100);
  open(); await Promise.all(balances);
  await new Promise((r) => realTimeout(r, 50));
  assert.equal(w.count('eth_sendRawTransaction'), 0, 'nothing was sent, also after the pool emptied');
  const [t] = (await w.popup({ type: 'txs.list' })).transactions; assert.equal(t.status, 'cancelled'); assert.match(t.note, /^Not sent/);
});
test('Codex review: a Lock pressed while the worker encrypts a new wallet is kept: the wallet is saved and stays locked', async () => {
  for (const kind of ['create', 'import']) {
    const w = await world();
    const kdf = holdKdf();
    try {
      const pending = w.options(kind === 'create' ? { type: 'account.create', password: PW } : { type: 'account.import', kind: 'privateKey', privateKey: KEY2, password: PW });
      await kdf.entered;
      assert.equal((await w.popup({ type: 'account.lock' })).ok, true);
      kdf.open();
      const r = await pending; assert.equal(r.ok, true, JSON.stringify(r));
      assert.equal(vaultOf(w).address, r.address, 'the wallet is saved'); assert.deepEqual(r.backup, vaultOf(w), 'its backup is returned');
      assert.equal(r.unlocked, false, 'the reply says it stays locked');
      const st = await w.popup({ type: 'account.status' }); assert.equal(st.unlocked, false, kind + ': the Lock was kept'); assert.equal(st.address, r.address);
      assert.equal((await w.send({ type: 'send.prepare', name: 'devnet', to: TO, amount: '1' })).error.code, 4100, 'nothing can be signed');
      assert.equal((await w.popup({ type: 'account.unlock', password: PW })).ok, true, 'it unlocks with its password');
    } finally { kdf.restore(); }
  }
});
