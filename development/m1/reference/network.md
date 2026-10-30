# Node and network interfaces

**P2P**
libp2p over TCP+Noise, with gossipsub and request/response on '/pocol/1/req'.

**Messages**
- Status = [chainId, genesisHash, protocolVersion, headHash, height, totalWork]. An identity mismatch or unsupported version disconnects the peer. Status is informational and does not replace 1b.
- Request = [reqId, kind, body]: 1 GetHeaders [locator ≤ 32, count ≤ 512], 2 GetBodies [hashes ≤ 64], 3 GetTemplate, and 4 GetEnvelope [blockHash, exclude ≤ 4 hdrId] (D131). GetEnvelope returns the stored envelope-valid copy if its hdrId is not in exclude, otherwise NotFound.
- Response = [reqId, status, payload]. status: 0 success, 1 not found, 2 limit exceeded, 3 invalid, 4 rate limited. A missing body is 0x80; truncate at 8 MiB.

**gossip (byte limit)**
- tx: 131072.
- template: 197294. Check: size, signed form (otherwise templateForm), 1b, ecrecover, then evaluable items.
- block: 3067 (header only).
- share: 43.
- evidence1: 1368.
- evidence2: 243.

**Receiving block**
1. Size, then 1b without the parent. Failure incurs a penalty without storage.
2. Unknown parent: unknownParent queue ≤ 64, without work or ASERT. Parent rejection drops the child as ancestorInvalid.
3. Remaining H-pre (including 10a), then body, then H-full, then broadcast.
4. (D129) envReject drops the copy and penalizes its sender without judging blockHash. Request another copy from a peer that announced the block. Broadcast sends the stored copy byte for byte.

