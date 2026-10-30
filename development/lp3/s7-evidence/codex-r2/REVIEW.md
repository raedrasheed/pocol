# Separate sequential S7 core review

Single author, separate post-implementation review; no independent reviewer identity
is claimed. Reviewed against preserved consensus §1/§8 and the owner's new CF
mapping. M1, accepted S0–S6 evidence, RP parser/priority and ASERT arithmetic are
unchanged. No signature generation or private keys are introduced.

Findings closed: initial shell had no cargo on PATH (NOT RUN, never counted as a
pass); explicitly used the retained pinned toolchain. Maximum-width version test
initially exceeded the header cap (failed); corrected to supported v1, retained
failure. Initial adapter import path corrected before oracle execution.

Boundary review: widened addition/multiplication; RET=0 rejected; subtraction
underflow rejected rather than saturated into acceptance; u64 cursor overflow
rejected; final closeH+D tested beyond u64; each final conjunct independently
falsified. Empty cursor convention documented. Parent booleans are inputs to pure
predicates, never accepted as evidence of actual execution or boot authorization.

T0 review: decode rejects shape malleability before net/Rule3 checks; template
recovery hashes exact decoded UT, not stale mutated typed input. New entry point is
not connected to production boot. Full rule diagnostics explicitly mark unavailable
parent/state checks. No blanket full validity or original literal V1–V26 pass.

Actual test/independent oracle/mutation receipts are in RESULTS.json and logs;
mutation compilation/import failures do not count as detected assertion failures.
