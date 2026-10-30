# Separate sequential review

Same author, separate pass after tests; no other agent, Claude or coordinator invoked.
- The baseline includes every20661 case. Preserve85 mismatches and33 timeouts. An
  arbitrary Python exception is EXCEPTION_PRESENCE_ONLY, never semantic PASS.
- No fixture secretKey crosses the final worker boundary; output contains only case
  identities, statuses, root/log hashes, exception class or time budget. No key used.
- Pending futures bounded to2/worker, workers<=4; per-case SIGALRM always disarmed.
  Timeout/error is observable; selected case identities must all exist in frozen input.
- Comparison VM copies the precompile map; it cannot modify unmodified CancunVM.
  Key union includes actual initial and dynamic writes; current-journal reads handle
  clears/deletes/reverts. No caller-provided storage-success flag or hidden DB assumed.
  It is fixture-only, not general state, bounded resource proof or production backend.
- MODEXP correction runs AFTER original gas charging; empty exponent, encoded zero,
  positive exponent, modulus0/1, ordinary nonzero base covered with actual computation.
- Rejection predicates use independent Python integers and exact raw fee/balance
  fields. Unknown reason/label rejects. Decimal/hex inputs and frozen bigint adapter
  notation remain distinct; no claim that unsigned format checks validate RLP/signatures.
- Both Rust controls compiled; native programs emitted results rejected by meaningful
  oracle assertions. No compiler failure credited. Portable wrapper repeated successfully.
- Workflow preserves existing jobs and requires a complete independent comparison;
  any timeout/mismatch/engine error causes failure. New-head checks are not assumed.
- 73 unsigned/engine admission differences and6 remaining budgets are still open.
  The12 and27 selected matches do not equal a full patched-suite pass or T2 acceptance.
  Full provenance, actual GenesisBuilder/systems/apply/settlement/finalization, general
  R18/R21/caps64 and persistence/crash/reorg remain mandatory; ledger unchanged.

Publication review: two archive uploads were rejected by Auto-review; subsequent
semantic field/identity inspection did not clear the restriction. User instructed no
repeat/splitting. Complete local archive verified54 members and both file hashes;
publication tree omits those two payloads and explicitly preserves this evidence gap.
No new raw-independent-result CI export substitutes for the rejected publication.
