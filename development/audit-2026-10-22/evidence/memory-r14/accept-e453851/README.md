# Acceptance on e453851 (node code: the deferred checkpoint release, the review's fixes, open on a damaged store)

- **The tree:** `e453851`, which holds every node change of the deferred release: `93a40d0`, `22a82c2`,
  `ad301e7` and `e453851`. `HEAD` records the commit and its prototypes tree. The 2 untracked paths are the
  worktree's `target` and `node_modules` links.
- **Scope:** local nodes, local RPC and a test Chrome profile only. The runner and the witness programs stay
  outside the repository.
- **The files:** `summary` gives each step's exit code. Each log holds the step's command, start, end and output.
- **When:** 2026-10-27, 23:13–23:53 UTC.
- **Load:** a sweep and a seeded-fault run ran beside it at nice 19.

| step | whole-state mode | store mode |
|---|---|---|
| pocold binaries (release) | built | |
| boot calibration (`jm_r14_boot_timing_by_mode`, 10 runs) | 8–19 ms per phase | 12–37 ms (t_exec 1,000 ms) |
| e2e_devnet | **PASS 44/44** | **PASS 44/44** |
| e2e_site | **PASS 21/21** | **PASS 21/21** |
| e2e_browser (Chrome) | **PASS 80/80** | **PASS 80/80** |
| e2e_integrated, run 1 | **FAIL at check 10:** "timeout: c reorganized" | **PASS 20/20** |
| e2e_integrated, run 2 | **PASS 20/20** | **PASS 20/20** |

## The e2e_integrated failure (whole-state, run 1): which side reorganizes after the heal

The test partitions c from a and b and waits until the a/b branch has more total work. It then restarts all
three nodes, meshed, and waited for **c** to reorganize.
- **What the node logs showed** (read before the runner's cleanup removed the work directory; the files were
  not kept):
  - c had mined its own branch without a break, from h=1 to 310, with no reorganization;
  - a and b each logged "TrieRebuild for a fork below the K_hist window" after the restart;
  - all three ended on one head, h=310 `0x38ea5d36a8d6`.
- **So a and b reorganized onto c's branch.** By the time the nodes reconnected, c's branch had more total work:
  the three mine on while they reconnect, and c's uninterrupted blocks carry a higher difficulty. That is fork
  choice by total work (D133) as specified.
- **The fault is in the test:** it assumed the side that reorganizes is always c. No node code is involved.
  Whole-state mode does not use the state store.
- **The fix is in the test only, in the commit after `e453851`:**
  - the test now waits until the three nodes share one head, and the side that lost (c, or both a and b) has
    reorganized;
  - the checks after it (the canonical site version on every node, the pools, v3, the restarts) already allowed
    either branch to win.

The reruns with the fixed test are in `../integrated-rerun/`.

**Security witnesses** (`../witnesses-e453851.log`): the Rust, live and extension parts.
- Covered: A01, A02 (R9, R13, SYS_SLOT_MAX), A03–A12, A14.
- **17 NOT_REPRODUCED, none reproduced; every control OK.**
