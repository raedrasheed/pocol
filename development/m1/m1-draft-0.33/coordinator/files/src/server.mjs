// Loopback-only HTTP adapter. Security pattern adapted from PoCol Dialogue 0.3.6
// (app.mjs): 127.0.0.1 bind, strict Host/Origin, random token delivered in the URL
// fragment, timingSafeEqual comparison, CSP, no-store, fixed static file map.
// Added here: a separate reviewer capability that is never sent to the browser,
// JSON content-type/size limits, and the persisted broker behind every request.
// 0.33 patch: a graceful restart may keep the previous local URL (port and capabilities) when the
// private connection.json is valid for THIS loopback workspace; anything else fails closed to new
// capabilities with an explicit notice. Capabilities never appear in arguments or logs.
import http from 'node:http';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { timingSafeEqual } from 'node:crypto';
import { BrokerError, Controller } from './broker.mjs';
import { findClaude } from './worker.mjs';
import { Notifier } from './notifier.mjs';
import { newToken, readJson, writeJsonAtomic, isoNow } from './util.mjs';
import { redact, redactDeep, registerSecret } from './redact.mjs';

const base = path.dirname(fileURLToPath(import.meta.url));
const PUBLIC = path.join(base, '..', 'public');
const BODY_MAX = 64 * 1024;
const STATIC = new Map([
  ['/', [path.join(PUBLIC, 'index.html'), 'text/html; charset=utf-8']],
  ['/app.js', [path.join(PUBLIC, 'app.js'), 'text/javascript; charset=utf-8']],
  ['/style.css', [path.join(base, '..', 'dialogue-style.css'), 'text/css; charset=utf-8']],
  ['/coordinator.css', [path.join(PUBLIC, 'coordinator.css'), 'text/css; charset=utf-8']],
]);
const CSP = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'none'; font-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const TOKEN_RE = /^[0-9a-f]{64}$/;
const PORT_MIN = 1024, PORT_MAX = 65535;

function tokenOk(supplied, expected) {
  const a = Buffer.from(String(supplied || '')), b = Buffer.from(expected);
  return a.length === b.length && timingSafeEqual(a, b);
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = []; let size = 0, failed = false;
    req.on('data', (c) => {
      if (failed) return;
      size += c.length;
      if (size > BODY_MAX) { failed = true; reject(new BrokerError(413, 'الطلب أكبر من الحد المسموح.')); req.resume(); }
      else chunks.push(c);
    });
    req.on('end', () => {
      if (failed) return;
      try {
        const v = JSON.parse(Buffer.concat(chunks).toString('utf8') || 'null');
        if (!v || typeof v !== 'object' || Array.isArray(v)) return reject(new BrokerError(400, 'يجب أن يكون الطلب كائن JSON.'));
        resolve(v);
      } catch { reject(new BrokerError(400, 'صيغة JSON غير صالحة.')); }
    });
    req.on('error', reject);
  });
}

/**
 * Create the HTTP app. Returns { server, controlToken, reviewerToken }.
 * The control token is for the browser; the reviewer token is for the host coordinator only.
 */
