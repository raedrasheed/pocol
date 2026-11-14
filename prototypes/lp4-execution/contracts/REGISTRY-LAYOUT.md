# Registry payment layout v1 (LP4 engineering implementation)

Authority: LP4-PAY-APPROVAL.md, LP4-PAY-LIFECYCLE-APPROVAL.md,
LP4-REGISTRY-APPROVAL.md and LP4-REGISTRY-WRITE-APPROVAL.md in development/.
New implementation encoding, not recovered M1 bytecode. M1 field locations unchanged.
No complete GenesisBuilder/apply/settlement/finalization acceptance is implied.

MinerRegistry: slot0 RecLen,1 ActLen,2 sweepCur,3 packed counters/phase metadata,
4 heldTotal; RecArr at Keccak(be256(5))+i; ActList at Keccak(be256(6))+i;
Rec[id] four words at Keccak(pad32(id)||be256(7))+k.

| Word | Bit ranges, low first |
|---|---|
| w0 | status0..7 (none0/active1/exited2), tomb8..15, bondState16..23 (HELD1/BURNED2/PAID3), recPos24..55, actPos56..87, lastShare88..151; remaining zero/reserved |
| w1 | reward address0..159; remaining zero |
| w2 | bond u256 |
| w3 | exitHeight0..63, dueHeight64..127, burnHeight128..191, graceStart192..255 |
| slot3 | userOps0..15, shared register/reactivate16..23, lastBegin24..87, beginInitialized88, Closing89, closeCursor90..121, closeInitialized122, lastMark123..186, closeH187..250; remaining zero/reserved |

Both counter fields update in one SSTORE. Phase/cursor metadata also shares slot3;
there is no hidden persistent counter, unjournaled success flag or extra user write.
markClose runs once per height, including pre-closing; first Closing mark initializes
and processes up to RET immediately (M1 cursorDone bound starts at closeH). beginBlock
requires that height's mark and runs once, resetting budgets then ordinary sweep.
Full host phase ordering/invalid-block atomicity remains a separate apply obligation.

ChainCaps: complete CP52 fields at slots0..51 in accepted schema order, immutable
through this runtime ABI; real GenesisBuilder must validate and populate them.
No constructor/root/ParamGate/resource/admission proof is implied by installing
synthetic test storage. RewardEscrow releaseBond has no storage writes; it authenticates
Registry, reads the actual HELD non-tomb record and calls native PAY from Escrow.
Pending pots, burns, settlement/finalization are not implemented by this bond module.

IRegistryUser is the canonical user ABI. Runtime fallback validates exact selector,
length, upper address padding and BadCalldata→BadValue→Closing→RegOpsLimit prefix.
All six ABI entries are payable so custom BadValue is returned by the strict dispatcher.
Other semantic errors follow the prefix. SYSTEM-only markClose/beginBlock are distinct
from Registry-only Escrow.releaseBond; no generic arbitrary-debit forwarder is exposed.

Bounds for successful user execution (SSTORE instructions, including unchanged writes):
register10; reward2; claim middle10/tail8; deactivate middle7/tail5; reactivate5.
No loop in user paths; swap removal touches at most one moved record. Only constant
views, authenticated bridge and native PAY are called; recipient code is never called.
These structural write ceilings do not prove SYS gas/access, R18/R21, caps64, R16/LAG,
GenesisBuilder, SV5/SV6, finalization, storage durability or an admitted profile.
Re-measure all of those with complete CP/profile/model identities before merge.

## v3 registry budgets (PoColSystemV3.sol only; V3-08, V3-09, V3-PR-E1)

Applies to the v3 program set (system-v3.json), reachable only through its sysCodeHash on a specVersion-2
genesis (CP schema v2). PoColSystemV1.sol and PoColSystemEarnIn.sol, their JSONs and every layout above are
unchanged. Earn-in already uses w0 bits 152..215 (registeredAt) and slot 8 (ACC_COUNT); the v3 programs record
registeredAt only and, since FN-2b (no probation pool), never write slot 8.

