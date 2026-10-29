# R5 — adversarial review of `462fffc` ("PR #27 review fixes")

Reviewed in worktree `agent-a4df4c2ed1e5aa758`, detached at `ec148cf`. That is the tip of `claude/audit-repair-a01-a14`, and its `prototypes/` tree equals `462fffc`; the later commits change documents only.

- Comparison base: `462fffc^`.
- Builds: `CARGO_TARGET_DIR=…/scratchpad/review-target`, `--offline`, release.
- Every mutation was reverted with `git checkout -- <file>`. `git status --short` is empty at the end.
- The benchmark is a scratch crate under `…/scratchpad/r5/bench/` and is not in the repository.

## Summary

| ID | Severity | Topic | Status |
|---|---|---|---|
| R5-1 | MEDIUM | Dropping the "kept" hash set makes an ordinary reorg run a full pool execution, O(state), for every transaction both branches include. Measured: the adopting import goes from 32.0 s to 74.6 s (and 82.4 s on a rerun) | REPRODUCED |
| R5-2 | LOW | The new calibration test passes on the unfixed (pre-commit) `admission.rs`, so the test alone does not catch a regression | REPRODUCED |
| R5-3 | LOW | The deep-reorg test walks only 2,203 steps; a reinstated constant bound of 2,304 survives, while real reorgs can need more than 4,096 steps | REPRODUCED |
| R5-4 | NOTE | The reorg walk is correct and terminates for missing entries, genesis, offloaded or evicted blocks, `retain()` before the body reads, replay, and a missing `body_off` | CODE-READING + tests |
| R5-5 | NOTE | Without the "kept" set, pool contents and order stay the same (nonce rejection covers every shared transaction) | REPRODUCED (pool contents) + CODE-READING |
| R5-6 | NOTE | The rebuild does not stop once the pool is full: every remaining transaction still pays for decoding and ecrecover | CODE-READING + bench |
| R5-7 | NOTE | Test gaps: journal read errors, order inside a block, and abandoned blocks evicted by `retain()` before the read | REPRODUCED (N7 and N9 survive) |
| R5-8 | NOTE | Calibration: holding the base fee at baseFee0 is sound, the guard cannot falsely reject, and `need` is enough | CODE-READING + tests |
| R5-9 | NOTE | Memory per reorg goes down; what is held, and remaining pre-existing terms | CODE-READING |
| R5-10 | NOTE | The audit documents for this commit omit the CPU change of R5-1 | CODE-READING |

---

## R5-1 — MEDIUM — an ordinary reorg now re-executes every transaction the new branch also includes

**Where**
- `prototypes/pocold/src/node.rs:432-446` (`rebuild_pool`)
- `node.rs:399-407` (`submit_tx`)
- `prototypes/lp4-execution/src/native.rs:44-47` and `:91-92` (`build_engine` runs `snapshot.clone()`)
- `prototypes/lp4-execution/src/code_caps.rs:32-38` (`validate_snapshot` scans every account)

**Claim**
- Before: the old `detached_txs` skipped any detached transaction whose hash the new branch includes, at the cost of one HashSet lookup.
- Now: each such transaction goes through `submit_tx` and is only rejected inside `native::execute_for_profile`. That call runs `validate_snapshot`, a scan of every account in the pending state, and clones the whole `InMemoryDB` (`build_engine`) before revm sees the low nonce.
- Each rejected transaction therefore costs O(state), not O(1).
- In a normal reorg, both branches carry mostly the same transactions. That case therefore turned from almost free into the most expensive one:
  - A transaction rejected by nonce never fills the pool, so the `-32016 pool full` shortcut (`node.rs:405`) never triggers.
  - In the adversarial case (the new branch omits the transactions), the pool fills at 4,096 and the rest pay only for decoding.
- All of this runs on the consensus event loop, inside the import that adopts the branch.

**Evidence**

