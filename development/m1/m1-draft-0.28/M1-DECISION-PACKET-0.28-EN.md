# M1 owner decision packet (draft 0.28)

**Status:** decision requests, not decisions. No owner-controlled option has been adopted; M1 is not complete.

## New: retry wait for a “rate” response (P-C32-2)

**Problem.** The source bounds a “busy” wait to 250–2000 milliseconds but specifies no duration for “rate”. An RP server can therefore request an unbounded wait unless another constraint applies.

**Options:**

| Option | Rule | Effect |
|---|---|---|
| A | Accept 0–2000 milliseconds; other values immediately stop display | Total wait is at most 6 seconds. Servers requesting more than two seconds lose display |
| B | Owner specifies the server format in the source (for example 50 or 1000 milliseconds), and the client applies it | Tighter bound, but the server-side source must also change |
| C | No bound | With per-request timeouts, an attacker can stall display for a long time. With a whole-load timeout, it stops after 10 seconds |
| D | Same range as “busy”: 250–2000 | Rejects honest servers requesting 50 milliseconds |
| E | Do not retry “rate” | Conflicts with the source rule retrying every −32021 |

**Recommendation:** A as an explicit policy, with an explicit answer to U14.

**Note.** The 6000-millisecond bound covers waiting only. It excludes response and computation time and does not establish a ten-second timeout.

## U14: meaning of the ten-second timeout

The choice still belongs to the owner:
- **Per request:** the entire load may exceed 10 seconds and succeed. Example: 1.6-second responses with three retries finish at 14.05 seconds.
- **Entire load:** the same example stops at the tenth second and display is rejected.

Examples in the file are conditional, not measurements.

## Existing decisions, unchanged

| ID | Required decision | Recommendation |
|---|---|---|
| V1-SHA-COST | Literal “13 SHA-256 operations” bound is unattainable | A: component bounds totaling at most 3355, retaining all checks |
| U01 | Factory address | Confirm |
| U02 | Explicit revoked-version viewing | Warning page then click |
| U10 | Previous owner's publisher permission | Retain with warning |
| U14 | Ten-second timeout: per request or entire load | Per request, following the 0.2 recommendation |
| CR-M1-01 | Proof-backed state reads | Adopt with E03 |
| U08 | Manifest chunking into 1–3 chunks | A, with D as alternative |
| RF-E6-1 | Literal x12b total | Open question; not decided by this packet |

Seven rows remain pending on owner decisions.
