# RpcGuard round 3c: mutation controls on dd7d1ec

Runner: `mut_r3c.py` (scratch, sha256 `17e0dc125d67cd16…`), which imports the round-3 runner `run_mutations_rpcguard_r3_final.py` (its machinery and verdict rules unchanged) and replaces two things: the mutant set, and the status parse, which now reads a test's status from the next line when the test prints a measurement line on its own test line (the third review's runner marked RV3-SV-GRACE-ONLY and RV3-NO-BACKSTOP INVALID for that reason).
Tree: an exported copy (archive of prototypes/) of `dd7d1ec dd7d1ec086b29d1edea40b003a5c6e1e759cf322 (git archive of prototypes/, round 3c)`, checked against a fresh export after the run: no difference.
Mutant set: the round-3 mutants whose code or tests round 3c touched (14), the third review's survivors and probes against the round-3c tests (7, with I3b), the two round-3 mutants whose anchors moved or whose expectation changed (2), and one or more mutants per round-3c change (12).
Each mutant edits one or two anchors (each occurs exactly once) and runs the named committed tests (release, `--locked`, `--offline`, `--exact`, one thread, under the shared CPU lock). Control: every named test passes on the clean tree first. CAUGHT: a named test FAILED having run. SURVIVED: all ran and passed. A timeout is INCOMPLETE, never caught; a compile failure or a missing test INVALID.
`expect` is what this round expected before the run; the verdict is the run's.

Totals: CAUGHT 30, SURVIVED 5 (of 35). Every control passed; no timeout; not as expected: none.

