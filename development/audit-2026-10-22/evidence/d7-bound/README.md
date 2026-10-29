# evidence/d7-bound: D-7 at 4 × GAS_LIMIT, the code-loading audit, store mode run end to end (2026-10-25)

What these files show, on which commit, and what was not run. Runners for mutations and witnesses are kept out
of the repository; only their logs are here. A13-DERIVATION.md §2d has the derivation; DECISION-PACKAGE.md
("After D-7 and the code-loading audit") has the numbers and the options.

## Commits

| commit | what |
|---|---|
| `ee14458` | D-7 bound, its behaviours and tests; A13 pool, overlay and code terms after the audit; `--state-store`; the churn measurement |
| `9bd9bc0` | adds `jm_r14_import_by_mode`, an ignored measurement (tests/journal_measure.rs only) |
| `919a50c` | e2e_integrated: v3 is published once the pools settle after the heal (pocol-extension/tests only; see below) |
| `8584283` | a transaction returned from an abandoned block is offered to the peers (node.rs, net.rs; two tests; e2e_integrated prints the pools on a timeout) |
| `d939d1a`, `0833b6c` | e2e_browser: a navigation race and a receipt read once across a short fork (pocol-extension/tests only) |

The node code is the same in the first three; 8584283 changes the relay of returned transactions only. **Machine** (Linux runs): Intel Xeon @ 2.80 GHz, 4 vCPU under KVM,
15 GiB RAM, ext4 on virtio; Linux 6.18.44; rustc 1.99.1 (lp4, pocold) and 1.58.1 (lp1-node); Node 22.22.0;
Playwright's Chromium.

## Acceptance on 9bd9bc0 (accept-9bd9bc0/, one log per step with its command, times and exit code)

`HEAD` records "untracked-or-dirty-prototypes 2": the `pocold/target` and `pocol-extension/node_modules`
symlinks; no tracked file differed.

| step | result |
|---|---|
| lp4-release | **193 passed, 0 failed** |
| pocold-release (`--no-fail-fast`) | **229 passed, 0 failed, 22 ignored** (the 20 of the last pass, plus `jm_store_mode_churn` and `jm_r14_import_by_mode`, run below) |
| pocold-crash-steps (`--features crash-test`) | **passed** |
| pocold-kills-damage | **passed** (E44-KILLS, E44-DAMAGE in the log) |
| pocold-spec-depth-store | **passed**: store mode at the spec depth 2048/2049, 306.6 s |
| pocold-ignored-deep | **passed**: adoption at 2048 and not 2049, D133 at depth 400, A11 deep reorg |
| a13-models | the network and devnet bounds, both modes (below) |
| d7-hold, d7-run-loop | **passed** (below) |
| lp1-node-1.58.1 | 137 passed, **11 failed**, exit 101: all 11 are tests/store_journal.rs, which return UnsupportedPlatform (that journal is Windows-only). **Not a pass on Linux**; they pass on Windows (run 64) |
| ext-build, ext-unit | build passed; **234 of 234** |
| e2e, whole-state mode (default) | devnet **PASS 43**; site **PASS 21**; browser (Chromium) **PASS 80**; integrated **FAIL, 1 of 11** (below) |
| e2e, store mode (`POCOLD_RUN_ARGS=--state-store`) | devnet **PASS 43**; site **PASS 21**; browser **PASS 80**; integrated **PASS 19** |
| rss-load-whole-state, rss-load-store | measured (below) |
| jm-r14-whole-state, jm-r14-store, jm-churn-30 | measured (below) |

Each e2e suite now checks that every node runs in the requested mode (`pocol_status.stateStore`); that check
is one of the counts above (42 → 43, 20 → 21, 79 → 80, 18 → 19).

**e2e_integrated, whole-state, on 9bd9bc0: a race in the test, not a node defect.**
- After the heal, node c returned the v2 transactions of its abandoned branch to its pool (consensus.md:
  excluded transactions return after they are checked again). They carry the publisher's next nonces.
- The first v3 transaction, sent through b with nonce 11, lost to c's own nonce-11 transaction from the
  abandoned branch, because c proposed first. The canonical blocks 47–50 hold nonces 11–17, all from the
  abandoned branch.
