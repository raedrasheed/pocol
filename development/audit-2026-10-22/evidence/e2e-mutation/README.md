# e2e devnet: a heal check that fails on a wrong choice, and its mutation validation

This directory holds only new files. The test change is in commit `bfeb226`
(`prototypes/pocold/tests/e2e_devnet.py`, sha256 `31cace51…6fd76`). No node code was changed. Every
run below used pocold built from the a03e089 tree, whose code is identical to `3c3792e`
(base binary sha256 `d542b31e…dc617`). The mutants were built from that tree plus one patch each.
The mutant patches and the runner exist only in the session scratch directory and are not committed.

## 1. Why the 3c3792e heal check was not enough

In 3c3792e the heal check waited until all three heads were equal (up to 120 s). It then required:

- the common head's totalWork ≥ max(c_tip, ab_tip);
- at least one reorg summed over all nodes.

That check had four gaps.

- **Convergence on the lighter branch could pass.** Every miner kept mining during the wait. Once
  the converged chain grew past the heavier tip's work, the check passed. In old-test run
  `old-M1b` below, the lighter-preferring mutant M1b passes this check.
- **The reorg sum was not about the heal.** The counters count every reorg since each process
  started, including 1-block tip races. In the old-test runs here the counters read a/b/c =
  [4, 6, 0], [10, 12, 1] and [4, 4, 1] (`old-M1b.log`, `old-M3b.log`, `old-M6.log`), so the sum was
  ≥ 1 without the heal reorg.
- **No tip was fixed when it was sampled.** c_tip was read 8 s after c's 2-block wait, and c went
  on mining until SIGTERM. ab_tip was read from a while a and b kept mining. So neither side was
  known to be heavier.
- **The heal could pass without the specified sync path.** With miners running, nodes heal through
  new-block broadcasts and missing-parent requests. A node that never syncs from a peer's Status
  still converges (`old-M3b`: PASS). consensus.md:235 says a branch that becomes heavier is fetched
  by sync via Status.

## 2. What the test now does (partition, heal, relay)

1. **Isolation.** c restarts isolated, with no listener and no peers, and keeps mining.
2. **a/b branch, then a/b frozen.** a and b extend their branch at least 2 blocks past c's isolated
   head. The fork point is at most that height, so their reorganization is at least 2 deep. Then a
   and b restart without a miner and settle (equal totalWork). They may keep two equal-work
   siblings of one kind, because that D133 tie is local. Both tips are recorded as lighter tips.
3. **c becomes strictly heavier.** c mines until its work is strictly greater than every a/b tip,
   with a bounded wait of 300 s.
4. **Recorded before the heal:**
   - the fork point of each a/b tip with c;
   - a's branch above the fork, as RLP;
   - a's and b's reorg counters.
5. **c frozen.** c restarts without a miner and joins. From here **no tip can move**, so c's tip is
   the known winner.
6. **Heal checks** (bounded wait 120 s). Nothing is mined, so no new block is broadcast, and the
   heal can only start from Status sync:
   - every node's head is exactly c's tip;
   - on every node, the canonical block at c's tip height is c's tip;
   - no a/b tip is canonical on any node;
   - the reorg counters of a and b moved.
7. **Relay.** A test peer connects to c. It sends a valid Hello that claims genesis, so no sync
   session starts with it. It then sends the lighter a/b branch to c, parents first. c never held
   that branch's tip, which was mined after c was isolated. c must store the branch, which makes
   the check non-vacuous, and keep its head. The relayed tip must not be canonical: per
   consensus.md:215 the head is the ≻-maximum, whatever the order of arrival.
8. **Restart.** All three nodes restart with miners, and the rest of the test (crash, ≥ 100 blocks,
   convergence, replay) runs as before.

**Other changes in the same commit:**

- **Malicious peer.** The old Hello had 6 fields, so it was refused for its arity, not as a wrong
  network. The corrupted block was sent before any Hello, so it was closed unread (rejects=0 in
  every recorded run). The test now sends:
  - a 7-field wrong-network Hello;
  - a well-formed Hello as the control, which must stay open;
  - the corrupted block (one stateRoot byte flipped) after that Hello, which must add a
    `pocol_getRejects` entry.