**Synchronization (D38)**
1. Status and locator.
2. R: H-pre for every header; violations drop the header and descendants and ban the peer. ≤ 4096 per branch and ≤ 16384 overall.
3. E: GetBodies, then sequential H-full.
   - A body not matching txRoot and evidenceRoot is a response error (BodyMismatch, D125): drop it, penalize the peer and request it from another peer; retain the header.
   - A matching body violating H-full through blockReject invalidates the block and descendants. envReject (including the signature part of item 13) rejects only the copy; retain the header and its entry and request another copy (D130).
   - Fetch scheduling (D128, node behavior, not validity) uses three mechanisms:
     - DiscRun: one discovery run with a dirty flag and per-run tried set.
     - BodySched: one in-flight body request per hash, with triedPeers.
       - Reeligibility (D130, amending D129) places the peer in reeligible without removing it from tried in the current cycle. Triggers: its Status, announcement of the block or a descendant, or expiry of exponential backoff min(1s·2^k, 60s).
       - Strict selection: untried peers in ID order, then reeligible peers in announcement order. Never request a reeligible peer while x has an untried peer.
       - cycleReset(x) starts a new cycle: clears tried, reeligible and backoff timers for x. In pocold this occurs when x becomes requestable again after removal; in SimNet it occurs for all hashes in the atomic t_avail step (fixture command).
     - EnvFetch (D131, node behavior, not validity): activated for x when every header copy at i is rejected by envReject, or i has no envOk copy once parent state is available.
       - Request GetEnvelope [x, exclude], where exclude contains the latest ≤ 4 rejected hdrId values for x. An honest node stores and serves only an envOk copy.
       - Scheduling follows BodySched: one in-flight request per x, per-cycle triedEnv and reeligible, and strict ID-order selection of untried peers. reeligible is FIFO by entry time, with one position per peer; reentry after a request moves it to the tail (D139). cycleReset at t_avail in SimNet; exponential backoff in pocold.
       - (D137) No request for x before parent state is available. envOwn must be evaluable, so a solicited response never enters envPending.
       - EnvQueue: one entry per x needing a fetch, FIFO by need time, ≤ 64 B per entry. Entries ≤ synchronization queue bound (16384).
       - EnvSlots (D137): ENV_SOLICIT_SLOTS = 32 slots for solicited requests, separate from ENV_GOSSIP_SLOTS = 32 for unsolicited-copy checks. Total = ENV_INFLIGHT_GLOBAL = 64.
         - A slot owns one check and a response buffer ≤ ENV_REPLY_MAX = 3067+64 = 3131 B.
         - Grant: only to the EnvQueue head, in the same step that selects the peer and sends the request. No request without a slot. Releasing a solicited slot grants it to the EnvQueue head in the same step, if present; grants are work-conserving (D139).
         - States (D138): reserved (request sent, buffer empty), then buffered (response read into the reserved buffer), then checking (check reads the buffer), then released.
         - Attempt: each grant receives a node-local monotonic u64 attemptId, never reused.
           - Every event carries (slotIdx, attemptId): response, timeout, disconnection and check result.
           - An event not matching the slot's current attempt is ignored and changes no slot.
         - Ordering: EnvFetch events are processed in one event loop in occurrence order. At equal times, response completion precedes timeout: a response is on time if its final byte arrives at t ≤ sendTime+ENV_REQ_TIMEOUT.
         - Timeout: ENV_REQ_TIMEOUT measures only from send to the final response byte. Transition to checking cancels its timer; no timeout runs during checking.
         - Cancellation is separate from ownership. Every termination cause sets cancelRequested, then:
           - In reserved or buffered: reset the request substream, discard partial bytes and release in the same step, because no task is reading the buffer.
           - In checking: no release. The check (envPre then envOwn, without apply) is synchronous, uninterruptible computation run only in EnvCheckPool (D139). It owns the slot and buffer until settlement, however long that takes, and releases in the settlement step itself.
             - EnvCheckPool: ENV_CHECK_WORKERS = W = 4 dedicated threads running only envelope checks, with a FIFO queue by entry into checking. No envelope check runs in ComputePool or the consensus runtime.
             - c_exec: wall-clock execution time of one check; assumed ENV_EXEC_MAX = 50 ms.
             - Waiting-bound lemma: live checks ≤ 64, so at most 63 precede any check; a thread becomes available within ≤ ⌈63/W⌉·c_exec.
             - Therefore c_env = (⌈63/W⌉+1)·ENV_EXEC_MAX = 17·50 = 850 ms, provided c_exec ≤ ENV_EXEC_MAX.
             - Exceeding ENV_EXEC_MAX increments envCheckOverrun, does not release the slot, and invalidates the time bound, not the capacity bound.
         - Cancelled-check result:
           - Retain envReject and penalize the sender if x still has a local entry, because the judgment depends only on the bytes.
           - Use envOk only if x remains in envWait.
           - Otherwise discard the result and increment lateCheckDropped.
           - In all cases: never import a removed or already imported block, or release another attempt's slot.
         - Capacity lemma (D138): every checking task and received byte is owned by a slot from grant until release, and release never precedes check settlement. Thus at every instant, regardless of repeated cancellations:
           - In-flight checks ≤ 32 solicited + 32 gossip.
           - Live buffers ≤ 64·3131 B.
         - Gossip slots follow the same rules: acquireGossip reserves before checking, and copy bytes move into the slot buffer.
         - A response exceeding ENV_REPLY_MAX is cut off at the bound and treated as a response error penalizing the sender.
         - Release occurs exactly once, guarded by a flag and attempt ID. In checking it occurs only at check settlement (D138). There are eight attempt-termination causes:
           - Check completion (envOk or envReject).
           - NotFound.
           - ENV_REQ_TIMEOUT = 2s expires after send before full response reception. It does not apply in checking.
           - Peer disconnect.
           - Oversized response.
           - Import of x.
           - An envOk copy of x is stored through another path.
           - blockReject of x or removal of x.
         - After unsuccessful release, x returns to the EnvQueue tail if it remains in envWait and has an untried or reeligible peer. Otherwise it awaits a reeligibility event, then joins the tail.
         - Timeout places the peer in triedEnv for the cycle, with exponential reeligibility backoff as for NotFound. No permanent exclusion and no penalty for slowness alone.
         - After timeout or cancellation in reserved or buffered, reset the substream, so a late solicited response cannot arrive on that request. A later copy sent by the peer has no solicited slot and follows the unsolicited path below, without a lateness penalty.
       - A response whose copy fails envelope checks is a response error: penalize the sender and place it in triedEnv. A duplicate copy (hdrId in EnvCache) is not checked again.
       - envPending copies ≤ ENV_PENDING_MAX = 4 per x. Drop excess without checking or evicting stored copies; dropping does not prevent later EnvFetch.
       - Envelope-judgment retention (D136, local node policy, not validity):
         - Stored envReject judgment = {hdrId, blockHash, rule, peer, seq} ≤ ENV_ENTRY_MAX = 128 B. Rejected-copy bytes are not retained.
         - Retain judgments only for a blockHash with a local entry: HeaderStore, synchronization queues, unknownParent or side blocks. Otherwise penalize and discard the copy without retention.
         - Bounds: rej(x) ≤ ENV_REJ_PER_BLOCK = 8; total ≤ ENV_REJ_GLOBAL = 16384. Global envPending ≤ ENV_PENDING_GLOBAL = 1024. In-flight checks ≤ ENV_INFLIGHT_GLOBAL = 64, including ≤ 32 solicited and ≤ 32 unsolicited (D137).
         - An unsolicited copy (gossip or announcement) of x with no envOk copy follows:
           - Stored hdrId: drop without checking.
           - rej(x) = 8, pending(x) = 4, global capacity full, or no free gossip slot while parent state is available (D137): drop without checking or penalty, increment envFloodDropped. Without parent state, retain as envPending within both bounds, without a slot. x stays in envWait and EnvFetch remains active.
           - Otherwise: check it.
         - A solicited copy (GetEnvelope response from the selected peer, on a slot reserved before requesting, D137) is always checked within ENV_SOLICIT_SLOTS, never exceeding the check cap. If rej(x) = 8, evict x's oldest judgment before insertion. At most one request per (x, peer) per cycle; ≤ 32 globally in flight.
         - Global capacity pressure evicts the oldest inserted judgment and increments envEvicted.
         - Cleanup: when x is imported, an envOk copy is stored, blockReject occurs, or x is removed by queue bounds or D118, delete all its judgments and pending copies.
         - An evicted judgment may be recomputed on rearrival, with the same deterministic result. “Each hdrId once” applies while its judgment remains stored.
         - Capacity pressure never records blockReject or pocol_getRejects, or triggers descendant cancellation.
         - Recovery lemma: for each x in envWait at honest i, an honest holder h* has an envOk copy (D139). Definitions:
           - t_0 (D140): first instant when G1–G4 all hold and continue holding until acceptance:
             - G1: parent state of x is available at i (D137 send prerequisite); x remains in envWait with a local entry not removed by queue bounds or D118.
             - G2: h* is connected to i, holds an envOk copy and serves it.
             - G3 (responsiveness): every GetEnvelope request from i to h* sent after t_0 receives its final response byte within RTT_h* < ENV_REQ_TIMEOUT.
             - G4: i's peer set is stable during the interval (P defined below).
             - Time before t_0 (preCondWait) is excluded from D_x and recorded as a measurement without any timing claim.
             - Losing any prerequisite before acceptance voids the guarantee. When all return, start a new t_0' and compute P, Q and B from it.
             - The earlier D139 definition (only x in envWait and connected h*) is withdrawn: D137 forbids requesting before parent state, and connectivity does not guarantee a timely response.
           - P: number of distinct peers EnvFetch may select for x during the interval, including h*.
             - Assuming i's peer set is stable, P ≤ 50 (P2P bound).
             - Each new connection increments P; no fixed bound is claimed under unlimited churn.
           - Q: maximum EnvQueue length during the interval including x; Q ≤ 16384.
           - B: zero if h* is untried or reeligible at t_0; otherwise its remaining exponential backoff, B ≤ 60s.
           - Solicited requests for x through the h* response, including success, ≤ 2P−1. Reason:
             - At most P−1 untried peers.
             - Then at most P−1 peers preceding h* in reeligible.
             - Then h*.
           - Each response is checked on its reserved slot.
           - (D137, D138) Each slot turn releases within ≤ ENV_REQ_TIMEOUT + c_env, because:
             - The timeout measures reception alone.
             - Subsequent checking, from entering checking to settlement including EnvCheckPool wait, is ≤ c_env and uninterruptible (D139).
             - Cancellation neither shortens the turn nor extends it beyond this bound.
           - Wait until each request is granted ≤ ⌈Q/32⌉·T_role, where T_role = ENV_REQ_TIMEOUT + c_env. Reason:
             - Grants are work-conserving, so each T_role interval includes ≥ 32 grants while the queue stays nonempty.
             - At most Q−1 entries precede x.
           - The request's own turn is then ≤ T_role.
           - Hence acceptance delay after t_0 ≤ D_x = (2P−1)·(⌈Q/32⌉+1)·T_role + B.
             - For P=1, Q=1, B=0: D_x = 2·T_role, covering reception and checking.
             - The former formula (P−1)·⌈Q/32⌉·T_role is withdrawn because it omitted the successful request and its turn.
           - Reeligibility waits while x is outside the queue without a selectable peer are confined to [t_0, t_0+B], because h* remains selectable afterward until requested.
           - The bound grows with Q but stays finite because Q ≤ 16384. Capacity pressure does not prevent acceptance while G1–G4 hold.
           - Slow holder (D140): if h* consistently responds later than ENV_REQ_TIMEOUT, every attempt times out and resets the substream; the late response is unread.
             - No acceptance guarantee through the solicited path; this is not merely delay bounded by exponential backoff.
             - Acceptance remains possible only through gossip when a gossip slot is free, or through another holder satisfying G3.
             - This is service policy: x stays in envWait, without blockReject or an entry in pocol_getRejects; timeouts increment envTimeouts.
         - Memory (D137): judgments 16384·128 + pending copies 1024·3067 + slots 64·3131 + EnvQueue 16384·64 = 2097152 + 3140608 + 200384 + 1048576 = 6486720 B ≤ ENV_MEM_MAX = 7 MiB (7340032). No response buffer outside slots.
       - Drain-phase fetch lemma:
         - envOk is deterministic: envPre is stateless and envOwn uses committed parent state. An honest response is valid at the requester.
         - Each x ∈ Rec has an honest holder of an envOk copy, established by HaltPin (validEnv in validation).
         - Strict selection reaches it within ≤ |H|−1 requests, with ≤ one request per (x, peer).
         - Thus GetEnvelope requests after t_avail ≤ |Henv_i|·(|H|−1).
         - Every received copy existed before t_avail (copy lemma), so envCheck ≤ Venv_i.
     - HfullDone and HdrCache: H-full is not repeated for a settled block. Two-key storage (D129):
       - BlockCache keyed by blockHash: completed judgments of committed content only.
       - EnvCache keyed by hdrId: envelope judgments.

     Details in validation.
