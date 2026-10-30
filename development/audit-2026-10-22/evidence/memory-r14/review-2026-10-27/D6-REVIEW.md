# Independent review of D-6's open by diff and deferred release (2026-10-27)

- **Under review:**
  - `b6d57e8`: at open, a stored version is read through `NodeTrie::diff` from its parent's version;
  - `93a40d0` and `22a82c2`: the deferred checkpoint release.
- **The tree reviewed:** `22a82c2`, in a separate worktree.
- **Who reviewed:** a separate agent instance, given the code and the questions, not this work's conclusions.
  It did not modify the repository.
- **Its test programs stay outside the repository** (adversarial and reproduction programs are not committed).
  Their results are quoted below.

## Summary

Nothing critical or high. Neither correctness claim could be broken:
- `NodeTrie::diff` read every node only the newer version holds, and the summary it moves equalled
  `Summary::scan_view` in every case;
- the count invariant (a node's stored count = the stored nodes naming it + its pins + what it owes) held over
  thousands of random interleavings.

There is one medium robustness regression, two low findings and several informational ones. All are fixed in
the commit that adds this file, except where the table says otherwise.

## Findings and what was done

| # | severity | finding | fix | test |
|---|---|---|---|---|
| 1 | medium | Any error in a release step marked the store damaged, an I/O error included. Every later state read then panics, and the next open throws the checkpoints away and replays from genesis. The same error in a block commit returns an error and leaves the store whole. Reproduced with RLIMIT_FSIZE 4096: the release step reported damage, while a block commit reopened with `rebuilt=false checkpoints=2`. | `work` tells a storage error (`Fault::Storage`) from a count that contradicts what is owed (`Fault::Broken`). Only `Broken` marks the store damaged, in `release_step`, `write_checkpoint` and `release_all`. A storage error loses the step: the steps stop, and the next checkpoint's write or open finishes what is owed. Open no longer marks the store damaged on a storage error from `release_all`. | `state_store::a_release_step_that_fails_on_io_leaves_the_store_whole` (Unix; a child process sets the limit) |
| 2 | low | `background_step` gave every step to the pool while it had a backlog. A backlog that does not empty between heads could leave the whole release to the next checkpoint's write, which is the stall the steps remove. Found by reading the code. | With both pending, pool slices and release steps alternate (`Node::release_turn`). An event still waits for at most one step of either kind. | `consensus::e4_release_steps_alternate_with_pool_slices` |
| 3 | low | The ids each transaction deletes were collected in a list (`gone`), 32 B per deleted node, and A13 does not count that list. The new call sites are the write's finishing loop and `release_all`; `drop_one` and `reset_pins` predate the change. | No list is kept. `work` and `release` take each deleted node out of the cache as they delete it. A transaction that then fails leaves a stored node uncached, which is only a cache miss. Existence checks read the table, not the cache. | the existing cache assertions (`a_released_version_is_not_served_from_the_cache`, and "neither stored nor cached" in the release test) |
| 4a | info | `write_checkpoint` set the `owed` flag inside the transaction, before its commit. | The flag is set after the commit. | (no failing case with KEPT = 2) |
| 4b | info | `stored_bytes` skipped the `release` table. | It counts each row as 32 + 4 bytes. | not tested (a measurement helper) |
| 4c | info | FORMAT was still 2, although what a count means had changed. An older build opening a store with owed rows would keep those nodes for good. | FORMAT 3. A format-2 store opens and is marked format 3: before the table it owed nothing, and the builds that wrote the table already counted what it owes. A format-2 build then sets the store aside rather than miss what is owed. | `state_store::a_format_2_store_opens_as_format_3_and_another_format_is_rebuilt` |
| 4d | info | `release_step`'s documentation and `stats.released_refs` said references, but the budget and the result count rows of the `release` table. | Documented as rows; the stat is `released_rows`, and jm_store_scale's key is `rowsReleased`. In the logs committed before, `refsReleased` counts rows. | |
| 4e | info | `checkpoint::restore`'s documentation was attached to `struct Read`. | Moved. | |
| 4f | info | The error text "could not be read whole" also covered reads by diff. | "could not be read". | |
| 4g | info | The `open-replay-transient` condition said "two 64-node paths". | It now names the recursion's path through the two world tries and the two storage tries of one account, plus the storage walk's stack. | |

## What the reviewer checked and found sound

**The diff restore**
- **By reading the code:**
  - the parted-prefix test uses only nibbles that every key below shares;
  - where the two nodes' prefixes differ in length, only the shorter node's child on the path can hold the
    longer node's keys;
  - a node is skipped only when its id equals the one at the same position in the old version;
  - each recursion step advances at least one nibble;
  - a leaf against a branch, and the empty trie, are handled.
- **A fuzz test of 11,360 pairs.** The inputs:
  - keys brute-forced to share 5 or more nibbles;
  - accounts removed and re-created in one step;
  - storage tries that appear, become a single leaf, are zeroed or are cleared;
  - versions consecutive, further apart in both directions, or identical;
  - update-built versions against build-built versions of the same state. Their ids differed in 1,023 of
    1,200, so the trie is not canonical.
- **What the fuzz test found:**
  - every account and slot `Touched::between` lists was in the diff. Its 457 extra listings were accounts
    equal on both sides, whose storage tries had other ids but the same content;
  - `moved` equalled `scan_view` every time;
  - every node only the newer version holds was read.
- **Which versions are read through a diff:** genesis, and any entry whose parent holds no version, are read
  whole. Side branches use their own parent. A slimmed parent's summary is computed, never sealed; but a child
  whose own summary is sealed is compared against it, so an error would be caught.

**The deferred release**
- **By reading the code:** every path that touches counts keeps the invariant: `commit`, `release`, `unpin`,
  `pin_stored`, `owe_one`, `work`, `drop_one` and `reset_pins`. A row of the `release` table always names a
  stored node.
- **A model test of 30 seeds × 400 random operations:**
  - 3,075 blocks, side branches included;
  - 1,049 re-commits with the same ids, 90 of them touching owed nodes;
  - 1,948 unpins, durable and not;
  - 381 checkpoint writes;
  - 632 release steps of 1–4 rows;
  - 199 `drop_checkpoint`;
  - 89 reopens with `release_all` and `reset_pins`, and 95 plain reopens.
- **Checked after every operation:**
  - the counts recount;
  - the pins equal the model's;
  - every live version reads as its state;
  - with nothing owed, the stored nodes are exactly what the pins reach;
  - after each write, everything owed lies within the checkpoint it dropped;
  - at open, the flag matches the table.
- **The A13 file term:** what is owed always lies within one dropped checkpoint's reach (one held set). There
  is at most one release row per node, and every node counted is at least ROW + BRANCH_ENC bytes.
- **Liveness:** each step makes progress and ends, and a failed step clears the flag.
- **Positive controls**, both reverted afterwards:
  - removing the storage walk for a new account made the diff fuzz fail;
  - removing the owing of children in `work` made the model fail (338 nodes stored, 322 reachable).
