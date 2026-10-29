# Annex E3: fence → recovery → checkpoint; viewer behaviour; BR22a/b/c

**Status: Partial.** Source: browser.md:544–624; validation.md:390–436.

## 1. Sequence per generation and key (browser.md:544–566)

1. Boot. Then the fence (annex E2), with no data, session or snapshot before confirmation.
2. **First access** to a key, a snapshot or a `storage_set` from any tab: `unrecovered → recovering`.
   - **One** shared RecoveryGate per key per generation. Every snapshot and every set waits on it, and nothing starts a second one.
   - The single gate prevents two parallel checkpoints (browser.md:605).
3. **Read:** list the key's records and read the largest valid one (annex E1). The dictionary read becomes the checkpoint's dictionary.
4. **Checkpoint:** `s_c = SeqAlloc.alloc()`, then `set (E, s_c)` with **the same dictionary**.
5. **On a successful settle:** `ready`, with `confirmed = (E, s_c)`; then remove the lower records.
   - **Snapshots are given only after this successful checkpoint settle** (browser.md:567–569).
   - Later writes take their `seq` from the same SeqAlloc.
6. **Data `set`:** a new dictionary written at `(E, alloc)`. On success: reply `null`, then remove the lower records.

E4/E5 parts **not modelled here**, recorded as boundaries in `E-REFERENCE-AND-FINDINGS.md`:
- `gateId`;
- the read deadline and ReadSlots;
- RecoverySlots;
- the second checkpoint attempt;
- `busy` / `readTimeout` / `failed` outcomes;
- the DiskLedger reservation;
- the 25 s bound.

In the E3 schedules the read is atomic and the single checkpoint attempt settles successfully, unless its generation dies.

## 2. Lemmas (restated)

- **Resolution** (browser.md:596–600). Every write of an earlier generation has a version below `(E, 0)`.
  - If it is applied before the recovery read, it becomes the checkpoint's basis.
  - Otherwise it stays below every later record and never surfaces.
  - So its effect is decided once and never flips. A write answered `null` was committed before the reply and is never lost.
- **Allocation** (browser.md:602–605). Each `set` takes a fresh number before it is issued, so no record name repeats. A late operation is below every later allocation.
- **Survival / GC** (browser.md:607–610).
  - A removal deletes a record r only after seeing an existing record h > r of the same key.
  - **The greatest record is never removed**, so the greatest applied record always survives, even when a remove is applied late.
  - The model checks this on every event (`greatestRecordLost`, `removedWithoutGreater`).

## 3. Memory and frames (browser.md:612–624)

**Memory.**
- A dead worker loses its queues, counters, locks, sessions and reservations.
- The new generation starts with zero counters and inherits no message ids.
- **No reply ever reaches a message of an earlier generation.**

**Frames.**
- The viewer holds a runtime port, and the worker announces `genId` in its `hello`.
- On `onDisconnect`, or when a different `genId` appears:
  - the viewer tears down every site frame at once;
  - it shows «انقطع عامل الإضافة؛ قد تكون آخر الكتابات غير محفوظة» ("the extension worker disconnected; the last writes may not have been saved") with a «إعادة التحميل» ("Reload") button.
- **There is no automatic reload.** A new session needs a user action (D99).
- The torn frame's dirty keys are lost, with that announcement, and are never re-sent from an old snapshot.
- After a reload, the snapshot comes after the key's recovery.

**Storage API assumptions** (browser.md:509, 550, 630–631; validation.md:436). `getKeys` exists and returns every present name; listing reflects the operations applied by then. `getBytesInUse` is used only by E4 (D104). Both are things BR22c is to measure. Here they are a FakeBackend that lists exactly.

## 4. BR22a (validation.md:391–421)

**Initial state.**
- Tabs T1 and T2 for S_A.
- `k0 = 'a'` confirmed as record (1,1); gen1 is alive and ready with `seq` next = 2.
- Gen1 accepts A = ['k1', 'x'] from T1.

**After the deaths.** Reload T1 and T2 by click, then T2 writes B = ['k2', 'y'], then T1 writes C = ['k1', 'z']. Each gets `null`.

**Axes.** Exact definitions are in `vectors/e3-br22a.json`.

| Axis | Values |
|---|---|
| first death | **P0** (before set), **P1** (set issued, not applied), **P2** (applied, no reply), **P3** (after the storeTimeout reply), **fail** (A's promise rejected; A' = ['k1', 'q'] then succeeds; A applied later, violating A15d on purpose) |
| second death | none; **P4** (gen2 dies while its fence write is unsettled); **P5** (gen2 dies while its checkpoint is unsettled) |
| extra death | no; yes: the recovering generation dies after its snapshots and before any data write. P-E3-3: this is the baseline's "third death before any write in the new generation" |
| late position L of each pending op (A, gen2's F, gen2's checkpoint) | L0 never; L1 before R's epoch listing; L2 after R's confirmation, before the read; L3 between the read and the checkpoint; L4 after the checkpoint; L5 after B (placed after C settles, P-E3-2) |

The runner enumerates every combination: 2 × (2 + 3·6) × (1 + 6 + 6) = 520. It records the number of distinct schedules from the actual traces.

