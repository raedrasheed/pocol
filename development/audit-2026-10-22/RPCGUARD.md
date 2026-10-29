# RpcGuard in pocold: spec item, mechanism, status, test and mutant

**Scope.** This file covers `development/m1/reference/network.md:182–330`, the RpcGuard section
(D87, D88, D90, D92, D93, D94, D95). RpcGuard is a local service policy, not a validity rule
(D87, consensus.md §13). Changing it never changes consensus and needs no fork.

**Architecture.** The spec assumes an async server with a 20-thread compute pool and HTTP/1.1
keep-alive. pocold differs:

- it runs **one consensus thread plus a light lane**;
- HTTP/1.1 carries **one request per connection**, and the connection closes after its response.

**Statuses.** Each mechanism has one of these statuses:

- **ENFORCED**: implemented as specified.
- **EQUIVALENT**: a different mechanism gives the same bound or a stronger one; the proof is stated.
- **PARTIAL**: enforced for part of what the spec covers. The part that is not enforced is named.
  It is reported absent in admission.
- **NOT ENFORCED**: the reason is given, with what stays open.

**Where things are.**

- Code: `prototypes/pocold/src/rpc.rs`; `prototypes/pocold/src/lib.rs` (`RPC_ENFORCED`, `rpc_facts`);
  `prototypes/lp4-execution/src/native.rs` (`simulate_for_profile` stop hook).
- Tests: `prototypes/pocold/tests/net.rs` (loopback sockets on 127.0.0.1, and in-process `rpc::answer`);
  `tests/consensus.rs`; the rpc unit tests; `prototypes/lp4-execution/tests/simulate_stop.rs`.
- Running values: `pocol_getParams.rpcLimits`. A limit pocold does not enforce is `null` there,
  never a nominal value.

**History.** This file was revised on wf/rpcguard-r2, after the independent review of wf/rpcguard
(stage A, 87367fe). The review refuted several of stage A's ENFORCED and EQUIVALENT statuses.
§9 lists every finding and how it was resolved. It was revised again on wf/rpcguard-r3 (from
fa97594), after the second independent review of round 2 and the integration (273e785): §12 lists
those findings (RG2-M1 to RG2-m8) and their resolution, §4a the SendPacer measurements. It was
revised again on wf/rpcguard-r3c (from 5e7446f), after the third review of round 3: §13 lists those
findings (RG3V-M1 to RG3V-m5) and their resolution. The statuses below are the current ones.

## 1. Transport and the S1 invariant (أ)

