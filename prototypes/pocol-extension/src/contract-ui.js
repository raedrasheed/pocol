import { NETWORK } from './lib/studio-network.js';
import { importKeyfile, exportKeyfile } from './lib/studio-keyfile.js';
import { addressOf, signTx, LOCK_MS } from './lib/wallet.js';
import { prepareContractReview, submitContractReview, checkContractReceipt } from './lib/contracts.js';
import { request as transportRequest, Pool, TRUST_POOL } from './lib/transport.js';

const $ = id => document.getElementById(id);
const JOB_PREFIX = 'ide:job:', STUDIO_PREFIX = 'studio:job:', VAULT_KEY = 'studio:wallet';
const jobId = location.hash.slice(1), storageKey = JOB_PREFIX + jobId;
const allowedOrigins = new Set(['https://ide.pocol.net', 'https://pocol-ide.mystic-gecko-9511.chatgpt.site']);
const pool = new Pool(TRUST_POOL); let rpcId = 0;
let job = null, review = null, secret = null, until = 0, busy = false, walletBusy = false, walletEpoch = 0, controller = null, writes = Promise.resolve(), storageFailure = false, checkingReceipt = false, nextReceiptCheck = 0;
const rpc = (method, params) => transportRequest(NETWORK.endpoint, method, params, {pool, deadline: 30000, id: ++rpcId, signal: controller?.signal || null});
const stringify = value => JSON.stringify(value, (_, item) => typeof item === 'bigint' ? item.toString() : item, 2);
const errorMessage = error => String(error?.rpc?.message || error?.message || error).slice(0, 600);
const signedRecord = current => current?.checkpoint?.transactions?.contract || null;
const hasSigned = current => Object.keys(current?.checkpoint?.transactions || {}).length > 0;
const terminal = current => ['complete', 'cancelled'].includes(current?.status);
const currentAddress = () => secret ? addressOf(secret) : null;
function formatPCOL(value) {
  const n = BigInt(value || '0'), whole = n / 1000000000000000000n;
  const fraction = (n % 1000000000000000000n).toString().padStart(18, '0').replace(/0+$/, '');
  return `${whole}${fraction ? '.' + fraction : ''} PCOL`;
}
function status(message, type = '') { $('status').textContent = message; $('status').className = 'notice' + (type ? ` ${type}` : ''); $('status').hidden = !message; }
function setJob(patch) {
  // Commit memory only after durable storage succeeds. Keep a failed queue
  // blocked until reload so a lost signed checkpoint can never be broadcast.
  writes = writes.then(async () => {
    const snapshot = structuredClone({...job, ...patch});
    try { await chrome.storage.local.set({[storageKey]: snapshot}); }
    catch {
      storageFailure = true;
      $('fatal').hidden = false;
      $('fatal').textContent = 'Extension storage could not be updated. Further signing and broadcasting are paused. Reload this tab after resolving the storage problem.';
      throw new Error('Could not save this operation. Reload this tab after resolving the extension storage problem.');
    }
    job = snapshot;
  });
  return writes;
}
function checkedJob(value) {
  if (!value || value.id !== jobId || !allowedOrigins.has(value.origin) || !value.request || !['deploy', 'call'].includes(value.request.action)) throw new Error('This contract request is missing or invalid. Open it again from PoCol IDE.');
  return value;
}
function renderReview() {
  $('reviewDetails').hidden = !review;
  if (!review) return;
  $('from').textContent = review.from;
  $('toLabel').textContent = review.predictedAddress ? 'Expected contract address' : 'Contract';
  $('to').textContent = review.predictedAddress || review.to || review.tx.to || 'Contract creation';
  $('gas').textContent = BigInt(review.gas).toLocaleString('en-US');
  $('fee').textContent = formatPCOL(review.maxFeeWei);
  $('value').textContent = formatPCOL(review.valueWei);
  $('total').textContent = formatPCOL(review.maxCostWei);
  $('dataHash').textContent = review.calldataHash;
  $('bytecodeFingerprint').hidden = !review.bytecodeHash;
  $('bytecodeHash').textContent = review.bytecodeHash || '';
  $('dataSize').textContent = `${Number(review.dataBytes).toLocaleString('en-US')} bytes`;
  $('calldata').textContent = review.tx.data;
  $('transaction').textContent = stringify(review.tx);
}
function render() {
  const signed = !!signedRecord(job), done = terminal(job), unlocked = !!secret && Date.now() < until;
  const expired = !!review && Date.now() >= review.expiresAt;
  $('walletAddress').textContent = unlocked ? currentAddress() : 'Wallet locked';
  $('unlock').disabled = busy || walletBusy || storageFailure || !job;
  $('keyfile').disabled = busy || walletBusy || !job;
  $('password').disabled = busy || walletBusy || !job;
  $('lock').disabled = !secret && !walletBusy;
  $('prepare').disabled = busy || walletBusy || storageFailure || !unlocked || !job || done || signed;
  $('prepare').textContent = review && !signed ? 'Refresh transaction review' : 'Prepare transaction review';
  $('consent').disabled = busy || walletBusy || storageFailure || !unlocked || !review || done || (!signed && expired);
  $('confirm').disabled = busy || walletBusy || checkingReceipt || storageFailure || !unlocked || !review || done || !$('consent').checked || (!signed && expired);
  $('confirm').textContent = signed ? 'Resume saved transaction' : 'Confirm & sign transaction';
  $('pause').disabled = !busy;
  $('cancel').disabled = busy || walletBusy || storageFailure || !job || done || hasSigned(job);
  $('consentText').textContent = signed ? 'I approve checking and, if needed, resending this exact saved signed transaction. No replacement transaction will be signed.' : 'I approve this contract transaction and its displayed value and maximum fee.';
  if (done) $('reviewNote').textContent = job.status === 'cancelled' ? 'This unsigned request was cancelled.' : 'This operation is complete. Its transaction record is retained.';
  else if (signed) $('reviewNote').textContent = 'The extension checks this saved transaction’s receipt automatically. Resume may resend the exact same signed bytes; its original fee cap remains unchanged.';
  else if (review) $('reviewNote').textContent = expired ? 'This fee review expired. Prepare a fresh review before signing.' : `Review expires in ${Math.max(0, Math.ceil((review.expiresAt - Date.now()) / 1000))} seconds. The actual fee may be lower.`;
  else $('reviewNote').textContent = 'Unlock your wallet to check the network, estimate gas, and prepare a fee limit.';
}
function lock() {
  walletEpoch++;
  if (secret) secret.fill(0);
  secret = null; until = 0; $('password').value = ''; $('consent').checked = false;
  if (!signedRecord(job)) { review = null; renderReview(); }
  controller?.abort(); render();
}
function checkEpoch(epoch) { if (epoch !== walletEpoch) throw new Error('Wallet action cancelled because the wallet was locked or changed.'); }
async function walletAction(action) {
  if (busy || walletBusy || storageFailure) return;
  walletBusy = true; const epoch = ++walletEpoch;
  if (secret) secret.fill(0);
  secret = null; until = 0;
  $('consent').checked = false;
  if (!signedRecord(job)) { review = null; renderReview(); }
  render(); $('walletNote').textContent = 'Unlocking your wallet locally…';
  try { await action(epoch); checkEpoch(epoch); $('walletNote').textContent = 'Unlocked locally. This wallet locks after 15 minutes or when you close the tab.'; }
  catch (error) { $('walletNote').textContent = errorMessage(error); }
  finally { walletBusy = false; render(); }
}
$('unlock').onclick = () => walletAction(async epoch => {
  const password = $('password').value;
  if (!password) throw new Error('Enter your wallet password.');
  const file = $('keyfile').files[0]; let bytes, key;
  try {
    if (file) {
      if (file.size > 16384) throw new Error('Wallet file exceeds the supported size.');
      bytes = new Uint8Array(await file.arrayBuffer());
    } else {
      const vault = (await chrome.storage.local.get(VAULT_KEY))[VAULT_KEY];
      checkEpoch(epoch);
      if (!vault) throw new Error('Choose your existing PoCol wallet file first.');
      bytes = new TextEncoder().encode(JSON.stringify(vault));
    }
    key = await importKeyfile(bytes, password); checkEpoch(epoch);
    if (job?.walletAddress && hasSigned(job) && addressOf(key).toLowerCase() !== job.walletAddress.toLowerCase()) throw new Error('Unlock the original signing wallet to resume this saved transaction.');
    if (file) {
      const vault = await exportKeyfile(key, password); checkEpoch(epoch);
      await chrome.storage.local.set({[VAULT_KEY]: vault}); checkEpoch(epoch);
      $('keyfile').value = '';
    }
    if (secret) secret.fill(0);
    secret = key; key = null; until = Date.now() + LOCK_MS; $('password').value = '';
  } finally { bytes?.fill(0); key?.fill(0); }
});
$('lock').onclick = lock;
$('consent').onchange = render;
$('pause').onclick = () => controller?.abort();

