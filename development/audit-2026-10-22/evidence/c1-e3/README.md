# C-1 E3: the executor, the pool and RPC on overlays (2026-10-25)

This covers steps E3.1–E3.3 of the S5 + S3b design (proposals/C1-STORAGE-DESIGN.md, sections "E3
design" and "E3 status"), under D-1 option (a), which the owner approved. E3.4, the head extended in
place, is not done yet.

**Unchanged:** validity, every root, receipt and RPC answer, and the Ok/Err outcome of every block.
Under the reference check, every block is applied a second time on a copy and compared field by field.

## Code

- **`prototypes/lp4-execution/src/view.rs` (new):**
  - the traits `View` (reads) and `WorkState` (writes), which `InMemoryDB` implements with the code they
    replace;
  - `Overlay`, the writes over a borrowed parent: `top`, `cleared`, the tombstones `deleted`, `retain`
    and the BLOCKHASH window. It provides `merge` (the state the copy would reach) and `touched` (the
    complete write set);
  - `Layer` and `LayerView`, the writes kept without the parent;
  - the unit test in `view/tests.rs`.
- **The lp4 executor reads and writes through the traits:**
  - `state_view`, `code_caps`, `collision`, `pay`, `native`;
  - `user_phase` (`execute_user_phase_on`, `commit_tx_on`);
  - `apply` (`Job::run<S>`, generic `Exec`, `BlockOutput.writes`, `same_outputs`);
  - `state_summary` (`moved` on views).
- **pocold:**
  - `Entry.state` is an `Arc<InMemoryDB>`;
  - `StateDiff::between_written` and `of_output`. Under the reference check the diff is applied in both
    directions and checked against the child and the parent;
  - the pool's `Layer` over the head's `Arc`;
  - template selection on an overlay;
  - RPC `TagState`; `eth_call` on an overlay;
  - `resources.rs`: `TRANSIENT_SNAPSHOTS` 10 → 2, `POOL_LAYER_SNAPSHOTS`, `overlay_max`.
- **New test:** `tests/consensus.rs::e3_pending_answers_are_the_pool_layer`.

## Results

| | |
|---|---|
| validation at the near-cap devnet state (`validation-near-cap-e3.log`, reference checks off, `accts-end` of the `all` series: 18,833 accounts, 46,636 slots) | empty child 62 → **8.5 ms**; 380 transfers 174 → **55 ms**; CREATE2-heavy 122 → **17 ms**; 149 one-create transactions 154 → **32 ms** (E2 → E3) |
| devnet model (`a13-devnet-breakdown-e3.log`) | 3,489,051,177 → **3,186,963,844 B** |
| network model (`a13-network-profile-e3.log`) | 40.20 → **28.08 GiB**, gate 32.09 GiB. Still far above 6 GiB; see the decision package for what E4 and E5 must remove |
| suites (`suite-lp4.log`, `suite-pocold.log`, release, reference checks on) | lp4 187 passed / 0 failed; pocold 200 passed / 0 failed / 17 ignored |
| mutation controls (`mutants.log`) | 19 mutants. Pass 1 kills 17; the strengthened overlay test kills E3M5 in pass 2. **18 of 19 killed**; E3M7 is equivalent (below) |

**The A13 recount.** These are the full copies that remain, from the code (`resources.rs`):
- a block's merged child (one);
- a rolling re-execution's state and its next block's merged state (two). The pool's base after a new
  head shares the previous head's `Arc` until `rebuild_pool`, and is counted within the same two, since
  `restore_canonical` does not overlap a rolling re-execution;
- the pool's layer, counted as one whole state. The pool admits only within the block caps, and its gas
  is bounded only by POOL_MAX_TXS × GAS_LIMIT until D-7's value is approved.

The applied block's overlay is a diffs term (`overlay_max`). The table-growth copy stays.

**The survivors:**
- **E3M5** (a cleared account not written whole): execution cannot reach it as far as I can tell. A
  CREATE onto an account with storage is a collision, and a self-destruct changes existence, so the account
  is written whole anyway. Pass 2's unit test now requires every changed slot to be in the write set.
- **E3M7** (a self-destruct not marked cleared): **equivalent** on normalized commit sequences, the only
  ones the overlay models. The normalization after every commit marks a destroyed account cleared and
  deleted. The insertion is kept as a defence.

## Provenance

- The suites ran on the prototypes tree `3b2e0baaa42f26b57ef5686b937c7f7c5241fc46`, the tree of the E3 commit.
- The mutation runner and its copies are in the scratchpad, not in the repository.
- The timing ran on this container, one run, with no other cargo job running.
