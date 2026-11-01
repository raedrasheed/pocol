# M1 Draft 0.17: Status

**Nothing in 0.17 was executed by the author.** Read with `../m1-draft-0.16/M1-STATUS-0.16.md`.

## Coverage

- **Accepted:** 0.15, at 36 Partial, 5 Not started, 0 Complete.
- **Proposed after a successful root review and test:** E6–E7 Partial, giving **38 Partial, 3 Not started (V2–V4), 0 Complete**.
- **Not M1 completion.** The evidence is model-only and BR22c is not run.

## Evidence plan (for root)

Run `run_checks_017.py`. Expect:
- zero FAIL;
- `suite016.rerun015.countsMatchAccepted`;
- `coverage017.*` all passing;
- the two former C27 FAIL cells now passing against the full timeline;
- `C27.firstWitness@6000` passing.

## Partial gaps (recorded, never green)

- sweepMax control;
- d15c boundary cells at 10100 and 60300;
- d13 checked as properties only;
- x12b literal total: the 0.16 entry, plus `RF-E6-1.x12b.literalTotal` with the blocker and options;
- BR22c (Chrome + CDP) not executed.

## For owner/root

- RF-E6-1: choose option A, B or C (annex §3). No choice is assumed.

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open.
- CR-E4-01/02 remain proposals.
- CONF_DEPTH required; full gate; no merge or deploy.
