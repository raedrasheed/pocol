# Owner change request P-C32-2: client policy for −32021 `rate` delays in HeaderNetCheck

**Status: OPEN, for the owner. Nothing is adopted.** Root classified P-C32-2 as an unapproved owner client policy (REVIEW-0.27; ledger `P-C32-2`).

## The gap

browser.md:41 retries −32021 after `retryAfterMs`, up to 3 times. network.md:364 shapes the reply as `{reason ∈ {busy, rate}, retryAfterMs}`. network.md:234 bounds busy delays to [250, 2000] ms, but **no range or formula is given for rate delays** (network.md:215–218).

A client must therefore choose what to do with a rate delay of, for example, 5000 ms or 10^9 ms. Without a rule, an RP server can make the client sleep for as long as it likes (up to 3 times), unless U14 imposes a whole-load deadline.

## Alternatives

| ID | Rule for `rate` delays | Observable consequence (rate replies only) | Relationship to U14 |
|---|---|---|---|
| A | Defensive client cap: an integral delay in [0, 2000] is retried; anything else gives viewIncomplete immediately, with no sleep | Total retry sleep ≤ 6000 ms. A server sending a rate delay above 2000 ms loses the RP view at once (U14-T6: viewIncomplete at t = 200). Servers using formulas F1 or F2 (annex section 2) are retried normally | Under PR it is the only bound on sleep. Under WL it is a tighter inner bound that keeps a 10 s budget from being consumed by sleep alone |
| B | Source-derived cap: the owner fixes the server formula in network.md (for example F1: ≤ 50 ms for single requests, or F2: ≤ 1000 ms), and the client enforces the same bound | A tighter bound with a stated basis. It needs a source change on the server side as well. Servers using a different formula would be rejected | As A, with a smaller sleep budget |
| C | No rate cap: any non-negative integral delay is retried | Under PR an attacker-chosen delay stalls the view for up to 3 × the delay (U14-T6-C: 15 s). Under WL the deadline ends the stall at 10 s | Bounded only if U14 = WL |
| D | Same range as busy, [250, 2000] | Rejects F1 servers whose correct delay is below 250 ms (for example 50 ms). Contradicts the token-bucket timescale | As A |
| E | No retry for `rate`: viewIncomplete immediately | No sleep at all, but the view fails on every transient rate limit. Changes browser.md:41, which retries −32021 without distinguishing the reason | Independent of U14 |

## Recommendation

**A**, stated explicitly as a client policy, together with an explicit U14 answer.

- **Why A:** it is the smallest rule that bounds sleep under either U14 branch and retries every delay that formulas F1 and F2 can produce. It needs no server-side source change.
- **Why not B:** B is preferable only if the owner also wants to fix the server's formula.
- **Why not C:** C is acceptable only if U14 = WL.
- **Why not D or E:** each conflicts with the bucket timescale or with browser.md:41.

## Exact observable consequences (fixtures)

- **Under A:**
  - 0.27 cases C32-ok-rate0 and C32-ok-rate2000 retry;
  - C32-rate2001 → viewIncomplete with no sleep;
  - U14-T6 → viewIncomplete at t = 200.
- **Under C:** C32-rate2001 would retry (sleep 2001). U14-T6 completes after 15550 ms under PR and fails at 10000 ms under WL.
- **Under B (F1 bound 50):** C32-ok-rate2000 would become viewIncomplete.

## Until the owner decides

The reference checker keeps the 0.27 behaviour (alternative A) only as a labelled proposal. Every rate-branch result in the 0.27 evidence is conditional on it (root: `rateCasesAreConditionalOnUnapprovedCap: true`).
