import test from 'node:test';
import assert from 'node:assert/strict';
import {prepareSite,quotePublish,publishSite,selector,word,abiUint,abiBytes,PublishPendingError,PublishRevertedError,PublishCapsError,PublishFeeRoseError,publishNeeds,capsPrecheck,planTransactions,remainingTransactions,migrateCheckpoint,readNetworkFees,mimeIdFor,FACTORY,LEGACY_MAX_FEE,LEGACY_PRIORITY_FEE} from '../dist/lib/publisher.js';
import {nextBaseFee,feeSchedule,capFor,GAS_LIMIT,BASE_FEE_FLOOR,ABSOLUTE_CEILING} from '../dist/lib/fees.js';
import {KeyLease,JOB_DEADLINE_MAX_MS} from '../dist/lib/lease.js';
import {isForeignJob,blocksAccount,archiveForeignJobs,archivedJobs,jobNetwork,archiveJob} from '../dist/lib/jobs.js';
import {denyReason,globRegex,selectFiles,parseIncludeList} from '../dist/lib/publish-input.js';
import {readFileSync} from 'node:fs';
import {WEBSITE_INITCODE,WEBSITE_RUNTIME} from '../dist/lib/website-bytecode.js';
import {concat,toHex,fromHex} from '../dist/lib/hex.js';
import {decodeManifest,chunkAddress,decodeCurrent,MIME} from '../dist/lib/site.js';
import {keccak,addressOfSecret} from '../dist/lib/crypto.js';
import {signTx} from '../dist/lib/wallet.js';
import * as rlp from '../dist/lib/rlp.js';
const enc = new TextEncoder();
const secret = fromHex('0x'+'11'.repeat(32));
const owner = toHex(addressOfSecret(secret));
const chainId=1599846715;
const genesis='0x'+'ab'.repeat(32);
const blockHash='0x'+'bc'.repeat(32);
const sample=()=>prepareSite([{path:'/index.html',data:enc.encode('<h1>Hello</h1>')},{path:'/style.css',data:enc.encode('body{color:red}')},{path:'/empty.txt',data:new Uint8Array()}]);
const dynamic=(items)=>concat(word(items.length),...items.map(a=>concat(new Uint8Array(12),fromHex(a))));
// Reference London recurrence (lp4-execution/src/fees.rs next_base_fee), written independently of dist/lib/fees.js.
function refNext(parent,used,limit,floor){
  const target=limit/2n;if(target===0n||used>limit)throw new Error('ParentGas');
  let fee;if(used===target)fee=parent;else if(used>target){const d=parent*(used-target)/target/8n;fee=parent+(d<1n?1n:d);}else fee=parent-parent*(target-used)/target/8n;
  return fee<floor?floor:fee;
}
const FLOOR_FEES={baseNext:BASE_FEE_FLOOR.toString(),tip:(10n**9n).toString()};
const approve=(plan,more={})=>quotePublish(plan,{fees:FLOOR_FEES,...more}).approval;
function mock() {
  const codes=new Map(),receipts=new Map(),versions=[];let nonce=0n,current=null,site=null;
  const logs={signed:[],sent:[],calls:[],codeReads:[],safeReads:[],bases:[]};
  // Sites W4: the mock chain mines one block per accepted transaction, its gasUsed the declared gas, and moves the base
  // fee by the reference London rule below (`chain.base` is the next block's base fee). `unsafe` chunks are absent at
  // the `safe` tag; `reorgOut` chunks disappear from `latest` too when the sweep first reads them at `safe`.
  const chain={head:42n,base:BASE_FEE_FLOOR,unsafe:new Set(),reorgOut:new Set(),spikeAfter:null,spike:1n,gasUsedPercent:100n,clockOnSend:null};
  const control={allow:true,wrongNetwork:false,failAfterSend:null,unknownBeforeSend:false,noReceipt:false,failSave:false,wrongHash:false,revert:false,publishAfterCreate:false,wrongVersionChunks:false,orphanBlock:false,
    // FN-1: pocol_getCaps. `room` overrides a cap's remaining room (count = cap - room); `caps` 'missing' answers -32601, 'transport' a relay refusal.
    caps:'ok',room:{},capCount:{},cumulative:true,
    // FN-1: the first eth_getCode (the chunk scan of a new site) fails: 'transport' as a gateway 429 does, 'rpc' an error object, 'error' anything else.
    scanFail:null};
  const CAPS=['accounts','storageSlots','codeBytes','txCount','txBytes','logBytes'];
  function capsAnswer(){
    const out={number:'0x2a',hash:blockHash,v3:true,cumulative:control.cumulative,alarmPercent:80,occupancy:{},alarm:false};
    for(const name of CAPS){const cap=10n**9n,count=control.capCount[name]!==undefined?BigInt(control.capCount[name]):control.room[name]!==undefined?cap-BigInt(control.room[name]):1000n;
      out[name]='0x'+count.toString(16);out[name==='accounts'?'accountsCap':name==='storageSlots'?'storageSlotsCap':name==='codeBytes'?'codeBytesCap':name==='txCount'?'txCap':name==='txBytes'?'txBytesCap':'logCap']='0x'+cap.toString(16);
      out.occupancy[name]={percent:Number(count*10000n/cap)/100,remaining:'0x'+(cap-count).toString(16),alarm:count*100n>=cap*80n};}
    return out;
  }
  const n=(b)=>b.length?BigInt(toHex(b)):0n;
  function versionOut(v){const x=versions[Number(v)];if(!x) throw new Error('BadVersion');return toHex(concat(fromHex(x.hash),word(x.length),word(x.status),word(42),word(160),dynamic(x.chunks)));}
  async function rpc(method,params) {
    logs.calls.push([method,params]);
    if(method==='eth_chainId')return '0x'+(control.wrongNetwork?1:chainId).toString(16);
    if(method==='pocol_status')return {genesisHash:genesis};
    if(method==='pocol_getCaps'){
      if(control.caps==='missing')throw Object.assign(new Error('the method pocol_getCaps does not exist'),{rpc:{code:-32601,message:'the method pocol_getCaps does not exist'}});
      if(control.caps==='transport')throw Object.assign(new Error('transport'),{transport:'transport'});
      if(control.caps==='invalid')return {accounts:'zz'};
      return capsAnswer();
    }
    if(method==='eth_feeHistory')return {oldestBlock:'0x'+chain.head.toString(16),baseFeePerGas:['0x'+chain.base.toString(16),'0x'+chain.base.toString(16)],gasUsedRatio:[0],reward:[]};
    if(method==='eth_gasPrice')return '0x'+(chain.base+10n**9n).toString(16);
    if(method==='eth_maxPriorityFeePerGas')return '0x'+(10n**9n).toString(16);
    if(method==='eth_blockNumber'){if(control.emptyBlockOnPoll){chain.head++;chain.base=refNext(chain.base,0n,GAS_LIMIT,BASE_FEE_FLOOR);logs.bases.push(chain.base);}return '0x'+chain.head.toString(16);}
    if(method==='eth_getBalance')return '0x'+(10n**30n).toString(16);
    if(method==='eth_getTransactionCount')return '0x'+nonce.toString(16);
    if(method==='eth_getCode'){
      logs.codeReads.push(params[0]);
      if(control.scanFail){const kind=control.scanFail;control.scanFail=null;
        if(kind==='transport')throw Object.assign(new Error('transport'),{transport:'transport'});
        if(kind==='rpc')throw Object.assign(new Error('limited'),{rpc:{code:-32005,message:'limit exceeded'}});
        throw new Error('boom');}
      if(params[1]==='safe'){logs.safeReads.push(params[0]);if(chain.reorgOut.has(params[0])){chain.reorgOut.delete(params[0]);codes.delete(params[0]);}if(chain.unsafe.has(params[0]))return '0x';}
      return codes.get(params[0])||'0x';
    }
    if(method==='eth_getTransactionReceipt')return control.noReceipt?null:(receipts.get(params[0])||null);
    if(method==='eth_getBlockByNumber'){
      if(params[0]==='latest'||params[0]==='safe')return {hash:blockHash,number:'0x'+chain.head.toString(16),baseFeePerGas:'0x'+chain.base.toString(16),gasUsed:'0x0',gasLimit:'0x'+GAS_LIMIT.toString(16)};
      return {hash:control.orphanBlock?'0x'+'dd'.repeat(32):blockHash,number:params[0]};
    }
    if(method==='eth_call') {
      const data=params[0].data,sel=data.slice(0,10);
      if(sel===selector('isPublisher(address)'))return toHex(word(control.allow?1:0));
      if(sel===selector('versionCount()'))return toHex(word(versions.length));
      if(sel===selector('version(uint256)'))return versionOut(BigInt('0x'+data.slice(10)));
      if(sel===selector('current()')){
        if(current===null) throw new Error('No current'); const v=versions[current];
        return toHex(concat(word(current),fromHex(v.hash),word(v.length),word(42),word(160),dynamic(v.chunks)));
      }
      throw new Error('Unexpected call '+data);
    }
    if(method==='eth_sendRawTransaction') {
      const raw=fromHex(params[0]),hash=toHex(keccak(raw));logs.sent.push(params[0]);
      if(control.unknownBeforeSend) throw new Error('Network unavailable');
      if(receipts.has(hash)) return hash;
      const f=rlp.decode(raw.subarray(1));const txNonce=n(f[1]);assert.equal(txNonce,nonce);
      const to=f[5].length?toHex(f[5]):null,data=f[7],sel=toHex(data.subarray(0,4));nonce++;
      // EIP-1559 inclusion: the fee cap must cover the block's base fee and the tip may not exceed the cap.
      const maxPrio=n(f[2]),maxFee=n(f[3]),gas=n(f[4]);assert.ok(maxFee>=chain.base,`fee cap ${maxFee} below base ${chain.base}`);assert.ok(maxPrio<=maxFee);
      // gasUsed: a chunk store at the estimate 70000 + 218 * size (a full chunk ~5.4M, measured 5.395M), else the declared gas.
      const used=(to===FACTORY?70000n+218n*n(data.subarray(36,68)):gas)*chain.gasUsedPercent/100n;
      chain.head++;logs.bases.push(chain.base);chain.base=refNext(chain.base,used,GAS_LIMIT,BASE_FEE_FLOOR);
      if(chain.spikeAfter!==null&&logs.sent.length===chain.spikeAfter)chain.base*=chain.spike;
      if(chain.clockOnSend)chain.clockOnSend();
      const receipt={transactionHash:hash,status:control.revert?'0x0':'0x1',blockNumber:'0x'+chain.head.toString(16),blockHash,contractAddress:null};
      if(!control.revert){
        if(!to){assert.equal(toHex(data),WEBSITE_INITCODE);site=toHex(keccak(rlp.encode([fromHex(owner),rlp.encUint(txNonce)])).subarray(12));codes.set(site,WEBSITE_RUNTIME);receipt.contractAddress=site;}
        else if(to===FACTORY){const size=Number(n(data.subarray(36,68))),chunk=data.subarray(68,68+size),a=toHex(chunkAddress(chunk));codes.set(a,toHex(concat(Uint8Array.of(0),chunk)));chain.unsafe.delete(a);}
        else if(sel===selector('createVersion(bytes32,uint32,address[])')){const hash=toHex(data.subarray(4,36)),length=Number(n(data.subarray(36,68))),count=Number(n(data.subarray(100,132)));const chunks=[];for(let i=0;i<count;i++)chunks.push(toHex(data.subarray(144+32*i,164+32*i)));if(control.wrongVersionChunks)chunks[0]='0x'+'aa'.repeat(20);versions.push({hash,length,status:control.publishAfterCreate?1:0,chunks});}
        else if(sel===selector('publish(uint256)'))versions[Number(n(data.subarray(4)))].status=1;
        else if(sel===selector('setCurrent(uint256)'))current=Number(n(data.subarray(4)));
        else throw new Error('Unexpected tx selector '+sel);
      }
      receipts.set(hash,receipt);
      if(control.failAfterSend===sel||control.failAfterSend==='all')throw new Error('Connection lost after accepted');
      return control.wrongHash?'0x'+'ee'.repeat(32):hash;
    }
    throw new Error('Unexpected RPC '+method);
  }
  let state;
  // The approval is quoted at the floor base fee and the node tip, as the publisher page quotes an idle chain.
  const options=(plan=sample(),more={})=>({plan,rpc,address:owner,chainId,genesisHash:genesis,...(more.approval?{}:{approval:approve(plan,{site:more.site})}),sign:async tx=>{logs.signed.push(tx);return signTx(secret,tx);},onCheckpoint:async s=>{if(control.failSave)throw new Error('Storage full');state=s;},pollIntervalMs:0,receiptTimeoutMs:10,readBurst:64,...more});
  return {rpc,logs,control,chain,codes,receipts,versions,options,reorg(){codes.clear();receipts.clear();versions.length=0;nonce=0n;current=null;site=null;},get state(){return state;},get site(){return site;}};
}
test('canonical manifest and chunks are accepted by existing independent viewer',()=>{
  const p=sample(),d=decodeManifest(p.manifest);assert.equal(d.files.length,3);assert.equal(d.files[d.entry].path,'/index.html');assert.equal(d.files[0].size,0);assert.deepEqual(d.files.map(f=>f.path),['/empty.txt','/index.html','/style.css']);
});
test('manifest is independent of upload file order',()=>{const p=sample();assert.equal(prepareSite([...p.files].reverse()).manifestHash,p.manifestHash);});
test('fixed canonical publication vector',()=>{const p=prepareSite([{path:'/index.html',data:enc.encode('<h1>Hello</h1>')}]);assert.equal(p.manifestHash,'0xca30ba54ed04ecd44e90734487ec3782a5da68997f2b2480c5da6f6d309cf698');assert.equal(quotePublish(p).maxGas,'2924180');assert.equal(quotePublish(p).transactionCount,6);});
test('byte-for-byte parity with independent M1 draft 0.2 pos-minimal vector',()=>{const html='<!doctype html><html><head><meta charset="utf-8"><title>PoCol</title></head><body><h1>PoCol</h1></body></html>\n';const p=prepareSite([{path:'/index.html',data:enc.encode(html)}]);assert.equal(p.totalBytes,111);assert.equal(p.manifestHash,'0x17f69a21ff29e4deaef80bf635e57531863713eb2bc5df15ed4497df068f33e0');assert.equal(p.files[0].chunks[0].address,'0xbffcaeb40682a2dfe0d39de9eee7d1f20891ad3b');assert.equal(toHex(p.manifest),'0xf84d0180f849f8478b2f696e6465782e68746d6c016fa027c02dac108feef56130154f3c74951ba4e96b15ccbedb6e66bef4a6e825e451d7d694bffcaeb40682a2dfe0d39de9eee7d1f20891ad3b6f');});
for(const [label,files] of [
  ['empty',[]],['traversal',[{path:'/../index.html',data:new Uint8Array()}]],['unicode path',[{path:'/عربي.html',data:new Uint8Array()}]],['duplicate',[{path:'/index.html',data:new Uint8Array()},{path:'/index.html',data:new Uint8Array()}]],['oversize',[{path:'/index.html',data:new Uint8Array(1048577)}]],['bad extension',[{path:'/index.php',data:new Uint8Array()}]],['missing entry',[{path:'/page.html',data:new Uint8Array()}]],['wrong bytes',[{path:'/index.html',data:'hello'}]],['too many',Array.from({length:257},(_,i)=>({path:`/${i}.html`,data:new Uint8Array()}))],
])test('rejects '+label,()=>assert.throws(()=>prepareSite(files)));
test('site total limit is validated before hashing oversized final input',()=>assert.throws(()=>prepareSite(Array.from({length:5},(_,i)=>({path:i?`/${i}.txt`:'/index.html',data:new Uint8Array(1048576)}))),/4 MiB/));
test('input byte arrays are copied and plan mutations detected',()=>{const data=enc.encode('safe'),p=prepareSite([{path:'/index.html',data}]);data[0]=0;assert.equal(p.files[0].data[0],115);p.files[0].data[0]=0;assert.throws(()=>quotePublish(p),/changed/);});
test('identical file contents reuse a single storage chunk',()=>{const p=prepareSite([{path:'/index.html',data:enc.encode('same')},{path:'/two.html',data:enc.encode('same')}]);assert.equal(p.chunks.length,2);});
test('successful complete publication is fetched and verified',async()=>{const m=mock(),r=await publishSite(m.options());assert.equal(r.site,m.site);assert.equal(r.version,0);assert.equal(r.publishedBlock,42);assert.equal(m.state.complete,true);assert.equal(m.logs.sent.length,7);assert.equal(r.manifestHash,sample().manifestHash);});
test('source chunks already on chain skip transactions and verify content',async()=>{const m=mock(),p=sample();for(const c of p.chunks)m.codes.set(c.address,toHex(concat(Uint8Array.of(0),c.data)));await publishSite(m.options(p));assert.equal(m.logs.sent.length,4);});
test('corrupt existing chunk is rejected before signing',async()=>{const m=mock(),p=sample();m.codes.set(p.chunks[0].address,'0x00ee');await assert.rejects(publishSite(m.options(p)),/chunk/);assert.equal(m.logs.signed.length,0);});
test('wrong network cannot trigger a signature',async()=>{const m=mock();m.control.wrongNetwork=true;await assert.rejects(publishSite(m.options()),/identity/);assert.equal(m.logs.signed.length,0);});
test('insufficient approved fee cannot trigger a signature',async()=>{const m=mock();await assert.rejects(publishSite(m.options(undefined,{approval:{...approve(sample()),budgetWei:'1'}})),e=>e instanceof PublishFeeRoseError&&e.reason==='budget');assert.equal(m.logs.signed.length,0);});
test('checkpoint persistence failure prevents broadcast',async()=>{const m=mock();m.control.failSave=true;await assert.rejects(publishSite(m.options()),/Storage full/);assert.equal(m.logs.sent.length,0);});
test('checkpoint failure after signing still prevents broadcast',async()=>{const m=mock();let count=0;await assert.rejects(publishSite(m.options(undefined,{onCheckpoint:async()=>{if(++count===2)throw new Error('Storage full');}})),/Storage full/);assert.equal(m.logs.signed.length,1);assert.equal(m.logs.sent.length,0);});
test('signer cannot substitute another transaction',async()=>{const m=mock();await assert.rejects(publishSite(m.options(undefined,{sign:async tx=>signTx(secret,{...tx,to:owner,value:'1'})})),/changed transaction/);assert.equal(m.logs.sent.length,0);});
test('wrong signing key is rejected',async()=>{const m=mock();await assert.rejects(publishSite(m.options(undefined,{sign:async tx=>signTx(fromHex('0x'+'22'.repeat(32)),tx)})),/publishing account/);assert.equal(m.logs.sent.length,0);});
test('ambiguous submission resumes the SAME raw without another signature',async()=>{const m=mock();m.control.unknownBeforeSend=true;await assert.rejects(publishSite(m.options()),PublishPendingError);const saved=m.state,raw=m.logs.sent[0];m.control.unknownBeforeSend=false;await publishSite(m.options(undefined,{state:saved}));assert.equal(m.logs.sent[1],raw);assert.equal(m.logs.signed.length,7);});
test('lost submit response reconciles chain receipt without duplicate signing',async()=>{const m=mock();m.control.failAfterSend='all';await publishSite(m.options());assert.equal(m.logs.signed.length,7);assert.equal(m.logs.sent.length,7);});
test('receipt timeout preserves pending transaction and resumes without re-signing',async()=>{const m=mock();m.control.noReceipt=true;await assert.rejects(publishSite(m.options()),PublishPendingError);const saved=m.state;m.control.noReceipt=false;await publishSite(m.options(undefined,{state:saved}));assert.equal(m.logs.signed.length,7);assert.equal(m.logs.sent.length,7);});
test('completed checkpoint is idempotent',async()=>{const m=mock();const first=await publishSite(m.options());const r=await publishSite(m.options(undefined,{state:m.state}));assert.equal(r.site,first.site);assert.equal(m.logs.signed.length,7);});
test('checkpoint cannot be reused for a different document or allowance',async()=>{const m=mock();await publishSite(m.options());await assert.rejects(publishSite(m.options(undefined,{state:m.state,approval:{...approve(sample()),budgetWei:'999999999999999999999'}})),/another publication/);});
test('checkpoint fee ledger tampering is rejected',async()=>{const m=mock();await publishSite(m.options());const state=structuredClone(m.state);state.spentCapWei='0';await assert.rejects(publishSite(m.options(undefined,{state})),/accounting/);});
test('existing site requires publisher authorization before uploading',async()=>{const m=mock(),r=await publishSite(m.options());m.control.allow=false;const count=m.logs.signed.length;await assert.rejects(publishSite(m.options(undefined,{site:r.site})),/not authorized/);assert.equal(m.logs.signed.length,count);});
test('existing site requires exact supported contract runtime',async()=>{const m=mock();const target='0x'+'44'.repeat(20);m.codes.set(target,'0x6000');await assert.rejects(publishSite(m.options(undefined,{site:target})),/supported/);assert.equal(m.logs.signed.length,0);});
test('update keeps site address and creates next version',async()=>{const m=mock(),r=await publishSite(m.options()),p=prepareSite([{path:'/index.html',data:enc.encode('New version')}]);const r2=await publishSite(m.options(p,{site:r.site}));assert.equal(r2.site,r.site);assert.equal(r2.version,1);assert.equal(m.versions.length,2);});
test('reverted receipt stops publication with structured terminal error and remains recorded',async()=>{const m=mock();m.control.revert=true;await assert.rejects(publishSite(m.options()),e=>e instanceof PublishRevertedError&&e.code==='PUBLISH_REVERTED'&&e.transactionHash===Object.values(m.state.transactions)[0].hash);assert.equal(m.logs.sent.length,1);assert.equal(Object.values(m.state.transactions)[0].receipt.status,'0x0');});
test('pause leaves checkpoint without extra transaction',async()=>{const m=mock(),abort=new AbortController();abort.abort();await assert.rejects(publishSite(m.options(undefined,{signal:abort.signal})),/paused/);assert.equal(m.logs.sent.length,0);});
test('matching version published concurrently does not send redundant publish',async()=>{const m=mock();m.control.publishAfterCreate=true;const result=await publishSite(m.options());assert.equal(result.version,0);assert.equal(m.logs.sent.length,6);assert.equal(m.logs.signed.some(tx=>tx.data.startsWith(selector('publish(uint256)'))),false);});
test('version must contain the exact prepared manifest chunk addresses',async()=>{const m=mock();m.control.wrongVersionChunks=true;await assert.rejects(publishSite(m.options()),/not found/);assert.equal(m.logs.signed.some(tx=>tx.data.startsWith(selector('publish(uint256)'))),false);});
test('per-account concurrent publication is refused while first is pending',async()=>{const m=mock();let resolve;const gate=new Promise(r=>{resolve=r;});let entered;const started=new Promise(r=>{entered=r;});const first=publishSite(m.options(undefined,{sign:async tx=>{entered();await gate;return signTx(secret,tx);}}));await started;await assert.rejects(publishSite(m.options()),/already using/);resolve();await first;});
test('orphan cached receipts cause exact raw rebroadcast without new signatures',async()=>{const m=mock(),r=await publishSite(m.options()),state=m.state,raw=[...m.logs.sent];m.reorg();const again=await publishSite(m.options(undefined,{state}));assert.equal(again.site,r.site);assert.equal(m.logs.signed.length,7);assert.deepEqual(m.logs.sent.slice(7),raw);});
test('receipt from a noncanonical block is not accepted as confirmation',async()=>{const m=mock();m.control.orphanBlock=true;await assert.rejects(publishSite(m.options()),PublishPendingError);assert.equal(m.logs.signed.length,1);assert.equal(Object.values(m.state.transactions)[0].receipt,null);});

