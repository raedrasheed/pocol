# M1 Draft 0.2 — Unresolved Decisions, Experiments and Prerequisites

Nothing here is resolved by this document. Each entry states a **preferred proposal**, its rationale, its compatibility with the baseline, the tests that would falsify it, and **who or what must decide**. "Owner" means the baseline/project owner. "Reviewers" means Claude and Codex, whose agreement is not sufficient where the owner is named.

## 1. Decisions

| ID | Question | Preferred proposal | Rationale | Baseline compatibility | Falsifiable tests | Decider / dependency |
|---|---|---|---|---|---|---|
| U01 | Full 20-byte factory address (F23) | `0x0000000000000000000000000000000000c0c005` | Zero-fill of the elided `0x…C0C005` (FD:L777) | An inference from elided text | Fixtures regenerate deterministically under any value; the generator asserts the constant | Owner confirms the value |
| U02 | Explicit `@v<n>` view of a revoked version (F04) | Interstitial naming the revocation, an explicit click, then a persistent banner | FD:L914 limits revocation to *default* display | Compatible. Refusing would be stricter and needs a change request | Viewer test: `@v1` on a revoked v1 shows the interstitial with zero chunk fetches until confirmed | Owner (security policy) |
| U03 | Explicit `@v<n>` of a draft | Refuse, no frame | Drafts never passed `publish` | The baseline is silent, so this is an addition | Viewer test: `@v2` on a draft gives a refusal with zero fetches | Reviewers; owner if contested |
| U04 | Document-relative reference policy (F05) | §6.4 table; above-root gives 404; `//` is not followed | Keeps BR18c literal-path behaviour; makes T12 relative CSS work | Compatible with BR18c and T12 as far as I can tell; unverified in a browser | `path-cases.json` reference cases; E06 browser run on T12 sites | Reviewers, after E06 |
| U05 | Snapshot mechanism (F07) | Proof-bound reads against one `stateRoot` (§4.2) | Defeats A → B and A → B → A; needs no EIP-1898 | Uses only existing methods. Needs extension-internal `eth_getProof` (D95 bars sites, not the extension) and a D88 cost account | E03 mock scenarios: A → B, A → B → A, −32017, proof failure, 3 restarts then error with zero frames | Owner (heavy-method budget) + E03 |
| U06 | Retrieval ABI (F08) | Slot reads with P13 keys | FD:L4958 asks M1-spec for slots; `eth_call` is heavy | Compatible | E04 compiler layout equals `abi-and-slots.json`; T1-02 slot values | Reviewers, after E04 |
| U07 | On-chain manifest verification in `createVersion` (F09/O01) | Undecided. Preference **pending E02**: adopt it if the measured worst-case gas for 65536 bytes is ≤ 10% of the 8M `GAS_LIMIT` | Makes on-chain records self-consistent; the viewer still revalidates | Compatible (contract limits FD:L904) | E02 gas measurement; T1-14 extensions with `ManifestChunkInvalid` and `ManifestHashMismatch` | Reviewers + E02 |
| U08 | Canonical manifest split as a contract rule (F09) | Adopt: exactly ⌈len/24575⌉ chunks, 1–3 | Bounds writes; deterministic addresses | **New restriction**; the baseline does not require it | `ver-manifest-noncanonical-split`; T1-14 `ManifestSplit(0)` | Reviewers; owner if treated as a change request |
| U09 | Duplicate drafts and concurrency (F10) | The publisher reuses the lowest matching ID; the contract does not reject duplicates | Contract-level rejection adds a hash index (storage) and blocks legitimate re-drafts | Compatible | T1-11 K3; a concurrency test with two publishers creating the same manifest (both drafts exist; the tool picks the lowest) | Reviewers |
| U10 | Publisher rights of the previous owner after a transfer (F11) | Keep (P09): unchanged, with a tool warning | Draft 0.1 already chose this; changing it alters P09 | Compatible (the baseline is silent) | T1-12 | Owner (authority policy) |
| U11 | Transition edge policies (F12) | `transferOwnership(0)` cancels; self-nomination reverts `InvalidCandidate`; `setCurrent(current)` reverts `AlreadyCurrent`; no-op publisher calls emit nothing | Explicit failures make publisher scripts reconcile instead of silently succeeding | Compatible | T1-13, T1-15 | Reviewers |
| U12 | Definition of site size (F16) | Σ `size` over files (logical), excluding the manifest | Decidable before any fetch; bounds unique bytes too | Consistent with ChunkCache 4 MiB + 64 KiB (FD:L5319) | `pos-site-4194304`, `neg-site-4194305` | Reviewers |
| U13 | Omnibox parse order (F18) | Codex order (§6.2) | Removes the ambiguity of `?x=@v2` | Compatible with FD:L967 | 30 omnibox cases | Reviewers |
| U14 | Meaning of "timeout 10s" (FD:L905; F25) | Per-request timeout; no whole-load deadline | The baseline sentence does not fix a scope; a whole-load deadline would be a new rule | Per-request is the narrower reading | Mock: one slow `eth_getCode` (> 10 s) fails that request; a 40-chunk load at 1 s per request is not aborted for total time | Owner (baseline intent) |
| U15 | Staged implementation before the annexes are complete (F26) | **No carve-out by default**; the full gate applies | FD:L4955–5000 lists the annexes as part of M1-spec | The full gate is the baseline reading | Gate check: the annex checklist is 100% complete, or a signed decision record is present | Owner |
| U16 | Publisher confirmation depth (F24) | A parameter. No default value proposed | It depends on the reorg profile, which this draft does not have | — | A simulated reorg below the depth gives re-store before `createVersion` | Owner/reviewers, after network parameters |
| U17 | Duplicate chunk references in a manifest | Allowed | Required for shared content; the cache is keyed by (addr, len) | Compatible (FD:L1309) | `pos-shared-chunk`, `pos-site-4194304` | Reviewers |
| U18 | Factory events | None | Minimal; `store` returns the address; publishers verify code | Compatible with "no storage" | — | Reviewers |
| U19 | Exposure of the query/fragment to the page | Out of M1 scope | Lookup only is defined here | — | — | M2 extension annex |
| U20 | `entryIndex` width | u32 | Matches the other count fields; the real range is < 256 | The baseline is silent | `neg-entry-index-range` | Reviewers |
| U21 | Hardfork and tool pins | Cancun (B); exact tool versions recorded when selected | — | FD:L843 | E01 log | M0 versions task |
| U22 | ABI error, event and enumeration set (O03) | §5.2–5.3 | Separate authorization, existence and state errors; event-based enumeration | Compatible | T1-13; E04 ABI equals `abi-and-slots.json` | Reviewers |

