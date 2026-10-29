# Annex E4–E5: RecoveryGate, slots, SeqAlloc, fmt 2, DiskLedger; BR22d–g

**Status: Partial (proposed).** This is an English restatement of browser.md:533–713, 716–779 and validation.md:437–509. Model choices the baseline does not fix are tagged **P-E4-n / P-E5-n**. Proposed baseline changes are tagged **CR-E4-n**; they are requests for review or owner decision, **not approved**.

## 1. RecoveryGate state machine (D104, D110; browser.md:544–593)

There is one gate per (key, generation), with one shared recovery operation. Every snapshot and every `storage_set` from any tab waits on it, and nothing starts a second one.

| From | Event | To | Effects |
|---|---|---|---|
| unrecovered | first access (snapshot or set) | recovering | new `gateId`; `t_s = now`; total read deadline `t_s + 5000` starts |
| recovering (read) | getKeys / get results | recovering (read) | each read holds a ReadSlot until **its own** settle; up to 4 `get`s descending, skipping corrupt (`recoveryCorrupt++`) |
| recovering (read) | no record | recovering (slot 1) | dictionary `{}` |
| recovering (read) | corrupt with no valid record within 4 gets | **failed** {corrupt} | messages −32603 `{storeRecovery, corrupt}`; no issue, no alloc |
| recovering (read) | read deadline (`t_s + 5000`) | **unrecovered** {readTimeout} | messages −32603 `{storeRecovery, readTimeout}`, released; no alloc, set or ticket; queued reads withdrawn; **issued reads stay owned until they settle** |
| recovering (slot 1) | no RecoverySlot within 5000 | **unrecovered** {busy} | `recoveryBusy++`; messages −32603 `{storeRecovery, busy}`; no alloc |
| recovering | slot granted | recovering (attempt n issued) | one synchronous step: guard (current gateId, unresolved), then DiskLedger.reserve, then `seq = alloc()`, then issue `(E, seq)` with the same dictionary; 5000 ms deadline |
| recovering | disk rejected | recovering | no issue; slot freed in the same step; the attempt counts as failed (attempt 1 then requests attempt 2) |
| recovering | attempt 1 fails or passes its deadline | recovering | request attempt 2 (fresh alloc at issue); attempt 1 keeps its slot and ticket until it settles |
| recovering | attempt-2 slot wait (5000) expires | per below | counts as attempt 2's deadline |
| recovering | first successful settle of any attempt | **ready** | `confirmed` = that version; the pending attempt-2 request is withdrawn; snapshots served; lower records removed |
| recovering | every attempt failed or expired | **failed** | messages −32603 `{storeRecovery}`; no snapshot in this generation |
| ready | later successful settle of a larger version | ready | `confirmed` = max (monotonic); the dictionary is unchanged |
| ready or failed | late success or late apply of a smaller version | unchanged | `lateRecordsSeen++`; **failed never becomes ready** |
| unrecovered | next access | recovering | new `gateId`; late read results of an old gate only settle their slot (`lateReadsDropped++`) |

**Bound:** read 5000 + 2·(slot 5000 + checkpoint 5000) = **25000 ms** from `t_s`, assuming no promise settles (browser.md:575–576, 584).

**Messages** waiting during recovery remain subject to `STORE_QUEUE_WAIT = 10 s`, so they may get −32005 `{store}` first (browser.md:577).

## 2. ReadSlots and RecoverySlots

| Pool | Size | Order | Release | Notes |
|---|---|---|---|---|
| ReadSlots (D110) | 2, extension-wide | global ReadFIFO | settle of that read only | deadline does not release; late result: slot only + `lateReadsDropped` |
| RecoverySlots (D109) | 2, separate from `STORE_ACTIVE_GLOBAL` and AdminSlots | global RecoveryFIFO by request | settle of the checkpoint promise only (or disk rejection before issue) | wait ≤ 5000 ms; a withdrawn request is skipped; a stale grant is returned with `recoveryStaleDropped++` |

**Lemma** (browser.md:583): unsettled checkpoints ≤ 2 at every moment, including the moment of death.

## 3. SeqAlloc (D104; browser.md:536–540)

- **One allocator per (key, generation)**, starting at 0. `alloc()` returns the current value and increments it **before** any set is issued: checkpoint, retry or data.
- **A `seq` is never reused**, even after a rejected or expired promise.
- On exhaustion (above 2^32 − 1): −32603 `{storeRecovery}` without a set.

## 4. fmt-2 record codec (browser.md:534–535) and limits

**Value:** `{fmt: 2, epoch, seq, tomb, b64}`.

**`b64`:** standard padded base64 of `[be32(len k) ‖ k ‖ be32(len v) ‖ v]*`.
- Keys and values are UTF-8.
- Pairs are sorted by the **key bytes**, which is not JS UTF-16 order; see the known answer `utf8-byte-order-not-utf16`.

