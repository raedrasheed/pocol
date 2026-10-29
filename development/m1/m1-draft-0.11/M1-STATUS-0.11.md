# M1 Draft 0.11: Status

Read with `../m1-draft-0.10/M1-STATUS-0.10.md` and `../m1-draft-0.9/M1-STATUS-0.9.md`. **Nothing in 0.11 was executed by the author.**

## Coverage

- **Accepted (until root review of 0.11):** unchanged from the 0.9 scope. 31 Partial, 10 Not started, 0 Complete.
- **Proposed after a successful independent review of this repair:** E1–E3 Partial. That gives 34 Partial, 7 Not started (E4–E7, V2–V4), 0 Complete.
- **Full M1 remains incomplete.**

## Preserved evidence

- The 0.10 author-suite result (86 pass, 27 recorded, 0 fail) and the independent C24 result (24 checks, 7 FAIL) remain as they are.
- 0.11 reproduces the 7 failures on an unpatched copy, as evidence, instead of replacing them.

## Not executed / open

- **Not executed:** BR22c (Chrome + CDP), the A15c/A15d measurements, any TypeScript implementation, `chrome.storage` behaviour, the fmt-2 codec (E4), the D104 bounds (E4/E6).
- **Owner decisions:** U01, U02, U10, U14 and CR-M1-01 remain open.
- **Unchanged:** CONF_DEPTH required; full gate; no merge or deploy.
- **Reviewer-level items:**
  - the 0.10 P items and RF-1 to RF-6;
  - P-C24-2 (integral floats rejected) and P-C24-4 (the bootMs range);
  - RF-8 (invalid bootMs value) and RF-9 (malformed names under the epoch prefix).

## Next

After the root review: E4–E7, then V2–V4.