export function createApp({ controller, controlToken = newToken(), reviewerToken = newToken() }) {
  registerSecret(controlToken); registerSecret(reviewerToken);
  const browserPost = {
    '/api/guidance': (b) => controller.submitGuidance(b),
    '/api/owner-answer': (b) => controller.submitOwnerAnswer(b),
    '/api/author-job': (b) => controller.requestAuthorJob({ mode: b.mode, note: b.note, idempotencyKey: b.idempotencyKey }),
    '/api/retry': (b) => controller.requestAuthorJob({ mode: b.mode, note: b.note, idempotencyKey: b.idempotencyKey, retryOf: b.retryOf }),
    '/api/pause': (b) => controller.pause(b),
    '/api/resume': (b) => controller.resume(b),
    '/api/notify-retry': (b) => controller.retryNotification({ notificationId: b.notificationId, idempotencyKey: b.idempotencyKey }),
  };
  const reviewerPost = {
    '/api/reviewer/attach': (b) => controller.attach(b),
    '/api/reviewer/heartbeat': () => controller.heartbeat(),
    '/api/reviewer/detach': () => controller.detach(),
    '/api/reviewer/claim': (b) => controller.claim(b),
    '/api/reviewer/ack': (b) => controller.ack(b),
    '/api/reviewer/review': (b) => controller.review(b),
  };

  const server = http.createServer(async (req, res) => {
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Referrer-Policy', 'no-referrer');
    res.setHeader('X-Frame-Options', 'DENY');
    res.setHeader('Content-Security-Policy', CSP);
    const send = (status, data) => {
      res.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8' });
      res.end(JSON.stringify(redactDeep(data)));
    };
    try {
      const port = server.address().port;
      const host = `127.0.0.1:${port}`, origin = `http://${host}`;
      if (req.headers.host !== host) return send(403, { error: 'مضيف غير مسموح.' });
      const url = new URL(req.url, origin);
      const isReviewer = url.pathname.startsWith('/api/reviewer/');
      if (isReviewer) {
        // The reviewer capability is for the host coordinator process, never for a browser page.
        if (req.headers.origin !== undefined || req.headers['sec-fetch-site'] !== undefined) return send(403, { error: 'واجهة المراجع لا تقبل طلبات المتصفح.' });
        if (!tokenOk(req.headers['x-pocol-reviewer'], reviewerToken)) return send(403, { error: 'رمز المراجع غير صالح.' });
      } else {
        if (req.headers.origin !== undefined && req.headers.origin !== origin) return send(403, { error: 'مصدر غير مسموح.' });
        if (req.headers['sec-fetch-site'] && !['same-origin', 'none'].includes(req.headers['sec-fetch-site'])) return send(403, { error: 'مصدر غير مسموح.' });
      }
      if (req.method === 'GET' && STATIC.has(url.pathname)) {
        const [file, type] = STATIC.get(url.pathname);
        res.writeHead(200, { 'Content-Type': type }); res.end(readFileSync(file)); return;
      }
      if (!isReviewer && !tokenOk(req.headers['x-pocol-control'], controlToken)) return send(403, { error: 'افتح الرابط الذي طبعه الخادم في نافذة التشغيل.' });
      if (req.method === 'GET' && url.pathname === '/api/state') return send(200, controller.view());
      if (req.method === 'GET' && url.pathname === '/api/history') return send(200, { cards: controller.timelineCards(), timelineSig: controller.timelineSig() });
      if (req.method === 'GET' && url.pathname === '/api/reviewer/queue') return send(200, { items: controller.reviewerQueue(), status: controller.reviewerStatus() });
      const table = isReviewer ? reviewerPost : browserPost;
      if (req.method === 'POST' && Object.hasOwn(table, url.pathname)) {
        if (!String(req.headers['content-type'] || '').toLowerCase().startsWith('application/json')) return send(415, { error: 'المطلوب application/json.' });
        if (!isReviewer && req.headers.origin !== origin) return send(403, { error: 'طلبات التحكم تتطلب ترويسة Origin من الصفحة المحلية.' });
        const body = await readBody(req);
        return send(200, table[url.pathname](body));
      }
      return send(404, { error: 'المسار غير موجود.' });
    } catch (e) {
      const status = e instanceof BrokerError ? e.status : 500;
      if (!res.headersSent) send(status, { error: redact(status === 500 ? 'خطأ داخلي في الخادم؛ راجع server.log.' : e.message) });
      if (status === 500) controller.log({ event: 'http.error', message: String(e?.message || e) });
    }
  });
  server.requestTimeout = 30_000;
  server.headersTimeout = 15_000;
  return { server, controlToken, reviewerToken };
}

function arg(name) {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : undefined;
}

export function resolveWorkspace(explicit) {
  const candidates = explicit ? [explicit] : [process.cwd(), path.resolve(base, '..', '..', '..', '..', '..')];
  for (const c of candidates) if (existsSync(path.join(c, 'coordination', 'issue-ledger.json'))) return path.resolve(c);
  throw new Error('لم يُعثر على coordination/issue-ledger.json. استخدم --workspace بمسار مساحة العمل.');
}

/**
 * The ONLY settings recovered from a previous private connection.json: the host notifier's
 * codex executable path and existing thread ID. Tokens, URLs, model, permissions and every
 * other field are ignored. Missing or corrupt file -> nothing.
 */
export function savedHostNotifier(connectionFile) {
  const old = readJson(connectionFile, null);
  const h = old && typeof old === 'object' && !Array.isArray(old) ? old.hostNotifier : null;
  const pick = (v) => (typeof v === 'string' && v.length > 0 && v.length <= 1024 ? v : null);
  if (!h || typeof h !== 'object' || Array.isArray(h)) return { codexBin: null, threadId: null };
  return { codexBin: pick(h.codexBin), threadId: pick(h.threadId) };
}

