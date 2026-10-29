# Decision package (2026-10-24)

**Status: for the owner's decision.** Nothing in this file is approved because it is written here, and no normative text is
edited. Each item has one of three tags:

- **[CONFORMANCE]**: brings pocold closer to text that is already approved (the M1 reference). It changes
  no block validity, root, receipt, genesisHash or RPC-visible result. Under the current authorization
  ("don't stop for routine implementation steps") it is being implemented, in the order of §6, each change
  with an equivalence test and a mutation control. You may stop or reorder it.
- **[NORMATIVE]**: changes protocol text, consensus, CP values, a protocol constant, the RPC interface or a
  node requirement. **It needs your approval.** Nothing here is implemented beyond the "provisional"
  implementations already recorded in proposals/README.md.
- **[SCOPE]**: no consensus effect, but a choice of target or effort that is yours to make.

The Arabic summary for approval is [DECISION-PACKAGE-AR.md](DECISION-PACKAGE-AR.md).

Priorities, as you set them:
1. booting the network profile within the budget;
2. block validation time;
3. sustainability of the caps;
4. journal growth and recovery cost.

The other pending decisions (the six consensus proposals and RpcGuard P1–P4) are in §5.

---

## 1. Priority 1: booting the network profile within 6 GiB

### Facts (measured with the code's own model)

`pocold::resources::derive` was run on the governance.md network parameters (K_hist 32), with the new
test `tests/network_budget.rs` (evidence/decision-package/a13-network-profile.log):

| group | GiB | why it is this large |
|---|---:|---|
| state snapshots | 75.12 | 45 **full copies** of the state (1.63 GiB each at ACCT_CAP 10^6, SLOT_CAP 4·10^6): head + 32 K_hist + 2 side + 10 transient copies |
| code | 54.69 | CODE_CAP (1 GiB) × 9/8 counted **per copy**, unshared at worst |
| history | 19.19 | all canonical history in memory: tx bytes 8.00, tx index and receipts 3.54, logs 1.03, evidence 1.99, per-block entries 4.63 |
| execution | 1.10 | one EVM execution at 30M gas |
| side blocks, queues, RPC, threads | 0.73 | — |
| **model total** | **150.83** | the gate needs ⌈8·total/7⌉ = **172.4 GiB**, against 6 GiB |

Round 3 wrote that "TXBYTES_CAP = 8 GiB alone exceeds the budget". That was true, but it understated
the gap about 25-fold. The dominant terms are the full state copies and code counted per copy.

The spec's own design is disk-backed. storage.md:63–73 keeps blocks, receipts, logs, code and indexes
in redb, with a live trie, historical versions K_eff ≤ 32 within 1.6 GiB, and diffs for 2048 blocks.
FINAL_DESIGN.md D30 sets "K_eff ≤ 32, diffs for 2048 blocks, TrieRebuild" because "128 exceeds 6 GiB".
So the 6 GiB budget is the spec's assumption, and pocold's in-memory architecture is what breaks it.

### C-1 [CONFORMANCE]: storage program toward storage.md / D30

Five stages. Each stage is validity-neutral, and each is verified against the current implementation:
the same roots, receipts and RPC answers on every suite and replay.

**Correction (2026-10-24, owner's instruction):** the figures first given here (≈ 136 / 83 / 14 / 10 GiB)
were not results. The table below replaces them.
- Each projection starts from the **measured `derive()` items after S2** (105.83 GiB,
  evidence/decision-package/a13-network-profile-s2.log) and removes the items the stage moves.
- What each stage adds back (caches, diffs, overlays) is a **new term not yet derived**, shown as "+ new".
- A stage's figure becomes a result only when its code lands and `derive()` prints it.

| stage | change (spec text) | items removed (GiB, measured) | network model after it |
|---|---|---|---:|
| S2 ✔ | code shared between snapshots (done, `d567ce1`) | code 54.69 → 9.69 | **105.83 (measured)** |
| S1 ✔ | canonical bodies, receipts, logs and evidence off the heap below a resident window of K_hist blocks, read from the journal and history.redb on demand (storage.md:70) | history 19.19 → 4.28 (tx bytes, receipts and index, logs, evidence moved; offloaded entries drop their header bytes; added: window 0.013, redb cache and allocator 0.025) | **90.93 (measured)**, gate 103.92 |
| S3a ✔ | retained canonical states as diffs from their parents, materialized from the head on demand (storage.md:70, D30) | 33 retained copies (53.89) less 4 added (genesis counted, 2 materialized), and their code containers (2.62); added: the diffs 0.46 | **41.41 (measured)**, gate 47.33 |
| S4 ✔ | old canonical entries slimmed, restored exactly from the journal and history.redb | history 4.28 → 0.83 | **37.96 (measured)**, gate 43.38 |
| E1 ✔ (S5 step 1) | incremental state root (C-2 item 4): the head's trie in memory, any other parent's derived from it through the diffs or built | added: the trie group 2.23 (the head's trie, one derived or built, a block's changed paths); it moves to state.redb with E4 | **40.20 (measured)**, gate 45.94 |
| E2 ✔ (S5 step 2) | the per-block whole-state checks (candidate checks, code checks, code drop) from a state summary moved through the changed keys | added: one boxed summary per entry (devnet +2.1 MB) | **40.20 (measured)** |
| E3 ✔ (S3b, E3.1–E3.3) | the executor, production, the pool and RPC on overlays over the parent (lp4 `view::Overlay`); the S3a diff from the block's write set | snapshots 27.76 → 16.33 (transient copies 10 → 2; the pool's layer counted as one whole state; growth copy kept), code containers 1.45 → 0.81, history 0.84 → 0.78 (smaller `Entry`); added: the applied block's overlay 0.004 | **28.08 (measured)**, gate 32.09. The projection was ≈ 19.1: it assumed the growth copy and every transient copy go; two transient copies, the growth copy and the pool's layer remain |
| E4 (S5; E3.4 folded in; D-6 is E4.4) | the trie on disk is the state: every retained state (head, window, side, genesis, checkpoint) is a root over hash-addressed nodes in state.redb, shared between versions; reads through bounded caches; a block commits its nodes, reverse diff and root before the journal append | projected: every whole state (16.33), the in-memory tries (2.23) and the code containers (0.81) | ≈ 8.7 before E5 (the four code sets remain) + new terms: node and code caches, the state.redb page cache, one block's new nodes |
| E5 | code read from the store by the rebuild and the pool; a smaller inline entry | projected: the code sets (5.63) | ≈ 3.08 + E4's and E5's new terms; target ≤ 5.25, so the new terms must stay ≤ 2.17 |

**Recomputed after E3 (2026-10-25), from the measured E3 items (evidence/c1-e3/a13-network-profile-e3.log).**
The earlier S5 projection (≈ 6.5, or 6.0 GiB) assumed that S3b would remove every transient copy and the
growth copy, and it was not a measured result. The measured E3 figure is 28.08 GiB. What is not a whole
state, a code set or a trie comes to 3.08 GiB:
- the diffs 0.46;
- execution 1.10;
- history 0.78;
- side metadata 0.20, queues 0.24, RPC 0.20, threads 0.08.