Scratch benchmark `…/scratchpad/r5/bench/tests/reorg_cost.rs`, a crate with a path dependency on this worktree:
- Our branch: 120 signed blocks of 60 transfers each, 7,200 transactions, each to a fresh address, so the head state ends at 7,210 accounts.
- Their branch carries the same bodies (honest case, `R5_HONEST=1`) or empty bodies (`R5_HONEST=0`), plus fallback blocks until it is heavier.
- The figure is the duration of the import that adopts their branch.

Command: `R5_HONEST={1,0} cargo test --release --offline --test reorg_cost -- --nocapture`

| node.rs | case | adopting import | pool after |
|---|---|--:|--:|
| `462fffc^` | honest (same txs on both sides) | **31.99 s** | 0 |
| `462fffc` | honest | **74.59 s** (rerun 82.41 s) | 0 |
| `462fffc^` | adversarial (theirs empty) | 6.32 s | 4096 |
| `462fffc` | adversarial | 7.52 s | 4096 |

Cost of a single call at the end of the run (head state has 7,210 accounts):
```
R5BENCH head_accounts=7210 rejected=500/500 submit_us_per_tx=6505 decode_us_per_tx=96
```
7,200 × 6.5 ms ≈ 47 s, which matches the difference (+42.6 s and +50.4 s).

**Scale**
- On the devnet profile, the detached transactions are bounded by `TX_CAP = 20,000` per branch, and the state by `ACCT_CAP = 20,000`. That gives several minutes added to an honest adoption stall, where it used to be about 0.
- On the network profile (`TX_CAP = 10^7`, `ACCT_CAP = 10^6`, governance.md:13), the bound is R_keep × transactions per block × O(state). It is effectively unbounded in wall-clock time.
- The adversarial worst case already existed before this commit, at up to 1,024 blocks. It is now up to R_keep = 2,048 blocks, so the worst case doubles and the common case now reaches it.

**Proposed fix**
- Restore a cheap filter without journal reads. After `restore_canonical`, every canonical block is resident (`Chain::canonical_resident`), and `tx_hashes` is set for each adopted block (`chain.rs:575`).
  - Build `HashSet<B256>` from `canonical[fork_h+1..=head]` in `rebuild_pool`. That is at most side_cap + protect_max + 1 blocks and at most TX_CAP hashes, about 640 KiB at devnet caps.
  - Skip a detached raw transaction when `keccak256(raw)` is in the set.
- Also, in `submit_tx`, compare `tx.nonce` with the sender's nonce in `pending_state` right after decoding, before `execute_for_profile`. This also cuts the cost of re-submitting the old pool on every new head, which is pre-existing (A13 item 7).

## R5-2 — LOW — the calibration test does not detect a revert of the fix

**Where:** `prototypes/lp4-execution/src/admission/tests.rs:46-50`

**Claim**
- The test only asserts `t.samples >= 30`.
- `samples` counts every block that was applied, including empty ones.
- Detection rests only on the new production guard (`admission.rs:360`).
- Reverting `admission.rs` to `462fffc^`, which has both the bug and no guard, passes the test.

**Evidence**
- Mutation C2 (`git show 462fffc^:…/admission.rs > …/admission.rs`):
  ```
  test admission::tests::calibration_keeps_the_full_gas_workload_through_a_long_closing_sequence ... ok
  ```
- Mutation C1 (the same revert plus the guard) fails with `calibration h=25: the block used 0 gas`, which confirms the commit message.

**Proposed fix**
- Report the workload in `Timing`, for example `min_gas_used` or `full_gas_samples`.
- Make the test assert it, e.g. `t.full_gas_samples == t.samples`, so that the test checks the workload itself rather than relying on the guard in `measure`.

## R5-3 — LOW — the deep-reorg test covers the old bound with only about 7% margin

**Where:** `prototypes/pocold/tests/consensus.rs:445-466`

**Claim**
- The test builds 1,101 + 1,102 blocks, a walk of 2,203 steps.
- Any constant step bound of at least 2,204 passes it.
- The reachable maximum is much larger: the abandoned side can have R_keep = 2,048 blocks (`chain.rs:461`), and the adopted side must be heavier, so it is usually at least 2,049 blocks. That is more than 4,096 steps.
- The test therefore does not pin "the walk always reaches the common ancestor" at the depth the commit message calls reachable.

