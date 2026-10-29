# Caps track evidence, round 3b (the fix round after the third review)

These files support the round-3b revision of
[../../proposals/CAPS-SUSTAINABILITY.md](../../proposals/CAPS-SUSTAINABILITY.md) (§3a, §6, §7d, §7e).
That study is PROPOSED and not approved. The earlier files in [../caps/](../caps/) and
[../caps-r3/](../caps-r3/) are unchanged.

How the runs were made:
- Linux, release build, with the toolchain from `rust-toolchain.toml` (1.99.1).
- Each run used this track's own target directory, under the shared CPU lock. The whole pocold suite
  ran under the e2e lock as well.
- Each log names its commit and the exact command.
- The test code is e985ce1's (tests/caps.rs only; no source change since fa97594). The later commit on
  `wf/caps-r3b` changes only `development/`, so its `prototypes/` tree equals e985ce1's.
- No run hit a harness timeout. The 240 s limits inside `tests/caps.rs` are test assertions, and they
  pass on the unmutated code.

| file | commit | what it shows |
|---|---|---|
| `caps-tests-e985ce1.log` | e985ce1 | `tests/caps.rs`, 9 pass, no compiler warnings. Its `CAPS` lines: the TX fork case (A at 3 = TX_CAP, B at 1, room on B 2, templates [2,0,0]); the two boundary matrices (11 TX_CAP and 12 TXBYTES_CAP cases, each `[parentTxs, k, short, cap]`; every transfer is 113 B). Also `lp4-execution` `tests/caps_identity.rs`: 3 pass |
| `caps-test-build-e985ce1.log` | e985ce1, `--features caps-test` | `tests/log_cap.rs`, 2 pass (`exhaustedAt` 513, 22 blocks after exhaustion), and `tests/caps.rs`, 9 pass, in the caps-test build |
| `pocold-suite-e985ce1.log` | e985ce1 | the whole pocold suite, once, run with `--no-fail-fast`: 155 pass, 0 fail, 8 ignored, no compiler warnings (44 + 3 + 9 + 43 + 1 + 1 + 35 + 1 + 6 + 12; round 3 had 153, and the 2 more are the new matrix tests). The ignored tests are the 2 spec-depth tests and the 6 journal measurements, none of them run |
| `mutations-e985ce1.log`, `mutations-e985ce1.json` | e985ce1 | 84 mutants, one at a time, against `tests/caps.rs` (`log_cap` / `caps_identity` for the five LOG_CAP mutants): round 3's 55, the third review's 12 new ones (R1–R3, R6–R9, T1–T4, T8) and Y1–Y17, new in this round. **83 killed; R7 survives and is equivalent** (study §7e). Y5's first edit did not compile (recorded as incomplete in run 1); the corrected edit is killed in run 2. Only the outputs are recorded |
| `r7-probe-e985ce1.log` | e985ce1 | the R7 equivalence probe, unmutated and with R7: a sealed, correctly committed block of TX_CAP+1 transfers on a zero-count parent is `Block("16 caps")` without R7 and `Block("apply Budget(\"TX_MAX\")")` with it; as a template it is not locked in either case, and the valid control template locks in both |

These runs do not establish full acceptance. They cover the caps track's tests and the pocold suite
on Linux only. The Windows CI was not run in this round. The round changes only `tests/caps.rs`,
which has no platform code, so the new tests are not yet verified on Windows.

The new tests are regression tests that assert conformance (consensus.md:170 item 16, D49,
execution.md:83). Their devnets are in-process, with no network listener. The R7 probe builds an invalid
block on purpose, so it ran from a gitignored `tests/zz_audit_witness_caps_r7.rs`, was removed after the
run and is not committed; the log gives its sha256. The mutation runner and the mutant edits stay in the
scratch directory; only their outputs are recorded here.
