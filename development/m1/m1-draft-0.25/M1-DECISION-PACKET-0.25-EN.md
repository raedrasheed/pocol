# M1 decision packet (draft 0.25)

**Status:** decision requests, not decisions. The ledger (`coordination/issue-ledger.json`) records no owner answer, so the six items below remain open. Nothing was executed by the author.

## First: decisions requiring the owner

| ID | Requested choice | Effect | Recommendation | Evidence |
|---|---|---|---|---|
| U01 | Confirm full factory address `0x0000000000000000000000000000000000c0c005`, or specify another | Affects every chunk address, CREATE2 input, address-bearing manifest and T1-02 slot value. Another value requires deterministic vector regeneration and verification by all three libraries for affected rows | Confirm if `0x…C0C005` in FD:L777 abbreviates a zero-padded address | P28, `code-table.json`, three-way binding in `hash-freeze-export-0.25.json` |
| U02 | Explicit `@v<n>` viewing of a revoked version: warning page, explicit click and persistent banner, or rejection | Determines E07 row T43-9 and viewer behavior. Rejection is a stricter rule requiring a change request | Warning then click, because FD:L914 prohibits only default viewing | `supplements/s25-e07-viewer-table.json` |
| U10 | Previous owner's publisher rights after transfer: retained until explicitly removed by the new owner, or automatically revoked | Changes T1-12 and contract `transferOwnership` semantics | Retain (P09), with a publisher-tool warning | M1-SPEC-0.2 section 5.1, T1-12 |
| U14 | Meaning of “10s timeout” (FD:L905): per request or entire load | Loading 40 chunks at one second per request succeeds under the first option and is interrupted under the second | Per request, with no whole-load timeout | M1-UNRESOLVED-0.2 (U14) |
| CR-M1-01 | Adopt, amend or reject proof-bound snapshot reads, including BLK-01, U05, U25 and trie assumption PR-1/PR-2 (BLK-03) | Adoption adds `eth_getProof` with a 524288-byte bound (conditional on PR-1/PR-2), a 1 MiB STATE_POOL and retry limits (≤ 3 per request, ≤ 10 s per wait, ≤ 20 s per load). Rejection recasts P36 as detection only, without an A→B→A guarantee | Adopt, with E03 as a confirming PR-1/PR-2 test in phase A | `m1-draft-0.4/CR-M1-01-STATE-PROOF-READS.md`, `m1-draft-0.5/CR-M1-01-REV2.md` |
| RF-E6-1 (owner or root reviewer) | x12b conflict: text says `adminStaleDropped = 1`, complete table gives 2 | Determines baseline meaning for the `adminNoCancel` fault | Option A: add cell 5001; total 2 (op1 = 1 and op2 = 1) | `m1-draft-0.17/annex/C27-RFE61.md` |

**Effect of owner decisions.** If all six are answered, C1–C4, R1 and E6 remain blocked only by reviewer decisions. V1 remains blocked by incomplete authoring.

## Second: no owner decision needed

- **U15:** the standing assumption is the complete gate, without exception.
- **U16 (CONF_DEPTH):** required operational input, no default. Accepting this wording is a reviewer decision; the owner may add a value later.
- **U19:** outside M1.
- **U42:** withdrawn.
- **BLK-02:** resolved by restoring X3.

## Third: technical conventions for the root reviewer

These do not change owner-controlled policy; details are in `audit/decision-register.json`:
- **Drafts 0.2–0.5:**
  - U03, U04, U06–U09, U11–U13, U17, U18, U20–U24, U26;
  - U31–U41, U43;
  - P items in drafts 0.2 and 0.4–0.7;
  - P-X1, TK-1, I1, I6.
- **Drafts 0.9–0.24:**
  - P-Q1 through P-Q4;
  - P-E1 through P-E7, P-C24, RF-E5;
  - P-V2, P-V3, PB1–PB12.
- **New in draft 0.25:**
  - P-SM-1 (sweepMax supplement);
  - P-RG3B-1 (RG3b hook);
  - CR-E4-01: recommend alternative A, a hex16 string epoch. JS reads 9007199254740993 as 9007199254740992 and 2^64−1 as 2^64. Cost: two bytes; the u64 range is unchanged;
  - CR-E4-02: recommend alternative R. The baseline already rejects lone surrogates in B3 through `str(n)` (browser.md:200 and 225), so no change is needed;
  - CID-1: recommend exact-integer parsing of profile chainId. The same values risk JS rounding; format and range are unchanged.

## Fourth: incomplete work not requiring a decision

V1 still needs:
- Window vectors;
- HC message texts;
- NX-V3e/f/g, which depend on the M3-TS/ASERT model.

This is specification work, not a decision.
