# M1 Draft 0.29: Status

**The author ran nothing. M1 is not complete.**

- **CompleteCandidate is not Complete.** A row is Complete only after root executes `tools/run_checks_029.py` in a preserved copy, reviews the result and records the row in the ledger.
- **What is expected below.** The statuses are the author's expectations. The runner computes them and fails `consolidation.rowsMatchProposal` if they differ.

## Rows (41)

| Rows | Expected status | What still has to happen |
|---|---|---|
| X1–X3, B1–B11, R2–R4, S1–S4, Q1–Q4, E1–E5, E7, V2–V4 (34) | CompleteCandidate (unchanged since 0.27/0.28) | root's ledger action |
| C1, C2 | PendingRootReview | root review of the U01 overlay and the `U01.*` checks |
| R1 | PendingRootReview | root review of the CR-M1-01 deadline amendment (`CRM101.*`) |
| E6 | PendingRootReview | root review of the RF-E6-1 binding (`RFE61.*`) |
| C3 | Partial | root review, plus root decisions on P-D29-1, P-D29-2, P-D29-3, P-U02-1, P-U02-2 and P-U10-1 |
| C4 | Partial | root review, plus root decisions on P-U08-1 and P-U10-1 |
| V1 | Partial | root review, plus root decisions on P-V1-1, P-V1-2, P-V1-4 … P-V1-10, P-D29-1 and P-D29-3 |

Because of the delegation, no row is blocked by an owner answer any more. The saved owner questions remain unanswered, and nothing here answers them.

## Exact remaining criteria

1. **Root review (c3) of the 0.29 overlay** for C1, C2, C3, C4, R1, E6 and V1. This needs:
   - a preserved-copy run of `run_checks_029.py` with zero FAIL;
   - root's independent check of the new fixtures. Suggested items:
     - the deadline arithmetic of H-01…H-24, CL-01…CL-16, BUD and RP-COMB;
     - the U08 boundaries;
     - the U02 and U10 traces;
     - the SHA-W preimage binding.
2. **Root decisions (c4)** on the 16 conventions listed in M1-SPEC-0.29-OVERLAY.md §3:
   - P-V1-1, P-V1-2, P-V1-4, P-V1-5, P-V1-6, P-V1-7, P-V1-8, P-V1-9, P-V1-10;
   - P-D29-1, P-D29-2, P-D29-3;
   - P-U08-1;
   - P-U02-1, P-U02-2;
   - P-U10-1.

   No group approval is assumed.
3. **Ledger records**, made only by root after the evidence:
   - `delegatedDecisions.status`;
   - C06, C07, U08-CR, RF-E6-1, V1-SHA-COST and P-C32-2;
   - findings F04, F06, F07, F09, F11, F17, F23 and F25.
4. **The gate itself (F26 / C01)** stays unmet until all 41 rows are Complete. Agreement is not proof, and there is no cap on cycles.

## Missing artifacts

- **Experiment definitions.** None is missing. E01, E03 and E07 are amended, and X-U14 is new.
- **Phase A runs.** EVM, Chrome, M3-TS, node and real-clock deadline runs are implementation results, not M1 specification gate conditions.

If the run shows otherwise, the missing artifact or test is named in `experiments.definitionsComplete` or `coverage029.required`.

## Historical failures (kept, rebound)

| ID | Saved FAIL | Repaired by |
|---|---|---|
| HF-1, HF-2 | C14 request-bound cells (0.4, 0.5) | 0.6 `c14.*` |
| HF-3 | `br19a.a7` (0.6) | 0.8 `br19a.*` |
| HF-4 | C27 final cells of BR24-x9 (0.16) | 0.17 C27 inside 0.18 |
| HF-5 | `reuse.keccakFrom0.2` (0.21 preserved copy) | 0.22 |
| HF-6 | 0.25 output-path failure | canonical 0.25 rerun |
| HF-7 | three malformed −32021 crashes (root 0.26 probes) | 0.27 C32, with 26 independent probes |

Two document corrections are also recorded:
- the 0.27 overclaims, corrected in 0.28;
- P-V1-3, withdrawn.