test('concurrently stored chunk still reconciles its unresolved signed transaction before advancing nonce',async()=>{
  const m=mock(),p=sample();m.control.unknownBeforeSend=true;
  await assert.rejects(publishSite(m.options(p)),PublishPendingError);
  const saved=m.state,raw=m.logs.sent[0],chunk=p.chunks[0];
  m.codes.set(chunk.address,toHex(concat(Uint8Array.of(0),chunk.data)));
  m.control.unknownBeforeSend=false;
  await publishSite(m.options(p,{state:saved}));
  assert.equal(m.logs.sent[1],raw);
  assert.equal(m.state.transactions['chunk:'+chunk.address].receipt.status,'0x1');
  assert.equal(m.logs.signed.length,7);
  assert.equal(new Set(m.logs.signed.map(tx=>tx.nonce)).size,7);
});

// FN-1 (C9): the pocol_getCaps pre-check refuses, before any signature, a site the chain caps cannot hold.
const capsCalls=(m)=>m.logs.calls.filter(c=>c[0]==='pocol_getCaps').length;
test('a site that cannot fit is refused before signing, with each short cap named',async()=>{
  const m=mock(),p=sample(),need=publishNeeds(p,{newSite:true,missing:new Set(p.chunks.map(c=>c.address))});
  m.control.room={accounts:need.accounts-1,codeBytes:need.codeBytes-1};
  await assert.rejects(publishSite(m.options(p)),e=>e instanceof PublishCapsError&&e.code==='PUBLISH_CAPS'&&/cannot fit/.test(e.message)
    &&/accounts \(ACCT_CAP\)/.test(e.message)&&/code bytes \(CODE_CAP\)/.test(e.message)&&e.shortfalls.length===2&&e.shortfalls[0].need===String(need.accounts)&&e.shortfalls[0].remaining===String(need.accounts-1));
  assert.equal(m.logs.signed.length,0);assert.equal(m.logs.sent.length,0);assert.equal(m.state,undefined,'no checkpoint with a transaction');
});
test('a site that exactly fits every cap is published',async()=>{
  const m=mock(),p=sample(),need=publishNeeds(p,{newSite:true,missing:new Set(p.chunks.map(c=>c.address))});
  m.control.room=Object.fromEntries(Object.entries(need).map(([k,v])=>[k,v]));
  const r=await publishSite(m.options(p));assert.equal(r.site,m.site);assert.equal(capsCalls(m),1);assert.equal(m.logs.signed.length,7);
});
test('the needs are what the publication adds: accounts, code, transactions, and bytes bounded',async()=>{
  const m=mock(),p=sample(),need=publishNeeds(p,{newSite:true,missing:new Set(p.chunks.map(c=>c.address))});
  await publishSite(m.options(p));
  assert.equal(need.accounts,m.codes.size,'one account per stored chunk and the site');
  assert.equal(need.codeBytes,[...m.codes.values()].reduce((n,c)=>n+(c.length-2)/2,0),'every new runtime byte');
  assert.equal(need.txCount,m.logs.sent.length,'every transaction sent');
  assert.ok(need.txBytes>=m.logs.sent.reduce((n,r)=>n+(r.length-2)/2,0),'raw transaction bytes are bounded');
  // The mock does not execute Website.sol: pocold tests/fn1_caps.rs (a_website_publication_adds_what_the_publisher_pre_check_expects)
  // executes a publication and checks these slot counts (4 + 3 + n new, 3 + n update) against the chain.
  assert.equal(need.storageSlots,4+3+p.manifestChunks.length);assert.equal(need.logBytes,0);
});
test('chunks already on chain are not counted against the caps',async()=>{
  const m=mock(),p=sample();for(const c of p.chunks)m.codes.set(c.address,toHex(concat(Uint8Array.of(0),c.data)));
  const need=publishNeeds(p,{newSite:true,missing:new Set()});assert.equal(need.accounts,1);assert.equal(need.txCount,4);
  m.control.room={accounts:1};await publishSite(m.options(p));assert.equal(m.logs.sent.length,4);
});
test('an update of an existing site needs no new site account',async()=>{
  const m=mock(),r=await publishSite(m.options()),p=prepareSite([{path:'/index.html',data:enc.encode('Second')}]);
  const need=publishNeeds(p,{newSite:false,missing:new Set(p.chunks.map(c=>c.address))});assert.equal(need.accounts,p.chunks.length);assert.equal(need.storageSlots,3+p.manifestChunks.length);
  m.control.room={accounts:need.accounts};const r2=await publishSite(m.options(p,{site:r.site}));assert.equal(r2.version,1);
  m.control.room={accounts:0};const p3=prepareSite([{path:'/index.html',data:enc.encode('Third')}]);const before=m.logs.signed.length;
  await assert.rejects(publishSite(m.options(p3,{site:r.site})),PublishCapsError);assert.equal(m.logs.signed.length,before);
});
test('an older node without pocol_getCaps (-32601) is published after a warning',async()=>{
  const m=mock();m.control.caps='missing';const warnings=[],phases=[];
  const r=await publishSite(m.options(undefined,{onWarning:w=>warnings.push(w),onProgress:e=>phases.push(e)}));
  assert.equal(r.site,m.site);assert.equal(warnings.length,1);assert.match(warnings[0],/pre-check was skipped/);
  assert.ok(phases.some(e=>e.phase==='warning'&&/pocol_getCaps/.test(e.message)));assert.equal(m.logs.signed.length,7);
});
test('a relay that does not forward pocol_getCaps is tolerated with a warning; other RPC failures are not',async()=>{
  const m=mock();m.control.caps='transport';const warnings=[];
  await publishSite(m.options(undefined,{onWarning:w=>warnings.push(w)}));assert.match(warnings[0],/did not answer pocol_getCaps/);
  const bad=mock();bad.control.caps='invalid';await assert.rejects(publishSite(bad.options()),/pocol_getCaps/);assert.equal(bad.logs.signed.length,0);
  await assert.rejects(capsPrecheck(async()=>{throw new Error('boom')},{}),/boom/);
});
test('a cap at or above 80 % is reported, and the site still fits',async()=>{
  const m=mock();m.control.capCount={accounts:8n*10n**8n};const phases=[];
  await publishSite(m.options(undefined,{onProgress:e=>phases.push(e)}));
  assert.deepEqual(phases.find(e=>e.phase==='capsAlarm').alarms,['accounts']);
  const quiet=mock();quiet.control.capCount={accounts:8n*10n**8n-1n};const none=[];await publishSite(quiet.options(undefined,{onProgress:e=>none.push(e)}));
  assert.equal(none.some(e=>e.phase==='capsAlarm'),false,'79.99... % raises no alarm');
});
test('a resumed publication with signed transactions is not re-checked',async()=>{
  const m=mock();m.control.unknownBeforeSend=true;await assert.rejects(publishSite(m.options()),PublishPendingError);
  assert.equal(capsCalls(m),1);const saved=m.state;m.control.unknownBeforeSend=false;m.control.room={accounts:0};
  await publishSite(m.options(undefined,{state:saved}));assert.equal(capsCalls(m),1,'the committed publication finishes');assert.equal(m.logs.signed.length,7);
});

