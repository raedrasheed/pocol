# Annex Q1–Q2: StoreQueue and SiteStorage (D96, D99, D101, D102, I54–I56)

**Status: review draft. Rows Q1 and Q2 proposed Partial.** This is an English restatement of the Arabic baseline. Where the baseline is silent, the modelled choice is tagged **P-…** and listed in `Q4-REFERENCE-EXTENSION.md` §5. No new policy is introduced.

"MUST" restates a baseline rule. Citations are `file:line` in `reference/`.

## Q1.1 Byte accounting

**UTF-8 length.** `utf8len(s)` is the length of the string encoded by `TextEncoder` (implementation.md:206), never `String.length`:
- 1–4 bytes per code point;
- a lone surrogate encodes as U+FFFD, i.e. 3 bytes (P-Q1-1);
- `null` counts 0 (browser.md:410).

**Queue size of a message** (browser.md:409–412):
- `storage_set`: `qbytes = 64 + utf8len(k) + utf8len(v)`.
- `site_storageClear`: 64.
- Largest message: 64 + 256 + 61440 = 61760.

**Quota accounting** (browser.md:395–400; implementation.md:205–208). Constant: `SITE_STORE_MAX = 1048576`.
- `entry(k, v) = utf8len(k) + utf8len(v)`.
- `total` = Σ `entry` over the stored pairs.
- **Set with a string value:** `newTotal = total − entry(k, old if present) + entry(k, v)`. Accepted if `newTotal ≤ 1048576`. Otherwise 4300 `{reason: quota, limit: 1048576}`, with no write and the dictionary unchanged byte for byte.
- **Set with `null`:** delete. Never quota-rejected. A delete of a missing key leaves `total` unchanged and is still written with one `set` (P-Q1-2).
- **Clear:** `newTotal = 0`. Never quota-rejected.
- The D104 reasons `entries`, `disk` and `sites` belong to rows E1–E7 and are out of scope here.

## Q1.2 Admission (B4)

Admission happens before append and without any write. A message is admitted only if all four conditions hold (browser.md:414–420); equality is accepted:

| Field | Condition | Default |
|---|---|---|
| `sessN` | `sessQ.n + 1 ≤ STORE_SESSION_MSGS_MAX` | 8 |
| `sessBytes` | `sessQ.bytes + qbytes ≤ STORE_SESSION_BYTES_MAX` | 262144 |
| `globN` | `globQ.n + 1 ≤ STORE_GLOBAL_MSGS_MAX` | 64 |
| `globBytes` | `globQ.bytes + qbytes ≤ STORE_GLOBAL_BYTES_MAX` | 4194304 |

Any violation gives −32005 `{reason: store}` immediately. The trace records the **literal** set of violated fields as `storeViolated` (validation.md:289), in the order of the table.

The counters include every admitted message not yet in `ownState = released`. That includes an active message already answered with `storeTimeout`.

I55 (browser.md:422): with the defaults the global byte limit cannot be reached, because 64 · 61760 = 3952640 < 4194304. The global count limit always decides first. The byte condition is therefore tested only with the test parameters TQ (BR21e).

## Q1.3 Storage key and ordering

- The logical storage key is `'site:' + netKey + ':' + address`, **derived from the frame context, never from the message** (browser.md:394, 495; implementation.md:228). Two tabs of the same site share the key. Different addresses or different `netKey`s never share.
- **Per-key FIFO** in acceptance order (`arrivalMs`, then `seq`), across tabs (browser.md:425). `set` and `clear` run in that same order (browser.md:426).
- At most **one active write per key** and at most **`STORE_ACTIVE_GLOBAL = 2`** active writes in the extension (browser.md:427–428).

## Q1.4 Head evaluation (D102, I56) and seats

**Head evaluation.** A message is evaluated when it becomes the first unreleased message of its key and the key has no active write (browser.md:431). It is evaluated at once against the confirmed `total`, without waiting for a seat and without any backend call:
- **Over quota:** reply 4300, then release in the same step (`queued → released`). Zero `set` calls, and no seat or lock is held. The next head of the key is evaluated immediately (browser.md:432).
- **Accepted** (deletes and clears always are): `ownState = ready` (browser.md:433).

**Seats.**
- Ready messages wait in a **global ready FIFO ordered by acceptance** (`arrivalMs`, then `seq`), not by when they became ready (browser.md:433; implementation.md:219).
- A seat is taken only when the write starts (`ready → active`, one `chrome.storage.local.set` of the whole dictionary). So a quota-rejected message never holds a seat (browser.md:434, 437).

