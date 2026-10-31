import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';
import {build} from '../build.mjs';
import {installChromeMock,loadWorker} from './chrome_mock.mjs';
const {dist,manifest}=build();
const C=await import('../dist/lib/contracts.js'),N=await import('../dist/lib/studio-network.js'),W=await import('../dist/lib/wallet.js');
const SECRET=new Uint8Array(32);SECRET[31]=1;const ADDRESS=W.addressOf(SECRET),OTHER='0x2222222222222222222222222222222222222222';
const REQUEST={schema:1,action:'deploy',title:'Counter',abi:['constructor(uint256 initial)'],bytecode:'0x60006000',args:['7'],valueWei:'0'};
const BLOCKHASH='0x'+'ab'.repeat(32);
function harness(overrides={}) {
  const state={chain:'0x5f5bb93b',genesis:N.NETWORK.genesisHash,nonce:'0x2',balance:'0x3635c9adc5dea00000',receipt:null,checkpoint:null,...overrides},calls=[],events=[];
  const rpc=async(method,params)=>{calls.push({method,params});
    switch(method){
      case 'eth_chainId':return state.chain;
      case 'pocol_status':return {chainId:N.NETWORK.chainId,genesisHash:state.genesis};
      case 'eth_getBlockByNumber':return params[0]==='latest'?{baseFeePerGas:'0x3b9aca00',gasLimit:'0x7a1200'}:{hash:state.canonicalHash||BLOCKHASH};
      case 'eth_getTransactionCount':return state.nonce;
      case 'eth_getBalance':return state.balance;
      case 'eth_estimateGas':return '0x186a0';
      case 'eth_getCode':return '0x6000';
      case 'eth_getTransactionReceipt':return state.receipt;
      case 'eth_sendRawTransaction':events.push('broadcast');assert.ok(state.checkpoint,'broadcast requires durable checkpoint');if(state.broadcastError)throw Error('Transport timed out');return state.checkpoint.transactions.contract.hash;
      default:throw Error('Unexpected RPC '+method);
    }
  };
  return {state,calls,events,rpc,address:ADDRESS,request:REQUEST,sign:async tx=>{events.push('sign');return W.signTx(SECRET,tx);},loadCheckpoint:async()=>state.checkpoint,saveCheckpoint:async cp=>{events.push('save');if(state.failSave)throw Error('Storage failed');state.checkpoint=structuredClone(cp);}};
}
const prepared=async ctx=>C.prepareContractReview(ctx);
const signedCalls=ctx=>ctx.calls.filter(c=>c.method==='eth_sendRawTransaction');

