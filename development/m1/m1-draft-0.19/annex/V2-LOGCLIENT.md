# Annex V2: LogClient (D89, D91), M1 draft 0.19

**Proposed for review. Not approved. Nothing here was executed by the author.**

- **Sources:** `reference/browser.md:74–141`; `validation.md:818–861` (LC-unit); `FINAL_DESIGN.md:2040–2068` (eth_getLogs/pocol_getLogs limits, lemmas P1/P2/P3); `implementation.md:30–32, 293, 359`.
- **Reference:** `tools/logclient_ref.py`. Manual clock and `MockLogServer` only: no TypeScript, no Chrome, no RPC server.

## 1. Page path and limits

- **Page path.** The page calls `eth_getLogs`. The steps are:
  1. BridgeAuth checks the filter schema (D95; not modelled).
  2. The bridge resolves tags to numbers (not modelled).
  3. A range of more than 1024 blocks is rejected with `−32020 {reason:'range'}` before anything is sent.
  4. Otherwise the request goes to `fetchAll`. A raw `eth_getLogs` is never sent.
- **`fetchEach`** (the Node test tool) is not reachable from the bridge. The runner checks this structurally.

| Item | Value |
|---|---|
| Window | 1024 blocks, aligned at `from` |
| P1 reasons | resultBytes, resultCount, scanBytes; requires a ≤ lc ≤ b−1 |
| RES reasons | deadline, scratch; requires a−1 ≤ lc ≤ b−1 |
| Singleton retry | 500 ms; at most 3 retries (4 replies), then `Error{r, block:a}` |
| −32021 | Retry after `retryAfterMs`; counted in totalRequests and logRequests, never in `retries` |
| −32022 | anchorNotCanonical / beyondAnchor → Restart |
| maxRestarts | 3, so at most 4 attempts; then `abort(id, reorgUnstable)` and Error |
| fetchAll | 4194304 bytes per attempt, 30 s, 4096 requests; no partial result |
| fetchEach | external sink, 600 s, 65536 requests |

## 2. Counters

- **logRequests:** `pocol_getLogs` requests in the attempt, including retries.
- **headRequests:** `eth_getBlockByNumber` requests in the attempt: the anchor `H(latest)` and the pre-commit `H(anchor.number)`.
- **totalRequests:** every RPC request of every attempt of the run so far.
- **maxRequests** limits totalRequests only. It is checked **before** sending. The request that would make the total exceed maxRequests is not sent; the result is `abort(id, tooManyRequests)` and `Error{tooManyRequests}`.

## 3. Pure step (normative table)

`step({a, b, withinLimit, retries}, reply, last, hashes) →` one of:
- `append(logs)`
- `replace(first, second)`
- `split(first, second)`
- `retry(delayMs, retries')`
- `restart`
- `error{…}`
- `violation(why)`

**Complete(logs).** The reply is `append` if it passes these checks; otherwise it is a violation (`why` in parentheses):
- every blockNumber is in [a, b] (`outOfRange`);
- (blockNumber, logIndex) strictly increases and follows the last appended log of the attempt (`notStrictlyIncreasing`);
- each height has one blockHash (`blockHashChanged`);
- every log has canonical hex quantities (`malformedLog`);
- the result is a list (`resultNotList`).

**Limit (`−32020 {reason, fromBlock, lastCompleteBlock = lc, nextFromBlock}`):**

| Class | lc | Range | withinLimit | Action |
|---|---|---|---|---|
| any | not an integer / missing | any | any | violation (invalidLc) |
| unknown reason (incl. `range`) | any | any | any | violation |
| any | fromBlock ≠ a or nextFromBlock ≠ lc+1 | any | any | violation (**P-V2-2, proposed**) |
| P1 | any | any | **true** | violation (lemma P3) |
| P1 | a ≤ lc ≤ b−1 | a<b | false | replace([a,lc] **marked**, [lc+1,b] parent mark); the prefix completes before the tail |
| P1 | otherwise | any | false | violation (always for a=b) |
| RES | a ≤ lc ≤ b−1 | a<b | any | replace([a,lc], [lc+1,b]), both with the parent mark |
| RES | lc = a−1 | a<b | any | split at mid = a+⌊(b−a)/2⌋, both with the parent mark |
| RES | lc = a−1 | a=b, retries < 3 | any | retry(500, retries+1) |
| RES | lc = a−1 | a=b, retries = 3 | any | error{reason, block:a} |
| RES | lc < a−1 or lc ≥ b | any | any | violation |

The mark is inherited recursively: every sub-range of a marked range is marked.

**Other replies:**
- `−32021 {busy|rate, retryAfterMs}` → retry(retryAfterMs) with `retries` unchanged. A malformed reply is a violation (P-V2-3).
- `−32022 {anchorNotCanonical|beyondAnchor}` → restart. Any other reason is a violation (P-V2-3).
- `−32601` → error{unsupported}.
- Any other code → error{rpcError, code}.

The concrete golden table is `vectors/lc-step-table.json`: 74 hand-written rows × 2 ranges × each reason of the class, plus 26 non-Limit rows.

## 4. Attempt FSM

```
Idle → [check budget] H(latest) → anchor → begin(id, anchor)
  to > anchor.number                → abort(id, beyondHead); Error
  queue := windows(from, to)
  Fetching: head range r → [check budget] L(r, anchor.hash) → step
     append  → [maxBytes] piece(id, seq++, r, logs); pop r
     replace/split → r := first, second (first is fetched first)
     retry   → wait delayMs; same r
     restart → Restarting
     error / violation / budget / timeout / tooLarge → abort(id, reason); Error (terminal)
  queue empty → [check budget] H(anchor.number):
     same number and hash → commit(id, summary); done
     otherwise            → Restarting
Restarting: restarts = 3 → abort(id, reorgUnstable); Error
            else         → abort(id, reorg); id+1; restarts+1; Idle
```

