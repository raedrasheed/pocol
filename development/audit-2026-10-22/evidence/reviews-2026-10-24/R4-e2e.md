# Independent review: e2e devnet test, round 4 (`ad84942`, merged at `c959379`)

Reviewer worktree: `agent-a8a2b25b389f9ad93`, detached at `claude/audit-repair-a01-a14` tip `ec148cf`
while reviewing (restored to its own branch afterwards; no tracked file left modified, nothing committed
or pushed). The test file at the tip is byte-identical to `ad84942` (sha256 `9c6d7c0d…98814c`, 38 checks).
`chain.rs` and the fork-choice/sync parts of `node.rs` are unchanged since `c959379`. The only node.rs
change since then is 462fffc's A11 pool-rebuild walk (`detached_blocks`).

All scratch material is under
`/tmp/claude-0/-home-user-Proof-of-Collaboration/5c42c059-8cb3-5c8b-83e8-431ffbd082d1/scratchpad/r4rev/`:
- `mut.py`: the exact-string mutations;
- `patches/*.diff`: the diff of each one against pristine;
- `builds.txt`: binary sha256s;
- `logs/run-*.log`: one e2e run each;
- `logs/unit-*.log`: the unit runs.

## Answer to the owner's question

**For fork choice by work, the test detects wrong-branch selection, and it is not timing luck.** The
partition builds a winner that is strictly heavier and at least 2 blocks lower, and the test checks
that precondition. The heal, the orphan path in both directions, the relays with and without a session,
and the replay each assert the exact head, the reorg/session counters and every Status the node sends.
They catch every "longest chain" or "skip fork choice" mutant tried, in the round-4 evidence and in this
review's Nwantsh (a height-based sync trigger, caught at the heal: c kept its higher, lighter tip).

**The test has one structural blind spot.** Its only oracle for "heavier" is the node's own `totalWork`.
A W(t) accounting bug that every node shares therefore passes. Nworkpar, which adds the parent's work
instead of the block's own, passes the e2e test 38/38 and every lib/consensus/store unit test.

**Coverage of the network mechanisms the test does not stress is weak:**
- Block relay: Nnorelay, where a node never forwards a peer's block, passes the e2e test and all unit
  tests run.
- Status refresh: Nhello1, where a peer's Hellos after the first are ignored, passes both too.
- Multi-round sync (Nround1) and reorgs deeper than about 10 blocks (Nreorg16) pass the e2e test, but
  unit tests catch both.
- Tie-break order: Mtie and Nlongtie pass the e2e test. Mtie is caught by 5 unit tests, including the
  one named in the evidence. Nlongtie, "higher wins on equal work", is caught by nothing.

The base run at the tip passed 38/38 (299 s). None of the 9 runs in this review failed for timing.

## 1. Mutant coverage: claimed check → exact assertion (`prototypes/pocold/tests/e2e_devnet.py`)

| Claimed check | Assertion (line) | Would a realistic bug fail it? | Weaknesses |
|---|---|---|---|
| Precondition: heavier AND ≥ 2 lower | 263-264 | It is a guard, not a detector | Uses node-reported `totalWork` (71, 238). See R4-1 |
| Heal, frozen (Status sync) | 279 (heads), 280, 281, 283 (reorgs == 1), 285 (a/b counters) | Yes: M2, M3b, Mheight, Mwants, M4 and this review's Nwantsh were caught here | 281 is vacuous for a and b (they are lower than `c_tip`, so `canon` returns None). See R4-9 |
| Orphan path, heavier branch (d) | 299-300 (adopt, reorgs +1, sessions 0), 303-304 (Status sequence) | Yes: Morph, Mcount, Msilent, Mtransient | — |
| Orphan path, lighter branch (f) | 318-320, 323-324 | Yes: VMorphHi, VMorphAll | The Status check reads only the connect Status unless the head moves. The canonical sub-check is implied by the head check |
| Relay without a session (a) | 333 (precondition), 340-341, 343-344 | Yes: Mhu, Msilent, Mtransient (round 3) | `canon(0, c_tip-1) != late[-2]` is implied by head == ab_tip (lower) |
| Relay in a session (a) | 358-359 (sessions +1, reorgs 0, head), 361-362 | Yes: Msess | — |
| Replay after the relay | 367-368 | Yes: Mreplay | Journal order is one fixed order (ab first, c last) |
| Heal under mining (e) | 381-388 | Only liveness. The live chain is heavier AND higher, so a longest-chain node passes (stated in 373-377) | No mutant is caught here. See R4-11 |
| Crash replay | 399-401 | Yes: Mcrash | The torn-tail allowance is unexercised (README §5) |
| Convergence while mining | 414, 420 | Yes for permanent splits (Mfollow at 444) | Compared only CONFIRM = 6 deep |
| Frozen tips | 442-452 | Equal totalWork; siblings allowed one block deep | Accepts either outcome by design. See R4-8 |
| Full-restart replays | 470, 475 | Determinism of replay | — |

