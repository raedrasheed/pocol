// V3-PR-G3a: the off-node verifier side of a spec-2 (v3) genesis. The release build keeps SPEC_VERSION semantics at
// every shipped entry point (a spec-2 genesis is refused with gsVersion until activation, V3-PR-I); the test-only
// v3-preview build (build.mjs --v3-preview, as pocold --features v3-preview) decodes it and RP verifies its window
// with the one v3 predicate (rp.js v3Active), whose rules RP cannot evaluate and therefore names.
import { readFileSync, mkdtempSync, mkdirSync, cpSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { build } from '../build.mjs';
import { rpWindow } from './rp_fixture.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const unhex = (s) => Uint8Array.from((s.match(/../g) || []).map((x) => parseInt(x, 16)));

export async function v3PreviewChecks(rel) {
  const checks = [], ok = (name, pass, detail = '') => checks.push({ name, pass: !!pass, detail });
  const L = (dist, m) => import(pathToFileURL(join(dist, 'lib', m)).href + '?' + dist);
  const corpus = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-corpus-v3.json'), 'utf8')).items;
  const item = (label) => unhex(corpus.find((x) => x.label === label).hex);
  const v3Base = item('v3/base');
  const gs = (f) => { try { f(); return 'ok'; } catch (e) { return e.gs ? `${e.gs}:${e.detail}` : 'THROW:' + e.message; } };

  // ---- the release build ----
  const RP = await L(rel, 'rp.js'), D = await L(rel, 'refdec.js'), F = await L(rel, 'forks.js');
  ok('release: the shipped entry point keeps the SPEC_VERSION ceiling', D.DECODE_CEILING === 1 && D.SPEC_VERSION === 1 && F.MAX_FORK_VERSION === 2);
  ok('release: RP.decodeSpec refuses a v3 genesis with gsVersion, an explicit ceiling of 2 decodes it', gs(() => RP.decodeSpec(v3Base)) === 'gsVersion:2' && gs(() => RP.decodeSpec(v3Base, 2)) === 'ok');
  ok('release: the manifest is the release one', JSON.parse(readFileSync(join(rel, 'manifest.json'), 'utf8')).name === 'PoCol Studio, IDE & Viewer');

  // ---- v3Active: active_version(h) >= 3 || specVersion >= 2 ----
  const spec1 = RP.decodeSpec(unhex(readFileSync(join(HERE, 'data', 'devnet-spec.hex'), 'utf8').trim().replace(/^0x/, '')));
  const spec2 = RP.decodeSpec(v3Base, 2);
  const v1 = F.normalizeForkSchedule(), v2 = F.normalizeForkSchedule([[1, 0], [2, '5']]), v3 = F.normalizeForkSchedule([[1, 0], [2, '5'], [3, '9']], 3);
  ok('v3Active: a spec-2 genesis is v3 from height 0 under any schedule', [0n, 1n, 1n << 63n].every((h) => RP.v3Active(spec2, v1, h) && RP.v3Active(spec2, v2, h)));
  ok('v3Active: a spec-1 genesis is not v3 under v1 or v2', [0n, 4n, 5n, 1n << 63n].every((h) => !RP.v3Active(spec1, v1, h) && !RP.v3Active(spec1, v2, h)));
  ok('v3Active: a spec-1 genesis becomes v3 at a fork-3 activation (test bound only; MAX_FORK_VERSION stays 2)', !RP.v3Active(spec1, v3, 8n) && RP.v3Active(spec1, v3, 9n) && RP.v3Active(spec1, v3, 10n));

  // ---- RP over a v3 chain's window (the corpus's v3 headers come from the same fixture) ----
  const win2 = await rpWindow(rel, v3Base, 14, undefined, 2);
  const r2 = await RP.verifyWindow(win2.rpc, spec2, win2.lastTs + 1, [[1, '0']]);
  ok('a v3 window verifies with the unchanged items 1-9 and says v3 is active', r2.ok && r2.v3 === true && r2.n === 13 && /Consensus v3 is active: its membership, fork-choice, evidence and reward rules are not evaluated by RP$/.test(r2.message), r2.message);
  const win1 = await rpWindow(rel, unhex(readFileSync(join(HERE, 'data', 'devnet-spec.hex'), 'utf8').trim().replace(/^0x/, '')), 14);
  const r1 = await RP.verifyWindow(win1.rpc, win1.spec, win1.lastTs + 1);
  ok('a spec-1 window result is unchanged (no v3 field, the release message)', r1.ok && !('v3' in r1) && Object.keys(r1).join() === 'ok,n,toGenesis,minWork,head,message'
    && r1.message === `RP: work and identity bound for 13 headers, with computed work of at least ${r1.minWork} per header (work(ceilTarget)), without execution or membership verification`, r1.message);
  // The v3 window's headers are bound to the v3 genesis: verified against the spec-1 genesis they fail netGenesis.
  let cross = ''; try { await RP.verifyWindow(win2.rpc, { ...spec1, chainId: spec2.chainId }, win2.lastTs + 1); } catch (e) { cross = e.flag; }
  ok('a v3 window does not verify against another genesis', cross === 'netGenesis', cross);
  // A spec-2 window under a header version the local schedule does not activate is netVersion, as for spec 1.
  let premature = ''; const win2b = await rpWindow(rel, v3Base, 3, (h, ut) => { if (h === 3) ut[6] = Uint8Array.of(2); }, 2);
  try { await RP.verifyWindow(win2b.rpc, spec2, win2b.lastTs + 1, [[1, '0']]); } catch (e) { premature = e.flag; }
  ok('a v3 window keeps the fork-version check (netVersion)', premature === 'netVersion', premature);

  // ---- the test-only v3-preview build ----
  let both = ''; try { build({ fault: 'noDnr', v3Preview: true }); } catch (e) { both = e.message; }
  ok('a fault build is never a v3-preview build', /never a v3-preview build/.test(both));
  const pre = build({ v3Preview: true }).dist;
  try {
    const PRP = await L(pre, 'rp.js'), PD = await L(pre, 'refdec.js'), PF = await L(pre, 'forks.js');
    ok('v3-preview: built to its own directory with a marked manifest', pre.endsWith('dist-v3-preview') && /\[V3 PREVIEW, NOT FOR RELEASE\]$/.test(JSON.parse(readFileSync(join(pre, 'manifest.json'), 'utf8')).name));
    ok('v3-preview: the entry point ceiling is 2; SPEC_VERSION, the decoder default and MAX_FORK_VERSION are unchanged', PD.DECODE_CEILING === 2 && PD.SPEC_VERSION === 1 && PF.MAX_FORK_VERSION === 2
      && gs(() => PD.decodeGenesisSpec(v3Base)) === 'gsVersion:2');
    const s = PRP.decodeSpec(v3Base);
    ok('v3-preview: RP.decodeSpec decodes the v3 genesis (CP schema v2, the v3 program set) by default', s.specVersion === 2 && s.cp.length === 67 && PRP.cpv(s, 'rho_bp') === 2500n);
    ok('v3-preview: the size bound is the preview MAX_SPEC_BYTES', gs(() => PRP.decodeSpec(new Uint8Array(PD.maxSpecBytes(2) + 1))) === `InputTooLarge:${PD.maxSpecBytes(2) + 1}` && gs(() => RP.decodeSpec(new Uint8Array(PD.MAX_SPEC_BYTES + 1))) === `InputTooLarge:${PD.MAX_SPEC_BYTES + 1}`);
    // Every v3-corpus spec item: the preview entry point gives the Rust preview verdict, the release one the release verdict.
    const want = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-verdicts-v3.json'), 'utf8'));
    const bad = corpus.filter((it) => it.kind === 'spec').filter((it) => {
      const b = unhex(it.hex);
      return gs(() => PRP.decodeSpec(b)) !== want.preview[it.label] || gs(() => RP.decodeSpec(b)) !== want.release[it.label];
    }).map((it) => it.label);
    ok('v3-preview and release entry points give the Rust verdicts of their builds on the v3 corpus', bad.length === 0, bad.slice(0, 5).join(', '));
    const rp = await PRP.verifyWindow(win2.rpc, s, win2.lastTs + 1, [[1, '0']]);
    ok('v3-preview: RP verifies the v3 window from the default-decoded spec', rp.ok && rp.v3 === true);
    // The release packager refuses a preview dist placed where the release dist goes.
    const tmp = mkdtempSync(join(tmpdir(), 'pocol-pkg-'));
    try {
      mkdirSync(join(tmp, 'scripts'));
      cpSync(join(HERE, '..', 'scripts', 'package-release.py'), join(tmp, 'scripts', 'package-release.py'));
      writeFileSync(join(tmp, 'package.json'), readFileSync(join(HERE, '..', 'package.json')));
      const pack = (dist) => { rmSync(join(tmp, 'dist'), { recursive: true, force: true }); cpSync(dist, join(tmp, 'dist'), { recursive: true }); return spawnSync('python3', ['-B', join(tmp, 'scripts', 'package-release.py'), join(tmp, 'out.zip')], { encoding: 'utf8' }); };
      const refused = pack(pre);
      ok('the release packager refuses a v3-preview build', refused.status !== 0 && /Refusing a v3-preview build/.test(refused.stderr) && !existsSync(join(tmp, 'out.zip')), refused.stderr);
      const packed = pack(rel);
      ok('the release packager still packages the release build', packed.status === 0 && existsSync(join(tmp, 'out.zip')), packed.stderr);
    } finally { rmSync(tmp, { recursive: true, force: true }); }
  } finally { rmSync(pre, { recursive: true, force: true }); }
  return checks;
}
