# SYS-01 R3: compiled nested boundaries and an unapproved funding decision

LP4-SYS-01 remains owner-approved. No runtime issuance path or new rule is
installed. [ISSUANCE-DECISION.md](ISSUANCE-DECISION.md) consolidates the distinct
missing funding-point/host interface and complete proposed acceptance contract.
It is NOT APPROVED. Both timing diagnostics are unapproved unsigned EVM controls,
not verified PoCol blocks. The proposal is separate from every recorded approval.

83 Rust tests pass, including three new R3 tests. A separately source-bound P3+R3 scratch overlay passes102 tests; PR25 itself is unchanged. Thirteen current-source direct
SYS frame comparisons reproduce the R2 outputs byte-for-byte. Five independently
executed compiled nested cases cover EIP150 equality102758/minus1, child rollback,
normal success and outer revert; four successful frames match gas/refund/output/
complete roots and the outer error matches poisoned-candidate rejection. Rejected
frame gas is an independent measurement, not claimed as a Rust error-gas match.
Fifteen base and two new nested hook mutants all build and fail assertions.
The two issuance countermodels independently match full roots/gas/observed state;
their roots differ despite equal final escrow balance/issuance and gas43452.

Reproduce using the same pinned Rust/solc/Python environments as R2:

```sh
NODE_PATH=/path/to/pinned/node_modules node prototypes/lp4-execution/tests/compile_system_boundary.cjs /tmp/boundary.json
cmp prototypes/lp4-execution/tests/data/system-boundary-witness.json /tmp/boundary.json
/path/to/pinned/python prototypes/lp4-execution/tests/system_boundary_oracle.py /tmp/boundary-independent.json
cmp prototypes/lp4-execution/tests/data/system-boundary-independent.json /tmp/boundary-independent.json
LP4_ISSUANCE_DIAGNOSTIC_PATH=/tmp/issuance.json LP4_SYS_REPORT_PATH=/tmp/sys.json cargo +1.99.1 test --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml
/path/to/pinned/python prototypes/lp4-execution/tests/system_boundary_oracle.py /tmp/boundary-repeat.json /tmp/issuance.json /tmp/issuance-independent.json
/path/to/pinned/python prototypes/lp4-execution/tests/system_oracle.py /tmp/sys.json /tmp/sys-independent.json
RUSTUP_TOOLCHAIN=1.99.1 python prototypes/lp4-execution/tests/system_mutations.py /tmp/fresh-base-mutants
RUSTUP_TOOLCHAIN=1.99.1 python prototypes/lp4-execution/tests/system_mutations.py /tmp/fresh-nested-mutants boundary
```

[RESOURCE-REVALIDATION.md](RESOURCE-REVALIDATION.md) derives the required remaining
budget condition and actual measured EIP150 headroom; general R18/R21/caps64 remain
NOT PASSED. [REVIEW.md](REVIEW.md) records the separate author pass and limits.
Source bindings, compact results/logs/digests are public in this packet; full raw
attempts and all mutant sources/logs remain local with preserved archive hashes.
All M1, LP3 and R1/R2 evidence remains unchanged. No source-result inheritance is
claimed for hosted runs; verify the exact new head and all artifacts/cases.

Still NOT PASSED: full validated UT/parent schedule; real five complete programs,
GenesisBuilder/manifest; actual closeBlock/pending/tips36/funding; whole USER+SYS
and disk atomicity/crash/reorg; real settlement/finalization/SV6 and every general
CP/resource/import criterion. All27 full LP4 gates and boot=false remain unchanged.
No PR25/26 merge into P2, main merge, activation/deploy/billing/protection bypass.
