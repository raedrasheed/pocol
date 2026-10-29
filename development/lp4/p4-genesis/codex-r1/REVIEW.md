# Separate sequential author review of the decision packet

No Claude/coordinator/subagent or external review was invoked. This is an author
review of diagnostic evidence and a proposal, not GitHub approval or acceptance.

Reviewed source citations against preserved execution/incentives/consensus and
the recorded LP4 approvals. The old unsigned P2 controls do not specify a real
SystemCall. Metadata amount/kind/proposer is required, while the tips calldata is
exactly36 bytes. No EVM ORIGIN mapping or explicit SYS epoch/nonce/refund accounting
was found in the inspected accepted sources. The proposal identifies these
choices as new, preserves the closed decisions, and asks once for the full packet.

Reviewed direct-frame setup, standard gas/storage/transient APIs, immutable base
versus private candidate state, fixed SYSTEM caller with independent origin,
actual compiled Registry controls and both independent roots. The diagnostic's
fresh model retains persistent writes while resetting its EVM context. Shared
original storage/refund/warmth/transient state intentionally persists as a
countermodel. Neither is installed as the selected implementation.

Findings fixed and retained:

- An initial reporter showed zero for an unloaded storage slot. The independent
  oracle rejected it. Resolve the actual base value under the current journal
  rather than treating the loaded subset as complete state. Earlier output/log
  remains in the archive and is not success.
- An added header-validator probe initially called the unbound instance method
  incorrectly. Instantiate the unmodified VM correctly; preserve the TypeError
  log, then verify both independent validation rejections. API failure is not
  an accepted negative consensus case.

All25 final observations now compare through independent unmodified APIs. The
actual compiler artifact, execution JSON and independent JSON reproduce exactly
using the committed reproducer against clean R20. No source-level or compiled
mutation here passes a SystemCall gate: those required controls and full real
protocol-contract execution follow the owner decision. The initial probe is not
one of the five actual system programs, and the synthetic Registry state is not
a ParamGate/GenesisBuilder-valid profile.

The proposed min(30M,remainingG_SYS), gross gas, new epoch, ORIGIN transport and
pending kind/word encoding are not existing verified behavior. Their boundary,
EIP150, authorization, PAY, whole-block rollback and general resource implications
are explicitly mandatory. All27 existing gates remain NOT PASSED, boot false.
