// BR16 generator, JavaScript implementation (M1 draft 0.6, R6-06). Specification tooling.
// Mirrors tools/br16_gen.py; both follow annex/br16-generator.json.
// Usage: node m1-draft-0.6/tools/br16_gen.cjs -> results/br16-node.json
// Also evaluates the httpsUrl predicate (native WHATWG URL) for every string that
// the stream places in params[0] of site_openExternal, so the Python harness never
// meets an httpsUrl value outside an oracle.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const pkg = path.resolve(__dirname, '..');
const SPEC = JSON.parse(fs.readFileSync(path.join(pkg, 'annex', 'br16-generator.json'), 'utf8'));
const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);

function seed(i) { const b = Buffer.alloc(4); b.writeUInt32BE(i); return crypto.createHash('sha256').update(Buffer.concat([Buffer.from('PoColSeed'), b])).digest(); }
class Prng {
  constructor(s) { this.s = s; this.k = 0n; this.buf = Buffer.alloc(0); }
  u32() {
    if (this.buf.length === 0) { const kb = Buffer.alloc(8); kb.writeBigUInt64BE(this.k); this.buf = crypto.createHash('sha256').update(Buffer.concat([this.s, kb])).digest(); this.k += 1n; }
    const v = this.buf.readUInt32BE(0); this.buf = this.buf.subarray(4); return v;
  }
  r(n) { return this.u32() % n; }
}
class Pairs { constructor(items) { this.items = items || []; } }
function esc(s) {
  let out = '';
  for (const ch of s) {
    const c = ch.codePointAt(0);
    if (ch === '"') out += '\\"'; else if (ch === '\\') out += '\\\\';
    else if (c < 0x20) out += '\\u' + c.toString(16).padStart(4, '0');
    else out += ch;
  }
  return '"' + out + '"';
}
function ser(v) {
  if (v === null) return 'null';
  if (v === true) return 'true';
  if (v === false) return 'false';
  if (typeof v === 'number') return String(v);
  if (typeof v === 'string') return esc(v);
  if (v instanceof Pairs) return '{' + v.items.map(([k, x]) => esc(k) + ':' + ser(x)).join(',') + '}';
  if (Array.isArray(v)) return '[' + v.map(ser).join(',') + ']';
  return '{' + Object.keys(v).map((k) => esc(k) + ':' + ser(v[k])).join(',') + '}';
}
const msg = (i, kind, method, params) => ser(new Pairs([['id', i], ['kind', kind], ['payload', new Pairs([['method', method], ['params', params]])]]));
const hex8 = (n) => n.toString(16).padStart(8, '0');
function gen(p, depth) {
  const c = p.r(8);
  if (c === 0) return p.r(1000);
  if (c === 1) return SPEC.strs[p.r(SPEC.strs.length)];
  if (c === 2) return p.r(2) === 0;
  if (c === 3) return null;
  if (c === 4) { let s = '0x'; for (let i = 0; i < 5; i++) s += hex8(p.u32()); return s; }
  if (c === 5) { let s = '0x'; for (let i = 0; i < 8; i++) s += hex8(p.u32()); return s; }
  if (c === 6) { if (depth >= 3) return null; const n = p.r(3); const a = []; for (let i = 0; i < n; i++) a.push(gen(p, depth + 1)); return a; }
  if (depth >= 3) return null;
  const n = p.r(3); const out = new Pairs();
  for (let i = 0; i < n; i++) { const k = SPEC.keys[p.r(SPEC.keys.length)]; out.items.push([k, gen(p, depth + 1)]); }
  return out;
}
function mutate(p, name, m) {
  if (m === 0) return name.replace(/[a-z]/g, (ch) => ch.toUpperCase());
  if (m === 1) return name + ' ';
  if (m === 2) return name + '\u0000';
  if (m === 3) { const i = name.indexOf('a'); return i < 0 ? name : name.slice(0, i) + 'а' + name.slice(i + 1); }
  if (m === 4) return name.slice(0, -1);
  return SPEC.proto[p.r(SPEC.proto.length)];
}
function template(j, m) {
  const i = j + 1; const bp = '{"method":"eth_blockNumber","params":[]}';
  switch (m) {
    case 0: { const t = msg(i, 'rpc_read', 'eth_blockNumber', []); return t.slice(0, Math.floor(t.length / 2)); }
    case 1: return `{"id":${i},"kind":"rpc_read","payload":[${bp},{"method":"eth_sendRawTransaction","params":[]}]}`;
    case 2: return `{"id":${i},"kind":"rpc_read","payload":${bp},"x":0}`;
    case 3: return `{"id":${i},"kind":"rpc_read","payload":{"method":"eth_blockNumber","params":[],"jsonrpc":"2.0"}}`;
    case 4: return `{"id":${i},"kind":"rpc_read","payload":{"method":"eth_blockNumber"}}`;
    case 5: return `{"id":${i},"kind":"rpc_read","payload":{"method":"eth_blockNumber","method":"eth_sendRawTransaction","params":[]}}`;
    case 6: return `{"id":"${i}","kind":"rpc_read","payload":${bp}}`;
    case 7: return `{"id":-1,"kind":"rpc_read","payload":${bp}}`;
    case 8: return `{"id":4294967296,"kind":"rpc_read","payload":${bp}}`;
    case 9: return `{"id":1.5,"kind":"rpc_read","payload":${bp}}`;
    case 10: return `{"id":${i},"kind":"rpc_read","payload":{"method":"eth_call","params":[[[[[[[]]]]]]]}}`;
    default: { const head = `{"id":${i},"kind":"rpc_read","payload":{"method":"eth_call","params":["`; const tail = '"]}}'; return head + 'a'.repeat(65537 - head.length - tail.length) + tail; }
  }
}
function httpsUrl(s) {
  for (let i = 0; i < s.length; i++) { const c = s.charCodeAt(i); if (c < 0x21 || c > 0x7e) return false; }
  if (s.length > 2048 || !s.startsWith('https://')) return false;
  let u; try { u = new URL(s); } catch { return false; }
  return u.protocol === 'https:' && u.hostname !== '' && u.username === '' && u.password === '';
}

