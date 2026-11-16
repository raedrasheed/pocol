# Contribution and rewards

These are the reward, registry and settlement rules of the v3 program set (`PoColSystemV3.sol`, system-v3.json), the
only set a genesis may name (specVersion 2, CP schema v2). The fresh-network values are in
[FRESH-NETWORK-CP.md](../../decisions/FRESH-NETWORK-CP.md); the decisions behind the rules are in
[the decision record](../../decisions/V3-DECISIONS.md). The registry encoding is
`prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md`.

**From the thesis**
- R_tot = subsidy+fees [PDF:103 (5.6)]; Σf = 1 [(5.7)].
- Equality incentivizes Sybil identities [PDF:103].
- Small rounding error is acceptable [PDF:104].
- Activity is not proof of search [PDF:105].
- Inactivity removal has no bond penalty [PDF:89, PDF:112].

**Share (D6)**
- A shareList item satisfies items 9 and 14, where T_share = min(2^256−1, target·m).
- The winning nonce is an implicit share; header counts include it.
- (D142) c_i, C_j and lastShare are derived in recordShares(j) from ParentResult(j) of block j on the executed branch. Settlement therefore depends on block j's own committed shareList, not the first copy seen by the miner or node.
- Shares sample work; they do not prove exhaustive search. κm = 64.
- pocol_submitShare is a local-only mining interface. A site cannot reach it through the bridge (D95), nor can any request carrying Origin.

**Late shares (D-V3-02)**
- Kind-3 evidence in a signed body credits valid ancestor shares a header left out, before their pot settles (consensus §1b). Owners come from the branch's assignment of template j, deduplication from the branch, and C_header(j)+C_late(j) ≤ κm. The winning nonce and shares already credited by the header or another carrier are excluded.
- recordLate(j, owners) runs in phase 0 after recordResult(h−1) and before settle(h−D); it raises count[j][owner] and C_j and sets lastShare = max(lastShare, j). The settle denominator stays max(C_j, κm), so late credit never dilutes header shares below the κm floor.
- Reorganizations restore the credit and the branch's deduplication. A node-local share cache only helps a proposer find omitted shares; it is not consensus evidence by itself.

**Evidence**

Type 2 (D29): RLP([2, TemplateID, nonce, shareRoot1, sig1, shareRoot2, sig2]).
- Valid if the roots differ, owner(nonce) signed both, an ancestor block exists at j ≥ h−E_win, and the accused has a record with tomb=0.
- Effect (D-V3-16, D-V3-17): in phase 0, invalidate(j, accused) for a pot still pending, then punish(accused, reporter). Pot j settles normally: the tombed offender's winner part, proposer part and shares burn through payTo and distribute, and honest contributors are paid. The whole pot is burned at settle(j) only if the accused is pot j's winner and was ACCRUING when recordResult(j) recorded it (WIN_ACCRUING: the fact is committed then, not read from later bond state). Several pieces of evidence may name one pot.

Type 1 is valid if:
- UT1 ≠ UT2, both signed at a=0 with the same (parentHash, h) and cfg identity.
- h ≥ h_incl−E_win.
- The signer is DrawIndex[h mod E_win].
- The accused has a record with tomb=0.

Its effect is punish(accused, reporter).

punish (D-V3-17): the reporter is the carrying block's signed proposer (zero on a fallback block). A live record's whole bond is burned, except that rho_bp/10^4 of it (25 % at rho_bp = 2500) is paid to the reporter's reward address, a self-report included; a fallback carrier has no reporter, so the whole bond burns. Self-reporting therefore still loses 75 %.

Evidence violating its conditions invalidates the containing block.

**Registry**
Rec[id] has four words:
- w0: status, tomb, bondState (HELD 1, BURNED 2, PAID 3, ACCRUING 4), recPos, actPos, lastShare, registeredAt (bits 152..215, written by register and never changed).
- w1: rewardAddr, lastReactHeight (bits 160..223) and lastMaintHeight mod 2^32 (bits 224..255).
- w2: bond.
- w3: exitHeight, dueHeight, burnHeight, graceStart.