4. Adoption at depth ≤ R_keep.

**Input constraints**
No RPC accepts blocks, headers or templates. pocol_submitShare checks TemplateID against an accepted local template.

**Mempool**
- len > TX_MAX: −32014.
- chainId mismatch: −32015.
- Full: −32016; after F: −32016 {final: true}.
- Transactions returned after reorg are rechecked.

**P2P and node resource limits**
- ≤ 50 peers, queue ≤ 256, ≤ 16 per peer.
- Mempool ≤ 64 MiB.
- Node memory excluding RPC ≤ 750 MiB, including redb cache, envelope judgments, slots and EnvQueue ≤ ENV_MEM_MAX = 7 MiB (D136, D137). RPC memory ≤ 192 MiB.
- ASERT ≤ 512 bits.
- Ban after 3 violations of 1b or templateForm in 10 minutes through gossip; immediately during synchronization.

**Standard JSON-RPC**
- eth_chainId, net_version, web3_clientVersion.
- eth_blockNumber, eth_getBlockByNumber/ByHash, eth_getTransactionByHash, eth_getTransactionReceipt, eth_sendRawTransaction.
- eth_call, eth_estimateGas, eth_gasPrice, eth_maxPriorityFeePerGas, eth_feeHistory (≤ 1024).
- eth_getBalance, eth_getCode, eth_getStorageAt, eth_getTransactionCount.
- eth_getLogs (≤ 1024 and D88 limits), eth_getProof (within K_eff, otherwise −32017).
- Unsupported: filters and eth_subscribe (−32601). HTTP/1.1 transport only.

