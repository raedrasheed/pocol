# FN-3 (C6) implementation plan, final: replace `Entry.ranges` with member-set records

Based on main 984ea50, read only (nothing edited, built or committed). Line numbers are re-checked against 984ea50; FN-1 and Phase 4 will move them. Before each step, re-run
`grep -n '\.ranges\b\|ranges_of\|LateBase{\|RANGES\|entry_of(' prototypes/pocold/src/*.rs prototypes/pocold/tests/*.rs`.

Two critiques (correctness lens; numbers/consumers/lock lens) were checked against the code. Every accepted point is folded in below, marked [C1-Dn] or [C2-n]. Rejected or partly rejected points are in Appendix A, with reasons.

**Ordering.** FN-3 lands after FN-1 (one-pass `membership()`, its caches, binary-search `Assignment::owner()`, `pocol_getCaps`) and after Phase 4 (v3-preview feature removed, spec 2 default). It lands before Phase 5c, so the launch evidence is measured on the final format.

**Owner rule.** Verdicts, collaborative consensus, the template block and the principles do not change. Everything here is node-local storage and caching. Every verdict still comes from the same `Assignment` code on the same inputs. Check order and error texts in type-2 and kind-3 stay the same.

---

## 0. What the code fixes, and the checks to run first

### Facts verified in the code
- **Only three code paths read `Entry.ranges`, and each needs only nonce→owner:**
  - type-2 at chain.rs:981;
  - kind-3 `late_check` at :1676, reached through `LateBase` at :892;
  - node-local `late_offer` at :911.
  - Live checks use `ctx.assignment`. No RPC handler reads ranges.
  - The Entry literals are at chain.rs:285, :499 and :812, and checkpoint.rs:364. No test builds one.
- **`Entry.ranges` of block b is a pure function of these inputs:**
  - S_b = `Membership(state(parent(b)), h_b).ids()`, sorted;
  - the parent's `seal` (as `members.seal`);
  - `b.height`;
  - `b.header.a`;
  - `b.target` (the ASERT target from the parent, chain.rs:722);
  - `effective_nonce_mode(b.height)` and CP `c`.
  - It is computed in lp1-node assignment.rs:176-198. `accruing` plays no part.
  - The seal, `header.a` and `target` all survive `slim()` (chain.rs:247-248).
- **The Δ bound depends on the CP [C2-7].**
  - |S_b Δ S_parent| ≤ REG_OPS + R_react + E_max, checked in PoColSystemV3.sol:
    - register, deregister, removeInactive and claimBond share REG_OPS, and `registerReplacing` costs 2 units for its 2 changes;
    - reactivate is capped by R_react;
    - punish's removeActive is capped by E_max;
    - the sweep (`erase`) and markClose do not touch S_b.
  - On the fresh CP this is 16 + 4 + 4 = 24. With the genesis_spec defaults (REG_OPS 16, R_react 8, E_max 8) it is 32. ParamGate has no upper bound on the sum.
- **A13 has no term today for `Membership`, `Assignment` or `ChildCtx`.** The only M-scaled memory terms are `mul(64,m)` in `per_block` (resources.rs:557) and `52*m` in the history payload (:590-591).
- **The consensus boundary [C2-5].** consensus-boundary.txt:38 lists `pocold/src/{chain,types,statediff,state_store,checkpoint,history,store,secp_backend,lib,identity}.rs`, and :33 lists lp1-node `assignment.rs`.
  - So chain.rs, history.rs, checkpoint.rs and assignment.rs are all inside the boundary.
  - assignment.rs is also compiled by lp4 through `#[path]`, so the refactor must build in both crates.

### Run these before writing code
1. **Baseline A13 at 10k.** Use the fresh-network CP built by hand from the design §1a table: M_max 10,000, E_win 360, D 4, E_max 4, R_react 4, REG_RECORDS_MAX 24,576, **SYS_SLOT_MAX 147,456** (A13 reads SYS_SLOT_MAX at resources.rs:472, not REG_RECORDS_MAX [C2-11]). Use FN-4's committed profile instead if it has landed.
   - Settings: store mode, eviction on, defaults, and `Limits.disk` set. Without `Limits.disk` a windowed continuous chain makes `derive` return None ("A13: disk sizes not observed", resources.rs:436-440).
   - The computed figure is 640,000 B × 11,221 entry-equivalents = 7,181,440,000 B = 6.69 GiB, against 5.25 GiB usable (7/8 × 6 GiB, resources.rs:925-935). Main should therefore refuse to open (node.rs:950). Record the real `derive` output.
2. **Design risk 1 (FRESH-NETWORK-DESIGN.md:161) understates the problem.** Its 2.8 GiB counts canonical slim depth only. The A13 worst case is about 6.69 GiB:
   - window 32;
   - offloaded 4,681;
   - shadow max(257, E_win+1) = 361;
   - side_cap + 2·R_keep = 6,144;
   - side_full 1;
   - views 2.
   Correct the text in FN-3b's docs (see §7).

---

## 1. Data structures

### 1.1 `MemberRec`, one per Entry (new `prototypes/pocold/src/members.rs`, added to the `+` list [C2-5])

```rust
pub const SNAPSHOT_EVERY: u64 = 256;  // a snapshot at h % 256 == 1 (h = 1, 257, 513, ...)
pub const DELTA_CAP: usize = 32;      // a larger change is written as a snapshot (rule below)
pub const WALK_MAX: u64 = 256;        // hard bound on a delta walk (cadence guarantees <= 255)

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRec {
    pub n: u16,            // |S_b| (<= M_max <= u16::MAX, as Membership::from_parent checks)
    pub digest: [u8; 32],  // sum of keccak256(id) mod 2^256 over S_b: order-free, O(|Δ|) to update
    pub step: MemberStep,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberStep {
    /// S_b = S_parent + adds - rems. Both sorted ascending and disjoint; adds ∩ S_parent = ∅; rems ⊆ S_parent.
    Delta { adds: Box<[Address]>, rems: Box<[Address]> },
    /// S_b is stored whole (history.redb `member_sets`, or the bare chain's MemSets).
    Snapshot,
    /// Placeholder for an RPC view built by `indexed_view`. It never enters a Chain; any reader rejects it as Local.
    Unread,
}
```

