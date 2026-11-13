# PoCol v3 — owner decision (2026-11-12): the all-probationer exception

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