- The v3 transaction became invalid, and the publisher waited out its receipt (300 s).
- D-7's bound was not reached: the returned transactions declare 8.36M gas, against the 32M bound.
- 919a50c makes the test wait until every pool and revalidation backlog is empty before publishing v3, and
  records that as a check. Its reruns found a node defect, below.

## e2e_integrated on 919a50c: returned transactions held by one node (integrated-919a50c/)

Four runs: whole-state 1 **PASS**, store 1 **PASS**, whole-state 2 **FAIL**, store 2 **FAIL**. A further
whole-state run, with the pools printed on a timeout (the print is in 8584283), also **FAILED**. Each failure
was the same: "timeout: pools settled after the heal", 240 s. In the diagnostic run, a and b's pools were
empty and c held 7 transactions (8,356,420 gas) at height 254.

What the chains show (proposers-*.json; a copy of c's data directory opened read-only over RPC, then
removed):
- After the heal, c was not the drawn proposer at any height, and won none: 0 of 223 (whole-state 2), 0 of
  224 (store 2) and 0 of 215 (diagnostic). In the passing whole-state 1 it was drawn 17 times of 63.
- Only Draw(h,0) builds a block with transactions. Every other node locks that proposer's template. c's
  returned transactions were relayed to nobody (relay off for re-inserted ones), so they could only be
  included by c.
- Why c was not drawn (heal-depth.txt): the active set is A_h = {lastShare ≥ h−W_act or graceStart ≥
  h−W_act} (consensus.md §2), with W_act = 32.
  - In the failing runs the majority branch was 31, 59 and 61 heights past the fork point at the heal; in
    the passing run, 12.
  - A copy of c's directory opened with `--miner c` reports "mining=false": c has no assignment on the
    healed chain. It returns only through `reactivate`.
  - **That is the specified membership behaviour, and it is unchanged.** How long the partition lasts
    depends on how fast v2 is mined on c, so the test passes or fails by timing.
- **The defect:** a transaction that only this node holds, returned from an abandoned block, never reached
  the nodes that build blocks.
- **The fix (8584283, local relay policy):** each returned transaction admitted again is offered to every
  peer through `Peer::offer`.
  - A peer with half its queue in use does not get it and is kept.
  - The previous pool's transactions are not sent again.
  - `pocol_status` reports `stats.reinsertedOffered` and `stats.reinsertedUnoffered`.
  - Tests: `transactions_returned_from_an_abandoned_block_are_offered_to_the_peers` (consensus.rs, a loopback
    peer) and `an_offer_past_half_the_queue_is_refused_and_keeps_the_peer` (net.rs).
