# Audit repair r2 Windows verification

- commit: `a03e089a35affc22fed9f5aa3c961eac97d61658` (requested `a03e089a35affc22fed9f5aa3c961eac97d61658`), dirty: False
- OS: Microsoft Windows NT 10.0.20348.0; PowerShell 7.6.6
- Rust: rustc 1.58.1 (db9d1b20b 2022-01-20) / rustc 1.99.1 (58cf8f9c3 2026-10-13); Node: v22.22.0

| check | status | detail |
|---|---|---|
| lp1-store-journal | PASS | Rust 1.58.1; 12 passed, 0 failed in 1 test binaries |
| lp1-node-all | PASS | Rust 1.58.1; 150 passed, 0 failed in 20 test binaries |
| lp4-execution | PASS | Rust 1.99.1; 162 passed, 0 failed in 19 test binaries |
| pocold | FAIL | exit 101; Rust 1.99.1; 75 passed, 48 failed in 12 test binaries; thread-count assertions not measured on this platform (THREADS-NOT-MEASURED) |
| extension-npm-ci | PASS |  |
| extension-build | PASS |  |
| extension-unit | PASS |  "result": "PASS",  "passed": 234,  "failed": [] |

Result: FAIL (1 checks not PASS)