| Location | Bit ranges, low first |
|---|---|
| slot9 (budgets) | maintenance used 0..7, reactivations used 8..15, budgetHeight 16..79; remaining zero |
| w0 | earn-in layout; bits 216..255 unused (they held the V3-10 epoch counters from V3-PR-E4 until FN-2b deleted them) |
| w1 | reward address 0..159, lastReactHeight 160..223, lastMaintHeight mod 2^32 224..255 (moved from w0 bits 216..255, mod 2^40, by V3-PR-E4) |
| slot3 | as above, but userOps0..15 counts register/deregister/claimBond/removeInactive only and 16..23 register only |

Budgets per block (the user-path prefix, after BadValue and Closing, before BadPhase):
- setRewardAddress and topUp (maintenance): RegOpsLimit when maintenance used >= REG_MAINT_MAX = 8 (program
  constant); after the record is found live, RegOpsLimit when lastMaintHeight == height mod 2^32 (one maintenance
  operation per identity per block). They no longer touch slot3.
- register, deregister, claimBond, removeInactive: RegOpsLimit when userOps + maintenance used >= REG_OPS (CP 23);
  register also when its counter >= R_max (CP 17).
- reactivate: RegOpsLimit when reactivations used >= R_react (CP 56; 58 before FN-2b renumbered CP schema v2); it no longer touches slot3 or R_max. Valid
  only when exitHeight + W_act <= height < dueHeight (strictly before the refund sweep at dueHeight = exitHeight + U),
  and, when lastReactHeight != 0, height >= lastReactHeight + C_react (CP 57); otherwise BadState.

The three budgets are independent: maintenance and reactivate are not bounded by REG_OPS, so once the REG_OPS users
have used it up, setRewardAddress/topUp and reactivate still succeed in the same block, while maintenance used first
shrinks what is left for the REG_OPS users. A block therefore admits at most REG_OPS + REG_MAINT_MAX + R_react
registry user operations (vector reg-ops-exhausted-maint-and-react; mutants maint-bounded-by-reg-ops and
react-counts-reg-ops). The per-identity maintenance clock lives in the record (w0), which erase zeroes and register
writes with a zero clock, so "one maintenance operation per identity per block" holds per record lifetime: an exited
identity in its dueHeight block, before the bounded sweep reaches it, can maintain, claimBond, register again and
maintain once more. The second operation costs two REG_OPS and an R_max entry, so it never amplifies REG_OPS use
(vector maintenance-clock-per-record). (The clock moved to w1 with V3-PR-E4; w1 is a record word too, zeroed by erase and
written by register, so the rule is unchanged.)

slot9 is valid only for the height it carries; any other height reads as nothing used, so beginBlock writes nothing
new and the sweep, markClose and SYSTEM paths are unchanged. lastReactHeight is a full height. lastMaintHeight keeps
32 bits (w1's spare bits since V3-PR-E4; 40 spare w0 bits before): it can only refuse, never admit, so a maintenance
operation exactly k*2^32 blocks after the identity's previous one (or at such a height, for one with none) reverts once;
at T_blk 30 s that is about 4,000 years. setRewardAddress keeps both of w1's clocks; recordWords returns w1 as the reward
address alone and recordClocks(id) returns (lastMaintHeight mod 2^32, lastReactHeight). The host reads only bits it masks (apply.rs
miner_record, Escrow via bondRecord/payInfos), so the clocks change no host reading. ParamGate R8 on CP schema v2
requires U >= W_act + 2 (the window holds at least two blocks) and C_react >= U (lp1-node/src/gate.rs).

SYS footprint (R18, resources::derive on CP schema v2): Registry fixed slots 6 (0..4 and 9; 8 and the V3-10 slot 10 until
FN-2b), four words per record as before (no fifth word; erase clears the clocks with w0/w1), so SYS_SLOT_MAX grows by one
slot plus the 7 appended CP slots of the 59-field schema. A fifth record word was measured and rejected: zeroing it on erase put the worst beginBlock sweep
(64 scans, 16 refunds of ACCRUING records) at 930,080 gas, above admission's GAS_ENVELOPE beginBlock 926,000; with
the spare bits it is 864,902, as before the change (tests/v3_contracts.rs v3_sweep_deletions_keep_the_sys_envelope).

Changed Registry slots of successful user calls, earn-in -> v3 (tests/v3_contracts.rs v3_registry_write_ceilings,
wallet_bond_features_keep_their_write_ceilings_and_gas_on_the_v3_set): register10 -> 10; reward2 -> 2 (w1 with its clock,
slot9; 3 before V3-PR-E4 moved the clock out of w0); deactivate middle7/tail5 -> 7/5; reactivate5 -> 6 (w1 clock; slot9
for slot3); claim middle10/tail8 -> 10/8; topUp 3 -> 4 (w1 clock; slot9 for slot3); topUp with promotion 5 -> 6 (w1
clock and w0 promotion; FN-2b: 5, no ACC_COUNT write); partial register 11 -> 10 (FN-2b: no ACC_COUNT); accrue 2/4 ->
2/3 (FN-2b: a promotion writes no ACC_COUNT).
The beginBlock sweep changes the same slots per deletion as earn-in. Gas: a zero-bond register 174,447 against earn-in's
194,172 (FN-2b removed the probation-pool check; the cold slot9 read remains), within the wallet's EARNIN_REGISTER_GAS 400,000; every other user path below 200,000 (REGISTRY_GAS
1,000,000). The independent model is tests/v3_model.py (vectors tests/v3/registry-budget-vectors.json and
tests/v3/earnin-on-v3-vectors.json); seven planted budget mutants are in tests/v3/system-v3-mutants.json.

