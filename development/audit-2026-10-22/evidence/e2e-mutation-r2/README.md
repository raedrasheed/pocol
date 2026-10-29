# e2e devnet, round 2: a winner that is heavier but lower, a session relay, a replay check

This directory holds only new files. It follows `../e2e-mutation/` (stage A, test commit `bfeb226`),
whose files are not edited; §2 corrects what they say. The round-2 test is
`prototypes/pocold/tests/e2e_devnet.py` at commit `f583c7f` (sha256 `9d73605e…c1c15`). No node code
was changed: every run used pocold built from the c195a89 tree, whose code equals a03e089 (base
binary sha256 `71d36fe4…ca0f1`, built from this worktree; pocol-sign `1aa79f9d…18f29`). Mutants are
that tree plus one patch each. The mutant patches, the builder and the runner exist only in the
session scratch directory and are not committed. This is not a full acceptance: the full integration
(`prototypes/lp10/run_lp10.py`) and the Windows workflow were not re-run.

## 1. Review findings and their resolution

| Finding | Resolution | Where |
|---|---|---|
| F1 (major): the test could not tell most work (consensus.md:209) from longest chain; the winner was always also the higher tip, and the height-first mutant Mheight passed 25/25 | **Fixed.** The winner is now strictly heavier AND strictly lower than the other tip; that precondition is asserted before the heal. Mheight fails at the heal (§4.2) | test step 1–3; `Mheight.log` |
| F2: lighter-preference detection rested on one relay shape (unsolicited, no session, peer claims genesis) | **Fixed.** A second relay runs inside a sync session with a peer whose Status claims more work than the node's head; it answers the node's GetFrom with the lighter branch. The new mutant Msess (a session-delivered block becomes the head) passes the stage A test and fails the new one (§4.2, §4.3) | test step 6; `Msess.log`, `old-Msess.log` |
| F3: the head after the relay was read once; the restart after it checked only heights | **Fixed.** The relayed node restarts alone (no peers, no miner) and must replay to the same head, with the relayed tip stored and not canonical. The new mutant Mreplay (replay takes the last record as the head) passes the stage A test and fails the new one | test step 7; `Mreplay.log`, `old-Mreplay.log` |
| F4: the corrupted-block check accepted any new reject entry | **Fixed (cheap form).** Exactly one new entry is required, for a block a does not store and that is not the original. The block hash is not recomputed (that needs Keccak and the template-id encoding in the test); the reason is printed (`3 template signature` in every run here) | malicious-peer check |
| F5: "c never held the relayed branch" was stated, not asserted | **Fixed.** The relayed node must hold none of the late branch's blocks before the relays. The first version (`f45ed5c`) asserted it for the whole branch above the fork; that was too strong (§3.3) | test step 5 |
| F6: "9 caught" counted 3 mutants that fail before the partition | **Fixed in reporting.** §4.2 separates "at the intended check" from "early"; §2 corrects stage A's counts | §2, §4.2 |
| F7: old-M1b was cited as convergence on the lighter branch | **Corrected** in §2 | §2 |
| F8: `stop_node` dropped a process from `procs` before it had exited | **Fixed.** `stop_nodes` signals a set at once and keeps a process in `procs` until it has exited (on a 30 s timeout it is killed and the stop fails). `run_node` refuses to start a node whose ports are still open: in a draft run a second process failed on the locked store while RPC kept answering from the old one | `stop_nodes`, `run_node` |
| F9: the reorg check accepted any increase | **Fixed.** The reorganizing node must count exactly one reorganization at the heal (nothing is mined, so its head switches once) | heal check |
| F10: the heal no longer runs while miners are mining | **Not changed; scope stated** in §5. Reorganizations under mining remain covered only by tip races during production (1-block reorgs, and the receipts and convergence checks) | §5 |
| Survivor Mtie (inverted D133 tie-break) | **Not a defect of the test; explained.** A tie between siblings of one kind is local by design (D133), and the inverted order is applied the same way on every node, so the nodes still converge. The unit test `fork_choice_heavier_tied_signed_first_and_rollback` catches it (stage A: `../e2e-mutation/unit-tiebreak-Mtie.log`) | §4.2 |
| Survivor Mheight | Now caught by the e2e test (F1), and by 4 unit tests (§4.4) | §4.2, §4.4 |

