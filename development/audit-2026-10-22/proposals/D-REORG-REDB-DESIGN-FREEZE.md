# D-REORG-REDB — frozen design and acceptance tests (owner-approved implementation)

Status: **APPROVED for implementation** as a repair to meet the R_keep adoption requirement, D133 and
block validity fully preserved (owner message 2026-10-23). The six consensus proposals are **not**
approved by that message. This document freezes the design and the acceptance tests **before** any
implementation, per the owner's point 1. Any deviation that would change a normative rule stops for
explicit approval.

## 1. Goal and non-goals

- **Goal:** adopt a heavier valid branch at any fork depth ≤ R_keep = 2048 (network.md:144), within the
  6 GiB budget, with D133 and block validity unchanged.
- **Non-goals (unchanged):** D133 fork-choice rule; block validity / state roots / gas / accepted-tx
  semantics; genesisHash; the six consensus proposals. The local retention limit is **not** turned
  into a block-validity rule (owner point 4).

## 2. Key facts the design rests on (verified in the current code)

1. `chain::better`/`key`/`rebuild_canonical` read only metadata — `total_work`, `parent`, `height`,
   and `key = (signed, importSeq, blockHash)` — never a body or executed state
   (`src/chain.rs` `better` lines 462–476). **So fork choice already computes the full D133 total
   order from metadata; no rule changes.**
2. Every validated imported block is already journaled, canonical or side, before `insert`
   (`src/node.rs` import: `journal.append(encode_record(header, body, result))` at line 244). **The
   journal is already a complete, durable, off-heap store of every block's bytes.**
3. `retain` is the only thing that bounds memory, and it does so by **removing** side entries past
   `SIDE_BLOCKS_MAX = 320` (`self.blocks.remove`), because a resident `Entry` carries the full
   `body`, `receipts`, `tx_hashes` and (within the window) `state`.
4. `ensure_state` already rebuilds a pruned block's state on demand by rolling re-execution from the
   nearest ancestor that still has state, using each block's `body`.

So the change is **storage, not fork choice**: give the journal a by-hash index, let `Entry` hold its
body/receipts/state off-heap (in the journal) beyond a small resident window, and let `retain` keep
**metadata** for the whole R_keep window instead of deleting side blocks at 320. Adoption then reaches
R_keep through the existing `better` + `ensure_state` path.

## 3. Design

### 3.1 Block store = the journal + a by-hash index

- `Journal::append` returns the record's **frame offset** (`u64`, the value of `end` before the write).
- New `Journal::read_at(offset) -> Result<(header, body, result), StoreError>`: seek, read the frame,
  verify `lenCheck` and `sha256(payload)` (same checks as replay), `decode_record`. A failed check is
  a hard error (never a silent empty body).
- A by-hash index `offset_of: HashMap<[u8;32], u64>` lives in `Chain` (the metadata owner), filled:
  - on live append: the node sets `entry.body_off = Some(off)` after `append` returns `off`;
  - on replay: the node already decodes every payload into `(header, body, result)` and forms the
    `Block`; it records `off` (recomputable: offsets start at 40, each frame is `payload.len() +
    OVERHEAD`) as it goes.
- The journal file itself is unchanged on disk (append-only, two-slot commit, fsync-ordered). The
  index is in-memory, rebuilt on every open from the committed records — never a new on-disk format.

### 3.2 Entry: resident window vs off-heap

`Entry` gains `body_off: Option<u64>` (journal frame offset) and an `offloaded: bool`. An **offloaded**
entry has `body`, `receipts`, `tx_hashes` and `header.encoded` cleared (empty), keeping only metadata:
`hash, parent, height, total_work, import_seq, signed, ts, target, base_fee, gas_used, seal, is_final,
result, cum_txs, cum_tx_bytes, cum_log_bytes, drawn, ranges`, and the parsed `header` fields (minus the
`encoded` bytes). State is already governed by `pruned`.

Residence policy (deterministic, tied to the existing window):
- **State** (`pruned`): resident for the canonical `K_hist` window and the `side_max` retained side
  tips — **unchanged**.
- **Body/receipts/tx_hashes/header.encoded** (`offloaded`): resident for the same set that keeps state
  (not pruned) **plus just-imported blocks until the next `retain`**; offloaded otherwise. So an
  offloaded block always has its bytes in the journal (it was journaled at import), and `body_off` is
  set.
- Anything needing offloaded content fetches it with `read_at(body_off)`: `ensure_state`
  (re-execution), `GetBlock`/`GetRange` (serving peers), the A11 reorg (abandoned-branch tx recovery,
  re-deriving `tx_hashes` from the fetched body). RPC history reads stay inside the trie window
  (`K_eff ≤ K_hist`, consensus.md §10: outside it is −32017), so RPC never needs an offloaded body.

### 3.3 retain: keep R_keep metadata, enforce D118 per height

