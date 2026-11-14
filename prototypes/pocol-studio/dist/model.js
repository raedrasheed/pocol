export const LIMITS={files:256,file:1048576,total:4194304};
export const encoder=new TextEncoder();
export function escapeHTML(value){return String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));}
export function uid(){if(crypto.randomUUID)return crypto.randomUUID();const b=crypto.getRandomValues(new Uint8Array(16));b[6]=(b[6]&15)|64;b[8]=(b[8]&63)|128;const x=[...b].map(n=>n.toString(16).padStart(2,'0')).join('');return x.slice(0,8)+'-'+x.slice(8,12)+'-'+x.slice(12,16)+'-'+x.slice(16,20)+'-'+x.slice(20);}
export function validPath(p){return typeof p==='string'&&p.length<=256&&/^\/[A-Za-z0-9._~/-]+$/.test(p)&&p.slice(1).split('/').every(s=>s&&s!=='.'&&s!=='..');}
// Manifest MIME types (the extension's lib/site.js MIME and pocold site.rs): Sites W5 (owner answer Q3) adds favicon.ico
// (id 12) and site.webmanifest (id 13).
const EXT={html:'text/html',htm:'text/html',css:'text/css',js:'application/javascript',mjs:'application/javascript',json:'application/json',png:'image/png',jpg:'image/jpeg',jpeg:'image/jpeg',webp:'image/webp',svg:'image/svg+xml',woff2:'font/woff2',txt:'text/plain',ico:'image/x-icon',webmanifest:'application/manifest+json'};
export function mimeFor(path){return EXT[path.split('.').pop().toLowerCase()]||null;}
export function encode64(bytes){let s='';for(let i=0;i<bytes.length;i+=8192)s+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(s);}
export function decode64(s){if(typeof s!=='string'||s.length>Math.ceil(LIMITS.file/3)*4+4||!/^([A-Za-z0-9+/]{4})*([A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(s))throw Error('Invalid file encoding');const raw=atob(s);return Uint8Array.from(raw,c=>c.charCodeAt(0));}
// Sites W5: the hard-deny list, applied whatever the user selects (the extension's lib/publish-input.js and pocold
// site.rs apply the same rules; tests/model.test.mjs checks the shared vectors). Publishing is permanent, so a folder
// import drops these with their reason listed, and validateFiles refuses them loudly.
export const DENY_REASONS=['dotfile','config','secret-key','database','backup','server-api','checksums'];
export function denyReason(path){
 const segs=String(path).replace(/^\/+/,'').split('/'),base=segs[segs.length-1].toLowerCase();
 if(segs.some(s=>s.startsWith('.')))return'dotfile';
 if(segs[0].toLowerCase()==='api'&&segs.length>1)return'server-api';
 if(base.startsWith('config.'))return'config';
 if(/\.(key|pem)$/.test(base))return'secret-key';
 if(/\.sql$/.test(base))return'database';
 if(/\.bak[^/]*$/.test(base))return'backup';
 if(base.startsWith('sha256sums'))return'checksums';
 return null;
}
/// A folder import, in this order: the hard-deny filter, then the user's explicit exclusions, then the PoCol limits.
/// `entries` are [{path,size}]. Returns {kept, denied:[{path,reason}], excluded:[path], unsupported:[path], error}: an
/// unsupported file is refused loudly (error 'filePath') until the user excludes it.
export function planImport(entries,{exclude=[]}={}){
 const skip=new Set(exclude),kept=[],denied=[],excluded=[];
 for(const e of entries){const reason=denyReason(e.path);if(reason){denied.push({path:e.path,reason});continue;}if(skip.has(e.path)){excluded.push(e.path);continue;}kept.push(e);}
 const unsupported=kept.filter(e=>!validPath(e.path)||!mimeFor(e.path)).map(e=>e.path),total=kept.reduce((n,e)=>n+e.size,0);
 const error=!kept.length||kept.length>LIMITS.files?'fileCount':unsupported.length?'filePath':kept.some(e=>e.size>LIMITS.file)?'fileSize':total>LIMITS.total?'totalSize':null;
 return{kept,denied,excluded,unsupported,error};
}
/// What a folder import left out, kept with the project so the publish review lists it (bounded).
export function importReport(plan){return{denied:plan.denied.slice(0,500).map(d=>({path:String(d.path).slice(0,300),reason:DENY_REASONS.includes(d.reason)?d.reason:'dotfile'})),excluded:plan.excluded.slice(0,500).map(p=>String(p).slice(0,300)),more:Math.max(0,plan.denied.length-500)+Math.max(0,plan.excluded.length-500)};}
export function validateFiles(files,entry='/index.html'){
 if(!Array.isArray(files)||files.length<1||files.length>LIMITS.files)throw Error('fileCount');
 const seen=new Set();let total=0;
 for(const f of files){if(denyReason(f.path))throw Error('fileDenied');if(!validPath(f.path)||!mimeFor(f.path))throw Error('filePath');if(seen.has(f.path))throw Error('duplicatePath');seen.add(f.path);if(!(f.data instanceof Uint8Array)||f.data.length>LIMITS.file)throw Error('fileSize');total+=f.data.length;}
 if(total>LIMITS.total)throw Error('totalSize');if(!files.some(f=>f.path===entry&&mimeFor(f.path)==='text/html'))throw Error('entryMissing');return{count:files.length,total};
}
export const TEMPLATE_IDS=['personal','business','product'];
const defaults={
 ar:{personal:{name:'مساحتي الأولى',brand:'اسمك هنا',eyebrow:'أفكار • تجارب • أثر',title:'مساحة لأفكار\nتستحق أن تُرى.',description:'أشارك هنا ما أتعلمه وما أبنيه. مرحبًا بك في مساحتي المستقلة على شبكة PoCol.',button:'تعرّف عليّ',sections:[{type:'text',title:'أهلًا بك في مساحتي.',body:'أؤمن أن الأفكار الجيدة تبدأ بالفضول، وتكبر بالمشاركة. هذه الصفحة بداية حكاية جديدة.'},{type:'features',title:'ما أعمل عليه',body:'تعلّم مستمر|أبحث عن أسئلة جديدة وأشارك ما أكتشفه.\nبناء مفيد|أحوّل الأفكار إلى تجارب بسيطة وقابلة للاستخدام.\nتعاون مفتوح|أصنع مساحة للمعرفة والعمل المشترك.'},{type:'contact',title:'لنصنع شيئًا معًا.',body:'كل فكرة تستحق محادثة. يسعدني التواصل ومشاركة التجارب.'}]},business:{name:'صفحة المؤسسة',brand:'أفق',eyebrow:'نحو أثر يدوم',title:'أفكار طموحة.\nحلول واضحة.',description:'نساعد المؤسسات على تحويل رؤيتها إلى تجارب رقمية مفيدة، من الفكرة الأولى إلى التفاصيل الأخيرة.',button:'اكتشف خدماتنا',sections:[{type:'features',title:'كيف نساعدك؟',body:'استراتيجية|خطة واضحة مبنية على احتياجاتك.\nتصميم|تجارب تجعل الاستخدام أسهل.\nتطوير|أدوات موثوقة تنمو معك.'},{type:'text',title:'نؤمن بقوة العمل المشترك.',body:'نبدأ بالاستماع، ونبني بالتعاون، ونقيس النجاح بالأثر الذي نتركه.'},{type:'contact',title:'ابدأ مشروعك القادم.',body:'أخبرنا عن فكرتك، ولنعمل على الخطوة الأولى.'}]},product:{name:'إطلاق منتج',brand:'مدار',eyebrow:'أقل تعقيدًا. أكثر تركيزًا.',title:'أفكارك تستحق\nمساحة أفضل.',description:'أداة بسيطة لتنظيم ما يهمّك، لتقضي وقتًا أقل في الترتيب ووقتًا أكثر في الإنجاز.',button:'اكتشف مدار',sections:[{type:'features',title:'مصمّم ليبقى بسيطًا.',body:'كل شيء في مكانه|اجمع أفكارك ومشاريعك في مساحة واضحة.\nإيقاعك الخاص|نظّم يومك بالطريقة التي تناسبك.\nبداية سهلة|ابدأ سريعًا وركّز على ما تحب.'},{type:'text',title:'من الفكرة إلى الخطوة التالية.',body:'لا تحتاج إلى نظام معقد لتبدأ. مساحة هادئة، فكرة واضحة، وخطوة صغيرة كل يوم.'},{type:'contact',title:'كن جزءًا من البداية.',body:'تابع تحديثاتنا واكتشف ما نبنيه قريبًا.'}]}},
 en:{personal:{name:'My first space',brand:'Your name',eyebrow:'IDEAS • EXPERIENCES • IMPACT',title:'A home for ideas\nworth sharing.',description:'A place for what I learn and what I build. Welcome to my independent space on the PoCol network.',button:'Get to know me',sections:[{type:'text',title:'A little about me.',body:'I believe good ideas begin with curiosity and grow through collaboration. This page is the beginning of a new story.'},{type:'features',title:'What I’m working on',body:'Always learning|Finding new questions and sharing discoveries.\nBuilding useful things|Turning ideas into simple, thoughtful experiences.\nWorking together|Making room for knowledge and collaboration.'},{type:'contact',title:'Let’s build something together.',body:'Every idea deserves a conversation. I would love to exchange ideas and experiences.'}]},business:{name:'Our organization',brand:'Horizon',eyebrow:'BUILDING LASTING IMPACT',title:'Ambitious ideas.\nClear solutions.',description:'We help organizations turn their vision into useful digital experiences, from the first idea to the final detail.',button:'Explore our services',sections:[{type:'features',title:'How can we help?',body:'Strategy|A clear plan shaped around your needs.\nDesign|Experiences that make things easier.\nDevelopment|Reliable tools that grow with you.'},{type:'text',title:'Better, together.',body:'We start by listening, build through collaboration, and measure success by the impact we make.'},{type:'contact',title:'Start your next project.',body:'Tell us about your idea. Let’s take the first step.'}]},product:{name:'Product launch',brand:'Orbit',eyebrow:'LESS FRICTION. MORE FOCUS.',title:'Your ideas deserve\na better space.',description:'A simple tool for organizing what matters. Spend less time arranging things, and more time making progress.',button:'Discover Orbit',sections:[{type:'features',title:'Made to stay simple.',body:'Everything together|Keep ideas and projects in one clear space.\nYour own rhythm|Organize your day in the way that suits you.\nAn easy beginning|Start quickly and focus on what you love.'},{type:'text',title:'From an idea to the next step.',body:'You do not need a complex system to begin. A calm space, a clear idea, and one small step each day.'},{type:'contact',title:'Be part of the beginning.',body:'Follow our updates and discover what we are building next.'}]}}
};
export function newProject(template='personal',lang='en'){
 if(!TEMPLATE_IDS.includes(template))throw Error('template');const d=structuredClone(defaults[lang==='en'?'en':'ar'][template]);return{id:uid(),template,lang:lang==='en'?'en':'ar',mode:'visual',name:d.name,brand:d.brand,eyebrow:d.eyebrow,title:d.title,description:d.description,button:d.button,color:template==='business'?'#147d8d':template==='product'?'#6743c3':'#187b68',surface:'#f7f5ef',logo:null,sections:d.sections.map(s=>({...s,id:uid()})),created:Date.now(),updated:Date.now(),published:null};
}
// Upgrade only the untouched starter from releases whose initial content was Arabic.
export function upgradeLegacyStarter(p){
 if(p.mode!=='visual'||p.template!=='personal'||p.lang!=='ar'||p.published||p.logo)return p;
 const original=newProject('personal','ar');
 const fields=['name','brand','eyebrow','title','description','button','color','surface'];
 if(fields.some(k=>p[k]!==original[k])||p.sections.length!==original.sections.length)return p;
 if(p.sections.some((s,i)=>['type','title','body'].some(k=>s[k]!==original.sections[i][k])))return p;
 const english=newProject('personal','en');
 return{...p,lang:'en',...Object.fromEntries(fields.map(k=>[k,english[k]])),sections:p.sections.map((s,i)=>({...s,title:english.sections[i].title,body:english.sections[i].body}))};
}
export function safeColor(c,fallback){return /^#[\da-f]{6}$/i.test(c||'')?c:fallback;}
export function validateProject(p){
 if(!p||typeof p!=='object'||!['visual','files'].includes(p.mode))throw Error('projectInvalid');
 const o={id:typeof p.id==='string'&&p.id.length<100?p.id:uid(),mode:p.mode,name:String(p.name||'Untitled').slice(0,100),lang:p.lang==='en'?'en':'ar',created:Number(p.created)||Date.now(),updated:Date.now(),published:null};
 if(p.published&&/^0x[0-9a-f]{40}$/i.test(p.published.site))o.published={site:p.published.site,version:p.published.version==null?null:String(p.published.version).slice(0,30),publishedBlock:p.published.publishedBlock??null,attached:p.published.attached===true};
 if(p.mode==='files'){if(!Array.isArray(p.files)||p.files.length>256)throw Error('fileCount');o.files=p.files.map(f=>({path:f.path,base64:f.base64}));o.entry=typeof p.entry==='string'?p.entry:'/index.html';validateFiles(o.files.map(f=>({path:f.path,data:decode64(f.base64)})),o.entry);if(p.importReport&&typeof p.importReport==='object')o.importReport=importReport({denied:Array.isArray(p.importReport.denied)?p.importReport.denied.filter(d=>d&&typeof d.path==='string'):[],excluded:Array.isArray(p.importReport.excluded)?p.importReport.excluded.filter(x=>typeof x==='string'):[]});return o;}
 o.template=TEMPLATE_IDS.includes(p.template)?p.template:'personal';for(const k of ['brand','eyebrow','title','description','button'])o[k]=String(p[k]||'').slice(0,k==='description'?5000:500);o.color=safeColor(p.color,'#187b68');o.surface=safeColor(p.surface,'#f7f5ef');
 o.logo=null;if(p.logo){const d=decode64(p.logo.base64);if(d.length>200*1024||!['image/png','image/jpeg','image/webp'].includes(p.logo.mime))throw Error('logoSize');o.logo={base64:p.logo.base64,mime:p.logo.mime};}
 if(!Array.isArray(p.sections)||p.sections.length>20)throw Error('sectionsLimit');o.sections=p.sections.map(s=>({id:uid(),type:['text','features','contact'].includes(s.type)?s.type:'text',title:String(s.title||'').slice(0,500),body:String(s.body||'').slice(0,12000)}));return o;
}
export function generateHTML(p){
 const h=escapeHTML,c=safeColor(p.color,'#187b68'),bg=safeColor(p.surface,'#f7f5ef'),ar=p.lang==='ar';
 const logo=p.logo?`<img class="logo" alt="" src="data:${p.logo.mime};base64,${p.logo.base64}">`:`<span class="brand-symbol" aria-hidden="true">✳</span>`;
 const sections=p.sections.map((s,i)=>`<section class="section ${s.type}" id="section-${i}"><span class="section-number">${String(i+1).padStart(2,'0')}</span><h2>${h(s.title)}</h2>${s.type==='features'?`<div class="feature-grid">${s.body.split('\n').filter(Boolean).slice(0,6).map((line,j)=>{const [title,...desc]=line.split('|');return`<article><span class="feature-dot">${String(j+1).padStart(2,'0')}</span><h3>${h(title)}</h3><p>${h(desc.join('|'))}</p></article>`;}).join('')}</div>`:`<p>${h(s.body).replace(/\n/g,'<br>')}</p>`}</section>`).join('');
 return`<!doctype html><html lang="${ar?'ar':'en'}" dir="${ar?'rtl':'ltr'}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="description" content="${h(p.description)}"><title>${h(p.name)}</title><style>*{box-sizing:border-box}html{scroll-behavior:smooth}body{margin:0;background:${bg};color:#172823;font-family:${ar?'Tahoma,Arial':'Arial,Helvetica'},sans-serif;line-height:1.8}a{color:inherit;text-decoration:none}header,main,footer{max-width:1160px;margin:auto;padding-inline:7%}.site-header{height:92px;display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid #17282322}.brand{display:flex;gap:12px;align-items:center;font-weight:700}.brand-symbol{color:${c};font-size:38px;line-height:1}.logo{height:42px;max-width:120px;object-fit:contain}.nav{font-size:12px;color:#50605a}.hero{padding:94px 0 88px;display:grid;grid-template-columns:1.3fr .7fr;gap:38px;align-items:center;min-height:570px}.eyebrow{color:${c};font-size:11px;letter-spacing:${ar?'0':'2px'};font-weight:700}.hero h1{font-size:clamp(35px,6vw,72px);line-height:${ar?'1.55':'1.12'};letter-spacing:${ar?'0':'-3px'};margin:20px 0}.hero p{font-size:16px;color:#5d6a63;max-width:480px}.cta{display:inline-block;background:${c};color:white;padding:12px 24px;border-radius:5px;font-size:13px;margin-top:20px}.art{aspect-ratio:1;position:relative;display:grid;place-items:center;max-width:310px;width:100%;margin:auto}.art::before,.art::after{content:'';position:absolute;width:81%;height:81%;border:1px solid ${c}55;border-radius:50%;transform:rotate(45deg) scaleX(.55)}.art::after{transform:rotate(-45deg) scaleX(.55)}.art-core{width:42%;height:42%;background:${c};border-radius:24% 24% 24% 4%;transform:rotate(45deg);box-shadow:22px 22px 0 ${c}18;position:relative}.art-core:after{content:'';position:absolute;inset:24%;border:1px solid #ffffff88;border-radius:50%}.art-dot{width:10px;height:10px;background:${c};border-radius:50%;position:absolute;top:13%;left:24%}.section{padding:64px 0;border-top:1px solid #17282322;position:relative}.section-number{color:${c};font-size:11px;letter-spacing:2px}.section h2{font-size:29px;line-height:1.5;margin:13px 0 22px}.section>p{max-width:690px;color:#5d6a63;font-size:16px}.feature-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:30px}.feature-grid article{padding-top:24px}.feature-dot{display:inline-grid;place-items:center;width:36px;height:36px;border:1px solid ${c}55;color:${c};border-radius:50%;font-size:11px}.feature-grid h3{font-size:19px}.feature-grid p{color:#5d6a63;font-size:14px}.contact{padding-bottom:80px}.site-footer{display:flex;justify-content:space-between;gap:20px;border-top:1px solid #17282322;padding-block:24px;font-size:11px;color:#65736c}.site-footer b{font-weight:400;color:${c}}@media(max-width:650px){.site-header{height:76px}.hero{grid-template-columns:1fr;padding:52px 0;gap:20px}.hero h1{font-size:42px;letter-spacing:0}.hero p{font-size:14px}.art{max-width:190px}.feature-grid{grid-template-columns:1fr;gap:12px}.section{padding:40px 0}.section h2{font-size:25px}.site-footer{flex-wrap:wrap}.nav{font-size:10px}}</style></head><body><header class="site-header"><a class="brand" href="#">${logo}<span>${h(p.brand)}</span></a><span class="nav">${ar?'مساحة مستقلة على الشبكة':'An independent space on the network'}</span></header><main><section class="hero"><div><span class="eyebrow">${h(p.eyebrow)}</span><h1>${h(p.title).replace(/\n/g,'<br>')}</h1><p>${h(p.description)}</p><a class="cta" href="#section-0">${h(p.button)} <span aria-hidden="true">↗</span></a></div><div class="art" aria-hidden="true"><div class="art-core"></div><i class="art-dot"></i></div></section>${sections}</main><footer class="site-footer"><span>© ${new Date(p.created).getFullYear()} ${h(p.brand)}</span><span>${ar?'صُنع باستخدام':'Created with'} <b>PoCol Studio</b></span></footer></body></html>`;
}
export function projectFiles(p){if(p.mode==='files'){const f=p.files.map(f=>({path:f.path,data:decode64(f.base64)}));validateFiles(f,p.entry);return f;}const files=[{path:'/index.html',data:encoder.encode(generateHTML(p))}];validateFiles(files);return files;}
export function publicProject(p){return JSON.stringify({format:'pocol-studio-v1',project:p},null,2);}
export function parseProject(text){const v=JSON.parse(text);if(v.format!=='pocol-studio-v1')throw Error('projectInvalid');return validateProject(v.project);}
// FN-1 (C9): the network capacity pre-check before the publisher opens. Studio does not know which chunks the chain
// already holds, so it refuses only a site that cannot fit at its least (`siteNeedsAtLeast`: no chunk stored, only the
// site account and its Website slots and the three version transactions) and reports a site that may not fit at its
// most (`siteNeedsAtMost`: every chunk new). The extension publisher checks exactly, with the chain's chunks, before it
// signs. Website.sol: the constructor's 3 slots and the version counter; a version's manifestHash, meta, chunk-array
// length and one slot per manifest chunk (1 to 3). On a continuous chain (pocol_getCaps `cumulative` false, H_END = 0)
// item 16 and LOG_CAP count each block from zero, and the publisher confirms each transaction before it sends the next,
// so the txCount, txBytes and logBytes rows are checked per transaction (one transaction, the largest one's bytes).
export const CHUNK_MAX=24575,MANIFEST_CHUNKS_MAX=3,CAPS_ALARM_PERCENT=80,TX_ENVELOPE_MAX=160,WEBSITE_RUNTIME_MAX=24576;
export const CAP_FIELDS=[['accounts','accountsCap','ACCT_CAP'],['storageSlots','storageSlotsCap','SLOT_CAP'],['codeBytes','codeBytesCap','CODE_CAP'],['txCount','txCap','TX_CAP'],['txBytes','txBytesCap','TXBYTES_CAP'],['logBytes','logCap','LOG_CAP']];
export function siteNeedsAtLeast(files,{newSite}){const txs=3+(newSite?1:0);return{accounts:newSite?1:0,storageSlots:(newSite?4:0)+4,codeBytes:0,txCount:txs,txBytes:txs*36,logBytes:0,largestTxBytes:36};}
export function siteNeedsAtMost(files,{newSite}){
  const chunks=files.reduce((n,f)=>n+Math.ceil(f.data.length/CHUNK_MAX),0)+MANIFEST_CHUNKS_MAX,bytes=files.reduce((n,f)=>n+f.data.length,0)+MANIFEST_CHUNKS_MAX*CHUNK_MAX;
  const txs=chunks+3+(newSite?1:0);
  return{accounts:chunks+(newSite?1:0),storageSlots:(newSite?4:0)+3+MANIFEST_CHUNKS_MAX,codeBytes:bytes+chunks+(newSite?WEBSITE_RUNTIME_MAX:0),txCount:txs,txBytes:bytes+chunks*100+(newSite?2*WEBSITE_RUNTIME_MAX:0)+txs*(TX_ENVELOPE_MAX+200),logBytes:0,
    largestTxBytes:Math.max(CHUNK_MAX+100,newSite?2*WEBSITE_RUNTIME_MAX:0)+TX_ENVELOPE_MAX+200};
}
const PER_BLOCK=new Set(['txCount','txBytes','logBytes']);
/// A row's need: the whole publication, or (a per-block row of a continuous chain) one transaction, the largest
/// transaction's bytes and the publication's log bytes (0).
export function capNeed(needs,name,perBlock){
  if(!perBlock||!PER_BLOCK.has(name))return BigInt(needs[name]);
  if(name==='txCount')return BigInt(needs.txCount)>0n?1n:0n;
  if(name==='txBytes')return BigInt(needs.largestTxBytes??needs.txBytes);
  return BigInt(needs.logBytes);
}
const capQty=(v,name)=>{if(typeof v!=='string'||!/^0x[0-9a-fA-F]{1,64}$/.test(v))throw Error('capsInvalid:'+name);return BigInt(v);};
/// The verdict on a pocol_getCaps answer: `refuse` lists the caps the site cannot fit even at its least, `tight` those it
/// may not fit at its most, `alarms` those at or above 80 %. Each row names the cap, the need and the room.
/// With `cumulative` false the txCount, txBytes and logBytes rows are one block's room (labelled "per block"); an answer
/// without the field is read as cumulative (the stricter check).
export function capsVerdict(caps,least,most){
  if(!caps||typeof caps!=='object')throw Error('capsInvalid');
  if(caps.cumulative!==undefined&&typeof caps.cumulative!=='boolean')throw Error('capsInvalid:cumulative');
  const perBlock=caps.cumulative===false;
  const refuse=[],tight=[],alarms=[];
  for(const [name,capName,cap_label] of CAP_FIELDS){
    const count=capQty(caps[name],name),cap=capQty(caps[capName],capName),room=cap>count?cap-count:0n;
    const label=perBlock&&PER_BLOCK.has(name)?cap_label+' per block':cap_label,lo=capNeed(least,name,perBlock),hi=capNeed(most,name,perBlock);
    if(lo>room)refuse.push({name,label,need:lo.toString(),remaining:room.toString()});
    else if(hi>room)tight.push({name,label,need:hi.toString(),remaining:room.toString()});
    if(count*100n>=cap*BigInt(CAPS_ALARM_PERCENT))alarms.push(name);
  }
  return{checked:true,refuse,tight,alarms};
}
/// pocol_getCaps through the Studio gateway (`post(body)` resolves to {ok, json()}), then the verdict. A node or gateway
/// without the method (-32601, or an HTTP refusal) or no answer gives {checked:false}: the pre-check is skipped with a
/// warning, and the extension's own checks still run before signing.
export async function capsPrecheck(post,files,{newSite}){
  let v;
  try{const r=await post(JSON.stringify({jsonrpc:'2.0',id:1,method:'pocol_getCaps',params:[]}));if(!r.ok)return{checked:false,reason:'http'};v=await r.json();}
  catch{return{checked:false,reason:'unreachable'};}
  if(v?.error)return{checked:false,reason:v.error.code===-32601?'method':'error',code:v.error.code};
  return capsVerdict(v?.result,siteNeedsAtLeast(files,{newSite}),siteNeedsAtMost(files,{newSite}));
}
export function capsDetails(rows){return rows.map(r=>`${r.label}: ${r.need} / ${r.remaining}`).join(' · ');}
