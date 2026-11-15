# Local security checks

Run from the repository root. These tests use disposable local state and public
synthetic data. They do not call the live PoCol network or use an existing wallet.

```bash
python3 -B -m unittest discover -s deploy/websites/tests -p 'test_*.py' -v
python3 -B deploy/websites/tools/site_identity.py --check
php deploy/websites/tests/api_security.php
PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs \
  POCOL_CHROMIUM=/path/to/chromium node deploy/websites/tests/browser_security.mjs
```

The Python tests (`test_site_identity.py`) switch a temporary copy of the snapshots to a test identity and back
(byte-identical), check that a moved, duplicated or stray anchor fails and writes nothing, exercise anchored edits and
every `--check` refusal, and guard `config.php`: none is tracked, `.gitignore` ignores it, the tool never reads one,
and `explorer.pocol.net/api/.htaccess` keeps its deny rule.

The PHP test needs PHP 8, curl and POSIX functions on Linux. Its 67 checks exercise
the actual request/response validators (including the v3 membership and finality
reads and their bounds), pinned summary identity/branch consistency,
exact reward arithmetic and private cache/lock handling with injectable transports.
It does not start a PHP web server or validate the production host configuration.

The browser test needs Playwright (tested with 1.62.1) and an installed Chromium
(tested with 155.0.8059.79). It serves the four website roots on loopback only,
aborts HTTP requests outside each test origin, closes WebSockets, and checks that
no service worker was registered. It compiles with the existing IDE's bundled
Solidity 0.8.38, refuses a remote Solidity import, then deploys and calls a simple
contract in the existing browser worker's local Ethereum simulation. No compiler
or generated website bundle is modified. `POCOL_CHROMIUM` defaults to the system
Chromium path `/usr/bin/chromium`.

These 20 browser checks use no browser extension. They do not establish extension
viewer isolation, signing authorization, production HTTP headers, compatibility on
other browsers or equivalence between this local EVM and PoCol consensus.

## Explorer smoke on a v3 devnet (Sites W11)

```bash
node deploy/websites/tests/explorer_smoke.mjs   # needs php, python3, Playwright/Chromium and the pocold release binaries
```

It starts a local v3 devnet, switches a temporary copy of the snapshots (under `$RUNNER_TEMP`, else the system temp
directory) to the devnet's identity with `tools/site_identity.py --to`, writes that copy's `api/config.php` from
`deploy/websites/explorer-config.example.php` (the host's `config.php` is never read; the example lives outside the
document root, so it is neither in `SHA256SUMS.txt` nor uploaded), and serves the explorer with `php -S` against a
local `deploy/rewards/gateway.py --v3-network`. It checks that `api/rpc.php` and `api/health.php` relay and match the
devnet, that the bundle shows the devnet's live head, that no relay answer reaches 4 MiB and that no request leaves the
page's origin. `php -S` ignores `.htaccess`; its deny rule is checked by `test_site_identity.py`. The explorer's v3
membership semantics (W7: "Earning bond" for an ACCRUING member, a decoded `registerReplacing`) are not checked: the
bundle has no `pocol_getMembers` yet. The IDE smoke (a contract compiled with the IDE's own compiler and deployed
through the extension and a local Studio gateway) is in `prototypes/pocol-extension/tests/e2e_publisher_gateway.mjs`.
Both run in `.github/workflows/sites-e2e.yml`.
