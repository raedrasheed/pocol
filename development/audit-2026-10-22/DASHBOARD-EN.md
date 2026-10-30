# Audit repair dashboard (A01–A14) — Rounds 2 and 3

> This replaces the round-1 dashboard, which described every item as “closed with limitations”. That overstated the result.
> **No item is closed or owner-accepted.** Each has five independent status axes; details: [FINDING-STATUS.json](FINDING-STATUS.json).
> PR #27 is **draft**: verification is incomplete and consensus decisions are unapproved.
> **2026-10-24 update (round 3):** no full acceptance anywhere. Owner approved only D-A13-BUDGET, D-REORG-REDB and RpcGuard implementation. Windows repair-scope verification passed (run 22 on `c959379`, latest code commit). Critical A13 performance results are all **open** (§2).

> **2026-10-28 update (R4 on PR #2, claude/memory-r14-followup through `1bba829`; [evidence/r4/README.md](evidence/r4/README.md)):**
> - **RV10–RV12 seeded faults:** all killed by tests, including deterministic I/O injection; none classified equivalent. Tests exposed and fixed a defect: state.redb I/O failure was treated as an invalid block.
> - **Reorganization on healing:** two deterministic tests, one per direction, with negative controls (lighter-branch selection, no reorg, no sync) that fail them.
> - **Failure evidence:** automatically retained (node logs, inputs, seeds, Playwright trace).
>   - Established e2e_browser BR18 failure cause: navigation finished in a new frame while Playwright's click remained pending on the removed frame. Harness defect, not extension; fixed.
>   - Established both acceptance failures on `719da04`: both harness defects.
>   - **Not yet explained:** one debug-assertion devnet-admission refusal on `e028068`; reconnect_churn under severe load (also fails on `719da04`); one Windows R14/R19 refusal on `1074b47`.
>   - **Explained and fixed:** checkpoints unused at high-height reopen. Snapshot record-count decoder assumed 134 bytes/record, but writer uses 126; snapshot rejected when most records were minimum-sized (around 100,000 heights).
>   - **Fix verified at H_END** (`b375d4c`): reopened from checkpoint 129,024 in 13.3 s, versus ≈1,272 s replaying from genesis.
> - **Second independent review** ([evidence/r4/review-2/REVIEW.md](evidence/r4/review-2/REVIEW.md)):
>   - F-A: state.redb/history.redb open I/O failure now aborts opening and preserves file; only corrupt store quarantined. Test fails without fix; history.redb path untested.
>   - F-B: corrupt journal at a previously verified position is now local failure. Untested.
>   - F-C: side-block set is separate A13 item.
>   - All suites after fixes: 0 failures.
> - **Memory:**
>   - Computed network store model 5.199 GiB after F-C; model ×8/7 ≤6 GiB means model ≤5.25 GiB.
>   - Measured peaks 455–486 MB, and 601 MB at H_END with empty blocks; workloads do not reach largest model bounds.
>   - **Unmeasured:** worst execution/code set; RPC/peers at limits; depth-R_keep reorg.
> - **Journal through H_END:**
>   - Empty blocks: 2,011 bytes/record, 260.6 MB.
>   - Canonical-chain bound ≈11.3 GiB.
>   - Refetched side blocks unbounded (row-5 decision).
>   - Fixed two causes of import time growing with height.
> - **Checkpoint open:** 245 →207 s on identical input, without weakening integrity/recovery.
> - **Boot:** actual R21/R22/A13 values in BOOT-BLOCKERS; network profile does not boot. No limit raised or consensus rule changed.
> - **Decisions:** open table at proposals/DECISION-TABLE-2026-10-28. This round approves none.

> **2026-10-27 update (R3 on PR #2): Windows, network gates, MODEXP, target-scale storage, independent review.** No full acceptance; PR remains draft.
> - **Windows:**
>   - Corrected SendPacer test passed all 18 runs since `7624419`, latest `e453851`.
>   - Slow connection closed 1–10 ms after violation, within ENFORCE_SLACK 200 ms.
>   - Since `3705559`, test also measures client-side. On `7e565b7`, `d104891`, `b47fb85`, first-byte-to-close 0.508/0.509/0.506 s; on `bc38d41`, `fbe6062`, `71eb650`, `206eaa4`, 0.504 s. Bounds 0.45/1.0 s.
>   - Closure 1.0–1.25 s after request, so longer 20-second timeout did not cause success.
>   - `182d73a` failure cause remains unproved because send time was not logged.
>   - **Latest:** `e453851` passed everything, pocold 245/0, including corruption test that failed on `22a82c2`. That failure came from a real `93a40d0` defect, fixed below, not timing fluctuation.
> - **Why network profile does not boot** ([NETWORK-BOOT-GATES.md](NETWORK-BOOT-GATES.md)):
>   - No network boot code path (R22), no network genesis.
>   - Only R21 fails among R1–R26: 59.6 GB full snapshots versus 1.72 GB HIST_TRIE_BUDGET.
>   - Specification-scope conditions unmet; A13 fails whole-state, passes store (calculation, not measurement).
> - **Isolated node stall** (`563a0c3`): reproduced with full logs. Each miner searches only assigned nonce range; membership persists W_act blocks after last contribution. Specified behavior, not defect.
> - **MODEXP** (`b46658a`, `0c2ad75`, `b8fb7ab`, node code):
>   - Two revm-equivalent fast paths: one-word modulus; wide even q·2^t modulus.
>   - **Correction:** after `b46658a` I said D-2's premise no longer held. A 384-shape sweep then found another class, 1.46–1.56 s/block; `0c2ad75` fixed it.
>   - **Now:** all measured kinds below t_exec=1 s in both modes; at most 474 ms validation/510 ms import. Only ≈2× margin; shape grid is not maximum proof.
>   - **Correctness:** revm differential tests, all 25 seeded faults caught; independent review ≈1.5 million inputs, no disagreement/crash.
>   - **D-2** row 1 recommendation: **(0) no normative change**, with slowest-kind calibration row 2 gating slower hosts. **Both await your decision.**
> - **Network-cap storage** ([evidence/memory-r14/scale-d30dc71/](evidence/memory-r14/scale-d30dc71/README.md)):
>   - **Setup:** store node grown to 950,000 accounts/3,800,300 slots, 95% of network limits.
>   - **Computed:** profile A13 4.894 GiB, 5.005 after deferred release (`93a40d0`).
>   - **Measured:** peak memory 539 MB at open, 477 after repair 1; state.redb 2.0 GiB; stored versions one fifth of model.
>   - **Measurement limits:** synthetic workload not worst-case pool/code/RPC/peers. Does not close network bound (now 5.198 GiB), H_END journal or Windows.
>   - **Reorg:** depth-8 fork adopted in 1.53 s.
>   - **Restart, new defect fixed `b6d57e8`:** checkpoint open 783–801 s, including 547 s reading 33 full versions. Now each version reads differences from parent; every node remains read/checked and summary compared with seal. Open **262 s**.
>   - **Import stalls at checkpoints, new defect fixed `93a40d0`/`22a82c2`** ([deferred-release/](evidence/memory-r14/deferred-release/README.md)):
>     - **Before:** dropping oldest checkpoint took 10.3–13.7 s at this scale, including 8–11 s releasing uniquely held nodes on consensus thread. Import stalled up to 10.6 s every 1,024 records.
>     - **Fix, local D-6 mechanism, no rule change:** dropped checkpoint nodes recorded as debt in `release`; a write first finishes prior debt, so at most one checkpoint owed. `release_step` frees 256 references per disk-synchronized transaction between events/after pool slices; opening finishes remainder.
>     - **A13:** file term counts (KEPT+2) retained-version sets plus release row per indebted node. Network store **5.198 GiB**, gate 5.94, below 5.25 ceiling.
>     - **Measured `22a82c2`:** write 0.13–0.24 s; release 1,399 steps, mean 36 ms/max 50 (180 in next release); 70 empty-block imports between steps, median 120/max 155 ms.
>     - **Why fsync steps:** without it (`93a40d0`), each durable import wrote their pages: median 379/max 905 ms, state.redb 4.3–8.6 GB.
>     - **Remaining:** checkpoint open at this scale still 262–322 s, mostly post-checkpoint replay; `reset_pins` ≈12 s. Open only.
> - **Independent reviews** ([review-2026-10-27/](evidence/memory-r14/review-2026-10-27/)): no critical/high defect.
>   - MODEXP: three findings fixed `b8fb7ab`.
>   - Other repairs: four low findings fixed: unwrapped restore table (`5abbaf3`); client-side pacer test (`3705559`, PT5 survived old test); partition nodes after failure (`3705559`); handle count (`5abbaf3`).
>   - **D-6** ([D6-REVIEW.md](evidence/memory-r14/review-2026-10-27/D6-REVIEW.md)): diff-read correctness/counting intact: 11,360 random pairs, 30 seeds×400 random operations. All findings fixed `ad301e7`:
>     1. Medium: release-step I/O error marked store corrupt, rebuilding genesis. Now only inconsistent refcount row does; verified via child-process RLIMIT_FSIZE.
>     2. Low: pool backlog monopolized background steps; now alternate pool slices/releases.
>     3. Low: removed-ID list no longer retained; A13 had omitted it.
>     4. Informational: flag set after commit, FORMAT3, `stored_bytes` counts release rows.
> - **Windows-discovered defect** (run 40589342452, `22a82c2`, [RUN.md](evidence/windows-22a82c2/RUN.md)):
>   - Corruption test crashed opening store just marked corrupt by open-time release (`93a40d0`), rather than rebuilding.
>   - Deleted node chosen from HashSet; intermittent failure.
>   - Linux candidate sweep reproduced 14 crashes among first 363.
>   - Fixed `e453851`: quarantine/replay genesis. Deterministic test fails without fix, passes with it; all 587 candidates then reopened and matched genesis replay.
> - **D-7 loaded test** (`e6d07b2`, test-only): failed once alongside scale measurement. Reproduced cause: measurement request returned busy at QUEUE_WAIT as specified. After fix 6/6 under same load; D-7 mutations still caught.
> - **Acceptance, local nodes/RPC/experimental Chrome profile only:**
>   - **`e453851`** ([accept-e453851/](evidence/memory-r14/accept-e453851/README.md), all node changes): both modes devnet 44/44, site 21/21, browser 80/80; store integrated 20/20 twice; whole-state integrated 20/20 once, healing failure once;17 security witnesses not reproduced, all controls sound.
>   - **Healing failure was harness assumption, not node defect.** c had heavier work at reunion; a/b reorganized to c per D133. Test expected c to reorganize. Test-only `a01148e` waits for one head and losing side's reorg. Then integrated 5/5:3 whole-state,2 store ([integrated-rerun/](evidence/memory-r14/integrated-rerun/README.md)). c reorganized in all five, so opposite case not tested there.
>   - **`ad301e7`:** everything/witnesses passed except whole-state browser: Playwright BR18 click timed out after 44 checks on loaded host. Cause unproved; same steps 80/80 on `e453851`.
> - **Linux suites** `a01148e` (node code=`e453851`): lp4 211/0, pocold 246/0 ([release-a01148e.log](evidence/memory-r14/release-a01148e.log)).
> - **Seeded faults:** diff-read open 16,14 caught, remaining 2 output-equivalent for all inputs (checkpoint-diff/README.md); deferred release 16 on `dd85cc0`, all caught; then 28 on `ad301e7`,25 caught. Three documented gaps: `stored_bytes` measurement function; corruption marking on open storage error requires Node::open I/O injection; precommit flag has same result with KEPT=2.

> **Later 2026-10-24 update, decision package:**
> - C-2 items 1–3 (`49b2bc2`,`2ce03c1`) reduce near-full 380-transfer validation 22.6→0.66 s under identical load. MODEXP/root/journal obligations remain open.
> - Five independent reviews R1–R5 completed; medium+ findings fixed `6e99b19`/`af2f3a8`, except R3-6 pool gas bound, D-7 awaiting approval.
> - Network still not within 6 GiB. Model 28.08 after S2/S1/S3a/S4/E1/E2/E3.1–E3.3, gate 32.09, formerly 150.83 (C-1/D-1). E4 (disk trie state with D-6, incorporating E3.4)/E5 remain. Earlier ≈6 GiB post-S5 estimate withdrawn as unmeasured. Floor after removing full states/code sets/tries 3.08 GiB before new E4/E5 terms.
> - **S1 done**, under D-1(a): canonical history older than K_hist off heap; RPC byte-identical; corrupt derived store never served, rebuilt from journal. [evidence/c1-s1/](evidence/c1-s1/README.md).
> - **S3a done:** parent diffs reconstructed on demand;8,388 states compared with replaced full states; state RPC unchanged; network 90.93→41.41 GiB. [evidence/c1-s3a/](evidence/c1-s3a/README.md).
> - **S4 done:** slim old entries restored exactly from journal/history.redb; model 41.41→37.96 GiB;9/9 mutants killed. [evidence/c1-s4/](evidence/c1-s4/README.md).
> - **E1 done**, also C-2 item 4: incremental shared trie only used if root equals committed parent root; every test root compared with full computation. Near-full empty 491→262 ms,380 transfers 672→349 ms; trie term adds model to 40.20 GiB. [evidence/c1-e1/](evidence/c1-e1/README.md).
> - **E2 done:** whole-state checks from summary; empty 262→62 ms;11/12 mutants killed. [evidence/c1-e2/](evidence/c1-e2/README.md).
> - **E3.1–E3.3 done:** executor/production/selection/pool/RPC parent overlays, S3a diff from block write set. Every test block also applied to full copy, field-by-field comparison. Near-full empty 62→8.5 ms,380 transfers 174→55 ms. Transient copies 10→2 plus pool layer; model 28.08 GiB.18/19 mutants killed, remaining equivalent. [evidence/c1-e3/](evidence/c1-e3/README.md).
> - Details: [proposals/DECISION-PACKAGE-EN.md](proposals/DECISION-PACKAGE-EN.md).

> **Later 2026-10-25 update: D-7, code-load audit, store-mode E2E:**
> - **D-7 approved/implemented:** summed declared pool gas 4×GAS_LIMIT configurable within resources; admission/replacement/removal/reinsertion/counters/RPC tested;18/18 mutants killed, none equivalent.
>   - **Post-integrated repair:**2/4 failures on 919a50c left c's reinserted transactions only on c. c left active set during partition (W_act 32), not drawn after healing, specified behavior. Local relay fix offers reinserted transactions without dropping peers. Tests/mutations/reruns in evidence/d7-bound.6/7 mutants killed; R5 send instead of offer not detected deterministically on loopback, not equivalent.
>   - **8584283 acceptance:** pocold 231 passed/0 failed/22 deferred; both modes devnet 43/43, site 21/21, integrated 20/20 in 3/3 each; c offered all 7 reinserted transactions each time. A01–A14 not reproduced, controls sound. Windows run 66 passed.
>   - **Chrome:** two harness races, not node: navigation destroys frame before answer; receipt read once across short fork. Fixed d939d1a/0833b6c; latter 80/80 in 3/3 per mode.
> - **revm 19.7 audit:** store loads code with every loaded account before gas charge, retaining full CODE_CAP execution term; earlier−1.0 GiB projection withdrawn. Created account retains every loaded slot; local witness confirmed, block/pool overlay terms corrected.
> - **Network A13:** store 7.61 GiB, whole-state 25.24; ceiling 5.25, no non-normative option reaches it. Numeric options in decision package.
>   - **2026-10-27 `892dca6`:** four proven reductions without consensus/acceptance changes: pool without code copies; state.redb shared nodes counted once/by kind; block batch by kind; boxed chain entries. Network store**5.086 GiB**, gate**5.81**, whole-state 24.98. Calculated bound, not acceptance; store opt-in (A13 §2e).
>   - **2026-10-27 `8d80354`–`073923f`:**
>     - Both-mode E2E `8d80354` (evidence/memory-r14/accept-8d80354/): devnet 44/44, site 21/21, browser 80/80.
>     - Integrated: both store runs 20/20; first whole-state run failed 1/20 due harness race: reinserted v2 included after healing between Chrome reads and canonical-version read. Fixed `073923f`. Store rerun exposed another assumption: longer branch means heavier; D133 uses work. Fixed `92f6a80`/`2c40a9f`, then 6/6 both modes.
>     - Windows pacer failed once `182d73a`, timeout measured from request not first sent byte. Harness fixed `6a1e480`, Windows pending.
>     - Security A01/A02/A03/A04/A05/A10/A11 not reproduced, all controls pass.
>     - Mutants 24/25 killed; P6 proved equivalent: appending block never crosses a full-version bound and product cannot overflow.
>     - MODEXP/worst-load calibration remain open; successful boot calibration does not close them.
> - **Store selectable** `--state-store`, default unchanged. All store E2E on `9bd9bc0`: devnet 43, site 21, Chrome 80, integrated 19. Default integrated nonce race after healing fixed `919a50c`; reruns in evidence.
> - **Measurements, not guarantees:** loaded RSS13 MB default/15.7 store; state.redb churn stable 67.9 MB/factor≤7.95. R14/R19 initially excluded store commit/fsync, calibration open.
>   - Update: store boot now times actual path, including temporary disk store/durable block commits; gate takes maximum. Host store 10–26 ms, memory 8–18 ms,15 runs, t_exec 1,000 ms.
>   - **Your decision:** compute-only calibration writes little state; state-heavy blocks exceed t_exec both modes (up to 1,068.5 ms), so including them rejects both at 1 s.
> - **Witnesses/Windows:** A01–A14 all NOT_REPRODUCED, controls OK; Windows run 64 passed.
> - [evidence/d7-bound/](evidence/d7-bound/README.md).

> **2026-10-25 update: E4/E5/D-6:**
> - **E4.1–E4.5 done:** every retained state a pinned state.redb trie version read in place; opt-in Limits::state_store, default off. Store node matches whole-state per block/receipt/query, live/post-reorg/reopen, including specified 2048/2049 depth.
> - **D-6 done, E4.4:** local sealed journal-bound checkpoints; open resumes latest verified point without truncating journal/history.redb. Matches full replay field-by-field after kill, seven interruption steps 22 cases,17 corruption kinds; unverified state never used.
> - **E5 parts 1–3:** shared empty state; key preimages in leaves; actual redb allocator accounting; measured file factor 12; close leaves versions for next open.
> - **Computed model, not RAM:** store 9.67 GiB/gate 11.05; whole-state 27.92. Network still cannot boot within 6. Actual disk/time/RSS [disk-impact-final.md](evidence/c1-e4/disk-impact-final.md).
> - **Required D-7 value:** recommendation 4·GAS_LIMIT, projected−2.7 GiB.
> - **fc24969 acceptance:** Linux lp4 193/0, pocold 221/0, devnet 42/42, site 20/20, Chrome 79/79 and 18/18, extension 234/234; Windows 61 all passed. **Unrun:** debug, witnesses, D-REORG-REDB/RpcGuard mutations, loaded RSS, R14/R19; none counted passed.
> - **fc24969 mutants:**30/30 killed, none equivalent:21 E4.4, C7/C10 E4.3,7 E5 ([mutants-final.log](evidence/c1-e4/mutants-final.log)).
> - **Deep reorg tests (`--ignored`):**2048/2049 adoption, D133 depth 400, A11 passed.
> - [evidence/c1-e4/](evidence/c1-e4/README.md).

## Reference commits

| Role | Commit |
|---|---|
| Audited snapshot | `090eae4` |
| PR #27 head | Latest on `claude/audit-repair-a01-a14` (docs/evidence only after `462fffc`; tree equivalence in EVIDENCE-RECONCILIATION.md) |
| Latest code-changing commit (prototypes/ and .github/) | `462fffc`: two PR review threads fixed—A11 reorg path and R14/R19 calibration, each tested. Before it `c959379` (test files only); latest source change `dd7d1ec`, merged `2b1a571`. Before round 3: `3c3792e` |
| Round 3 suite commits | `2b1a571` (pocold/lp4/caps-test), `c959379` (devnet/site/consensus) |
| Full real-Chrome integration | `3c3792e`, not rerun on round 3 code |
| Witnesses | `3c3792e`, not rerun on round 3 code |
| Windows | `462fffc` (run 26 on `43e7e0f`, success); `c959379` (22); earlier `1cc96c4` (21), `2b1a571` (19), `25bec25` (16), `29fd44a` (9) |

After latest code commit, only documentation/evidence changed. Tree equivalence and commit roles: [EVIDENCE-RECONCILIATION.md](EVIDENCE-RECONCILIATION.md). Before round 3, table listed `687ac38` code, `c19dc8a` integration, `ec09120` witnesses; `3c3792e` results preserved below.

## 1. Implemented and verified (Linux)

**Automated PR #27 review threads (2026-10-24, `462fffc`, evidence/pr27-review-threads/):**
- **Side-block pruning:** already fixed, no change required.
- **Transaction return after deep reorg, A11:**
  - Defect: traversal stopped after 2048 steps; combined branches >2048 lost oldest abandoned-block transactions. Possible since D-REORG-REDB permits depth 2048 adoption.
  - Fix: traverse to common ancestor, reading bodies one block at a time from journal.
  - New test fails before fix, passes after.
- **R14/R19 calibration:**
  - Defect: samples became empty blocks after about 24 blocks.
  - Fix: pin base fee; measurement rejects any block not consuming GAS_LIMIT.
  - New test fails before fix, passes after.
- **462fffc results:** lp4 169/0, pocold 171/0, deferred 3/3, devnet 38/38; Windows 26 passed. Run excludes deep-reorg test because `#[ignore]`.

| Item | Implemented | Verified | Note |
|---|---|---|---|
| A01 body binding | Yes | Yes, Linux | Regression and positive-controlled witness |
| A02 ParamGate | Yes | Partial | §2 |
| A03 log/state limits | Yes, recommended alternative provisionally | Yes, Linux; independent vectors | Awaits consensus decisions, §3 |
| A04 journal framing | Yes | Linux/Windows CI | v2 and store_journal 12/12 pass Windows; actual power loss untested |
| A05 P2P/RPC limits | Owner-approved RpcGuard, partial where RPCGUARD.md says | Yes, mutation controls each round | §2 |
| A06 signing eligibility/send states | Yes | Linux/real Chrome | signed → transport queued → request started → broadcast |
| A07–A12, A14 | Yes | Linux | Positive-controlled witnesses |
| A13 resource derivation | Partial | Partial | Critical performance/journal obligations open, §2 |
| CAPS-01 builder/cumulative caps (found during repair, not audit) | Yes, execution.md:83 conformance | Linux/Windows CI;83/84 mutants killed | N5 pool rule proposed, decision-blocked |

“Yes” verification means Linux, plus Windows repair scope since round 3 (lp1-node, lp4-execution, pocold, extension build/unit). Windows check does not run E2E.

### Round 3 results (2026-10-23–2026-10-24)

Four tracks, each with its branch, independent reviews and repair rounds, merged into repair branch. Latest round governs conflicts. REPAIR-REPORT §3c.

- **Caps, CAPS-01 conformance:**
  - Builder ignored cumulative TX_CAP/TXBYTES_CAP. After exhaustion its own block rejected “16 caps”; each height waited D_att+Γ for fallback, stalling chain 74–78 s/block on test network (measured).
  - Fix matches execution.md:83: skip transactions exceeding parent-branch remaining allowance; treat received over-cap template as absence (consensus.md:101).
  - Tests 6/6 fail before repair; added exact byte boundary, both counter-branch directions,11 TX_CAP/12 TXBYTES_CAP boundary cases. Mutants 16/16, then 55/55, then 83/84 (R7 equivalent).
  - caps-test LOG_CAP=2^24 simnet-only with exhaustion test. HIST_TRIE_BUDGET unimplemented, item partial.
  - Independent genesisHash model: LOG_CAP not encoded, but changing it still hard-forks; differing nodes silently split.
  - CAPS-SUSTAINABILITY proposed/unapproved: every network cap reachable before H_END organically or cheaply by attacker. Recommend R27–R29 or linear allowance, N1–N8.
- **RpcGuard A05, owner-approved implementation:**
  - RPCGUARD.md item-by-item: enforced/equivalent with proof/partial/unenforced.
  - Enforced: QUEUE_WAIT every pre-admission wait; compute deadline from admission including encoding; response pools; SendPacer poll/WSAPoll with 50 ms backstop; SO_SNDBUF32 KiB and per-write bound.
  - Late effect-free responses become−32020 {deadline}; class overflow−32020 {resultBytes} on both lanes.
  - R26:25 fields present with actual values,6 equivalent;4 absent.
  - Mutation controls 21/20 detected,59/53,35/30; five survivors explained.
  - Unapproved P1 aggregate rate, P2 precompile CANCEL_MAX, P3 effect after deadline, P4 unlisted methods.
- **Journal/A13:** measurements only, source unchanged; three obligations open, critical performance in§2.
- **e2e_devnet round 4 `ad84942`:**
  - Partition winner simultaneously heavier/shorter, distinguishing most-work from longest-chain.
  - Unsolicited/orphan paths checked both directions.
  - Three consecutive clean 38/38;23 mutants,22 detected (16 intended check,6 early); tie-break survivor caught by unit test.
- **Merged-head Linux release integration:**
  - `2b1a571`: pocold 171/0,14 ignored; lp4 168/0; caps-test 11/0.
  - `c959379`: devnet 38/38, site 20/20, consensus 43/0.
  - **Not rerun on round 3:** full LP10/Chrome, witnesses, RSS, debug; last `3c3792e`.
- **Windows**, audit-r2-windows-verify.yml pull_request limited to target `claude/affectionate-pascal-rjhh2k`:
  - Runs 1–7 exposed/fixed four Windows defects: retention.rkeep directory fsync rejected; shutdown did not wake dropped P2P reader; Nagle/delayed ACK≈200 ms/RPC reply; kernel absorption of unread RPC responses (fixed SO_SNDBUF32 KiB and writes≤SEND_SOCK_BUF).
  - All checks pass runs 9 `29fd44a`,16 `25bec25` first round 3,19 `2b1a571`,21 `1cc96c4`.
  - Run 20 `49ae9fa`, same code as 19, failed timing test abandoned_queued_bodies_are_capped_and_released; `1cc96c4` changed test settings only,21 passed.
  - Excludes Chrome/Python E2E, thread counts, `#[ignore]` measurements. Rust-test change `c959379` passed run 22.
- **Round 3 corrections:** withdrew “wrong choice always fails” (E2E phase A), “deep reorg takes a few seconds” and “devnet open fits” (journal), “creation-heavy block is worst” (3b withdrawn3c), “round 3 not run on Windows” (run 16 did).

### Round 2 review F1–F9

All independent-review repairs implemented, each with test/mutation control:

| Finding | Repair | Verification |
|---|---|---|
| F1 | Set close flag under lock, preventing lost notification | Deterministic close between writer check/wait; mutant fails |
| F2 | One light-lane notification, `LIGHT_TURN = 8` requests per event | Deterministic/flood tests; both mutants fail |
| F3 | Execution-memory model by per-gas classes, including unpaid loading | General model test, private adversarial measurements |
| F4 | Exact-sized request body, queued-body cap, JSON-value-count limit, response pools | Tests/mutations fail |
| F5 | Experimental scope admits Simnet only | Test |
| F6 | RPC field list derived with RpcLimits | Removing field breaks compilation |
| F7 | Stop/join old miner before new; join finished threads before reusing slot | Deterministic counter; mutant fails |
| F8 | Tests independent of build speed;413 arrives before close; halt type checked in D-A03-DIRTY vectors | Tests |
| F9 | Document light-before-heavy priority | Documentation |

Second independent budget review `45712cd..b132a05` found 8 items: highest were SIDE_BLOCKS_MAX64 preventing longer-fork adoption (HIGH), replay failure on duplicate evicted-block journal record (HIGH), unbounded parsing on busy path (MEDIUM). All repaired/tested/mutation-controlled (A13§2a, evidence/MUTATIONS-R2.md).64 reverted to 256, then raised 320 through tested path.

### Final post-D-REORG-REDB verification (`3c3792e`)

- **Release success, specified depth/default limits:** heavier fork at 2048 adopted/persists restart,51.8 s;2049 neither adopted nor marked invalid; D133 depth 400 survives eviction/refetch,19 s. evidence/spec-depth-441ce73.log; src/ matches `3c3792e`.
- **All D-REORG-REDB acceptance within pocold 123 passed:** three-node TCP refetch with block-by-block RPC comparison; adoption crashes; competing-fork flood; lying-status peer cannot monopolize session.
- **30/30 mutations caught**, evidence/MUTATIONS-REDB.md. Earlier partial runs preserved, including two missed controls whose tests were repaired.
- **Integration `3c3792e`:** all except expected lp1 store_journal platform limitation: lp4 162, pocold 123, extension 234, network 17/17, site 20, real Chrome 79, integrated Chrome 18.
- **Recorded/classified `82ddb03` integration failure:** “c reorganizes” assumed a/b always heavier. c was heavier; a/b correctly reorganized to it, all converged per D133. “Every miner produced blocks” also fails old `687ac38`2/4. Both tests corrected to protocol properties; original logs retained.
- **Witnesses `3c3792e`:**17 NOT_REPRODUCED,15 controls OK.
- **RSS `3c3792e`:**11.98–12.30 MB/node,0.21–0.22% model; measurement, not bound.
- **Debug `3c3792e`, one test at a time:** release with assertions/overflow 44/44; pure debug 44/44, depth 2048/2049 took 1473 s, D133 at depth 400 took 589 s. Other debug binaries pass: lib 38, admission 3, net 20, store 12, site 6, decoder 1, secp 1. Earlier `687ac38`28/31 preserved as **partial**.

### Earlier verification (`687ac38`, historical)

Final-code roles in EVIDENCE-RECONCILIATION.md; prototypes tree `68ece272a3c7` identical in `687ac38`,`c19dc8a`,`ec09120`. Results separated:

- **Pass, Linux release integration `c19dc8a`:**lp4 162; pocold 110 including two R_keep adoption tests; extension build/234 units; three-node network 17; site 20; real Chrome 79; integrated Chrome 18. Tools cargo/rustc 1.97.0, lp1 1.58.1, pinned 1.99.1, Node 22.22.0.
- **Fail, expected platform limitation only:**11 Windows lp1 store_journal tests run/fail on Linux UnsupportedPlatform, matching `090eae4`;`failuresOnlyExpectedPlatformLimitation = true`. Not passes (§4).
- **Not run then:** Windows (§4), running since 2026-10-23, same store_journal tests pass there.
- **Final-code RSS:**12.26–12.32 MB/node,0.22% model, A13§2b; measurement, not ceiling/bound.
- **Debug final code, evidence/debug-final.log:**lp4 162 pass; pocold lib 37+admission 3+net 20=60 pass.
- **TIMEOUT/incomplete, pocold consensus debug:**single-thread run stopped at 5000 s before result;**unknown** outcome. Timeout cannot distinguish slow/stuck test; no individual result captured. Release success within 110 is separate evidence, not replacement. Individual-test completion runs underway with separate deadlines; until complete, binary's debug verification incomplete.
- **Final-code mutation controls** `687ac38`, evidence/mutations-final.json:**14/14** test mutants caught plus compile-time F6=15 controls. Each clean positive control passes and actual mutant test executes/fails; zero matched tests rejected. Classifier fix/M9/M2 redirection in evidence/MUTATIONS-R2.md.
- Witnesses 17 all NOT_REPRODUCED,15 positive controls OK,0 missing,0 false REPRODUCED (`ec09120`, evidence/witnesses-r2-final.log).A13 no operational witness; A02 contributes 3 observations.NOT_REPRODUCED plus control is observation, not acceptance.
- **Witness source:** I reconstructed from REPORT.md; original audit programs unavailable.
- Adversarial measurements/witness programs preserved in private review package delivered to owner, not public repo; hashes [PRIVATE-REVIEW-PACKAGE.md](evidence/PRIVATE-REVIEW-PACKAGE.md).

### 1b. D-REORG-REDB: heavier-branch adoption through R_keep=2048 (approved, implemented)

- **Changes:**storage/retention/sync only; D133, validity, roots, genesisHash unchanged; no local retention limit became validity rule.
- **Mechanism:**
  - Noncanonical blocks metadata-only; body read from journal as needed.
  - Canonical blocks remain complete in memory, RPC scope unchanged (storage.md:70 “until the end”).
  - Unprotected side-block cap R_keep 2048, one per height underD118.
  - Protected sync refetches evicted branch using locator containing canonical block atR_keep; protects up to 4096 blocks (network.md:27).
  - Adopted blocks fully restored by root-checked replay.
  - Replay matches live operation.
- **Metadata cap alone insufficient for multiple forks:**eviction starts lowest cumulative work, exactly deep-branch beginnings. Guarantee comes from protected refetch, not cap. ProofR2.3/reviewR2.8 [D-REORG-REDB-DESIGN-FREEZE.md](proposals/D-REORG-REDB-DESIGN-FREEZE.md).
- **Second review:**two HIGH session-liveness defects fixed—below-window block and duplicate counted as progress; post-depth 2048 fix for quadratic deep-side-branch rebuild; LOW findings fixed.
- **Final verification:**verification section below, REPAIR-REPORT§3b.

## 2. Technically incomplete (partial solution)

- **A02:**
  - Only “experimental devnet” can currently be admitted. Specification scope rejects all current profiles for explicit reasons:
    - Four RpcGuard fields absent (heavy/light SCRATCH, RPC_MEM_TOTAL, CANCEL_MAX); six equivalent rather than literal fields in admission notes (before round 3:“RpcGuard mechanisms absent”);
    - Synthetic calibration;
    - Sampled timing, not upper bound;
    - Assumed gas monotonicity;
    - Deferred LP3 items.
  - R14/R19 measured on host, fail closed under load: parallel debug consensus tests fail, all pass sequentially on quiet host.
- **A05:**
  - RpcGuard itemized in RPCGUARD.md, owner-approved implementation. Round 3: late effect-free responses−32020 {deadline}; pacer waits writability poll/WSAPoll,50 ms backstop rather than 1 ms retries; class overflow−32020 {resultBytes}. Round3c: light response bound on both lanes; conditional light entries get light deadline/bound.
  - Partial/unenforced: SCRATCH outside log handlers; precompile CANCEL_MAX(P2); RPC_MEM_TOTAL field; PacerTrace/chunked batch streaming; numeric log range before admission/pocol_getLogs anchors; unlisted-method ruleP4, aggregate reconnect rateP1, effectful-method P3 interpretation.
  - Running precompile not interrupted; eth_call/estimateGas EVM simulations poll deadline every 10^6 gas.
  - Round 3 code passed Windows 16 `25bec25`;3c tests passed 19/21.
  - Supersedes round 2 statement:“partially implemented; HEAVY/LIGHT permits, SCRATCH, per-connection bucket, class deadlines, SendPacer unimplemented; running request not interrupted.”
- **A13** ([A13-DERIVATION.md](A13-DERIVATION.md)§2a/§2b/§4):
  - **Commit-dated total:**round 3 `8039a09` onward through head **5,581,426,268 B (5.198 GiB)**; gate 6,378,772,878 B, unused 63,678,066 B; a13_bound_breakdown pins total/pass `2b1a571`. After RpcGuard round 2 `fa97594`:5,577,343,580 B(5.194 GiB), unused 68,343,995 (evidence/caps-r3/a13-fa97594.log).
  - **Final D-REORG-REDB `9ce5a7c`:**5,627,687,516 B(5.241 GiB). One-eighth reserve requires 6,431,642,876 of 6,442,450,944 B, unused 10,808,068. Was 5,623,469,297 on `687ac38`,6.62 GiB on `45712cd`.
  - **Not measurement or guaranteed ceiling.**Actual three-node loaded≈12 MB/node does not establish worst case.**No guaranteed process-memory ceiling.**
  - **Reserve:**one eighth=768 MiB outside model for allocator/fragmentation; margin, not proof.
  - **Open:**gas monotonicity, worst timing, allocator/fragmentation overhead, CPU costs;
    - **Critical round 3 performance, open, JOURNAL-GROWTH-AND-REPLAY.md:**
      - Valid 380-transfer block≈10.2 s validation near devnet caps, aboveT_blk 4 s. Each USER transaction validate_snapshot scans whole state/copies twice, every scan keccak-hashes each account's code; O(accounts+slots+code bytes).
      - Worst measured single-transaction MODEXP≈1.0 s, above devnet t_exec 1 s.
      - Full root each block adds≈0.23 s nearSLOT_CAP/ACCT_CAP.
      - Worst devnet H_END open≈7.3 hours.
      - All lower bounds on worst case; no fixes implemented for them.
    - RetentionStore/redb: pocold in-memory needs 5.198 GiB round 3/5.241 on `9ce5a7c`; spec targets≤750 MiB+192 MiB.
    - **Three journal obligations measured round 3 remain open:**
      - Append-only, no compaction; canonical max≈11.3 GiB network/0.22 devnet, grows every import/refetch including required evicted-block refetch. Local effect, no validity effect.
      - Opening reexecutes every record from genesis. Deep reorg belowK_hist reconstructs fork point **twice**: no reverse diffs, retain prunes canonical fork point belowhead−K_hist.96 s empty chain near devnet head, up to≈11 hours worst kind, in two stalls each exceedingD_att.
      - Open transient≈1.0×journal beyond permanent memory; peak 2.1–5.7×depending block kind; existing `open-replay-transient`.
    - Network profile not admitted: TXBYTES_CAP8 GiB alone exceeds 6 GiB; all network figures projections.
    - Journal§7 mitigations all proposed, none implemented. Withdrew round 2 “devnet open fits” and3b “creation-heavy block is worst per block”.
  - **Inspection only:**transient snapshot inventory, one EVM execution at a time, response-tree construction factor.
  - **Derived:**execution memory, documented revm 19.7 facts.
  - **R_keep depth:**implemented owner-approved D-REORG-REDB,§1b; D133/validity unchanged.
  - **Enforced:**thread stacks/inventory, response pools, request bodies, reserve.
- **A04:**actual power loss untested.

## 3. Decisions

### Owner-approved

**D-REORG-REDB (2026-10-23):**storage fix satisfyingR_keep, preservingD133/all validity rules; excludes six consensus proposals.

**RpcGuard implementation (2026-10-23):**owner instruction, translated:“Complete authorized work: RpcGuard and Windows verification.”Local service policyD87, no validity effect.

**D-7 (2026-10-25):**local summed declared pool-gas bound 4×GAS_LIMIT, configurable within resource bounds. Pool policy only, no block-validity/consensus effect. Implemented admission/replacement/removal/reinsertion/counters/RPC (A13§2d).

**D-A13-BUDGET:**option 1, reduce nonconsensus local limits, retain 6 GiB:

| Limit | Before → after |
|---|---|
| side_max | 8 →2 |
| SIDE_BLOCKS_MAX | Reduction 64 reverted, broke adoption; final 320 increases adoption depth,§1 |
| Transaction pool | 64 →32 MiB |
| Inbound peers | 32 →8 |
| EVENT_QUEUE | 256 →128 |
| NET_EVENT_QUEUE | 64 →32 |

Added response pools, two proved accounting corrections.K_hist, RPC_CONN_MAX, consensus parameters unchanged.A13§2a details/tradeoffs.

### Consensus proposals awaiting owner, unapproved

Each includes literal normative text, alternatives/recommendation, minimal example, root/receipt/gas/validity/compatibility effects, and independent-model vectors.

[proposals/](proposals/README.md):

| Proposal | Changes | Status |
|---|---|---|
| LOG-ACCT-01 | Which log halts, receipts/roots | Proposed |
| D-A03-DIRTY | Meaning of dirty; when halt invalidates block | Proposed |
| D-A03-CAPS | SLOT_CAP/ACCT_CAP scope; post-block SYS_SLOT_MAX | Proposed |
| D-BODY-MAX | Encoded body versus summed elements | Proposed |
| D-A03-SYSUSER | User-context system budget scope | Proposed |
| LOG-CAP-AGGREGATE | LOG_CAP scope | Proposed |

Decisive D-A03-DIRTY case: transaction halts because changes exceedACCT_DIRTY_BLOCK_MAX; halt's own sender-nonce/fee effects also exceed it; block invalid, builder skips transaction. Test now checks halt kind/changed-account count, not state alone.

Passing implementation tests approves no proposal. Historical M1 files were not amended.

### Round 3 proposals, unapproved

| Proposal | Changes | Status |
|---|---|---|
| CAPS-SUSTAINABILITY N1–N8 | R27–R29 (LOG_CAP2^32, TX_CAP5·10^7, TXBYTES_CAP24 GiB) or linear allowance; N5 pool; pocol_getCaps; eth_call A03 halts; scope text | Proposed |
| RPCGUARD P1–P4 | Aggregate connection rate, precompile CANCEL_MAX, effectful methods after deadline, unlisted methods | Proposed |
| Journal§7 | Headers-first fetch, dedup markers, compaction behind signed snapshot, reverse diffs, incremental root, per-gas replay-cost bound | Proposed, none implemented |

**Reviews prepared for owner, not decisions:**
- [INDEPENDENT-REVIEW-SIX-PROPOSALS.md](proposals/INDEPENDENT-REVIEW-SIX-PROPOSALS.md).
- [LOG-CAP-AGGREGATE-ALTERNATIVES.md](proposals/LOG-CAP-AGGREGATE-ALTERNATIVES.md): permanent-exhaustion alternatives with consensus/storage/compatibility effects.
  - **Recommendation:**retain chain interpretation, LOG_CAP2^32, gateH_END·LOG_BLOCK_MAX≤LOG_CAP; exhaustion impossible in every admitted profile.
  - **Alternatives:**remove consensus LOG_CAP(A4), or sliding window(A2) if lifetime 1 GiB mandatory.
  - LOG-CAP-AGGREGATE remains **unapproved**.
  - Another unresolved normative question: TX_CAP/TXBYTES_CAP cumulative, reachable before network H_END, invalidating blocks. Analyzed in round 3 [CAPS-SUSTAINABILITY.md](proposals/CAPS-SUSTAINABILITY.md), unapproved proposal.

## 4. Environment-only blockers

- **Windows, round 3 update:**repair-scope verification passed, limited coverage.
  - Round 2 unrun: dispatch 404 because manual workflow unavailable until on main, merge unauthorized.
  - Since 2026-10-23, pull_request trigger limited to target `claude/affectionate-pascal-rjhh2k`, optionB [WINDOWS-VERIFICATION.md](WINDOWS-VERIFICATION.md). Nothing merged main.
  - Pinned Rust 1.58.1/1.99.1, Node 22.22.0 hash verified, evidence full-SHA bound.
  - All checks pass 9/16/19/21,§1.
  - Uncovered: Chrome/Python E2E,/proc thread counts,`#[ignore]`measurements. These remain environment-blocked. Run 22 passed `c959379`, latest code.
- **lp1-node store_journal 11:**run/fail Linux UnsupportedPlatform; expected/matches audited snapshot, not pass.**Windows passed 12/12 in every recorded run.**
- **Windows journalv2:**tests pass. Skipping directory fsync after rename intentional, store::sync_dir/retention.rkeep. Actual power loss untested.
- **Thread counts/RSS outside Linux:**record “unmeasured”, not pass.

## 5. Not independently reviewed

- Two independent-agent reviews: F1–F9 `df63725`; budget `45712cd..b132a05`, eight findings all repaired as defects rather than changed expectations, each test/mutation-controlled.
- No external human audit. Shared GitHub account is not independent approval.
- Round 3: separate-agent reviews/repair cycles per track: caps 3, RpcGuard 3, journal 4, E2E3. Latest caps r3b, RpcGuard r3c, journal r3c, E2E fourth, plus `462fffc`, reviewed 2026-10-24 R1–R5, evidence/reviews-2026-10-24/.
- Owner has not reviewed consensus proposals.
- Windows script runs on GitHub since 2026-10-23, not locally: PowerShell absent here. Before round 3 this line said “never executed”.

## What did not happen

- No main merge.
- No deployment.
- No real funds.
- No billing changes.
- No full PoCol/LP4 acceptance or full acceptance of any finding.
- Witness/adversarial-measurement programs not public; delivered to owner through session files.
