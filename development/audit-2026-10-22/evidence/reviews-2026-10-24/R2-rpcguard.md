# R2: independent adversarial review of RpcGuard (round 3c) and the socket platform code

- **Subject:** branch `claude/audit-repair-a01-a14` at head `ec148cf`. The RpcGuard code is that of `dd7d1ec`, merged at `2b1a571`: `git diff dd7d1ec ec148cf -- prototypes/pocold/src/rpc.rs src/net.rs src/lib.rs src/resources.rs Cargo.toml` is empty. tests/net.rs differs by 6 lines and node.rs by unrelated A11 changes.
- **Worktree state:** the reviewer's worktree was created on `main`. It was detached at `claude/audit-repair-a01-a14` (ec148cf) for this review.
- **Line numbers:** taken from `ec148cf`.
- **Build and run:** Rust 1.99.1, `--release --offline`, `CARGO_TARGET_DIR=…/scratchpad/review-target`, this Linux host (4 CPUs). Other sessions were building and testing on the same machine at the same time, so every timing below was measured under contention.
- **Tests on the unmodified tree:**
  - `--test net`: 49 passed, 0 failed (176 s).
  - `--test admission`: 3 passed.
  - `--lib rpc::`: 11 passed.
- **Scratch material:** everything is under `scratchpad/reviews/r2/`, and nothing was written to the repository.
  - Probes: `probe/tests/probe.rs`, a separate crate that depends on pocold by path. Every node is in-process on 127.0.0.1.
  - Mutation runner: `mut.py`. Its logs are in `mut/*.log` and `mut-run.log`.
  - Probe outputs: `p234.log`, `p2.log`, `p1b5.log`.
- **Repository state at the end:** `git status` is empty. Each mutation was restored byte for byte, and the runner checked this with a sha256.

## Summary of findings

| ID | Sev | Short title | Status |
|---|---|---|---|
| R2-1 | MEDIUM | One RPC batch, even from a web page's Origin, holds the consensus thread for the whole batch: P2P and ticks wait 14.3 s (measured), up to about 35 s by construction | REPRODUCED |
| R2-2 | LOW | eth_sendRawTransaction runs its EVM execution with no stop hook and no COMPUTE_DEADLINE, on the light lane | REPRODUCED |
| R2-3 | LOW | ERR_ITEM_MAX ("enforced") does not hold for ORIGIN_DENY items or other per-member error items: they echo unbounded ids (100,091 B measured) | REPRODUCED |
| R2-4 | LOW | Bytes after Content-Length are never read, so close() sends a RST after every such answer | REPRODUCED (RST); truncation NOT-REPRODUCED on Linux |
| R2-5 | LOW | The whole-request REQ_READ_DEADLINE is pinned by no test: a per-read timeout (slowloris-able) survives | REPRODUCED (mutant M13 survives) |
| R2-6 | NOTE | Admission-side QUEUE_WAIT check and queue-place release are each pinned by a single test | REPRODUCED (mutants M3, M5) |
| R2-7 | NOTE | SO_SNDBUF refusal is silent and per connection; the read-back window is platform-agnostic | CODE-READING |
| R2-8 | NOTE | The acceptor spins on accept() errors (EMFILE) | CODE-READING |
| R2-9 | NOTE | Log SCRATCH charges encoded bytes, not the materialized tree the doc says | CODE-READING |
| R2-10 | NOTE | `rate` is in RPC_ENFORCED but bounds nothing; `enforce_slack_ms` is measured, not enforced | CODE-READING |
| R2-11 | NOTE | Small doc/code disagreements (BATCH_WALL derivation, light-lane comment, HEAD_MAX, the probe on duplicate keys) | CODE-READING |

No HIGH finding. The FFI and unsafe code is sound (§3), and the A13 RPC term holds by inspection (§2). Of the 13 mutants, 12 were caught and 1 survived (§5).

---

## R2-1 (MEDIUM): an RPC batch monopolizes the consensus thread; P2P and ticks starve

- **Where:**
  - `prototypes/pocold/src/rpc.rs:751-771`: `handle_until_budgeted`, the batch loop. Between members it drains only the light lane (`if k>0{n.drain_light()}`, :763).
  - `src/node.rs:257-263` (run loop) and `:284-288` (`Event::Rpc` handled inline).
  - `development/audit-2026-10-22/RPCGUARD.md:160`: the (ط) row.