- **Mutation controls (mutants-relay.log): 6 of 7 killed.**
  - R5 survives: the offer arm calling `send` instead of `offer`. Telling them apart needs a peer whose
    queue is half full while transactions return, which a loopback test does not produce deterministically.
    It is a survivor, not an equivalent mutant.
  - R6 and R7 (`offer`'s own limit and its keeping the peer) are killed by the net.rs test.
- Still open: a returned transaction that a peer refused, or that no peer had room for, stays here until this
  node is drawn; it is not offered again.

## Acceptance on 8584283 (accept-8584283/; the node code of the relay fix)

`HEAD` records "untracked-or-dirty-prototypes 2": the `pocold/target` and `pocol-extension/node_modules`
symlinks; no tracked file differed.

| step | result |
|---|---|
| pocold-release (`--no-fail-fast`) | **231 passed, 0 failed, 22 ignored** (229 before, plus the two relay tests) |
| pocold-bins, ext-build | built |
| e2e, whole-state mode | devnet **PASS 43**; site **PASS 21**; browser **FAIL 1 of 49** (a race in the test, below); integrated **PASS 20, three runs of three** |
| e2e, store mode (`--state-store`) | devnet **PASS 43**; site **PASS 21**; browser **PASS 80**; integrated **PASS 20, three runs of three** |
| witnesses | witnesses-8584283.log, and the rerun below |
| Windows | run 66 on 8584283 **passed every step** |

- **e2e_integrated** now has 20 checks (19 before): the settle wait of 919a50c is one.
- In each of the six runs, node c returned 7 transactions after the heal and offered them all
  (`reinsertedOffered` 7, `reinsertedUnoffered` 0, in the "heal" check's stats). The pools settled before
  v3.
- Not rerun on 8584283, because the change does not touch them (their last runs are on 9bd9bc0, above):
  - lp4;
  - lp1-node;
  - the crash, kill and damage steps;
  - the spec-depth and deep-reorg tests;
  - the A13 models;
  - the RSS and R14 measurements;
  - the churn measurement.

**e2e_browser, whole-state, on 8584283: a race in the test.**
- The step that failed: "frame.evaluate: Frame was detached", at the navigation to "/" after BR18c.
- The test called `pocol.navigate` inside `fr.evaluate`. The navigation tears the frame down, and the
  teardown can land before Playwright receives the evaluate's answer.
- The same call passed at the three navigations before it, and in every earlier run. It does not involve
  the node.
- d939d1a schedules the navigation for a later task (`setTimeout`), so the evaluate answers first. What each
  step checks is unchanged.

**Browser reruns.**
- **On d939d1a** (browser-d939d1a/): whole-state **PASS 80, three runs of three**; store **PASS 80 once,
  FAIL twice**. Both failures were "timeout: receipts" at BR14 (browser-d939d1a/forks.txt):
  - BR14 read node a's receipt once and then waited for b and c to report the same block hash;
  - at the inclusion height the nodes had different blocks for the same template (a short fork, two-way in
    one run and three-way in the other);
  - a then reorganized, and the receipt it had answered named a block no longer canonical;
  - a copy of a's directory, opened afterwards, answers with the canonical block (`find_tx` takes the
    canonical location), so the node is right and the test was not.
- **0833b6c** reads all three nodes at each poll and requires one block hash.
- **On 0833b6c** (browser-0833b6c/): **PASS 80 in three runs of three per mode**, six of six.

**Witnesses on 8584283** (witnesses-8584283.log):
- These ran **NOT_REPRODUCED, with their positive controls OK**: A01, A03, A04, A11, A06 (three cases), A07,
  A08, A09, A12, A14.
- These did not run, because the disk allowance ran out during their builds (the log has the linker and
  write errors): A02 (lp4) and the live A05 and A10.
- Space was freed (old build directories in the scratchpad), and they were run again
  (witnesses-8584283-rerun.log):
  - **NOT_REPRODUCED, with their positive controls OK**: A02 (R9, R13, SYS_SLOT_MAX), A05 and A10;
  - A01, A03, A04 and A11 ran again with the same result.
- So every witness from A01 to A14 has run on 8584283. They ran in whole-state mode, with variant v2.
- A NOT_REPRODUCED is an observation, not acceptance.

## D-7 (accept-9bd9bc0/d7-*.log)

- **Hold at the bound** (`d7_pool_revalidation_hold_under_a_heavy_pool`): four BLAKE2F transactions of 7.9M
  gas, at most what the 32M bound holds.
  - Sliced: the import held the thread 184 ms, and the longest slice 193 ms (one transaction, 193 ms).
  - Revalidating at once: the import held it 627 ms.
- **Consensus during revalidation** (`pool_revalidation_at_the_gas_bound_does_not_hold_up_block_import`,
  under `Node::run`): the second block was imported while revalidation was pending. The second block and an
  RPC answer took 736 ms, against 189 ms for one transaction.
- **Counters** (`the_pool_counters_equal_the_contents_through_admission_revalidation_and_reorgs`): equal to
  the contents after every one of 160 steps. Count of each kind of step: 31 admitted, 41 refused for gas,
  2 relays in a backlog, 12 heads, 13 reorganizations.
- **Mutation controls** (mutants.log): **18 of 18 killed, none classed equivalent** (the relay of returned
  transactions has its own, mutants-relay.log, above). They cover the bound,
  the counters, the order of re-insertion, the RPC field, the slicing, and the A13 terms (unpaid slot loads,
  the count cap, the CODE_CAP cap, the SYSTEM phase in the overlay, the bound check, the whole-state pool
  layer, the abandoned body).

## The code-loading audit (A13-DERIVATION §2d)

- **witness-slots-9bd9bc0.log.** A witness run on a local node: one transaction of 1,000,000 gas left 2,000
  zero-valued slot entries in the pool's layer. Counting one slot per 2,900 gas of writes allows 344. The
  program is kept outside the repository.
- **a13-models.log** (computed bounds, not RAM):

  | profile | whole-state mode | store mode |
  |---|---:|---:|
  | network | 25.24 GiB (gate 28.85) | 7.61 GiB (gate 8.69) |
  | devnet | 3.04 GiB (gate 3.47) | 3.48 GiB (gate 3.98) |

  The network store-mode file is 14.3 TiB (adversarial), or 3,653 regions.

## Measurements (accept-9bd9bc0/; this host; not bounds)

- **RSS under load** (tools/measure_rss.py; 60 s idle, then 60 s of RPC load and transfers; 3 nodes):

  | mode | requests answered (light, heavy, batch, tx) | peak RSS per node | model each node reports | peak over model |
  |---|---|---:|---:|---:|
  | whole-state | 19,954, 1,083, 96, 180 | 12.8–13.0 MB | 3.06 GiB | 0.0039 |
  | store | 17,774, 969, 84, 175 | 15.5–15.7 MB | 3.51 GiB | 0.0042 |

  - The store run's nodes report the store-mode model, which identifies the mode (the tool does not record
    `stateStore`).
  - The load does not fill the pool to its gas bound, and it does not test the bound.
