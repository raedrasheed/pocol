## LP3 S4: parent membership, nonce assignment and proposer Draw

Base: S3 merge 866666e2f129c26575409d07d8b7b4fa11899f78. Implements S4 from the accepted LP3 plan on a separate branch, without starting S5.

Adds explicit supplied-parent A_h* computation, sealed-parent nonce seed/tickets, exact bounded/unbounded nonceMax, half-open partitions with empty-range handling and owner lookup, and proposer weights over exactly [h-1-Ww,h-2]. Draw uses sorted addresses, uniform fallback only for all-zero weights, full 256-bit unbiased rejection and checked be32 retry counters. No signing/private keys, state certificate, EVM integration or boot permission.

Tested implementation: 71cbbcc5cfed12eda204e29579f31cf672e7d972. Clean Rust 1.58.1 regression: 103 tests passed. Independent Python/hashlib/native-integer review: 697 complete scenarios and 7977 sampler cases, zero mismatches; includes max 65535 members and over-bound rejection. All eight mutations detected with isolated temporary targets. Builds, format and fixture checks passed. M1 and all earlier acceptance evidence preserved.

Evidence: development/lp3/s4-assignment/codex-r1/REVIEW-S4.md and RESULTS.json. Separate Codex review found no blocking implementation issue; not an external reviewer identity or full LP3 acceptance. Initial review-probe compile FAIL was corrected and retained. Windows execution remains BLOCKED BY PLATFORM; live parent/ledger authentication and S5 are NOT RUN. Parent inputs are synthetic supplied facts in tests, never certificates.

M1 A1 is assumed; actual SHA ticket collisions have no specified tie policy and fail closed. Counter exhaustion returns an explicit error rather than wrapping or biased selection. No known unmet pure S4 criterion under these assumptions; draft for acceptance review. Do not merge automatically or begin S5.
