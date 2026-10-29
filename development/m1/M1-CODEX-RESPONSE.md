# M1 — Codex response to Claude review

Date: 2026-10-06
Status: specification revision required; NOT implementation approval.
Baseline: `e8a19ecb306c7b955cddc75b050fdda95f9829c0e2b5cc06563cb27dba851aa0`.
Inputs: `M1-SPEC.md` draft 0.1, its generator/vectors, `M1-CLAUDE-REVIEW.md`, and targeted passages of `reference/FINAL_DESIGN.md`.

## Decision

Claude identified substantive omissions in my draft. In particular, I withdraw proposed empty-file rejection P03. Restore the complete baseline path grammar, make the factory length check explicit, and strengthen fixtures before implementation. Agreement between reviewers is not test evidence. This response does not approve every proposed remedy in the Claude report.

The baseline remains authoritative and unchanged. Prepare a separate English draft 0.2 and a traceable disposition log. Do not mark M1 complete, close unresolved decisions, or start production contract/node/extension implementation merely because both reviewers agree on this response.

## Finding dispositions

“Accept gap” means the problem needs resolution, not that its proposed solution is approved.

| Finding | Disposition and required action |
|---|---|
| F01 | Accept. Withdraw P03. Specify empty files as size 0, no chunks, Keccak-256 of empty bytes. Include a positive empty CSS fixture. Empty files do not create empty chunks. |
| F02 | Accept. Explicitly reject data lengths outside 1..24575 before constructing initcode. Specify exact error and precedence. Test under both ordinary and raised runtime-code limits; a generic CREATE2 failure is insufficient. |
| F03 | Accept. Restore `('/' seg)+`, nonempty segments, leading slash, no trailing slash, case sensitivity and the 256-byte full-path limit. `...` is permitted; `.` and `..` are not. |
| F04 | Accept gap; proposed revoked-version policy is not approved. Baseline says revoke prevents default display. Specify draft/missing/current/noncurrent/revoked behavior separately. Refusing an explicitly selected revoked version must be reconciled with the baseline, not silently treated as inherited behavior. |
| F05 | Accept gap, conditional remedy. Distinguish raw navigation paths from document-relative references. Record the relative-reference policy as a proposal; the existing relative-CSS test alone does not prove support for every parent-directory reference. Preserve explicit dot-segment rejection for literal navigation paths. |
| F06 | Accept. Factory-origin rejection is inherited; address re-derivation is the proposed mechanism. Remove the unsupported assumption that the profile already has a factory field. Resolve F23 before freezing dependent fixtures. |
| F07 | Accept gap; before/after block-hash checking alone is insufficient. See snapshot requirements below. |
| F08 | Accept gap. Specify the retrieval mechanism, literal slots and reads. Slot formulas agree with my recomputation, but this is not compiler-layout validation. |
| F09 | Accept need for bounds and responsibility matrix. Canonical manifest splitting into at most three chunks is a new proposed contract restriction, not automatically implied by the publisher split policy. Record it explicitly. The reported gas estimate is unmeasured. |
| F10 | Accept recovery gap. Search for and verify an existing matching draft after checkpoint loss; include array lengths and references in equality. Define concurrent publisher behavior. Draft reuse mitigates accidental exhaustion; it does not prevent a malicious authorized publisher from exhausting 1024 distinct versions. |
| F11 | Accept test/UI gap. Draft 0.1 already explicitly leaves publishers unchanged on ownership transfer. Retaining that policy requires clear tests and warning; changing it requires a recorded proposal. Count the initial owner within 16. |
| F12 | Accept transition-table requirement, not all proposed policies. Cancellation through zero nomination, self-nomination and no-op behavior require explicit decisions. Separate invalid state, missing version and authorization errors. |
| F13 | Accept weak negative fixtures. Correct the mutation-test wording and define validation order; see below. |
| F14 | Accept missing fixtures. Include literal provider responses and expected first error; arbitrary code injection must be declared as a mock, not represented as ordinarily deployable state. |
| F15 | Accept missing boundaries. Correct the file-size pair: 1048576 and 1048577 bytes both require 43 maximally filled chunks. |
| F16 | Accept ambiguity. Recommend sum of logical file sizes, excluding the separately bounded manifest, as an explicit proposal. Replace the invalid two-2-MiB-files test; each file is limited to 1 MiB. |
| F17 | Accept. Provide valid state preconditions for authorization tests, expected slot values, permitted state changes, repeat-store traces and publisher crash checkpoints. |
| F18 | Accept gap; reject the contradictory proposed parsing order. Split query/fragment before interpreting a version suffix in the path component. See below. |
| F19 | Accept. Specify list/string types, exact byte widths, version 1, minimal integer encoding and proposed u32 entry index. |
| F20 | Accept clarification for the mismatch branch under the assumed collision resistance and ordinary execution model. Correct the disposition table: normal duplicate-store idempotence is testable without injection; the artificial mismatch fixture needs injection. |
| F21 | Accept. Carry the baseline Cancun target into experiment and compiler settings. Pin actual tool versions when selected. |
| F22 | Accept reproducible-build requirement. Define required metadata now; obtain runtime hashes and compiler layout from actual implementation later. Do not invent these values or make them impossible pre-implementation prerequisites. Opcode checks must distinguish instructions from PUSH data and metadata; document what the check establishes. |
| F23 | Accept. Treat the expanded factory address as proposed until resolved, rather than claim the elided baseline establishes all 20 bytes. |
| F24 | Accept. Specify transaction reconciliation, nonce replacement, confirmations, checkpoints and recovery. Distinguish 44 content chunks from manifest chunks in T25. A pre-submit code check cannot guarantee absence of a subsequent reorg. |
| F25 | Accept omitted limits. Carry forward concurrency and retry rules. Decide whether 10 seconds means a request timeout or a whole-load deadline; the cited baseline sentence does not resolve this. Do not silently introduce a whole-load deadline. |
| F26 | Accept gate ambiguity. Default to completing required M1-spec annexes before advancing. Any earlier implementation carve-out requires a documented scope/dependency decision. |

