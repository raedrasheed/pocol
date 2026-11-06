# Application-local EVM cryptography compatibility patches

These three libraries bridge the accepted revm 19.7.0 engine to the corrected
k256 0.14 / elliptic-curve 0.14.3 / crypto-bigint 0.7.10 stack. Their upstream
release lines, and the latest upstream releases inspected on 2026-11-05, still
require k256 0.13. These local forks are maintained by this repository and are
not unmodified upstream releases or registry publications.

| Directory / upstream version | Local package version | License |
| --- | --- | --- |
| alloy-primitives-0.8.26 | 0.8.26+pocol.crypto.1 | MIT OR Apache-2.0 |
| alloy-eip7702-0.5.1 | 0.5.1+pocol.crypto.1 | MIT OR Apache-2.0 |
| revm-precompile-16.2.0 | 16.2.0+pocol.crypto.1 | MIT |

`MANIFEST.json` records the original registry archive checksums, original file
hashes, current file hashes, and exact local changes. The archives were verified
against the prior product lockfile before extraction. `PATCHES.diff` contains
the complete changes to upstream files. Added upstream license texts come from
each crate's pinned VCS commit; `NOTICE.txt` accompanies the native packages.
The original Cargo.toml.orig files are retained for provenance. The active
Cargo.toml files intentionally contain the local dependency/version changes.

The runtime adaptations are SEC1 point encoding and the changed normalize_s
return type. A test/arbitrary-only key sampler adapts rand 0.8 to the new
SigningKey API. EVM opcodes, gas constants, fork selection, unsigned primitive
representations and PoCol policy are unchanged. Public Rust APIs that expose
k256 types now expose the 0.14 types; this is an application-specific port.

Path overrides are explicit in the node, execution, and fuzz manifests. Supported
application configurations include the default C backend and the pure Rust
ECRECOVER fallback, serde/RLP, and the existing application test features.
Other upstream feature combinations require their own review; in particular,
enabling an optional older curve stack must fail the complete crypto graph check.

The node's compatibility identity includes the active manifests, READMEs and
Rust sources here. Keep release source-path checks enabled. Verify the source
snapshot and the resolved graph from the repository root:

```bash
python3 development/security-evm-crypto-2026-11-05/verify_vendor.py
cargo +1.99.1 metadata --locked --offline --format-version 1 \
  --manifest-path prototypes/pocold/Cargo.toml > /tmp/pocol-crypto-metadata.json
python3 development/security-evm-crypto-2026-11-05/verify_crypto_graph.py \
  --metadata /tmp/pocol-crypto-metadata.json
```

Maintain these patches until compatible upstream libraries adopt the corrected
stack. Any update must refresh the reviewed file manifest/diff, compare public
recovery and EVM results, run application regressions and native builds, and
retain the upstream licenses. See the [migration report](../../../development/security-evm-crypto-2026-11-05/REPORT-AR.md)
for evidence, remaining warnings, and the independently reproduced upstream unit-test limitations.
