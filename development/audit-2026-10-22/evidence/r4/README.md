# Round 4 evidence (2026-10-28)

Every result below names the tree it ran on. Runners for seeded faults and controls (mut.py, controls.py) stay
outside the repository; only their logs are here. Nothing here is an approval of an open decision
([../../proposals/DECISION-TABLE-2026-10-28.md](../../proposals/DECISION-TABLE-2026-10-28.md)).

Commits of this round (on claude/memory-r14-followup, after 260d021):

| commit | what |
|---|---|
| 3adc356 | pocold: a local storage failure (a state.redb commit, a journal read) stops the node (exit 70) and is never a validity verdict; the journal is streamed at open; `io-fault` test feature; OpenTiming |
| 16838eb | e2e: a failed run keeps its node logs, inputs and seeds before cleanup; `e2e_heal.mjs`; deterministic heals in `e2e_integrated` |
| 118afd7 | pocold: checkpoint open without per-record chain scans (prune cursor, one side scan, orphans only after a removal), with replay's version releases in the next block's commit; tests |
| e028068 | pocold: the independent review's findings F1–F7 (every I/O failure of the node's own storage stops it and keeps the store); e2e F8, F9 |
| 1074b47 | tests (the io-fault `Op::Read` match arm; the kill test keeps its child's stderr) and evidence |

## 1. Seeded faults RV10, RV11, RV12, and the defect they led to

- RV10 (stored_bytes leaves out the release table), RV11 (open does not release what a checkpoint owes),
  RV12 (a checkpoint write that fails on I/O leaves the owed flag unlike the table): each is now detected by a
  test, with deterministic I/O fault injection (`--features io-fault`: a `redb` file backend that fails one write
  or one fsync after a named step). None is classed as equivalent. [mutants-r4.log](mutants-r4.log): RL1–RL16,
  RV1–RV12 and LF1–LF5 all KILLED on 260d021 + the round's tree (LF5 first SURVIVED, then KILLED after
  `a_journal_read_that_fails_in_an_import_stops_the_node` was added; both lines are in the log).
- **Defect found by the fault tests:** a state.redb commit that failed on I/O was taken as `Reject::Block`: the
  valid block and its descendants were marked invalid ([io-fault-before-fix.log](io-fault-before-fix.log):
  `Err(Block(...))`, `1 blocks marked invalid`). Fixed in 3adc356 (`Reject::Local`: the node logs FATAL and exits
  70; the next open recovers). A failed fsync can leave the commit durable; the tests accept "undone or whole"
  for a sync fault and require "undone" for a write fault.

## 2. Reorganization at a heal, both directions

- Rust, three nodes over TCP with no miner: `d133_heal_c_reorganizes_onto_the_heavier_a_b_branch` and
  `d133_heal_a_and_b_reorganize_onto_the_heavier_c_branch` (consensus.rs). The heavier branch has fewer blocks
  (signed, height 6, work 24,794 against fallback, height 18, work 20,590), so height cannot stand in for work.
- e2e, real pocold processes: [e2e-heal-run.log](e2e-heal-run.log), 12/12 PASS on 16838eb. Each scenario
  stops every miner and measures each head alone before the heal, so nothing can change between measurement
  and heal. The first run failed ([e2e-heal-first-run.log](e2e-heal-first-run.log)): a and b both mined on the
  a/b side and ended on two sibling tips of equal work, which D133 leaves to local import order; the test now
  lets only a mine there. That run's kept directory was deleted by mistake; only its console log remains.
- Negative controls ([controls-heal.log](controls-heal.log)): K1 fork choice takes the lighter branch, K2 a node
  never reorganizes, K3 a node never syncs. Each fails both Rust tests and e2e_heal (K1 and K2 already at the
  common prefix, K3 at the heal checks).

## 3. Failure evidence kept automatically

