# Windows verification of f65b44a

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40507248611 (run number 2 in that repository),
  https://github.com/palcertcom/pocol/actions/runs/40507248611
- Trigger: pull_request on `claude/memory-r14-followup` into `claude/affectionate-pascal-rjhh2k`
  (palcertcom/pocol#2, draft).
- Commit under test: `f65b44ab2cb8500c49b1e516c1bd3f3f7c4cfd5d`, not dirty.
- Job: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  12:52:52 to 13:22:01 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 200/0, pocold 235/0
  (1,307.9 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-f65b44ab2cb8500c49b1e516c1bd3f3f7c4cfd5d`, id 12736504715, 25,106 bytes,
  sha256 `7b8fd53821f78346a82706b0c3e0634e5dc4306ccce3b3b713f1c159dac7e075` (the digest GitHub reports and the digest of the downloaded file). Its contents are this
  directory, unchanged.

The node code under test is 892dca6's (the four reductions); f65b44a adds documents and evidence only. A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the
thread-count assertions, or the ignored tests.
