// Responsive, link and header checks of every page of pocol.net, studio.pocol.net and ide.pocol.net (stage-2 item e,
// 2026-11-22). Local only: each document root is served on loopback with the headers of its own .htaccess (pocol.net's
// Content-Security-Policy and ErrorDocument 404, Studio's CSP); every request outside the test servers is aborted.
//
// Each page is loaded at 390x844 and 1366x900, in English and Arabic (pocol.net and Studio; the IDE is English only,
// owner decision 2026-11-16), and must:
//   - have no horizontal scroll (scrollWidth <= innerWidth) and, in Arabic, lang="ar" dir="rtl";
//   - raise no page error and no Content-Security-Policy violation under the site's own CSP;
// and every link it shows must resolve:
//   - a link within the three sites (relative, or https://pocol.net, studio.pocol.net, ide.pocol.net) names a file of
//     that snapshot, and a #fragment names an element of the target page once it has rendered;
//   - an explorer.pocol.net link names a file of the explorer snapshot;
//   - a GitHub link to raedrasheed/pocol at a pinned commit names a path that exists at that commit in the local Git
//     history (`git cat-file -e`), or is reported as not checked when the commit is not in the local history;
//   - any other external link is listed, never fetched.
//
//   PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs POCOL_CHROMIUM=/path/to/chromium node deploy/websites/tests/site_pages.mjs
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,readdir,stat} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {dirname,extname,resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const {chromium}=await import(process.env.PLAYWRIGHT_MODULE||'playwright');
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../../..');
const mime={'.html':'text/html; charset=utf-8','.js':'text/javascript','.css':'text/css','.json':'application/json','.webmanifest':'application/manifest+json',
  '.wasm':'application/wasm','.svg':'image/svg+xml','.png':'image/png','.ico':'image/x-icon','.txt':'text/plain; charset=utf-8','.xml':'application/xml'};
const SITES={
  main:{folder:'deploy/websites/pocol.net',host:'pocol.net',langs:['en','ar']},
  studio:{folder:'prototypes/pocol-studio/dist',host:'studio.pocol.net',langs:['en','ar']},
  ide:{folder:'deploy/websites/ide.pocol.net',host:'ide.pocol.net',langs:['en']},
};
const EXPLORER='deploy/websites/explorer.pocol.net';
const VIEWPORTS=[[390,844],[1366,900]];
const failures=[],notes=[];let checks=0;
const check=(name,ok,detail='')=>{checks++;if(!ok)failures.push(name+(detail?' — '+detail:''));};

/** The headers an .htaccess sets with `Header always set` / `Header set`, and its ErrorDocument 404. */
async function htaccess(folder){
  let text='';try{text=await readFile(resolve(root,folder,'.htaccess'),'utf8');}catch{return{headers:{},notFound:null};}
  const headers={};
  for(const m of text.matchAll(/^\s*Header (?:always )?set ([A-Za-z-]+) "([^"]*)"/gm))if(m[1]!=='Cache-Control')headers[m[1]]=m[2];
  const notFound=text.match(/^\s*ErrorDocument 404 (\/\S+)/m)?.[1]||null;
  return{headers,notFound};
}
async function exists(path){try{return(await stat(path)).isFile();}catch{return false;}}
async function serve(folder){
  const base=resolve(root,folder),{headers,notFound}=await htaccess(folder);
  const server=createServer(async(request,response)=>{
    for(const [k,v] of Object.entries(headers))response.setHeader(k,v);
    try{
      const path=resolve(base,'.'+decodeURIComponent(new URL(request.url,'http://localhost').pathname));
      if(path!==base&&!path.startsWith(base+sep))throw Error('path');
      const file=(await stat(path)).isDirectory()?resolve(path,'index.html'):path;
      if(/(^|\/)\.[^/]+$/.test(file))throw Error('hidden');
      response.setHeader('Content-Type',mime[extname(file)]||'application/octet-stream');
      response.end(await readFile(file));
    }catch{
      response.statusCode=404;
      if(notFound&&await exists(resolve(base,'.'+notFound))){response.setHeader('Content-Type',mime['.html']);response.end(await readFile(resolve(base,'.'+notFound)));}
      else response.end('Not found');
    }
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  return{base,origin:`http://127.0.0.1:${server.address().port}`,headers,close:()=>new Promise(r=>server.close(r))};
}

const LEGACY=new Set([...(await readFile(resolve(root,'deploy/websites/tools/site_identity.py'),'utf8'))
  .match(/^LEGACY_NATIVE = frozenset\(\{([^}]*)\}\)/m)[1].matchAll(/'([^']+)'/g)].map(m=>m[1]));
const KEPT_OUT=JSON.parse(await readFile(resolve(root,'deploy/websites/site-identity.json'),'utf8')).keptOut;
const servers={};
for(const [key,site] of Object.entries(SITES))servers[key]=await serve(site.folder);
const hostOf=Object.fromEntries(Object.entries(SITES).map(([k,s])=>[s.host,k]));
const browser=await chromium.launch({executablePath:process.env.POCOL_CHROMIUM||'/usr/bin/chromium',headless:true});
let blocked=0;
const gitHas=new Map();
function atCommit(sha,path){
  const key=sha+':'+path;
  if(!gitHas.has(key)){
    let state='missing';
    try{execFileSync('git',['-C',root,'cat-file','-e',sha+'^{commit}'],{stdio:'ignore',timeout:20000});
      try{execFileSync('git',['-C',root,'cat-file','-e',`${sha}:${path.replace(/\/$/,'')}`],{stdio:'ignore',timeout:20000});state='ok';}catch{state='missing';}
    }catch{state='unknown-commit';}
    gitHas.set(key,state);
  }
  return gitHas.get(key);
}

async function newContext(lang){
  const context=await browser.newContext();
  await context.route('**/*',route=>{
    const url=new URL(route.request().url());
    if(Object.values(servers).some(s=>s.origin===url.origin))return route.continue();
    blocked++;return route.abort('blockedbyclient');
  });
  await context.routeWebSocket('**/*',socket=>socket.close());
  await context.addInitScript(([lang])=>{
    window.__csp=[];document.addEventListener('securitypolicyviolation',e=>window.__csp.push(e.violatedDirective+' '+e.blockedURI));
    // Studio keeps its language in its settings (app.js SETTINGS); pocol.net takes ?lang=.
    if(location.port&&lang==='ar'&&!localStorage.getItem('pocol-studio-settings-v1'))
      try{localStorage.setItem('pocol-studio-settings-v1',JSON.stringify({lang:'ar',theme:'dark'}));}catch{}
  },[lang]);
  return context;
}

async function pagesOf(key){
  if(key!=='main')return['/'];
  const files=(await readdir(servers.main.base)).filter(f=>f.endsWith('.html')).sort();
  return files.map(f=>f==='index.html'?'/':'/'+f);
}

const renderedIds=new Map();
async function idsOf(context,key,path){
  const k=key+path;
  if(!renderedIds.has(k)){
    const page=await context.newPage();
    await page.goto(servers[key].origin+path,{waitUntil:'networkidle',timeout:30000});
    renderedIds.set(k,new Set(await page.evaluate(()=>[...document.querySelectorAll('[id]')].map(e=>e.id))));
    await page.close();
  }
  return renderedIds.get(k);
}

async function checkLink(context,key,pagePath,href,ids){
  if(!href||href.startsWith('javascript:'))return check(`${key}${pagePath}: link ${href}`,false,'empty or javascript link');
  if(/^(mailto|chrome):/.test(href)){notes.push(`not fetched: ${href}`);return;}
  const url=new URL(href,servers[key].origin+pagePath);
  let target=null;
  if(Object.values(servers).some(s=>s.origin===url.origin))target=Object.keys(servers).find(k=>servers[k].origin===url.origin);
  else if(url.protocol==='https:'&&hostOf[url.hostname])target=hostOf[url.hostname];
  if(target){
    let path=decodeURIComponent(url.pathname);if(path.endsWith('/'))path+='index.html';
    const file=resolve(servers[target].base,'.'+path);
    // The retired network's package that the pages link, labelled as such, until the signed release (site_identity.py
    // LEGACY_NATIVE): not in the snapshot or its SHA256SUMS.txt, kept on the host (tools/host_files.py KEEP).
    const legacy=target==='main'&&LEGACY.has(path.replace(/^\/downloads\//,''))&&path.startsWith('/downloads/');
    if(legacy){notes.push(`retired package kept on the host (LEGACY_NATIVE): ${href}`);return;}
    // A signed release file kept out of Git (site-identity.json keptOut; its SHA256SUMS.txt line holds its digest).
    if((KEPT_OUT[SITES[target].host]||[]).includes(path.slice(1))){notes.push(`kept-out release file, not in Git: ${href}`);return;}
    const ok=await exists(file);
    check(`${key}${pagePath}: ${href} names a file of ${SITES[target].host}`,ok,file);
    if(ok&&url.hash&&url.hash.length>1&&!url.hash.startsWith('#page=')){
      const anchor=decodeURIComponent(url.hash.slice(1));
      const same=target===key&&(url.pathname===pagePath||(pagePath==='/'&&url.pathname==='/index.html'));
      const set=same?ids:await idsOf(context,target,url.pathname==='/index.html'?'/':url.pathname);
      check(`${key}${pagePath}: ${href} names an element #${anchor}`,set.has(anchor));
    }
    return;
  }
  if(url.protocol==='https:'&&url.hostname==='explorer.pocol.net'){
    const path=url.pathname==='/'?'/index.html':url.pathname;
    return check(`${key}${pagePath}: ${href} names a file of the explorer snapshot`,await exists(resolve(root,EXPLORER,'.'+path)));
  }
  const gh=url.hostname==='github.com'&&url.pathname.match(/^\/raedrasheed\/pocol\/(?:blob|tree)\/([0-9a-f]{40})\/?(.*)$/);
  if(gh){
    const state=atCommit(gh[1],decodeURIComponent(gh[2])||'.');
    if(state==='unknown-commit')notes.push(`not checked (commit not in local history): ${href}`);
    else check(`${key}${pagePath}: ${href} names a path at its commit`,state==='ok');
    return;
  }
  if(url.hostname==='github.com'&&/^\/raedrasheed\/pocol\/?$/.test(url.pathname)){notes.push(`repository home, not fetched: ${href}`);return;}
  if(url.hostname==='github.com'&&url.pathname.startsWith('/raedrasheed/pocol'))return check(`${key}${pagePath}: ${href} pins a commit`,false,'a repository link that pins no commit');
  notes.push(`external, not fetched: ${href}`);
}

try{
  for(const key of Object.keys(SITES)){
    for(const lang of SITES[key].langs){
      const context=await newContext(lang);
      try{
        for(const path of await pagesOf(key)){
          for(const [width,height] of VIEWPORTS){
            const page=await context.newPage(),errors=[];
            page.on('pageerror',e=>errors.push(String(e)));
            await page.setViewportSize({width,height});
            const q=key==='main'?(path.includes('?')?'&':'?')+'lang='+lang:'';
            const response=await page.goto(servers[key].origin+path+q,{waitUntil:'networkidle',timeout:30000});
            const name=`${key}${path} ${lang} ${width}px`;
            check(name+': HTTP status',(response?.status()??0)===(path==='/404.html'?200:200));
            const state=await page.evaluate(()=>{
              const doc=document.documentElement,wide=[];
              if(doc.scrollWidth>window.innerWidth)for(const e of document.body.querySelectorAll('*')){const r=e.getBoundingClientRect();
                if((r.right>window.innerWidth+1||r.left<-1)&&getComputedStyle(e).position!=='fixed'&&r.width>0)wide.push((e.id?'#'+e.id:e.tagName.toLowerCase()+'.'+[...e.classList].join('.'))+' '+Math.round(r.left)+'..'+Math.round(r.right));}
              return{lang:doc.lang,dir:doc.dir,scroll:doc.scrollWidth,inner:window.innerWidth,wide:wide.slice(0,5),csp:window.__csp};
            });
            check(name+': no horizontal scroll',state.scroll<=state.inner,`${state.scroll} > ${state.inner}: ${state.wide.join(', ')}`);
            if(lang==='ar')check(name+': Arabic, right to left',state.lang==='ar'&&state.dir==='rtl',`${state.lang} ${state.dir}`);
            check(name+': no page error',errors.length===0,errors.join(' | '));
            check(name+': no CSP violation',state.csp.length===0,state.csp.join(' | '));
            if(width===1366){
              const ids=new Set(await page.evaluate(()=>[...document.querySelectorAll('[id]')].map(e=>e.id)));
              const hrefs=[...new Set(await page.evaluate(()=>[...document.querySelectorAll('a[href]')].map(a=>a.getAttribute('href'))))];
              for(const href of hrefs)await checkLink(context,key,path,href,ids);
            }
            await page.close();
          }
        }
      }finally{await context.close();}
    }
  }
  // pocol.net's ErrorDocument: a missing path is answered with 404 and the bilingual 404 page, whose assets load.
  const context=await newContext('ar'),page=await context.newPage(),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  const missing=await page.goto(servers.main.origin+'/deep/missing/page.html?lang=ar',{waitUntil:'networkidle'});
  const nf=await page.evaluate(()=>({title:document.querySelector('#not-found-heading')?.textContent,dir:document.documentElement.dir,
    styled:getComputedStyle(document.body).backgroundColor!=='rgba(0, 0, 0, 0)',csp:window.__csp}));
  check('main: a missing path answers 404 with the 404 page',missing.status()===404&&nf.title==='الصفحة غير موجودة'&&nf.dir==='rtl'&&nf.styled,JSON.stringify(nf));
  check('main: the 404 page has no errors or CSP violations at any depth',errors.length===0&&nf.csp.length===0);
  await context.close();
  check('pocol.net serves its CSP from .htaccess',/default-src 'self'/.test(servers.main.headers['Content-Security-Policy']||''));
  check('studio serves its CSP from .htaccess',/default-src 'self'/.test(servers.studio.headers['Content-Security-Policy']||''));
}finally{
  await browser.close();
  for(const s of Object.values(servers))await s.close();
}
for(const n of [...new Set(notes)].sort())console.log('NOTE '+n);
for(const f of failures)console.log('FAIL '+f);
console.log(JSON.stringify({result:failures.length?'FAIL':'PASS',checks,failures:failures.length,externalRequestsBlocked:blocked,notes:new Set(notes).size}));
assert.equal(failures.length,0,failures.length+' failed checks');
