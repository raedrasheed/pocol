# EVM cryptography follow-up, 2026-11-05

This application-specific port removes the affected older RustCrypto graph from
the node, execution library and security fuzz harness while keeping revm 19.7.0.
It follows the [private signing repair](../security-crypto-timing-2026-11-05/REPORT-AR.md)
and [compiler tool dependency repair](../security-dependency-followup-2026-11-05/REPORT-AR.md).

- [Arabic report, results and limits](REPORT-AR.md)
- [Reproduction and owner CodeQL commands](COMMANDS.md)
- [Reviewed local compatibility patches](../../prototypes/third-party/evm-crypto/README.md)
- [Historical review of the owner's 197 CodeQL alerts](CODEQL-197-REVIEW-AR.md)
- [Evidence index](evidence/provenance.json), [tested source hashes](evidence/tested-inputs.json)

The evidence records local verification before delivery. It does not certify a
deployed release, real-device Android behavior, Windows/browser CI, or a change
to the GitHub default CodeQL setup. Its upstream library unit-test comparison
retains all three reproducible baseline failures. See the report before reusing
the results. `SHA256SUMS` binds this directory's review tools and evidence;
`MANIFEST.json` in the patch directory separately binds the library sources.

The CodeQL helper defaults to read-only operation. The explicit `--apply` mode
uses the operator's existing GitHub CLI session and only the 197 pinned decisions;
it stops on changed source, locations, other open refs, permissions or main HEAD.
No alert was changed by the local verification recorded here.
