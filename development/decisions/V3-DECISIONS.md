# PoCol v3 decisions — 2026-11-11

Status: records the owner approval of 2026-11-08 and its implementation constraints. This is a documentation decision record, not an activation, a deployment approval or evidence that the implementation is complete. The live network remains untouched.

## Owner rule and scope

> أوافق على تعديل التي لا تغيير خوارزمية الاجماع بدون اي تحفظ. أما التي تحتاج تغيير في خوارزمية الاجماع فأوافق على التغيير الذي لا يغيير فكرة الاجماع بالتعاون ولا يغير فكرة الكتلة النموذج أو تغيير أي مبدأ من المبادئ الاساسية ل PoCol.

Node-local changes are approved without reservation. Consensus changes are approved only while preserving collaboration, the common immutable template, and every basic PoCol principle P1–P10 in [V3-DESIGN.md](V3-DESIGN.md). Stop and ask the owner before crossing that boundary or deciding an unanswered owner question.

For compliant items with the choices below, this standing owner authorization supplies the required owner decision; it supersedes the historical proposal-before-implementation gate cited as `development/m1/reference/README.md:484–486` for those items only. It does not approve rejected mechanisms, deferred work, unanswered questions reserved to the owner or live deployment. Preserve earlier decision records as history; the scoped amendments below take precedence only for their stated profiles.

Sources: [approved design](V3-DESIGN.md), [dated owner answers](V3-OWNER-DECISIONS-2026-11-08.md), and [consensus boundary policy](../VA-SPEC.md). The implementation PR sequence is the handoff roadmap; its later dormant-gate strategy supersedes the design earlier suggestion to spend a gate at the first v3 scaffold PR.

## Owner choices

| Question | Item | Decision |
|---|---|---|
| O-0 Vehicle for v3 | — | **a**: v3 rides the held earn-in re-genesis, and the same release builds the in-place fork machinery for later changes |
| O-6 Block time | V3-06 | **b**: 30 s (D_att 150, dFbWait 10, Δ_rel 13), valid if the measured WAN Δ ≤ 2 s; otherwise 60 s (option c) |
| O-10 Contestable seats when the set is full | V3-10 | **b**: contest by work over one day with a statistical margin (θ=1/8, 3σ), only when all seats are full, with the displaced member refunded, and only together with V3-02 |
| O-11 Inactivity removal delay Θ_inact | V3-11 | **b**: 1 hour |
| O-12 Registration bond B_reg | V3-12 | **a**: 20 PCOL |
| O-13 Probationers as proposers | V3-13 | **b**: yes, once the accrued bond is at least B_reg/4; retain the all-probationer exception clarified below |
| O-14 Probation pool | V3-14 | **b**: M_max/4 slots (64), contest by work inside the pool, T_PROB = 30 days, both as CP fields (requires V3-09) |
| O-15 Uncovered share pot | V3-15 | **b**: carried into the next block's pot at closeBlock |
| O-17 Reward for reporting misbehaviour | V3-17 | **b**: 25% of the burned bond to the reporter, 75% burned |
| O-18 Evidence window and bond delay | V3-18 | **b**: 1 hour, with U = E_win + D + a margin |
| O-19 Issuance schedule | V3-19 | **M3**: M1's rate (2 PCOL per 60 s, scaled by T_blk) halving every 4 years, with a tail of S0/32 |
| O-20 Fee market | V3-20 | **b**: the proposer receives 50% of tips directly, the rest stays in the shared pot, base-fee floor 0.01 gwei; the pool becomes fee-ordered with replacement and eviction (node-local) |
| O-21 State caps | V3-21 | **b**: raise the caps (A13-verified) and add a burned creation fee that rises as the cap fills |
| O-32 The 4,000,000 PCOL genesis allocation | V3-32 | **b**: publish the holder addresses and their purpose, and move the development fund into a public time-locked account with published spending reports |
| O-33 How the re-genesis is run | V3-33 | **b**: no user transactions after S; publish the old-chain data; ship an Android key-export build first; and commit publicly that this is the last owner-only re-genesis, with later consensus changes going through the fork schedule |

## Owner clarification — 2026-11-11

For O-13, the owner answered: **“Retain the all-probationer exception below B_reg/4.”** The quarter-bond filter applies normally; when it would exclude every member of the draw membership, retain the existing exception that admits all those probationers to the deterministic share-weighted draw. This avoids removing the draw in that case. The exception means the B_reg/4 minimum equivocation penalty is not universal. This clarification supplements the dated O-13=b answer without rewriting its historical source.

## Decisions D-V3-01 through D-V3-34

### D-V3-01

Give a nonce range to every live (non-tombstoned) member whenever the nonce domain is unbounded; removes the permanent-halt and range-starvation path (MISSED share inclusion, ECON-03). For v3 unbounded mode, the parent-state active list excluding tombstones owns the ranges regardless of recent-share activity. Bounded mode and spec-1 behavior remain unchanged. Membership remains independent of attempt.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch8 (activity gating) but preserved; P4/P5 kept, Q3.5 improved. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-02

