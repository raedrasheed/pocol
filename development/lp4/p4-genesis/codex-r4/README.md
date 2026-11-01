# R4 approved phase-2 issuance implementation — partial execution evidence

LP4-ISSUANCE-01 option1 was explicitly approved. The immutable R3 proposal is
historical; `development/LP4-ISSUANCE-APPROVAL.md` records the decision separately.
M1, LP3, prior evidence and numeric limits remain unchanged. Boot remains false.

The crate-private candidate now executes authenticated, actually signed USER
transactions using byte-identical P3 primitives, derives tips from executed gas,
then credits the existing Escrow exactly once before the normal closeBlock call.
There is no public credit method, caller-supplied amount, success flag, final flag,
extra EVM transaction, fee, nonce, callback, log or USER receipt. A new SYSTEM/PAY
epoch starts after the credit. Checked arithmetic and real code/profile/storage/
root checks reject malformed candidates. Failure poisons the whole owned candidate.

Actual genesis construction installs the five supplied code images, all52 CP
words, ordered M0 records/lists, heldTotal and funded Escrow, checks R23 and builds
the complete Ethereum state MPT against allocRoot. The fixture includes three
real components (Registry, pending/bond Escrow, ChainCaps) and two explicitly
synthetic witness roles. Consequently this is NOT full GenesisBuilder acceptance.
PendingEscrow implements normal pending creation and delayed winner recording;
it does not implement settlement or real finalization. FinalFundingWitness is a
synthetic observer only: prior pending deliberately remains in its final test.

The final local suite passes92 Rust tests. Independent py-evm execution matches
two genesis roots (4/1024 members) and six actually signed USER/phase-2 executions
across legacy155/2930/1559 and signed/fallback proposer metadata, including exact
receipts, authenticated tips, raw gas, complete post-state and roots. The full
vectors remain local with a digest/reproducer and are required in the next CI
artifact. Compiler and signed fixtures reproduce byte-for-byte. Raw failed build
and gas-boundary attempts remain local; no failed attempt counts as passing.

`hosted-r2-final/` closes the historical R2 run39451150747: five successful jobs,
all five ZIP hashes verified, full20661 identities/source2807 checked, no excluded,
blocked or incomplete comparison cases. Its1173 rejection-presence controls are
not rejection-reason equality or signed transaction coverage. Legacy16 PASS,
8 platform-blocked and2 unrun remain distinct. That source is d741363e..., not R4.

All27 full execution/resource acceptance obligations remain NOT PASSED. Actual
five-program semantics, validated UT/current and stored parent/grandparent header
context, complete prescribed phases, real finalization/SV6, general R18/R21,
caps64, immediate/native cap enforcement and disk/crash/reorg atomicity remain
mandatory. No foundation merge is authorized by these partial results alone.
