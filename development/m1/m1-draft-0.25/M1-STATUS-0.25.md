# M1 Draft 0.25: Acceptance report

**Nothing in 0.25 was executed by the author.** Every statement below is a claim about what root's run of `tools/run_checks_025.py` must show. A claim holds only if that run shows it.

## Coverage

- **Accepted (0.24):** 41 Partial, 0 Not started, 0 Complete.
- **Proposed after root's run:** 41 Partial, 0 Complete.
- **This is not M1 completion.** The full gate (C01) stays unmet.

## Demonstrably satisfied (with existing, reviewed evidence)

These hold now, because saved root results and reviews exist for unchanged files. The 0.25 runner re-checks that the files are unchanged.

1. **R3-08 c1 (spec text and literal fixtures exist):** 35 rows.
   - Not V1.
   - Not C3, E3, E4, V2 or V3, whose new 0.25 material is still pending review.
2. **R3-08 c2 (computable claims executed, machine-readable):** those 35 rows plus V1 (its RW/HC checks ran), so 36 rows. Every cited saved check-ID pattern has at least one pass and no unexplained FAIL. The three historical FAILs are tied to their closing evidence.
3. **R3-08 c3 (root review):** the same 36 rows. REVIEW-0.2 … REVIEW-0.24 exist, and each accepted the batch that carries the row.
4. **E05 for the triad set and GSV1.** The 611 preimages were checked by three libraries, and GSV1 and K1–K3 by the E05 evidence. The 0.25 runner re-binds every preimage to the current fixtures; it does not re-hash them.
5. **Hash fixtures freezable now:** these need no unapproved parameter.
   - K1–K3 and GSV1;
   - the 486 file contents;
   - the 36 chunk data, runtime and initcode preimages;
   - any triad manifest that lists no chunk address.

## Pending root review (new 0.25 material)

These checks run and pass in the author's design, but have not yet been run:
- **Supplements:**
  - S25-SWEEPMAX (row E3);
  - S25-LCLIT and S25-RG3B (V2);
  - S25-E07 (C3);
  - S25-EXPSPEC (E01–E07 definitions).
- **Representation alternatives:** CR-E4-01 and CR-E4-02 (E4); CID-1 (V3).
- **The hash-freeze export.** Root's three-library verification of the new preimage groups is the E05 step that remains.

## Blocked

1. **R3-08 c4 (decisions by their named decider): blocked for 40 rows.** Only V1 has no pending decision.
   - **Owner items:** U01, U02, U10, U14, CR-M1-01 and RF-E6-1. The last is decided by the owner or root.
   - **Reviewer items:** listed in `audit/decision-register.json`. No reviewer decision on them is recorded.
2. **R3-08 c5 (genuine blockers closed): blocked for 10 rows.**
   - C1, C2, C3 and C4 (U01; C3 also CR-M1-01, U02, U10, U14; C4 also U10).
   - R1 (CR-M1-01).
   - E6 (RF-E6-1).
   - V1 (missing definitions).
   - X1 and B4 (the netKey hash depends on P-X1).
   - B8 (the txA hash depends on TK-1).
3. **V1, missing definitions:**
   - the HeaderNetCheck window vectors (h = 3 anchored on genesis; viewIncomplete for a missing header and a missing reference at h = 20; viewGenesis; h = 0 → viewNoBlocks);
   - the HC message texts;
   - NX-V3e/f/g.

   This is authoring work. NX-V3e/f/g also depend on the M3-TS/ASERT model (FD:L4951).
4. **Hash freeze, blocked entries:**
   - every CREATE2 preimage and every address-bearing manifest (U01);
   - the selectors, topics and slot keys (P-ABI/P13; compiler confirmation is E04, Phase A);
   - the netKey (P-X1);
   - txA (TK-1);
   - the alternative-E selector (not adopted);
   - the V3 network-profile digests over placeholders (DG-V3-1/2);
   - key-derived addresses (HF-BLK-KEYS).

## What would make rows Complete

If root records the reviewer decisions and accepts the 0.25 material, 34 rows have no remaining blocker. They are X1–X3, B1–B11, R2–R4, S1–S4, Q1–Q4, E1–E5, E7 and V2–V4. For X1, B4 and B8, their hash entries then become freezable after the three-library check.

The other seven rows also need:
- **C1–C4, R1, E6:** the owner answers.
- **V1:** the missing authoring work.

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open (no named owner answer in the ledger).
- RF-E6-1 is a documented source discrepancy pending the owner or root.
- CR-E4-01/02 are proposals.
- CONF_DEPTH is a required runtime input.
- Full gate; no production work, merge or deploy.