const p = new Prng(seed(SPEC.seedIndex));
const h = crypto.createHash('sha256');
const urls = {};
for (let j = 0; j < SPEC.count; j++) {
  let text, p0 = null;
  if (j % 50 === 49) {
    const kl = [1, 255, 256, 257][p.r(4)]; const vl = [0, 1, 61439, 61440, 61441][p.r(5)];
    text = msg(j + 1, 'storage_set', 'site_storageSet', ['k'.repeat(kl), 'a'.repeat(vl)]);
  } else if (p.r(10) === 0) {
    text = template(j, p.r(12));
  } else {
    const r5 = p.r(5); const id = r5 === 0 ? 0 : (r5 === 1 ? 4294967295 : j + 1);
    const kind = p.r(10) < 8 ? SPEC.validKinds[p.r(4)] : SPEC.badKinds[p.r(6)];
    let method;
    if (SPEC.validKinds.includes(kind) && p.r(2) === 0) { const lst = SPEC.matrixMethods[kind]; method = lst[p.r(lst.length)]; }
    else method = SPEC.names[p.r(SPEC.names.length)];
    if (p.r(4) === 0) method = mutate(p, method, p.r(6));
    let params;
    if (own(SPEC.samples, method) && p.r(2) === 0) params = SPEC.samples[method];
    else { const n = p.r(4); params = []; for (let i = 0; i < n; i++) params.push(gen(p, 1)); }
    if (method === 'site_openExternal' && params.length && typeof params[0] === 'string') p0 = params[0];
    text = msg(id, kind, method, params);
  }
  if (p0 !== null && !own(urls, p0)) urls[p0] = httpsUrl(p0);
  h.update(Buffer.from(text, 'utf8')); h.update('\n');
}
const out = { node: process.version, seedIndex: SPEC.seedIndex, count: SPEC.count, streamSha256: h.digest('hex'), httpsUrlOracle: urls };
fs.mkdirSync(path.join(pkg, 'results'), { recursive: true });
fs.writeFileSync(path.join(pkg, 'results', 'br16-node.json'), JSON.stringify(out, null, 2) + '\n');
process.stdout.write(JSON.stringify({ streamSha256: out.streamSha256, urls: Object.keys(urls).length }) + '\n');
