# libsecp256k1 0.5.0 → k256 0.13.4: equivalence evidence (D-DEP-SECP)

## Scope and reachability
- `libsecp256k1 = "=0.5.0"` was a direct dependency of lp1-node and pocold.
- Its only use was `hashes::recover_address` (lp1-node `src/hashes.rs`, compiled into pocold by `#[path]`).
  That function recovers the signer of header proposer signatures and winner signatures, so it is
  consensus-relevant.
- No product code signed with libsecp256k1: pocold signs with lp4-execution `txsign` (k256).
- `rand 0.7.3` (RUSTSEC-2026-0097, alias GHSA-cq8v-f236-94qc; conditional on the `log` feature, which the
  feature tree does not enable) was reachable only as a dependency of libsecp256k1. It leaves pocold and
  stays in lp1-node.

## Change
- `recover_address` keeps the unchanged `check_sig_shape` filter (r ∈ [1, n), s ∈ [1, n/2], v ∈ {0, 1},
  length 65). It then calls `crate::secp_backend::recover_uncompressed`, which each crate supplies:
  - pocold `src/secp_backend.rs`: `k256::ecdsa::{Signature::from_slice, RecoveryId::from_byte,
    VerifyingKey::recover_from_prehash}`;
  - lp1-node `src/secp_backend.rs`: the original libsecp256k1 calls, moved verbatim.
- Error mapping is kept: unparsable → RRange, bad id → BadV, no point → NotRecoverable.
- lp1-node keeps libsecp256k1 because of its accepted Rust 1.58.1 toolchain. The k256 0.13 tree needs
  ≥ 1.65 (base64ct 1.8.3: 1.85). Its Cargo.toml and Cargo.lock are unchanged from 090eae4.

## Equivalence run
- Tool: `development/audit-2026-10-22/tools/secp-equiv` (both crates linked; source SHA-256
  `6cfd6fb601c69d65473786f8e9390792c1219dc77e180f60412c7a98d1bb1023`).
- Toolchain: rustc/cargo 1.97.0; release build, offline, versions as locked in pocold at 51aa8f7.
- Command: `secp-equiv 50000`. Exit status 0. Run time 56 s.

| Corpus | Cases | Result |
|---|---|---|
| A honest signatures: k256 RFC 6979, low-s normalized; both libraries recover; compared with the signer's address | 50 000 | all equal to the signer |
| A′ RFC 6979 signing determinism: libsecp256k1 `sign` vs k256 `sign_prehash_recoverable` (low-s) | 50 000 | byte-identical (r, s, recovery id) |
| B arbitrary r, s, v (half with s < 2^255), arbitrary message | 100 000 | identical verdicts: Ok 25 170, NotRecoverable 24 785, HighS 24 988, BadV 25 057 |
| C boundaries: r ∈ {1, 2, 5, n−1, n, n+1, p−1, 2^256−1, 0, hash}; s ∈ {1, n/2, n/2+1, n−1, 0, hash}; m ∈ {0, 1, n−1, n, n+1, 2^256−1, hash}; v ∈ {0, 1, 2} | 1 260 + 3 lengths | identical: Ok 126, NotRecoverable 84, RRange 630, SRange 105, HighS 210, BadV 105 |
| D honest signatures over messages n−1 … n−200 | 200 | equal to the signer |

Mismatches: **0**. Raw output: `{"cases":{"A honest signatures":50000,"B BadV":25057,"B HighS":24988,"B NotRecoverable":24785,"B Ok":25170,"C BadV":105,"C HighS":210,"C NotRecoverable":84,"C Ok":126,"C RRange":630,"C SRange":105,"C lengths":3,"D messages near n":200},"mismatches":0,"first":[]}`.

## Regression vectors after the change
- lp1-node: all suites pass, including the accepted fixtures `fixtures_integration::a3_signatures_and_hash_oracle`
  and `adversarial_lp3`. They ran on the lp1-node backend, and also on k256 during the reverted first attempt.
  - `store_journal` (11 tests) is BLOCKED BY PLATFORM. It fails identically on the audited snapshot
    090eae4: `UnsupportedPlatform("exclusive single-writer enforcement is implemented only on Windows")`.
- pocold and lp4-execution suites and the live devnet runs: see the integration record.

## Not replaced
- `derivative 2.2.0` and `paste 1.0.15`: unmaintained notices for build-time proc-macros, reached through
  revm 19.7.0 / alloy-primitives (ark-ff, alloy). No vulnerability is reported for them.
- Replacing them requires a revm upgrade, which is a broad, consensus-relevant change (EVM semantics and
  roots). It is outside this repair. Kept as maintenance risks.
