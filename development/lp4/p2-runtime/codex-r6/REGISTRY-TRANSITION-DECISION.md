# Consolidated Registry execution semantics — PROPOSED, NOT APPROVED

This does not reopen PAY-01, CREATE-01 or the complete approved PAY-02 lifecycle
contract. Those decisions remain binding. M1 and prior evidence remain unchanged.
No proposed transition below is implemented, counted PASS or used to admit a block.

## Why an owner decision is required

The preserved normative inventory gives:

- consensus.md:21 defines REG_OPS, SWEEP_MAX and SWEEP_SCAN CP fields; :86 uses
  lastShare/graceStart in active eligibility; :88–89 fixes M_min and R_max.
- incentives.md:43–56 defines HELD/exited/tomb/PAID, dueHeight=h+U, reactivation
  “after W_act”, refunds “after dueHeight”, tomb deletion after burnHeight+E_win+D+1,
  and the BadCalldata→BadValue→Closing→RegOpsLimit precedence.
- execution.md:44–60 fixes arrays/record words and user write ceilings; :64 specifies
  markClose before recordShares, settlement and beginBlock. It does not specify
  REG_OPS consumption or user-transition timestamp assignments.
- threat.md:119 repeats reactivation after W_act; validation.md:1799 repeats that
  delay without its comparison/anchor; CL-OPS1 checks R_max=0 but does not define
  REG_OPS. FINAL_DESIGN.md repeats these statements, not additional algorithms.

The source-bound search receipt includes the preserved reference files, decisions,
legacy code and LP3 model. No implementation of these missing transitions was found.
The corpus has no Registry transition oracle resolving them. Ordinary engineering
choices such as Solidity packing/compiler settings will be documented by implementation;
these timing, validity, budget and payout-order choices change consensus-visible state.
PAY-02 authorizes the bridge but requires Registry to establish *actual eligibility*;
it does not define these missing maturity/counter rules.

Concrete alternatives disagree: at h=dueHeight an inclusive check refunds but a
strict check does not; setting a new member's graceStart=h can include it in A_(h+1)
where retaining genesis's zero excludes it; consuming REG_OPS for reward changes
can reject a later claim that a register-only counter permits. See counterexamples.json.
These are decision witnesses, not execution acceptance or tests of an approved rule.

## Option 1 — approve all following additive LP4 Registry rules

1. **Scope and time.** h is the execution block NUMBER. Arithmetic for height sums
   is checked in a wider intermediate; unrepresentable state height is an error,
   never wrap/saturation. Membership writes affect consensus selection only from
   the next parent-state snapshot as already required. No extra user-supplied height,
   eligibility boolean or counter reset is accepted.
2. **Budgets.** Each successful user call among register/deregister/removeInactive/
   reactivate/setRewardAddress/claimBond consumes one REG_OPS unit in slot3's
   per-block counters. register/reactivate additionally consume one shared R_max
   unit. Failed/reverted calls consume neither unit nor records/bonds (normal gas
   charges remain). beginBlock, SYSTEM_ADDRESS-only, initializes counters once per
   h; duplicate or backwards initialization invalidates the mandatory phase. Calls
   cannot reset counters; internal releaseBond, system sweep/cursor, getters and
   PAY probes are not additional user operations. Reject a would-exceed budget
   with RegOpsLimit, after the existing prefix error precedence. Preserve the
   required R_max=0 register/reactivate rejection even when REG_OPS>0.
3. **Admission/status.** register requires no record for msg.sender and room under
   M_max/REG_RECORDS_MAX; it cannot overwrite exited, tomb, HELD or PAID records.
   Member identity cannot be zero, SYSTEM_ADDRESS or C0C001..C0C0FF, matching the
   genesis member exclusion explicitly for runtime registration. Existing records
   have active/exited status, tomb flag and HELD/BURNED/PAID bond states. Ordinary
   operations cannot operate on tomb/BURNED/PAID records. setRewardAddress requires
   an active or exited, HELD, non-tomb record; it cannot redirect a paid/burned bond.
   All existing Closing restrictions remain. Other semantic failures revert after
   the specified prefix; no implicit repair/replacement/fallback recipient.
