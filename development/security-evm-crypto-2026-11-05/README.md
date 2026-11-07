# EVM cryptography follow-up, 2026-11-05

Latest delivery: [PR29 merged after all 27 checks succeeded and the owner
completed the 11 reviewed dismissals](DELIVERY-2026-11-06-AR.md).

2026-11-07 update: the owner completed the historical batch (190 confirmed
dismissals and seven already closed). A subsequent full main inventory found
[19 additional findings outside that plan, now independently source-reviewed](../security-codeql-remaining-2026-11-07/README-AR.md).

This application-specific port removes the affected older RustCrypto graph from
the node, execution library and security fuzz harness while keeping revm 19.7.0.
It follows the [private signing repair](../security-crypto-timing-2026-11-05/REPORT-AR.md)
and [compiler tool dependency repair](../security-dependency-followup-2026-11-05/REPORT-AR.md).

- [Arabic report, results and limits](REPORT-AR.md)
- [Reproduction and owner CodeQL commands](COMMANDS.md)
- [Reviewed local compatibility patches](../../prototypes/third-party/evm-crypto/README.md)
- [Historical review of the owner's 197 CodeQL alerts](CODEQL-197-REVIEW-AR.md)
- [Current owner evidence and full-batch main application guard](CODEQL-MAIN-APPLY-AR.md)
- [Review and guarded owner helper for the 11 new PR29 findings](CODEQL-PR29-REVIEW-AR.md)
- [Final source CI: 26 successful jobs; CodeQL alert gate remains blocked](CI-PR29-AR.md)
- [PR29 helper ref correction, 2026-11-06](CODEQL-PR29-REF-FIX-AR.md)
- [PR29 reviewed source versions and batch preflight](CODEQL-PR29-CROSS-REF-AR.md)
- [PR29 nullable global state and instance confirmation](CODEQL-PR29-NULL-STATE-AR.md)
- [Evidence index](evidence/provenance.json), [tested source hashes](evidence/tested-inputs.json)

The evidence records local verification before delivery. It does not certify a
deployed release, real-device Android behavior, Windows/browser CI, or a change
to the GitHub default CodeQL setup. Its upstream library unit-test comparison
retains all three reproducible baseline failures. See the report before reusing
the results. `SHA256SUMS` binds this directory's review tools and evidence;
`MANIFEST.json` in the patch directory separately binds the library sources.

The CodeQL helper defaults to read-only operation. The explicit `--apply` mode
uses the operator's existing GitHub CLI session and only the 197 pinned decisions;
it preflights the whole batch before its first write and checks current main
source as well as every open ref. Changed metadata, unknown instance states,
locations, source, permissions or main HEAD stop the run. Live metadata and
instances are read again before each PATCH. An unconfirmed write is recorded
explicitly and stops subsequent writes.
No alert was changed by the local verification recorded here.

Delivery update: [PR29](https://github.com/palcertcom/pocol/pull/29) merged at
`03190bed86aacc8e9fdaa895c5be349031bcceb5`. Windows/Linux/Android and browser CI,
all eight default CodeQL analyses and the aggregate alert gate succeeded on the
exact tested source. The owner completed all 11 reviewed dismissals; the receipt
and independent gate verification are linked in the delivery report. The local
snapshot and earlier stopped-run evidence retain their historical meanings.
