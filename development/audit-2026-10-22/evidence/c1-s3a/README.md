# C-1 S3a: retained canonical states as diffs (2026-10-24)

This is part of D-1 option (a), approved by the owner. The design is in proposals/C1-STORAGE-DESIGN.md,
sections "S3 refinement" and "S3a status".

**Unchanged:** validity, roots, receipts, genesisHash, the wire format and every RPC answer.

**What changes:** which states are held in full and which are rebuilt on demand.

## Code

- **`prototypes/pocold/src/statediff.rs` (new):**
  - `StateDiff::between`, `apply_before` and `apply_after` turn each state into the other exactly;
  - `states_equal` compares every field, the cached code included.
- **`src/chain.rs`:**
  - `Entry.full` and `Entry.diff`;
  - `validate` computes the block's diff and materializes a non-full parent itself;
  - `retain` is split into `retain_unsettled` and `settle`;
  - `settle`:
    - keeps only the head, genesis and the side states within side_max full;
    - makes every kept side state full;
    - sheds the full state of every other canonical block above the cut;
  - `materialize`, `state_of` and `full_entry`;
  - `ensure_state_with` materializes before it re-executes;
  - `restore_canonical` gives adopted blocks their missing diffs when a retained state lies below them;
  - `keep_full_states` is the reference behaviour, for tests only.
- **`src/node.rs`:**
  - live import and replay run `retain_unsettled`, then `restore_canonical`, then `settle`;
  - production takes its parent through `full_entry`;
  - `Limits::full_states` is for test references.
- **`src/rpc.rs`:** state queries and `eth_call` take their state through `state_of`.
- **`src/resources.rs`:**
  - the snapshot terms: head, genesis, side_max, 2 materialized, transient, growth;
  - the diff term (`diffs_held` × `diff_max`, derived from the gas schedule);
  - the `s3a-removal` condition.

## Results

| | |
|---|---|
| devnet model | 4,323,240,949 → **3,601,500,638 B** (`a13-devnet-breakdown-s3a.log`) |
| network model | 90.93 → **41.41 GiB**, gate 47.33 GiB (`a13-network-profile-s3a.log`); snapshots 75.12 → 27.76 GiB, diffs 0.46 GiB |
| validation at the near-cap devnet state (`validation-near-cap-s3a.log`) | 380 transfers 672 ms (655 ms before S3a, C-2 final), empty child 491 ms (465 ms); the diff alone 12.5 ms |
| equivalence (`s3a-tests.log`) | 8,388 states equal to a full-state reference over 374 steps. Every pruned flag is identical, and so is the rebuild count (20 and 20). At most 4 full states, against up to 35. Every state RPC is equal at every height and tag, live, after two reorgs and after reopen |
| ignored deep-reorg tests (`consensus-ignored-s3a.log`, on `bef6c4f`) | 2048/2049, D133 at 400, A11 deep reorg: 3 of 3 pass |
| mutation controls (`mutants.log`, `mutants*.json`) | final pass on `4b49d03`: 9 of 11 killed; SM8 equivalent; SM10 a guard on a case that no longer occurs |
| suites (`suite-*.log`, `4b49d03`) | lp4 173 passed / 0 failed; pocold 194 passed / 0 failed / 17 ignored |

## Findings made during the work (all fixed; proposals/C1-STORAGE-DESIGN.md "S3a status")

1. **An adopted branch whose blocks were pruned as side blocks broke the path to the states below the
   fork.** Those heights would have answered −32017. Fix: the restore gives those blocks their diffs, and
   it runs before `settle`.
2. **That restore at first rebuilt a deep adopted branch from genesis.** Fix: it is limited to the case
   where a retained state lies below the block. The rebuild count is now equal to the reference's.
3. **A kept side state could be held as a diff only and become unreachable** (found from the surviving
   mutant SM10 of the first pass). Fix: kept side states are made full, as they all were before S3a. The
   equivalence test now covers this case and fails on the earlier behaviour (SM11).

## Open

- The ignored deep-reorg tests ran on `bef6c4f`. The S3a fix in `4b49d03` changes `settle`, so they are
  re-run in the final acceptance pass.
- Genesis is counted as a full snapshot. Its real size is the genesis allocation, so a tighter term is
  possible.
- S3b (the 10 transient copies as overlays) needs an executor change, designed with S5.
