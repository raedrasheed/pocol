// Viewer (browser.md D26): fixes the context, asks the worker to fetch and verify,
// hosts the site in a sandboxed frame, relays bridge text to the worker unparsed,
// shows confirmations outside the frame, tears the frame down on a second load.
const $ = (id) => document.getElementById(id);
const unb64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
const TARGET = /^([A-Za-z0-9_-]{1,32})\/(0x[0-9a-fA-F]{40})(\/[\x21-\x7e]*)?$/;

let ctx = null; // {name, site, sessionId, port, files, frame, current}
const status = (t) => { $('status').textContent = t; };
const fail = (t) => { $('stage').replaceChildren(Object.assign(document.createElement('p'), { className: 'err', textContent: t })); status(''); };

// Confirmations are serialized; each resolves true only by an explicit click on "approve".
// The worker withdraws a confirmation it cancelled (wallet lock, profile or identity change, A06).
const confirms = []; let showing = null;
function confirmUI(q, id) {
  return new Promise((resolve) => { confirms.push({ q, id, resolve }); pump(); });
}
function dismiss(id) {
  const i = confirms.findIndex((c) => c.id === id); if (i >= 0) { confirms.splice(i, 1)[0].resolve(false); return; }
  if (showing && showing.id === id) showing.done(false);
}
// The session ended (revoked or worker lost): every queued or shown confirmation is withdrawn.
function dismissAll() { for (const c of confirms.splice(0)) c.resolve(false); if (showing) showing.done(false); }
// Approve is gated against carried-over/rapid input (F8): a pause with no press (restarted on every
// press, so a burst can't outlast it) AND, for a mouse, a fresh move onto the button (a fixed-spot
// autoclicker holds the mouse still); confirmTx needs a second, independently-armed step.
const ARM_MS = 600;
function pump() {
  if (showing || !confirms.length) return;
  const { q, id, resolve } = confirms.shift();
  const titles = { confirmExternal: 'Open an external link in a new tab', confirmAccounts: 'Grant this site access to your account on this network', confirmTx: 'Sign and send a transaction' };
  $('mtitle').textContent = titles[q.type] || q.type;
  $('mtext').textContent = q.type === 'confirmExternal' ? q.url
    : q.type === 'confirmAccounts' ? `site: ${q.site}\nnetwork: ${q.network}\naccount: ${q.address}`
    : `network: ${q.network} (chainId ${q.chainId})\nsite: ${q.site}\nfrom: ${q.from}\nto: ${q.to}\nvalue (wei): ${q.value}\nmax cost (wei, gas·maxFee + value): ${q.maxCost}\ngas: ${q.gas}\nmaxFeePerGas (wei): ${q.maxFeePerGas}\nmaxPriorityFeePerGas (wei): ${q.maxPriorityFeePerGas}\nnonce: ${q.nonce}${q.creationBurn === undefined ? '' : `\ncreation burn (wei, quoted; burned only within the signed maximum): ${q.creationBurn}\ngas fee at most (wei, gasUsed·maxFee): ${q.gasFeeMax}\nsigned maximum (wei, gas·maxFee; bounds gas + burn together): ${q.signedMax}`}\ndata: ${q.data}\nsigningHash: ${q.signingHash}\nunsigned: ${q.payload}`;
  $('modal').hidden = false; $('modal').dataset.kind = q.type;
  // F8: Approve becomes live only after BOTH (a) ARM_MS with no pointer press anywhere on the dialog —
  // every press during the wait restarts the delay, so a fast burst (the clickjacking "game") can never
  // outlast it — AND (b), for a mouse, a fresh pointer movement onto the Approve button, which a fixed-spot
  // autoclicker (mouse held still) never produces at any cadence. Keyboard (click detail 0) and touch taps,
  // which the sandboxed site cannot drive and which produce no hover, are allowed once the delay passes.
  // confirmTx (moving funds) needs a second, independently-armed click ("Confirm send"). The button is
  // never set `disabled`, so it keeps receiving clicks (a disabled button drops them); the disarmed look
  // is shown via aria-disabled.
  const approve = $('approve'), modal = $('modal');
  const twoStep = q.type === 'confirmTx'; let step = 0; let armTimer = null, timeReady = false, movedOver = false, lastType = '';
  const arm = () => {
    timeReady = false; movedOver = false; approve.setAttribute('aria-disabled', 'true');
    if (armTimer) clearTimeout(armTimer);
    armTimer = setTimeout(() => { if (showing === cur) timeReady = true; }, ARM_MS);
  };
  const onPress = (e) => { lastType = e.pointerType; if (!timeReady) arm(); };               // a press during the wait restarts it
  const onMove = (e) => { if (timeReady && e.pointerType === 'mouse' && e.target === approve) { movedOver = true; approve.removeAttribute('aria-disabled'); } };
  const done = (ok) => {
    if (showing !== cur) return;
    approve.onclick = $('reject').onclick = null;
    modal.removeEventListener('pointerdown', onPress); modal.removeEventListener('pointermove', onMove);
    if (armTimer) clearTimeout(armTimer);
    modal.hidden = true; approve.textContent = 'Approve'; approve.removeAttribute('aria-disabled');
    showing = null; resolve(ok); pump();
  };
  const cur = { id, done }; showing = cur;
  modal.addEventListener('pointerdown', onPress); modal.addEventListener('pointermove', onMove);
  approve.textContent = 'Approve'; arm();
  approve.onclick = (e) => {
    if (!timeReady) { arm(); return; }                                                        // too early, or part of a burst
    if (e.detail > 0 && lastType === 'mouse' && !movedOver) return;                           // a mouse click with no fresh move onto Approve
    if (twoStep && step === 0) { step = 1; approve.textContent = 'Confirm send'; arm(); return; }
    done(true);
  };
  $('reject').onclick = () => done(false);
}

