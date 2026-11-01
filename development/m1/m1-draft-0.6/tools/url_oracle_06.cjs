// httpsUrl oracle for draft 0.6 (Node WHATWG URL). Same predicate as m1-draft-0.5/tools/url_oracle.cjs;
// writes only m1-draft-0.6/results/url-oracle-0.6.json. Inputs: the 0.5 URL cases, every
// site_openExternal string in the 0.4 bridge cases, and the BR16 generator's string pool.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const pkg = path.resolve(__dirname, '..');
const root = path.resolve(pkg, '..');
const read = (p) => JSON.parse(fs.readFileSync(p, 'utf8'));

function predicate(s) {
  const out = { input: s };
  for (let i = 0; i < s.length; i++) { const c = s.charCodeAt(i); if (c < 0x21 || c > 0x7e) { out.accept = false; out.reason = 'printable'; return out; } }
  if (s.length > 2048) { out.accept = false; out.reason = 'length'; return out; }
  if (!s.startsWith('https://')) { out.accept = false; out.reason = 'prefix'; return out; }
  let u; try { u = new URL(s); } catch (e) { out.accept = false; out.reason = 'parse'; return out; }
  out.accept = u.protocol === 'https:' && u.hostname !== '' && u.username === '' && u.password === '';
  if (!out.accept) out.reason = 'properties';
  return out;
}
const inputs = [];
const uc = read(path.join(root, 'm1-draft-0.5', 'vectors', 'url-cases.json'));
for (const c of uc.cases) inputs.push(c.input);
for (const c of uc.constructed) { const k = c.construction; inputs.push(k.prefix + k.repeat.repeat(k.totalLength - k.prefix.length)); }
for (const c of read(path.join(root, 'm1-draft-0.4', 'vectors', 'bridge-check-cases.json')).cases) {
  const m = c.message; if (m && m.payload && m.payload.method === 'site_openExternal') for (const v of m.payload.params) if (typeof v === 'string') inputs.push(v);
}
const g = read(path.join(pkg, 'annex', 'br16-generator.json'));
for (const s of g.strs) inputs.push(s);
for (const v of g.samples.site_openExternal) inputs.push(v);
const seen = new Set(); const results = [];
for (const s of inputs) if (!seen.has(s)) { seen.add(s); results.push(predicate(s)); }
fs.mkdirSync(path.join(pkg, 'results'), { recursive: true });
fs.writeFileSync(path.join(pkg, 'results', 'url-oracle-0.6.json'), JSON.stringify({ oracle: 'Node WHATWG URL', node: process.version, results }, null, 2) + '\n');
process.stdout.write(JSON.stringify({ values: results.length }) + '\n');