| mutant | verdict | as expected | tests (target) | expectation |
|---|---|---|---|---|
| RGM1-PACER | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| RGM4-ENTRY | CAUGHT | yes | --test net: light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | caught |
| RGM8-POSTCHECK | CAUGHT | yes | --test net: a_precompile_call_past_its_deadline_is_32020_not_a_late_success, a_late_revert_or_halt_past_its_deadline_is_32020, a_late_get_logs_answer_carries_resume_data | caught |
| V2-PACER-LATE | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| V8-NO-ENTRY-CHECK-HEAVY | SURVIVED | yes | --test net: light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | survivor: equivalent (RPCGUARD.md §9 V8): every heavy handler that works re-checks the deadline, the others are bounded and their late answer is replaced |
| N3-PARTIAL-WRITE-AS-WHOLE | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace, an_answer_is_reserved_until_its_connection_has_written_it | caught |
| N4-HEADER-BODY-BOUNDARY | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| N9-ONE-DEADLINE-PER-BATCH | CAUGHT | yes | --test net: batch_members_each_get_their_own_compute_deadline | caught |
| N11-LIGHT-USES-HEAVY-DEADLINE | CAUGHT | yes | --test net: light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | caught |
| R3-M1-RESULTS-ONLY | CAUGHT | yes | --test net: a_late_revert_or_halt_past_its_deadline_is_32020 | caught |
| R3-M2-SLEEP-POLL-1MS | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| R3-M2-NO-WALL-IN-WAIT | SURVIVED | yes | --test net: batch_wall_closes_the_batch | survivor: bounded by the backstop: BATCH_WALL is then seen at most PACER_BACKSTOP (50 ms) late, within its 100 ms release (network.md:282) |
| R3-M4-32008 | CAUGHT | yes | --lib: rpc::tests::responses_above_resp_max_become_an_error; --test net: an_answer_over_its_limit_is_32020_result_bytes_with_a_clean_release | caught |
| R3-M2-BACKSTOP-ONLY | SURVIVED | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | survivor: the close is then up to PACER_BACKSTOP (50 ms) after s_v, within ENFORCE_SLACK (200 ms), the bound network.md:247 binds and the test asserts |
| RV3-SV-GRACE-ONLY | CAUGHT | yes | --test net: send_pacer_timer_follows_a_over_r_past_grace, send_pacer_closes_a_slow_reader_after_grace | caught |
| I3b-NO-WRITE-CAP | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace, send_pacer_timer_follows_a_over_r_past_grace, an_answer_is_reserved_until_its_connection_has_written_it | caught |
| RV3-NO-BACKSTOP | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace, send_pacer_timer_follows_a_over_r_past_grace | caught |
| R3C-WAIT-OUT-BACKSTOP | CAUGHT | yes | --test net: send_pacer_closes_a_slow_reader_after_grace | caught |
| RV3-LATE-CHECK-BEFORE-ENCODING | CAUGHT | yes | --test net: the_compute_deadline_includes_encoding | caught |
| RV3-LATE-HEAVY-ONLY | CAUGHT | yes | --test net: the_compute_deadline_includes_encoding | caught |
| RV3-M5-PAD-ONLY | CAUGHT | yes | --test net: the_conditional_light_entries_have_the_light_deadline, a_light_method_over_light_body_max_has_the_light_deadline | caught |
| RV3-CAS-CHECK-THEN-SET | SURVIVED | yes | --test net: the_node_never_runs_a_request_its_connection_gave_up | survivor: a test limit, not a defect (RPCGUARD.md §2): the race window of a load-then-store is nanoseconds and no hook can sit inside one compare_exchange; atomicity is by construction |
| R3-m5-DEADLINE-BY-LANE | CAUGHT | yes | --test net: a_light_method_over_light_body_max_has_the_light_deadline, the_conditional_light_entries_have_the_light_deadline | caught |
| R3-M4-NO-CLASS-LIMIT | SURVIVED | yes | --test net: an_answer_over_its_limit_is_32020_result_bytes_with_a_clean_release, a_light_method_answer_over_light_resp_max_is_32020_on_the_heavy_lane | survivor: equivalent (RPCGUARD.md §9): member() applies both class limits on every path and batch() bounds a batch below BATCH_BYTES_HARD, so answer()'s check over the reservation is defence in depth |
| R3C-CLASS-LIMIT-LANE-ONLY | CAUGHT | yes | --test net: a_light_method_answer_over_light_resp_max_is_32020_on_the_heavy_lane | caught |
| R3C-CLASS-LIMIT-LIGHT-LANE-REMOVED | CAUGHT | yes | --test net: an_answer_over_its_limit_is_32020_result_bytes_with_a_clean_release | caught |
| R3C-COND-NOT-LIGHT | CAUGHT | yes | --test net: the_conditional_light_entries_have_the_light_deadline; --lib: rpc::tests::the_class_probe_agrees_with_the_parsed_class | caught |
| R3C-COND-FULL-FLAG-IGNORED | CAUGHT | yes | --test net: the_conditional_light_entries_have_the_light_deadline; --lib: rpc::tests::the_class_probe_agrees_with_the_parsed_class | caught |
| R3C-COND-COUNT-65 | CAUGHT | yes | --test net: the_conditional_light_entries_have_the_light_deadline; --lib: rpc::tests::the_class_probe_agrees_with_the_parsed_class | caught |
| R3C-COND-HEX-COUNT-IGNORED | CAUGHT | yes | --test net: the_conditional_light_entries_have_the_light_deadline; --lib: rpc::tests::the_class_probe_agrees_with_the_parsed_class | caught |
| R3C-PROBE-PARAMS-IGNORED | CAUGHT | yes | --test net: the_conditional_light_entries_have_the_light_deadline; --lib: rpc::tests::the_class_probe_agrees_with_the_parsed_class | caught |
| R3C-PROBE-FIRST-PARAM | CAUGHT | yes | --lib: rpc::tests::the_class_probe_agrees_with_the_parsed_class | caught |
| R3C-VALUE-NONARRAY-LIGHT | CAUGHT | yes | --lib: rpc::tests::the_class_probe_agrees_with_the_parsed_class | caught |
| R3C-BATCH-MEMBER-LIST-ONLY | CAUGHT | yes | --test net: the_conditional_light_entries_have_the_light_deadline, batch_members_each_get_their_own_compute_deadline | caught |
| R3C-ENCODE-CHECK-SKIPS-HOOK | CAUGHT | yes | --test net: the_compute_deadline_includes_encoding | caught |

Failing assertions of the third review's mutants, from the per-mutant logs (scratch `r3c/mut/mut-logs/`):

- RV3-SV-GRACE-ONLY: `send_pacer_timer_follows_a_over_r_past_grace`: "past GRACE with A/R ahead of sendTime the pacer waited 2170384 times in 1.50 s (bound 1094)" (`send_pacer_closes_a_slow_reader_after_grace` still passes: its non-reader's A/R is below GRACE).
- I3b-NO-WRITE-CAP: `send_pacer_closes_a_slow_reader_after_grace`: "the honest reader's ~14 MiB answer took 8198 ms to send".
- R3C-WAIT-OUT-BACKSTOP: the same assertion, 10949 ms.
- RV3-NO-BACKSTOP: `send_pacer_closes_a_slow_reader_after_grace`: "the pacer waited 2 times before GRACE: the backstop check is missing (floor 5)".
- RV3-LATE-CHECK-BEFORE-ENCODING, RV3-LATE-HEAVY-ONLY: `the_compute_deadline_includes_encoding` (a slow encoding past the deadline answered with its result).
- RV3-M5-PAD-ONLY: `the_conditional_light_entries_have_the_light_deadline` (the 4 KiB-id eth_blockNumber answered with a result).
