# Journal round-3b evidence (audit-2026-10-22, TRACK journal)

Second fix pass of round 3. New logs only; no earlier evidence file is edited. Report:
`../../JOURNAL-GROWTH-AND-REPLAY.md` (§3.2a, §3.2b, §10 J-REV3-1/2/3).

**Code under test: `e3dfb47`** (branch `wf/journal-r3b`, base `256428f`) = the round-3 tree plus
`prototypes/pocold/tests/journal_measure.rs` (six state-interacting compute kinds in `compute_kinds` and a
new `#[ignore]` test `jm_create_state_term`) and `tools/journal_projections.py` (a guarded `[10]/[11]`
section). No `src` file changed from `256428f`/`fa97594` (`git diff --stat fa97594 e3dfb47 -- prototypes/*/src`
is empty). Host Linux 6.18.44-fc-v153, 4 CPUs. Build:
`cargo test --release --locked --test journal_measure --no-run` (toolchain 1.99.1 from
`rust-toolchain.toml`, `CARGO_TARGET_DIR` in the agent scratch dir), under `flock cpu.lock`, no warnings.
Every measurement ran under `flock cpu.lock` as the release test binary run directly:
`<bin> <test> --exact --ignored --nocapture --test-threads=1`, `TMPDIR` in the scratch dir. Each log
starts with its command line and ends with `=== end <UTC time> exit=<code>`; each reached
`test result: ok`. Nothing timed out.

| log | test | env | start (UTC) | end | result |
|---|---|---|---|---|---|
| `create.log` | `jm_create_state_term` | `(defaults)` | 2026-10-24T05:47:29Z | 2026-10-24T05:49:04Z exit 0 | test result: ok. 1 passed; finished in 68.23s |
| `exec.log` | `jm_exec_cost_per_gas` | `(defaults; now 51 kinds)` | 2026-10-24T05:50:07Z | 2026-10-24T05:51:37Z exit 0 | test result: ok. 1 passed; finished in 74.87s |
| `exec-repeat.log` | `jm_exec_cost_per_gas` | `(defaults)` | 2026-10-24T05:51:47Z | 2026-10-24T05:54:35Z exit 0 | test result: ok. 1 passed; finished in 75.11s |
| `suite-journal_measure.log` | `cargo test --release --locked --test journal_measure` | `(the non-ignored test)` | 2026-10-24T05:57:24Z | 2026-10-24T05:59:44Z exit 0 | test result: ok. 1 passed; 11 ignored; finished in 1.22s |

What each log shows:
- `create.log` — `JM-CREATE` (J-REV3-1). Growing the live user-account state (1-wei CALLs to fresh
  addresses) and timing, at each checkpoint, an empty child, a CREATE2-heavy child (one tx, 247 creates),
  a cold-CALL control (one tx, ~3000 value-0 cold CALLs, no creates) and a many-small-create block
  (149 one-create txs). The create-specific term `create2ChildMs − emptyChildMs` rises from 15.6 ms at
  5 live accounts to 487 ms at 15,645 (≈ 63 → ≈ 1972 µs/create); the cold-CALL control tracks the empty
  child (no `current_total` scan). This is the O(creates × live-accounts) regime of
  `code_caps::current_total`, not captured by the additive §3.2b root/clone term.
- `exec.log` / `exec-repeat.log` — `JM-EXEC` with the six new one-tx full-gas kinds (`sstore_fresh`,
  `sstore_warm`, `cold_call`, `extcodecopy_cold`, `log0_loop`, `create2_loop`) measured on the small
  genesis state. On that state all six are ≤ 2.1 ns/gas (create2 ≈ 2.0, sstore_warm ≈ 1.2, cold_call
  ≈ 0.7, sstore_fresh ≈ 0.18, log0 ≈ 0.15 — `log0_loop` halts at LOG_COUNT_MAX, all gas charged), far
  below MODEXP's ≈ 134 ns/gas, which stays the per-gas worst on a small state. Their state dependence is
  in `create.log`. The two runs agree within ≈ ±5%.
- `suite-journal_measure.log` — `cargo test --release --locked --test journal_measure` on `e3dfb47`: the
  one non-ignored test (`jm_journal_bytes_per_record`, unchanged) passes; the eleven measurements
  (ten round-3 + `jm_create_state_term`) are ignored by design; 0 warnings.

Other files:
- `derived.log` — `python3 -I tools/journal_projections.py evidence` (run from
  `development/audit-2026-10-22`; stdlib only). Its sections `[1]`–`[9]` are byte-identical to
  `../journal-r3/derived.log` (the round-3 figures, reproduced here from the same inputs); the new
  `[10]` (the create-term fit and the devnet/network per-block projection — the network figure labelled
  an extrapolation of an extrapolation with its steps) and `[11]` (the devnet state term extended from
  the ≈ 95% measured fill to the caps) appear only because `journal-r3b/create.log` is present, so
  `../journal-r3/derived.log` stays reproducible when the script is run without this directory.

Not re-run this pass: the round-3 measurements (`../journal-r3/`), the adversarial re-fetch witness
(gitignored, round-2 `../witness-refetch-below-khist.log`), and the Windows FFI check (the cfg-gated
`trim_heap` helper is unchanged from round 3, so `../journal-r3/ffi-check.log` still applies; no new crate
or windows-sys feature was added).
