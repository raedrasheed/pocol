# Consolidated payment lifecycle decision — PROPOSED, NOT APPROVED

No M1 rewrite, waiver or assumed owner answer. LP4-PAY-01 and LP4-CREATE-01 are
already approved and implemented; do not request their approval again. This proposal
concerns additional reward-address lifetime and bond-custody rules needed to finish
actual Registry/Escrow payment integration. All P2/LP4 gates remain NOT PASSED.

## Binding facts and reproduced gap

- M1 consensus.md:31–41 constrains member **id**, not rewardAddr's existence or
  nonce; genesis loads alloc and system records, and credits M0 bonds to **Escrow**.
- incentives.md:33–48 stores rewardAddr/bond and refunds via PAY after eligibility;
  validation.md:971–977 requires all 1024 closing refunds exactly once, every
  rewardAddr receiving B_reg, heldTotal=0. It does not authorize burning a bond
  because a reward destination is unavailable.
- incentives.md:87–89 / SV5 explicitly require **reward share** payBurn for absent
  accounts. That must not silently become permission to burn bond refunds.
- LP4-PAY-01 permits only immediate Registry/Escrow code, debits their own balances,
  requires nonzero existing recipients and no account creation. Its approved
  eligibility does not require pre-transaction nonce/code or address durability.
- GenesisSpec format witnesses: the unchanged LP3 Rust decoder accepts zero and
  not-proven-present reward addresses. This proves FORMAT permissiveness only;
  it does not claim valid ParamGate, GenesisBuilder or boot acceptance.
- Compiled RewardDurabilityWitness: Registry-address code creates a child, performs
  a successful zero-amount PAY validation, then deletes that child later in the same
  Cancun transaction. This succeeds both with an absent initial child and with a
  **pre-funded balance-only** child (nonce=0, code empty). The source account and
  predicted address are derived from real execution; no success flag is supplied.
  Therefore “exists now” or even “existed and was funded before this transaction”
  cannot establish durable future closing/refund eligibility.

The full Registry implementation cannot choose between new admission restrictions,
implicit account initialization, a new refund destination or bond destruction without
changing protocol behavior. The preserved sources do not specify that choice. The
proposal below changes validity explicitly; it is not an inferred original M1 rule.

## Option 1 — complete additive LP4 payment contract (recommended)

1. **Custody.** Keep bonds in Escrow as M1 genesis specifies. register forwards its
   received B_reg from Registry to Escrow through PAY. Every bond return (ordinary
   claim, sweep and closing cursor) pays the record's stored rewardAddr and bond
   amount from Escrow. Registry must never acquire arbitrary debit authority.
2. **Internal bridge.** Add Escrow.releaseBond(id), callable only by Registry.
   It derives recipient/amount from the actual Registry record, not caller-supplied
   amount/address/authorization flags. Registry must establish HELD, tomb/ordinary
   due-height or closing-cursor eligibility before the call; Escrow verifies the
   actual HELD record and performs PAY from its own balance. Existing host phase
   entrypoints remain SYSTEM_ADDRESS-only; releaseBond is a distinct authenticated
   internal bridge, not a public or new SYSTEM_ADDRESS arbitrary-debit capability.
   PAID/deletion/heldTotal and transfer effects share one reverting journal.
3. **Durable native eligibility (new LP4-PAY-02).** In addition to every PAY-01
   precondition, the recipient must already have **nonce>0 OR nonempty code in the
   complete pre-transaction state**. Current-state nonzero/existence/overflow and
   no-selfdestruct checks remain. Balance-only accounts do not qualify: the compiled
   witness shows CREATE+SELFDESTRUCT can erase them. Apply this extra condition to
   all PAY calls, including zero amount; retain exact 64-byte calldata, 5000 native
   gas plus call overhead, own-balance debit, no callback/account creation and
   journal rollback. This explicitly narrows PAY-01's existing-at-call criterion;
   it is NOT already approved or silently implemented.
4. **Admission.** GenesisBuilder rejects a member reward failing this predicate
   with a new semantic GenesisRewardNotDurable error, preserving the original
   GenesisSpec decoder/schema/error order. No automatic nonce, balance or account
   initialization is added. Actual LP4 alloc files must explicitly provide durable
   destinations (e.g. nonce=1, zero balance for generated M0 reward addresses), with
   newly derived roots/identities. Keep prior opaque M1/LP3 fixtures unchanged and
   non-bootable. register and setRewardAddress probe PAY with amount zero, after
   applicable existing BadCalldata/BadValue/Closing/RegOpsLimit checks; false gives
   RewardNotDurable revert, with no record/heldTotal/payment commitment. The new
   condition applies even when B_reg/payment amount is zero.
5. **Lifetime/closing.** Preserve all existing Closing restrictions. Prove that an
   admitted nonce>0/code-nonempty account cannot be erased under Cancun/EIP-6780,
   including creation overwrites and same-transaction attacks. No transfer callback,
   implicit account creation, fallback recipient, arbitrary debit or minting is added.
   An unavailable bond destination in a corrupted/unadmitted execution fixture must
   revert the ordinary refund or invalidate a mandatory system phase without marking
   PAID/deleting/decrementing heldTotal. It must not burn or silently skip the bond.
6. **Existing reward burns stay mandatory.** Actual settlement still implements SV5
   payBurn on absent reward accounts, with no invalid native PAY call. Preserve its
   compiled execution fixture even if new admission makes that state unreachable
   through a valid new registration. Do not waive SV1–SV7s, CL-END, CF/CL closing,
   finalization, conservation, tomb penalties or any old acceptance value.
7. **Acceptance/resources.** Add actual compiled Registry/Escrow routing and bridge
   authorization tests, genesis admission, zero/balance-only/newly-created/nonce>0/
   code-nonempty destinations, both witness attacks, cross-transaction persistence,
   exact error precedence, reward updates, ordinary/sweep/cursor refunds, missing
   recipients, rollback/crash/reorg, gas/caps and independent balance/storage/root
   comparisons plus compiled controls. Re-derive/re-measure every affected R18/SYS
   access/gas and R21/account/trie obligation, with full CP/profile/model binding.
   No prior estimated bound is assumed valid. All deferred obligations, caps64,
   GenesisBuilder, real finalization and SV6 remain mandatory. Boot stays disabled.

Option 2: owner supplies a complete alternative lifecycle/custody contract that
preserves all mandatory payouts and liveness. No unavailable-recipient case, bond
amount or original acceptance criterion is silently excluded or weakened.

## Current safe scope

Only the approved collision rule, test witnesses and evidence are implemented here.
PAY-02, its admission errors and internal payment bridge are NOT implemented or
accepted. The witness deliberately asserts current PAY-01 behavior; its positive
receipt must remain historical evidence if a later approved rule rejects the call.
Do not merge P2 or label PAY/T2b/GenesisBuilder/closing acceptance PASS before the
full selected contract, required execution evidence and GitHub checks succeed.
