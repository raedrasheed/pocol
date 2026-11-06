# Reproduction commands and outcomes

Run from `/workspace/pocol`. Validation used Linux x86_64, Rust 1.99.1, Node 24.22.1, Python 3.12.14. Local socket tests needed executor loopback capability. All nodes/accounts were disposable local simnet fixtures.

```bash
export CARGO_HOME=/workspace/.pocol-cargo
export RUSTUP_HOME=/workspace/.pocol-rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_BUILD_JOBS=4

git status --short
git fetch origin main
git rev-parse origin/main
# 0dda5056d2a78b2e135defb803df480e80422f8f; initial tree clean
git switch -c security/audit-2026-11-04
git fetch origin pull/24/head:refs/remotes/origin/pr24
git rev-parse origin/pr24
# c690f990c619214d1e75354208f32580b17b4544

# Installed rustup using TLS-verified https://sh.rustup.rs, with no PATH modification;
# rustup verified/downloaded the pinned toolchain. No TLS/checksum verification disabled.
rustup toolchain install 1.99.1 --profile minimal
npm ci --ignore-scripts --no-audit --no-fund --prefix prototypes/pocol-extension
npm ci --ignore-scripts --no-audit --no-fund --prefix prototypes/lp3-model
npm ci --ignore-scripts --no-audit --no-fund --prefix prototypes/lp4-execution/tools
cargo +1.99.1 build --release --locked --manifest-path prototypes/pocold/Cargo.toml --bins
npm run build --prefix prototypes/pocol-extension

cargo +1.99.1 test --release --locked --manifest-path prototypes/pocold/Cargo.toml
# Baseline exits 101: obsolete unhex vulnerability witness expects a panic.
cargo +1.99.1 test --release --locked --manifest-path prototypes/pocold/Cargo.toml --no-fail-fast
# Final exit 0: 319 passed, 30 ignored.
cargo +1.99.1 test --release --locked --manifest-path prototypes/pocold/Cargo.toml --test wire_mutation
# Exit 0, 1 test, 20,000 mutations plus truncation/length cases.

cargo +1.99.1 test --release --locked --manifest-path prototypes/lp4-execution/Cargo.toml
# Baseline exits 101: obsolete earnin_halt_proof expectations (167 passed, 2 failed,
# 1 ignored before Cargo stops; this is not the entire execution suite).
cargo +1.99.1 test --release --locked --manifest-path prototypes/lp4-execution/Cargo.toml --no-fail-fast
# 219 passed, 2 failed, 1 ignored. The new escrow assertion incorrectly expected
# zero instead of preserving the genesis bond. Corrected to compare before/after.
cargo +1.99.1 test --release --locked --manifest-path prototypes/lp4-execution/Cargo.toml --test earnin_halt_proof
# Final target: exit 0, 3 passed. All other targets passed in the preceding run.

npm test --prefix prototypes/pocol-extension
# Final exit 0: 268 existing component checks + 3 lifecycle regressions.
# The same 3 lifecycle tests were run against an isolated copy of the main worker:
# all 3 failed (evidence/wallet-races-before.log). No tracked source was reverted.
npm run test:studio --prefix prototypes/pocol-extension     # 34 checks
npm run test:publisher --prefix prototypes/pocol-extension  # 42 tests
npm run test:contracts --prefix prototypes/pocol-extension  # 16 tests
npm run test:contract-ui --prefix prototypes/pocol-extension # 13 tests
npm test --prefix prototypes/pocol-studio                  # 9 tests
npm run check --prefix prototypes/pocol-studio             # links/syntax pass
npm run typecheck --prefix prototypes/lp3-model            # pass
npm test --prefix prototypes/lp3-model                     # 11 tests
python3 -m unittest discover -s deploy/studio -p 'test*.py' # 22 pass
python3 prototypes/pocold/tests/e2e_devnet.py               # 44 pass

npm audit --json --prefix prototypes/pocol-extension       # 0 advisories
npm audit --json --prefix prototypes/lp4-execution/tools   # baseline: 3 tagged packages
npm update follow-redirects --package-lock-only --ignore-scripts --no-audit --no-fund --prefix prototypes/lp4-execution/tools
npm ci --ignore-scripts --no-audit --no-fund --prefix prototypes/lp4-execution/tools
npm audit --json --prefix prototypes/lp4-execution/tools   # after: 2 tagged packages, exit 1

cargo +1.99.1 install cargo-audit --locked # installed 0.22.2, tools only
cargo audit --json --file prototypes/pocold/Cargo.lock --db /tmp/pocol-rustsec-db
# exit 1, crypto-bigint advisory; warnings retained
cargo audit --json --no-fetch --file prototypes/lp4-execution/Cargo.lock --db /tmp/pocol-rustsec-db
cargo +1.99.1 fetch --locked --manifest-path prototypes/lp1-node/Cargo.toml
cargo audit --json --no-fetch --file prototypes/lp1-node/Cargo.lock --db /tmp/pocol-rustsec-db
cargo +1.99.1 tree --locked --offline --manifest-path prototypes/pocold/Cargo.toml -i crypto-bigint
cargo +1.99.1 tree --locked --offline --manifest-path prototypes/lp1-node/Cargo.toml -e features -i keccak
cargo +1.99.1 tree --locked --offline --manifest-path prototypes/lp1-node/Cargo.toml -e features -i rand

git diff --check  # pass
git push -u origin security/audit-2026-11-04 # success
```

