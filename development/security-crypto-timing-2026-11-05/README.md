# Private signing timing remediation evidence

See [the Arabic review](REPORT-AR.md) for the change, compatibility edge case, results, and remaining work. This is a local follow-up to the earlier security delivery, not a claim that the audit or a release is complete.

- Product change: `prototypes/lp4-execution/src/txsign.rs` and its manifest; locked resolution is updated for execution, pocold, and the fuzz harness.
- Regression: `prototypes/lp4-execution/tests/signing_compat.rs`, using public synthetic keys and an independent C libsecp256k1 RFC6979 oracle.
- `check_signer_graph.py` checks routing/version floors offline. It reports legacy dependencies separately and does not suppress them.
- [Reproduction commands](COMMANDS.md) and [provenance](evidence/provenance.json) identify the source, toolchain, advisory database, test counts, and build identity.
- `evidence/SHA256SUMS` covers the evidence files only. Raw logs are preserved, including the initial failed assumption about legacy byte equivalence. That log is an investigation artifact, not the final test outcome.
- Unchanged tool logs and the copied license retain their original trailing spaces/blank lines. The full staged `git diff --check` reports those evidence bytes; the strict source/documentation check excludes `evidence/` and passes. These exceptions are recorded in `evidence/final-validation.json`; no global Git whitespace setting was changed.
- Cargo-audit returns **1**, because legacy public recovery/EVM still includes crypto-bigint 0.5.5. No audit ignore was added. The updated private signing closure uses 0.7.10.
- The Android APK is a local unsigned build artifact; it is not included in Git. Its hash, native library hashes, and 16KB alignment checks are recorded. No device runtime or Windows native result is claimed.

## Advisory attribution

`evidence/RUSTSEC-2026-0387.md` is reproduced **unchanged** from the RustSec advisory database at commit `26fd2d43f7280a1fb925a3743ec463017afe7119`, path `crates/crypto-bigint/RUSTSEC-2026-0387.md`.

Source: https://github.com/RustSec/advisory-db/blob/26fd2d43f7280a1fb925a3743ec463017afe7119/crates/crypto-bigint/RUSTSEC-2026-0387.md

Advisory: https://rustsec.org/advisories/RUSTSEC-2026-0387.html

The advisory declares **CC-BY-4.0**. Attribution belongs to its authors/contributors in the source history. The license is included as `evidence/RUSTSEC-CC-BY-4.0.txt`; see also https://creativecommons.org/licenses/by/4.0/. This copy does not imply endorsement. The upstream fix is https://github.com/RustCrypto/crypto-bigint/pull/1365 (merge commit `c34044b8a1479ef32bec0a9030cf2f07b06506d9`).

The raw cargo-audit JSON also contains RustSec advisory descriptions with their original license/source fields intact. RustSec's default CC0 terms and the explicit advisory license exceptions continue to apply to that content.
