# Deployed website snapshots (2026-11-01)

These folders record the public document roots of `pocol.net`, `ide.pocol.net` and `explorer.pocol.net` as they were
deployed for the retired network, taken from `websites/` in the release asset `PoCol-Complete-Handoff-2026-11-01.zip`
([release `handoff-2026-11-01`](https://github.com/palcertcom/pocol/releases/tag/handoff-2026-11-01), SHA-256
`b9a3885a630bd7f32270488f741b68a5c923e3fc48c1478499ceb887c520d2f3`). They are not editable sources: the original IDE
TypeScript project and the main-site sources are not in this repository. `studio.pocol.net` is built from
`prototypes/pocol-studio/dist`, which was updated to the same deployed files. The sites are rebuilt for the new
network before its launch (development/decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md).

Removed for the new network (2026-11-13): the old network's upgrade notes (`NETWORK-UPDATE-*.txt`,
`README-MAINNET-UPDATE-AR.txt`), its invitation (`pocol.net/downloads/network.pocol.json`), the retained-download
manifest and the checksums of the superseded 0.3.x and 0.4.0 packages. Each `SHA256SUMS.txt` was regenerated for the
files that remain. Until the rebuild, pages that link the removed invitation or the old packages are stale.

## Launch decisions (owner, 2026-11-16)

- **Languages.** The sites launch in English and Arabic. pocol.net offers exactly `en` and `ar` (every page's
  language selector, `locales.js`, `about-locales.js`; `app.js` falls back to English for any other requested or stored
  language). The 43 other translations were translations of the retired network's text and would not have followed the
  English and Arabic rebuild, so they were removed from the served files; they stay in Git history (snapshot 0df6f62).
  Studio is already English and Arabic. The explorer, the IDE and the extension UI stay English at launch (their
  bundles have no sources). `tests/test_launch_decisions.py` checks the set.
- **Explorer gateway host.** The explorer gateway, its `/rpc`, and the Studio relay `/studio/*` are served at
  `explorer.pocol.net`, which replaces the old PHP site; the owner moves its DNS record at launch. The extension's host
  permission and endpoint, the IDE `rpcUrl` (`site_identity.py --rpc-url`), the explorer's named gateway host
  (`--gateway-host`), the Studio app and its CSP `connect-src`, the reward page's upstream and the Studio gateway
  installer all name `explorer.pocol.net`; `gateway.pocol.net` is named nowhere. **Done (W8):** the gateway host
  serves the explorer snapshot as static files (Caddy `file_server`, with the CSP and headers its `.htaccess` set) and
  answers its same-origin reads `/api/rpc.php`, `/api/health.php`, `/api/miner-locations.php` and
  `/api/transactions.php` in the gateway process (`deploy/rewards/explorer_api.py`, with the checks of the PHP relays
  it replaces); pocol.net's network summary (`pocol.net/api/network.php`, `POCOL_SUMMARY_RPC`) keeps reading the same
  `/api/rpc.php`. `deploy/explorer` installs the gateway service, the static copy and the Caddy site (to which the
  Studio installer adds its route). The PHP relays, both `.htaccess` files and `explorer-config.example.php` are
  retired: the explorer snapshot holds no server code, and `site_identity.py --check` refuses any.
