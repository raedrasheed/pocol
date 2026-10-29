# Decision package (2026-10-24)

**Status: presented to the owner for decision.** Inclusion here does not approve an item; no normative text has been amended. Technical details and evidence: [DECISION-PACKAGE.md](DECISION-PACKAGE.md).

## Reading this package

Each item has one of three tags:

- **[CONFORMANCE]:** brings pocold closer to the approved specification.
  - No change to block validity, roots, receipts, genesisHash or RPC results.
  - Implemented now under current authorization, with an equivalence test and mutation control for each change.
  - You may stop or reorder it.
- **[NORMATIVE]:** changes protocol/consensus text, CP values, a constant, an RPC interface or node requirements. **Requires your approval** before implementation.
- **[SCOPE]:** no consensus effect, but a target/effort choice belonging to you.

## Priority 1: booting the network profile within 6 GiB

**Problem (measured with the code's own model):** the network profile needs about **150.8 GiB**, requiring a 172.4 GiB budget against 6 GiB available:

| Cause | Size |
|---|---|
| 45 full state copies | 75 GiB |
| Code counted separately in every copy | 55 GiB |
| Entire history in memory | 19 GiB |

Round 3 attributed the overrun to TXBYTES_CAP alone, substantially understating it.

The specification itself uses disk-backed storage (storage.md, D30): redb, a live trie, ≤ 32 historical versions and diffs for 2048 blocks. D30 explicitly states that 128 exceeds 6 GiB. The budget comes from the specification; pocold's architecture violates it.

**C-1 [CONFORMANCE]: five stages toward the specified design.**

**Correction (2026-10-24):** earlier figures here (≈ 136, 83, 14, 10 GiB) were not established results and are replaced below. Calculations start at measured post-S2 `derive()` items, 105.83 GiB, then subtract each stage's moved items. Added cache/diff/overlay memory remains a new, underived item, marked “+ new”. A stage's figure becomes a result only after implementation and measurement.

| Stage | Contents | Removed items (GiB, measured) | Resulting model |
|---|---|---|---:|
| S2 ✔ | Shared code between versions (done) | Code 54.69 → 9.69 | **105.83 (measured)** |
| S1 ✔ | Canonical history off heap (done) | History 19.19 → 4.28 | **90.93 (measured)** |
| S3a ✔ | Retained states as parent diffs (done) | Snapshots 75.12 → 27.76 | **41.41 (measured)** |
| S4 ✔ | Slim old block metadata; restore exactly from journal/history.redb (done) | History 4.28 → 0.83 | **37.96 (measured)** |
| E1 ✔ (S5 step 1) | Incremental state root: head trie in memory, other parent tries derived through diffs or built (done) | Adds trie item 2.23; moves to state.redb in E4 | **40.20 (measured)** |
| E2 ✔ (S5 step 2) | Whole-state checks read a summary updated from changed keys (done) | Adds small summary per entry | **40.20 (measured)** |
| E3 ✔ (S3b, E3.1–E3.3) | Executor, production, pool and RPC use parent overlays; S3a diff from block write set (done) | Snapshots 27.76 → 16.33 (transient copies 10 → 2, pool layer counted as full state, growth copy retained), code containers 1.45 → 0.81, history 0.84 → 0.78 | **28.08 (measured)**. Earlier ≈ 19.1 assumed all transient/growth copies disappear; that did not occur |
| E4 (S5, incorporating E3.4; D-6 is E4.4) | Disk trie is state: every retained state (head/window/side/genesis/checkpoint) is a root over shared state.redb nodes, read through bounded cache; block nodes, reverse diff and root committed before journal append | Projected: all full states 16.33, both tries 2.23, code containers 0.81 | ≈ 8.7 before E5 (four code sets remain) + new node/code cache, state.redb cache, current-block nodes |
| E5 | Store-backed code reads in rebuild/pool; smaller entry | Projected: code sets 5.63 | ≈ 3.08 + E4/E5 new items; target ≤ 5.25, so new items ≤ 2.17 |

**Recomputed after E3 (2026-10-25), using measured E3 items.** Earlier S5 projections (≈ 6.5/6.0 GiB) assumed S3b removed every transient/growth copy; they were not measured results. Measured E3 total: 28.08 GiB.

Items other than full states, code sets or tries total 3.08 GiB: diffs 0.46, execution 1.10, history 0.78, remainder 0.72. This is E4/E5's floor before new items.

Reaching it needs three local, non-normative changes:
- Move every full state off heap: head/trie to state.redb in E4; side, materialized and working states through store-backed overlays;
- Store-backed code reads in rebuild/pool instead of copies (E5);
- Bound the pool overlay by pool gas rather than block limits. D-7's local work bound does this; its value is unapproved, so the model still counts a full state.

These remain projections until `derive()` prints them.

No consensus/genesisHash effect. Local storage format changes; disk use grows toward specified inventory (~29.5 GiB).

**Acceptance tests:**
- `a13_model_at_the_network_profile` establishes boot within 6 GiB.
- All equivalence suites unchanged, including depths 2048/2049 and D133.
- Crash test for every new structure.
- RSS measurement on near-full network state.
- Mutation control per stage.

**D-1 [SCOPE]: target?**
- (a) Full S1–S5 conformance.
- (b) Stop at S3, raise memory requirement to about 16 GiB: **[NORMATIVE]**, changing node requirements.
- (c) Keep pocold devnet-only.

**Recommendation: (a), staged.** Start S2, then S1.

**S2 completed and measured with the same model:**
- Test proves state copies share code memory; independent execution creates its own copy.
- Code counted once for the state chain, once per side state, rebuild and pool.
- Network: code 54.69 → 9.69 GiB; total 150.83 → **105.83 GiB**; gate needs **120.95 GiB**, still far above 6.
- Devnet: total reduced by 94 MB.
- Large remaining items: 45 state copies (75 GiB, S3/S5) and in-memory history (19 GiB, S1).

**S1 completed and measured with the same model:**
- Canonical blocks older than K_hist retain only metadata in memory. Body/header read from journal; receipts/transaction index from `history.redb`.
- Network: history 19.19 → 4.28 GiB; total 105.83 → **90.93 GiB**; gate **103.92 GiB**. State copies (75 GiB, S3/S5) dominate.
- Devnet: total 5,487,223,178 → 4,323,240,949 bytes.
- RPC answers match pre-S1 tree byte for byte in acceptance scenario.
- Corrupt data is never served; store rebuilt from journal for deletion, truncation, random overwrite, byte flip and wrong content.
- Two new local, non-normative behaviors:
  - Opening runs full redb integrity check, about 1–1.4 s/GiB;
  - Runtime corruption makes history reads return −32603; node stops on next import and rebuilds at open.

**S3a completed and measured:**
- Full states remain only for head, genesis and side states within side_max. Other retained states are parent diffs, reconstructed on demand.
- Network snapshots 75.12 → 27.76 GiB; total 90.93 → **41.41 GiB**; gate **47.33 GiB**. Devnet total 3,601,500,638 bytes.
- Every retained state and state RPC answer matches the full-state reference, with identical rebuild work.
- Validating 380 transfers on near-full state: 672 ms versus 655 before S3a. Diff calculation alone: 12.5 ms.
- Remaining: ten transient copies (executor change S3b), full head/side states (S5), block metadata (S4).

**S4 completed and measured:**
- Old canonical entries retain only fork-choice/context/evidence needs at their depth. Header, result and nonce ranges restored exactly from journal/history.redb.
- Network history 4.28 → 0.83 GiB; total 41.41 → **37.96 GiB**; gate 43.38 GiB. Devnet 3,322,680,105 bytes.
- Remaining transient/full states: S3b/S5, executor changes.

**E1 completed and measured:** first S5/S3b step, C-2 item 4.
- State root updated from block-changed keys using a shared parent-state trie.
- Parent trie used only if root equals committed parent-header state root.
- Every test root compared with full calculation.
- Network adds 2.23 GiB trie item: total 37.96 → **40.20 GiB**, gate 45.94. Devnet 3,486,909,657 bytes.
- **Correction:** design estimated 0.4–0.5 GiB. Bound counts two 1.07 GiB tries at worst branch count. Nodes move to state.redb in E4.

## Priority 2: block validation time

**Problem:**
- Validating 380 transfers on near-full devnet state takes **≈ 10.2 s**, against T_blk = 4 s.
- MODEXP block ≈ 1.0 s, against devnet t_exec = 1 s (network 3 s).
- Full state root recalculated every block.

Implementation cause: each transaction scans all code with keccak and copies the whole state twice.

**C-2 [CONFORMANCE], identical results:**
1. Check code once per phase, not transaction.
2. Keep verified code length/running total instead of keccak per code per scan.
3. Read-only view instead of two full copies.
4. Incremental state root proven equal to full root.

**Acceptance:** all vectors/suites unchanged; 380-transfer block < 2 s, target ≤ 1 s; transaction cost independent of state size; incremental/full-root equivalence; mutation controls.

**D-2: MODEXP cost, a profile decision touching approved text.** C-2 does not change this cost. Options:
- (a) Keep Cancun gas table; raise local devnet t_exec 1 → 2 s if R14/R19 admits it. No consensus/genesisHash effect, but changes governance.md P11.
- (a2) Devnet GAS_LIMIT = 6M: new devnet-only genesisHash.
- (b) Reprice MODEXP: **[NORMATIVE]**, consensus change affecting all vectors.
- (c) Precompile input cap: **[NORMATIVE]**, new validity rule.

**Recommendation: (a), decided after C-2 remeasurement.** If gate rejects it, (a2).

**C-2 completed so far:** items 1–3 (`49b2bc2`, `2ce03c1`), measured against prior code on the same near-full state:
- 380 transfers: 22.6 → 0.66 s; acceptance < 2 s achieved at load 2–3 on 4 CPUs.
- CREATE2 block: 4.9 → 0.56 s.
- Empty block: 0.78 → 0.47 s; mostly root calculation, item 4 deferred until after C-1.

**Item 4 implemented in C-1 E1**, compared with S3a on the same state:
- Empty: 491 → **262 ms**;
- 380 transfers: 672 → **349 ms**;
- CREATE2: 554 → **290 ms**;
- Root alone: 314 → 3.6 ms, plus 12.9 ms identifying changed keys.

**Then E2:** whole-state checks read a state summary. Near-full state:
- Empty 262 → **62 ms**;
- 380 transfers 349 → **174 ms**;
- CREATE2 290 → **122 ms**.

Most remaining cost is state copying, removed by E3.

**Then E3.1–E3.3:** block/production/pool/RPC use parent overlays:
- Empty 62 → **8.5 ms**;
- 380 transfers 174 → **55 ms**;
- CREATE2 122 → **17 ms**.

One full final-merge copy remains per block; E4 removes it by making child state a root.

Incremental/full-root equivalence passed on every suite block. A parent whose trie cannot be derived—over 64 diffs from head or a side block deeper than side_max—retains pre-E1 cost.

Remeasure without load on final code.

**D-7 [NORMATIVE, local pool policy]: pool work per new head** (review R3-6).

Problem:
- Every new head reexecutes all pool transactions on consensus thread.
- Pool bounded by count 4096 and bytes, not gas. Filling it with MODEXP may cost about 68 minutes per head.
- network.md does not specify pool, but acceptance appears in RPC results; bound needs approval.

Options:
- (a) Pool gas: summed gas_limit ≤ 4·GAS_LIMIT; excess −32016 (existing code). Work/head ≤ about 4·t_exec (≈ 4 s devnet, ≈ 12 s network). No consensus/genesisHash effect.
- (b) Incremental revalidation for block senders only: identical results but no worst-case bound.
- (c) Both.
- (d) Unchanged.

**Your decision (2026-10-24):** approved a local work bound in principle, not value 4·GAS_LIMIT.

**Implemented design** (evidence/d7-pool):
- Revalidation after each new head in pool order, slices ≤ 50 ms, one event processed between slices.
- **Consensus thread held ≤ 50 ms plus one transaction**, whose gas ≤ GAS_LIMIT, bounded by t_exec.
- RPC submission during revalidation waits only its deadline, then −32021 busy without entering pool.
- Templates see every transaction.
- Test with mutation controls proves full-rebuild equivalence.

**Measurement, 48 heavy transactions:**
- Monolithic revalidation held thread 7.97 s unloaded, 7.93 s loaded.
- Sliced maximum hold 189 ms unloaded, 214 ms loaded: one transaction's time.

**Recommended gas bound** (bounds work/staleness per head, not hold duration):
- **POOL_GAS_MAX = max(1, ⌊T_blk/(2·t_exec)⌋)·GAS_LIMIT**: 2·GAS_LIMIT devnet, 10·GAS_LIMIT network.
- Derived from approved profile values, ensuring worst revalidation within half a block interval.
- Implemented but **disabled by default pending approval**.

## Priority 3: cap sustainability

**Problem:** network TX_CAP/TXBYTES_CAP/LOG_CAP can be exhausted before H_END:
- Organically at 20–34% utilization;
- Or attacked in 23–37 days at trivial cost.

After exhaustion, user transactions freeze on the branch; rewards/refunded bonds become unspendable.

GenesisHash:
- TX_CAP/TXBYTES_CAP changes alter it.
- LOG_CAP changes do not, but cause silent forks.
- Verified with independent model and Rust test.

Conformance implemented: builder skips what does not fit; invalid templates do not lock; caps-test build.

**D-3 [NORMATIVE]: caps regime.**
- O1: unchanged.
- **O2: exhaustion impossible by construction.** R27–R29, LOG_CAP = 2^32, TX_CAP = 5·10^7, TXBYTES_CAP = 24 GiB.
- O3: linear allowance.

**Recommendation: O2 for network, keep approved devnet caps.**
- Effects: new network genesisHash; no network chain exists. Logical storage +≈23 GiB. Memory depends on S1.
- Acceptance: boundary gate tests, new genesis vectors from independent model, nonexhaustion test at H_END.

**D-4 [NORMATIVE]: cap interfaces.**

| Item | Proposal | Recommendation |
|---|---|---|
| N5 | Pool −32016 {cap} for nonfitting transaction; evict after new head, ending pool occupation | Adopt |
| N6 | pocol_getCaps {txCount, txBytes, logBytes, txCap, txBytesCap, logCap}; method listed network.md:333 but fields undefined | Adopt list |
| N7 | eth_call/eth_estimateGas report A03 halt instead of success | Adopt |
| N8 | Explicit scope: branch through parent plus current block | Adopt |

**D-5 [NORMATIVE]: LOG_CAP identity.**
- (a) Nothing.
- **(b) Version in ForkSchedule and Hello, compared at connection.**
- (c) LOG_CAP in CP, changing GSV1/all vectors.

**Recommendation: (b).**

**Requested before approving D-3/D-5** (full independent-agent analysis in `D3-D5-HEND-ANALYSIS.md`; calculations/citations personally verified):

1. **At/after H_END:** specification and code agree.
   - No valid block above H_END or valid child of final F.
   - Network reaches height first unless average block time exceeds 80 s.
   - F executes user transactions, issues no subsidy, settles remaining rewards. All bonds refunded earlier.
   - After F, balances permanently freeze.
2. **O2 nonexhaustion proof:** exact scripted arithmetic, no disagreement with study vectors.
   - LOG_CAP 2^32: 1.12% margin.
   - TXBYTES_CAP 24 GiB: 1.13%.
   - TX_CAP 5·10^7: 1.50%, conditional on lemma that each included transaction adds ≥21,000 gas.
   - All three caps **unreachable** before H_END: proof, not load assumption.
3. **Two corrections before D-3 approval:**
   - Scope R28/R29 to network, otherwise they reject devnet boot.
   - State 21,000-gas lemma in R28, or raise TX_CAP to 61,560,000.
4. **Proposed anti-silent-fork release plan:**
   - R27 gate for every profile.
   - Add protocolVersion to Hello, already required network.md:6 but not sent.
   - Add consensus-constant digest; reject differing peers.
   - Ship with N1 in same release.
   - No new devnet genesis needed.
   - Mixed-version pair test must reject connection, not fork.
5. **Code conformance gaps, queued but unimplemented:**
   - Pool accepts transactions after F; specification requires −32016 {final}.
   - pocol_getClosing/getCaps/getPot/getSettlement not served.

**Recommendation:** D-3 with both corrections; D-5 (b) with release plan. Nothing implemented before approval.

## Priority 4: journal growth and recovery cost

**Problem:**
- Worst canonical journal ≈11.3 GiB on network.
- Refetch rewrites branch into journal.
- Opening reexecutes genesis at Θ(H²): about 66–69 minutes network, up to 7.3 hours near-full devnet.
- Deep reorg reconstructs from genesis twice.

**C-3 [CONFORMANCE], after S3 because they share diffs:**
1. Reverse diffs for last R_keep blocks (storage.md:70).
2. Incremental canonical index removing H².
3. Retain fork-point state.
4. Headers-first sync, approved D38.
5. Prevent rewritten refetched branches.

**Acceptance:** opening scales with blocks since checkpoint, not H²; depth-R_keep reorg without genesis rebuild; replay equals live state after every record; refetch adds no bytes.

**D-6: opening-time bound.**
- (a) Local state checkpoints every 2048 blocks: local, no approval required, stated for information.
- (b) Compact block history: **[NORMATIVE]**, conflicts with storage.md:70 retention “until the end”.
- (c) Other parties' signed snapshots: **[NORMATIVE]**, specified only after ts_F+30 days.

**Recommendation: (a) only, confirming block history is never compacted.**

## Other pending decisions

**Six consensus proposals [NORMATIVE]:** all provisionally implement recommended alternative; recommend adopting each. No genesisHash/code changes.

| Proposal | Recommended alternative |
|---|---|
| LOG-ACCT-01 | (c): 36 + 32·topics + data, ≤ 546 logs/block |
| D-A03-DIRTY | A: net change is dirty; a block whose halt effects exceed bound is invalid, builder skips transaction |
| D-A03-CAPS | 2: user state only, SYS_SLOT_MAX after system phases |
| D-BODY-MAX | (a): encoded body as decoder measures it |
| D-A03-SYSUSER | (a): program storage writes only |
| LOG-CAP-AGGREGATE | chain: cumulative per branch, unreachable on network with O2 |

**RpcGuard P1–P4:** local service policy, no consensus effect.

| Item | Question | Recommendation |
|---|---|---|
| P1 | Request rate across connections | C: HTTP keep-alive with per-connection token bucket, matching text |
| P2 | Precompile CANCEL_MAX | B: explicit −32020 above measured cap, not a different result |
| P3 | Effectful method completing after deadline | A: check before effect, return result afterward |
| P4 | Unlisted methods | B: table rows for served methods, early −32601 for others |

## Work now under authorization

**Completed:**
- C-2 items 1–3.
- C-1 S2 (shared code).
- “Peer penalized” (consensus.md:101, R1-5): invalid template from drawn proposer closes sender connection; counted in `templatePenalties`.
- R4-1/R4-2 E2E: baseline 42/42 twice; all three reviewer mutants failed their intended checks.
- Independent-review repairs: R1-1/R1-2/R1-4, R2-1–R2-5, R3-3, R5-1, R4-1 unit test; commits `6e99b19`, `af2f3a8`.

**Next, in order:**
1. C-1 S1.
2. S3 with C-3.
3. C-2 item 4 (incremental root), moved after C-1:
   - Storing storage roots alongside each retained state costs ≈1.7 GiB network; budget cannot bear it while states are full copies.
   - Build on C-1 shared/disk-backed state.
   - Until then root costs 465 ms devnet.
4. Small conformance items: received-template TXBYTES test (R1-3); journal-study corrections in a new file, preserving original.

Each change gets a pre-fix failure test, equivalence test, mutation control and full suites. If a change affects validity, stop and present it here as **[NORMATIVE]**.

## Independent reviews and reruns

**Five independent reviews completed.** Separate agents received code/specification without my conclusions. Reports/disposition for every medium-or-higher finding: `evidence/reviews-2026-10-24/README.md`:
- Caps R1: R1-1/R1-2 repaired.
- RpcGuard R2: R2-1 and low R2-2–R2-5 repaired.
- Journal R3:
  - High R3-3 fixed: opening now exactly matches live work; formerly 4 rebuilds/76 replays versus 0/2.
  - R3-2/R3-7 addressed by C-2.
  - R3-1 study correction.
  - **R3-6 requires your D-7 decision.**
- E2E R4: R4-1/R4-2 repaired with unit/E2E checks, mutation-verified.
- Commit `462fffc` R5: R5-1 addressed by C-2.

No review found a deviation needing normative change except R3-6.

**Rerun on final code:**
- Release suites; latest decision-package run lp4 173/0, pocold 184/0.
- R4 e2e_devnet passed 42/42 on `d567ce1` (twice earlier on `af2f3a8`); e2e_site not run since round 3.
- Full debug, last on `3c3792e`.
- RpcGuard mutation pass, because R2-1–R2-4 changed rpc.rs.
- Deferred tests.
- 30 D-REORG-REDB mutation controls, because rollback/open recovery changed (R3-3).
- Caps tests/caps-test build.
- Full LP10/Chrome integration, last on `3c3792e`.
- Witnesses.
- RSS versus post-S2 model.
- Journal measurements.
- R14/R19 calibration.
- Windows CI passed `d567ce1` (run 33), reruns on every push; success is not acceptance.

Windows success is not comprehensive acceptance. No full acceptance; PR #27 stays draft.

## Post-E4/E5 measurement (2026-10-25): store-mode A13 model

- **Completed:** E4.1–E4.5 and E5 parts 1–3 merged. Store mode opt-in (`Limits::state_store`), default off.
- **Computed network model**, `fc24969`:
  - Store **9.67 GiB**, requiring 11.05 with reserve;
  - Whole-state 27.92 GiB.
  - Calculated bound, not RAM measurement; actual RAM separately in `evidence/c1-e4/disk-impact-final.md`.
- **Store-bound progression:** 10.58 after E4.5 part 1; 10.24 after key preimages moved into leaves (E5 part 1); 9.07 after actual redb allocator accounting (part 2); 9.67 after measured state.redb factor 12 instead of history.redb's 8 (part 3).
- **File factor:**
  - Initial ≈18 caused by close: each version released in its own nondurable transaction, growing file 3–4×.
  - Close now leaves versions for next open, released in one durable transaction.
  - After fix: ≤6.07 at 14.9 MB stored; ≤11.31 at 1.5–2.8 MB.
  - Not established for adversarial-state-scale files (terabytes); remains open.
- **Required non-normative decision:** D-7 pool gas value.
  - Recommend 4·GAS_LIMIT, not yet approved.
  - Alternatives 2·GAS_LIMIT, or unlimited keeping 9.67 GiB model.
  - Projected ≈−2.7 GiB to ≈7.0.
- **Possible unimplemented local work:** gas-derived execution code-read bound (≈−1.0 GiB), requiring revm 19.7 load-path verification; large-scale file-factor measurement; rebuild/pool code sets (unfinished E5).
- **Owner modelling choice:** computational prefix-grinding assumption, paths ≤21 nodes. Batch 1.02 →≈0.33 GiB; not adopted without approval.
- **Conclusion:**
  - 6 GiB requires model ≤5.25.
  - D-7 value + gas-derived bound + path assumption project ≈5.3, at limit rather than below.
  - Store remains opt-in, network unbootable until `derive()` prints a fitting value.
- **Disk/time**, measured `5ab73d1`, devnet, 4 vCPU KVM, ext4/virtio; `evidence/c1-e4/disk-impact-final.md`. Small state 2,770 accounts/2.8 MB stored; larger 16,110/14.9 MB:
  - Import: store 1.8–2.4× slower small state because each commit fsyncs; larger state 1.23× slower account-creation blocks, roughly equal (0.94×) full blocks.
  - Reorg: store 1.5×/2.7× faster.
  - Checkpoint open: 1.8×/4.0× faster than whole-state replay; store replay from start slower because it refills store.
  - Files: state.redb 9.0/67.9 MB, plus history.redb 17.4/4.7 MB and journal 1.9/0.5 MB.
  - RSS after sync/reorg: 48.3 versus 42.0 MB; 112.7 versus 113.4 MB.
  - Variability: disk-sync time varies; store timings approximate within about 2×.

## After D-7 approval and code-loading audit (2026-10-25)

Previous section preserved as recorded. Details: A13-DERIVATION §2d; evidence `evidence/d7-bound/`.

- **D-7 implemented at approved value.**
  - Default 4 × GAS_LIMIT = 32,000,000 gas, configurable with `--pool-gas-max`.
  - Open rejects bounds below GAS_LIMIT or exceeding A13 budget.
  - **Admission:** excess transaction rejected before execution, −32016 “pool full (gas)”.
  - **Replacement:** none. Same-nonce second transaction fails against pending state, not counted.
  - **Removal:** only new head, with recount.
  - **After reorg:** abandoned-block transactions reinserted first within bound; nonfitting ones dropped.
  - **Relay reinserted transactions (local policy):** every abandoned-block transaction readmitted is offered to every peer. Peer with half-full queue receives none but stays connected.
    - Added because e2e_integrated failed 2/4 runs on 919a50c.
    - c left active set during long partition (W_act = 32, consensus.md §2), was not drawn after healing, retaining its seven reinserted transactions alone.
    - Membership conforms and is unchanged.
  - **Counters:** match contents after each of 160 mixed steps.
  - **Consensus:** second block imported during revalidation at pool capacity. Gas bound alone does not guarantee timing; slice 50 ms plus measured one-transaction duration bounds hold.
- **Audit findings:**
  - **Code read with every account:** store mode loads code with each account whether execution needs it or not.
  - **Loading before gas deduction:** revm loads account before charging; load may cost only 100 gas. Gas therefore does not bound code copies; execution-code item remains full CODE_CAP.
  - **Withdrawn projection:** previous “gas-derived bound ≈−1.0 GiB” **withdrawn**.
  - **Earlier omission:** transaction-created account retains every loaded slot, including zero slots and loads in out-of-gas frames. External witness confirmed, log in evidence. Block-overlay item corrected 0.004 →0.041 GiB.
- **Current network model:** store **7.61 GiB** (was 9.67); whole-state 25.24; approved ceiling 5.25, **overrun 2.36 GiB**.
- **Options numerically** (no calculated bound reduced merely to fit):

| Option | Type | Total |
|---|---|---:|
| No change | — | 7.61 |
| D-7 at 2·GAS_LIMIT | Your decision | 7.23 |
| key-path assumption, ≤21 nodes | Unapproved A13 assumption | 6.79 |
| Pool layer without code copies | Unimplemented local change | ≈7.0 |
| NODE_CACHE 256 →64 MiB | Local setting | ≈7.42 |
| File factor 12 →6 | Requires large-scale measurement | ≈6.70 |
| CODE_CAP 1 GiB →256 MiB | **Normative** | 6.43 |
| key-path + CODE_CAP 128 MiB | Assumption + normative | 5.33 |

- **No non-normative combination reaches 5.25 GiB.**
  - Remaining large items: redb allocator 1.81, execution code 1.125, execution 1.10, batch 1.02 GiB.
  - Recommend keep store opt-in, decide D-1 target with these figures.
- **Recalculated 2026-10-27** (A13-DERIVATION §2e, `892dca6`; preceding paragraphs preserved):
  - Decomposed 7.61 GiB item by item. Reduced four items only where code carries less or more exact accounting was proved/tested on real store. No normative/local-limit/acceptance-criterion changes.

| Reduction | Type | Network item (GiB) | Proof |
|---|---|---:|---|
| Pool layer keeps code hash, not copy (above option now implemented) | Local implementation | 0.618 →0.0003 | Equivalence on 2,000 random layers, store-mode node test |
| state.redb counts shared node once, by node kind | Proven accounting | Allocator 1.809 →0.279 | Content-addressed/refcounted store; real-store and 64-branch-path tests |
| Block batch by node kind | Proven accounting | 1.024 →0.916 | Same tests, hitting both worst cases exactly |
| Box chain entries inside table | Local implementation | Metadata 0.786 →0.524 | Type size; pinned devnet test total updated |

  - **Result:** network store **5.086 GiB, gate 5.81**, below ceiling 5.25/budget 6. Whole-state 24.98; store devnet 2.44.
    - *R3 update 2026-10-27:* **5.198 GiB, gate 5.94**, still fits. `5abbaf3` adds two stored-version handles; deferred checkpoint release `93a40d0` adds indebted checkpoint/release rows (A13-DERIVATION §3c).
  - **Two omissions found/fixed:** file bound omitted abandoned window until `settle` and rolling replay, now in `stored_versions`; 65-node/key count omitted the leaf displaced by a changed key.
  - **Cost:** pool reads code for a touched account from state.redb, like an untouched account.
  - **Not established:** acceptance, network-scale measured RSS, network boot (other gates still reject), large-file factor (`store-file-factor`).
  - key-path assumption unused; would reduce bound ≈0.7 GiB.
  - **Recommendation:** keep opt-in until near-network-cap state run/measured. Open decisions: [OPEN-DECISIONS-EN.md](OPEN-DECISIONS-EN.md).
- **Boot-mode R14/R19 (implemented later):**
  - Store-mode boot calibrates its own path: same blocks through temporary on-disk store, durable commit per block, gate takes maximum per phase.
  - Host: store 10–26 ms, memory 8–18 ms (15 runs), t_exec 1,000 ms.
  - **Your unapproved non-normative decision: calibration workload.**
    - GAS_LIMIT compute loop, almost no state writes.
    - State-heavy blocks exceed t_exec both modes: up to 1,068.5 ms whole-state, 1,045.1 ms store, one 4,841.8 ms outlier.
    - Including state-heavy load would reject host in both modes at t_exec = 1 s.
    - Alternatives: larger local t_exec (P11 option (a)), or retain `timing-not-a-bound`.
    - **Correction 2026-10-27, A13-DERIVATION §3b:** likely measured with test root checks enabled (`POCOL_ROOT_CHECK` defaults on in tests; runs omit its value); every block also ran full-state reference path.
      - With checks off, state-heavy blocks well below t_exec in both modes: ≤0.42 s near caps, except MODEXP.
      - Only even-modulus MODEXP exceeds one second (1.2–1.6 s), D-2.
      - Same measurement found duplicate store-mode block execution during import; fixed.
      - Decision now row 2 of [OPEN-DECISIONS-EN.md](OPEN-DECISIONS-EN.md), alongside D-2 row 1.
- **key-path assumption, unapproved:**
  - **Definition:** every state-trie key path ≤21 nodes, versus current 65.
  - **Rationale:** longer paths require keys sharing at least 80 hash-prefix bits; creating one costs order 2^76–2^80 hashes/path, not prohibited by protocol rule.
  - **Worst-case effect:** if false, path items undercount (batch, full-state paths, state.redb file); node exceeds calculated bound according to attacker spending.
  - **Conservative current alternative:** 65 nodes without assumption; costs 0.82 GiB more.