That is the floor E4 and E5 can reach, before their own new terms. Reaching it needs all three of these
local changes, none of them normative:
- every whole state off the heap: the head and its trie in state.redb (E4), the side, materialized and
  working states read through overlays over the store;
- code read from the store by the rebuild and the pool, not copied (E5). Their code sets are full because
  a separate execution makes separate buffers;
- the pool's layer bounded by the pool's gas, not by the block caps. D-7's local work bound would give this.
  The bound is approved in principle, but its value is not, so the model keeps the layer as a whole state.

These figures are projections until `derive()` prints them. They are derived and pinned stage by stage.

**Measured after E4 and E5 (2026-10-25), `derive()` in store mode** (evidence/c1-e4/a13-final.log, commit
fc24969; A13-DERIVATION §2c). E4.1–E4.5 and E5 parts 1–3 have landed. Store mode is opt-in
(`Limits::state_store`, off by default).

The network model is **9.67 GiB (gate 11.05)**, against 27.92 GiB in whole-state mode. These are
computed bounds, not measured RAM:

| group | GiB | what it is |
|---|---:|---|
| snapshots | 1.63 | the pool's layer, counted as a whole state |
| code | 2.34 | code read by the execution in progress (1.13), the pool's code (1.13), containers (0.09) |
| store | 3.23 | a block's batch 1.02; node cache 0.25; redb page cache 0.06; redb allocator 1.81; checkpoint 0.08; handles 0.005 |
| execution | 1.10 | unchanged |
| history | 0.63 | slimmed and offloaded metadata; one empty state shared by every entry without a state (E5) |
| side, queues, RPC, threads, diffs | 0.73 | unchanged |

How it moved:
- 10.58 GiB at E4.5 part 1;
- 10.24 GiB after preimages went into the leaves (E5 part 1);
- 9.07 GiB with redb's allocator counted as redb builds it (E5 part 2);
- 9.67 GiB with state.redb's own measured file factor, 12 instead of history.redb's 8 (E5 part 3).

The first file-factor measurement (about 18) came from closing: a closing node released every version in a
non-durable transaction of its own, so a close grew the file threefold to fourfold. A close now leaves
the versions to the next open.

**Decision needed (blocking 6 GiB at the network profile; not a normative change):**

1. **The D-7 value (the pool's gas bound).** Approved in principle; its value is not.
   - With a bound G, the pool's layer (1.63) and its code set (1.13) follow G instead of a whole state and a
     whole code set.
   - Projection at G = 4·GAS_LIMIT, from the D-7 work: about −2.7 GiB, to ≈ 7.0 GiB. `derive()` computes it
     only once a value is approved.
   - Recommended, as in the D-7 work where it was measured under load: G = 4·GAS_LIMIT.
   - Alternatives:
     - 2·GAS_LIMIT: a little lower, with more pool churn;
     - none: the model stays at 9.67 GiB.

**Local work that would lower it further (none of it normative; not done):**
- **A gas-derived bound on the code one execution reads (≈ −1.0 GiB).** Today one execution may read every
  account's code (CODE_CAP). Every account an execution loads costs gas: a cold access, an access-list
  entry, or a transaction. Each deployed runtime is at most 24,576 bytes (EIP-170). That bound holds only after revm 19.7's
  loading paths and the genesis programs' sizes are checked (which loads are free, and whether the set
  lives per transaction or per block), so it is not counted.
- **The file factor measured at scale.** It is measured on stores of 1.5 to 15 MB only. Each unit of the
  factor is about 0.15 GiB of the bound.
- **E5's code sets:** the rebuild and the pool still read code into their own sets.

**A modelling choice for the owner (unchanged):** the batch term (1.02 GiB) takes every key path at 65 nodes
(no limit on key-prefix grinding). A computational bound on grinding, for example paths ≤ 21 nodes (2^80
hashes), would cut the batch to ≈ 0.33 GiB. It is an A13 assumption, so it is not adopted without approval.

**Where that leaves 6 GiB.** The gate needs a model total of at most 5.25 GiB. With the D-7 value, the
gas-derived code bound and the path assumption, the projection is ≈ 5.3 GiB: at the limit, not below it.
Store mode stays opt-in, and the network profile does not boot, until a computed `derive()` fits.

**After D-7 and the code-loading audit (2026-10-25; A13-DERIVATION §2d, evidence/d7-bound).** The paragraphs
above are kept as recorded. D-7 is approved at 4·GAS_LIMIT and implemented as the default. Before any term
was lowered, revm 19.7's loading paths were audited.

The network store-mode model is now **7.61 GiB (gate 8.69)**; whole-state mode 25.24 GiB:
- the pool's layer follows G: 1.633 → 0.127 GiB;
- the pool's code in store mode is its touched accounts' code and its creations: 1.125 → 0.618 GiB;
- the block overlay is re-derived upward (0.004 → 0.041 GiB), because a created account keeps every slot it
  loaded, which was not counted.

**Withdrawn:** the projection "a gas-derived bound on the code one execution reads, about −1.0 GiB". In store
mode every account load reads its code, and revm loads an account before it charges for it, so a load can
cost as little as 100 gas. The term stays one CODE_CAP set.

**Still over: 7.61 GiB against the 5.25 GiB cap, by 2.36 GiB.** The options, with their numbers. Each figure
is the computed network store-mode total with only that change; they combine roughly additively:

| option | kind | total | effect |
|---|---|---:|---|
| none | — | 7.61 | store mode stays opt-in; the network profile does not boot |
| G = 2·GAS_LIMIT instead of 4 | your D-7 value | 7.23 | more pool churn under load; as approved, not taken |
| key paths ≤ 21 nodes (below) | A13 assumption | 6.79 | batch 1.02 → 0.33, allocator 1.81 → 1.68 |
| pool layer keeps no code copies (code read again on demand) | local implementation | ≈ 7.0 | −0.62; more state.redb reads per pool execution; not implemented |
| NODE_CACHE 256 → 64 MiB | local setting | ≈ 7.42 | slower reads in store mode |
| state.redb factor 12 → 6 (the largest measured) | measurement at scale | ≈ 6.70 | not established for terabyte files; cannot be counted until measured there |
| CODE_CAP 1 GiB → 256 MiB | **normative** (genesis parameter) | 6.43 | less contract code on the network |
| key paths ≤ 21 + CODE_CAP 128 MiB | assumption + normative | 5.33 | still above 5.25 |

Measured on 9bd9bc0, not bounds (evidence/d7-bound/README.md):
- **state.redb under churn:** 30 cycles; the file stays at 67.9 MB from cycle 5 on, at most 7.95 times what it
  stores.
- **R14/R19:** the admission timing does not include store mode's commit and fsync. A GAS_LIMIT compute
  block imports in 64 ms in store mode against 29 ms whole-state, and state-heavy blocks took up to 4.8 s.
  Calibrating in the booting mode is open.
