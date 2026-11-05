# Dependency follow-up evidence

See [the Arabic report](REPORT-AR.md) and [reproduction commands](COMMANDS.md).

This follow-up updates only the compiler tools' tmp dependency and adds verification tools/documentation. It deliberately retains solc 0.8.28. It does not migrate EVM, change protocol source, or claim the complete audit is finished.

- The npm audit snapshots are raw registry responses: before exit 1, after exit 0. Before has two affected package entries (tmp plus its solc consumer), driven by two tmp advisories; those are different counts.
- Security regression logs are final before/after results from the same test source: 2 pass / 5 fail before, 7 pass / 0 fail after. All inputs are synthetic; file traversal remains inside test-owned directories. The solver process is a controlled fixture.
- The initial sandbox runner failure is retained separately. It is an environment failure, not part of the before/after vulnerability counts.
- The compiler comparison covers 12 existing scripts plus the continuous mode. Both phases are fresh locked npm installs with lifecycle scripts disabled. All 13 output hashes also match the prior accepted comparison in the earlier audit evidence.
- `resolved-tools.json` records the dependency resolved from solc's own module context, and proves soljson.js bytes are unchanged.
- `evm-assessment.json` records remaining dependency constraints and the separate migration work. No Rust source or lock changes are included; no new Rust/Android test run is claimed.
- `SHA256SUMS` covers the evidence files. Raw tool logs retain their original whitespace, which may be reported by a full Git whitespace check; source and documentation are checked strictly.

Registry metadata was read from https://registry.npmjs.org/ and https://crates.io/api/v1/crates/ on 2026-11-05. The official advisory links and titles are preserved in the raw npm responses:

- https://github.com/advisories/GHSA-52f5-9888-hmc6
- https://github.com/advisories/GHSA-ph9p-34f9-6g65

Attribution for advisory content belongs to its authors/contributors at those sources. These local results are a dated snapshot, not a guarantee about later advisory database changes.
