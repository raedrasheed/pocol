# M1 Draft 0.12: Status

Read with `../m1-draft-0.11/M1-STATUS-0.11.md` and `../m1-draft-0.10/M1-STATUS-0.10.md`. **Nothing in 0.12 was executed by the author.**

## Coverage

- **Accepted:** unchanged at the 0.9 scope (31 Partial, 10 Not started, 0 Complete) until the root review.
- **Proposed after a successful root review of C25:** E1–E3 Partial. That gives **34 Partial, 7 Not started (E4–E7, V2–V4), 0 Complete**.
- **Full M1 remains incomplete.**

## Preserved evidence

All of these remain as they are:
- the 0.10 author run;
- the C24 independent probes (7 FAIL on 0.10);
- the 0.11 run (211 pass, 48 recorded, 0 fail);
- the C25 namespace probe (FAIL on 0.11).

0.12 reproduces C25 on an unpatched 0.11 instead of overwriting it.

## Not executed / open

- **Not executed:** BR22c (Chrome + CDP), the A15c/A15d measurements, any TypeScript implementation, `chrome.storage` behaviour, the fmt-2 codec (E4), the full D104 bounds.
- **Owner decisions:** U01, U02, U10, U14 and CR-M1-01 remain unapproved.
- **Unchanged:** CONF_DEPTH required; full gate; no merge or deploy.
- **Reviewer-level items still open:**
  - P-C24-2/3/4 (integral floats rejected; invalid bootMs gives no window time; the bootMs range);
  - the 0.10 P items and RF-1 to RF-6.
- **Closed if the review agrees:** RF-9 (malformed names under the prefix), now counted physically.
- **Cleanup policy:** no automatic cleanup of malformed `pocol:epoch:*` items exists. A namespace held full only by malformed items stays disabled (`epochNames`, action 25) until some cleanup is approved. That is fail-closed, by design.

## Next

After the root review: E4–E7, then V2–V4.