**Expected for every combination.** These are the baseline criteria, with the rule tables derived by hand.

| Criterion | Statement |
|---|---|
| (1) | k0 = 'a' in every snapshot and at the end |
| (2) | the end holds k1 = 'z' and k2 = 'y' |
| (3) | the k1 value in the new generation's first snapshot is 'x' or absent (in the fail variant, 'q'), and it is the same in every later snapshot until C, across any extra death |
| (4) | zero replies to a message of an earlier generation, and zero duplicate replies |
| (5) | the epochs of the data-writing generations strictly increase, and no record name is issued twice |
| (6) | no snapshot before the checkpoint is confirmed |
| (7a) | the greatest record is always present after the sweeps |
| (7b) | the FakeBackend metric stays under the bounds; see the note below |

**Rule k1First (hand derived).**

| Case | k1 in the first new snapshot |
|---|---|
| fail | 'q' |
| P0 | absent |
| P2 | 'x' |
| P1 or P3, and L(A) ∈ {L1, L2}, and not (P5 and L(checkpoint₂) ∈ {L1, L2}) | 'x' |
| otherwise | absent |

**Rule E (hand derived).**

| Generation | E |
|---|---|
| gen2 | 2 |
| gen3 under P4 | 3 if gen2's F(2) applies at L1, else 2 (the shared name) |
| gen3 under P5 | 3 |
| a generation after an extra death | E(R) + 1 |

**Note on (7b).** Disk criterion (7) needs fmt-2 sizes, `RECORD_BYTES_MAX`, DiskLedger and the reserves. Those are rows E4/E6 and are an **unexecuted dependency**. The runner checks only a scoped FakeBackend metric: name count, and `len(name) + len(compact JSON value)`, against `STORE_RECORDS_MAX` and `STORE_DISK_HARD`. That is **not** D104 coverage. In the fail variant (A15d violated) no disk or site bound is claimed at all (finding RF-2).

**Negative controls.** The same combination must pass without the fault.

| Control | Combination | Must fail |
|---|---|---|
| `singleItem` | P1, L(A) = L5 | (2): A overwrites the single item after C |
| `noEpochConfirm` | P4 | (5): gen2 writes (2,0) unconfirmed; gen3 also takes E = 2 and reissues (2,0) |
| `noCheckpoint` | P1, L(A) = L3, extra death | (3): an empty first snapshot, then k1 = 'x' after the extra death |
| `reuseSeq` | fail, L(A) = L1 | (5): A' reuses (1,2), and the late A overwrites it |

## 5. Timeline conventions (P-E3-1)

Generation R boots at r:

| r + … | Event |
|---|---|
| 0 | fence issued |
| 1 | fence confirmed |
| 3 | clicks |
| 4 | read |
| 6 | checkpoint issued |
| 7 | checkpoint confirmed, snapshots |
| 9 | extra death |
| 10 / 11 | B issued / settled |
| 13 / 14 | C issued / settled |

- A new generation boots 50 ms after the previous death.
- gen1 dies at 10 (P0), 20 (P1, P2, fail) or 5020 (P3, after the storeTimeout at 5010).

## 6. BR22b (validation.md:422–427), literal schedule in `e2-fence-cases.json`

| t | Event |
|---|---|
| 0 | G1 reads max = 5 and writes F(6), which stays pending. Its write request waits for the confirmation, which never comes |
| 10 | G1 dies |
| 1000 | G2 reads 5 and writes F(6) |
| 1001 | G2 confirms. It then writes records (6,0) and (6,1) |
| 2000 | G1's F(6) applies late, so bootMs = 0 |
| 3000 | G2 dies |
| 4000 | G3 reads 6 and confirms F(7) at 4001. It writes (7,0) and (7,1) |
| 64000 | G3's sweep removes F5 and F6 and keeps F7 |

**Criterion:** zero data records from G1, and the largest epoch item is never swept.

## 7. BR22c (FUTURE, Chrome + CDP; NOT executed, no evidence claimed)

**Set-up.**
- A test user profile with a DevTools endpoint on 127.0.0.1.
- BadSite writes from two tabs.
- The tool stops the service worker through CDP at times aimed at P0, P1, P2 and P3.

**G (to be compared when executed).**
- The storage read through CDP equals the SR-model for the classified point.
- Frames are torn down and the banner appears within ≤ 1 s of `onDisconnect`.
- Zero automatic reloads.
- After the click, the dictionary equals the SR-model.

**C (validation.md:435).**
- For each of P1 and P2: if the point is not hit within **20 attempts**, the result for that point is **«inconclusive»**, with the recorded distribution of hit points. Its logic stays covered by BR22a.
- The attempt budget is per point. A hit is classified from the CDP trace (set call seen, not applied / applied, not replied).

**Measurement obligations (validation.md:436).**
- Whether Chrome applies a dead worker's operation late, and how late. This feeds A15c.
- Whether a rejected promise is ever applied later. This feeds A15d.
- Recovery time.
- Availability of `getKeys`.
- `getBytesInUse` against the size of the storage file.
- Every measurement is reported with its raw values. An unmeasurable item is reported as inconclusive, never as a pass.
