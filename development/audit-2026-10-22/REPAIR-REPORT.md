# Audit repair report: A01–A14 and dependencies (rounds 2 and 3)

> **Status: not accepted, not closed** (updated 2026-10-24, round 3).
> - PR #27 is a draft. Nothing is merged into `main`.
> - No finding is closed. Acceptance belongs to the owner, and none is recorded. No full acceptance is
>   claimed anywhere in this report.
> - Approved by the owner: D-A13-BUDGET, D-REORG-REDB and the RpcGuard implementation. Still PROPOSED,
>   not approved: the six consensus proposals, CAPS-SUSTAINABILITY N1–N8, RPCGUARD P1–P4 and every
>   journal mitigation (§6).
> - A13 keeps open obligations (§5). The R_keep adoption blocker is addressed by D-REORG-REDB, a storage
>   repair the owner approved (D133 and validity unchanged; implementation and verification in §3b).
> - **New in round 3, critical and OPEN (A13, §3c):** a valid block of 380 transfers validates in about
>   10.2 s on a near-cap devnet state, over T_blk = 4 s; the worst measured one-transaction block
>   (MODEXP) takes about 1.0 s, over the devnet t_exec = 1 s; the state root is recomputed on every
>   block; the worst devnet open at H_END takes about 7.3 h. The three journal obligations (disk
>   growth, re-execution from genesis, the open-time memory transient) stay OPEN.
> - **Since then (§3d, decision package):**
>   - C-2 items 1–3 cut that 380-transfer block to 655 ms, against 22,553 ms for the earlier code, both
>     under the same load. The MODEXP block, the state root and the journal obligations are unchanged
>     and stay OPEN.
>   - The five independent reviews of the round-3 tracks are done, and their MEDIUM-and-above findings
>     are fixed, except R3-6 (decision D-7) (§3d).
>   - The network profile still does not boot within 6 GiB. Its model is 28.08 GiB (gate 32.09 GiB) after
>     C-1 S2, S1, S3a, S4, E1, E2 and E3.1–E3.3, against 150.83 GiB before them (C-1, D-1 approved as
>     option (a)). The near-cap validation of 380 transfers is now 55 ms, and an empty child 8.5 ms.
>     E4 (the trie on disk as the state, with D-6; E3.4 folded in) and E5 remain. The earlier ≈ 6 GiB projection for S5 was not a measured result
>     and has been withdrawn: the measured floor without whole states, code sets and tries is 3.08 GiB,
>     before the new terms E4 and E5 add (decision package §1).
> - Windows: the repair-scope CI passes on the last code commit `d567ce1` (run 33; runs 29–32 on the earlier
>   decision-package commits). Before them it passed on `462fffc` (run 26, PR head `43e7e0f`; the
>   review-thread fix, §3c); earlier runs 9, 16, 19, 21 and 22. The CI does not
>   cover the real-Chrome and Python e2e suites, the thread-count assertions or the `#[ignore]`
>   measurement tests (§5). Until 2026-10-23 this box said "Windows was not run"; that was true then.
>
> This report replaces the round-1 report. §7 lists what round 1 overstated; §3c lists what round 3
> corrected in its own earlier passes.