A site's extension-accessible methods are a subset defined by the BridgeAuth matrix in browser, not this list. site_* methods are extension-local; the node does not recognize them and returns −32601 if they reach it.

**pocol_getHeaders(from, count) (D80, D81)**
RLP list of canonical-chain headers from..from+count−1. Fixed-order checks stop at the first violation:
- F0: valid JSON quantities within u64 (hex without leading zeros). Invalid, missing or extra parameters: −32602.
- F1: from < 1: −32018 {fromZero}.
- F2: count < 1 or > 512: −32018 {countRange}.
- F3: from > head: −32018 {fromAboveHead}.
- F4: from+count−1 > head (u128): −32018 {beyondHead}.

F0–F2 do not read state. F3–F4 read head from the request snapshot. At head=0 every request passing F0–F2 returns fromAboveHead. Success returns exactly count headers; the client sends ≤ 14.

**RpcGuard (D87, D88, D90, D92, D93, D94): local service policy, not a validity rule**

Bounds are test assumptions, exposed in pocol_getParams.rpcLimits.

**(A) Transport and invariant S1**
- HTTP/1.1 on hyper, no HTTP/2 or pipelining, on 127.0.0.1 only.
- One request at a time; do not read the next before sendDone of the previous reply.
- S1: at most one response reservation and one waiting request per connection.
- Explicit SO_SNDBUF = SEND_SOCK_BUF = 32 KiB per connection disables autotuning. No bound is assumed on the acceptance/read gap; measure it in RG14(0) and tracing, without using it in any criterion (D93).