**Recheck.** `set` recomputes `newTotal` defensively. Any difference from the head evaluation increments `stats.quotaRecheckMismatch`, and a non-zero value is a failure (implementation.md:212). In a correct queue it is always 0, because only the key's own writes change its `total` and they are ahead in the FIFO (browser.md:435).

## Q1.5 Timers

| Timer | Applies to | Fires at | Effect |
|---|---|---|---|
| `STORE_QUEUE_WAIT` = 10000 ms | `queued` and `ready` (browser.md:436, 441) | exactly `acceptMs + 10000` (browser.md:463) | −32005 `{store}`, release, no write |
| `STORE_WRITE_DEADLINE` = 5000 ms | `active` | exactly `startMs + 5000` | −32603 `{storeTimeout}`, once (browser.md:442, 468) |

After `storeTimeout`, `replyState = sent`, but the message **keeps** its session and global count and bytes, its seat and its key lock until the backend promise actually settles (browser.md:443, 468).

Same-millisecond order (P-Q1-4): queue-wait expiries, then write deadlines, then navigator deadlines, then the scheduled event of that millisecond.

## Q1.6 Settle

`release(msg)` is the single place that subtracts the counters and frees the seat and the lock. It runs exactly once per admitted message (browser.md:457). `stats()` reports `releaseCount` and `doubleReleaseCount`; a non-zero `doubleReleaseCount` is a failure (implementation.md:222).

**Settle before the deadline** (browser.md:438, 467):
1. Update the dictionary and `total` (on failure: reload from storage).
2. Reply `null` (or −32603 `{transport}` on failure).
3. Release.
4. Evaluate the key's next head and pump the seats.

**Settle after `storeTimeout` or after teardown** (browser.md:444, 469): reload the dictionary from storage and recompute `total`, then release, with **no reply**. Only then may the key's next write start, so commands are never reordered (browser.md:445).

**A failed or late settle** always reloads the confirmed backend state before releasing or pumping. A failed write leaves the bytes of that message out of the confirmed dictionary (BR21a q7).

## Q1.7 Parameters and BG1

The production defaults are immutable:

| Parameter | Default |
|---|---|
| `sessMsgs` | 8 |
| `sessBytes` | 262144 |
| `globMsgs` | 64 |
| `globBytes` | 4194304 |
| `active` | 2 |
| `wait` | 10000 |
| `write` | 5000 |

BG1 (governance.md:132–133) requires:
- `sessBytes ≥ 61760`;
- `globBytes ≥ globMsgs · 61760`;
- `globMsgs ≥ sessMsgs ≥ 1`;
- `active ≥ 1`;
- `wait > write`.

TQ (validation.md:347) is the defaults with `globBytes = 300000`. It deliberately violates the second condition and is accepted **only in a test build**. A release build with any non-default StoreQueue value fails (governance.md:142). P-Q1-3: in a test build, every other BG1 condition is still enforced.

## Q2 Teardown, session end and the wait before the snapshot (D99, D101)

**Cancellation.** `cancelFrame(frame)` (implementation.md:224; browser.md:476–477):
- The frame's `queued` and `ready` messages: `replyState = suppressed` (no reply ever), released at once. The key's next head is then evaluated, and it may belong to another tab of the same site.
- The frame's `active` message **cannot** be cancelled or released early. Its counters, seat and lock stay until settle. If it has not been answered, `replyState = suppressed`. If it was already answered with `storeTimeout`, it stays `sent` and the counters do not change (browser.md:470).
- Replies are addressed to the originating frame. **A reply for an old frame never reaches the new frame** (browser.md:477).

**Navigator** (implementation.md:269; browser.md:478):
1. `cancelFrame(old)`.
2. Then wait for the key's active write until it settles or until `navigationStart + 5000`, whichever comes first.
   - **Settle first:** after the reload and the release, the new frame's snapshot is the confirmed dictionary, with no badge.
   - **Deadline first:** the snapshot is the confirmed state at that moment, with the badge «حالة الحفظ غير مؤكدة» (uncertain save state). The write stays counted until it settles, then one release with no reply.
   - **No active write:** the snapshot is taken immediately.
3. P-Q2-1: the wait covers the write that was active at navigation time only. Another tab's write that starts at that settle does not delay the snapshot.

**Session end (tab close)** follows the same rule (browser.md:479). The session record is kept, accepting nothing, until its active writes settle, and is then deleted (validation.md:330). P-Q2-3: a closed session's frames send nothing further.

**Multiple frames on one storage key.** Each tab is its own session with its own counters but shares the key's FIFO, active write and dictionary. Cancelling one frame touches only that frame's waiting messages (case Q2-n1).
