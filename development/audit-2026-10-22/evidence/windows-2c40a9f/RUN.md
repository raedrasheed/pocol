# Windows verification of 2c40a9f

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40525606586,
  https://github.com/palcertcom/pocol/actions/runs/40525606586
- Trigger: pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- Commit under test: `2c40a9ff670cc9af79591c2048810609961c145d`, not dirty.
- Job 122269715314: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  15:03:34 to 15:27:28 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 201/0, pocold 237/0
  (1,075.6 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-2c40a9ff670cc9af79591c2048810609961c145d`, id 12745363272, 25,122 bytes,
  sha256 `c554984a4ec586960b5c2ecd9016c2a87c99e6b95e7e283e95ab545ea74dfc48` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is 2839f2a's. 2c40a9f still has the pacer test as it was before 6a1e480, so this run
prints no PACER line. It passed that test, as earlier runs did; `windows-182d73a` is the run that failed it.
ABSORB reads `kernelTookWholeAnswer:false`. A Windows pass is not acceptance. The run does not include the Python
and Chrome e2e suites, the thread-count assertions, or the ignored tests.
