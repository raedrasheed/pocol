# Acceptance on d45dfe2 (node code as 0c2ad75: the MODEXP precompile with both fast paths)

`b46658a` and `0c2ad75` changed node code: the EVM's precompile at address 5. So the acceptance on `8d80354`
no longer covered the code under test, and it was run again on `d45dfe2`, whose prototypes tree is `0c2ad75`'s
plus tests.

- Local nodes, local RPC and a test Chrome profile only. The runner and the witness programs stay outside the
  repository.
- `HEAD` records the commit and its prototypes tree. The 2 untracked paths are the worktree's `target` and
  `node_modules` links.
- `summary` gives each step's exit code. Each log holds the step's command, start, end and output.
- 2026-10-27, 17:04–17:49 UTC. The store-mode scale run (`jm_store_scale`) ran on the same host throughout, and
  from 17:47 so did a mutation run. So timings here are under load.

| step | whole-state mode | store mode |
|---|---|---|
| pocold binaries (release) | built | |
| boot calibration (`jm_r14_boot_timing_by_mode`, 10 runs) | 8–30 ms per phase | 11–38 ms (t_exec 1,000 ms) |
| e2e_devnet | **PASS 44/44** | **PASS 44/44** |
| e2e_site | **PASS 21/21** | **PASS 21/21** |
| e2e_browser (Chrome) | **PASS 80/80** | **PASS 80/80** |
| e2e_integrated | **PASS 20/20**, two runs | **PASS 20/20**, two runs |

**Security witnesses** (`../witnesses-d45dfe2.log`; the Rust, live and extension parts, all three this time):
A01, A02 (R9, R13, SYS_SLOT_MAX), A03, A04, A05, A06 (three scenarios), A07, A08, A09, A10, A11, A12, A14.
**17 NOT_REPRODUCED, none reproduced; every control OK.**

Not covered by this run: `b8fb7ab`, which routes trivial MODEXP powers to revm. Its results are revm's for every
input either way (lp4 209/0, differential tests), and its Windows run is the next on the branch.
