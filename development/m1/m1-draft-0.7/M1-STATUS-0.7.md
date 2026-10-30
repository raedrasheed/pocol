# M1 Draft 0.7 — Dispositions and Coverage

Read with `../m1-draft-0.6/M1-STATUS-0.6.md`, which stays in force where not amended here. The ledger is not edited. **Nothing in 0.7 was executed by the author.**

## 1. Dispositions

| Item | Revision | Status proposed |
|---|---|---|
| BR19 checker collision | R7-01 | Fixed in the checker; to be confirmed by the run |
| C20 | R7-02 | Instrumentation and cache fixed; regression over all 0.2 fixtures; one isolation claim corrected |
| C21 | R7-03 | U42 withdrawn (reviewer decision to preserve the baseline); pure predicate; fixtures |
| R2–R4 | R7-04 | Drafted + model fixtures |
| S1–S4 | R7-05 | Drafted + model fixtures |
| C06 (alternative E) | R7-06 | Evaluated: not equivalent in RP; CR-M1-01 still pending |
| B7 fetch rule | R7-07 | Criterion closes the alias bypass; CI verdicts are future |

## 2. Coverage

| Status | Count | Rows |
|---|---|---|
| Partial | 27 | C1–C4, X1–X3, V1, B1–B11, R1–R4, S1–S4 |
| Blocked | 0 | — |
| Not started | 14 | Q1–Q4 (D101), E1–E7 (D103/D104), V2–V4 |
| Complete | 0 | — |

"Partial" means drafted with model fixtures, awaiting review, execution and (where stated) future browser, socket or node tests.

Next feasible batches: Q1–Q4 (StoreQueue, D101), then E1–E3 (D103/D134).

## 3. Evidence split

- **Computed model evidence** (this runner, once executed): C20, C21, BR19a, MR1–MR6d, MR8, MR9, MR11, MR12, MR-neg, MR10a/b/c/f, BR20a/b, BR20c (bounds on a model run), BR20d, alternative E, and the structure of the fetch criterion.
- **Future tests, not executed:**
  - the MaliciousHttp server over real sockets, the Chrome stream reader and TestHooks;
  - MR7 timing; CAP1; MR10d memory;
  - BR20c/BR20d in Chrome with the C measurements;
  - DNR; the MV3 extension;
  - anvil, pocold and the `eth_call` getter; MPT;
  - ESLint, dependency-cruiser and the C2/C3 runtime tests.

## 4. Pending decisions (unchanged; requests, not approvals)

- **CR-M1-01:** proof format premise, `STATE_POOL`, retry caps. Alternative E is noted as an LN/DEV-only option inside it.
- **U01, U02, U10, U14.**

Reviewer-level items:
- U43 (the stale-request code);
- the P details of the scopes (WalletSubmit pool, content not session-counted), strict FIFO, the MaliciousHttp wire format, and the BR20c model parameters;
- the getter ABI (only if E is adopted for LN/DEV).

CONF_DEPTH stays a required runtime input. The full gate stays in force.
