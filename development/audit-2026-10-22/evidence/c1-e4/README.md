# evidence/c1-e4: C-1 E4 (state.redb store mode, D-6 checkpoints) and E5

Every file names the commit it was produced on. Earlier files are kept as they were recorded; the
corrections and final results are in the files added on 2026-10-25 (below). Mutation, witness and
adversarial runners are kept out of the repository; only their logs are here.

**Final tree:** `fc249699bad145100bba2cfaa9687647c9ee123e` (prototypes tree
`c3e04e931559fc4b4122dc254da72b1600ee2322`). Later commits change only documentation and evidence. The
measurements were taken on `5ab73d1`, which differs from it only in one comment in state_store.rs.

**Commits that are not on the branch.** `c389ca1`, `22942c7`, `23a5e5d` and `48ac524` are E4.5 part 1, the
E4.4 follow-up, and E5 parts 1 and 2 before they were rebased onto `45d540f`. Each differs from its commit
on the branch (`314de11`, `3c0fe15`, `90ba824`, `5722c23`) only in `45d540f`'s test change in
tests/consensus.rs: `copy_dir` skips LOCK, and the leftover test closes its node first (`git diff --stat`:
one file, 5 insertions, 2 deletions). The logs keep the commit each run used.

**Machine** (Linux runs): Intel Xeon @ 2.80 GHz, 4 vCPU under KVM, 15 GiB RAM, ext4 on virtio; Linux
6.18.44; rustc 1.99.1 for lp4 and pocold, 1.58.1 for lp1-node; Node 22.22.0; Playwright's Chromium.

## Recorded before 2026-10-25 (unchanged)

| file | what | commit |
|---|---|---|
| e41-suite-lp4.log, e41-suite-pocold.log | E4.1 suites | as recorded in each |
| mutants-e41.log | E4.1 mutation controls | as recorded |
| mutants-e42.log, mutants-e43.log | E4.2 and E4.3 mutation passes | frozen tree b91c490 |

## Added on 2026-10-25

| file | what | commit |
|---|---|---|
| mutants-e44.log | E4.4 (D-6) pass 1: 10 of 20 killed, and what answered each survivor | 1a5b792 |
| mutants-e44-retest.log | the retest of pass 1's survivors and of E4.3's C7 and C10: 8 of 13 killed, and the tests that answer the 5 survivors | 22942c7 |
| mutants-final.log | the final pass: 30 of 30 killed (E4.4's 21, E4.3's C7 and C10, E5's 7), none classed equivalent | fc24969 |
| a13-final.log | `derive()` at the network and devnet profiles, whole-state and store mode | fc24969 |
| disk-impact-final.md | disk, time and RSS of store mode against whole-state mode; state.redb's file factor per phase; the close A/B | 5ab73d1 |
| disk-final-*.log | raw output of those measurements | 5ab73d1 |
| disk-close-ab-*.log | the close change A/B | 5ab73d1, and 5ab73d1 with one line changed |
| disk-impact-early-c389ca1.log, disk-impact-early-48ac524-large.log | the first measurements (the file factor of about 18) | c389ca1, 48ac524 |
| accept/ | the final suites and the acceptance runs, one log per step, with their command, times and exit code | fc24969 |
| windows-runs.md | the Windows runs on the E4.4 commits: what failed and why, and the run on the final head | as named |

## Final acceptance on fc24969 (accept/README.md)

- **Linux, all passed:** lp4 193/0; pocold 221/0 (20 ignored, each listed with whether and where it ran); the
  crash-step, kill and damage tests; the store-mode spec-depth test; e2e_devnet 42/42; e2e_site 20/20;
  real-Chromium e2e_browser 79/79; LP10 e2e_integrated 18/18; the extension's 234 unit tests.
- **Linux, not a pass:** lp1-node's 11 store_journal tests return UnsupportedPlatform, because that journal
  is Windows-only.
- **Windows run 61:** PASS on every check (windows-runs.md); run 62 on the docs-only head f14f2c2 passed too.
- **Mutation controls:** the final pass on fc24969 killed 30 of 30 (mutants-final.log).
- **Deep-reorg `--ignored` tests:** 2048/2049 adoption, D133 at depth 400 and A11 passed (accept/).
- **Not run in this pass:** the debug suites, the witnesses, the D-REORG-REDB and RpcGuard mutation passes,
  the RSS run under load, and the R14/R19 calibration. They are not counted.

