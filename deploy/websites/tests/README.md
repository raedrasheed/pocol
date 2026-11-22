# Local security checks

Run from the repository root. These tests use disposable local state and public
synthetic data. They do not call the live PoCol network or use an existing wallet.

```bash
python3 -B -m unittest discover -s deploy/websites/tests -p 'test_*.py' -v
python3 -B deploy/websites/tools/site_identity.py --check
php deploy/websites/tests/api_security.php
python3 -B -m unittest discover -s deploy/rewards -p test_explorer_api.py -v   # the explorer's /api reads (Sites W8)
python3 -B -m unittest discover -s deploy/explorer -v                         # the explorer gateway host's Caddy site
node --test deploy/websites/tests/explorer_w7.test.mjs                        # the explorer's W7 edits and helpers
node --test deploy/websites/tests/explorer_prod.test.mjs                      # the explorer's production-readiness edits
PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs \
  POCOL_CHROMIUM=/path/to/chromium node deploy/websites/tests/browser_security.mjs
PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs \
  POCOL_CHROMIUM=/path/to/chromium node deploy/websites/tests/explorer_w7_browser.mjs   # the W7 Overview, fake relay
```

## Explorer v3 membership (Sites W7)

The explorer bundle has no sources (owner answer Q1), so W7 is a list of anchored edits,
`tools/explorer-w7-edits.json`, applied once with `python3 -B deploy/websites/tools/site_identity.py --edits
deploy/websites/tools/explorer-w7-edits.json` (the tool renames the bundle and rewrites index.html, SHA256SUMS.txt and
the S1 include list). `explorer_w7.test.mjs` checks that the served bundle carries exactly those edits (undone in
reverse they leave no `PocolW7` name; redone they give the bundle byte for byte), runs the extracted label, bond and
Registry-decoding helpers in `node:vm` over every bondState, status and tomb value, the ACCRUING thresholds and all 8
Registry selectors (checked against keccak-256 of the PoColSystemV3.sol signatures) with bad calldata, and checks that
the refresh reads one `pocol_getMembers` page and light blocks. `test_explorer_w7_size.py` sends that refresh through
the gateway's relay at the mainnet profile's bounds: each request (and all three together) stays under 4 MiB, one
full block (about 393 KB) fits, and 20 full blocks, the refresh before W7, are refused.
`explorer_w7_browser.mjs` loads the snapshot in Chromium against a fake relay (no node or gateway; every outside
request aborted) with 300 members: when the gateway refuses the refresh's transaction read (429, its busy answer) after
the status and block reads, the Latest transactions panel says so with the gateway's message instead of "No
transactions in this window" (also for a message without a closing period); the map's summary counts the loaded
members page ("256 members on the loaded page"); and an account view left open follows the loaded window: when the head
moves on, the account's new transaction appears, one that left the latest 20 blocks goes, and only the new block is
read in full (the activity caches each block's matches by hash); when that read is refused, the last list stays with a
warning above it that it may be out of date.

## Explorer production readiness (2026-11-22)

`tools/explorer-prod-edits.json` holds anchored edits applied after W7 and the site-truth edits (same tool, same
rules), checked by `explorer_prod.test.mjs`: undone in reverse they leave no `PocolP1` name and redone they give the
served bundle byte for byte. The bundle's relay read (`it`, every `/api/rpc.php` request) retries a transient refusal
twice, 1.5 s and then 3 s later: the relay's busy or failure answer (HTTP 429, 502, 503) and a read that the node or
gateway answered busy or temporarily unavailable (-32021, -32000). Any other answer is reported at once, as before. The
test runs the read in `node:vm` with a scripted fetch. The cause was `explorer_smoke.mjs`'s registerReplacing check,
which failed in CI (run 44985665129, the dialog never showed the decoded call). The transaction view's receipt read is
heavy on v3, and a node whose heavy lane is full answers it -32021 busy. The view reported that as final ("Unable to
load this result: busy"), so the smoke waited its 60 s and failed. `explorer_w7_browser.mjs` scenario 5 reproduces
this with a fake relay: the receipt read answered busy once now shows the decoded call and the named member, and busy
three times in a row is reported after two retries. On a failure the smoke now prints what the dialog showed. The same
edits add two texts. The Transactions tab names the first block the gateway's transaction index still keeps
(`pocol_transactions.py` `index.from`) once older blocks have been removed. The Miner map says that the network records
no miner location and that peers are not placed by their IP addresses; it no longer points to a GeoIP installer that
does not exist.

## Site texts and download checks (Sites W14)

The retired activation texts and the in-place extension update are anchored edits, `tools/w14-text-edits.json`
(pocol.net, the IDE's DEPLOY-README-AR.txt and download label, the Studio's install dialog), plus a direct edit of
`prototypes/pocol-studio/QUICKSTART-AR.txt`. `test_w14_texts.py` refuses "PoCol Mainnet activation", the
`[2,"15000"]` schedule and every same-folder, replace-the-files or Reload update text (English and Arabic) in each
served file, the Studio files and the quick start; it checks that its patterns find the old texts, that the guide, the
pocol.net help, the Studio dialog, the quick start and DEPLOY-README-AR.txt give README-IDE.md's order (export the
wallet, Remove, verify the signature, Load unpacked, compare the extension ID with pocol.net, import the wallet) in both
languages without a version number, and that DEPLOY-README-AR.txt sections 2 and 4 are for a fresh network with the
anchored network block kept. The Studio's `tests/model.test.mjs` pins the two install3 texts.
`test_site_identity.py` covers the extension version labels (package.json's version; "X or later" capability messages
exempt), the native package names (assemble_release.py's at package.py's version; the linked retired names exempt only
while linked), `--download` (kept-out lines written, superseded ones dropped, a tracked superseded file refused), the
`keptOut` list (only its lines are carried, so a deleted tracked `.sha256` fails `--check`) and
that the W14 edits, undone on a copy, give back every served file byte for byte.

## Download links and privacy policy (2026-11-22)

`test_downloads.py` checks that every pocol.net `downloads/` link names a file of the snapshot, a `keptOut` file or a
`LEGACY_NATIVE` package, that the retired network's packages are only linked with a label saying so (English and
Arabic), and that the "MAINNET EDITION" buttons offer no package before the signed release. `test_privacy_page.py`
checks `privacy.html` against `privacy.js` (English static text, Arabic for every entry, text-only rendering, the footer
link on every page) and each statement of the policy against the code it describes. `browser_security.mjs` renders the
policy in both languages at 390 px. `api_security.php` counts HELD (1) and ACCRUING (4) members as eligible miners.