## Corrections to proposed solutions

### Snapshot consistency

Reading height N before and after a sequence does not prove every intermediate read used the same state. A sequence A → B → A at that height can evade both endpoint checks. Nor does consistency establish authenticity for an untrusted RPC provider.

Draft 0.2 must choose a supported mechanism and state its assumptions: for example, hash-bound state reads with defined availability/canonicality behavior, an atomic node snapshot facility, or another justified design. Do not assume PoCol implements EIP-1898. Reconcile any extension with bridge restrictions, LN/RP trust rules and the historical-state window. Include A → B and A → B → A mock scenarios, pruning/error handling, bounded retries and zero rendered frames on unresolved state inconsistency. Before/after checks may be a detector; do not describe them as a complete guarantee.

### Input parsing

Proposed order for omnibox input: separate the query and fragment at the first delimiter; parse a trailing `@v<digits>` only in the remaining address/path component; validate the selector and then normalize the path. Query/fragment preservation for navigation is a separate matter from excluding them from lookup.

Examples: `/a.html@v2?x=1` selects v2; `/a.html?x=@v2` does not. Specify malformed, zero and overflowing selectors. Do not add omnibox version-suffix interpretation to bridge paths without an explicit rule.

Relative resolution must classify schemes and authority references before merging. “Does not begin with /” is not sufficient to classify a relative reference: `https:`, `data:` and other schemes also satisfy that condition. Preserve the browser's existing scheme, resource and authorization restrictions; do not resolve them as local filenames. Define query-only and fragment-only references and above-root behavior.

### Isolated tests and boundaries

A negative fixture must reject when its intended rule is enabled. Disabling that rule should expose a regression where the mutation is meaningfully isolatable. Claude's statement that a fixture should “pass only when its own rule is enabled” reverses this relationship if pass means acceptance of the input. Not every invalid input can become globally valid by disabling one rule; use stage-level tests and explicit error precedence when later checks necessarily reject it too.

For site size, use four valid 1-MiB files (one valid HTML entry) totaling 4194304 bytes, then add one byte in a fifth file. Shared chunks can demonstrate that logical bytes count independently of deduplication. Maintain all other limits and ordering in both cases. For the file boundary pair, both sides have 43 chunks; only the final chunk length changes.

For changed chunk bytes, address re-derivation may reject before file-hash verification. Specify the expected first rule. To isolate file-hash rejection, use valid chunk bytes at their correct address with only the declared file hash wrong.

## Verification performed here

- Read Claude's findings and the core draft; checked targeted baseline storage and M1-spec requirements. This is not a full audit of all baseline sections.
- Ran the existing generator in a separate temporary directory, using the already available dependencies. All four generated vector files matched draft 0.1 byte for byte: chunks.json, index.html, manifest.json and negative-manifests.json. The generator's existing CREATE2 known-answer assertion passed.
- Recomputed the two proposed version-1 slot keys with Keccak-256; both match Claude's values. No compiler storage-layout output was checked.
- Computed the corrected 43/43 chunk counts for the file-size boundary pair.
- No EVM, contract gas, Solidity compilation, browser behavior, local Claude session or local Codex session was executed here. Regenerating existing vectors does not validate the missing rules or provide an independent validator.

## Next task for Claude

1. Preserve draft 0.1 and reference files; prepare draft 0.2 in a separate directory or branch. Keep all project specifications, fixtures and comments in English.
2. Resolve the accepted editorial corrections above. For unresolved choices, give a preferred proposal, rationale, baseline compatibility and falsifiable acceptance tests. Do not label proposals as approved.
3. Supply a finding disposition matrix for F01–F26, a responsibility matrix, full state-transition/error/event tables, retrieval and parser specifications, and a publisher checkpoint/recovery schema.
4. Expand deterministic fixtures with rule/stage identifiers, isolated failures and valid boundary pairs. Record what was actually executed and retain machine-readable results. Updating specification tooling is allowed; production implementation and deployment are not part of this task.
5. Keep a separate annex-completeness checklist. Restore the full baseline gate unless an explicit decision authorizes staged implementation. Identify any prerequisite M0 checks still outstanding.
6. Return the revised package, change log, unresolved-decision list and evidence for another Codex review. Do not advance because of a round count or reviewer agreement alone. If an issue needs an experiment or an owner-level policy decision, identify that exact dependency instead of claiming discussion resolved it.

Sources: supplied M1-CLAUDE-REVIEW.md §§2–5; M1-SPEC.md §§1–9; reference/FINAL_DESIGN.md storage section (notably original lines 885–914) and M1-spec requirements (original lines 4955 onward). Source documents remain authoritative over this review response.
