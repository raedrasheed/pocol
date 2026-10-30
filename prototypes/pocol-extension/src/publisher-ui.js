import { NETWORK } from './lib/studio-network.js';
import { JOB_PREFIX, base64Bytes } from './lib/studio-bridge.js';
import { importKeyfile, exportKeyfile, validSecret } from './lib/studio-keyfile.js';
import { addressOf, signTx, LOCK_MS } from './lib/wallet.js';
import { prepareSite, quotePublish, publishSite } from './lib/publisher.js';
import { request, Pool, TRUST_POOL } from './lib/transport.js';

const $ = (id) => document.getElementById(id);
const vaultKey = 'studio:wallet';
const params = new URLSearchParams(location.search);
let language = params.get('lang') === 'en' ? 'en' : 'ar';
const t = (ar, en) => language === 'ar' ? ar : en;
let job = null, plan = null, quote = null, secret = null, until = 0, busy = false, controller = null, writes = Promise.resolve(), walletBusy = false, walletEpoch = 0;
const jobId = location.hash.slice(1), storageKey = JOB_PREFIX + jobId;
const pool = new Pool(TRUST_POOL); let rpcId = 0;
const rpc = (method, values) => request(NETWORK.endpoint, method, values, {pool, deadline:30000, id:++rpcId});
const errorMessage = (e) => (e?.rpc?.message || e?.message || String(e)).slice(0, 500);
function setJob(patch) {
  Object.assign(job, patch); const snapshot = structuredClone(job);
  writes = writes.then(() => chrome.storage.local.set({ [storageKey]: snapshot })); return writes;
}
const formatFee = (s) => { const n = BigInt(s), whole = n / 1000000000000000000n, frac = (n % 1000000000000000000n).toString().padStart(18,'0').slice(0,8).replace(/0+$/,''); return `${whole}${frac ? '.'+frac : ''} PoCol`; };
const size = (n) => n < 1024 ? n + ' B' : n < 1048576 ? (n / 1024).toFixed(1) + ' KiB' : (n / 1048576).toFixed(2) + ' MiB';
function applyLanguage() {
  document.documentElement.lang = language; document.documentElement.dir = language === 'ar' ? 'rtl' : 'ltr';
  for (const el of document.querySelectorAll('[data-ar][data-en]')) el.textContent = el.dataset[language];
  $('language').textContent = language === 'ar' ? 'English' : 'العربية';
  $('networkNote').textContent = t('RN · عقدة بعيدة موثوقة — تُفحص هوية الشبكة وبصمات الملفات', 'RN · Trusted remote node — network identity and file hashes verified');
  renderWallet();
}
function renderWallet() {
  $('walletAddress').textContent = secret ? addressOf(secret) : t('المحفظة مقفلة', 'Wallet locked');
  $('publish').disabled = busy || walletBusy || !secret || !plan || !$('consent').checked || job?.status === 'complete' || job?.status === 'cancelled';
  $('pause').disabled = !busy; $('unlock').disabled = busy || walletBusy; $('create').disabled = busy || walletBusy; $('keyfile').disabled = busy || walletBusy;
  $('password').disabled = busy || walletBusy; $('cancel').disabled = busy || job?.status === 'complete';
}
function lock() { walletEpoch++; if (secret) secret.fill(0); secret = null; until = 0; $('password').value = ''; if (controller) controller.abort(); renderWallet(); }
function checkWalletEpoch(epoch) { if(epoch !== walletEpoch) throw new Error('Wallet action cancelled by locking'); }
function adopt(bytes, epoch) { checkWalletEpoch(epoch); if (secret) secret.fill(0); secret = bytes; until = Date.now() + LOCK_MS; $('password').value = ''; renderWallet(); }
async function walletAction(action) {
  if (busy || walletBusy) return;
  walletBusy = true; const epoch = ++walletEpoch; renderWallet();
  try { $('walletNote').textContent = t('جارٍ فتح المحفظة…','Unlocking…'); await action(epoch); checkWalletEpoch(epoch); $('walletNote').textContent = t('تم فتح المحفظة محليًا. ستُقفل بعد 15 دقيقة.', 'Wallet unlocked locally. It locks after 15 minutes.'); }
  catch (e) { $('walletNote').textContent = errorMessage(e); }
  finally { walletBusy = false; renderWallet(); }
}
$('unlock').onclick = () => walletAction(async (epoch) => {
  const password = $('password').value; if (!password) throw new Error(t('أدخل كلمة المرور','Enter your password'));
  const file = $('keyfile').files[0]; let bytes;
  if (file) { if (file.size > 16384) throw new Error('Wallet file too large'); bytes = new Uint8Array(await file.arrayBuffer()); }
  else { const vault = (await chrome.storage.local.get(vaultKey))[vaultKey]; if (!vault) throw new Error(t('اختر ملف المحفظة المشفّر أولًا','Choose your encrypted wallet file first')); bytes = new TextEncoder().encode(JSON.stringify(vault)); }
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
$('language').onclick = () => { language = language === 'ar' ? 'en' : 'ar'; applyLanguage(); };
$('consent').onchange = renderWallet;
$('pause').onclick = () => controller?.abort();
$('cancel').onclick = async () => {
  if (!job || busy) return;
  try { await navigator.locks.request(`pocol:job:${jobId}`,{mode:'exclusive',ifAvailable:true},async(lease)=> {
    if(!lease)throw new Error(t('هذه العملية نشطة في نافذة أخرى.','This publication is active in another tab.'));
    job=(await chrome.storage.local.get(storageKey))[storageKey]; if(!job)throw new Error('Publication no longer exists');
    if(job.status==='complete')throw new Error('This publication is already complete');
    const records=Object.values(job.checkpoint?.transactions || {});
    if(records.length){
      const cid=await rpc('eth_chainId',[]),identity=await rpc('pocol_status',[]);
      if(BigInt(cid)!==BigInt(NETWORK.chainId)||identity.genesisHash?.toLowerCase()!==NETWORK.genesisHash)throw new Error('Network identity mismatch');
      for(const record of records){
        const receipt=await rpc('eth_getTransactionReceipt',[record.hash]);
        if(!receipt || receipt.transactionHash?.toLowerCase()!==record.hash || !['0x0','0x1'].includes(receipt.status))throw new Error(t('توجد معاملات غير محسومة. احتفظ بهذه العملية واستأنفها.','Unresolved transactions remain. Keep this job and resume it.'));
        const block=await rpc('eth_getBlockByNumber',[receipt.blockNumber,false]);
        if(!block?.hash || block.hash.toLowerCase()!==receipt.blockHash?.toLowerCase())throw new Error('Receipt is not on the current chain');
      }
      if(!confirm(t('جميع المعاملات الموقعة مؤكدة حاليًا. إنهاء هذه العملية لن يحذف الملفات المخزنة أو يعيد الرسوم. هل تريد أرشفتها؟','All signed transactions are currently confirmed. Abandoning this operation will not remove stored files or refund fees. Archive it?')))return;
    }
    await setJob({status:'cancelled',archivedAt:Date.now()}); $('status').textContent=records.length?t('أُرشفت العملية واحتُفظ بسجل معاملاتها.','Publication archived; transaction history retained.'):t('أُلغي الطلب دون توقيع معاملات.','Request cancelled without signing transactions.');
  }); } catch(e){$('status').textContent=errorMessage(e);}
  $('consent').checked=false; renderWallet();
};
function hasUnfinishedSigned(other) { return other.id !== job.id && other.walletAddress === addressOf(secret) && !['complete','cancelled'].includes(other.status) && Object.keys(other.checkpoint?.transactions || {}).length > 0; }
async function runPublication() {
  if (!secret || Date.now() >= until) { lock(); throw new Error(t('افتح المحفظة مجددًا','Unlock the wallet again')); }
  const address = addressOf(secret);
  if (!navigator.locks) throw new Error('Your browser cannot safely coordinate publication tabs');
  return navigator.locks.request(`pocol:job:${jobId}`, {mode:'exclusive',ifAvailable:true}, async (jobLease) => {
    if(!jobLease)throw new Error(t('هذه العملية مفتوحة للنشر في نافذة أخرى.','This job is already publishing in another tab.'));
    return navigator.locks.request(`pocol:publish:${NETWORK.chainId}:${address}`, {mode:'exclusive',ifAvailable:true}, async (lease) => {
    if (!lease) throw new Error(t('يوجد نشر آخر من هذه المحفظة. أكمله أو أوقفه أولًا.','Another tab is publishing from this wallet. Finish or pause it first.'));
    // A second tab may have completed or advanced this operation while this tab waited.
    const latest=(await chrome.storage.local.get(storageKey))[storageKey]; if(!latest)throw new Error('Publication no longer exists');
    job=latest;
    if(job.status==='complete'){$('resultAddress').textContent=job.result.site;$('success').hidden=false;return;}
    if(job.status==='cancelled')throw new Error('Publication was cancelled');
    if(job.walletAddress && job.walletAddress!==address && Object.keys(job.checkpoint?.transactions || {}).length)throw new Error('Resume with the original publishing wallet');
    const all = await chrome.storage.local.get(null);
    const unresolved = Object.entries(all).find(([k,v]) => k.startsWith(JOB_PREFIX) && hasUnfinishedSigned(v));
    if (unresolved) {
      const link = document.createElement('a'); link.href=chrome.runtime.getURL('publisher.html')+'#'+unresolved[1].id; link.textContent=t('استئناف العملية السابقة','Resume the earlier publication');
      const holder=document.createElement('p');holder.id='resumeEarlier';holder.append(link);$('status').after(holder);
      throw new Error(t('توجد عملية سابقة بمعاملات موقعة لم تكتمل. افتحها من سجل النشر وأكملها أولًا.','An earlier publication has unresolved signed transactions. Resume it from the publication history first.'));
    }
    try {
    await setJob({status:'publishing',walletAddress:address,error:null});
    const result = await publishSite({plan, rpc, sign:async(tx)=> {
      if (!secret || Date.now() >= until || controller.signal.aborted) throw new Error('Wallet locked or publication paused');
      return signTx(secret,tx);
    },address,chainId:NETWORK.chainId,genesisHash:NETWORK.genesisHash,site:job.request.site,
      maxFeePerGas:'200000000000',maxPriorityFeePerGas:'1000000000',maxTotalFeeWei:quote.maxFeeWei,state:job.checkpoint || null,signal:controller.signal,
      onCheckpoint:async(checkpoint)=>setJob({checkpoint}),
      onProgress:(event)=> {
        setJob({progress:event}).catch(()=>controller?.abort());
        const descriptions={upload:t('تخزين أجزاء الموقع','Storing website chunks'),broadcast:t('إرسال المعاملة','Broadcasting transaction'),confirmed:t('تأكيد المعاملة','Transaction confirmed'),complete:t('اكتمل النشر','Publication complete')};
        $('status').textContent=descriptions[event.phase] || event.phase;
        if(event.transactionHash) $('status').textContent+='\n'+event.transactionHash;
        if(event.total){$('progress').max=event.total;$('progress').value=event.completed || 0;}
      }
    });
    await setJob({status:'complete',result,error:null});
    $('status').textContent=t('نُشر الموقع وتحققنا من الإصدار الحالي على الشبكة.','Published and verified the current version on the network.');
    $('resultAddress').textContent=result.site; $('success').hidden=false;
    } catch(e) { await setJob({status:'paused',error:errorMessage(e)}).catch(()=>{}); throw e; }
    });
  });
}
$('publish').onclick = async () => {
  if (busy || walletBusy || !plan || !secret || !$('consent').checked) return;
  $('resumeEarlier')?.remove();
  busy=true; until=Date.now()+LOCK_MS; controller=new AbortController(); $('progress').hidden=false; renderWallet();
  try { await runPublication(); }
  catch(e) {
    const error=errorMessage(e);
    $('status').textContent=error+'\n'+t('أعد فتح هذه العملية واضغط تأكيد ونشر للاستئناف بالحالة المحفوظة.','Reopen this job and confirm again to resume from its saved state.');
  } finally { busy=false; controller=null; $('progress').hidden=true; $('consent').checked=false; renderWallet(); }
};
$('openSite').onclick=()=>{if(job?.result?.site)chrome.tabs.create({url:chrome.runtime.getURL('viewer.html')+'#'+NETWORK.name+'/'+job.result.site+'/'});};
setInterval(()=>{if(secret&&Date.now()>=until)lock();},1000);
window.addEventListener('beforeunload',(e)=>{if(busy){e.preventDefault();e.returnValue='';}});
async function init() {
  applyLanguage();
  if (!/^[a-f0-9-]{36}$/.test(jobId)) throw new Error(t('افتح طلب النشر من PoCol Studio.','Open a publication request from PoCol Studio.'));
  job=(await chrome.storage.local.get(storageKey))[storageKey]; if(!job)throw new Error(t('طلب النشر غير موجود.','Publication request not found.'));
  plan=prepareSite(job.request.files.map(f=>({path:f.path,data:base64Bytes(f.base64)})),{entry:job.request.entry});
  quote=quotePublish(plan,{site:job.request.site});
  $('siteTitle').textContent=job.request.title;
  $('origin').textContent=job.origin;
  $('siteTarget').textContent=job.request.site || t('إنشاء عنوان موقع جديد','Create a new website address');
  $('fileCount').textContent=plan.files.length; $('totalBytes').textContent=size(plan.totalBytes); $('transactions').textContent=quote.transactionCount;
  $('manifestHash').textContent=plan.manifestHash; $('fee').textContent=formatFee(quote.maxFeeWei);
  $('files').replaceChildren(...plan.files.map(f=>{const li=document.createElement('li'),n=document.createElement('span'),z=document.createElement('span');n.textContent=f.path;z.textContent=size(f.size);li.append(n,z);return li;}));
  if(job.status==='complete'){$('resultAddress').textContent=job.result.site;$('success').hidden=false;$('status').textContent=t('اكتمل نشر هذه العملية.','This publication is complete.');}
  else if(job.checkpoint)$('status').textContent=t('حالة سابقة محفوظة. افتح المحفظة ووافق لاستئناف العملية.','Saved progress found. Unlock the wallet and approve to resume.');
  const vault=(await chrome.storage.local.get(vaultKey))[vaultKey]; if(vault)$('walletNote').textContent=t('توجد محفظة مشفّرة محفوظة. أدخل كلمة مرورها دون اختيار ملف.','An encrypted wallet is saved. Enter its password without selecting a file.');
  renderWallet();
}
init().catch(e=>{$('fatal').hidden=false;$('fatal').textContent=errorMessage(e);});
