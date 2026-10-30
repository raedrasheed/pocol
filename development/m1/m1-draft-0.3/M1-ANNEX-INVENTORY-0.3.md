# M1-spec Coverage and Dependency Inventory (draft 0.3)

Supersedes nothing. `../m1-draft-0.2/M1-ANNEX-CHECKLIST-0.2.md` is unchanged. This inventory re-derives the same 41 rows (C1–C4 plus 37 annex rows) from the baseline and adds three things for each row: its defining source lines, its dependencies, and its planned batch. The acceptance rule is R3-08 in `M1-SPEC-0.3-AMENDMENTS.md`. The full gate applies (U15 default); there is no carve-out.

Line numbers are those of `reference/FINAL_DESIGN.md` (FD:Lnnn). "Req" is the line in the M1-spec list (FD:L4955–5020) that requires the item. "Defined at" lists the lines whose content the item must specify. The section anchors were verified in this turn. Where only a section start is given, the exact sub-range is pinned when the batch is written. That is marked "(section)".

## 1. Status vocabulary

- **Not started:** no content in any draft.
- **Partial:** some content exists. "Remaining" says what is missing.
- **Blocked:** a genuine blocker exists (`M1-OPEN-0.3.md`). Missing work alone is not a blocker.
- **Complete:** all five conditions of R3-08 are met. No row qualifies yet.

## 2. Inventory

### Core (FD:L4957–4958)

| # | Item | Defined at | Status | Remaining | Depends on |
|---|---|---|---|---|---|
| C1 | Initcode, address formula, 3 vectors | FD:L884–889 | Partial | E05 (K1–K3 agreement); U01 | M0 Keccak libraries |
| C2 | Manifest encoding and normalization | FD:L891–898, FD:L907 | Partial (0.2, plus R3-01, R3-02, R3-05) | U04, U12, U13, U17, U20 decisions; the 0.3 checks executed and reviewed | — |
| C3 | Website slots and permissions | FD:L899–905, FD:L913–915 | Partial (0.2, plus R3-05, R3-06) | U06–U11, U22; the snapshot read path (BLK-01) | R1 (RECV_LIMIT table) for the read path |
| C4 | T1 and T16 with literal inputs | FD:L907, FD:L4958 | Partial (0.2, plus R3-03, R3-06) | T1 rows decided; T16 fixtures reviewed and E05 | C2, C3 |

### Extension annex (FD:L4959)

