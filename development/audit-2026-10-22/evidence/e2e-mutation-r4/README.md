# e2e devnet, round 4: the third review's findings E2E-R4-1 … E2E-R4-3

This directory holds only new files. It follows `../e2e-mutation-r3/`; none of its files is edited,
and §2 corrects what its README says.

- **The test** is `prototypes/pocold/tests/e2e_devnet.py` at commit `ad84942`, sha256
  `9c6d7c0d32334f25a676945f2ea9e72a9496de3a0a0ff41ff3fb1846d198814c`, 38 checks (round 3: 35, sha256
  `6667e11a…a621bf`).
- **The review** ran on `fd5c4d0`. It found E2E-R4-1 (major) and two minor findings, and two
  surviving mutants of its own, VMorphHi and VMorphAll.
- **No node code changed.** `git diff --name-only fa97594 ad84942 -- prototypes` lists only
  `tests/consensus.rs` (round 3, `16bebfd`) and `tests/e2e_devnet.py`.
- **Binaries.** base-r4 is this branch's tree at `ad84942`. VMorphHi and VMorphAll are the
  round-3 mutant source export (`fa97594`'s `prototypes/`, whose node sources equal this branch's)
  plus the review's edits, copied unchanged from its builder. All three were built with
  `CARGO_TARGET_DIR=<scratch>/wf-target-r3-e2e flock <scratch>/cpu.lock cargo build --release --locked --bins`,
  with no warnings (`builds.txt`). The other 21 mutants are the round-3 binaries, unchanged
  (`../e2e-mutation-r3/builds.txt`); every run log records its pocold sha256, and each matches that
  file (§4.4). The patches, builders and runners exist only in the session scratch directory.
- **This is not a full acceptance.** The full integration (`prototypes/lp10/run_lp10.py`) and the
  Windows workflow were not re-run.

## 1. Findings and their resolution

| Finding | Resolution | Evidence |
|---|---|---|
| E2E-R4-1 (major): the orphan path was tested in one direction only. Step 4b sends d a heavier, lower branch. No step sent a lighter branch tip first, so a longest-chain rule on the orphan path (VMorphHi) and an orphan-resolved block adopted without fork choice (VMorphAll) passed 35/35 | **Fixed in the test** (`ad84942`). New step 4c: f, a copy of b's store taken after the heal (head a/b's tip, none of c's late blocks), is sent c's lighter branch above the fork tip first by a peer claiming genesis, so the late blocks, higher than f's head and lighter, are released from the orphan pool. f must store c's tip and keep a/b's tip, c's tip must not be canonical, f's reorgs and syncSessions must not move, and every Status f sends the peer must name a/b's tip. Step 4b also reads every Status d sends: the first names c's tip, the last a/b's tip, and none carries less work than c's tip. Ports 30305 and 8545 joined the busy-port check | step 4c: VMorphHi, VMorphAll §4.2; step 4b Status: Msilent, Mtransient §4.2 |
| E2E-R4-2 (minor): `../e2e-mutation-r3/README.md` gave a cause (the embedded worktree path) for base-r3d's binary differing from base-r3c's, which the evidence does not support | **Corrected** in §2 below. The r3 README is not edited | §2, `builds.txt` |
| E2E-R4-3 (minor): step 9 read b's head kb and its canonical block kb−1 while b mined, then killed it; a legitimate reorganization of depth ≥ 2 in between failed the run without a defect (a false FAIL only) | **Fixed in the test** (`ad84942`). The chain check now compares b's canonical block CONFIRM = 6 below the pre-kill head (a tip race can replace the block one below the head, not one 6 deep), and adds that the pre-kill head's parent must be stored after the replay. That parent was journaled before the head, so it is never the torn record. The height check (replayed head ≥ kb−1) is unchanged. The log still prints whether kb−1 is the same after the replay | step 9; Mcrash §4.2 |

## 2. Corrections to `../e2e-mutation-r3/README.md`

- **§1, E2E-R3-3 "Fixed".** Read: *fixed for the heavier-branch direction*. A lighter branch
  delivered tip first was not tested until step 4c of this round (review mutants VMorphHi and
  VMorphAll, §4.2).
