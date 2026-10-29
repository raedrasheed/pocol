// LP9 unit gates for the extension libraries (Node, built dist/ and fault builds).
// BR19a (bucket table, exact), BR13, BR1-BR12 schema/allowlist cases, BR17d/e/g,
// BR18d/g, StoreQueue admission (D101), manifest T16 negatives, wallet, netKey,
// the BR-neg fault controls that must FAIL their gates, the A07/A08 decoder differential and the
// A06/A09/A12/A14 worker and transport regressions.
import { build } from '../build.mjs';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { recvGuardChecks } from './recv_guard.mjs';
import { recordChecks } from './records_test.mjs';
import { workerChecks } from './worker_test.mjs';
import { decoderDiffChecks } from './decoder_diff.mjs';

const load = async (dist, m) => import(pathToFileURL(join(dist, 'lib', m)).href + '?' + dist);
const rel = build().dist;
const B = await load(rel, 'bridge.js'), S = await load(rel, 'sitestore.js'), SITE = await load(rel, 'site.js');
const RLP = await load(rel, 'rlp.js'), HEX = await load(rel, 'hex.js'), C = await load(rel, 'crypto.js'), WAL = await load(rel, 'wallet.js');
let passed = 0; const failed = [];
const ok = (name, cond, detail = '') => { if (cond) passed++; else failed.push(name + (detail ? ' — ' + detail : '')); };
const msg = (id, kind, method, params) => JSON.stringify({ id, kind, payload: { method, params } });
const code = (r) => (r.ok ? 'ok' : r.reply.error.code);

// ---------------- BR19a: bucket alone (Bpre), FakeClock, session created at 1000 ms ----------------
function br19a(Bm) {
  const s = Bm.newSession(1000); const res = []; const m = (i) => msg(i, 'rpc_read', 'eth_blockNumber', []);
  const run = (t, n, mk) => { const out = []; for (let i = 0; i < n; i++) { const r = Bm.check(s, mk(i), t); out.push(r.ok ? 'ok' : r.reply.error.code + ':' + (r.reply.error.data ? r.reply.error.data.reason : '') + ':' + r.reply.id); } return out; };
  const a1 = run(1000, 60, m); res.push(['a1 1..50 ok', a1.slice(0, 50).every((x) => x === 'ok') && s.tokens === 0 || a1.slice(0, 50).every((x) => x === 'ok')]);
  res.push(['a1 51..60 rate id=null', a1.slice(50).every((x) => x === '-32005:rate:null') && s.tokens === 0]);
  const a2 = run(1019, 1, m); res.push(['a2 1019 rate, tokens 950', a2[0] === '-32005:rate:null' && s.tokens === 950]);
  const a3 = run(1020, 1, m); res.push(['a3 1020 ok, tokens 0', a3[0] === 'ok' && s.tokens === 0]);
  const a4 = run(3000, 51, m); res.push(['a4 3000: 50 ok then rate', a4.slice(0, 50).every((x) => x === 'ok') && a4[50] === '-32005:rate:null']);
  const a5 = run(5000, 50, () => '{bad json'); const a5b = run(5000, 1, m);
  res.push(['a5 invalid JSON consumes tokens', a5.every((x) => x === '-32600::null') && a5b[0] === '-32005:rate:null']);
  const big = 'x'.repeat(65537); const a6 = run(7000, 50, () => big); const a6b = run(7000, 51, m);
  res.push(['a6 size rejects do not consume', a6.every((x) => x === '-32600:size:null') && a6b.slice(0, 50).every((x) => x === 'ok') && a6b[50] === '-32005:rate:null']);
  return res;
}
for (const [n, c] of br19a(B)) ok('BR19a ' + n, c);

// ---------------- B0-B3 cases (fresh session each, < 50 messages) ----------------
const fresh = () => B.newSession(0);
const chk = (text) => B.check(fresh(), text, 0);
const A = '0x' + '11'.repeat(20), H = '0x' + '22'.repeat(32);
ok('BR1 rpc_read eth_sendRawTransaction -> 4200', code(chk(msg(1, 'rpc_read', 'eth_sendRawTransaction', ['0x02']))) === 4200);
ok('BR2 rpc_read eth_sendTransaction -> 4200', code(chk(msg(1, 'rpc_read', 'eth_sendTransaction', [{ to: A }]))) === 4200);
ok('BR3 pocol_submitShare / pocol_getTemplate -> 4200', code(chk(msg(1, 'rpc_read', 'pocol_submitShare', []))) === 4200 && code(chk(msg(2, 'rpc_read', 'pocol_getTemplate', []))) === 4200);
for (const bad of ['ETH_SENDRAWTRANSACTION', 'eth_call ', 'eth_call\u0000', 'eth_getBаlance', 'e'.repeat(65), 'Site_storageSet'])
  ok('BR4 mangled name ' + JSON.stringify(bad).slice(0, 24) + ' -> 4200', code(chk(msg(1, 'rpc_read', bad, []))) === 4200);
