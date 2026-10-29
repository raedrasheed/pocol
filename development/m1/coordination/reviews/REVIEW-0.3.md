# Codex independent review of 0.3 / author turn 001

Receipt: claude-001-retry.json; Claude session 83a06b92-c5ea-45e3-852e-b5b5f28a3eea; exit 0, is_error false, no permission denials. Review decision: further revision required; M1 not accepted.

Executed in preserved review copy: run_checks_03.py, 335 entries (333 pass, 2 recorded, 0 fail), 34.6 seconds. Rerunning the already reproduced 0.2 suite again is unnecessary. Design-ID brute-force probe is unnecessary for acceptance: FD:L3 embeds the design ID and baseline.json records approval of it; no raw byte equality was promised.

Independent own probes in independent-probes-0.3.json confirm C03, C04 and C05 corrected in the specification models: deep RLP => struct.shape with zero requests; 5000-digit selector => malformedSelector; shared chunk => one actual request. Review accepts these remedies for specification tooling, not production certification.

Independent pycryptodome 3.24.0 recomputation: 17/17 checks pass (K1-K3 known answers, all 13 code-table CREATE2 addresses, official CREATE2 example 0). netKey = 0xacfd846cb4e99df47de0b04af39136202c3d466fdcdd2b1ad7a6a8586b01d044. Its preimage is 51 bytes. My first probe hand-counted 50; corrected that assertion, preserving the failed first-run JSON. Full M0 triad remains unexecuted; Rust toolchain absent and @noble/hashes not checked here.

New required revisions:

- C10: explicitly selected version status 3 renders one frame in snapshot_model.load_p20. Version-selection prose only authorizes published or explicitly confirmed revoked content. Undefined status must be stateInvariant with zero frames; include corrupt slot-2/version count/current consistency and absence cases with clear precedence.
- C11: gate table says EVM/compiler evidence is required before implementing the unit even though that evidence needs candidate implementation. Distinguish prerequisites to start drafting implementation from checks before accepting production implementation. Do not permit implementation in this task.
- C12: 426 content requests is only a canonical publisher split bound, not a viewer bound. Own accepted 23131-byte manifest with 1000 distinct two-byte chunks plus one HTML chunk causes 1001 actual requests for 2013 logical bytes. Derive a conservative valid-manifest bound from RLP size, or label publisher-only assumption; do not silently restrict valid file chunk splitting.
- C13: missing-ID count is 17, not 22 (5 Q + 7 X + 5 d). Q8-Q12 and X1-X7 appear only as references in baseline. Existing BR23 d1-d5 definitions are possible namesakes; mapping to Arabic extension د1-د5 is unestablished, not proved absent. Correct inventory and prepare concrete proposed addendum if needed.

BLK-01 independent confirmation: FD:L1274 forbids sending unlisted methods; table L1262-L1273 omits eth_getProof. D95's site ban is separate. CR-M1-01 needs numeric request/response schemas, proof-size derivation and resource accounting before owner approval. Keeping Website reads within ChunkFetcher does not expand the six-importer list and is a routine proposed placement, not independently an owner-only question.

Unnecessary owner questions rejected: batch ordering and workspace portable interpreter are technical choices within this task. Do not repair host Python launcher. C08 design-ID derivation is a provenance limitation, not a completion blocker absent evidence of inconsistent approval.

Preservation verified: 51 protected files, 0 changed (protected-hashes-after-001.json). Remaining gate: 6 partial, 1 reported blocked, 34 unstarted, 0 complete. Continue feasible annex specifications; final acceptance remains blocked by baseline conflicts/undefined tests and owner policy decisions.
