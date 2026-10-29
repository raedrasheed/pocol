# LP3 S0/S1 assessment for PR #10 (after Windows run 37976178670)

**Status:**

- This is the author's assessment of the evidence, not acceptance.
- No criterion here is accepted.
- S0 and S1 are not declared complete.
- A green workflow run is one input to acceptance, not acceptance itself.

The criteria are quoted from `development/LP3-MILESTONE.md` (contribution 1, S0, S1). Accepting a contribution requires sequential independent review, and LP3 needs S0–S8.

## Three kinds of evidence, kept apart

| Label | Meaning | Where |
|---|---|---|
| **A**: author verification | Run by the author in Linux cloud sessions (rounds 1–4). Windows-only checks there are BLOCKED BY PLATFORM or NOT RUN. | `author-r1/`, `author-r2/`, `author-r3-windows-ci/`, `author-r4-encoding-fix/` |
| **W**: Windows verification | GitHub-hosted Windows Server 2022, Rust 1.58.1, dispatched by the owner. It runs the author's script, so it is real Windows execution but not independent review. | `windows-run-37976178670/` (commit `4e26e4d`): PASS 20, FAIL 0, BLOCKED BY PLATFORM 16, NOT RUN 0. The failed first run is kept in `windows-run-37972422364/`. |
| **R**: independent review | Review by someone other than the author. | Round 1 (`c9364a2`) was reviewed outside this repository; the report is not in this session. Its two findings, 57 masked gsVersion mismatches and SIGABRT under a memory limit, were reproduced and fixed by the author in round 2 (`author-r2/`). **Rounds 2–4 have not been independently reviewed.** |

## Tested code

| Commit | Content |
|---|---|
| `26ee0a4` | Round 2: last change to Rust source, `Cargo.toml` or Rust tests |
| `f2ac599`, `9c02c7f` | Round 3: byte-exact harness inputs; the CI workflow and script |
| `086a3ea` | Round 4: explicit UTF-8 in the Python harnesses |
| **`4e26e4d`** | **The commit verified on Windows.** It adds only Linux smoke evidence to `086a3ea`. |
| Later commits | Documentation and evidence only: nothing under `prototypes/` or `.github/` |

## S0

> S0: LP1/LP2 regression suite and preserved M1/previous evidence; source/input provenance, exact versions/commands/exit codes and licenses.

| Element | A (Linux) | W (Windows, `4e26e4d`) | R |
|---|---|---|---|
| LP1/LP2 regression suite | Partial. On Linux, 11 `store_journal` writer tests and `e2e_store.py` are BLOCKED BY PLATFORM (the LP2 writer is Windows-only). `store_process.rs` and `exclusive_writer_in_process` are NOT RUN (not compiled). | **PASS.** Full `cargo test`: 98 ordinary tests plus the 2 allocation harnesses, with `store_journal` 12/12 and `store_process` 2/2 (the helper ignored by design). Also `verify-fixtures` (22/1027/3677, K1–K3), `e2e_http.py` 45/45, `e2e_store.py` 24/24 and `rustfmt`. The LP2 counts equal the accepted LP2 Windows evidence (`development/lp2/reviews/r2/`). | Pending |
| Preserved M1 and previous evidence | Unchanged since the LP3 base `fd7a17f`: `development/m1`, `lp1`, `lp2`, `decisions`. Each earlier LP3 evidence directory is unchanged since its own round. Round 3's README gained its smoke-run section in `9c02c7f`, within that round. The first Windows run is preserved byte-exact. | The six pinned M1 inputs had their git-blob hashes on the runner (`.gitattributes` protects `/development/m1/**`). | Pending |
| Source and input provenance | `tested-source-manifest.json` per round; pinned input hashes | The job log shows the checkout of `4e26e4d`; `trackedFilesModified: false`. All 52 `source-hashes.json` entries match the commit: 46 after CRLF checkout conversion, 6 unchanged. The differential corpus hashes equal Linux. Rechecked by `windows-run-37976178670/check_evidence.py` (0 problems). | Pending |
| Exact versions, commands, exit codes | `commands.json`, version logs per round | `environment.json` and `commands.json`: 24 commands, all exit 0, with durations. Toolchain rustc 1.58.1 msvc, cargo 1.58.0, Python 3.11.9, runner image `win22 20261004.326.1`. The `rustup` field's "1.99.0" is the runner default recorded before toolchain selection; see the run README. | Pending |
| Licenses | No new dependency | `Cargo.lock` and `fixtures/LICENSES.json` are unchanged since the LP2 acceptance `f5737e3`, so the dependency and license set is LP2's. The harnesses are Python stdlib only. | Pending |