**(B) Classification table (worst-case encoded response)**

Light (LIGHT_RESP_MAX = 512 KiB):
- ≤ 4 KiB:
  - eth_chainId, net_version, web3_clientVersion, eth_blockNumber.
  - eth_gasPrice, eth_maxPriorityFeePerGas, eth_getBalance, eth_getStorageAt, eth_getTransactionCount, eth_sendRawTransaction.
  - pocol_getTiming, pocol_getTarget, pocol_getCaps, pocol_syncStatus, pocol_getPot, pocol_getSettlement.
  - pocol_getBurnTotals, pocol_getEscrow, pocol_getRetention, pocol_getClosing, pocol_getRpcStats, pocol_submitShare.
- eth_getCode: ≤ 2·B_code_max+1 KiB ≈ 65 KiB.
- eth_getTransactionByHash: ≤ 2·TX_MAX+2 KiB ≈ 258 KiB.
- eth_getTransactionReceipt: ≤ 2·LOG_BLOCK_MAX+546·512 B+2 KiB ≈ 339 KiB.
- eth_getBlockBy* with hashes: ≤ 2 KiB+(BODY_MAX/85)·70 B ≈ 160 KiB.
- pocol_getHeaders with count ≤ 64: ≤ 64·(2·3067+16) B ≈ 385 KiB.
- pocol_getParams: ≤ 2·|genesisPre|+4 KiB, thus ≤ 92 KiB in end/full.
- pocol_getAssignment, pocol_getMinerSet and pocol_getTemplate: ≤ 1024·160 B = 160 KiB.

Heavy (RESP_MAX = 8 MiB): eth_getLogs, pocol_getLogs, eth_call, eth_estimateGas, eth_getProof, eth_feeHistory, pocol_getHeaders with count > 64, eth_getBlockBy* with full transactions, pocol_getRejects.

Unlisted method: −32601 before admission. Adding one requires a table row and passing RG13. Overflow while encoding: −32020 {resultBytes}, with clean release (RG12).

**(C) Global budgets (node-wide)**
- RPC_CONN_MAX = 32; close excess connections without reading.
- Permits: HEAVY_ACTIVE = 4, HEAVY_QUEUE = 16, LIGHT_ACTIVE = 16, LIGHT_QUEUE = 32. FIFO queues; an honest request waits behind ≤ 31 requests.
- Per-connection token bucket: rate 20/s, capacity 20:
  - Single request consumes one token; a batch consumes k tokens at parsing.
  - Insufficient tokens return −32021 {rate, retryAfterMs} without execution. For a batch, write an array of k errors ≤ 256 B from CONN_BUF.
  - No waiting for tokens within a request or batch.
- Response memory: HEAVY_RESP_MEM = 48 MiB, LIGHT_RESP_MEM = 16 MiB = 32·512 KiB, so light reservations never run out (S1).
- SCRATCH: ≤ 16 MiB per heavy permit, ≤ 1 MiB per light permit. Computation is synchronous without await on a thread from a fixed pool (20 threads), with thread-local accounting; overflow returns −32020 {scratch}.
- REQ_BUF = 1 MiB, CONN_BUF = 64 KiB per connection.
- RPC_MEM_TOTAL = 48+16+4·16+16·1+32·(1+1/16) = 178 MiB ≤ 192 MiB.
- Encode into a list of 64 KiB segments inside the reservation, without doubling reallocations.
- Kernel buffers (32 KiB per connection) are outside accounting and measured within RSS, including accepted-but-unread bytes after sendDone.
- Snapshots ≤ 20: one per permit.

