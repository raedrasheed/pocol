# M1 Draft 0.6 — Dispositions and Coverage

Read with `../m1-draft-0.5/M1-STATUS-0.5.md`, which stays in force where not amended here. The ledger is not edited. No issue is closed by this document. **Nothing in 0.6 was executed by the author.**

## 1. Dispositions

| Issue | Revision | Change | Evidence (unexecuted by the author) | Proposed status |
|---|---|---|---|---|
| C13 | R6-04 | Originals restored in English; ADD-M1-01 withdrawn; reconciliation I1–I8 | `annex/X3-RESTORED.md`, `x3-restored.json`; `vectors/profile-race-cases.json` (16 scenarios); `vectors/dnr-cases.json` (6) | Drafted + fixtures. BLK-02 is resolved as a source question; it is no longer a missing-definition blocker |
| C14 | R6-01 | Corrected 24-byte head | `vectors/c14-head.json` | Drafted + check |
| C19 | R6-02 | Value semantics applied consistently to the bridge, the proof envelope, the error code and the retry delay | `vectors/id-integer-cases.json` (22 tokens, 8 envelope cases); Node oracle | Drafted + fixtures |
| C08 | R6-07 | Closed by provenance (per review) | `coordination/review-001/source-provenance-verified.json` | Closed (ledger owner) |

## 2. Coverage

| Row | 0.5 | 0.6 |
|---|---|---|
| X2 | Not started | Partial: signEligible model and 16 race scenarios. Browser run is Phase A |
| X3 | Blocked | Partial: restored definitions with literal fixtures and models. I1/I6 proposals need review |
| B10 | Not started | Partial: BR19a and BR19b executable; BR19c script plus synthetic BridgeRef trace |
| B11 | Not started | Partial: BR16 generator in two languages, Refs, BridgeTrace format |

Totals:

| Status | Count | Rows |
|---|---|---|
| Partial | 20 | C1–C4, X1, X2, X3, V1, B1–B11, R1 |
| Blocked | 0 | — |
| Not started | 21 | R2–R4, S1–S4, Q1–Q4, E1–E7, V2–V4 |
| Complete | 0 | — |

The next feasible batch is R2–R4 (D98: MR scripts, read steps and pools), then S1–S4.

## 3. Remaining blockers and real decisions

| ID | Exact source | Decider | Status |
|---|---|---|---|
| BLK-01 / CR-M1-01 | FD:L1274 with the table FD:L1262–1273; proof format premise PR-1/PR-2 (BLK-03); `STATE_POOL`; retry caps | Owner | Open |
| U01 | Full factory address (FD:L777 elided) | Owner | Open |
| U02 | Explicit view of a revoked version (FD:L914) | Owner | Open |
| U10 | Publisher rights of the previous owner after a transfer | Owner | Open |
| U14 | Meaning of "timeout 10s" (FD:L905) | Owner | Open |

**Reviewer-level items, not owner questions:**
- the I1 and I6 proposals;
- U42 (epoch re-stamp);
- the P details of BR16, BR19 and BridgeTrace;
- TK-1 and the txA tuple (independently verified);
- CONF_DEPTH, which stays a required runtime input with no default.

## 4. Not executed

These are outside the reference tooling:
- the browser and the MV3 extension (X1–X7, the د1–د5 runs in Chrome, the Q8–Q12 runs with two anvil instances);
- Chrome DNR itself (only modelled here);
- anvil and pocold;
- MPT proofs;
- ESLint and dependency-cruiser;
- any transaction.

The decision models for Q, د, BR19 and BR16 are reference models of the text. They are not evidence about the extension.
