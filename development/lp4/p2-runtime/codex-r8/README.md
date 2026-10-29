# Actual Registry/payment implementation — P2/LP4 NOT PASSED

Owner approval is recorded separately in development/LP4-REGISTRY-WRITE-APPROVAL.md:
only LP4 register10, setRewardAddress2, claimBond10 replace9/1/9. M1 and old evidence
are unchanged; original9/1/9 are not passed. Every other limit remains binding.

Actual source-built contracts implement six user operations, exact error prefix,
shared budgets, time/status/admission rules, swap deletion, ordinary sweep and closing
bond cursor, plus Registry-only Escrow.releaseBond(id) with actual record-derived
recipient/amount and native PAY custody routing. CP reader exposes no runtime setter.
The explicit ABI/layout and source-bound successful SSTORE proof are documented.
RewardEscrow here implements the bond module, not pending/settlement/finalization.

Recorded verification:40 Rust tests,63 full-bytecode comparisons with independent
py-evm whole-state roots/gas/output and a separate domain-object transition model;
actual SSTORE operation tracing (stronger than only dirty-key counts);17 independently
Solidity/Rust compiled assertion controls from a frozen source snapshot. Raw first
attempts/classification failures remain; REVIEW.md states fixes and scope honestly.

Observed gas/access/writes are stored in resource-measurements.json.gz with complete
synthetic CP52, code/native/model/source/input hashes. Admitted full profile identity,
general resource derivations/horizon and caps64 evidence are explicitly missing;
these measurements cannot pass R18/R21 or be accepted resource facts. The user-write
source proof is conditional on the documented complete-runtime/host assumptions.

Full GenesisBuilder/admission1024, remaining source-built systems, real apply and
settlement/SV5/SV6/finalization, general R18/R21/caps64/R16/LAG, persistence/crash/reorg,
P2 deviation/GST/license criteria remain mandatory. All26 acceptance ledger entries
remain NOT PASSED. New-source CI must be inspected separately after publication.
No foundation merge, main merge, boot, activation, deployment or billing change.
