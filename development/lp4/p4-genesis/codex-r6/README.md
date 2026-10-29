# R6 source-built D27 ChunkFactory component

Implements the already documented M1 storage.md D27 at system address C0C005:
store(bytes), data length1..24575, salt=keccak256(data), exact prescribed initcode,
runtime=00||data and idempotent CREATE2. It uses no storage or logs; no M1 rule,
numeric limit, owner decision or historical evidence is changed. The source-built
runtime is762 bytes with pinned solc0.8.28/cancun/optimizer200/noCBORmetadata.

Three component cases (data lengths1/31/24575) execute creation and a second
idempotent call in Rust and independently in py-evm. All six executions compare
exact gas, returned address, complete roots/post-state, creator/child nonces and
runtime bytes. Actual opcode tracing observes zero SSTOREs and CREATE2 only on
the first call. Empty/oversize/value/nonzero-storage-collision calls independently
reject; those four comparisons assert rejection outcome, not identical gas/root.

A separate Rust profile-bound execution test sets B_code_max32768 and still
requires D27 data24576 to reject. Its compiled larger-data-cap mutation must fail
that assertion: the boundary cannot be masked by Cancun's baseline24576 runtime
ceiling. No production CP is altered. All profile inputs are synthetic components,
not complete GenesisBuilder/validated apply acceptance.

The full local Rust suite passes95 tests. Compiler artifacts and complete vector
files reproduce byte-for-byte. The final compiled contract mutation results and
all failed attempts are retained separately; compiler failures never count as
semantic assertion controls. The source/full-vector/compact result receipts bind
these measurements to their exact files.

This adds an actual fourth program component, but does not replace either
synthetic role in the historical R4 manifest fixture. ShareLedger, complete
Escrow settlement/finalization, actual full-five GenesisBuilder, validated UT/
stored parent/grandparent phases, general R18/R21/caps64 and durable crash/reorg
atomicity remain mandatory NOT PASSED. All27 final gates remain NOT PASSED;
boot=false; PR26 stays draft/review-only, no merge into P2 or main.
