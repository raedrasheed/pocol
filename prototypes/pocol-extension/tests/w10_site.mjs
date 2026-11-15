// Sites W10/W11: the test site the v3 publish->serve suites publish (pocold/tests/e2e_site.py with the Rust CLI, and
// the browser suites). One generator, so the CLI suite and the browser suites publish the same shape:
//   index.html linking style.css (an @font-face on font.woff2), a classic app.js, an ES module graph
//   main.mjs -> lib.mjs -> util.mjs plus a literal import('./dyn.mjs'), data.json, logo.svg, logo.png and font.woff2
//   (a real WOFF2 font with one glyph, made here: Chrome's font sanitizer must accept it for the "font loaded" check).
// Options: version (shown in the page), maxFile (adds max.txt of exactly SITE_FILE_MAX = 1,048,576 bytes), padBytes
// (adds pad-<i>.js files of up to 1 MiB each, pseudo-random so no chunk repeats), seed (changes every padded byte).
//   node tests/w10_site.mjs OUT_DIR [--version N] [--max-file] [--pad BYTES] [--seed S]   (prints {files, bytes})
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { brotliCompressSync, deflateSync, constants as Z } from 'node:zlib';
import { fileURLToPath } from 'node:url';

export const SITE_FILE_MAX = 1048576, SITE_MAX = 4 * 1048576, FILES_MAX = 256, CHUNK_MAX = 24575;

const u16 = (v) => { const b = Buffer.alloc(2); b.writeUInt16BE(v & 0xffff); return b; };
const i16 = (v) => { const b = Buffer.alloc(2); b.writeInt16BE(v); return b; };
const u32 = (v) => { const b = Buffer.alloc(4); b.writeUInt32BE(v >>> 0); return b; };
const cat = (...a) => Buffer.concat(a.flat());

