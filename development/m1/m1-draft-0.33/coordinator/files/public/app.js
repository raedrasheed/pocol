// Browser side. All rendering uses textContent / DOM nodes; no innerHTML, no Markdown rendering.
const $ = (id) => document.getElementById(id);
const token = location.hash.slice(1) || sessionStorage.getItem('pocol-coordinator-token') || '';
if (location.hash) { sessionStorage.setItem('pocol-coordinator-token', token); history.replaceState(null, '', '/'); }

// One idempotency key per pending action; reused until the server confirms, so a double
// click or a retry after a lost response cannot create a second request.
const pendingKeys = JSON.parse(sessionStorage.getItem('pocol-pending-keys') || '{}');
function keyFor(action) {
  if (!pendingKeys[action]) { pendingKeys[action] = crypto.randomUUID(); sessionStorage.setItem('pocol-pending-keys', JSON.stringify(pendingKeys)); }
  return pendingKeys[action];
}
function clearKey(action) { delete pendingKeys[action]; sessionStorage.setItem('pocol-pending-keys', JSON.stringify(pendingKeys)); }

async function api(url, data) {
  const res = await fetch(url, { method: data === undefined ? 'GET' : 'POST', headers: { 'X-PoCol-Control': token, 'Content-Type': 'application/json' }, body: data === undefined ? undefined : JSON.stringify(data) });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) { const e = new Error(body.error || `HTTP ${res.status}`); e.http = true; throw e; }
  return body;
}

function el(tag, text, cls) { const n = document.createElement(tag); if (text !== undefined && text !== null) n.textContent = String(text); if (cls) n.className = cls; if (cls === 'tech') n.dir = 'auto'; return n; }
function showError(t) { $('error').textContent = t || ''; $('error').hidden = !t; }
function setConnection(ok, text) { $('connection').textContent = text; $('connection').classList.toggle('lost', !ok); }
const na = (v) => (v === null || v === undefined || v === '' ? 'Unavailable' : String(v));

// Prose and technical fragments use plain text nodes. Each time, path, ID, hash, command
// or count has its own <bdi dir="ltr"> so mixed-direction user content cannot reorder it.
// Legacy *Ar API fields are retained for compatibility; built-in display text is English.
const L = (v) => ({ ltr: na(v) });
function ltr(text) { const b = document.createElement('bdi'); b.dir = 'ltr'; b.textContent = String(text); return b; }
function mixed(node, parts) {
  node.replaceChildren(...parts.filter((p) => p !== null && p !== undefined && p !== false && p !== '')
    .map((p) => (typeof p === 'object' ? ltr(p.ltr) : document.createTextNode(String(p)))));
  return node;
}
function elm(tag, parts, cls) { const n = document.createElement(tag); if (cls) n.className = cls; return mixed(n, parts); }
/** English-only technical line: one LTR element. */
function elLtr(tag, text, cls) { const n = el(tag, text, cls); n.dir = 'ltr'; return n; }
/** Segments (each an array of parts) joined by " · ". */
const joined = (segments) => segments.filter(Boolean).flatMap((s, k) => (k ? [' · ', ...s] : s));

const STATE_EN = { queued: 'Queued', claimed: 'Claimed by coordinator', acknowledged: 'Acknowledged by coordinator', approved: 'Approved by coordinator; awaiting execution', running: 'Running',
  completed: 'Completed; awaiting Codex review', reviewed: 'Reviewed', failed: 'Failed', interrupted: 'Interrupted', cancelled: 'Cancelled' };
const KIND_EN = { guidance: 'Guidance', ownerAnswer: 'Owner-question answer', authorJob: 'Author cycle', continuation: 'Continuation request after an actual review' };
const WAIT_EN = { idle: 'Nothing pending', workerRunning: 'Author task running',
  workerFailed: 'Last author task failed or was interrupted; awaiting its actual Codex review, then an explicit retry if requested',
  waitingReview: 'Awaiting actual Codex review; no further cycle starts before it', waitingCoordinator: 'Waiting for the coordinator to claim requests',
  waitingReviewerAttach: 'Host coordinator disconnected (current Codex turn inactive); requests are saved and await attachment',
  dispatchBlocked: 'Approved task blocked; see the reason in the queue', retryAvailable: 'Failed task reviewed; an explicit retry can be requested' };
