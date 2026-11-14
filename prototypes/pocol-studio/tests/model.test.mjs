import test from 'node:test';import assert from 'node:assert/strict';import{newProject,generateHTML,projectFiles,validateFiles,validateProject,publicProject,parseProject,encode64,decode64,validPath}from'../dist/model.js';import{zipFiles,crc32}from'../dist/zip.js';
import{upgradeLegacyStarter}from'../dist/model.js';
test('new projects default to English content and left-to-right output',()=>{for(const template of['personal','business','product']){const p=newProject(template);assert.equal(p.lang,'en');assert.doesNotMatch(p.name+p.title+p.description+p.sections.map(s=>s.title+s.body).join(''),/[\u0600-\u06ff]/);assert.match(generateHTML(p),/<html lang="en" dir="ltr">/);}});
test('legacy upgrade changes only untouched unpublished Arabic starter content',()=>{const p=newProject('personal','ar'),next=upgradeLegacyStarter(p);assert.equal(next.lang,'en');assert.equal(next.id,p.id);assert.equal(next.created,p.created);assert.deepEqual(next.sections.map(s=>s.id),p.sections.map(s=>s.id));assert.equal(p.lang,'ar');for(const key of['name','brand','eyebrow','title','description','button','color','surface']){const edited=structuredClone(p);edited[key]+=' changed';assert.strictEqual(upgradeLegacyStarter(edited),edited);}for(const mutate of[x=>x.sections[0].body+=' custom',x=>x.sections.reverse(),x=>x.sections.pop(),x=>x.logo={mime:'image/png',base64:'AA=='},x=>x.published={site:'0x'+'ab'.repeat(20)},x=>x.mode='files',x=>x.lang='en',x=>x.template='business']){const edited=structuredClone(p);mutate(edited);assert.strictEqual(upgradeLegacyStarter(edited),edited);}});
test('all templates generate self-contained UTF-8 sites in both languages',()=>{for(const lang of['ar','en'])for(const template of['personal','business','product']){const p=newProject(template,lang),files=projectFiles(p),html=generateHTML(p);assert.equal(files.length,1);assert.equal(files[0].path,'/index.html');assert.ok(files[0].data.length<1048576);assert.match(html,new RegExp(`lang="${lang}"`));assert.match(html,new RegExp(`dir="${lang==='ar'?'rtl':'ltr'}"`));assert.ok(!/https?:\/\//.test(html));assert.equal(validateFiles(files).count,1);}});
test('editable template content cannot break out into HTML or CSS',()=>{const p=newProject();p.title='<script>alert(1)</script>';p.brand='"><img src=x onerror=alert(1)>';p.description='</style><iframe src=https://attacker.invalid>';p.color='red;}body{background:url(https://attacker.invalid)';const html=generateHTML(p);assert.ok(!html.includes('<script>'));assert.ok(!html.includes('<iframe'));assert.match(html,/&lt;script&gt;/);assert.ok(!html.includes('body{background:url'));});
test('PoCol limits and unsupported or traversal paths fail before publishing',()=>{const file=(path,n=1)=>({path,data:new Uint8Array(n)});for(const path of['/../x.html','/a/../x.html','//index.html','/a%20b.html','/a\\b.html','/مرحبا.html','/test.php','/'])assert.throws(()=>validateFiles([file(path)],path));assert.throws(()=>validateFiles([file('/index.html',1048577)]),/fileSize/);assert.throws(()=>validateFiles([file('/index.html'),file('/index.html')]),/duplicatePath/);assert.throws(()=>validateFiles([file('/test.css')]),/entryMissing/);assert.throws(()=>validateFiles(Array.from({length:257},(_,i)=>file('/x'+i+'.html'))),/fileCount/);assert.throws(()=>validateFiles(Array.from({length:5},(_,i)=>file('/x'+i+'.html',1048576)),'/x0.html'),/totalSize/);assert.equal(validPath('/assets/my-font.woff2'),true);});
test('project roundtrip retains publication identity and layout but validates bytes',()=>{const p=newProject('business','en');p.published={site:'0x'+'ab'.repeat(20),version:'2',publishedBlock:100};const copy=parseProject(publicProject(p));assert.equal(copy.name,p.name);assert.equal(copy.published.site,p.published.site);assert.deepEqual(copy.sections.map(s=>s.title),p.sections.map(s=>s.title));assert.throws(()=>parseProject('{}'),/projectInvalid/);assert.throws(()=>validateProject({mode:'files',files:[{path:'/index.html',base64:'%%'}]}),/encoding/);});
test('binary files roundtrip exactly through bridge base64',()=>{const bytes=Uint8Array.from({length:65536},(_,i)=>i%256);assert.deepEqual(decode64(encode64(bytes)),bytes);});
test('project import rejects injected logo MIME and invalid section data',()=>{const p=newProject();p.logo={mime:'image/svg+xml',base64:'PHN2Zz4='};assert.throws(()=>validateProject(p),/logoSize/);p.logo=null;p.sections=Array(21).fill({});assert.throws(()=>validateProject(p),/sectionsLimit/);});
test('ZIP records contain exact filenames, lengths, CRC and terminal directory',async()=>{const payload=new TextEncoder().encode('مرحبا PoCol'),files=[{path:'/index.html',data:payload},{path:'/assets/app.js',data:new Uint8Array([1,2,255])}];const zip=new Uint8Array(await zipFiles(files).arrayBuffer()),v=new DataView(zip.buffer);assert.equal(v.getUint32(0,true),0x04034b50);assert.equal(v.getUint32(14,true),crc32(payload));assert.equal(v.getUint32(18,true),payload.length);assert.equal(new TextDecoder().decode(zip.slice(30,40)),'index.html');assert.deepEqual(zip.slice(40,40+payload.length),payload);assert.equal(v.getUint32(zip.length-22,true),0x06054b50);assert.equal(v.getUint16(zip.length-12,true),2);const centralOffset=v.getUint32(zip.length-6,true);assert.equal(v.getUint32(centralOffset,true),0x02014b50);});
import{capsPrecheck,capsVerdict,siteNeedsAtLeast,siteNeedsAtMost,capsDetails,capNeed,CAP_FIELDS}from'../dist/model.js';
// FN-1 (C9): the pocol_getCaps pre-check before the publisher opens.
const capsOf=(room={},count={})=>{const out={number:'0x1',hash:'0x'+'ab'.repeat(32),cumulative:true,alarmPercent:80,occupancy:{}};for(const[name,capName]of CAP_FIELDS){const cap=10n**9n,c=count[name]!==undefined?BigInt(count[name]):room[name]!==undefined?cap-BigInt(room[name]):5n;out[name]='0x'+c.toString(16);out[capName]='0x'+cap.toString(16);}return out;};
const answer=(v,ok=true)=>async()=>({ok,json:async()=>v});
const site=[{path:'/index.html',data:new Uint8Array(60000)},{path:'/a.css',data:new Uint8Array(10)}];
test('capacity needs: at least the site account, its slots and three version transactions; at most every chunk new',()=>{
  const least=siteNeedsAtLeast(site,{newSite:true}),most=siteNeedsAtMost(site,{newSite:true});
  assert.deepEqual([least.accounts,least.storageSlots,least.txCount],[1,8,4]);
  assert.equal(siteNeedsAtLeast(site,{newSite:false}).accounts,0);
  assert.equal(most.accounts,3+1+3+1,'three chunks of the 60,000-byte file, one of the stylesheet, three manifest chunks and the site');
  for(const[name]of CAP_FIELDS)assert.ok(most[name]>=least[name],name);
});
test('a site that cannot fit even at its least is refused before the publisher opens, naming the cap',async()=>{
  const v=await capsPrecheck(answer({jsonrpc:'2.0',id:1,result:capsOf({accounts:0})}),site,{newSite:true});
  assert.equal(v.checked,true);assert.deepEqual(v.refuse.map(r=>r.name),['accounts']);assert.match(capsDetails(v.refuse),/ACCT_CAP: 1 \/ 0/);
  const update=await capsPrecheck(answer({jsonrpc:'2.0',id:1,result:capsOf({accounts:0})}),site,{newSite:false});
  assert.deepEqual(update.refuse,[],'an update of an existing site may reuse stored chunks and needs no new account at least');
  assert.deepEqual(update.tight.map(r=>r.name),['accounts'],'but it may not fit if every chunk is new');
});
test('room for the least but not the most is reported, not refused; 80 % raises the alarm',async()=>{
  const most=siteNeedsAtMost(site,{newSite:true});
  const v=await capsPrecheck(answer({jsonrpc:'2.0',id:1,result:capsOf({codeBytes:most.codeBytes-1})}),site,{newSite:true});
  assert.deepEqual(v.refuse,[]);assert.deepEqual(v.tight.map(r=>r.name),['codeBytes']);assert.deepEqual(v.alarms,['codeBytes']);
  const calm=capsVerdict(capsOf({},{accounts:8n*10n**8n-1n}),siteNeedsAtLeast(site,{newSite:true}),siteNeedsAtMost(site,{newSite:true}));assert.deepEqual(calm.alarms,[]);
  const loud=capsVerdict(capsOf({},{accounts:8n*10n**8n}),siteNeedsAtLeast(site,{newSite:true}),siteNeedsAtMost(site,{newSite:true}));assert.deepEqual(loud.alarms,['accounts']);
});
test('an older node (-32601), a gateway refusal or no answer skips the pre-check; a malformed answer is refused',async()=>{
  assert.deepEqual(await capsPrecheck(answer({jsonrpc:'2.0',id:1,error:{code:-32601,message:'Method not found'}}),site,{newSite:true}),{checked:false,reason:'method',code:-32601});
  assert.deepEqual(await capsPrecheck(answer({error:'Unsupported or invalid Studio RPC request'},false),site,{newSite:true}),{checked:false,reason:'http'});
  assert.deepEqual(await capsPrecheck(async()=>{throw new TypeError('Failed to fetch')},site,{newSite:true}),{checked:false,reason:'unreachable'});
  await assert.rejects(capsPrecheck(answer({jsonrpc:'2.0',id:1,result:{accounts:'12'}}),site,{newSite:true}),/capsInvalid/);
});
test('the app runs the pre-check before opening the publisher and has every caps message in both languages',async()=>{
  const{readFile}=await import('node:fs/promises');const app=await readFile(new URL('../dist/app.js',import.meta.url),'utf8');
  const at=app.indexOf("capsPrecheck(gatewayPost"),open=app.indexOf("bridge('openPublisher'");assert.ok(at>0&&at<open,'the pre-check comes first');
  const keys=(key)=>(app.match(new RegExp('[{,]'+key+":'",'g'))||[]).length;
  for(const key of['capsRefused','capsTight','capsAlarm','capsSkipped'])assert.equal(keys(key),2,key);
  // The extension publisher's progress phases Studio shows (renderJob: t('phase_'+phase)), its FN-1 'warning' and
  // 'capsAlarm' included, each in both languages: never a raw key.
  for(const phase of['upload','broadcast','confirmed','complete','warning','capsAlarm'])assert.equal(keys('phase_'+phase),2,'phase_'+phase);
  const{createHash}=await import('node:crypto');const html=await readFile(new URL('../dist/index.html',import.meta.url),'utf8');
  assert.ok(html.includes('app.js?v='+createHash('sha256').update(app).digest('hex').slice(0,12)),'index.html loads this app.js content version');
});
test('a continuous chain (cumulative false) checks each transaction against one block\'s room',()=>{
  const least=siteNeedsAtLeast(site,{newSite:true}),most=siteNeedsAtMost(site,{newSite:true});
  assert.ok(most.txCount>1&&most.largestTxBytes<most.txBytes);
  const block={...capsOf({txCount:1,txBytes:most.largestTxBytes}),cumulative:false};
  const v=capsVerdict(block,least,most);assert.deepEqual([v.refuse,v.tight],[[],[]],'one block holds each transaction');
  const summed=capsVerdict(capsOf({txCount:1,txBytes:most.largestTxBytes}),least,most);assert.deepEqual([summed.refuse.map(r=>r.name),summed.tight.map(r=>r.name)],[['txCount'],['txBytes']],'a cumulative chain counts the whole publication');
  const tight=capsVerdict({...capsOf({txBytes:most.largestTxBytes-1}),cumulative:false},least,most);
  assert.deepEqual(tight.tight.map(r=>r.name),['txBytes']);assert.match(capsDetails(tight.tight),/TXBYTES_CAP per block: /);
  const none=capsVerdict({...capsOf({txCount:0}),cumulative:false},least,most);assert.deepEqual(none.refuse.map(r=>r.name),['txCount']);
  assert.equal(capNeed(most,'accounts',true),BigInt(most.accounts),'the state rows count the whole publication either way');
  assert.throws(()=>capsVerdict({...capsOf(),cumulative:'no'},least,most),/capsInvalid/);
});
test('Sites W2b: module imports are checked before publishing (graph, cycle, missing file, bare specifier)',()=>{const f=(path,text)=>({path,data:new TextEncoder().encode(text)});const ok=validateFiles([f('/index.html','<script type="module" src="main.js"></script>'),f('/main.js',"import{a}from'./lib/a.js';import('./lazy.js');import(x);"),f('/lib/a.js',"export const a=1;"),f('/lazy.js',"import{a}from'./lib/a.js';export default a;")]);assert.equal(ok.count,4);assert.deepEqual(ok.moduleWarnings.length,1);assert.match(ok.moduleWarnings[0],/computed specifier/);assert.throws(()=>validateFiles([f('/index.html',''),f('/a.js',"import'./b.js';"),f('/b.js',"import'./a.js';")]),/module import cycle: \/a\.js -> \/b\.js -> \/a\.js/);assert.throws(()=>validateFiles([f('/index.html',''),f('/a.js',"import'./gone.js';")]),/\/gone\.js, which is not in the site/);assert.throws(()=>validateFiles([f('/index.html',''),f('/a.js',"import React from'react';")]),/"react" cannot load/);});
