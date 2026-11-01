# Windows verification of b8fb7ab (MODEXP: trivial powers handed to revm)

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40550009922,
  https://github.com/palcertcom/pocol/actions/runs/40550009922
- Trigger: pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- Commit under test: `b8fb7ab065e1a7def01ec56596db3cb979aa0b26`, not dirty.
- Job 122353068139: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  17:26:54 to 17:52:19 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 209/0, pocold 238/0
  (1,071.5 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-b8fb7ab065e1a7def01ec56596db3cb979aa0b26`, id 12755882596, 25,449 bytes,
  sha256 `1a6a6d0ec66bb6981fceb767b9e0194fe708459ebf811f88f9ddb4a83dec2443` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is `b8fb7ab`'s: the MODEXP precompile with both fast paths, and the trivial powers
on wide even moduli handed to revm (the review's finding 1). lp4's 209 include the eight `modexp::tests`.

PACER (`logs/pocold.log`): send time 0.501535 s, the kernel took 32,940 B, the close 1 ms after s_v
(ENFORCE_SLACK 200 ms), 10 waits, 1,257 ms after the request. ABSORB `kernelTookWholeAnswer:false`.

A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the thread-count
assertions, or the ignored tests.
