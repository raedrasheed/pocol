# Contribution and rewards

**From the thesis**
- R_tot = subsidy+fees [PDF:103 (5.6)]; Σf = 1 [(5.7)].
- Equality incentivizes Sybil identities [PDF:103].
- Small rounding error is acceptable [PDF:104].
- Activity is not proof of search [PDF:105].
- Inactivity removal has no bond penalty [PDF:89, PDF:112].

**Share (D6)**
- A shareList item satisfies items 9 and 14, where T_share = min(2^256−1, target·m).
- The winning nonce is an implicit share.
- (D142) c_i, C_j and lastShare are derived in recordShares(j) from ParentResult(j) of block j on the executed branch. Settlement therefore depends on block j's own committed shareList, not the first copy seen by the miner or node.
- Shares sample work; they do not prove exhaustive search. κm = 64.
- pocol_submitShare is a local-only mining interface. A site cannot reach it through the bridge (D95), nor can any request carrying Origin.

**Evidence**

Type 2 (D29): RLP([2, TemplateID, nonce, shareRoot1, sig1, shareRoot2, sig2]).
- Valid if the roots differ, owner(nonce) signed both, an ancestor block exists at j ≥ h−E_win, and the accused has a record with tomb=0.
- Before settle(j), its effect is to invalidate and burn pot_j.

Type 1 is valid if:
- UT1 ≠ UT2, both signed at a=0 with the same (parentHash, h) and cfg identity.
- h ≥ h_incl−E_win.
- The signer is DrawIndex[h mod E_win].
- The accused has a record with tomb=0.

Evidence violating its conditions invalidates the containing block.

**Registry**
Rec[id] has four words:
- w0: status, tomb, bondState, recPos, actPos, lastShare.
- w1: rewardAddr.
- w2: bond.
- w3: exitHeight, dueHeight, burnHeight, graceStart.

M_0 members are written in genesis (D79).

**Authorization (D62)**
- id = msg.sender in all functions except permissionless removeInactive.
- calldata length is exactly 4 or 36 bytes; no fallback or receive.

**Bond state machine**
- register with value B_reg: active and HELD.
- deregister and removeInactive: active → exited, with dueHeight = h+U.
- reactivate: from exited, HELD and tomb=0, after W_act.
- claimBond or sweeping: after dueHeight and outside closing, through PAY, then delete the record.
- Closing cursor: HELD → PAID, at ≤ RET per block.
- Evidence:
  - Against an active record: BURNED and tomb=1; remove from the active set.
  - Against PAID: write tomb=1 only.
  - Against a tombstone or identity without a record: invalidate the block.
- Delete a tombstone after burnHeight+E_win+D+1.

Error order: BadCalldata, BadValue, Closing, RegOpsLimit. At R_max=0, register returns RegOpsLimit outside closing and Closing during it.

**Sweeping and registry bound (D61, D86)**
- ≤ 64 steps and ≤ 16 deletions per block.
- LAG = 2048: protocol constant and maximum sweeping delay.
- Lmax = max(U, 2·E_win+D+1): maximum inactive-record lifetime before deletion eligibility.
- R16: REG_RECORDS_MAX ≥ M_max+R_max·(Lmax+LAG).
- network: Lmax = 4113; bound 256+2·6161 = 12578 ≤ 16384.

**Fees (D68)**
- blockTips = Σ gasUsed·effectiveTip for phase-1 transactions.
- effectiveTip = min(maxPriority, maxFee−baseFee) for type 1559; gasPrice−baseFee otherwise.
- Base fee is burned outside Escrow and does not enter burnedTotal.

**Commitment and settlement (D78, COINBASE = Escrow)**
- pot_j = subsidy+blockTips_j, written in phase 2 of block j as pending[j] = {amount, kind ∈ {signed, fallback}, proposer}. All three values are known from UT and the body before mining.
- winner starts at zero; recordResult(j) writes it in phase 0 of block j+1 as the owner of block j's nonce (D141).
- Recording lemma: R8 requires D ≥ 2, so j+1 ≤ j+D−1 and every settle(j) reads a recorded winner.
  - Reading winner = 0 in settle is a system assertion failure invalidating the block; it cannot occur on a valid chain.
  - Fallback effect: propPart goes to the recorded winner.
- Block F: no pending[F] is created. recordResult(F−1) precedes finalize in F, so pending[F−1] is settled with its winner.
- Type-2 evidence for block j does not need the recorded winner because it invalidates and burns pot_j.
- settle(j) in block j+D reads and deletes pending[j]; a reorg restores it.

settle formulas:
- invalid: invalidBurn = pot.
- normal:
  - winnerPart = floor(pot·α_bp/10^4).
  - propPart = floor(pot·γ_bp/10^4), to the signed proposer or fallback winner.
  - R = pot−winnerPart−propPart; Q = max(C_j, κm).
  - p_i = floor(R·c_i/Q); A = floor(R·C_j/Q).
  - A tombstone's share goes to penaltyBurn; an empty account's share to payBurn.
  - baseBurn = R−A; roundBurn = A−Σp_i ≤ k_j−1.
  - Identity: pot = paidTotal+payBurn+penaltyBurn+roundBurn+baseBurn.
- F: subsidy_F = 0, finalBurn = blockTips_F; block F settles everything remaining.

Aggregation:
- burnedTotal is one slot; burned value remains as Escrow balance.
- Surplus = balance−(Σpending+heldTotal+burnedTotal) ≥ 0.

Issuance bound (D85): block count ≤ H_END; F has no subsidy, so subsidies ≤ subsidy·(H_END−1).

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

**ChainFixture**
- SV8: Δburned(F)=1155008, w +208992.
- SV8-fb: 1071008.
- SV9: pot_30=253015.
- SV9-final: 1244023.
- SV8-reorg: 1208230 since 198.
- SV9-reorg: pending[30]=80000.

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

**T17 per-block invariants**
- Conservation, roundBurn ≤ k−1, pot−subsidy = tips, and balanced Escrow.
- PAID at most once.
- pending[j] exists for every nonfinal j ∈ [h−D+1, h].
- (D141) pending[j].winner ≠ 0 for every j ∈ [h−D+1, h−1]; pending[h].winner = 0 at the end of block h.
- heldTotal(0) = |M_0|·B_reg; h ≤ H_END.

PAY: 5000 gas, cannot fail and executes no code.

RPC service has no reward; its cost is borne by the operator.

**Disclosed**
- No direct reward for F's winner.
- Selfish withholding is unresolved (T10).
- reward_mode=equal is for comparison only [PDF:103 (5.9)].
