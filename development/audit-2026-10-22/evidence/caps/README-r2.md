# Caps track evidence, round 2 (the fix round after the independent review)

These logs support the round-2 revision of
[../../proposals/CAPS-SUSTAINABILITY.md](../../proposals/CAPS-SUSTAINABILITY.md) (§6, §7), which is
PROPOSED and not approved. The stage A logs and [README.md](README.md) are unchanged.

- They were run on Linux, in release, with the toolchain from `rust-toolchain.toml` (1.99.1), using
  this track's own target directory under the shared CPU lock.
- Each log names the commit it ran on and the exact command.
- No run timed out. The 240 s limits of `tests/caps.rs` are test assertions, and they pass at the
  commits below.

| file | tree | what it shows |
|---|---|---|
| `r2-caps-tests-0b9b4d0.log` | commit 0b9b4d0 (its `prototypes/` tree equals c635594's) | `tests/caps.rs`, 7 pass, including the new `txbytes_cap_boundary_is_byte_exact_in_the_builder_and_in_item_16` (its `CAPS` lines: TXBYTES_CAP 226 = a0+b0, templates [2,0,0], room left 0; TXBYTES_CAP 225, templates [1,0,0], room left 112). Also `lp4-execution` `tests/caps_identity.rs` on the regenerated vectors: 3 pass |
| `r2-caps-test-build-0b9b4d0.log` | commit 0b9b4d0, `--features caps-test` | the same two test files in the caps-test build: 7 pass and 3 pass |
| `r2-suites-release-0b9b4d0.log` | commit 0b9b4d0 | the full `lp4-execution` (165 pass) and `pocold` (130 pass, 2 ignored spec-depth tests not run) suites, release build, no warnings |
| `r2-caps-model-0b9b4d0.log` | commit 0b9b4d0 | `python3 -I proposals/model/caps_model.py`: 25 checks pass; output byte-identical to the committed `vectors/caps-sustainability.json`; `emptyBlocksFromBaseFee0ToIdle` = 145 on every profile |
| `r2-mutations-c635594.log` | commit c635594, one mutant at a time | the review's 16 mutants of the builder, item 16 and template locking against the round-2 `tests/caps.rs`: 16 of 16 killed, including the review's six survivors (M2a, M2b, M2f, M2h, M3a, M3b), all by the new boundary test. Only the outputs are recorded; the runner and the mutant edits stay in the scratch directory |

Not rerun in round 2, because the round changes neither their code nor their tests: `tests/log_cap.rs`
(stage A, `caps-test-build-a0dd771.log`) and the caps-test full suites (`suites-caps-test-a0dd771.log`).
The one source change in round 2 is a comment in `pocold/src/node.rs`.

No exploit-shaped program was written. The new test is a regression test asserting the conformance
(execution.md:83, consensus.md:170, D49), and its devnets are in-process (no network listener).
