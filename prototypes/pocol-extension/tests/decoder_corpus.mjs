// A07/A08 differential corpus: valid GenesisSpec and header bases (the accepted GSV1 vector, a
// pocold devnet spec, synthetic RP headers) and mutations of every field and of the framing.
// `node tests/decoder_corpus.mjs` writes tests/data/decoder-corpus.json; the Rust reference
// decoders give the verdicts (pocold/tests/decoder_reference.rs, REFERENCE_WRITE=1 writes
// tests/data/decoder-verdicts.json) and tests/decoder_diff.mjs checks the extension against them.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { build } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
export const CORPUS = join(HERE, 'data', 'decoder-corpus.json'), VERDICTS = join(HERE, 'data', 'decoder-verdicts.json');
const hex = (b) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
const unhex = (s) => Uint8Array.from((s.replace(/^0x/, '').match(/../g) || []).map((x) => parseInt(x, 16)));

export async function generate() {
  const dist = build().dist;
  const R = await import(pathToFileURL(join(dist, 'lib', 'rlp.js')).href);
  const { rpWindow } = await import(pathToFileURL(join(HERE, 'rp_fixture.mjs')).href);
  // Tree codec: arrays are lists, Uint8Array leaves are byte strings, {raw} injects bytes verbatim.
  const parse = (b) => R.decode(b, 64);
  const enc = (n) => (n && n.raw ? n.raw : Array.isArray(n) ? R.encode(n.map(encRaw)) : R.encode(n));
  const encRaw = (n) => (n && n.raw ? { raw: n.raw } : n);
  // R.encode cannot embed raw children; re-implement list encoding with raw support.
  const prefix = (base, n) => { if (n < 56) return [base + n]; const l = []; let v = n; while (v) { l.unshift(v & 255); v = Math.floor(v / 256); } return [base + 55 + l.length, ...l]; };
  const encode = (n) => {
    if (n && n.raw) return n.raw;
    if (Array.isArray(n)) { const parts = n.map(encode); const len = parts.reduce((a, p) => a + p.length, 0); const out = new Uint8Array(prefix(0xc0, len).length + len); out.set(prefix(0xc0, len)); let o = prefix(0xc0, len).length; for (const p of parts) { out.set(p, o); o += p.length; } return out; }
    if (n.length === 1 && n[0] < 0x80) return n;
    return Uint8Array.from([...prefix(0x80, n.length), ...n]);
  };
  void enc;
  const clone = (n) => (Array.isArray(n) ? n.map(clone) : n && n.raw ? { raw: n.raw.slice() } : n.slice());
  const at = (t, path, f) => { const c = clone(t); let p = c; for (let i = 0; i < path.length - 1; i++) p = p[path[i]]; p[path.at(-1)] = f(p[path.at(-1)]); return c; };
  const del = (t, path) => { const c = clone(t); let p = c; for (let i = 0; i < path.length - 1; i++) p = p[path[i]]; p.splice(path.at(-1), 1); return c; };
  const be = (v) => { const o = []; let x = BigInt(v); while (x) { o.unshift(Number(x & 0xffn)); x >>= 8n; } return Uint8Array.from(o); };
  const bytes = (n, fill = 0x11) => new Uint8Array(n).fill(fill);
  const items = [];
  const add = (kind, label, b) => items.push({ kind, label, hex: hex(b) });
  const byteMutations = (kind, label, b) => {
    for (const cut of [1, 2, Math.floor(b.length / 2), b.length - 1]) if (cut > 0 && cut < b.length) add(kind, `${label}/truncate-${cut}`, b.subarray(0, cut));
    add(kind, `${label}/trailing-byte`, Uint8Array.from([...b, 0]));
    add(kind, `${label}/empty`, new Uint8Array(0));
    // Long form for a short top-level length prefix is non-canonical framing.
    if (b[0] >= 0xf8) { const ll = b[0] - 0xf7; add(kind, `${label}/length-leading-zero`, Uint8Array.from([b[0] + 1, 0, ...b.subarray(1, 1 + ll), ...b.subarray(1 + ll)])); }
  };

  // ---------------- GenesisSpec ----------------
  const v = JSON.parse(readFileSync(join(HERE, '..', '..', '..', 'development', 'm1', 'm1-draft-0.21', 'vectors', 'v3-gsv1.json'), 'utf8'));
  const gsv1 = unhex(v.flatSegments.map((s) => (typeof s === 'string' ? s : s.repeat.repeat(s.count))).join(''));
  if (gsv1.length !== v.expected.length) throw new Error('GSV1 length');
  const devnet = unhex(readFileSync(join(HERE, 'data', 'devnet-spec.hex'), 'utf8').trim());
  const REFDEC = await import(pathToFileURL(join(dist, 'lib', 'refdec.js')).href);
  const V1 = { schema: REFDEC.CP_SCHEMA, versions: [['0', new Uint8Array(0)], ['2', Uint8Array.of(2)], ['wrapped-1', { raw: Uint8Array.of(0x81, 0x01) }], ['leading-zero-1', Uint8Array.of(0, 1)], ['256', Uint8Array.of(1, 0)]] };
  // Every base gets the same mutations of the framing, the six top items, each CP field of its own schema (zero,
  // the bounds and one past them, the width, non-canonical and list encodings), both roots and M_0List.
  const specMutations = (label, base, { schema, versions, cpCounts = (t) => [[`cp-${schema.length - 1}`, del(t, [2, schema.length - 1])], [`cp-${schema.length + 1}`, at(t, [2], (c) => [...c, Uint8Array.of(1)])]] }) => {
    const t = parse(base); add('spec', `${label}/base`, encode(t));
    byteMutations('spec', label, base);
    add('spec', `${label}/top-string`, encode(bytes(5)));
    add('spec', `${label}/top-5`, encode(t.slice(0, 5))); add('spec', `${label}/top-7`, encode([...t, bytes(1)]));
    for (let i = 0; i < 6; i++) add('spec', `${label}/top[${i}]-kind`, encode(at(t, [i], (x) => (Array.isArray(x) ? bytes(1) : [x]))));
    for (const [n, val] of versions) add('spec', `${label}/version-${n}`, encode(at(t, [0], () => val)));
    for (const [n, val] of [['empty', new Uint8Array(0)], ['9-bytes', bytes(9, 1)], ['leading-zero', Uint8Array.of(0, 5)], ['wrapped', { raw: Uint8Array.of(0x81, 0x05) }], ['max', bytes(8, 0xff)]]) add('spec', `${label}/chainId-${n}`, encode(at(t, [1], () => val)));
    for (const [n, tree] of cpCounts(t)) add('spec', `${label}/${n}`, encode(tree));
    schema.forEach(([name, width, lo, hi], k) => {
      const cp = (val) => encode(at(t, [2, k], () => val));
      add('spec', `${label}/cp.${name}=0`, cp(new Uint8Array(0)));
      if (lo > 0) add('spec', `${label}/cp.${name}=lo-1`, cp(be(lo - 1)));
      if (typeof hi === 'number') { add('spec', `${label}/cp.${name}=hi`, cp(be(hi))); add('spec', `${label}/cp.${name}=hi+1`, cp(be(hi + 1))); }
      if (hi === 'gamma') { add('spec', `${label}/cp.${name}=10000`, cp(be(10000))); }
      add('spec', `${label}/cp.${name}=2^width`, cp(be(1n << BigInt(width))));
      add('spec', `${label}/cp.${name}=2^width-1`, cp(be((1n << BigInt(width)) - 1n)));
      add('spec', `${label}/cp.${name}-leading-zero`, cp(Uint8Array.of(0, 1)));
      add('spec', `${label}/cp.${name}-wrapped`, cp({ raw: Uint8Array.of(0x81, 0x05) }));
      add('spec', `${label}/cp.${name}-list`, cp([Uint8Array.of(1)]));
    });
    for (const [i, name] of [[3, 'allocRoot'], [5, 'sysCodeHash']]) for (const n of [0, 31, 33]) add('spec', `${label}/${name}-${n}`, encode(at(t, [i], () => bytes(n))));
    const m0 = t[4];
    add('spec', `${label}/m0-scalar`, encode(at(t, [4], () => bytes(20))));
    add('spec', `${label}/m0-empty`, encode(at(t, [4], () => [])));
    add('spec', `${label}/m0-entry-1`, encode(at(t, [4, 0], (e) => [e[0]])));
    add('spec', `${label}/m0-entry-3`, encode(at(t, [4, 0], (e) => [...e, e[1]])));
    add('spec', `${label}/m0-entry-scalar`, encode(at(t, [4, 0], () => bytes(20))));
    add('spec', `${label}/m0-entry-nested`, encode(at(t, [4, 0], (e) => [[e[0]], e[1]])));
    for (const n of [19, 21]) { add('spec', `${label}/m0-id-${n}`, encode(at(t, [4, 0, 0], () => bytes(n, 0x31)))); add('spec', `${label}/m0-reward-${n}`, encode(at(t, [4, 0, 1], () => bytes(n, 0x31)))); }
    if (m0.length > 1) { add('spec', `${label}/m0-unsorted`, encode(at(t, [4], (l) => [l[1], l[0], ...l.slice(2)]))); add('spec', `${label}/m0-duplicate`, encode(at(t, [4], (l) => [l[0], l[0], ...l.slice(2)]))); }
    const sys = new Uint8Array(20).fill(0xff); sys[19] = 0xfe; const prog = new Uint8Array(20); prog.set([0xc0, 0xc0, 0x01], 17); const progHi = new Uint8Array(20); progHi.set([0xc0, 0xc0, 0xff], 17); const progOut = new Uint8Array(20); progOut.set([0xc0, 0xc1, 0x00], 17);
    for (const [n, id] of [['zero', new Uint8Array(20)], ['system', sys], ['program-low', prog], ['program-high', progHi], ['just-above-programs', progOut]]) add('spec', `${label}/m0-id-${n}`, encode(at(t, [4], (l) => [[id, l[0][1]]])));
    add('spec', `${label}/long-form-short-list`, Uint8Array.from([0xf8, 2, 0xc1, 0x01]));
  };
  for (const [label, base] of [['gsv1', gsv1], ['devnet', devnet]]) specMutations(label, base, V1);
  // Depth: a CP item replaced by nested lists far below any limit is a structure error.
  { let deep = Uint8Array.of(1); for (let i = 0; i < 40; i++) deep = [deep]; add('spec', 'gsv1/deep-cp-item', encode(at(parse(gsv1), [2, 0], () => deep))); }

  // ---------------- headers ----------------
  const { headers } = await rpWindow(dist, devnet, 3);
  for (const [hi, h] of headers.entries()) {
    const base = unhex(h); const t = parse(base); const label = `header${hi}`;
    add('header', `${label}/base`, encode(t)); byteMutations('header', label, base);
    const ut = (k, val) => encode(at(t, [0, 0, k], () => val));
    const FIXED = { 2: 32, 3: 32, 9: 32, 10: 32, 11: 32, 12: 256, 16: 32 }; const INT = { 1: 8, 4: 8, 5: 4, 6: 2, 7: 8, 8: 32, 13: 8, 14: 8, 15: 32 };
    for (let k = 0; k < 18; k++) {
      add('header', `${label}/ut[${k}]-list`, ut(k, [Uint8Array.of(1)]));
      add('header', `${label}/ut[${k}]-empty`, ut(k, new Uint8Array(0)));
      if (FIXED[k]) { add('header', `${label}/ut[${k}]-${FIXED[k] - 1}`, ut(k, bytes(FIXED[k] - 1))); add('header', `${label}/ut[${k}]-${FIXED[k] + 1}`, ut(k, bytes(FIXED[k] + 1))); }
      if (INT[k]) { add('header', `${label}/ut[${k}]-leading-zero`, ut(k, Uint8Array.of(0, 1))); add('header', `${label}/ut[${k}]-width+1`, ut(k, bytes(INT[k] + 1, 1))); add('header', `${label}/ut[${k}]-max`, ut(k, bytes(INT[k], 0xff))); }
      add('header', `${label}/ut[${k}]-wrapped`, ut(k, { raw: Uint8Array.of(0x81, 0x05) }));
    }
    add('header', `${label}/tag`, ut(0, new TextEncoder().encode('PoCol-tpl-v2')));
    add('header', `${label}/proposer-20`, ut(17, bytes(20, 0x44))); add('header', `${label}/proposer-19`, ut(17, bytes(19, 0x44)));
    add('header', `${label}/ut-17`, encode(at(t, [0, 0], (u) => u.slice(0, 17)))); add('header', `${label}/ut-19`, encode(at(t, [0, 0], (u) => [...u, bytes(1)])));
    add('header', `${label}/st-1`, encode(at(t, [0], (s) => [s[0]]))); add('header', `${label}/st-3`, encode(at(t, [0], (s) => [...s, bytes(1)])));
    add('header', `${label}/top-3`, encode(t.slice(0, 3))); add('header', `${label}/top-5`, encode([...t, bytes(1)]));
    for (const n of [64, 65, 66]) add('header', `${label}/sig-${n}`, encode(at(t, [0, 1], () => bytes(n, 0x55))));
    add('header', `${label}/sig-list`, encode(at(t, [0, 1], () => [bytes(65)])));
    for (const n of [7, 9]) add('header', `${label}/nonce-${n}`, encode(at(t, [1], () => bytes(n))));
    add('header', `${label}/nonce-list`, encode(at(t, [1], () => [bytes(8)])));
    add('header', `${label}/shares-scalar`, encode(at(t, [2], () => bytes(8))));
    add('header', `${label}/share-7`, encode(at(t, [2], () => [bytes(7)])));
    add('header', `${label}/share-list`, encode(at(t, [2], () => [[bytes(8)]])));
    add('header', `${label}/shares-256`, encode(at(t, [2], () => Array.from({ length: 256 }, (_, i) => be(BigInt(i) + (1n << 56n))))));
    add('header', `${label}/shares-257`, encode(at(t, [2], () => Array.from({ length: 257 }, (_, i) => be(BigInt(i) + (1n << 56n))))));
    for (const n of [64, 66]) add('header', `${label}/winnerSig-${n}`, encode(at(t, [3], () => bytes(n))));
    add('header', `${label}/target-zero`, ut(8, new Uint8Array(0)));
    { let deep = Uint8Array.of(1); for (let i = 0; i < 20; i++) deep = [deep]; add('header', `${label}/depth`, ut(14, deep)); }
    add('header', `${label}/oversize`, encode(at(t, [2], () => Array.from({ length: 300 }, (_, i) => be(BigInt(i) + (1n << 56n))))));
  }
  // The audit's malformed fields (A08): 31-byte stateRoot, empty bloom, list gasLimit, together.
  { const t = parse(unhex(headers[0])); const m = at(at(at(t, [0, 0, 9], () => bytes(31)), [0, 0, 12], () => new Uint8Array(0)), [0, 0, 13], () => [Uint8Array.of(1)]); add('header', 'audit-a08/combined', encode(m)); }

  // ---------------- GenesisSpec version 2 (CP schema v2, V3-PR-B) ----------------
  // The devnet spec as a v3 genesis: specVersion 2, nonceMode 1 (testnet.rs Config.v3) and the fifteen appended CP
  // v2 fields at the lp4 devnet defaults (devnet.rs DevnetConfig). The release verdicts (decoder-verdicts.json) refuse
  // every version-2 item with gsVersion until activation (V3-PR-I); decoder-verdicts-preview.json holds the v3-preview
  // decoder's verdicts (decode_bounded at ceiling 2) for the whole corpus.
  const V2_DEVNET = [31557600n, 62500000000000000n, 16n, 648000n, 1000000000000000n, 0n, 0n, 5000n, 10000000n, 4n, 40n, 21600n, 1250n, 3n, 2500n];
  if (V2_DEVNET.length !== REFDEC.CP_V2_EXTRA.length) throw new Error('CP v2 defaults');
  const v1t = parse(devnet);
  const v2t = [Uint8Array.of(2), v1t[1], [...v1t[2].map((x, k) => (k === 4 ? Uint8Array.of(1) : x)), ...V2_DEVNET.map(be)], v1t[3], v1t[4], v1t[5]];
  const devnetV2 = encode(v2t);
  specMutations('devnet-v2', devnetV2, {
    schema: REFDEC.CP_SCHEMA_V2,
    // 1 with 67 CP items is gsCount; 3 has no schema (a future version never inherits v2's layout). The v1 bases'
    // version-2 items are the converse: 52 CP items under version 2 (gsCount in the preview decoder).
    versions: [['0', new Uint8Array(0)], ['1', Uint8Array.of(1)], ['3', Uint8Array.of(3)], ['wrapped-2', { raw: Uint8Array.of(0x81, 0x02) }], ['leading-zero-2', Uint8Array.of(0, 2)], ['255', Uint8Array.of(0xff)], ['256', Uint8Array.of(1, 0)], ['258', Uint8Array.of(1, 2)]],
    cpCounts: (t) => [['cp-66', del(t, [2, 66])], ['cp-68', at(t, [2], (c) => [...c, Uint8Array.of(1)])], ['cp-52', at(t, [2], (c) => c.slice(0, 52))], ['cp-0', at(t, [2], () => [])]],
  });
  // Every extra field at its widest value, and the upper bounds together: the longest CP v2 values still decode.
  add('spec', 'devnet-v2/cp-v2-extra-all-max', encode(at(v2t, [2], (c) => [...c.slice(0, 52), ...REFDEC.CP_V2_EXTRA.map(([, w, , hi]) => (typeof hi === 'number' ? be(hi) : be((1n << BigInt(w)) - 1n)))])));
  add('spec', 'devnet-v2/cp-v2-extra-all-lo', encode(at(v2t, [2], (c) => [...c.slice(0, 52), ...REFDEC.CP_V2_EXTRA.map(([, , lo]) => be(lo))])));
  // Stage order on a v2 spec: a version-2 item whose CP v2 field is out of range and whose M_0List is empty is gsCount.
  add('spec', 'devnet-v2/empty-m0-and-bad-rho', encode(at(at(v2t, [4], () => []), [2, 66], () => be(10001))));
  return items;
}

if (process.argv[1] && process.argv[1].endsWith('decoder_corpus.mjs')) {
  const items = await generate(); mkdirSync(dirname(CORPUS), { recursive: true });
  writeFileSync(CORPUS, JSON.stringify({ note: 'A07/A08 differential corpus; generated by tests/decoder_corpus.mjs', items }, null, 0) + '\n');
  console.log(`wrote ${items.length} items to ${CORPUS}`);
}
