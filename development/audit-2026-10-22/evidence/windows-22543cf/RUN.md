# Windows verification of 22543cf (round 4: Reject::Local, streamed journal open, io-fault tests, D133 heal tests; node code as 3adc356)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40662875687, https://github.com/palcertcom/pocol/actions/runs/40662875687
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `22543cf400289f0475d776bb6e18ed24ec8a0104`, not dirty. Node code as 3adc356; 16838eb (e2e harness) and 22543cf (documents) change no node code.
- **Run:** 09:15:50 to 10:18:04 UTC on 2026-10-28.
- **Result: PASS on every step:** lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, pocold 250/0,
  **pocold-io-fault 106/0** (the new step: `--features io-fault --test state_store --test consensus`, with the
  deterministic I/O fault tests); extension npm ci, build, and unit tests 234/0.
- **Artifact** `audit-r2-windows-22543cf400289f0475d776bb6e18ed24ec8a0104`, id 12808251971, 29,005 bytes, sha256
  `d588994a048821a29577aac802cc13063bf1d3c637cff98c861373632dde7959`: the digest GitHub reports, and the digest of the
  downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client** (`logs/pocold.log`):

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5091611,"closedMs":1093,"firstByteMs":584,"honestFirstByteMs":710,"os":"windows","pacerLateMaxMs":4,"pacerWaits":10,"sendTimeS":0.504477,"violationS":0.5}
```

Not on Windows: 118afd7 (the checkpoint-open changes), which was pushed after this run. A Windows pass is not
acceptance: the run does not include the Python and Chrome e2e suites, the thread-count assertions or the
ignored tests.
