# M1 summary — Draft 0.29

**The author ran nothing. M1 is not complete.** Counts below are expected and will be computed by the root when running `dashboard-0.29-en.json`.

## New

This draft applies nine technical decisions made by the coordinator under the user's standing delegation. They are applied in a new specification layer; all earlier files remain unchanged.

- These decisions are **not** personal owner approval.
- Preserved owner questions remain unanswered.

| Decision | Now adopted |
|---|---|
| U01 | Reference factory address 0x0000000000000000000000000000000000c0c005 for M1 tests only; no deployment confirmed |
| U02 | Revoked versions are not displayed by default. An explicit request shows a warning page; loading requires a deliberate click, followed by a persistent “Revoked” banner |
| U10 | The former owner remains a publisher until removed by the new owner, with a clear warning |
| U14 | Ten seconds from the first request through the final verdict, including waits, retries and computation. At 10000 milliseconds the deadline expires |
| CR-M1-01 | Proof-backed site-state reads adopted with their bounds and an amended deadline. The cumulative 20-second wait rule is removed. Node support not yet measured (E03) |
| U08 | Both contract and viewer enforce canonical manifest chunking |
| RF-E6-1 | Effective x12b total is 2: one per operation |
| V1-SHA-COST | Cost: 14 TemplateID, 13 PoW and 3328 share hashes, total 3355, retaining all checks |
| P-C32-2 | “rate” delay 0–2000 milliseconds; “busy” 250–2000; at most three retries |

## Expected row status

- **34 candidate-complete rows.** These rows did not change.
- **4 rows awaiting root review only:** C1, C2, R1 and E6.
- **3 partial rows:** C3, C4 and V1; awaiting root review and decisions on specific conventions.

## Awaiting root decisions (16 conventions)

- **V1 conventions:** P-V1-1, P-V1-2 and P-V1-4 through P-V1-10.
- **Deadline conventions:**
  - P-D29-1: do not wait for a delay that cannot fit before the deadline.
  - P-D29-2: name of the load-timeout outcome.
  - P-D29-3: separate deadlines for header checking/content loading, or one shared deadline?
- **P-U08-1:** position index in ManifestSplit error.
- **P-U02-1 and P-U02-2:** revoked-version approval covers one load; clicking starts a new load with a new deadline.
- **P-U10-1:** timing of the former-publisher warning.

## Notes

- Deadline results here are computed virtual time, not browser measurements.
- Browser measurement is a later experiment (X-U14), not an M1-gate prerequisite.
- No experiment definition is missing.
- Earlier failures are preserved and linked to their repairs.
- No production code, deployment or transactions.
