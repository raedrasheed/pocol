# PoCol v3 decisions — 2026-11-11

Status: records the owner approval of 2026-11-08 and its implementation constraints. This is a documentation decision record, not an activation, a deployment approval or evidence that the implementation is complete. The live network remains untouched.

## Owner rule and scope

> أوافق على تعديل التي لا تغيير خوارزمية الاجماع بدون اي تحفظ. أما التي تحتاج تغيير في خوارزمية الاجماع فأوافق على التغيير الذي لا يغيير فكرة الاجماع بالتعاون ولا يغير فكرة الكتلة النموذج أو تغيير أي مبدأ من المبادئ الاساسية ل PoCol.

Node-local changes are approved without reservation. Consensus changes are approved only while preserving collaboration, the common immutable template, and every basic PoCol principle P1–P10 in [V3-DESIGN.md](V3-DESIGN.md). Stop and ask the owner before crossing that boundary or deciding an unanswered owner question.

For compliant items with the choices below, this standing owner authorization supplies the required owner decision; it supersedes the historical proposal-before-implementation gate in the repository root `README.md:484–486` for those items only. That bullet reads: “**Consensus is owner-gated.** A change to a consensus rule, a `CP` parameter or a wire encoding needs an owner decision. Write a proposal in `development/audit-2026-10-22/proposals/` with the alternatives and their effects, and do not implement it before it is approved.” (An earlier version of this record cited the path `development/m1/reference/README.md:484–486`, which does not exist.) It does not approve rejected mechanisms, deferred work, unanswered questions reserved to the owner or live deployment. Preserve earlier decision records as history; the scoped amendments below take precedence only for their stated profiles.

Sources: [approved design](V3-DESIGN.md), [dated owner answers](V3-OWNER-DECISIONS-2026-11-08.md), and [consensus boundary policy](../VA-SPEC.md). The implementation PR sequence is the handoff roadmap; its later dormant-gate strategy supersedes the design earlier suggestion to spend a gate at the first v3 scaffold PR.

## Owner choices

| Question | Item | Decision |
|---|---|---|
| O-0 Vehicle for v3 | — | **a**: v3 rides the held earn-in re-genesis, and the same release builds the in-place fork machinery for later changes |
| O-6 Block time | V3-06 | **b**: 30 s (D_att 150, dFbWait 10, Δ_rel 13), valid if the measured WAN Δ ≤ 2 s; otherwise 60 s (option c) |
| O-10 Contestable seats when the set is full | V3-10 | **b**: contest by work over one day with a statistical margin (θ=1/8, 3σ), only when all seats are full, with the displaced member refunded, and only together with V3-02 |
| O-11 Inactivity removal delay Θ_inact | V3-11 | **b**: 1 hour |
| O-12 Registration bond B_reg | V3-12 | **a**: 20 PCOL |
| O-13 Probationers as proposers | V3-13 | **b**: yes, once the accrued bond is at least B_reg/4 (the all-probationer exception below B_reg/4 is retained: owner decision 2026-11-12, option a) |
| O-14 Probation pool | V3-14 | **b**: M_max/4 slots (64), contest by work inside the pool, T_PROB = 30 days, both as CP fields (requires V3-09) |
| O-15 Uncovered share pot | V3-15 | **b**: carried into the next block's pot at closeBlock |
| O-17 Reward for reporting misbehaviour | V3-17 | **b**: 25% of the burned bond to the reporter, 75% burned |
| O-18 Evidence window and bond delay | V3-18 | **b**: 1 hour, with U = E_win + D + a margin |
| O-19 Issuance schedule | V3-19 | **M3**: M1's rate (2 PCOL per 60 s, scaled by T_blk) halving every 4 years, with a tail of S0/32 |
| O-20 Fee market | V3-20 | **b**: the proposer receives 50% of tips directly, the rest stays in the shared pot, base-fee floor 0.01 gwei; the pool becomes fee-ordered with replacement and eviction (node-local) |
| O-21 State caps | V3-21 | **b**: raise the caps (A13-verified) and add a burned creation fee that rises as the cap fills |
| O-32 The 4,000,000 PCOL genesis allocation | V3-32 | **b**: publish the holder addresses and their purpose, and move the development fund into a public time-locked account with published spending reports |
| O-33 How the re-genesis is run | V3-33 | **b**: no user transactions after S; publish the old-chain data; ship an Android key-export build first; and commit publicly that this is the last owner-only re-genesis, with later consensus changes going through the fork schedule |

