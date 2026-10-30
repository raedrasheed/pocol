# M1 Draft 0.13: Status and Coverage

Read with `../m1-draft-0.12/M1-STATUS-0.12.md`. **Nothing in 0.13 was executed by the author.**

## Coverage

| Status | Accepted (0.12 tooling) | 0.13 proposed | Rows |
|---|---|---|---|
| Partial | 34 | **36** | … E1–E3, **E4–E5** |
| Not started | 7 | **5** | E6–E7, V2–V4 |
| Complete | 0 | 0 | — |

**E4 and E5 stay Partial.** They await:
- the root's independent execution and review;
- the TypeScript implementation passing the same tables;
- BR22c in Chrome;
- E6/E7 for the AdminDelete, tomb and sites parts;
- decisions on CR-E4-01 (u64 representation) and CR-E4-02 (unpaired surrogates).

**Full M1 remains incomplete.**

## Evidence split

- **Model evidence (this runner, once executed):**
  - E4 codec and DiskLedger units;
  - all E5 tables and controls;
  - the full 0.12/0.11/0.10 regression re-run.
- **Not executed:**
  - Chrome, CDP or `chrome.storage` behaviour, including `getKeys`/`getBytesInUse`;
  - JS Number handling of u64 fields;
  - TextEncoder/TextDecoder behaviour (only the root's Node observation is cited);
  - TypeScript;
  - E6/E7 machines.
- **All byte figures are model metrics, not Chrome bytes.**

## Pending decisions (unchanged)

- **U01, U02, U10, U14, CR-M1-01:** open.
- CONF_DEPTH required; full gate; no merge or deploy.
- CR-E4-01 and CR-E4-02 are proposals only.
