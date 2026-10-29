# RpcGuard round 3: mutation controls on the final tree

Runner: `run_mutations_rpcguard_r3_final.py` (scratch, sha256 `f9c6822cb2755287…`; the round-3 runner's 53 mutants unchanged, plus 6 added in the continuation), on an exported copy of the committed tree `d0e87f93a417ec6888fc9bd0d433136ab238d741` (prototypes/pocold, lp4-execution, lp1-node and the proposal vectors), run in 4 consecutive parts of the mutant list (each under the background-run limit; every part ran to its end, and the tree was checked against its export afterwards: no content difference), merged here in the runner's order.
Each mutant edits one or two anchors (each occurs exactly once in the tree) and runs the named committed tests (release, `--locked`, `--exact`, one thread, under the shared CPU lock).
Control: every named test passes on the clean tree first (once per part). CAUGHT: a named test FAILED having run. SURVIVED: all ran and passed. A timeout is INCOMPLETE, never caught; a compile failure or a missing test INVALID.
`expect` is what the round expected before the run (caught, or a survivor with its reason); the verdict is the run's.

Totals: CAUGHT 53, SURVIVED 6 (of 59). Not as expected: none.

| mutant | verdict | as expected | tests (target) | expectation |
|---|---|---|---|---|
| RGM1-PACER | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| RGM2-RATE | CAUGHT | yes | --test net: the_per_connection_bucket_charges_k_tokens_per_request | caught |
| RGM3-HEAVYQUEUE | CAUGHT | yes | --test net: abandoned_queued_bodies_are_capped_and_released | caught |
| RGM4-ENTRY | CAUGHT | yes | --test net: light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | caught |
| RGM5-STOPHOOK | CAUGHT | yes | --test net: heavy_compute_deadline_cancels_eth_call | caught |
| RGM6-SCRATCH | CAUGHT | yes | --test consensus: get_logs_scratch_limit_is_32020_scratch_with_resume_data | caught |
| RGM7-PARAMS | CAUGHT | yes | --test net: get_params_and_rpc_stats_report_the_limits | caught |
| RGM8-POSTCHECK | CAUGHT | yes | --test net: a_precompile_call_past_its_deadline_is_32020_not_a_late_success, a_late_revert_or_halt_past_its_deadline_is_32020, a_late_get_logs_answer_carries_resume_data | caught |
| RGM9-FROM-ARRIVAL | CAUGHT | yes | --test net: compute_deadline_runs_from_admission_and_queue_wait_bounds_admission, light_requests_behind_long_heavy_work_are_answered_never_deadline | caught |
| RGM10-STOPGAS | CAUGHT | yes | --test simulate_stop: the_stop_hook_is_polled_at_least_every_stop_gas | caught |
| RGM11-PARSE-TREE | CAUGHT | yes | --lib: rpc::tests::a_busy_answer_allocates_no_parse_tree | caught |
| V1a-NO-QUEUE-WAIT-CONN | CAUGHT | yes | --test net: queued_requests_are_busy_at_queue_wait | caught |
| V1b-NO-QUEUE-WAIT-NODE | CAUGHT | yes | --test net: compute_deadline_runs_from_admission_and_queue_wait_bounds_admission | caught |
| V2-PACER-LATE | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| V3-LIGHT-FULL-503 | CAUGHT | yes | --test net: a_full_light_lane_answers_busy | caught |
| V4a-NO-BATCH-WALL | CAUGHT | yes | --test net: batch_wall_closes_the_batch | caught |
| V4b-NO-SEND-WALL | CAUGHT | yes | --test net: batch_wall_closes_the_batch | caught |
| V5-NO-LOGS-DEADLINE | CAUGHT | yes | --test consensus: get_logs_scratch_limit_is_32020_scratch_with_resume_data | caught |
| V6-BATCH-ONE-TOKEN | CAUGHT | yes | --test net: the_per_connection_bucket_charges_k_tokens_per_request | caught |
| V7-ESTIMATE-SWALLOWS-DEADLINE | CAUGHT | yes | --test net: estimate_gas_stops_its_search_at_the_deadline | caught |
| V8-NO-ENTRY-CHECK-HEAVY | SURVIVED | yes | --test net: light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | survivor: equivalent (RPCGUARD.md §9 V8): every heavy handler that works re-checks the deadline, the others are bounded and their late answer is replaced |
| I1-NO-SET-SOCKBUF | CAUGHT | yes | --test net: get_params_and_rpc_stats_report_the_limits | caught |
| I2-SOCKBUF-ERROR-SERVED | CAUGHT | yes | --test net: a_connection_whose_send_buffer_cannot_be_set_is_refused | caught |
| I3b-NO-WRITE-CAP | SURVIVED | yes | --test net: send_pacer_closes_a_slow_reader_after_grace, batch_wall_closes_the_batch, an_answer_is_reserved_until_its_connection_has_written_it | survivor on Linux: a non-blocking send takes at most the free (locked) send buffer, so A is unchanged; the cap bounds Windows, which takes a whole send() (runs 3, 5, 7), and on Linux it keeps throughput (no delayed-ACK stall) |
| I4-READ-BY-NO-CLOSED | CAUGHT | yes | --lib: net::tests::a_reader_sees_its_closed_flag_without_a_shutdown | caught |
| N1-PLACE-WAIT-TO-WORK-DEADLINE | CAUGHT | yes | --test net: queue_wait_bounds_the_wait_for_a_heavy_queue_place | caught |
| N2-MAINQ-WAIT-TO-WORK-DEADLINE | CAUGHT | yes | --test net: queue_wait_bounds_the_wait_on_a_full_main_queue | caught |
| N3-PARTIAL-WRITE-AS-WHOLE | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace, an_answer_is_reserved_until_its_connection_has_written_it | caught |
| N4-HEADER-BODY-BOUNDARY | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| N5-ADMITTED-ANSWERED-BUSY | CAUGHT | yes | --test net: queued_requests_are_busy_at_queue_wait | caught |
| N6-NODE-ADMIT-WITHOUT-CAS | CAUGHT | yes | --test net: the_node_never_runs_a_request_its_connection_gave_up | caught |
| N7-BATCH-WALL-FROM-ADMISSION | CAUGHT | yes | --test net: batch_wall_counts_the_time_queued | caught |
| N8-LATE-LOGS-WITHOUT-RESUME-DATA | CAUGHT | yes | --test net: a_late_get_logs_answer_carries_resume_data | caught |
| N9-ONE-DEADLINE-PER-BATCH | CAUGHT | yes | --test net: batch_members_each_get_their_own_compute_deadline | caught |
| N10-RATE-OVER-BATCH-MAX | CAUGHT | yes | --test net: the_per_connection_bucket_charges_k_tokens_per_request | caught |
| N11-LIGHT-USES-HEAVY-DEADLINE | CAUGHT | yes | --test net: light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | caught |
| NF1-NOMINAL-COMPUTE-HEAVY-FACT | CAUGHT | yes | --test admission: pocold_admission_is_an_experimental_devnet_with_absent_rpc_mechanisms | caught |
| NF2-NOMINAL-CONN-MAX-FACT | CAUGHT | yes | --test admission: pocold_admission_is_an_experimental_devnet_with_absent_rpc_mechanisms | caught |
| NF3-NOMINAL-LIGHT-RESP-MEM-FACT | CAUGHT | yes | --test admission: pocold_admission_is_an_experimental_devnet_with_absent_rpc_mechanisms | caught |
| R3-M1-RESULTS-ONLY | CAUGHT | yes | --test net: a_late_revert_or_halt_past_its_deadline_is_32020 | caught |
| R3-M1-OWN-DEADLINE-REPLACED | CAUGHT | yes | --test net: estimate_gas_stops_its_search_at_the_deadline, a_late_revert_or_halt_past_its_deadline_is_32020; --test consensus: get_logs_scratch_limit_is_32020_scratch_with_resume_data | caught |
| R3-M2-SLEEP-POLL-1MS | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| R3-M2-NO-WALL-IN-WAIT | SURVIVED | yes | --test net: batch_wall_closes_the_batch | survivor: bounded by the backstop: BATCH_WALL is then seen at most PACER_BACKSTOP (50 ms) late, within its 100 ms release (network.md:282) |
| R3-m4-READ-POLL-TIMEOUT-FATAL | CAUGHT | yes | --test net: idle_peers_without_hello_are_bounded_and_expire, a_peer_that_stops_reading_is_dropped_and_consensus_keeps_serving, inbound_floods_cannot_take_the_slots_for_configured_peers | caught |
| R3-m4-NO-WAIT-BEFORE-READ | CAUGHT | yes | --lib: net::tests::a_reader_sees_its_closed_flag_without_a_shutdown | caught |
| R3-M4-32008 | CAUGHT | yes | --lib: rpc::tests::responses_above_resp_max_become_an_error; --test net: an_answer_over_its_limit_is_32020_result_bytes_with_a_clean_release | caught |
| R3-M4-NO-CLASS-LIMIT | CAUGHT | yes | --test net: an_answer_over_its_limit_is_32020_result_bytes_with_a_clean_release | caught |
| R3-m5-DEADLINE-BY-LANE | CAUGHT | yes | --test net: a_light_method_over_light_body_max_has_the_light_deadline | caught |
| R3-m6-READBACK-UNCHECKED | CAUGHT | yes | --test net: a_connection_whose_send_buffer_cannot_be_set_is_refused | caught |
| R3-m6-SHARED-COUNTER | CAUGHT | yes | --test net: a_connection_whose_send_buffer_cannot_be_set_is_refused | caught |
| R3-m1-RATE-NOTE | CAUGHT | yes | --test admission: pocold_admission_is_an_experimental_devnet_with_absent_rpc_mechanisms | caught |
| R3-m2-SMALL-TERM | CAUGHT | yes | --test consensus: a13_bound_breakdown | caught |
| R3-m2-NO-PARSE-FILTER | SURVIVED | yes | --lib: rpc::tests::a_busy_answer_allocates_no_parse_tree, rpc::tests::busy_answers_scan_the_ids_a_parse_would_echo | survivor: the measured peaks stay below SMALL_REPLY_PEAK without the filter (worst parse transient about 85 KB); the filter is what bounds ID_PARSE_PEAK by derivation |
| R3-SOCKWAIT-ROUND-DOWN | CAUGHT | yes | --lib: rpc::tests::sockwait_reports_readiness_within_its_timeout | caught |
| R3-SOCKWAIT-EVENTS-SWAPPED | CAUGHT | yes | --lib: rpc::tests::sockwait_reports_readiness_within_its_timeout | caught |
| R3-m4-WAIT-WITHOUT-READ-POLL | CAUGHT | yes | --lib: net::tests::a_reader_sees_its_closed_flag_without_a_shutdown | caught |
| R3-M2-BACKSTOP-ONLY | SURVIVED | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | survivor: the close is then up to PACER_BACKSTOP (50 ms) after s_v, within ENFORCE_SLACK (200 ms), the bound network.md:247 binds and the test asserts |
| R3-m6-NO-UPPER-BOUND | CAUGHT | yes | --test net: a_connection_whose_send_buffer_cannot_be_set_is_refused | caught |
| R3-P3-NO-EFFECTFUL-EXEMPTION | SURVIVED | yes | --test net: a_request_reaching_its_handler_past_the_deadline_never_runs, compute_deadline_runs_from_admission_and_queue_wait_bounds_admission | survivor: no committed test pins P3 option A (a late effectful answer keeps its result): it is the running reading pending a decision (proposals P3), deliberately not pinned |

The JSON beside this file (`mutations-r3-final-d0e87f9.json`) has every control and mutant run with its verdict and duration. Per-mutant logs were kept in scratch (`r3-mut-final/mut-logs/`). The earlier, incomplete runs are described in `MUTATIONS-R3-EARLIER-RUNS.md`.
