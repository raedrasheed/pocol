# Final acceptance on fc24969 (Linux), 2026-10-25

- **Commit:** `fc249699bad145100bba2cfaa9687647c9ee123e`, prototypes tree
  `c3e04e931559fc4b4122dc254da72b1600ee2322` (HEAD).
- **Working tree:** HEAD's "dirty-prototypes 1" is the untracked `prototypes/pocold/target` symlink to the
  build directory, so the e2e scripts find the release binaries; no tracked file differed.
- **Logs:** each step's log starts with its command and start time and ends with its exit code; `summary`
  lists the exits in order.
- **Machine:** see ../README.md. Nothing else ran during these steps, apart from the deep-reorg tests at the
  end, which ran alongside the final mutation pass.

| step | result |
|---|---|
| lp4-release | **193 passed, 0 failed** (21 binaries) |
| pocold-release | **221 passed, 0 failed, 20 ignored** (18 binaries; the ignored tests are listed below) |
| pocold-crash-steps (`--features crash-test`) | **passed**: 21 live-import crash points and one in replay (E44-STEPS in the log) |
| pocold-kills-damage (`--nocapture`) | **passed**: 11 kills (E44-KILLS) and 17 damage cases (E44-DAMAGE). The panics printed are redb's and the store's own, caught by the damage paths |
| pocold-spec-depth-store (`--ignored`) | **passed**: store mode at the spec depth 2048/2049, 317.7 s |
| lp1-node-1.58.1 | 137 passed, **11 failed**, exit 101. All 11 are tests/store_journal.rs, which return UnsupportedPlatform: that atomic journal is Windows-only, as on the audited 090eae4. On Windows they pass (run 61: 12 of 12). **Not a pass on Linux.** |
| ext-build, ext-unit | exit 1: the new worktree had no `node_modules` (`npm ci` was not run). This is an environment fault, not a test result |
| ext-build-rerun, ext-unit-rerun | with `node_modules` linked from the main worktree (the extension tree is identical): build passed, **234 of 234** unit tests passed |
| pocold-bins | release binaries built |
| e2e-devnet | **PASS, 42 checks**: 3 nodes, a partition and its heal, relays, and replays from the journal |
| e2e-site | **PASS, 20 checks** |
| e2e-browser (real Chromium) | **PASS, 79 checks, 0 failed** |
| e2e-integrated (LP10, Chromium) | **PASS, 18 checks, 0 failed**, over at least 100 live blocks |
| pocold-ignored-* (`--ignored`, alongside the mutation pass) | **passed**: `adoption_at_the_spec_depth_2048_and_not_at_2049`, `d133_tie_break_at_depth_400_holds_through_eviction_and_refetch`, `a11_deep_reorg_restores_the_oldest_detached_transaction` |

**What these runs exercise.** Store mode has no command-line switch; it is `Limits::state_store` only. So:
- the network, site and Chrome e2e runs exercise the default whole-state mode, together with E5's changes
  that apply to every mode (the shared empty state);
- store mode is exercised by the Rust tests above.

**Ignored in pocold-release, and whether they ran:**
- **Ran on fc24969 elsewhere in this pass:**
  - `e4_store_node_at_the_spec_depth_2048_and_2049_answers_as_a_whole_state_node` (pocold-spec-depth-store);
  - `e4_checkpoint_kill_child` (the child process of `e4_checkpoint_survives_kills`);
  - `adoption_at_the_spec_depth_2048_and_not_at_2049`, `d133_tie_break_at_depth_400_holds_through_eviction_and_refetch`
    and `a11_deep_reorg_restores_the_oldest_detached_transaction` (pocold-ignored-*).
- **Ran on 5ab73d1** (one comment from fc24969): `jm_store_mode_disk_impact` (../disk-final-*.log).
- **Not run in this pass** (they are measurements that assert nothing):
  - `d7_pool_revalidation_hold_under_a_heavy_pool`;
  - `history_integrity_check_cost`;
  - `jm_create_state_term`, `jm_deep_fork_rebuild_vs_height`, `jm_deep_reorg_latency`, `jm_exec_cost_per_gas`;
  - `jm_open_memory`, `jm_rebuild_by_kind`, `jm_replay_by_kind`, `jm_replay_component_split`;
  - `jm_replay_empty_blocks`, `jm_state_root_synthetic`, `jm_state_size_term`, `jm_tx_state_term`.

  Their last runs are in evidence/journal-r3*, d7-pool and c1-s1.

**Not run in this pass at all:**
- the debug-mode suites;
- the witnesses;
- the D-REORG-REDB and RpcGuard mutation passes;
- the RSS run under load;
- the R14/R19 calibration.

None of these is counted as passed. Their last runs are recorded in FINDING-STATUS.json.