ok('BR5 array payload -> -32600', code(chk(JSON.stringify({ id: 1, kind: 'rpc_read', payload: [{ method: 'eth_blockNumber', params: [] }, { method: 'eth_sendRawTransaction', params: [] }] }))) === -32600);
ok('BR6 extra payload fields -> -32600', code(chk(JSON.stringify({ id: 1, kind: 'rpc_read', payload: { method: 'eth_call', params: [{ to: A }, 'latest'], jsonrpc: '2.0', id: 7 } }))) === -32600);
ok('BR6 extra envelope field -> -32600', code(chk(JSON.stringify({ id: 1, kind: 'rpc_read', payload: { method: 'eth_blockNumber', params: [] }, x: 1 }))) === -32600);
ok('BR6 duplicate method key -> -32600', code(chk('{"id":1,"kind":"rpc_read","payload":{"method":"eth_blockNumber","method":"eth_sendRawTransaction","params":[]}}')) === -32600);
ok('BR7a wallet_req eth_getBalance -> 4200', code(chk(msg(1, 'wallet_req', 'eth_getBalance', [A, 'latest']))) === 4200);
ok('BR7b wallet_req eth_sendRawTransaction -> 4200', code(chk(msg(1, 'wallet_req', 'eth_sendRawTransaction', ['0x02']))) === 4200);
ok('BR7c storage_set eth_call without params -> -32600', code(chk(JSON.stringify({ id: 1, kind: 'storage_set', payload: { method: 'eth_call' } }))) === -32600);
{ const r = chk(msg(1, 'storage_set', 'eth_call', [])); ok('BR7d storage_set eth_call [] -> 4200 {method}', code(r) === 4200 && r.reply.error.data.method === 'eth_call'); }
{ const r = chk(msg(1, 'storage_set', 'site_storageSet', ['k', 5])); ok('BR7e params[1] -> -32602', code(r) === -32602 && r.reply.error.data.path === 'params[1]'); }
{ const r = chk(msg(1, 'rpc', 'eth_blockNumber', [])); ok('BR7f kind rpc -> -32600 {kind}', code(r) === -32600 && r.reply.error.data.reason === 'kind'); }
ok('BR7g rpc_read site_* -> 4200', code(chk(msg(1, 'rpc_read', 'site_storageSet', ['k', 'v']))) === 4200 && code(chk(msg(1, 'rpc_read', 'site_navigate', ['/']))) === 4200);
ok('BR7h nav eth_call -> 4200', code(chk(msg(1, 'nav', 'eth_call', [{ to: A }, 'latest']))) === 4200);
{ const r = chk(msg(1, 'rpc_read', 'eth_call', [{ to: A }, 'latest', {}])); ok('BR8 state override -> -32602 params', code(r) === -32602 && r.reply.error.data.path === 'params'); }
ok('BR9 gas 30000001 -> -32602', code(chk(msg(1, 'rpc_read', 'eth_call', [{ to: A, gas: '0x1c9c381' }, 'latest']))) === -32602);
ok('BR9 no to -> -32602', code(chk(msg(1, 'rpc_read', 'eth_call', [{ data: '0x' }, 'latest']))) === -32602);
ok('BR9 data and input -> -32602', code(chk(msg(1, 'rpc_read', 'eth_call', [{ to: A, data: '0x', input: '0x' }, 'latest']))) === -32602);
ok('BR10 filter with 17 addresses -> -32602', code(chk(msg(1, 'rpc_read', 'eth_getLogs', [{ fromBlock: '0x1', toBlock: '0x64', address: Array(17).fill(A) }]))) === -32602);
ok('BR10 filter 1..100 passes check', chk(msg(1, 'rpc_read', 'eth_getLogs', [{ fromBlock: '0x1', toBlock: '0x64' }])).ok);
for (const kind of ['rpc_read', 'wallet_req', 'storage_set', 'nav']) for (const m of ['__proto__', 'constructor', 'hasOwnProperty', 'toString'])
  ok(`BR11 ${kind} ${m} -> 4200`, code(chk(msg(1, kind, m, []))) === 4200);