- **R14/R19 in the booting mode, done after this (A13-DERIVATION §3).**
  - A node that boots in store mode is now admitted on a timing that includes the calibration blocks applied
    through a scratch state store on its own disk, each committed durably. The gate takes the larger time per
    phase.
  - On this host: 10–26 ms per phase through the store, against 8–18 ms in memory (15 runs); t_exec is 1,000 ms.
- **Decision for you (not normative, and not adopted): the calibration workload.**
  - The calibration is a GAS_LIMIT compute loop, which writes little state.
  - State-heavy blocks are slower than t_exec on this host in both modes:
    - large state: up to 1,068.5 ms whole-state and 1,045.1 ms store;
    - small store: one outlier at 4,841.8 ms (disk-impact-final.md).
  - With a state-heavy calibration this host would refuse both modes at t_exec = 1 s. The alternatives are a
    larger local t_exec, which is the P11 value of option (a) above, or keeping the condition
    `timing-not-a-bound` as it is.
  - **Correction (2026-10-27, A13-DERIVATION §3b).** The state-heavy figures above were very likely measured
    with pocold's test-only root checks on (`POCOL_ROOT_CHECK` defaults to on in test builds; the runs do not
    show the setting). Each block was then also run on a whole-state reference path.
    - With the checks off, state-heavy blocks stay well under t_exec in both modes: at most 0.42 s on a
      near-cap state, except MODEXP.
    - The only blocks over 1 s are MODEXP with an even modulus (1.2–1.6 s), which is D-2.
    - The same measurement found a store-mode import that executed blocks twice; it is fixed.
    - The decision now reads as row 2 of [OPEN-DECISIONS.md](OPEN-DECISIONS.md), together with D-2 (row 1).

No combination of local, non-normative options reaches 5.25 GiB. The remaining terms are the redb
allocator over the adversarial file (1.81), the execution's code set (1.125), the EVM execution at 30M gas
(1.10) and the batch (1.02). Recommendation: keep store mode opt-in, and decide the network target (D-1)
with these figures. No bound was lowered to reach the budget.

**Recounted on 2026-10-27 (A13-DERIVATION §2e, `892dca6`; the paragraphs above are kept as recorded).** The
7.61 GiB was itemized term by term. Four terms were cut where the code was changed to hold less, or where a
tighter count was proven and tested against a real store. Nothing normative, no local limit and no
acceptance criterion changed.

| reduction | kind | network store term | proof |
|---|---|---:|---|
| the pool's layer keeps the code hash, not a code copy (the row "pool layer keeps no code copies" above, now implemented) | local implementation | 0.618 → 0.0003 | equivalence over 2,000 random layers; a store-mode node test |
| state.redb counts a shared node once, and nodes by kind | proven count | allocator 1.809 → 0.279 | the store is content-addressed with reference counts; tests over real stores, paths of 64 branches |
| a block's batch by node kind | proven count | 1.024 → 0.916 | the same tests, both worst cases reached exactly |
| the chain's entries boxed in their map | local implementation | metadata 0.786 → 0.524 | the type's size; pinned devnet total updated |

- **Result:** network store mode **5.086 GiB, gate 5.81 GiB**, under the 5.25 GiB model cap and the 6 GiB
  budget. Whole-state mode is 24.98 GiB; the devnet in store mode is 2.44 GiB.
  - *Updated 2026-10-27 (R3):* **5.198 GiB, gate 5.94 GiB**, still under the cap. `5abbaf3` adds two
    stored-version handles. `93a40d0`'s deferred checkpoint release adds the owed checkpoint and its release
    rows (A13-DERIVATION §3c).
- **Two undercounts found and fixed on the way:**
  - the file bound left out the abandoned window and the rolling re-execution, which `stored_versions`
    lists;
  - the batch's 65 nodes per key left out the leaf a changed key displaces.
- **Cost:** a pool read of an account the pool touched now reads its code from state.redb. That is the
  same read as for an account the pool did not touch.
- **Not established by this:**
  - acceptance;
  - a measured RSS at network scale;
  - the network profile booting (its other gates still refuse it);
  - the file factor for large files (condition `store-file-factor`).
- **The key-path assumption** is still not used. With it the bound would be about 0.7 GiB lower.
- **Recommendation: keep store mode opt-in** until a near-cap network-scale state has been run and measured.
  The open decisions are in [OPEN-DECISIONS.md](OPEN-DECISIONS.md).

**The key-path assumption (not adopted; presented as asked).**
- **Definition.** Every key path in a state trie has at most 21 nodes. Today `TRIE_PATH_NODES` = 65: a
  branch at each of the 64 nibbles of a hashed key, and the leaf.
- **Rationale.** Keys are keccak hashes. A path has one branch for each depth at which another key diverges
  from it, so a path longer than 21 nodes needs keys that share at least 20 nibbles (80 bits) of hash
  prefix with it. Producing those is a computational cost (on the order of 2^76–2^80 hashes per such path),
  not something a protocol rule prevents.
- **Worst case if it fails.** The terms that count paths undercount: the batch (1.02 GiB at 65 nodes, 0.33
  at 21), the whole-state trie paths, and the state.redb file behind the allocator term. Each key with a
  longer path adds up to 44 nodes to its block's batch. The node would then run past its computed bound
  in proportion to how many such keys an adversary paid for.
- **Conservative alternative (current).** Count 65 nodes. It holds with no assumption and costs 0.82 GiB of
  the network store-mode bound against the assumption.

**Disk and time (measured on 5ab73d1, devnet profile, 4 vCPU KVM, ext4 on virtio;
evidence/c1-e4/disk-impact-final.md).** Small store: 2,770 accounts, 2.8 MB stored. Larger: 16,110 accounts,
14.9 MB stored.
- **Import:** store mode is 1.8–2.4× slower per block on the small store, where every block's commit is
  fsynced. On the larger state it is 1.23× slower for account-creating blocks and about equal (0.94×) for
  full blocks.
- **Reorg:** store mode is faster, 1.5× and 2.7×, because it reads stored versions in place.
- **Open from a checkpoint:** 1.8× and 4.0× faster than whole-state mode's replay. Store mode's replay from
  genesis is slower, because it refills the store.
- **Files:** state.redb is 9.0 MB and 67.9 MB, beside history.redb (17.4 and 4.7 MB) and the journal (1.9 and
  0.5 MB).
- **RSS after sync and reorg:** 48.3 against 42.0 MB, and 112.7 against 113.4 MB.
- **Variance:** fsync latency varies between runs on this disk, so store-mode times are indicative, within
  about a factor of two.

- **Consensus, genesisHash and validity:** none.
- **Compatibility:** the journal and storage format change. This is a local migration; no on-chain or RPC
  change.
- **Resources:** the disk grows toward the spec inventory (≈ 19.65 GiB logical, ≈ 29.5 GiB on disk at the
  network caps). Memory falls as in the table. Disk reads add latency to RPC history queries and to
  deep reorgs.