## 2. Corrections to `../e2e-mutation/README.md` (stage A)

- **"A wrong choice fails every time" was too broad.** It held only for wrong rules that disagree
  with most work when the heavier tip is also the higher one. The reviewer's Mheight passed the stage
  A test 25/25. With this round's partition the claim is: a rule that does not choose the strictly
  heavier tip fails in a scenario where that tip is strictly lower (Mheight at the heal), and a rule
  that prefers the lighter tip across a fork fails at the heal or at a relay (M1b, §4.2).
- **The counts.** Stage A's "10 mutants: 9 caught, 1 survived" should read: 6 caught at the intended
  check; 3 caught early (M1a and M3 at the test's own bounded liveness waits, M2a at the receipts
  check); survivors Mtie and Mheight (Mheight found by the review), both caught by unit tests.
- **old-M1b.** §1 there says that in `old-M1b` "the lighter-preferring mutant M1b passes this check"
  and frames it as convergence on the lighter branch. `old-M1b.log` does not show which branch the
  nodes converged on. The correct reading: the pre-change heal check passed under M1b; the stage A
  heal checks also pass under M1b (they converge on the heavier branch, `M1b.log`), and only the relay
  caught it.
- **The canonical-wins argument** ("c's isolated branch is canonical by construction") no longer
  holds: in this round's partition the isolated node's branch loses. §3.2 gives the new check.

## 3. The partition (test `f583c7f`)

### 3.1 Steps

1. **c is isolated and frozen at once**: restarted with no listener, no peers and no miner, at its
   head c0.
2. **a and b mine on** until each leads c0 by more than LEAD = 4 times the work of c0's block
   (`difficulty` of c0) and is at least 2 blocks past it. They are then frozen on **one** tip: a's
   miner stops first, a follows b's next block, then b's miner stops. (Two equal-work siblings of one
   kind would stay apart, a local D133 tie, and there would be no single winner.)
3. **c mines late**: c restarts isolated with a miner PAUSE = 120 s after c0's timestamp. c0's
   attempt 0 has then expired (C > U(0)+Γ, consensus.md:97), so c0's child takes an attempt ≥ 1
   fallback timestamp L(a) ≥ p.ts+42 (consensus.md:93–95), and ASERT's target grows with
   p.ts−g_ts−T_blk·p.h (consensus.md §7). Every late block after the first is cheap. c mines until it
   is higher than a/b's tip and is frozen again (no miner). From here **no tip can move**.
   **Precondition, asserted**: a/b's tip is strictly heavier and strictly lower than c's tip, and at
   least 2 blocks past the fork.
4. **Heal.** c rejoins without a miner. Nothing is mined, so no block is broadcast and the heal can
   only start from Status sync (consensus.md:235). Checks: every head is exactly a/b's tip; on every
   node the canonical block at that height is a/b's tip; c's tip is canonical on no node; c's reorg
   counter moved by exactly 1.
5. **Relay without a session.** a holds none of c's late blocks (asserted). A test peer sends a
   Hello claiming genesis (so a starts no session with it) and then c's branch below c's tip, parents
   first. a must store it, keep its head, and not make it canonical (consensus.md:215).
6. **Relay in a session.** A second test peer claims c's tip with more work than a's head. a opens a
   session and sends GetFrom (the peer waits for it); the peer answers with all of c's branch. a must
   store c's tip, count a new session, keep its head, and not make c's tip canonical.
7. **Replay.** a restarts alone (no peers, no miner); its journal ends with c's branch. The replayed
   head must still be a/b's tip, with c's tip stored and not canonical.
