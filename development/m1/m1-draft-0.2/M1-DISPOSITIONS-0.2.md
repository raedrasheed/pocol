# M1 Draft 0.2 — Finding Disposition Matrix

Sources: `../M1-CLAUDE-REVIEW.md` (F01–F26), `../M1-CODEX-RESPONSE.md` (dispositions and corrections), `M1-SPEC-0.2.md`.

**No finding is closed.** Closure requires Codex review of draft 0.2 and, where noted, experiment results. Draft text and fixtures are evidence that a remedy has been *proposed and made testable*, not that it is correct or approved.

Status vocabulary:
- **Drafted + fixtures:** spec text added, plus executable fixtures that pass the reference model (`results/run-results.json`). Awaiting review.
- **Drafted:** spec text only. Awaiting review.
- **Open: U##:** blocked on an unresolved decision (`M1-UNRESOLVED-0.2.md`). The draft gives a preferred option only.
- **Open: E##:** blocked on an experiment that has not been run.

| ID | Sev | Codex disposition | Draft 0.2 action (location) | Codex corrections adopted | Evidence | Status |
|---|---|---|---|---|---|---|
| F01 | High | Accept; withdraw P03 | P03 withdrawn; B07a states the empty-file form as a consequence of B07 (§2) | Empty files create no empty chunk | `pos-empty-file`, `neg-empty-with-chunk`, `neg-nonempty-no-chunks` | Drafted + fixtures |
| F02 | High | Accept; exact error and precedence; test both limits | P14: `ChunkLength` first, then mismatch, then create (§1); T1-06a/b | A generic CREATE2 failure is not enough | `abi-and-slots.json` revert data; T1-06 rows are specified but not run | Drafted. Open: E01 (EVM) |
| F03 | High | Accept | B05 restored verbatim from FD:L895 (§2); P08 duplicate removed | `...` allowed | 14 `neg-path-*` fixtures, `pos-path-256`, `pos-path-special-segments` | Drafted + fixtures |
| F04 | High | Accept the gap; the revoked policy is not approved | Version-selection table (§4.3); draft = refuse (P), revoked = U02 with a preferred interstitial | Revoked refusal is not presented as inherited | Selector cases in `path-cases.json`; the viewer table has no executable fixture yet | Open: U02, U03 |
| F05 | High | Accept the gap; conditional remedy | Stage R classification (§6.4): scheme, network-path, absolute, query-only, fragment-only, relative, above-root | Classify schemes and authority references before merging; literal dot segments still give 404 | 17 `reference` cases in `path-cases.json` (model only) | Open: U04, E06 (browser) |
| F06 | Medium | Accept | Requirement relabelled B (FD:L907); P06 is the mechanism; no profile field (§3.2) | The profile has no factory field | `neg-fetch-other-factory`, `codeTable.allEntriesFactoryDerived` | Drafted + fixtures; depends on U01 |
| F07 | Medium | Accept the gap; endpoint checks insufficient | Proof-bound reads against one `stateRoot` (§4.2); endpoint checks called a detector only; bounded restarts; zero frames | A → B → A; no EIP-1898 assumption; LN/RP and `K_eff` reconciled | None. The A → B and A → B → A mock scenarios are specified in E03 but not built | Open: U05, E03 |
| F08 | Medium | Accept the gap | Slot reads (§4.1); literal slot keys; T1-02 expected state | The slot formulas are not compiler validation | `abi-and-slots.json`; slot keys for version 1 match the Codex recomputation | Open: U06, E04 |
| F09 | Medium | Accept bounds and matrix; canonical split is a new restriction | Responsibility matrix (§3.1); canonical split recorded as P22/U08; on-chain verification as U07 | Gas unmeasured; the restriction is stated explicitly | `ver-manifest-noncanonical-split`, `ver-manifest-65536`, `ver-manifest-length-0`; T1-14 | Open: U07, U08, E02 |
| F10 | Medium | Accept the recovery gap | Find-or-create on full equality including arrays (§7.1); concurrency stated; exhaustion by a malicious publisher stated | Reuse mitigates accidents only | T1-11 crash points K1–K4 (specified, not run) | Open: U09; E01 |
| F11 | Medium | Accept the test/UI gap; count the owner within 16 | P09 kept (§5.1); T1-07 rewritten; T1-12 added; publisher warning | Changing the policy needs a recorded proposal | T1-07/T1-12 specified, not run | Open: U10 |
| F12 | Medium | Accept the table; policies need decisions | Full transition and error table (§5.2), with separate authorization, existence and state errors | Zero nomination, self-nomination and no-op behaviour made explicit decisions | Selectors in `abi-and-slots.json`; T1-13/T1-15 specified | Open: U11, U22 |
| F13 | Medium | Accept; fix mutation wording; define order | Stage order and rule IDs (§3.2); isolation classes (§3.4) | Disabling the rule exposes the regression (the earlier wording was reversed) | 62 negatives; 59 isolation checks (55 manifest + 4 version-record) pass | Drafted + fixtures |
| F14 | Medium | Accept; literal provider responses; mocks declared | Fetch-stage fixtures with `providerOverrides`, `mock: true` | Changed bytes are rejected by `fetch.origin` before `file.hash` | `neg-fetch-*`, `neg-file-hash`, decode and structure fixtures | Drafted + fixtures |
| F15 | Medium | Accept; correct 43/43 | Boundary pairs (§8.2) | 1048576 and 1048577 both need 43 chunks | `boundary.split` checks; 6 data boundary pairs | Drafted + fixtures (contract boundaries: E01) |
| F16 | Medium | Accept; Σ logical sizes proposed; replace the invalid test | P25 (§2) | 4 × 1 MiB files with shared chunks, plus a 1-byte file | `pos-site-4194304`, `neg-site-4194305` | Open: U12, U17 |
| F17 | Medium | Accept | T1 rewritten with preconditions, expected slots, permitted changes, traces, crash points (§8.1) | — | `t1_02_expectedState`; T1 is not executed | Drafted. Open: E01 |
| F18 | Medium | Accept the gap; my order rejected | Codex order adopted (§6.2); `site_navigate` has no selector | Query/fragment split first; selector only in the path component | 30 omnibox and 11 navigate cases | Open: U13 |
| F19 | Low | Accept | P02 table (§2) | u32 `entryIndex` as a proposal | `neg-version-*`, `neg-entry-index-range`, `neg-int-*`, `neg-hash-*`, `neg-addr-*` | Drafted + fixtures; `entryIndex` width is U20 |
| F20 | Low | Accept the clarification; correct the table | P01a clarified (§1); T1-06c is the only injection test | Ordinary idempotence needs no injection | T1-05/T1-06c specified, not run | Drafted. Open: E01 |
| F21 | Low | Accept | B-EVM (§1) | Pin tool versions when selected | — | Drafted. Open: E01 |
| F22 | Low | Accept the record requirement; no invented values | P27 lists the required metadata (§1) and the scope of the opcode check | Not a pre-implementation prerequisite | — | Drafted. Open: E04 |
| F23 | Low | Accept; treat as proposed | P28 (§1); every fixture tagged `factoryAssumption` | — | Fixture headers | Open: U01 |
| F24 | Low | Accept | Phases, reconciliation, nonce replacement, confirmation depth, checkpoint schema (§7) | A reorg after the check remains possible; T25 has 44 content chunks plus manifest chunks | `boundary.T25 v2 content chunks = 44` | Open: U16 |
| F25 | Low | Accept; decide the 10 s meaning | B limits carried (§3.3); per-request preferred; no whole-load deadline | No whole-load deadline introduced silently | — | Open: U14 |
| F26 | Low | Accept; default to the full gate | Gate restored (§9); annex checklist | — | `M1-ANNEX-CHECKLIST-0.2.md` | Open: U15 |

