# M1 Draft 0.27: Status and exact blockers

**Nothing was executed by the author.** These are the values `run_checks_027.py` must compute. `consolidation.rowsMatchProposal` and `consolidation.carriedFrom0_26` fail on any difference.

| Rows | Status | Blocked by |
|---|---|---|
| X1–X3, B1–B11, R2–R4, S1–S4, Q1–Q4, E1–E5, E7, V2–V4 (34) | CompleteCandidate (evidence confirmed in REVIEW-0.26 at specification scope) | root's ledger action only |
| C1, C2 | Partial | owner U01 |
| C3 | Partial | owner U01, U02, U10, U14, CR-M1-01, U08 |
| C4 | Partial | owner U01, U10, U08 |
| R1 | Partial | owner CR-M1-01 |
| E6 | Partial | owner RF-E6-1 (pending question) |
| V1 | Partial | owner **V1-SHA-COST** (source «≤ 13 SHA-256» cannot be met); root review of C32 and the SHA accounting; root decisions on P-V1 (without the withdrawn P-V1-3) and P-C32-1…4 |

## Fixed now (reference and instrumentation, pending root verification)

- **C32:** no input raises or waits without bound.
- **Accounting:** every SHA-256 is counted, including shares.

## Not changed (normative, not approved)

- The source bound.
- Which checks run.
- Any owner branch.

## Unanswered

U01, U02, U10, U14, CR-M1-01, U08, RF-E6-1 and V1-SHA-COST. Full M1 is unmet.