- **Intro bullet and §4.1 ("a different binary (each embeds its own worktree path)").** Read: *the
  binaries differ (pocold 39,559,664 B for base-r3c and 39,531,216 B for base-r3d, same toolchain,
  same pocold sources); the cause was not established.* Each binary does embed its own source path
  (44 times; both worktree names have 17 characters), but that does not explain a 28 KB size
  difference. base-r4, built from a third path, is 39,558,872 B. Builds here are not
  byte-reproducible. base-r3d's value as a control run is not affected.

## 3. The test (`ad84942`)

Steps 1–4 and 5–12 are as in round 3 (`../e2e-mutation-r3/README.md` §3), except for:

- **4b.** After d adopts a/b's tip, the test peer reads d's frames for 2 s. A new check: every
  Status d sent (on connect, then on each new head) names c's tip or a head with at least c's tip's
  work; the first names c's tip (the control), and the last names a/b's tip. Fork choice never takes
  a lighter head, even briefly.
- **4c (new).** b is stopped, its store is copied to `data/f`, and b restarts as before (connected,
  no miner). f opens the copy with a listener (P2P 30305, RPC 8545), no peers and no miner. A test
  peer sends a Hello claiming genesis, then c's branch above the fork (the step-5 relay list, c's tip
  included) in reverse order. Two checks: (i) f's starting head is a/b's tip and it holds none of
  c's late blocks (precondition); f stores c's tip; its head is still a/b's tip; c's tip is not
  canonical; reorgs and syncSessions are unchanged. (ii) Every Status f sent names a/b's tip. f is
  stopped.
- **9.** As in §1, E2E-R4-3.

38 checks (round 3: 35).

## 4. Results

Every run is one `e2e_devnet.py` run, made by a scratch runner as
`POCOLD=<bin>/pocold flock <scratch>/e2e.lock flock <scratch>/cpu.lock timeout -k 30 1800 python3 -I <worktree>/prototypes/pocold/tests/e2e_devnet.py`
from `<worktree>/prototypes/pocold`, where `<worktree>` is `wf_86607a23-c25-10` at `ad84942` with a
clean `prototypes/`. Each log starts with the time, the worktree HEAD, the script's and the
binaries' sha256 and the command, and ends with the runner's summary (`exit` is the test process's
own status; `outcome` PASS, FAIL or INCOMPLETE, a harness timeout, never counted; none occurred).
Every log here has script sha256 `9c6d7c0d…98814c`.

### 4.1 Base: 3 consecutive runs on base-r4, PASS 38/38 each

