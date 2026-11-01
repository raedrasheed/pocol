# Windows verification of 206eaa4 (evidence only (acceptance on fbe6062); node code as b6d57e8)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40583789163, https://github.com/palcertcom/pocol/actions/runs/40583789163
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `206eaa48253e84ad4ef3b09759b9a87d58da96d8`, not dirty. No node code changed since `bc38d41`.
- **Run:** 21:05:30 to 21:29:12 UTC on 2026-10-27.
- **Result: PASS on every step:** lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, pocold 239/0;
  extension npm ci, build, and unit tests 234/0.
- **Artifact** `audit-r2-windows-206eaa48253e84ad4ef3b09759b9a87d58da96d8`, id 12772056943, 25,521 bytes, sha256 `edb31036f70a7bf9a445c9b7ffec6deb9132aa63cc9afcff24a6e478c6ed5c2e`: the digest GitHub reports, and
  the digest of the downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client** (`logs/pocold.log`): client window 0.504 s (bounds 0.45 to 1.0), first
  byte 751 ms against an honest reader's 784 ms, pacer lateness 3 ms.

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5043867,"closedMs":1255,"firstByteMs":751,"honestFirstByteMs":784,"os":"windows","pacerLateMaxMs":3,"pacerWaits":10,"sendTimeS":0.503198,"violationS":0.5}
```

A Windows pass is not acceptance: the run does not include the Python and Chrome e2e suites, the thread-count
assertions or the ignored tests.