const PAUSE_EN = { active: 'Active', draining: 'Pause requested; waiting for the running task to finish at a safe boundary', paused: 'Paused' };
// Notification delivery is a transport state only; it is never shown as a claim, an ack or a review.
const NOTIFY_EN = { held: 'Held until server startup completes', sending: 'Sending to the existing Codex thread',
  delivered: 'Delivered to the existing Codex thread queue (not acknowledgment or review)', failed: 'Sending failed; the request itself is saved',
  uncertain: 'Uncertain: server stopped or timed out while sending; retrying may duplicate the notification', notConfigured: 'Notifier not configured; request saved, no notification sent' };
const NOTIFY_REASON_EN = { queued: 'New request', jobFinished: 'Review request for a finished task', continuation: 'Continuation request after an actual review' };
const REVIEWER_EN = { attached: 'Attached', expired: 'Attachment expired; no review currently running', disconnected: 'Disconnected' };
// Why no continuation request is (or would be) queued. Display only; nothing here changes policy.
const CONT_EN = { ledgerUnreadable: 'Could not read the coordinator ledger', noStandingAuthorization: 'No standing authorization recorded',
  autonomousCyclesOff: 'Sequential cycles are not enabled in the authorization', noProductionNotTrue: 'The “no production” condition is not explicitly recorded',
  scopeNotM1: 'Authorization scope does not name M1', noWorklist: 'No saved independent worklist', worklistComplete: 'Worklist complete; no continuation',
  worklistNotReady: 'Worklist not ready', worklistMalformed: 'Invalid worklist; no continuation', noIndependentReadyItem: 'No independent ready item',
  stopped: 'Server stopping', paused: 'Paused; continuation deferred until resume', writerActive: 'An author or worker lease is active',
  unreviewedReceipt: 'An unreviewed receipt exists', openContinuation: 'A continuation request is already open' };
const CONT_STATUS_EN = { pending: 'Pending decision', queued: 'One continuation request queued', suppressed: 'Not queued', deferredPaused: 'Deferred due to pause' };
const yesNo = (b) => (b ? 'Yes' : 'No');

function renderStatus(v) {
  const s = v.status;
  $('phase').textContent = s.phaseAr;
  mixed($('updatedAt'), [L(s.updatedAt)]);
  const c = s.latestCheck;
  mixed($('latestCheck'), c
    ? joined([['Draft ', L(c.revision), ': ', L(c.summary.passed), ' passed, ', L(c.summary.recorded), ' recorded, ', L(c.summary.failed), ' failed'], [L(c.executedAtUtc)], [L(c.source)]])
    : ['Unavailable (no saved results file)']);
  const w = s.activeWorker;
  const la = s.ledgerActiveAuthor;
  mixed($('activeWorker'), w
    ? joined([['Task ', L(w.mode)], [L(`pid ${na(w.pid)}`)], ['Started ', L(w.startedAt)]])
    : joined([['None'], ['Coordinator ledger: ', ...(la ? [L(la.task), ' (', L(la.state), ')'] : ['Unavailable'])]]));
  mixed($('reviewer'), joined([[REVIEWER_EN[v.reviewer.status] || v.reviewer.status], v.reviewer.heartbeatAt ? ['Last heartbeat ', L(v.reviewer.heartbeatAt)] : null]));
  const nt = v.notifier;
  mixed($('notifier'), nt.configured
    ? joined([['Configured'], [L(`codex queue --remote ${nt.remote}`)], ['Thread ', L(`…${nt.threadSuffix}`)], nt.held ? ['Temporarily held'] : null])
    : ['Not configured: ', nt.reason]);
  $('pauseState').textContent = PAUSE_EN[v.pauseState] || v.pauseState;
  $('waiting').textContent = WAIT_EN[v.waiting] || v.waiting;
  $('findings').replaceChildren(...s.openFindings.map((f) => el('li', `${f.id} (${f.status}): ${f.decision || ''}`)));
  $('statusNote').textContent = s.noteAr;
  $('pause').disabled = v.pauseState !== 'active';
  $('resume').disabled = v.pauseState === 'active';
}

