# LP3 S1 first increment: GenesisSpec v1 decoder (author evidence, round 1)

**Status: authored and self-tested only. Not independently reviewed. Not LP3 acceptance. S0 is not complete.**

Mode: manual, owner-prompted cloud session. The local coordinator, broker and automatic author/reviewer dispatch were not invoked. Base: `work/lp3-consensus-foundation` at `fd7a17fd51691b275343b0cbc96bfce1d2f90745` (LP2 accepted at `f5737e3`; the LP3 commits change documentation only, so the crate source is identical to LP2). Accepted M1 0.32 and all earlier evidence are unchanged.

## Scope

Implemented (S1 subset, `prototypes/lp1-node`):

- `src/genesis_spec.rs`: `decode`, `encode`, `genesis_hash` and `verify_hash` (the genesisHash part of R24(a), L4n N11) for `GenesisSpec = RLP([specVersion, chainId, CP, allocRoot, M_0List, sysCodeHash])`. All six fields, the 52 CP values with widths, minima, the nonceMode enum, `S_max <= 256`, `alpha_bp <= 10^4`, `gamma_bp <= 10^4 - alpha_bp`, M_0List entries, strictly ascending ids, and reserved ids (zero, SYSTEM_ADDRESS, `0x00..00C0C001`–`0x00..00C0C0FF`).
- Evaluation order and details follow the accepted reference (`m1-draft-0.21/tools/netprofile_ref.decode_genesis` with the `m1-draft-0.22` C30 explicit-stack parser): L0, structure, gsVersion, gsCount, gsInt, gsLen, gsRange, gsOrder, gsSys. A wrapped single byte (`81 xx`, xx < 0x80) is judged as gsInt, not L0. The LP1 scoped decoder `src/genesis.rs` is unchanged and still used by the LP1 fixture profile.
- Resources: explicit stack and a flat node arena, so parsing and dropping do not recurse. No allocation is sized from a declared length; memory is linear in the input length.
- `src/main.rs`: `genesis-decode --in FILE [--out FILE]`, one hex input per line, for differential checks (same pattern as `asert-batch`). No fault options.
- `tests/genesis_spec.rs` (14 tests) and `tests/lp3_genesis_diff.py` (differential against the reference).

Not implemented here, by design: ParamGate (S5, including `M_min <= |M_0List| <= M_max`, which is R12 per DG-V3-5), ForkSchedule and profile/node-kind gates (S2), timing (S3), membership/Draw (S4), the TypeScript model and Rust/TS literal match (S6), AS*/ASQ/ASB (S7), allocRoot / sysCodeHash recomputation and bootability (LP4). A decoded spec is identity only; `genesis-decode` prints `"bootable":false`.

## Inputs (read in place, SHA-256 pinned, not copied)

| File | SHA-256 |
|---|---|
| `development/m1/m1-draft-0.21/vectors/v3-gsv1.json` | `88097ee2…056f32` |
| `development/m1/m1-draft-0.22/vectors/c30-depth.json` | `728d8b4a…fb72464` |
| `development/m1/m1-draft-0.22/vectors/v3-e05-supplement.json` | `cd8ead8d…ce0cdb` |
| `development/m1/m1-draft-0.21/tools/netprofile_ref.py` (oracle) | `ca94ef34…1690e5` |
| `development/m1/m1-draft-0.22/tools/iterative_parse.py` (oracle) | `742e1936…eb7034a` |

Full hashes: `tested-source-manifest.json`.

## Environment

Linux x86_64 cloud container; rustc/cargo 1.58.1 (the accepted LP1/LP2 version), installed with rustup. Dependencies fetched once with `cargo fetch --locked` (network), then every build and test ran `--offline --locked`. `Cargo.lock` and the dependency set are unchanged, so no new licenses. Python 3.13.16 (root used 3.11 for M1). The accepted LP2 evidence ran on Windows.

## Results (Linux)

Exit codes and durations: `commands.json`. Baseline before any change: `baseline-fd7a17f/`.

