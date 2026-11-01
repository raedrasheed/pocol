# M1 Draft 0.30: C33, C34 and the 17 delegated conventions

**For root review. The author ran nothing. M1 is not complete.**

This revision is narrow. It fixes the two issues REVIEW-0.29 identified and applies the conventions in `coordination/DELEGATED-M1-CONVENTIONS-030.json`. Everything in 0.29 that these do not touch is bound by hash and by root's 0.29 results; it is not redone.

## 1. C34: the U08 negative-case invariant

**The defect.** The 0.29 runner required `sum(lens) == len` for every noncanonical case. N05 deliberately declares 24576 bytes with chunks summing to 24575. Contract and viewer both rejected it correctly (`ManifestSplit(1)`, `manifest.split`, zero fetches), but the blanket assertion turned that into four FAILs:
- `U08.noncanonical.N05`;
- `decision.U08.applied`;
- `consolidation.findingsMatchProposal` (F09, F17);
- `coverage029.required`.

**The repair** (`vectors/c34-u08-invariants.json`):
- Every U08 case carries its own `expectedSum`, `delta` and `sumMatches`. That covers the 6 boundaries, 2 invalid lengths, 1 array mismatch and 12 noncanonical splits.
- N05 states 24575, −1 and `false`. Every other case states its exact sum and `sumMatches: true`.
- Blanket coverage stays. Every case must have an invariant and satisfy it, and the case sets must equal those of the 0.29 vector.
- That vector is read unchanged. Its sha256 must equal the value root's 0.29 run recorded.
- **Eight mutants** run through the same evaluator and must all be killed:
  - canonical lengths for N05;
  - a wrong expected sum;
  - a wrong `sumMatches`;
  - a wrong split index;
  - an N06 without its zero chunk;
  - a length above 65536;
  - a wrong delta;
  - a dropped N05.
- The four 0.29 FAILs stay in root's saved file. `history.HF-8` binds each one to the check that now replaces it.

## 2. C33: guarded HeaderNetCheck receive boundary (P-V1-11)

browser.md:271–288 routes every HeaderNetCheck request through HttpTransport/RecvGuard. The 0.29 `DeadlineRpc` handed raw text straight to the 0.27 classifier, so root's three probes rendered a frame: wrong id 9999, missing id and a duplicated `result`.

`tools/guard_ref_030.py` adds a receive guard **before** the unchanged 0.27 semantic checker. It applies to every raw reply: `eth_blockNumber`, every `pocol_getHeaders` attempt, errors and retries.

| Step | Rule | Failure | Source |
|---|---|---|---|
| 1 | Declared Content-Length > recvLimit | recvLimit, before any byte is read | browser.md:275 |
| 2 | Received bytes > recvLimit (4096 for `eth_blockNumber`, 98304 for `pocol_getHeaders`) | recvLimit | browser.md:278, 299, 301–302 |
| 3 | Fatal UTF-8 decode; one leading BOM dropped, as the WHATWG TextDecoder default does | recvParse | browser.md:280 |
| 4 | Container depth > 16 | recvDepth | browser.md:281 |
| 5 | parseStrict: a duplicate key at any depth, NaN/Infinity, any syntax error | recvParse | browser.md:281; the 0.4 parser `bridge_ref.py` |
| 6 | Error `message` cut to 256 UTF-16 units; `data` dropped if its compact UTF-8 encoding exceeds 4096 bytes | — (bounded, not silently accepted) | browser.md:283–285 |
| 7 | The reply is an object whose `id` has the C19 value equal to the current request id (0..2³²−1; bool, string, null and non-finite values are invalid; 2.0 and 2e0 match 2) | envelope | P-V1-11; `bridge_ref_06.integral_value` |

**Request ids** are a reference fixture convention (P-V1-11):
- `eth_blockNumber` is 1;
- `pocol_getHeaders` is 2;
- every retry is again 2.

**When the guard fails**, the outcome is controlled:
- viewIncomplete, no frame, 4901;
- no retry, no wait and no header or SHA-256 work;
- the reason and stage are recorded.

**Error replies.** A guard-accepted reply reaches the checker unchanged. Only a clipped error reply is re-serialized. A −32021 whose `data` was dropped has no delay, so it ends controlled without sleeping. That is the baseline consequence; there is no silent acceptance.

**Not claimed.** The pool reservation (browser.md:274) and any allocation or memory behaviour are outside this fixture model and are claimed nowhere.

