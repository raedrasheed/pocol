# Windows verification of b66ba65 (the first Windows run of the MODEXP precompile)

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40538722157,
  https://github.com/palcertcom/pocol/actions/runs/40538722157
- Trigger: pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- Commit under test: `b66ba65136983b562329f35e63a5306396e2ddea`, not dirty.
- Job 122315186720: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  16:20:32 to 16:45:06 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 205/0, pocold 238/0
  (991.6 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-b66ba65136983b562329f35e63a5306396e2ddea`, id 12751786738, 25,278 bytes,
  sha256 `0761041b8ef7cbea04df88057a185e92714045ed807ac2289dd06cc1a2accf7f` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is `b46658a`'s: the one-word MODEXP path at address 5. lp4's 205 include the four
`modexp::tests` (`logs/lp4-execution.log`): revm's results on 60,000 random inputs, 20,000 one-word moduli, the
measured shapes, and the engine's address 5. The wide even path (`0c2ad75`) is not in this run.

PACER (`logs/pocold.log`): send time 0.510413 s, the kernel took 32,940 B, s_v = 0.5 s, the close 10 ms after s_v
(ENFORCE_SLACK 200 ms), 10 waits, 1,243 ms after the request. ABSORB `kernelTookWholeAnswer:false`.

A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the thread-count
assertions, or the ignored tests.
