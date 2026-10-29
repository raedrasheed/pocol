# LP3 S1 round 3: CRLF harness fix and manual Windows workflow (author evidence)

**Status: author work only. No Windows run has been performed, nothing has been reviewed, and this is not LP3 acceptance.** Made in a manual, owner-prompted session. Codex, the local coordinator and automatic dispatch were not invoked. Earlier evidence in `../author-r1/` and `../author-r2/` is unchanged.

## 1. CRLF issue in the Python harness

### Problem

`tests/lp3_genesis_cli.py` wrote its generated input files in text mode (`open(path, 'w')`). On Windows, Python then translates every `\n` into `\r\n`, and an explicit `\r\n` into `\r\r\n`. That changes the bytes the test intends to feed to `genesis-decode`.

`tests/lp3_genesis_diff.py` had the same pattern for its corpus file (`Path.write_text`).

### Reproduction

There is no Windows host in this session, so this is a simulation. `crlf-reproduction/simulate_windows_newlines.py` runs a harness unchanged, giving every text-mode write `newline='\r\n'`. That is the translation Python applies on Windows. Binary writes are untouched, and with `--keep` the generated inputs are kept for hashing. The binary is the round-2 release build of `26ee0a4`.

| Harness | Native Linux | Simulated Windows newlines |
|---|---|---|
| `lp3_genesis_cli.py` at `26ee0a4` | 9 PASS | **3 FAIL**, 6 PASS |
| `lp3_genesis_cli.py` fixed | 9 PASS | 9 PASS |

The three failures (`cli-before-windows.json`):

- **`lineForms`:** this is the reported issue. `0x<GSV1>\r\n` became `\r\r\n`. One `\r` survived stripping, so the line was refused as non-hex, and the summary counts were off.
- **`largeOneLine192MiB.limit256MiB` and `.limit64MiB`:** the over-long line kept its `\r`, so `lineBytes` was one byte larger than expected. On real Windows these two checks are BLOCKED BY PLATFORM (no `RLIMIT_AS`). So on Windows only `lineForms` would have failed.

Generated input files compared with the intended bytes, i.e. round 2 on Linux (`input-hashes-before.txt`, `input-hashes-after.txt`):

| Run | Inputs identical to intended |
|---|---|
| round 2, native | 6/6 |
| round 2, simulated Windows | **0/6** |
| fixed, native | 6/6 |
| fixed, simulated Windows | 6/6 |

The differential corpus file shows the same pattern:

- Round 2 under simulated Windows wrote `a76aca47…`. Its 30075/30075 agreement still held only because `genesis-decode` strips one `\r`.
- The fixed harness writes `22f80cbb…` in both modes, which equals the recorded `corpusSha256`.

### Fix

- **`lp3_genesis_cli.py`:** `write()` opens the file in binary mode (`'wb'`) and writes `str` chunks as ASCII bytes. This is an explicit byte write, never newline-translated.
- **`lp3_genesis_diff.py`:** the corpus is written with `Path.write_bytes(text.encode('ascii'))`.
- **Inputs preserved:** every generated input is byte-identical to the round-2 inputs, so no expectation changed.
- **Not changed:**
  - `e2e_store.py` and `e2e_http.py` already use binary I/O.
  - JSON output files are still written in text mode. Only JSON parsers read them back, so line endings don't matter.

## 2. Windows workflow (prepared, not run)

Files:

- `.github/workflows/lp3-s1-windows-verify.yml`
- `.github/scripts/lp3-s1-windows-verify.ps1`

**Trigger and security:**

- **Trigger:** `workflow_dispatch` only. Inputs are `commit` (optional exact 40-hex SHA, validated before checkout) and `include_slow` (boolean). There is no push or pull_request trigger.
- **Permissions:** `contents: read` only. The checkout uses `persist-credentials: false`, and no secrets are used.
- **Script injection:** inputs reach scripts only through environment variables.
- **Pinned actions** (full commit SHAs, all verified as commits):

  | Action | Version | SHA |
  |---|---|---|
  | actions/checkout | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
  | actions/setup-python | v7.0.0 | `5fda3b95a4ea91299a34e894583c3862153e4b97` |
  | actions/upload-artifact | v7.0.2 | `cf430e030ddbb5b0abf93d22962f4752f3646cd9` |

**Runner:**

- GitHub-hosted `windows-2022` (no self-hosted runner).
- `timeout-minutes: 90`, one run at a time.

**Toolchain:**

- `rustup toolchain install 1.58.1-x86_64-pc-windows-msvc --profile minimal -c rustfmt --no-self-update`.
- `RUSTUP_TOOLCHAIN` is set for child processes only, and the script asserts `rustc` reports release 1.58.1.
- `cargo fetch --locked` runs once; every build and test then uses `--offline --locked`.
- Python 3.11 comes from `actions/setup-python`.

**Checks, in order:**