## All-probationer exception — owner decision (2026-11-12)

An earlier version of this section (commit 3d526e0, 2026-11-11) recorded an owner clarification of O-13: “Retain the all-probationer exception below B_reg/4.” The audit of 2026-11-12 found no verifiable owner source for that text, so the question was put to the owner directly on 2026-11-12.

**The owner chose (a) and keeps the exception.** The verbatim answer is «أوافق على (أ)، أبقِ الاستثناء» (“I approve (a); keep the exception”). The question and the answer are recorded in [V3-OWNER-DECISIONS-2026-11-12.md](V3-OWNER-DECISIONS-2026-11-12.md).

What the decision means:
- Under O-13=b, an ACCRUING member normally enters the proposer draw only once its bond is at least B_reg/4.
- When that filter would exclude every draw member, the existing earn-in D1 rule ([EARN-IN-SPEC.md](EARN-IN-SPEC.md) §6) applies instead. All those probationers enter the deterministic, share-weighted draw, so signed blocks continue.
- In that case a drawn proposer may hold less than B_reg/4, possibly zero. The B_reg/4 minimum loss for a type-1 equivocation therefore does not hold then.
- The dormant v3 code already implements this, so no code change follows.
- The liveness interplay with all-live membership is disclosed in [threat.md](../m1/reference/threat.md) (v3 amendment, A6).

## Decisions D-V3-01 through D-V3-34

### D-V3-01

Give a nonce range to every live (non-tombstoned) member whenever the nonce domain is unbounded; removes the permanent-halt and range-starvation path (MISSED share inclusion, ECON-03). For v3 unbounded mode, the parent-state active list excluding tombstones owns the ranges regardless of recent-share activity. Bounded mode and spec-1 behavior remain unchanged. Membership remains independent of attempt.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch8 (activity gating) but preserved; P4/P5 kept, Q3.5 improved. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-02

Late share credit: proposers of blocks j+1..j+D carry uncredited valid shares of template j as body evidence; late credit only fills the pool up to κm, so it never dilutes header shares. Signed bodies may carry RLP([3, be64(j), [n_1<...<n_k]]) for ancestor templates j in [max(1,h-D),h-1]. Check target, range owner, exclusion of the winning nonce and header shares, branch deduplication and C_header(j)+C_late(j)<=kappa*m. Record late credit after recordResult and before settlement; lastShare=max(lastShare,j). Roll credit back with the branch. This never dilutes header shares.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: touches Ch6 but preserved; P2/P5 kept, P6 strengthened (winner no longer alone decides credit). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-03

Multi-hop relay of valid templates and shares.

Approved within the owner rule; NODE-LOCAL (already in tree at 53a9e87). Principle check: kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-04

Fork choice uses timeliness-qualified signed precedence, with the specified arrival allowance for attempt-0 fallback. At first import, a signed block is timely iff this node first locked its signed template (validated as minable by T_j: drawn signer, window, item 16, body rebuilding to its UT; or its own) by T_j+Phi, because T_j is the template deadline; a shell merely received never counts, and a slot (parent, signer, height) whose signer equivocated counts no template. Only if this node never locked the template, or its slot is poisoned, is the block's own arrival compared with T_j+Phi (the earlier time counts). Template first-lock times are local, bounded (one TemplateID per slot) and not persisted; the bit they give is frozen at first import. Attempt-0 fallback is timely iff arrival>=T_j-(Delta+Phi); later attempts are timely. On v3 only, sibling key=(timely?0:1,signed?0:1,importSeq,blockHash), compared at first path divergence after cumulative work. Persist first-import time across restart. Amends D133; validity is unchanged.

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

### D-V3-11

Inactivity eviction delay Θ_inact, set in hours. O-11=b: one hour, converted into blocks using the final measured T_blk.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameter. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-12

Registration bond B_reg. O-12=a: B_reg=20 PCOL; cutover tooling reads the new genesis CP[16], never a mirrored literal.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameter (Ch9). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-13

Probationary (ACCRUING) members as proposer candidates. O-13=b: an ACCRUING member may enter the proposer draw once bond>=B_reg/4. The existing all-probationer exception below B_reg/4, used when the filter would otherwise exclude every draw member, is retained (owner decision 2026-11-12, option a; see 'All-probationer exception' and V3-OWNER-DECISIONS-2026-11-12.md). Keep the share-weighted deterministic draw.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch1 candidate set; P3 kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-14

