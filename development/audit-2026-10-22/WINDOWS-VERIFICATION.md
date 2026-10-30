# Windows verification of the repair scope (round 2)

Status (2026-10-24): **Run 26 (43e7e0f, code 462fffc, the last code commit) PASS on every check**. Before it, Run 22 (c959379) PASS. Run 21 (1cc96c4) confirmed the fix of
the one flaky test that Run 20 found. Earlier: **Run 16 (25bec25) PASS on every check**, on the round-3 integration: RpcGuard r3
(poll/WSAPoll pacer and P2P reader), caps r3 tests and journal r3 measurements. Earlier: Run 9 (29fd44a) PASS on every check
of the repair scope. That covers the
integrated caps-r2 and rpcguard-r2 code and the Windows fixes found by Runs 1–7. Not covered: the e2e
fix round, merged after 29fd44a, which changes only the Python e2e test that this workflow does not
run. Any later code commit needs its own run. See "Runs" at the end. The sections before "Runs" are the original preparation record and are kept as they were.

## The existing workflows (inspected)

| workflow (on `main`) | trigger | covers | covers the repair scope? |
|---|---|---|---|
| `lp3-s1-windows-verify.yml` | manual dispatch | lp1-node only, Rust 1.58.1: full suite, LP2 storage journal, crash recovery, CLI harness | partly: lp1-node (including `store_journal`); not pocold, lp4-execution, journal v2, sockets or the extension |
| `lp3-regression.yml` | pull_request to `work/lp3-consensus-foundation` | the same script | not triggered by PR #27 (other base) |
| `lp4-execution-controls.yml` | pull_request to two LP4 bases, Ubuntu | LP4 controls | not Windows; not triggered by PR #27 |
| `lp3-typescript.yml` | pull_request, Ubuntu | LP3 model | no |

PR #27 has no check runs.

## Prepared for this scope
`.github/workflows/audit-r2-windows-verify.yml` and `.github/scripts/audit-r2-windows-verify.ps1`.

How it runs:
- Manual dispatch only.
- Windows Server 2022.
- The `commit` input is required and must be a full SHA. The script refuses any other checkout, and the
  artifact is named `audit-r2-windows-<sha>`.
- Actions are pinned to the same commit SHAs as the LP3 workflow. There are no secrets, and the
  token is read-only.

| check | toolchain | what it verifies |
|---|---|---|
| `lp1-store-journal` | Rust 1.58.1 | the Windows-only journal tests that execute and fail with UnsupportedPlatform on Linux (11 tests) |
| `lp1-node-all` | Rust 1.58.1 | the full lp1-node suite, including the secp backend change of this repair |
| `lp4-execution` | Rust 1.99.1, release | execution, admission scope, A13 execution model, proposal vector replays |
| `pocold` | Rust 1.99.1, release | journal v2 (store), P2P and RPC sockets (net), the work deadline and light lane, admission, consensus |
| `extension-npm-ci`, `-build`, `-unit` | Node 22.22.0, official zip, SHA-256 `c97fa376…c77a` pinned | the extension build and unit/worker suites |

What it does not cover:
- The real-Chrome and live-devnet suites. Playwright/Chromium and the Python devnets are not
  prepared for Windows.
- pocold's thread-count assertions. They read `/proc` and are reported as `THREADS-NOT-MEASURED`
  on Windows. The other assertions of those tests still run.

What deserves attention on Windows:
- Journal v2 replaces files by rename while other handles may be open. Windows has different
  sharing rules from Linux.
- Directory fsync is skipped on Windows (`sync_dir` ignores the open error).
- Socket timeouts.

## Why it has not run: the exact blocker
1. `POST /repos/raedrasheed/Proof-of-Collaboration/actions/workflows/audit-r2-windows-verify.yml/dispatches`
   with `ref=claude/audit-repair-a01-a14` and `commit=df637252fd0691dc5891295271d164bc132d04fe` returned
   **404 Not Found** (2026-10-22). GitHub dispatches a workflow only when its file is on the default
   branch (`main`). The file exists only on the PR branch, and merging into `main` is not authorized.
2. The LP3 S1 workflow is on `main` and could be dispatched against this branch, but it covers lp1-node
   only. It is manual by design, and running it was left to the owner in round 1, so it was not
   dispatched.