`retain` changes from "delete side blocks past a count" to:
1. Prune **state** at height ≤ head−K_hist (unchanged) and **offload** body/receipts/tx_hashes for any
   block whose state is pruned (set `offloaded`, clear bytes, keep `body_off`).
2. Enforce **D118 per height** within the window `[head−R_keep, head]`: at each height keep the
   canonical block and **at most one** non-canonical block — the one highest in cumulative work for
   its branch, then earliest `importSeq` (consensus.md §10 / D118). A losing same-height non-canonical
   block and its descendants are removed.
3. Remove any non-canonical block at height ≤ head−R_keep, with its descendants (unchanged boundary).
4. Keep executed state for at most `side_max` side tips (unchanged).

Count bound after `retain`: ≤ (R_keep+1) canonical metadata entries within the window + ≤ R_keep
non-canonical metadata entries (one per height) = **≤ 2·R_keep+1 ≈ 4097 entries**, each metadata-only
beyond the resident window.

### 3.4 Adoption (unchanged mechanism, now reaching R_keep)

A heavier branch is adopted exactly as today: its new child is imported → `validate` (needs the
parent's state, obtained by `ensure_state` on `NeedsRebuild`, which now fetches offloaded bodies from
the journal) → `insert` runs `better` on metadata → `rebuild_canonical` walks parents → `retain`. No
new splice path, no new fork-choice code. `better` already preserves D133.

### 3.5 Crash consistency and write ordering

Invariant: **the journal is the source of truth.** On restart, replay re-executes every committed
block in import order and rebuilds the chain, state, receipts, `importSeq` and the by-hash index
exactly (A04, already verified). The in-memory metadata index and offload flags are derived state,
rebuilt on open; they are never the source of truth and are never persisted. Write ordering is
unchanged: `append` writes the record, `sync_data`, then updates the fsynced commit file; a block is
committed iff its full record is fsynced and counted. Because offload only ever drops bytes that are
already durably in the journal, no crash can lose data the journal did not already hold. There is no
new on-disk artifact and no new ordering constraint.

## 4. Resource accounting — multiple branches (owner point 2)

Per-branch, within the R_keep window, bounded explicitly:

| item | count bound | size bound | residence / eviction | refetch |
|---|---|---|---|---|
| canonical metadata | ≤ R_keep+1 | ~1–2 KB each (header fields, result, ranges, cum_*) | resident within R_keep; removed beyond | n/a (in journal) |
| non-canonical metadata | ≤ R_keep (D118: ≤1/height) | ~1–2 KB each | resident within R_keep; the D118-loser per height and >R_keep removed | re-fetched by sync if it becomes heavier (D118) |
| executed state snapshots | ≤ K_hist+1 canonical + side_max side | ≤ snapshot bound each | pruned at height ≤ head−K_hist; side past side_max pruned | rebuilt by `ensure_state` re-execution |
| resident bodies/receipts | = the not-pruned set (K_hist + side_max + just-imported) | ≤ BODY_MAX + receipts each | offloaded when state is pruned | `read_at` from journal |
| journal records (disk) | all imported within retention | ≤ MAX_RECORD each | append-only; **compaction past R_keep is a documented follow-up** | the durable copy |
| by-hash index (RAM) | = resident metadata entries | 40 B/entry | rebuilt on open; grows/shrinks with metadata | n/a |

**Memory model (A13, 6 GiB kept):** the dominant resident terms shrink because offloaded canonical and
side blocks no longer carry body/receipts/logs/state. The side term becomes metadata-only; the history
`blocks`, `logs` and body terms count only the resident window, not H_END/R_keep. Exact bytes go in
A13-DERIVATION after implementation; the boot gate and 1/8 reserve are unchanged. **Proven bounds are
separated from measured RSS and from assumptions** (A13-DERIVATION §2b layout).

**Disk:** the live working set is ≤ (2·R_keep) × MAX_RECORD. The append-only journal file holds all
imported history absent compaction (a pre-existing property, unchanged by this change); compaction to
bound it past R_keep is recorded as a follow-up, not implemented here, and is local (no validity/root
effect).

## 5. Acceptance tests (frozen before implementation)

Derived from the accepted text; a retention/storage limit is never turned into a validity rule.

1. **Depth 2048 adoption.** Build a canonical chain, fork a strictly-heavier branch 2048 deep from an
   ancestor at depth 2048, deliver its blocks, assert the node adopts it (head = the heavy branch tip)
   and serves/produces on it. Memory stays within the A13 budget (asserted via the model).
2. **2049 behaviour from the text, not a new validity rule.** A heavier branch forked 2049 deep:
   network.md:144 adopts "at depth ≤ R_keep"; governance.md:200 routes deeper rollbacks to a manual
   checkpoint, and D118 keeps retention "local, not validity". So at 2049 the node **does not adopt in
   one shot and does not mark the branch invalid** — it stays on the valid lighter chain; the deeper
   branch is preserved in the journal. The test asserts: head unchanged, the 2049-branch blocks are
   **not** marked invalid (no `Reject::Block`), and no panic. It does **not** assert the branch is
   refused as invalid (that would be inventing a validity rule).