- **R14/R19 against the modes** (`jm_r14_import_by_mode`): import times of 10 GAS_LIMIT compute blocks, the
  admission timing's workload:

  | mode | median | max |
  |---|---:|---:|
  | whole-state | 29 ms | 32 ms |
  | store | 64 ms | 69 ms |

  - t_exec is 1,000 ms.
  - This workload writes little state. The admission timing (admission::measure) applies blocks to an
    in-memory state, so it does not include store mode's commit and fsync.
  - The state-heavy blocks of evidence/c1-e4/disk-impact-final.md took up to 4.8 s (an fsync outlier) and
    1.05 s in store mode.
  - **So R14/R19 as measured do not cover store mode's import of state-heavy blocks.** Calibrating them in
    the mode the node boots is open.
- **state.redb under churn** (`jm_store_mode_churn`, 30 cycles). Each cycle writes 1,200 new slots, rewrites
  them, and deletes the previous cycle's 1,200.
  - The file reached 67.9 MB at cycle 5 and stayed there through cycle 29 and a reopen.
  - The stored bytes stayed between 8.5 and 10.7 MB.
  - File over stored was 6.35–7.95 from cycle 5 on (at most 7.95 overall); allocated over stored at most
    1.83.
  - Freed pages are reused, so the file does not grow under steady churn. **This is a measured factor on a
    10 MB store, not a bound** for the adversarial file (condition `store-file-factor`); A13 counts 12.

## After the GitHub break (2026-10-25, local commits 78fe971, 90c4185, 24cc167; not pushed yet)

GitHub refused this session's credentials from 15:46 UTC. These commits are local until access returns (see
../../pr27/RESUME.md).

**Relay mutant R5 killed (90c4185).**
- `node::relay_tests::a_returned_transaction_is_not_queued_to_a_peer_past_half_its_queue` gives the node a
  test peer whose outbox has no writer thread.
- With the queue empty the returned transaction is queued. Past half the queue and below the whole of it,
  it is not, although gossip of it would still fit. The peer stays connected.
- Relay mutants rerun (mutants-relay-2.log): **7 of 7 killed**.

**R14/R19 for a store-mode boot (24cc167; r14-store-boot/).**
- A store-mode node was admitted on the in-memory timing.
- Now `load_spec_for` also runs the calibration blocks through a scratch state store at
  `<data>/calibration.store`, on the node's own disk, and the gate takes the larger time per phase:
  - `apply_block_on`, the trie update, the root and a durable commit fall inside each block's timed span;
  - the scratch store is removed before the node opens.
- Suites on its prototypes tree (`c73e863f`): **lp4 196 passed, 0 failed; pocold 233 passed, 0 failed, 23
  ignored** (the new measurement is the 23rd).