**Evidence**

| Mutation | Change | Result |
|---|---|---|
| N1 | `steps<2048` | CAUGHT: `left: []` at `consensus.rs:464` |
| N1b | `steps<2304` | `a11_deep_reorg… ... ok`, SURVIVED |

**Proposed fix**
- Run the ignored test at full depth: `ours = R_KEEP` (2,048) blocks, with theirs heavier.
- Expect roughly twice the run time, about 80–110 s in release; the current test takes 39–60 s here.
- Alternatively, have `detached_blocks` report the depth it walked (a counter in `Stats`), and assert in a small lowered-limits test (`rnode`) that it equals `old.h − fork_h`. That catches any constant bound regardless of size.

## R5-4 — NOTE — the walk is correct and terminates

**Where:** `node.rs:421-431`

1. **Termination.**
   - `validate` requires `h = p.h + 1`, so each step lowers the height of `a` or of `b` by exactly 1.
   - The loop ends when `a == b`, at the latest at genesis, which is unique.
   - It runs at most `(old.h − F) + (new.h − F)` steps, all metadata lookups.
   - Abandoned side, at most R_keep: a non-canonical block at or below `head − r_keep` is removed (`chain.rs:461`), and so is a new block at or below `head − r_keep` (`node.rs:315`). The fork child must therefore satisfy `F + 1 + r_keep > H`.
   - Adopted side, at most side_cap + protect_max + 1.
2. **Missing entries.**
   - `retain()` removes descendants whose parent is gone (`chain.rs:463-465`), so every stored block has a stored parent.
   - The walk runs before `retain()`, and `insert` removes nothing.
   - The `else{break}` arm is unreachable. It is silent, unlike the `body_off` arm; adding a log line there would be consistent.
3. **Genesis.** It is always an ancestor of both heads, so it is never pushed; its `body_off = None` is never logged.
4. **Offloaded or evicted blocks.**
   - Abandoned blocks were canonical, so they are resident.
   - The new code reads every body from the journal anyway, so residence does not matter.
   - Adopted-side blocks may be offloaded; only `height` and `parent` are read, and offloading keeps both.
5. **`retain()` between offset collection and body reads.**
   - `retain()` may offload, or (side cap) evict, the abandoned branch.
   - The reads use journal frame offsets, and the live journal is append-only (`store.rs:184-191`; `read_at` at `store.rs:195-207` re-checks the length check, digest and marker).
   - `restore_canonical` only reads.
   - Nothing appends between collection and read: `on_new_head` runs before orphan children are imported (`node.rs:351-370`).
6. **Replay on open.**
   - Replay never calls `detached_blocks`; the pool starts empty, which is pre-existing (the pool is not durable).
   - Offsets are recomputed (`node.rs:197-218`) consistently with v2 framing, including after torn-tail recovery (prefix rewrite) and v1 migration (re-framed `v2` bytes).
   - A reimport updates `body_off` to the later record, whose body is the same because the roots commit to it.
7. **Abandoned block without `body_off`.** Only genesis, or a bare `Chain` with no journal, which the Node never uses. The block is logged and skipped, and the walk continues.

**Tests:** all three `a11_` tests pass on `462fffc`:
```
cargo test --release --offline --test consensus -- --include-ignored a11_
3 passed; 0 failed … finished in 38.84s
```

## R5-5 — NOTE — dropping the "kept" set gives the same pool contents and order

**Claim**
- Every transaction included on the new branch spends its sender's nonce in the new head's state. This includes exceptional halts, which still apply the nonce increment and fee (`user_phase.rs:215-252`).
- Nonces never decrease: Cancun (`native.rs:93`), EIP-3607, and EIP-161 cleanup never removes an account with nonce ≥ 1.
- `submit_tx` therefore rejects every such transaction, without side effects: nothing is pushed or committed on error.
- The duplicate and pool-full checks run before execution, so pool capacity is not affected.
- Order is unchanged: oldest block first, block order, then the old pool.
- I found no transaction that is on both branches and accepted on the new head.