/** Read-only: standing delegation and continuation progress from safe ledger fields. Never an owner answer. */
function renderAuthority(v) {
  const a = v.authority;
  if (!a) return;
  const s = a.standing;
  mixed($('authStanding'), s
    ? joined([['Sequential cycles: ', yesNo(s.autonomousSequentialCycles)], ['Delegated technical decisions: ', yesNo(s.delegatedTechnicalDecisions)],
      ['No production: ', yesNo(s.noProduction)], ['Personal owner approval: ', yesNo(s.personalApproval)], s.at ? ['Since ', L(s.at)] : null])
    : ['No standing authorization recorded']);
  $('authScope').textContent = s?.scope || 'Unavailable';
  const w = a.worklist;
  mixed($('authWorklist'), w
    ? joined([['Status ', L(w.status)], ...(w.items.length ? w.items.map((it) => [L(it.id), ' (', L(it.kind), it.status ? ', ' : '', it.status ? L(it.status) : null, ')']) : [['No items']])])
    : ['Unavailable']);
  const c = a.continuation;
  const last = c.lastReview;
  mixed($('authContinuation'), joined([
    [c.enabled ? 'Allowed by saved policy' : `Not allowed: ${CONT_EN[c.reason] || c.reason}`],
    c.gate && c.enabled ? ['Now: ', CONT_EN[c.gate] || c.gate] : null,
    last ? ['Latest review: ', CONT_STATUS_EN[last.status] || last.status, last.reason ? ` (${CONT_EN[last.reason] || last.reason})` : ''] : null,
    c.recent.length ? ['Continuation requests: ', L(c.recent.length)] : null,
  ]));
  $('authNote').textContent = a.noteAr;
}

function renderQueue(v) {
  const box = $('queue');
  if (!v.items.length) { box.replaceChildren(el('p', 'No requests yet.', 'empty')); return; }
  box.replaceChildren(...v.items.slice().reverse().map((i) => {
    const d = el('div', null, 'item');
    const head = el('div', KIND_EN[i.kind] || i.kind);
    head.append(el('span', STATE_EN[i.status] || i.status, `state ${i.status}`));
    d.append(head);
    if (i.payload?.text) d.append(el('p', i.payload.text));
    if (i.kind === 'ownerAnswer') d.append(elm('p', joined([[L(i.payload.questionId), ': ', L(i.payload.choice), i.payload.testOnly ? ' (test only)' : null], i.payload.note ? [i.payload.note] : null])));
    if (i.kind === 'continuation') {
      d.append(elm('p', joined([['After review ', L(i.payload.reviewId)], ['Ready items: ', ...i.payload.readyItems.flatMap((x, k) => (k ? [', ', L(x.id)] : [L(x.id)]))],
        i.nextJobId ? ['Coordinator-selected next step: ', L(i.nextJobId)] : null])));
    }
    if (i.kind === 'authorJob') {
      d.append(elm('p', joined([['Type: ', L(i.payload.mode)], i.dispatch?.plannedDir ? ['Directory ', L(i.dispatch.plannedDir)] : null,
        i.payload.continuationOf ? ['From continuation request ', L(i.payload.continuationOf)] : null,
        i.blockedReason ? ['Blocked: ', i.blockedReason] : null])));
      if (i.error) d.append(el('p', 'Error:', 'muted'), el('pre', i.error, 'tech'));
    }
    if (i.receipt?.result) d.append(el('pre', i.receipt.result.text || '(No result text)', 'tech'));
    if (i.stderrExcerpt) { d.append(el('div', 'stderr excerpt (secrets redacted, abbreviated):', 'entry-kind'), el('pre', i.stderrExcerpt, 'tech')); }
    for (const n of v.notifications.filter((x) => x.itemId === i.id)) {
      const last = n.attempts.at(-1);
      d.append(elm('p', joined([[`Host-coordinator notification (${NOTIFY_REASON_EN[n.reason] || n.reason}): ${NOTIFY_EN[n.status] || n.status}`],
        last ? ['Attempt ', L(last.n)] : null, last ? [L(last.endedAt || last.startedAt)] : null, n.queueId ? ['Queue ID ', L(n.queueId)] : null]), 'muted'));
      if (n.error) d.append(el('pre', n.error, 'tech'));
      if (['failed', 'uncertain', 'notConfigured'].includes(n.status)) {
        const b = el('button', 'Explicitly resend notification (same request)', 'secondary');
        b.addEventListener('click', () => act(`notify-${n.id}-${n.attemptSeq ?? n.attempts.length}`, '/api/notify-retry', { notificationId: n.id }));
        d.append(b);
      }
    }
    if (i.kind === 'authorJob' && ['completed', 'failed', 'interrupted'].includes(i.status)) {
      const rv = i.reviewId ? v.reviews.find((r) => r.id === i.reviewId) : null;
      d.append(elm('p', i.reviewId
        ? ['Actually reviewed: ', ...(rv ? joined([[L(rv.verdict)], [L(rv.reviewerId)], [L(rv.at)]]) : [L(i.reviewId)])]
        : ['Not reviewed yet; no other task starts before an actual Codex review.'], 'muted'));
    }
    // A failed or interrupted job keeps its retry button after its review; the retry still
    // needs the coordinator's approval and only starts once the failure has been reviewed.
    if (i.kind === 'authorJob' && ['failed', 'interrupted'].includes(i.status)) {
      const b = el('button', i.reviewId ? 'Request explicit retry from checkpoint' : 'Request explicit retry (starts after failure review)', 'secondary');
      b.addEventListener('click', () => act(`retry-${i.id}`, '/api/retry', { mode: i.payload.mode, retryOf: i.id, note: 'Explicit retry of failed task' }));
      d.append(b);
    }
    d.append(elm('div', joined([[L(i.id)], [L(i.createdAt)]]), 'meta'));
    return d;
  }));
}

