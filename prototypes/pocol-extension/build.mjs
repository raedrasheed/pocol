// Build the MV3 extension into dist/ (release) or dist-fault-<name>/ (negative-control
// builds for tests only; browser.md BR-neg: "fault builds are absent from the release").
//   node build.mjs                 -> dist/
//   node build.mjs --fault=noDnr   -> dist-fault-noDnr/
// Fault regions in sources: /*@fault NAME*/release code/*@else*/fault code/*@end*/
import { readFileSync, writeFileSync, mkdirSync, rmSync, readdirSync, statSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
export const FAULTS = ['noDnr', 'cspOpen', 'cspOpenNoDnr', 'noRtc', 'allowAll', 'noRefill', 'quotaNoSubtract'];
const MARK = /\/\*@fault (\w+)\*\/([\s\S]*?)\/\*@else\*\/([\s\S]*?)\/\*@end\*\//g;
export function applyFaults(text, fault) {
  return text.replace(MARK, (m, name, rel, alt) => (name === fault || (fault === 'cspOpenNoDnr' && name === 'noDnr') ? alt : rel));
}
const SANDBOX_CSP = (hash, open) => open
  ? `sandbox allow-scripts allow-forms; script-src 'sha256-${hash}' blob:; style-src * blob: 'unsafe-inline'; img-src * blob: data:; font-src * blob:; media-src * blob:; connect-src *; frame-src *`
  : `sandbox allow-scripts allow-forms; default-src 'none'; script-src 'sha256-${hash}' blob:; style-src blob: 'unsafe-inline'; img-src blob: data:; font-src blob:; media-src blob:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'`;

export function build({ fault = null, out } = {}) {
  if (fault && !FAULTS.includes(fault)) throw new Error('unknown fault ' + fault);
  const dist = out || join(HERE, fault ? `dist-fault-${fault}` : 'dist');
  rmSync(dist, { recursive: true, force: true }); mkdirSync(join(dist, 'lib'), { recursive: true });
  const src = join(HERE, 'src');
  const emit = (rel, text) => { const p = join(dist, rel); mkdirSync(dirname(p), { recursive: true }); writeFileSync(p, text); };
  for (const f of readdirSync(join(src, 'lib'))) emit(join('lib', f), applyFaults(readFileSync(join(src, 'lib', f), 'utf8'), fault));
  for (const f of ['sw.js', 'viewer.html', 'viewer.js', 'viewer.css', 'options.html', 'options.js']) emit(f, applyFaults(readFileSync(join(src, f), 'utf8'), fault));
  // Vendored @noble (MIT): only the modules reached from lib/crypto.js; the one bare specifier is made relative.
  const nh = join(HERE, 'node_modules', '@noble', 'hashes'), ns = join(HERE, 'node_modules', '@noble', 'secp256k1');
  for (const f of ['sha3.js', 'sha2.js', 'hmac.js', 'utils.js', '_md.js', '_u64.js', 'crypto.js'])
    emit(join('vendor', 'hashes', f), readFileSync(join(nh, 'esm', f), 'utf8').replace(/from '@noble\/hashes\/crypto'/g, "from './crypto.js'").replace(/\n\/\/# sourceMappingURL=.*$/m, ''));
  emit(join('vendor', 'hashes', 'LICENSE'), readFileSync(join(nh, 'LICENSE'), 'utf8'));
  emit(join('vendor', 'secp256k1', 'index.js'), readFileSync(join(ns, 'index.js'), 'utf8').replace(/\n\/\/# sourceMappingURL=.*$/m, ''));
  emit(join('vendor', 'secp256k1', 'LICENSE'), readFileSync(join(ns, 'LICENSE'), 'utf8'));
  // Sandbox page: the loader is the only inline script; its hash is the only script hash in the CSP.
  const loader = applyFaults(readFileSync(join(src, 'sandbox-loader.js'), 'utf8'), fault);
  const hash = createHash('sha256').update(loader, 'utf8').digest('base64');
  emit('sandbox.html', `<!doctype html>\n<html><head><meta charset="utf-8"><script>${loader}</script></head><body></body></html>\n`);
  const manifest = {
    manifest_version: 3,
    name: 'PoCol Local Viewer' + (fault ? ` [TEST FAULT ${fault}]` : ''),
    version: JSON.parse(readFileSync(join(HERE, 'package.json'), 'utf8')).version,
    description: 'Verified PoCol on-chain websites from a local devnet node: sandboxed rendering, DNR isolation, BridgeAuth, local wallet.',
    minimum_chrome_version: '130', // chrome.storage.local.getKeys (D103 recovery)
    background: { service_worker: 'sw.js', type: 'module' },
    permissions: ['storage', 'unlimitedStorage', 'declarativeNetRequest'],
    host_permissions: ['http://127.0.0.1/*'],
    optional_host_permissions: ['https://*/*'],
    omnibox: { keyword: 'pocol' },
    options_page: 'options.html',
    action: { default_title: 'PoCol' },
    sandbox: { pages: ['sandbox.html'] },
    content_security_policy: { extension_pages: "script-src 'self'; object-src 'self'", sandbox: SANDBOX_CSP(hash, fault === 'cspOpen' || fault === 'cspOpenNoDnr') },
  };
  emit('manifest.json', JSON.stringify(manifest, null, 2) + '\n');
  // Release hygiene: no fault markers or bare module specifiers survive.
  const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]));
  for (const f of walk(dist).filter((f) => f.endsWith('.js') || f.endsWith('.html'))) {
    const t = readFileSync(f, 'utf8');
    if (/\/\*@(fault|else|end)/.test(t)) throw new Error('fault marker left in ' + f);
    if (/^\s*(import|export)\b[^\n]*\bfrom\s+['"](?![./])/m.test(t) || /^\s*import\s+['"](?![./])/m.test(t)) throw new Error('bare module specifier in ' + f);
  }
  return { dist, loaderHash: hash, manifest };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const fa = process.argv.find((a) => a.startsWith('--fault='));
  const r = build({ fault: fa ? fa.slice(8) : null });
  console.log(JSON.stringify({ dist: r.dist, loaderSha256: r.loaderHash, csp: r.manifest.content_security_policy.sandbox }, null, 1));
  if (!existsSync(join(r.dist, 'vendor', 'hashes', 'sha3.js'))) process.exit(1);
}
