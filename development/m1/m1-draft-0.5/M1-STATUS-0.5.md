# M1 Draft 0.5 — Dispositions, Inventory, Blockers and Pending Decisions

Read with `../m1-draft-0.4/M1-STATUS-0.4.md`, which remains in force where it is not amended here. The ledger is not edited. **No issue is closed by this document.** Nothing in 0.5 was executed by the author.

## 1. Dispositions

| Issue | Revision | Change | Evidence (unexecuted) | Proposed status |
|---|---|---|---|---|
| C14 | R5-01 | Header growth at the 65536 boundary explained level by level; the 2848 fixture is now 65561 bytes; the bound is unchanged | `vectors/request-bound-c14.json` | Drafted + fixtures |
| C15 | R5-02 | A native Node WHATWG oracle decides httpsUrl; values not in the oracle raise an error; all 0.4 bridge cases are re-run | `tools/url_oracle.cjs`, `vectors/url-cases.json` (31 + 2 cases) | Drafted + fixtures |
| C16 | R5-03 | Internal handles; frame ids are used only in replies; no rejection of repeated ids | `vectors/pending-handles.json` (PH1–PH6) | Drafted + fixtures |
| C17 | R5-04 | EIP-1186 shape; optional address with a match check; 20-byte address versus 32-byte slot hashing; the verified-absence branch comes first; empty proofs only with the empty-trie root | `CR-M1-01-REV2.md`; `vectors/proof-response-cases.json` | Drafted + decision fixtures; MPT verification is Phase A |
| C18 | R5-04 | Strict envelope and id; −32021 waits for `retryAfterMs`, ≤ 3 retries per request, ≤ 20 s waiting per load, ≤ 48 sends | Same | Drafted + fixtures |
| U34 | R5-09 | Envelope reserve of 128; guard precedence | `vectors/bridge-guard-cases.json` | Resolved (proposed) |
| B7/U33 | R5-10 | Concrete realization of the import rule and the fetch rule; no longer an owner question | `annex/import-and-fetch-rules.json` | Drafted; tool runs are in Phase D/A |

## 2. Inventory

Changes since 0.4:

| Row | 0.4 | 0.5 |
|---|---|---|
| B4 | Not started | Partial: drafted + fixtures (SiteStorage, Navigator, openExternal) |
| B6 | Not started | Partial: drafted + property checks |
| B8 | Not started | Partial: literal messages and replies, txA, RpcTap criterion. Pending: BR10 LogClient mapping (V2) and the BR14 granted account, which needs funding (P) |
| B9 | Not started | Partial: full table, generated texts, mutants |
| B3 | Partial | Partial: httpsUrl now uses the oracle |

Totals:

| Status | Count | Rows |
|---|---|---|
| Partial | 16 | C1–C4, X1, V1, B1–B9, R1 |
| Blocked | 1 | X3 |
| Not started | 24 | the remaining rows |
| Complete | 0 | — |

The next feasible batch is batch 4: B10 (BR19a/b/c, BridgeRef), B11 (BR16 generator, BridgeTrace), and X2 (signEligible). The batch order is a technical choice, not an owner question.

## 3. Unresolved blockers

| ID | Exact source | Preferred solution | Falsifiable acceptance |
|---|---|---|---|
| BLK-01 | FD:L1274 with the table FD:L1262–1273: `eth_getProof` is not listed | Approve CR-M1-01 (0.4 §1, §7–§11 together with rev 2), or choose P36 | Rev 2 §3/§6 fixtures now; MPT-1…16 and E03 in Phase A |
| BLK-02 | FD:L4959 requires 17 IDs (Q8–Q12, X1–X7, د1–د5) that have no definitions in reference/ | Sign ADD-M1-01 (0.4), possibly amended, or supply the originals | Each of the 17 has an approved definition and a T12 row |
| BLK-03 | PR-1/PR-2 (the trie format behind the proof byte bound) are not stated in the baseline; FD:L935 `P(d)` is undefined | The owner states the trie format, or accepts E03 as the confirming test | Bound arithmetic checked now; MPT-9/MPT-12 in Phase A |

## 4. Pending owner decisions (exact)

1. **CR-M1-01:** approve, amend or reject rev 2 together with the unchanged 0.4 sections. Rejection selects P36.
2. **ADD-M1-01:** sign, amend, or supply the original definitions. This includes the mapping decisions: Q_i ↔ T12 site i, and د versus BR23.
3. **Trie premise PR-1/PR-2** (BLK-03).
4. **Proposals that take baseline-level values:**
   - TK-1, the fixture-key derivation (anvil default accounts);
   - the txA fee and gas fields;
   - the −32021 limits (10000 ms per wait, 20000 ms per load);
   - `STATE_POOL` = 1 MiB.
5. Unchanged from 0.2/0.3: U01, U02, U10, U14, U15, U16.

Withdrawn as owner questions: U33/U34 (now technical resolutions R5-10 and R5-09), batch ordering, interpreter and launcher choices, and the design-ID derivation.

## 5. Reviewer-level items (new)

| ID | Question | Proposal |
|---|---|---|
| U38 | Does an accepted no-op delete write? | Yes: one write per accepted message |
| U39 | Precedence of quota versus entries | Quota first |
| U40 | Reply-code mapping for write-path exits | `annex/write-path.json` `exits` |
| U41 | Request ids for state reads | A per-load counter; replies must match them exactly |
