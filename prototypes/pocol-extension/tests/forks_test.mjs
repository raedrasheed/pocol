// Real RP verification over signed synthetic headers crossing the maximum-range
// fork. Synthetic headers test verification rules, not execution or membership.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export async function forkChecks(dist) {
  const L = (m) => import(pathToFileURL(join(dist, 'lib', m)).href);
  const F = await L('forks.js'), RP = await L('rp.js'), R = await L('rlp.js'), H = await L('hex.js'), C = await L('crypto.js');
  const checks = [], ok = (name, pass, detail = '') => checks.push({ name, pass: !!pass, detail });
  const normalized = F.normalizeForkSchedule([[1, 0], [2, '3']]);
  ok('local schedule normalizes heights and retains v1 before activation', JSON.stringify(normalized) === '[[1,"0"],[2,"3"]]' && F.activeVersion(normalized, 2n) === 1n && F.activeVersion(normalized, 3n) === 2n && F.activeVersion(normalized, 4n) === 2n);
  ok('omitted schedule never activates v2, including at u64 max', F.activeVersion(F.normalizeForkSchedule(), (1n << 64n) - 1n) === 1n);
  ok('u64 max activation retained exactly', F.normalizeForkSchedule([[1, '0'], [2, '18446744073709551615']])[1][1] === '18446744073709551615');
  for (const bad of [null, [], [[2, 0]], [[1, 1]], [[1, 0], [2, 0]], [[1, 0], [3, 3]], [[1, 0], [1, 3]], [[1, 0], [2, '03']], [[1, 0], [2, '-1']], [[1, 0], [2, '18446744073709551616']], [[1, 0], [2, 9007199254740992]], [[1, 0], [2, 3], [2, 4]]]) {
    let rejected = false; try { F.normalizeForkSchedule(bad); } catch { rejected = true; }
    ok('malformed or unknown schedule rejected: ' + JSON.stringify(bad), rejected);
  }
  ok('full nonce range holds from v2 onwards and under genesis nonceMode 1', RP.fullRange(3n, 0n) && RP.fullRange(2n, 0n) && RP.fullRange(1n, 1n) && !RP.fullRange(1n, 0n));
  const three = F.normalizeForkSchedule([[1, 0], [2, '10'], [3, '20']], 3);
  ok('a later version activates at its own height under a higher test bound', F.activeVersion(three, 9n) === 1n && F.activeVersion(three, 10n) === 2n && F.activeVersion(three, 19n) === 2n && F.activeVersion(three, 20n) === 3n);
  for (const bad of [[[1, 0], [2, '10'], [3, '20']], [[1, 0], [3, '20']]]) {
    let rejected = false; try { F.normalizeForkSchedule(bad); } catch { rejected = true; }
    ok('a version above MAX_FORK_VERSION is rejected by default: ' + JSON.stringify(bad), rejected && F.MAX_FORK_VERSION === 2);
  }
  for (const bad of [[[1, 0], [2, '10'], [3, '10']], [[1, 0], [2, '10'], [3, '9']], [[1, 0], [3, '20']]]) {
    let rejected = false; try { F.normalizeForkSchedule(bad, 3); } catch { rejected = true; }
    ok('heights increase and versions are not skipped under a higher bound: ' + JSON.stringify(bad), rejected);
  }
  const hex = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'data/devnet-spec.hex'), 'utf8').trim();
  const rawSpec = R.decode(H.fromHex(hex.startsWith('0x') ? hex : '0x' + hex));
  rawSpec[2][1] = R.encUint((1n << 256n) - 1n); // Every nonce satisfies PoW: boundaries are deterministic.
  rawSpec[2][2] = R.encUint(7n); rawSpec[2][4] = R.encUint(0n); rawSpec[2][5] = R.encUint(2n);
  rawSpec[2][6] = R.encUint(60n); rawSpec[2][7] = R.encUint(6n);
  const spec = RP.decodeSpec(R.encode(rawSpec)), spec1 = spec;
  const u = R.encUint, empty = new Uint8Array(), zero = () => new Uint8Array(32), maxNonce = (1n << 64n) - 1n;
  function chain({ head = 4, versions = {}, nonces = {}, shares = {}, upgraded = true, spec = spec1 } = {}) {
    const headers = []; let parent = spec.genesisHash;
    for (let n = 1; n <= head; n++) {
      const height = BigInt(n), version = BigInt(versions[n] ?? (upgraded && n >= 3 ? 2 : 1));
      const ut = [H.utf8('PoCol-tpl-v1'), u(spec.chainId), spec.genesisHash, parent, u(height), u(0n), u(version), u(RP.cpv(spec, 'g_ts') + 7n * height), u((1n << 256n) - 1n), zero(), zero(), zero(), new Uint8Array(256), u(8000000n), u(0n), u(1n), zero(), empty];
      const nonce = BigInt(nonces[n] ?? (upgraded && n >= 3 ? maxNonce : 1n));
      const ns = H.be(nonce, 8), ss = (shares[n] || []).map((x) => H.be(x, 8));
      const tid = C.sha256(R.encode(ut)), shareRoot = C.keccak(R.encode(ss));
      const winnerMessage = C.keccak(H.concat(Uint8Array.of(0x19), H.utf8('PoCol winner'), Uint8Array.of(0x0a), tid, ns, shareRoot));
      const bytes = R.encode([[ut, empty], ns, ss, C.sign(winnerMessage, H.be(1n, 32))]);
      headers.push(H.toHex(bytes)); parent = RP.decodeHeader(bytes).hash;
    }
    const calls = [];
    return { calls, spec, rpc: async (method, params) => {
      calls.push(method);
      if (method === 'eth_blockNumber') return '0x' + head.toString(16);
      if (method === 'pocol_getHeaders') { const start = Number(BigInt(params[0])) - 1, count = Number(BigInt(params[1])); return headers.slice(start, start + count); }
      // An endpoint advertising any activation must not be consulted for verifier rules.
      if (method === 'pocol_getParams') return { forkSchedule: [[1, 0], [2, 1]], activeVersion: 2, nonceMode: 1 };
      throw new Error('unexpected RPC ' + method);
    } };
  }
  const clock = Number(RP.cpv(spec, 'g_ts')) + 1000;
  const verdict = async (c, forks, extraSpec = {}) => { try { return (await RP.verifyWindow(c.rpc, { ...c.spec, ...extraSpec }, clock, forks)).ok ? 'accepted' : 'not-ok'; } catch (e) { return e.flag || e.message; } };
  for (const head of [2, 3, 4]) ok('H-1/H/H+1 header window accepted at head ' + head, await verdict(chain({ head }), normalized) === 'accepted');
  ok('high nonce rejected under v1 immediately before H', await verdict(chain({ head: 2, nonces: { 2: maxNonce } }), normalized) === '6');
  ok('high share accepted under v2 at H', await verdict(chain({ head: 3, shares: { 3: [maxNonce - 1n] } }), normalized) === 'accepted');
  ok('high share rejected under v1 at H-1', await verdict(chain({ head: 2, shares: { 2: [maxNonce - 1n] } }), normalized) === '9');
  ok('premature v2 header rejected', await verdict(chain({ head: 2, versions: { 2: 2 } }), normalized) === 'netVersion');
  ok('v1 header rejected at trusted v2 activation', await verdict(chain({ head: 3, versions: { 3: 1 } }), normalized) === 'netVersion');
  ok('unknown header version rejected', await verdict(chain({ versions: { 4: 3 } }), normalized) === 'netVersion');
  ok('old profile accepts legacy v1 windows', await verdict(chain({ upgraded: false })) === 'accepted');
  const untrusted = chain();
  ok('RPC cannot activate v2 for an old profile', await verdict(untrusted) === 'netVersion' && !untrusted.calls.includes('pocol_getParams'));
  ok('schedule attached to untrusted spec metadata cannot override local profile', await verdict(chain(), undefined, { forkSchedule: normalized, activeVersion: 2 }) === 'netVersion');
  // A spec-2 (v3) genesis: the same CP prefix plus the twelve CP v2 fields (refdec.js CP_V2_EXTRA). The release
  // entry point refuses it with the reference's gsVersion until activation (V3-PR-I); an explicit ceiling of 2 (the
  // v3-preview decoder) accepts it, and RP then applies exactly the same nMax rule, keyed on the header version and
  // CP nonceMode only.
  const D = await L('refdec.js');
  const extra = D.CP_V2_EXTRA.map(([, , lo]) => u(BigInt(lo + 3)));
  const raw2 = [u(2n), rawSpec[1], [...rawSpec[2], ...extra], rawSpec[3], rawSpec[4], rawSpec[5]], spec2Bytes = R.encode(raw2);
  let refused = ''; try { RP.decodeSpec(spec2Bytes); } catch (e) { refused = `${e.gs}:${e.detail}`; }
  ok('the release RP entry point refuses a spec-2 genesis with gsVersion until activation', refused === 'gsVersion:2' && D.SPEC_VERSION === 1);
  const spec2 = RP.decodeSpec(spec2Bytes, 2);
  ok('the explicit preview ceiling decodes a spec-2 genesis with its version and the 64-field CP v2 schema',
    spec2.specVersion === 2 && spec2.cp.length === 64 && D.CP_V2_LEN === 64 && RP.cpv(spec2, 'H_half') === 4n && RP.cpv(spec2, 'rho_bp') === 3n
    && RP.cpv(spec2, 'D_att') === RP.cpv(spec, 'D_att') && spec.specVersion === 1);
  let v2Name = false; try { RP.cpv(spec, 'H_half'); } catch { v2Name = true; }
  ok('a CP v2 field name is not readable from a spec-1 genesis', v2Name);
  ok('the preview size bound covers the longer CP v2 encoding only above the release ceiling', D.maxSpecBytes(2) > D.MAX_SPEC_BYTES && D.maxSpecBytes(1) === D.MAX_SPEC_BYTES);
  // The same numbers as genesis_spec.rs MAX_SPEC_BYTES of a release and a v3-preview build (pinned in pocold/tests/decoder_reference.rs).
  ok('the size bounds equal the Rust MAX_SPEC_BYTES of each build', D.MAX_SPEC_BYTES === 2818474 && D.maxSpecBytes(2) === 2818581);
  for (const [name, c, want] of [
    ['H-1/H/H+1 window accepted', chain({ spec: spec2 }), 'accepted'],
    ['high nonce rejected under v1 immediately before H', chain({ spec: spec2, head: 2, nonces: { 2: maxNonce } }), '6'],
    ['high share accepted under v2 at H', chain({ spec: spec2, head: 3, shares: { 3: [maxNonce - 1n] } }), 'accepted'],
    ['high share rejected under v1 at H-1', chain({ spec: spec2, head: 2, shares: { 2: [maxNonce - 1n] } }), '9'],
    ['premature v2 header rejected', chain({ spec: spec2, head: 2, versions: { 2: 2 } }), 'netVersion'],
  ]) ok('spec 2 keeps the nMax rule: ' + name, await verdict(c, normalized) === want);
  const fullSpec2 = { ...spec2, cp: spec2.cp.map((x, i) => (i === 4 ? 1n : x)) };
  ok('spec 2 with nonceMode 1 takes the full nonce range under v1, as spec 1 does', await verdict({ ...chain({ spec: spec2, head: 2, nonces: { 2: maxNonce } }), spec: fullSpec2 }, normalized) === 'accepted'
    && await verdict({ ...chain({ head: 2, nonces: { 2: maxNonce } }), spec: { ...spec, cp: spec.cp.map((x, i) => (i === 4 ? 1n : x)) } }, normalized) === 'accepted');
  return checks;
}
