# Independent adversarial review of this PR's other fixes (2026-10-27)

- **Scope:** `git diff 13f1eac..HEAD -- prototypes`, MODEXP excluded (MODEXP-REVIEW.md covers it):
  - the store-mode `restore_canonical` fix (`2839f2a`);
  - the pool layer without code copies;
  - the A13 counts (`resources.rs`, `node_trie.rs`, the boxed entries);
  - the pacer test (`6a1e480`);
  - the two e2e_integrated fixes;
  - the new tests.
- **Reviewer:** a separate agent instance, given the code and the questions, not this work's conclusions. It
  ran tests and mutants in a worktree of its own, and changed nothing in the repository.

## Result

No correctness defect in node code. Four findings, all low severity.

| # | finding | done |
|---|---|---|
| 1 | **Checkpoint restore built an unboxed entry map** (`checkpoint.rs` `restore`, `HashMap<[u8;32],Entry>`) and converted it to the boxed map at the end. A13 counts each entry as a boxed slot plus one Entry (~1,284 B). The inline map's slots are 1,120 B, so at its growth peak open held ~3.8 KB per entry: ~2.5 KB above the model, ~320 MB at the network profile's H_END. The `open-replay-transient` condition did not list it | `restore` now builds the boxed map as it restores (`5abbaf3`). Before/after, VmHWM after an open from a checkpoint of the grown scale directory (950,000 accounts, 28,673 entries; ../scale-d30dc71): 538,980,352 B at `d30dc71`, 476,962,816 B at `bc38d41`, about 2.2 KB less per entry |
| 2 | **The pacer test no longer bounded the time to the first byte**, and its "not before GRACE" check was measured from the request, so it never bound. Every lateness check read the pacer's own figures. A 4 s sleep before the pacer's clock passes the test (15 other net tests fail on it) | `3705559`: the slow reader peeks to see its first byte. Seen from the client, the close is GRACE or more after it and within ENFORCE_SLACK of s_v (+0.3 s for polls). Its first byte comes about as soon as an honest reader's of the same batch, read just before. Faults (mutants-pacer-review.log): PC1, PC2, PT4, PT5 caught by the pacer test. PT5 closes early and reports a longer send time; it **survives the old test**, so the client-side window is what catches it. PT1 (a uniform delay) is caught by 15 other net tests, not by this one, since its honest reference is delayed alike |
| 3 | **e2e_integrated left the partition's a and b running when the total-work wait timed out:** they are started outside devnet's pids.json. The failure itself is reported clearly, but the next run meets busy ports | `3705559`: they are stopped in `main`'s finally. Seeded fault (cleanup.log): with an error thrown in place of the wait, the old test leaves a and b running (2 processes); the fixed one leaves none |
| 4 | **The stored-version handles were counted for 2·K_hist + side_max + 2 versions**, while the versions' bytes count 2·K_hist + side_max + 4: the block being imported and a rolling re-execution's next version were missing (two handles, a few tens of KB) | Both now read one list, `held_runs` (`5abbaf3`). Test `a13_counts_a_handle_for_every_held_version` pins it against the inventory written out. The scale profile's bound moves by the two handles: 5,255,359,885 → 5,255,514,985 B |

## Notes on test quality (no change needed, recorded)

- `store_mode_imports_on_the_head_execute_each_block_once` fails with the fix reverted, through its store-mode
  half. Its whole-state half is 0 before and after, and it checks `restore_reexecs` only.
- `a13_store_file_and_batch_terms_follow_the_node_trie_bounds` re-derives the formula with the same node_trie
  functions, so it pins how the terms are put together, not the bounds themselves.
  - The bounds are tested in lp4's node_trie tests, on random states of up to 600 accounts.
  - That a real block's `Overlay::touched` stays within `block_writes` rests on inspection (condition
    `store-inventory`).

## Checked and reported sound

- **`restore_canonical` in store mode:**
  - nothing is lost after a reorg, a replay, a checkpoint resume, a deep fork, or offloaded or slimmed blocks;
  - store mode has no in-memory entries;
  - whole-state mode is unchanged.
- **`drop_code_copies`:**
  - every written entry's address is in the drop;
  - every read path fills the code back in;
  - the pool's base changes only together with a reset of the layer;
  - self-destruct and re-creation, `code_total`, and "pending" `eth_getCode` are correct.
- **The A13 counts:**
  - the encoding constants equal `Node::encode`, and ROW matches the row;
  - batch nodes are a subset of the child's, and there are at most 64 branches per key;
  - `block_bytes` ≤ `version_bytes` term by term;
  - the held runs and the (KEPT+1)× checkpoint factor cover what is pinned;
  - the new arithmetic is all checked;
  - the 4-buckets-per-entry figure holds at every table size.
- **The pacer test's other bounds** match `rpc.rs` and its spec comments.
- **The e2e fixes:**
  - the stability window cannot skip the check;
  - the total-work wait corrects the test's assumption, not a node defect (D133 compares total work).
- **The new tests** fail with their fixes reverted (view, D-7, node_trie, ROW, P6 premise).
