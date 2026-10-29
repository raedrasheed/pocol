# Reproduce without excluding a Cancun case

Use the isolated Rust1.99.1 toolchain/CARGO_HOME and Python3.12 comparison venv
documented in R14. Verify/extract the pinned2807-file legacy corpus with gst_extract.py
and R2 source-receipts.json. Base python-requirements.lock plus the separate hashed
python-comparison-extra.lock remain unchanged.

```sh
python prototypes/lp4-execution/tests/gst_independent.py CORPUS OUTPUT --profile lp4-create01-modexp-admission --timeout 7200 --workers 2
python -c "import json; s=json.load(open('OUTPUT/summary.json')); c=s['counts']; assert c['total']==20661 and not s['limited'] and s['selectedCases'] is None; assert all(c.get(k,0)==0 for k in ['MISMATCH','ENGINE_ERROR','BLOCKED_TIMEOUT']), s"
cargo +1.99.1 test --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml
cargo +1.99.1 run --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml --example context_vectors > work/context-vectors.json
python prototypes/lp4-execution/tests/context_oracle.py work/context-vectors.json work/context-oracle.json
RUSTUP_TOOLCHAIN=1.99.1 python prototypes/lp4-execution/tests/context_mutations.py work/context-mutations
python prototypes/lp4-execution/tests/tool_inventory.py NODE_MODULES RUST_TOOLCHAIN work/tool-inventory.json
python development/lp4/p2-runtime/codex-r15/synthetic-loop-timing.py CORPUS work/synthetic-timing.json
```

Python means the pinned comparison venv interpreter. OUTPUT and work must be real
task-specific directories. Timing diagnostics change an in-memory copy only and do
not count toward20661. Native context roots use an unmodified second engine with
its own ordered ancestor list, not the native cache or precomputed success flags.
Run Registry vectors/oracle as in R14; compare all63 cases to R8 oracle-expanded.json.gz
or the hosted96ab artifact's registry-oracle.json. Do not compare to the older37-case
oracle-with-model.json.gz and mistakenly infer a runtime regression.

Timeout negative: select the two prior loopMul metadata receipts and execute the same
collector with --timeout1 --workers1 --selected-receipts FILE. Require two exact
incomplete identities, BLOCKED_TIMEOUT2/total2 and the strict acceptance assertion to
reject them. Never use these selected/limited diagnostics as the full corpus result.

GitHub job/artifact IDs, source head/tree, digests and failure steps are in hosted-run.json.
Verify ZIP hashes before extracting; reject absolute/traversal member paths. Preserve
failed/partial attempts. No fixture secretKey is emitted, passed to Python workers or
uploaded; the pinned public corpus itself stays outside the repository.
