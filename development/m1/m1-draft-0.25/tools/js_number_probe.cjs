// Optional root probe for CR-E4-01, CR-E4-02 and CID-1 (M1 draft 0.25). NOT executed by the author.
// Node only, no dependency, no network, no file write. Usage from D:\PoCol-Development:
//   node m1-draft-0.25\tools\js_number_probe.cjs
// Prints one JSON object; compare it with the 'jsonNumberReadAs' / 'double' columns of
// representation/cr-e4-01-u64-epoch.json and representation/cid-1-profile-chainid.json.
'use strict';

const literals = ['1', '9007199254740991', '9007199254740992', '9007199254740993', '18446744073709551615', '18446744073709551616'];
const out = { node: process.version, jsonParse: {}, bigintStringify: null, textEncoder: {} };

for (const s of literals) {
  const v = JSON.parse('{"x":' + s + '}').x;
  out.jsonParse[s] = { asBigInt: BigInt(v).toString(), safeInteger: Number.isSafeInteger(v) };
}

try {
  JSON.stringify({ epoch: 1n });
  out.bigintStringify = 'no error';
} catch (e) {
  out.bigintStringify = e.name;
}

const te = new TextEncoder();
const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, '0')).join('');
for (const [name, s] of [['d800', '\ud800'], ['d801', '\ud801'], ['pair1f600', '\ud83d\ude00'], ['reversed', '\ude00\ud83d']]) {
  const b = te.encode(s);
  out.textEncoder[name] = { length: b.length, hex: hex(b) };
}

let sourceAccess = false;
try {
  JSON.parse('{"x":9007199254740993}', function (k, v, ctx) {
    if (k === 'x' && ctx && typeof ctx.source === 'string') sourceAccess = ctx.source;
    return v;
  });
} catch (e) {
  sourceAccess = 'error:' + e.name;
}
out.jsonParseSourceAccess = sourceAccess;

console.log(JSON.stringify(out, null, 2));
