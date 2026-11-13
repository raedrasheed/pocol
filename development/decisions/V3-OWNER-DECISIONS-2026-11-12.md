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
