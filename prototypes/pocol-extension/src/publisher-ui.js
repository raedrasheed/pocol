import { NETWORK } from './lib/studio-network.js';
import { JOB_PREFIX, base64Bytes } from './lib/studio-bridge.js';
import { importKeyfile, exportKeyfile, validSecret } from './lib/studio-keyfile.js';
import { addressOf, signTx, LOCK_MS } from './lib/wallet.js';
import { prepareSite, quotePublish, publishSite, readNetworkFees, remainingTransactions } from './lib/publisher.js';
import { KeyLease, JOB_DEADLINE_DEFAULT_MS, JOB_DEADLINE_MAX_MS } from './lib/lease.js';
import { isForeignJob, blocksAccount, archiveJob, networkOf, filesKey, rawKey, removeSideKeys } from './lib/jobs.js';
import { request, Pool, TRUST_POOL } from './lib/transport.js';

const $ = (id) => document.getElementById(id);
const vaultKey = 'studio:wallet';
// Sites W4: the key lease (lease.js) replaces the fixed 15-minute unlock for an approved publication.
const lease = new KeyLease({lockMs:LOCK_MS});
let job = null, plan = null, quote = null, secret = null, busy = false, controller = null, writes = Promise.resolve(), walletBusy = false, walletEpoch = 0, foreign = false;
const jobId = location.hash.slice(1), storageKey = JOB_PREFIX + jobId;
const pool = new Pool(TRUST_POOL); let rpcId = 0;
const rpc = (method, values) => request(NETWORK.endpoint, method, values, {pool, deadline:30000, id:++rpcId});
const errorMessage = (e) => (e?.rpc?.message || e?.message || String(e)).slice(0, 500);
function setJob(patch) {
  Object.assign(job, patch); const snapshot = structuredClone(job);
  writes = writes.then(() => chrome.storage.local.set({ [storageKey]: snapshot })); return writes;
}
// Each signed raw transaction is written once under its own key (lib/jobs.js rawKey), before the checkpoint names it.
const rawStore = {
  put: async (hash, raw) => { await chrome.storage.local.set({[rawKey(jobId, hash)]: raw}); },
  get: async (hash) => (await chrome.storage.local.get(rawKey(jobId, hash)))[rawKey(jobId, hash)] ?? null,
};
const formatFee = (s) => { const n = BigInt(s), whole = n / 1000000000000000000n, frac = (n % 1000000000000000000n).toString().padStart(18,'0').slice(0,8).replace(/0+$/,''); return `${whole}${frac ? '.'+frac : ''} PCOL`; };
const formatGwei = (s) => { const n = BigInt(s), whole = n / 1000000000n, frac = (n % 1000000000n).toString().padStart(9,'0').slice(0,4).replace(/0+$/,''); return `${whole}${frac ? '.'+frac : ''} gwei`; };
const size = (n) => n < 1024 ? n + ' B' : n < 1048576 ? (n / 1024).toFixed(1) + ' KiB' : (n / 1048576).toFixed(2) + ' MiB';
const deadlineMs = () => { const h = Number($('deadline').value); return Number.isFinite(h) && h > 0 ? Math.min(h * 3600000, JOB_DEADLINE_MAX_MS) : JOB_DEADLINE_DEFAULT_MS; };
function applyLanguage() {
  document.documentElement.lang = 'en'; document.documentElement.dir = 'ltr';
  $('networkNote').textContent = 'RN · Trusted remote node — network identity and file hashes verified';
  renderWallet();
}
function renderWallet() {
  const unlocked = !!secret && lease.unlocked;
  $('walletAddress').textContent = unlocked ? addressOf(secret) : 'Wallet locked';
  $('publish').disabled = busy || walletBusy || foreign || !unlocked || !plan || !quote || !$('consent').checked || job?.status === 'complete' || job?.status === 'cancelled';
  $('pause').disabled = !busy; $('unlock').disabled = busy || walletBusy; $('create').disabled = busy || walletBusy; $('keyfile').disabled = busy || walletBusy;
  $('password').disabled = busy || walletBusy; $('cancel').disabled = busy || job?.status === 'complete';
  $('refreshFees').disabled = busy || foreign || !plan; $('economy').disabled = busy || foreign; $('tip').disabled = busy || foreign; $('deadline').disabled = busy || foreign;
}
function lock() { walletEpoch++; if (secret) secret.fill(0); secret = null; lease.lock(); $('password').value = ''; if (controller) controller.abort(); renderWallet(); }
function checkWalletEpoch(epoch) { if(epoch !== walletEpoch) throw new Error('Wallet action cancelled by locking'); }
function adopt(bytes, epoch) { checkWalletEpoch(epoch); if (secret) secret.fill(0); secret = bytes; lease.unlock(); $('password').value = ''; renderWallet(); }
async function walletAction(action) {
  if (busy || walletBusy) return;
  walletBusy = true; const epoch = ++walletEpoch; renderWallet();
  try { $('walletNote').textContent = 'Unlocking…'; await action(epoch); checkWalletEpoch(epoch); $('walletNote').textContent = 'Wallet unlocked locally. It locks after 15 minutes, or at the deadline of an approved publication.'; }
  catch (e) { $('walletNote').textContent = errorMessage(e); }
  finally { walletBusy = false; renderWallet(); }
}
$('unlock').onclick = () => walletAction(async (epoch) => {
  const password = $('password').value; if (!password) throw new Error('Enter your password');
  const file = $('keyfile').files[0]; let bytes;
  if (file) { if (file.size > 16384) throw new Error('Wallet file too large'); bytes = new Uint8Array(await file.arrayBuffer()); }
  else { const vault = (await chrome.storage.local.get(vaultKey))[vaultKey]; if (!vault) throw new Error('Choose your encrypted wallet file first'); bytes = new TextEncoder().encode(JSON.stringify(vault)); }
  const key = await importKeyfile(bytes, password); bytes.fill(0);
  try { checkWalletEpoch(epoch); if (file) { const vault = await exportKeyfile(key, password); checkWalletEpoch(epoch); await chrome.storage.local.set({[vaultKey]:vault}); $('keyfile').value=''; } adopt(key,epoch); }
  catch (e) { key.fill(0); throw e; }
});
$('create').onclick = () => walletAction(async (epoch) => {
  const password = $('password').value; let key;
  do { key = crypto.getRandomValues(new Uint8Array(32)); try { validSecret(key); break; } catch { key.fill(0); } } while (true);
  try {
    const vault = await exportKeyfile(key,password); checkWalletEpoch(epoch); const blob = new Blob([JSON.stringify(vault,null,2)], {type:'application/json'});
    const url = URL.createObjectURL(blob), a = document.createElement('a'); a.href=url; a.download='PoCol-Studio-Wallet.json'; a.click(); setTimeout(()=>URL.revokeObjectURL(url),1000);
    await chrome.storage.local.set({[vaultKey]:vault}); adopt(key,epoch);
  } catch(e) { key.fill(0); throw e; }
});
$('lock').onclick = lock;
$('consent').onchange = renderWallet;
$('pause').onclick = () => controller?.abort();
// Sites W4: the fees are quoted from the node after connecting (no fixed caps). A saved publication is quoted for its
// remaining transactions only; approving the quote approves a new budget and ceiling for those.
function renderQuote(note = '') {
  if (!quote) { $('fee').textContent = '—'; $('feeDetails').textContent = note; return; }
  $('fee').textContent = formatFee(quote.budgetWei);
  $('feeDetails').textContent = `Expected cost about ${formatFee(quote.expectedWei)} · ceiling ${formatGwei(quote.ceilingWei)} per gas · tip ${formatGwei(quote.tipWei)} · next base fee ${formatGwei(quote.baseNextWei)}`
    + (quote.economy ? ' · economy pacing (one transaction every second block, about half speed)' : '')
    + (quote.quotedTransactions < quote.transactionCount ? ` · covers the ${quote.quotedTransactions} remaining transactions` : '') + (note ? '\n' + note : '');
  const end = new Date(Date.now() + deadlineMs());
  $('deadlineNote').textContent = `After approval the wallet stays unlocked for this publication only, at most until ${end.toLocaleTimeString()}; Lock stops it at once.`;
}
async function refreshQuote(note = '') {
  quote = null; $('consent').checked = false; renderWallet(); $('fee').textContent = 'Reading network fees…';
  try {
    const cid = await rpc('eth_chainId', []), status = await rpc('pocol_status', []);
    if (BigInt(cid) !== BigInt(NETWORK.chainId) || status?.genesisHash?.toLowerCase() !== NETWORK.genesisHash.toLowerCase()) throw new Error('Network identity mismatch');
    const fees = await readNetworkFees(rpc), tipGwei = $('tip').value.trim();
    const tip = tipGwei ? BigInt(Math.round(Number(tipGwei) * 1e9)).toString() : fees.tip;
    if (tipGwei && !(Number(tipGwei) >= 0 && Number(tipGwei) <= 2)) throw new Error('The tip must be between 0 and 2 gwei');
    const txs = job.checkpoint ? remainingTransactions(plan, job.checkpoint, {site:job.request.site}) : undefined;
    // Every transaction is signed already: resume under the saved approval (or, for a schema-1 checkpoint, an approval
    // that allows no further signature), so the remaining checks finish without a new budget.
    if (txs && !txs.length) quote = {...quotePublish(plan,{site:job.request.site}), quotedTransactions:0, budgetWei:'0', expectedWei:'0', ceilingWei:'1', tipWei:'0', baseNextWei:fees.baseNext,
      approval:job.approval ?? {budgetWei:'1', ceilingWei:'1', tipWei:'0', economy:false, schedule:{}}};
    else quote = quotePublish(plan, {site:job.request.site, fees, tip, economy:$('economy').checked, txs});
    $('transactions').textContent = quote.transactionCount; renderQuote(note);
  } catch (e) { quote = null; renderQuote('Fees could not be read: ' + errorMessage(e)); }
  renderWallet();
}
$('refreshFees').onclick = () => refreshQuote();
$('economy').onchange = () => refreshQuote();
$('tip').onchange = () => refreshQuote();
$('deadline').onchange = () => renderQuote();
$('cancel').onclick = async () => {
  if (!job || busy) return;
  try { await navigator.locks.request(`pocol:job:${jobId}`,{mode:'exclusive',ifAvailable:true},async(jobLock)=> {
    if(!jobLock)throw new Error('This publication is active in another tab.');
    job=(await chrome.storage.local.get(storageKey))[storageKey]; if(!job)throw new Error('Publication no longer exists');
    if(job.status==='complete')throw new Error('This publication is already complete');
    // A job of another network cannot be reconciled here: it is archived (read-only) without receipts.
    if(isForeignJob(job, NETWORK)){await archiveJob(chrome.storage.local,storageKey,job,'cancelled-foreign-network');foreign=true;$('status').textContent='This publication belonged to another network. It was archived read-only; nothing was signed or sent.';return;}
    const records=Object.values(job.checkpoint?.transactions || {});
    if(records.length){
      const cid=await rpc('eth_chainId',[]),identity=await rpc('pocol_status',[]);
      if(BigInt(cid)!==BigInt(NETWORK.chainId)||identity.genesisHash?.toLowerCase()!==NETWORK.genesisHash)throw new Error('Network identity mismatch');
      for(const record of records){
        const receipt=await rpc('eth_getTransactionReceipt',[record.hash]);
        if(!receipt || receipt.transactionHash?.toLowerCase()!==record.hash || !['0x0','0x1'].includes(receipt.status))throw new Error('Unresolved transactions remain. Keep this job and resume it.');
        const block=await rpc('eth_getBlockByNumber',[receipt.blockNumber,false]);
        if(!block?.hash || block.hash.toLowerCase()!==receipt.blockHash?.toLowerCase())throw new Error('Receipt is not on the current chain');
      }
      if(!confirm('All signed transactions are currently confirmed. Abandoning this operation will not remove stored files or refund fees. Archive it?'))return;
    }
    await setJob({status:'cancelled',archivedAt:Date.now()}); await removeSideKeys(chrome.storage.local,storageKey,{files:false});
    $('status').textContent=records.length?'Publication archived; transaction history retained.':'Request cancelled without signing transactions.';
  }); } catch(e){$('status').textContent=errorMessage(e);}
  $('consent').checked=false; renderWallet();
};
async function runPublication() {
  if (!secret || !lease.unlocked) { lock(); throw new Error('Unlock the wallet again'); }
  if (!quote?.approval) throw new Error('Read the network fees and approve them first');
  const address = addressOf(secret);
  if (!navigator.locks) throw new Error('Your browser cannot safely coordinate publication tabs');
  return navigator.locks.request(`pocol:job:${jobId}`, {mode:'exclusive',ifAvailable:true}, async (jobLease) => {
    if(!jobLease)throw new Error('This job is already publishing in another tab.');
    return navigator.locks.request(`pocol:publish:${NETWORK.chainId}:${address}`, {mode:'exclusive',ifAvailable:true}, async (accountLease) => {
    if (!accountLease) throw new Error('Another tab is publishing from this wallet. Finish or pause it first.');
    // A second tab may have completed or advanced this operation while this tab waited.
    const latest=(await chrome.storage.local.get(storageKey))[storageKey]; if(!latest)throw new Error('Publication no longer exists');
    job=latest;
    if(job.status==='complete'){$('resultAddress').textContent=job.result.site;$('success').hidden=false;return;}
    if(job.status==='cancelled')throw new Error('Publication was cancelled');
    if(isForeignJob(job, NETWORK))throw new Error('This publication belongs to another network. Cancel archives it.');
    if(job.walletAddress && job.walletAddress!==address && Object.keys(job.checkpoint?.transactions || {}).length)throw new Error('Resume with the original publishing wallet');
    const all = await chrome.storage.local.get(null);
    // Jobs of another network (another chain id or genesis) never block this wallet (lib/jobs.js).
    const unresolved = Object.entries(all).find(([k,v]) => (k.startsWith(JOB_PREFIX)||k.startsWith('ide:job:')) && v?.id !== job.id && blocksAccount(v, address, NETWORK));
    if (unresolved) {
      const link = document.createElement('a'); link.href=chrome.runtime.getURL(unresolved[0].startsWith('ide:job:')?'contract.html':'publisher.html')+'#'+unresolved[1].id; link.textContent='Resume the earlier operation';
      const holder=document.createElement('p');holder.id='resumeEarlier';holder.append(link);$('status').after(holder);
      throw new Error('An earlier publication has unresolved signed transactions. Resume it from the publication history first.');
    }
    const approval = quote.approval;
    try {
    await setJob({status:'publishing',walletAddress:address,network:networkOf(NETWORK),approval,error:null});
    lease.startJob(jobId, deadlineMs());
    const result = await publishSite({plan, rpc, rawStore, sign:async(tx)=> {
      if (!secret || !lease.unlocked || controller.signal.aborted) throw new Error('Wallet locked or publication paused');
      const signed = signTx(secret,tx); lease.signed(jobId); return signed;
    },address,chainId:NETWORK.chainId,genesisHash:NETWORK.genesisHash,site:job.request.site,
      approval,reapprove:!!job.checkpoint,state:job.checkpoint || null,signal:controller.signal,
      onCheckpoint:async(checkpoint)=>setJob({checkpoint}),
      onProgress:(event)=> {
        setJob({progress:event}).catch(()=>controller?.abort());
        const descriptions={upload:'Storing website chunks',broadcast:'Broadcasting transaction',confirmed:'Transaction confirmed',complete:'Publication complete',
          sweep:'Checking that every chunk is at the safe height',restore:'Storing again a chunk the network no longer holds',
          warning:event.message,capsAlarm:'The network is above 80 % of a capacity cap ('+(event.alarms||[]).join(', ')+'); this site still fits'};
        $('status').textContent=descriptions[event.phase] || event.phase;
        if(event.transactionHash) $('status').textContent+='\n'+event.transactionHash;
        if(event.total){$('progress').max=event.total;$('progress').value=event.completed || 0;}
      }
    });
    await setJob({status:'complete',result,error:null}); await removeSideKeys(chrome.storage.local,storageKey,{files:false});
    $('status').textContent='Published, read back and verified the current version on the network.';
    $('resultAddress').textContent=result.site; $('success').hidden=false;
    } catch(e) {
      await setJob({status:'paused',error:errorMessage(e)}).catch(()=>{});
      // A fee rise pauses before signing: quote the remaining transactions for a new approval.
      if (e?.code === 'PUBLISH_FEE_ROSE') await refreshQuote(e.message);
      throw e;
    } finally { lease.endJob(jobId); }
    });
  });
}
$('publish').onclick = async () => {
  if (busy || walletBusy || !plan || !secret || !quote || !$('consent').checked) return;
  $('resumeEarlier')?.remove();
  busy=true; controller=new AbortController(); $('progress').hidden=false; renderWallet();
  try { await runPublication(); }
  catch(e) {
    const error=errorMessage(e);
    $('status').textContent=error+'\n'+(e?.code === 'PUBLISH_FEE_ROSE' ? 'Review the new budget for the remaining transactions and confirm again.' : 'Reopen this job and confirm again to resume from its saved state.');
  } finally { busy=false; controller=null; $('progress').hidden=true; $('consent').checked=false; renderWallet(); }
};
$('openSite').onclick=()=>{if(job?.result?.site)chrome.tabs.create({url:chrome.runtime.getURL('viewer.html')+'#'+NETWORK.name+'/'+job.result.site+'/'});};
setInterval(()=>{if(secret&&!lease.unlocked)lock();},1000);
window.addEventListener('beforeunload',(e)=>{if(busy){e.preventDefault();e.returnValue='';}});
// Sites W4: the request's files are written once under their own key, so later job writes stay small.
async function loadFiles() {
  if (Array.isArray(job.request.files)) {
    await chrome.storage.local.set({[filesKey(jobId)]: job.request.files});
    const {files, ...rest} = job.request; await setJob({request:{...rest, filesKey:filesKey(jobId)}});
    return files;
  }
  const files = (await chrome.storage.local.get(filesKey(jobId)))[filesKey(jobId)];
  if (!Array.isArray(files)) throw new Error('The files of this publication are missing.');
  return files;
}
async function init() {
  applyLanguage();
  if (!/^[a-f0-9-]{36}$/.test(jobId)) throw new Error('Open a publication request from PoCol Studio.');
  job=(await chrome.storage.local.get(storageKey))[storageKey]; if(!job)throw new Error('Publication request not found.');
  $('deadline').value = String(JOB_DEADLINE_DEFAULT_MS / 3600000); $('deadline').max = String(JOB_DEADLINE_MAX_MS / 3600000);
  foreign = isForeignJob(job, NETWORK);
  const files = await loadFiles();
  plan=prepareSite(files.map(f=>({path:f.path,data:base64Bytes(f.base64)})),{entry:job.request.entry});
  const counts=quotePublish(plan,{site:job.request.site});
  $('siteTitle').textContent=job.request.title;
  $('origin').textContent=job.origin;
  $('siteTarget').textContent=job.request.site || 'Create a new website address';
  $('fileCount').textContent=plan.files.length; $('totalBytes').textContent=size(plan.totalBytes); $('transactions').textContent=counts.transactionCount;
  $('manifestHash').textContent=plan.manifestHash;
  $('files').replaceChildren(...plan.files.map(f=>{const li=document.createElement('li'),n=document.createElement('span'),z=document.createElement('span');n.textContent=f.path;z.textContent=size(f.size);li.append(n,z);return li;}));
  if(foreign){$('status').textContent='This publication belongs to another network and is kept read-only. Cancel archives it without contacting the network.';renderQuote();}
  else if(job.status==='complete'){$('resultAddress').textContent=job.result.site;$('success').hidden=false;$('status').textContent='This publication is complete.';}
  else { if(job.checkpoint)$('status').textContent='Saved progress found. Unlock the wallet and approve the remaining fees to resume.'; await refreshQuote(); }
  const vault=(await chrome.storage.local.get(vaultKey))[vaultKey]; if(vault)$('walletNote').textContent='An encrypted wallet is saved. Enter its password without selecting a file.';
  renderWallet();
}
init().catch(e=>{$('fatal').hidden=false;$('fatal').textContent=errorMessage(e);});
