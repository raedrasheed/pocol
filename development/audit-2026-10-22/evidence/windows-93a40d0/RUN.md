# Windows verification of 93a40d0 (node code: D-6's deferred checkpoint release, steps not fsynced)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40585164582, https://github.com/palcertcom/pocol/actions/runs/40585164582
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `93a40d01ed7c2d25e137922f4d692cb9a9f0cad2`, not dirty.
- **Run:** 21:15:02 to 21:54:37 UTC on 2026-10-27.
- **Result: PASS on every step:**
  - lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, **pocold 240/0** (the deferred-release tests
    included);
  - extension npm ci, build, and unit tests 234/0.
- **Artifact** `audit-r2-windows-93a40d01ed7c2d25e137922f4d692cb9a9f0cad2`, id 12774091121, 25,562 bytes, sha256
  `83c309f2671bbf8d73f51bd6eac49c2df7ca43e587cc716594b72c9d959b711b`. That is the digest GitHub reports, and the
  digest of the downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client:** client window 0.506 s, first byte 742 ms against an honest reader's
  727 ms, pacer lateness 3 ms.

A Windows pass is not acceptance: the run does not include the Python and Chrome e2e suites, the thread-count
assertions or the ignored tests.