**The id cache is not part of `MemberRec` [C1-D3, C2-2].**
- `Entry` gets a separate field `member_ids: Option<Arc<[Address]>>`.
  - It is set only by `h_full`, on snapshot blocks.
  - `offload()` drops it.
  - It is never loaded by `entry_of`.
- It is excluded from the fingerprint and `PartialEq` comparisons. consensus.rs:3776 compares `&e.members`, not the ids.
- Result: `restore_canonical` (chain.rs:1391) and `view` (node.rs:3154) can set `offloaded=false` on an entry without ids, and live, replayed and restored entries still compare equal.
- The node does not need to keep `ctx` until `history.put`, because the fresh entry carries its ids.
  - Replay calls `put_with` (node.rs:1175) before `chain.insert`.
  - Live import calls `put` (:1539) before `journal.append`, and that comes before `body_off` and any offload.

**Snapshot rule (enforced at write and at read [C1-D6]).**
- Block h is a snapshot iff `h % 256 == 1` or `|adds| + |rems| > DELTA_CAP`.
- The rule depends only on (S_parent, S_b, h), so live, replayed and restored records are identical.
- Genesis gets `MemberRec{n:0, digest:0, step:Snapshot}` with an empty set. Nothing reads it: the type-2 walk stops at height 0 and kind-3 has j ≥ 1.
- With `delta_max(cp) = REG_OPS + R_react + E_max ≤ DELTA_CAP` (fresh CP 24, defaults 32), snapshots follow the cadence only.
- A larger CP, or a spec-1 Bounded W_act flip or `active.is_empty()` fallback, writes off-cadence snapshots. That is correct, with more disk. A13 counts it (§4).

**Digest.** An integrity check of local data only; it never enters a verdict.
- `h_full` updates it in O(|Δ|): the parent's digest plus Σadds minus Σrems.
- A snapshot block computes it once over all M ids, about 10k keccaks per 256 blocks.

**In-memory cost per Entry.**
- `MemberRec` plus the `member_ids` Option is about 80 B of fixed size, against 24 B for the old `Vec`. `size_of::<Entry>()` picks this up through `resources::entry_bytes`.
- Delta heap: at most 20·min(DELTA_CAP, delta_max) + 2·32 B, so ≤ 704 B; typically 0.
- Snapshot ids: 20·M = 200 KB, only on non-offloaded entries freshly built by `h_full`.

### 1.2 Chain-level source and caches (chain.rs, `Chain` struct near :384)

**`member_src: Arc<dyn MemberSets + Send + Sync>`** defaults to an in-memory `MemSets` [C1-D2].
- **The bare chain.** `normalize` (:281, reached from `evict_below` :1145 and `restore_low` :1130) and `shadow_push` (:1468) offload without a `body_off` check. tests/memory_window.rs:43 evicts on a bare chain. So "bare chains never offload" was false.
  - `MemSets` keeps every record and snapshot set that `Chain::insert` gives it, keyed by hash.
  - `evict_below` prunes `MemSets` to heights ≥ floor − WALK_MAX, which keeps the heap flat for `long_chain_heap_is_flat`.
- **The node.** It calls `chain.set_member_source(history.clone())` right after `History::open` at node.rs:986, before checkpoint restore (:1063, which mutates the same chain) and before replay [C1-D10].
  - With a History source, `insert` does not record into it: the node's `put` does.

**The `MemberSets` trait** (in members.rs):

```rust
trait MemberSets {
    fn note(&self, e: &Entry) {}                       // MemSets stores; History is a no-op
    fn prune_below(&self, h: u64) {}
    fn record(&self, hash: &[u8;32]) -> Result<Option<(/*parent*/[u8;32], /*h*/u64, MemberRec)>, Reject>;
    fn set(&self, hash: &[u8;32]) -> Result<Option<Arc<[Address]>>, Reject>;
    fn damaged(&self, why: &str);                      // History: mark_damaged, so the next open rebuilds (I1)
}
```

**Caches.** Each is a hand-rolled `SmallLru` (a `VecDeque` with move-to-front); no `lru` crate (§6). The `Mutex` is never held across a call that can re-enter `member_set` or `assignment_of`, since std `Mutex` deadlocks on re-entry [C1-D8]. Capacities come from the CP at `Chain::new` [C1-D8]:

| Cache | Key | Value | Capacity | At 10k |
|---|---|---|---|---|
| `sets` | block hash | `Arc<[Address]>` | `SETS_CAP = max(16, 2·E_max + D + 2)` | 16 |
| `assigns` | (parent, attempt) | `Arc<Assignment>` | `ASSIGN_CAP = max(8, E_max + D + 1)` | 9 |
| `owners` | (block hash, nonce) | `Option<Address>` | `OWNERS_CAP = 2·EVIDENCE_POOL_MAX·(S_max+1)`, capped at 4,096 | ~64 B each |

- `sets` is keyed by block hash (S_b), never by height. Devnet uses D 16 and E_max 8, which gives 34.
- `assigns` uses FN-1's (parent, attempt) key. Look up FN-1's cache first, read-only; reconstructions go **only** into `assigns`.
- `owners` is a memo for the proposer's `includable_evidence`, which re-evaluates up to EVIDENCE_POOL_MAX = 64 items, twice each, per template (node.rs:3532-3540) [C2-10].
- Every key commits to S_b through the block hash, so an entry can never be stale, even across reorgs or for a block that later fails.

**Counters** (`AtomicU64`), shown in node status:
- set hits and misses, disk set reads, record reads;
- assignment hits and misses, owner-memo hits;
- reconstruct µs (max and a p99 sample);
- snapshots written, max Δ;
- oracle checks and oracle mismatches.

### 1.3 Readers

**`member_set(&self, e:&Entry) -> Result<Arc<[Address]>, Reject>`**
1. Check `sets[e.hash]`, then `e.member_ids`.
2. Walk e → parent → … through `self.get` (resident and shadow entries), collecting `Delta` steps, until an anchor is found. The anchor search order is:
   1. an entry whose set is cached;
   2. an entry with `member_ids`;
   3. a `Snapshot` record whose set is read through `member_src.set(hash)`.
3. If the walk leaves resident entries, continue through `member_src.record(hash)`, which carries the parent hash.
   - A rebuild near the floor can do this. The store-mode anchor reach `keep` = max(257, E_win+1) (resources.rs:858-860) puts the oldest evidence ancestor just above the floor, and the walk can go up to 255 below it [C1-D7].