## v3 evidence penalties (PoColSystemV3.sol only; V3-16, V3-17, V3-PR-E2b)

Applies to the v3 program set only, as above; the spec-1 programs, host calls and phase order are unchanged.

| Location | Change |
|---|---|
| RewardEscrow pending[j] k=2 | winner 0..159, WIN_ACCRUING at bit 160 (new), remaining zero |
| RewardEscrow pending[j] k=1 | unchanged: proposer<<8, kind 0..7, INVALID at bit 168 (now set only as below) |
| MinerRegistry | no new slot or bit |

Host (apply.rs, v3 program set only), phase 0: recordShares(h-1), recordResult(h-1), then the evidence
(`invalidate(j, accused)` for a pot still pending, `punish(accused, reporter)`), markClose, pruneShares,
settle(h-D), beginBlock. Recording the parent's result first makes recordResult read the winner's record as of
block h-1, before any evidence of block h can tomb it. Every SYSTEM call keeps tx.origin = the block's signed proposer
(zero on a fallback block); `reporter` is that same address.

- recordResult(j, winner): as before, and sets WIN_ACCRUING when the winner is a live ACCRUING record (its bond is
  below B_reg: reaching B_reg promotes it). The same single write of the winner word.
- invalidate(j, accused), 68 bytes: BadPending unless j < h, pending[j] exists and its winner is recorded. Sets
  INVALID only when accused is pot j's winner and WIN_ACCRUING is set; otherwise writes nothing, so the pot settles
  normally after punish (the tombed offender's winner part, proposer part and shares burn through payTo and payInfos;
  honest payees are paid). Several pieces of evidence may name one pot (spec-1 reverted the second one).
- settleOne is unchanged: INVALID burns the whole pot; otherwise the M1 split.
- punish(id, reporter), 68 bytes: BadCalldata unless reporter == tx.origin. The bond branch calls
  Escrow.burnBond(bond, reward), where reward is the reporter record's reward address (w1 bits 0..159, tomb or not, so
  a self-report is paid) or zero (no reporter, or no record). The PAID branch is unchanged (tomb only, no bond).
- Escrow.burnBond(amount, reward), Registry-only, 68 bytes: cut = amount * rho_bp / 10^4 when reward is
  nonzero (CP 58 since FN-2b; 63 before), paid by PAY directly to the reward address (not into an ACCRUING reporter's
  bond); a failed PAY burns it;
  burnedTotal += amount - cut. The Escrow invariant holds with the same terms: held_total falls by the bond, burnedTotal
  rises by the bond less the cut that leaves Escrow.
- The spec-1 selectors punish(address), invalidate(uint256) and burnBond(uint256) do not exist in the v3 programs.

