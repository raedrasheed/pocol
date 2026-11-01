# Windows verification of ad301e7 (node code: the independent review's fixes)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40595988585, https://github.com/palcertcom/pocol/actions/runs/40595988585
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `ad301e752b596e42d98aaad8abeca3f3b946ccc1`, not dirty.
- **Run:** 22:35:31 to 23:07:15 UTC on 2026-10-27.
- **Result: PASS on every step:**
  - lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, pocold 244/0;
  - extension npm ci, build, and unit tests 234/0.
- **Not run on Windows:** `a_release_step_that_fails_on_io_leaves_the_store_whole` is Unix only (RLIMIT_FSIZE).
- **Note:** The damage test passed here with the defect `e453851` fixes still present: its run chose a node that is not owed.
- **Artifact** `audit-r2-windows-ad301e752b596e42d98aaad8abeca3f3b946ccc1`, id 12778770135, 25,700 bytes, sha256 `937d940919d87ab9f5f73296eb2cebfd415ec80ebf93a725653710d0a7eeace6`: the digest GitHub reports, and
  the digest of the downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client:** client window 0.505 s, first byte 732 ms
  against an honest reader's 748 ms, pacer lateness 2 ms.

A Windows pass is not acceptance: the run does not include the Python and Chrome e2e suites, the thread-count
assertions or the ignored tests.