// FN-1 review: the chunk scan is paced, its answers are reused by the upload, a refused scan read skips the pre-check,
// and a continuous chain's per-block rows are checked per transaction.
const many=()=>prepareSite([{path:'/index.html',data:enc.encode('<h1>many</h1>')},...Array.from({length:9},(_,i)=>({path:`/f${i}.txt`,data:enc.encode('file '+i)}))]);
for(const kind of['transport','rpc'])test(`a scan read the network refuses (${kind}) skips the pre-check with a warning and the site is published`,async()=>{
  const m=mock();m.control.scanFail=kind;const warnings=[],phases=[];
  const r=await publishSite(m.options(undefined,{onWarning:w=>warnings.push(w),onProgress:e=>phases.push(e)}));
  assert.equal(r.site,m.site);assert.equal(m.state.complete,true);assert.equal(m.logs.signed.length,7);
  assert.equal(warnings.length,1);assert.match(warnings[0],/chunk scan .*pre-check was skipped/);assert.ok(phases.some(e=>e.phase==='warning'));
  assert.equal(capsCalls(m),0,'no needs to check without the scan');
});
test('any other scan failure still stops the publication before signing',async()=>{
  const m=mock();m.control.scanFail='error';await assert.rejects(publishSite(m.options()),/boom/);assert.equal(m.logs.signed.length,0);
});
test('the upload reuses the scan: a stored chunk is read once, a missing one again before its transaction',async()=>{
  const m=mock(),p=sample(),stored=p.chunks[0];m.codes.set(stored.address,toHex(concat(Uint8Array.of(0),stored.data)));
  await publishSite(m.options(p));
  const reads=(a)=>m.logs.codeReads.filter(x=>x===a).length;
  assert.equal(reads(stored.address),3,'the scan answer is reused: the scan, the safe sweep and the read-back');
  for(const c of p.chunks.slice(1))assert.equal(reads(c.address),5,'scan, re-read before storing, verification after, the safe sweep, the read-back');
});
test('the chunk reads are paced: a burst, then the read rate',async()=>{
  // A virtual clock that only the publisher's sleeps advance: no wall-clock timing is asserted.
  const m=mock(),p=many();assert.ok(p.chunks.length>=10);let t=0;const at=[],sleeps=[];
  const o=m.options(p,{readBurst:2,readRate:50,clock:()=>t,sleep:async ms=>{sleeps.push(ms);t+=ms;}});
  const chunks=new Set(p.chunks.map(c=>c.address)),inner=o.rpc;o.rpc=async(method,params)=>{if(method==='eth_getCode'&&chunks.has(params[0]))at.push(t);return inner(method,params);};
  await publishSite(o);assert.equal(m.state.complete,true);
  assert.equal(at.length,5*p.chunks.length,'every chunk read (the scan, the re-read before storing, the check after, the safe sweep, the read-back) goes through the pacer');
  assert.deepEqual(at.slice(0,3),[0,0,20],'a burst of 2, then one read per 20 ms at 50 per second');
  // The token bucket: by the k-th read (from 0) at most burst + rate·t reads were allowed.
  at.forEach((x,k)=>assert.ok(k+1<=2+x*50/1000+1e-9,`read ${k} at ${x} ms`));
  // The receipt polls sleep pollIntervalMs (0 here) through the same injected sleep; every other wait is the pacer's.
  const paced=sleeps.filter(ms=>ms!==0);assert.ok(paced.length>0&&paced.every(ms=>ms>0&&ms<=20),'each wait is at most one token');
  await assert.rejects(publishSite(mock().options(p,{readRate:0})),/pacing/);
});
test('a continuous chain (cumulative false) checks each transaction against one block\'s room',async()=>{
  const p=sample(),need=publishNeeds(p,{newSite:true,missing:new Set(p.chunks.map(c=>c.address))});
  assert.ok(need.txCount>1&&need.largestTxBytes<need.txBytes&&need.largestTxBytes>0);
  const fits=mock();fits.control.cumulative=false;fits.control.room={txCount:1,txBytes:need.largestTxBytes};
  const r=await publishSite(fits.options(p));assert.equal(r.site,fits.site,'one block holds each transaction');
  const cumulative=mock();cumulative.control.room={txCount:1,txBytes:need.largestTxBytes};
  await assert.rejects(publishSite(cumulative.options(p)),e=>e instanceof PublishCapsError&&e.shortfalls.map(x=>x.name).join()==='txCount,txBytes','a cumulative chain counts the whole publication');
  const short=mock();short.control.cumulative=false;short.control.room={txBytes:need.largestTxBytes-1};
  await assert.rejects(publishSite(short.options(p)),e=>e instanceof PublishCapsError&&/transaction bytes in one block \(TXBYTES_CAP\)/.test(e.message)&&e.shortfalls[0].need===String(need.largestTxBytes));
  assert.equal(short.logs.signed.length,0);
  const none=mock();none.control.cumulative=false;none.control.room={txCount:0};await assert.rejects(publishSite(none.options(p)),/transactions in one block/);
  await assert.rejects(capsPrecheck(async()=>({...Object.fromEntries(['accounts','accountsCap','storageSlots','storageSlotsCap','codeBytes','codeBytesCap','txCount','txCap','txBytes','txBytesCap','logBytes','logCap'].map(k=>[k,'0x1'])),occupancy:{},cumulative:'no'}),need),/Invalid pocol_getCaps/);
});
test('the largest transaction is bounded by the raw transactions sent',async()=>{
  const m=mock(),p=many(),need=publishNeeds(p,{newSite:true,missing:new Set(p.chunks.map(c=>c.address))});
  await publishSite(m.options(p));assert.ok(need.largestTxBytes>=Math.max(...m.logs.sent.map(r=>(r.length-2)/2)));
});

