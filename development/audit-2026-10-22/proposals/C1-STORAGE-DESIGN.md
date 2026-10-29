# C-1 storage program and D-6 checkpoints: design freeze (2026-10-24)

The owner approved D-1 option (a) on 2026-10-24: complete S1–S5 so that pocold runs the network profile
within 6 GiB, with equivalence proofs, data preservation and crash tests. D-6 is approved too: local state
checkpoints that keep the block history the spec requires, check their own integrity and recover from a
partial write.

This document fixes the design and the acceptance tests **before** the code, as was done for
D-REORG-REDB. None of it changes validity, roots, receipts, genesisHash, the wire format or an RPC result.
If a step would change one of them, it stops and goes back to the decision package.

## Invariants kept by every stage

- **I1 (source of truth).** `blocks.journal` stays append-only and is never compacted (storage.md:70:
  blocks are kept "until the end"). Every other on-disk structure is **derived**: it can be rebuilt from
  the journal by re-execution, and a lost or damaged copy is rebuilt, never trusted.
- **I2 (equivalence).** On every suite, replay and e2e run, the node answers every RPC block, transaction,
  receipt and log query exactly as it did before the stage, and its head, canonical chain and side sets
  after each journal record are those of live import (the D-REORG-REDB replay equivalence).
- **I3 (crash order).** Derived data for a block is written before the block's journal record. A crash
  in between leaves an orphan entry that nothing points to, never a journaled block without its data.
  Open checks that the derived stores cover the journal and rebuilds whatever is missing.
- **I4 (bounded memory).** Each stage replaces a `derive()` term by a proven bound: a cache with a fixed
  size, or a window of blocks. A `derive()` breakdown and the A13 pin are updated with it.

## The store

- The spec's store is redb (storage.md:63–73). pocold adds **redb 2.6.4**, which needs Rust ≥ 1.85
  (pocold uses 1.99.1). Its only runtime dependency, libc, is already in the lock; everything else it
  lists is dev-only.
- Its page cache is set explicitly (`Builder::set_cache_size`) and counted in A13. redb commits are
  crash-safe (copy-on-write with checksummed commit slots). I1 does not rely on that alone: a store that
  fails to open, or is inconsistent with the journal, is deleted and rebuilt.

## S1: canonical history off the heap (storage.md:70)

- `history.redb` holds two tables:
  - RECEIPTS: block hash → the encoded receipts of every validated block, side blocks included;
  - TXIDX: (tx hash, block hash) → index.
  It is written at import before the journal append (I3), and at replay for re-executed blocks.
- `Chain::retain` offloads canonical journaled blocks older than the resident window (head − K_hist).
  Their body and encoded header come from the journal; their receipts and transaction index come from
  `history.redb`. Blocks inside the window stay resident.
- Readers use one view:
  - RPC: blocks, transactions, receipts, logs, getHeaders, getBlockRlp;
  - P2P: GetFrom, GetRange and GetBlock.
  `find_tx` uses TXIDX and takes the canonical candidate.
- `restore_canonical` is no longer needed for receipts: the receipts of an adopted side block were stored
  when it was imported. It remains only to make blocks inside the resident window whole.
- **A13:** the history group (tx bytes, receipts and index, logs, evidence: 14.56 GiB at the network
  profile) is replaced by:
  - K_hist + 1 resident full blocks;
  - the redb cache;
  - the per-block metadata entries, which stay until S4.
- **Acceptance tests:**
  1. RPC equivalence: for every block, transaction, receipt and log, the answer of an offloaded block
     equals the answer before offload. This holds live and after reopen, on a chain with reorgs.
  2. P2P serving of offloaded canonical blocks: a node syncs from one whose history is offloaded.
  3. Crash and data preservation:
     - a crash between the receipts write and the journal append (simulated);
     - `history.redb` deleted, truncated or corrupted.
     In each case open rebuilds the store from the journal and the RPC answers are unchanged.
  4. `derive()` shows the new terms; the A13 pin is updated.
  5. Mutation controls: a receipt read from the wrong block, a missing TXIDX entry, and offloading
     inside the window.

### S1 status (2026-10-24): landed; evidence/c1-s1/

What was built as frozen above, and what was added after the tests showed a gap:
- `src/history.rs`: the two tables, written at import before the journal append (I3) and at replay for
  every re-executed block. Replay commits without fsync and makes them durable every 256 blocks and at
  the end; the journal records already exist, so a crash before that only means replay refills again.
- **Added: the store is never trusted past what it can prove.** The tests found two gaps, now closed:
  - a truncated `history.redb` made redb **panic** at open (an internal assertion), not return an error;
  - a flipped byte in a transaction-index key went **undetected** on read (redb checks page checksums
    only in `check_integrity`). The transaction was then reported "not found".
  The fixes:
  - open catches a panic inside redb and runs redb's full `check_integrity` (every page checksum). A store
    that fails either is set aside as `history.redb.aside` and rebuilt by replay. The check costs about
    1.0–1.4 s per GiB of store (measured), against the replay from genesis that open does anyway;
  - each receipts record carries keccak(block hash ++ record). A transaction-index entry is checked
    against the block's transaction hashes. Either failure while running marks the store damaged
    (`history.redb.damaged`), and the next open rebuilds it;
  - while the store is marked damaged, an RPC history read is **−32603**, never a block without its
    receipts, a shortened log list or "not found". An import's history write fails, and the node stops
    as on a failed journal append (exit 70). This cannot happen with an intact store, which was the only
    store before S1.
- `pocol_status` reports historyRefilled, historyRebuilt and historyDamaged.
- **Window:** `Limits::history_window`, default K_hist. A test reference sets `u64::MAX` (nothing
  offloaded).
- **Disk:** the redb file was at most 4.1 times the bytes it stores (synthetic records; redb grows by
  regions and rounds values to page orders). The A13 allocator term counts a factor of 8.

**A13, measured by `derive()`:**

| | before S1 | after S1 |
|---|---:|---:|
| devnet model total | 5,487,223,178 B | **4,323,240,949 B** |
| network history group | 19.19 GiB | **4.28 GiB**, of which 4.25 GiB is offloaded per-block metadata (S4) |
| network model total | 105.83 GiB | **90.93 GiB** (gate 103.92 GiB) |

A window of the whole chain reproduces the pre-S1 model exactly, apart from two differences:
- history.redb's cache and allocator are added;
- the logs term is now also bounded per block (LOG_BLOCK_MAX), which the USER phase enforces.

`a13_bound_breakdown` asserts this identity.

**Acceptance tests (all pass):**
- `canonical_history_off_the_heap_answers_as_it_did_resident` covers tests 1–3. A node that offloads
  answers as a resident reference after each of these:
  - live import, reopen;
  - deletion, junk overwrite and truncation of history.redb;
  - a crash between the history write and the journal append.
  It also checks:
  - a third node syncs the whole chain over P2P from the offloading node alone;
  - the transaction index is checked against the blocks themselves.
- `a_damaged_history_store_is_never_served_and_is_rebuilt` (test 3, extended):
  - 39 single-byte flips (receipts records, index keys, spread positions);
  - two kinds of wrong content that pass redb's checksums.
  No wrong answer is ever returned, and every damaged store is rebuilt.
- **Cross-version (I2):** the reference answers of the scenario on the tree before S1 (`e6025ce`) and on
  S1 are byte-identical (1,156,012 B, the same sha256).
- `a13_bound_breakdown` (test 4).
- Mutation controls (test 5): `evidence/c1-s1/mutants.log`.

## S3: retained and side states as reverse diffs; copy-on-write overlays (storage.md:70, D30)

- Only the head state is held in full.
- Each retained canonical state, and each side state, is the head state plus a **reverse diff**: the
  previous values of the accounts and slots that the blocks since then changed. The diffs cover at most
  R_keep blocks (2048, "diffs for 2048 blocks").
- A working copy (block apply, the USER phase, pool, production, RPC) is an **overlay**: writes go to a
  small map on top of a shared base. No full clone remains.
- A deep-fork rebuild starts from the head and applies reverse diffs. It no longer replays from genesis
  (journal review: the "double genesis rebuild").
- **A13:** the 45 full copies (73.49 GiB) become one copy, plus diffs bounded per block by the gas
  schedule (writes per block ≤ GAS_LIMIT / the cheapest state write), plus overlays bounded by the
  writes of the work in progress. Both bounds are derived and pinned with the stage.
- **Acceptance tests:**
  - every retained and side state read through diffs equals the full state it replaces, root by root,
    on every block of the suites;
  - the spec-depth 2048/2049 test and D133 at depth 400;
  - deep-reorg timing scales with depth, not height (measured);
  - overlay isolation: a dropped working copy leaves the base unchanged;
  - mutation controls.

## S5: live state on disk (storage.md:63–67)

- The head state lives in `state.redb`: accounts, slots and code, plus the reverse diffs of S3. A
  bounded cache sits in front of it.
