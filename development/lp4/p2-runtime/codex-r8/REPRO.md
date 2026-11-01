# Reproduce source-built Registry/payment component

Pinned Rust1.99.1/revm19.7.0 and Cargo.lock; isolated CARGO_HOME, RUSTUP_HOME and
pinned compiler/py-evm setup from R4/HANDOFF.md. No LP1 dependency/toolchain edits.
solc0.8.28+commit.7893614a, Cancun, optimizer200, noCBOR/hash. Node package lock and
Python requirements lock unchanged. Use rustup's cargo proxy for +1.99.1.

```sh
NODE_PATH=work/pinned-solc/node_modules node prototypes/lp4-execution/tests/compile_registry.cjs work/registry-payment.json
cmp prototypes/lp4-execution/tests/data/registry-payment.json work/registry-payment.json
cargo +1.99.1 test --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml
cargo +1.99.1 run --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml --example registry_vectors > work/registry-vectors.json
work/pinned-pyevm/bin/python prototypes/lp4-execution/tests/registry_execution_oracle.py work/registry-vectors.json work/registry-oracle.json
NODE_PATH=work/pinned-solc/node_modules RUSTUP_TOOLCHAIN=1.99.1 python prototypes/lp4-execution/tests/registry_mutations.py work/registry-mutations
```

The last command freezes and hashes its baseline before cloning mutants. Each must
compile Solidity and Rust and fail the selected meaningful assertion. No build error
counts. Initial classification failures and source compilation failure remain in logs.

Decompress evidence with gzip; compression.json records exact raw SHA256/length.
Root comparisons use decimal balances explicitly, canonical RLP/Keccak and a separate
py-evm state trie; the domain model uses object/list transitions and independently
encodes the documented layout. Every fixture is synthetic/unadmitted, including
explicit corrupt-state negatives. They are not a complete CP/profile resource PASS.

Evidence logs use actual /workspace toolchain paths; adapt those to the isolated setup.
Source-only whitespace/Python compile checks pass. Whole raw evidence whitespace is
not asserted clean; Cargo terminal blank lines are byte-preserved. Experimental local
rustfmt is NOT RUN (not installed); old evidence is not reformatted or relabelled.
