# e2e devnet, round 3: the second review's findings E2E-R3-1 … E2E-R3-9

This directory holds only new files. It follows `../e2e-mutation-r2/` and
`../e2e-integration/`; none of their files is edited, and §2 corrects what they say.

- **The test** is `prototypes/pocold/tests/e2e_devnet.py` at commit `1512409`, sha256
  `6667e11a3e80989b2bdcfc7d5a3b665a6a1e17f1c4bb85868f0d3dea16a621bf`, 35 checks. Commit `0e51a36`
  (sha256 `0c4987d0…a76b60`, 33 checks) addressed E2E-R3-1 to 6 and 8; `1512409` adds the Status
  checks of steps 5 and 6 (E2E-R3-2, §3).
- **The review** found E2E-R3-1 to E2E-R3-9 and three surviving mutants, Mhu, Mtransient and Morph.
  It was run on `fa97594` with the test at sha256 `9d73605e…c1c15`.
- **A container restart** interrupted the first validation, after three clean runs and four mutant
  runs on `0e51a36`; the worktree they ran from was lost with it. Those logs are kept as recorded
  in `pre-restart-0e51a36/` (§4.4). Every verdict in §4.1 and §4.2 comes from runs made after the
  restart, on the test at `1512409`; §4.3 runs the round-2 test on purpose. A session limit then
  interrupted the validation a second time; §4.6 lists the one log it truncated.
