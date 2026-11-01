/** LP3 reference computation only: supplied facts, no state proof, signing or boot. */
import { createHash } from 'node:crypto';
export const MAX256=(1n<<256n)-1n, NONCE_SPACE=1n<<64n;
export function floorDiv(n:bigint,d:bigint):bigint { if(d<=0n)throw Error('divisor');return n>=0n?n/d:-((-n+d-1n)/d); }
export function bits(n:bigint):number {return n===0n?0:(n<0n?-n:n).toString(2).length;}
function bounded(n:bigint,width:number,name:string):void {if(n<0n||n>=(1n<<BigInt(width)))throw Error(name);}
export interface AsertTrace {target:bigint;dt:bigint;e:bigint;s:bigint;f:bigint;F:bigint|null;early:'high'|'low'|null;maxBits:number}
export function asert(target:bigint,g:bigint,T:bigint,tau:bigint,pTs:bigint,pH:bigint):AsertTrace {
 bounded(target,256,'target');if(target===0n||T<1n||tau<1n)throw Error('parameters');
 bounded(g,64,'g_ts');bounded(T,32,'T_blk');bounded(tau,32,'tau');bounded(pTs,64,'p.ts');bounded(pH,64,'p.h');
 const dt=pTs-g-T*pH;if(dt<=-(1n<<97n)||dt>=(1n<<97n))throw Error('dt');
 const num=dt*65536n,e=floorDiv(num,tau),s=floorDiv(e,65536n),f=e-65536n*s;
 let maxBits=Math.max(bits(num),bits(e));
 if(s>=256n||s<=-257n)return {target:s>=256n?MAX256:1n,dt,e,s,f,F:null,early:s>=256n?'high':'low',maxBits};
 const poly=195766423245049n*f+971821376n*f*f+5127n*f*f*f+(1n<<47n),F=65536n+(poly>>48n),X=target*F,k=s-16n,Y=k>=0n?X<<k:X>>(-k);
 maxBits=Math.max(maxBits,bits(poly),bits(X),bits(Y));if(maxBits>512)throw Error('intermediateWidth');
 return {target:Y<1n?1n:Y>MAX256?MAX256:Y,dt,e,s,f,F,early:null,maxBits};
}
export function window(parent:bigint,attempt:bigint,duration:bigint):{lower:bigint;upper:bigint} {
 bounded(parent,64,'parent');bounded(attempt,32,'attempt');bounded(duration,32,'duration');if(duration===0n)throw Error('duration');
 const lower=parent+attempt*duration+1n,upper=parent+(attempt+1n)*duration;if(upper>=(1n<<63n))throw Error('timestampLimit');return {lower,upper};
}
export function fallback(parent:bigint,attempt:bigint,duration:bigint,wait:bigint):bigint {
 bounded(wait,32,'wait');const w=window(parent,attempt,duration),ts=w.lower+(attempt===0n?wait:0n);if(ts>w.upper)throw Error('window');return ts;
}
export function deadlineMs(parent:bigint,duration:bigint,wait:bigint,receivedMs:bigint,rel:bigint):bigint {
 bounded(receivedMs,128,'receivedMs');bounded(rel,64,'rel');const a=receivedMs+rel*1000n,b=fallback(parent,0n,duration,wait)*1000n;if(a>=(1n<<128n))throw Error('milliseconds');return a>b?a:b;
}
export function metrics(delta:bigint,phi:bigint,t:bigint,rel:bigint,wait:bigint):{sigma:bigint;xh:bigint;xw:bigint} {
 for(const n of [delta,phi,t,rel,wait])bounded(n,64,'metric');const max=(a:bigint,b:bigint)=>a>b?a:b;
 return {sigma:max(delta,phi+1n)+t,xh:max(delta+t,phi+1n)+delta+2n*t,xw:max(phi+1n+wait,delta+t+rel)};
}
export function arrivesByDeadline(arrival:bigint,deadline:bigint):boolean {return arrival<=deadline;}
export function isFuture(ts:bigint,clock:bigint,phi:bigint):boolean {bounded(ts,64,'ts');bounded(clock,64,'clock');bounded(phi,64,'phi');return ts>clock+phi;}
export const sha256=(b:Uint8Array):Uint8Array=>new Uint8Array(createHash('sha256').update(b).digest());
export const hex=(b:Uint8Array):string=>Buffer.from(b).toString('hex');
export function unhex(h:string):Uint8Array {if(!/^(?:[0-9a-f]{2})*$/.test(h))throw Error('hex');return new Uint8Array(Buffer.from(h,'hex'));}
export function be(n:bigint,size:number):Uint8Array {bounded(n,size*8,'width');const b=new Uint8Array(size);for(let i=size-1;i>=0;i--){b[i]=Number(n&255n);n>>=8n;}return b;}
export function concat(...items:Uint8Array[]):Uint8Array {const out=new Uint8Array(items.reduce((n,x)=>n+x.length,0));let p=0;for(const b of items){out.set(b,p);p+=b.length;}return out;}
const utf=(s:string)=>new TextEncoder().encode(s);
function address(id:string):void {if(!/^[0-9a-f]{40}$/.test(id))throw Error('address');const n=BigInt('0x'+id);if(n===0n||n===(1n<<160n)-2n||(n>=0xc0c001n&&n<=0xc0c0ffn))throw Error('reserved');}
export interface Member {id:string;tomb:boolean;last:bigint;grace:bigint}
export function membership(parentHeight:bigint,h:bigint,act:Member[],w:bigint,max:bigint):string[] {
 bounded(h,64,'height');bounded(parentHeight,64,'parent');bounded(w,32,'window');bounded(max,16,'memberBound');
 if(h===0n||parentHeight!==h-1n)throw Error('parentHeight');if(max===0n||BigInt(act.length)>max)throw Error('memberBound');
 const seen=new Set<string>();for(const m of act){if(seen.has(m.id))throw Error('duplicateMember');seen.add(m.id);address(m.id);bounded(m.last,64,'last');bounded(m.grace,64,'grace');}
 const live=act.filter(m=>!m.tomb),active=live.filter(m=>m.last>=h-w||m.grace>=h-w);return(active.length?active:live).map(m=>m.id).sort();
}
export function nonceSeed(seal:string,h:bigint,attempt:bigint):Uint8Array {if(!/^[0-9a-f]{64}$/.test(seal))throw Error('seal');return sha256(concat(utf('PoColNonce'),unhex(seal),be(h,8),be(attempt,4)));}
export function nonceMax(target:bigint,mode:bigint,c:bigint):bigint {
 bounded(target,256,'target');bounded(c,8,'c');if(!target||!c||(mode!==0n&&mode!==1n))throw Error('nonceParams');const n=c*((1n<<256n)/(target+1n));return mode===1n||n>NONCE_SPACE?NONCE_SPACE:n;
}
export interface NonceRange {id:string;ticket:string;start:bigint;endExclusive:bigint}
export function assignment(ids:string[],seal:string,h:bigint,attempt:bigint,target:bigint,mode:bigint,c:bigint):{seed:string;maximum:bigint;ranges:NonceRange[]} {
 if(!ids.length)throw Error('emptyMembership');if(ids.length>65535||new Set(ids).size!==ids.length)throw Error('members');ids.forEach(address);
 const seed=nonceSeed(seal,h,attempt),maximum=nonceMax(target,mode,c),tickets=ids.map(id=>({id,ticket:hex(sha256(concat(seed,unhex(id))))})).sort((a,b)=>a.ticket<b.ticket?-1:a.ticket>b.ticket?1:0);
 for(let i=1;i<tickets.length;i++)if(tickets[i].ticket===tickets[i-1].ticket)throw Error('ticketCollision');
 return {seed:hex(seed),maximum,ranges:tickets.map((m,i)=>({...m,start:BigInt(i)*maximum/BigInt(ids.length),endExclusive:BigInt(i+1)*maximum/BigInt(ids.length)}))};
}
export function owner(ranges:NonceRange[],nonce:bigint):string|null {bounded(nonce,64,'nonce');return ranges.find(r=>r.start<=nonce&&nonce<r.endExclusive)?.id??null;}
export interface ShareCount {height:bigint;id:string;count:bigint}
export function weights(ids:string[],h:bigint,ww:bigint,history:ShareCount[]):{id:string;weight:bigint}[] {
 bounded(h,64,'height');bounded(ww,16,'window');if(!ids.length)throw Error('emptyMembership');if(ids.length>65535||new Set(ids).size!==ids.length)throw Error('members');ids.forEach(address);
 const seen=new Set<string>();for(const r of history){if(!/^[0-9a-f]{40}$/.test(r.id))throw Error('address');bounded(r.height,64,'height');bounded(r.count,64,'count');const key=r.height+':'+r.id;if(seen.has(key))throw Error('duplicateCount');seen.add(key);}
 const result=[...ids].sort().map(id=>({id,weight:history.filter(r=>r.id===id&&r.height>=h-1n-ww&&r.height<=h-2n).reduce((n,r)=>n+r.count,0n)}));
 if(result.every(r=>r.weight===0n))result.forEach(r=>r.weight=1n);for(const r of result)bounded(r.weight,128,'weight');return result;
}
export function sample(digest:string,total:bigint):bigint|null {bounded(total,128,'weight');if(!total||!/^[0-9a-f]{64}$/.test(digest))throw Error('sample');const x=BigInt('0x'+digest);return x<((1n<<256n)/total)*total?x%total:null;}
function weightTotal(w:{id:string;weight:bigint}[]):bigint {let total=0n;for(let i=0;i<w.length;i++){address(w[i].id);bounded(w[i].weight,128,'weight');if(i&&w[i-1].id>=w[i].id)throw Error('weightOrder');total+=w[i].weight;}bounded(total,128,'total');if(!total)throw Error('zeroWeight');return total;}
export function choose(w:{id:string;weight:bigint}[],index:bigint):string {const total=weightTotal(w);if(index<0n||index>=total)throw Error('index');let sum=0n;for(const r of w){sum+=r.weight;if(sum>index)return r.id;}throw Error('index');}
export function draw(w:{id:string;weight:bigint}[],seal:string,h:bigint):{id:string;counter:bigint} {
 if(!/^[0-9a-f]{64}$/.test(seal))throw Error('seal');const total=weightTotal(w);
 const result=retryIndex(total,0n,k=>hex(sha256(concat(utf('PoColProposer'),unhex(seal),be(h,8),be(0n,4),be(k,4)))));return{id:choose(w,result.index),counter:result.counter};
}
/** Generic deterministic sampler retry primitive; digest injection tests error paths only. */
export function retryIndex(total:bigint,start:bigint,digest:(counter:bigint)=>string):{index:bigint;counter:bigint} {
 bounded(start,32,'counter');for(let k=start;k<=0xffffffffn;k++){const index=sample(digest(k),total);if(index!==null)return{index,counter:k};}throw Error('counterExhausted');
}
/** SV arithmetic only. Tomb/account inputs are supplied facts, not authenticated state. */
export function shareRewards(R:bigint,counts:{id:string;count:bigint;tomb:boolean;accountExists:boolean}[],kappa:bigint,m:bigint):{payments:{id:string;amount:bigint;route:string}[];amount:bigint;baseBurn:bigint;roundBurn:bigint;paid:bigint;penaltyBurn:bigint;payBurn:bigint} {
 bounded(R,256,'R');bounded(kappa,8,'kappa');bounded(m,16,'m');if(!kappa||!m||counts.length>257)throw Error('parameters');
 const seen=new Set<string>();let C=0n;for(const r of counts){if(!/^[0-9a-f]{40}$/.test(r.id)||seen.has(r.id)||r.count<1n||r.count>257n)throw Error('counts');seen.add(r.id);C+=r.count;}if(C>257n)throw Error('counts');
 const Q=C>kappa*m?C:kappa*m,amount=R*C/Q;let paid=0n,penaltyBurn=0n,payBurn=0n,sum=0n;
 const payments=counts.map(r=>{const amount=R*r.count/Q;sum+=amount;const route=r.tomb?'penaltyBurn':!r.accountExists?'payBurn':'paid';if(route==='penaltyBurn')penaltyBurn+=amount;else if(route==='payBurn')payBurn+=amount;else paid+=amount;return{id:r.id,amount,route};});
 return{payments,amount,baseBurn:R-amount,roundBurn:amount-sum,paid,penaltyBurn,payBurn};
}
export function splitPot(pot:bigint,alpha:bigint,gamma:bigint):{winner:bigint;proposer:bigint;remainder:bigint} {bounded(pot,256,'pot');bounded(alpha,16,'alpha');bounded(gamma,16,'gamma');if(alpha+gamma>10000n)throw Error('parts');const winner=pot*alpha/10000n,proposer=pot*gamma/10000n;return{winner,proposer,remainder:pot-winner-proposer};}

