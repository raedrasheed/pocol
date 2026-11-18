// Local website/worker checks. All requests outside each temporary static server
// are aborted, except the fee faucet's two routes, which one context answers with
// stubs. No extension, wallet connection, live RPC, live faucet or signing key is used.
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
        // Launch languages (owner decision 2026-11-16): English and Arabic only; any other request falls back to English.
        check('main offers exactly English and Arabic',await page.locator('button[data-language]').count()===2);
        await page.goto(server.origin+'/?lang=de',{waitUntil:'networkidle'});
        check('main falls back to English for a retired language',await page.evaluate(()=>document.documentElement.lang==='en'&&document.documentElement.dir==='ltr'));
        await page.goto(server.origin+'/about.html?lang=en',{waitUntil:'networkidle'});
        check('guide renders its chapters',await page.locator('#docs-article').innerText().then(text=>text.length>200));
        // The fee faucet page with the faucet blocked (the route above aborts explorer.pocol.net like any other host).
        const address='0x'+'12'.repeat(20);
        await page.goto(server.origin+'/faucet.html?lang=en',{waitUntil:'networkidle'});
        check('faucet page English language, direction and title',await page.evaluate(()=>document.documentElement.lang==='en'
          &&document.documentElement.dir==='ltr'&&document.title==='Get your first fee · PoCol'));
        check('faucet status shows the faucet unreachable when blocked',
          await page.locator('#faucet-status').getAttribute('data-state')==='unreachable');
        await page.fill('#faucet-address',' '+address+' ');
        await page.click('#faucet-submit');
        await page.waitForFunction(()=>document.querySelector('#faucet-form').getAttribute('aria-busy')==='false'
          &&!document.querySelector('#faucet-message').hidden);
        check('faucet claim shows the unreachable message when blocked',
          /could not be reached/.test(await page.locator('#faucet-message').innerText())
          &&await page.locator('#faucet-result').isHidden());
        await page.goto(server.origin+'/faucet.html?lang=ar',{waitUntil:'networkidle'});
        check('faucet page Arabic language, direction and title',await page.evaluate(()=>document.documentElement.lang==='ar'
          &&document.documentElement.dir==='rtl'&&document.title==='احصل على رسومك الأولى · PoCol'));
        // The faucet stubbed with its answers (the real one allows exactly https://pocol.net; the stub allows this origin).
        const stub=await browser.newContext(),claims=[],hash='0x'+'ab'.repeat(32);
        let remaining=7,claimAnswer={status:200,body:{type:'sent',hash,amountWei:'2000000000000000',to:address}};
        try{
          await stub.route('**/*',route=>{
            const request=route.request(),url=new URL(request.url());
            if(url.origin===server.origin)return route.continue();
            const cors={'Access-Control-Allow-Origin':server.origin,'Cache-Control':'no-store'};
            if(request.method()==='OPTIONS')return route.fulfill({status:204,headers:{...cors,
              'Access-Control-Allow-Methods':'GET, POST','Access-Control-Allow-Headers':'Content-Type'}});
            if(url.href==='https://explorer.pocol.net/faucet/status'&&request.method()==='GET')return route.fulfill({status:200,
              headers:cors,contentType:'application/json',body:JSON.stringify({type:'status',chainId:1,amountWei:'2000000000000000',
                faucet:'0x'+'34'.repeat(20),remainingToday:remaining})});
            if(url.href==='https://explorer.pocol.net/faucet/claim'&&request.method()==='POST'){
              claims.push({type:request.headers()['content-type'],body:request.postData()});
              return route.fulfill({status:claimAnswer.status,headers:cors,contentType:'application/json',
                body:claimAnswer.raw??JSON.stringify(claimAnswer.body)});
            }
            blocked++;return route.abort('blockedbyclient');
          });
          const faucet=await stub.newPage(),faucetErrors=[];
          faucet.on('pageerror',error=>faucetErrors.push(String(error)));
          await faucet.goto(server.origin+'/faucet.html?lang=en',{waitUntil:'networkidle'});
          check('faucet status shows the amount in PCOL and the transfers left today',
            await faucet.locator('#faucet-amount').innerText()==='0.002 PCOL'&&await faucet.locator('#faucet-remaining').innerText()==='7');
          await faucet.fill('#faucet-address','0x1234');
          await faucet.click('#faucet-submit');
          check('faucet refuses a malformed address before sending',claims.length===0
            &&/full address/.test(await faucet.locator('#faucet-message').innerText()));
          await faucet.fill('#faucet-address',address);
          await faucet.click('#faucet-submit');
          await faucet.locator('#faucet-result').waitFor({state:'visible'});
          check('faucet claim sends exactly {"address"} as JSON',claims.length===1&&claims[0].type==='application/json'
            &&claims[0].body===JSON.stringify({address}));
          check('faucet success shows the transaction hash, the address and the explorer link',
            await faucet.locator('#faucet-hash').innerText()===hash&&await faucet.locator('#faucet-to').innerText()===address
            &&await faucet.locator('#faucet-link').getAttribute('href')===`https://explorer.pocol.net/#page=Transactions&q=${hash}`);
          // A success answer over the 16 KiB cap is not read, and a transfer may exist: never reported as sent or failed.
          claimAnswer={status:200,raw:JSON.stringify({type:'sent',hash,amountWei:'2000000000000000',to:address,pad:'x'.repeat(20000)})};
          await faucet.goto(server.origin+'/faucet.html?lang=en',{waitUntil:'networkidle'});
          await faucet.fill('#faucet-address',address);
          await faucet.click('#faucet-submit');
          await faucet.waitForFunction(()=>document.querySelector('#faucet-form').getAttribute('aria-busy')==='false'
            &&!document.querySelector('#faucet-message').hidden);
          check('faucet treats an oversized success answer as a transfer that may exist',
            /may already have been sent/.test(await faucet.locator('#faucet-message').innerText())
            &&await faucet.locator('#faucet-result').isHidden());
          claimAnswer={status:502,body:{type:'error',code:'unknown_outcome',message:'<b>may already</b>'}};
          await faucet.goto(server.origin+'/faucet.html?lang=ar',{waitUntil:'networkidle'});
          check('faucet figures line up with their labels in Arabic (right to left)',await faucet.evaluate(()=>{
            const edge=id=>document.getElementById(id).getBoundingClientRect().right;
            return ['amount','remaining'].every(name=>Math.abs(edge(`faucet-${name}`)-edge(`faucet-${name}-label`))<2);
          }));
          await faucet.fill('#faucet-address',address);
          await faucet.click('#faucet-submit');
          await faucet.waitForFunction(()=>document.querySelector('#faucet-form').getAttribute('aria-busy')==='false'
            &&!document.querySelector('#faucet-message').hidden);
          check('faucet unknown outcome says to check the explorer, in Arabic, never the server text',
            await faucet.locator('#faucet-message').innerText()==='ربما أُرسل التحويل بالفعل؛ تحقّق من العنوان في المستكشف قبل إعادة المحاولة.'
            &&await faucet.locator('#faucet-link').getAttribute('href')===`https://explorer.pocol.net/#page=Transactions&q=${address}`
            &&await faucet.locator('main b').count()===0);
          remaining=0;
          await faucet.goto(server.origin+'/faucet.html?lang=en',{waitUntil:'networkidle'});
          check('faucet disables the claim when no transfer is left today',await faucet.locator('#faucet-submit').isDisabled());
          check('faucet page has no uncaught JavaScript errors',faucetErrors.length===0||assert.fail(faucetErrors.join('\n')));
        }finally{await stub.close();}
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