/** Precedence: codexBin = argument > saved; threadId = argument > CODEX_THREAD_ID > saved. Validation is the Notifier's. */
export function resolveNotifierConfig({ argBin = null, argThread = null, env = process.env, saved = { codexBin: null, threadId: null } }) {
  const envThread = typeof env?.CODEX_THREAD_ID === 'string' && env.CODEX_THREAD_ID ? env.CODEX_THREAD_ID : null;
  const codexBin = argBin || saved.codexBin || null;
  const threadId = argThread || envThread || saved.threadId || null;
  return {
    codexBin, threadId,
    source: {
      codexBin: argBin ? 'argument' : saved.codexBin ? 'saved' : 'none',
      threadId: argThread ? 'argument' : envThread ? 'environment' : saved.threadId ? 'saved' : 'none',
    },
  };
}

const sameDir = (a, b, platform) => {
  const x = path.resolve(a), y = path.resolve(b);
  return platform === 'win32' ? x.toLowerCase() === y.toLowerCase() : x === y;
};

/**
 * Validate the previous private connection.json for reuse. Valid only if it is an object for THIS
 * workspace whose url, port, reviewerApi, header and both capabilities are exactly the loopback
 * shapes this server writes, with two different 64-hex capabilities. Returns
 * { status: 'none' | 'valid' | 'invalid', reason, port, controlToken, reviewerToken }.
 */
export function savedConnection(connectionFile, workspace, { platform = process.platform } = {}) {
  if (!existsSync(connectionFile)) return { status: 'none', reason: 'noFile' };
  const c = readJson(connectionFile, undefined);
  const bad = (reason) => ({ status: 'invalid', reason });
  if (!c || typeof c !== 'object' || Array.isArray(c)) return bad('notAnObject');
  if (typeof c.workspace !== 'string' || !sameDir(c.workspace, workspace, platform)) return bad('otherWorkspace');
  if (!Number.isInteger(c.port) || c.port < PORT_MIN || c.port > PORT_MAX) return bad('port');
  if (!TOKEN_RE.test(c.reviewerToken || '')) return bad('reviewerCapability');
  const m = typeof c.url === 'string' ? /^http:\/\/127\.0\.0\.1:(\d{1,5})\/#([0-9a-f]{64})$/.exec(c.url) : null;
  if (!m || Number(m[1]) !== c.port) return bad('url');
  if (m[2] === c.reviewerToken) return bad('sameCapabilities');
  if (c.reviewerApi !== `http://127.0.0.1:${c.port}/api/reviewer/` || c.reviewerHeader !== 'X-PoCol-Reviewer') return bad('reviewerApi');
  return { status: 'valid', reason: null, port: c.port, controlToken: m[2], reviewerToken: c.reviewerToken };
}

/**
 * Decide the port and capabilities for this start. Reuse happens only for a valid saved connection,
 * without --fresh-connection, and when no different --port is requested. An invalid saved file is
 * never reused: new capabilities are created and the reason is reported (fail closed, actionable).
 */
export function resolveConnection({ saved, argPort = null, fresh = false }) {
  const requested = argPort === null || argPort === undefined || argPort === '' ? null : Number(argPort);
  if (requested !== null && (!Number.isInteger(requested) || requested < 0 || requested > PORT_MAX)) throw new Error('قيمة --port غير صالحة.');
  if (!fresh && saved.status === 'valid' && (requested === null || requested === saved.port)) {
    return { port: saved.port, controlToken: saved.controlToken, reviewerToken: saved.reviewerToken, reused: true, notice: null };
  }
  const notice = fresh ? 'طُلب رابط جديد صراحة (--fresh-connection)؛ لم يُعد استخدام الاتصال المحفوظ.'
    : saved.status === 'invalid' ? `ملف الاتصال المحفوظ غير صالح لهذه المساحة (${saved.reason})؛ لم يُعد استخدام أي رمز منه وأُنشئ رابط جديد. يلزم فتح الرابط الجديد في المتصفح.`
    : requested !== null && saved.status === 'valid' ? 'طُلب منفذ مختلف؛ أُنشئ رابط جديد.' : null;
  return { port: requested ?? 0, controlToken: newToken(), reviewerToken: newToken(), reused: false, notice };
}