## 2. Mutation evidence (`development/audit-2026-10-22/evidence/e2e-mutation-r4/`)

- **Base runs.** `base-r4-run1..3.log` are clean: PASS 38/38 each, with script sha `9c6d7c0d…`
  and base-r4 binary `ddabdea0…`, matching `builds.txt`.
- **Attribution.** For each mutant log, the first FAIL is the check the README names (16 at the
  intended check, 6 early, Mtie survived). Spot checks:
  - Mhu fails at 340 with a's head on c's block and reorgs 0→1.
  - VMorphHi and VMorphAll fail at 318 with f's head moved.
  - Msess fails at 358 with reorgs +1.
  - Mwants fails at 285 with sessions 0→1.
- **Was the mutant really applied?** The committed logs carry only binary sha256s. The patches are not
  committed; they exist only in scratch. I found them in `scratchpad/e2e-r3/patches/*.patch`, and each
  matches its README description. The sha256s of 5 scratch binaries (Mtie, Mhu, Morph, Msess, Mheight)
  and of the 3 r4 binaries match the hashes in the logs. Provenance is therefore real but cannot be
  checked from the repository (R4-12).
- **Mtie.** The committed unit evidence (`../e2e-mutation/unit-tiebreak-Mtie.log`) is from the older
  `a03e089` sources. Re-verified at the tip by applying the same inversion
  (`self.key(&cx)>self.key(&cy)`, identical to `Mtie.patch`) and running
  `cargo test --release --offline --lib --test consensus --test store`. Five consensus tests fail:
  - `fork_choice_heavier_tied_signed_first_and_rollback`, at `tests/consensus.rs:96` "signed sibling
    wins the tie": this is the claimed test, so the claim holds;
  - `d133_tie_break_at_depth_holds_through_eviction_and_refetch`;
  - `replay_gives_a_reimported_block_a_new_import_seq`;
  - `a_crash_around_an_adoption_reopens_on_one_whole_chain`;
  - `the_r_keep_boundary_removes_a_side_block_exactly_r_keep_below_the_head`.

  The oracle `best_of` (`tests/consensus.rs:76`) is independent of `Chain::better`. Log:
  `r4rev/logs/unit-Mtie.log`.

## 3. New mutants (this review), one e2e run each at tip `ec148cf`

Build: `CARGO_TARGET_DIR=…/scratchpad/review-target cargo build --release --offline --bins`.

The target directory is shared, and other sessions overwrote `release/pocold` during this review: at
16:09 it held agent-a4df4c2ed's build. Each binary was therefore copied from
`release/deps/pocold-cc8816738e37b2e1`, this worktree's artifact, which embeds this worktree's path; the
copies are in `r4rev/bins/<name>/`.

Run: `POCOLD=<bin> flock e2e.lock flock cpu.lock python3 -I prototypes/pocold/tests/e2e_devnet.py`.

Unit runs: `cargo test --release --offline --lib --test consensus --test store`. The 3 ignored deep tests
were added (`--include-ignored`) for base and Nreorg16. `tests/net.rs` was not run; grep finds no block-relay
or Status-refresh test in it. Builds had 0 warnings. Sources were restored after each build, and the
sha256 of the restored files was checked against pristine.

