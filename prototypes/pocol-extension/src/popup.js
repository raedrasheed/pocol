// The toolbar popup (EXT-02): network and connection status, lock state, address, balance, the transfers sent from
// this extension, and the way to Send. Everything comes from the service worker (account.*, net.status, txs.*); the
// popup holds no key. The password typed here goes to the worker once (account.unlock) and is cleared at once.
const $ = (id) => document.getElementById(id);
const call = async (msg) => { try { return (await chrome.runtime.sendMessage(msg)) || { ok: false, error: { message: 'No answer from the extension' } }; } catch (e) { return { ok: false, error: { message: String(e?.message || e) } }; } };
const errText = (r) => String(r?.error?.message || r?.error || 'Unknown error');
let selected = null, status = null;
// Final review (2026-11-22): refreshes overlap (the agreement's refresh still reads the built-in profile when the user
// picks another one). Only the latest refresh may write the page; an earlier one that answers later is dropped, so the
// popup never shows the status, balance or list of a profile that is no longer selected.
let generation = 0;
const current = (gen) => gen === generation;

function dot(state) { $('netDot').className = 'dot ' + state; }
function shortHash(h) { return typeof h === 'string' && h.length > 20 ? h.slice(0, 10) + '…' + h.slice(-8) : String(h ?? ''); }
function age(sec) { return sec < 120 ? `${sec} s ago` : sec < 7200 ? `${Math.round(sec / 60)} min ago` : `${Math.round(sec / 3600)} h ago`; }

async function loadNetworks() {
  const r = await call({ type: 'profiles.list' });
  const list = (r.profiles || []).filter((p) => !p.invalid);
  const builtIn = status?.network || 'pocol-public';
  // The profile list is read on every refresh (a profile added or removed in Settings shows at once); the choice is
  // kept while its profile exists.
  let saved = selected; if (saved === null) try { saved = localStorage.getItem('pocol:popup:network'); } catch { /* storage may be unavailable */ }
  selected = list.some((p) => p.name === saved) ? saved : builtIn;
  $('network').replaceChildren(...list.map((p) => Object.assign(document.createElement('option'), { value: p.name, textContent: `${p.name === builtIn ? 'PoCol Mainnet (built-in)' : p.name} · chain ${p.chainId}`, selected: p.name === selected })));
}

async function renderNetwork(gen) {
  dot(''); $('netText').textContent = 'Checking…'; $('netDetail').textContent = '';
  const r = await call({ type: 'net.status', name: selected });
  if (!current(gen)) return null;
  if (!r.ok) { dot('bad'); $('netText').textContent = errText(r); return r; }
  const detail = [`Chain ID ${r.chainId}`, `endpoint ${r.endpoint}`, `genesis ${shortHash(r.genesisHash)}`];
  if (r.placeholder) { dot('warn'); $('netText').textContent = 'Not connected: this build pins a placeholder, not PoCol Mainnet'; $('netDetail').textContent = detail.join(' · ') + '\n' + r.note; return r; }
  if (!r.reachable) { dot('bad'); $('netText').textContent = 'Endpoint unreachable'; $('netDetail').textContent = detail.join(' · ') + '\n' + (r.error || ''); return r; }
  if (!r.chainOk || !r.genesisOk) { dot('bad'); $('netText').textContent = 'Wrong network: the endpoint is not this profile\'s network (sending is refused)'; $('netDetail').textContent = detail.join(' · ') + '\n' + (r.difference || ''); return r; }
  const stale = r.head && r.head.ageSec > 600;
  dot(stale ? 'warn' : 'ok');
  $('netText').textContent = `Connected · chain ID and genesis match · block ${r.head.height}, ${age(r.head.ageSec)}${stale ? ' (no new block for a while)' : ''}`;
  $('netDetail').textContent = detail.join(' · ');
  return r;
}