- Every block commits its writes and its reverse diff in one redb transaction, after validation and
  before the journal append (I3). A crash leaves the store at a block that is either journaled or ahead
  of the journal by one block. Open detects the second case and rolls back one diff.
- The trie root stays recomputed from the stored state until C-2 item 4 adds the incremental root on
  this structure. That is a measured cost, not a memory term.
- **Acceptance tests:**
  - every block's stateRoot equals the in-memory implementation's (the suites);
  - crash tests: a kill between the state commit and the journal append, in both orders; a torn redb
    file; a missing file (rebuilt from the journal);
  - `a13_model_at_the_network_profile` is inverted and asserts `budget_required ≤ 6 GiB`;
  - RSS measured on a synthetic near-cap network state.

### S3 refinement (2026-10-24, before code): two parts

Reading the executor showed that S3 is two changes of very different reach:
- `lp4-execution` takes and returns whole `InMemoryDB` states (`apply_block` clones its parent, and the
  USER phase result is cloned back);
- its checks walk the whole state (`code_caps::validate_snapshot`, the genesis image check, the state
  root);
- `InMemoryDB` appears 124 times in `lp4-execution/src`.

**S3a: retained and side states as diffs (pocold only; the executor is unchanged).** This removes 34 of
the 45 copies: K_hist + 1 retained states and side_max side states, 55.5 GiB at the network profile.
- **Diffs.** Each stored block within the window, and each side block with a state, keeps a `StateDiff`:
  the before and after values of every account and slot that differ between its parent's state and its
  own. It is computed when the block is inserted, while both states are present (one comparison of the
  two maps).
- **Which states stay full.** The head stays full, and so does genesis (its state is the genesis
  allocation, small). Every other state is materialized on demand:
  - start from a clone of the head, which shares its code buffers (S2);
  - apply the before values from the head down to the lowest common ancestor;
  - apply the after values up the requested branch.
  At most one materialized state is alive at a time; it is a new A13 term, counted at one full copy.
- **Callers.** Every caller that read `&e.state` goes through `Chain::state_of`. It borrows the head's
  state and builds any other state. The callers are:
  - validating a child of a non-head block;
  - the RPC state queries at a tag;
  - production on a non-head parent;
  - rebuild and restore.
- **Bounds and A13.**
  - Per block, the changed slots are at most GAS_LIMIT / 2,900 (the cheapest first write of a distinct
    slot, warm SSTORE reset), plus SYS_KEYS_BLOCK_MAX.
  - The changed accounts are at most GAS_LIMIT / 21,000 × 3 (sender, recipient, coinbase), plus the
    accounts a CALL with value can touch per 9,000 gas, plus SYS_ACCT_BLOCK_MAX.
  - Storage cleared by an account's removal is bounded by the same writes: under Cancun only an account
    created in the same transaction is destroyed, so it had no earlier storage. This is to be confirmed
    against revm 19.7 and `commit_epoch` before the bound is pinned; if it does not hold, the diff keeps
    the cleared storage and the bound is stated without it.
  - The derived per-block diff bound × (K_hist + 1 + side_max) replaces the 34 copies in `derive()`.
- **CPU.** The comparison is O(state) per block, as the existing whole-state checks are. It is measured
  on the near-cap devnet state and reported with C-2. If it is too slow, the executor can return its
  written set instead; that would be an lp4 change.
- **Acceptance tests:**
  - every materialized state equals the full state it replaces, account by account and by stateRoot, at
    every height of the window, on chains with reorgs (the old resident copies kept as the reference in
    the test);
  - the suites unchanged: lp4 vectors, pocold consensus and caps, e2e, replay equals live, 2048/2049,
    D133 at 400;
  - RPC state answers at each retained tag are unchanged;
  - mutation controls: a diff missing a slot, a diff applied in the wrong direction, materialization
    from the wrong LCA.

**S3a status (2026-10-24): landed; evidence/c1-s3a/.**
- `src/statediff.rs`: a `StateDiff` holds the before and after value of every field that differs:
  accounts with their cached code, account state, each slot, contracts, block hashes and logs.
  `states_equal` compares every field, including the cached code that `AccountInfo`'s PartialEq ignores.
- `src/chain.rs`:
  - each validated block gets its diff;
  - `settle` (the end of `retain`) keeps full only the head, genesis and the side states within
    side_max;
  - `materialize` and `state_of` build any other state from the nearest full state through the diffs;
  - `validate` materializes a non-full parent itself, so its NeedsRebuild contract is unchanged.
- **Two findings made during the work, both fixed:**
  1. **An adopted branch must not break the path to the states below the fork.** When the adopted
     blocks had been pruned as side blocks, they had no diffs, and the retained canonical states below
     the fork became unreachable. They would then answer −32017 where the old code answered.
     - Fix: `restore_canonical` also re-executes canonical blocks above the cut that lack a diff, but
       only when a retained state lies below them.
     - The node calls `retain_unsettled`, then the restore, then `settle`, in live import and replay,
       so the fork point is still reachable during the restore.
  2. **No extra rebuild.** At first, after a fork below the cut, that re-execution rebuilt the adopted
     branch from genesis. Since no retained state lies below it, it is now skipped. The equivalence test
     shows the same rebuild count as the reference (20 and 20).
  3. **Kept side states must be full** (found from a surviving mutant).
     - The fault in the first version (`bef6c4f`): a side state kept within side_max could be held as a
       diff only, for example a block of the abandoned branch protected by a sync session. Once its
       neighbours were pruned it became unreachable. Had that branch been adopted again, its height
       would have answered −32017 where the code before S3a answered.
     - Fix: `settle` makes every kept side state full, materialized before the diffs that lead to it
       are dropped. Before S3a every kept side state was full too, so A13 already counts side_max full
       copies.
     - Test: the equivalence test now protects such a block, requires identical pruned flags for every
       block, and checks that the block answers again when it is canonical. It fails on `bef6c4f`'s
       behaviour (mutant SM11).
- **A13, measured by `derive()`:**

  | | before S3a | after S3a |
  |---|---:|---:|
  | devnet model | 4,323,240,949 B | **3,601,500,638 B** |
  | network snapshots group | 75.12 GiB | **27.76 GiB** (17 copies: head, genesis, side_max, 2 materialized, 10 transient, growth) |
  | network diff term | — | **0.46 GiB** (2·K_hist + side_max + 1 diffs of at most 7.0 MB each, derived from the gas schedule) |
  | network model total | 90.93 GiB | **41.41 GiB** (gate 47.33 GiB) |

  - A new condition, `s3a-removal`, records that a removed account keeps no earlier storage (Cancun).
  - Genesis is counted as a full copy. It is in fact the genesis allocation; a tighter term is open.
- **Validation time** at the near-cap devnet state (18,828 accounts, 46,636 slots): the diff alone takes
  12.5 ms per block. 380 transfers take 672 ms, against 655 ms for C-2's final code. An empty child takes
  491 ms, against 465 ms.
- **Acceptance tests (pass):**
  - `s3a_every_retained_state_equals_the_full_state_it_replaces`: 374 import steps, among them a 10-deep
    reorg, a re-reorg and a fork below the cut. 8,388 states are compared field by field and by
    stateRoot against a full-state reference. The canonical pruned flags are identical. At most 4 full
    states are held, against 35 in the reference.
  - `s3a_state_queries_answer_as_with_full_states`: every balance, nonce, code, storage and `eth_call`
    query at every height and tag equals the reference node's. This holds live, after two reorgs and
    after reopen, with the same rebuild and restore work.
  - The statediff unit tests: 2,000 random state pairs, both directions; composition over a chain; the
    cached code.
  - The S2 code-sharing test, now run over materialized states.
  - The full suites; the ignored deep-reorg tests; the mutation controls (`evidence/c1-s3a/`).

**S3b: transient working copies as overlays (an executor change, done together with S5).**
- This removes the 10 transient copies and the growth copy, 18.0 GiB at the network profile.
- It needs `lp4-execution` to run on a layered state: a `CacheDB` over a borrowed base, with whole-state
  iteration through a merged view, and the whole-state checks made incremental. The state root is
  C-2 item 4.
- S5 needs the same generalization, since its base is on disk. So S3b is designed and frozen with S5,
  as one executor change verified against the lp4 vectors, not as a separate pass.

Projected network model after S3a: 90.93 − 55.5 − 3.0 (the per-copy code containers) − 2.25 (side code
sets) ≈ **30.2 GiB, plus the diff and materialization terms (new)**. This is a projection until
`derive()` prints it.
**Correction after the code (S3a status below):** the measured total is 41.41 GiB. The projection
assumed two things that the code did not do:
- the side states as diffs: they stay full, so that a deep side branch does not rebuild;
- no extra copies: genesis and two materialized states are now counted.

### S5 + S3b design freeze (2026-10-24, before code): the executor on a layered state