| Mutant | Change (exact) | e2e result | Unit tests |
|---|---|---|---|
| base | — | **PASS 38/38**, 299 s (`run-base.log`) | lib 46, consensus 46 incl. ignored, store 12: all ok |
| **Nwantsh** | `wants()`: `st.work>=head.total_work` → `st.height>=head.height` (sync trigger by claimed height) | **caught at the intended check**: heal, 279. c kept its higher, lighter 29; a/b at 27 after 120 s | — |
| **Nworkpar** | `validate()`: `total_work = p.total_work + work(p.target)` (the parent's work, not the block's own) | **survived 38/38** (late c 2.53 against a/b 5.37 c-tip blocks, so still lighter) | **survived all** (lib, consensus 43, store) |
| **Nu64** | `better()`: work compared on `as_limbs()[0]` (u64 that wraps) | **survived 38/38** (totalWork ≈ 0x17_0000 < 2^21) | **survived all** |
| **Nlongtie** | `better()`: on equal work, the higher block wins before D133's key | **survived 38/38** | **survived all** |
| **Nreorg16** | `insert()`: refuse a head switch whose LCA is > 16 below the head | **survived 38/38** (deepest reorg in the run: c 30, fork 21, 9 deep) | **caught**: 10 consensus tests, e.g. "heavier branch forked at depth R_keep (2048) is adopted" |
| **Nround1** | `sync_tick()`: a session never asks a second round | **survived 38/38** | **caught**: `range_sync_reaches_the_peer_head_through_truncated_responses`, `a_long_catch_up_is_not_bounded_by_the_protected_branch_size` |
| **Nnorelay** | `import_with()`: blocks with `from=Some(peer)` are not re-broadcast | **survived 38/38** (frozen tips: 3 distinct equal-work siblings at 107) | **survived all** |
| **Nhello1** | `on_msg(Hello)`: every Hello after a peer's first is ignored (no Status refresh) | **survived 38/38** | **survived all** |
| Mtie (unit only) | key comparison inverted | survived in the evidence (`Mtie.log`) | **caught**: 5 consensus tests (§2) |

Summary of the 8 new e2e mutants: 1 caught (Nwantsh), 7 survived. Of the 7 survivors:
- 2 are caught by unit tests: Nreorg16 and Nround1;
- 5 pass every test run: Nworkpar, Nu64, Nlongtie, Nnorelay and Nhello1.

## 4. Findings

### R4-1 MEDIUM: "heavier" is decided only by the node's own totalWork, so a shared W(t) bug passes
- **Where:** `e2e_devnet.py:71` (`tip()` reads `totalWork`), `:238`, `:242`, `:263-264`, `:304`, `:444`.
- **Claim:** every work comparison uses the node-reported `totalWork`, the very quantity fork choice
  computes. A W(t) bug common to all nodes keeps them in agreement and keeps "a/b heavier", so every
  check passes although D133's W(t) = Σ work from genesis to t (consensus.md §9, work(t) = floor(2^256/(t+1)),
  consensus.md:117) is violated.
- **Evidence:** Nworkpar (`chain.rs` validate, `total_work:p.total_work.saturating_add(work(&p.target))`)
  passed 38/38 (`r4rev/logs/run-Nworkpar.log`). It also passes the lib, consensus and store unit tests
  (`unit-Nworkpar.log`). No unit test compares a stored `total_work` with Σ work(target): see
  `tests/consensus.rs:94` and `:99`, which compare sums built from stored values.
- **Status:** REPRODUCED.
- **Fix:**
  - In the e2e test, after the heal and at the frozen tips, walk the canonical chain over RPC and check
    `totalDifficulty(n) == totalDifficulty(n-1) + difficulty(n)`, with `totalDifficulty(0) == 0`.
  - Check `difficulty == 2^256 // (target+1)`, decoding `target` from `pocol_getBlockRlp`'s header in
    Python.
  - Base the heavier/lighter precondition on the Python-computed sum.
  - Add a unit test asserting `entry.total_work == parent.total_work + work(header.target)` for every
    imported block.

### R4-2 MEDIUM: block relay and Status refresh are never needed in the topology, so both can be removed undetected
- **Where:** `e2e_devnet.py:150-160` (every node dials every other: a full mesh); every peer process
  is (re)started fresh before it must sync (`:276`, `:371`, `:380`, `:402`).
- **Claim:**
  - Every node receives each block directly from its miner, so a node that never forwards peer blocks
    is never noticed. Relay is required: network.md "استلام block" step 3, "… ثم البث" ("… then the broadcast").
  - Every sync starts from a first Hello on a new connection, so Status refreshes, the later Hellos
    that sync_tick's round-robin relies on, are never needed.
- **Evidence:** Nnorelay and Nhello1 each passed 38/38 (`run-Nnorelay.log`, `run-Nhello1.log`). Both
  also pass lib, consensus and store (`unit-Nnorelay.log`, `unit-Nhello1.log`).