`harness.mjs` `keepOnFailure` (e2e_devnet, e2e_site, e2e_browser with Playwright traces, e2e_integrated,
e2e_heal) and `devnet.py keep_on_failure`: on a failure, before cleanup, the run directory (node logs, the
network's inputs, the seeds) is copied to `POCOL_E2E_KEEP` or `tests/failures/`, without `*.redb`, with a
manifest of SHA-256 sums, the commit, the pocold binary's hash and the environment. The two earlier failures
whose cause was not proven stay open: [../memory-r14/accept-e453851/CORRECTIONS-2026-10-28.md](../memory-r14/accept-e453851/CORRECTIONS-2026-10-28.md)
(e2e_integrated run 1) and [../memory-r14/accept-ad301e7/CORRECTIONS-2026-10-28.md](../memory-r14/accept-ad301e7/CORRECTIONS-2026-10-28.md)
(BR18). A passing rerun does not explain either.

## Independent review of 3adc356, 16838eb, 118afd7

A reviewer that had not written the code read the three commits against 260d021 (no build, no run). It found no
blocking defect and ten findings. All of them are on 118afd7's tree or older, and the fixes are in e028068:

| # | finding | severity | fix | test |
|---|---|---|---|---|
| F1 | at open, a journal read error in a checkpoint restore counted as a refusal: every checkpoint was dropped in turn and state.redb deleted (a regression of the streamed journal, 3adc356) | should-fix | `Source::payload_at` separates a local failure (Err) from a refusal (Ok(None)); open fails and keeps everything | `e4_an_open_that_cannot_read_its_storage_fails_and_keeps_the_store` (journal read) |
| F2 | the same for a state.redb I/O failure in a restore, in dropping a refused checkpoint, or in reading the checkpoints | should-fix | `StateStore::io_failed` (any redb Io/PreviousIo error); open fails | the same test (a write while pinning a stored version; a write while dropping a refused checkpoint) |
| F3 | a history.redb read error in restoring a slimmed entry was `Reject::Block` | should-fix | `Reject::Local` | none: there is no fault injection for history.redb |
| F4 | a live checkpoint write or release step that failed on I/O was only logged; a read before the next commit could panic (exit 101) | should-fix | the node stops at once (exit 70); in replay, open fails | `e4_a_state_store_io_fault_stops_the_node_and_marks_no_block_invalid` (checkpoint modes added; release modes must stop in the step) |
| F5, F6 | a late release that contradicts the counts failed the commit and was lost; a failed commit dropped what waited | minor | the store is marked damaged (rebuilt at the next open); what waited is put back | none beyond the late-release test |
| F7 | replay's read back of a record was not bound again to the rolling hash | minor | it is | none |
| F8 | e2e_integrated could measure a and b on different tips | minor | a catch-up phase before the measurement | the acceptance runs |
| F9 | kept evidence recorded every POCOL/E2E/PLAYWRIGHT variable | minor | only the variables the drivers read | — |
| F10 | the io-fault tests run only with the feature | minor | not changed: the Windows workflow runs them (step pocold-io-fault) and so does this round's suite | — |

Seeded faults for the fixes ([mutants-review.log](mutants-review.log)): 5 of 5 killed. Two survived the first
round (a release step only logged; a failed drop of a refused checkpoint); the tests were strengthened and
both were then killed.

## 5. Checkpoint open

Input: the store a node at the network's state caps left (949,767 accounts, 3.8 M slots, 1.36 M trie nodes,
state.redb 2.06 GB), copied fresh for each run; the open resumes from the checkpoint at record 28,672 and replays
855 records. Every run ends on the same head. The 262–322 s of the earlier rounds were measured with other work
on the host; on this host and input, 16838eb takes 235–253 s (six runs).

Where the time went (16838eb with timers, [open/baseline-fine.log](open/baseline-fine.log), [open/prof-after.log](open/prof-after.log)):

| part | time |
|---|---:|
| block commits to state.redb (896 durable transactions, about 140 ms each) | 126 s |
| releases of the versions entries stopped holding (867 transactions, about 45 ms each; counted inside "retain") | 39 s |
| the rest of retain: whole-chain walks at every record (prune, three side scans, the orphan pass) | 33–36 s |
| restore of the checkpoint (each stored version read and checked) | 15 s |
| reset of the pins left by the previous run | 9–13 s |
| execution of the 855 blocks | 5 s |

