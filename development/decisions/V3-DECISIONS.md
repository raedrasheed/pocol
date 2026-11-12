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

Probationary (ACCRUING) members as proposer candidates. O-13=b: an ACCRUING member may enter the proposer draw once bond>=B_reg/4. Owner clarification on 2026-11-11: retain the existing all-probationer exception below B_reg/4 when the filter would otherwise exclude every draw member. Keep the share-weighted deterministic draw.

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
