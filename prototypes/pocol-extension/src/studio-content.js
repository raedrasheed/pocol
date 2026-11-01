// Runs only at the exact Studio origins, in Chrome's isolated content-script world.
(() => {
  const allowed = ['https://studio.pocol.net', 'https://pocol-studio.mystic-gecko-9511.chatgpt.site'];
  if (window.top !== window || !allowed.includes(location.origin)) return;
  const methods = new Set(['status', 'openPublisher', 'jobStatus', 'openSite']);
  const pending = new Set();
  window.addEventListener('message', async (event) => {
    const m = event.data;
    if (event.source !== window || event.origin !== location.origin || !m || m.source !== 'pocol-studio' || !methods.has(m.method)) return;
    if (typeof m.id !== 'string' || m.id.length < 1 || m.id.length > 100 || pending.has(m.id) || pending.size >= 8) return;
    pending.add(m.id);
    try {
      const response = await chrome.runtime.sendMessage({ type: 'studio.' + m.method, payload: m.payload || {} });
      window.postMessage({ source: 'pocol-extension', id: m.id, ...(response || { ok: false, error: 'Extension unavailable' }) }, location.origin);
    } catch { window.postMessage({ source: 'pocol-extension', id: m.id, ok: false, error: 'Reload this page after updating the PoCol extension.' }, location.origin); }
    finally { pending.delete(m.id); }
  });
})();