**Acceptance tests:**
1. `a13_model_at_the_network_profile` passes with `budget_required ≤ 6 GiB`. The test currently asserts
   the opposite and must be inverted when S5 lands.
2. For each stage, the moved term is gone from `derive()` and replaced by its proven bound.
3. The existing equivalence suites pass unchanged: lp4 vectors, pocold consensus and caps, e2e_devnet,
   replay equals live, the spec-depth 2048/2049 test and the D133 test at depth 400.
4. A crash test for each new on-disk structure, in the style of the D-REORG-REDB R2.5 tests.
5. Measured RSS on a synthetic near-cap network state stays under the budget.
6. One mutation control per stage.

### D-1 [SCOPE]: the target for the network profile

| option | what it means | effect |
|---|---|---|
| **(a) full conformance S1–S5** | pocold boots the network profile within 6 GiB, as the spec's design intends | largest effort; no spec change |
| (b) S1–S3 plus a larger node memory requirement | stop after S3 (9.30 GiB + the diff terms, not yet derived) | **[NORMATIVE]**: changes the node requirement behind D30 |
| (c) pocold stays devnet-only | the network node is a separate implementation | no work, but the network profile stays unverified |

**Recommendation: (a), in stages.** **Approved by the owner on 2026-10-24**: complete S1–S5 so that the
network profile runs within 6 GiB, with equivalence proofs, data preservation and crash tests, without
waiting for routine approvals.

**S2 status (2026-10-24, done; measured by `derive()`):**
- The test `journal_measure::retained_states_of_one_lineage_share_their_code_buffers` shows that a
  state copy shares its runtime buffers. Its control shows that a separate execution makes its own.
- The code term is now:
  - one set for the canonical lineage;
  - one set per side state, one for a rebuild or restore in progress, and one for the pool's pending
    state (unshared at worst);
  - plus the per-copy containers.
- Network code group: 54.69 → 9.69 GiB. Model total: 150.83 → **105.83 GiB**, so the gate needs
  **120.95 GiB** (evidence/decision-package/a13-network-profile-s2.log). Still far over 6 GiB.
- Devnet model total: 5,581,426,268 → 5,487,223,178 B, which leaves 163.4 MiB unused under the 6 GiB
  gate.
- The real remaining terms are the 45 full state copies (75.12 GiB, S3/S5) and the in-memory history
  (19.19 GiB, S1).

**S1 status (2026-10-24, done; measured by `derive()`):** proposals/C1-STORAGE-DESIGN.md "S1 status" and
evidence/c1-s1/.
- Canonical blocks below a resident window (K_hist) keep only their metadata in memory. Bodies and
  headers are read from the journal; receipts and the transaction index come from `history.redb`.
- Network history group: 19.19 → 4.28 GiB. Model total: 105.83 → **90.93 GiB**, so the gate needs
  **103.92 GiB**. Still far over 6 GiB: the full state copies (75.12 GiB) are S3/S5's.
- Devnet model total: 5,487,223,178 → 4,323,240,949 B.
- The answers are byte-identical to the tree before S1 on the acceptance scenario. Damage to the store
  (deletion, truncation, junk, flipped bytes, wrong content) is never served and is rebuilt from the
  journal.
- Two local behaviours are new, and neither is normative:
  - open runs redb's full integrity check, about 1–1.4 s per GiB of store;
  - while a damaged store is detected, history RPC reads answer −32603, and the node stops at its next
    import, as on a journal error.

**S3a status (2026-10-24, done; measured by `derive()`):** proposals/C1-STORAGE-DESIGN.md "S3a status" and
evidence/c1-s3a/.
- Only the head, genesis and the side states within side_max stay full. Every other retained state is a
  diff from its parent's and is materialized on demand.
- Network snapshots group: 75.12 → 27.76 GiB. Model total: 90.93 → **41.41 GiB**, so the gate needs
  **47.33 GiB**. Devnet model: 4,323,240,949 → 3,601,500,638 B.
- Every retained state and every state RPC answer equals the full-state reference's, with the same
  rebuild and restore work.
- Validation of 380 transfers on the near-cap state: 672 ms, against 655 ms before S3a (the diff alone
  takes 12.5 ms).
- What remains: the 10 transient working copies (S3b, an executor change), the full head and side states
  (S5), and the per-block metadata (S4).

**S4 status (2026-10-24, done; measured by `derive()`):** proposals/C1-STORAGE-DESIGN.md "S4 status" and
evidence/c1-s4/.
- Old canonical entries keep only what fork choice, contexts and evidence read at their depth. Their header,
  ParentResult and nonce ranges are restored exactly from the journal and history.redb when needed.
- Network history group: 4.28 → 0.83 GiB. Model total: 41.41 → **37.96 GiB** (gate 43.38 GiB). Devnet
  model: 3,322,680,105 B.
- What remains: the transient working copies and the full states (S3b and S5, an executor change).

**E1 status (2026-10-24, done; measured by `derive()`):** proposals/C1-STORAGE-DESIGN.md "E1 status" and
evidence/c1-e1/. This is the first step of S5 + S3b, and item 4 of C-2.
- The state root is updated from the keys a block changed, using a trie of the parent's state that is
  shared between versions.
- A parent's trie is used only when its root equals the parent's committed stateRoot.
- Every root in the tests is compared with the whole-state computation.
- **A13:** a new trie group of 2.23 GiB at the network profile. Model 37.96 → **40.20 GiB** (gate 45.94
  GiB). Devnet model: 3,486,909,657 B.
- **Correction:** the design estimated 0.4–0.5 GiB. The bound counts two tries of 1.07 GiB each, at the
  worst-case branch count. In E4 the nodes move to state.redb.

---

## 2. Priority 2: block validation time

### Facts (JOURNAL-GROWTH-AND-REPLAY.md §3.2, A13-DERIVATION.md §4a)

| case | measured | budget |
|---|---|---|
| a valid block of 380 transfers on a near-cap devnet state | **≈ 10.2 s** | devnet T_blk = 4 s |
| the worst one-transaction block (MODEXP) | ≈ 1.0 s | devnet t_exec = 1 s (network t_exec = 3 s, governance.md) |
| the whole state root recomputed every block | ≈ 0.23 s devnet; ≈ 14.4 s at the network caps (synthetic) | — |
| network projection per transaction | ≈ 7.9 s (a linear projection) | network T_blk = 60 s |

The causes are implementation, not protocol. For every USER transaction, `native::execute_for_profile`:
- runs `code_caps::validate_snapshot` over the whole state, keccak-ing every runtime (`code_len` →
  `hash_slow`);
- `build_engine` clones the whole state (`with_db(snapshot.clone())`);
- `NativeContext::from_snapshot` clones it again (`pay::pre_transaction`).

Each create also rescans the state (`code_caps::current_total`). Every block clones the state and
rebuilds the world root.

### C-2 [CONFORMANCE]: result-equivalent execution

1. `validate_snapshot` runs once per phase, not per transaction. The phase-level checks stay where they
   are (`apply.rs:221` and `:293`). The invariant that makes this result-equivalent: within a phase, code
   enters only through CREATE, whose `create_return` hook enforces B_code_max and CODE_CAP. Under Cancun
   (EIP-6780) code leaves only within its creating transaction.
