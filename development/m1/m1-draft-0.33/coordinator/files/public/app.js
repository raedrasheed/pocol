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
const na = (v) => (v === null || v === undefined || v === '' ? 'غير متاح' : String(v));

// Mixed Arabic/technical lines. Arabic prose stays plain text nodes; every English or numeric
// fragment (time, path, ID, hash, command, count) is its own <bdi dir="ltr"> holding a text node,
// so the RTL layout cannot reorder it. Text only: textContent / createTextNode.
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

const STATE_AR = { queued: 'في الطابور', claimed: 'استلمه المنسق', acknowledged: 'أكد المنسق الاستلام', approved: 'وافق المنسق؛ ينتظر التشغيل', running: 'قيد التشغيل',
  completed: 'اكتمل؛ ينتظر مراجعة Codex', reviewed: 'رُوجع', failed: 'فشل', interrupted: 'انقطع', cancelled: 'أُلغي' };
const KIND_AR = { guidance: 'توجيه', ownerAnswer: 'إجابة سؤال مالك', authorJob: 'دورة مؤلف', continuation: 'طلب متابعة بعد مراجعة فعلية' };
const WAIT_AR = { idle: 'لا شيء معلق', workerRunning: 'مهمة المؤلف قيد التشغيل',
  workerFailed: 'آخر مهمة مؤلف فشلت أو انقطعت؛ بانتظار مراجعة Codex الفعلية لها، ثم إعادة صريحة إن طُلبت',
  waitingReview: 'بانتظار مراجعة Codex الفعلية؛ لن تبدأ دورة أخرى قبلها', waitingCoordinator: 'بانتظار المنسق ليستلم الطلبات',
  waitingReviewerAttach: 'المنسق المضيف غير متصل (دورة Codex الحالية غير نشطة)؛ الطلبات محفوظة وتنتظر اتصاله',
  dispatchBlocked: 'مهمة موافق عليها لكنها محجوبة؛ انظر السبب في الطابور', retryAvailable: 'رُوجعت المهمة الفاشلة؛ يمكن طلب إعادة صريحة' };
const PAUSE_AR = { active: 'يعمل', draining: 'إيقاف مطلوب؛ ينتظر انتهاء المهمة الجارية عند حد آمن', paused: 'متوقف مؤقتًا' };
// Notification delivery is a transport state only; it is never shown as a claim, an ack or a review.
const NOTIFY_AR = { held: 'محجوز حتى يكتمل تشغيل الخادم', sending: 'جارٍ الإرسال إلى خيط Codex القائم',
  delivered: 'وصل إلى طابور خيط Codex القائم (هذا ليس استلامًا ولا مراجعة)', failed: 'فشل الإرسال؛ الطلب نفسه محفوظ',
  uncertain: 'غير مؤكد: توقف الخادم أو انتهت المهلة أثناء الإرسال؛ الإعادة قد تكرر الإشعار', notConfigured: 'الناقل غير مضبوط؛ الطلب محفوظ ولم يُرسل إشعار' };
const NOTIFY_REASON_AR = { queued: 'طلب جديد', jobFinished: 'طلب مراجعة لمهمة انتهت', continuation: 'طلب متابعة بعد مراجعة فعلية' };
const REVIEWER_AR = { attached: 'متصل', expired: 'انتهت مهلة الاتصال؛ لا مراجعة جارية الآن', disconnected: 'غير متصل' };
// Why no continuation request is (or would be) queued. Display only; nothing here changes policy.
const CONT_AR = { ledgerUnreadable: 'تعذرت قراءة سجل المنسق', noStandingAuthorization: 'لا يوجد تفويض دائم مسجل',
  autonomousCyclesOff: 'الدورات المتتابعة غير مفعلة في التفويض', noProductionNotTrue: 'شرط «لا إنتاج» غير مسجل صراحة',
  scopeNotM1: 'نطاق التفويض لا يذكر M1', noWorklist: 'لا توجد قائمة عمل مستقل محفوظة', worklistComplete: 'قائمة العمل مكتملة؛ لا متابعة',
  worklistNotReady: 'قائمة العمل ليست جاهزة', worklistMalformed: 'قائمة العمل غير صالحة؛ لا متابعة', noIndependentReadyItem: 'لا يوجد عنصر مستقل جاهز',
  stopped: 'الخادم يتوقف', paused: 'متوقف مؤقتًا؛ تؤجل المتابعة حتى الاستئناف', writerActive: 'يوجد مؤلف أو عقد عامل نشط',
  unreviewedReceipt: 'يوجد إيصال لم يُراجع بعد', openContinuation: 'يوجد طلب متابعة مفتوح بالفعل' };