8. All nodes restart with miners and the rest of the test runs as before (crash, ≥ 100 blocks,
   convergence, miner checks, replay).

### 3.2 Other changes

- **Miner cross-check.** "Every miner won blocks": a and b must have won canonical blocks (heights
  1..15 are theirs alone), and every block of c's late branch must name c as winner (c mined it
  alone). c's canonical wins are not certain any more: in `faeb5da-base-run1` c won 0 of 100
  canonical heights. The `mined` counter check is unchanged and covers c through its late blocks.
- **Waits on a lone miner** (a/b's lead, a following b's next block, c's late climb) allow for an
  attempt whose ranges hold no solution (the next attempt starts 46 s later): 180 s, 180 s, 240 s.
- 27 checks (stage A: 25).

### 3.3 How the design got here (runs kept under `superseded/`)

- **`f45ed5c`** kept c as the winner: c was isolated and mined; a and b were frozen; c mined until it
  led them by 4 a/b tip blocks of work; a and b then mined late. Its first three clean runs all
  failed. In two (`f45ed5c-base-run1.log`, `-run2.log`) the check "c holds no block of a's branch
  above the fork" failed: a tip race when c is isolated leaves an a/b block on c. In one
  (`f45ed5c-base-run3.log`) lone-miner c fell far behind schedule; its blocks became so cheap that it
  could not build its lead in 300 s (c at h=105, a/b at h=24). The late-mining mechanism itself
  worked (late work 1.44–1.46 a/b tip blocks).
- **`faeb5da`** swapped the roles (the two-miner majority builds the lead; lone c mines late) and
  limited the absence check to c's late blocks. Its first clean run (`faeb5da-base-run1.log`) failed
  at "every miner won canonical blocks" (c 0 of 100); the second passed. The third was stopped by
  hand after the first failed and is not counted.
- **`f583c7f`** replaced the canonical-wins check (§3.2) and widened the lone-miner waits.
- Development runs on uncommitted drafts are not evidence and are not kept.

### 3.4 Margins measured

In the 11 runs with the final test that reached the precondition (3 clean; M1b, M3b, M4, M5, Mtie, Mheight,
Msess, Mreplay), in works of c0's block: a/b's lead over c0 was 5.11–6.66; c's late branch added
1.31–1.53; c's first two late blocks always took attempts ≥ 1; the late climb took 7–107 s (the
precondition line of each log prints all of these). Several runs repeat the same ratios: the
targets follow from timestamps and heights, and the attempt pattern repeats.

## 4. Results

Each run: `POCOLD=<bins>/<build>/pocold flock <scratch>/e2e.lock flock <scratch>/cpu.lock timeout -k 30
1800 python3 -I prototypes/pocold/tests/e2e_devnet.py` (cwd `prototypes/pocold`). Every log header
records the script and binary sha256. The harness timeout is 1800 s; a run stopped by it counts as
INCOMPLETE. No run reached it: every exit code is 0 or 1 (`summary-r2.txt`, `summary-old.txt`). A
"timeout waiting for …" verdict is the test's own named wait (exit 1). Elapsed times include waiting
for the cpu lock, which unit-test builds also took between runs.

### 4.1 Unmutated build: 3 consecutive clean runs

| Run | Result | a/b lead / late c (c0 blocks) | Heal and relays | Log |
|---|---|---|---|---|
| base-run1 | PASS 27/27 | 6.66 / 1.44; c 28 vs a/b 27 | c reorgs 0→1; session 0→1; wins a/b 70/24, c late 7/7 | `base-run1.log` |
| base-run2 | PASS 27/27 | 5.55 / 1.35; c 29 vs a/b 28 | c reorgs 0→1; session 0→1; wins 59/41 (c canonical 0), c late 6/6 | `base-run2.log` |
| base-run3 | PASS 27/27 | 5.55 / 1.35; c 27 vs a/b 26 | c reorgs 0→1; session 0→1; wins 35/56, c late 6/6 | `base-run3.log` |