- **The fee faucet.** Decided 2026-11-16 as "no faucet on mainnet"; superseded on 2026-11-18: the fee faucet
  (`deploy/faucet`) runs on mainnet with the owner's limits. Its claim page is `pocol.net/faucet.html` (with
  `faucet.js` and `faucet.css`), linked from the fourth "Get your first fee" card of the home page's Get started
  section. The browser calls the faucet itself, so the faucet's per-client limit counts each visitor's IP address rather
  than this host: `GET https://explorer.pocol.net/faucet/status` for the amount and the transfers left today (the button is
  disabled at 0), and `POST https://explorer.pocol.net/faucet/claim` with the JSON body `{"address": ...}` after the
  page checks the address format. Those routes and the faucet's `allowOrigin` of exactly `https://pocol.net` are configured
  with the faucet and the gateway host, not in these snapshots. Every faucet error carries a `code`; the page shows its own English or Arabic text for
  each of the 21 codes (falling back on the HTTP status when an answer has none) and never inserts server text as
  HTML. An `unknown_outcome`, an unreadable success answer and a claim that got no answer tell the visitor to check
  the address in the explorer before trying again. The page pins no chain ID: the faucet pins its own network.
  Answers over 16 KiB are not read to the end and count as unreadable.
  `tests/test_launch_decisions.py` (`FaucetPage`) checks that, tests and READMEs aside, the only files under
  `deploy/websites` and `prototypes/pocol-studio/dist` that name the faucet are the claim page files, the home page (its card link and keys),
  `locales.js` (the card keys), `SHA256SUMS.txt` (the three page files) and the privacy policy (`privacy.html`,
  `privacy.js`, which name no faucet URL), and that the only faucet URLs are those two
  routes (the IDE's third-party bundle is checked for faucet links only); `tests/browser_security.mjs` runs the page against stubbed answers.
  pocol.net's Content-Security-Policy (`pocol.net/.htaccess`, 2026-11-22) has `connect-src 'self'
  https://explorer.pocol.net`; without that host every claim fails as unreachable.

- **Privacy policy (2026-11-22).** `pocol.net/privacy.html` (English static text, Arabic through `privacy.js`, like the
  faucet page; `privacy.css`) is the privacy policy of the Chrome extension and the four sites; the Chrome Web Store
  listing links it (`prototypes/pocol-extension/store/listing.md`), and every page's footer links it (`privacyLink` in
  `locales.js`). It states only what the repository's code does: keys and password stay in the extension; the extension
  contacts only `https://explorer.pocol.net/studio/rpc` or a loopback node; the faucet keeps address, day and
  transaction, and counts claims per IP (IPv6 /64) in memory for the current UTC day; the gateway, the Studio relay and
  the faucet write no request log and the Caddy sites enable no access log; the explorer loads Google Fonts; the shared
  host may keep its own access logs. `tests/test_privacy_page.py` checks the page against `privacy.js` and each of those
  statements against the code it describes (a change there fails the test until the policy follows), and
  `tests/browser_security.mjs` renders it in both languages at phone width. The owner reviews the text before the page
  is published.

Left out of Git and kept in the release asset:
- the download archives (`*.zip`): the old native packages and the extension packages;
- a partial upload left by an earlier transfer (a hidden `downloads/.PoCol-Mainnet-*-Linux.zip.*` file). It is not
  served (HTTP 404).

The `.sha256` and `SHA256SUMS.txt` files that are kept list the archive hashes.

## Identity and edits (`tools/site_identity.py`)

The sources of the explorer and the IDE are not available (owner answer Q1, 2026-11-14), so every change to their
bundles is an anchored edit made by `tools/site_identity.py`. `site-identity.json` declares the identity the snapshots
pin (chain ID, genesis hash and the GenesisSpec that proves both, the display name, the IDE's RPC URL, the explorer's
seed list and gateway host, and the source links). Each anchor is a literal text that must occur exactly its count of
times, and no copy of an identity value may sit outside the anchors, so a moved anchor fails instead of leaving an old
copy behind. Every run checks the tree it would write the same way (after `--edits` too), so the tool never writes a
tree that its own `--check` refuses.

```sh
python3 -B deploy/websites/tools/site_identity.py --check            # what CI runs (.github/workflows/websites.yml)
python3 -B deploy/websites/tools/site_identity.py --to NEW.json --gateway-host HOST --commit SHA   # the switch (W15, W16)
python3 -B deploy/websites/tools/site_identity.py --edits EDITS.json  # anchored bundle edits (W7, W14)
python3 -B deploy/websites/tools/site_identity.py --download deploy/websites/pocol.net/downloads/NAME=SHA256   # a kept-out line
python3 -B deploy/websites/tools/site_identity.py --drop-download deploy/websites/pocol.net/downloads/NAME     # withdraw one
```