Late share credit: proposers of blocks j+1..j+D carry uncredited valid shares of template j as body evidence; late credit only fills the pool up to κm, so it never dilutes header shares. Signed bodies may carry RLP([3, be64(j), [n_1<...<n_k]]) for ancestor templates j in [max(1,h-D),h-1]. Check target, range owner, exclusion of the winning nonce and header shares, branch deduplication and C_header(j)+C_late(j)<=kappa*m. Record late credit after recordResult and before settlement; lastShare=max(lastShare,j). Roll credit back with the branch. This never dilutes header shares.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch6 but preserved; P2/P5 kept, P6 strengthened (winner no longer alone decides credit). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E3; pocold chain.rs `late_fields`/`late_window`/`late_check`, apply.rs, PoColSystemV3.sol MinerRegistry and ShareLedger `recordLate`, prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md "v3 late share credit"). Kind 3 is valid only on the v3 program set (v3_active and the v3 sysCodeHash, a specVersion-2 genesis); elsewhere it stays "15 evidence form". Checks, in order: form (j and each nonce exactly eight bytes, 1 <= k <= S_max, strictly ascending), window, then on template j of the carrying branch the cap C_header(j) + C_late(j) + k <= kappa*m (C_header counts the winner's share; for j = h-1 from the parent's ParentResult, otherwise the parent state's ledger total, which already includes the branch's late credits) and per nonce: below nMax_j (owner from block j's stored ranges), not the winning nonce, not a header share, not credited on the branch, not credited earlier in this block, on target. Several kind-3 items may name one j in one block (the design's "not repeated in this block" is the nonce rule); the cap counts the block's earlier credits. The effect is recordLate(j, owners) in body order among the evidence, after recordShares/recordResult of h-1 and before settle(h-D). Branch deduplication stays on the Entry without a new field: an offloaded entry keeps its body's kind-3 items, a slimmed one drops them and `unslim` (and a rebuild's restored copies) read them back from the journal body, so Entry's layout, the journal and checkpoint bytes are unchanged (a valid spec-1 block carries no kind-3 item). Coupling with V3-15 (principles-review finding 4): since V3-PR-E2a, the uncovered pool part of pot j (its base burn) goes to the carry at settle(j). It enters the pot closed in the next block, because the carry is lagged by one block. recordLate for j always precedes settle(j), so that carry is computed after every late credit to j. Each pot is fixed before its template is built (see the D-V3-15 implementation note). The lag also makes the last carrier incentive-neutral. Block h is the last carrier of j = h - D. Had pot h received settle(h - D)'s base burn, h's signed proposer would have been paid to leave out other members' late shares of h - D, exactly the items the node-local policy below includes first. Under the lag that value reaches pot h + 1, whose block can no longer carry them (the D-V3-15 incentive finding and decision, option (b)). The late window [max(1,h-D),h-1] is unchanged. Node-local part (review of E3): the share cache keeps, for the next carrier, the branch's template of each height in [max(1, H+1-D), H] and the two newest templates of the production height (never the last D templates the node locked, which dead attempts could fill); a template locked again leaves the cache and takes its shares back; a new valid share of a cached template is relayed within the per-template cap of 4·S_max; the proposer fills E_max oldest template first (j = h-D has no later carrier). A received template whose height is not its parent's plus one is punished (item 2) before its evidence is judged, and kind-3 rules judged at such a height are the verdict "15 ls window". That height check is node-local and runs on every chain: on spec 1 such a template used to be locked and mined although its block could never be valid (item 2); no verdict changes.

### D-V3-03

Multi-hop relay of valid templates and shares.

Approved within the owner rule; NODE-LOCAL (already in tree at 53a9e87). Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-04

Fork choice uses timeliness-qualified signed precedence, with the specified arrival allowance for attempt-0 fallback. At first import, signed blocks are timely iff arrival<=T_j+Phi; attempt-0 fallback is timely iff arrival>=T_j-(Delta+Phi); later attempts are timely. On v3 only, sibling key=(timely?0:1,signed?0:1,importSeq,blockHash), compared at first path divergence after cumulative work. Persist first-import time across restart. Amends D133; validity is unchanged.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch10 (amends D133) but preserved; D15 signed precedence kept for honest proposers; Q2.8 improved. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-05

Δ_rel default derived from measured WAN Δ (not lowered to 7 s); per-proposer template-latency counters in `pocol_status`.

Approved within the owner rule; NODE-LOCAL. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-06

Recalibrate T_blk, D_att, dFbWait, Δ_rel from the ParamGate inequalities on measured WAN latency. O-6=b: T_blk=30 s, D_att=150 s, dFbWait=10 s, local delta_rel=13 s, subject to measured WAN Delta<=2 s and phi<=2 s and ParamGate. Otherwise use the approved 60 s fallback and recompute time-derived block counts. These are candidate profile values until measurement and resource proofs.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameters (Ch5/Ch11); kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-07

M_max 64 → 256; compute draw weights from ledger owners instead of M_max×W_w record reads. M_max=256 is subject to the A13 proof. Iterating ledger owners for draw weights must be result-identical on every old-chain input.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: parameter; P8 loosened. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-08

REG_OPS: separate per-block maintenance budget (setRewardAddress/topUp) and at most one maintenance op per identity per block. Use separate REG_MAINT_MAX=8 per-block maintenance budget and at most one maintenance operation per identity per block; registration/deregistration/claim/removal retain REG_OPS minus maintenance used.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: Ch9; restores P8/Q5.2. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-09

R_max entry slot: reactivate gets its own budget and a cooldown ≥ U; R_max 1 → 4. Use R_max=4 registration entries, a separate R_react=4 budget and per-record C_react>=U cooldown. Reactivation remains after W_act cooldown and strictly before the refund sweep. Require U>=W_act+2.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: Ch9 + parameter; restores P8/Q5.2. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-10

Contestable seats when the set is full: displace (with refund) a member whose credited work in the last full epoch is statistically below threshold. O-10=b: contest only a full set, with previous full-day epoch count below E-3*sqrt(E), E=(1/8)*Tprev/active, with deterministic integer rounding. Victim must be live, active, past grace and active throughout the previous epoch. Refund after U; V3-02 is mandatory first. Validate honest-member eviction rates by simulation.

Approved within the owner rule; OWNER-CHOICE. Principle check: touches P8 (new removal reason) and P7; preserved only with V3-02 and the statistical margin. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E4; PoColSystemV3.sol MinerRegistry recordShares/recordLate/registerContest/contestStatus, prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md "v3 contestable seats", tests/v3_model.py, pocold wallet.rs quote_contest). Dormant: the v3 program set only (its sysCodeHash on a specVersion-2 genesis, which SPEC_VERSION=1 builds refuse); spec-1 and earn-in programs, the host's calls and verdicts, every constant and the identity are unchanged. E_epoch, theta_bp and k_sigma are the CP schema v2 fields 63..65. A credited share (header or V3-02 late) counts in the epoch of its template j, j / E_epoch, so late credit enters the statistic like a header share. Per record, countCur and countPrev (20 bits each, saturating; a saturated count is never contestable) describe the epoch of lastShare and the one before, in spare bits of the record word recordShares and recordLate already write (V3-08's lastMaintHeight moved to w1, modulo 2^32), so no SYS key per owner is added; slot 10 holds the epoch totals of every credited share. registerContest(reward, victim), payable as register, takes two REG_OPS units (a removal and a registration, REG_OPS less the maintenance used) and one R_max entry. The rule, in order: the victim live and active; the set full (active >= M_max), for a pool contest too (D-V3-14); a measured epoch p = h / E_epoch - 1 exists and its late window has closed (h mod E_epoch >= D - 1, so all of p's late shares are credited before it is judged: omission cannot manufacture a victim; contests need E_epoch >= D); past grace (h >= graceStart + Theta_inact); registration or last reactivation at or before p's first block (graceStart <= p * E_epoch); countPrev < E - k_sigma*sqrt(E), E = theta*Tprev/active with active the active count now, which the previous condition makes M_max. Deterministic integer rounding, exact: with N = theta_bp*Tprev, Dn = 10^4*active, s = isqrt(k^2*N*Dn) (floor), the integer threshold is ceil((N - s)/Dn) (0 when N <= s), and an integer count is below it iff it is below the real E - k*sqrt(E); at E = 45 (active 256) it is 25. The victim exits as by deregister (refund after U, no burn; evidence exposure until dueHeight) and the newcomer registers as register(reward). Error: NotContestable (BadState for a victim without a live active record). Views contestStatus(victim, probation), contestThreshold(tprev, active), epochTotals(). Gas: GAS_ENVELOPE_V3 recordShares 14,389,000 (a new v3 entry; 257 owners without a record measure 14,388,687), recordLate 3,466,000, settle/settleFinal 6,315,000 (the larger registry dispatch), markClose 2,735,000 (review of V3-PR-E4: the closing phase's 128 refunds through the larger dispatch, 64 of them ACCRUING, measure 2,734,171, above the spec-1 2,655,000); MinerRegistry 13,969 bytes. Open (for the owner, not decided by the implementation): the eligibility rule restarts at every reactivation as the design states, so a member that exits and reactivates once per epoch (within C_react) is never contestable; SimNet eviction rates (V3-PR-H) remain to be measured. Input for that study (review of V3-PR-E4, a Poisson estimate of the implemented statistic, Tprev = 92,160 shares a day, active 256, threshold 25, under a persistent prober): a member with 0.05% of the credited shares is evictable on 2.6e-4 of days (about 9% a year), 0.055% about 0.9% a year, 0.06% about 0.07% a year, and from 0.07% on negligibly; at 0.04% eviction within a year is near certain. V3-DESIGN.md's "a member with >= 0.05% of hashrate is essentially never displaced" is therefore optimistic at exactly 0.05%; the safe range is from about 0.06 to 0.07%.

### D-V3-11

Inactivity eviction delay Θ_inact, set in hours. O-11=b: one hour, converted into blocks using the final measured T_blk.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameter. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-12

Registration bond B_reg. O-12=a: B_reg=20 PCOL; cutover tooling reads the new genesis CP[16], never a mirrored literal.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameter (Ch9). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-13

Probationary (ACCRUING) members as proposer candidates. O-13=b: an ACCRUING member may enter the proposer draw once bond>=B_reg/4. Owner clarification on 2026-11-11: retain the existing all-probationer exception below B_reg/4 when the filter would otherwise exclude every draw member. Keep the share-weighted deterministic draw.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch1 candidate set; P3 kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-14

Probation pool size and in-pool contest; P_MAX and T_PROB become CP fields set in time. O-14=b: P_MAX=M_max/4 (64 at M_max=256), T_PROB=30 days as CP fields. Use the same work statistic for contests within the ACCRUING pool, refund accrued bond and conserve ACC_COUNT. V3-09 precedes expansion.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch9; P8. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E4; PoColSystemV3.sol registerContest, REGISTRY-LAYOUT.md "v3 contestable seats"). The pool contest is registerContest by a probationer newcomer (msg.value < B_reg) when ACC_COUNT >= P_MAX and the set is full (active == M_max, as every contest; review fix below): the same rule and statistic as D-V3-10 (E = theta*Tprev/active over all active members), restricted to ACCRUING victims (another victim is BadState). The victim leaves the active list, its accrued bond is refunded through refund() (ACC_COUNT and held_total fall there, as the design's ":394"), its record is erased in the same call, and the newcomer takes the freed probation seat. A probationer newcomer with room in the pool, or a full-bond one, uses the seat contest (set full only). ACC_COUNT = the live ACCRUING records after every step (promote, exit, displace, punish) in the model and on the bytecode. Open (for the owner, not decided by the implementation): erasing the victim at once, as the design says, ends its evidence exposure, so an ACCRUING member that committed an offence within E_win and is contested before any block carries the evidence escapes the bond burn and V3-16's whole-pot burn (the seat contest keeps the victim punishable until dueHeight); keeping the pool victim exited until U would close this but delay the freed seat by U blocks. Review of V3-PR-E4: (1) the same immediate erasure burns an honest victim's parts of the pots still pending (its shares, winner and proposer parts of the last D templates, and late shares of them credited afterwards): they settle to an id without a record, so payInfos names no reward address and they burn instead of filling the accrued bond it was refunded (tests/v3/contest-vectors.json pool-victim-pending-pots-burn: the burn exceeds the chain without the contest by exactly what its bond would have accrued). This conflicts with V3-10's "refund, no stake penalty" (Q5.4) and is left with the evidence point for the owner; the same choice settles both (keep the pool victim exited, payable and punishable, until its pots settle or until U, with the probation slot freed at once or after; or route the unsettled parts to its reward address; or accept the burn). (2) The first draft allowed a pool contest on a set that is not full and divided E by the active count at the contest, so a contestant could exit its own members in the contest block to raise the threshold (and bonded joins could lower it to shield idle probationers). The pool contest now needs the full set, as O-10=b ("only when all seats are full") and the roadmap ("registerContest only when active == M_max") read literally, so E's denominator is always M_max (vector pool-contest-exits-cannot-raise-the-threshold; mutant pool-contest-set-not-full-allowed). Consequence for the owner: a full pool with free seats admits no contest (as on the earn-in set); pool contests on a set that is not full would need a denominator the contestant cannot move (M_max, or the active count recorded for the measured epoch).

### D-V3-15

Carry the uncovered share pot into the next pot at closeBlock. O-15=b: send uncovered base-burn share remainder to carry at settlement; roundBurn remains burned. closeBlock(j) adds and zeroes carry after that block phase-0 settlement, so pot j is fixed by its template. Burn carry at a finite final block. Escrow invariant includes carry.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch7; P6 kept (carry fixed before mining). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E2a, v3 program set only; prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md "v3 issuance, carry and tip split"). The carry is lagged by one block (decision below): RewardEscrow slot 4 + (t & 1) holds the carry into pot t. settleOne's distribute in block h adds the base burn R - A to the carry into pot h + 1; roundBurn, payBurn, penaltyBurn and the whole-pot INVALID burn still burn. closeBlock(tips, issued) of block h records pot h = issued + (1 - gamma_tip)·tips + the carry into pot h and zeroes that slot. So at most two carries exist: inside block h the carry into pot h (left by block h - 1) and the one block h's settle(h - D) is writing into pot h + 1; between blocks only the carry into the next pot, which starts at zero (genesis writes no Escrow storage). Pot h therefore depends on block h - 1 for its carry and on block h for its issuance and tips. Its carry is the base burn of settle(h - 1 - D) in block h - 1's phase 0, which runs after every late credit to h - 1 - D (recordLate precedes settle, principles-review finding 4). Changing late-share inclusion after block h never changes pot h; the model and the bytecode test this. At a final block, finalize burns both outstanding carries with the tips: the carry into pot F, which no pot receives, and the carry of F's own settle(F-D). settleFinal (slot 1 set) burns its base burns directly, so nothing is carried past F. The host invariant counts both carries (need = Σpending + held + burned + carry into h + carry into h + 1). After phase 2 it refuses a carry into pot h, a carry into pot h + 1 before any settlement ran (h <= D), and any carry after a final block. Spec-1 programs never write slots 4 and 5 and their invariant is unchanged. The model and the bytecode compare pots up to h both as closeBlock(h) recorded them and as settle later distributed them, and both checks fail on a planted mutant that adds half of a settle's base burn to an already-closed pending pot (the shape safety finding 8 rejects).

Incentive finding and decision (review of V3-PR-E2a; decided under the owner's rule that consensus changes keeping collaborative consensus, the template block and every PoCol principle are approved). Block h is the last block that can carry late shares of j = h - D: the window is [max(1, h-D), h-1], and recordLate precedes settle(h-D) in h's phase 0. As first implemented, pot h received settle(h-D)'s base burn. While C_j < kappa·m, each late share of j that h's signed proposer left out raised R_j - A_j by about R_j/Q and moved that value from the share's owner into pot h. The proposer then received gamma_bp/10^4 of it (5% at 85/10/5), plus alpha_bp/10^4 if it also won h, plus its own pool fraction of pot h. Size: at S0 = 1 PCOL, R ≈ 0.85 PCOL and Q = 64, omitting 63 late shares added ≈ 0.84 PCOL to pot h and ≈ +0.042 PCOL to its proposer, close to doubling its 0.05 PCOL proposer part. The model scenario (D = 4, eight late shares of another member left out) gives the proposer +0.010625 PCOL while that member loses 0.2125 PCOL. That paid the last carrier to drop other members' late shares, exactly the ones the node-local policy includes first, and weakened P6 (V3-02's aim that the winner no longer alone decides credit). Chosen: option (b), lag the carry by one block. settle(h-D)'s base burn now enters pot h + 1, whose block can no longer carry late shares of h - D because the window has closed. The proposer of h therefore gains nothing in its own pot from omitting LS(h-D). tests/v3_model.py check_last_carrier_gains_nothing and tests/v3_contracts.rs last_carrier_gains_nothing_by_omitting_late_shares compare pots h and h + 1 with and without the omission: pot h is identical, the omitted value appears in pot h + 1, and the proposer is paid the same. Both checks fail on the planted carry-not-lagged, the previous rule. Why every principle is kept: (1) collaborative consensus is unchanged, because the share ledger, Q = max(C, kappa·m) and the no-dilution cap are untouched. (2) The template block is unchanged. The full late window [max(1, h-D), h-1] stays, so no honest late share loses a carrier. (3) P6 is kept and strengthened. Pot h + 1's carry is fixed by block h, before template h + 1 is built, so it is fixed before mining, more strictly than before. Its proposer had no say in it. (4) Value is conserved. Escrow holds every outstanding carry; the invariant, the model and the harness count both slots, and the final block burns both. (5) Spec-1 and earn-in are unchanged, as are every constant and the identity (only the v3 programs and their sysCodeHash change). Costs: one more Escrow slot can be nonzero between blocks, which R18 counts (and two carry keys per block). finalize can now zero two slots; it measures 61,932 on its worst path, so GAS_ENVELOPE_V3 finalize is 62,000. Implementation choice: two slots keyed by the parity of the receiving pot, rather than one carry slot copied forward at closeBlock. Each slot is written only in block t - 1 and drained only by closeBlock(t) or finalize, no copy is made, so closeBlock's gas and worst path are unchanged. Settle's write is still the zero-to-nonzero store it was. Not chosen: (a) Accept and document the gain, measuring it in SimNet T7c(S). This keeps a known, targeted reward for omission. (c) On v3, close j's late window at j + D - 1. This also removes the gain, but it shortens the late-credit window by a block, so an honest late share of j found by block j + D's proposer could no longer be credited. It also changes kind-3 validity (pocold late_window and the vectors), which is a wider consensus change than which pot receives the carry. What remains after (b) is the ordinary diffuse interest of any miner in a later pot. An omitter expects about its hashpower fraction of pot h + 1, not knowing whether it will win or propose it. That interest existed before for every carrier j+1..j+D-1, under every option, and is inherent to carrying rather than burning the base burn (O-15=b). V3-PR-H's SimNet T7c(S) (share omission with late credit) still measures omission with late credit.

### D-V3-16

Type-2 evidence burns only the offender's parts; whole-pot burn kept when the accused is ACCRUING. Settle normally after punishment; tombed offender winner/proposer/share parts burn, while honest contributors are paid. Retain whole-pot burn if the accused was ACCRUING below B_reg at block j, recording that fact at recordResult rather than reading later state.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: Ch7; P6/P7 strengthened. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E2b, v3 program set only; prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md). The fact recordResult can commit is about pot j's winner, the only identity known when the result is recorded, and evidence may arrive later; so the whole-pot burn applies when the accused is pot j's winner and that winner was ACCRUING when the result was recorded (bit 160 of the winner word). Evidence against any other member of block j (a share owner, the signed proposer) settles the pot normally, burning that member's parts. To record the fact as of block j even when the evidence is in block j+1, the v3 host runs recordShares(h-1) and recordResult(h-1) before the block's evidence in phase 0. Several pieces of type-2 evidence may name one pot. Whole-pot burn for a non-winner accused (which would let a zero-bond equivocator burn an honest winner's pot) is not implemented.

### D-V3-17

Reporter reward ρ of a burned bond. O-17=b: 25% of punished bond goes to the carrying signed proposer reward address; burn 75%. A fallback has no reporter and burns the whole bond. Self-reporting still loses 75%.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch7/Ch9; P6 kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E2b, v3 program set only). ρ is CP schema v2 field rho_bp (CP 66; 2500 in the draft profile). The host passes the reporter to punish; the program requires it to equal tx.origin, the carrying block's signed proposer (zero on a fallback). The cut is floor(bond·rho_bp/10^4), paid by PAY directly to the reward address of the reporter's record, whether that record is live, ACCRUING (no accrual into its bond) or a tomb (a self-report, or a proposer punished earlier in the same block); a failed PAY burns it. The rest burns.

### D-V3-18

Evidence window E_win and bond delay U, set in hours. O-18=b: E_win is one hour; U=E_win+D+margin, with U>=W_act+2 and existing retention constraints. Re-derive memory bounds.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameters. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-19

Use M3 issuance: a scaled initial rate, four-year halvings and a tail. O-19=M3: issuance(0)=0; otherwise max(k>=256?0:S0>>k,S_tail), k=floor(h/H_half). Rate is 2 PCOL/60 s, scaled by T_blk; halve every four years, tail S0/32. Bound 2*S0*H_half+S_tail*2^64<2^127. The host passes issued explicitly to closeBlock.

Approved within the owner rule; OWNER-CHOICE. Principle check: P6 kept (issuance(h) known before mining). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E2a, v3 program set only). lp4-execution issuance.rs implements issuance(h) with S0 = CP 15, H_half = CP 52 and S_tail = CP 53. The v3 host credits it and passes it to closeBlock(tips, issued), which does not recompute it. Spec-1 program sets keep crediting CP 15 every block through closeBlock(tips). The supply term 2·S0·H_half + S_tail·2^64 is used for a genesis running the v3 set, both by genesis_builder.rs (checked at build and on every candidate) and by ParamGate R23 (lp1-node gate.rs `future_issuance`, through IdentityFacts.m3_issuance, which admission.rs sets from the sysCodeHash). Every other program set, a CP schema v2 genesis naming the continuous or v1 programs included, keeps CP 15 · H_END (u64::MAX blocks when continuous) in both places.