export async function main() {
  if (Number(process.versions.node.split('.')[0]) < 22) throw new Error('Node.js 22 أو أحدث مطلوب.');
  const workspace = resolveWorkspace(arg('--workspace'));
  let claudeBin = null, claudeNote = null;
  try { claudeBin = findClaude(arg('--claude-bin')); } catch (e) { claudeNote = e.message; }
  const connectionFile = path.join(workspace, 'coordination', 'ui-control', 'connection.json');
  // Host transport: `codex queue --remote unix:// --thread <existing>` only. Path and thread come
  // from the operator's command line, CODEX_THREAD_ID, or the previous private connection.json
  // (read BEFORE it is replaced below) — never from the browser.
  const saved = savedHostNotifier(connectionFile);
  const notifierCfg = resolveNotifierConfig({ argBin: arg('--codex-bin') || null, argThread: arg('--codex-thread') || null, saved });
  const notifier = new Notifier({ codexBin: notifierCfg.codexBin, threadId: notifierCfg.threadId, ws: workspace });
  // Notifications are held until connection.json (which the message points to) is written.
  const controller = new Controller({ workspace, sessionId: arg('--claude-session') || null, claudeBin, notifier, holdNotifications: true });
  controller.acquireService(null);                         // a second service fails here, before any connection is reused
  const conn = resolveConnection({ saved: savedConnection(connectionFile, workspace), argPort: arg('--port') ?? null, fresh: process.argv.includes('--fresh-connection') });
  controller.recover();
  const { server, controlToken, reviewerToken } = createApp({ controller, controlToken: conn.controlToken, reviewerToken: conn.reviewerToken });
  try {
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(conn.port, '127.0.0.1', resolve); });
  } catch (e) {
    controller.stop(); controller.releaseService();
    throw new Error(conn.reused && e.code === 'EADDRINUSE'
      ? `المنفذ المحفوظ ${conn.port} مشغول ببرنامج آخر؛ لم يُبدأ الخادم. أوقف ذلك البرنامج أو ابدأ مع --fresh-connection (سيلزم فتح رابط جديد).`
      : String(e.message));
  }
  const actual = server.address().port;
  controller.acquireService(actual);
  const url = `http://127.0.0.1:${actual}/#${controlToken}`;
  const connection = {
    url, port: actual, pid: process.pid, startedAt: isoNow(), workspace,
    reviewerApi: `http://127.0.0.1:${actual}/api/reviewer/`, reviewerHeader: 'X-PoCol-Reviewer', reviewerToken,
    stateFile: controller.stateFile, logFile: controller.logFile, claudeBin, claudeNote,
    // Keep the operator's chosen inputs even if validation failed this time (e.g. codex.exe moved),
    // so the next plain start reports the same honest error instead of silently forgetting them.
    hostNotifier: { ...notifier.describe(), codexBin: notifier.codexBin || notifierCfg.codexBin, threadId: notifierCfg.threadId, source: notifierCfg.source },
    noteAr: 'ملف خاص بالمنسق المضيف. لا تنشره ولا تضفه إلى Git. وصول الإشعار ليس اتصالًا ولا استلامًا ولا مراجعة؛ الاتصال يكون باستدعاء attach فعليًا.',
  };
  writeJsonAtomic(controller.connectionFile, connection);
  controller.releaseNotifications();
  controller.log({ event: 'server.started', port: actual, pid: process.pid, claudeAvailable: Boolean(claudeBin), notifierConfigured: notifier.configured().ok, connectionReused: conn.reused });
  if (conn.notice) controller.log({ event: 'server.connectionNotReused', notice: conn.notice });
  console.log('\nمنسق PoCol المحلي (M1)\n' + (conn.reused ? 'أُعيد استخدام الرابط المحلي المحفوظ؛ الصفحة المفتوحة تستمر دون إعادة فتح:\n' : 'افتح هذا الرابط المحلي في المتصفح:\n') + url);
  if (conn.notice) console.log('\nتنبيه: ' + conn.notice);
  console.log('\nسجل الخادم: ' + controller.logFile + '\nالحالة المحفوظة: ' + controller.stateFile +
    '\nمعلومات الاتصال الخاصة بالمنسق المضيف: ' + path.join(controller.uiDir, 'connection.json'));
  if (claudeNote) console.log('\nتنبيه: ' + claudeNote + ' (العرض والطابور يعملان؛ تشغيل مهام المؤلف معطل).');
  const nc = notifier.configured();
  console.log(nc.ok ? `\nإشعار المنسق المضيف: codex queue --remote unix:// إلى الخيط القائم …${notifier.threadId.slice(-8)} (المصدر: ${notifierCfg.source.codexBin}/${notifierCfg.source.threadId})`
    : '\nتنبيه: ' + nc.reason + ' (الطلبات تُحفظ وتظهر حالة الإشعار في الصفحة).');
  console.log('\nأبقِ النافذة مفتوحة. Ctrl+C يوقف الخادم ولا يقتل مهمة مؤلف قائمة.\n');
  let closing = false;
  const close = () => {
    if (closing) return; closing = true;
    controller.stop(); controller.releaseService();
    controller.log({ event: 'server.stopped' });
    server.close(); server.closeAllConnections();
  };
  process.on('SIGINT', close); process.on('SIGTERM', close);
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main().catch((e) => { console.error(redact(String(e.message))); process.exitCode = 1; });
}
