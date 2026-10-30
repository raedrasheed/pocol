# Acceptance on ad301e7 (node code: the deferred checkpoint release and the review's fixes)

- **Why it was run:** `93a40d0`, `22a82c2` and `ad301e7` changed node code: the deferred checkpoint release, its
  fsynced steps, and the independent review's fixes.
- **The tree:** `ad301e7`. `HEAD` records the commit and its prototypes tree. The 2 untracked paths are the
  worktree's `target` and `node_modules` links.
- **Not covered:** `e453851` came after this run. It changes only how open handles a store it finds damaged,
  which no step below exercises. Acceptance on `e453851` is in `../accept-e453851/`.
- **Scope:** local nodes, local RPC and a test Chrome profile only. The runner and the witness programs stay
  outside the repository.
- **The files:** `summary` gives each step's exit code. Each log holds the step's command, start, end and output.
- **When:** 2026-10-27, 22:35–23:25 UTC.
- **Load:** two seeded-fault and sweep runs ran beside it. The first ran at nice 19. A test of mine ran at
  normal priority (twelve back-to-back runs of the damage test) while `e2e_browser` ran in whole-state mode.

| step | whole-state mode | store mode |
|---|---|---|
| pocold binaries (release) | built | |
| boot calibration (`jm_r14_boot_timing_by_mode`, 10 runs) | 9–20 ms per phase | 13–35 ms (t_exec 1,000 ms) |
| e2e_devnet | **PASS 44/44** | **PASS 44/44** |
| e2e_site | **PASS 21/21** | **PASS 21/21** |
| e2e_browser (Chrome) | **FAIL: 44 of 45 checks passed, then the driver stopped** | **PASS 80/80** |
| e2e_integrated | **PASS 20/20**, two runs | **PASS 20/20**, two runs |

## The e2e_browser failure (whole-state mode)

`e2e-browser-whole-state.log`, at BR18 (site navigation):
- BR18a and BR18b passed: the frame navigated to `/about.html`, then to `/docs/`.
- The next step, `fr.click('#home')` on the docs page's "home" link, timed out after 30 s in Playwright. The link
  was found and visible, so the click itself never completed.
- The driver then stopped, at 45 of its 80 checks.

This step does not touch the node's state mode: whole-state mode uses no state store. The same steps passed
in store mode in this run, and in both modes on `fbe6062` and `d45dfe2`. The failure is recorded here as it
happened; its cause is not established. On `e453851`, with the same extension and test code, the same steps passed 80/80 in both modes
(`../accept-e453851/`, run while nothing else ran at normal priority).

**Security witnesses** (`../witnesses-ad301e7.log`): the Rust, live and extension parts.
- Covered: A01, A02 (R9, R13, SYS_SLOT_MAX), A03, A04, A05, A06 (three scenarios), A07, A08, A09, A10, A11, A12,
  A14.
- **17 NOT_REPRODUCED, none reproduced; every control OK.**