| # | Item | Defined at | Status | Batch | Depends on |
|---|---|---|---|---|---|
| X1 | netKey with vector (777001, 0x11×32) | FD:L981 | **Partial (R3-07):** literal preimage; hash recorded by the runner, not asserted | 1 (done in part) / 4 | E05 for the asserted hash |
| X2 | signEligible | FD:L1321–1325; profile acceptance FD:L971–984; RP window FD:L985–1036 | Not started | 4 | X1, V1 |
| X3 | Q8–Q12, X1–X7, d1–d5 | **Not defined in reference/** (only referenced at FD:L2644 and FD:L4959) | **Blocked (BLK-02)** | after the owner answers | owner |

### Bridge authorization annex (FD:L4960–4971; section FD:L1106–1231)

| # | Item | Defined at | Status | Batch | Depends on |
|---|---|---|---|---|---|
| B1 | Envelope, Bpre, B0–B4 order, error codes and data, returned-id rule | FD:L1106–1146 (section; Bpre at L1127), errors FD:L1225–1228; `check`/`parseStrict` FD:L4858–4859 | Not started | 2 | — |
| B2 | Integer bucket formula; arrivalMs; pendingReads events | FD:L1127–1146, FD:L1196–1197; RpcReadClient FD:L4862; constants BG1 FD:L2608 | Not started | 2 | B1 |
| B3 | (kind, method) matrix with machine-readable schemas; str, pathStr, httpsUrl | FD:L1106–1205 (section); FD:L1178; deny list FD:L1202–1205 | Not started | 2 | B1 |
| B4 | SiteStorage semantics; Navigator semantics | FD:L1356–1369; Navigator FD:L4925–4929; FD:L1193–1198 | Not started (Navigator normalization partly in 0.2 §6.3) | 3 | B3 |
| B5 | Explicitly tested deny list | FD:L1202–1205 | Not started | 2 | B3 |
| B6 | Flow diagram of the single write path to WalletSubmit | FD:L1207–1213 | Not started | 3 | B3 |
| B7 | dependency-cruiser rule text | FD:L1218, BR-dep FD:L3262 | Not started | 2 | — |
| B8 | BR1–BR14, BR17–BR18 byte-exact with replies (BR7a–BR7h); txA and its hash; RpcTap commands; "zero request" | FD:L2649–2735 (BR1 L2670, BR7a L2677, BR13 L2690, BR14 L2691, BR17 L2706, BR18 L2726); txA FD:L2654; RpcTap FD:L4843 | Not started | 3 | B1–B5. txA needs secp256k1 signing in the spec tooling (computable, no EVM) |
| B9 | Full BR17e table (24 messages) | FD:L2711–2721; derivation constraint FD:L2357 | Not started | 3 | B4 |
| B10 | BR19a (a1)–(a7), BR19b (b1)–(b5), BR19c, BridgeRef and its decision rule | FD:L2737–2759; BridgeRef FD:L2659, FD:L4848 | Not started | 4 | B2 |
| B11 | BR16 generator and seed; SchemaRef, BridgeRef, SiteStorageRef; BridgeTrace format | BR16 FD:L2697; refs FD:L4848; BridgeTrace FD:L1220–1223 | Not started | 4 | B3, B4, B10 |

### D98 annex (FD:L4972–4976; section FD:L1232–1289)

| # | Item | Defined at | Status | Batch | Depends on |
|---|---|---|---|---|---|
| R1 | RECV_LIMIT table and derivation; exact/capped; WorstLegit, RecvFit | FD:L1253–1276; RecvFit FD:L979; constraint FD:L2359–2361 | Not started. **Needed by C06/BLK-01** | 5 | genesis parameters FD:L2237 (section) |
| R2 | MR6b–MR6d, CAP1, RF1–RF4, MR10a–MR10f | FD:L3266–3331 (MR6b L3287, CAP1 L3290, RF1 L3295, MR10 L3302) | Not started | 5 | R1, R3 |
| R3 | Read steps 1–6 and the pools | FD:L1236–1288 | Not started | 5 | — |
| R4 | MaliciousHttp scripts MR1–MR12, byte-exact | FD:L3266–3331 (section); MaliciousHttp FD:L4943 | Not started | 5 | R1–R3 |

### D99 annex (FD:L4977–4982; section FD:L1290–1320)

| # | Item | Defined at | Status | Batch | Depends on |
|---|---|---|---|---|---|
| S1 | SiteSession lifecycle; orphan request states | FD:L1290–1306; FD:L4930–4932 | Not started | 6 | B2, R3 |
| S2 | Navigation bucket formula | FD:L1196–1197, FD:L1316 | Not started | 6 | B2 |
| S3 | Corrected BR20a table; BR20b; BR20c script | FD:L2763–2799 (BR20a L2763, BR20b L2779, BR20c L2783) | Not started | 6 | S1, S2 |
| S4 | BridgeRef session extension | FD:L2659, FD:L4980 | Not started | 6 | B10, S3 |

### D101 annex (FD:L4983–4987; section FD:L1370–1459)

| # | Item | Defined at | Status | Batch | Depends on |
|---|---|---|---|---|---|
| Q1 | qbytes, four counters, per-key FIFO, global active limit, two timeouts | FD:L1370–1459 (qbytes L1373, limits L1379–1385) | Not started | 7 | B4 |
| Q2 | Teardown and wait-before-snapshot | FD:L1370–1459 (section); Navigator step 3 FD:L4928 | Not started | 7 | Q1, S1 |
| Q3 | BR21a (q1)–(q10), BR21b (r1)–(r6), BR21e, BR21f/(f2); TQ; BR21c/d; replyState × ownState | FD:L2800–2904 (BR21a L2811, BR21b L2839, BR21e L2860, BR21f L2873); D102 FD:L4882 | Not started | 7 | Q1, Q2 |
| Q4 | BridgeRef/SiteStorageRef StoreQueue extension | FD:L4987 | Not started | 7 | Q3, B11 |

### D103 annex incl. D134, and D104 annex (FD:L4988–5016; sections FD:L1460–1743)

| # | Item | Defined at | Status | Batch | Depends on |
|---|---|---|---|---|---|
| E1 | Epoch item and record names, values, largest-wins | FD:L1460–1506 (fence L1470, record L1496) | Not started | 8 | Q1 |
| E2 | D134: shared name per E, name gate, post-T_LATE sweep, lemmas, BR22h (h1–h6), SR-model | FD:L3024–3065 (BR22h); FD:L1597–1599; FD:L2370–2371 | Not started | 8 | E1 |
| E3 | Fence and recovery steps; two lemmas; viewer on loss; BR22a; BR22b/c | FD:L1470–1506; BR22a FD:L2905; BR22b FD:L2936 | Not started | 8 | E1, E2 |
| E4 | D104: RecoveryGate, SeqAlloc, fmt 2, RECORD_BYTES_MAX, disk acceptance, generation window, sweep, tombstone | FD:L1507–1740; RecoveryGate API FD:L4894 | Not started | 9 | E3 |
| E5 | BR22d (V-a..V-e), BR22e, BR22f (f2–f4), BR22g (g1–g6), 25 s derivation | FD:L2951–3023 (BR22d L2951, BR22e L2979, BR22f L2982, BR22g L2998) | Not started | 9 | E4 |
| E6 | BR23 (d1–d11, d11b), BR24, AdminDelete/DiskLedger/AdminSlots; LATE_NAMES, EPOCH_NAMES_MAX, META_RESERVE, 45 s; BR24 (x9–x12, x12b); BR22f-f5 | BR23 FD:L3066–3196; BR24 FD:L3197–3261; constants FD:L2270 | Not started | 9 | E4, E5 |
| E7 | DeleteOp (D111); D112 sitesLive; D113 tomb rules; D114 BR23 (d15c) with its literal replies | FD:L5005–5016; FD:L3066–3261 (section) | Not started | 10 | E6 |

### Other required vectors (FD:L5017–5020)

| # | Item | Defined at | Status | Batch | Depends on |
|---|---|---|---|---|---|
| V1 | HeaderNetCheck vectors | FD:L5017; RW-unit FD:L2646; HC-unit FD:L2647; window FD:L985–1036; NX-V3e FD:L3589 | **Partial (R3-07):** RW1–RW6 and HC1–HC7 transcribed with checks | 1 (in part) / 11 | M3-TS model (FD:L4951) for NX-V3* |
| V2 | LogClient vectors | FD:L5018; LogClient FD:L1037–1105; LC-unit FD:L3332–3376 (LC1 L3344, LC10 L3353) | Not started | 11 | R1 (8 MiB class) |
| V3 | NetworkProfiles.validate: GSV1, N1, N6, N11 | FD:L5019; GSV1 FD:L2578–2589; N1–N11 FD:L2591 | Not started | 11 | M3-TS model and K1–K3 (FD:L2589) |
| V4 | X8: pages C1–C8, flags K0–K5 | FD:L5020; FD:L2645, FD:L1353 | Not started | 11 | — |

### Summary

| Status | Count | Rows |
|---|---|---|
| Partial | 6 | C1–C4, X1, V1 |
| Blocked | 1 | X3 (BLK-02) |
| Not started | 34 | the remaining annex rows |
| Complete | 0 | — |

Compared with 0.2 (4 partial, 37 not started): X1 and V1 moved to Partial and X3 to Blocked. No row is Complete.

## 3. Batch 1 content (this turn; partial)

- **X1** (`vectors/annex-batch1.json`): the literal 51-byte RLP preimage. Its byte layout is `f2 | 8c "PoCol-net-v1" | 83 0b db 29 | a0 11…11`.
  - The chainId integer encoding is labelled P; the baseline gives the formula, not the integer encoding.
  - The runner computes and records the netKey. It is not asserted until E05.
- **V1, partial:** RW1–RW6 with the exhaustive property over h ∈ [1, 10⁶]; HC1–HC7 with the formulas, HC4 minWork = 4095, HC7 recorded as a literal decimal, and the 10⁴-sample property with seed 20261007. The message texts and the window vectors are batch 11.

Not executed in this turn (see `README.md`).

## 4. Batch plan

Batches are sequential, and each is one reviewable unit: spec text, literal fixtures, executed checks and a Codex review. The order follows the dependencies in §2 and the baseline's own build order inside M2 (FD:L4949: bucket and authorization first, then SiteStorage with BR17e, then WalletSubmit, Navigator and the rest of BR).

| Batch | Rows | Why at this point |
|---|---|---|
| 1 | X1 (preimage), V1 (RW/HC) | Pure transcriptions; no dependency (done in part) |
| 2 | B1, B2, B3, B5, B7 | The foundation of every bridge row (FD:L4949) |
| 3 | B4, B9, B6, B8 | SiteStorage + BR17e, then the write path and the literal BR messages; txA signing is in the spec tooling |
| 4 | B10, B11, X2, X1 completion | BridgeRef and the generators; signEligible needs X1 and the window |
| 5 | R1, R3, R2, R4 | RECV_LIMIT is also needed to resolve BLK-01 (C06). **Owner may move R1 earlier** |
| 6 | S1–S4 | Needs B2 and R3 |
| 7 | Q1–Q4 | Needs B4 and S1 |
| 8 | E1–E3 (D103, D134 first, as FD:L4990 requires) | Needs Q1 |
| 9 | E4–E6 (D104) | Needs E3 |
| 10 | E7 (D111–D114) | Needs E6 |
| 11 | V1 completion, V2, V3, V4 | V3 needs the M3-TS model (FD:L4951) and K1–K3 |
| — | X3 | After the owner supplies the definitions (BLK-02) |

## 5. Separation from implementation experiments

Per R3-08: E01–E04, E06 and E07 are not M1-spec acceptance conditions. They gate implementation of the affected unit, and their results may reopen the spec. E05 (K1–K3) gates the freeze of every fixture that contains a hash, so it applies to C1, C4, X1 and V3 at acceptance time. U05 needs a **decision** for M1-spec acceptance (CR-M1-01 accepted or rejected, BLK-01). The E03 outcome is an implementation gate.
