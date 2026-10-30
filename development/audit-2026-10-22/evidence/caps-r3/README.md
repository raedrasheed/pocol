# Caps track evidence, round 3 (the fix round after the second review)

These files support the round-3 revision of
[../../proposals/CAPS-SUSTAINABILITY.md](../../proposals/CAPS-SUSTAINABILITY.md) (§5b, §6, §7b, §7c).
That study is PROPOSED and not approved. The stage A and round-2 files in [../caps/](../caps/) are
unchanged.

How the runs were made:
- Linux, release build, with the toolchain from `rust-toolchain.toml` (1.99.1).
- Each run used this track's own target directory, under the shared CPU lock. The whole pocold suite
  ran under the e2e lock as well.
- Each log names its commit and the exact command.
- The test code is 5965b8d's. Later commits on `wf/caps-r3` change only `development/`, so their
  `prototypes/` tree equals 5965b8d's.
- No run timed out. The 240 s limits inside `tests/caps.rs` are test assertions, and they pass.

| file | commit | what it shows |
|---|---|---|
| `caps-tests-5965b8d.log` | 5965b8d | `tests/caps.rs`, 7 pass, with the round-3 cases. Its `CAPS` lines: the cross-block fit gives templates [1,1,0,0] and 226 B = TXBYTES_CAP; the fork case has TXBYTES_CAP 828, A at 716 B (room 112), B at 113 B (room 715). Also `lp4-execution` `tests/caps_identity.rs`: 3 pass |
| `caps-test-build-5965b8d.log` | 5965b8d, `--features caps-test` | `tests/log_cap.rs`, 2 pass (`exhaustedAt` 513), and `tests/caps.rs`, 7 pass, in the caps-test build |
| `pocold-suite-5965b8d.log` | 5965b8d | the whole pocold suite, once, run with `--no-fail-fast`: 153 pass, 0 fail, 8 ignored, no compiler warnings. The ignored tests are the 2 spec-depth tests and the 6 journal measurements, none of them run |
| `mutations-5965b8d.log`, `mutations-5965b8d.json` | 5965b8d | 55 mutants, one at a time, against `tests/caps.rs` (`log_cap` / `caps_identity` for the five LOG_CAP mutants): the second review's B1–B12 and W1–W12, the first review's M1–M11, and X1–X10 new in this round. **55 of 55 killed**, none incomplete. Only the outputs are recorded |
| `a13-fa97594.log` | fa97594 | `a13_bound_breakdown` at the integrated head: total 5,577,343,580 B (= RPCGUARD.md:158) |
| `o2-memory-fa97594.json` | — | the O2 resident-memory arithmetic (§5b), in integers, on both bases (fa97594 and c3a78e7), for the three log-byte assumptions |
| `caps-model-5965b8d.log` | 5965b8d | `python3 -I proposals/model/caps_model.py`: 25 checks pass; the output is byte-identical to the committed `vectors/caps-sustainability.json` |

These runs do not establish full acceptance. They cover the caps track's tests and the pocold suite
on Linux only. The Windows CI was not run in this round. The round changes only `tests/caps.rs`,
which has no platform code, so it is not yet verified on Windows.

No exploit-shaped program was written. The new tests are regression tests that assert conformance
(consensus.md:170 item 16, D49, execution.md:83). Their devnets are in-process, with no network
listener. The mutation runner and the mutant edits stay in the scratch directory; only their outputs
are recorded here.