- **Status:** REPRODUCED.
- **Fix (cheap, inside existing steps):**
  1. In step 5, keep a second test peer connected to a, which claims genesis and only reads. Assert
     that it receives the relayed c blocks as Block frames (catches Nnorelay).
  2. In step 6, send the session-claiming Hello as a *second* Hello on the step-5 connection, not on a
     new socket. Assert a GetFrom within SESSION_GAP_MS + slack (catches Nhello1).
  3. Optionally run 8b on a line topology: e dials only c.

### R4-3 LOW: the D133 tie-break is not covered end-to-end, and "height on equal work" is caught by nothing
- **Where:** `e2e_devnet.py:436-452` (siblings accepted); `chain.rs:613-626`.
- **Claim:** ties are local by D133, so the e2e test cannot check the order. That is accepted and
  documented for Mtie. But Nlongtie, which on equal W prefers the higher block (a D133 violation: the
  key at the LCA children decides), passes the e2e test **and** every unit test.
- **Evidence:** `run-Nlongtie.log` PASS 38/38; `unit-Nlongtie.log` all ok. Mtie, by contrast, is
  caught by 5 unit tests (§2).
- **Status:** REPRODUCED.
- **Fix:** add a unit case with two equal-W tips of different heights (one branch with fewer, heavier
  blocks), whose LCA children's keys order them against the height. Assert `head == best_of()`.

### R4-4 LOW: cumulative work never exceeds about 2^21 in the devnet, so a width or overflow bug in W is invisible
- **Where:** `e2e_devnet.py:185` (`--work-bits 15`); `chain.rs:618`.
- **Claim:** comparing W truncated to 64 bits (or even 32) is indistinguishable at devnet scale, and no
  unit test uses work above 2^64.
- **Evidence:** Nu64 passed the e2e test 38/38 and every unit test run. Observed totalWork values are
  `0xd78a0`–`0x178b64`.
- **Status:** REPRODUCED.
- **Fix:** unit-test `Chain::better` on synthetic entries with `total_work` values differing only
  above bit 64, or across 2^64. The e2e test cannot reach this scale.

### R4-5 NOTE: reorg depth and multi-round sync are exercised only shallowly end-to-end (unit tests cover them)
- **Where:** `e2e_devnet.py:242-260`. The fork is 7-10 blocks below c's tip in every run here.
- **Evidence:** Nreorg16 (refuse reorgs deeper than 16) and Nround1 (no second GetFrom round) each
  passed the e2e test 38/38. Unit tests catch both: 10 tests and 2 tests respectively
  (`unit-Nreorg16.log`, `unit-Nround1.log`).
- **Status:** REPRODUCED.
- **Fix:** none needed for the e2e test. State the scope in the docstring: reorgs ≤ about 10, single-round
  sessions.

### R4-6 LOW: the climb bound is fixed while the number of late blocks needed varies (false FAIL)
- **Where:** `e2e_devnet.py:259` (`240` s).
- **Claim:** the number of late blocks c must mine is `ab_tip − c0 + 2`. It depends on how long a/b took
  to reach LEAD, while the bound does not. A slow a/b lead fails the precondition with no defect. This is
  a false FAIL only, never a false pass.
- **Evidence:** the committed `M4.log`: 18 late blocks needed, 14 made in 240 s. The README states this
  in §5 but leaves it unfixed.
- **Status:** CODE-READING plus the committed log.
- **Fix:** make the bound `max(240, 20 * need)`, or cap the a/b lead phase by height (stop a/b when
  `h >= c0 + k`), so that `need` is bounded.

### R4-7 LOW: frozen-tip allowance of one block can false-FAIL on a legitimate deeper equal-work fork
- **Where:** `e2e_devnet.py:446-452` (`shared >= low - 1`).
- **Claim:** D133 keeps equal-W tips apart by local import order. An equal-W fork 2 or more blocks deep
  (sibling fallback blocks share a timestamp, so their children share a target) is legal and persists
  among frozen nodes, because each keeps the branch it imported first.
- **Evidence:** CODE-READING. Not observed: every run here had shared ≥ low − 1, and in Nnorelay 3
  distinct siblings sat 1 deep.
- **Status:** CODE-READING.
- **Fix:** assert equal totalWork, plus agreement up to the LCA of the tips. Allow any depth among
  equal-W branches, but require the lower part to agree at a CONFIRM-deep point.

