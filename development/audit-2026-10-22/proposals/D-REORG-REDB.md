# D-REORG-REDB — off-heap block store for full R_keep-depth adoption

Status: **APPROVED by the owner (2026-10-23) as a storage repair for the R_keep requirement, with D133
and the validity/consensus rules fully preserved; IMPLEMENTED** (`66d69c6`, `f7c345b`, `54b2da1`,
`9ce5a7c`). The final design, with the multi-branch adoption proof, replay equivalence, RPC scope and
resource bounds, is **D-REORG-REDB-DESIGN-FREEZE.md, Revision 2**, which supersedes the design sketched
below where they differ. Verification on the final code is recorded in REPAIR-REPORT.md. This approval
does not cover the six consensus proposals.

History (the proposal as first written, before approval):
Status then: PROPOSED, not implemented, needing explicit owner approval because it adds consensus-
critical code (a bespoke reorganization path), per the standing rule on new consensus-adjacent mechanisms.

> Round-2 correction (this revision). The earlier draft said the design would "adopt only on strictly
> greater total work (no equal-work deep reorg), which sidesteps the D133 tie-break entirely." That
> was wrong twice over: dropping the equal-work tie-break would **change** the fork-choice rule (D133)
> for deep forks, not sidestep it, and it is unnecessary. Fork choice reads only metadata (total work,
> parent, height, and `key = (signed, importSeq, blockHash)`), so the full D133 total order is computed
> from the resident metadata index with **no change to the rule**. This revision keeps D133 intact and
> makes the change storage-only; the strictly-greater shortcut is rejected.

## Problem

network.md:144 requires a node to adopt a heavier valid branch at any fork depth ≤ R_keep = 2048
(governance.md). pocold's one-shot adoption depth is **SIDE_BLOCKS_MAX = 320** (A13-DERIVATION §4
item 18), memory-bound, because:
- fork choice (`chain::better`, `rebuild_canonical`) walks a branch's ancestors through resident
  `self.blocks` entries, and `retain` can only drop leaves without creating holes, so a competing
  branch must be **fully resident** (header, body and executed state) to be adopted;
- a resident side block costs ~0.42 MiB in the A13 model; 2048 of them would add ~0.85 GB, which does
  not fit the 6 GiB budget beside the other terms.

A branch deeper than 320 is **preserved in the journal (lossless) and not refused**, but the node
stays on the valid lighter chain until the heavier branch is delivered within the cap or a manual
checkpoint is applied (consensus.md D118: "a branch that becomes heavier later is fetched by sync";
governance.md:200: "a rollback deeper than R_keep: manual checkpoint"). This is a liveness gap, not a
safety break: the node never adopts an invalid chain.

### A second, related gap this design closes (evicted-block refusal vs D118)

To fix a round-2 restart bug (duplicate journal records of re-fetched evicted blocks), import now
refuses a hash that retention has evicted (`chain::was_evicted`, `node.rs` import and replay). The
refusal is **temporary** — the evicted set is a FIFO of `EVICTED_MAX = 1024`, so an evicted hash is
forgotten after 1024 further evictions and can then be re-imported — but while it holds it is in
tension with D118's "a branch that becomes heavier is fetched by sync": a branch evicted at the cap
and then becoming heavier is refused on immediate re-delivery. Holding the whole R_keep window as
metadata (below) removes the eviction within R_keep, so the refusal no longer bears on any in-window
reorg.

## Proposed design

Move block **bodies and executed state** off the heap, keep only per-block **metadata** resident
within R_keep, and make reorg a **streaming** re-execution:

1. **Block store (redb or an append-only file + index).** `hash → (header, body, parentResult)` on
   disk; RAM holds only per-block metadata within R_keep: `hash, parent, height, total_work, signed,
   importSeq` ≈ 120 B, plus the executed-state snapshot only for the K_hist canonical window and the
   `side_max` retained side tips (exactly what `retain` keeps today). The journal already holds the
   bytes losslessly; this adds the by-hash index and on-demand reads (`Journal::read_at`, prototyped
   and reverted on this branch, kept in the private package).
2. **Fork choice on metadata — D133 unchanged.** `better`/`rebuild_canonical`/the LCA walk operate on
   the metadata index, not resident `Entry` values. They already read only `total_work`, `parent`,
   `height` and `key = (signed, importSeq, blockHash)` (verified in `chain::better`/`key`), so a
   competing branch need not be resident to be compared, and the **full D133 total order — cumulative
   work, then the LCA-children key — is preserved exactly**. No equal-work case is dropped.