### D-V3-20

Fee market: tip share to proposer and base-fee floor (consensus); fee-ordered pool with replacement, eviction and per-sender quota (node-local). O-20=b: a signed proposer receives 50% of tips directly; the other half enters the shared pot. Fallback sends all tips to the pot. Base-fee floor 10^7 wei. Fee ordering, replace-by-fee and lowest-fee eviction are node-local.

Approved within the owner rule; OWNER-CHOICE (+ NODE-LOCAL pool). Principle check: the chosen half-tip split preserves shared precommitted rewards; pool policy is node-local. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

Implementation note (V3-PR-E2a, the tip split; v3 program set only; the base-fee floor and the pool are other PRs). closeBlock takes share = floor(tips · gamma_tip_bp / 10^4) (CP 59) of a signed block's tips and pays it to tx.origin, the block's signed proposer, through payTo, exactly as the proposer part of a pot is paid. A HELD record's reward address receives it. An ACCRUING record's bond takes it first (EARN-IN P1), and the excess is paid. An absent record, a tomb (a proposer that reported itself in its own block) or a failed PAY burns it. The rest of the tips enters pot h. A fallback block (no proposer) leaves all its tips in the pot, and a final block still burns its tips. Using payTo, not a direct PAY like the V3-17 reporter cut, is an implementation choice that the owner may want to confirm: it keeps every proposer earning on one rule. Both rules conserve value and keep P6, since the share is fixed by the block's own transactions and proposer. A direct PAY would pay a tomb's reward address and never fill a bond, and would drop closeBlock's promotion path (GAS_ENVELOPE_V3 closeBlock 133,000, measured 132,825 with held_total going from zero to nonzero and a nonzero carry into pot h drained; 132,697 before the D-V3-15 carry lag). Pre-existing on every program set and not changed here: register and setRewardAddress accept a system program (0xC0C001, 0xC0C002) as a reward address, since the PAY probe sees it as durable; wei paid there afterwards stays in Registry's balance or as Escrow surplus outside every accounting term. The tip share is one more payment that can reach such an address. Refusing reward addresses in 0xC0C001..0xC0C0FF would be a separate program change for its own review.