4. Structural checks at each step [C1-D6]:
   - `record.h == child.h − 1`, and the parent hash links;
   - a record at `h % 256 == 1` must be a `Snapshot`;
   - the step count is ≤ WALK_MAX.
   - Any violation is `Reject::Local` plus `member_src.damaged(..)`.
5. Apply the deltas forward, oldest first, through an overlay `BTreeMap<Address,bool>`.
   - At each step check `rems ⊆ current` and `adds ∩ current = ∅`. "current" means the overlay first, then a binary search in S_anchor.
   - Then do one merge with S_anchor. This is exact whatever the order of adds and removes. Cost: O(Σ|Δ|·log + M).
6. Check `n` and `digest` against `e.members`.
   - Any inconsistency, missing record or missing set, and an `Unread` step, is `Reject::Local`, never a block verdict.
   - When the data came from history, also call `member_src.damaged(..)`.
7. Insert into `sets` under `e.hash`.

**`assignment_of(&self, e:&Entry) -> Result<Arc<Assignment>, Reject>`**
- Key = (e.parent, e.header.a). Check FN-1's cache, then `assigns`.
- On a miss:
  ```rust
  Assignment::from_ids(&member_set(e)?, &parent.seal, e.height, e.header.a, &e.target,
                       spec.effective_nonce_mode(e.height), cp.c)
  ```
  - The parent comes from `self.get(e.parent)`. Both the parent and e are inside E_win + 1 above the floor (keep term), and inside the shadow max(257, E_win+1) in `disk_walk`.
  - A missing parent is `Reject::Local` with a debug assert.
- **Every** failure here is `Reject::Local`, including `AssignmentError` (EmptyMembership, TicketCollision, nonce_max) [C1-D4]. A stored block was valid, so these are unreachable. Note that `child_ctx` maps the same errors to `Reject::Block` at :604; that is right for a new block and wrong for a reconstruction.
- Insert the result into `assigns`.
- Note for FN-5: if `cp_at(h)` ever touches `c`, use `cp_at(e.height)` [C2-12].

**Laziness keeps the check order [C1-D4].** `NonceOwners` (§2, `LateBase`) is lazy:

```rust
pub trait NonceOwners { fn owner(&self, n: u64) -> Result<Option<Address>, Reject>; }
```

- Implemented for `[(Address,u128,u128)]`: the linear find the test vectors use, same semantics, never Err.
- Implemented for `LazyOwners<'a>{chain, e, cell: OnceCell<Arc<Assignment>>}`, which consults `owners` first and resolves `assignment_of(e)` on the first lookup that misses.
- So `late_check`'s cap check (:1673) still runs before any reconstruction, as it runs before the ranges lookup today.
- Type-2 calls the lookup at the same point as today's `e.ranges` find (:981).

**lp1-node assignment.rs:176-198, a pure refactor.**
- `Assignment::new(members, attempt, target, mode, c)` delegates to a new `pub fn from_ids(ids:&[Address], seal:&H256, height:u64, attempt:u32, target:&U256, mode, c)`, passing `(&members.ids, &members.seal, members.height, …)`.
- One code path computes ranges for live and historical use.
- It must build in pocold and in lp4 (`#[path]`).

**Type safety.** A reconstructed set is an `Arc<[Address]>`, never a `Membership`. It cannot reach DrawWeights or FN-1's membership cache, which carries `accruing`. Type-1 at the tip still uses the live `membership()` (chain.rs:958).

### 1.4 Computing the record (in `h_full`)
- S_b = `ctx.membership.ids()`.
- If `h % 256 == 1`, write a snapshot: copy the ids into `member_ids` and compute the full digest.
- Otherwise get S_parent with `member_set(p)`. In steady state that is a `sets` hit, since the previous `h_full` inserted S_p; FN-1's membership-cache entry for p (ids only) also works.
  - Compute Δ by a sorted merge-diff, O(M) ≈ 10k compares.
  - If |Δ| > DELTA_CAP, write a snapshot instead.
- Insert S_b into `sets`.
- `Chain::insert` calls `member_src.note(&entry)` (MemSets only).

---

## 2. Changes by file (984ea50 line numbers)

### lp1-node/src/assignment.rs
| Line | Change |
|---|---|
| :176-198 | `new` delegates to `from_ids`. Behaviour unchanged. |
| :208-215 | `owner()` becomes a binary search. This is FN-1's change; FN-3 only depends on it. |

### pocold/src/members.rs (new; `+` list)
- `MemberRec`, `MemberStep`, `digest_add/sub` (U256 wrapping add of keccak256(id)), `diff(a,b) -> (adds, rems)`, `Overlay::apply`.
- `is_snapshot(h, delta_len)`, `delta_max(cp)`.
- Record encode/decode (§3), `SmallLru`, the `MemberSets` trait, `MemSets`, `NonceOwners`, `LazyOwners`.

### pocold/consensus-boundary.txt
- :38: add `members` to the `{…}` group [C2-5]. This file is itself locked, so the change goes in FN-3a's neutral line.