Probation pool size and in-pool contest; P_MAX and T_PROB become CP fields set in time. O-14=b: P_MAX=M_max/4 (64 at M_max=256), T_PROB=30 days as CP fields. Use the same work statistic for contests within the ACCRUING pool, refund accrued bond and conserve ACC_COUNT. V3-09 precedes expansion.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch9; P8. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-15

Carry the uncovered share pot into the next pot at closeBlock. O-15=b: send uncovered base-burn share remainder to carry at settlement; roundBurn remains burned. closeBlock(j) adds and zeroes carry after that block phase-0 settlement, so pot j is fixed by its template. Burn carry at a finite final block. Escrow invariant includes carry.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch7; P6 kept (carry fixed before mining). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-16

Type-2 evidence burns only the offender's parts; whole-pot burn kept when the accused is ACCRUING. Settle normally after punishment; tombed offender winner/proposer/share parts burn, while honest contributors are paid. Retain whole-pot burn if the accused was ACCRUING below B_reg at block j, recording that fact at recordResult rather than reading later state.

Approved within the owner rule; CONSENSUS-COMPLIANT. Principle check: Ch7; P6/P7 strengthened. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-17

Reporter reward ρ of a burned bond. O-17=b: 25% of punished bond goes to the carrying signed proposer reward address; burn 75%. A fallback has no reporter and burns the whole bond. Self-reporting still loses 75%.

Approved within the owner rule; OWNER-CHOICE. Principle check: Ch7/Ch9; P6 kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-18

Evidence window E_win and bond delay U, set in hours. O-18=b: E_win is one hour; U=E_win+D+margin, with U>=W_act+2 and existing retention constraints. Re-derive memory bounds.

Approved within the owner rule; OWNER-CHOICE. Principle check: parameters. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-19

Use M3 issuance: a scaled initial rate, four-year halvings and a tail. O-19=M3: issuance(0)=0; otherwise max(k>=256?0:S0>>k,S_tail), k=floor(h/H_half). Rate is 2 PCOL/60 s, scaled by T_blk; halve every four years, tail S0/32. Bound 2*S0*H_half+S_tail*2^64<2^127. The host passes issued explicitly to closeBlock.

Approved within the owner rule; OWNER-CHOICE. Principle check: P6 kept (issuance(h) known before mining). Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-20

Fee market: tip share to proposer and base-fee floor (consensus); fee-ordered pool with replacement, eviction and per-sender quota (node-local). O-20=b: a signed proposer receives 50% of tips directly; the other half enters the shared pot. Fallback sends all tips to the pot. Base-fee floor 10^7 wei. Fee ordering, replace-by-fee and lowest-fee eviction are node-local.

Approved within the owner rule; OWNER-CHOICE (+ NODE-LOCAL pool). Principle check: the chosen half-tip split preserves shared precommitted rewards; pool policy is node-local. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

### D-V3-21

Live-state caps: raise to an A13-verified middle and add a utilisation-priced, burned creation fee per new account/slot/code byte. O-21=b: candidate caps are ACCT=200000, SLOT=500000, CODE=32 MiB, subject to A13. Burn sender-paid creation charges F0*CAP/(CAP-used), based on running in-block utilization. F0_acct candidate=10^15 wei; F0_slot and F0_code remain to be calibrated in the resource/profile PR. Do not introduce a per-block growth griefing budget. Preserve copied user nonce, code and storage; apply the approved old-bond credits and new-miner bond deductions, and never lower caps below copied usage.

Approved within the owner rule; OWNER-CHOICE. Principle check: execution layer (Ch13); kept. Authority: V3-DESIGN.md, V3-OWNER-DECISIONS-2026-11-08.md. Applies to v3 rules where relevant. Preserve consensus verdicts for all spec-1 inputs accepted by the dormant builds; no live deployment is authorized.

**Open pre-launch item: a user bound on the creation burn (recorded 2026-11-12, V3-PR-C3 review C3-4).** The burn is limited only by the sender's post-execution balance. It is priced at the running in-block occupancy, which the builder's selection order sets, so no field the sender signs (gas limit, maxFeePerGas, priority fee) bounds it: near a full cap a plain transfer to a fresh address can be charged up to F0·CAP (200 PCOL at the candidate ACCT_CAP and F0_acct). V3-PR-C3 adds no consensus field for it. The bound stays open until the owner chooses one of:
- (a) node-local: the pool and eth_estimateGas quote the burn and refuse a submission whose quoted burn exceeds a user-supplied limit (wallet/client support in V3-PR-G); inclusion order can still change the charged amount, so this bounds quotes, not the consensus charge;
- (b) consensus: a sender-signed cap (for example a halt with CreationFee when burn > max_fee·gas_limit − gas cost, or a separate per-transaction burn limit), compatible with the owner rule and every basic PoCol principle.