## Site truth (owner decision 4, 2026-11-18)

`test_launch_decisions.py` (`LaunchTexts`) refuses the removed launch-blocking and retired-network texts in every
served file, in English and Arabic. These include "production readiness", "simnet-only", "production qualification",
"production admission", the old "Upgrade existing network" and migration steps, a pending Chrome Web Store release and
the old explorer installer, the pending Android update and the key backup that the retired wallet lacks. A check also confirms that each pattern finds its old text, and that the home page's static wallet-download texts are about-locales.js's English. The test checks that the
status page reports the open gates as node-local and that each guide page's static source list is the guide's, with
every `<code>` label matching the commit its link pins. `test_site_identity.py` checks that `tools/site-truth-edits.json`,
undone on a copy, gives back the served explorer bundle and its derived files byte for byte.

The Python tests (`test_site_identity.py`) switch a temporary copy of the snapshots to a test identity and back
(byte-identical), check that a moved, duplicated or stray anchor fails and writes nothing, exercise anchored edits and
every `--check` refusal, and guard `config.php`: none is tracked, `.gitignore` ignores it, the tool never reads one.
They also check that the explorer snapshot is static (Sites W8): no `api/`, `.php` or dotfile, which `--check` refuses.

The PHP test needs PHP 8, curl and POSIX functions on Linux. Its 26 checks exercise pocol.net's network summary and
reward page (`pocol.net/api/network.php`, `reward.php`): pinned summary identity/branch consistency, exact reward
arithmetic and private cache/lock handling with injectable transports. It does not start a PHP web server or validate
the production host configuration. The explorer relay's 43 checks (the request/response validators of the retired
`explorer.pocol.net/api/lib.php`, including the v3 membership and finality reads and their bounds) are in
`deploy/rewards/test_explorer_api.py`, against the gateway handlers that replace it, with tests of every handler over
loopback and 131 requests recorded from the PHP relays themselves (`deploy/rewards/testdata/explorer-php-vectors.json`).

