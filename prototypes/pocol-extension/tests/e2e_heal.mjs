// D133 at a partition's heal with real pocold processes (2026-10-28), in both directions, mining controlled.
// Each scenario: a fresh devnet (a, b, c); a common prefix mined meshed; a partition (a and b meshed, c alone),
// a and c mining until both branches have their own blocks; then only the side meant to win mines until its
// total work is strictly more. Every node is then stopped and each head is measured with no node mining and no
// peer (so nothing can change it before the heal). The heal restarts the three meshed with no miner: they must
// end on the measured heavier tip, the losing side's nodes must each reorganize onto it, the winners must not,
// and the heads must stay put. Scenario "ab": c follows a and b; scenario "c": a and b follow c.
// Local nodes and loopback RPC only. On a failure the nodes' logs and the run's inputs are kept (harness keep).
import { spawn } from 'node:child_process';
import { mkdtempSync, openSync, appendFileSync, writeFileSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { EXT, BIN, sleep, wait, checker, devnet, rpc, portFree, keepOnFailure } from './harness.mjs';

const { ok, results, failed } = checker();
const EXTRA = (process.env.POCOLD_RUN_ARGS || '').split(' ').filter(Boolean);
const LABELS = ['a', 'b', 'c'];
const runs = [];

// One pocold process. Its stdout and stderr go to WORK/<label>.log, after a line naming the phase and arguments.
function startNode(work, i, { miner, peers, listen = true, phase }) {
  const l = LABELS[i]; const log = join(work, `${l}.log`);
  const args = ['run', '--network', join(work, 'net'), '--data', join(work, 'data', l), '--name', l, '--rpc', `127.0.0.1:${8540 + i}`, '--log',
    ...(miner ? ['--miner', l] : []), ...(listen ? ['--listen', `127.0.0.1:${30300 + i}`] : []), ...peers.flatMap((j) => ['--peer', `127.0.0.1:${30300 + j}`]), ...EXTRA];
  appendFileSync(log, `\n=== ${new Date().toISOString()} ${phase}: pocold ${args.join(' ')}\n`);
  const fd = openSync(log, 'a'); const p = spawn(BIN('pocold'), args, { stdio: ['ignore', fd, fd], detached: true });
  p.label = l; return p;
}
async function stopAll(ps) {
  for (const p of ps) { try { process.kill(-p.pid, 'SIGTERM'); } catch {} }
  for (const p of ps) { if (p.exitCode === null && p.signalCode === null) await Promise.race([new Promise((r) => p.once('exit', r)), sleep(20000)]); try { process.kill(-p.pid, 'SIGKILL'); } catch {} }
  ps.length = 0;
}
const status = (i) => rpc(8540 + i, 'pocol_status');
const up = (is) => wait(async () => { try { return await Promise.all(is.map(status)); } catch { return null; } }, 60000, 'nodes up');
const tip = (s) => ({ hash: s.head.hash, height: s.head.height, work: BigInt(s.head.totalWork) });
const hashAt = async (i, h) => (await rpc(8540 + i, 'eth_getBlockByNumber', ['0x' + h.toString(16), false]))?.hash ?? null;

async function scenario(winner) {
  const work = mkdtempSync(join(tmpdir(), `pocol-heal-${winner}-`)); const ps = []; const run = { winner, work, phases: [] };
  const phase = (name, extra = {}) => { run.phases.push({ name, at: new Date().toISOString(), ...extra }); };
  runs.push(run);
  try {
    devnet(['init', work, '--miners', LABELS.join(','), '--t-blk', '2', '--tau', '20', '--c', '8', '--d-att', '41', '--fb-wait', '3', '--work-bits', '15']);
    run.devnet = JSON.parse(readFileSync(join(work, 'net', 'devnet.json'), 'utf8'));
    // The common prefix, meshed.
    phase('prefix');
    for (const i of [0, 1, 2]) ps.push(startNode(work, i, { miner: true, peers: [0, 1, 2].filter((j) => j < i), phase: 'prefix' }));
    await up([0, 1, 2]);
    await wait(async () => { const s = await Promise.all([0, 1, 2].map(status)); return s.every((x) => x.head.height >= 6 && x.head.hash === s[0].head.hash); }, 240000, 'common prefix of 6 blocks');
    await stopAll(ps);
    // The partition: a and b meshed, c alone. On the a/b side only a mines (b follows it): two miners there can
    // end on two sibling tips of equal work, which D133's tie-break (local import order) leaves apart (the first
    // run of this test, 2026-10-28). Both sides mine until each branch has its own blocks.
    const side = (what, abMiner, cMiner) => { ps.push(startNode(work, 0, { miner: abMiner, peers: [], phase: what }), startNode(work, 1, { miner: false, peers: [0], phase: what }), startNode(work, 2, { miner: cMiner, peers: [], listen: false, phase: what })); };
    phase('partition: a and c mine'); side('partition', true, true);
    const base = Math.min(...(await up([0, 1, 2])).map((s) => s.head.height));
    await wait(async () => { const s = await Promise.all([0, 2].map(status)); return s.every((x) => x.head.height >= base + 2) && (await hashAt(0, base + 1)) !== (await hashAt(2, base + 1)); }, 240000, 'both branches have their own blocks');
    await stopAll(ps);
    // Only the side meant to win mines on, until its total work is strictly more.
    const abMines = winner === 'ab';
    phase(`partition: only ${abMines ? 'a' : 'c'} mines`); side('partition 2', abMines, !abMines);
    await up([0, 1, 2]);
    await wait(async () => { const [a, , c] = await Promise.all([0, 1, 2].map(status)); const wa = BigInt(a.head.totalWork), wc = BigInt(c.head.totalWork); return abMines ? wa > wc : wc > wa; }, 300000, `the ${winner} branch heavier`);
    await stopAll(ps);
    // No miner from here on. b catches up with a (a block a made just before it stopped), then all stop.
    phase('partition: no miner, b catches up with a'); side('partition, no miner', false, false);
    await wait(async () => { const [a, b] = await Promise.all([0, 1].map(status)); return a.head.hash === b.head.hash; }, 60000, 'b on a\'s tip');
    await stopAll(ps);
    // The measurement: each node alone, with no miner and no peer.
    for (const i of [0, 1, 2]) ps.push(startNode(work, i, { miner: false, peers: [], listen: false, phase: 'measure' }));
    const m = (await up([0, 1, 2])).map(tip); await sleep(3000); const m2 = (await Promise.all([0, 1, 2].map(status))).map(tip);
    await stopAll(ps);
    phase('measured', { tips: m.map((x) => ({ ...x, work: x.work.toString() })) });
    ok(`${winner}: positive control: a and b share one tip, c another, and nothing moved while measured`, m[0].hash === m[1].hash && m[0].hash !== m[2].hash && m.every((x, i) => x.hash === m2[i].hash), JSON.stringify(run.phases.at(-1).tips));
    const win = abMines ? m[0] : m[2], lose = abMines ? m[2] : m[0];
    ok(`${winner}: positive control: the ${winner} branch has strictly more total work`, win.work > lose.work, `win ${win.work} at ${win.height}, lose ${lose.work} at ${lose.height}`);
    // The heal: meshed, no miner.
    phase('heal');
    for (const i of [0, 1, 2]) ps.push(startNode(work, i, { miner: false, peers: [0, 1, 2].filter((j) => j < i), phase: 'heal (no miner)' }));
    await up([0, 1, 2]);
    let st = [];
    const converged = await wait(async () => { st = await Promise.all([0, 1, 2].map(status)); return st.every((x) => x.head.hash === win.hash); }, 120000, 'the measured heavier tip on all three').then(() => true, () => false);
    if (!converged) st = await Promise.all([0, 1, 2].map(status));
    const after = st.map((x) => ({ head: x.head.hash, height: x.head.height, reorgs: x.stats.reorgs, syncSessions: x.stats.syncSessions }));
    phase('healed', { after });
    ok(`${winner}: the three nodes end on the heavier tip measured before the heal (D133)`, converged, JSON.stringify(after));
    const losers = abMines ? [2] : [0, 1];
    ok(`${winner}: the losing side reorganized onto it, the winning side did not`, [0, 1, 2].every((i) => (losers.includes(i) ? after[i].reorgs >= 1 : after[i].reorgs === 0)), JSON.stringify(after.map((x) => x.reorgs)));
    await sleep(4000); const later = (await Promise.all([0, 1, 2].map(status))).map((x) => x.head.hash);
    ok(`${winner}: with no miner the heads stay on that tip`, later.every((h) => h === win.hash), later.join(','));
    let same = true; for (let h = 1; h <= win.height; h++) { const hs = await Promise.all([0, 1, 2].map((i) => hashAt(i, h))); if (!hs.every((x) => x && x === hs[0])) same = false; }
    ok(`${winner}: every canonical block 1..${win.height} is the same on the three nodes`, same, '');
  } finally { await stopAll(ps); }
}

async function main() {
  for (const p of [30300, 30301, 30302, 8540, 8541, 8542]) if (!(await portFree(p))) throw new Error('port busy ' + p);
  for (const w of ['ab', 'c']) await scenario(w);
}

try { await main(); } catch (e) { ok('e2e_heal driver completed', false, e.stack || e.message); }
const summary = { result: failed() ? 'FAIL' : 'PASS', checks: results.length, failed: failed(), runs: runs.map((r) => ({ winner: r.winner, work: r.work })) };
if (failed()) summary.kept = runs.map((r) => keepOnFailure('e2e_heal', r.work, { results, phases: r.phases, winner: r.winner, devnet: r.devnet }));
writeFileSync(join(EXT, 'tests', 'e2e_heal.last.json'), JSON.stringify({ ...summary, results, runs }, (k, v) => (typeof v === 'bigint' ? v.toString() : v), 1) + '\n');
console.log(JSON.stringify(summary));
process.exit(failed() ? 1 : 0);
