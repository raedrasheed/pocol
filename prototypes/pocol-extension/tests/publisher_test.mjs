import test from 'node:test';
import assert from 'node:assert/strict';
import {prepareSite,quotePublish,publishSite,selector,word,abiUint,PublishPendingError,PublishRevertedError,FACTORY} from '../dist/lib/publisher.js';
import {WEBSITE_INITCODE,WEBSITE_RUNTIME} from '../dist/lib/website-bytecode.js';
import {concat,toHex,fromHex} from '../dist/lib/hex.js';
import {decodeManifest,chunkAddress,decodeCurrent} from '../dist/lib/site.js';
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
function mock() {
  const codes=new Map(),receipts=new Map(),versions=[];let nonce=0n,current=null,site=null;
  const logs={signed:[],sent:[],calls:[]};
  const control={allow:true,wrongNetwork:false,failAfterSend:null,unknownBeforeSend:false,noReceipt:false,failSave:false,wrongHash:false,revert:false,publishAfterCreate:false,wrongVersionChunks:false,orphanBlock:false};
  const n=(b)=>b.length?BigInt(toHex(b)):0n;
  function versionOut(v){const x=versions[Number(v)];if(!x) throw new Error('BadVersion');return toHex(concat(fromHex(x.hash),word(x.length),word(x.status),word(42),word(160),dynamic(x.chunks)));}
  async function rpc(method,params) {
    logs.calls.push([method,params]);
    if(method==='eth_chainId')return '0x'+(control.wrongNetwork?1:chainId).toString(16);
    if(method==='pocol_status')return {genesisHash:genesis};
    if(method==='eth_getBalance')return '0x'+(10n**30n).toString(16);
    if(method==='eth_getTransactionCount')return '0x'+nonce.toString(16);
    if(method==='eth_getCode')return codes.get(params[0])||'0x';
    if(method==='eth_getTransactionReceipt')return control.noReceipt?null:(receipts.get(params[0])||null);
    if(method==='eth_getBlockByNumber')return {hash:control.orphanBlock?'0x'+'dd'.repeat(32):blockHash,number:'0x2a'};
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
      const receipt={transactionHash:hash,status:control.revert?'0x0':'0x1',blockNumber:'0x2a',blockHash,contractAddress:null};
      if(!control.revert){
        if(!to){assert.equal(toHex(data),WEBSITE_INITCODE);site=toHex(keccak(rlp.encode([fromHex(owner),rlp.encUint(txNonce)])).subarray(12));codes.set(site,WEBSITE_RUNTIME);receipt.contractAddress=site;}
        else if(to===FACTORY){const size=Number(n(data.subarray(36,68))),chunk=data.subarray(68,68+size);codes.set(toHex(chunkAddress(chunk)),toHex(concat(Uint8Array.of(0),chunk)));}
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
  const options=(plan=sample(),more={})=>({plan,rpc,address:owner,chainId,genesisHash:genesis,maxTotalFeeWei:quotePublish(plan).maxFeeWei,sign:async tx=>{logs.signed.push(tx);return signTx(secret,tx);},onCheckpoint:async s=>{if(control.failSave)throw new Error('Storage full');state=s;},pollIntervalMs:0,receiptTimeoutMs:10,...more});
  return {rpc,logs,control,codes,receipts,versions,options,reorg(){codes.clear();receipts.clear();versions.length=0;nonce=0n;current=null;site=null;},get state(){return state;},get site(){return site;}};
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
test('insufficient approved fee cannot trigger a signature',async()=>{const m=mock();await assert.rejects(publishSite(m.options(undefined,{maxTotalFeeWei:'1'})),/maximum fee/);assert.equal(m.logs.signed.length,0);});
test('checkpoint persistence failure prevents broadcast',async()=>{const m=mock();m.control.failSave=true;await assert.rejects(publishSite(m.options()),/Storage full/);assert.equal(m.logs.sent.length,0);});
test('checkpoint failure after signing still prevents broadcast',async()=>{const m=mock();let count=0;await assert.rejects(publishSite(m.options(undefined,{onCheckpoint:async()=>{if(++count===2)throw new Error('Storage full');}})),/Storage full/);assert.equal(m.logs.signed.length,1);assert.equal(m.logs.sent.length,0);});
test('signer cannot substitute another transaction',async()=>{const m=mock();await assert.rejects(publishSite(m.options(undefined,{sign:async tx=>signTx(secret,{...tx,to:owner,value:'1'})})),/changed transaction/);assert.equal(m.logs.sent.length,0);});
test('wrong signing key is rejected',async()=>{const m=mock();await assert.rejects(publishSite(m.options(undefined,{sign:async tx=>signTx(fromHex('0x'+'22'.repeat(32)),tx)})),/publishing account/);assert.equal(m.logs.sent.length,0);});
test('ambiguous submission resumes the SAME raw without another signature',async()=>{const m=mock();m.control.unknownBeforeSend=true;await assert.rejects(publishSite(m.options()),PublishPendingError);const saved=m.state,raw=m.logs.sent[0];m.control.unknownBeforeSend=false;await publishSite(m.options(undefined,{state:saved}));assert.equal(m.logs.sent[1],raw);assert.equal(m.logs.signed.length,7);});
test('lost submit response reconciles chain receipt without duplicate signing',async()=>{const m=mock();m.control.failAfterSend='all';await publishSite(m.options());assert.equal(m.logs.signed.length,7);assert.equal(m.logs.sent.length,7);});
test('receipt timeout preserves pending transaction and resumes without re-signing',async()=>{const m=mock();m.control.noReceipt=true;await assert.rejects(publishSite(m.options()),PublishPendingError);const saved=m.state;m.control.noReceipt=false;await publishSite(m.options(undefined,{state:saved}));assert.equal(m.logs.signed.length,7);assert.equal(m.logs.sent.length,7);});
test('completed checkpoint is idempotent',async()=>{const m=mock();const first=await publishSite(m.options());const r=await publishSite(m.options(undefined,{state:m.state}));assert.equal(r.site,first.site);assert.equal(m.logs.signed.length,7);});
test('checkpoint cannot be reused for a different document or allowance',async()=>{const m=mock();await publishSite(m.options());await assert.rejects(publishSite(m.options(undefined,{state:m.state,maxTotalFeeWei:'999999999999999999999'})),/another publication/);});
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
