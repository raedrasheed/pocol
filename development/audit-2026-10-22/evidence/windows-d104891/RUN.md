# Windows verification of d104891 (an evidence commit (e2e_integrated's partition cleanup under a seeded failure))

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40558373933, https://github.com/palcertcom/pocol/actions/runs/40558373933
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `d104891d0a9a5b6b11c0ab0385ee114f3ba93fa9`, not dirty.
- **Job 122391681346:** Windows Server 2022, Rust 1.58.1 / 1.99.1, Node 22.22.0; 18:36:43 to 18:58:29 UTC on 2026-10-27.
- **Result: PASS on every step.**
  - lp1 store journal 12/0, lp1-node 150/0, lp4-execution 209/0, pocold 238/0;
  - extension npm ci, build, and unit tests 234/0.
- **Artifact** `audit-r2-windows-d104891d0a9a5b6b11c0ab0385ee114f3ba93fa9`, id 12761509526, 25,516 bytes, sha256 `ff03018aa1534311786dfc3b6a9acd712065fb0c7decdd160cdc0f54982c4d02`.
  - That is the digest GitHub reports, and the digest of the downloaded file.
  - Its contents are this directory, unchanged.

The node code under test is `b8fb7ab`'s, and the tests are `3705559`'s.

## The pacer seen from the client (`logs/pocold.log`)

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5086954,"closedMs":1241,"firstByteMs":732,"honestFirstByteMs":783,"os":"windows","pacerLateMaxMs":10,"pacerWaits":10,"sendTimeS":0.510547,"violationS":0.5}
```

| check | bound | Windows |
|---|---|---|
| from the client's first byte to the close, not before GRACE | ≥ 0.45 s | 0.509 s |
| the same, within ENFORCE_SLACK of s_v (0.3 s more for polls) | ≤ 1.0 s | 0.509 s |
| the slow reader's first byte against an honest reader's of the same batch | ≤ 2 × 783 ms + 1 s | 732 ms |
| the pacer's own lateness | ≤ 200 ms | 10 ms |

A Windows pass is not acceptance. The run does not include:
- the Python and Chrome e2e suites;
- the thread-count assertions;
- the ignored tests.
