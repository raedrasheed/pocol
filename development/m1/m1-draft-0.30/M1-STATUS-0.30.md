# M1 Draft 0.30: Status

**The author ran nothing. M1 is not complete.** The statuses below are expectations. `tools/run_checks_030.py` computes them and fails `consolidation.rowsMatchProposal` if they differ.

| Rows | Expected | Remaining |
|---|---|---|
| 34 earlier candidates plus C1, C2, R1, E6 (38) | CompleteCandidate | root's ledger action |
| C3, C4 | PendingRootReview | root review of C34 (`U08.*`, `C34.*`, `decision.U08.applied`) and the shared RP budget (`U14.rp.*`) |
| V1 | PendingRootReview | root review of C33 (`C33.*`), the shared RP budget and `freeze.v1` |

## Exact remaining root work

1. **Run the suite.** Execute `run_checks_030.py` in a preserved copy and require zero FAIL.
2. **Check the repairs independently.** C33 and C34 stay open until root does. Suggested items:
   - the C33 cases, especially the three original probes and the byte and depth boundaries;
   - the C34 mutants;
   - the shared-budget RP-COMB result;
   - a sample of `hash-freeze-0.30.json`.
3. **Record final verification in the ledger** for C06, C07, U08-CR, RF-E6-1, V1-SHA-COST, P-C32-2, C33 and C34, and for findings F01–F26.
4. **Keep the gate unmet** (F26/C01) until all 41 rows are Complete. CompleteCandidate is not Complete.
5. **Phase A stays deferred.** X-C33, X-U14, E01–E04, E06 and E07 are later implementation results, not gate conditions. No experiment definition is missing.

## Preserved failures, now rebound

| ID | Kept in | Now covered by |
|---|---|---|
| HF-8 | four 0.29 FAILs | `U08.noncanonical.N05`, `decision.U08.applied`, `consolidation.findingsMatchProposal`, `coverage030.required` |
| HF-9 | root guard probes 0/3 | `C33.case.G-P1/G-P2/G-P3` |
| HF-10 | root's 611 null-reference harness failures | the 668/668 executed triad and `freeze.legacy` |
| HF-11 | two envelope-scope observations | `C33.case.G-DUP-errorCode`, `C33.case.G-SCOPE-busyWrongId` |
| HF-1 … HF-7, DC-1, DC-2 | earlier failures | bound through root's 0.29 `history.*` results |

If the run shows otherwise, the missing artifact is named in `coverage030.required`, `freeze.*` or `experiments.definitionsComplete`.