### pocold/src/chain.rs
| Line | Change |
|---|---|
| :124-131 | 3a: add `pub members: MemberRec` and `pub member_ids: Option<Arc<[Address]>>`. 3b: remove `ranges`. Update the doc at :124-127. |
| :235-245 | `BlockBytes::ranges_of`: remove (3b). Update the `canonical_entry` doc: slim=false no longer reads ranges. |
| :247-248 `slim()` | Stop clearing ranges (3b). `members` is kept. |
| :269 `offload()` | Also `e.member_ids = None`. This covers `normalize` (:281), `shadow_push` (:1468) and the cursors (:1072, :1089). |
| :284-288 `meta_clone` | Clone `members` and `member_ids` (cheap Arc). Drop the `ranges` clone (3b). |
| ~:384 `Chain` | Fields from §1.2. Add `set_member_source`, the test hooks `clear_member_caches()` and `set_member_oracle(bool)`, and `member_counters()`. |
| :499 genesis | `members: MemberRec::genesis()`. Drop `ranges: vec![]` (3b). |
| near :605 | New: `member_set`, `assignment_of`, `member_rec_for(p, h, ids) -> (MemberRec, Option<Arc<[Address]>>)`. |
| :785-818 `h_full` | Compute `members` and `member_ids`, and insert S_b into `sets`. 3b: delete the :816 `ranges:` construction (about 640 KB per block at M=10k). Oracle: §5.1. |
| :873-895 `evidence_late` | 3a: verdict still from `&e.ranges`; shadow check (§5.1). 3b: `LateBase{owners:&LazyOwners::new(self,e), …}`. Keep the slim check at :886, since TemplateID and header shares need an unslimmed entry. |
| :899-921 `late_offer` | As above. A Local error becomes `None`, which is node-local. |
| :964-988 `evidence_type2` | 3b: :981 becomes `self.owner_of(e, nonce)?.ok_or_else(||Reject::Block("ev2 nonce outside nMax".into()))?`. Keep the slim check at :977; error text unchanged. 3a: shadow check. |
| `Chain::insert` | `member_src.note(&entry)`. |
| :1137-1150 `evict_below` | `member_src.prune_below(floor.saturating_sub(WALK_MAX))`. |
| :1100-1106 doc | "No verdict reads an entry below the floor" becomes: verdicts read none; FN-3's member walk may read records below the floor, through `member_src` [C1-D7]. Update `evict_depth_of`'s doc too (resources.rs:855). |
| reexec (~:1336-1347) | Check `e.members.n == ctx.membership.ids().len()`. When the oracle is on, or on every snapshot height, also check the full digest. On a mismatch: `member_src.damaged(..)` plus `Reject::Local`; the next open rebuilds, so a restored bad record cannot cause a boot loop [C2-12]. Optional: insert `ctx.membership` ids into `sets` to warm rebuilds. |
| :1354-1394 `restore_canonical` | Drop `x.ranges=er.ranges.clone()` (3b). `offloaded=false` at :1391 is harmless now: ids are only a cache. |
| :1411-1446 | `restored_fields` returns `(header, result, late)`. Drop `ranges_of`. `restored` and `unslim` drop the ranges assignment (3b). |
| :1549-1562 `disk_walk` | Unchanged in shape. `canonical_entry(h,false)` reads a 112 B members record instead of a 520 KB ranges record. |
| :1627-1629 `LateBase` | `ranges:&'a [(Address,u128,u128)]` becomes `owners:&'a dyn NonceOwners` (3b; in 3a both fields, verdict from `ranges`). |
| :1676 | `b.owners.owner(*n)?.ok_or_else(||ls("nonce outside nMax"))?`. Order and texts unchanged. |
| :1688-1690 `assignment_range` | Live ctx only; no change. |

### pocold/src/history.rs
| Line | Change |
|---|---|
| :1-12 | Doc: S4's ranges table becomes FN-3's member records. This is a boundary file, so the edit goes in 3a/3b, not 3c [C2-5]. |
| :25-29 | Add `MEMBERS: hash → seal ‖ rec` (every block) and `MEMBER_SETS: hash → seal ‖ 20·n ids` (snapshot blocks only). 3b: remove `RANGES` and `Ranges`. |
| :54-75 `open` | See the migration rule in §3. There is **no format marker and no set-aside** [C1-D1, C2-3]. The table opens go inside the existing `create` closure, after `io_point("history-open-tables")`, through `rerr` [C2-4]. 3b adds `w.delete_table(RANGES)` in that same write transaction. |
| :94-98 `has` | 3a: `receipts && ranges && members`, since `restored_fields` and unslim still read ranges in 3a [C2-3]. 3b: `receipts && members`. The commit is atomic, so a snapshot's `member_sets` row exists whenever its `members` row does. |
| :103-121 `put`/`put_with` | Signature `(block, receipts, tx_hashes, members:&MemberRec, set:Option<&[Address]>, durable)`, with 3a also taking `ranges`. Return Err if the step is a snapshot without a set, or is `Unread`. Write MEMBERS (and MEMBER_SETS) in the **same commit** as RECEIPTS and TXIDX, so I3 is unchanged. |
| :141-146 | Replace `ranges` (3b) with `members(hash) -> Option<(parent,h,MemberRec)>` and `member_set(hash) -> Option<Arc<[Address]>>`. Each checks its seal and shape: ids strictly ascending; length, n and digest agree; the snapshot rule against h. A failure calls `mark_damaged`. |
| — | `impl chain::MemberSets for History`. A read error becomes `Reject::Local`; `damaged` calls `mark_damaged`. |

### pocold/src/checkpoint.rs
| Line | Change |
|---|---|
| :205-222 `Source` | Add `members(hash) -> Result<Option<MemberRec>,String>`. 3b: remove `ranges` (:215). The meaning of `has` (:216) follows history's `has`. |
| :270 | Unchanged (`src.has`). |
| :281 | Pass `Some(||src.members(&r.hash))`. |
| :341-372 `entry_of` | New parameter `members: Option<impl FnOnce()->Result<Option<MemberRec>,String>>`. `Some` is read always, not gated by `slim`: a missing record is `why("member record not in history.redb")`. `None` sets `MemberRec::unread()`. `member_ids` is always None (ids are never loaded). Set `members` in the literal at :364-367. Update the doc at :341-349. 3b: drop the `ranges` parameter. |
| :378-399 `JournalSource` | Implement `members` over `history`. |
| Snapshot, Rec, MAGIC, Params, REC_MIN/REC_MAX | Unchanged. An old checkpoint against a history without member rows fails `src.has` and falls back to replay, as today. |

### pocold/src/node.rs
| Line | Change |
|---|---|
| :28-47 `Src` | `canonical_entry` passes `Some(members closure)`. 3b: drop `ranges_of` and the history ranges read (:40-43). This is one small history read per reloaded entry (up to R_keep = 2048 at a head-drop reload, plus each `disk_walk` height) [C2-9]. |
| :3122-3134 `indexed_view` | Pass `None` for members, so views never read member rows or ids [C2-1]. 3b: drop the ranges closure. |
| :986 and after | `history` becomes `Arc<History>`. Call `chain.set_member_source(history.clone())` right after :986. Check every `&history` use (:1165, :1175, :1203, `Src`). |
| :1175 replay, :1539 live | `put_with(&entry.hash,&entry.receipts,&entry.tx_hashes,&entry.members,entry.member_ids.as_deref(),false)` (3a also `&entry.ranges`). The ctx is not kept. |
| :3532-3540 `includable_evidence` | Today `Err(_)=>{}` drops the item. After this change, `Err(Reject::Local(_))` keeps the item and logs it (rate-limited); only other errors drop it [C1-D5, C2-10]. Node-local; the pool stays ≤ EVIDENCE_POOL_MAX. |
| :2439, :2975, :3141-3166 | No code change. Each `meta_clone`/`full_entry` copy drops from about 640 KB to about 1 KB at M=10k (3b). |
| status (:3259-3315) | Add a `members` block with §1.2's counters. |