**(D) Request ownership lifecycle (D88)**
1. Reading:
   - Starts at the first byte. Headers and body ≤ REQ_BUF within REQ_READ_DEADLINE = 10s, otherwise close.
   - Close idle connections between requests after IDLE = 30s, measured from previous response sendDone or connection opening.
2. Parsing: F0, origin denial (H2), classification, rate bound and numeric log-range check. All within REQ_BUF, without reservation or snapshot.
3. Atomic admission: permit and class-maximum reservation together for the queue head.
   - Waiting requests hold nothing.
   - Within QUEUE_WAIT = 2s, otherwise −32021 {busy, retryAfterMs ∈ [250, 2000]}.
4. Computation owns {permit, reservation, snapshot, SCRATCH}; encode inside the reservation. COMPUTE_DEADLINE from admission includes encoding: 5s heavy, 500 ms light; expiry returns −32020 {deadline}.
5. End of computation: release snapshot, SCRATCH and permit; shrink reservation to actual size.
6. Sending under SendPacer (D90, D92, D93, D94):
   - P = bytes submitted by server to pacer (written); A = bytes accepted by server socket; A ≤ P.
   - pending is true when P > A.
   - sendTime advances only while pending, starting with the first submitted byte.
   - R = MIN_SEND_RATE = 65536 B/s; GRACE = 2s.
   - Sole binding rule: close if sendTime > GRACE and A < R·sendTime.
   - Credit = A − R·sendTime; violation instant s_v = max(GRACE, A/R).
   - Enforcement:
     - Timer at sendTime = s_v+TIMER_EPS, where TIMER_EPS = 1 ms resolves equality because the rule is strict.
     - Reset timer whenever A or pending changes; backstop check every 50 ms, and recheck exact values when it fires.
     - lateness ≤ ENFORCE_SLACK = 200 ms, recorded in pacerLateMaxMs.
   - Bound theorem:
     - While the connection remains open, sendTime ≤ max(GRACE, A/R)+ENFORCE_SLACK and A ≤ size.
     - Thus SEND_MAX(size) = GRACE+size/R+ENFORCE_SLACK. An 8 MiB reply → 130.2s; a 512 KiB reply → 10.2s.
     - Credit does not raise the bound.
   - CloseOK: every rate-triggered close satisfies sendTime > GRACE, A < R·sendTime, and sendTime − s_v ≤ ENFORCE_SLACK; record (sendTime, A) in RpcStats.
   - PacerTrace (D93):
     - rpc.pacerTrace=true, available in release builds.
     - Record a row on every P, A or pending change and at close/sendDone: {conn, t_ns (monotonic), sendTime_ns, P, A, pending, event ∈ {produce, accept, pendStart, pendStop, close, sendDone}}.
     - Does not change decisions or consensus.
     - Fixed 4 MiB buffer; overflow increments traceDropped; traceDropped > 0 makes a run inconclusive.
   - sendDone (D94): first instant A = P = size after all reply bytes (or the entire batch including ']') have been submitted. At this instant:
     - Pacer.write returns Done; release the reservation.
     - sendTime stops permanently, record sendDone, start IDLE.
   - receiveDone: instant Rc = size at the client.
     - Invisible to the server; does not delay reservation release or enter any timeout/rule.
     - Measured only in client logs and PacerSim.
     - Bytes between A and Rc at sendDone (unreadAtSendDone) are in kernel buffers, not the reservation.
   - Release the reservation at sendDone or close, whichever comes first.
7. Do not read the next request before sendDone.

The only independent deadlines are REQ_READ_DEADLINE, IDLE, QUEUE_WAIT, COMPUTE_DEADLINE, the SendPacer rule and BATCH_WALL. All measure server events, not client reception.