// ---------------- Sites W4: fees from the node, the approval, the lease, checkpoint schema 2, foreign jobs ----------------
const GWEI=10n**9n;
// Distinct pseudo-random content (xorshift32), so no two chunks of a large test site share an address.
function noise(n,seed){const b=new Uint8Array(n);let x=seed>>>0||1;for(let i=0;i<n;i++){x^=x<<13;x>>>=0;x^=x>>>17;x^=x<<5;x>>>=0;b[i]=x&255;}return b;}
// About 4 MiB − 4 KiB in four files of 1 MiB − 1 KiB: 172 file chunks (four of 43) and the manifest chunk.
const bigSite=()=>prepareSite(['/index.html','/a.js','/b.css','/c.txt'].map((path,i)=>({path,data:noise(1047552,i+7)})));
// The page's raw-transaction store (one key per transaction) keeps the checkpoint small.
const rawStore=()=>{const m=new Map();return {map:m,async put(h,r){m.set(h,r);},async get(h){return m.get(h)??null;}};};
// About 1 MiB in one file: 43 file chunks and the manifest chunk.
const mibSite=()=>prepareSite([{path:'/index.html',data:noise(1047552,3)}]);
const virtual=()=>{let t=0;return {clock:()=>t,sleep:async ms=>{t+=ms;},advance:ms=>{t+=ms;},get now(){return t;}};};
test('W4: nextBaseFee is the fees.rs London rule (lp4-execution london-fees.json vectors, floor tests)',()=>{
  const text=readFileSync(new URL('../../lp4-execution/tests/data/london-fees.json',import.meta.url),'utf8').replace(/:\s*(\d+)/g,':"$1"');
  const rows=JSON.parse(text);assert.equal(rows.length,192);
  for(const r of rows){
    if(r.expected==='ParentGas'||r.expected==='Overflow')assert.throws(()=>nextBaseFee(BigInt(r.base),BigInt(r.used),BigInt(r.limit),0n),new RegExp(r.expected));
    else assert.equal(nextBaseFee(BigInt(r.base),BigInt(r.used),BigInt(r.limit),0n),BigInt(r.expected),JSON.stringify(r));
  }
  // fees.rs floor_tests: a thousand empty blocks reach the v3 floor while London without a floor decays to 7.
  let v3=GWEI,v1=GWEI;for(let i=0;i<1000;i++){v3=nextBaseFee(v3,0n,8000000n,10000000n);v1=nextBaseFee(v1,0n,8000000n,0n);assert.ok(v3>=10000000n);}
  assert.equal(v3,10000000n);assert.equal(v1,7n);assert.equal(nextBaseFee(1n,4000000n,8000000n,10000000n),10000000n);
  assert.equal(nextBaseFee(GWEI,8000000n,8000000n,0n),1125000000n);assert.equal(nextBaseFee(GWEI,0n,8000000n,0n),875000000n);
});
test('W4: the schedule simulates the London recurrence over the planned transactions, one per block',()=>{
  const p=bigSite(),txs=planTransactions(p);assert.equal(txs.length,p.chunks.length+4);assert.equal(p.chunks.length,173);
  for(const economy of[false,true]){
    const s=feeSchedule({txs,baseNext:BASE_FEE_FLOOR,tip:GWEI,economy});let base=BASE_FEE_FLOOR,budget=0n,expected=0n,top=0n;
    s.schedule.forEach((x,i)=>{
      assert.equal(x.id,txs[i].id);assert.equal(BigInt(x.base),base,'projected base of '+x.id);
      const used=x.id.startsWith('chunk:')?70000n+218n*BigInt(p.chunks[i].data.length):BigInt(x.gas);assert.equal(BigInt(x.gasUsed),used);
      const cap=(base*1125n+999n)/1000n+GWEI;assert.equal(BigInt(x.maxFee),cap);budget+=BigInt(x.gas)*cap;expected+=used*(base+GWEI);if(cap>top)top=cap;
      base=refNext(base,used,GAS_LIMIT,BASE_FEE_FLOOR);if(economy)base=refNext(base,0n,GAS_LIMIT,BASE_FEE_FLOOR);
    });
    assert.equal(BigInt(s.budgetWei),budget);assert.equal(BigInt(s.expectedWei),expected);assert.equal(BigInt(s.ceilingWei),top*2n);
    if(economy)assert.ok(s.schedule.every(x=>BigInt(x.base)<=BASE_FEE_FLOOR*1045n/1000n+1n),'economy keeps the base fee at the floor');
    else assert.ok(s.schedule.some(x=>BigInt(x.base)>BASE_FEE_FLOOR*1000n),'a 4 MiB publication raises the base fee by three orders of magnitude (about 0.01 to 14.5 gwei)');
  }
  const fast=feeSchedule({txs,baseNext:BASE_FEE_FLOOR,tip:GWEI}),slow=feeSchedule({txs,baseNext:BASE_FEE_FLOOR,tip:GWEI,economy:true});
  const burn=(s)=>s.schedule.reduce((n,x)=>n+BigInt(x.gas)*BigInt(x.base),0n);assert.ok(burn(slow)*100n<burn(fast),'economy burns about a hundredth');
  assert.throws(()=>feeSchedule({txs,baseNext:300n*GWEI,tip:GWEI}),/absolute ceiling/);assert.throws(()=>feeSchedule({txs,baseNext:1n,tip:3n*GWEI}),/2 gwei/);
  const top=feeSchedule({txs:[{id:'x',gas:1n}],baseNext:150n*GWEI,tip:GWEI});assert.equal(BigInt(top.ceilingWei),ABSOLUTE_CEILING,'the ceiling never exceeds 200 gwei');
});
test('W4: the fees are read from the node: the largest base reading, the tip capped at 2 gwei',async()=>{
  const answers={eth_maxPriorityFeePerGas:'0x'+(5n*GWEI).toString(16),eth_gasPrice:'0x'+(3n*GWEI).toString(16),eth_feeHistory:{baseFeePerGas:['0x1','0x'+(GWEI/2n).toString(16)]},eth_getBlockByNumber:{baseFeePerGas:'0x'+GWEI.toString(16),gasUsed:'0x0',gasLimit:'0x7a1200'}};
  const f=await readNetworkFees(async m=>answers[m]);assert.equal(f.tip,(2n*GWEI).toString());assert.equal(f.baseNext,GWEI.toString(),'gasPrice 3 gwei − tip 2 gwei = 1 gwei, the largest reading');
  const m=mock();const g=await readNetworkFees(m.rpc);assert.equal(g.baseNext,BASE_FEE_FLOOR.toString());assert.equal(g.tip,GWEI.toString());
  const only=await readNetworkFees(async(method)=>{if(method==='eth_feeHistory')return {baseFeePerGas:['0x'+(7n*GWEI).toString(16)]};throw Object.assign(new Error('x'),{rpc:{code:-32601}});});
  assert.equal(only.baseNext,(7n*GWEI).toString());assert.equal(only.tip,GWEI.toString(),'the node tip falls back to 1 gwei');
  await assert.rejects(readNetworkFees(async()=>{throw Object.assign(new Error('x'),{transport:'down'});}),/base fee/);
});
test('W4: a 173-chunk publication on a chain whose base fee follows London completes with no "fee rose" pause, within the budget',async()=>{
  const m=mock(),p=bigSite(),v=virtual();const fees=await readNetworkFees(m.rpc),q=quotePublish(p,{fees});
  assert.equal(q.transactionCount,177);assert.ok(BigInt(q.expectedWei)<BigInt(q.budgetWei));
  const raws=rawStore();const r=await publishSite(m.options(p,{approval:q.approval,clock:v.clock,sleep:v.sleep,readRate:1000,rawStore:raws}));
  assert.equal(r.verified,true);assert.equal(m.logs.sent.length,177);assert.equal(raws.map.size,177);assert.ok(Object.values(m.state.transactions).every(rec=>rec.raw===undefined&&rec.rawStored),'no raw transaction inside the checkpoint');assert.ok(BigInt(r.maxFeeCommittedWei)<=BigInt(q.budgetWei));
  // Every transaction was signed with exactly its scheduled cap, and the chain's base fee is the projection.
  for(const tx of m.logs.signed){const id=Object.entries(m.state.transactions).find(([,rec])=>rec.tx.nonce===tx.nonce)[0];assert.equal(tx.maxFee,q.approval.schedule[id],id);}
  assert.deepEqual(m.logs.bases.map(String),q.schedule.map(x=>x.base));
  assert.ok(m.logs.bases.some(b=>b>BASE_FEE_FLOOR*1000n));
});
test('W4: a real gasUsed below the declared gas keeps the base fee below the projection: still no pause',async()=>{
  const m=mock(),p=mibSite(),v=virtual();m.chain.gasUsedPercent=82n;
  const r=await publishSite(m.options(p,{approval:quotePublish(p,{fees:FLOOR_FEES}).approval,clock:v.clock,sleep:v.sleep,readRate:1000,rawStore:rawStore()}));assert.equal(r.verified,true);
});
test('W4: an external fee spike pauses before signing, keeps the confirmed chunks, and resumes after a re-approval of the rest',async()=>{
  const m=mock(),p=many(),q=quotePublish(p,{fees:FLOOR_FEES});m.chain.spikeAfter=3;m.chain.spike=200n;
  const err=await publishSite(m.options(p,{approval:q.approval})).catch(e=>e);
  assert.ok(err instanceof PublishFeeRoseError&&err.code==='PUBLISH_FEE_ROSE',String(err));assert.match(err.message,/Confirmed chunks are kept/);
  assert.equal(m.logs.signed.length,3,'no signature past the approval');const saved=m.state,before=Object.keys(saved.transactions).length;assert.equal(before,3);
  // Without an explicit re-approval the saved checkpoint is not reused with other fees.
  const remaining=remainingTransactions(p,saved);assert.equal(remaining.length,planTransactions(p).length-3);
  const q2=quotePublish(p,{fees:await readNetworkFees(m.rpc),txs:remaining});assert.equal(q2.quotedTransactions,remaining.length);
  await assert.rejects(publishSite(m.options(p,{state:saved,approval:q2.approval})),/another publication or approval/);
  const r=await publishSite(m.options(p,{state:saved,approval:q2.approval,reapprove:true}));assert.equal(r.verified,true);
  assert.equal(m.state.approvals.length,2);assert.equal(m.logs.signed.length,planTransactions(p).length,'the confirmed chunks were not signed again');
  const fees=new Set(Object.values(m.state.transactions).map(rec=>rec.tx.maxFee));assert.ok(fees.size>1,'records signed under both approvals');
  // Resume of the finished publication under the second approval: records with different caps pass the ceiling check.
  const again=await publishSite(m.options(p,{state:m.state,approval:q2.approval}));assert.equal(again.site,r.site);
});
test('W4: the ceiling and the budget are enforced; a record above its approval ceiling is refused',async()=>{
  const p=sample(),q=quotePublish(p,{fees:FLOOR_FEES});
  const low=mock();low.chain.base=GWEI;await assert.rejects(publishSite(low.options(p,{approval:q.approval})),e=>e instanceof PublishFeeRoseError&&e.reason==='ceiling');assert.equal(low.logs.signed.length,0);
  const tight=mock();await assert.rejects(publishSite(tight.options(p,{approval:{...q.approval,budgetWei:String(BigInt(q.approval.budgetWei)-1n)}})),e=>e instanceof PublishFeeRoseError&&e.reason==='budget');
  assert.equal(tight.logs.signed.length,planTransactions(p).length-1,'the budget stops the last transaction');
  for(const bad of[{ceilingWei:String(ABSOLUTE_CEILING+1n)},{ceilingWei:'0'},{tipWei:String(3n*GWEI)},{schedule:{x:String(ABSOLUTE_CEILING)}},{schedule:null}])
    await assert.rejects(publishSite(mock().options(p,{approval:{...q.approval,...bad}})),/ceiling|tip|schedule/);
  const m=mock();await publishSite(m.options(p,{approval:q.approval}));const state=structuredClone(m.state),rec=Object.values(state.transactions)[0];
  state.approvals[0].ceilingWei=String(BigInt(rec.tx.maxFee)-1n);await assert.rejects(publishSite(m.options(p,{state,approval:q.approval})),/ceiling|approval/);
});
test('W4: a schema-1 checkpoint of this network is migrated and resumed with its saved raw transaction; another network\'s is refused',async()=>{
  const m=mock(),p=sample(),chunk=p.chunks[0];
  const gas=BigInt(Math.min(200000+chunk.data.length*260,7900000)),maxGas=p.chunks.reduce((n,c)=>n+BigInt(Math.min(200000+c.data.length*260,7900000)),0n)+1500000n+1000000n;
  const legacy=(g)=>toHex(keccak(enc.encode(JSON.stringify([p.manifestHash,owner,String(chainId),g,null,LEGACY_MAX_FEE,LEGACY_PRIORITY_FEE,(maxGas*BigInt(LEGACY_MAX_FEE)).toString()]))));
  const tx={chainId:String(chainId),nonce:'0',maxPriority:LEGACY_PRIORITY_FEE,maxFee:LEGACY_MAX_FEE,gas:gas.toString(),to:FACTORY,value:'0',data:abiBytes('store(bytes)',chunk.data)};
  const signed=signTx(secret,tx);
  const v1={schema:1,binding:legacy(genesis),site:null,version:null,versionCountBefore:null,transactions:{['chunk:'+chunk.address]:{...signed,tx,receipt:null}},spentCapWei:(gas*BigInt(LEGACY_MAX_FEE)).toString()};
  const migrated=migrateCheckpoint(v1,{plan:p,address:owner,chainId,genesisHash:genesis});assert.equal(migrated.schema,2);assert.equal(migrated.migratedFrom,1);assert.equal(migrated.approvals[0].ceilingWei,LEGACY_MAX_FEE);
  await assert.rejects(publishSite(m.options(p,{state:v1})),/another publication or approval/,'the migrated checkpoint needs a re-approval of the remaining fees');
  const r=await publishSite(m.options(p,{state:v1,reapprove:true}));assert.equal(r.verified,true);assert.equal(m.logs.sent[0],signed.raw,'the saved legacy raw transaction is broadcast, not replaced');
  assert.equal(m.logs.signed.length,planTransactions(p).length-1);
  const foreign={...v1,binding:legacy('0x'+'cd'.repeat(32))};
  await assert.rejects(publishSite(mock().options(p,{state:foreign,reapprove:true})),/another network/);
  const other=structuredClone(m.state);other.network.genesisHash='0x'+'cd'.repeat(32);await assert.rejects(publishSite(m.options(p,{state:other})),/another network/);
});
test('W4: the key lease: a publication longer than LOCK_MS completes, Lock still aborts it, and the job deadline bounds it',async()=>{
  const LOCK=15*60*1000,v=virtual();
  const run=async({lockAfter=null,deadline}={})=>{
    const m=mock(),p=many(),lease=new KeyLease({lockMs:LOCK,clock:v.clock}),abort=new AbortController();lease.unlock();lease.startJob('job-1',deadline);let signatures=0;
    // Each block takes four minutes; the page's one-second lock check aborts the job once the lease has expired.
    m.chain.clockOnSend=()=>{v.advance(4*60*1000);if(!lease.unlocked)abort.abort();};
    const sign=async tx=>{if(!lease.unlocked||abort.signal.aborted)throw new Error('Wallet locked or publication paused');const s=signTx(secret,tx);lease.signed('job-1');if(++signatures===lockAfter){lease.lock();abort.abort();}return s;};
    const out=await publishSite(m.options(p,{sign,signal:abort.signal,clock:v.clock,sleep:v.sleep,readRate:1000})).then(r=>({r}),e=>({e}));
    return {...out,m,signatures,lease};
  };
  const start=v.now,done=await run();assert.equal(done.r?.verified,true,String(done.e));assert.ok(v.now-start>LOCK,'the publication outlasted LOCK_MS');
  assert.equal(done.lease.signed('job-2'),false,'another job cannot extend the lease');
  const locked=await run({lockAfter:2});assert.equal(locked.e?.code,'PUBLISH_PAUSED');assert.equal(locked.m.logs.signed.length===undefined?0:locked.signatures,2);assert.equal(locked.lease.unlocked,false);
  const bounded=await run({deadline:20*60*1000});assert.ok(bounded.e,'the job deadline stops the lease');assert.ok(bounded.signatures<planTransactions(many()).length);
  const lease=new KeyLease({lockMs:LOCK,clock:v.clock});assert.throws(()=>lease.startJob('x'),/Unlock/);lease.unlock();assert.throws(()=>lease.startJob('x',JOB_DEADLINE_MAX_MS+1),/6 hours/);
  lease.startJob('x');lease.endJob('x');assert.equal(lease.signed('x'),false,'an ended job no longer extends the lease');
});
test('W4: the safe sweep stores a chunk that was seen at latest and then reorged out',async()=>{
  const m=mock(),p=sample(),other=p.chunks[1];m.codes.set(other.address,toHex(concat(Uint8Array.of(0),other.data)));m.chain.unsafe.add(other.address);m.chain.reorgOut.add(other.address);
  const phases=[];const r=await publishSite(m.options(p,{onProgress:e=>phases.push(e)}));assert.equal(r.verified,true);
  assert.ok(m.state.transactions['chunk:'+other.address],'the vanished chunk was stored by this publication');
  assert.ok(phases.some(e=>e.phase==='restore'&&e.chunk===other.address));
  const createdAt=m.logs.signed.findIndex(tx=>tx.data.startsWith(selector('createVersion(bytes32,uint32,address[])')));
  const storedAt=m.logs.signed.findIndex(tx=>tx.to===FACTORY&&tx.data===abiBytes('store(bytes)',other.data));assert.ok(storedAt>=0&&storedAt<createdAt,'stored before createVersion');
});
test('W4: the sweep waits, bounded, for chunks to reach safe',async()=>{
  const m=mock(),p=sample(),v=virtual();m.chain.unsafe.add(p.chunks[0].address);m.codes.set(p.chunks[0].address,toHex(concat(Uint8Array.of(0),p.chunks[0].data)));
  let reads=0;const o=m.options(p,{clock:v.clock,sleep:v.sleep,sweepTimeoutMs:5000,pollIntervalMs:1000});const inner=o.rpc;
  o.rpc=async(method,params)=>{if(method==='eth_getCode'&&params[1]==='safe'&&params[0]===p.chunks[0].address&&++reads===3)m.chain.unsafe.delete(p.chunks[0].address);return inner(method,params);};
  assert.equal((await publishSite(o)).verified,true);assert.equal(reads,3);
  const stuck=mock();stuck.chain.unsafe.add(p.chunks[0].address);stuck.codes.set(p.chunks[0].address,toHex(concat(Uint8Array.of(0),p.chunks[0].data)));const w=virtual();
  await assert.rejects(publishSite(stuck.options(p,{clock:w.clock,sleep:w.sleep,sweepTimeoutMs:5000,pollIntervalMs:1000})),e=>e instanceof PublishPendingError&&/safe height/.test(e.message));
  assert.equal(stuck.logs.signed.some(tx=>tx.data.startsWith(selector('createVersion(bytes32,uint32,address[])'))),false,'no createVersion over a chunk that is not at safe');
});
test('W4: economy pacing signs one publication transaction every second block and keeps the base fee at the floor',async()=>{
  const m=mock(),p=mibSite(),v=virtual();m.control.emptyBlockOnPoll=true;const q=quotePublish(p,{fees:FLOOR_FEES,economy:true});
  assert.ok(BigInt(q.budgetWei)<BigInt(quotePublish(p,{fees:FLOOR_FEES}).budgetWei));
  const r=await publishSite(m.options(p,{approval:q.approval,clock:v.clock,sleep:v.sleep,readRate:1000,pollIntervalMs:1000,rawStore:rawStore()}));assert.equal(r.verified,true);
  const blocks=Object.values(m.state.transactions).map(rec=>BigInt(rec.receipt.blockNumber)).sort((a,b)=>a<b?-1:a>b?1:0);
  for(let i=1;i<blocks.length;i++)assert.ok(blocks[i]-blocks[i-1]>=2n,'one publication transaction every second block');
  assert.ok(m.logs.bases.every(b=>b<=BASE_FEE_FLOOR*1045n/1000n+1n),'the base fee stays at the floor');
  assert.equal(quotePublish(p,{fees:FLOOR_FEES}).approval.economy,false,'economy is off by default');
});
test('W4: jobs of another network never block the wallet, and are archived read-only',async()=>{
  const NET={chainId:chainId,genesisHash:genesis},addr=owner;
  const signedJob=(network,chain=chainId)=>({id:'j',status:'paused',walletAddress:addr,network,checkpoint:{transactions:{'chunk:x':{tx:{chainId:String(chain)}}}}});
  assert.equal(blocksAccount(signedJob({chainId:String(chainId),genesisHash:genesis}),addr.toUpperCase().replace('0X','0x'),NET),true);
  assert.equal(isForeignJob(signedJob({chainId:'1599846715',genesisHash:'0x'+'cd'.repeat(32)}),{chainId:1599846715,genesisHash:genesis}),true,'same chain id, other genesis');
  assert.equal(blocksAccount(signedJob({chainId:'777',genesisHash:genesis}),addr,NET),false,'another chain id');
  const legacy=signedJob(undefined,1599846715);assert.deepEqual(jobNetwork(legacy),{chainIds:['1599846715'],genesisHash:null});
  assert.equal(isForeignJob({status:'paused',checkpoint:{transactions:{upload:{raw:'0x02'}}}},NET),false,'a record without a chain id proves nothing: the job still blocks');
  assert.equal(isForeignJob(legacy,{chainId:4242,genesisHash:genesis}),true,'a pre-W4 job of the retired chain id');assert.equal(isForeignJob(legacy,{chainId:1599846715,genesisHash:genesis}),false);
  assert.equal(isForeignJob({id:'u',status:'awaitingApproval'},NET),false,'an unsigned job without identity is not foreign');
  const data=new Map([['studio:job:a',signedJob({chainId:'777',genesisHash:genesis})],['ide:job:b',{...signedJob(undefined,777),kind:'contract'}],['studio:job:c',signedJob({chainId:String(chainId),genesisHash:genesis})],['studio:wallet',{x:1}]]);
  const storage={async get(){return Object.fromEntries([...data].map(([k,v])=>[k,structuredClone(v)]));},async set(o){for(const[k,v]of Object.entries(o))data.set(k,structuredClone(v));},async remove(k){data.delete(k);}};
  assert.deepEqual((await archiveForeignJobs(storage,NET,5)).sort(),['ide:job:b','studio:job:a']);
  assert.deepEqual([...data.keys()].sort(),['archive:ide:job:b','archive:studio:job:a','studio:job:c','studio:wallet']);
  const list=await archivedJobs(storage);assert.equal(list.length,2);assert.equal(list[0].job.archivedReason,'foreign-network');assert.equal(list[0].job.archivedAt,5);
  await archiveJob(storage,'studio:job:c',data.get('studio:job:c'),'cancelled-foreign');assert.equal(data.has('studio:job:c'),false);
});
test('W4: a foreign-network checkpoint is refused by the publisher before any signature',async()=>{
  const m=mock();await assert.rejects(publishSite(m.options(sample(),{chainId:777})),/identity/);assert.equal(m.logs.signed.length,0);
  const n=mock();await publishSite(n.options());const st=structuredClone(n.state);st.network.chainId='777';await assert.rejects(publishSite(n.options(undefined,{state:st})),/another network/);
});