| Run | Result | Partition (heights; work above c0 in c0 blocks) | Late blocks, attempts, climb | Node d (4b): head, reorgs; Statuses (work above c's tip, in c tip blocks) | Node f (4c) | SIGKILL (9) | Frozen tips (11) |
|---|---|---|---|---|---|---|---|
| `base-r4-run1.log` 09:06Z | PASS 38/38, 327 s | a/b 26, c 29, fork 19; 6.32 / 1.46 | 9, `[1,1,0,…]`, 20 s | 29→26, 0→1, sessions 0; 6 Statuses: 0, 0.65, 1.76, 2.79, 3.86, 4.86 | held 0/9 late blocks; 10 sent tip first; head 26 kept, reorgs 0→0, sessions 0→0; 1 Status (a/b's tip) | 36 → replayed 36 | 106, equal-work siblings |
| `base-r4-run2.log` 09:12Z | PASS 38/38, 260 s | a/b 27, c 29, fork 21; 6.66 / 1.52 | 8, `[1,1,0,…]`, 16 s | 29→27, 0→1, 0; 6 Statuses: 0 … 5.14 | 0/8; 8 sent; 27 kept, 0→0, 0→0; 1 Status | 36 → 36 | 106, identical |
| `base-r4-run3.log` 09:16Z | PASS 38/38, 327 s | a/b 27, c 29, fork 22; 5.11 / 1.34 | 7, `[1,2,0,0,1,0,0]`, 63 s | 29→27, 0→1, 0; 5 Statuses: 0 … 3.77 | 0/7; 7 sent; 27 kept, 0→0, 0→0; 1 Status | 36 → 36 | 106, identical |

The three runs are one batch with nothing between them. In each, the heal left a's and b's
counters unchanged, the Status checks of steps 4c, 5 and 6 each read exactly one Status (the
connect Status: the head never moved), and d's Statuses rose from c's tip's work to a/b's tip's.
Node e followed within 8 s. In run 1 the frozen tips were two equal-work siblings at 106, so step
11's sibling allowance was used for the first time in a logged base run.

### 4.2 Mutants on the final test

One run each. "Intended" means the run first failed at the check the mutant was aimed at. "Early"
means it first failed at a check before that one: it is caught, but the run does not show what the
later check is worth by itself.

The mutants are described in `../e2e-mutation-r3/README.md` §4.2; VMorphHi and VMorphAll are the
review's. Round 3's verdict is given for comparison.

| Mutant | Aimed at | Observed first FAIL (log) | Verdict | Round 3 |
|---|---|---|---|---|
| **VMorphHi** (review): a block released from the orphan pool becomes the head if it is higher than the head | 4c (E2E-R4-1) | 4c: f head 25→27 (c's lighter late block), reorgs 0→1 (`VMorphHi.log`); d's 4b checks passed | **intended** | survived (review run, 35/35) |
| **VMorphAll** (review): a block released from the orphan pool becomes the head without fork choice | 4c (E2E-R4-1) | 4c: f head 26→28 (c's tip), reorgs 0→1 (`VMorphAll.log`) | **intended** | survived (review run, 35/35) |
| Morph | 4b | d head 27→27, reorgs 0→0 (`Morph.log`) | **intended** | intended |
| Mcount | 4b | d head 30→28 as required, reorgs 0→6 (`Mcount.log`) | **intended** | intended |
| Mcrash | 9 | pre-kill head 35, replayed alone 32; the pre-kill parent is not canonical after the replay (`Mcrash.log`) | **intended** | intended |
| Mhu | 5 | a's head became c's 27 (a/b 26), reorgs 0→1 (`Mhu.log`) | **intended** | intended |
| Mwants | heal counters | a/b syncSessions 0→1 each (`Mwants.log`) | **intended** | intended |
| M2 | heal | c kept 27 (a/b 25) after 120 s (`M2.log`) | **intended** | intended |
| M3b | heal | c kept 29 (a/b 27) (`M3b.log`) | **intended** | intended |
| Mheight | heal | c kept its higher, lighter 28 (a/b 26) (`Mheight.log`) | **intended** | intended |
| M4 | heal: exactly one reorganization | `M4-run2.log`: c reorgs 0→0. `M4.log` (first run) is not counted: it failed the step-3 precondition (§5) | **intended** (run 2) | intended |
| M5 | every miner produced blocks | `{"a": 0, "b": 0, "c": 0}` (`M5.log`) | **intended** | intended |
| M6 | malicious peer | wrong-network closed=False (`M6.log`) | **intended** | intended |
| Msess | 6 | a's head became c's 29, reorgs +1, sessions +1 (`Msess.log`) | **intended** | intended |
| Mreplay | 7 | a's replayed head was c's lighter 27 (`Mreplay.log`) | **intended** | intended |
| Mfollow | 11 | passed step 10 (prefix 1..101 identical); 11: no common totalWork within 60 s, heads a/b/c 107/126/127 (`Mfollow.log`) | **intended** | early (step 10's wait) |
| Msilent | 5 Status | new 4b Status check: d sent a Status 0.22 c tip blocks lighter than c's tip (`Msilent.log`) | early (new check) | intended |
| Mtransient | 5 counters | new 4b Status check: a Status 0.59 lighter (`Mtransient.log`) | early (new check) | intended |
| M1b | heal | `timeout waiting for a,b reach 15` (`M1b.log`) | early (liveness wait) | intended |
| M1a | — | `timeout waiting for a,b reach 15` (`M1a.log`) | early (liveness wait) | early |
| M2a | — | `user transactions included; receipts identical` (`M2a.log`) | early (receipts) | early |
| M3 | late join | `timeout waiting for c syncs` (`M3.log`) | early (liveness wait) | early |
| Mtie | survives by design | PASS 38/38 (`Mtie.log`) | survived (D133's tie-break is local; the unit test `fork_choice_heavier_tied_signed_first_and_rollback` catches it) | survived |

Totals: 23 mutants, 22 caught and 1 survivor (Mtie, explained). Caught at the intended check: 16,
including both review mutants at the new step 4c, and Mfollow now at step 11. Caught early: 6.
Msilent and Mtransient make an unsolicited block that is not better the head for a moment; in step
4b the lowest block of a/b's branch, sent last and directly, is such a block for d, so the new 4b
Status check sees the lighter head before step 5 is reached (round 3 showed that the step-5 checks
catch both: `../e2e-mutation-r3/Msilent.log`, `Mtransient.log`). M1b's lighter-tip rule stalled
the first 15 blocks this time; in round 3 it reached the heal.

VMorphAll is also caught by the consensus unit test
`a_session_block_arriving_before_its_parent_stays_protected`, and VMorphHi passes the consensus
and net unit binaries (review evidence, not re-run here).

### 4.3 Full pocold suite

`pocold-suite-ad84942.log`: `flock <scratch>/cpu.lock cargo test --release --locked` in
`prototypes/pocold` at `ad84942` (clean tree), `CARGO_TARGET_DIR=<scratch>/wf-target-r3-e2e`:
exit 0, every binary `ok` (lib 44, admission 3, caps 7, consensus 43 + 2 ignored, decoder 1,
journal_measure 1 + 6 ignored, net 35, secp 1, site 6, store 12). No Rust file changed in this
round; the run shows the crate still passes at the branch head.

### 4.4 Provenance checks

A scratch checker read every committed run log (27: 3 base, 23 mutants, M4's second run). Each
has the script sha256 `9c6d7c0d…`, worktree HEAD `ad84942` with no change under `prototypes/`, a
summary line, and a pocold sha256 equal to its build record (`builds.txt` here for base-r4 and the
VMorph binaries, `../e2e-mutation-r3/builds.txt` for the others). None is INCOMPLETE. The
VMorph edits were compared with the review's own diffs (`<scratch>/e2e-ver3/logs/diff-VMorph*.txt`):
only the file timestamps differ.

## 5. Scope and limits

- **One run per mutant** (M4: two, the first not counted). A mutant caught once is caught; one
  passing run of Mtie does not show it always passes.
- **A precondition can fail without a defect.** `M4.log`: a and b took 61 s and 13 blocks to
  lead c0 by 4 c0 blocks of work (4–7 s in the base runs), so c needed 18 late blocks and made 14
  in the 240 s climb bound. The run failed at the step-3 precondition, before anything M4 changes
  is checked (M4 only stops the reorg counter). That is neither a catch nor a pass; M4 was run
  again. The climb bound is unchanged since round 3; a bound that grows with the number of late
  blocks needed would remove this false FAIL (not done here: it would change the test after all
  runs were made).
- **Step 8b catches no mutant by itself** (as in round 3). Step 11 now does: Mfollow.
- **The new 4b Status check catches Msilent and Mtransient before step 5**, so in this round the
  step-5 checks are not shown to catch them by themselves; round 3 showed it on the same binaries.
  No mutant was aimed at the 4b check's lower bound alone: under VMorphAll the first block released
  from d's orphan pool was already heavier than c's tip (+0.65 c tip blocks), so that bound did not
  apply. Step 4c is what catches both orphan-path mutants.
- **The step-9 changes were not exercised by a race**: in every base run b's replayed head was its
  pre-kill head, and the block one below it was unchanged. The torn-tail allowance was not used.
- **No node code changed**; only `tests/e2e_devnet.py`.
- **Not a full acceptance.** The full integration (`prototypes/lp10/run_lp10.py`) and the Windows
  workflow were not re-run on this branch.