**Evidence**
- In the benchmark, `pool_after` is the same on old and new code (0 honest, 4096 adversarial).
- The existing "included" case (`consensus.rs:436-441`) passes on the new code.
- **Difference:** cost only (R5-1).

## R5-6 — NOTE — the rebuild keeps decoding once the pool is full

**Where:** `node.rs:440-445`, `node.rs:402-405`

**Claim**
- `submit_tx` decodes, which includes ecrecover at about 96 µs (measured), before it checks the pool-full condition.
- Once `pool.txs.len() == POOL_MAX_TXS`, nothing more can enter during this rebuild.
- Yet every remaining detached body is still read and every transaction decoded, and the same happens for the whole old pool.

**Proposed fix:** `if self.pool.txs.len() >= POOL_MAX_TXS || self.pool.bytes >= POOL_MAX_BYTES { break }` in both loops.

## R5-7 — NOTE — test gaps

| Mutation | Change | Result |
|---|---|---|
| N7 | `continue` → `break` on a journal read error (`node.rs:442`) | all three `a11_` pass, SURVIVED: no fault-injection test |
| N9 | `txs.into_iter().rev()` (order inside a block) | all three pass, SURVIVED: every A11 block holds one tx (gap also in the old code) |

- No test covers an abandoned branch that `retain()` evicts by the side cap before the body read. That is exactly the case for which the offsets are taken first.
- A test with lowered limits could cover it: `rnode(tag, r_keep, side_cap)` with the abandoned branch longer than `side_cap`.

**Proposed fix:** add the eviction test, a two-transaction block in the order test, and a journal-corruption case in which the remaining blocks are still restored.

## R5-8 — NOTE — calibration (R14/R19)

**Where:** `admission.rs:336-365`

- **Holding the base fee at baseFee0 is sound.**
  - `apply_block` takes `base_fee` only as a value for the native block context (`apply.rs:264`). It does not check the value against the London rule; that is item 11, in the node.
  - The base fee changes only price arithmetic: the sender's debit and the burn.
  - Tips stay 0 (priority 0, as before), so `closeBlock(tips)` does the same two `put`s regardless (`PoColSystemV1.sol:138-145`).
  - The measured work does not depend on the fee.
  - Remaining pre-existing gap: one SPIN transaction is not a worst-case real block (many transactions, state writes). The admission report already flags this (`timing-not-a-bound`, `admission.rs:308`).
- **The guard `gas_used < GAS_LIMIT` cannot falsely reject.**
  - `gas_used` is the USER phase total from the commitments (`apply.rs:323`); system gas is separate (`system_gas`).
  - SPIN (`JUMPDEST; PUSH1 0; JUMP`) always ends out of gas, so the full limit is used with no refund. An exceptional halt also consumes all gas.
  - Final blocks execute user transactions: the test reaches `seen[2]` with the guard in place.
  - A profile where the transaction is invalid (GAS_LIMIT < 21,000, or not fundable) fails in `apply_block` both before and after this commit.
- **`need` is sufficient.**
  - `need = max(base,1)·GAS_LIMIT·blocks`. The loop sends at most `blocks` transactions.
  - Each transaction is charged `GAS_LIMIT·base ≤ GAS_LIMIT·max_fee`, so the upfront check `balance ≥ GAS_LIMIT·max_fee` always holds.
  - `need` is 16× smaller than before, so donor funding is easier.
  - With `baseFee0 = 0`, `max_fee = 1` is accepted and the transactions cost nothing.
- **Tests:** `cargo test --release --offline --lib -- admission::` gives 13 passed.

## R5-9 — NOTE — memory and CPU accounting per reorg