**S0 status:**

- Evidence for every S0 element is in place for `4e26e4d`, including the Windows-only LP2 checks, which passed on Windows.
- None is counted from a BLOCKED or NOT RUN result.
- S0 is not declared complete: confirming it is the independent reviewer's role (R).

## S1

> S1: Rust full GenesisSpec six fields/52CP values and M0 entries; exact literal341-byte GSV1 roundtrip, frozen three-library Keccak/K1-K3, all L4n negative/error-order vectors; at least1500-level structurally invalid and malformed deep vectors rejected without panic/recursive-drop exhaustion. Bounded bytes before allocations; unknown/unsupported versions, integer widths/minima/enums/gamma, IDs sorted/unique/notreserved exactly match accepted schema. Do not inherit LP1 scoped decoder omissions or framing-vs-gsInt diagnostic convention.

Test names are from `tests/genesis_spec.rs`. Every test listed passed on Linux (A) and on Windows (W, within `cargo-test-full`).

| Element | Implementation and tests | A (Linux) | W (Windows) | R |
|---|---|---|---|---|
| Six fields, 52 CP values, M0 entries | `src/genesis_spec.rs`. Tests `gsv1_decodes_to_every_expected_field_and_reencodes`, `every_cp_width_minimum_enum_and_gamma_bound`, `member_list_order_reserved_ids_and_shapes`. Differential against the accepted reference. | PASS; differential 30075/30075 | PASS; differential 30075/30075, and 30077/30077 with `--max-version`, on the same corpus bytes | Round 1 only |
| Exact literal 341-byte GSV1 roundtrip | `gsv1_decodes_to_every_expected_field_and_reencodes`, `encode_round_trips_valid_specs` | PASS | PASS. The CLI also reports genesisHash `0xde518e30…eb2277` and `reencodeEqual: true` | Round 1 only |
| Frozen three-library Keccak, K1–K3 | `k1_k3_and_three_library_gsv1_hash`, `accepted_inputs_are_pinned_and_unchanged` | PASS | PASS, and `verify-fixtures` `knownAnswersK1K3: true` | Round 1 only |
| All L4n negative and error-order vectors | `l4n_and_all_supplement_negatives` (N1–N10 and 22 supplements, including O1–O5), `n11_different_genesis_hash_is_rejected`, `stage_order_across_fields` | PASS | PASS; the differential's 32 negatives all agree on code and detail | Round 1 only |
| ≥1500-level invalid and deep vectors, no panic or recursive-drop exhaustion | `c30_deep_and_shallow_twins_on_a_small_stack` (13 C30 cases at depth 1500 plus twins, on a 128 KiB thread stack), `extreme_depths_keep_the_shallow_result` (5000, 100000), `seeded_corpus_never_panics_and_accepts_only_canonical_bytes` (20000 mutants) | PASS | PASS on the Windows stack; the differential's 26 C30 inputs and the deepest nesting within `MAX_SPEC_BYTES` agree | Round 1 only |
| Bounded bytes before allocations | **Decoder:** no allocation is sized from a declared length; framing uses an end-offset stack. **CLI:** fixed buffers reserved with `try_reserve_exact`, and lines above `MAX_SPEC_BYTES` are refused. Tests `max_spec_bytes_is_the_largest_valid_encoding`, `tests/genesis_batch_alloc.rs`, `tests/lp3_genesis_cli.py`. | PASS: the allocation test, plus every CLI case under `RLIMIT_AS` (64/256/8 MiB), with no abort | Allocation test PASS in debug (6/6) and release (7/7), with peaks byte-identical to Linux. The CLI checks that need no limit PASS. **The 8 cases that need an OS memory limit are BLOCKED BY PLATFORM** (15 rows plus 1 summary row) and evidenced on Linux only; the reservation-failure exit has no Windows coverage. See `windows-run-37976178670/blocked-checks.json`. | Round 1 finding (SIGABRT) fixed in round 2; **fix not re-reviewed** |
| Versions, widths, minima, enums, gamma; IDs sorted, unique, not reserved | `version_chain_id_and_root_fields`, `every_cp_width_minimum_enum_and_gamma_bound`, `member_list_order_reserved_ids_and_shapes`, `member_count_relations_are_param_gate_not_decode`, `gs_version_detail_is_the_exact_decimal_at_every_length` | PASS; differential with no exception (round 2) | PASS; differential with no exception, including two full-size 2,818,466-byte gsVersion items | Round 1 finding (57 masked mismatches) fixed in round 2; **fix not re-reviewed** |
| No LP1 scoped-decoder omissions or framing-vs-gsInt convention | A wrapped single byte (`81 xx`) is gsInt, not L0. The LP1 decoder `src/genesis.rs` is untouched and not used by S1. Mutation control M3 (LP1 convention reinstated) is caught by tests and the differential (2403 mismatches). | PASS; mutation controls 12/12 | PASS (no mutation run on Windows) | Round 1 only |

