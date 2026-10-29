# M1 Draft 0.10: Status and Coverage

Read with `../m1-draft-0.9/M1-STATUS-0.9.md`, which stays in force where not amended here. The ledger is not edited. **Nothing in 0.10 was executed by the author.**

## 1. Coverage (proposed; not accepted until independent execution and review)

| Status | 0.9 (verified) | 0.10 proposed | Rows |
|---|---|---|---|
| Partial | 31 | **34** | C1–C4, X1–X3, V1, B1–B11, R1–R4, S1–S4, Q1–Q4, **E1–E3** |
| Blocked | 0 | 0 | — |
| Not started | 10 | **7** | E4–E7 (D104–D114), V2–V4 |
| Complete | 0 | 0 | — |

**E1–E3 stay Partial** even if the run is clean. They await:
- independent execution and adversarial review;
- the TypeScript StoreQueue and StoreRecovery reproducing the goldens (BR22a, in Node over FakeBackend);
- E4–E7, on which criterion (7) disk and the D112 adapter depend;
- BR22c in Chrome, which measures A15c/A15d.

**Full M1 remains incomplete.**

## 2. Evidence split

- **Model evidence (this runner, once executed):**
  - E1 units;
  - BR22h h1–h6, including the 2 × 729 h5 runs;
  - BR22b; the retry and sweep vectors;
  - BR22a, all combinations, with the distinct-schedule count recorded;
  - all controls; provenance.
- **Not executed, future:**
  - BR22c (CDP);
  - any Chrome storage behaviour, including A15c/A15d;
  - the fmt-2 codec and the D104 bounds;
  - the TypeScript implementation;
  - the browser.
- **Inherited, still unexecuted:** as in 0.9.

## 3. Pending decisions (unchanged)

- **U01, U02, U10, U14, CR-M1-01:** unapproved.
- CONF_DEPTH stays required.
- The full gate stays.
- No merge or deploy.
- The 0.10 P items and RF-1 to RF-6 are reviewer-level, not owner decisions.

## 4. Next batch

E4–E7 (RecoveryGate, SeqAlloc, fmt 2, DiskLedger, slots, AdminDelete, sites and tombs), then V2–V4.
