# Independent-review package for the six consensus proposals (NOT an adoption)

Prepared at the owner's request (2026-10-23) to support an independent review of the six consensus
proposals. **None is approved or merged as a final rule by this document.** Each remains PROPOSED; the
implementation carries the recommended alternative only so the other repairs can be tested (see
proposals/README.md). The historical M1 files are unchanged. This package frames the decision for a
reviewer: the question, what turns on it, the liveness/continuation risk, and what to check — it does
not decide.

Scope note: D-REORG-REDB is a separate, owner-approved storage repair (D133/validity preserved) and is
not one of these six.

## How to read each row

- **Decision:** the single normative question the spec text leaves open.
- **Turns on it:** state roots / receipts / validity / halts (what diverges between alternatives).
- **Liveness / continuation risk:** whether an alternative can brick or stall normal operation.
- **Reviewer check:** the concrete thing to verify against the accepted text and the independent model.

| # | proposal | decision | turns on | liveness / continuation risk | reviewer check |
|---|---|---|---|---|---|
| 1 | LOG-ACCT-01 | per-log charge against LOG_BLOCK_MAX: `36 + 32·topics + data` with a ≤546 count (rec. c) | state root, receipts, gas (a halted tx reverts to nonce+fee+tip) | low per block (32 KiB cap only halts the over-limit tx, never the block) | that (c) alone reproduces LG2 (546 ok/547 halt) and P1's ≤546-logs receipt bound; (a),(b),(d) each violate one quoted line |
| 2 | D-A03-DIRTY | what "dirty" counts (net per-tx, rec. A); and a halt whose own residue won't fit → block invalid | validity (A adds one block-invalid case), roots, receipts | low and only at GAS_LIMIT ≥ ~120M (unreachable at devnet 8M); A's invalid case mirrors TX_CAP (builder skips) | that A's new block-invalid condition is the same *kind* as the existing TX_CAP one, not a new validity axis; that B's exemption would enlarge every ACCT_DIRTY-derived bound (incl. A13) |
| 3 | D-A03-CAPS | do SLOT_CAP/ACCT_CAP count the system programs; when is SYS_SLOT_MAX checked (rec. 2: user-state caps; SYS_SLOT_MAX after the system phases) | validity, roots, receipts, genesis acceptance | **medium under alt 1:** coupling user caps to program storage means the builder must predict the system phase or produce invalid blocks → can stall production near the cap. Alt 2 removes that coupling | that alt 2 is the same split R21's bound already uses; that R18 makes the post-phase SYS_SLOT_MAX check unreachable for an admitted profile (so it is not a live validity risk) |
| 4 | D-BODY-MAX | BODY_MAX bounds the RLP-encoded body on every path (rec. a), not the item sum | validity (import now matches decode) | low; it *removes* a self-inconsistency (a block one path accepted and another rejected), improving liveness | that no spec-valid block becomes invalid (the spec bounds the encoded body); that alt b would force a decode change and RECV header-slack |
| 5 | D-A03-SYSUSER | what the user-context SYS allowance counts (rec. a: program storage keys/accounts; tip and factory nonce are **not** writes) | halts only (no validity change) | low; **vacuous for the v1 programs** (no USER path writes program storage). Alt b would halt every tipping tx during closing (contradicts D70) | that a tip is a balance change, not a system key (D70); that (a) never triggers with v1 programs |
| 6 | LOG-CAP-AGGREGATE | scope of LOG_CAP = 1 GiB (rec. chain: cumulative charged log bytes over the canonical chain) | halts, roots, receipts once the chain nears the cap; **and the A13 history bound** | **HIGH in principle — see the dedicated section below.** Cumulative-forever means an eventual permanent, chain-wide halt of all logging | that "chain" is the only node-consistent reading (block is vacuous; retained is local, not consensus-safe); then weigh the exhaustion cliff below |

## LOG-CAP-AGGREGATE: what happens at exhaustion, and the effect on continuation (owner's question)

**The rule (recommended "chain" alternative).** `LOG_CAP` bounds the sum of *charged* log bytes
(LOG-ACCT-01: `36 + 32·topics + data` per log) over the **canonical chain, this block included**. The
first transaction in a block that would push the running total above `LOG_CAP` **halts exceptionally**
(status 0, whole gas limit charged, every effect reverted except the sender's nonce+fee and the Escrow
tip). The total is monotone — it only grows as the canonical chain extends; nothing lowers it.