3. **Two-pass streaming adoption (fail-closed).**
   - Pass 1 (validate, no mutation): stream the new branch from the LCA, re-executing one block at a
     time (rolling ≤ 2 state snapshots, as `ensure_state` already does), checking every state root,
     PoW, membership, evidence and parentResult. It touches nothing in the canonical head/index, so a
     failure needs no rollback.
   - Pass 2 (splice, only after Pass 1 fully succeeds): swap the canonical suffix height-by-height
     (spill the old block's state at height h, install the new one), keeping the resident state set ≈
     the K_hist window. A crash mid-splice is recovered by journal replay.
   Peak resident state ≈ the K_hist window + O(1), independent of the reorg depth, so it reaches
   R_keep within budget.

## Consensus vs storage — what this does and does not change

- **Storage/retention only (local, consensus.md:29 lists R_keep and K_hist as NodeLocal):** where
  block bodies and executed state live (disk vs heap); how deep a branch can be held resident; the
  `was_evicted` refusal within R_keep.
- **Fork-choice rule (D133):** **unchanged.** The metadata index computes the same total order. D133
  is already classified "a consensus chain-selection rule, does not change validity".
- **Block validity, state roots, gas rules, accepted-transaction semantics:** **unchanged** — every
  adopted block is fully re-validated in Pass 1 against the same rules.
- genesisHash: unchanged.

So the change is storage-only **in its rules**. It needs owner approval not because it changes a rule
but because of its **implementation risk** (below): it replaces the tested resident-`Entry` fork
choice and `insert`/`retain` path with a metadata index and a splice/restore executed while the chain
is mid-mutation — consensus-critical code whose bugs could corrupt the canonical chain or diverge two
honest nodes even though the rule on paper is the same.

## Why it needs your approval, beyond the current authorization

The standing authorization (`development/OWNER-STANDING-AUTHORIZATION.md`) and D-A13-BUDGET cover
*local, non-consensus* repairs and the selection of *local resource limits*. This design is outside
that envelope for three reasons:
1. **Consensus-critical code path.** Even with D133's rule unchanged, re-implementing fork choice and
   adoption on a new storage backing is new consensus-critical code. A defect reorganizes the chain
   wrongly. The standing authorization explicitly does not extend to new consensus-adjacent
   mechanisms without sign-off.
2. **It is net-new work, not a repair.** The audit finding (adoption ≤ R_keep) is already addressed
   defensively (cap raised to 320 on the tested path; deeper branches preserved losslessly, never an
   invalid adoption). Closing 320 → 2048 is an enhancement to meet the spec's full depth, not a
   correction of a wrong behaviour, so it is a scope decision for you.
3. **It wants a dedicated review.** A bespoke reorg path should get its own independent review and
   test plan (the acceptance vectors below) before it is written, not be folded into the audit repair.

## Effects (if approved and implemented)

- **Memory:** the canonical-history term (H_END+1 blocks resident) shrinks substantially (bodies and
  most state on disk; only the K_hist window + side tips keep executed state), freeing budget; the
  side term becomes metadata-only (~120 B × R_keep ≈ 0.25 MiB instead of 320 × ~0.42 MiB ≈ 132 MiB).
- **Disk:** a bounded block store. The spec's own figure (storage.md:73) is ≈ 19.65 GiB logical /
  ≈ 29.5 GiB on disk for the full retained history at H_END; the reorg store proper is ≤ R_keep ×
  max-block ≈ 0.4 GB live, with compaction.
- **Crash recovery:** unchanged in guarantee. The journal is the source of truth (two-slot commit
  file, `sync_all`; a block is committed iff its full fsynced record is counted). Restart replays and
  re-executes every committed block in import order, rebuilding state, receipts and `importSeq`
  exactly; a crash mid-splice leaves the node on the old chain or the fully-spliced new chain, never a
  partial mix (replay is atomic per record). The on-disk block store must itself be fsync-ordered
  behind the journal commit so a torn store read never outruns a committed record.
- **CPU:** a deep reorg now re-executes up to R_keep blocks from disk on adoption (bounded, but a new
  worst-case timing term — A13 keeps worst-case timing OPEN regardless).
- **Compatibility:** local only (retention/storage); does not change genesisHash or block validity.

## Independent expected-result acceptance vectors (for the review of an eventual implementation)

- A heavier branch forked exactly 2048 deep is adopted; forked 2049 deep is not (R_keep boundary).
- A branch that fails Pass-1 validation at block k leaves head/canonical unchanged (no rollback, no
  partial state).
- An **equal-work** competing branch forked > 320 and ≤ 2048 deep is adopted **iff D133's key**
  (signed, then importSeq, then blockHash at the LCA children) ranks it above the incumbent — i.e.
  the same decision `better()` makes today for an in-window fork, now at depth. (This is the vector the
  old "strictly-greater" draft would have failed.)
- A branch evicted at the old cap and later delivered heavier within R_keep is adopted, not refused
  (the in-window eviction is gone).
- A crash during Pass-2 splice: on restart the node is on the old chain or the fully-spliced new
  chain, never a partial mix.
- Memory under a full-depth reorg stays within the A13 budget (peak resident state ≈ the K_hist
  window), measured, not only modelled.
- Three-node devnet: a node offline across a > 320-deep heavier reorg catches up to the heavier head
  on reconnect without a manual checkpoint.
