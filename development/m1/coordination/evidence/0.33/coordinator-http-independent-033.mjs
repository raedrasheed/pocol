import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {Controller} from './coordinator-033-assembled/src/broker.mjs';
import {createApp} from './coordinator-033-assembled/src/server.mjs';
import {makeWorkspace,fakeSpawner,resultLine} from './coordinator-033-assembled/test/helpers.mjs';
const ws=makeWorkspace(),lf=path.join(ws,'coordination/issue-ledger.json');
const read=()=>JSON.parse(fs.readFileSync(lf,'utf8').replace(/^\uFEFF/,''));
const write=(p)=>fs.writeFileSync(lf,JSON.stringify({...read(),...p}));
write({standingAuthorization:{scope:'M1 specifications/tests/coordinator improvements',autonomousSequentialCycles:true,noProduction:true,personalApproval:false},continuationWork:{status:'ready',remainingIndependentItems:[{id:'http-fixture-next',scope:'M1',kind:'referenceTest'}]}});
const sp=fakeSpawner(),sent=[];const notifier={configured:()=>({ok:true}),describe:()=>({remote:'unix://',configured:true}),send:async x=>{sent.push(x);return {ok:true,queueId:'synthetic-http-delivery'}}};
const ctl=new Controller({workspace:ws,claudeBin:'synthetic-claude',spawnImpl:sp.spawnImpl,isAlive:()=>false,notifier});
const app=createApp({controller:ctl});await new Promise(r=>app.server.listen(0,'127.0.0.1',r));const origin='http://127.0.0.1:'+app.server.address().port;
const rh={'X-PoCol-Reviewer':app.reviewerToken,'Content-Type':'application/json'},bh={Origin:origin,'X-PoCol-Control':app.controlToken,'Content-Type':'application/json'};
async function req(url,body,headers=rh){const r=await fetch(origin+url,{method:'POST',headers,body:JSON.stringify(body)});return {status:r.status,json:await r.json()};}
const checks=[];function check(name,ok){assert.ok(ok,name);checks.push(name)}
try{
 check('attach actual HTTP', (await req('/api/reviewer/attach',{reviewerId:'independent-http-fixture',leaseMs:600000})).status===200);
 const own=(await req('/api/owner-answer',{questionId:'UI-TEST',choice:'yes',idempotencyKey:'http-test-only-answer'},bh)).json.item;await req('/api/reviewer/claim',{itemId:own.id});await req('/api/reviewer/ack',{itemId:own.id,note:'Synthetic test only'});const before=JSON.stringify(ctl.state.ownerAnswers);
 const first=(await req('/api/author-job',{mode:'smoke',idempotencyKey:'http-fixture-first-job'},bh)).json.item;await req('/api/reviewer/claim',{itemId:first.id});await req('/api/reviewer/ack',{itemId:first.id,dispatch:{}});check('one first worker via normal gates',sp.calls.length===1);
 check('review before receipt forbidden',(await req('/api/reviewer/review',{jobId:first.id,verdict:'accept',summaryAr:'Synthetic fixture'})).status===409);
 fs.writeFileSync(path.join(ctl.uiDir,'jobs',first.id,'receipt.jsonl'),resultLine());sp.calls[0].child.emit('exit',0,null);
 check('actual fixture review accepted',(await req('/api/reviewer/review',{jobId:first.id,verdict:'accept',summaryAr:'Synthetic receipt independently checked'})).status===200);await ctl.notifyIdle();
 const cont=ctl.state.order.map(x=>ctl.state.items[x]).filter(x=>x.kind==='continuation');check('one durable continuation and notification',cont.length===1&&sent.filter(x=>x.reason==='continuation').length===1);check('continuation alone starts no author',sp.calls.length===1);
 check('duplicate review rejected',(await req('/api/reviewer/review',{jobId:first.id,verdict:'accept',summaryAr:'duplicate'})).status===409);
 check('browser reviewer request blocked',(await req('/api/reviewer/attach',{reviewerId:'forbidden'}, {...rh,Origin:origin})).status===403);
 write({activeAuthor:{...read().activeAuthor,state:'reviewed',brokerJobId:first.id}});
 await req('/api/reviewer/claim',{itemId:cont[0].id});check('explicit next plan acknowledged',(await req('/api/reviewer/ack',{itemId:cont[0].id,dispatch:{mode:'smoke'},note:'Explicit synthetic next useful task'})).status===200);check('second worker starts exactly once after review',sp.calls.length===2);
 check('duplicate continuation ack rejected',(await req('/api/reviewer/ack',{itemId:cont[0].id,dispatch:{mode:'smoke'}})).status===409&&sp.calls.length===2);
 await req('/api/pause',{},bh);const second=ctl.state.currentJob;fs.writeFileSync(path.join(ctl.uiDir,'jobs',second,'receipt.jsonl'),resultLine());sp.calls[1].child.emit('exit',0,null);const rv=await req('/api/reviewer/review',{jobId:second,verdict:'accept',summaryAr:'Second synthetic receipt checked'});check('pause defers continuation',rv.json.continuation.status==='deferredPaused');
 write({continuationWork:{status:'complete',remainingIndependentItems:[]},activeAuthor:{...read().activeAuthor,state:'reviewed',brokerJobId:second}});await req('/api/resume',{},bh);check('complete scope suppresses continuation',ctl.state.order.map(x=>ctl.state.items[x]).filter(x=>x.kind==='continuation').length===1&&sp.calls.length===2);
 check('saved owner answers unchanged',JSON.stringify(ctl.state.ownerAnswers)===before);check('normal permission mode and same-host transport',sp.calls.every(x=>!x.args.some(a=>String(a).includes('dangerously'))&&x.args.includes('--permission-mode'))&&sent.filter(x=>x.reason==='continuation').every(x=>x.kind==='continuation'));
 const out={scenario:'Independent HTTP integration with synthetic worker/receipt/notifier fixtures; no real author loop or owner decisions',checks:checks.length,passed:checks.length,failed:0,workerStarts:sp.calls.length,continuationNotifications:sent.filter(x=>x.reason==='continuation').length,results:checks};const f='coordination/review-001/coordinator-http-independent-033.json';if(fs.existsSync(f))throw Error('Preserve');fs.writeFileSync(f,JSON.stringify(out,null,2));console.log(JSON.stringify({checks:out.checks,passed:out.passed,failed:0}));
}finally{ctl.stop();app.server.closeAllConnections();await new Promise(r=>app.server.close(r));}
