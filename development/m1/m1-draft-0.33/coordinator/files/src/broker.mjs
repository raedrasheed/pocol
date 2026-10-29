// Persisted control broker for the CURRENT host coordinator. It never reviews, never
// creates agent turns on its own, and never starts a reviewer process. Browser requests
// and the host coordinator's reviewer API go through this one persisted state.
//
// Continuation (0.33 patch): after an ACTUAL review is recorded, and only if the saved ledger
// carries an explicit standing authorization (autonomousSequentialCycles === true,
// noProduction === true, scope naming M1) and a ready worklist with at least one M1 item, the
// broker queues ONE durable 'continuation' item for that review and notifies the SAME existing
// host thread once. It never reviews, never starts an author by itself and never runs a loop:
// the host coordinator claims the item and chooses the next action (or none). Default off.
import { existsSync, mkdirSync, openSync, closeSync, readFileSync, readSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { appendLog, bound, decodeText, isoNow, newId, pidAlive, readJson, sha256, writeJsonAtomic } from './util.mjs';
import { redact, redactDeep } from './redact.mjs';
import { loadHistory, loadCheckResults, parseReceipt } from './history.mjs';
import { findQuestion, OWNER_QUESTIONS } from './owner.mjs';
import { PLANNED_DIR, TASK_FILE, Worker } from './worker.mjs';

const KEY_RE = /^[A-Za-z0-9_-]{8,100}$/;
const TEXT_MAX = 8000, OPEN_ITEMS_MAX = 200, EVENTS_MAX = 500;
const LEASE_MIN = 15_000, LEASE_MAX = 600_000;
const STDERR_TAIL = 4096, STDERR_SHOWN = 2000;
const NOTIFY_RETRYABLE = ['failed', 'uncertain', 'notConfigured'];
const ATTEMPTS_SHOWN = 20, RETRY_KEYS_MAX = 10_000;
// Ledger states that describe an author that is no longer writing. They unblock dispatch ONLY
// when the ledger names a broker job that this broker persisted and actually reviewed.
const INACTIVE_REVIEWED = ['reviewed', 'ended', 'finished'];
const CONT_ID_RE = /^[A-Za-z0-9._-]{1,80}$/, CONT_WORD_RE = /^[A-Za-z]{1,40}$/, CONT_ITEMS_MAX = 20;
const OPEN_STATUSES = ['queued', 'claimed'];

export class BrokerError extends Error {
  constructor(status, message) { super(message); this.status = status; }
}
const fail = (status, msg) => { throw new BrokerError(status, msg); };

/** Launch errors made actionable: ENOENT from spawn means the executable (or cwd) was not found. */
function describeLaunchError(msg) {
  const m = redact(String(msg));
  return /\bENOENT\b/.test(m) ? `لم يُعثر على ملف claude التنفيذي أو مجلد العمل (ENOENT): ${m}` : m;
}

/**
 * Dictionary for maps keyed by browser-supplied strings (idempotency keys). It has no
 * prototype, so valid keys such as '__proto__', 'constructor' or 'toString' are ordinary own
 * entries: they cannot reach Object.prototype, collide with inherited members or bypass dedup.
 * Copying a loaded map keeps every existing entry (JSON.parse already made them own properties).
 */
const dict = (src) => Object.assign(Object.create(null), src && typeof src === 'object' && !Array.isArray(src) ? src : {});
const own = (map, key) => (Object.hasOwn(map, key) ? map[key] : undefined);

function emptyState(now) {
  return { schema: 'pocol-local-coordinator/1', createdAt: isoNow(now), paused: false, pauseRequestedAt: null,
    items: {}, order: [], keys: dict(), reviewer: null, currentJob: null, reviews: [], ownerAnswers: {}, notifications: {}, notifyRetryKeys: dict(), events: [] };
}

/**
 * The continuation policy from the saved ledger. Off unless every condition holds; anything
 * missing, malformed, complete or blocked-without-ready-items gives { enabled: false, reason }.
 */
export function continuationPolicy(ledger) {
  const off = (reason) => ({ enabled: false, reason, items: [], worklistStatus: null });
  if (!ledger || typeof ledger !== 'object' || Array.isArray(ledger)) return off('ledgerUnreadable');
  const a = ledger.standingAuthorization;
  if (!a || typeof a !== 'object' || Array.isArray(a)) return off('noStandingAuthorization');
  if (a.autonomousSequentialCycles !== true) return off('autonomousCyclesOff');
  if (a.noProduction !== true) return off('noProductionNotTrue');
  if (typeof a.scope !== 'string' || !/\bM1\b/.test(a.scope)) return off('scopeNotM1');
  const w = ledger.continuationWork;
  if (!w || typeof w !== 'object' || Array.isArray(w)) return off('noWorklist');
  if (w.status === 'complete') return off('worklistComplete');
  if (!['ready', 'blocked'].includes(w.status)) return off('worklistNotReady');
  if (!Array.isArray(w.remainingIndependentItems)) return off('worklistMalformed');
  const items = [];
  for (const it of w.remainingIndependentItems) {
    if (!it || typeof it !== 'object' || Array.isArray(it) || !CONT_ID_RE.test(it.id || '') || it.scope !== 'M1'
      || (it.kind !== undefined && !CONT_WORD_RE.test(String(it.kind)))) return off('worklistMalformed');
    if (it.status === undefined || it.status === 'ready') items.push({ id: it.id, kind: it.kind ?? null });
    else if (!['blocked', 'done'].includes(it.status)) return off('worklistMalformed');
  }
  if (!items.length) return off('noIndependentReadyItem');
  return { enabled: true, reason: null, items: items.slice(0, CONT_ITEMS_MAX), worklistStatus: w.status };
}

export class Controller {
  /**
   * notifier: optional host transport ({ configured(), describe(), send(info) -> Promise }).
   * holdNotifications: create notification records but send nothing until releaseNotifications()
   * (the server holds them until connection.json, which the message points to, is written).
   */
  constructor({ workspace, uiDir = path.join(workspace, 'coordination', 'ui-control'), sessionId = null, claudeBin = null,
    spawnImpl, isAlive = pidAlive, now = () => Date.now(), watchMs = 2000, notifier = null, holdNotifications = false }) {
    this.ws = workspace; this.uiDir = uiDir; this.now = now; this.isAlive = isAlive; this.watchMs = watchMs;
    mkdirSync(uiDir, { recursive: true });
    this.stateFile = path.join(uiDir, 'state.json');
    this.logFile = path.join(uiDir, 'server.log');
    this.cacheFile = path.join(uiDir, 'history-cache.json');
    this.state = readJson(this.stateFile, null) || emptyState(now());
    this.state.notifications ??= {};
    // Loaded (or new) key maps become null-prototype dictionaries; their entries are kept as they are.
    this.state.keys = dict(this.state.keys);
    this.state.notifyRetryKeys = dict(this.state.notifyRetryKeys);
    for (const n of Object.values(this.state.notifications)) {
      // Migration: attempt counter from the highest attempt seen; per-record key lists into the durable registry.
      n.attemptSeq = Math.max(n.attemptSeq ?? 0, ...(n.attempts || []).map((x) => Number(x.n) || 0));
      for (const k of n.retryKeys || []) this.state.notifyRetryKeys[k] ??= { notificationId: n.id, attempt: null, at: null };
      delete n.retryKeys;
    }
    this.connectionFile = path.join(uiDir, 'connection.json');
    this.notifier = notifier; this.notifyHeld = holdNotifications; this.inflight = new Set();
    this.ledgerFile = path.join(workspace, 'coordination', 'issue-ledger.json');
    this.sessionId = sessionId || readJson(this.ledgerFile, {})?.activeAuthor?.claudeSessionId || null;
    this.worker = new Worker({ ws: workspace, uiDir, claudeBin, spawnImpl, isAlive, log: (r) => this.log(r) });
    this.watchTimer = null; this.stopped = false;
    this.historyAt = 0; this.history = null;
  }

  // ---------- persistence ----------
  save() { writeJsonAtomic(this.stateFile, this.state); }
  log(record) { appendLog(this.logFile, redactDeep(record)); }
  event(kind, detail = {}) {
    this.state.events.push({ at: isoNow(this.now()), kind, ...redactDeep(detail) });
    if (this.state.events.length > EVENTS_MAX) this.state.events.splice(0, this.state.events.length - EVENTS_MAX);
    this.log({ event: kind, ...detail });
  }

  // ---------- single service per ui-control directory ----------
  // The lock names its owner: pid plus a random token for THIS Controller instance. Only the
  // instance holding that token may update or remove it (another Controller in the same
  // process is not the owner). An empty, partial or malformed lock is never taken over: its
  // writer may still be between open('wx') and write. A lock whose owner pid is dead is taken
  // over only under an exclusive recovery mutex, after re-reading it unchanged, and the old
  // content is preserved as evidence. Nothing is deleted recursively.

  _readLockRaw(file) { try { return readFileSync(file, 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : undefined; } }
  _parseLock(raw) {
    if (typeof raw !== 'string') return null;
    try {
      const j = JSON.parse(raw);
      if (!j || typeof j !== 'object' || !Number.isInteger(j.pid) || j.pid <= 0) return null;
      if (j.owner !== undefined && (typeof j.owner !== 'string' || !j.owner)) return null;
      return j;                                                         // a legacy lock without owner still names its pid
    } catch { return null; }
  }
  _lockInfo(port) { return { pid: process.pid, owner: this.serviceOwner, startedAt: this.serviceStartedAt, port, updatedAt: isoNow(this.now()) }; }

  acquireService(port) {
    const lock = path.join(this.uiDir, 'service.lock');
    this.serviceOwner ??= newId();
    this.serviceStartedAt ??= isoNow(this.now());
    if (this.serviceOwned) {
      // Normal second call (the real port is known now): update only our own, still-current lock.
      const cur = this._parseLock(this._readLockRaw(lock));
      if (!cur || cur.owner !== this.serviceOwner) { this.serviceOwned = false; fail(409, 'قفل الخدمة لم يعد مملوكًا لهذه النسخة؛ لن يُكتب فوقه.'); }
      writeJsonAtomic(lock, this._lockInfo(port));
      return;
    }
    let fd = null;
    try { fd = openSync(lock, 'wx', 0o600); }
    catch (e) {
      if (e.code !== 'EEXIST') throw e;
      return this._contendLock(lock, port);
    }
    try { writeFileSync(fd, JSON.stringify(this._lockInfo(port))); }
    catch (e) { try { closeSync(fd); } catch { /* closed */ } fd = null; try { unlinkSync(lock); } catch { /* gone */ } throw e; }
    finally { if (fd !== null) closeSync(fd); }
    this.serviceLock = lock; this.serviceOwned = true;
  }

  _contendLock(lock, port) {
    const raw = this._readLockRaw(lock);
    if (raw === null) fail(409, 'قفل الخدمة تغيّر أثناء الفحص؛ أعد المحاولة.');
    const old = this._parseLock(raw);
    if (!old) {
      this.event('service.lockUnreadable', { length: typeof raw === 'string' ? raw.length : null });
      fail(409, `قفل الخدمة فارغ أو غير صالح (${lock}). قد تكون خدمة أخرى في طور البدء؛ لن يُستولى عليه تلقائيًا. إن تأكدت أنه قديم فاحذف هذا الملف يدويًا.`);
    }
    // Same pid is this very process, so alive by definition: another Controller here owns it.
    if (old.pid === process.pid || this.isAlive(old.pid)) fail(409, `خدمة المنسق تعمل بالفعل (pid ${old.pid}).`);
    this._takeOverStale(lock, raw, old, port);
  }

  /** Stale takeover under an exclusive mutex; the lock must still be exactly what was judged stale. */
  _takeOverStale(lock, rawSeen, old, port) {
    const mutex = `${lock}.recovery`;
    const me = JSON.stringify({ pid: process.pid, owner: this.serviceOwner, at: isoNow(this.now()) });
    let mfd;
    try { mfd = openSync(mutex, 'wx', 0o600); }
    catch (e) {
      if (e.code !== 'EEXIST') throw e;
      const holder = this._parseLock(this._readLockRaw(mutex));
      fail(409, holder && this.isAlive(holder.pid)
        ? `استعادة قفل الخدمة جارية في عملية أخرى (pid ${holder.pid}).`
        : `يوجد ملف استعادة متروك (${mutex}). لن يُستولى عليه تلقائيًا؛ احذفه يدويًا بعد التأكد من عدم وجود خدمة تعمل.`);
    }
    try { writeFileSync(mfd, me); } finally { closeSync(mfd); }
    try {
      const now = this._readLockRaw(lock);
      if (now !== rawSeen) fail(409, 'تغيّر قفل الخدمة أثناء الاستعادة؛ لم يُستولَ عليه.');
      const evidence = path.join(this.uiDir, `service.lock.stale-${isoNow(this.now()).replace(/[:.]/g, '-')}-${newId().slice(0, 8)}.json`);
      writeFileSync(evidence, rawSeen, { flag: 'wx', mode: 0o600 });
      writeJsonAtomic(lock, this._lockInfo(port));
      this.serviceLock = lock; this.serviceOwned = true;
      this.event('service.staleLockRecovered', { previousPid: old.pid, previousOwner: old.owner ?? null, evidence: path.basename(evidence) });
    } finally {
      // Remove the mutex only if it is still ours.
      if (this._readLockRaw(mutex) === me) try { unlinkSync(mutex); } catch { /* gone */ }
    }
  }

  /** Remove the lock only if this instance still owns it; a successor's lock is never touched. */
  releaseService() {
    if (!this.serviceOwned) return;
    this.serviceOwned = false;
    const cur = this._parseLock(this._readLockRaw(this.serviceLock));
    if (cur && cur.owner === this.serviceOwner) { try { unlinkSync(this.serviceLock); } catch { /* gone */ } }
    else this.log({ event: 'service.releaseSkipped', reason: 'lock not owned by this instance' });
  }

  // ---------- restart recovery (never spawns) ----------
  recover() {
    this.recoverNotifications();
    const r = this.worker.recover();
    const jobId = r.lease?.jobId ?? this.state.currentJob;
    const item = jobId ? this.state.items[jobId] : null;
    let interrupted = null;
    if (r.state === 'alive') {
      this.state.currentJob = r.lease.jobId;
      if (item) item.status = 'running';
      this.event('worker.reattached', { jobId: r.lease.jobId, pid: r.lease.pid });
      this.watch();
    } else if (r.state === 'finished') {
      this.finishJob(r.lease.jobId, { code: r.result.is_error ? 1 : 0, recovered: true });
    } else if (r.state === 'interrupted') {
      if (item) { item.status = 'interrupted'; item.endedAt = isoNow(this.now()); this.attachStderr(item); interrupted = item; }
      this.worker.release(r.lease.jobId); this.state.currentJob = null;
      this.event('worker.interrupted', { jobId: r.lease.jobId, pid: r.lease.pid });
    } else if (this.state.currentJob && item && item.status === 'running') {
      item.status = 'interrupted'; item.endedAt = isoNow(this.now()); this.state.currentJob = null; interrupted = item;
      this.event('worker.interrupted', { jobId: item.id, reason: 'no lease file' });
    }
    this.save();
    if (interrupted) this.notifyHost(interrupted.id, 'jobFinished');
    this.recoverContinuations();
  }

  watch() {
    if (this.watchTimer) return;
    this.watchTimer = setInterval(() => {
      const lease = this.worker.readLease();
      if (!lease || (lease.pid && this.isAlive(lease.pid))) return;
      clearInterval(this.watchTimer); this.watchTimer = null;
      const res = this.worker.receiptResult(lease.jobId);
      this.finishJob(lease.jobId, { code: res && !res.is_error ? 0 : 1, recovered: true });
      this.save();
    }, this.watchMs);
    this.watchTimer.unref?.();
  }

  stop() { this.stopped = true; if (this.watchTimer) clearInterval(this.watchTimer); this.watchTimer = null; }

  // ---------- idempotent submissions (browser) ----------
  _existing(key) {
    if (!KEY_RE.test(key || '')) fail(400, 'مفتاح الطلب (idempotencyKey) غير صالح.');
    const id = own(this.state.keys, key);
    return typeof id === 'string' ? own(this.state.items, id) ?? null : null;
  }
  _add(kind, key, payload) {
    const open = this.state.order.filter((id) => !['acknowledged', 'reviewed', 'cancelled'].includes(this.state.items[id].status)).length;
    if (open >= OPEN_ITEMS_MAX) fail(429, 'الطابور ممتلئ؛ انتظر استلام المنسق.');
    const item = { id: newId(), kind, idempotencyKey: key, createdAt: isoNow(this.now()), status: 'queued', payload };
    this.state.items[item.id] = item; this.state.order.push(item.id); this.state.keys[key] = item.id;
    this.event(`${kind}.queued`, { itemId: item.id });
    this.save();
    return item;
  }
  _text(text) {
    if (typeof text !== 'string' || !text.trim()) fail(400, 'النص فارغ.');
    if (text.length > TEXT_MAX) fail(413, 'النص أطول من الحد المسموح.');
    return text;
  }

  // A duplicate key returns before anything is queued or notified; a new item is persisted
  // (by _add) before its one host notification is even recorded.
  submitGuidance({ text, idempotencyKey }) {
    const ex = this._existing(idempotencyKey); if (ex) return { item: ex, duplicate: true };
    const item = this._add('guidance', idempotencyKey, { text: this._text(text) });
    this.notifyHost(item.id, 'queued');
    return { item, duplicate: false };
  }

  submitOwnerAnswer({ questionId, choice, note = '', idempotencyKey }) {
    const ex = this._existing(idempotencyKey); if (ex) return { item: ex, duplicate: true };
    const q = findQuestion(questionId); if (!q) fail(400, 'سؤال غير معروف.');
    if (!q.options.some((o) => o.id === choice)) fail(400, 'خيار غير معروف.');
    if (typeof note !== 'string' || note.length > TEXT_MAX) fail(400, 'ملاحظة غير صالحة.');
    const item = this._add('ownerAnswer', idempotencyKey, { questionId, choice, note, testOnly: Boolean(q.testOnly) });
    this.state.ownerAnswers[questionId] = { itemId: item.id, choice, at: item.createdAt, status: 'queued', testOnly: Boolean(q.testOnly) };
    this.save();
    this.notifyHost(item.id, 'queued');
    return { item, duplicate: false };
  }

  requestAuthorJob({ mode = 'smoke', note = '', idempotencyKey, retryOf = null }) {
    const ex = this._existing(idempotencyKey); if (ex) return { item: ex, duplicate: true };
    if (!['smoke', 'author'].includes(mode)) fail(400, 'نوع المهمة غير معروف.');
    if (retryOf) {
      const prev = this.state.items[retryOf];
      if (!prev || prev.kind !== 'authorJob' || !['failed', 'interrupted'].includes(prev.status)) fail(409, 'لا توجد مهمة فاشلة أو متوقفة بهذا المعرف.');
    }
    const item = this._add('authorJob', idempotencyKey, { mode, note: note ? this._text(note) : '', retryOf });
    this.notifyHost(item.id, 'queued');
    return { item, duplicate: false };
  }

  pause({ idempotencyKey }) {
    if (!KEY_RE.test(idempotencyKey || '')) fail(400, 'مفتاح الطلب غير صالح.');
    if (!this.state.paused) { this.state.paused = true; this.state.pauseRequestedAt = isoNow(this.now()); this.event('pause.requested', { draining: Boolean(this.state.currentJob) }); this.save(); }
    return { pauseState: this.pauseState() };
  }

  resume({ idempotencyKey, reason = '' }) {
    if (!KEY_RE.test(idempotencyKey || '')) fail(400, 'مفتاح الطلب غير صالح.');
    if (this.state.paused) {
      this.state.paused = false; this.state.pauseRequestedAt = null;
      this.event('resume', { reason: bound(String(reason), 500) }); this.save();
      this.tryDispatch();
      // A continuation deferred by the pause is re-evaluated once, for the latest review only.
      const last = this.state.reviews.at(-1);
      if (last && last.continuation?.status === 'deferredPaused') this._continueAfterReview(last);
    }
    return { pauseState: this.pauseState() };
  }

  pauseState() { return this.state.paused ? (this.state.currentJob ? 'draining' : 'paused') : 'active'; }

  // ---------- reviewer (host coordinator) API ----------
  reviewerStatus() {
    const r = this.state.reviewer;
    if (!r) return { status: 'disconnected' };
    const expires = Date.parse(r.heartbeatAt) + r.leaseMs;
    return { status: this.now() <= expires ? 'attached' : 'expired', reviewerId: r.id, heartbeatAt: r.heartbeatAt, expiresAt: isoNow(expires) };
  }
  attach({ reviewerId, leaseMs = 120_000 }) {
    if (typeof reviewerId !== 'string' || !/^[A-Za-z0-9._ -]{1,60}$/.test(reviewerId)) fail(400, 'reviewerId غير صالح.');
    const ms = Math.min(LEASE_MAX, Math.max(LEASE_MIN, Number(leaseMs) || 120_000));
    const at = isoNow(this.now());
    this.state.reviewer = { id: reviewerId, attachedAt: at, heartbeatAt: at, leaseMs: ms };
    this.event('reviewer.attached', { reviewerId }); this.save();
    this.tryDispatch();                                   // only items the coordinator already approved
    return this.reviewerStatus();
  }
  heartbeat() {
    if (!this.state.reviewer) fail(409, 'لا يوجد مراجع متصل.');
    this.state.reviewer.heartbeatAt = isoNow(this.now()); this.save();
    this.tryDispatch();
    return this.reviewerStatus();
  }
  detach() { this.state.reviewer = null; this.event('reviewer.detached'); this.save(); return { status: 'disconnected' }; }

  reviewerQueue() {
    return this.state.order.map((id) => this.state.items[id])
      .filter((i) => ['queued', 'claimed'].includes(i.status) || (i.kind === 'authorJob' && ['completed', 'failed', 'interrupted'].includes(i.status) && !i.reviewId))
      .map((i) => ({ ...i, receipt: i.kind === 'authorJob' && i.status !== 'queued' ? this.jobSummary(i) : undefined }));
  }

  claim({ itemId }) {
    const i = this.state.items[itemId]; if (!i) fail(404, 'عنصر غير موجود.');
    if (i.status !== 'queued') fail(409, `لا يمكن استلام عنصر بحالة ${i.status}.`);
    i.status = 'claimed'; i.claimedAt = isoNow(this.now()); i.claimedBy = this.state.reviewer?.id ?? 'host-coordinator';
    if (i.kind === 'ownerAnswer') this.state.ownerAnswers[i.payload.questionId].status = 'claimed';
    this.event('item.claimed', { itemId }); this.save();
    return i;
  }

  /** The author-mode plan check shared by author-job approval and continuation follow-ups. */
  _validatePlan(mode, dispatch) {
    if (mode === 'author') {
      if (!TASK_FILE.test(dispatch.taskFile || '') || !existsSync(path.join(this.ws, 'coordination', dispatch.taskFile))) fail(400, 'ملف المهمة غير موجود في coordination.');
      if (!PLANNED_DIR.test(dispatch.plannedDir || '')) fail(400, 'المجلد المخطط يجب أن يكون m1-draft-0.N.');
      if (existsSync(path.join(this.ws, dispatch.plannedDir))) fail(409, 'المجلد المخطط موجود بالفعل؛ يجب أن يكون جديدًا.');
    }
    return { mode, sessionId: this.sessionId, taskFile: mode === 'author' ? dispatch.taskFile : null, plannedDir: mode === 'author' ? dispatch.plannedDir : null };
  }

  /**
   * Acknowledge an item. For an author job the coordinator supplies the dispatch plan. For a
   * continuation item the coordinator MAY supply a plan ({ mode, taskFile, plannedDir }) for the
   * next author turn it chose; without one the continuation is just acknowledged (no action).
   */
  ack({ itemId, note = '', dispatch = null }) {
    const i = this.state.items[itemId]; if (!i) fail(404, 'عنصر غير موجود.');
    if (!['queued', 'claimed'].includes(i.status)) fail(409, `لا يمكن تأكيد عنصر بحالة ${i.status}.`);
    if (i.kind === 'authorJob') {
      if (!dispatch || typeof dispatch !== 'object') fail(400, 'خطة التنفيذ مطلوبة لمهمة المؤلف.');
      const plan = this._validatePlan(i.payload.mode, dispatch);
      i.ackNote = bound(String(note), 2000); i.ackAt = isoNow(this.now());
      i.dispatch = plan;
      i.status = 'approved';
      this.event('authorJob.approved', { itemId, mode: plan.mode });
      this.save();
      this.tryDispatch();
      return this.state.items[itemId];
    }
    if (i.kind === 'continuation' && dispatch !== null) {
      if (typeof dispatch !== 'object' || Array.isArray(dispatch) || !['smoke', 'author'].includes(dispatch.mode)) fail(400, 'خطة المتابعة يجب أن تحدد mode بقيمة smoke أو author.');
      const plan = this._validatePlan(dispatch.mode, dispatch);
      i.ackNote = bound(String(note), 2000); i.ackAt = isoNow(this.now());
      const job = this._add('authorJob', `contjob-${i.id}`, { mode: plan.mode, note: '', retryOf: null, continuationOf: i.id });
      job.dispatch = plan; job.status = 'approved'; job.ackAt = i.ackAt; job.ackNote = 'خطة اختارها المنسق المضيف عبر عنصر المتابعة';
      i.status = 'acknowledged'; i.nextJobId = job.id;
      this.event('continuation.nextApproved', { itemId, jobId: job.id, mode: plan.mode });
      this.save();
      this.tryDispatch();
      return i;
    }
    i.ackNote = bound(String(note), 2000); i.ackAt = isoNow(this.now());
    i.status = 'acknowledged';
    if (i.kind === 'ownerAnswer') this.state.ownerAnswers[i.payload.questionId].status = 'acknowledged';
    this.event('item.acknowledged', { itemId, kind: i.kind }); this.save();
    return i;
  }

  /**
   * Record the host coordinator's actual review. Completed, failed and interrupted jobs all
   * need one before another job may start. A failed/interrupted job keeps its status after
   * review so an explicit retry stays possible; nothing is retried automatically. The review is
   * saved with continuation 'pending' first, so a restart can finish the continuation decision.
   */
  review({ jobId, verdict, summaryAr, text = '' }) {
    const i = this.state.items[jobId]; if (!i || i.kind !== 'authorJob') fail(404, 'مهمة غير موجودة.');
    if (i.reviewId) fail(409, 'هذه المهمة رُوجعت بالفعل.');
    if (!['completed', 'failed', 'interrupted'].includes(i.status)) fail(409, 'لا يمكن مراجعة مهمة لم تنته.');
    if (!['accept', 'revise', 'reject'].includes(verdict)) fail(400, 'حكم المراجعة غير صالح.');
    const review = { id: newId(), jobId, verdict, summaryAr: bound(this._text(summaryAr), 2000), text: bound(String(text), 20000),
      at: isoNow(this.now()), reviewerId: this.state.reviewer?.id ?? 'host-coordinator', jobStatus: i.status,
      continuation: { status: 'pending', at: null, reason: null, itemId: null } };
    this.state.reviews.push(review); i.reviewId = review.id; i.reviewedAt = review.at;
    if (i.status === 'completed') i.status = 'reviewed';
    this.event('review.recorded', { jobId, verdict }); this.save();
    this.tryDispatch();
    this._continueAfterReview(review);
    return review;
  }

  // ---------- continuation after an actual review ----------
  continuationPolicy() { return continuationPolicy(readJson(this.ledgerFile, null)); }

  /** Why no continuation may be queued now (null: allowed). Order: stop, pause, policy, writer, unreviewed, open item. */
  _continuationGate(policy) {
    if (this.stopped) return 'stopped';
    if (this.state.paused) return 'paused';
    if (!policy.enabled) return policy.reason;
    if (this.state.currentJob || this.worker.readLease()) return 'writerActive';
    if (this.unreviewedJob()) return 'unreviewedReceipt';
    if (this.state.order.some((id) => this.state.items[id].kind === 'continuation' && OPEN_STATUSES.includes(this.state.items[id].status))) return 'openContinuation';
    return null;
  }

  _continueAfterReview(review) {
    // An item already persisted for THIS review (a stop between saving the item and the review) is
    // only finished, never duplicated; its notification record deduplicates as well.
    if (this._existing(`cont-${review.id}`)) return this._enqueueContinuation(review, null);
    const policy = this.continuationPolicy();
    const gate = this._continuationGate(policy);
    const at = isoNow(this.now());
    if (gate) {
      review.continuation = { status: gate === 'paused' ? 'deferredPaused' : 'suppressed', at, reason: gate, itemId: null };
      this.event('continuation.suppressed', { reviewId: review.id, reason: gate });
      this.save();
      return null;
    }
    return this._enqueueContinuation(review, policy);
  }

  /** One item per review: the idempotency key is derived from the review ID, so a restart cannot add a second. */
  _enqueueContinuation(review, policy) {
    const key = `cont-${review.id}`;
    let item = this._existing(key);
    if (!item) item = this._add('continuation', key, { reviewId: review.id, jobId: review.jobId, verdict: review.verdict, readyItems: policy.items, worklistStatus: policy.worklistStatus });
    review.continuation = { status: 'queued', at: isoNow(this.now()), reason: null, itemId: item.id };
    this.event('continuation.queued', { reviewId: review.id, itemId: item.id });
    this.save();
    this.notifyHost(item.id, 'continuation');
    return item;
  }

  /** After a restart: a review saved as 'pending' (crash before the decision) is decided once now. Legacy reviews have no field and are left alone. */
  recoverContinuations() {
    for (const rv of this.state.reviews) if (rv.continuation?.status === 'pending') this._continueAfterReview(rv);
  }

  // ---------- dispatch: one author process, only for approved items ----------
  ledgerBlock() {
    const l = readJson(this.ledgerFile, null);
    if (!l) return 'تعذّرت قراءة سجل المنسق (issue-ledger.json).';
    const a = l.activeAuthor;
    if (!a || !a.state || a.state === 'completed') return null;
    if (INACTIVE_REVIEWED.includes(a.state)) {
      // An ended author is no longer blocking only if the ledger names a broker job that this broker
      // persisted AND actually reviewed. Unknown, mismatched or unreviewed jobs stay blocked.
      const j = typeof a.brokerJobId === 'string' ? own(this.state.items, a.brokerJobId) : undefined;
      const reviewed = j && j.kind === 'authorJob' && j.reviewId && ['reviewed', 'failed', 'interrupted'].includes(j.status)
        && this.state.reviews.some((r) => r.id === j.reviewId && r.jobId === j.id);
      if (reviewed) return null;
      return `السجل يُظهر مؤلفًا منتهيًا (${a.task ?? '?'}: ${a.state}) دون مراجعة وسيط مطابقة محفوظة للمهمة ${a.brokerJobId ?? '?'}.`;
    }
    return `السجل يُظهر مؤلفًا نشطًا أو غير مُثبَّت (${a.task ?? '?'}: ${a.state}).`;
  }

  /** First finished author job (completed, failed or interrupted) without an actual review. */
  unreviewedJob() {
    return this.state.order.map((id) => this.state.items[id])
      .find((i) => i.kind === 'authorJob' && ['completed', 'failed', 'interrupted'].includes(i.status) && !i.reviewId) ?? null;
  }

  dispatchBlock() {
    if (this.state.paused) return 'متوقف مؤقتًا';
    if (this.state.currentJob) return 'مهمة مؤلف قيد التشغيل';
    if (this.worker.readLease()) return 'يوجد عقد عامل قائم';
    const awaiting = this.unreviewedJob();
    if (awaiting) return `بانتظار مراجعة Codex الفعلية للمهمة السابقة (${awaiting.id}: ${awaiting.status})`;
    if (this.reviewerStatus().status !== 'attached') return 'المنسق المضيف غير متصل أو انتهت مهلة اتصاله؛ لا تبدأ مهمة دون مراجع حاضر';
    if (!this.sessionId) return 'معرف جلسة Claude غير معروف';
    if (!this.worker.claudeBin) return 'claude.exe غير متاح لهذا الخادم (استخدم --claude-bin)';
    return this.ledgerBlock();
  }

  tryDispatch() {
    const next = this.state.order.map((id) => this.state.items[id]).find((i) => i.kind === 'authorJob' && i.status === 'approved');
    if (!next) return null;
    const block = this.dispatchBlock();
    if (block) { next.blockedReason = block; this.save(); return null; }
    if (next.dispatch.plannedDir && existsSync(path.join(this.ws, next.dispatch.plannedDir))) {
      next.status = 'failed'; next.endedAt = isoNow(this.now()); next.error = 'المجلد المخطط أصبح موجودًا قبل البدء.'; this.save();
      this.notifyHost(next.id, 'jobFinished');
      return null;
    }
    next.dispatch.sessionId = this.sessionId;
    let lease;
    try { lease = this.worker.start(next, (r) => { this.finishJob(next.id, r); this.save(); }); }
    catch (e) {
      next.status = 'failed'; next.endedAt = isoNow(this.now()); next.error = describeLaunchError(e.message);
      this.event('worker.startFailed', { itemId: next.id }); this.save();
      this.notifyHost(next.id, 'jobFinished');
      return null;
    }
    next.status = 'running'; next.startedAt = lease.startedAt; next.pid = lease.pid; next.blockedReason = null;
    this.state.currentJob = next.id;
    this.event('worker.started', { itemId: next.id, pid: lease.pid, mode: next.dispatch.mode });
    this.save();
    return next;
  }

  finishJob(jobId, { code = null, error = null, recovered = false } = {}) {
    const i = this.state.items[jobId];
    const res = this.worker.receiptResult(jobId);
    let ended = false;
    if (i && i.status === 'running' || i && recovered) {
      i.status = res && !res.is_error && (code === 0 || recovered) ? 'completed' : 'failed';
      i.endedAt = isoNow(this.now()); i.exitCode = code; if (error) i.error = describeLaunchError(error);
      if (i.status === 'failed') this.attachStderr(i);
      if (!res) i.error = i.error || (i.stderrExcerpt ? `لم يكتب Claude نتيجة نهائية في الإيصال. آخر stderr: ${i.stderrExcerpt}` : 'لم يكتب Claude نتيجة نهائية في الإيصال، ولا يوجد stderr.');
      ended = true;
    }
    this.worker.release(jobId);
    if (this.state.currentJob === jobId) this.state.currentJob = null;
    this.event('worker.finished', { jobId, status: i?.status, exitCode: code });
    // One review request per job: repeated exit/error/watch callbacks never enqueue twice.
    if (ended) this.notifyHost(jobId, 'jobFinished');
  }

  /**
   * Bounded, redacted tail of a job's private stderr.log, kept on the item so a failed CLI
   * invocation (e.g. a session that no longer exists) shows an actionable error. The raw file
   * itself is never served.
   */
  stderrExcerpt(jobId) {
    const f = path.join(this.uiDir, 'jobs', jobId, 'stderr.log');
    let fd = null;
    try {
      const size = statSync(f).size;
      if (!size) return null;
      const len = Math.min(size, STDERR_TAIL);
      const buf = Buffer.alloc(len);
      fd = openSync(f, 'r');
      readSync(fd, buf, 0, len, size - len);
      let text = decodeText(buf);
      if (size > len) text = text.slice(text.indexOf('\n') + 1);        // drop a partial first line (could be half a secret)
      text = redact(text).trim();                                        // redact before cutting, keep the tail (the actual error)
      if (!text) return null;
      return text.length > STDERR_SHOWN ? `… [قُصّ أوله] ${text.slice(-STDERR_SHOWN)}` : text;
    } catch { return null; }
    finally { if (fd !== null) try { closeSync(fd); } catch { /* closed */ } }
  }
  attachStderr(item) { const ex = this.stderrExcerpt(item.id); if (ex) item.stderrExcerpt = ex; }

  jobSummary(item) {
    const f = path.join(this.uiDir, 'jobs', item.id, 'receipt.jsonl');
    if (!existsSync(f)) return item.stderrExcerpt ? { result: null, toolCalls: 0, entries: [], lastAt: null, stderrExcerpt: item.stderrExcerpt } : null;
    const p = parseReceipt(readFileSync(f).toString('utf8'));
    return { result: p.result, toolCalls: p.toolCalls, entries: p.entries.slice(-60), lastAt: p.lastAt, stderrExcerpt: item.stderrExcerpt ?? null };
  }

  // ---------- host notification transport (separate from reviewer presence, claim, ack, review) ----------
  /**
   * Record and send at most one notification per (reason, item). The record — and its attempt
   * marked 'sending' — is persisted before any process is launched, so a restart can only find
   * an attempt as 'uncertain', never replay it.
   */
  notifyHost(itemId, reason) {
    const id = `${reason}:${itemId}`;
    if (this.state.notifications[id]) return this.state.notifications[id];
    const n = { id, itemId, reason, createdAt: isoNow(this.now()), status: 'held', attempts: [], error: null, deliveredAt: null, queueId: null, output: null };
    this.state.notifications[id] = n;
    if (this.notifyHeld) { this.event('notify.held', { notificationId: id }); this.save(); return n; }
    this._sendNotification(n, 'auto');
    return n;
  }

  /** Send notifications created while held (server start, before connection.json existed). */
  releaseNotifications() {
    this.notifyHeld = false;
    for (const n of Object.values(this.state.notifications)) if (n.status === 'held') this._sendNotification(n, 'auto');
  }

  _sendNotification(n, trigger) {
    const cfg = this.notifier ? this.notifier.configured() : { ok: false, reason: 'لم يُضبط ناقل الإشعار (--codex-bin و--codex-thread)؛ الطلب محفوظ ولم يُرسل إشعار.' };
    // Attempt numbers come from a persistent counter, never from the (trimmed) display history,
    // so they are unique and increasing for the life of the notification.
    n.attemptSeq = Math.max(n.attemptSeq ?? 0, ...n.attempts.map((x) => Number(x.n) || 0)) + 1;
    const attempt = { n: n.attemptSeq, trigger, startedAt: isoNow(this.now()), status: 'sending', endedAt: null, error: null };
    n.attempts.push(attempt);
    if (n.attempts.length > ATTEMPTS_SHOWN) n.attempts.splice(0, n.attempts.length - ATTEMPTS_SHOWN);
    if (!cfg.ok) {
      attempt.status = n.status = 'notConfigured'; attempt.endedAt = attempt.startedAt; attempt.error = n.error = redact(String(cfg.reason));
      this.event('notify.notConfigured', { notificationId: n.id }); this.save();
      return null;
    }
    n.status = 'sending'; n.error = null;
    this.event('notify.sending', { notificationId: n.id, attempt: attempt.n, trigger });
    this.save();                                                        // durable BEFORE the launch
    const item = this.state.items[n.itemId];
    const info = { itemId: n.itemId, reason: n.reason, kind: item?.kind ?? null, status: item?.status ?? null, connectionPath: this.connectionFile };
    let p;
    try { p = Promise.resolve(this.notifier.send(info)); } catch (e) { p = Promise.resolve({ ok: false, error: String(e?.message || e) }); }
    const done = p.then((r) => this._notifyResult(n.id, attempt.n, r), (e) => this._notifyResult(n.id, attempt.n, { ok: false, error: String(e?.message || e) }))
      .finally(() => this.inflight.delete(done));
    this.inflight.add(done);
    return done;
  }

  _notifyResult(id, attemptNo, r = {}) {
    const n = this.state.notifications[id];
    const matches = n ? n.attempts.filter((x) => x.n === attemptNo) : [];
    const a = matches.length === 1 ? matches[0] : null;                // unique attempt, or nothing is changed
    if (!a || a.status !== 'sending') return;                          // completes once
    a.endedAt = isoNow(this.now());
    const output = r.output ? redact(bound(String(r.output), 2000)) : null;
    const stderr = r.stderr ? redact(bound(String(r.stderr), 2000)) : null;
    if (r.ok) {
      a.status = n.status = 'delivered';
      n.deliveredAt = a.endedAt; n.queueId = r.queueId ? redact(String(r.queueId)) : null; n.output = output; n.error = null;
      a.queueId = n.queueId;
    } else {
      a.status = n.status = r.uncertain ? 'uncertain' : 'failed';
      a.error = n.error = redact(bound(`${r.error || 'فشل غير معروف'}${stderr ? ' · stderr: ' + stderr : ''}`, 3000));
      n.output = output;
    }
    this.event(`notify.${n.status}`, { notificationId: id, attempt: attemptNo });
    this.save();
  }

  /**
   * Explicit retry from the browser for the same item; never automatic. Every accepted retry key
   * is kept in a durable registry that is never trimmed, so an old key can never trigger another
   * attempt. The registry has an explicit hard limit instead of silently forgetting keys.
   */
  retryNotification({ notificationId, idempotencyKey }) {
    if (!KEY_RE.test(idempotencyKey || '')) fail(400, 'مفتاح الطلب غير صالح.');
    const prior = own(this.state.notifyRetryKeys, idempotencyKey);
    if (prior) return { notification: own(this.state.notifications, prior.notificationId) ?? null, duplicate: true };
    const n = typeof notificationId === 'string' ? own(this.state.notifications, notificationId) ?? null : null;
    if (!n) fail(404, 'إشعار غير موجود.');
    if (!NOTIFY_RETRYABLE.includes(n.status)) fail(409, n.status === 'delivered' ? 'سُلّم الإشعار بالفعل.' : `لا يمكن إعادة إشعار بحالة ${n.status}.`);
    if (Object.keys(this.state.notifyRetryKeys).length >= RETRY_KEYS_MAX) {
      fail(507, `بلغ سجل مفاتيح إعادة الإشعار حده الصريح (${RETRY_KEYS_MAX}). لن تُنسى مفاتيح قديمة؛ أرشف ui-control/state.json يدويًا قبل أي إعادة جديدة.`);
    }
    this.state.notifyRetryKeys[idempotencyKey] = { notificationId: n.id, attempt: (n.attemptSeq ?? 0) + 1, at: isoNow(this.now()) };
    this._sendNotification(n, 'explicitRetry');                        // persists the key together with the attempt
    return { notification: n, duplicate: false };
  }

  /** After a restart: an attempt that was 'sending' is uncertain, never resent automatically. */
  recoverNotifications() {
    let changed = false;
    for (const n of Object.values(this.state.notifications)) {
      if (n.status !== 'sending') continue;
      const a = n.attempts.at(-1);
      if (a && a.status === 'sending') { a.status = 'uncertain'; a.endedAt = isoNow(this.now()); a.error = 'توقف الخادم أثناء الإرسال.'; }
      n.status = 'uncertain'; n.error = 'توقف الخادم أثناء الإرسال؛ لا يُعرف هل وصل الإشعار. لا إعادة تلقائية؛ أعد الإرسال صراحة إن لزم (قد يتكرر الإشعار).';
      this.event('notify.uncertainAfterRestart', { notificationId: n.id });
      changed = true;
    }
    if (changed) this.save();
  }

  /** Resolves when no notification attempt is in flight (tests, shutdown). */
  async notifyIdle() { while (this.inflight.size) await Promise.allSettled([...this.inflight]); }

  notifierStatus() {
    const d = this.notifier?.describe?.() ?? null;
    const cfg = this.notifier ? this.notifier.configured() : { ok: false, reason: 'لم يُضبط ناقل الإشعار.' };
    return { configured: cfg.ok, reason: cfg.ok ? null : cfg.reason, remote: d?.remote ?? null, threadSuffix: d?.threadId ? String(d.threadId).slice(-8) : null, held: this.notifyHeld };
  }

  // ---------- read models ----------
  historyCards() {
    if (!this.history || this.now() - this.historyAt > 30_000) {
      const cache = readJson(this.cacheFile, {});
      const h = loadHistory(this.ws, cache);
      writeJsonAtomic(this.cacheFile, h.cache);
      this.history = h; this.historyAt = this.now();
    }
    return this.history.cards;
  }

  /**
   * Cards for what happened through this broker: one Claude card per author job that was
   * started or failed to start (from its own receipt), and one Codex card per review the
   * host coordinator actually recorded. Times are the broker's own recorded times.
   */
  currentCards() {
    const rel = (f) => path.relative(this.ws, f).split(path.sep).join('/');
    const cards = [];
    for (const i of this.state.order.map((id) => this.state.items[id])) {
      if (i.kind !== 'authorJob' || !(i.startedAt || i.endedAt)) continue;
      const f = path.join(this.uiDir, 'jobs', i.id, 'receipt.jsonl');
      const raw = existsSync(f) ? readFileSync(f) : null;
      const p = raw ? parseReceipt(raw.toString('utf8')) : null;
      const r = p?.result ?? null;
      const mode = i.dispatch?.mode ?? i.payload.mode;
      const outcome = i.status === 'running' ? 'قيد التشغيل' : !r ? 'لا توجد نتيجة نهائية في الإيصال' : r.isError ? 'انتهى بخطأ' : 'اكتمل';
      cards.push({
        id: `job-${i.id}`, role: 'author', actor: 'Claude', kind: 'currentReceipt', current: true, jobId: i.id, status: i.status,
        task: i.dispatch?.taskFile ?? (mode === 'smoke' ? 'smoke (قراءة فقط)' : null), revision: i.dispatch?.plannedDir?.replace('m1-draft-', '') ?? null,
        time: i.endedAt ?? i.startedAt ?? null,
        timeNote: i.endedAt ? 'وقت انتهاء المهمة كما سجله الوسيط' : 'وقت بدء المهمة كما سجله الوسيط؛ لم تنته بعد',
        sha256: raw ? sha256(raw) : null, commit: null, source: raw ? rel(f) : `${rel(f)} (غير موجود)`,
        sessionId: r?.sessionId ?? i.dispatch?.sessionId ?? null,
        summaryAr: `دورة المؤلف الحالية (${mode}) ${i.id}: ${outcome} · ${p ? p.toolCalls : 0} استدعاء أداة · رفض أذونات: ${r ? r.permissionDenials.length : 'غير معروف'}${i.error ? ' · خطأ: ' + i.error : ''}`,
        result: r, technical: p?.entries ?? [], truncatedEntries: p?.truncatedEntries ?? 0,
      });
    }
    for (const rv of this.state.reviews) {
      const job = this.state.items[rv.jobId];
      cards.push({
        id: `review-${rv.id}`, role: 'reviewer', actor: 'Codex', kind: 'currentReview', current: true, jobId: rv.jobId, verdict: rv.verdict,
        reviewerId: rv.reviewerId, task: job?.dispatch?.taskFile ?? null, revision: job?.dispatch?.plannedDir?.replace('m1-draft-', '') ?? null,
        time: rv.at ?? null, timeNote: rv.at ? 'وقت تسجيل المراجعة عبر واجهة المراجع' : 'غير مسجل',
        sha256: rv.text ? sha256(rv.text) : null, commit: null, source: `${rel(this.stateFile)}#reviews/${rv.id}`,
        summaryAr: `مراجعة Codex الفعلية (${rv.reviewerId}) للمهمة ${rv.jobId}${rv.jobStatus ? ' [' + rv.jobStatus + ']' : ''}: ${rv.verdict} · ${rv.summaryAr}`,
        technical: [{ kind: 'text', text: rv.text || '(لم يُرسل نص تقني مع المراجعة)' }],
      });
    }
    return redactDeep(cards);
  }

  /**
   * Historical and current cards in one chronological list. A card without a recorded time
   * keeps "unavailable" as its time and is ordered right after its preceding neighbour in
   * the source sequence (never given an invented time). Stable for equal times.
   */
  timelineCards() {
    let key = -Infinity;
    const keyed = [...this.historyCards(), ...this.currentCards()].map((c, n) => {
      const t = c.time ? Date.parse(c.time) : NaN;
      if (Number.isFinite(t)) key = t;
      return { c, key, n };
    });
    keyed.sort((a, b) => (a.key === b.key ? 0 : a.key - b.key) || a.n - b.n);
    return keyed.map((k) => k.c);
  }

  /** Changes whenever a current card would change; the page refetches the timeline then. */
  timelineSig() {
    const jobs = this.state.order.map((id) => this.state.items[id]).filter((i) => i.kind === 'authorJob' && (i.startedAt || i.endedAt))
      .map((i) => `${i.id}:${i.status}:${i.endedAt ?? ''}`);
    return sha256(jobs.concat(this.state.reviews.map((r) => r.id)).join('|')).slice(0, 16);
  }

  statusPanel() {
    const ledger = readJson(this.ledgerFile, null);
    const checks = loadCheckResults(this.ws);
    const latest = checks.find((c) => c.revision === (ledger?.latestCompletedRevision || '').replace('m1-draft-', '')) || checks[0] || null;
    const running = this.state.currentJob ? this.state.items[this.state.currentJob] : null;
    return redactDeep({
      phaseAr: 'M1: مرحلة المواصفات فقط (لا تنفيذ إنتاجي، لا نشر، لا معاملات حقيقية)',
      updatedAt: isoNow(this.now()),
      ledgerStatus: ledger?.status ?? null,
      latestRevision: ledger?.latestCompletedRevision ?? null,
      latestCheck: latest,
      activeWorker: running ? { itemId: running.id, mode: running.dispatch?.mode, pid: running.pid ?? null, startedAt: running.startedAt ?? null } : null,
      ledgerActiveAuthor: ledger?.activeAuthor ?? null,
      openFindings: (ledger?.issues || []).filter((x) => !/^closed|^resolved|^documented/.test(String(x.status))).map((x) => ({ id: x.id, status: x.status, decision: x.decision })),
      noteAr: 'اتفاق الوكيلين ليس اختبارًا. النتائج أعلاه من ملفات فحص محفوظة فعلًا؛ ما لا يوجد له ملف يظهر «غير متاح».',
    });
  }

  ownerView() {
    return OWNER_QUESTIONS.map((q) => ({ ...q, answer: this.state.ownerAnswers[q.id] ?? null }));
  }

  /**
   * Read-only display of the standing delegation and continuation progress from SAFE ledger
   * fields only (booleans, bounded scope text, worklist IDs/kinds/status). It is shown apart from
   * owner questions and never turns guidance or delegation into an owner answer or approval.
   */
  authorityView() {
    const l = readJson(this.ledgerFile, null);
    const a = l && typeof l === 'object' ? l.standingAuthorization : null;
    const w = l && typeof l === 'object' ? l.continuationWork : null;
    const word = (v, n) => (typeof v === 'string' ? bound(v, n) : null);
    const policy = continuationPolicy(l);
    const conts = this.state.order.map((id) => this.state.items[id]).filter((i) => i.kind === 'continuation').slice(-10)
      .map((i) => ({ id: i.id, status: i.status, reviewId: i.payload?.reviewId ?? null, nextJobId: i.nextJobId ?? null, createdAt: i.createdAt }));
    return redactDeep({
      standing: a && typeof a === 'object' && !Array.isArray(a) ? {
        scope: word(a.scope, 300), at: word(a.at, 40),
        autonomousSequentialCycles: a.autonomousSequentialCycles === true, delegatedTechnicalDecisions: a.delegatedTechnicalDecisions === true,
        personalApproval: a.personalApproval === true, noProduction: a.noProduction === true,
      } : null,
      worklist: w && typeof w === 'object' && !Array.isArray(w) ? {
        status: word(w.status, 40),
        items: Array.isArray(w.remainingIndependentItems) ? w.remainingIndependentItems.slice(0, CONT_ITEMS_MAX)
          .map((it) => ({ id: word(it?.id, 80), kind: word(it?.kind, 40), scope: word(it?.scope, 20), status: word(it?.status, 20) })) : [],
      } : null,
      continuation: { enabled: policy.enabled, reason: policy.reason, gate: this._continuationGate(policy), recent: conts,
        lastReview: this.state.reviews.at(-1)?.continuation ?? null },
      latestCompletedRevision: word(l?.latestCompletedRevision, 40),
      noteAr: 'تفويض دائم مسجل في سجل المنسق، وليس إجابات من المالك ولا موافقة شخصية. إجابات أسئلة المالك تُعرض منفصلة ولا تتغير.',
    });
  }

  waitingReason() {
    const items = this.state.order.map((id) => this.state.items[id]);
    if (this.state.currentJob) return 'workerRunning';
    const unreviewed = this.unreviewedJob();
    if (unreviewed) return unreviewed.status === 'completed' ? 'waitingReview' : 'workerFailed';
    const pending = items.some((i) => ['queued', 'claimed', 'approved'].includes(i.status));
    if (pending && this.reviewerStatus().status !== 'attached') return 'waitingReviewerAttach';
    if (items.some((i) => i.status === 'queued' || i.status === 'claimed')) return 'waitingCoordinator';
    if (items.some((i) => i.kind === 'authorJob' && i.status === 'approved')) return 'dispatchBlocked';
    const last = items.filter((i) => i.kind === 'authorJob').at(-1);
    if (last && ['failed', 'interrupted'].includes(last.status)) return 'retryAvailable';
    return 'idle';
  }

  /** Everything the browser may see. No tokens, no reviewer secret, redacted text. */
  view() {
    const items = this.state.order.map((id) => this.state.items[id]).slice(-100)
      .map((i) => ({ ...i, receipt: i.kind === 'authorJob' && i.status !== 'queued' && i.status !== 'approved' ? this.jobSummary(i) : undefined }));
    return redactDeep({
      status: this.statusPanel(), pauseState: this.pauseState(), reviewer: this.reviewerStatus(), waiting: this.waitingReason(),
      items, reviews: this.state.reviews.slice(-50), timelineSig: this.timelineSig(), owner: this.ownerView(), events: this.state.events.slice(-40),
      notifier: this.notifierStatus(),
      notifications: Object.values(this.state.notifications).slice(-200),
      authority: this.authorityView(),
    });
  }
}