**(E) Batches (D88, D90, D92)**
- ≤ BATCH_MAX = 20 items, chunked streaming: write '[', then each item passes stages 3–5 and submits its reply to the pacer.
- Item k+1 starts admission as soon as the socket accepts the final byte of item k. The server can therefore precede client reading by up to socket-buffer capacity.
- Between items hold no permit, snapshot or reservation; each item reenters admission at the queue tail.
- One SendPacer per batch: one sendTime starting at '[', paused without pending (queue wait and computation), one GRACE, and the rule uses cumulative A; no per-item deadline or grace.
- Batch sendDone occurs when the final framing byte is accepted. Each item's reservation releases on acceptance of its final byte.
- batchSent = all submitted bytes. The item that makes batchSent > BATCH_RESP_MAX = 16 MiB is computed, then replaced with −32020 {batchBytes}; subsequent items receive the same error without execution. Each error ≤ ERR_ITEM_MAX = 256 B from CONN_BUF.
- BATCH_BYTES_HARD = 16 MiB+20·256+64 = 16782400 B.
- Derived bounds:
  - BATCH_SEND_MAX = 2+256.0791015625 = 258.0791015625s; monitoring bound 258.2791015625s.
  - Waiting ≤ 20·(2+0.2) = 44s; computation ≤ 20·(5+0.1) = 102s.
  - Total ≤ 404.2791015625s.
- BATCH_WALL = 420s backstop from end of reading to sendDone; R26(F) requires ≥ 404.2791015625. Reaching it closes/releases within ≤ 100 ms and increments batchWallHits; any nonzero value in release is a defect.
- For a busy or rate item, write its error and continue. Preserve order and copy IDs. Mid-batch disconnection produces incomplete JSON that is not parsed.

**(F) Cancellation**
Set a token on close or any timeout; poll every 16 scanned blocks, every 1 MiB encoded and every 10^6 gas. Release all resources within ≤ CANCEL_MAX = 100 ms.

**(G) eth_getLogs**
Order:
1. F0 (−32602).
2. Numeric range ≤ 1024, otherwise −32020 {range, maxRange: 1024} without admission.
3. Admission.
4. Resolve 'latest', 'safe' and 'finalized' from the snapshot, then check range.
5. Ascending scan.

P1 bound:
- Process block x completely.
- If adding it makes response > RESP_MAX, count > LOGS_COUNT_MAX = 10000, or scan > LOGS_SCAN_BYTES = 32 MiB, return −32020 {reason ∈ {resultBytes, resultCount, scanBytes}, fromBlock, lastCompleteBlock = x−1, nextFromBlock = x}.

Resource bound: deadline or SCRATCH returns −32020 {deadline|scratch, fromBlock, lastCompleteBlock ≥ fromBlock−1, nextFromBlock = lastCompleteBlock+1}, without assuming progress.

Discard the entire partial buffer.

P1 lemma: one block ≤ 32 KiB raw and ≤ 546 logs; its encoding < 400 KiB. Therefore at P1, lastCompleteBlock ≥ fromBlock.

P3 lemma: P1 is raised only at lastCompleteBlock+1 and encoding is deterministic, so no subrange of [fromBlock, lastCompleteBlock] raises P1 on the same chain. It may raise only a resource bound.

blockHash filter = one block.

**(H) pocol_getLogs(filter, anchorHash) (D89)**
- Heavy, with eth_getLogs output, bounds and the same −32020. fromBlock/toBlock are numeric only; tags return −32602.
- Snapshot checks:
  - canonicalHash(height(anchorHash)) ≠ anchorHash returns −32022 {anchorNotCanonical, head}.
  - toBlock > height(anchorHash) returns −32022 {beyondAnchor}.
- P2 lemma: blockHash recursively commits to its parent. If anchorHash is canonical in two snapshots, their canonical blocks ≤ height(anchor) match, so successful chunks with the same anchor belong to one branch.
- No session state; no consensus change.

**(H2) Node origin denial (D95, second defense)**
- ORIGIN_DENY = {pocol_submitShare, pocol_getTemplate}.
- If a request carries Origin with any allowed value and its method is in ORIGIN_DENY, return −32601 at stage 2 without admission or snapshot.
- Apply per item in a batch.
- Local mining programs send no Origin. eth_sendRawTransaction remains allowed for the extension origin because WalletSubmit needs it; BridgeAuth protects it.
- Does not protect against a local process removing Origin (A10); not a validity rule.

**(I) Separation from consensus**
- RPC uses a two-thread tokio runtime and a 20-thread pool. Validation, mining and synchronization use another runtime.
- redb snapshots do not hold the writer.
- Changing RpcGuard, PacerTrace or ORIGIN_DENY does not change validity or require a fork.

