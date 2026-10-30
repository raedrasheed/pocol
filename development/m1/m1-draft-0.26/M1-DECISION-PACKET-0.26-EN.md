# M1 owner decision packet (draft 0.26)

**Status:** decision requests, not decisions. The ledger records no owner answer. Technical decisions adopted by the reviewer in 0.25 do not select any option below.

## Owner decisions needed

| ID | Requested choice | Effect | Recommendation |
|---|---|---|---|
| U01 | Confirm factory address `0x…00c0c005` or specify another | Determines chunk addresses, every manifest and T1-02 slot values. A different value requires regeneration and verification by all three libraries | Confirm |
| U02 | Explicit revoked-version viewing: warning page then click, or rejection | Determines T43-9 and viewer behavior | Warning page then click |
| U10 | Retain previous owner's publisher permission after transfer | Changes T1-12 and the contract | Retain with publisher-tool warning |
| U14 | Ten-second timeout: per request or entire load | Determines whether long loads are interrupted | Per request |
| CR-M1-01 | Adopt or reject proof-backed state reads (eth_getProof with six keys, 1 MiB STATE_POOL and retry limits) | Without adoption, a consistent snapshot against A→B→A is not guaranteed | Adopt with E03 as a confirming test |
| U08 (change request) | Canonical manifest chunking into 1–3 chunks | **A:** enforce in contract and viewer, narrowing contract inputs. **B:** at most 3 chunks, flexible sizes. **C:** no rule; amend CR-M1-01 for multiple proof reads. **D:** viewer rejects only, so a version can be stored but not displayed | A; D if narrowing the contract is unwanted |
| RF-E6-1 | Literal total conflict in x12b: text says 1, complete table gives 2 | Determines baseline meaning | The owner question remains open; this packet does not decide it. Earlier recommendation: A (total 2) |

Technical reason for U08: the CR-M1-01 proof read carries exactly six keys, including three manifest-chunk slots. More than three chunks cannot fit in one request, and bounds grow without a ceiling.

## No owner decision needed

Technically adopted by the reviewer, subject to limitations:
- U03, U04, U06, U07 (only as a procedure conditional on E02), U09, U11–U13, U16–U18, U20–U24, U26, U31–U41, U43;
- P items;
- hex16 epoch representation;
- Existing str(n) rule;
- Exact-integer chainId parsing.

**New in 0.26, awaiting the reviewer rather than owner:**
- V1 conventions P-V1-1 through P-V1-10;
- Review of V1 vectors and independent hash verification.

## Overall effect

- If the reviewer confirms the 0.26 run, 34 rows are complete.
- C1–C4, R1 and E6 remain pending on the owner decisions above.
- V1 remains pending only on reviewer review.