**New path, held at once**
- `detached`: `Vec<u64>` of at most R_keep entries (16 KiB).
- One record at a time:
  - the `read_at` payload, at most `MAX_RECORD` = 4 MiB (in practice header + BODY_MAX of 192 KiB + result);
  - the field copies made by `decode_record`;
  - the `Body::decode` transaction vectors;
  - in total about 3× the record (under 1 MiB at devnet caps).
- `old` pool and new pool at the same time (each up to 32 MiB plus index; network.md:157 allows 64 MiB).
- One O(state) snapshot clone inside `execute_for_profile`.

**Old path:** additionally held a copy of every detached transaction (at most TXBYTES_CAP, 16 MiB devnet) and the kept HashSet. The new path is lower.

**CPU**
- Up to R_keep journal reads, each with a file open and SHA-256 of the payload (under 1 s for 2048 × 192 KiB).
- Every detached transaction: one decode with ecrecover (about 96 µs) and, until the pool fills, one O(state) execution (6.5 ms at 7.2k accounts).
- Old path: at most 1,024 abandoned blocks; transactions included on the new branch cost a hash lookup; plus journal reads of offloaded new-branch bodies, which are now gone.

**Other**
- The second walk `lca_height` (`node.rs:346`) repeats `detached_blocks`' walk. It is trivial, but the walk could return `fork_h`.
- Even a 1-block reorg now reads its body from disk instead of cloning it from memory. This is negligible.

## R5-10 — NOTE — the audit documents omit the honest-case cost

**Where:** `development/audit-2026-10-22/JOURNAL-GROWTH-AND-REPLAY.md:974-979` and `A13-DERIVATION.md:389`

**Claim**
- The documents extend the stall estimate to 2,048 abandoned blocks, stating it as the re-execution of abandoned transactions.
- They do not state two things:
  - transactions on both branches, which the old code skipped, are now re-executed;
  - each rejected submit is O(state).

**Proposed fix:** update both documents with the R5-1 figures, or restore the filter.

---

## Mutation table

Commands:
- Calibration: `cd prototypes/lp4-execution && cargo test --release --offline --lib -- calibration_keeps_the_full_gas`
- A11: `cd prototypes/pocold && cargo test --release --offline --test consensus -- --include-ignored a11_` (the deep test alone for N1b)

Each mutation was restored with `git checkout -- <file>`.

| ID | File | Mutation | Result | Output |
|---|---|---|---|---|
| C1 | admission.rs | pre-commit code plus the new guard (revert the base-fee hold only) | **CAUGHT** | `calibration h=25: the block used 0 gas, not the GAS_LIMIT workload` |
| C2 | admission.rs | full revert to `462fffc^` (no hold, no guard) | **SURVIVED** | `... ok` (R5-2) |
| C3 | admission.rs | remove the guard only | SURVIVED | equivalent behaviour: the hold keeps blocks full |
| C4 | admission.rs | reinstate London `next_base_fee` with the 16× cap; transaction always present | **CAUGHT** | `calibration h=25: User("Arithmetic")` |
| N1 | node.rs | step bound `steps<2048` | **CAUGHT** (deep) | `left: []` at consensus.rs:464 |
| N1b | node.rs | step bound `steps<2304` | **SURVIVED** (deep) | `... ok` (R5-3) |
| N2 | node.rs | skip the oldest detached block (`gone.pop()`) | **CAUGHT** (all 3) | consensus.rs:411, 426, 464 |
| N3 | node.rs | drop `gone.reverse()` (newest first) | **CAUGHT** by `a11_excludes…keeps_order`; the deep test survives | consensus.rs:426 |
| N4 | node.rs | full revert to `462fffc^` | **CAUGHT** (deep); the other two pass | `left: []` at consensus.rs:464 |
| N7 | node.rs | `continue` → `break` on a journal read error | SURVIVED | no fault injection (R5-7) |
| N9 | node.rs | reverse the transaction order inside a block | SURVIVED | single-transaction blocks only (R5-7; also in the old code) |

Equivalent mutant, not run: `ea.height>=eb.height` → `>` (the walk still meets at the common ancestor).
