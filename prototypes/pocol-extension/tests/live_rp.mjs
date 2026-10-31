// Live RP window check (browser.md D77, D80-D84) against headers produced by the
// Rust pocold nodes: the JS verifier must accept the real window, and each single
// mutation must be rejected with the flag of the first violated item.
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

export async function runLiveRp(dist, endpoint, profile) {
  const L = async (m) => import(pathToFileURL(join(dist, 'lib', m)).href);
  const RP = await L('rp.js'), R = await L('rlp.js'), H = await L('hex.js'), C = await L('crypto.js');
  const spec = RP.decodeSpec(H.fromHex(profile.genesisPre));
  const verify = (f) => RP.verifyWindow(f, spec, undefined, profile.forkSchedule);
  let id = 0;
  const rpc = async (method, params) => {
    const r = await fetch(endpoint, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params }) });
    const v = await r.json(); if (v.error) throw new Error(v.error.message); return v.result;
  };
  const checks = [];
  const ok = (name, cond, detail = '') => checks.push({ name, pass: !!cond, detail });
  const good = await verify(rpc);
  ok('RP window over real headers accepted (items 1, 1b, viewFuture, 2-9, viewTargetCeil)', good.ok && good.n === 13 && !good.toGenesis, good.message);
  ok('RP trust message names the header count and the work bound', /RP: work and identity bound for 13 headers/.test(good.message) && /work\(ceilTarget\)/.test(good.message));
  // Freeze one window, then mutate a single field of one header. The timestamp and target mutations
  // (items 4, 5) need an unsigned (fallback) header: on a signed one they first break the proposer's
  // signature (item 3). With every proposer up most blocks are signed, so the window is frozen to end
  // at the latest fallback header of the chain at height 14 or more; without one, the tip (and the check below fails).
  // Fallback headers come when a proposer is late, so the chain may have none yet: the test waits for one, up to
  // 180 s (acceptance on 719da04, whole-state: the only fallback headers were at heights 3, 5 and 12 when the
  // check ran, and the next came at 55; evidence/r4/accept-719da04/).
  const isSignedRaw = (x) => R.decode(H.fromHex(x))[0][1].length === 65;
  let fallbackAt = null, tip = 0n;
  for (const until = Date.now() + 180000; fallbackAt === null && Date.now() < until; ) {
    tip = BigInt(await rpc('eth_blockNumber', []));
    for (let top = tip; top >= 14n && fallbackAt === null; top -= 64n) {
      const start = top - 63n > 14n ? top - 63n : 14n;
      const xs = await rpc('pocol_getHeaders', ['0x' + start.toString(16), '0x' + (top - start + 1n).toString(16)]);
      for (let i = xs.length - 1; i >= 0; i--) if (!isSignedRaw(xs[i])) { fallbackAt = start + BigInt(i); break; }
    }
    if (fallbackAt === null) await new Promise((r) => setTimeout(r, 2000));
  }
  ok('RP mutation window ends at a fallback header (items 4 and 5 are reachable)', fallbackAt !== null, fallbackAt === null ? 'no fallback header in the chain' : 'height ' + fallbackAt + ' of tip ' + tip);
  const h = fallbackAt ?? tip; const b = h - 12n; const from = b - 1n;
  const raw = await rpc('pocol_getHeaders', ['0x' + from.toString(16), '0x' + (h - from + 1n).toString(16)]);
  const frozen = async (method, params) => (method === 'eth_blockNumber' ? '0x' + h.toString(16) : method === 'pocol_getHeaders' ? raw.slice() : rpc(method, params));
  ok('frozen real window accepted', (await verify(frozen)).ok);
  const dec = raw.map((x) => R.decode(H.fromHex(x)));
  const isSigned = (t) => t[0][1].length === 65;
  // Prefer the last unsigned (fallback) header so item 3 does not fire first; else the last header.
  let idx = dec.length - 1; for (let i = dec.length - 1; i >= 1; i--) if (!isSigned(dec[i])) { idx = i; break; }
  const signedIdx = dec.findIndex((t, i) => i >= 1 && isSigned(t));
  const run = async (mutate, at = idx) => {
    const copy = raw.slice(); const t = R.decode(H.fromHex(raw[at])); mutate(t); copy[at] = H.toHex(R.encode(t));
    const f = async (method, params) => (method === 'pocol_getHeaders' ? copy : frozen(method, params));
    // Item 1 carries the Rust reference decoder's detail (A08), e.g. '1 header arity'.
    try { const r = await verify(f); return r.ok ? 'accepted' : r.flag; } catch (e) { return e.flag ? (e.detail ? e.flag + ' ' + e.detail : e.flag) : e.message; }
  };
  const u = (v) => R.encUint(BigInt(v)); const big = (x) => H.toBig(x);
  const cases = [
    ['chainId changed -> netChain', (t) => { t[0][0][1] = u(big(t[0][0][1]) + 1n); }, 'netChain'],
    ['genesisHash changed -> netGenesis', (t) => { t[0][0][2] = new Uint8Array(32).fill(1); }, 'netGenesis'],
    ['timestamp in the far future -> viewFuture', (t) => { t[0][0][7] = u(BigInt(Math.floor(Date.now() / 1000)) + 3600n); }, 'viewFuture'],
    ['parentHash changed -> 2', (t) => { t[0][0][3] = new Uint8Array(32).fill(2); }, '2'],
    ['timestamp below L(a) -> 4', (t) => { t[0][0][7] = u(big(t[0][0][7]) - 1n); }, '4'],
    ['target not equal to ASERT -> 5', (t) => { t[0][0][8] = u(big(t[0][0][8]) - 1n); }, '5'],
    // A smaller nonce stays inside [0, nMax); pick one whose PoW hash exceeds the target so item 7 fires.
    ['nonce below nMax but PoW fails -> 7', (t) => {
      const tid = C.sha256(R.encode(t[0][0])); const target = big(t[0][0][8]); const n0 = big(t[1]);
      for (let k = 1n; k < 4096n; k++) { const c = n0 >= k ? n0 - k : n0 + k; if (H.toBig(C.sha256(H.concat(tid, H.be(c, 8)))) > target) { t[1] = H.be(c, 8); return; } }
    }, '7'],
    ['winnerSig r=0 -> 8', (t) => { const s = t[3].slice(); s.fill(0, 0, 32); t[3] = s; }, '8'],
    ['share equal to the winning nonce -> 9', (t) => { t[2] = [t[1]]; }, '9'],
    ['bad header shape -> 1', (t) => { t.push(new Uint8Array(1)); }, '1 header arity'],
  ];
  // No 8-byte nonce is outside a full 64-bit domain. Keep the range-negative
  // check only where the selected historical header actually has a smaller cap.
  const selected = RP.decodeHeader(H.fromHex(raw[idx]));
  const bounded = selected.version === 1n && RP.cpv(spec, 'nonceMode') === 0n;
  const nMax = RP.cpv(spec, 'c') * RP.work(selected.target);
  if (bounded && nMax < (1n << 64n)) cases.push(['nonce >= nMax -> 6', (t) => { t[1] = H.be(nMax, 8); }, '6']);
  else cases.push(['nonce exceeds 8-byte encoding -> 1', (t) => { t[1] = H.be(1n << 64n, 9); }, '1 nonce']);
  for (const [name, m, want] of cases) { const got = await run(m); ok('RP mutation: ' + name, got === want, 'got ' + got); }
  if (signedIdx >= 0) { const got = await run((t) => { const s = t[0][1].slice(); s[5] ^= 1; t[0][1] = s; }, signedIdx); ok('RP mutation: proposer signature altered -> 3', got === '3', 'got ' + got); }
  { const f = async (method, params) => (method === 'pocol_getHeaders' ? raw.slice(0, -1) : frozen(method, params));
    let got; try { await verify(f); got = 'accepted'; } catch (e) { got = e.flag; } ok('RP window with a missing header -> viewIncomplete', got === 'viewIncomplete', 'got ' + got); }
  return checks;
}
