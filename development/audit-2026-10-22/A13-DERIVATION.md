# A13 — resource derivation for the actual representation

Scope: the resident memory of one pocold node (single-host simnet devnet) and the R18/R21 facts that
admission feeds to the gate. The claim is an upper bound on the heap bytes the node's containers request
plus its reserved thread stacks, for the admitted parameter domain, **under the conditions of §4**. It is
not a measurement, and it is not an unconditional bound on the process's resident memory: allocator
overhead and fragmentation are not modelled (§4, item 3b), and some terms rest on code inspection
(items 11, 13 and 16). §2b separates the model total, the measured RSS of a run and a guaranteed ceiling
(there is none). Code: `prototypes/lp4-execution/src/resources.rs` (`memory`), `admission.rs`
(R18/R21 facts), `prototypes/pocold/src/resources.rs` (node bound, checked at `Node::open`).
Arithmetic is checked u128 throughout; an overflow rejects the profile.

## 1. One state snapshot (`resources::memory::snapshot`)

pocold keeps full `revm::InMemoryDB` snapshots (no trie sharing). A hashbrown table with n ≥ 1 entries
has at most max(4, next_pow2(⌈8n/7⌉)) ≤ 6 + 16n/7 buckets, each one entry plus one control byte, plus a
16-byte trailing group (32 bytes counted with alignment). Entry sizes come from `size_of` of the real
types: account entry 216 B, slot 64 B, code entry 96 B, `InMemoryDB` 168 B (Rust 1.97, revm 19.7.0).
The bound holds for every distribution of slots over accounts (each storage table costs at most
6 + 16s/7 buckets), and it includes the 256-entry BLOCKHASH window.

Assumptions and where they are enforced:
- Containers hold only live state. revm's `CacheDB::commit` stores every accessed slot (zero values and
  read-only slots) and the EIP-161 cleanup leaves `NotExisting` entries; `user_phase::normalize` (after
  every USER transaction and SYSTEM call) removes both and shrinks oversized tables, and
  `drop_unreferenced_code` drops unreferenced code at block end. Root-neutral: absent and zero read
  alike in an EmptyDB-backed state. Regression `a13_reads_and_empty_calls_leave_no_container_residue`
  (1000 fresh SLOADs and empty CALLs leave no residue); its mutation (normalization off) fails.
- Accounts ≤ ACCT_CAP + 5 programs (genesis `check_caps`; USER `Halt::AcctCap`, D-A03-CAPS).
- Slots ≤ SLOT_CAP (USER `Halt::SlotCap`) + SYS_SLOT_MAX (post-block check in `verify_candidate`).
- Code entries ≤ accounts + 2 defaults.
- Validation: `resources::tests::snapshot_model_bounds_measured_clone_bytes` measures the bytes a real
  `clone()` allocates (thread-local counting allocator) for spread slots, concentrated slots at
  power-of-two boundaries, a grown-then-cleared table and a real genesis; each is below the model.

## 2. Node bound (`pocold::resources::derive`)