**What exhaustion looks like.** Once the cumulative charged log bytes reach `LOG_CAP = 1 GiB`
(release) — or `2^24 = 16 MiB` under the simnet caps-test build (validation.md:2065) — the condition is
**permanent and chain-wide**:
- Every subsequent transaction that emits **any** log halts on the first such log, on every branch
  descending from that point (the total is a function of the canonical ancestors).
- A transaction that emits **no** log is unaffected and still succeeds. So value transfers that emit
  nothing continue; anything that logs does not.
- Because almost every standard contract interaction emits an event (an ERC-20 `Transfer`, a registry
  update, most settlement paths), the practical effect is that **event-emitting contract activity stops
  succeeding for the remaining life of the chain**, while non-logging transfers continue. The chain
  does not halt block production (fallback/signed templates still close blocks of halted or
  non-logging transactions), so it is a *functional* brick of logging, not a liveness stop of the
  consensus itself.

**Why it is nonetheless a real concern for the owner.**
- It is a **one-way cliff with no recovery path in the rule.** There is no per-epoch reset, no decay,
  no window. The only escape the spec offers for anything "deeper than retention" is the governance
  manual-checkpoint / genesis path (governance.md:200, :230 Shamir 2-of-3), which is an out-of-band
  operational action, not an in-protocol recovery.
- **Reachability depends on the profile — and the network profile CAN reach it.** The cap is reached
  only under sustained near-maximum logging (`LOG_BLOCK_MAX = 32 KiB`/block). Whether that is possible
  within the chain's own maximum length is `H_END · LOG_BLOCK_MAX` vs `LOG_CAP`:
  - a short devnet (small `H_END`) cannot reach 1 GiB — unreachable, cliff never hit;
  - the **network** CP has `H_END = 129600`, so `129600 · 32 KiB ≈ 3.95 GiB > 1 GiB` — a log-heavy
    network chain **reaches the cap before its own end** and then hits the permanent cliff. This is the
    case the owner should weigh; the reviewer should redo this arithmetic for the exact profile.
  - the simnet caps-test lowers `LOG_CAP` to `2^24 = 16 MiB` precisely so a test can reach it.
- The A13 history bound **uses this reading**: `history/logs (LOG_CAP)` counts `LOG_CAP` worth of
  log bytes as the resident history term. *(Corrected 2026-10-23, after the alternatives review:)* the
  term is finite under any reading. A canonical chain has at most H_END blocks (storage.md:73), each
  with at most LOG_BLOCK_MAX charged log bytes, so logs ≤ H_END·LOG_BLOCK_MAX: 3.955 GiB on network,
  625 MiB on the devnet. LOG_CAP lowers that bound on network; on the devnet it does not bind. The
  earlier "unbounded under 'block'" was imprecise. Under "retained" the term would track a local window,
  which is not consensus-safe and differs per node.

**The reviewer's decision.** "chain" is almost certainly the intended reading (it is the only one that
is node-consistent, makes the caps-test reachable, and bounds A13). The open question the owner should
weigh is **not** which alternative, but whether the **cumulative-forever cliff** is acceptable as a
protocol property, or whether a future amendment should add an in-protocol escape (e.g. a cap scaled to
`H_END` so it is provably unreachable within the chain's life, or an explicit epoch reset). That is a
new normative question beyond the current six proposals and is flagged here, **not** decided or
implemented. The alternatives that avoid permanent exhaustion are reviewed, with their consensus,
storage and compatibility effects, in `LOG-CAP-AGGREGATE-ALTERNATIVES.md` (PROPOSED, not approved).
That review also flags TX_CAP and TXBYTES_CAP: both are chain-cumulative and reachable before H_END on
the network profile, and reaching either makes blocks invalid. This is not decided here.

## Independent vectors (already present, for the reviewer to re-run)

`proposals/model/consensus_model.py` (Python stdlib only; run `python3 -I`) restates every rule and
every alternative from the normative text and does not read the Rust. `proposals/vectors/*.json` holds
its expected results per alternative. The Rust tests replay the recommended alternative only
(`lp4-execution apply::tests::a03::proposals::*`, `pocold consensus::d_body_max_vectors`) and pass —
which shows the code matches the recommendation, not that the recommendation is correct, and is not an
approval. Regenerate deterministically with
`python3 -I development/audit-2026-10-22/proposals/model/consensus_model.py <vectors-dir>`.

## Status

All six remain **PROPOSED, not approved**. The implementation uses the recommended alternatives
provisionally and its passing tests are not an adoption. The LOG-CAP exhaustion cliff is surfaced as a
possible follow-up normative question, not folded into any of the six.
