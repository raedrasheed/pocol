# Owner change request V1-SHA-COST: the RP client SHA-256 bound

**Status: OPEN, for the owner. Nothing here is adopted.** Any clarification below stays a proposal until the owner authorizes it.

## The conflict

browser.md:58 bounds the RP check at **≤ 13 SHA-256**. The same file, through consensus.md, requires, for each of up to 13 window headers:
- its TemplateID;
- a PoW hash;
- one SHA-256 per share (up to 256).

It also requires the reference's TemplateID. All these preimages are distinct.

The minimum is therefore:
- **27** for a window with no shares;
- **3355** for a full window with 256 shares per header.

Evidence:
- `annex/V1-SHA-COST.md` and `vectors/v1-sha-accounting.json`;
- root's 256-share probe: 258 for one header.

No caching lowers these figures.

## Alternatives

| ID | Change | Consequence |
|---|---|---|
| A | State the bound by category: «≤ 14 TemplateID (SHA-256 on ≤ 613 B) + ≤ 13 PoW + ≤ 13·256 share checks (SHA-256 on 40 B) = ≤ 3355 SHA-256» | Every check stays. The bound becomes exact and testable. Fixtures `SHA-W` and `SHA-S1` already prove it |
| B | Read «13 SHA-256» as the PoW evaluations only, and add no other bound | Every check stays, but the document understates the real work and bounds the share work nowhere |
| C | Do not verify the share hashes (item 9's SHA part) in RP | Total ≤ 27. The RP view would accept headers whose share list is invalid, which changes acceptance |
| D | Have RP reject headers with more than k shares | The bound becomes 14 + 13 + 13k. This narrows which honest headers can be viewed, because valid blocks may carry up to 256 shares |
| E | Keep «≤ 13» literally | It cannot be met with n = 13 (D82). The window would have to shrink to n ≤ 6 or drop checks. V1 stays unacceptable |

## Recommendation

**A.**
- It keeps the approved acceptance of every RP item.
- It is exact: it is reached by a real full-window fixture, and no implementation can go below it.
- The extra work is SHA-256 on short inputs.

Its wall-clock cost on the target machine is not measured here. That is a Phase A measurement.

## Until the owner decides

- V1 remains blocked.
- The reference checker keeps all checks and reports the real counts.
- The runner records the literal «≤ 13» result for every case. That result is recorded, never passed as satisfied.