### D-V3-21

Live-state caps: raise to an A13-verified middle and add a utilisation-priced, burned creation fee per new account/slot/code byte. O-21=b: candidate caps are ACCT=200000, SLOT=500000, CODE=32 MiB, subject to A13. Burn sender-paid creation charges F0*CAP/(CAP-used), based on running in-block utilization. F0_acct candidate=10^15 wei; F0_slot and F0_code remain to be calibrated in the resource/profile PR. Do not introduce a per-block growth griefing budget. Preserve copied user nonce, code and storage; apply the approved old-bond credits and new-miner bond deductions, and never lower caps below copied usage.

Approved within the owner rule; OWNER-CHOICE. Principle check: execution layer (Ch13); kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-22

Skip-on-revert for non-value mandatory SYSTEM calls. Deferred; no skip-on-revert change is authorized by this record.

Deferred; DEFER. Principle check: Ch13; would need Q4.3 proof. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-23

Height-gated system-code and CP replacement machinery (`cp_at(h)`, irregular state change at H). Compile a height-gated program/CP replacement path, with height-aware cp_at and fixed accounts, transition handling and replay/reorg tests. Ship dormant behind fork:3; this record does not authorize any live deployment or fork activation. Amends D72 only for an explicitly scheduled, tested replacement.

Approved within the owner rule; CONSENSUS-COMPLIANT (build now, use for later forks). Principle check: kept (code compiled in, selected by height). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-24

