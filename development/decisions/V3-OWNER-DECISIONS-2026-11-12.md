# PoCol v3 — owner decision (2026-11-12): the all-probationer exception

> **Affected by the owner decisions of 2026-11-13**
> ([OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md](OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)): PoCol launches a
> brand-new network from a fresh genesis and keeps nothing related to the old network. This file stays as dated
> history.
>
> - The **second decision** (the user bound on the v3 creation burn, D-V3-21) concerns the creation fee of O-21,
>   which the owner removed: new accounts pay ordinary gas only (answer 10d). It no longer applies once the fee is
>   removed from the programs.
> - The **third decision** (the probation-pool contest victim, D-V3-14) concerns the seat and probation-pool
>   contests of O-10 and O-14, which the owner removed: no seat cap, no seat contest and no probation pool (answers
>   10, 10b, 10c). Answer 13 of 2026-11-13 approved it again but notes that it may become moot.
> - The **first decision** (the all-probationer exception, O-13) is not changed by the 2026-11-13 answers.
> - Every mention of the earn-in re-genesis or of live-network changes on the old network is moot (O-0, O-33).

Status: **decided by the owner, Raed S. Rasheed, on 2026-11-12.** It adds to
[V3-OWNER-DECISIONS-2026-11-08.md](V3-OWNER-DECISIONS-2026-11-08.md), item O-13. Approving a design rule is not
approval to deploy it. Every live-network change stays on hold until the owner releases it.

## Why the question was asked

O-13=b (2026-11-08) makes a probationer (ACCRUING) eligible to propose once its accrued bond is at least B_reg/4.
That leaves one case open: what happens when **every** draw member is a probationer below B_reg/4?

An earlier record (commit 3d526e0, 2026-11-11) cited an owner clarification for this case. The audit of 2026-11-12
found no verifiable owner source for it, so the question was put to the owner directly.

## The question put to the owner (2026-11-12, in Arabic, summarized)

What should happen when every draw member is a probationer below B_reg/4?
- **(a) Keep the exception. This was the recommended option.** All of them become candidates, so the network keeps
  running. This is the earn-in rule D1 ([EARN-IN-SPEC.md](EARN-IN-SPEC.md) §6). The cost: in that case a
  misbehaving proposer may lose less than B_reg/4.
- **(b) No signed proposer.** Only fallback blocks are made until a probationer reaches B_reg/4. This needs a new
  rule, because simply removing the exception would stop the chain.

## The owner's answer (verbatim)

> أوافق على (أ)، أبقِ الاستثناء

Translation: "I approve (a); keep the exception."

## Effect

- D-V3-13 keeps the existing earn-in D1 exception under v3. If the B_reg/4 filter would exclude every draw member,
  all of those probationers enter the deterministic, share-weighted draw. The dormant v3 code already does this
  (`lp1-node/src/assignment.rs`, `pocold/src/chain.rs`), so no code change is needed.
- The B_reg/4 minimum loss for a type-1 equivocation holds whenever at least one draw member qualifies. It does
  not hold in the all-probationer case. This is disclosed in V3-DESIGN.md (V3-13) and threat.md (A6).
- Collaborative consensus, the template block and every PoCol principle are unchanged. The draw stays
  deterministic and share-weighted.

---

# Second decision (2026-11-12): a user bound on the v3 creation burn (D-V3-21)

Status: **decided by the owner on 2026-11-12, about 20:20 UTC.** Approving a design rule is not approval to deploy
it.

## The question put to the owner (summarized)

The v3 creation burn (O-21=b) is limited only by the sender's whole balance, and its price depends on the
builder's transaction order. Near a full cap, a plain transfer to a fresh address could burn about 200 PCOL while
the user signed only a tiny gas fee. Four options were presented:
- **A (recommended):** a consensus check that the gas cost plus the creation burn stays within what the sender
  signed (`max_fee × gas_limit`); otherwise the transaction halts with `CreationFee`. `eth_estimateGas` finds the
  needed gas limit. Fully bounds the user's loss and keeps MetaMask compatibility. Plus a node-local burn quote RPC.
- B: a new signed field or transaction type carrying a burn cap (breaks MetaMask).
- C: node-local quote and refusal only (does not bound what another proposer includes).
- D: A plus pricing at the parent block's occupancy (changes the approved pricing rule and roughly halves the
  anti-fill cost).

## The owner's answer (verbatim)

