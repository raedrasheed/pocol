# M1 Spec 0.20 Amendment: C29 SinkChecker grammar repair

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Latest accepted:** 0.18.
- **0.19 independent run:** 493 pass / 23 recorded / 0 FAIL. Root's probe nonetheless showed a false green: an unknown terminal event was accepted.

## Changes

1. **`tools/sink_checker_strict.py`:** a closed sink grammar with exact types and arities.
   - Only a valid `abort` or `commit` settles an attempt.
   - Malformed events give S-a.
   - Malformed entries and summaries give S-e.
   - A commit anchor that differs from its begin anchor gives S-e.
2. **`tools/run_checks_020.py`:**
   - installs the strict checker in the loaded 0.19 model globals, in memory only;
   - replays the whole 0.19 suite, including the 15 exact sink controls and all 43 case-level SinkChecker runs;
   - runs 25 C29 controls against both the strict and the pristine checker;
   - checks the deferred-experiment specifications, coverage and preserved files.

   It writes only `m1-draft-0.20/results/run-results-0.20.json`.
3. **`vectors/c29-controls.json`** and **`vectors/v2-deferred-experiments.json`.**
4. **`annex/C29-SINK-GRAMMAR.md`.**

The client FSM, branches, goldens, limits and owner policy are unchanged.
