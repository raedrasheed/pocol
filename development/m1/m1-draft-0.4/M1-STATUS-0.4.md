# M1 Draft 0.4 — Dispositions, Inventory Delta, Blockers and Pending Decisions

Read with:
- `../m1-draft-0.3/M1-DISPOSITIONS-0.3.md`
- `../m1-draft-0.3/M1-OPEN-0.3.md`
- `../m1-draft-0.3/M1-ANNEX-INVENTORY-0.3.md`

These remain in force except where this file amends them. The ledger is not edited. **No issue is closed by this document.** Every 0.4 item below is drafted with fixtures and has **not been executed by the author**.

## 1. Dispositions C10–C13 (and C01, C06 progress)

| Issue | Revision | Change | Literal evidence (unexecuted) | Proposed status |
|---|---|---|---|---|
| C10 | R4-01 | Ordered state checks S1–S3 and V1–V4. Undefined status, corrupt count or current value, and absent records give `stateInvariant` with zero frames | `vectors/snapshot-invariants.json` (SNAP-20…34); the 13 inherited 0.3 P20 scenarios; the property "only status 1 renders" | Drafted + fixtures |
| C11 | R4-02 | Phases S, D and A. Experiments are Phase A acceptance conditions, not Phase D entry conditions. This task is Phase S only | Text | Drafted |
| C12 | R4-03 | 426 is withdrawn as a viewer bound. The derived viewer bound is 2847 content references, or 2850 requests per load. Non-canonical splits stay valid | `vectors/request-bound.json`: Codex 1001, the 2847 maximum, 2848 does not fit, two non-canonical splits | Drafted + fixtures |
| C13 | R4-04 | Count corrected to 17. ADD-M1-01 is an unsigned addendum proposal with mapping decisions, inputs and pass criteria | `annex/addendum-x3-proposal.json` | Blocked on the owner (BLK-02) |
| C06 | R4-05 | CR-M1-01 as a concrete addendum: schemas, acceptance order, conditional bound, pool, budget, assumptions, MPT tests | `CR-M1-01-STATE-PROOF-READS.md`; `annex/recv-limits.json` | Blocked on the owner (BLK-01) |
| C01 | R4-06, R4-07 | Batch 2 complete as drafted: B1, B2, B3, B5, B7. R1 brought forward in part | `annex/*.json`; `vectors/bridge-check-cases.json`, `vectors/dependency-graphs.json` | Open |
| C08 | R4-08 | Treated as approval provenance; no raw-fingerprint claim | — | Ledger: documented limitation |

## 2. Inventory delta (other rows unchanged from 0.3)

| Row | 0.3 | 0.4 | Remaining |
|---|---|---|---|
| B1 | Not started | Partial: drafted + fixtures | Codex review and execution; U35 (size unit) |
| B2 | Not started | Partial: drafted + fixtures | Review and execution; BR19a/b tables are B10 (batch 4) |
| B3 | Not started | Partial: drafted + fixtures | Review and execution; U31, U32; httpsUrl browser check (E06, Phase A) |
| B5 | Not started | Partial: drafted + fixtures | Review and execution |
| B7 | Not started | Partial: drafted + fixtures | Review and execution; U33 (fetch rule needs ESLint) |
| R1 | Not started | Partial | RecvFit for sv-fix, end/full and logs/dense; BG1 constants |
| X3 | Blocked (count 22) | Blocked (count 17; ADD-M1-01 proposed) | Owner signs ADD-M1-01 or supplies the originals |

Totals:

| Status | Count | Rows |
|---|---|---|
| Partial | 12 | C1–C4, X1, V1, B1, B2, B3, B5, B7, R1 |
| Blocked | 1 | X3 |
| Not started | 28 | the remaining rows |
| Complete | 0 | — |

The rest of the batch plan is unchanged, except that R1 has moved forward. Batch 3 is next: B4, B9, B6, B8.

## 3. Genuine blockers

| ID | Exact source | Preferred solution | Falsifiable acceptance |
|---|---|---|---|
| BLK-01 | FD:L1274 with the table FD:L1262–1273 (no `eth_getProof` row) | Approve CR-M1-01 (§11 text changes), or choose P36 (conditional detector wording) | CR §10; MPT-1…12 and E03 in Phase A |
| BLK-02 | FD:L4959 requires 17 IDs; FD:L2644 and reference/validation.md:130 only reference them; no definitions exist in reference/ | Sign ADD-M1-01 (possibly amended) as CR-M1-02, or supply the originals | Every one of the 17 IDs has an approved definition, inputs and a pass criterion, and appears as a T12 row |
| BLK-03 (new, partial) | CR-M1-01's byte bound rests on PR-1/PR-2, which the baseline does not state. FD:L935 `P(d)` is undefined and has no depth bound | The owner states the trie format (hexary MPT, keccak-hashed 32-byte keys), or accepts E03 as the confirming test | The runner checks the bound arithmetic now; E03 / MPT-9 / MPT-12 confirm it in Phase A |

Missing annex work is not a blocker. It is scheduled.

## 4. Pending owner decisions

1. **CR-M1-01:** approve, amend or reject. If rejected, P36 applies.
2. **ADD-M1-01:** sign, amend, or supply the original definitions of Q8–Q12, X1–X7 and د1–د5. This includes the mapping decisions for Q_i ↔ site i, and د versus BR23.
3. **PR-1/PR-2 trie premise** (BLK-03).
4. **U33:** accept ESLint for the FD:L3264 fetch rule. The baseline names dependency-cruiser.
5. Still open from 0.2/0.3: U01 (factory address), U02 (revoked interstitial), U10, U14 (timeout meaning), U15 (gate carve-out; the default is the full gate), U16.

Withdrawn as owner questions, per the review: batch order, the interpreter choice, the launcher repair and the derivation of the design ID.

## 5. Decisions for reviewers (new)

| ID | Question | Proposal |
|---|---|---|
| U31 | `value` typed qty (u64) in callObj and txObj | Observation only; no change proposed |
| U32 | Path for an extra params element (BR8 vs BR17d) | `params` in general; `params[2]` for eth_call and eth_estimateGas |
| U34 | BRIDGE_MSG_MAX derivation (61824 vs a count of 61790) | Explain or correct BG1's minimum; the literal limit is unchanged |
| U35 | Unit of BRIDGE_MSG_MAX | UTF-8 bytes of the message text |
| U36 | Unknown fields in an `eth_getProof` result | Ignored, pending E03 |
| U37 | `txObj.from` required at B3 | Required (the 4100 check stays in Wallet) |
