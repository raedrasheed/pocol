# Windows verification of 2839f2a

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40510911424 (run number 3 in that repository),
  https://github.com/palcertcom/pocol/actions/runs/40510911424
- Trigger: pull_request on `claude/memory-r14-followup` into `claude/affectionate-pascal-rjhh2k`
  (palcertcom/pocol#2, draft).
- Commit under test: `2839f2ac87b3eaf86bd9025e0c867b24f54b97c1`, not dirty.
- Job: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  13:22:21 to 13:47:23 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 200/0, pocold 236/0
  (1,058.3 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-2839f2ac87b3eaf86bd9025e0c867b24f54b97c1`, id 12738139359, 25,092 bytes,
  sha256 `3a735c4111f678a342f536864a8284f034cd4a450979e19ff51abb86099aa81c` (the digest GitHub reports and the digest of the downloaded file). Its contents are this
  directory, unchanged.

The node code under test is 2839f2a's own (892dca6's reductions and the store-mode import fix). A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the
thread-count assertions, or the ignored tests.
