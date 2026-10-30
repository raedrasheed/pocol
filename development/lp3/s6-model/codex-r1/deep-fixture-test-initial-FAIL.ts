import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as m from './model.ts';
import * as g from './genesis.ts';
const json=(path:string):any=>JSON.parse(readFileSync(new URL(path,import.meta.url),'utf8'));
const id=(n:number)=>n.toString(16).padStart(40,'0');
test('AS1-AS19 targets, traces and bounded intermediates',()=>{
 const T=1n<<240n;
 const cases:[bigint,bigint,bigint,bigint,bigint,bigint][]=[
  [0n,0n,10n,600n,T,T],[700n,10n,10n,600n,T,1n<<241n],[400n,100n,10n,600n,T,1n<<239n],
  [400n,10n,10n,600n,T,92674n<<224n],[700n,100n,10n,600n,T,46337n<<224n],
  [11n,1n,10n,65536n,T,65537n<<224n],[9n,1n,10n,65536n,T,131071n<<223n],
  [65545n,1n,10n,65536n,T,131071n<<224n],[34463n,10000n,10n,65536n,T,131071n<<222n],
  [9n,1n,10n,65537n,T,131071n<<223n],[10300n,10n,10n,600n,T,m.MAX256],[55400n,20000n,10n,600n,T,1n],
  [0n,0n,10n,600n,m.MAX256,m.MAX256],[1n<<62n,7n,60n,3600n,T,m.MAX256],
  [0n,(1n<<64n)-1n,(1n<<32n)-1n,1n,T,1n],[153000n,0n,10n,600n,1n,1n<<255n],
  [153600n,0n,10n,600n,1n,m.MAX256],[0n,15420n,10n,600n,m.MAX256,1n],[0n,15360n,10n,600n,m.MAX256,1n]
 ];
 cases.forEach(([ts,h,t,tau,target,expect],i)=>{const v=m.asert(target,0n,t,tau,ts,h);assert.equal(v.target,expect,'AS'+(i+1));assert.ok(v.maxBits<=512);});
 assert.equal(m.asert(T,0n,10n,65537n,9n,1n).e,-1n);
 assert.equal(m.asert(T,0n,10n,600n,400n,10n).F,92674n);
 assert.throws(()=>m.asert(T,0n,10n,0n,0n,0n));
});
test('1027 previously frozen ASERT rows',()=>{
 const rows=json('../lp1-node/fixtures/asert-oracle.json');assert.equal(rows.checks,1027);
 for(const r of rows.rows){const a=m.asert(BigInt(r.targetG),BigInt(r.gts),BigInt(r.blockTime),BigInt(r.tau),BigInt(r.ts),BigInt(r.h));assert.equal(a.target.toString(),r.target);assert.ok(a.maxBits<=512);}
});
test('timing metrics, TJ1-4 and overflow equality',()=>{
 assert.deepEqual(m.metrics(5n,10n,3n,26n,25n),{sigma:14n,xh:22n,xw:36n});
 assert.deepEqual(m.metrics(1n,2n,1n,8n,6n),{sigma:4n,xh:6n,xw:10n});
 for(const [b,expected] of [[1001n,1009n],[998n,1007n],[1030n,1038n]])assert.equal(m.deadlineMs(1000n,60n,6n,b*1000n,8n),expected*1000n);
 assert.ok(m.arrivesByDeadline(1009000n,1009000n));assert.ok(!m.arrivesByDeadline(1009001n,1009000n));
 assert.equal(m.fallback(1000n,0n,60n,6n),1007n);assert.equal(m.fallback(1000n,1n,60n,6n),1061n);
 assert.throws(()=>m.window((1n<<63n)-60n,0n,60n));assert.throws(()=>m.deadlineMs(1000n,60n,6n,(1n<<128n)-1n,8n));
 assert.equal(m.isFuture((1n<<64n)-1n,(1n<<64n)-1n,10n),false);
});
test('membership, partition completeness, owner and exact hashes',()=>{
 const act=[{id:id(3),last:6n,grace:6n,tomb:false},{id:id(2),last:6n,grace:7n,tomb:false},{id:id(1),last:7n,grace:0n,tomb:false},{id:id(4),last:9n,grace:9n,tomb:true}];
 assert.deepEqual(m.membership(9n,10n,act,3n,65535n),[id(1),id(2)]);
 assert.deepEqual(m.membership(9n,10n,act,0n,65535n),[id(1),id(2),id(3)]);
 assert.throws(()=>m.membership(9n,10n,[{id:id(0),last:0n,grace:0n,tomb:false}],3n,5n));
 const seal=Array.from({length:32},(_,i)=>i.toString(16).padStart(2,'0')).join('');
 assert.equal(m.hex(m.nonceSeed(seal,0x0102030405060708n,0x090a0b0cn)),'07f7c97972f41ec183ed19cbb9a1078780a59e221aee9eaa53d1083a75639a92');
 for(const n of [1n,2n,3n,7n,255n]){const a=m.assignment([id(1),id(2),id(3)],'07'.repeat(32),10n,0n,m.MAX256,0n,n);assert.equal(a.ranges[0].start,0n);assert.equal(a.ranges[2].endExclusive,n);for(let j=0n;j<n;j++)assert.ok(m.owner(a.ranges,j));assert.equal(m.owner(a.ranges,n),null);}
 assert.equal(m.nonceMax(1n<<192n,0n,1n),(1n<<64n)-1n);assert.equal(m.nonceMax(m.MAX256,1n,1n),1n<<64n);
});
test('weight interval, strict CDF, unbiased rejection and uniform fallback',()=>{
 const history=[{height:8n,id:id(1),count:3n},{height:9n,id:id(2),count:99n}];
 const w=m.weights([id(2),id(1)],10n,1n,history);assert.deepEqual(w,[{id:id(1),weight:3n},{id:id(2),weight:0n}]);assert.equal(m.choose(w,2n),id(1));assert.throws(()=>m.choose(w,3n));
 assert.equal(m.sample('ff'.repeat(32),3n),null);assert.equal(m.sample('00'.repeat(32),3n),0n);
 assert.deepEqual(m.weights([id(2),id(1)],10n,1n,[]),[{id:id(1),weight:1n},{id:id(2),weight:1n}]);
 assert.throws(()=>m.weights([id(1)],10n,1n,[history[0],history[0]]));
 assert.deepEqual(m.draw(w,'07'.repeat(32),10n).id,id(1));
 const counters:bigint[]=[];assert.deepEqual(m.retryIndex(3n,0n,k=>{counters.push(k);return(k===0n?'ff':'00').repeat(32);}),{index:0n,counter:1n});assert.deepEqual(counters,[0n,1n]);
 assert.throws(()=>m.retryIndex(3n,0xffffffffn,()=> 'ff'.repeat(32)),/counterExhausted/);
 assert.throws(()=>m.choose([{id:id(2),weight:1n},{id:id(1),weight:1n}],0n),/weightOrder/);
 assert.throws(()=>m.choose([{id:id(1),weight:-1n}],0n),/weight/);
});
test('Keccak K1-K3 and frozen literal GSV1 Rust/TS identity',()=>{
 const vectors=json('../../development/m1/m1-draft-0.21/vectors/v3-gsv1.json');
 for(const k of ['K1','K2','K3']){const [input,expected]=vectors.keccakVectors[k];assert.equal(m.hex(g.keccak256(m.unhex(input))),expected);}
 const literal=vectors.flatSegments.map((x:any)=>typeof x==='string'?x:x.repeat.repeat(x.count)).join('');const b=m.unhex(literal);assert.equal(b.length,341);
 const identity=g.genesisIdentity(b);assert.equal(identity.hash,'de518e30a5e333ac3ddf127b6201f1a66f80dce2b56566524df4037bcdeb2277');assert.equal(identity.bootable,false);assert.equal(identity.spec.chainId,777902n);assert.equal(identity.spec.cp.length,52);assert.equal(m.hex(g.encodeGenesis(identity.spec)),literal);
 const profile=json('../lp1-node/fixtures/profile.json');assert.equal(g.genesisIdentity(m.unhex(profile.genesis_pre_hex)).hash,profile.genesis_hash_hex);
});
test('full schema width/order/reserved failures and deep malformed framing',()=>{
 const profile=json('../lp1-node/fixtures/profile.json'),base=g.decodeGenesis(m.unhex(profile.genesis_pre_hex));
 const code=(fn:()=>unknown,c:string)=>assert.throws(fn,(e:unknown)=>e instanceof g.GenesisError&&e.code===c);
 const clone=()=>({...base,cp:[...base.cp],members:base.members.map(m=>({...m}))});
 let s=clone();s.cp[4]=2n;code(()=>g.decodeGenesis(g.encodeGenesis(s)),'gsRange');s=clone();s.members.reverse();code(()=>g.decodeGenesis(g.encodeGenesis(s)),'gsOrder');s=clone();s.members[0].id=id(0);code(()=>g.decodeGenesis(g.encodeGenesis(s)),'gsSys');s=clone();s.cp[0]=1n<<64n;code(()=>g.decodeGenesis(g.encodeGenesis(s)),'gsRange');
 let b=m.unhex('80');for(let i=0;i<1500;i++)b=g.rlpList([b]);code(()=>g.decodeGenesis(b),'gsStructure');code(()=>g.decodeGenesis(b.subarray(0,b.length-1)),'L0');
});
test('all accepted L4n/order supplements and 1500-level malformed twins',()=>{
 const v=json('../../development/m1/m1-draft-0.21/vectors/v3-gsv1.json');
 const raw=(x:any):Uint8Array=>typeof x==='string'?m.unhex(x):Array.isArray(x)?g.rlpList(x.map(raw)):'repeat'in x?m.unhex(x.repeat.repeat(x.count)):m.concat(...x.concat.map(raw));
 const at=(t:any,path:number[]):any=>path.reduce((n,i)=>n[i],t);
 const put=(t:any,path:number[],value:any):void=>{at(t,path.slice(0,-1))[path[path.length-1]]=value;};
 const code=(b:Uint8Array):string=>{try{g.decodeGenesis(b);return'ok';}catch(e){if(e instanceof g.GenesisError)return e.code;throw e;}};
 for(const row of v.negatives){const t=structuredClone(v.tree);let suffix='',header='';for(const edit of row.edits){if(edit.op==='set')put(t,edit.path,edit.value);else if(edit.op==='delete')at(t,edit.path.slice(0,-1)).splice(edit.path.at(-1),1);else if(edit.op==='swap'){const a=at(t,edit.a),b=at(t,edit.b);put(t,edit.a,b);put(t,edit.b,a);}else if(edit.op==='copy')put(t,edit.to,structuredClone(at(t,edit.from)));else if(edit.op==='appendBytes')suffix+=edit.hex;else if(edit.op==='setOuterHeader')header=edit.hex;else throw Error('fixture edit');}
  let b=raw(t);if(header)b=m.concat(m.unhex(header),b.subarray(3));b=m.concat(b,m.unhex(suffix));assert.equal(code(b),row.expect,row.id);}
 const depth=json('../../development/m1/m1-draft-0.22/vectors/c30-depth.json');
 for(const row of depth.cases){for(const variant of ['deep','shallow']){let b=m.unhex(row.inner);for(let i=0;i<row[variant].depth;i++)b=g.rlpList([b]);if(row.dropLast)b=b.subarray(0,b.length-1);if(row.suffix)b=m.concat(b,m.unhex(row.suffix));if(row.place==='gsv1'){const t=structuredClone(v.tree);put(t,row.path,m.hex(b));b=raw(t);}assert.equal(code(b),row.expect.code,row.id+' '+variant);}}
});
