# Acceptance of the local LP1 prototype

The third revision passed all acceptance criteria A0–A8 for the documented experimental scope. The offline build, 54 Rust tests, formatting check and 375 independent RLP-depth checks passed. All 1027 ASERT results, 22 window cases and 3677 hashes matched. The memory-allocation test recorded 1069 calls without any allocation. The real service passed 45 HTTP tests and 35 independent connection tests.

Claude corrected RLP depth bounds and the classification of two unchanged mutation inputs; Codex reran the full tests and independently compared the evidence. M1 0.32 files and 139 original files remained unchanged, and earlier revisions and evidence were preserved.

Acceptance applies to a local Rust prototype reading a fixed fixture chain, not a production node or completion of M3/M7. No EVM, peer network, transactions, wallet, deployment or merge. There are no pending owner decisions within LP1; reversible engineering choices are documented as delegated decisions, not personal approvals from you. Draft link: https://github.com/raedrasheed/Proof-of-Collaboration/pull/5