function teardown(reason) {
  const c = ctx.current; if (!c) return;
  ctx.current = null; try { c.channel.port1.close(); } catch {}
  c.iframe.remove(); if (reason) status(reason);
}
async function mount(path, opts = {}) {
  teardown();
  const frame = ++ctx.frame;
  const storage = opts.storage || (await chrome.runtime.sendMessage({ type: 'session.storage', sessionId: ctx.sessionId })).storage || {};
  if (frame !== ctx.frame) return;
  const iframe = document.createElement('iframe');
  iframe.setAttribute('sandbox', 'allow-scripts allow-forms');
  iframe.setAttribute('referrerpolicy', 'no-referrer');
  iframe.id = 'site'; iframe.dataset.frame = String(frame); iframe.src = 'sandbox.html';
  const channel = new MessageChannel(); const cur = { iframe, channel, frame };
  let loads = 0;
  iframe.addEventListener('load', () => {
    loads++;
    if (loads > 1) { teardown('Frame removed: the site attempted to navigate itself (D26)'); return; }
    // Bridge text is forwarded as-is; only the envelope {seq, text} shape is checked here.
    channel.port1.onmessage = (e) => {
      const m = e.data; if (!ctx.current || ctx.current.frame !== frame) return;
      if (!m || typeof m !== 'object' || !Number.isInteger(m.seq) || typeof m.text !== 'string') return;
      ctx.port.postMessage({ seq: m.seq, frame, text: m.text });
    };
    const files = {}; for (const [p, f] of ctx.files) files[p] = { mime: f.mime, data: f.data };
    iframe.contentWindow.postMessage({ type: 'pocol-init', files, path, entry: ctx.entry, storage, notFound: !!opts.notFound, requested: opts.requested, version: ctx.version }, '*', [channel.port2]);
  });
  ctx.current = cur; $('stage').replaceChildren(iframe);
  history.replaceState(null, '', '#' + ctx.name + '/' + ctx.site + (opts.requested || path));
  status(opts.notFound ? '404' : path);
}

// D103 section 5: worker gone or replaced -> tear frames down, announce possible loss, reload only by the user.
function workerLost() {
  dismissAll();
  if (!ctx || ctx.revoked) return; teardown(); ctx.current = null; ctx.frame = Number.MAX_SAFE_INTEGER;
  const p = Object.assign(document.createElement('p'), { className: 'err', textContent: 'The extension worker disconnected; recent writes may not have been saved ' });
  const b = Object.assign(document.createElement('button'), { type: 'button', id: 'reload', textContent: 'Reload' }); b.onclick = () => location.reload();
  p.append(b); $('stage').replaceChildren(p); status('');
}
async function start() {
  const t = TARGET.exec(decodeURI(location.hash.slice(1)));
  if (!t) { fail('Address: #<profile>/<0x…address>[/path]'); return; }
  const [, name, site, path = '/'] = t;
  $('where').textContent = `${name}/${site}`; status('Fetching and verifying…');
  const r = await chrome.runtime.sendMessage({ type: 'open', name, site, path });
  if (!r || !r.ok) { fail('Could not open site: ' + (r && r.error ? (r.error.message || r.error) : 'unknown')); $('trust').textContent = 'Rejected'; return; }
  $('trust').textContent = r.trust.message; $('trust').className = 'badge ' + r.trust.level; $('trust').dataset.level = r.trust.level;
  $('ver').textContent = `version ${r.version} · block ${r.publishedBlock}`;
  const files = new Map(); for (const p of Object.keys(r.files)) files.set(p, { mime: r.files[p].mime, data: unb64(r.files[p].b64) });
  const port = chrome.runtime.connect({ name: 'bridge:' + r.sessionId });
  ctx = { name, site, sessionId: r.sessionId, port, files, frame: 0, current: null, entry: r.entry, version: r.version };
  port.onMessage.addListener(async (m) => {
    if (m.hello !== undefined) { if (ctx.genId && ctx.genId !== m.hello) workerLost(); ctx.genId = m.hello; return; }
    // The port may be gone by the time a withdrawn dialog resolves (revocation, lost worker).
    if (m.ask !== undefined) { const ok = await confirmUI(m.q, m.ask); try { port.postMessage({ answer: m.ask, ok }); } catch {} return; }
    if (m.dismiss !== undefined) { dismiss(m.dismiss); return; }
    // Identity or profile change (browser.md: 4901): the frame is torn down, nothing reloads by itself.
    if (m.revoked) { ctx.revoked = true; dismissAll(); teardown(); ctx.frame = Number.MAX_SAFE_INTEGER; fail(`Session terminated (${m.revoked.code}): ${m.revoked.message}`); return; }
    if (!ctx.current || m.frame !== ctx.current.frame) return; // replies to a torn-down frame are dropped (D99)
    if (m.navigate) {
      const to = m.navigate;
      if (to.notFound || !ctx.files.has(to.path === '' ? ctx.entry : to.path)) await mount(ctx.entry, { notFound: true, requested: to.path });
      else await mount(to.path === '' ? ctx.entry : to.path);
      return;
    }
    ctx.current.channel.port1.postMessage({ seq: m.seq, reply: m.reply });
  });
  port.onDisconnect.addListener(workerLost);
  await mount(r.path, { storage: r.storage, notFound: r.notFound, requested: r.notFound ? path : undefined });
}
start().catch((e) => fail(String(e && e.message ? e.message : e)));
