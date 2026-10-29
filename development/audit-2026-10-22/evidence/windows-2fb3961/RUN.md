# Windows verification of 2fb3961

- Repository: `palcertcom/pocol` (the working repository since 2026-10-27; a mirror of
  raedrasheed/Proof-of-Collaboration#27's branch).
- Workflow: audit-r2-windows-verify.yml, run 40499609598 (run number 1 in that repository),
  https://github.com/palcertcom/pocol/actions/runs/40499609598
- Trigger: pull_request on `claude/audit-repair-a01-a14` into `claude/affectionate-pascal-rjhh2k`
  (palcertcom/pocol#1, merged afterwards as `13f1eac` at the owner's direction; the merge is not acceptance).
- Commit under test: `2fb396155061dfff8433754300e954337437583c`, not dirty.
- Job: Windows Server 2022, Rust 1.58.1 / 1.99.1, Node 22.22.0; 11:54:14 to 12:21:20 UTC on 2026-10-27.
- Result: **PASS on every step**. lp1 store journal 12/0, lp1-node 150/0, lp4-execution 196/0, pocold 233/0
  (1,168.5 s), extension npm ci, build, and unit tests 234/0.
- Artifact `audit-r2-windows-2fb396155061dfff8433754300e954337437583c`, id 12733527815, 24,965 bytes,
  sha256 `1dfe97272261cb6ca24aa6c9d271ec92c6304d237994881c7b2b8849be126c13` (the digest GitHub reports and the
  digest of the downloaded file). Its contents are this directory, unchanged.

The node code under test is 24cc167's; 2fb3961 adds documents and evidence only. A Windows pass is not
acceptance. The run does not include the Python and Chrome e2e suites, the thread-count assertions, or the
ignored tests.
