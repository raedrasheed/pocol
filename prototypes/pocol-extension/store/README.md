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
The owner also decides the item's scope first: the extension is a publishing signer and on-chain site viewer for
PoCol Studio and PoCol IDE, not a general wallet (it shows no balance or transaction history and has no Send); the
listing describes it as it is.

## Why a separate store zip

- The release manifest carries `key`, the owner's public key from `release/manifest-key.pub`, which gives the
  side-loaded release its fixed extension id `gggcoflmedlhlkcdhebmoepmggdnafmp` (`python3 scripts/package-release.py
  --extension-id`). The Chrome Web Store refuses an upload whose manifest has a `key` field ("key field is not allowed
  in manifest").
- `scripts/package-store.py` runs every check of `scripts/package-release.py`, then writes the same files with
  `manifest.json` minus `key` (`tests/package_store_test.mjs`). No other file and no behaviour changes, so the
  store item and the signed side-loaded release are the same build.

## How the extension id is kept

A store item's id is fixed by its first upload. A zip that holds the item's private key as `key.pem` at its root gets
the id of that key (the method Chrome documents for publishing an extension that was already packaged; developer
reports confirm it). So the first upload carries the private key that matches `release/manifest-key.pub`:

```sh
cd prototypes/pocol-extension
npm ci --ignore-scripts
npm run build
python3 scripts/package-store.py --first-upload-key /path/to/owner-manifest-key.pem
```

The packager checks with `openssl pkey` that the key is the private key of `release/manifest-key.pub` and refuses any
other. It writes `releases/PoCol-Studio-IDE-Viewer-<version>-chrome-web-store-first-upload.zip`, which **contains the
private key**: upload it once, from the owner's machine, as the new item's first package, then delete it. Never
publish, sign or commit it. Before submitting for review, check that the item id the dashboard shows is
`gggcoflmedlhlkcdhebmoepmggdnafmp`; if it is not, delete the draft item and do not publish it (the id cannot be changed
afterwards). Every later upload uses the zip without the key:

```sh
npm run package:store      # releases/PoCol-Studio-IDE-Viewer-<version>-chrome-web-store.zip and its .sha256
```

The store zip and its `.sha256` are reproducible from the same `dist/` (the release zip's fixed metadata).

## How existing users keep their wallets

Chrome keeps an extension's storage per extension id and deletes it when the extension is removed.

- **Side-loaded 0.4.1 and older (Load unpacked, no `key`):** their id came from the folder path, so it is not the store
  item's id, and the store version starts with empty storage. Before switching: finish or cancel open publications
  (Settings, Publication history), and keep the encrypted Studio wallet backup (`PoCol-Studio-Wallet.json`, saved when
  the wallet was created; the Windows wallet's export helper `tools/Export-Studio-Wallet.ps1` makes one from a native
  wallet). Install the store version, open a publication from PoCol Studio and import the file under "Encrypted wallet
  file" with its password: the same address comes back. Then remove the old copy.
- **The signed side-loaded release (same id as the store item):** Chrome cannot install the store item while an
  unpacked copy with the same id is loaded. Keep the encrypted backup, remove the unpacked copy (its storage is
  deleted), install from the store and import the backup.
- **The Settings page's "Local wallet"** is for local networks only. It has no export: move anything it holds before
  removing the extension.

pocol.net's extension page should give these steps with the store link (W16, with the release).

## Submission checklist (owner)

1. Identity switch merged (`deploy/mainnet/README.md` step 7) and one extension version for the release in
   `package.json` (the READMEs say 0.5.0; `deploy/release/assemble_release.py` requires it to equal the native
   version).
2. `npm ci --ignore-scripts && npm test` pass; `npm run package` packages the side-loaded release (the owner signs it
   with `deploy/release`).
3. `python3 scripts/package-store.py --first-upload-key …` for the first upload (later: `npm run package:store`).
4. Render the images from the switched build and check `screenshots/RENDERED-FROM.json` says `"placeholderBuild": false`:
   `PLAYWRIGHT_BROWSERS_PATH=… PLAYWRIGHT=/path/to/playwright/index.mjs node store/render-store-assets.mjs`.
   The script loads `dist/` in Chromium, answers studio.pocol.net and ide.pocol.net with a local bridge page and
   blocks every other request, so it contacts no PoCol or other server.
5. Publish `https://pocol.net/privacy.html` (`deploy/websites/pocol.net/privacy.html`) before submitting: the listing
   links it.
6. Fill the dashboard from `listing.md`; choose the remote-code answer (`listing.md`, "Remote code": an owner decision
   against the current policy text) and confirm the data-usage answers; add the support
   contact e-mail the dashboard asks for (not in this repository).
7. Upload, check the item id, submit for review. Then publish the store link, the extension id and the release key
   fingerprint on pocol.net (W16).

Known review points: the package ships the wallet export helpers (`tools/Export-Studio-Wallet.ps1`,
`tools/export-studio-wallet.py`, offered as downloads by the Settings page, never run by the extension) and READMEs
that describe installing with Developer mode (`README-EN.md`, `README-AR.md`, `README-STUDIO.md`); the viewer runs
on-chain website code in the sandboxed page (disclosed in `listing.md`).
