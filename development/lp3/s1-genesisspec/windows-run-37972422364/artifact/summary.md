# LP3 S1 Windows verification

Commit `9c02c7f7c7f977d2f0ec97107cbee5fad7a1e8f0`, 1.58.1-x86_64-pc-windows-msvc, Microsoft Windows 10.0.20348, image win22 20261004.326.1.

PASS 12, FAIL 4, BLOCKED BY PLATFORM 1, NOT RUN 0.

| Check | Status | Exit | Detail |
|---|---|---|---|
| toolchain | PASS |  | 1.58.1-x86_64-pc-windows-msvc; cargo 1.58.0 (f01b232bc 2022-01-19) |
| cargo-build | PASS | 0 | exit 0 |
| cargo-build-release | PASS | 0 | exit 0 |
| rustfmt-check | PASS | 0 | exit 0 |
| store-journal | PASS | 0 | running 12 tests<br>test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.91s<br>     Running tests\store_journal.rs (D:\a\_temp\lp3-target\debug\deps\store_journal-d6d8fd240b507529.exe) |
| store-process-crash-recovery | PASS | 0 | running 3 tests<br>test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.37s<br>     Running tests\store_process.rs (D:\a\_temp\lp3-target\debug\deps\store_process-b1cccd58013145d4.exe) |
| cargo-test-full | PASS | 0 | running 49 tests<br>test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s<br>running 0 tests<br>test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s<br>running 5 tests<br>test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s<br>{"test":"asert_alloc","calls":1069,"allocations":0,"counterProbe":true,"maxShiftBits":512,"earlyHigh":286,"none":3,"probeLen":64}<br>running 12 tests<br>test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.25s<br... |
| genesis-batch-alloc-debug | PASS | 0 | {"case":"oversizedLine256MiB","ok":true,"peakHeapBytes":8455441,"boundBytes":8520962,"perInputAboveBuffers":15,"rows":3}<br>{"case":"gsv1Times30000","ok":true,"peakHeapBytes":8455847,"boundBytes":8520962,"perInputAboveBuffers":421,"rows":30001}<br>{"case":"limitPlusOneThenMaxValid","ok":true,"peakHeapBytes":13895089,"boundBytes":14092374,"perInputAboveBuffers":5439663,"rows":3}<br>{"case":"maxDeepNest","ok":true,"peakHeapBytes":16844034,"boundBytes":16910848,"perInputAboveBuffers":8388608,"rows":2}<br>{"case":"maxFlatList","ok":true,"peakHeapBytes":8455458,"boundBytes":8520962,"perInputAboveBu... |
| genesis-batch-alloc-release-max | PASS | 0 | {"case":"oversizedLine256MiB","ok":true,"peakHeapBytes":8455441,"boundBytes":8520962,"perInputAboveBuffers":15,"rows":3}<br>{"case":"gsv1Times30000","ok":true,"peakHeapBytes":8455847,"boundBytes":8520962,"perInputAboveBuffers":421,"rows":30001}<br>{"case":"limitPlusOneThenMaxValid","ok":true,"peakHeapBytes":13895089,"boundBytes":14092374,"perInputAboveBuffers":5439663,"rows":3}<br>{"case":"maxDeepNest","ok":true,"peakHeapBytes":16844034,"boundBytes":16910848,"perInputAboveBuffers":8388608,"rows":2}<br>{"case":"maxFlatList","ok":true,"peakHeapBytes":8455458,"boundBytes":8520962,"perInputAboveBu... |
| verify-fixtures | PASS | 0 | {"check":"asert","ok":true,"rows":1027,"matched":1027,"declared":1027}<br>{"check":"hash","ok":true,"knownAnswersK1K3":true,"entries":3677,"digestMatches":3677,"groups":{"v1.blockHash":{"derived":15,"entries":15},"v1.powHash":{"derived":15,"entries":15},"v1.shareHash":{"derived":3587,"entries":3587},"v1.shareRoot":{"derived":15,"entries":15},"v1.sigMsg":{"derived":15,"entries":15},"v1.templateId":{"derived":15,"entries":15},"v1.winMsg":{"derived":15,"entries":15}},"failures":[]}<br>{"check":"summary","ok":true} |
| e2e-http | PASS | 0 | {"e2e": "lp1-http", "checks": 45, "failed": 0, "failures": []} |
| e2e-store | PASS | 0 | {"e2e": "lp2-store", "checks": 24, "failed": 0, "failures": []} |
| lp3-genesis-diff | FAIL | 1 | exit 1<br> |
| lp3-genesis-cli | FAIL | 1 | exit 1<br> |
| lp3-genesis-diff-release-max | FAIL | 1 | exit 1<br> |
| lp3-genesis-cli-release-max | FAIL | 1 | exit 1<br> |
| RLIMIT_AS address-space limit checks | BLOCKED BY PLATFORM |  | Windows has no RLIMIT_AS; bounded memory is covered here by genesis-batch-alloc (counting allocator) |
