// Sites W2b: the module specifier scanner shared by the sandbox loader and the publish-time check.
// Scanner vectors (tests/data/modscan-vectors.json, also for a port to the pocol-site CLI), adversarial inputs,
// rewriting (the output must still parse), the publish-time graph check, and the build's inlining into the loader.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as M from '../src/lib/modscan.js';
import { inlineModscan } from '../build.mjs';

const vectors = JSON.parse(readFileSync(new URL('./data/modscan-vectors.json', import.meta.url), 'utf8'));
const enc = new TextEncoder();
const scan = (src) => { const r = M.scanModule(src); return { statics: r.statics.map((x) => x.spec), dynamics: r.dynamics.map((x) => x.spec), meta: r.meta }; };
/** Parses `src` as a module with node --check (the rewritten output must stay valid JavaScript). */
function parsesAsModule(src) {
  const dir = mkdtempSync(join(tmpdir(), 'pocol-modscan-'));
  try { writeFileSync(join(dir, 'm.mjs'), src); return spawnSync(process.execPath, ['--check', join(dir, 'm.mjs')], { encoding: 'utf8' }); }
  finally { rmSync(dir, { recursive: true, force: true }); }
}

test('scanner vectors: every specifier found, none invented', () => {
  assert.ok(vectors.length >= 40, String(vectors.length));
  for (const v of vectors) assert.deepEqual(scan(v.src), { statics: v.statics, dynamics: v.dynamics, meta: v.meta ?? 0 }, v.name);
});
test('the scanner never throws and stays linear on adversarial input', () => {
  const big = 1 << 20; // a whole file at the 1 MiB limit
  for (const src of ['('.repeat(big), 'import('.repeat(big / 7), 'import a,'.repeat(big / 9), 'export {'.repeat(big / 8), '`${'.repeat(big / 3), '/'.repeat(big), '"\\'.repeat(big / 2), '/*'.repeat(big / 2), "'".repeat(big), 'import'.repeat(big / 6), 'a/b/c/'.repeat(big / 6), '\u2028/'.repeat(big / 2)]) {
    const t0 = process.hrtime.bigint(); M.scanModule(src); M.rewriteModule(src, '/a.js', () => 'blob:x');
    const ms = Number(process.hrtime.bigint() - t0) / 1e6; assert.ok(ms < 10000, `${JSON.stringify(src.slice(0, 12))}: ${ms} ms`);
  }
  for (const src of ['', 'import', 'import(', 'import("', 'export * from', '`${', '}', ')))', '/[/', 'import x from', 'import.meta', '\\u0069mport("./a.js")']) assert.doesNotThrow(() => M.rewriteModule(src, '/a.js', () => 'blob:x'));
});
test('rewriting replaces static specifiers by their URL and import() by the runtime call; the result still parses', () => {
  const src = `import a, { b as c } from './a.js';\nimport * as ns from "../lib/b.mjs";\nimport './side.js';\nexport * from './c.js';\nexport { d } from '/d.js';\nexport { c };\nconst re = /import\\('\\.\\/x.js'\\)/g, s = "import './no.js'";\n// import './comment.js'\nconst t = \`import('./tpl.js') \${await import('./inner.js')}\`;\nexport const lazy = () => import('./lazy.js');\nexport const computed = (n) => import(n, { with: { type: 'json' } });\nconst o = { import(x) { return x; } };\nconsole.log(import.meta.url, o, re, s, t, ns, a);\n`;
  const seen = [];
  const out = M.rewriteModule(src, '/app/main.js', (spec) => { seen.push(spec); return 'blob:null/' + seen.length; });
  assert.deepEqual(seen, ['./a.js', '../lib/b.mjs', './side.js', './c.js', '/d.js']);
  for (let i = 1; i <= 5; i++) assert.ok(out.includes(`"blob:null/${i}"`), out);
  assert.ok(out.includes(`${M.RUNTIME_IMPORT}("/app/main.js",'./inner.js')`) && out.includes(`${M.RUNTIME_IMPORT}("/app/main.js",'./lazy.js')`) && out.includes(`${M.RUNTIME_IMPORT}("/app/main.js",n, { with`), out);
  assert.ok(out.includes("import('./tpl.js')") && out.includes("\"import './no.js'\"") && out.includes("// import './comment.js'") && out.includes('{ import(x) { return x; } }'), out);
  const r = parsesAsModule(out); assert.equal(r.status, 0, r.stderr);
  // Rewriting the output again changes nothing (the loader keeps blob: specifiers; the runtime call is a plain call).
  assert.equal(M.rewriteModule(out, '/app/main.js', (spec) => (M.resolveSpecifier(spec, '/').keep ? null : 'blob:other')), out);
  // A JSON file is imported as a JavaScript module: its import attributes go with it.
  assert.equal(M.rewriteModule("import d from './d.json' with { type: 'json' };\nimport c from './c.css' with { type: 'css' };", '/m.js', (spec) => (spec.endsWith('.json') ? { url: 'blob:j', dropAttributes: true } : 'blob:c')),
    'import d from "blob:j" ;\nimport c from "blob:c" with { type: \'css\' };');
  // A classic script: import() only.
  const classic = M.rewriteModule("var x = import('./a.js'); var y = 'import(\"./b.js\")';", '/c.js', () => { throw new Error('no statics for a classic script'); }, { statics: false });
  assert.equal(classic, `var x = ${M.RUNTIME_IMPORT}("/c.js",'./a.js'); var y = 'import("./b.js")';`);
  // Every vector's rewrite of a valid module still parses.
  for (const v of vectors.filter((x) => x.module)) { const o = M.rewriteModule(v.src, '/v/m.js', () => 'blob:null/x'); const c = parsesAsModule(o); assert.equal(c.status, 0, v.name + ': ' + c.stderr + '\n' + o); }
});
test('specifier resolution: site paths, bare names, URLs, blob:/data:', () => {
  assert.deepEqual(M.resolveSpecifier('./x.js', '/a/b.js'), { path: '/a/x.js' });
  assert.deepEqual(M.resolveSpecifier('../x.js?v=1#h', '/a/b/c.js'), { path: '/a/x.js' });
  assert.deepEqual(M.resolveSpecifier('/x.js', '/a/b.js'), { path: '/x.js' });
  assert.deepEqual(M.resolveSpecifier('../../../x.js', '/a.js'), { path: '/x.js' });
  assert.deepEqual(M.resolveSpecifier('react', '/a.js'), { bare: true });
  assert.deepEqual(M.resolveSpecifier('x.js', '/a.js'), { bare: true });
  for (const u of ['https://cdn.example/x.js', '//cdn.example/x.js', 'node:fs']) assert.deepEqual(M.resolveSpecifier(u, '/a.js'), { external: true });
  for (const u of ['blob:null/1', 'data:text/javascript,1']) assert.deepEqual(M.resolveSpecifier(u, '/a.js'), { keep: true });
});

