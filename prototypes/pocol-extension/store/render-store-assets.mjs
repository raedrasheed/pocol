// Renders the Chrome Web Store images from the real extension (2026-11-22):
//   screenshots/01-wallet-popup.png, 02-send.png, 03-settings.png, 04-studio-publisher.png, 05-ide-contract.png (1280x800;
//   the store takes at most five; final review 2026-11-22, STORE-02: the toolbar wallet and its Send page lead, since
//   the listing names the item a wallet)
//   icon-128.png        (the store icon: the extension's artwork at 96x96 centred on a transparent 128x128 canvas)
//   promo-small-440x280.png (the small promo tile: the PoCol logo of pocol.net on the site's dark colour)
//
//   node build.mjs && node store/render-store-assets.mjs [DIST] [OUT]      (DIST: dist/, OUT: store/)
//
// The extension is loaded unpacked into Playwright's Chromium, new headless mode (PLAYWRIGHT: the playwright module to import, default
// 'playwright'). Its pages are opened as a user opens them: the Settings page, and the publisher and contract pages
// that https://studio.pocol.net and https://ide.pocol.net open through the extension's content-script bridge. Those two
// origins are answered here by a minimal local page that only speaks the bridge's postMessage protocol (Playwright
// request routing), and every other host name is unresolvable (--host-resolver-rules), so nothing reaches a PoCol or any
// other server: fee reads fail or stay pending, the wallet shows its network as not reachable, and no wallet holds
// funds. The wallet the popup and Send page show is a throwaway made in a temporary browser profile (by the Studio
// publisher's Create) that is deleted afterwards.
//
// The pages show the network the build pins. Before the identity switch (deploy/mainnet/README.md step 7) that is the
// D1 placeholder profile, and the publisher and contract pages show the chain ID the tree carries today, not PoCol
// Mainnet's: re-render from the switched release build before uploading (store/README.md). The script says so when it
// renders a placeholder build.
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url)), EXT = dirname(HERE);
const DIST = resolve(process.argv[2] || join(EXT, 'dist')), OUT = resolve(process.argv[3] || HERE);
const SITE = join(EXT, '..', '..', 'deploy', 'websites', 'pocol.net');
const { chromium } = await import(process.env.PLAYWRIGHT || 'playwright');
const W = 1280, H = 800;
if (!existsSync(join(DIST, 'manifest.json'))) throw new Error(`no built extension at ${DIST}: run node build.mjs first`);
const placeholder = readFileSync(join(DIST, 'lib', 'studio-network.js'), 'utf8').includes('NETWORK_PLACEHOLDER');
mkdirSync(join(OUT, 'screenshots'), { recursive: true });

const bridgePage = (source, reply) => `<!doctype html><html lang="en"><meta charset="utf-8"><title>PoCol</title>
<body style="font:16px system-ui;background:#060e18;color:#e8f0f8;padding:40px">Opening the PoCol extension…<script>
window.bridge=(method,payload)=>new Promise(res=>{const id=Math.random().toString(36).slice(2);
addEventListener('message',function h(e){if(e.data&&e.data.source==='${reply}'&&e.data.id===id){removeEventListener('message',h);res(e.data);}});
postMessage({source:'${source}',id,method,payload},location.origin);});</script></body></html>`;
const SAMPLE_SITE = [
  { path: '/index.html', base64: Buffer.from('<!doctype html><html lang="en"><meta charset="utf-8"><title>My PoCol site</title><link rel="stylesheet" href="style.css"><h1>Hello from PoCol</h1><p>Published with PoCol Studio.</p>').toString('base64') },
  { path: '/style.css', base64: Buffer.from('body{font-family:system-ui;margin:3rem;color:#102030}h1{color:#1488c8}').toString('base64') },
];
// A deployment request of the IDE: a constructor-less contract whose runtime code is a single STOP.
const SAMPLE_CONTRACT = { schema: 1, action: 'deploy', title: 'Greeter', abi: [], args: [], valueWei: '0',
  bytecode: '0x600a600c600039600a6000f3600080fd00000000000000000000' };

