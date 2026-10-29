# LP4 experimental execution controls

Isolated Rust 1.99.1 / revm =19.7.0 / Cancun crate. Cargo.lock pins resolved
sources; default crypto precompiles remain enabled. Legacy LP1 dependencies are
untouched. No accepted PoCol apply, full PAY integration, GST gate, caps or boot path.

`Sandbox::execute` clones its input database and returns revm's actual execution
result/state diff; it never commits. Synthetic unfunded-network fixtures use no
keys or signatures. CALL-family access to PAY is rejected by an experimental outer
guard as a historical scaffold; use the separate native backend for approved PAY rules.

With the pinned toolchain on PATH and an isolated CARGO_HOME:

```sh
cargo test --locked --manifest-path prototypes/lp4-execution/Cargo.toml
cargo run --locked --manifest-path prototypes/lp4-execution/Cargo.toml --example control_vectors > work/control-vectors.json
python3 prototypes/lp4-execution/tests/control_oracle.py work/control-vectors.json
python3 prototypes/lp4-execution/tests/mutations.py work/mutations
```

Use `--offline` after fetching the exact lockfile. The Python oracle implements
only PUSH1, CHAINID, MSTORE, cold zero-to-nonzero SSTORE, RETURN and REVERT for
three fixed initial conditions; it is not an independent full EVM. Mutants are
compiled separately and must fail a selected assertion; compile errors do not count.
GeneralStateTests, PoCol deviations, independent full-EVM/state-root differential,
legacy regression and licenses/provenance audit remain acceptance obligations.

Native boundary work now lives in `pay`/`native`, separately from the original
`Sandbox` guard. Its authority is [LP4-PAY-01](../../development/LP4-PAY-APPROVAL.md)
and [LP4-PAY-02](../../development/LP4-PAY-LIFECYCLE-APPROVAL.md), including the linked complete approved contracts. `PayBoundaryHarness.sol` is explicitly test
code, not the completed Registry/Escrow implementation. Compiled-harness comparison
with independently extended py-evm checks 44 boundary executions; full reward,
closing, system-phase, caps and GenesisBuilder acceptance remains outstanding.

`cancun_control` runs upstream Ethereum control fixtures (chain ID 1, upstream
fields and transaction types), not PoCol apply. `gst_control.py` checks resulting
full-state and log hashes using Python's independent trie/RLP/Keccak against frozen
geth-generated fixture roots. It does not validate signed wire transaction bytes.
The owner has approved the additive creation rule in
[LP4-CREATE-01](../../development/LP4-COLLISION-APPROVAL.md). The same collision
handler now runs in `native::execute` and the upstream control runner; the old
optional probe has been removed. Complete InMemoryDB base storage is overlaid by
current journal values, including unloaded keys and cleared/restored slots. This
backend has no external storage and therefore no hidden slots; this does not claim
persisted-state integration, a resource bound or P2/T2 acceptance. Historical probe
receipts remain unchanged.

PAY-02 reads private immutable facts from a complete pre-transaction InMemoryDB
snapshot, never caller-supplied flags or the current creation journal. All PAY
calls, including zero, require original nonce>0 or nonempty code. Both compiled
same-transaction deletion attacks are independently re-executed by py-evm and
rejected. Historical PAY-01 positive receipts remain preserved. Snapshot cloning
and arbitrary backing databases are not proven bounded/persisted implementations;
R18/R21, actual system admission/custody/refunds and GenesisBuilder remain NOT PASSED.

Actual source-built Registry/Caps/bond-bridge work is in contracts/RegistryPayment.sol
with canonical encoding in contracts/REGISTRY-LAYOUT.md. LP4-REGISTRY-WRITE-APPROVAL.md
explicitly authorizes only10/2/10 (LP4-only), never passes the original9/1/9 or changes
other limits. User paths, ordinary sweep and closing bond cursor are implemented;
complete Escrow settlement, ShareLedger/GenesisBuilder, block apply/finalization and
fully admitted resource binding remain NOT PASSED. Independent py-evm execution and
an independently written domain model compare complete state roots and actual SSTORE
counts. Synthetic CP52 fixtures and corrupt-state negatives are not admitted profiles,
R18/R21 resource facts, caps64 acceptance or permission to boot/merge.

`native::execute_in_context` additionally executes explicit NUMBER, TIMESTAMP,
GASLIMIT, BASEFEE and PREVRANDAO, with fixed Escrow COINBASE and Cancun blob base
fee1. BLOCKHASH reads the immutable snapshot's ancestor cache with the EVM256-block
window. Nine independently re-executed context/full-state-root controls and seven
compiled field/cache mutation controls are recorded in R15. This low-level control
API does not validate headers, derive parent-seal randomness, prove complete stored
ancestry or admit signed transactions. The historical30M/zero-basefee wrappers are
synthetic fixtures. All remaining adaptations and replacement obligations are mapped
in development/lp4/p2-runtime/codex-r15/DEVIATIONS.md; no GST case is excluded.