Per-block log caps 32 KiB/546 → 256 KiB/4,368; builder and pool model log caps. On v3 use 256 KiB/4368 block log caps and require RESP_MAX>=2*LOG_BLOCK_MAX. Model caps in builder and gas estimation. Keep old-chain caps unchanged.

Approved within the owner rule; CONSENSUS-COMPLIANT (+ NODE-LOCAL builder). Principle check: Ch13; kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-25

Derive worst-case SYS gas and block-validation time for the exact v3 programs and CP (EXEC-03/EXEC-05). Measure exact v3 execution and system-call gas envelopes, prove A13 with all added memory terms, and require ParamGate. Synthetic calibration is not a worst-case or security proof.

Approved within the owner rule; NODE-LOCAL. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-26

RPC `finalized` = persisted local irreversibility floor; reorg telemetry; honest finality docs (CONS-3). Persist finalized_h as a local irreversibility floor; retention floor=max(head-r_keep,finalized_h). Expose local semantics and reorg telemetry. It is not committee or mathematical finality and adds no signed checkpoint.

Approved within the owner rule; NODE-LOCAL. Principle check: kept (Q0.1: not consensus). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-27

Disclose seal/timestamp grinding bounds and a PREVRANDAO warning; a VDF beacon later. Disclose timestamp/seal grinding bounds and biasable PREVRANDAO. Do not claim a safe adversarial-work threshold. A VDF beacon remains deferred.