O01 maps to U07/U08 plus the matrix in §3.1; O02 to U04/U13; O03 to U11/U22. All three remain **open**.

## 2. Experiments not yet run

| ID | Experiment | Produces | Blocks |
|---|---|---|---|
| E01 | T1 suite on anvil (Cancun), including the raised code-size limit (T1-06b) and declared injection (T1-06c) | Pass/fail per row, revert data, traces, tool versions | F02, F10, F17, F20, F21 |
| E02 | Gas of `createVersion` with and without on-chain verification, for a 79-byte and a 65536-byte manifest; gas of `store` at 1 and 24575 bytes (the baseline assumes about 5.4M per chunk) | Measured gas | U07 |
| E03 | `eth_getProof` on anvil and pocold: MPT format, verification against `stateRoot`, behaviour outside `K_eff`; mock RPC for A → B and A → B → A | Proof verifier and its mock results | U05, F07 |
| E04 | Compiler ABI and `storageLayout` compared with `abi-and-slots.json`; reproducible-build record (P27) | Diff report | U06, U22, F08, F22 |
| E05 | All fixtures re-checked with the three M0 Keccak libraries (K1–K3) | Cross-library hash agreement | Fixture freeze |
| E06 | Browser: Stage R resolution on the T12 sites (relative CSS, folders, SPA links) | Per-site results | U04 |
| E07 | Viewer: version-selection table (§4.3) with mocked Website state | Per-row results | U02, U03 |

## 3. Outstanding M0 prerequisites (FD:L4683)

M0 is listed before M1. Items relevant before M1 implementation:
- versions and licenses, including the anvil, solc and Foundry versions to pin;
- anvil availability;
- the three Keccak libraries with K1–K3.

None of these has been performed in this package. The 0.1 package and Codex's response say the same.

Other M0 items (the canary, the rpc-fault audit with its three faults, bridge-fault with its 32 faults, and checking `chrome.storage.local.getKeys`) are prerequisites for M2 rather than for M1 contracts. They are also outstanding.

## 4. Verified vs. assumed in this draft

- **Verified by execution here** (`results/run-results.json`): fixture outcomes against the reference model, the isolation claims, the draft 0.1 reproduction, determinism, and the boundary arithmetic.
- **Assumed, not verified:** U01; that pocold's trie is MPT and supports `eth_getProof` (U05); any EVM behaviour; gas figures; browser URL handling; the meaning of R9 `B_code_max ≥ 24577` (FD:L2318); the parts of the baseline not read in full (see `../M1-CLAUDE-REVIEW.md` §5).
