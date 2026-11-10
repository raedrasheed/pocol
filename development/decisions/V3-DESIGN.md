# PoCol v3 — final design (consensus bundle within the PoCol principles)

Status: design, approved by the owner on 2026-11-08 (see V3-OWNER-DECISIONS-2026-11-08.md). Nothing here is deployed or built yet. Every file:line was read at commit
`53a9e878f5d1fd9caa5a39275911a9d5960de8a9` (head of PR #1). Paths are relative to the repository root
(`WT`) at that commit.
Thesis citations are "T p<PDF page>/<printed page>" of `WT/PhD-thesis-Raed-Rasheed-120211566-61.pdf`.

**The owner's rule (verbatim):** «أوافق على تعديل التي لا تغيير خوارزمية الاجماع بدون اي تحفظ. أما التي تحتاج
تغيير في خوارزمية الاجماع فأوافق على التغيير الذي لا يغيير فكرة الاجماع بالتعاون ولا يغير فكرة الكتلة النموذج أو
تغيير أي مبدأ من المبادئ الاساسية ل PoCol.»

Here is how this document applies it:
- **NODE-LOCAL:** approved without reservation.
- **CONSENSUS-COMPLIANT:** approved by the rule, because the change keeps P1–P10 (§2).
- **OWNER-CHOICE:** the mechanism is compliant, but a value or a policy needs the owner's answer (§5).
- **DEFER:** not now.
- **REJECT:** violates a principle, so it would need a new explicit owner decision.

---

## 1. Summary table

| Item | Proposed change (one line) | Principle check | Classification | Deployment | Effort | Risk |
|---|---|---|---|---|---|---|
| V3-01 | Give a nonce range to every live (non-tombstoned) member whenever the nonce domain is unbounded; removes the permanent-halt and range-starvation path (MISSED share inclusion, ECON-03) | touches Ch8 (activity gating) but preserved; P4/P5 kept, Q3.5 improved | CONSENSUS-COMPLIANT | B (host rule, dual-key gated) | S | low |
| V3-02 | Late share credit: proposers of blocks j+1..j+D carry uncredited valid shares of template j as body evidence; late credit only fills the pool up to κm, so it never dilutes header shares | touches Ch6 but preserved; P2/P5 kept, P6 strengthened (winner no longer alone decides credit) | CONSENSUS-COMPLIANT | B (host + program) | L | medium |
| V3-03 | Multi-hop relay of valid templates and shares | kept | NODE-LOCAL (already in tree at 53a9e87) | done | — | — |
| V3-04 | Fork choice: signed precedence only for a signed block first imported by the local deadline; a fallback block imported before the deadline ranks as untimely | touches Ch10 (amends D133) but preserved; D15 signed precedence kept for honest proposers; Q2.8 improved | CONSENSUS-COMPLIANT | B (host) | S | low-medium |
| V3-05 | Δ_rel default derived from measured WAN Δ (not lowered to 7 s); per-proposer template-latency counters in `pocol_status` | kept | NODE-LOCAL | any release | S | low |
| V3-06 | Recalibrate T_blk, D_att, dFbWait, Δ_rel from the ParamGate inequalities on measured WAN latency | parameters (Ch5/Ch11); kept | OWNER-CHOICE | B only (ASERT anchor) | S + measurement | medium |
| V3-07 | M_max 64 → 256; compute draw weights from ledger owners instead of M_max×W_w record reads | parameter; P8 loosened | CONSENSUS-COMPLIANT | B | S | low |
| V3-08 | REG_OPS: separate per-block maintenance budget (setRewardAddress/topUp) and at most one maintenance op per identity per block | Ch9; restores P8/Q5.2 | CONSENSUS-COMPLIANT | B (program) | S | low-medium |
| V3-09 | R_max entry slot: reactivate gets its own budget and a cooldown ≥ U; R_max 1 → 4 | Ch9 + parameter; restores P8/Q5.2 | CONSENSUS-COMPLIANT | B (program + CP) | S | low |
| V3-10 | Contestable seats when the set is full: displace (with refund) a member whose credited work in the last full epoch is statistically below threshold | touches P8 (new removal reason) and P7; preserved only with V3-02 and the statistical margin | OWNER-CHOICE | B (program) | M | medium |
| V3-11 | Inactivity eviction delay Θ_inact, set in hours | parameter | OWNER-CHOICE | B | S | low |
| V3-12 | Registration bond B_reg | parameter (Ch9) | OWNER-CHOICE | B | S | low |
| V3-13 | Probationary (ACCRUING) members as proposer candidates | Ch1 candidate set; P3 kept | OWNER-CHOICE | B (host) | S | low |
| V3-14 | Probation pool size and in-pool contest; P_MAX and T_PROB become CP fields set in time | Ch9; P8 | OWNER-CHOICE | B | M | low-medium |
| V3-15 | Uncovered share pot: keep burning it, or carry it into the next pot at closeBlock | Ch7; P6 kept (carry fixed before mining) | OWNER-CHOICE | B | M | medium |
| V3-16 | Type-2 evidence burns only the offender's parts; whole-pot burn kept when the accused is ACCRUING | Ch7; P6/P7 strengthened | CONSENSUS-COMPLIANT | B (program) | S | low |
| V3-17 | Reporter reward ρ of a burned bond | Ch7/Ch9; P6 kept | OWNER-CHOICE | B | S | low |
| V3-18 | Evidence window E_win and bond delay U, set in hours | parameters | OWNER-CHOICE | B | S | low |
| V3-19 | Issuance schedule: flat / time-scaled / halving / halving with tail | P6 kept (issuance(h) known before mining) | OWNER-CHOICE | B only | S–M | medium |
| V3-20 | Fee market: tip share to proposer and base-fee floor (consensus); fee-ordered pool with replacement, eviction and per-sender quota (node-local) | (a),(b) keep P6; (c) touches P6 | OWNER-CHOICE (+ NODE-LOCAL pool) | B; pool part in any release | S–M | low-medium |
| V3-21 | Live-state caps: raise to an A13-verified middle and add a utilisation-priced, burned creation fee per new account/slot/code byte | execution layer (Ch13); kept | OWNER-CHOICE | B | M | medium |
| V3-22 | Skip-on-revert for non-value mandatory SYSTEM calls | Ch13; would need Q4.3 proof | DEFER | — | S | medium |
| V3-23 | Height-gated system-code and CP replacement machinery (`cp_at(h)`, irregular state change at H) | kept (code compiled in, selected by height) | CONSENSUS-COMPLIANT (build now, use for later forks) | ships in release B, unused at first | L | high |
| V3-24 | Per-block log caps 32 KiB/546 → 256 KiB/4,368; builder and pool model log caps | Ch13; kept | CONSENSUS-COMPLIANT (+ NODE-LOCAL builder) | B | S | low-medium |
| V3-25 | Derive worst-case SYS gas and block-validation time for the exact v3 programs and CP (EXEC-03/EXEC-05) | kept | NODE-LOCAL | before B genesis | M | low |
| V3-26 | RPC `finalized` = persisted local irreversibility floor; reorg telemetry; honest finality docs (CONS-3) | kept (Q0.1: not consensus) | NODE-LOCAL | any release | S | low |
| V3-27 | Disclose seal/timestamp grinding bounds and a PREVRANDAO warning; a VDF beacon later | kept | NODE-LOCAL (docs); beacon DEFER | docs | S | none |
| V3-28 | Computed launch-readiness flag and a real network-admission path (CONS-4, MAT-02, EXEC-03) | kept | NODE-LOCAL | any release (H2 needs V3-21/25) | S / L | low-medium |
| V3-29 | One dated v3 decision record plus normative amendments (CONS-7, MAT-03, MAT-04) | kept | NODE-LOCAL (process) | docs, before B genesis | M | low |
| V3-30 | Multi-threaded miner, localhost work port, network hashrate estimate (security-budget MISSED) | kept (every thread inside the member's own range) | NODE-LOCAL | any release | S–M | low |
| V3-31 | Formal security analysis of PoCol (CONS-2) | — | DEFER (external work) | — | — | — |
| V3-32 | Disclose the 4,000,000 PCOL genesis allocation and decide how it is held (ECON-05) | not consensus | OWNER-CHOICE | docs / on-chain transactions | S | low |
| V3-33 | How the re-genesis is run, and the policy for any later one (DEC-06) | not consensus | OWNER-CHOICE (+ NODE-LOCAL operations) | before and during B | S–M | medium |
| V3-34 | Fork machinery: schedule v≥3, launcher upgrade path, `==2` → `>=2`, one dual-key rule predicate, program-set registry, off-node verifiers | kept (schedule semantics unchanged) | CONSENSUS-COMPLIANT (infrastructure) | first PRs of release B | M–L | medium |
| V3-R | BFT/committee finality, operator-signed checkpoints, lower R_keep as "finality", a different PoW hash, per-identity hashrate caps, stake-weighted chain selection | violates P1/P9 (Q1.3, Q1.4, Q6.2) | REJECT | — | — | — |

Owner question O-0 decides the vehicle itself (B = ride the held earn-in re-genesis). The recommended answer is B.

---

## 2. PoCol core principles and the compliance checklist

### 2.0 The boundary the sources draw
- **What may change.** The thesis family Π "retain[s] the underlying hash function and a cumulative-work
  chain-selection objective, but may change canonical-template formation, round structure, work assignment,
  verification rules, miner-set handling, and reward distribution" (T p79/62 §4.5; text re-checked).
- **What makes a protocol PoCol.** The combination of immutable-template agreement, deterministic disjoint
  nonce ownership, collaborative reward precommitment, and explicit post-range operating policies
  (T p57-58/40-41; T p139/122 §8.3). The thesis states "The core of PoCol is its deterministic nonce-range
  allocation function" (T p112/95; re-checked).
- **What "collaboration" means.** Nonce ownership bound to one common template (T p58/41), plus a reward
  that is shared and fixed before mining (T p103-105/86-88; "The winning miner does not create or alter this
  transaction", T p104/87, re-checked).
- **What "the template block" (الكتلة النموذج) means.** One immutable canonical block template and one
  nonce-free header template (T p4/III). The implementation realizes it as TM1: a template signed by the
  attempt-0 proposer, with a deterministic fallback and exclusive branching. The docs call TM1–TM14
  "modifications, not replacement" (`development/m1/reference/scope.md:67-83`).

### 2.1 Core principles: a consensus change must keep all ten
- **P1 Proof-of-work basis.**
  - Validity is SHA-256(TemplateID‖be64(nonce)) ≤ a protocol target. The canonical chain is the one with the
    greatest Σwork.
  - No vote, quorum, stake or committee decides validity or chain selection.
  - Sources: T p72/55, T p79/62; `consensus.md:159`, `:204-210`; `decisions.json:18-20` (D4);
    `prototypes/pocold/src/types.rs:42`.
- **P2 The template block.**
  - All members mine one immutable template per (parent, height, attempt). Every field except the nonce is
    fixed before mining, and candidates differ only in the nonce.
  - A block on any other template is invalid.
  - Sources: T p89-90/72-73, T p99-102/82-85; `consensus.md:53-59`, `:73-81`, `:166`;
    `decisions.json:143` (D29).
- **P3 Template agreement without a quorum.**
  - One proposer, Draw(h,0), is computed from committed data. A deterministic fallback lets every node build
    the template without that proposer.
  - Sources: T p86/69, T p100/83; `consensus.md:100-111`; `chain.rs:503`, `:584-588`.
- **P4 Coordinated nonce ownership.**
  - Each round the nonce domain is split into disjoint per-member ranges, computed locally with no
    coordinator and no messages.
  - Sources: T p90-93/73-76; T p112/95; `consensus.md:113-118`; `prototypes/lp1-node/src/assignment.rs:174-196`.
- **P5 Validity depends on range ownership.**
  - The winning nonce, and every credited share, must lie in the claimant's own range on the canonical
    template.
  - Sources: T p75/58; T p102/85; `consensus.md:161`, `:167-168`; `chain.rs:592-594`.
- **P6 A shared reward, fixed before mining, that conserves value.**
  - R_tot = subsidy + fees is shared by a deterministic rule fixed before mining. The winner cannot change
    the payout.
  - Paid amounts plus classified burns equal R_tot.
  - Sources: T p87/70, T p103-105/86-88; `incentives.md:70-89`;
    `PoColSystemV1.sol:184-216` (re-read: `:185-200`).
- **P7 Reward in proportion to auditable work.** A goal, not yet proven. Splitting one miner into several
  identities, free-riding and suppressing other miners' evidence must not pay (T p80-81/63-64; T p103/86;
  `FINAL_DESIGN.md:4303-4334` T7/T7b/T7c).
- **P8 Open, on-chain membership with a cost per identity.**
  - The miner set is the on-chain registry. Anyone joins with an ordinary transaction, and every identity
    carries a cost.
  - Inactive members are removed by a public rule, with no stake penalty.
  - Sources: T p22/5, T p79/62, T p86/69, T p88-89/71-72, T p112/95; `incentives.md:8`, `:44-54`.
- **P9 Determinism and no trusted party.** Every consensus value is a function of committed data and the
  pinned cfg, with no trusted coordinator, key or hardware (T p81/64, T p85/68, T p91/74;
  `consensus.md:46`, `:244-251`).
- **P10 An explicit post-range policy.** Each deployment declares the idle or the continuous policy (T p59/42,
  T p95/78). v2's full 64-bit domain is the continuous alternative (`chain.rs:55-59`).

**Design choices** (changeable if P1–P10 hold):
- Ch1 proposer draw (`assignment.rs:253-306`, `chain.rs:423-429`)
- Ch2 fallback details
- Ch3 seed source
- Ch4 encodings
- Ch5 timing windows
- Ch6 shares as work evidence
- Ch7 reward structure (pot, winnerPart/propPart, `max(C,κm)`; winnerPart is "our addition", `scope.md:81-83`)
- Ch8 activity gating of ranges (`assignment.rs:79-84`)
- Ch9 bonded registration and its rate limits
- Ch10 fork-choice tie-break and R_keep
- Ch11 ASERT difficulty
- Ch12 GenesisSpec/CP/ForkSchedule/horizon
- Ch13 EVM, system programs, sites, RPC (`scope.md:50-58`)

**Parameters:** all CP and local numeric values. They already differ between profiles: M_max is 256, 64 or
1024; T_blk is 60 s or 10 s (`governance.md:5`, `:10`).

**Where current choices already sit away from a principle.** A fix that moves these back is compatible:
- **Winner-chosen share list** (`chain.rs:560-565`) weakens P6/P7.
- **Proposer head start** weakens P2 (Q2.8).
- **Seat cap, bond and R_max/REG_OPS limits** weaken P8.
- **Activity-gated ranges** (`assignment.rs:79-84`) depart from the thesis baseline (T p88/71).
- **Seal bias** is left open by the thesis itself (T p91/74).

### 2.2 Compliance checklist (every answer must be Yes)

**Q0. Is this a consensus change at all?**
- Q0.1: does it touch a validity rule (`consensus.md:151-171`), fork choice, a CP/GenesisSpec field, system
  code, the ForkSchedule or a consensus encoding? If not, it is node-local and approved; stop here.
- Q0.2: does it touch only design choices or parameters?

**P1, proof of work**
- Q1.1 SHA-256 ≤ protocol target.
- Q1.2 heaviest Σwork, with only a deterministic per-node tie-break.
- Q1.3 no vote, quorum, committee, stake or signed checkpoint.
- Q1.4 the hash is still SHA-256.

**P2/P3, the template block**
- Q2.1 one template per round for all members.
- Q2.2 every field but the nonce is committed before mining.
- Q2.3 the nonce is the only PoW variable.
- Q2.4 apply(h) ignores block h's mining result.
- Q2.5 one deterministic attempt-0 proposer.
- Q2.6 a deterministic proposer-free fallback.
- Q2.7 any other template is invalid.
- Q2.8 no new or longer head start.

**P4/P5, dividing the work**
- Q3.1 disjoint ranges.
- Q3.2 computable before the round from committed data.
- Q3.3 bound to (parent, height, attempt).
- Q3.4 the winner and every credited share lie in the claimant's own range.
- Q3.5 all eligible members take part every round.

**P6/P7, the shared reward**
- Q4.1 shared by a deterministic rule, not paid to one party.
- Q4.2 fixed before mining.
- Q4.3 exact conservation.
- Q4.4 non-winner reward only for verifiable in-range work counted once.
- Q4.5 proportional to work, with no flat payment per identity.
- Q4.6 splitting identities or suppressing evidence is no more profitable (re-run T7/T7b/T7c).

**P8, membership**
- Q5.1 on-chain miner set.
- Q5.2 anyone can join with an ordinary transaction.
- Q5.3 every identity has a cost, not weakened without analysis.
- Q5.4 explicit exit; inactivity removal with no stake penalty.
- Q5.5 joining is not made harder without analysis.

**P9/P10, determinism**
- Q6.1 pure function of committed data and cfg.
- Q6.2 no trusted party in validity, proposer choice, nonce assignment or rewards.
- Q6.3 idle/continuous policy separate from range allocation.

**Automatic failures:**
- committee/BFT finality or operator checkpoints;
- templates chosen by vote, or miners building their own templates;
- an extranonce;
- overlapping, ignorable or non-recomputable ranges;
- winner-take-all, discretionary or flat per-identity rewards;
- an admission whitelist;
- a different PoW hash or trusted hardware.

---

## 3. Deployment plan for the consensus bundle

### 3.1 What exists today (verified)
- **The schedule cannot express v3.**
  - `ForkSchedule` rejects any version other than 1 or 2, and anything after v2
    (`prototypes/lp4-execution/src/p1_profile.rs:231-236`; byte-identical copy in `lp1-node/src/profile.rs`).
  - The node accepts only `[(1,0)]` or `[(1,0),(2,H)]` (`prototypes/pocold/src/lib.rs:202-204`).
- **The v2 rule is an equality in three places.** If a (3,H) entry were appended, every height ≥ H would
  silently fall back to the bounded domain (live CP4 = nonceMode 0, `deploy/mainnet/h15000/README.md:18`):
  - `chain.rs:72` `active_version(h)==2||cp().nonceMode==1`;
  - `prototypes/pocol-extension/src/lib/rp.js:56` `x.version === 2n || …`;
  - `deploy/rewards/pocol_rewards.py:277` `header['protocolVersion'] == 2 or cp[4] == 1`.
- **CP and system code are pinned to genesis.** Both are re-checked on every parent and candidate block
  (`genesis_builder.rs:22-27`). Changing CP "means a new network" (`governance.md:185`).
- **Every release is a hard partition.** The handshake digest covers the source (`build.rs:24-35`), and
  `pocol upgrade` accepts only "v1-only plus one v2 append" (`launcher.rs:107-112`).
- **The held re-genesis.**
  - The earn-in genesis has schedule `[[1,0]]` and nonceMode 1 (`development/decisions/EARN-IN-SPEC.md:17`;
    `deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK.md:287`).
  - The release that builds it must replay the old chain to S (`prototypes/pocold/src/cutover.rs:104-113`).
  - It hard-codes a 20 PCOL bond (`cutover.rs:120`) and requires `earnin:true, continuous:true`
    (`cutover.rs:102`).

### 3.2 Recommended path: B, the held re-genesis carrying v3, with generalized fork machinery in the same release

**Why B:**
- Almost every item changes a program-read CP field or system code (V3-02/08/09/10/14/15/16/19/21). Today
  only a new genesis can deliver those.
- B gives a fresh ASERT anchor, so T_blk, D and W_w can change at no cost, and their transition traps never
  arise (`asert.rs:57`; `PoColSystemV1.sol:91-94`, `:162-166`).
- The operator still runs all three seeds. Once the seeds stop (the owner's plan after discovery works), a
  re-genesis would need community consent. B is the last cheap point.

**Why also build the machinery (V3-23, V3-34) now:** every later fix can then go in place through the fork
schedule, without another re-genesis.

**Alternatives** (owner question O-0):
- **A1.** In-place version-keyed host overrides. These are only possible after V3-34, and they cannot
  deliver CP or program changes.
- **A2.** In-place `cp_at(h)` plus a code swap at H on the live chain. This is high risk (about 105 `.cp()`
  call sites, transition traps, a D72 amendment) and needs a testnet rehearsal (`development/public-network/PLAN.md:35`).

### 3.3 Gating rule (resolves the review's keying conflict)

Every v3 host rule uses one predicate. It is defined once and mirrored in the host, `rp.js` and
`pocol_rewards.py`:

```
v3_active(h) = active_version(h) >= 3  ||  spec.spec_version >= 2      // specVersion 2 = v3 genesis
```

- On the live and old chain both terms are false. The cutover replay from 0 to S is therefore unchanged.
- On the v3 genesis the second term is true from height 0.
- On the live chain, a later in-place A1 fork turns the first term on.
- Every v2 test changes from `==2` to `>=2`.

### 3.4 What must be built first, and PR order
1. **PR-0 Decision record (docs).**
   - Add `development/decisions/V3-DECISIONS.md` and `decisions.json` entries D-V3-*.
   - Quote the owner's rule verbatim. Record the §5 answers.
   - State that the rule supersedes `development/m1/reference/README.md:484-486` (the proposal-before-implementation
     gate) for compliant items.
   - Write the normative amendments (V3-29).
   - Must merge before any consensus code.
2. **PR-1 Fork machinery (V3-34).**
   - Generalize ForkSchedule to v≥3 (both profile files), `lib.rs:202-204`, and the 8 schedule consumers
     (`launcher.rs:107-112`, `deploy/public-testnet/nonce_upgrade.py:58-66`,
     `deploy/standalone/prepare_nonce_upgrade.py:29-34`, `forks.js:7,20`, `pocol_rewards.py:52-53`,
     `deploy/mainnet/preflight.py:129-134`).
   - Change `==2` to `>=2` in the three places above.
   - Add `v3_active`.
   - Add a launcher path for digest-only and append-only upgrades. The compatibility digest otherwise strands
     every home (`launcher.rs:29-31`, `:459-465`).
   - Tests: a [(1,0),(2,H2),(3,H3)] schedule keeps nMax = 2^64 above H3; a v3 schedule fails closed against
     stored headers.
3. **PR-2 Program-set registry and genesis tooling.**
   - Replace `Spec::earnin()` equality (`chain.rs:49-52`) with a feature predicate ("earn-in or later").
   - Add the v3 set to `SystemPrograms::for_spec` (`apply.rs:54`), `testnet.rs:159`, `devnet.rs` and
     `build.rs:27-30`.
   - Give the `testnet.rs` Config (currently `deny_unknown_fields`, `:19`) explicit CP overrides.
   - Make `cutover.rs:120` read cp[16].
   - Add CP schema version 2 (specVersion 2), appending: H_half, S_tail, P_MAX, T_PROB, F0_acct, F0_slot,
     F0_code, γ_tip_bp, baseFeeMin, R_react, the contest parameters, and ρ_bp (`p1_genesis_spec.rs:35`, `:67-121`;
     both profile files).
4. **PR-3 Host rules.**
   - V3-01 membership, V3-04 fork choice, V3-24 log caps and the V3-21 creation fee, all gated by `v3_active`.
   - V3-13 draw candidates.
   - V3-02 host validation of late shares.
5. **PR-4 v3 system programs** (`system-v3.json`, a fork of `PoColSystemEarnIn.sol`).
   - V3-08/09 registry budgets, V3-02 `recordLate`, V3-16 type-2, V3-15 carry (if chosen),
     V3-19 `closeBlock(tips, issued)`, V3-10/14 contest (if chosen), V3-17 ρ, and P_MAX/T_PROB through `cfg()`.
   - Delete the host mirrors at `apply.rs:48-49`.
6. **PR-5 Escrow invariant and issuance.**
   - Add `issuance.rs`. Add the carry term to `check_escrow_invariant` (`apply.rs:524-543`).
   - Replace the supply bound (`genesis_builder.rs:70`, `gate.rs:324`).
7. **PR-6 Resource proofs (V3-25).**
   - GAS_ENVELOPE for the v3 programs (`admission.rs:86-87`).
   - A13 re-run at the v3 CP in store mode (`resources.rs:571-577`).
   - ParamGate R1–R26 at the v3 CP and the WAN locals (V3-06).
8. **PR-7 Off-node verifiers.**
   - `pocol_rewards.py:272-281` (V3-01 membership, late shares, issuance), `rp.js:41-61`, `refdec.js:18`.
9. **PR-8 Harnesses and vectors.**
   - SimNet T7c(S) (share omission), T10b(iv) (withholding proposer plus a pre-mined fallback coalition),
     T10 with ASERT (carry and burn).
   - Test vectors for every new validity rule.
10. **PR-9 Rehearsal.**
    - Local cutover at the v3 genesis, then the pending real-server rehearsal
      (`local-rehearsal-reviewed.json:11,15`).
    - A multi-host WAN soak to measure Δ and φ for V3-06.

**Node-local items (V3-05, V3-20 pool part, V3-24 builder part, V3-26, V3-28, V3-30)** can merge at any time.
Every release changes the handshake digest, so they reach the live network only with PR-1's upgrade path or in
release B.

**Activation:**
- The owner answers §5, then PR-0 through PR-9 land.
- The new genesis is generated with specVersion 2, and S is announced in advance.
- The cutover runs as in the runbook, with the V3-33 conduct rules.

---

## 4. Items in full

Live parameters below are decoded from `deploy/mainnet/h15000/network.pocol.json`:

| Parameter | Live value |
|---|---|
| T_blk | 10 s |
| D_att / dFbWait | 60 / 6 |
| m / κ / S_max | 32 / 2 / 256 |
| D / W_w | 4 / 8 |
| B_reg | 100 PCOL (earn-in: 20) |
| R_max / M_max | 1 / 64 |
| REG_OPS | 16 |
| W_act / Θ_inact | 32 / 64 |
| E_win / U | 16 / 40 |
| subsidy | 2 PCOL |

Local timing values are Δ=1, φ=1, Φ=2, t_exec=1, Δ_rel=8 (`prototypes/pocold/src/testnet.rs:50-56`).

### V3-01 Ranges for every live member (unbounded mode)

**Problem.**
- Ranges go only to members whose lastShare or graceStart lies within W_act. The only fallback is when nobody
  qualifies: `ids = if active.is_empty() {live} else {active}` (`prototypes/lp1-node/src/assignment.rs:78-84`).
- Membership has no attempt input (`chain.rs:415-421`, `:438`).
- So if every member in A_h stops mining, no other member owns a nonce at any attempt, and the chain halts for
  good. The winner chooses which shares are credited (`chain.rs:560-565`), so a dominant member can also strip
  other members' ranges within W_act=32 blocks.

**Change.**
- When `v3_active(h) && effective_nonce_mode(h)==Unbounded`, set `ids = live`, meaning every non-tombstoned
  active-list member.
- Bounded mode keeps today's rule.
- Membership stays independent of the attempt. That avoids the `drawn` trap: `chain.rs:609` stores
  `drawn: ctx.proposer`, and `:680-681` checks type-1 evidence against it.
- The draw is unchanged: offline members have weight 0, and the uniform fallback applies only when nobody has
  weight (`assignment.rs:291-296`).
- An unsearched range costs nothing when each range holds 2^64/n nonces.

**Principle check.**
- Q3.1–Q3.4 hold. Q3.5 improves, matching the thesis baseline (T p88/71).
- P2, P6 and P9 are untouched. P8 holds: eligibility is still on-chain state.

**Classification:** CONSENSUS-COMPLIANT. **Deployment:** B (host rule) and PR-7 (gateway membership,
`pocol_rewards.py:272`).

**Safety.**
- Pure function of the parent state. In bounded mode (unused on both networks) the rule is off, so progress
  bound 11 (`consensus.md` §11) is unchanged.

**Tests.**
- Fails on the original code: 3 members; only X has a recent share, then X stops. Today every candidate fails
  "13 owner". Under v3 another member mines a block.
- Old-chain replay byte-identical; the gateway reconstructs owners.

**Effort:** S. **Risk:** low.

### V3-02 Late share credit

**Problem.**
- Header shares are chosen by the winner after mining; validity checks only size, order, range and target
  (`chain.rs:560-565`). Omitting a rival's share is free while C ≤ κm, and it removes the rival's pay, draw
  weight and lastShare (`incentives.md:144-150`).
- Today the payout to each contributor is effectively set after mining by the winner. That is the
  P6 weakness of T p104-105/87-88.

**Change.**
- **Body evidence kind 3.** `LS = RLP([3, be64(j), [n_1<…<n_k]])` in signed bodies only.
- **Validity checks:**
  - 1 ≤ k ≤ S_max, and j ∈ [max(1,h−D), h−1] on the carrying branch;
  - each n_i satisfies n_i < nMax_j, n_i ≠ nonce_j, and SHA256(tid_j‖be64(n_i)) ≤ target_j·m;
  - each n_i's owner is taken from the stored `e.ranges` of block j, as type-2 does (`chain.rs:697-703`);
  - n_i is not in block j's header shares, nor in any LS(j) already on the branch, nor repeated in this block;
  - the shares fit the no-dilution cap below.
- **No-dilution cap.** LS(j) is accepted only while `C_header(j) + C_late(j) ≤ κm`. q_j = max(C_header, κm)
  therefore never changes, and late credit fills only the uncovered part of the pool. It never lowers the
  value of shares already credited.
- **Effect** (phase 0, after `recordResult`, before `settle(h−D)`; `apply.rs:446-457`):
  - a SYSTEM-only `recordLate(j, owners)` raises count[j][a] and total[j];
  - it sets lastShare = max(lastShare, j), with no revert when lastShare > j.
- **Node-local part.** Cache the shares seen per TemplateID for the last D canonical templates (at most
  4·S_max each). The proposer adds any uncredited ones, the same way it adds evidence
  (`node.rs:1539-1550`).

**Principle check.**
- Q2.2: LS is committed by evidence_root inside TemplateID before mining. Q2.3: the nonce is still the only PoW
  variable. Q2.4: apply reads only ancestor data.
- Q3.4/Q4.4: in-range PoW on the canonical tid_j, counted once.
- Q4.2: block j's pot amount is fixed by block j's template. Credit moves away from the winner's post-mining
  choice, which is P6's intent.
- Q4.5 holds (proportional).

**Classification:** CONSENSUS-COMPLIANT. **Deployment:** B (host, program, PR-7 gateway).

**Safety.**
- The deduplication set is derived from the branch, and reorgs roll back through inverse diffs.
- `recordLate` is bounded by S_max; SYS keys by ≤ 64 owners per j; the A13 cache is about D·4·S_max·8 bytes.
- Mining a stale template j pays no more than mining the current one, because of the cap.

**Attacker cost.** Omission now works only if the cartel also controls every carrier proposer for j+1..j+D:
about b^D, which is 6.3% at b=0.5, D=4.

**Tests.**
- Fails on the original code: the winner omits B's share, and LS(j) in j+1 pays B at settle(j).
- Negative vectors: duplicates (within the list, across carriers, within one block); j outside the window; a
  non-ancestor tid; a nonce ≥ nMax or in another member's range; a target miss; LS in a fallback body; LS past
  the κm cap.
- Replay/reorg determinism; T17 on every block; SimNet T7c(S) at b ∈ {0.3, 0.5}.

**Effort:** L. **Risk:** medium.

### V3-03 Multi-hop relay (done)
Valid templates and shares for the locked template are relayed (`node.rs:1354-1364`, commit 53a9e87).
NODE-LOCAL. Nothing further.

### V3-04 Fork choice: timely signed precedence

**Problem.**
- `key(b) = (signed?0:1, importSeq, hash)` (`chain.rs:1084`; `consensus.md:206`; D133 `decisions.json:663-665`).
- A signed block withheld past the deadline still beats an honest fallback sibling of equal work until that
  fallback has a child. Full withholding therefore pays (the MISSED finding on withholding).
- Simply dropping signed precedence (the earlier 2a) would reverse D15/D133. It would also reward any member
  that privately pre-mines the deterministic a=0 fallback template:
  - the fallback ts is L(0)+dFbWait (`lp1-node/src/timing.rs:66-67`), importable from ts−Φ (`timing.rs:115-116`);
  - honest nodes wait until T_j = max(b_j+Δ_rel, L(0)+dFbWait) (`timing.rs:96-105`);
  - the fallback body is empty and its winner also takes propPart (`PoColSystemV1.sol:194-196`).

**Change.** Each block gets a `timely` flag at its first import, which is local, like importSeq.
- **signed:** timely iff first import ≤ T_j(local) + Φ.
- **attempt-0 fallback:** timely iff first import ≥ T_j(local) − (Δ + Φ).
- **attempt ≥ 1:** always timely.

The key becomes `(timely?0:1, signed?0:1, importSeq, blockHash)`.

**Effects:**
- An honest timely signed block beats everything.
- A withheld signed block loses to a timely fallback.
- A pre-mined early fallback loses to the timely signed block.
- The key is fixed per stored block, so D133's total-order and transition lemmas still hold.

**Principle check.**
- Q1.2: heaviest work first; the tie-break stays per-node deterministic, the same class as importSeq. Q1.3: no
  vote.
- P2/P3: exclusive branching (`consensus.md:166`) and validity are unchanged. Q2.8 improves.

**Classification:** CONSENSUS-COMPLIANT. It amends D133 and keeps D15 for honest templates; record it in PR-0.

**Withholding gain** (exponential race, after V3-04, release at the deadline; w = Δ_rel/T_blk):

| Case | f = 0.1 | f = 0.2 | f = 0.3 |
|---|---|---|---|
| Today, full withholding, 10 s | +0.145 | +0.244 | — |
| After V3-04, 10 s (w = 0.7) | +0.061 | +0.105 | +0.133 |

The remainder depends on V3-06.

**Deployment:** B (host). **Safety:** a node restart re-imports blocks and assigns new flags, exactly as
importSeq is local today.

**Tests.**
- Fails on the original code: import a timely fallback, then a late signed sibling. Today the head switches;
  under v3 it stays.
- Run the twenty-import-order D133 suite again.
- SimNet T10b(iv) with a pre-mined-fallback coalition at b ∈ {0.2, 0.35, 0.5}, reporting the empty-block rate
  and the extra reward.

**Effort:** S. **Risk:** low-medium.

### V3-05 Δ_rel default and template telemetry
- **Problem.** The 7 s gate minimum holds only for single-host locals. R2 is
  `Δ_rel ≥ σ+Δ+2t` (`lp1-node/src/gate.rs:258`); with WAN locals Δ=5, Φ=10, t=3 it requires Δ_rel ≥ 25.
- **Change.**
  - Derive the default Δ_rel = σ+Δ+2t from measured Δ, rather than lowering it.
  - Add per-proposer template-arrival latency counters to `pocol_status`.
- **Classification:** NODE-LOCAL. **Deployment:** any release. **Tests:** the default satisfies R2 for each
  locals profile. **Effort:** S.

### V3-06 Timing recalibration (OWNER-CHOICE O-6)

**Problem.**
- The live timing is calibrated for one host. With governance WAN locals the gate formulas
  (`lp1-node/src/timing.rs:128-130`; `gate.rs:258-262`) give: σ=14, R2 Δ_rel ≥ 25, R3 dFbWait ≥ 21, and
  X^W ≈ 33–36 s.
- An earlier proposal (T_blk=30, D_att=150, dFbWait=12) fails R3 and R6 at those locals.

**Gate-valid options:**
- **(a) 10 s.** Valid only with Δ≈1 s, so not on the Internet.
- **(b) 30 s.** T_blk=30, D_att=150, dFbWait=10, Δ_rel=13. Valid if measured WAN Δ ≤ 2 s and φ ≤ 2 s:
  - σ=7, R2 13, R3 10, X^W=17, R6 2(17+60)+4 ≤ 150+Γ(10);
  - residual withholding gain at f=0.2 is +0.066.
- **(c) 60 s.** The governance profile T_blk=60, D_att=300, dFbWait=25, Δ_rel=26: X^W=36, R6 322 ≤ 330.
  - Gain +0.066 at the profile Δ_rel; +0.034 if measured Δ=2 allows Δ_rel=13.

**Coupling.** The subsidy per block scales with T_blk (V3-19), and all block-count parameters are set in
seconds and converted (V3-11/14/18).

**Principle check:** parameters; Q6.1 holds. **Deployment:** B only. ASERT is anchored at genesis
(`asert.rs:57`), so an in-place T_blk change halts the chain.

**Tests:** ParamGate R1–R26 at the chosen values; a multi-host soak with injected latency, reporting the
fallback ratio. **Effort:** S plus measurement. **Risk:** medium.

### V3-07 M_max 64 → 256

**Problem.**
- `register` reverts `Capacity` at active ≥ M_max (`PoColSystemV1.sol:290`; `PoColSystemEarnIn.sol:327`).
- About 61 free live seats could be filled in minutes.

**Change.**
- Set CP 18 = 256 (the network-profile value). REG_RECORDS_MAX = 8192 already satisfies R12/R16.
- Node-local: compute draw weights by iterating ledger owners instead of M_max×W_w record reads
  (`chain.rs:424-427`). The result is the same.

**Principle check:** loosens P8 (Q5.5). **Classification:** CONSENSUS-COMPLIANT. **Deployment:** B.

**Safety:** about 52 bytes × ΔM per resident entry (`resources.rs:452-453`), roughly +64 MB; re-run A13 against
the 6 GiB budget (`node.rs:85`).

**Tests:** A13 and ParamGate at 256; T7b (identity splitting) at 256. **Effort:** S. **Risk:** low.

### V3-08 REG_OPS budget split

**Problem.**
- There is one chain-wide counter: `(c&65535)>=cfg(23)` reverts `RegOpsLimit` on every path
  (`PoColSystemV1.sol:285`), incremented at `:311`.
- One HELD seat calling `setRewardAddress` 16× per block (it needs only `held(w)`, `:296-297`) freezes every
  join, exit and eviction, including eviction of itself.

**Change.**
- `setRewardAddress` and `topUp` count in a separate per-block maintenance counter with cap REG_MAINT_MAX
  (program constant 8).
- Register, deregister, claimBond and removeInactive use `REG_OPS − maintenance_used`.
- At most one maintenance op per identity per block, tracked by a `lastMaintHeight` field in spare record bits
  (earn-in uses w0 bits 152..215; this needs ≤ 40 more bits, or one record word, re-derived against SYS_SLOT_MAX).

**Principle check:** restores Q5.2. **Classification:** CONSENSUS-COMPLIANT (program). **Deployment:** B.

**Safety:** the footprint stays under R18 (`gate.rs:285`). Conservation is untouched.

**Tests.** Fails on the original code: 16 `setRewardAddress` calls from one identity, then a second identity's
register reverts today and succeeds under v3; the spammer's seat is evictable. **Effort:** S.
**Risk:** low-medium.

### V3-09 R_max entry-slot fix (from both reviews)

**Problem.**
- register and reactivate share the R_max counter `((c>>16)&255)>=cfg(17)` (`PoColSystemV1.sol:285`;
  `PoColSystemEarnIn.sol:322`).
- reactivate needs only an exited record whose W_act cooldown has passed (`PoColSystemV1.sol:304`;
  `PoColSystemEarnIn.sol:357`). It moves no bond, and it restarts grace (`:305`).
- With live R_max=1, an attacker with W_act+1 = 33 records deregisters one and reactivates another every block,
  holding the entry slot permanently. The attacker's transaction must come first, which the FIFO pool
  (`node.rs:919`) or the attacker's own proposals provide.
- Cost: 3,300 PCOL live or 660 PCOL under earn-in, all refundable. It is nearly free with zero-bond probation
  records (`PoColSystemEarnIn.sol:320`, `:358`).

**Change.**
- (1) reactivate draws from its own per-block budget R_react (a new CP field), not from R_max.
- (2) each identity may reactivate at most once per C_react blocks (a new CP field, C_react ≥ U), counted from
  its previous reactivation in a new per-record clock. Reactivation itself stays inside today's window, after the
  W_act cooldown and before the refund sweep erases the record at exitHeight + U
  (`PoColSystemEarnIn.sol:374-376`, run by beginBlock before user transactions). ParamGate requires
  U ≥ W_act + 2 so that this window always holds at least one block. (Review fix: an earlier draft required
  height ≥ exitHeight + U, which is exactly the height at which the sweep erases the record, so a refundable
  member could never reactivate.)
- (3) R_max 1 → 4. In continuous mode R_max adds nothing to the A13 identity bound:
  `M_max + R_max·H_END` with H_END=0 (`resources.rs:441`).

**Effect.** Reactivations come from their own budget and each identity reactivates at most once per C_react,
so to hold the register slot the attacker must now register k new identities per block, each locked
for ≥ U blocks before refund. That needs about R_max·U identities: 4·400·20 = 32,000 PCOL at the V3-18
recommendation. Zero-bond probation registrations are additionally capped by P_MAX in flight.

**Principle check:** restores Q5.2/Q5.5. **Classification:** CONSENSUS-COMPLIANT. **Deployment:** B.

**Tests.** Fails on the original code: 33 attacker records cycle deregister/reactivate, and a newcomer's
register must succeed within k blocks. Also check ACC_COUNT conservation through each path. **Effort:** S.
**Risk:** low.

### V3-10 Contestable seats (OWNER-CHOICE O-10)

**Problem.** Seats are bought with a refundable bond and kept with very little work, so a funded party can
close entry (ECON-02, DEC-05, CONS-1). CONS-1's "committee" is overstated: there is no vote, and wins follow
hashrate (`chain.rs:592`).

**Rejected form.**
- The earlier rule `8·cv·active < T` over 64 blocks reduces to cv = 0 at active=256 (T ≈ 64·32 = 2048).
- An average member (8 expected shares per window) would then be evictable about 30% of days by chance
  (e^−8 per gap × 1,080 gaps per day), and by share omission.

**Recommended form (option b).**
- The registry keeps per-record epoch counters (epoch = 1 day = 86400/T_blk blocks): (epochIndex, countCur,
  countPrev), updated in recordShares and recordLate. A global total per epoch is kept as well.
- `registerContest(reward, victim)` is allowed only when active == M_max. The victim must be live and active,
  past grace, and satisfy `countPrev(victim) < E − 3·sqrt(E)`, where `E = θ·Tprev/active` and θ = 1/8.
- The victim must also have been active for the whole measured epoch: its registration or last reactivation
  height must be at or before the first block of the previous epoch. A member that joined during the previous
  or current epoch cannot be contested, so a zero or partial-day count never makes a productive newcomer a
  victim. (Review fix: without this, a newcomer past its short grace has countPrev = 0 and could be evicted
  repeatedly by a persistent contestant.)
- The victim goes to exiting with a normal refund after U (no burn). Then a normal register follows.
- V3-02 is a hard prerequisite, so omission cannot manufacture a victim. Each contest is O(1) storage reads.
- At T_blk=30 and active=256: E = 45 and the threshold is about 25 shares per day.
  - A member with ≥ 0.05% of hashrate is essentially never displaced.
  - Holding 200 seats needs about 5.4% of hashrate instead of capital.

**Principle check.**
- It adds a work-based removal reason. The thesis names only exit and inactivity (T p88/71). That makes this a
  value choice under P8/P7, hence OWNER-CHOICE.
- Q5.4: refund, no stake penalty. Q4.5/P7: based on audited work. Q6.1: deterministic.

**Deployment:** B (program). **Tests:**
- full set with victim below threshold → success and refund after U;
- victim above threshold, in grace, set not full, or joined after the start of the previous epoch → revert;
- SimNet eviction rate of honest 0.1–1% members under a persistent prober and an omitting cartel.

**Effort:** M. **Risk:** medium.

### V3-11 Θ_inact (OWNER-CHOICE O-11)
- `removeInactive` requires height ≥ max(lastShare, grace) + cfg(25) (`PoColSystemV1.sol:300`).
- After V3-01 an idle range costs nothing, so liveness no longer requires a short Θ. But a longer Θ makes
  squatting cheaper wherever V3-10 does not apply.
- Express it in time and convert at T_blk. **Deployment:** B. **Effort:** S.

### V3-12 Bond B_reg (OWNER-CHOICE O-12)
- The bond is the instance of P8's "cost per identity" (`decisions.json:38` D8; `threat.md:16` A6), already
  lowered to 20 PCOL and made earnable (`EARN-IN-MEMBERSHIP-PROPOSAL.md:15`).
- **Deployment:** B (CP 16; `cutover.rs:120` must read it). **Effort:** S.

### V3-13 Probationers as proposer candidates (OWNER-CHOICE O-13)
- ACCRUING records are excluded from the draw (`chain.rs:418-420`, earn-in E5). At earn-in launch only operator
  servers can propose (DEC-05).
- Option (b) filter: `bond_state==4 && bond < B_reg/4` → excluded. A type-1 equivocation then always burns at
  least B_reg/4.
- The draw stays deterministic and share-weighted (P3). **Deployment:** B (host). **Effort:** S.

### V3-14 Probation pool (OWNER-CHOICE O-14)

**Problem.**
- Zero-bond probation records are capped by the constant P_MAX=8 (`PoColSystemEarnIn.sol:261`, `:334-335`).
- They can be held with about 1 share per Θ_inact, and rotated with claimBond + register.
- T_PROB=60,480 and P_MAX are program constants mirrored in the host (`apply.rs:48-49`) and the wallet.

**Change.**
- Make P_MAX and T_PROB CP fields read through `cfg()`, and delete the host mirrors.
- Optionally run a contest inside the pool (the V3-10 statistic, restricted to ACCRUING victims; the victim's
  accrued bond is refunded and the record erased; ACC_COUNT is updated as at `:394`).
- Re-derive T_PROB with the chosen issuance. Under M3 at 30 s, earning 20 PCOL in 7 days needs about 0.23% of
  hashrate; 30 days needs about 0.054%.
- V3-09 is a prerequisite for any larger zero-bond pool.

**Deployment:** B. **Tests:** fill the pool, contest below and above threshold; a rotation race; ACC_COUNT
conservation through promote, exit, displace and punish. **Effort:** M. **Risk:** low-medium.

### V3-15 Uncovered share pot (OWNER-CHOICE O-15)

**Problem.**
- With q = max(C, κm), the uncovered r − A is burned (`PoColSystemEarnIn.sol:213`, `:224-225`; V1 `:206`,
  `:215`). That is about 57% of R (`economics.md:68`).
- Net issuance therefore depends on behaviour, and holding solutions pays collectively (ECON-06, partially
  confirmed).

**Carry design (option b), fixed before mining.**
- Escrow slot `carry`. In settleOne, base burn r − A goes to `carry` instead of `burned`; roundBurn is still
  burned.
- `closeBlock(j)` sets `pending[j].amount = issued + tips + carry` and zeroes `carry`.
- closeBlock(j) runs after phase-0 `settle(j−D)` of the same block (`apply.rs:457`, `:489-499`), and that
  settle depends only on block j's own body. So pot j's amount is fixed by block j's template. This corrects
  the earlier argument, which put carry into settleOne (both reviews).
- At a final block (H_END ≠ 0) the carry is burned.
- Invariant: `need = Σpending + held + burned + carry` (`apply.rs:536-541`).
- Effect: net issuance equals the schedule, and the effective split becomes the documented 85/10/5.

**Principle check:**
- Q4.1: no treasury.
- Q4.2: fixed by the template.
- Q4.3: paid + burned + carry_out = amount, exactly.

**Deployment:** B. **Tests:**
- T17 with the carry term;
- fuzz C from 1 to S_max+1 plus late shares;
- changing LS inclusion in blocks j+1..j+D must not change pot j+1's amount once block j+1 is fixed;
- SimNet T10 with ASERT.

**Effort:** M. **Risk:** medium.

### V3-16 Type-2 evidence: offender-only burn

**Problem.** Type-2 marks the pot INVALID and burns all of it, including honest contributors' parts
(`PoColSystemEarnIn.sol:198`; V1 `:191`).

**Change.**
- Settle normally. The accused is tombed by `punish` earlier in the same phase 0 (`apply.rs:440-445`;
  `PoColSystemEarnIn.sol:455-462`). Its winner part, propPart and own shares then burn through `payTo`
  (tomb → burn, `PoColSystemV1.sol:225-232`) and `payInfos`.
- Exception: if the accused was ACCRUING with bond < B_reg when block j was produced, keep the whole-pot burn.
  A zero-bond winner otherwise loses almost nothing (principles review, finding 8).

**Principle check:** P6 conserves with classified burns; P7 stops penalizing innocents. **Classification:**
CONSENSUS-COMPLIANT. **Deployment:** B.

**Tests:**
- Fails on the original code: type-2 evidence before settle(j) pays contributors and burns the offender's parts.
- The ACCRUING-accused vector burns the whole pot.
- Evidence at the E_win boundary.

**Effort:** S. **Risk:** low.

### V3-17 Reporter reward ρ (OWNER-CHOICE O-17)
- `punish` burns the whole bond (`PoColSystemEarnIn.sol:460`), so nobody is paid to report.
- Option: pay ρ·bond to the reward address of `tx.origin`, the proposer of the carrying block, and burn
  (1−ρ)·bond. Self-reporting still loses (1−ρ)·B.
- Conserving (Q4.3). **Deployment:** B. **Effort:** S.

### V3-18 E_win and U (OWNER-CHOICE O-18)
- E_win=16 (`chain.rs:672`, `:697`).
- Gate R7 requires U ≥ E_win + D and E_win ≤ R_keep (`gate.rs:264`). A larger E_win raises slim_depth
  (2·R_keep + E_win + 257), so re-run A13.
- Set the values in hours. **Deployment:** B. **Effort:** S.

### V3-19 Issuance (OWNER-CHOICE O-19)

**Problem.**
- The host credits `cp[15]` every block (`apply.rs:489-492`), and the program records `cfg(15)+tips`
  (`PoColSystemEarnIn.sol:149`).
- With H_END=0 the supply bound multiplies by u64::MAX (`genesis_builder.rs:70`; `gate.rs:324`).
- The only documented policy is finite (`economics.md:67`), and PLAN E1 is open (`development/public-network/PLAN.md:36`).
- Gross issuance is 17,280 PCOL/day, about 6.31M/year, against about 4.0M genesis supply.

**Rule for M2/M3:**

```
issuance(h) = h==0 ? 0 : max(k>=256 ? 0 : S0>>k, S_tail),  k = h / H_half
```

- The host passes `issued` to `closeBlock(tips, issued)`. That removes the host/program coupling.
- The supply bound becomes `2·S0·H_half + S_tail·2^64 < 2^127`.

**Principle check:** Q4.1–Q4.3 hold, because issuance(h) is known before mining. **Deployment:** B only.

**Tests:**
- vectors at h = 1, H_half−1, H_half, 256·H_half;
- host credit equals the recorded pot, every block for 3·H_half on a compressed fixture;
- old-chain replay byte-identical.

**Effort:** S (M1) / M (M2/M3). **Risk:** medium.

### V3-20 Fee market (OWNER-CHOICE O-20, plus a node-local part)

**Problem.**
- The proposer receives γ = 5% of the pot, tips included (`PoColSystemV1.sol:194`), so a side payment is worth
  up to 20× an on-chain tip.
- The pool is FIFO (`node.rs:919`) with no replacement, eviction or per-sender limit.
- There is no base-fee floor: `fees.rs:4-11` London arithmetic can decay to about 7 wei.

**Change.**
- **Consensus (if chosen):**
  - a tip split γ_tip_bp, so the proposer gets γ_tip of the tips directly and the rest enters the pot;
  - baseFee = max(London, baseFeeMin) (CP).
- **NODE-LOCAL (PR6 in the task list):** a fee-ordered pool with replace-by-fee, lowest-fee eviction and
  per-sender quotas.

**Principle check.**
- (a) and (b) keep P6: R_tot is still shared by a deterministic pre-mining rule, and the proposer is drawn
  share-weighted, so its part is proportional in expectation.
- (c), 100% of tips to the proposer, moves fees out of the shared R_tot (Q4.1).

**Deployment:** B; the pool in any release. **Effort:** S–M. **Risk:** low-medium.

### V3-21 Live-state caps (OWNER-CHOICE O-21)

**Problem (EXEC-01, ECON-01, MAT-05, SCALE-01, SCALE-05, MISSED).**
- Live caps: CODE_CAP 2 MiB, SLOT_CAP 50,000, ACCT_CAP 20,000 (`devnet.rs:45`).
- A user transaction that would pass a cap halts with full gas (`user_phase.rs:222-223`).
- Nothing shrinks state. About 2 PCOL fills everything, and then onboarding and deployment freeze for good.
- Site chunks are code accounts (`ChunkFactory.sol:10-17`), so the 4 MiB SITE_MAX already exceeds CODE_CAP.
- The caps are load-bearing for A13 (`resources.rs:337-341`, `:349`, `:376`). Network-scale caps need K_eff,
  which is "not started" (`BOOT-BLOCKERS-2026-10-28.md:24-25`).

**Change.**
- **Raise the caps** to an A13/R21-verified middle in store mode, for example ACCT 200k, SLOT 500k,
  CODE 32 MiB. The seeds already run `state_store:true` (`launcher.rs:524`).
- **Add a utilisation-priced creation fee**, computed from the running count within the block (deterministic,
  sequential), required from the sender, and burned as a classified burn:
  - new account: `F0_acct·CAP/(CAP−used)`;
  - new nonzero slot: likewise with F0_slot;
  - new code byte: likewise with F0_code.
- **Effect.** The ceiling is never reached cheaply. The fill cost is about F0·CAP·(ln CAP + 0.58), and the
  last unit costs F0·CAP.
  - At F0_acct = 0.001 PCOL with ACCT 200k: about 2,560 PCOL to fill, with the last account at 200 PCOL, while
    ordinary users pay about 0.001 PCOL.
- **Dropped:** the per-block growth ceiling. It would create a new shared per-block griefing budget, the same
  pattern as the log caps (principles review, finding 5).
- **Dropped:** the flat surcharge. At F0 = subsidy/ACCT_CAP the fill cost stays 2 PCOL.

**Principle check:** execution layer; Q2.4 holds (a function of state and the transaction); Q4.3 holds
(classified burn); Q5 untouched.

**Deployment:** B. The cutover copies all state unchanged (`cutover.rs:16-33`), so the caps may only be raised,
never lowered, across it (`genesis_builder.rs:60`).

**Tests:**
- Fails on the original code: dust-fill each cap, then a legitimate fund or deploy halts today and succeeds
  (paying the fee) under v3.
- A griefer-cost test at the chosen F0.
- Conservation.
- A13 boot in store mode at the new caps.
- The raised caps survive `transform` → `genesis_builder`.

**Effort:** M. **Risk:** medium.

### V3-22 SYSTEM-call failure semantics (DEFER)
- Any mandatory SYSTEM-call failure discards the candidate (`apply.rs:238`), and the fallback runs the same
  calls (`chain.rs:496-498`; `node.rs:1116` goes idle).
- No user-reachable trigger exists in the live programs (the verifier's correction). The earn-in programs
  already removed the F1–F3 revert class (`PoColSystemEarnIn.sol:132-137`).
- Skip-on-revert for non-value calls changes validity semantics for no current gain. Revisit if EXEC-04
  fuzzing finds a trigger. The remedy for a program bug is V3-23.

### V3-23 System-code and CP replacement machinery (build now, use later)
- **Change.**
  - At `h==H` in `Job::run` before phase 0 (`apply.rs:430-441`), install compiled-in runtimes with
    `WorkState::insert_info` and migrate ChainCaps through `WorkState::commit` (`view.rs:189-195`, `:216-233`,
    `:245`).
  - Make `fixed_accounts` (`genesis_builder.rs:22-27`) and `SystemPrograms::for_spec` height-aware.
  - Add `cp_at(h)` (about 105 `.cp()` call sites).
  - Re-anchor ASERT when T_blk or τ changes. Add transition code for D, W_w and subsidy
    (`PoColSystemV1.sol:91-94`, `:162-166`; `apply.rs:489`).
  - Raise CODE_CAP at the same height (`code_caps.rs:27-32`).
  - Exclude or bound irregular writes in the SYS caps (`apply.rs:502-503`).
- **Principle check:** the code is compiled into the release and selected by height, identically on every node
  (Q6.2); no mining-result dependency (Q2.4).
- **Classification:** CONSENSUS-COMPLIANT. It needs a D72 amendment (`governance.md:185`) in PR-0.
- **Deployment:** ships with release B but is not exercised there. Its first use is a later fork, after a
  testnet rehearsal.
- **Tests:** a devnet swap at H (parent hashes old, candidate new); replay and reorg across H; an old-vs-new
  differential on live-shaped state (`REGISTRY-LAYOUT.md:14`).
- **Effort:** L. **Risk:** high.

### V3-24 Log caps
- **Problem.** `LOG_BLOCK_MAX` = 32 KiB and `LOG_COUNT_MAX` = 546 are per-block constants (`user_phase.rs:19-21`),
  checked cumulatively (`:214-218`). About 3% of a block's gas makes every later log-emitting transaction halt
  with full gas.
- **Change.**
  - Under `v3_active`: 256 KiB and 4,368. The A13 log window term `window·LOG_BLOCK_MAX`
    (`resources.rs:445`) stays about 8 MiB at K_hist=32.
  - Raise RESP_MAX ≥ 2·LOG_BLOCK_MAX (R26) in lockstep.
  - NODE-LOCAL: model the log caps in PoolMeter/`template_txs` and in `eth_estimateGas`.
- **Classification:** CONSENSUS-COMPLIANT. **Deployment:** B; later forks through A1.
- **Tests:** fails on the original code: front-load 32 KiB of logs, then a later ERC-20 transfer halts today and
  succeeds under v3. Also an A13 boot test.
- **Effort:** S. **Risk:** low-medium.

### V3-25 Execution bounds (EXEC-03, EXEC-05)
- **Problem.**
  - SYS gas assumes GAS_ENVELOPE is monotone (`admission.rs:323`), measured for V1 only (`admission.rs:86-87`).
  - R14/R19 use a compute loop.
  - State-heavy blocks measured about 1.07 s against t_exec = 1 s (A13-DERIVATION.md:736-737).
- **Change:** opcode-level worst-case SYS gas for the v3 programs, and an adversarial block-validation sweep at
  the v3 CP. Set t_exec from the result.
- **Classification:** NODE-LOCAL. Must be done before the B genesis is fixed. **Effort:** M.

### V3-26 Finality semantics (CONS-3)

**Problem.**
- The tags are fixed offsets (`rpc.rs:914-915`: safe=6, finalized=60).
- Any heavier fork above head−R_keep (2048) is adopted (`node.rs:714`; `chain.rs:756-757`).

**Change.**
- `finalized_h = max(finalized_h, head−r_keep)`, persisted in journal metadata.
- The import and retention floor becomes `max(head−r_keep, finalized_h)` at both `node.rs:714` and `retain()`
  (`chain.rs:757`). A heavier but shorter branch (`chain.rs:1086-1088`) can then no longer reopen the
  "finalized" block (safety review, finding 5).
- k_safe = 60, configurable.
- Telemetry: `max_reorg_depth`, `reorgs_ge_k_safe`, and `pocol_finality {safe, finalized, kSafe, rKeep,
  maxReorgDepth, networkHashrateEst, model:"probabilistic"}`.
- Docs (`consensus.md:242`, `governance.md:17`): the Nakamoto table, labelled indicative only (T p76-77/59-60).

**Principle check:** Q0.1 No: RPC and retention are local (`consensus.md:244-251`). Lowering R_keep as
"finality" is rejected (V3-R).

**Tests:** a depth-100 heavier fork keeps the old finalized block; a heavier shorter branch followed by a deep
fork; a restart test. **Effort:** S. **Risk:** low.

### V3-27 Grinding disclosure (CONS-6, CRY-08)
- **Inputs.**
  - The seal is `keccak(tid‖nonce)` (`types.rs:43`). It feeds the nonce seed, the proposer digest
    (`assignment.rs:123-129`, `:253-260`) and PREVRANDAO (`types.rs:45`).
  - The timestamp is a template field chosen by the proposer (`chain.rs:550`). ASERT is absolutely anchored
    (`asert.rs:57`).
- **Bounds:**
  - target inflation ≤ 2^(Φ/τ);
  - one-block swing ≤ 2^((D_att−1)/τ);
  - discarding a block costs α·R and gains at most γ·R plus the head-start value;
  - searching for a second nonce risks losing the block with probability 1−e^{−(1−q)/q}.
- **Change:** write these into `threat.md:67-68`, and add "PREVRANDAO is biasable by the block winner" to
  execution.md.
- **Classification:** NODE-LOCAL (docs). A VDF beacon is CONSENSUS-COMPLIANT but DEFERRED until the proposer
  slot is cheap (V3-04/06).

### V3-28 Admission and readiness (CONS-4, MAT-02, EXEC-03)
- **Problem.**
  - Every boot uses `Simnet / ExperimentalDevnet` (`lib.rs:227`).
  - Specification scope rejects any condition (`admission.rs:291-297`).
  - R22 ties Network to chainId 777001 (`lp1-node/src/profile.rs:279`).
  - `publicLaunchReady:false` is a literal (`launcher.rs:83`, `build.rs:101`, `testnet.rs:235`).
- **Change.**
  - H1: `launch_ready = scope==Specification && no conditions && no waivers && PLAN gates closed`, emitted with
    `launchBlockers`.
  - H2: a release-pinned `NETWORK_PROFILES [(chainId, genesisHash)]`, then close each condition (V3-21, V3-25,
    V3-29, K_eff).
- **Classification:** NODE-LOCAL. **Effort:** H1 S, H2 L.

### V3-29 Decision record and normative amendments (CONS-7, MAT-03, MAT-04)
- **Problem.**
  - `DECISION-TABLE-2026-10-28-EN.md:3` ("Nothing … is approved") conflicts with
    `OWNER-STANDING-AUTHORIZATION.md:12-14`.
  - Deployed rules are missing from the normative text: H_END=0 (`testnet.rs:159-161`), v2 (`chain.rs:57-58`),
    earn-in.
- **Change (PR-0).**
  - `V3-DECISIONS.md` with the owner's rule verbatim and the §5 answers.
  - Amendments:
    - consensus.md §5 (nMax=2^64 for version ≥ 2), §8/10a (H_END=0), §9 (V3-04 key), §12 (V3-26);
    - threat.md A5/A6 and `:68`, `:182`;
    - economics.md `:67`;
    - incentives.md (earn-in, V3-02/15/16);
    - governance.md (a "P-live" profile table; D72 amended for V3-23).
- **Classification:** NODE-LOCAL (process). **Effort:** M.

### V3-30 Honest throughput (security-budget MISSED)
- **Problem.**
  - One miner thread per node (`miner.rs:29-46`); a single SHA-256 per hash (`types.rs:42`).
  - target_g is synthetic, about 26 kH/s (`testnet.rs:105`).
  - One GPU (an estimate, not measured) dominates.
- **Change.**
  - S1 `--mine-threads N`: thread i scans `[start+i·L/N, start+(i+1)·L/N)` of the member's own range; count N
    threads in A13 `io_threads` (`resources.rs:83-91`).
  - S2 localhost-only `pocol_getWork`/`pocol_submitWork`, checked against the own range, off by default.
  - S3 `networkHashrateEst` over 360 blocks.
- **Principle check:** P4/P5 hold, with every hash inside the own range; Q6.3 holds.
- **Classification:** NODE-LOCAL.
- **Tests:** the union of thread slices equals the own range, with no overlap; cancellation joins all threads;
  an out-of-range submit is refused.
- **Effort:** S–M.

### V3-31 Security analysis (CONS-2, DEFER)
- Code cannot prove PoW-equivalent security (T p4/III; `threat.md:6`, `:15`).
- Commission a common-prefix and chain-quality analysis under deterministic nonce ownership, with ASERT and
  V3-04. Keep current claims modest. The owner's planned post-launch human audit can include it.

### V3-32 Genesis allocation (OWNER-CHOICE O-32)
- 4 × 1,000,000 PCOL (`network.pocol.json:10`), copied unchanged by the cutover (`EARN-IN-SPEC.md:135-144`).
  PLAN E1 is open.
- The thesis allows genesis balances "to founding entities or to a treasury account" (T p97/80), and bootstrap is
  "a deployment choice" (T p95/78). Not consensus.
- The owner states the allocation funds network development. The finding concerns disclosure and holding.
- **Deployment:** docs, plus optional on-chain transactions before or after B.

### V3-33 Re-genesis conduct (OWNER-CHOICE O-33)
- **Problem.**
  - Transactions after S are voided (`EARN-IN-CUTOVER-RUNBOOK.md:81`, `:96`).
  - Android installs cannot follow (`:434`) and keys cannot be exported (`deploy/standalone/README.md:171-173`).
  - The owner alone decides (DEC-06).
- **NODE-LOCAL parts:**
  - a builder flag that includes no user transactions after S, so nothing is voided;
  - publish old-chain data and rebuild instructions;
  - an Android key-export/migration path. Android installs an update only if it is signed by the installed
    app's signer or an authorized signing-key lineage (`deploy/standalone/RELEASE-UPGRADE.md:81-105`), so an
    export build signed only with the owner's new key cannot reach existing wallets. (Review fix.) In order:
    (1) if the key that signed the installed APKs can be recovered (for a debug-signed build, the debug
    keystore of the machine that built it), ship the export build signed with it, or with a v3 signing lineage
    from it to the new key; (2) otherwise, existing users move their funds before S with the installed app's
    Send action to an address of the new app (new key, new applicationId so both apps can be installed at the
    same time), and member identities deregister first. The cutover waits until this has been announced for
    long enough.
- **Policy part:** owner question O-33.

### V3-34 Fork machinery and verifier hygiene
Content as PR-1, PR-2 and PR-7 in §3.4.
- **Principle check:** the schedule semantics (`consensus.md:46`) are unchanged.
- **Tests:** a [(1,0),(2,H2),(3,H3)] schedule keeps nMax=2^64; `v3_active` is false for every old-chain block
  0..S; the wallet's bond features still work on the v3 program set.
- **Effort:** M–L. **Risk:** medium.

### V3-R Rejected
| Proposal | Fails |
|---|---|
| BFT/committee finality | Q1.3 (`decisions.json:18-20` D4) |
| Operator-signed checkpoints | Q6.2 |
| R_keep lowered as "finality" | Q1.2/Q6.1: arrival-time chain selection, eclipse capture |
| A different PoW hash | Q1.4 (T p79/62) |
| Per-identity hashrate caps | unenforceable under Sybil identities |
| Stake-weighted selection | Q1.3 |

Only a new explicit owner decision could reopen these.

---

## 5. OWNER-CHOICE items (answer with one word each)

Answer sheet: `O-0=… O-6=… O-10=… O-11=… O-12=… O-13=… O-14=… O-15=… O-17=… O-18=… O-19=… O-20=… O-21=… O-32=… O-33=…`
Recommended: `O-0=a O-6=b O-10=b O-11=b O-12=a O-13=b O-14=b O-15=b O-17=b O-18=b O-19=M3 O-20=b O-21=b O-32=b O-33=b`

**O-0 Vehicle for v3** — كيف يُطلق v3؟
- (a) Ride the held earn-in re-genesis (B), and build the in-place fork machinery in the same release.
- (b) No re-genesis: change the live chain in place at a height (A2, high risk, needs a testnet rehearsal).
- (c) Postpone v3.
- **Recommend a.** Almost every item changes program-read CP or system code, which only a new genesis delivers
  safely today. It is also the last cheap point while the operator still runs all three seeds.

**O-6 Block time** — زمن الكتلة.
- (a) Keep 10 s. Valid only for a single-host latency of about 1 s.
- (b) 30 s (D_att 150, dFbWait 10, Δ_rel 13). Valid if measured WAN Δ ≤ 2 s; otherwise use c.
- (c) 60 s (the governance profile: D_att 300, dFbWait 25).
- **Recommend b.** It passes the gate on a measured Internet, and it cuts the proposer's residual head start
  from +0.105 to about +0.066 at 20% hashrate.

**O-10 Contestable seats when the set is full** — هل يحق لقادم جديد أن يأخذ مقعد عضو لا يعدّن فعلاً؟
- (a) No contest; rely on 256 seats and inactivity removal.
- (b) Contest by work over one day, with a statistical margin (θ=1/8, 3σ), only when all seats are full, the
  victim refunded, and only together with V3-02.
- (c) The original 64-block rule. Not recommended: it evicts average honest members by chance.
- **Recommend b.**

**O-11 Inactivity removal delay Θ_inact** — مهلة إزالة العضو الخامل.
- (a) Keep 64 blocks (about 11 minutes at 10 s).
- (b) 1 hour.
- (c) 1 day. Take this only if O-10=b.
- **Recommend b.**

**O-12 Registration bond B_reg** — وديعة التسجيل.
- (a) 20 PCOL (already approved for earn-in).
- (b) 100 PCOL.
- (c) 5 PCOL.
- **Recommend a.**

**O-13 Probationers as proposers** — هل يصبح العضو تحت الاختبار مرشحاً لاقتراح الكتل؟
- (a) No (keep E5).
- (b) Yes, once its accrued bond is at least B_reg/4.
- (c) Yes, immediately.
- **Recommend b.** It ends "only operator servers propose", and equivocation still burns at least 5 PCOL.

**O-14 Probation pool** — حجم قائمة الاختبار.
- (a) Keep 8 slots, no contest.
- (b) M_max/4 slots (64), contest by work inside the pool, T_PROB = 30 days, both as CP fields (requires V3-09).
- (c) 64 slots, no contest.
- **Recommend b.**

**O-15 Uncovered share pot** — الجزء غير المغطى من حصة المشاركين.
- (a) Keep burning it (net issuance about 43–52% of gross and behaviour-dependent).
- (b) Carry it into the next block's pot at closeBlock (net issuance equals the schedule).
- **Recommend b.**

**O-17 Reward for reporting misbehaviour** — مكافأة المُبلّغ.
- (a) 0 (burn the whole bond).
- (b) 25% of the bond to the reporter, 75% burned.
- (c) 50%.
- **Recommend b.**

**O-18 Evidence window and bond delay** — نافذة الأدلة ومدة حبس الوديعة.
- (a) Keep 16 / 40 blocks.
- (b) 1 hour, with U = E_win + D + a margin.
- (c) The profile values, 2048 / 2200 blocks.
- **Recommend b.**

**O-19 Issuance schedule** — سياسة الإصدار.
- M0: keep 2 PCOL per block forever (6.31M per year at 10 s).
- M1: flat 2 PCOL per 60 s, scaled by T_blk (1.05M per year, no cap).
- M3: M1's rate halving every 4 years, with a tail of S0/32 (about 8.4M total plus a small perpetual tail).
- **Recommend M3.** It keeps a permanent security budget, bounds dilution, and with O-15=b net issuance equals
  the schedule.

**O-20 Fee market** — سوق الرسوم.
- (a) Keep: the proposer gets 5% of tips, and there is no base-fee floor.
- (b) The proposer gets 50% of tips directly (the rest stays in the shared pot), plus a base-fee floor of 0.01 gwei.
- (c) 100% of tips to the proposer, plus the floor. This moves fees out of the shared reward (touches P6).
- **Recommend b.** In every case the pool becomes fee-ordered with replacement and eviction (node-local).

**O-21 State caps** — حدود حالة السلسلة.
- (a) Raise the caps only (ACCT 200k, SLOT 500k, CODE 32 MiB).
- (b) Raise the caps and add a burned creation fee that rises as the cap fills (about 0.001 PCOL per new account
  at low use).
- (c) Network-scale caps now (blocked on K_eff, so effectively a deferral).
- **Recommend b.**

**O-32 The 4,000,000 PCOL genesis allocation** — مخصص التأسيس.
- (a) Publish the holder addresses and their purpose only.
- (b) Publish them, and move the development fund into a public time-locked account with published spending
  reports.
- (c) Publish them, and burn part of the allocation.
- **Recommend b.**

**O-33 How the re-genesis is run** — طريقة إعادة التكوين.
- (a) As in the current runbook.
- (b) Include no user transactions after S, publish the old-chain data, ship an Android key-export build first,
  and publicly commit that this is the last owner-only re-genesis; later consensus changes go through the fork
  schedule.
- (c) Like b, but without the public commitment.
- **Recommend b.**

---

## 6. The "64-seat committee", in plain language

**English.**
- **Who can mine.** Only addresses written in the on-chain miner registry may mine. In every block each of them
  gets its own slice of nonce numbers to search. That is the collaboration: nobody repeats anybody else's work,
  and everyone mines the same template block.
- **The seat limit.** The registry has room for at most 64 active addresses (M_max = 64).
- **Why "committee" overstates it.** The reviewers called this list a "committee", but it does not vote and does
  not sign blocks for validity. Every block is still won by proof of work, and a member's chance follows its
  computing power, not how many seats it holds.
- **Today.** Three of the 64 seats belong to your three servers, so they produce every block.
- **The real risk is the empty seats.** Anyone can take one by depositing a refundable bond (100 PCOL now,
  20 PCOL on the planned network). A well-funded party could fill all free seats in minutes, keep them with very
  little mining, and lock every newcomer out.
- **Two smaller doors.**
  - The single "new registration" slot per block can be kept busy by cycling the party's own seats.
  - The shared registry-operation budget can be used up by one member.
- **The v3 fix:**
  - 256 seats;
  - separate budgets so registrations can never be blocked;
  - a longer return delay for cycled seats;
  - optionally, a rule that lets a newcomer take over a seat whose holder is not really mining. The holder gets
    its bond back. It is judged over a whole day of recorded work, so honest small miners are not pushed out by
    bad luck.

**بالعربية.**
- **من يحق له التعدين.** لا يعدّن إلا العناوين المسجلة في سجل المعدّنين على السلسلة. في كل كتلة يأخذ كل عنوان شريحة خاصة
  به من أرقام الـ nonce. هذا هو التعاون: لا يكرر أحد عمل غيره، والجميع يعدّن الكتلة النموذج نفسها.
- **حدّ المقاعد.** يتسع السجل لـ 64 عنواناً نشطاً على الأكثر.
- **لماذا تسمية «لجنة» مبالغ فيها.** سمّى المراجعون هذه القائمة «لجنة»، لكنها لا تصوّت ولا توقّع الكتل. ما زالت كل كتلة تُكسب
  بإثبات العمل، وحظ العضو يتبع قدرته الحسابية لا عدد مقاعده.
- **اليوم.** تشغل خوادمك الثلاثة 3 من الـ 64 مقعداً، فهي التي تنتج كل الكتل.
- **الخطر الحقيقي في المقاعد الفارغة.** يستطيع أي شخص أخذ مقعد بوديعة تُردّ إليه (100 PCOL الآن و20 في الشبكة الجديدة). لذلك
  يستطيع طرف واحد يملك المال أن يملأ كل المقاعد الشاغرة في دقائق، ويحتفظ بها بتعدين قليل، فيغلق الباب أمام كل قادم جديد.
- **بابان أصغر.**
  - خانة «التسجيل الجديد» الوحيدة في كل كتلة يمكن إشغالها بتدوير مقاعد الطرف نفسه.
  - ميزانية عمليات السجل المشتركة يمكن أن يستنفدها عضو واحد.
- **علاج v3:**
  - 256 مقعداً؛
  - ميزانيات منفصلة حتى لا يُمنع التسجيل أبداً؛
  - مهلة أطول قبل إعادة تفعيل المقعد المُدوَّر؛
  - واختيارياً، قاعدة تسمح للقادم الجديد بأخذ مقعد صاحبه لا يعدّن فعلاً، مع ردّ وديعته. يُحكم على ذلك من عمل يوم
    كامل مسجّل، فلا يُطرد المعدّن الصغير الأمين بسبب الحظ.

---

## 7. Review findings: decisions

### Principles/economics review

1. **The R_max entry slot can be drained by deregister/reactivate cycling. ACCEPT (high).**
   - Verified: one counter for register and reactivate (`PoColSystemV1.sol:285`, incremented `:311`;
     `PoColSystemEarnIn.sol:322`). reactivate needs only `(w&255)==2` and the W_act cooldown, and restarts grace
     (`PoColSystemV1.sol:304-305`). ACCRUING records may reactivate before T_PROB (`PoColSystemEarnIn.sol:358`).
   - Refinement: the attacker's transaction must be ordered first; FIFO admission (`node.rs:919`) or its own
     proposals make that practical.
   - Adopted as V3-09. V3-14 option b is conditional on it.
2. **Contest threshold degenerates to cv = 0. ACCEPT (high).**
   - Verified: T_share = target·m (`chain.rs:442`) gives about 32 shares per block, so T ≈ 2048 over 64 blocks,
     and at active=256 the rule is cv < 1. The e^−8 × 1,080 gaps per day arithmetic checks out.
   - V3-10 is reclassified OWNER-CHOICE and redesigned: daily epoch counters, a 3σ margin, V3-02 as a
     prerequisite.
3. **Θ_inact = 8640 makes squatting about 135× cheaper. ACCEPT (medium).**
   - Verified at `PoColSystemV1.sol:300` and `PoColSystemEarnIn.sol:353`.
   - Now OWNER-CHOICE O-11; 1 day is offered only with O-10=b.
4. **Late credit plus carry breaks "fixed before mining", and late shares dilute. ACCEPT (medium).**
   - Verified: phase order `apply.rs:446-457`; `distribute` uses q = max(C, κm) (`PoColSystemEarnIn.sol:213`).
   - Adopted both fixes: carry enters at closeBlock (V3-15), and the no-dilution cap C_header + C_late ≤ κm
     (V3-02).
5. **Surcharge does not remove the cap attack; per-block ceiling griefable; A1 does not exist. ACCEPT (medium).**
   - Verified: 20,000 × 1e14 wei = 2 PCOL; `lib.rs:202-204`; the shared-budget halt pattern at
     `user_phase.rs:214-218`.
   - V3-21 now uses raised caps plus a utilisation-priced fee, drops the per-block ceiling, and ships via B.
6. **Dropping signed precedence reverses D15/D133. ACCEPT (medium).**
   - Verified: `consensus.md:236-238`; `decisions.json:663-665` (option C rejected for removing signed
     preference).
   - Replaced by V3-04's arrival-qualified key, which keeps D15 for timely templates and is recorded as a D133
     amendment in PR-0.
7. **The 7 s Δ_rel default is single-host only. ACCEPT (low).**
   - Verified: `gate.rs:258`; `testnet.rs:50-56`; `governance.md:6`.
   - V3-05 derives Δ_rel from measured Δ.
8. **Accused-only burn under-deters zero-bond equivocators. ACCEPT (low).**
   - Verified: zero-bond register `PoColSystemEarnIn.sol:320`, `:333-335`; tomb burns at
     `PoColSystemV1.sol:225-232`.
   - V3-16 keeps the whole-pot burn when the accused was ACCRUING below B_reg.

### Safety/implementability review

1. **A1 does not exist; `==2` checks revert v2 at a v3 height; version-only keys are off on the [[1,0]] network.
   ACCEPT (high).**
   - Verified: `p1_profile.rs:231-236`; `lib.rs:202-204`; `chain.rs:58`; `rp.js:56`; `pocol_rewards.py:277`;
     CP4=0 (`h15000/README.md:18`); the [[1,0]] schedule (`EARN-IN-SPEC.md:17`, `EARN-IN-CUTOVER-RUNBOOK.md:287`).
   - Fixed by PR-1 (`>=2`) and the single `v3_active` predicate (§3.3). Every "A1 now" is restated as later work.
2. **2a makes privately pre-mining the fallback profitable for any member. ACCEPT (medium).**
   - Verified: an a=0 fallback is valid whenever it arrives (`consensus.md:166`). Its ts = L(0)+dFbWait is
     importable from ts−Φ (`timing.rs:66-67`, `:115-116`), before T_j (`timing.rs:96-105`). It has an empty body
     and pays propPart to the winner (`PoColSystemV1.sol:194-196`).
   - Merged with principles finding 6 into V3-04 (early fallback ranks untimely), with a SimNet adversary in PR-8.
3. **The WAN timing example fails R3/R6; the 2c gains are about 3× optimistic. ACCEPT (medium).**
   - Verified by recomputing `timing.rs:128-130` and `gate.rs:258-262`: R3 21 > 12; R6 needs D_att ≥ 166 > 150.
     At w = 25/30 and f = 0.2 the gain is +0.123.
   - V3-06 offers only gate-valid options and recomputed gains.
4. **The R_max slot is a second capturable chokepoint. ACCEPT (medium; duplicate of principles finding 1).**
   - Also verified: R_max adds nothing to the continuous A13 identity bound (`resources.rs:441`), so raising it
     is cheap.
   - Part of V3-09.
5. **"finalized never moves backwards" is false. ACCEPT (medium).**
   - Verified: the floor follows the current head (`node.rs:714`; `chain.rs:757`), and work is compared first
     (`chain.rs:1086-1088`), so a heavier shorter branch lowers the floor. The in-memory value is lost on
     restart.
   - V3-26 persists finalized_h and uses it as the floor.
6. **Block-count parameters were set for 10 s; T_PROB/P_MAX are constants with host mirrors. ACCEPT (low).**
   - Verified: `PoColSystemEarnIn.sol:261-262`; `apply.rs:48-49`.
   - V3-14 makes them CP fields. Every horizon is set in seconds and converted; T_PROB is re-derived against
     issuance.
7. **Off-node verifiers and identity gates are missing from scope. ACCEPT (low).**
   - Verified: `pocol_rewards.py:272`, `:277`, `:281`; `apply.rs:54`; `chain.rs:49-52`; `cutover.rs:102`, `:120`.
   - Covered by PR-2 and PR-7.
8. **The carry Q4.2 argument misstates when carry_in is fixed. ACCEPT (low; duplicate of principles finding 4).**
   - Pot j−1 settles at block j−1+D (`apply.rs:457`), after block j.
   - Resolved by moving carry into closeBlock (V3-15). I chose this instead of the reviewer's alternative of
     lagging the carry by D blocks, because it also fixes the pot at the template.
9. **Attempt-dependent membership breaks the stored `drawn`. ACCEPT (low).**
   - Verified: `chain.rs:438-445` draws over the per-attempt membership; `:609` stores `drawn: ctx.proposer`;
     `:680-681` check type-1 evidence against it.
   - V3-01 is keyed on nonce mode, not on the attempt, so membership stays attempt-independent and the trap
     cannot arise.

**No review finding was rejected.** Two were merged as duplicates (safety 4 into principles 1; safety 8 into
principles 4). One fix was taken in a different form than proposed (safety 8: closeBlock injection instead of a
D-lag).

**Areas not covered by the input designs, added here:**
- EXEC-03 → V3-25 and V3-28.
- ECON-05 → V3-32.
- ECON-07 → V3-20.
- DEC-06 → V3-33.
- The vehicle itself → O-0.
