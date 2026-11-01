# M1 Draft 0.26: Acceptance report

**Nothing in 0.26 was executed by the author.** Every value below is what `tools/run_checks_026.py` must compute on root's run. The check `consolidation.rowsMatchProposal` fails if the computation differs.

## Rows (R3-08 c1–c5, recomputed with the recorded decisions)

| Rows | c1 | c2 | c3 | c4 | c5 | Status |
|---|---|---|---|---|---|---|
| X1–X3, B1–B11, R2–R4, S1–S4, Q1–Q4, E1–E5, E7, V2–V4 (34) | S | S | S | S | S | **CompleteCandidate** |
| C1, C2 | S | S | S | B (U01) | B (U01; the hashes depend on U01) | Partial |
| C3 | S | S | S | B (U01, U02, U10, U14, CR-M1-01, U08-CR) | B | Partial |
| C4 | S | S | S | B (U01, U10, U08-CR) | B | Partial |
| R1 | S | S | S | B (CR-M1-01) | B | Partial |
| E6 | S | S | S | B (RF-E6-1, pending owner question) | B | Partial |
| V1 | P | P | P | P (P-V1-1…10) | P (the V1 hashes await independent verification) | Partial |

Legend:
- **S**: satisfied.
- **B**: blocked.
- **P**: pending root review or decision.

**CompleteCandidate.** All five criteria are satisfied from reviewed evidence and recorded decisions. Such a row becomes Complete only when root confirms this 0.26 run. Root review of the consolidation itself is still pending.

## Exact remaining blockers

1. **Owner (unanswered):**
   - U01, U02, U10, U14 and CR-M1-01;
   - RF-E6-1 (the owner question is pending; not decided here);
   - the U08 change request.
2. **Root, for V1:**
   - review of the new V1 material;
   - decisions on P-V1-1…10;
   - independent verification of `hash-freeze-v1-0.26.json`.
3. **Root, for the consolidation:** confirmation of this run. That turns the 34 CompleteCandidate rows into Complete.
4. **F01–F26:**
   - 18 are closable at spec scope once root records them in the ledger: F01, F02, F03, F05, F08, F10, F12–F16, F18–F22, F24, F26.
   - 8 stay open on owner items: F04 (U02), F06 and F23 (U01), F07 (CR-M1-01), F09 (U08-CR), F11 (U10), F17 (U10, U08-CR), F25 (U14).
5. **Full gate:** C01 stays unmet while any row is not Complete.

## Not blockers

- **Phase A experiments:**
  - E01–E04, E06, E07;
  - S26-NXV3E-PING, S26-NX-D, S26-NXV4-LIVE;
  - BR22c, X8, RG3/RG3b.
- **A production M3-TS model.**
- **U07:** accepted as a conditional E02 procedure, not as an outcome.
