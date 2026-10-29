# LP3 S1 Windows verification

Commit `4e26e4d3049731645ab1d85ccfb9972d29d63390`, 1.58.1-x86_64-pc-windows-msvc, Microsoft Windows 10.0.20348, image win22 20261004.326.1.

PASS 20, FAIL 0, BLOCKED BY PLATFORM 16, NOT RUN 0.

| Check | Status | Exit | Detail |
|---|---|---|---|
| toolchain | PASS |  | 1.58.1-x86_64-pc-windows-msvc; cargo 1.58.0 (f01b232bc 2022-01-19) |
| cargo-build | PASS | 0 | exit 0 |
| cargo-build-release | PASS | 0 | exit 0 |
| rustfmt-check | PASS | 0 | exit 0 |
| store-journal | PASS | 0 | running 12 tests<br>test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.36s<br>     Running tests\store_journal.rs (D:\a\_temp\lp3-target\debug\deps\store_journal-d6d8fd240b507529.exe) |
| store-process-crash-recovery | PASS | 0 | running 3 tests<br>test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.62s<br>     Running tests\store_process.rs (D:\a\_temp\lp3-target\debug\deps\store_process-b1cccd58013145d4.exe) |
| cargo-test-full | PASS | 0 | running 49 tests<br>test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s<br>running 0 tests<br>test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>running 5 tests<br>test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.14s<br>{"test":"asert_alloc","calls":1069,"allocations":0,"counterProbe":true,"maxShiftBits":512,"earlyHigh":286,"none":3,"probeLen":64}<br>running 12 tests<br>test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.15s<br... |
| genesis-batch-alloc-debug | PASS | 0 | {"case":"oversizedLine256MiB","ok":true,"peakHeapBytes":8455441,"boundBytes":8520962,"perInputAboveBuffers":15,"rows":3}<br>{"case":"gsv1Times30000","ok":true,"peakHeapBytes":8455847,"boundBytes":8520962,"perInputAboveBuffers":421,"rows":30001}<br>{"case":"limitPlusOneThenMaxValid","ok":true,"peakHeapBytes":13895089,"boundBytes":14092374,"perInputAboveBuffers":5439663,"rows":3}<br>{"case":"maxDeepNest","ok":true,"peakHeapBytes":16844034,"boundBytes":16910848,"perInputAboveBuffers":8388608,"rows":2}<br>{"case":"maxFlatList","ok":true,"peakHeapBytes":8455458,"boundBytes":8520962,"perInputAboveBu... |
| genesis-batch-alloc-release-max | PASS | 0 | {"case":"oversizedLine256MiB","ok":true,"peakHeapBytes":8455441,"boundBytes":8520962,"perInputAboveBuffers":15,"rows":3}<br>{"case":"gsv1Times30000","ok":true,"peakHeapBytes":8455847,"boundBytes":8520962,"perInputAboveBuffers":421,"rows":30001}<br>{"case":"limitPlusOneThenMaxValid","ok":true,"peakHeapBytes":13895089,"boundBytes":14092374,"perInputAboveBuffers":5439663,"rows":3}<br>{"case":"maxDeepNest","ok":true,"peakHeapBytes":16844034,"boundBytes":16910848,"perInputAboveBuffers":8388608,"rows":2}<br>{"case":"maxFlatList","ok":true,"peakHeapBytes":8455458,"boundBytes":8520962,"perInputAboveBu... |
| verify-fixtures | PASS | 0 | {"check":"asert","ok":true,"rows":1027,"matched":1027,"declared":1027}<br>{"check":"hash","ok":true,"knownAnswersK1K3":true,"entries":3677,"digestMatches":3677,"groups":{"v1.blockHash":{"derived":15,"entries":15},"v1.powHash":{"derived":15,"entries":15},"v1.shareHash":{"derived":3587,"entries":3587},"v1.shareRoot":{"derived":15,"entries":15},"v1.sigMsg":{"derived":15,"entries":15},"v1.templateId":{"derived":15,"entries":15},"v1.winMsg":{"derived":15,"entries":15}},"failures":[]}<br>{"check":"summary","ok":true} |
| e2e-http | PASS | 0 | {"e2e": "lp1-http", "checks": 45, "failed": 0, "failures": []} |
| e2e-store | PASS | 0 | {"e2e": "lp2-store", "checks": 24, "failed": 0, "failures": []} |
| lp3-genesis-diff | PASS | 0 | {"check": "lp3-genesis-diff", "ok": true, "inputs": 30075, "agree": 30075, "mismatches": 0, "firstMismatches": [], "rustExit": 0, "rustSummary": {"check": "genesis-decode", "inputs": 30075, "accepted": 2749, "rejected": 27326, "refused": 0, "reencodeEqual": true, "maxSpecBytes": 2818474}, "referenceCodes": {"L0": 8516, "gsCount": 5621, "gsInt": 1535, "gsLen": 1566, "gsOrder": 635, "gsRange": 948, "gsStructure": 7741, "gsSys": 324, "gsVersion": 440, "ok": 2749}, "byGroup": {"bytes": {"L0": 8500, "gsCount": 177, "gsInt": 52, "gsOrder": 4, "gsRange": 29, "gsStructure": 107, "gsVersion": 1, "ok": ... |
| lp3-genesis-cli | PASS | 0 | { |
| lp3-genesis-cli: largeOneLine192MiB.limit256MiB | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli: largeOneLine192MiB.limit64MiB | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli: limitPlusOneThenMaxValid | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli: limitPlusOneThenMaxValid.noLimit | PASS |  | exit 1, 1.22 s |
| lp3-genesis-cli: maxDeepAndFlat | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli: gsv1Times100000 | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli: gsVersion262144Bytes | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli: lineForms | PASS |  | exit 1, 0 s |
| lp3-genesis-cli: reservationFailure.limit8MiB | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-diff-release-max | PASS | 0 | {"check": "lp3-genesis-diff", "ok": true, "inputs": 30077, "agree": 30077, "mismatches": 0, "firstMismatches": [], "rustExit": 0, "rustSummary": {"check": "genesis-decode", "inputs": 30077, "accepted": 2749, "rejected": 27328, "refused": 0, "reencodeEqual": true, "maxSpecBytes": 2818474}, "referenceCodes": {"L0": 8516, "gsCount": 5621, "gsInt": 1535, "gsLen": 1566, "gsOrder": 635, "gsRange": 948, "gsStructure": 7741, "gsSys": 324, "gsVersion": 442, "ok": 2749}, "byGroup": {"bytes": {"L0": 8500, "gsCount": 177, "gsInt": 52, "gsOrder": 4, "gsRange": 29, "gsStructure": 107, "gsVersion": 1, "ok": ... |
| lp3-genesis-cli-release-max | PASS | 0 | { |
| lp3-genesis-cli-release-max: largeOneLine192MiB.limit256MiB | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli-release-max: largeOneLine192MiB.limit64MiB | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli-release-max: limitPlusOneThenMaxValid | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli-release-max: limitPlusOneThenMaxValid.noLimit | PASS |  | exit 1, 0.05 s |
| lp3-genesis-cli-release-max: maxDeepAndFlat | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli-release-max: gsv1Times100000 | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli-release-max: gsVersion262144Bytes | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli-release-max: gsVersion2818466Bytes | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| lp3-genesis-cli-release-max: lineForms | PASS |  | exit 1, 0.02 s |
| lp3-genesis-cli-release-max: reservationFailure.limit8MiB | BLOCKED BY PLATFORM |  | RLIMIT_AS / wait4 not available on Windows |
| RLIMIT_AS address-space limit checks | BLOCKED BY PLATFORM |  | Windows has no RLIMIT_AS; bounded memory is covered here by genesis-batch-alloc (counting allocator) |
