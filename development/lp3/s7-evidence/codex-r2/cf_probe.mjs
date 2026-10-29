import {createInterface} from 'node:readline';
import * as m from '../../../../prototypes/lp3-model/model.ts';
const result=f=>{try{return f();}catch(e){return{error:e.message}}};
for await(const line of createInterface({input:process.stdin})){const v=JSON.parse(line),a=v.a.map(BigInt),p={closing:v.closing,closeH:a[0],cursorDone:v.done,isFinal:v.final};
console.log(JSON.stringify({closing:m.closing(v.first===null?null:BigInt(v.first),a[3],a[4],a[9],a[10]),cursor:result(()=>m.cursorBound(a[0],a[1],a[2])),gate:result(()=>m.gate2(a[3],a[4],a[5],a[6],a[7],a[8])),final:m.finalPredicate(p,a[3],a[4],a[11],a[5],a[6]),child:m.childAllowed(p.isFinal,a[3],a[5])},(_,v)=>typeof v==='bigint'?v.toString():v));}
