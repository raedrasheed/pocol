// Sites W4: the publisher page (src/publisher-ui.js) in a VM with mocked DOM and storage and the real fee, lease, job
// and plan libraries from dist/. publishSite is stubbed so each test controls the publication's outcome.
import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';

const VG = await import('../dist/lib/vault-guard.js'), I = await import('../dist/lib/identity.js'), R = await import('../dist/lib/retry.js'), P = await import('../dist/lib/publisher.js'), L = await import('../dist/lib/lease.js'), J = await import('../dist/lib/jobs.js'), B = await import('../dist/lib/studio-bridge.js');
const SOURCE = readFileSync(new URL('../src/publisher-ui.js', import.meta.url), 'utf8').replace(/^import [^;]*;\n/gm, '');
const HTML = readFileSync(new URL('../src/publisher.html', import.meta.url), 'utf8');
const JOB_ID = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee', KEY = 'studio:job:' + JOB_ID, ADDRESS = '0x1111111111111111111111111111111111111111';
const NETWORK = {chainId: 2656089850, genesisHash: '0x' + 'ab'.repeat(32), endpoint: 'https://explorer.pocol.net/studio/rpc', name: 'pocol-public'};
const FLOOR = '10000000', TIP = '1000000000';
const settle = async () => { for (let i = 0; i < 40; i++) await new Promise((r) => setImmediate(r)); };
function makeElement(id) { return {id, textContent: '', hidden: false, disabled: false, checked: false, value: '', files: [], children: [], replaceChildren(...items) { this.children = items; }, after() {}, append() {}, remove() {}, click() {}}; }
async function fixture({job = {}, publish, base = FLOOR, now = 1000, rpc: editAnswers, vault = {schema: 'pocol-studio-vault-v1'}} = {}) {
  let clock = now;
  class Clock extends Date { static now() { return clock; } }
  const elements = new Map([...HTML.matchAll(/id="([^"]+)"/g)].map((m) => [m[1], makeElement(m[1])]));
  const files = [{path: '/index.html', base64: btoa('<h1>hi</h1>')}, {path: '/style.css', base64: btoa('b{}')}];
  const storage = new Map([[KEY, {id: JOB_ID, origin: 'https://studio.pocol.net', status: 'awaitingApproval', network: {chainId: String(NETWORK.chainId), genesisHash: NETWORK.genesisHash}, request: {title: 'Site', files, entry: '/index.html', site: null}, ...job}], ...(vault ? [['studio:wallet', vault]] : [])]);
  const changed = [];
  const state = {calls: [], publishes: [], signed: 0};
  const rpcAnswers = {eth_chainId: '0x' + NETWORK.chainId.toString(16), pocol_status: {genesisHash: NETWORK.genesisHash}, eth_maxPriorityFeePerGas: '0x' + BigInt(TIP).toString(16),
    eth_gasPrice: () => '0x' + (BigInt(state.base ?? base) + BigInt(TIP)).toString(16), eth_feeHistory: () => ({baseFeePerGas: ['0x' + BigInt(state.base ?? base).toString(16)]}), eth_getBlockByNumber: () => ({baseFeePerGas: '0x' + BigInt(state.base ?? base).toString(16), gasUsed: '0x0', gasLimit: '0x7a1200'})};
  if (editAnswers) editAnswers(rpcAnswers);
  const globals = {
    Date: Clock, Uint8Array, TextEncoder, structuredClone, AbortController, BigInt, Number, Math, JSON, Object, Array, Error, Promise, Map, Set, console, btoa, atob,
    location: {hash: '#' + JOB_ID}, document: {documentElement: {}, getElementById: (id) => { if (id === 'resumeEarlier') return null; assert.ok(elements.has(id), id); return elements.get(id); }, createElement: (t) => makeElement(t)},
    window: {addEventListener() {}}, setInterval: (fn) => { state.tick = fn; }, setTimeout, confirm: () => true, crypto: globalThis.crypto, Blob, URL,
    NETWORK, assertIdentity: I.assertIdentity, JOB_PREFIX: B.JOB_PREFIX, base64Bytes: B.base64Bytes, LOCK_MS: 900000,
    importKeyfile: async () => new Uint8Array(32).fill(7), exportKeyfile: async () => ({address: ADDRESS, ct: 'new'}), validSecret() {}, addressOf: () => ADDRESS,
    saveVaultIfNone: VG.saveVaultIfNone, savedVault: VG.savedVault, vaultAddressOf: VG.vaultAddressOf, createRefusal: VG.createRefusal, otherWalletNote: VG.otherWalletNote, checkNewPassword: VG.checkNewPassword, watchLockSignal: VG.watchLockSignal,
    signTx: () => { state.signed++; return {raw: '0x02', hash: '0x' + 'ee'.repeat(32)}; },
    prepareSite: P.prepareSite, quotePublish: P.quotePublish, readNetworkFees: P.readNetworkFees, remainingTransactions: P.remainingTransactions,
    publishSite: async (o) => { state.publishes.push(o); return publish ? publish(o, state, (v) => { clock = v; }) : {site: '0x' + '22'.repeat(20), version: 0, transactions: []}; },
    KeyLease: class extends L.KeyLease { constructor(o) { super({...o, clock: () => clock}); } }, JOB_DEADLINE_DEFAULT_MS: L.JOB_DEADLINE_DEFAULT_MS, JOB_DEADLINE_MAX_MS: L.JOB_DEADLINE_MAX_MS,
    isForeignJob: J.isForeignJob, blocksAccount: J.blocksAccount, archiveJob: J.archiveJob, networkOf: J.networkOf, filesKey: J.filesKey, rawKey: J.rawKey, removeSideKeys: J.removeSideKeys,
    request: async (_url, method, params) => { state.calls.push(method); if (state.busy && state.busy[method] > 0) { state.busy[method]--; throw Object.assign(new Error('rate'), {transport: 'rate', status: 429, retryAfterMs: 2000}); } const a = rpcAnswers[method]; if (a === undefined) throw Object.assign(new Error('no ' + method), {rpc: {code: -32601}}); return typeof a === 'function' ? a(params) : a; },
    Pool: class {}, TRUST_POOL: 1, RESERVE_WAIT_MS: 2000, withRetry: (fn, o) => R.withRetry(fn, {...o, sleep: async () => {}}),
    navigator: {locks: {request: async (_n, _o, fn) => fn({})}},
    chrome: {runtime: {getURL: (p) => 'chrome-extension://x/' + p}, tabs: {create() {}}, storage: {onChanged: {addListener: (f) => changed.push(f)}, local: {
      get: async (k) => k === null ? Object.fromEntries([...storage].map(([a, b]) => [a, structuredClone(b)])) : (storage.has(k) ? {[k]: structuredClone(storage.get(k))} : {}),
      set: async (o) => { for (const [k, v] of Object.entries(o)) storage.set(k, structuredClone(v)); },
      remove: async (k) => { for (const x of [].concat(k)) storage.delete(x); }, getKeys: async () => [...storage.keys()]}}},
  };
  const context = vm.createContext(globals); vm.runInContext(SOURCE, context); await settle();
  const click = async (id) => { await elements.get(id).onclick(); await settle(); };
  const unlock = async () => { elements.get('password').value = 'password-123'; await click('unlock'); };
  const consent = () => { elements.get('consent').checked = true; elements.get('consent').onchange(); };
  return {elements, storage, state, context, click, unlock, consent, setNow: (v) => { clock = v; }, storageChange: (c) => { for (const f of changed) f(c, 'local'); }};
}
test('the chain ID chip shows the built-in profile\'s chain ID; the page names no network of its own', async () => {
  const f = await fixture();
  assert.equal(f.elements.get('networkChain').textContent, String(NETWORK.chainId));
  assert.match(HTML, /<span class="chip" id="networkChain" title="Chain ID"><\/span>/);
  assert.doesNotMatch(HTML, /(?<![0-9A-Za-z])[0-9]{7,}(?![0-9A-Za-z])/, 'no chain ID in the page');
});
test('W4 page: the files move once to their own key, and the fees are quoted from the node, not fixed caps', async () => {
  const f = await fixture();
  assert.equal(f.elements.get('fatal').textContent, '', 'the page opened without a fatal error');
  const job = f.storage.get(KEY);assert.equal(job.request.files, undefined);assert.equal(job.request.filesKey, J.filesKey(JOB_ID));assert.equal(f.storage.get(J.filesKey(JOB_ID)).length, 2);
  assert.ok(f.state.calls.includes('eth_feeHistory') && f.state.calls.includes('eth_gasPrice'), 'the base fee was read from the node');
  assert.match(f.elements.get('fee').textContent, /PCOL/);assert.match(f.elements.get('feeDetails').textContent, /Expected cost .* ceiling .* gwei per gas .* next base fee 0\.01 gwei/);
  assert.doesNotMatch(SOURCE, /200000000000|'1000000000'/, 'no literal fee caps');
  await f.unlock(); f.consent(); assert.equal(f.elements.get('publish').disabled, false, JSON.stringify(['fee',f.elements.get('fee').textContent,f.elements.get('feeDetails').textContent,f.elements.get('walletAddress').textContent,f.elements.get('walletNote').textContent,f.elements.get('fatal').textContent]));
  await f.click('publish');
  const o = f.state.publishes[0]; assert.equal(o.approval.economy, false); assert.ok(Number(o.approval.ceilingWei) > 0); assert.equal(o.reapprove, false); assert.equal(o.rawStore !== undefined, true);
  assert.equal(f.storage.get(KEY).status, 'complete');assert.deepEqual(f.storage.get(KEY).network, {chainId: String(NETWORK.chainId), genesisHash: NETWORK.genesisHash});
});
test('W4 page: economy pacing and a lower tip are options, off and the node tip by default', async () => {
  const f = await fixture();
  f.elements.get('economy').checked = true; f.elements.get('tip').value = '0.5'; await f.elements.get('economy').onchange(); await settle();
  await f.unlock(); f.consent(); await f.click('publish');
  const a = f.state.publishes[0].approval; assert.equal(a.economy, true); assert.equal(a.tipWei, '500000000');
});
test('W4 page: a publication longer than LOCK_MS keeps the key through its signatures; Lock aborts it', async () => {
  const f = await fixture({publish: async (o, state, setNow) => {
    for (let i = 1; i <= 8; i++) { setNow(1000 + i * 10 * 60 * 1000); state.tick(); await o.sign({}); }
    return {site: '0x' + '22'.repeat(20), version: 0, transactions: []};
  }});
  await f.unlock(); f.consent(); await f.click('publish');
  assert.equal(f.state.signed, 8, '80 minutes of signatures, every one within the lease'); assert.equal(f.storage.get(KEY).status, 'complete');
  const g = await fixture({publish: async (o, state) => { await o.sign({}); g.elements.get('lock').onclick(); assert.equal(o.signal.aborted, true); await assert.rejects(o.sign({}), /locked|paused/); throw Object.assign(new Error('Publication paused'), {code: 'PUBLISH_PAUSED'}); }});
  await g.unlock(); g.consent(); await g.click('publish'); assert.equal(g.state.signed, 1); assert.equal(g.storage.get(KEY).status, 'paused');
});
test('W4 page: a "fee rose" pause quotes the remaining transactions and the next approval is a re-approval', async () => {
  let first = true;
  const f = await fixture({publish: async (o) => {
    if (first) { first = false; await o.onCheckpoint({schema: 2, transactions: {['chunk:' + '0x' + '33'.repeat(20)]: {hash: '0x1', tx: {}}}}); throw Object.assign(new Error('Network fees rose'), {code: 'PUBLISH_FEE_ROSE'}); }
    return {site: '0x' + '22'.repeat(20), version: 0, transactions: []};
  }});
  await f.unlock(); f.consent(); f.state.base = '2000000000'; await f.click('publish');
  assert.match(f.elements.get('status').textContent, /new budget for the remaining transactions/); assert.match(f.elements.get('feeDetails').textContent, /Network fees rose/);
  assert.equal(f.elements.get('consent').checked, false, 'the new budget needs a new consent');
  f.consent(); await f.click('publish'); const o = f.state.publishes[1]; assert.equal(o.reapprove, true); assert.notDeepEqual(o.approval, f.state.publishes[0].approval);
});
test('W4 page: a job of another network is read-only, never blocks, and Cancel archives it without the network', async () => {
  const f = await fixture({job: {status: 'paused', walletAddress: ADDRESS, network: {chainId: '777', genesisHash: '0x' + 'cd'.repeat(32)}, checkpoint: {transactions: {x: {hash: '0x1', tx: {chainId: '777'}}}}}});
  await f.unlock(); f.consent(); assert.equal(f.elements.get('publish').disabled, true); assert.match(f.elements.get('status').textContent, /another network/);
  const before = f.state.calls.length; await f.click('cancel');
  assert.equal(f.state.calls.length, before, 'no RPC'); assert.equal(f.storage.has(KEY), false); assert.equal(f.storage.get('archive:' + KEY).archivedReason, 'cancelled-foreign-network');
  const g = await fixture(); g.storage.set('studio:job:other', {id: 'other', status: 'paused', walletAddress: ADDRESS, network: {chainId: '777', genesisHash: NETWORK.genesisHash}, checkpoint: {transactions: {x: {}}}});
  await g.unlock(); g.consent(); await g.click('publish'); assert.equal(g.state.publishes.length, 1, 'the foreign job does not block this wallet');
  const h = await fixture(); h.storage.set('studio:job:other', {id: 'other', status: 'paused', walletAddress: ADDRESS, network: {chainId: String(NETWORK.chainId), genesisHash: NETWORK.genesisHash}, checkpoint: {transactions: {x: {}}}});
  await h.unlock(); h.consent(); await h.click('publish'); assert.equal(h.state.publishes.length, 0, 'this network\'s unresolved job still blocks'); assert.match(h.elements.get('status').textContent, /earlier publication/);
});

// W2/W11: the page's reads go through withRetry (a gateway's 429 is retried), a broadcast is never retried.
test('W2 page: a read answered 429 by the gateway is retried, eth_sendRawTransaction is sent once', async () => {
  const f = await fixture();
  f.state.busy = {eth_gasPrice: 2}; f.state.calls.length = 0;
  await f.click('refreshFees');
  assert.equal(f.state.calls.filter((m) => m === 'eth_gasPrice').length, 3, 'two 429 answers, then the read'); assert.match(f.elements.get('fee').textContent, /PCOL/);
  const rpc = vm.runInContext('rpc', f.context); f.state.busy = {eth_sendRawTransaction: 1}; f.state.calls.length = 0;
  await assert.rejects(rpc('eth_sendRawTransaction', ['0x02']), /rate/); assert.equal(f.state.calls.filter((m) => m === 'eth_sendRawTransaction').length, 1);
});

// 2026-11-22: an endpoint of another network is refused before any fee quote or signature, and the page shows the
// difference (lib/identity.js): the chain ID or genesis hash this page signs for and the one the endpoint answered.
test('identity page: another chain ID or genesis is refused and the difference is shown; nothing can be published', async () => {
  const f = await fixture();
  assert.equal(f.state.signed, 0);
  const genesis = NETWORK.genesisHash, otherGenesis = '0x' + 'cd'.repeat(32);
  for (const [field, set, expected] of [['chain', (a) => { a.eth_chainId = '0x1'; }, new RegExp(`chain ID: expected ${NETWORK.chainId}, the endpoint answered 1`)],
    ['genesis', (a) => { a.pocol_status = {genesisHash: otherGenesis}; }, new RegExp(`genesis hash: expected ${genesis}, the endpoint answered ${otherGenesis}`)]]) {
    const g = await fixture({rpc: set});
    assert.match(g.elements.get('feeDetails').textContent, /Network identity mismatch/, field);
    assert.match(g.elements.get('feeDetails').textContent, expected, field);
    await g.unlock(); g.consent(); assert.equal(g.elements.get('publish').disabled, true, field + ': no quote, so nothing can be approved');
    assert.equal(g.state.signed, 0); assert.equal(g.state.publishes.length, 0);
  }
});

// Final review (2026-11-22): the publisher page never replaces a saved PoCol wallet of another address. A Create is
// refused while a wallet is saved and needs the password twice; a file of another wallet is unlocked in the tab only.
const SAVED = {schema: 'pocol-studio-vault-v1', address: '0x' + '99'.repeat(20), ct: 'saved'};
test('final review: Create never replaces a saved wallet, and needs the same password twice', async () => {
  const f = await fixture({vault: SAVED});
  f.elements.get('password').value = 'password-123'; f.elements.get('passwordConfirm').value = 'password-123'; await f.click('create');
  assert.deepEqual(f.storage.get('studio:wallet'), SAVED, 'the saved wallet is unchanged');
  assert.match(f.elements.get('walletNote').textContent, /already saved in this browser \(0x9{40}\)/);
  assert.equal(f.elements.get('walletAddress').textContent, 'Wallet locked');
  const g = await fixture({vault: null});
  g.elements.get('password').value = 'password-123'; g.elements.get('passwordConfirm').value = 'password-124'; await g.click('create');
  assert.equal(g.storage.has('studio:wallet'), false); assert.match(g.elements.get('walletNote').textContent, /differ/);
  g.elements.get('password').value = 'short'; g.elements.get('passwordConfirm').value = 'short'; await g.click('create');
  assert.equal(g.storage.has('studio:wallet'), false); assert.match(g.elements.get('walletNote').textContent, /at least 10/);
  g.elements.get('password').value = 'password-123'; g.elements.get('passwordConfirm').value = 'password-123'; await g.click('create');
  assert.deepEqual(g.storage.get('studio:wallet'), {address: ADDRESS, ct: 'new'}, 'with no wallet saved, Create saves the new one');
  assert.equal(g.elements.get('walletAddress').textContent, ADDRESS);
});
test('final review: unlocking with a file of another wallet keeps the saved wallet; with none saved the file is saved', async () => {
  const f = await fixture({vault: SAVED});
  f.elements.get('keyfile').files = [{size: 10, arrayBuffer: async () => new Uint8Array(10).buffer}];
  await f.unlock();
  assert.deepEqual(f.storage.get('studio:wallet'), SAVED, 'the saved wallet is unchanged');
  assert.equal(f.elements.get('walletAddress').textContent, ADDRESS, 'the file\'s wallet is unlocked in this tab');
  assert.match(f.elements.get('walletNote').textContent, /unlocked in this tab only.*\(0x9{40}\) was not changed/);
  const g = await fixture({vault: null});
  g.elements.get('keyfile').files = [{size: 10, arrayBuffer: async () => new Uint8Array(10).buffer}];
  await g.unlock(); assert.deepEqual(g.storage.get('studio:wallet'), {address: ADDRESS, ct: 'new'});
});
test('final review: the toolbar\'s Lock (the worker\'s lock signal) locks this page and stops an approved publication', async () => {
  const f = await fixture({publish: async (o) => { await o.sign({}); f.storageChange({'wallet:lockSignal': {newValue: {at: 1, id: 'x'}}}); assert.equal(o.signal.aborted, true); await assert.rejects(o.sign({}), /locked|paused/); throw Object.assign(new Error('Publication paused'), {code: 'PUBLISH_PAUSED'}); }});
  await f.unlock(); f.consent(); await f.click('publish');
  assert.equal(f.state.signed, 1); assert.equal(f.elements.get('walletAddress').textContent, 'Wallet locked'); assert.equal(f.storage.get(KEY).status, 'paused');
  const g = await fixture(); await g.unlock(); g.storageChange({'something:else': {newValue: 1}}); assert.equal(g.elements.get('walletAddress').textContent, ADDRESS, 'another key changing does not lock');
});
