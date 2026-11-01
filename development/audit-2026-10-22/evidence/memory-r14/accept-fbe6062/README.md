# Acceptance on fbe6062 (node code as b6d57e8: the review fixes and stored versions read by diff at open)

- **Why it was rerun:** `5abbaf3` and `b6d57e8` changed node code (checkpoint restore and A13's counts), so the
  acceptance on `d45dfe2` no longer covered the code under test.
- **The tree:** `fbe6062`'s prototypes are `bc38d41`'s, which are `b6d57e8`'s plus an ignored measurement.
- **Scope:** local nodes, local RPC and a test Chrome profile only. The runner and the witness programs stay
  outside the repository.
- **The files:**
  - `HEAD` records the commit and its prototypes tree. The 2 untracked paths are the worktree's `target` and
    `node_modules` links.
  - `summary` gives each step's exit code. Each log holds the step's command, start, end and output.
- **When:** 2026-10-27, 20:25–21:09 UTC. Two scratch builds ran beside the last minutes of the witnesses, both
  at low priority.

| step | whole-state mode | store mode |
|---|---|---|
| pocold binaries (release) | built | |
| boot calibration (`jm_r14_boot_timing_by_mode`, 10 runs) | 8–20 ms per phase | 11–23 ms (t_exec 1,000 ms) |
| e2e_devnet | **PASS 44/44** | **PASS 44/44** |
| e2e_site | **PASS 21/21** | **PASS 21/21** |
| e2e_browser (Chrome) | **PASS 80/80** | **PASS 80/80** |
| e2e_integrated | **PASS 20/20**, two runs | **PASS 20/20**, two runs |

**Security witnesses** (`../witnesses-fbe6062.log`): the Rust, live and extension parts.
- Covered: A01, A02 (R9, R13, SYS_SLOT_MAX), A03, A04, A05, A06 (three scenarios), A07, A08, A09, A10, A11, A12,
  A14.
- **17 NOT_REPRODUCED, none reproduced; every control OK.**
