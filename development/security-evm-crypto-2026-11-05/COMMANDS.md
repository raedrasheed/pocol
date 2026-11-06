# Reproduction commands

Run from the repository root with Python 3, Rust 1.99.1, the locked Cargo cache,
and the platform tools required by the existing build scripts. No commands here
deploy services, create signing keys, disable policy, or change the default
GitHub CodeQL configuration. Select fresh output paths to preserve prior evidence.

## Local source and graph guards

```bash
python3 development/security-evm-crypto-2026-11-05/verify_vendor.py
cargo +1.99.1 metadata --locked --offline --format-version 1 \
  --manifest-path prototypes/pocold/Cargo.toml > /tmp/pocol-node-crypto-metadata.json
python3 development/security-evm-crypto-2026-11-05/verify_crypto_graph.py \
  --metadata /tmp/pocol-node-crypto-metadata.json
```

Repeat metadata/graph verification for `prototypes/lp4-execution/Cargo.toml` and
`development/security-audit-2026-11-04/fuzz/Cargo.toml`. `--offline` requires
dependencies already cached. A missing cache is not a passing verification.

```bash
cargo +1.99.1 test --release --locked --offline \
  --manifest-path prototypes/lp4-execution/Cargo.toml -- --test-threads=1
cargo +1.99.1 test --release --locked --offline \
  --manifest-path prototypes/pocold/Cargo.toml -- --test-threads=1
python3 deploy/public-testnet/test_nonce_upgrade.py -v
python3 deploy/standalone/test_release_package.py -v
```

The recorded complete suites preceded the addition of `evm_numeric_edges`; that
new test was run separately and passed. Blackbox launcher/integration/wallet
checks require the freshly built node binaries; use the existing standalone
build/test scripts. The recorded Android run used
`bash deploy/standalone/build-android.sh` and the configured SDK/NDK/JDK/Gradle;
its unsigned output needs actual-device validation before release.

## Independent before/after recovery and execution

Keep an untouched checkout at base commit
`d2c92a7dd1b88d65a636240ba10f948cd92fc419` and this checkout separately.
Set `CARGO_TARGET_DIR` to a writable scratch cache if desired. The runner creates
fresh scratch manifests/locks, leaving the product locks unchanged; it requires
the already cached old and new dependency graphs. For example:

```bash
python3 development/security-evm-crypto-2026-11-05/run_equivalence.py \
  --before-root /path/to/before-checkout \
  --after-root /path/to/this-checkout \
  --output /tmp/pocol-new-equivalence-evidence
```

The saved result records source and binary hashes, selected versions, case counts
and byte-level output comparisons. Fixture generation intentionally runs with
the historical k256 0.13.4 in a separate scratch project, not in an active product
lockfile. `historical-fixture-provenance.json` records the executed generator hash.
The upstream comparison logs retain three identical baseline test failures.

## Owner's reviewed CodeQL batch

Run these from the repository root on a computer where `gh` already has normal
code-scanning access to `palcertcom/pocol`. The cloud integration's 403 is not
resolved by this helper. It never requests tokens or starts a login flow.
The two output filenames must be new and their parent directory must exist.

Read-only verification (PowerShell, Bash and cmd accept this single line):

```text
python development/security-evm-crypto-2026-11-05/apply_reviewed_codeql.py --out codeql-reviewed-dry-run.json
```

Apply the pinned, previously reviewed decisions with fresh validation:

```text
python development/security-evm-crypto-2026-11-05/apply_reviewed_codeql.py --apply --out codeql-reviewed-apply.json
```

Already closed alerts are skipped. Changed rule/location/source on any open ref,
missing immutable source, a moving main branch or insufficient permission stops
the batch and saves the result. Inspect that report before resuming. The current
source may have advanced since the historical review; a stopped row needs a new
source review, not a weakened guard or blanket dismissal.

```bash
python3 -m unittest discover \
  -s development/security-evm-crypto-2026-11-05 \
  -p test_apply_reviewed_codeql.py -v
```

These are 13 mocked API tests and perform no real alert writes. The separate
`CodeQL built CSharp and Go diagnostics` workflow performs compiler-traced
extraction and preserves quality diagnostics with SARIF upload disabled. Existing
default setup remains authoritative until an owner-managed, all-language
configuration transition has actually been verified.

## New PR29 gate: 11 separately reviewed findings

Use the separate [source review](CODEQL-PR29-REVIEW-AR.md) and helper for these
new locations. The historical 197-alert batch does not include them.

```text
python development/security-evm-crypto-2026-11-05/apply_pr29_codeql.py --out codeql-pr29-verify-v3.json
python development/security-evm-crypto-2026-11-05/apply_pr29_codeql.py --apply --out codeql-pr29-apply-v3.json
```

The operator needs normal code-scanning permission. The helper discovers server
IDs by exact reviewed PR/rule/location matches and validates every open ref
against an exact reviewed location and whole-file hash for that source version.
The PR target must match its current review; other refs may use an independently
reviewed older version from the separately hash-pinned historical plan, with the
same rule/path/decision. An unreviewed version or location still stops writes.
The entire batch must pass preflight before any PATCH; instances and heads are
then revalidated before each write. It leaves the failed CheckRun unchanged;
verify GitHub's subsequent gate result before merging.

```bash
python3 -m unittest discover \
  -s development/security-evm-crypto-2026-11-05 -p 'test_apply*codeql.py' -v
```

This runs 38 mocked tests (13 historical-batch and 25 PR29), with no real writes.
The PR29 helper now uses the actual default-setup analysis ref
`refs/pull/29/head`; see the [correction evidence](CODEQL-PR29-REF-FIX-AR.md).
Update an existing owner checkout with `git pull --ff-only` before retrying.
Use the new output filenames above to preserve the first stopped run.
If blocked, the JSON includes the failed phase and the alert/ref/location/source
hash where available. See the [cross-ref source review](CODEQL-PR29-CROSS-REF-AR.md).
