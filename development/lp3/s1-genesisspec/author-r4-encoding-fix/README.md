# LP3 S1 round 4: explicit UTF-8 in the Python harnesses (author evidence)

**Status: author fix with Linux-side verification only. It still needs a Windows run, has had no independent review, and is not LP3 acceptance.** Made in a manual, owner-prompted session. Codex and the coordinator were not invoked. The original Windows evidence is preserved unchanged in `../windows-run-37972422364/`.

## Failure being fixed

In Windows run 37972422364 (commit `9c02c7f`), the four Python harness checks failed. These were `lp3-genesis-diff` and `lp3-genesis-cli`, each in debug and in release `--max-version`. All four stopped with:

`UnicodeDecodeError: 'charmap' codec can't decode byte 0x81 in position 423`

The error came from `Path.read_text()` without an encoding. On that runner, Python's default text encoding is cp1252, while the M1 vector files are UTF-8. Exact tracebacks and the cause analysis are in `../windows-run-37972422364/README.md`.

## Reproduction (Linux simulation, not a Windows run)

`simulate_windows_text_defaults.py` runs a harness unchanged, but applies the Windows defaults:

- a text-mode `open()` with no encoding uses cp1252;
- `io.text_encoding(None)` returns `locale`. Without this, this Linux Python's UTF-8 mode hid the problem: the first attempt passed for exactly that reason;
- text-mode writes translate `\n` to `\r\n`;
- subprocess text mode without an encoding uses cp1252.

With `--no-resource`, the `resource` module is made unavailable, as on Windows, so the CLI harness takes its Windows code path.

Unfixed harnesses at `9c02c7f` (`reproduction-unfixed/`) fail with the identical exception. The traceback lines match the runner's: `lp3_genesis_diff.py` 386→323 and `lp3_genesis_cli.py` 246→115.

## Fix

Every text read and write in `tests/lp3_genesis_diff.py` and `tests/lp3_genesis_cli.py` now names `encoding='utf-8'`:

- both vector reads (`v3-gsv1.json`, `c30-depth.json`);
- the CLI output reads;
- the `subprocess.run` call (`encoding='utf-8'` instead of a bare `text=True`);
- the summary JSON writes.

Generated inputs were already written as exact bytes (round 3). No assertion, expectation, input or classification changed, and no Rust source changed.

## Verification (`fixed/`)

| Run | Result |
|---|---|
| Differential, simulated Windows defaults, debug | 30075/30075 agree |
| CLI harness, simulated Windows defaults, debug (RLIMIT path) | PASS 9 |
| Differential, simulated Windows defaults, release `--max-version` | 30077/30077 agree |
| CLI harness, simulated Windows defaults, release `--max-version` | PASS 10 |
| CLI harness, Windows code path (`--no-resource`) + simulated defaults, debug | PASS 2 (`lineForms`, `limitPlusOneThenMaxValid.noLimit`), BLOCKED BY PLATFORM 7 (the RLIMIT_AS checks) |
| Same, release `--max-version` | PASS 2, BLOCKED BY PLATFORM 8 |
| Both harnesses natively with `python -X warn_default_encoding -W error::EncodingWarning` | pass: no remaining implicit-encoding text I/O on the executed paths |
| Control: the unfixed CLI harness with the same flags | raises `EncodingWarning: 'encoding' argument not specified` |

In the `--no-resource` runs, the blocked reason says "on Linux" only because the simulation runs on Linux. On Windows the harness names Windows.

The Linux smoke run of the full CI script at the fix commit is in `linux-smoke-not-windows-evidence/`. It is not Windows evidence.

## Still required

A Windows run of the new commit, dispatched from `main` with that commit's full SHA. Expected outcome, unconfirmed until it runs:

- the same 12 Rust, storage and crash-recovery checks PASS as before;
- the four Python harness checks PASS;
- the CLI harness's RLIMIT_AS sub-checks are BLOCKED BY PLATFORM.
