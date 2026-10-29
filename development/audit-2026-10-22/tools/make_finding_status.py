#!/usr/bin/env python3
"""Writes development/audit-2026-10-22/FINDING-STATUS.json (round 2; round-3 update of 2026-10-24).

Each finding is classified on five separate axes, never folded into one "closed" label:
  implemented         YES | PARTIAL                 the repair code exists on the branch
  verified            VERIFIED (Linux and Windows CI) | PARTIAL
                                                    regression tests, mutation controls and the reconstructed
                                                    witness (with its positive control) on the recorded commits;
                                                    the Windows CI record is in verified.windows
  partiallyResolved   []                            requirements still technically incomplete
  blocked             {environment: [], decision: []} what only another environment, or an owner decision, can clear
  accepted            NOT ACCEPTED                  acceptance belongs to the owner; nothing is accepted here
The disposition is the weakest axis: PARTIALLY RESOLVED, then BLOCKED BY DECISION, then
VERIFIED (Linux and Windows CI) — BLOCKED BY ENVIRONMENT (Windows e2e and thread counts).
Until round 3 the last label read "VERIFIED (Linux) — BLOCKED BY ENVIRONMENT (Windows)", because
Windows had not run; the Windows CI has run since 2026-10-23 (WINDOWS-VERIFICATION.md).

usage: python3 make_finding_status.py <commits.json>
commits.json: {"prHead", "lastImplementation", "integrationTested", "witnessTested", "chromeTested", "windowsTested"}
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE)
# Round 3: the Windows CI has run. WIN is what it verified; WIN_GAP is what it does not cover.
WIN = ("Windows CI (audit-r2-windows-verify.yml, a pull_request trigger limited to PRs into claude/affectionate-pascal-rjhh2k; WINDOWS-VERIFICATION.md): "
       "runs 1-7 found and fixed four Windows defects (a retention.rkeep directory fsync Windows refuses; a dropped P2P peer's reader not woken by shutdown; "
       "a Nagle/delayed-ACK stall of about 200 ms on every RPC answer; kernel absorption of unread RPC answers, fixed by SO_SNDBUF 32 KiB plus a write cap of SEND_SOCK_BUF per call). "
       "PASS on every check: run 9 (29fd44a), run 16 (25bec25, the first run of the round-3 poll/WSAPoll code), run 19 (2b1a571), run 21 (1cc96c4) and run 22 (c959379) and run 26 (43e7e0f, code 462fffc, the last code commit). "
       "Run 20 (49ae9fa, same code as run 19) failed one timing test, abandoned_queued_bodies_are_capped_and_released; 1cc96c4 changed only that test's settings, and run 21 passed")
WIN_GAP = ("Not covered on Windows: the real-Chrome and Python e2e suites (not in the workflow), pocold's thread-count assertions (they read /proc), and the #[ignore] measurement tests")
WIT = "reconstructed witness (independently written from REPORT.md; the original audit programs were not received)"
NOREV = ("round-2 changes reviewed by independent agent instances (two passes: the F1-F9 review of df63725 and the review of the D-A13-BUDGET range); "
         "round 3: per-track reviews by separate agent instances (caps: three; RpcGuard: three; journal: four; e2e: three); the last pass of each track (caps r3b, RpcGuard r3c, journal r3c, e2e round 4) is not reviewed yet; no external human audit")
DV = "VERIFIED (Linux and Windows CI) — BLOCKED BY ENVIRONMENT (Windows e2e and thread counts)"

F = [
 ("A01", "HIGH", "Valid block hash poisoning through an unbound body", ["ac3a951", "d132203"],
  ["consensus::a01_malformed_or_unbound_bodies_reject_only_the_copy", "consensus::a01_committed_invalid_body_is_still_a_block_rejection_and_cached",
   "consensus::a_valid_header_with_a_foreign_body_rejects_only_that_copy", "consensus::body_max_bounds_the_encoded_body_not_the_item_sum", "consensus::d_body_max_vectors"],
  [], [], ["D-BODY-MAX (encoded body on every path; found while reviewing this repair)"]),
 ("A02", "HIGH", "ParamGate not enforced before boot", ["10678de", "d132203", "33f98b7", "77d6984"],
  ["admission::tests (R1–R26 at their boundaries; audit reproductions)", "admission::tests::{r26_predicates_are_the_gate_conjunction, absent_rpc_fields_enter_the_gate_as_zero_never_nominal, specification_scope_refuses_every_open_condition, experimental_devnet_waives_only_r26_predicates_reading_absent_fields}",
   "pocold admission::pocold_admission_is_an_experimental_devnet_with_absent_rpc_mechanisms", "compile_fail doc tests (no bypass constructor)",
   "admission::tests: the experimental scope admits only ProfileKind::Simnet (Synthetic and Network refused, review F5)", "RPC_FIELDS derived with an exhaustive RpcLimits pattern (review F6; omitting a field fails to compile)"],
  ["Only an experimental-devnet admission is possible: Specification scope refuses every current profile (rpc-absent, calibration-synthetic, timing-not-a-bound, gas-monotonicity-assumed, gate-deferred LP3 items)",
   "R14/R19 timing is a single measured sample on the booting host, not a p99 or worst-case bound; under CPU contention a debug build fails closed [R14, R19]",
   "R18 gas facts assume monotonicity in the domain fields (A13 §4 item 1)",
   "R26: 4 RpcGuard fields are absent (scratch_heavy, scratch_light, mem_total, cancel_max_ms) and waived only in the experimental scope; 25 are present at their running values, 6 of them by an equivalent bound named in the admission notes (heavy_active, light_active, conn_buf, light_resp_mem, rate, bucket_cap) (RPCGUARD.md §8; round 3)"],
  [], ["D-A02-PROFILE (devnet parameters; engineering, approved under the standing authorization)"]),
 ("A03", "HIGH", "Runtime log and state limits not enforced", ["10678de", "07ac864", "b132a05", "a797c83"],
  ["apply::tests::a03::* (LOG_BLOCK_MAX, LG2, topics, cumulative, LOG_CAP, dirty keys/accounts, caps, halt semantics)", "apply::tests::a03::proposals::{log_acct_01_vectors, d_a03_dirty_vectors (status, halt kind and dirty-account count, incl. the block-invalid residue case), d_a03_caps_vectors, log_cap_aggregate_vectors}",
   "consensus::a03_over_limit_log_halts_the_transaction_and_the_block_is_valid",
   "caps track (round 3): pocold tests/log_cap.rs in the caps-test build (LOG_CAP = 2^24, simnet only, validation.md:2065): the chain's log bytes are filled to exactly 2^24, then a logging tx halts (status 0, whole gas limit charged), transfers, plain creations and silent calls succeed, and 20 more blocks keep the escrow invariant (T17); caps-test build 11/0 on 2b1a571 (evidence/integration-r3/capstest-2b1a571.log)",
   "caps track: independent genesisHash model (proposals/model/caps_model.py, own Keccak and GenesisSpec encoder, 25 checks; lp4 tests/caps_identity.rs): LOG_CAP is not encoded in GenesisSpec, so nodes with different LOG_CAP share a genesisHash; a LOG_CAP change is still a hard fork, and a mismatch is a silent split at check 17 (argued from code; no two-node run)"],
  [], [], ["LOG-ACCT-01", "D-A03-DIRTY (including the block-invalid halt-residue case)", "D-A03-CAPS", "D-A03-SYSUSER", "LOG-CAP-AGGREGATE",
           "CAPS-SUSTAINABILITY N1 (LOG_CAP = 2^32 with gate rule R27, = LOG-CAP-AGGREGATE-ALTERNATIVES A1a) and N7 (eth_call/eth_estimateGas report an A03 halt): PROPOSED, not approved"]),
 ("A04", "HIGH", "Journal frame corruption drops committed records", ["20bedce", "d132203", "cedabe7"],
  ["store::a04_* (every single-bit flip fails closed; truncation; boundary edits; commit-file damage; uncommitted tail; v1 migration; 4 KiB slots)",
   "Windows CI (round 3): the pocold store tests (journal v2) and lp1-node store_journal, the Windows-only journal, 12 passed (runs 9-21; WINDOWS-VERIFICATION.md)"],
  ["Real power loss is not tested (simulated by file edits only)",
   "On Windows a rename is not followed by a directory flush (store::sync_dir and retention.rkeep skip it; WINDOWS-VERIFICATION.md run 1)"], [], ["D-A04-FORMAT journal v2 (local storage format, engineering)"]),
 ("A05", "HIGH", "P2P and RPC resource exhaustion", ["a2dbf16", "d132203", "cedabe7", "33f98b7", "45712cd", "77d6984", "b132a05", "a5e2517",
   "ad71484", "6a561b6", "dc4e3d1", "a39379b", "be95e26", "29fd44a", "0f48e6e", "65a828b", "1530d7f", "8039a09", "d0e87f9", "dd7d1ec", "1cc96c4", "af2f3a8"],
  ["net::* (idle/hello deadlines, frames, slow reader, churn, inbound floods, connection cap, batches)", "net::rpc_work_deadline_skips_expired_work_and_releases_resources (deadline derived from the measured heavy duration; mutations: no skip, no batch deadline: each fails)",
   "net::legitimate_requests_progress_under_bounded_heavy_load", "net::a_light_turn_runs_at_most_light_turn_requests_then_wakes_the_node (mutation: no LIGHT_TURN bound fails)", "net::a_light_flood_coalesces_wakes_and_main_events_progress (mutation: a Wake per request fails)",
   "net::abandoned_queued_bodies_are_capped_and_released (mutation: no cap fails)", "net::a_full_heavy_pool_answers_busy_before_anything_runs (mutation: reservations never fail fails)", "rpc::tests::response_pools_reserve_shrink_and_release",
   "net::rpc_request_buffer_covers_headers_and_body (413 drained before close; its mutation is caught in release 2 of 3 runs, a race)", "net::tests::a_close_between_the_writers_test_and_its_wait_wakes_it (F1; mutation fails)",
   "net::miner_replacement_keeps_at_most_one_miner_thread (F7; mutation fails)", "net::peer_threads_stay_within_the_inventory_while_consensus_is_stalled (invariant check)",
   "net::light_lane_wakes_coalesce_until_taken (mutation fails)", "net::an_answer_is_reserved_until_its_connection_has_written_it (mutation fails)", "rpc::tests::a_busy_answer_does_not_parse_an_over_limit_body (mutation fails)",
   "consensus::a_max_size_transaction_is_answered_in_the_light_lane (mutation fails)",
   "RpcGuard (network.md:182-330, rounds 1-3c): per-item table in RPCGUARD.md (spec item, mechanism, ENFORCED / EQUIVALENT / PARTIAL / NOT ENFORCED, test, mutant); tests/net.rs among them queued_requests_are_busy_at_queue_wait, queue_wait_bounds_the_wait_for_a_heavy_queue_place, queue_wait_bounds_the_wait_on_a_full_main_queue, the_node_never_runs_a_request_its_connection_gave_up, compute_deadline_runs_from_admission_and_queue_wait_bounds_admission, a_late_revert_or_halt_past_its_deadline_is_32020, the_compute_deadline_includes_encoding, the_conditional_light_entries_have_the_light_deadline, a_light_method_answer_over_light_resp_max_is_32020_on_the_heavy_lane, send_pacer_closes_a_slow_reader_after_grace, send_pacer_timer_follows_a_over_r_past_grace, unread_default_client_absorption; rpc::tests::sockwait_reports_readiness_within_its_timeout; lp4 tests/simulate_stop.rs",
   "RpcGuard mutation controls: round 2 on dc4e3d1, 21 mutants, 20 caught, 1 equivalent; round 3 on d0e87f9, 59 mutants, 53 caught, 6 survivors explained; round 3c on dd7d1ec, 35 mutants, 30 caught, 5 survivors explained (2 equivalent, 2 within the spec's bounds, 1 test limit) (evidence/rpcguard-r3c/MUTATIONS-RPCGUARD-R3C.md)",
   "Review R2 fixes (af2f3a8, evidence/reviews-2026-10-24/r2-fixes/): net::a_batch_does_not_hold_p2p_events_for_its_whole_length (fail-before: a Hello 4.85 s into a 5.05 s batch), net::a_transaction_admission_past_its_compute_deadline_is_refused_and_not_pooled (3 mutants killed), net::guard_and_dispatch_error_items_stay_within_err_item_max (2 mutants killed), net::bytes_after_the_request_do_not_reset_the_answer (fail-before: a reset after the body), the REQ_READ_DEADLINE trickle in net::rpc_connection_cap_refuses_excess_unread_and_keeps_serving (reviewer mutant M13 killed)",
   "net::abandoned_queued_bodies_are_capped_and_released after 1cc96c4 (test settings only: HEAVY_QUEUE 2, QUEUE_WAIT 5 ms, up to 12 rounds; assertions unchanged): Linux 8/8 (evidence/integration-r3/abandoned-queued-8x-linux.log), Windows run 21 PASS"],
  ["RpcGuard is mapped item by item in RPCGUARD.md (ENFORCED, EQUIVALENT, PARTIAL, NOT ENFORCED). Not enforced or partial: SCRATCH outside the log handlers and light SCRATCH (absent), RPC_MEM_TOTAL as a field (absent; bounded by A13), CANCEL_MAX for precompile runs (absent; proposal P2), PacerTrace, chunked batch streaming, the numeric log range before admission, pocol_getLogs anchors, the unlisted-method rule (PARTIAL; proposal P4), the conditional light entries' lane (they run on the heavy lane, with the light deadline and answer limit), the aggregate rate across reconnects (proposal P1); RPC runs on the consensus thread (no separate runtime)",
   "A running request is preempted only inside EVM simulation (eth_call/eth_estimateGas: the stop hook is polled every 10^6 gas and before every call); a precompile run is uninterruptible (a 30M-gas BLAKE2F takes about 0.5 s), so CANCEL_MAX = 100 ms is not enforced; encoding is checked once, not every 1 MiB",
   "Effectful methods: pocol_submitShare is checked against COMPUTE_DEADLINE only before it runs (the P3 reading, option A). eth_sendRawTransaction's pool admission runs under the deadline since af2f3a8 (review R2-2): past it nothing is pooled"],
  [], ["RpcGuard implementation in pocold (owner approved: the 2026-10-23 instruction to complete the authorized works, RpcGuard and the Windows verification; a local service policy, D87, no validity effect)",
       "RPCGUARD P1 (aggregate rate across connections), P2 (CANCEL_MAX for precompile runs), P3 (effectful methods past COMPUTE_DEADLINE), P4 (methods outside the classification table) (proposals/RPCGUARD-R2-PROPOSALS.md): PROPOSED, not approved"]),
 ("A06", "HIGH", "Signing eligibility across wallet and profile transitions", ["51aa8f7", "4856842", "cedabe7", "df63725"],
  ["worker_test.mjs (A06 and A06-R2 submission states over a deterministic transport barrier; mutations: no re-check, no hook, submission under the mutex: each fails)", "e2e_browser.mjs real Chrome A06 and A06-R2 (lock, profile change and endpoint change while queued; lock after initiation)"],
  ["After a request is initiated it may be broadcast; it is reported as initiated/broadcast, never as cancelled (by design)"], [], ["D-A06-CONFLICT (engineering)", "D-A06-SUBMIT (changed expectation: a profile change during signing cancels the not-yet-initiated submission)"]),
 ("A07", "MEDIUM", "Extension GenesisSpec decoder accepts invalid specs", ["51aa8f7", "4856842"], ["tests/decoder_diff.mjs (1151 items vs Rust verdicts)", "pocold decoder_reference"], [], [], []),
 ("A08", "HIGH", "RP header structure not validated", ["51aa8f7"], ["decoder corpus (every UT field, seal, shares, winner signature)", "live RP mutations in e2e_browser.mjs"], [], [], []),
 ("A09", "MEDIUM", "Session receive reservations not atomic", ["51aa8f7"], ["worker_test.mjs A09 (peak <= cap; release on every path)"], [], [], []),
 ("A10", "MEDIUM", "ORIGIN_DENY not applied per batch member", ["a2dbf16"], ["net::a10_origin_deny_applies_to_every_batch_member"], [], [], []),
 ("A11", "MEDIUM", "Reorg drops detached transactions", ["a2dbf16", "d132203"], ["consensus::a11_reorg_restores_detached_transactions_without_resubmission", "consensus::a11_excludes_included_and_conflicting_transactions_and_keeps_order"], [], [], []),
 ("A12", "MEDIUM", "Storage admission released before the write settles", ["51aa8f7"], ["worker_test.mjs A12"], [], [], []),
 ("A13", "HIGH", "Resource derivations do not match the representation", ["25be2da", "d132203", "cedabe7", "33f98b7", "45712cd", "77d6984", "b132a05", "3c07cb5", "a5e2517", "66d69c6", "f7c345b", "54b2da1", "9ce5a7c", "ad71484", "6a561b6", "0f48e6e", "49b2bc2", "2ce03c1", "6e99b19", "af2f3a8"],
  ["resources::tests::{snapshot_model_bounds_measured_clone_bytes, execution_model_bounds_measured_adversarial_peaks (public cases), loaded_accounts_start_with_unallocated_storage_maps, sys_gas_envelope_fixture}", "lp4 tests/rss_snapshot.rs (full-cap snapshot clones: RSS 0.45 of the model)",
   "private adversarial execution measurements, each case against its own term (evidence/PRIVATE-REVIEW-PACKAGE.md; not in the public repository)", "consensus::{a13_* (side states, deep-fork rebuild, boot bound with separate default/below/exact/network-scale assertions, pool caps), a13_bound_breakdown}",
   "evidence/rss-*.json (measured RSS of a 3-node devnet under load: not worst-case evidence)", "A13-DERIVATION.md §2a, §2b, §4 (each assumption ENFORCED / DERIVED / INSPECTION / OPEN)",
   "consensus::a_competing_branch_within_side_blocks_max_is_adopted",
   "D-REORG-REDB (R_keep adoption): consensus::{adoption_at_the_spec_depth_2048_and_not_at_2049 and d133_tie_break_at_depth_400_holds_through_eviction_and_refetch (release, --ignored; evidence/spec-depth-9ce5a7c.log), r_keep_boundary_adopts_at_depth_r_keep_and_keeps_the_valid_chain_beyond, d133_tie_break_at_depth_holds_through_eviction_and_refetch, side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again, a_session_branch_is_adopted_through_a_flood_of_competing_side_branches, replay_gives_a_reimported_block_a_new_import_seq, an_offloaded_block_is_served_viewed_and_re_executed_from_the_journal, a_crash_around_an_adoption_reopens_on_one_whole_chain, a_sync_session_refetches_an_evicted_branch_and_serves_it, a_long_catch_up_is_not_bounded_by_the_protected_branch_size, a_side_branch_below_the_k_hist_window_imports_without_rebuilding_each_parent, a_call_on_a_pruned_state_is_32017_not_an_empty_state_simulation}; mutation controls evidence/mutations-redb.json",
   "consensus::a13_bound_breakdown asserts the model total on Linux x86_64: 5,581,426,268 B in round 3 (RPCGUARD.md §8), 5,487,223,178 B since C-1 S2 (code counted once per lineage: journal_measure::retained_states_of_one_lineage_share_their_code_buffers; A13-DERIVATION.md)",
   "Decision package C-2 items 1-3 (49b2bc2, 2ce03c1; evidence/c2-execution/): lp4 code_caps_engine_tests (known code total, once-per-phase check, revalidate_touched equals the full check), mutation controls; 380 transfers on the near-cap devnet state 22,553 ms before, 655 ms after (same host, same load)",
   "Review R3-3 (af2f3a8): consensus::replay_restores_each_adopted_branch_with_the_work_live_did (fail-before: 4 rebuilds and 76 re-executions at open against live's 0 and 2)",
   "tests/network_budget.rs: the network profile model, 150.83 GiB before C-1 S2 and 105.83 GiB after (gate 120.95 GiB); the test asserts it does not fit 6 GiB",
   "journal track, rounds 1-3c (JOURNAL-GROWTH-AND-REPLAY.md): #[ignore] measurements in tests/journal_measure.rs (JM-BYTES, JM-REPLAY, JM-KIND, JM-EXEC, JM-STATE, JM-CREATE, JM-TXSTATE, jm_deep_reorg_latency, JM-OPENMEM-FRESH) on code whose src equals fa97594's; they measure and assert nothing about time (evidence/journal-r3/ on 7f587bb, journal-r3b/ on e3dfb47, journal-r3c/ on 1d094de)"],
  ["OPEN: gas monotonicity in the domain fields (§4.1)",
   "OPEN: R14/R19 timing is not a worst-case bound (§4.2); measured (journal round 3, §3.2a): one valid one-tx full-gas MODEXP block (small even modulus) validates in about 1.0 s, above the devnet t_exec = 1 s",
   "OPEN: allocator overhead and fragmentation (§4.3b); the boot gate keeps a 1/8 reserve for them, which is a margin, not a proof; the model total (5,581,426,268 B = 5.198 GiB at the round-3 code, 8039a09 and later; 5,577,343,580 B = 5.194 GiB at fa97594; 5,627,687,516 B = 5.241 GiB at 9ce5a7c) counts requested bytes",
   "OPEN, CRITICAL, partly addressed (decision package C-2 items 1-3, 49b2bc2 and 2ce03c1: on the same near-cap state, 380 transfers 22,553 ms before and 655 ms after on one loaded host; a CREATE2 block 4,862 -> 562 ms; an empty child 777 -> 465 ms, the state root remaining; the MODEXP block unchanged; evidence/c2-execution/). As measured in round 3 (journal round 3c, JOURNAL-GROWTH-AND-REPLAY.md §3.2d): a valid block of 380 transfers validates in about 10.2 s on a devnet state with accounts, slots and code near the caps, above T_blk = 4 s. Causes: every USER transaction runs code_caps::validate_snapshot and two whole-snapshot clones (native::execute_for_profile, build_engine), and every scan keccaks every runtime (code_caps::code_len), so each scan is O(accounts + slots + logical code bytes). On the same state a CREATE2-heavy child takes about 2.4 s and an empty block about 0.41 s; the create term code_caps::current_total is 0.47-0.54 s per 247-create block at the devnet ACCT_CAP (§3.2c). Lower bounds on the worst case",
   "OPEN: CPU costs (world-root rebuild, chain scans, pool re-execution, deep-fork rebuild) (§4.5–4.7); the whole world root is recomputed on every block (about +0.23 s per block near the devnet SLOT_CAP/ACCT_CAP; about 14.4 s per block at network caps, synthetic; §3.2b); the devnet open at H_END takes up to about 7.3 h, plus about 8.5 min for the TX_CAP-bounded transfer blocks (§3.3); pool re-execution of 50 abandoned one-tx MODEXP transactions in an adopting reorg took about 50 s (§5.3)",
   "OPEN: RetentionStore and the redb trie are not implemented (§4.10); the specification's node targets <= 750 MiB plus 192 MiB RPC (network.md:158), pocold's in-memory architecture needs 5.198 GiB of model at the round-3 code (5.194 GiB at fa97594; 5.241 GiB at 9ce5a7c)",
   "OPEN (journal obligation, local, no validity effect): journal disk growth; the append-only journal has no compaction and grows with every import and re-fetch (D-REORG-REDB-DESIGN-FREEZE.md R2.6); canonical maximum about 11.3 GiB at network caps and about 0.22 GiB on the devnet; the solicited re-fetch vector re-journals evicted in-window blocks per cycle and, with the fork point below K_hist, also rebuilds from genesis per cycle (JOURNAL-GROWTH-AND-REPLAY.md §2, §4)",
   "OPEN (journal obligation): re-execution from genesis; open replays every record from genesis (Theta(H^2)); a deep-fork rebuild/restore below K_hist re-executes the fork point from genesis twice, because retain prunes the canonical fork point below head-K_hist: a depth-2048 reorg near the devnet tip takes 96 s on an empty chain and up to about 11 h under the worst measured kind (about 13 h with the state at the devnet caps), in two stalls that each exceed D_att (§5.3); no snapshot, reverse-diff or incremental-root mitigation is implemented",
   "OPEN (journal obligation) and CONDITION (not in the steady-state bound): the open-time memory transient; open-time replay holds the whole journal (store::open, pre-existing) and a two-entry-per-record replay index; the transient is about 1.0x the journal on top of the steady resident chain, peak 2.1-5.7x the journal by block kind (§3.4); at the network maximum or under the adversarial journal it exceeds the budget, so a node can be pushed into a state where it cannot restart",
   "The network profile is not admissible on pocold: the A13 history term TXBYTES_CAP = 8 GiB alone exceeds the 6 GiB budget, so every network figure is a projection, not a configuration this node boots",
   "History term (caps track): caps::LOG_CAP is 2^24 in the caps-test build (a smaller, still valid upper bound there); sizing the devnet TX caps to cover H_END (CAPS-SUSTAINABILITY O2) puts the model at 11.12-14.62 GiB at fa97594, which fails the 6 GiB gate unless canonical history moves off the heap (CAPS-SUSTAINABILITY §5b)",
   "INSPECTION only: transient-snapshot inventory, one EVM execution at a time, the answer-being-built tree ratio (§4.11, §4.13, §4.16)", "DERIVED with stated revm 19.7 facts: execution memory, operation classes (§4.4, §3a)",
   "No guaranteed process-memory ceiling exists (§2b)"], [], ["D-A13-BUDGET (owner approved option 1: local limits reduced, 6 GiB kept)", "D-REORG-REDB (owner approved 2026-10-23 as a storage repair for R_keep adoption; D133 and validity unchanged; §4.18 implemented)",
   "Journal mitigations (JOURNAL-GROWTH-AND-REPLAY.md §7: headers-first sync, dedupe markers, compaction behind a signed snapshot, reverse diffs, an O(window) retain and canonical index, keeping the fork-point state, an incremental state root with O(dirty) block checks, and bounding re-execution cost per gas): every one PROPOSED, none approved or implemented"]),
 ("A14", "LOW", "JSON-RPC responses not correlated with the request", ["51aa8f7", "d132203"], ["worker_test.mjs A14 (13 malformed shapes)", "rpc unit tests (-32020 {resultBytes} carries the request id; round 3, network.md:210; it was -32008 before)"], [], [], []),
]

# Round 3: a finding the repair itself found (not an audit finding, so no reproduction on 090eae4).
EXTRA = [
 {"id": "CAPS-01", "origin": "found during the repair (caps track, 2026-10-23); not an audit finding, so there is no reproduction on 090eae4",
  "severity": "not rated by the audit",
  "title": "The block builder ignored the chain-cumulative TX_CAP/TXBYTES_CAP: past exhaustion the node's own block was rejected '16 caps' and every height waited D_att+Γ for a fallback (74-78 s measured on the D_att=73 test devnet; about 30 s on the default devnet and about 330 s on the network profile, derived); starvation only when TX_CAP is below one block's count (tests only)",
  "normative": False, "conformance": "execution.md:83 (the builder skips past the remaining room); consensus.md:101 §4(a) (a violating template counts as absent); consensus.md:170 (item 16)",
  "repairCommits": ["bdb14bb", "9094f98", "c635594", "5965b8d", "e985ce1"],
  "implemented": "YES",
  "verified": {"status": "VERIFIED (Linux and Windows CI)",
               "tests": ["pocold tests/caps.rs: the builder tests, a reorg at the exhaustion boundary, template locking and live single-miner nodes; 6/6 fail on the unfixed builder (evidence/caps/unfixed-*.log); the byte-exact TXBYTES boundary (round 2); the TXBYTES counter's block and branch, both directions, and the cross-block exact fit (round 3); the TX_CAP branch scope both ways and a boundary matrix of 11 TX_CAP and 12 TXBYTES_CAP cases (round 3b); caps.rs 9/0 on 2b1a571",
                         "mutation controls: round 2, 16/16 killed (c635594); round 3, 55/55 (5965b8d); round 3b, 84 mutants, 83 killed, R7 argued equivalent (e985ce1, evidence/caps-r3b/)",
                         "Windows CI: tests/caps.rs passes (run 15 for round 3; runs 19 and 21 for round 3b)"],
               "evidence": ["evidence/caps/ (README.md, README-r2.md)", "evidence/caps-r3/", "evidence/caps-r3b/", "evidence/integration-r3/pocold-2b1a571.log"]},
  "relatedConformance": "caps-test build (validation.md:2065), a797c83: LOG_CAP = 2^24, simnet only, with tests/log_cap.rs. PARTIAL: HIST_TRIE_BUDGET = 64 MiB is not set, because pocold has no historical trie or K_eff",
  "partiallyResolved": ["The pool keeps transactions that can never be included after exhaustion, and a sender can squat the pool near the TXBYTES boundary for free (code inspection): the spec states no rule (network.md:149-153), so this is proposal N5"],
  "openRelated": ["No peer punishment for a violating template (consensus.md:101): pocold has no template peer scoring",
                  "eth_call/eth_estimateGas ignore the A03 halts (N7; code inspection only)",
                  "pocold's Hello lacks protocolVersion (network.md:7): a wire-format conformance gap outside the caps track",
                  "The base-fee effect of halted gas counted in gasUsed is from code inspection, not measured",
                  "No network-profile chain was run; network figures are model arithmetic",
                  "Commits 182026d, bdb14bb and ddf023a carry the cap tests with the builder unfixed, by design: tests/caps.rs fails at them (bisecting)"],
  "blocked": {"environment": [WIN_GAP], "decision": ["CAPS-SUSTAINABILITY N1-N8 (proposals/CAPS-SUSTAINABILITY.md): PROPOSED, not approved"]},
  "accepted": "NOT ACCEPTED (owner acceptance pending)",
  "independentReview": "three agent reviews (CV-1..CV-12, R2C-1..R2C-6, C3R-1..C3R-4); the round-3b changes are not reviewed yet",
  "disposition": "BLOCKED BY DECISION"},
]

DECISIONS = {
 "approvedByOwner": ["D-A13-BUDGET (option 1, local)",
                     "D-REORG-REDB (2026-10-23): a storage repair for R_keep adoption; D133 and validity unchanged; it does not cover the six consensus proposals",
                     "RpcGuard implementation in pocold (the 2026-10-23 instruction to complete the authorized works, RpcGuard and the Windows verification); a local service policy (D87)",
                     "engineering decisions taken under the standing authorization (REPAIR-PLAN.md)"],
 "proposedNotApproved": ["LOG-ACCT-01", "D-A03-DIRTY", "D-A03-CAPS", "D-BODY-MAX", "D-A03-SYSUSER", "LOG-CAP-AGGREGATE",
                         "CAPS-SUSTAINABILITY N1-N8 (proposals/CAPS-SUSTAINABILITY.md)",
                         "RPCGUARD P1-P4 (proposals/RPCGUARD-R2-PROPOSALS.md)",
                         "every journal mitigation (JOURNAL-GROWTH-AND-REPLAY.md §7)"],
 "note": "Passing tests approve nothing. The implementation uses the recommended alternatives of the six consensus proposals provisionally."}

INTEGRATION_R3 = {
 "note": "Round-3 suites on the integrated head, release, Linux. The full LP10 integration (extension, real Chrome, integrated Chrome), the witnesses and the RSS run were not re-run on the round-3 code; their last run is the 'integration' block (3c3792e). No full acceptance.",
 "runs": [
  {"commit": "2b1a571", "suite": "pocold", "result": "171 passed, 0 failed, 14 ignored", "log": "evidence/integration-r3/pocold-2b1a571.log"},
  {"commit": "2b1a571", "suite": "lp4-execution", "result": "168 passed, 0 failed", "log": "evidence/integration-r3/lp4-2b1a571.log"},
  {"commit": "2b1a571", "suite": "caps-test build (caps.rs 9, log_cap.rs 2)", "result": "11 passed, 0 failed", "log": "evidence/integration-r3/capstest-2b1a571.log"},
  {"commit": "1cc96c4", "suite": "abandoned_queued_bodies_are_capped_and_released, 8 runs", "result": "8/8 passed, busy in round 1", "log": "evidence/integration-r3/abandoned-queued-8x-linux.log"},
  {"commit": "c959379", "suite": "e2e_devnet.py (ad84942's test)", "result": "PASS 38/38", "log": "evidence/integration-r3/e2e-devnet-c959379.log"},
  {"commit": "c959379", "suite": "e2e_site.py", "result": "PASS 20/20", "log": "evidence/integration-r3/e2e-site-c959379.log"},
  {"commit": "c959379", "suite": "pocold tests/consensus.rs", "result": "43 passed, 0 failed, 2 ignored", "log": "evidence/integration-r3/consensus-c959379.log"},
  {"commit": "25bec25", "suite": "pocold; lp4-execution", "result": "164 passed, 0 failed, 12 ignored; 168 passed", "log": "evidence/integration-r3/pocold-25bec25.log, lp4-25bec25.log"},
  {"commit": "d40fd91", "suite": "e2e_devnet.py (f583c7f's test), 3 runs; e2e_site.py", "result": "3x PASS 27/27; PASS 20/20", "log": "evidence/e2e-integration/"}]}

WINDOWS = {
 "workflow": "audit-r2-windows-verify.yml, pull_request trigger limited to PRs into claude/affectionate-pascal-rjhh2k (a03e089)",
 "runs": [{"run": 9, "commit": "29fd44a", "result": "PASS on every check"},
          {"run": 16, "commit": "25bec25", "result": "PASS on every check (round-3 code: poll/WSAPoll in the SendPacer and the P2P reader)"},
          {"run": 19, "commit": "2b1a571", "result": "PASS (caps r3b, journal r3c, RpcGuard r3c)"},
          {"run": 20, "commit": "49ae9fa", "result": "FAIL, 1 timing test (abandoned_queued_bodies_are_capped_and_released); same code as run 19"},
          {"run": 21, "commit": "1cc96c4", "result": "PASS (the test's settings changed only)"}],
 "defectsFoundAndFixed": ["retention.rkeep: a directory fsync Windows refuses (run 1)", "a dropped P2P peer's reader not woken by shutdown (run 2)",
                          "a Nagle/delayed-ACK stall of about 200 ms on every RPC answer (run 2)",
                          "kernel absorption of unread RPC answers (runs 3, 5, 7): SO_SNDBUF 32 KiB plus a write cap of SEND_SOCK_BUF per call (29fd44a)"],
 "notCovered": WIN_GAP, "record": "WINDOWS-VERIFICATION.md"}

E2E_MUTATION = [
 {"suite": "e2e-devnet-mutation", "round": "stage A", "testCommit": "bfeb226", "codeTree": "a03e089 (= 3c3792e)", "cleanRuns": "3/3 PASS 25/25",
  "mutants": {"total": 10, "caughtIntended": 6, "caughtEarly": 3, "survived": 1, "incomplete": 0},
  "superseded": "its claim that a wrong choice fails every time is withdrawn: a height-first fork choice (Mheight) passed it 25/25 (e2e-mutation-r2/README.md §2)",
  "evidence": "evidence/e2e-mutation/README.md"},
 {"suite": "e2e-devnet-mutation-r2", "round": "2", "testCommit": "f583c7f", "codeTree": "c195a89 (= a03e089)", "cleanRuns": "3/3 PASS 27/27",
  "mutants": {"total": 13, "caughtIntended": 8, "caughtEarly": 4, "survived": 1, "incomplete": 0},
  "survivorCoveredBy": "tests/consensus.rs fork_choice_heavier_tied_signed_first_and_rollback",
  "newMutantsVsStageATest": {"Msess": "PASS 25/25 (survived)", "Mreplay": "PASS 25/25 (survived)", "Mheight": "PASS 25/25 (review run)"},
  "cargoSuite": "pocold release 123 passed", "evidence": "evidence/e2e-mutation-r2/README.md"},
 {"suite": "e2e-devnet-mutation-r3", "round": "3", "testCommit": "1512409", "script": "sha256 6667e11a…a621bf, 35 checks", "cleanRuns": "3/3 PASS 35/35 on base-r3c, plus one control run on base-r3d",
  "mutants": {"total": 21, "caughtIntended": 16, "caughtEarly": 4, "survived": 1, "incomplete": 0},
  "note": "not merged on its own; its test changes (and 16bebfd, E2E-R3-7) reach the branch with round 4 at c959379",
  "evidence": "evidence/e2e-mutation-r3/README.md"},
 {"suite": "e2e-devnet-mutation-r4", "round": "4 (current)", "testCommit": "ad84942", "script": "sha256 9c6d7c0d…98814c, 38 checks", "cleanRuns": "3/3 PASS 38/38 on base-r4",
  "mutants": {"total": 23, "caught": 22, "caughtIntended": 16, "caughtEarly": 6, "survived": 1, "incomplete": 0,
              "notCounted": "M4's first run failed the step-3 precondition before anything M4 changes is checked; its second run was caught at the intended check"},
  "survivorCoveredBy": "tests/consensus.rs fork_choice_heavier_tied_signed_first_and_rollback (Mtie: the D133 tie-break is local)",
  "cargoSuite": "pocold release at ad84942: exit 0", "integrated": "c959379: PASS 38/38 (evidence/integration-r3/e2e-devnet-c959379.log)",
  "openItems": ["the step-3 climb bound (240 s) can give a false FAIL when a/b's lead overshoots (M4.log); a bound growing with the late blocks needed is proposed, not made",
                "one run per mutant; Mtie's single pass does not show it always passes",
                "Msilent and Mtransient are now caught first by the new 4b Status check, so step 5 is not shown to catch them on its own in this round",
                "step 8b (heal while mining) catches no mutant by itself; the step-9 race allowance and the torn-tail allowance were never exercised",
                "the full LP10 integration and the Windows workflow were not re-run on this test"],
  "evidence": "evidence/e2e-mutation-r4/README.md"}]

def main():
    commits = json.load(open(sys.argv[1]))
    integ = json.load(open(os.path.join(ROOT, "evidence", "integration.json")))["runs"][-1]
    out = {"auditedSnapshot": "090eae4", "repairBranch": "claude/audit-repair-a01-a14", "pr": "https://github.com/raedrasheed/Proof-of-Collaboration/pull/27",
           "commits": commits,
           "axes": {"implemented": "YES | PARTIAL", "verified": "VERIFIED (Linux and Windows CI) | PARTIAL: regression tests, mutation controls and the reconstructed witness with its positive control on the recorded commits; the Windows CI record is verified.windows",
                    "partiallyResolved": "requirements still technically incomplete", "blocked": "environment: only another platform can clear it; decision: only an owner decision can clear it",
                    "accepted": "owner acceptance; none is recorded"},
           "witnessProvenance": WIT, "integration": {"commit": integ["commit"], "result": integ["result"], "failuresOnlyExpectedPlatformLimitation": integ.get("failuresOnlyExpectedPlatformLimitation"),
               "suites": [{k: s.get(k) for k in ("suite", "exit", "pass", "executedAndFailed", "onlyExpectedPlatformLimitation", "platformNote") if k in s} for s in integ["suites"]]},
           "integrationR3": INTEGRATION_R3, "windowsVerification": WINDOWS, "e2eDevnetMutation": E2E_MUTATION, "decisions": DECISIONS,
           "findings": []}
    for fid, sev, title, rc, tests, partial, env, dec in F:
        f = {"id": fid, "severity": sev, "title": title,
             "reproduction": {"onAuditedSnapshot": "REPRODUCED" if fid != "A13" else "source review (no runtime witness)", "by": WIT if fid != "A13" else "source review", "evidence": "evidence/baseline-090eae4.log"},
             "repairCommits": rc, "implemented": "PARTIAL" if fid in ("A05",) else "YES",
             "verified": {"status": "PARTIAL" if fid == "A13" else "VERIFIED (Linux and Windows CI)", "tests": tests,
                          "witnessOnRepair": "n/a (source review)" if fid == "A13" else "NOT_REPRODUCED with its positive control (evidence/WITNESS-CONTROLS.md)",
                          "commits": {"integration": commits["integrationTested"], "witness": commits["witnessTested"], "realChrome": commits["chromeTested"], "windows": commits.get("windowsTested")},
                          "windows": WIN},
             "partiallyResolved": partial, "blocked": {"environment": [WIN_GAP] + env, "decision": dec}, "accepted": "NOT ACCEPTED (owner acceptance pending)",
             "independentReview": NOREV}
        f["disposition"] = ("PARTIALLY RESOLVED" if partial and fid in ("A02", "A05", "A13") else
                            "BLOCKED BY DECISION" if any(d for d in dec if "engineering" not in d and "standing" not in d and "changed expectation" not in d and "owner approved" not in d) else
                            DV)
        out["findings"].append(f)
    out["additionalFindings"] = EXTRA
    out["summary"] = {d: [f["id"] for f in out["findings"] if f["disposition"] == d] for d in ("PARTIALLY RESOLVED", "BLOCKED BY DECISION", DV)}
    out["summary"]["ACCEPTED"] = []
    out["summary"]["additionalFindings"] = {f["id"]: f["disposition"] for f in EXTRA}
    json.dump(out, open(os.path.join(ROOT, "FINDING-STATUS.json"), "w"), indent=1, ensure_ascii=False); open(os.path.join(ROOT, "FINDING-STATUS.json"), "a").write("\n")
    print(json.dumps(out["summary"]))

if __name__ == "__main__":
    main()
