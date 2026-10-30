# RpcGuard fix round: mutation controls (round 2)

| item | value |
|---|---|
| tree under test | `dc4e3d1c6668381b47cf21f7ac0142a149d4c734` (wf/rpcguard-r2; its code is 6a561b6 plus one test) |
| tree export | `git archive` of `prototypes/{pocold,lp4-execution,lp1-node}` and `development/audit-2026-10-22/proposals/vectors` into a scratch directory; the worktree is never mutated |
| runner | scratch `run_mutations_rpcguard_r2.py`, modeled on `run_mutations_rpcguard.py` |
| toolchain | Rust 1.99.1, `cargo test --offline --release --locked … -- --exact --test-threads=1 <tests>`, every run under the shared CPU lock, loopback 127.0.0.1 only |
| machine-readable results | `mutations-rpcguard-r2.json` |

## Method

For each mutant:

1. **Positive control.** The named tests pass on the clean tree.
2. **Mutation.** One mechanism is disabled or weakened. Each anchor must occur exactly once, or the
   result is APPLY-FAIL.
3. **Verdict.** The named tests run again:
   - **CAUGHT** if a named test FAILED having run;
   - **SURVIVED** if they all pass;
   - **INCOMPLETE** for a timeout, never caught.
4. **Restore.** The file is restored from its pristine text.

Every mutant below failed on the assertion written for its mechanism; the scratch logs show each
failing assertion. Each edit only removes or weakens a fix. None is an exploit.

## Results

21 mutants: 20 CAUGHT, 1 SURVIVED. The survivor, V8, is an equivalent mutant (explained below).

| id | what it disables | tests | control | mutant | result |
|---|---|---|---|---|---|
| RGM1-PACER | the SendPacer close rule | send_pacer_closes_a_slow_reader_after_grace | OK | FAILED ("the pacer closed the non-reading client") | CAUGHT |
| RGM2-RATE | the per-connection bucket never refuses | the_per_connection_bucket_charges_k_tokens_per_request | OK | FAILED (6 members ran) | CAUGHT |
| RGM3-HEAVYQUEUE | the HEAVY_QUEUE place cap | abandoned_queued_bodies_are_capped_and_released | OK | FAILED (no busy, peak 16 > 4) | CAUGHT |
| RGM4-ENTRY | the COMPUTE_DEADLINE check at handler entry | light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | OK | FAILED (a late transaction was pooled and answered) | CAUGHT |
| RGM5-STOPHOOK | the eth_call stop hook | heavy_compute_deadline_cancels_eth_call | OK | FAILED (ran to OutOfGas) | CAUGHT |
| RGM6-SCRATCH | the getLogs SCRATCH charge | get_logs_scratch_limit_is_32020_scratch_with_resume_data | OK | FAILED (546 logs returned) | CAUGHT (pass 2, see note) |
| RGM7-PARAMS | rpcLimits.connMax wrong | get_params_and_rpc_stats_report_the_limits | OK | FAILED | CAUGHT |
| RGM8-POSTCHECK | the deadline check after encoding (review B3) | a_precompile_call_past_its_deadline_is_32020_not_a_late_success | OK | FAILED (late BLAKE2F success returned) | CAUGHT |
| RGM9-FROM-ARRIVAL | COMPUTE_DEADLINE from arrival (stage A's B1 regression) | compute_deadline_runs_from_admission_and_queue_wait_bounds_admission, light_requests_behind_long_heavy_work_are_answered_never_deadline | OK | FAILED (−32020 for a request that waited) | CAUGHT |
| RGM10-STOPGAS | the per-10^6-gas poll (only every 64th JUMP) | lp4 the_stop_hook_is_polled_at_least_every_stop_gas | OK | FAILED (9 polls for 30M gas) | CAUGHT |
| RGM11-PARSE-TREE | busy/rate answers parse the request into a tree (A13) | rpc::tests::a_busy_answer_allocates_no_parse_tree | OK | FAILED (842,147 B requested) | CAUGHT |
| V1a-NO-QUEUE-WAIT-CONN | the connection waits past QUEUE_WAIT (review V1) | queued_requests_are_busy_at_queue_wait | OK | FAILED (answered at 543 ms, after the holder) | CAUGHT |
| V1b-NO-QUEUE-WAIT-NODE | the node admits past QUEUE_WAIT (review V1) | compute_deadline_runs_from_admission_and_queue_wait_bounds_admission | OK | FAILED (the late transaction ran) | CAUGHT |
| V2-PACER-LATE | the pacer re-checks only every 1.5 s (review V2) | send_pacer_closes_a_slow_reader_after_grace | OK | FAILED (lateness 1001 ms > ENFORCE_SLACK) | CAUGHT |
| V3-LIGHT-FULL-503 | a full light lane answers 503 (review V3) | a_full_light_lane_answers_busy | OK | FAILED (four 503s) | CAUGHT |
| V4a-NO-BATCH-WALL | BATCH_WALL between members (review V4) | batch_wall_closes_the_batch | OK | FAILED (answered 200) | CAUGHT |
| V4b-NO-SEND-WALL | BATCH_WALL during the send | batch_wall_closes_the_batch | OK | FAILED (never closed) | CAUGHT |
| V5-NO-LOGS-DEADLINE | the getLogs deadline check in the scan (review V5) | get_logs_scratch_limit_is_32020_scratch_with_resume_data | OK | FAILED (the scan materialized 179,362 B) | CAUGHT |
| V6-BATCH-ONE-TOKEN | a k-member batch costs one token (review V6) | the_per_connection_bucket_charges_k_tokens_per_request | OK | FAILED | CAUGHT |
| V7-ESTIMATE-SWALLOWS-DEADLINE | eth_estimateGas continues after −32020 (review V7) | estimate_gas_stops_its_search_at_the_deadline | OK | FAILED (10 deadline hits, not 1) | CAUGHT |
| V8-NO-ENTRY-CHECK-HEAVY | the entry check only for light methods (review V8) | light_compute_deadline_fires, a_request_reaching_its_handler_past_the_deadline_never_runs | OK | OK | SURVIVED: equivalent mutant |

**Note on RGM6.** Pass 1 was INVALID: control and mutant were both COMPILE-FAIL, because the export
lacked `development/audit-2026-10-22/proposals/vectors`, which `tests/consensus.rs` includes. After
exporting it, RGM6 alone was re-run (pass 2): CAUGHT. V5, which uses the same test binary, ran after
the export in pass 1.

**V8 is equivalent.** It does not change any answer:

- every heavy handler that does work checks the deadline before doing it: eth_call and
  eth_estimateGas before each simulation, the log handlers at their first block;
- every other heavy handler is bounded and read-only, and `member` replaces its late result with
  −32020;
- pocold has no effectful heavy method.

The mutant only lets such a bounded handler run once past its deadline before its answer is replaced.

Stage A's mutants RGM1–RGM7 (evidence/MUTATIONS-RPCGUARD.md, at ad71484) are re-expressed here
against the round-2 code. RGM3 now uses the queued-bodies test, because queued requests are now also
busy at QUEUE_WAIT. RGM4 adds the effectful entry test, because the post-encoding check alone
already answers a late read-only result −32020.
