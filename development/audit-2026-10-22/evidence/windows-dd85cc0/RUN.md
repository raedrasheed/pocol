# Windows verification of dd85cc0 (the release at open and in the event loop tested; node code as 22a82c2)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40592643692, https://github.com/palcertcom/pocol/actions/runs/40592643692
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `dd85cc0055df6dccd6c4107d3de8e96096727fa0`, not dirty.
- **Run:** 22:09:51 to 22:44:59 UTC on 2026-10-27.
- **Result: PASS on every step:**
  - lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, pocold 241/0;
  - extension npm ci, build, and unit tests 234/0.
- **Not run on Windows:** `a_release_step_that_fails_on_io_leaves_the_store_whole` is Unix only (RLIMIT_FSIZE).
- **Note:** The damage test passed here with the defect `e453851` fixes still present: its run chose a node that is not owed.
- **Artifact** `audit-r2-windows-dd85cc0055df6dccd6c4107d3de8e96096727fa0`, id 12777061863, 25,628 bytes, sha256 `54e467c5e999c16ade0c6789c47713fe48456ee3009fc702a5dd4fe8cba55a09`: the digest GitHub reports, and
  the digest of the downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client:** client window 0.514 s, first byte 583 ms
  against an honest reader's 574 ms, pacer lateness 10 ms.

A Windows pass is not acceptance: the run does not include the Python and Chrome e2e suites, the thread-count
assertions or the ignored tests.
