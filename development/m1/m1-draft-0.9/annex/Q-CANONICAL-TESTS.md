# Annex Q3: canonical StoreQueue tests (BR21a–BR21f)

**Status: Partial.**
- BR21a, BR21b, BR21e and BR21f are restored with their literal numbers. Their golden checkpoints are in `../vectors/storequeue-cases.json` and run against the Python reference model with a fake clock and a holding fake backend.
- **BR21c and BR21d remain executable-test SPECIFICATIONS. They have not been executed and there is no evidence for them.**

The golden numbers were written by hand from validation.md:286–389. They were not produced by the model. The runner also checks that the literal anchor numbers of each case appear in the cited source lines.

Notation:
- `M(kNN) = ['kNN', 61440 × 'a']`, `qbytes` 61507.
- `m(kNN) = ['kNN', 'x']`, `qbytes` 68.
- `(n, bytes)` are counters after the event.
- `settle(i)` / `fail(i)` resolve the i-th backend `set` call.

## Common recorded fields (validation.md:286–296)

Per `storage_set`, the trace records:
- `storeQBefore {sessN, sessBytes, globN, globBytes}`;
- `storeRoute ∈ {queued, store, cancelled, active, done}`;
- `storeViolated` (literal);
- reply and release events, with the counters after each.

Per run, the reference records:
- `releaseCount`, `doubleReleaseCount`, `quotaRecheckMismatch`, `backendSetCalls`, `maxActive`;
- the dictionary and `total` of every key;
- the counters after every event.

**Criterion G:**
- every admitted message has exactly one release and at most one reply;
- the counters equal the unreleased messages;
- no counter is negative;
- `glob ≥ sess`.

## BR21a (one session S1, key K empty; the writer does not wait for replies)

| Case | Schedule | Golden |
|---|---|---|
| q1 | t=0 M(k00) | active, 1 set, (1, 61507) |
| q2 | t=1..3 M(k01..k03) | queued; (4, 246028); still 1 set |
| q3 | t=4 M(k04) | 307535 > 262144 → −32005 store, `storeViolated = {sessBytes}`, not appended |
| q4 | t=5..8 m(k90..k93) | (8, 246300) |
| q5 | t=9 m(k94) | n = 9 > 8 → −32005 store, `{sessN}` |
| q6 | settle(1..8) at 100..800 | `set` order k00, k01, k02, k03, k90..k93. One call each, with the cumulative dictionary. Replies `null` in order. (0, 0). Final total 245788 = SiteStorageRef |
| q7 | fail(1) at 100 | k00 → −32603 transport, reload (total 0), k01 starts. Extension: settle 2..8; final total 184345, no byte from k00 |
| q8 | from the q5 state, without q6/q7 | see the table below |
| q9 | 8 sessions × 8 small messages (glob 64, 4352) | the 9th session's first message → −32005 `{globN}`. Concurrent `set` calls ≤ 2. Extension: the full settle order (pairs S1/S2, S3/S4, … by the acceptance FIFO), 64 calls |
| q10 | S1..S16 × 4 M at t=4(j−1)+i | at t=63 glob = (64, 3936448), 2 sets. At t=64, S17's M → −32005 `{globN}` only (globBytes 3997955 ≤ 4194304). glob unchanged, 2 sets |

**q8: hold after the timeout (validation.md:305–314)**

| t | Event | Reply | S1 after |
|---|---|---|---|
| 5000 | k00 write deadline | −32603 storeTimeout | (8, 246300); k00 stays active |
| 6000 | m(k95) | −32005 `{sessN}` | (8, 246300) |
| 10001 / 10002 / 10003 | k01 / k02 / k03 wait expires | −32005 store each | (5, 61779) after 10003 |
| 10005..10008 | k90..k93 wait expires | store each | (1, 61507) |
| 10010 | m(k96) | — (queued, key blocked) | (2, 61575), still 1 set |
| 12000 | settle(1) | **none** | reload, total 61443, one release → (1, 68); k96 active, 2nd set |
| 12100 | settle(2) | `null` for k96 | total 61447, (0, 0) |

Criterion: k00 has exactly one reply and one release.

## BR21b (teardown and navigation)

- **r1/r2.** F1 has k00 active and k01..k03 waiting. `site_navigate` at 100 cancels k01..k03 with no reply → (1, 61507). No snapshot before settle or 5100.
- **r3.** settle(1) at 300: no reply to any frame. The F2 snapshot = {k00}. (0, 0).
- **r4.** No settle: at 5100 F2 loads the confirmed state ({}) with the badge. k00 stays counted until it settles (extension: settle at 6000 → (0, 0), no reply).
- **r5.** Tab close at 100: same cancellation. The session record is kept until k00 settles at 300, then removed.
- **r6.**
  - k00 active and k01 waiting, (2, 123014).
  - At 5000: storeTimeout.
  - At 5100: navigate; k01 suppressed → (1, 61507); the wait runs until 10100.
  - At 7000: settle → no reply, (0, 0); F2 snapshot = {k00}.
  - **Variant:** no settle until 10100 → the snapshot is {} with the badge. k00 stays (1, 61507) until it settles; one release.
