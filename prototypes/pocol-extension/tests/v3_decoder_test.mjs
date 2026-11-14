// G3a: the off-node verifier side of a spec-2 (v3) genesis on the final CP schema v2 (59 fields). Since phase 4 of the
// fresh network the release build decodes it at every shipped entry point (refdec.js SPEC_VERSION 2, as pocold's
// genesis_spec.rs), and RP verifies its window with the one v3 predicate (rp.js v3Active), whose rules RP cannot
// evaluate and therefore names. A spec-1 genesis still decodes (CP schema v1).
import { readFileSync, mkdtempSync, mkdirSync, cpSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { rpWindow } from './rp_fixture.mjs';
import { applyFaults } from '../build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, '..', '..', '..');
// The committed placeholder invitation (decision D1) and the fixtures set_network_identity.py makes from it.
const PLACEHOLDER_FILE = join(REPO, 'deploy', 'mainnet', 'placeholder-network.pocol.json');
const PLACEHOLDER = JSON.parse(readFileSync(PLACEHOLDER_FILE, 'utf8'));
const FIXTURE = join(HERE, 'identity_fixture.py');
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
  // Its fixtures do not depend on the identity the build pins: the same checks hold for this build and for the build
  // of a tree switched to a network by deploy/mainnet/set_network_identity.py (simulated on a copy of that tree).
  for (const c of packagerChecks(rel, REPO, { spec2, v3Base, C })) checks.push(c);
  const sw = mkdtempSync(join(tmpdir(), 'pocol-switched-'));
  try {
    const root = join(sw, 'root'), dist = join(sw, 'dist');
    const made = spawnSync('python3', ['-B', FIXTURE, 'switch', REPO, root], { encoding: 'utf8' });
    const net = made.status === 0 ? JSON.parse(made.stdout.trim().split('\n').pop()) : null;
    ok('a simulated identity switch of a copy of the tree (set_network_identity.py) succeeds', net !== null, made.stderr);
    if (net) {
      cpSync(rel, dist, { recursive: true });
      const switched = applyFaults(readFileSync(join(root, net.networkFile), 'utf8'), null);
      writeFileSync(join(dist, 'lib', 'studio-network.js'), switched);
      ok('the switched build pins the switched network, without the placeholder mark or identity',
        switched.includes(`"chainId": ${net.chainId},`) && switched.includes(`"genesisHash": "${net.genesisHash}"`)
        && switched.includes(`"genesisPre": "${net.genesisPre}"`) && !switched.includes('NETWORK_PLACEHOLDER')
        && !switched.includes(PLACEHOLDER.files['genesis.spec.hex']));
      for (const c of packagerChecks(dist, root, { spec2, v3Base, C })) checks.push({ ...c, name: 'after a simulated identity switch: ' + c.name });
    }
  } finally { rmSync(sw, { recursive: true, force: true }); }
  return checks;
}

