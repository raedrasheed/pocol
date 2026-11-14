// A07/A08: the extension's GenesisSpec and header decoders give exactly the accepted Rust
// decoders' verdicts (code and detail) on the whole corpus, and the shipped RP entry points use them.
// Twice: the release decoder (default ceiling SPEC_VERSION, decoder-verdicts.json, which refuses spec 2 until
// activation) and the v3-preview decoder (ceiling 2, decoder-verdicts-preview.json, the CP v2 schema). The spec-2
// supplement (decoder-corpus-v3.json, G3a) is checked the same way against decoder-verdicts-v3.json {release, preview}.
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
  const wantPreview = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-verdicts-preview.json'), 'utf8'));
  const verdict = (kind, b, maxVersion) => {
    try { if (kind === 'spec') { if (maxVersion === undefined) D.decodeGenesisSpec(b); else D.decodeGenesisSpec(b, maxVersion); } else D.decodeHeaderRef(b); return 'ok'; }
    catch (e) { if (kind === 'spec') return e.gs ? `${e.gs}:${e.detail}` : 'THROW:' + e.message; return e.flag === '1' ? `1:${e.detail}` : 'THROW:' + e.message; }
  };
  const mismatches = [], previewMismatches = []; let accepted = 0, previewAccepted = 0, spec2 = 0;
  for (const it of corpus) {
    const b = unhex(it.hex);
    const got = verdict(it.kind, b); if (got === 'ok') accepted++; if (got !== want[it.label]) mismatches.push(`${it.label}: js=${got} ref=${want[it.label]}`);
    const pre = verdict(it.kind, b, 2); if (pre === 'ok') previewAccepted++; if (pre !== wantPreview[it.label]) previewMismatches.push(`${it.label}: js preview=${pre} ref=${wantPreview[it.label]}`);
    if (it.label.startsWith('devnet-v2/')) spec2++;
  }
  const labels = (o, items = corpus) => Object.keys(o).length === items.length && items.every((it) => it.label in o);
  // The spec-2 supplement: both ceilings, and the release decoder accepts no spec-2 item there either.
  const v3 = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-corpus-v3.json'), 'utf8')).items;
  const wantV3 = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-verdicts-v3.json'), 'utf8'));
  const v3Mismatches = []; let v3Accepted = 0, v3Spec2Release = 0;
  for (const it of v3) {
    const b = unhex(it.hex);
    const r = verdict(it.kind, b), p = verdict(it.kind, b, 2);
    if (r !== wantV3.release[it.label]) v3Mismatches.push(`${it.label}: js=${r} ref=${wantV3.release[it.label]}`);
    if (p !== wantV3.preview[it.label]) v3Mismatches.push(`${it.label}: js preview=${p} ref=${wantV3.preview[it.label]}`);
    if (it.kind === 'spec' && p === 'ok') { v3Accepted++; if (D.decodeGenesisSpec(b, 2).specVersion === 2 && r === 'ok') v3Spec2Release++; }
  }
  return { items: corpus.length, accepted, mismatches, previewAccepted, previewMismatches, spec2,
    v3Items: v3.length, v3Accepted, v3Mismatches, v3Complete: labels(wantV3.release, v3) && labels(wantV3.preview, v3) && v3Spec2Release === 0,
    // Both verdict files cover exactly the corpus, and the release decoder accepts no spec-2 item.
    complete: labels(want) && labels(wantPreview) && corpus.every((it) => !(it.label.startsWith('devnet-v2/') && want[it.label] === 'ok')),
    entryPoints: RP.decodeHeader === D.decodeHeaderRef && RP.decodeSpec.toString().includes('decodeGenesisSpec') };
}
if (process.argv[1] && process.argv[1].endsWith('decoder_diff.mjs')) {
  const { build } = await import('../build.mjs'); const r = await decoderDiffChecks(build().dist);
  console.log(JSON.stringify({ items: r.items, accepted: r.accepted, mismatches: r.mismatches.length, previewAccepted: r.previewAccepted, previewMismatches: r.previewMismatches.length, spec2: r.spec2, complete: r.complete, entryPoints: r.entryPoints,
    v3Items: r.v3Items, v3Accepted: r.v3Accepted, v3Mismatches: r.v3Mismatches.length, v3Complete: r.v3Complete }));
  for (const m of [...r.mismatches, ...r.previewMismatches, ...r.v3Mismatches].slice(0, 40)) console.log(m);
  process.exit(r.mismatches.length || r.previewMismatches.length || r.v3Mismatches.length || !r.complete || !r.v3Complete || !r.entryPoints ? 1 : 0);
}
