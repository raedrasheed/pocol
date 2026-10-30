# M1 Spec 0.18 Amendment: C28 checker repair

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Latest accepted:** 0.15.
- **0.17 independent run:** 1037 pass / 156 recorded / 1 FAIL (C28, a checker alias at `x9 row@5000.violations`).

## Changes

1. **`tools/observe_capture.py`.** Observation-time deep copy installed into the loaded 0.16 and 0.17 runner globals (annex table). Nothing in the specification, the models or the goldens changes.
2. **`tools/run_checks_018.py`:**
   - adversarial nested-mutation regressions, including a pristine-defect control;
   - a replay of the full 0.17 scope, which includes the 0.16 suite and the 0.15 read-only replay;
   - the x9 per-row history;
   - the `coverage018` guard;
   - input SHA records and preserved-file hashes.

   No old `main()` is called. It writes only `m1-draft-0.18/results/run-results-0.18.json`.

## Status

C27 stays closed. RF-E6-1 stays an open partial gap with its options unapproved. The proposed coverage after root acceptance is unchanged at 38 Partial / 3 Not started (V2–V4) / 0 Complete.