2. Verified code length per code hash, kept with the code, and a running logical-code total, instead of a
   keccak of every runtime on every scan.
3. A read-only view of the pre-transaction state for the engine and for PAY, instead of two full clones
   per transaction. revm writes into its journaled state, not into the database, during `transact()`.
4. An incremental state root and O(dirty) block checks: cached account storage roots and trie nodes,
   proven equal to the full `world_root` on every block (JOURNAL-GROWTH §7 option 7).

**Acceptance tests:**
1. Every existing vector and suite passes unchanged: lp4 169, pocold, caps, the consensus vectors and
   e2e_devnet.
2. On the same near-cap devnet state (`jm_tx_state_term`), the 380-transfer block validates in under
   T_blk/2 = 2 s, with ≤ 1 s as the aim.
3. The per-transaction cost no longer grows with the state size (a measured series).
4. An equivalence test between the incremental and the full root, and between running and scanned
   totals, on every block of the suites.
5. Mutation controls: a stale cache, a skipped dirty account, a wrong running total.

**Status (2026-10-24):**
- Items 1–3 landed in `49b2bc2` and `2ce03c1`. Measured against `3a52ae6` on the same states
  ([evidence/c2-execution](../evidence/c2-execution/README.md)), at the near-cap state:
  - 380 transfers: 22,553 ms → 655 ms;
  - 247 CREATE2 creates: 4,862 ms → 562 ms;
  - an empty child: 777 ms → 465 ms.
- Acceptance test 2 (< 2 s, aim ≤ 1 s) is met for that block, at a load average of 2–3 on 4 CPUs. It
  will be re-measured unloaded with the final code.
- Item 4 (the state root, most of the empty child's 465 ms) waits for C-1; see §6.
- **Item 4 landed as C-1 E1 (2026-10-24, evidence/c1-e1/).** At the near-cap state, with the S3a code as the
  baseline:
  - an empty child: 491 → **262 ms**;
  - 380 transfers: 672 → **349 ms**;
  - a CREATE2-heavy child: 554 → **290 ms**;
  - the root alone: 314 ms → 3.6 ms, plus 12.9 ms to list the changed keys.

- **E2 (2026-10-24, evidence/c1-e2/)** reads the per-block whole-state checks from a state summary. At the
  near-cap state: an empty child 262 → **62 ms**; 380 transfers 349 → **174 ms**; CREATE2-heavy 290 →
  **122 ms**. What remains is mostly the state copies (E3).
- **E3.1–E3.3 (2026-10-25, evidence/c1-e3/)** run the block, production, the pool and RPC on overlays over
  the parent. At the near-cap state: an empty child 62 → **8.5 ms**; 380 transfers 174 → **55 ms**;
  CREATE2-heavy 122 → **17 ms**. One whole-state copy is left per block (the merge at the end); E4
  removes it (a child state is its root).
  Acceptance test 4 (incremental against full root on every block of the suites) passes: the tests build
  with lp4's `root-check` feature. A parent whose trie cannot be derived (more than 64 diffs from the head,
  or a side block deeper than side_max) costs what it did before E1.

### D-2 [NORMATIVE / profile]: the per-gas worst case (MODEXP)

C-2 does not change the cost of a MODEXP block, which is pure precompile work.

| option | consensus | genesisHash | compatibility |
|---|---|---|---|
| **(a) keep the Cancun gas schedule; raise the devnet's local t_exec from 1 s to 2 s**, if the R14/R19 gate admits it with the derived timings | none: a local timing value. But it is the governance.md P11 / D-A02-PROFILE value, so it is a profile-text change that needs your approval | none | none |
| (a2) keep the gas schedule; lower the devnet GAS_LIMIT from 8M to 6M (worst MODEXP block ≈ 0.72 s) | devnet CP only | a new **devnet** genesisHash; no network change | the devnet diverges from the network gas limit |
| (b) reprice MODEXP for small moduli, as later Ethereum proposals do | gas schedule change: every vector, a hard fork | none for the spec bytes, but every existing chain changes | departs from Cancun |
| (c) cap precompile input sizes | a new validity rule | as (b) | departs from Cancun |

**Recommendation: (a).** The network profile already has t_exec = 3 s, so it covers the ≈ 1.0 s case.
Decide after C-2 is re-measured. If the gate refuses 2 s, take (a2). Use (b) only if the worst block
still exceeds the devnet budget after C-2.

**Acceptance tests:** admission passes with the new devnet timing; the R14/R19 calibration on a MODEXP
block stays ≤ t_exec; the vectors are unchanged.

### D-7 [NORMATIVE / local pool policy]: bounding the pool's work per new head (review R3-6)

**Problem:**
- Every new head re-executes every pooled transaction (`rebuild_pool`) on the consensus thread.
- The pool is bounded by count (POOL_MAX_TXS = 4096) and bytes (32 MiB), not by gas. So 4,096 pooled
  MODEXP transactions near GAS_LIMIT cost about 68 min per new head on an idle host (the reviewer
  measured 1.39 s per pooled transaction under load).
- C-2 removed the per-submission scan and clone, but not the EVM work itself.
- network.md does not specify the pool. Which transactions it accepts is RPC-visible (an
  eth_sendRawTransaction error), so a bound is a policy choice that needs your approval.

| option | effect | consensus | genesisHash | compatibility | resources |
|---|---|---|---|---|---|
| **(a) a pool gas bound**: Σ gas_limit of pooled transactions ≤ POOL_GAS_MAX = 4·GAS_LIMIT; a submission beyond it is −32016 {pool full} | the per-head work is at most about 4 blocks of execution | none | none | a wallet sees −32016 earlier under load (an existing code) | per-head re-execution bounded by about 4·t_exec (devnet ≈ 4 s, network ≈ 12 s), down from about 68 min |
| (b) incremental revalidation: on a new head, re-execute only the pooled transactions of senders the block touched, then rebuild the pending state lazily | none for validity; the pool's cap meter must be recomputed, which needs care | none | none | none | depends on the block; no bound in the worst case |
| (c) both | (a)'s bound, (b)'s typical cost | none | none | as (a) | as (a), lower typically |
| (d) keep as is | — | — | — | — | about 68 min per head in the worst case |

**Owner's decision (2026-10-24):** a local work bound is approved in principle. The value 4·GAS_LIMIT is
not. The owner asked for a design that keeps revalidation from blocking the consensus thread, with a clear
bound on its hold, measurements under load, and then a recommended value.

**Design, implemented** ([evidence/d7-pool](../evidence/d7-pool/README.md)):
- Revalidation after a new head is queued in pool order and runs in slices of POOL_SLICE = 50 ms, with
  one event between two slices.
- **The consensus thread is held for at most POOL_SLICE plus one transaction** (≤ GAS_LIMIT, bounded by
  t_exec).
- An RPC submission during the backlog waits at most its own compute deadline, then gets −32021 busy
  (nothing pooled).
