# Acceptance on 719da04 (2026-10-28)

Runner: the scratch `accept-r4b.sh` (outside the repository), on a clean worktree at 719da04. Order: release bins,
the boot timing, the extension build, then for each state mode e2e_devnet, e2e_site, e2e_browser, e2e_integrated
twice and e2e_heal; then the witnesses. Logs are as the runner wrote them; `kept/` holds what the harness kept of
each failed run (node logs, the network's inputs, the seeds, the manifest with SHA-256 sums), without the
`blocks.journal` files, whose sums are in each manifest. The H_END run of § 4 (nice 10) ran beside it.

| step | whole-state | store |
|---|---|---|
| e2e_devnet | pass | pass |
| e2e_site | pass | pass |
| e2e_browser | **fail** (3 of 80 checks) | pass |
| e2e_integrated, run 1 | **fail** | pass |
| e2e_integrated, run 2 | **fail** | pass |
| e2e_heal | pass | pass |

Witnesses ([witnesses.out](witnesses.out)): 17 NOT_REPRODUCED, 15 controls OK; the Rust witnesses (A01, A03,
A04, A11, A02-*) compiled and ran this time (they did not on b4f13f3).

## The failures, with their causes

Both causes are in the tests, shown by the kept node logs; neither is a node defect. The fixes are in the commit
that follows 719da04, and the steps are run again there.

1. **e2e_browser (whole-state): "RP mutation window ends at a fallback header — no fallback header in the chain",
   and the two checks that need it (items 4 and 5).** The test needs an unsigned (fallback) header at height 14 or
   more. In `kept/e2e_browser-…/work/a.log` the only fallback headers before the check were at heights 3, 5 and 12;
   the next came at 55, after the check. Fallback headers come only when a proposer is late, so whether one exists
   when the check runs depends on timing. Fix (`live_rp.mjs`): the test waits for one, up to 180 s.
2. **e2e_integrated (whole-state, runs 1 and 2): "timeout: a and b on one tip".** This is the catch-up phase added
   for review finding F8 (e028068): a and b are meshed with no miner until they share a tip. During the partition
   both mined a proposer block of the same template at the same height, which gives two sibling tips of equal
   work: run 1 at h=52 (0x06af… and 0x205b…), run 2 at h=160 (0x587b… and 0xa078…), in `kept/e2e_integrated-*/work/a.log`
   and `b.log`. D133 leaves such a tie to local import order, so with no miner nothing breaks it. The flaw is in that
   F8 change, not in the node. Fix (`e2e_integrated.mjs`): only a mines while they mesh, until b is on a's tip;
   then both restart with no miner and b takes whatever a made last.

The store-mode runs passed the same checks; that they did is not evidence against either cause (both depend on
timing: whether a fallback header falls in the window, whether both sides mine at the same height).
