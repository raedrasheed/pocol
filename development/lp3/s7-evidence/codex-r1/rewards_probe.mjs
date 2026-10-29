import {createInterface} from 'node:readline';
import {shareRewards,splitPot} from '../../../../prototypes/lp3-model/model.ts';
for await (const l of createInterface({input:process.stdin,crlfDelay:Infinity})){const v=JSON.parse(l),s=splitPot(BigInt(v.pot),BigInt(v.alpha),BigInt(v.gamma)),r=shareRewards(s.remainder,v.counts.map(([id,count,tomb,accountExists])=>({id,count:BigInt(count),tomb,accountExists})),BigInt(v.kappa),BigInt(v.m));console.log(JSON.stringify({...s,...r},(_,v)=>typeof v==='bigint'?v.toString():v));}
