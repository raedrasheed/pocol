import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, unlinkSync } from 'node:fs';
import path from 'node:path';
import { Controller, continuationPolicy } from '../src/broker.mjs';
import { buildNotifyArgs, notificationMessage } from '../src/notifier.mjs';
import { SESSION, clock, fakeSpawner, makeWorkspace, resultLine } from './helpers.mjs';

// The host coordinator is simulated only through the reviewer API (attach/claim/ack/review).
// Nothing here fabricates a review: every review below is an explicit call, as the real host makes it.
const HOST = 'codex-host-sim';
const THREAD = '11111111-2222-4333-8444-555555555555';
const AUTH = { at: '2026-10-08T08:02:53.390Z', scope: 'M1 specifications, reference models, tests and coordinator improvements',
  autonomousSequentialCycles: true, delegatedTechnicalDecisions: true, personalApproval: false, noProduction: true };
const WORK = { status: 'ready', remainingIndependentItems: [
  { id: 'coordinator-improvement', scope: 'M1', kind: 'coordinatorImprovement' }, { id: 'publish-final-verify', scope: 'M1', kind: 'finalVerification' }] };

const ledgerFile = (ws) => path.join(ws, 'coordination', 'issue-ledger.json');
const readLedger = (ws) => JSON.parse(readFileSync(ledgerFile(ws), 'utf8').replace(/^﻿/, ''));
const patchLedger = (ws, patch) => writeFileSync(ledgerFile(ws), '﻿' + JSON.stringify({ ...readLedger(ws), ...patch }));

function fakeNotifier({ results = [] } = {}) {
  const sends = [];
  return {
    sends,
    configured: () => ({ ok: true }),
    describe: () => ({ configured: true, remote: 'unix://', threadId: THREAD }),
    send(info) { sends.push(info); const r = results.length ? results.shift() : { ok: true, queueId: 'queued-0001' }; return r instanceof Promise ? r : Promise.resolve(r); },
  };
}

function controller(ws, c, notifier, sp = fakeSpawner()) {
  return new Controller({ workspace: ws, claudeBin: 'claude-fake', spawnImpl: sp.spawnImpl, isAlive: () => false, now: c.now, notifier, watchMs: 10 });
}

function setup({ enabled = true, ws = makeWorkspace(), c = clock(), notifier = fakeNotifier(), sp = fakeSpawner() } = {}) {
  if (enabled) patchLedger(ws, { standingAuthorization: AUTH, continuationWork: WORK });
  const ctl = controller(ws, c, notifier, sp);
  ctl.attach({ reviewerId: HOST });
  return { ws, c, sp, ctl, notifier };
}

/** Browser request + coordinator approval: the job starts through the normal gates. */
function startSmoke(ctl, key) {
  const r = ctl.requestAuthorJob({ mode: 'smoke', idempotencyKey: key });
  ctl.ack({ itemId: r.item.id, dispatch: {} });
  return r.item.id;
}
function finishCurrent(ctl, sp) {
  const jobId = ctl.state.currentJob;
  writeFileSync(path.join(ctl.uiDir, 'jobs', jobId, 'receipt.jsonl'), resultLine(false));
  sp.calls.at(-1).child.emit('exit', 0, null);
  return jobId;
}
const conts = (ctl) => ctl.state.order.map((id) => ctl.state.items[id]).filter((i) => i.kind === 'continuation');
const contSends = (n) => n.sends.filter((s) => s.reason === 'continuation');