Changes (118afd7), none of which skips a check or a durable write:
- the R21 prune visits each height once, and again only where a reorg, a rebuild or a restore could have
  given a block a state back (`Chain::prune_cursor`); debug builds assert the invariant;
- the side blocks are found once per retain, and the orphan pass runs only after a removal;
- during replay, a released version waits for the next block's commit and is released in that durable
  transaction (`StateStore::set_deferred`); a crash in between leaves the version pinned, which the next open
  resets, as after any crash.

Result, same input, alternating runs ([open/before-after-final.log](open/before-after-final.log)):

| | before (16838eb) | after (118afd7's node code) |
|---|---:|---:|
| open | 245.0 s, 243.1 s | 207.1 s, 214.5 s |
| retain during replay | 73.7 s, 74.5 s | 14.0 s, 16.1 s |
| validate (execution and block commits) | 132.6 s, 130.2 s | 156.1 s, 160.7 s (the releases now run inside the commits) |

What is left is the durable block commits (about 150 s). Not done: making them cheaper. One attempt was
measured and reverted: commits without an fsync during replay let state.redb grow from 1.3 GB to more than
5.6 GB in 90 s, because redb reuses no freed page until a durable commit; the disk filled and the node stopped
with a local storage failure, as designed ([open/unsynced-commits-fill-the-disk.log](open/unsynced-commits-fill-the-disk.log)).
The same full disk stopped the H_END run (§4) with exit 70.

Tests: `retain_prunes_each_canonical_height_once_and_again_where_a_state_came_back`,
`retain_prunes_side_blocks_made_canonical_below_the_cut_and_leaves_no_orphan`,
`a_late_release_leaves_the_rows_an_immediate_one_leaves`, the replay crash cases added to
`e4_checkpoint_crash_at_each_step`. Seeded faults: [mutants-prune.log](mutants-prune.log) (6 of 7 killed; the
survivor, a side-list reuse, was removed from the code), [mutants-late.log](mutants-late.log) (4 of 4 killed
after the test was strengthened; the earlier survivors are kept in the log).

## 4. Memory and storage

**Two different quantities.** A13's model is a computed bound: `resources::derive` adds a worst case for every term
(the pool at its gas bound, a code set at CODE_CAP, the largest execution, a block's batch at the most writes,
RPC and peers at their caps, the abandoned window of a deep reorg). What a node measured (RSS, VmHWM) is one
host and one workload. Neither stands in for the other, and nothing below is closed by an extrapolation.

**5.25 GiB and 6 GiB are one condition.** A13 requires model × 8/7 ≤ budget, so for the 6 GiB budget the model
must stay at or below 6 × 7/8 = 5.25 GiB; the 1/8 is for what the model does not count (allocator fragmentation,
per-allocation headers). The network profile in store mode, computed:

| | model | model × 8/7 | against 6 GiB (6,442,450,944 B) |
|---|---:|---:|---|
| store mode, on 1074b47 | 5,581,321,485 B (5.198 GiB) | 6,378,653,126 B | passes, 0.052 GiB under 5.25 |
| store mode, with the side set below (+294,912 B: 6,144 side entries × 48 B) | 5,581,616,397 B (5.198 GiB) | 6,378,990,168 B | passes, 0.052 GiB under 5.25 |
| store mode, the side set as its own term after review F-C ((side entries + r_keep) × 76 B) | 5,581,944,077 B (5.199 GiB) | 6,379,364,660 B | passes, 0.051 GiB under 5.25 |
| store mode at the O2 caps (decision row 3) | 5,617,166,961 B (5.231 GiB) | 6,419,619,384 B | passes, 0.019 GiB under 5.25 |
| whole-state mode (the default) | 26,819,799,029 B | 30,651,198,891 B | refused |

The largest store-mode terms (`a13_store_model_at_the_network_profile`): a code set read by one execution
(1.125 GiB), execution memory at the worst gas split (0.979 GiB), a block's new trie nodes before commit
(0.916 GiB), state.redb's allocator over its largest file (0.391 GiB), the node cache (0.250 GiB).

**Measured, store mode, at the network's state caps** (949,767 accounts, 3.8 M slots; `jm_store_scale`, Linux):

| what ran | VmHWM | source |
|---|---:|---|
| open from a checkpoint (855 records replayed) | 454–455 MB | [open/before-after-final.log](open/before-after-final.log), [load/](load/) |
| the pool at its gas bound (4 creations of 7.95 M gas, 1,424 fresh slots in its layer), then a full-gas block of fresh slots and the pool revalidated | 455–471 MB | [load/](load/) |
| then a reorg from 24 blocks below the head onto 25 blocks, 13 of them full-gas fresh-slot blocks (20.7 s) | 455 MB (no rise after the open) | [load/](load/README.md) |
| round 3: growth to the caps, a reorg of 8, three checkpoints | 474 MB (539 MB before that round's fix 1) | [../memory-r14/scale-d30dc71/](../memory-r14/scale-d30dc71/README.md) |
| H_END (129,600 empty blocks, devnet profile with the network's H_END) | 486 MB | [hend/continue.raw](hend/continue.raw) |

The measured peaks are below a tenth of the model. That does not show the model is loose: the workloads do not
reach the large terms (no execution near its memory bound, no code set near CODE_CAP, no RPC or peers, no
deep-reorg window at the caps). The terms they reach behave as counted: the stored versions are a fifth of
the per-version model, the node cache sits at its bound, the pool's layer holds what its gas wrote. **Not
measured:** the execution, code-set and batch terms at their worst together, RPC and peers at the caps, a
reorg of R_keep depth at the caps, Windows.

**Journal growth to H_END.**
- Measured, empty blocks: 2,011 B per record, 260,616,349 B at H_END ([hend/continue.raw](hend/continue.raw)).
- Bound for the canonical chain: about 11.3 GiB at the network caps. It comes from the cumulative caps
  (TXBYTES_CAP 8 GiB, the identity bound on evidence, headers, parentResult), not from H_END × BODY_MAX
  ([../../JOURNAL-GROWTH-AND-REPLAY.md](../../JOURNAL-GROWTH-AND-REPLAY.md) § 2.1).
- No bound for side blocks: a block evicted and fetched again is journaled again, once per cycle while it stays
  in the R_keep window (same document, § 4; decision row 5).
- The H_END journal at full blocks was not measured.

**What the H_END runs found.**
- An import's time grew with height: 8 → 165 ms per empty block on the first run, 16 → 72 ms by h = 77,760 on
  the second (eb85bed), and quadratic over a replay from genesis. Two causes were found by stack samples:
  - `rebuild_canonical`'s walk to genesis at every insert, fixed in eb85bed;
  - the side-block scans of `retain_unsettled` and `settle`, and settle's trie pass, each over every entry.
    The commit after 719da04 keeps the side blocks as a set and clears only the tries given since the last settle.
    Seeded faults ([mutants-side.log](mutants-side.log)): S1–S6, S8, S9 killed in release; S7 (a restore keeps the
    old set) and S10 (shed skips the lowest kept height) survived, and were killed after a test was added for
    each. S11 was shown equivalent (shed runs after settle has pruned or made full every side state), and the
    redundant code was removed.
  - Measured since: an H_END run on b375d4c, which has the side set (below).
- The first reopen at H_END did not use its checkpoints and replayed from genesis. Why is not known: its
  node log was off, and the store was started over. The second run (eb85bed, node log on) was stopped at h ≈ 114,901 by
  the session's 2-hour limit on a background command ([hend/grow2-eb85bed.log](hend/grow2-eb85bed.log)). Its reopen
  started the store over too, and its log lines were lost ([hend/reopen2-eb85bed.md](hend/reopen2-eb85bed.md)). On
  051dfaa, reopens at small heights resume from their checkpoints (a clean stop at 20,000, a `kill -9` at
  12,651).
- **Cause found** by a run to H_END on 5cd7d43 with the log written straight to a file
  ([hend/hend-5cd7d43-grow-and-reopen.log](hend/hend-5cd7d43-grow-and-reopen.log)):
  - the log reads `checkpoints unusable, the state store is rebuilt: checkpoint 124: snapshot count beyond its bytes`;
  - `checkpoint::decode` guarded the entry count with a record size of 134 bytes, but `put_rec` writes 126 (five u64
    fields, not six);
  - a snapshot of mostly minimal entries (no summary, no root: the slimmed ones, past about 100,000 heights) was
    refused, every checkpoint with it, and open rebuilt from genesis.

  Fixed in the commit after 5cd7d43 (`REC_MIN`), with `a_snapshot_of_minimal_entries_reads_back`, which fails
  without the fix.
- **That run on 5cd7d43:** grow to H_END in 2,184 s, median import 10–12 ms in every segment (165 ms at the end of
  the first run). VmHWM 601 MB at H_END (486 MB on the first run, which held fewer side and trie entries).
- **Verified at H_END on b375d4c** (the `REC_MIN` fix, 50f9207, plus evidence only;
  [hend/hend-b375d4c-grow-and-reopen.log](hend/hend-b375d4c-grow-and-reopen.log)): grow to 129,600 in 2,304 s, median
  import 10–15 ms in every segment, VmHWM 600 MB. The reopen **resumed from checkpoint 129,024** and replayed 576
  records: open 13.3 s (journal 1.9 s, restore 5.4 s, replay 5.5 s), VmHWM after open 391 MB. On 5cd7d43 the same
  reopen was refused and replayed from genesis (about 1,272 s). Files at H_END: journal 260.6 MB (2,011 B per empty
  record), state.redb 101.5 MB, history.redb 81.7 MB. Empty blocks only: not a worst load.

## 6. Boot and performance

All on 1074b47 with the H_END run (§4, nice 10) beside them; logs in [boot/](boot/). (During the last log,
modexp-random, two documents were being edited; no code.)

**R21, R22 and A13 with their actual values:** [../../BOOT-BLOCKERS-2026-10-28.md](../../BOOT-BLOCKERS-2026-10-28.md), sorted by cause
(no approved genesis or measured input, implementation gaps, decisions not taken, resource limits);
[boot/boot-gates.log](boot/boot-gates.log). The network profile does not boot: R21 refuses in both modes, and
A13 passes only in store mode.

**R14/R19 calibration** ([boot/r14-boot-timing.log](boot/r14-boot-timing.log)): the boot gate times one compute loop.
- Phases: 6–10 ms in memory and 8–15 ms through the store, against t_exec = 1,000 ms (devnet).
- In the network gate: 7, 9 and 11 ms against 3,000 ms.
- It passes without timing the slowest blocks (condition `timing-not-a-bound`, decision row 2, not approved,
  not changed here).

**The slowest valid blocks measured** (one transaction of 7.95 M gas, `POCOL_ROOT_CHECK=0` as in the node binary):

| set | slowest | validate | import |
|---|---|---:|---:|
| workload gap, 16 kinds × 2 states × 2 modes ([boot/r14-workload-gap.log](boot/r14-workload-gap.log)) | MODEXP, 16-byte even modulus, grown state, whole-state mode | median 328 ms (41 ns/gas) | 359 ms |
| shape sweep, 385 shapes ([boot/modexp-sweep.log](boot/modexp-sweep.log)) | MODEXP, 16-byte modulus 2·odd, 8,192-bit exponent | median 400 ms, min 371 ms (50 ns/gas) | — |
| seeded random, 3,000 inputs, seed 0x5eed20261028 ([boot/modexp-random.log](boot/modexp-random.log)) | 9-byte base and modulus, 54-byte exponent | median 277 ms (projected 346 ms) | — |

- Every measured block is within t_exec = 1 s, by about 2.5×.
- The worst figure of round 3 was 0.47 s to validate and 0.51 s to import. This round's worst is 0.40 s, on
  the same host class under a different load.
- The grid and the random set are not a proof of the maximum.
- What would bound it is decision row 2 (calibrating on the slowest kinds), which is open.

**Not changed:** no limit raised and no consensus rule changed. The missing network genesis, the implementation
gaps (K_eff, a network admission path, the RpcGuard mechanisms) and the resource limits are listed apart in
BOOT-BLOCKERS.

## Suites

| log | tree | release, all tests | io-fault | crash-test | with debug assertions |
|---|---|---|---|---|---|
| [suite-early.log](suite-early.log), [suite-early-iofault.log](suite-early-iofault.log) | 3adc356's tree before commit | 250 / 0 (28 ignored) | 106 / 0 | — | — |
| [suite-118afd7.log](suite-118afd7.log) | 118afd7 | 254 / 0 (30 ignored) | 110 / 0 | 1 / 0 | plain debug build: consensus 25 / 66 (see below), state_store 14 / 0 |
| [suite-e028068.log](suite-e028068.log) | e028068 (+ two test-only fixes, then committed in 1074b47) | 254 / 0 | 111 / 0 (rerun with the fixes) | 1 / 0 | consensus 89 / 2 (see below), state_store 14 / 0 |
| [suite-eb85bed.log](suite-eb85bed.log) | eb85bed | 254 / 0 | — | — | — |
| [suite-side-release.log](suite-side-release.log), [suite-side-debug-assertions.log](suite-side-debug-assertions.log) | 719da04 + the side set (the commit after it, before its last test additions) | 252 / 2 (30 ignored; see below) | 111 / 0 | 1 / 0 | caps 11, consensus 91, net 53, state_store 14, store 12; 0 failed |
| [suite-side-final2.log](suite-side-final2.log) | 5cd7d43 | consensus, network_budget | — | — | consensus, state_store; 206 / 0 in all |
| [suite-recmin.log](suite-recmin.log) | 50f9207 | consensus, state_store, network_budget | — | — | consensus; 208 / 0 in all |
| [review-2/suite-review-fixes.log](review-2/suite-review-fixes.log) | 1bba829 (the review's F-A, F-B, F-C) | 256 / 0 (30 ignored) | 113 / 0 | 1 / 0 | consensus, state_store 107 / 0 |

(passed / failed.) Failures, each kept in its log:
- **118afd7, plain debug build:** 64 tests panic at `tests/consensus.rs:15`, the devnet admission. An unoptimized
  build is refused by the boot timing gate (R14/R19), as in earlier rounds. The other two failures are
  timing-bound tests (a submission's deadline, a long catch-up) in that build. From e028068 on, the debug-assertion
  runs use a release build with `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true` in a separate target directory.
- **e028068, release with debug assertions:** two tests (`a_damaged_history_store_is_never_served_and_is_rebuilt`,
  `e4_checkpoint_damage_falls_back_to_a_verified_state`) panicked at the same admission line, with the H_END run
  beside them. The runner's filter did not keep the refusal's text. It did not recur in the one later run with debug
  assertions on ([suite-side-debug-assertions.log](suite-side-debug-assertions.log): consensus 91 / 0). **The cause is not established.** It is the same symptom as the Windows refusal on
  1074b47 (run 40680583220, whole-state R14/R19), which also remains open. The 1074b47 commit message called both
  "the admission timing gate under load and passed when rerun": that the rerun passed is true, the cause was not shown.
- **The side-set tree, release:** two failures, with seeded-fault builds and the H_END run beside the suite (load
  about 20 on 4 CPUs). Reruns are in [suite-side-reruns.log](suite-side-reruns.log).
  - `a_submission_during_a_long_backlog_is_busy_at_its_deadline_and_not_pooled` (consensus.rs:752, a reply within
    its deadline plus 400 ms) passed 5 of 5 reruns. The suite's failure is not explained by that.
  - `reconnect_churn_keeps_threads_and_slots_bounded` (net.rs:127, "still accepts peers": a new peer is not live
    after 200 churned connections) fails on 719da04 too under the same load: 3 of 6 there, 1 of 6 on this tree,
    alternating. A wait of 3 s in place of 500 ms still failed 4 of 6, so it is not a latency bound; the change
    was reverted. **The cause is not established;** it is older than this round's change and stays open.