**Owner decision 2026-11-12: option (b), the consensus bound inside the signed maximum fee** (gas cost plus burn ≤ max_fee·gas_limit, otherwise Halt::CreationFee; plus a node-local burn quote RPC); see [V3-OWNER-DECISIONS-2026-11-12.md](V3-OWNER-DECISIONS-2026-11-12.md), second decision (the owner's option A, quoted verbatim there). It was the enforced launch blocker `D-V3-21 creation-burn user bound` (`lp4-execution/src/admission.rs` `V3_LAUNCH_BLOCKERS`) until the implementation below. No current build reaches that check through `admit` (preview builds refuse a non-simnet profile first; release builds cannot decode spec 2), so a source check (`admit_calls_the_v3_launch_gate_before_any_fact_is_built`) keeps the call in place; V3-PR-I, which makes the path reachable, replaces it with an `admit`-level test and must show `V3_LAUNCH_BLOCKERS` empty. The V3-PR-G scope includes the client part (V3-DESIGN.md, PR-7).

**Implementation note (V3-D21-burn-bound, dormant, v3 only).** Details in [V3-EXECUTION-FEES.md](V3-EXECUTION-FEES.md), "The signed maximum fee".
- Consensus: under `v3_active`, a nonzero burn is charged only if `gas_used·effective_price + burn ≤ max_fee·gas_limit` (types 0/1: the gas price; value separate, as in the Ethereum balance pre-check); otherwise `Halt::CreationFee` (`user_phase::SignedFee`, `charge_creation`). Spec-1 paths, constants and identity are unchanged.
- Halt charge, decided under the owner rule: a `Halt::CreationFee` charges the gas the execution spent before the EIP-3529 refund (`gas_used + gas_refunded`, at most gas_limit; priced like a revert of the same work, since the halt discards the storage clears that earned the refund), not the declared gas_limit, so that a builder order that pushes the burn past the maximum cannot turn the burn allowance a wallet adds to gas_limit into a gas charge through this halt. Other A03 cap halts keep the declared gas.
- Residual, not closed: another order-dependent cap halt (LogCount, by filling the block's log count ahead, since every burning transaction emits its CreationFeeBurn log; or AcctCap/SlotCap, LogBytes, LogCap, the dirty caps) charges the declared gas. A hostile proposer can force one, and so can any user against an honest builder: the review reproduced a LogCount halt of an estimated creating transfer that the honest append round (B5) added behind a user's log filler, once heavy calls the selection left out had used its execution budget. Under v3 the append round now drops an appended transaction that halts (node-local); the honest selection still includes, by design, a transaction whose own logs pass a log cap, and caps it does not model (dirty keys/accounts) can still halt a selected one. Whoever builds the block, the guaranteed loss bound is therefore the owner's bound, the signed `max_fee·gas_limit`, not only the spent gas. Extending the spent-gas charge to those halts would change A03's full-gas semantics and is not part of option A (open for the owner).
- Per-transaction burn ceiling: `max_fee·GAS_LIMIT` minus the gas cost. Near a full cap a creation therefore needs a large signed maximum (the last account at the candidate F0_acct and ACCT_CAP, 200 PCOL, needs max_fee ≥ 25,000 gwei at an 8M GAS_LIMIT).
- Node-local: the pool refuses and the builder skips a transaction whose burn would pass its maximum in their order; eth_estimateGas returns a gas limit that covers the burn with a margin of `ESTIMATE_DRIFT_UNITS = 4` creations ahead per class (4 accounts, 4 slots, 4·B_code_max code bytes), applied when a limit below the first run's passes it; `pocol_quoteCreationBurn` returns the burn, split by class and from the gas, at the head's occupancy (network.md, "Creation burn quote").
- Client consequence: a transaction that creates an account, slot or code needs a gas_limit above its executed gas (at least ⌈(gas_used·effective_price + burn)/max_fee⌉), so a transfer to a new address signed with a fixed 21000 gas is refused by the pool and skipped by the builder; clients use eth_estimateGas or `pocol_quoteCreationBurn`.
- `V3_LAUNCH_BLOCKERS` is now empty; the gate, its `admit` call and their tests stay for any later blocker.

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

**Implementation note (H1, N5).** `prototypes/pocold/src/readiness.rs` replaces the literal `publicLaunchReady:false`
(`build.rs`, `launcher.rs`, `testnet.rs`) with a computed value: `publicLaunchReady` is true only when
`launchBlockers` is empty, and both are reported by `pocold build-info`/`pocol build-info` (the build alone, which
is never ready: blocker `no-network`), `pocold status` and `pocol_status` (the admission the node booted with),
`init-testnet` (the profile it admitted) and the launcher's network description (`pocol network`/`import`/
`upgrade`, from what genesis.spec.hex commits to; the launcher admits nothing, so it is never ready). Each blocker
has a stable `id` and a `detail`:
- build: `v3-preview-build`; `spec-version` (SPEC_VERSION < 2, cleared by V3-PR-I); `consensus-unpinned`;
  `test-features` (caps-test, crash-test, io-fault); `v3-launch-blocker` (each `V3_LAUNCH_BLOCKERS` entry, none
  today); `plan-gate-<id>` (each acceptance-ledger gate of `development/public-network/PLAN.md` not marked
  closed, all nine today; a test keeps `PLAN_GATES_OPEN` equal to the ledger);
- network: `v3-inactive` (spec-1 genesis and no fork to version 3); `network-not-release-pinned` ((chainId,
  genesisHash) not in `NETWORK_PROFILES`, the H2 hook, empty today); `admission-scope` (not `specification`);
  `admission-condition-<id>` and `admission-waiver-<rule>` (each condition and waiver); `profile-kind` (not the
  network profile on a network node, R22); `admission-not-booted` (always), `admission-unrecorded` and
  `genesis-unreadable` (launcher only). The admission devnet.json records about itself is operator metadata that
  nothing commits to, so the launcher never treats it as evidence: `admission-not-booted` always blocks, and the
  recorded scope, condition ids and waiver rules are reflected only as further blockers, in the gate's id shape
  (`[a-z0-9-]{1,64}`, `R[0-9]{1,3}`, else `unreadable`), deduplicated and capped at 32 each (then `overflow`); a
  recorded network profile counts only on chain 777001, as R22 requires at boot.

It is false today with the build reasons `spec-version` and the nine PLAN gates and, for any network, the network
reasons above. Deployment branding (`network_display`) clears none of them. Node-local reporting only: no validity
rule, fork choice, persisted byte or consensus identity reads it; `build.rs` (a boundary file) only drops the
literal, under a `neutral` lock line. H2 (release-pinned profiles and closing the conditions) is unchanged.

Open for H2 and the owner. (1) If the launcher is ever to report a network ready before a node boots it, each
`NETWORK_PROFILES` entry must also pin the expected admission (scope specification, no condition, no waiver, the
network profile on a network node, or a digest of them) and the launcher must compare against that pin, never
against devnet.json; until then `admission-not-booted` stays. (2) `profile-kind` clears only with a Network profile,
which R22 allows only on chainId 777001: the designated PoCol Mainnet (chain 1599846715) or any v3 re-genesis on
another chain id can never clear it without an R22 or admission change, or a launch on 777001. That change belongs
to H2, may be consensus-adjacent (R22 is checked at profile validation), and is the owner's decision; N5 changes
nothing there.

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

Seeds (owner statement, V3-OWNER-DECISIONS-2026-11-08.md §3): the three operator seeds will be stopped once peer-address discovery works. Recommendation only, not an owner statement: also wait until enough independent nodes exist. The owner creates the real Android release signing key. The planned human security audit is after the real launch; that plan supplies no current audit or readiness evidence. These statements do not authorize live operations in this work.

## Validation and operational boundaries

Each implementation PR starts from newest main and merges only after all CI jobs are green and valid review findings are fixed or answered. Never skip or disable a test. Require old-vs-new spec-1 differential checks, exact A13 accounting and the unchanged H15000 identity. This documentation PR must merge before v3 code. Rehearsals and WAN soak run on private disposable networks only. Do not deploy, reinitialize the live network or choose S/date without the owner.

Candidate profile values, particularly WAN timing, caps, creation-fee coefficients and gas/memory bounds, are not measured results. The profile/resource PR must resolve them with evidence. No implementation or CI result proves PoW-equivalent security or closes the external audit.
