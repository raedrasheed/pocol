# Chrome Web Store submission (owner procedure)

This directory holds what the Chrome Web Store item of the extension needs: the listing text and dashboard answers
(`listing.md`), the store icon, the small promo tile and the screenshots, and the script that renders them from the
real extension (`render-store-assets.mjs`). `scripts/package-store.py` makes the upload zip. Nothing here uploads,
signs or publishes anything; every step below is the owner's.

**Packaging is possible since the identity switch** (`deploy/mainnet/README.md` step 7, on this branch): the built-in
profile pins PoCol Mainnet and both packagers accept the build. Before it, the D1 network placeholder made them refuse
("network placeholder"). The committed screenshots (final review, 2026-11-22, STORE-02) were rendered from the switched
0.5.0 build (`screenshots/RENDERED-FROM.json`: `"placeholderBuild": false`), with every network request blocked, so
the popup and Send page show PoCol Mainnet's endpoint as unreachable; the owner may render them again against the live
network (step 4) before upload. Submission still waits for the owner's steps below.
The item's scope is the PoCol wallet (`listing.md`, "Single purpose"): create or import a wallet, address, balance,
network and connection status (toolbar popup), Send PCOL with a confirmation page, the transfers sent from the
extension with their status, an encrypted backup at any time, the Studio and IDE confirmations and the on-chain site
viewer. It is not a miner. The owner decides the item name (`listing.md`).

Owner decisions of 2026-11-23 ([OWNER-DECISIONS-2026-11-23.md](../../../development/decisions/OWNER-DECISIONS-2026-11-23.md)):
the store item keeps the id the store assigns (D2, below); the on-chain site viewer stays in the store build and the
remote-code answer is "Yes" (D3, `listing.md`); only Studio and the IDE connect to the wallet, and websites opened in the
viewer get no account on PoCol Mainnet in this release (D1).

## Why a separate store zip

- The release manifest carries `key`, the owner's public key from `release/manifest-key.pub`, which gives the
  side-loaded release its fixed extension id `gggcoflmedlhlkcdhebmoepmggdnafmp` (`python3 scripts/package-release.py
  --extension-id`). Developer reports say the Chrome Web Store refuses an upload whose manifest has a `key` field
  ("key field is not allowed in manifest"); no current official page states it, so the store zip simply leaves it out.
- `scripts/package-store.py` runs every check of `scripts/package-release.py`, then writes the same files with
  `manifest.json` minus `key`, except the side-loading material (EXT-10): no `tools/` export helpers, no
  `README-STUDIO.md`, and the store-install READMEs of `install/` (`tests/package_store_test.mjs`). The code of the
  store item and of the signed side-loaded release is the same build.

## The store item's id: the id the store assigns (owner decision 2026-11-23 D2)

A store item's id is fixed by its first upload, and Chrome keeps an extension's storage per id. The officially
documented route (developer.chrome.com, manifest `key`: "Package tab ... View public key") goes from the store to the
side-loaded build, not the other way. The owner decided on 2026-11-23 to accept the id the Chrome Web Store assigns
("قبول المعرّف الذي يمنحه Chrome Web Store"):

- **The route taken: the id the store assigns.** Upload the normal store zip (`npm run package:store`, no private key
  in it). The store item then has its own id, not the side-loaded release's `gggcoflmedlhlkcdhebmoepmggdnafmp`, so the
  two are separate extensions with separate storage, and users move their wallet between them with an encrypted backup
  (below). Nothing secret leaves the owner's machine. After the first upload, note the item's id; pocol.net publishes
  it with the store link (W16). The dashboard's public key (Package tab, View public key) could later be committed as
  `release/manifest-key.pub`, so that side-loaded builds share the store item's id (`build.mjs` then gives `dist/` that
  id, and `package-release.py --extension-id` prints it); that is a later owner decision, and until it side-loaded
  releases keep `gggcoflmedlhlkcdhebmoepmggdnafmp`.
- **Not used: the historical key.pem method.** An archived Chromium page (not the current publishing documentation)
  describes uploading a zip with the item's private key as `key.pem` at its root, so that the store item gets that
  key's id. It is unofficial, developer reports mention upload errors and ids that stay reserved after a failed
  attempt, and it puts the release private key into an upload; the owner did not choose it. `scripts/package-store.py`
  still refuses `--first-upload-key` without the explicit `--unofficial-key-pem` acknowledgement
  (`tests/package_store_test.mjs`), so the default store zip never holds a key.

The owner should also opt in to Verified CRX Uploads (developer.chrome.com, "Update your Chrome Web
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
2. Install the store version. Its id is the one the store assigned (D2), not the side-loaded copy's, so Chrome keeps
   both installed side by side, each with its own storage, and both run on Studio and the IDE until step 4. (Only if a later release gave side-loaded builds the store
   item's id would Chrome refuse to install both: then remove the side-loaded copy first, after step 1, since removing
   it deletes its storage.)
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
3. `npm run package:store` (the store-assigned id, D2; no `--first-upload-key`).
4. Render the images from the switched build and check `screenshots/RENDERED-FROM.json` says `"placeholderBuild": false`:
   `PLAYWRIGHT_BROWSERS_PATH=… PLAYWRIGHT=/path/to/playwright/index.mjs node store/render-store-assets.mjs`.
   The script loads `dist/` in Chromium, answers studio.pocol.net and ide.pocol.net with a local bridge page and
   blocks every other request, so it contacts no PoCol or other server.
5. Publish `https://pocol.net/privacy.html` (`deploy/websites/pocol.net/privacy.html`) before submitting: the listing
   links it.
6. Fill the dashboard from `listing.md`: the remote-code answer is "Yes" with the justification and test instructions
   there; confirm the data-usage answers; add the support contact e-mail the dashboard asks for (not in this
   repository).
7. Upload, note the item id the store assigned, submit for review. Then publish the store link, the store item's id
   (beside the side-loaded release's) and the release key fingerprint on pocol.net (W16).

Known review points: the viewer runs on-chain website code in the sandboxed page (disclosed in `listing.md`; it stays
in the store build, owner decision 2026-11-23 D3, so a longer review is expected). EXT-10
(final review, 2026-11-22): the store zip no longer ships the wallet export helpers (`tools/`) or the Developer-mode
READMEs; `scripts/package-store.py` writes the store-install texts of `install/` as `README-EN.md` and `README-AR.md`,
leaves out `README-STUDIO.md`, and refuses a store text that says "Load unpacked", "Developer mode" or
"ExecutionPolicy Bypass" (`tests/package_store_test.mjs`). The Settings page hides its helper links when the helpers
are not installed. The side-loaded release zip is unchanged.