- **Processes and ports.** Every node process is recorded in `pids.json` (the isolated c was not,
  so a failing run leaked it). Stops wait for the ports to close instead of fixed sleeps. Busy
  ports refuse the run.
- **Miner checks.** The `mined` counter is added up before every stop. A new check requires every
  miner to have won canonical blocks.
- **pocol-sign** is taken from beside `POCOLD`.

## 3. How it was run

Each run used one command. The exact command is in the header of each log, with the sha256 of the
script and the binaries:

```
POCOLD=<bins>/<build>/pocold flock <scratch>/e2e.lock flock <scratch>/cpu.lock \
  timeout -k 30 1800 python3 -I prototypes/pocold/tests/e2e_devnet.py      # cwd prototypes/pocold
```

- **Harness timeout:** 1800 s. A run stopped by it counts as INCOMPLETE, never as caught or as pass.
  No run reached it: every exit code is 0 or 1 (`summary-new-test.txt`, `summary-pre-change-test.txt`:
  one JSON line per run, as the runner wrote it).
- **Bounded waits:** a "timeout waiting for …" verdict is the test's own named wait failing (exit 1).
- **Old-test runs:** the pre-change test (`git show 3c3792e:prototypes/pocold/tests/e2e_devnet.py`,
  sha256 `83b4ca4d…bd20d`) was run the same way from a scratch copy.

## 4. Results

### 4.1 Unmutated build: 3 consecutive clean runs (flakiness check)

| Run | Result | Checks | Fork / heal detail | Log |
|---|---|---|---|---|
| base-run1 | PASS | 25/25 | c=24 vs a/b=23 (one tip), fork 20, lead in 13 s; a/b reorgs [0,0]→[1,1]; relay 3 blocks; wins a/b/c 20/39/41 | `base-run1.log` |
| base-run2 | PASS | 25/25 | c=25 vs a/b=22 (two equal-work siblings), fork 19, lead in 13 s; [0,0]→[1,1]; relay 3 blocks; wins 32/26/42 | `base-run2.log` |
| base-run3 | PASS | 25/25 | c=25 vs a/b=23 (two equal-work siblings), fork 18, lead in 53 s; [0,0]→[1,1]; relay 5 blocks; wins 64/17/19 | `base-run3.log` |

The three runs were consecutive, with no other run between them.

### 4.2 Mutants against the new test (`bfeb226`)

| Mutant | Change (scratch patch) | Expected failing check | Observed (first FAIL) | Verdict |
|---|---|---|---|---|
| M1a | `better()`: inverted cumulative-work comparison (`<`) | No head can advance (a child is always heavier), so the run fails before the heal | `timeout waiting for a,b reach 15` (bounded wait, 121 s) | **caught, early.** It never reaches the heal, which is why M1b exists |
| M1b | `better()`: across a fork (neither tip an ancestor of the other), the lighter tip wins | Heal passes: each node switches to the first block of the delivered branch, then follows it. The **relay** check fails | relay: c's head became a's lighter tip `24:0x425c778e52` | **caught at the intended check** |
| M2 | `better()`: no reorganization below the head's parent (1-block tip races still resolve) | heal: every node's head is c's tip | heads a/b = 22 (own branch), c = 23 after 120 s | **caught at the intended check** |
| M2a | `better()`: only a descendant of the head can win (no reorganization at all) | Early divergence (tip races need 1-block reorgs) or the heal | `user transactions included; receipts identical` (a=15, b=31 at the first check) | **caught, early** |
| M3 | No sync session, and a missing parent is never requested | The first sync: the late join | `timeout waiting for c syncs` (bounded wait) | **caught, early** (non-convergence) |
| M3b | No sync session; missing-parent requests kept | Late join passes through broadcasts and parent requests. The heal fails, since nothing is mined and only Status sync can heal | heal: heads a/b = 23, c = 25 after 120 s | **caught at the intended check** |
| M4 | The reorg counter is never incremented | heal: a and b reorganized | a/b reorgs [0,0]→[0,0] (heads did converge) | **caught at the intended check** |
| M5 | The `mined` counter is not incremented when a node's own solution is imported | every miner produced blocks | `{"a": 0, "b": 0, "c": 0}` | **caught at the intended check** |
| M6 | A wrong-network Hello keeps the connection open | malicious peer: wrong-network Hello closes | wrong-network closed=False | **caught at the intended check** |
| Mtie | Inverted D133 tie-break (`key(c1) > key(c2)`) | Survives: see below | PASS 25/25 | **survived** |

