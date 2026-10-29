# R3 EIP150 and remaining-SYS derivation — general resources NOT PASSED

Approved semantics remain LP4-SYS-01; no issuance timing is selected.
The compiled SystemBoundaryWitness is a measurement fixture, not a system program.
Its runtime/compiler artifacts and independent oracle inputs are hash-bound.

For the nested call requiring100000 at leaf entry, independent py-evm finds the
minimum outer allocation102758. Rust independently binary-searches that same
threshold and compares an exact equality/minus1 fixture plus nested/outer reverts.
At102758 the child observes100000 and writes slot0=9/transient7. The raw outer cost
is only24156. At102757 the frame returns child-success=false, raw3669, slot0=0.
At30M the same leaf observes29530098; raw cost remains24156. This demonstrates
why summing optimistic raw costs does NOT prove that later calls have sufficient
remaining allocation under EIP150. Parent ABI/dispatch/memory/CALL costs are real.

If r_i is actual raw cost and a_i is required allocation for prescribed call i,
full R18 must show, for EVERY supported CP/state/path, a_i<=30000000 and
`CP.G_SYS >= max_i(sum_{k<i}(r_k)+a_i)` with checked sums and all nested headroom,
in addition to overall raw accounting and all existing limits. Refund counters
grant no credit. Bounds must refer to the actual five compiled programs and
complete phase schedule; this witness does not supply their a_i/r_i facts.

The R2 inverse EIP150 formula remains: to forward g>0 after ordinary CALL dynamic
costs needs at least g+floor((g-1)/63); zero needs0. A PAY frame requiring5000 thus
needs at least5079 after CALL overhead, plus actual parent continuation costs.
This arithmetic does not replace actual gas/cap/path proofs. No numeric cap changed.

Current comparisons cover four successful outer messages with exact raw gas,
refund/output/full roots, and an independently observed outer revert matched by
Rust's poisoned candidate rejection. The rejected call's raw23998 is an independent
measurement, not a Rust error-gas comparison (the error API returns no gas report).
Nested rollback/transient reset and late candidate invalidation are tested.
Two successfully built Rust hook mutations fail semantic assertions: excess frame
allocation and committing a reverted nested frame. They do not mutate expected
roots by changing bytecode. Older15 primitive controls are re-executed separately.

General R18, R21 (all host clones/scans/trie/history/sharing), caps64 minimum,
validated UT/parent context, actual programs/GenesisBuilder, full USER+SYS/disk
atomicity, finalization/SV6 and all full acceptance gates remain NOT PASSED.
Issuance host timing/work/counter effects additionally require the separate owner
contract in ISSUANCE-DECISION.md; no diagnostic model is accepted by this report.
