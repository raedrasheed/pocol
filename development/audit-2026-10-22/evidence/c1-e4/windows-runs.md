# Windows runs on the C-1 E4.4 commits (workflow audit-r2-windows-verify.yml)

The workflow runs on Windows Server 2022 with Rust 1.58.1 (lp1-node) and 1.99.1 (lp4, pocold), and Node
22.22.0. It does not run the Python or real-Chrome e2e suites, pocold's thread-count assertions, or the
`#[ignore]` tests (WINDOWS-VERIFICATION.md). A Windows pass is not acceptance.

| run | commit | result | what failed | cause | fixed in |
|---|---|---|---|---|---|
| 40163936714 | cc929a7 | FAIL: pocold 211 passed, 1 failed; lp1 12 + 150, lp4 193, extension 234 passed | `e4_checkpoint_open_releases_what_a_crash_left_in_the_store` | the test copied its data directory while its node was open. Windows refused to read the journal's LOCK file (OS error 33, a locked range) | 45d540f (test only: `copy_dir` skips LOCK; the node closes first) |
| 40166523072 | 45d540f | FAIL: pocold 211 passed, 1 failed; lp1 12 + 150, lp4 193, extension 234 passed | the same test | the test still held a clone of the node's `StateStore` handle when it copied the directory, so state.redb stayed open and locked (OS error 33). Linux does not enforce redb's file lock, so the copy passed there | 53a80e9 (test only: the handle is dropped with the node) |
| 40172836981 (run 61) | fc24969 | **PASS** on every check: lp1 12 + 150, lp4 193, pocold 221 passed and 0 failed, extension 234 passed | | | |
| 40175036400 (run 62) | f14f2c2 (documentation and evidence only; code as fc24969) | **PASS** | | | |

Both failures were in the test harness; no product code changed in either fix. In both runs every other
E4 test in those trees passed on Windows:
- `e4_checkpoint_survives_kills` (TerminateProcess at increasing delays);
- `e4_checkpoint_damage_falls_back_to_a_verified_state`;
- `e4_checkpoint_open_resumes_and_equals_replay_from_genesis`;
- `e4_a_store_node_answers_as_a_whole_state_node`;
- `e4_store_node_at_the_r_keep_boundary_answers_as_a_whole_state_node`;
- `e4_store_node_rebuilds_a_fork_below_k_hist_as_a_whole_state_node`.

Two were ignored by design: `e4_checkpoint_kill_child`, which is the kill test's child process, and the
spec-depth test.

In run 61 every E4 and E5 test passed on Windows, including those added after run 60:
- the sibling-record binding;
- the slimmed, offloaded and evicting resume;
- the protect-set resume;
- a missing node marking the store damaged;
- a store of another format being rebuilt;
- the shared empty state;
- a close leaving its versions to the next open.

The kill child and the spec-depth test are ignored there by design. Run 61's artifact digest is
sha256:2927accf6d45c4806f718d8c3207d79800c34520aee46234faa5521b1c387a90.

The crash-step test needs `--features crash-test`, which the workflow does not build, so it ran on Linux
only.
