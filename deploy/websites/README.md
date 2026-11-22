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
  `locales.js` (the card keys) and `SHA256SUMS.txt` (the three page files), and that the only faucet URLs are those two
  routes (the IDE's third-party bundle is checked for faucet links only); `tests/browser_security.mjs` runs the page against stubbed answers.
  If the pocol.net host sends a Content-Security-Policy (none is in this repository), its `connect-src` must allow
  `https://explorer.pocol.net`, or every claim fails as unreachable.

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
network, and the IDE's download button names the package it links (0.4.1, it said 0.4.0). Step 7 of the mainnet runbook moved that button and its link to 0.5.0 with the identity switch, recorded as the anchored edits `tools/step7-release-edits.json` on top of W14's (the W14 test undoes and redoes both). `--check` also refuses an
extension version label that is not package.json's (a capability message "X or later" is exempt) and a native package
name that is not `deploy/release/assemble_release.py`'s at `deploy/standalone/package.py`'s version; the retired
`PoCol-Mainnet-0.4.0-Windows.zip` and `-Linux.zip` that the pages link until the signed release (W16) are exempt while a
page still names them. `--download PATH=SHA256` (repeatable) writes the `SHA256SUMS.txt` line of a release file kept
out of Git (an archive, its `.sig` or `.sha256`), adds its name to the site's `keptOut` list in `site-identity.json`
and drops the kept-out lines (and names) of other versions of the same archive family and their unversioned copies; a
tracked file it supersedes must be removed first. Only the lines `keptOut` names are carried for files that are not in
the snapshot, so a tracked file that is deleted (a `.sha256`) loses its line and `--check` fails until a run drops it.

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
static source lists are the guide's, and the README, V3 program and CP links pin main at `4db2652`. The 0.5.0
download links and names and the IDE's "0.4.0 or later" messages change with the signed release (W16). Since step 7
pocol.net publishes the extension ID and the release key fingerprint users compare (the extension guide,
`chrome-extension.html` and the home page's IDE help, in English and Arabic; `tests/test_w14_texts.py`,
`PublishedReleaseIds`). Until then the links beside "Coming from the retired network?" (home page and
tools page) are labelled as the retired network's 0.4.0 Windows package; W16 replaces or removes them.
`tests/test_launch_decisions.py` (`LaunchTexts`) refuses the removed texts.

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
