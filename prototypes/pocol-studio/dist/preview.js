import{encode64,mimeFor}from'./model.js';
const decoder=new TextDecoder();
const PREVIEW_CSP="default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; media-src 'none'; frame-src 'none'; object-src 'none'; connect-src 'none'; form-action 'none'; base-uri 'none'";
export function previewHTML(files,entry='/index.html'){
 const map=new Map(files.map(f=>[f.path,f.data]));const base=entry.substring(0,entry.lastIndexOf('/')+1);
 const resolve=(url,from=base)=>{if(!url||url.startsWith('#'))return null;try{const u=new URL(url,'https://preview.invalid'+from);return u.origin==='https://preview.invalid'?u.pathname:null;}catch{return null;}};
 const dataURL=(url,from=base)=>{if(/^data:(?:image\/(?:png|jpeg|webp|svg\+xml)|font\/woff2);base64,[A-Za-z0-9+/=]+$/i.test(url))return url;const p=resolve(url,from),d=map.get(p);return d?`data:${mimeFor(p)};base64,${encode64(d)}`:'';};
 const css=(text,from=base)=>text.replace(/@import[^;]*(?:;|$)/gi,'').replace(/url\(\s*(['"]?)(.*?)\1\s*\)/gi,(_,q,url)=>`url("${dataURL(url,from)}")`);
 const doc=new DOMParser().parseFromString(decoder.decode(map.get(entry)||new Uint8Array()),'text/html');
 doc.querySelectorAll('script,iframe,frame,object,embed,base,animate,animateTransform,animateMotion,set,meta[http-equiv],link[rel=preload],link[rel=prefetch],link[rel=modulepreload]').forEach(e=>e.remove());
 for(const link of doc.querySelectorAll('link')){const p=resolve(link.getAttribute('href'));if(link.rel==='stylesheet'&&p&&map.has(p)){const s=doc.createElement('style');s.textContent=css(decoder.decode(map.get(p)),p.slice(0,p.lastIndexOf('/')+1));link.replaceWith(s);}else link.remove();}
 for(const e of doc.querySelectorAll('*')){
  for(const a of [...e.attributes]){if(a.name.startsWith('on')||['srcdoc','srcset','ping','action','formaction','target','poster','background','autofocus'].includes(a.name.toLowerCase()))e.removeAttribute(a.name);}
  if(e.hasAttribute('src')){const d=dataURL(e.getAttribute('src'));if(e.tagName==='IMG'&&d)e.setAttribute('src',d);else e.removeAttribute('src');}
  if(e.hasAttribute('href'))e.removeAttribute('href');if(e.hasAttribute('download'))e.removeAttribute('download');if(e.hasAttribute('xlink:href'))e.removeAttribute('xlink:href');
  if(e.hasAttribute('style'))e.setAttribute('style',css(e.getAttribute('style')));
  if(['INPUT','TEXTAREA','SELECT','BUTTON'].includes(e.tagName))e.setAttribute('disabled','');
 }
 for(const s of doc.querySelectorAll('style'))s.textContent=css(s.textContent);
 const meta=doc.createElement('meta');meta.httpEquiv='Content-Security-Policy';meta.content=PREVIEW_CSP;doc.head.prepend(meta);
 return'<!doctype html>'+doc.documentElement.outerHTML;
}
