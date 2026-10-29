# M1 Spec 0.24 Amendment: C31 X8 evidence integrity

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Accepted:** 0.22.
- **0.23 independent run:** 104 pass / 29 recorded / 0 FAIL, but root's probe found C31: a duplicate empty window erased a K0 reach (PASS).

## Changes

1. **`tools/x8_eval_v2.py`.** The evaluator now applies an integrity contract (annex §2):
   - malformed or unknown input is INVALID, with deterministic diagnostics;
   - duplicate windows are INVALID;
   - evidence is aggregated per cell, so no reach is ever lost;
   - self-test sensitivity is strict.
2. **`vectors/c31-controls.json`.** 34 controls, with the expected outcome for both the new and the old evaluator.
3. **`vectors/c7-path-plan.json`** and **`pages-supplemental/C7-P1…P5`.** The supplemental per-path C7 plan, specified only. PB6 and PB8 remain open.
4. **`tools/run_checks_024.py`.** It:
   - patches the 0.23 evaluator in memory and replays the whole 0.23 suite;
   - runs the controls, the root-probe replica and the both-orders duplicate sweep;
   - runs the C7 plan static checks, the no-network check and the leak scan.

   It writes only `m1-draft-0.24/results/run-results-0.24.json`.

No 0.23 asset, golden, clause, flag, experiment claim or owner position changes.