**Why this is one stage.**
- After S3a and S4 the network model is 37.96 GiB. Most of what remains is full state copies:
  - the head, genesis, side_max and materialized states (9.80 GiB);
  - the 10 transient working copies and the growth copy (17.96 GiB).
- Removing them needs `lp4-execution` to run without whole states:
  - on a base it only reads (in memory, or `state.redb` for S5);
  - with an overlay for its writes;
  - returning the writes as a diff.
- The executor walks the whole state on every block in five places. Each must become incremental and be
  proven equal to the full computation:

| whole-state walk (lp4-execution) | replacement | proof |
|---|---|---|
| state root (`trie::state_root` over every account and slot) | an incremental trie: node hashes kept per account and storage trie, updated from the block's diff (C-2 item 4) | the full root recomputed and compared on every block of every suite (a test feature), and on every vector |
| `code_caps::validate_snapshot` and `inventory` | running totals of logical code per state, updated from the diff (C-2 item 2 already keeps them within a block) | equal to the full scan at every block in the suites |
| the caps counters (live user accounts and slots, system slots; `user_phase` lines 130–165) | counters updated from the diff | equal to the full scan at every block |
| `drop_unreferenced_code` (all accounts' code hashes) | a reference count per code hash, updated from the diff | the contracts map equal to the full rule at every block |
| `genesis.image.verify_candidate` (`check_caps`, the system slot count and the sum of every balance against the supply bound; the code and profile checks read five fixed accounts) | running totals (user accounts and slots, system slots, the balance sum) updated from the diff; the five fixed accounts read directly | equal results on every vector and block; a mutant that leaves a total stale must fail |

**The steps, each landed separately with its equivalence proof:**
1. **E1, incremental root.** This is also priority 2 (validation time): the root dominates the 465–491
   ms of an empty child at the near-cap state.
   - **How:** `p1_trie::node(rows, depth)` is a pure function of all the rows under the key prefix
     `rows[0].key[..depth]`. So a cache of each subtree's child reference (an inline encoding under 32
     bytes, or a hash), keyed by its prefix and invalidated along every changed key, reproduces the root
     exactly while recomputing only the changed paths.
   - **Cost:** about one entry of roughly 70–100 B per trie node. At the network caps (about 1M accounts
     and 4M slots) that is about 0.4–0.5 GiB. In memory it is a new A13 term. In S5 the node cache lives
     in `state.redb`, with only a bounded cache in memory.
2. **E2, running counters and code reference counts.**
3. **E3, the executor on a borrowed base with an overlay.** `apply_block(base, …)` returns its writes as a
   `StateDiff`. Every `parent.clone()` and `users.state().clone()` goes. The pool, production, RPC
   simulation and rebuilds use overlays. This removes the transient copies (S3b).
4. **E4, `state.redb`.** The head state and the window's reverse diffs live on disk behind a bounded cache.
   Each block commits its writes and its reverse diff before the journal append (I3). Open detects a store
   one block ahead and rolls it back. This is S5; D-6 checkpoints are committed versions of this store.
5. **E5, the two reductions that are still needed:**
   - a smaller inline entry;
   - the rebuild and the pool reading code from the store instead of their own sets.

**Unchanged:** validity, every root, every receipt, genesisHash, the wire format and every RPC answer.
If any step would change one of them, it stops and goes back to the decision package.

**Reference.** The current executor stays in the tree as a test-only reference until E4 lands, so that
every step is compared block by block against it, not only against vectors.

### E1 status (2026-10-24): landed; evidence/c1-e1/

**Unchanged:** validity, every root, every receipt, genesisHash, the wire format and every RPC answer.
Only how the state root is computed changes.

**What was built:**
- **`lp4-execution/src/state_trie.rs` (new):** `StateTrie`, the world trie and the storage tries of a state,
  as immutable nodes shared between versions.
  - Every node is encoded as `p1_trie::node` encodes the same rows. Extensions are not stored: a branch
    keeps the nibble it splits on, and its extension is encoded from the depth it hangs at.
  - `Touched::between(parent, child)` lists every account and slot whose committed value differs.
  - `update` copies and re-encodes only the paths of those keys.
  - `genesis_builder::world_root` stays as the reference.
- **`apply.rs`:** `apply_block_with(parent, parent_trie, …)`:
  - with the parent's trie, the root is updated from `Touched::between`; without it, the trie is built from
    the whole state;
  - `BlockOutput.trie` carries the child's trie;
  - in lp4's tests, and with the `root-check` feature that pocold's tests enable, every root is also
    computed by `world_root` and compared (a difference panics).
- **pocold `chain.rs`:**
  - `Entry.trie`; after `settle` only the head keeps one;
  - `trie_for(p)` gives p's own trie, or the head's moved through the S3a diffs between the two blocks (at
    most `TRIE_PATH_MAX` = 64 diffs, via `StateDiff::touch`);
  - either one is used **only if its root equals p's committed stateRoot**; otherwise the trie is built
    from the whole state, which is the cost before E1;
  - the rolling re-executions (rebuild, restore) carry the trie from block to block;
  - counters `trie_derived`, `trie_built`, `trie_refused`.

**A finding made during the work (fixed before the trie was used):** a batch that deletes a key not in the
trie, where that key leaves a branch's prefix early, could put the branch's change runs out of nibble order.
A valid change was then skipped and the root was wrong. The random-sequence unit test found it; such
deletions are now dropped before grouping.

**Validation time** at the near-cap devnet state (18,833 accounts, 46,636 slots;
`validation-near-cap-e1.log`, reference check off):

| | before E1 (S3a) | E1 |
|---|---:|---:|
| empty child | 491 ms | **262 ms** |
| 380 transfers | 672 ms | **349 ms** |
| CREATE2-heavy child | 554 ms | **290 ms** |
| the root alone: whole state / incremental | 314 ms | **3.6 ms**, plus 12.9 ms for `Touched::between` |

Most of what remains in an empty child is in the other whole-state walks (E2) and the state copies (E3).

**A13:**
- **The model:** a new trie group of three terms:
  - the head's trie;
  - one parent trie, derived or built, at most one at a time on the consensus thread (its distinct
    nodes are counted as a whole trie);
  - the changed paths of the block executed on it.
- **Node count:** a trie of n keys is counted at 2n nodes (each branch has at least two children) of
  168 B (world) or 88 B (storage), plus 2n child pointers.
- **Measured** on the near-cap devnet state: 162 B per key, branches included.
- **Totals:** devnet 3,322,680,105 → **3,486,909,657 B**. Network profile 37.96 → **40.20 GiB** (gate
  45.94 GiB); the trie group is 2.23 GiB.
