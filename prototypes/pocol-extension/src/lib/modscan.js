// ES-module specifier scanner for on-chain sites (Sites W2b), shared by the sandbox loader (build.mjs inlines this
// file into sandbox-loader.js) and the publish-time check (checkModuleGraph; Studio carries a byte-identical copy as
// dist/modscan.js). Self-contained: no imports, no globals beyond the language built-ins.
//
// A site's scripts run from blob: URLs, which have no hierarchical base, so a relative specifier inside a module
// cannot resolve. The loader rewrites, before creating a module's blob: URL,
//   - every static `import ... from '<s>'`, `import '<s>'`, `export * [as n] from '<s>'` and `export {...} from '<s>'`
//     whose string-literal specifier names a site file, to that file's blob: URL, building blobs leaf-first;
//   - every `import(<args>)` call to `__pocol_import__(<base path>, <args>)`, which resolves the specifier at run time
//     relative to the importing file and imports the same blob: URL (built leaf-first on demand), so a lazily loaded
//     chunk may import back from the module that loaded it (the default code-split shape of Vite and Rollup).
// A cycle of static imports cannot be expressed with blob: URLs (each URL needs the other first) and is refused with a
// clear error, at publish and at view. A JSON file is imported as a JavaScript module whose default export is its
// parsed value (the sandbox CSP's connect-src 'none' blocks JSON module fetches). Bare specifiers and absolute URLs are left as they are: the sandbox CSP blocks
// them. The tokenizer skips strings, comments, template literals (nested ${...} expressions are scanned as code) and
// regular expression literals; it runs in linear time and never throws on malformed input.
export const MODULE_DEPTH_MAX = 8; // like CSS @import nesting in the loader
export const MODULE_COUNT_MAX = 256; // FILES_MAX: every file of a site may be a module
export const RUNTIME_IMPORT = '__pocol_import__';

