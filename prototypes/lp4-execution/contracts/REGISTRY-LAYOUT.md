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
unchanged. Earn-in already uses w0 bits 152..215 (registeredAt) and slot 8 (ACC_COUNT).

| Location | Bit ranges, low first |
|---|---|
| slot9 (budgets) | maintenance used 0..7, reactivations used 8..15, budgetHeight 16..79; remaining zero |
| w0 | earn-in layout (bits 216..255 hold the V3-10 epoch counters since V3-PR-E4, below) |
| w1 | reward address 0..159, lastReactHeight 160..223, lastMaintHeight mod 2^32 224..255 (moved from w0 bits 216..255, mod 2^40, by V3-PR-E4) |
| slot3 | as above, but userOps0..15 counts register/deregister/claimBond/removeInactive only and 16..23 register only |

Budgets per block (the user-path prefix, after BadValue and Closing, before BadPhase):
- setRewardAddress and topUp (maintenance): RegOpsLimit when maintenance used >= REG_MAINT_MAX = 8 (program
  constant); after the record is found live, RegOpsLimit when lastMaintHeight == height mod 2^32 (one maintenance
  operation per identity per block). They no longer touch slot3.
- register, deregister, claimBond, removeInactive: RegOpsLimit when userOps + maintenance used >= REG_OPS (CP 23);
  register also when its counter >= R_max (CP 17).
- reactivate: RegOpsLimit when reactivations used >= R_react (CP 61); it no longer touches slot3 or R_max. Valid
  only when exitHeight + W_act <= height < dueHeight (strictly before the refund sweep at dueHeight = exitHeight + U),
  and, when lastReactHeight != 0, height >= lastReactHeight + C_react (CP 62); otherwise BadState.

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

SYS footprint (R18, resources::derive on CP schema v2): Registry fixed slots 7 (0..4, 8, 9), four words per record
as before (no fifth word; erase clears the clocks with w0/w1), so SYS_SLOT_MAX grows by two slots plus the 15
appended CP slots. A fifth record word was measured and rejected: zeroing it on erase put the worst beginBlock sweep
(64 scans, 16 refunds of ACCRUING records) at 930,080 gas, above admission's GAS_ENVELOPE beginBlock 926,000; with
the spare bits it is 864,902, as before the change (tests/v3_contracts.rs v3_sweep_deletions_keep_the_sys_envelope).

Changed Registry slots of successful user calls, earn-in -> v3 (tests/v3_contracts.rs v3_registry_write_ceilings,
wallet_bond_features_keep_their_write_ceilings_and_gas_on_the_v3_set): register10 -> 10; reward2 -> 2 (w1 with its clock,
slot9; 3 before V3-PR-E4 moved the clock out of w0); deactivate middle7/tail5 -> 7/5; reactivate5 -> 6 (w1 clock; slot9
for slot3); claim middle10/tail8 -> 10/8; topUp 3 -> 4 (w1 clock; slot9 for slot3); topUp with promotion 5 -> 6 (w1
clock and w0 promotion; 5 before V3-PR-E4); partial register 11 -> 11; accrue 2/4 -> 2/4.
The beginBlock sweep changes the same slots per deletion as earn-in. Gas: register +5,225 (cfg(P_MAX) and the cold
slot9 read), within the wallet's EARNIN_REGISTER_GAS 400,000; every other user path below 200,000 (REGISTRY_GAS
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
- Escrow.burnBond(amount, reward), Registry-only, 68 bytes: cut = amount * rho_bp / 10^4 (CP 66) when reward is
  nonzero, paid by PAY directly to the reward address (not into an ACCRUING reporter's bond); a failed PAY burns it;
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
of GAS_ENVELOPE applies unchanged. Runtime sizes: MinerRegistry 10,209 bytes, RewardEscrow 5,218 bytes.

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
composes GAS_ENVELOPE_V3 recordLate 3,458,000 for a genesis running the v3 set, counting each of the E_max evidence
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
  floor(tips · gamma_tip_bp / 10^4) (CP 59) for a signed block (tx.origin nonzero), 0 for a fallback block;
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
the carry into pot h); on normal and closing blocks five keys for the tip share's payTo (bond, state word, ACC_COUNT,
held_total, burnedTotal) and the proposer's reward address as an account even without evidence. SYS gas
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

