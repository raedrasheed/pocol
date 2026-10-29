# Mutation controls on the D-REORG-REDB final code

Run: `3342af386571` (2026-10-23T08:23:03Z), isolated git worktree, release build per crate, runner
`run_mutations_redb.py` (kept with the private package; it only removes fixes, with no exploit payload).
For each control, the named test passes on clean code (positive control), every anchor of the edit
occurs exactly once, and the mutant run must FAIL with the test actually executed. A run that matches
zero tests is an error. A TIMEOUT counts as incomplete, never as a catch.

**Result: 30 of 30 caught.** Machine-readable form: `mutations-redb.json`. Full
output: `mutations-redb-3342af3.out`.

| id | test | control | mutant | result | what the mutation removes |
|---|---|---|---|---|---|
| MR1-SOLICIT | `side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again` | OK | FAILED | **CAUGHT** | a solicited (session) block that retention removed is refused like an unsolicited copy |
| MR2-PROTECT | `a_session_branch_is_adopted_through_a_flood_of_competing_side_branches` | OK | FAILED | **CAUGHT** | the side cap may evict session-protected blocks |
| MR3-REIMPORT | `replay_gives_a_reimported_block_a_new_import_seq` | OK | FAILED | **CAUGHT** | a re-imported block keeps its old importSeq on replay (contradicts consensus.md:205) |
| MR4-RESTORE | `r_keep_boundary_adopts_at_depth_r_keep_and_keeps_the_valid_chain_beyond` | OK | FAILED | **CAUGHT** | adopted blocks offloaded as side blocks are not restored |
| MR5-REPLAYPROTECT | `a_session_branch_is_adopted_through_a_flood_of_competing_side_branches` | OK | FAILED | **CAUGHT** | replay does not protect blocks that later records build on |
| MR6-PROTECTCANON | `a_long_catch_up_is_not_bounded_by_the_protected_branch_size` | OK | FAILED | **CAUGHT** | canonical session blocks count toward protect_max (both the import and the retain site) |
| MR7-RKEEP | `the_r_keep_boundary_removes_a_side_block_exactly_r_keep_below_the_head` | OK | FAILED | **CAUGHT** | R_keep boundary off by one (depth R_keep+1 adopted) |
| MR8-OFFLOADCANON | `a_crash_around_an_adoption_reopens_on_one_whole_chain` | OK | FAILED | **CAUGHT** | canonical blocks are offloaded too (RPC history changes) |
| MR9-SERVE | `an_offloaded_block_is_served_viewed_and_re_executed_from_the_journal` | OK | FAILED | **CAUGHT** | an offloaded block is served from its emptied in-memory entry |
| MR10-SIMULATE | `a_call_on_a_pruned_state_is_32017_not_an_empty_state_simulation` | OK | FAILED | **CAUGHT** | eth_call simulates on an empty (pruned) state |
| MR11-KEY | `d133_tie_break_at_depth_holds_through_eviction_and_refetch` | OK | FAILED | **CAUGHT** | the D133 key ignores signed-before-fallback |
| MR12-SESSIONFLAG | `a_sync_session_refetches_an_evicted_branch_and_serves_it` | OK | FAILED | **CAUGHT** | session blocks arriving over the network are not treated as solicited |
| MR13-RETAINLOOP | `side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again` | OK | FAILED | **CAUGHT** | one-pass leaf trimming: a newly exposed parent leaf is not re-queued |
| MR14-EVICTGUARD | `side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again` | OK | FAILED | **CAUGHT** | an evicted block received unsolicited is validated and journaled again |
| MR15-SIDEPRUNE | `a_side_branch_below_the_k_hist_window_imports_without_rebuilding_each_parent` | OK | FAILED | **CAUGHT** | the K_hist prune also drops side states (quadratic rebuilds for a deep competing branch) |
| MR16-PROGRESS | `a_peer_with_a_false_status_cannot_hold_the_sync_session` | OK | FAILED | **CAUGHT** | any stored copy (duplicates included) counts as session progress |
| MR18-RKEEPFILE | `replay_uses_the_largest_r_keep_the_data_directory_has_used` | OK | FAILED | **CAUGHT** | replay uses the configured R_keep even when live used a larger one |
| MR19-BELOWWINDOW | `r_keep_boundary_adopts_at_depth_r_keep_and_keeps_the_valid_chain_beyond` | OK | FAILED | **CAUGHT** | a new block below the R_keep window is validated and journaled, then removed |
| MR20-ORPHANSOLICIT | `a_session_block_arriving_before_its_parent_stays_protected` | OK | FAILED | **CAUGHT** | an orphaned session block loses its solicited status |
| M5-DEADLINE | `rpc_work_deadline_skips_expired_work_and_releases_resources` | OK | FAILED | **CAUGHT** | expired batch members are run, not skipped with -32020 |
| M4-BUSY | `a_full_heavy_pool_answers_busy_before_anything_runs` | OK | FAILED | **CAUGHT** | a response reservation never fails (the pool is unbounded) |
| M13-RESV | `an_answer_is_reserved_until_its_connection_has_written_it` | OK | FAILED | **CAUGHT** | the reservation is released before the answer is written |
| M14-BUSYPARSE | `a_busy_answer_does_not_parse_an_over_limit_body` | OK | FAILED | **CAUGHT** | the busy path parses the body unconditionally |
| M15-TXRAW | `a_max_size_transaction_is_answered_in_the_light_lane` | OK | FAILED | **CAUGHT** | tx_json carries raw beside input (~4x TX_MAX) |
| M3-QUEUECAP | `abandoned_queued_bodies_are_capped_and_released` | OK | FAILED | **CAUGHT** | no cap on queued heavy bodies |
| M7-LIGHTTURN | `a_light_turn_runs_at_most_light_turn_requests_then_wakes_the_node` | OK | FAILED | **CAUGHT** | no LIGHT_TURN bound |
| M1b-WAKE | `light_lane_wakes_coalesce_until_taken` | OK | FAILED | **CAUGHT** | a Wake for every light request (no coalescing) |
| F1-CLOSE | `a_close_between_the_writers_test_and_its_wait_wakes_it` | OK | FAILED | **CAUGHT** | close without the queue lock (the old racy code) |
| M2-MINER | `miner_replacement_keeps_at_most_one_miner_thread` | OK | FAILED | **CAUGHT** | the old miner handle is never dropped/joined (two LIVE at once) |
| F8-DIRTY | `d_a03_dirty_vectors` | OK | FAILED | **CAUGHT** | report DirtyKeys where the block meter halts on dirty accounts |

## Superseded partial runs (kept unchanged, `mutations-redb-superseded/`)

- `mutations-redb-9ce5a7c-partial.out`: 21 of the 26 controls then defined ran before the run was stopped for the
  second review's fixes. 20 were CAUGHT. **MR5-REPLAYPROTECT was not caught (mutant OK)**: the flood
  test then used siblings of an older block, so replay evicted those and never needed its
  child-record protection. Corrected in `f2477e7` (flood siblings of the head); MR5 is caught above.
- `mutations-redb-441ce73-partial.out`: 9 controls ran. **MR3-REIMPORT was COMPILE-FAIL**: its anchor
  was stale after `reimport` gained the copy and offset arguments, so it was retargeted. **MR7-RKEEP was
  not caught**: the import-time refusal of blocks below the window masked retain()'s boundary. A
  dedicated boundary test was added in `3342af3`. Both are caught above.

Not separately controlled: the first-Hello-only session start. A mutant would only win a timing race
against the round-robin tick, so no deterministic test distinguishes it; the progress rule (MR16) is
the load-bearing control. The locator's depth-R_keep entry is an efficiency property (a deeper common
point only adds duplicate rounds), not a correctness one.