Summary:
- Drafted + fixtures: F01, F03, F06, F13, F14, F15, F19.
- Drafted only, or drafted with an experiment pending: F02, F17, F20, F21, F22.
- Open on a decision or experiment: the remaining 14.
- Closed: none.

## Errata to `M1-CLAUDE-REVIEW.md`

The review file is preserved unchanged. These corrections, raised by Codex and accepted, apply to it:

1. **F15:** "file 1048576/1048577 bytes (43/44 chunks)" was wrong. Both need 43 chunks (42 × 24575 + 16426 or 16427).
2. **F16:** the acceptance test using two 2097152-byte files was invalid, because each file is limited to 1 MiB. It is replaced by 4 × 1 MiB plus 1 byte.
3. **F13:** "pass only when its own rule is enabled" reversed the relationship. A fixture must be rejected while its rule is enabled. Disabling the rule should expose the regression (acceptance, or the next rule).
4. **F18:** the proposed order (strip `@v<n>` before splitting at `?` and `#`) contradicted its own example table. The query/fragment split comes first.
5. **F07:** the before/after block-hash check was presented as a remedy. It is only a detector: an A → B → A sequence evades it.
6. **F05:** "references that do not begin with `/`" was not enough to classify a relative reference. Schemes and `//` network-path references must be classified first.
7. **F20:** the disposition table implied that ordinary idempotence needs injection. Only the artificial mismatch case does.
8. **F09:** the canonical manifest split was presented as following from P04. It is a new contract restriction.