| Item | Value |
|---|---|
| Audited snapshot | `090eae4dc059d1a7df330d9fe674c873ac515564` (branch `claude/affectionate-pascal-rjhh2k`) |
| Repair branch, PR | `claude/audit-repair-a01-a14`, [PR #27](https://github.com/raedrasheed/Proof-of-Collaboration/pull/27) (draft) |
| Last implementation commit (prototypes/ and .github/) | `462fffc` (fixes for two PR #27 review threads, §3c: the A11 reorg walk in node.rs and the R14/R19 calibration fee in admission.rs, each with a test). Before it: `c959379` (round 3; its prototypes/ changes are test files only). Last source change: `dd7d1ec` (RpcGuard r3c), merged at `2b1a571`. `.github/` unchanged since `a03e089`. Before round 3: `3c3792e` (D-REORG-REDB final code; before it, `687ac38`) |
| Integration-tested commit | `462fffc`: pocold, lp4-execution, the ignored consensus tests and e2e_devnet (evidence/pr27-review-threads/). Round-3 suites: `2b1a571` (pocold, lp4-execution, caps-test build) and `c959379` (e2e_devnet, e2e_site, consensus), evidence/integration-r3/. The full LP10 integration (extension, real Chrome, integrated Chrome) last ran on `3c3792e` and was not re-run on the round-3 code |
| Witness-tested commit | `3c3792e` (not re-run on the round-3 code) |
| Real-Chrome-tested commit | `3c3792e` (in the integration; not re-run on the round-3 code) |
| Windows-tested commit | `462fffc` (CI run 26 on `43e7e0f`, PASS; the new deep-reorg test is `#[ignore]` and not run there); `c959379` (run 22); earlier `1cc96c4` (run 21), `2b1a571` (run 19), `25bec25` (run 16), `29fd44a` (run 9) |
| Mutation-controlled commits | `3342af3` for D-REORG-REDB (Rust sources equal to `3c3792e`); round 3: caps `e985ce1`, RpcGuard `dd7d1ec`, e2e test `ad84942` (each on its track branch) |
| PR head | latest commit on `claude/audit-repair-a01-a14` (docs and evidence only after `462fffc`; see EVIDENCE-RECONCILIATION.md) |
| Tree equivalence | [EVIDENCE-RECONCILIATION.md](EVIDENCE-RECONCILIATION.md) |
| Plan, decisions | [REPAIR-PLAN.md](REPAIR-PLAN.md) |
| Per-finding status (five axes) | [FINDING-STATUS.json](FINDING-STATUS.json) |
| A13 derivation, breakdown, assumptions | [A13-DERIVATION.md](A13-DERIVATION.md) |
| Consensus proposals | [proposals/](proposals/README.md) |
| Round-3 track documents | [RPCGUARD.md](RPCGUARD.md), [proposals/RPCGUARD-R2-PROPOSALS.md](proposals/RPCGUARD-R2-PROPOSALS.md), [proposals/CAPS-SUSTAINABILITY.md](proposals/CAPS-SUSTAINABILITY.md), [JOURNAL-GROWTH-AND-REPLAY.md](JOURNAL-GROWTH-AND-REPLAY.md), [WINDOWS-VERIFICATION.md](WINDOWS-VERIFICATION.md) |
| Arabic dashboard | [DASHBOARD-AR.md](DASHBOARD-AR.md) |

A witness printing NOT_REPRODUCED is an observation, not an acceptance pass. Test counts are not acceptance.

## 1. Status per finding

Each finding is recorded on five separate axes: implemented, verified, partially resolved, blocked and
accepted. The disposition is the weakest of them. "Windows CI" means the repair-scope workflow
(lp1-node, lp4-execution, pocold, extension build and unit tests); it does not run the e2e suites.

| Finding | Implemented | Verified (Linux; Windows CI) | Remaining | Disposition |
|---|---|---|---|---|
| A01 HIGH | yes | yes | D-BODY-MAX proposal | BLOCKED BY DECISION |
| A02 HIGH | yes | yes | Only an experimental-devnet admission is possible; timing is a sample, not a bound; R26: 4 RpcGuard fields absent, 6 equivalent | PARTIALLY RESOLVED |
| A03 HIGH | yes (recommended alternatives, provisional) | yes; independent vectors; caps-test LOG_CAP exhaustion | LOG-ACCT-01, D-A03-DIRTY, D-A03-CAPS, D-A03-SYSUSER, LOG-CAP-AGGREGATE; CAPS-SUSTAINABILITY N1 and N7 | BLOCKED BY DECISION |
| A04 HIGH | yes | yes; on Windows, journal v2 and lp1 `store_journal` 12/12 | Power loss untested | VERIFIED (Linux and Windows CI), BLOCKED BY ENVIRONMENT (Windows e2e and thread counts) |
| A05 HIGH | RpcGuard implemented (owner approved), mapped item by item; partial where RPCGUARD.md says so | yes; mutation controls per round | SCRATCH outside the log handlers, CANCEL_MAX for precompile runs, PacerTrace, chunked batches, the unlisted-method rule; P1–P4 proposed | PARTIALLY RESOLVED |
| A06 HIGH | yes | yes, real Chrome (Linux); extension unit tests on Windows | A request already initiated may be broadcast (reported, never "cancelled") | VERIFIED (Linux and Windows CI), BLOCKED BY ENVIRONMENT (Windows e2e and thread counts) |
| A07–A12, A14 | yes | yes | — | VERIFIED (Linux and Windows CI), BLOCKED BY ENVIRONMENT (Windows e2e and thread counts) |
| A13 HIGH | partly | partly | Open: gas monotonicity, worst-case timing, allocator overhead, CPU costs (including deep-reorg rebuild and restore), RetentionStore; round 3's critical performance findings (380-transfer block about 10.2 s > T_blk 4 s; MODEXP block about 1.0 s > t_exec 1 s; state root every block; devnet open up to about 7.3 h); the three journal obligations; no guaranteed memory ceiling. R_keep adoption: implemented by D-REORG-REDB (§3b) | PARTIALLY RESOLVED |
| CAPS-01 (found in the repair, not an audit finding) | yes (conformance, execution.md:83) | yes; 83 of 84 mutants killed, R7 equivalent | Pool rule for transactions that can never be included (N5, proposed) | BLOCKED BY DECISION |

Until round 3 the A04, A06 and A07–A12/A14 rows read "VERIFIED (Linux), BLOCKED BY ENVIRONMENT",
because Windows had not run.

## 2. What was repaired

Round 1 (commits listed in FINDING-STATUS.json): the repairs of A01–A14 and the dependency changes, as
described per finding in REPAIR-PLAN.md.

Round 2, on the owner's review:
- **Admission scope (D-A02-SCOPE).**
  - RpcGuard facts that pocold does not enforce are absent, never nominal, and enter the gate as 0.
  - Only an experimental-devnet admission is possible, and only for a Simnet profile on a Simnet node.
  - The specification scope refuses every current profile and lists the open conditions.
  - The RPC field inventory is tied to `RpcLimits` at compile time.
- **A13: each assumption enforced, derived, inspected or kept open (A13-DERIVATION §4).**
  - Thread stacks are reserved explicitly, and exiting threads are joined before reuse.
  - The EVM execution model was rebuilt from per-gas operation classes, which cover entries revm loads
    before charging their cold cost.
  - RPC request and answer memory is enforced: exact-size bodies, a queued-body cap, a JSON value cap
    and response pools.
- **D-A13-BUDGET (owner approved option 1).** At 45712cd the corrected model put the default devnet at
  6.62 GiB, over the 6 GiB budget. Local, non-consensus reductions, two proven accounting corrections
  and the response pools bring it to 5.20 GiB. The boot gate now keeps a 1/8 reserve.
- **RPC work deadline, light lane and tick coalescing (D-A05-R2)**, then the review fixes:
  - a single pending Wake;
  - LIGHT_TURN;
  - the 413 drain;
  - the peer-close lost wakeup;
  - one miner thread at a time.
- **Wallet submission states (D-A06-SUBMIT): signed, queued for transport, request initiated, broadcast.**
  - Epochs and eligibility are rechecked after the transport reservation and immediately before
    submission.
  - A queued submission is cancelled on lock or revocation.
  - A broadcast transaction is never claimed cancelled.
- **Consensus decision proposals** (proposals/): for each, the exact normative text, the alternatives,
  a minimal example, the effects, and independent expected-result vectors. All are **PROPOSED**; none is
  approved.
- **Windows scope workflow** prepared, with explicit toolchains and the evidence bound to a full SHA. It
  had not run when round 2 closed; since 2026-10-23 it runs on a pull_request trigger (§3c).

Round 3 (2026-10-23 to 2026-10-24), four tracks merged into the repair branch (§3c):
- **caps:** the builder's conformance fix (CAPS-01), the caps-test build, an independent genesisHash
  model and the CAPS-SUSTAINABILITY study (PROPOSED);
- **RpcGuard:** the owner-approved implementation in pocold, mapped item by item in RPCGUARD.md;
- **journal:** measurement only, no source change; the three journal obligations stay OPEN and the
  critical performance findings are new;
- **e2e:** the devnet test now distinguishes most work from longest chain, covers the orphan path,
  and is mutation-validated;
- **Windows:** four Windows defects found and fixed; the repair-scope CI passes.

## 3. Verification on the final implementation commit

Commands:
- `python3 development/audit-2026-10-22/tools/run_integration.py`: lp1-node with its accepted Rust 1.58.1,
  the extension build, and the LP10 suites (lp4 and pocold in release, extension unit, 3-node devnet,
  website publishing, real Chrome, Chrome with the integrated devnet).
- Debug runs of lp4-execution and pocold.
- The mutation controls (evidence/MUTATIONS-R2.md).
- The witness runner (programs in the private package).
- `tools/measure_rss.py`.

Earlier passing results are not reused as evidence for this code.

Results on the final code, separated by outcome (commit roles in EVIDENCE-RECONCILIATION.md; the
prototypes/ tree is `68ece272a3c7` at `687ac38`, `c19dc8a` and `ec09120` alike, so these apply to the
implementation commit):

- **PASS (Linux, release integration on `c19dc8a`, evidence/integration.json):** lp4-execution 162;
  pocold 110 (including the two R_keep adoption tests); extension build + 234 unit; 3-node devnet 17
  checks; website publishing 20 checks; real Chrome 79 checks; integrated Chrome 18 checks. Toolchains:
  cargo/rustc 1.97.0; lp1-node rustc 1.58.1; pinned rustc 1.99.1; node 22.22.0.
- **FAIL — expected platform limitation only:** lp1-node `store_journal`, 11 Windows-only tests
  executed on Linux and failed (UnsupportedPlatform), identical on `090eae4`; the integration records
  `failuresOnlyExpectedPlatformLimitation = true`. Not counted as a pass (§5). On Windows the same 11
  tests pass (lp1-store-journal 12 passed, round 3, §3c).
- **NOT RUN (at the time):** Windows verification (the scoped workflow returned 404 on dispatch until
  its file was on main; WINDOWS-VERIFICATION.md). It has run since 2026-10-23 (§3c).
- **Witnesses (`ec09120`, evidence/witnesses-r2-final.log):** 17 observations, all NOT_REPRODUCED; 15
  positive controls OK, 0 MISSING, 0 erroneous REPRODUCED. An observation with its control, not an
  acceptance pass.
- **Measured RSS (final code, evidence/rss-final.json):** 12.26–12.32 MB VmHWM per node under idle +
  bounded adversarial load, 0.22% of the model. Measured, not the model bound and not a ceiling
  (A13-DERIVATION §2b).
- **Debug build (final code, evidence/debug-final.log):** lp4-execution full debug suite 162 passed;
  pocold lib 37 + admission 3 + net 20 = 60 passed.
- **TIMEOUT / INCOMPLETE — pocold `consensus` binary in debug.** Its single-threaded debug run was
  stopped at the 5000s wall cap before printing a result, so its debug outcome is **unknown**: a
  timeout does not distinguish a slow test from a stalled one, and no per-test result was captured.
  The release integration's pass (pocold 110) is separate evidence and does not stand in for the debug
  run. A per-test completion run (each test alone, with its own timeout) is in progress; until it
  reports, debug verification of this binary is not complete.
- **Mutation controls re-run on the final code (`687ac38`, evidence/mutations-final.json, §MUTATIONS):**
  14 of 14 test-failure mutations caught, plus the F6 compile-time field-inventory catch (15 controls in
  all). Each has a positive control (the named test passes on clean code) and a mutant run that fails
  with the test actually executed; zero-matching-test runs are rejected. The classifier correction and
  the M9/M2 re-targeting (from the first FINAL attempt) are recorded in evidence/MUTATIONS-R2.md.

## 3b. D-REORG-REDB (owner-approved): implementation and verification on the final code

**What it is.** A storage repair for adopting a heavier branch at depth ≤ R_keep = 2048
(network.md:144). D133, block validity, state roots, receipts and genesisHash are unchanged, and no
retention limit is a validity rule. Design, proofs and resource bounds are in
proposals/D-REORG-REDB-DESIGN-FREEZE.md, Revision 2:
- R2.3 multi-branch adoption theorem;
- R2.4 RPC scope matched to storage.md:70;
- R2.5 replay equal to live;
- R2.6 bounds;
- R2.8 independent review.

**Mechanism.**
- Non-canonical blocks are offloaded to metadata, with their bytes in the journal. Canonical blocks stay
  whole, so no RPC scope changed.
- At most R_keep unprotected side blocks are kept (D118's one per height).
- Locator sync sessions over Status `totalWork` re-fetch evicted branches. Their non-canonical blocks
  are protected, up to 2·R_keep (network.md:27).
- Adopted blocks are restored by root-checked re-execution.
- Replay replays every record. A re-import gets a new importSeq (consensus.md:205), a block with a
  pending child record is protected, and the largest R_keep the directory has used is applied.
- New blocks at or below head − R_keep are refused before validation, as local retention
  (governance.md:200), never as invalid.
- `eth_call`/`eth_estimateGas` on a pruned state now answer −32017 like the state reads (consensus.md
  §10). This was a pre-existing inconsistency found during the RPC check.
- `pocol_status` gains additive `retention` and sync counters.

**Why a metadata cap ≥ R_keep is not enough for several branches.** The cap evicts the lowest
cumulative work first, which is exactly the start of a deep competing branch. The guarantee therefore
rests on protected re-fetch (R2.1, R2.3), and the tests below exercise it with a flood past the cap.

**Defects found and fixed during verification** (each with a test and a mutation control):
- the spec-depth run exposed quadratic rebuilds of deep side branches, because the K_hist prune also
  dropped side states (`9ce5a7c`);
- an independent agent review (R2.8) found two HIGH session-liveness bugs. A block removed at once and
  a duplicate both counted as progress, and a Hello started a session at once. It also found three LOW
  items: orphaned session blocks, replay copy and offset, and R_keep persistence (`441ce73`). All fixed;
- a mutation control showed the flood test did not make replay protection load-bearing (`f2477e7`);
- another showed retain()'s boundary was masked by the import-time refusal (`3342af3`).

**Results, separated by outcome:**
- **PASS, release, spec depth with the default limits** (`--ignored`, evidence/spec-depth-441ce73.log;
  same `src/` as 3c3792e):
  - `adoption_at_the_spec_depth_2048_and_not_at_2049`: a 611-block heavier branch forked 2048 deep is
    adopted and survives restart. Forked 2049 deep it is not adopted and not marked invalid. 51.8 s;
  - `d133_tie_break_at_depth_400_holds_through_eviction_and_refetch`: signed-before-fallback wins
    400 deep, the smaller importSeq wins between two signed branches, and X is evicted (784) then
    re-fetched and adopted. 19.0 s.
- **PASS, release, the D-REORG-REDB acceptance tests** (in pocold 123):
  - R_keep boundary and retain boundary;
  - side cap with solicited-only re-fetch;
  - a multi-branch flood during a session;
  - replay importSeq and largest-R_keep rules;
  - off-heap fidelity;
  - crash around an adoption (fsynced-uncommitted and torn records);
  - a 3-node TCP re-fetch with RPC compared block by block and a third node served by the restored
    node;
  - long catch-up;
  - a false-Status peer that cannot hold the session;
  - out-of-order session blocks;
  - linear deep side import;
  - pruned-state eth_call.
- **Mutation controls (evidence/MUTATIONS-REDB.md): 30 of 30 caught.** These are 19 D-REORG-REDB
  controls plus the 11 earlier controls that still apply (the 687ac38 controls on the removed 320-cap
  tests are superseded). Each control passed on clean code, and its mutant failed with the test run.
  The superseded partial runs are kept unchanged:
  - one uncaught control (MR5) on 9ce5a7c;
  - one compile-fail and one uncaught control (MR3, MR7) on 441ce73.
- **Integration on 3c3792e (evidence/integration.json, last run):** every suite PASS except lp1
  `store_journal`, the expected Windows-only limitation (result FAIL;
  `failuresOnlyExpectedPlatformLimitation = true`):
  - lp4 162, pocold 123 (2 ignored: the spec-depth tests above), extension 234;
  - 3-node devnet 17/17, website 20;
  - real Chrome 79, integrated Chrome 18.
- **FAIL, recorded and classified: the earlier integration on 82ddb03 (code 3342af3).**
  - e2e-devnet failed at "heal: c reorganizes". c's isolated branch happened to be heavier, so a and
    b reorganized to it and all nodes converged, which is correct by D133. The check had assumed the
    a/b side was always heavier.
  - A standalone rerun failed "every miner produced blocks": the counter restarts with its process.
    That check also fails in 2 of 4 runs on the pre-change 687ac38.
  - Both checks now assert the protocol property (evidence/e2e-devnet-classification/, original logs
    kept).
- **Witnesses on 3c3792e (evidence/witnesses-redb-final.log):** 17 observations, all NOT_REPRODUCED;
  15 positive controls OK, 0 MISSING, 0 erroneous REPRODUCED.
- **Measured RSS on 3c3792e (evidence/rss-redb-final.json):** 11.98–12.30 MB VmHWM per node (26–29
  threads) under 71,466 light, 3,853 30M-gas, 352 batch and 193 transfer requests. That is 0.21–0.22%
  of the model, a measurement and not the bound.
- **A13 model on the final code (evidence/a13-breakdown-redb.json):** 5,627,687,516 B. The gate needs
  6,431,642,876 B of 6,442,450,944 B; reserve 803,955,360 B, unused 10,808,068 B (A13-DERIVATION §2a-redb,
  §2b). Later totals, each dated to its code: 5,577,343,580 B at the integrated head `fa97594` (after
  RpcGuard round 2; unused 68,343,995 B, evidence/caps-r3/a13-fa97594.log), and 5,581,426,268 B at the
  round-3 code (`8039a09` and later; the gate needs 6,378,772,878 B, unused 63,678,066 B;
  `a13_bound_breakdown` asserts it and passes on `2b1a571`). A13-DERIVATION §2b.
- **Debug on 3c3792e** (evidence/debug-redb/; the 687ac38 partial record, 28 of 31 in pure debug, is
  kept unchanged as PARTIAL in evidence/debug-complete-687ac38.json):
  - phase A, assert-release (release with debug assertions and overflow checks), each consensus test
    alone including the two spec-depth tests: **44 of 44 PASS**;
  - phase B, pure debug (unoptimized), each consensus test alone including the spec-depth tests, with a
    3600 s per-test timeout: **44 of 44 PASS**. The spec-depth 2048/2049 test took 1,473 s and the
    D133 400-deep test 589 s. The 150-deep adoption test took 110.6 s, against 616 s on 687ac38
    (the side-state prune fix);
  - phase C, the other pocold binaries in pure debug: **PASS**: lib 38, admission 3, net 20, store 12,
    site 6, decoder 1, secp 1;
  - lp4-execution is unchanged since 687ac38 (its tree is identical), and its full debug run there was
    162 PASS. Summary: evidence/debug-redb/SUMMARY.json.

**Still open around R_keep** (as recorded at `3c3792e`; round 3 measured the first three, and they stay
OPEN, §3c):
- journal compaction;
- the CPU cost of a deep reorg (fork-point state rebuilt from genesis below K_hist; round 3: rebuilt
  twice);
- the open-time replay transient;
- Windows (not run at the time; the CI has run since, §3c).

## 3c. Round 3 (2026-10-23 to 2026-10-24): caps, RpcGuard, journal, e2e; integration and Windows

Four tracks ran on their own branches from the integrated head. Each had one or more reviews by a
separate agent instance and a fix pass after each review, and each pass was merged into the repair
branch. Where passes disagree, the latest one is reported here; what the later passes withdrew is
listed at the end of this section. Commit roles and tree equivalence: EVIDENCE-RECONCILIATION.md
(round 3).

| track | passes: branch (final code, head) | merged at |
|---|---|---|
| caps | wf/caps (`a0dd771`, `f64ed4d`); wf/caps-r2 (`c635594`, `c3a78e7`); wf/caps-r3 (`5965b8d`, `938cb94`); wf/caps-r3b (`e985ce1`, `8778587`) | `7e5c062`, `2cbac12`, `04a25c5` |
| RpcGuard | wf/rpcguard (`ad71484`, `87367fe`); wf/rpcguard-r2 (`dc4e3d1`, `a3da6aa`); integration `be95e26`, `29fd44a`; wf/rpcguard-r3 (`d0e87f9`, `5e7446f`); wf/rpcguard-r3c (`dd7d1ec`, `5560af7`) | `ba7d294`, `ba67038`, `2b1a571` |
| journal | wf/journal (`86b339e`); wf/journal-r2 (`47bfc11`); wf/journal-r3 (`7f587bb`, `256428f`); wf/journal-r3b (`e3dfb47`, `11f8e89`); wf/journal-r3c (`1d094de`, `007d579`) | `0d6f9dd`, `25bec25`, `bce7a6c` |
| e2e | wf/e2e (`bfeb226`, `c195a89`); wf/e2e-r2 (`f583c7f`, `8fd3d6e`); wf/e2e-r3 (`1512409`, `fd5c4d0`); wf/e2e-r3c (`ad84942`, `252dc1c`) | `e35f165`, `c959379` |

### Caps (CAPS-01; study in proposals/CAPS-SUSTAINABILITY.md, PROPOSED)

- **Builder conformance fix** (execution.md:83; consensus.md:101 §4(a); consensus.md:170), `9094f98`
  on `bdb14bb`:
  - before the fix the builder ignored the chain-cumulative TX_CAP/TXBYTES_CAP. Past TXBYTES_CAP, or
    past TX_CAP while one block's count still fits, the node's own block was rejected "16 caps", and
    every height waited D_att+Γ for a fallback: the chain stalled at 74–78 s per block on the D_att=73
    test devnet (measured), about 30 s on the default devnet and about 330 s on the network profile
    (derived). With TX_CAP below one block's count (tests only) the template was emptied;
  - now `Node::template_txs` skips past the room left by `Chain::within_caps`, the single definition
    of item 16, and `try_lock_signed` treats an over-cap template as absent;
  - tests/caps.rs: 6/6 fail on the unfixed builder. Later passes added the byte-exact TXBYTES
    boundary, the counter's block and branch in both directions, the cross-block exact fit and a
    boundary matrix (11 TX_CAP and 12 TXBYTES_CAP cases). Mutation checks: 16/16 (round 2), 55/55
    (round 3), 84 mutants with 83 killed and R7 argued equivalent (round 3b, `e985ce1`). Whole pocold
    suite at `e985ce1`: 155 passed, 0 failed, 8 ignored.
- **caps-test build** (validation.md:2065), `a797c83`: LOG_CAP = 2^24, simnet only. tests/log_cap.rs
  fills the chain's log bytes to exactly 2^24 and checks the halts and 20 more blocks (T17).
  HIST_TRIE_BUDGET = 64 MiB is not set (pocold has no historical trie or K_eff), so this item is
  PARTIAL.
- **genesisHash, verified independently:** proposals/model/caps_model.py (its own Keccak and
  GenesisSpec encoder, 25 checks) gives H_GSV1 = de518e30… and the devnet hash 4db6aa5c… in both builds.
  LOG_CAP is not encoded in GenesisSpec, so nodes with different LOG_CAP share a genesisHash. A LOG_CAP
  change is still a hard fork, and a mismatch is a silent split at check 17 once cumulative logs cross
  the smaller cap (argued from code; no two-node run).
- **Study (PROPOSED, not approved):** on the network profile every cap is reachable before H_END,
  organically or cheaply by an attacker. Recommendation: gate rules R27–R29 (LOG_CAP 2^32, TX_CAP
  5·10^7, TXBYTES_CAP 24 GiB), or a linear per-height allowance; N1–N8 in §5a of the study. On the
  devnet, resizing the caps first needs canonical history off the heap (the O2 model is 11.12–14.62
  GiB at `fa97594`; the 6 GiB gate refuses it).
- **Open:** the pool rule for transactions that can never be included (N5; the spec states none), and
  the free pool squatting near the TXBYTES boundary (code inspection); eth_call/eth_estimateGas ignore
  the A03 halts (N7; inspection); no peer punishment for a violating template; pocold's Hello lacks
  protocolVersion (network.md:7, outside this track); the base-fee effect of halted gas is from
  inspection; no network-profile chain was run. Commit message `9094f98` cites consensus.md:136; the
  code, tests and documents cite :101.

### RpcGuard (A05; RPCGUARD.md; the implementation is owner-approved)

- **Implemented within pocold's architecture** (one consensus thread plus a light lane, one request per
  connection), item by item in RPCGUARD.md:
  - ENFORCED: the connection cap, REQ_BUF, HEAVY_QUEUE, LIGHT_QUEUE, QUEUE_WAIT for every wait before
    admission (−32021 busy; a request answered busy never runs), COMPUTE_DEADLINE from admission and
    including encoding (5 s heavy, 500 ms light; any late answer of a method without an effect is
    −32020 {deadline}), the response pools, the SendPacer (a non-blocking socket that waits for
    writability with poll/WSAPoll, a 50 ms backstop and the A/R timer), BATCH_WALL, and SO_SNDBUF =
    32 KiB with a write cap of SEND_SOCK_BUF per call;
  - EQUIVALENT (stated proofs): HEAVY_ACTIVE and LIGHT_ACTIVE (1, the consensus thread), the
    per-connection token bucket on the connection's one request (`rate` vacuous per connection),
    CONN_BUF, LIGHT_RESP_MEM (512 KiB above the spec's budget);
  - PARTIAL: the classification (an answer over its class limit is −32020 {resultBytes} on either lane
    since round 3c; the conditional light entries have the light deadline and limit but run on the
    heavy lane; methods outside the table, P4), SCRATCH (log handlers only), cancellation (every 16
    blocks and every 10^6 gas; a precompile run is uninterruptible);
  - NOT ENFORCED: CANCEL_MAX for precompile runs (P2), light SCRATCH, RPC_MEM_TOTAL as a field,
    PacerTrace, chunked batch streaming, the numeric log range before admission, pocol_getLogs
    anchors, a separate RPC runtime.
- **R26:** 25 facts present at their running values, 6 of them equivalent and named in the admission
  notes; 4 absent (scratch_heavy, scratch_light, mem_total, cancel_max_ms).
- **A13 rpc term:** 218,579,136 B; model total 5,581,426,268 B at the round-3 code (A13-DERIVATION §2).
- **Mutation controls:** round 2, 21 mutants, 20 caught, 1 equivalent; round 3 (`d0e87f9`), 59 mutants,
  53 caught, 6 survivors explained; round 3c (`dd7d1ec`), 35 mutants, 30 caught, 5 survivors explained
  (2 equivalent, 2 within the spec's bounds, 1 test limit). Full pocold suite at `dd7d1ec`: 169 passed,
  0 failed, 8 ignored; e2e_devnet.py PASS (27 checks).
- **Proposals (PROPOSED, not approved):** P1 the aggregate rate across reconnects, P2 CANCEL_MAX for
  precompile runs, P3 effectful methods past COMPUTE_DEADLINE (option A runs; no test pins it), P4
  methods outside the classification table (proposals/RPCGUARD-R2-PROPOSALS.md).
- **Windows:** the round-3 code passed run 16 (`25bec25`); round 3c's new and tightened tests passed
  runs 19 and 21.

### Journal (A13; JOURNAL-GROWTH-AND-REPLAY.md; measurement only, no source change)

The journal stores only RLP([header, body, parentResult]); receipts, logs, state and trie are
re-derived on replay. Every figure below is a measurement on the devnet profile on this host, or a
labelled projection; the worst cases measured are lower bounds on the worst case.

**Critical performance findings (OPEN):**
- **A valid block of 380 transfers validates in about 10.2 s** on a devnet state with accounts, slots
  and code near the caps, over T_blk = 4 s (§3.2d, `1d094de`). Causes, attributed by four scratch timing
  mutants: every USER transaction runs `code_caps::validate_snapshot` and two whole-snapshot clones
  (`native::execute_for_profile`, `build_engine`), and every scan keccaks every runtime
  (`code_caps::code_len`), so each scan is O(accounts + slots + logical code bytes). On that state a
  CREATE2-heavy child takes about 2.4 s and an empty block about 0.41 s. TX_CAP bounds such transfer
  blocks to about 52 per devnet chain.
- **The worst measured one-transaction block (MODEXP, small even modulus, long exponent) takes about
  1.0 s**, over the devnet t_exec = 1 s (§3.2a; 45 kinds measured, `7f587bb`).
- **The state root is recomputed on every block:** `apply_block` clones the state and rebuilds the
  whole world root, about +0.23 s per block near the devnet SLOT_CAP/ACCT_CAP and about 14.4 s per
  block at network caps (synthetic) (§3.2b).
- **The worst devnet open at H_END takes about 7.3 h** (MODEXP blocks on the near-cap state), plus about
  8.5 min for the TX_CAP-bounded transfer blocks (§3.3).

**The three journal obligations stay OPEN:**
- **disk growth:** append-only, no compaction; the canonical maximum is about 11.3 GiB at network caps
  and about 0.22 GiB on the devnet; the solicited re-fetch vector re-journals evicted in-window blocks
  each cycle and, with the fork point below K_hist, also rebuilds from genesis each cycle (§2, §4);
- **re-execution from genesis:** open replays every record (Θ(H²)); a deep reorg below K_hist
  rebuilds the fork point from genesis twice, because `retain` prunes the canonical fork point below
  head−K_hist: a depth-2048 reorg near the devnet tip takes 96 s on an empty chain and up to about
  11 h under the worst measured kind, in two stalls that each exceed D_att (§5.3); pool re-execution of
  50 abandoned MODEXP transactions in an adopting reorg took about 50 s;
- **the open-time memory transient:** about 1.0× the journal on top of the steady resident chain
  (peak 2.1–5.7× by block kind), outside the A13 steady-state bound (§3.4).

The network profile is not admissible on pocold (the A13 history term TXBYTES_CAP = 8 GiB alone
exceeds the 6 GiB budget), so every network figure is a projection. Every mitigation in §7 of the
journal report (headers-first sync, dedupe markers, compaction behind a signed snapshot, reverse
diffs, an O(window) retain and index, keeping the fork-point state, an incremental state root and
per-transaction scans, bounding re-execution cost per gas) is PROPOSED; none is implemented.

### e2e devnet test (round 4 is current; evidence/e2e-mutation-r4/)

- tests/e2e_devnet.py at `ad84942` (sha256 9c6d7c0d…98814c, 38 checks). The partition fixes a winner
  that is strictly heavier and strictly lower than the other tip, so most work (consensus.md:209) is
  told apart from longest chain; the heal starts from Status sync only. The lighter branch is relayed
  without a session and inside a sync session, and a node replays its journal alone. Step 4b sends a
  heavier branch through the unsolicited and orphan path; step 4c (E2E-R4-1) sends the lighter branch
  tip first through the orphan pool, and the node must keep its head with no reorganization. Steps 8b
  (heal while mining), 9 (replay after SIGKILL, chain check at depth CONFIRM) and 11 (frozen tips)
  were added in rounds 3 and 4.
- Base: PASS 38/38 three consecutive times. Mutants, one run each: 23, of which 22 were caught (16 at
  the intended check, 6 earlier) and Mtie survived by design (the D133 tie-break is local; the unit test
  `fork_choice_heavier_tied_signed_first_and_rollback` catches it). M4's first run failed a step-3
  precondition and is not counted; its second run was caught at the intended check.
- Earlier rounds, kept as history: stage A (`bfeb226`, 10 mutants), round 2 (`f583c7f`, 13 mutants,
  3/3 PASS 27/27), round 3 (`1512409`, 21 mutants, PASS 35/35).
- Open: the step-3 climb bound can give a false FAIL when the lead overshoots; one run per mutant;
  step 8b catches no mutant by itself; the step-9 race and torn-tail allowances were never exercised;
  the full LP10 integration and Windows were not re-run on this test.

### Integration on the merged head (release, Linux; evidence/integration-r3/)

- **PASS on `2b1a571`** (merges of caps r3b, journal r3c, RpcGuard r3c): pocold 171 passed, 0 failed,
  14 ignored (the 2 spec-depth tests and journal_measure's 12 measurements); lp4-execution 168 passed,
  0 failed; caps-test build 11 passed, 0 failed.
- **PASS on `c959379`** (merge of e2e round 4): e2e_devnet 38/38, e2e_site 20/20, tests/consensus.rs
  43 passed, 0 failed (2 ignored).
- `1cc96c4` (test settings only): `abandoned_queued_bodies_are_capped_and_released` 8/8 on Linux.
- Earlier in round 3: pocold 164/0 and lp4 168/0 on `25bec25`; e2e_devnet 3× PASS 27/27 and e2e_site
  20/20 on `d40fd91` (evidence/e2e-integration/).
- **NOT RE-RUN on the round-3 code:** the full LP10 integration (extension, real Chrome, integrated
  Chrome), the witnesses, the RSS measurement and the debug runs. Their last results are on `3c3792e`
  (§3b). This is not a full acceptance.

### Windows (WINDOWS-VERIFICATION.md)

The owner's 2026-10-23 instruction was taken as authorizing a pull_request trigger, limited to PRs into
`claude/affectionate-pascal-rjhh2k` (`a03e089`). Nothing was merged into `main`.
- **Runs 1–7 found and fixed four Windows defects:** a `retention.rkeep` directory fsync that Windows
  refuses (48 tests); a dropped P2P peer's reader that `shutdown` does not wake; a Nagle/delayed-ACK
  stall of about 200 ms on every RPC answer; kernel absorption of unread RPC answers, fixed by SO_SNDBUF
  = 32 KiB plus a write cap of SEND_SOCK_BUF per call.
- **PASS on every check:** run 9 (`29fd44a`); run 16 (`25bec25`, the first run of the round-3
  poll/WSAPoll code); run 19 (`2b1a571`); run 21 (`1cc96c4`).
- **Run 20 (`49ae9fa`, the same code as run 19) failed one timing test,**
  `abandoned_queued_bodies_are_capped_and_released`. `1cc96c4` changed only that test's settings
  (HEAVY_QUEUE 2, QUEUE_WAIT 5 ms, up to 12 rounds; assertions unchanged), and run 21 passed.
- **Not covered on Windows:** the real-Chrome and Python e2e suites, pocold's thread-count assertions
  (they read /proc) and the `#[ignore]` measurement tests. `c959379`'s Rust test change (tests/consensus.rs
  from `16bebfd`) ran on Windows in run 22 (PASS); tests/e2e_devnet.py is a Python e2e test the workflow does not run.

### PR #27 review threads (2026-10-24; evidence/pr27-review-threads/)

Three automated review threads from 2026-10-22 were still open. Each was checked against `8ce75c4`.

- **Side-block pruning (`chain.rs`, outdated).** Already fixed: `retain` evicts leaves repeatedly, so the
  cap holds after a reorg. No change.
- **A11 restore after a deep reorg (`node.rs`). Reproduced and fixed in `462fffc`.**
  - The cause: the walk to the common ancestor stopped after 2·1,024 steps. A reorg whose two sides total
    more than 2,048 blocks therefore lost the oldest detached transactions. This case is reachable since
    D-REORG-REDB adopts at depth R_keep.
  - The fix: the walk now reaches the ancestor, and the bodies are read back from the journal one at a
    time. That removes an unmodelled A13 transient, a copy of all the detached transactions (at most
    TXBYTES_CAP = 16 MiB on the admitted profiles).
  - The test: a new ignored release test fails before the fix and passes after it.
- **R14/R19 calibration (`admission.rs`). Reproduced and fixed in `462fffc`.**
  - The cause: the London base fee of full blocks passed the fixed fee cap after about 24 blocks, and
    later samples were empty blocks. With D = 30, block 25 used 0 gas.
  - The fix: the base fee is now held at baseFee0, and `measure` refuses a block below GAS_LIMIT gas.
  - The test: a new test fails before the fix (with the guard) and passes after it.
  - The approved profiles (11 blocks) were not affected.

Suites on `462fffc`: lp4-execution 169/0; pocold 171/0 (15 ignored); the three ignored consensus tests 3/3
(spec depth 2048/2049, D133 at 400, the new deep reorg); e2e_devnet 38/38; Windows CI run 26 PASS (the
new calibration test runs there; the deep-reorg test is `#[ignore]` and does not). Neither
change is normative.

### Corrections within round 3 (each withdrawn by a later pass, history kept in the track documents)

- **e2e:** stage A's "a wrong choice fails every time" and "9 caught": a height-first fork choice
  passed that test 25/25 (e2e-mutation-r2/README.md §2). Round 3's "E2E-R3-3 fixed" holds for the
  heavier-branch direction only; the lighter direction is step 4c of round 4.
- **journal:** stage A's "deep reorgs take seconds" (a deep reorg rebuilds from genesis twice); round
  2's "devnet open fits" (J-V6 reopened); round 3b's "on a full state a create-heavy block is the
  per-block worst" (withdrawn in round 3c: the 380-transfer block is the worst measured).
- **RpcGuard:** stage A's 26 R26 facts included fields that were not enforced; round 3 called the I3b
  survivor harmless, and it was not (round 3c tests it); round 3's "the round-3 code has not run on
  Windows" is withdrawn, because run 16 ran it.
- **caps:** stage A called the caps-test history term a lower bound (it is a smaller, still valid upper
  bound); "at most 12.8 coins" was one strategy, not a bound; `9094f98`'s message cites consensus.md:136
  instead of :101.
- **Windows:** run 5's diagnosis (the client's receive window) was refuted by run 7.

## 3d. Decision package (2026-10-24): conformance work and the five independent reviews

The owner asked for one decision package ([proposals/DECISION-PACKAGE.md](proposals/DECISION-PACKAGE.md),
Arabic summary [DECISION-PACKAGE-AR.md](proposals/DECISION-PACKAGE-AR.md)). It separates conformance fixes
within the current authorization, which are being done, from normative changes, which are shown with a
recommendation and are not approved or implemented. The priorities are: booting the network profile, block
validation time, sustainable caps, journal growth and recovery.

**Measured facts:**
- The network profile needs 172.4 GiB against 6 GiB (`tests/network_budget.rs`, which asserts that it
  does not fit).

**Conformance work done:**
- C-2 items 1–3 (`49b2bc2`, `2ce03c1`; evidence/c2-execution/):
  - the snapshot is checked once per USER phase;
  - running code totals;
  - the engine borrows the state instead of cloning it, and PAY reads the engine's database.
- Measured against `3a52ae6` on the same near-cap state:
  - 380 transfers: 22,553 → 655 ms;
  - 247 CREATE2 creates: 4,862 → 562 ms;
  - an empty child: 777 → 465 ms.

**The five reviews (evidence/reviews-2026-10-24/)** and what was done:

| review | findings ≥ MEDIUM | done |
|---|---|---|
| R1 caps r3b | R1-1 builder starvation, R1-2 LOG_CAP unpinned | fixed in `6e99b19`, with R1-4 |
| R2 RpcGuard r3c | R2-1 a batch held the consensus thread | fixed in `af2f3a8`, with the LOWs R2-2 to R2-5 |
| R3 journal r3c | R3-2, R3-3 (HIGH); R3-1, R3-6, R3-7 | R3-3 fixed in `af2f3a8`; R3-2, R3-7 by C-2; R3-1, R3-8, R3-13 in [JOURNAL-GROWTH-CORRECTIONS-2026-10-24.md](JOURNAL-GROWTH-CORRECTIONS-2026-10-24.md); R3-6 is decision D-7 |
| R4 e2e round 4 | R4-1 shared W(t) bug passes, R4-2 relay and Status refresh unneeded | fixed: unit test in `af2f3a8`; e2e checks with base 42/42 twice and the reviewer's three mutants each caught at its check (evidence/reviews-2026-10-24/r4-fixes/) |
| R5 `462fffc` | R5-1 a full scan and clone per shared reorg transaction | fixed by C-2 |

Also done in this pass:
- **C-1 S2.** The code term counts one set per lineage, since a state copy shares its runtime buffers
  (a test proves it). The network model falls from 150.83 to 105.83 GiB, still far over 6 GiB. The
  devnet model total is 5,487,223,178 B (A13-DERIVATION.md).
- **C-1 S1** (owner-approved D-1 (a), 2026-10-24). Canonical history below a resident window of
  K_hist blocks moves off the heap:
  - bodies and headers are read from the journal;
  - receipts and the transaction index come from `history.redb`, which is derived and rebuilt from the
    journal when damaged.
  Results:
  - the RPC answers are byte-identical to the tree before S1;
  - the network model falls from 105.83 to 90.93 GiB;
  - the devnet model total is 4,323,240,949 B.
  Evidence: [evidence/c1-s1/](evidence/c1-s1/README.md); design and status:
  [proposals/C1-STORAGE-DESIGN.md](proposals/C1-STORAGE-DESIGN.md).
- **C-1 S3a** (2026-10-24). Retained canonical states are diffs from their parents, materialized from a
  full state on demand. Only the head, genesis and the side states within side_max are held full.
  - Every retained state equals the full state it replaces (8,388 states compared).
  - Every state RPC answers as before, live, after reorgs and after reopen.
  - Network model 90.93 → 41.41 GiB; devnet 3,601,500,638 B.
  - Mutation controls: 9 of 11 killed; SM8 is equivalent; SM10 guards a case that no longer occurs.
  - Evidence: [evidence/c1-s3a/](evidence/c1-s3a/README.md).
- **C-1 S4** (2026-10-24). Old canonical entries are slimmed in memory, and their header, ParentResult and
  nonce ranges are restored exactly from the journal and history.redb.
  - Network model 41.41 → 37.96 GiB; devnet 3,322,680,105 B.
  - Mutation controls: 9 of 9 killed.
  - Evidence: [evidence/c1-s4/](evidence/c1-s4/README.md).
- **C-1 E1** (2026-10-24; also C-2 item 4). The state root is updated incrementally from a trie shared
  between versions, used only when its root equals the parent's committed one.
  - Every root in the tests is compared with the whole-state computation.
  - Near-cap validation: an empty child 491 → 262 ms; 380 transfers 672 → 349 ms.
  - A13 adds a trie group (2.23 GiB at the network profile): network model 37.96 → 40.20 GiB; devnet
    3,486,909,657 B.
  - Evidence: [evidence/c1-e1/](evidence/c1-e1/README.md).
- **C-1 E2** (2026-10-24). The per-block whole-state checks (candidate checks, code checks, code drop)
  are read from a state summary moved through the changed keys.
  - Near-cap validation: an empty child 262 → 62 ms; 380 transfers 349 → 174 ms.
  - Mutation controls: 11 of 12 killed; E2M8 is unreachable.
  - Evidence: [evidence/c1-e2/](evidence/c1-e2/README.md).
- **C-1 E3.1–E3.3** (2026-10-25). The executor, production, template selection, the pool and RPC run on
  overlays over the parent. The S3a diff is built from the block's write set.
  - Every block in the tests is also applied on a copy and compared field by field.
  - Near-cap validation: an empty child 62 → 8.5 ms; 380 transfers 174 → 55 ms.
  - A13: transient copies 10 → 2, plus the pool's layer. Network model 40.20 → 28.08 GiB; devnet
    3,186,963,844 B.
  - Mutation controls: 18 of 19 killed; E3M7 is equivalent.
  - Evidence: [evidence/c1-e3/](evidence/c1-e3/README.md).
- **consensus.md:101 "the peer is punished"** (R1-5): a local disconnect, with a test and a mutation
  control.

Each fix has a fail-before log or a killed mutant. Suites on `af2f3a8`: lp4 173/0 and pocold 181/0
(15 ignored).

## 4. Independent review

Round 1:
- three reviews by separate agent instances (Rust side, extension side, review-fix commits);
- all confirmed findings were fixed (REPAIR-PLAN.md).

Round 2: two more reviews by separate agent instances, which ran code in scratch copies.
1. **df63725 (F1–F9).** All nine were fixed, each with a test and a mutation control:
   - a lost wakeup in the peer outbox;
   - an unbounded light-lane Wake;
   - the execution model's state accounting;
   - RPC request memory undercounted;
   - the scope guard let Synthetic through;
   - the field inventory was not tied to RpcLimits;
   - miner and thread overlap;
   - build-speed-dependent tests;
   - light requests overtaking heavy ones (documented).
2. **45712cd..b132a05 (the D-A13-BUDGET range).** Findings (A13-DERIVATION §2a):
   - SIDE_BLOCKS_MAX 64 blocked the adoption of long branches (HIGH): reverted to 256;
   - a restart failure on duplicate journal records (HIGH): fixed;
   - an unbounded parse on the busy path: fixed;
   - the side-block term after a reorg: fixed;
   - a light-answer regression for TX_MAX transactions: fixed;
   - two host-dependent tests: fixed;
   - test gaps and small undercounts: fixed;
   - the stats method's name: renamed.

Round 3 (§3c): each track was reviewed by separate agent instances, and each review was followed by a
fix pass:
- caps: three reviews (CV-1..CV-12, R2C-1..R2C-6, C3R-1..C3R-4);
- RpcGuard: three reviews (B1–B5 with M1–M6 and m1–m7; RG2-*; RG3V-*);
- journal: four reviews (J-V1..J-V13, J2-*, J-REV3-1..3, JV3B-1..4);
- e2e: three reviews (F1–F10, E2E-R3-1..9, E2E-R4-1..3).

The last pass of each track (caps r3b, RpcGuard r3c, journal r3c, e2e round 4) and `462fffc` were
reviewed on 2026-10-24 (R1–R5, §3d; evidence/reviews-2026-10-24/). The reviewers were given the code and
the spec, not this track's conclusions.

These are agent reviews, not an external human audit, and the shared GitHub account is not an
independent approval. The owner has not yet reviewed the consensus proposals.

Both round-2 reviews ran the code in scratch copies, and every finding was treated as a defect with its
own regression test and a mutation control (evidence/MUTATIONS-R2.md), not as a test expectation to
adjust. The second review's highest findings (SIDE_BLOCKS_MAX 64 blocking adoption, the duplicate-record
restart failure) were HIGH; SIDE_BLOCKS_MAX was reverted to 256 and then raised to 320 on the tested
path, and the gap to R_keep is kept open as an architectural limitation (§5, D-REORG-REDB). No review
weakened a consensus rule, and no review constitutes acceptance.

## 5. Remaining limitations and blocked evidence

- **Windows (updated in round 3): the repair-scope CI passes; its coverage is limited.**
  - Round 2 recorded "Windows: not run": the scoped workflow returned 404 on dispatch until its file
    was on `main`. Since 2026-10-23 it runs on a pull_request trigger limited to PRs into
    `claude/affectionate-pascal-rjhh2k` (WINDOWS-VERIFICATION.md, option B).
  - PASS on every check: runs 9 (`29fd44a`), 16 (`25bec25`), 19 (`2b1a571`) and 21 (`1cc96c4`). Run 20
    failed one timing test on the code of run 19; `1cc96c4` fixed the test's settings (§3c).
  - Not covered on Windows: the real-Chrome and Python e2e suites, pocold's thread-count assertions
    (they read /proc, THREADS-NOT-MEASURED), the `#[ignore]` measurement tests, and the Python e2e test
    (tests/e2e_devnet.py). Run 22 (`c959379`) passed the Rust test change of `16bebfd`.
  - Directory fsync is skipped on Windows after a rename (store::sync_dir, retention.rkeep).
- **lp1-node `store_journal` (11 tests):** executed on Linux and **failed** with UnsupportedPlatform.
  This is the expected platform limitation (Windows-only atomic journal), identical on 090eae4, and is
  recorded as failures, not passes. On Windows they pass (lp1-store-journal 12 passed in every recorded Windows run).
- **Original audit programs** (FINDINGS.json, REPRODUCE.md, overlays, SHA256SUMS): not received. Every
  witness was reconstructed from REPORT.md and carries a positive control.
- **A13** (A13-DERIVATION §4):
  - Open:
    - gas monotonicity;
    - worst-case timing;
    - allocator overhead and fragmentation (a reserve is kept, but nothing proves it suffices);
    - CPU costs;
    - RetentionStore and redb.
  - **Critical performance findings of round 3, OPEN** (§3c; JOURNAL-GROWTH-AND-REPLAY.md §3.2a–§3.3):
    - a valid 380-transfer block validates in about 10.2 s on a near-cap devnet state, over T_blk =
      4 s (per-transaction `validate_snapshot` and two whole-snapshot clones; a keccak of every
      runtime on each scan);
    - the worst measured one-transaction block (MODEXP) takes about 1.0 s, over the devnet t_exec =
      1 s;
    - the whole state root is recomputed on every block;
    - the worst devnet open at H_END takes about 7.3 h.

    Each is a lower bound on the worst case. No mitigation is implemented; the incremental state root
    and per-transaction scans are PROPOSED (journal §7, option 7).
  - The network profile is not admissible on pocold (TXBYTES_CAP = 8 GiB alone exceeds the 6 GiB
    budget). Every network figure is a projection.
  - Inspection-only items remain.
  - There is no guaranteed process-memory ceiling. The measured RSS of a run is not the bound.
  - **R_keep adoption (formerly the architectural blocker):** implemented by the owner-approved
    D-REORG-REDB and verified as in §3b. Still open around it, the three journal obligations, measured
    in round 3 and all OPEN:
    - journal disk growth: no compaction; the journal is append-only and grows with imports and
      re-fetches, including the solicited re-fetch re-journal vector (local, no validity effect);
    - re-execution from genesis: open replays every record; a deep reorg rebuilds the fork point's
      state from genesis twice when it lies below K_hist, because pocold keeps no reverse diffs
      (consensus.md §10) and `retain` prunes the canonical fork point; condition `cpu-and-timing`;
    - the open-time memory transient: about 1.0× the journal on top of the steady chain (the whole
      journal is held while replaying; condition `open-replay-transient`).
- **A05 (updated in round 3):** RpcGuard is mapped item by item in RPCGUARD.md (ENFORCED, EQUIVALENT,
  PARTIAL, NOT ENFORCED). Round 3, after the second review of round 2:
  - any late answer of a method without an effect, result or error, is −32020 {deadline}
    (network.md:235);
  - the SendPacer waits for writability (poll/WSAPoll, 50 ms backstop, network.md:245–246) instead of
    retrying every 1 ms; on this Linux host that is 18–25 times less CPU per blocked connection and
    18–31 times the honest throughput (RPCGUARD.md §4a);
  - an answer over its class limit is −32020 {resultBytes} (network.md:210), since round 3c on either
    lane, and the conditional light entries (eth_getBlockBy* with hashes only, pocol_getHeaders with
    count ≤ 64) have the light deadline and answer limit (network.md (ب), :235);
  - the P2P reader waits with poll/WSAPoll and no longer uses SO_RCVTIMEO;
  - mutation passes: round 3 on `d0e87f9`, 59 mutants, 53 caught, 6 survivors
    (evidence/rpcguard-r3/MUTATIONS-RPCGUARD-R3.md). The third review then showed that one survivor
    (I3b) and four terms (the pacer's A/R timer term, the backstop, the post-encoding check, the light
    late check) were pinned by no test; round 3c adds those tests, and its pass on `dd7d1ec` gives 35
    mutants, 30 caught, 5 survivors, each explained (V8 and R3-M4-NO-CLASS-LIMIT equivalent;
    R3-M2-NO-WALL-IN-WAIT and R3-M2-BACKSTOP-ONLY within the spec's bounds; RV3-CAS-CHECK-THEN-SET a
    test limit) (evidence/rpcguard-r3c/MUTATIONS-RPCGUARD-R3C.md); full pocold suite on `dd7d1ec`: 169
    passed, 0 failed, 8 ignored; e2e_devnet.py PASS (27 checks).

  Not enforced or open: SCRATCH outside the log handlers, CANCEL_MAX for precompiles (P2),
  RPC_MEM_TOTAL as a field, PacerTrace, chunked batches, the unlisted-method rule (P4), the aggregate
  rate (P1), and the P3 reading for effectful methods (option A runs). A running precompile call is not
  preempted. Round 3's code passed Windows run 16 (`25bec25`); round 3c's tests passed runs 19 and 21.

  Round 2 recorded here: "RpcGuard is implemented only in part. Not implemented: HEAVY/LIGHT permits and
  QUEUE_WAIT; scratch budgets; rate buckets; per-class compute deadlines; the send watchdog. A running
  request is not preempted." The items above replace it.
- **Caps (CAPS-01 and the study, round 3):** the pool rule N5 is not implemented (no spec rule);
  eth_call/eth_estimateGas ignore the A03 halts (N7, inspection); the caps-test HIST_TRIE_BUDGET is not
  set; no peer punishment for a violating template; Hello lacks protocolVersion; every cap is reachable
  before H_END on the network profile, and CAPS-SUSTAINABILITY N1–N8 are PROPOSED, not approved.
- **e2e (round 4):** one run per mutant; the step-3 climb bound can give a false FAIL; step 8b catches no
  mutant by itself; the Mtie tie-break mutant survives by design (a unit test catches it).
- **Full integration:** the LP10 suites with real Chrome, the witnesses, the RSS run and the debug runs
  were not re-run on the round-3 code (§3c).
- **A02:** only an experimental-devnet admission is possible. The R14/R19 timing sample fails closed
  under CPU contention: pocold's consensus tests in a debug build fail when run in parallel, and pass
  single-threaded on an idle host.
- **A06:** a request already initiated may reach the network and is reported as initiated or broadcast.
- **A04:** real power loss is untested.
- **Dependencies:** libsecp256k1 0.5.0 and rand 0.7.3 remain in lp1-node (accepted toolchain pin).
  derivative and paste remain as build-time proc-macros via revm.

## 6. Decisions

- **Approved by the owner:**
  - D-A13-BUDGET (option 1, local);
  - D-REORG-REDB (2026-10-23), a storage repair for R_keep adoption with D133 and validity fully
    preserved. It does not cover the six consensus proposals;
  - the RpcGuard implementation in pocold (2026-10-23, "complete the authorized works: RpcGuard and
    the Windows verification"), a local service policy (D87) with no validity effect;
  - engineering decisions taken under the standing authorization (REPAIR-PLAN.md).
- **Proposed, awaiting the owner** (they change roots or validity):
  - LOG-ACCT-01;
  - D-A03-DIRTY (including the block-invalid halt-residue case);
  - D-A03-CAPS;
  - D-BODY-MAX;
  - D-A03-SYSUSER;
  - LOG-CAP-AGGREGATE.

  The implementation uses the recommended alternatives provisionally. Passing tests do not approve
  them. The historical M1 files are unchanged.
- **Proposed in round 3, awaiting the owner, none approved:**
  - CAPS-SUSTAINABILITY N1–N8 (proposals/CAPS-SUSTAINABILITY.md §5a): gate rules R27–R29 with LOG_CAP
    2^32, TX_CAP 5·10^7 and TXBYTES_CAP 24 GiB, or the linear allowance N4; the pool rule N5;
    pocol_getCaps N6; A03 halts in eth_call N7; the scope statement N8;
  - RPCGUARD P1–P4 (proposals/RPCGUARD-R2-PROPOSALS.md): the aggregate rate, CANCEL_MAX for precompile
    runs, effectful methods past COMPUTE_DEADLINE, methods outside the classification table;
  - every journal mitigation (JOURNAL-GROWTH-AND-REPLAY.md §7). Headers-first sync and reverse diffs
    are conformance gaps against approved text; option 8 (bounding re-execution cost per gas) is
    normative.
- **Reviews prepared for the owner (not decisions):**
  - proposals/INDEPENDENT-REVIEW-SIX-PROPOSALS.md;
  - proposals/LOG-CAP-AGGREGATE-ALTERNATIVES.md, on ways to avoid permanent exhaustion of LOG_CAP
    (LOG-CAP-AGGREGATE stays unapproved). It also flags that TX_CAP and TXBYTES_CAP are
    chain-cumulative and reachable before H_END on the network profile, which would make blocks
    invalid. That is a further normative question, not decided here. Round 3 analyses it in
    proposals/CAPS-SUSTAINABILITY.md (PROPOSED, not approved).

## 7. Corrections to round 1

- "Closed with limitations" for every finding was overstated. No finding is closed (§1).
- **Witnesses:** "18/18 NOT_REPRODUCED" was 17 observations. Round 1 had no positive controls; round 2
  adds one per witness.
- **A13:** "4.92 GiB ≤ 6 GiB" was not an upper bound. Three things were undercounted or assumed:
  - thread stacks were assumed resident at 64 KiB;
  - EVM execution memory was assumed;
  - RPC answers were undercounted.

  The corrected model exceeded the budget until D-A13-BUDGET. The model stays conditional: 5.20 GiB when
  this was written, 5.198 GiB (5,581,426,268 B) at the round-3 code.
- **Windows** was labelled "blocked by platform". It was not run, for the precise reason recorded in §5
  at round 2; since 2026-10-23 the repair-scope CI runs and passes (§3c).
- **lp1-node:** the 11 `store_journal` tests were described as blocked. They executed and failed.
- **Rust 1.58.1** was said to be unavailable. It was installed through rustup, and lp1-node now runs
  with it in the integration.
- **A06:** round 1 left a submission proceeding after a lock while transport capacity was awaited.
  That gap is closed by the submission states.
