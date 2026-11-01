# Windows verification of e453851 (node code: open sets aside a store it finds damaged)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40600109673, https://github.com/palcertcom/pocol/actions/runs/40600109673
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `e4538518d9e4770d83b473231d9c837951dfa81b`, not dirty. It holds every node change of the deferred release and its fixes:
  `93a40d0`, `22a82c2`, `ad301e7` and `e453851`.
- **Run:** 23:09:13 to 23:39:45 UTC on 2026-10-27.
- **Result: PASS on every step:**
  - lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, **pocold 245/0**;
  - extension npm ci, build, and unit tests 234/0.
- **The new tests pass on Windows** (`logs/pocold.log`):
  - `e4_checkpoint_open_rebuilds_a_store_its_release_finds_damaged`;
  - `e4_release_steps_alternate_with_pool_slices`;
  - `e4_checkpoint_damage_falls_back_to_a_verified_state`, the test that failed on `22a82c2`.
- **Not run on Windows:** `a_release_step_that_fails_on_io_leaves_the_store_whole` is Unix only (RLIMIT_FSIZE).
- **Artifact** `audit-r2-windows-e4538518d9e4770d83b473231d9c837951dfa81b`, id 12779298538, 25,686 bytes, sha256 `a2872790dc602b6b6bfe33f82a5f01dcdb44875e3565e7a59d7e8e87d2533e39`: the digest GitHub reports, and
  the digest of the downloaded file. Its contents are this directory, unchanged.
- **The pacer seen from the client:** client window 0.503 s, first byte 762 ms
  against an honest reader's 743 ms, pacer lateness 2 ms.

A Windows pass is not acceptance: the run does not include the Python and Chrome e2e suites, the thread-count
assertions or the ignored tests.