> أوافق على الخيار أ، استمر في العمل

Translation: "I approve option A; continue working."

## Effect

- D-V3-21 gets the consensus bound of option A, v3 only: `gas_used × effective_price + creation_burn ≤
  max_fee × gas_limit` (types 0/1: the gas price), otherwise `Halt::CreationFee`; value stays separate, as in the
  Ethereum balance pre-check. A node-local burn quote RPC is added for wallets.
- When it is implemented and tested, the `D-V3-21 creation-burn user bound` entry leaves `V3_LAUNCH_BLOCKERS`.
- Collaborative consensus, the template block and every PoCol principle are unchanged: the check is a function of
  the transaction and ancestor state.

---

# Third decision (2026-11-13): the probation-pool contest victim (D-V3-14)

Status: **decided by the owner on 2026-11-13, about 06:30 UTC.** It adds to
[V3-OWNER-DECISIONS-2026-11-08.md](V3-OWNER-DECISIONS-2026-11-08.md), items O-10 and O-14. (The second decision of
2026-11-12, on the v3 creation-burn user bound, D-V3-21, is recorded above in this file.) Approving a design rule is
not approval to deploy it. Every live-network change stays on hold until the owner releases it.

## The question put to the owner (2026-11-13, in Arabic, summarized)

In V3-PR-E4 a probation-pool contest erased its victim (an ACCRUING member) at once. That had two effects:
1. The victim's parts of the pots not yet settled burned. They should have joined the bond that was refunded to it.
   This conflicts with V3-10's "refund, no stake penalty".
2. Evidence carried in a later block could no longer punish it. A seat-contest victim stays punishable until its
   due height.

Three options were presented:
- **(a) Recommended.** Keep the victim EXITED, payable and punishable, until its pots settle or until U.
- (b) Route the unsettled parts to its reward address.
- (c) Accept the burn.

## The owner's answer (verbatim)

> أوافق على (أ)، استمر في العمل

Translation: "I approve (a); continue working."

## Effect

- A pool-contest victim leaves the active list and becomes EXITED exactly as a seat-contest victim does, which is
  also how deregister exits a record: exitHeight = h, dueHeight = h + U. It is refunded at dueHeight by the sweep or
  by claimBond, with no burn.
  - Until then its parts of pending pots, and any late credit to them, are paid to it as to any exited record. For an
    ACCRUING record they fill its accrued bond first, so the refund includes them. A bond that reaches B_reg
    promotes the record to HELD.
  - Until then it can be punished: the bond burn, and V3-16's whole-pot burn as pot j's ACCRUING winner.
  - Not decided separately (an implementation consequence, for the owner to confirm or forbid): as an exited record
    it may also reactivate inside the usual window, because deregister's exit allows that. An ACCRUING victim
    reactivates only into a free pool seat; one its pots promoted to HELD reactivates as any HELD record. Reactivation
    keeps registeredAt, so the T_PROB deadline still runs, but resets graceStart. It therefore meets the open point
    already recorded in D-V3-10's implementation note: a member that exits and reactivates once per epoch is never
    contestable.
- The newcomer takes the freed probation seat at once. ACC_COUNT now counts only the ACCRUING records that hold a
  pool seat.
  - The victim leaves ACC_COUNT in the contest call. A new record bit, POOL_OUT (w0 bit 87), marks it.
  - Its later refund, punishment or promotion does not lower ACC_COUNT again. Reactivation clears the bit and takes
    a pool seat again, with Capacity if the pool is full.
  - held_total is unchanged by the contest. It falls at the refund or the punishment. Escrow balance = pending +
    held + burned holds after every step.
  - Live ACCRUING records can now outnumber P_MAX, because exited victims are not counted. Settlement is still bounded:
    one pot fills at most P_MAX bonds. Its payees come from the ActList of its block's parent, and every ACCRUING
    member of that list holds a pool seat. So GAS_ENVELOPE_V3 settle is unchanged (review of this decision;
    REGISTRY-LAYOUT.md "v3 contestable seats").
- Everything else in V3-PR-E4 is unchanged: the full-set requirement, the statistic and the budgets. The change is
  in the dormant v3 programs only (PoColSystemV3.sol MinerRegistry). The spec-1 and earn-in programs, SPEC_VERSION,
  MAX_FORK_VERSION, CONSENSUS_RULES_VERSION and the identity are unchanged.
- Collaborative consensus, the template block and every PoCol principle are unchanged.
