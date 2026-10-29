# Windows verification of b47fb85 (an evidence commit (the Windows runs on beed0d2 and 7e565b7))

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40561773086, https://github.com/palcertcom/pocol/actions/runs/40561773086
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `b47fb8503b3ab33d6a4c6cd04ff52daa951d3701`, not dirty.
- **Job 122403390420:** Windows Server 2022, Rust 1.58.1 / 1.99.1, Node 22.22.0; 18:58:36 to 19:31:47 UTC on 2026-10-27.
- **Result: PASS on every step.**
  - lp1 store journal 12/0, lp1-node 150/0, lp4-execution 209/0, pocold 238/0;
  - extension npm ci, build, and unit tests 234/0.
- **Artifact** `audit-r2-windows-b47fb8503b3ab33d6a4c6cd04ff52daa951d3701`, id 12764677621, 25,490 bytes, sha256 `288159be1b9d71c7738874216e8eaceb8cceb80669c769ba1d6ed4c5b07d6262`.
  - That is the digest GitHub reports, and the digest of the downloaded file.
  - Its contents are this directory, unchanged.

The node code under test is `b8fb7ab`'s, and the tests are `3705559`'s.

## The pacer seen from the client (`logs/pocold.log`)

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5062751,"closedMs":1247,"firstByteMs":741,"honestFirstByteMs":746,"os":"windows","pacerLateMaxMs":3,"pacerWaits":10,"sendTimeS":0.503426,"violationS":0.5}
```

| check | bound | Windows |
|---|---|---|
| from the client's first byte to the close, not before GRACE | ≥ 0.45 s | 0.506 s |
| the same, within ENFORCE_SLACK of s_v (0.3 s more for polls) | ≤ 1.0 s | 0.506 s |
| the slow reader's first byte against an honest reader's of the same batch | ≤ 2 × 746 ms + 1 s | 741 ms |
| the pacer's own lateness | ≤ 200 ms | 3 ms |

A Windows pass is not acceptance. The run does not include:
- the Python and Chrome e2e suites;
- the thread-count assertions;
- the ignored tests.
