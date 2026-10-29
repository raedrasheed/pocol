# M1 — Draft 0.8: Amendment R8-01 (C23 repair only)

Status: **review draft, NOT approved. Phase S only.** This is a deliberately small repair cycle; no new annex batch is included. **Nothing in 0.8 was executed by the author**, because the session had file tools only.

## R8-01 — C23

**Defect.** `m1-draft-0.7/tools/run_checks_07.py` line 453 (`e_checks`) reads the U3 state from `m1-draft-0.6/vectors/snapshot-invariants.json`. That file does not exist. The fixture was written in draft 0.4 (R4-01) as `m1-draft-0.4/vectors/snapshot-invariants.json`. As a result the alternative-E comparison step raised an exception. The coordinator's run gave 252 entries: 249 passed, 2 recorded, 1 failed, exit 1.

The same wrong reference appears as prose in `m1-draft-0.7/vectors/snapshot-e-cases.json` (`states`). It is corrected here by reference: read it as "U3 from m1-draft-0.4/vectors/snapshot-invariants.json". 0.7 is not modified.

**Repair.** `tools/run_checks_08.py`:
1. **Lineage.** It verifies the SHA-256 of the inherited inputs against the `localSha256` values recorded by the coordinator in `coordination/shared-repo/development/m1/PUBLICATION-MANIFEST.json`:

   | File | localSha256 |
   |---|---|
   | `m1-draft-0.4/vectors/snapshot-invariants.json` (the source fixture) | `760ef12403607d628149999be3f0a5adb5908608c35d03d8525f8c591418f957` |
   | `m1-draft-0.3/vectors/snapshot-mock-scenarios.json` | `b1602db9256512ad2f40ca4d374c40ee55b39aa34be8758a9a426869a86a484a` |
   | `m1-draft-0.7/vectors/snapshot-e-cases.json` | `1b875889f6044f0732be744fd390e850751cd8d3149e201006eefdebb634f47e` |
   | `m1-draft-0.7/tools/run_checks_07.py` (preservation of the 0.7 runner) | `badd6e346db21b0ff6463a9ae3e670e8b0f896c73d1db286bc41f7b782cbc8fa` |

   I copied these values from the coordinator's manifest. I did not compute them. A mismatch fails the run.

   It also checks three facts about the paths:
   - the wrong 0.6 path does not exist;
   - the 0.4 file contains U3;
   - the other 0.6 fixture that 0.7 reads, `profile-race-cases.json`, still resolves.
2. **Inheritance.** It imports `run_checks_07` as a module and does not call its `main()`, so nothing is written into 0.7. It runs the 0.7 steps unchanged: `c20_checks`, `epoch_checks`, `br19_checks`, `mr_checks`, `session_checks` and `fetch_rule_checks`. These are the applicable regressions.
3. **Fixed step.** `e_checks_08` is 0.7's `e_checks` with only the U3 source path changed. It runs the five comparative alternative-E cases (E-1a, E-1b, E-1c, E-2, E-3) and records the getter selector.

## Status and coverage

Unchanged from 0.7 apart from C23:
- 27 rows partial, 0 blocked, **14 not started** (Q1–Q4, E1–E7, V2–V4), 0 complete. **Full M1 remains incomplete.**
- Open owner decisions: **CR-M1-01**, **U01**, **U02**, **U10**, **U14**.
- Still unexecuted:
  - the browser and MV3 extension, and DNR;
  - MaliciousHttp over real sockets, and TestHooks;
  - anvil, pocold, the `eth_call` getter and MPT;
  - ESLint and dependency-cruiser;
  - any real transaction.
