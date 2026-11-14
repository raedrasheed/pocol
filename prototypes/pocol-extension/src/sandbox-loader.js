// PoCol sandbox loader (browser.md D26 step 5). Inlined into sandbox.html at build
// time; its sha256 is the only script hash allowed by the sandbox CSP. Order:
// RtcLockdown, guards and freezing, then (after the single MessageChannel
// handoff) sanitize, rewrite to blob:, and execute scripts from blob in order.
// The site is an adversary (A7b): nothing here is a security boundary on its own;
// BridgeAuth in the worker checks every message, CSP and DNR block the network.
(() => {
  'use strict';
  const W = window, D = document;
  const define = (o, k, v) => { try { Object.defineProperty(o, k, { value: v, writable: false, configurable: false }); } catch { /* already locked */ } };
  const stringify = JSON.stringify, MapC = Map, PromiseC = Promise, BlobC = Blob, ResponseC = Response, TE = TextEncoder, TD = TextDecoder;
  const createObjectURL = URL.createObjectURL.bind(URL), URLC = URL;
  // lib/modscan.js (W2b), inlined by build.mjs: the module specifier scanner shared with the publish-time check.
  const MS = /*@modscan*/null;

  // ---- Layer 3: RtcLockdown + removal of network/worker constructors ----
  /*@fault noRtc*/for (const k of ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel', 'RTCSessionDescription', 'RTCIceCandidate', 'RTCRtpSender', 'RTCRtpReceiver',
    'RTCRtpTransceiver', 'RTCDtlsTransport', 'RTCIceTransport', 'RTCSctpTransport', 'RTCCertificate', 'RTCDTMFSender', 'RTCEncodedAudioFrame', 'RTCEncodedVideoFrame',
    'RTCRtpScriptTransform', 'WebSocket', 'WebSocketStream', 'WebTransport', 'EventSource', 'Worker', 'SharedWorker', 'BroadcastChannel']) define(W, k, undefined);/*@else*//*@end*/
  try { Object.defineProperty(Navigator.prototype, 'sendBeacon', { value: () => false, writable: false, configurable: false }); } catch {}
  try { Object.defineProperty(Navigator.prototype, 'mediaDevices', { get: () => undefined, configurable: false }); } catch {}

  // ---- Frame guard: no nested browsing context can be created (fresh realms would undo RtcLockdown) ----
  const FORBID = new Set(['IFRAME', 'FRAME', 'FRAMESET', 'OBJECT', 'EMBED', 'PORTAL', 'BASE', 'APPLET', 'FENCEDFRAME']);
  const FORBID_SEL = 'iframe,frame,frameset,object,embed,portal,base,applet,fencedframe,meta[http-equiv],svg script,link[rel~=prefetch],link[rel~=preconnect],link[rel~=dns-prefetch],link[rel~=prerender]';
  const qsa = Element.prototype.querySelectorAll, fqsa = DocumentFragment.prototype.querySelectorAll, dqsa = Document.prototype.querySelectorAll;
  const all = (root, sel) => (root.nodeType === 1 ? qsa : root.nodeType === 11 ? fqsa : dqsa).call(root, sel);
  const removeNode = Element.prototype.remove;
  const getAttrNames = Element.prototype.getAttributeNames, getAttr = Element.prototype.getAttribute, rmAttr = Element.prototype.removeAttribute, setAttr = Element.prototype.setAttribute;
  const JS_URL = /^[\u0000- ]*j[\s]*a[\s]*v[\s]*a[\s]*s[\s]*c[\s]*r[\s]*i[\s]*p[\s]*t[\s]*:/i;
  /** Removes forbidden elements, on* handlers and javascript: URLs (browser.md sanitization). */
  function sanitize(root) {
    if (!root || (root.nodeType !== 1 && root.nodeType !== 11 && root.nodeType !== 9)) return;
    if (root.nodeType === 1 && (FORBID.has(root.nodeName.toUpperCase()) || root.matches(FORBID_SEL))) { removeNode.call(root); return; }
    for (const e of Array.from(all(root, FORBID_SEL))) removeNode.call(e);
    const els = root.nodeType === 1 ? [root, ...all(root, '*')] : Array.from(all(root, '*'));
    for (const e of els) for (const a of getAttrNames.call(e)) {
      const v = getAttr.call(e, a);
      if (/^on/i.test(a) || (v && JS_URL.test(v)) || a.toLowerCase() === 'srcdoc') rmAttr.call(e, a);
    }
  }
  const blocked = (n) => n && (n.nodeType === 1 || n.nodeType === 11) && ((n.nodeType === 1 && FORBID.has(n.nodeName.toUpperCase())) || all(n, 'iframe,frame,frameset,object,embed,portal,base,applet,fencedframe').length > 0);
  const guardNodes = (nodes) => { for (const n of nodes) if (blocked(n)) throw new DOMException('PoCol sandbox: nested frames/objects are not supported', 'NotSupportedError'); };
  const wrap = (proto, name, pick) => {
    const d = Object.getOwnPropertyDescriptor(proto, name); if (!d || typeof d.value !== 'function') return;
    const orig = d.value; const f = { [name](...args) { guardNodes(pick(args)); return orig.apply(this, args); } }[name];
    Object.defineProperty(proto, name, { value: f, writable: false, configurable: false, enumerable: d.enumerable });
  };
  const O = {
    appendChild: Node.prototype.appendChild, insertBefore: Node.prototype.insertBefore, replaceChild: Node.prototype.replaceChild,
    createElement: Document.prototype.createElement, replaceWith: Element.prototype.replaceWith, append: Element.prototype.append,
    innerHTML: Object.getOwnPropertyDescriptor(Element.prototype, 'innerHTML'), addEventListener: EventTarget.prototype.addEventListener,
  };
  wrap(Node.prototype, 'appendChild', (a) => [a[0]]); wrap(Node.prototype, 'insertBefore', (a) => [a[0]]); wrap(Node.prototype, 'replaceChild', (a) => [a[0]]);
  for (const P of [Element.prototype, Document.prototype, DocumentFragment.prototype]) for (const m of ['append', 'prepend', 'replaceChildren']) wrap(P, m, (a) => a);
  for (const P of [Element.prototype, CharacterData.prototype, DocumentType.prototype]) for (const m of ['before', 'after', 'replaceWith']) wrap(P, m, (a) => a);
  wrap(Element.prototype, 'insertAdjacentElement', (a) => [a[1]]); wrap(Range.prototype, 'insertNode', (a) => [a[0]]); wrap(Range.prototype, 'surroundContents', (a) => [a[0]]);
  // HTML string sinks: sanitize an inert template parse first, then sanitize the live result synchronously.
  const tplParse = (html) => { const t = O.createElement.call(D, 'template'); O.innerHTML.set.call(t, String(html)); sanitize(t.content); return O.innerHTML.get.call(t); };
  const htmlSink = (proto, name) => {
    const d = Object.getOwnPropertyDescriptor(proto, name); if (!d || !d.set) return;
    Object.defineProperty(proto, name, { get: d.get, set(v) { const parent = name === 'outerHTML' ? this.parentNode : this; d.set.call(this, tplParse(v)); if (parent) sanitize(parent); }, configurable: false, enumerable: d.enumerable });
  };
  htmlSink(Element.prototype, 'innerHTML'); htmlSink(Element.prototype, 'outerHTML'); htmlSink(ShadowRoot.prototype, 'innerHTML');
  { const orig = Element.prototype.insertAdjacentHTML; define(Element.prototype, 'insertAdjacentHTML', function insertAdjacentHTML(pos, html) { orig.call(this, pos, tplParse(html)); sanitize(this.parentNode || this); }); }
  { const orig = Document.prototype.execCommand; define(Document.prototype, 'execCommand', function execCommand(cmd, ui, value) {
    const c = String(cmd).toLowerCase(); if (c === 'inserthtml') value = tplParse(value); const r = orig.call(this, cmd, ui, value); if (c === 'inserthtml' || c === 'paste') sanitize(this); return r; }); }
  if (Element.prototype.setHTML) define(Element.prototype, 'setHTML', function setHTML(html) { this.innerHTML = html; });
  for (const [P, m] of [[Element.prototype, 'setHTMLUnsafe'], [ShadowRoot.prototype, 'setHTMLUnsafe'], [Document.prototype, 'write'], [Document.prototype, 'writeln'], [Document.prototype, 'open']])
    if (P[m]) define(P, m, function () { throw new DOMException('PoCol sandbox: ' + m + ' is not supported', 'NotSupportedError'); });
  for (const C of [W.HTMLIFrameElement, W.HTMLFrameElement, W.HTMLObjectElement, W.HTMLEmbedElement]) if (C) for (const k of ['contentWindow', 'contentDocument'])
    if (Object.getOwnPropertyDescriptor(C.prototype, k)) Object.defineProperty(C.prototype, k, { get: () => null, configurable: false });
  try { Object.defineProperty(Document.prototype, 'cookie', { get: () => '', set: () => {}, configurable: false }); } catch {}

  // ---- Single MessageChannel handoff (D26 step 4) ----
  let port = null;
  const onInit = (e) => {
    if (port || e.source !== W.parent || !e.data || e.data.type !== 'pocol-init' || !e.ports || e.ports.length !== 1) return;
    port = e.ports[0]; W.removeEventListener('message', onInit, true);
    boot(e.data).catch((err) => { D.documentElement.textContent = 'PoCol loader error: ' + err.message; });
  };
  W.addEventListener('message', onInit, true);

  async function boot(init) {
    const files = new MapC(); for (const p of Object.keys(init.files)) files.set(p, init.files[p]);
    let currentPath = init.path;
    // ---- bridge client ----
    let seq = 0, nextId = 1; const waiting = new MapC();
    port.onmessage = (e) => { const m = e.data; if (!m || !Number.isInteger(m.seq)) return; const r = waiting.get(m.seq); if (r) { waiting.delete(m.seq); r(m.reply); } };
    const raw = (text) => new PromiseC((res) => { const s = ++seq; waiting.set(s, res); port.postMessage({ seq: s, text: String(text) }); });
    const request = async (kind, method, params = []) => {
      const id = nextId; nextId = (nextId + 1) >>> 0;
      const reply = await raw(stringify({ id, kind, payload: { method, params } }));
      if (reply && reply.error) throw Object.assign(new Error(reply.error.message), { code: reply.error.code, data: reply.error.data });
      return reply ? reply.result : undefined;
    };
    // ---- localStorage shim (D96): synchronous reads from the snapshot, local quota, batched writes ----
    const u8 = (s) => new TE().encode(s).length; const entry = (k, v) => u8(k) + u8(v);
    const snap = Object.assign(Object.create(null), init.storage || {}); const confirmed = Object.assign(Object.create(null), snap);
    let total = Object.keys(snap).reduce((a, k) => a + entry(k, snap[k]), 0);
    let dirty = new MapC(), clearMark = false, flushTimer = null, inflight = 0;
    const schedule = () => { if (!flushTimer) flushTimer = setTimeout(() => { flushTimer = null; flush(); }, 250); };
    async function flush() {
      const jobs = [];
      if (clearMark) { clearMark = false; jobs.push(request('storage_set', 'site_storageClear', []).then(() => { for (const k of Object.keys(confirmed)) delete confirmed[k]; }, () => { clearMark = true; })); }
      const batch = dirty; dirty = new MapC();
      for (const [k, v] of batch) {
        if (inflight >= 8) { if (!dirty.has(k)) dirty.set(k, v); continue; }
        inflight++;
        jobs.push(request('storage_set', 'site_storageSet', [k, v]).then(() => { if (v === null) delete confirmed[k]; else confirmed[k] = v; }, (e) => {
          if (e.code === 4300) { revert(k); W.dispatchEvent(new CustomEvent('pocolstorageerror', { detail: { key: k, code: 4300 } })); }
          else if (!dirty.has(k)) dirty.set(k, v);
        }).finally(() => { inflight--; }));
      }
      await PromiseC.all(jobs); if (dirty.size || clearMark) schedule();
    }
    const revert = (k) => { const had = Object.prototype.hasOwnProperty.call(snap, k); if (had) total -= entry(k, snap[k]); if (k in confirmed) { snap[k] = confirmed[k]; total += entry(k, snap[k]); } else delete snap[k]; };
    const quotaErr = () => new DOMException('PoCol site storage quota (1048576) exceeded', 'QuotaExceededError');
    const ls = {
      get length() { return Object.keys(snap).length; },
      key(i) { const k = Object.keys(snap)[i]; return k === undefined ? null : k; },
      getItem(k) { k = String(k); return Object.prototype.hasOwnProperty.call(snap, k) ? snap[k] : null; },
      setItem(k, v) {
        k = String(k); v = String(v);
        if (u8(k) < 1 || u8(k) > 256 || u8(v) < 1 || u8(v) > 61440) throw new TypeError('PoCol site storage: key 1..256 bytes, value 1..61440 bytes');
        const has = Object.prototype.hasOwnProperty.call(snap, k); const nt = total - (has ? entry(k, snap[k]) : 0) + entry(k, v);
        if (nt > 1048576 || (!has && Object.keys(snap).length + 1 > 4096)) throw quotaErr();
        snap[k] = v; total = nt; dirty.set(k, v); schedule();
      },
      removeItem(k) { k = String(k); if (!Object.prototype.hasOwnProperty.call(snap, k)) return; total -= entry(k, snap[k]); delete snap[k]; dirty.set(k, null); schedule(); },
      clear() { for (const k of Object.keys(snap)) delete snap[k]; total = 0; dirty = new MapC(); clearMark = true; schedule(); },
    };
    Object.freeze(ls);
    const mem = new MapC(); const ss = Object.freeze({ get length() { return mem.size; }, key: (i) => [...mem.keys()][i] ?? null, getItem: (k) => mem.get(String(k)) ?? null,
      setItem: (k, v) => { mem.set(String(k), String(v)); }, removeItem: (k) => { mem.delete(String(k)); }, clear: () => mem.clear() });
    define(W, 'localStorage', ls); define(W, 'sessionStorage', ss);

    // ---- resource resolution and blob rewriting ----
    const ROOT = 'https://pocol.invalid';
    const SANDBOX_BASE = location.href.replace(/[?#].*$/, '').replace(/[^/]*$/, '');
    /** {path} for an in-site reference, {external} for absolute URLs, {keep} for blob:/data:, null when unusable. */
    function resolve(ref, base) {
      ref = String(ref).trim(); if (ref === '') return null;
      if (/^(blob|data):/i.test(ref)) return { keep: true };
      if (ref.startsWith(SANDBOX_BASE)) ref = '/' + ref.slice(SANDBOX_BASE.length);
      if (/^[a-z][a-z0-9+.-]*:/i.test(ref) || ref.startsWith('//')) return { external: ref };
      if (ref.startsWith('#')) return { fragment: true };
      let u; try { u = new URLC(ref, ROOT + base); } catch { return null; }
      const path = u.pathname.endsWith('/') ? u.pathname + 'index.html' : u.pathname;
      return { path, hash: u.hash };
    }
    const blobs = new MapC(); const cssDone = new MapC();
    const blobOf = (data, mime) => createObjectURL(new BlobC([data], { type: mime }));
    function urlFor(path, depth = 0) {
      const f = files.get(path); if (!f) return null;
      if (blobs.has(path)) return blobs.get(path);
      let u;
      if (f.mime === 'text/css') { if (depth > 8 || cssDone.has(path)) return null; cssDone.set(path, true); u = blobOf(rewriteCss(new TD().decode(f.data), path, depth + 1), 'text/css'); }
      else u = blobOf(f.data, f.mime);
      blobs.set(path, u); return u;
    }
    const refUrl = (ref, base, depth) => { const r = resolve(ref, base); if (!r) return null; if (r.keep) return ref; if (r.path) return urlFor(r.path, depth); return null; };
    function rewriteCss(text, base, depth = 0) {
      text = text.replace(/@import\s+(?:url\(\s*)?(['"]?)([^'")\s;]+)\1\s*\)?/gi, (m, q, ref) => { const u = refUrl(ref, base, depth); return u ? `@import url("${u}")` : '/* PoCol: blocked @import */'; });
      return text.replace(/url\(\s*(['"]?)([^'")]*)\1\s*\)/gi, (m, q, ref) => { if (/^blob:/i.test(ref)) return m; const u = refUrl(ref, base, depth); return u ? `url("${u}")` : 'url("")'; });
    }
    // ---- ES modules (W2b): relative specifiers rewritten to blob: URLs, built leaf-first ----
    const JS = 'application/javascript'; const modBlobs = new MapC(), classicBlobs = new MapC(); let modCount = 0;
    const warn = (msg) => { try { W.console.warn('PoCol: ' + msg); } catch { /* console replaced by the site */ } };
    /** blob: URL of the module at `path`, its static imports first; `stack` holds the importers being built. */
    function moduleUrl(path, stack = []) {
      if (modBlobs.has(path)) return modBlobs.get(path);
      const at = stack.indexOf(path);
      if (at >= 0) throw new Error('module import cycle: ' + [...stack.slice(at), path].join(' -> ') + ' (blob: URLs cannot express a cycle of static imports)');
      if (stack.length > MS.MODULE_DEPTH_MAX) throw new Error('module imports nest deeper than ' + MS.MODULE_DEPTH_MAX + ' at ' + path);
      if (++modCount > MS.MODULE_COUNT_MAX) throw new Error('a site may load at most ' + MS.MODULE_COUNT_MAX + ' modules');
      const next = [...stack, path];
      const text = MS.rewriteModule(new TD().decode(files.get(path).data), path, (spec) => staticTarget(spec, path, next));
      const u = blobOf(new TE().encode(text), JS); modBlobs.set(path, u); return u;
    }
    /** URL for a static specifier in the module at `base`, or null to leave it (bare/external: blocked by the CSP). */
    function staticTarget(spec, base, stack) {
      const r = MS.resolveSpecifier(spec, base); if (!r || !r.path) return null;
      const f = files.get(r.path); if (!f) { warn(base + ': import of ' + stringify(spec) + ' names ' + r.path + ', which is not in this site version'); return null; }
      return f.mime === JS ? moduleUrl(r.path, stack) : f.mime === JSON_MIME ? { url: jsonModuleUrl(r.path), dropAttributes: true } : urlFor(r.path);
    }
    // The CSP's connect-src 'none' also blocks JSON module fetches, so a JSON file is imported as a JavaScript module
    // whose default export is JSON.parse of its text (its `with { type: 'json' }` is dropped). Parse only, no code.
    const jsonBlobs = new MapC(), JSON_MIME = 'application/json';
    function jsonModuleUrl(path) {
      if (!jsonBlobs.has(path)) jsonBlobs.set(path, blobOf(new TE().encode('export default JSON.parse(' + stringify(new TD().decode(files.get(path).data)) + ');\n'), JS));
      return jsonBlobs.get(path);
    }
    /** A classic script: only its import() calls are rewritten. */
    function classicUrl(path) {
      if (classicBlobs.has(path)) return classicBlobs.get(path);
      const f = files.get(path), text = new TD().decode(f.data);
      const u = text.includes('import') ? blobOf(new TE().encode(MS.rewriteModule(text, path, null, { statics: false })), JS) : blobOf(f.data, JS); classicBlobs.set(path, u); return u;
    }
    const inlineScript = (text, module) => blobOf(new TE().encode(module ? MS.rewriteModule(text, currentPath, (spec) => staticTarget(spec, currentPath, []))
      : MS.rewriteModule(text, currentPath, null, { statics: false })), JS);
    function scriptUrl(ref, base, el) {
      const r = resolve(ref, base); const f = r && r.path ? files.get(r.path) : null;
      if (!f || f.mime !== JS) return refUrl(ref, base, 0);
      return (getAttr.call(el, 'type') || '').trim().toLowerCase() === 'module' ? moduleUrl(r.path) : classicUrl(r.path);
    }
    // Every import(...) of a site script calls this with the importing file's path: the specifier is resolved when the
    // call runs (also a computed one), so a lazily loaded module may import back from the module that loaded it.
    define(W, MS.RUNTIME_IMPORT, function pocolImport(base, spec, options) {
      try {
        const s = String(spec), r = MS.resolveSpecifier(s, String(base));
        if (r && r.keep) return options === undefined ? import(s) : import(s, options);
        if (!r || !r.path) return PromiseC.reject(new TypeError('PoCol site: import(' + stringify(s.slice(0, 120)) + ') is blocked: only files of this site can be imported'));
        const f = files.get(r.path);
        if (!f) return PromiseC.reject(new TypeError('PoCol site: module ' + r.path + ' is not in this site version'));
        if (f.mime === JSON_MIME) return import(jsonModuleUrl(r.path));
        const u = f.mime === JS ? moduleUrl(r.path) : urlFor(r.path);
        return options === undefined ? import(u) : import(u, options);
      } catch (e) { return PromiseC.reject(e); }
    });
    function rewriteSrcset(v, base) { return v.split(',').map((part) => { const [u, ...d] = part.trim().split(/\s+/); const r = refUrl(u, base, 0); return r ? [r, ...d].join(' ') : null; }).filter(Boolean).join(', '); }
    const URL_ATTRS = ['src', 'poster', 'href', 'xlink:href', 'data'];
    function rewriteEl(e, base) {
      const tag = e.localName;
      for (const a of URL_ATTRS) {
        const v = getAttr.call(e, a); if (v === null) continue;
        if (tag === 'a' || (tag === 'area')) continue; // links are intercepted, not loaded
        if (a === 'href' && tag === 'link' && !/(^|\s)(stylesheet|icon|preload|apple-touch-icon|manifest)(\s|$)/i.test(getAttr.call(e, 'rel') || '')) { rmAttr.call(e, a); continue; }
        if (/^blob:/i.test(v)) continue;
        const u = tag === 'script' && a === 'src' ? scriptUrl(v, base, e) : refUrl(v, base, 0); if (u) setAttr.call(e, a, u); else { rmAttr.call(e, a); setAttr.call(e, 'data-pocol-blocked', v.slice(0, 256)); }
      }
      const ss = getAttr.call(e, 'srcset'); if (ss !== null && !/blob:/.test(ss)) setAttr.call(e, 'srcset', rewriteSrcset(ss, base));
      const st = getAttr.call(e, 'style'); if (st !== null && /url\(|@import/i.test(st) && !/blob:/.test(st)) setAttr.call(e, 'style', rewriteCss(st, base));
      if (tag === 'style' && /url\(|@import/i.test(e.textContent) && !/blob:/.test(e.textContent)) e.textContent = rewriteCss(e.textContent, base);
    }
    function rewriteTree(root, base) { if (root.nodeType === 1) rewriteEl(root, base); for (const e of Array.from(all(root, '*'))) rewriteEl(e, base); }

    // ---- fetch / XHR shims: served from the verified file map, never from the network ----
    function serve(input, method) {
      let ref = typeof input === 'string' ? input : input instanceof URLC ? input.href : input && input.url;
      const r = resolve(ref, currentPath);
      if (!r || r.external || r.keep || r.fragment) return { error: new TypeError('Failed to fetch: network is disabled in PoCol sites (' + String(ref).slice(0, 80) + ')') };
      if (method !== 'GET' && method !== 'HEAD') return { error: new TypeError('Failed to fetch: only GET/HEAD of site files') };
      const f = files.get(r.path); return f ? { file: f } : { notFound: true };
    }
    define(W, 'fetch', function fetch(input, init) {
      const method = String((init && init.method) || (input && input.method) || 'GET').toUpperCase(); const s = serve(input, method);
      if (s.error) return PromiseC.reject(s.error);
      if (s.notFound) return PromiseC.resolve(new ResponseC('Not Found', { status: 404, statusText: 'Not Found' }));
      return PromiseC.resolve(new ResponseC(method === 'HEAD' ? null : new BlobC([s.file.data], { type: s.file.mime }), { status: 200, headers: { 'content-type': s.file.mime, 'content-length': String(s.file.data.length) } }));
    });
    class PoColXHR extends EventTarget {
      constructor() { super(); this.readyState = 0; this.status = 0; this.statusText = ''; this.responseType = ''; this.response = null; this.responseText = ''; this._m = 'GET'; this._u = ''; this._h = ''; this.timeout = 0; this.withCredentials = false; this.upload = new EventTarget(); }
      open(m, u) { this._m = String(m).toUpperCase(); this._u = String(u); this._set(1); }
      setRequestHeader() {} overrideMimeType() {} abort() {}
      getResponseHeader(n) { return String(n).toLowerCase() === 'content-type' && this._h ? this._h : null; }
      getAllResponseHeaders() { return this._h ? 'content-type: ' + this._h + '\r\n' : ''; }
      _set(s) { this.readyState = s; const ev = new Event('readystatechange'); if (typeof this.onreadystatechange === 'function') this.onreadystatechange(ev); this.dispatchEvent(ev); }
      _fire(t) { const ev = new ProgressEvent(t); if (typeof this['on' + t] === 'function') this['on' + t](ev); this.dispatchEvent(ev); }
      send() {
        setTimeout(() => {
          const s = serve(this._u, this._m);
          if (s.error) { this._set(4); this._fire('error'); this._fire('loadend'); return; }
          if (s.notFound) { this.status = 404; this.statusText = 'Not Found'; this.responseText = 'Not Found'; this.response = this.responseType === '' || this.responseType === 'text' ? 'Not Found' : null; }
          else {
            this.status = 200; this.statusText = 'OK'; this._h = s.file.mime; const t = this.responseType; const d = s.file.data;
            if (t === 'arraybuffer') this.response = d.slice().buffer; else if (t === 'blob') this.response = new BlobC([d], { type: s.file.mime });
            else if (t === 'json') { try { this.response = JSON.parse(new TD().decode(d)); } catch { this.response = null; } }
            else { this.responseText = new TD().decode(d); this.response = this.responseText; }
          }
          this._set(2); this._set(3); this._set(4); this._fire('load'); this._fire('loadend');
        }, 0);
      }
    }
    define(W, 'XMLHttpRequest', PoColXHR);

    // ---- site APIs: window.pocol and a minimal EIP-1193 provider (all through BridgeAuth) ----
    const WALLET = new Set(['eth_requestAccounts', 'eth_accounts', 'eth_sendTransaction']);
    const navigate = async (path) => { await PromiseC.race([flush(), new PromiseC((r) => setTimeout(r, 2000))]); return request('nav', 'site_navigate', [path]); };
    define(W, 'pocol', Object.freeze({ version: init.version, path: init.path, entry: init.entry, request: (kind, method, params) => request(kind, method, params), raw: (text) => raw(text),
      navigate, openExternal: (url) => request('nav', 'site_openExternal', [url]), flushStorage: () => flush() }));
    define(W, 'ethereum', Object.freeze({ isPoCol: true, request: ({ method, params = [] }) => request(WALLET.has(method) ? 'wallet_req' : 'rpc_read', method, params), on() {}, removeListener() {} }));

    // ---- link interception (D96): in-site -> site_navigate, https external -> site_openExternal ----
    O.addEventListener.call(W, 'click', (e) => {
      if (e.defaultPrevented || e.button !== 0) return;
      const a = e.target && e.target.closest ? e.target.closest('a[href],area[href]') : null; if (!a) return;
      const href = getAttr.call(a, 'href'); const r = resolve(href, currentPath);
      if (r && r.fragment) return; // same-document fragment
      e.preventDefault();
      if (r && r.path) navigate(r.path).catch(() => {});
      else if (r && r.external && /^https:\/\//i.test(r.external)) request('nav', 'site_openExternal', [r.external]).catch(() => {});
    }, false);

    // ---- build the document ----
    const html = init.notFound ? '<!doctype html><title>404</title><h1>404</h1><p>Not found in this site version: ' + String(init.requested || init.path).replace(/[<>&"]/g, '') + '</p>'
      : new TD().decode(files.get(currentPath).data);
    const doc = new DOMParser().parseFromString(html, 'text/html');
    sanitize(doc); for (const n of Array.from(dqsa.call(doc, 'noscript'))) removeNode.call(n); rewriteTree(doc, currentPath);
    const scripts = [];
    for (const s of Array.from(dqsa.call(doc, 'script'))) {
      const type = (getAttr.call(s, 'type') || '').trim().toLowerCase();
      const module = type === 'module'; if (!(module || type === '' || type === 'text/javascript' || type === 'application/javascript')) continue;
      if (getAttr.call(s, 'data-pocol-blocked') !== null) { s.remove(); continue; } // external script: never executed
      const src = getAttr.call(s, 'src'); let url = null;
      if (src !== null) url = /^blob:/i.test(src) ? src : null; else url = inlineScript(s.textContent, module);
      const mark = doc.createComment('pocol-script'); s.replaceWith(mark); if (url) scripts.push({ mark, url, module });
    }
    const root = D.adoptNode(doc.documentElement);
    O.replaceChild.call(D, root, D.documentElement);
    // Later DOM additions: sanitize and rewrite relative resources (CSP blocks them until rewritten).
    new MutationObserver((recs) => {
      for (const r of recs) try {
        if (r.type === 'childList') for (const n of r.addedNodes) { if (n.nodeType === 1) { sanitize(n); if (n.isConnected) rewriteTree(n, currentPath); } }
        else if (r.type === 'attributes' && r.target.isConnected) { const v = getAttr.call(r.target, r.attributeName); if (v !== null && /^on/i.test(r.attributeName)) rmAttr.call(r.target, r.attributeName); else if (v !== null && !/blob:/.test(v)) rewriteEl(r.target, currentPath); }
      } catch (e) { warn(e.message); }
    }).observe(D, { subtree: true, childList: true, attributes: true, attributeFilter: ['src', 'href', 'srcset', 'style', 'poster', 'data', 'onclick', 'onload', 'onerror'] });
    for (const s of scripts) {
      await new PromiseC((res) => {
        const el = O.createElement.call(D, 'script'); if (s.module) el.type = 'module';
        el.addEventListener('load', res); el.addEventListener('error', res); el.src = s.url;
        O.replaceChild.call(s.mark.parentNode, el, s.mark);
      });
    }
    D.dispatchEvent(new Event('DOMContentLoaded', { bubbles: true }));
    W.dispatchEvent(new Event('load'));
    D.documentElement.setAttribute('data-pocol-ready', '1');
  }
})();
