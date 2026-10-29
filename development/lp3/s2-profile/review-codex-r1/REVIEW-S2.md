# Separate Codex S2 review — 2026-10-17

## Verdict

**PASS: the library implementation at f7b521591fa5b3d9a039c8157e0a50d1acee2a86 meets the scoped S2 criteria**, with the limitations below. No blocking implementation finding. This is a new sequential Codex review pass treating author claims as untrusted; it is not an external reviewer identity or a formal GitHub approval. It does not accept S3 or complete contribution 1/LP3.

## Method and reference

Reviewed profile.rs, its callers, accepted genesis_spec.rs and tests directly against LP3-MILESTONE.md S2, M1 reference/consensus.md:7–34 and ForkSchedule definition, governance.md network identity and R22/R24. No AGENTS.md found under /workspace. Author report, tests and hashes were inspected but not treated as sufficient proof. Tests used Rust 1.58.1/Cargo 1.58.0, Linux x86_64, offline unchanged lockfile/dependencies.

`cp-audit.json` records all 52 manually transcribed names, widths, minima and source indices, plus the accepted reference hash. The reviewer table is not imported from CP_SCHEMA. `generate.py` produces `review_checks.rs`; it uses explicit named-field assertions and independently enumerated width/minimum/upper-bound expectations. Every CP narrowing occurs after schema validation; U256 values remain full width. CP-schema and wire-index order match consensus.md. Upper bounds nonceMode=1, S_max=256, alpha_bp=10000 and gamma_bp=10000-alpha_bp are exercised at and beyond boundaries. Integer widths and minima are tested across every field, with U256::MAX on all four wide fields.

## ForkSchedule, identity and gate review

- Schedule construction enforces nonempty, first (1,0), strictly increasing heights and version membership before copying. The private vector prevents bypass through public struct construction. Every u16 version value was tried at a later activation: only v1 succeeds. Heights 0/1/50/u64::MAX and descending/duplicate schedules were tested. The lookup unwrap is safe because all valid schedules start at zero. Since only v1 exists, boundary queries do not demonstrate a future multi-version transition; that needs testing when another version is implemented.
- GenesisSpec decode/hash check precedes advertised chainId equality and gates. chainId is a u64 and decode rejects zero. Every hash byte was tampered individually; every allocRoot/sysCodeHash byte was changed with both stale hash rejection and fresh hash identity-only acceptance. ForkSchedule stays outside the genesis hash, per M1.
- All 30 named profile/node/chain combinations were checked, including production-node rejection of simnet/synthetic and reserved network chainId 777001 separation. Enums are internal API types, not an untrusted JSON parser. Full ParamGate/error-list ordering is not claimed.
- ProfileIdentity exposes immutable references and has no boot-enabling path. External compile-negative probes prevent mutation of its kind and construction of an invalid ForkSchedule. Rebinding arbitrary roots never changes bootable=false. Real builder verification remains mandatory before future boot. Mutable public ConsensusParams alone is raw data, not a validated capability; it cannot mutate a ProfileIdentity through its public API.

## Fresh results and outcome classification

| Check | Status | Evidence |
|---|---|---|
| Clean offline build; rustfmt | PASS | build.log, fmt.log |
| Library 49; fixtures integration 12; GenesisSpec 16; S2 6 | PASS (83 total) | regression.log |
| Five new reviewer test groups: explicit field mapping/bounds, all 65536 version values, binding/gates, 1000 seeded profiles | PASS | review_checks.rs, review-checks.log |
| Fixture verification: window 22, ASERT 1027, hashes 3677, K1–K3 | PASS | verify-fixtures.log |
| Explicit Linux unsupported-writer behavior | PASS (1 test; 11 filtered Windows cases are not counted) | linux-store-explicit-rejection.log |
| Two private-state compile-negative probes | PASS (expected compiler exit 1) | privacy-negative.log, privacy-profile-negative.log |
| Protected tree comparison and source identity | PASS | preservation.json |
| Author evidence SHA256SUMS | PASS | author-checksums.log |
| Initial shared-cache regression | FAIL, environment cause corrected | stale-cache-regression-FAIL.log, stale-cache-commands.json |
| New Windows execution/writer/crash checks | BLOCKED BY PLATFORM | Linux host; no Windows run claimed |
| Full S1 differential/allocation reruns | NOT RUN | Source unchanged; accepted historical evidence retained |
| ParamGate/builders/boot, S3–S8 | NOT RUN | Outside S2 scope |
| Live PR metadata/checks | NOT RUN | No usable GitHub API access established; PR number not inferred from placeholder |

Expected compiler rejections are passing negative controls, not implementation test failures. 65536 version values and 1000 seeded profiles are cases within the five reviewer tests, not extra libtest test counts. Filtered tests are not passes.

### RV1 — nonblocking reproducibility issue

The initial cargo build reused the author's shared target directory, including outputs from temporary mutation copies. The first regression attempt passed 49 library tests but failed all 12 fixture tests because an embedded CARGO_MANIFEST_DIR pointed into a deleted temporary mutation checkout. Cargo stopped before the remaining selected targets. This is preserved as FAIL, not hidden or relabeled PASS.

Diagnosis: the author verify.py shares a CARGO_TARGET_DIR among original and scratch builds. A fresh target `/workspace/pocol-s2-review/target` rebuilt the reviewed, unchanged source and all 83 tests passed. Reviewer-specific tests then passed against that clean library. This closes the environment failure for this review; future mutation harnesses should isolate targets per copy. The historical author harness/evidence was deliberately not rewritten. No S2 code fix was needed.

## Preservation and reproduction

`preservation.json` verifies M1, LP1/LP2 and S1 evidence, decisions, .github and dependency manifests/lockfile unchanged from foundation merge d17273af35880f0e1d8a438e529e69c22a18f731. It also confirms all prototypes/ and .github/ blobs at f7b5215 equal author-tested a9f0460. Prior author evidence was checked by checksums and remained unchanged. No accepted specification or implementation was modified in this pass.

The actual runner `run.py` records command arrays, working directories, raw exits and durations in commands.json. `generate.py` must run before it, with files under /workspace/pocol-s2-review as shown in the script. Run it with a fresh target directory, not shared mutation outputs. Only .rs/.py/.json/.log evidence is retained here; binaries/dependencies/build caches are excluded.

Remaining limits: v1-only schedule, library-only typed identity API, no builder facts or boot authorization, no full ParamGate, no new Windows run, no external reviewer/GitHub approval. These do not prevent the scoped technical S2 PASS; they prevent claiming complete LP3 or boot readiness. Do not merge or start S3 from this review alone.
