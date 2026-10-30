# P2 checkpoint — genuine native-interface decision pending

Foundation: c0ef36c68fb83f9474de7e70fe61973b466647a8 (PR23 approved test definitions).
Branch: codex-lp4-p2-runtime. Draft only: do not merge incomplete P2.

Implemented an isolated pinned revm Cancun control sandbox, immutable database
snapshots, real SSTORE/REVERT/OOG/CHAINID results and experimental PAY access guard.
Final local evidence: 8 unit tests, 3 independently compared restricted executions,
3 compiled assertion-killed mutants after package-identity isolation. No LP4 gate
is passed by these tests. All 23 mandatory ledger obligations remain NOT PASSED;
boot disabled. General R18/R21, caps64 minimum, GenesisBuilder, real finalization
and SV6 are still mandatory, unverified. P1 acceptance remains only its primitive.

Next owner action: decide the complete proposed PAY native boundary in
PAY-DECISION.md, or supply a complete alternative. Its 5000 gas and never-fail
requirements cannot safely be translated into an invented authorization/ABI rule.
No repeated approval is needed for engineering work within the accepted plan.

After resolution: implement native handler plus independent full-EVM/state-root
harness and Cancun GeneralStateTests pinned corpus, enumerate every individual
PoCol exception/replacement, actual PAY boundary/rollback/negative/mutation tests,
complete dependency-license/source audit and legacy regression. Only then mark P2
ready, inspect actual GitHub requirements and merge normally into foundation.
Continue P3–P10 per LP4-MILESTONE.md, preserving all prior evidence and deferred gates.
No main merge, activation, deploy, billing, Claude or coordinator.

Reproduction: README in prototypes/lp4-execution; Rust 1.99.1, isolated CARGO_HOME
required because the legacy shared Cargo config points to LP1-only vendor files.
Successful final commands used --offline --locked. Original and mutation crate
names must remain distinct. Do not upload caches, build binaries or credentials.
