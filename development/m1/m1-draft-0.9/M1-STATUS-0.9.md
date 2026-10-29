# M1 Draft 0.9: Status and Coverage

Read with `../m1-draft-0.7/M1-STATUS-0.7.md` and `../m1-draft-0.8/M1-SPEC-0.8-AMENDMENT.md`, which stay in force where not amended here. The ledger is not edited. **Nothing in 0.9 was executed by the author.**

## 1. Coverage (proposed; not accepted until independent execution and review)

| Status | 0.8 | 0.9 proposed | Rows |
|---|---|---|---|
| Partial | 27 | **31** | C1–C4, X1–X3, V1, B1–B11, R1–R4, S1–S4, **Q1–Q4** |
| Blocked | 0 | 0 | — |
| Not started | 14 | **10** | E1–E7 (D103/D104), V2–V4 |
| Complete | 0 | 0 | — |

**Q1–Q4 stay Partial** even if the run is clean. "Partial" means the rows are drafted with model fixtures and await:
- independent execution and review;
- the TypeScript StoreQueue and SiteStorage passing the same goldens in Node (M2);
- BR21c and BR21d in Chrome.

**Full M1 remains incomplete.**

## 2. Evidence split

- **Computed model evidence (this runner, once executed):**
  - UTF-8 and `qbytes` vectors; admission; BR17e and UTF-8 quota;
  - BG1 configuration;
  - BR21a q1–q10, BR21b r1–r6 (both variants), BR21e, BR21f/f2;
  - the authored Q1/Q2 cases;
  - the state-table conformance; the eight fault controls; provenance.
- **Not executed, future:**
  - BR21c (Chrome on anvil-br, with the C measurements and `chrome.storage.local.set` atomicity);
  - BR21d (the real loader);
  - the TypeScript implementation;
  - the browser and MV3 extension, and DNR.
- **Inherited from 0.7/0.8, still unexecuted:**
  - MaliciousHttp over sockets, and TestHooks;
  - anvil, pocold, the `eth_call` getter and MPT;
  - ESLint and dependency-cruiser;
  - any real transaction.

This runner does not re-run the 0.8 checks. Those keep their own recorded result (261 pass, 3 recorded, 0 fail).

## 3. Pending decisions (unchanged; requests, not approvals)

- **CR-M1-01**, **U01**, **U02**, **U10**, **U14**: unapproved.
- CONF_DEPTH stays a required runtime input.
- The full M1 gate stays in force.
- The 0.9 P items (`annex/Q4-REFERENCE-EXTENSION.md` §5) are reviewer-level readings, not owner decisions.

## 4. Next feasible batch

E1–E3 (D103/D134 generation recovery), which interact with StoreQueue through RecoveryGate and ReadSlots.
