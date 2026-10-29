# Journal round-3 evidence (audit-2026-10-22, TRACK journal)

New logs only; no earlier evidence file is edited. Report: `../../JOURNAL-GROWTH-AND-REPLAY.md` (round 3).

**Code under test: `7f587bb`** (branch `wf/journal-r3`) = `fa97594` + `prototypes/pocold/tests/journal_measure.rs`
only (`git diff --stat fa97594 7f587bb` lists that one file; every `src` file is fa97594's). Host Linux
6.18.44-fc-v153, 4 CPUs. Build: `cargo test --release --locked --test journal_measure --no-run` (toolchain
1.99.1 from `rust-toolchain.toml`, `CARGO_TARGET_DIR` in the agent scratch dir), under `flock cpu.lock`,
no warnings. Every measurement ran under `flock cpu.lock` as the release test binary run directly:
`<bin> <test> --exact --ignored --nocapture --test-threads=1`, with the environment in the table
(`TMPDIR` in the scratch dir). Each log starts with its command line and ends with
`=== end <UTC time> exit=<code>`; each reached `test result: ok`. Nothing timed out.

A first pass on `346246e` (the commit before 7f587bb) was **stopped** (exit 143) during
`jm_state_size_term`: the "both" series stalled in the harness PoW search, because signed harness blocks
were stamped 3 s ahead of the ASERT schedule and hardened the target exponentially (fixed in 7f587bb;
re-execution costs do not depend on it). That pass is incomplete; its logs are not used and not
committed. Every log below is from 7f587bb.

Not re-run this round: the adversarial re-fetch witness (gitignored, round-2 log
`../witness-refetch-below-khist.log`), `jm_journal_bytes_per_record`'s JM-BYTES figures (round 2), and
`jm_deep_fork_rebuild_vs_height` (round 2); round 3 cites them.

| log | test | env | start (UTC) | end | result |
|---|---|---|---|---|---|
| `exec-repeat.log` | `jm_exec_cost_per_gas` | `(defaults)` | 2026-10-24T01:40:20Z | 2026-10-24T01:41:37Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 76.26s |
| `exec.log` | `jm_exec_cost_per_gas` | `(defaults)` | 2026-10-24T00:50:16Z | 2026-10-24T00:51:29Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 73.17s |
| `openmem-empty-1.log` | `jm_open_memory` | `JM_MEM_KIND=empty JM_MEM_OPEN=<scratch>/memdirs/empty` | 2026-10-24T01:23:35Z | 2026-10-24T01:23:44Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 8.46s |
| `openmem-empty-2.log` | `jm_open_memory` | `JM_MEM_KIND=empty JM_MEM_OPEN=<scratch>/memdirs/empty` | 2026-10-24T01:23:56Z | 2026-10-24T01:24:04Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 8.20s |
| `openmem-empty-build.log` | `jm_open_memory` | `JM_MEM_KIND=empty JM_MEM_DIR=<scratch>/memdirs/empty` | 2026-10-24T01:22:49Z | 2026-10-24T01:23:23Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 33.75s |
| `openmem-full_body_max-1.log` | `jm_open_memory` | `JM_MEM_KIND=full_body_max JM_MEM_OPEN=<scratch>/memdirs/full_body_max` | 2026-10-24T01:22:23Z | 2026-10-24T01:22:24Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.55s |
| `openmem-full_body_max-2.log` | `jm_open_memory` | `JM_MEM_KIND=full_body_max JM_MEM_OPEN=<scratch>/memdirs/full_body_max` | 2026-10-24T01:22:37Z | 2026-10-24T01:22:38Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.56s |
| `openmem-full_body_max-build.log` | `jm_open_memory` | `JM_MEM_KIND=full_body_max JM_MEM_DIR=<scratch>/memdirs/full_body_max` | 2026-10-24T01:22:08Z | 2026-10-24T01:22:11Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 2.33s |
| `openmem-full_gas_transfers-1.log` | `jm_open_memory` | `JM_MEM_KIND=full_gas_transfers JM_MEM_OPEN=<scratch>/memdirs/full_gas_transfers` | 2026-10-24T01:24:30Z | 2026-10-24T01:24:32Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 2.76s |
| `openmem-full_gas_transfers-2.log` | `jm_open_memory` | `JM_MEM_KIND=full_gas_transfers JM_MEM_OPEN=<scratch>/memdirs/full_gas_transfers` | 2026-10-24T01:24:42Z | 2026-10-24T01:24:45Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 2.59s |
| `openmem-full_gas_transfers-build.log` | `jm_open_memory` | `JM_MEM_KIND=full_gas_transfers JM_MEM_DIR=<scratch>/memdirs/full_gas_transfers` | 2026-10-24T01:24:04Z | 2026-10-24T01:24:17Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 12.93s |
| `rebuild-kind.log` | `jm_rebuild_by_kind` | `JM_REBUILD_KINDS=empty,full_gas_transfers,full_gas_compute,bn_pair_2,kzg_point_eval,modexp_b8_e256_m8_even,modexp_b8_e1024_m8_even` | 2026-10-24T00:58:05Z | 2026-10-24T01:00:37Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 152.20s |
| `reorg-loaded.log` | `jm_deep_reorg_latency` | `JM_REORG_HEIGHT=100 JM_REORG_KIND=modexp_b8_e256_m8_even` | 2026-10-24T01:08:21Z | 2026-10-24T01:21:56Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 815.01s |
| `reorg.log` | `jm_deep_reorg_latency` | `(defaults)` | 2026-10-24T01:07:54Z | 2026-10-24T01:08:09Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 15.34s |
| `replay-empty.log` | `jm_replay_empty_blocks` | `JM_CHECKPOINTS=1000,2500,5000,10000,15000,20000` | 2026-10-24T01:30:03Z | 2026-10-24T01:40:10Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 607.55s |
| `replay-kind.log` | `jm_replay_by_kind` | `JM_KINDS=empty,full_gas_transfers,log_heavy_546,full_body_max,full_gas_compute,bn_pair_2,kzg_point_eval,modexp_b8_e256_m8_even,modexp_b8_e1024_m8_even` | 2026-10-24T01:00:51Z | 2026-10-24T01:07:40Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 409.32s |
| `split.log` | `jm_replay_component_split` | `JM_SPLIT_N=20000` | 2026-10-24T01:24:55Z | 2026-10-24T01:29:53Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 298.20s |
| `state-syn.log` | `jm_state_root_synthetic` | `(defaults)` | 2026-10-24T00:56:52Z | 2026-10-24T00:57:53Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 61.36s |
| `state.log` | `jm_state_size_term` | `(defaults)` | 2026-10-24T00:51:42Z | 2026-10-24T00:56:37Z exit 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 294.80s |
| `suite-journal_measure.log` | `cargo test --release --locked --test journal_measure` | `(the non-ignored test)` | 2026-10-24T01:41:52Z | 2026-10-24T01:41:53Z exit 0 | test result: ok. 1 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 1.39s |

`<scratch>` is the agent scratch directory (journals built there and deleted after the runs).

Other files:
- `derived.log`: every derived figure of the report, printed by
  `python3 -I tools/journal_projections.py evidence` (run from `development/audit-2026-10-22`; stdlib only).
  Projections in it are arithmetic on the measured per-block costs above, not measurements.
- `ffi-check.log`: the cfg-gated `malloc_trim` helper of `journal_measure.rs` (`trim_heap`, identical in
  346246e and 7f587bb), copied verbatim into a standalone crate with `#![deny(warnings)]`, checked with
  `cargo check --target x86_64-pc-windows-msvc` and `cargo check` (host), toolchain 1.99.1, `--offline`:
  both exit 0. The whole test file was not built for Windows here (blst needs an MSVC compiler); no new
  crate and no new windows-sys feature.
- `suite-journal_measure.log`: `cargo test --release --locked --test journal_measure` on 7f587bb: the one
  non-ignored test (`jm_journal_bytes_per_record`) passes; the ten measurements are ignored by design.
