# M1 owner decision packet (draft 0.27)

**Status:** decision requests, not decisions. No owner-controlled option has been adopted.

## New: SHA-256 bound in RP verification (V1-SHA-COST)

**Problem.** browser.md:58 says verification uses at most 13 SHA-256 operations. Checks required by that same source need substantially more:
- 27 operations for a 13-header window without shares;
- 3355 operations for a full window with 256 shares in each header.

Caching cannot reduce either count because every input is distinct.

**Options:**

| Option | Change | Effect |
|---|---|---|
| A | State component bounds: 14 template IDs, 13 proof-of-work hashes and 256 share hashes per header, at most 3355 overall | All checks remain; the bound becomes correct and testable |
| B | Interpret 13 as proof-of-work operations only | Checks remain, but other costs have no written bound |
| C | Do not check shares in RP | Bound becomes 27, but headers with invalid shares can be displayed |
| D | Reject headers exceeding a chosen share count | Narrows the honest blocks that can be displayed |
| E | Retain literal bound 13 | Impossible for a 13-header window; V1 remains blocked |

**Recommendation:** A.

## Still outstanding, unchanged

| ID | Required decision | Recommendation |
|---|---|---|
| U01 | Factory address | Confirm |
| U02 | Explicit revoked-version viewing | Warning page then click |
| U10 | Previous owner's publisher permission | Retain with warning |
| U14 | Meaning of ten-second timeout | Per request |
| CR-M1-01 | Proof-backed state reads | Adopt with E03 |
| U08 | Canonical manifest chunking into 1–3 chunks | A, with D as alternative |
| RF-E6-1 | Literal x12b total | Open question; not decided by this packet |

## Repairs not requiring the owner

- **C32:** a malformed −32021 response now gives viewIncomplete without an exception or long wait. The adopted retry rule is unchanged.
- **SHA-256 accounting:** all operations are now counted.

These reference-tool repairs await reviewer review. V1-SHA-COST is the only new owner decision.