async function underJobLock(action) {
  if (!navigator.locks) throw new Error('This browser cannot safely coordinate wallet transactions. Use a current Chrome release.');
  return navigator.locks.request(`pocol:job:${jobId}`, {mode: 'exclusive', ifAvailable: true}, async lease => {
    if (!lease) throw new Error('This contract request is active in another extension tab.');
    job = checkedJob((await chrome.storage.local.get(storageKey))[storageKey]);
    if (terminal(job)) { renderResult(job.result); throw new Error(job.status === 'cancelled' ? 'This request was cancelled.' : 'This request is already complete.'); }
    return action();
  });
}
function showEarlierJob(key, other) {
  const link = document.createElement('a');
  link.href = chrome.runtime.getURL(key.startsWith(STUDIO_PREFIX) ? 'publisher.html' : 'contract.html') + '#' + other.id;
  link.textContent = 'Open the earlier operation';
  $('earlierJob').replaceChildren(link); $('earlierJob').hidden = false;
}
async function ensureAccountAvailable(address) {
  const unresolvedForAccount = ([key, other]) => (key.startsWith(STUDIO_PREFIX) || key.startsWith(JOB_PREFIX)) && key !== storageKey && other?.walletAddress?.toLowerCase() === address.toLowerCase() && !terminal(other) && hasSigned(other);
  const all = await chrome.storage.local.get(null);
  for (const [key, other] of Object.entries(all).filter(unresolvedForAccount)) {
    if (!key.startsWith(JOB_PREFIX) || !signedRecord(other)) continue;
    // A previous contract may already be included after its tab was closed.
    // Reconcile only through the core's signature/network/canonical checks.
    try {
      await navigator.locks.request(`pocol:job:${key.slice(JOB_PREFIX.length)}`, {mode: 'exclusive', ifAvailable: true}, async lease => {
        if (!lease) return;
        const current = (await chrome.storage.local.get(key))[key];
        if (!unresolvedForAccount([key, current]) || !signedRecord(current)) return;
        const result = await checkContractReceipt({request: current.request, checkpoint: current.checkpoint, rpc});
        if (!['included', 'reverted'].includes(result.state)) return;
        const checkpoint = structuredClone(current.checkpoint);
        checkpoint.transactions.contract.receipt = result.receipt;
        await chrome.storage.local.set({[key]: {...current, checkpoint, status: 'complete', result, error: null, progress: {phase: result.state, transactionHash: result.hash}}});
      });
    } catch { /* Network/storage failure leaves the prior job unresolved. */ }
  }
  const refreshed = await chrome.storage.local.get(null);
  const unresolved = Object.entries(refreshed).find(unresolvedForAccount);
  if (unresolved) { showEarlierJob(...unresolved); throw new Error('This wallet has an unresolved signed transaction in another operation. Resume that operation first.'); }
}

