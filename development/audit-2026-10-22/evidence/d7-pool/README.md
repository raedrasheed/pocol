# D-7: a bounded hold of the consensus thread for pool revalidation

The owner approved D-7 in principle on 2026-10-24: a local bound on the pool's work. The value was not
approved. This is the design, its tests and measurements, and the recommended value. The pool is local
policy. Validity, roots, receipts and genesisHash are unchanged.

## Design (`prototypes/pocold/src/node.rs`)

- **Backlog.** A new head no longer re-executes the pool at once. `rebuild_pool` resets the pending state
  to the head's and queues everything in pool order:
  1. the abandoned blocks (as journal offsets; each expands to its transactions when reached);
  2. the previous pool's admitted transactions;
  3. its own backlog.
  It then runs one slice.
- **Slices.** `pool_step` revalidates until `Limits::pool_slice` has passed, checked between
  transactions. The default is POOL_SLICE = 50 ms.
  - `run()` alternates: one waiting event, then one slice, while a backlog lasts.
  - So the consensus thread is held for at most **POOL_SLICE plus the transaction in progress**. One
    transaction is at most GAS_LIMIT, which the R14/R19 gate bounds by t_exec.
- **Submissions during a backlog:**
  - An RPC eth_sendRawTransaction drives the backlog until its own COMPUTE_DEADLINE (500 ms light).
  - If the backlog is not done by then, the answer is **−32021 {busy, retryAfterMs: 250}** and nothing is
    pooled. That is an existing error code, so the hold is the request's own deadline.
  - A P2P transaction joins the end of the backlog (within POOL_MAX_BYTES) and is relayed once admitted.
  - An in-process caller settles the backlog first.
- **Templates during a backlog.** Their candidates are the admitted transactions and then the backlog's,
  since the builder executes each candidate on the parent state anyway (R1-1).
  - Not yet expanded abandoned blocks are not candidates.
  - So a template made during a deep reorg's backlog can hold fewer transactions. This is local
    production policy.
- **Optional gas bound.** `Limits::pool_gas_max`: when the pooled gas limits would pass it, a submission
  is −32016. It is **off by default** until the owner approves a value.
- **Diagnostics.** `pocol_status` reports poolSlices, poolSliceMaxMs and poolBacklog.

## Tests (`tests/consensus.rs`)

- `sliced_pool_revalidation_equals_the_whole_rebuild`: across a reorg that detaches a block, a node with
  a one-item slice ends, once settled, with the same transactions in the same order and the same pending
  state root as a node that revalidates at once (the behaviour before D-7).
  - During its backlog, its templates see the same candidates.
  - A submission waits for the backlog.
  - Mutants: the detached block placed after the old pool, and templates that ignore the backlog. Each
    fails it (`mutants.log`).
- `a_submission_during_a_long_backlog_is_busy_at_its_deadline_and_not_pooled`: busy at about its 100 ms
  deadline plus one transaction, nothing pooled; after the backlog the same request is admitted.
- `the_pool_gas_bound_refuses_past_it_and_is_recounted_on_a_new_head`.
- The A11 tests settle the pool before reading it. They were unchanged otherwise, and pass.

## Measurements (`d7_pool_revalidation_hold_under_a_heavy_pool`, release, `--ignored`)

The pool holds BLAKE2F transactions with 7.8M rounds each (about GAS_LIMIT; uninterruptible). "Load" means
three busy loops on the 4-CPU host.

| run | pool | one transaction | **revalidating at once: the import holds the thread** | **sliced: import / longest slice** | slices | sliced total |
|---|---:|---:|---:|---:|---:|---:|
| `idle-12.log` | 12 | 175–188 ms | **1,918 ms** | **264 / 182 ms** | 11 | 1.83 s |
| `idle-48.log` | 48 | 195–246 ms | **7,965 ms** | **196 / 189 ms** | 47 | 7.53 s |
| `load-48.log` | 48 | 212–234 ms | **7,930 ms** | **165 / 214 ms** | 47 | 7.64 s |

The longest slice is always one transaction: the transaction in progress when POOL_SLICE ends. The
import, which also validates the block, stays within a slice plus one transaction. Without slicing, the
hold grows with the pool: about 68 min at 4,096 MODEXP transactions (review R3-6).

## The recommended value of the gas bound (for the owner's decision)

Slicing bounds the **hold**. The gas bound limits two other things:
- the **revalidation work** after each head, that is, how long the pool stays partly stale and new
  submissions may see busy;
- the CPU that revalidation takes from block import.

The rule recommended: one revalidation must finish within half a block interval on a host that meets
t_exec, so that it ends before the next head.

**POOL_GAS_MAX = max(1, ⌊T_blk / (2·t_exec)⌋) · GAS_LIMIT**

| profile | T_blk | t_exec | POOL_GAS_MAX |
|---|---:|---:|---|
| devnet | 4 s | 1 s | **2 · GAS_LIMIT** (≤ 2 s of worst-case work per head) |
| network | 60 s | 3 s (governance.md:7) | **10 · GAS_LIMIT** (≤ 30 s) |

- It is derived from approved profile values, so it follows a profile change without a new constant.
- It is below the 4·GAS_LIMIT first proposed for the devnet: that value would let the worst-case
  revalidation take the whole 4 s interval.
- The worst per-gas cost measured is MODEXP, about 1.0 s per GAS_LIMIT on the devnet (journal study
  §3.2a), which is exactly the t_exec bound.
- **Not applied:** `pool_gas_max` stays None until the owner approves this rule or another value.

## Suites on this tree

Release: lp4 173 passed / 0 failed (`suite-lp4.log`); pocold 187 passed / 0 failed / 16 ignored, the D-7 measurement among the ignored (`suite-pocold.log`).
