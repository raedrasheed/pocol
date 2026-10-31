// Options page: profile management, local wallet, open a site by profile/address/path.
const $ = (id) => document.getElementById(id);
const send = (m) => chrome.runtime.sendMessage(m);
async function refresh() {
  const r = await send({ type: 'profiles.list' });
  $('profiles').replaceChildren(...(r.profiles || []).map((p) => {
    const li = document.createElement('li');
    // A stored profile that no longer validates is listed as invalid (removable, unusable).
    li.textContent = p.invalid ? `${p.name} · Invalid: ${p.invalid} ` : `${p.name} · chainId ${p.chainId} · ${p.trustLevel} · ${p.endpoint} · ${p.forkSchedule.length === 2 ? 'Version 2 from block ' + p.forkSchedule[1][1] : 'Version 1'} · netKey ${p.netKey.slice(0, 18)}…${p.conflict ? ' · network or activation conflict (read-only)' : ''} `;
    const rm = Object.assign(document.createElement('button'), { type: 'button', textContent: 'Delete' });
    rm.onclick = async () => { await send({ type: 'profiles.remove', name: p.name }); refresh(); };
    li.append(rm); return li;
  }));
  const w = await send({ type: 'wallet.status' });
  $('wallet').textContent = !w.exists ? 'No wallet' : w.unlocked ? 'Unlocked' : 'Locked';
  if (w.unlocked) for (const p of (r.profiles || []).filter((x) => !x.invalid)) { const a = await send({ type: 'wallet.address', name: p.name }); if (a.ok) $('wallet').textContent += `\n${p.name}: ${a.address}`; }
}
$('add').onclick = async () => {
  let p; try { p = JSON.parse($('profile').value); } catch { alert('Invalid JSON'); return; }
  const r = await send({ type: 'profiles.add', profile: p }); if (!r.ok) alert('Profile rejected: ' + (r.error.message || r.error)); refresh();
};
$('create').onclick = async () => { const r = await send({ type: 'wallet.create', password: $('pw').value }); if (!r.ok) alert(r.error.message || r.error); refresh(); };
$('unlock').onclick = async () => { const r = await send({ type: 'wallet.unlock', password: $('pw').value }); if (!r.ok) alert('Could not unlock'); refresh(); };
$('lock').onclick = async () => { await send({ type: 'wallet.lock' }); refresh(); };
$('open').onclick = () => { window.open(chrome.runtime.getURL('viewer.html') + '#' + encodeURI($('target').value.trim()), '_blank', 'noopener'); };
refresh();
