# M1 Spec 0.25 Amendment: consolidated audit, supplements, hash freeze, representations

**For root review. Not approved. Nothing was executed by the author.** No earlier file, result, ledger or baseline text is changed. Every new item is labelled **S25-…** (supplement), **CR-…/CID-…** (representation proposal) or **P-…** (proposed convention).

## Basis

- **Accepted:** 0.24 (REVIEW-0.24: 41 Partial, 0 Not started, 0 Complete). The review asked for a consolidated audit and the hash-freeze inventory.
- **Ledger:** C01, C06 and C07 open; F01–F26 open collectively; CR-E4-01/02 proposed; RF-E6-1 unapproved. No owner answer is recorded.

## Changes

1. **Audit** (`audit/`):
   - `row-inventory.json`, with R3-08 criteria computed by the runner;
   - `findings-trace.json`, giving F01–F26 individually and the eight Codex corrections;
   - `decision-register.json`, separating owner from reviewer items;
   - `gap-registry.json`, classifying every saved partial gap;
   - `M1-AUDIT-0.25.md`, a readable summary.
2. **Supplements** (`supplements/`):
   - **S25-SWEEPMAX:** the BR22a/BR23 negative control of validation.md:682, with convention **P-SM-1**.
   - **S25-LCLIT:** literal LogClient replies, implementing P-V2-10.
   - **S25-RG3B:** the RG3b hook protocol **P-RG3B-1** and three scripts.
   - **S25-E07:** the viewer version-selection table.
   - **S25-EXPSPEC:** exact inputs and pass criteria for E01–E07.
3. **Hash freeze** (`hash/hash-freeze-plan.json`): classes, parameters and blockers. The runner writes `results/hash-freeze-export-0.25.json` with the raw preimage hex, the expected digest, the source and the blockers for root's three-library verification.
4. **Representation alternatives** (`representation/`):
   - **CR-E4-01:** recommended A, the hex16 epoch string.
   - **CR-E4-02:** recommended R, no baseline change. Finding **F25-SUR-1:** browser.md:200/225 already reject lone surrogates at B3.
   - **CID-1:** recommended A, exact chainId reading.

   `tools/js_number_probe.cjs` is an optional Node probe for root.
5. **Runner** (`tools/run_checks_025.py`, `tools/supplements_ref.py`):
   - reuses saved evidence after proving that the reviewed copies equal the current packages;
   - executes every new supplement and alternative;
   - writes only new files under `results/` and never overwrites an earlier run.

## Proposed for root decision (not owner decisions)

- **Conventions:** P-SM-1 and P-RG3B-1.
- **Representation alternatives:** CR-E4-01 A, CR-E4-02 R and CID-1 A.
- **The F01–F26 dispositions:** proposals only.
- **The reviewer items** of `audit/decision-register.json`.

## Not changed

- No owner policy and no earlier package.
- The deferred experiments are not run.
- V1's missing window vectors, texts and NX-V3e/f/g are not authored in this turn; they are recorded as blockers.
