# C-1 E4/E5: store mode's effect on disk, time and RSS (final tree)

These are measurements, not the A13 bound. A13 is computed (a13-final.log; A13-DERIVATION.md §2c), and these
small states do not test it.

## Setup

- **Commit:** `5ab73d1fdde57c181eeb1040bd2fc047c79e05cb` for every row below except the close A/B (see that
  section). The final commit `fc24969` differs from it only in one comment in state_store.rs (the sizes
  this A/B measured).
- **Test:** `journal_measure::jm_store_mode_disk_impact` (release, `--ignored --nocapture`), one mode per
  process. Raw output: disk-final-*.log.
- **Machine:** Intel Xeon @ 2.80 GHz, 4 vCPU under KVM, 15 GiB RAM, ext4 on a virtio disk. The kernel reports
  rotational=1 for that disk; the backing medium is not known. Linux 6.18.44; rustc 1.99.1.
- **Load:** nothing else ran on the machine during these four runs.
- **Workload:**
  - small: 12 blocks that create accounts, 40 full-gas transfer blocks, then a heavier 9-block fallback
    branch from 8 blocks below the tip;
  - large: 70 account-creating blocks, 8 full blocks, then the same kind of reorg.
- **Configuration:** devnet profile, default limits, checkpoints every 16 records.

## State and history sizes

| | small | large |
|---|---:|---:|
| blocks (then side blocks of the reorg) | 52 (+9) | 78 (+9) |
| accounts at the head | 2,770 | 16,110 |
| nonzero slots | 114 | 114 |
| transactions | 12,172 | 70 |
| stored versions (store mode) | 29 | 29 |
| trie nodes in state.redb | 7,788 | 36,426 |
| bytes stored in state.redb (keys and values) | 2.82 MB | 14.90 MB |
| blocks.journal | 1.93 MB | 0.52 MB |
| history.redb | 17.38 MB | 4.75 MB |
| state.redb after live import | 8.96 MB | 67.91 MB |

## Time

Times are wall clock. Import times are per block.

