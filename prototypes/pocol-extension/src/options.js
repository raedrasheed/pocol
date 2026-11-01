// Options page: profile management, local wallet, open a site by profile/address/path.
const $ = (id) => document.getElementById(id);
const send = (m) => chrome.runtime.sendMessage(m);
async function refresh() {
  const jobs = Object.entries(await chrome.storage.local.get(null)).filter(([k]) => k.startsWith('studio:job:')||k.startsWith('ide:job:')).map(([, v]) => v).sort((a,b) => b.createdAt-a.createdAt);
  $('studioJobs').replaceChildren(...jobs.map((job) => { const li=document.createElement('li'), link=document.createElement('a'); link.href=chrome.runtime.getURL(job.kind==='contract'?'contract.html':'publisher.html')+'#'+job.id; link.textContent=(job.request?.title || 'PoCol operation')+' · '+job.status; li.append(link); return li; }));
  if (!jobs.length) $('studioJobs').textContent = 'No publications yet · لا توجد عمليات نشر بعد';
  const r = await send({ type: 'profiles.list' });
  const publicProfile = (r.profiles || []).find((p) => p.name === 'pocol-public');
  if (publicProfile && !publicProfile.invalid) $('publicForkSchedule').value = JSON.stringify(publicProfile.forkSchedule);
  else if (publicProfile?.invalid) $('publicForkStatus').textContent = 'Saved schedule is invalid. Enter the operator-approved schedule to repair it.';
  $('profiles').replaceChildren(...(r.profiles || []).map((p) => {
    const li = document.createElement('li');
    // A stored profile that no longer validates is listed as invalid (removable, unusable).
    li.textContent = p.invalid ? `${p.name} · Invalid: ${p.invalid} ` : `${p.name} · chainId ${p.chainId} · ${p.trustLevel} · ${p.endpoint} · ${p.forkSchedule.length === 2 ? 'Version 2 from block ' + p.forkSchedule[1][1] : 'Version 1'} · netKey ${p.netKey.slice(0, 18)}…${p.conflict ? ' · network or activation conflict (read-only)' : ''} `;
    const rm = Object.assign(document.createElement('button'), { type: 'button', textContent: 'Delete' });
    rm.onclick = async () => { await send({ type: 'profiles.remove', name: p.name }); refresh(); };
    if(p.name !== 'pocol-public')li.append(rm); return li;
  }));
  const w = await send({ type: 'wallet.status' });
  $('wallet').textContent = !w.exists ? 'No wallet' : w.unlocked ? 'Unlocked' : 'Locked';
  if (w.unlocked) for (const p of (r.profiles || []).filter((x) => !x.invalid)) { const a = await send({ type: 'wallet.address', name: p.name }); if (a.ok) $('wallet').textContent += `\n${p.name}: ${a.address}`; }
}
$('add').onclick = async () => {
  let p; try { p = JSON.parse($('profile').value); } catch { alert('Invalid JSON'); return; }
  const r = await send({ type: 'profiles.add', profile: p }); if (!r.ok) alert('Profile rejected: ' + (r.error.message || r.error)); refresh();
};
$('savePublicForkSchedule').onclick = async () => {
  let forkSchedule;
  try { forkSchedule = JSON.parse($('publicForkSchedule').value); }
  catch { $('publicForkStatus').textContent = 'Invalid JSON. Enter the exact approved schedule.'; return; }
  const r = await send({ type: 'profiles.publicSchedule', forkSchedule });
  if (!r.ok) { $('publicForkStatus').textContent = 'Schedule rejected: ' + (r.error?.message || r.error); return; }
  $('publicForkStatus').textContent = 'Trusted schedule saved. Reopen active Viewer tabs to use it. Wallet identity is unchanged.';
  await refresh();
};
$('create').onclick = async () => { const r = await send({ type: 'wallet.create', password: $('pw').value }); if (!r.ok) alert(r.error.message || r.error); refresh(); };
$('unlock').onclick = async () => { const r = await send({ type: 'wallet.unlock', password: $('pw').value }); if (!r.ok) alert('Could not unlock'); refresh(); };
$('lock').onclick = async () => { await send({ type: 'wallet.lock' }); refresh(); };
$('open').onclick = () => { window.open(chrome.runtime.getURL('viewer.html') + '#' + encodeURI($('target').value.trim()), '_blank', 'noopener'); };
refresh();
