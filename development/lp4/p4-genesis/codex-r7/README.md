# R7 sealed genesis integrates actual factory and signed USER creation

Replaces the factory witness in the synthetic genesis bundle with the actual,
source-built D27 ChunkFactory from R6. The bundle has Registry, pending/bond
Escrow, ChainCaps and ChunkFactory components; ShareLedger remains a witness,
and Escrow settlement/finalization is still incomplete. No full-five claim.

Complete actual genesis MPT construction and independent py-evm/Python genesis
derivation agree for4 and1024 members. The new complete code/profile identities
and roots replace only the current synthetic inputs, never historical evidence
or production CP. All52 ChainCaps words, ordered records/lists, bond custody,
R23, durable rewards and exact supplied code/RLP identity remain checked.

The actual signed USER pipeline now also creates a D27 chunk from a factory
installed by the sealed genesis image, then executes approved phase-2 issuance.
An assertion checks the real factory code hash at C0C005, actual child code00aa,
creator/child nonce, successful receipt, authenticated tips, no early credit,
exact mint and rollback of the private candidate after a late bad commitment.
The original image contains no child and remains byte-identical after rejection.
This does not establish disk/crash/reorg rollback.

The local full suite passes96 Rust tests. Independent actual execution compares
eight signed USER/SYSTEM cases: three transaction types observing pre-issuance
COINBASE balance and one legacy factory creation, each with signed/fallback SYS
metadata. Exact receipts, authenticated tips, raw SYSTEM gas, complete states
and roots match. These are actual signed transactions inside synthetic context;
validated UT/header/parent admission remains NOT PASSED.

Re-executed issuance controls are preserved on this exact source. Two additional
compiled integration omissions must fail semantic assertions: restoring a factory
witness despite recomputing a matching genesis identity, and skipping the actual
signed USER creation. Build errors never count as kills. Raw vectors, copied
mutant sources and every earlier attempt remain local with explicit receipts;
the compact public packet and CI reproducer do not imply raw archives are public.

RESOURCE-FACTS.json binds numeric synthetic measurements to the complete genesis
profile hash, specification hash, five individual code hashes and model version.
These are not general bounds or admission success flags. Logical code is8939
bytes, not the historical8787. Slots remain75/5175 for the two images. Factory
USER gas57941 yields tips173823; observer USER gas43452 yields130356. Raw SYS
totals224958/224973 are measurements, not general allocation sufficiency proofs.

All27 full gates remain NOT PASSED: real ShareLedger/full Escrow, full-five
GenesisBuilder and independent end/full execution, validated phases/context,
real finalization/SV6, general R18/R21/caps64, immediate all-operation resource
enforcement and durable atomicity remain mandatory. Boot=false, no merge.