const CONT_STATUS_AR = { pending: 'قيد التقرير', queued: 'أُدرج طلب متابعة واحد', suppressed: 'لم يُدرج', deferredPaused: 'مؤجل بسبب الإيقاف' };
const yesNo = (b) => (b ? 'نعم' : 'لا');

function renderStatus(v) {
  const s = v.status;
  $('phase').textContent = s.phaseAr;
  mixed($('updatedAt'), [L(s.updatedAt)]);
  const c = s.latestCheck;
  mixed($('latestCheck'), c
    ? joined([['المسودة ', L(c.revision), ': ', L(c.summary.passed), ' ناجح، ', L(c.summary.recorded), ' مسجل، ', L(c.summary.failed), ' فاشل'], [L(c.executedAtUtc)], [L(c.source)]])
    : ['غير متاح (لا يوجد ملف نتائج محفوظ)']);
  const w = s.activeWorker;
  const la = s.ledgerActiveAuthor;
  mixed($('activeWorker'), w
    ? joined([['مهمة ', L(w.mode)], [L(`pid ${na(w.pid)}`)], ['بدأت ', L(w.startedAt)]])
    : joined([['لا يوجد'], ['سجل المنسق: ', ...(la ? [L(la.task), ' (', L(la.state), ')'] : ['غير متاح'])]]));
  mixed($('reviewer'), joined([[REVIEWER_AR[v.reviewer.status] || v.reviewer.status], v.reviewer.heartbeatAt ? ['آخر نبضة ', L(v.reviewer.heartbeatAt)] : null]));
  const nt = v.notifier;
  mixed($('notifier'), nt.configured
    ? joined([['مضبوط'], [L(`codex queue --remote ${nt.remote}`)], ['الخيط ', L(`…${nt.threadSuffix}`)], nt.held ? ['محجوز مؤقتًا'] : null])
    : ['غير مضبوط: ', nt.reason]);
  $('pauseState').textContent = PAUSE_AR[v.pauseState] || v.pauseState;
  $('waiting').textContent = WAIT_AR[v.waiting] || v.waiting;
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
    ? joined([['دورات متتابعة: ', yesNo(s.autonomousSequentialCycles)], ['قرارات تقنية مفوضة: ', yesNo(s.delegatedTechnicalDecisions)],
      ['لا إنتاج: ', yesNo(s.noProduction)], ['موافقة شخصية من المالك: ', yesNo(s.personalApproval)], s.at ? ['منذ ', L(s.at)] : null])
    : ['لا يوجد تفويض دائم مسجل']);
  $('authScope').textContent = s?.scope || 'غير متاح';
  const w = a.worklist;
  mixed($('authWorklist'), w
    ? joined([['الحالة ', L(w.status)], ...(w.items.length ? w.items.map((it) => [L(it.id), ' (', L(it.kind), it.status ? ', ' : '', it.status ? L(it.status) : null, ')']) : [['لا عناصر']])])
    : ['غير متاح']);
  const c = a.continuation;
  const last = c.lastReview;
  mixed($('authContinuation'), joined([
    [c.enabled ? 'مسموح بالسياسة المحفوظة' : `غير مسموح: ${CONT_AR[c.reason] || c.reason}`],
    c.gate && c.enabled ? ['الآن: ', CONT_AR[c.gate] || c.gate] : null,
    last ? ['آخر مراجعة: ', CONT_STATUS_AR[last.status] || last.status, last.reason ? ` (${CONT_AR[last.reason] || last.reason})` : ''] : null,
    c.recent.length ? ['طلبات المتابعة: ', L(c.recent.length)] : null,
  ]));
  $('authNote').textContent = a.noteAr;
}