3. **D133 tie-break at depth.** Two equal-cumulative-work branches forked > 320 and ≤ 2048 deep; assert
   the head is the one D133's key selects (signed before fallback; then smaller importSeq; then smaller
   blockHash at the LCA children) — identical to the in-window decision, now at depth. Mutating the key
   order flips the result (mutation control).
4. **Evicted-branch re-fetch within R_keep.** A branch dropped by the old count cap is, under the new
   retention, kept as metadata within R_keep and adopted when it becomes heavier **without** being
   refused; a branch beyond R_keep is still removed and its re-delivery handled by sync (D118).
5. **Crash consistency.** Inject a crash (a) between the journal `append` fsync and the commit update,
   (b) mid-adoption (after `insert`, before `retain` completes), (c) with an offloaded body: after
   restart the head, canonical chain, executed state, by-hash index and journal are mutually
   consistent — head is the old chain or the fully-adopted new chain, never a partial mix — and every
   resident block's offloaded body is readable from the journal.
6. **Off-heap read fidelity.** For a block whose body is offloaded, `read_at` returns bytes that
   re-hash to the block hash and re-execute to the committed state root (so an off-heap body is never
   silently wrong or empty).
7. **6 GiB budget under multi-branch load.** With the window full of canonical + one non-canonical
   per height across R_keep, the A13 model total stays within the 6 GiB budget behind the 1/8 reserve,
   and measured RSS is reported separately (not as the bound).
8. **Positive controls** for tests 1, 3, 4, 6 (the scenario is built and the adopted/served path runs),
   so a pass cannot mean the path was never reached.

A **TIMEOUT** on any test is recorded as incomplete, never as a pass (owner point 6).

## 6. Independent review before coding

This frozen design is sent to an independent reviewer (a separate agent instance) to refute: the D133
claim (metadata suffices), the crash invariant, the D118-per-height count bound, and the 2049 "not a
validity rule" reading. Implementation begins only after that review; its result is recorded in the
repair history.

---

## Revision 1 (after the independent adversarial review)

An independent agent instance tried to refute the four load-bearing claims. Result: CLAIM 1 (D133 on
metadata) CONFIRMED, CLAIM 2 (journaled-before-offload crash invariant) CONFIRMED, CLAIM 4 (2049 is not
a validity rule) CONFIRMED in substance, **CLAIM 3 (the §4 count bound) REFUTED on two points**, plus a
miscitation and a broader-than-stated rewiring. All verified against the code and spec. The frozen
design above is superseded by this revision where they conflict; it is kept for the record.

### R1. Canonical metadata is resident for the whole chain (≈ H_END+1), not R_keep (fixes §3.3/§4)

`retain` removes only non-canonical blocks: `gone` is built from `side(self) = !is_canonical`
(chain.rs:405-407), and `rebuild_canonical`/`canonical: Vec`/`is_canonical`/`canonical_at` and the
`better` LCA walk all require the full head→genesis chain. **Canonical metadata is therefore never
evicted** and scales with H_END (= 129600, governance.md:16; already the model's
`history/blocks (H_END+1)` term), not R_keep. The "≤ 2·R_keep+1 ≈ 4097 entries" bound in §4 is wrong
(≈ 63× undercount); removing canonical metadata to make it true would panic `rebuild_canonical` and
break `is_canonical`, `GetRange` history and the LCA walk, so it must not be attempted.

**Corrected accounting.** This change does **not** reduce the canonical metadata count; that term
(history/blocks ≈ H_END+1 entries) is unchanged from today and already in the A13 model. The memory
**win** is elsewhere: offloading each block's **body, receipts and log bytes** beyond the K_hist
window moves the large *aggregate* resident terms — `history/logs (LOG_CAP)` ≈ 1.03 GiB,
`history/transaction bytes` and `history/.../receipts` — down to the K_hist window only (they are read
on demand from the journal outside it). Net resident drops ≈ 1 GiB. A13 is re-derived with: the
metadata term on H_END (unchanged), the offloaded byte terms on the K_hist window, and the side term
on the metadata cap below.

### R2. One-per-height + the evicted refusal loses a valid heavier branch (fixes §3.3; the core bug)

`retain` adds **every** removed hash (including a per-height-losing non-canonical branch dropped with
its descendants) to the `evicted` set (chain.rs:425); `import` (node.rs:227) and `replay` (node.rs:141)
then **refuse** any evicted hash. Counterexample: two side branches A, B fork from a canonical ancestor
within R_keep; A is marginally heavier per height, so retain keeps A and removes all of B → B's hashes
are evicted. B later gets a heavy extension and becomes the single heaviest valid branch; on
re-delivery its blocks are refused by `was_evicted` until the 1024-entry FIFO drains
(adversarially controllable). A **valid heavier branch within R_keep is not adopted** — defeating the
Goal and contradicting the proposal's "holding R_keep metadata removes the eviction within R_keep."
D118 explicitly expects the dropped branch to be "يُعاد جلبه بالمزامنة إن صار أثقل" (re-fetched by sync
if it becomes heavier); the refusal blocks exactly that path.

