# M1 Spec 0.23 Amendment: V4 X8 specification and fixture pages

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Accepted:** 0.22, at 40 Partial / 1 Not started (V4) / 0 Complete.
- Owner decisions, the CR-E4 proposals, RF-E6-1, CONF_DEPTH and the full gate are unchanged. No merge or deploy.

## Adds

1. **`annex/V4-X8.md`.** It separates canonical clauses (B) from reconciled historical definitions (H, cited by message index, round and hash only) and from proposals (P). It gives:
   - the exact layer configuration per K, including DNR fail-closed;
   - the event schema, the experiment plan and the verdict;
   - claim limits, and precision blocks PB1–PB12 with recommended clarifications.
2. **`pages/C1…C8/{index.html, main.js}`.** Literal fixture pages, one per channel, with script-only attempts and the canary host and run tokens.
3. **`vectors/x8-config.json`.** The configuration table: ports, channels, flags, cell rules, layer specs, schema, plan and precision blocks.
4. **`vectors/x8-synthetic-runs.json`.** 14 synthetic logs with hand-derived verdicts.
5. **`tools/x8_eval.py`.** The offline verdict evaluator.
6. **`tools/run_checks_023.py`.** It checks:
   - static asset properties and asset hashes;
   - configuration consistency and canonical provenance;
   - sanitized historical traceability and the no-leak scan;
   - the evaluator against the synthetic verdicts;
   - the absence of network modules, and coverage.

   It writes only `m1-draft-0.23/results/run-results-0.23.json`.