function renderQueue(v) {
  const box = $('queue');
  if (!v.items.length) { box.replaceChildren(el('p', 'لا توجد طلبات بعد.', 'empty')); return; }
  box.replaceChildren(...v.items.slice().reverse().map((i) => {
    const d = el('div', null, 'item');
    const head = el('div', KIND_AR[i.kind] || i.kind);
    head.append(el('span', STATE_AR[i.status] || i.status, `state ${i.status}`));
    d.append(head);
    if (i.payload?.text) d.append(el('p', i.payload.text));
    if (i.kind === 'ownerAnswer') d.append(elm('p', joined([[L(i.payload.questionId), ': ', L(i.payload.choice), i.payload.testOnly ? ' (اختبار فقط)' : null], i.payload.note ? [i.payload.note] : null])));
    if (i.kind === 'continuation') {
      d.append(elm('p', joined([['بعد المراجعة ', L(i.payload.reviewId)], ['العناصر الجاهزة: ', ...i.payload.readyItems.flatMap((x, k) => (k ? [', ', L(x.id)] : [L(x.id)]))],
        i.nextJobId ? ['الخطوة التي اختارها المنسق: ', L(i.nextJobId)] : null])));
    }
    if (i.kind === 'authorJob') {
      d.append(elm('p', joined([['النوع: ', L(i.payload.mode)], i.dispatch?.plannedDir ? ['المجلد ', L(i.dispatch.plannedDir)] : null,
        i.payload.continuationOf ? ['من طلب متابعة ', L(i.payload.continuationOf)] : null,
        i.blockedReason ? ['محجوب: ', i.blockedReason] : null])));
      if (i.error) d.append(el('p', 'خطأ:', 'muted'), el('pre', i.error, 'tech'));
    }
    if (i.receipt?.result) d.append(el('pre', i.receipt.result.text || '(لا يوجد نص نتيجة)', 'tech'));
    if (i.stderrExcerpt) { d.append(el('div', 'مقتطف stderr (محجوب الأسرار، مختصر):', 'entry-kind'), el('pre', i.stderrExcerpt, 'tech')); }
    for (const n of v.notifications.filter((x) => x.itemId === i.id)) {
      const last = n.attempts.at(-1);
      d.append(elm('p', joined([[`إشعار المنسق المضيف (${NOTIFY_REASON_AR[n.reason] || n.reason}): ${NOTIFY_AR[n.status] || n.status}`],
        last ? ['محاولة ', L(last.n)] : null, last ? [L(last.endedAt || last.startedAt)] : null, n.queueId ? ['معرف الطابور ', L(n.queueId)] : null]), 'muted'));
      if (n.error) d.append(el('pre', n.error, 'tech'));
      if (['failed', 'uncertain', 'notConfigured'].includes(n.status)) {
        const b = el('button', 'إعادة إرسال الإشعار صراحة (للطلب نفسه)', 'secondary');
        b.addEventListener('click', () => act(`notify-${n.id}-${n.attemptSeq ?? n.attempts.length}`, '/api/notify-retry', { notificationId: n.id }));
        d.append(b);
      }
    }
    if (i.kind === 'authorJob' && ['completed', 'failed', 'interrupted'].includes(i.status)) {
      const rv = i.reviewId ? v.reviews.find((r) => r.id === i.reviewId) : null;
      d.append(elm('p', i.reviewId
        ? ['رُوجعت فعليًا: ', ...(rv ? joined([[L(rv.verdict)], [L(rv.reviewerId)], [L(rv.at)]]) : [L(i.reviewId)])]
        : ['لم تُراجع بعد؛ لن تبدأ أي مهمة أخرى قبل مراجعة Codex الفعلية.'], 'muted'));
    }
    // A failed or interrupted job keeps its retry button after its review; the retry still
    // needs the coordinator's approval and only starts once the failure has been reviewed.
    if (i.kind === 'authorJob' && ['failed', 'interrupted'].includes(i.status)) {
      const b = el('button', i.reviewId ? 'طلب إعادة صريحة من نقطة التثبيت' : 'طلب إعادة صريحة (تبدأ بعد مراجعة الفشل)', 'secondary');
      b.addEventListener('click', () => act(`retry-${i.id}`, '/api/retry', { mode: i.payload.mode, retryOf: i.id, note: 'إعادة صريحة لمهمة فاشلة' }));
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
    card.append(el('h3', `${q.id} · ${q.titleAr}${q.testOnly ? ' — اختبار فقط، لا يعتمد سياسة' : ''}`), el('p', q.questionAr), el('p', 'لماذا: ' + q.whyAr, 'muted'), el('p', 'التوصية: ' + q.recommendationAr));
    const opts = el('div', null, 'options');
    for (const o of q.options) {
      const label = el('label');
      const r = document.createElement('input'); r.type = 'radio'; r.name = `q-${q.id}`; r.value = o.id;
      label.append(r, el('span', `${o.labelAr} — النتيجة: ${o.consequenceAr}`));
      opts.append(label);
    }
    const note = document.createElement('textarea'); note.rows = 2; note.maxLength = 2000; note.placeholder = 'ملاحظة اختيارية'; note.setAttribute('aria-label', `ملاحظة ${q.id}`);
    const send = el('button', 'حفظ الإجابة وتسليمها للمنسق', 'secondary');
    send.addEventListener('click', () => {
      const chosen = opts.querySelector('input:checked');
      if (!chosen) return showError('اختر خيارًا أولًا.');
      act(`owner-${q.id}`, '/api/owner-answer', { questionId: q.id, choice: chosen.value, note: note.value });
    });
    card.append(opts, note, send);
    if (q.answer) card.append(elm('p', ['آخر إجابة محفوظة: ', ...joined([[L(q.answer.choice)], [STATE_AR[q.answer.status] || q.answer.status], [L(q.answer.at)]])], 'answer'));
    return card;
  }));
}

const ROLE = { author: ['CLAUDE · المؤلف', 'author'], reviewer: ['CODEX · المراجع', 'reviewer'], coordinator: ['المنسق · مهمة', 'coordinator'] };
function renderHistory(cards) {
  const current = cards.filter((c) => c.current).length;
  $('historyCount').textContent = `${cards.length} بطاقة (${current} حالية)`;
  $('history').replaceChildren(...cards.map((c) => {
    const a = el('article', null, `message ${c.role === 'reviewer' ? 'codex' : ''}`);
    const h = el('h3');
    const [label, cls] = ROLE[c.role] || [c.role, ''];
    h.append(el('span', label, `badge ${cls}`), el('span', c.current ? 'حالية' : 'تاريخية', 'badge'), el('span', c.summaryAr));
    a.append(h);
    a.append(elm('p', joined([[L(`time: ${c.time ?? 'unavailable'}`), c.timeNote ? ` (${c.timeNote})` : null], c.jobId ? [L(`job: ${c.jobId}`)] : null,
      c.verdict ? [L(`verdict: ${c.verdict}`)] : null, c.reviewerId ? [L(`reviewer: ${c.reviewerId}`)] : null,
      c.task ? [L(`task: ${c.task}`)] : null, c.revision ? [L(`revision: ${c.revision}`)] : null,
      c.sha256 ? [L(`sha256: ${c.sha256.slice(0, 16)}…`)] : null, c.commit ? [L(`commit: ${String(c.commit).slice(0, 12)}`)] : null, [L(`source: ${c.source}`)]]), 'meta'));
    const det = el('details'); det.append(el('summary', 'النص التقني (بالإنجليزية)'));
    if (c.result) { det.append(elLtr('div', `result · error=${c.result.isError} · turns=${na(c.result.numTurns)} · permission denials=${c.result.permissionDenials.length}`, 'entry-kind'), el('pre', c.result.text || '(empty)', 'tech')); }
    for (const e of c.technical || []) {
      if (e.kind === 'tool_use') det.append(elLtr('div', `tool call: ${e.name} ${e.at ? '· ' + e.at : ''}`, 'entry-kind'), el('pre', JSON.stringify(e.input, null, 1), 'tech'));
      else det.append(elLtr('div', `${e.kind}${e.isError ? ' (error)' : ''} ${e.at ? '· ' + e.at : ''}`, 'entry-kind'), el('pre', e.text, 'tech'));
    }
    if (c.truncatedEntries) det.append(el('p', `حُذف ${c.truncatedEntries} إدخالًا أقدم من العرض.`, 'muted'));
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
    showError(e.http ? e.message : 'انقطع الاتصال بالخادم المحلي. الطلب محفوظ بمفتاحه؛ أعد الإرسال بعد عودة الاتصال ولن يتكرر.');
  }
}

$('sendGuidance').addEventListener('click', () => act('guidance', '/api/guidance', { text: $('guidance').value }));
$('requestJob').addEventListener('click', () => act(`job-${$('jobMode').value}`, '/api/author-job', { mode: $('jobMode').value, note: $('guidance').value.trim() ? $('guidance').value : '' }));
$('pause').addEventListener('click', () => act('pause', '/api/pause', {}));
$('resume').addEventListener('click', () => act('resume', '/api/resume', { reason: 'استئناف من الواجهة' }));

async function refresh() {
  try {
    const v = await api('/api/state');
    setConnection(true, `متصل بالخادم المحلي · ${new Date().toLocaleTimeString('ar')}`);
    renderStatus(v); renderAuthority(v); renderQueue(v); renderOwner(v);
    // Current Claude receipts and Codex reviews join the timeline on the ordinary poll:
    // the timeline is refetched whenever the server's signature of current cards changes.
    if (v.timelineSig !== timelineSig) { timelineSig = v.timelineSig; await loadHistory(); }
  } catch (e) {
    setConnection(false, e.http ? `رفض الخادم الطلب: ${e.message}` : 'انقطع الاتصال بالخادم المحلي (الخادم متوقف أو الشبكة المحلية غير متاحة). لم يُفقد أي طلب محفوظ.');
  }
}
let timelineSig = null;
async function loadHistory() {
  try { renderHistory((await api('/api/history')).cards); }
  catch (e) { timelineSig = null; $('history').replaceChildren(el('p', 'تعذر تحميل السجل: ' + e.message, 'empty')); }
}
async function poll() { await refresh(); setTimeout(poll, 2500); }
poll(); setInterval(loadHistory, 60_000);
