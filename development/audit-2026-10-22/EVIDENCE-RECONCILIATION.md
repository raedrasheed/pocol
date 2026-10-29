# Evidence reconciliation (rounds 2 and 3)

Commit roles for the verification, and proof of which evidence commits carry the same built code
(prototypes/ and .github/ trees), so each result is applied only to the code it ran on. The round-3
section comes first; the reconciliation recorded for `3c3792e` and the one for `687ac38` follow,
kept for history.

## After round 3 (2026-10-24): code at `462fffc` (PR #27 review threads)

`462fffc` changes `pocold/src/node.rs` (the A11 reorg walk) and `lp4-execution/src/admission.rs` (the
R14/R19 calibration), each with a new test. It is now the last implementation commit. Results from
`c959379` and earlier apply to the code before this change.

| role | commit | prototypes/ | .github/ | pocold/src | lp4-execution/src |
|---|---|---|---|---|---|
| last implementation commit | `462fffc5e4af6006a0ef5aa96bb1070d50a2749b` | `e1314693fc8b` | `0c6344e64559` | `828205b1c784` | `6ef83cebf0a3` |

Ran on `462fffc` (release, Linux; evidence/pr27-review-threads/):
- lp4-execution 169/0;
- pocold 171/0 (15 ignored);
- the ignored consensus tests 3/3;
- e2e_devnet 38/38.

Windows CI run 26 (PR head `43e7e0f`, same prototypes/ tree as `462fffc`): PASS.

Not yet run on `462fffc`: the debug runs, the mutation controls of other tracks, and the
LP10/Chrome integration.

## Round 3 (2026-10-24): code at `c959379`

In round 3 the evidence ran on several commits, because four tracks were merged one after another.
No single evidence commit carries the head's code for every suite, so each role below names its own
commit and how its tree differs from the head's. Every difference listed is in test files; the source
trees (`prototypes/pocold/src`, `prototypes/lp4-execution/src`) are identical from `2b1a571` to the head.

