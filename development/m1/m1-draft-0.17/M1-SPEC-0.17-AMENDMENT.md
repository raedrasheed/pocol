# M1 Spec 0.17 Amendment: C27 repair and RF-E6-1 traceability

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Latest accepted:** 0.15, at 36 Partial / 5 Not started / 0 Complete.
- **0.16 independent run:** 944 pass / 140 recorded / 2 FAIL. Both FAILs are C27.
- Owner decisions, CR-E4-01/02, CONF_DEPTH and the full gate are unchanged. No merge or deploy.

## Changes

1. **C27 (fixture correction only; model unchanged).** The BR24-x9 adminEarlyRelease faulty path now has the full hand-derived timeline (`vectors/c27-x9-early-release.json`, annex §1):
   - the exact first witness at 6000 (3 unsettled) is kept;
   - the final shows a maximum of 4 and six sets, because of T2b at 11000;
   - the control is not weakened.
2. **RF-E6-1.** A supplemental full timeline (`vectors/rf-e6-1-x12-timelines.json`, annex §3):
   - covers x12, x12b (global adminNoCancel) and an op1-only analysis mode;
   - shows the omitted 5001 retry;
   - gives per-op and total stale drops, with zero alloc, ticket and set for the stale requests;
   - includes the complete FIFO guard trace and the recreated K1.

   The blocker and options A, B and C are recorded. Nothing is accepted.
3. **Conventions.** RF-E7-1 and RF-E7-2 are restated as CONV-E7-1 and CONV-E7-2. They are not source changes.
4. **Tooling:**
   - `tools/admin_trace.py`, a trace-only subclass;
   - `tools/run_checks_017.py`, which replays the full 0.16 suite (and with it the 0.15 replay) with only the C27 entry replaced, then the new checks and a coverage guard.

   Old `main()` functions are never called; preserved result files are hashed before and after. The runner writes only `m1-draft-0.17/results/run-results-0.17.json`.
