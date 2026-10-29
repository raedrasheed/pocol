# RpcGuard mutation controls

Each mutant disables **one** RpcGuard mechanism (a fix-removal; no exploit payloads) and the named test
must then FAIL, having actually run. CAUGHT = the positive control passed on clean code (the test reaches
the mechanism) and the mutant made the test FAIL. A control or mutant that times out is recorded as
incomplete, never as a pass or a catch — none did here.

- Commit under test: `ad714843c58dbee7c24d9eff80ceac8c5b955cfd` (branch `wf/rpcguard`).
- Runner (scratch only, not committed): `scratchpad/run_mutations_rpcguard.py`, in an isolated detached
  worktree at that commit. Every cargo invocation ran under the shared CPU lock (`flock cpu.lock`), release,
  `--offline`, `--test-threads=1`.
- Machine-readable results: `mutations-rpcguard.json` (this directory).
- Toolchain: Rust 1.99.1 (rust-toolchain.toml). Linux; the tests bind only 127.0.0.1 on ephemeral ports.

| mutant | mechanism disabled | test | control | mutant | result |
|---|---|---|---|---|---|
| RGM1-PACER | SendPacer close rule (A < R·sendTime after GRACE) | `send_pacer_closes_a_slow_reader_after_grace` | OK (1.8 s) | FAILED (20.1 s) | **CAUGHT** |
| RGM2-RATE | per-source token bucket (charges 0 tokens) | `rate_bucket_limits_a_source_then_refills` | OK (17.2 s) | FAILED (15.4 s) | **CAUGHT** |
| RGM3-HEAVYQUEUE | HEAVY_QUEUE cap (no busy on overflow) | `heavy_queue_overflow_answers_busy_with_retry_after` | OK (16.5 s) | FAILED (24.6 s) | **CAUGHT** |
| RGM4-LIGHTDL | COMPUTE_DEADLINE handler-entry check (light) | `light_compute_deadline_fires` | OK (26.8 s) | FAILED (15.5 s) | **CAUGHT** |
| RGM5-HEAVYDL | eth_call simulation stop hook (heavy COMPUTE_DEADLINE/cancellation) | `heavy_compute_deadline_cancels_eth_call` | OK (16.4 s) | FAILED (16.5 s) | **CAUGHT** |
| RGM6-SCRATCH | eth_getLogs SCRATCH budget (no −32020 scratch) | `get_logs_scratch_limit_is_32020_scratch_with_resume_data` | OK (16.5 s) | FAILED (16.7 s) | **CAUGHT** |
| RGM7-PARAMS | pocol_getParams.rpcLimits running value (connMax zeroed) | `get_params_and_rpc_stats_report_the_limits` | OK (16.0 s) | FAILED (16.1 s) | **CAUGHT** |

Caught 7 of 7.

Observed mutant failures (from the recorded panics):
- RGM1: the non-reading client was never closed; `send_pacer_…` asserted `pacer_closed` increased and failed.
- RGM2: with 0 tokens charged, all 25 of a burst were served (`ok 25, rate 0`); the rate assertion failed.
- RGM3: the extra heavy calls ran to `halt OutOfGas` (no busy); the busy assertion failed.
- RGM4: the light request returned a result (`0x0`) instead of −32020 deadline.
- RGM5: eth_call ran to `halt OutOfGas` instead of −32020 deadline.
- RGM6: eth_getLogs returned a result array instead of −32020 scratch.
- RGM7: pocol_getParams reported connMax 0, so the connMax==32 assertion failed.

Mechanisms whose status is EQUIVALENT or NOT ENFORCED (RPCGUARD.md) have no fix to remove, so they carry no
mutant: the SO_SNDBUF option (D93: enters no criterion), the full PacerTrace ring buffer, batch streaming,
the exact TIMER_EPS tie-break, and the HEAVY_ACTIVE/LIGHT_ACTIVE/IDLE equivalences (the single consensus
thread and connection-per-request give them structurally). The pre-existing round-2 RPC mutants
(M3-QUEUECAP, M4-BUSY, M5-DEADLINE, M13-RESV, M14-BUSYPARSE, M1b-WAKE, M7-LIGHTTURN) remain recorded in
`mutations-redb.json` / `mutations-final.json` and still apply to the response pools, the work deadline,
the reservation lifetime and the light lane.