3. Billing:
   - The repository is now **public**. Its workflow comment ("private … uses the owner's included
     minutes") is out of date.
   - GitHub Actions on standard GitHub-hosted runners, Windows included, are free for public
     repositories.
   - Billing therefore does not block a run. The blocker is the dispatch rule and the merge
     authorization.

## Owner options (any one is enough)
- **A.** Put only the two `.github` files on `main` (a separate commit or PR containing just them),
  then dispatch "Audit repair r2 Windows verification (manual)" with the PR head SHA.
- **B.** Authorize a `pull_request` trigger for PR #27, limited to base `claude/affectionate-pascal-rjhh2k`
  and the repair paths. It would then run on every push, at the head SHA.
- **C.** For lp1-node only, dispatch the existing LP3 S1 workflow on `claude/audit-repair-a01-a14`
  with `commit` = the PR head SHA.

Until one of these happens:
- Windows evidence for the repair commit does not exist.
- Every Windows-dependent item stays BLOCKED (environment), in FINDING-STATUS.json and the dashboard.

## Runs

The owner's 2026-10-23 instruction ("complete the authorized works: RpcGuard and the Windows
verification") was taken as authorizing option B:
- commit a03e089 adds a `pull_request` trigger, limited to PRs into `claude/affectionate-pascal-rjhh2k`
  from this repository, and to the repair paths;
- the commit under test is the PR head SHA.
Nothing was merged into `main`, and manual dispatch is unchanged.

### Run 1: a03e089 (code tree identical to 3c3792e): FAIL
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/39942501824 (pull_request).
- Artifact `audit-r2-windows-a03e089a35affc22fed9f5aa3c961eac97d61658`, zip SHA-256
  `6b2a90c1e45e118607438f3ffe96ea370478bd96de1f2917b328c6a7cc27f482`. Its content is copied unchanged to
  `evidence/windows-a03e089/`.
- Platform: Windows Server 2022 (NT 10.0.20348), image win22 20261012.333.1; Rust 1.58.1 / 1.99.1;
  Node 22.22.0, checksum verified.

| check | status | detail |
|---|---|---|
| lp1-store-journal | PASS | 12 passed, 0 failed (the Windows-only journal tests) |
| lp1-node-all | PASS | 150 passed, 0 failed, 20 binaries |
| lp4-execution | PASS | 162 passed, 0 failed, 19 binaries |
| pocold | **FAIL** | 75 passed, 48 failed, 12 binaries |
| extension npm ci / build / unit | PASS | unit: 234 passed |

The 48 pocold failures have **one cause**:
- All 48 are in `tests/consensus.rs` (28) and `tests/net.rs` (20), and each panics with
  `retention.rkeep: Access is denied. (os error 5)`.
- `retention_rkeep` (src/node.rs, added by D-REORG-REDB in 3c3792e) fsyncs the data directory with
  `File::open(dir)`. Windows cannot open a directory as a file without backup semantics, so every
  `Node::open` failed on Windows.
- `store::sync_dir` already skips that case. This new call did not, and the Linux suites cannot see the
  difference.
- No other test failed. Every test that did not call `Node::open` passed, including the
  store/journal v2 tests. The thread-count assertions were reported THREADS-NOT-MEASURED, as designed.

Fix (next commit): on non-Unix targets the directory fsync after the rename is skipped, as in
`store::sync_dir`. On Linux the code path is unchanged. Linux check:
`cargo test --release --locked --test consensus replay_uses_the_largest_r_keep` → 1 passed.
The fix is verified only by a Windows run on the commit that carries it (Run 2).

Durability note:
- On Windows, the rename of `retention.rkeep` is not followed by a directory flush. This is the same
  as the journal's own renames.
- A crash right after the rename can leave the old value. The old value is never smaller than the one
  used by the records written before it, because the file is raised before replay and never lowered.
  So the worst case is that the next open rewrites the file.

### Run 2: 5d25fae (the retention.rkeep fix): FAIL, with 3 tests left
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/39951153803.
- Artifact `audit-r2-windows-5d25faecdb25776c068b86adb3d5ab2a63b8920a`, zip SHA-256
  `97767fff26350f6136d3ccb274368fb9fbf14fb4528ad53470ebf4b0208eeec7`. Copied unchanged to
  `evidence/windows-5d25fae/`.
- lp1-store-journal 12/12, lp1-node-all 150/150, lp4-execution 162/162 and the extension (234 unit tests)
  PASS again.
- pocold: **120 passed, 3 failed**. The retention.rkeep fix is verified on Windows: none of the 48
  earlier failures recur.

The 3 failures are all in `tests/net.rs`. Diagnosis from the code and the messages:

| test | Windows message | cause | fix (next commit) |
|---|---|---|---|
| `a_peer_that_stops_reading_is_dropped_and_consensus_keeps_serving` | `wait_until(1000, live==0)` false | The drop calls `shutdown(Both)`, which wakes the reader's blocked `read` on Linux but not on Windows. The dropped peer's reader then held its slot and thread until the idle deadline (60 s in the test, 300 s by default). **A real defect on Windows**: dropped peers kept inbound slots. | `read_by` blocks at most `READ_POLL` = 100 ms per read and returns once the peer's closed flag is set. New unit test `a_reader_sees_its_closed_flag_without_a_shutdown` sets only the flag (Windows semantics on any OS): with the flag ignored it waits 10.006 s and fails (`evidence/windows-fixes/read-poll-mutant-flag-ignored.log`); with the fix it passes. |
| `rpc_work_deadline_skips_expired_work_and_releases_resources` | heavy call answered 200, expected 504 | The test sets the deadline to a quarter of the measured end-to-end time of the heavy call. On Windows the measured time included Nagle plus delayed-ACK stalls (about 200 ms): `write!` on an unbuffered `TcpStream` issues one write per formatted piece, on both the test client and pocold's `respond`. So the deadline exceeded the real work. The same stall added about 200 ms to every RPC answer on Windows. | pocold sets `TCP_NODELAY` on accepted RPC sockets and writes the header and the body in two writes. The test client sets `TCP_NODELAY` and sends its request in one write. |
| `an_answer_is_reserved_until_its_connection_has_written_it` | `during` = 8388608 (only the stats request's own reservation) | Not determined. Either the 14 MiB answer was absorbed by the Windows loopback buffers while unread (then the test's premise does not hold there), or it was never observed above the threshold. | The message now reports `built` separately. Run 3 decides; the assertion is unchanged. |

Linux after the fixes: `cargo test --release --locked --lib --test net` → 39 + 20 passed
(`evidence/windows-fixes/linux-lib-net-after-fix.log`). The Windows result is Run 3.

### Run 3: a39379b (peer read poll, RPC without Nagle stalls): FAIL, with 1 test left
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/39954404741.
- Artifact `audit-r2-windows-a39379b92fb6ed3b9a3348b05743cd8371d2b882`, zip SHA-256
  `aa578a8cf576aafe27b49c74d7f332908419cdcd989806ea22f25395c0e33fd0`. Copied unchanged to
  `evidence/windows-a39379b/`.
- lp1 (12 + 150), lp4-execution (162) and the extension (234) PASS.
- pocold: **123 passed, 1 failed**, including the new unit test.
- Verified on Windows by this run:
  - `a_peer_that_stops_reading_is_dropped_and_consensus_keeps_serving` passes: the read poll fix.
  - `rpc_work_deadline_skips_expired_work_and_releases_resources` passes: TCP_NODELAY and whole writes.

The one failure is `an_answer_is_reserved_until_its_connection_has_written_it`, with
`built=true, 8388608 B reserved`.
- The answer (about 14 MiB) was built and reserved.
- Within one second its write had completed into the kernel, although the client never read. Only the
  stats request's own reservation remained.
- So the Windows loopback buffers took the whole unread answer. pocold's accounting behaved as specified:
  a reservation is held until the connection has written the answer.
- The test's premise ("an answer larger than the socket buffers") does not hold on Windows with default
  socket options.

This is a real gap, not only a test issue:
- With default options, kernel buffering per unread RPC connection on Windows is at least about 14 MiB.
  That is up to conn_max × BATCH_RESP_MAX of kernel memory, outside the A13 resident bound.
- RpcGuard bounds exactly this with SO_SNDBUF = 32 KiB (network.md), which pocold does not set: std
  exposes no setsockopt.
- Status: OPEN, to be resolved together with the RpcGuard integration. Options: set SO_SNDBUF through a
  small platform shim (libc / windows-sys, both already in the lock), or state the kernel-buffer term
  explicitly.
- The test assertion is not weakened.

### Run 4: ad7064a (evidence-only commit, code as in a39379b): same result as Run 3
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/39956990270. It was triggered by
  a push that added only run 3's evidence.
- Artifact zip SHA-256 `5967228c777b7ea4446ab860d89607decfd0a75c3327c01203f6f239355d543d`, copied to
  `evidence/windows-ad7064a/`.
- pocold 123/124, with the same single failure (`an_answer_is_reserved_until_its_connection_has_written_it`).
  Everything else PASS.
- **Not counted as new verification**: the code is the same as in Run 3. It only shows the result
  repeats.

### Run 5: be95e26 (integration: caps-r2 + rpcguard-r2 + SO_SNDBUF): FAIL, with 3 tests left
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/39995765956.
- Artifact zip SHA-256 `689cc2e20d2be85d16491a8c7c31246aa627fa0f155482567fc098c0ed74fe8c`, copied to
  `evidence/windows-be95e26/`.
- PASS:
  - lp1-store-journal 12;
  - lp1-node-all 150;
  - **lp4-execution 168** (with the caps track's tests and RpcGuard's simulate stop hook);
  - the extension (234).
- pocold: **148 passed, 3 failed**, 14 binaries. New tests that pass on Windows:
  - tests/caps.rs and the caps identity tests;
  - the RpcGuard round-2 tests;
  - `get_params_and_rpc_stats_report_the_limits`. **SO_SNDBUF is set on Windows and reads back 32768.**

The 3 failures have one cause: the client's receive window on Windows loopback.

| test | Windows message |
|---|---|
| `an_answer_is_reserved_until_its_connection_has_written_it` | `built=true, 8388608 B reserved` |
| `send_pacer_closes_a_slow_reader_after_grace` | the pacer did not close the non-reading client within 5 s |
| `batch_wall_closes_the_batch` | the wall did not close the send within 20 s |

- In all three, a client that never reads asks for a ~14 MiB answer.
- With the server's send buffer now 32 KiB, the Windows kernel still took the whole answer within a
  second. So the absorption is on the **client's** side: its receive window, which Windows loopback
  grows without the application reading.
- Then the server's socket accepts every byte:
  - the reservation is released;
  - the pacer sees A = size, a fast reader, and correctly does not close;
  - the send completes before BATCH_WALL.
- pocold's logic is unchanged. What differs on Windows is that an unread client cannot be told apart
  from a fast one by the bytes the server's socket accepted.

What this means:
- Server-side memory stays bounded: reservations are released when the kernel has the bytes.
- The kernel memory belongs to the client's socket, which the server cannot size. A local client can
  hold up to about one answer (≤ BATCH_BYTES_HARD) of kernel memory per connection.
- The SendPacer's slow-reader detection is effectively disabled for such clients on Windows loopback.
- Status: **OPEN, documented platform limitation.** RpcGuard's SO_SNDBUF bounds only the server's side.

Change in the next commit:
- The three tests use a client with SO_RCVBUF = 64 KiB (`unread_client`). They test pocold's pacer, wall
  and reservations, which need a kernel that does not absorb the answer. The pocold assertions are
  unchanged.
- A new measurement, `unread_default_client_absorption`, records the default-window behavior on every
  platform (ABSORB line, `kernelTookWholeAnswer`) without asserting it. The Windows runs keep the
  limitation visible.

### Run 6: 2a226d8 (evidence-only commit, code as in be95e26)
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/39995905345.
- Same code as Run 5. Not counted as new verification, and its artifact is not copied.

### Run 7: 49508a5 (client SO_RCVBUF = 64 KiB in the three tests): FAIL, the same 3 tests
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40000115736.
- Artifact zip SHA-256 `53916781c1b8cef6a7c58e0cb20a98e7150d8f034520c31a0e82d88b80065be6`, copied to
  `evidence/windows-49508a5/`.
- lp1 (12 + 150), lp4 (168) and the extension PASS. pocold 149 passed, 3 failed: the same three, with
  the same messages.
- **This refutes Run 5's diagnosis.** Bounding the client's receive window did not stop the absorption,
  so the receive window is not where the answer goes.
- Corrected cause: Windows (AFD) accepts a whole `send()` whenever the socket's buffer is not yet full
  when the call starts, so SO_SNDBUF is checked before the copy, not against its size. pocold's pacer
  passed the whole remaining answer (~14 MiB) to one `write`.
- Fix (next commit):
  - `pace_write` writes at most SEND_SOCK_BUF per call.
  - The tests go back to clients with default socket options, which is the realistic case. The
    client-side SO_RCVBUF helper is removed.
  - `unread_default_client_absorption` now writes its ABSORB line straight to stdout, so it reaches the
    log of a passing run (libtest captures `println!` output for passing tests).
- Linux after the fix: the five affected tests pass 3 times out of 3
  (`evidence/rpcguard-integration/write-cap-tests-3x-linux.log`), with ABSORB `kernelTookWholeAnswer:false`.
  Windows: the next run.

### Run 8: ecf8f2a (evidence-only commit, code as in 49508a5)
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40000455354. Same code as Run 7.
- Not counted, and its artifact is not copied.

### Run 9: 29fd44a (pacer writes at most SEND_SOCK_BUF per call): **PASS**
- Run: https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40003897313.
- Artifact zip SHA-256 `04bc0d5bdef900301077e4f4d152471fd49bc9ecd3a58b343705b7c5458dd1ec`, copied unchanged to
  `evidence/windows-29fd44a/`.
- Windows Server 2022, Rust 1.58.1 / 1.99.1, Node 22.22.0.

| check | status | detail |
|---|---|---|
| lp1-store-journal | PASS | 12 passed (Windows-only journal) |
| lp1-node-all | PASS | 150 passed |
| lp4-execution | PASS | 168 passed, 21 binaries: caps identity, simulate stop hook, admission |
| pocold | PASS | 152 passed, 0 failed, 2 ignored (the spec-depth tests, ignored by design), 14 binaries |
| extension npm ci / build / unit | PASS | unit 234 passed |

pocold binaries in this run:
- journal v2 store, P2P and RPC sockets;
- RpcGuard round 2 with SO_SNDBUF and the write cap;
- caps (tests/caps.rs), log_cap with its feature off (compiled, no tests run), admission and consensus.

ABSORB on Windows (`unread_default_client_absorption`):
`{"os":"windows","answerBytes":14681122,"peakReserved":14680950,"reservedOneSecondLater":14680950,"kernelTookWholeAnswer":false}`
- With SO_SNDBUF and writes of at most SEND_SOCK_BUF, the kernel no longer takes a whole unread answer.
  The reservation stays held while the client does not read, as on Linux.
- This closes the gap that Runs 3, 5 and 7 showed, and it corrects Run 5's diagnosis (Run 7).

Not measured on Windows:
- Thread-count assertions read /proc and are skipped there. Their THREADS-NOT-MEASURED line is captured by
  libtest in a passing run, so the summary no longer mentions it; the assertions were still not measured.
- The real-Chrome and live-devnet suites, and the Python e2e tests, are not in this workflow.

Windows defects found and fixed by this verification:
1. `retention.rkeep`: a directory fsync that Windows refuses (Run 1, 48 tests).
2. A dropped P2P peer's reader that `shutdown` does not wake on Windows (Run 2).
3. Nagle plus delayed-ACK stalls of about 200 ms on every RPC answer (Run 2).
4. Kernel absorption of unread RPC answers (Runs 3, 5, 7): SO_SNDBUF plus the write cap.

### Runs 10–13: all PASS
| run | commit | what changed since Run 9 | counted as |
|---|---|---|---|
| 10 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40006362144 | 273e785 | evidence only | repeat of Run 9 (same code) |
| 11 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40006493330 | 0d6f9dd | merge of wf/journal-r2: adds `tests/journal_measure.rs` (#[ignore] measurement tests) | **verifies that journal_measure.rs builds on Windows** (its tests are ignored, so they are not run) |
| 12 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40007742530 | e35f165 | merge of wf/e2e-r2: changes only tests/e2e_devnet.py, which this workflow does not run | repeat of Run 11 for the Rust code |
| 13 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40009973331 | d40fd91 | evidence only | repeat |

Their artifacts are not copied: the code under test is that of Run 9 or Run 11.

### Runs 14–17
| run | commit | content | result | counted as |
|---|---|---|---|---|
| 14 | fa97594 | evidence only | PASS | repeat |
| 15 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40041180282 | 2cbac12 | merge of wf/caps-r3 (tests/caps.rs: the TXBYTES counter's block and branch, the cross-block exact fit) | PASS | verifies the caps r3 tests on Windows |
| 16 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40053978772 | 25bec25 | merges of wf/rpcguard-r3 (rpc::sockwait: poll on Unix, WSAPoll on Windows, in the SendPacer and the P2P reader; late answers -32020; new RpcGuard tests) and wf/journal-r3 (journal_measure.rs with a cfg-gated malloc_trim helper) | **PASS** | **first Windows execution of the round-3 code** |
| 17 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40054605962 | 5e1b2dd | evidence only | PASS | repeat of Run 16 |

Run 16 details:
- Artifact zip SHA-256 `5cac46054aebdc00558e9804d0dcf7da8a42915f490a5729d7eded62fe3cdb1c`, copied unchanged to
  `evidence/windows-25bec25/`.
- Results:
  - lp1-store-journal 12 passed;
  - lp1-node-all 150 passed;
  - lp4-execution 168 passed, 21 binaries;
  - **pocold 164 passed, 0 failed, 12 ignored, 15 binaries.** The ignored tests are the spec-depth tests and journal_measure's #[ignore] measurements, which this workflow does not run;
  - extension 234 passed.
- ABSORB on Windows: `kernelTookWholeAnswer:false`.
- The round-3 RpcGuard tests passed on Windows, among them `sockwait_reports_readiness_within_its_timeout`.
- journal_measure.rs builds on Windows, including its cfg-gated helper.

### Runs 18–20
| run | commit | result | notes |
|---|---|---|---|
| 19 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40081065412 | 2b1a571 (merges of caps r3b, journal r3c, rpcguard r3c) | PASS | first Windows run of the round-3 final passes |
| 20 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40082181726 | 49ae9fa (evidence only, same code as Run 19) | **FAIL**, 1 test | `abandoned_queued_bodies_are_capped_and_released`: queue reached its cap (peak 4) but no -32021 busy within 5 rounds. Artifact (SHA-256 `eb61b267c7575709e5f6d8a6121b68c0cdb1f083153c706e6ccfedc801ece283`) in `evidence/windows-49ae9fa/` |

Run 20 is a timing flake, not a code difference: Run 19 passed on the same code.
- The test needed a QUEUE_WAIT-long full queue to arise from racing senders, and five rounds were not enough on that runner.
- Fix (next commit, test only):
  - HEAVY_QUEUE 2 instead of 4, QUEUE_WAIT 5 ms instead of 20 ms, and up to 12 rounds;
  - assertions unchanged (busy ≥ 1, peak ≤ HEAVY_QUEUE, nothing left queued).
- Linux: 8 of 8 runs reach busy in round 1, with 10–14 busy answers
  (`evidence/integration-r3/abandoned-queued-8x-linux.log`).
- The next Windows run decides.

### Runs 21–23: all PASS
| run | commit | content | counted as |
|---|---|---|---|
| 21 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40084357701 | 1cc96c4 | the abandoned-queue test settings (Run 20's flake) | **verifies the flaky-test fix on Windows** |
| 22 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40088225658 | c959379 | merge of wf/e2e-r3c: tests/consensus.rs (16bebfd, the deterministic long catch-up test) and tests/e2e_devnet.py (not run by this workflow) | **the last code commit: every Rust change of round 3 has now run on Windows** |
| 23 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40088664386 | d91d141 | evidence only | repeat of Run 22 |

Their artifacts are not copied. The workflow's own artifacts hold them for 14 days; Runs 16 and 9 are the
copied references.

### Runs 24–26: all PASS
| run | commit | content | counted as |
|---|---|---|---|
| 24 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40090047292 | c104e64 | docs only (code as in c959379) | repeat of Run 22 |
| 25 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40090138075 | 8ce75c4 | docs only (code as in c959379) | repeat of Run 22 |
| 26 https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/40092151252 | 43e7e0f | evidence for 462fffc (the prototypes/ tree is 462fffc's): the A11 reorg walk (pocold/src/node.rs) and the R14/R19 calibration (lp4-execution/src/admission.rs) | **the last code commit has run on Windows**. The new calibration test runs there. The new deep-reorg test is `#[ignore]` (release only) and does not |

Their artifacts are not copied.

### palcertcom/pocol, PR #2 (claude/memory-r14-followup), 2026-10-27

| run | commit | result |
|---|---|---|
| 40507248611 | f65b44a (node code as 892dca6) | PASS on every step; evidence/windows-f65b44a/ |
| 40510911424 | 2839f2a | PASS on every step; evidence/windows-2839f2a/ |
| 40512470697 | c0a0e66 (evidence only) | PASS |
| 40516277272, 40516762578 | 8d80354, 799e809 | cancelled: replaced by newer pushes (one pending run per PR) |
| 40518244446 | 69d5794 | PASS on every step (lp4 201/0, pocold 237/0); evidence/windows-69d5794/ |
| 40522024488 | 182d73a (evidence only) | **FAIL, 1 test**: `send_pacer_closes_a_slow_reader_after_grace`, no close within the test's 5 s wait; evidence/windows-182d73a/ |
| 40525606586 | 2c40a9f (tests only) | PASS on every step (lp4 201/0, pocold 237/0); the pacer test as before 6a1e480; evidence/windows-2c40a9f/ |
| 40530131878 | 7624419 (contains 6a1e480) | PASS on every step (lp4 201/0, pocold 237/0); the first run of the pacer test with its PACER line; evidence/windows-7624419/ |
| 40533664638, 40537362826 | 1babbc7, b46658a | cancelled: replaced by newer pushes |
| 40535963171 | 563a0c3 | PASS on every step (lp4 201/0, pocold 238/0); evidence/windows-563a0c3/ |
| 40538722157 | b66ba65 | PASS on every step (lp4 205/0 with the four MODEXP tests, pocold 238/0): the first Windows run of the MODEXP precompile (`b46658a`'s one-word path); evidence/windows-b66ba65/ |
| 40546902481 | d45dfe2 | PASS on every step (lp4 208/0 with the seven MODEXP tests, pocold 238/0): the first Windows run of the wide even path (`0c2ad75`); PACER close 2 ms after s_v; evidence/windows-d45dfe2/ |
| 40550009922 | b8fb7ab | PASS on every step (lp4 209/0 with the eight MODEXP tests, pocold 238/0): the trivial powers handed to revm; PACER close 1 ms after s_v; evidence/windows-b8fb7ab/ |
| 40550825203 | beed0d2 (evidence only; node code as b8fb7ab) | PASS on every step (artifact id 12757853501, sha256 b7d8e186…fc3b; not copied) |
| 40557534874 | 7e565b7 (contains 3705559) | PASS on every step (lp4 209/0, pocold 238/0): the first Windows run of the strengthened pacer test; the client saw 0.508 s from its first byte to the close, its first byte 517 ms after the request against an honest reader's 504 ms; evidence/windows-7e565b7/ |
| 40558373933 | d104891 (evidence only; node code as b8fb7ab, tests as 3705559) | PASS on every step (lp4 209/0, pocold 238/0); client window 0.509 s, first byte 732 ms against an honest reader's 783 ms, pacer lateness 10 ms; evidence/windows-d104891/ |
| 40561773086 | b47fb85 (evidence only; node code as b8fb7ab, tests as 3705559) | PASS on every step (lp4 209/0, pocold 238/0); client window 0.506 s, first byte 741 ms against an honest reader's 746 ms, pacer lateness 3 ms; evidence/windows-b47fb85/ |
| 40572992585 | bc38d41 (node code as b6d57e8: review fixes 1 and 4, stored versions read by diff at open; e6d07b2's D-7 test) | PASS on every step (lp4 211/0, pocold 239/0), the new diff, checkpoint, A13 and D-7 tests included; client window 0.504 s, first byte 583 ms against an honest reader's 562 ms, pacer lateness 3 ms; evidence/windows-bc38d41/ |
| 40575426466 | fbe6062 (evidence only; node code as b6d57e8) | PASS on every step (lp4 211/0, pocold 239/0); client window 0.504 s, first byte 721 ms against an honest reader's 721 ms, pacer lateness 1 ms; evidence/windows-fbe6062/ |
| 40578404549 | 71eb650 (evidence only; node code as b6d57e8) | PASS on every step (lp4 211/0, pocold 239/0); client window 0.504 s, first byte 737 ms against an honest reader's 795 ms, pacer lateness 1 ms; evidence/windows-71eb650/ |
| 40583789163 | 206eaa4 (evidence only; node code as b6d57e8) | PASS on every step (lp4 211/0, pocold 239/0); client window 0.504 s, first byte 751 ms against an honest reader's 784 ms, pacer lateness 3 ms; evidence/windows-206eaa4/ |
| 40585164582 | 93a40d0 (node code: the deferred checkpoint release) | PASS on every step (lp4 211/0, pocold 240/0); client window 0.506 s, pacer lateness 3 ms; evidence/windows-93a40d0/ |
| 40589342452 | 22a82c2 (node code: release steps fsynced) | **FAIL, one test:** pocold 239/1, `e4_checkpoint_damage_falls_back_to_a_verified_state`. Open panicked on a store the release at open had marked damaged: a defect of `93a40d0`, reproduced on Linux and fixed after `ad301e7` (evidence/windows-22a82c2/RUN.md). lp4 211/0, extension 234/0 |
| 40592643692 | dd85cc0 (the release at open and in the event loop tested; node code as 22a82c2) | PASS on every step (lp4 211/0, pocold 241/0); client window 0.514 s, pacer lateness 10 ms; evidence/windows-dd85cc0/ |
| 40595988585 | ad301e7 (node code: the independent review's fixes) | PASS on every step (lp4 211/0, pocold 244/0); client window 0.505 s, pacer lateness 2 ms; evidence/windows-ad301e7/ |
| 40600109673 | e453851 (node code: open sets aside a store it finds damaged) | PASS on every step (lp4 211/0, pocold 245/0), the damage test and the new release tests included; client window 0.503 s, pacer lateness 2 ms; evidence/windows-e453851/ |
| 40608577150 | 260d021 (evidence only; node code as e453851) | PASS (conclusion success; artifact not copied) |
| 40662875687 | 22543cf (round 4; node code as 3adc356: Reject::Local, streamed journal open) | PASS on every step (lp4 211/0, pocold 250/0, **pocold-io-fault 106/0**, the new step with the deterministic I/O fault tests); client window 0.509 s, pacer lateness 4 ms; evidence/windows-22543cf/ |
| 40680583220 | 1074b47 (round 4: checkpoint open, the review's fixes) | **FAIL, one test:** pocold 252/1, `a_store_mode_boot_is_timed_through_a_scratch_state_store_in_its_data_directory`: the whole-state boot timing gate refused (R14, R19) with no times recorded; pocold-io-fault 110/0. Cause not established; the test now prints the times with a refusal. evidence/windows-1074b47/ |
| 40683933211 | eed8309 (documents and logs) | **PASS**; the 1074b47 failure did not recur, which does not explain it |
| 40688067254 | 748b16d (a refused admission prints a calibration run's times) | **PASS** |
| 40697654215 | a136f27 (JM_HEND_LOG) | **PASS** |
| 40706925600 | b4f13f3 (eb85bed: canonical path from the fork point) | **PASS** |
| 40711254355 | 719da04 (e2e_integrated test) | **PASS** |
| 40725594174 | 051dfaa | cancelled (replaced by the next push) |
| 40728995184 | 5cd7d43 (87d2392 + 5cd7d43: the side set, tries cleared where given) | **PASS**: the side set's node code on Windows |
| 40737560413 | b375d4c (50f9207: REC_MIN, e2e_browser harness) | **PASS** |
| 40744711022 | 1bba829 (the second review's F-A, F-B, F-C) | **PASS** (conclusion success; artifact not copied) |

The 182d73a failure:
- It is not the absorption defect of Runs 5 and 7. ABSORB reads `kernelTookWholeAnswer:false`, as in the
  passing runs.
- The test's wait counted from the request, so it included executing the batch before any byte was sent.
  That takes 0.67–0.91 s on Linux and can take longer on a busy Windows runner.
- The test now waits up to 20 s and bounds the send time (from the first byte) by s_v + 1 s, with
  s_v = max(GRACE, A/R). Lateness ≤ ENFORCE_SLACK and CloseOK are unchanged.
- It prints a PACER line on every platform. Details in evidence/windows-182d73a/RUN.md.
- Run 40530131878 on 7624419 printed it: `sendTimeS` 0.502009 s, `acceptedBytes` 32,940, s_v = GRACE = 0.5 s,
  `pacerLateMaxMs` 2 (ENFORCE_SLACK 200), 10 pacer waits (bounds 5 to 30), the connection closed. The close came
  1,092 ms after the request: 0.59 s of execution before the first byte, then 0.502 s of sending. Every bound of
  network.md:237-256 holds on Windows. The longer wait is not what makes it pass: the 5 s wait would have held
  in this run too.
- Two later runs printed it too: 563a0c3, the close 1 ms after s_v (send time 0.501655 s); b66ba65, 10 ms after
  (0.510413 s). On all three Windows runs the kernel took 32,940 B and every bound held.
- This shows the mechanism on Windows. It does not show what delayed the 182d73a run, which recorded no send
  time. That cause stays likely, not established. Details in evidence/windows-7624419/RUN.md.

