# Continuation checkpoint — no new owner decision required

Continue on the existing codex-lp4-p2-runtime branch and PR24, never a duplicate.
PAY-02 option 1 is approved in full. Do not request PAY/CREATE/lifecycle approval again.
The native pre-transaction predicate is implemented and partially verified, but the
whole lifecycle remains NOT PASSED; no foundation merge is authorized by these results.

Next: finish actual compiled Registry/Escrow routing and Registry-only releaseBond(id)
with record-derived HELD amount/recipient, ordinary/due/closing cursor eligibility and
atomic PAID/deletion/heldTotal/transfer rollback. Preserve existing error precedence,
Closing and SYSTEM_ADDRESS restrictions; admission zero PAY must give RewardNotDurable.
Implement semantic GenesisRewardNotDurable against actual complete alloc without
altering GenesisSpec format/error order or old fixtures. Full contract cases include
zero/balance-only/new/nonce/code, lifetime across transactions, reward updates, missing
bond destinations without burn/skip, independent storage/root comparisons and compiled
mutations. SV5 absent-reward settlement fixture remains mandatory. Re-derive resources
for all affected actual paths with complete CP/profile/model binding; do not reuse old
estimates. Full P2 checks and license/provenance audit still mandatory before merge.
Follow the P0-P10 plan for all remaining increments, including real finalization/SV6,
R18/general R21/caps64 and crash/reorg. All 24 ledger entries are NOT PASSED; boot false.

Use working GitHub connector with expected-head lease and full local/remote tree
comparison. Check new-head CI separately. Prior head run39053777725 success is not a
new-head receipt. Preservation and no-main/deploy/billing/Claude/coordinator rules apply.
