// Public contract intents; signing stays in a trusted extension confirmation page.
import { Interface, Fragment, Transaction, getAddress, getCreateAddress, keccak256, toUtf8Bytes } from '../vendor/ethers.js';
import { NETWORK } from './studio-network.js';
export const IDE_JOB_PREFIX = 'ide:job:';
export const IDE_ORIGINS = Object.freeze(['https://ide.pocol.net', 'https://pocol-ide.mystic-gecko-9511.chatgpt.site']);
export const IDE_REQUEST_MAX = 262144;
const MAX_INIT = 49152, MAX_DATA = 58000, MAX_GAS = 8000000n, MAX_FEE = 200000000000n, PRIORITY = 1000000000n;
const HEX = /^0x(?:[a-f0-9]{2})*$/i, HASH = /^0x[a-f0-9]{64}$/i;
const issued = new WeakSet(), running = new Set();
const clone = value => JSON.parse(JSON.stringify(value));
const fail = (message, code = 'CONTRACT_ERROR') => { throw Object.assign(new Error(message), {code}); };
const assert = (condition, message, code) => { if (!condition) fail(message, code); };
const uint = (value, name='integer') => { assert(typeof value === 'string' && /^(?:0|[1-9][0-9]{0,77})$/.test(value), `Invalid ${name}`, 'INVALID_REQUEST'); const n=BigInt(value); assert(n<(1n<<256n),`${name} is too large`,'INVALID_REQUEST');return n; };
const qty = value => { assert(typeof value==='string' && /^0x(?:0|[1-9a-f][a-f0-9]{0,63})$/i.test(value),'Invalid RPC quantity','INVALID_RESPONSE');return BigInt(value); };
const hexQty = value => '0x'+BigInt(value).toString(16);
const addressOf = value => { try { return getAddress(value).toLowerCase(); } catch { fail('Invalid account or contract address','INVALID_REQUEST'); } };
const abort = signal => { if(signal?.aborted)fail('Operation paused. Resume the same request to check its transaction.','CONTRACT_PAUSED'); };
function jsonTree(value, depth=0, count={n:0}) {
  assert(depth<=32 && ++count.n<=12000,'Request is too deeply nested or contains too many values','INVALID_REQUEST');
  if(value===null || typeof value==='boolean' || typeof value==='string') return;
  if(typeof value==='number'){assert(Number.isSafeInteger(value),'Quote large integers in ABI arguments to preserve their exact value','INVALID_REQUEST');return;}
  assert(typeof value==='object' && (Array.isArray(value) || Object.getPrototypeOf(value)===Object.prototype || Object.getPrototypeOf(value)===null),'Use plain JSON ABI arguments','INVALID_REQUEST');
  for(const item of Object.values(value))jsonTree(item,depth+1,count);
}
export function isIdeOrigin(url) { try { return IDE_ORIGINS.includes(new URL(url).origin); } catch { return false; } }
function normalizeArgs(inputs,args) {
  assert(Array.isArray(args)&&inputs.length===args.length,'ABI argument count does not match','INVALID_REQUEST');
  const parse=(type,value)=>{
    if(type.baseType==='array') {assert(Array.isArray(value)&&(type.arrayLength===-1||value.length===type.arrayLength),'Invalid ABI array length','INVALID_REQUEST');return value.map(item=>parse(type.arrayChildren,item));}
    if(type.baseType==='tuple') {
      let tuple=value;
      if(!Array.isArray(tuple)) {
        const names=type.components.map(c=>c.name);
        assert(tuple&&typeof tuple==='object'&&names.every(name=>name&&Object.hasOwn(tuple,name))&&new Set(names).size===names.length&&Object.keys(tuple).length===names.length,'Invalid named tuple arguments','INVALID_REQUEST');
        tuple=names.map(name=>tuple[name]);
      }
      return normalizeArgs(type.components,tuple);
    }
    if(type.type==='bool'){assert(typeof value==='boolean','ABI bool values must be true or false, not text','INVALID_REQUEST');return value;}
    if(/^u?int\d+$/.test(type.type)) {
      assert((typeof value==='string'&&/^(?:-?\d+|0x[a-f0-9]+)$/i.test(value))||(typeof value==='number'&&Number.isSafeInteger(value)),'ABI integer values must be exact whole numbers','INVALID_REQUEST');
      const n=BigInt(value),bits=BigInt(type.type.replace(/\D/g,'')),unsigned=type.type.startsWith('uint');
      assert(n>=(unsigned?0n:-(1n<<(bits-1n)))&&n<=(unsigned?(1n<<bits)-1n:(1n<<(bits-1n))-1n),'ABI integer is outside its declared range','INVALID_REQUEST');return n.toString();
    }
    if(type.type==='address')return addressOf(value);
    assert(typeof value==='string',`ABI ${type.type} must be a string`,'INVALID_REQUEST');
    if(/^bytes\d*$/.test(type.type)){const width=Number(type.type.slice(5));assert(HEX.test(value)&&(!width||value.length===2+width*2),'Invalid ABI bytes length or encoding','INVALID_REQUEST');return value.toLowerCase();}
    assert(type.type==='string','Unsupported ABI argument type','INVALID_REQUEST');return value;
  };
  return inputs.map((type,index)=>parse(type,args[index]));
}
export function validateContractRequest(input) {
  assert(input && typeof input==='object' && !Array.isArray(input),'Invalid contract request','INVALID_REQUEST');
  let encoded; try { encoded=JSON.stringify(input); } catch { fail('Contract request must contain plain JSON','INVALID_REQUEST'); }
  assert(encoded && new TextEncoder().encode(encoded).length<=IDE_REQUEST_MAX,'Contract request exceeds 256 KiB','REQUEST_TOO_LARGE');
  assert(input.schema===1 && ['deploy','call'].includes(input.action),'Unsupported contract request','INVALID_REQUEST');
  assert(Array.isArray(input.abi) && input.abi.length<=512 && JSON.stringify(input.abi).length<=98304,'ABI exceeds the supported limit','INVALID_REQUEST');
  assert(Array.isArray(input.args) && JSON.stringify(input.args).length<=65536,'Invalid or oversized ABI arguments','INVALID_REQUEST');
  jsonTree(input.abi); jsonTree(input.args);
  const valueWei=uint(input.valueWei??'0','value').toString();
  const title=typeof input.title==='string'?input.title.slice(0,120):'Contract';
  let iface;try{for(const item of input.abi)Fragment.from(item);iface=new Interface(input.abi);}catch{fail('Invalid contract ABI','INVALID_REQUEST');}
  let data,to=null,method=null,bytecode=null,args;
  try {
    if(input.action==='deploy') {
      assert(input.to===undefined || input.to===null,'A deployment cannot specify a recipient','INVALID_REQUEST');
      bytecode=input.bytecode;
      assert(typeof bytecode==='string' && HEX.test(bytecode) && bytecode!=='0x' && bytecode.length<=2+MAX_INIT*2,'Invalid, linked, empty or oversized deployment bytecode','INVALID_REQUEST');
      const ctor=input.abi.find(item=>item?.type==='constructor');
      const payable=ctor?.stateMutability?ctor.stateMutability==='payable':ctor?.payable??iface.deploy.payable;
      assert(valueWei==='0' || payable,'This constructor does not accept PCOL','NON_PAYABLE');
      args=normalizeArgs(iface.deploy.inputs,input.args);
      data=bytecode+iface.encodeDeploy(args).slice(2);
      assert(data.length<=2+MAX_INIT*2,'Encoded initcode exceeds the Cancun 49,152-byte limit','INVALID_REQUEST');
      bytecode=bytecode.toLowerCase();
    } else {
      to=addressOf(input.to);assert(!/^0x0{40}$/.test(to),'The zero address is not a contract target','INVALID_REQUEST');
      assert(typeof input.method==='string' && input.method.length<=500,'Choose a contract method','INVALID_REQUEST');
      const fn=iface.getFunction(input.method);assert(fn && !['view','pure'].includes(fn.stateMutability),'Read functions do not require a signed transaction','INVALID_REQUEST');
      assert(valueWei==='0' || fn.stateMutability==='payable','This function does not accept PCOL','NON_PAYABLE');
      method=fn.format('sighash');args=normalizeArgs(fn.inputs,input.args);data=iface.encodeFunctionData(fn,args);
      assert(data.length<=2+MAX_DATA*2,'Encoded call data exceeds 58,000 bytes','INVALID_REQUEST');
    }
  } catch(error) { if(error.code && ['INVALID_REQUEST','NON_PAYABLE'].includes(error.code))throw error;fail('Contract arguments do not match the ABI: '+String(error.shortMessage||error.message).slice(0,200),'INVALID_REQUEST'); }
  const request={schema:1,action:input.action,title,abi:clone(input.abi),bytecode,args,valueWei,to,method,data:data.toLowerCase()};
  request.requestHash=keccak256(toUtf8Bytes(JSON.stringify(request)));return request;
}
export function publicContractJob(job) {
  return {jobId:job.id,status:job.status,progress:job.progress||null,error:job.error||null,result:job.result||null};
}
async function network(rpc) {
  const chain=qty(await rpc('eth_chainId',[]));const status=await rpc('pocol_status',[]);
  assert(chain===BigInt(NETWORK.chainId) && status?.chainId===NETWORK.chainId && typeof status.genesisHash==='string' && status.genesisHash.toLowerCase()===NETWORK.genesisHash,'Unexpected PoCol network identity. Signing is blocked.','WRONG_NETWORK');
}
export async function prepareContractReview({request:input,rpc,address,now=Date.now}) {
  const request=validateContractRequest(input),from=addressOf(address);await network(rpc);
  if(request.action==='call') {const code=await rpc('eth_getCode',[request.to,'latest']);assert(typeof code==='string'&&HEX.test(code)&&code!=='0x','No contract code was found at this address','NO_CONTRACT');}
  const block=await rpc('eth_getBlockByNumber',['latest',false]);const base=qty(block?.baseFeePerGas),blockLimit=qty(block?.gasLimit);
  const maxFee=base*2n+PRIORITY;assert(maxFee<=MAX_FEE,'The current network fee exceeds the extension safety cap. Try again later.','FEE_CAP');
  const nonce=qty(await rpc('eth_getTransactionCount',[from,'pending']));
  assert(nonce<=BigInt(Number.MAX_SAFE_INTEGER),'Account nonce exceeds the supported signing range','INVALID_RESPONSE');
  const call={from,data:request.data,value:hexQty(request.valueWei)};if(request.to)call.to=request.to;
  const estimate=qty(await rpc('eth_estimateGas',[call]));const cap=blockLimit<MAX_GAS?blockLimit:MAX_GAS;
  assert(estimate>0n&&estimate<=cap,'Transaction exceeds the PoCol gas limit','GAS_LIMIT');
  const wanted=(estimate*120n+99n)/100n+1000n,gas=wanted<cap?wanted:cap,maxFeeWei=gas*maxFee,maxCostWei=maxFeeWei+BigInt(request.valueWei);
  assert(qty(await rpc('eth_getBalance',[from,'pending']))>=maxCostWei,'Insufficient PCOL for the value and maximum network fee','INSUFFICIENT_FUNDS');
  const tx=Object.freeze({chainId:String(NETWORK.chainId),nonce:nonce.toString(),maxPriority:PRIORITY.toString(),maxFee:maxFee.toString(),gas:gas.toString(),to:request.to,value:request.valueWei,data:request.data});
  const review=Object.freeze({requestHash:request.requestHash,action:request.action,title:request.title,method:request.method,from,to:request.to,predictedAddress:request.to?null:getCreateAddress({from,nonce}).toLowerCase(),valueWei:request.valueWei,gas:gas.toString(),estimate:estimate.toString(),maxFeeWei:maxFeeWei.toString(),maxCostWei:maxCostWei.toString(),calldataHash:keccak256(request.data),bytecodeHash:request.bytecode?keccak256(request.bytecode):null,dataBytes:(request.data.length-2)/2,createdAt:now(),expiresAt:now()+120000,tx});
  issued.add(review);return review;
}
function verifySigned(signed, tx, from) {
  assert(signed && typeof signed.raw==='string'&&HEX.test(signed.raw)&&signed.raw.length<=120002,'Signer returned invalid or oversized transaction','INVALID_SIGNATURE');
  let parsed;try{parsed=Transaction.from(signed.raw);}catch{fail('Signer returned an invalid transaction','INVALID_SIGNATURE');}
  assert(parsed.type===2 && parsed.signature && parsed.from?.toLowerCase()===from,'Signature does not match this wallet','INVALID_SIGNATURE');
  assert(parsed.chainId===BigInt(tx.chainId)&&parsed.nonce===Number(tx.nonce)&&parsed.gasLimit===BigInt(tx.gas)&&parsed.maxFeePerGas===BigInt(tx.maxFee)&&parsed.maxPriorityFeePerGas===BigInt(tx.maxPriority)&&parsed.value===BigInt(tx.value)&&parsed.data.toLowerCase()===tx.data&&((parsed.to?.toLowerCase()||null)===tx.to)&&parsed.accessList?.length===0,'Signer changed the reviewed transaction','INVALID_SIGNATURE');
  const hash=keccak256(signed.raw);assert(!signed.hash||signed.hash.toLowerCase()===hash,'Signed transaction hash mismatch','INVALID_SIGNATURE');return {raw:signed.raw.toLowerCase(),hash};
}
function validateSavedReview(review,request,from) {
  assert(review&&review.requestHash===request.requestHash&&review.from===from,'Saved request or signing wallet does not match','CHECKPOINT_MISMATCH');
  const tx=review.tx;
  assert(tx&&tx.chainId===String(NETWORK.chainId)&&tx.to===request.to&&tx.data===request.data&&tx.value===request.valueWei,'Saved transaction does not match the public request','CHECKPOINT_MISMATCH');
  const gas=uint(tx.gas),fee=uint(tx.maxFee),tip=uint(tx.maxPriority);uint(tx.nonce);
  assert(gas>0n&&gas<=MAX_GAS&&fee>0n&&fee<=MAX_FEE&&tip<=fee&&uint(review.maxFeeWei)===gas*fee&&uint(review.maxCostWei)===gas*fee+BigInt(tx.value),'Saved fee review is invalid','CHECKPOINT_MISMATCH');
  assert(review.calldataHash===keccak256(tx.data),'Saved calldata hash mismatch','CHECKPOINT_MISMATCH');
  const predicted=tx.to?null:getCreateAddress({from,nonce:BigInt(tx.nonce)}).toLowerCase();assert(review.predictedAddress===predicted,'Saved deployment address mismatch','CHECKPOINT_MISMATCH');
}
async function canonicalReceipt(rpc,hash) {
  const receipt=await rpc('eth_getTransactionReceipt',[hash]);if(!receipt)return null;
  assert(receipt.transactionHash?.toLowerCase()===hash&&['0x0','0x1'].includes(receipt.status)&&HASH.test(receipt.blockHash||''),'Invalid transaction receipt','INVALID_RESPONSE');qty(receipt.blockNumber);
  const block=await rpc('eth_getBlockByNumber',[receipt.blockNumber,false]);return typeof block?.hash==='string'&&block.hash.toLowerCase()===receipt.blockHash.toLowerCase()?receipt:null;
}
export async function checkContractReceipt({request:input,checkpoint,rpc}) {
  const request=validateContractRequest(input);
  assert(checkpoint?.version===1&&checkpoint.requestHash===request.requestHash,'Saved transaction belongs to another request','CHECKPOINT_MISMATCH');
  const review=checkpoint.review,from=addressOf(review?.from);validateSavedReview(review,request,from);
  const record=checkpoint.transactions?.contract;assert(record&&JSON.stringify(record.tx)===JSON.stringify(review.tx),'Signed checkpoint is incomplete or changed','CHECKPOINT_MISMATCH');
  verifySigned(record,record.tx,from);await network(rpc);
  const receipt=await canonicalReceipt(rpc,record.hash);
  if(receipt?.status==='0x1'&&review.predictedAddress)assert(receipt.contractAddress?.toLowerCase()===review.predictedAddress,'Deployment address differs from the signed transaction','RECEIPT_MISMATCH');
  return {state:!receipt?'pending':receipt.status==='0x1'?'included':'reverted',hash:record.hash,address:receipt?.status==='0x0'?null:review.predictedAddress,from,receipt};
}
export async function submitContractReview({request:input,review:givenReview,rpc,address,sign,loadCheckpoint,saveCheckpoint,signal,now=Date.now}) {
  assert(typeof loadCheckpoint==='function'&&typeof saveCheckpoint==='function','Durable transaction storage is required','CHECKPOINT_REQUIRED');
  const request=validateContractRequest(input),from=addressOf(address),key=from+':'+request.requestHash;
  assert(!running.has(key),'This contract request is already active','REQUEST_BUSY');running.add(key);
  try {
    abort(signal);await network(rpc);abort(signal);
    let checkpoint=await loadCheckpoint(),review=givenReview,record;
    if(checkpoint) {
      assert(checkpoint.version===1&&checkpoint.requestHash===request.requestHash,'Saved transaction belongs to another request','CHECKPOINT_MISMATCH');
      review=checkpoint.review;validateSavedReview(review,request,from);record=checkpoint.transactions?.contract;
      assert(record,'Signed checkpoint is incomplete; do not create a replacement transaction','CHECKPOINT_MISMATCH');
      assert(JSON.stringify(record.tx)===JSON.stringify(review.tx),'Saved transaction fields changed','CHECKPOINT_MISMATCH');verifySigned(record,record.tx,from);
    } else {
      assert(issued.has(review),'Prepare a fresh fee review in this tab first','INVALID_REVIEW');validateSavedReview(review,request,from);
      assert(now()<=review.expiresAt,'Fee review expired. Prepare a new review.','REVIEW_EXPIRED');
      const nonce=qty(await rpc('eth_getTransactionCount',[from,'pending']));assert(nonce===BigInt(review.tx.nonce),'Account nonce changed. Prepare a new review.','NONCE_CHANGED');
      assert(qty(await rpc('eth_getBalance',[from,'pending']))>=BigInt(review.maxCostWei),'Insufficient PCOL for the reviewed maximum cost','INSUFFICIENT_FUNDS');
      const block=await rpc('eth_getBlockByNumber',['latest',false]);assert(qty(block.baseFeePerGas)+BigInt(review.tx.maxPriority)<=BigInt(review.tx.maxFee),'Network fees changed. Prepare a new review.','FEE_CHANGED');
      await network(rpc);abort(signal);assert(now()<=review.expiresAt,'Fee review expired. Prepare a new review.','REVIEW_EXPIRED');
      const signed=verifySigned(await sign(clone(review.tx)),review.tx,from);
      record={...signed,tx:clone(review.tx),receipt:null};checkpoint={version:1,requestHash:request.requestHash,review:clone(review),transactions:{contract:record}};
      // No broadcast can happen until this exact signed envelope is durable.
      await saveCheckpoint(clone(checkpoint));
    }
    await network(rpc);abort(signal);
    let receipt=await canonicalReceipt(rpc,record.hash);
    if(!receipt) {
      abort(signal);
      try { const hash=await rpc('eth_sendRawTransaction',[record.raw]);assert(typeof hash==='string'&&hash.toLowerCase()===record.hash,'Gateway returned a different transaction hash','SUBMISSION_UNKNOWN'); }
      catch { try {receipt=await canonicalReceipt(rpc,record.hash);}catch{};if(!receipt)return {state:'pending',hash:record.hash,address:review.predictedAddress,from,receipt:null}; }
      if(!receipt) {try{receipt=await canonicalReceipt(rpc,record.hash);}catch{}}
    }
    if(!receipt)return {state:'pending',hash:record.hash,address:review.predictedAddress,from,receipt:null};
    if(receipt.status==='0x1'&&review.predictedAddress)assert(receipt.contractAddress?.toLowerCase()===review.predictedAddress,'Deployment address differs from the signed transaction','RECEIPT_MISMATCH');
    record.receipt=receipt;await saveCheckpoint(clone(checkpoint));
    return {state:receipt.status==='0x1'?'included':'reverted',hash:record.hash,address:receipt.status==='0x1'?review.predictedAddress:null,from,receipt};
  } finally {running.delete(key);}
}