async function refreshPendingReceipt() {
  if (busy || walletBusy || checkingReceipt || storageFailure || !job || terminal(job) || !signedRecord(job) || !navigator.locks) return;
  checkingReceipt = true; render();
  try {
    await navigator.locks.request(`pocol:job:${jobId}`, {mode: 'exclusive', ifAvailable: true}, async lease => {
      if (!lease || busy || storageFailure) return;
      const current = checkedJob((await chrome.storage.local.get(storageKey))[storageKey]);
      if (terminal(current)) { job = current; renderResult(job.result); return; }
      if (!signedRecord(current)) return;
      const result = await checkContractReceipt({request: current.request, checkpoint: current.checkpoint, rpc});
      job = current; review = current.checkpoint.review;
      if (['included', 'reverted'].includes(result.state)) {
        const checkpoint = structuredClone(current.checkpoint);
        checkpoint.transactions.contract.receipt = result.receipt;
        await setJob({checkpoint, status: 'complete', result, error: null, progress: {phase: result.state, transactionHash: result.hash}});
        $('consent').checked = false; renderResult(result);
      } else if (job.status !== 'pending' || job.result?.hash !== result.hash) {
        await setJob({status: 'pending', result, progress: {phase: 'pending', transactionHash: result.hash}});
        renderResult(result);
      }
    });
  } catch { /* A missing response is not a failed transaction or an inclusion. */ }
  finally { checkingReceipt = false; render(); }
}
async function underAccountLock(address, action) {
  return navigator.locks.request(`pocol:publish:${NETWORK.chainId}:${address.toLowerCase()}`, {mode: 'exclusive', ifAvailable: true}, async lease => {
    if (!lease) throw new Error('Another extension tab is using this wallet. Finish or pause that operation first.');
    await ensureAccountAvailable(address);
    return action();
  });
}
$('prepare').onclick = async () => {
  if (busy || walletBusy || storageFailure || !secret || Date.now() >= until || terminal(job) || signedRecord(job)) return;
  const epoch = walletEpoch, address = currentAddress(); busy = true; controller = new AbortController(); $('consent').checked = false; $('earlierJob').hidden = true; render(); status('Checking PoCol and preparing the transaction…');
  try {
    await underJobLock(async () => {
      if (signedRecord(job)) throw new Error('A signed transaction was saved in another tab. Reload this page to resume it.');
      await underAccountLock(address, async () => {
        const next = await prepareContractReview({request: job.request, rpc, address});
        checkEpoch(epoch);
        if (!secret || Date.now() >= until || controller.signal.aborted) throw new Error('Wallet locked or preparation paused.');
        review = next;
        await setJob({status: 'awaitingApproval', review: next, walletAddress: address, error: null});
        renderReview(); status('Transaction ready. Check the address, value and maximum fee before approving.');
      });
    });
  } catch (error) { review = signedRecord(job) ? job.checkpoint.review : null; renderReview(); status(errorMessage(error), 'error'); }
  finally { busy = false; controller = null; render(); }
};
$('confirm').onclick = async () => {
  if (busy || walletBusy || checkingReceipt || storageFailure || !secret || Date.now() >= until || !review || terminal(job) || !$('consent').checked) return;
  if (!signedRecord(job) && Date.now() >= review.expiresAt) { render(); return; }
  const address = currentAddress(), epoch = walletEpoch;
  busy = true; until = Date.now() + LOCK_MS; controller = new AbortController(); $('earlierJob').hidden = true; render();
  status(signedRecord(job) ? 'Checking the saved transaction on PoCol…' : 'Verifying your approved transaction…');
  try {
    await underJobLock(async () => underAccountLock(address, async () => {
      checkEpoch(epoch);
      if (signedRecord(job)) review = job.checkpoint.review;
      if (!review || review.from.toLowerCase() !== address.toLowerCase()) throw new Error('The signing wallet changed. Prepare a new review with the correct wallet.');
      await setJob({status: 'submitting', walletAddress: address, error: null});
      const result = await submitContractReview({
        request: job.request, review, rpc, address, signal: controller.signal,
        sign: async tx => {
          checkEpoch(epoch);
          if (!secret || Date.now() >= until || controller.signal.aborted) throw new Error('Wallet locked or transaction paused before signing.');
          return signTx(secret, tx);
        },
        loadCheckpoint: async () => job.checkpoint || null,
        saveCheckpoint: async checkpoint => {
          await setJob({checkpoint, progress: {phase: 'signed', transactionHash: checkpoint.transactions?.contract?.hash || null}});
          if (checkpoint.transactions?.contract) { review = checkpoint.review; status('Signed transaction saved on this device. Waiting for its network receipt…'); }
        },
      });
      await setJob({status: ['included', 'reverted'].includes(result.state) ? 'complete' : 'pending', result, error: null, progress: {phase: result.state, transactionHash: result.hash}});
      renderResult(result);
    }));
  } catch (error) {
    const message = errorMessage(error), record = signedRecord(job);
    // Another trusted tab may have completed the operation before we obtained
    // its lock. Never replace that terminal state with a stale pending state.
    if (terminal(job)) {
      renderResult(job.result);
      if (job.status === 'cancelled') status('This request was cancelled before signing.');
      return;
    }
    if (record) review = job.checkpoint.review;
    const result = record ? {state: 'pending', hash: record.hash, address: job.checkpoint.review.predictedAddress, from: job.checkpoint.review.from, receipt: null} : null;
    await setJob({status: record ? 'pending' : 'awaitingApproval', result, error: message, progress: record ? {phase: 'pending', transactionHash: record.hash} : null}).catch(() => {});
    if (result) renderResult(result);
    status(message + (record ? '\nThe signed transaction remains saved. Resume this operation to check its receipt or resend the exact same bytes.' : '\nNo broadcast was confirmed. Prepare a fresh review before trying again.'), record ? 'warning' : 'error');
    if (!record) { review = null; renderReview(); }
  } finally { busy = false; controller = null; $('consent').checked = false; render(); }
};
$('cancel').onclick = async () => {
  if (busy || walletBusy || storageFailure || !job || terminal(job) || hasSigned(job)) return;
  busy = true; render();
  try {
    await underJobLock(async () => {
      if (hasSigned(job)) throw new Error('A transaction has already been signed. Retain this operation and resume it.');
      await setJob({status: 'cancelled', cancelledAt: Date.now(), review: null, error: null});
      review = null; renderReview(); status('Request cancelled before any transaction was signed.', 'success');
    });
  } catch (error) { status(errorMessage(error), 'error'); }
  finally { busy = false; $('consent').checked = false; render(); }
};
function renderResult(result) {
  if (!result?.hash) return;
  $('result').hidden = false; $('resultHash').textContent = result.hash;
  $('contractResult').hidden = !result.address; $('resultAddress').textContent = result.address || '';
  $('resultAddressLabel').textContent = result.state === 'included' ? 'Contract address' : 'Expected contract address (pending)';
  if (result.state === 'included') {
    $('resultTitle').textContent = 'Transaction included on PoCol';
    $('resultNote').textContent = 'This receipt is currently on the canonical chain. Inclusion is not a promise of irreversible finality.';
    status('Transaction included. You can return to PoCol IDE.', 'success');
  } else if (result.state === 'reverted') {
    $('resultTitle').textContent = 'Transaction reverted';
    $('resultNote').textContent = 'The network included the transaction, but contract execution reverted. Gas fees may still have been charged.';
    status('The transaction reverted. Its record is retained; no replacement transaction was signed.', 'error');
  } else {
    $('resultTitle').textContent = 'Transaction still pending';
    $('resultNote').textContent = 'The signed transaction is retained and its receipt is checked automatically while this tab is open. Resume may resend the same bytes without creating a replacement.';
    status('No current receipt yet. Checking its status automatically; no new transaction will be signed.', 'warning');
  }
}
async function init() {
  if (!/^[a-f0-9-]{36}$/.test(jobId)) throw new Error('Open a contract transaction request from PoCol IDE.');
  job = checkedJob((await chrome.storage.local.get(storageKey))[storageKey]);
  review = signedRecord(job) ? job.checkpoint.review : null;
  $('requestTitle').textContent = job.request.title || 'Contract transaction';
  $('origin').textContent = job.origin;
  $('action').textContent = job.request.action === 'deploy' ? 'DEPLOY CONTRACT' : 'CALL CONTRACT';
  $('method').textContent = job.request.action === 'deploy' ? 'Deploy creation bytecode' : job.request.method || 'Contract call';
  $('destination').textContent = job.request.to || 'New contract address, derived from your account and nonce';
  $('intent').textContent = stringify(job.request);
  const vault = (await chrome.storage.local.get(VAULT_KEY))[VAULT_KEY];
  if (vault) $('walletNote').textContent = 'Your encrypted Studio wallet is already saved. Enter its password without choosing another file.';
  renderReview();
  if (review) { status('A signed transaction is saved. Its network receipt is checked automatically. Unlock the original wallet only if you want to resend the same transaction.', 'warning'); }
  if (job.error) status(job.error + (hasSigned(job) ? '\nResume the saved transaction; do not create a replacement.' : ''), 'warning');
  if (job.status === 'cancelled') status('This request was cancelled before signing.');
  renderResult(job.result); render();
}
setInterval(() => {
  if (secret && Date.now() >= until) lock(); else render();
  if (Date.now() >= nextReceiptCheck) { nextReceiptCheck = Date.now() + 3000; return refreshPendingReceipt(); }
}, 1000);
window.addEventListener('beforeunload', event => { if (busy) { event.preventDefault(); event.returnValue = ''; } });
window.addEventListener('pagehide', lock);
init().catch(error => { job = null; $('fatal').hidden = false; $('fatal').textContent = errorMessage(error); render(); });
