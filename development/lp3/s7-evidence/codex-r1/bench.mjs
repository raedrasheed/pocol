import {asert,MAX256} from '../../../../prototypes/lp3-model/model.ts';
import assert from 'node:assert/strict';
const t=1n<<240n,cases=[['AS1',[t,0n,10n,600n,0n,0n]],['AS14',[t,0n,60n,3600n,1n<<62n,7n]],['AS15',[t,0n,(1n<<32n)-1n,1n,0n,(1n<<64n)-1n]],['AS16',[1n,0n,10n,600n,153000n,0n]],['AS17',[1n,0n,10n,600n,153600n,0n]],['AS18',[MAX256,0n,10n,600n,0n,15420n]],['AS19',[MAX256,0n,10n,600n,0n,15360n]]];let checksum=0n,base;
for(const[name,args]of cases){for(let i=0;i<10000;i++)checksum^=asert(...args).target;const ns=[];for(let round=0;round<11;round++){const start=process.hrtime.bigint();for(let i=0;i<100000;i++)checksum^=asert(...args).target;ns.push(Number(process.hrtime.bigint()-start)/100000);}ns.sort((a,b)=>a-b);if(name==='AS1')base=ns[5];assert(ns[5]<=1000000&&ns[5]<=10*base);console.log(JSON.stringify({case:name,medianNs:ns[5],minNs:ns[0],maxNs:ns[10],rounds:11,callsPerRound:100000}));}
console.log(JSON.stringify({passed:true,checksum:checksum.toString()}));