No other e2e run came between them.

### 4.2 Mutants against the round-2 test (`f583c7f`)

| Mutant | Change (scratch patch) | Expected failing check | Observed first FAIL | Verdict |
|---|---|---|---|---|
| M1a | `better()`: inverted work comparison | no head advances: fails before the partition | `timeout waiting for a,b reach 15` | caught early (bounded liveness wait) |
| M1b | `better()`: across a fork the lighter tip wins | heal or a relay (below) | heal: c kept its late tip 25 (a/b 23) after 120 s | **caught at the intended check** |
| M2 | `better()`: no reorganization below the head's parent | heal | `user transactions included; receipts identical` | caught early (receipts check; stage A's run reached the heal) |
| M2a | `better()`: only a descendant of the head wins | early (tip races) or heal | `user transactions included; receipts identical` | caught early (receipts check) |
| M3 | no sync session, no missing-parent request | late join | `timeout waiting for c syncs` | caught early (bounded liveness wait) |
| M3b | no Status-driven sync session | heal (nothing mined: only Status sync can heal) | heal: c kept its late tip 26 after 120 s | **caught at the intended check** |
| M4 | reorg counter never incremented | heal: exactly one reorganization | c reorgs 0→0 (the heads did converge) | **caught at the intended check** |
| M5 | `mined` not incremented on the own-solution path | every miner produced blocks | `{"a": 0, "b": 0, "c": 0}` | **caught at the intended check** |
| M6 | a wrong-network Hello keeps the connection | malicious peer | wrong-network closed=False | **caught at the intended check** |
| Mtie | inverted D133 tie-break | survives (§1) | PASS 27/27 | survived (unit test catches it) |
| Mheight | `better()`: greater height first, then work | heal | heal: c kept its higher, lighter tip 26 (a/b 25) after 120 s | **caught at the intended check** |
| Msess | a block delivered in a sync session becomes the head | relay in a session | a's head became c's lighter tip 27 | **caught at the intended check** |
| Mreplay | journal replay takes the last record as the head | replay after the relay | a's replayed head was c's lighter tip 27 | **caught at the intended check** |

Totals: 13 mutants; 8 caught at the intended check, 4 caught early (2 at bounded liveness waits, 2 at
the receipts check), 1 survived (Mtie, caught by a unit test), 0 incomplete. Each mutant has
`<M>.log`, and a failing run also `<M>-nodes.txt` (the last 60 lines of each node log).

**Why M1b fails deterministically here.** Under M1b a node switches from its head to a block across
a fork only if that block is lighter than the head. At the heal c holds its late tip; if c imports,
as a new block, an a/b block above the fork that is lighter than c's tip, c switches to it, follows
a/b's branch to the tip (descendants compare by work as usual), the heal passes, and then the relay
without a session fails: every block of c's branch is lighter than a/b's tip, so the first one a
imports as new moves a's head. If no such a/b block is imported as new, c stays on its late tip and
the heal fails. In this run the heal failed: c's log (`M1b-nodes.txt`) shows no head change after
c rejoined, so no a/b block lighter than c's tip was imported as new. c0 (h=18) is not on a/b's
chain (fork 17) and c held 22 journal records at h=18, so the first a/b block above the fork was
most likely already stored on c (a duplicate is not compared again); this was not checked further.
Stage A's run of M1b took the other branch (heal passed, relay failed).

### 4.3 The new mutants against the stage A test (`bfeb226`, sha256 `31cace51…6fd76`)

Run the same way from a scratch copy of the stage A script, to show what F2 and F3 add.

