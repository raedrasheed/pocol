// Send PCOL (EXT-02): the worker builds the transfer from the profile endpoint (send.prepare) and this page shows every
// field it will sign: network, chain ID, genesis, endpoint, sender, recipient, amount, fee, total. Only an explicit
// click on "Confirm and send" after the consent box asks the worker to sign it (send.confirm, once, within two minutes);
// the worker re-checks the network identity and the nonce, signs, submits with eth_sendRawTransaction to the profile
// endpoint and records the hash, and this page then follows the receipt. No key or password is kept here.
const $ = (id) => document.getElementById(id);
const call = async (msg) => { try { return (await chrome.runtime.sendMessage(msg)) || { ok: false, error: { message: 'No answer from the extension' } }; } catch (e) { return { ok: false, error: { message: String(e?.message || e) } }; } };
const errText = (r) => String(r?.error?.message || r?.error || 'Unknown error');
const name = (() => { try { return decodeURIComponent(location.hash.slice(1)) || null; } catch { return null; } })();
let review = null, net = null, tracking = null;

function show(box) { for (const id of ['formBox', 'reviewBox', 'resultBox']) $(id).hidden = id !== box; }
function fatal(text) { $('fatal').hidden = !text; $('fatal').textContent = text || ''; }

async function renderNetwork() {
  net = await call({ type: 'net.status', name });
  const dot = (c) => { $('netDot').className = 'dot ' + c; };
  if (!net.ok) { dot('bad'); $('netText').textContent = errText(net); return false; }
  $('netDetail').textContent = `${net.builtIn ? 'PoCol Mainnet (built-in profile)' : 'Custom profile ' + net.name} · chain ID ${net.chainId} · endpoint ${net.endpoint} · genesis ${net.genesisHash}`;
  if (net.placeholder) { dot('warn'); $('netText').textContent = net.note; return false; }
  if (!net.reachable) { dot('bad'); $('netText').textContent = 'The endpoint is unreachable: ' + (net.error || ''); return false; }
  if (!net.chainOk || !net.genesisOk) { dot('bad'); $('netText').textContent = 'Wrong network, sending is refused: ' + (net.difference || ''); return false; }
  dot('ok'); $('netText').textContent = `Connected: chain ID and genesis match the profile · block ${net.head.height}`;
  return true;
}
async function renderAccount() {
  const st = await call({ type: 'account.status' });
  if (!st.exists) { fatal('No PoCol wallet is saved in this browser yet. Create or import one in Settings.'); $('formBox').hidden = true; return false; }
  $('from').textContent = st.address;
  $('lockState').textContent = st.unlocked ? 'Unlocked' : 'Locked'; $('lockState').className = 'badge' + (st.unlocked ? ' unlocked' : '');
  $('unlockBox').hidden = !!st.unlocked;
  $('review').disabled = !st.unlocked || !(net?.ok && !net.placeholder && net.reachable && net.chainOk && net.genesisOk);
  return st.unlocked;
}
async function refresh() { fatal(''); await renderNetwork(); await renderAccount(); }

$('unlock').onclick = async () => {
  const password = $('password').value; $('password').value = '';
  const r = await call({ type: 'account.unlock', password });
  $('formNote').textContent = r.ok ? '' : errText(r); refresh();
};
$('password').onkeydown = (e) => { if (e.key === 'Enter') $('unlock').click(); };
$('review').onclick = async () => {
  $('formNote').textContent = ''; $('review').disabled = true;
  const r = await call({ type: 'send.prepare', name, to: $('to').value, amount: $('amount').value });
  $('review').disabled = false;
  if (!r.ok) { $('formNote').textContent = errText(r); return; }
  review = r;
  $('rNetwork').textContent = r.builtIn ? `PoCol Mainnet (${r.network}, ${r.trustLevel})` : `${r.network} (custom profile, ${r.trustLevel})`;
  $('rChain').textContent = String(r.chainId); $('rGenesis').textContent = r.genesisHash; $('rEndpoint').textContent = r.endpoint;
  $('rFrom').textContent = r.from; $('rTo').textContent = r.to; $('rAmount').textContent = r.valuePcol;
  $('rFee').textContent = `about ${r.expectedFeePcol}, at most ${r.maxFeePcol} (${r.gas} gas at most ${r.maxFeePerGas} wei per gas)`;
  $('rTotal').textContent = r.totalPcol; $('rBalance').textContent = r.balancePcol;
  $('rNonce').textContent = `${r.nonce} · ${r.gas}`; $('rHash').textContent = r.signingHash;
  $('consent').checked = false; $('confirm').disabled = true; show('reviewBox');
};
$('consent').onchange = () => { $('confirm').disabled = !$('consent').checked; };
$('cancel').onclick = async () => { if (review) await call({ type: 'send.cancel', id: review.id }); review = null; show('formBox'); };
$('confirm').onclick = async (e) => {
  // Only the user's own click on this page signs (an extension page; no site script runs here).
  if (!e.isTrusted || !review || !$('consent').checked) return;
  const id = review.id; review = null; $('confirm').disabled = true; $('cancel').disabled = true;
  const r = await call({ type: 'send.confirm', id });
  $('cancel').disabled = false; show('resultBox');
  // "Not sent" only for a refusal the worker proved (ok:false); an ambiguous node answer comes back as status unknown.
  if (!r.ok) { $('resultText').textContent = 'Not sent: ' + errText(r); $('resultHash').textContent = r.error?.hash || ''; return; }
  $('resultHash').textContent = r.hash;
  $('resultText').textContent = r.status === 'pending' ? 'Sent to the network. Waiting for it to be included in a block…'
    : 'The node gave no clear answer; the transfer may have been sent. Do not send it again until its status below is known.' + (r.note ? ' (' + r.note + ')' : '');
  track(r.hash);
};
const WORDS = { included: 'Included in block', failed: 'Included but failed (reverted) in block', dropped: 'Dropped: another transaction used its nonce; it was not included.', rejected: 'Rejected by the node.' };
function track(hash) {
  clearInterval(tracking); const until = Date.now() + 10 * 60 * 1000;
  const tick = async () => {
    const r = await call({ type: 'txs.refresh' }); const t = (r.transactions || []).find((x) => x.hash === hash);
    if (t?.explorer) { $('explorer').href = t.explorer; $('explorer').hidden = false; }
    if (t && WORDS[t.status]) { clearInterval(tracking); $('resultText').textContent = WORDS[t.status] + (t.blockNumber !== undefined && ['included', 'failed'].includes(t.status) ? ' ' + t.blockNumber : ''); return; }
    if (Date.now() > until) { clearInterval(tracking); $('resultText').textContent = 'Still pending after ten minutes. Its status is kept in the wallet\'s list (toolbar popup, Refresh).'; }
  };
  tracking = setInterval(tick, 3000); tick();
}
$('again').onclick = () => { clearInterval(tracking); $('to').value = ''; $('amount').value = ''; show('formBox'); refresh(); };
$('close').onclick = () => window.close();
if (!name) { fatal('Open Send from the PoCol wallet in the toolbar.'); $('formBox').hidden = true; } else refresh();
