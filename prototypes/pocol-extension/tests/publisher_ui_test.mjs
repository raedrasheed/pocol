// Sites W4: the publisher page (src/publisher-ui.js) in a VM with mocked DOM and storage and the real fee, lease, job
// and plan libraries from dist/. publishSite is stubbed so each test controls the publication's outcome.
import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';

const P = await import('../dist/lib/publisher.js'), L = await import('../dist/lib/lease.js'), J = await import('../dist/lib/jobs.js'), B = await import('../dist/lib/studio-bridge.js');
const SOURCE = readFileSync(new URL('../src/publisher-ui.js', import.meta.url), 'utf8').replace(/^import [^;]*;\n/gm, '');
const HTML = readFileSync(new URL('../src/publisher.html', import.meta.url), 'utf8');
const JOB_ID = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee', KEY = 'studio:job:' + JOB_ID, ADDRESS = '0x1111111111111111111111111111111111111111';
const NETWORK = {chainId: 1599846715, genesisHash: '0x' + 'ab'.repeat(32), endpoint: 'https://gateway.pocol.net/studio/rpc', name: 'pocol-public'};
const FLOOR = '10000000', TIP = '1000000000';
const settle = async () => { for (let i = 0; i < 40; i++) await new Promise((r) => setImmediate(r)); };
function makeElement(id) { return {id, textContent: '', hidden: false, disabled: false, checked: false, value: '', files: [], children: [], replaceChildren(...items) { this.children = items; }, after() {}, append() {}, remove() {}}; }
async function fixture({job = {}, publish, base = FLOOR, now = 1000} = {}) {
  let clock = now;
  class Clock extends Date { static now() { return clock; } }
  const elements = new Map([...HTML.matchAll(/id="([^"]+)"/g)].map((m) => [m[1], makeElement(m[1])]));
  const files = [{path: '/index.html', base64: btoa('<h1>hi</h1>')}, {path: '/style.css', base64: btoa('b{}')}];
  const storage = new Map([[KEY, {id: JOB_ID, origin: 'https://studio.pocol.net', status: 'awaitingApproval', network: {chainId: String(NETWORK.chainId), genesisHash: NETWORK.genesisHash}, request: {title: 'Site', files, entry: '/index.html', site: null}, ...job}], ['studio:wallet', {schema: 'pocol-studio-vault-v1'}]]);
  const state = {calls: [], publishes: [], signed: 0};
  const rpcAnswers = {eth_chainId: '0x' + NETWORK.chainId.toString(16), pocol_status: {genesisHash: NETWORK.genesisHash}, eth_maxPriorityFeePerGas: '0x' + BigInt(TIP).toString(16),
    eth_gasPrice: () => '0x' + (BigInt(state.base ?? base) + BigInt(TIP)).toString(16), eth_feeHistory: () => ({baseFeePerGas: ['0x' + BigInt(state.base ?? base).toString(16)]}), eth_getBlockByNumber: () => ({baseFeePerGas: '0x' + BigInt(state.base ?? base).toString(16), gasUsed: '0x0', gasLimit: '0x7a1200'})};
  const globals = {
    Date: Clock, Uint8Array, TextEncoder, structuredClone, AbortController, BigInt, Number, Math, JSON, Object, Array, Error, Promise, Map, Set, console, btoa, atob,
    location: {hash: '#' + JOB_ID}, document: {documentElement: {}, getElementById: (id) => { if (id === 'resumeEarlier') return null; assert.ok(elements.has(id), id); return elements.get(id); }, createElement: (t) => makeElement(t)},
    window: {addEventListener() {}}, setInterval: (fn) => { state.tick = fn; }, setTimeout, confirm: () => true, crypto: globalThis.crypto, Blob, URL,
    NETWORK, JOB_PREFIX: B.JOB_PREFIX, base64Bytes: B.base64Bytes, LOCK_MS: 900000,
    importKeyfile: async () => new Uint8Array(32).fill(7), exportKeyfile: async () => ({}), validSecret() {}, addressOf: () => ADDRESS,
    signTx: () => { state.signed++; return {raw: '0x02', hash: '0x' + 'ee'.repeat(32)}; },
    prepareSite: P.prepareSite, quotePublish: P.quotePublish, readNetworkFees: P.readNetworkFees, remainingTransactions: P.remainingTransactions,
    publishSite: async (o) => { state.publishes.push(o); return publish ? publish(o, state, (v) => { clock = v; }) : {site: '0x' + '22'.repeat(20), version: 0, transactions: []}; },
    KeyLease: class extends L.KeyLease { constructor(o) { super({...o, clock: () => clock}); } }, JOB_DEADLINE_DEFAULT_MS: L.JOB_DEADLINE_DEFAULT_MS, JOB_DEADLINE_MAX_MS: L.JOB_DEADLINE_MAX_MS,
    isForeignJob: J.isForeignJob, blocksAccount: J.blocksAccount, archiveJob: J.archiveJob, networkOf: J.networkOf, filesKey: J.filesKey, rawKey: J.rawKey, removeSideKeys: J.removeSideKeys,
    request: async (_url, method, params) => { state.calls.push(method); const a = rpcAnswers[method]; if (a === undefined) throw Object.assign(new Error('no ' + method), {rpc: {code: -32601}}); return typeof a === 'function' ? a(params) : a; },
    Pool: class {}, TRUST_POOL: 1,
    navigator: {locks: {request: async (_n, _o, fn) => fn({})}},
    chrome: {runtime: {getURL: (p) => 'chrome-extension://x/' + p}, tabs: {create() {}}, storage: {local: {
      get: async (k) => k === null ? Object.fromEntries([...storage].map(([a, b]) => [a, structuredClone(b)])) : (storage.has(k) ? {[k]: structuredClone(storage.get(k))} : {}),
      set: async (o) => { for (const [k, v] of Object.entries(o)) storage.set(k, structuredClone(v)); },
      remove: async (k) => { for (const x of [].concat(k)) storage.delete(x); }, getKeys: async () => [...storage.keys()]}}},
  };
  const context = vm.createContext(globals); vm.runInContext(SOURCE, context); await settle();
  const click = async (id) => { await elements.get(id).onclick(); await settle(); };
  const unlock = async () => { elements.get('password').value = 'password-123'; await click('unlock'); };
  const consent = () => { elements.get('consent').checked = true; elements.get('consent').onchange(); };
  return {elements, storage, state, context, click, unlock, consent, setNow: (v) => { clock = v; }};
}
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