Registry slot 9 holds this block's maintenance and reactivation budgets (valid only for the height it carries). M_0 members are written in genesis (D79), each HELD with bond = B_reg.

A record is live when status ≠ 0, tomb = 0 and bondState ∈ {HELD, ACCRUING}.

**Authorization (D62)**
- id = msg.sender in all functions except permissionless removeInactive and the victim of registerReplacing.
- calldata length is exactly 4, 36 or 68 bytes (68: registerReplacing(address,address)); any other selector reverts BadCalldata; no receive.

**Bond state machine**
- register(reward) with value 0..B_reg: active; HELD if the value is B_reg, ACCRUING below it (Earn-in rules).
- registerReplacing(reward, victim) with value 0..B_reg: the victim exits as by deregister and the sender registers as by register (Removal at a full set).
- topUp(): ACCRUING → HELD once the bond reaches B_reg.
- Settlement payments to an ACCRUING record fill its bond first (Earn-in rules, P1); it becomes HELD at B_reg.
- deregister and removeInactive: active → exited, with dueHeight = h+U.
- reactivate: from exited, live, after W_act and strictly before dueHeight.
- claimBond or sweeping: at or after dueHeight and outside closing, through PAY, then delete the record. A record whose bond PAY fails stays live and claimable, its bond kept in Escrow.
- Closing cursor (H_END ≠ 0 only): live → PAID, at ≤ RET per block.
- Evidence:
  - Against a live record (active or exited): BURNED and tomb=1; an active one leaves the active set.
  - Against PAID: write tomb=1 only.
  - Against a tombstone or identity without a record: invalidate the block.
- Delete a tombstone after burnHeight+E_win+D+1.

Error order: BadCalldata, BadValue, Closing, RegOpsLimit, BadPhase, then the semantic errors. At R_max=0, register returns RegOpsLimit outside closing and Closing during it.

Budgets per block (D-V3-08, D-V3-09): register, deregister, claimBond and removeInactive share REG_OPS less the maintenance used, and register also R_max; registerReplacing takes two REG_OPS units and one R_max entry. setRewardAddress and topUp are maintenance: at most REG_MAINT_MAX = 8 per block (a program constant) and one per identity per block. reactivate has its own R_react budget and a C_react cooldown counted from the identity's previous reactivation; it needs U ≥ W_act+2 and C_react ≥ U (ParamGate R8). A block therefore admits at most REG_OPS + REG_MAINT_MAX + R_react registry user operations.

**Earn-in rules (the open entry; formerly EARN-IN-SPEC §1-6)**

E1. Constants. B = B_reg (the programs read it as cfg(16)), 20 PCOL on the fresh network. HELD = 1 and ACCRUING = 4 are bondState values. nonceMode is 1. There is no probation pool, no pool bound and no deadline (FN-2b; owner decisions 10b and 10c of 2026-11-13): ACCRUING records count within M_max like any other and never exit by time.

E2. Record layout. registeredAt is w0 bits 152..215: register writes it and nothing changes it; since FN-2b no rule reads it (it is reported by pocol_getMiners). `live(w)` ⇔ status(w) ≠ 0 ∧ tomb(w) = 0 ∧ bondState(w) ∈ {HELD, ACCRUING}; `held(w)` keeps its meaning (bondState = HELD).

E3. User paths (MinerRegistry fallback). The dispatcher order of checks is `BadCalldata → BadValue → Closing → RegOpsLimit → BadPhase → semantic`.
- V1 (value): register and registerReplacing require msg.value ≤ B; topUp requires msg.value > 0; every other selector requires msg.value = 0; otherwise BadValue.
- U1 register(reward): the identity is not zero, SYSTEM or 0xC0C001–0xC0C0FF, has no record, and RecLen < REG_RECORDS_MAX and ActLen < M_max (else Capacity). Reward durability (R9): unless reward = msg.sender = tx.origin, the PAY-02 probe Sys.pay(reward, 0) must succeed, else RewardNotDurable; a self reward of the transaction's signer needs no probe, because that account persists past the transaction. If v = msg.value > 0, Sys.pay(ESCROW, v) must succeed, else PaymentFailed. The record gets bond = v, bondState = (v = B ? HELD : ACCRUING) and registeredAt = h; heldTotal += v.
- U2 topUp(): the caller's record is live, ACCRUING and active (status 1), else BadState. add = min(msg.value, B − bond); the rest returns to the caller. Sys.pay(ESCROW, add) must succeed, and so must Sys.pay(msg.sender, msg.value − add) when there is a rest, else PaymentFailed. bond += add and heldTotal += add; at bond = B the record becomes HELD. Rewards accrue every block, so a top-up quoted for the remainder can arrive after the remainder shrank; it still fills the bond, and only the remainder at inclusion is kept.
- U3 setRewardAddress (with the PAY-02 probe), deregister and removeInactive use `live` in place of `held`; removeInactive also needs a full set (Removal at a full set).
- U4 reactivate() uses `live` in place of `held` and does not change registeredAt.
- U5 claimBond() refunds any live record (S2).

