# Separate sequential source and evidence review

Same author, separate pass after initial compiled Rust and py-evm comparisons.
No external review or complete Registry acceptance is claimed.

1. The previously proposed contract simultaneously required slot3 consumption and
   original write ceilings. That was a real review omission. The new approval is
   recorded in full; part8 requires returning this contradiction to the owner.
2. First py-evm fixture setup was not persisted, so EIP-2200 treated original storage
   incorrectly and gas differed by2800. Initial failed log is preserved and was not
   a passing comparison. Persisting setup before the transaction fixed gas; actual
   storage-key counts agreed already. Final execution compares keys/values and gas.
3. Initial lower-bound fixtures used an empty register state and two exited records.
   They were not represented as accepted network states. Review strengthened them:
   register preserves an existing active member; middle claim preserves M_min1 with
   a separate active member. Final bond is governance's10^20, all reward identities
   have pre-transaction nonce1. Packing is witness-only, not approved full layout.
4. Count distinct actually changed storage keys, not caller-supplied success/count
   flags. Rust checks journal changes; separate py-evm instruments actual SSTORE
   opcodes and independently computes the mandatory key identities. Full parameter/
   resource-model binding is still NOT PASSED; these witnesses are not R18 facts.
5. Removing slot3 consumption can lower counts but violates the just-approved rule.
   Three separately Solidity/Rust compiled assertion controls detect those omissions.
   Mutant build failures do not count. Earlier controls remain preserved separately.
6. Native execute defaults are unchanged; explicit experimental NUMBER exists only
   to execute actual h1 witness bytecode. This is not full PoCol apply/system phase
   authorization, admission, genesis, finalization, caps or persisted execution.

No new upper bound or complete runtime contract is accepted. The proposed10/2/10
ceilings require explicit owner amendment and actual full execution/resource proofs.
All original ceilings, unrun/blocked tests and every old LP4 obligation retain their
truthful status. No M1/LP3 or previous evidence change and no boot/merge authorization.

A final-run attempt invoked a direct toolchain cargo with +1.99.1, which failed
before running tests. Its log is preserved; the final retry uses the rustup proxy
at /workspace/pocol-tools/cargo/bin/cargo. No failed/unrun invocation is counted
as success. The corrected run records29 Rust passes,44+2 prior boundary comparisons,
three final budget contradictions and three compiled omission controls.
