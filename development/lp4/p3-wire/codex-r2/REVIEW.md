# R2 separate sequential same-author review

Reviewed after positive and negative tests, then six compiled controls. No second
human/agent review claimed. No owner decision or consensus amendment required:
USER-only context uses existing Cancun semantics and accepted M1 execution.md.
P5 must still bind the provided context/ancestors/parent to stored validated headers.

USER phase clones the complete in-memory snapshot, decodes/recover-signs every
transaction, reserves declared gas against remaining block gas, executes actual
native Cancun sequentially, adds checked host gasUsed*effectiveTip to blockTips,
encodes cumulative typed receipts, clears touched empty/selfdestructed accounts,
and computes independent P1 transaction/receipt roots and intermediate state root.
Commitment mismatch or any later invalid transaction returns error without caller
snapshot writes. Candidate state/receipts/tips are private with read-only getters;
this is not persistent commit or full PoCol block admission. Intermediate USER
state root must NEVER be compared as the final header root before system phases.

Independent four-transaction fixture uses unmodified CancunVM.apply_transaction,
not a sender-spoof or success flag. It covers signed legacy155/2930/1559, sequential
nonce effects, actual SSTORE clear refund4800, transaction revert/value rollback,
zero-tip zero-value call to absent address with EIP161 deletion, all account sets,
nonces/balances/code/storage, complete secure state root, receipt bytes/indexed
roots/cumulative gas and host blockTips. Distinct tests corrupt txroot/receiptsroot/
gas/bloom and append invalid nonce/wrong-chain/unprotected/unsupported types; every
failure leaves the input DB byte-debug-identical. Empty USER phase and exact/minus1
local budgets covered. Tight block gas admits each tx alone but rejects declared
reservation against remaining gas even when sum of actual gas would fit.

Findings and fixes:
- Initial independent harness used state.apply_transaction directly across multiple
  transactions, omitting VM lock_changes, causing stale original-storage/warmth and
  incorrect19900 refund. Preserved failed attempt; fixed harness to actual unmodified
  VM.apply_transaction. Correct4800 refund and107913 gas for first3, then fourth
  zero-value call. Rust production logic was not changed to fit the stale oracle.
- CacheDB::commit retains touched empty accounts. Apply explicit existing EIP161
  cleanup, as the pre-existing GST control does, before the next transaction/root.
- Cleanup mutation initially failed via unwrap rather than assertion; not counted
  PASS. Added explicit root-is-ok assertion and reran all6 compiled controls on final
  source. All6 compiled and then failed meaningful assertions. Historical receipts
  and logs preserved locally and prior results public.

64 Rust tests pass, no ignored/filtered full-suite checks; independent fixture
regenerates identically. Six final compiled kills: cleanup, commitments, gas
reservation, tips, cumulative receipt, state application. No dependency/lock/M1/LP3/
ledger/owner-decision changes. Existing P2 tests remain strict in CI. No claim of
actual RPC/pool CI2, full block CI3, final roots/system phases, disk/crash/reorg,
complete profile resource bounds or P2/P3/LP4 acceptance. Boot remains disabled.
