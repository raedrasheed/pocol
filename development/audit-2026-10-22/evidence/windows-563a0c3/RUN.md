# Windows verification of 563a0c3

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40535963171,
  https://github.com/palcertcom/pocol/actions/runs/40535963171
- Trigger: pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- Commit under test: `563a0c35a6f03d2320badeef0facd24b32a25583`, not dirty.
- Job 122302991830: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  15:59:48 to 16:20:14 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 201/0, pocold 238/0
  (928.3 s; the network-gates test of `1babbc7` included), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-563a0c35a6f03d2320badeef0facd24b32a25583`, id 12750745784, 25,179 bytes,
  sha256 `e997e6fa03d9bf1be8c7a1357ca9f6515bc7163747e9e3e950e8dc57da8e3fa3` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is 2839f2a's. PACER (`logs/pocold.log`): send time 0.501655 s, the kernel took 32,940 B,
s_v = 0.5 s, the close 1 ms after s_v (ENFORCE_SLACK 200 ms), 10 waits, 961 ms after the request. ABSORB
`kernelTookWholeAnswer:false`.

A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the thread-count
assertions, or the ignored tests.
