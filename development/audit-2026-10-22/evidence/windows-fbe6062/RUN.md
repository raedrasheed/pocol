# Windows verification of fbe6062 (evidence only (the scale measurement, the D-7 test under load, Windows on d104891 and b47fb85); node code as b6d57e8)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40575426466, https://github.com/palcertcom/pocol/actions/runs/40575426466
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `fbe606235e1535c5360d784de1fd9ed2a002a344`, not dirty. No node code changed since `bc38d41`.
- **Run:** 20:08:57 to 20:43:27 UTC on 2026-10-27.
- **Result: PASS on every step:** lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, pocold 239/0;
  extension npm ci, build, and unit tests 234/0.
- **Artifact** `audit-r2-windows-fbe606235e1535c5360d784de1fd9ed2a002a344`, id 12769073268, 25,527 bytes, sha256 `3a6f38e36071508ae3067f4a267e78f0ce13e6f599d383809ed113e7471d434a`: the digest GitHub reports, and
  the digest of the downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client** (`logs/pocold.log`): client window 0.504 s (bounds 0.45 to 1.0), first
  byte 721 ms against an honest reader's 721 ms, pacer lateness 1 ms.

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5036391,"closedMs":1225,"firstByteMs":721,"honestFirstByteMs":721,"os":"windows","pacerLateMaxMs":1,"pacerWaits":10,"sendTimeS":0.50164,"violationS":0.5}
```

A Windows pass is not acceptance: the run does not include the Python and Chrome e2e suites, the thread-count
assertions or the ignored tests.
