// G3a: the off-node verifier side of a spec-2 (v3) genesis on the final CP schema v2 (59 fields). Since phase 4 of the
// fresh network the release build decodes it at every shipped entry point (refdec.js SPEC_VERSION 2, as pocold's
// genesis_spec.rs), and RP verifies its window with the one v3 predicate (rp.js v3Active), whose rules RP cannot
// evaluate and therefore names. A spec-1 genesis still decodes (CP schema v1).
import { readFileSync, mkdtempSync, mkdirSync, cpSync, writeFileSync, rmSync, existsSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { rpWindow } from './rp_fixture.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const unhex = (s) => Uint8Array.from((s.match(/../g) || []).map((x) => parseInt(x, 16)));

export async function v3DecoderChecks(rel) {
  const checks = [], ok = (name, pass, detail = '') => checks.push({ name, pass: !!pass, detail });
  const L = (dist, m) => import(pathToFileURL(join(dist, 'lib', m)).href + '?' + dist);
  const corpus = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-corpus-v3.json'), 'utf8')).items;
  const item = (label) => unhex(corpus.find((x) => x.label === label).hex);
  const v3Base = item('v3/base');
  const gs = (f) => { try { f(); return 'ok'; } catch (e) { return e.gs ? `${e.gs}:${e.detail}` : 'THROW:' + e.message; } };

  // ---- the release build: spec 2 is the release GenesisSpec ----
  const RP = await L(rel, 'rp.js'), D = await L(rel, 'refdec.js'), F = await L(rel, 'forks.js');
  ok('release: SPEC_VERSION and the shipped entry-point ceiling are 2; MAX_FORK_VERSION is unchanged', D.DECODE_CEILING === 2 && D.SPEC_VERSION === 2 && F.MAX_FORK_VERSION === 2);
  ok('release: RP.decodeSpec and the decoder default decode a v3 genesis; only an explicit spec-1 ceiling refuses it with gsVersion',
    gs(() => RP.decodeSpec(v3Base)) === 'ok' && gs(() => D.decodeGenesisSpec(v3Base)) === 'ok' && gs(() => RP.decodeSpec(v3Base, 1)) === 'gsVersion:2');
  ok('release: the manifest is the release one', JSON.parse(readFileSync(join(rel, 'manifest.json'), 'utf8')).name === 'PoCol Studio, IDE & Viewer');

  // ---- v3Active: active_version(h) >= 3 || specVersion >= 2 ----
  const spec1Bytes = unhex(readFileSync(join(HERE, 'data', 'devnet-spec.hex'), 'utf8').trim().replace(/^0x/, ''));
  const spec1 = RP.decodeSpec(spec1Bytes);
  const spec2 = RP.decodeSpec(v3Base);
  ok('release: a spec-1 genesis still decodes, with CP schema v1', spec1.specVersion === 1 && spec1.cp.length === 52);
  const v1 = F.normalizeForkSchedule(), v2 = F.normalizeForkSchedule([[1, 0], [2, '5']]), v3 = F.normalizeForkSchedule([[1, 0], [2, '5'], [3, '9']], 3);
  ok('v3Active: a spec-2 genesis is v3 from height 0 under any schedule', [0n, 1n, 1n << 63n].every((h) => RP.v3Active(spec2, v1, h) && RP.v3Active(spec2, v2, h)));
  ok('v3Active: a spec-1 genesis is not v3 under v1 or v2', [0n, 4n, 5n, 1n << 63n].every((h) => !RP.v3Active(spec1, v1, h) && !RP.v3Active(spec1, v2, h)));
  ok('v3Active: a spec-1 genesis becomes v3 at a fork-3 activation (test bound only; MAX_FORK_VERSION stays 2)', !RP.v3Active(spec1, v3, 8n) && RP.v3Active(spec1, v3, 9n) && RP.v3Active(spec1, v3, 10n));

  // ---- RP over a v3 chain's window (the corpus's v3 headers come from the same fixture) ----
  const win2 = await rpWindow(rel, v3Base, 14);
  const r2 = await RP.verifyWindow(win2.rpc, spec2, win2.lastTs + 1, [[1, '0']]);
  ok('a v3 window verifies with the unchanged items 1-9 and says v3 is active', r2.ok && r2.v3 === true && r2.n === 13 && /Consensus v3 is active: its membership, fork-choice, evidence and reward rules are not evaluated by RP$/.test(r2.message), r2.message);
  const win1 = await rpWindow(rel, spec1Bytes, 14);
  const r1 = await RP.verifyWindow(win1.rpc, win1.spec, win1.lastTs + 1);
  ok('a spec-1 window result is unchanged (no v3 field, the release message)', r1.ok && !('v3' in r1) && Object.keys(r1).join() === 'ok,n,toGenesis,minWork,head,message'
    && r1.message === `RP: work and identity bound for 13 headers, with computed work of at least ${r1.minWork} per header (work(ceilTarget)), without execution or membership verification`, r1.message);
  // The v3 window's headers are bound to the v3 genesis: verified against the spec-1 genesis they fail netGenesis.
  let cross = ''; try { await RP.verifyWindow(win2.rpc, { ...spec1, chainId: spec2.chainId }, win2.lastTs + 1); } catch (e) { cross = e.flag; }
  ok('a v3 window does not verify against another genesis', cross === 'netGenesis', cross);
  // A spec-2 window under a header version the local schedule does not activate is netVersion, as for spec 1.
  let premature = ''; const win2b = await rpWindow(rel, v3Base, 3, (h, ut) => { if (h === 3) ut[6] = Uint8Array.of(2); });
  try { await RP.verifyWindow(win2b.rpc, spec2, win2b.lastTs + 1, [[1, '0']]); } catch (e) { premature = e.flag; }
  ok('a v3 window keeps the fork-version check (netVersion)', premature === 'netVersion', premature);

  // The source modules stay plain JavaScript: src/lib/refdec.js parses as it is.
  const parsed = spawnSync(process.execPath, ['--check', join(HERE, '..', 'src', 'lib', 'refdec.js')], { encoding: 'utf8' });
  ok('src/lib/refdec.js parses outside the build', parsed.status === 0, parsed.stderr);

  // ---- the v3 genesis at the release entry points ----
  // The v3 program set's sysCodeHash, recomputed from the bundled programs (apply.rs SystemPrograms::v3).
  const C = await L(rel, 'crypto.js'), R = await L(rel, 'rlp.js');
  const system = JSON.parse(readFileSync(join(HERE, '..', '..', 'lp4-execution', 'tests', 'data', 'system-v3.json'), 'utf8'));
  const v3Hash = C.keccak(R.encode([...system.programs.map((p) => unhex(p.runtime.replace(/^0x/, ''))), new Uint8Array(0)]));
  ok('release: RP.decodeSpec decodes the v3 genesis (the final CP schema v2, the v3 program set) by default', spec2.specVersion === 2 && spec2.cp.length === 59 && D.CP_V2_LEN === 59
    && RP.cpv(spec2, 'rho_bp') === 2500n && RP.cpv(spec2, 'baseFeeMin') === 10000000n && spec2.sysCodeHash.every((x, i) => x === v3Hash[i]) && v3Hash.length === 32);
  // The deleted fields are no CP names of either schema.
  ok('release: the fields FN-2a and FN-2b deleted are not CP fields', ['P_MAX', 'T_PROB', 'F0_acct', 'F0_slot', 'F0_code', 'E_epoch', 'theta_bp', 'k_sigma'].every((n) => D.cpIndexFor(2, n) === -1 && gs(() => RP.cpv(spec2, n)) !== 'ok'));
  ok('release: the size bound is MAX_SPEC_BYTES of CP schema v2 (2,818,562); the spec-1 bound is smaller', D.MAX_SPEC_BYTES === D.maxSpecBytes(2) && D.MAX_SPEC_BYTES === 2818562 && D.maxSpecBytes(1) < D.MAX_SPEC_BYTES
    && gs(() => RP.decodeSpec(new Uint8Array(D.MAX_SPEC_BYTES + 1))) === `InputTooLarge:${D.MAX_SPEC_BYTES + 1}`);
  // Every v3-corpus spec item: the release entry point gives the Rust release verdict.
  const want = JSON.parse(readFileSync(join(HERE, 'data', 'decoder-verdicts-v3.json'), 'utf8'));
  const bad = corpus.filter((it) => it.kind === 'spec').filter((it) => gs(() => RP.decodeSpec(unhex(it.hex))) !== want[it.label]).map((it) => it.label);
  ok('release: RP.decodeSpec gives the Rust verdicts on the v3 corpus', bad.length === 0, bad.slice(0, 5).join(', '));

  // ---- the release packager packages the release and refuses a stale dist that does not decode spec 2 ----
  const tmp = mkdtempSync(join(tmpdir(), 'pocol-pkg-'));
  try {
    mkdirSync(join(tmp, 'scripts'));
    cpSync(join(HERE, '..', 'scripts', 'package-release.py'), join(tmp, 'scripts', 'package-release.py'));
    writeFileSync(join(tmp, 'package.json'), readFileSync(join(HERE, '..', 'package.json')));
    const pack = (edit = null) => {
      rmSync(join(tmp, 'dist'), { recursive: true, force: true }); rmSync(join(tmp, 'out.zip'), { force: true });
      cpSync(rel, join(tmp, 'dist'), { recursive: true });
      if (edit) { const f = join(tmp, 'dist', 'lib', 'refdec.js'); writeFileSync(f, edit(readFileSync(f, 'utf8'))); }
      // Sites W3: the packager refuses the retired network the tree still pins (in NETWORK and in every page or README
      // that shows it); a release identity stands in for it here, as deploy/mainnet/set_network_identity.py would pin it.
      const retiredFile = join(HERE, '..', '..', '..', 'deploy', 'mainnet', 'retired-networks.json');
      const old = JSON.parse(readFileSync(retiredFile, 'utf8')).retired[0];
      const n = join(tmp, 'dist', 'lib', 'studio-network.js');
      writeFileSync(n, readFileSync(n, 'utf8').replace(/"chainId": \d+/, '"chainId": 4242').replace(/"genesisHash": "0x[0-9a-f]{64}"/, `"genesisHash": "0x${'42'.repeat(32)}"`));
      const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]));
      for (const f of walk(join(tmp, 'dist')).filter((f) => /\.(js|html|md|json|txt)$/.test(f))) {
        const t = readFileSync(f, 'utf8'), u = t.replaceAll(String(old.chainId), '4242').replaceAll(old.genesisHash.slice(2), '42'.repeat(32));
        if (u !== t) writeFileSync(f, u);
      }
      return spawnSync('python3', ['-B', join(tmp, 'scripts', 'package-release.py'), join(tmp, 'out.zip'), '--retired', retiredFile], { encoding: 'utf8' });
    };
    const packed = pack();
    ok('the release packager packages the release build', packed.status === 0 && existsSync(join(tmp, 'out.zip')), packed.stderr);
    const stale = pack((t) => t.replace('export const SPEC_VERSION = 2,', 'export const SPEC_VERSION = 1,'));
    ok('the release packager refuses a stale dist whose decoder does not decode spec 2', stale.status !== 0 && /does not decode spec 2/.test(stale.stderr) && !existsSync(join(tmp, 'out.zip')), stale.stderr);
  } finally { rmSync(tmp, { recursive: true, force: true }); }
  return checks;
}
