/** Review adapter; lossless integer JSON, no oracle arithmetic. */
import {createInterface} from 'node:readline';
import * as m from './model.ts';
import * as g from './genesis.ts';
function parseIntegers(line:string):any {
 let s='';for(let i=0;i<line.length;){if(line[i]==='"'){const start=i++;while(i<line.length){if(line[i]==='\\'){i+=2;continue;}if(line[i++]==='"')break;}s+=line.slice(start,i);}
 else if(line[i]==='-'||/[0-9]/.test(line[i])){const start=i++;while(i<line.length&&/[0-9]/.test(line[i]))i++;s+='"'+line.slice(start,i)+'"';}else s+=line[i++];}return JSON.parse(s);
}
function run(v:any):unknown {
 if(v.type==='asert')return m.asert(BigInt(v.target),BigInt(v.g),BigInt(v.T),BigInt(v.tau),BigInt(v.ts),BigInt(v.h));
 if(v.type==='genesis'){try{const b=m.unhex(v.hex),identity=g.genesisIdentity(b);return{encoded:m.hex(g.encodeGenesis(identity.spec)),hash:identity.hash,bootable:identity.bootable};}catch(e){return{error:e instanceof g.GenesisError?e.code:String(e)}}}
 if(v.type==='timing'){const p=BigInt(v.parent),a=BigInt(v.attempt),d=BigInt(v.duration);return{window:m.window(p,a,d),fallback:m.fallback(p,a,d,BigInt(v.wait))};}
 try {
  const h=BigInt(v.height),target=BigInt('0x'+v.target),ids=m.membership(BigInt(v.parentHeight),h,v.act.map((r:any[])=>({id:r[0],tomb:r[1],last:BigInt(r[2]),grace:BigInt(r[3])})),BigInt(v.wAct),BigInt(v.mMax));
  const maximum=m.nonceMax(target,BigInt(v.mode),BigInt(v.c)),seed=m.nonceSeed(v.seal,h,BigInt(v.attempt)),a=ids.length?m.assignment(ids,v.seal,h,BigInt(v.attempt),target,BigInt(v.mode),BigInt(v.c)):null;
  const w=ids.length?m.weights(ids,h,BigInt(v.ww),v.history.map((r:any[])=>({height:BigInt(r[0]),id:r[1],count:BigInt(r[2])}))):null;
  const draw=w?m.draw(w,v.seal,h):null;
  return{members:ids,seed:m.hex(seed),nonceMax:maximum.toString(),ranges:a?.ranges.map(r=>[r.id,r.ticket,r.start.toString(),r.endExclusive.toString()])??[],
   owners:v.queries.map((n:string)=>a?m.owner(a.ranges,BigInt(n)):null),weights:w?.map(r=>[r.id,r.weight.toString()])??null,
   choices:v.indices.map((n:string)=>{try{return w?m.choose(w,BigInt(n)):null;}catch{return null;}}),draw:draw?[draw.id,Number(draw.counter)]:null,
   samples:v.samples.map(([w,d]:[string,string])=>m.sample(d,BigInt(w))?.toString()??null),
   digests:[0n,1n,0xffffffffn].map(k=>m.hex(m.sha256(m.concat(new TextEncoder().encode('PoColProposer'),m.unhex(v.seal),m.be(h,8),m.be(0n,4),m.be(k,4))))) };
 }catch(e){const name=e instanceof Error?e.message:String(e),map:Record<string,string>={parentHeight:'ParentHeight',memberBound:'MemberBound',duplicateMember:'DuplicateMember',reserved:'ReservedMember',nonceParams:'ZeroTargetOrC'};return{error:map[name]??name};}
}
for await (const line of createInterface({input:process.stdin,crlfDelay:Infinity})){try{console.log(JSON.stringify(run(parseIntegers(line)),(_,v:unknown)=>typeof v==='bigint'?v.toString():v));}catch(e){console.log(JSON.stringify({error:e instanceof Error?e.message:String(e)}));}}
