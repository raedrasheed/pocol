# Windows verification of 7e565b7 (the first with the strengthened pacer test of 3705559)

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40557534874,
  https://github.com/palcertcom/pocol/actions/runs/40557534874
- Trigger: pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- Commit under test: `7e565b7f8b055be4dd4b11051ba48f4cd12906a0`, not dirty. It contains 3705559.
- Job 122379914317: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  18:15:27 to 18:36:33 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 209/0, pocold 238/0
  (948.2 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-7e565b7f8b055be4dd4b11051ba48f4cd12906a0`, id 12760482875, 25,464 bytes,
  sha256 `7b9ae0a4c3fc08a7aa32ed73fef7ba9d295d99a9a67dff4145370000864f8656` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is `b8fb7ab`'s.

## The pacer seen from the client (`logs/pocold.log`)

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5079767,"closedMs":1025,"firstByteMs":517,"honestFirstByteMs":504,"os":"windows","pacerLateMaxMs":7,"pacerWaits":10,"sendTimeS":0.507924,"violationS":0.5}
```

| check (review 2026-10-27) | bound | Windows |
|---|---|---|
| from the client's first byte to the close, not before GRACE | ≥ 0.45 s | 0.508 s |
| the same, within ENFORCE_SLACK of s_v (0.3 s more for polls) | ≤ 1.0 s | 0.508 s |
| the slow reader's first byte against an honest reader's of the same batch | ≤ 2 × 504 ms + 1 s | 517 ms |
| the pacer's own lateness | ≤ 200 ms | 7 ms |

The client's window (0.508 s) equals the pacer's own send time (0.508 s) to within a millisecond.

A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the thread-count
assertions, or the ignored tests.
