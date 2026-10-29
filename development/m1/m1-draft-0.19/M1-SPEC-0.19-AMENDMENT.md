# M1 Spec 0.19 Amendment: V2 LogClient specification and reference fixtures

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Accepted:** 0.18, at 38 Partial / 3 Not started (V2–V4) / 0 Complete. C28 is closed; RF-E6-1 is open with an owner question.
- Owner decisions U01, U02, U10, U14 and CR-M1-01, the proposals CR-E4-01/02, CONF_DEPTH and the full gate are unchanged. No merge or deploy.

## Adds

1. **`annex/V2-LOGCLIENT.md`.** English normative text:
   - the page path, limits and counters;
   - the full step table;
   - the attempt FSM, termination and the correctness lemma;
   - the sink contract and S-a..S-e;
   - consumers and collectors, boundaries, and conventions P-V2-1..8.
2. **`tools/logclient_ref.py`.** Pure-stdlib reference with a manual clock: `step`, `LogClient`, `MockLogServer`, `SinkChecker`, the consumers, `fetch_all`/`fetch_each` and `Bridge`, with nine named control faults.
3. **Hand-derived fixtures:**
   - `vectors/lc-data.json`: branches, literal hashes, RefA/RefB/RefA′;
   - `vectors/lc-step-table.json`;
   - `vectors/lc-cases.json`: LC1–LC18, every variant, supplements and controls;
   - `vectors/lc-sink-controls.json`.
4. **`tools/run_checks_019.py`.** A standalone runner:
   - input SHAs, and preserved older results and ledger hashed before and after;
   - a safe positional-only logger that copies diagnostics at call time;
   - provenance literals for every case;
   - a coverage guard for every LC case and variant, the nine controls and the fifteen sink controls.

   It writes only `m1-draft-0.19/results/run-results-0.19.json`.

## Source points needing review

- P-V2-1 to P-V2-8 (annex §8) are conventions or proposed supplements, recorded as partial gaps.
- **P-V2-1 is a concrete wording gap.** In `X-maxBeforeBegin` (LC16 with maxRequests = 6), the budget runs out at attempt 2's `H(latest)`. There is no open attempt id for the `abort(id, tooManyRequests)` that browser.md:107 names. The model returns `Error{tooManyRequests}` without begin or abort. A supplementary sentence is proposed; nothing in the source is redefined.