| Check | Baseline | After change | Status |
|---|---|---|---|
| `cargo build --offline --locked` | exit 0 | exit 0 | PASS |
| `cargo fmt -- --check` | exit 0 | exit 0 | PASS |
| lib unit tests | 39/39 | 44/44 (5 new) | PASS |
| `tests/adversarial.rs` | 5/5 | 5/5 | PASS |
| `tests/asert_alloc.rs` | 1069 calls, 0 allocations | same | PASS |
| `tests/fixtures_integration.rs` | 12/12 | 12/12 | PASS |
| `tests/genesis_spec.rs` (new) | — | 14/14 | PASS |
| `tests/http_loopback.rs` | 2/2 | 2/2 | PASS |
| `tests/store_journal.rs`, Linux-only test `writer_is_explicitly_unsupported_off_windows` | 1/1 | 1/1 | PASS |
| `tests/store_journal.rs`, 11 writer tests (P1–P4, write/sync poisoning) | fail at `Writer::open`/`init` with `UnsupportedPlatform` | same | **NOT RUN (Windows-only)** |
| `tests/store_journal.rs::exclusive_writer_in_process` | `#[cfg(windows)]`, not compiled | same | **NOT RUN (Windows-only)** |
| `tests/store_process.rs` (2 tests + helper) | `#![cfg(windows)]`, 0 tests compiled | same | **NOT RUN (Windows-only)** |
| `verify-fixtures` | provenance, profile, chain 20, windows 22/22, ASERT 1027/1027, hash 3677/3677 | same | PASS |
| `tests/e2e_http.py` | 45/45 | 45/45 | PASS |
| `tests/e2e_store.py` | aborts: `store init` exits 2 `unsupportedPlatform` | same | **NOT RUN (Windows-only)** |
| `tests/lp3_genesis_diff.py` (new) | — | 30059/30059 agree | PASS |
| Mutation controls (8 faults) | — | 8/8 detected by both | PASS |

`cargo test` as a whole exits 101 on Linux only because of the 11 Windows-only writer tests. Their exact reason: `src/store.rs` `writer_options` returns `UnsupportedPlatform("exclusive single-writer enforcement is implemented only on Windows (share_mode 0); no portable lock is claimed")` off Windows, by LP2 design. Platform locking was not changed. These checks are not counted as passed.

### What the new tests cover

- K1–K3; GSV1 rebuilt from the `validation.md` literal segments equals the item tree; 341 / 179 / 88 / 338 bytes; SHA-256 and the three-library H_GSV1 `0xde518e30…eb2277` from the E05 supplement.
- GSV1 decodes to every expected field (chainId 777902, all 52 CP values from exact JSON lexemes, roots, both members) and re-encodes to the same bytes.
- All 32 GSV1 negatives (N1–N10 and 22 supplements, including O1–O5 order pairs), with the three positive edges; N11 through `verify_hash`.
- All 13 C30 cases at depth 1500 and their shallow twins (lengths, code and detail), then at depths 5000 and 100000, each on a 128 KiB thread stack.
- Per CP field: largest value of the width, 2^width, 0, 1, minimum, upper bound and one above, wrapped / leading-zero / zero-byte integers; gamma against alpha; version, chainId and root length cases; member order, duplicates and reserved boundaries; cross-stage order pairs.
- 20000 seeded byte mutants: no panic, and every accepted input re-encodes to itself.

### Differential against the accepted reference

`tests/lp3_genesis_diff.py` loads the M1 reference decoder read-only, installs the C30 parser as the 0.22 runner does, and compares both sides on GSV1, the 32 negatives, the 26 C30 inputs, 20000 structured edits of the GSV1 tree and 10000 byte mutants (seed `0x4c503301`). It compares the code and detail of every rejection, and genesisHash, chainId, member count and re-encoding for every accepted input. Result: 30059/30059 agree, all nine codes plus `ok` exercised (`lp3-genesis-diff.json`). The oracle is independent code, but the author ran it: this is not an independent review.

## Decisions and dependencies

- No unresolved owner decision blocks this increment. The open owner items (U01, U02, U10, U14, CR-M1-01) concern the factory address, revoked-content display, publisher rights, request timeouts and state-proof reads. U01 affects system code and sysCodeHash recomputation (LP4), not decoding.
- `gsStructure` is the accepted reference convention P-V3-2 for an error name the source omits (DG-V3-4). It is a reference/delegated convention, not an owner approval.
- `M_min <= |M_0List| <= M_max` is left to ParamGate R12 (DG-V3-5), as in the reference; a test records that decode accepts it.
- `gsVersion` detail: decimal like the reference; for values >= 2^128 the Rust side prints a fixed string instead of a long decimal (bounded output). The differential documents this one comparison rule.
- Input-size bound: the decoder takes an in-memory slice and is linear in it; there is no file or profile loader in this increment, so no input cap is added. A loader must bound what it reads.

## Outstanding before any S1 or S0 acceptance

1. Windows run of the full suite at this commit: `cargo test --offline --locked -- --test-threads=1 --nocapture`, `tests/e2e_store.py`, and `tests/store_process.rs` (exclusive writer, kill release, crash recovery).
2. Independent review of the decoder, tests and differential.
3. Remaining S1 items outside this increment: TS literal match (S6) and any review findings.

No merge, deployment, funds or acceptance claim.
