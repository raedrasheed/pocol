# LP4 SYS-01 R2 — internal message execution, NOT accepted integration

Owner approval is recorded separately in
[LP4-SYS-APPROVAL.md](../../../LP4-SYS-APPROVAL.md), binding complete option1 at
b1f25e0613fbeb3832dfb20252f76b92ef72b90d. The R1 proposal/diagnostics are
historical and unchanged. M1, LP3 evidence and all numeric ceilings are preserved.

The runtime now has a crate-private direct-message primitive using the SAME
native PAY, CREATE-01 and CODE-01 hooks as USER execution. Only SYSTEM frames
receive ORIGIN from the structural signed/fallback metadata input; CALLER stays
SYSTEM. Ordinary USER transact(), authentication, nonce, fee and ORIGIN behavior
are unchanged. A fresh native context/database epoch starts at each call. The
meter allocates min(30M, remaining G_SYS) and charges raw gas before refunds.
The private in-memory candidate becomes unusable after any error, revert, OOG,
budget/code violation or unexpected log. Epoch deletion/empty-account cleanup
is applied before the next call. There is no canonical commit/boot API.

These are implementation primitives, not a public adapter accepting unvalidated
UT/parent facts. P5 must derive the metadata/context from verified current UT and
stored headers and enforce phases. Actual five-program code inventory/hash and
GenesisBuilder admission, complete pending storage/closeBlock/finalize, USER+
SYSTEM whole-block atomicity, persistence/crash/reorg and general resources are
still NOT PASSED. The pending word helper has byte controls; it is NOT actual
Escrow pending execution. Structural proposer checks do NOT verify a UT signature
or fallback eligibility. No current-winner field is accepted by the primitive.

Reproduction uses the pinned Rust1.99.1, solc0.8.28 and hash-locked py-evm tools:

```sh
NODE_PATH=/path/to/pinned/node_modules node prototypes/lp4-execution/tests/compile_system_epoch.cjs /tmp/system-epoch.json
cmp prototypes/lp4-execution/tests/data/system-epoch-witness.json /tmp/system-epoch.json
LP4_SYS_REPORT_PATH=/tmp/system-vectors.json cargo +1.99.1 test --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml
/path/to/pinned/python prototypes/lp4-execution/tests/system_oracle.py /tmp/system-vectors.json /tmp/system-independent.json
RUSTUP_TOOLCHAIN=1.99.1 python prototypes/lp4-execution/tests/system_mutations.py /tmp/fresh-system-mutants
```

There are nine new Rust tests: fresh epochs, SYSTEM-only ORIGIN/CALLER and absence
of synthetic account/nonce effects; zero/exact/minus1/raw-refund budget; late
candidate failure and logs; exact pending helper bytes/context rejection; actual
compiled Registry markClose/beginBlock/closing bond PAY and CALLER authority;
unchanged USER ORIGIN/fees/nonce; independent report; and observable allocation,
GASLIMIT/BASEFEE/GASPRICE including30M/remaining/u64 maximum boundaries.

The independent oracle executes13 actual frames, comparing complete roots,
accounts/code/storage, output, raw gas/refunds and zero logs. Its separately
written PAY hook executes real Registry/Escrow bond calls. All52 ChainCaps words
in those fixtures now match the full synthetic profile, checked independently.
The initial held bond, aggregate custody and escrow balance match that profile's
B_reg; they are synthetic initialized state, NOT actual GenesisBuilder output.
The full GenesisSpec/hash, CP, chain, kind, fork schedule and semantic/model
version are preserved. AllocRoot/sysCodeHash are not verified builder products.
Opcode and storage traces are measurements only; see RESOURCE-REVALIDATION.md.

Fifteen successful-build mutation controls exercise wrong ORIGIN/CALLER, zero
signed metadata, intrinsic/fees/nonce, refunds, allocation/remaining budget,
warm/transient contamination, failed candidate reuse, discarded logs and pending
kind/offset. The first attempt's surviving allocation mutant and all failed
build/oracle/fixture attempts are retained, never counted as PASS. The corrected
allocation test observes gas from inside the frame and on its second call.

CI now also triggers for PRs stacked on the P2 branch, with the same five jobs,
read-only token and pinned actions. It compiles the witness, runs the independent
oracle and compiled controls, and preserves outputs. No branch requirement is
removed; hosted results must be verified separately for the new source. The old
R4/R20 corpus results are not inherited as a new-source run. PR26 and PR25 must
NEVER merge into P2; dependency acceptance and foundation retargeting remain.

All27 complete LP4 obligations and every full SYS acceptance group remain
NOT PASSED. No acceptance, boot, main merge, deployment or billing authorization.
