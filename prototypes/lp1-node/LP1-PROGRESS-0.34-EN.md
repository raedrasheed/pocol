# LP1 progress — Author revision 1 (handoff 0.34)

**Status:** code written, not compiled or tested. The author ran no commands. No completion claim until the root passes A0–A8.

**Scope:** an experimental local slice of the M3 foundation over accepted M1 formats (baseline 0.32). It is not an M1 amendment, complete M3, consensus or EVM. No keys, signing, transactions or external network. The V1NET profile is synthetic and non-bootable; implementation of the M1 contracts/publisher stage is not claimed.

| Item | Written | Awaiting the root |
|---|---|---|
| A0 | SHA-256 fixture check against PROVENANCE and check for secret material | Copy fixtures byte for byte and compare the baseline |
| A1 | Cargo 2021 with publish=false, pinned dependencies and rustfmt.toml | Generate lockfile, build and test offline on 1.58.1, run fmt |
| A2 | Strict RLP with nesting limit 16, V1 header encoding with reencoding, fixed-seed adversarial corpus | Run tests |
| A3 | SHA-256, Keccak, domain messages, secp256k1 recovery with low-S and v, and 3677 header-derived inputs | Run `hash` and tests |
| A4 | Heap-free ASERT using i128, u128 and [u64;8], 1027 rows, allocation-counter test (harness=false), and `asert-batch` command | Independent differential comparison by the root |
| A5 | RP window with netid/future/2–9, 0.27 counters, per-snapshot TemplateID cache, 22 cases and mutations | Verify 3355 and remaining counters |
| A6 | Fixed chain verified before serving, read-only router, F0–F4, empty chain and -32601 | Run tests |
| A7 | Bounded HTTP transport on 127.0.0.1 only, JSON readiness record, bounded shutdown and Python-standard-library E2E script | Run the real process |
| A8 | — | Codex review and corrections |

**Not claimed:** full M7 requirements (hyper, socket2, RpcGuard, pricing and resources), ParamGate, full genesis configuration or items 10–17.

**Questions recorded without silent changes:** seven, detailed in README, including exact decimal id semantics, the prototype data.reason convention, and nesting limit 16 in window responses.
