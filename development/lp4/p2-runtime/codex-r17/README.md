# R17 — completed scoped CI evidence and independently verified tool archives

Tested implementation commit: c6e56b069ad3d5a7c1323cae81ea53004f542512;
tree d643038c4a6210587c14c364a041f161c14980b7; run39394783398, PR24.
This documentation-only evidence branch is codex-lp4-p2-evidence-r17, based on that
exact implementation. It leaves the implementation branch/active CI head fixed.
Do not attribute implementation CI status to an untested evidence commit. No new PR,
merge, activation or owner decision is introduced.

Four jobs completed successfully; independent-cancun-comparison is still RUNNING,
therefore NOT PASSED. All four completed artifact ZIP digests match the GitHub API
before safe extraction. Raw ZIPs/reports are preserved locally; selected public
receipts, independent outputs and checksums are retained here.

- Linux43 and Windows43 actual Rust tests, none failed/ignored.
- Nine independently executed context/output/gas/complete-state-root comparisons;
  seven separately compiled context assertion mutations, with successful builds.
- 63 Registry bytecode/domain/full-state-root/gas/write comparisons exactly match
  R8's expanded63-case measurements; no general admitted resource bound is implied.
- Primary20661 Cancun receipts are byte-identical to the approved published R4
  reference, SHA2561bd455f96ded27e7f7f65253a3a3912f8044089ec60394cc036e994cb3c5a28b.
  Numeric category/predicate audit1172 plus1 adapter-only bigint input control.
- Existing compiled controls16PAY/7CREATE/17Registry/2rejection plus3 historical
  budget-contradiction witnesses remain separately scoped. The witnesses do not
  accept full Registry resources. External Geth2 compiled mutations also pass their
  required baseline/build/assertion checks.
- Independent native Geth executes ALL20661:19488 state/native-log assertions and
  1173 expected rejection-presence results; exit0, no limit,111.231seconds.
  Its success does not relabel Python timeouts, prove signed wire or pass PoCol T2.
- Preserved legacy report:16PASS,8BLOCKED BY PLATFORM,2NOT RUN. Those10 remain
  explicit non-passes despite the enclosing GitHub job's successful exit.

The full local1200-second attempt completed all20661 without dropping cases:
19487 state/log MATCH,1173 expected rejection presence,1BLOCKED_TIMEOUT. The exact
remaining identity is loopMul variant1, data index1. It is NOT PASSED. Variants0/2
actually matched in this attempt; the separate selected attempt timed out both0/1.
These timing-dependent observations do not recover the old39148982844 hosted three
identities, which were not retained. The full7200-second local/hosted executions
remain in progress; neither scaled timing nor native Geth is used to invent PASS.

Independent source/provenance work completed alongside the execution wait:

| Test/build tool source | Exact verified evidence | Explicit limit |
| --- | --- | --- |
|9 npm packages | All82 packaged regular files equal installed bytes; each official registry archive matches lockfile integrity | Compiler binary-to-source rebuild not proven; Solidity compiler GPL3 attribution remains separate from MIT wrapper |
|24 Python distributions | All1547 upstream wheel payload files equal installed bytes; each official PyPI wheel SHA256 matches public version metadata | Generated RECORD/pyc/installer files not counted; source rebuild/full distribution audit not claimed |
|3 Rust components | Official channel manifest+companion SHA256 verified; installed version/archive URL/hash records match; all147 regular payload files equal official pinned archives | Installer packaging/link entries not counted as regular payloads; no compiler source rebuild claim |

Source archives/wheels remain in local source caches with their original verified
bytes. Reproduction scripts/proofs record exact names, versions, source URLs, hashes,
file counts and excluded installer metadata. This extends R9/R14 default-runtime/
Geth/license provenance without claiming a complete release/distribution audit.

No executable, contract, Cargo/lock, M1/LP3, accepted decision, resource ceiling or
ledger edit. All26 obligations/P2/LP4 remain NOT PASSED; boot false. General R18/R21,
caps64, GenesisBuilder, real systems/apply/settlement/finalization/SV6 and persistence
remain mandatory. Continue from CONTINUATION.md without another approval request.