**Decoder rejects (→ corrupt):**
- non-canonical or garbage base64;
- truncation or trailing bytes;
- duplicate or unsorted keys;
- invalid UTF-8 (overlong forms, encoded surrogates);
- a key over 256 B or a value over 61440 B;
- more than 4096 entries, or Σ(k + v) over 1048576;
- `fields` not exactly {fmt, epoch, seq, tomb, b64};
- `fmt` not exactly the integer 2;
- `epoch` or `seq` not exact integers equal to the name's version;
- `tomb` not a boolean;
- a tomb carrying pairs.

**Limits:**
- RECORD_BYTES_MAX = 4·ceil((1048576 + 8·4096)/3) + 256 = **1442048**.
- The maximum pair payload is 1081344 bytes, which base64-encodes to 1441792.

**Size metric (P-E4-3).** `enc = UTF-8 bytes(name) + compact JSON(value)` is a model metric, not Chrome's `getBytesInUse` accounting. For the maximum record with the largest JS-safe epoch, seq = 2^32 − 1 and a 66-character netKey (P-E4-4), enc = 1442007 ≤ 1442048. This holds for netKey length ≤ 107.

**Open item A: u64 representation (P-E4-1, proposed CR-E4-01).**
- The baseline epoch range is [1, 2^64 − 1] (browser.md:539).
- A JSON number read by JS is exact only up to 2^53 − 1. The value field's representation is not specified.
- This reference assumes JSON numbers and rejects value epochs above 2^53 − 1 (`epochNotJsSafe`). The name keeps the full hex16.
- Alternatives for review: a hex16 string in the value, a BigInt-aware decoder, or a declared cap of 2^53 − 1 on epoch exhaustion.
- Python big integers prove nothing about Chrome Number behaviour.

**Open item B: unpaired surrogates (P-E4-2, proposed CR-E4-02).**
- Bridge quota accounting uses TextEncoder (implementation.md:206): a lone surrogate counts as U+FFFD, 3 bytes.
- Persistence is UTF-8. Two distinct JS keys `'\ud800'` and `'\ud801'` both encode to `EF BF BD`.
- So a round trip cannot preserve them. This was observed by the root in native Node; vector `surrogateCollision`.
- The reference does **not** normalize or drop anything: `serialize_pairs` raises `unpairedSurrogate`.
- **Proposed narrow reconciliation, for review:** validate Unicode scalar values (no lone surrogates) for keys and values at the bridge, before quota accounting, with a defined rejection (for example −32602 on `params[0]`/`params[1]`). This needs a baseline change and is not assumed approved.
- Codec claims here cover **scalar-value input only**. All-input coverage stays unresolved.

## 5. DiskLedger (D105; browser.md:632–653)

**Admission.** `reserve(op)` is synchronous: check and add in one step, with no `await`.

| Dimension | Rule | Default threshold |
|---|---|---|
| Bytes (all sets except a tomb) | `inUse + unres + enc + LATE_RESERVE + META_RESERVE ≤ 256 MiB` | `inUse + unres + enc ≤ 244314112` |
| Names | `names + unresN + 1 + LATE_NAMES + EPOCH_NAMES_MAX ≤ 1024` | `names + unresN ≤ 991` |

- A tomb's bytes are counted under META_RESERVE instead.
- A remove gets a zero ticket.
- A breach gives 4300 `{disk, limit}` before any set.

**Ticket lifetime.**
- On settle, a ticket gets a `settleSeq` and stays counted.
- It is dropped only when a refresh **issued after** that settle completes and its measurement is adopted in the same step.
- Refreshes are serialized.

**Constants.**
- LATE_RESERVE = 4·(2+2)·1442048 = 23072768.
- LATE_NAMES = 4·(2+2+2) = 24.
- EPOCH_NAMES_MAX = 8.
- META_RESERVE = 1 MiB, which is ≥ 531456.

**Deferred.** Sites, pins and TombReaper (D112/D113) are E7. AdminDelete, AdminSlots and tombs (D106/D108/D111) are E6. Only a narrow tomb adapter is used here for BR22f-f4 and BR22g-g5. **No full D104 coverage is claimed.**

## 6. BR22 canonical tables (vectors: `e5-schedules.json`)

Notation: set#n is the n-th backend set; versions are (E, seq) with E = 2.

