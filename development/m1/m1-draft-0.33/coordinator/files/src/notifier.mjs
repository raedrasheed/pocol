// Host notification transport: queues ONE fixed, short message into the EXISTING Codex
// coordinator thread with `codex queue --remote unix:// --thread <id> --message <text>`.
// It is a delivery mechanism only. It never starts, forks or resumes a thread on its own
// terms, never runs `codex exec` or an app-server, never overrides model or permissions,
// and never carries browser text or secrets: the message names a broker item ID and the
// private connection file path, nothing else. Delivery is not a claim, an ack or a review.
import { spawn } from 'node:child_process';
import { statSync } from 'node:fs';
import path from 'node:path';
import { bound } from './util.mjs';
import { redact } from './redact.mjs';

export const THREAD_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const REMOTE = 'unix://';                          // the already-running local daemon; never spawns a server
const ITEM_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const WORD_RE = /^[A-Za-z]{1,24}$/;
const OUT_MAX = 4000;
const REASONS = {
  queued: 'a new user request was saved in the local broker queue',
  jobFinished: 'a user-requested author job has ended and needs an actual review of its real receipt',
  continuation: 'an actual review was recorded and the saved standing authorization allows one continuation request',
};

/** An explicit executable path: an existing regular file; on Windows a real .exe (shell:false cannot run .cmd). */
export function validateExe(p, { platform = process.platform, label = 'codex' } = {}) {
  const abs = path.resolve(p);
  let st = null;
  try { st = statSync(abs); } catch { /* missing */ }
  if (!st) throw new Error(`مسار ${label} المحدد غير موجود.`);
  if (!st.isFile()) throw new Error(`مسار ${label} المحدد ليس ملفًا تنفيذيًا.`);
  if (platform === 'win32' && path.extname(abs).toLowerCase() !== '.exe') throw new Error(`على Windows يجب أن يكون مسار ${label} ملف .exe؛ ملفات .cmd و.bat و.ps1 غير مدعومة لأن التشغيل دون shell.`);
  return abs;
}

/** The fixed notification text. Only server-side values: item ID, fixed vocabularies, private file path. */
export function notificationMessage({ itemId, reason, kind = null, status = null, connectionPath }) {
  if (!ITEM_RE.test(itemId || '')) throw new Error('معرف عنصر الوسيط غير صالح.');
  if (!Object.hasOwn(REASONS, reason)) throw new Error('سبب الإشعار غير معروف.');
  if (kind !== null && !WORD_RE.test(kind)) throw new Error('نوع العنصر غير صالح.');
  if (status !== null && !WORD_RE.test(status)) throw new Error('حالة العنصر غير صالحة.');
  if (typeof connectionPath !== 'string' || !path.isAbsolute(connectionPath) || /[\r\n]/.test(connectionPath)) throw new Error('مسار ملف الاتصال غير صالح.');
  const head = [
    'PoCol local coordinator notice (transport only: not a review, not an approval, not a new task).',
    `Broker item ${itemId}${kind ? ` [${kind}${status ? ', ' + status : ''}]` : ''}: ${REASONS[reason]}.`,
  ];
  if (reason === 'continuation') {
    return [...head,
      `As the existing host coordinator, read the private connection file ${connectionPath} for the reviewer API and header, attach,`,
      'then read coordination/issue-ledger.json (standingAuthorization, continuationWork) and the latest checkpoint, claim this one item,',
      'and decide the next useful M1 action yourself: acknowledge it with an explicit plan, or with no action. Nothing was reviewed or started automatically.',
      'Preserve the M1 policy, the pause control and the single-writer rule; never approve anything on the owner\'s behalf.',
      'Ignore continuation items that are already acknowledged.',
    ].join(' ');
  }
  return [...head,
    `As the existing host coordinator, read the private connection file ${connectionPath} for the reviewer API and header, attach,`,
    'then claim/ack the actual pending requests; for an ended author job, review its real receipt before recording a review.',
    'Preserve the M1 policy and the single-writer rule; never approve anything on the owner\'s behalf.',
    'Ignore items that are already acknowledged or reviewed.',
  ].join(' ');
}