ok('BR12 getBlockByNumber [latest,true] -> -32602', code(chk(msg(1, 'rpc_read', 'eth_getBlockByNumber', ['latest', true]))) === -32602);
for (const t of ['pending', 'earliest', { blockHash: H }]) ok('BR12 tag ' + JSON.stringify(t) + ' -> -32602', code(chk(msg(1, 'rpc_read', 'eth_getBlockByNumber', [t, false]))) === -32602);
{ let p = []; for (let i = 0; i < 8; i++) p = [p]; ok('BR13 depth 9 -> -32600', code(chk(JSON.stringify({ id: 1, kind: 'rpc_read', payload: { method: 'eth_blockNumber', params: p } }))) === -32600); }
{ const s = fresh(); const before = s.tokens; const r = B.check(s, 'x'.repeat(65537), 0); ok('BR13 64KiB+1 -> -32600 {size} id null, no token', code(r) === -32600 && r.reply.error.data.reason === 'size' && r.reply.id === null && s.tokens === before); }
{ const r = chk('{"id":1,'); ok('BR13 invalid JSON id null', code(r) === -32600 && r.reply.id === null); }
for (const id of [-1, 2 ** 32, '"7"']) { const r = chk(`{"id":${typeof id === 'string' ? id : id},"kind":"rpc_read","payload":{"method":"eth_blockNumber","params":[]}}`); ok('BR13 id ' + id + ' -> -32600 id null', code(r) === -32600 && r.reply.id === null); }

// ---------------- BR17: storage schema, quota table, legacy shape ----------------
const ss = (k, v) => chk(msg(1, 'storage_set', 'site_storageSet', v === undefined ? [k] : [k, v]));
const path = (r) => r.reply && r.reply.error.data && r.reply.error.data.path;
ok("BR17d [''] -> params", path(ss('')) === 'params');
ok("BR17d ['', 'v'] -> params[0]", path(ss('', 'v')) === 'params[0]');
ok('BR17d 257-byte key -> params[0]', path(ss('é'.repeat(128) + 'a', 'v')) === 'params[0]');
ok('BR17d 61441-byte value -> params[1]', path(ss('k', 'a'.repeat(61441))) === 'params[1]');
ok("BR17d ['k','v','x'] -> params", path(chk(msg(1, 'storage_set', 'site_storageSet', ['k', 'v', 'x']))) === 'params');
ok("BR17d [1,'v'] -> params[0]", path(ss(1, 'v')) === 'params[0]');
ok('BR17d lone surrogate -> params[0]', path(chk('{"id":1,"kind":"storage_set","payload":{"method":"site_storageSet","params":["\\ud800","v"]}}')) === 'params[0]');
ok('BR17d storageClear [1] -> params', path(chk(msg(1, 'storage_set', 'site_storageClear', [1]))) === 'params');
ok('BR17g legacy {key,value} -> -32600', code(chk(JSON.stringify({ id: 1, kind: 'storage_set', payload: { key: 'k', value: 'v' } }))) === -32600);
function br17e(Sm) {
  let d = {}; const out = []; const step = (k, v) => { const before = Sm.total(d); const r = Sm.apply(d, 'site_storageSet', [k, v]); if (!r.error) d = r.next; return { code: r.error ? r.error.code : null, before, after: Sm.total(d) }; };
  const A61 = 'a'.repeat(61440);
  for (let i = 0; i < 17; i++) { const r = step('k' + String(i).padStart(2, '0'), A61); out.push(['e1.' + (i + 1), r.code === null && r.after === 61443 * (i + 1)]); }
  let r = step('k17', A61); out.push(['e2', r.code === 4300 && r.after === 1044531 && !('k17' in d)]);
  r = step('k17', 'a'.repeat(4043)); out.push(['e3', r.code === 4300 && r.after === 1044531]);
  r = step('k17', 'a'.repeat(4042)); out.push(['e4 (equality accepted)', r.code === null && r.after === 1048576]);
  r = step('k18', 'x'); out.push(['e5', r.code === 4300 && r.after === 1048576 && !('k18' in d)]);
  r = step('k17', 'a'.repeat(4043)); out.push(['e6', r.code === 4300 && d.k17.length === 4042]);
  r = step('k00', 'b'); out.push(['e7 (replace subtracts old entry)', r.code === null && r.after === 987137]);
  r = step('k01', null); out.push(['e8 (delete never quota-rejected)', r.code === null && r.after === 925694]);
  out.push(['e9 final dictionary', d.k00 === 'b' && !('k01' in d) && !('k18' in d) && d.k17.length === 4042 && Sm.total(d) === 925694]);
  return out;
}
for (const [n, c] of br17e(S)) ok('BR17e ' + n, c);
{ const a = new S.StoreAdmission(); const s = a.session(); let n = 0; for (let i = 0; i < 9; i++) n += a.admit(s, 64) ? 1 : 0; ok('D101 session msgs <= 8', n === 8); }
{ const a = new S.StoreAdmission(); const s = a.session(); ok('D101 session bytes: 262144 accepted, +1 rejected', a.admit(s, 262144) && !a.admit(s, 1)); }
{ const a = new S.StoreAdmission(); let n = 0; for (let i = 0; i < 70; i++) n += a.admit(a.session(), 64) ? 1 : 0; ok('D101 global msgs <= 64', n === 64); }
ok('D101 qbytes', S.qbytes('site_storageSet', ['k', null]) === 65 && S.qbytes('site_storageSet', ['ab', 'é']) === 68 && S.qbytes('site_storageClear', []) === 64);

