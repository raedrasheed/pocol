// Synthetic RP header windows: consecutive fallback headers (a=0) built from a GenesisSpec with
// exact timing (L(0)+dFbWait), ASERT targets, real PoW nonces below nMax and a recoverable winnerSig.
// `mutate(h, ut)` may alter UT fields of height h (as RLP items) before sealing. `maxVersion` is an explicit decoder
// ceiling for the spec (by default the release ceiling, SPEC_VERSION 2, which decodes a v3 genesis).
import { readFileSync } from 'node:fs';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { join, dirname } from 'node:path';

// The devnet GenesisSpec fixtures. data/devnet-spec-v3.hex is the one a profile uses: a spec-2 (v3) genesis with nonceMode
// 1, as pocold admits, written by the Rust tool (genesis.spec.hex of
//   pocold init-devnet --out DIR --miners a,b,c --chain-id 777960 --genesis-time 1700000000 --t-blk 4 --tau 40 \
//     --d-att 25 --fb-wait 3 --c 2 --work-bits 8
// ), never by hand. data/devnet-spec.hex is the retired spec-1 devnet genesis under the same chain ID and genesis time: it
// still decodes (the decoder corpus and the spec-1 decode checks), but no node admits it and no profile accepts it.
// tests/devnet_spec_fixture.mjs (npm run test:devnet-spec, run by CI after pocold is built) reruns the command above and
// compares its genesis.spec.hex with data/devnet-spec-v3.hex byte for byte; keep the command here in that one form.
const DATA = join(dirname(fileURLToPath(import.meta.url)), 'data');
const readHex = (name) => readFileSync(join(DATA, name), 'utf8').trim().replace(/^0x/, '');
/** The v3 devnet GenesisSpec as hex without 0x (the profile fixture). */
export const devnetSpecV3Hex = () => readHex('devnet-spec-v3.hex');
/** The retired spec-1 devnet GenesisSpec as hex without 0x (decode-compatibility fixture only). */
export const devnetSpecV1Hex = () => readHex('devnet-spec.hex');

export async function rpWindow(dist, specBytes, heights, mutate = () => {}, maxVersion = undefined) {
  const L = async (m) => import(pathToFileURL(join(dist, 'lib', m)).href);
  const RP = await L('rp.js'), R = await L('rlp.js'), H = await L('hex.js'), C = await L('crypto.js');
  const spec = maxVersion === undefined ? RP.decodeSpec(specBytes) : RP.decodeSpec(specBytes, maxVersion); const cp = (k) => RP.cpv(spec, k);
  const u = (v) => R.encUint(BigInt(v)); const enc = new TextEncoder();
  const winKey = new Uint8Array(32).fill(3);
  let parent = { hash: spec.genesisHash, ts: cp('g_ts'), h: 0n };
  const out = [];
  for (let h = 1n; h <= BigInt(heights); h++) {
    let ut, tid, n, ts;
    for (let a = 0n; ; a++) { // a bounded nonce space may hold no solution: next attempt (consensus §4)
      if (a > 32n) throw new Error('no nonce in 32 attempts');
      const Lw = parent.ts + a * cp('D_att') + 1n; ts = a === 0n ? Lw + cp('dFbWait') : Lw;
      const target = RP.asert(cp('target_g'), cp('g_ts'), cp('T_blk'), cp('tau'), parent.ts, parent.h);
      const z32 = new Uint8Array(32); const tb = H.be(target, 32);
      ut = [enc.encode('PoCol-tpl-v1'), u(spec.chainId), spec.genesisHash, parent.hash, u(h), u(a), u(1), u(ts), tb.subarray(tb.findIndex((x) => x !== 0)),
        z32, z32, z32, new Uint8Array(256), u(8000000), u(0), u(1000000000), z32, new Uint8Array(0)];
      mutate(Number(h), ut);
      tid = C.sha256(R.encode(ut));
      const cw = cp('c') * RP.work(target); const nMax = cw < (1n << 64n) ? cw : 1n << 64n;
      n = 0n; while (n < nMax && H.toBig(C.sha256(H.concat(tid, H.be(n, 8)))) > target) n++;
      if (n < nMax) break;
    }
    const nonceB = H.be(n, 8); const shares = []; const shareRoot = C.keccak(R.encode(shares));
    const winMsg = C.keccak(H.concat(Uint8Array.of(0x19), enc.encode('PoCol winner'), Uint8Array.of(0x0a), tid, nonceB, shareRoot));
    const header = R.encode([[ut, new Uint8Array(0)], nonceB, shares, C.sign(winMsg, winKey)]);
    out.push(H.toHex(header));
    parent = { hash: C.keccak(H.concat(tid, nonceB, shareRoot)), ts, h };
  }
  const rpc = async (method, params) => {
    if (method === 'eth_blockNumber') return '0x' + out.length.toString(16);
    if (method === 'pocol_getHeaders') { const from = Number(BigInt(params[0])), count = Number(BigInt(params[1])); return out.slice(from - 1, from - 1 + count); }
    throw new Error('unexpected ' + method);
  };
  return { headers: out, rpc, spec, lastTs: Number(parent.ts) };
}
