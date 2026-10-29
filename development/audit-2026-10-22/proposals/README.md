# Consensus decision proposals (audit repair, round 2)

These proposals cover every repair decision that changes block validity, receipts or state roots
relative to a literal reading of the M1 reference. None is approved.

## Rules
- Each proposal quotes the exact normative text, cites file:line, and translates the Arabic.
- Each gives a proposed amendment, the alternatives with a recommendation, a minimal example with
  the outcome under every alternative, and the effects on state roots, receipts, gas, validity and
  compatibility.
- The historical M1 files (`development/m1/reference/*`) are not edited. An amendment exists only in
  its proposal file until the owner approves it.

## Status

| id | file | changes | approval | implementation |
|---|---|---|---|---|
| LOG-ACCT-01 | [LOG-ACCT-01.md](LOG-ACCT-01.md) | which logs halt (receipts, roots) | **PROPOSED**, not approved | recommended alternative (c) implemented provisionally |
| D-A03-DIRTY | [D-A03-DIRTY.md](D-A03-DIRTY.md) | what counts as dirty; when a halt makes the block invalid (validity, roots) | **PROPOSED**, not approved | recommended alternative A (net changes) implemented provisionally |
| D-A03-CAPS | [D-A03-CAPS.md](D-A03-CAPS.md) | what SLOT_CAP/ACCT_CAP count; the SYS_SLOT_MAX post-block check (validity) | **PROPOSED**, not approved | recommended alternative 2 implemented provisionally |
| D-BODY-MAX | [D-BODY-MAX.md](D-BODY-MAX.md) | encoded body versus item sum (validity) | **PROPOSED**, not approved | recommended alternative (a) implemented provisionally |
| D-A03-SYSUSER | [D-A03-SYSUSER.md](D-A03-SYSUSER.md) | what the user-context SYS allowance counts (halts) | **PROPOSED**, not approved | recommended alternative (a) implemented provisionally; vacuous for the v1 programs |
| LOG-CAP-AGGREGATE | [LOG-CAP-AGGREGATE.md](LOG-CAP-AGGREGATE.md) | the scope of LOG_CAP (halts) | **PROPOSED**, not approved | recommended alternative (chain) implemented provisionally |

**Decision package (2026-10-24):** [DECISION-PACKAGE.md](DECISION-PACKAGE.md) (Arabic summary: [DECISION-PACKAGE-AR.md](DECISION-PACKAGE-AR.md)) lists every open decision by your priorities, with the problem, the alternatives, a recommendation, the effects on consensus, genesisHash, compatibility and resources, and acceptance tests. It separates conformance work (being implemented) from normative changes (awaiting approval).

Related reviews (not decisions): [INDEPENDENT-REVIEW-SIX-PROPOSALS.md](INDEPENDENT-REVIEW-SIX-PROPOSALS.md)
frames the six for an independent reviewer. [LOG-CAP-AGGREGATE-ALTERNATIVES.md](LOG-CAP-AGGREGATE-ALTERNATIVES.md)
compares the ways to avoid a permanent LOG_CAP exhaustion and their consensus, storage and compatibility
effects. It is PROPOSED, not approved, and LOG-CAP-AGGREGATE stays unapproved.
[CAPS-SUSTAINABILITY.md](CAPS-SUSTAINABILITY.md) studies LOG_CAP, TX_CAP and TXBYTES_CAP together
(reachability before H_END, effects at exhaustion, genesisHash, an integrated recommendation); it is
PROPOSED, not approved. Its conformance items (the builder's skip, execution.md:83; template locking,
consensus.md:101; the caps-test build constant LOG_CAP, validation.md:2065) are implemented. The storage repair
D-REORG-REDB is owner-approved and implemented; it is not one of the six
([D-REORG-REDB.md](D-REORG-REDB.md), [design and proofs](D-REORG-REDB-DESIGN-FREEZE.md)).

Approval note: `development/OWNER-STANDING-AUTHORIZATION.md` approves recommended options for project
decisions. The owner's round-2 instruction asks that consensus changes be presented as explicit
proposals and not treated as approved because their implementation tests pass. These proposals
therefore stay PROPOSED until the owner decides each one.

"Implemented provisionally" means the branch carries the recommended alternative so that the other
repairs can be tested. It is not a decision. If the owner picks another alternative, the code and
its tests change, and blocks produced on a devnet under the provisional rule may become invalid under
the chosen one. No such chain exists outside local test runs.

## Decisions reviewed and not included
These repair decisions do not change validity, receipts or roots, so they need no consensus
proposal:
- D-A02-PROFILE: devnet parameters.
- D-A02-SCOPE: admission reporting.
- D-A04 journal v2: local storage.
- D-A05: local RPC/P2P limits, the light lane and the work deadline.
- D-A06: the wallet.
- D-A11: pool re-insertion.
- D-A13-PROFILE: devnet caps.
- D-DEP-SECP: an equivalent signature backend (secp-equivalence.md).

## Independent vectors
- `model/consensus_model.py` (Python stdlib only, run with `python3 -I`) restates each rule and
  every alternative from the normative text. It does not read the Rust implementation.
- `vectors/*.json` holds its expected results for every alternative.
- `model/caps_model.py` (also stdlib only) has its own Keccak-256 and GenesisSpec encoder. It writes
  `vectors/caps-sustainability.json`: the GSV1, devnet and cap-variant genesis hashes and the cap
  reachability. `lp4-execution/tests/caps_identity.rs` replays those vectors.
- The Rust tests replay the scenarios against the implementation and compare with the recommended
  alternative only:
  - `lp4-execution` `apply::tests::a03::proposals::{log_acct_01_vectors, d_a03_dirty_vectors,
    d_a03_caps_vectors, log_cap_aggregate_vectors}`;
  - `pocold` `consensus::d_body_max_vectors`.
  - They pass. That shows the code implements the recommendation. It is not evidence that the
    recommendation is right, and it is not an approval.

To regenerate: `python3 -I development/audit-2026-10-22/proposals/model/consensus_model.py
development/audit-2026-10-22/proposals/vectors`. The output is deterministic and matches the
committed files byte for byte.