SYS footprint (resources::derive on CP schema v2): pending pots keep three words, so slots and keys are unchanged;
a block with evidence touches one more account (the proposer's reward address receiving the cut). SYS gas, measured
at the GAS_DOMAIN maxima with P_MAX 64 (tests/v3_contracts.rs v3-preview `v3_sys_gas_and_caps_within_the_v3_envelope`):
recordResult 36,135, invalidate 8,515, punish 98,677 (a genesis member's never-written clock word, a middle active
record, the reporter's PAY and the chain's first burn; an ACCRUING accused with a bond and the first burn measures
86,701, since register writes its clock word), settle 6,313,113 (all P_MAX = 64 probationers live and at bond 0 filled
to B_reg and promoted in one settlement, the chain's first burn, 192 HELD payees: the earn-in accrual, first measured
here). Admission composes them from GAS_ENVELOPE_V3 (37,000; 9,000; 99,000; 6,314,000,
also for settleFinal) for a genesis running the v3 set, whose gas domain also requires P_MAX <= 64; every other entry
is GAS_ENVELOPE_EARNIN's (PoColSystemV3.sol is built on the earn-in programs, whose markClose and beginBlock measure
above GAS_ENVELOPE; the same test measures v3 beginBlock 573,516 and markClose 30,889 within it). markClose took the
earn-in entry until V3-PR-E4 gave it a v3 entry (2,735,000, its closing-phase refunds; see the V3-PR-E4 section below);
beginBlock still takes the earn-in entry. Runtime sizes: MinerRegistry 10,209 bytes, RewardEscrow 5,218 bytes. FN-2b
re-measured every entry without the probation pool (section "v3 removal at a full set" below): settle 11,956,401 with
every payee ACCRUING and held_total 0 at the first accrue (11,939,301 with a bond held), recordResult 36,149, invalidate
8,537, punish 98,654.

The independent model is tests/v3_model.py (class Chain; vectors tests/v3/evidence-penalty-vectors.json, whole blocks
with settlement); five planted mutants (reporter-paid-on-fallback, whole-pot-for-bonded-accused,
accruing-read-at-evidence, accruing-read-at-settle, rho-on-pot-not-bond) are in tests/v3/system-v3-mutants.json.

## v3 late share credit (PoColSystemV3.sol only; V3-02, V3-PR-E3)

Applies to the v3 program set only, as above; the spec-1 programs, host calls and phase order are unchanged.

| Location | Change |
|---|---|
| ShareLedger total[j], count[j][a], owners[j] | no new slot: late shares raise the same count and total, and a new owner is pushed to the same list |
| MinerRegistry record w0 bits 88..151 | lastShare, now also raised (never lowered) by recordLate |

Host (apply.rs, v3 program set only), phase 0: recordShares(h-1), recordResult(h-1), then the evidence in body order,
where a kind-3 item (chain.rs `late_check`: RLP([3, be64(j), [n_1<...<n_k]]), validated against template j on the
carrying branch) becomes `recordLate(j, owners)` with one owner per late share in nonce order; then markClose,
pruneShares, settle(h-D), beginBlock. A carrier at h = j + D therefore still credits j before settle(j).

- MinerRegistry.recordLate(uint64 j, address[] owners), SYSTEM only, exactly selector||j||0x40||N||N words (N >= 1,
  N <= S_max): one nested CALL to ShareLedger.recordLate(j, owners), then for every owner with a record
  lastShare = max(lastShare, j) (a later header share already moved it past j: no revert, unlike recordShares). An
  owner without a record is skipped (its four words must be zero: BadState otherwise), as recordShares does.
- ShareLedger.recordLate(j, owners), Registry only, the same calldata shape: BadPhase unless 1 <= j < h and
  j + D >= h; BadState unless total[j] != 0 (the header shares of j are recorded) and total[j] + N <= kappa*m (the
  no-dilution cap: q_j = max(total, kappa*m) stays kappa*m, so late credit fills only the uncovered part of the pool and
  no header share loses value). count[j][a] += 1 per share (a new owner is pushed to owners[j]); total[j] += N.
