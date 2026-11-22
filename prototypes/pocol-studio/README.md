# PoCol Studio

PoCol Studio is a static, Arabic/English visual editor for websites published to PoCol Mainnet. The intended production origin is `https://studio.pocol.net`.

English is the default interface language and the default content language for new projects. Arabic remains selectable. The first upgrade from the initial Arabic-default release converts only an untouched, unpublished personal starter; edited, imported, linked and published projects remain intact.

The editor creates and previews lightweight pages, imports an existing website folder, stores local project drafts, exports a site ZIP, and submits an explicit publication request to the PoCol Chrome extension. The website never receives a wallet private key. Review and signing happen in an extension-owned page.

## Local checks

This project has no npm dependencies.

```sh
npm test
npm run check
```

To reproduce the downloadable packages from this repository, build and test
`prototypes/pocol-extension` first, then run from the repository root:

```sh
python3 prototypes/pocol-studio/scripts/build-release.py --output-dir /tmp/pocol-studio-release
```

The builder creates the Chrome ZIP, cPanel ZIP, checksum-verified gateway
installer, gateway source ZIP, `release-manifest.json` and `SHA256SUMS.txt`.
It also refreshes the editor's `dist/downloads/PoCol-Studio-IDE-Viewer-<version>.zip` from the
matching extension build. Generated ZIPs are release artifacts, not source.
An optional `--guide /path/to/quick-start.txt` adds the Arabic quick-start.
For separate checkouts, pass `--studio-dir`, `--extension-dir` and
`--gateway-dir` explicitly. ZIP metadata is fixed for reproducible output.
Packaging validates archive integrity; it does not run the component test suites.
The public download (`PoCol-Studio-IDE-Viewer-<version>.zip`) is published only with its ssh-ed25519 release
signature (`deploy/release/README.md`); the cPanel, gateway installer and source ZIPs are the owner's deploy inputs,
not public downloads.

Serve `dist/` with a static HTTP server for development. In the managed Sites environment, use `sites-preview start` with the absolute checkout directory. A preview origin must be explicitly included in the extension's Studio origin allowlist before the publish bridge is usable there.

## Production components

1. **Studio:** upload the contents of `dist/` to the separate cPanel document root for `studio.pocol.net`. Enable its normal HTTPS certificate. The included `.htaccess` disables directory listings and adds browser security headers. An upload by overwrite leaves the files that `dist/` no longer has on the host, still served: after each upload list the document root on the host (`find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ~/studio.pocol.net-files.txt` in cPanel's Terminal) and run `python3 deploy/websites/tools/host_files.py --site studio.pocol.net --listing studio.pocol.net-files.txt`; delete every `DELETE` file and upload every `DIFFERS` or `MISSING` one until it prints `OK` (`deploy/websites/README.md`). Never replace a host `.htaccess` that the tool marks `MERGE`: cPanel writes its PHP handler block (`# php -- BEGIN cPanel-generated handler, do not edit` ... `END`) and any HTTPS redirect there. Back it up, keep those blocks, add `dist/.htaccess` unchanged before or after them, and check a downloaded copy with `--host-copy .htaccess=COPY`; `.user.ini`, `php.ini` and `error_log` are kept.
2. **Extension:** use the matching PoCol Studio extension build. It has a pinned PoCol Mainnet identity, a bounded Studio message bridge, a trusted confirmation page, password-encrypted local signing keys, and restartable publication records. Chrome Web Store submission and approval are separate from this release.
3. **Gateway:** install the separate `deploy/studio` relay on the existing first PoCol node. `https://explorer.pocol.net/studio/rpc` forwards only bounded, allowlisted RPC requests to loopback. It never signs a transaction or holds a wallet key. The existing Explorer endpoint remains separate.

The hosted preview is for inspecting the editor. Neither source publication nor preview deployment configures the user's DNS, installs the production gateway, or proves a successful on-chain publish.

## Site format and behavior

- Maximum 256 files; 1 MiB per file; 4 MiB total; manifest maximum 64 KiB.
- Supported extensions are HTML, CSS, JS/MJS, JSON, PNG, JPEG, WebP, SVG, WOFF2, and TXT.
- File paths follow PoCol's ASCII path rules. The entry point must be an included HTML file.
- Imported JavaScript is preserved for export/publication but not executed in the editor preview. The preview is an opaque sandbox and blocks external requests.
- Project drafts belong to this browser and origin. Export a project JSON to move it between the preview origin and the production domain. Clearing browser data may remove local drafts.
- Public sites are stored as on-chain content, not as server-side PHP or Node applications.
- A website owner can publish a new version at the same contract address. The signer must be authorized by that contract; Studio does not transfer ownership.

## Signing and connection model

The public profile identifies chain `1599846715` and genesis `0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca`. RN means a trusted remote node: the extension checks network identity and content hashes, but does not reexecute the complete chain locally.

For an existing Windows desktop wallet, `identity.pocol-key` is protected with Windows DPAPI under the current user. The provided one-time Windows export helper produces a password-encrypted portable file for local import in the extension. The private key is never sent to Studio or the relay. New users can create an encrypted Studio wallet; viewing sites does not require a wallet. Signing requires a funded PoCol Mainnet account.

Publication is a bounded sequence of transactions. The approval shows its files, target, manifest hash, and worst-case fee cap. Signed bytes are checkpointed before broadcast, and uncertain submissions are resumed using the same transaction rather than silently allocating another nonce. Stored receipts and the current contract state are checked before reporting completion. Browser confirmation is required for a resumed job.

## Sources

- [PoCol website publisher and storage format](https://github.com/raedrasheed/pocol/blob/72cf38f6cdb143aeb41431d2702317db1a0d85ba/prototypes/pocold/src/site.rs)
- [PoCol extension viewer and profile validation](https://github.com/raedrasheed/pocol/blob/72cf38f6cdb143aeb41431d2702317db1a0d85ba/prototypes/pocol-extension/src/sw.js)
- [PoCol desktop key format](https://github.com/raedrasheed/pocol/blob/72cf38f6cdb143aeb41431d2702317db1a0d85ba/prototypes/pocold/src/keyfile.rs)
- [cPanel Domains](https://docs.cpanel.net/cpanel/domains/domains/)
- [Chrome Web Store publishing](https://developer.chrome.com/docs/webstore/publish)

## Release verification limits

Automated protocol, editor, extension, and gateway tests are included with their respective components. The final release notes record the actual runs. Production DNS/HTTPS activation, the Windows DPAPI export on the user's computer, and a live publish from the updated Chrome extension still require verification in that environment. No live blockchain transaction was sent during preparation.