/** A minimal TrueType font: .notdef (empty) and 'A' (a 500 x 700 box), family "PoColW10". */
export function ttf() {
  const glyphA = cat(i16(1), i16(50), i16(0), i16(550), i16(700), u16(3), u16(0), Buffer.from([1, 1, 1, 1]),
    i16(50), i16(0), i16(500), i16(0), i16(0), i16(700), i16(0), i16(-700));
  const tables = {
    head: cat(u32(0x00010000), u32(0x00010000), u32(0), u32(0x5f0f3cf5), u16(0x000b), u16(1000), u32(0), u32(0), u32(0), u32(0),
      i16(0), i16(0), i16(600), i16(700), u16(0), u16(8), i16(2), i16(0), i16(0)),
    hhea: cat(u32(0x00010000), i16(800), i16(-200), i16(0), u16(600), i16(0), i16(0), i16(600), i16(1), i16(0), i16(0), i16(0), i16(0), i16(0), i16(0), i16(0), u16(2)),
    maxp: cat(u32(0x00010000), u16(2), u16(4), u16(1), u16(0), u16(0), u16(2), u16(0), u16(0), u16(0), u16(0), u16(0), u16(0), u16(0), u16(0)),
    'OS/2': cat(u16(4), i16(600), u16(400), u16(5), u16(0), i16(650), i16(600), i16(0), i16(75), i16(650), i16(600), i16(0), i16(350), i16(50), i16(250),
      i16(0), Buffer.alloc(10), u32(1), u32(0), u32(0), u32(0), Buffer.from('NONE'), u16(0x40), u16(0x41), u16(0x41), i16(800), i16(-200), i16(0),
      u16(800), u16(200), u32(1), u32(0), i16(500), i16(700), u16(0), u16(32), u16(1)),
    hmtx: cat(u16(600), i16(0), u16(600), i16(50)),
    cmap: cat(u16(0), u16(1), u16(3), u16(1), u32(12),
      u16(4), u16(32), u16(0), u16(4), u16(4), u16(1), u16(0), u16(0x41), u16(0xffff), u16(0), u16(0x41), u16(0xffff), u16(1 - 0x41), u16(1), u16(0), u16(0)),
    loca: cat(u16(0), u16(0), u16(glyphA.length / 2)),
    glyf: glyphA,
    post: cat(u32(0x00030000), u32(0), i16(-100), i16(50), u32(0), u32(0), u32(0), u32(0), u32(0)),
  };
  const names = [[1, 'PoColW10'], [2, 'Regular'], [3, 'PoColW10-Regular'], [4, 'PoColW10 Regular'], [5, 'Version 1.0'], [6, 'PoColW10-Regular']];
  const strs = names.map(([, s]) => Buffer.from(s, 'utf16le').swap16());
  let off = 0; const recs = names.map(([id], i) => { const r = cat(u16(3), u16(1), u16(0x409), u16(id), u16(strs[i].length), u16(off)); off += strs[i].length; return r; });
  tables.name = cat(u16(0), u16(names.length), u16(6 + 12 * names.length), ...recs, ...strs);
  return tables;
}
const base128 = (n) => { const out = [n & 0x7f]; n >>>= 7; while (n) { out.unshift(0x80 | (n & 0x7f)); n >>>= 7; } return Buffer.from(out); };
// WOFF2 known-table indices (W3C WOFF2 §5.1); glyf and loca use transform version 3 (null transform).
const KNOWN = { cmap: 0, head: 1, hhea: 2, hmtx: 3, maxp: 4, name: 5, 'OS/2': 6, post: 7, glyf: 10, loca: 11 };
export function woff2() {
  const tables = ttf(); const order = ['cmap', 'head', 'hhea', 'hmtx', 'maxp', 'name', 'OS/2', 'post', 'glyf', 'loca'];
  const dir = cat(...order.map((t) => cat(Buffer.from([KNOWN[t] | (t === 'glyf' || t === 'loca' ? 0xc0 : 0)]), base128(tables[t].length))));
  const data = brotliCompressSync(cat(...order.map((t) => tables[t])), { params: { [Z.BROTLI_PARAM_QUALITY]: 11, [Z.BROTLI_PARAM_MODE]: Z.BROTLI_MODE_FONT } });
  const sfnt = 12 + 16 * order.length + order.reduce((a, t) => a + Math.ceil(tables[t].length / 4) * 4, 0);
  const body = cat(dir, data); const pad = (4 - ((48 + body.length) % 4)) % 4; const total = 48 + body.length + pad;
  const header = cat(Buffer.from('wOF2'), u32(0x00010000), u32(total), u16(order.length), u16(0), u32(sfnt), u32(data.length), u16(1), u16(0), u32(0), u32(0), u32(0), u32(0), u32(0));
  return cat(header, body, Buffer.alloc(pad));
}
/** The 2x2 PNG of the browser suites. */
export function png2x2() {
  const t = []; for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; }
  const crc = (b) => { let x = 0xffffffff; for (const v of b) x = t[(x ^ v) & 255] ^ (x >>> 8); return (x ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => { const td = Buffer.concat([Buffer.from(type), data]); return cat(u32(data.length), td, u32(crc(td))); };
  const raw = Buffer.from([0, 255, 0, 0, 0, 255, 0, 0, 0, 0, 255, 255, 255, 0]);
  return cat(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', Buffer.from([0, 0, 0, 2, 0, 0, 0, 2, 8, 2, 0, 0, 0])), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0)));
}
/** Deterministic pseudo-random printable bytes (xorshift32), so that no 24,575-byte chunk repeats. */
export function noise(n, seed) {
  const out = Buffer.alloc(n); let x = (seed * 2654435761) >>> 0 || 1;
  for (let i = 0; i < n; i++) { x ^= x << 13; x >>>= 0; x ^= x >>> 17; x ^= x << 5; x >>>= 0; out[i] = 0x61 + (x % 26); }
  return out;
}
export function w10Site({ version = 1, maxFile = false, padBytes = 0, seed = 1 } = {}) {
  const files = {
    'index.html': `<!doctype html><html><head><meta charset="utf-8"><title>PoCol W10 site v${version}</title><link rel="stylesheet" href="style.css"></head><body>
<h1 id="t">PoCol W10 site v${version}</h1><p id="font">A</p><img id="png" src="logo.png" alt="png"><img id="svg" src="logo.svg" alt="svg">
<script src="app.js"></script><script type="module" src="main.mjs"></script></body></html>`,
    'style.css': `@font-face{font-family:PoColW10;src:url(font.woff2) format("woff2")}\nbody{background:rgb(244, 246, 248)}\nh1{color:rgb(17, 170, 85)}\n#font{font-family:PoColW10,monospace;font-size:40px}\n`,
    'app.js': `document.getElementById('t').dataset.loaded='yes';
fetch('data.json').then(r=>r.json()).then(j=>{document.body.dataset.rows=String(j.rows.length)});
document.fonts.load('40px PoColW10').then(f=>{document.body.dataset.font=String(f.length)},e=>{document.body.dataset.font='error:'+e.name});`,
    'main.mjs': `import { lib } from './lib.mjs';\ndocument.body.dataset.mod = lib();\nimport('./dyn.mjs').then((m) => { document.body.dataset.dyn = m.dyn; });\n`,
    'lib.mjs': `import { util } from './util.mjs';\nexport const lib = () => 'lib>' + util();\n`,
    'util.mjs': `export const util = () => 'util v${version}';\n`,
    'dyn.mjs': `export const dyn = 'dynamic';\n`,
    'data.json': JSON.stringify({ version, rows: Array.from({ length: 1400 }, (_, i) => ({ i, v: 'x'.repeat(40) })) }),
    'logo.svg': '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#1a5"/></svg>',
    'logo.png': png2x2(),
    'font.woff2': woff2(),
  };
  if (maxFile) files['max.txt'] = noise(SITE_FILE_MAX, seed * 7919); // the same bytes in every version of one seed
  for (let i = 0, left = padBytes; left > 0; i++, left -= SITE_FILE_MAX) files[`pad-${i}.js`] = Buffer.concat([Buffer.from('//'), noise(Math.min(left, SITE_FILE_MAX) - 2, seed * 104729 + version * 31 + i)]);
  return files;
}
export function writeSiteFiles(dir, files) {
  let bytes = 0;
  for (const [p, c] of Object.entries(files)) { const f = join(dir, p); mkdirSync(dirname(f), { recursive: true }); writeFileSync(f, c); bytes += Buffer.byteLength(c); }
  return { files: Object.keys(files).length, bytes };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const a = process.argv.slice(2), opt = (k, d) => (a.includes(k) ? a[a.indexOf(k) + 1] : d);
  if (!a[0] || a[0].startsWith('--')) { console.error('usage: node tests/w10_site.mjs OUT_DIR [--version N] [--max-file] [--pad BYTES] [--seed S]'); process.exit(2); }
  console.log(JSON.stringify(writeSiteFiles(a[0], w10Site({ version: Number(opt('--version', 1)), maxFile: a.includes('--max-file'), padBytes: Number(opt('--pad', 0)), seed: Number(opt('--seed', 1)) }))));
}
