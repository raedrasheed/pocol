# Separate author review pass — SYS-01 R2

This is a separate sequential author review, not an independent human signoff.
The Python EVM/trie comparison is independent execution, not PR approval.

Reviewed the complete direct-frame setup, reused native hooks, cleanup, ownership,
meter/poison path, USER refactor, full profile/CP fixture binding, independent
finalizer and mutation/test coverage. No existing M1/LP3 source, runtime dependency
or numeric cap was changed. Nine copied accepted primitives remain byte-identical.

Findings and fixes:

1. First oracle parsed decimal balances as GST hex, then lacked direct-message
   EIP161 cleanup. Retained failures. Explicit decimal conversion and the upstream
   touched-account/actual deletion cleanup now compare full roots, not partial
   balances alone. No transaction fee/nonce finalizer was introduced.
2. One compiled wrong-allocation mutation survived its original test: post-call
   budget rejection masked the excessive allocation. Retained15-control first
   attempt (14 killed,1 survived). The test now measures actual GAS and the second
   remaining allocation, and the mutation targets that observable boundary test.
3. Historical Registry initialization did not match all52 fields of the new
   synthetic profile. Retained earlier diagnostic outputs. Current initialization
   overwrites ALL52 CP slots and derives bond, aggregate and escrow funding from
   that exact profile. Oracle independently recomputes identity and checks CP.
   The first rebuild exposed an inconsistent aggregate; fixed that fixture too.
   This remains synthetic state, not a GenesisBuilder acceptance substitute.
4. Full UT validation, stored parent context, phase ordering, five complete system
   programs and block/disk atomic acceptance are not implemented by this primitive.
   No public SystemCall constructor or commit endpoint is exposed. All full gates
   remain explicitly NOT PASSED rather than interpreting structural inputs as
   validated facts. P5/P9 must supply these requirements before acceptance.
5. General host RAM/work/trie bounds cannot be inferred from gas. Documented actual
   candidate/epoch/PAY snapshots and whole-code scans; updated measured traces and
   gross-meter/EIP150 derivation, keeping general R18/R21/caps64 NOT PASSED.

Remaining required tests include compiled actual closeBlock pending bytes/tips36,
wrong padding/kind/offset mutations on that actual contract, validated signed and
fallback UT negatives/current-winner independence, compiled nested EIP150 and
rollback/USER integration, real GenesisBuilder/full programs/finalization/SV6,
full supported CP domains and new-source complete hosted corpus. Approval and
primitive test success do not satisfy any whole increment's acceptance.