- **Mutation controls (r14-store-boot/mutants.log): 11 of 12 killed.**
  - C12 (the leftover scratch store of a crash is not cleared first) survives.
  - Without the pre-clear, `StateStore::open` moves an unreadable leftover aside or reuses a readable one.
    The directory is removed afterwards either way, so the only difference is that the measurement may run
    on a store that is not fresh. No test observes that. It is not classed as equivalent.
- **Measured on this host** (jm-boot-timing in accept-24cc167/, default devnet profile, ms per phase:
  ordinary, closing, final): in memory 8–18 ms, through the store 10–20 ms, so the gate sees 10–20 ms, over 10 runs; t_exec is 1,000 ms. An
  earlier 5-run measurement on the working tree gave 9–14 ms and 11–26 ms.
- **Not covered:** the calibration writes little state. State-heavy blocks exceed t_exec = 1 s in both modes
  on this host (../c1-e4/disk-impact-final.md: up to 1,068.5 ms whole-state, 1,045.1 ms store, one outlier
  at 4,841.8 ms). Calibrating on them is a decision, presented in DECISION-PACKAGE.md and not adopted.

**Acceptance on 24cc167 (accept-24cc167/).** Prototypes tree `c73e863f` (`HEAD` records the two usual symlinks).

| step | result |
|---|---|
| pocold-bins, ext-build | built |
| jm-boot-timing (10 runs) | as above |
| e2e, whole-state mode | devnet **PASS 44**; site **PASS 21**; browser **PASS 80**; integrated **PASS 20, two runs of two** |
| e2e, store mode | devnet **PASS 44**; site **PASS 21**; browser **PASS 80**; integrated **PASS 20, two runs of two** |
| witnesses (witnesses-24cc167.log, rust and live parts) | A01, A02 (R9, R13, SYS_SLOT_MAX), A03, A04, A05, A10, A11 **NOT_REPRODUCED, controls OK** |

- e2e_devnet's 44th check is new: each node's R14/R19 timing covers the mode it runs. It reads `True` for
  the three store-mode nodes and `False` in whole-state mode.
- In each integrated run node c offered every transaction it returned after the heal: 7 in three runs, and
  21 in one where its abandoned branch was longer. None was left unoffered.
- The extension witnesses (A06–A09, A12, A14) were not rerun. The extension is unchanged since 8584283, where
  they ran.
- **Not run on 24cc167:**
  - lp1-node;
  - the crash, kill and damage steps;
  - the spec-depth and deep-reorg tests;
  - the A13 models;
  - RSS and churn.

  Their last runs are above; the change does not touch them.
- **Not run on GitHub:** Windows CI cannot run on these commits until they are pushed.

## Windows

Run 64 on 9bd9bc0 (audit-r2-windows-verify.yml) passed every step: lp1 store journal, lp1-node, lp4, pocold
(1,719.6 s), and the extension's build and unit tests. Run 65 on 919a50c and run 66 on 8584283 (the relay
fix, its two tests included) also passed every step. A Windows pass is not acceptance. That run does not
include the Python and Chrome e2e suites, the thread-count assertions, or the ignored tests.

## Security witnesses (A01–A14)

witnesses-9bd9bc0.log: the reconstructed witnesses (WITNESS-CONTROLS.md), run on local nodes, test browser
profiles and the repaired API (variant v2). The programs are kept outside the repository; only their
observation lines are here.
- Every witness is **NOT_REPRODUCED, with its positive control OK**: A01, A02 (R9, R13, SYS_SLOT_MAX), A03,
  A04, A05, A06 (three cases), A07, A08, A09, A10, A11, A12, A14.
- The "1 uncommitted path" in the header is the `node_modules` symlink.
- They ran in whole-state mode: their launcher does not take `--state-store`.
- A NOT_REPRODUCED is an observation, not acceptance.

## Not run in this pass

- the debug-mode suites;
- the private adversarial EVM measurements (PRIVATE-REVIEW-PACKAGE.md);
- the other `jm_*` measurements;
- RSS at the network scale (A13 at the network profile is computed only).

None of these is counted as passed.
