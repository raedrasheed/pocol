# M1 Draft 0.18: Status

**Nothing in 0.18 was executed by the author.**

## Coverage

- **Accepted:** 0.15, at 36 Partial, 5 Not started, 0 Complete.
- **Proposed after a successful root review and test:** E6–E7 Partial, giving **38 Partial, 3 Not started (V2–V4), 0 Complete**.
- **Not M1 completion.**

## Expected on root's run

- Zero FAIL.
- `suite017.suite016.BR24-x9.faultyPath.adminEarlyRelease.row@5000.violations` passes with `[]`.
- The row at 6000 passes with the exact violation.
- The `C28.*` regressions pass, and the pristine control detects the defect.
- `coverage018.*` passes.

## Partial gaps (unchanged, recorded)

- sweepMax control;
- d15c boundary cells at 10100 and 60300;
- d13 checked as properties only;
- the x12b literal total (RF-E6-1, options A, B and C unapproved);
- BR22c (Chrome + CDP) not executed.

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open.
- CR-E4-01/02 remain proposals.
- CONF_DEPTH required; full gate; no merge or deploy.
