// Public intent bridge only. It cannot connect, unlock, export or sign a wallet.
(() => {
  const origins=['https://ide.pocol.net','https://pocol-ide.mystic-gecko-9511.chatgpt.site'];
  if(window.top!==window || !origins.includes(location.origin))return;
  const methods=new Set(['status','openContract','jobStatus']),pending=new Set();
  window.addEventListener('message',async event=>{
    const message=event.data;
    if(event.source!==window || event.origin!==location.origin || !message || message.source!=='pocol-ide' || !methods.has(message.method))return;
    if(typeof message.id!=='string'||message.id.length<1||message.id.length>100||pending.has(message.id)||pending.size>=8)return;
    pending.add(message.id);
    try {
      const response=await chrome.runtime.sendMessage({type:'ide.'+message.method,payload:message.payload||{}});
      window.postMessage({source:'pocol-ide-extension',id:message.id,...(response||{ok:false,error:'Extension unavailable'})},location.origin);
    } catch { window.postMessage({source:'pocol-ide-extension',id:message.id,ok:false,error:'Reload this page after updating the PoCol extension.'},location.origin); }
    finally {pending.delete(message.id);}
  });
})();
