# Chrome Web Store submission (owner procedure)

This directory holds what the Chrome Web Store item of the extension needs: the listing text and dashboard answers
(`listing.md`), the store icon, the small promo tile and the screenshots, and the script that renders them from the
real extension (`render-store-assets.mjs`). `scripts/package-store.py` makes the upload zip. Nothing here uploads,
signs or publishes anything; every step below is the owner's.

**Not ready to submit yet.** The extension can be packaged only after the identity switch
(`deploy/mainnet/README.md` step 7): until then the built-in profile is the D1 network placeholder and both packagers
refuse the build ("network placeholder"). The committed screenshots were rendered from that pre-switch build
(`screenshots/RENDERED-FROM.json`: `"placeholderBuild": true`); the network chip of the publisher and contract pages
shows the chain ID the tree carries today, not PoCol Mainnet's, so they must be rendered again (step 4) before upload.
The item's scope is the PoCol wallet (`listing.md`, "Single purpose"): create or import a wallet, address, balance,
network and connection status (toolbar popup), Send PCOL with a confirmation page, the transfers sent from the
extension with their status, an encrypted backup at any time, the Studio and IDE confirmations and the on-chain site
viewer. It is not a miner. The owner decides the item name (`listing.md`).

## Why a separate store zip

- The release manifest carries `key`, the owner's public key from `release/manifest-key.pub`, which gives the
  side-loaded release its fixed extension id `gggcoflmedlhlkcdhebmoepmggdnafmp` (`python3 scripts/package-release.py
  --extension-id`). Developer reports say the Chrome Web Store refuses an upload whose manifest has a `key` field
  ("key field is not allowed in manifest"); no current official page states it, so the store zip simply leaves it out.
- `scripts/package-store.py` runs every check of `scripts/package-release.py`, then writes the same files with
  `manifest.json` minus `key` (`tests/package_store_test.mjs`). No other file and no behaviour changes, so the
  store item and the signed side-loaded release are the same build.

## The store item's id: an OWNER DECISION

A store item's id is fixed by its first upload, and Chrome keeps an extension's storage per id. The officially
documented route (developer.chrome.com, manifest `key`: "Package tab ... View public key") goes from the store to the
side-loaded build, not the other way. The two choices, for the owner:

- **A. Recommended: accept the id the store assigns.** Upload the normal store zip (`npm run package:store`, no private
  key in it). After the first upload, copy the item's public key from the dashboard (Package tab, View public key) and,
  if side-loaded builds should share the store item's id from then on, commit it as `release/manifest-key.pub` in a
  later release (`build.mjs` then gives `dist/` the store item's id, and `package-release.py --extension-id` prints it;
  pocol.net publishes that id). Side-loaded installs made before that keep the old id: they are a separate extension,
  and their users move their wallet with an encrypted backup (below). Nothing secret leaves the owner's machine.
- **B. The historical key.pem method (unofficial, not recommended).** An archived Chromium page (not the current
  publishing documentation) describes uploading a zip with the item's private key as `key.pem` at its root, so that the
  store item gets that key's id (`gggcoflmedlhlkcdhebmoepmggdnafmp`). Developer reports mention upload errors for it,
  and that a store id stays reserved even after a draft is deleted, so a failed attempt may make that id unusable.
  It also puts the release private key into an upload. If the owner chooses it anyway:
  `python3 scripts/package-store.py --first-upload-key /path/to/owner-manifest-key.pem --unofficial-key-pem` (the
  packager refuses `--first-upload-key` without that acknowledgement, and checks with `openssl pkey` that the key is
  the private key of `release/manifest-key.pub`). Upload that zip once from the owner's machine, delete it at once, never
  publish, sign or commit it, and check the item id on the dashboard before submitting for review.

Whatever the choice, the owner should opt in to Verified CRX Uploads (developer.chrome.com, "Update your Chrome Web
Store item") with a separate RSA key kept by the owner (its loss takes up to a week to replace), and never reuse an id
for a different item. Every later upload uses the zip without a key:

```sh
cd prototypes/pocol-extension
npm ci --ignore-scripts
npm run build
npm run package:store      # releases/PoCol-Studio-IDE-Viewer-<version>-chrome-web-store.zip and its .sha256
```

The store zip and its `.sha256` are reproducible from the same `dist/` (the release zip's fixed metadata).

## How users keep their wallets (every case)

Chrome deletes an extension's storage when the extension is removed, and an extension with another id is a separate
extension with empty storage. In every case, the supported way to keep a wallet is its **encrypted backup**:

1. In the old copy: open Settings, "PoCol wallet", enter the wallet password and choose "Download encrypted backup"
   (`PoCol-Wallet-<first 8 hex digits of the address>-backup.json`). Finish or cancel open publications first (Settings, Publication history).
   A Studio wallet backup saved earlier (`PoCol-Studio-Wallet.json`) and the Windows helper's export
   (`tools/Export-Studio-Wallet.ps1`) are the same format and work too.
2. Install the store version. If the store item has the same id as the side-loaded copy (choice B), remove the
   side-loaded copy first: Chrome does not install both, and removing it deletes its storage, so step 1 comes first.
3. In the store version: Settings, "PoCol wallet", "An encrypted PoCol wallet file", choose the file, enter its
   password and a password for this browser: the same address comes back (`tests/account_test.mjs`, "EXT-04";
   `tests/wallet_ui_browser.mjs` restores it into a fresh browser profile).
4. Then remove the old copy (if not already removed).

The Settings page's "Local development wallet" is for local networks only and has no export; it never holds an
address on PoCol Mainnet. pocol.net's extension page should give these steps with the store link (W16, with the
release).

## Submission checklist (owner)

1. Identity switch merged (`deploy/mainnet/README.md` step 7) and one extension version for the release in
   `package.json` (the READMEs say 0.5.0; `deploy/release/assemble_release.py` requires it to equal the native
   version).
2. `npm ci --ignore-scripts && npm test` pass; `npm run package` packages the side-loaded release (the owner signs it
   with `deploy/release`).
3. `npm run package:store` (choice A above; choice B only with `--first-upload-key … --unofficial-key-pem`).
4. Render the images from the switched build and check `screenshots/RENDERED-FROM.json` says `"placeholderBuild": false`:
   `PLAYWRIGHT_BROWSERS_PATH=… PLAYWRIGHT=/path/to/playwright/index.mjs node store/render-store-assets.mjs`.
   The script loads `dist/` in Chromium, answers studio.pocol.net and ide.pocol.net with a local bridge page and
   blocks every other request, so it contacts no PoCol or other server.
5. Publish `https://pocol.net/privacy.html` (`deploy/websites/pocol.net/privacy.html`) before submitting: the listing
   links it.
6. Fill the dashboard from `listing.md`: the remote-code answer is "Yes" with the justification and test instructions
   there; confirm the data-usage answers; add the support contact e-mail the dashboard asks for (not in this
   repository).
7. Upload, note the item id (choice A) or check it (choice B), submit for review. Then publish the store link, the
   extension id and the release key fingerprint on pocol.net (W16).

Known review points: the package ships the wallet export helpers (`tools/Export-Studio-Wallet.ps1`,
`tools/export-studio-wallet.py`, offered as downloads by the Settings page, never run by the extension) and READMEs
that describe installing with Developer mode (`README-EN.md`, `README-AR.md`, `README-STUDIO.md`); the viewer runs
on-chain website code in the sandboxed page (disclosed in `listing.md`).