The browser test needs Playwright (tested with 1.62.1) and an installed Chromium
(tested with 155.0.8059.79). It serves the four website roots on loopback only,
aborts HTTP requests outside each test origin, closes WebSockets, and checks that
no service worker was registered. It compiles with the existing IDE's bundled
Solidity 0.8.38, refuses a remote Solidity import, then deploys and calls a simple
contract in the existing browser worker's local Ethereum simulation. No compiler
or generated website bundle is modified. `POCOL_CHROMIUM` defaults to the system
Chromium path `/usr/bin/chromium`.

The fee faucet's claim page (`pocol.net/faucet.html`) is loaded in English and Arabic with explorer.pocol.net blocked
(its status and a claim must show the unreachable message), then in a second context where a Playwright route stubs
`https://explorer.pocol.net/faucet/status` and `/faucet/claim` (CORS answers for the local origin): the amount and the
transfers left, a malformed address refused before any request, a claim body of exactly `{"address"}` as JSON, the
transaction hash, address and explorer link after a transfer, a success answer over the 16 KiB cap read as a
transfer that may exist, the figures aligned with their labels in Arabic, the Arabic "check the explorer first" text for
`unknown_outcome` (with a server message that is never rendered as HTML), and a disabled button when no transfer is
left today. The real faucet and gateway route are not contacted.

These 35 browser checks use no browser extension. They do not establish extension
viewer isolation, signing authorization, production HTTP headers (the explorer smoke checks the explorer's), compatibility on
other browsers or equivalence between this local EVM and PoCol consensus.

## Explorer smoke on a v3 devnet (Sites W11, W8)

```bash
node deploy/websites/tests/explorer_smoke.mjs   # needs python3, Playwright/Chromium, the pocold release binaries and caddy
```

It starts a local v3 devnet, switches a temporary copy of the snapshots (under `$RUNNER_TEMP`, else the system temp
directory) to the devnet's identity with `tools/site_identity.py --to`, and serves the explorer the way the gateway
host does (owner decision 9): its static files (checked against its `SHA256SUMS.txt`) behind a loopback copy of the
gateway host's Caddy site (`deploy/explorer/explorer_site.py test-config`: the production block with an
`http://127.0.0.1` address, the test gateway port and the copy's root), in front of a local
`deploy/rewards/gateway.py --v3-network --site-origin`. It checks that `/api/rpc.php` relays to the devnet, that
`/api/health.php` answers HTTP 200 with `networkMatch: true` and `PASS`, that the page and the `/api` answers carry the
explorer's CSP, X-Frame-Options, Referrer-Policy, Permissions-Policy, nosniff and no-store, that `/api/rpc.php`
refuses a foreign Origin, a GET, a 16385-byte body, a batch of 21 and a method outside its allow-list, that server,
hidden and missing paths are 404, that the bundle shows the devnet's live head under its CSP without a violation, that
no answer reaches 4 MiB and that no request leaves the page's origin. `POCOL_EXPLORER_FRONT=caddy` (CI) requires
Caddy; without Caddy (`=node`, the default then) a small Node front serves the same block's routes, headers and
hidden-path rule, read from its text, so Caddy's own handling is tested only with Caddy. The explorer's v3 membership
(Sites W7) is checked live: a throwaway devnet account registers with a quarter of B_reg and another sends
`registerReplacing` naming a miner; the refresh reads one `pocol_getMembers` page and `full=false` block lists only
(never `pocol_getMiners`; full blocks one per request), the account view shows "Earning bond" with the bond as
x/B PCOL, the transaction view decodes the call and shows that miner's record read at the head, and the Miners tab and the
Overview count the loaded page. A three-miner devnet cannot fill the member set, so that replacement reverts: its
decoding and the victim read are what is checked. A failed run keeps its work directory's logs and inputs (`explorer-gateway.log`,
`caddy.log`, the `Caddyfile`, the devnet's node logs) in `$POCOL_E2E_KEEP` (CI: `evidence/kept`, uploaded) or
`prototypes/pocol-extension/tests/failures/`.

The IDE smoke (a contract compiled with the IDE's own compiler and deployed through the extension and a local Studio
gateway, then loaded and read in the IDE's own UI through its switched `rpcUrl`) is in
`prototypes/pocol-extension/tests/e2e_publisher_gateway.mjs`. Both run in `.github/workflows/sites-e2e.yml`.