// ---------------- BR18: navigation schema; nav bucket ----------------
const nav = (m, p) => chk(msg(1, 'nav', m, [p]));
ok("BR18d 'about.html' -> params[0]", path(nav('site_navigate', 'about.html')) === 'params[0]');
ok("BR18d '/a b' -> params[0]", path(nav('site_navigate', '/a b')) === 'params[0]');
ok('BR18d 2049 chars -> params[0]', path(nav('site_navigate', '/' + 'a'.repeat(2048))) === 'params[0]');
ok("BR18c '/a/../b.html' '/a%2fb' '//x' pass the schema", ['/a/../b.html', '/a%2fb', '//x'].every((p) => nav('site_navigate', p).ok));
ok('BR18c normalization -> 404 for those paths', ['/a/../b.html', '/a%2fb', '//x'].every((p) => SITE.normalize(p) === null));
ok("BR18b '/x/' -> /x/index.html, '/' -> entry", SITE.normalize('/x/') === '/x/index.html' && SITE.normalize('/') === '');
for (const u of ['http://example.org/', 'javascript:alert(1)', 'chrome-extension://abc/x', 'https://user:pw@example.org/', 'https://'])
  ok('BR18g ' + u + ' -> params[0]', path(nav('site_openExternal', u)) === 'params[0]');
ok('BR18e https://example.org/ passes', nav('site_openExternal', 'https://example.org/').ok);
ok('BR18-L legacy nav {path} -> -32600', code(chk(JSON.stringify({ id: 1, kind: 'nav', payload: { path: '/a' } }))) === -32600);
{ const s = B.newSession(0); const r = [0, 0, 0, 0, 1999, 2000].map((t) => B.navTake(s, t)); ok('D99 nav bucket: 3 then one per 2 s', String(r) === 'true,true,true,false,false,true'); }

