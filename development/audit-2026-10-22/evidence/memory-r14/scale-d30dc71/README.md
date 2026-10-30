# A store-mode node at the network's state caps (R3-4, 2026-10-27)

What A13 computes for this profile, compared with what a node measured on this host. A measurement on one
host and one workload is not a bound, and a computed bound is not acceptance.

## Setup

- **Test:** `jm_store_scale` (tests/journal_measure.rs, ignored), in release, with `POCOL_ROOT_CHECK=0` (the
  test-only whole-root check off). One process holds the node alone, so RSS and VmHWM are the node's.
- **Profile:** `scale_cfg()`. It is the devnet profile with the network's state caps and the caps around
  them:
  - ACCT_CAP 10^6, SLOT_CAP 4·10^6, CODE_CAP 1 GiB, TXBYTES_CAP 8 GiB, TX_CAP 10^7;
  - H_END 129,600 and its closing window;
  - every other parameter the devnet's: GAS_LIMIT 8,000,000; the single-host locals K_hist 32 and
    side_max 2.
- **Mode:** store mode (state.redb). Checkpoints every 1,024 journal records, the default.
- **Workload:** full-gas blocks that create fresh slots or fresh accounts, each followed by an empty block,
  until 950,000 user accounts and 3,800,000 user slots (95% of the caps). It is synthetic: one transaction
  per block, small code, no pool, no RPC load, no peers, one fork.

## Runs

| file | code | phase |
|---|---|---|
| `grow.log` | `d30dc71` | grow to 950,000 accounts / 3,800,300 slots (29,518 blocks, 6,764 s) |
| `reopen.log` | `d30dc71` | open from the newest checkpoint; a fork from 8 blocks below the head, adopted (the reorg) |
| `verify.log` | `d30dc71` | open again; the head is the reorg's tip |
| `open-instrumented.log` | `b47fb85` + review fixes 1 and 4 + timing prints (scratch build) | the verify phase with each step of open timed. A pocold suite ran beside it, so its times are under load |
| `verify-bc38d41.log` | `bc38d41` (clean tree) | open after `b6d57e8` (stored versions read through their diffs) and `5abbaf3` (fix 1) |
| `ckpt-instrumented.log` | `bc38d41` + timing prints in `write_checkpoint` and `reset_pins` (scratch build), on a copy of the directory | the ckpt phase: what a checkpoint costs at this size |
| `ckpt-93a40d0.log` | `93a40d0` (clean tree), on a fresh copy | the ckpt phase after the deferred release; steps not fsynced, run back to back |
| `ckpt-live.log` | `93a40d0` + the ckpt phase's live variant (the test change committed in `22a82c2`), on a fresh copy | the first release's steps between imports, 20 at a time; steps not fsynced |
| `ckpt-live-durable.log` | the tree committed as `22a82c2` (steps fsynced), on a fresh copy | the same, steps fsynced |
| `ckpt-22a82c2.log` | `22a82c2` (clean tree), on a fresh copy | the same, on the commit |

## Computed (A13, `resources::check` for this profile in store mode)

- **The bound:** 5,255,359,885 B (4.894 GiB). After `5abbaf3` (two more stored-version handles):
  5,255,514,985 B. With the 8/7 headroom, 6,006,125,583 B is required, against the 6 GiB budget.
- **One version at the caps** (`node_trie::version_bytes`): 1,977,381,717 B.
- **state.redb's file bound** (`state_file_parts`): 2,388,797,023,392 B. It is a disk bound, but redb's
  allocator memory grows with it.

## Measured

### Grow (`grow.log`)

| at | accounts | slots | RSS | VmHWM | state.redb file | allocated | stored (all versions + checkpoints) | one version's model at these counts | stored / model |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1/20 | 47,689 | 190,104 | 95 MiB | 95 MiB | 64 MiB | 56 MiB | 21 MiB | 88 MiB | 0.24 |
| 10/20 | 475,003 | 1,900,328 | 314 MiB | 314 MiB | 1,028 MiB | 335 MiB | 183 MiB | 884 MiB | 0.21 |
| 20/20 | 950,000 | 3,800,300 | 467 MiB | 474 MiB | 2,056 MiB | 638 MiB | 353 MiB | 1,769 MiB | 0.20 |

- **The state.redb file** grows by doubling, from 64 MiB to 2,056 MiB. At the end, 1.64 GiB of it is free
  pages inside the file, not allocated.
- **The stored bytes** are a fifth of one version's model at the same counts. That figure covers the 33
  versions the entries hold and the 2 checkpoints, which share most of their nodes.
- **The node cache** reached its bound: 267.7 MB at the reopen, against NODE_CACHE (256 MiB) plus each
  entry's share.