## v3 contestable seats and the probation-pool contest (PoColSystemV3.sol only; V3-10, V3-14, V3-PR-E4)

Applies to the v3 program set only, as above; the spec-1 programs, host calls and phase order are unchanged. O-10 = b and
O-14 = b (V3-DESIGN.md, V3-DECISIONS.md D-V3-10/D-V3-14): E_epoch, theta_bp and k_sigma are CP schema v2 fields 63..65
(one day, 1250, 3 in the draft v3 CP). No host code changes: registerContest is a user transaction, and the host reads
only the bits it masks (apply.rs miner_record).

| Location | Bit ranges, low first |
|---|---|
| w0 | earn-in layout, plus countCur 216..235 and countPrev 236..255 (V3-10); lastMaintHeight moved to w1 |
| w1 | reward address 0..159, lastReactHeight 160..223, lastMaintHeight mod 2^32 224..255 |
| slot10 (epoch totals) | epoch 0..63, its total 64..159, the previous epoch's total 160..255 |

Epoch counters. A credited share counts in the epoch of its template j, j / E_epoch (a late share of j counts where a
header share of j would). A record's counters describe the epoch of its lastShare (w0 88..151), so no epoch index is
stored: countCur is the shares credited in epoch lastShare / E_epoch and countPrev those of the epoch before. Crediting n
shares of j (recordShares per distinct owner with its count, from Ledger.entriesOf; recordLate per share) sets lastShare
= max(lastShare, j) and: j in the record's epoch, countCur += n; a later epoch, countPrev = countCur when it is the next
one (else 0) and countCur = n; the epoch before (a late share older than lastShare's epoch), countPrev += n; older (only
when E_epoch < D), nothing. Each count saturates at 2^20 - 1 (at T_blk 30 s and S_max 256 a member credits at most
2,880 * 257 = 740,160 shares a day, below it); a saturated count is never contestable. slot10 rolls the same way over
every credited share (header and late, owners with a record or not); its totals cannot overflow 96 bits (at most S_max
+ 1 + E_max * S_max shares per block). The counters live in the w0 word recordShares and recordLate already write, so
they add no SYS key per owner; erase clears them with w0 and register writes them as zero, so a record never inherits a
previous incarnation's counts. recordShares reverts on lastShare > j as before; recordLate never reverts on it.

registerContest(address reward, address victim), payable 0..B_reg, 68 bytes, dispatched first in the fallback. Prefix as
the other user paths: BadCalldata (length, dirty address words), BadValue (> B_reg), Closing, RegOpsLimit (REG_OPS less the
maintenance used must hold two units, a removal and a registration, and R_max one entry), BadPhase. Then BadState if the
sender has a record, then the rule (contestCheck, codes in order; code 1 or 3 reverts BadState, 2 and 4..7 NotContestable):

| Code | Refusal |
|---|---|
| 1 | the victim is not a live active record (no record, a tomb, a PAID or BURNED bond, or exited) |
| 2 | nothing to contest: the set is not full, active < M_max, for a pool contest too (O-10=b: contests only when all seats are full) |
| 3 | the pool contest's victim is not ACCRUING |
| 4 | no measured epoch yet (height < E_epoch), or the late window of its last template still open (height mod E_epoch < D - 1) |
| 5 | the victim is in grace: height < graceStart + Theta_inact (graceStart = registration or last reactivation, w3 192..255) |
| 6 | the victim registered or reactivated after the first block of the measured epoch: graceStart > p * E_epoch |
| 7 | count >= threshold, or the count is saturated |

