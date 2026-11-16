# PoCol v3 decisions — 2026-11-11

> **Changed by the owner decisions of 2026-11-13**
> ([OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md](OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)): PoCol launches a
> brand-new network from a fresh genesis, with new keys and new chain ids, and keeps nothing of the old network. The v3
> rules below are that network's release rules (CONSENSUS_RULES_VERSION 2, SPEC_VERSION 2, MAX_FORK_VERSION 3; see
> "Release rules and lock"). The final parameter values are in [FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md), whose §7
> maps every changed owner item. Items changed or superseded:
>
> - **O-0** (v3 rides the held earn-in re-genesis): superseded. v3 launches as a brand-new network from a fresh
>   genesis; there is no re-genesis or cutover (answers 1-5).
> - **O-6** (block time 30 s): changed. Difficulty adjustment stays, with a 10 s average block time (answer 10a).
> - **O-10** (contestable seats when the set is full): superseded. There is no seat contest; registrations are
>   rate-limited per block, and an inactive member is removed after about an hour, only when the set is full, and may
>   return (answers 10, 10b; Q2(a)).
> - The member limit **M_max** (V3-07, 256): changed to **M_max = 10,000** (second follow-up, answer 2).
> - **O-14** (probation pool of M_max/4 slots with a contest inside the pool): superseded. There is no separate
>   probation pool: newcomers accruing their 20 PCOL bond count within M_max = 10,000. New miners register for free
>   and the first 20 PCOL of their rewards is held as their bond, forfeited on misbehaviour (answers 10, 10c; second
>   follow-up, answer 2).
> - **O-19** (issuance, M3): changed. S0 = 4 PCOL per block at the 10 s average block, halving every 4 years
>   (H_half = 12,623,040 blocks), with a tail of S0/32 = 0.125 PCOL (answer 7; second follow-up, answer 1).
> - **O-21** (raised state caps with a burned creation fee): changed. There is no creation fee; new accounts pay
>   ordinary gas only (answer 10d), and the caps are ACCT 1,000,000 / SLOT 4,000,000 / CODE 2^30 (Q1 = a).
> - **O-32** (the 4,000,000 PCOL genesis allocation): superseded. The genesis gives each of the four founding miners
>   1,000,000 PCOL, its 20 PCOL bond included (answers 8, 9, 15; "Founders' allocation" below).
> - **O-33** (how the re-genesis is run): moot. Nothing carries over, so there is no freeze height S, old-chain
>   export or migration (answers 1-4).
>
> Implemented in FN-2a (D-V3-21: the creation fee and F0_* deleted), FN-2b (D-V3-10, D-V3-14 and T_PROB: no seat
> contest, probation pool or deadline; inactivity removal only at a full set, atomically through registerReplacing,
> owner decision Q2(a); the activeSlice view; CP schema v2 at 59 fields), FN-4 (the committed network profiles and the
> genesis builder's checks) and FN-5 (D-V3-23: the cap-raise path, fork version 3). Owner-rule checks (collaborative
> consensus, the template block and P1-P10 unchanged): FRESH-NETWORK-CP.md §6, C1-C10 pass.

Status: records the owner approval of 2026-11-08, as changed on 2026-11-13, and its implementation constraints. It is a decision record, not a deployment approval: the fresh network's genesis is made only at the ceremony, after the rehearsal, with the owner's approval.

## Owner rule and scope

> أوافق على تعديل التي لا تغيير خوارزمية الاجماع بدون اي تحفظ. أما التي تحتاج تغيير في خوارزمية الاجماع فأوافق على التغيير الذي لا يغيير فكرة الاجماع بالتعاون ولا يغير فكرة الكتلة النموذج أو تغيير أي مبدأ من المبادئ الاساسية ل PoCol.

Node-local changes are approved without reservation. Consensus changes are approved only while preserving collaboration, the common immutable template, and every basic PoCol principle P1–P10 in [V3-DESIGN.md](V3-DESIGN.md). Stop and ask the owner before crossing that boundary or deciding an unanswered owner question.

For compliant items with the choices below, this standing owner authorization supplies the required owner decision; it supersedes the proposal-before-implementation gate of the repository root `README.md` (the bullet "Consensus is owner-gated") for those items only. That bullet reads: “**Consensus is owner-gated.** A change to a consensus rule, a `CP` parameter or a wire encoding needs an owner decision. Write a proposal in `development/audit-2026-10-22/proposals/` with the alternatives and their effects, and do not implement it before it is approved.” It does not approve rejected mechanisms, deferred work, unanswered questions reserved to the owner or live deployment. Earlier decision records stay as dated history; the amendments below take precedence over them.

Sources: [approved design](V3-DESIGN.md), [dated owner answers](V3-OWNER-DECISIONS-2026-11-08.md), and [consensus boundary policy](../VA-SPEC.md). The launch path is [V3-DESIGN.md](V3-DESIGN.md) §3 (fresh v3 genesis launch); the final values are [FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md).

## Owner choices

| Question | Item | Decision |
|---|---|---|
| O-0 Vehicle for v3 | — | **a**: v3 rides the held earn-in re-genesis, and the same release builds the in-place fork machinery for later changes. **Superseded** (FRESH-NETWORK-CP.md §7): a brand-new network from a fresh genesis; the fork machinery ships in its release. |
| O-6 Block time | V3-06 | **b**: 30 s (D_att 150, dFbWait 10, Δ_rel 13), valid if the measured WAN Δ ≤ 2 s; otherwise 60 s (option c). **Changed** (FRESH-NETWORK-CP.md §7): T_blk 10 s, D_att 90, dFbWait 8, Δ_rel 7. |
| O-10 Contestable seats when the set is full | V3-10 | **b**: contest by work over one day with a statistical margin (θ=1/8, 3σ), only when all seats are full, with the displaced member refunded, and only together with V3-02. **Superseded** (FRESH-NETWORK-CP.md §7): no seat contest; removal at a full set only. |
| O-11 Inactivity removal delay Θ_inact | V3-11 | **b**: 1 hour |
| O-12 Registration bond B_reg | V3-12 | **a**: 20 PCOL |
| O-13 Probationers as proposers | V3-13 | **b**: yes, once the accrued bond is at least B_reg/4 (the all-probationer exception below B_reg/4 is retained: owner decision 2026-11-12, option a) |
| O-14 Probation pool | V3-14 | **b**: M_max/4 slots (64), contest by work inside the pool, T_PROB = 30 days, both as CP fields (requires V3-09). **Superseded** (FRESH-NETWORK-CP.md §7): no probation pool and no deadline. |
| O-15 Uncovered share pot | V3-15 | **b**: carried into the next block's pot at closeBlock |
| O-17 Reward for reporting misbehaviour | V3-17 | **b**: 25% of the burned bond to the reporter, 75% burned |
| O-18 Evidence window and bond delay | V3-18 | **b**: 1 hour, with U = E_win + D + a margin |
| O-19 Issuance schedule | V3-19 | **M3**: M1's rate (2 PCOL per 60 s, scaled by T_blk) halving every 4 years, with a tail of S0/32. **Changed** (FRESH-NETWORK-CP.md §7): S0 4 PCOL per 10 s block, H_half 12,623,040. |
| O-20 Fee market | V3-20 | **b**: the proposer receives 50% of tips directly, the rest stays in the shared pot, base-fee floor 0.01 gwei; the pool becomes fee-ordered with replacement and eviction (node-local) |
| O-21 State caps | V3-21 | **b**: raise the caps (A13-verified) and add a burned creation fee that rises as the cap fills. **Changed** (FRESH-NETWORK-CP.md §7): no creation fee; ACCT 1,000,000 / SLOT 4,000,000 / CODE 2^30. |
| O-32 The 4,000,000 PCOL genesis allocation | V3-32 | **b**: publish the holder addresses and their purpose, and move the development fund into a public time-locked account with published spending reports. **Superseded** (FRESH-NETWORK-CP.md §7): 1,000,000 PCOL to each of the four founders, its bond included. |
| O-33 How the re-genesis is run | V3-33 | **b**: no user transactions after S; publish the old-chain data; ship an Android key-export build first; and commit publicly that this is the last owner-only re-genesis, with later consensus changes going through the fork schedule. **Moot** (FRESH-NETWORK-CP.md §7): nothing carries over. |

