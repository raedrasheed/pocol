# C-1 E2: the per-block whole-state checks from a state summary (2026-10-24)

This is step 2 of the S5 + S3b design (proposals/C1-STORAGE-DESIGN.md, sections "E2 design" and "E2
status"), under D-1 option (a), which the owner approved.

**Unchanged:** validity, every root, receipt and RPC answer, and the Ok/Err outcome of every check.

## Code

- **`prototypes/lp4-execution/src/state_summary.rs` (new):** `Summary`, with:
  - `scan`: the whole state, counted as the old walks count it;
  - `moved`: carries a summary through the changed keys;
  - `code_check`;
  - the incremental `drop_unreferenced_code`.
- **`src/genesis_builder.rs`:** `GenesisImage::verify_summary`, the candidate check from a summary (the
  fixed accounts are still read).
- **`src/apply.rs`:** `apply_block_with(parent, parent_trie, parent_summary, …)`; `BlockOutput.summary`. With a
  summary, these read summaries and no longer walk the state:
  - the parent's check;
  - the SYSTEM calls' starting code total;
  - the USER-phase code check;
  - the final candidate check;
  - the code drop.

  In lp4's tests and under `root-check`, every summary is compared with `scan`, and every outcome and
  the contracts map with the old walks.
- **`prototypes/pocold/src/chain.rs`:** `Entry.summary` (boxed; a slimmed entry drops it); genesis's from
  `scan`.

## Results

| | |
|---|---|
| validation at the near-cap devnet state (`validation-near-cap-e2.log`) | empty child 262 → **62 ms**; 380 transfers 349 → **174 ms**; CREATE2-heavy 290 → **122 ms**; 149 one-create transactions 321 → **154 ms** (E1 → E2) |
| devnet model (`a13-devnet-breakdown-e2.log`) | 3,486,909,657 → **3,489,051,177 B** |
| network model (`a13-network-profile-e2.log`) | 40.20 GiB, gate 45.94 GiB |
| E2 tests (`e2-tests.log`) | pass |
| suites (`suite-lp4.log`, `suite-pocold.log`, release, `1774b90`, reference checks on) | lp4 184 passed / 0 failed; pocold 199 passed / 0 failed / 17 ignored |
| mutation controls (`mutants.log`) | 12 mutants on `1774b90`. The base passes; 8 are killed in pass 1. Three survivors were then killed by added tests: E2M4, E2M9 and E2M11. **11 of 12 killed**; E2M8 cannot be reached (below) |

The survivors of pass 1:
- **E2M4** (the drop is skipped although the contracts map grew): execution never adds unreferenced code,
  because revm's commit skips a destroyed account's code. A constructed state now tests it.
- **E2M9** (the system programs counted as user accounts): this only matters at exactly ACCT_CAP.
  `e2_failing_candidates_fail_alike` now also passes states at exactly ACCT_CAP, SLOT_CAP and
  SYS_SLOT_MAX, alike with and without the summary.
- **E2M11** (the SYSTEM calls start one byte above the parent's code total): this only matters when the
  parent is exactly at CODE_CAP. `e2_code_cap_reached_exactly_with_the_summary` creates code up to
  CODE_CAP exactly, then applies a child, with and without the summary.
- **E2M8** (the USER phase's code check result is dropped): **not reachable.**
  - Within the USER phase, code enters only through CREATE, whose `create_return` hook enforces
    B_code_max and CODE_CAP.
  - Under Cancun, code leaves only within its creating transaction.
  - So the post-USER check cannot fail after real execution, as C-2 item 1 already established.

  The check is kept as a defence. The mutant is recorded as equivalent under Cancun execution.

The tests added after pass 1 are in `prototypes/lp4-execution/src/state_summary/tests.rs` and
`src/apply/tests.rs` (commit after `1774b90`).
