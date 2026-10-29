# C-1 S4: old canonical block metadata slimmed in memory (2026-10-24)

This is part of D-1 option (a), approved by the owner. The design and its status are in
proposals/C1-STORAGE-DESIGN.md, sections "S4" and "S4 status".

**Unchanged:** validity, roots, receipts, genesisHash, the wire format and every RPC and P2P answer.

**What changes:** canonical journaled entries deeper than `slim_depth` keep only their fork-choice
fields, scalars and diff. The dropped fields are restored exactly when they are needed.

## Code

- **`prototypes/pocold/src/history.rs`:** a third table, RANGES (`block → seal ‖ (20+16+16)·n`). It is
  sealed like the receipts and written in the same transaction, before the journal append (I3).
  `has` now needs both receipts and ranges.
- **`src/chain.rs`:**
  - `Entry.slim`, `slim_depth`, `slim_cursor`, `unslimmed`;
  - `retain_unsettled` slims canonical journaled entries deeper than `slim_depth`, and slims again the
    restored entries that are still deep enough;
  - `restored_fields`, `restored`, `meta_restored` and `unslim`:
    - the header comes from the journal record and must hash to the block;
    - the ParentResult is decoded from the same record;
    - the ranges come from history.redb;
  - `validate` answers NeedsRebuild for a slimmed parent, and so does the type-2 evidence walk for a
    slimmed ancestor;
  - `ensure_state_with` re-executes slimmed blocks from restored copies.
- **`src/node.rs`:**
  - `slim_depth` defaults to 2·R_keep + E_win + 257 (`Limits::slim_depth` sets it for tests);
  - the NeedsRebuild retry loops in replay and live import restore the entry first, at most
    `REBUILD_TRIES` (17) times;
  - `view` and P2P `block_msg` restore a slimmed entry instead of serving it from memory;
  - status field `unslims`.
- **`src/resources.rs`:** the history group splits the old entries into offloaded ones (between the
  window and `slim_depth`) and slimmed ones (deeper). The ranges are added to the redb payload.

## Results

| | |
|---|---|
| devnet model (`a13-devnet-breakdown-s4.log`) | 3,601,500,638 → **3,322,680,105 B** |
| network model (`a13-network-profile-s4.log`) | 41.41 → **37.96 GiB**, gate 43.38 GiB; history group 4.28 → 0.83 GiB |
| S4 tests (`s4-tests.log`) | 2 of 2 pass (the tests are described in the design's "S4 status") |
| mutation controls (`mutants.log`, `mutants.json`, tree `13aeba7`) | the base passes; **9 of 9 mutants killed** |
| suites (`suite-lp4.log`, `suite-pocold.log`, release, `13aeba7`) | lp4 173 passed / 0 failed; pocold 196 passed / 0 failed / 17 ignored |
| ignored deep-reorg tests (`consensus-ignored-s4.log`, `13aeba7`) | 2048/2049, D133 at 400, A11 deep reorg: 3 of 3 pass |

The mutants:
1. views do not restore a slimmed entry;
2. validate reads a slimmed parent as it is;
3. the ranges are not stored;
4. unslim does not restore the ParentResult;
5. the evidence walk reads a slimmed ancestor as it is;
6. slimming at half the depth;
7. restored entries are never slimmed again;
8. P2P serves a resident slimmed entry from memory;
9. a slimmed block is re-executed from its slimmed copy.

Each mutant is listed in `mutants.log` with its exact replacement. The mutation runner is kept outside
the repository.

## Open

- A slimmed entry is still about 5 KB in the model: three times the inline `Entry` for map growth, plus
  an empty state. A smaller inline entry is step E5 of the S5 + S3b design.
