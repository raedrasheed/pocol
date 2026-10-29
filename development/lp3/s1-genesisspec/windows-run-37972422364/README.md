# Windows run 37972422364: original evidence and failure analysis

`artifact/` holds the workflow's uploaded artifact exactly as downloaded. Do not edit it. `.gitattributes` keeps its bytes from line-ending conversion, and `artifact-files.sha256` lists the SHA-256 of each file.

## Provenance

| Item | Value |
|---|---|
| Run | https://github.com/raedrasheed/Proof-of-Collaboration/actions/runs/37972422364 (run 1, attempt 1, `workflow_dispatch` by the owner, conclusion `failure`) |
| Workflow | `.github/workflows/lp3-s1-windows-verify.yml@refs/heads/main`, workflow SHA `15a38774d9b47f4282366d7b294a1d8b8e919b0d` (merge of PR #11) |
| Tested commit | `9c02c7f7c7f977d2f0ec97107cbee5fad7a1e8f0`, `trackedFilesModified: false` |
| Runner | `win22 20261004.326.1`, Microsoft Windows 10.0.20348, PowerShell 7.6.6, Python 3.11.9 (MSC v.1938 64-bit), git 2.56.0.windows.1 |
| Toolchain | `rustc 1.58.1 (db9d1b20b 2022-01-20)`, host `x86_64-pc-windows-msvc` |
| Options | `include_slow: true` |
| Script | `.github/scripts/lp3-s1-windows-verify.ps1` SHA-256 `c2eb204deff90d4577575e473f49f3f483998364ecbd2faa2949839405e3d43f` |
| Artifact | `lp3-s1-windows-9c02c7f7c7f977d2f0ec97107cbee5fad7a1e8f0`, id 11636752868, 21215 bytes, GitHub digest `sha256:26b5c4fab0e006808ec6dd95f11d62feec5dd4b3f95a7f51d9d3aaf5562e4073`; the downloaded zip has that same SHA-256 |

## Results reported by the run

PASS 12, FAIL 4, BLOCKED BY PLATFORM 1, NOT RUN 0 (`artifact/results.json`).

| Status | Checks |
|---|---|
| PASS | `toolchain`, `cargo-build`, `cargo-build-release`, `rustfmt-check`, `store-journal`, `store-process-crash-recovery`, `cargo-test-full`, `genesis-batch-alloc-debug`, `genesis-batch-alloc-release-max`, `verify-fixtures`, `e2e-http`, `e2e-store` |
| FAIL (exit 1) | `lp3-genesis-diff`, `lp3-genesis-cli`, `lp3-genesis-diff-release-max`, `lp3-genesis-cli-release-max` |
| BLOCKED BY PLATFORM | `RLIMIT_AS address-space limit checks` (Windows has no RLIMIT_AS) |

This is the first Windows execution of this code. The Rust suite, the LP2 storage journal, the real-process crash-recovery tests, both portable allocation tests (including the release run with a full-size gsVersion), fixtures, `e2e_http.py` and `e2e_store.py` all passed on Windows.

## The four failures: exact errors

All four logs (`artifact/logs/lp3-genesis-*.log`, 972–976 bytes) contain one traceback that ends in the same exception. Each process exited with code 1 within 0.07–0.15 s, before any check or comparison ran:

```
File "...\prototypes\lp1-node\tests\lp3_genesis_diff.py", line 323, in main
  gdoc = json.loads((m1 / 'm1-draft-0.21/vectors/v3-gsv1.json').read_text())
File "C:\hostedtoolcache\windows\Python\3.11.9\x64\Lib\pathlib.py", line 1059, in read_text
File "C:\hostedtoolcache\windows\Python\3.11.9\x64\Lib\encodings\cp1252.py", line 23, in decode
UnicodeDecodeError: 'charmap' codec can't decode byte 0x81 in position 423: character maps to <undefined>
```

`lp3_genesis_cli.py` fails the same way at line 115 (`doc = json.loads((m1 / 'm1-draft-0.21/vectors/v3-gsv1.json').read_text())`), in both its debug and release `--max-version` invocations.

## Confirmed cause

- **The read:** `Path.read_text()` was called without an encoding.
- **What Python did on Windows:** without UTF-8 mode, it uses the ANSI code page, which is cp1252 on this runner, as the traceback shows.
- **The file:** `development/m1/m1-draft-0.21/vectors/v3-gsv1.json` is valid UTF-8 and quotes Arabic source literals, 386 non-ASCII bytes in all.
- **The failing byte:** byte 423 is `0x81`, the second byte of `d9 81` (the letter ف in "حذف"). cp1252 has no character for `0x81`.
- **Also affected:** `c30-depth.json`, which the differential reads next, contains UTF-8 as well.
- **Why Linux didn't show it:** Python there uses UTF-8 by default.

The fix and its verification are in `../author-r4-encoding-fix/`.