- Settlement is unchanged (entriesOf, distribute): late owners are paid like header owners; an absent record's part
  burns (payBurn), a tomb's (penaltyBurn), an ACCRUING record's fills its bond (EARN-IN P1).

SYS footprint (resources::derive on CP schema v2): owners per height stay within O = min(S_max + 1, M_max) (late credit
applies only while total[j] < kappa*m <= S_max/4), so the slots are unchanged and pruneShares deletes late owners as
before; a kind-3 item writes at most 3k + 2 keys for k shares, and a block credits at most L = min(E_max * min(S_max,
kappa*m - 1), D * (kappa*m - 1)) late shares, so the evidence keys are at most 9 * E_max + 3 * L; no account is added (recordLate pays
nobody). SYS gas (tests/v3_contracts.rs v3-preview `v3_sys_gas_and_caps_within_the_v3_envelope`, chain C): one call
crediting kappa*m - 1 = 63 shares to distinct owners new to owners[j], as the block's first call: 3,457,157 with owners
without a record (four cold words read each), 3,228,593 with recorded owners whose lastShare it writes. Admission
composes GAS_ENVELOPE_V3 recordLate 3,458,000 for a genesis running the v3 set (the one v3-only call name, appended to
the GAS_ENVELOPE_V3-over-GAS_ENVELOPE_EARNIN composition), counting each of the E_max evidence
items as the dearer of recordLate and invalidate + punish; the v3 gas domain also requires kappa*m <= 64. Runtime
sizes: MinerRegistry 10,734 bytes, ShareLedger 3,478 bytes (RewardEscrow unchanged, 5,218).

The independent model is tests/v3_model.py (Chain._record_late; vectors tests/v3/late-share-vectors.json, whole blocks
with settlement and the ledger compared per height); five planted mutants (late-last-share-reverts,
late-last-share-overwrites, late-cap-removed, late-window-one-wider, late-total-not-raised) are in
tests/v3/system-v3-mutants.json. The host's kind-3 validity is modelled by `late_verdict` (vectors
tests/v3/late-share-validity-vectors.json, replayed by pocold/tests/v3_late_shares.rs), with 17 planted mutants.

## v3 issuance, carry and tip split (PoColSystemV3.sol only; V3-19, V3-15, V3-20, V3-PR-E2a)

Applies to the v3 program set only, as above; the spec-1 programs keep closeBlock(tips) with CP 15 every block.

