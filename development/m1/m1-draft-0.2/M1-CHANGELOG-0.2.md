# M1 Changelog: Draft 0.1 → Draft 0.2

Draft 0.1 (`../M1-SPEC.md`, `../vectors/`, `../generate_vectors.py`, `../VALIDATION.json`) and `../reference/` are unchanged. All draft 0.2 material is in `m1-draft-0.2/`. Labels follow `M1-SPEC-0.2.md` §0.

## Restored baseline text

- **B05:** full path grammar `('/' seg)+` with the 256-byte full-path limit (FD:L895). Draft 0.1 had dropped it (F03).
- **B07a:** the empty-file form follows from B07 (F01).
- **B-EVM:** Cancun target carried into the experiment settings (F21).
- **B09:** factory-origin rejection relabelled as a baseline requirement (T16 "another factory"); P06 kept as the mechanism (F06).
- **B:** retrieval limits of 8 connections and −32021 retry carried forward (F25).
- **§9:** full implementation gate restored by default (F26).

## Withdrawn

- **P03** (reject empty files): withdrawn. It contradicted the baseline (F01; Codex agreed).

## Revised proposals

- **P01:** no factory events (U18).
- **P01a:** idempotence is testable normally; the mismatch branch only through a declared mock (F20).
- **P02:** full type/width table; version = 1; u32 `entryIndex` (U20) (F19).
- **P05:** publisher and viewer share the rule IDs.
- **P08:** split into §6.1 lookup, §6.2 omnibox, §6.3 `site_navigate` and §6.4 references.
- **P09:** constructor argument; the owner counts within 16; publishers kept on transfer (U10), with a tool warning.
- **P11:** draft reuse is the publisher's job.
- **P13:** unchanged in substance; literal slot keys added.

## New proposals

| ID | Content | Finding(s) | Decision |
|---|---|---|---|
| P14 | Factory length check first; error precedence; error identifiers | F02 | — |
| P16 | Validation stages, rule IDs, first-error precedence, zero fetches before the fetch stage | F13, F14 | — |
| P17 | Version-selection table | F04 | U02, U03 |
| P18 | Omnibox parse order (Codex order) | F18 | U13 |
| P19 | Stage R reference classification and resolution | F05 | U04 |
| P20 | Proof-bound snapshot reads | F07 | U05 |
| P21 | Slot-read retrieval | F08 | U06 |
| P22 | Responsibility matrix; canonical manifest split as a contract rule | F09 | U07, U08 |
| P23 | Find-or-create draft on full equality | F10 | U09 |
| P24 | Website transition, error and event tables | F12, O03 | U11, U22 |
| P25 | Site size = Σ logical file sizes; duplicate chunk refs allowed | F16 | U12, U17 |
| P26 | Publisher phases, reconciliation, checkpoint schema | F24 | U16 |
| P27 | Reproducible-build record requirements | F22 | — |
| P28 | Factory address constant | F23 | U01 |
| P30 | Navigation stays within the loaded version; reload re-anchors | — | — |

## Tests

- **T1** rewritten (T1-01 … T1-15): preconditions, exact errors, slot diffs, and the broken variant each test must catch.
- **T16** replaced by fixture files with `rule`, `stage`, `isolation` and `fetchCalls`.
- **Fixture counts:**

| Kind | Count | Notes |
|---|---|---|
| Positive manifests | 11 | |
| Negative manifests | 62 | |
| Version records | 6 | |
| Path cases | 58 | |
| Boundary pairs | 6 | |

The 11 draft 0.1 negatives are kept:
- 8, byte-identical, as single-fault fixtures under new IDs;
- 3, byte-identical, as documented multi-fault fixtures (`neg-draft01-*`).

## Tooling (specification tooling only; no third-party packages)

- `tools/keccak.py`: an independent Keccak-256. It reproduces all four draft 0.1 vector files byte for byte.
- `tools/rlp_strict.py`: a strict RLP decoder with rule IDs and rule-disable hooks for the mutation harness.
- `tools/m1model.py`: the reference validator for §3, including the manifest-record stage.
- `tools/m1paths.py`: the reference model for §6.
- `tools/m1abi.py`: proposed selectors, topics and slot keys.
- `tools/gen_fixtures.py`: a deterministic generator.
- `tools/run_checks.py`: executes every claimed check and writes `results/run-results.json` and `results/fixture-hashes.json`.

## Errata to the Claude review

See `M1-DISPOSITIONS-0.2.md`, "Errata". The review file itself is unchanged.