### pocold/src/wallet.rs (optional, node-local)
- :99 and :332: replace `m.ids().contains(..)` with `binary_search(..).is_ok()`.

### pocold/src/resources.rs
- See §4.

---

## 3. Persistence format and migration

**`members` record.**
- Key: block hash (32 B).
- Value: `keccak(hash ‖ rec) ‖ rec`, all big-endian.
- Fields of `rec`:

| Field | Size |
|---|---|
| ver | u8 = 1 |
| kind | u8 (0 delta, 1 snapshot) |
| parent | 32 B |
| h | u64 |
| n | u16 |
| digest | 32 B |
| n_add | u16 |
| n_rem | u16 |
| adds, then rems | 20 B each, both ascending |

- The fixed part is 80 B, so the value is 112 B + 20·|Δ|.
- There is no seed, nonce_max or own seal. These are derived from the parent's `seal`, `header.a` and `target`. The parent is resident wherever an assignment is built (Appendix A, C1-D7).

**`member_sets` row.**
- Key: hash.
- Value: `keccak(hash ‖ ids) ‖ ids` (20·n B, ascending).

**Migration: no marker; reuse the refill machinery** [C1-D1, C2-3, C2-4].
- **Pre-FN-3 file (devnet, test or e2e data dir).**
  - `has()` is false for every old block, because it has no member row.
  - A checkpoint restore fails `src.has` and falls back to replay. Replay's `put_with` refills the missing rows.
  - This is the S4 path described at history.rs:94-98. The file is not set aside, so receipts are kept and no Windows rename of an open redb file is involved.
- **3a → 3b.**
  - 3b's open deletes the RANGES table in the open transaction. All member rows already exist, so there is no rebuild.
  - redb does not shrink the file; acceptable for devnet and test dirs.
- **3b → 3a downgrade.**
  - 3a's `has` needs ranges, so its restore falls back to replay, which refills.
  - Not a supported path; noted only.
- **Errors.**
  - An I/O error in these opens keeps the file and fails the open (F-A), through `rerr`.
  - Add an io-fault test following the `#[cfg(feature="io-fault")]` pattern at consensus.rs:3933.
- The journal and canon.redb are unchanged.

---

## 4. A13 (resources.rs) at M = 10,000

Settings: fresh-network CP, store mode, eviction on, `Limits.disk` observed, K_hist 32, R_keep 2048, slim_depth 4,713, side 6,144, shadow 361. DISK_HEADROOM is 2,304 records, so `blocks` = 2,305 [C2-minor].

**Removed (3b)**
- `per_block` `mul(64,m)` (:557). This propagates to the offloaded, window, views, shadow, side_block and side_entry terms.
  - 640,000 B × 11,221 = **7,181,440,000 B ≈ 6.69 GiB**.
- History payload `52*m` (:590-591).
  - About 2.3 MB of allocator over 2,305 records × factor 8.

**Added (3a; new hist items plus CONDITIONS)**

Let `dm = min(DELTA_CAP, delta_max(cp))` [C2-7].

| Term | Formula | At M=10k (dm 24) |
|---|---|---|
| Delta heap on every resident entry, slim and reload included | (2·32 + 20·dm) × (11,221 + 2,050 + 2,048) | 544 × 15,319 = 8.3 MB |
| Snapshot ids on non-offloaded entries (window + side_full) | 33 × (20m + 32) | 6.6 MB |
| `sets` | SETS_CAP × (20m + 64) | 16 × 200,064 = 3.2 MB |
| `assigns` | ASSIGN_CAP × (96m + 64 + size_of::<Assignment>()) | 9 × ~960 KB = 8.6 MB |
| `owners` | OWNERS_CAP × 64 | ≤ 0.26 MB |
| Reconstruction scratch (one at a time, consensus thread) | S_anchor 20m + merge 20m + tickets 52m + `tickets.sort()` scratch 52m + NonceRange 96m + disk value 20m + 4,096 [C2-12] | 260m ≈ 2.6 MB |
| History payload | blocks·(32 + 112 + 20·dm) + snaps·(32 + 32 + 20m) | allocator ≈ 0.1 MB |
| **Total added** | | **≈ 29.7 MB** |

- `snaps = ⌈blocks/256⌉ + 1` only on a profile that is v3 and Unbounded at every height **and** has `delta_max(cp) ≤ DELTA_CAP`. Otherwise `snaps = blocks`.
- **Net change: about −7.15 GB.**

**ChildCtx gap (coordinate with FN-1; count it once).**
- A13 has no `ChildCtx` term today. Each context holds `Membership` (ids 20m + accruing ≤ 20m) and `Assignment` (96m).
- Construction peaks at about 250m + 52m of sort scratch.
- Held contexts: `Locked{ctx}` (node.rs:359), `template_ctx`, the one in `validate`, and FN-1's cache entries.
- If FN-1 did not add `ctx_bytes(m) × CTX_HELD`, FN-3a adds it.

**CONDITIONS (:345-364): add `member-records-inventory`.** It states:
- the caps SETS_CAP, ASSIGN_CAP and OWNERS_CAP, and how they derive from the CP;
- that `offload` drops `member_ids` (memory_bounds test);
- that |Δ| ≤ DELTA_CAP is enforced by the snapshot rule;
- `delta_max(cp) = REG_OPS + R_react + E_max`, with the Solidity pointers (`checkBudgets`, `replacing`, `punish`);
- that reconstruction runs one at a time on the consensus thread;
- that a bare chain's `MemSets` is pruned to floor − 256.

**Pinned A13 tests that move [C2-6].** Each is re-pinned in its PR with the recorded delta, and pins stay gated by `cfg(all(target_os="linux",target_arch="x86_64"))`.
- **consensus.rs:1974 `a13_bound_breakdown`.** It moves in 3a (larger `size_of::<Entry>`, the new terms) and in 3b (−64·m, −52·m).
- **network_budget.rs:289-346 `a13_live_chain_halt_records`.** It pins `got=[1_145_894,…]`.
  - 3b removes 4,096 B per entry at the devnet m_max 64 with eviction off, so the halt likely rises past `R10[0]=1,169,713`.
  - The subtractions `R10[0]-got[2]` and `R10[1]-got[5]` are u64 and would underflow. Make them signed (`i128`).
  - Restate the owner's seed rule: a rise only widens the margin, so the rule's outcome cannot get worse. Record it in the PR.