| role | commit |
|---|---|
| audited snapshot | `090eae4` |
| last implementation commit (prototypes/ and .github/) | `c959379` (merge of wf/e2e-r3c; its prototypes/ changes are tests/consensus.rs and tests/e2e_devnet.py) |
| last source change | `dd7d1ec` (RpcGuard r3c), merged at `2b1a571` |
| PR head before this documentation commit | `d91d141` (evidence only: 3 logs in evidence/integration-r3/) |
| e2e_devnet 38/38, e2e_site 20/20, tests/consensus.rs 43/0 | `c959379` (evidence/integration-r3/*-c959379.log) |
| pocold 171/0 (14 ignored), lp4-execution 168/0, caps-test build 11/0 | `2b1a571` (evidence/integration-r3/*-2b1a571.log, committed in `49ae9fa`) |
| Windows CI run 21, PASS | `1cc96c4` |
| Windows CI run 19, PASS; run 20, FAIL on one timing test (same code) | `2b1a571`; `49ae9fa` (evidence/windows-49ae9fa/) |
| Windows CI run 16, PASS (first run of the round-3 RpcGuard code) | `25bec25` (evidence/windows-25bec25/) |
| Windows CI run 9, PASS | `29fd44a` (evidence/windows-29fd44a/) |
| full LP10 integration (real Chrome), witnesses, RSS, debug | `3c3792e`, not re-run on the round-3 code (history section below) |

### Tree equivalence (git rev-parse `<commit>:<path>`)

| role | commit | prototypes/ | .github/ | pocold/src | lp4-execution/src | prototypes/ difference from `c959379` |
|---|---|---|---|---|---|---|
| last implementation commit | `c9593791151cb24228f317dec42c0ee5a6b1093e` | `04b3bd07f906` | `0c6344e64559` | `7616fb18d367` | `5ffc9ad4e170` | — |
| PR head before this commit | `d91d1416f11137e4ff40bc44959fb06233ee439a` | `04b3bd07f906` | `0c6344e64559` | `7616fb18d367` | `5ffc9ad4e170` | none |
| Windows run 21 | `1cc96c4140505ee64a2350f11462dbb9b562043d` | `f7739d6d12d4` | `0c6344e64559` | `7616fb18d367` | `5ffc9ad4e170` | tests/consensus.rs (`16bebfd`, E2E-R3-7) and tests/e2e_devnet.py; neither has run on Windows |
| suites on `2b1a571`; Windows run 19 | `2b1a5713f3aa47cfbbe289242732501693ab0253` | `799b17febb78` | `0c6344e64559` | `7616fb18d367` | `5ffc9ad4e170` | the above, plus tests/net.rs (`1cc96c4`: the abandoned-queue test's settings) |
| Windows run 16 | `25bec2530f7f6700834c636434f1bfabf104962b` | `fe6a2b568249` | `0c6344e64559` | `f703397f2909` | `5ffc9ad4e170` | src/rpc.rs (RpcGuard r3c, `dd7d1ec`) and the tests caps.rs (caps r3b), journal_measure.rs (journal r3b–r3c), net.rs (RpcGuard r3c, `1cc96c4`), consensus.rs and e2e_devnet.py |
| Windows run 9 | `29fd44a7aa167984d0005ab5ab5e16b45c9dd528` | `b8eb87163abd` | `0c6344e64559` | `4c41c1419324` | `5ffc9ad4e170` | every source and test change merged after it: RpcGuard rounds 3 and 3c (src/lib.rs, net.rs, resources.rs, rpc.rs), and the tests of RpcGuard r3–r3c, journal r2–r3c, caps r3–r3b and e2e r2–r4 |
| D-REORG-REDB final code (LP10 integration, witnesses, Chrome, RSS) | `3c3792e90eb3c31c83abc91f26e861c8510a824f` | `85e63f96f225` | `347e2324e80c` | `71a85119ddee` | `4e365a9520cb` | every round-3 change, and `.github/` (the pull_request trigger, `a03e089`) |

Commits after the implementation commit `c959379`: `d91d141` (evidence only) and this documentation
commit (development/ only). Neither changes prototypes/ or .github/.

### Commit roles per track (round 3)

Each track's commits were made on its own branch and merged; the merge commits are listed in
REPAIR-REPORT.md §3c. Every tree equality below was checked with `git diff --stat <a> <b> -- prototypes`
(empty) or the path stated.

**caps** (evidence/caps/, caps-r3/, caps-r3b/):
- Stage A: `182026d`, `bdb14bb` and `ddf023a` are tests-first commits with the unfixed builder;
  tests/caps.rs fails at them by design (keep this in mind when bisecting). `9094f98` is the fix (its
  message cites consensus.md:136, which should be :101). `a797c83` is the caps-test feature. `bc53984`
  is the model and vectors. `a0dd771` is stage A's final code tree. `f64ed4d` adds documents and
  evidence; its prototypes/ tree equals `a0dd771`'s. The unfixed-final log is the `9094f98` tree with
  node.rs from `ddf023a`.
- Round 2: `c635594` is the boundary test and comment citations (the round-2 code tree). `0b9b4d0` is
  the study, model and vectors; its prototypes/ tree equals `c635594`'s, and all round-2 runs are on
  it. `c3a78e7` is evidence and study only (prototypes/ equal to `0b9b4d0`'s).
  r2-mutations-c635594.log records outputs only; the runner and mutant edits stay in scratch.
- Round 3: `5965b8d` changes tests/caps.rs only (prototypes/ otherwise equal to `fa97594`'s); the
  tests and the 55 mutants ran on it. `938cb94` changes development/ only (prototypes/ equal to
  `5965b8d`'s). A13 was measured at `fa97594` (evidence/caps-r3/a13-fa97594.log).
- Round 3b: `e985ce1` changes tests/caps.rs only (prototypes/ otherwise equal to `fa97594`'s); the
  tests, the 84 mutants and the R7 probe ran on it. `8778587` changes development/ only (prototypes/
  equal to `e985ce1`'s). The probe program is not committed (its sha256 is in r7-probe-e985ce1.log).

**RpcGuard** (evidence/rpcguard-r3/, rpcguard-r3c/, rpcguard-integration/; RPCGUARD.md §11, §11a, §13a):
- Stage A: `ad71484` is the implementation; `31500a4` and `87367fe` add evidence only (prototypes/
  equal to `ad71484`'s).
- Round 2: `6a561b6` code and tests, `dc4e3d1` one more test; `b473107` and `a3da6aa` documents and
  evidence (prototypes/ equal to `dc4e3d1`'s).
- Integration: `be95e26` sets SO_SNDBUF = SEND_SOCK_BUF on every RPC connection; `49508a5` and
  `29fd44a` are the Windows-driven changes (the write cap of SEND_SOCK_BUF per call is `29fd44a`).
- Round 3: code `0f48e6e`, `65a828b`, `1530d7f`, `8039a09`, `d0e87f9`. `d0e87f9`'s src/ equals
  `8039a09`'s up to one doc-comment line in rpc.rs. Final runs on `d0e87f9`: both release suites,
  e2e_devnet.py, the FFI cross-check, the pacer measurement and the mutation pass. Earlier runs on
  `1530d7f` and `8039a09` are kept; the `65a828b` partial and the `1530d7f` truncated mutation runs are
  not counted (MUTATIONS-R3-EARLIER-RUNS.md). `5e7446f` adds documents and evidence only. The
  measurement witness tests/zz_audit_witness_rpcguard_r3.rs is gitignored and not committed (sha256
  580734f8…); its output logs are committed.
- Round 3c: `dd7d1ec` is the code under test (src/rpc.rs, tests/net.rs). Runs: the new and changed
  tests three times; the mutation pass on an exported tree of `dd7d1ec`, verified unchanged after the
  run; the full pocold release suite; e2e_devnet.py. `5560af7` adds documents and evidence only. No
  witness file is committed.

**journal** (evidence/journal-measure*.log, witness-refetch*.log, journal-r3/, journal-r3b/, journal-r3c/):
- Stage A `86b339e` and round 2 `47bfc11`: tests/journal_measure.rs and documents; src identical to
  `a03e089`'s. The re-fetch witness (tests/zz_audit_witness_refetch.rs) is gitignored; only its logs
  are committed.
- Round 3: `346246e` measurement tests (its first evidence pass was stopped and is not used); `7f587bb`
  the harness fix (signed blocks follow the ASERT schedule), the code under test of evidence/journal-r3/,
  src identical to `fa97594`'s; `256428f` report, evidence and tools/journal_projections.py, no code.
- Round 3b: `e3dfb47` six state-interacting kinds and jm_create_state_term (src identical to
  `fa97594`'s), the code under test of evidence/journal-r3b/; `11f8e89` report and evidence, no code;
  its derived.log [1]–[9] reproduce journal-r3/derived.log.
- Round 3c: `1d094de` jm_tx_state_term, pack_txs and zero_code_init (src identical to `fa97594`'s),
  the code under test of evidence/journal-r3c/ (txstate.log, create-r1/r2.log, suite log). The mutant
  logs ran on `1d094de` plus a scratch env-gated patch to lp4-execution src that is not committed and
  was reverted. `007d579` report, tools [12]–[15] and evidence, no code; its derived.log's [1]–[11]
  reproduce journal-r3b/derived.log, and its INDEX.md corrects journal-r3b/INDEX.md's section
  reference.

**e2e** (evidence/e2e-mutation/, e2e-mutation-r2/, e2e-mutation-r3/, e2e-mutation-r4/, e2e-integration/):
- Stage A: `bfeb226` is test-only; its prototypes/ tree differs from `a03e089`'s only in
  tests/e2e_devnet.py. `8fbb340` and `c195a89` add evidence only. Every run used pocold built from the
  `a03e089` code (base sha256 d542b31e…dc617).
- Round 2: `f45ed5c`, `faeb5da` and `f583c7f` are test-only (prototypes/ differs from `a03e089`'s only
  in tests/e2e_devnet.py); `f45ed5c` and `faeb5da` are superseded versions whose clean-run logs are in
  e2e-mutation-r2/superseded/. `8fd3d6e` adds evidence only. Every run used pocold built from the
  `c195a89` code tree (base sha256 71d36fe4…ca0f1; it differs from stage A's binary because the source
  path differs).
- Round 3: test commits `0e51a36`, `1512409` and `16bebfd` (tests/consensus.rs, E2E-R3-7); `d19f515`
  and `fd5c4d0` evidence only. The logs before 03:40Z ran from a lost worktree at `1512409`, the later
  ones from a worktree at `d19f515`, whose prototypes/ equals `1512409`'s. One truncated scratch log
  (M2) was excluded and re-run. The pre-restart logs (test `0e51a36`) are kept but not counted.
- Round 4: `ad84942` changes tests/e2e_devnet.py only (prototypes/ otherwise equal to `fd5c4d0`'s).
  `252dc1c` adds e2e-mutation-r4/ only (new files); its README corrects ../e2e-mutation-r3/README.md
  without editing it. All its logs ran at `ad84942` with a clean prototypes/. M4.log, a precondition
  failure, is kept but not counted.

**Integration and Windows (on the repair branch):**
- `5d25fae` (retention.rkeep: no directory fsync on Windows) and `a39379b` (peer read poll,
  TCP_NODELAY and whole RPC writes) are the Windows fixes of runs 1–2; `be95e26`, `49508a5` and
  `29fd44a` as above.
- `1cc96c4` changes one test's settings (tests/net.rs; HEAVY_QUEUE 2, QUEUE_WAIT 5 ms, up to 12 rounds)
  and adds the run-19/20 record; assertions unchanged.
- Evidence-only commits: `2a226d8`, `ecf8f2a`, `273e785`, `d40fd91`, `fa97594`, `5e1b2dd`, `a129656`,
  `49ae9fa`, `d91d141`.

---

## History: reconciliation as recorded for `3c3792e` (D-REORG-REDB final code)

Commit roles for the final verification, and proof that every evidence commit carries the same built
code (prototypes/ and .github/ trees), so the results apply to the final implementation.

| role | commit |
|---|---|
| audited snapshot | `090eae4` |
| last implementation commit (prototypes/ and .github/) | `3c3792e` |
| integration-tested commit | `3c3792e` (evidence/integration.json, last run) |
| witness-tested commit | `3c3792e` (evidence/witnesses-redb-final.log) |
| real-Chrome-tested commit | `3c3792e` (in the integration) |
| RSS-measured commit | `3c3792e` (evidence/rss-redb-final.json) |
| mutation-controlled commit | `3342af3`, whose Rust sources equal `3c3792e`'s. The two prototypes/ trees differ only in `tests/e2e_devnet.py`, by `git diff 3342af3 3c3792e -- prototypes` |
| spec-depth release runs | `441ce73` (evidence/spec-depth-441ce73.log). `prototypes/pocold/src/` is identical to `3c3792e`'s (`git diff 441ce73 3c3792e -- prototypes/pocold/src` is empty); the later commits only add and correct tests |

The debug-phase-A run (assert-release, `--include-ignored`) also runs both spec-depth tests on
`3c3792e` itself (REPAIR-REPORT §3b).

The integration's `dirty: true` is one uncommitted evidence file: the previous run's append to
`integration.json`, made by the failed run on 82ddb03 and committed later. No code was uncommitted. The
witness log header's "2 uncommitted paths" are evidence files written by the integration and the RSS
run.

## Tree equivalence (tools/reconcile.py)

```
| role | commit | prototypes/ tree | .github/ tree | code equal to the implementation commit |
|---|---|---|---|---|
| last implementation commit | `3c3792e90eb3c31c83abc91f26e861c8510a824f` | `85e63f96f225` | `347e2324e80c` | yes |
| integration-tested commit | `3c3792e90eb3c31c83abc91f26e861c8510a824f` | `85e63f96f225` | `347e2324e80c` | yes |
| witness-tested commit | `3c3792e90eb3c31c83abc91f26e861c8510a824f` | `85e63f96f225` | `347e2324e80c` | yes |
| real-Chrome-tested commit | `3c3792e90eb3c31c83abc91f26e861c8510a824f` | `85e63f96f225` | `347e2324e80c` | yes |
| PR head | `7e786137d66799c0b468e86d1fed8d47b8d1322e` | `85e63f96f225` | `347e2324e80c` | yes |

Commits after the implementation commit (1):

- `7e786137d667` audit-repair r2: final-code evidence for 3c3792e — integration, RSS, witnesses: 3 paths, all docs/meta (no code)

Result: every evidence commit and the head carry the implementation commit's code trees; later commits change development/ and root metadata (.gitignore) only.
```

---

## History: reconciliation as recorded for `687ac38` (before D-REORG-REDB)

### (round 2, 687ac38)

Commit roles for this round's verification, and proof that every evidence commit carries the same
built code (prototypes/ and .github/ trees), so the test results apply to the final implementation.

| role | commit |
|---|---|
| audited snapshot | `090eae4` |
| last implementation commit (prototypes/ and .github/) | `687ac38` |
| integration-tested commit | `c19dc8a` (code = 687ac38) |
| witness-tested commit | `ec09120` (code = 687ac38) |
| real-Chrome-tested commit | `c19dc8a` (in the integration) |
| PR head at this reconciliation | `ec09120` |

## Tree equivalence (tools/reconcile.py)

```
| role | commit | prototypes/ tree | .github/ tree | code equal to the implementation commit |
|---|---|---|---|---|
| last implementation commit | `687ac38be6c693ee2e92d65d77fa4dd90be2beb7` | `68ece272a3c7` | `347e2324e80c` | yes |
| integration-tested commit | `c19dc8a9f9fb38a41b7376e90152986965736d0d` | `68ece272a3c7` | `347e2324e80c` | yes |
| witness-tested commit | `ec091200f89d58c649cd49a23d2366b8501e1359` | `68ece272a3c7` | `347e2324e80c` | yes |
| real-Chrome-tested commit | `c19dc8a9f9fb38a41b7376e90152986965736d0d` | `68ece272a3c7` | `347e2324e80c` | yes |
| PR head | `d610492f9e4da579f9e88fbcaf3c05d54a59023a` | `68ece272a3c7` | `347e2324e80c` | yes |

Commits after the implementation commit (3):

- `c19dc8a9f9fb` audit-repair r2: document R_keep adoption depth 320 + D-REORG-REDB proposal; thread inventory evidence: 9 paths, all docs/meta (no code)
- `ec091200f89d` audit-repair r2: final integration on c19dc8a (code 687ac38): PASS but for the expected lp1 store_journal: 1 paths, all docs/meta (no code)
- `d610492f9e4d` audit-repair r2: gitignore the ephemeral witness programs (kept out of the public repo): 1 paths, all docs/meta (no code)

Result: every evidence commit and the head carry the implementation commit's code trees; later commits change development/ only.
```

Every role's prototypes/ tree is `68ece272a3c7` and .github/ tree is `347e2324e80c`: identical. The
commits after the implementation commit change only development/ docs and root metadata (.gitignore),
not the built code, so the integration, witness and Chrome results obtained on c19dc8a/ec09120 apply
to 687ac38 unchanged.

## Verification outcomes on the final code (separated, per the owner's instruction)

- **PASS (Linux):** lp4-execution 162, pocold 110 (incl. the two R_keep adoption tests), extension
  build + 234 unit, 3-node devnet 17, website publishing 20, real Chrome 79, integrated Chrome 18
  (evidence/integration.json, run on c19dc8a).
- **FAIL — expected platform limitation only:** lp1-node `store_journal` — 11 Windows-only tests
  executed on Linux and failed with UnsupportedPlatform, identically on the audited 090eae4. The
  integration records `failuresOnlyExpectedPlatformLimitation: true`. Not counted as a pass.
- **NOT RUN:** Windows verification (the scoped workflow returns 404 on dispatch until its file is on
  main; WINDOWS-VERIFICATION.md). Recorded separately, outstanding.
- **Witnesses (ec09120):** 17 observations, all NOT_REPRODUCED; 15 positive controls OK, 0 MISSING,
  0 erroneous REPRODUCED (evidence/witnesses-r2-final.log; A13 has no runtime witness; A02 contributes
  three). A NOT_REPRODUCED is an observation with its positive control, never an acceptance pass.
- **Measured RSS (final code):** evidence/rss-final.json (3-node devnet under idle + bounded
  adversarial load). Measured peak, not the model bound and not a guaranteed ceiling (A13-DERIVATION
  §2b).

## Witness provenance and positive controls

The witnesses were independently reconstructed from REPORT.md (the audit's own programs were not
received). Each prints `WITNESS <id> NOT_REPRODUCED` plus `CONTROL <id> OK|MISSING`, where the control
proves the scenario was built and the vulnerable path executed on the same build, so NOT_REPRODUCED
cannot mean the node refused to start, an API changed, or the path was never reached. Details:
evidence/WITNESS-CONTROLS.md.

## Restricted delivery of the exploit-shaped programs

The witness programs and the adversarial EVM measurement programs are NOT in this public repository
(the ephemeral copies are gitignored). They were delivered to the owner through the session's file
channel as a private review package; its files and SHA-256 hashes are recorded in
evidence/PRIVATE-REVIEW-PACKAGE.md (archive `fc46a03e…`; witnesses-r2.tgz `06730f27…`).
