# Separate sequential review (same author)

Scope: sandbox source, actual results, restricted oracle, mutation runner, provenance,
PAY proposal and preservation against foundation c0ef36c68fb83f9474de7e70fe61973b466647a8.
No second human/agent review is claimed. No Claude/coordinator was used.

Findings and corrections before publication:
1. A reverted revm journal can retain a loaded storage slot at its original zero
   value. The original test incorrectly required an absent cache entry. Corrected
   to assert all returned slot values remain zero; snapshot remains unchanged.
2. Added exact SSTORE gas equality/minus-one and STATICCALL/DELEGATECALL PAY guards.
3. Initial wrong-chain mutant died at unwrap, not a meaningful assertion. Rejected
   that control and changed the valid-chain test to assert acceptance explicitly.
4. Shared package identities in the mutation target cache contaminated a later
   unmodified example (41012 instead of 43112 gas). The independent oracle caught
   this. Rejected pre-isolation mutation/aggregate results. Mutation crates now
   have distinct manifest AND lockfile package names. Cleaned original package,
   reran mutants, then reran original 8 tests and example/oracle successfully.
5. Live ledger text still said named definitions pending after their approval;
   updated that wording only. Every obligation remains NOT PASSED with no evidence
   claiming gate acceptance. Historical LP3/P0/P1 evidence is untouched.
6. PAY ABI/source authority/call modes are unspecified in preserved M1; the proposal
   is explicitly unapproved. Experimental guard is not installed in consensus.

Limits: 3 restricted oracle executions are not independent full-EVM acceptance.
3 compiled controls test only this scaffold, not all required PoCol mutations.
Full GST, full root differential, licenses audit, legacy regression, PAY and later
LP4 increments are NOT RUN/NOT PASSED. rustfmt is unavailable in installed toolchain;
formatting was not run/passed. Dependencies were fetched with inherited proxy;
initial vendor/PATH failures are preserved, never counted as test passes.
