// A07/A08: the extension's GenesisSpec and header decoders give exactly the accepted Rust
// decoders' verdicts (code and detail) on the whole corpus, and the shipped RP entry points use them.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const HERE = dirname(fileURLToPath(import.meta.url));
const unhex = (s) => Uint8Array.from((s.match(/../g) || []).map((x) => parseInt(x, 16)));
export async function decoderDiffChecks(dist) {
  // The same module URLs rp.js resolves, so function identity can be compared.
  const D = await import(pathToFileURL(join(dist, 'lib', 'refdec.js')).href);
  const RP = await import(pathToFileURL(join(dist, 'lib', 'rp.js')).href);
  const corpus = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-corpus.json'), 'utf8')).items;
  const want = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-verdicts.json'), 'utf8'));
  const verdict = (kind, b) => {
    try { if (kind === 'spec') D.decodeGenesisSpec(b); else D.decodeHeaderRef(b); return 'ok'; }
    catch (e) { if (kind === 'spec') return e.gs ? `${e.gs}:${e.detail}` : 'THROW:' + e.message; return e.flag === '1' ? `1:${e.detail}` : 'THROW:' + e.message; }
  };
  const mismatches = []; let accepted = 0;
  for (const it of corpus) { const got = verdict(it.kind, unhex(it.hex)); if (got === 'ok') accepted++; if (got !== want[it.label]) mismatches.push(`${it.label}: js=${got} ref=${want[it.label]}`); }
  return { items: corpus.length, accepted, mismatches, entryPoints: RP.decodeHeader === D.decodeHeaderRef && RP.decodeSpec.toString().includes('decodeGenesisSpec') };
}
if (process.argv[1] && process.argv[1].endsWith('decoder_diff.mjs')) {
  const { build } = await import('../build.mjs'); const r = await decoderDiffChecks(build().dist);
  console.log(JSON.stringify({ items: r.items, accepted: r.accepted, mismatches: r.mismatches.length, entryPoints: r.entryPoints }));
  for (const m of r.mismatches.slice(0, 40)) console.log(m);
  process.exit(r.mismatches.length ? 1 : 0);
}
