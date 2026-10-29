# M1 Spec 0.27 Amendment

**For root review. Not approved. Nothing was executed by the author.**

## Basis

REVIEW-0.26 returned **revise** for V1. It recorded:
- **C32:** three malformed −32021 replies raised exceptions.
- **V1-SHA-COST:** the literal «≤ 13 SHA-256» is not met (27 for h = 20; 258 for one header with 256 shares; a structural bound of 3355).

The 34 unaffected rows had their evidence confirmed at specification scope.

## Changes

1. **C32, reference repair.**
   - `v1_ref_027.py` parses replies as literal text and validates the envelope and every −32021 reply (P-C32-1…4).
   - Malformed input gives viewIncomplete with no frame, no wait and no exception.
   - Approved semantics are kept: retry after `retryAfterMs`, at most 3 times; total wait ≤ 6000 ms.
   - 41 fixtures: the 3 root failures, 27 near-neighbour malformed inputs, legitimate retries, exact boundaries and exhaustion.
2. **V1-SHA-COST, instrumentation repair.**
   - Every SHA-256 is counted by category: TemplateID, PoW and shares.
   - The counts are given cached, uncached and as unique preimages.
   - Real share fixtures: S1 (256 shares → 258, matching root's probe), S1bad (an invalid share → rule 9), and W (a full window with 256 shares per header → 3355 cached, 6721 uncached, 3355 unique).
   - Caching cannot go below the number of unique preimages.
   - P-V1-3 is withdrawn.
3. **V1-SHA-COST, normative question.**
   - Routed to the owner (`owner/V1-SHA-COST-CHANGE-REQUEST.md`); recommendation A is an itemized bound of ≤ 3355.
   - Not adopted. The source «≤ 13» is unchanged and stays an unresolved discrepancy. Per-case results are recorded, never passed.
4. **Carry-forward.**
   - The 0.26 V1 vectors are re-run under the new checker; the outcomes, counts and rebuilt header bytes must equal 0.26.
   - The 0.26 run, the 0.25 verifications and the reviewer decisions are re-bound.
   - Standalone `--root`/`--out` is kept, along with every previous assertion and the bounded-ASERT check.
