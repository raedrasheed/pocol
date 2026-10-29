# Windows verification of 7624419 (the first run with the pacer test of 6a1e480)

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40530131878,
  https://github.com/palcertcom/pocol/actions/runs/40530131878
- Trigger: pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- Commit under test: `76244190da73669e19f1001ad6ab34f1723627b4`, not dirty. It contains 6a1e480.
- Job 122284043545: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  15:27:47 to 15:59:29 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 201/0, pocold 237/0
  (1,506.5 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-76244190da73669e19f1001ad6ab34f1723627b4`, id 12749415387, 25,218 bytes,
  sha256 `7e793140b53976e7a84bcc1813ae076fa90b517348bec0fa4319408fbce74fff` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is 2839f2a's.

## The slow reader's close on Windows (`logs/pocold.log`)

```
PACER {"acceptedBytes":32940,"closedMs":1092,"os":"windows","pacerLateMaxMs":2,"pacerWaits":10,"sendTimeS":0.502009,"violationS":0.5}
ABSORB {"answerBytes":14681122,"kernelTookWholeAnswer":false,"os":"windows","peakReserved":14680950,"reservedOneSecondLater":14680950}
```

The test runs with R = MIN_SEND_RATE = 1 MiB/s, GRACE = 0.5 s and ENFORCE_SLACK = 0.2 s (network.md:237-256).
The send time counts from the first byte sent.

| bound (spec) | required | Windows, this run | Linux, 5 runs (`memory-r14/pacer-linux-5x.log`) |
|---|---|---|---|
| CloseOK: sendTime > GRACE | > 0.5 s | 0.502009 s | 0.501477–0.502219 s |
| CloseOK: A < R·sendTime | < 526,394 B | A = 32,940 B | A = 211,628 B |
| violation instant s_v = max(GRACE, A/R) | | 0.5 s (A/R = 0.031 s) | 0.5 s (A/R = 0.202 s) |
| close within ENFORCE_SLACK of s_v (pacerLateMaxMs) | ≤ 200 ms | 2 ms | 1–2 ms |
| send time below s_v + 1 s | < 1.5 s | 0.502 s | 0.502 s |
| pacer waits, bound 2·(send ms / 50 ms backstop) + 10 | ≤ 30 | 10 | 10 |
| pacer waits, backstop floor (GRACE / 50 ms / 2) | ≥ 5 | 10 | 10 |
| the connection is closed (read returns 0 or an error) | closed | closed | closed |

- **The close is on time.** On Windows the pacer closed the non-reading client 2 ms after the violation instant.
  The spec allows ENFORCE_SLACK = 200 ms. The lateness bound and CloseOK are the same assertions the test had
  before 6a1e480. The new upper bound on the send time is an added check, not a looser one.
- **The longer wait is not the evidence.** 6a1e480 raised only how long the test polls for the close, from 5 s to
  20 s from the request. Here the close came 1,092 ms after the request: 0.59 s executing the batch before the
  first byte, then 0.502 s of sending. The 5 s wait would also have held in this run.
- **Windows' kernel accepted less.** It took 32,940 B of the answer for the client that does not read, against
  211,628 B on Linux. A/R stays far below GRACE on both, so s_v is GRACE on both.
- **The failure on 182d73a stays unexplained by a measurement.** That run saw no close within 5 s of the
  request and recorded no send time (`windows-182d73a/RUN.md`). This run shows the mechanism on Windows meeting
  every bound. It does not show what delayed that run. The likely cause is still a long execution before the
  first byte on a busy runner, and it is not established.

A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the thread-count
assertions, or the ignored tests.
