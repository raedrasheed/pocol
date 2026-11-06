// Local website/worker checks. All requests outside each temporary static server
// are aborted. No extension, wallet connection, live RPC or signing key is used.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,stat} from 'node:fs/promises';
import {dirname,extname,resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const {chromium}=await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../../..');
const mime={'.html':'text/html','.js':'text/javascript','.css':'text/css','.json':'application/json',
  '.wasm':'application/wasm','.svg':'image/svg+xml','.png':'image/png','.ico':'image/x-icon'};
let checks=0,blocked=0;
const check=(name,condition)=>{assert.ok(condition,name);checks++;console.log('PASS '+name);};

async function serverFor(folder){
  const base=resolve(root,folder);
  const server=createServer(async(request,response)=>{
    try{
      const path=resolve(base,'.'+decodeURIComponent(new URL(request.url,'http://localhost').pathname));
      if(path!==base&&!path.startsWith(base+sep))throw Error('path');
      const file=(await stat(path)).isDirectory()?resolve(path,'index.html'):path;
      response.setHeader('Content-Type',mime[extname(file)]||'application/octet-stream');
      response.end(await readFile(file));
    }catch{response.writeHead(404);response.end('Not found');}
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  return {origin:`http://127.0.0.1:${server.address().port}`,close:()=>new Promise(resolve=>server.close(resolve))};
}

const browser=await chromium.launch({executablePath:process.env.POCOL_CHROMIUM || '/usr/bin/chromium',headless:true});
const sites=[['deploy/websites/pocol.net','main'],['deploy/websites/explorer.pocol.net','explorer'],
  ['prototypes/pocol-studio/dist','studio'],['deploy/websites/ide.pocol.net','ide']];
try{
  for(const [folder,name] of sites){
    const server=await serverFor(folder),context=await browser.newContext();
    try{
      await context.route('**/*',route=>{
        const url=new URL(route.request().url());
        if(url.origin===server.origin)return route.continue();
        blocked++;return route.abort('blockedbyclient');
      });
      await context.routeWebSocket('**/*',socket=>{blocked++;socket.close();});
      const page=await context.newPage(),errors=[];
      page.on('pageerror',error=>errors.push(String(error)));
      await page.goto(server.origin,{waitUntil:'networkidle',timeout:20000});
      check(name+' renders PoCol',/PoCol/i.test(await page.locator('body').innerText()));
      if(name==='main'){
        await page.click('#language-trigger');
        await page.click('button[data-language="ar"]');
        check('main Arabic language and direction',await page.evaluate(()=>document.documentElement.lang==='ar'&&document.documentElement.dir==='rtl'));
        await page.goto(server.origin+'/about.html?lang=en',{waitUntil:'networkidle'});
        check('guide renders its chapters',await page.locator('#docs-article').innerText().then(text=>text.length>200));
      }
      if(name==='ide'){
        const result=await page.evaluate(async()=>{
          function client(path){
            const worker=new Worker(path);let next=0;
            return {worker,request(payload){return new Promise((resolve,reject)=>{
              const id=++next,deadline=setTimeout(()=>{worker.removeEventListener('message',receive);reject(Error('Worker deadline'));},30000);
              function receive(event){if(event.data.id!==id)return;clearTimeout(deadline);worker.removeEventListener('message',receive);resolve(event.data);}
              worker.addEventListener('message',receive);worker.postMessage({id,...payload});
            });}};
          }
          const compiler=client('/compiler/worker.js'),sandbox=client('/assets/sandbox.worker-DKPDIOr-.js');
          try{
            const input={language:'Solidity',sources:{'Audit.sol':{content:'pragma solidity ^0.8.38; contract Audit { function answer() external pure returns(uint256) { return 42; } }'}},settings:{evmVersion:'cancun',outputSelection:{'*':{'*':['abi','evm.bytecode','evm.deployedBytecode']}}}};
            const compiled=await compiler.request({input});
            if(!compiled.result?.success)throw Error(JSON.stringify(compiled));
            const contract=compiled.result.contracts.find(c=>c.name==='Audit');
            const missing=await compiler.request({input:{...input,sources:{'Audit.sol':{content:'pragma solidity ^0.8.38; import "https://example.invalid/Outside.sol";'}}}});
            const info=await sandbox.request({method:'info'});
            const refused=await sandbox.request({method:'constructor'});
            const deployed=await sandbox.request({method:'deploy',args:[contract,[],{}]});
            if(!deployed.result?.contractAddress)throw Error(JSON.stringify(deployed));
            const called=await sandbox.request({method:'call',args:[deployed.result.contractAddress,contract,'answer',[],{}]});
            return {compiler:compiled.result.compiler,missing:missing.result,info:info.result,refused:refused.error,deployed:deployed.result,called:called.result};
          }finally{compiler.worker.terminate();sandbox.worker.terminate();}
        });
        check('pinned IDE Solidity compiler runs',result.compiler.startsWith('0.8.38+'));
        check('remote Solidity imports refused',result.missing?.success===false&&result.missing.diagnostics.some(d=>d.severity==='error'));
        check('sandbox identifies local simulation',result.info?.label==='Local sandbox');
        check('sandbox denies arbitrary method dispatch',/Unsupported local sandbox operation/.test(result.refused?.message));
        check('sandbox deploy succeeds locally',result.deployed.status===1&&result.deployed.environment==='sandbox');
        check('sandbox pure call returns exactly 42',result.called?.returnData==='0x'+'0'.repeat(62)+'2a'&&result.called.environment==='sandbox');
      }
      check(name+' has no uncaught JavaScript errors',errors.length===0||assert.fail(errors.join('\n')));
      check(name+' registers no service worker',context.serviceWorkers().length===0);
    }finally{await context.close();await server.close();}
  }
  console.log(JSON.stringify({result:'PASS',checks,browser:browser.version(),externalRequestsBlocked:blocked,extensionLoaded:false,liveRpcRequests:0}));
}finally{await browser.close();}