let ownerSig = null;
function renderOwner(v) {
  // Re-render only when stored answers change, so an answer being typed is not wiped by polling.
  const sig = JSON.stringify(v.owner.map((q) => q.answer));
  if (sig === ownerSig) return;
  ownerSig = sig;
  $('owner').replaceChildren(...v.owner.map((q) => {
    const card = el('div', null, `owner-card${q.testOnly ? ' test' : ''}`);
    card.append(el('h3', `${q.id} · ${q.titleAr}${q.testOnly ? ' — test only, adopts no policy' : ''}`), el('p', q.questionAr), el('p', 'Why: ' + q.whyAr, 'muted'), el('p', 'Recommendation: ' + q.recommendationAr));
    const opts = el('div', null, 'options');
    for (const o of q.options) {
      const label = el('label');
      const r = document.createElement('input'); r.type = 'radio'; r.name = `q-${q.id}`; r.value = o.id;
      label.append(r, el('span', `${o.labelAr} — Effect: ${o.consequenceAr}`));
      opts.append(label);
    }
    const note = document.createElement('textarea'); note.rows = 2; note.maxLength = 2000; note.placeholder = 'Optional note'; note.setAttribute('aria-label', `Note ${q.id}`);
    const send = el('button', 'Save answer and deliver to coordinator', 'secondary');
    send.addEventListener('click', () => {
      const chosen = opts.querySelector('input:checked');
      if (!chosen) return showError('Select an option first.');
      act(`owner-${q.id}`, '/api/owner-answer', { questionId: q.id, choice: chosen.value, note: note.value });
    });
    card.append(opts, note, send);
    if (q.answer) card.append(elm('p', ['Latest saved answer: ', ...joined([[L(q.answer.choice)], [STATE_EN[q.answer.status] || q.answer.status], [L(q.answer.at)]])], 'answer'));
    return card;
  }));
}