## All-probationer exception — owner decision (2026-11-12)

An earlier version of this section (commit 3d526e0, 2026-11-11) recorded an owner clarification of O-13: “Retain the all-probationer exception below B_reg/4.” The audit of 2026-11-12 found no verifiable owner source for that text, so the question was put to the owner directly on 2026-11-12.

**The owner chose (a) and keeps the exception.** The verbatim answer is «أوافق على (أ)، أبقِ الاستثناء» (“I approve (a); keep the exception”). The question and the answer are recorded in [V3-OWNER-DECISIONS-2026-11-12.md](V3-OWNER-DECISIONS-2026-11-12.md).

What the decision means:
- Under O-13=b, an ACCRUING member normally enters the proposer draw only once its bond is at least B_reg/4.
- When that filter would exclude every draw member, the earn-in D1 rule ([incentives.md](../m1/reference/incentives.md), Earn-in rules E6) applies instead. All those probationers enter the deterministic, share-weighted draw, so signed blocks continue.
- In that case a drawn proposer may hold less than B_reg/4, possibly zero. The B_reg/4 minimum loss for a type-1 equivocation therefore does not hold then.
- The v3 code implements this (pocold `DrawWeights`), so no code change follows.
- The liveness interplay with all-live membership is disclosed in [threat.md](../m1/reference/threat.md) (A6).

## Decisions D-V3-01 through D-V3-34

### D-V3-01

Give a nonce range to every live (non-tombstoned) member whenever the nonce domain is unbounded; removes the permanent-halt and range-starvation path (MISSED share inclusion, ECON-03). In unbounded mode (nonceMode 1, the only mode admission accepts, R24), the parent-state active list excluding tombstones owns the ranges regardless of recent-share activity. Membership remains independent of attempt.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch8 (activity gating) but preserved; P4/P5 kept, Q3.5 improved. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-02

