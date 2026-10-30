# Audit repair r2 Windows verification

- commit: `49508a5dacfe65f1cc7326cfbe82e9fd57b953f0` (requested `49508a5dacfe65f1cc7326cfbe82e9fd57b953f0`), dirty: False
- OS: Microsoft Windows NT 10.0.20348.0; PowerShell 7.6.7
- Rust: rustc 1.58.1 (db9d1b20b 2022-01-20) / rustc 1.99.1 (58cf8f9c3 2026-10-13); Node: v22.22.0

| check | status | detail |
|---|---|---|
| lp1-store-journal | PASS | Rust 1.58.1; 12 passed, 0 failed in 1 test binaries |
| lp1-node-all | PASS | Rust 1.58.1; 150 passed, 0 failed in 20 test binaries |
| lp4-execution | PASS | Rust 1.99.1; 168 passed, 0 failed in 21 test binaries |
| pocold | FAIL | exit 101; Rust 1.99.1; 149 passed, 3 failed in 14 test binaries |
| extension-npm-ci | PASS |  |
| extension-build | PASS |  |
| extension-unit | PASS |  "result": "PASS",  "passed": 234,  "failed": [] |

Result: FAIL (1 checks not PASS)