test('bounded public intent independently encodes ABI and ignores unknown fields',()=>{
 const r=C.validateContractRequest({...REQUEST,endpoint:'https://evil',privateKey:'not-stored'});
 assert.equal(r.data.length,REQUEST.bytecode.length+64);assert.equal(r.endpoint,undefined);assert.equal(r.privateKey,undefined);
 assert.equal(r.requestHash,C.validateContractRequest(r).requestHash);
 assert.throws(()=>C.validateContractRequest({...REQUEST,title:'x'.repeat(C.IDE_REQUEST_MAX)}),{code:'REQUEST_TOO_LARGE'});
 assert.throws(()=>C.validateContractRequest({...REQUEST,bytecode:'0x'+'00'.repeat(49153)}),{code:'INVALID_REQUEST'});
 assert.throws(()=>C.validateContractRequest({...REQUEST,bytecode:'0x'+'00'.repeat(49152)}),{code:'INVALID_REQUEST'});
});
test('ABI validation rejects boolean strings, unsafe integers and malformed nested values',()=>{
 const r={...REQUEST,abi:['constructor(tuple(bool flag,uint256 amount)[] members)'],args:[[[false,'1000000000000000001']]]};
 assert.deepEqual(C.validateContractRequest(r).args,[[[false,'1000000000000000001']]]);
 assert.throws(()=>C.validateContractRequest({...r,args:[[['false','1']]]}),{code:'INVALID_REQUEST'});
 assert.throws(()=>C.validateContractRequest({...r,args:[[[false,1000000000000000001]]]}),{code:'INVALID_REQUEST'});
 assert.throws(()=>C.validateContractRequest({...REQUEST,abi:['constructor(uint8 v)'],args:['256']}),{code:'INVALID_REQUEST'});
 assert.throws(()=>C.validateContractRequest({...REQUEST,abi:[{type:'notvalid'}]}),{code:'INVALID_REQUEST'});
});
test('JSON payable constructor accepts value without changing the existing wallet address',async()=>{
 const ctx=harness();ctx.request={...REQUEST,abi:[{type:'constructor',stateMutability:'payable',inputs:[]}],args:[],valueWei:'1'};
 const review=await prepared(ctx);assert.equal(review.from,ADDRESS);assert.equal(review.valueWei,'1');
 assert.equal(review.tx.to,null);assert.equal(review.tx.chainId,String(N.NETWORK.chainId));
 assert.equal(ctx.events.length,0);assert.equal(Object.isFrozen(review),true);assert.equal(Object.isFrozen(review.tx),true);
});
test('wrong chain and genesis block preparing before any signature',async()=>{
 for(const changes of [{chain:'0x1'},{genesis:BLOCKHASH}]){const ctx=harness(changes);await assert.rejects(prepared(ctx),{code:'WRONG_NETWORK'});assert.equal(ctx.events.length,0);}
});
test('nonpayable, read methods, malformed targets, EOA targets and insufficient funds fail',async()=>{
 assert.throws(()=>C.validateContractRequest({...REQUEST,valueWei:'1'}),{code:'NON_PAYABLE'});
 assert.throws(()=>C.validateContractRequest({...REQUEST,action:'call',to:OTHER,method:'read',abi:['function read() view returns(uint256)'],args:[]}),{code:'INVALID_REQUEST'});
 const poor=harness({balance:'0x0'});await assert.rejects(prepared(poor),{code:'INSUFFICIENT_FUNDS'});
 const eoa=harness();eoa.request={...REQUEST,action:'call',to:OTHER,method:'set',abi:['function set(uint256 value)']};const original=eoa.rpc;eoa.rpc=(m,p)=>m==='eth_getCode'?'0x':original(m,p);await assert.rejects(prepared(eoa),{code:'NO_CONTRACT'});
});
test('stale nonce, expired review, changed identity or address never sign',async()=>{
 for(const kind of ['nonce','expiry','genesis','account']){const ctx=harness();const review=await prepared(ctx);if(kind==='nonce')ctx.state.nonce='0x3';if(kind==='genesis')ctx.state.genesis=BLOCKHASH;const options={...ctx,review,...(kind==='expiry'?{now:()=>review.expiresAt+1}:{}),...(kind==='account'?{address:OTHER}:{})};await assert.rejects(C.submitContractReview(options));assert.equal(ctx.events.length,0);}
});
test('fresh review cannot be replaced by a serialized or edited review',async()=>{
 const ctx=harness(),review=await prepared(ctx);await assert.rejects(C.submitContractReview({...ctx,review:structuredClone(review)}),{code:'INVALID_REVIEW'});assert.equal(ctx.events.length,0);
});
test('signature is checkpointed before broadcast and an ambiguous result preserves the known hash',async()=>{
 const ctx=harness({broadcastError:true}),review=await prepared(ctx),result=await C.submitContractReview({...ctx,review});
 assert.deepEqual(ctx.events,['sign','save','broadcast']);assert.equal(result.state,'pending');assert.equal(result.hash,ctx.state.checkpoint.transactions.contract.hash);assert.ok(ctx.state.checkpoint.transactions.contract.raw.startsWith('0x02'));
});
test('resume retransmits exact durable bytes after quote expiry without signing another nonce',async()=>{
 const ctx=harness({broadcastError:true}),review=await prepared(ctx);await C.submitContractReview({...ctx,review});const raw=ctx.state.checkpoint.transactions.contract.raw;ctx.state.nonce='0x3';
 const result=await C.submitContractReview({...ctx,review:structuredClone(review),now:()=>review.expiresAt+999999,sign:()=>{throw Error('Must never sign again');}});
 assert.equal(result.state,'pending');assert.equal(signedCalls(ctx).length,2);assert.equal(signedCalls(ctx)[1].params[0],raw);assert.equal(ctx.events.filter(x=>x==='sign').length,1);
});
test('storage failure prevents any broadcast',async()=>{
 const ctx=harness({failSave:true}),review=await prepared(ctx);await assert.rejects(C.submitContractReview({...ctx,review}),/Storage failed/);assert.deepEqual(ctx.events,['sign','save']);assert.equal(signedCalls(ctx).length,0);
});
test('a signer changing value or recipient is rejected before durable storage or broadcast',async()=>{
 const ctx=harness(),review=await prepared(ctx);await assert.rejects(C.submitContractReview({...ctx,review,sign:tx=>W.signTx(SECRET,{...tx,value:'1'})}),{code:'INVALID_SIGNATURE'});assert.equal(signedCalls(ctx).length,0);assert.equal(ctx.state.checkpoint,null);
});
test('parallel submissions cannot sign or broadcast twice',async()=>{
 const ctx=harness(),review=await prepared(ctx);const result=await Promise.allSettled([C.submitContractReview({...ctx,review}),C.submitContractReview({...ctx,review})]);assert.equal(result.filter(r=>r.status==='fulfilled').length,1);assert.equal(result.find(r=>r.status==='rejected').reason.code,'REQUEST_BUSY');assert.equal(signedCalls(ctx).length,1);
});
test('read-only receipt reconciliation validates canonical block and never broadcasts',async()=>{
 const ctx=harness(),review=await prepared(ctx);await C.submitContractReview({...ctx,review});const hash=ctx.state.checkpoint.transactions.contract.hash;ctx.state.receipt={transactionHash:hash,status:'0x1',blockHash:BLOCKHASH,blockNumber:'0x1',contractAddress:review.predictedAddress};
 const before=signedCalls(ctx).length;const checked=await C.checkContractReceipt({...ctx,checkpoint:ctx.state.checkpoint});assert.equal(checked.state,'included');assert.equal(signedCalls(ctx).length,before);
 ctx.state.canonicalHash='0x'+'ef'.repeat(32);assert.equal((await C.checkContractReceipt({...ctx,checkpoint:ctx.state.checkpoint})).state,'pending');
 ctx.state.canonicalHash=BLOCKHASH;ctx.state.receipt.status='0x0';assert.equal((await C.checkContractReceipt({...ctx,checkpoint:ctx.state.checkpoint})).state,'reverted');
});
test('a modified checkpoint cannot resume or reconcile',async()=>{
 const ctx=harness(),review=await prepared(ctx);await C.submitContractReview({...ctx,review});ctx.state.checkpoint.transactions.contract.tx.value='1';await assert.rejects(C.submitContractReview({...ctx,review}),{code:'CHECKPOINT_MISMATCH'});await assert.rejects(C.checkContractReceipt({...ctx,checkpoint:ctx.state.checkpoint}),{code:'CHECKPOINT_MISMATCH'});assert.equal(signedCalls(ctx).length,1);
});
test('service worker validates top-frame IDE origin/tab binding and exposes no wallet APIs',async t=>{
 const schedule=globalThis.setTimeout;t.mock.method(globalThis,'setTimeout',(fn,ms,...args)=>{const timer=schedule(fn,ms,...args);timer.unref?.();return timer;});
 const mock=installChromeMock();await loadWorker(dist,'ide-tests');const sender={id:chrome.runtime.id,url:'https://ide.pocol.net/',frameId:0,tab:{id:73}};
 const call=(type,payload={},who=sender)=>new Promise(resolve=>{const keep=mock.listeners.message[0]({type,payload},who,resolve);if(keep!==true)resolve(undefined);});
 const status=await call('ide.status');assert.equal(status.ok,true);assert.equal(status.result.version,manifest.version);assert.equal(status.result.wallet,undefined);assert.ok(status.result.capabilities.includes('contractTransactions'));
 for(const who of [{...sender,url:'https://ide.pocol.net.evil/'},{...sender,frameId:1},{...sender,url:'https://studio.pocol.net/'}])assert.equal((await call('ide.status',{},who)).ok,false);
 for(const method of ['wallet.unlock','wallet.address','profiles.add','ide.eth_sendRawTransaction'])assert.equal((await call(method)).ok,false);
 const opened=await call('ide.openContract',REQUEST);assert.equal(opened.ok,true);assert.match(mock.tabsCreated.at(-1).url,/contract.html#/);const id=opened.result.jobId;
 const job=mock.storage.data.get(C.IDE_JOB_PREFIX+id);assert.equal(job.status,'awaitingApproval');assert.equal(job.checkpoint,undefined);assert.equal(job.origin,'https://ide.pocol.net');assert.equal(job.sourceTabId,73);
 assert.equal((await call('ide.jobStatus',{jobId:id})).ok,true);assert.equal((await call('ide.jobStatus',{jobId:id},{...sender,tab:{id:74}})).ok,false);assert.equal((await call('ide.jobStatus',{jobId:id},{...sender,url:'https://pocol-ide.mystic-gecko-9511.chatgpt.site/'})).ok,false);
 const oldId='00000000-0000-0000-0000-000000000000';await mock.storage.set({[C.IDE_JOB_PREFIX+oldId]:{id:oldId,origin:sender.url,createdAt:1,status:'pending',checkpoint:{transactions:{contract:{raw:'0x02'}}}}});await call('ide.openContract',REQUEST);assert.ok(mock.storage.data.has(C.IDE_JOB_PREFIX+oldId));
});
test('content script filters same window/origin, exposes only three intent methods',async()=>{
 let listener;const replies=[],calls=[];const window={top:null,postMessage:(...args)=>replies.push(args),addEventListener:(_type,fn)=>listener=fn};window.top=window;
 vm.runInNewContext(readFileSync(dist+'/ide-content.js','utf8'),{window,location:{origin:'https://ide.pocol.net'},chrome:{runtime:{sendMessage:async message=>{calls.push(message);return {ok:true,result:{version:'0.3.1'}};}}}});
 const payload={source:'pocol-ide',id:'request-1',method:'status'};
 await listener({source:{},origin:'https://ide.pocol.net',data:payload});await listener({source:window,origin:'https://evil',data:payload});await listener({source:window,origin:'https://ide.pocol.net',data:{...payload,method:'wallet.unlock'}});assert.equal(calls.length,0);
 await listener({source:window,origin:'https://ide.pocol.net',data:payload});assert.deepEqual(JSON.parse(JSON.stringify(calls[0])),{type:'ide.status',payload:{}});assert.equal(replies[0][0].source,'pocol-ide-extension');assert.equal(replies[0][1],'https://ide.pocol.net');
 assert.deepEqual(manifest.host_permissions,['http://127.0.0.1/*','https://gateway.pocol.net/*']);assert.equal(manifest.content_scripts[1].all_frames,false);assert.equal(manifest.externally_connectable,undefined);
});
