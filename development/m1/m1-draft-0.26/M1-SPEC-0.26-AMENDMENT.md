# M1 Spec 0.26 Amendment: V1 completion and consolidated criteria

**For root review. Not approved. Nothing was executed by the author.** No earlier file, helper, result, ledger or baseline text is changed.

## Basis

- **REVIEW-0.25** accepted the 0.25 audit and supplement batch: 702 entries, 617 pass, 85 recorded, 0 FAIL.
- **REVIEWER-DECISIONS-0.25** records the qualified technical decisions:
  - U08 is an owner change request.
  - Owner items stay open.
- **Root verification** covered:
  - the 56 new 0.25 preimages, with three libraries;
  - the hex-epoch codec, natively and with 10 round-trips;
  - the 1442009-byte maximum record.

## Changes

1. **V1** (`annex/V1-HEADERNETCHECK.md`, `vectors/v1-*.json`, `tools/v1_ref.py`):
   - the normative RP window;
   - literal header field tables;
   - 20 window cases with literal requests, expected outcomes, messages and instrumented counts;
   - RW1–RW6 with the exhaustive check;
   - HC1–HC7 with the literal Arabic messages and the 10^4 seeded check;
   - ASERT rows A1–A7;
   - NX-V3e/f/g;
   - deferred experiment definitions (`supplements/s26-v1-experiments.json`);
   - proposed conventions P-V1-1…10.
2. **N26** (`normative/N26-REPRESENTATION.md`, `vectors/n26-representation-cases.json`): CR-E4-01 A, CR-E4-02 (existing B3 rule) and CID-1, as normative text, with literal decode fixtures.
3. **U08** (`owner/U08-CHANGE-REQUEST.md`): routed to the owner, with alternatives A–D, the consequences for the 6-key proof read, and recommendation A (fallback D).
4. **Consolidation** (`audit/consolidation-0.26.json`, `audit/findings-trace-0.26.json`):
   - the R3-08 criteria are recomputed from the recorded decisions;
   - U08 is moved to the owner;
   - the accepted 0.25 supplements are bound to their canonical results.
5. **Hash extension** (`hash/hash-freeze-plan-0.26.json`). The runner exports:
   - every V1 TemplateID, powHash (SHA-256), blockHash, SigMsg and WinMsg (Keccak);
   - the V1NET genesis hash;
   - `keccak256('other')`;
   - the public-key preimages for HF-BLK-KEYS. No private key is exported.
6. **Runner** (`tools/run_checks_026.py`):
   - standalone, with `--root`/`--out` or the environment variables;
   - validates every generated artifact in its own output directory, which fixes the 0.25 copy-run failure;
   - never overwrites an earlier run.