- **No node code changed.** The branch's `prototypes/` differs from `fa97594`'s only in
  `tests/e2e_devnet.py` and `tests/consensus.rs`. The base binary of §4.1 was built after the
  restart from this branch's tree at `a437bcc`, with `CARGO_TARGET_DIR=<scratch>/wf-target-r3-e2e
  cargo build --release --locked --bins` under the cpu lock and no warnings: pocold sha256
  `08d46b5dc23e…8ad8fa20`, pocol-sign `d14fbd9b4bee…52a7db55`. A second build of the same sources
  (base-r3d, from this branch at `d19f515`, pocold `b21fa0a78309…cccac09`) ran one more control
  after the mutants.
- **Mutants** are an export of `fa97594`'s `prototypes/` plus one patch each, built the same way
  (`builds.txt`). After the restart the export was compared with this branch (`diff -r`: only the two
  test files differ) and each surviving binary was re-hashed against its build log. The patches, the
  builder, the runner and the summaries exist only in the session scratch directory; none is
  committed.
- **E2E-R3-7** changes one unit test in `tests/consensus.rs` (commit `16bebfd`): test code only (§4.5).
- **This is not a full acceptance.** The full integration (`prototypes/lp10/run_lp10.py`) and the
  Windows workflow were not re-run.

## 1. Findings and their resolution

| Finding | Resolution | Evidence |
|---|---|---|
| E2E-R3-1 (major): the relay without a session never carried a block higher than a's head, so a longest-chain rule on the broadcast path (Mhu) passed | **Fixed.** c climbs until it is at least 2 blocks above a/b's tip, and the precondition asserts `ab_tip + 2 <= c_tip`. The relay below c's tip therefore ends at least one block above a's head, and that block is lighter | step 3; Mhu §4.2 |
| E2E-R3-2 (major): a's head was read once after each relay, so a switch that reverted at the next tick passed (Mtransient); no check that step 5 started no session | **Fixed.** The test reads a's `reorgs` and `syncSessions` counters before and after each relay. Step 5 must leave both unchanged. Step 6 must add exactly one session and no reorganization. a's and b's counters must not change across the heal. Since `1512409` the test peers of steps 5 and 6 also read every frame a sends them: each Status (sent on connect and on every new head) must name a/b's tip, so a switch that the counters do not record is seen too | steps 4–6; Mtransient, Msilent, Mwants §4.2, §4.3 |
| E2E-R3-3 (major): no deep reorganization went through the unsolicited or orphan path | **Fixed.** Node d is a copy of c's store taken before the heal, with a listener, no peers and no miner. A test peer claiming genesis, so that d starts no session, sends a/b's branch above the fork tip first. d must switch from c's tip to a/b's tip with exactly one reorganization and zero sessions | step 4b; Morph, Mcount §4.2, §4.3 |
| E2E-R3-4 (minor): F10, no heal while miners run | **Closed as coverage.** Node e is a second copy of c's pre-heal store. It joins a, b and c while they mine. Within 120 s, e's canonical block CONFIRM = 6 below its head must equal a's and lie above c's tip height; c's tip must not be canonical on e; e must count at least one reorganization. Telling most work from longest chain remains the frozen heal's job, since the live chain is also the higher one. No mutant here is caught by this check alone (§5) | step 8b |
| E2E-R3-5 (minor): the frozen tips were never compared | **Fixed.** After the miner checks, a, b and c restart connected and without miners, so no tip can move. Within 60 s they must reach one totalWork. Their canonical blocks must be identical up to the last height they all share, at most one block below the lowest head (D133 keeps equal-work siblings apart, consensus.md:224) | step 11; no mutant is caught by it alone: Mfollow is caught by step 10 first (§4.2, §5) |
| E2E-R3-6 (minor): b restarted connected after SIGKILL, so a lost journal plus a resync passed | **Fixed.** After SIGKILL, b first restarts alone (no peers, no miner). Its replayed head must be at least its pre-kill height less one (a torn tail loses at most one record), and its block at that height must be the one it had before the kill. Then b restarts connected | step 9; Mcrash §4.2, §4.3 |
| E2E-R3-7 (minor): the unit test `a_long_catch_up_is_not_bounded_by_the_protected_branch_size` was timing-dependent (once `syncRounds` = 1 in 11 runs) | **Fixed in test code** (`16bebfd`). The catch-up is SYNC_COUNT + 8 = 520 blocks, more than one GetFrom answer carries, so a second round is certain whatever the timing. 20 runs of the consensus binary before and 20 after, all passing | §4.5 |
| E2E-R3-8 (minor): the waits and the number of late blocks were not printed; README §3.4 said "always" | **Fixed.** Every named bounded wait prints `WAIT <what>: met/NOT met after <s> (bound <s>)`. The precondition line prints the number of late blocks, and the result line prints the test's own run time (`elapsed_s`). The wording is corrected in §2 here, because the round-2 README is not edited | logs; §2 |
| E2E-R3-9 (minor): the integrated 3/3 log records only the binary | **Fixed** in a new note, `PROVENANCE-e2e-integration.md`. It gives the script sha256 `9d73605e…c1c15`, the exact command, the working directory, the build and the locks, and points out that the log's `exit=0` lines are `tail`'s status, not the test's | `PROVENANCE-e2e-integration.md` |

## 2. Corrections to `../e2e-mutation-r2/README.md`

These hold for the round-2 test (`f583c7f`, sha256 `9d73605e…c1c15`):

- **§2, first bullet ("a rule that does not choose the strictly heavier tip fails …").** This held
  for the session path (the heal) and for a global rule (Mheight). It did not hold for the
  session-less path. c usually ended exactly one block above a/b, so the relay below c's tip never
  carried a block higher than a's head, and the review's Mhu (an unsolicited higher block becomes the
  head) passed 27/27. With round 3 (c ≥ a/b + 2, asserted), Mhu fails at that relay (§4.2).
- **§1, F3 "Fixed".** Read: *partly fixed*. The replay after the relay checks that the head
  persists. A transient switch that reverts at the next tick passed: the review's Mtransient passed
  27/27 twice. Round 3 catches it through a's counters, and through the Status a sends when the
  switch is not counted (Msilent, §4.2).
- **§3.4 ("c's first two late blocks always took attempts ≥ 1").** Read: *in the runs logged
  there*. The review's runs had late attempts `[2, 0, …]`. The precondition does not depend on this:
  the mechanism is printed, while the outcome (heavier and lower) is asserted.
- **§5, "No heal under mining (F10)".** Now covered by node e as a convergence check under mining
  (§1, E2E-R3-4). "One heal direction" remains stated scope.

## 3. The test (`1512409`)

Steps 1–3, the partition, are as in round 2, except for the higher climb. Each step below names
what it checks.

1. **c is isolated and frozen** at its head c0.
2. **a and b lead** c0 by more than LEAD = 4 c0 blocks of work and are frozen on one tip.
3. **c mines late** (PAUSE = 120 s after c0's timestamp) until it is **at least a/b's height + 2**,
   then is frozen. Precondition, asserted: a/b's tip is strictly heavier than c's tip, at least 2
   blocks lower, and at least 2 blocks past the fork. Printed: the work margins, the number of late
   blocks, their attempts and the climb time.
4. **Heal.** a's and b's counters are read, c is stopped, and its store is copied to `data/d` and
   `data/e`. c then rejoins without a miner. Checks: every head is a/b's tip; the canonical block at
   that height is a/b's tip; c's tip is canonical on no node; c reorganized exactly once; **a and b
   neither reorganized nor started a session**.
   - **4b. Heal without a session (node d).** d opens the copy (head c's tip) with a listener,
     no peers and no miner. A test peer sends a Hello claiming genesis, then a/b's branch above the
     fork in reverse order. Every block but the lowest arrives before its parent. Check: d's head
     goes from c's tip to a/b's tip, with reorgs +1 and syncSessions 0. d is stopped.
5. **Relay without a session** of c's branch below c's tip, whose last block is above a's head.
   Checks: a stores it, keeps its head, the branch is not canonical, and **a's reorgs and
   syncSessions are unchanged**; then, a separate check, **every Status a sent the test peer names
   a/b's tip**. The test peer reads for 2 s after the last block is stored. pocold sends a Status on
   connect and on every new head (its Status refresh); the connect Status is the control that the
   peer reads at all, so the check needs at least one.
6. **Relay in a sync session.** Checks: a stores c's tip, keeps its head, c's tip is not canonical,
   with **exactly one new session and no reorganization**; then **every Status a sent the session
   peer names a/b's tip**.
7. **Replay of a** alone, as in round 2.
8. All three nodes mine again; production resumes.
   - **8b. Heal while mining (node e).** e first opens its copy alone, to read its starting head
     (c's tip), and is stopped. It then dials a, b and c, which are mining, with no listener and no
     miner. Bounded check (120 s), as in §1. e is stopped.
9. **SIGKILL crash of b.** b's head and its canonical block one below the head are read, then
   b is killed. b restarts alone; check: its replayed head is at least the pre-kill height less one,
   and its block at that height is unchanged. b is stopped and restarts connected with a miner;
   check: it rejoins.
10. ≥ 100 blocks; identical canonical blocks on the confirmed common prefix; the miner checks. All
    unchanged.
11. **Frozen tips.** a, b and c restart connected without miners. Checks: one totalWork within
    60 s; identical canonical hash, stateRoot and receiptsRoot up to the last height all three
    share, which must be at most one below the lowest head.
12. **Full restart replay** of each node and two independent `pocold status` replays, as before.
    The live heads are now read on frozen nodes.

35 checks (round 2: 27; `0e51a36`: 33). The busy-port check covers 30300–30303 and 8540–8544: d uses
RPC 8543 and P2P 30303; e uses RPC 8544 and does not listen.

## 4. Results

Every run below is one `e2e_devnet.py` run, made by a scratch runner as
`POCOLD=<bin>/pocold flock <scratch>/e2e.lock flock <scratch>/cpu.lock timeout -k 30 1800 python3 -I <worktree>/prototypes/pocold/tests/e2e_devnet.py`
with the working directory `<worktree>/prototypes/pocold`. Each log starts with the time, the
worktree HEAD, the script's sha256, the binaries' sha256 and the command. Its last line is the
runner's summary: `exit` is the test process's own exit status (not a pipe's), and `outcome` is
PASS, FAIL or INCOMPLETE (a harness timeout, never counted as caught or as a pass; none occurred).
`test_s` is the test's own run time; `elapsed_s` adds the lock waits.

Except in §4.3 and §4.4, every log ran the test at sha256 `6667e11a…a621bf` (`1512409`). The
runs up to 03:40Z were made from the worktree `wf_e4388509-f06-3` at `1512409`, which a session
limit then lost. The runs from 06:13Z were made from the worktree `wf_86607a23-c25-4` at `d19f515`,
whose `prototypes/` equals `1512409`'s (`git diff --stat 1512409 d19f515 -- prototypes` is empty).
Each log was checked for its result line and its script sha256 before it was counted (§4.6).

### 4.1 Base: 4 runs, PASS 35/35 each

| Run | Binary | Result | Partition (heights; work above c0 in c0 blocks) | Late blocks, attempts, climb | Node d (4b) | Node e (8b) | SIGKILL (9) | Frozen tips (11) |
|---|---|---|---|---|---|---|---|---|
| `base-run1.log` 02:20Z | base-r3c | PASS 35/35, 271 s | a/b 27, c 30, fork 19; 5.32 / 1.49 | 8, `[1,1,0,…]`, 16 s | 30→27, reorgs 0→1, sessions 0 | 30→37 in 7 s, reorgs 3 | 37 → replayed 37 | 106, heads identical |
| `base-run2.log` 02:26Z | base-r3c | PASS 35/35, 260 s | a/b 27, c 29, fork 20; 5.90 / 1.53 | 8, `[1,1,0,…]`, 13 s | 29→27, 0→1, 0 | 29→36 in 4 s, reorgs 3 | 36 → 36 | 106, identical |
| `base-run3.log` 02:36Z | base-r3c | PASS 35/35, 283 s | a/b 26, c 28, fork 21; 5.55 / 1.44 | 7, `[1,1,0,…]`, 4 s | 28→26, 0→1, 0 | 28→35 in 7 s, reorgs 2 | 35 → 35 | 106, identical |
| `base-r3d-run1.log` 06:36Z | base-r3d | PASS 35/35, 296 s | a/b 26, c 29, fork 21; 5.55 / 1.23 | 8, `[2,0,0,0,0,0,1,0]`, 58 s | 29→26, 0→1, 0 | 29→36 in 3 s, reorgs 2 | 36 → 36 | 106, identical |

Runs 1–3 are consecutive (one batch, nothing between them) on base-r3c, built after the container
restart from this branch's tree at `a437bcc`. base-r3d was built at 06:08Z from this worktree at
`d19f515`, with the same command and target directory (`builds.txt`): same pocold sources, a
different binary (each embeds its own worktree path). Its run, made after all mutant runs, is a
control that the environment still passes. In every base run the heal left a's and b's counters
unchanged, and each Status check read exactly one Status (the connect Status: a's head never
moved).

### 4.2 Mutants on the final test

One run each. "Intended" means the run first failed at the check the mutant was aimed at. "Early"
means it failed first at a check before that one; such a mutant is caught but does not show what
the later check is worth.

| Mutant | Change (scratch patch) | Aimed at | Observed first FAIL (log) | Verdict |
|---|---|---|---|---|
| M1a | `better()`: inverted work comparison | — (no head advances) | `timeout waiting for a,b reach 15` (`M1a.log`) | caught early (bounded liveness wait), as in round 2 |
| M1b | `better()`: across a fork the lighter tip wins | heal | heal: c kept its late tip 29 (a/b 27) after 120 s (`M1b.log`) | **caught at the intended check** |
| M2 | `better()`: no reorganization below the head's parent | heal | heal: c kept 28 (a/b 25) after 120 s (`M2.log`) | **caught at the intended check** (round 2: early, receipts) |
| M2a | `better()`: only a descendant of the head wins | early (tip races) or heal | `user transactions included; receipts identical` (`M2a.log`) | caught early (receipts check), as in round 2 |
| M3 | no sync session, no missing-parent request | late join | `timeout waiting for c syncs` (`M3.log`) | caught early (bounded liveness wait), as in round 2 |
| M3b | no Status-driven sync session | heal | heal: c kept 27 (a/b 25) after 120 s (`M3b.log`) | **caught at the intended check** |
| M4 | reorg counter never incremented | heal: exactly one reorganization | c reorgs 0→0 (`M4.log`) | **caught at the intended check** |
| M5 | `mined` not incremented on the own-solution path | every miner produced blocks | `{"a": 0, "b": 0, "c": 0}` (`M5.log`) | **caught at the intended check** |
| M6 | a wrong-network Hello keeps the connection | malicious peer | wrong-network closed=False (`M6.log`) | **caught at the intended check** |
| Mtie | inverted D133 tie-break | survives by design | PASS 35/35 (`Mtie.log`) | survived, as in round 2: a tie between siblings of one kind is local (D133) and the inverted order is applied alike on every node; the unit test `fork_choice_heavier_tied_signed_first_and_rollback` catches it (`../e2e-mutation/unit-tiebreak-Mtie.log`) |
| Mheight | `better()`: greater height first, then work | heal | heal: c kept its higher, lighter tip 28 (a/b 26) (`Mheight.log`) | **caught at the intended check** |
| Msess | a block delivered in a sync session becomes the head | step 6 | a's head became c's lighter tip 28, reorgs +1, sessions +1 (`Msess.log`) | **caught at the intended check** |
| Mreplay | journal replay takes the last record as the head | step 7 | a's replayed head was c's lighter tip 27 (`Mreplay.log`) | **caught at the intended check** |
| **Mhu** (review) | an unsolicited peer block higher than the head becomes the head | step 5 (E2E-R3-1) | a's head became c's 26 (a/b 25), reorgs 0→1 (`Mhu.log`) | **caught at the intended check** (survived round 2) |
| **Mtransient** (review) | an unsolicited non-better block becomes the head until the next tick | step 5 counters (E2E-R3-2) | head read back as a/b's tip, but reorgs 0→2 (`Mtransient.log`) | **caught at the intended check** (survived round 2 twice) |
| **Morph** (review) | a block resolved from the orphan pool never becomes the head | step 4b (E2E-R3-3) | d head 27→27, reorgs 0→0, sessions 0 (`Morph.log`) | **caught at the intended check** (survived round 2) |
| Msilent (own) | Mtransient, but the revert takes back the reorg count, so a's counters do not move | step 5 Status check (E2E-R3-2) | the counter check passed; 8 Status heads: a/b's tip first and last, 6 other heads between (`Msilent.log`) | **caught at the intended check** |
| Mwants (own) | `wants()` drops its work condition: a session starts for any unknown head | heal: a and b start no session (E2E-R3-2) | a/b syncSessions 0→1 each, heads still right (`Mwants.log`) | **caught at the intended check** |
| Mcount (own) | a head switch made while the orphan pool resolves counts as a reorganization | step 4b: exactly one (E2E-R3-3) | d head 30→27 as required, but reorgs 0→4 (`Mcount.log`) | **caught at the intended check** |
| Mcrash (own) | after an unclean stop the journal's last 5 records are not replayed (a clean SIGTERM leaves a marker) | step 9 (E2E-R3-6) | pre-kill head 37, replayed alone 34 (`Mcrash.log`) | **caught at the intended check** |
| Mfollow (own) | no peer block above height 104 is imported | step 11 (E2E-R3-5) | `timeout waiting for confirmed common prefix >= 100` (step 10) after 300 s (`Mfollow.log`) | caught early, by the step-10 wait that round 2 already had; step 11 is not shown to catch anything by itself (§5) |

Totals: 21 mutants, 20 caught and 1 survivor (Mtie, explained). Caught at the intended check: 16,
including the review's three round-2 survivors and the four own mutants aimed at the round-3
checks of steps 4, 4b, 5 and 9. Caught early: M1a, M2a, M3 (as in round 2) and Mfollow.

Every mutant source is `fa97594`'s `prototypes/` plus one scratch patch; after the restart and
again in this continuation (06:07Z) the export was compared with this branch (`diff -r`: only `tests/e2e_devnet.py` and
`tests/consensus.rs` differ) and every binary in `builds.txt` was re-hashed and matched. Mhu,
Mtransient and Morph change the same lines as the review's patches
(`<scratch>/e2e-rev2/patches/*.diff`); Mheight differs only by a comment. Msess and Mreplay are the
round-2 patches, not the review's (same behaviour, different code).

### 4.3 The own mutants against the round-2 test

To show that the round-3 checks are what catch them, the four own mutants caught at their
intended check were also run once against the round-2 test (`fa97594`, sha256 `9d73605e…c1c15`,
27 checks, with its own `scripts/devnet.py`, sha256 `1876a5f4…`, unchanged since), from a scratch
copy, with the same binaries:

| Mutant | Round-2 test | Round-3 test (§4.2) |
|---|---|---|
| Mcount | **PASS 27/27, survived** (`old-Mcount.log`): no check counts d's reorganizations, and there is no node d | caught at step 4b (reorgs 0→4) |
| Msilent | **PASS 27/27, survived** (`old-Msilent.log`): the head is read once after each relay | caught at the step-5 Status check |
| Mcrash | **PASS 27/27, survived** (`old-Mcrash.log`): b restarts connected after the kill, so the lost records come back by resync | caught at step 9 (replayed 34, pre-kill 37) |
| Mwants | FAIL 12/27 at `relay: a holds none of c's late blocks` (`old-Mwants.log`): the sessions a and b start at the heal fetch c's late branch | caught at the heal, by the counter check (one check earlier) |

So three of the four own mutants survive the round-2 test and are caught only by round-3 checks.
Mwants is caught by both; the round-3 counter check names the cause (an unwanted session) rather
than a consequence. The old test prints no `elapsed_s`, so these summaries have `test_s` null.

### 4.4 Runs before the container restart (`pre-restart-0e51a36/`)

These ran the `0e51a36` test (sha256 `0c4987d0…a76b60`, 33 checks, without the Status checks)
on the `base` binary and the mutants above, from a worktree that the restart lost. They are kept
as recorded and are **not counted** above: the test has changed since. Base: PASS 33/33 three times
in a row. Mhu and Mtransient failed at the step-5 relay check, Morph at step 4b, as on the final
test; Mfollow failed at step 10's wait, as on the final test.

### 4.5 E2E-R3-7: the long catch-up unit test

`unit-catchup-before.txt`: 20 runs of the consensus test binary on `fa97594` (consensus.rs sha256
`04bec2fd…`), each `flock cpu.lock cargo test --release --locked --test consensus`: all 20 passed.
The review's single failure (1 in 11) did not recur, so the "before" runs show the flakiness only
by its absence; the fix rests on construction. `unit-catchup-after.txt`: 20 runs on `16bebfd`
(consensus.rs sha256 `6cc0f16f…`): all 20 passed, 43 passed and 2 ignored each. The catch-up is now
SYNC_COUNT + 8 = 520 blocks, more than one GetFrom answer carries (SYNC_COUNT = 512, node.rs:61), so a
second round does not depend on timing.

`pocold-suite-1512409.log`: the full pocold suite on `1512409`
(`flock cpu.lock cargo test --release --locked`, `CARGO_TARGET_DIR=<scratch>/wf-target-r3-e2e`):
exit 0, every binary `ok` (lib 44, admission 3, caps 7, consensus 43 + 2 ignored, decoder 1,
journal_measure 1 + 6 ignored, net 35, secp 1, site 6, store 12).

### 4.6 Logs not counted

- `<scratch>/e2e-r3/runs-c/r2mut2/M2.log` (started 03:40:10Z): it ends after the partition, before
  the heal, with no result line. The session limit stopped it. Not committed and not counted; M2
  was run again (`M2.log`, 06:13Z).
- `<scratch>/e2e-r3/runs/old/*.log` and `runs/mut/{M1a,Mtie}.log` (23:46:50Z): a header only. The
  container restart stopped them before the test started. Not committed and not counted.
- Every committed log of §4.1 and §4.2 ends with its summary line, and its script sha256 is
  `6667e11a…`, the sha256 of `tests/e2e_devnet.py` at `1512409` and at the branch head.

## 5. Scope and limits

- **One run per mutant.** A mutant that is caught once is caught; one passing run of a survivor
  (Mtie) does not show it would always pass.
- **Step 8b and step 11 catch no mutant by themselves.** Step 8b (node e under mining) is stated
  as convergence coverage (§1, E2E-R3-4). Mfollow, aimed at step 11, is caught by step 10's bounded
  wait first: a node that stops following is caught there unless it stops within the last few
  blocks before step 10 ends. In all four base runs the frozen heads were identical at height
  106, so step 11's allowance for equal-work siblings was never used.
- **The Status checks read one Status in every base run** (the connect Status, the control). The
  refresh Statuses are seen only when a's head moves, as under Msilent (8 Statuses).
- **The torn-tail allowance of step 9 was not used**: in every base run b's replayed head was its
  pre-kill head.
- **No node code changed** in this round; only `tests/e2e_devnet.py` and the unit test of §4.5.
- **Not a full acceptance.** The full integration (`prototypes/lp10/run_lp10.py`) and the Windows
  workflow were not re-run on this branch.