const site = (o) => Object.entries(o).map(([path, text]) => ({ path, data: enc.encode(text) }));
test('publish check: a multi-module site with JSON and CSS module imports passes', () => {
  const r = M.checkModuleGraph(site({ '/index.html': '<script type="module" src="main.mjs"></script>', '/main.mjs': "import { lib } from './lib/lib.mjs'; import data from './data.json' with { type: 'json' }; import sheet from './s.css' with { type: 'css' }; lib(data, sheet);",
    '/lib/lib.mjs': "export { util as lib } from './util.js'; export * from '../util.js';", '/lib/util.js': 'export const util = () => 1;', '/util.js': 'export const u = 2;', '/data.json': '{}', '/s.css': 'body{}' }));
  assert.deepEqual(r, { errors: [], warnings: [], modules: 4 });
});
test('publish check: the code-split bundle shape (a lazy chunk importing back from its entry) passes', () => {
  const r = M.checkModuleGraph(site({ '/index.html': '<script type="module" src="/assets/index-a1.js"></script>',
    '/assets/index-a1.js': "import { v as vendor } from './vendor-c3.js'; export const shared = vendor(); document.body.onclick = () => import('./page-b2.js').then((m) => m.show());",
    '/assets/page-b2.js': "import { shared } from './index-a1.js'; import { v } from './vendor-c3.js'; export const show = () => console.log(shared, v);",
    '/assets/vendor-c3.js': 'export const v = () => 42;' }));
  assert.deepEqual(r.errors, []); assert.equal(r.modules, 3);
});
test('publish check: a static cycle is refused with a clear error naming the cycle', () => {
  const r = M.checkModuleGraph(site({ '/index.html': '', '/a.js': "import './b.js';", '/b.js': "import './c.js';", '/c.js': "import { x } from './a.js';" }));
  assert.equal(r.errors.length, 1);
  assert.match(r.errors[0], /^module import cycle: \/a\.js -> \/b\.js -> \/c\.js -> \/a\.js \(blob: URLs cannot express a cycle of static imports/);
  assert.throws(() => M.assertModuleGraph(site({ '/a.js': "import './a.js';" })), (e) => e.code === 'MODULE_GRAPH' && /cycle: \/a\.js -> \/a\.js/.test(e.message));
});
test('publish check: unresolvable static specifiers are errors; computed and external import() are warnings', () => {
  const r = M.checkModuleGraph(site({ '/a.js': "import 'react'; import x from 'https://cdn.example/x.js'; import './missing.js'; import p from './p.png'; import('./gone.js'); import(name); import('lodash'); console.log(import.meta.url);", '/p.png': 'x' }));
  assert.equal(r.errors.length, 5, JSON.stringify(r.errors));
  assert.match(r.errors[0], /"react" cannot load/); assert.match(r.errors[1], /"https:\/\/cdn\.example\/x\.js" cannot load/);
  assert.match(r.errors[2], /names \/missing\.js, which is not in the site/); assert.match(r.errors[3], /\/p\.png, which is not a module/);
  assert.match(r.errors[4], /import\("\.\/gone\.js"\) names \/gone\.js/);
  assert.equal(r.warnings.length, 3, JSON.stringify(r.warnings));
  assert.match(r.warnings.join('\n'), /computed specifier/); assert.match(r.warnings.join('\n'), /"lodash"\) is blocked/); assert.match(r.warnings.join('\n'), /import\.meta\.url is a blob: URL/);
  assert.throws(() => M.assertModuleGraph(site({ '/a.js': "import 'react'; import 'vue';" })), /"react" cannot load.*\(and 1 more\)/);
});
test(`publish check: static imports may nest ${M.MODULE_DEPTH_MAX} deep, not deeper`, () => {
  const chain = (n) => site(Object.fromEntries(Array.from({ length: n + 1 }, (_, i) => [`/m${i}.js`, i < n ? `import './m${i + 1}.js';` : 'export {};'])));
  assert.deepEqual(M.checkModuleGraph(chain(M.MODULE_DEPTH_MAX)).errors, []);
  const deep = M.checkModuleGraph(chain(M.MODULE_DEPTH_MAX + 1)).errors;
  assert.equal(deep.length, 1); assert.match(deep[0], new RegExp(`nest ${M.MODULE_DEPTH_MAX + 1} deep, more than ${M.MODULE_DEPTH_MAX}`));
});
test('publish check: only .js/.mjs files are scanned; non-UTF-8 bytes do not throw', () => {
  assert.deepEqual(M.checkModuleGraph(site({ '/index.html': "<script>import 'x'</script>", '/t.txt': "import 'react'" })).errors, []);
  assert.doesNotThrow(() => M.checkModuleGraph([{ path: '/a.js', data: Uint8Array.of(0xff, 0xfe, 0x69) }]));
});
test('the build inlines modscan into the loader once, exports frozen, and refuses a missing placeholder', () => {
  const src = readFileSync(new URL('../src/lib/modscan.js', import.meta.url), 'utf8');
  const out = inlineModscan('const MS = /*@modscan*/null;\nresult = MS;', src);
  const box = {}; new Function('result', out.replace('result = MS;', 'box.MS = MS;').replace('const MS', 'var MS').replace(/^/, 'var box = this;\n')).call(box);
  assert.ok(Object.isFrozen(box.MS)); assert.equal(box.MS.MODULE_DEPTH_MAX, M.MODULE_DEPTH_MAX); assert.equal(box.MS.RUNTIME_IMPORT, M.RUNTIME_IMPORT);
  assert.deepEqual(Object.keys(box.MS).sort(), Object.keys(M).sort());
  assert.deepEqual(box.MS.scanModule("import './a.js'").statics.map((s) => s.spec), ['./a.js']);
  assert.throws(() => inlineModscan('no placeholder', src), /exactly once/);
  assert.throws(() => inlineModscan('/*@modscan*/null /*@modscan*/null', src), /exactly once/);
  assert.throws(() => inlineModscan('/*@modscan*/null', "import x from './y.js';\n" + src), /self-contained/);
});
test('Studio carries a byte-identical copy of the scanner', () => {
  assert.equal(readFileSync(new URL('../../pocol-studio/dist/modscan.js', import.meta.url), 'utf8'), readFileSync(new URL('../src/lib/modscan.js', import.meta.url), 'utf8'));
});