| Mutant | Stage A test | Round-2 test |
|---|---|---|
| Msess | **PASS 25/25, survived** (`old-Msess.log`): its heal is session-driven and the session delivers the heavier branch anyway; its relay has no session | caught at the relay in a session |
| Mreplay | **PASS 25/25, survived** (`old-Mreplay.log`). By reading that test: after its relay c restarts connected and with a miner, so live imports from heavier peers pull a wrong replayed head back, and its final replays pass when the last journal record is the live head | caught at the replay after the relay |
| Mheight | PASS 25/25 (the review's run, not repeated) | caught at the heal |

### 4.4 pocold test binaries against the new mutants

`cargo test --release --locked --test <consensus|net|store>` on the scratch source copy, base and
each new mutant (`unit-<binary>-<mutant>.log`, `unit-summary.txt`):

| Build | consensus | net | store |
|---|---|---|---|
| base | ok 42 passed (2 ignored) | ok 20 | ok 12 |
| Mheight | FAILED: 4 (`a_session_branch_is_adopted_through_a_flood_of_competing_side_branches`, `r_keep_boundary_adopts_at_depth_r_keep_and_keeps_the_valid_chain_beyond`, `replay_uses_the_largest_r_keep_the_data_directory_has_used`, `side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again`) | ok 20 | ok 12 |
| Msess | FAILED: 5 (`a_session_block_arriving_before_its_parent_stays_protected`, `a_session_branch_is_adopted_through_a_flood_of_competing_side_branches`, `a_sync_session_refetches_an_evicted_branch_and_serves_it`, `d133_tie_break_at_depth_holds_through_eviction_and_refetch`, `side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again`) | ok 20 | ok 12 |
| Mreplay | FAILED: 3 (`a_crash_around_an_adoption_reopens_on_one_whole_chain`, `journal_replay_rebuilds_pruned_state_for_a_journaled_fork`, `replay_gives_a_reimported_block_a_new_import_seq`) | ok 20 | ok 12 |

So the unit tests already caught all three; the e2e test now catches them too, across real
processes and the wire.

### 4.5 Full pocold release suite, once, at the end

`cargo test --release --locked` in `prototypes/pocold` at `f583c7f` (`pocold-suite-f583c7f.log`):
every test binary ok, 123 passed, 0 failed, 2 ignored; exit 0; no compiler warnings. The crate's Rust
sources are unchanged since a03e089; `tests/e2e_devnet.py` is not part of `cargo test`.

## 5. Scope and limits

- **One heal direction.** The isolated single node reorganizes to the two-node majority's heavier,
  lower branch. Stage A asserted the opposite (the majority reorganizing to the minority); this round
  does not. The relays test a node on the winning branch receiving the losing one.
- **No heal under mining (F10).** The heal runs with every miner stopped, so the winner is fixed. A
  reorganization while miners keep running (a moving session target, broadcasts and parent fetches
  overlapping it) is exercised only by tip races during production.
- **Tie-breaks.** The e2e test does not discriminate D133's tie-break (Mtie); the unit test does.
- **Reject attribution (F4)** is by exclusion (one new entry, not stored, not the original), not by
  recomputing the corrupted block's hash.
- **Single observations.** Each mutant ran once; the deterministic arguments are in §3.1 and §4.2.
- **Not re-run:** the full integration and the Windows workflow. Not a full acceptance.

## 6. Files

- `base-run1.log` … `base-run3.log`: the clean runs on `f583c7f`.
- `<M>.log` (13 mutants) and `<M>-nodes.txt` for each failing run; `summary-r2.txt`: one JSON line
  per run, as the runner wrote it.
- `old-Msess.log`, `old-Mreplay.log`, `summary-old.txt`: the stage A test against the new mutants.
- `unit-<consensus|net|store>-<base|Mheight|Msess|Mreplay>.log`, `unit-summary.txt`: §4.4.
- `superseded/`: the clean runs of the two superseded versions (`f45ed5c-*`, `faeb5da-*`, §3.3),
  kept as recorded.
- `pocold-suite-f583c7f.log`: the full pocold release suite, once, at the end (the Rust sources are
  unchanged; only `tests/e2e_devnet.py` changed in the crate).
