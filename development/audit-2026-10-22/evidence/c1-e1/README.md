# C-1 E1: the incremental state root (2026-10-24)

This is step 1 of the S5 + S3b design (proposals/C1-STORAGE-DESIGN.md, sections "S5 + S3b design freeze"
and "E1 status"), and item 4 of C-2. Both fall under D-1 option (a), which the owner approved.

**Unchanged:** validity, every root, every receipt, genesisHash, the wire format and every RPC answer.

**What changes:** how the state root is computed. It is updated from the keys a block changed, using a trie
of the parent's state whose nodes are shared between versions.

## Code

- **`prototypes/lp4-execution/src/state_trie.rs` (new):**
  - `StateTrie` (build, update, root, stats) and `Touched` (`between`, which lists every changed account
    and slot);
  - node encodings identical to `p1_trie::node`'s;
  - unit tests in `state_trie/tests.rs`.
- **`src/apply.rs`:**
  - `apply_block_with(parent, parent_trie, …)`; `apply_block` is the same with no parent trie (a whole-state
    build);
  - `BlockOutput.trie`;
  - `root_check()`: in lp4's tests and under the `root-check` feature, every root is compared with
    `world_root`. pocold's dev-dependency enables the feature; `POCOL_ROOT_CHECK=0` turns the check off for
    timing runs.
- **`prototypes/pocold/src/chain.rs`:**
  - `Entry.trie`, which only the head keeps after `settle`;
  - `trie_for`: a parent's own trie, or the head's moved through at most `TRIE_PATH_MAX` (64) S3a diffs;
    either is used only if its root equals the parent's committed stateRoot;
  - the rolling re-executions carry the trie;
  - counters `trie_derived`, `trie_built`, `trie_refused`; `incremental_root` (off: a test reference).
- **`src/statediff.rs`:** `StateDiff::touch`.
- **`src/resources.rs`:**
  - `block_writes`, which diff_max now shares;
  - `trie_bytes`, `trie_paths`, `TRIE_PATH_NODES`;
  - the trie group of A13.

## Results

| | |
|---|---|
| validation at the near-cap devnet state (`validation-near-cap-e1.log`, `accts-end`: 18,833 accounts, 46,636 slots) | empty child 491 → **262 ms**; 380 transfers 672 → **349 ms**; CREATE2-heavy 554 → **290 ms**. The root alone: whole state 314 ms; incremental 3.6 ms, plus 12.9 ms for `Touched::between` |
| trie size (same log) | 65,582 leaves and 23,956 branches in 10.6 MB: 162 B per key |
| devnet model (`a13-devnet-breakdown-e1.log`) | 3,322,680,105 → **3,486,909,657 B** |
| network model (`a13-network-profile-e1.log`) | 37.96 → **40.20 GiB**, gate 45.94 GiB; the trie group is 2.23 GiB |
| trie unit tests (`state-trie-tests.log`) | 5 of 5 pass |
| E1 tests (`e1-tests.log`) | 2 of 2 pass. Per phase as (derived, built, refused): main (0,0,0); side (3,9,0); back (3,12,0); deep (3,571,0) |
| suites (`suite-lp4.log`, `suite-pocold.log`, release, `28594cc`, reference check on) | lp4 178 passed / 0 failed; pocold 198 passed / 0 failed / 17 ignored |
| mutation controls (`mutants.log`, `mutants.json`) | 13 mutants on `28594cc`. The base passes; 11 are killed in pass 1. The two survivors were killed after a test was added for each: E1M9 (a code-hash change alone is not touched; unreachable under Cancun, now tested directly) and E1M11 (a rebuild carries the previous trie forward; never a wrong root, because the stale trie is refused, but no test rebuilt several blocks). **13 of 13 killed** |
| runner fix (`mutants-discarded-stale-build.log`) | The first pass copied the tree with its file times. After a mutant in lp4 (E1M1), cargo kept that mutated build for the next mutants, which only touched pocold. That pass is discarded. The runner now gives every copy new times. The S1, S3a and S4 passes mutated pocold only, so every mutant rebuilt the crate it changed, and they were not affected |

## Correction to the design estimate

The design put E1 at about 0.4–0.5 GiB at the network caps. The derived bound is 1.07 GiB per trie, and two
tries are counted. The reasons:
- it assumes the worst case of n − 1 branches;
- a world branch is as large as a world leaf;
- the head's trie and one derived or built trie are both counted.

The nodes move to `state.redb` in E4, as frozen.

## Not sped up by E1

A parent whose trie cannot be derived has its root computed from the whole state, which is the cost before
E1:
- a side block deeper than side_max, which lost its diff when its state was pruned;
- a parent more than 64 diffs from the head.
