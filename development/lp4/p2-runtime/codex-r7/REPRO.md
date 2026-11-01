# Reproduce the write-bound conflict

Use the isolated CARGO_HOME/RUSTUP_HOME and pinned Rust1.99.1 setup in R4/HANDOFF.
Locked revm19.7.0, solc0.8.28+commit.7893614a, Python py-evm lock are unchanged.
Compiler settings: Cancun, optimizer200, no CBOR/hash; JSON captures source/settings,
ABI, deployed/creation bytecode and storage layout. Witness is explicitly NOT the
complete Registry and cannot pass full execution acceptance.

```sh
NODE_PATH=work/pinned-solc/node_modules node prototypes/lp4-execution/tests/compile_budget_witness.cjs work/recompiled.json
cmp prototypes/lp4-execution/tests/data/registry-write-budget.json work/recompiled.json
cargo +1.99.1 test --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml
cargo +1.99.1 run --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml --example registry_write_budget > work/write-vectors.json
work/pinned-pyevm/bin/python prototypes/lp4-execution/tests/registry_budget_oracle.py work/write-vectors.json prototypes/lp4-execution/tests/data/registry-write-budget.json work/counts.json
NODE_PATH=work/pinned-solc/node_modules RUSTUP_TOOLCHAIN=1.99.1 python prototypes/lp4-execution/tests/registry_budget_mutations.py work/budget-mutations
```

The comparison command succeeds because it detects and reproduces the conflicts;
it does NOT pass the ceiling gate. CI explicitly labels this scope. Source whitespace
and Python compile are checked separately from byte-preserved raw logs/Windows CRLF.
Experimental local rustfmt is NOT RUN (not installed in pinned toolchain).
