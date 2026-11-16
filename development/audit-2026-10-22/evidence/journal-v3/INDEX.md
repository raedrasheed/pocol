# Phase 5c journal measurements (v3 devnet and its spec-1 twin)

The logs behind JOURNAL-GROWTH-AND-REPLAY.md §14, produced on branch `claude/fresh-net-phase5bc-tests` (base main
`daabfbd`), release, one host (Linux 6.18.44, 4 CPUs), one run after the other. Only the `JM-*` lines and the test
result are kept. `JM_SPEC=1` selected the retired spec-1 devnet; without it the harness runs the v3 devnet. Phase 6a deleted the spec-1
rule set, its `_spec1` twin tests and `JM_SPEC=1` (now refused), so the spec-1 logs are historical: the commands below
reproduce only the v3 logs.

| log | command (from `prototypes/pocold`) |
|---|---|
| `bytes-v3.log`, `bytes-spec1.log` | `cargo +1.99.1 test --release --locked --test journal_measure jm_journal_bytes_per_record[_spec1] -- --nocapture --exact` (two default tests: the v3 one and its spec-1 twin) |
| `replay-empty-v3.log`, `replay-empty-spec1.log` | `JM_SPEC=0\|1 JM_HEIGHT=5000 JM_CHECKPOINTS=1000,2500,5000 cargo +1.99.1 test --release --locked --test journal_measure jm_replay_empty_blocks -- --ignored --nocapture --test-threads=1` |
| `replay-kind-v3.log`, `replay-kind-spec1.log` | `JM_SPEC=0\|1 JM_KIND_COUNT=20 cargo +1.99.1 test --release --locked --test journal_measure jm_replay_by_kind -- --ignored --nocapture --test-threads=1` |