- **network_budget.rs:350+ `a13_live_chain_halt_disk_bytes`.** Re-pin it.

**history.redb growth per day** at T_blk = 10 s (8,640 blocks/day); the C6 target is ≪ 100 MB/day:

| Part | Today | After FN-3 |
|---|---|---|
| Ranges or members per block | 8,640 × 520,064 ≈ **4.49 GB** | 8,640 × 144 = 1.24 MB idle; 5.39 MB at |Δ| = 24 every block |
| Snapshots | — | 33.75 × 200,064 = 6.75 MB raw; ≈ 8.85 MB with redb's 64-page rounding |
| **Total (M-dependent)** | ≈ 4.49 GB | **≈ 8–12 MB raw, ≈ 14–31 MB on file** |

- Two years: 3.28 TB today, about 10–23 GB after.
- `check_integrity` at open: about +0.05 s per day of chain, against 4–6 s per day today.
- Receipts and txidx are not M-dependent: about 372 B per tx, so about 0.6 GB/day at 190 tx per block. M1 and D1 report the two parts separately, at a stated tx load.

---

## 5. Test plan

The oracle is a runtime flag, `Chain::member_oracle`, defaulting to `cfg!(debug_assertions)`. It is not a cargo feature (a feature can change the lock's dependency digest) and not `cfg(test)` (integration tests cannot see it).

1. **Oracle (identical verdicts on every v3 test).**
   - **(a) Import time, in `h_full`.** The cold-path `assignment_of(entry)` (caches bypassed) must equal `ctx.assignment.ranges()`, and in 3a also `entry.ranges`. owner(winner) and owner(each share) must also agree.
   - **(b) Shadow at the readers, 3a only [C2-8].** At chain.rs:892, :911 and :981 the verdict comes from `e.ranges`. When the oracle is on, the reconstruction is compared for the nonces looked up and for the whole `ranges()`. A mismatch bumps `oracle_mismatch`, logs, and debug-asserts. This covers restored, replayed, disk-walk and reorg paths at the moment they are read.
   - **(c) End-of-test cold sweep [C1-D9].** At the end of every v3 test, and after each restart and reorg step:
     - call `clear_member_caches()`;
     - compare `assignment_of(e).ranges()` for every held non-genesis entry with what was captured at import: `entry.ranges` in 3a; in 3b, a test-side map `hash → keccak(ranges)` filled from an `h_full` hook.
   - A positive control in every v3 suite asserts `oracle_checks > 0` and `oracle_mismatch == 0`. The suites: v3_membership, v3_late_shares, v3_earnin, v3_reward_gateway, rejoin, full_nonce, earnin, consensus.
2. **Unit tests (members.rs).**
   - Property: random sorted sets give `apply(diff(a,b), a) == b`.
   - The overlay forward-apply equals sequential application, including add→remove→add of the same id.
   - The O(Δ) digest update equals the full recompute.
   - The snapshot rule at h = 1, 256, 257 and Δ = 32/33.
   - Encode/decode round-trip.
   - Seal, shape and structure violations (wrong h, a Delta at a cadence height, a walk over 256 steps) are Local and call `damaged`.
   - `SmallLru` caps.
3. **`NonceOwners` equivalence.** The slice and `LazyOwners` impls agree on random nonces, including nonce ≥ nMax and Bounded-mode empty ranges.
   - The v3_late_shares.rs vector JSON is **unchanged** and passes; only :35-39 builds `owners:&ranges[..]`.
   - Add a test that a kind-3 item failing the cap check is rejected `ls("cap")` with no reconstruction (counter unchanged) [C1-D4].
4. **Kind-3 and type-2 across slim/unslim.** Extend consensus.rs:2938-2950 (slim_depth 8, reference node unslimmed):
   - type-2 against an ancestor deeper than 8 (the NeedsRebuild → unslim path);
   - kind-3 at j = h−D;
   - verdicts and effects equal the reference.
   - In the "restored exactly" tuple (:2944) replace `r.ranges` with `members`, and adjust the `small` sum (:2948).
5. **Cold path from disk.**
   - `clear_member_caches()`, then evidence against an ancestor whose nearest snapshot is 255 below and offloaded, so its set comes from `member_sets`. The disk-read counter is above 0 and the verdict is unchanged.
   - A walk crossing `canon_base` after a checkpoint restore uses `member_src.record`.
   - **Minimum-E test [C1-D7]:** store mode at exactly `evict_depth_of`'s e_min, an anchor rebuild whose evidence ancestors' walks cross the floor.
6. **Restart.**
   - Three ways: (a) checkpoint restore; (b) checkpoints deleted, so replay; (c) history.redb truncated (consensus.rs:2521) or deleted, so rebuilt.
   - Then compare fingerprints: consensus.rs:3776 uses `&e.members`; live == replay == restored. `member_ids` is excluded by construction.
   - Then a following block carrying type-2 and kind-3 evidence is judged exactly as the reference judges it.
   - Fix consensus.rs:2527 to the new `put` signature, and extend the crash-point test (:2525) to snapshot heights.
7. **Reorg.**
   - Sibling branches differing by `registerReplacing` (Δ +1/−1) or a type-2 punish (−1). Fork choice switches, and evidence on the new branch references ancestors above and below the fork. Verdicts equal the reference.
   - Repeat with the fork at a snapshot height, and with a branch crossing one.
   - A store-mode deep rebuild with caches cleared, plus a deliberately corrupted record: the `reexec` n/digest check gives Local plus `damaged`, and the next open rebuilds.
8. **Snapshot by overflow.**
   - A spec-1 Bounded W_act flip changing more than DELTA_CAP ids writes an off-cadence snapshot; verdicts unchanged. This test goes after Phase 6a.
   - Also a CP with `delta_max > DELTA_CAP` checks that A13's `snaps = blocks` branch is taken.
9. **Migration and corruption [C2-12].**
   - Build an S4-format history.redb at test time with redb (receipts and ranges tables, no members), not as a committed binary.
   - Open it: no set-aside (`rebuilt == false`); the restore falls back to replay; member rows are refilled; `has` is true for every block.
   - A 3a store opened by 3b loses its RANGES table without a rebuild.
   - A flipped byte in a member row is marked damaged, and the next open rebuilds.
   - An io-fault at open keeps the file.
10. **Bare chain [C1-D2].**
    - memory_window.rs `long_chain_heap_is_flat` still passes with the heap flat (MemSets is pruned).
    - Add a bare-chain eviction test with evidence against an ancestor whose snapshot is evicted.
11. **Memory bounds** (tests/memory_bounds.rs).
    - Offloaded entries hold no `member_ids`.
    - The LRUs never exceed their caps.
    - The delta-heap term ≥ the actual heap of any entry.
12. **Proposer pool [C2-10, C1-D5].**
    - A full pool of 64 type-2 items against distinct ancestors: the second template's `includable_evidence` is served from `owners` (hits > 0), and the time is measured for M1.
    - A Local error (an injected `member_src` failure) keeps the item in the pool.
13. **A13 store mode (FN-3c).**
    - Derive at the hand-built fresh CP (§0: SYS_SLOT_MAX 147,456) with `Limits.disk` set.
    - `check` passes and prints the margin.
    - A sibling assertion records that the pre-FN-3 formula exceeds 5.25 GiB.
    - Snapshot the member term table.
14. **Growth and CPU measurement (FN-3c; `#[ignore]` with a recipe, Linux-only, outside the Windows job budget [C2-12]).**
    - **Harness [C2-11].** FN-3c builds a minimal test-only host-level 10k-record genesis helper that bypasses testnet.rs:257's 256-miner limit. Phase 5's CI full-set test reuses it.
    - Produce about 600 blocks spanning two snapshots, idle and at |Δ| = 24.
    - Measure history bytes per block from `diskObserved` (node.rs:3315), and assert the M-dependent part is < 100 MB/day.
    - Measure reconstruction p99, cold (255-step walk plus disk snapshot) and warm; about 5–10 ms cold is expected.
    - Measure `h_full` overhead, the pool case from §5.12, and RSS against A13.
    - These feed M1 (child_ctx p99 < 100 ms comes from FN-1; RSS within A13; history growth) and D1.

---

## 6. Lock line
- **The files touched inside the boundary:** chain.rs, history.rs, checkpoint.rs, the new members.rs, consensus-boundary.txt and lp1-node assignment.rs [C2-5].
- **Each code PR (3a, 3b)** re-pins **one `kind=neutral` line with no gate**. Note: "C6/FN-3: nonce ranges no longer stored; historical owner lookups recompute Assignment from persisted member-set deltas; verdict code paths unchanged (oracle)".
- **What keeps it neutral:**
  - no vector file changes (`NonceOwners` has a slice impl);
  - no dependency or feature change (hand-rolled LRU, a runtime oracle flag);
  - full_nonce, decoder_reference, wire_mutation and profile tests pass;
  - the guard is run locally, including check 7 and the "(+) references … node-local" check for members.rs.
- **If anything forces a vector or dependency change,** stop and escalate (`neutral-vector`/`neutral-dep`, presumed bump, excluded by the owner rule).

---

## 7. PR split

**FN-3a: member records, additive, with the shadow oracle (lock: neutral).**
- Code:
  - members.rs and the boundary entry;
  - `Entry.members`/`member_ids` next to `ranges`;
  - the `h_full` record, the Chain source, MemSets and caches, and the `from_ids` refactor;
  - `NonceOwners`/`LazyOwners`, with the readers in shadow mode (verdict from `ranges`);
  - history MEMBERS/MEMBER_SETS written next to RANGES, `has` = receipts && ranges && members;
  - checkpoint, `Src` and `indexed_view` (None) members wiring;
  - `Arc<History>` plus `set_member_source` at :986;
  - reexec check, `includable_evidence` Local fix, status counters.
- A13: the new terms (64·m kept), the ChildCtx term if FN-1 lacks it, and CONDITIONS. Re-pin the three A13 tests.
- History.rs doc.
- Tests §5.1–5.7, §5.9 (S4 refill and io-fault), §5.10, §5.12.
- Size: about +1,000/−100.

**FN-3b: switch the readers and drop ranges (lock: neutral).**
- Readers take their verdict from `LazyOwners`.
- Remove `Entry.ranges`, RANGES (with `delete_table` at open), `Ranges` and `ranges_of`, and the ranges code in slim, meta_clone, restored_fields, restored, unslim, restore_canonical, entry_of, `Src` and indexed_view. `has` = receipts && members.
- A13: −64·m, −52·m; re-pin the three A13 tests, with the halt-test subtraction made signed.
- Tests:
  - oracle against `ctx.assignment` and the 3b capture map;
  - consensus.rs:2527/2944/2948/3776;
  - v3_membership.rs:64-65/:361 (`entry.ranges` becomes `chain.assignment_of(&entry).ranges()`);
  - §5.8, §5.11, and §5.9's 3a→3b case.
- Docs inside the boundary: chain.rs:1100-1106 and history.rs.
- Size: about −350/+150.

**FN-3c: evidence and docs (no boundary file, so no lock line).**
- §5.13 and §5.14: the 10k A13 test, the 10k genesis helper, the growth and CPU harness.
- Docs: FRESH-NETWORK-DESIGN risk 1 (corrected figures), the C6 verification results, A13-DERIVATION.
- If 3c must touch any boundary file, it gets its own neutral line or folds into 3b [C2-5].

**Why not "3a in-memory / 3b persistence".**
- An in-memory-only step cannot survive offload, restart or checkpoint restore.
- It would need throwaway code deriving sets from `Entry.ranges`, which slim entries do not carry.
- "Additive with shadow, then switch and remove" keeps the old field as a free oracle on every read path for one PR.

---

## 8. Risks and checks

| # | Risk | Check |
|---|---|---|
| 1 | Reconstructed assignment differs, so a verdict changes | One code path (`from_ids`); oracle at import, at every reader in 3a, and a cold end-of-test sweep (§5.1); reexec n/digest; error texts and order unchanged (§5.3). |
| 2 | A reconstructed set reaches DrawWeights or FN-1's membership cache | Type separation (`Arc<[Address]>`); reconstructions are inserted only into FN-3's LRUs; type-1 uses live `membership()`; review item. |
| 3 | CPU DoS through cold reconstructions | Per block ≤ E_max + D ancestors; CP-derived caps; lazy lookup after the cap check; owner memo for the pool; cold cost measured (§5.12, §5.14). |
| 4 | A local failure becomes a block verdict or drops evidence | Every reader failure, `AssignmentError` included, is `Reject::Local` plus `damaged`; `includable_evidence` keeps items on Local; §5.9, §5.12. |
| 5 | I3: a snapshot's ids are not durable before the journal append | Same commit as receipts; `put` errors on a snapshot without ids; `member_ids` dropped only by offload, which follows `put`; crash-point test (§5.6). |
| 6 | Live, replayed and restored entries differ | Deterministic rule; ids outside the compared data; fingerprint test (§5.6). |
| 7 | A walk crosses the floor or `canon_base` | Records carry the parent hash; `member_src.record`; structural bounds; minimum-E test (§5.5); doc at chain.rs:1104 updated. |
| 8 | Old data dirs | `has`-driven refill via replay, no set-aside; 3b drops RANGES; §5.9. |
| 9 | Lock kind escalates or the guard fails | members.rs in the `+` list; no vectors or dependencies touched; guard run locally (§6). |
| 10 | Memory regressions | offload drops ids; MemSets pruned; memory_bounds and memory_window (§5.10, §5.11). |
| 11 | A CP with Δ > DELTA_CAP, or spec-1 Bounded, writes many snapshots | Correct by rule; A13 uses `delta_max(cp)`; §5.8. |
| 12 | FN-1 API drift or Phase 4 cfg changes | Rebase after both; `assigns` covers a missing FN-1 cache; re-grep the line numbers. |
| 13 | The design understates the risk (2.8 GiB vs ≈ 6.69 GiB) | FN-3c records both A13 outputs and corrects risk 1. |
| 14 | "≪ 100 MB/day" read as total disk under tx load | M-dependent and receipts parts reported separately (M1, D1). |
| 15 | The pinned halt test panics on u64 underflow when the halt rises | Signed subtraction; re-pin with the delta; restate the seed rule (§4). |

---

## 9. Step order for the implementer
1. Rebase on FN-1 and Phase 4. Run baseline A13 at 10k with the hand-built CP and `Limits.disk`; record the result (§0).
2. assignment.rs `from_ids` refactor. Build pocold and lp4; all tests unchanged.
3. members.rs with unit tests (§5.2); add it to consensus-boundary.txt.
4. history.rs: tables, opens inside `create`, put/read, the `MemberSets` impl, io-fault (§5.9).
5. chain.rs: `Entry.members`/`member_ids`, `h_full` record, source, MemSets and caches, offload drops ids, evict prune, `NonceOwners`/`LazyOwners` in shadow mode, reexec check, oracle.
6. checkpoint.rs `entry_of`/`Source`; node.rs `Src`, `indexed_view`, `Arc<History>`, `set_member_source` at :986, puts, `includable_evidence`, status.
7. resources.rs: new terms with `delta_max`, the ChildCtx term if missing, CONDITIONS; re-pin the A13 tests.
8. Tests §5.1, §5.3–5.7, §5.9, §5.10, §5.12. Run the lock guard and re-pin `neutral`. PR **FN-3a**.
9. FN-3b: switch the readers, remove ranges, delete RANGES at open, A13 removals and re-pins (signed subtraction), tests §5.8 and §5.11, boundary docs, re-pin. PR **FN-3b**.
10. FN-3c: 10k genesis helper, 10k A13 test, growth and CPU harness, design and A13 docs.

---

## Appendix A: critique points rejected or partly rejected

| Point | Decision | Reason |
|---|---|---|
| C1-D1: "use a format marker" (one of its fixes) | Marker dropped; alternative taken | C1-D1's own second option and C2-3 point to `has`-driven refill, which already exists (history.rs:94-98). It needs no set-aside, so C2-4's Windows-rename hazard disappears and receipts are kept. The `delete_table` and opens still go through `rerr` inside `create` (C2-4 accepted for that part). |
| C1-D3 / C2-2: "chain-level `pending_sets` map, removed after put" | Partly: the ids moved out of `MemberRec` into an `Entry.member_ids` cache field instead | A pending map needs cleanup for blocks that fail after `h_full` or are never put, plus bounds. A cache field on the Entry dies with the Entry, is dropped by `offload`, and is excluded from equality. That solves the same live/restored mismatch, and it also removes "keep ctx until put" (accepted). |
| C1-D5: "on NeedsRebuild keep it too" | Rejected for FN-3 | Dropping on NeedsRebuild is today's behaviour, not something FN-3 introduces. FN-3 fixes only the new Local case. Changing pool policy for NeedsRebuild is a separate node-local change. |
| C1-D7: "put the block's own seal in the record (+32 B)" | Rejected; the doc, test and debug assert are accepted | `assignment_of` needs the parent's seal only for e within E_win + D of the carrier. The store-mode keep term max(257, E_win+1) (resources.rs:858-860) and the shadow (chain.rs:1466) always hold that parent. The walk below the floor reads only records and sets, which need no seal. A missing parent stays Local plus a debug assert. |
| C2-8: "optionally split 3a into records/shadow and readers switched" | Taken as stated: 3a is shadow-only and 3b switches | No further split is needed. |
| C2-10: "memoize the verdict per (p, item)" | Replaced by an owner memo keyed by (block hash, nonce) | A verdict memo keyed by p would miss when p changes every block. Owner lookups are the expensive part and stay valid across parents; verdict logic stays unchanged and uncached. |
| C2-12: "reexec warms `sets`" | Accepted as optional | It is a cache warm-up, not needed for correctness. |
| C1-D10 line-number drift | Accepted | The tables now use re-checked 984ea50 numbers (offload :269, normalize :281, shadow_push :1468). The walk bound is restated as max(E_win, D) + 256 in §1.3 via WALK_MAX and the keep term. |

All other points in both critiques were verified in the code and accepted as marked.

Paths:
- Repo (read only): /tmp/claude-0/-home-user-Proof-of-Collaboration/5c42c059-8cb3-5c8b-83e8-431ffbd082d1/scratchpad/ro-main
- Design: /tmp/claude-0/-home-user-Proof-of-Collaboration/5c42c059-8cb3-5c8b-83e8-431ffbd082d1/scratchpad/handoff-wt/development/handoff/FRESH-NETWORK-DESIGN.md (C6 :119, FN-3 :146, risk 1 :161, M1 :214, D1 :219)