| spec item | pocold mechanism | status | test | mutant |
|---|---|---|---|---|
| HTTP/1.1 on 127.0.0.1, no HTTP/2, no pipelining | `serve` binds loopback only, otherwise it refuses. `respond`/`pace_write` write HTTP/1.1 with `Connection: close` | ENFORCED | existing `rpc_*` tests | — |
| One request at a time; the next is not read before sendDone | The connection closes after one response, so it never has a next request | EQUIVALENT (stronger) | every `rpc_*` test | — |
| S1: at most 1 reservation and 1 waiting request per connection | One `conn` thread handles one request, with one `Reply` reservation | ENFORCED | `an_answer_is_reserved_until_its_connection_has_written_it` | — |
| SO_SNDBUF = 32 KiB per connection (network.md:190) | **Integration (after round 2):** `rpc::sockbuf` sets SO_SNDBUF = `SEND_SOCK_BUF` (32 KiB) on every accepted RPC connection, through `libc` (Unix) or `windows-sys` WinSock (Windows). Both crates and versions were already in the lock; the lock change is only the two dependency edges. A connection whose buffer cannot be set is not served. **Round 3 (review m6):** "cannot be set" now also covers a read-back outside [SEND_SOCK_BUF, 2·SEND_SOCK_BUF] (`sockbuf_ok`: Linux reports the doubled value, Windows the value), and such a connection is counted as `sockbufRefused`, no longer with the RPC_CONN_MAX refusals; a test hook (`SOCKBUF_FAULT`) injects both failures. Set explicitly, the buffer is also excluded from autotuning: Linux locks it, and Windows stops its ideal-send-backlog tuning. So an unread answer holds at most the send buffer plus the client's receive window in the kernel. Measured on Linux: 226,476 B for a non-reading client, against an autotuned 3,939,840 B send buffer without the setting. Windows run 3 showed the default there: a 14 MiB unread answer taken by the kernel within a second. Linux reads back 2·32 KiB (bookkeeping), Windows 32 KiB. pocol_getRpcStats.sendSockBuf reports the value read back from the last connection. **The write cap (29fd44a):** the pacer writes at most SEND_SOCK_BUF per call, because Windows takes a whole send() whenever its buffer is not full when the call starts (Windows runs 3, 5 and 7), so SO_SNDBUF bounds the kernel's share there only with the cap. On Linux a non-blocking send takes at most the free (locked) send buffer, so the cap does not change A there; it keeps Linux throughput: without it one 64 KiB segment is in flight and each waits for a delayed ACK (1.7–1.8 MiB/s in the scratch bench, `evidence/rpcguard-r3/pollbench.log`, against 1.3–2.0 GiB/s with it; the third review measured pocold itself at 1.7–2.2 MiB/s without the cap). **Round 3c (review RG3V-M2):** round 3 called the cap's Linux survivor (I3b) harmless by design; it was not: no committed test asserted the honest throughput. `send_pacer_closes_a_slow_reader_after_grace` now requires the honest reader's ~14 MiB answer to be sent within 2 s (batchSendMaxS; at least 7 MiB/s; 15–18 ms measured in three runs), so I3b is caught on Linux by throughput (§13). | ENFORCED (`send_sock_buf` = 32768 at its running value; `sendSockBuf<=64KiB` checked) | `get_params_and_rpc_stats_report_the_limits` (read-back equals 2·32 KiB on Linux, 32 KiB on Windows); `a_connection_whose_send_buffer_cannot_be_set_is_refused` (round 3: a failing setting and read-backs 3·32 KiB and 32 KiB−1 are refused unserved and counted as sockbufRefused; 32 KiB and 64 KiB are served); `send_pacer_closes_a_slow_reader_after_grace` (R = 1 MiB/s; round 3c: the honest reader's send time ≤ 2 s) | no-set mutant: read-back 3,939,840 ≠ 65,536, caught (`evidence/rpcguard-integration/mutant-no-set.log`); round 3: I1, I2, R3-m6-READBACK-UNCHECKED, R3-m6-SHARED-COUNTER, R3-m6-NO-UPPER-BOUND caught; I3b survived on Linux in round 3 and is caught in round 3c (throughput, §13) |

## 2. Classification (ب) and response limits (ج)

| spec item | pocold mechanism | status | test | mutant |
|---|---|---|---|---|
| Light table (LIGHT_RESP_MAX 512 KiB). An unlisted method is −32601 before admission; exceeding the limit while encoding is −32020 {resultBytes} with a clean release (network.md:210) | `LIGHT_METHODS` + `is_light`. **Round 3 (review RG2-M4), conformance with network.md:210 and the error list (network.md:363, where −32020's reasons include resultBytes and −32008 does not appear):** a light answer over LIGHT_RESP_MAX is −32020 {resultBytes}; the reservation shrinks to the error and is released with the reply. Before round 3 it was −32008. Neither the extension (prototypes/pocol-extension: no −32008 anywhere) nor any test or tool depended on −32008; the one unit test that asserted it now asserts −32020 {resultBytes}. **Not as specified:** pocold serves methods outside the table, on the heavy lane: net_listening, net_peerCount, eth_syncing, eth_mining, eth_accounts, pocol_status, pocol_getBlockRlp, pocol_getMiners, pocold_rpcCounters (pocold's tools and tests use pocol_status, pocol_getBlockRlp, pocol_getMiners and pocold_rpcCounters). A method pocold does not know is −32601 after admission, not before; so are the listed methods it does not implement (eth_getProof; pocol_getTiming, pocol_getTarget, pocol_getCaps, pocol_syncStatus, pocol_getPot, pocol_getSettlement, pocol_getBurnTotals, pocol_getEscrow, pocol_getRetention, pocol_getClosing, pocol_getAssignment, pocol_getMinerSet; pocol_submitShare is not an RPC method in pocold). Not changed: the decision is proposed (proposals/RPCGUARD-R2-PROPOSALS.md P4). **Round 3c (review RG3V-M3), conformance with network.md (ب) and :210 (the limit is the class's, and the class is the method's):** round 3 applied LIGHT_RESP_MAX only on the light lane, so a light method whose body was over LIGHT_BODY_MAX (heavy lane) was limited by RESP_MAX only, and RG12's own scenario (an eth_blockNumber answer of 600 KiB, its id inflating it) was answered with a result over the socket. `member` now applies the light limit to every light-table entry's answer, alone or as a batch member, whatever its lane; the heavy lane's RESP_MAX reservation is kept. **Round 3c (review RG3V-m1), conformance with network.md (ب):** the conditional light entries (eth_getBlockBy* with hashes only, ≈ 160 KiB; pocol_getHeaders with count ≤ 64, ≈ 385 KiB) are of the light class for the deadline and the answer limit, read from params[1] as their handlers read it (`light_class`, a borrowing probe that allocates nothing in proportion to the body, and `light_value` for parsed members; a rpc unit test checks they agree). **Not as specified:** they still run on the heavy lane, with a RESP_MAX reservation (conservative); `is_light` (the lane) is unchanged | PARTIAL: the byte bound and the overflow answer as specified, on either lane (round 3c); the unlisted-method rule not enforced (P4); the conditional entries' lane is the heavy one | `an_answer_over_its_limit_is_32020_result_bytes_with_a_clean_release` (RG12-equivalent, in process: an eth_blockNumber answer inflated past LIGHT_RESP_MAX by a 600 KiB id on the light lane is −32020 {resultBytes}, the id elided, lightResv 0 after the reply; within the limit it is answered); round 3c: `a_light_method_answer_over_light_resp_max_is_32020_on_the_heavy_lane` (sockets: the same 600 KiB-id answer on the heavy lane, alone and as a batch member, −32020 {resultBytes} with the id elided; a 400 KiB id is answered; pocol_status with a 600 KiB id has RESP_MAX; no reservation left), rpc unit test `the_class_probe_agrees_with_the_parsed_class`; `a_max_size_transaction_is_answered_in_the_light_lane`, rpc unit tests | R3-M4-32008, R3C-CLASS-LIMIT-*, R3C-COND-*, R3C-PROBE-*, R3C-VALUE-NONARRAY-LIGHT; R3-M4-NO-CLASS-LIMIT is equivalent since round 3c (§9) |
| Heavy table (RESP_MAX 8 MiB) | Heavy by default. `capped` returns −32020 {resultBytes} over RESP_MAX (round 3; −32008 before), with the id echoed only while its encoding is ≤ 128 B, so an inflating id cannot inflate the error. The unlisted-method rule as in the light row | PARTIAL (as the light row) | `responses_above_resp_max_become_an_error`, `an_answer_over_its_limit_…` (heavy: an answer past RESP_MAX, heavyResv 0 after) | R3-M4-32008 |
| RPC_CONN_MAX = 32; the excess is closed unread | `conn_max`; the acceptor answers 503 and closes without reading | ENFORCED | `rpc_connection_cap_refuses_excess_unread_and_keeps_serving` | — |
| HEAVY_ACTIVE = 4 | One consensus thread, so active heavy work ≤ 1 ≤ 4 (`heavyActive` gauge) | EQUIVALENT (stronger); reported as 1 | `queued_requests_are_busy_at_queue_wait` (holder detected by the gauge) | — |
| LIGHT_ACTIVE = 16 | The one consensus thread drains the light lane, so active light work ≤ 1 ≤ 16 (`lightActive` gauge) | EQUIVALENT (stronger); reported as 1 | `get_params_and_rpc_stats_report_the_limits` (lightActive = 1 while it runs) | — |
| HEAVY_QUEUE = 16 | `reserve_heavy`: at most `heavy_queue` places, waited for until QUEUE_WAIT. An abandoned body keeps its place until the node reaches it | ENFORCED | `abandoned_queued_bodies_are_capped_and_released` | RGM3 |
| LIGHT_QUEUE = 32 | The light lane's capacity. A full lane is −32021 {busy} (HTTP 200) | ENFORCED | `a_full_light_lane_answers_busy` | V3 |
| QUEUE_WAIT = 2 s: admitted within it, otherwise −32021 {busy, retryAfterMs ∈ [250, 2000]} (network.md:234) | **Round 2.** QUEUE_WAIT runs from the end of parsing, across the wait for a place (or a full main queue) and the wait in the queue. A shared admission state (`Request::state`: QUEUED, ADMITTED or GONE) makes it atomic. At QUEUE_WAIT the connection marks the request gone and answers busy itself. The consensus thread admits a request only while it is QUEUED and younger than QUEUE_WAIT. So a request answered busy never runs, and an admitted one is always answered. Applies to heavy and light requests. **Round 3 (review RG2-M3):** each of the three waits now has its own committed test, and so does the node-side compare-and-swap | ENFORCED | `queued_requests_are_busy_at_queue_wait` (sockets: a heavy and a light request behind a 0.5 s precompile run are busy at QUEUE_WAIT, about 83 ms), `compute_deadline_runs_from_admission_and_queue_wait_bounds_admission` (in process: one queued past QUEUE_WAIT is busy and its transaction is not pooled), `heavy_queue_overflow_answers_busy_with_retry_after`; round 3: `queue_wait_bounds_the_wait_for_a_heavy_queue_place` (N1, W4: HEAVY_QUEUE = 1, the place waiter is busy at QUEUE_WAIT, 72–75 ms in three runs, not when the place frees, after the holder's three BLAKE2F runs of 434–451 ms, about 1.3 s), `queue_wait_bounds_the_wait_on_a_full_main_queue` (N2: 150 heavy requests behind a holder, started 1 ms apart, 23 of them found the 128-event main queue full; all busy at QUEUE_WAIT, worst 121–144 ms at QUEUE_WAIT 106–128 ms in 6 runs, while the queue drains only after the holder's three BLAKE2F runs of 424–513 ms, about 1.3–1.5 s), `the_node_never_runs_a_request_its_connection_gave_up` (N6, W10: a request its connection marked gone is never run, deterministically; and 1000 in-process races of the two compare-and-swaps per run: in three runs 425–433 admitted and answered, 470–472 gone at the connection and skipped, 95–103 gone at the node and answered busy, 0 violations). **Limit of that test (review RG3V-m4):** it pins that the node checks the state, not that the check and the set are one atomic step: a load-then-store survives its 1000 races (RV3-CAS-CHECK-THEN-SET), because the window between the two is nanoseconds and no test hook can sit inside one `compare_exchange`. Atomicity holds by construction (one `compare_exchange` on each side) | V1a, V1b, N1, N2, N5, N6; RV3-CAS-CHECK-THEN-SET survives (a test limit, §13) |
| FIFO queues; an honest request waits behind ≤ 31 requests | Heavy requests are FIFO in the main event queue. The light lane is drained before every main event and between batch members. Light requests therefore overtake heavy ones; heavy requests do not interleave between a batch's members (each member does not re-queue). Every wait is bounded by QUEUE_WAIT | EQUIVALENT for the wait bound (QUEUE_WAIT); the ordering differs (documented) | `a_light_turn_runs_at_most_light_turn_requests_then_wakes_the_node`, `legitimate_requests_progress_under_bounded_heavy_load` | — |
| Token bucket per connection, 20/s, capacity 20; k tokens at parse; shortfall −32021 {rate, retryAfterMs} (network.md:215–218) | **Round 2.** Stage A's per-source-IP bucket is removed: it was not the spec's mechanism, it was evadable through 127/8 addresses, it grouped honest tools, and its refill was truncated below 1 ms. The bucket now applies exactly as specified per connection, to the connection's one request (`rate_refused`): a full bucket of `bucket_cap`, k tokens taken after the BATCH_MAX check. At the spec values (20/s, capacity 20 = BATCH_MAX) no request within BATCH_MAX is refused; that is the per-connection bound, met because each connection carries one request. Neither the spec nor pocold bounds the aggregate rate across reconnects (see proposals/RPCGUARD-R2-PROPOSALS.md P1). **Round 3 (review m1):** the `rate` fact (20) is vacuous per connection: at the running values nothing reads it (a connection's one request takes ≤ BATCH_MAX = bucket_cap tokens from a full bucket, so no refill and no retryAfterMs is ever computed); a client opening a connection per request at 5,520 req/s was never rate-limited (review W16). The admission notes now say so (`pocold::RPC_EQUIVALENT`: "R26 rate: equivalent, vacuous per connection …"), with the other equivalent facts | EQUIVALENT (per connection, as specified, on its one request; `rate` vacuous there, noted in the admission); aggregate bound open (P1, proposed) | `the_per_connection_bucket_charges_k_tokens_per_request` (capacity 5: 6 members are 6 rate items with retryAfterMs = 50, 5 members run, 21 members are −32600), rpc unit test `the_per_connection_bucket_refuses_only_more_tokens_than_its_capacity` | RGM2, V6 |
| HEAVY_RESP_MEM = 48 MiB | `HEAVY_RESP_POOL` = 48 MiB. A full pool is −32021 busy before anything runs | ENFORCED | `a_full_heavy_pool_answers_busy_before_anything_runs` | existing M4-BUSY |
| LIGHT_RESP_MEM = 16 MiB = 32·512 KiB, "so light reservations never run out (S1)" (network.md (ج)) | Light pool = (conn_max+1)·512 KiB = 17,301,504 B (16.5 MiB): one LIGHT_RESP_MAX per connection, as the spec's 32·512 KiB, plus one for the light request the consensus thread may be computing for a connection that already left (its reply is then dropped unread, and its reservation with it). So a light reservation never fails, which is the property the spec's budget exists for, with 512 KiB more than the spec's 16 MiB; the fact is reported at its running value (17,301,504) and the extra 512 KiB is in the A13 term. **Round 3 (review m8):** stage A and round 2 called this "ENFORCED (≥ spec)", which overstated it: the spec's 16 MiB budget itself is exceeded | EQUIVALENT (the S1 property, at 512 KiB above the spec's budget; noted in the admission) | `response_pools_reserve_shrink_and_release` (conn_max + 1 light reservations succeed, one more fails) | — |
| SCRATCH ≤ 16 MiB per heavy permit, −32020 {scratch} | eth_getLogs and pocol_getLogs charge the bytes they materialize against `scratch_heavy`. An overflow is −32020 {scratch} with resume data, and the partial result is dropped. No other handler is charged: not eth_call/eth_estimateGas EVM memory and state copy (bounded by gas through the A13 execution model, not by 16 MiB), and not the JSON trees of the other heavy methods | PARTIAL (log handlers only): `scratch_heavy` absent; rpcLimits.scratchHeavy = null, rpcLimits.logsScratch = 16 MiB | `get_logs_scratch_limit_is_32020_scratch_with_resume_data` | RGM6 |
| SCRATCH ≤ 1 MiB per light permit | No light handler is charged. `scratchLightMax` is null in pocol_getRpcStats | NOT ENFORCED (`scratch_light` absent) | — | — |
| REQ_BUF = 1 MiB | Request line + headers + body ≤ REQ_BUF | ENFORCED | `rpc_request_buffer_covers_headers_and_body` | — |
| CONN_BUF = 64 KiB (the per-connection buffer a batch's k error items are written from) | A busy or rate answer is at most BATCH_MAX error items of ≤ ERR_ITEM_MAX each: ≤ SMALL_REPLY_MAX = 5184 B. Connection threads build it by scanning the ids (`scan_ids`), never by parsing a tree. Reported as conn_buf = 5184, the structural bound, not a configured value. **Round 3 (review m2):** the heap a connection thread takes to build it was not 5184 B: round 2 kept each echoed id as its parsed tree, up to 340,378 B for 20 members whose ids nest 25 objects (scratch measurement on fa97594's busy path, `evidence/rpcguard-r3/busy-path-peaks.log`; the review measured 24,383 B for 128-byte string ids). Ids are now kept as their encodings (≤ 128 B, `id_encoding`), and an id is parsed only if it can fit (≤ 1 KiB raw, ≤ 64 containers, ≤ 258 values; no id that would be echoed is dropped, and the answers equal the parse's). The heap is bounded by SMALL_REPLY_PEAK = 132,768 B (kept ids, the answer, its header and one transient id parse, ID_PARSE_PEAK = 124,032 B, derived in rpc.rs); the worst measured over 13 adversarial id shapes, singleton and 20-member batch, is 47,627 B. A13 counts conn_max × SMALL_REPLY_PEAK | EQUIVALENT (stronger: 5184 B ≤ 64 KiB) | rpc unit tests `error_items_fit_err_item_max_and_a_batch_of_them_small_reply_max`, `a_busy_answer_allocates_no_parse_tree` (round 3: every adversarial shape within SMALL_REPLY_PEAK), `busy_answers_scan_the_ids_a_parse_would_echo` (round 3: the adversarial shapes too) | RGM11, R3-m2-SMALL-TERM; R3-m2-NO-PARSE-FILTER survives (§10a) |
| RPC_MEM_TOTAL = 178 MiB ≤ 192 MiB | pocold bounds RPC memory inside its own A13 budget (rpc term 208.5 MiB at the round-3 code, §8) | NOT ENFORCED as the field (`mem_total` absent) | `a13_boot_refuses_a_bound_above_the_memory_budget` | — |
| Encoding into 64 KiB segments inside the reservation | The answer String is built within the reservation and shrunk to fit; it is not chunked | EQUIVALENT for memory (≤ reservation); the chunked form is not enforced | `an_answer_is_reserved_until_its_connection_has_written_it` | — |
| Snapshots ≤ 20 (one per permit) | Handlers read the chain on the one consensus thread. eth_call/estimateGas take one state copy at a time (the A13 transient working-copy term). `snapshotsOpen` is null | EQUIVALENT (≤ 1 working copy at a time) | A13 tests | — |

## 3. Request ownership, COMPUTE_DEADLINE and cancellation (د, و)

| spec item | pocold mechanism | status | test | mutant |
|---|---|---|---|---|
| REQ_READ_DEADLINE = 10 s | `read_deadline` over the whole request | ENFORCED | `rpc_connection_cap_…` | — |
| IDLE = 30 s | The connection closes after one response, so there is no idle period | EQUIVALENT | — | — |
| Stage 2 order: F0, origin deny, classification, rate, numeric log range, within REQ_BUF | The rate check runs on the connection thread, after the BATCH_MAX check and before ORIGIN_DENY (which is applied per member on the consensus thread). The order is observable only for a batch of k > bucket_cap, unreachable at the spec values. The numeric log range (≤ 1024) is checked in the handler, after admission | PARTIAL (order; documented) | — | — |
| Atomic admission: permit and the class's reservation together, for the head of the queue; a waiting request holds nothing | The consensus thread admits (`Request::admit`) and reserves (`RespPools::take`) in one step at dequeue. A waiting request holds only its body (the A13 queued-body terms) | EQUIVALENT | `compute_deadline_runs_from_admission_…` | V1b |
| COMPUTE_DEADLINE 5 s heavy / 500 ms light, **from admission**, including encoding; expiry is −32020 {deadline} (network.md:235) | **Round 2.** `Budget.compute_by` = admission instant + the class's deadline; time spent queued never counts (stage A measured from arrival, review B1). Checked: at handler entry (except the log handlers, which check at their first block so the error carries resume data); inside long loops (eth_getLogs every 16 blocks; eth_call/estimateGas before every simulation and through the stop hook); and once the answer is encoded. **Round 3 (review RG2-M1):** any answer completed after the deadline, a result or an error, is replaced by −32020 {deadline} (`member`; the log handlers' with resume data). Round 2 replaced results only, so a read-only eth_call whose precompile run outlasted the deadline and then reverted or halted was answered with its late error (code 3 or −32000, 435–484 ms after a 20 ms deadline, review W1). Kept unchanged: a −32020 {deadline} the handler produced itself (it carries its own resume data and is counted once), and the answers of methods whose effect already happened (eth_sendRawTransaction, pocol_submitShare), which are checked only before they run: a late −32020 would hide an accepted transaction. That is the running reading of the text pending a decision (proposals/RPCGUARD-R2-PROPOSALS.md P3, option A). **Round 3 (review m5):** the deadline follows the method's class (`light_method`, the classification table), as batch members' already did: a light method whose body exceeds LIGHT_BODY_MAX runs on the heavy lane, with a RESP_MAX reservation (conservative), under the light deadline. Batch members get their own class's deadline from when they start. **Round 3c (review RG3V-m1):** the conditional light entries (§2 light row) get the light deadline too, alone and as batch members. **Round 3c (review RG3V-m3):** "including encoding" and the late replacement of a light answer were pinned by no test (moving the check before encoding, or exempting light methods from it, survived); the `ENCODE_DELAY_MS` test hook (0 in the node; it holds the encoded answer of one test id) now makes a handler that ends in time but encodes past its deadline, and the test requires −32020 {deadline} for a heavy method, a light method, a conditional entry and a batch member | ENFORCED (the answer); the cancellation latency is below | `compute_deadline_runs_from_admission_and_queue_wait_bounds_admission`, `light_requests_behind_long_heavy_work_are_answered_never_deadline` (default limits: a light request behind a 1.2 s holder gets its result), `light_compute_deadline_fires`, `a_request_reaching_its_handler_past_the_deadline_never_runs`, `heavy_compute_deadline_cancels_eth_call`, `a_precompile_call_past_its_deadline_is_32020_not_a_late_success`, `estimate_gas_stops_its_search_at_the_deadline`; round 3: `a_late_revert_or_halt_past_its_deadline_is_32020` (W1: BLAKE2F of 28M rounds then REVERT, or INVALID; −32020 {deadline} at 402–492 ms over three runs with a 20 ms deadline; with a wide deadline the same calls answer code 3 and −32000), `a_light_method_over_light_body_max_has_the_light_deadline`, `batch_members_each_get_their_own_compute_deadline` (N9, W13: three uninterruptible members of 167–188 ms each, under a heavy deadline of twice that, 335–376 ms, all answer their results; the batch took 512–564 ms); round 3c: `the_conditional_light_entries_have_the_light_deadline` (light deadline zero: eth_getBlockBy* with false or no full flag, pocol_getHeaders with count 64 or "0x40", a batch member, and eth_blockNumber with a 4 KiB id are −32020 {deadline}; full blocks and count 65 are answered), `the_compute_deadline_includes_encoding` (deadlines 300 ms, encoding held 700 ms) | RGM9, RGM4, RGM5, RGM8, V7, N9, N11, R3-M1-RESULTS-ONLY, R3-M1-OWN-DEADLINE-REPLACED, R3-m5-DEADLINE-BY-LANE; round 3c: RV3-LATE-CHECK-BEFORE-ENCODING, RV3-LATE-HEAVY-ONLY, RV3-M5-PAD-ONLY, R3C-BATCH-MEMBER-LIST-ONLY; R3-P3-NO-EFFECTFUL-EXEMPTION survives: no committed test pins the P3 reading (§10a) |
| Cancellation checks every 16 blocks scanned, every 1 MiB encoded, every 10^6 gas; release within CANCEL_MAX = 100 ms (network.md:286) | 16 blocks: eth_getLogs, from its first block. 10^6 gas: `native::STOP_GAS`; the stop hook is polled at the first JUMP/JUMPI or data-sized instruction (KECCAK256, *COPY, LOG) once a frame has spent 10^6 gas since the last poll, at every 64th JUMP/JUMPI, and before every CALL/CREATE-family instruction. Not enforced: 1 MiB encoded (encoding is not chunked; it is checked once, after) and precompiles. A precompile call is not interpreted, so it runs to its end: a 29.9M-round BLAKE2F (30M gas) took about 0.5 s, and its −32020 came 480–569 ms after a 20 ms deadline. `cancelMaxMs` reports the largest observed lateness | PARTIAL: the 16-block and 10^6-gas checks are enforced; CANCEL_MAX itself is NOT ENFORCED (`cancel_max_ms` absent; proposals P2) | lp4 `the_stop_hook_is_polled_at_least_every_stop_gas` (29 polls for 30M gas), `a_stop_ends_the_simulation`, `a_precompile_call_is_never_polled`; consensus `get_logs_scratch_…` (deadline part: the scan stops at its first block) | RGM10, V5 |
| End of compute: release snapshot, SCRATCH and permit; shrink the reservation | The `Budget`, the state copy and the `Active` gauge drop at return; the reservation shrinks to the answer | ENFORCED | `a_full_heavy_pool_…` | — |

## 4. SendPacer (د.6, D90/D92/D93/D94)

| spec item | pocold mechanism | status | test | mutant |
|---|---|---|---|---|
| Binding rule: close if sendTime > GRACE and A < R·sendTime (R = 65536 B/s, GRACE = 2 s) | `pace_write`: **round 2, a non-blocking socket** (review M5). A write that would block sends nothing on every platform; there is no SO_SNDTIMEO, so there is no indeterminate Windows state. The rule is evaluated with the exact values after every write attempt. A response of `size` bytes holds its reservation at most SEND_MAX(size) | ENFORCED | `send_pacer_closes_a_slow_reader_after_grace` (a slow reader is closed after GRACE; an honest reader gets every byte, Content-Length checked and the JSON complete) | RGM1 |
| Timer at s_v + TIMER_EPS (1 ms), re-armed on every change of A, backstop check every 50 ms, exact re-check when it fires | **Round 3 (review RG2-M2).** A blocked write waits for the socket to become writable (`rpc::sockwait`: poll(2) on Unix, WSAPoll on Windows; the socket is already non-blocking), for at most min(PACER_BACKSTOP = 50 ms, s_v + TIMER_EPS − sendTime, the BATCH_WALL instant), s_v = max(GRACE, A/R); after every wake the next write is attempted and the rule re-checked with the exact values. The timer is thus at s_v + TIMER_EPS (rounded up to the next ms), re-armed on every change of A, and the backstop is the spec's 50 ms. Round 2 retried every `pacer_poll` = 1 ms whether or not the socket could take data, so CPU and throughput were both set by the poll interval (§4a); `pacer_poll` is removed. Each blocked wait is counted (`pacerWaits` in pocold_rpcCounters) **Round 3c (review RG3V-M1, RG3V-m2):** the only committed bound used a client whose A/R stayed below GRACE, so the A/R term of s_v was pinned by no test; without it every block past GRACE is a zero wait (the review measured 0.244 cores and 332,465 waits/s per slow reader). And the backstop was pinned by nothing. Both are now tested | ENFORCED (`timer_eps_ms` = 1 present) | `send_pacer_…` (round 3: a client that does not read costs ≤ 2·GRACE/PACER_BACKSTOP + 10 waits until its close, 10 measured; lateness 1 ms; round 3c: and at least GRACE/PACER_BACKSTOP/2 = 5 waits, the backstop); round 3c: `send_pacer_timer_follows_a_over_r_past_grace` (R = 256 KiB/s, GRACE = 200 ms, a client reading 64 KiB every 100 ms: over 1.5 s past GRACE at most 2·(window/PACER_BACKSTOP) + 1 per KiB read + 10 waits, no close; 36 waits measured in each of three runs against a bound of 1,094; then, once it stops reading, CloseOK at sendTime ≈ A/R, 5.4–6.4 s, lateness 1–2 ms, asserted ≤ 200 ms); rpc unit test `sockwait_reports_readiness_within_its_timeout` | V2, R3-M2-SLEEP-POLL-1MS, R3-SOCKWAIT-ROUND-DOWN, R3-SOCKWAIT-EVENTS-SWAPPED; round 3c: RV3-SV-GRACE-ONLY, RV3-NO-BACKSTOP, R3C-WAIT-OUT-BACKSTOP; R3-M2-NO-WALL-IN-WAIT and R3-M2-BACKSTOP-ONLY survive (§10a, §10b) |
| lateness ≤ ENFORCE_SLACK = 200 ms, recorded in pacerLateMaxMs | Recorded at every pacer close. The test asserts ≤ 200 ms; measured 0 ms in round 2, 1 ms in round 3 (the wait ends at the next whole ms) | ENFORCED (as a measured, asserted bound) | `send_pacer_…` | V2 |
| CloseOK record (sendTime, A) in RpcStats | `lastPacerClose {sendTime (s), accepted}` | ENFORCED | `send_pacer_…` (sendTime > GRACE, A < R·sendTime) | — |
| sendDone releases the reservation; sendOverrunMaxMs | `pace_write` returns at A = size. The `Reply` reservation drops; `sendDoneCount` and `sendOverrunMaxMs` (sendTime − (GRACE + size/R)) are recorded | ENFORCED | `send_pacer_…`, `an_answer_is_reserved_…` | existing M13-RESV |
| PacerTrace (D93): 4 MiB ring of produce/accept/pend/close/sendDone events; traceDropped | Not kept. Only close accounting is kept. `traceDropped` is null | NOT ENFORCED | — | — |

### 4a. SendPacer cost and throughput, before and after round 3 (review RG2-M2)

Measured with the gitignored witness `tests/zz_audit_witness_rpcguard_r3.rs` (its two tests, M2 and
M3, modelled on the review's W2 and W3), release build, under the shared CPU lock, loopback, this
Linux host. Before: fa97594 (the 1 ms poll). After: the round-3 code. CPU is the process's user +
system time (getrusage) over the window, less an idle window; it includes the test's own client
threads.

- **Honest throughput (M2):** one client reads a 14,681,122 B answer (10-member batch) as fast as it
  can, 5 runs per process; a raw loopback copy of the same size (default buffers, blocking
  write_all) is the reference.
- **CPU per blocked connection (M3):** 16 clients that do not read their ~2 MiB answers, during
  1.5 s after the answers are built (the pacer closes each at about 3.5 s at the defaults); then 16
  clients reading 5 KiB every 50 ms (100 KiB/s, just above MIN_SEND_RATE: the pacer keeps them), over
  2 s.

Three runs each (`evidence/rpcguard-r3/pacer-measure-before-fa97594.log`,
`evidence/rpcguard-r3/pacer-measure-after-1530d7f.log`, `evidence/rpcguard-r3/pacer-measure-after-d0e87f9.log`;
the same witness, sha256 580734f8…). d0e87f9 is the final tree; its src/ differs from 1530d7f's only in
one doc comment (the backstop's line citation), and it was measured again in the continuation:

| quantity | before: fa97594 (sleep 1 ms) | after: 1530d7f (poll/WSAPoll, 50 ms backstop) | final: d0e87f9 |
|---|---|---|---|
| honest reader, median of 5 per run | 57.9, 87.8, 58.2 MiB/s | 1,823, 1,571, 1,630 MiB/s | 1,842, 1,911, 1,885 MiB/s |
| raw loopback copy, same host and runs | 2,446–3,016 MiB/s | 2,288–2,510 MiB/s | 2,647–3,114 MiB/s |
| CPU per non-reading connection | 0.0125, 0.0137, 0.0123 cores | 0.00068, 0.00054, 0.00057 cores | 0.00042, 0.00045, 0.00056 cores |
| the same × RPC_CONN_MAX (32) | 0.39–0.44 cores | 0.017–0.022 cores | 0.013–0.018 cores |
| CPU per slow reader (100 KiB/s) | 0.0137, 0.0126, 0.0131 cores | 0.0012, 0.0013, 0.0011 cores | 0.0011, 0.0010, 0.0019 cores |
| idle process | 0.0048–0.0054 cores | 0.0035–0.0040 cores | 0.0034–0.0036 cores |
| pacer closes: 16 non-readers / 16 slow readers | 16 / 0 every run | 16 / 0 every run | 16 / 0 every run |

So, between fa97594 and 1530d7f, the honest throughput rose 18 to 31 times and is now 64–80% of a raw
loopback copy, and a blocked connection costs 18 to 25 times less CPU (a slow reader 10 to 12 times
less: its pacer still wakes for every chunk the client frees). The final tree's three runs are of the same
order: the honest reader at 61–71% of the same run's raw copy (which was faster in these runs), a
non-reading connection at 0.00042–0.00056 cores, and a slow reader at 0.0010–0.0019 cores (one run
above the 1530d7f range). The pacer's verdicts are unchanged: every
non-reader is closed, no slow reader above MIN_SEND_RATE is. The review's W2 and W3 on 273e785 (code
identical to fa97594) measured the same "before" (57–59 MiB/s; 0.013 cores per blocked
connection). These are one Linux host's numbers, not bounds; Windows was not measured.

The write cap stays (at most SEND_SOCK_BUF per call). In the standalone bench of a non-blocking
writer with SO_SNDBUF = 32 KiB and TCP_NODELAY (`evidence/rpcguard-r3/pollbench.log`), the cap is what
keeps the poll-based writer fast on Linux: uncapped writes send one 64 KiB segment at a time and each
waits for a delayed ACK (1.8 MiB/s, whether the writer polls or sleeps).

## 5. Batches (هـ)

| spec item | pocold mechanism | status | test | mutant |
|---|---|---|---|---|
| ≤ BATCH_MAX = 20 members | A larger batch is −32600 (before the bucket) | ENFORCED | `rpc_batches_…`, `the_per_connection_bucket_…` | — |
| Streamed with chunked encoding; member k+1 admitted once member k is accepted | The batch answer is built within BATCH_BYTES_HARD, then written under one pacer. Members run one at a time on the one consensus thread, each with its own class's COMPUTE_DEADLINE | EQUIVALENT for memory and per-member deadlines; streaming NOT ENFORCED | `an_answer_is_reserved_…`, `rpc_work_deadline_…` | — |
| One SendPacer per batch, GRACE once, the rule on the cumulative A | One `pace_write` per batch answer | ENFORCED | `send_pacer_…` (a 10-member batch) | RGM1 |
| BATCH_RESP_MAX = 16 MiB; the member crossing it and every later one are −32020 {batchBytes} | `batch` | ENFORCED | `batch_members_beyond_the_byte_limit_are_not_run` | — |
| BATCH_WALL = 420 s from read-end to sendDone; reaching it closes, releases and counts batchWallHits; any nonzero value in a release run is a defect | **Round 2** (review m5). Measured from the read-end through compute and send: between members (the batch stops and the connection closes without an answer) and in `pace_write` (the send is cut). Derivation at the defaults: read-end to sendDone ≤ QUEUE_WAIT 2 s + work deadline 30 s (after which the connection answers 504 and no batch answer is sent) + SEND_MAX(BATCH_BYTES_HARD) 258.3 s = 290.3 s < 420 s, so a release run never reaches it. Round 3: the pacer's wait also ends at the BATCH_WALL instant, so a send is cut at the wall, not up to a backstop later | ENFORCED (backstop, with the derivation) | `batch_wall_closes_the_batch` (compute phase and send phase); round 3: `batch_wall_counts_the_time_queued` (N7, W12: a batch that waited behind a holder past BATCH_WALL is closed without running when the node reaches it) | V4a, V4b, N7 |
| busy/rate items written; order kept; ids copied | Per-member items. Ids are echoed when their encoding is ≤ 128 B | ENFORCED | `a_full_heavy_pool_…`, `the_per_connection_bucket_…` | — |

## 6. eth_getLogs and pocol_getLogs (ز, ح)

| spec item | pocold mechanism | status | test | mutant |
|---|---|---|---|---|
| P1 per-block limits with resume data | `logs` `stop` (existing) | ENFORCED | `get_logs_limits_answer_32020_with_resume_data` | — |
| Deadline / SCRATCH: −32020 {deadline\|scratch, fromBlock, lastCompleteBlock ≥ fromBlock−1, nextFromBlock}; the partial buffer dropped | **Round 2** (review M3). The log handlers check the deadline at their first block, not at a generic entry check, so a deadline at entry carries resume data. An answer completed but late carries the resume data of no progress. lastCompleteBlock = fromBlock−1 is encoded null, as for a limit at fromBlock. A handler's own −32020 {deadline} (at a later block, with its progress) is kept as it is (round 3) | ENFORCED | `get_logs_scratch_…` (eth_getLogs, pocol_getLogs numeric and blockHash); round 3: `a_late_get_logs_answer_carries_resume_data` (N8: a query whose range starts past the head completes without reaching a block check, past a zero deadline; eth_getLogs and pocol_getLogs answer −32020 {deadline, fromBlock, lastCompleteBlock: null, nextFromBlock = fromBlock}) | V5, RGM6, N8 |
| Numeric range ≤ 1024 before admission, −32020 {range, maxRange} | Checked in the handler after admission; the error carries no data | NOT ENFORCED as specified (pre-existing; open) | — | — |
| pocol_getLogs(filter, anchorHash): −32022 {anchorNotCanonical, head} / {beyondAnchor} | pocold's pocol_getLogs takes no anchorHash. `anchorRejects` is null | NOT IMPLEMENTED (open; out of this round) | — | — |
| ORIGIN_DENY (ح2) | Existing, per member. `originDenied` is counted | ENFORCED | `a10_origin_deny_applies_to_every_batch_member` | — |
| (ط) RPC separated from consensus (its own runtime) | RPC runs on the consensus thread. This is why active ≤ 1 and why light requests wait behind consensus work; the wait is bounded by QUEUE_WAIT | NOT ENFORCED (architecture) | — | — |

## 7. Stats and parameters

| spec item | pocold mechanism | status | test | mutant |
|---|---|---|---|---|
| pocol_getParams → {profileKind, nodeKind, nonceMode, chainId, genesisHash, genesisPre, netKey, forkSchedule, activeVersion, rpcLimits} (network.md:336) | Every field (round 2 adds `netKey` = keccak256(RLP(['PoCol-net-v1', chainId, genesisHash])), checked against the extension's derivation, and `forkSchedule`). rpcLimits holds the running values (sendSockBuf = 32768 since the integration), and null for SCRATCH, CANCEL_MAX and RPC_MEM_TOTAL | ENFORCED | `get_params_and_rpc_stats_report_the_limits`, rpc unit test `net_key_matches_the_extension` | RGM7 |
| pocol_getRpcStats → the 27-field record (network.md:347) | Every field. Null for the mechanisms pocold does not have: scratchLightMax, snapshotsOpen, anchorRejects, traceDropped. sendOverrunMaxMs is null until the first sendDone. pocold's own counters (also in pocold_rpcCounters, outside the spec record) gain `sockbufRefused` and `pacerWaits` in round 3 | ENFORCED (the record; the four null fields are not tracked) | `get_params_and_rpc_stats_report_the_limits`, `a_connection_whose_send_buffer_cannot_be_set_is_refused` (sockbufRefused) | — |

## 8. Admission (R26) and the A13 RPC memory term

`pocold::RPC_ENFORCED` names 25 fields at integration (24 in round 2, plus send_sock_buf), at their running values:

- conn_max, req_buf, conn_buf (5184), send_sock_buf (32768), heavy_active (1), light_active (1), light_queue;
- heavy_resp_mem, light_resp_mem, resp_max, light_resp_max, batch_resp_max, batch_max, err_item_max;
- logs_count_max, logs_scan_bytes, rate (20; equivalent, vacuous per connection), bucket_cap (20), queue_wait_ms (2000), compute_heavy_ms (5000);
- grace_ms, min_send_rate, batch_wall_ms, enforce_slack_ms, timer_eps_ms (1).

Four fields are absent, never nominal (send_sock_buf was the fifth until the integration):

- `scratch_heavy` and `scratch_light` (only the log handlers are charged);
- `mem_total`;
- `cancel_max_ms` (precompile runs are uninterruptible).

Under the experimental-devnet scope, 15 of the 18 R26 predicates read only present fields; they are
checked and all pass, `sendSockBuf<=64KiB` among them. Three are waived and reported:

- `derive` (reads scratch and cancel_max);
- `memoryRequired<=memTotal`;
- `batchWallMin<=batchWallMs` (derive).

The specification scope refuses (`specification:rpc-absent`). Test:
`pocold_admission_is_an_experimental_devnet_with_absent_rpc_mechanisms`.

**Round 3 (review RG2-M3 NF1, m1).** The admission test now checks all 25 facts, one row per
`RPC_ENFORCED` entry in order, against the limits the node runs with (`rpc::Limits::default()`,
which is `node::Limits::default().rpc`, and the rpc constants), written out in the test rather than
taken from `rpc_facts`: a fact reported at another value than the running one fails it (mutants NF1,
NF2, NF3). Six of the facts hold by an equivalent bound, not by the spec's mechanism; they are
listed in `pocold::RPC_EQUIVALENT`, and `admission_json` adds one admission note per fact ("R26
<field>: equivalent, <why>"): heavy_active and light_active (1, the single consensus thread),
conn_buf (SMALL_REPLY_MAX), light_resp_mem (S1, 512 KiB above the spec's budget), rate (vacuous per
connection) and bucket_cap (the per-connection bucket on the connection's one request). The test
checks the notes.

Stage A's 26 facts carried `scratch_heavy`, `scratch_light`, `cancel_max_ms` and a nominal
`conn_buf`; those were not truly enforced (review B4/M4). It omitted `timer_eps_ms`, which is now
implemented.

A13 (`pocold::resources::derive`, rpc term), at the round-2 code:

- the per-source bucket map term is removed (−233,472 B);
- busy and rate answers built on connection threads no longer parse the request into a tree. Stage
  A had added that parse on up to conn_max threads, which the model did not count;
- a queued body is shared with its connection through an `Arc` (never copied); the Arc header and
  the admission state fit the +256 B per queued body.

Round 3 (review m2): the small-answer term was conn_max × SMALL_REPLY_MAX (the answer's size); it is
now conn_max × SMALL_REPLY_PEAK, the heap a connection thread takes to build the answer (§2, CONN_BUF
row): +32 × (132,768 − 5,184) = +4,082,688 B.

| quantity | round 2 (fa97594) | round 3 |
|---|---|---|
| total | 5,577,343,580 B (5.194 GiB) | 5,581,426,268 B (5.198 GiB) |
| required budget with the 1/8 reserve | 6,374,106,949 B ≤ 6 GiB | 6,378,772,878 B ≤ 6 GiB (6,442,450,944 B) |
| rpc term | 214,496,448 B (204.6 MiB) | 218,579,136 B (208.5 MiB) |
| rpc/small answers outside the pools | 165,888 B (32 × 5,184) | 4,248,576 B (32 × 132,768) |

Tests: `a13_boot_refuses_a_bound_above_the_memory_budget`, `a13_bound_breakdown`. Round 3: the
breakdown test asserts the total (5,581,426,268 B) on Linux x86_64, where the evidence is produced
(two terms are type sizes, `Entry` and `Block`, so other targets only check the total against the
budget), and the small-answer term against conn_max × SMALL_REPLY_PEAK (mutant R3-m2-SMALL-TERM).

## 9. Review findings (round 2) and their resolution

| finding | resolution |
|---|---|
| B1 deadline from arrival (regression) | FIXED: from admission; regression tests in process and over sockets at the default limits; mutant RGM9 |
| B2 QUEUE_WAIT not bounding queued requests | FIXED: the shared admission state; busy at QUEUE_WAIT from the connection; the node never runs a request it did not admit in time; mutants V1a, V1b |
| B3 late precompile success; CANCEL_MAX | FIXED for the answer (post-encoding check, per-simulation check; mutant RGM8). The stop hook polls per 10^6 gas (RGM10). CANCEL_MAX: `cancel_max_ms` now absent (NOT ENFORCED; proposals P2) |
| B4 SCRATCH outside getLogs | DOWNGRADED: `scratch_heavy` PARTIAL and absent, `scratch_light` NOT ENFORCED and absent; rpcLimits null; `scratchLightMax` null |
| B5 flaky work-deadline test | FIXED: snapshot after the heavy call and the lane have drained (gauges). 3 consecutive runs, plus 21 runs under load (three test processes at once): 21/21 ok, `skipped` = `late` every time (evidence/rpcguard-r2-b5-stress-dc4e3d1.log) |
| M1 refill truncation | RESOLVED BY REMOVAL: no per-source bucket; the per-connection bucket keeps no time state |
| M2 per-IP not equivalent | FIXED: per-connection bucket as specified; the aggregate rate is proposed (P1), not implemented |
| M3 getLogs deadline without resume data | FIXED: first-block check in the handler; post-encoding resume data; mutant V5 |
| M4 nominal conn_buf / sendSockBuf | FIXED: conn_buf = SMALL_REPLY_MAX (structural, tested); sendSockBuf null; the unused Limits fields removed. *Superseded at integration (be95e26): SO_SNDBUF is set on every RPC connection and `send_sock_buf` = 32768 is an enforced fact; sendSockBuf reports the read-back (§1)* |
| M5 Windows SO_SNDTIMEO | FIXED: non-blocking writes; the honest-reader test compares the length and parses the JSON. Not run on Windows here. *Since then: Windows runs 9–13 (29fd44a to d40fd91; Rust code of 29fd44a and 0d6f9dd) passed the pocold suite (WINDOWS-VERIFICATION.md); round 3's waits (WSAPoll) have not run on Windows (§12)* |
| M6 survivors V1–V8 | Tests added. V1a/V1b/V2/V3/V4a/V4b/V5/V6/V7 caught; V8 equivalent (below) |
| m1 A13 doc numbers | Corrected in the proposed A13 text (doc_updates) |
| m2 bucket map undercount | RESOLVED BY REMOVAL |
| m3 SO_SNDBUF rationale | CORRECTED (§1) |
| m4 stats and params fields | FIXED: every field present (null where pocold has no such mechanism) |
| m5 BATCH_WALL semantics | FIXED: from read-end through the send, closes; derivation in §5 |
| m6 rate edge cases | FIXED: batch-size check before the bucket (21 members are −32600); no overflow (no time state). The order versus ORIGIN_DENY is documented (§3) |
| m7 verification gaps | The lp4-execution release suite runs at the end. e2e and Windows: see the evidence index |

**V8, an equivalent mutant (entry check limited to light methods).** The answer is the same
without the heavy entry check:

- every heavy handler that does work checks the deadline again before doing it: eth_call and
  eth_estimateGas before each simulation, the log handlers at their first block;
- every other heavy handler is a bounded read-only one, whose late result `member` replaces with
  −32020;
- no heavy method is effectful.

The mutant only lets a bounded read-only heavy handler run once past its deadline before its answer
is replaced.

**R3-M4-NO-CLASS-LIMIT, equivalent since round 3c (`answer`'s check over the reservation).** Since
round 3c `member` applies both class limits on every path: `capped` (RESP_MAX) and the light limit
(LIGHT_RESP_MAX for a light-table entry, whatever its lane); a light-lane request is always a light
entry (`is_light` implies `light_class`, and `light_value` agrees, unit-tested). A batch is bounded by
`batch` below BATCH_BYTES_HARD (BATCH_RESP_MAX plus one error item per member). So `answer`'s check
`body.len() > need` can no longer fire; it is kept as defence in depth for the reservation. Removing it
together with the member check (R3C-CLASS-LIMIT-LIGHT-LANE-REMOVED) is caught by the round-3 RG12 test.

## 10. Mutation controls

Runner: scratch `run_mutations_rpcguard_r2.py`, on an exported copy of the committed tree
(dc4e3d1). Results: `evidence/MUTATIONS-RPCGUARD-R2.md` and `evidence/mutations-rpcguard-r2.json`.
Each mutant disables one mechanism, and its named tests must then FAIL. A timeout is incomplete,
never caught.

21 mutants, no timeouts: 20 CAUGHT, each on the assertion written for its mechanism, and 1
equivalent (V8, §9).

- Stage A's RGM1–RGM7, re-expressed against the round-2 code.
- RGM8–RGM11: the post-encoding deadline check, the deadline from admission, the per-10^6-gas poll,
  and busy answers without a parse tree.
- The review's survivors: V1a, V1b, V2, V3, V4a, V4b, V5, V6, V7 and V8.

### 10a. Round 3

Runner: scratch `run_mutations_rpcguard_r3_final.py` (derived from the second review's runner: the
round-3 runner's 53 mutants unchanged, plus 6 added in the continuation), on an exported copy of the
final tree `d0e87f9`, run in four consecutive parts and merged. Results:
`evidence/rpcguard-r3/MUTATIONS-RPCGUARD-R3.md` and `evidence/rpcguard-r3/mutations-r3-final-d0e87f9.json`.
The mutants are the round-2 and integration controls re-expressed against the round-3 code, every
survivor the second review named, and one or more mutants per round-3 change. Earlier, incomplete
runs (65a828b partial, 1530d7f truncated after 46 of 53, N2 alone on 8039a09) are kept in
`evidence/rpcguard-r3/MUTATIONS-R3-EARLIER-RUNS.md` and not counted.

59 mutants, every control passing, no timeout: 53 CAUGHT, 6 SURVIVED. Every verdict is the one expected before the run.

- The second review's survivors, now each with a committed test: N1, N2, N6, N7, N8, N9, I2, NF1 (I2 caught, N1 caught, N2 caught, N6 caught, N7 caught, N8 caught, N9 caught, NF1 caught).
- Round-3 changes: R3-M1-* (late errors, a handler's own deadline), R3-M2-* and R3-SOCKWAIT-* (the wait
  helper and the pacer's wait), R3-m4-* (read_by), R3-M4-* (−32020 {resultBytes}), R3-m5 (deadline by
  class), R3-m6-* (SO_SNDBUF read-back, its counter, its upper bound), R3-m1 (the rate note), R3-m2-*
  (the A13 small-answer term and the id-parse filter). Six were added in the continuation:
  R3-SOCKWAIT-ROUND-DOWN, R3-SOCKWAIT-EVENTS-SWAPPED, R3-m4-WAIT-WITHOUT-READ-POLL and
  R3-m6-NO-UPPER-BOUND are caught; R3-M2-BACKSTOP-ONLY and R3-P3-NO-EFFECTFUL-EXEMPTION survive, as
  expected (below).

Survivors, each expected and explained. Two of them mark what the committed tests deliberately do not
pin: the pacer's timer term below ENFORCE_SLACK (a tighter assertion would be timing-fragile on
Windows' timer tick), and the P3 reading (not yet decided):

- **V8-NO-ENTRY-CHECK-HEAVY**: equivalent (RPCGUARD.md §9 V8): every heavy handler that works re-checks the deadline, the others are bounded and their late answer is replaced
- **I3b-NO-WRITE-CAP** (on Linux): a non-blocking send takes at most the free (locked) send buffer, so A is unchanged; the cap bounds Windows, which takes a whole send() (runs 3, 5, 7), and on Linux it keeps throughput (no delayed-ACK stall)
- **R3-M2-NO-WALL-IN-WAIT**: bounded by the backstop: BATCH_WALL is then seen at most PACER_BACKSTOP (50 ms) late, within its 100 ms release (network.md:282)
- **R3-m2-NO-PARSE-FILTER**: the measured peaks stay below SMALL_REPLY_PEAK without the filter (worst parse transient about 85 KB: `busy-path-peaks.log`, objs127 on the unfiltered path); the filter is what bounds ID_PARSE_PEAK by derivation
- **R3-M2-BACKSTOP-ONLY**: the close is then up to PACER_BACKSTOP (50 ms) after s_v, within ENFORCE_SLACK (200 ms), the bound network.md:247 binds and the test asserts
- **R3-P3-NO-EFFECTFUL-EXEMPTION**: no committed test pins P3 option A (a late effectful answer keeps its result): it is the running reading pending a decision (proposals P3), deliberately not pinned

*Superseded in part by the third review (§13):* the I3b explanation above was wrong (the survival came
from a missing throughput assertion, not from Linux socket semantics), and the review found five
further unpinned terms (the pacer's A/R timer term, the backstop, the post-encoding check, the light
late check, the check-then-set race). Round 3c's pass is below.

### 10b. Round 3c

Runner: scratch `mut_r3c.py` (it imports the round-3 runner and keeps its verdict rules; it reads a
test's status from the next line when the test prints a measurement on its own test line, the third
review's parse artifact), on an exported copy of dd7d1ec, checked unchanged against a fresh export
after the run. Results: `evidence/rpcguard-r3c/MUTATIONS-RPCGUARD-R3C.md` and
`evidence/rpcguard-r3c/mutations-r3c-dd7d1ec.json`.

35 mutants, every control passing, no timeout: 30 CAUGHT, 5 SURVIVED, every verdict as expected
before the run.

- The third review's survivors and probes, against the round-3c tests: RV3-SV-GRACE-ONLY (2,170,384
  waits in 1.5 s against a bound of 1,094), I3b-NO-WRITE-CAP (the honest answer took 8,198 ms to
  send), RV3-NO-BACKSTOP (2 waits before GRACE, floor 5), RV3-LATE-CHECK-BEFORE-ENCODING,
  RV3-LATE-HEAVY-ONLY and RV3-M5-PAD-ONLY are caught. RV3-CAS-CHECK-THEN-SET survives (a test limit,
  §2 and §13).
- Round 3c's changes: R3C-WAIT-OUT-BACKSTOP (a pacer sleeping out the backstop on every block: 10,949
  ms), R3C-CLASS-LIMIT-LANE-ONLY, R3C-CLASS-LIMIT-LIGHT-LANE-REMOVED, R3C-COND-NOT-LIGHT,
  R3C-COND-FULL-FLAG-IGNORED, R3C-COND-COUNT-65, R3C-COND-HEX-COUNT-IGNORED, R3C-PROBE-PARAMS-IGNORED,
  R3C-PROBE-FIRST-PARAM, R3C-VALUE-NONARRAY-LIGHT, R3C-BATCH-MEMBER-LIST-ONLY and
  R3C-ENCODE-CHECK-SKIPS-HOOK (the encoding test depends on its hook) are caught; R3-m5-DEADLINE-BY-LANE
  (anchor moved) is caught.
- The round-3 mutants on the code and tests round 3c touched, re-run: RGM1, RGM4, RGM8, V2, N3, N4, N9,
  N11, R3-M1-RESULTS-ONLY, R3-M2-SLEEP-POLL-1MS and R3-M4-32008 are caught.

Survivors: V8 (equivalent, §9), R3-M2-NO-WALL-IN-WAIT and R3-M2-BACKSTOP-ONLY (both within the spec's
bounds, as in §10a: BATCH_WALL seen ≤ 50 ms late within its 100 ms release; the close ≤ 50 ms after s_v
within ENFORCE_SLACK), R3-M4-NO-CLASS-LIMIT (equivalent since round 3c, §9) and RV3-CAS-CHECK-THEN-SET
(a test limit: atomicity holds by construction, §2). The round-3 survivors that round 3c did not touch
(R3-m2-NO-PARSE-FILTER, R3-P3-NO-EFFECTFUL-EXEMPTION) keep the §10a explanation and were not re-run.

## 11. Evidence index (round 2)

All runs use Rust 1.99.1, release, `--locked`, under the shared CPU lock, on loopback 127.0.0.1.

| file | what it records | result |
|---|---|---|
| `evidence/rpcguard-r2-new-tests-3x-6a561b6.log` | the new and changed tests (15 net, 1 consensus, 1 admission, 9 rpc unit, 3 lp4), three consecutive runs on 6a561b6 | 3/3 green |
| `evidence/rpcguard-r2-entry-test-3x-dc4e3d1.log` | the test added in the second commit, three consecutive runs | 3/3 green |
| `evidence/rpcguard-r2-b5-stress-dc4e3d1.log` | review B5: the work-deadline test, 21 runs under load | 21/21 ok |
| `evidence/MUTATIONS-RPCGUARD-R2.md`, `evidence/mutations-rpcguard-r2.json` | mutation controls on dc4e3d1 | 20 caught, 1 equivalent |
| `evidence/rpcguard-r2-suite-pocold-dc4e3d1.log` | full pocold release suite, once at the end | exit 0 |
| `evidence/rpcguard-r2-suite-lp4-dc4e3d1.log` | full lp4-execution release suite (native.rs changed) | exit 0, no warnings |
| `evidence/rpcguard-r2-e2e-dc4e3d1.log` | tests/e2e_devnet.py and tests/e2e_site.py (fixed loopback ports, e2e lock) | both exit 0; 37 PASS, no FAIL |
| `evidence/rpcguard-r2-netkey-vectors.txt` | provenance of the netKey test vectors (the extension's own derivation) | 4 vectors |

**Not run here (round 2).** The Windows CI (`audit-r2-windows-verify.yml`) and the browser (Chrome)
e2e. *Since then (integration): Windows runs 9–13 passed the lp1, lp4-execution and pocold suites on
29fd44a to d40fd91, whose Rust code is that of 29fd44a and 0d6f9dd (WINDOWS-VERIFICATION.md), so the round-2 pacer and SO_SNDBUF did run on Windows.
For round 3 see §12.*

### 11a. Evidence index (round 3), `evidence/rpcguard-r3/`

All runs use Rust 1.99.1, release, `--locked`, under the shared CPU lock (the e2e also under the e2e
lock), on loopback 127.0.0.1, this Linux host. The final tree is d0e87f9: its Rust code is 8039a09's
plus one doc-comment citation in src/rpc.rs and one assertion message in tests/consensus.rs, and
8039a09's src/ is 1530d7f's. The runs on d0e87f9 are the round's results; the earlier ones are kept
as they were written.

| file | what it records | result |
|---|---|---|
| `suite-pocold-d0e87f9.log` | full pocold release suite on the final tree, once at the end | exit 0, no warnings; 164 committed tests passed, 0 failed, 8 ignored |
| `suite-lp4-d0e87f9.log` | full lp4-execution release suite on the final tree (lp4-execution is unchanged in round 3) | exit 0, no warnings; 168 passed |
| `e2e-devnet-d0e87f9.log` | tests/e2e_devnet.py against the release pocold of d0e87f9 (POCOLD) | exit 0; PASS, 27 checks, no FAIL |
| `MUTATIONS-RPCGUARD-R3.md`, `mutations-r3-final-d0e87f9.json` | the mutation pass on the final tree (§10a) | see §10a |
| `ffi-windows-check-d0e87f9.log` | rpc::sockbuf + rpc::sockwait of d0e87f9 copied into a standalone crate: `cargo check --target x86_64-pc-windows-msvc` and on Linux, `#![deny(warnings)]` | both exit 0 |
| `ffi-windows-control.log` | positive control for that check: one wrong argument type in the WSAPoll call | the Windows check fails (E0308), the Linux check passes |
| `pacer-measure-after-d0e87f9.log` | SendPacer CPU and throughput on the final tree, three runs (§4a) | §4a |
| `pacer-measure-before-fa97594.log`, `pacer-measure-after-1530d7f.log` | the same before and after the change (§4a) | §4a |
| `pollbench.log` | standalone bench: poll vs 1 ms sleep, capped vs uncapped writes, with its source | §1, §4a |
| `busy-path-peaks.log` | heap of a busy answer per id shape, before and after round 3, with the scratch test's source | §2 (CONN_BUF row) |
| `a13-breakdown-8039a09.log` | the A13 itemized bound at the round-3 code (src/ as d0e87f9; `a13_bound_breakdown` asserts the total and passed in the d0e87f9 suite) | total 5,581,426,268 B |
| `new-tests-3x-8039a09.log` | the new and changed tests (12 net, 5 rpc/net unit, admission, A13 breakdown), three consecutive runs on 8039a09 | 3/3 green |
| `mainq-test-6x-8039a09.log` | the main-queue test after its fix, six consecutive runs | 6/6 green; worst 121–144 ms at QUEUE_WAIT 106–128 ms |
| `suite-pocold-8039a09.log`, `suite-pocold-1530d7f.log`, `suite-lp4-1530d7f.log`, `e2e-devnet-8039a09.log`, `ffi-windows-check.log` (8039a09) | the same checks on the earlier round-3 commits | all exit 0 (164 and 168 passed; e2e 27 checks) |
| `MUTATIONS-R3-EARLIER-RUNS.md`, `mutations-r3-65a828b-partial.json`, `mutations-r3-1530d7f-truncated.json`, `mutations-r3-n2-8039a09.json` | earlier mutation runs: a partial run, a run truncated after 46 of 53 mutants, and N2 alone | not counted (the note says why); re-run in full on d0e87f9 |

**Not run here (round 3).** The browser (Chrome) e2e and tests/e2e_site.py (the task asked for
e2e_devnet.py). The Windows CI was not run by this track; *since then (review RG3V-m5): Windows run 16
on 25bec25 (recorded in a129656 on claude/audit-repair-a01-a14, `evidence/windows-25bec25/`) passed
the pocold suite, net 45 passed among them, including `sockwait_reports_readiness_within_its_timeout`,
`a_reader_sees_its_closed_flag_without_a_shutdown`, `a_late_revert_or_halt_past_its_deadline_is_32020`
and `queue_wait_bounds_the_wait_on_a_full_main_queue`. 25bec25 contains 5e7446f, and its pocold src/,
tests/net.rs and lp4-execution src/ are identical to 5e7446f's (`git diff 5e7446f 25bec25` is empty
for them), so that run is a Windows run of the round-3 code. Round 3c's changes (§13) have not run on
Windows.*

## 12. Second review (round 2 and integration, 273e785) and round 3

Branch wf/rpcguard-r3, from fa97594. Code commits: 0f48e6e (the fixes and their tests), 65a828b (the
pacer's wait counter; the closed-flag test cannot hang), 1530d7f (the heavy-deadline test checks that
the stop hook ended the run), 8039a09 (the main-queue test starts its 150 clients 1 ms apart: in a
burst they overflowed the listen backlog, and a SYN retried after 1 s made it fail 2 of 3 repeats and
the N2 control), d0e87f9 (continuation after a container restart: the backstop's citation in a doc
comment, network.md:246, and an assertion message that still named −32008). The Rust code under src/
is the same in 1530d7f and 8039a09, and differs in d0e87f9 by that one comment. Every change below is either conformance with the approved text (the
line is cited), the authorized RpcGuard implementation, or tests and measurements; nothing normative
is changed (proposals P3 and P4 record the readings that need a decision).

| finding | resolution |
|---|---|
| RG2-M1 (major) late revert or halt answered with its late error | FIXED, conformance with network.md:235 ("انتهاؤها −32020 {deadline}"): any answer of a method without an effect that completes after its COMPUTE_DEADLINE, a result or an error, is −32020 {deadline} (the log handlers' with resume data); a handler's own −32020 {deadline} and the EFFECTFUL methods' answers are kept (§3). W1-equivalent test with the revert and the halt variants, and a wide-deadline control. The mutation pass then found that the heavy-deadline test no longer told the stop hook from the post-encoding check (RGM5 survived on 65a828b); the test now requires the answer before 3/4 of the whole run (1530d7f) |
| RG2-M2 (major) the pacer polls every 1 ms | FIXED, the spec's mechanism (network.md:245–246): the pacer waits for writability through poll(2) or WSAPoll (`rpc::sockwait`) for min(PACER_BACKSTOP 50 ms, s_v + TIMER_EPS − sendTime, BATCH_WALL), and keeps the write cap of SEND_SOCK_BUF per call (§4). Measurements before and after in §4a. A committed test bounds the waits of a client that does not read, so a return to polling fails it |
| RG2-M3 (major) mechanisms without a committed test | Tests committed for every survivor named: N1, N2, N6, N7, N8, N9 (§2, §3, §5, §6), I2 through the `SOCKBUF_FAULT` test hook (§1), NF1 (all 25 facts against the running limits, §8). I3b survives on Linux, by design: a non-blocking send on Linux takes at most the free (locked) send buffer, so the cap does not change the accepted count A; its effect on A is on Windows (runs 3, 5 and 7), and on Linux it keeps throughput (§1). Mutation results: §10a |
| RG2-M4 (major) −32008 and unlisted methods | Overflow: FIXED, conformance with network.md:210 and the error list (network.md:363): −32020 {resultBytes} with the reservation shrunk and released; nothing depended on −32008 (the extension never uses it; one unit test asserted it and now asserts −32020). Unlisted methods: not changed; the light and heavy rows are PARTIAL, the nine extra methods and the listed but unimplemented ones are named (§2), and the decision is proposed (P4) |
| RG2-m1 `rate` has no observable effect | The fact stays at its running value (20) with status EQUIVALENT, vacuous per connection, and the admission notes say so (`RPC_EQUIVALENT`, tested) |
| RG2-m2 the A13 small-answer term undercounts; the total is not asserted | FIXED, and larger than reported: round 2 kept echoed ids as parse trees, up to 340,378 B per answer. Ids are kept as their encodings; the A13 term is conn_max × SMALL_REPLY_PEAK (132,768 B, derived), asserted against 13 adversarial id shapes; `a13_bound_breakdown` asserts the total, 5,581,426,268 B (§8) |
| RG2-m3 stale documents and comments | Code comments fixed (rpc.rs `conn`, the SO_SNDBUF comment; tests/net.rs, the 503 comment). This file's §9 and §11 are annotated with the integration state. FINDING-STATUS.json and REPAIR-REPORT.md are the orchestrator's; the text proposed for them is in the round-3 report (doc_updates) |
| RG2-m4 P2P read_by reuses a socket after SO_RCVTIMEO timeouts | FIXED: `read_by` waits for readability with `sockwait::readable` (poll or WSAPoll, at most READ_POLL) and then reads; no SO_RCVTIMEO, so no receive ever times out on the socket; the closed-flag check stays; the socket stays blocking (its writer thread shares it). The closed-flag unit test fails instead of hanging if a read blocks. The RPC side's `read_until` still sets SO_RCVTIMEO, to the remaining REQ_READ_DEADLINE: there a timed-out receive ends the connection (the request is not read further), so the socket is never used for reading after a timeout |
| RG2-m5 a light method over 4 KiB gets the heavy deadline | FIXED, conformance with network.md (ب) and :235 (the class is the method's): `light_method` picks the deadline; tested |
| RG2-m6 SO_SNDBUF read-back unchecked; shared refusal counter | FIXED: read-back outside [SEND_SOCK_BUF, 2·SEND_SOCK_BUF] refuses the connection, counted as `sockbufRefused`; fault-injection test |
| RG2-m7 P3 says nothing is implemented | FIXED in the proposals document: option A is the running behaviour pending a decision, encoded since round 2 by `EFFECTFUL` |
| RG2-m8 LIGHT_RESP_MEM "ENFORCED (≥ spec)" | FIXED: EQUIVALENT, with the S1 argument and the extra 512 KiB stated (§2), and an admission note |

**Windows.** The new platform code (`rpc::sockwait`: poll on Unix, WSAPoll on Windows, next to
`rpc::sockbuf`) was cross-checked for compilation: both modules, copied verbatim into a standalone
crate with `#![deny(warnings)]`, pass `cargo check --target x86_64-pc-windows-msvc` and `cargo check`
on Linux (`evidence/rpcguard-r3/ffi-windows-check-d0e87f9.log` on the final tree;
`ffi-windows-check.log` is the same check on 8039a09). No new crate and no new windows-sys feature
(WSAPoll, WSAPOLLFD, POLLRDNORM and POLLWRNORM are in Win32_Networking_WinSock). *Since then (review
RG3V-m5): Windows run 16 on 25bec25, whose pocold and lp4-execution code is 5e7446f's, passed the
pocold suite with the WSAPoll checks (`sockwait_reports_readiness_within_its_timeout`, the closed-flag
test, the pacer and P2P tests; `evidence/windows-25bec25/`, a129656), so round 3's code has run on
Windows. Round 3c (§13) changes no platform code; its new tests have not run on Windows.*

## 13. Third review (round 3, 5e7446f) and round 3c

Branch wf/rpcguard-r3c, from 5e7446f. Code commit: dd7d1ec (the class limit and the conditional
entries in `rpc.rs`, the encoding test hook, and the tests). Every change is conformance with the
approved text (the line is cited) or tests; nothing normative is changed, and no platform code (FFI)
is touched.

| finding | resolution |
|---|---|
| RG3V-M1 (major) the pacer's A/R timer term is pinned by no test | TESTED: `send_pacer_timer_follows_a_over_r_past_grace`, a reader above R whose A/R stays ahead of sendTime past GRACE: its waits over 1.5 s past GRACE are bounded (backstop plus one per KiB read), it is not closed, and once it stops reading it is closed at A/R (CloseOK, lateness ≤ ENFORCE_SLACK; sendTime 5.4–6.4 s against a GRACE of 0.2 s). RV3-SV-GRACE-ONLY (s_v = GRACE) is caught (§4, §10b) |
| RG3V-M2 (major) I3b (no write cap) is not harmless on Linux | Agreed: round 3's "by design" was wrong; the survival came from a missing throughput assertion. TESTED: the honest reader's ~14 MiB answer must be sent within 2 s (batchSendMaxS, from the first byte to sendDone; at least 7 MiB/s, against about 2 MiB/s without the cap). I3b-NO-WRITE-CAP and R3C-WAIT-OUT-BACKSTOP (a pacer that sleeps out the backstop on every block) are caught on Linux (§1, §10b) |
| RG3V-M3 (major) LIGHT_RESP_MAX not applied to light methods on the heavy lane | FIXED, conformance with network.md (ب) and :210: `member` applies the light limit to every light-table entry's answer, whatever the lane, alone or as a batch member; the heavy lane keeps its RESP_MAX reservation. Socket test with RG12's scenario (§2) |
| RG3V-m1 conditional light entries get the heavy deadline | FIXED, conformance with network.md (ب) and :235: eth_getBlockBy* with hashes only and pocol_getHeaders with count ≤ 64 have the light deadline and answer limit, from params[1] as their handlers read it. Their lane stays heavy, named in the PARTIAL light row (§2, §3) |
| RG3V-m2 the 50 ms backstop is pinned by no test | TESTED: a non-reader's waits before its close must be at least GRACE/PACER_BACKSTOP/2; RV3-NO-BACKSTOP is caught (§4) |
| RG3V-m3 "including encoding" and the light late check pinned by no test | TESTED through the `ENCODE_DELAY_MS` test hook (0 in the node; it holds the encoded answer of one test id): `the_compute_deadline_includes_encoding`. RV3-LATE-CHECK-BEFORE-ENCODING and RV3-LATE-HEAVY-ONLY are caught (§3) |
| RG3V-m4 the check-then-set survivor of the CAS test | NOT A DEFECT, a test limit, now stated in §2: atomicity holds by construction (one `compare_exchange` on each side); the race window of a load-then-store is nanoseconds and a test hook cannot be placed inside a single compare-and-swap, so the test pins the check, not the atomicity. RV3-CAS-CHECK-THEN-SET remains a survivor, recorded as such |
| RG3V-m5 stale Windows statements | FIXED here (§11a, §12): Windows run 16 on 25bec25 ran the round-3 code; §12 cites `ffi-windows-check-d0e87f9.log`. The texts proposed for the orchestrator's documents are corrected in the round-3c report |
| RV3-M5-PAD-ONLY (informational) | Caught now: `the_conditional_light_entries_have_the_light_deadline` also sends a big-bodied light method whose size comes from its id, not a padding field |

Evidence: `evidence/rpcguard-r3c/` (§13a).

### 13a. Evidence index (round 3c), `evidence/rpcguard-r3c/`

All runs use Rust 1.99.1, release, `--locked`, `--offline`, under the shared CPU lock (the e2e also
under the e2e lock), on loopback 127.0.0.1, this Linux host, on the code of dd7d1ec (the logs' headers
give the commit; "dirty=1" there is this file, uncommitted at the time; nothing under prototypes/ was).

| file | what it records | result |
|---|---|---|
| `suite-pocold-dd7d1ec.log` | full pocold release suite, once, `--no-fail-fast` | exit 0, no warnings; 169 passed (164 + the 5 new tests), 0 failed, 8 ignored |
| `new-tests-3x-dd7d1ec.log` | the new and changed tests (8 net with `--nocapture`, 1 rpc unit), three consecutive runs | 3/3 green; honest reader sent in 15, 18, 16 ms; A/R reader 36 waits in each 1.5 s window (bound 1,094), CloseOK at 6.37, 5.40, 5.40 s with lateness 1–2 ms; non-reader 10 waits (floor 5, bound 30) |
| `e2e-devnet-dd7d1ec.log` | tests/e2e_devnet.py against the release pocold and pocol-sign of dd7d1ec | exit 0; PASS, 27 checks, no FAIL |
| `MUTATIONS-RPCGUARD-R3C.md`, `mutations-r3c-dd7d1ec.json` | the round-3c mutation pass (§10b) | 30 caught, 5 survivors as expected |

Not run in round 3c: lp4-execution (unchanged), the Windows CI, the browser e2e and
tests/e2e_site.py. No platform code changed, so no new FFI cross-check was needed. No witness file
is committed; the third review's witnesses were not re-run (the committed tests now assert what they
showed).

