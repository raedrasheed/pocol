# M1 Draft 0.24: Status

**Nothing in 0.24 was executed by the author. No Chrome, CDP or CanarySink result exists or is claimed.**

## Coverage

- **Accepted:** 0.22, at 40 Partial, 1 Not started (V4), 0 Complete.
- **Proposed after a successful root review and test:** V4 Partial, giving **41 Partial, 0 Not started, 0 Complete**.
- **Why V4 stays Partial:**
  - the X8 matrix and the C7 per-path windows are specified, not executed;
  - PB1–PB12 remain open, with PB6 and PB8 now carrying proposed resolutions;
  - the RtcLockdown argument is unproven by design.

## Expected on root's run

- Zero FAIL.
- `C31.new.*`, `C31.old.*` (the old evaluator's derived behaviour, including its false PASS) and `C31.rootProbeReplica` pass.
- `C31.duplicateSweepBothOrders` passes over 54 runs.
- `C7plan.*` and `coverage024.*` pass.
- Every `suite023.*` entry passes or is recorded under the new evaluator.

## Next

The consolidated specification/fixture completeness audit.

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open.
- CR-E4-01/02 remain proposals.
- RF-E6-1 owner question pending.
- CONF_DEPTH required; full gate; no merge or deploy.
