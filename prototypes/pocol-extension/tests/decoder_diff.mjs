// A07/A08: the extension's GenesisSpec and header decoders give exactly the accepted Rust
// decoders' verdicts (code and detail) on the whole corpus, and the shipped RP entry points use them.
// One verdict file per corpus, the release decoder's (default ceiling SPEC_VERSION 2: CP schema v1 under spec 1 and
// CP schema v2 under spec 2): decoder-verdicts.json for decoder-corpus.json, and decoder-verdicts-v3.json for the
// spec-2 supplement decoder-corpus-v3.json (G3a). Both are regenerated from Rust (pocold/tests/decoder_reference.rs).
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const HERE = dirname(fileURLToPath(import.meta.url));
const unhex = (s) => Uint8Array.from((s.match(/../g) || []).map((x) => parseInt(x, 16)));
export async function decoderDiffChecks(dist) {
  // The same module URLs rp.js resolves, so function identity can be compared.
  const D = await import(pathToFileURL(join(dist, 'lib', 'refdec.js')).href);
  const RP = await import(pathToFileURL(join(dist, 'lib', 'rp.js')).href);
  const verdict = (kind, b, maxVersion) => {
    try { if (kind === 'spec') { if (maxVersion === undefined) D.decodeGenesisSpec(b); else D.decodeGenesisSpec(b, maxVersion); } else D.decodeHeaderRef(b); return 'ok'; }
    catch (e) { if (kind === 'spec') return e.gs ? `${e.gs}:${e.detail}` : 'THROW:' + e.message; return e.flag === '1' ? `1:${e.detail}` : 'THROW:' + e.message; }
  };
  const labels = (o, items) => Object.keys(o).length === items.length && items.every((it) => it.label in o);
  // One corpus against its verdict file: the default ceiling, which is the explicit SPEC_VERSION ceiling.
  const run = (corpusFile, verdictFile) => {
    const items = JSON.parse(readFileSync(join(HERE, 'data', corpusFile), 'utf8')).items;
    const want = JSON.parse(readFileSync(join(HERE, 'data', verdictFile), 'utf8'));
    const mismatches = []; let accepted = 0, spec2Accepted = 0;
    for (const it of items) {
      const b = unhex(it.hex);
      const got = verdict(it.kind, b);
      if (got !== want[it.label]) mismatches.push(`${it.label}: js=${got} ref=${want[it.label]}`);
      if (it.kind === 'spec' && verdict(it.kind, b, D.SPEC_VERSION) !== got) mismatches.push(`${it.label}: the default ceiling is not SPEC_VERSION`);
      if (got === 'ok') { accepted++; if (it.kind === 'spec' && D.decodeGenesisSpec(b).specVersion === 2) spec2Accepted++; }
    }
    return { items, mismatches, accepted, spec2Accepted, complete: labels(want, items) };
  };
  const main = run('decoder-corpus.json', 'decoder-verdicts.json');
  const v3 = run('decoder-corpus-v3.json', 'decoder-verdicts-v3.json');
  return { items: main.items.length, accepted: main.accepted, mismatches: main.mismatches, spec2: main.items.filter((it) => it.label.startsWith('devnet-v2/')).length,
    spec2Accepted: main.spec2Accepted, complete: main.complete,
    v3Items: v3.items.length, v3Accepted: v3.accepted, v3Spec2Accepted: v3.spec2Accepted, v3Mismatches: v3.mismatches, v3Complete: v3.complete,
    entryPoints: RP.decodeHeader === D.decodeHeaderRef && RP.decodeSpec.toString().includes('decodeGenesisSpec') && D.DECODE_CEILING === D.SPEC_VERSION && D.SPEC_VERSION === 2 };
}
if (process.argv[1] && process.argv[1].endsWith('decoder_diff.mjs')) {
  const { build } = await import('../build.mjs'); const r = await decoderDiffChecks(build().dist);
  console.log(JSON.stringify({ items: r.items, accepted: r.accepted, mismatches: r.mismatches.length, spec2: r.spec2, spec2Accepted: r.spec2Accepted, complete: r.complete, entryPoints: r.entryPoints,
    v3Items: r.v3Items, v3Accepted: r.v3Accepted, v3Spec2Accepted: r.v3Spec2Accepted, v3Mismatches: r.v3Mismatches.length, v3Complete: r.v3Complete }));
  for (const m of [...r.mismatches, ...r.v3Mismatches].slice(0, 40)) console.log(m);
  process.exit(r.mismatches.length || r.v3Mismatches.length || !r.complete || !r.v3Complete || !r.entryPoints ? 1 : 0);
}