| | small, whole-state | small, store | large, whole-state | large, store |
|---|---:|---:|---:|---:|
| import, account-creating blocks: median / max | 47.8 / 91.7 ms | 114.4 / 154.2 ms | 378.0 / 787.6 ms | 465.6 / 1,045.1 ms |
| import, full blocks: median / max | 179.5 / 267.5 ms | 331.2 / 4,841.8 ms | 817.7 / 1,068.5 ms | 768.6 / 856.3 ms |
| sync of the whole chain (all imports) | 8.08 s | 23.00 s | 31.18 s | 37.97 s |
| reorg (the heavier branch's imports) | 1.52 s | 1.01 s | 12.30 s | 4.54 s |
| open (store mode: resumes from its checkpoint) | 9.66 s | 5.32 s | 39.89 s | 9.99 s |
| open replaying from genesis (store mode: store emptied first) | 8.89 s | 33.26 s | 41.08 s | 51.47 s |

Reading:
- **Import in store mode.** Every block's commit is fsynced before its journal record (I3).
  - Small store: about 1.8–2.4× slower per block, and sync 2.8× slower. One full block took 4.8 s, an
    outlier; the median is 331 ms. Store-mode times vary with this disk's fsync latency (see the A/B).
  - Larger state: account-creating blocks are 1.23× slower; full blocks are about equal (0.94×).
- **Reorg.** Store mode is faster: 1.5× on the small store and 2.7× on the large one. It reads stored
  versions in place instead of rebuilding whole states.
- **Open.** From a checkpoint, open is 1.8× (small) and 4.0× (large) faster than whole-state mode's replay.
  A store-mode replay from genesis is slower than whole-state replay, because it refills the store.

## RSS (the test process's VmRSS and VmHWM from /proc/self/status, in MB)

| | small, whole-state | small, store | large, whole-state | large, store |
|---|---:|---:|---:|---:|
| RSS before the node opens | 34.7 MB | 34.7 MB | 52.7 MB | 62.5 MB |
| RSS after sync and reorg | 42.0 MB | 48.3 MB | 113.4 MB | 112.7 MB |
| VmHWM after sync and reorg | 44.2 MB | 48.9 MB | 113.4 MB | 112.7 MB |
| VmHWM after both reopens | 49.6 MB | 52.0 MB | 127.4 MB | 124.3 MB |

The test process also holds the generated blocks, and "before" is measured after they are built, so the
differences between columns are what the node adds. The node cache held 4.5 MB (small) and 22.9 MB (large),
against its 256 MiB bound.

## state.redb file over stored bytes, per phase (store mode)

| phase | small: file | small: stored | small: file/stored | large: file | large: stored | large: file/stored |
|---|---:|---:|---:|---:|---:|---:|
| after live import | 8.96 MB | 2.82 MB | 3.18 | 67.91 MB | 14.90 MB | 4.56 |
| after closing it | 17.38 MB | | (6.17) | 67.91 MB | | (4.56) |
| open resumed from a checkpoint | 13.17 MB | 2.82 MB | 4.68 | 67.91 MB | 14.90 MB | 4.56 |
| open replayed from genesis (no checkpoint pins) | 17.38 MB | 1.54 MB | 11.31 | 67.91 MB | 11.18 MB | 6.07 |

- redb's allocated pages were at most 2.37 times the stored bytes on the small store and 1.77 times on the
  large one.
- **Why closing grows the small file.** redb writes its allocator state into the file when the database
  closes (redb 2.6.4 db.rs, `Database::drop` → `ensure_allocator_state_table`). On the small, nearly full
  file that needs new pages, and below one region redb grows a file by doubling.
- **What A13 counts.** STATE_FILE_FACTOR is 12, above every value here.

## The close change, A/B on the same commit

The A/B runs `5ab73d1` as committed and with one line changed: Node's Drop does not call
`StateStore::close`, which is the behaviour before C-1 E5 part 3. Raw output: disk-close-ab-*.log.

| state.redb file | small, with close | small, without | large, with close | large, without |
|---|---:|---:|---:|---:|
| after live import | 8.96 MB | 8.96 MB | 67.91 MB | 67.91 MB |
| after closing it | 17.38 MB | 17.38 MB | 67.91 MB | 67.91 MB |
| after an open resumed from a checkpoint | 13.17 MB | 17.38 MB | 67.91 MB | 67.91 MB |
| after closing it | 13.17 MB | 17.38 MB | 67.91 MB | 67.91 MB |
| after an open replayed from genesis | 17.38 MB | 17.38 MB | 67.91 MB | 67.91 MB |
| **after closing it** | **17.38 MB** | **34.22 MB** | **67.91 MB** | **270.02 MB** |
| open from the checkpoint / from genesis | 5.50 s / 18.47 s | 5.67 s / 18.32 s | 10.19 s / 51.58 s | 10.76 s / 50.59 s |

Reading:
- **Without the change**, closing a node that holds many versions releases each one in a non-durable
  transaction of its own. redb reuses those pages only after a durable commit, so after a replay from
  genesis the close grew the file 2× (small) and 4× (large).
- **With the change**, the close leaves the versions pinned, and the next open releases them in one durable
  transaction.
- **Open times are equal** within noise.
- **Store-mode times vary with the disk.** Store mode fsyncs every block's commit, and this virtual disk's
  fsync latency varies between runs. On the same code and workload, the small store's open from genesis
  took 33.3 s in the timing run above and 18.5 s here. Neither run shared the machine. The 4.8 s full-block
  outlier above is of the same kind. Read store-mode times as indicative, within about a factor of two.

## Earlier measurements (kept as recorded)

- disk-impact-early-c389ca1.log: commit c389ca1 (E4.5 part 1), the small workload in both modes.
- disk-impact-early-48ac524-large.log: commit 48ac524, the large workload in store mode, before the close
  change. It reports stateFileFactor 18.12. That figure was measured after two reopens and their closes, as
  the A/B above explains.