Approved within the owner rule; NODE-LOCAL (docs); beacon DEFER. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-28

Computed launch-readiness flag and a real network-admission path (CONS-4, MAT-02, EXEC-03). Compute readiness from specification admission, no conditions or waivers and completed launch gates; expose blockers. Never turn deployment branding into readiness evidence.

Approved within the owner rule; NODE-LOCAL. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-29

One dated v3 decision record plus normative amendments (CONS-7, MAT-03, MAT-04). Record these decisions and scoped normative amendments before v3 code. The approved owner rule is the standing authorization only for changes preserving P1-P10.

Approved within the owner rule; NODE-LOCAL (process). Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-30

Multi-threaded miner, localhost work port, network hashrate estimate (security-budget MISSED). Split only the miner own range into disjoint thread slices; local work RPC is off by default, validates own-range work and is unreachable through the site bridge. Account for threads in A13.

Approved within the owner rule; NODE-LOCAL. Principle check: kept (every thread inside the member's own range). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-31

Formal security analysis of PoCol (CONS-2). Deferred external common-prefix, chain-quality and economic security analysis; no PoW-equivalent security claim follows from implementation or tests.

Deferred; DEFER (external work). Principle check: —. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-32

Disclose the 4,000,000 PCOL genesis allocation and decide how it is held (ECON-05). O-32=b: disclose the four public genesis holders and development purpose; plan public time-lock custody and spending reports as owner-operated actions. Do not move, change or reallocate the owner personal funds in this work.

Approved within the owner rule; OWNER-CHOICE. Principle check: not consensus. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-33

How the re-genesis is run, and the policy for any later one (DEC-06). O-33=b: no user transactions after the agreed S; publish old-chain data and rebuild instructions; deliver Android migration before S. An export update needs the installed signer or authorized lineage; otherwise use a separate applicationId and pre-S transfers with deregistration. This is the last owner-only re-genesis; future changes use the fork schedule. All deployment remains on hold.

Approved within the owner rule; OWNER-CHOICE (+ NODE-LOCAL operations). Principle check: not consensus. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-34

Fork machinery: schedule v≥3, launcher upgrade path, `==2` → `>=2`, one dual-key rule predicate, program-set registry, off-node verifiers. Use v3_active(h)=active_version(h)>=3 || genesis.spec_version>=2 everywhere. Keep full nonce range for every v>=2 or nonceMode=1. Use a program-set registry and versioned CP schema. Retain sequential schedule versions: future v3-chain in-place forks append (2,H) as a no-op for nonceMode=1, then (3,Hprime).

Approved within the owner rule; CONSENSUS-COMPLIANT (infrastructure). Principle check: kept (schedule semantics unchanged). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

## Lock and activation strategy

All new host rules use `v3_active(h) = active_version(h) >= 3 || genesis.spec_version >= 2`. A spec-1 H15000 build must keep byte-identical verdicts and identity `bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110`.

Keep `CONSENSUS_RULES_VERSION=1`, `SPEC_VERSION=1` and `MAX_FORK_VERSION=2` through the dormant PRs. Never bump the rules version while H15000 must remain compatible. Re-pin dormant host changes as `neutral gate=spec:2`; the line still records `spec=1`, so that gate remains unspent. Boot-policy changes and behavior-identical local refactors in boundary files use plain `neutral` with a reason. Vector-directory edits require `neutral-vector` with an explicit `spec:2-dormant` reachability note and no gate. Keep the lock history append-only, merge main into shared branches and re-pin after intervening boundary changes; never force-push.

For the dormant sequence, this qualifies VA-SPEC §2.3: gate freshness comes from the constants recorded on earlier merged history lines, not the mere presence of an earlier `gate=spec:2` label. A line recording spec=1 does not spend spec:2.

`v3-preview` is test-only acceptance of spec-2 genesis files: identity bit 2 and the sources aggregate isolate preview builds with different consensus sources, admission allows only simnet, and packaging refuses preview artifacts. All restrictions must land together. The ordinary release decoder continues refusing spec 2 until activation.

Only the activation PR after implementation, verifiers, resource proofs and simulations are green raises both `SPEC_VERSION` copies to 2 and removes preview-only acceptance. Its `neutral gate=spec:2` line records `spec=2` and spends the gate. Subsequent v3 corrections need a new spec:3 genesis/gate, or a separately authorized rules-version bump only after H15000 retirement. In-place replacement machinery stays behind unspent `fork:3` with `MAX_FORK_VERSION=2` until its own future activation.

A future fork version 3 on a spec-1 genesis would also make the predicate true. That route is unreachable while release MAX_FORK_VERSION=2; activating it on H15000 is not authorized here.

The schedule remains sequential: on a v3 genesis with nonceMode=1, append version 2 before version 3; version 2 is a nonce-rule no-op there. No version skipping is needed or authorized by this record.

## Deferred and rejected work

V3-22 skip-on-revert and V3-31 external formal analysis remain deferred. A VDF randomness beacon is deferred. V3-R is rejected: no BFT/committee finality, operator-signed checkpoints, lowered retention marketed as finality, new PoW hash, per-identity hashrate cap or stake-weighted chain selection.

## Owner-operated allocation disclosure

The public H15000 invitation in `deploy/mainnet/h15000/network.pocol.json`, file `alloc.json`, records four initial balances of 1,000,000 PCOL each:

| Initial holder | Genesis allocation |
|---|---:|
| `0x155f1ebebd6c3fa428de98c6f855a77e91b7805a` | 1,000,000 PCOL |
| `0x854e2a71a9d84dc63290804fed3f51e79e1f5d93` | 1,000,000 PCOL |
| `0x8f8fff3e07669f41cb2c2e458a0282eeef858b7e` | 1,000,000 PCOL |
| `0xa28f9bd53bffdc2651832b238eaa96917f33adca` | 1,000,000 PCOL |

The owner describes the 4,000,000 PCOL allocation as personal funds intended to finance network development. These are genesis allocations, not asserted current balances. O-32=b calls for a public time-locked development-fund account and spending reports. That remains an owner-operated plan; this record neither moves funds nor invents a custodian, beneficiary, unlock schedule or completed lock. Publish those concrete details and reports when the owner arranges custody. The approved cutover copies permitted non-system accounts with nonce, code and storage preserved. It credits eligible HELD, non-tomb old bonds and debits each new genesis miner bond from its own durable reward balance. Protocol accounts are rebuilt; excluded balances, pending pots and burns are disclosed rather than copied. Follow C2–C6 of EARN-IN-SPEC.md and publish the transformation report. This record authorizes no live cutover.

## Related owner actions

The three seeds are to stop only after peer discovery works and enough independent nodes exist. The owner creates the real Android release signing key. The planned human security audit is after the real launch; that plan supplies no current audit or readiness evidence. These statements do not authorize live operations in this work.

## Validation and operational boundaries

Each implementation PR starts from newest main and merges only after all CI jobs are green and valid review findings are fixed or answered. Never skip or disable a test. Require old-vs-new spec-1 differential checks, exact A13 accounting and the unchanged H15000 identity. This documentation PR must merge before v3 code. Rehearsals and WAN soak run on private disposable networks only. Do not deploy, reinitialize the live network or choose S/date without the owner.

Candidate profile values, particularly WAN timing, caps, creation-fee coefficients and gas/memory bounds, are not measured results. The profile/resource PR must resolve them with evidence. No implementation or CI result proves PoW-equivalent security or closes the external audit.