**Fixtures** (`vectors/c33-guard-cases.json`, 60 cases):
- a positive control on the saved SHA-S1 replies;
- the three root probes;
- 14 headers-reply id forms, including the matches 2.0/2e0/0.2e1/20e-1 and the mismatches "2", true, 1, 3, −2, 2³²+2, 2.5, 1e400, null and [2], plus the `eth_blockNumber` id;
- duplicate keys at the top level, in `id`, through an escaped key, in `error.code` (root scope probe 1), in `data.reason`, in `retryAfterMs` and in `eth_blockNumber`;
- near-neighbour non-duplicates;
- busy with id 9999 (root scope probe 2);
- the exact 4096/4097 and 98304/98305 byte boundaries, and a declared Content-Length;
- depth exactly 16 and 17;
- invalid, overlong, surrogate and truncated UTF-8, a single BOM and a double BOM;
- malformed JSON, empty and whitespace bodies, a top-level array and a bare number;
- NaN and ±Infinity;
- message clipping, and `data` of exactly 4096 and 4097 bytes.

**Valid replies are preserved.**
- All 24 HeaderNetCheck deadline cases of 0.29 re-run through the guard with unchanged expectations. The one exception is H-24: its −32021 carries id 2 while answering `eth_blockNumber`, so it now stops at the guard, at the same time and with the same request count.
- Every reviewed 0.27 transcript re-runs through the guard. Verdict category, request list and send times must equal the saved ones. Where every reply passes the guard, the whole verdict must be equal.
- No saved `ok` verdict may meet a guard rejection.
- SHA-W keeps exactly 3355 SHA-256 evaluations (14 + 13 + 3328), and its headers reply fits 98304 bytes.

**Phase A.** Real HttpTransport enforcement is the new definition X-C33 (`vectors/experiment-amendments-0.30.json`).

## 3. The 17 delegated conventions

`decisions/conventions-applied-0.30.json` copies each `selection` and `status` verbatim; the runner checks them against the coordinator file. Each entry also gives a source citation and the evidence that must pass. They are coordinator selections under the user's standing delegation, not personal owner approvals. No owner answer is written or inferred.

Changes of effect:
- **P-D29-3, shared budget.**
  - The initial RP navigation has ONE 10 s budget, from the first HeaderNetCheck request through the content verdict.
  - RP-COMB is therefore effectively: HeaderNetCheck ok at 6900, content `unavailable` at 10000.
  - The separate-budget result (render at 10500) is kept only as the rejected comparison.
  - X-U14 is amended to match.
- **P-U02-2.** The interstitial ends the first load, user think time is outside every budget, and the click starts a fresh navigation budget.
- **Unchanged scope.** RpcReadClient site-frame reads keep their per-request 10 s (implementation.md:203). They are not a covered operation.
- **P-V1-11.** This is the guard of §2.
- **The other 13** (P-V1-1, -2, -4…-10, P-D29-1, P-D29-2, P-U08-1, P-U02-1, P-U10-1) adopt the 0.29 fixture behaviour as written. Their evidence is root's 0.27 and 0.29 results plus root's independent files:
  - 1120 deadline cases;
  - 3677 hashes;
  - 31 signatures.
- **P-V1-8.** V1NET (chainId 777002) stays a labelled placeholder network, with no network claim.

## 4. Hash freeze

`hash-freeze-0.30.json` is written by the runner. It freezes:
- **The 668 legacy exported preimages.** For each entry:
  - the runner recomputes the digest with the reference Keccak;
  - it requires equality with root's fresh three-library execution (`full-hash-triad-executed-029.json`, 668/668) and with that execution's input SHA-256;
  - it checks the asserted full digest or 4-byte prefix;
  - it records the delegated or reviewer decision IDs that resolve the entry's parameters;
  - it keeps any Phase A confirmation (E04) as a note, not a blocker.
- **The 4024 V1 entries** from the 0.26 and 0.27 freezes (347 + 3677). Each is recomputed and must carry an id that root verified independently.

Root's initial 611-entry failure (`full-hash-triad-029.json`) is preserved. It is explained by `full-hash-audit-029.json`: the comparison used null legacy reference fields.

The synthetic opaque groups stay labelled and are never frozen as computed hashes:
- `syn.lc.branchHashes`;
- `privateProvenance`;
- `ph.v3.networkProfiles`.

## 5. Gate

All 41 rows and F01–F26 are recomputed from root's 0.29 matrix, root's 0.29 row evidence and this run:
- **c3** is satisfied for content root reviewed in 0.29 or earlier.
- **c4** counts a delegated item or convention only when its checks pass in a root-executed run, or in this run.
- **c5** requires the repair to be proven and the hash groups not to be blocked.

Expected (computed, not asserted):
- **38 CompleteCandidate**, including C1, C2, R1 and E6, whose 0.29 content REVIEW-0.29 accepted as unaffected.
- **3 PendingRootReview**: C3 and C4 (the C34 repair and the shared budget) and V1 (C33 and the shared budget).
- **Findings:** 23 closable at specification scope; F09, F17 and F25 closable after root review of 0.30.

The ledger items C06, C07, U08-CR, RF-E6-1, V1-SHA-COST and P-C32-2 await root's final verification. They are not unanswered-owner blockers.