4. **Register/reactivate clocks.** Successful register sets lastShare=0,
   graceStart=h, active/HELD, bond=B_reg, exitHeight=dueHeight=burnHeight=0.
   Successful deregister/removeInactive writes exited, exitHeight=h, dueHeight=h+U,
   preserving bond/reward/lastShare/graceStart and HELD. Reactivate requires exited,
   HELD, tomb=0 and h>=exitHeight+W_act; set active, graceStart=h and clear
   exitHeight/dueHeight, preserving bond/reward/lastShare. It never bypasses
   R_max/REG_OPS, M_max or Closing, and cannot revive BURNED/PAID.
5. **Inactivity/refund boundaries.** removeInactive requires active/HELD/non-tomb,
   h>=max(lastShare,graceStart)+Theta_inact, and preservation of M_min. Claim/sweep
   requires exited/HELD/non-tomb, h>=dueHeight and not Closing. Tomb cleanup requires
   h>=burnHeight+E_win+D+1 and no outstanding HELD liability. Equality is eligible
   in all three checks; earlier heights are not. These are new explicit boundary
   definitions, not recovered M1 definitions. Theta_inact=0 or W_act=0 follows the
   same inclusive formulas without unchecked subtraction.
6. **Storage traversal/order.** RecArr/ActList append new entries; removing an entry
   swaps the last into its position and updates the moved record's position, then
   shortens and clears the old tail. No implicit address sorting after genesis;
   consensus's separately specified address sorting remains unchanged. Ordinary
   sweep occurs during beginBlock after its counter reset: at most SWEEP_SCAN
   inspected records and SWEEP_MAX deletions (also never over the existing 64/16
   maxima), starting at sweepCur with wraparound. A swapped-in entry at a deleted
   cursor is next inspected and consumes another scan step. Store the next unvisited
   cursor, or zero if empty. Refund/tomb deletion happens only when eligible above;
   failure of an eligible PAY in the mandatory sweep invalidates the phase/block.
7. **Closing cursor.** The existing close transition freezes ordinary Registry user
   writes. First markClose initializes its cursor at index0; later calls inspect at
   most RET RecArr entries in ascending current index order, advancing over every
   inspected entry, including tomb/BURNED/PAID. Since user deletion is disabled in
   Closing, no array compaction occurs during this traversal. For each HELD/non-tomb
   entry, releaseBond(id) pays the actual stored reward/bond from Escrow and marks
   PAID, subtracting heldTotal atomically, regardless of ordinary dueHeight/status.
   BURNED/PAID entries incur no duplicate payment; their original tomb/evidence rules
   remain. No empty-recipient skip or burn; a required refund failure invalidates the
   phase and restores cursor and every payment/status/counter in the call journal.
8. **No waiver.** Escrow releaseBond derives real Registry HELD record and has
   Registry-only authorization; Registry establishes the ordinary/sweep/cursor
   eligibility above. Do not add caller-supplied payout fields, arbitrary debit,
   callbacks or account creation. Preserve all approved PAY-02 admission/probe
   precedence and semantic genesis rejection. Re-prove R16/LAG and all original
   per-function write ceilings, R18/general SYS gas/access, R21 and caps64 against
   this implementation and full supported CP/profile/model identities. If a proof
   conflicts, do not alter an existing cap/formula or exclude the case silently:
   acceptance remains blocked and the contradiction returns to the owner.

## Required acceptance (all NOT PASSED)

Actual compiled contracts must exercise every operation, budget at 0/limit/limit+1,
failed-call rollback and duplicate initialization; both user/mandatory error contexts;
time at threshold-1/equality/+1 and zero/max CP boundaries; activity/grace membership
from the correct parent snapshot; duplicate registration/status exclusions; M_min,
M_max, record capacity; tail/middle/single deletion, cursor wrap/swapped entry and
scan/delete caps; ordinary/sweep/closing Escrow routes; mixed HELD/BURNED/PAID/tomb
cursor with RET-1/RET/RET+1 and exactly-once refunds; bad destinations and nested
rollback. Compare complete balances, storage and roots to an independent transition
oracle; kill compiled boundary/counter/order/cursor/authorization mutations. Preserve
all original SV/CF/CL values, finalization, zero final subsidy and pending[F] absence.
No counterexample witness alone is an execution PASS. Boot remains disabled.

Option2: owner supplies a complete alternative for the unresolved Registry rules;
PAY-01/CREATE-01/PAY-02 and every existing acceptance obligation remain binding.