- Templates still see every transaction.
- The sliced result equals the whole rebuild (a test with mutation controls).

**Measured** (48 near-GAS_LIMIT BLAKE2F transactions):
- revalidating at once held the thread for **7.97 s** idle and **7.93 s** under load;
- sliced, the longest hold was **189 ms** idle and **214 ms** under load (one transaction).

**Recommended value of the gas bound** (it limits the work and staleness per head, not the hold):
- **POOL_GAS_MAX = max(1, ⌊T_blk / (2·t_exec)⌋) · GAS_LIMIT**, which gives 2·GAS_LIMIT on the devnet
  and 10·GAS_LIMIT on the network.
- It is derived from approved profile values, so that worst-case revalidation ends within half a block
  interval.
- Implemented as `Limits::pool_gas_max`, **off by default until you approve it**.

**Owner's decision (2026-10-25): approved at 4·GAS_LIMIT** as the local default bound on the pooled
transactions' summed declared gas limits, configurable within the resource limits; pool policy only.
Implemented (A13-DERIVATION §2d):
- `POOL_GAS_BLOCKS` = 4, so G = 32,000,000 at both profiles. `Limits::pool_gas_max` or `pocold run
  --pool-gas-max` sets another value. One below GAS_LIMIT is refused at open, and A13 counts the pool at G,
  so one the budget cannot hold is refused too.
- Admission refuses past G with −32016 "pool full (gas)" before executing. A duplicate returns its hash.
- There is no replacement: a same-nonce transaction fails on the pending state and counts nothing.
- Removal happens only at a new head, which re-executes and recounts.
- After a reorganization the abandoned blocks' transactions come first, then the pool, each within every
  bound. One that no longer fits is dropped.
- A returned transaction admitted again is offered to every peer (local relay policy). A peer with half its
  queue in use does not get it and is kept.
  - Found by e2e_integrated: a node that had left the active set during a long partition (W_act = 32;
    consensus.md §2) was not drawn after the heal, so the transactions it alone held were never included.
  - The membership behaviour is the specified one and is unchanged.
- The counters are checked against the contents (`Node::pool_audit`) through 160 mixed steps.
- Under `Node::run`, with the pool at its bound, a second block is imported while revalidation is still
  pending. The gas bound bounds no time; the slice plus one measured transaction does.

**Acceptance tests** (all pass):
- `sliced_pool_revalidation_equals_the_whole_rebuild`;
- `a_submission_during_a_long_backlog_is_busy_at_its_deadline_and_not_pooled`;
- `the_pool_gas_bound_refuses_past_it_and_is_recounted_on_a_new_head`;
- the measurement `d7_pool_revalidation_hold_under_a_heavy_pool`;
- the mutants in `mutants.log`.

---

## 3. Priority 3: sustainability of the caps (proposals/CAPS-SUSTAINABILITY.md)

### Facts

- TX_CAP (10^7), TXBYTES_CAP (2^33) and LOG_CAP (2^30) are cumulative per branch.
- On the network profile each can be exhausted before H_END, organically or by an attacker:
  - organically, at averages of 1.29 tx/s, 66 KB of transactions per block or 8.3 KB of logs per block;
  - by an attacker, in 23–37 days, for under 10^-5 coins at the idle base fee.
- After exhaustion, no user transaction can be included on that branch, or every logging transaction
  halts. The chain keeps producing system-only blocks, but rewards and refunded bonds can no longer be
  spent.
- genesisHash:
  - TX_CAP and TXBYTES_CAP are CP fields, so changing them changes genesisHash.
  - LOG_CAP is a constant outside the encoding, so the hash is unchanged.
  - Two nodes with different LOG_CAP therefore split silently, because Hello has no version.
  - This was verified with an independent keccak/RLP model and a Rust test.
- **Already done ([CONFORMANCE], implemented):**
  - the builder skips what no longer fits (execution.md:83);
  - templates that break the caps are not locked (consensus.md:101);
  - the caps-test build (validation.md:2065).

### D-3 [NORMATIVE]: the caps regime

| option | consensus | genesisHash | storage (network) | liveness |
|---|---|---|---|---|
| O1 keep as specified | none | none | 19.65 GiB logical | permanent freeze of user activity is reachable |
| **O2 unreachable by construction (N1–N3)**: gates R27–R29 with LOG_CAP 2^32, TX_CAP 5·10^7 and TXBYTES_CAP 24 GiB | three gate predicates; LOG_CAP is a new constant | **changes** (TX caps are CP); no network chain exists | ≈ 43 GiB logical (+23) | no cumulative freeze |
| O3 a linear allowance (N4) | a new validity/halt rule | changes only if BURST is in CP | unchanged | a griefable rate limit instead of a cliff |

**Recommendation: O2 on the network profile, and keep the approved devnet caps.**
- O2 adds no consensus mechanism.
- Its memory side depends on C-1 S1 (history off the heap); without S1 the devnet cannot even take the
  larger caps.

**Acceptance tests:**
1. Gate tests for R27–R29: the boundary passes and one below fails.
2. New genesis vectors from the independent model (`caps_model.py`), replayed by `caps_identity.rs`.
3. A test that exhaustion is unreachable at H_END under O2.
4. The caps-test build still reaches exhaustion.

### D-4 [NORMATIVE]: the interface items around the caps

| id | proposal | recommendation |
|---|---|---|
| N5 | the pool answers −32016 {cap} for a transaction that cannot fit the head branch's remaining room, and evicts such transactions when a new head leaves them no room. This also ends the "pool squatting" near the byte boundary | approve |
| N6 | `pocol_getCaps` → {txCount, txBytes, logBytes, txCap, txBytesCap, logCap} of the head branch. The method is listed in network.md:333, so serving it is conformance, but its fields are not specified; pocold answers −32601 today | approve the field list |
| N7 | `eth_call` and `eth_estimateGas` report an A03 halt instead of success | approve |
| N8 | state the scope in consensus.md:170: "the branch's counts up to the parent plus this block" (what the code does) | approve |

**Acceptance tests:** an RPC test for each; a pool eviction test across a reorg; the existing caps
tests unchanged.

### D-5 [NORMATIVE]: the identity of LOG_CAP

| option | meaning |
|---|---|
| (a) nothing | rely on there being no live chain; any later change of LOG_CAP is a silent split |
| **(b) a version in ForkSchedule and in Hello** | LOG_CAP (and any constant outside the CP) is tied to an activeVersion that peers compare at Hello (network.md) |
| (c) LOG_CAP into the CP | a CP schema version 2 (52 → 53 fields): GSV1 and every vector change |

**Recommendation: (b).** It is the smallest change that makes a constant change detectable, and it
leaves GSV1 alone.

**Acceptance test:** two nodes with different activeVersion refuse each other at Hello with a reported
reason, and identical nodes connect as before.

### D-3 / D-5: what the owner asked for before deciding (2026-10-24)

The owner asked for three things before approving D-3 and D-5. The full analysis is
[D3-D5-HEND-ANALYSIS.md](D3-D5-HEND-ANALYSIS.md), made by an independent agent; I checked its
arithmetic and its spec citations myself.

