# R3 separate author review

Separate sequential author pass, not external PR approval. The independent
py-evm executions/HexaryTrie checks do not constitute human GitHub review.

Reviewed new compiled bytecode, exact EIP150 equality/minus1, nested transient/
storage and outer rollback controls, oracle bytecode/source binding and raw versus
required gas. The binary search alone would be self-referential; Rust additionally
checks the threshold independently derived by unmodified py-evm and all successful
outputs/gas/full roots. The outer-revert gas is not falsely called a Rust match.

Both new mutants build successfully and fail runtime assertions with original
bytecode/codehash. Neither a compilation error nor a trivially changed codehash
is counted as a mutation kill. New ABI/compiler/independent data reproduce exactly.

Reviewed protocol subsidy funding against preserved M1 execution/incentives/
economics/governance/FINAL_DESIGN and all recorded approvals. No specific balance
credit point or authorized native mint interface is explicit in those sources.
Actual compiled COINBASE BALANCE controls, with independent gas/state roots, show
that choosing before USER versus phase2 is consensus-observable. The proposal
therefore requests a NEW additive owner decision and installs neither model.
It does not reopen SYS-01/PAY/CODE/CREATE/Registry or genesis M0 bond approval.
No runtime mint path, public SystemCall adapter, numeric cap or M1 change is added.

R2 runtime native.rs/system.rs remains byte-identical. Only tests, fixtures,
oracles, reproducibility/CI and this proposal/evidence are changed. General
acceptance and resource gates remain NOT PASSED, boot=false. Both countermodels
are explicitly unapproved diagnostics, not valid PoCol-block acceptance results.

After approval, implement the complete chosen funding interface and every listed
acceptance/mutation/resource requirement. Until then stop dependent issuance/full
closeBlock acceptance; permitted independent work includes actual GenesisBuilder/
program preparation, validated UT/stored-parent context, existing runs/artifacts
verification and further non-issuance resource/test work. No routine approval is
needed for that work; no old decision should be requested again.
