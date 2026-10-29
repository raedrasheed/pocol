# Annex V1-SHA-COST: the client SHA-256 bound against the approved source

**For root review. Not approved. The author ran nothing.** This annex repairs the counting and records the discrepancy. It does not change normative acceptance and does not adopt any relaxed bound.

## Source

**The bound (browser.md:58):** «كلفة العميل: ≤ 14 فك رأس، و≤ 13 ASERT بوسائط ≤ 512 بتًا، و≤ 13 SHA-256».

**The checks that require SHA-256 in the RP window.** browser.md:44–50 requires items 1b, 2–5, viewTargetCeil and 6–9 for every window header:
- **TemplateID** = SHA256(RLP(UT)) (consensus.md:59). It is needed:
  - for `blockHash(p)` (consensus.md:64), in item 2 of the child, including the reference for the first window header;
  - for SigMsg (item 3), the PoW preimage (item 7), WinMsg (item 8) and every share preimage (item 9).
- **powHash** = SHA256(TemplateID ‖ be64(nonce)) ≤ target (item 7, consensus.md:61, 159).
- **Share check:** for each share, SHA256(TemplateID ‖ be64(n)) ≤ T_share (item 9, consensus.md:161). There are up to 256 shares per header (consensus.md:55), and T_share = min(2^256 − 1, target·m) (validation.md:926, D143).
- **No other SHA-256 path.** The seed, ticket and Draw hashes belong to the membership items, which are not evaluated in RP. Keccak is a different function. The reference header is never PoW- or share-checked.

## Accounting (all SHA-256 evaluations, `tools/v1_ref_027.py`)

| Window | TemplateID | PoW | Shares | Total (cached) | Total (no cache) | Unique preimages |
|---|---|---|---|---|---|---|
| h = 3 (genesis anchor) | 3 | 3 | 0 | 6 | 14 | 6 |
| h = 20, no shares | 14 | 13 | 0 | **27** | 65 | 27 |
| h = 1, 256 shares (root probe) | 1 | 1 | 256 | **258** | 516 | 258 |
| h = 20, 256 shares per header | 14 | 13 | 3328 | **3355** | 6721 | 3355 |

The last three rows are real fixtures built and checked by `run_checks_027.py` (`SHA-W`, `SHA-S1`, `NX-V3g`/`RW-h20`). The runner compares every number with the hand derivations of `vectors/v1-sha-accounting.json`.

## Does caching change the bound?

- **Logical evaluations, yes:** without a TemplateID cache, each use recomputes it. That gives 3380 TemplateIDs instead of 14 in the full worst case.
- **Unique preimages, no.** Every TemplateID preimage is a distinct header. Each share preimage differs from the PoW preimage (n ≠ nonce) and from the other shares (strictly ascending).

So 3355 is the minimum for a full window that checks every required item, and 27 for a share-less one. **No implementation can meet «≤ 13 SHA-256» for n ≥ 7 without skipping a required check.** Even h = 7 with no shares needs 7 + 7 = 14.

## Status

- **Withdrawn:** the 0.26 reading P-V1-3 («SHA-256 = PoW only»). It omitted the share checks, and it reinterpreted the source without authority.
- **Unresolved discrepancy.** The literal source bound «≤ 13» stays unresolved. It goes to the owner as a change request: `../owner/V1-SHA-COST-CHANGE-REQUEST.md`.
- **V1 stays blocked** on that answer.
- **What this annex does** (an instrumentation and reference repair, permitted now):
  - counts every SHA-256 call;
  - reports cached, uncached and unique preimages;
  - adds the share fixtures.
- **What it does not do** (a normative change, not approved): change which checks run, or the stated bound.
