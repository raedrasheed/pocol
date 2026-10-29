# Windows verification of 69d5794

- Repository: `palcertcom/pocol`.
- Workflow: audit-r2-windows-verify.yml, run 40518244446 (run number 7 in that repository),
  https://github.com/palcertcom/pocol/actions/runs/40518244446
- Trigger: pull_request on `claude/memory-r14-followup` into `claude/affectionate-pascal-rjhh2k`
  (palcertcom/pocol#2, draft).
- Commit under test: `69d579494d5ba2ade59b79823f0dcc18c10f648c`, not dirty.
- Job: Windows Server 2022 (runner image win22 20261022.344.1), Rust 1.58.1 / 1.99.1, Node 22.22.0;
  14:10:25 to 14:36:30 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 201/0, pocold 237/0
  (1,192.0 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-69d579494d5ba2ade59b79823f0dcc18c10f648c`, id 12742951082, 25,143 bytes,
  sha256 `7e7b364e342a9a17f6fc8c0b63e567510e1f6d103a06400d12ff5a904f2395c8` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is 2839f2a's. 69d5794 adds two tests against it: `a13_row_is_what_state_redb_keeps_per_node`
(8d80354), which is in pocold's 237, and `a_blocks_addition_is_at_most_a_whole_version`, which is in lp4's 201.
A Windows pass is not acceptance. The run does not include the Python and Chrome e2e suites, the thread-count
assertions, or the ignored tests.
