import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';

const SOURCE = readFileSync(new URL('../src/contract-ui.js', import.meta.url), 'utf8').replace(/^import .*;\n/gm, '').replace(/^init\(\)\.catch\(.*\);\n?$/m, '');
const HTML = readFileSync(new URL('../src/contract.html', import.meta.url), 'utf8');
const JOB_ID = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
const ADDRESS = '0x1111111111111111111111111111111111111111';
const HASH = '0x' + 'ab'.repeat(32);
const KEY = 'ide:job:' + JOB_ID;
function makeElement(id) { return {id, textContent:'',hidden:false,disabled:false,checked:false,value:'',files:[],children:[],replaceChildren(...items){this.children=items;}}; }
async function fixture(options = {}) {
  let now = 1000;
  class Clock extends Date { static now() { return now; } }
  const elements = new Map([...HTML.matchAll(/id="([^"]+)"/g)].map(match => [match[1], makeElement(match[1])]));
  const state = { prepared:0, submitted:0, signed:0, locks:[], generatedKeys:[], ...options };
  const baseReview = {from:ADDRESS,to:null,predictedAddress:'0x2222222222222222222222222222222222222222',gas:'100000',valueWei:'0',maxFeeWei:'100000000000000',maxCostWei:'100000000000000',calldataHash:HASH,dataBytes:2,expiresAt:121000,requestHash:HASH,tx:{chainId:'1599846715',nonce:'0',maxPriority:'1000000000',maxFee:'1000000000',gas:'100000',to:null,value:'0',data:'0x6000'}};
  const job = {id:JOB_ID,origin:'https://ide.pocol.net',request:{schema:1,action:'deploy',title:'Counter',bytecode:'0x6000',abi:[],args:[],valueWei:'0'},status:'awaitingApproval',...options.job};
  const storage = new Map([[KEY,structuredClone(job)],['studio:wallet',{schema:'pocol-studio-vault-v1'}]]);
  if(options.otherJob)storage.set(options.otherJob[0],options.otherJob[1]);
  let tick; const listeners = new Map();
  const globals = {
    Date:Clock, Uint8Array, TextEncoder, structuredClone, AbortController, console,
    location:{hash:'#'+JOB_ID}, document:{getElementById:id=>{assert.ok(elements.has(id),id);return elements.get(id)},createElement:tag=>makeElement(tag)},
    window:{addEventListener:(name,callback)=>listeners.set(name,callback)},setInterval:fn=>{tick=fn},
    NETWORK:{chainId:1599846715,endpoint:'https://gateway.pocol.net/studio/rpc'},LOCK_MS:900000,
    addressOf:()=>ADDRESS, importKeyfile:async()=>{state.importEntered=true;const key = options.importGate ? await options.importGate : new Uint8Array(32).fill(7);state.generatedKeys.push(key);return key;}, exportKeyfile:async()=>({encrypted:true}),
    signTx:()=>{state.signed++;return{raw:'0x02',hash:HASH}}, Pool:class{},TRUST_POOL:1,transportRequest:async()=>{throw Error('No network should be used by this mocked UI test')},
    prepareContractReview:async()=>{state.prepared++;return baseReview},
    submitContractReview:async args=>{state.submitted++;state.submitArgs=args;return options.submit ? options.submit(args,state) : {state:'pending',hash:HASH,address:baseReview.predictedAddress,from:ADDRESS,receipt:null}},
    checkContractReceipt:async args=>{state.receiptChecks=(state.receiptChecks||0)+1;state.checkArgs=args;return options.checkReceipt?options.checkReceipt(args,state):{state:'pending',hash:HASH,address:baseReview.predictedAddress,from:ADDRESS,receipt:null}},
    navigator:{locks:{request:async(name,_options,action)=>{state.locks.push(name);return action(state.unavailableLock===name?null:{name})}}},
    chrome:{runtime:{getURL:path=>'chrome-extension://test/'+path},storage:{local:{
      get:async key=>key===null?Object.fromEntries([...storage].map(([k,v])=>[k,structuredClone(v)])):{[key]:structuredClone(storage.get(key))},
      set:async values=>{if(state.failCheckpointWrites&&values[KEY]?.checkpoint)throw Error('Storage quota exceeded');for(const[k,v]of Object.entries(values))storage.set(k,structuredClone(v));},
    }}},
  };
  const context=vm.createContext(globals);vm.runInContext(SOURCE,context);await vm.runInContext('init()',context);
  const click=async id=>elements.get(id).onclick();
  const unlock=async()=>{elements.get('password').value='test-password-only';await click('unlock');};
  const consent=()=>{elements.get('consent').checked=true;elements.get('consent').onchange();};
  return {elements,storage,state,context,click,unlock,consent,review:baseReview,tick:()=>tick(),setNow:value=>{now=value},dispatch:name=>listeners.get(name)?.()};
}

test('unsigned reload discards a stored review and never signs automatically', async()=>{
  const f=await fixture({job:{review:{expiresAt:999999}}});
  assert.equal(f.elements.get('confirm').disabled,true);
  assert.equal(f.elements.get('reviewDetails').hidden,true);
  assert.equal(f.state.prepared,0);assert.equal(f.state.submitted,0);assert.equal(f.state.signed,0);
  assert.equal(f.elements.get('origin').textContent,'https://ide.pocol.net');
});
test('confirmation requires unlock, an explicit fresh review and consent; expiry disables it',async()=>{
  const f=await fixture();await f.unlock();
  assert.equal(f.elements.get('prepare').disabled,false);assert.equal(f.elements.get('confirm').disabled,true);
  await f.click('prepare');assert.equal(f.state.prepared,1);assert.equal(f.elements.get('confirm').disabled,true);
  f.consent();assert.equal(f.elements.get('confirm').disabled,false);
  f.setNow(f.review.expiresAt);f.tick();assert.equal(f.elements.get('confirm').disabled,true);
  await f.click('confirm');assert.equal(f.state.submitted,0);assert.match(f.elements.get('reviewNote').textContent,/expired/);
});
test('locking while decryption is pending discards and zeroizes the recovered key',async()=>{
  let resolve;const gate=new Promise(r=>{resolve=r});const f=await fixture({importGate:gate});
  const unlocking=f.unlock();for(let i=0;i<20&&!f.state.importEntered;i++)await Promise.resolve();assert.equal(f.state.importEntered,true);await f.click('lock');
  const key=new Uint8Array(32).fill(9);resolve(key);await unlocking;
  assert.equal(key.every(v=>v===0),true);assert.equal(f.elements.get('walletAddress').textContent,'Wallet locked');assert.equal(f.elements.get('confirm').disabled,true);
});
test('pagehide during decryption prevents later key adoption, including BFCache suspension',async()=>{
  let resolve;const gate=new Promise(r=>{resolve=r});const f=await fixture({importGate:gate});
  const unlocking=f.unlock();for(let i=0;i<20&&!f.state.importEntered;i++)await Promise.resolve();assert.equal(f.state.importEntered,true);
  f.dispatch('pagehide');const key=new Uint8Array(32).fill(11);resolve(key);await unlocking;
  assert.equal(key.every(v=>v===0),true);assert.equal(vm.runInContext('secret',f.context),null);
  assert.equal(f.elements.get('walletAddress').textContent,'Wallet locked');assert.equal(f.elements.get('confirm').disabled,true);
  assert.equal(f.elements.get('password').value,'');
});
test('signed resume restores the original review despite expiry and keeps exact checkpoint',async()=>{
  const review={from:ADDRESS,to:null,predictedAddress:ADDRESS,gas:'100000',valueWei:'0',maxFeeWei:'1',maxCostWei:'1',calldataHash:HASH,dataBytes:2,expiresAt:1,requestHash:HASH,tx:{data:'0x6000'}};
  const checkpoint={version:1,review,transactions:{contract:{raw:'0x02',hash:HASH}}};
  const f=await fixture({job:{status:'pending',walletAddress:ADDRESS,checkpoint}});await f.unlock();f.consent();
  assert.equal(f.elements.get('prepare').disabled,true);assert.equal(f.elements.get('confirm').disabled,false);assert.equal(f.elements.get('cancel').disabled,true);
  await f.click('confirm');assert.equal(f.state.submitted,1);assert.equal(f.state.prepared,0);assert.equal(f.state.signed,0);
  assert.equal((await f.state.submitArgs.loadCheckpoint()).transactions.contract.raw,'0x02');
  assert.equal(f.storage.get(KEY).status,'pending');
  assert.ok(f.state.locks.includes('pocol:publish:1599846715:'+ADDRESS));
});
test('another unresolved Studio transaction blocks the same wallet',async()=>{
  const f=await fixture({otherJob:['studio:job:old',{id:'old',walletAddress:ADDRESS,status:'paused',checkpoint:{transactions:{upload:{raw:'0x02'}}}}]});await f.unlock();await f.click('prepare');
  assert.equal(f.state.prepared,0);assert.equal(f.elements.get('earlierJob').hidden,false);assert.match(f.elements.get('status').textContent,/unresolved signed/);
  assert.equal(f.elements.get('earlierJob').children[0].href,'chrome-extension://test/publisher.html#old');
});
test('terminal status from another tab is not overwritten by a stale confirmation',async()=>{
  const f=await fixture();await f.unlock();await f.click('prepare');f.consent();
  f.storage.set(KEY,{...f.storage.get(KEY),status:'complete',result:{state:'included',hash:HASH}});
  await f.click('confirm');assert.equal(f.storage.get(KEY).status,'complete');assert.equal(f.state.submitted,0);
});
test('wallet lock zeroizes private bytes and invalidates an unsigned review',async()=>{
  const f=await fixture();await f.unlock();await f.click('prepare');f.consent();await f.click('lock');
  assert.equal(f.state.generatedKeys[0].every(v=>v===0),true);assert.equal(f.elements.get('reviewDetails').hidden,true);assert.equal(f.elements.get('confirm').disabled,true);
});

test('failed signed checkpoint storage is not advertised as saved and blocks further actions',async()=>{
  const f=await fixture({submit:async(args,state)=>{
    await args.saveCheckpoint({version:1,review:args.review,transactions:{contract:{raw:'0x02',hash:HASH}}});
    state.broadcasts=(state.broadcasts||0)+1;
    return{state:'pending',hash:HASH};
  }});
  await f.unlock();await f.click('prepare');f.consent();f.state.failCheckpointWrites=true;
  await f.click('confirm');
  assert.equal(f.state.broadcasts||0,0);
  assert.equal(f.storage.get(KEY).checkpoint,undefined);
  assert.equal(vm.runInContext('job.checkpoint',f.context),undefined);
  assert.equal(f.elements.get('fatal').hidden,false);
  assert.equal(f.elements.get('prepare').disabled,true);
  assert.equal(f.elements.get('confirm').disabled,true);
  assert.doesNotMatch(f.elements.get('status').textContent,/remains saved/);
});

test('an error after durable signing persists the known hash as a pending result',async()=>{
  const f=await fixture({submit:async args=>{
    await args.saveCheckpoint({version:1,review:args.review,transactions:{contract:{raw:'0x02',hash:HASH}}});
    throw Error('Connection interrupted');
  }});
  await f.unlock();await f.click('prepare');f.consent();await f.click('confirm');
  assert.equal(f.storage.get(KEY).status,'pending');
  assert.equal(f.storage.get(KEY).result.hash,HASH);
  assert.equal(f.storage.get(KEY).result.state,'pending');
  assert.equal(f.storage.get(KEY).result.from,ADDRESS);
  assert.match(f.elements.get('status').textContent,/remains saved/);
});

function savedPendingJob() {
  const review={from:ADDRESS,to:null,predictedAddress:ADDRESS,gas:'100000',valueWei:'0',maxFeeWei:'1',maxCostWei:'1',calldataHash:HASH,dataBytes:2,expiresAt:1,requestHash:HASH,tx:{data:'0x6000'}};
  return {status:'pending',walletAddress:ADDRESS,checkpoint:{version:1,review,transactions:{contract:{raw:'0x02',hash:HASH,receipt:null}}}};
}
test('automatic pending receipt completion is read-only and needs no unlocked wallet',async()=>{
  const receipt={transactionHash:HASH,status:'0x1',blockHash:'0x'+'cd'.repeat(32),blockNumber:'0x4'};
  const f=await fixture({job:savedPendingJob(),checkReceipt:async()=>({state:'included',hash:HASH,address:ADDRESS,from:ADDRESS,receipt})});
  await f.tick();
  assert.equal(f.state.receiptChecks,1);
  assert.equal(f.state.prepared,0);assert.equal(f.state.submitted,0);assert.equal(f.state.signed,0);
  assert.equal(f.elements.get('walletAddress').textContent,'Wallet locked');
  assert.equal(f.storage.get(KEY).status,'complete');
  assert.deepEqual(f.storage.get(KEY).checkpoint.transactions.contract.receipt,receipt);
  assert.equal(f.storage.get(KEY).checkpoint.transactions.contract.raw,'0x02');
  assert.equal(f.storage.get(KEY).result.state,'included');
  assert.equal(Object.hasOwn(f.state.checkArgs,'sign'),false);
});
test('receipt checks leave pending state intact on transient errors and skip held locks',async()=>{
  const f=await fixture({job:savedPendingJob(),checkReceipt:async()=>{throw Error('Network timeout')}});
  await f.tick();assert.equal(f.storage.get(KEY).status,'pending');assert.equal(f.storage.get(KEY).checkpoint.transactions.contract.receipt,null);
  f.state.unavailableLock='pocol:job:'+JOB_ID;f.setNow(5000);await f.tick();assert.equal(f.state.receiptChecks,1);
  f.state.unavailableLock=null;vm.runInContext('busy=true',f.context);f.setNow(9000);await f.tick();assert.equal(f.state.receiptChecks,1);
});
test('a prior canonical IDE receipt is reconciled before checking account availability',async()=>{
  const previous={...savedPendingJob(),id:'previous',origin:'https://ide.pocol.net',request:{schema:1,action:'deploy',abi:[],args:[],bytecode:'0x6000',valueWei:'0'}};
  const receipt={transactionHash:HASH,status:'0x1',blockHash:'0x'+'cd'.repeat(32),blockNumber:'0x4'};
  const f=await fixture({otherJob:['ide:job:previous',previous],checkReceipt:async()=>({state:'included',hash:HASH,address:ADDRESS,from:ADDRESS,receipt})});
  await f.unlock();await f.click('prepare');
  assert.equal(f.state.receiptChecks,1);assert.equal(f.state.prepared,1);assert.equal(f.state.submitted,0);assert.equal(f.state.signed,0);
  assert.equal(f.storage.get('ide:job:previous').status,'complete');
  assert.deepEqual(f.storage.get('ide:job:previous').checkpoint.transactions.contract.receipt,receipt);
  assert.ok(f.state.locks.includes('pocol:job:previous'));
});
