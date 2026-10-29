# M1-spec Annex Completeness Checklist (draft 0.2)

Source of the required items: baseline FD:L4955–5000 ("first reviewable and testable task: M1-spec") and the M1 milestone (FD:L4685). The full gate applies (`M1-SPEC-0.2.md` §9, U15). M1-spec is complete only when every row below is **Complete** and has been reviewed.

Status values: **Not started**, **Partial (draft 0.2)**, **Complete**. In this draft, Complete is used only where both reviewers have already reviewed the work, and nothing qualifies yet.

## Core

| # | Required item (baseline) | Status | Where | Remaining |
|---|---|---|---|---|
| C1 | Initcode bytes and address formula with three vectors: `0x61`, 24575 × `0xaa`, a specific HTML file | Partial (draft 0.2) | §1; `../vectors/chunks.json` (reproduced byte-identically) | E05 (K1–K3); U01 |
| C2 | Manifest encoding and normalization | Partial (draft 0.2) | §2, §3, §6 | U04, U12, U13, U17, U20; E06 |
| C3 | Website slots and permissions | Partial (draft 0.2) | §5 | U06–U11, U22; E04 |
| C4 | T1 and T16 with literal inputs | Partial (draft 0.2) | §8; `vectors/` | T1 not executed (E01); T16 fixtures exist but await review and E05 |

## Extension annex

| # | Required item | Status |
|---|---|---|
| X1 | netKey with the vector (777001, 0x11×32) | Not started |
| X2 | signEligible | Not started |
| X3 | Q8–Q12, X1–X7, d1–d5 | Not started |

## Bridge authorization annex (D95, D96, D97)

| # | Required item | Status |
|---|---|---|
| B1 | Envelope `{id, kind, payload}`, `payload {method, params}`; the order Bpre, B0–B4; error codes and literal data; the returned-id rule | Not started |
| B2 | Integer bucket formula (capacity, refill, cost, consumption rules); `arrivalMs`; `pendingReads` and its events | Not started |
| B3 | Complete (kind, method) matrix with machine-readable schemas; primitive types with regexes (str, pathStr, httpsUrl) | Not started |
| B4 | SiteStorage semantics (entry, total, newTotal for add/replace/delete/clear; ≤ 1048576; one atomic write; key derivation); Navigator semantics | Not started. Navigator path normalization is partly covered by §6.3 of draft 0.2 |
| B5 | The explicitly tested deny list | Not started |
| B6 | Flow diagram of the single write path to WalletSubmit | Not started |
| B7 | dependency-cruiser rule text | Not started |
| B8 | BR1–BR14, BR17–BR18 messages byte-exact with expected replies (incl. BR7a–BR7h); txA and its hash; RpcTap commands; the "zero request" criterion | Not started |
| B9 | The full BR17e table (24 messages, literal text, byte lengths, totals) | Not started |
| B10 | BR19a (a1)–(a7), BR19b (b1)–(b5), the BR19c script, the BridgeRef spec and its decision rule | Not started |
| B11 | The BR16 generator and seed; SchemaRef, BridgeRef, SiteStorageRef; the BridgeTrace format | Not started |

## D98 annex

| # | Required item | Status |
|---|---|---|
| R1 | RECV_LIMIT table and derivation (exact/capped, WorstLegit, RecvFit) | Not started |
| R2 | MR6b–MR6d, CAP1, RF1–RF4, MR10a–MR10f | Not started |
| R3 | Read steps 1–6 and the pools | Not started |
| R4 | MaliciousHttp scripts MR1–MR12, byte-exact | Not started |

## D99 annex

| # | Required item | Status |
|---|---|---|
| S1 | SiteSession lifecycle; orphan request states | Not started |
| S2 | Navigation bucket formula | Not started |
| S3 | Corrected BR20a table; BR20b; the BR20c script | Not started |
| S4 | BridgeRef session extension | Not started |

## D101 annex

| # | Required item | Status |
|---|---|---|
| Q1 | qbytes, the four counters, per-key FIFO, the global active limit, both timeouts | Not started |
| Q2 | The teardown and wait-before-snapshot rule | Not started |
| Q3 | BR21a (q1)–(q10), BR21b (r1)–(r6), BR21e, BR21f/(f2); TQ parameters; the BR21c/d scripts; the replyState × ownState diagram | Not started |
| Q4 | BridgeRef and SiteStorageRef StoreQueue extension | Not started |

## D103 annex (including D134) and D104 annex

| # | Required item | Status |
|---|---|---|
| E1 | Epoch item and record names, values, and the largest-wins rule | Not started |
| E2 | D134: the shared name per E, the name gate, the post-T_LATE sweep, the writer and name-limit lemmas, BR22h (h1–h6) with its three controls, the SR-model extension | Not started |
| E3 | Fence and recovery steps; the two lemmas; viewer behaviour on loss; the full BR22a table; BR22b/c | Not started |
| E4 | D104: the RecoveryGate state machine, SeqAlloc, fmt 2 and RECORD_BYTES_MAX, disk acceptance, the generation window, sweep, tombstone | Not started |
| E5 | BR22d (V-a..V-e), BR22e, BR22f (f2–f4) with RecoverySlots, BR22g (g1–g6) with ReadSlots/gateId and the 25 s derivation | Not started |
| E6 | BR23 (d1–d11, d11b), BR24 literal, the AdminDelete/DiskLedger/AdminSlots machines; LATE_NAMES = 24, EPOCH_NAMES_MAX = 8, META_RESERVE, the 45 s AdminDelete bound, BR24 (x9–x12, x12b), BR22f-f5 | Not started |
| E7 | DeleteOp (D111); D112 sitesLive and its rules; BR23 (d1, d9, d12–d15), BR24 (x6); D113 tomb rules; D114 BR23 (d15c) with its two paths and literal replies | Not started |

## Other required vectors

| # | Required item | Status |
|---|---|---|
| V1 | HeaderNetCheck vectors (chainId 777002, RW1–RW6, HC1–HC7, NX-V3e/f/g, …) | Not started |
| V2 | LogClient vectors (branches A/B/A', LC1–LC18, request totals, the step table) | Not started |
| V3 | NetworkProfiles.validate: GSV1, N1, N6, N11 | Not started |
| V4 | X8: pages C1–C8, flags K0–K5 | Not started |

## Summary

| Status | Count | Rows |
|---|---|---|
| Partial (draft 0.2) | 4 | C1–C4 |
| Not started | 37 | everything else |
| Complete | 0 | — |

Under the default gate (U15), no M1 unit may enter implementation.
