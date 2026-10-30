# M1 Spec 0.28 Amendment: retry-policy and status corrections

**For root review. Not approved. Nothing was executed by the author.** This is a narrow policy and status repair. No reference module, fixture builder or source text changes. The 0.27 tools and vectors are reused byte-identically.

## Basis

REVIEW-0.27:
- **C32 crashes:** closed at reference scope.
- **P-C32-1, -3, -4:** accepted.
- **P-C32-2:** an unapproved owner client policy.
- **6000 ms:** sleep-only.
- **U14:** unanswered.
- **V1-SHA-COST:** routed to the owner. A is recommended; the decision is pending.

## Changes

1. **`annex/C32-POLICY-CORRECTIONS.md` (normative text).**
   - Supersedes six 0.27 sentences.
   - States that the rate cap is a proposal.
   - Withdraws the claim that honest servers always comply for rate; gives the source facts and the conditional formulas F1–F4.
   - Restricts 6000 ms to retry sleep under the proposed cap.
   - Defines U14 branches PR, WL-a and WL-b, with literal conditional counterexamples.
2. **`owner/P-C32-2-CHANGE-REQUEST.md`.** Alternatives A–E, with observable consequences and their relationship to U14. A is recommended, together with an explicit U14 answer.
3. **`vectors/u14-timing-cases.json`.** Seven conditional timelines with expected outcomes per branch. They are hypothetical inputs, not measurements.
4. **`audit/carry-forward-0.28.json`, `tools/run_checks_028.py`.** These bind:
   - the saved 0.27 results (320/248/72/0);
   - root's independent evidence (26/26, 18/18, 3677/3677, 31/31, 1027/1027);
   - the reused files, byte-identical against the review-copy manifest;
   - the unchanged source;
   - the ledger statuses;
   - the routing.

   The runner evaluates the timing arithmetic and does not rebuild any cryptographic fixture.

## Unchanged

- The literal «≤ 13 SHA-256» stays unresolved.
- P-V1-3 stays withdrawn.
- The full-window SHA-256 counts stay as verified: 3355 cached, 6721 uncached.
- No owner choice is adopted.
