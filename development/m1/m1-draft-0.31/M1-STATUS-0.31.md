# M1 Draft 0.31: Status

**The author ran nothing. M1 is not complete.** `tools/run_checks_031.py` computes the statuses below and writes them to `status-0.31.json`. It fails `consolidation.rowsMatchProposal` if they differ.

| Rows | Expected | Remaining |
|---|---|---|
| 38 (34 earlier, plus C1, C2, R1, E6) | CompleteCandidate | root's ledger action |
| C3, C4 | PendingRootReview | root confirms the final bindings of the 0.30 C34 repair and the shared RP budget (`bind030.*`, `U14.rp.*`) |
| V1 | PendingRootReview | root review of C35 (`codec.*`, `C35.*`) and C36 (`C36.*`), and the 0.31 C33 reruns |

## Exact remaining root work

1. **Run the suite.** Execute `run_checks_031.py` in a preserved copy, with Node v22.13.1 reachable (`--node`, `POCOL_NODE` or `PATH`), and require zero FAIL.
   - Without Node, the `codec.*`, `C35.*` and `C33.*` checks fail. That is intended.
2. **Check C35 independently.** Compare the 17 oracle rows and the 6 boundaries with root's native results. Confirm zero codec calls for the ordering cases.
3. **Check C36.** Confirm the two parsed-literal and review-decision bindings.
4. **Record final verification in the ledger** for C35, C36, C06, C07, U08-CR, RF-E6-1, V1-SHA-COST and P-C32-2, and for findings F01–F26.
5. **Keep the gate unmet** (F26) until all 41 rows are Complete.

If evidence is missing, the run names it in `coverage031.required`, or in the `missingRequired` field of `status-0.31.json`.

## Preserved failures

| ID | Kept in | Rebound to |
|---|---|---|
| HF-8 | four 0.29 FAILs | 0.30 passes plus `coverage031.required` |
| HF-12 | six 0.30 FAILs | the repair checks in `vectors/c36-binding-repairs.json` |
| HF-13 | root's first native comparison (platform-default loader) | `C35.oracle.utf8Readers`, `C35.direct.oracle030.emojiData` |
| HF-14 | the genuine lone-surrogate undercount | `C35.direct/integrated` lone-surrogate checks |