For compiler equivalence, before and after the lockfile patch, ran every `prototypes/lp4-execution/tests/compile_*.cjs` with `NODE_PATH=/workspace/pocol/prototypes/lp4-execution/tools/node_modules`, writing one JSON output per script to `/tmp/pocol-solc-before` and `/tmp/pocol-solc-after`. Also ran `compile_system_v1.cjs <output> --continuous`. Asserted exact byte equality for all 13 outputs, not only compiler version or bytecode length. No tracked fixture was rewritten. Hashes: `evidence/compiler-equivalence.json`.

PR24 validation loaded `git show origin/pr24:deploy/studio/install_studio_gateway.py` into an isolated Python module, substituted it as `test_studio_gateway.installer`, and ran the six `InstallerTests`. A separate real SSLContext check mocked only transport/HTTPS response and asserted `minimum_version == TLSv1_2`, `check_hostname`, and `verify_mode == CERT_REQUIRED`. No live gateway request.

Local Semgrep:

```bash
python3 -m venv /workspace/pocol-audit-tools
/workspace/pocol-audit-tools/bin/pip install semgrep # 1.183.0
git clone --depth 1 https://github.com/semgrep/semgrep-rules.git /tmp/pocol-semgrep-rules
git -C /tmp/pocol-semgrep-rules rev-parse HEAD
# 16b9e6d45f10649f6a531ec4838ed9924372c3bc

SEMGREP_SETTINGS_FILE=/workspace/pocol-audit-tools/semgrep-settings.yml \
/workspace/pocol-audit-tools/bin/semgrep scan \
  --metrics=off --disable-version-check --no-rewrite-rule-ids \
  --jobs=4 --timeout=10 --max-target-bytes=2000000 \
  --config /tmp/pocol-semgrep-rules/javascript/browser/security \
  --config /tmp/pocol-semgrep-rules/javascript/lang/security \
  --config /tmp/pocol-semgrep-rules/python/lang/security \
  --config /tmp/pocol-semgrep-rules/rust \
  --config /tmp/pocol-semgrep-rules/solidity \
  --json --output /tmp/pocol-semgrep-local.json \
  prototypes/pocold/src prototypes/lp1-node/src prototypes/lp4-execution/src \
  prototypes/lp4-execution/contracts prototypes/pocol-extension/src \
  prototypes/pocol-studio/dist deploy
# 254 findings, 5 errors, 355 selected files; see report for skipped files.
```

The four Studio JS files were copied byte-for-byte to `/tmp/pocol-studio-scan`, then scanned with the same two JavaScript configurations, jobs=2 and timeout=10. This recovered default `dist` exclusions: 57 additional findings, no parse error. Path mapping is in provenance.json. The main comparison scans the exact 355 paths listed in the post-fix scan, materialized via `git show origin/main:<path>` under `/tmp/pocol-semgrep-main`, with the same configurations/limits. This deliberately compares the selected scope, not the entire repository.

Browser attempt:

```bash
npm install --prefix /workspace/pocol-browser-tools --ignore-scripts --no-audit --no-fund playwright@1.62.1
# System Chromium was 155.0.8059.79; an external browser-cache symlink pointed to
# it solely for this attempt. It was not the pinned Playwright browser.
PLAYWRIGHT_MODULE=/workspace/pocol-browser-tools/node_modules/playwright/index.mjs \
PLAYWRIGHT_BROWSERS_PATH=/workspace/pocol-browser-cache \
node prototypes/pocol-extension/tests/e2e_browser.mjs
# FAIL: 19 passed pre-browser checks + 1 failed driver check (20 total): no extension serviceworker.
# Diagnosed with isolated launches, including ignoreDefaultArgs for the default
# --disable-extensions switch and direct system executable; neither loaded it.

PLAYWRIGHT_BROWSERS_PATH=/workspace/pocol-playwright-browsers \
node /workspace/pocol-browser-tools/node_modules/playwright/cli.js install chromium
# FAIL: cdn.playwright.dev returned 403 Domain forbidden.
```

Remote checks blocked by the API destination policy:

```bash
gh pr view 24 --repo palcertcom/pocol --json state,isDraft,headRefOid,statusCheckRollup
gh run view 41889848835 --repo palcertcom/pocol --json conclusion,headSha
gh run download 41889848835 --repo palcertcom/pocol --dir /tmp/pocol-semgrep
gh run download 41875907642 --repo palcertcom/pocol --dir /tmp/pocol-osv
gh api repos/palcertcom/pocol/code-scanning/alerts/7 --jq '{number,state,rule:.rule.id}'
# Forbidden. curl against api.github.com confirmed a CONNECT 403.
```

Public PR/run pages on github.com could be read; the run pages require sign-in for artifacts. Draft environment changes do not confer API credentials or apply network access until the product's review/save/publish flow completes. No credential values were read or printed. No CLI login or replacement token was requested.

Final comparable runs used the same 355 materialized paths on main and fixed snapshots, the same rules, jobs=2 and timeout=30. Both returned 258 findings; normalized rule/path/line/column sets were identical. The two IDE worker syntax errors remain; Explorer timeouts resolved at 30 seconds (semgrep-comparison.json). The four findings absent from the earlier faster post-fix run were timeout-related, not fixes.

PR creation attempted with `gh pr create --draft --repo palcertcom/pocol --base main --head security/audit-2026-11-04 --title "Security: fence extension wallet creation and lock races" --body-file /tmp/pocol-security-pr.md`: failed with `Post https://api.github.com/graphql: Forbidden`. No PR number exists. Body retained in PULL_REQUEST.md.
