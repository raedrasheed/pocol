// Real worker, mocked Chrome: deterministic wallet lifecycle races, no network or real keys.
import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { build } from '../build.mjs';
import { installChromeMock, loadWorker } from './chrome_mock.mjs';

const dist = build().dist;
// RecordStore schedules a late background sweep at worker boot. These tests do
// not use that sweep; release its timers when the component scenarios finish.
const later = [], realTimeout = globalThis.setTimeout;
globalThis.setTimeout = (fn, ms, ...args) => {
  const timer = realTimeout(fn, ms, ...args);
  if (ms >= 10000) later.push(timer);
  return timer;
};
after(() => { globalThis.setTimeout = realTimeout; for (const timer of later) clearTimeout(timer); });
const create = (byte, extra = {}) => ({ type: 'wallet.create', password: 'lifecycle-test-password', seedHex: '0x' + byte.repeat(32), ...extra });
let generation = 0;
async function world() {
  const mock = installChromeMock();
  await loadWorker(dist, 'lifecycle-' + ++generation);
  return mock;
}

test('concurrent initial creates cannot replace a wallet without explicit confirmation', async () => {
  const mock = await world();
  const results = await Promise.all([mock.send(create('41')), mock.send(create('42'))]);
  assert.equal(results.filter(r => r.ok).length, 1);
  assert.match(results.find(r => !r.ok).error.message, /already (exists|in progress)/);
});

test('lock during creation encryption wins and preserves the existing encrypted wallet', async () => {
  const mock = await world();
  assert.equal((await mock.send(create('41'))).ok, true);
  const before = (await mock.storage.get('wallet')).wallet;
  const original = crypto.subtle.encrypt;
  let release, entered;
  const gate = new Promise(resolve => { release = resolve; });
  const atEncryption = new Promise(resolve => { entered = resolve; });
  crypto.subtle.encrypt = async function (...args) {
    entered(); await gate;
    return original.apply(this, args);
  };
  try {
    const pending = mock.send(create('42', { replace: true }));
    await atEncryption;
    await mock.send({ type: 'wallet.lock' });
    release();
    assert.equal((await pending).ok, false);
    assert.deepEqual((await mock.storage.get('wallet')).wallet, before);
    assert.equal((await mock.send({ type: 'wallet.status' })).unlocked, false);
  } finally { release(); crypto.subtle.encrypt = original; }
});

test('lock during wallet persistence leaves the newly persisted wallet locked', async () => {
  const mock = await world();
  const original = mock.storage.set;
  let release, entered;
  const gate = new Promise(resolve => { release = resolve; });
  const atWrite = new Promise(resolve => { entered = resolve; });
  mock.storage.set = async obj => {
    if (obj.wallet) { entered(); await gate; }
    return original(obj);
  };
  try {
    const pending = mock.send(create('41'));
    await atWrite;
    await mock.send({ type: 'wallet.lock' });
    release();
    await pending;
    assert.ok((await mock.storage.get('wallet')).wallet);
    assert.equal((await mock.send({ type: 'wallet.status' })).unlocked, false);
  } finally { release(); mock.storage.set = original; }
});