Sites W7 (the explorer's v3 membership, light refresh and Registry decoding) is `tools/explorer-w7-edits.json`, applied
once with `--edits`: the explorer counts and labels members from one `pocol_getMembers` page of 256 (Held, Earning
bond, Tombed; the Miners tab pages by 256 while it is open), shows each account's `pocol_getMember` record (state,
bond as x/B PCOL with B the record's `bondTargetWei`, proposer flag, reward address, removal check, refund height),
refreshes with `full=false` blocks (the latest transactions by hash, the account activity one full block per request),
and decodes the eight Registry calls at `0x…c0c001`, reading a `registerReplacing` or `removeInactive` member at the
head and saying so. Its tests are in `tests/README.md`.

Sites W14 (site texts) is `tools/w14-text-edits.json`, applied once with `--edits`: the retired activation texts
("Options → PoCol Mainnet activation", `[[1,"0"],[2,"15000"]]`) are replaced by "the release pins the network; there is
no activation setting", every in-place extension update (replace the files in the same folder, Reload) by README-IDE.md's
order without version numbers (export the wallet, Remove the old version, verify the signature, Load unpacked, compare
the extension ID with pocol.net, import the wallet), DEPLOY-README-AR.txt sections 2 and 4 are rewritten for a fresh
network, and the IDE's download button names the package it links (0.4.1, it said 0.4.0). Step 7 of the mainnet runbook moved that button and its link to 0.5.0 with the identity switch, recorded as the anchored edits `tools/step7-release-edits.json` on top of W14's (the W14 test undoes and redoes both); `tools/release-pending-edits.json` then replaced that link with `pocol.net/chrome-extension.html#verify-release` (SITE-01, below). `--check` also refuses an
extension version label that is not package.json's (a capability message "X or later" is exempt) and a native package
name that is not `deploy/release/assemble_release.py`'s at `deploy/standalone/package.py`'s version. No retired name is
exempt (`LEGACY_NATIVE` is empty): the retired `-Linux.zip` left it with its last link on 2026-11-22, and the retired
`PoCol-Mainnet-0.4.0-Windows.zip`, which the pages linked, labelled as the retired network's, under an earlier decision
to keep it until the signed release (W16), left it on 2026-11-23, when the owner's request for the pocol.net deployment
(item 4: no Testnet and no 0.4.x release in the deployment content) replaced that decision. `--download PATH=SHA256` (repeatable) writes the `SHA256SUMS.txt` line of a release file kept
out of Git (an archive, its `.sig` or `.sha256`), adds its name to the site's `keptOut` list in `site-identity.json`
and drops the kept-out lines (and names) of other versions of the same archive family and their unversioned copies; a
tracked file it supersedes must be removed first. Only the lines `keptOut` names are carried for files that are not in
the snapshot, so a tracked file that is deleted (a `.sha256`) loses its line and `--check` fails until a run drops it.
`--drop-download PATH` (repeatable) withdraws a kept-out file: its `keptOut` name and its `SHA256SUMS.txt` line go, and
the copy on the host is then deleted.

Site truth (owner decision 4, 2026-11-18: no text states anything untrue or presents the public launch as prevented or
conditional). The explorer bundle's installer step names `deploy/explorer/install_explorer_gateway.py --site DIR` and
its gateway note says that the gateway listens on 127.0.0.1:8088 behind Caddy (`tools/site-truth-edits.json`, applied
once with `--edits` after W7). pocol.net, DEPLOY-README-AR.txt and the Studio app were edited directly. The status page
reports the open gates and says that the node's readiness report is node-local and stops nothing. Since
OWNER-DECISIONS-2026-11-20 (D1, D4 to D6) it describes PoCol Mainnet as the public network (launched without a separate
rehearsal, admitted in scope public-network with the owner's R13, R21 and R26 waivers reported) and names the launch
approval fields (`publicLaunchApproved`, `launchApprovalReference`, `publicLaunchReady`, `launchBlockers`) apart from
`technicalReadiness`, as the node computes them; it states no value of either, and `tests/test_launch_decisions.py`
refuses the old experimental-admission text, a fixed readiness value and a launch after a closed rehearsal in the
served files and in the clients' user interface sources. The Studio note and the extension's Options page also call
PoCol Mainnet the public network. The retired
network's upgrade, migration and Android-update texts are replaced: nothing carries over to the new network. The
static source lists are the guide's, and the README, V3 program and CP links pin main at `4db2652`. No site links
or names an extension package before the signed release (SITE-01, below); the IDE's "0.4.0 or later" messages change
with the signed release (W16); the extension
ID and the release key fingerprint are in the snapshot (2026-11-22, below); they are public once the owner approves
their early publication and uploads it. Since step 7
pocol.net publishes the extension ID and the release key fingerprint users compare (the extension guide,
`chrome-extension.html` and the home page's IDE help, in English and Arabic; `tests/test_w14_texts.py`,
`PublishedReleaseIds`). Until 2026-11-23 the links beside "Coming from the retired network?" (home page,
tools page and guide) offered the retired network's 0.4.0 Windows package, labelled as such; the owner's request of
2026-11-23 replaced the earlier decision to keep them until W16, so the links, their labels (`appsWindowsUpdateDownload`)
and the `LEGACY_NATIVE` exemption are gone, the home page's "Verify a download" says that the page links no package of
the retired network either, and the host's copy of that package is a file to delete (`tools/host_files.py` DELETE). The
migration warning beside it (balances do not carry over; move the old wallet folder aside) stays. No page offers a
native package before W16: the home page's "MAINNET EDITION" buttons
(`wallet-downloads.js`) show "No release available", and the tools page and the guide no longer offer the 0.4.0
Windows and Linux packages as PoCol Mainnet applications, the removed invitation (`downloads/network.pocol.json`, the
retired network's) or `downloads/PoCol-Apps-README.txt` (never in the snapshot). `tests/test_downloads.py` checks that
every `downloads/` link is a file of the snapshot or a `keptOut` file, and that no page links or names a retired
`PoCol-Mainnet-0.4.x` (or older) package. `api/network.php` counts a HELD (1) or ACCRUING (4) active record as an eligible miner, as the node
does.
`tests/test_launch_decisions.py` (`LaunchTexts`) refuses the removed texts.

## Production readiness, sites track (2026-11-22)

- **No retired extension package (SITE-01).** No page of pocol.net, the guide, Studio's install dialog or the IDE
  links an extension package before the signed release: they link `pocol.net/chrome-extension.html#verify-release`
  and say that the PoCol Mainnet extension is published with the signed release. The owner approved this policy on
  2026-11-23 ("لا رابط ولا بصمة قبل الإصدار الموقّع", no link and no digest before the signed release: D4 of
  `development/decisions/OWNER-DECISIONS-2026-11-23.md`). The IDE's button is the anchored edit
  `tools/release-pending-edits.json` (after W14). The retired network's 0.4.1 package, its unversioned copy
  `PoCol-Studio-Chrome.zip` and the two tracked `.sha256` files left `SHA256SUMS.txt` and `keptOut` through
  `site_identity.py --drop-download`, so they are files to delete from the hosts. `tests/test_downloads.py` refuses any
  extension package name other than the signed release's (`assemble_release.py`, none before the switch) recorded as a
  kept-out download. Step 7 had also listed an unsigned 0.5.0 extension zip in both sites' `SHA256SUMS.txt`, `keptOut`
  and tracked `.sha256` files and linked it: the digest of an unsigned build, which goes stale with any change to the
  extension and is not a signed release. Those lines and files were withdrawn when step 7 was merged here, and until
  `tools/release_page.py` records a signed release no site lists an extension package digest
  (`tests/test_downloads.py`). `DEPLOY-README-AR.txt` section 4, which still said the extension ships with the IDE copy,
  is the anchored edit `tools/release-pending-text-edits.json`, applied after `release-pending-edits.json` (the W14
  reproduction test undoes and redoes both). The home page's install steps say the package comes with the signed
  release, and no site text tells a user to download an extension package now. Studio's release builder no longer
  copies the zip into `dist/downloads` or the cPanel ZIP, and refuses a `dist/app.js` that links an extension package
  or a `dist/` that holds one (`prototypes/pocol-studio/tests/build-release.test.mjs`). CI (`full-nonce-upgrade.yml`)
  packages the extension twice, requires the same bytes, and refuses a listed extension package line that is not this
  build's. `tests/test_downloads.py` also checks every site's `downloads/` links (pocol.net, the IDE, Studio's `dist/`
  and the explorer) and that no site file names the content version of a retired network's invitation.
- **Extension ID and release key (SITE-03).** The extension page (section `verify-release`) and the guide's
  extension topic carry, in English and Arabic, the extension ID `gggcoflmedlhlkcdhebmoepmggdnafmp` and the release
  key fingerprint `SHA256:FlrCFc4LXIOOPMrT2g4dgV6CuOEiw1L30NWKzGPBZt8` with `deploy/release/README.md`'s user
  procedure; the home page's "Verify a download" carries the fingerprint only and links that section.
  `tests/test_downloads.py` derives both from `manifest-key.pub` and `allowed_signers`. That ID is the side-loaded
  signed release's: the Chrome Web Store item gets the ID the store assigns at its first upload (owner decision
  2026-11-23 D2, `prototypes/pocol-extension/store/README.md`), and pocol.net publishes it with the store link (W16),
  beside this one. The page says that no signed release is offered yet. This is the repository snapshot: nothing is public until the owner approves the early
  publication and uploads it (pending on 2026-11-22).
- **Download structure.** `wallet-downloads.js` has a `release` record. `tools/release_page.py RELEASE_DIR` fills the
  buttons (URL, size, SHA-256, `.sig`) and the record (version, signed `SHA256SUMS` and `.sig`) only from a directory
  that `deploy/release/verify_release.py` verifies with the committed `allowed_signers`, and records every release
  file, its `.sig` and `SHA256SUMS(.sig)` as kept-out lines (`--download` accepts `SHA256SUMS` and
  `network.pocol.json` with their `.sig`).
- **Reward page (F-74).** `api/reward.php` and `reward.js` read `pocol-block-reward-v3` (`deploy/rewards/README.md`) and
  show its summary with neutral labels. The headline wording ("Collaboration. Rewarded." and its subtitle) and the
  new labels await the owner's S1 wording decision (`development/STATUS-EN.md`).
- **Network summary (SITE-05, 2026-11-22 review).** `api/network.php` takes the eligible-miner count from the
  gateway's `pocol_getMemberCounts` (the ActList's counts at one block, at most 10,000 members) instead of the unpaged
  `pocol_getMiners`. Paging `pocol_getMembers` instead would cost 5 of the gateway's shared request tokens a page
  (80, refilled at 20 a second, shared with every explorer visitor): 222 tokens a refresh at 10,000 members, refused
  part-way from about 4,000. A refresh is now two requests, 42 tokens, at any size, and all its requests together keep
  to 15 s. `tests/network_budget.py` replays them through the real `Gateway.charge`. The gateway that serves
  `pocol_getMemberCounts` must be installed before this snapshot is uploaded; with an older gateway the summary
  answers "temporarily unavailable".
- **Deployment content (2026-11-23).** For the owner's request of 2026-11-23 (the pocol.net cPanel deployment of
  PoCol Mainnet, item 4: no Testnet and no 0.4.x release in the deployment content), besides the retired Windows package
  above: the i18n keys `networkTestnet`, `appsTestnet` and `appsTestnetNote`, whose values already said PoCol Mainnet,
  are `networkName`, `appsEdition` and `appsEditionNote`; the unused design-preview strings `walletTitle`, `walletBody`,
  `nodeTitle` and `nodeBody` ("will be published here when ready") left `locales.js`; the status page and its guide
  topic say that PoCol Mainnet launched on 2026-11-21 (genesis time 20:20:56 UTC, `development/public-network/
  GENESIS-RECORD-2026-11-21.md`) instead of "launches"; and `sitemap.xml` lists `faucet.html`.
  `tests/test_served_content.py` checks each.