**Fix (two parts, both within D118/D133):**
1. **Retain competing-branch metadata under a generous cap**, not just one per height. Raise retention
   from `SIDE_BLOCKS_MAX = 320` full entries to `SIDE_META_MAX` *metadata* entries (each ~1–2 KB vs
   ~0.42 MiB), evicting the lowest-work leaf first only past the cap (the existing `retain` leaf
   algorithm, now on metadata). With a cap well above the realistic number of competing valid
   branches, two branches within R_keep are both retained and `better` adopts the heavier **directly,
   with no re-fetch**. D118 mandates *at least* canonical + 1 non-canonical per height; keeping more is
   a local retention choice that only improves liveness and changes no rule.
2. **Scope the evicted refusal to beyond R_keep**, so D118 re-fetch works when the cap is exceeded. A
   re-delivered block is refused only when it cannot lead to an adoptable branch, i.e. when it is
   beyond the window (`height + R_KEEP <= head_height`); within the window it is accepted and
   re-validated (D118 re-fetch). Replay stays correct because a re-fetched branch is journaled
   parent-before-child (orphan resolution fetches the parent first), so on replay its records are in
   ascending height and no record hits UnknownParent; the replay `was_evicted` skip is scoped the same
   way. The original purpose of the refusal — stopping an *unadoptable* (beyond-R_keep) branch from
   being re-fetched/re-executed/re-journaled every sync turn — is preserved.

New acceptance test (added to §5): **two competing branches.** A and B fork within R_keep; A leads, so
retain may drop B; B then becomes heaviest. Assert B is adopted (head = B tip) — directly if under the
metadata cap, or via re-fetch if B was evicted — and never permanently refused within R_keep. Positive
control: the adopted-B path executes.

### R3. Offload rewiring is broader than §3.2 stated (acknowledged, scoped)

- `ensure_state` is a `Chain` method but the journal lives in `Node`; its signature and every call
  site must thread a journal reader, including the **replay** loop (node.rs:136-150) which runs before
  the `Node` exists and must read through `rec.journal`. Borrow-checker-feasible (disjoint fields),
  but a real refactor, not a one-liner.
- `detached_txs` (A11, node.rs:317) reads `e.body.txs` and `e.tx_hashes` of abandoned blocks that may
  be offloaded; `tx_hashes` is **not** in the journal record (record = header/body/result), so it is
  re-derived by re-hashing the fetched body (keccak per tx).
- `GetBlock`/`GetRange` (node.rs:544-548) and `eth_getBlockByHash` (rpc.rs:431) send
  `header.encoded`/`body.encode()`, empty when offloaded — they must read from the journal. Serving
  peers is the very mechanism the D118 re-fetch in R2 relies on, so this must land.

### R4. Genesis-never-offloaded invariant (documented, guarded)

The offload invariant ("offloaded ⇒ bytes in the journal") holds because genesis is created in
`Chain::new` (never journaled, chain.rs:141-144) **and** is never offloaded: `retain` excludes height
0 from state pruning (chain.rs:404), and offload is gated on pruned state. The implementation asserts
`height > 0 && body_off.is_some()` before offloading, so a future change that pruned genesis would fail
loudly rather than create an offloaded entry with no journal backing.

### R5. Citation fix (both files)

The manual-checkpoint-beyond-R_keep text is **governance.md:200** ("تراجع أعمق من R_keep: checkpoint
يدوي"), and the rollback-to-R_keep-then-TrieRebuild text is **consensus.md §10 (التعافي, lines
227-231)** — not "consensus.md §11", which is "التقدم (مشروط)". Fixed here and in D-REORG-REDB.md.

### Net effect on the plan

The design remains storage-only in its rules (D133 and validity unchanged) and still reaches R_keep.
The corrections are: count the canonical metadata on H_END (not R_keep) in A13; retain competing side
metadata under `SIDE_META_MAX` and scope the evicted refusal to beyond R_keep so D118 re-fetch works;
thread the journal reader through `ensure_state`/`detached_txs`/serving and re-derive `tx_hashes`;
guard the genesis invariant; and the two-branch test is added. Implementation proceeds on this revised
design.

---

## Revision 2 (final design, implemented; supersedes §3.3, §4, R1 and R2 where they conflict)

Owner direction (2026-10-23): before fixing the final design, address that a metadata cap >= R_keep
alone does not cover multiple branches, and prove that eviction and re-fetch cannot prevent adopting
the branch D133 selects within the required depth. Everything below changes **local storage, retention
and sync only**: D133 (`better`/`key`), block validity, state roots, receipts and genesisHash are
untouched, and no retention limit becomes a validity rule.