E4. System paths.
- S2 refund: `refund` and `Escrow.releaseBond` accept any live record. If bond = 0, no PAY is made.
- S3 markClose (H_END ≠ 0 only) refunds every live record and marks it PAID.
- S4 punish(id, reporter): the bond branch applies to any live record and burns its bond, whatever its size (less the reporter's cut, Evidence).
- S5 accrue(id, amount) → excess: only Escrow may call it (else Unauthorized). For a live ACCRUING record: add = min(amount, B − bond); bond += add, heldTotal += add; at bond = B the record becomes HELD; return amount − add. Otherwise return amount and change nothing.

E5. Settlement (RewardEscrow).
- P1: when payTo or distribute owes x > 0 to a live ACCRUING record, excess = Registry.accrue(id, x); x − excess stays in Escrow, now held (neither burned nor paid), and excess is paid to the reward address (a failed PAY burns it, as for any payee). The signed proposer's tip share (Fees) is paid the same way. Payments to HELD records are unchanged.
- P2: settled value that moves into heldTotal leaves the Escrow balance unchanged, so the Escrow invariant (Aggregation) still holds.

E6. Proposer draw (node, `DrawWeights`; consensus §6).
- D1 Candidates: the members of A_h* except ACCRUING members whose bond is below B_reg/4 (D-V3-13: a member at B_reg/4 or above takes part). If there are none, every member of A_h* is a candidate (the all-probationer exception; owner decision 2026-11-12, option a).
- D2 Weights: candidates are weighted by their share counts over the draw window. A member that is not a candidate has weight 0. If every candidate's weight is 0, each candidate has weight 1. The sampler, seed and order are those of consensus §6.

Tests. `tests/v3/earnin-on-v3-vectors.json` replays the earn-in model's scenarios (tests/earnin_model.py, imported by tests/v3_model.py) on the v3 programs; `pocold/tests/v3_earnin.rs` runs the flow on a full chain (R9 for a first-transaction registration, a zero bond filled by real Escrow settlement and promoted at exactly B, a partial and an over-filling top-up, the escrow invariant after every block). Four planted MinerRegistry mutants (tests/compile_system_v3.cjs `MUTANTS`, tests/v3/system-v3-mutants.json; tests/v3_contracts.rs `EARNIN_MUTANTS`) each fail those vectors:
- r9-removed: the PAY-02 probe also for a signer's self reward;
- no-promotion-at-b: promotion only above B instead of at B;
- over-fill-bad-value: the first top-up rule, BadValue when msg.value exceeds B − bond (found by the wallet end-to-end test: an exact-remainder top-up reverted whenever a reward accrued between the quote and inclusion);
- excess-kept: the over-fill rest kept instead of returned.

Write counts (SSTOREs, tests/v3_contracts.rs wallet_bond_features_keep_their_write_ceilings_and_gas_on_the_v3_set): register 10, with a full or a partial bond; topUp 4, or 5 when it promotes; accrue 2, or 3 when it promotes. The same test bounds a zero-bond register's gas within the wallet's 400,000-gas quote (it measured 174,447 gas; contracts/REGISTRY-LAYOUT.md).

**Removal at a full set (FN-2b C4; owner decisions 10b and Q2(a) of 2026-11-13)**
- There is no seat contest. An inactive member, one with no credited share and no (re)activation within Theta_inact (height ≥ max(lastShare, graceStart) + Theta_inact), is removable only when active = M_max and more than M_min are active. removeInactive reverts below a full set; register alone reverts Capacity at a full set.
- A newcomer registers atomically into such a member's seat with registerReplacing(reward, victim): two REG_OPS units and one R_max entry, the sender must have no record, and the victim must be removable now (else BadState or NotRemovable). The victim exits as by deregister (refund after U, no burn; its unsettled pot parts still pay it and evidence can still punish it until the refund) and may register again; it reactivates only while a seat is free.
- No performance measure or threshold is involved: this makes atomic two operations anyone may already do in one block. The views removalStatus(victim) and activeSlice(start, count) write nothing.

**Sweeping and registry bound (D61, D86)**
- ≤ 64 steps and ≤ 16 deletions per block (SWEEP_SCAN, SWEEP_MAX, capped at 64/16 in the program). The sweep refunds exited records at dueHeight and deletes tombstones; it never removes an idle active member.
- LAG = 2048: protocol constant and maximum sweeping delay.
- Lmax = max(U, 2·E_win+D+1): maximum inactive-record lifetime before deletion eligibility.
- R16: REG_RECORDS_MAX ≥ M_max+R_max·(Lmax+LAG).
- Fresh network: Lmax = max(380, 2·360+4+1) = 725; bound 10,000+4·(725+2048) = 21,092 ≤ 24,576.

**Fees (D68, D-V3-20)**
- blockTips = Σ gasUsed·effectiveTip for phase-1 transactions.
- effectiveTip = min(maxPriority, maxFee−baseFee) for type 1559; gasPrice−baseFee otherwise.
- Base fee is burned outside Escrow and does not enter burnedTotal. The base fee never falls below baseFeeMin (10^7 wei on the fresh network).
- Tip split: on a signed block, floor(blockTips·gamma_tip_bp/10^4) (50 % at gamma_tip_bp = 5000) is paid at closeBlock to the proposer through payTo (an ACCRUING proposer fills its bond first; a tomb or a failed PAY burns it). The rest of the tips enters the pot. On a fallback block all tips enter the pot.
- There is no account-creation fee (FN-2a, owner decision 10d): a new account, nonzero slot or code byte pays ordinary gas only.

**Commitment and settlement (D78, COINBASE = Escrow)**
- pot_j = issuance(j) + (blockTips_j − tipShare_j) + carry_j, written in phase 2 of block j by closeBlock(tips, issued) as pending[j] = {amount, kind ∈ {signed, fallback}, proposer}; tipShare_j is 0 on a fallback block. issuance(j) is the M3 schedule (economics), credited to Escrow by the host. All values are known from UT, the body and the parent state before mining.
- carry_j (D-V3-15) is the base burn of the settle(j−1−D) that block j−1 ran, lagged by one block: closeBlock(j) adds it to pot j and zeroes it, and block j's own settle(j−D) feeds the carry into pot j+1. A pot is therefore fixed by its own block before mining; a later carrier of late shares never changes it, and the last carrier of late shares for j−D gains nothing in its own pot.
- winner starts at zero; recordResult(j) writes it in phase 0 of block j+1 as the owner of block j's nonce (D141), with WIN_ACCRUING when the winner is a live ACCRUING record.
- Recording lemma: R8 requires D ≥ 2, so j+1 ≤ j+D−1 and every settle(j) reads a recorded winner.
  - Reading winner = 0 in settle is a system assertion failure invalidating the block; it cannot occur on a valid chain.
  - Fallback effect: propPart goes to the recorded winner.
- Block F (H_END ≠ 0 only): no pending[F] is created. recordResult(F−1) precedes finalize in F, so pending[F−1] is settled with its winner.
- settle(j) in block j+D reads and deletes pending[j]; a reorg restores it.

settle formulas:
- invalid (an ACCRUING accused winner, Evidence): invalidBurn = pot.
- normal:
  - winnerPart = floor(pot·α_bp/10^4).
  - propPart = floor(pot·γ_bp/10^4), to the signed proposer or fallback winner.
  - R = pot−winnerPart−propPart; Q = max(C_j, κm).
  - p_i = floor(R·c_i/Q); A = floor(R·C_j/Q).
  - A tombstone's share goes to penaltyBurn; an empty account's share to payBurn; an ACCRUING payee's share fills its bond first (P1).
  - baseBurn = R−A, which goes to the carry into the next pot (D-V3-15), not to burnedTotal; roundBurn = A−Σp_i ≤ k_j−1 is burned.
  - Identity: pot = paidTotal+accrued+payBurn+penaltyBurn+roundBurn+baseBurn.
- F (H_END ≠ 0 only): no issuance at F, finalBurn = blockTips_F; finalize burns both outstanding carries, and the pots block F settles burn their base burn directly.

Aggregation:
- burnedTotal is one slot; burned value remains as Escrow balance.
- Surplus = balance−(Σpending+heldTotal+burnedTotal+carry) ≥ 0, where carry is the two lagged carry slots. A forced transfer into Escrow may raise the balance without raising the tracked amounts, so the surplus is nonnegative, not zero.

Issuance bound (R23): issuance(h) = max(S0 >> ⌊h/H_half⌋, S_tail) for h ≥ 1 (0 after 256 halvings of the S0 term), so over every height in 1..2^64 the issuance is at most 2·S0·H_half + S_tail·2^64. The genesis builder and ParamGate R23 add the allocations and genesis bonds and require the total below 2^127. With H_END ≠ 0 the block count is also ≤ H_END.

**ExecFixture (κm=64, α_bp=1000, γ_bp=500)**
- SV1: R=64000 and {w:1} → A=1000, baseBurn=63000.
- SV2: R=101 and {w:1, 64 identities with 1 share each} → A=101, Σp=65, roundBurn=36.
- SV3: R=1000 and {w:3, x:2} → p=46 and 31, roundBurn=1, baseBurn=922.
- SV4: R=6400 and t a tombstone → paidTotal=100, penaltyBurn=100, baseBurn=6200.
- SV5: as SV4, but t has no account → payBurn=100.
- SV6: invalid pot → invalidBurn=5000.
- SV7: pot=80000, fallback, C=1 → 8000 and 4000, R=68000, A=1062, baseBurn=66938, paidTotal=13062.
- SV7s: pot=164000, signed, C=1 → 16400 and 8200, R=139400, A=2178, baseBurn=137222, paidTotal=26778.
- SV10a (D116): R=64000, c_w=1, C_full=128 (w and 127 identities with 1 share each), C_kept=1. Full: p_w=500, A=64000, roundBurn=0, baseBurn=0. With exclusion: p_w=1000, A=1000, baseBurn=63000. g_dir=500 < 501.
- SV10b: C_full=64 → p_w=1000 in both cases, g_dir=0.
- SV10c: C_full=65 → 984 versus 1000, g_dir=16 < 16.39.
- In SV10a–c, w is represented only by ν (c_w=1); other identities' shares are explicit.
- SV10d (D120): R=64000; E_full = two explicit w shares and 127 others, so C_full=130, c_w^full=3, p_w=1476.
  - ownSharesOnly: C_kept=3, so p_w=3000 and g_dir=1524 < 1524.08.
  - emptyShares: C_kept=1, so p_w=1000 and g_dir=−476.
- SV10e (S_max boundary): R=64000; 300 explicit non-w shares arrive, trimmed to the 256 smallest nonces. Thus C_full=257, c_w=1, p_i=249 per share, Σp=63993 and roundBurn=7.
  - ownSharesOnly: p_w=1000, g_dir=751 < 751.98.
- SV10f: a 257-item shareList invalidates the block under item 1. Explicitly adding ν to the list is rejected under item 9.
- In SV1–SV10 baseBurn is the base burn of the split; on the v3 programs it goes to the carry into the next pot (Commitment and settlement). SV1–SV7s run through the compiled Escrow in `lp4-execution/src/apply/tests.rs`.

**ChainFixture**
- The M1 chain-level values (SV8, SV9 and their fallback, final and reorg variants) were computed under the M1 reward rules (a flat subsidy, no tip split and no carry); they are superseded and are not v3 vectors. The v3 chain-level vectors are tests/v3/issuance-carry-vectors.json, evidence-penalty-vectors.json, late-share-vectors.json, registry-budget-vectors.json and earnin-on-v3-vectors.json (independent model tests/v3_model.py), replayed by tests/v3_contracts.rs and the pocold v3 tests.

**Exclusion effect (T7c, D116)**

Notation:
Share sets (D120), for each height j won by w:
- ν: winning nonce. Always an implicit share outside shareList, counted in C_j and its owner's share count. Thus C_j = 1+|shareList_j|; c_i counts ν for its owner (consistent with SV1).
- E_full: valid explicit shares for height j (items 9 and 14, all ≠ ν) received by the winner before publishing its header.
  - Trim to S_max using the smallest nonces; this is honest construction policy.
  - ν does not enter the trimming or occupy a slot.
- E_kept: actual shareList content; length ≤ S_max under item 1.
- C_full = 1+|E_full| ≤ S_max+1 = 257; C_kept = 1+|E_kept|.
- c_w^full = 1 + w's explicit shares in E_full; c_w^kept = 1 + w's explicit shares in E_kept.
- Hence c_w ≥ 1 always; an empty list gives C_kept = c_w^kept = 1.

Effect:
- General direct gain: g_dir = floor(R·c_w^kept/max(C_kept, κm)) − floor(R·c_w^full/max(C_full, κm)).
  - winnerPart and propPart do not depend on shares.
  - g_dir may be negative.
- ownSharesOnly arm: E_kept = w's explicit shares in E_full. Thus c_w^kept = c_w^full = c_w, and C_kept = c_w ≤ C_full. Then:
  - g_dir = 0 if C_full ≤ κm because both denominators are κm.
  - Otherwise 0 ≤ g_dir < R·c_w·(1/max(C_kept, κm) − 1/C_full)+1 ≤ R·c_w·(1/κm − 1/C_full)+1.
  - The bound requires fixed c_w; it does not apply to a list adding w shares absent from E_full.
- emptyShares arm: E_kept = ∅, so c_w^kept = 1 and Q = κm.
  - g_dir = floor(R/κm) − floor(R·c_w^full/max(C_full, κm)), computed exactly without another bound.
  - It equals ownSharesOnly only when c_w^full = 1.
- Indirect effect: excluded shares enter neither w(m) in Draw nor lastShare for excluded miners, potentially increasing the adversary's later proposer share and propPart. There is no analytical bound; relative g in T7c(S) measures it, and the 2% bound is a falsifiable hypothesis.
- On the v3 programs a share the winner left out can still be credited before settlement by a later signed carrier (Late shares), so the exclusion lasts only while no carrier within D blocks includes it; the bounds above are for the header alone.

**T17 per-block invariants**
- Conservation, roundBurn ≤ k−1, pot = issuance + tips − tipShare + carry, and balanced Escrow: balance ≥ Σpending+heldTotal+burnedTotal+carry (apply.rs `check_escrow_invariant`); after phase 2 of block h the carry into pot h is zero.
- PAID at most once.
- pending[j] exists for every nonfinal j ∈ [h−D+1, h].
- (D141) pending[j].winner ≠ 0 for every j ∈ [h−D+1, h−1]; pending[h].winner = 0 at the end of block h.
- heldTotal(0) = |M_0|·B_reg; h ≤ H_END when H_END ≠ 0.
- Evidence, rewards, registry counters, late credit and carry all follow inverse diffs on reorganization.

PAY: 5000 gas, cannot fail and executes no code.

RPC service has no reward; its cost is borne by the operator.

**Disclosed**
- On the continuous chain (H_END = 0) there is no final block F. On a finite fixture, F's winner has no direct reward.
- Selfish withholding is unresolved (T10).
- reward_mode=equal is for comparison only [PDF:103 (5.9)].
- Theta_inact and E_win are one hour (360 blocks); U = 380 covers E_win+D with margin and the reactivation constraint. Time-derived block counts, resource bounds and contract write ceilings are measured in the profile and resource tests, not derived here.
- On-chain costs and credited shares do not establish Sybil resistance or exhaustive search; see threat.md, A6.
