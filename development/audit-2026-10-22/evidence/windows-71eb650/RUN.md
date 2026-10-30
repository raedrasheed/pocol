# Windows verification of 71eb650 (evidence only (seeded faults in the checkpoint-open diff; Windows on bc38d41; A13 §3c); node code as b6d57e8)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40578404549, https://github.com/palcertcom/pocol/actions/runs/40578404549
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `71eb650ef67ae548db5b03e3108a8458990bfa6d`, not dirty. No node code changed since `bc38d41`.
- **Run:** 20:28:47 to 21:05:44 UTC on 2026-10-27.
- **Result: PASS on every step:** lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, pocold 239/0;
  extension npm ci, build, and unit tests 234/0.
- **Artifact** `audit-r2-windows-71eb650ef67ae548db5b03e3108a8458990bfa6d`, id 12769989513, 25,537 bytes, sha256 `babf62abf42cad80893f652a312174e728be6954497f14204d7300e52b50fc8b`: the digest GitHub reports, and
  the digest of the downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client** (`logs/pocold.log`): client window 0.504 s (bounds 0.45 to 1.0), first
  byte 737 ms against an honest reader's 795 ms, pacer lateness 1 ms.

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5039856,"closedMs":1241,"firstByteMs":737,"honestFirstByteMs":795,"os":"windows","pacerLateMaxMs":1,"pacerWaits":10,"sendTimeS":0.501891,"violationS":0.5}
```

A Windows pass is not acceptance: the run does not include the Python and Chrome e2e suites, the thread-count
assertions or the ignored tests.
