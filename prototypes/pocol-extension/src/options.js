// Options page: profile management, local wallet, open a site by profile/address/path.
const $ = (id) => document.getElementById(id);
const send = (m) => chrome.runtime.sendMessage(m);
async function refresh() {
  const jobs = Object.entries(await chrome.storage.local.get(null)).filter(([k]) => k.startsWith('studio:job:')||k.startsWith('ide:job:')).map(([, v]) => v).sort((a,b) => b.createdAt-a.createdAt);
  $('studioJobs').replaceChildren(...jobs.map((job) => { const li=document.createElement('li'), link=document.createElement('a'); link.href=chrome.runtime.getURL(job.kind==='contract'?'contract.html':'publisher.html')+'#'+job.id; link.textContent=(job.request?.title || 'PoCol operation')+' · '+job.status; li.append(link); return li; }));
  if (!jobs.length) $('studioJobs').textContent = 'No publications yet';
  // Sites W4: jobs of another network, archived on install or update (lib/jobs.js), listed read-only.
  const archived = Object.entries(await chrome.storage.local.get(null)).filter(([k]) => k.startsWith('archive:studio:job:') || k.startsWith('archive:ide:job:')).map(([, v]) => v).sort((a, b) => (b.archivedAt || 0) - (a.archivedAt || 0));
  $('archivedJobs').replaceChildren(...archived.map((job) => { const li = document.createElement('li'); li.textContent = `${job.request?.title || 'PoCol operation'} · ${job.status} · chain ${job.network?.chainId ?? 'unknown'} · ${Object.keys(job.checkpoint?.transactions || {}).length} signed transaction(s) · archived ${new Date(job.archivedAt || 0).toISOString().slice(0, 10)}`; return li; }));
  if (!archived.length) $('archivedJobs').textContent = 'None';
  const r = await send({ type: 'profiles.list' });
  $('profiles').replaceChildren(...(r.profiles || []).map((p) => {
    const li = document.createElement('li');
    // A stored profile that no longer validates is listed as invalid (removable, unusable).
    li.textContent = p.invalid ? `${p.name} · Invalid: ${p.invalid} ` : `${p.name} · chainId ${p.chainId} · ${p.trustLevel} · ${p.endpoint} · ${p.forkSchedule.length > 1 ? p.forkSchedule.slice(1).map(([v, h]) => 'Version ' + v + (v === 3 ? ' (cap raise)' : '') + ' from block ' + h).join(', ') : 'Version 1'} · netKey ${p.netKey.slice(0, 18)}…${p.conflict ? ' · network or fork-schedule conflict (read-only)' : ''} `;
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
$('create').onclick = async () => {
  // F11: require a real password here too (the worker enforces it regardless), and confirm before
  // replacing an existing wallet — creating a new one permanently discards the current key.
  const password = $('pw').value;
  if (typeof password !== 'string' || password.length < 10) { alert('Password must be at least 10 characters.'); return; }
  let r = await send({ type: 'wallet.create', password });
  if (!r.ok && /already exists/.test((r.error && (r.error.message || r.error)) || '')) {
    if (!confirm('A wallet already exists. Creating a new one PERMANENTLY discards the current key and any funds it holds, and the seed cannot be exported. Continue?')) return;
    r = await send({ type: 'wallet.create', password, replace: true });
  }
  if (!r.ok) alert((r.error && (r.error.message || r.error)) || 'Could not create wallet'); refresh();
};
$('unlock').onclick = async () => { const r = await send({ type: 'wallet.unlock', password: $('pw').value }); if (!r.ok) alert('Could not unlock'); refresh(); };
$('lock').onclick = async () => { await send({ type: 'wallet.lock' }); refresh(); };
$('open').onclick = () => { window.open(chrome.runtime.getURL('viewer.html') + '#' + encodeURI($('target').value.trim()), '_blank', 'noopener'); };
refresh();

// ---------------- the PoCol wallet (account.*, EXT-02/03/04/05) ----------------
// The worker holds the key; this page sends the password, the key, the phrase or the wallet file to it once and clears
// the fields. The only key material that comes back is the encrypted backup, which is saved as a file on the user's click.
const call = async (m) => { try { return (await chrome.runtime.sendMessage(m)) || { ok: false, error: { message: 'No answer from the extension' } }; } catch (e) { return { ok: false, error: { message: String(e?.message || e) } }; } };
const errText = (r) => String(r?.error?.message || r?.error || 'Unknown error');
const note = (t) => { $('acctNote').textContent = t; };
function download(box, label) {
  const blob = new Blob([JSON.stringify(box, null, 2) + '\n'], { type: 'application/json' });
  const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(blob), download: `PoCol-Wallet-${box.address.slice(2, 10)}-${label}.json` });
  document.body.append(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(a.href), 10000);
}
async function refreshAccount() {
  const st = await call({ type: 'account.status' });
  $('acctState').textContent = !st.exists ? 'No PoCol wallet in this browser yet.' : `Address ${st.address} · ${st.unlocked ? 'unlocked' : 'locked'}`;
  $('acctDisclosure').hidden = !!st.disclosureAccepted;
  const c = await call({ type: 'connections.list' });
  $('connections').replaceChildren(...(c.connections || []).map((x) => {
    const li = document.createElement('li'); li.textContent = `Viewer tab: site ${x.site} on ${x.network}, account ${x.address} `;
    const b = Object.assign(document.createElement('button'), { type: 'button', textContent: 'Disconnect' });
    b.onclick = async () => { const r = await call({ type: 'connections.revoke', id: x.id }); if (!r.ok) note(errText(r)); refreshAccount(); };
    li.append(b); return li;
  }));
  if (!(c.connections || []).length) $('connections').textContent = c.ok ? 'No site is connected.' : errText(c);
}
$('acctAccept').onclick = async () => { await call({ type: 'account.acceptDisclosure' }); refreshAccount(); };
$('importKind').onchange = () => { const k = $('importKind').value; $('importFile').hidden = k !== 'file'; $('importKey').hidden = k !== 'privateKey'; $('importPhrase').hidden = k !== 'phrase'; };
$('backup').onclick = async () => {
  const password = $('backupPw').value; $('backupPw').value = '';
  const r = await call({ type: 'account.export', password });
  if (!r.ok) { note('Backup refused: ' + errText(r)); return; }
  download(r.backup, 'backup'); note(`Encrypted backup of ${r.address} saved to your downloads. Keep it with its password.`);
};
const fileBase64 = (file) => new Promise((resolve, reject) => {
  if (!file) { reject(new Error('Choose a wallet file')); return; }
  if (file.size > 16384) { reject(new Error('This is not a PoCol wallet file (too large)')); return; }
  const fr = new FileReader(); fr.onerror = () => reject(new Error('The file could not be read'));
  fr.onload = () => { const u8 = new Uint8Array(fr.result); let s = ''; for (const b of u8) s += String.fromCharCode(b); u8.fill(0); resolve(btoa(s)); };
  fr.readAsArrayBuffer(file);
});
$('acctSave').onclick = async () => {
  const password = $('newPw').value, kind = $('importKind').value;
  if (password !== $('newPw2').value) { note('The two passwords differ.'); return; }
  if (password.length < 10) { note('Use a password of at least 10 characters.'); return; }
  let msg;
  try {
    if (kind === 'create') msg = { type: 'account.create', password };
    else if (kind === 'privateKey') msg = { type: 'account.import', kind, privateKey: $('privateKey').value, password };
    else if (kind === 'phrase') msg = { type: 'account.import', kind, phrase: $('phrase').value, password };
    else msg = { type: 'account.import', kind: 'file', fileBase64: await fileBase64($('walletFile').files[0]), filePassword: $('filePw').value || password, password };
  } catch (e) { note(String(e.message || e)); return; }
  const clear = () => { for (const id of ['newPw', 'newPw2', 'privateKey', 'phrase', 'filePw']) $(id).value = ''; $('walletFile').value = ''; };
  let r = await call(msg);
  if (!r.ok && r.error?.code === 'exists') {
    if (!confirm(`Another PoCol wallet (${r.error.saved}) is saved in this browser. Replacing it removes it from this browser; only its backup file can bring it back. Did you back it up, and do you want to replace it?`)) { clear(); note('Nothing was changed.'); return; }
    // Codex review (PR #114): the confirmation covers only the wallet it named; the worker refuses ('exists', a new
    // confirmation) when another Settings tab saved a different wallet meanwhile.
    r = await call({ ...msg, replace: true, replaceAddress: r.error.saved ?? null });
  }
  clear();
  if (!r.ok) { note(errText(r)); refreshAccount(); return; }
  const locked = r.unlocked === false ? ' It stays locked because the wallet was locked meanwhile; unlock it with its password.' : '';
  if (kind === 'create') { download(r.backup, 'new'); note(`New wallet ${r.address} created; its encrypted backup was saved to your downloads. Keep it with its password.${locked}`); }
  else note(`Wallet ${r.address} imported${locked ? '.' + locked : ' and unlocked.'} Make a backup above if you have none.`);
  refreshAccount();
};
refreshAccount();

// EXT-10 (final review, 2026-11-22): the Chrome Web Store build leaves out the native-wallet export helpers
// (scripts/package-store.py STORE_OMIT), so their links are hidden when the files are not in this installation.
export async function hideMissingHelpers(fetcher = fetch) {
  for (const [id, path] of [['helperWin', 'tools/Export-Studio-Wallet.ps1'], ['helperLinux', 'tools/export-studio-wallet.py']]) {
    let present = false;
    try { present = (await fetcher(chrome.runtime.getURL(path), { method: 'HEAD' })).ok; } catch { present = false; }
    if (!present) { $(id).hidden = true; $('helperNote').hidden = false; }
  }
}
hideMissingHelpers().catch(() => {});