const ROLE = { author: ['CLAUDE · Author', 'author'], reviewer: ['CODEX · Reviewer', 'reviewer'], coordinator: ['Coordinator · Task', 'coordinator'] };
function renderHistory(cards) {
  const current = cards.filter((c) => c.current).length;
  $('historyCount').textContent = `${cards.length} cards (${current} current)`;
  $('history').replaceChildren(...cards.map((c) => {
    const a = el('article', null, `message ${c.role === 'reviewer' ? 'codex' : ''}`);
    const h = el('h3');
    const [label, cls] = ROLE[c.role] || [c.role, ''];
    h.append(el('span', label, `badge ${cls}`), el('span', c.current ? 'Current' : 'Historical', 'badge'), el('span', c.summaryAr));
    a.append(h);
    a.append(elm('p', joined([[L(`time: ${c.time ?? 'unavailable'}`), c.timeNote ? ` (${c.timeNote})` : null], c.jobId ? [L(`job: ${c.jobId}`)] : null,
      c.verdict ? [L(`verdict: ${c.verdict}`)] : null, c.reviewerId ? [L(`reviewer: ${c.reviewerId}`)] : null,
      c.task ? [L(`task: ${c.task}`)] : null, c.revision ? [L(`revision: ${c.revision}`)] : null,
      c.sha256 ? [L(`sha256: ${c.sha256.slice(0, 16)}…`)] : null, c.commit ? [L(`commit: ${String(c.commit).slice(0, 12)}`)] : null, [L(`source: ${c.source}`)]]), 'meta'));
    const det = el('details'); det.append(el('summary', 'Technical text (English)'));
    if (c.result) { det.append(elLtr('div', `result · error=${c.result.isError} · turns=${na(c.result.numTurns)} · permission denials=${c.result.permissionDenials.length}`, 'entry-kind'), el('pre', c.result.text || '(empty)', 'tech')); }
    for (const e of c.technical || []) {
      if (e.kind === 'tool_use') det.append(elLtr('div', `tool call: ${e.name} ${e.at ? '· ' + e.at : ''}`, 'entry-kind'), el('pre', JSON.stringify(e.input, null, 1), 'tech'));
      else det.append(elLtr('div', `${e.kind}${e.isError ? ' (error)' : ''} ${e.at ? '· ' + e.at : ''}`, 'entry-kind'), el('pre', e.text, 'tech'));
    }
    if (c.truncatedEntries) det.append(el('p', `${c.truncatedEntries} older entries were omitted from display.`, 'muted'));
    a.append(det);
    return a;
  }));
}

async function act(action, url, data) {
  showError('');
  try {
    await api(url, { ...data, idempotencyKey: keyFor(action) });
    clearKey(action);
    if (action === 'guidance') $('guidance').value = '';
    await refresh();
  } catch (e) {
    showError(e.http ? e.message : 'Connection to the local server lost. The request key is saved; resend after reconnection without duplicating the request.');
  }
}

$('sendGuidance').addEventListener('click', () => act('guidance', '/api/guidance', { text: $('guidance').value }));
$('requestJob').addEventListener('click', () => act(`job-${$('jobMode').value}`, '/api/author-job', { mode: $('jobMode').value, note: $('guidance').value.trim() ? $('guidance').value : '' }));
$('pause').addEventListener('click', () => act('pause', '/api/pause', {}));
$('resume').addEventListener('click', () => act('resume', '/api/resume', { reason: 'Resume from UI' }));

async function refresh() {
  try {
    const v = await api('/api/state');
    setConnection(true, `Connected to local server · ${new Date().toLocaleTimeString('en')}`);
    renderStatus(v); renderAuthority(v); renderQueue(v); renderOwner(v);
    // Current Claude receipts and Codex reviews join the timeline on the ordinary poll:
    // the timeline is refetched whenever the server's signature of current cards changes.
    if (v.timelineSig !== timelineSig) { timelineSig = v.timelineSig; await loadHistory(); }
  } catch (e) {
    setConnection(false, e.http ? `Server rejected request: ${e.message}` : 'Connection to the local server lost (server stopped or local network unavailable). No saved requests were lost.');
  }
}
let timelineSig = null;
async function loadHistory() {
  try { renderHistory((await api('/api/history')).cards); }
  catch (e) { timelineSig = null; $('history').replaceChildren(el('p', 'Could not load history: ' + e.message, 'empty')); }
}
async function poll() { await refresh(); setTimeout(poll, 2500); }
poll(); setInterval(loadHistory, 60_000);