**1. Behaviour at and after H_END (spec and code agree on the end):**
- No valid block has h > H_END, and no valid block has the final block F as its parent.
- On the network profile, height ends the chain first. Time ends it first only if blocks average more
  than 80 s.
- In F, user transactions still run. F mints no subsidy and settles every remaining reward pot. All
  bonds are refunded before F.
- After F, balances are frozen for good. The spec keeps the data for 30 days and then takes a signed
  snapshot.

**2. Proofs of non-exhaustion.** Exact arithmetic, reproduced by script; 0 mismatches against the
study's vectors:

| cap | O2 value | worst use until H_END | margin |
|---|---|---|---|
| LOG_CAP | 2^32 | H_END × LOG_BLOCK_MAX = 4,246,732,800 | 1.12% |
| TXBYTES_CAP | 24 GiB | H_END × the largest body = 25,478,323,200 | 1.13% |
| TX_CAP | 5·10^7 | H_END × ⌊GAS_LIMIT / 21,000⌋ = 49,248,000 | 1.50% |

- So under O2 each cap is **unreachable** before H_END, a proof, not an assumption about load.
- The TX_CAP line rests on one lemma: every included transaction adds at least 21,000 gas.
  - It holds for Cancun refunds. It was checked over 2,178 storage-write sequences, which is not a formal
    proof.
  - Without the lemma, a block holds up to 475 transactions, and 5·10^7 would not be enough.
- With the current network values all three caps are reachable. N1 alone fixes only LOG_CAP. On the
  devnet, LOG_CAP is already unreachable.

**Two corrections to D-3 before approval:**
1. The study's R28/R29 gates would refuse to boot the devnet and caps/64 caps, which the same
   recommendation keeps. **Scope R28/R29 to the network profile.**
2. **State the 21,000-gas lemma in R28**, or raise TX_CAP to at least 61,560,000.

**3. Release and compatibility plan against a silent split (proposals, not approved):**
- Make R27 a boot gate on every profile except caps-test.
- Add protocolVersion to Hello. network.md:6 already requires it, and pocold does not send it.
- Add a digest of the consensus constants outside the CP to Hello, and refuse a peer on a mismatch.
- Ship these in the same release as N1, and no later than the first binary that boots the network
  profile.
- After that, every change to a constant takes a new protocolVersion with a ForkSchedule entry.
- No re-genesis of existing devnet data is needed: 2^30 → 2^32 changes no block there, since
  H_END × 32,768 ≤ both values.
- A mixed-version two-node test must refuse at handshake, never fork.

**Spec gaps the analysis found in the code.** These are conformance items, queued in §6 and not yet
implemented:
- after F the pool still accepts transactions; network.md:152 says −32016 {final: true};
- `pocol_getClosing`, `pocol_getCaps`, `pocol_getPot` and `pocol_getSettlement` are not served;
- Hello lacks protocolVersion (network.md:6). This changes the wire format, so it goes with the release
  plan above.

**Recommendation:** approve D-3 with the two corrections and D-5 (b) together with the release plan.
Nothing is implemented until you approve.

---

## 4. Priority 4: journal growth and recovery cost (JOURNAL-GROWTH-AND-REPLAY.md)

### Facts

- The canonical journal's worst case is about 11.3 GiB on the network profile.
- A re-fetched branch is journaled again; there is no deduplication.
- Open replays from genesis, with cost a·H + b·H²/2: about 66–69 min at the network H_END, and up to
  about 7.3 h on the devnet on a near-cap state.
- A deep reorg rebuilds the fork-point state from genesis, twice.
- Since `462fffc`, an adopting reorg restores up to R_keep abandoned blocks' transactions (derived:
  up to about 34 min for the worst kind).
- All three journal obligations stay OPEN.

### C-3 [CONFORMANCE] (after C-1 S3, with which it shares the diffs)

1. **Reverse diffs for the last R_keep blocks** (storage.md:70; consensus.md:230–232). A deep-fork
   rebuild scales with depth instead of height, and the double genesis rebuild disappears. This is S3 of
   C-1.
2. **An incremental canonical index**, so `rebuild_canonical` no longer walks head → genesis on every
   head change. This removes the b·H² term.
3. **Keep the fork-point state while a side branch depends on it** (option 6).
4. **Headers-first sync** (network.md D38, already approved). A losing branch is not journaled in full.
5. **Dedupe markers** for re-fetched blocks, so a re-fetch is not journaled again.

**Acceptance tests:**
1. Open time on the devnet scales with the blocks since the last checkpoint or diff window, not with H²,
   on a measured series.
2. A deep reorg at depth R_keep needs no genesis rebuild (measured).
3. Replay equals live after every record; the D-REORG-REDB crash tests pass.
4. Re-fetching a branch adds no journal bytes.
5. One mutation control per item.

### D-6 [SCOPE / NORMATIVE boundary]: bounding open time

