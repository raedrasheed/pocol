# Separate R4 review pass

After implementing the positive path, review traced each failure through the
candidate, USER receipts/tips, mint, compiled SYSTEM call and final commitment.

1. Found missing global capacity verification after SYSTEM closeBlock storage
   writes. Added the post-call manifest/cap/supply check; an actual SLOT_CAP
   boundary test now rejects and poisons the complete candidate after mint.
2. Found root-computation errors could escape without poisoning. Root mismatch
   and computation failure now share the poison path; late rejection tests retain
   no USER state or issuance.
3. A raw-prefix+1 OOG fixture failed before USER because EIP150 and SSTORE's2300
   gas sentry require allocation headroom. Preserved both failed attempts and
   selected measured prefix+20000 to reach actual USER and fail at closeBlock.
   This is a measured negative fixture, not a general sufficiency bound.
4. Added an actually signed USER closeBlock attack with ORIGIN equal to its sender.
   CALLER remains the USER sender; the receipt fails and pending is absent. The
   subsequent host call succeeds with actual tips from the failed USER execution.
5. Supply controls use root-matching invalid allocations, ensuring R23 and u256
   wrapping mutations cannot be killed merely by an unrelated root mismatch.
6. Confirmed native credit is internal, existing-account-only, checked and after
   USER; final selection is derived from parent Registry facts, not a boolean input.
   Full validated UT/header admission remains absent and explicitly NOT PASSED.

No consensus rule was inferred to fill the absent complete phase/header admission,
settlement or finalization. No accepted decision is reopened. FinalFundingWitness
is never counted as real finalization, absence of pending[F] there is host-path
evidence only, and the deliberately retained earlier pending forbids a SV6 claim.
The generic primitive remains crate-private, without a public canonical commit.
RAM cloning does not establish disk rollback or crash/reorg correctness.

Reviewed code and all affected inputs are bound in source-bindings.json. Compiled
mutation results are separately recorded only after build exit0 and an assertion
failure exit101; a compiler error is never a killed mutation.