// ---------------- Sites W5: publish input (shared vectors), MIME ids 12 and 13 ----------------
const W5=JSON.parse(readFileSync(new URL('./data/publish-input-vectors.json',import.meta.url),'utf8'));
test('W5: the hard-deny list and the exclusion globs follow the shared vectors',()=>{
  for(const[path,reason]of W5.deny)assert.equal(denyReason(path),reason,path);
  for(const[pattern,path,match]of W5.exclude)assert.equal(globRegex(pattern).test(path.slice(1)),match,pattern+' '+path);
  const s=selectFiles(W5.select.paths,{exclude:W5.select.exclude});
  assert.deepEqual(s.kept,W5.select.kept);assert.deepEqual(s.denied,W5.select.denied.map(([path,reason])=>({path,reason})));assert.deepEqual(s.excluded,W5.select.excluded);
  const inc=selectFiles(W5.include.paths,{include:parseIncludeList(W5.include.text)});
  assert.deepEqual(inc.kept,W5.include.kept);assert.deepEqual(inc.denied,W5.include.denied.map(([path,reason])=>({path,reason})));assert.deepEqual(inc.excluded,W5.include.excluded);assert.deepEqual(inc.missing,W5.include.missing);
  assert.throws(()=>parseIncludeList('# only\n'),/empty/);assert.throws(()=>parseIncludeList('a\n/a'),/Duplicate/);assert.throws(()=>globRegex(''),/pattern/);
});
test('W5: the publisher refuses a hard-denied file loudly, whatever its extension, and still refuses unsupported ones',()=>{
  for(const path of['/.htaccess.txt','/api/data.json','/config.js','/SHA256SUMS.txt','/old.bak.html','/.git/x.txt'])
    assert.throws(()=>prepareSite([{path:'/index.html',data:enc.encode('x')},{path,data:enc.encode('y')}]),e=>/Refused file/.test(e.message)&&e.message.includes(path),path);
  assert.throws(()=>prepareSite([{path:'/index.html',data:enc.encode('x')},{path:'/x.php',data:enc.encode('y')}]),/Unsupported file extension: \/x\.php/);
});
test('W5: favicon.ico (12, image/x-icon) and site.webmanifest (13, application/manifest+json) are published and decoded; 11 stays unknown',()=>{
  for(const[path,id,mime]of W5.mime){assert.equal(mimeIdFor(path),id,path);if(id!==null)assert.equal(MIME.get(id),mime);}
  const p=prepareSite([{path:'/index.html',data:enc.encode('<link rel="icon" href="favicon.ico">')},{path:'/favicon.ico',data:new Uint8Array([0,0,1,0])},{path:'/site.webmanifest',data:enc.encode('{"name":"x"}')}]);
  const d=decodeManifest(p.manifest);assert.deepEqual(d.files.map(f=>[f.path,f.mime]),[['/favicon.ico',12],['/index.html',1],['/site.webmanifest',13]]);
  for(const id of W5.unknownMimeIds)assert.equal(MIME.has(id),false,'mime '+id);
});
test('W5: the S1 explorer include list publishes 12 files from the tracked snapshot, with the expected unique chunks',()=>{
  const root=new URL('../../../deploy/websites/explorer.pocol.net/',import.meta.url);
  const include=parseIncludeList(readFileSync(new URL('../tools/explorer-s1-include.txt',root),'utf8'));
  assert.equal(include.length,12);assert.ok(include.every(path=>denyReason(path)===null&&mimeIdFor(path)!==null),'nothing denied or unsupported is listed');
  const files=include.map(path=>({path,data:new Uint8Array(readFileSync(new URL(path.slice(1),root)))}));
  const p=prepareSite(files);const fileChunks=p.files.reduce((n,f)=>n+f.chunks.length,0);
  // icon-512.png and pocol-icon.png are byte-identical, so their 7 chunks are stored once.
  assert.equal(p.manifestChunks.length,1);assert.equal(fileChunks,75);assert.equal(p.chunks.length,69,"68 unique file chunks and the manifest");assert.equal(planTransactions(p).length,73,"69 chunks, deploy, createVersion, publish, setCurrent");
});