**Claim.** Batch members run back to back on the one consensus thread, and nothing yields to the main event queue between them. A batch admitted at QUEUE_WAIT keeps the thread until all of these hold:
- every member has run;
- or the work deadline (30 s from read-end, `deadline` at :765) stops new members.

The last member started can then run its own COMPUTE_DEADLINE (5 s). An uninterruptible precompile can add about 0.5 s on top of that. So by construction one batch can hold the consensus thread for about 35 s. A client can repeat this: a new batch queued within QUEUE_WAIT of the previous one's end is admitted. During that time:
- P2P events (Connected/Message/Closed);
- the 50 ms `Tick` that drives production attempts (`tick`, node.rs:614, `advance_due`);
- and every other main event

all wait behind the batch.

The request can carry an Origin header: ORIGIN_DENY only covers the two mining methods, and eth_call is allowed to sites. So a web page the user visits can do this, in any browser that lets it POST to 127.0.0.1. pocold answers the CORS preflight with `Access-Control-Allow-Origin: *` and does not check Content-Type.

RPCGUARD.md (ط) marks the separation NOT ENFORCED but describes only the opposite direction ("light requests wait behind consensus work; the wait is bounded by QUEUE_WAIT"). It states no bound on how long consensus waits behind RPC. The spec's own batch bound is "compute ≤ 20·(5+0.1) = 102 s" (network.md:279). That bound assumes RPC runs on its own runtime (ط). In pocold it is consensus time.

**Evidence.** Probe `p1_a_batch_starves_the_consensus_loop`, run at default limits (`Limits::default()`). The batch was 20 × eth_call to BLAKE2F with 29.9M rounds, sent with `Origin: https://site.example`. A P2P client connected 300 ms after the batch started, sent its Hello, and timed the node's Hello. The node sends its Hello when the consensus thread handles `Connected`.

```
P1 {"baselineHelloMs":0,"singleBlakeMs":752,"batchCode":200,"batchMs":14578,"helloDuringBatchMs":14272}
```

`p1b_back_to_back_batches` gave the same pattern: a Hello sent at 1.5 s was answered after 13,459 ms, and in the gap between batches it took 2–3 ms. A first attempt that did not send its own Hello was closed by the node's 5 s hello deadline before the consensus thread reached the `Connected` event. So the stall also drops inbound peers mid-handshake.

**Status:** REPRODUCED.