The measured epoch p is the previous one, height / E_epoch - 1. The count is countCur when the victim's epoch
(lastShare / E_epoch) is p, countPrev when it is p + 1, else 0; Tprev likewise from slot10. The threshold decides
count < E - k_sigma * sqrt(E), E = theta * Tprev / active (theta = theta_bp / 10^4, active = the active count now, which
code 2 has required to be M_max: exits just before a contest cannot raise E, nor bonded joins lower it), exactly
over the reals: with N = theta_bp * Tprev, Dn = 10^4 * active and s = isqrt(k_sigma^2 * N * Dn) (floor; the
initial estimate 2^(floor(log2 x) / 2), seven Newton steps, then min(y, x / y), OpenZeppelin's Math.sqrt method), the
real threshold is (N - sqrt(k^2 N Dn)) / Dn, and since ceil(N - x) = N - floor(x) the integer threshold is ceil((N - s) /
Dn), 0 when N <= s. An integer count is below the real threshold iff it is below the integer one, so there is no rounding
error in either direction (tests/v3_model.py `threshold` and `below_exact`, the threshold table). At the owner's values,
E = 45 (active 256): threshold 25.

Code 4's late-window condition keeps the statistic final when it is judged: every late share of the measured epoch is
credited by height (p + 1) * E_epoch + D - 1 (block j + D is the last carrier of j), so omission followed by a contest
before the carriers ran cannot manufacture a victim (V3-02 is the prerequisite of V3-10). It requires E_epoch >= D for any
contest to be possible.

Effects. Pool contest (the set full, a probationer newcomer and ACC_COUNT >= P_MAX, the victim ACCRUING): the victim leaves the active
list, its accrued bond is refunded (Escrow.releaseBond; an undeliverable refund reverts PaymentFailed), held_total and
ACC_COUNT fall as in refund, and the record is erased now, so the newcomer takes its probation seat and its active seat
in the same call. Pots still pending then (the victim's shares, winner or proposer parts of the last D templates, and late
shares of them credited afterwards) settle to an id without a record: payInfos names no reward address, so its parts burn
(payTo returns them for payBurn) instead of filling the bond it was refunded (vector pool-victim-pending-pots-burn: the
burn exceeds the same chain without the contest by exactly what the victim's bond would have accrued). If the victim
registers again within D blocks, those parts go to its new record. Seat contest (otherwise, the set full as well): the victim exits as by deregister
(status 2, exitHeight = h, dueHeight = h + U, no M_min check: the newcomer takes the seat at once), keeps its bond and
evidence exposure until dueHeight, may reactivate inside the usual window, and is refunded by the sweep or claimBond.
Then the sender registers exactly as register(reward) (capacity, R9 / PAY-02, P_MAX, the bond into Escrow). slot3 gains
two userOps and one R_max entry.

Views: contestStatus(victim, probation) returns (code, count, threshold) at block.number, the rule registerContest
applies; contestThreshold(tprev, active) returns (threshold, s) for the CP's theta_bp and k_sigma (active 0 reverts);
epochTotals() returns slot10's three fields. pocold's wallet (quote_contest) runs contestStatus on the head state at the
next height and explains each code before anything is signed.

Write ceilings and gas (tests/v3_contracts.rs contest_write_ceilings_gas_and_user_sys_keys; a slot written back to its value
is unchanged): seat contest, victim in the middle of the active list 13 slots (232,451 gas), at the tail 11 (221,871);
pool contest, victim at bond 0 in the middle of both lists 12 (189,028), a 5 PCOL victim at the tail 11 (181,430); the
user-context SYS keys equal these counts (Registry only), within the devnet SYS_KEYS_USER_MAX 256. The wallet quotes
CONTEST_GAS 500,000. Other user paths: register 205,207 gas (the internal enroll it now shares with registerContest),
every other path below 85,000.

SYS footprint and gas (resources::derive on CP schema v2): the fixed Registry slots grow to 8 (slot10); recordShares writes
one more key (slot10), recordLate one more per item (within an evidence item's nine). SYS gas (tests/v3_contracts.rs
v3-preview `v3_record_shares_gas_with_the_epoch_counters`, `v3_sys_gas_and_caps_within_the_v3_envelope`): recordShares
naming 257 distinct owners without a record as the chain's first call 14,388,687 (256 recorded owners 13,628,815, an epoch
roll 13,626,629), above the spec-1 entry 13,216,000, so admission composes GAS_ENVELOPE_V3 recordShares 14,389,000 for a
genesis running the v3 set; recordLate 3,465,613 on its stranger path (GAS_ENVELOPE_V3 3,466,000); the larger Registry
dispatch adds a few gas to every nested Registry call: settle 6,314,420 on its 64-fill path (GAS_ENVELOPE_V3 settle and
settleFinal 6,315,000), punish 98,699, closeBlock 132,869 (entries unchanged). The beginBlock sweep is unchanged (865,273
at the program caps). markClose's closing phase (`v3_closing_mark_close_gas_within_the_envelope`: RET 128 refunds a call,
each through Escrow.releaseBond and the larger dispatch, an ACCRUING one also writing ACC_COUNT) measures 2,734,171 for 64
HELD and 64 ACCRUING refunds (128 HELD 2,711,375), above the spec-1 entry 2,655,000: GAS_ENVELOPE_V3 markClose 2,735,000.
Runtime sizes: MinerRegistry 13,969 bytes (from 10,734; below 24,576), RewardEscrow 5,591,
ShareLedger 3,478.