const profile = mkdtempSync(join(tmpdir(), 'pocol-store-render-'));
const ctx = await chromium.launchPersistentContext(profile, {
  channel: 'chromium', headless: true, viewport: { width: W, height: H }, deviceScaleFactor: 1, acceptDownloads: true, colorScheme: 'dark',
  args: [`--disable-extensions-except=${DIST}`, `--load-extension=${DIST}`, '--no-proxy-server',
    '--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1'],
});
const outside = [];
ctx.on('request', (r) => { const u = r.url(); if (!/^(chrome-extension|data|blob|about):/.test(u) && !/^https:\/\/(studio|ide)\.pocol\.net\//.test(u)) outside.push(u); });
await ctx.route('https://studio.pocol.net/**', (route) => route.fulfill({ contentType: 'text/html', body: bridgePage('pocol-studio', 'pocol-extension') }));
await ctx.route('https://ide.pocol.net/**', (route) => route.fulfill({ contentType: 'text/html', body: bridgePage('pocol-ide', 'pocol-ide-extension') }));
// The built-in endpoint and every other http(s) request are refused here as well, whatever the resolver does.
await ctx.route(/^https?:\/\/(?!(studio|ide)\.pocol\.net\/)/, (route) => route.abort('blockedbyclient'));
const settle = (page) => page.waitForTimeout(1500);
const shot = async (page, name) => { await page.setViewportSize({ width: W, height: H }); await settle(page); await page.evaluate(() => window.scrollTo(0, 0)); await page.screenshot({ path: join(OUT, 'screenshots', name) }); console.log('wrote', join(OUT, 'screenshots', name)); };
try {
  let [sw] = ctx.serviceWorkers(); if (!sw) sw = await ctx.waitForEvent('serviceworker', { timeout: 20000 });
  const id = new URL(sw.url()).host;
  const settings = await ctx.newPage(); await settings.goto(`chrome-extension://${id}/options.html`);
  await shot(settings, '03-settings.png');

  const studio = await ctx.newPage(); await studio.goto('https://studio.pocol.net/');
  await studio.waitForTimeout(500);
  const opened = ctx.waitForEvent('page', { timeout: 20000 });
  const job = await studio.evaluate((files) => window.bridge('openPublisher', { title: 'My PoCol site', files, entry: '/index.html' }), SAMPLE_SITE);
  if (!job.ok) throw new Error('openPublisher: ' + JSON.stringify(job));
  const publisher = await opened; await publisher.waitForLoadState();
  await shot(publisher, '04-studio-publisher.png');
  await publisher.evaluate(() => { for (const d of document.querySelectorAll('details')) d.open = true; });
  const password = 'store-screenshot-' + Math.random().toString(36).slice(2);
  await publisher.fill('#password', password); await publisher.fill('#passwordConfirm', password);
  const [backup] = await Promise.all([publisher.waitForEvent('download', { timeout: 60000 }), publisher.click('#create')]);
  await backup.delete();
  await publisher.waitForFunction(() => /^0x[0-9a-fA-F]{40}$/.test(document.querySelector('#walletAddress')?.textContent.trim()), null, { timeout: 60000 });

  // The toolbar wallet (popup.html, 380 px wide) after its first-run agreement, unlocked, on a 1280x800 canvas; then
  // the Send page with a recipient and an amount typed (no review: the network is not reachable here).
  const popup = await ctx.newPage(); await popup.setViewportSize({ width: 380, height: 640 });
  await popup.goto(`chrome-extension://${id}/popup.html`); await settle(popup);
  if (await popup.isVisible('#accept')) { await popup.click('#accept'); await settle(popup); }
  if (await popup.isVisible('#unlock')) { await popup.fill('#password', password); await popup.click('#unlock'); }
  await popup.waitForFunction(() => /^0x[0-9a-fA-F]{40}$/.test(document.querySelector('#address')?.textContent.trim()), null, { timeout: 60000 });
  await settle(popup);
  const popupPng = await popup.screenshot({ fullPage: true });
  const canvas = await ctx.newPage(); await canvas.setViewportSize({ width: W, height: H });
  await canvas.setContent(`<html><body style="margin:0;width:${W}px;height:${H}px;background:#060e18;display:flex;align-items:center;justify-content:center">
    <img src="data:image/png;base64,${popupPng.toString('base64')}" style="max-height:${H - 60}px;border:1px solid #253549;border-radius:10px;box-shadow:0 10px 40px rgba(0,0,0,.5)"></body></html>`);
  await canvas.waitForTimeout(300); await canvas.screenshot({ path: join(OUT, 'screenshots', '01-wallet-popup.png') }); console.log('wrote', join(OUT, 'screenshots', '01-wallet-popup.png'));
  const send = await ctx.newPage(); await send.goto(`chrome-extension://${id}/send.html#${encodeURIComponent(await popup.evaluate(() => document.querySelector('#network').value))}`); await settle(send);
  if (await send.isVisible('#unlock')) { await send.fill('#password', password); await send.click('#unlock'); await settle(send); }
  if (await send.isVisible('#to')) { await send.fill('#to', '0x' + '5a'.repeat(20)); await send.fill('#amount', '1.25'); }
  await shot(send, '02-send.png');

  const ide = await ctx.newPage(); await ide.goto('https://ide.pocol.net/');
  await ide.waitForTimeout(500);
  const contractTab = ctx.waitForEvent('page', { timeout: 20000 });
  const contractJob = await ide.evaluate((request) => window.bridge('openContract', request), SAMPLE_CONTRACT);
  if (!contractJob.ok) throw new Error('openContract: ' + JSON.stringify(contractJob));
  const contract = await contractTab; await contract.waitForLoadState();
  await shot(contract, '05-ide-contract.png');

  // The store icon and the small promo tile, from the existing artwork.
  const art = (file) => 'data:image/png;base64,' + readFileSync(file).toString('base64');
  const page = await ctx.newPage();
  await page.setViewportSize({ width: 128, height: 128 });
  await page.setContent(`<html><body style="margin:0;background:transparent"><img src="${art(join(SITE, 'icon-512.png'))}" width="96" height="96" style="display:block;margin:16px"></body></html>`);
  await page.screenshot({ path: join(OUT, 'icon-128.png'), omitBackground: true }); console.log('wrote', join(OUT, 'icon-128.png'));
  await page.setViewportSize({ width: 440, height: 280 });
  await page.setContent(`<html><body style="margin:0;width:440px;height:280px;background:#060e18;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:18px;font-family:system-ui,sans-serif">
    <img src="${art(join(SITE, 'assets', 'pocol-logo.png'))}" width="300" height="85">
    <div style="color:#cfe3f5;font-size:19px;letter-spacing:.02em">Studio · IDE · Viewer</div></body></html>`);
  await page.screenshot({ path: join(OUT, 'promo-small-440x280.png') }); console.log('wrote', join(OUT, 'promo-small-440x280.png'));
} finally {
  await ctx.close().catch(() => {});
  rmSync(profile, { recursive: true, force: true });
}
// Which build the screenshots show (store/README.md: upload only renders whose placeholderBuild is false).
const manifest = JSON.parse(readFileSync(join(DIST, 'manifest.json'), 'utf8'));
const provenance = { version: manifest.version, name: manifest.name, placeholderBuild: placeholder,
  studioNetworkSha256: createHash('sha256').update(readFileSync(join(DIST, 'lib', 'studio-network.js'))).digest('hex'),
  screenshots: ['01-wallet-popup.png', '02-send.png', '03-settings.png', '04-studio-publisher.png', '05-ide-contract.png'] };
writeFileSync(join(OUT, 'screenshots', 'RENDERED-FROM.json'), JSON.stringify(provenance, null, 2) + '\n');
console.log(JSON.stringify({ extensionBuild: DIST, ...provenance, blockedRequests: [...new Set(outside)] }, null, 1));
if (placeholder) console.log('NOTE: this is the pre-identity-switch build (D1 placeholder); re-render from the switched release build before uploading.');
