# Reproduce from the repository root

Use Node 18 or newer for the test runner and npm 8.3 or newer for overrides. Recorded validation used Node 24.22.1 and npm 11.9.0 on Linux. Use a clean scratch directory for before/after data; do not modify accepted contract fixtures.

```bash
npm ci --prefix prototypes/lp4-execution/tools --ignore-scripts --no-audit --no-fund --engine-strict
node --test --test-reporter=tap prototypes/lp4-execution/tools/test_tmp_security.cjs
npm audit --prefix prototypes/lp4-execution/tools --json --package-lock-only --ignore-scripts
```

Expected after result: 7 tests pass on Linux, and npm audit exits 0 for this tools lock. The local solver fixture test skips on Windows; native Windows was not validated. The SMT case starts a local subprocess and requires an executor that permits it; a denied subprocess is not a security regression result.

## Isolated before install and compiler equivalence

The following commands create a new scratch directory and read the old manifests from the fixed signing commit without switching branches:

```bash
review_dir=$(mktemp -d /tmp/pocol-compiler-review.XXXXXX)
mkdir "$review_dir/before"
git show e3d6ae4ae15554e0ed60446e13d1b71585edde44:prototypes/lp4-execution/tools/package.json > "$review_dir/before/package.json"
git show e3d6ae4ae15554e0ed60446e13d1b71585edde44:prototypes/lp4-execution/tools/package-lock.json > "$review_dir/before/package-lock.json"
npm ci --prefix "$review_dir/before" --ignore-scripts --no-audit --no-fund --engine-strict

POCOL_COMPILER_TOOLS="$review_dir/before" node --test --test-reporter=tap \
  prototypes/lp4-execution/tools/test_tmp_security.cjs

python3 development/security-dependency-followup-2026-11-05/compare_compilers.py \
  --before-tools "$review_dir/before" \
  --after-tools prototypes/lp4-execution/tools \
  --output "$review_dir/compiler-comparison" --jobs 2
```

The before regression deliberately exits **1**, with exactly five security cases failing and both compatibility checks passing. It writes only test-owned files. The comparison then exits **0** for 13 identical outputs. If using `set -e`, run the expected-failure command separately or explicitly capture its exit status before proceeding. The helper refuses an existing output directory so previous evidence cannot be overwritten.

## Remaining legacy EVM dependency

```bash
cargo +1.99.1 tree --locked --offline --manifest-path prototypes/pocold/Cargo.toml \
  -i crypto-bigint@0.5.5 -e features
```

This is an unresolved Rust dependency, not part of npm's zero-alert result. The corrected private signer and the full source manifest are inherited from e3d6ae4; Rust suites and Android were not repeated for this Node-only change.

## Evidence integrity

```bash
cd development/security-dependency-followup-2026-11-05/evidence
sha256sum -c SHA256SUMS
```