Each mutant has a `<M>.log`; a failing run also has `<M>-nodes.txt`, the last 60 lines of each
node log.

**Mtie.** The e2e test cannot tell this mutant apart:

- a tie between siblings of one kind is local by design (D133), so it never decides convergence;
- the signed-versus-fallback order is inverted the same way on every node, so the nodes still
  converge.

The unit test `fork_choice_heavier_tied_signed_first_and_rollback` does cover it. It passes on the
a03e089 sources (`unit-tiebreak-base.log`) and fails on Mtie at tests/consensus.rs:96, "signed
sibling wins the tie" (`unit-tiebreak-Mtie.log`).

### 4.3 The same mutants against the pre-change test (one run each)

| Mutant | Pre-change test (3c3792e version) | New test |
|---|---|---|
| M1b | **Heal check PASSED** (c=30 vs ab=26; converged). The run failed later, at `timeout waiting for 100 blocks on all nodes` (400 s): the last head lines in `old-M1b-nodes.txt` are a h=97, b h=104 and c h=102. That is indirect and depends on timing | relay check, deterministic |
| M3b | **PASS 17/17, survived.** c kept mining, so a/b got its branch through broadcasts and parent requests | caught at heal |
| M6 | **PASS 17/17, survived.** The vacuous malicious check | caught |
| M2 | caught: `timeout waiting for heal convergence` | caught at heal |
| M4 | caught: reorgs [0,0,0]. A dead counter fails the sum, but tip races alone satisfy it | caught at heal |

These are single observations of a timing-dependent test, recorded as observed.

## 5. Review notes

**The miner counter is sound in its direction.** The counter counts only a node's own blocks
imported successfully (the solution path and the own-future retry). Each process's value is added
up just before that process stops. A block mined between that read and the stop is not counted. So
the sum can only be low: the check can fail spuriously, but never pass spuriously.

What the counter does not say is that the blocks became canonical, because an abandoned branch also
counts. The new check `every miner won canonical blocks` uses the chain itself (block `miner` =
winner = owner(nonce)), and it is deterministic in this scenario:

- c's isolated branch is canonical by construction;
- a and b mine heights 1..15 alone, so each wins at least one block with probability about
  1 − 2·2⁻¹⁵.

The classification note in `e2e-devnet-classification/README.md` rejected a canonical-winner check
because c could lose its branch and leave W_act. In this partition c's branch always wins, and it is
far shorter than W_act = 32.

**Other e2e_devnet checks.**

- The malicious-peer check was vacuous; it is fixed, see §2 and M6.
- The isolated c process leaked on failure; it is fixed.
- Fixed sleeps around stops were replaced by bounded waits for the ports to close.
- The late-join check compares heights only. It is kept, because hash, stateRoot and receiptsRoot
  equality over the whole confirmed prefix is asserted later.

**tests/e2e_site.py** creates no partition and no fork. It reads per-node site state after bounded
waits and does not assert head agreement, which its purpose does not need. It is unchanged. Its
`pocol-site`/`pocol-sign` paths are fixed to `ROOT/target/release`, so it does not follow `POCOLD`.

## 6. Scope and limits

- **One heal direction.** The two-node majority abandons its own branch for the minority's heavier
  one. The relay tests the opposite decision: a node holding the heavier branch receives the
  lighter one. A single node reorganizing to the majority uses the same path (Status, session,
  GetFrom, import) and is not separately asserted.
- **Tie-breaks.** The e2e test does not discriminate D133's tie-break (Mtie); the unit test does.
- **Not re-run:** the full integration (`prototypes/lp10/run_lp10.py`) and the other suites were not
  re-run on `bfeb226`. Only `e2e_devnet.py` changed. This is not a full acceptance.
