// A06, A09, A12, A14 regressions on the real built service worker and transport (Node, mocked
// Chrome and RPC; component evidence, not real-browser evidence). A06: signing eligibility across
// wallet and profile transitions; A09: one session receive budget; A12: storage admission follows
// the operation; A14: responses must answer exactly the request.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { installChromeMock, installFetchMock, loadWorker } from './chrome_mock.mjs';
import { siteFixture } from './site_fixture.mjs';
const HERE = dirname(fileURLToPath(import.meta.url));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function workerChecks(dist) {
  const L = (m) => import(pathToFileURL(join(dist, 'lib', m)).href);
  const R = await L('rlp.js'), H = await L('hex.js'), C = await L('crypto.js'), RP = await L('rp.js'), T = await L('transport.js'), W = await L('wallet.js');
  const out = []; const ok = (name, pass, detail = '') => out.push({ name, pass: !!pass, detail: String(detail) });
  // Specs: the devnet GenesisSpec, the same chainId with another genesis time (conflict), another chainId.
  const hex1 = readFileSync(join(HERE, 'data', 'devnet-spec.hex'), 'utf8').trim().replace(/^0x/, '');
  const mk = (h) => { const pre = '0x' + h; const b = H.fromHex(pre); const sp = RP.decodeSpec(b); return { pre, hash: H.toHex(C.keccak(b)), chainId: Number(sp.chainId) }; };
  const s1 = mk(hex1), s2 = mk(hex1.replace('846553f100', '846553f101')), s3 = mk(hex1.replace('830bdee8', '830bdee9'));
  ok('fixture specs: s2 shares the chainId with another genesis, s3 has another chainId', s1.chainId === s2.chainId && s1.hash !== s2.hash && s3.chainId !== s1.chainId, `${s1.chainId} ${s3.chainId}`);
  const netKeyOf = (sp) => H.toHex(C.keccak(R.encode([new TextEncoder().encode('PoCol-net-v1'), R.encUint(BigInt(sp.chainId)), H.fromHex(sp.hash)])));
  const prof = (name, s, endpoint = 'http://127.0.0.1:8599') => ({ name, chainId: s.chainId, genesisPre: s.pre, genesisHash: s.hash, endpoint, trustLevel: 'LN' });

  // The 30 s identity ticks are captured (not scheduled) so the tests can fire them.
  const ticks = []; const realSI = globalThis.setInterval, realCI = globalThis.clearInterval;
  globalThis.setInterval = (f, ms, ...a) => { if (ms === 30000) { const t = { f, live: true }; ticks.push(t); return t; } return realSI(f, ms, ...a); };
  globalThis.clearInterval = (t) => { if (t && t.f) t.live = false; else realCI(t); };
  const realNow = Date.now; let skew = 0; Date.now = () => realNow() + skew;

  let n = 0;
  /** A fresh worker with one LN profile p1 (s1) open in tab 7, wallet unlocked and accounts granted. */
  async function world({ grant = true, wallet = true, extra = [] } = {}) {
    const env = { genesis: s1.hash, chainId: s1.chainId, gates: {} };
    const gate = (m) => { let open; env.gates[m] = new Promise((r) => { open = r; }); return () => { delete env.gates[m]; open(); }; };
    const mock = installChromeMock(); const fx = await siteFixture(dist);
    const wait = async (m, v) => { if (env.gates[m]) await env.gates[m]; return typeof v === 'function' ? v() : v; };
    const log = installFetchMock({ ...fx.handlers,
      eth_chainId: () => wait('eth_chainId', () => '0x' + env.chainId.toString(16)), pocol_status: () => wait('pocol_status', () => ({ genesisHash: env.genesis })),
      eth_getTransactionCount: () => wait('eth_getTransactionCount', () => (env.nonce === undefined ? '0x0' : env.nonce)), eth_getBlockByNumber: () => wait('eth_getBlockByNumber', { number: '0x5', baseFeePerGas: '0x3b9aca00' }),
      eth_estimateGas: () => wait('eth_estimateGas', '0x5208'), eth_sendRawTransaction: ([raw]) => wait('eth_sendRawTransaction', () => (env.sendFail ? { __raw: () => 'not json' } : H.toHex(C.keccak(H.fromHex(raw))))),
      pocol_getLogs: () => wait('pocol_getLogs', []) });
    await loadWorker(dist, 'wt' + ++n);
    for (const p of [prof('p1', s1), ...extra]) { const r = await mock.send({ type: 'profiles.add', profile: p }); if (!r.ok) throw new Error('profile ' + JSON.stringify(r)); }
    if (wallet) await mock.send({ type: 'wallet.create', password: 'x', seedHex: '0x' + '42'.repeat(32) });
    const t0 = ticks.length; const op = await mock.send({ type: 'open', name: 'p1', site: fx.site, path: '/' });
    if (!op.ok) throw new Error('open ' + JSON.stringify(op));
    const port = mock.connect('bridge:' + op.sessionId); const tick = ticks[t0]; const tickInfo = { t0, n: ticks.length };
    let seq = 0;
    const req = (method, params = []) => { const s = ++seq; port.post({ seq: s, frame: 1, text: JSON.stringify({ id: s, kind: 'wallet_req', payload: { method, params } }) }); return s; };
    const reply = async (s) => (await port.next((m) => m.seq === s)).reply;
    const ask = (type) => port.next((m) => m.ask !== undefined && m.q.type === type);
    const answer = (a, yes) => port.post({ answer: a.ask, ok: yes });
    const count = (m) => log.filter((x) => x.method === m).length;
    let address = null;
    if (grant) { const s = req('eth_requestAccounts'); answer(await ask('confirmAccounts'), true); const r = await reply(s); address = r.result && r.result[0]; }
    const tx = () => req('eth_sendTransaction', [{ to: '0x' + '22'.repeat(20), value: '0x1' }]);
    // A06 round 2 barrier: four sessions (this one and tabs 8-10) each hold an 8 MiB pocol_getLogs
    // behind the gate, so SITE_POOL (32 MiB) is full and the next reservation waits in the queue.
    const fill = async () => {
      const release = gate('pocol_getLogs'); const ports = [port];
      for (const tab of [8, 9, 10]) { const o = await mock.send({ type: 'open', name: 'p1', site: fx.site, path: '/' }, tab); ports.push(mock.connect('bridge:' + o.sessionId, tab)); }
      ports.forEach((p, i) => p.post({ seq: 1000 + i, frame: 1, text: JSON.stringify({ id: 1000 + i, kind: 'rpc_read', payload: { method: 'eth_getLogs', params: [{ blockHash: '0x' + 'ab'.repeat(32) }] } }) }));
      for (let i = 0; i < 100 && count('pocol_getLogs') < 4; i++) await sleep(10);
      return release;
    };
    return { env, gate, mock, port, req, reply, ask, answer, count, tick, tickInfo, tx, address, log, op, site: fx.site, fill };
  }
  const code = (r) => (r && r.error ? r.error.code : 'ok');
  // A scenario that throws (e.g. waits for a reply that never comes) is a failure, not a crash.
  let sc = 0; const scenario = async (f) => { sc++; try { await f(); } catch (e) { ok(`scenario ${sc} completed`, false, e && e.message); } };
  const noAsk = (w, type) => !w.port.inbox.some((m) => m.ask !== undefined && m.q.type === type);

  // ---- A06-conflict: both profiles of a chainId with two genesis hashes are read-only (4100) ----
  await scenario(async () => {
    const w = await world({ grant: false, extra: [prof('p2', s2)] });
    const list = (await w.mock.send({ type: 'profiles.list' })).profiles;
    ok('A06 conflict is recomputed over the set: both profiles flagged', list.every((p) => p.conflict === true), JSON.stringify(list.map((p) => [p.name, p.conflict])));
    const r1 = await w.reply(w.req('eth_requestAccounts')); const r2 = await w.reply(w.req('eth_accounts')); const r3 = await w.reply(w.tx());
    ok('A06 conflicting profile: eth_requestAccounts 4100 without a dialog, eth_accounts [], eth_sendTransaction 4100, nothing sent',
      code(r1) === 4100 && noAsk(w, 'confirmAccounts') && Array.isArray(r2.result) && r2.result.length === 0 && code(r3) === 4100 && w.count('eth_sendRawTransaction') === 0, JSON.stringify([r1, r2, r3]));
    await w.mock.send({ type: 'profiles.remove', name: 'p2' });
    const s = w.req('eth_requestAccounts'); w.answer(await w.ask('confirmAccounts'), true); const r4 = await w.reply(s);
    ok('A06 conflict cleared by removing the other profile: the session survives and accounts can be granted', Array.isArray(r4.result) && r4.result.length === 1 && !w.port.port.disconnected, JSON.stringify(r4));
  });
  // ---- A06 legitimate path: the dialog shows the unsigned bytes; the signature commits to them after approval ----
  await scenario(async () => {
    const w = await world();
    const s = w.tx(); const a = await w.ask('confirmTx'); const st0 = w.count('pocol_status');
    const shown = H.fromHex(a.q.payload);
    ok('A06 confirmTx carries the unsigned payload and its signing hash, no signed bytes', a.q.raw === undefined && a.q.txHash === undefined && shown[0] === 2 && H.toHex(C.keccak(shown)) === a.q.signingHash, Object.keys(a.q).join(','));
    ok('A06 nothing signed or sent before the click', w.count('eth_sendRawTransaction') === 0);
    w.answer(a, true); const r = await w.reply(s);
    const sent = w.log.filter((x) => x.method === 'eth_sendRawTransaction'); const raw = sent.length ? H.fromHex(sent[0].params[0]) : new Uint8Array(1);
    let pre = null, signer = null;
    try { const f = R.decode(raw.subarray(1)); pre = H.toHex(H.concat(Uint8Array.of(2), R.encode(f.slice(0, 9)))); const pad = (b) => H.concat(new Uint8Array(32 - b.length), b);
      signer = H.toHex(C.recover(C.keccak(H.fromHex(pre)), H.concat(pad(f[10]), pad(f[11]), Uint8Array.of(f[9].length ? f[9][0] : 0)))); } catch {}
    ok('A06 approved: one submission whose signed preimage is the displayed payload, signed by the granted account, result = keccak(raw)',
      sent.length === 1 && pre === a.q.payload && signer === w.address.toLowerCase() && r.result === H.toHex(C.keccak(raw)), `${sent.length} ${signer} ${w.address}`);
    ok('A06 identity re-checked at signing (pocol_status after the click)', w.count('pocol_status') === st0 + 1, `${st0} -> ${w.count('pocol_status')}`);
  });
  // ---- A06 rejection ----
  await scenario(async () => { const w = await world(); const s = w.tx(); w.answer(await w.ask('confirmTx'), false); const r = await w.reply(s);
    ok('A06 rejected confirmation -> 4001, nothing sent', code(r) === 4001 && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r)); });
  // ---- A06 lock during confirmation: the request ends 4100 and the dialog is withdrawn; a late click does nothing ----
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx');
    await w.mock.send({ type: 'wallet.lock' }); const r = await w.reply(s); const d = await w.port.next((m) => m.dismiss === a.ask, 2000).catch(() => null);
    w.answer(a, true); await sleep(100);
    ok('A06 wallet.lock during confirmation -> 4100, dialog dismissed, a late approval sends nothing', code(r) === 4100 && d !== null && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  // ---- A06 the audit sequence: endpoint genesis changes and the wallet locks during confirmation ----
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx');
    w.env.genesis = '0x' + '77'.repeat(32); await w.mock.send({ type: 'wallet.lock' }); w.answer(a, true); const r = await w.reply(s);
    ok('A06 audit sequence (endpoint change + lock during confirmation) -> refused, nothing sent', code(r) === 4100 && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  // ---- A06 endpoint identity change during confirmation: refused at signing with 4901, session ended ----
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx');
    w.env.genesis = '0x' + '77'.repeat(32); w.answer(a, true); const r = await w.reply(s); const rv = await w.port.next((m) => m.revoked, 2000).catch(() => null); await sleep(20);
    ok('A06 endpoint genesis changed before signing -> 4901, frame revoked, port closed, nothing sent', code(r) === 4901 && rv && rv.revoked.code === 4901 && w.port.port.disconnected && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx');
    w.env.chainId = s3.chainId; w.answer(a, true); const r = await w.reply(s); await sleep(20);
    ok('A06 endpoint chainId changed before signing -> 4901, nothing sent', code(r) === 4901 && w.port.port.disconnected && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  // ---- A06 profile switch during confirmation ----
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx');
    await w.mock.send({ type: 'profiles.add', profile: prof('p1', s1, 'http://127.0.0.1:8600') }); const r = await w.reply(s);
    const rv = await w.port.next((m) => m.revoked, 2000).catch(() => null); await sleep(20); w.answer(a, true); await sleep(50);
    ok('A06 profile changed (endpoint) during confirmation -> 4901, session revoked, nothing sent', code(r) === 4901 && rv && rv.revoked.code === 4901 && w.port.port.disconnected && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx');
    await w.mock.send({ type: 'profiles.add', profile: prof('p3', s3) }); const r = await w.reply(s); const d = await w.port.next((m) => m.dismiss === a.ask, 2000).catch(() => null);
    const s2 = w.tx(); w.answer(await w.ask('confirmTx'), true); const r2 = await w.reply(s2);
    ok('A06 another profile added during confirmation -> the pending approval is cancelled (4901); the unchanged session signs afterwards',
      code(r) === 4901 && d !== null && !w.port.port.disconnected && typeof r2.result === 'string' && w.count('eth_sendRawTransaction') === 1, JSON.stringify([r, r2]));
  });
  // ---- A06 delayed responses: a transition while an RPC is outstanding ----
  await scenario(async () => {
    const w = await world(); const open = w.gate('eth_estimateGas'); const s = w.tx(); await sleep(50);
    await w.mock.send({ type: 'wallet.lock' }); open(); const r = await w.reply(s);
    ok('A06 wallet.lock while eth_estimateGas is outstanding -> 4100, no dialog, nothing sent', code(r) === 4100 && noAsk(w, 'confirmTx') && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx'); const open = w.gate('pocol_status');
    w.answer(a, true); await sleep(50); await w.mock.send({ type: 'wallet.lock' }); open(); const r = await w.reply(s);
    ok('A06 wallet.lock while the signing identity check is outstanding -> 4100, nothing sent', code(r) === 4100 && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const s = w.tx(); const a = await w.ask('confirmTx'); const open = w.gate('pocol_status');
    w.answer(a, true); await sleep(50); const pr = w.mock.send({ type: 'profiles.add', profile: prof('p1', s1, 'http://127.0.0.1:8600') }); await sleep(50);
    const changedEarly = (await Promise.race([pr.then(() => 'done'), sleep(50).then(() => 'waiting')])) === 'done';
    open(); const r = await w.reply(s); await pr;
    // Round 2: the change waits for signing (ProfileMutex), then cancels the submission that has not
    // started yet (it would otherwise go to the old endpoint). Round 1 expected the transaction sent.
    ok('A06 a profile change waits for ProfileMutex while signing, then cancels the not-yet-initiated submission (4901, nothing sent)',
      !changedEarly && code(r) === 4901 && r.error.data && r.error.data.submission === 'cancelled' && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const open = w.gate('eth_getTransactionCount'); const s = w.tx(); await sleep(50);
    await w.mock.send({ type: 'wallet.create', password: 'y', seedHex: '0x' + '43'.repeat(32) }); open(); const r = await w.reply(s);
    const acc = await w.reply(w.req('eth_accounts'));
    ok('A06 wallet replaced while a request is outstanding -> 4100; the old grant does not cover the new account', code(r) === 4100 && noAsk(w, 'confirmTx') && acc.result && acc.result.length === 0 && w.count('eth_sendRawTransaction') === 0, JSON.stringify([r, acc]));
  });
  // ---- A06 a stored profile that no longer validates (e.g. edited storage) is unusable ----
  await scenario(async () => {
    const w = await world(); const st = structuredClone(w.mock.storage.data.get('profiles')); st[0].genesisHash = '0x' + '00'.repeat(32); w.mock.storage.data.set('profiles', st);
    const list = (await w.mock.send({ type: 'profiles.list' })).profiles; const r = await w.reply(w.tx());
    const op = await w.mock.send({ type: 'open', name: 'p1', site: w.site, path: '/' });
    ok('A06 a stored profile that no longer validates is listed invalid, cannot sign (4901) and cannot open a site',
      list.length === 1 && typeof list[0].invalid === 'string' && code(r) === 4901 && noAsk(w, 'confirmTx') && op.ok === false && op.error.code === 4901 && w.count('eth_sendRawTransaction') === 0, JSON.stringify([list, r, op]));
  });
  // ---- A06 periodic identity check (every 30 s) and the lock timeout ----
  await scenario(async () => {
    const w = await world(); w.env.genesis = '0x' + '77'.repeat(32); const s = w.tx(); const a = await w.ask('confirmTx');
    w.tick.f(); const r = await w.reply(s); const rv = await w.port.next((m) => m.revoked, 2000).catch(() => null); await sleep(20);
    const st = await w.mock.send({ type: 'session.storage', sessionId: w.op.sessionId });
    ok('A06 30 s identity check: endpoint genesis changed -> pending request 4901, frame revoked, session gone, interval cleared',
      code(r) === 4901 && rv && w.port.port.disconnected && st.ok === false && w.tick.live === false && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
    void a;
  });
  await scenario(async () => {
    const w = await world(); const s = w.tx(); await w.ask('confirmTx');
    skew = W.LOCK_MS + 1000; w.tick.f(); const r = await w.reply(s); const st = await w.mock.send({ type: 'wallet.status' }); skew = 0;
    ok('A06 lock timeout reached during confirmation -> 4100 at the next check, wallet reported locked', code(r) === 4100 && st.unlocked === false && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const s = w.tx(); await w.ask('confirmTx'); w.port.close(); await sleep(50);
    ok('A06 viewer disconnected during confirmation -> no signature, interval cleared', w.count('eth_sendRawTransaction') === 0 && w.tick.live === false);
    void s;
  });
  // ---- independent review follow-ups (A06) ----
  await scenario(async () => {
    const w = await world(); const open = w.gate('eth_chainId');
    const op = w.mock.send({ type: 'open', name: 'p1', site: w.site, path: '/' }, 8); await sleep(50);
    await w.mock.send({ type: 'profiles.add', profile: prof('p1', s1, 'http://127.0.0.1:8600') }); open(); const r = await op;
    ok('A06 review: a profile change while a site is opening -> the open is refused (4901), no session on the old profile', r.ok === false && r.error.code === 4901, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world();
    w.port.post({ seq: 900, frame: 1, text: JSON.stringify({ id: 900, kind: 'nav', payload: { method: 'site_openExternal', params: ['https://example.org/'] } }) });
    const a = await w.ask('confirmExternal'); w.env.genesis = '0x' + '77'.repeat(32); w.tick.f(); await w.port.next((m) => m.revoked, 2000); w.answer(a, true); await sleep(100);
    ok('A06 review: an external-link dialog answered after the session was revoked opens no tab', w.mock.tabsCreated.length === 0, JSON.stringify(w.mock.tabsCreated));
  });
  await scenario(async () => {
    const w = await world({ grant: false });
    const un = w.mock.send({ type: 'wallet.unlock', password: 'x' }); await sleep(20);
    await w.mock.send({ type: 'wallet.create', password: 'y', seedHex: '0x' + '43'.repeat(32) }); const r = await un;
    const addr = (await w.mock.send({ type: 'wallet.address', name: 'p1' })).address;
    const want = W.addressOf(await W.networkKey(H.fromHex('0x' + '43'.repeat(32)), netKeyOf(s1)));
    ok('A06 review: an unlock that was decrypting when the wallet was replaced does not reinstate the old seed', r.ok === false && addr === want, JSON.stringify([r, addr, want]));
  });
  await scenario(async () => {
    const w = await world(); w.env.nonce = -1; const t = realNow(); const r = await w.reply(w.tx());
    ok('A06 review: a negative quantity from the endpoint fails the request (no hang, nothing sent)', code(r) === -32603 && realNow() - t < 3000 && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    // Earlier scenarios' workers resolve the global chrome mock at call time: let their delayed DNR
    // removals (revocations of tab 7 sessions, REVOKE_DNR_GRACE_MS) fire before this world exists.
    await sleep(2500);
    const w = await world(); const tab = (r) => r.filter((x) => x.condition.tabIds[0] === 7); const before = tab(w.mock.rules()).length;
    w.env.genesis = '0x' + '77'.repeat(32); w.tick.f(); await w.port.next((m) => m.revoked, 2000); await sleep(100);
    const during = tab(w.mock.rules()).length; await sleep(2300); const after = tab(w.mock.rules()).length;
    ok('A06 review: a revoked tab keeps its DNR rules while the frame is torn down, then they are removed', before === 2 && during === 2 && after === 0, `${before} -> ${during} -> ${after}`);
  });
  // ---- A06 round 2: submission states (signed -> queued -> initiated -> broadcast) ----
  // Deterministic barrier: the site pool is held full by four gated 8 MiB reads, so an approved,
  // signed transaction waits in the transport queue; the transition happens while it is queued.
  // The transaction is built (its reads use the site pool) and shown first; the pool fills while
  // the dialog is open; after approval, signing uses the trust pool and the submission queues.
  const queued = async (w) => { const s = w.tx(); const a = await w.ask('confirmTx'); const release = await w.fill(); const st0 = w.count('pocol_status');
    w.answer(a, true); for (let i = 0; i < 200 && w.count('pocol_status') === st0; i++) await sleep(5); await sleep(50); return [s, release]; };
  const cancelled = (r, c, before) => code(r) === c && r.error.data && r.error.data.submission === 'cancelled' && r.error.data.before === before;
  await scenario(async () => {
    const w = await world(); const [s, release] = await queued(w);
    const rp = w.reply(s); const pending = await Promise.race([rp.then(() => 'replied'), sleep(100).then(() => 'queued')]);
    await w.mock.send({ type: 'wallet.lock' }); const r = await rp; release(); await sleep(150);
    ok('A06-R2 the signed transaction waits in the transport queue while the pool is full (not yet sent)', pending === 'queued' && w.count('pocol_getLogs') === 4, `${pending} ${w.count('pocol_getLogs')}`);
    ok('A06-R2 wallet.lock while queued -> 4100 {submission: cancelled, before: queued}; never sent, also after the pool frees', cancelled(r, 4100, 'queued') && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const [s, release] = await queued(w);
    await w.mock.send({ type: 'profiles.add', profile: prof('p1', s1, 'http://127.0.0.1:8600') }); const r = await w.reply(s); release(); await sleep(150);
    const sentTo = w.log.filter((x) => x.method === 'eth_sendRawTransaction').map((x) => x.url);
    ok('A06-R2 endpoint changed while queued -> 4901 cancelled before queued; nothing sent to the old or the new endpoint', cancelled(r, 4901, 'queued') && sentTo.length === 0 && w.port.port.disconnected, JSON.stringify([r, sentTo]));
  });
  await scenario(async () => {
    const w = await world(); const [s, release] = await queued(w);
    if (!w.tick) throw new Error(`no tick captured: ${JSON.stringify(w.tickInfo)}`);
    w.env.genesis = '0x' + '77'.repeat(32); w.tick.f(); const r = await w.reply(s); release(); await sleep(150);
    ok('A06-R2 identity revocation (30 s check) while queued -> 4901 cancelled before queued, nothing sent', cancelled(r, 4901, 'queued') && w.count('eth_sendRawTransaction') === 0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); const [s, release] = await queued(w);
    w.port.close(); release(); await sleep(150);
    ok('A06-R2 viewer disconnected while queued -> the submission is cancelled, nothing sent', w.count('eth_sendRawTransaction') === 0, String(w.count('eth_sendRawTransaction')));
  });
  await scenario(async () => {
    // The lock timeout has no event: only the synchronous re-check after the reservation sees it.
    const w = await world(); const [s, release] = await queued(w);
    const logs = w.count('pocol_getLogs'), early = w.port.inbox.some((m) => m.seq === s);
    skew += 16 * 60 * 1000; release(); const r = await w.reply(s); skew = 0;
    ok('A06-R2 lock timeout while queued is caught by the re-check after the reservation, immediately before the request -> 4100 cancelled, nothing sent',
      cancelled(r, 4100, 'queued') && w.count('eth_sendRawTransaction') === 0, JSON.stringify({ r, logs, early }));
  });
  await scenario(async () => {
    // Initiated: the request has started; a later lock does not (and cannot) cancel it.
    const w = await world(); const hold = w.gate('eth_sendRawTransaction'); const st0 = w.count('pocol_status'); const s = w.tx(); w.answer(await w.ask('confirmTx'), true);
    for (let i = 0; i < 200 && w.count('eth_sendRawTransaction') === 0; i++) await sleep(5);
    await w.mock.send({ type: 'wallet.lock' }); hold(); const r = await w.reply(s);
    ok('A06-R2 wallet.lock after the request was initiated -> not reported as cancelled; the broadcast hash is returned', w.count('eth_sendRawTransaction') === 1 && typeof r.result === 'string' && w.count('pocol_status') > st0, JSON.stringify(r));
  });
  await scenario(async () => {
    const w = await world(); w.env.sendFail = true; const s = w.tx(); w.answer(await w.ask('confirmTx'), true); const r = await w.reply(s);
    ok('A06-R2 a failure after initiation reports {submission: initiated} (outcome unknown), never cancelled', code(r) === -32603 && r.error.data.submission === 'initiated' && w.count('eth_sendRawTransaction') === 1, JSON.stringify(r));
  });
  globalThis.setInterval = realSI; globalThis.clearInterval = realCI; Date.now = realNow;
  // ---- A07 review: the gsVersion detail of a huge specVersion item is computed in linear time ----
  {
    const huge = R.encode([new Uint8Array(200000).fill(0x9a)]); const t = Date.now(); let e = null; try { RP.decodeSpec(huge); } catch (x) { e = x; }
    ok('A07 review: a 200 KB specVersion item is rejected (gsVersion) quickly', e && e.gs === 'gsVersion' && Date.now() - t < 2000, `${e && e.gs} ${Date.now() - t} ms`);
  }

  // ---- A12: the admission slot of an unsettled write is kept after its storeTimeout reply ----
  await scenario(async () => {
    const mock = installChromeMock(); const fx = await siteFixture(dist);
    installFetchMock({ ...fx.handlers, eth_chainId: () => '0x' + s1.chainId.toString(16), pocol_status: () => ({ genesisHash: s1.hash }) });
    await loadWorker(dist, 'wt-a12'); await mock.send({ type: 'profiles.add', profile: prof('p', s1) });
    const op = await mock.send({ type: 'open', name: 'p', site: fx.site, path: '/' }); const port = mock.connect('bridge:' + op.sessionId); let seq = 0;
    const set = (k) => { const s = ++seq; port.post({ seq: s, frame: 1, text: JSON.stringify({ id: s, kind: 'storage_set', payload: { method: 'site_storageSet', params: [k, 'v'] } }) }); return s; };
    mock.storage.holdSets = 1; const first = set('k0'); const r1 = await port.next((m) => m.seq === first, 9000);
    // The site's FIFO chain is blocked behind the unsettled write: admitted writes wait, only a refusal replies at once.
    const burst = []; for (let i = 1; i <= 8; i++) burst.push(set('k' + i)); await sleep(300);
    const early = burst.map((s) => { const m = port.inbox.find((x) => x.seq === s); return m ? (m.reply.error && m.reply.error.data && m.reply.error.data.reason) || 'ok' : 'pending'; });
    ok('A12 unsettled write keeps its admission slot after storeTimeout: of 8 further writes 7 are admitted and the 8th is refused {store}',
      r1.reply.error && r1.reply.error.data.reason === 'storeTimeout' && early.slice(0, 7).every((x) => x === 'pending') && early[7] === 'store', `${JSON.stringify(r1.reply.error && r1.reply.error.data)} ${early.join(',')}`);
    mock.storage.release(); const done = []; for (const s of burst.slice(0, 7)) done.push((await port.next((m) => m.seq === s, 9000)).reply);
    const again = set('k9'); const r2 = (await port.next((m) => m.seq === again, 9000)).reply;
    ok('A12 once the write settles the queued writes complete and the slots return', done.every((r) => r.result === null) && r2.result === null, JSON.stringify([done.map((r) => r.result === null), r2]));
  });

  // ---- A09: one session receive budget for concurrent requests; released once on every path ----
  await scenario(async () => {
    const pool = new T.Pool(T.SITE_POOL, 48); const session = { recv: 0 }; let peak = 0;
    const slow = async () => { peak = Math.max(peak, session.recv); await sleep(60); return new Response('{"jsonrpc":"2.0","id":1,"result":[]}', { status: 200 }); };
    const rs = await Promise.allSettled([0, 1, 2].map(() => T.request('http://x/', 'pocol_getLogs', [{}], { pool, session, fetchImpl: slow })));
    ok('A09 three concurrent 8 MiB requests: peak reservation <= SESSION_RECV_MAX, one served, two busy, all released',
      peak <= T.SESSION_RECV_MAX && rs.filter((r) => r.status === 'fulfilled').length === 1 && rs.filter((r) => r.status === 'rejected' && r.reason.transport === 'busy').length === 2 && session.recv === 0 && pool.free === pool.cap,
      `peak=${peak} ${rs.map((r) => r.status).join(',')} recv=${session.recv}`);
    const paths = {
      'fetch error': { fetchImpl: async () => { throw new Error('net'); } },
      deadline: { deadline: 30, fetchImpl: (u, i) => new Promise((_, rej) => i.signal.addEventListener('abort', () => rej(new Error('abort')))) },
      'HTTP 500': { fetchImpl: async () => new Response('{"jsonrpc":"2.0","id":1,"result":"0x1"}', { status: 500 }) },
      'pool exhausted': { pool: new T.Pool(1024, 48), fetchImpl: slow },
    };
    for (const [name, o] of Object.entries(paths)) {
      const s = { recv: 0 }; const p = o.pool || new T.Pool(T.SITE_POOL, 48);
      const r = await T.request('http://x/', 'eth_blockNumber', [], { pool: p, session: s, id: 1, ...o }).then(() => 'ok', (e) => e.transport || e.message);
      ok(`A09 ${name}: fails closed and releases the session and pool shares`, r !== 'ok' && s.recv === 0 && p.free === p.cap, `${r} recv=${s.recv} free=${p.free}/${p.cap}`);
    }
  });
  // ---- A14: response correlation ----
  await scenario(async () => {
    const pool = new T.Pool(T.SITE_POOL, 48);
    const run = (body, status = 200) => T.request('http://x/', 'eth_blockNumber', [], { pool, id: 123, fetchImpl: async () => new Response(body, { status }) }).then((v) => ({ v }), (e) => (e.rpc ? { rpc: e.rpc } : { e: e.transport || e.message }));
    const cases = [
      ['jsonrpc 1.0, foreign id (audit witness)', '{"jsonrpc":"1.0","id":999,"result":"0x99"}', 'recvParse'],
      ['foreign id', '{"jsonrpc":"2.0","id":124,"result":"0x1"}', 'recvParse'],
      ['id as string', '{"jsonrpc":"2.0","id":"123","result":"0x1"}', 'recvParse'],
      ['id null', '{"jsonrpc":"2.0","id":null,"result":"0x1"}', 'recvParse'],
      ['missing jsonrpc', '{"id":123,"result":"0x1"}', 'recvParse'],
      ['missing id', '{"jsonrpc":"2.0","result":"0x1"}', 'recvParse'],
      ['result and error', '{"jsonrpc":"2.0","id":123,"result":"0x1","error":{"code":1,"message":"m"}}', 'recvParse'],
      ['neither result nor error', '{"jsonrpc":"2.0","id":123}', 'recvParse'],
      ['error code not an integer', '{"jsonrpc":"2.0","id":123,"error":{"code":"1","message":"m"}}', 'recvParse'],
      ['error message not a string', '{"jsonrpc":"2.0","id":123,"error":{"code":1,"message":5}}', 'recvParse'],
      ['error not an object', '{"jsonrpc":"2.0","id":123,"error":"boom"}', 'recvParse'],
      ['batch array', '[{"jsonrpc":"2.0","id":123,"result":"0x1"}]', 'recvParse'],
      ['HTTP 500 with a result', '{"jsonrpc":"2.0","id":123,"result":"0x1"}', 'transport', 500],
    ];
    for (const [name, body, want, status] of cases) { const r = await run(body, status); ok(`A14 ${name} -> ${want}`, r.e === want, JSON.stringify(r)); }
    const good = await run('{"jsonrpc":"2.0","id":123,"result":"0x1"}'); ok('A14 exact response accepted', good.v === '0x1', JSON.stringify(good));
    const err = await run('{"jsonrpc":"2.0","id":123,"error":{"code":-32000,"message":"' + 'm'.repeat(300) + '","data":{"x":1}}}');
    ok('A14 exact error object passes through (message clipped, data kept)', err.rpc && err.rpc.code === -32000 && err.rpc.message.length === 256 && err.rpc.data.x === 1, JSON.stringify(err).slice(0, 120));
  });
  return out;
}

if (process.argv[1] && process.argv[1].endsWith('worker_test.mjs')) {
  const { build } = await import('../build.mjs'); const r = await workerChecks(build().dist);
  for (const c of r) console.log(`${c.pass ? 'PASS' : 'FAIL'} ${c.name}${c.pass ? '' : ' — ' + c.detail}`);
  const f = r.filter((c) => !c.pass).length; console.log(JSON.stringify({ result: f ? 'FAIL' : 'PASS', passed: r.length - f, failed: f }));
  process.exit(f ? 1 : 0);
}