| option | meaning | status |
|---|---|---|
| **(a) local state checkpoints** every 2048 blocks (the node's own verified state, written to disk; open replays from the latest one) | local storage only; nothing is trusted from outside | no approval needed; listed so that you know |
| (b) compacting block history below finality | storage.md:70 keeps blocks "until the end" | **[NORMATIVE]**: contradicts the retention text |
| (c) signed snapshots from others | storage.md mentions a signed snapshot after ts_F+30 days only | **[NORMATIVE]** before then |

**Recommendation: (a) only, and confirm that block history is never compacted (b is rejected).**

**Owner's decision (2026-10-24): (a) approved.** Local state checkpoints, keeping the block history the spec
requires, with integrity checks of every checkpoint and recovery from a partial write. The design is in
[C1-STORAGE-DESIGN.md](C1-STORAGE-DESIGN.md) (D-6). It is built on S5, since a checkpoint is then a committed
store version, not a new serialization of an in-memory state.

---

## 5. Other pending decisions

### 5a. The six consensus proposals ([README](README.md); each is implemented provisionally with its recommended alternative)

| id | question | recommendation |
|---|---|---|
| LOG-ACCT-01 | which bytes a log is charged | (c): 36 + 32·topics + data, ≤ 546 logs per block. The only reading consistent with every quoted text (LG2 and the receipt bound) |
| D-A03-DIRTY | what counts as "dirty", and when a halt makes the block invalid | A: a net change counts; a halt whose residue would exceed the limit makes the block invalid, and the builder skips it |
| D-A03-CAPS | what SLOT_CAP/ACCT_CAP count | 2: user state only; SYS_SLOT_MAX checked after the system phases (R18 makes it unreachable for an admitted profile) |
| D-BODY-MAX | encoded body or item sum | (a): the encoded body, the literal reading and what decode does |
| D-A03-SYSUSER | what the user-context SYS allowance counts | (a): storage writes to the programs only; balance credits do not count |
| LOG-CAP-AGGREGATE | the scope of LOG_CAP | chain: cumulative per branch, the bound A13 relies on. With D-3 O2 it becomes unreachable on the network |

**Consensus effect:** each fixes validity, receipts or roots where the text is silent. Approving the
recommended alternatives keeps the current code, with no change to genesisHash. Choosing another
alternative changes the code, the vectors and the validity of blocks produced on test devnets. No chain
exists outside local test runs.

**Acceptance tests:** the model vectors `proposals/vectors/*.json` (independent Python) and their Rust
replays pass. They already do for the recommended alternatives.

### 5b. RpcGuard P1–P4 ([RPCGUARD-R2-PROPOSALS.md](RPCGUARD-R2-PROPOSALS.md); local service policy, no consensus effect)

| id | question | recommendation |
|---|---|---|
| P1 | the request rate across connections | **C**: HTTP keep-alive with the bucket per connection. This is the model the text assumes (persistent connections, an aggregate ≤ RPC_CONN_MAX·rate) |
| P2 | CANCEL_MAX for precompile runs | **B**: eth_call/eth_estimateGas refuse, with −32020, a single precompile call whose gas exceeds a measured per-precompile cap. This is an explicit error, never a different result |
| P3 | an effectful method that finishes after its deadline | **A**: the deadline is checked before the effect; once the effect is applied, its result is answered |
| P4 | methods outside the classification table | **B**: add table lines for the methods pocold serves; −32601 before admission for every other method |

**Acceptance tests:** an RPC test for each chosen option, under the existing RpcGuard tests and mutation
controls.

---

## 6. What is being done now under the current authorization

**Done:**
- **C-2 items 1–3** (`49b2bc2`, `2ce03c1`). Measured in §2.
- **C-1 S2** (code counted once per lineage; §1).
- **consensus.md:101, "the peer is punished"** (R1-5). The drawn proposer's signed template for this
  node's parent that fails item 16, its evidence or its rebuild closes its sender's connection
  (`templatePenalties` in pocol_status). Test: `consensus::an_invalid_template_from_the_drawn_proposer_punishes_its_peer`.
- **Fixes for the review findings** (`6e99b19`, `af2f3a8`): R1-1, R1-2, R1-4, R2-1 to R2-5, R3-3, R5-1,
  and R4-1/R4-2 (the unit test and the e2e checks). Details are in §7.

**Next, in order (by dependency and by gain against risk):**
1. **C-1 S1.** Canonical history off the heap, reusing the D-REORG-REDB offload.
2. **C-1 S3 together with C-3 item 1** (diffs), then C-3 items 2–5, then C-1 S4–S5 if you choose D-1 (a).
3. **C-2 item 4, the incremental state root, moved after C-1.**
   - Caching account storage roots beside every retained state costs about 52 B per account. At the
     network's 10⁶ accounts and K_hist = 32 states, that is about 1.7 GiB, which the 6 GiB budget cannot
     take while the states are full copies.
   - It is therefore built on C-1's shared or on-disk state (S3/S5, the spec's persistent trie,
     storage.md:63–73), not before it.
   - Until then the root stays a full recomputation per block: 465 ms on the near-cap devnet state.
4. **Smaller conformance items:**
   - R1-3 (a TXBYTES test for received templates).
   - The R3 study corrections (R3-1, R3-2, R3-7, R3-8, R3-13), in a new note; the study itself stays
     unchanged.

Each lands with:
- a fail-before or measurement test;
- an equivalence test;
- a mutation control;
- the full suites;
- an evidence directory.

None changes validity, roots, receipts, genesisHash or RPC results. If one would, it stops and comes
back here as [NORMATIVE].

## 7. Independent reviews (requested 2026-10-24; done)

The five reviews are done. Each was made by a separate agent, given the code and the spec but not this
track's conclusions. The reports and the disposition of every finding rated MEDIUM or above are in
[evidence/reviews-2026-10-24](../evidence/reviews-2026-10-24/README.md).

| review | subject | findings ≥ MEDIUM | state |
|---|---|---|---|
| R1 | caps round 3b | R1-1, R1-2 | fixed (`6e99b19`) |
| R2 | RpcGuard round 3c, Windows socket code | R2-1 | fixed (`af2f3a8`); the LOWs R2-2 to R2-5 too |
| R3 | journal round 3c, `462fffc` | R3-2, R3-3 (HIGH); R3-1, R3-6, R3-7 | R3-2 and R3-7 by C-2; R3-3 fixed (`af2f3a8`); R3-1 a study correction; **R3-6 needs your decision (D-7)** |
| R4 | e2e round 4 | R4-1, R4-2 | fixed: unit test (`af2f3a8`) and e2e checks; base 42/42 twice, and the reviewer's three mutants each fail at the check aimed at them (evidence/reviews-2026-10-24/r4-fixes) |
| R5 | `462fffc` | R5-1 | fixed by C-2 |

No review found a deviation that needs a normative change, apart from R3-6 (D-7).

## 8. Tests to re-run on the final code (and why)

| test | last run | why it must be re-run |
|---|---|---|
| lp4 and pocold suites, release | the decision-package commit after `af2f3a8` (lp4 173/0, pocold 184/0) | every later change |
| debug runs (assert-release and pure debug, the consensus tests one at a time, spec depth included) | `3c3792e` | node.rs changed in round 3, in `462fffc`, in C-2 and in the replay restore (R3-3) |
| the ignored consensus tests (spec depth 2048/2049, D133 at 400, the A11 deep reorg) | `2ce03c1` | R3-3 changed replay; C-1 S1/S3 touch the same paths |
| D-REORG-REDB mutation controls (30) | `3342af3` | R3-3 changed the replay restore; `462fffc` the reorg restore; C-1 changes storage |
| RpcGuard mutation pass | `dd7d1ec` round 3c | R2-1 to R2-4 changed rpc.rs and the batch loop |
| caps.rs, caps_identity, log_cap, the caps-test build | `6e99b19` | C-2 item 4 will change block execution |
| e2e_devnet, e2e_site | e2e_devnet with the R4 checks: 42/42 on `d567ce1` (and twice on `af2f3a8`); e2e_site on `c959379` | every later code change; e2e_site has not run since round 3 |
| the full LP10 integration: extension build and 234 unit tests, real Chrome e2e, integrated Chrome | `3c3792e` | round-3 RPC changes (RpcGuard, −32020) and every later change |
| witnesses: 17 observations and 15 positive controls (scratch only) | `3c3792e` | round-3 and later code |
| RSS measurement | `3c3792e` | C-1 changes memory; S2 lowered the code term, so the measured RSS must be checked against the new model |
| journal measurements (validation time, reorg restore, open) | round 3c | C-2 and C-1 change the measured paths |
| R14/R19 admission calibration | `462fffc` | C-2 changes the timing |
| Windows CI | `d567ce1` (run 33, PASS; also runs 29–32 on `49b2bc2`, `c5fbd00`, `6e99b19`, `af2f3a8`) | runs on every push of the PR. A pass covers the repair-scope suites on Windows only; it is not acceptance |

A Windows pass covers the repair-scope suites on Windows only. It is not acceptance.
