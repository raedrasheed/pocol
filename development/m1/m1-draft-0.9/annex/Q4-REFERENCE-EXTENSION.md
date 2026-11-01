# Annex Q4: BridgeRef and SiteStorageRef extension for StoreQueue

**Status: Partial.** Files: `../tools/storequeue_ref.py` (model) and `../tools/run_checks_09.py` (runner). Python standard library only.

These are specification fixture tooling, with a manual fake clock and a fake backend. They are **not** the TypeScript StoreQueue/SiteStorage of the extension, and they do not touch Chrome.

## 1. Interfaces (implementation.md:189, 204–228)

**SiteStorageRef.apply(dict, msg)** → `{decision ∈ {null, 4300}, totalBefore, totalAfter, newDict}`.
- A pure function of one dictionary and one message, with the literal `entry` and `newTotal` accounting.
- It never consults StoreQueue state.

**Admission** `admission(params, sess, glob, qbytes)` → the literal violated list, empty if admitted.

**Response decision inputs.** Each message's decision is a function of:
- the admission counters before it (`storeQBefore`);
- the confirmed dictionary at head evaluation;
- the fake clock (`acceptMs + wait`, `startMs + write`, `navigationStart + 5000`);
- the backend outcome of its `set` (settle ok or fail, and when).

**Replies.**

| Situation | Reply |
|---|---|
| admission | −32005 `{store, storeViolated}` |
| queue wait expired | −32005 `{store}` |
| quota | 4300 `{quota, limit: 1048576}` |
| write failed (transport) | −32603 `{transport}` |
| write deadline | −32603 `{storeTimeout}` (an unknown outcome, not a failure) |
| success | `null` |

**Once-only resources.** `release(msg)` is the only place that subtracts the counters and frees the seat and the lock. `releaseCount` and `doubleReleaseCount` are counted.

**Typed trace.**
- Every reply: id, frame, code, reason, `storeViolated`, delivered (frame alive) and time.
- Every state change: (reply, own) before → after, with its cause.
- Every backend call: number, message, key, written dictionary, settle outcome.
- Every snapshot: frame, keys, total, badge, time.

**Stats.** `releaseCount`, `doubleReleaseCount`, `quotaRecheckMismatch`, `backendSetCalls`, `maxActive`.

## 2. Oracle comparison: not circular

- **Golden first.** The golden checkpoints in `vectors/` are hand calculations from the baseline. The model is compared **to** them, and no expected value is regenerated from the model.
- **Provenance.** Each golden block names a source file and line range. The runner checks that the literal anchor numbers appear in that range of the current file, for example 61507, 246300, 3936448, 300000, 4109/68/67/66/66, 987133, 987137. It also records the SHA-256 of every input file. If the baseline file changes, the hashes show it.
- **Cross-checks.** SiteStorageRef is checked separately against the BR17e table (validation.md:198–206) and UTF-8 boundary vectors. The backend contents after every schedule are compared with an independent SiteStorageRef replay of the successful `set` calls. This last comparison is consistency only, not proof.
- **No proof by agreement.** Agreement between this model and a future TypeScript implementation would be **consistency only**. It is not evidence about Chrome. BR21c measures Chrome.

## 3. Fault controls (negative controls)

Each fault must be detected by a literal golden case no later than the time given. Where the baseline describes the faulty behaviour, the faulty model must show exactly those numbers.

| Fault | Meaning | Detected by (time) | Baseline-described observation |
|---|---|---|---|
| `storeUnbounded` | admission ignores the counters | BR21a-q10 (64), BR21e (5), BR21a-q1q5 (4) | accepts S17 / accepts at t=5 |
| `storeEarlyRelease` | releases the counters with the storeTimeout reply | BR21a-q8 (≤ 6000), BR21b-r6 (≤ 5100) | q8: (7, 184793) at 5000, `doubleReleaseCount` 1. r6: (1, 61507) at 5000, (0, 0) at 5100 |
| `quotaNoRelease` | 4300 sent but not released | BR21f (100), BR21f2 (0) | S_A (2, 135) after t=100; f2: S_A (1, 68) |
| `quotaAfterSeat` | quota checked only when a seat is granted | BR21f2 (0) | no immediate 4300 while the seats are held |
| `readyFifoByReadyTime` | the global ready FIFO is ordered by readiness, not acceptance | BR21f (100), BR21a-q9 (final order) | m4 would take the seat before m2 |
| `perItemKey` | the storage key comes from the item key | BR21a-q1q5 (1) | a 2nd `set` at t=1 |
| `sessionKey` | the storage key is per tab, not per site | Q2-n1 (1) | G1's write starts beside F1's |
| `keyFromMessage` | the storage key is read from a message field | Q1-key1 (1) | x2 waits behind x1 |

## 4. Mapping to the future implementation

- The TypeScript StoreQueue and SiteStorage (implementation.md:204–228) must reproduce every golden checkpoint, driven by the same schedules over FakeClock and FakeStorage (BR21a/b/e/f in Node).
- Running that, and BR21c/BR21d in Chrome, is **pending** and was not attempted.
- D103/D134 generation recovery (EpochFence, RecoveryGate, ReadSlots) and the D104 disk ledger interact with StoreQueue. They belong to rows E1–E7 and are not modelled here.

## 5. Modelled choices not stated by the baseline (P), and blockers

| ID | Topic | Choice | Citation |
|---|---|---|---|
| P-Q1-1 | lone surrogates | TextEncoder → U+FFFD, 3 bytes. The baseline says "UTF-8 of the strings after JSON decoding" and "TextEncoder"; they differ only for invalid UTF-16 | browser.md:396; implementation.md:206 |
| P-Q1-2 | delete of a missing key | accepted, total unchanged, still one `set` | implementation.md:207 |
| P-Q1-3 | BG1 in test builds | only the global-bytes bound may be violated (TQ); the others still bind | governance.md:133, 142; validation.md:348 |
| P-Q1-4 | same-millisecond order | expiries, then write deadlines, then navigator deadlines, then the scheduled event | not stated |
| P-Q1-5 | deadline after teardown | no storeTimeout reply is emitted; replyState stays suppressed | browser.md:468, 477 |
| P-Q2-1 | navigator wait scope | only the write active at navigation; a write that starts on that settle does not delay the snapshot | browser.md:478; implementation.md:225, 269 |
| P-Q2-2 | after the badge snapshot | a later settle is not pushed into the already-loaded frame | browser.md:478 |
| P-Q2-3 | closed session | its frames send nothing more; the record is kept only until its active writes settle | browser.md:479; validation.md:330 |
| P-Q3-1 | timing in extensions | the settle times and the oldest-first settle order of the extensions; BR21a q2 "t=1..3" taken literally | validation.md:297–315 |
| P-Q3-2 | f2 setup | the holding writes at t=−2 and −1, so the literal t=0 and t=1 are kept | validation.md:386–388 |
| P-Q4-1 | the f2 fault wording | validation.md:389 attributes "waits for a seat in f2" to `quotaNoRelease`; modelled as a separate `quotaAfterSeat` fault, and both are detected | validation.md:389 |

**Blockers:** none for the specification and fixtures. Rows Q1–Q4 cannot become Complete until all of these happen:
- independent execution and review of this package;
- the TypeScript implementation passes the same goldens (M2);
- BR21c and BR21d run in Chrome.
