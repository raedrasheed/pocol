# Journal round-3c evidence (audit-2026-10-22, TRACK journal)

Third fix pass of round 3 (fourth review, JV3B-1 … JV3B-4). New logs only; no earlier evidence file is
edited. Report: `../../JOURNAL-GROWTH-AND-REPLAY.md` (round-3c note; §3.2b last bullet, §3.2c, §3.2d,
§3.3, §5.4, §7 option 7, §8, §12).

**Correction to `../journal-r3b/INDEX.md` (JV3B-4).** Its line 4 cites the report as "(§3.2a, §3.2b, §10
J-REV3-1/2/3)". The round-3b sections are **§3.2a, §3.2b, §3.2c and §11** (§10 is the round-3 table). That
file is existing evidence and is not edited; this note is the correction.

**Code under test: `1d094de`** (branch `wf/journal-r3c`, base `11f8e89`) = the round-3b tree plus one new
`#[ignore]` test `jm_tx_state_term` and two helpers (`pack_txs`, `zero_code_init`) in
`prototypes/pocold/tests/journal_measure.rs`. No `src` file differs from `fa97594`
(`git diff --stat fa97594 1d094de -- prototypes/*/src` is empty). Host Linux 6.18.44-fc-v153, 4 CPUs,
toolchain 1.99.1 (`rust-toolchain.toml`). Build `cargo test --release --locked --test journal_measure
--no-run` under `flock cpu.lock`: 0 warnings. Every run below: `cargo test --release --locked --test
journal_measure -- <test> --exact --ignored --nocapture --test-threads=1` under `flock cpu.lock`,
`CARGO_TARGET_DIR` and `TMPDIR` in the agent scratch dir. Each log starts with the command, `HEAD` and
the `git status` of `prototypes/` ("dirty:"), then the `JM_*` environment, `start` (UTC, taken before the
lock was acquired) and ends with `end <UTC> exit=<code>`. Every run reached `test result: ok`; nothing
timed out.

| log | test | env | start (UTC) | end | result |
|---|---|---|---|---|---|
| `txstate.log` | `jm_tx_state_term` | defaults (series accts, code, slots, all; 4 points) | 2026-10-24T08:24:16Z | 08:46:34Z exit 0 | ok, 1 passed, 1259.52 s |
| `create-r1.log` | `jm_create_state_term` | defaults (repeat 1 of the round-3b test) | 08:46:34Z | 08:50:01Z exit 0 | ok, 1 passed, 69.89 s |
| `create-r2.log` | `jm_create_state_term` | defaults (repeat 2) | 08:50:01Z | 08:52:47Z exit 0 | ok, 1 passed, 69.88 s |
| `suite-journal_measure.log` | `cargo test --release --locked --test journal_measure` | — | 08:52:47Z | 08:52:48Z exit 0 | ok, 1 passed, 12 ignored |
| `mutants/mut-none.log` | `jm_tx_state_term` on the mutant build, no mutant enabled | `JM_TX_SERIES=accts,code JM_TX_POINTS=1` | 08:54:02Z | 08:56:59Z exit 0 | ok, 176.92 s |
| `mutants/mut-A.log` | same, `ZZ_MUT_A=1` | same | 08:56:59Z | 08:59:41Z exit 0 | ok, 161.48 s |
| `mutants/mut-B.log` | same, `ZZ_MUT_B=1` | same | 08:59:41Z | 09:01:54Z exit 0 | ok, 87.66 s |
| `mutants/mut-C.log` | same, `ZZ_MUT_C=1` | same | 09:01:54Z | 09:05:30Z exit 0 | ok, 198.88 s |
| `mutants/mut-D.log` | same, `ZZ_MUT_D=1` | same | 09:05:30Z | 09:06:36Z exit 0 | ok, 55.42 s |

What each log shows:
- `txstate.log` — `JM-TXSTATE` (JV3B-1/2). Four fills from genesis: accounts (1-wei CALLs to fresh
  addresses, to 15,415 live accounts), code (62 deploys of a 32,768-B zero runtime: 2,047,125 logical
  bytes = 97.6% of the devnet CODE_CAP on 67 accounts), slots (fresh SSTOREs, to 46,636), and all three
  on one chain (code, then slots, then accounts: 18,828 / 46,636 / 2,047,125). At each checkpoint:
  `validate_snapshot`, `inventory` and one state clone alone (median of 5), then the validate of an
  empty child (5), a block of the most 21k transfers (380), a CREATE2-heavy child (247 creates) and a
  block of the most one-create txs (149) (median of 3). On every grown state the transfer block is the
  per-block worst; at the all-caps state 10.25 s, against 2.38 s for the CREATE2 child and 0.41 s for
  an empty block.
- `create-r1.log`, `create-r2.log` — `JM-CREATE`, two repeats of the round-3b `jm_create_state_term` on
  `1d094de` (JV3B-3); with `../journal-r3b/create.log` they give three runs per checkpoint.
- `suite-journal_measure.log` — the one non-ignored test passes; 12 measurements are ignored by design.
- `mutants/` — scratch timing mutants (JV3B survivors). The mutant source is a scratch patch (not
  committed) that adds env-gated edits to `prototypes/lp4-execution/src/{code_caps,native,pay}.rs`; it was
  built once (`--no-run`, 0 warnings) and reverted (`git checkout` of the three files) after the runs; the
  "dirty:" field of each log names the three patched files. `ZZ_MUT_A`: `code_caps::current_total`
  returns 0. `ZZ_MUT_B`: `native::execute_for_profile` skips `code_caps::validate_snapshot`.
  `ZZ_MUT_C`: `NativeContext::from_snapshot` makes one extra whole-snapshot clone and drops it
  (semantics unchanged). `ZZ_MUT_D`: `code_caps::code_len` skips the `hash_slow` comparison. All four are
  timing probes, not proposed changes. `mut-none.log` is the control on the same build.

Other files:
- `derived.log` — `python3 -I tools/journal_projections.py evidence` (from `development/audit-2026-10-22`).
  Its first 19,828 bytes are byte-identical to `../journal-r3b/derived.log` (sections [1]–[11]), whose
  [1]–[9] in turn equal `../journal-r3/derived.log`; the new sections, printed only when this directory is
  present, are [12] (the `JM-TXSTATE` table, per-block worst per state, slopes, network projection),
  [13] (devnet bounds on the all-caps state), [14] (the three `JM-CREATE` runs, median fit and range) and
  [15] (the mutant table).

Not re-run this pass: the round-3/3b measurements other than `jm_create_state_term`, the adversarial
re-fetch witness (gitignored), and the Windows FFI check: the cfg-gated `trim_heap` helper is unchanged
from round 3 (`../journal-r3/ffi-check.log` still applies), and `jm_tx_state_term` is pure Rust over the
crate's public API; no new crate or windows-sys feature was added.