| term | formula | enforced by |
|---|---|---|
| snapshots | (1 genesis + K_hist + side_max + 10 transient) × snapshot (before C-1; after C-1 E3: head, genesis, side_max, MATERIALIZED, 2 transient and the pool's layer, see the C-1 E3 row of the totals table), plus one snapshot for a table growing in a working copy (old and new tables alive at once, one table at a time) | `Chain::retain` (K_hist window; side states ≤ side_max, lowest work pruned first); transient: apply copy, USER copy, per-tx engine and PAY copies, production clones, pool state and its copies, rolling rebuild (2). The pool's pending state commits through the block's `commit_tx` (EIP-161 cleanup, normalization) and refuses a transaction whose effects would pass SLOT_CAP, ACCT_CAP or SYS_SLOT_MAX (`PoolMeter`, checked against a recount), so it is one snapshot like the others |
| code | (3 + side_max) × CODE_CAP · 9/8 + snapshots × 97 B per code entry + one execution's new runtime (max(GAS_LIMIT, PER_CALL_GAS)/200 · 9/8). From C-1 S2 (2026-10-24); before it: snapshots × (CODE_CAP · 9/8 + 97 B per code entry), counted unshared | `code_caps::validate_snapshot` (logical code per state, duplicates counted per account). A state copy shares its runtime buffers (revm `Bytecode`: `Bytes` and an `Arc` jump table; `journal_measure::retained_states_of_one_lineage_share_their_code_buffers`), and under Cancun code leaves a state only in its creating transaction. So one lineage holds one set. A full set each is counted for every side state, for a rebuild or restore in progress, and for the pool's pending state, since a separate execution makes separate buffers (the same test's control) |
| history | From C-1 S1 (2026-10-24): (H_END+1 − W) × (per-block − header_max − E_max·24) offloaded canonical entries + W × per-block + min(W·BODY_MAX, TXBYTES_CAP) + min(W·E_max·EVID_BYTES_MAX, identities × (EVID_BYTES_MAX+24)) + min(W·txs per block, TX_CAP) × 380 B + min(W·LOG_BLOCK_MAX, LOG_CAP) × 33/32 + HISTORY_CACHE (16 MiB) + history.redb allocator (payload × 8 / 4096), with W = min(history_window or K_hist, H_END+1). Before S1: (H_END+1) × per-block + TXBYTES_CAP + TX_CAP × 380 B + LOG_CAP × 33/32 + identities × (EVID_BYTES_MAX+24) | H_END (apply_block); TX_CAP/TXBYTES_CAP chain-cumulative (block invalid); LOG_CAP (USER `Halt::LogCap`); evidence ≤ one per identity ever (a tombstoned record makes further evidence a host error), identities ≤ M_max + R_max · H_END |
| per block | 3 × (map slot of `Entry` 1184 B) + 2 × header_max + bloom/sig/proposer + 8·S + 32·(S+1) shares + 64·M ranges + an empty state | header_max = 768 + 9·S_max (network.md 3067 B for S=256) |
| side blocks (D-REORG-REDB) | (side_cap + 2·R_keep) × (per-block − header_max − E_max·24 + 48) offloaded metadata entries + one full block (per-block + BODY_MAX + min(BODY_MAX/85, GAS_LIMIT/21000) transactions with receipts + LOG_BLOCK_MAX) + EVICTED_MAX × 96 | `Chain::retain`, after every import and on replay. Every journaled non-canonical block is offloaded at that retain: its body (evidence included), receipts, tx hashes and encoded header are dropped and read back from the journal. Live: ≤ side_cap = R_keep = 2048 unprotected (the lowest-work unprotected leaf goes first, repeatedly, so the cap holds right after a reorg too) plus ≤ protect_max = 2·R_keep = 4096 blocks protected by a sync session (network.md:27). Replay: all ≤ side_cap + protect_max (`Chain::replaying`). None R_keep or more below the head, descendants removed with them. The one full block is the just-imported one before its retain, or the block `restore_canonical` is re-executing. Canonical blocks are never evicted; since C-1 S1 those below the resident window are offloaded too (history term). Derivation and proofs: proposals/D-REORG-REDB-DESIGN-FREEZE.md Revision 2 |
| queues | pool (32 MiB and ≤ POOL_MAX_TXS = 4096 transactions, plus entries) + orphans (16 MiB) + future (64) + templates (64) + seen caches (4096) + invalid (4096) + rejects (256) + receipt times (1024) + evidence pool (64) + per-peer queues for (peers_max = 8 inbound + one per dialer) + event queues (EVENT_QUEUE 128, NET_EVENT_QUEUE 32 per forwarder) | constants in `node.rs`, `chain.rs`, `net.rs` (A05) |
| rpc | conn_max × (4·(HEAD_MAX + READ_CHUNK) + REQ_BUF) requests + HEAVY_RESP_POOL (48 MiB) + (conn_max + 1) × LIGHT_RESP_MAX (512 KiB) + conn_max × SMALL_REPLY_PEAK (132,768 B) + HEAVY_QUEUE × (REQ_BUF + 256) + LIGHT_QUEUE × (LIGHT_BODY_MAX + 256) + parse tree (`parse_bytes_max(REQ_BUF)`) + answer being built (VEC × BATCH_BYTES_HARD + 4 × RESP_MAX). Round 3 code (`8039a09` and later); the row as it stood before RpcGuard is below the table | `rpc.rs` RpcGuard (RPCGUARD.md): connection cap (excess refused unread; finished threads joined before their slot is reused); REQ_BUF over request line, headers and body; response pools (the class maximum reserved at admission, shrunk to the answer, released when the Reply drops; a full heavy pool is −32021 busy); HEAVY_QUEUE places and LIGHT_QUEUE capacity, with QUEUE_WAIT bounding every wait before admission (−32021 busy); a queued body shared with its connection through an Arc, never copied (the Arc and the admission state fit the +256 B per body); busy and rate answers on connection threads keep ids as their encodings and take at most SMALL_REPLY_PEAK of heap (the kept id encodings, the answer, its HTTP header and one transient id parse, ID_PARSE_PEAK = 124,032 B; round 3, review RG2-m2: round 2 kept echoed ids as parse trees, up to 340,378 B per busy answer, evidence/rpcguard-r3/busy-path-peaks.log); the per-connection token bucket keeps no state; COMPUTE_DEADLINE from admission, plus a getLogs SCRATCH budget; the SendPacer writes straight from the reservation; JSON_VALUES_MAX before parsing (−32600); batch members one at a time, ≤ BATCH_RESP_MAX; a light answer over LIGHT_RESP_MAX is −32020 {resultBytes} (network.md:210), and `capped` (RESP_MAX → −32020 {resultBytes}); eth_getLogs stopping at RESP_MAX, LOGS_COUNT_MAX or LOGS_SCAN_BYTES |
| threads | (2 × peers_max + 4 × dials + conn_max + 6) × (IO_STACK 256 KiB + 4 KiB heap) + CONSENSUS_STACK 16 MiB + main thread 1 MiB + glibc stack cache 40 MiB | every thread is created by `pocold::threads::spawn` with an explicit reservation, so its resident stack cannot exceed it. Inventory (`resources::io_threads`): per inbound peer a reader and a writer, per configured peer a dialer, a forwarder, a reader and a writer, conn_max RPC connection threads, and 6 singletons (miner, two forwarders, timer, P2P listener, RPC acceptor). Exiting threads count too: the RPC acceptor, the P2P listener and each dialer join a finished thread before its place is reused (`threads::reap`), a reader joins its writer, and the old miner is joined before the next one starts (`miner::PEAK`, tested). The consensus thread runs every command; main only spawns and joins it |
| execution | `memory::execution(max(GAS_LIMIT, PER_CALL_GAS), B_code_max)` for one execution at a time (30M gas: 1,131.2 MiB) | derived from the gas schedule (§3a); eth_call/eth_estimateGas are capped at CALL_GAS_MAX = PER_CALL_GAS (compile-time check); every EVM execution runs on the consensus thread |

The rpc row as it stood from `687ac38` to `3c3792e`, before RpcGuard (kept for history; −32008 was replaced by −32020 {resultBytes} in RpcGuard round 3):

| term | formula | enforced by |
|---|---|---|
| rpc | conn_max × (2·(HEAD_MAX + READ_CHUNK) + REQ_BUF) requests + HEAVY_RESP_POOL (48 MiB) + (conn_max + 1) × LIGHT_RESP_MAX (512 KiB) + conn_max × SMALL_REPLY_MAX + queued_max (2·conn_max) × REQ_BUF + conn_max × LIGHT_BODY_MAX + parse tree (`parse_bytes_max`) + answer being built (3 × BATCH_BYTES_HARD + 4 × RESP_MAX) | `rpc.rs`: connection cap (excess refused unread; finished threads joined before their slot is reused), REQ_BUF over request line, headers and body together, a header buffer allocated once and a body of exactly Content-Length; response pools (`answer`: class maximum reserved before the request runs, shrunk to the answer, released when the reply drops; a full heavy pool is −32021 busy before anything runs; a light answer over LIGHT_RESP_MAX is −32008); queued heavy bodies capped (`Held`, 503 beyond); JSON_VALUES_MAX before parsing (−32600); batch members run and serialized one at a time with the delivered bytes ≤ BATCH_RESP_MAX; `capped` (RESP_MAX → −32008); eth_getLogs stopping at RESP_MAX, LOGS_COUNT_MAX or LOGS_SCAN_BYTES |

The rpc term at the round-3 code, itemized by `a13_bound_breakdown` (evidence/rpcguard-r3/a13-breakdown-8039a09.log):

| rpc item | B | MiB |
|---|---:|---:|
| connection requests (conn_max) | 36,700,160 | 35.0 |
| heavy response pool | 50,331,648 | 48.0 |
| light response pool ((conn_max + 1) × 512 KiB) | 17,301,504 | 16.5 |
| small answers outside the pools (conn_max × SMALL_REPLY_PEAK) | 4,248,576 | 4.05 (0.16 at round 2: conn_max × SMALL_REPLY_MAX) |
| queued heavy bodies (HEAVY_QUEUE = 16) | 16,781,312 | 16.0 (64.0 at 2·conn_max before RpcGuard round 2) |
| light lane (LIGHT_QUEUE = 32) | 139,264 | 0.13 |
| request parse tree | 9,175,040 | 8.75 |
| answer being built | 83,901,632 | 80.0 |
| **rpc total** | **218,579,136** | **208.5** (204.6 at round 2, `fa97594`) |

Notes on the §2 history term (caps track, round 3; no change to the model numbers):
- the logs term uses `caps::LOG_CAP`, which is 2^24 in the caps-test build, so the term is a smaller, still valid upper bound for that build;
- sizing the devnet TX caps to the CAPS-SUSTAINABILITY O2 minima (7.6·10^6 transactions, 3,931,840,000 B) makes the history term 6.98 GiB (with min(LOG_CAP, H_END·LOG_BLOCK_MAX); 7.38 GiB as coded; 10.48 GiB with N1's 2^32) and the whole model 11.12–14.62 GiB at `fa97594` (needing 12.71–16.70 GiB of budget). The 6 GiB gate refuses boot in every case unless canonical bodies and receipts move off the heap (proposals/CAPS-SUSTAINABILITY.md §5b, evidence/caps-r3/o2-memory-fa97594.json).

### 2a. D-A13-BUDGET: itemized bound, reductions and reserve (round 2)

At 45712cd the corrected model (§3a, rpc term) put the devnet default (default limits, two configured
peers) at 7,112,758,897 B (6.62 GiB), above the 6 GiB budget: `Node::open` refused to boot it. The owner
approved option 1 of D-A13-BUDGET: reduce local, non-consensus limits and keep the 6 GiB budget. That
approval does not cover consensus changes and waives no specification requirement.

Itemized model, before (45712cd) and after (`687ac38`, the head when this table was made: SIDE_BLOCKS_MAX 320, the exiting-threads term; later totals in §2b). `Bound::items` names every term; at 45712cd's values the
itemized arithmetic reproduces 45712cd's total exactly (7,071,790,705 B without configured peers, as its
failing boot test printed; two peers add 40,968,192 B).

| group | item | 45712cd (MiB) | after (MiB) |
|---|---|---:|---:|
| snapshots | head and K_hist retained states | 1,259.1 | 1,259.1 |
| snapshots | side_max side-branch states | 305.2 | 76.3 |
| snapshots | transient working copies | 381.5 | 381.5 |
| snapshots | table growth in a working copy | — | 38.2 |
| code | head and K_hist retained states | 135.3 | 135.3 |
| code | side_max side-branch states | 32.8 | 8.2 |
| code | transient working copies | 41.0 | 41.0 |
| history | blocks (H_END+1) | 495.8 | 495.8 |
| history | transaction bytes (TXBYTES_CAP) | 16.0 | 16.0 |
| history | transaction index and receipts (TX_CAP) | 7.2 | 7.2 |
| history | logs (LOG_CAP) | 1,056.0 | 1,056.0 |
| history | evidence | 0.5 | 0.5 |
| side | SIDE_BLOCKS_MAX stored side blocks | 285.2 | 132.3 (cap 256→320, round 2) |
| queues | transaction pool | 124.2 | 62.1 |
| queues | orphan blocks | 16.6 | 16.6 |
| queues | future blocks | 16.0 | 16.0 |
| queues | templates | 32.5 | 32.5 |
| queues | seen sets | 37.3 | 37.3 |
| queues | rejection and evidence caches | 2.0 | 2.0 |
| queues | peer send queues and frames | 85.1 | 25.0 |
| queues | main event queue | 64.1 | 32.0 |
| queues | network event forwarders | 48.0 | 24.8 |
| rpc | connection requests (conn_max) | 33.5 | 35.0 |
| rpc | answers held by connections | 512.2 (32 × 16 MiB) | 64.7 (pools) |
| rpc | queued heavy bodies (queued_max) | 64.0 | 64.0 |
| rpc | light lane (conn_max) | 0.1 | 0.1 |
| rpc | request parse tree | 8.8 | 8.8 |
| rpc | answer being built | 48.0 | 80.0 (corrected) |
| threads | I/O thread stacks and heap | 27.9 | 15.7 |
| threads | exiting not-yet-joined (conservative) | — | 10.5 |
| threads | consensus stack | 16.0 | 16.0 |
| threads | main stack and heap | 1.0 | 1.0 |
| threads | exited-stack cache | 40.0 | 40.0 |
| execution | frames, code analysis, preallocations, precompiles | 128.6 | 128.6 |
| execution | memory and operation classes (worst gas split) | 1,461.4 | 1,002.5 |
| **total** | | **6,783.3** (7,112,758,897 B) | **5,363.1** (5,623,469,297 B) |

Changes, each checked to be local before it was made: none changes block validity, state roots, gas
rules or accepted-transaction semantics, none goes below a normative minimum, and a maximum-size valid
block is processed as before.
- One change failed the reorganization check after it was made: SIDE_BLOCKS_MAX 64 made long competing
  branches unadoptable (second review). It was reverted to the 45712cd value, 256.
- The remaining gap between 256 and R_keep predates this decision and is recorded as §4 item 18.

| change | before → after | saved (MiB) | why it is local and within the specification | operational trade-off |
|---|---|---:|---|---|
| RPC response pools (mechanism) | 16 MiB per connection → 48 MiB heavy + (conn_max+1) × 512 KiB light | 447.5 | modelled on RpcGuard LIGHT_RESP_MAX, HEAVY_RESP_MEM, LIGHT_RESP_MEM (network.md (C)); RpcGuard is local (consensus.md:29); RPC_CONN_MAX stays 32 | a heavy request meeting a full pool is answered −32021 busy at once (no QUEUE_WAIT); a light answer over 512 KiB is −32008. Not the RpcGuard permit mechanism, so the RpcGuard fields stay absent in admission |
| execution classes (accounting correction) | 1,461.4 → 1,002.5 | 458.9 | proof (§3a): an empty storage map allocates nothing (`loaded_accounts_start_with_unallocated_storage_maps`), so a map's share belongs to the account's first slot | none |
| answer being built (correction) | 48.0 → 80.0 | −32.0 | the batch String grows (counted at 3 × its maximum) | none |
| undercounts found by the second review | — | −41.5 | a working copy's growing table (38.2), the lossy header copy (up to 3 B per input byte), one event held by each blocked forwarder | none |
| light answers | eth_getTransactionByHash carried `raw` beside `input` (about 4 × TX_MAX) | — | the standard fields only (raw only when the transaction does not decode), within network.md:201's 2·TX_MAX + 2 KiB, so a TX_MAX transaction stays answerable in the light lane | none |
| side_max | 8 → 2 | 253.5 | NodeLocal parameter (consensus.md:29: local, outside CP); R11/R20/R21 hold | a third competing side tip's state is pruned and rebuilt on demand from its nearest retained ancestor (CPU, §4 item 6) |
| SIDE_BLOCKS_MAX | 256 → 64 → **256** (reverted) | 179.3 at 256, by the gas bound on transactions per side block | The second review showed that 64 made a heavier branch more than about 64 blocks long unadoptable. network.md:144 requires adoption up to R_keep (2048), and a branch is held as side blocks until it is heavier. 256 is the value at 45712cd. The saving now comes from a proven tightening instead: a validated block holds at most GAS_LIMIT/21,000 = 380 transactions, not BODY_MAX/85 = 2,313 | adoption depth is SIDE_BLOCKS_MAX, reverted to 256 here and then raised to 320 in round 2 (below R_keep: §4 item 18); the cap holds after a reorg; evicted hashes are refused when received again |
| POOL_MAX_BYTES | 64 → 32 MiB | 62.1 | network.md:157 "pool ≤ 64 MiB" (an upper limit) | 256 transactions of TX_MAX (128 KiB) or 4096 small ones; beyond, −32016 |
| peers_max (inbound) | 32 → 8 | 72.3 (queues and threads) | network.md:156 "≤ 50 peers" (an upper limit) | at most 8 inbound peers; a devnet node has 2 |
| EVENT_QUEUE | 256 → 128 | 32.1 | network.md:156 "queue ≤ 256" (an upper limit) | senders block earlier (backpressure) |
| NET_EVENT_QUEUE | 64 → 32 per forwarder | 24.0 | local constant | peer readers block earlier behind a busy node |

Not changed, and why:
- K_hist stays 32 (governance.md:15). Lowering it would save about 42 MiB per block of depth. But a
  fork deeper than K_hist is rebuilt from genesis (up to H_END = 20,000 blocks on the devnet), and the
  RPC trie window narrows (consensus.md §10).
- RPC_CONN_MAX stays 32 (network.md:213).
- Terms set by consensus caps are unchanged: GAS_LIMIT and PER_CALL_GAS, LOG_CAP, H_END, TX_CAP,
  TXBYTES_CAP, ACCT_CAP, SLOT_CAP and SYS_SLOT_MAX.
- TRANSIENT_SNAPSHOTS stays 10: it is an inventory, and lowering it would need proof.

Reserve. The boot gate is now stricter: the model total must fit in 7/8 of the budget
(`RESERVE_KEEP_EIGHTHS`), so 12.5% (768 MiB of 6 GiB) is kept for what the model does not count.
- Devnet default: model total 5,623,469,297 B (5.237 GiB). Required budget ceil(8·total/7) = 6,426,822,054 B (≈ 15 MiB under the 6 GiB budget, over the 768 MiB reserve).
  That leaves 15,628,890 B (≈ 14.9 MiB) unused below the budget (reconciliation and workload RSS: §2b).
- What the reserve is for: allocator chunk headers and size-class rounding, which add up to about 31 B
  per allocation; fragmentation and per-thread arenas, which are not bounded; and the inspection-based
  inventories (§4 items 11, 13, 16).
- Evidence: the measured RSS in §2b. The reserve is a margin, not a proof: §4 item 3b stays OPEN.

The boot test asserts four things separately (`a13_boot_refuses_a_bound_above_the_memory_budget`):
- the approved default boots;
- a budget one byte below its requirement is refused;
- exactly the requirement boots;
- network-scale caps (ACCT_CAP 10^6, SLOT_CAP 4·10^6) are refused.

Second independent review (of 45712cd..b132a05). Its findings and what was done:
- **SIDE_BLOCKS_MAX 64 blocked adoption of branches longer than about 64 blocks (HIGH).** Reverted to
  256, and the gap to R_keep is recorded (§4 item 18).
- **Restart failed on duplicate journal records of re-fetched evicted blocks (HIGH).** Evicted hashes
  are refused on import. Replay retains after every record, as live import does, and skips duplicates
  and records of evicted blocks.
- **The busy path parsed over-limit bodies (MEDIUM).** It now applies JSON_VALUES_MAX first
  (`a_busy_answer_does_not_parse_an_over_limit_body`).
- **The side-block term did not hold right after a reorg (MEDIUM).** Trimming now repeats until the
  cap holds.
- **eth_getTransactionByHash for TX_MAX transactions answered −32008 (MEDIUM).** Fixed: `raw` is
  dropped from the answer.
- **Two tests depended on the host (MEDIUM).** The miner test now stops at height 2 and counts starts;
  the flood test no longer asserts a ratio, and a deterministic wake test was added.
- **Test gaps (LOW).** Added the end-to-end reservation test (`an_answer_is_reserved_until_its_connection_has_written_it`,
  whose mutation fails) and corrected the inventory test's bound for one not-yet-joined RPC thread.
- **Undercounts (LOW).** Added to the model.
- **The stats method reused the spec's name (LOW).** Renamed `pocold_rpcCounters`; pocol_getRpcStats
  stays unavailable.

### 2a-redb. D-REORG-REDB: the side term re-derived (owner-approved storage repair)

The 320 full side blocks (132.3 MiB) are replaced by offloaded metadata. Devnet profile: per-block term
26,067 B; an offloaded entry is 26,067 − 3,072 (encoded header) − 96 (evidence) + 48 (protect-set slot)
= **22,947 B** (`resources::derive`, `side_entry`). Terms printed by `a13_bound_breakdown` on the final
commit (`evidence/a13-breakdown-redb.json`, 9ce5a7c):
- side/offloaded side-block metadata (side_cap + 2·r_keep = 6,144 entries): **140,986,368 B**;
- side/one full block before offload or during restore: **433,635 B**;
- 2026-10-28 (after 719da04): each entry also holds a slot in the chain's side set (`Chain::side`, which
  replaced the per-retain walk over every entry), 48 B more: 22,995 B per entry, +294,912 B for the 6,144
  entries. The devnet total pinned by `a13_bound_breakdown` is 3,233,892,640 B; network store mode
  5,581,616,397 B (5.198 GiB).
- 2026-10-28 (independent review F-C, evidence/r4/review-2/REVIEW.md): the side set is its own term
  instead, `side/the chain's side set`, (side entries + r_keep) × 76 B: a hash table of 32-byte keys can
  take up to about 76 B an element after it grows and does not shrink, and a reorg moves up to r_keep
  hashes in before retain trims. Devnet total 3,234,220,320 B; network store mode 5,581,944,077 B
  (5.199 GiB), 6,379,364,660 B required against 6,442,450,944 B. The protect-set slot and the evicted FIFO
  keep the 48 B convention: not re-derived, open.
- side/evicted-hash FIFO (EVICTED_MAX): **98,304 B**;
- side total **141,518,307 B = 134.96 MiB** (was 320 × 433,563 = 138,740,160 B = 132.31 MiB at 687ac38).
Model total **5,627,687,516 B** (was 5,623,469,297 B): +4,218,219 B = +2,778,147 (side) + 1,440,072, the
`history/blocks` term growing by 72 B per block (20,001 blocks). `Entry` gained `body_off` and
`offloaded`, 24 B per map slot counted three times. Canonical blocks stay whole, so the canonical
history terms keep their meaning and only the slot size changed: storage.md:70 retains them "until the
end", so no RPC scope changed.
New condition recorded: `open-replay-transient`, the whole journal held by `store::open` plus two
replay-index entries per record while replaying. This was pre-existing and is not in the steady-state bound.

### 2b. Model total, measured RSS and guaranteed ceiling

Reported on five separate axes, so none is read as the others.

Model totals, each dated to the code it was computed on (the heading "Final code (D-REORG-REDB,
`9ce5a7c` and later)" used here until round 3 was stale: RpcGuard changed the rpc term after it):

| code | model total | gate requirement ceil(8·total/7) | reserve | unused | source |
|---|---:|---:|---:|---:|---|
| `45712cd` (before D-A13-BUDGET) | 7,112,758,897 B (6.62 GiB) | — (refused) | — | — | §2a |
| `687ac38` (round 2 final code) | 5,623,469,297 B (5.237 GiB) | 6,426,822,054 B | 803,352,757 B | 15,628,890 B | axes 1–3 below |
| **D-REORG-REDB final code** (`9ce5a7c`; unchanged at `3c3792e` and at the caps round-2 head `c3a78e7`) | **5,627,687,516 B (5.241 GiB)** | **6,431,642,876 B** | **803,955,360 B = 766.7 MiB** | **10,808,068 B = 10.3 MiB** | evidence/a13-breakdown-redb.json |
| **integrated head after RpcGuard round 2** (merged at `ba7d294`; measured at `273e785` by the second caps review, at `fa97594` in caps round 3, and at `938cb94` by the third review) | **5,577,343,580 B (5.194 GiB)** | **6,374,106,949 B** | **796,763,369 B = 759.9 MiB** | **68,343,995 B = 65.2 MiB** | evidence/caps-r3/a13-fa97594.log; RPCGUARD.md §8 |
| **round-3 code** (RpcGuard round 3, `8039a09`; merged at `ba67038`; unchanged by round 3c, `dd7d1ec`, and at the head `c959379`) | **5,581,426,268 B (5.198 GiB)** | **6,378,772,878 B** | **797,346,610 B = 760.4 MiB** | **63,678,066 B = 60.7 MiB** | evidence/rpcguard-r3/a13-breakdown-8039a09.log; `a13_bound_breakdown` asserts the total on Linux x86_64 and passes on `2b1a571` (evidence/integration-r3/pocold-2b1a571.log) |

| **C-1 S2** (shared code buffers, 2026-10-24, on the tree after `af2f3a8`) | **5,487,223,178 B (5.110 GiB)** | **6,271,112,204 B** | **783,889,026 B = 747.6 MiB** | **171,338,740 B = 163.4 MiB** | evidence/decision-package/a13-devnet-breakdown-s2.log; the code group drops from 191,130,849 B (182.3 MiB) to 96,927,759 B (92.4 MiB) on the devnet, which is the whole change of the total (−94,203,090 B), and from 54.69 GiB to 9.69 GiB at the network profile (evidence/decision-package/a13-network-profile-s2.log: total 105.83 GiB, gate 120.95 GiB, still far over 6 GiB) |
| **C-1 S1** (canonical history off the heap below the K_hist window, 2026-10-24) | **4,323,240,949 B (4.026 GiB)** | **4,940,846,799 B** | **617,605,850 B = 589.0 MiB** | **1,501,604,145 B = 1432.0 MiB** | evidence/c1-s1/a13-devnet-breakdown-s1.log; the history group is now the window (K_hist full blocks, capped by the chain caps), the offloaded per-block metadata, and history.redb's cache (16 MiB) and allocator. A window of the whole chain reproduces the S2 total except for the logs term, now also bounded per block by LOG_BLOCK_MAX (`a13_bound_breakdown`). Network profile: 105.83 → 90.93 GiB, gate 103.92 GiB (evidence/c1-s1/a13-network-profile-s1.log) |
| **C-1 S3a** (retained canonical states as diffs, 2026-10-24) | **3,601,500,638 B (3.354 GiB)** | **4,116,000,730 B** | **514,500,092 B = 490.7 MiB** | **2,326,450,214 B = 2218.7 MiB** | evidence/c1-s3a/a13-devnet-breakdown-s3a.log. The snapshots are the head, genesis (counted as a full one), side_max, 2 materialized states, the 10 transient copies and the growth copy, against 1 + K_hist + side_max + 10 + growth before. Plus (2·K_hist + side_max + 1) × `diff_max` (resources.rs: the gas schedule bounds a block's changed slots and accounts). Network profile: 90.93 → 41.41 GiB, gate 47.33 GiB (evidence/c1-s3a/a13-network-profile-s3a.log) |
| **C-1 S4** (old canonical entries slimmed, 2026-10-24) | **3,322,680,105 B (3.094 GiB)** | **3,797,348,692 B** | **474,668,587 B = 452.7 MiB** | **2,645,102,252 B = 2522.6 MiB** | evidence/c1-s4/a13-devnet-breakdown-s4.log. The canonical entries deeper than slim_depth (2·R_keep + E_win + 257) are counted at 3 × the inline entry plus an empty state; the others keep the per-block term. history.redb's payload adds each block's nonce ranges. Network profile: 41.41 → 37.96 GiB, gate 43.38 GiB (evidence/c1-s4/a13-network-profile-s4.log) |
| **C-1 E1** (incremental state root, 2026-10-24) | **3,486,909,657 B (3.247 GiB)** | **3,985,039,608 B** | **498,129,951 B = 475.1 MiB** | **2,457,411,336 B = 2343.6 MiB** | evidence/c1-e1/a13-devnet-breakdown-e1.log. A new trie group: the head state's trie, one parent trie derived or built (at most one at a time on the consensus thread; its distinct nodes counted as a whole trie) and the changed paths of the block executed on it (`trie_paths`: 65 nodes per changed slot and per changed account, from `block_writes`). A trie of n keys is counted at 2n nodes of `state_trie::node_bytes` (168 B world, 88 B storage, Arc counts included) plus 2n child pointers; measured on the near-cap devnet state at 162 B per key, branches included (evidence/c1-e1/validation-near-cap-e1.log). The entry grows by its trie handle. Network profile: 37.96 → 40.20 GiB, gate 45.94 GiB; the trie group is 2.23 GiB (evidence/c1-e1/a13-network-profile-e1.log) |
| **C-1 E2** (whole-state checks from a state summary, 2026-10-24) | **3,489,051,177 B (3.249 GiB)** | **3,987,487,060 B** | **498,435,883 B = 475.3 MiB** | **2,454,963,884 B = 2341.2 MiB** | evidence/c1-e2/a13-devnet-breakdown-e2.log. Each entry that is not slimmed holds its state's `state_summary::Summary`, boxed (the per-block term adds its size). Network profile: 40.20 GiB, gate 45.94 GiB (evidence/c1-e2/a13-network-profile-e2.log) |
| **C-1 E3** (executor, pool and RPC on overlays, 2026-10-25) | **3,186,963,844 B (2.968 GiB)** | **3,642,244,394 B** | **455,280,550 B = 434.2 MiB** | **2,800,206,550 B = 2670.5 MiB** | evidence/c1-e3/a13-devnet-breakdown-e3.log. TRANSIENT_SNAPSHOTS 10 → 2 (a block's merged child; a rolling re-execution's state and next merged state), POOL_LAYER_SNAPSHOTS 1 (the pool's layer counted as a whole state until D-7's value bounds it), and the applied block's overlay (`overlay_max`, in the diffs group). `Entry.state` is an `Arc`, so the entry is smaller. Network profile: 28.08 GiB, gate 32.09 GiB (evidence/c1-e3/a13-network-profile-e3.log) |

Each row reconciles exactly: model + reserve + unused = 6,442,450,944 B. Stage A of RpcGuard printed an
intermediate 5,577,577,052 B, which also counted a per-source token-bucket map (233,472 B) that round 2
removed. Round 3 added conn_max × (SMALL_REPLY_PEAK − SMALL_REPLY_MAX) = +4,082,688 B (RPCGUARD.md §8);
the rpc term is 218,579,136 B (208.5 MiB). Measured RSS was last taken on `3c3792e` (row "redb-final"
in axis 4); it was not re-measured on the round-3 code. The paragraphs below give the same five axes
as recorded at 687ac38, kept for history.

**1. Calculated model terms** (`pocold::resources::derive`, every cap at its maximum at once, under §4's
conditions). Devnet default: **5,623,469,297 B = 5.237 GiB**. The itemized terms are §2a's "after" column;
`Bound::items` is the machine-readable form, printed by `a13_bound_breakdown`.

**2. Explicit reserve** (the 1/8 boot gate, `RESERVE_KEEP_EIGHTHS = 7`). The gate admits a boot only when
`budget_required(total) = ceil(8·total/7) ≤ MEM_BUDGET`. For the devnet total that is
ceil(8 × 5,623,469,297 / 7) = **6,426,822,054 B**, so the reserve it holds back is
6,426,822,054 − 5,623,469,297 = **803,352,757 B = 766.1 MiB** for what the model does not count.

**3. Remaining budget** (unused headroom below the 6 GiB cap after the gate):
6,442,450,944 − 6,426,822,054 = **15,628,890 B = 14.9 MiB**.

These three reconcile exactly to the budget:
5,623,469,297 (model) + 803,352,757 (reserve) + 15,628,890 (unused) = 6,442,450,944 B = 6.000 GiB.
The owner's review quoted the pre-round-2 figures "5.20 GiB … 50 MiB". Raising SIDE_BLOCKS_MAX 256 → 320
added 56.0 MiB of side-block state and the exiting-threads term added 10.5 MiB, so the model rose to
5.237 GiB and the unused headroom fell from ≈ 50 MiB to 14.9 MiB. The 766 MiB reserve is unchanged.

**4. Measured peak RSS under each workload.** `tools/measure_rss.py`: a 3-node release devnet, 90 s idle
production, then 90 s of ordinary and bounded-adversarial RPC load. This is what a real run touched; it
reaches none of the caps, so it is not evidence about the worst case and is not the bound.

| run (commit) | workload | peak RSS / node (VmHWM) | threads / node | model total | RSS ÷ model |
|---|---|---:|---:|---:|---:|
| redb-final (`3c3792e`, D-REORG-REDB final code; `evidence/rss-redb-final.json`) | 90 s idle, then 71,466 light + 3,853 30M-gas eth_calls + 352 heavy batches + 193 transfers | 11.98–12.30 MB (idle 10.1–10.3 MB) | 26–29 | 5,627,687,516 B | 0.0021–0.0022 |
| final (`d610492`, code tree-equal to `bb9d8b8`; `evidence/rss-final.json`) | 90 s idle, then 48,065 light + 2,287 30M-gas eth_calls + 242 heavy batches + 187 transfers; height 154, 4 peers each | 12.26–12.32 MB (idle 10.5–10.6 MB) | 26–29 | 5,623,469,297 B | 0.0022 |
| b132a05 (`evidence/rss-b132a05.json`, before the second review's changes) | 39,815 light + 1,960 eth_calls + 194 batches + 187 transfers; height 157, 4 peers | 11.9–12.0 MB | — | 5,506,111,035 B | 0.0022 |
| 10 full-cap state clones (`lp4-execution/tests/rss_snapshot.rs`) | 20,005 accounts, 115,536 slots | +181,104,640 B vs 10 × 40,006,582 B modelled | — | — | 0.45 |

The devnet run sits at 0.22% of the model: a 3-node, 154-block run fills none of the caps (no max side
branch, no full canonical history to H_END, no full response pools). The state-clone run is the only one
that loads the largest single term (snapshots, 1.72 GiB of the model) to its cap, and there the model's
own slack covers the allocator's headers and page use at 0.45 of the modelled bytes. One passing RSS run
is not a worst-case proof.

**5. Unmodelled overhead and unresolved assumptions.** No guaranteed process-memory ceiling exists. The
model leaves open, and the 766 MiB reserve is the margin (not a proof) for:
- allocator per-allocation headers and size-class rounding (glibc ≤ 31 B/allocation; estimated below);
- fragmentation and per-thread arenas (not bounded by anything);
- the inspection-based inventories (§4 items 11, 13, 16);
- the five conditions `derive` records on the live `a13` status (allocator overhead, transient-snapshot
  inventory, main-thread stack, revm representation, CPU/timing), echoed in `evidence/rss-final.json`.

Allocation-header estimate for the large terms (glibc, ≤ 31 B/allocation):
- history: ≈ 10 allocations/block × 20,001 blocks, plus one per receipt (≤ TX_CAP): under 7 MB;
- logs: ≈ 1 per receipt, inside encoded receipts: under 1 MB;
- execution and RPC terms: a few large buffers.

An operating-system limit (cgroup `memory.max`, `RLIMIT_AS`) would turn an overrun into a failure rather
than make the bound hold; none is configured. §4 item 3b stays OPEN.

R21 fact (admission): (K_hist + side_max) × snapshot = 34 × 38.2 MiB = 1.27 GiB ≤ HIST_TRIE_BUDGET, so the devnet is
admitted without the simnet retentionStress exception; at network-scale caps the bound exceeds the budget
and only that exception admits a simnet node (`resource_rules_r14_to_r21`).

### 2c. C-1 E4/E5: the store-mode bound (2026-10-25, `derive` with `Limits::state_store`)

In store mode every retained state is a version pinned in state.redb and read in place. No whole state,
diff or E1 trie is held; the checkpoint encoder refuses an entry that holds one. `derive` itemizes what is
held instead. Each term names the code it rests on (src/resources.rs). Network-profile values are from
evidence/c1-e4/a13-final.log (commit fc24969).

| item | term | why it is bounded | network |
|---|---|---|---:|
| snapshots/the pool's layer | 1 whole state | the pool's gas is not bounded until the D-7 value is approved | 1.63 GiB |
| diffs/the block's overlay | `overlay_max` | as in whole-state mode (C-1 E3) | 0.004 GiB |
| store/a block's batch | `BATCHES` (1) × `batch_max` | one batch at a time: `child_state` and `rolled` commit it before the next block (inspection) | 1.02 GiB |
| store/node cache | `NODE_CACHE` (256 MiB) | resident bytes: `CACHE_ENTRY` + `Node::resident_bytes` per node, enforced by `Cache::put` | 0.25 GiB |
| store/redb page cache | `STATE_PAGE_CACHE` (64 MiB) | set on the builder | 0.06 GiB |
| store/redb allocator | `redb_allocator_bytes(state_file_max)` | redb 2.6.4 keeps, for every 4 GiB region of the file, a buddy allocator sized for the whole region; `redb_region_allocator` mirrors buddy_allocator.rs and bitmap.rs | 1.81 GiB |
| store/stored-version handles | `stored_versions` × `pinned_bytes` | every version `held_runs` lists (2·K_hist + side_max + 4): genesis, the K_hist window with the block being imported, side_max, a rolling rebuild with the version it commits next, and an abandoned window until `settle`. Review 2026-10-27: this count once listed 2·K_hist + side_max + 2, apart from the versions' bytes; both now read `held_runs` (`a13_counts_a_handle_for_every_held_version`) | 0.005 GiB |
| store/a checkpoint being written | `checkpoint_bytes` | its bytes (`REC_MAX` per entry), the entry index, and redb's copy | 0.08 GiB |
| code/read by the execution | one code set | each loaded account's code is read from the store; bounded by CODE_CAP | 1.13 GiB |
| code/the pool's state | one code set | the pool's own creates | 1.13 GiB |
| history/entries without a state | one empty state (C-1 E5) | every such entry shares `chain::empty_state` | 0 |

**The largest state.redb file** (`state_file_max`):
- the head's and genesis's versions whole;
- every other version an entry or a checkpoint pins. Each of those differs from the head by the paths of
  the blocks between them, and by at most a whole state;
- the code every block to H_END can deploy (code is kept for good);
- the checkpoints' snapshots.

Key preimages are no longer a separate term: since E5 a leaf keeps its key's preimage, and the preimage is
released with the leaf.

Everything is counted at STATE_FILE_FACTOR (12). At the network profile every version saturates, about 108
whole-state versions. This is the adversarial worst case on disk: every block makes maximal writes along
65-node paths.

**The file factor** is the file size over the bytes stored (journal_measure::jm_store_mode_disk_impact).
It was measured after live import, after an open resumed from a checkpoint and after an open replayed
from genesis, and after each close (evidence/c1-e4/disk-phases-*.log, disk-impact-final.md):
- at most 6.07 with 14.9 MB stored;
- at most 11.31 with 1.5 to 2.8 MB stored. redb doubles a file while it is below one region, so a small
  file carries more slack; above one region the file grows a region at a time;
- redb's allocated pages are at most 2.38 times the stored bytes.

The first measurement gave about 18. That came from closing: a closing node released every version in a
non-durable transaction of its own, and redb reuses those pages only after a durable commit. Since E5
part 3 a close leaves the versions to the next open, which releases them in one durable transaction.

Below one region the allocator term is one region's allocator, whatever the factor. The factor is not
established for terabyte files (condition `store-file-factor`). Each unit of the factor is about 0.15 GiB
of the network bound.

**Result** (tests/network_budget.rs; the computed bound, not measured RAM):

| profile | whole-state mode | store mode |
|---|---:|---:|
| network (governance.md genesis parameters) | 27.92 GiB (gate 31.91) | **9.67 GiB (gate 11.05)** |
| devnet as pocold boots it | 2.91 GiB (gate 3.33) | 3.36 GiB (gate 3.83) |

How the network store bound moved:
- 10.58 GiB at E4.5 part 1;
- 10.24 GiB after E5 part 1 (no preimage term);
- 9.07 GiB after E5 part 2 (redb's allocator counted as it is built);
- 9.67 GiB after E5 part 3 (the measured factor of 12 in place of history.redb's 8).

At the devnet profile store mode is the larger bound. Its fixed caches and its worst-case batch and file
terms outweigh the small states they replace.

**Measured RSS** is in evidence/c1-e4/disk-impact-final.md. It covers small states and does not test the
bound.

What the store-mode bound still holds at the network profile, and what would lower it, is in
DECISION-PACKAGE.md ("Measured after E4 and E5").

### 2d. D-7 and the audit of what an execution loads and keeps (2026-10-25)

§2c is kept as recorded. This section adds the pool's gas bound (decision D-7, approved 2026-10-25) and
re-derives the terms that depend on it. Before any term was lowered, the code paths of revm 19.7 that load
accounts, slots and code were audited, together with how long the loaded data stays. The audit changed
three terms: one up and two down. It also withdraws one projection.

**D-7, as implemented** (src/node.rs; tests in tests/consensus.rs):
- **The bound.** The pooled transactions' declared gas limits sum to at most G. G is
  `POOL_GAS_BLOCKS` × GAS_LIMIT (4 × 8,000,000 = 32,000,000 at both profiles).
  - It is a local setting: `Limits::pool_gas_max`, or `pocold run --pool-gas-max GAS`.
  - A value below GAS_LIMIT is refused at open, because a transaction a block can hold must fit an empty
    pool.
  - A13 counts the pool at G, so a G the budget cannot hold is refused at open too ("within the resource
    limits").
  - It is pool policy only. No block-validity or consensus rule changes.
- **Admission** (`admit_tx`), in this order:
  1. size: -32014;
  2. decoding and chain: as before;
  3. a duplicate is answered with its hash and counts nothing;
  4. the byte and count bounds: -32016 pool full;
  5. the gas bound: -32016 pool full (gas). This is checked before execution, so a refused transaction
     costs no execution and changes nothing;
  6. execution on the pending state;
  7. the cap meter.
- **Replacement.** There is none. A second transaction with a pooled one's sender and nonce, whatever its
  fee, fails its execution on the pending state (-32000, NonceTooLow) and changes no counter. The pooled one
  leaves only when a new head makes it invalid.
- **Removal.** Only at a new head (`rebuild_pool`). Every pooled transaction is re-executed on the new head.
  One the head included, or one that is no longer valid, is dropped, and the counters are recounted from
  what stays. There is no expiry.
- **Re-insertion after a reorganization** (consensus.md: excluded transactions return to the pool once
  checked again). The order is:
  1. the abandoned blocks' transactions, oldest block first, each block in its own order;
  2. then the previous pool, in its order;
  3. then its backlog.

  Each is admitted within every pool bound. One that no longer fits the gas bound is dropped and is not
  pooled later; a later, smaller one is still admitted.

  **Relay of a returned transaction** (a local policy). A transaction that leaves an abandoned block and is
  admitted again is offered to every peer (`Relay::Offer`, through `Peer::offer`):
  - a peer with half its outbound queue in use does not get it, and the peer is kept;
  - one reorganization can return up to a whole pool at once, so plain gossip, which drops a peer whose queue
    is full, is not used;
  - the previous pool's transactions are not sent again, since they were relayed when first admitted;
  - `pocol_status` reports `stats.reinsertedOffered` and `stats.reinsertedUnoffered`.

  Why: e2e_integrated failed 2 of 4 runs on 919a50c, and failed again in a diagnostic run, at "pools
  settled after the heal". Node c held 7 returned transactions (8,356,420 gas) and no other node had them.
  - A node that is not drawn never builds a block, and c was not drawn at any of the 215–224 heights after
    the heal, nor did it win any of them.
  - The cause is the active set (consensus.md §2: A_h = {lastShare ≥ h−W_act or graceStart ≥ h−W_act}, with
    W_act = 32). At the heal the majority branch was 31–61 heights past the fork point, and c was no longer in A_h. It
    has no assignment and no longer mines (its production reads "mining=false"). It returns only through
    `reactivate`.
  - In the passing run that was checked, the branch was 12 heights past the fork point, and c was drawn 17
    times after the heal.
  - So the membership behaviour is the specified one. The defect was that the returned transactions reached
    nobody else.

  Still open: a returned transaction that a peer refused, or that no peer had room for, stays here until
  this node is drawn. It is not offered again. The peer queue bound is unchanged (the A13 peer term counts
  `queue_bytes_max` per peer).
- **Counters.** `Node::pool_audit` recomputes them from the pool's contents: gas and bytes summed over the
  admitted transactions, backlog bytes over the backlog, each transaction pooled once, and every bound.
  - It held after each of 160 mixed steps: submissions inside and past the bound, duplicates, same-nonce and
    wrong-nonce transactions, relays during a backlog, one-item slices, new heads and reorganizations.
  - Count of each kind of step: 31 admitted, 41 refused for gas, 2 relays in a backlog, 12 heads, 13
    reorganizations.
- **RPC.** eth_sendRawTransaction past the bound answers -32016 with the message "-32016 pool full (gas)".
  pocol_status reports `poolGas` and `poolGasMax`.
- **Revalidation does not hold up consensus.** The run loop takes one event between two slices of
  revalidation. The test runs `Node::run` with the pool at its bound: four BLAKE2F transactions of 7.9M gas,
  which no stop hook can cut short.
  - Two blocks arrive back to back. The second is imported while the first head's revalidation is still
    pending, and an RPC request is answered between slices.
  - In release on 9bd9bc0 (evidence/d7-bound), one such transaction took 189 ms. The second block and the
    answer took 736 ms, against about four transactions plus two imports.
  - The hold measurement at the bound (`d7_pool_revalidation_hold_under_a_heavy_pool`, four such
    transactions): sliced, the longest hold was 193 ms (one transaction); revalidating at once, the import
    held the thread 627 ms.
  - The gas bound alone bounds no time. What bounds the hold is the slice (POOL_SLICE, 50 ms) plus one
    transaction, and that transaction's time is measured, not derived. Gas is not time:
    - BLAKE2F is priced per round;
    - in store mode each cold account is a state.redb read.

**The audit of revm 19.7.** Sources: revm-19.7.0 journaled_state.rs and context/, revm-interpreter-15.2.0
instructions/host.rs and interpreter/contract.rs, and this repository's state_view.rs, view.rs, node_trie.rs
and state_store.rs.

| path | gas paid before the load | code loaded | how long it stays |
|---|---|---|---|
| sender (validation, fee) | none (intrinsic) | store mode: its code is read with the account (`TrieState::info`); a sender with code is then rejected (EIP-3607) | until the transaction ends |
| recipient of the transaction (`make_call_frame`, `load_code`) | none (intrinsic) | yes | until the transaction ends; kept if touched (a call touches it) |
| coinbase (fee) | none (warm, EIP-3651) | store mode: with the account | as above; touched by the fee |
| access-list addresses (`initial_account_load`) | 2,400 each, intrinsic | store mode: with the account | until the transaction ends (untouched) |
| BALANCE, EXTCODESIZE, EXTCODEHASH, EXTCODECOPY, CALL family, SELFDESTRUCT's beneficiary | **none before the load**: the account (and, through `load_code`, its code) is loaded, then the cold or warm cost is charged. A frame that cannot pay keeps the load and fails; a frame costs at least a warm CALL (100) | EXTCODE\* and calls: yes; BALANCE and SELFDESTRUCT: store mode only, with the account | until the transaction ends (a revert only marks it cold) |
| SLOAD | none before the load (the slot is inserted, then 2,100 or 100 is charged) | — | until the transaction ends; an account created in the same transaction keeps every slot it loaded in its committed entry |
| a frame's code | — | revm analyses raw code per frame (`Contract::new` → `to_analysed`): a padded copy and a jump table | until the frame returns (execution term, `fixed_heap`) |
| created code | 200 per byte (deposit) | analysed once | in the state, the layer or the overlay |

Findings:
1. **Store mode reads code with every account.** `TrieState::info` reads the code with the account, and
   `StateStore::code` returns a fresh copy. So in store mode every account load brings its code, whether or
   not the execution runs or reads it. In whole-state mode a load clones the state's `Bytecode`, which
   shares its buffers.
2. **Gas does not bound those copies at the network profile.** An account load costs as little as 100 gas,
   through a frame that runs out of gas after the load.
   - One 30M-gas execution could load 300,000 accounts × B_code_max (32,768) bytes = 9.2 GiB.
   - CODE_CAP (1 GiB) is smaller. The copies are of distinct existing accounts, so they are at most the
     logical code of every existing account.
   - So the store-mode term "code read by the execution in progress" stays one CODE_CAP set: 1.125 GiB at
     the network profile. That set also covers the copies a block's or template's overlay keeps for its
     touched accounts, since one execution and one overlay run at a time.
   - **The projection "a gas-derived execution code bound, about −1.0 GiB" (DECISION-PACKAGE.md, "Measured
     after E4 and E5") is withdrawn.**
   - Loading code lazily (code_hash only, code read on demand) would not change the worst case. EXTCODESIZE
     and EXTCODEHASH load the whole code (`load_code`), unpaid in the same way.
3. **The pool's layer keeps only touched accounts.** `Overlay::commit` takes only touched accounts, and a
   touch is paid:
   - a call or a transfer to a cold account (2,600);
   - a call or transfer to a warm account not loaded before: an access-list entry (2,400 paid), the
     precompiles, the coinbase;
   - a creation (32,000);
   - a self-destruct's beneficiary.

   A touch in a reverted frame is undone (AccountTouched). So with G gas the layer has at most
   3·⌊G/21,000⌋ + ⌊G/2,400⌋ + 32 accounts (`layer_writes`): 17,934 at 32M.
   - In store mode each keeps its code copy, so the pool's code is at most
     min(CODE_CAP, 17,934 × B_code_max) plus its creations (G/200 bytes).
   - In whole-state mode the touched accounts share the head's buffers, and only the creations are new.
4. **A created account keeps every slot it loaded.** That includes zero-valued slots and slots loaded in
   frames that ran out of gas. This was not counted before.
   - **Confirmed by a witness** run on a local node only and kept outside the repository, like the other
     adversarial programs; its log is in evidence/d7-bound. One transaction of 1,000,000 gas left 2,000 slot
     entries in the pool's layer, where counting one slot per 2,900 gas of writes allows 344.
   - So a layer's slot entries are at most ⌊g/2,900⌋ + ⌊g/100⌋ (SLOT_LOAD_GAS_MIN).
   - **Consequence for the block overlay.** `overlay_max` counted changed slots only (one per 2,900 gas) and
     each element at twice its inline size. It is re-derived with `layer_bytes`, every element at
     `memory::entry_bytes`: 0.004 → 0.041 GiB.
   - **Consequence for the pool's layer.** "One whole state" did not bound the layer either: zero-valued
     slots are not counted by SLOT_CAP. Before D-7 the pool's gas was bounded only by POOL_MAX_TXS ×
     GAS_LIMIT, so that term was an undercount. With D-7 the layer is bounded by G.

**The terms after D-7 and the audit** (src/resources.rs: `layer_writes`, `layer_bytes`, `pool_gas`, `overlay_max`;
test `d7_a13_pool_terms_follow_the_gas_bound`):

| item | before (§2c, fc24969) | now | network | devnet |
|---|---|---|---:|---:|
| the pool's layer | one whole state (snapshots) | `layer_bytes(layer_writes(G))` (diffs) | 1.633 → 0.127 GiB | 0.037 → 0.127 GiB |
| the pool's code, store mode | one CODE_CAP set | touched accounts' code (≤ CODE_CAP) + creations | 1.125 → 0.618 GiB | 0.0022 → 0.0041 GiB |
| the pool's code, whole-state mode | one CODE_CAP set | its creations (G/200 bytes) | 1.125 → 0.0003 GiB | 0.0022 → 0.0003 GiB |
| containers of the pool's layer | one snapshot's | inside the two terms above | 0.090 → 0 GiB | |
| the block overlay | changed slots, ×2 | `layer_bytes` of 5/4 GAS_LIMIT + SYSTEM | 0.004 → 0.041 GiB | 0.004 → 0.041 GiB |
| an abandoned body in revalidation | not counted | BODY_MAX + its transactions' headers | +0.0004 GiB | +0.0004 GiB |
| code read by the execution, store mode | one CODE_CAP set | unchanged (finding 2) | 1.125 GiB | 0.0022 GiB |

A bound past POOL_MAX_TXS × GAS_LIMIT counts as that, since the count bound binds first. CODE_CAP binds
the store-mode pool code at the devnet profile, not at the network one. Condition `revm-loading` records
what rests on source inspection.

**Result** (tests/network_budget.rs; the computed bound, not measured RAM):

| profile | whole-state mode | store mode |
|---|---:|---:|
| network | 27.92 → **25.24 GiB** (gate 28.85) | 9.67 → **7.61 GiB** (gate 8.69) |
| devnet | 2.91 → 3.04 GiB (gate 3.47) | 3.36 → 3.48 GiB (gate 3.98) |

At the devnet profile both bounds rose. Their states are small, so the D-7 layer (0.127 GiB) is larger than
the whole state it replaces (0.037 GiB), and the overlay is re-derived. That whole state was not a bound on
the layer (finding 4).

**The network store-mode bound is 7.61 GiB. The model cap is 5.25 GiB (6 GiB with its reserve), so it is
2.36 GiB over.** Its largest terms:

| item | GiB |
|---|---:|
| state.redb allocator | 1.81 (a 14.3 TiB adversarial file, 3,653 regions) |
| execution's code set | 1.125 |
| EVM execution | 1.10 |
| a block's batch | 1.02 |
| history | 0.63 |
| the pool's code | 0.62 |
| node cache | 0.25 |

The options, with their numbers and effects, are in DECISION-PACKAGE.md ("After D-7 and the code-loading
audit"). No bound was lowered to reach the budget.

### 2e. The network store-mode bound recounted: four provable reductions (2026-10-27, `892dca6`)

The 7.61 GiB bound of §2d was itemized term by term. A term was cut only where the code was changed so that
it holds less, or where a tighter count was proven and tested against a real store. No consensus rule,
validity rule, local limit or acceptance criterion changed. No term was lowered without that proof.

**1. The pool's layer keeps no code copies** (lp4 `view::Layer::drop_code_copies`, `Layer::info`;
pocold `node::admit_tx`).
- In store mode an account is read with a copy of its code (`TrieState::info`). Each entry of the pool's
  pending layer kept that copy for as long as the layer lived. The term was min(touched accounts ×
  B_code_max, CODE_CAP) plus the creations.
- After each admission the touched entries now keep the code hash only. Reads give the code back, from
  the layer's own code or from the base:
  - `account`, `each_account` and `merge` do so;
  - `contract_of` puts every code a commit or `insert_info` writes in the layer unless the base has it;
  - a base never loses code (a store keeps code for good; a whole state is not changed under the layer).
- So the layer holds its creations only, as in whole-state mode. The copies a transaction reads while it
  is admitted belong to the execution's set, which is unchanged.
- Tests:
  - `view::tests::a_layer_without_its_code_copies_reads_and_merges_as_before`: 2,000 random layers;
    reads, code checks, `each_account` and `merge` are identical with and without the copies.
  - `consensus::d7_the_store_mode_pool_layer_keeps_no_code_copies`: on a store-mode node, after pool
    transactions call a contract of the head and create one, no entry holds code. "pending" still answers
    both codes, and later pool transactions run both.
- 0.618 → 0.0003 GiB.

**2. state.redb's file counts a shared node once** (pocold `resources::chain_versions_bytes`,
`held_versions_bytes`; lp4 `node_trie::version_bytes`, `block_bytes`).
- The old count took each held version separately and counted every node at the largest encoding (a
  16-child branch, 1,162 bytes with its row). At the network profile every version saturated: about 108
  whole versions, a 14.3 TiB file and 3,653 redb regions.
- Two facts, both in the code:
  - **Nodes are stored once.** They are content-addressed: `state_store::commit` adds a reference to a
    node that is already stored, and `release` deletes a node when its last reference goes. So n versions
    at consecutive heights of one chain keep the oldest version's nodes plus what each later block's
    version added. They also keep at most n whole versions.
  - **Nodes are counted by kind.** A version of A accounts and S slots has:
    - one leaf per key (≤ 191 bytes for an account, which includes its storage trie's top; ≤ 98 for a slot);
    - at most one branch per leaf, since `build` and `finish` make no branch with fewer than two children
      (70 bytes without its children);
    - at most one child reference per node (66 bytes each), since each node hangs from one branch.

    A block adds its batch's branches and changed leaves. The displaced leaf is the parent's, unchanged.
- **The held versions** (`held_runs`, which `stored_versions` also sums for the handles), counted as runs:
  - genesis;
  - the K_hist window with the block being imported (K_hist + 1 consecutive);
  - the abandoned window until `settle` (K_hist consecutive);
  - side_max side states, one each;
  - a rolling re-execution and the version it commits next (2 consecutive).

  The entries hold these once, and each of the KEPT checkpoints holds them once more.
- **Finding.** The old formula did not count the abandoned window or the rolling entry, which
  `stored_versions` lists. Its saturation hid that undercount. The new formula counts both.
- Tests (lp4 `node_trie::tests`):
  - `a_store_keeps_at_most_what_a13_counts_for_its_versions`: random states of accounts with storage,
    with accounts created, removed and cleared. Each version, each block's addition and a window of four
    pinned versions stay within the bounds. A positive control checks that a block reaches a sizeable
    share of its bound.
  - `deep_shared_prefixes_stay_within_what_a13_counts`: raw keys sharing up to 64 nibbles, with branches
    beyond nibble 60.
  - `a13_store_file_and_batch_terms_follow_the_node_trie_bounds` (network_budget): re-derives the file
    parts from the node_trie bounds.
- File 14.27 → 2.20 TiB (563 regions). Allocator 1.809 → 0.279 GiB.

**3. A block's batch by node kind** (lp4 `node_trie::batch_nodes`).
- Every node an update puts in its batch is a node of the child version. It holds at most:
  - 64 branches per changed key, and no more than the child's leaves;
  - two leaves per changed key: the new one and the leaf it displaces;
  - 16 children per branch, and no more than one per node of the child.
- **Finding.** The 65-node count per key missed the displaced leaf, which `update` puts in the batch again
  (unchanged). In the old bound the leaves were charged at branch size, which covered it.
- `a_path_of_64_branches_and_a_displaced_leaf_reach_the_batch_bounds` builds both worst cases exactly. It
  checks a path of 64 branches rebuilt whole, and two leaves for one changed key.
- 1.024 → 0.916 GiB. The children term (0.63 GiB) now binds at the child's node count, not at 16 per branch.

**4. The chain's entries are boxed** (pocold `Chain::blocks: HashMap<[u8;32], Box<Entry>>`).
- A hash table keeps spare buckets. While it grows it holds its old buckets and twice as many new ones:
  up to 24/7 buckets per entry.
- Inline, each bucket was a whole 1,088-byte `Entry`, and the old ×3 count was short of 24/7.
- Boxed, a bucket is the 40-byte key and pointer. The term counts it at ×4, plus the Entry once and its
  allocation header.
- Slimmed, offloaded and side metadata: 0.786 → 0.524 GiB.

**Result** (tests/network_budget.rs; the computed bound, not measured RAM;
evidence/memory-r14/a13-models.log):

| profile | whole-state mode | store mode |
|---|---:|---:|
| network | 25.24 → **24.98 GiB** (gate 28.55) | 7.605 → **5.086 GiB** (gate **5.81**, budget 6) |
| devnet | 3.04 → 2.99 GiB (gate 3.41) | 3.48 → 2.44 GiB (gate 2.79) |

| item (network, store mode) | §2d | now |
|---|---:|---:|
| code read by the execution in progress (CODE_CAP) | 1.125 | 1.125 |
| EVM execution memory and operation classes | 0.979 | 0.979 |
| a block's batch | 1.024 | 0.916 |
| state.redb allocator | 1.809 | 0.279 |
| node cache (NODE_CACHE) | 0.250 | 0.250 |
| canonical metadata, offloaded and slimmed | 0.593 | 0.342 |
| side metadata | 0.193 | 0.181 |
| the pool's layer: writes / code | 0.127 / 0.618 | 0.127 / 0.0003 |
| everything else | 0.887 | 0.887 |
| **total** | **7.605** | **5.086** |

**Updated 2026-10-27 (R3).** Two changes raise this total.
- `5abbaf3` adds two stored-version handles.
- The deferred checkpoint release counts one owed checkpoint's versions, and a release row for at most each of
  their nodes (§3c).

Network store mode is now **5,581,321,485 B (5.198 GiB), gate 5.94 GiB**, budget 6 GiB, model cap 5.25 GiB.
- The state.redb allocator term rises from 0.279 to 0.391 GiB; its largest file is 3.08 TiB.
- Devnet store mode is 2.449 GiB.

**Mutation controls** (evidence/memory-r14/mutants.log): 25 mutants of the new code, 24 killed, one
equivalent.
- P6 drops the `min(n·whole, …)` of `chain_versions_bytes`. It is equivalent for every input the types allow:
  - every term of `block_bytes` is at most the same term of `version_bytes`, so the `min` never picks
    its first argument;
  - `n·whole` cannot overflow, since it is below 2^107 for u64 caps and a u32 K_hist, so the original never
    returns `None` where the mutant would not.
  - The proof is in evidence/memory-r14/README.md. `node_trie::tests::a_blocks_addition_is_at_most_a_whole_version`
    pins its premise.
- N11 (`ROW` 36 → 32 bytes) survived the node_trie tests: their bounds have slack for a row 4 bytes short.
  It is killed by `state_store::a13_row_is_what_state_redb_keeps_per_node` (`8d80354`), which checks `ROW`
  plus each node's encoding against the rows state.redb keeps, exactly.
- P8 (the map slot of a block entry at ×3) survived `network_budget` and is killed by
  `a13_bound_breakdown`, which pins the devnet total.

**What this does and does not establish:**
- The store-mode model of the network profile is now under the 5.25 GiB model cap. Its gate (5.81 GiB) is
  under the 6 GiB budget.
- This is the computed bound. It is not measured RSS and it is not acceptance:
  - the network profile is still refused at boot for its other gates (A13 is one of them);
  - store mode stays opt-in;
  - nothing here was measured on a near-cap network-scale state.
- **It still rests on:**
  - the inventories by inspection (`BATCHES`, `EXECUTIONS`, `stored_versions`; conditions `store-inventory`
    and `transient-snapshot-inventory`);
  - the measured file factor (STATE_FILE_FACTOR = 12, not established for files above a few hundred MB:
    condition `store-file-factor`; each unit is now about 0.023 GiB of the bound);
  - revm 19.7's loading behaviour (`revm-loading`).
- **The key-path assumption** (at most 21 nodes per path) is still not used. With it, the batch would be
  about 0.33 GiB and the allocator about 0.15 GiB (estimated from the same formulas): about 0.7 GiB less.

## 3. System gas (R18) and timing (R14/R19)

- Gas: per-call maxima measured by `resources::tests::sys_gas_envelope_fixture` (worst case plus a
  sweep-deletion scenario, which also exposed two gaps in the earlier P7 fixture: ordinary-phase pruning
  and sweep deletions were never exercised), composed into the worst call sequence per phase; the
  fixture fails if any value exceeds the table. Worst composition 87.7M ≤ G_SYS 96M (measured worst
  block 83.7M). Applicable only inside S_max ≤ 256, M_max ≤ 256, D ≤ 16, E_max ≤ 4, RET ≤ 128; outside
  it the gas facts are absent and R18 fails.
- Timing: measured per profile on the admitting machine (blocks with a GAS_LIMIT compute loop through
  closing and the final block). Representative, not adversarial worst case.
- **Timing of a store-mode node (2026-10-25).** Until then a node that boots in store mode was admitted on the
  in-memory timing. That timing does not include the path such a node runs: each block applied to a version
  read in place from state.redb, its trie updated, and its nodes committed and fsynced.
  - `load_spec_for` now runs the same calibration blocks also through a scratch state store at
    `<data>/calibration.store`, on the node's own disk, before the node opens:
    `admission::measure_on_store`, which uses `apply_block_on`, the trie update, the root and a durable
    commit, all inside each block's timed span.
  - R14/R19 then see, per phase class, the larger of the two times (`Timing::with_store`).
  - The admission condition `timing-not-a-bound` says that the store was included.
  - The scratch store is removed before the node opens. One a crash left behind is removed first.
  - Whole-state mode is unchanged.
  - The memory it uses is transient, before the node allocates its own, and within the store terms the
    node itself is counted for (the node cache and redb's page cache).
  - **On this host** (`jm_r14_boot_timing_by_mode`, five runs, default devnet profile):
    - in memory, 8–18 ms per phase (15 runs, five on the working tree and ten in acceptance on 24cc167);
    - through the store, 10–26 ms;
    - t_exec is 1,000 ms.
  - **What it still does not cover.** The workload is compute-only and writes little state. State-heavy
    blocks are slower than t_exec on this host in both modes (evidence/c1-e4/disk-impact-final.md, large
    state: max 1,068.5 ms whole-state and 1,045.1 ms store; small store: one outlier at 4,841.8 ms).
    Whether the calibration should include a state-heavy workload is a decision. If it did, this host
    would refuse both modes at t_exec = 1 s.

### 3b. The calibration workload against the slowest blocks (2026-10-27)

`journal_measure::jm_r14_workload_gap` (ignored; evidence/memory-r14/r14-gap-*.log) runs in one
process:
- the boot calibration, in memory and through a scratch store;
- for a whole-state node and a store-mode node, the validate time (median of 3) and the import time of one
  full-gas block of each kind;
- each kind first on the devnet genesis state, then after the state is grown to 18,787 accounts and 47,704
  slots, near the devnet caps (ACCT_CAP 20,000, SLOT_CAP 50,000).

Each kind block is followed by an empty block. It is a measurement on this host (4 vCPU KVM, ext4 on
virtio), not a bound, and it gates nothing.

**A correction first: the test-only root checks.**
- pocold's tests build lp4 with the `root-check` feature, and `POCOL_ROOT_CHECK` defaults to on.
- Under it, every block is also run on the full reference path:
  - a whole-state copy, with the state root recomputed from every key;
  - in store mode, whole-state scans of the views.
- The node binary does not have the feature.
- The first run left the checks on. Building the grown state took 499 s with them and 8.5 s without.
- On the grown state, the calibration's own compute block validated in:
  - 1,475 ms with the checks against 17 ms without (whole-state mode);
  - 330 ms against 38 ms (store mode).
- The figures below are with `POCOL_ROOT_CHECK=0`, as C-1 E1 prescribes for timing runs.
- **The state-heavy times cited in DECISION-PACKAGE.md and §3 above came from journal_measure runs that do
  not show the setting** (evidence/c1-e4/disk-impact-final.md: up to 1,068.5 / 1,045.1 / 4,841.8 ms). So they
  likely include this work. They are kept as recorded, and this run supersedes them for the decision.

**Times with the checks off** (ms, validate median / import; store-mode imports before the fix below):

| kind (one full-gas block) | whole-state, genesis | whole-state, grown | store, genesis | store, grown |
|---|---:|---:|---:|---:|
| GAS_LIMIT compute loop (the calibration's workload) | 12 / 12 | 17 / 25 | 11 / 28 | 38 / 72 |
| 380 transfers | 68 / 95 | 76 / 78 | 71 / 132 | 94 / 255 |
| **MODEXP, 8-byte even modulus, 1,024-byte exponent** | **1,572 / 1,625** | **1,285 / 1,182** | **1,230 / 2,432** | **1,441 / 2,609** |
| **MODEXP, 8-byte even modulus, 4,096-byte exponent** | **1,417 / 1,535** | **1,261 / 1,397** | **1,381 / 2,569** | **1,315 / 2,535** |
| BN pairing, one pair | 411 / 279 | 315 / 303 | 277 / 666 | 367 / 621 |
| keccak over 32 KiB | 240 / 232 | 178 / 188 | 191 / 437 | 192 / 404 |
| ecrecover | 135 / 192 | 149 / 178 | 159 / 336 | 151 / 304 |
| cold CALLs | 22 / 25 | 43 / 56 | 68 / 136 | 300 / 465 |
| CREATE2 loop | 11 / 13 | 18 / 25 | 47 / 112 | 215 / 246 |
| fresh accounts (value CALLs) | 7 / 8 | 15 / 22 | 20 / 51 | 130 / 202 |
| fresh SSTOREs, LOG0, memory growth | ≤ 7 / ≤ 14 | ≤ 13 / ≤ 20 | ≤ 18 / ≤ 44 | ≤ 30 / ≤ 72 |

Calibration on the same run: 8–10 ms per phase in memory and 15–19 ms through the store; t_exec is 1,000 ms.
Repeated runs on this host vary by up to about 2× (the MODEXP block took 2.5 s in the first run).

**The difference between the calibration and the blocks that exceed t_exec.**
- **The calibration's workload is the cheapest per gas there is:** about 1.5–2.5 ns per gas.
  - It is one transaction looping JUMPDEST/PUSH/JUMP, so it measures the interpreter's dispatch only.
  - It reads no state, writes almost none, calls no precompile and verifies one signature.
- **The only blocks over t_exec are MODEXP with an even modulus:** 160–200 ns per gas, about 100× the
  calibration.
  - EIP-2565 prices MODEXP from the input lengths and the exponent's bits.
  - revm 19.7 takes a slower path for an even modulus, which that pricing does not reflect.
  - This is D-2: per-gas pricing, not state.
- **Next per gas:** BN pairing (≈ 40), keccak over large inputs (≈ 22–24), ecrecover (≈ 19), and many
  transfers (≈ 10–12: a signature per transaction).
  - All of them are under t_exec here, in both modes and on both states.
- **State size adds little when the checks are off:**
  - the compute block goes from 12 to 17 ms in whole-state mode and from 11 to 38 ms in store mode;
  - state-heavy kinds read accounts from state.redb in store mode, so cold CALLs, CREATE2 and fresh
    accounts are slower there (130–300 ms on the grown state), but well under t_exec.
- **So the gap is in what the workload costs per gas, not in the state:** a calibration on the compute loop
  measures the fastest block and calls it the slowest.

**A store-mode import defect found by this measurement, and fixed.**
- Store-mode imports took about twice their validation (MODEXP: 2.6 s against 1.4 s).
- The cause: `Chain::restore_canonical` re-executes every canonical block above the K_hist cut that has no
  S3a diff, so that the diff links the head to a retained state.
- A store-mode entry has no diff. Its state is a stored version, and a re-executed one is only rolled and
  released.
- So every import that moved the head executed the old head and the new one once more, and committed and
  released a rolling version.
- The fix limits that condition to whole-state mode: the diffs are whole-state mode's. Offloaded blocks are
  still restored in both modes.
- Test `consensus::store_mode_imports_on_the_head_execute_each_block_once`:
  - store mode: 9 re-executions in 6 imports before the fix, 0 after;
  - whole-state mode: 0 before and after.
- The times after the fix are in evidence/memory-r14/r14-gap-after-fix.log.

### 3b-2. MODEXP: two exact fast paths, the shape sweep, and the slowest blocks after them (2026-10-27)

The precompile pocol's EVM runs at address 5 is now `lp4-execution/src/modexp.rs` (`native.rs` `build_engine`
puts it in place of revm's entry; the other addresses are revm's Cancun set). For every input and gas limit
it returns what revm 19.7's `berlin_run` returns: the same gas, bytes and errors. It computes two classes
itself and hands every other input to revm unchanged. No gas, rule or result changes.

**The two fast paths.**
- **A modulus value below 2^64** (`b46658a`): Montgomery multiplication on one word for the odd part q; the
  power of two 2^t directly (an even base to an exponent of at least t is 0; an odd base's exponent counts
  mod 2^64); joined by the Chinese remainder theorem (CRT).
- **An even modulus q·2^t of more than one word with t > 64 and q > 1** (`0c2ad75`): x1 = base^exp mod q by
  revm's own MODEXP (or the one-word path when q fits a word), and x2 = base^exp mod 2^t by revm's own (its
  power-of-two path). q^-1 mod 2^t comes from Newton's iteration, and CRT joins them.
  - Why this class: aurora-engine-modexp 1.3.0 inverts q mod 2^t one bit at a time (Koç's algorithm,
    `mpnat.rs` `koc_2017_inverse`). That is t steps on t/64 words.
  - EIP-2565 prices the call at c/3 with c = (m/8)^2 for a small exponent. The inverse grows as t² ∝ m², so
    the cost per gas stays at about 190 ns however large the modulus.
  - Newton needs about log2(t/64) multiplications.

**The sweep that found the second class** (`journal_measure::jm_modexp_sweep`, ignored).
- 17 modulus sizes (1 to 1,024 bytes) × 5 kinds (odd; even; a power of two; the low half zero; 3·2^(8m-2))
  × 5 exponents (all ones, from the 200-gas floor up to 32,768 bits), with the base as long as the modulus.
- 384 shapes within a block's gas. Each is checked equal to revm.
- Each is timed per call, for pocol and for revm. A full-gas block's time is projected as the calls a block
  makes (131 gas of loop per call) times the time per call. The slowest projections are then built as real
  blocks and validated.

| shape (base = modulus length) | gas per call | revm per call | pocol per call | full block, before → after |
|---|---:|---:|---:|---:|
| 3·2^8190, exponent 3 (1,024 bytes) | 5,461 | 1,062 µs | 156 µs | 1,554 ms → 227–267 ms |
| 3·2^4094, exponent 3 (512 bytes) | 1,365 | 273 µs | 47 µs | 1,563 ms → (not built) |
| 3·2^2046, exponent 3 (256 bytes) | 341 | 79 µs | 17 µs | 1,457 ms → 291–348 ms |
| 2^4096 · odd, exponent 3 (1,024 bytes) | 5,461 | 425 µs | 135 µs | |
| one word, even, 1,024-byte exponent | 2,730 | 512 µs | 86 µs | 957–1,023 ms → 238–272 ms |

The before and after per-call figures are from sweep-evenmax-before-wide.log and sweep-0c2ad75.log. The
one-word row is from exec-cost-per-gas.log. The blocks are from those logs and from r14-gap-d30dc71.log
(both modes, genesis and grown state).

**The slowest blocks now** (`r14-gap-d30dc71.log`, POCOL_ROOT_CHECK=0, ms, validate median / import; whole-state
genesis, whole-state grown, store genesis, store grown):

| kind | ws genesis | ws grown | store genesis | store grown |
|---|---:|---:|---:|---:|
| MODEXP 16-byte modulus, low half zero, 2,048-bit exponent (revm's path) | 464 / 421 | 409 / 456 | 474 / 454 | 449 / 476 |
| MODEXP 16-byte even modulus, 1,024-byte exponent (revm's path) | 425 / 419 | 427 / 490 | 403 / 450 | 434 / 510 |
| MODEXP 1,024-byte even modulus (t = 1), exponent 3 (revm's path) | 425 / 401 | 420 / 448 | 444 / 458 | 429 / 433 |
| MODEXP 3·2^2046, exponent 3 (wide path) | 308 / 309 | 348 / 320 | 291 / 304 | 330 / 302 |
| MODEXP 9-byte odd modulus, 2,048-bit exponent (revm's path) | 294 / 318 | 298 / 315 | 338 / 308 | 321 / 315 |
| BN pairing, one pair | 282 / 310 | 295 / 294 | 305 / 285 | 347 / 303 |
| BN mul | 271 / 254 | 281 / 275 | 271 / 258 | 285 / 295 |
| MODEXP one word, even, 1,024-byte exponent (one-word path) | 238 / 253 | 256 / 266 | 251 / 249 | 272 / 278 |
| MODEXP 3·2^8190, exponent 3 (wide path) | 227 / 281 | 243 / 277 | 257 / 241 | 267 / 268 |
| the calibration's compute loop | 12 / 19 | 15 / 23 | 19 / 21 | 30 / 37 |

- **Every kind measured is now under the devnet t_exec of 1 s,** in both modes and on both states.
  - The slowest is about 0.47 s to validate and 0.51 s to import, up to about 60 ns per gas.
  - These are MODEXP moduli of 9–16 bytes on revm's own path. BN pairing and BN mul are next, at 0.27–0.35 s.
- **The margin is about 2×,** and repeated runs on this host vary by up to 2×. So a host somewhat slower than
  this one could exceed t_exec on these blocks. That is what decision row 2 (ii) would detect at boot.
- **The sweep is a grid of measured shapes, not a proof of the maximum.** It found the second class only
  because a kind was added for it.

**Correctness evidence.**
- `modexp::tests`: 60,000 random and edge inputs (gas limits around the cost, inputs cut short) and 20,000
  one-word moduli against revm.
- 20,000 random wide moduli: q of 1 to 160 bytes, t around 64 and at its largest, leading zero bytes. The
  wide path ran in more than a third of the cases.
- The Newton inverse is checked directly. The swept shapes are checked one by one.
- Seeded faults (mutants.log, mutants-wide.log): 22 mutants of the arithmetic, the parsing, the gas check and
  the wiring, all caught. WX11 moves the threshold t > 64 to t > 1. That changes only which path runs, not the
  results, and the routing assertion catches it.
- **A defect of the new code, caught before commit:** `mul_trunc` dropped a row's carry when the second operand
  was shorter than the remaining words. The differential tests and the inverse test failed on it; it was fixed.
- One-shot per-call timings in the sweep are noisy on this host, up to about 2×. Interleaved
  (`modexp::tests::wrapper_overhead_interleaved`, wrapper-overhead.log), pocol's time on inputs it hands to
  revm equals revm's within noise. Its own checks cost 0.07–0.56 µs per call.

### 3a. EVM execution memory (`resources::memory::execution`, round 2, corrected)

One execution consuming at most G gas, with code at most `B_code_max` (revm 19.7, CANCUN). The first
round-2 version charged each kind of state separately at full gas and assumed that a loaded account or
slot costs its cold gas. The independent review showed that revm loads an entry into the journaled state
and the engine database before it charges the cold cost. So a frame that then runs out of gas keeps one
entry it did not pay for. The model was rewritten accordingly:

- **Operation classes** (`memory::classes`). Each class has the heap one operation leaves behind for the
  rest of the execution (growth peaks included) and the least gas the operation costs. A frame that runs
  out of gas after a load keeps that one entry and cannot load another. Creating the frame costs at least
  a warm CALL (100 gas). The classes:
  - an account loaded unpaid (100 gas);
  - a slot loaded unpaid (100 gas);
  - an account with its first slot (200 gas; it carries both storage maps' own share, since an empty
    hashbrown map allocates nothing, which is tested);
  - a TSTORE slot (100 gas);
  - a journaled write (100 gas);
  - a committed empty frame, whose journal Vec revm keeps after a commit (100 gas);
  - a log (375 gas), and its data and topics (8 gas per byte);
  - created code (200 gas per byte) and a creation (32,000 gas);
  - a modexp output (at most 192 B per 200 gas, EIP-2565).

  Paid loads (cold 2600/2100, access lists 2400/1900) add the same entries for more gas.

  With a accounts, s slots and m ≤ min(a, s) non-empty maps, the bytes are at most A·a + S·s + B·m for at
  least 100(a + s) gas. Over that region the largest bytes per gas is at a = s or on an axis, which are
  the three state classes. Mixing classes adds nothing beyond the largest rate, since each class is linear
  in its own gas (`class_heap`).
- **Memory** (`memory_heap`). At most N = 2·1025 regions are paid for at once (the active frames, plus one
  returned child per frame whose output is held), from disjoint gas, at 3w + ⌊w²/512⌋ ≥ 3w + w²/512 − 1
  each. By convexity their words total at most W = N·(words(g/N + 2) + 1). The bytes counted:
  - the shared buffer, at 3 × its length (Vec capacity ≤ 2 × the length, plus the old buffer while it
    reallocates);
  - the frames' inputs (copies of ranges of their callers' memory);
  - the returned outputs held: 160 B per word.
- **Gas split.** `memory_heap` is increasing and `class_heap` is linear, so on each step of a 4096-point
  grid the sum is at most memory_heap(g_k+1) + class_heap(G − g_k). The bound is the largest of these.
- **Fixed part** (`fixed_heap`). It covers:
  - 1025 frames, each with a 32 KiB stack and a boxed frame of ≤ 4 KiB (size checked by a test);
  - each frame's analysed code: revm re-analyses raw bytecode per frame into a padded copy and a jump
    table, at most max(B_code_max, 49,152 B of init code), with one frame also holding the raw copy;
  - the call-stack and shared-memory preallocations, and small precompile outputs (64 B per frame);
  - 33 fixed accounts;
  - the BLOCKHASH entries the engine database caches (revm asks it only for the last 256 numbers);
  - precompile working memory ≤ 8 × the largest single-frame memory, and 8 MiB of static tables.

At G = 30M and B_code_max = 24,577 the bound is 1,131.2 MiB: fixed part 128.6 MiB, plus 1,002.5 MiB at the
worst gas split.

revm facts it relies on, checked in the revm 19.7.0 / revm-interpreter 15.2.0 sources:
- frames run iteratively from a `Vec<Frame>` (capacity 1025);
- `checkpoint` pushes one journal Vec per frame, and a revert truncates it;
- loads precede the cold charge (host.rs `sload`, `balance`, the call target);
- `Contract::new` analyses raw bytecode per frame;
- the initcode limit is 2 × MAX_CODE_SIZE, unless a code-size limit is set (LP4 sets one only around the
  deposit check);
- BLOCKHASH queries the database only within 256 blocks;
- `Stack` has capacity 1024;
- an empty `HashMap` does not allocate;
- type sizes come from `size_of`.

A revm upgrade requires re-checking these facts.

Validation:
- Public, model-level: `resources::tests::execution_model_bounds_measured_adversarial_peaks`. The earlier
  flood, recursion and memory cases run at 30M gas against the bound. Every peak is far below it (largest:
  TSTORE flood, 93 MB).
- Also public: `loaded_accounts_start_with_unallocated_storage_maps`, which checks the map accounting.
- Private: the measurements that exercise unpaid loads, committed-frame journals and deep recursion with
  large code, each against its own term rather than the total, are kept out of this public repository.
  They are delivered to the owner in the private review package (`evidence/PRIVATE-REVIEW-PACKAGE.md`
  lists the files and hashes). The public tests do not replace them.

### 3c. A store-mode node at the network's state caps, measured against this bound (2026-10-27)

Evidence: evidence/memory-r14/scale-d30dc71/README.md (`jm_store_scale`, release, ignored). It is a measurement
on one host under one synthetic workload, not a bound.

- **Profile:** the devnet profile with the network's state caps (ACCT_CAP 10^6, SLOT_CAP 4·10^6, CODE_CAP 1 GiB,
  TXBYTES_CAP 8 GiB, H_END 129,600), in store mode. **This section's bound** for it: 5,255,359,885 B (4.894 GiB);
  5,255,514,985 B after `5abbaf3`.
- **Grown** to 950,000 accounts and 3,800,300 slots:
  - VmHWM 474 MiB; after an open from a checkpoint 539 MB, and 477 MB after `5abbaf3`;
  - state.redb 2,056 MiB, with 638 MiB allocated;
  - the stored versions and checkpoints 0.20 of one version's `version_bytes` at the same counts;
  - the node cache at NODE_CACHE.
- **The terms this workload exercises behave as counted:** the stored versions, the node cache, the entries.
  The largest terms are worst cases it does not reach: the pool at its gas bound, CODE_CAP code sets, a
  maximal batch, RPC, peers, a deep reorg's abandoned window. So the measurement does not close the bound.
- **Open time (D-6):**
  - an open from a checkpoint read each of 33 stored versions whole, 16–17 s each, 783–801 s in all;
  - `b6d57e8` reads a version whose parent's was read through `NodeTrie::diff` (§ condition
    `open-replay-transient`), and the open took 262 s;
  - the touched set it holds is one block's writes at most, as an import's write set, so it adds no term.
- **Checkpoint stalls:**
  - a checkpoint that dropped the oldest one spent 8–11 s releasing what only the dropped one held, and 2.1–2.5 s
    committing, inside its transaction on the consensus thread.
  - **Deferred release** (D-6's mechanism, no rule change; C1-STORAGE-DESIGN.md, E4.4): the write owes the
    dropped checkpoint's nodes in a `release` table, and `release_step` works them off between events. The next
    write finishes what is still owed first, so at most one dropped checkpoint is owed.
  - **The file term** (`state_file_parts`) now counts (KEPT + 2) sets of held versions instead of (KEPT + 1),
    plus a `release` row (ROW bytes) for at most each node of the owed set. Every node it counts is at least
    ROW + BRANCH_ENC bytes, so a set has at most bytes / (ROW + BRANCH_ENC) nodes.
  - Computed for the network profile in store mode, with (KEPT + 2) sets before the release rows (scratch
    build):
    - the largest file from 2.20 to 2.86 TiB;
    - the redb allocator term from 0.2789 to 0.3626 GiB;
    - the total from 5.086 to 5.170 GiB.
  - **Committed, with the release rows** (`a13_store_model_at_the_network_profile`): **5,581,321,485 B (5.198
    GiB)**, 5.941 GiB required. The allocator term is 0.3908 GiB, and the largest file 3.08 TiB.
  - The rows' bound is loose: every node counted at ROW + BRANCH_ENC bytes, where most are larger.

## 4. Assumptions, one by one (round 2)

Each assumption of the bound and of the R14/R18/R19/R21/R25/R26 facts, with what was done:
- **ENFORCED**: code now makes it true, and the bound uses the enforced value;
- **DERIVED**: a conservative bound from stated rules, validated by measurement;
- **INSPECTION**: holds by the structure of the code, but no counter or test enforces it;
- **OPEN**: not established. The corresponding acceptance obligation stays open.

The admission reports each open item that affects a gate fact as a `Condition`. A `Scope::Specification`
admission is refused while any remains (D-A02-SCOPE).

| # | assumption | status | basis / remaining obligation |
|---|---|---|---|
| 1 | SYS gas is monotone in the domain fields (GAS_ENVELOPE covers every profile in GAS_DOMAIN) | OPEN | No opcode-level worst-case derivation. Admission condition `gas-monotonicity-assumed` (R18) |
| 2 | R14/R19 timing is a worst-case bound | OPEN | `measure` applies one synthetic GAS_LIMIT compute workload per phase on this host, a representative sample. Not a bound for worst-case state size, world-root rebuild, PAY or settlement paths. Admission condition `timing-not-a-bound` (R14, R19). **Round 3 (JOURNAL-GROWTH-AND-REPLAY.md §3.2a, §3.2d), measured on this host:** one valid one-tx MODEXP block validates in about 1.0 s, and in about 1.3 s with the state at the devnet caps, above the devnet t_exec = 1 s (single_host; R14 requires t_exec ≥ validate_p99, governance.md:93); a valid 380-transfer block re-executes in about 10.2 s on a near-cap devnet state, above t_exec = 1 s and T_blk = 4 s. The admission gate itself was not re-measured |
| 3a | thread stacks and the exiting-thread overlap | ENFORCED (measured + conservative) | Every thread is spawned with an explicit reservation and counted at it (io_threads). The inventory is enforced by join ordering: the acceptor, listener and dialers join a finished thread before reusing its place (`threads::reap`); a reader joins its writer; the miner is joined before the next starts. A thread can finish between its reaper's visits: the bound on exiting-not-yet-joined threads is E = peers_max + dials + conn_max (writers join synchronously; singletons never respawn), counted CONSERVATIVELY at a full IO_STACK each (10.5 MiB). Measurement (evidence/threadmem.rs, evidence/THREAD-INVENTORY.md): a finished joinable thread retains only ~25.8 KiB, not 256 KiB — the bulk returns to the glibc stack cache (the 40 MiB STACK_CACHE term) — so the term is an upper bound, not the measured figure, as the owner directed (measurement is evidence, not a guaranteed ceiling). Tests: `peer_threads_stay_within_the_inventory_while_consensus_is_stalled`, `miner_replacement_keeps_at_most_one_miner_thread` (handle high-water 1; mutation fails), F1's lost-wakeup unit test. Residual: the main thread's stack is platform-reserved, counted at 1 MiB (INSPECTION) |
| 3b | allocator per-allocation overhead and fragmentation | OPEN (reserve kept) | Not modelled; the total is requested bytes. The boot gate keeps 1/8 of the budget outside the model (D-A13-BUDGET). Measured: full-cap snapshot clones use 0.45 of their model in RSS; header estimate for history under 8 MB (§2b). Fragmentation is not bounded. Status condition `allocator-overhead` |
| 4 | revm per-transaction journal, engine cache and memory fit in the transient-snapshot allowance | DERIVED (replaced, corrected) | Its own term, §3a (1,131.2 MiB at 30M gas): operation classes (unpaid loads included), memory over a gas split, a fixed part (per-frame code analysis, BLOCKHASH cache). Validated publicly at model level and by private adversarial measurements, each case against its own term (PRIVATE-REVIEW-PACKAGE.md). Depends on the revm 19.7 facts listed in §3a (status condition `revm-representation`). One execution at a time is by INSPECTION |
| 5 | world-root rebuild per block; `eth_getTransactionByHash` chain scan | OPEN (CPU) | Not memory terms; part of the timing obligation (2). **Measured (round 3, §3.2b):** `apply_block` clones the state and recomputes the whole world root on every block, about 3 µs per live slot and about 6 µs per account on the devnet (about 0.23 s with both devnet caps nearly full); at the network caps the world root alone takes about 14.4 s (synthetic state). **Round 3c (§3.2d):** every USER tx runs `validate_snapshot` and two whole-snapshot clones, and every scan keccaks each runtime (`code_len` hash_slow), so re-execution is the per-block term + txs × (validate_snapshot + 2 clones) + creates × current_total, each O(accounts + slots + logical code bytes). Measured near the devnet caps: 380-transfer block 10.2 s, CREATE2 child 2.4 s (about 8 ms per create), empty block 0.41 s; attributed by four scratch timing mutants. The create term `current_total` is 0.47–0.54 s per 247-create block at the devnet ACCT_CAP (accounts only, §3.2c). Network: about 7.9 s per tx projected (linear, about 512× beyond the measured code range). Round 3b's "a create-heavy block is the per-block worst" was withdrawn in round 3c |
| 6 | deep-fork rebuild re-executes from the nearest retained ancestor | memory ENFORCED, CPU OPEN | Two snapshots (`a13_rebuild_of_a_deep_fork_keeps_only_the_target_state`); CPU proportional to depth. **Round 3 (§5.3):** below K_hist the nearest retained ancestor is genesis, and a deep reorg pays the rebuild twice, because `retain` prunes the canonical fork point below head−K_hist: per block 1.8 ms (empty) to about 1.0 s (worst kind measured); a depth-2048 reorg near the devnet tip takes 96 s (empty) to about 11 h (worst kind), in two stalls that each exceed D_att |
| 7 | every new head re-executes the pool | memory ENFORCED, CPU OPEN | ≤ POOL_MAX_TXS, one execution at a time; CPU not bounded. **Measured in a loaded reorg (round 3, §5.3):** the adopting import's pool re-execution of 50 abandoned one-tx MODEXP transactions took about 50 s; the bound of up to 1024 blocks (REORG_RESTORE_MAX) is from reading the code. **Since `462fffc`** (PR #27 review thread): the walk reaches the common ancestor, so up to R_keep = 2,048 abandoned blocks are restored (the old walk stopped after 2·1,024 steps and lost the oldest ones). Their bodies are read from the journal one at a time; an unmodelled transient, a copy of every detached transaction (≤ TXBYTES_CAP = 16 MiB on the admitted profiles), is gone |
| 8 | timing measured at load; R25d calibration is synthetic | OPEN for specification | Admission conditions `timing-not-a-bound` and `calibration-synthetic` (R25) |
| 9 | R26 facts for unimplemented RpcGuard mechanisms | FIXED as evidence; mechanisms IMPLEMENTED in part (round 3) | Absent fields enter the gate as 0 and are never nominal. Round 2 recorded: 4 predicates checked, 14 waived; the 22 mechanisms not implemented. **Round 3 (RPCGUARD.md §8):** RpcGuard is implemented (owner approved); 25 facts are present at their running values, 6 of them by an equivalent bound named in the admission notes; 4 are absent (scratch_heavy, scratch_light, mem_total, cancel_max_ms). In the experimental-devnet scope 15 of the 18 R26 predicates are checked and pass, and 3 are waived and reported. The specification scope still refuses (`specification:rpc-absent`) |
| 10 | RetentionStore (disk) and redb trie | OPEN | Not implemented. The governance R21 historical-trie model is not exercised. The gate's deferred items `LP4:general-R18`, `LP4:general-R21` and `LP4:caps64-minimum-SYS_SLOT_MAX` remain (admission conditions `gate-deferred-*`) |
| 11 | TRANSIENT_SNAPSHOTS = 10 live clones (C-1 E3: 2, plus POOL_LAYER_SNAPSHOTS = 1) | INSPECTION | Listed in `resources.rs` with their code locations; no runtime counter. Since C-1 E3 the block, production, template selection, RPC simulation and the pool run on overlays, and the two remaining copies are named in `resources.rs`. Status condition `transient-snapshot-inventory` |
| 12 | hashbrown layout and entry sizes | DERIVED | Sizes via `size_of`; growth policy validated for the pinned toolchain by `snapshot_model_bounds_measured_clone_bytes` |
| 13 | EVM executions do not overlap | INSPECTION | Blocks, pool, eth_call/estimateGas and admission timing run on the consensus thread (`EXECUTIONS` = 1) |
| 14 | RPC answers held by connections | ENFORCED | Response pools (`rpc::answer`): the class maximum is reserved before the request runs and released when the reply drops; a full heavy pool answers busy before anything runs. Tests `response_pools_reserve_shrink_and_release` and `a_full_heavy_pool_answers_busy_before_anything_runs` (its mutation, reservations never fail, fails) |
| 15 | RPC request bodies, queued and in flight | ENFORCED | A body of exactly Content-Length ≤ REQ_BUF. Queued heavy bodies are capped at queued_max (HEAVY_QUEUE = 16 since RpcGuard round 2) and released when skipped. Test `abandoned_queued_bodies_are_capped_and_released` (peak = cap; its mutation, no cap, fails); since `1cc96c4` it runs with HEAVY_QUEUE 2, QUEUE_WAIT 5 ms and up to 12 rounds, assertions unchanged (Windows run 20 flake) |
| 16 | the answer being built: one member's JSON tree and string ≤ 4 × RESP_MAX | INSPECTION | The parse tree of the request is ENFORCED (JSON_VALUES_MAX before parsing, `parse_bytes_max`). The member's result tree is bounded by RESP_MAX of output (`capped`), but its tree-to-text ratio is counted at 4 by inspection |
| 18 | adoption of a heavier competing branch at any depth ≤ R_keep (network.md:144) | **IMPLEMENTED (D-REORG-REDB, owner-approved 2026-10-23); verification on the final commit: REPAIR-REPORT.md §D-REORG-REDB.** Mechanism and proof: proposals/D-REORG-REDB-DESIGN-FREEZE.md Revision 2: metadata fork choice; offloaded side blocks; protected locator sync sessions that re-fetch evicted branches; restore of adopted blocks; replay equal to live. Tests: `adoption_at_the_spec_depth_2048_and_not_at_2049` (release), `r_keep_boundary_adopts_at_depth_r_keep_and_keeps_the_valid_chain_beyond`, `d133_tie_break_at_depth_400_holds_through_eviction_and_refetch` (release), `a_session_branch_is_adopted_through_a_flood_of_competing_side_branches`, `a_sync_session_refetches_an_evicted_branch_and_serves_it`. The round-2 text below is kept for history: | OPEN (architectural); depth raised 256 → 320 | Fork choice (`better`, `rebuild_canonical`) walks a branch's ancestors through resident `self.blocks`, and `retain` drops only leaves (no holes), so a competing branch must be FULLY resident to be adopted: the one-shot adoption depth IS SIDE_BLOCKS_MAX. Round 2 raised it 256 → 320 (the largest value fitting the budget with margin, D-A13-BUDGET addendum) via the tested validate()+insert() path. A branch deeper than 320 is PRESERVED in the journal (lossless) and NOT refused forever; it is simply not adopted in one shot — the node stays on the valid lighter chain (consensus.md D118: fetched by sync; governance.md:200: manual checkpoint beyond R_keep). Reaching R_keep = 2048 within budget needs the block bodies off the heap (~0.85 GB otherwise), i.e. a redb block store + streaming reorg — a consensus-critical mechanism PROPOSED for explicit owner approval (proposals/D-REORG-REDB.md), not implemented. No consensus rule is weakened: the node never adopts an invalid chain. Tests: `adopt_a_heavier_branch_at_the_cap_and_survive_restart` (a 270-deep heavier branch is adopted and survives restart; mutation cap=256 fails it), `a_branch_deeper_than_the_cap_is_preserved_not_adopted` (a 382-deep branch is kept in the journal, not adopted, reopens without panic), and `a_competing_branch_within_side_blocks_max_is_adopted` (150-deep) |
| 17 | the budget's reserve | ENFORCED (gate) | `check` requires the model total ≤ 7/8 of the budget (`RESERVE_KEEP_EIGHTHS`). The boot test asserts that the default boots, that one byte below its requirement is refused, and that exactly the requirement boots |

Consequence:
- The devnet admission is an **experimental-devnet admission** (D-A02-SCOPE), reported as such at boot
  and in `status`.
- The model total (5,581,426,268 B = 5.198 GiB at the round-3 code; 5.237 GiB, 5,623,469,297 B, when
  this list was written at 687ac38; dated totals in §2b) is conditional on:
  - item 3b, which has a 12.5% reserve kept for it but is not proven;
  - item 4's revm facts;
  - items 11, 13 and 16, which rest on inspection.
- It is not a measured value and not a guaranteed ceiling (§2b).
- A13 is not closed: items 1, 2, 3b, 5, 6 (CPU), 7 (CPU), 8 and 10 remain open acceptance obligations.
  Item 18 (R_keep adoption) is implemented by D-REORG-REDB, and its verification status is recorded in
  REPAIR-REPORT.md.
- Round 3 adds the critical performance findings below and keeps the three journal obligations OPEN.

### 4a. Critical performance findings (round 3, 2026-10-24): all OPEN

Measured on this host on the devnet profile (JOURNAL-GROWTH-AND-REPLAY.md, journal rounds 3–3c; code
whose src equals `fa97594`'s, so they apply to the head). Each is a lower bound on the worst case. None
is mitigated; the mitigations in §7 of the journal report are PROPOSED.

| finding | measured | limit it exceeds | cause | items |
|---|---|---|---|---|
| a valid block of 380 transfers on a devnet state with accounts, slots and code near the caps | about 10.2 s (`1d094de`, evidence/journal-r3c/txstate.log) | T_blk = 4 s (and t_exec = 1 s) | per-tx `code_caps::validate_snapshot` plus two whole-snapshot clones (`native::build_engine`), and a keccak of every runtime on each scan (`code_caps::code_len`) | 2, 5 |
| the worst measured one-transaction full-gas block (MODEXP, small even modulus, long exponent) | about 1.0 s (0.96–1.08 s; about 1.3 s with the state at the devnet caps) | devnet t_exec = 1 s | per-gas cost of the precompile, about 128 ns/gas | 2 |
| the whole state root on every block | about +0.23 s per block near the devnet SLOT_CAP/ACCT_CAP; about 14.4 s per block at network caps (synthetic) | grows with the live state | `apply_block` clones the state and rebuilds the world root | 5 |
| the worst devnet open at H_END | about 7.3 h, plus about 8.5 min of TX_CAP-bounded transfer blocks | none stated; a restart takes hours | open replays every record from genesis (journal obligation) | 2, 6 |

The three journal obligations (§8 of the journal report) stay OPEN: disk growth (no compaction; the
solicited re-fetch re-journal vector), re-execution from genesis (open, and the deep-fork rebuild paid
twice), and the open-time memory transient (about 1.0× the journal on top of the steady chain; condition
`open-replay-transient`). The network profile is not admissible on pocold (TXBYTES_CAP = 8 GiB alone
exceeds the 6 GiB budget), so every network figure is a projection.

## 4-r1. Open obligations as recorded in round 1 (kept for history)

1. Gas monotonicity in the domain fields is assumed, not proved; no opcode-level worst-case derivation.
2. R14/R19 timing is a representative measurement, not a worst-case bound (e.g. worst-case state size
   for the full world-root rebuild per block, or worst-case PAY/settlement paths).
3. Thread stacks are assumed to keep ≤ 64 KiB resident; allocator fragmentation and per-allocation
   headers are not modelled.
4. revm's per-transaction journal and engine cache grow with the keys a transaction touches (bounded by
   gas, ≈ GAS_LIMIT/2100 slots, a few MiB) and are covered only by the transient-snapshot allowance.
5. The full world root is rebuilt from all accounts every block (CPU proportional to state size), and
   `eth_getTransactionByHash` scans the canonical chain; both are CPU, not memory, costs.
6. A deep-fork rebuild re-executes every block from the nearest retained ancestor (memory is now two
   snapshots, CPU is proportional to the depth).
7. Every new head re-executes the pool (≤ POOL_MAX_TXS transactions, each on a copy of the pending
   state): CPU, not memory.
8. R14/R19 timing is measured when a node loads the devnet (`load_spec`, used by `run` and `status`), on
   that machine; the record init-devnet writes is informational. R25d is the declared synthetic devnet
   calibration (`--work-bits`), not a measured hash rate.
9. R26 receives pocold's implemented RpcGuard values from its code (`pocold::rpc_facts`: connection cap,
   request size, RESP_MAX, BATCH_MAX, BATCH_RESP_MAX, LOGS_COUNT_MAX, LOGS_SCAN_BYTES). pocold does not
   implement the remaining RpcGuard mechanisms: HEAVY/LIGHT response memory accounting, scratch budgets,
   BatchStreamer pacing, rate buckets, RPC_MEM_TOTAL. Those fields keep their nominal values; the memory
   they would bound is covered by the RPC term above instead.
10. The node bound covers pocold's in-memory representation only; the spec's RetentionStore (disk) and
   redb-based trie are not implemented, so the governance R21 historical-trie model is not exercised.