// The release packager over a build `rel` of the tree `root` (the repository, or a switched copy of it): it refuses the
// marked placeholder build and the placeholder identity without its mark, and packages a build switched to a real
// spec-2 network. The marked placeholder build is made from rel's lib/studio-network.js by set_network_identity.py
// --placeholder with the committed placeholder invitation (deploy/mainnet/placeholder-network.pocol.json), whatever
// identity rel pins, so the checks never assume the build still carries the placeholder.
function packagerChecks(rel, root, { spec2, v3Base, C }) {
  const checks = [], ok = (name, pass, detail = '') => checks.push({ name, pass: !!pass, detail });
  // The packager runs in a copy of the repository layout (prototypes/pocol-extension beside deploy/mainnet), so it also
  // reads the placeholder file there.
  const tmp = mkdtempSync(join(tmpdir(), 'pocol-pkg-'));
  try {
    const ext = join(tmp, 'prototypes', 'pocol-extension'), phFile = join(tmp, 'deploy', 'mainnet', 'placeholder-network.pocol.json');
    mkdirSync(join(ext, 'scripts'), { recursive: true }); mkdirSync(dirname(phFile), { recursive: true });
    cpSync(join(HERE, '..', 'scripts', 'package-release.py'), join(ext, 'scripts', 'package-release.py'));
    writeFileSync(join(ext, 'package.json'), readFileSync(join(HERE, '..', 'package.json')));
    const phText = readFileSync(PLACEHOLDER_FILE, 'utf8'), ph = PLACEHOLDER, phInfo = JSON.parse(ph.files['devnet.json']);
    // The marked placeholder build, as set_network_identity.py --placeholder writes the committed placeholder.
    const built = readFileSync(join(rel, 'lib', 'studio-network.js'), 'utf8');
    const fixture = join(tmp, 'studio-network.js'); writeFileSync(fixture, built);
    const made = spawnSync('python3', ['-B', FIXTURE, 'placeholder', root, fixture], { encoding: 'utf8' });
    const marked = readFileSync(fixture, 'utf8');
    // The identity switch (D1) leaves no NETWORK_PLACEHOLDER mark and a real spec-2 identity in place of the
    // placeholder's: here the corpus's v3 devnet genesis under its own chain ID.
    const MARK = /\/\/ PLACEHOLDER \(decision D1\)[^]*?export const NETWORK_PLACEHOLDER = true;\n/;
    const hexOf = (b) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
    const real = { chainId: String(spec2.chainId), genesisHash: '0x' + hexOf(C.keccak(v3Base)), genesisPre: '0x' + hexOf(v3Base) };
    const chainLine = (id) => `"chainId": ${id},`;
    ok('the marked placeholder build pins the committed placeholder identity with its mark (and is the build itself while the build carries the placeholder)',
      made.status === 0 && MARK.test(marked) && marked.includes(chainLine(phInfo.chainId)) && marked.includes(`"genesisHash": "${phInfo.genesisHash}"`)
      && marked.includes(`"genesisPre": "${ph.files['genesis.spec.hex']}"`) && (!MARK.test(built) || marked === built), made.stderr);
    const swap = (t) => t.replace(chainLine(phInfo.chainId), chainLine(real.chainId)).replace(phInfo.genesisHash, real.genesisHash)
      .replace(ph.files['genesis.spec.hex'], real.genesisPre);
    const swapped = swap(marked.replace(MARK, ''));
    ok('the switched test build pins a real spec-2 identity, not the placeholder', real.chainId !== String(phInfo.chainId)
      && real.genesisHash !== phInfo.genesisHash && swapped.includes(chainLine(real.chainId)) && swapped.includes(real.genesisHash)
      && swapped.includes(real.genesisPre) && !swapped.includes(String(phInfo.chainId)) && !swapped.includes(phInfo.genesisHash.slice(2))
      && !swapped.includes('NETWORK_PLACEHOLDER') && spec2.specVersion === 2);
    // The build's lib/studio-network.js is the marked placeholder build; switched: the mark removed and the identity
    // replaced; identity: false keeps the placeholder identity.
    const pack = (edit = null, { switched = true, identity = true, network = null, placeholder = phText } = {}) => {
      rmSync(join(ext, 'dist'), { recursive: true, force: true }); rmSync(join(tmp, 'out.zip'), { force: true });
      cpSync(rel, join(ext, 'dist'), { recursive: true });
      writeFileSync(phFile, placeholder);
      if (edit) { const f = join(ext, 'dist', 'lib', 'refdec.js'); writeFileSync(f, edit(readFileSync(f, 'utf8'))); }
      let t = marked;
      if (switched) { t = t.replace(MARK, ''); if (identity) t = swap(t); }
      if (network) t = network(t);
      writeFileSync(join(ext, 'dist', 'lib', 'studio-network.js'), t);
      return spawnSync('python3', ['-B', join(ext, 'scripts', 'package-release.py'), join(tmp, 'out.zip')], { encoding: 'utf8' });
    };
    const refused = (r, rx) => r.status !== 0 && rx.test(r.stderr) && !existsSync(join(tmp, 'out.zip'));
    const asBuilt = pack(null, { switched: false });
    ok('the release packager refuses a build whose built-in profile is the placeholder (D1)', refused(asBuilt, /network placeholder/), asBuilt.stderr);
    for (const [name, switched, network] of [['the mark set to false', false, (t) => t.replace('NETWORK_PLACEHOLDER = true', 'NETWORK_PLACEHOLDER = false')],
                                             ['only the mark line kept', true, (t) => t + 'export const NETWORK_PLACEHOLDER = true;\n']]) {
      const r = pack(null, { switched, network });
      ok('the release packager refuses a placeholder mark: ' + name, refused(r, /network placeholder/), r.stderr);
    }
    // The mark dropped by hand or by a merge while the placeholder identity stays: refused by the identity itself.
    for (const [name, opts] of [['the mark removed, the placeholder identity kept', { identity: false }],
                                ['the placeholder chain ID kept', { network: (t) => t.replace(chainLine(real.chainId), chainLine(phInfo.chainId)) }],
                                ['the placeholder chain ID kept in hex', { network: (t) => t.replace(chainLine(real.chainId), `"chainId": 0x${phInfo.chainId.toString(16).toUpperCase()},`) }],
                                ['the placeholder genesis hash kept', { network: (t) => t.replace(real.genesisHash, phInfo.genesisHash.toUpperCase().replace('0X', '0x')) }],
                                ['a re-placed placeholder that deploy/mainnet/placeholder-network.pocol.json names',
                                 { placeholder: JSON.stringify({ ...ph, files: { ...ph.files, 'devnet.json': JSON.stringify({ chainId: Number(real.chainId), genesisHash: real.genesisHash }) } }) }]]) {
      const r = pack(null, opts);
      ok('the release packager refuses the placeholder identity: ' + name, refused(r, /network placeholder identity/), r.stderr);
    }
    const unreadable = pack(null, { placeholder: '{"files": {}}' });
    ok('the release packager refuses to package when the placeholder file names no identity', refused(unreadable, /names no placeholder identity/), unreadable.stderr);
    const packed = pack();
    ok('the release packager packages the release build once the placeholder is replaced', packed.status === 0 && existsSync(join(tmp, 'out.zip')), packed.stderr);
    const stale = pack((t) => t.replace('export const SPEC_VERSION = 2,', 'export const SPEC_VERSION = 1,'));
    ok('the release packager refuses a stale dist whose decoder does not decode spec 2', refused(stale, /does not decode spec 2/), stale.stderr);
  } finally { rmSync(tmp, { recursive: true, force: true }); }
  return checks;
}
