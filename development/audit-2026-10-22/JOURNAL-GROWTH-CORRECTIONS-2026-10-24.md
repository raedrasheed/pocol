# Corrections to JOURNAL-GROWTH-AND-REPLAY.md (from the independent review R3, 2026-10-24)

[JOURNAL-GROWTH-AND-REPLAY.md](JOURNAL-GROWTH-AND-REPLAY.md) stays as it was written. This note records
where review R3 ([evidence/reviews-2026-10-24/R3-journal.md](evidence/reviews-2026-10-24/R3-journal.md))
found it wrong or incomplete. For each item it also gives the state of the code now. Each figure below
is the reviewer's, from scratch measurements on a loaded host, unless marked otherwise.

## R3-3 (HIGH): open re-executed the chain once more after any reorg

- **The study said:** Open(H) = Σ per-record re-execution + b·H²/2 + JM-OPEN0. A shallow reorg is cheap
  at open.
- **The correction:** replay restored the adopted branches only once, at the end of open. That restore
  rebuilt from genesis and re-executed every canonical block up to the highest offloaded one. So a single
  late depth-1 reorg about doubled every open (+5.4–7.7 s at 1,600 blocks, 1,499 rebuilds), and every open
  bound of §3.3, §5.4 and §8 was about 2× too low for a chain that has had a late reorg.
- **Now:** fixed (`af2f3a8`). Replay restores each adopted branch when it is adopted, as live import
  does. A test pins it: the reopened node does exactly the rebuild and restore work live did, and before
  the fix it did 4 rebuilds and 76 re-executions where live did 0 and 2
  ([r3-fixes](evidence/reviews-2026-10-24/r3-fixes/README.md)). The study's open model now holds as
  written for this term.

## R3-2 (HIGH): a reverting create-heavy transaction repeats the worst state scan at every height

- **The study said:** whether the cumulative number of creates is bounded "is not analysed here". The
  per-TX_CAP-unit worst on a grown state is the MODEXP block.
- **The correction:**
  - Every successful inner create ran `current_total` over the whole state, before the enclosing frame's
    fate was known. A CREATE2 loop that ends in REVERT therefore pays all those scans, grows nothing, and
    can be placed at every height. That makes it about 1.7× the MODEXP block per TX_CAP unit on the
    all-caps state.
  - With it, the devnet worst open becomes about 12.6 h, not 7.3–7.45 h. The depth-2048 reorg at the tip
    becomes about 26 h, not about 15 h. Both are lower bounds.
  - The sentence "not analysed" is withdrawn: such creates are not bounded.
- **Now:** C-2 item 2 (`2ce03c1`) makes a create read only the journaled accounts, so the scan no longer
  grows with the state.
  - The non-reverting CREATE2 block on the near-cap state went from 4,862 ms to 562 ms on the same host
    ([c2-execution](evidence/c2-execution/README.md); measured, this track).
  - The reverting variant has not been re-measured. Its term is removed by construction, and its
    measurement is in the re-run list.

## R3-1 (MEDIUM): the devnet worst open double-counted TX_CAP

- **The study said:** about 7.3 h, plus about 8.5 min for "≤ 52 TX_CAP-bounded 380-transfer blocks".
- **The correction:**
  - TX_CAP is cumulative per branch. The 7.3 h chain already uses 274 + 19,451 = 19,725 of the 20,000
    transactions, so 0.72 of a transfer block remains, not 52.
  - The MODEXP transaction also pays the per-USER-transaction state term (25.9 ms on that state).
  - The bound was about 7.45 h before R3-2 and R3-3, and the 8.5 min term goes.
- **Now:** C-2 removed most of the per-transaction state term. Item 4 will change the per-block term. The
  bound is to be re-derived from the final measurement, and the original stays unchanged.

## R3-6 (MEDIUM): every new head re-executes the whole pool, with no gas bound

- **The correction:** `rebuild_pool` runs on every new head, not only on reorgs. 4,096 pooled
  near-GAS_LIMIT MODEXP transactions cost about 68 min per head on an idle host. The study's pool term
  (the abandoned branch's transactions only) is too narrow.
- **Now:** a bound changes which transactions the pool accepts, which is RPC-visible. It is decision D-7
  in the [decision package](proposals/DECISION-PACKAGE.md), and is not implemented.

## R3-7 (MEDIUM): after `462fffc`, an already-included detached transaction paid a full scan

- **The correction:**
  - The note's bound is right: up to R_keep abandoned blocks, about 34 min.
  - But `462fffc` removed the "already in the new branch" filter. Every such transaction paid one
    `validate_snapshot` of the head state before its nonce rejection: about 14–24 ms each, up to TX_CAP
    of them, so up to about 4.6 min on the devnet.
  - The study's lines that still say "≤ 1,024 blocks" and "≈ 17 min" are stale.
- **Now:** C-2 (`2ce03c1`) removed the scan. The pool executes on the already-checked pending state, and
  the nonce is checked first. The stale lines stand corrected by this note: read "≤ R_keep = 2,048
  abandoned blocks, ≈ 34 min".

## R3-8 (LOW): the network empty-block open range was too narrow

- **The correction:** the models that fit the six devnet points within 0.3 s give 62.6–100.6 min at the
  network H_END, not 64–69 min. The steps in `b` are hashbrown resizes (7,168 and 14,336 entries).
- **Now:** read the range as **63–101 min**. C-3 item 2 (an incremental canonical index) removes the
  b·H² term that drives the spread.

## R3-13 (NOTE): side_max is 2, not 8

`single_host_local()` sets `side_max` to SIDE_MAX = 2 (D-A13-BUDGET), and every measurement ran with 2.
The results are unaffected; only the profile table's text and §7 option 6 are wrong.

## Not corrected here

R3-9 (re-run spread under load), R3-10 (the open's transient memory), R3-11 (the `retain` cascade),
R3-12, R3-14 and R3-15 are listed in the review. They are queued with C-3, or need no change.
