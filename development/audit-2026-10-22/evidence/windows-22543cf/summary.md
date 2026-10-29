# Audit repair r2 Windows verification

- commit: `22543cf400289f0475d776bb6e18ed24ec8a0104` (requested `22543cf400289f0475d776bb6e18ed24ec8a0104`), dirty: False
- OS: Microsoft Windows NT 10.0.20348.0; PowerShell 7.6.8
- Rust: rustc 1.58.1 (db9d1b20b 2022-01-20) / rustc 1.99.1 (58cf8f9c3 2026-10-13); Node: v22.22.0

| check | status | detail |
|---|---|---|
| lp1-store-journal | PASS | Rust 1.58.1; 12 passed, 0 failed in 1 test binaries |
| lp1-node-all | PASS | Rust 1.58.1; 150 passed, 0 failed in 20 test binaries |
| lp4-execution | PASS | Rust 1.99.1; 211 passed, 0 failed in 21 test binaries |
| pocold | PASS | Rust 1.99.1; 250 passed, 0 failed in 18 test binaries |
| pocold-io-fault | PASS | Rust 1.99.1; 106 passed, 0 failed in 2 test binaries |
| extension-npm-ci | PASS |  |
| extension-build | PASS |  |
| extension-unit | PASS |  "result": "PASS",  "passed": 234,  "failed": [] |

Result: PASS
