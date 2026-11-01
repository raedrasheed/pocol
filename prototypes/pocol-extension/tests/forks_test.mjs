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
  const hex = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'data/devnet-spec.hex'), 'utf8').trim();
  const rawSpec = R.decode(H.fromHex(hex.startsWith('0x') ? hex : '0x' + hex));
  rawSpec[2][1] = R.encUint((1n << 256n) - 1n); // Every nonce satisfies PoW: boundaries are deterministic.
  rawSpec[2][2] = R.encUint(7n); rawSpec[2][4] = R.encUint(0n); rawSpec[2][5] = R.encUint(2n);
  rawSpec[2][6] = R.encUint(60n); rawSpec[2][7] = R.encUint(6n);
  const spec = RP.decodeSpec(R.encode(rawSpec));
  const u = R.encUint, empty = new Uint8Array(), zero = () => new Uint8Array(32), maxNonce = (1n << 64n) - 1n;
  function chain({ head = 4, versions = {}, nonces = {}, shares = {}, upgraded = true } = {}) {
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
    return { calls, rpc: async (method, params) => {
      calls.push(method);
      if (method === 'eth_blockNumber') return '0x' + head.toString(16);
      if (method === 'pocol_getHeaders') { const start = Number(BigInt(params[0])) - 1, count = Number(BigInt(params[1])); return headers.slice(start, start + count); }
      // An endpoint advertising any activation must not be consulted for verifier rules.
      if (method === 'pocol_getParams') return { forkSchedule: [[1, 0], [2, 1]], activeVersion: 2, nonceMode: 1 };
      throw new Error('unexpected RPC ' + method);
    } };
  }
  const clock = Number(RP.cpv(spec, 'g_ts')) + 1000;
  const verdict = async (c, forks, extraSpec = {}) => { try { return (await RP.verifyWindow(c.rpc, { ...spec, ...extraSpec }, clock, forks)).ok ? 'accepted' : 'not-ok'; } catch (e) { return e.flag || e.message; } };
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
  return checks;
}