### R2.0 Citation fix

"Adoption at depth <= R_keep" is **network.md:144** (`4. التبني بعمق ≤ R_keep.`), not :143 as earlier
documents had it. That citation has been corrected in every audit document, including the text above;
the evidence logs are left unchanged. The sync stage bound is **network.md:27** (`≤ 4096 لكل فرع و≤ 16384
إجمالًا`). The re-import rule is **consensus.md:205** (`الكتلة المحذوفة بـD118 ثم المستوردة ثانية تأخذ
رقمًا جديدًا`), and the re-fetch rule is **consensus.md:235** (`الفرع الذي يصير أثقل لاحقًا يُجلب
بالمزامنة عبر Status`).

### R2.1 Why a metadata cap alone is not enough

Any finite cap can be exceeded by competing side blocks (many peers, many short forks). The cap evicts
the **lowest cumulative work** leaf first, and a deep fork's early blocks have the lowest cumulative
work of all, so under pressure a deep competing branch is eaten first, exactly the branch a deep
reorg needs. D118 itself keeps only one non-canonical block per height and relies on re-fetch ("a
branch that later becomes heavier is fetched by sync via Status"). So the guarantee cannot come from
the cap. It comes from **re-fetch by a sync session whose blocks are protected from the cap**, plus a
re-fetch that the evicted-hash guard does not block.

### R2.2 Mechanism (as implemented)

- **Retention (`Chain::retain`):** (1) state pruned at height <= head-K_hist (unchanged); (2) a
  non-canonical block with `height + r_keep <= head height` is removed with its descendants (the R_keep
  boundary, unchanged); (3) at most `side_cap` (= `SIDE_BLOCKS_MAX` = R_keep = 2048, D118's
  one-per-height count over the window) **unprotected** non-canonical blocks: the lowest-work
  unprotected leaf is evicted first, repeatedly; protected blocks are exempt from (3) only;
  (4) every journaled non-canonical block is **offloaded** (body, receipts, tx hashes, encoded header
  dropped; read back from the journal by `body_off`); (5) at most `side_max` side states, unprotected
  and lowest-work first. **Canonical blocks are never evicted and never offloaded by `retain`.**
- **Status:** `Hello` carries the head's cumulative work (network.md:7 `totalWork`); a node
  re-announces its Status on every new head.
- **Sync session (`Node::sync_tick`, one at a time):** started when a peer announces work >= ours with
  a head we do not store: immediately on that peer's **first** Hello if no session runs, otherwise
  round-robin among such peers, at most one start per second. Later Hellos only update the peer's
  Status, and a claim set aside stays aside until the announced head changes. Round 1 sends `GetFrom{locator, 512}`. The locator (<= 32 hashes, network.md:8)
  holds our head and the 9 blocks below it, doubling steps, **the canonical block at depth R_keep**,
  and genesis. The responder serves its canonical blocks after the highest locator hash on its
  canonical chain. Later rounds put the last delivered block first in the locator. A delivered block counts as **progress**
  only if it is stored after its import and is either newly imported, or a stored copy at a height
  above the session's best height inside the R_keep window. A new round is asked only after a round
  with progress. The session ends when the peer's head is stored or our work exceeds its claim
  (success), after 3 s without progress, or when the protected branch reaches `protect_max`. The
  last two set the claim aside.
- **Solicited import (`Node::import_with`):** a block from the session peer bypasses the
  evicted-hash refusal (that refusal remains for unsolicited copies as a churn guard), and while it is
  **non-canonical** it is protected from the side cap. An orphan child imported when its parent
  arrives keeps the parent import's solicited status. **Any** new block at or below head − R_keep is
  refused before validation or journaling: the next `retain` would remove it, and it can be neither
  canonical nor the D133 maximum there, since its work is at most the canonical block's at that height.
  This is local retention (governance.md:200), never an invalid mark. At most `protect_max` = 2·R_keep = 4096
  protected blocks, the per-branch sync figure of network.md:27. Adopted blocks are canonical and need
  no protection, so a linear catch-up of any length is not bounded by it.
- **Restore (`Chain::restore_canonical`):** after an adoption, every adopted block that was offloaded
  while non-canonical is made whole again: encoded header and body from the journal, receipts and tx
  hashes by re-executing the adopted suffix in order from the fork point (a rolling state), each block
  checked against stateRoot, receiptsRoot, gasUsed and ParentResult. Order in `import`: insert,
  detached transactions (A11), `retain` (offloads the abandoned branch), restore. The two branches'
  bytes are never resident together.

### R2.3 Theorem (adoption is not prevented by eviction or re-fetch)

Let node N have head H. Let a connected peer P have canonical tip T with T ≻ H under D133. Let L be
the highest block on both N's and P's canonical chains, and let d = height(H) − height(L).
Preconditions:
(P1) d <= R_keep at every moment until adoption (the depth network.md:144 requires);
(P2) P keeps T, or a successor at least as heavy, as its canonical chain and answers each `GetFrom`
     within 3 s of quiet time;
(P3) the prefix of P's chain above L, up to its first block heavier than N's head at that time, has at
     most `protect_max` blocks;
(P4) N's clock admits P's timestamps (no `Future` rejection beyond the retry queue).
Claim: N's head becomes a block with work >= W(T), whose chain is P's canonical chain. If W(T) = W(H),
N's head becomes the D133 maximum of N's stored set including P's branch, with importSeq assigned as
consensus.md:205 prescribes.

Proof.
1. *Trigger.* T is not stored at N: a stored T with T ≻ H would be N's head, since the head is the
   D133 maximum of the stored set and `insert` keeps it so. P's Status announces W(T) >= W(H) with an
   unknown head, so P is a claimant. Any running session with another peer Q ends within bounded time.
   Q's progress is bounded by the blocks it can make N store: new valid blocks, or stored copies at
   increasing heights inside the window (at most R_keep plus the blocks above N's head). Without
   progress it ends 3 s later, or at the protect bound. N then asks claimants round-robin, so P is
   asked within (number of claimants) sessions. A claimant that only re-sends Hello cannot jump the
   queue: only a first Hello starts at once.
2. *Common point within the window.* The locator holds N's canonical block at height(H) − R_keep,
   which is <= height(L) by P1, so it lies on P's canonical chain. P answers from F + 1, where F, the
   highest locator hash on P's canonical chain, satisfies height(H) − R_keep <= height(F) <= height(L).
   The blocks F+1..L are N's canonical blocks (duplicates). L+1 onward are P's branch.
3. *Every delivered block's parent is present.* Each served block is the child of F or of the
   previous served block. Canonical blocks are never removed. A non-canonical session block is
   (a) protected from the cap, (b) not removed by the R_keep boundary, since its height is > height(L)
   >= height(H) − R_keep by P1, (c) not refused as evicted, since it is solicited. Its ancestors are
   canonical or earlier session blocks, so the same holds for them (removal by descent cannot apply).
   Validation is deterministic, so a block valid at P is valid at N (P4). A pruned parent state is
   rebuilt from the journal (`ensure_state_with`).
4. *Progress.* Each round delivers up to 512 blocks (fewer when P's send queue is half full); the
   next round continues after the last delivered block, which lies on P's canonical chain (P2). The
   prefix of P3 is protected throughout (<= `protect_max`), so it is not lost to any number of
   competing side blocks: only unprotected leaves are evicted.
5. *Adoption.* `insert` compares each new block with the head by `better`, which reads only metadata
   (W, parent, height, key). This is unchanged and valid on offloaded entries. At the first delivered
   block heavier than N's head, that block becomes the head. Later session blocks extend the canonical
   chain. The session ends once T is stored or N's work exceeds P's claim. At most
   ⌈(height(T) − height(F)) / 512⌉ + 1 rounds are needed when every response is full.
6. *Equal work.* At W(T) = W(H), D133 compares the children of the fork point by
   (signed, importSeq, hash). P's blocks, being new or re-imported at N, take new importSeq values
   (consensus.md:205; FINAL_DESIGN.md:6246 notes that this local preference may change "without
   affecting work"). Signed-before-fallback does not depend on importSeq. So the head after re-fetch
   is D133's maximum over the stored set with the spec-defined importSeq. Eviction never changes the
   head: only non-canonical blocks are evicted, and a re-imported block's key only gets worse
   (larger importSeq).
7. *Depth beyond R_keep.* If P1 fails (d > R_keep), P's blocks at or below head − R_keep are refused
   before validation by local retention. Their children above the line become orphans, nothing counts
   as progress, and the session ends after 3 s. Nothing is marked invalid, no blockReject is issued,
   nothing is journaled, and the node stays on its valid chain. Deeper rollback is the manual checkpoint
   of governance.md:200. Retention never becomes a validity rule.  ∎

Liveness note (not a safety property; corrected after the second independent review, R2.8): a peer
can announce a false work claim. Its session ends 3 s after its last progress, and junk, removed
blocks or copies below the frontier are not progress. Its claim is then set aside until its announced
head changes, and other claimants are asked in turn. The cost per claim is bounded by the progress it
can show, at most `protect_max` protected blocks, and 3 s.

### R2.4 Canonical residency and the RPC scope (matched to the approved text)

Invariant (checked by `Chain::canonical_resident` in the tests): after every import and after open,
**every canonical block holds its encoded header, body, receipts and tx hashes in memory**, exactly as
before this change. storage.md:70 retains "blocks, receipts, logs, code and indexes until the end";
logs are keyed by (height, blockHash) of the canonical chain (storage.md:61). So every canonical
RPC read keeps its current scope: `eth_getBlockByNumber`, `eth_getTransactionByHash`/`Receipt`
(canonical scan), `eth_getLogs`/`pocol_getLogs`, `pocol_getHeaders`, `pocol_getBlockRlp`, and the
GetRange/GetFrom serving. No RPC answer is restricted to a window, and no new error is introduced for
canonical history. A non-canonical block remains available exactly while retention keeps it, as
before: `eth_getBlockByHash` and `GetBlock` materialize an offloaded one from its journal record,
byte-equal. State reads keep their existing window (consensus.md §10: K_eff <= K_hist, otherwise
−32017). One pre-existing inconsistency was found and fixed during this check: `eth_call` and
`eth_estimateGas` on a block whose state is pruned simulated on an empty state. They now answer
−32017 like the state reads (test `a_call_on_a_pruned_state_is_32017_not_an_empty_state_simulation`).

### R2.5 Crash consistency and replay equivalence

Write ordering is unchanged: validate, append the record, `sync_data`, commit-file update (fsynced),
then the in-memory insert, retain and restore. Offload and restore only drop or recompute bytes that
are durably in the journal. Restore is derived state, re-executed and root-checked on every adoption
and at open. So a crash leaves the journal holding either the old chain or the whole adopted chain, and
replay rebuilds exactly that chain. Head, canonical chain, state, receipts and indexes are recomputed,
never read from a partial structure.

Replay is made equal to live without recording live's session decisions:
(1) Every record is replayed in order, with `retain` after each, as live did.
(2) A record of a block already present is a re-import. Live journals a block only when it is absent,
    so live must have removed it. Replay gives it a new importSeq (consensus.md:205), so every block's
    importSeq is the position of its last record, in live and in replay alike. Replay also takes that
    record's header copy and offset, as live did (the block hash does not commit the envelope bytes).
(3) A block whose next journal event is a record of one of its **children** is protected from the cap
    until then.
(4) During replay the cap bounds all non-canonical blocks by side_cap + protect_max.
(5) Replay uses the largest R_keep the data directory has used. `retention.rkeep` is written durably
    before replay, and the configured value applies afterwards. A smaller value could drop a parent live
    kept. A larger one only keeps blocks that were below the head when imported, and no later record
    builds on them (new blocks below the window are refused).

*Lemma.* After every record i, replay's head equals live's head. By induction: a protected block B
(next event a child record j, no own record before j) was present in live from i to j, since live had
it at j, validated the child on it, and could not have re-imported it without a record. So replay
never evicts a block a later record needs. Replay's R_keep removals coincide with live's for every
block both hold (same head heights and canonical sets by induction). Hence each record's parent is
present, its validation is identical, and `better` compares the same works and the same order of
importSeq values, so `insert` makes the same head decision. A re-imported block X was not canonical
(live did not hold it, and canonical sets coincide), and its importSeq only increases, so it cannot
become the head by the re-import itself. *Memory:* the replay-protected set is a subset of live's
non-canonical set at the same record (<= side_cap + protect_max), and (4) evicts unprotected blocks
past that sum.

Tests: `a_crash_around_an_adoption_reopens_on_one_whole_chain` covers (a) a record fsynced before the
commit update (reopens on the adopted chain) and (b) a record torn mid-write (reopens on the old chain,
and the block delivered again adopts). In both, the chain, receipts, transaction index and RPC answers
are compared. `replay_gives_a_reimported_block_a_new_import_seq` covers (2).
`a_session_branch_is_adopted_through_a_flood_of_competing_side_branches` covers (3) and asserts
`replay_skipped == 0`. Every adoption test also restarts and compares.

### R2.6 Resource bounds (proven / measured / assumed kept apart)

Proven by the code (A13 model, `resources::derive`, boot-gated with the 1/8 reserve):
| item | count bound | size per item | enforced by |
|---|---|---|---|
| canonical blocks, resident and whole | <= H_END+1 | per-block term; bodies, receipts and logs under TXBYTES_CAP / TX_CAP / LOG_CAP (chain-cumulative) | validity caps; never evicted (unchanged) |
| non-canonical metadata | live: <= side_cap unprotected + protect_max protected; replay: <= side_cap + protect_max | per-block term − one header − evidence + protect slot (22,947 B on the devnet profile) | `retain` (cap, R_keep boundary, `replaying`) |
| one full block in flight | 1 | full side-block term | consensus thread (import then retain, restore rolling) |
| evicted-hash FIFO | EVICTED_MAX = 1024 | 96 B | `retain` |
| side states / transient snapshots | side_max / 10 | snapshot term | unchanged |
| sync session | one; <= 512 blocks per round on the wire | frames within existing peer queues | `sync_tick`, `Peer::offer` |
Devnet defaults: side metadata (2048 + 4096) × 22,947 = 140,986,368 B, which replaces the former 320 full
side blocks. Model total 5,627,687,516 B; with the 1/8 reserve it needs 6,431,642,876 B of
6,442,450,944 B (10,808,068 B unused). Measured RSS is reported separately in A13-DERIVATION §2b.
Assumed, not bounded by this model: the open-time transient (`store::open` holds the whole journal,
plus two replay-index entries per record; condition `open-replay-transient`); CPU time of deep
rebuilds and restores (condition `cpu-and-timing`).
Disk: the journal is append-only. A re-fetch appends the block again, which the re-import rule above
accounts for. Compaction remains a documented follow-up (local, no validity effect).

### R2.7 Acceptance tests (final names) and controls

| frozen test | test | build |
|---|---|---|
| 1 depth 2048 adopted | `adoption_at_the_spec_depth_2048_and_not_at_2049` (defaults) | release, `--ignored` |
| 1/2 boundary, fast | `r_keep_boundary_adopts_at_depth_r_keep_and_keeps_the_valid_chain_beyond` (R_keep 24) | all |
| 2 depth 2049 not adopted, not invalid | both of the above | |
| 3 D133 tie-break at depth | `d133_tie_break_at_depth_400_holds_through_eviction_and_refetch` (release, `--ignored`); `d133_tie_break_at_depth_holds_through_eviction_and_refetch` (depth 40) | |
| 4 evicted branch re-fetched | `side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again`; `a_sync_session_refetches_an_evicted_branch_and_serves_it` (3 TCP nodes, RPC compared) | |
| multi-branch (owner) | `a_session_branch_is_adopted_through_a_flood_of_competing_side_branches` | |
| 5 crash consistency | `a_crash_around_an_adoption_reopens_on_one_whole_chain` | |
| 6 off-heap fidelity | `an_offloaded_block_is_served_viewed_and_re_executed_from_the_journal` | |
| 7 budget | `a13_bound_breakdown`, `a13_boot_refuses_a_bound_above_the_memory_budget` | |
| 8 positive controls | asserted inside each test (eviction happened, refetched > 0, restored > 0, flood exceeded the cap) | |
| sync liveness | `a_long_catch_up_is_not_bounded_by_the_protected_branch_size`, `range_sync_reaches_the_peer_head_through_truncated_responses` | |
| replay rule | `replay_gives_a_reimported_block_a_new_import_seq`, `replay_uses_the_largest_r_keep_the_data_directory_has_used` | |
| session liveness | `a_peer_with_a_false_status_cannot_hold_the_sync_session` (a peer with a false Status replays our own blocks), `a_session_block_arriving_before_its_parent_stays_protected` | |
| side-state window | `a_side_branch_below_the_k_hist_window_imports_without_rebuilding_each_parent` | |
Mutation controls (each must fail at least one test) are recorded in evidence/MUTATIONS-REDB.md.

### R2.8 Second independent review of the implementation (agent instance) and fixes

An independent agent instance reviewed `0cc59ee..9ce5a7c` against this revision. Each finding was
verified by reading the code end to end. It found no counterexample to the replay lemma (R2.5), and
no path that leaves a canonical block offloaded, changes D133 or validity, or breaks the write
ordering. The two HIGH findings were in the adoption proof (R2.3):
- **HIGH: a peer forking deeper than R_keep kept a session alive and grew the journal.** A delivered
  block that retention removed at once still counted as progress. Each copy was validated (rebuilding
  from genesis), journaled and removed. *Fixed:* new blocks at or below head − R_keep are refused before
  validation, and progress requires a stored block. Steps 1 and 7 above were corrected.
- **HIGH: a Duplicate reply kept any session alive.** A peer claiming huge work could answer with our
  own blocks forever, and a Hello started a session at once. *Fixed:* the progress rule (best height
  inside the window), stall timed from the last progress, and only the first Hello starts at once. The
  liveness note above was corrected. Regression test
  `a_peer_with_a_false_status_cannot_hold_the_sync_session`; with the old progress rule it fails
  (A never adopts).
- LOW, all fixed: orphaned session blocks lost their solicited status; replay kept the first record's
  copy of a re-imported block; R_keep was not persisted (now `retention.rkeep`).
- LOW, documented, not changed: a restore needs the fork point's state. When that state is pruned
  below K_hist it is rebuilt from genesis, so the CPU cost scales with the chain height (condition
  `cpu-and-timing`). The spec's reverse diffs (consensus.md §10) would bound it by the depth; pocold
  keeps no diffs.
Earlier, the spec-depth run had exposed quadratic rebuilds of deep side branches. That was fixed in
`9ce5a7c` (the K_hist prune covers canonical states only).

## Implementation status

Steps 1–2 (`66d69c6`, `f7c345b`) added the journal read primitive and the off-heap plumbing. Step 3
(this revision) is implemented: retention with protection and offload, restore, sync sessions over
Status work and locators, solicited re-fetch, journal-backed serving, the replay rules, and the
re-derived A13 side term. The earlier step-3 plan to offload canonical blocks outside the K_hist window
and floor RPC history at that window is **withdrawn**. It would have narrowed RPC scope against
storage.md:70, so canonical blocks stay whole (R2.4). Verification results on the final commit are in
evidence/ and REPAIR-REPORT.md. PR #27 stays Draft.