Late share credit: proposers of blocks j+1..j+D carry uncredited valid shares of template j as body evidence; late credit only fills the pool up to κm, so it never dilutes header shares. Signed bodies may carry RLP([3, be64(j), [n_1<...<n_k]]) for ancestor templates j in [max(1,h-D),h-1]. Check target, range owner, exclusion of the winning nonce and header shares, branch deduplication and C_header(j)+C_late(j)<=kappa*m. Record late credit after recordResult and before settlement; lastShare=max(lastShare,j). Roll credit back with the branch. This never dilutes header shares.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch6 but preserved; P2/P5 kept, P6 strengthened (winner no longer alone decides credit). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Implementation note (V3-PR-E3; pocold chain.rs `late_fields`/`late_window`/`late_check`, apply.rs, PoColSystemV3.sol MinerRegistry and ShareLedger `recordLate`, prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md "v3 late share credit"). Kind 3 is valid on the v3 program set (v3_active and the v3 sysCodeHash of a specVersion-2 genesis, the only genesis admission accepts); elsewhere it is "15 evidence form". Checks, in order: form (j and each nonce exactly eight bytes, 1 <= k <= S_max, strictly ascending), window, then on template j of the carrying branch the cap C_header(j) + C_late(j) + k <= kappa*m (C_header counts the winner's share; for j = h-1 from the parent's ParentResult, otherwise the parent state's ledger total, which already includes the branch's late credits) and per nonce: below nMax_j (owner from block j's stored ranges), not the winning nonce, not a header share, not credited on the branch, not credited earlier in this block, on target. Several kind-3 items may name one j in one block (the design's "not repeated in this block" is the nonce rule); the cap counts the block's earlier credits. The effect is recordLate(j, owners) in body order among the evidence, after recordShares/recordResult of h-1 and before settle(h-D). Branch deduplication stays on the Entry without a new field: an offloaded entry keeps its body's kind-3 items, a slimmed one drops them and `unslim` (and a rebuild's restored copies) read them back from the journal body, so Entry's layout, the journal and checkpoint bytes are unchanged. Coupling with V3-15 (principles-review finding 4): since V3-PR-E2a, the uncovered pool part of pot j (its base burn) goes to the carry at settle(j). It enters the pot closed in the next block, because the carry is lagged by one block. recordLate for j always precedes settle(j), so that carry is computed after every late credit to j. Each pot is fixed before its template is built (see the D-V3-15 implementation note). The lag also makes the last carrier incentive-neutral. Block h is the last carrier of j = h - D. Had pot h received settle(h - D)'s base burn, h's signed proposer would have been paid to leave out other members' late shares of h - D, exactly the items the node-local policy below includes first. Under the lag that value reaches pot h + 1, whose block can no longer carry them (the D-V3-15 incentive finding and decision, option (b)). The late window [max(1,h-D),h-1] is unchanged. Node-local part (review of E3): the share cache keeps, for the next carrier, the branch's template of each height in [max(1, H+1-D), H] and the two newest templates of the production height (never the last D templates the node locked, which dead attempts could fill); a template locked again leaves the cache and takes its shares back; a new valid share of a cached template is relayed within the per-template cap of 4·S_max; the proposer fills E_max oldest template first (j = h-D has no later carrier). A received template whose height is not its parent's plus one is punished (item 2) before its evidence is judged, and kind-3 rules judged at such a height are the verdict "15 ls window". That height check is node-local: such a template could otherwise be locked and mined although its block can never be valid (item 2); no verdict changes.

### D-V3-03

Multi-hop relay of valid templates and shares.

Approved within the owner rule; NODE-LOCAL (already in tree at 53a9e87). Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-04

Fork choice uses timeliness-qualified signed precedence, with the specified arrival allowance for attempt-0 fallback. At first import, a signed block is timely iff this node first locked its signed template (validated as minable by T_j: drawn signer, window, item 16, body rebuilding to its UT; or its own) by T_j+Phi, because T_j is the template deadline; a shell merely received never counts, and a slot (parent, signer, height) whose signer equivocated counts no template. Only if this node never locked the template, or its slot is poisoned, is the block's own arrival compared with T_j+Phi (the earlier time counts). Template first-lock times are local, bounded (one TemplateID per slot) and not persisted; the bit they give is frozen at first import. Attempt-0 fallback is timely iff arrival>=T_j-(Delta+Phi); later attempts are timely. On v3 only, sibling key=(timely?0:1,signed?0:1,importSeq,blockHash), compared at first path divergence after cumulative work. Persist first-import time across restart. Amends D133; validity is unchanged.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch10 (amends D133) but preserved; D15 signed precedence kept for honest proposers; Q2.8 improved. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-05

Δ_rel default derived from measured WAN Δ (not lowered to 7 s); per-proposer template-latency counters in `pocol_status`.

Approved within the owner rule; NODE-LOCAL. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-06

Recalibrate T_blk, D_att, dFbWait, Δ_rel from the ParamGate inequalities on measured WAN latency. O-6=b: T_blk=30 s, D_att=150 s, dFbWait=10 s, local delta_rel=13 s, subject to measured WAN Delta<=2 s and phi<=2 s and ParamGate. Otherwise use the approved 60 s fallback and recompute time-derived block counts. These are candidate profile values until measurement and resource proofs.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameters (Ch5/Ch11); kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Changed (FRESH-NETWORK-CP.md §7, owner answer 10a of 2026-11-13): T_blk = 10 s, D_att = 90 s, dFbWait = 8 s; the declared local timing is (Δ, φ, t_exec) = (1, 1, 1) with Δ_rel = 7 s, or (2, 1, 1) with Δ_rel = 8 s if the rehearsal measures more than 1 s. ParamGate admits the CP at both. The rehearsal measures the values (launch evidence T1-T8).

### D-V3-07

M_max 64 → 256; compute draw weights from ledger owners instead of M_max×W_w record reads. M_max=256 is subject to the A13 proof. Iterating ledger owners must give the same draw weights as reading every record.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: parameter; P8 loosened. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Changed (FRESH-NETWORK-CP.md §2, owner second follow-up of 2026-11-13, answer 2): M_max = 10,000, with REG_RECORDS_MAX 24,576 (R16). The node memory and CPU work for it is FN-1 and FN-3 (C6); A13 runs in store mode.

### D-V3-08

REG_OPS: separate per-block maintenance budget (setRewardAddress/topUp) and at most one maintenance op per identity per block. Use separate REG_MAINT_MAX=8 per-block maintenance budget and at most one maintenance operation per identity per block; registration/deregistration/claim/removal retain REG_OPS minus maintenance used.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: Ch9; restores P8/Q5.2. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-09

R_max entry slot: reactivate gets its own budget and a cooldown ≥ U; R_max 1 → 4. Use R_max=4 registration entries, a separate R_react=4 budget and per-record C_react>=U cooldown. Reactivation remains after W_act cooldown and strictly before the refund sweep. Require U>=W_act+2.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: Ch9 + parameter; restores P8/Q5.2. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Client note (G3b', pocold wallet.rs `rejoin_step_with`, on the FN-2b rules): on the v3 network every live member keeps a nonce range (D-V3-01; the network builder always sets nonceMode 1), so a status-1 member is always in the active set and rejoin applies only after a voluntary exit. The wallet's rejoin reads the record's own clock (lastReactHeight, w1 bits 160..223): the window after an exit opens at max(exit + W_act, lastReactHeight + C_react) and closes at dueHeight (the v3 set has no probation deadline, FN-2b C3); a window the clock leaves empty is reported at once with that cause (`RejoinStep::ClockMissed`: the next allowed height and the refund height). The `Exit` and `Cooldown` steps of a status-1 member outside the active set (an exit offered only once it leaves min(U - W_act, REACTIVATE_ATTEMPTS + 1) blocks of window) are defensive code: since phase 6a admission takes only nonceMode 1 (R24), so only the wallet's pure-function tests reach them. At a full set an exited record is `RejoinStep::Full` (FN-2b C4): no reactivation is quoted or signed, since reactivate() reverts with Capacity.

### D-V3-10

**Superseded by owner decision 10b (2026-11-13; [OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md](OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)) and its follow-up Q2(a): no seat contest; inactive members are removed after about an hour and may return, only when the network is full.** FN-2b (FRESH-NETWORK design C2 and C4) deleted the seat contest and everything that served it: registerContest, contestStatus, contestThreshold, epochTotals, the per-record epoch counters (w0 bits 216..255) and the epoch totals (Registry slot 10), the CP schema v2 fields E_epoch, theta_bp and k_sigma, the contest vectors (tests/v3/contest-vectors.json), its eighteen mutants and pocold's quote_contest. In their place (C4): removeInactive reverts unless the set is full (active == M_max), and registerReplacing(reward, victim), payable 0..B_reg, registers the sender into the seat of a victim removeInactive could remove at that height (no credited share and no (re)activation within Theta_inact = 360 blocks in the fresh profile; more than M_min active), with two REG_OPS units and one R_max entry; the victim exits as by deregister (refund after U, no burn, payable and punishable until then) and may register again. This is not a contest: no performance measure and no threshold, only an atomic removal and registration that anyone could already do in one block. The wallet picks the victim at random among the removable members (pocold wallet.rs replace_plan, the node's pocol_getRemovable). Owner-rule check: collaborative consensus (shared shares and rewards), the template block and P1-P10 are unchanged; removal reasons shrink to exit, inactivity at a full set and punishment (P8). Pass. The disclosed exposure (threat.md): at a full set of equal small miners about a third are removable at any moment, and the R_max entries are ordered by priority fee, so honest entry at a full set competes with funded re-registration. A second consequence of the gate, also disclosed (threat.md A6): below a full set an offline but live bonded member is never removed, so the all-probationer exception's fallback-only period (D-V3-13) lasts until a probationer reaches B_reg/4, not until Θ_inact; this changes no consensus rule (the period existed before, and below a full set the removal path simply does not apply), and the rehearsal records how long such periods last. Wallet (review of FN-2b): a member replaced (or exited) at a full set is shown as waiting for its refund, and Rejoin mining is refused, because reactivate() reverts with Capacity while the set stays full (wallet.rs RejoinStep::Full); a newcomer's victim is drawn among the removable members not already named by a registerReplacing in the node's pool. The text below is the record of the superseded decision.

Contestable seats when the set is full: displace (with refund) a member whose credited work in the last full epoch is statistically below threshold. O-10=b: contest only a full set, with previous full-day epoch count below E-3*sqrt(E), E=(1/8)*Tprev/active, with deterministic integer rounding. Victim must be live, active, past grace and active throughout the previous epoch. Refund after U; V3-02 is mandatory first. Validate honest-member eviction rates by simulation.

Approved within the owner rule; OWNER-CHOICE. Principle check: touches P8 (new removal reason) and P7; preserved only with V3-02 and the statistical margin. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Implementation note (V3-PR-E4; deleted by FN-2b with the contest). The contest's implementation, gas figures, vectors, mutants and the open points recorded for it are in this file's history (for example at commit 52b2507).

### D-V3-11

Inactivity eviction delay Θ_inact, set in hours. O-11=b: one hour, converted into blocks using the final measured T_blk.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameter. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-12

Registration bond B_reg. O-12=a: B_reg=20 PCOL; the genesis builder, wallet and reward gateway read B_reg from CP[16], never a mirrored literal.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameter (Ch9). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-13

Probationary (ACCRUING) members as proposer candidates. O-13=b: an ACCRUING member may enter the proposer draw once bond>=B_reg/4. The existing all-probationer exception below B_reg/4, used when the filter would otherwise exclude every draw member, is retained (owner decision 2026-11-12, option a; see 'All-probationer exception' and V3-OWNER-DECISIONS-2026-11-12.md). Keep the share-weighted deterministic draw.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch1 candidate set; P3 kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-14

**Superseded by owner decision 10c (2026-11-13) and its second follow-up (answer 2): no separate probation pool and no probation deadline; newcomers accruing their 20 PCOL bond count within M_max = 10,000.** FN-2b (FRESH-NETWORK design C2 and C3) deleted the pool, its contest and the deadline: the CP schema v2 fields P_MAX and T_PROB, ACC_COUNT (Registry slot 8, never written on the v3 set), POOL_OUT, accruingCount, the beginBlock sweep's probation exit and topUp's and reactivate's deadline checks (the v3 set has no probation state). Kept: the ACCRUING state, the earned bond (earn-in P1, incentives.md: settlement fills it before paying), promotion to HELD at B_reg and the B_reg/4 draw exclusion (D-V3-13). ACCRUING records never exit by time; topUp and reactivate work without a deadline. With P_MAX gone, every payee of a pot may be ACCRUING: GAS_ENVELOPE_V3 settle is re-measured with 258 payees all ACCRUING at bond 0 promoted in one call: 11,956,401 with held_total 0 at the first accrue (11,939,301 with a bond held), so GAS_ENVELOPE_V3 settle and settleFinal are 11,957,000, and the v3 gas domain drops the M_max term for the payee bound S_max + 3 (admission.rs in_gas_domain). Owner-rule check: collaborative consensus, the template block and P1-P10 are unchanged. Pass. Disclosed exposure (threat.md, A6 and the D-V3-16 entry): up to M_max members may be ACCRUING, so an ACCRUING winner accused with type-2 evidence can make a whole pot INVALID more often, and members at bond >= B_reg/4 are drawable proposers; measured in the rehearsal (M4). The text below is the record of the superseded decision.

Probation pool size and in-pool contest; P_MAX and T_PROB become CP fields set in time. O-14=b: P_MAX=M_max/4 (64 at M_max=256), T_PROB=30 days as CP fields. Use the same work statistic for contests within the ACCRUING pool, refund accrued bond and conserve ACC_COUNT. V3-09 precedes expansion.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch9; P8. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Implementation note (V3-PR-E4; deleted by FN-2b with the pool). The pool contest's implementation (POOL_OUT, ACC_COUNT, its vectors and mutants), the settle bound it needed and the owner decision of 2026-11-13 on the pool victim (V3-OWNER-DECISIONS-2026-11-12.md, third decision) are in this file's history (for example at commit 52b2507).

### D-V3-15

Carry the uncovered share pot into the next pot at closeBlock. O-15=b: send uncovered base-burn share remainder to carry at settlement; roundBurn remains burned. closeBlock(j) adds and zeroes carry after that block phase-0 settlement, so pot j is fixed by its template. Burn carry at a finite final block. Escrow invariant includes carry.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch7; P6 kept (carry fixed before mining). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Implementation note (V3-PR-E2a; prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md "v3 issuance, carry and tip split"). The carry is lagged by one block (decision below): RewardEscrow slot 4 + (t & 1) holds the carry into pot t. settleOne's distribute in block h adds the base burn R - A to the carry into pot h + 1; roundBurn, payBurn, penaltyBurn and the whole-pot INVALID burn still burn. closeBlock(tips, issued) of block h records pot h = issued + (1 - gamma_tip)·tips + the carry into pot h and zeroes that slot. So at most two carries exist: inside block h the carry into pot h (left by block h - 1) and the one block h's settle(h - D) is writing into pot h + 1; between blocks only the carry into the next pot, which starts at zero (genesis writes no Escrow storage). Pot h therefore depends on block h - 1 for its carry and on block h for its issuance and tips. Its carry is the base burn of settle(h - 1 - D) in block h - 1's phase 0, which runs after every late credit to h - 1 - D (recordLate precedes settle, principles-review finding 4). Changing late-share inclusion after block h never changes pot h; the model and the bytecode test this. At a final block, finalize burns both outstanding carries with the tips: the carry into pot F, which no pot receives, and the carry of F's own settle(F-D). settleFinal (slot 1 set) burns its base burns directly, so nothing is carried past F. The host invariant counts both carries (need = Σpending + held + burned + carry into h + carry into h + 1). After phase 2 it refuses a carry into pot h, a carry into pot h + 1 before any settlement ran (h <= D), and any carry after a final block. The model and the bytecode compare pots up to h both as closeBlock(h) recorded them and as settle later distributed them, and both checks fail on a planted mutant that adds half of a settle's base burn to an already-closed pending pot (the shape safety finding 8 rejects).

Incentive finding and decision (review of V3-PR-E2a; decided under the owner's rule that consensus changes keeping collaborative consensus, the template block and every PoCol principle are approved). Block h is the last block that can carry late shares of j = h - D: the window is [max(1, h-D), h-1], and recordLate precedes settle(h-D) in h's phase 0. As first implemented, pot h received settle(h-D)'s base burn. While C_j < kappa·m, each late share of j that h's signed proposer left out raised R_j - A_j by about R_j/Q and moved that value from the share's owner into pot h. The proposer then received gamma_bp/10^4 of it (5% at 85/10/5), plus alpha_bp/10^4 if it also won h, plus its own pool fraction of pot h. Size: at S0 = 1 PCOL, R ≈ 0.85 PCOL and Q = 64, omitting 63 late shares added ≈ 0.84 PCOL to pot h and ≈ +0.042 PCOL to its proposer, close to doubling its 0.05 PCOL proposer part. The model scenario (D = 4, eight late shares of another member left out) gives the proposer +0.010625 PCOL while that member loses 0.2125 PCOL. That paid the last carrier to drop other members' late shares, exactly the ones the node-local policy includes first, and weakened P6 (V3-02's aim that the winner no longer alone decides credit). Chosen: option (b), lag the carry by one block. settle(h-D)'s base burn now enters pot h + 1, whose block can no longer carry late shares of h - D because the window has closed. The proposer of h therefore gains nothing in its own pot from omitting LS(h-D). tests/v3_model.py check_last_carrier_gains_nothing and tests/v3_contracts.rs last_carrier_gains_nothing_by_omitting_late_shares compare pots h and h + 1 with and without the omission: pot h is identical, the omitted value appears in pot h + 1, and the proposer is paid the same. Both checks fail on the planted carry-not-lagged, the previous rule. Why every principle is kept: (1) collaborative consensus is unchanged, because the share ledger, Q = max(C, kappa·m) and the no-dilution cap are untouched. (2) The template block is unchanged. The full late window [max(1, h-D), h-1] stays, so no honest late share loses a carrier. (3) P6 is kept and strengthened. Pot h + 1's carry is fixed by block h, before template h + 1 is built, so it is fixed before mining, more strictly than before. Its proposer had no say in it. (4) Value is conserved. Escrow holds every outstanding carry; the invariant, the model and the harness count both slots, and the final block burns both. (5) Only the v3 programs and their sysCodeHash change. Costs: one more Escrow slot can be nonzero between blocks, which R18 counts (and two carry keys per block). finalize can now zero two slots; it measures 61,932 on its worst path, so GAS_ENVELOPE_V3 finalize is 62,000. Implementation choice: two slots keyed by the parity of the receiving pot, rather than one carry slot copied forward at closeBlock. Each slot is written only in block t - 1 and drained only by closeBlock(t) or finalize, no copy is made, so closeBlock's gas and worst path are unchanged. Settle's write is still the zero-to-nonzero store it was. Not chosen: (a) Accept and document the gain, measuring it in SimNet T7c(S). This keeps a known, targeted reward for omission. (c) On v3, close j's late window at j + D - 1. This also removes the gain, but it shortens the late-credit window by a block, so an honest late share of j found by block j + D's proposer could no longer be credited. It also changes kind-3 validity (pocold late_window and the vectors), which is a wider consensus change than which pot receives the carry. What remains after (b) is the ordinary diffuse interest of any miner in a later pot. An omitter expects about its hashpower fraction of pot h + 1, not knowing whether it will win or propose it. That interest existed before for every carrier j+1..j+D-1, under every option, and is inherent to carrying rather than burning the base burn (O-15=b). V3-PR-H's SimNet T7c(S) (share omission with late credit) still measures omission with late credit.

### D-V3-16

Type-2 evidence burns only the offender's parts; whole-pot burn kept when the accused is ACCRUING. Settle normally after punishment; tombed offender winner/proposer/share parts burn, while honest contributors are paid. Retain whole-pot burn if the accused was ACCRUING below B_reg at block j, recording that fact at recordResult rather than reading later state.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: Ch7; P6/P7 strengthened. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Implementation note (V3-PR-E2b; prototypes/lp4-execution/contracts/REGISTRY-LAYOUT.md). The fact recordResult can commit is about pot j's winner, the only identity known when the result is recorded, and evidence may arrive later; so the whole-pot burn applies when the accused is pot j's winner and that winner was ACCRUING when the result was recorded (bit 160 of the winner word). Evidence against any other member of block j (a share owner, the signed proposer) settles the pot normally, burning that member's parts. To record the fact as of block j even when the evidence is in block j+1, the v3 host runs recordShares(h-1) and recordResult(h-1) before the block's evidence in phase 0. Several pieces of type-2 evidence may name one pot. Whole-pot burn for a non-winner accused (which would let a zero-bond equivocator burn an honest winner's pot) is not implemented.

### D-V3-17

Reporter reward ρ of a burned bond. O-17=b: 25% of punished bond goes to the carrying signed proposer reward address; burn 75%. A fallback has no reporter and burns the whole bond. Self-reporting still loses 75%.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch7/Ch9; P6 kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Implementation note (V3-PR-E2b). ρ is CP schema v2 field rho_bp (CP 58 since FN-2b; 63 after FN-2a, 66 before it; 2500 in the draft profile). The host passes the reporter to punish; the program requires it to equal tx.origin, the carrying block's signed proposer (zero on a fallback). The cut is floor(bond·rho_bp/10^4), paid by PAY directly to the reward address of the reporter's record, whether that record is live, ACCRUING (no accrual into its bond) or a tomb (a self-report, or a proposer punished earlier in the same block); a failed PAY burns it. The rest burns.

### D-V3-18

Evidence window E_win and bond delay U, set in hours. O-18=b: E_win is one hour; U=E_win+D+margin, with U>=W_act+2 and existing retention constraints. Re-derive memory bounds.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameters. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-19

Use M3 issuance: a scaled initial rate, four-year halvings and a tail. O-19=M3: issuance(0)=0; otherwise max(k>=256?0:S0>>k,S_tail), k=floor(h/H_half). Rate is 2 PCOL/60 s, scaled by T_blk; halve every four years, tail S0/32. Bound 2*S0*H_half+S_tail*2^64<2^127. The host passes issued explicitly to closeBlock.

Approved within the owner rule; OWNER-CHOICE. Principle check: P6 kept (issuance(h) known before mining). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Changed (FRESH-NETWORK-CP.md §7, owner answer 7 and second follow-up, answer 1, of 2026-11-13): S0 = 4 PCOL per 10 s block (34,560 PCOL per day), H_half = 12,623,040 blocks (4 years), S_tail = S0/32 = 0.125 PCOL from era 5.

Implementation note (V3-PR-E2a). lp4-execution issuance.rs implements issuance(h) with S0 = CP 15, H_half = CP 52 and S_tail = CP 53. The v3 host credits it and passes it to closeBlock(tips, issued), which does not recompute it. The supply term 2·S0·H_half + S_tail·2^64 is used both by genesis_builder.rs (checked at build and on every candidate) and by ParamGate R23 (lp1-node gate.rs `future_issuance`, through IdentityFacts.m3_issuance, which admission.rs sets for the V3 program set, the only one a release runs).

### D-V3-20

Fee market: tip share to proposer and base-fee floor (consensus); fee-ordered pool with replacement, eviction and per-sender quota (node-local). O-20=b: a signed proposer receives 50% of tips directly; the other half enters the shared pot. Fallback sends all tips to the pot. Base-fee floor 10^7 wei. Fee ordering, replace-by-fee and lowest-fee eviction are node-local.

Approved within the owner rule; OWNER-CHOICE (+ NODE-LOCAL pool). Principle check: the chosen half-tip split preserves shared precommitted rewards; pool policy is node-local. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

Implementation note (V3-PR-E2a, the tip split; the base-fee floor and the pool are other PRs). closeBlock takes share = floor(tips · gamma_tip_bp / 10^4) (CP 54 since FN-2b; 56 after FN-2a, 59 before it) of a signed block's tips and pays it to tx.origin, the block's signed proposer, through payTo, exactly as the proposer part of a pot is paid. A HELD record's reward address receives it. An ACCRUING record's bond takes it first (earn-in P1, incentives.md), and the excess is paid. An absent record, a tomb (a proposer that reported itself in its own block) or a failed PAY burns it. The rest of the tips enters pot h. A fallback block (no proposer) leaves all its tips in the pot, and a final block still burns its tips. Using payTo, not a direct PAY like the V3-17 reporter cut, is an implementation choice that the owner may want to confirm: it keeps every proposer earning on one rule. Both rules conserve value and keep P6, since the share is fixed by the block's own transactions and proposer. A direct PAY would pay a tomb's reward address and never fill a bond, and would drop closeBlock's promotion path (GAS_ENVELOPE_V3 closeBlock 133,000, measured 132,825 with held_total going from zero to nonzero and a nonzero carry into pot h drained; 132,697 before the D-V3-15 carry lag). Pre-existing on every program set and not changed here: register and setRewardAddress accept a system program (0xC0C001, 0xC0C002) as a reward address, since the PAY probe sees it as durable; wei paid there afterwards stays in Registry's balance or as Escrow surplus outside every accounting term. The tip share is one more payment that can reach such an address. Refusing reward addresses in 0xC0C001..0xC0C0FF would be a separate program change for its own review.

### D-V3-21

**Superseded by owner decision 10d (2026-11-13; [OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md](OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)): no account-creation fee; new accounts pay ordinary gas only.** FN-2a deleted the creation burn and everything that served it: `creation_fees.rs`, `Halt::CreationFee`, the signed-maximum bound (option A) and its executed-gas halt charge, the CreationFeeBurn receipt annotation, the pool/builder/eth_estimateGas burn pricing, `pocol_quoteCreationBurn`, and the CP schema v2 fields F0_acct, F0_slot and F0_code (64 fields; gamma_tip_bp 59 → 56, baseFeeMin 60 → 57, R_react 61 → 58, C_react 62 → 59, E_epoch 63 → 60, theta_bp 64 → 61, k_sigma 65 → 62, rho_bp 66 → 63). Kept: the live-state caps ACCT_CAP and SLOT_CAP as A03 halts and CODE_CAP as a failed CREATE (CreateContractSizeLimit), as before, and every other A03 cap halt charges the declared gas (owner answer 11). The owner accepted the cap-fill risk (Q1 option a; disclosed in threat.md); the cap-raise path is FN-5 (C10, D-V3-23). A fee rule only: collaborative consensus, the template block and P1-P10 are unchanged. The text below is the record of the superseded decision.

Live-state caps: raise to an A13-verified middle and add a utilisation-priced, burned creation fee per new account/slot/code byte. O-21=b: candidate caps are ACCT=200000, SLOT=500000, CODE=32 MiB, subject to A13. Burn sender-paid creation charges F0*CAP/(CAP-used), based on running in-block utilization. F0_acct candidate=10^15 wei; F0_slot and F0_code remain to be calibrated in the resource/profile PR. Do not introduce a per-block growth griefing budget.

Approved within the owner rule; OWNER-CHOICE. Principle check: execution layer (Ch13); kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

The user bound of the superseded fee (open item C3-4 of 2026-11-12; owner decision 2026-11-12, option A: the burn bounded by the signed maximum fee, implemented and then deleted with the fee by FN-2a) is recorded in [V3-OWNER-DECISIONS-2026-11-12.md](V3-OWNER-DECISIONS-2026-11-12.md), second decision. `V3_LAUNCH_BLOCKERS` (`lp4-execution/src/admission.rs`) is empty; the gate and its `admit` call stay for any later blocker.

### D-V3-22

Skip-on-revert for non-value mandatory SYSTEM calls. Deferred; no skip-on-revert change is authorized by this record.

Deferred; DEFER. Principle check: Ch13; would need Q4.3 proof. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-23

Height-gated system-code and CP replacement machinery (`cp_at(h)`, irregular state change at H). Compile a height-gated program/CP replacement path, with height-aware cp_at and fixed accounts, transition handling and replay/reorg tests. Ship it compiled in and unscheduled, as fork version 3; this record does not schedule any fork. Amends D72 only for an explicitly scheduled, tested replacement.

Approved within the owner rule; CONSENSUS-COMPLIANT (build now, use for later forks). Principle check: kept (code compiled in, selected by height). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

**Implementation note (FN-5, fresh-network C10).** The machinery is built restricted to the cap fields, compiled in and
unscheduled: the fresh genesis schedule is `[(1,0)]`. Fork version 3 is the cap raise (`CAP_RAISE_VERSION`, MAX_FORK_VERSION 3): a schedule's (3,H) entry comes with
`capRaise` {ACCT_CAP, SLOT_CAP, CODE_CAP, SYS_SLOT_MAX} beside `forks` in devnet.json, and from H on
`ProfileIdentity::cp_at(h)` (`Spec::cp_at`) returns the genesis CP with those four fields replaced; every reader of them
(the USER-phase A03 halts, the per-block parent and candidate cap checks with SYS_SLOT_MAX, CODE_CAP in every engine
path, the pool and eth_estimateGas meters, pocol_getCaps) takes the block's height, and A13 is computed at
`cp_at(u64::MAX)`. No state changes at H: the genesis, its hash, the system programs and their ChainCaps copy of the CP
(none reads these fields) stay. ParamGate evaluates R1-R26 at both CPs (`admission::admit_with`; R24's CP comparison at
the genesis CP only) and refuses, as R24, a raise that lowers a cap, raises none or lacks its fork-3 entry. The consensus identity appends the raise's values. A release activates it by appending (2,H')
(a no-op under nonceMode 1) and (3,H) with the values (`pocol upgrade`; a scheduled raise never changes). A node without
it agrees on every block below H and refuses block H (`netVersion`). FN-5 was pinned `neutral gate=fork:3`.
The off-node verifiers take fork version 3 in the same release, so the activation release needs no client release of
its own: the extension (`forks.js` MAX_FORK_VERSION 3), the reward gateway (`pocol_rewards.py` MAX_FORK_VERSION 3; it
checks the advertised `capRaise` against the trusted schedule's fork-3 height), the
identity tool (`pocol_identity.py`, default bound 3, otherwise build-info's `maxForkVersion`) and the public gateways,
which pass `capRaise` through. A client release with bound 2 fails closed from the moment its node advertises the new
schedule, so no activation may be attempted with one in use. `pocol inspect-upgrade` shows the raise (`capRaise`), and
development/public-network/OPERATIONS.md has the activation and rehearsal-S2 procedure.

### D-V3-24

Per-block log caps 32 KiB/546 → 256 KiB/4,368; builder and pool model log caps. On v3 use 256 KiB/4368 block log caps and require RESP_MAX>=2*LOG_BLOCK_MAX. Model caps in builder and gas estimation.

Approved within the owner rule; CONSENSUS-COMPLIANT (+ NODE-LOCAL builder). Principle check: Ch13; kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-25

Derive worst-case SYS gas and block-validation time for the exact v3 programs and CP (EXEC-03/EXEC-05). Measure exact v3 execution and system-call gas envelopes, prove A13 with all added memory terms, and require ParamGate. Synthetic calibration is not a worst-case or security proof.

Approved within the owner rule; NODE-LOCAL. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-26

RPC `finalized` = persisted local irreversibility floor; reorg telemetry; honest finality docs (CONS-3). Persist finalized_h as a local irreversibility floor; retention floor=max(head-r_keep,finalized_h). Expose local semantics and reorg telemetry. It is not committee or mathematical finality and adds no signed checkpoint.

Owner decision 12 (2026-11-13): two node-local settings, the RPC `safe` depth k_safe = 6 (`--k-safe`, POCOL_K_SAFE) and the reorg alarm k_alarm = 60 (`--k-alarm`, POCOL_K_ALARM; pocol_finality.reorgsGeKAlarm), 6 and 60 blocks being one and ten minutes at 10 s blocks. A node opens only with 1 ≤ k_safe ≤ k_alarm and k_safe < K_hist, so state queries at `safe` are answered while safe = head − k_safe (when a heavier, shorter branch makes safe = finalized_h, they answer −32017 as at `finalized`).

Approved within the owner rule; NODE-LOCAL. Principle check: kept (Q0.1: not consensus). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-27

Disclose seal/timestamp grinding bounds and a PREVRANDAO warning; a VDF beacon later. Disclose timestamp/seal grinding bounds and biasable PREVRANDAO. Do not claim a safe adversarial-work threshold. A VDF beacon remains deferred.

Approved within the owner rule; NODE-LOCAL (docs); beacon DEFER. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-28

Computed launch-readiness flag and a real network-admission path (CONS-4, MAT-02, EXEC-03). Compute readiness from specification admission, no conditions or waivers and completed launch gates; expose blockers. Never turn deployment branding into readiness evidence.

Approved within the owner rule; NODE-LOCAL. Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

**Implementation note (H1, N5).** `prototypes/pocold/src/readiness.rs` replaces the literal `publicLaunchReady:false`
(`build.rs`, `launcher.rs`, `testnet.rs`) with a computed value: `publicLaunchReady` is true only when
`launchBlockers` is empty, and both are reported by `pocold build-info`/`pocol build-info` (the build alone, which
is never ready: blocker `no-network`), `pocold status` and `pocol_status` (the admission the node booted with),
`init-testnet` (the profile it admitted) and the launcher's network description (`pocol network`/`import`/
`upgrade`, from what genesis.spec.hex commits to; the launcher admits nothing, so it is never ready). Each blocker
has a stable `id` and a `detail`:
- build: `consensus-unpinned`;
  `test-features` (caps-test, crash-test, io-fault); `v3-launch-blocker` (each `V3_LAUNCH_BLOCKERS` entry, none
  today); `plan-gate-<id>` (each acceptance-ledger gate of `development/public-network/PLAN.md` not marked
  closed: every `PLAN_GATES_OPEN` gate; a test keeps `PLAN_GATES_OPEN` equal to the ledger);
- network: `v3-inactive` (a genesis below specVersion 2, which admission refuses anyway, R24; fork 3 is the cap raise,
  not a route to v3); `network-not-release-pinned` ((chainId,
  genesisHash) not in `NETWORK_PROFILES`, the H2 hook, empty today); `admission-scope` (not `specification`);
  `admission-condition-<id>` and `admission-waiver-<rule>` (each condition and waiver); `profile-kind` (not the
  network profile on a network node, R22); `admission-not-booted` (always), `admission-unrecorded` and
  `genesis-unreadable` (launcher only). The admission devnet.json records about itself is operator metadata that
  nothing commits to, so the launcher never treats it as evidence: `admission-not-booted` always blocks, and the
  recorded scope, condition ids and waiver rules are reflected only as further blockers, in the gate's id shape
  (`[a-z0-9-]{1,64}`, `R[0-9]{1,3}`, else `unreadable`), deduplicated and capped at 32 each (then `overflow`); a
  recorded network profile counts only on chain 777001, as R22 requires at boot.

It is false today with the build reasons of every open PLAN gate and, for any network, the network reasons above. The
fresh network launches with it false and its blockers disclosed (`admission-scope`, `profile-kind`; FRESH-NETWORK-CP.md
§1, "Launch admission"). Deployment branding (`network_display`) clears none of them. Node-local reporting only: no validity
rule, fork choice, persisted byte or consensus identity reads it; `build.rs` (a boundary file) only drops the
literal, under a `neutral` lock line. H2 (release-pinned profiles and closing the conditions) is unchanged.

Open for H2 and the owner. (1) If the launcher is ever to report a network ready before a node boots it, each
`NETWORK_PROFILES` entry must also pin the expected admission (scope specification, no condition, no waiver, the
network profile on a network node, or a digest of them) and the launcher must compare against that pin, never
against devnet.json; until then `admission-not-booted` stays. (2) `profile-kind` clears only with a Network profile,
which R22 allows only on chainId 777001: the fresh network (rehearsal 1656550731, mainnet 2656089850) can never clear
it without an R22 or admission change, or a launch on 777001. That change belongs
to H2, may be consensus-adjacent (R22 is checked at profile validation), and is the owner's decision; N5 changes
nothing there.

### D-V3-29

One dated v3 decision record plus normative amendments (CONS-7, MAT-03, MAT-04). Record these decisions and scoped normative amendments before v3 code. The approved owner rule is the standing authorization only for changes preserving P1-P10.

Approved within the owner rule; NODE-LOCAL (process). Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-30

Multi-threaded miner, localhost work port, network hashrate estimate (security-budget MISSED). Split only the miner own range into disjoint thread slices; local work RPC is off by default, validates own-range work and is unreachable through the site bridge. Account for threads in A13.

Approved within the owner rule; NODE-LOCAL. Principle check: kept (every thread inside the member's own range). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-31

Formal security analysis of PoCol (CONS-2). Deferred external common-prefix, chain-quality and economic security analysis; no PoW-equivalent security claim follows from implementation or tests.

Deferred; DEFER (external work). Principle check: —. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-32

Disclose the 4,000,000 PCOL genesis allocation and decide how it is held (ECON-05). Superseded O-32=b (FRESH-NETWORK-CP.md §7; owner answers 8, 9 and 15 of 2026-11-13): the genesis allocates 1,000,000 PCOL to each of the four founding miners, its 20 PCOL bond included, and nothing else; the genesis builder enforces it. The allocation is disclosed with the launch (deploy/standalone/README.md, launch disclosures; PLAN E1). Superseded text: disclose the four public genesis holders and development purpose; plan public time-lock custody and spending reports as owner-operated actions. Do not move, change or reallocate the owner personal funds in this work.

Approved within the owner rule; OWNER-CHOICE. Principle check: not consensus. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-33

How the re-genesis is run, and the policy for any later one (DEC-06). Moot (FRESH-NETWORK-CP.md §7; owner answers 1-4 of 2026-11-13): a brand-new network, with no re-genesis and nothing carried over, so there is no freeze height S, old-chain export or migration. What remains is the policy part: later consensus changes go through the fork schedule (VA-SPEC §2.3; "Release rules and lock" below).

Approved within the owner rule; OWNER-CHOICE (+ NODE-LOCAL operations). Principle check: not consensus. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

### D-V3-34

Fork machinery: schedule v≥3, launcher upgrade path, `==2` → `>=2`, one dual-key rule predicate, program-set registry, off-node verifiers. Use v3_active(h)=active_version(h)>=3 || genesis.spec_version>=2 everywhere. Keep full nonce range for every v>=2 or nonceMode=1. Use a program-set registry and versioned CP schema. Retain sequential schedule versions (D3, below): in-place forks append (2,H) as a no-op for nonceMode=1, then (3,Hprime).

Approved within the owner rule; CONSENSUS-COMPLIANT (infrastructure). Principle check: kept (schedule semantics unchanged). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md.

## Release rules and lock

The v3 rules are the release rule set: `CONSENSUS_RULES_VERSION` 2, both `SPEC_VERSION` copies 2 and `MAX_FORK_VERSION`
3. Admission accepts only a specVersion-2 genesis with nonceMode 1 (R24). Fork version 2 is a nonce-rule no-op under
nonceMode 1; fork version 3 is the cap raise (FN-5, D-V3-23). The fresh genesis schedule is `[(1,0)]`, so neither takes
effect until a release schedules it.

Every v3 host rule uses one predicate, `v3_active(h) = active_version(h) >= 3 || genesis.spec_version >= 2`, mirrored by
pocold `Spec::v3_active`, lp1 `ProfileIdentity::v3_active` and the extension's `v3Active` (`rp.js`). On every admitted
network the second term is true from genesis (V3-DESIGN.md §3.3).

**D3 (owner decision): fork versions are sequential.** A schedule appends version 2 before version 3, each version
once and in order; version 2 is a nonce-rule no-op under nonceMode 1. The runtime refuses a skipped version
(`configured_forks`), and ParamGate refuses one as R24 (lp1-node `gate.rs`, the skip check of `bad[23]`), although the
schedule type itself would accept it. No version skipping is authorized.

The consensus-sources lock is to be reset once, at the release cut before the fresh genesis: its history restarts at a
single `kind=base` line recording the v3 constants, and nothing of the retired network's history is kept (VA-SPEC §2.3;
not made yet, and until then the development history stays append-only). From that line on the history is append-only. A later rule change on the running chain is a new fork version V ≥ 4 behind
`active_version(h) >= V` with a `neutral gate=fork:V` line; a change that needs a new genesis is a `kind=bump` or
`gate=spec:3` line with that genesis.

## Deferred and rejected work

V3-22 skip-on-revert and V3-31 external formal analysis remain deferred. A VDF randomness beacon is deferred. V3-R is rejected: no BFT/committee finality, operator-signed checkpoints, lowered retention marketed as finality, new PoW hash, per-identity hashrate cap or stake-weighted chain selection.

## Founders' allocation

The fresh genesis allocates the initial supply of exactly 4,000,000 PCOL to the four founding miners
([FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md) §1 "Founders' bond" and §3 "Founders"; owner answers 8, 9 and 15):

| Founder | Device | On its reward address | Live HELD bond (in Escrow) | Total |
|---|---|---:|---:|---:|
| 1 | US server 1 (first in `-Servers` order) | 999,980 PCOL | 20 PCOL | 1,000,000 PCOL |
| 2 | US server 2 | 999,980 PCOL | 20 PCOL | 1,000,000 PCOL |
| 3 | US server 3 | 999,980 PCOL | 20 PCOL | 1,000,000 PCOL |
| 4 | The owner's PC | 999,980 PCOL | 20 PCOL | 1,000,000 PCOL |

Each founder generates fresh keys on its own device at the ceremony; the phone is a wallet only. The miner ids and
reward addresses are published at the ceremony, with the genesis (in the committed profiles they are `PLACEHOLDER`
strings). The genesis builder checks that the founders are four and distinct, that they are the only members and the
only allocation, and that each founder's allocation plus B_reg is 10^24 wei. (Owner decision pending: whether each
server founder's reward address is its own hot miner key or a separate owner-held address; FRESH-NETWORK-CP.md §9.)

The owner describes the allocation as funds intended to finance network development. This record neither moves funds
nor invents a custodian, beneficiary or unlock schedule.

## Related owner actions

Seeds (owner statement, V3-OWNER-DECISIONS-2026-11-08.md §3): the three operator seeds will be stopped once peer-address discovery works. Recommendation only, not an owner statement: also wait until enough independent nodes exist. The owner creates the real Android release signing key. The planned human security audit is after the real launch; that plan supplies no current audit or readiness evidence. These statements do not authorize live operations.

## Validation and operational boundaries

Each implementation PR starts from newest main and merges only after all CI jobs are green and valid review findings are fixed or answered. Never skip or disable a test. Require exact A13 accounting, ParamGate R1-R26 at the committed profiles (FN-4, `fn4_profiles.rs`) and a pinned consensus-sources lock. The rehearsal runs on a closed network with the rehearsal chain id (1656550731) before the mainnet ceremony. Do not run the genesis ceremony or choose the launch date without the owner.

The profile values, particularly WAN timing, caps and gas/memory bounds, are decided values, not measured results. The rehearsal measures them (the launch evidence T1-T8, M1-M4; FRESH-NETWORK-CP.md §9); a failed threshold changes only profile values, recorded in FRESH-NETWORK-CP.md. No implementation or CI result proves PoW-equivalent security or closes the external audit.