The independent model is tests/v3_model.py (Registry.credit, credit_total, contest_check, register_contest; vectors
tests/v3/contest-vectors.json: the seat contest with the refund after U; refusals above the threshold, by a registered
sender, of an exited victim, with the set not full; the threshold off by one; grace; joined or reactivated after the
start of the measured epoch; the budgets; the pool contest and its refusals (the set not full included); exits in the
contest block that cannot turn a refused pool contest into one that lands, and the same chain without them; a pool
victim with pending pots (they burn), and the same chain without the contest; both full; a probationer newcomer's seat
contest of an ACCRUING victim; seat victims (HELD and ACCRUING) punished between the contest and their due height, before
and after the sweep; a rotation race in the pool; ACC_COUNT through promote, exit, displace and punish; late credit
counted, and the same chain without it; the counters rolling at epoch boundaries; an epoch shorter than D; the threshold
table). Thirteen planted MinerRegistry mutants (contest-threshold-off-by-one, contest-grace-ignored,
contest-joined-in-epoch-allowed, contest-set-not-full-allowed, pool-contest-set-not-full-allowed (the first draft's pool
contest on a set that is not full), contest-late-window-open, pool-contest-refund-skipped,
pool-contest-exits-instead, pool-contest-held-victim, counters-not-rolled, late-not-counted, isqrt-not-floored,
contest-one-reg-ops-unit) are in tests/v3/system-v3-mutants.json; the E1 mutant maint-twice-per-block and the E3 mutants
late-last-share-reverts/-overwrites are re-patched onto the moved clock and the credit call.

Open points the design leaves to the owner (not decided here): a pool contest erases its victim at once, as the design
says, so an ACCRUING member that has just committed an offence within E_win escapes the evidence penalty if it is
contested before any block carries the evidence, and an honest victim's parts of its pending pots burn (above; the seat
contest keeps the victim exited, payable and punishable until dueHeight); a pool contest is allowed only with the set
full, the literal reading of O-10=b and of the roadmap ("registerContest only when active == M_max"), so a full pool
with free seats admits no contest (the owner may instead want pool contests on a set that is not full, which needs a
denominator the contestant cannot move: M_max, or the active count recorded for the measured epoch); and a member that exits
and reactivates (or rotates claimBond + register) once per epoch restarts its eligibility each time, as the design's
"registration or last reactivation" rule says, so it is never contestable.
