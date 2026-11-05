# Reproduce from the repository root

Use Rust/Cargo **1.99.1** and the committed lockfiles. Fetch dependencies once if the local cache is empty; subsequent checks below use locked, offline resolution. No GitHub alert write, publish, or deployment is involved.

```bash
cargo +1.99.1 fetch --locked --manifest-path prototypes/lp4-execution/Cargo.toml
cargo +1.99.1 fetch --locked --manifest-path prototypes/pocold/Cargo.toml
cargo +1.99.1 fetch --locked --manifest-path development/security-audit-2026-11-04/fuzz/Cargo.toml

cargo +1.99.1 metadata --locked --offline --format-version 1 \
  --manifest-path prototypes/lp4-execution/Cargo.toml > /tmp/pocol-signer-metadata.json
python3 development/security-crypto-timing-2026-11-05/check_signer_graph.py \
  --metadata /tmp/pocol-signer-metadata.json

POCOL_SIGNING_COMPAT_KEYS=4096 cargo +1.99.1 test --release --locked --offline \
  --manifest-path prototypes/lp4-execution/Cargo.toml \
  --test signing_compat -- --nocapture --test-threads=1

cargo +1.99.1 test --release --locked --offline \
  --manifest-path prototypes/lp4-execution/Cargo.toml -- --test-threads=1
cargo +1.99.1 test --release --locked --offline \
  --manifest-path prototypes/pocold/Cargo.toml -- --test-threads=1
cargo +1.99.1 check --release --locked --offline \
  --manifest-path development/security-audit-2026-11-04/fuzz/Cargo.toml
```

For the other graph snapshots, replace the metadata manifest with the pocold or fuzz harness manifest and feed the resulting JSON to the same checker. The pre-change failure uses resolved metadata from base commit `840accc5f4f839edb7c439f612fe35539f0056c7`; its graph summary is preserved in evidence. It is a dependency regression witness, not a timing exploit.

## Raw dependency audit

Use cargo-audit **0.22.2** with the RustSec database pinned to `26fd2d43f7280a1fb925a3743ec463017afe7119` for the recorded snapshot:

```bash
cargo +1.99.1 audit --no-fetch --db /path/to/advisory-db \
  --file prototypes/lp4-execution/Cargo.lock --json
cargo +1.99.1 audit --no-fetch --db /path/to/advisory-db \
  --file prototypes/pocold/Cargo.lock --json
```

**Expected exit status: 1.** RUSTSEC-2026-0387 remains against crypto-bigint 0.5.5 in public recovery/EVM; derivative and paste remain unmaintained. The new signing closure resolves to 0.7.10. Do not turn this into a zero-exit assertion or add an ignore to mask it. Later advisory database versions may add results.

## Android build and artifact checks

Provide the existing Android SDK, NDK **27.2.12479018**, JDK **21**, Gradle **8.11.1**, and Rust target `aarch64-linux-android`. Set `ANDROID_HOME`, `JAVA_HOME`, and PATH for those installed tools. The repository script copies from the default pocold target directory, so leave `CARGO_TARGET_DIR` unset when reproducing it.

```bash
bash deploy/standalone/build-android.sh

"$ANDROID_HOME/build-tools/35.0.0/zipalign" -c -P 16 -v 4 \
  prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk
readelf -W -l prototypes/pocol-android/app/src/main/jniLibs/arm64-v8a/libpocold.so
readelf -W -l prototypes/pocol-android/app/src/main/jniLibs/arm64-v8a/libpocol_bridge.so
"$ANDROID_HOME/build-tools/35.0.0/apksigner" verify --verbose \
  prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk
```

The last command **must fail for this unsigned artifact**, rather than being counted as an Android build failure. Do not sign it with a new identity or install it as an update. Recorded verification also checked that each packaged native library exactly matches its fresh build bytes and all LOAD alignments are at least `0x4000`.

## Evidence integrity

```bash
cd development/security-crypto-timing-2026-11-05/evidence
sha256sum -c SHA256SUMS
```

Native build metadata hashes current source bytes and excludes Git commit IDs and timestamps. Linux and Android metadata agree on the recorded default-feature compatibility identity. Documentation-only edits or the local commit do not change that identity. Rebuilding the same sources on a different host is not claimed to reproduce the APK hash byte-for-byte.