- **The journal** was 41.4 MB at height 29,518, and history.redb 21.9–43.2 MB. Both grow with height. At
  H_END this workload would give about 182 MB of journal. The journal's payloads are held in memory during
  open (condition `open-replay-transient`); that was not measured at H_END.
- **Import times** (median per segment): a full-gas block 58 ms at the start, 293 ms at the end; an empty
  block 41 ms, 179 ms.
- **Import stalls at checkpoints:**
  - The slowest empty-block import of each segment grew from 0.2 s to 10.6 s.
  - Records alternate full and empty, so every checkpoint (each 1,024 records) lands on an empty block's
    import.
  - § ckpt attributes the time.
- **Unattributed stalls** in full-block imports: 8.7 s, 8.0 s and 3.5 s, in segments 3, 11 and 4. No
  checkpoint lands on those imports, and they do not line up with the file's doublings.

### Restart and reorg

| run | open | resumed after record | stored versions read (whole / by diff) | VmHWM after open | RSS |
|---|---:|---:|---|---:|---:|
| reopen, `d30dc71` | 783.3 s | 28,672 | all whole | 539,095,040 B | 460.6 MB |
| verify, `d30dc71` | 801.0 s | 28,672 | all whole | 538,980,352 B | 460.6 MB |
| instrumented, `b47fb85` + fixes, under load | 800.7 s | 28,672 | all whole (33) | 473,440,256 B | 461.0 MB |
| verify, `bc38d41` | **262.1 s** | 28,672 | 2 / 31 | **476,962,816 B** | 461.8 MB |