async function renderAccount(net, gen) {
  const st = await call({ type: 'account.status' });
  if (!current(gen)) return;
  status = st;
  $('disclosure').hidden = !!status.disclosureAccepted;
  $('noWallet').hidden = !!status.exists; $('haveWallet').hidden = !status.exists;
  // Codex review (PR #114): the Studio publisher and IDE contract tabs decrypt their own copy of the key, so the worker
  // can be locked while such a tab is not. The popup knows only its own wallet's state ("Locked here"), and its Lock,
  // which signals those tabs (sw.js account.lock, lib/vault-guard.js), stays available whatever that state is.
  $('lockState').textContent = !status.exists ? 'No wallet' : status.unlocked ? 'Unlocked' : 'Locked here';
  $('lockState').className = 'badge' + (status.unlocked ? ' unlocked' : '');
  $('unlockBox').hidden = !status.exists || !!status.unlocked;
  $('lock').hidden = !status.exists;
  $('lock').textContent = status.unlocked ? 'Lock' : 'Lock all tabs';
  $('lockHint').hidden = !status.exists || !!status.unlocked;
  if (!status.exists) return;
  $('address').textContent = status.address;
  $('note').textContent = '';
  if (!status.disclosureAccepted) { $('balance').textContent = '—'; return; }
  const b = await call({ type: 'account.balance', name: selected });
  if (!current(gen)) return;
  if (!b.ok) { $('balance').textContent = '—'; $('note').textContent = errText(b); }
  else if (b.placeholder) { $('balance').textContent = '—'; $('note').textContent = b.note; }
  else $('balance').textContent = b.pcol;
  const link = b.ok && b.explorer;
  $('explorer').hidden = !link; $('explorerNote').hidden = !link; if (link) $('explorer').href = link;
  $('send').disabled = !net?.ok || !!net.placeholder || !net.reachable || !net.chainOk || !net.genesisOk;
}

const STATUS_WORDS = { sending: 'sending', pending: 'pending', unknown: 'outcome unknown', included: 'included', failed: 'failed (included, reverted)', dropped: 'dropped (not included)', rejected: 'rejected by the node' };
async function renderTxs(refresh = false, gen = generation) {
  const r = await call({ type: refresh ? 'txs.refresh' : 'txs.list' });
  if (!current(gen)) return;
  const list = (r.transactions || []).filter((t) => !status?.address || t.from === status.address).slice(0, 10);
  $('txs').replaceChildren(...list.map((t) => {
    const li = document.createElement('li');
    li.textContent = `${STATUS_WORDS[t.status] || t.status} · ${t.network} · to ${shortHash(t.to)} · ${formatWei(t.value)} · ${shortHash(t.hash)}${t.blockNumber !== undefined ? ' · block ' + t.blockNumber : ''}`;
    return li;
  }));
  if (!list.length) $('txs').replaceChildren(Object.assign(document.createElement('li'), { className: 'muted', textContent: r.ok ? 'None yet' : errText(r) }));
}
function formatWei(v) {
  try { const n = BigInt(v), w = 10n ** 18n, f = (n % w).toString().padStart(18, '0').replace(/0+$/, ''); return (n / w).toString() + (f ? '.' + f : '') + ' PCOL'; } catch { return String(v); }
}

async function refresh(refreshTxs = false) {
  const gen = ++generation;
  const st = await call({ type: 'account.status' });
  if (!current(gen)) return;
  status = st;
  await loadNetworks();
  if (!current(gen)) return;
  const net = status.disclosureAccepted ? await renderNetwork(gen) : (() => { dot('warn'); $('netText').textContent = 'Waiting for your agreement below'; return null; })();
  if (!current(gen)) return;
  await renderAccount(net, gen);
  if (!current(gen)) return;
  await renderTxs(refreshTxs && status.disclosureAccepted, gen);
}

$('network').onchange = () => { selected = $('network').value; try { localStorage.setItem('pocol:popup:network', selected); } catch { /* not kept */ } refresh(); };
$('accept').onclick = async () => { const r = await call({ type: 'account.acceptDisclosure' }); if (!r.ok) $('netText').textContent = errText(r); refresh(); };
$('setup').onclick = () => chrome.runtime.openOptionsPage();
$('settings').onclick = () => chrome.runtime.openOptionsPage();
$('unlock').onclick = async () => {
  const password = $('password').value; $('password').value = '';
  const r = await call({ type: 'account.unlock', password });
  if (!r.ok) { $('note').textContent = errText(r); return; }
  refresh();
};
$('password').onkeydown = (e) => { if (e.key === 'Enter') $('unlock').click(); };
$('lock').onclick = async () => {
  const r = await call({ type: 'account.lock' });
  await refresh();
  $('note').textContent = r.ok ? 'Locked. Open Studio publisher and IDE contract tabs lock too.' : errText(r);
};
$('refresh').onclick = () => refresh(true);
$('send').onclick = () => chrome.tabs.create({ url: chrome.runtime.getURL('send.html') + '#' + encodeURIComponent(selected) });
refresh(true);
