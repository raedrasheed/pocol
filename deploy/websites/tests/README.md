# Local security checks

Run from the repository root. These tests use disposable local state and public
synthetic data. They do not call the live PoCol network or use an existing wallet.

```bash
python3 -B -m unittest discover -s deploy/websites/tests -p 'test_*.py' -v
python3 -B deploy/websites/tools/site_identity.py --check
php deploy/websites/tests/api_security.php
python3 -B -m unittest discover -s deploy/rewards -p test_explorer_api.py -v   # the explorer's /api reads (Sites W8)
python3 -B -m unittest discover -s deploy/explorer -v                         # the explorer gateway host's Caddy site
PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs \
  POCOL_CHROMIUM=/path/to/chromium node deploy/websites/tests/browser_security.mjs
```

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

These 20 browser checks use no browser extension. They do not establish extension
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
semantics (W7: "Earning bond" for an ACCRUING member, a decoded `registerReplacing`) are not checked: the bundle has
no `pocol_getMembers` yet. A failed run keeps its work directory's logs and inputs (`explorer-gateway.log`,
`caddy.log`, the `Caddyfile`, the devnet's node logs) in `$POCOL_E2E_KEEP` (CI: `evidence/kept`, uploaded) or
`prototypes/pocol-extension/tests/failures/`.

The IDE smoke (a contract compiled with the IDE's own compiler and deployed through the extension and a local Studio
gateway, then loaded and read in the IDE's own UI through its switched `rpcUrl`) is in
`prototypes/pocol-extension/tests/e2e_publisher_gateway.mjs`. Both run in `.github/workflows/sites-e2e.yml`.
