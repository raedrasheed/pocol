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
| w0 | earn-in layout, plus lastMaintHeight mod 2^40 at 216..255 |
| w1 | reward address 0..159, lastReactHeight 160..223; remaining zero |
| slot3 | as above, but userOps0..15 counts register/deregister/claimBond/removeInactive only and 16..23 register only |

Budgets per block (the user-path prefix, after BadValue and Closing, before BadPhase):
- setRewardAddress and topUp (maintenance): RegOpsLimit when maintenance used >= REG_MAINT_MAX = 8 (program
  constant); after the record is found live, RegOpsLimit when lastMaintHeight == height mod 2^40 (one maintenance
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
(vector maintenance-clock-per-record).

slot9 is valid only for the height it carries; any other height reads as nothing used, so beginBlock writes nothing
new and the sweep, markClose and SYSTEM paths are unchanged. lastReactHeight is a full height. lastMaintHeight keeps
40 bits (the spare w0 bits): it can only refuse, never admit, so a maintenance operation exactly k*2^40 blocks after
the identity's previous one (or at such a height, for one with none) reverts once; at T_blk 30 s that is about a
million years. setRewardAddress keeps w1's clock; recordWords returns w1 as the reward address alone and
recordClocks(id) returns (lastMaintHeight mod 2^40, lastReactHeight). The host reads only bits it masks (apply.rs
miner_record, Escrow via bondRecord/payInfos), so the clocks change no host reading. ParamGate R8 on CP schema v2
requires U >= W_act + 2 (the window holds at least two blocks) and C_react >= U (lp1-node/src/gate.rs).

SYS footprint (R18, resources::derive on CP schema v2): Registry fixed slots 7 (0..4, 8, 9), four words per record
as before (no fifth word; erase clears the clocks with w0/w1), so SYS_SLOT_MAX grows by two slots plus the 15
appended CP slots. A fifth record word was measured and rejected: zeroing it on erase put the worst beginBlock sweep
(64 scans, 16 refunds of ACCRUING records) at 930,080 gas, above admission's GAS_ENVELOPE beginBlock 926,000; with
the spare bits it is 864,902, as before the change (tests/v3_contracts.rs v3_sweep_deletions_keep_the_sys_envelope).

Changed Registry slots of successful user calls, earn-in -> v3 (tests/v3_contracts.rs v3_registry_write_ceilings,
wallet_bond_features_keep_their_write_ceilings_and_gas_on_the_v3_set): register10 -> 10; reward2 -> 3 (w0, w1,
slot9); deactivate middle7/tail5 -> 7/5; reactivate5 -> 6 (w1 clock; slot9 for slot3); claim middle10/tail8 -> 10/8;
topUp 3 -> 4 (w0 clock; slot9 for slot3); topUp with promotion 5 -> 5; partial register 11 -> 11; accrue 2/4 -> 2/4.
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
is GAS_ENVELOPE_EARNIN's (PoColSystemV3.sol is built on the earn-in programs, whose markClose and beginBlock measure
above GAS_ENVELOPE; the same test measures v3 beginBlock 573,516 and markClose 30,889 within it). Runtime sizes: MinerRegistry 10,209 bytes, RewardEscrow 5,218 bytes.

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