**Other custom interfaces**
- pocol_getTemplate, pocol_submitShare (local mining interfaces, denied with Origin).
- pocol_getAssignment(h, a) → {seed, nMax, nonceMode, members[{id, start, end}]}.
- pocol_getMinerSet, pocol_getCaps, pocol_syncStatus.
- pocol_getTiming → {L0, U0, Tj, bj, fsmState}.
- pocol_getTarget(h) → {deltaT, e, s, f, F, target, work, saturated ∈ {none, high, low}}.
- pocol_getParams → {profileKind, nodeKind, nonceMode, chainId, genesisHash, genesisPre, netKey, forkSchedule, activeVersion, rpcLimits}.
- pocol_getPot(j) → {status, amount, kind, winner|null, blockHash, settlesAt}, otherwise −32013. winner = null until block j+1 is imported on the canonical branch (D141).
- pocol_getSettlement(j): decimal wei strings; unsettled returns −32013 {settlesAt}.
- pocol_getBurnTotals.
- pocol_getEscrow → {balance, pendingSum, heldTotal, burnedTotal, surplus}.
- pocol_getRetention → {complete, fromHeight, replicasConfigured, replicasLive, checkedAt} (D119).
  - ReplicaMonitor checks a list of local replicas (including this node) every 10s.
  - A replica is live if it replies within 2s, complete=true, and head ≥ local head−6.
  - Informational, not validity. Trusted in LN, not RP.
- pocol_getClosing → {closing, closeH, cursor, recLen, hEnd, finalAt|null}.
- pocol_getRejects: latest 256 in form {hash, rule, peer, path}.
- pocol_getRpcStats → {conns, heavyActive, heavyQueued, lightActive, lightQueued, heavyResv, lightResv, scratchHeavyMax, scratchLightMax, snapshotsOpen, maxResPerConn, cancelled, cancelMaxMs, busyCount, rateCount, limitCount, anchorRejects, originDenied, pacerClosed, pacerLateMaxMs, lastPacerClose{sendTime, accepted}, sendDoneCount, sendOverrunMaxMs, batchSendMaxS, batchWallMaxS, batchWallHits, traceDropped}.
  - sendOverrunMaxMs = maximum (sendTime at sendDone − (GRACE+size/R)) in milliseconds.
  - originDenied counts H2 responses.

**Local RPC**
- 127.0.0.1; disallowed Host returns 403; Origin absent or chrome-extension://<ID>.
- Body ≤ 1 MiB, batch ≤ 20, eth_call ≤ 30M gas.
- All requests pass RpcGuard; ORIGIN_DENY methods are not served with Origin.

**Errors**
- Standard: −32700, −32600, −32601, −32602, −32603, −32000.
- Custom:
  - −32010 template, −32011 range, −32012 share, −32013 unknown or unsettled.
  - −32014 limit, −32015 type or chainId, −32016 cap or finalized, −32017 outside trie window.
  - −32018 parameters (boot and pocol_getHeaders reasons).
  - −32019 {rule}: client log. rule ∈ items 1–9, net*, viewIncomplete, viewGenesis, viewNoBlocks, viewFuture, viewTargetCeil, recvFit (D100), or gs*.
  - −32020 {reason ∈ {range, resultBytes, resultCount, scanBytes, deadline, scratch, batchBytes}, fromBlock?, lastCompleteBlock?, nextFromBlock?}.
  - −32021 {reason ∈ {busy, rate}, retryAfterMs}.
  - −32022 {reason ∈ {anchorNotCanonical, beyondAnchor}, head}.
- Extension to frame:
  - EIP-1193: 4001, 4100, 4200 {method} (disallowed method, D95), 4901.
  - Bridge-local (D96, D97): −32600 {reason ∈ {size, kind}?}, −32602 {path}, −32005 {reason ∈ {rate, pending, nav, busy, store}} (EIP-1474 code), 4300 {reason: quota, limit} (nonstandard, disclosed), −32603 {reason ∈ {transport, recvLimit, recvDepth, recvParse, storeTimeout}}.

**Trust levels (D13)**
- DEV: anvil.
- LN (default): full verification.
- RP: D77/D82 window with D80, D81 and D84. A remote server may not implement D88–D94 or H2.
  - The extension protects itself through the bridge, BridgeAuth, LogClient, 10s timeout, RecvGuard (D98) and site session (D99).
  - RpcGuard bounds protect the node, not the extension.

A profile and its hash from the same RPC do not establish correctness except in LN.
