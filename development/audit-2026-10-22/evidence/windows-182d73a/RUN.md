# Windows verification of 182d73a: FAIL, one test

- Repository: `palcertcom/pocol`. Workflow audit-r2-windows-verify.yml, run 40522024488,
  https://github.com/palcertcom/pocol/actions/runs/40522024488 (PR palcertcom/pocol#2, draft).
- Commit under test: `182d73afd75faa629b35e6d74d9311ca6483e767`, not dirty. Its node code is 2839f2a's, plus the
  tests of 8d80354, 69d5794 and 073923f. 182d73a itself adds evidence only.
- Result: lp1 store journal 12/0, lp1-node 150/0, lp4-execution 201/0, extension 234/0: PASS.
  **pocold: 236 passed, 1 failed**, `send_pacer_closes_a_slow_reader_after_grace` (tests/net.rs:545,
  "the pacer closed the non-reading client"): no close within the test's 5 s wait.
- Artifact `audit-r2-windows-182d73afd75faa629b35e6d74d9311ca6483e767`, id 12744302197, 25,389 bytes,
  sha256 `10be7cb75d95a4d91f71e5a9324ccd453c8fd41e65a266f4ea2ce450b314c84e` (as GitHub reports it and as
  downloaded). Its contents are this directory, unchanged.

## Analysis

- **Not the earlier Windows absorption defect (Runs 5 and 7, fixed in 29fd44a).** The same run's ABSORB line reads
  `kernelTookWholeAnswer:false`, as in the passing runs on 2839f2a, c0a0e66 and 69d5794.
- **The same node code passed on Windows three times** (runs 40510911424, 40512470697, 40518244446). This PR
  does not change the RPC pacer or this test's code path.
- **The test's 5 s wait counts from the request, not from the first byte sent.** It includes executing the
  batch (10 eth_calls, each reverting with 1 MiB) before the answer is sent.
  - On Linux the close comes 1.17–1.41 s after the request. Its send time is 0.5015–0.5022 s against
    s_v = 0.5 s, with a lateness of 1–2 ms (memory-r14/pacer-linux-5x.log).
  - So execution alone takes 0.67–0.91 s here.
  - The old bounds were "close within 5 s" and "closed_at < 4 s", both from the request. They leave about
    3–4 s for execution plus s_v. A busy Windows runner can need more.
  - This is the likely cause. The failure message did not record the time to the first byte or the
    accepted bytes, so it is not established. The fixed test prints both on every platform.
- **The fix (test only, tests/net.rs).**
  - The wait runs to 20 s.
  - The promptness bound and the wait-count bound are on the send time (`lastPacerClose.sendTime`): send
    time < s_v + 1 s, with s_v = max(GRACE, A/R). The wait-count bound is 2·(send time / PACER_BACKSTOP) + 10.
  - ENFORCE_SLACK (lateness ≤ 200 ms) and CloseOK (send time > GRACE, A < R·send time) are unchanged.
  - A failure now reports the RPC stats.
  - A `PACER` line (closed ms, send time, accepted bytes, s_v, waits, lateness) goes to stdout directly, as
    ABSORB does, so it reaches the log of a passing run.
- **Checks of the fix on Linux:**
  - the test 5 of 5 (pacer-linux-5x.log);
  - tests/net.rs 53/53;
  - two mutants of the close rule (memory-r14/mutants-pacer.log), both killed:
    - PC1, the pacer never closes, caught at the 20 s wait;
    - PC2, the close only after 4·GRACE, caught by the send-time bound and the lateness check.
- **Windows:** the next run on the PR's head.
