# LP3 S1 first increment, round 2: gsVersion detail and bounded `genesis-decode` (author evidence)

**Status: author fixes and self-tests only. Not independently re-reviewed. Not LP3 acceptance. S0 is not complete.**

Mode: manual, owner-prompted cloud session. Codex, the local coordinator and automatic dispatch were not invoked. Branch `work/lp3-genesisspec-s1` (draft PR #10); round-1 commit `c9364a2`, whose evidence in `../author-r1/` is unchanged. Accepted M1 0.32, LP1/LP2 sources, `Cargo.lock` and platform locking are unchanged.

## The two reported issues

The reviewer's full report, inputs and memory limit are not available in this session. Everything below was derived here. Where a number matches the report, that is noted, but it is not proof that the inputs were the same.

### 1. gsVersion details masked by the differential exception

- **Reported:** 57 error-detail mismatches hidden by the gsVersion comparison exception.
- **Reproduced here** (`reproduction-c9364a2/`): the round-1 harness with only that exception removed (`round1-diff-without-exception.patch`; Python's int-to-string digit limit lifted), run on the round-1 binary built from `c9364a2`, same default seed. Result: 30059 inputs, **57 mismatches, all `gsVersion`**. In every case the reference detail is the decimal value of a specVersion item of 17 to 33 bytes, while round 1 printed `specVersion above 2^128`. The count matches the reported 57.
- **Contract:** the module documents details "exactly as the reference". The reference raises `GsError('gsVersion', value)` with the integer value of the specVersion item, of any length.
- **Fix:** `src/decimal.rs` renders the exact decimal of any length. It splits base-2^32 limbs recursively and multiplies with Karatsuba in base 10^9, so the cost is O(n^1.59 log n) instead of O(n^2). The fixed string is removed. Error precedence and acceptance are unchanged: the stage order and every other check are byte-for-byte the same rules, and the differential below confirms it.
- **Harness:** the exception is removed. Every detail is now compared as a plain string. The harness lifts Python's 4300-digit `str(int)` limit (Python ≥ 3.11), because the reference reports the integer itself. Any `refused` row counts as a mismatch.

### 2. Unbounded CLI input

- **Reported:** SIGABRT under a memory limit when reading a large input file.
- **Reproduced here** (`reproduction-c9364a2/round1-large-one-line.log`), with my own input and limit: a 192 MiB file holding one hex line of a 96 MiB input, under `RLIMIT_AS` = 256 MiB. Round 1 printed `memory allocation of 100663301 bytes failed` and was killed by **SIGABRT**. Cause: `std::fs::read_to_string` loaded the whole file, then `hex::decode` allocated the bytes, then the decoder built a node arena (about 40 bytes per input byte).
- **Fix** (`src/genesis_batch.rs`, called from `main.rs`):
  - The file is streamed line by line through a fixed line buffer.
  - Two buffers are reserved once with `try_reserve_exact` and reused for every line: `MAX_LINE_BYTES` = 2·MAX_SPEC_BYTES + 4 and `MAX_SPEC_BYTES`, 8,455,426 bytes in total. A failed reservation exits 2 with a message, not an abort.
  - A line over the limit is consumed in the reader's own chunks without being stored. It is reported as `{"refused":"inputTooLarge","lineBytes":N,"maxSpecBytes":2818474}`, and the next line is processed.
  - A line that is not hex is reported as `{"refused":"hex"}`. Round 1 stopped the whole command on bad hex.
  - Exit status: 0 when every line was decoded or rejected by the decoder; 1 when any line was refused; 2 on an I/O or reservation error.
- **Limit and justification:** `MAX_SPEC_BYTES` = 2,818,474 bytes is computed in `genesis_spec.rs` from the schema. It is the largest encoding that can pass decode and ParamGate R12 (`M_max ≥ |M_0List|`, with `M_max` a u16): every field at its widest plus 65535 members. A test builds that exact spec and decodes it. Every possibly valid GenesisSpec therefore fits.
- **Validity is not affected:** this is a tool limit, not a decode rule. `decode` itself has no cap and still accepts a spec with 65536 members (R12 belongs to ParamGate), and a test shows this. A refused line gets no gs* code and no verdict.
- **Decoder memory:** framing is now validated with a stack of list-end offsets only, and the stages re-walk the validated bytes. The node arena is gone. `encode` computes lengths first and allocates once.

## Results (Linux x86_64, rustc/cargo 1.58.1, offline locked; Python 3.13.16)

Exit codes, signals and durations: `commands.json`. Status meanings:

- **PASS:** the check ran and passed.
- **FAIL:** the check ran and failed.
- **BLOCKED BY PLATFORM:** the check ran on Linux but needs Windows behavior.
- **NOT RUN:** the check did not run here.

| Check | Result | Status |
|---|---|---|
| `cargo build --offline --locked` (debug, release) | exit 0, exit 0 | PASS |
| `cargo fmt -- --check` | exit 0 | PASS |
| lib unit tests | 49/49 (round 1: 44; +3 `decimal`, +2 `genesis_batch`) | PASS |
| `tests/genesis_spec.rs` | 16/16 (round 1: 14; +exact gsVersion detail at 2–4096 bytes, +`MAX_SPEC_BYTES` = largest valid encoding) | PASS |
| `tests/genesis_batch_alloc.rs` (new, counting allocator) | 6/6 cases in debug, 7/7 in release with `LP3_ALLOC_MAX_VERSION=1` | PASS |
| adversarial 5/5, fixtures_integration 12/12, http_loopback 2/2, asert_alloc (1069 calls, 0 allocations) | unchanged | PASS |
| `store_journal.rs::writer_is_explicitly_unsupported_off_windows` | 1/1 | PASS |
| `store_journal.rs`, 11 writer tests | fail at `Writer::open`/`init` with `UnsupportedPlatform` (Windows-only writer, by LP2 design) | **BLOCKED BY PLATFORM** |
| `store_journal.rs::exclusive_writer_in_process`, `store_process.rs` (2 + helper) | `cfg(windows)`, not compiled on Linux | **NOT RUN** |
| `verify-fixtures` | windows 22/22, ASERT 1027, hash 3677 | PASS |
| `tests/e2e_http.py` | 45/45 | PASS |
| `tests/e2e_store.py` | `store init` exits 2 `unsupportedPlatform` | **BLOCKED BY PLATFORM** |
| `tests/lp3_genesis_diff.py` (debug, no exception) | 30075/30075 agree, 0 refused | PASS |
| `tests/lp3_genesis_diff.py --max-version` (release) | 30077/30077 agree, including a 2,818,466-byte gsVersion (6,787,543 digits) | PASS |
| `tests/lp3_genesis_cli.py` (debug) | 9/9 | PASS |
| `tests/lp3_genesis_cli.py --max-version` (release) | 10/10 | PASS |
| Mutation controls (12 single faults) | see below | see below |
| Any check on Windows at this commit | no Windows host in this session | **NOT RUN** |

`cargo test` as a whole exits 101 on Linux, only because of the 11 BLOCKED BY PLATFORM writer tests.

### Bounded memory, measured

`tests/genesis_batch_alloc.rs` measures heap bytes in use during `genesis_batch::run` with a counting global allocator. It runs on every platform. Peak above the 8,455,426 reserved bytes:

| Input | Above buffers | Asserted bound |
|---|---|---|
| one 256 MiB line (generated, never stored) followed by `c0` | 15 B | 64 KiB |
| 30000 GSV1 lines (~20 MB stream) | 421 B | 64 KiB |
| MAX_SPEC_BYTES + 1 (refused), then the largest valid spec | 5,439,663 B | 2·MAX |
| deepest nesting within MAX_SPEC_BYTES | 8,388,608 B | 3·MAX |
| flat list of MAX_SPEC_BYTES − 4 single bytes | 32 B | 64 KiB |
| gsVersion of 256 KiB | 1,945,536 B | 10·256 KiB + 64 KiB |
| gsVersion of 2,818,466 B (release) | 19,106,628 B | 10·MAX |

`tests/lp3_genesis_cli.py` runs the real binary under `RLIMIT_AS` (address space, including the binary's own mappings). It reads the child's peak RSS with `wait4` from a minimal helper interpreter. A forked child's `ru_maxrss` otherwise includes the parent script's memory: the first attempt showed this, and it was corrected. The helper's own baseline is about 6.4 MiB (6576 KiB). RSS values below are MiB.

| Check (limit) | Debug | Release |
|---|---|---|
| 192 MiB one-line file (256 MiB) | exit 1, refused, RSS 9.3 | exit 1, RSS 8.0 |
| 192 MiB one-line file (64 MiB) | exit 1, refused, RSS 9.4 | exit 1, RSS 8.1 |
| MAX+1 refused, largest valid spec, `c0` (64 MiB) | exit 1, rows refused / ok (65535 members) / gsCount, RSS 17.4 | RSS 16.0 |
| deepest nesting and flat list at MAX (64 MiB) | exit 0, gsStructure top[0] / top[2], RSS 17.3 | RSS 16.1 |
| 100000 GSV1 lines, ~68 MB file (64 MiB) | exit 0, 100000 accepted, RSS 6.7 | RSS 6.6 |
| gsVersion 256 KiB, exact digits (64 MiB) | exit 0, RSS 7.2, 12.7 s | 0.49 s |
| gsVersion 2,818,466 B, exact 6,787,543 digits (64 MiB) | not run in debug | exit 0, RSS 34.7, **23.05 s** |
| line forms: `0x`, CRLF, empty, odd, non-hex | exit 1, expected rows | same |
| reservation failure (8 MiB) | exit 2, `cannot reserve the line buffer…`, no signal | same |

Under a 64 MiB address-space limit, none of these inputs aborts. Without `RLIMIT_AS` (Windows), the script reports its limit checks as BLOCKED BY PLATFORM.

### Mutation controls

Each of 12 faults was applied alone to a scratch copy. Each copy ran the lib, `genesis_spec` and `genesis_batch_alloc` tests, the differential and the CLI harness (`mutation-controls.json`). The repository source was not modified.

| Mutant | Fault | Rust tests | Differential (mismatches) | CLI harness (failed checks) |
|---|---|---|---|---|
| M1-lenBeforeInt | gsLen root checked before gsInt (stage order) | caught | caught (37) | — (0) |
| M2-noGamma | gamma_bp upper bound removed | caught | caught (9) | — (0) |
| M3-wrappedIsL0 | wrapped single byte rejected as L0 (LP1 convention) | caught | caught (2403) | — (0) |
| M4-reservedHigh | reserved id range ends at C0C0FE | caught | caught (4) | — (0) |
| M5-duplicateIds | duplicate member ids allowed | caught | caught (220) | — (0) |
| M6-chainIdZero | chainId 0 accepted | caught | caught (31) | — (0) |
| M7-sysBeforeOrder | gsSys judged before gsOrder | caught | caught (810) | — (0) |
| M8-crossesAsTruncated | L0 detail changed | caught | caught (2951) | — (0) |
| M9-versionLow128 | gsVersion detail keeps only the low 128 bits (masking-like) | caught | caught (69) | caught (1) |
| M10-karatsubaShift | Karatsuba high product shifted one digit | caught | caught (6) | caught (1) |
| M11-noInputCap | per-input cap not enforced below the line cap | caught | — (0) | caught (2) |
| M12-unboundedLineRead | whole line read into memory regardless of size | caught | — (0) | caught (2) |

Result: 12 of 12 faults detected. Each decoder fault is caught by the Rust tests and the differential. The two version-detail faults (M9, M10) are also caught by the CLI harness. The two limit faults (M11, M12) are caught by the allocation test and the CLI harness; under M12 the binary again aborts with SIGABRT under 64 and 256 MiB limits, as in round 1.

## Remaining limitations

- **CPU:** the only superlinear path is the gsVersion detail. A full-size specVersion item takes 23 s in release on this container, and about 13 s for 256 KiB in debug. CPython 3.13 needs about 6 s for the same `str(int)`, and Python ≥ 3.11 refuses it by default above 4300 digits. A faster method (FFT/NTT multiplication) was not added.
- **Abort under very low limits:** decoder working memory (framing stack, result, decimal workspace) is ordinary allocation. Below the measured working set (about 35 MiB RSS at worst; 8 MiB is already refused at reservation), the process can still abort. The reserved buffers fail cleanly, but per-input working memory does not.
- **Library callers:** the cap applies to `genesis-decode`. A future file or profile loader must bound its own input; `MAX_SPEC_BYTES` is exported for that.
- **Measurement method:** allocator peaks count requested bytes, and `realloc` is counted as growth in place. RSS figures come from Linux `wait4`.
- **Windows:** no Windows run at this commit. The LP2 writer, crash and store end-to-end checks remain BLOCKED BY PLATFORM / NOT RUN here. The new CLI limit checks have no `RLIMIT_AS` on Windows.
- **Review:** no independent review of round 2.
