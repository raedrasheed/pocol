// httpsUrl reference oracle (M1 draft 0.5, R5-02, C15). Specification tooling only.
// Usage: node m1-draft-0.5/tools/url_oracle.cjs
// Reads ../vectors/url-cases.json and ../../m1-draft-0.4/vectors/bridge-check-cases.json
// (every httpsUrl value used by the bridge fixtures), evaluates the FD:L1165
// predicate with Node's native WHATWG URL, and writes ../results/url-oracle-0.5.json.
// It records the oracle's verdict; it does not compare with author expectations
// (the Python runner does that).
'use strict';
const fs = require('node:fs');
const path = require('node:path');

const here = __dirname;
const pkg = path.resolve(here, '..');
const cases = JSON.parse(fs.readFileSync(path.join(pkg, 'vectors', 'url-cases.json'), 'utf8'));

function predicate(s) {
  const out = { input: s };
  if (typeof s !== 'string') { out.accept = false; out.reason = 'type'; return out; }
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c < 0x21 || c > 0x7e) { out.accept = false; out.reason = 'printable'; return out; }
  }
  if (s.length > 2048) { out.accept = false; out.reason = 'length'; return out; }
  if (!s.startsWith('https://')) { out.accept = false; out.reason = 'prefix'; return out; }
  let u;
  try { u = new URL(s); } catch (e) { out.accept = false; out.reason = 'parse'; out.error = String(e.code || e.name); return out; }
  out.protocol = u.protocol; out.hostname = u.hostname; out.username = u.username; out.password = u.password;
  out.accept = u.protocol === 'https:' && u.hostname !== '' && u.username === '' && u.password === '';
  if (!out.accept) out.reason = 'properties';
  return out;
}

const inputs = [];
for (const c of cases.cases) inputs.push(c.input);
for (const c of cases.constructed) {
  const k = c.construction;
  inputs.push(k.prefix + k.repeat.repeat(k.totalLength - k.prefix.length));
}
// Every httpsUrl-typed value in the 0.4 bridge fixtures and the 0.4/0.5 matrix sample.
const bridge = JSON.parse(fs.readFileSync(path.resolve(pkg, '..', 'm1-draft-0.4', 'vectors', 'bridge-check-cases.json'), 'utf8'));
for (const c of bridge.cases) {
  const m = c.message;
  if (m && m.payload && m.payload.method === 'site_openExternal' && Array.isArray(m.payload.params)) {
    for (const p of m.payload.params) if (typeof p === 'string') inputs.push(p);
  }
}
inputs.push('https://example.org/');

const seen = new Set();
const results = [];
for (const s of inputs) { if (!seen.has(s)) { seen.add(s); results.push(predicate(s)); } }
const out = { oracle: 'Node WHATWG URL', node: process.version, predicate: cases.predicate, results };
fs.mkdirSync(path.join(pkg, 'results'), { recursive: true });
fs.writeFileSync(path.join(pkg, 'results', 'url-oracle-0.5.json'), JSON.stringify(out, null, 2) + '\n');
process.stdout.write(JSON.stringify({ node: process.version, values: results.length }) + '\n');
