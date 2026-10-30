// Native JavaScript oracle for the C19 id rule (M1 draft 0.6, R6-02). Specification tooling.
// Usage: node m1-draft-0.6/tools/id_oracle_06.cjs  -> m1-draft-0.6/results/id-oracle-0.6.json
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const pkg = path.resolve(__dirname, '..');
const doc = JSON.parse(fs.readFileSync(path.join(pkg, 'vectors', 'id-integer-cases.json'), 'utf8'));
const results = doc.cases.map(({ token }) => {
  const text = doc.messageTemplate.replace('TOKEN', token);
  let v;
  try { v = JSON.parse(text).id; } catch (e) { return { token, parse: 'error' }; }
  const accept = typeof v === 'number' && Number.isFinite(v) && Number.isInteger(v) && v >= 0 && v <= 4294967295;
  return { token, accept, returnedId: accept ? (v === 0 ? 0 : v) : null, replyJson: accept ? JSON.stringify({ id: v }) : null };
});
fs.mkdirSync(path.join(pkg, 'results'), { recursive: true });
fs.writeFileSync(path.join(pkg, 'results', 'id-oracle-0.6.json'), JSON.stringify({ node: process.version, results }, null, 2) + '\n');
process.stdout.write(JSON.stringify({ node: process.version, cases: results.length }) + '\n');