| Location | Change |
|---|---|
| RewardEscrow slots 4 and 5 | carry (new), lagged by one block (D-V3-15): slot 4 + (t & 1) holds the carry into pot t, the base burn of the pots settled in block t - 1; between blocks h and h + 1 only the carry into pot h + 1 can be nonzero |
| RewardEscrow slot 2 | not used (an earlier draft's unlagged carry) |
| RewardEscrow pending[j] | unchanged layout; k=0 amount = issued + (1 - gamma_tip)·tips + carry into pot j |

Host (apply.rs, v3 program set only), phase 2 of a non-final block h: credit issued = issuance(h) to Escrow
(lp4-execution issuance.rs: h == 0 ? 0 : max(k >= 256 ? 0 : S0 >> k, S_tail), k = h / H_half; S0 = CP 15, H_half =
CP 52, S_tail = CP 53), then `closeBlock(tips, issued)`. A final block calls finalize(tips) and settleFinal(j) as before.

- closeBlock(uint256 tips, uint256 issued), SYSTEM only, 68 bytes: BadPending if pending[h] exists. share =
  floor(tips · gamma_tip_bp / 10^4) (CP 56) for a signed block (tx.origin nonzero), 0 for a fallback block;
  pending[h] = {issued + tips - share + carry into pot h, kind/proposer as before, winner 0}; that carry (slot 4 + (h & 1))
  is zeroed. The share then goes
  to tx.origin through payTo, as the proposer part does: a HELD record's reward address is PAID, an ACCRUING record's bond
  takes it first (EARN-IN P1: promoted on reaching B_reg, the excess PAID), an absent record, a tomb (a proposer that
  reported itself in its own block) or a failed PAY burns it.
- settleOne / distribute in block h: the base burn R - A (Q = max(C, kappa·m)) is added to the carry into pot h + 1
  (slot 4 + ((h + 1) & 1)) instead of burnedTotal, never to the pot block h closes; roundBurn A - Σp_i, payBurn,
  penaltyBurn and a whole-pot (INVALID) burn still burn. Once slot 1 is set (the final block's settleFinal), the base burn
  burns.
- finalize(tips): burnedTotal += tips + both carries (the carry into pot F, which block F - 1's settle left and no pot
  receives, and the carry of F's own phase-0 settle(F-D)), both slots zeroed, slot 1 = F.
- carry() view: slot 4 + slot 5, every outstanding carry.

Pot h is fixed by block h: its carry is the base burn of settle(h - 1 - D) in block h - 1's phase 0, which runs after
every late credit to h - 1 - D (recordLate precedes settle; the carriers are blocks h - D .. h - 1), so changing
late-share inclusion after block h - 1 never changes pot h, neither the amount closeBlock(h) records nor the amount
settle(h) later distributes (tests/v3_model.py `check_pot_fixed_by_its_block`, tests/v3_contracts.rs
`pot_j_plus_1_is_fixed_by_block_j_plus_1`; both fail on the planted base-burn-into-closed-pot, which adds half of a
settle's base burn to an already-closed pot).

Why the lag (D-V3-15, the review's incentive finding): block h is the last block that can carry late shares of
template h - D (window [max(1, h - D), h - 1]). Had pot h received settle(h - D)'s base burn, h's signed proposer would
have gained gamma_bp/10^4 (plus its own winner and pool fraction of pot h) of every late share of h - D it left out
while C < kappa·m. With the lag that value reaches pot h + 1, whose block can no longer carry late shares of h - D, so
the last carrier gains nothing in its own pot (tests/v3_model.py `check_last_carrier_gains_nothing`, tests/v3_contracts.rs
`last_carrier_gains_nothing_by_omitting_late_shares`; both fail on the planted carry-not-lagged, the unlagged rule).

Escrow invariant (apply.rs `check_escrow_invariant`, v3 set): balance >= Σpending + held + burnedTotal + both carries;
after phase 2 of a non-final block h the carry into pot h is zero and the carry into pot h + 1 is zero while h <= D (no
settlement ran yet); after a final block both are zero ("carry left after phase 2" otherwise). The model and the contract
harness check the equality after every SYSTEM step, and the v3-preview host test after every block.

Supply bound (genesis_builder.rs and lp1-node gate.rs R23, for a genesis running the v3 set; admission.rs sets the
gate's `m3_issuance` from the sysCodeHash): allocations + genesis bonds + 2·S0·H_half + S_tail·2^64 < 2^127, the M3
schedule summed over every height (issuance.rs module doc). Every other program set keeps CP 15 · H_END (u64::MAX blocks
when continuous) on either CP schema.

SYS footprint (resources::derive on CP schema v2): one more Escrow slot (the carry into the next pot outlives the block)
and two keys for the carry in every phase class (settle writes the carry into pot h + 1, closeBlock or finalize zeroes
the carry into pot h); on normal and closing blocks four keys for the tip share's payTo (bond, state word, held_total,
burnedTotal; five with ACC_COUNT until FN-2b) and the proposer's reward address as an account even without evidence. SYS gas
(tests/v3_contracts.rs v3-preview): closeBlock at its worst 132,825 (a nonzero carry into pot h zeroed and a 21 PCOL tip
share to an ACCRUING proposer at bond 0: accrue fills and promotes it, the excess PAID, while held_total is zero, so its
store goes from zero to nonzero; the same promotion with held_total nonzero 115,725, a partial fill 82,620, a HELD
proposer 76,871, a tomb 73,923, a fallback block 52,951); finalize 61,932 (both carries nonzero, burned with the tips as
the chain's first burn); settle 6,311,582 on the 256-payee path (C >= kappa·m, no carry write; a carry write needs
C < kappa·m, so at most 63 payees: 134,192 measured; a settle accruing from held_total zero has no HELD payee: 162,150).
Admission composes GAS_ENVELOPE_V3 closeBlock 133,000 and finalize 62,000 (settle and settleFinal keep 6,314,000).
Runtime size: RewardEscrow 5,591 bytes (MinerRegistry 10,734 and ShareLedger 3,478 unchanged).

The independent model is tests/v3_model.py (issuance, Chain.close/finalize; vectors tests/v3/issuance-carry-vectors.json:
the schedule table, a compressed H_half = 4 chain over 6·H_half, the tip split for HELD, fallback, ACCRUING and tombed
proposers, C from 1 to S_max + 1 with late credits, a random chain, the late-inclusion pair, a final block with both
carries outstanding, the last-carrier pair), with eight planted model mutants (ECON_MUTANTS) each failing it; tests/issuance_oracle.py --v3 checks the schedule a second time;
eight planted RewardEscrow mutants (carry-not-added, tip-share-to-pot, base-burn-still-burned, issued-ignored-flat-s0,
carry-kept-at-final, base-burn-into-closed-pot, carry-not-lagged, lagged-carry-kept-at-final) are in
tests/v3/system-v3-mutants.json.

## v3 removal at a full set (PoColSystemV3.sol only; FN-2b, FRESH-NETWORK design C2-C5)

Owner decisions 10b, 10c and Q2(a) of 2026-11-13 (development/decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)
supersede V3-10 (contestable seats), V3-14 (the probation pool and its contest) and the T_PROB deadline. FN-2b deletes:
registerContest, contestStatus, contestThreshold, epochTotals, accruingCount; the epoch counters (w0 bits 216..255) and
totals (slot 10); POOL_OUT (w0 bit 87); ACC_COUNT (slot 8, never written); the probation deadline (the beginBlock
sweep's probation exit, topUp's and reactivate's deadline checks); and the CP schema v2 fields P_MAX, T_PROB, E_epoch,
theta_bp and k_sigma (with FN-2a's F0_*, the schema has 59 fields: H_half 52, S_tail 53, gamma_tip_bp 54, baseFeeMin
55, R_react 56, C_react 57, rho_bp 58; ChainCaps reads n < 59). ACCRUING records count within M_max like any other,
keep the earned bond (EARN-IN P1: settlement fills it before paying) and the B_reg/4 draw rule, and never exit by time;
recordShares is one pass again (lastShare = j per distinct owner) and recordLate keeps lastShare = max(lastShare, j).

Removal rule (internal `removal(vr, vw, height)`, codes in order):

| Code | Meaning |
|---|---|
| 0 | removable |
| 1 | the victim is not a live active record (absent, tomb, closing or exited) |
| 2 | the set is not full: active < M_max (below a full set nobody is removed) |
| 3 | no more than M_min active |
| 4 | a credited share or a (re)activation within Theta_inact: height < max(lastShare, graceStart) + Theta_inact (CP 25) |

- removeInactive(victim): as before (BadState unless the victim is active and more than M_min are active), and now
  BadState unless the code is 0, so it reverts below a full set.
- registerReplacing(reward, victim), payable 0..B_reg like register, exactly 68 bytes with two clean address words
  (BadCalldata), BadValue above B_reg, Closing, then RegOpsLimit with two REG_OPS units (a removal and a registration,
  less the maintenance used) and one R_max entry, BadPhase before the block's beginBlock, BadState when the sender has a
  record, then the removal code of the victim (1: BadState; 2..4: NotRemovable). The victim exits through exitAt as by
  deregister: status 2, exitHeight = h, dueHeight = h + U, its record and bond kept, so its parts of pots not yet
  settled still pay it (or fill its bond) and evidence can still punish it until the refund (sweep or claimBond at
  dueHeight, no burn). The sender then registers exactly as register(reward) into the freed seat (capacity, R9 / PAY-02,
  the bond into Escrow; below B_reg an ACCRUING record). slot3 gains two userOps and one R_max entry. This is not a
  contest: no performance measure, no threshold; it makes atomic two operations anyone may already do in one block.
  register alone still reverts Capacity at a full set; a victim another sender already removed reads code 1.
- The removed member may return: it reactivates only while a seat is free (Capacity at a full set), or registers
  again after its refund (with B_reg: HELD at once), into another removable seat when the set is full.

Views: removalStatus(victim) returns the code at block.number (pocold's wallet and its RPC run it on the head state at
the next height); activeSlice(start, count) returns (ids, w0): ActList[start, min(start + count, active)) and each one's
record word w0, empty when start >= active, safe for any count (C5: the reward gateway's paged reads at M_max 10,000).
Neither writes anything.

Write ceilings and gas (tests/v3_contracts.rs replacing_write_ceilings_gas_and_user_sys_keys; a slot written back to its
value is unchanged): victim in the middle of the active list with a full-bond newcomer 13 slots (217,381 gas), with a
zero-bond newcomer 11 (186,725), victim at the tail with a full bond 11 (206,815); each within half the wallet's
REPLACE_GAS 500,000 and SYS_KEYS_USER_MAX 256. Runtime size: MinerRegistry 11,580 bytes (EIP-170: 24,576;
tests/v3_contracts.rs the_v3_programs_fit_eip_170 and the CI size check).

SYS gas (admission.rs GAS_ENVELOPE_V3, tests/v3_contracts.rs v3-preview): with up to M_max members ACCRUING, every payee
of a pot may fill and promote its bond in settlement. Pot j's payees are at most S_max + 3 (its distinct share owners,
its winner and block j's proposer), none counted by M_max, so the v3 gas domain drops the M_max term and requires
S_max + 3 <= 259 (`in_gas_domain`); no v3 SYSTEM call iterates the members. settle with 258 payees all ACCRUING at
bond 0, promoted in one call as the chain's first burn with held_total 0 at the first accrue: 11,956,401
(`v3_settle_gas_with_every_payee_accruing`, M_max 1,000; 11,939,301 with a bond held); the carry path (C < kappa*m, 64
payees promoted) 3,022,717; recordShares with 257 owners without a record 14,199,448; markClose's closing refunds
2,683,610 (no v3 entry: within the earn-in entry). SYS footprint (resources::derive): Registry fixed slots 6,
recordShares 3·O + 2 keys, the tip share's payTo 4 keys, settle and each settleFinal 4 + 2·(O + 2) + 1 keys (each
payee's bond and state word through accrue, and held_total); a profile that never closes counts only the ordinary
class. activeSlice reads two cold slots per element: a page of 512 measures 2,741,170 gas at 10,000 members, so the
reward gateway sends each page with 30,000,000 gas (pocold's CALL_GAS_MAX), not its ordinary 2,000,000.

The independent model is tests/v3_model.py (Registry.removal_check, register_replacing, remove_inactive; Chain with
shares and late credit; check_payee_bound asserts at most S_max + 3 payees per settlement of every host chain); its
scenarios are in tests/v3/registry-budget-vectors.json: removal only at a full set (code 2 and removeInactive's BadState
below it, Capacity at it, code 4 in grace, two newcomers on distinct victims in one block, a third naming an exited
victim, a removeInactive freeing a seat a plain register takes, the victims refunded after U, one punished while exited,
both returning, one with B_reg as HELD and one with nothing as ACCRUING), the budgets, M_min, whole chains where header
and late shares keep a member (and the same chain without the late credit), and a victim payable and punishable until
its refund; tests/v3/earnin-on-v3-vectors.json re-runs the earn-in model's scenarios (its probation-seats and deadline
scenarios differ on v3: no pool, no deadline). Nine planted MinerRegistry mutants (removal-below-full-set,
removal-theta-off-by-one, removal-ignores-grace, removal-at-m-min, replacing-one-reg-ops-unit, replacing-no-r-max-entry,
replacing-victim-erased-at-once, replacing-any-victim, replacing-sender-with-record: without registerReplacing's own
"sender has no record" check, enroll(..., w = 0) would write over the sender's record) are in tests/v3/system-v3-mutants.json, each failing its scenario
(tests/v3_contracts.rs the_removal_mutants_fail_the_v3_vectors). The V3-PR-E4 vectors (contest-vectors.json) and the
earn-in model at P_MAX 64 (earnin-vectors-p64.json, tests/v3_model_vectors.py) were deleted with the rules they tested.