- **Authored Q2 cases:**
  - **Q2-n1:** two tabs on one key; only F1's waiting message is cancelled; G1's message runs next; the only reply goes to G1.
  - **Q2-c1:** a ready message is cancelled; the new frame gets an immediate snapshot with no badge.

## BR21c (FUTURE: Chrome on anvil-br; not executed)

**Script.**
- BadSite sends `M(k·)` messages for 60 s, at the highest rate its bucket allows, without awaiting replies.
- The extension under test runs with the TestHooks build. RpcTap observes the network.

**G (to be computed when executed).**
- Every decision matches BridgeRef extended with this StoreQueue model, replayed on the recorded arrival times.
- In every trace line: `sess.n ≤ 8` and `sess.bytes ≤ 262144`.
- Zero RpcTap requests.
- The final dictionary equals SiteStorageRef.

**C.**
- Worker ping p99 ≤ 100 ms.
- Extension process memory ≤ baseline + 32 MiB.
- Every `set` completes within 5 s.
- If a measurement is not possible, the result is "inconclusive" with the recorded values.

**Also measures** the `chrome.storage.local.set` single-item atomicity that the model assumes (browser.md:437).

**Status:** not started as an executed test. No browser or performance evidence exists.

## BR21d (FUTURE: real loader; not executed)

**Script.** A site calls `setItem` 10^4 times on 50 keys within 10 s.

**Criteria.**
- The loader's unanswered messages ≤ 8 at every moment.
- Zero unrecovered `store` or `rate` rejections.
- After settling, the confirmed dictionary holds the last value of each key.
- A `setItem` that would exceed 1 MiB locally throws `QuotaExceededError` without sending any message.
- An internal link followed after a write saves that write before navigating (browser.md:486–493).

**Status:** not started as an executed test.

## BR21e (TQ, global byte edge; test build only)

| t | Message | Reply | glob after | storeViolated |
|---|---|---|---|---|
| 0..3 | S1 M(k00..k03) | — | (4, 246028) | — |
| 4 | S2 ['k50', 53905 × 'a'], qbytes 53972 | — (active, 2nd seat) | (5, 300000) = limit | — |
| 5 | S2 ['k51', 'x'], 68 | −32005 store | (5, 300000) | {globBytes} (S2 would be (2, 54040), globN 6 ≤ 64) |
| 6 | S3 ['k60', null], 67 | −32005 store | (5, 300000) | {globBytes} |
| 100 | settle(1) | `null` k00 | (4, 238493) | — |
| 101 | S2 ['k51', 'x'] | — (queued) | (5, 238561) | — |

## BR21f (quota before the seat, I56) and f2

| t | Event | Reply | S_A / glob | sets |
|---|---|---|---|---|
| 0 | m0 S_A ['k17', 4042 × 'a'] (4109); newTotal 1048576 = limit | — (active) | (1, 4109) / (1, 4109) | 1 |
| 1 | m1 ['k18', 'x'] (68) | queued | (2, 4177) | 1 |
| 2 | m2 ['k01', null] (67) | queued | (3, 4244) / (3, 4244) | 1 |
| 3 | m3 S_B ['k', 'v'] (66) | — (active, 2nd seat) | glob (4, 4310) | 2 |
| 4 | m4 S_C ['k', 'v'] (66) | — (ready, no seat) | glob (5, 4376) | 2 |
| 100 | settle(1) | `null` m0; then m1: 1048580 > 1048576 → 4300 and release, no `set`; m2 ready and takes the seat **before m4** (accepted earlier) | (1, 67) / (3, 199) | 3 |
| 200 | settle(2) | `null` m3; m4 active | glob (2, 133) | 4 |
| 300 | settle(3) | `null` m2; total(S_A) 987133 | (0, 0) | 4 |
| 400 | settle(4) | `null` m4 | glob (0, 0) | 4 |
| 500 / 600 | S_A ['k18', 'x'] → active / settle | `null`; total 987137 | (0, 0) | 5 |

**BR21f criteria:**
- m1 has one reply (4300) and one release;
- k18 is in no `set` before the 5th;
- `releaseCount` = 6 = admitted, `doubleReleaseCount` = 0, `quotaRecheckMismatch` = 0;
- the counters return to (0, 0);
- the dictionaries equal SiteStorageRef.

**f2** (validation.md:385–388). S_B and S_C hold both seats (placed at t=−2 and t=−1).
- t=0: S_A ['k18', 'x'] → 4300 at once, released, no `set`.
- t=1: S_A ['k01', null] → ready, waiting for a seat.

**Negative controls (validation.md:389):**
- `quotaNoRelease` leaves S_A at (2, 135) after t=100 and blocks m2.
- In f2, `quotaAfterSeat` waits for a seat before answering. Both must fail.