### R4-8 NOTE: the counter checks after the heal have no settle window
- **Where:** `e2e_devnet.py:284-285`.
- **Claim:** a/b counters are read immediately after the heads match, and the heal completes in 0 s. A
  bug that starts a/b sessions on a delayed path (sync_tick's round-robin, later than the first Hello,
  SESSION_GAP_MS = 1 s) could be missed. Mwants starts its session on the first Hello and is caught.
- **Status:** CODE-READING.
- **Fix:** wait 2·SESSION_GAP_MS + 1 s after the heal before reading `ab1`.

### R4-9 NOTE: vacuous or redundant sub-assertions
- **Where:**
  - `:281`: for a and b, `canon(i, c_tip.number)` is None because both are lower;
  - `:319`: the f canonical check;
  - `:341`: `canon(0, c_tip-1) != late[-2]`;
  - `:383`: `c_tip` not canonical on e, implied by the CONFIRM-deep block being a's;
  - `:431-432`: `late_c == len(late)`, true by construction, since c mined alone.
- **Claim:** each is implied by another assertion and never fails alone. They are harmless but
  overstate the number of independent checks.
- **Status:** CODE-READING.
- **Fix:** drop them, or label them as consistency checks.

### R4-10 NOTE: the "corrupted block" exercises an envelope reject, not a content reject
- **Where:** `e2e_devnet.py:223-232`.
- **Claim:** flipping a stateRoot byte in a signed header changes the template ID, so the template
  signature fails (item 3, envReject). H-full stateRoot validation (blockReject, item 17) is never
  reached. The comment "a stateRoot byte: still decodes, new hash" suggests otherwise, and the reason is
  not asserted.
- **Evidence:** `run-base.log`: `new rejects=[{… 'reason': '3 template signature'}]`; the committed
  `base-r4-run1.log` shows the same.
- **Status:** REPRODUCED.
- **Fix:** corrupt a fallback (unsigned) block, or re-sign the template with a non-drawn key, and assert
  the reason. Alternatively, rename the check.

### R4-11 NOTE: heal under mining is a liveness check only
- **Where:** `e2e_devnet.py:373-388`.
- **Claim:** as stated in the comment, the live chain is heavier and higher, so no fork-choice mutant
  is caught here. No new issue; recorded so that the "heal under mining" claim is not read as a
  fork-choice check.
- **Status:** CODE-READING.
- **Fix:** none required.

### R4-12 NOTE: evidence provenance and what the evidence covers
- **Where:** `evidence/e2e-mutation-r4/README.md` (front matter, §4.4).
- **Claim:**
  - The patches for all 23 mutants are scratch-only. A reader of the repository cannot see what each
    binary contained; only sha256s are committed.
  - The mutant binaries were built from the e2e-track tree (`fa97594` export ≈ `ad84942`), not from
    the merged `c959379`, which also differs in lib, net, resources and rpc. Since then the branch has
    462fffc's node.rs change.
  - M1b's intended check (the heal) was not reached in round 4: it was caught early at genesis.

  These do not change the verdicts: this review's base and its 8 mutants ran at the tip, and the
  fork-choice and sync code is unchanged.
- **Status:** CODE-READING.
- **Fix:** commit `patches/*.patch` beside the logs, with the base commit each applies to.

### R4-13 NOTE: shared machine and ports
- **Where:** `e2e_devnet.py:143-144`, `:154`.
- **Claim:** isolation from other sessions relies on the flock convention and the busy-port checks. A
  foreign session's e2e run on the same ports between the check and the bind would make this test talk
  to foreign nodes. During this review other sessions' cargo builds and tests ran concurrently. None of
  the 9 runs false-failed; the bounds are generous: worst observed wait, "a,b reach 15" about 68 s of 120 s.
- **Status:** CODE-READING.
- **Fix:** use per-run port offsets, for example from an environment variable.

## 5. Flakiness summary (question 4)

- **False passes:** none found from timing. Every positive check is bounded and asserted after the wait.
- **Status checks:** the Status-sequence checks read buffered frames after the wait, so a transient head
  change sent before the read is still seen.
- **The c precondition:** the condition that c's late branch is lighter is asserted, not assumed.
- **False FAILs:**
  - R4-6, the climb bound: observed in the evidence (M4.log).
  - R4-7, the one-block sibling allowance: theoretical.
  - The 60 s / 120 s / 180 s bounds are wide relative to observed times: heal 0 s, d about 1 s, e 3-8 s,
    lead 3-7 s.
- **PAUSE = 120 s:** a fixed wall-clock wait, correctly greater than D_att + Γ = 46.