**S1 status:**

- Every S1 element is implemented and author-verified, on Linux and on Windows at `4e26e4d`.
- **One platform gap:** OS-limit behaviour is verified on Linux only, and on Windows at heap level only.
- Independent review of rounds 2–4 is outstanding.
- S1 is not accepted.

## Remaining requirements, precisely

1. **Independent review of PR #10 at `4e26e4d`, for S0 and S1.** The reviewer must:
   - confirm the S0 and S1 rows above against the evidence;
   - confirm with their own checks that both round-1 findings are closed. The gsVersion differential now has no exception and 0 mismatches. `genesis-decode` is bounded, with no abort under the limits tested;
   - review the round 3–4 harness changes, the CI script and its BLOCKED/NOT RUN classification.

   The LP2 precedent (`development/lp2/reviews/r2/REVIEW-LP2-R2.md`, `LP2-ACCEPTANCE.json`) records such a review with the reviewer's own checks and an acceptance record. Rounds 2–4 have had no review of this kind.
2. **A decision on the Windows memory-limit gap.** It belongs to the reviewer or owner, not to the author. The choice is between:
   - accepting the Linux `RLIMIT_AS` evidence plus the Windows heap-level allocation evidence for "bounded bytes before allocations"; or
   - requiring a Windows OS-level limit check, for example a Job Object memory limit. That would be new harness work and another Windows run.

   The criterion text does not name a platform, and the author does not decide this.
3. **Optional cleanups, non-blocking, from the run's logs** (`windows-run-37976178670/README.md`, observations 2–4):
   - the `rustup` field recorded before toolchain selection;
   - the `{` detail on the CLI parent rows;
   - about 15 runner minutes spent computing a decimal for a check that is then BLOCKED.

   Any of these changes the tested tree and would need another Windows run. They are best batched with any review findings.
4. **Merge decision:** after review, the owner decides whether PR #10 merges into `work/lp3-consensus-foundation`. Nothing has been merged.
5. **Outside this increment (not started):**
   - the rest of contribution 1: membership, timing, nonce assignment and Draw APIs (S3, S4);
   - S2 and S5–S8.

   LP3 acceptance needs S0–S8 all supported.
6. **S9:** the Arabic dashboard and draft PR #10 are updated in this cycle.
