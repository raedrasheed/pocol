# PR #27 automated review threads (2026-10-24)

Three review threads from 2026-10-22 were still open on PR #27. Each one was checked against the
current code (`8ce75c4`, the same code as `c959379`). Fix commit: `462fffc`.

| thread | claim | status on `8ce75c4` | action |
|---|---|---|---|
| `chain.rs` side-block pruning (P1, outdated) | `take(over)` removes only leaves, so the side-block count can stay above the cap after a reorg | already fixed: `Chain::retain` evicts the lowest-work unprotected leaf repeatedly, and a parent that becomes a leaf joins the heap (second review, task "loop eviction") | none |
| `node.rs` detached transactions (P2) | a reorg whose sides total more than 2,048 blocks restores only part of the detached transactions | **reproduced**: the walk stopped after 2·REORG_RESTORE_MAX = 2,048 steps. D-REORG-REDB adopts at depth R_keep = 2,048, so this case is reachable | fixed in `462fffc` |
| `admission.rs` calibration fee cap (P1) | after about 24 full blocks the base fee passes 16·baseFee0, and later samples become empty blocks | **reproduced** with D = 30: block h=25 used 0 gas | fixed in `462fffc` |

## Fix 1: A11 restore after a deep reorg (`pocold/src/node.rs`)

- **The walk.** It now reaches the common ancestor, as `lca_height` does. Only the abandoned blocks'
  journal offsets are held. That side has at most R_keep blocks, because a deeper fork is never adopted.
- **The pool rebuild.** `rebuild_pool` reads one abandoned body at a time from the journal (append-only,
  never compacted). Before the fix, every detached transaction was cloned up front.
- **Transactions the new branch includes.** The new head rejects them by their spent nonce. Before the
  fix they were skipped by a hash set built from the new branch, which needed journal reads of its
  offloaded bodies.
- **Order.** It is unchanged: oldest abandoned block first, block order, then the previous pool.
- **The test.** `a11_deep_reorg_restores_the_oldest_detached_transaction` is an ignored release test.
  It builds 1,101 of our blocks, with the transaction in the first one, and a heavier branch of 1,102
  blocks.
  - Before the fix it fails with an empty pool: `a11-deep-reorg-before-8ce75c4.log`.
  - After the fix it passes: `consensus-ignored-462fffc.log`. The same run also passes the spec-depth
    test at 2048/2049 and the D133 test at depth 400.
- **A13 effect.** The transient of restoring an abandoned branch drops from a copy of all its
  transactions to one body (≤ BODY_MAX) plus 8 B per abandoned block. That copy was not a term in the A13
  model. On the admitted profiles it was at most TXBYTES_CAP = 16 MiB.
- **CPU.** Each abandoned transaction is decoded and re-executed on the new head, as before. The number of
  abandoned blocks is at most R_keep (2,048), not 2·1,024 walk steps. CPU stays OPEN (A13 item 7).

## Fix 2: R14/R19 calibration workload (`lp4-execution/src/admission.rs`)

- **The cause.** Full-gas blocks compound the London base fee by 1/8 per block. After about 24 blocks it
  passed the fixed fee cap of 16·baseFee0, and later samples (closing, final) were empty blocks.
- **The fix.** The base fee is now held at baseFee0. `apply_block` takes the base fee as an input, and
  the measured work does not depend on it.
- **The guard.** `measure` now refuses a block that did not use GAS_LIMIT gas.
- **The test.** `calibration_keeps_the_full_gas_workload_through_a_long_closing_sequence` uses D = 30
  (33 samples).
  - On the previous code plus the guard it fails: `calibration h=25: the block used 0 gas`
    (`calibration-before-8ce75c4-plus-guard.log`).
  - On `462fffc` it passes.
- **Profiles affected.** The approved devnet profiles have short sequences (11 blocks on the default),
  so their samples always carried the workload.

## Suites on `462fffc` (release, Linux)

- lp4-execution: 169 passed, 0 failed (`lp4-462fffc.log`).
- pocold: 171 passed, 0 failed, 15 ignored (`pocold-462fffc.log`).
- Ignored consensus tests: 3 of 3 pass (`consensus-ignored-462fffc.log`).
- e2e_devnet: PASS 38/38 (`e2e-devnet-462fffc.log`).
