# Windows verification of d45dfe2 (the wide MODEXP path)

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40546902481,
  https://github.com/palcertcom/pocol/actions/runs/40546902481
- Trigger: pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- Commit under test: `d45dfe215a22562e30ac9abd10a4d44cde46f485`, not dirty.
- Job 122339817873: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  17:03:32 to 17:26:31 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 208/0, pocold 238/0
  (934.8 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-d45dfe215a22562e30ac9abd10a4d44cde46f485`, id 12754117677, 25,444 bytes,
  sha256 `2f622767026e1709b9bcd9b8e9f02303573312a333b8d43c56d40f621ebde49f` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is `0c2ad75`'s: the MODEXP precompile with the one-word and the wide even paths. lp4's
208 include the seven `modexp::tests` (`logs/lp4-execution.log`); `wrapper_overhead_interleaved` is an ignored
measurement. `b8fb7ab` (trivial powers handed to revm) is not in this run.

PACER (`logs/pocold.log`): send time 0.502314 s, the kernel took 32,940 B, s_v = 0.5 s, the close 2 ms after s_v
(ENFORCE_SLACK 200 ms), 9 waits, 1,256 ms after the request. ABSORB `kernelTookWholeAnswer:false`.

A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the thread-count
assertions, or the ignored tests.