// ---------------- BR16 (invariant fuzz; no independent BridgeRef) ----------------
{
  let seed = 0x5eed1234; const rnd = () => { seed ^= seed << 13; seed >>>= 0; seed ^= seed >>> 17; seed ^= seed << 5; seed >>>= 0; return seed / 4294967296; };
  const pick = (a) => a[Math.floor(rnd() * a.length)];
  const names = [...[...B.ALLOW.values()].flatMap((m) => [...m.keys()]), 'eth_sendRawTransaction', 'eth_sign', 'personal_sign', 'eth_signTypedData_v4', 'wallet_addEthereumChain', 'eth_subscribe', 'eth_newFilter',
    'debug_traceCall', 'txpool_content', 'admin_peers', 'anvil_mine', 'evm_mine', 'eth_getProof', 'pocol_submitShare', 'pocol_getTemplate', 'pocol_status', '__proto__', 'constructor', 'toString'];
  const mutate = (m) => { const r = rnd(); if (r < 0.15) return m.toUpperCase(); if (r < 0.3) return m + ' '; if (r < 0.4) return m.slice(1); if (r < 0.5) return m + '\u0000'; return m; };
  const vals = [() => A, () => H, () => '0x1', () => '0x01', () => 'latest', () => 'pending', () => true, () => false, () => null, () => 5, () => 'k', () => 'é'.repeat(Math.floor(rnd() * 200)),
    () => ({ to: A }), () => ({ to: A, data: '0x', input: '0x' }), () => ({ fromBlock: '0x1', toBlock: '0x2' }), () => [], () => 'a'.repeat(61430 + Math.floor(rnd() * 20)), () => '/x.html', () => 'https://example.org/'];
  const kinds = ['rpc_read', 'wallet_req', 'storage_set', 'nav', 'rpc', 'RPC_READ', ''];
  let bad = 0; const codes = new Set([-32600, 4200, -32602, -32005]); const sess = B.newSession(0); let t = 0;
  for (let i = 0; i < 10000; i++) {
    const params = Array.from({ length: Math.floor(rnd() * 4) }, () => pick(vals)());
    let env = { id: rnd() < 0.05 ? -1 : i, kind: pick(kinds), payload: { method: mutate(pick(names)), params } };
    const r0 = rnd(); if (r0 < 0.05) env.extra = 1; else if (r0 < 0.1) env.payload.jsonrpc = '2.0'; else if (r0 < 0.13) env = [env];
    let text = JSON.stringify(env); if (rnd() < 0.03) text = text.slice(0, -1);
    t += 25; let r; try { r = B.check(sess, text, t); } catch (e) { bad++; continue; }
    if (r.ok) {
      const table = B.ALLOW.get(r.kind); const schema = table && table.get(r.method);
      if (!schema || schema(r.params) !== null || !Number.isInteger(r.id)) bad++;
    } else {
      const e = r.reply.error; if (!codes.has(e.code) || !(r.reply.id === null || (Number.isInteger(r.reply.id) && r.reply.id >= 0))) bad++;
    }
  }
  ok('BR16 invariant fuzz: 10^4 messages, no exception, every accepted message is allowlisted and schema-valid, fixed error codes', bad === 0, 'violations=' + bad);
}

// ---------------- manifest (storage.md T16) negatives ----------------
const enc = (files, entry = 0, ver = 1) => RLP.encode([RLP.encUint(BigInt(ver)), RLP.encUint(BigInt(entry)), files]);
const file = (p, mime, data) => { const a = SITE.chunkAddress(data); return [new TextEncoder().encode(p), RLP.encUint(BigInt(mime)), RLP.encUint(BigInt(data.length)), C.keccak(data), [[a, RLP.encUint(BigInt(data.length))]]]; };
const d1 = new TextEncoder().encode('<p>x'), d2 = new TextEncoder().encode('body{}');
const good = enc([file('/a.css', 2, d2), file('/index.html', 1, d1)], 1);
ok('T16 valid manifest decodes', SITE.decodeManifest(good).files.length === 2);
const throws = (f) => { try { f(); return false; } catch { return true; } };
ok('T16 unsorted paths rejected', throws(() => SITE.decodeManifest(enc([file('/index.html', 1, d1), file('/a.css', 2, d2)]))));
ok('T16 duplicate path rejected', throws(() => SITE.decodeManifest(enc([file('/index.html', 1, d1), file('/index.html', 1, d1)]))));
ok('T16 entry not html rejected', throws(() => SITE.decodeManifest(enc([file('/a.css', 2, d2), file('/index.html', 1, d1)], 0))));
ok('T16 bad path rejected', throws(() => SITE.decodeManifest(enc([file('/../x.html', 1, d1)]))) && throws(() => SITE.decodeManifest(enc([file('/a b.html', 1, d1)]))));
ok('T16 unknown mime rejected', throws(() => SITE.decodeManifest(enc([file('/index.html', 99, d1)]))));
ok('T16 version != 1 rejected', throws(() => SITE.decodeManifest(enc([file('/index.html', 1, d1)], 0, 2))));
ok('T16 size mismatch rejected', throws(() => { const f = file('/index.html', 1, d1); f[2] = RLP.encUint(5n); SITE.decodeManifest(enc([f])); }));
{ const code = HEX.concat(Uint8Array.of(0), d1); const a = SITE.chunkAddress(d1);
  ok('D27 chunk payload verified against CREATE2 address', HEX.eq(SITE.chunkPayload(a, code, d1.length), d1));
  ok('D27 chunk with code[0]!=0 rejected', throws(() => SITE.chunkPayload(a, HEX.concat(Uint8Array.of(1), d1), d1.length)));
  ok('D27 chunk at a foreign address rejected', throws(() => SITE.chunkPayload(SITE.chunkAddress(d2), code, d1.length))); }