/** Approved LP3 core CF mapping. Supplied parent facts are not execution certificates. */
export interface FinalityParent {closing:boolean;closeH:bigint;cursorDone:boolean;isFinal:boolean}
const cfU64=(v:bigint)=>{if(v<0n||v>=(1n<<64n))throw Error('width');return v;};
const cfU32=(v:bigint)=>{if(v<0n||v>=(1n<<32n))throw Error('width');return v;};
export function closing(first:bigint|null,h:bigint,ts:bigint,hClose:bigint,tClose:bigint):bigint|null {
 [h,ts,hClose,tClose].forEach(cfU64);if(first!==null)return cfU64(first);return h>=hClose||ts>=tClose?h:null;
}
export function cursorBound(closeH:bigint,records:bigint,ret:bigint):bigint {
 cfU64(closeH);cfU64(records);cfU32(ret);if(ret===0n)throw Error('ret');const n=(records+ret-1n)/ret;
 const result=closeH+(n===0n?0n:n-1n);if(result>=(1n<<64n))throw Error('overflow');return result;
}
export function gate2(h:bigint,ts:bigint,hEnd:bigint,tEnd:bigint,p2:bigint,tBlk:bigint):boolean {
 [h,ts,hEnd,tEnd].forEach(cfU64);[p2,tBlk].forEach(cfU32);const a=hEnd-p2,b=tEnd-p2*tBlk;
 if(a<0n||b<0n)throw Error('threshold');return h>=a||ts>=b;
}
export function finalPredicate(p:FinalityParent,h:bigint,ts:bigint,d:bigint,hEnd:bigint,tEnd:bigint):boolean {
 [p.closeH,h,ts,hEnd,tEnd].forEach(cfU64);cfU32(d);
 return p.closing&&p.cursorDone&&h>=p.closeH+d&&(h>=hEnd||ts>=tEnd);
}
export function childAllowed(parentFinal:boolean,h:bigint,hEnd:bigint):boolean {cfU64(h);cfU64(hEnd);return !parentFinal&&h<=hEnd;}