1. Toolchain.
2. Debug and release builds.
3. `cargo fmt -- --check`.
4. `store_journal`.
5. `store_process` (real-process exclusive writer, kill release, crash recovery).
6. Full `cargo test --no-fail-fast`.
7. `genesis_batch_alloc`, in debug and in release with `LP3_ALLOC_MAX_VERSION=1`.
8. `verify-fixtures`.
9. `e2e_http.py`.
10. `e2e_store.py`.
11. `lp3_genesis_diff.py`.
12. `lp3_genesis_cli.py`, whose own sub-checks are reported individually.
13. With `include_slow`: the release differential and CLI harness with a full-size gsVersion.

**Statuses:**

- **PASS:** exit 0 and, where tests are expected, at least one test actually ran.
- **FAIL:** non-zero exit or timeout.
- **NOT RUN:** optional, skipped after an earlier failure, or zero tests compiled for the platform.
- **BLOCKED BY PLATFORM:** the CLI harness's `RLIMIT_AS` checks on Windows.

Off Windows (smoke runs only), Windows-only storage checks are also BLOCKED BY PLATFORM, but only for the exact known `UnsupportedPlatform` failures. The full suite is BLOCKED there only if every failing test is one of those blocked `store_journal` tests. Negative controls show that any other failure stays FAIL. The job turns red if any check FAILS.

**Artifacts:** `lp3-s1-windows-<commit>`, kept 14 days, containing:

- `results.json`: machine-readable commit, `trackedFilesModified`, toolchain, OS, run URL, status counts, and per check the status, exit code and detail.
- `summary.md`, also written to the job summary.
- `environment.json`: runner image, Windows, PowerShell, git, Python, rustup, rustc `-vV`, cargo, rustfmt, script SHA-256, workflow ref and SHA.
- `commands.json`: each command with its exit code and seconds.
- `source-hashes.json`: SHA-256 of every crate and `.github` file.
- `logs/`.
- The harness JSON files.

## 3. Why it was not run, and how to run it

- **Capacity not confirmed:**
  - The repository is private (the GitHub API reports `visibility: private`). Runs therefore use the account's included Actions minutes, and GitHub counts Windows minutes at a higher rate (2× in its billing documentation).
  - This session cannot see the plan, the minutes already used, or the spending limit. So included capacity is not confirmed, and the workflow was not dispatched.
  - No push or pull_request trigger was added, so pushing it does not start a run. The repository had no workflows and no runs before this change.
- **Dispatch prerequisite:** GitHub can dispatch a workflow only once its file exists on the default branch (`main`). I was authorized to change only the PR branch.
  - To run it, the owner adds `.github/workflows/lp3-s1-windows-verify.yml` to `main` (the script is read from the tested branch).
  - Then: Actions → **LP3 S1 Windows verification (manual)** → Run workflow, with branch `work/lp3-genesisspec-s1`, `commit` set to the SHA to verify, and `include_slow` optional.
  - Alternatively, authorize a push or pull_request trigger on the PR branch.
- **Expected cost (estimate, not measured on Windows):** the Linux smoke run below took about 4 (248 s) minutes on this container. A hosted Windows run will likely take 20–40 minutes, before the Windows minute multiplier.

## 4. Local validation (not Windows evidence)

- `actionlint` 1.7.12 (checksum-verified download): no findings (`actionlint.txt`).
- PowerShell 7.4.6 parser: 0 errors in the script, and no syntax specific to PowerShell 7 in the script.
- Negative controls of the off-Windows classifier:
  - only known journal failures → BLOCKED;
  - journal failures plus another failure → FAIL;
  - a build error → FAIL.
- `linux-smoke-not-windows-evidence/`: the CI script run end to end with PowerShell 7 on Linux, on clean commit `f2ac59960d6b64830265325c2d4a6e2d80d5be80` (`trackedFilesModified: false`): PASS 19, FAIL 0, BLOCKED BY PLATFORM 3 (`store-journal`, `cargo-test-full`, `e2e-store`: the Windows-only LP2 writer), NOT RUN 3 (`store-process-crash-recovery`: no test compiled off Windows; the two optional slow checks); the CLI harness `lineForms` check passes. This shows that the script, its classification and its evidence files work. It is not Windows verification.

## 5. Status of the Windows verification

| Item | Status |
|---|---|
| Every Windows check (Rust suite, storage, crash recovery, allocation, differential, CLI) | **NOT RUN**: no authorized Windows execution; workflow prepared, not dispatched |
| CRLF fix, simulated Windows newlines on Linux | PASS (simulation, not Windows) |
| Workflow lint and script parse | PASS |
| Linux smoke run of the CI script | PASS 19 / FAIL 0 / BLOCKED BY PLATFORM 3 / NOT RUN 3 (Linux only) |

No merge, no acceptance claim, no paid usage, no billing change, no self-hosted runner.
