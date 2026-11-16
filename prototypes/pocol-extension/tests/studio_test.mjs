import assert from 'node:assert/strict';
import { createCipheriv, pbkdf2Sync, createHmac } from 'node:crypto';
import { build, readManifestKey } from '../build.mjs';
import { installChromeMock, installFetchMock, loadWorker } from './chrome_mock.mjs';
import { siteFixture } from './site_fixture.mjs';
import { devnetSpecV3Hex, devnetSpecV1Hex } from './rp_fixture.mjs';
import { readFileSync, writeFileSync, mkdtempSync, cpSync, rmSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import vm from 'node:vm';
const {dist,manifest}=build();
const K=await import('../dist/lib/studio-keyfile.js'), B=await import('../dist/lib/studio-bridge.js'), N=await import('../dist/lib/studio-network.js'), RP=await import('../dist/lib/rp.js'), H=await import('../dist/lib/hex.js');
let count=0; const test=async(name,fn)=>{await fn();count++;console.log('PASS '+name);};
const secret=new Uint8Array(32);secret[31]=1;
const password='correct horse battery staple';
const text=(v)=>new TextEncoder().encode(JSON.stringify(v));
await test('pinned genesis preimage proves chain and genesis hash',()=>{const spec=RP.decodeSpec(H.fromHex(N.NETWORK.genesisPre));assert.equal(spec.chainId,BigInt(N.NETWORK.chainId));assert.equal(H.toHex(spec.genesisHash),N.NETWORK.genesisHash);});
// D1: until the identity switch the built-in profile is the marked placeholder (deploy/mainnet/placeholder-network.pocol.json,
// a spec-2 genesis of no network); either way it is a spec-2 genesis of a network that is not retired.
await test('built-in profile is the marked spec-2 placeholder until the identity switch, never a retired network',()=>{
  const spec=RP.decodeSpec(H.fromHex(N.NETWORK.genesisPre));assert.equal(spec.specVersion,2);
  const retired=JSON.parse(readFileSync(new URL('../../../deploy/mainnet/retired-networks.json',import.meta.url),'utf8')).retired;
  assert.ok(retired.every(r=>r.chainId!==N.NETWORK.chainId&&r.genesisHash!==N.NETWORK.genesisHash),'the built-in profile pins a retired network');
  const placeholder=JSON.parse(readFileSync(new URL('../../../deploy/mainnet/placeholder-network.pocol.json',import.meta.url),'utf8'));
  const pinned=N.NETWORK.genesisPre===placeholder.files['genesis.spec.hex'];
  assert.equal(N.NETWORK_PLACEHOLDER===true,pinned,'the placeholder mark goes with the placeholder identity, and only with it');
  assert.equal(N.NETWORK.name,'pocol-public');assert.equal(N.NETWORK.trustLevel,'RN');assert.equal(N.NETWORK.endpoint,'https://gateway.pocol.net/studio/rpc');
});
await test('HTTPS only exact pinned RN endpoint',()=>{assert.equal(N.validateEndpoint(N.NETWORK.endpoint,'RN'),N.NETWORK.endpoint);for(const u of ['http://gateway.pocol.net/studio/rpc','https://gateway.pocol.net/studio/rpc?x=1','https://gateway.pocol.net.evil/studio/rpc','https://user@gateway.pocol.net/studio/rpc','https://example.org/'])assert.throws(()=>N.validateEndpoint(u,'RN'));});
await test('remote endpoint cannot claim local verification or unsupported RP',()=>{assert.throws(()=>N.validateEndpoint(N.NETWORK.endpoint,'LN'));assert.throws(()=>N.validateEndpoint(N.NETWORK.endpoint,'RP'));assert.match(N.trustMessage('RN'),/remote/);});
await test('loopback preserved and RN rejected on loopback',()=>{assert.equal(N.validateEndpoint('http://127.0.0.1:8540','LN'),'http://127.0.0.1:8540');assert.throws(()=>N.validateEndpoint('http://127.0.0.1:8540','RN'));});
await test('wallet AES-GCM export roundtrip preserves exact desktop key address',async()=>{const box=await K.exportKeyfile(secret,password);assert.equal(box.address,'0x7e5f4552091a69125d5dfcb7b8c2659029395bdf');assert.deepEqual(await K.importKeyfile(text(box),password),secret);await assert.rejects(K.importKeyfile(text(box),'wrong password'));});
await test('Windows encrypted export interoperates with independent Node crypto',async()=>{
  const salt=Buffer.alloc(16,4),iv=Buffer.alloc(16,9),derived=pbkdf2Sync(password,salt,600000,64,'sha256');
  const cipher=createCipheriv('aes-256-cbc',derived.subarray(0,32),iv),ct=Buffer.concat([cipher.update(secret),cipher.final()]);
  const mac=createHmac('sha256',derived.subarray(32)).update(Buffer.concat([Buffer.from('PoCol-Studio-export-v1\0'),salt,iv,ct])).digest();
  const box={schema:'pocol-studio-key-v1',kdf:'PBKDF2-SHA256',iterations:600000,cipher:'AES-256-CBC-HMAC-SHA256',salt:salt.toString('base64'),iv:iv.toString('base64'),ciphertext:ct.toString('base64'),mac:mac.toString('base64')};
  assert.deepEqual(await K.importKeyfile(text(box),password),secret);
  await assert.rejects(K.importKeyfile(text({...box,mac:Buffer.alloc(32).toString('base64')}),password));
  await assert.rejects(K.importKeyfile(text({...box,iv:Buffer.alloc(16).toString('base64')}),password));
  await assert.rejects(K.importKeyfile(text({...box,iterations:1}),password));
});
await test('DPAPI binary rejected with one-time Windows export instruction',async()=>{await assert.rejects(K.importKeyfile(new TextEncoder().encode('POCOL-DPAPI-USER-V1\0ciphertext'),password),/Export-Studio-Wallet/);});
await test('invalid scalar, short password and oversized keyfile rejected',async()=>{assert.throws(()=>K.validSecret(new Uint8Array(32)));assert.throws(()=>K.validSecret(new Uint8Array(31)));await assert.rejects(K.exportKeyfile(secret,'short'));await assert.rejects(K.importKeyfile(new Uint8Array(16385),password));});
const basic={title:'Example',files:[{path:'/index.html',base64:btoa('<h1>Hi</h1>')}],entry:'/index.html'};
await test('file broker validates exact bytes and strips unknown fields',()=>{const out=B.validatePublishRequest({...basic,privateKey:'never copied'});assert.equal(out.totalBytes,11);assert.equal(out.privateKey,undefined);});
await test('file broker accepts complete 4 MiB site, rejects overflow and per-file overflow',()=>{
  const meg=btoa('x'.repeat(1048576));const files=Array.from({length:4},(_,i)=>({path:i===0?'/index.html':`/f${i}.txt`,base64:meg}));
  assert.equal(B.validatePublishRequest({files}).totalBytes,4194304);
  assert.throws(()=>B.validatePublishRequest({files:[...files,{path:'/overflow.txt',base64:'eA=='}]}));
  assert.throws(()=>B.validatePublishRequest({files:[{path:'/index.html',base64:btoa('x'.repeat(1048577))}]}));
});
await test('duplicate, traversal, missing entry, malformed address and noncanonical base64 rejected',()=>{for(const input of [{files:[basic.files[0],basic.files[0]]},{files:[{path:'/../x.html',base64:''}]},{files:[{path:'/x.html',base64:''}]},{...basic,site:'0x123'},{files:[{path:'/index.html',base64:'eB=='}]}])assert.throws(()=>B.validatePublishRequest(input));});
await test('open target forbids URL traversal and arbitrary links',()=>{assert.equal(B.validateSiteTarget({site:'0x'+'ab'.repeat(20)}).path,'/');for(const path of ['https://evil/','/../x','//evil','/%2e'])assert.throws(()=>B.validateSiteTarget({site:'0x'+'ab'.repeat(20),path}));});
await test('W3: Studio and IDE content scripts match exactly the production origins; no retired preview origin is built',()=>{
  assert.deepEqual(manifest.content_scripts.map(c=>c.matches),[['https://studio.pocol.net/*'],['https://ide.pocol.net/*']]);
  assert.deepEqual(manifest.content_scripts.map(c=>c.js),[['studio-content.js'],['ide-content.js']]);
  assert.deepEqual(N.STUDIO_ORIGINS,['https://studio.pocol.net']);
  assert.equal(N.isStudioOrigin('https://studio.pocol.net/x'),true);
  for(const u of ['https://pocol-studio.mystic-gecko-9511.chatgpt.site/','https://studio.pocol.net.evil/','http://studio.pocol.net/','https://ide.pocol.net/'])assert.equal(N.isStudioOrigin(u),false,u);
  const walk=d=>readdirSync(d,{withFileTypes:true}).flatMap(e=>e.isDirectory()?walk(join(d,e.name)):[join(d,e.name)]);
  const hits=walk(dist).filter(f=>readFileSync(f).includes('chatgpt.site'));assert.deepEqual(hits,[]);
  for(const origin of ['https://studio.pocol.net','https://pocol-studio.mystic-gecko-9511.chatgpt.site']){let listener=null;const window={top:null,addEventListener:(_t,fn)=>{listener=fn;},postMessage(){}};window.top=window;
    vm.runInNewContext(readFileSync(dist+'/studio-content.js','utf8'),{window,location:{origin},chrome:{runtime:{sendMessage:async()=>({ok:true})}}});
    assert.equal(listener!==null,origin==='https://studio.pocol.net',origin);}
});
await test('manifest grants only fixed origins, publisher in package, unlimited checkpoint storage',()=>{assert.ok(manifest.permissions.includes('unlimitedStorage'));assert.deepEqual(manifest.host_permissions,['http://127.0.0.1/*','https://gateway.pocol.net/*']);assert.equal(manifest.optional_host_permissions,undefined);assert.equal(manifest.content_scripts[0].all_frames,false);assert.match(readFileSync(dist+'/publisher.html','utf8'),/publisher-ui.js/);});

await test('W14: the extension is 0.5.0 (the manifest version comes from package.json) with the committed manifest key, none while the slot is the placeholder',()=>{
  assert.equal(JSON.parse(readFileSync(new URL('../package.json',import.meta.url),'utf8')).version,'0.5.0');assert.equal(manifest.version,'0.5.0');
  const lock=JSON.parse(readFileSync(new URL('../package-lock.json',import.meta.url),'utf8'));assert.equal(lock.version,'0.5.0');assert.equal(lock.packages[''].version,'0.5.0');
  const key=readManifestKey();assert.equal(manifest.key,key===null?undefined:key);
});

const mock=installChromeMock();await loadWorker(dist,'studio-regressions');
const sender={id:chrome.runtime.id,url:'https://studio.pocol.net/',frameId:0,tab:{id:17}};
const call=(type,payload={},who=sender)=>new Promise(resolve=>{const keep=mock.listeners.message[0]({type,payload},who,resolve);if(keep!==true)resolve(undefined);});
await test('public bridge status exposes capability and network without wallet',async()=>{const r=await call('studio.status');assert.equal(r.ok,true);assert.equal(r.result.network.chainId,N.NETWORK.chainId);assert.equal(r.result.wallet,undefined);});
await test('built-in public profile cannot be shadowed by an imported custom profile',async()=>{const r=await mock.send({type:'profiles.add',profile:{...N.NETWORK,endpoint:'http://127.0.0.1:8540',trustLevel:'LN'}});assert.equal(r.ok,false);const list=(await mock.send({type:'profiles.list'})).profiles;assert.equal(list.find(p=>p.name===N.NETWORK.name).endpoint,N.NETWORK.endpoint);});
await test('content script cannot call generic wallet/profile operations',async()=>{for(const type of ['wallet.create','wallet.unlock','wallet.address','profiles.add','profiles.publicSchedule'])assert.equal((await call(type)).ok,false);assert.equal(mock.storage.data.has('wallet'),false);});
await test('lookalike origin and iframe cannot call broker',async()=>{assert.equal((await call('studio.status',{}, {...sender,url:'https://studio.pocol.net.evil/'})).ok,false);assert.equal((await call('studio.status',{}, {...sender,frameId:1})).ok,false);});
let jobId;
await test('publish request opens trusted page and stores only bounded public files, no signature',async()=>{const r=await call('studio.openPublisher',basic);assert.equal(r.ok,true);jobId=r.result.jobId;assert.match(mock.tabsCreated.at(-1).url,/chrome-extension:\/\/.*\/publisher.html#/);const j=mock.storage.data.get(B.JOB_PREFIX+jobId);assert.equal(j.status,'awaitingApproval');assert.equal(j.checkpoint,undefined);assert.equal(j.request.files.length,1);});
await test('job results are origin and tab bound',async()=>{assert.equal((await call('studio.jobStatus',{jobId})).ok,true);assert.equal((await call('studio.jobStatus',{jobId},{...sender,tab:{id:18}})).ok,false);for(const url of ['https://pocol-studio.example.org/','https://pocol-studio.mystic-gecko-9511.chatgpt.site/','http://studio.pocol.net/'])assert.equal((await call('studio.jobStatus',{jobId},{...sender,url})).ok,false);});
await test('status history survives page reload and is scoped to original tab',async()=>{const a=await call('studio.status'),b=await call('studio.status',{}, {...sender,tab:{id:18}});assert.equal(a.result.jobs[0].jobId,jobId);assert.equal(b.result.jobs.length,0);});
await test('old signed checkpoint survives cleanup, old unsigned draft removed',async()=>{await mock.storage.set({[B.JOB_PREFIX+'signed-old']:{id:'signed-old',createdAt:1,status:'paused',checkpoint:{transactions:{tx:{raw:'0x02'}}}},[B.JOB_PREFIX+'draft-old']:{id:'draft-old',createdAt:1,status:'awaitingApproval'}});const r=await call('studio.openPublisher',basic);assert.equal(r.ok,true);assert.ok(mock.storage.data.has(B.JOB_PREFIX+'signed-old'));assert.equal(mock.storage.data.has(B.JOB_PREFIX+'draft-old'),false);await mock.storage.remove(B.JOB_PREFIX+r.result.jobId);});
// Sites W4: jobs carry their network; on install or update, jobs of another network move to the read-only archive.
await test('W4: a new publish job records the network it was created for',async()=>{const j=mock.storage.data.get(B.JOB_PREFIX+jobId);assert.deepEqual(j.network,{chainId:String(N.NETWORK.chainId),genesisHash:N.NETWORK.genesisHash.toLowerCase()});});
await test('W4: install or update archives jobs of another network, keeps this network\'s, and drops their side keys',async()=>{
  const here={chainId:String(N.NETWORK.chainId),genesisHash:N.NETWORK.genesisHash},signed={transactions:{'chunk:x':{hash:'0x'+'aa'.repeat(32),tx:{chainId:'777'}}}};
  await mock.storage.set({[B.JOB_PREFIX+'foreign-net']:{id:'foreign-net',createdAt:Date.now(),status:'paused',walletAddress:'0x'+'11'.repeat(20),network:{chainId:'777',genesisHash:'0x'+'cd'.repeat(32)},checkpoint:signed},
    ['studio:files:foreign-net']:[{path:'/index.html',base64:'eA=='}],['studio:raw:foreign-net:0xaa']:'0x02',
    ['ide:job:legacy-chain']:{id:'legacy-chain',kind:'contract',createdAt:Date.now(),status:'pending',checkpoint:{transactions:{contract:{hash:'0x'+'bb'.repeat(32),tx:{chainId:'777'}}}}},
    [B.JOB_PREFIX+'same-net']:{id:'same-net',createdAt:Date.now(),status:'paused',network:here,checkpoint:{transactions:{'chunk:y':{tx:{chainId:String(N.NETWORK.chainId)}}}}}});
  assert.equal(mock.listeners.installed.length,1);mock.listeners.installed[0]({reason:'update'});
  for(let i=0;i<50&&mock.storage.data.has(B.JOB_PREFIX+'foreign-net');i++)await new Promise(r=>setTimeout(r,5));
  assert.equal(mock.storage.data.has(B.JOB_PREFIX+'foreign-net'),false);assert.equal(mock.storage.data.get('archive:'+B.JOB_PREFIX+'foreign-net').archivedReason,'foreign-network');
  for(let i=0;i<50&&mock.storage.data.has('ide:job:legacy-chain');i++)await new Promise(r=>setTimeout(r,5));
  assert.equal(mock.storage.data.has('ide:job:legacy-chain'),false,'a pre-W4 job of another chain id');assert.ok(mock.storage.data.has('archive:ide:job:legacy-chain'));
  assert.equal(mock.storage.data.has('studio:files:foreign-net'),false);assert.equal(mock.storage.data.has('studio:raw:foreign-net:0xaa'),false);
  assert.ok(mock.storage.data.has(B.JOB_PREFIX+'same-net'),'this network\'s job is kept');await mock.storage.remove(B.JOB_PREFIX+'same-net');
});
await test('W4: a collected job takes its side keys with it',async()=>{
  await mock.storage.set({[B.JOB_PREFIX+'draft-side']:{id:'draft-side',createdAt:1,status:'awaitingApproval'},['studio:files:draft-side']:[],['studio:raw:draft-side:0x01']:'0x02'});
  // Another tab: the broker allows each tab six requests a minute, and the later tests use tab 17's.
  const r=await call('studio.openPublisher',basic,{...sender,tab:{id:31}});assert.equal(r.ok,true);
  for(const k of[B.JOB_PREFIX+'draft-side','studio:files:draft-side','studio:raw:draft-side:0x01'])assert.equal(mock.storage.data.has(k),false,k);
  await mock.storage.remove(B.JOB_PREFIX+r.result.jobId);
});
await test('cleanup preserves an expired job while its publisher holds the job lock',async()=>{
  const id='active-old',key=B.JOB_PREFIX+id;
  await mock.storage.set({[key]:{id,createdAt:1,status:'publishing'}});
  await navigator.locks.request('pocol:job:'+id,{mode:'exclusive'},async()=>{
    const r=await call('studio.openPublisher',basic);assert.equal(r.ok,true);
    assert.ok(mock.storage.data.has(key));await mock.storage.remove(B.JOB_PREFIX+r.result.jobId);
  });
  const r=await call('studio.openPublisher',basic);assert.equal(r.ok,true);
  assert.equal(mock.storage.data.has(key),false);await mock.storage.remove(B.JOB_PREFIX+r.result.jobId);
});
await test('cleanup rereads a new signed checkpoint instead of deleting its stale unsigned snapshot',async()=>{
  const id='signed-during-cleanup',key=B.JOB_PREFIX+id,draft={id,createdAt:1,status:'paused'};
  await mock.storage.set({[key]:draft});
  const originalGet=mock.storage.get;let injected=false;
  mock.storage.get=async(keys)=>{const snapshot=await originalGet(keys);if(keys===null&&!injected){injected=true;await mock.storage.set({[key]:{...draft,checkpoint:{transactions:{tx:{raw:'0x02'}}}}});}return snapshot;};
  try {const r=await call('studio.openPublisher',basic);assert.equal(r.ok,true);assert.equal(mock.storage.data.get(key).checkpoint.transactions.tx.raw,'0x02');await mock.storage.remove(B.JOB_PREFIX+r.result.jobId);}
  finally {mock.storage.get=originalGet;await mock.storage.remove(key);}
});
await test('openSite uses only pinned network viewer, no arbitrary tab URL',async()=>{const r=await call('studio.openSite',{site:'0x'+'ab'.repeat(20)});assert.equal(r.ok,true);assert.match(mock.tabsCreated.at(-1).url,/#pocol-public\/0x/);});
await test('content bridge accepts only same window and exact origin, replies no secrets',async()=>{
  let listener;const posted=[];const window={top:null,postMessage:(m,o)=>posted.push([m,o]),addEventListener:(_,f)=>listener=f};window.top=window;
  const context={window,location:{origin:'https://studio.pocol.net'},chrome:{runtime:{sendMessage:async()=>({ok:true,result:{version:'0.2.0'}})}}},payload={source:'pocol-studio',id:'1',method:'status'};
  vm.runInNewContext(readFileSync(dist+'/studio-content.js','utf8'),context);
  await listener({source:{},origin:context.location.origin,data:payload});await listener({source:window,origin:'https://evil',data:payload});assert.equal(posted.length,0);
  await listener({source:window,origin:context.location.origin,data:payload});assert.equal(posted[0][0].source,'pocol-extension');assert.equal(posted[0][1],context.location.origin);
});
// W14: the built-in profile's fork schedule is pinned by the release ([[1,'0']], what the fresh network's genesis
// metadata carries). There is no settings operation for it, and neither storage nor RPC metadata changes it.
const legacyScheduleKey='pocol:publicForkSchedule';
const settingsSender={id:chrome.runtime.id,url:chrome.runtime.getURL('options.html'),frameId:0,tab:{id:31}};
const getPublic=async()=>(await mock.send({type:'profiles.list'})).profiles.find(p=>p.name===N.NETWORK.name);
const initialPublic=await getPublic();
await test('W14: the built-in profile carries the pinned schedule [[1,"0"]], also in studio.status and ide.status',async()=>{
  assert.deepEqual(initialPublic.forkSchedule,[[1,'0']]);assert.equal(initialPublic.invalid,undefined);assert.equal(initialPublic.conflict,false);
  assert.deepEqual((await call('studio.status')).result.network.forkSchedule,[[1,'0']]);
  assert.deepEqual((await call('ide.status',{}, {...sender,url:'https://ide.pocol.net/'})).result.network.forkSchedule,[[1,'0']]);
});
await test('W14: the removed settings operation for the built-in schedule is refused, from the settings page too, and writes nothing',async()=>{
  for(const who of [settingsSender,{id:chrome.runtime.id}]){
    const r=await new Promise(resolve=>mock.listeners.message[0]({type:'profiles.publicSchedule',forkSchedule:[[1,'0'],[2,'42']]},who,resolve));
    assert.equal(r.ok,false);assert.match(String(r.error),/unknown message/);
  }
  assert.equal(mock.storage.data.has(legacyScheduleKey),false);assert.deepEqual((await getPublic()).forkSchedule,[[1,'0']]);
});
await test('W14: a schedule saved by releases up to 0.4.1 is ignored, a corrupt one too, and install or update removes it',async()=>{
  const savedVault={schema:'pocol-studio-vault-v1',ciphertext:'opaque-test-marker'};
  await mock.storage.set({'studio:wallet':savedVault});
  for(const stale of [[[1,'0'],[2,'42']],[[1,'0'],[3,'43']],'not a schedule']){
    await mock.storage.set({[legacyScheduleKey]:stale});
    const pub=await getPublic();assert.equal(pub.invalid,undefined);assert.deepEqual(pub.forkSchedule,[[1,'0']]);
    for(const key of ['name','chainId','genesisPre','genesisHash','endpoint','trustLevel','netKey'])assert.equal(pub[key],initialPublic[key]);
    const status=await call('studio.status');assert.equal(status.ok,true);assert.deepEqual(status.result.network.forkSchedule,[[1,'0']]);
  }
  mock.listeners.installed[0]({reason:'update'});
  for(let i=0;i<50&&mock.storage.data.has(legacyScheduleKey);i++)await new Promise(r=>setTimeout(r,5));
  assert.equal(mock.storage.data.has(legacyScheduleKey),false);assert.deepEqual(mock.storage.data.get('studio:wallet'),savedVault);
});
await test('W14: RPC metadata advertising an activation cannot change the built-in schedule of an open Viewer session',async()=>{
  const fx=await siteFixture(dist);
  const log=installFetchMock({...fx.handlers,eth_chainId:()=> '0x'+N.NETWORK.chainId.toString(16),pocol_status:()=>({chainId:N.NETWORK.chainId,genesisHash:N.NETWORK.genesisHash,forkSchedule:[[1,'0'],[2,'1']],activeVersion:2})});
  await mock.send({type:'wallet.create',password:'studio-test-pw',seedHex:'0x'+'42'.repeat(32)});
  const address=(await mock.send({type:'wallet.address',name:N.NETWORK.name})).address;
  const opened=await mock.send({type:'open',name:N.NETWORK.name,site:fx.site,path:'/'});assert.equal(opened.ok,true,JSON.stringify(opened));
  assert.deepEqual((await getPublic()).forkSchedule,[[1,'0']]);
  assert.deepEqual((await call('studio.status')).result.network.forkSchedule,[[1,'0']]);
  assert.equal((await mock.send({type:'wallet.address',name:N.NETWORK.name})).address,address);
  assert.equal(log.some(entry=>entry.method==='eth_sendRawTransaction'),false);
});
await test('W14: a custom profile of the built-in chain with a cap-raise schedule conflicts with the pinned one and is read-only',async()=>{
  const custom={...N.NETWORK,name:'operator-local',trustLevel:'RP',endpoint:'http://127.0.0.1:8540',forkSchedule:[[1,'0'],[2,'42'],[3,'43']]};
  const added=await mock.send({type:'profiles.add',profile:custom});assert.equal(added.ok,true,JSON.stringify(added));
  const list=(await mock.send({type:'profiles.list'})).profiles;
  assert.equal(list.find(p=>p.name==='operator-local').conflict,true);assert.equal(list.find(p=>p.name===N.NETWORK.name).conflict,true);
  assert.deepEqual(list.find(p=>p.name===N.NETWORK.name).forkSchedule,[[1,'0']]);assert.equal(list.find(p=>p.name==='operator-local').netKey,initialPublic.netKey);
  await mock.send({type:'profiles.remove',name:'operator-local'});
  assert.equal((await getPublic()).conflict,false);
});
// W13: a profile is a spec-2 genesis with nonceMode 1, as pocold admits (R24); a spec-1 or bounded-range GenesisSpec still
// decodes but is refused as a profile with pocold's reason.
await test('W13: a spec-1 or nonceMode-0 GenesisSpec is refused as a profile with the node\'s reason',async()=>{
  const R=await import('../dist/lib/rlp.js'),Cr=await import('../dist/lib/crypto.js');
  const raw=R.decode(H.fromHex('0x'+devnetSpecV3Hex()));
  const asProfile=(name,bytes)=>{const spec=RP.decodeSpec(bytes);return {name,chainId:Number(spec.chainId),genesisPre:H.toHex(bytes),genesisHash:H.toHex(Cr.keccak(bytes)),trustLevel:'RP',endpoint:'http://127.0.0.1:8540'};};
  const v3=R.encode(raw),v1=H.fromHex('0x'+devnetSpecV1Hex()),bounded=R.encode([raw[0],raw[1],raw[2].map((x,i)=>(i===4?R.encUint(0n):x)),raw[3],raw[4],raw[5]]);
  assert.equal(RP.decodeSpec(v1).specVersion,1);assert.equal(RP.cpv(RP.decodeSpec(bounded),'nonceMode'),0n);
  const ok=await mock.send({type:'profiles.add',profile:asProfile('v3-devnet',v3)});assert.equal(ok.ok,true,JSON.stringify(ok));
  for(const [name,bytes,reason] of [['spec1-devnet',v1,/GenesisSpec version 1: the spec-1 rule set is retired; this release admits only a specVersion-2 \(v3\) genesis/],
                                    ['bounded-devnet',bounded,/nonceMode 0: bounded nonce ranges are a retired spec-1 rule/]]){
    const r=await mock.send({type:'profiles.add',profile:asProfile(name,bytes)});assert.equal(r.ok,false);assert.match(String(r.error?.message||r.error),reason);
  }
  // A stored spec-1 profile (saved by an earlier release) is listed invalid, so it is unusable.
  const stored=structuredClone(mock.storage.data.get('profiles'));stored.push(asProfile('spec1-stored',v1));mock.storage.data.set('profiles',stored);
  const listed=(await mock.send({type:'profiles.list'})).profiles.find(p=>p.name==='spec1-stored');assert.match(listed.invalid,/spec-1 rule set is retired/);
  for(const name of ['v3-devnet','spec1-stored'])await mock.send({type:'profiles.remove',name});
});
// Every profile keys its wallet with its own netKey (D14), the built-in one included: a wallet's address belongs to
// one network, so an extension pinned to another network gives the built-in profile that network's address, and
// no extension update moves a custom profile's address.
const W=await import('../dist/lib/wallet.js'),C=await import('../dist/lib/crypto.js'),{Transaction}=await import('ethers');
const viewerSeed='0x'+'42'.repeat(32);
const fixtureSpec=(from,to)=>{let pre='0x'+devnetSpecV3Hex();if(from){assert.ok(pre.includes(from));pre=pre.replace(from,to);}const spec=RP.decodeSpec(H.fromHex(pre));return {pre,chainId:Number(spec.chainId),hash:H.toHex(C.keccak(H.fromHex(pre)))};};
const localProfile=(name,s,forkSchedule=[[1,'0']])=>({name,chainId:s.chainId,genesisPre:s.pre,genesisHash:s.hash,trustLevel:'RP',endpoint:'http://127.0.0.1:8540',forkSchedule});
const walletOf=async(m,name)=>(await m.send({type:'wallet.address',name})).address;
const keyOf=async(m,name)=>(await m.send({type:'profiles.list'})).profiles.find(p=>p.name===name).netKey;
const addressFor=async(netKey)=>W.addressOf(await W.networkKey(H.fromHex(viewerSeed),netKey));
// The network the copy below is switched to; a custom profile of it is added here, before the "update".
const nextNetwork=fixtureSpec();let nextBefore;
let viewerAddress;
await test('the built-in profile and every custom profile key their wallets with their own netKey',async()=>{
  await mock.send({type:'wallet.create',password:'studio-test-pw',replace:true,seedHex:viewerSeed});
  const builtinKey=await keyOf(mock,N.NETWORK.name);assert.equal(builtinKey,(await getPublic()).netKey);
  viewerAddress=await walletOf(mock,N.NETWORK.name);
  assert.equal(viewerAddress,await addressFor(builtinKey));
  assert.equal(W.PUBLIC_WALLET_SALT,undefined,'no fixed built-in salt');
  // A custom profile with the pinned identity has the same netKey, so it shares the address.
  assert.equal((await mock.send({type:'profiles.add',profile:{...N.NETWORK,name:'operator-local',trustLevel:'RP',endpoint:'http://127.0.0.1:8540',forkSchedule:(await getPublic()).forkSchedule}})).ok,true);
  const localKey=await keyOf(mock,'operator-local');assert.equal(localKey,builtinKey);
  assert.equal(await walletOf(mock,'operator-local'),viewerAddress);
  assert.equal((await mock.send({type:'profiles.add',profile:localProfile('next-net',nextNetwork)})).ok,true);
  nextBefore=await walletOf(mock,'next-net');
  assert.equal(nextBefore,await addressFor(await keyOf(mock,'next-net')));assert.notEqual(nextBefore,viewerAddress);
  for(const name of ['operator-local','next-net'])await mock.send({type:'profiles.remove',name});
});
await test('after the pinned identity changes, the built-in profile keys its wallet with the new netKey, also for a site, and custom profiles keep theirs',async()=>{
  // A copy of the built extension pinned to nextNetwork, as the identity-switch PR switches it.
  const switched=mkdtempSync(join(tmpdir(),'pocol-switched-'));
  try{
    // The copy sits outside this package, so it needs its own module type.
    cpSync(dist,switched,{recursive:true});writeFileSync(join(switched,'package.json'),'{"type":"module"}\n');
    const file=join(switched,'lib','studio-network.js'),src=readFileSync(file,'utf8');
    const out=src.replace(/"chainId": \d+/,`"chainId": ${nextNetwork.chainId}`).replace(/"genesisHash": "0x[0-9a-f]{64}"/,`"genesisHash": "${nextNetwork.hash}"`).replace(/"genesisPre": "0x[0-9a-f]+"/,`"genesisPre": "${nextNetwork.pre}"`);
    assert.notEqual(out,src);writeFileSync(file,out);
    const m=installChromeMock();await loadWorker(switched,'switched-identity');
    const pub=(await m.send({type:'profiles.list'})).profiles.find(p=>p.name===N.NETWORK.name);
    assert.equal(pub.invalid,undefined);assert.equal(pub.chainId,nextNetwork.chainId);assert.equal(pub.genesisHash,nextNetwork.hash);
    await m.send({type:'wallet.create',password:'studio-test-pw',seedHex:viewerSeed});
    // The built-in profile now has the new network's netKey, so its wallet is that network's address, the one a
    // custom profile of that network had before the update; the old built-in address is not reused.
    const switchedAddress=await walletOf(m,N.NETWORK.name);
    assert.equal(switchedAddress,await addressFor(pub.netKey));assert.equal(switchedAddress,nextBefore);assert.notEqual(switchedAddress,viewerAddress);
    // The custom profile of nextNetwork keeps the address the extension gave it before the update.
    assert.equal((await m.send({type:'profiles.add',profile:localProfile('next-net',nextNetwork,pub.forkSchedule)})).ok,true);
    assert.equal(await walletOf(m,'next-net'),nextBefore);
    // Profiles of another chain ID, or of the same chain ID with another genesis, key their wallets by their netKey.
    const other=fixtureSpec('830bdee8','830bdee9'),twin=fixtureSpec('846553f100','846553f101');
    assert.notEqual(other.chainId,nextNetwork.chainId);assert.equal(twin.chainId,nextNetwork.chainId);assert.notEqual(twin.hash,nextNetwork.hash);
    for(const [name,s] of [['other-net',other],['same-chain',twin]]){
      assert.equal((await m.send({type:'profiles.add',profile:localProfile(name,s)})).ok,true);
      assert.equal(await walletOf(m,name),await addressFor(await keyOf(m,name)));assert.notEqual(await walletOf(m,name),switchedAddress);
    }
    for(const name of ['next-net','other-net','same-chain'])await m.send({type:'profiles.remove',name});
    // A site opened on the built-in profile is offered, and granted, the same address.
    const fx=await siteFixture(switched);
    const log=installFetchMock({...fx.handlers,eth_chainId:()=>'0x'+nextNetwork.chainId.toString(16),pocol_status:()=>({chainId:nextNetwork.chainId,genesisHash:nextNetwork.hash,forkSchedule:[[1,'0']]}),
      eth_getTransactionCount:()=>'0x0',eth_getBlockByNumber:()=>({number:'0x5',baseFeePerGas:'0x3b9aca00'}),eth_estimateGas:()=>'0x5208',eth_sendRawTransaction:([raw])=>H.toHex(C.keccak(H.fromHex(raw)))});
    const opened=await m.send({type:'open',name:N.NETWORK.name,site:fx.site,path:'/'});assert.equal(opened.ok,true,JSON.stringify(opened));
    const port=m.connect('bridge:'+opened.sessionId);
    port.post({seq:1,frame:1,text:JSON.stringify({id:1,kind:'wallet_req',payload:{method:'eth_requestAccounts',params:[]}})});
    const ask=await port.next(x=>x.ask!==undefined && x.q.type==='confirmAccounts');assert.equal(ask.q.address,switchedAddress);
    port.post({answer:ask.ask,ok:true});
    assert.deepEqual((await port.next(x=>x.seq===1)).reply.result,[switchedAddress]);
    // A transaction it signs there comes from that address, for the new chain ID.
    port.post({seq:2,frame:1,text:JSON.stringify({id:2,kind:'wallet_req',payload:{method:'eth_sendTransaction',params:[{to:'0x'+'22'.repeat(20),value:'0x1'}]}})});
    port.post({answer:(await port.next(x=>x.ask!==undefined && x.q.type==='confirmTx')).ask,ok:true});
    const sent=(await port.next(x=>x.seq===2)).reply;assert.equal(sent.error,undefined,JSON.stringify(sent));
    const raws=log.filter(entry=>entry.method==='eth_sendRawTransaction');assert.equal(raws.length,1);
    const signed=Transaction.from(raws[0].params[0]);assert.equal(signed.from.toLowerCase(),switchedAddress);assert.equal(Number(signed.chainId),nextNetwork.chainId);assert.equal(sent.result,signed.hash);
  }finally{rmSync(switched,{recursive:true,force:true});}
});
console.log(JSON.stringify({result:'PASS',passed:count}));
// The real worker schedules delayed storage maintenance. All requested assertions are awaited.
process.exit(0);