export function buildNotifyArgs({ threadId, message }) {
  if (!THREAD_RE.test(threadId || '')) throw new Error('معرف خيط Codex غير صالح.');
  if (typeof message !== 'string' || !message || message.length > 2000) throw new Error('نص الإشعار غير صالح.');
  return ['queue', '--remote', REMOTE, '--thread', threadId, '--message', message];
}

/** Best-effort queued-message ID from the CLI output: a JSON id field, else the first UUID that is not a known one. */
export function parseQueueId(stdout, known = []) {
  for (const line of String(stdout).split(/\r?\n/)) {
    if (!line.trim().startsWith('{')) continue;
    try { const j = JSON.parse(line); const id = j.id ?? j.queueId ?? j.queue_id ?? j.messageId ?? j.message_id; if (typeof id === 'string' && id.length <= 200) return id; } catch { /* not JSON */ }
  }
  const skip = new Set(known.map((k) => String(k).toLowerCase()));
  for (const m of String(stdout).matchAll(/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/gi)) if (!skip.has(m[0].toLowerCase())) return m[0];
  return null;
}

export class Notifier {
  constructor({ codexBin = null, threadId = null, ws, spawnImpl = spawn, timeoutMs = 30_000, platform = process.platform }) {
    Object.assign(this, { ws, spawnImpl, timeoutMs, threadId: threadId || null, codexBin: null, problem: null });
    if (!codexBin) this.problem = 'لم يُحدد --codex-bin؛ الطلبات تُحفظ لكن لا يُرسل إشعار إلى المنسق المضيف.';
    else { try { this.codexBin = validateExe(codexBin, { platform, label: 'codex' }); } catch (e) { this.problem = e.message; } }
    if (!this.problem && !THREAD_RE.test(this.threadId || '')) this.problem = 'معرف خيط Codex القائم غير محدد أو غير صالح (--codex-thread أو CODEX_THREAD_ID).';
  }

  configured() { return this.problem ? { ok: false, reason: this.problem } : { ok: true }; }
  describe() { return { configured: !this.problem, reason: this.problem, remote: REMOTE, threadId: this.threadId, codexBin: this.codexBin }; }

  /**
   * One attempt. Resolves (never rejects) with { ok, uncertain?, code, queueId?, output, stderr, error? }.
   * A launch error (e.g. ENOENT) means not sent; a timeout means the outcome is unknown.
   */
  send(info) {
    return new Promise((resolve) => {
      let args;
      try {
        if (this.problem) throw new Error(this.problem);
        args = buildNotifyArgs({ threadId: this.threadId, message: notificationMessage(info) });
      } catch (e) { resolve({ ok: false, error: String(e.message) }); return; }
      let out = '', err = '', settled = false, timer = null, child = null;
      const take = (cur, chunk) => (cur.length >= OUT_MAX ? cur : cur + String(chunk).slice(0, OUT_MAX - cur.length));
      const finish = (r) => {
        if (settled) return;
        settled = true;
        if (timer) clearTimeout(timer);
        resolve({ ...r, output: redact(bound(out.trim(), OUT_MAX)), stderr: redact(bound(err.trim(), OUT_MAX)) });
      };
      try {
        child = this.spawnImpl(this.codexBin, args, { cwd: this.ws, shell: false, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
      } catch (e) { finish({ ok: false, error: `تعذر تشغيل codex: ${e.message}` }); return; }
      child.stdout?.on('data', (c) => { out = take(out, c); });
      child.stderr?.on('data', (c) => { err = take(err, c); });
      child.on('error', (e) => finish({ ok: false, code: null, error: `تعذر تشغيل codex: ${e?.message || e}` }));
      child.on('close', (code, signal) => {
        if (code === 0) finish({ ok: true, code, queueId: parseQueueId(out, [this.threadId, info.itemId]) });
        else finish({ ok: false, code, error: `انتهى codex queue بالرمز ${code ?? signal}` });
      });
      timer = setTimeout(() => {
        try { child.kill(); } catch { /* already gone */ }
        finish({ ok: false, uncertain: true, error: `انتهت المهلة (${this.timeoutMs} ms)؛ لا يُعرف هل وصل الإشعار.` });
      }, this.timeoutMs);
      timer.unref?.();
    });
  }
}