- **Correction:** the design estimated about 0.4–0.5 GiB at the network caps. The bound is 1.07 GiB per
  trie, because:
  - it counts the worst case of n − 1 branches;
  - a world branch is as large as a world leaf;
  - two tries are counted (the head's and one being derived or built).
- **In S5 (E4)** the nodes move to `state.redb` behind a bounded cache, as frozen.

**What E1 does not speed up:** a parent whose trie cannot be derived has its root computed from the whole
state, as before E1:
- a side block more than side_max deep loses its diff when its state is pruned, and the path breaks;
- a parent more than 64 diffs from the head.

So a long competing branch is validated at the cost before E1, and a deep reorg too. Holding a second trie
for the tip of such a branch would make it incremental, at the price of one more trie in A13. That choice
is left to E4, where a trie costs disk rather than heap.

**Acceptance tests (pass):**
- **lp4 `state_trie` unit tests:**
  - 300 rounds of raw-key insert and delete batches (keys drawn to share long prefixes), against
    `p1_trie::root` after every batch;
  - every two-key shape (shared prefix 0–63);
  - 20 random state sequences of 30 blocks (accounts created, removed, marked not existing, storage
    cleared), against `world_root` and a fresh build, with the old version unchanged;
  - an incomplete touched set gives another root (the check that pocold relies on);
  - the node counts.
- **pocold `e1_incremental_roots_are_the_full_roots_on_every_path`:** 665 blocks: a main chain, a side
  branch, the branch back, and a fork 559 blocks deep from height 15 with rebuilds and restores. It runs
  against a reference with the incremental root off. The two have the same heads, canonical chains, pruned
  flags and state roots, and every root is also compared with `world_root`. The test checks each phase:
  - the main chain uses the head's own trie only;
  - the side branch derives its tries while it is within side_max;
  - the deep fork builds them;
  - nothing is ever refused.
- **pocold `e1_a_trie_that_is_not_the_parents_is_never_used`:** the head's trie is replaced by genesis's.
  - A child of the head validates with the right root: the wrong trie is refused and the root built.
  - A sibling validates too: the derivation through the wrong trie is refused.
  - With the right trie back, the sibling's parent trie is derived.
- **Every other test** (the suites, replay equals live, 2048/2049, D133, the S1, S3a and S4 equivalence
  tests) now runs with the reference comparison on every block.
- **Added after the mutation pass:**
  - `a_code_hash_change_alone_is_touched` (lp4);
  - `e1_a_rolling_rebuild_carries_the_trie`: a rebuild of 5 or more blocks builds one trie and refuses
    none.
- **Mutation controls:** 13 of 13 killed (evidence/c1-e1/mutants.log).

### E2 design (2026-10-24, before code): the per-block whole-state checks from a state summary

**What it replaces.** `apply_block` walks the whole state at least five times per block, apart from the
root:
1. `verify_candidate(parent)`;
2. the first SYSTEM call's full `validate_snapshot` and `inventory` (`code_total` starts as None);
3. `validate_snapshot` after the USER phase, plus the full check of the next SYSTEM call;
4. `verify_candidate(state)` at the end (`check_caps`, the system slots and the balance sum);
5. `drop_unreferenced_code`.

**The summary.** `state_summary::Summary` is a few numbers per state:
- the logical code total;
- the accounts whose code fails verification (missing bytes or a hash mismatch);
- the accounts over B_code_max;
- the user accounts and their nonzero slots;
- the system programs' nonzero slots;
- the balance sum, as a 512-bit number so that no intermediate sum can overflow.

`Summary::scan(db)` computes it from the whole state, exactly as the walks above count. `Summary::moved(from,
to, touched)` subtracts each touched account's contribution in `from` and adds its contribution in `to`.
It recounts slots per touched slot, and a whole account's storage only when the account appears or
disappears. The touched set is `Touched::between`, which is complete by construction.

**Where it is used.** `apply_block_with` takes the parent's summary next to its trie. With the summary:
- the parent's `verify_candidate` and the first SYSTEM call's full check read the summary;
  `code_total` starts from it;
- after the USER phase, the summary is moved through `Touched::between(parent, user state)`. That walk
  compares values without keccak-ing any code. The USER-phase check reads the new summary, and the
  following SYSTEM calls start from its code total;
- the final `verify_candidate` reads the summary moved to the final state. That move reuses E1's touched
  set;
- `drop_unreferenced_code` is a no-op when the contracts map did not grow and no touched account lost
  its code. During a block the map only grows (revm's commit inserts, only the drop removes), so equal
  sizes mean equal keys. Otherwise the drop runs as before. The capacity rule is unchanged.

Without a summary, every walk runs as before. `BlockOutput.summary` carries the child's summary. pocold
keeps it on every entry, since it is a few words, and computes genesis's with `scan`.

**Equivalence:**
- the Ok/Err outcome of every check is unchanged;
- when several accounts fail at once, the old scans reported whichever came first in HashMap order. The
  summary reports a code failure first, then B_code_max, then CODE_CAP. The block is invalid either way,
  and only the local rejection text differs;
- under `root_check()`, every summary is compared with `Summary::scan`, and every outcome and the
  contracts map with the old walks.

**Tests:**
- unit tests: random state sequences, with `moved` equal to `scan` at every step, including removed,
  not-existing and storage-cleared accounts, code created and destroyed, and system slots;
- a candidate that fails each check (an over-cap slot count, too many system slots, a balance over the
  supply bound, code over B_code_max) is refused with the summary as without it;
- the suites with the reference comparison on;
- mutation controls: a stale field, a skipped touched account, a drop skipped when the contracts map
  grew.

### E2 status (2026-10-24): landed; evidence/c1-e2/

As designed above, with these specifics:
- **lp4:**
  - `state_summary.rs`: `Summary`, `scan`, `moved`, `code_check` and the incremental `drop_unreferenced_code`;
  - `GenesisImage::verify_summary`;
  - `apply_block_with(parent, parent_trie, parent_summary, …)`; `BlockOutput.summary`.
- **pocold:** `Entry.summary` (boxed, dropped by `slim`); genesis's from `scan`. Rebuilds and restores
  give re-executed entries their summary.

**Validation time** at the near-cap devnet state (`validation-near-cap-e2.log`, reference checks off):

| | before E1 | E1 | **E2** |
|---|---:|---:|---:|
| empty child | 491 ms | 262 ms | **62 ms** |
| 380 transfers | 672 ms | 349 ms | **174 ms** |
| CREATE2-heavy child | 554 ms | 290 ms | **122 ms** |
| 149 one-create transactions | 556 ms | 321 ms | **154 ms** |

What remains in a block is mostly the state copies (E3), the two touched-set comparisons (about 14 ms
each; E3 replaces them with the executor's written set) and S3a's diff (13 ms).

**A13:** devnet 3,486,909,657 → **3,489,051,177 B** (each entry's summary). Network profile 40.20 GiB.

**Acceptance tests (pass):**
- **lp4 `state_summary` unit tests:**
  - 40 random state sequences of 25 steps, with `moved` equal to `scan` at every step. The steps include
    removed, not-existing and storage-cleared accounts, code that is missing, mismatched or over the
    limit, and system slots; a positive control requires every count to occur;
  - the code check against `validate_snapshot`;
  - 200 random drops against the full drop.
- **lp4 `e2_blocks_with_trie_and_summary_equal_the_reference`:** 30 blocks with transfers and contract
  creations, applied with trie and summary. They equal `apply_block` without them in roots, receipts, gas,
  state, contracts and summary.
- **lp4 `e2_failing_candidates_fail_alike`:** each of the following fails with the same error with the
  summary as without it:
  - a balance over the supply bound;
  - code over B_code_max;
  - missing code;
  - SLOT_CAP + 1 slots;
  - SYS_SLOT_MAX + 1 system slots;
  - ACCT_CAP + 1 accounts.
- **The suites, release, reference comparisons on:** lp4 184 passed / 0 failed; pocold 199 passed / 0
  failed / 17 ignored. The pocold E1 path test also checks every entry's summary.
- **Mutation controls: 11 of 12 killed** (evidence/c1-e2/mutants.log).
  - Tests were added after pass 1 for E2M4, E2M9 and E2M11:
    - a grown contracts map is dropped;
    - the states at exactly ACCT_CAP, SLOT_CAP and SYS_SLOT_MAX pass alike;
    - CODE_CAP is reached exactly, then a child is applied.
  - E2M8 (the post-USER code check result dropped) cannot be reached by execution: the create hook
    enforces both code limits inside the USER phase.

### E3 design (2026-10-24, before code): the executor on an overlay

**Where the transient copies are.**
- In `apply_block_with`:
  - `parent.clone()`;
  - the USER phase's `snapshot.clone()`;
  - `users.state().clone()`.

  C-2 item 3 already removed the per-transaction engine and PAY copies (`state_view::StateRef`).
- In pocold:
  - production's parent copies;
  - the pool's pending state and its execution copies;
  - the rolling rebuild's state;
  - materialized parents.

A13 still counts 10 transient copies and a growth copy (17.96 GiB at the network profile).

**The overlay.** revm's `CacheDB<B>` is an overlay already.
- Reads of an account or slot it does not hold fall through to `B` (`DatabaseRef`).
- Its `commit` records creates, self-destructs and storage clears with the same semantics as on a full
  state.
- `InMemoryDB` is `CacheDB<EmptyDB>`, so code written for `CacheDB<B>` behaves exactly as today when `B` is
  `EmptyDB`.

E3 makes the executor generic over `CacheDB<B>`:
1. **Reads.** Every lp4 read of the state goes through view helpers (`view::info`, `view::slot`,
   `view::exists`, `view::code`). The helpers read the overlay entry first: a held slot, or zero when the
   entry is created, cleared or not existing. Otherwise they read the base. Over `EmptyDB` the base answers
   "absent" and zero, which is what the direct map reads return today.
2. **Normalization over a base.** On a full state, normalization removes NotExisting accounts and zero
   slots. Over a base it must keep them as tombstones, because removing them would expose the base.
   `normalize` and the halt and commit paths take the base into account: over `EmptyDB` they behave as
   today.
3. **The write set.** After a block, the overlay holds exactly the accounts and slots the block wrote.
   - It is the touched set, complete by construction. It replaces both `Touched::between` walks.
   - The child's full state is `parent.clone()` with the overlay applied and normalized, built once at
     the end, where the old code cloned at the start.
   - The S3a diff, the trie and the summary are computed from the write set.
4. **The USER phase** runs on the same overlay, passed by value, so its two copies go.

**Steps, each verified before the next:**
- **E3.1 (lp4):**
  - view helpers; executor generic over `CacheDB<B>`;
  - `apply_block_with` runs on `CacheDB<&InMemoryDB>` over the parent and returns the write set with the
    merged child state.
  - Proof: under `root_check()` every block is also applied the old way (a clone), and the two child
    states are compared field by field (accounts, storage, code, contracts), with the roots, receipts
    and summaries.
- **E3.2 (pocold):** the S3a diff from the write set and the parent, instead of `StateDiff::between`. Under
  the reference check both diffs are compared.
- **E3.3 (pocold):** production, the pool's pending state, RPC simulation and rebuilds on overlays over
  a borrowed base:
  - the pool keeps only its write set, and builds a view over the head state for each operation;
  - A13's transient inventory is recounted from the code, and each remaining full copy is named.
- **E3.4 (pocold):** a child that becomes the head takes its parent's state by move, with the write set
  applied in place. The parent keeps its diff (S3a), so no copy is made when the head extends. A side
  child still gets a full copy (side states are full).

**Unchanged:** validity, every root, receipt and RPC answer. Any step that changes one stops and goes back
to the decision package.

**A13 after E3** (to be derived): the transient copies become write sets, each bounded by
`block_writes` (and, for the pool, by its own caps). The full copies left are the head, genesis, side_max,
the materialized states and the side child under construction. E4 then moves the head into `state.redb`.

### E3 status (2026-10-25): E3.1–E3.3 landed; evidence/c1-e3/

E3.4 (the head extended in place) is not done; it was later folded into E4 (see "E4 design").

**One deviation from the design, in the implementation only.** The executor is generic over two small
lp4 traits (`view::View` for reads, `view::WorkState` for writes). It is not generic over revm's
`CacheDB<B>`, for two reasons:
- `CacheDB<B>`'s `commit` and its normalization would have to be reproduced over a base anyway (point 2
  above);
- the pool and RPC need a layer they can keep between uses, without a borrow of the head.

`InMemoryDB` implements both traits with the code it replaces. Validity, roots, receipts and RPC answers
are unchanged, so no decision is affected.

**Code:**
- **lp4 `src/view.rs` (new):**
  - `View`, `WorkState`;
  - `Overlay` over a borrowed parent, with its `Layer`: `top`, `cleared`, the tombstones `deleted`,
    `retain` and the BLOCKHASH window;
  - `LayerView`, a layer read over its base;
  - `Overlay::merge`, the state the copy would have reached;
  - `Overlay::touched`, the complete write set.
- **lp4 executor on `&dyn View` / `WorkState`:**
  - `state_view::StateRef`, `code_caps`, `collision`, `pay`, `native`;
  - `user_phase::execute_user_phase_on` with `commit_tx_on`;
  - `apply::Job::run<S>` for phases 0–2;
  - `apply_block_with` on an overlay whenever the parent's summary is present; `BlockOutput.writes`;
  - `state_summary::moved` and `part` on views.
- **lp4 reference check.** Under `root_check()` (lp4's tests; pocold's tests through the `root-check`
  feature), every block applied on an overlay is applied again on a copy without trie or summary. The two
  are compared with `same_outputs`:
  - roots, receipts, bloom, facts, summary, meter;
  - every account (info, state, storage, code), the contracts, block hashes and logs;
  - a failing candidate must fail with the same error kind.
- **pocold:**
  - `Entry.state` is an `Arc<InMemoryDB>`, shared by the chain, the pool base and RPC reads;
  - `StateDiff::of_output` builds the S3a diff from the write set (`between_written`). Under the reference
    check it is applied both ways and the states are compared with the child and the parent;
  - the pool keeps a `Layer` over `pending_base`, the head's `Arc`: `pool_view`, `pool_written`;
  - template selection simulates on an overlay over the parent;
  - RPC `eth_call`/`estimateGas` run on an overlay, and "pending" is read in place (`TagState`);
  - `resources.rs`: `TRANSIENT_SNAPSHOTS` 10 → 2, `POOL_LAYER_SNAPSHOTS = 1`, `overlay_max`.

**A13, recounted from the code.** These remain full copies:
- a block's merged child (one);
- a rolling re-execution's state and its next block's merged state (two);
- the pool's base after a new head until `rebuild_pool`, which shares the previous head's `Arc` (counted
  in the transient two, since `restore_canonical` does not overlap a rolling re-execution);
- the pool's layer, counted as a whole state, because its gas is bounded only by POOL_MAX_TXS × GAS_LIMIT
  until D-7's value is approved.

The applied block's overlay is counted in the diffs group.

| | before E3 (E2) | **E3** |
|---|---:|---:|
| devnet model (`a13-devnet-breakdown-e3.log`) | 3,489,051,177 B | **3,186,963,844 B** |
| network profile (`a13-network-profile-e3.log`) | 40.20 GiB | **28.08 GiB** (required 32.09 GiB with the reserve) |
| snapshots group at the network profile | 27.76 GiB (16 whole states + growth) | **16.33 GiB** (9 whole states + growth, 1.63 GiB each) |
| code group (containers in every snapshot) | 7.07 GiB | 6.44 GiB |
| history group (`Entry` is smaller: its state is an `Arc`) | 0.84 GiB | 0.78 GiB |

The network figure is still far above 6 GiB. Most of what remains is whole states:
- head, genesis, the two side states, the two materialized states, the two transient copies, the pool
  layer, and growth;
- code sets, 6.44 GiB;
- the tries, 2.23 GiB.

E4 (`state.redb`: head state and trie nodes on disk, D-6 checkpoints) and E5 are what take these off the
heap.

**Validation time** at the near-cap devnet state (`validation-near-cap-e3.log`, reference checks off):
| | before E1 | E1 | E2 | **E3** |
|---|---:|---:|---:|---:|
| empty child | 491 ms | 262 ms | 62 ms | **8.5 ms** |
| 380 transfers | 672 ms | 349 ms | 174 ms | **55 ms** |
| CREATE2-heavy child | 554 ms | 290 ms | 122 ms | **17 ms** |
| 149 one-create transactions | 556 ms | 321 ms | 154 ms | **32 ms** |

The state is the `accts-end` point of the `all` series: 18,833 accounts and 46,636 slots. One run, on
this container.

What changed:
- the block no longer copies its parent at the start, and the USER phase no longer makes its two copies;
- the two `Touched::between` walks, about 14 ms each, are replaced by the overlay's write set;
- S3a's diff is built from the write set.

What remains is one copy of the parent, at the end (`merge`). E4 removes it (a child state is its root).
The rolling rebuild's and restore's diffs still use `StateDiff::between`; E4.1 moves them to the
write set too.

**Acceptance tests:**
- **lp4 `view` unit test `an_overlay_reads_and_merges_as_the_copy_it_replaces`:** 3000 random cases.
  Each case builds a normalized parent, then applies up to 12 steps to a copy and to an overlay alike. The
  steps are commits with created, self-destructed, removed and read-only accounts, `insert_info`,
  `info_mut` and the BLOCKHASH window. After every step every read (account, slot, nonzero slots, block
  hash) is the same. At the end the merged overlay equals the copy, and the write set names every changed
  account.
- **The reference comparison on every block:** every lp4 test and every pocold test that applies a block
  with a summary (validation, production, rebuilds, restores, the E1/E2/S3a/S4 path tests) also runs it
  on a copy and compares the outcomes field by field; the S3a diff from the write set is checked both
  ways against the parent and the child.
- **pocold `e3_pending_answers_are_the_pool_layer` (new):** after four pool transactions (transfers and
  a contract creation with storage), balance, nonce, code and storage at "pending" equal the merged
  pending state, and at "latest" the head, for written and unwritten accounts. eth_call from bob with
  more value than bob holds succeeds, and the head is unchanged.
- **The suites, release, reference comparisons on** (`suite-lp4.log`, `suite-pocold.log`): lp4 187 passed / 0 failed; pocold 200 passed / 0 failed / 17 ignored (prototypes tree `3b2e0baaa42f26b57ef5686b937c7f7c5241fc46`).
- **Mutation controls** (`mutants.log`): 19 mutants of the overlay, the write set, the diff from it, the pool's layer, RPC and
  template selection.
  - **Pass 1:** the base passes; 17 are killed. The new pocold test kills E3M16–E3M18; E3M17 and E3M18 by
    it alone.
  - **Pass 2:** after pass 1 the overlay unit test was strengthened. It now also requires every changed
    slot to be in the write set, or its account to be written whole. This kills E3M5: a cleared account
    not written whole. Execution cannot reach E3M5 as far as I can tell, because a CREATE onto an account
    with storage is a collision and a self-destruct changes existence; the test now holds the invariant
    directly.
  - **E3M7 is equivalent** on normalized commit sequences, the only ones the overlay models. A
    self-destruct that does not mark the account cleared is still marked cleared, and deleted, by the
    normalization that follows every commit. The insertion is kept as a defence.
  - **18 of 19 killed.**

### E4 design (2026-10-25, before code): the trie on disk is the state

**Why E3.4 is folded into E4.** E3.4 would have applied a block's write set to the head's in-memory
state in place. In E4 the head state is no longer in memory. Applying the write set to the store is the
same step, so E3.4 would be work thrown away a step later, and it does not change the A13 bound: a side
child is still merged into a full copy. Only E3.4's small part is done, with E4.1: the rebuild's and the
restore's diffs from the write set.

**Why "head on disk" is not enough.** After E3 the network model holds 10 whole states (16.33 GiB),
the code sets (6.44 GiB) and two tries (2.23 GiB). Moving only the head to disk leaves nine of them, and
the other states cannot become in-memory diff chains:
- a side state may be up to R_KEEP = 2048 blocks from the head;
- its path of diffs would then be up to 4096 diffs of up to 6.9 MiB each.

The spec's own inventory (storage.md, "trie" and "retention") answers this:
- one live trie, at most 447 B per entry;
- historical trie versions for K_eff ≤ 32 blocks, sharing their nodes, at most 1.6 GiB on disk;
- reverse diffs for 2048 blocks, 1.92 GiB;
- side data, 0.76 GiB.

All of this is on disk. E4 implements that model:
- **the trie on disk is the state store;**
- **a state is a root;**
- every retained state (the head, the K_hist window, the side states, genesis, a checkpoint) is read
  through its root, from nodes shared between versions.

What is in memory are bounded caches and the writes of the work in progress.

**The store.** `state.redb`, redb 2.6.4, already a dependency. Its tables:

| table | key → value | written |
|---|---|---|
| `node` | node hash (32 B) → node encoding (the `p1_trie::node` bytes) and a reference count | by a block's trie update, in the block's transaction |
| `code` | code hash → runtime bytes and a reference count | when an account's code first appears |
| `addr` | Keccak(address) → address | when an account first appears (preimage, for whole-state walks) |
| `slotkey` | (address, Keccak(slot)) → slot | when a slot is first written (preimage: the dirty-key meter and a removed account's keys need the slot) |
| `root` | block hash → state root, the version's pins, the BLOCKHASH window | per retained state |
| `diff` | block hash → the S3a reverse diff, encoded | per block within R_KEEP, canonical and side |
| `meta` | `head` → (block hash, height, root, last journal index); `checkpoint/*` (D-6) | per block, atomically with its nodes |

Inline nodes (encodings under 32 bytes) are kept inside their parent, as the trie encodes them, so every
stored node is hash-addressed. That makes a write idempotent and a version immutable.

**Reads (lp4 `view::TrieState`).** A `View` over a root reads as follows:
- `account(a)`: the world trie at Keccak(a). The leaf gives nonce, balance, code hash and the storage
  root. Its `AccountState` is Touched when the account exists. A NotExisting account is absent: the
  trie holds only existing accounts, and the full-state code removes NotExisting ones on normalization.
- `slot(a, k)`: a lookup in a's storage trie;
- `each_slot(a, ..)`: a walk of a's storage trie, with `slotkey` giving the slots. Execution needs it for
  emptiness (collision) and for a removed account's dirty keys;
- `each_account`: a walk of the world trie with `addr`. Only `Summary::scan` and the paths without a
  summary use it: genesis, and a fallback;
- `contract(h)`: the `code` table;
- `block_hash(n)`: the version's window.

Node reads go through a bounded cache (`NODE_CACHE` bytes, an A13 term), and code reads through a
bounded code cache. `View` returns owned values (`AccountInfo` clones are cheap: code is shared) so
that a store-backed view does not hand out references into a cache. This is the one interface change
to E3's traits, and `InMemoryDB` keeps implementing it unchanged in behaviour.

**Writes.** A block runs on an `Overlay` over its parent's `TrieState`, exactly as in E3. Its write set
updates the trie, using E1's update over nodes read from the store. The block's transaction then
records:
- the new nodes;
- the reference counts;
- new code and preimages;
- the reverse diff;
- `root[block]` and, when the block is the new head, `meta.head`.

This happens after validation and before the journal append (invariant I3). No whole state is merged
or copied: the child state is its root. A side child is a root too, so the merged side copy of E3 goes.

**Versions and reclamation.** A node lives while some pinned root reaches it. A root is pinned while
its block keeps a state under the existing retention rules:
- the head;
- the K_hist window;
- the side states within side_max;
- genesis;
- D-6 checkpoints.

When `settle` unpins a root, the nodes only it held are released by reference count, in the same
transaction. Within the window a state no longer needs materializing, because it is read from its
root. Deeper states are built as before: from the reverse diffs applied to a pinned root, with the
result committed as a new version. The disk bound is the spec's inventory.

**Crash safety.**
- redb commits are atomic, with checksummed commit slots.
- A block's transaction is committed before its journal record (I3). After a crash the store is
  therefore at the journal's head, or one block ahead.
- On open, `meta.head` is compared with the journal's last record. A head one block ahead is rolled back
  by unpinning that block's root. Its nodes are released, and the store is at the journal's head again.
- A store that fails to open, or whose head root does not equal the journal head's committed stateRoot
  after this, is deleted and rebuilt from the journal (I1), as history.redb is.

**D-6 checkpoints on it.**
- A checkpoint is a pinned root at a canonical height h_c below head − R_KEEP (every CHECKPOINT_EVERY =
  2048 blocks). It is stored with the serialized chain metadata (entries without states) and the index
  of the last journal record it covers, written in one transaction.
- **Integrity checks before a checkpoint is used:**
  - redb's checksums verify;
  - the root's recomputed hash from its stored nodes (a streaming walk) equals block h_c's committed
    stateRoot;
  - the metadata's hashes equal the journal records it claims.
- **Partial writes:**
  - a torn store fails redb's checks and is rebuilt;
  - a checkpoint whose checks fail is ignored, with a log line. Open falls back to the previous
    checkpoint, or to genesis.
- **History kept:** the journal stays complete (I1). A checkpoint only shortens the re-execution at
  open. It never deletes a block, receipt or log, as the owner required.

**A13 after E4 (to be derived from the code, then pinned):**
- **removed:** every whole state (16.33 GiB), the two in-memory tries (2.23 GiB), and the code containers;
- **added:**
  - NODE_CACHE and the code cache;
  - redb's page cache and allocator for `state.redb`;
  - one block's new nodes before commit (trie_paths);
  - the overlays already counted;
  - the reverse diff of the block in progress.

The code sets go in E5, when the rebuild and the pool read code from the store.

**Steps, each verified before the next:**
- **E4.1 (lp4).**
  - `View` with owned values.
  - `TrieState` over an in-memory node map: a state read from its root.
  - The rebuild's and the restore's diffs from the write set (pocold).
  - **Proof:** every block of the lp4 and pocold suites runs as before; under the reference check its
    parent is also read through a `TrieState` of the same root, and every read made by the block is
    compared with the full state.
- **E4.2 (lp4/pocold).**
  - `NodeStore` over `state.redb`: node, code and preimage tables; the bounded caches.
  - E1's update reads and writes through it.
  - **Proof:** roots equal to `world_root` on every block (the existing reference check), node reference
    counts equal to a recount from the pinned roots at every settle (a test-only check), and the disk
    stays within the spec inventory on a measured run.
- **E4.3 (pocold).**
  - `Entry.state` becomes a root handle.
  - Head, window, side and genesis states are read through roots.
  - `materialize` and `full_entry` give `TrieState`s.
  - Pinning and unpinning follow retention.
  - **Proof:** S3a's, S4's and E1's equivalence tests against `full_states`, extended to the store; every
    RPC state answer equal to the full-state node's.
- **E4.4 (pocold, D-6 and crash tests).**
  - Checkpoints.
  - Open with roll-back.
  - **Tests:**
    - a kill at each step of a block's commit and of a checkpoint's write;
    - a torn `state.redb`;
    - a flipped byte;
    - a checkpoint whose root does not match;
    - a missing store;
    - open from a checkpoint equal to open from genesis;
    - open time measured with and without one.
- **E4.5.** A13 recount and pin; the network model printed; `a13_model_at_the_network_profile`
  inverted if it is ≤ 6 GiB.

**Unchanged:** validity, every root, receipt and RPC answer, genesisHash and the wire format. The
journal stays the source of truth. If a step would change one of these, it stops and goes back to the
decision package.

**Costs that are not memory:**
- a node read on a cache miss is a disk read;
- validation time at the network caps then depends on the cache and the disk. It is measured with the
  near-cap state and reported, not assumed;
- deep rebuilds beyond the window cost what they cost now, plus the commits.

### E4.4 design refinement (2026-10-25, written with the code): checkpoints of the running chain

D-6 is owner-approved (local checkpoints). The frozen text above placed a checkpoint at one canonical
height below head − R_keep, holding one state. E4 makes every retained state a pinned version, so a
checkpoint can hold the chain as it is now. It is local derived data, never a validity input. Nothing
normative changes.

**What a checkpoint is (`checkpoint.rs`, `StateStore::write_checkpoint`).**
- A sealed snapshot of the chain's metadata after a journal record, with a pin of every state version
  an entry holds.
- The snapshot holds, for every entry, what its journal record and history.redb do not:
  - import sequence, total work, cumulative counters, drawn proposer;
  - the flags (pruned, full, slim, offloaded, final);
  - the state summary;
  - the stored root.
- It also holds the chain fields: head, next sequence, the slim and history cursors, the unslimmed list,
  the eviction FIFO and its total.
- Not kept: the invalid-block cache and the protect set. Replay recomputes the protect set from the
  journal.
- It is written in one durable `state.redb` transaction: the snapshot, its roots record, one pin per
  root, and the drop of checkpoints beyond `KEPT` = 2. redb commits atomically, so a crash leaves the
  checkpoints from before or after the write, never part of one. This replaces the temporary file and
  rename of the frozen text.
- **Deferred release** (2026-10-27, implementation; no rule changes). At the network's state caps,
  releasing a dropped checkpoint's nodes inside that transaction took 8–11 s on the consensus thread,
  plus 2.1–2.5 s to commit (evidence/memory-r14/scale-d30dc71).
  - The write now takes the dropped checkpoint's pins off and owes each root's node one reference, in the
    `release` table.
  - `StateStore::release_step` works the debt off: `RELEASE_SLICE` references per transaction, between the
    node's events, as the pool's revalidation slices run. A node left with no reference is deleted, and
    owes one reference to each of its children.
  - A node's stored count is the stored nodes naming it, plus its pins, plus what it owes.
  - The next checkpoint's write first finishes what is still owed, so at most one dropped checkpoint is
    owed at a time. A13's file term counts that set (`state_file_parts`).
  - A release step is fsynced: a crash loses the step, never part of it, and the work stays owed.
  - A step that fails on a storage error (an I/O error, say) loses only the step: the steps stop, the
    store stays whole, and the next checkpoint's write or open finishes what is owed. Only a row that
    contradicts the counts (a node owed but not stored, more owed than counted) marks the store damaged.
    Independent review, 2026-10-27, finding 1 (evidence/memory-r14/review-2026-10-27/D6-REVIEW.md).
  - With both pending, the event loop's background steps alternate between pool slices and release steps,
    so a pool backlog cannot hold the release back to the next write (finding 2).
  - A deleted node leaves the cache as it is deleted; no list of the deleted ids is held (finding 3).
  - The store's format is 3: a node's count includes what it owes. A format-2 store opens as format 3. Unsynced
    steps were measured too: each block's durable commit then wrote their pages, and imports during a release
    took up to 905 ms, against 155–209 ms with fsynced steps (two runs), and state.redb grew to 4.3–8.6 GB, since
    pages freed by unsynced transactions are not reused before a durable commit.
- When it is written: every `checkpoint_every` = 1024 journal records (`Limits`, local), by a running
  node and by replay. Replay first makes its history.redb writes durable.

**Binding.** A checkpoint names the journal prefix it describes:
- the record count;
- the end offset;
- a rolling hash, h_i = keccak(h_{i−1} ‖ keccak(payload_i)).

It is also bound to genesis and to the retention parameters (R_keep, side cap, K_hist, side_max, the
history window, slim depth). It is used only under the same ones.

**Open (`Node::open`).**
1. redb's full integrity check, as before.
2. Every checkpoint's seal, and its roots record against its snapshot. A failure here means the store is
   not sound: it is rebuilt.
3. What a checkpoint's write left owed is worked off (`release_all`, 2026-10-27). Then the pins become
   exactly the checkpoints' (`reset_pins`). This releases a running node's entry pins, and the version of
   a block whose journal record a crash cut off.
4. The newest checkpoint is tried, then the one before it. `checkpoint::restore` checks:
   - the binding against the journal;
   - every entry's journal record: the header hashes to the entry, its parent is restored, its total
     work follows from the parent;
   - every stored version's state root, computed from its stored root node, against the block's
     committed stateRoot;
   - every node of every stored version read: each node checked against its id, a missing node marks the
     store damaged, and each version's summary recomputed and compared with the sealed one. (2026-10-27,
     implementation, no rule change: a version whose parent's version was read is read through what differs
     from it, `NodeTrie::diff`, and its summary moved through those accounts and slots, as an import moves it;
     the oldest is read whole. Reading each whole took about 17 s a version at the network's state caps,
     evidence/memory-r14/scale-d30dc71.)
   A checkpoint that fails is released. A store found damaged is rebuilt.
5. Replay continues after the checkpoint's last record, with replay's protect set recomputed from the
   journal events. With no checkpoint usable, the store starts empty and replay begins at genesis, as
   in E4.3.

Checkpoints are not used when:
- the store or history.redb was set aside;
- history.redb was missing;
- the configured R_keep is below the one replay uses.

**History kept.** The journal and history.redb are never shortened. A checkpoint only shortens the
re-execution at open.

**Equivalence.**
- From a checkpoint written by replay, replay continues exactly as replay from genesis would: the same
  entries field by field.
- From a checkpoint written live, the snapshot is the live chain. Live retention can differ from
  replay's only through the protect set (a sync session's blocks against the journal's later children).
  The tests compare the two field by field and found no difference in the runs below.

**Tests (tests/consensus.rs, "C-1 E4.4"):**
- open from a checkpoint equals replay from genesis, for a replay-written checkpoint and a live-written
  one;
- leftovers of a crash are released;
- a kill at increasing delays (SIGKILL);
- an abort at each step of a block's commit and of a checkpoint's write (`--features crash-test`);
- damage: the store cut short, a changed byte, a missing store or history.redb, a broken seal, swapped
  roots, a changed summary, total work or binding, a node that is not its id, a missing node.

### E4 and E5 status (2026-10-25): E4.1–E4.5 and E5 parts 1–3 landed; store mode opt-in; evidence/c1-e4/

**Landed** (store mode is `Limits::state_store`, off by default):
- **E4.1:** node_trie, with the View on owned values (`7606a31`, `f8a58bb`).
- **E4.2:** state.redb, plus lp4's `apply_block_on` (`6c6e192`).
- **E4.3:** store mode in pocold: pins, retention, reads in place, the pool and RPC (`8f4b473`, `b91c490`).
- **E4.4 (D-6):** checkpoints (`1a5b792`); the checks its mutation pass showed were masked, and slimmed
  stored versions (`3c0fe15`); Windows test fixes (`45d540f`, `53a80e9`).
- **E4.5:** the store-mode A13 terms; the node cache counts resident bytes; streamed scans (`314de11`).
- **E5 part 1** (`90ba824`): entries without a state share one empty state; a trie leaf keeps its key's
  preimage, so preimages go with their versions (state.redb format 2; a store of another format is
  rebuilt).
- **E5 part 2** (`5722c23`): state.redb's allocator is counted the way redb 2.6.4 builds it.
- **E5 part 3** (`fc24969`):
  - state.redb's file factor is measured per phase and counted (STATE_FILE_FACTOR 12);
  - a closing node leaves its versions to the next open;
  - a damaged store serves nothing, not even its cache;
  - tests for the five retest survivors.

**Not done from E5's plan:** the rebuild and the pool still hold their own code sets. A13 counts 1.13 GiB
for the code an execution reads and 1.13 GiB for the pool's.

**Proofs (tests/consensus.rs, tests/state_store.rs).** A store-mode node answers as a whole-state node.
The comparison covers every block, receipt and state query at every height, the retained states and the
pins:
- live, after reorgs, and after reopening;
- at the R_keep boundary (24);
- with a fork deeper than K_hist (rebuild from a stored version);
- at the spec depth 2048/2049 (an ignored test, run in release on fc24969:
  evidence/c1-e4/accept/pocold-spec-depth-store.log).

Open from a checkpoint gives, field by field, the node that replay from genesis gives:
- from checkpoints written by replay and written live;
- with slimmed, offloaded and evicted entries;
- with replay's protect set restored, mid-flood;
- after kills at increasing delays;
- after an abort at each of seven steps of a block's commit and of a checkpoint's write: 21 cases in live
  import and one in replay;
- after seventeen kinds of damage, a history.redb that lost an old block's records, and a close that left
  its versions pinned.

**Mutation controls:**
- E4.1: 12/12 after N5's test.
- E4.2: 12/12 on b91c490.
- E4.3: 11/13 on b91c490 (C7, C10 survived). The deep-fork test killed both on 22942c7, which is 3c0fe15
  before the rebase (evidence/c1-e4/README.md).
- E4.4: 10/20 on 1a5b792. The retest on 22942c7 killed 8 of 13; K10a, K10b, K11b, K12 and K18 have
  tests since E5 part 3.
- Final pass on fc24969: **30 of 30 killed**, none classed equivalent. It covers E4.4's 21 mutants, E4.3's C7
  and C10, and E5's 7 (evidence/c1-e4/mutants-final.log).

**A13 (computed bound, not measured RAM; A13-DERIVATION §2c):**

| profile | whole-state mode | store mode |
|---|---:|---:|
| network | 27.92 GiB | 9.67 GiB |
| devnet as pocold boots it | 2.91 GiB | 3.36 GiB |

Store mode is not the default. The 6 GiB gate is unchanged, and the network profile does not boot in
either mode. DECISION-PACKAGE.md ("Measured after E4 and E5") lists what would lower the bound and the
owner decision it needs.

**Disk and time** (evidence/c1-e4/disk-impact-final.md, measured on 5ab73d1):
- import is 1.8–2.4× slower per block on a small store and about equal on a larger state;
- reorg is 1.5–2.7× faster;
- open from a checkpoint is 1.8–4.0× faster than whole-state replay;
- RSS after sync is within 15% of whole-state mode.

**Open:**
- E4.5's flip of the default: not now, because the model does not fit;
- E5's code sets;
- the state.redb file factor at the scale of the adversarial file (condition `store-file-factor`).

### After D-7 and the code-loading audit (2026-10-25; evidence/d7-bound/)

The section above is kept as recorded.

- **Store mode can be chosen for experiments.** `pocold run --state-store [--checkpoint-every N]`; the
  default stays whole-state. The e2e scripts pass `POCOLD_RUN_ARGS` to every node and check the mode each
  node reports.
  - On 9bd9bc0, in store mode: e2e_devnet 43/43, e2e_site 21/21, e2e_browser (Chromium) 80/80,
    e2e_integrated 19/19.
  - On 8584283 (the re-insertion relay), in store mode: e2e_devnet 43/43, e2e_site 21/21, e2e_browser 80/80,
    and e2e_integrated 20/20 in three runs of three. The browser reruns after the two test fixes (d939d1a,
    0833b6c): PASS 80/80 in three runs of three per mode.
  - The whole-state runs and the integrated reruns are in evidence/d7-bound/README.md.
  - **What these runs do not test in store mode:**
    - the network profile (A13 is computed there);
    - the adversarial file factor;
    - R14/R19 calibrated in store mode;
    - the witnesses, which run in whole-state mode.
- **E5's code sets.** The pool's code is now derived from D-7's gas bound:
  - store mode: its touched accounts' code, at most CODE_CAP, plus its creations, 0.62 GiB at the network
    profile;
  - whole-state mode: its creations.

  The code an execution reads stays one CODE_CAP set. revm 19.7 loads an account, and in store mode its
  code, before it charges for it, so gas does not bound those copies (A13-DERIVATION §2d). The rebuild's
  code is that execution set.
- **A13 now** (computed):

  | profile | whole-state mode | store mode |
  |---|---:|---:|
  | network | 25.24 GiB | 7.61 GiB |
  | devnet | 3.04 GiB | 3.48 GiB |

  The devnet bounds rose: the D-7 layer and the overlay count the slots a created account keeps, which
  "one whole state" did not bound.
- **state.redb under churn** (30 cycles of growth, update and delete, about 10 MB stored): the file stayed
  at 67.9 MB from cycle 5 on. File over stored was at most 7.95, allocated over stored at most 1.83. This is
  measured, not a bound; A13 counts 12.
- **R14/R19.** The admission timing applies blocks in memory. Store mode adds the commit and its fsync:
  - a GAS_LIMIT compute block imports in 64 ms (whole-state 29 ms);
  - state-heavy blocks took up to 4.8 s.

  Calibrating R14/R19 in the mode the node boots is open.
  - **Done after this:** a store-mode boot is timed through a scratch state store on its own disk, each
    calibration block committed durably, and the gate takes the larger time per phase (A13-DERIVATION §3):
    10–26 ms against 8–18 ms in memory on this host (15 runs). On 24cc167 the e2e suites pass in both modes
    (devnet 44, site 21, browser 80, integrated 20 twice).
  - **Still open:** the calibration is a compute workload. State-heavy blocks exceed t_exec in both modes on
    this host, so whether to calibrate on them is a decision.
- **Open:**
  - flipping the default;
  - the network target (D-1): 7.61 GiB against the 5.25 GiB cap;
  - the file factor at the adversarial scale;
  - the R14/R19 calibration workload (state-heavy blocks).

## S4: per-block metadata off the heap

- Entries older than R_keep keep only what fork choice and contexts need for the heights still reachable
  by a reorg. Everything else moves to `history.redb`, keyed by height and hash (storage.md "height 40 B").
- **Acceptance tests:** fork choice and the D-REORG-REDB tests unchanged; `derive()` term replaced.

### S4 status (2026-10-24): landed; evidence/c1-s4/

The frozen plan was to move old entries to `history.redb`. What was built instead: old entries stay in
memory but slimmed, and the journal plus a new `history.redb` table restore what was dropped, exactly.
The journal already holds the header and ParentResult bytes, so nothing else had to move.
- **What is slimmed:** canonical journaled entries deeper than `slim_depth` drop the header's heap fields,
  the ParentResult and the nonce ranges.
  - The default depth is 2·R_keep + E_win + 257. That covers any fork (R_keep), a head that fell back by
    up to R_keep, the evidence window, and the 256 ancestors of a child context.
  - Inside that depth no validation reads a slimmed entry.
  - Fork-choice fields, `drawn`, the scalars and the diff stay.
- **Restored, never trusted:**
  - the header and ParentResult come from the journal record, and the header must hash to the block;
  - the ranges come from history.redb (sealed, written before the journal append like the receipts).
  - Where restore happens:
    - a validation that needs a slimmed parent or evidence ancestor gets NeedsRebuild, and the node
      restores the entry (bounded retries);
    - a TrieRebuild re-executes slimmed blocks from restored copies;
    - RPC and P2P views restore too;
    - a restored entry is slimmed again once it is deep enough.
- **A13:**
  - network history group 4.28 → **0.83 GiB**; model 41.41 → **37.96 GiB** (gate 43.38 GiB);
  - devnet 3,601,500,638 → **3,322,680,105 B**.
  - A slimmed entry is still about 5 KB in the model, three times the inline `Entry` for map growth plus
    an empty state. A smaller inline entry is possible later.
- **Acceptance tests (pass):**
  - `s4_slimmed_entries_answer_as_full_ones_and_restore_exactly`: at depth 8,
    - exactly the canonical entries deeper than 8 are slimmed;
    - every block, transaction, receipt, log, header, RLP and P2P answer equals a never-slimming
      reference's;
    - every slimmed entry restores to the reference's fields;
    - a fork on a slimmed parent and a fork below the K_hist cut, which rebuilds through slimmed entries,
      both validate;
    - the same holds after reopen.
  - `s4_evidence_against_a_slimmed_ancestor_restores_it_first`: type-2 evidence whose ancestor is slimmed
    is accepted after the restore, with the same tombstone and state root as the reference.
  - Mutation controls: 9 of 9 killed (`evidence/c1-s4/mutants.log`). Suites, release, `13aeba7`: lp4 173
    passed / 0 failed; pocold 196 passed / 0 failed / 17 ignored. The ignored deep-reorg tests (2048/2049,
    D133 at 400, A11): 3 of 3 pass.

## D-6: local state checkpoints (owner approved)

- A checkpoint is a **committed `state.redb` version at a canonical height h_c**: every CHECKPOINT_EVERY =
  2048 blocks, below head − R_keep, where no reorg can reach. It is recorded together with a serialized
  chain-metadata snapshot (all entries, without states) and the index of the last journal record it
  covers.
- **Integrity:**
  - a checkpoint is valid only if its file checksum verifies;
  - its state root equals the stateRoot committed in block h_c's header;
  - its metadata snapshot matches the journal records it claims, compared by their hashes.
- **Partial write:** a checkpoint is written to a temporary file, fsynced, renamed and the directory
  fsynced. A torn or unverifiable checkpoint is ignored with a log line, and open falls back to the
  previous one, or to genesis.
- **History kept:** the journal stays complete (I1). A checkpoint only shortens the re-execution at
  open; it never deletes a block, receipt or log.
- **Acceptance tests:**
  - open from a checkpoint equals open from genesis: head, canonical chain, every RPC answer, the next
    imports;
  - a torn checkpoint, a checkpoint with a flipped byte, and one whose root does not match: each is
    ignored, and open still succeeds through the fallback;
  - a kill during the checkpoint write;
  - open time measured with and without a checkpoint.
- **Order:** D-6 is built on S5. A checkpoint is then a persisted store version plus metadata, not a new
  serialization of a full in-memory state.

## Order of work

1. S1 (done).
2. S3a (done).
3. S4 (done).
4. S5 with S3b, in steps: E1 (done), E2 (done), E3.1–E3.3 (done), E4.1–E4.5 (done; E3.4 folded in; D-6 is
   E4.4), and E5 (parts 1–3 done; the code sets are open).

Each stage lands with its own commit, evidence directory, suites and mutation controls. Its `derive()`
output replaces the projection in DECISION-PACKAGE.md §1.