- **Where the 801 s went** (`open-instrumented.log`):
  - resetting the pins: 12.0 s (29 released: the pins of the stopped node's entries);
  - restoring the checkpoint: 548.5 s, of which `Summary::scan_view` took 547.3 s. That is 33 stored versions
    read whole, 16.0–17.2 s each, since each holds about 1.36 M nodes and the node cache holds a fraction of
    one;
  - replaying the 855 records after it: 238.4 s.
- **Fix in `b6d57e8`** (D-6's local mechanism, no rule change):
  - A version whose parent's version was read is now read through `NodeTrie::diff`, and its summary is moved,
    as an import moves it. Every node of every version is still read and checked once.
  - At `bc38d41` the open took 262.1 s. The restore read 2 versions whole (genesis and the oldest of the
    window) and 31 by diff, so replay is now most of the open.
- **Fix 1** (`5abbaf3`, the boxed restore map): VmHWM after open went from 538,980,352 B to 473,440,256 B
  (instrumented build) and 476,962,816 B (`bc38d41`). That is 62–66 MB, or about 2.2 KB for each of the
  28,673 entries restored, in line with the review's estimate of about 2.5 KB.
- **The reorg** (`reopen.log`): a fork from 8 blocks below the head, 9 fallback blocks. It was adopted in
  1.53 s; the adopting import took 554 ms and the others 92–128 ms. The verify phase reopened on the
  reorg's tip.

### The cost of a checkpoint (`ckpt-instrumented.log`)

The ckpt phase opened a copy of the directory (330 s, 2 versions whole, 31 by diff; VmHWM 476,823,552 B).
It encoded the snapshot alone, then wrote three checkpoints in a row:

| | total | encode | pin the new roots | drop the oldest checkpoint | commit | nodes released |
|---|---:|---:|---:|---:|---:|---:|
| encoding alone | 77 ms | 77 ms (4,304,123 B, 29,528 entries, 29 roots) | | | | |
| checkpoint 1 | 13.69 s | 0.065 s | 0.001 s | 11.03 s | 2.49 s | 40,251 |
| checkpoint 2 | 10.29 s | 0.050 s | 0.002 s | 7.96 s | 2.07 s | 35,829 |
| checkpoint 3 | 0.33 s | 0.076 s | 0.001 s | 0.035 s | 0.11 s | 0 |

- **The cause of the import stalls:** releasing what the oldest checkpoint alone held, inside the checkpoint's
  transaction (`drop_one` → `release`), and committing it. That is about 0.2–0.3 ms per node released.
  - The released nodes are the versions from 2,048 records back that nothing newer shares. They lie
    scattered over the node table, and each removal rewrites a page.
  - Checkpoint 3 drops checkpoint 1, which holds the node's current versions, so nothing is released.
  - In a live node this runs in `checkpoint_if_due` after an import, on the consensus thread, so for those
    10–14 s no event is handled.
- **The same cost at open:** `reset_pins` took 12.0 s to release the 29 pins of the stopped node's entries.
- **No rule depends on this.** It is D-6's local mechanism, and it is still open; see the PR's "Still open".
  *(Fixed later the same day in `93a40d0` and `22a82c2`: § After the deferred release.)*
  - Releasing the dropped checkpoint later, in slices between events, would end the stall.
  - But then the store briefly holds one more checkpoint's versions than A13's file term counts
    ((KEPT + 1) sets). The file term and the redb allocator term that depends on it would have to be
    re-derived first.

## After the deferred release (`93a40d0`, `22a82c2`)

`93a40d0` owes a dropped checkpoint's nodes in a `release` table; `release_step` works off 256 owed references
at a time between the node's events, and the next checkpoint's write finishes what is left first.
`22a82c2` fsyncs each step. The ckpt phase was rerun on fresh copies of the directory, after the same open
(275–322 s; 2 versions read whole, 31 by diff; VmHWM 476.7–477.1 MB). Call 0 drops a checkpoint about 1,000
blocks old; in the live runs its release runs 20 steps between imports of empty blocks.

| run | call | the write | release steps (max) | release in all | nodes released | imports during the release: median / max (n) |
|---|---:|---:|---:|---:|---:|---|
| `93a40d0`, back to back | 0 | 0.354 s | 1,399 (61 ms) | 29.8 s | 40,251 | |
| | 1 | 2.465 s | 1,276 (72 ms) | 21.5 s | 35,829 | |
| | 2 | 3.442 s | 1 (3 ms) | 0.003 s | 0 | |
| live, not fsynced | 0 | 0.267 s | 1,399 (28 ms) | 47.2 s | 38,718 | 379 / 905 ms (70) |
| | 1 | 0.231 s | 1,276 (42 ms) | 17.9 s | 35,829 | |
| | 2 | 3.159 s | 130 (23 ms) | 1.7 s | 3,359 | |
| live, fsynced (tree of `22a82c2`) | 0 | 0.358 s | 1,399 (153 ms) | 62.3 s | 38,718 | 137 / 209 ms (70) |
| | 1 | 0.261 s | 1,276 (70 ms) | 48.8 s | 35,829 | |
| | 2 | 0.158 s | 130 (60 ms) | 4.8 s | 3,359 | |
| **`22a82c2`** | 0 | **0.235 s** | 1,399 (50 ms) | 50.0 s | 38,718 | **120 / 155 ms (70)** |
| | 1 | 0.195 s | 1,276 (180 ms) | 40.3 s | 35,829 | |
| | 2 | 0.133 s | 130 (41 ms) | 4.0 s | 3,359 | |

- **The stall is gone.** The write that held the consensus thread 10.3–13.7 s now takes 0.13–0.36 s. The release
  runs as steps of 32–45 ms on average, 180 ms at most, with events handled between them.
- **Why the steps are fsynced.** A step not fsynced leaves its pages for the next durable commit to write:
  - the imports during the release took 379 ms median and 905 ms max;
  - a write right after a release took 2.5–3.4 s (calls 1 and 2 of the back-to-back run, call 2 of the live one);
  - state.redb grew to 8.59 GB (back to back) and 4.30 GB (live): freed pages are not reused before a durable
    commit.

  Fsynced, the imports took 120–137 ms median and 155–209 ms max, and the empty-block imports at the end of the
  grow phase took 179 ms median. The file stayed at 2,156,408,832 B.
- **Fewer nodes released in the live runs** (38,718 against 40,251): the 70 blocks imported during the release
  pin new versions, which share some of the nodes the dropped checkpoint held.
- **Computed for this profile after `93a40d0`** (the file term counts the owed checkpoint and its release rows;
  A13-DERIVATION §3c): 5,374,647,145 B (5.005 GiB), 6,142,453,880 B required against the 6 GiB budget. The file
  bound is 3,350,848,730,760 B.
- `reset_pins` at open (12.0 s at this size) is unchanged: it releases the stopped node's entry pins, not a
  dropped checkpoint.

## Computed against measured

- **Memory:** the highest VmHWM was 539 MB, before fix 1, at an open from a checkpoint. The bound is 4.894
  GiB. The bound's largest terms are worst cases this workload does not reach:
  - the pool at its gas bound;
  - two code sets at CODE_CAP;
  - a block's batch at the most writes a block can make;
  - RPC and peers;
  - the abandoned window of a deep reorg.

  The terms this workload does reach behave as A13 counts them: the stored versions (a fifth of the model),
  the node cache (at its bound) and the entries' metadata.
- **What the measurement establishes:** at 95% of the network's state caps, under this workload, a store-mode
  node stays far below the bound. It grows, restarts from a checkpoint and adopts a reorg.
- **What it does not establish:**
  - that no workload reaches the bound;
  - the H_END journal length (this run reached height 29,518 of 129,600);
  - Windows (this run was Linux only).

  The bound is still a computed bound, not a measured one.