**Proposed fix (any one):**
- Give a batch one shared consensus-time budget, for example COMPUTE_DEADLINE heavy for the whole batch. The later members then answer −32020 {deadline} without running.
- Between members, handle pending main events (Net, Tick, Miner), not only the light lane. This is the spec's "each member re-admits from the back of the queue", network.md:274: `Event::Rpc` could carry the batch's continuation and be re-queued.
- Run RPC on its own thread over a head snapshot (the spec's (ط)).

Also state the consensus-side bound in RPCGUARD.md (ط), and add a test that times a P2P Hello during a batch.

## R2-2 (LOW): eth_sendRawTransaction's execution is not bounded by COMPUTE_DEADLINE

- **Where:**
  - `rpc.rs:916-917` (handler);
  - `rpc.rs:816` (`EFFECTFUL`);
  - `rpc.rs:852` (entry check only);
  - `node.rs:399-416` (`submit_tx` → `native::execute_for_profile` at :407, with no stop hook).

**Claim.** eth_sendRawTransaction is a light-table method. Its body is ≤ 4 KiB, so it runs on the light lane under the 500 ms light deadline. It is also EFFECTFUL, so `member` never replaces its late answer (P3 option A). The deadline is checked only at handler entry, and `submit_tx` executes the transaction with no stop hook. So a light request's cost is a full transaction execution at up to the profile's GAS_LIMIT, not ≤ 500 ms.

`drain_light` runs up to LIGHT_TURN = 8 light requests before every main event, so one turn can be 8 such executions. RPCGUARD.md presents COMPUTE_DEADLINE as ENFORCED for the answer, and LIGHT_TURN's comment bounds the delay as "LIGHT_TURN light executions". Neither says that a light execution can be a GAS_LIMIT transaction.

**Evidence.** Probe `p3`: `compute_light` set to 1 ms. Three signed EIP-1559 transactions (alice, nonces 0–2) to BLAKE2F with 7.9M rounds (devnet GAS_LIMIT is 8M). All three were answered with their hash (`result`) after 390, 200 and 246 ms, and none was answered −32020. At the devnet's 8M gas the effect is small. A profile with a larger GAS_LIMIT scales it linearly (about 0.5 s at 30M, from the BLAKE2F timing in R2-1).

**Status:** REPRODUCED (the deadline is not applied). The LIGHT_TURN amplification is CODE-READING: probe p5's burst was mostly rejected for nonce order, and the Hello delay measured was 495 ms.

**Proposed fix (either):**
- Execute the pool admission with the stop hook and the request's `compute_by`, and refuse with −32020 {deadline} before committing: nothing is pooled, so P3's "a late −32020 hides an accepted transaction" does not arise.
- Schedule eth_sendRawTransaction on the heavy lane.

## R2-3 (LOW): ERR_ITEM_MAX is not enforced for ORIGIN_DENY items or other member errors

- **Where:**
  - `rpc.rs:755`: `site_err` uses `err(&i.get("id").cloned()…)`, the full id.
  - `rpc.rs:853`, `:955`: other errors in `one()` echo the full id, and −32601 also echoes the method name, which is unbounded.
  - `lib.rs:56-58` (`err_item_max` in RPC_ENFORCED).

**Claim.**
- network.md:276: "every error ≤ ERR_ITEM_MAX = 256 B, from CONN_BUF". ORIGIN_DENY is stage 2 (network.md:231), before admission, so its per-member error is such an item.
- pocold caps the id at 128 B only in `item_err` (busy, rate, batchBytes, deadline) and `result_bytes`.
- An ORIGIN_DENY item in a batch echoes the whole id. So `err_item_max` is reported enforced while one class of guard-produced item is unbounded.
- Memory is not at risk: the item is built on the consensus thread inside the batch reservation.

**Evidence.** Probe `p4`, a batch with an Origin header and 100,000-byte ids:

```
P4 {"items":[{"bytes":100091,"code":-32601},{"bytes":100040,"code":null},{"bytes":100093,"code":-32601}],"errItemMax":256}
```

The first item is ORIGIN_DENY, the third is method-not-found.

**Status:** REPRODUCED.

**Proposed fix:** build `site_err` with `item_err`'s 128-byte id rule, and bound the method name in −32601 messages. Optionally also bound method errors inside batches. Add the case to `a10_origin_deny_applies_to_every_batch_member`.

## R2-4 (LOW): unread bytes after the request, so every such answer ends in a RST

- **Where:** `rpc.rs:654-655` reads exactly Content-Length. `rpc.rs:702` then calls `pace_write` and drops the socket. The 413 path at :646-652 handles this case; the 200 path does not.

**Claim.** Any bytes that arrive after the body are never read. Examples: a pipelined request (the spec forbids pipelining, but a client may still send one) or a trailing CRLF that some HTTP clients add. When `conn` returns, the socket is closed with unread receive data, so the kernel sends a RST and discards whatever is still in the send queue. pocold's own 413 comment names exactly this hazard. On Windows, a received RST also discards data the client has not yet read, so an answer that is still in flight can be lost (CODE-READING). On Linux loopback the receive window absorbed the tail in every case tried.

**Evidence.** Probe `p2`: the 14,680,950-byte 10-member answer from `pacer_batch()`. The client read it at about 16 MiB/s, waited 0, 300 or 1000 ms before reading, and sent the extra bytes 30 ms after the request.

| case | result |
|---|---|
| control (no extra bytes) | complete, no error |
| `\r\n` | complete body, then `Connection reset by peer (os error 104)`, all 3 delays |
| pipelined request | the same, all 3 delays |

Extra bytes sent in the same segment as the request are consumed by the header read and do not trigger this.

**Status:** RST REPRODUCED; truncation NOT-REPRODUCED on Linux.

**Proposed fix:** after sendDone, `shutdown(Write)` and drain the receive side for a short bounded time and byte count before closing, as the 413 path does. Or set a 0-byte read and discard unread input before close.

## R2-5 (LOW): REQ_READ_DEADLINE "over the whole request" is pinned by no test

- **Where:**
  - `rpc.rs:630` (`deadline` computed once);
  - `rpc.rs:634` (header loop);
  - `RPCGUARD.md:75` ("`read_deadline` over the whole request, ENFORCED, test `rpc_connection_cap_…`").

**Claim and evidence.** Mutant M13 replaces the header loop's deadline with a fresh `Instant::now()+read_deadline` on every read. That turns the whole-request deadline into a per-read idle timeout, so a client that sends one byte every 9 s can hold a connection slot forever. With 32 connections that denies all RPC.

Both named tests pass with the mutant: `rpc_connection_cap_refuses_excess_unread_and_keeps_serving` and `rpc_request_buffer_covers_headers_and_body`. The trickle check in the cap test sends 5 bytes 400 ms apart (2.0 s in total) and then waits 2 s for the close. Under the mutant the close comes 1.5 s after the last byte, which is inside that window.

**Status:** REPRODUCED (mutant survives; `mut/M13-READ-DEADLINE-PER-READ.log`).

**Proposed fix (test):** trickle for longer than read_deadline, for example 1 byte every 400 ms for 3 s against 1.5 s, and require the close before the trickle ends, at about read_deadline after the first byte.

## R2-6 (NOTE): two mechanisms are each caught by a single test

- **M5 (the node ignores QUEUE_WAIT at admission, `rpc.rs:210`).** Caught only by the in-process test `compute_deadline_runs_from_admission_and_queue_wait_bounds_admission`. All three socket tests passed with the mutant (`queued_requests_are_busy_at_queue_wait`, `the_node_never_runs_a_request_its_connection_gave_up`, `batch_wall_counts_the_time_queued`): the connection's own CAS to GONE at QUEUE_WAIT masks the node-side check. That is defence in depth working as designed, but the node-side check rests on one in-process test.
- **M3 (a HEAVY_QUEUE place never released, `rpc.rs:186`).** Caught only by `abandoned_queued_bodies_are_capped_and_released`, through its "only the stats request itself" queued-gauge assertion. `heavy_queue_overflow_answers_busy_with_retry_after` passes, because a leaked place looks like a full queue. So does `queue_wait_bounds_the_wait_for_a_heavy_queue_place`. Also note the abandoned-bodies test is probabilistic (up to 12 rounds to reach a full queue) and takes about 100 s.
- **M10 (HEAVY_QUEUE off by one).** Also caught only by that test's peak assertion.

**Proposed fix (test):** add a deterministic test that sends more than HEAVY_QUEUE sequential heavy requests on an idle node, all answered with results. That catches the leak without timing.

## R2-7 (NOTE): SO_SNDBUF fallback

- **Where:** `rpc.rs:600-605`, `rpc.rs:169`.
- **Silent refusal.** A connection whose SO_SNDBUF cannot be set, or reads back outside [32 KiB, 64 KiB], is shut down with no HTTP answer and no log. Only `sockbufRefused` counts it, and that counter is served by RPC. On a host where the setting always fails, RPC silently serves nothing, and the operator cannot read why.
- **The read-back check is not per platform.** [S, 2S] was chosen because Linux doubles the value. But a Windows or macOS read-back of 64 KiB, double the intended buffer, would be accepted. On Linux, a read-back of 32 KiB means an effective 16 KiB (`wmem_max` = 16 KiB), which is safe (smaller).
- **Fix:** probe once in `serve` (set and read back on the listener or the first socket); log and refuse to start RPC on failure; accept exactly 2S on Linux and S elsewhere.

## R2-8 (NOTE): the acceptor spins on accept() errors

- **Where:** `rpc.rs:594-597`: `for s in l.incoming(){ let Ok(mut s)=s else{continue}; …}`.
- **Claim:** on EMFILE or ENFILE the pending connection stays in the backlog and accept() fails again at once, so the acceptor thread busy-loops at 100% CPU until a descriptor frees. The P2P listener has the same shape (`net.rs`, `listen`).
- **Status:** CODE-READING; not reproduced, since it needs RLIMIT_NOFILE exhaustion.
- **Fix:** sleep briefly (for example 50 ms) on an accept error.

## R2-9 (NOTE): log SCRATCH charges encoded bytes, not materialized heap

- **Where:**
  - `rpc.rs:1101-1104`: `let sz=l.to_string().len()+1; b.charge(sz)`;
  - `rpc.rs:1097`: `receipt_json` builds every log of every receipt, then `.cloned()` copies the logs array;
  - RPCGUARD.md §2 SCRATCH row: "charge the bytes they materialize".
- **Claim:** the 16 MiB budget is applied to the matched logs' encoded size. The working set is larger:
  - each `serde_json::Value` log object is a BTreeMap leaf of about 630 B plus key and value strings, about 3× the encoding of a small log;
  - plus the per-receipt JSON of non-matching logs;
  - plus the cloned array.
- **Effect on A13:** the bound is not broken. With LOGS_COUNT_MAX = 10,000 the tree is about 11 MiB, which with its encoding stays within A13's "answer being built" term of 4 × RESP_MAX = 32 MiB. But `logsScratch = 16 MiB` in rpcLimits is not a bound on materialized bytes.
- **Fix:** charge an estimate of the tree (or the encoded size × a measured factor), or reword the doc.

## R2-10 (NOTE): two R26 facts that read stronger than they are

- **`rate` (20).** It is in `RPC_ENFORCED` and checked (`rate>=1`), but nothing reads it at the running values, and no request rate is bounded. Clients reconnect per request; the review W16 measured 5,520 req/s. The admission note says so ("vacuous per connection"). Still, "EQUIVALENT" is defined at RPCGUARD.md:16 as "the same bound or a stronger one", which a vacuous fact does not meet. Suggest a separate status (VACUOUS), or move it out of the enforced set.
- **`enforce_slack_ms` (200).** Reported ENFORCED "as a measured, asserted bound". No mechanism guarantees the slack under CPU starvation: the pacer thread's wake can be delayed arbitrarily. The tests assert it on an unloaded host. This is honest as written in §4, but it is a measurement, not enforcement.
- **Facts checked and found enforced in code** (each traced to its line):
  - conn_max (`serve` :607);
  - req_buf (:646);
  - light_queue (`LightLane::new` sync_channel);
  - heavy_resp_mem, resp_max, light_resp_max (:373-387, :832-833);
  - batch_resp_max, batch_max (:757, :801);
  - logs_count_max and logs_scan_bytes (:1096, :1106; scan counts whole receipts, conservative);
  - queue_wait_ms (:210, :690-696);
  - compute_heavy_ms (the answer, :836, :852);
  - grace_ms, min_send_rate, timer_eps_ms, batch_wall_ms (`pace_write` :278-290, :762);
  - send_sock_buf (:600-605).
- **Facts with the documented equivalent bound:** heavy_active and light_active (one thread), conn_buf (SMALL_REPLY_MAX), light_resp_mem ((conn_max+1)·512 KiB).
- **Partly enforced:** err_item_max, see R2-3.

## R2-11 (NOTE): smaller disagreements and nits

1. **The BATCH_WALL derivation double-counts.** RPCGUARD.md:148 adds QUEUE_WAIT 2 s to the 30 s work deadline. `work_by` is measured from the same read-end (`rpc.rs:664`), so it already includes the queue wait. The conclusion (< 420 s) holds a fortiori.
2. **A resources.rs comment disagrees with its formula.** The comment says "The light lane holds <= conn_max bodies"; the term uses LIGHT_QUEUE (equal at the defaults).
3. **HEAD_MAX is not exact.** The check runs after the terminator search (`rpc.rs:635`), so a header of up to HEAD_MAX + READ_CHUNK − 1 bytes (about 24 KiB) is accepted. REQ_BUF still bounds the total.
4. **Duplicate keys disagree between the probe and the parse.** For a body with duplicate `params` keys, `light_class` (the probe) fails and says heavy, while `light_value` (the parsed tree, which keeps the last key) says light. The single-request path then uses the heavy deadline (5 s) and the light answer limit, for a request the handler serves as a light one. Harmless, but it contradicts the "the probe agrees" claim for this case.
5. **The main-queue-full wait polls.** `rpc.rs:682` sleeps 1 ms per retry on every waiting connection thread: up to conn_max × 1000 wakeups/s for up to QUEUE_WAIT. A condition variable or a 5 ms poll like `reserve_heavy` would do.
6. **Minor EINTR gap.** `read_until` (:621-625) does not retry `ErrorKind::Interrupted`, so a signal aborts the request read. `pace_write` and `read_by` do handle it.

---

## 1. Liveness and fairness (Q1): what holds

- **A slow or non-reading client is bounded by the pacer.**
  - It is closed after GRACE when A < R·t.
  - Its waits are bounded (10 measured for a non-reader).
  - It costs about one wake per 50 ms.
  - SEND_MAX(size) holds the reservation for at most GRACE + size/R + slack.
- **Saturation is possible as the spec permits.** 32 readers at exactly R hold every slot. Six such readers on 8 MiB answers fill HEAVY_RESP_MEM for about 130 s. Both are spec-level consequences of RPC_CONN_MAX, HEAVY_RESP_MEM and MIN_SEND_RATE, not pocold defects.
- **Half-open and silent sockets:** bounded by REQ_READ_DEADLINE before the request and by the pacer after. Loopback has no true half-open.
- **Many connections:** refused with a 503 written on the acceptor. The write is about 200 B, never blocks, and has a 100 ms timeout. See R2-8 for the error path.
- **Keep-alive abuse:** not possible. Every answer is `Connection: close` and the thread exits after it.
- **Pipelining:** only the first request is read (R2-4).
- **Deadlines mid-write:**
  - COMPUTE_DEADLINE does not apply to the send (correct by the spec).
  - BATCH_WALL cuts a send (M6 caught).
  - The 504 at the work deadline drops a Reply computed afterwards; its reservation is released at the failed `send`.
- **Starving the P2P loop:** yes, see R2-1 and R2-2. This is the one stated bound that does not exist: the (ط) row covers the RPC → consensus direction only.

## 2. Memory (Q2)

**The A13 RPC term recomputes from the code to exactly 218,579,136 B (208.5 MiB), as RPCGUARD.md §8 states:**

| part | bytes |
|---|---|
| connection requests: conn_max × (4·(HEAD_MAX+READ_CHUNK) + REQ_BUF) | 36,700,160 |
| heavy response pool | 50,331,648 |
| light response pool | 17,301,504 |
| small answers: 32 × SMALL_REPLY_PEAK | 4,248,576 |
| queued heavy bodies: 16 × (REQ_BUF+256) | 16,781,312 |
| light lane | 139,264 |
| parse tree | 9,175,040 |
| answer being built: 3 × BATCH_BYTES_HARD + 4 × RESP_MAX | 83,901,632 |

**Each buffer, checked against its term:**
- **Request:** the header Vec is allocated once (it never reallocates; it stops past HEAD_MAX within one read), and the body Vec is exactly Content-Length ≤ REQ_BUF. Queued bodies are shared through an Arc, never copied. A body left in the queue by a connection that gave up holds its HEAVY_QUEUE place until dequeued (`Held`). The light lane is bounded by its sync_channel capacity; `hold(…,usize::MAX)` is only a gauge.
- **In-flight results:** reserved at admission and shrunk to `capacity()` (M7 caught). A Reply waiting in an mpsc channel is covered by its reservation. A Reply produced for a departed connection is dropped at `send`.
- **Busy and rate answers:** within SMALL_REPLY_PEAK. The round-3 unit test measures adversarial id shapes.
- **Pacer:** only the HTTP header string. The body is written from the reservation-backed String.
- **Transient copies on the consensus thread**, all inside the "answer being built" slack of 4 × RESP_MAX by estimate:
  - `one()` clones `params` (≤ the parse tree);
  - `member` and `one` clone the id twice (≤ body);
  - the `light_class` probe allocates an owned method string when the method is escaped (≤ body);
  - `capped` encodes a full answer before checking RESP_MAX. eth_call output is gas-bounded to about 4 MiB, so its hex is about 8 MiB and the String capacity up to 16 MiB.
- **Kernel send buffer:** 64 KiB per connection on Linux (2·SEND_SOCK_BUF), 2 MiB for 32 connections. It is not in A13, and the spec (network.md:224) places kernel buffers outside the count, so this is consistent.
- **Unaccounted allocation:** none found that exceeds its term. The SCRATCH accounting caveat is R2-9.

## 3. Platform code (Q3): `sockbuf`, `sockwait`, `read_by`

- **FFI signatures, Unix** (libc 0.2.191):
  - `setsockopt` and `getsockopt` get (fd, SOL_SOCKET, SO_SNDBUF, pointer to a c_int, sizeof c_int), with an in/out socklen;
  - `poll(&mut pollfd, 1, c_int)`; the nfds literal infers `nfds_t`.
- **FFI signatures, Windows** (windows-sys 0.61.2):
  - `setsockopt(SOCKET=usize, i32, i32, *const u8, i32)`;
  - `getsockopt(…, *mut u8, *mut i32)`;
  - `WSAPoll(*mut WSAPOLLFD, u32, i32)`;
  - `as_raw_socket() as usize` is correct for SOCKET (UINT_PTR).
  - These match the cross-check evidence (`ffi-windows-check-d0e87f9.log`).
- **Unsafe soundness:** every pointer is to a live stack value of the stated size, and each fd or SOCKET is borrowed from a live `&TcpStream` for the call. No aliasing, no lifetime escape. The code is **sound**.
- **Error handling:**
  - Unix poll: −1 with EINTR → `Ok(false)`, treated as a timeout. Both callers re-check their deadlines, so this is correct.
  - Other errors → `Err`. The pacer then ends the write and `read_by` returns false.
  - POLLHUP, POLLERR or POLLNVAL count as `n>0` → `Ok(true)`, and the following read or write reports the error. Correct.
  - Windows has no EINTR. SOCKET_ERROR → `last_os_error()` (GetLastError, which WSAGetLastError aliases). Correct.
- **Timeouts and overflow:**
  - `ms()` rounds up and saturates at i32::MAX. It never yields a negative (infinite) timeout, and 0 only for a zero Duration. The pacer's wait is ≥ TIMER_EPS (1 ms) because st ≤ s_v whenever the rule did not fire.
  - `Duration::from_secs_f64` gets a finite, non-negative value.
  - `Instant + batch_wall` (:288, :381) would panic only for absurd configured limits. These are local config values, not reachable from the network.
- **WSAPoll quirks:**
  - The known WSAPoll bug (no report of a failed non-blocking connect) does not apply: only accepted or connected sockets are polled.
  - Only POLLRDNORM and POLLWRNORM are requested, so the "unsupported flag → WSAEINVAL" quirk does not apply.
  - Timer-tick rounding can end a wait up to a tick early or late. The pacer and read_by tolerate both, and the unit test relaxes its bound on Windows.
  - Polling a blocking socket that another thread is writing to (the P2P reader and writer) is supported.
- **Fallback when SO_SNDBUF is refused:** see R2-7. No 503 is sent, there is no log, and the read-back window is platform-agnostic.
- **`read_by`** (net.rs:169-178):
  - the closed flag is checked before every wait;
  - each wait is ≤ READ_POLL (100 ms), so a closed peer is noticed within 100 ms on Windows, where shutdown does not wake a blocked read;
  - no SO_RCVTIMEO; the read follows readiness on a socket whose other user only writes, so it does not block;
  - EINTR on read is retried.
  - Correct.

## 4. Facts (Q4)

See R2-10 for the per-fact trace. 23 of the 25 RPC_ENFORCED facts are backed by code as reported, enforced or equivalent with the stated argument. Two read stronger than they are:
- `err_item_max` holds only for busy, rate, batchBytes and deadline items (R2-3);
- `rate` is vacuous (R2-10).

`enforce_slack_ms` is a measured, not a guaranteed, bound.

## 5. Mutation table (Q5)

Each mutant was applied alone to the worktree, run with `cargo test --release --offline --test net -- <tests>` and restored (sha256 checked). Logs: `scratchpad/reviews/r2/mut/<name>.log`. Machine under contention from other sessions' builds and tests throughout.

| # | Mutant (file:line) | Tests run | Verdict | Caught by |
|---|---|---|---|---|
| M1 | No write cap: `end=parts[pi].len()` (rpc.rs:269) | send_pacer_closes…, unread_default_client_absorption | CAUGHT | send_pacer_closes… ("the honest reader's ~14 MiB answer took 8637 ms") |
| M2 | Skip the post-encoding deadline check (`member`, rpc.rs:836) | 4 late-answer tests | CAUGHT | all 4 (late revert/halt, encoding, precompile, getLogs resume) |
| M3 | Forget to release a HEAVY_QUEUE place (`Held::drop`, rpc.rs:186) | abandoned_queued…, heavy_queue_overflow…, queue_wait_bounds…place, legitimate_requests… | CAUGHT | abandoned_queued… only (R2-6) |
| M4 | Process-global token bucket (20/s, cap 20) instead of per connection (`rate_refused`, rpc.rs:237) | the_per_connection_bucket…, rpc_batches… | CAUGHT | the_per_connection_bucket… |
| M5 | Node admits regardless of QUEUE_WAIT (`admit`, rpc.rs:210) | queued_requests_are_busy…, compute_deadline_runs_from_admission…, the_node_never_runs…, batch_wall_counts_the_time_queued | CAUGHT | compute_deadline_runs_from_admission… only (R2-6) |
| M6 | No BATCH_WALL check in the send (`pace_write`, rpc.rs:285) | batch_wall_closes_the_batch | CAUGHT | batch_wall_closes_the_batch |
| M7 | No reservation shrink after the answer (rpc.rs:387) | 5 reservation tests | CAUGHT | a_full_heavy_pool… ("shrank to the answer"), an_answer_over_its_limit… ("shrank to the error") |
| M8 | Node runs work whose deadline passed (node.rs:287) | rpc_work_deadline…, abandoned_queued… | CAUGHT | rpc_work_deadline_skips_expired_work… |
| M9 | RPC_CONN_MAX off by one (`>`; rpc.rs:607) | rpc_connection_cap… | CAUGHT | rpc_connection_cap… |
| M10 | HEAVY_QUEUE off by one (`hold`, rpc.rs:188) | abandoned_queued…, queue_wait_bounds…place, heavy_queue_overflow… | CAUGHT | abandoned_queued… (peak ≤ HEAVY_QUEUE) only |
| M11 | ORIGIN_DENY not applied to batch members (rpc.rs:764) | a10_origin_deny… | CAUGHT | a10_origin_deny… |
| M12 | Full light lane blocks instead of answering busy (`send` for `try_send`, rpc.rs:669) | a_full_light_lane_answers_busy | CAUGHT | a_full_light_lane_answers_busy |
| M13 | REQ_READ_DEADLINE per read instead of whole request (rpc.rs:634) | rpc_connection_cap…, rpc_request_buffer… | **SURVIVED** | none (R2-5) |

**Result:** 12 caught, 1 survived. Tests to test 1–2 mutants each took 94–190 s, including the rebuild and waits for the shared build lock.

**Timing-sensitivity of the tests.** No test failed spuriously in this review: the baseline net run plus all the unmutated tests inside 13 runs, under contention. These assertions depend on wall-clock margins and are the likeliest to flake on a loaded or Windows host:
- `send_pacer_closes_a_slow_reader_after_grace`:
  - the honest send must finish in ≤ 2000 ms (15–18 ms typical, so a wide margin);
  - the non-reader's pacer must close within 4 s;
  - pacer waits must fall within [5, 30].
- `send_pacer_timer_follows_a_over_r_past_grace`: needs the client's 100 ms read cadence to keep A/R ahead.
- `queued_requests_are_busy_at_queue_wait`: needs `t < 3/4` of a measured BLAKE2F duration.
- The queue-wait main-queue test (`queue_wait_bounds_the_wait_on_a_full_main_queue`): 150 clients started 1 ms apart, previously fixed for listen-backlog overflow.
- `rpc_connection_cap_refuses_excess_unread_and_keeps_serving`: "refused < 1 s" and "24 clients < 4 s".
- `abandoned_queued_bodies_are_capped_and_released`: probabilistic, up to 12 rounds.
- `a_reader_sees_its_closed_flag_without_a_shutdown` (net.rs unit test): 2 s.

## 6. Other observations (Q6)

- **BATCH_WALL in a release run:** pocold cannot reach it at the defaults (work deadline 30 s + SEND_MAX(BATCH_BYTES_HARD) 258.3 s < 420 s). Its "any nonzero value is a defect" property holds.
- **Earlier survivors re-confirmed:**
  - I3b (no write cap) is now caught on Linux by the throughput assertion, as round 3c claims (M1: 8.6 s against the 2 s bound).
  - RV3-CAS-CHECK-THEN-SET, R3-M2-NO-WALL-IN-WAIT and R3-M2-BACKSTOP-ONLY were not re-run. Their explanations in §10b are consistent with the code.
- **Doc mismatches** are listed in R2-1 (the (ط) row), R2-9, R2-10 and R2-11.
