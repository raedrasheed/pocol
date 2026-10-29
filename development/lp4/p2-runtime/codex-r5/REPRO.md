# Reproduce native component controls

Pinned Rust 1.99.1, revm 19.7.0 and Cargo.lock; use the isolated CARGO_HOME described
in R4/HANDOFF.md rather than the preserved LP1 dependency vendor configuration.
Python dependencies: prototypes/lp4-execution/tests/python-requirements.lock.
Solidity 0.8.28+commit.7893614a, Cancun, optimizer 200, no CBOR/hash; unchanged
compiled harness inputs/outputs are source-bound JSON fixtures. CI reproduces both.

```sh
cargo +1.99.1 test --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml
cargo +1.99.1 run --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml --example pay_vectors > work/pay-vectors.json
python prototypes/lp4-execution/tests/pay_oracle.py work/pay-vectors.json prototypes/lp4-execution/tests/data/pay-harness.json work/pay-oracle.json
cargo +1.99.1 run --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml --example lifecycle_vectors > work/lifecycle-vectors.json
python prototypes/lp4-execution/tests/lifecycle_oracle.py work/lifecycle-vectors.json prototypes/lp4-execution/tests/data/reward-durability.json work/lifecycle-oracle.json
RUSTUP_TOOLCHAIN=1.99.1 python prototypes/lp4-execution/tests/pay_mutations.py work/pay02-mutations
```

All listed component commands exit zero in the recorded runs. Full GST was verified
on the preceding published head, not re-counted as a new-source local run. Native
reader initial construction failed (missing trie nodes) before persist fix; see review.
Source-only whitespace and Python compile checks pass. Whole-diff whitespace
check reports preserved raw Windows CRLF and Cargo log final blank lines; it does
not pass. Raw receipt bytes were not reformatted. Experimental Rust formatting is
NOT RUN (rustfmt unavailable in local pinned toolchain); no formatting PASS claimed.
