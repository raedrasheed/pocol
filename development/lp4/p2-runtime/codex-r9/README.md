# Hosted actual-runtime evidence and locked-source notices

Tested runtime source head: c70961187c23d3fd4bab6a99a43fd12001f7518e;
full tree:2b3ed68adc3b37899236d60b650eff5d6f0b9d56; run39063488297.
All three jobs completed successfully. Artifact ZIP digests were checked before safe
extraction; exact metadata and raw reports are retained. This evidence labels that
source head explicitly; a later audit/documentation head has its own CI requirements.

- Actual pinned native EVM tests:40 on Linux and40 on Windows, no ignored tests.
- Independent complete-state-root/bytecode/domain model comparisons:63; final hosted
  oracle bytes exactly equal R8's local evidence.17 compiled Registry mutation controls.
- Existing44 PAY+2 lifetime comparisons and upstream20661 Cancun controls succeeded.
  GST cases are byte-identical to preserved R4/gst-source-final-cases.jsonl.gz; the
  raw SHA256 equivalence receipt avoids rewriting/duplicating historical evidence.
  Exception presence is checked, not full reason/wire equivalence or PoCol T2 acceptance.
- Legacy Windows reports16PASS,8BLOCKED BY PLATFORM,2NOT RUN. The latter remain
  explicitly non-passes. They are separate from the new actual-native40 test job.

Locked-source work verifies all266 registry archive SHA256s and binds107 active
--target-all default-runtime/build package declarations and notice bytes to those
archives. Ten missing packaged top-level notices are resolved using exact VCS-pinned
upstream licenses/workspace CC0 declaration; all upstream Git blob IDs verified.
Runtime notice tar includes nested secp256k1 C/test notices, Unicode3.0 additional
terms, package metadata and standard CC0 reference. License choices follow declared
permissive branches; the complete archive preserves bundled notices beyond the
primary crate SPDX label. Inactive optional packages are not counted as linked.
The audit helper rejects mismatched extracted notice/manifest bytes against the
actual checksum-verified .crate, missing grants and unrecognized license expressions.

This prepares default-runtime notices/provenance, not full compiler/test-tool or
optional-feature/release distribution acceptance. Build/test tools are not bundled
runtime objects here. CI now repeats archive/notice verification with isolated
CARGO_HOME. Whole P2/license/execution acceptance remains incomplete; no release claim.

M1/prior evidence and all other limits unchanged. Only approved10/2/10 replace9/1/9
for LP4; old ceilings are not passed. All26 ledger obligations NOT PASSED; boot false.
General admitted CP/profile-bound R18/R21/caps64, GenesisBuilder, real apply/settlement/
finalization/SV5/SV6 and persistence/crash/reorg remain mandatory before any merge.