const isIdStart = (c) => (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c === '_' || c === '$' || c > '\u007f';
const isIdPart = (c) => isIdStart(c) || (c >= '0' && c <= '9');
const isDigit = (c) => c >= '0' && c <= '9';
const isLineEnd = (c) => c === '\n' || c === '\r' || c === ' ' || c === ' ';
// After these words an expression starts, so `/` begins a regular expression literal.
const BEFORE_EXPR = new Set(['return', 'typeof', 'instanceof', 'in', 'of', 'new', 'delete', 'void', 'throw', 'case', 'do', 'else', 'yield', 'await', 'extends', 'default']);
const PUNCT3 = ['>>>=', '...', '===', '!==', '**=', '<<=', '>>=', '>>>', '&&=', '||=', '??='];
const PUNCT2 = ['=>', '==', '!=', '<=', '>=', '&&', '||', '??', '?.', '++', '--', '+=', '-=', '*=', '/=', '%=', '&=', '|=', '^=', '<<', '>>', '**'];

/** Decodes the body of a string literal (between the quotes); null for an invalid escape. */
function unescape(body) {
  if (!body.includes('\\')) return body;
  let out = '';
  for (let i = 0; i < body.length; i++) {
    const c = body[i]; if (c !== '\\') { out += c; continue; }
    const d = body[++i];
    if (d === undefined) return null;
    if (d === 'n') out += '\n'; else if (d === 't') out += '\t'; else if (d === 'r') out += '\r'; else if (d === 'b') out += '\b';
    else if (d === 'f') out += '\f'; else if (d === 'v') out += '\v'; else if (d === '0' && !isDigit(body[i + 1] || '')) out += '\0';
    else if (d === 'x') { const h = body.slice(i + 1, i + 3); if (!/^[0-9a-fA-F]{2}$/.test(h)) return null; out += String.fromCharCode(parseInt(h, 16)); i += 2; }
    else if (d === 'u') {
      let h;
      if (body[i + 1] === '{') { const j = body.indexOf('}', i); if (j < 0) return null; h = body.slice(i + 2, j); i = j; }
      else { h = body.slice(i + 1, i + 5); i += 4; }
      if (!/^[0-9a-fA-F]{1,6}$/.test(h) || parseInt(h, 16) > 0x10ffff) return null; out += String.fromCodePoint(parseInt(h, 16));
    } else if (d === '\r') { if (body[i + 1] === '\n') i++; } else if (isLineEnd(d)) { /* line continuation */ }
    else if (isDigit(d)) return null; else out += d;
  }
  return out;
}

/** Tokens of `src`: {t: 'id'|'str'|'tpl'|'num'|'re'|'p', v, s, e}; 'str' has v = decoded value (null if invalid),
 *  'tpl' is one whole template with `plain` (no substitutions) and v its raw text when plain. */
export function tokenize(src) {
  const toks = []; const n = src.length; let i = 0; const braces = []; // '{' or '`' (a ${ ... } inside a template)
  const prevSig = () => toks[toks.length - 1];
  const regexAllowed = () => {
    const p = prevSig(); if (!p) return true;
    if (p.t === 'num' || p.t === 'str' || p.t === 'tpl' || p.t === 're') return false;
    if (p.t === 'id') return BEFORE_EXPR.has(p.v);
    return !(p.v === ')' || p.v === ']' || p.v === '++' || p.v === '--');
  };
  // Scans template characters from i (just after ` or after the } closing a ${...}); returns true at a ${.
  let tplStart = -1, tplPlain = true;
  const scanTemplate = () => {
    while (i < n) {
      const c = src[i];
      if (c === '\\') { i += 2; continue; }
      if (c === '`') { i++; toks.push({ t: 'tpl', s: tplStart, e: i, plain: tplPlain, v: tplPlain ? src.slice(tplStart + 1, i - 1) : null }); return false; }
      if (c === '$' && src[i + 1] === '{') { i += 2; tplPlain = false; braces.push('`'); return true; }
      i++;
    }
    toks.push({ t: 'tpl', s: tplStart, e: n, plain: false, v: null }); return false; // unterminated
  };
  const tplStack = []; // [tplStart, tplPlain] of enclosing templates while their ${...} is scanned
  while (i < n) {
    const c = src[i];
    if (c === ' ' || c === '\t' || isLineEnd(c) || c === '\f' || c === '\v' || c === ' ' || c === '﻿') { i++; continue; }
    if (c === '#' && src[i + 1] === '!' && i === 0) { while (i < n && !isLineEnd(src[i])) i++; continue; }
    if (c === '/' && src[i + 1] === '/') { while (i < n && !isLineEnd(src[i])) i++; continue; }
    if (c === '/' && src[i + 1] === '*') { const j = src.indexOf('*/', i + 2); i = j < 0 ? n : j + 2; continue; }
    const s = i;
    if (c === '\'' || c === '"') {
      i++; let ok = true;
      while (i < n && src[i] !== c) { if (src[i] === '\\') { i += src[i + 1] === '\r' && src[i + 2] === '\n' ? 3 : 2; continue; } if (isLineEnd(src[i])) { ok = false; break; } i++; }
      const body = src.slice(s + 1, Math.min(i, n));
      if (ok && i < n) i++; else ok = false;
      toks.push({ t: 'str', s, e: i, v: ok ? unescape(body) : null }); continue;
    }
    if (c === '`') { i++; tplStart = s; tplPlain = true; if (scanTemplate()) tplStack.push([tplStart, tplPlain]); continue; }
    if (c === '}' && braces.length && braces[braces.length - 1] === '`') {
      braces.pop(); i++; [tplStart, tplPlain] = tplStack.pop(); tplPlain = false;
      if (scanTemplate()) tplStack.push([tplStart, tplPlain]); continue;
    }
    if (isIdStart(c) || c === '\\') { i++; while (i < n && (isIdPart(src[i]) || src[i] === '\\')) i++; toks.push({ t: 'id', s, e: i, v: src.slice(s, i) }); continue; }
    if (isDigit(c) || (c === '.' && isDigit(src[i + 1] || ''))) { i++; while (i < n && (isIdPart(src[i]) || src[i] === '.' || ((src[i] === '+' || src[i] === '-') && /[eE]/.test(src[i - 1]) && !/^0[xX]/.test(src.slice(s, i))))) i++; toks.push({ t: 'num', s, e: i }); continue; }
    if (c === '/' && regexAllowed()) {
      let j = i + 1, cls = false, ok = false;
      while (j < n && !isLineEnd(src[j])) {
        const d = src[j];
        if (d === '\\') { j += 2; continue; }
        if (d === '[') cls = true; else if (d === ']') cls = false; else if (d === '/' && !cls) { ok = true; break; }
        j++;
      }
      if (ok) { j++; while (j < n && isIdPart(src[j])) j++; i = j; toks.push({ t: 're', s, e: i }); continue; }
      // No closing / on this line: it was a division after all.
    }
    let v = c;
    for (const p of PUNCT3) if (src.startsWith(p, i)) { v = p; break; }
    if (v.length === 1) for (const p of PUNCT2) if (src.startsWith(p, i)) { v = p; break; }
    if (v === '?.' && isDigit(src[i + 2] || '')) v = '?';
    i += v.length;
    if (v === '{') braces.push('{'); else if (v === '}' && braces.length) braces.pop();
    toks.push({ t: 'p', s, e: i, v });
  }
  return toks;
}

const lit = (tk) => (tk && tk.t === 'str' ? tk.v : tk && tk.t === 'tpl' && tk.plain ? tk.v : null);
const isP = (tk, v) => tk && tk.t === 'p' && tk.v === v;
const isId = (tk, v) => tk && tk.t === 'id' && (v === undefined || tk.v === v);

/** Module references in `src`:
 *  statics: [{s, e, spec, attrs}] the string literal (with quotes) of each static import / export-from, and the
 *  [start, end) of its import attributes clause or null;
 *  dynamics: [{s, open, close, spec}] each import(...) call: `import` at s, its ( at open and ) at close; spec is the
 *  literal first argument or null when it is computed;
 *  meta: the number of import.meta uses. */
export function scanModule(src) {
  const toks = tokenize(String(src)); const statics = [], dynamics = []; let meta = 0;
  // The ) matching each ( in one pass, so the scan stays linear for any input.
  const closeOf = new Map(); { const open = []; for (let j = 0; j < toks.length; j++) { if (isP(toks[j], '(')) open.push(j); else if (isP(toks[j], ')') && open.length) closeOf.set(open.pop(), j); } }
  const matchClose = (k) => (closeOf.has(k) ? closeOf.get(k) : -1);
  const fromAfter = (k) => (isId(toks[k], 'from') && toks[k + 1] && toks[k + 1].t === 'str' && toks[k + 1].v !== null ? toks[k + 1] : null);
  // A static specifier, with the range of its import attributes (` with { type: 'json' }`, or the older `assert`).
  const pushStatic = (f) => {
    const at = toks.indexOf(f, k); let attrs = null; const w = toks[at + 1];
    if ((isId(w, 'with') || isId(w, 'assert')) && isP(toks[at + 2], '{')) {
      for (let j = at + 3; j < toks.length && j < at + 64; j++) { if (isP(toks[j], '}')) { attrs = [w.s, toks[j].e]; break; } if (isP(toks[j], '{')) break; }
    }
    statics.push({ s: f.s, e: f.e, spec: f.v, attrs });
  };
  let k = 0;
  for (k = 0; k < toks.length; k++) {
    const tk = toks[k]; if (tk.t !== 'id') continue;
    const prev = toks[k - 1];
    if (prev && (isP(prev, '.') || isP(prev, '?.'))) continue; // a property named import/export
    if (tk.v === 'import') {
      const nx = toks[k + 1];
      if (isP(nx, '.')) { if (isId(toks[k + 2], 'meta')) meta++; continue; }
      if (isP(nx, '(')) {
        const close = matchClose(k + 1);
        if (close < 0) continue;
        if (isP(toks[close + 1], '{') || isP(toks[close + 1], '=>')) continue; // a method or function named import
        const a = toks[k + 2], after = toks[k + 3];
        const spec = lit(a) !== null && (isP(after, ')') || isP(after, ',')) ? lit(a) : null;
        dynamics.push({ s: tk.s, open: nx.s, close: toks[close].s, spec }); continue;
      }
      if (nx && nx.t === 'str') { if (nx.v !== null) pushStatic(nx); continue; }
      // import <bindings> from '<s>': default, {named}, * as ns, and combinations (also `import defer`/`source`).
      let j = k + 1, depth = 0;
      for (; j < toks.length && j < k + 4096; j++) {
        const t = toks[j];
        if (depth === 0 && fromAfter(j)) break;
        if (isId(t, 'import') || isId(t, 'export')) { j = -1; break; } // reserved words: never a binding (keeps the scan linear)
        if (isP(t, '{')) depth++; else if (isP(t, '}')) depth--; else if (t.t === 'id' || isP(t, ',') || isP(t, '*') || (depth > 0 && t.t === 'str')) continue;
        else { j = -1; break; }
        if (depth < 0) { j = -1; break; }
      }
      const f = j > k ? fromAfter(j) : null; if (f) pushStatic(f);
      continue;
    }
    if (tk.v === 'export') {
      const nx = toks[k + 1];
      if (isP(nx, '*')) {
        let j = k + 2; if (isId(toks[j], 'as') && toks[j + 1] && (toks[j + 1].t === 'id' || toks[j + 1].t === 'str')) j += 2;
        const f = fromAfter(j); if (f) pushStatic(f);
      } else if (isP(nx, '{')) {
        let j = k + 2; while (j < toks.length && j < k + 4096 && !isP(toks[j], '}')) { if (isP(toks[j], '{') || isId(toks[j], 'export') || isId(toks[j], 'import')) { j = -1; break; } j++; }
        const f = j > k ? fromAfter(j + 1) : null; if (f) pushStatic(f);
      }
    }
  }
  return { statics, dynamics, meta };
}

/** Resolves a module specifier against the site path `base`: {path} for a site file reference ('./', '../', '/'),
 *  {keep} for blob:/data:, {external} for an absolute URL, {bare} for a bare name, null when unusable. */
export function resolveSpecifier(spec, base) {
  spec = String(spec);
  if (/^(blob|data):/i.test(spec)) return { keep: true };
  if (/^[a-z][a-z0-9+.-]*:/i.test(spec) || spec.startsWith('//')) return { external: true };
  if (!(spec.startsWith('./') || spec.startsWith('../') || spec.startsWith('/'))) return { bare: true };
  let u; try { u = new URL(spec, 'https://pocol.invalid' + base); } catch { return null; }
  if (u.origin !== 'https://pocol.invalid') return { external: true };
  let path; try { path = decodeURIComponent(u.pathname); } catch { return null; }
  return { path };
}

/** Rewrites the module references of `src` (the file at site path `base`). `staticUrl(spec)` returns the URL to use
 *  for a static specifier (or {url, dropAttributes: true} to also drop its import attributes, for a JSON file served
 *  as a JavaScript module), or null to leave it unchanged; `dynamic` (default true) turns every import(...) into
 *  RUNTIME_IMPORT(base, ...). With statics false only import() calls are rewritten (a classic script). */
export function rewriteModule(src, base, staticUrl, { statics = true, dynamic = true } = {}) {
  src = String(src); const r = scanModule(src); const edits = [];
  if (statics) for (const st of r.statics) {
    const u = staticUrl(st.spec), url = typeof u === 'string' ? u : u && typeof u.url === 'string' ? u.url : null;
    if (url === null) continue;
    edits.push([st.s, st.e, JSON.stringify(url)]);
    if (u.dropAttributes && st.attrs) edits.push([st.attrs[0], st.attrs[1], '']);
  }
  if (dynamic) for (const d of r.dynamics) edits.push([d.s, d.open + 1, RUNTIME_IMPORT + '(' + JSON.stringify(base) + ',']);
  edits.sort((a, b) => a[0] - b[0]);
  let out = '', at = 0; for (const [s, e, t] of edits) { if (s < at) continue; out += src.slice(at, s) + t; at = e; }
  return out + src.slice(at);
}

const MODULE_TARGET = /\.(m?js|json|css)$/i;
const isScript = (p) => /\.m?js$/i.test(p);
/**
 * Publish-time check of a site's module graph, before any signature. `files`: [{path, data: Uint8Array|string}].
 * Errors (publishing would produce modules that cannot load): a static specifier that is bare, an absolute URL, or
 * names no site file (or a file that is not .js/.mjs/.json/.css); a literal import() of a site path that does not
 * exist; a cycle of static imports; a static chain deeper than MODULE_DEPTH_MAX.
 * Warnings: a computed import() (checked only at run time), a literal import() of a bare specifier or URL (blocked by
 * the sandbox), import.meta (its url is a blob: URL, so URLs relative to it do not resolve).
 * Returns {errors, warnings, modules}.
 */
export function checkModuleGraph(files) {
  const errors = [], warnings = [], dec = new TextDecoder();
  const paths = new Set(files.map((f) => f.path)); const graph = new Map();
  for (const f of files) {
    if (!isScript(f.path)) continue;
    let text; try { text = typeof f.data === 'string' ? f.data : dec.decode(f.data); } catch { continue; }
    const r = scanModule(text); const deps = [];
    for (const st of r.statics) {
      const t = resolveSpecifier(st.spec, f.path);
      if (!t || !t.path) errors.push(`${f.path}: import of ${JSON.stringify(st.spec)} cannot load in a PoCol site (only ./, ../ or / paths to files of the site)`);
      else if (!paths.has(t.path)) errors.push(`${f.path}: import of ${JSON.stringify(st.spec)} names ${t.path}, which is not in the site`);
      else if (!MODULE_TARGET.test(t.path)) errors.push(`${f.path}: import of ${JSON.stringify(st.spec)} names ${t.path}, which is not a module (.js, .mjs, .json or .css)`);
      else if (isScript(t.path)) deps.push(t.path);
    }
    for (const d of r.dynamics) {
      if (d.spec === null) { warnings.push(`${f.path}: import() with a computed specifier is resolved only when it runs`); continue; }
      const t = resolveSpecifier(d.spec, f.path);
      if (t && t.path && !paths.has(t.path)) errors.push(`${f.path}: import(${JSON.stringify(d.spec)}) names ${t.path}, which is not in the site`);
      else if (!t || !t.path) warnings.push(`${f.path}: import(${JSON.stringify(d.spec)}) is blocked in a PoCol site (no network)`);
    }
    if (r.meta) warnings.push(`${f.path}: import.meta.url is a blob: URL in a PoCol site; URLs relative to it do not resolve`);
    graph.set(f.path, [...new Set(deps)]);
  }
  // Static cycles and depth: depth(p) = the longest chain of static imports below p.
  const state = new Map(), depth = new Map(); const reported = new Set();
  const visit = (p, stack) => {
    if (state.get(p) === 2) return depth.get(p);
    if (state.get(p) === 1) {
      const cyc = [...stack.slice(stack.indexOf(p)), p]; const key = [...cyc].sort().join('\n');
      if (!reported.has(key)) { reported.add(key); errors.push('module import cycle: ' + cyc.join(' -> ') + ' (blob: URLs cannot express a cycle of static imports; break it, for example with import())'); }
      return 0;
    }
    state.set(p, 1); stack.push(p); let d = 0;
    for (const q of graph.get(p) || []) d = Math.max(d, 1 + visit(q, stack));
    stack.pop(); state.set(p, 2); depth.set(p, d);
    return d;
  };
  for (const p of graph.keys()) {
    const d = visit(p, []);
    if (d > MODULE_DEPTH_MAX && !reported.has('depth')) { reported.add('depth'); errors.push(`${p}: static imports nest ${d} deep, more than ${MODULE_DEPTH_MAX}`); }
  }
  return { errors, warnings, modules: graph.size };
}
/** Throws the first module-graph error (with the count of the others), else returns the check's result. */
export function assertModuleGraph(files) {
  const r = checkModuleGraph(files);
  if (r.errors.length) throw Object.assign(new Error(r.errors[0] + (r.errors.length > 1 ? ` (and ${r.errors.length - 1} more)` : '')), { code: 'MODULE_GRAPH', moduleErrors: r.errors });
  return r;
}