- **Headers, 404, robots (SITE-06, SITE-07).** `pocol.net/.htaccess` sets nosniff, the referrer and permissions
  policies, `X-Frame-Options DENY` and a `'self'` CSP (`connect-src` adds `https://explorer.pocol.net`; style
  attributes are allowed for the guide's illustration), refuses directory listings and `config.php`, and serves the
  bilingual `404.html`; `robots.txt` and `sitemap.xml` are new. The host must honour `.htaccess` with `mod_headers`
  (owner check); HSTS is the owner's decision.
- **Uploads (SITE-02).** An overwrite upload leaves stale files served. After each upload the owner lists the document
  root on the host and runs `tools/host_files.py --site SITE --listing FILE` (pocol.net, ide.pocol.net,
  studio.pocol.net): it names each file to delete, each that differs and each that is missing, and keeps host
  configuration (`api/config.php`, cPanel's `.user.ini` and `php.ini`, `error_log`, `.well-known/`, `cgi-bin/`); the
  retired 0.4.0 Windows package is DELETE since 2026-11-23 (`LEGACY_NATIVE` is empty). A host `.htaccess` that differs is `MERGE`, never replaced: cPanel
  writes its MultiPHP handler block and redirects there, so the owner keeps those and adds the snapshot's `.htaccess`
  unchanged, and `--host-copy .htaccess=COPY` (a downloaded copy of the listed file) confirms that the snapshot's
  directives are in it as one block (2026-11-22 review). Where a snapshot has no `.htaccess` the host's is kept.
  `pocol.net/README-AR.txt`, the IDE's `DEPLOY-README-AR.txt` and the Studio README give the step.

A changed bundle is renamed `<prefix>-<first 12 hex digits of its SHA-256>.js` and `index.html` follows, as does the
explorer bundle's line in `tools/explorer-s1-include.txt` (a list that names any other explorer bundle fails
`--check`); `?v=` content versions and every `SHA256SUMS.txt` are regenerated. `--check` also refuses a retired identity
(`deploy/mainnet/retired-networks.json`, active once the snapshots pin a network that is not retired), any
`chatgpt.site` origin, a download link whose version is not the extension's `package.json` version, a missing
`index.html` script or stylesheet, any `config.php` (never read), and server code or a hidden file in the explorer
snapshot (an `api/` entry, a `.php` file, a dotfile such as `.htaccess`: Sites W8). A `--to` run without `--gateway-host` is refused
unless the explorer's current gateway host is one of the new seeds, and the repository's own snapshots must name an
https RPC URL (a loopback `--rpc-url` is only for a test copy given with `--root`). The explorer bundle was renamed to that scheme
(`assets/index-473b5bd9e7ec.js`, the same bytes as the deployed `assets/index-Cj8htKRO.js`), and the superseded IDE bundle
`assets/index-CJbrNXr_.js` (it differed from `index-extension041-02af63944c2b.js` only in its download link) was
deleted with its `SHA256SUMS.txt` line.

Not included, and must be preserved on the host: every hosting configuration and credential. No `config.php` is in
the repository (`.gitignore` ignores every `api/config.php`, CI fails if one is tracked, and `site_identity.py --check`
refuses one in a snapshot). The pocol.net PHP files (`api/network.php`, `api/reward.php`) need none. The explorer
snapshot has no PHP since Sites W8 (its `/api` reads are the gateway's, `deploy/explorer`), and its `SHA256SUMS.txt`
is not served there.

## Live comparison, 2026-11-01 (from outside, with strict TLS)

- **Identical:** 171 of 175 files byte-for-byte; no HTTP 403 or other error. PHP and dotfiles were not compared,
  because a server executes or hides them.
- **The four that differ:**
  - `pocol.net/style.css` and `pocol.net/assets/pocol-icon.png`, unversioned URL: a cached older copy (`age` about
    46 h). The pages request `style.css?v=11f712c55ca8`, and that URL returns the packaged file.
  - `studio.pocol.net/assets/pocol-icon.png`, unversioned URL: the same cached older copy. No page references it.
  - `ide.pocol.net/SHA256SUMS.txt`: the live copy lacks the line for the superseded bundle `assets/index-CJbrNXr_.js`.
- **Downloads:** the old Windows and Linux packages and the 0.4.1 extension matched the release hashes.
- **Not checked:** file permissions on the host itself; there is no cPanel access from here.
- **Stale, unchanged by this snapshot:** `pocol.net/downloads/PoCol-Apps-SHA256SUMS.txt` is served live but is not in
  the package. It still lists old Android/testnet files.