test('default off: without saved standing authorization a review queues no continuation and notifies nothing', async () => {
  const { ctl, sp, notifier } = setup({ enabled: false });
  startSmoke(ctl, 'off-job-00001');
  const jobId = finishCurrent(ctl, sp);
  const rv = ctl.review({ jobId, verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  await ctl.notifyIdle();
  assert.equal(conts(ctl).length, 0);
  assert.deepEqual([rv.continuation.status, rv.continuation.reason], ['suppressed', 'noStandingAuthorization']);
  assert.equal(contSends(notifier).length, 0);
  assert.equal(sp.calls.length, 1, 'nothing started after the review');
  assert.equal(ctl.view().authority.continuation.enabled, false);
});

test('enabled: exactly one durable continuation item and one fixed notification per actual review, also after restart', async () => {
  const { ws, c, ctl, sp, notifier } = setup();
  startSmoke(ctl, 'one-job-00001');
  const jobId = finishCurrent(ctl, sp);
  const rv = ctl.review({ jobId, verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  await ctl.notifyIdle();
  const all = conts(ctl);
  assert.equal(all.length, 1);
  const [item] = all;
  assert.equal(item.status, 'queued');
  assert.equal(item.payload.reviewId, rv.id);
  assert.deepEqual(item.payload.readyItems.map((x) => x.id), ['coordinator-improvement', 'publish-final-verify']);
  assert.deepEqual([rv.continuation.status, rv.continuation.itemId], ['queued', item.id]);
  const sent = contSends(notifier);
  assert.equal(sent.length, 1);
  assert.deepEqual([sent[0].itemId, sent[0].kind, sent[0].status], [item.id, 'continuation', 'queued']);
  assert.equal(ctl.state.reviews.length, 1, 'no review is invented');
  assert.equal(sp.calls.length, 1, 'no author is started by the continuation');
  assert.ok(ctl.reviewerQueue().some((i) => i.id === item.id), 'the host sees it in its reviewer queue');
  const n2 = fakeNotifier();
  const ctl2 = controller(ws, c, n2);
  ctl2.recover(); await ctl2.notifyIdle();
  assert.equal(conts(ctl2).length, 1, 'no second item after restart');
  assert.equal(n2.sends.length, 0, 'no repeated notification after restart');
});

test('policy: default off; every missing, malformed, complete or blocked-without-ready-item ledger disables it', () => {
  const L = (a, w) => ({ standingAuthorization: a, continuationWork: w });
  const cases = [
    [null, 'ledgerUnreadable'],
    [{}, 'noStandingAuthorization'],
    [L({ ...AUTH, autonomousSequentialCycles: false }, WORK), 'autonomousCyclesOff'],
    [L({ ...AUTH, autonomousSequentialCycles: 'true' }, WORK), 'autonomousCyclesOff'],
    [L({ ...AUTH, noProduction: undefined }, WORK), 'noProductionNotTrue'],
    [L({ ...AUTH, scope: 'M2 implementation' }, WORK), 'scopeNotM1'],
    [L({ ...AUTH, scope: 'M10 only' }, WORK), 'scopeNotM1'],
    [L(AUTH, undefined), 'noWorklist'],
    [L(AUTH, { ...WORK, status: 'complete' }), 'worklistComplete'],
    [L(AUTH, { ...WORK, status: 'paused' }), 'worklistNotReady'],
    [L(AUTH, { status: 'ready', remainingIndependentItems: 'x' }), 'worklistMalformed'],
    [L(AUTH, { status: 'ready', remainingIndependentItems: [{ id: 'a b', scope: 'M1' }] }), 'worklistMalformed'],
    [L(AUTH, { status: 'ready', remainingIndependentItems: [{ id: 'x1', scope: 'M2' }] }), 'worklistMalformed'],
    [L(AUTH, { status: 'ready', remainingIndependentItems: [{ id: 'x1', scope: 'M1', status: 'weird' }] }), 'worklistMalformed'],
    [L(AUTH, { status: 'ready', remainingIndependentItems: [] }), 'noIndependentReadyItem'],
    [L(AUTH, { status: 'blocked', remainingIndependentItems: [{ id: 'x1', scope: 'M1', status: 'blocked' }, { id: 'x2', scope: 'M1', status: 'done' }] }), 'noIndependentReadyItem'],
  ];
  for (const [ledger, reason] of cases) {
    const p = continuationPolicy(ledger);
    assert.equal(p.enabled, false, reason);
    assert.equal(p.reason, reason);
    assert.deepEqual(p.items, []);
  }
  const blocked = continuationPolicy(L(AUTH, { status: 'blocked', blocker: 'concrete blocker', remainingIndependentItems: [
    { id: 'x1', scope: 'M1', status: 'blocked' }, { id: 'x3', scope: 'M1', kind: 'finalVerification' }] }));
  assert.equal(blocked.enabled, true, 'a concrete blocker with independent ready work continues only that work');
  assert.deepEqual(blocked.items.map((x) => x.id), ['x3']);
});

test('completion: a complete worklist suppresses the callback after an actual review', async () => {
  const { ws, ctl, sp, notifier } = setup();
  patchLedger(ws, { continuationWork: { ...WORK, status: 'complete', remainingIndependentItems: [] } });
  startSmoke(ctl, 'done-job-0001');
  const rv = ctl.review({ jobId: finishCurrent(ctl, sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  await ctl.notifyIdle();
  assert.deepEqual([rv.continuation.status, rv.continuation.reason], ['suppressed', 'worklistComplete']);
  assert.equal(conts(ctl).length, 0); assert.equal(contSends(notifier).length, 0);
});

test('pause defers the continuation; a manual resume decides it once; stop suppresses', async () => {
  const { ctl, sp, notifier } = setup();
  startSmoke(ctl, 'pause-job-001');
  ctl.pause({ idempotencyKey: 'cont-pause-01' });
  assert.equal(ctl.pauseState(), 'draining');
  const rv = ctl.review({ jobId: finishCurrent(ctl, sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  assert.equal(rv.continuation.status, 'deferredPaused');
  assert.equal(conts(ctl).length, 0);
  ctl.resume({ idempotencyKey: 'cont-resume-1', reason: 'manual' });
  await ctl.notifyIdle();
  assert.equal(conts(ctl).length, 1); assert.equal(rv.continuation.status, 'queued');
  ctl.resume({ idempotencyKey: 'cont-resume-2', reason: 'again' });
  ctl.pause({ idempotencyKey: 'cont-pause-02' }); ctl.resume({ idempotencyKey: 'cont-resume-3' });
  await ctl.notifyIdle();
  assert.equal(conts(ctl).length, 1, 'repeated resumes never add items');
  assert.equal(contSends(notifier).length, 1, 'and never repeat the notification');

  const s = setup();
  startSmoke(s.ctl, 'stop-job-0001');
  const jobId = finishCurrent(s.ctl, s.sp);
  s.ctl.stop();
  const rv2 = s.ctl.review({ jobId, verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  assert.deepEqual([rv2.continuation.status, rv2.continuation.reason], ['suppressed', 'stopped']);
  assert.equal(conts(s.ctl).length, 0);
});

test('gates: an active writer or an open continuation suppresses; an acknowledged one allows the next', async () => {
  const w = setup();
  startSmoke(w.ctl, 'gate-job-0001');
  const second = w.ctl.requestAuthorJob({ mode: 'smoke', idempotencyKey: 'gate-job-0002' });
  w.ctl.ack({ itemId: second.item.id, dispatch: {} });
  assert.equal(w.sp.calls.length, 1, 'the second approved job waits');
  const rvA = w.ctl.review({ jobId: finishCurrent(w.ctl, w.sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  assert.equal(w.sp.calls.length, 2, 'the review released the already-approved job');
  assert.deepEqual([rvA.continuation.status, rvA.continuation.reason], ['suppressed', 'writerActive']);

  const { ctl, sp, notifier } = setup();
  startSmoke(ctl, 'open-job-0001');
  ctl.review({ jobId: finishCurrent(ctl, sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  const [c1] = conts(ctl);
  startSmoke(ctl, 'open-job-0002');
  const rv2 = ctl.review({ jobId: finishCurrent(ctl, sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  assert.deepEqual([rv2.continuation.status, rv2.continuation.reason], ['suppressed', 'openContinuation']);
  assert.equal(conts(ctl).length, 1, 'never a stream of items');
  ctl.claim({ itemId: c1.id }); ctl.ack({ itemId: c1.id, note: 'no action needed' });
  startSmoke(ctl, 'open-job-0003');
  const rv3 = ctl.review({ jobId: finishCurrent(ctl, sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  await ctl.notifyIdle();
  assert.equal(rv3.continuation.status, 'queued');
  assert.equal(conts(ctl).length, 2);
  assert.equal(contSends(notifier).length, 2, 'one notification per queued continuation');
});

test('second author: dispatched only AFTER the actual first review, through claim/ack, with a matching reviewed ledger; every gate still blocks', async () => {
  const { ws, ctl, sp, notifier } = setup();
  const r1 = ctl.requestAuthorJob({ mode: 'author', idempotencyKey: 'seq-job-00001' });
  ctl.ack({ itemId: r1.item.id, dispatch: { taskFile: 'task-006.md', plannedDir: 'm1-draft-0.20' } });
  assert.equal(sp.calls.length, 1);
  const active = { task: 'task-006.md', claudeSessionId: SESSION, state: 'running', brokerJobId: r1.item.id };
  patchLedger(ws, { activeAuthor: active });
  const job1 = finishCurrent(ctl, sp);
  assert.match(ctl.dispatchBlock(), /بانتظار مراجعة Codex الفعلية/, 'before the actual review nothing may start');
  assert.equal(conts(ctl).length, 0, 'no continuation before a review');
  ctl.review({ jobId: job1, verdict: 'accept', summaryAr: 'مراجعة فعلية للإيصال' });
  const [cont] = conts(ctl);
  ctl.claim({ itemId: cont.id });
  ctl.ack({ itemId: cont.id, note: 'next useful action chosen by the host', dispatch: { mode: 'author', taskFile: 'task-006.md', plannedDir: 'm1-draft-0.21' } });
  const job2 = ctl.state.items[cont.nextJobId];
  assert.equal(cont.status, 'acknowledged');
  assert.deepEqual([job2.kind, job2.status, job2.payload.continuationOf], ['authorJob', 'approved', cont.id]);
  assert.equal(sp.calls.length, 1); assert.match(job2.blockedReason, /مؤلفًا نشطًا/, 'a running ledger state still blocks');
  patchLedger(ws, { activeAuthor: { ...active, state: 'reviewed', brokerJobId: '00000000-0000-4000-8000-0000000000ff' } });
  ctl.heartbeat();
  assert.equal(sp.calls.length, 1); assert.match(job2.blockedReason, /دون مراجعة وسيط مطابقة/, 'a mismatched job ID stays blocked');
  patchLedger(ws, { activeAuthor: { ...active, state: 'reviewed' } });
  ctl.pause({ idempotencyKey: 'seq-pause-001' });
  ctl.heartbeat();
  assert.equal(sp.calls.length, 1); assert.equal(job2.blockedReason, 'متوقف مؤقتًا');
  writeFileSync(ctl.worker.leaseFile, JSON.stringify({ jobId: 'foreign-job', pid: 1 }));
  ctl.resume({ idempotencyKey: 'seq-resume-01' });
  assert.equal(sp.calls.length, 1); assert.equal(job2.blockedReason, 'يوجد عقد عامل قائم');
  unlinkSync(ctl.worker.leaseFile);
  ctl.heartbeat();
  assert.equal(sp.calls.length, 2, 'the second useful author starts now');
  assert.equal(job2.status, 'running');
  assert.ok(sp.calls[1].args.includes('Write(./m1-draft-0.21/**)'));
  assert.equal(sp.calls[1].opts.shell, false);
  assert.equal(ctl.state.reviews.length, 1, 'exactly the one actual review');
  await ctl.notifyIdle();
  assert.equal(contSends(notifier).length, 1);
});

test('ledger: unknown, unreviewed or mismatched inactive states stay blocked', () => {
  const { ws, ctl, sp } = setup();
  const jobId = startSmoke(ctl, 'ldg-job-00001');
  finishCurrent(ctl, sp);
  const base = { task: 'task-006.md', claudeSessionId: SESSION };
  patchLedger(ws, { activeAuthor: { ...base, state: 'reviewed', brokerJobId: jobId } });
  assert.match(ctl.ledgerBlock(), /دون مراجعة وسيط مطابقة/, 'finished but not reviewed');
  patchLedger(ws, { activeAuthor: { ...base, state: 'reviewed' } });
  assert.match(ctl.ledgerBlock(), /دون مراجعة وسيط مطابقة/, 'no broker job named');
  patchLedger(ws, { activeAuthor: { ...base, state: 'weird' } });
  assert.match(ctl.ledgerBlock(), /مؤلفًا نشطًا/);
  ctl.review({ jobId, verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  patchLedger(ws, { activeAuthor: { ...base, state: 'running', brokerJobId: jobId } });
  assert.match(ctl.ledgerBlock(), /مؤلفًا نشطًا/, 'running stays blocked even after the review');
  patchLedger(ws, { activeAuthor: { ...base, state: 'reviewed', brokerJobId: jobId } });
  assert.equal(ctl.ledgerBlock(), null, 'inactive and matched to an actual persisted review');
  patchLedger(ws, { activeAuthor: { ...base, state: 'completed' } });
  assert.equal(ctl.ledgerBlock(), null, 'terminal completed ledgers stay compatible');
});

test('restart at durable boundaries: a pending review is decided once; a half-recorded item is finished, never duplicated', async () => {
  const { ws, c, ctl, sp } = setup();
  startSmoke(ctl, 'crash-job-001');
  const jobId = finishCurrent(ctl, sp);
  ctl._continueAfterReview = () => null;                                   // the process stops right after the review was saved
  ctl.review({ jobId, verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  assert.equal(JSON.parse(readFileSync(ctl.stateFile, 'utf8')).reviews[0].continuation.status, 'pending');
  const n2 = fakeNotifier();
  const ctl2 = controller(ws, c, n2);
  ctl2.recover(); await ctl2.notifyIdle();
  assert.equal(conts(ctl2).length, 1); assert.equal(contSends(n2).length, 1);
  const st = JSON.parse(readFileSync(ctl2.stateFile, 'utf8'));
  st.reviews[0].continuation = { status: 'pending', at: null, reason: null, itemId: null };   // stop between item save and review update
  writeFileSync(ctl2.stateFile, JSON.stringify(st));
  const n3 = fakeNotifier();
  const ctl3 = controller(ws, c, n3);
  ctl3.recover(); await ctl3.notifyIdle();
  assert.equal(conts(ctl3).length, 1, 'the existing item is reused');
  assert.equal(ctl3.state.reviews[0].continuation.status, 'queued');
  assert.equal(n3.sends.length, 0, 'the delivered notification is not repeated');
});

test('legacy reviews saved before the update never trigger a continuation', async () => {
  const { ws, c, ctl, sp } = setup();
  startSmoke(ctl, 'legacy-job-01');
  const jobId = finishCurrent(ctl, sp);
  ctl._continueAfterReview = () => null;
  ctl.review({ jobId, verdict: 'accept', summaryAr: 'مراجعة قديمة' });
  const st = JSON.parse(readFileSync(ctl.stateFile, 'utf8'));
  delete st.reviews[0].continuation;                                       // shape written by the old broker
  writeFileSync(ctl.stateFile, JSON.stringify(st));
  const n2 = fakeNotifier();
  const ctl2 = controller(ws, c, n2);
  ctl2.recover(); await ctl2.notifyIdle();
  assert.equal(conts(ctl2).length, 0);
  assert.equal(n2.sends.length, 0);
  assert.equal(ctl2.state.reviews.length, 1, 'reviews preserved');
});

test('a continuation send interrupted by a restart is uncertain and never retried automatically', async () => {
  const hang = fakeNotifier({ results: [{ ok: true }, { ok: true }, new Promise(() => {})] });
  const { ws, c, ctl, sp } = setup({ notifier: hang });
  startSmoke(ctl, 'hang-job-0001');
  ctl.review({ jobId: finishCurrent(ctl, sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  const [item] = conts(ctl);
  assert.equal(ctl.state.notifications[`continuation:${item.id}`].status, 'sending');
  const n2 = fakeNotifier();
  const ctl2 = controller(ws, c, n2);
  ctl2.recover(); await ctl2.notifyIdle();
  assert.equal(ctl2.state.notifications[`continuation:${item.id}`].status, 'uncertain');
  assert.equal(n2.sends.length, 0);
  assert.equal(conts(ctl2).length, 1);
});

test('continuation ack: bad plans are refused, an ack without a plan starts nothing, a second ack is refused', () => {
  const { ctl, sp } = setup();
  startSmoke(ctl, 'ack-job-00001');
  ctl.review({ jobId: finishCurrent(ctl, sp), verdict: 'accept', summaryAr: 'مراجعة فعلية' });
  const [cont] = conts(ctl);
  assert.throws(() => ctl.ack({ itemId: cont.id, dispatch: { mode: 'author', taskFile: '../secret.md', plannedDir: 'm1-draft-0.30' } }), /ملف المهمة/);
  assert.throws(() => ctl.ack({ itemId: cont.id, dispatch: { mode: 'exec' } }), /mode/);
  assert.equal(cont.status, 'queued');
  ctl.ack({ itemId: cont.id, note: 'no action now' });
  assert.equal(cont.status, 'acknowledged'); assert.equal(cont.nextJobId, undefined);
  assert.throws(() => ctl.ack({ itemId: cont.id }), /لا يمكن تأكيد/);
  assert.equal(sp.calls.length, 1);
});

test('the continuation message is fixed, server-side and points the existing thread at the saved authority', () => {
  const info = { itemId: '0f0e0d0c-0b0a-4908-8706-050403020100', reason: 'continuation', kind: 'continuation', status: 'queued',
    connectionPath: path.resolve('/ws/coordination/ui-control/connection.json') };
  const msg = notificationMessage(info);
  assert.match(msg, /not a review, not an approval/);
  assert.match(msg, /standingAuthorization, continuationWork/);
  assert.match(msg, /Nothing was reviewed or started automatically/);
  assert.match(msg, /Ignore continuation items that are already acknowledged/);
  assert.ok(msg.includes(info.itemId) && msg.includes(info.connectionPath));
  const args = buildNotifyArgs({ threadId: THREAD, message: msg });
  assert.deepEqual(args.slice(0, 6), ['queue', '--remote', 'unix://', '--thread', THREAD, '--message']);
  assert.equal(args.length, 7);
  assert.throws(() => notificationMessage({ ...info, reason: 'autoReview' }), /سبب/);
});

test('authority display is read-only, from safe ledger fields, and separate from owner answers', () => {
  const { ctl } = setup();
  ctl.submitOwnerAnswer({ questionId: 'UI-TEST', choice: 'yes', idempotencyKey: 'auth-owner-01' });
  const a = ctl.view().authority;
  assert.equal(a.standing.autonomousSequentialCycles, true);
  assert.equal(a.standing.personalApproval, false);
  assert.match(a.standing.scope, /M1/);
  assert.deepEqual(a.worklist.items.map((i) => i.id), ['coordinator-improvement', 'publish-final-verify']);
  assert.equal(a.continuation.enabled, true);
  assert.match(a.noteAr, /وليس إجابات من المالك/);
  assert.ok(!JSON.stringify(a).includes('UI-TEST'), 'owner answers are not mixed in');
  assert.equal(ctl.state.ownerAnswers['UI-TEST'].status, 'queued', 'owner answers untouched');
  const off = setup({ enabled: false }).ctl.view().authority;
  assert.equal(off.standing, null); assert.equal(off.continuation.enabled, false);
});