// ---------------- M1 manifest vectors (draft 0.2): positives accepted, decode/structure/semantic negatives rejected ----------------
{
  const { readFileSync } = await import('node:fs'); const V = (n) => JSON.parse(readFileSync(new URL('../../../development/m1/m1-draft-0.2/vectors/' + n, import.meta.url), 'utf8')).fixtures;
  const pos = V('manifest-positive.json'); const bad = pos.filter((f) => throws(() => SITE.decodeManifest(HEX.fromHex(f.manifest)))).map((f) => f.id);
  ok('manifest vectors: all 11 positives accepted (incl. pos-empty-file)', pos.length === 11 && bad.length === 0, bad.join(','));
  const neg = V('manifest-negative.json').filter((f) => ['decode', 'structure', 'semantic'].includes(f.stage)); const miss = neg.filter((f) => !throws(() => SITE.decodeManifest(HEX.fromHex(f.manifest)))).map((f) => f.id);
  ok('manifest vectors: all 56 decode/structure/semantic negatives rejected', neg.length === 56 && miss.length === 0, miss.join(','));
}

// ---------------- wallet ----------------
{
  const seed = new Uint8Array(32).fill(7); const box = await WAL.encryptSeed(seed, 'pw');
  ok('wallet PBKDF2 600000 + AES-GCM roundtrip', box.iter === 600000 && HEX.eq(await WAL.decryptSeed(box, 'pw'), seed));
  ok('wallet wrong password rejected', await WAL.decryptSeed(box, 'nope').then(() => false, () => true));
  const k1 = await WAL.networkKey(seed, '0x' + '01'.repeat(32)), k2 = await WAL.networkKey(seed, '0x' + '02'.repeat(32));
  ok('wallet key differs per netKey (HKDF)', !HEX.eq(k1, k2));
  const tx = { chainId: 777900, nonce: 3n, maxPriority: 1n, maxFee: 2n, gas: 21000n, to: A, value: 1n, data: '0x' };
  const s = WAL.signTx(k1, tx); const raw = HEX.fromHex(s.raw);
  ok('signTx: txHash = keccak(raw)', HEX.toHex(C.keccak(raw)) === s.hash);
  const f = RLP.decode(raw.subarray(1)); const unsigned = HEX.concat(Uint8Array.of(2), RLP.encode(f.slice(0, 9)));
  const pad = (b) => HEX.concat(new Uint8Array(32 - b.length), b);
  const sig = HEX.concat(pad(f[10]), pad(f[11]), Uint8Array.of(f[9].length ? f[9][0] : 0));
  ok('signTx: signature recovers the wallet address', HEX.toHex(C.recover(C.keccak(unsigned), sig)) === WAL.addressOf(k1));
}

// ---------------- BR-neg: fault builds must fail their gates ----------------
{
  const F = await load(build({ fault: 'noRefill' }).dist, 'bridge.js');
  const a3 = br19a(F).find(([n]) => n.startsWith('a3'))[1]; ok('BR-neg noRefill fails BR19a(a3)', a3 === false);
  const FS = await load(build({ fault: 'quotaNoSubtract' }).dist, 'sitestore.js');
  const e7 = br17e(FS).find(([n]) => n.startsWith('e7'))[1]; ok('BR-neg quotaNoSubtract fails BR17e(e7)', e7 === false);
  const FA = await load(build({ fault: 'allowAll' }).dist, 'bridge.js');
  ok('BR-neg allowAll fails BR1 (raw tx passes BridgeAuth)', FA.check(FA.newSession(0), msg(1, 'rpc_read', 'eth_sendRawTransaction', ['0x02']), 0).ok === true);
}

for (const c of await recvGuardChecks(rel)) ok('D98 ' + c.name, c.pass, c.detail);
for (const c of await recordChecks(rel)) ok('D103 ' + c.name, c.pass, c.detail);
{ const r = await decoderDiffChecks(rel);
  ok(`A07/A08 GenesisSpec and header decoders give the Rust reference verdict on all ${r.items} corpus items`, r.items > 1000 && r.mismatches.length === 0, r.mismatches.slice(0, 5).join('; '));
  ok('A07/A08 the shipped RP entry points use the reference decoders', r.entryPoints); }
for (const c of await workerChecks(rel)) ok(c.name, c.pass, c.detail);

console.log(JSON.stringify({ result: failed.length ? 'FAIL' : 'PASS', passed, failed }, null, 1));
process.exit(failed.length ? 1 : 0);
