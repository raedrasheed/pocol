# C-1 S1: canonical history off the heap (2026-10-24)

The owner approved this on 2026-10-24 (D-1 option (a)). The design and acceptance tests were frozen
before the code: proposals/C1-STORAGE-DESIGN.md, sections "S1" and "S1 status".

**What is unchanged:** validity, roots, receipts, genesisHash, the wire format and the answers of an
intact node.

**What is new (local behaviour of a damaged store, which could not exist before S1):**
- open runs an integrity check and rebuilds a damaged store;
- while a store is marked damaged, its RPC reads answer −32603;
- an import with a damaged store stops the node with exit 70.

## Code

- `prototypes/pocold/src/history.rs` (new): `history.redb`, with two tables:
  - RECEIPTS, sealed per record;
  - TXIDX.

  How the store is checked and recovered:
  - open catches panics inside redb and runs `check_integrity`;
  - a damaged store is set aside and rebuilt;
  - the damaged marker survives a restart;
  - replay commits without fsync, and syncs every 256 blocks and at the end.
- `src/chain.rs`:
  - `history_window` and `history_cursor`;
  - `retain` offloads canonical journaled blocks below head − window + 1;
  - `restore_canonical` and `canonical_resident` act only inside the window.
- `src/node.rs`:
  - `history.put` before the journal append (I3), and at replay;
  - `view` materializes from the journal plus history.redb, and returns None on any history error;
  - P2P GetFrom and GetRange serve through `block_msg`;
  - `Limits::history_window`;
  - status fields historyRefilled, historyRebuilt and historyDamaged.
- `src/rpc.rs`:
  - every history read goes through `whole`, which answers −32603 when history is unavailable;
  - `find_tx` uses TXIDX, takes the canonical entry, and checks it against the block;
  - a getLogs or getHeaders that cannot read a block answers an error, not a shortened list.
- `src/resources.rs`: the A13 history group, rebuilt as listed in `A13-DERIVATION.md` §2, plus
  `HISTORY_FILE_FACTOR` and the `redb-representation` condition.
- `Cargo.toml`: `redb = "=2.6.4"`. Its only runtime dependency, libc, is already in the lock.

## Tests (`tests/consensus.rs`, `tests/journal_measure.rs`)

| test | what it shows |
|---|---|
| `canonical_history_off_the_heap_answers_as_it_did_resident` | <ul><li>A node with the default window answers like a reference node that keeps everything resident (`history_window = u64::MAX`). The answers compared: every block, transaction, receipt, getLogs, getHeaders, getBlockRlp and P2P Block message.</li><li>The chain: an early reorg plus 96 blocks, with 24 log-emitting creates.</li><li>Exactly the blocks below the window are offloaded.</li><li>The transaction index is checked against the blocks themselves; the abandoned block's transaction is not found.</li><li>The answers match again after each event: reopen; deletion, junk overwrite and truncation of history.redb (truncation made redb panic before the fix); a simulated crash between the history write and the journal append (the orphan entry is never read, and the block imports afterwards).</li><li>A third node syncs the whole chain over P2P from the offloading node alone.</li></ul> |
| `a_damaged_history_store_is_never_served_and_is_rebuilt` | <ul><li>39 single-byte flips: 5 inside receipts records, 5 in the tx half and 5 in the block half of index keys, 24 spread over the file. Every targeted flip is caught at open and the store rebuilt; no flip goes undetected with a changed answer.</li><li>Two kinds of wrong content that pass redb's checksums: a receipts record under another block's key, and an index entry at the wrong position. Each is caught by its reader; every answer is then equal or −32603, and the next open rebuilds.</li><li>Before the open check was added, 8 of the 10 key flips gave a silent "not found".</li></ul> |
| `a13_bound_breakdown` | <ul><li>The devnet total is pinned at 4,323,240,949 B.</li><li>A window of the whole chain reproduces the pre-S1 total of 5,487,223,178 B, once history.redb's terms are removed and the logs term is taken back to LOG_CAP·33/32.</li></ul> |
| `history_integrity_check_cost` (ignored measurement) | the cost of redb's full check, and the file size over its stored bytes |

## Results

- `cross-version.log`:
  - the reference answers on the tree before S1 (`e6025ce`) and on S1 are **byte-identical**:
    1,156,012 B, sha256 4c3183ea…1572;
  - the scratch test that produced the pre-S1 dump is `pre-s1-dump-test.rs.txt`.
- `a13-devnet-breakdown-s1.log`: devnet model 5,487,223,178 → **4,323,240,949 B**. The gate needs
  4,940,846,799 B, leaving 1,432.0 MiB unused.
- `a13-network-profile-s1.log`:
  - history group 19.19 → **4.28 GiB**, of which 4.25 GiB is offloaded per-block metadata (S4);
  - model 105.83 → **90.93 GiB**, gate **103.92 GiB**.
- `history-integrity.log`: the full check takes about 0.7–1.4 s per GiB of store with a warm cache. The
  file was at most 4.1 times its stored bytes; the A13 allocator term counts 8.
- `mutants.log` / `mutants.json`: 12 mutants, each applied to a copy of the `b517b0f` tree and built in its
  own target directory, against the two S1 tests. The base passes and **all 12 are killed**.
- `suite-lp4.log`, `suite-pocold.log` (release, `b517b0f`): **lp4 173 passed / 0 failed; pocold 189
  passed / 0 failed / 17 ignored**. The ignored tests include the new integrity-cost measurement.

## Open

- The integrity check reads the whole file at every open. With D-6 checkpoints, open will no longer
  replay from genesis, and the check then becomes open's main cost. Running it in the background, or
  verifying only what was written since the last clean close, is local policy for D-6.
- S4 moves the remaining 4.25 GiB of offloaded per-block metadata. S3 and S5 move the full state copies
  (75.12 GiB).