| Case | Literal source | Key cells (hand derived) |
|---|---|---|
| BR22d | validation.md:437–447 | 0: C0 = (2,0), set = 1. 1: T2 set + snapshot wait on the same gate. 5000: C1 = (2,1), set = 2, C0 still pending. 5100: settle C1 → ready (2,1); snapshots T1 and T2 = {k0:'a'}; W = (2,2) {k0:'a', k1:'x'}, set = 3. 5200: `null`; sweep leaves {(2,2)}. 5300: late C0 → `lateRecordsSeen` = 1. 5400: new snapshot {k0:'a', k1:'x'} |
| V-a | :449 | apply C0 at 5050 → same table, ready (2,1) at 5100, `lateRecordsSeen` = 0 |
| V-b | :450–454 | settle C0 at 5050 → ready (2,0), snapshots at 5050, W = (2,2) at 5050. 5100: confirmed (2,1), dictionary unchanged. 5200: `null` |
| V-c | :455 | settle C0 ok at 5250 → no change, `lateRecordsSeen` = 1 |
| V-d | :456–458 | C1 fails at 5100 → failed; m → −32603 `{storeRecovery}`. C0 ok at 5150 → still failed, `lateRecordsSeen` = 1; (2,0) is the largest record |
| V-e | :459 | no settle → failed at 10000 |
| fail | :461 | C1 fails at 5100 → failed, zero snapshots |
| BR22e | :465–467 | SeqAlloc from 2^32−2: checkpoint `…:fffffffe`, W `…:ffffffff` → `null`; the next write → −32603 `{storeRecovery}`, no set |
| BR22f | :468–478 | 0/1: C0_K1, C0_K2 on slots A/B (held 1, 2). 2: K3 waits. 5000/5001: C1 requests. 5002: K3 busy, `recoveryBusy` = 1, no alloc. 10000/10001: K1/K2 failed. held = 2, set = 2. Death at 13000 with late apply |
| f2 | :479 | C0_K1 fails at 7000 → C1_K1 = (2,1) on slot A, deadline 12000. K2 failed at 10001. K1 failed at 12000 |
| f3 | :480 | C0_K1 ok at 6000 → K1 ready, C1_K1 withdrawn, slot A → C1_K2 = (2,1) at 6000 |
| f4 | :481 | deleteSite(K1) at 3000 → tomb only after resolution at 10000: tomb (2,1). **E6 adapter** |
| g1 | :486–490 | readTimeout at 5000 (no alloc, set or ticket). Late getKeys at 6000 → `lateReadsDropped` = 1, R1 freed. 6100: gateId 2 |
| g2 | :491 | get held; timeout at 5000; the result at 7000 issues nothing |
| g3 | :492 | K1/K2 on R1/R2, K3 in ReadFIFO; all time out at 5000; `readSlotsHeld` = 2 throughout; K3 never reads |
| g4 | :493–496 | 4 corrupt of 5 → failed {corrupt} after exactly 4 gets (5 reads with getKeys). All-3-corrupt → failed. Zero records → checkpoint {} → ready |
| g5 | :497–501 | delete at 1000; readTimeout at 5000; tomb (2,0); the result at 6000 is ignored; deletion resolved at 5100 ≤ 45 s. **E6 adapter** |
| g6 | :502 | K's reads end at 4900 → slot request at 4900. Slots free at 9899 and 19898 → C0_K = (2,0) at 9899, C1_K = (2,1) at 19898 → failed at 24898 ≤ 24900 |

**Controls.**
- Baseline: `parallelRecovery` (BR22d at t=1), `seqFromCheckpoint` (W = (2,1) collides), `recoveryNoSlot` (unsettled = 4 in BR22f), `lateReadCheckpoint` (g1 at 6000).
- Author: `slotReleaseOnTimeout` (BR22f), `readSlotReleaseOnTimeout` (g3), `staleReadyAfterFailed` (V-d), `seqReuseOnFail` (f2).

## 7. Model conventions and findings (P-E5)

| ID | Choice |
|---|---|
| P-E5-1 | Same-millisecond order: apply/drop, death, settles, recovery timers, queue-wait expiry, requests. So at 10000 in BR22f, K1's failure precedes m1's queue-wait expiry, and m1 gets `{storeRecovery}` |
| P-E5-2 | In BR22d/e/f, reads settle synchronously. Only BR22g holds reads ("FakeBackend holds getKeys and get", validation.md:485) |
| P-E5-3 | In BR22f each key's pending `storage_set` arrives with its snapshot (t = 0/1/2) |
| P-E5-4 | A getKeys result lists the names present when the read settles |
| P-E5-5 | g2 "getKeys succeeds immediately" is settle at t = 1 |
| P-E5-6 | The g6 slot releases at 9899 and 19898 are chosen to exhibit the bound without same-millisecond ties |
| P-E5-7 | Removes settle immediately, except in BR22d (all ops held), where they settle at 5200 |

**Finding RF-E5-1.** browser.md:565 removes lower records right after the checkpoint succeeds. BR22d lists the removal of (1,3) at 5200. The reference issues the remove at 5100 and lets the BR22d FakeBackend apply it at 5200, which is consistent with both texts.

**Finding RF-E5-2.** "lateRecordsSeen counts records that appeared after the checkpoint with a lower version" (browser.md:779), but V-d also increments it after `failed`. The reference counts late successes and late applies after resolution when the version is below `confirmed`, or when the gate failed.