- `summary = {pieces, logs, logRequests, headRequests, totalRequests, anchor}`.
- totalRequests includes aborted attempts.

**Termination:**
- Replace and split strictly shorten the range.
- There are at most 3 retries per block and at most 3 restarts.
- Log requests per attempt are ≤ 5·(b−a+1) plus the −32021 retries (the runner checks this per case).
- maxRequests is an explicit ceiling.

**Correctness lemma.** The ranges of one committed attempt form an ordered, disjoint partition of [from, to], and every piece is complete on the anchor's branch (lemma P2).

## 5. Sink, checker and consumers

- **Sink:** `begin(id, anchor)`, `piece(id, seq, range, logs)`, `abort(id, reason)`, `commit(id, summary)`.
- **SinkChecker** works on the shared request/sink timeline:
  - S-a: per attempt, begin, then pieces, then exactly one terminal; piece seq runs from 0.
  - S-b: nothing after a terminal; no begin while an attempt is open; ids are 1, 2, ….
  - S-c: at most one commit, and only for the last attempt.
  - S-d: the committed ranges exactly partition [from, to].
  - S-e: the summary equals the delivered pieces and logs, and the MockLogServer's own log for the attempt's log/head requests and for all requests so far.
- **Requests and attempts.** A request belongs to the attempt whose terminal follows it, so `H(latest)` belongs to the attempt it begins.
- **Consumers:**
  - RefConsumer buffers pieces per attempt, drops the buffer at abort, and returns it at commit.
  - FaultyConsumer (the control) keeps everything. In LC16 it exposes 18 logs, including the 2 logs at 1000 carrying A's block hash. In LC18 it exposes 20 logs, 4 of them stale.
- **fetchAll** uses an internal RefConsumer. On abort the page gets nothing: no partial content.

## 6. Fixtures

- **`vectors/lc-data.json`:**
  - literal block hashes (P-V2-7);
  - branch tables A, B and A′ (shared 1..999, diverging from 1000);
  - the literal RefA, RefB and RefA′ lists (10 logs each; windows 8/0/2).

  The runner checks the literal hashes against the stated rule, and the literal lists against the branch tables (two independent encodings).
- **`vectors/lc-cases.json`:**
  - LC1–LC18 with every variant: LC3 rate/singleton, LC4 edge-ok/edge-over, LC7a–d, LC8-beyondAnchor, LC9a–f, LC14-variant/inherit, LC15a/b, LC17-variant;
  - supplementary cases: beyondHead, unsupported, other error, maxRequests before begin, fetchAll and fetchEach timeout boundaries, and the bridge range and pass-through.
  - Each case has the full request log with times and anchor hashes, every sink event (aborted attempts and their pieces kept), the full result content and its SHA-256, and the consumer outputs.
  - Literal totals: LC1 = 5, LC5 = 5, LC8 = 7, LC10 = 4 sent, LC16 = 11, LC17 = 12 (variant 8), LC18 = 10.
- **Controls:** nine client faults (tailFirst, markNotSet, noMarkInherit, countBusyRetries, maxCheckAfterSend, noPrecommitCheck, resetTotalOnRestart, acceptInvalidComplete, maxRestarts4). Each must change named witness cells.
- **`vectors/lc-sink-controls.json`:** 15 timelines: 2 baselines plus 13 corruptions, each with an exact expected S-rule set.

## 7. Boundaries (manual clock)

- **Timeouts** (P-V2-4) are inclusive and checked at each send:
  - fetchAll: a −32021 retry due at 30000 ms is sent; one due at 30001 ms gives `abort(timeout)` and no partial result;
  - fetchEach: the same at 600000 / 600001.
- **maxBytes** (P-V2-5): compact key-sorted JSON bytes per piece, cumulative per attempt.
  - One log is 198 + D bytes, where D is the number of data hex digits; the array adds 2.
  - D = 4194104 gives exactly 4194304 bytes, which is accepted.
  - D = 4194106 gives 4194306 bytes, which is tooLarge (data hex has even length).
  - LC4: piece 1 is delivered to the internal sink. Piece 2 alone is 4194506 bytes → tooLarge; fetchAll returns nothing.
- **Singleton retries** are spaced exactly 500 ms apart. The −32021 waits in LC3-singleton (700, 300) do not consume retries.

## 8. Conventions and proposed supplements (not source changes)

| Id | Point |
|---|---|
| P-V2-1 | If maxRequests is exhausted at a new attempt's `H(latest)`, there is no open id, so the result is Error{tooManyRequests} with no begin and no abort. The source names `abort(id, …)` (X-maxBeforeBegin). |
| P-V2-2 | The client checks fromBlock = a and nextFromBlock = lc+1; a mismatch is a serverViolation. |
| P-V2-3 | A malformed −32021, or a −32022 with an unknown reason, is a serverViolation. |
| P-V2-4 | Timeouts are inclusive and checked at send. |
| P-V2-5 | The maxBytes measure is as defined in §7. |
| P-V2-6 | A malformed head reply before begin gives Error{headViolation} with no sink call. |
| P-V2-7 | Literal hashes, address and data tags are author-chosen. |
| P-V2-8 | Waits are exactly retryAfterMs or 500 ms (the source says ≥). |

Each is recorded as a partial gap in the results until root or the owner accepts or replaces it.

## 9. Scope

V2 stays **Partial**:
- no TypeScript LogClient;
- no Chrome bridge;
- no pocold `pocol_getLogs` (RG3, RG3b, RG9);
- no Node `fetchEach` tool;
- BridgeAuth schema checking and tag resolution are not modelled.
