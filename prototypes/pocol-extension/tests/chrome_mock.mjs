// Deterministic Chrome extension API mock for running the real service worker module in
// Node (component tests). NOT real-browser evidence: real Chrome runs are separate.
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

export function installChromeMock({ extensionId = 'mockextensionidaaaaaaaaaaaaaaaaa' } = {}) {
  const data = new Map();
  const storage = {
    holdSets: 0, held: [], setCalls: 0, failSets: 0,
    async get(keys) {
      const ks = keys === null || keys === undefined ? [...data.keys()] : Array.isArray(keys) ? keys : typeof keys === 'string' ? [keys] : Object.keys(keys);
      const o = {}; for (const k of ks) if (data.has(k)) o[k] = structuredClone(data.get(k)); return o;
    },
    set(obj) {
      storage.setCalls++;
      if (storage.failSets > 0) { storage.failSets--; return Promise.reject(new Error('io')); }
      const apply = () => { for (const [k, v] of Object.entries(obj)) data.set(k, structuredClone(v)); };
      if (storage.holdSets > 0 && !Object.keys(obj).some((k) => k.startsWith('pocol:epoch:') || k === 'profiles' || k === 'wallet')) {
        storage.holdSets--; return new Promise((res, rej) => storage.held.push({ release: () => { apply(); res(); }, fail: () => rej(new Error('io')) }));
      }
      apply(); return Promise.resolve();
    },
    async remove(keys) { for (const k of Array.isArray(keys) ? keys : [keys]) data.delete(k); },
    async getKeys() { return [...data.keys()]; },
    async getBytesInUse() { let n = 0; for (const [k, v] of data) n += k.length + JSON.stringify(v).length; return n; },
    release() { const h = storage.held.shift(); if (h) h.release(); return !!h; },
    data,
  };
  const listeners = { message: [], connect: [], removed: [], omnibox: [], action: [] };
  let rules = [];
  const tabsCreated = [];
  globalThis.chrome = {
    storage: { local: storage },
    runtime: {
      id: extensionId, getURL: (p) => `chrome-extension://${extensionId}/${p}`, openOptionsPage() {},
      onMessage: { addListener: (f) => listeners.message.push(f) },
      onConnect: { addListener: (f) => listeners.connect.push(f) },
    },
    declarativeNetRequest: {
      async updateSessionRules({ removeRuleIds = [], addRules = [] }) {
        for (const id of [...removeRuleIds, ...addRules.map((r) => r.id)]) if (!Number.isInteger(id) || id < 1 || id > 0x7fffffff) throw new Error('rule id');
        rules = rules.filter((r) => !removeRuleIds.includes(r.id)).concat(addRules.map((r) => structuredClone(r)));
      },
      async getSessionRules() { return structuredClone(rules); },
    },
    tabs: { onRemoved: { addListener: (f) => listeners.removed.push(f) }, async create(o) { tabsCreated.push(o); return { id: 900 + tabsCreated.length }; }, async update() {} },
    omnibox: { onInputEntered: { addListener: (f) => listeners.omnibox.push(f) } },
    action: { onClicked: { addListener: (f) => listeners.action.push(f) } },
  };
  /** Sends an extension-page message (sender is an extension tab) and resolves with the reply. */
  const send = (msg, tabId = 7) => new Promise((resolve) => {
    const sender = { id: extensionId, tab: { id: tabId } };
    for (const f of listeners.message) { const keep = f(msg, sender, resolve); if (keep !== true) resolve(undefined); }
  });
  /** Opens a bridge port as the viewer of tab `tabId` would. */
  function connect(name, tabId = 7) {
    const inbox = []; const waiters = [];
    const port = {
      name, sender: { id: extensionId, tab: { id: tabId } }, disconnected: false,
      _msg: [], _disc: [],
      onMessage: { addListener: (f) => port._msg.push(f) }, onDisconnect: { addListener: (f) => port._disc.push(f) },
      postMessage: (m) => { const c = structuredClone(m); inbox.push(c); for (const w of waiters.splice(0)) w(); },
      disconnect: () => { port.disconnected = true; },
    };
    for (const f of listeners.connect) f(port);
    return {
      inbox,
      post: (m) => { for (const f of port._msg) f(structuredClone(m)); },
      close: () => { for (const f of port._disc) f(); },
      next: async (pred, ms = 5000) => {
        const t = Date.now();
        for (;;) { const i = inbox.findIndex(pred); if (i >= 0) return inbox.splice(i, 1)[0]; if (Date.now() - t > ms) throw new Error('timeout waiting for port message'); await new Promise((r) => { waiters.push(r); setTimeout(r, 20); }); }
      },
      port,
    };
  }
  return { storage, send, connect, rules: () => rules, tabsCreated, listeners };
}

/** Programmable JSON-RPC endpoint served through a fetch() mock. */
export function installFetchMock(handlers) {
  const log = [];
  globalThis.fetch = async (url, init) => {
    const body = JSON.parse(init.body); log.push({ url, method: body.method, params: body.params });
    const h = handlers[body.method];
    let reply;
    if (!h) reply = { jsonrpc: '2.0', id: body.id, error: { code: -32601, message: 'no handler ' + body.method } };
    else {
      const r = await h(body.params, { url, id: body.id });
      reply = r && r.__raw ? r.__raw(body.id) : r && r.__error ? { jsonrpc: '2.0', id: body.id, error: r.__error } : { jsonrpc: '2.0', id: body.id, result: r };
    }
    const text = typeof reply === 'string' ? reply : JSON.stringify(reply);
    return new Response(text, { status: 200, headers: { 'content-type': 'application/json' } });
  };
  return log;
}

/** Imports a fresh copy of the built service worker (query string defeats module caching). */
export async function loadWorker(dist, tag = String(Math.random())) {
  return import(pathToFileURL(join(dist, 'sw.js')).href + '?w=' + tag);
}
