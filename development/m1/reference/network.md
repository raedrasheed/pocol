# Node and network interfaces

**P2P**
libp2p over TCP+Noise, with gossipsub and request/response on '/pocol/1/req'.

**Messages**
- Status = [chainId, genesisHash, protocolVersion, headHash, height, totalWork]. An identity mismatch or unsupported version disconnects the peer. Status is informational and does not replace 1b.
  (Note, PR5: the implemented pocold wire is VA-SPEC §1–§2, where Hello carries the sender's maximum wire version and a consensus identity over chain, genesis, rules revision and fork schedule; this M1 reference text is unchanged.)
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

**Mempool fee policy (V3-20 node-local part, N4; not a validity rule)**
- Order. A signed template takes the pooled and parked transactions by priority at the block's base fee, highest first. A transaction's effective tip is min(maxPriorityFeePerGas, maxFeePerGas − baseFee) (a legacy or EIP-2930 transaction's gas price for both caps; tips past 2^64 − 1 wei per gas compare equal); its priority is the lowest effective tip among it and its sender's earlier pooled and parked transactions, so a sender's transactions keep nonce order and a cheap first one holds the later ones back. From the first transaction along a sender's queue whose summed cost (gas_limit × maxFeePerGas + value each) the sender's balance at the head does not cover, the priority is 0: such a transaction (one a new head revalidated, funded by another sender's pending transaction) is tried after its funding, in pool order. Ties keep the pool's order, then the parked transactions' arrival, so equal tips keep the earlier FIFO order and the order is deterministic. The big-first slot (the oldest transaction declaring more than GAS_LIMIT/2, first in its sender's queue) is still tried first, and backlog candidates still come last. The order costs O(n log n) for n ≤ POOL_MAX_TXS + POOL_PARKED_MAX candidates, one head-state read per sender and 12 B of scratch per candidate (A13); the evictions below rank with the same pass. Nonces and fee caps are read from the transactions' bytes, so the pool's entries keep their PR6 sizes.
- Overdraft rule. A new submission (RPC, or a peer's relay) is admitted only if its sender's balance at the head covers it together with the sender's pooled and parked transactions, each at gas_limit × maxFeePerGas + value. Otherwise the answer is −32000 "insufficient funds for gas * price + value with the sender's pooled and parked transactions" (at the gas bound, where it would be parked, the answer stays −32016 "pool full (gas)", as for any transaction that is not held; at the count or byte bound the plain −32016 "pool full" refusal and then the sender quota still come first, before the eviction's checks, so the answer does not depend on the bound). A new submission funded only by another pending transaction waits until that one is mined, so no new submission is pooled whose funds an eviction, a replacement or the fee order could take away before the next block. The guarantee covers new submissions only. Revalidation at a new head (the previous pool, the parked set, transactions returned from abandoned blocks) keeps the pending-state check alone, so a revalidated transaction can depend on another sender's pending funds (for example a funding transfer earlier in the same abandoned block). The fee order ranks it last (Order, above), so it is tried after its funding; an eviction or a replacement of the funding can still leave it pooled but unaffordable, and the selection then skips it until the next head's revalidation drops it.
- Replace-by-fee. A new submission with the sender and nonce of a pooled or parked transaction replaces it in place only if both maxFeePerGas and maxPriorityFeePerGas are strictly higher and each is at least 10 % (POOL_PRICE_BUMP) higher. Otherwise the answer is −32000 "replacement transaction underpriced", followed by the least caps that would replace it. A replacement also needs the engine's pre-execution checks, a fee cap at or above the next base fee and the overdraft rule (with the old transaction left out). The byte bound, the sender quota and (pooled) the gas bound must hold with the replacement in place of the old transaction. A pooled transaction is charged, in the gas bound and the sender quota, the most gas it or any replacement of it declared since the last head, so a replacement declaring less frees no gas until the next head and raising it again charges nothing twice (a sender cannot cycle replacements past its quota of the gas bound); a returned transaction's replacement is charged its own gas. A pooled replacement must also pass the pool's execution checks: it is executed on the head state (the next block's parent) with its sender's nonce at its own and its balance at what the overdraft rule leaves, then checked for its own logs (−32000 "logs would exceed a block cap (LogCount | LogBytes | LogCap)") and for the meter's caps at the pending state's utilisation (−32000 "pending state would exceed a cap (SlotCap | AcctCap | SysKeys)"); nothing is committed. No creation is charged beyond ordinary gas (owner decision 10d, FN-2a; D-V3-21 is superseded). That utilisation counts what the pooled transactions create, the old transaction's own creations included (its effects stay in the pending state), so a pooled replacement is checked as if one more of each unit it creates were taken: a replacement of a transaction creating the cap's last free account or slot is refused (−32000 "pending state would exceed a cap (AcctCap)", or SlotCap) until the next head; the old transaction stays pooled and selectable. A parked replacement is checked like a parked transaction (two reads); under v3, one of its sender's first parked transaction (the only kind that can have evicted, Parked set below) also passes the checks an evicting transaction passes, on the head state and on the pending state, so a transaction cannot evict and then be replaced by one that could not have evicted (for example one that would pass a live cap: −32000 "pending state would exceed a cap (AcctCap)", and the old one stays). The old transaction's effects stay in the pending state until the next head, which re-executes the replacement in its place; the selection executes it on the block's parent. Bounds: at most POOL_REPLACE_SLOT_MAX = 4 replacements per entry and POOL_REPLACE_SENDER_MAX = 8 per sender between two heads; past either the answer is −32016 "replacement limit: retry after the next block". There is no pool-wide acceptance limit, so one sender cannot use up another's. At most POOL_REPLACE_RELAY_MAX = 256 replacements are relayed per head (each offered to peers once); a later one is pooled here but not relayed (a parked one is relayed when admitted at the next head), counted as replaceUnrelayed. A replacement earns no peer credit.
- Lowest-fee eviction. At the count bound (POOL_MAX_TXS) or the byte bound (POOL_MAX_BYTES), a new submission that would be executed into the pending state may evict one pooled transaction. To qualify, its sender has nothing parked, the gas bound has room (the bound in effect is at most POOL_MAX_TXS × GAS_LIMIT, the pending state's term in A13: an evicted transaction's writes stay in the pending state until the next head), its effective tip is above 0 and its nonce is its sender's pending nonce (these cheap checks come before the eviction scan). The victim is the last transaction of another sender with nothing parked, holding enough bytes, with the lowest priority as the fee order ranks it (the lowest effective tip along its sender's pooled transactions, or 0 from the first one its sender's head balance does not cover; Order, above), the latest arrival winning ties. The newcomer's own priority (its tip, capped by its sender's pooled ones) must be strictly higher; otherwise the answer is −32016 "pool full: tip too low to evict", with the lowest evictable priority and the newcomer's. The victim leaves only after the newcomer has executed and passed the pool's checks (the engine, its own logs, the meter), so an invalid newcomer, or one that would pass a live cap, evicts nothing. Under v3 a newcomer that would evict is also run once more, on the head state (the next block's parent), through the checks a pooled replacement passes (the engine, its own logs, the meter at the pending state's utilisation), with its balance at what the overdraft rule leaves; if they fail, nothing is evicted, the refusal counts in evictRefused and the answer is their refusal (for example −32000 "pending state would exceed a cap (AcctCap)"). The pending state can hold writes no block will make (an evicted or replaced transaction's, which stay until the next head), so there a creation can look done and use no cap room; on the head it is checked without them. The head state lacks what the pooled transactions ahead of it write, its sender's own earlier ones included, which its execution on the pending state has; it evicts only when both pass. So it is checked on those two states only: the effects of another sender's parked transaction that a block takes before it are not modelled (one whose cap use depends on them can still evict, at the cost of that transaction's inclusion), and a follow-up to its sender's own pending creation (for example a 21000-gas transfer to an account its earlier pooled transaction creates) is checked on the head as a creation, so at the bounds near a cap it can be refused (AcctCap or SlotCap, where the pending utilisation already counts the creation) until the next head mines its predecessor, though below the bounds it is admitted and selected. An RPC submission runs both executions within its one submission deadline (−32020 "compute deadline (not pooled)" when they do not finish). Below the bounds a newcomer is still admitted on the pending state alone, so one checked against such a write can be pooled and then skipped by the selection, until the next head's revalidation refuses it; it holds pool room and gas-bound room, but takes none from another transaction. The victim's nonce goes back to its sender in the pending state (its other effects stay until the next head, whose revalidation drops it): its sender's pending nonce (eth_getTransactionCount "pending") no longer counts it, a later nonce is refused rather than pooled and dropped, and the sender can submit the nonce again at a priority that outbids the pool's lowest. A newcomer that would be parked at the count or byte bound is refused −32016 "pool full", as before.
- Where eviction applies. Pooled transactions are evicted at the count and byte bounds only. Under the default limits the gas bound (POOL_GAS_BLOCKS × GAS_LIMIT, about 1,523 plain transfers) binds before the count bound (4,096): evicted gas stays counted until the next head, so an eviction cannot free gas-bound room, and a newcomer at the gas bound is parked instead. Fee competition there works through the parked set: its candidates are fee-ordered with the pooled ones, and a full parked set evicts by priority (next item).
- Parked set. When the parked set is full (by count or bytes) and PR6's rules (most declared gas; the last entry of the longest run) make no room, a new submission takes the room from parked transactions of other senders ranked strictly below it (a parked transaction ranks at the lowest effective tip among its sender's pooled ones and its run up to it, or 0 from the first one its sender's head balance does not cover; Order, above), lowest first and the latest of equals first, so what stays of each run is still a run. The room is planned in one pass first; only when those candidates free all of it are they evicted. Otherwise nothing is evicted and the answer is −32016 "pool full (gas)". A parked transaction is not executed (two reads), so the live caps are not checked when it is parked; under v3, before a transaction evicts anything from the parked set (by PR6's rules or by priority, a new submission or one revalidated at a new head), it is run through the checks a pooled replacement passes (the engine, its own logs, the meter), once on the head state and once on the pending state, with its balance at what the overdraft rule leaves (a revalidated one: its pending-state balance less its sender's other parked transactions); it evicts only when both pass. If they fail, nothing is evicted and the answer is their refusal (for example −32000 "pending state would exceed a cap (AcctCap)"). Neither state has a parked transaction's effects, so under v3 a transaction behind its sender's parked ones (a run) evicts nothing: the answer is −32016 "pool full (gas)", whatever it would pay, and a revalidated one is dropped with its sender's later ones, as one finding no room is. It is checked on the head state and the pending state only: a transaction that would pass a live cap evicts nothing unless its cap use depends on another sender's parked transaction a block takes before it (not modelled; such an eviction costs that transaction's inclusion), and a follow-up to its sender's own pooled creation is checked on the head as a creation, so near a cap it can be refused at a full parked set until the next head (Lowest-fee eviction, above). Refusals by these checks count in evictRefused. An RPC submission runs both executions within its one submission deadline. A transaction that would pass a live cap can still take free parked room (and is relayed then, within the parked relay bound); the selection skips it, and the head that admits it refuses it. Until then it holds that room: the gas and run rules cannot evict a 21000-gas transaction (a newcomer declares no less, and it is no run), and the fee rule evicts it only for a newcomer of higher priority, so while the gas bound is held such transactions at a high tip, which pay nothing, keep honest lower-tip arrivals out of a full parked set (a revalidation re-parks them while the gas bound is held, and the head that admits them into the pool refuses them).
- Unchanged: the gas bound (evicted gas, and a replaced transaction's most declared gas, still count until the next head, so the pending state stays within it; a pool_gas_max above POOL_MAX_TXS × GAS_LIMIT applies as that product), the sender quota (−32016 "pool full (sender gas)"), parking (−32016 "pool full (gas): held for the next block") and its relay bound, POOL_MAX_BYTES, POOL_MAX_TXS and the D-7 revalidation order. Revalidation at a new head (the previous pool and parked set re-queued, abandoned blocks) replaces nothing and evicts by fee nothing; only a P2P transaction that arrived during revalidation is a new submission.
- Status counters (`pocol_status` stats): poolReplaced, replaceRefused, replaceRelayed, replaceUnrelayed, poolEvicted, evictRefused (pooled evictions refused, and under v3 parked evictions refused by the eviction checks or the run rule), parkedFeeEvicted, overdraftRefused.
- eth_estimateGas (V3-24) models the log caps of the next block (`user_phase::log_caps`): 256 KiB / 4,368 under v3, otherwise 32 KiB / 546, plus LOG_CAP when H_END ≠ 0. A call whose own logs pass one of them would halt with all its gas in every block on that parent. It gets −32000 "execution would halt (LogCount | LogBytes | LogCap)" with {reason}, and the pool refuses it the same way. eth_call is unchanged.

**P2P and node resource limits**
- ≤ 50 peers by default (32 established inbound; ≤ 18 outbound: configured peers, ≤ 8 automatic and 1 spare; the 8 handshaking places are not peers); an operator may raise inbound (`--peers-max` ≤ 128) within A13 and its task limit. Queue ≤ 256, ≤ 16 per peer.
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
- eth_getTransactionReceipt: ≤ 2·LOG_BLOCK_MAX+546·512 B+2 KiB ≈ 339 KiB. Under a profile that can activate v3 (V3-24: 256 KiB / 4,368 logs per block, all of which one transaction may emit) the worst receipt is 2·262144+4368·512 B+2 KiB ≈ 2.63 MiB, above LIGHT_RESP_MAX: the node then serves eth_getTransactionReceipt as heavy (heavy lane, reservation and deadline, RESP_MAX answer limit), and v3 R26 requires RESP_MAX ≥ that bound. Trade-off: under v3 every receipt request (which wallets poll often) takes a HEAVY_QUEUE place, a RESP_MAX reservation from HEAVY_RESP_POOL and the heavy deadline, so receipt polling competes with eth_getLogs and eth_call and is answered −32021 {busy, retryAfterMs} when the heavy lane or pool is full; v3 clients must treat it as heavy-class and back off on −32021. Spec-1 profiles keep this light row.
- eth_getBlockBy* with hashes: ≤ 2 KiB+(BODY_MAX/85)·70 B ≈ 160 KiB.
- pocol_getHeaders with count ≤ 64: ≤ 64·(2·3067+16) B ≈ 385 KiB.
- pocol_getParams: ≤ 2·|genesisPre|+4 KiB, thus ≤ 92 KiB in end/full.
- pocol_getAssignment, pocol_getMinerSet and pocol_getTemplate: ≤ 1024·160 B = 160 KiB.

Heavy (RESP_MAX = 8 MiB): eth_getLogs, pocol_getLogs, eth_call, eth_estimateGas, eth_getProof, eth_feeHistory, pocol_getHeaders with count > 64, eth_getBlockBy* with full transactions, pocol_getRejects, and eth_getTransactionReceipt under a v3-capable profile.

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

**Template latency (V3-05, node-local telemetry)**
- pocol_templateLatency → {local, tracked, trackedMax, evicted, bucketsMs, all, proposers[]}; the same object is pocol_status.templateLatency. Heavy class (unlisted method). Informational, not validity: no production, import or fork-choice decision reads it.
- Slot: an attempt-0 slot this node waits in, i.e. a parent it produces on whose Draw(h,0) is another member. Recorded when the wait starts, with b_j (the parent's first local receipt, the time T_j uses), L(0) (the attempt window's first second, ×1000) and T_j = max(b_j+Δ_rel, L(0)+dFbWait). A slot is counted once however often production restarts on its parent. The node's own templates are never timed.
- Arrival: the first copy of a signed a=0 template for a recorded slot whose signature recovers to the slot's Draw(h,0), whenever it arrives and whatever happens to it next (it may be late, or fail to lock). Later copies (relays) are not samples. A template that arrives before its parent's production started waits in the template buffer (one per parent) and is timed at slot start: an identical later copy keeps the first copy's receipt time, but a different template for the same parent (an equivocation, or an unsigned or junk one from any peer) replaces the buffered copy, and the slot is then timed at the replacement or, if its signature is not Draw(h,0)'s, counted missing. So a peer can inflate `missing` for slots whose template outran its parent; a reader should treat `missing` together with `early`, and with the fallback count, not alone. (Checking every buffered template's signature to avoid this would let any peer buy one signature recovery per frame: at buffer time the parent, and so Draw(h,0), is often not known yet.)
- Lock: the time the node locked that template after the §4(a) checks (signature, window, item 16, rebuild), once per slot.
- Per proposer and in `all` (every proposer, evicted ones included):
  - slots; received (arrivals); missing = slots − received (slots whose template never arrived: the fallback ratio's numerator); locked;
  - late: arrivals after T_j (the template could not lock; the node was on the attempt-0 fallback or later);
  - early: arrivals before b_j (the template outran its parent);
  - stampAtWindow: arrivals of a template stamped exactly L(0) (see sinceStampMs);
  - sinceParentMs = arrival − b_j (0 when early); lockMs = lock − b_j;
  - sinceStampMs = arrival − ts·1000 and beforeStampMs = ts·1000 − arrival, where ts is the template's own header timestamp (the proposer's clock, in whole seconds, when it built the template). Each arrival is in exactly one of the two: sinceStampMs when it is at or after its stamp by this node's clock, beforeStampMs when before (this node's clock is behind the proposer's by more than the delivery took). Nothing is clipped: received = sinceStampMs.count + beforeStampMs.count;
  - each histogram is {count, p50, p90, p99, max, mean} in milliseconds, null without samples.
- Quantiles come from fixed buckets with upper bounds bucketsMs (50 ms … 120 s, then one open bucket): pXX is the upper bound of the bucket holding the sample of rank ⌈XX·count/100⌉, capped at the exact max. It is never below the exact sample percentile and at most one bucket width above it. max and mean are exact (saturating at 2^32−1 ms).
- local: the node's Δ, φ, t_exec, Δ_rel, Γ and dFbWait, so a reader can recompute T_j.
- Bounds: at most trackedMax = 256 proposers (the least recently drawn is evicted, counted in evicted) and 64 recent slots; latency::BOUND_BYTES = 147,024 B on x86-64, asserted below 160 KiB at compile time and listed in the A13 conditions (telemetry-inventory), not as a term.
- Reading Δ and φ for V3-06: sinceParentMs covers the proposer's own receipt of the parent, its template build and the delivery (what R2's σ+Δ+2t_exec must cover), so zero late arrivals with p99 below Δ_rel·1000 show that Δ_rel holds at the measured locals. The stamp offset (sinceStampMs, or −beforeStampMs) is arrival − ts·1000 = build time + delivery + (this node's clock − the proposer's clock) + the sub-second part of the proposer's clock at the build (ts is truncated to the second, 0 to 1000 ms, about 500 ms on average). It is the φ-bearing quantity: for a pair of nodes, half the difference of the offsets each measures for the other's templates cancels build, delivery and truncation when they are about symmetric, and leaves their clock offset; beforeStampMs samples exist only where the receiver's clock is behind the proposer's. The offset is measured against the proposer's clock only when its stamp is past L(0): a proposer may sign up to Φ s before L(0) and then stamps L(0), not its clock (consensus.md:97, ts = max(L(0), C)), so a sample in stampAtWindow may sit up to Φ s below the true offset. In steady state L(0) = parent.ts + 1 is about one block round in the past when the template is built, so stampAtWindow stays near 0; read its count before the stamp histograms. (L(0) itself is no clock reference: b_j − L(0) is about the parent's PoW round, about T_blk, which is why no metric is taken relative to it.) Every time is the reporting node's local clock; compare the same metric across nodes, not raw times.

**Mining threads and local work (V3-30, D-V3-30, node-local)**
- `--mine-threads N` (1..64, default 1): the miner scans this node's own nonce range [start, end] of each locked template with N threads. With L = end − start + 1, thread i scans [start + ⌊i·L/N⌋, start + ⌊(i+1)·L/N⌋ − 1] (`miner::slices`): the slices are disjoint, cover the own range and depend on the range and N only; a slice that would be empty (N > L) is left out. Every hash stays inside the member's own range (P4/P5). The first solution, found by a thread or submitted through pocol_submitWork, stops every thread and the miner is joined; one exhaustion is reported once every slice is scanned. All threads are joined before the next template's miner starts. A13 counts each thread at IO_STACK + 4 KiB (`resources::io_threads`) and the task inventory one task each; N = 1 is the bound and inventory of before.
- Core count: the threads are CPU-bound and run at the consensus thread's priority. More of them than the host's cores delays the consensus thread (template validation, lock, relay, RPC), which eats into the δ / δ_rel budget: on a 4-core host, 64 threads raised pocol_status p95 from 7 ms to 146 ms (max 420 ms) and a template switch from 1-2 ms to 15 ms, while 4 threads stayed as fast as 1. Use at most the host's cores less one (one core for the consensus and network threads). A larger setting still runs, with a warning on stderr at start and in pocol_status.mining.warning.
- The work port (`--work-rpc ADDR`, off by default): a listener of its own, bound to a loopback address, that serves pocol_getWork and pocol_submitWork and nothing else (−32601 "the work port serves pocol_getWork and pocol_submitWork only"). The main RPC port never serves them (−32601 "method served on the work port only" with a work port, an unknown method without one), so a reverse proxy in front of the main port, such as the public-testnet nginx of deploy/public-testnet/prepare.py (which reaches the node from 127.0.0.1 without an Origin), never reaches them. Never put a proxy in front of the work port. Both are in ORIGIN_DENY (never with an Origin header, so never through the site bridge), and the work port refuses a connection whose peer is not loopback (defence in depth). Its connections are at most WORK_CONN_MAX = 4 and count among the main port's conn_max (`rpc::serve_all`), so the A13 connection terms are unchanged; the work port adds its acceptor thread (one task, IO_STACK + 4 KiB). Heavy class (unlisted methods). The methods run on the consensus thread like the in-process miner's events: no new thread.
- pocol_getWork [index, count]? → {templateId, header, height, attempt, fallback, rangeStart, rangeEnd, slice, sliceStart, sliceEnd, target, shareTarget, nonceMax, solved, minerRunning, ownShares, powHash}. The locked template and this node's own range of it; with [index, count] (count ≤ 65536) the slice `index` of `count` disjoint slices of the own range (computed alone, `miner::slice`; the same cut as the threads'), so several external workers never overlap each other. They do overlap the node's own miner threads (minerRunning of them), which scan the same own range at the same time: external work adds to the node's only where it reaches a nonce first, and an external worker duplicates the internal miner's hashes otherwise. A worker hashes sha256(templateId ‖ be64(nonce)) for nonces of its slice and submits each hash ≤ shareTarget. Quantities are 0x-hex. −32010 without a mining key or a locked template, −32011 without an own range or for an empty slice, −32013 once the node's miner reported the whole own range scanned for this template (it holds no solution: work on it is wasted until the next template, the post-range energy rule), −32602 for index ≥ count, count 0, count past 65536 or more than two parameters.
- pocol_submitWork [templateId, nonce] → {accepted, kind ∈ {share, solution}, reason?, imported?, pending?}. Checked in order: a mining key and a locked template, templateId is the locked one (otherwise −32010, stale work: a new parent, attempt or template replaced it), the nonce is inside this node's own range (otherwise −32011: a foreign range is refused), and its hash is ≤ shareTarget (otherwise −32012). A hash ≤ target is the solution: the miner is stopped and joined, and the block is sealed, signed and imported as the node's own (imported), or waits for its timestamp (pending). Otherwise it is a share, kept and relayed like the miner's. The node holds at most S_max own shares per template from its miner and this method together (`Production::own_shares`): a block credits at most S_max, and own shares past that would only take peers' relay places (SHARE_RELAY_PER_S_MAX · S_max per template for all members). {accepted: false, reason ∈ {duplicate, shareCap, solved}} for a share already held, one past the cap, or a solution after the template was solved. Effectful: its answer is never replaced by a late −32020. More than two parameters: −32602.
- pocol_status.mining → {threads, running, cores, warning, workRpc, workShares, workSolutions, workRefused}: the configured thread count, the threads of the running miner (0 once the template is solved or the own range exhausted), the host's cores (std::thread::available_parallelism, 0 when unknown), the core-count warning or null, the work port's address or null, and what the work interface accepted and refused.
- pocol_status.networkHashrateEst → {hashesPerSecond, blocks, fromHeight, toHeight, seconds, work, window} | null: the summed work of the last window = 360 canonical blocks (each block's 2^256/(target+1), the expected hashes of the whole membership per solution; fewer near genesis or below the in-memory window) over the seconds between the window's first and last timestamps, rounded down. Null with no block or no time in the window. Informational, never read by production, import or fork choice.

**No creation burn quote (FN-2a)**
- pocol_quoteCreationBurn is removed with the D-V3-21 creation burn (owner decision 10d of 2026-11-13: new accounts, slots and code pay ordinary gas only). Every node answers it −32601, exactly as for an unknown method. eth_estimateGas needs only the executed gas; a v3 estimate still answers −32000 {reason} for a call that would halt on a live cap (AcctCap, SlotCap, SysKeys) or on the log caps.

**Local finality (V3-26, D-V3-26, node-local retention, never validity)**
- finalized_h is this node's local irreversibility floor: the highest head − r_keep it has passed. It only rises: a heavier but shorter branch lowers the head, and with it head − r_keep, but not finalized_h (safety review finding 5). The retention floor is max(head − r_keep, finalized_h): a non-canonical block at or below it is removed by retention, and a new one is refused before validation or journaling (Evicted, never invalid, no penalty), so the canonical block at finalized_h is never reorganized away by this node. It is a local retention policy like r_keep, excluded from the consensus identity; no block validity, D133 comparison or persisted consensus byte changes.
- Persisted beside the journal in `retention.finalized` (one line: height and canonical block hash, written by temporary file, fsync, rename, directory fsync); the journal and checkpoint formats are unchanged. At open, journal replay runs as before (R_keep floor only, so a journal written by any build replays to the same head), then the floor is max(the file's height, head − r_keep), the file's height used only when its block is canonical in this journal (a file left by another journal, or by an older build that reorganized below it, is ignored, logged, shown as pocol_status.retention.finalityFileIgnored and replaced). A file that cannot be read or does not parse stops the open (exit 1, naming the file) rather than being overwritten; a canonical index that cannot be read at the file's height leaves the file unused for that run but kept. A data directory without the file (written before V3-26) opens at head − r_keep and the file is written.
- Durability: the file is rewritten every time finalized_h rises, after that import's retention and before the next block is journaled, so it always holds the floor every journaled block was imported under. A block that lowers the head (a heavier, shorter branch) never raises finalized_h, so the file already holds the floor it keeps; a crash before a rewrite loses only a rise that replay re-derives as head − r_keep. A failed write stops the node (exit 70), as any local storage failure does. pocol_status.retention.finalizedPersisted is the height the file holds.
- Clearing the floor (operator): stop the node and remove `<data>/retention.finalized`; the next open uses head − r_keep. This is the way to let the node follow a heavier branch that forks between head − r_keep and finalized_h (there is no flag for it); a fork deeper than R_keep stays governance.md:200 (procedure 5, manual checkpoint). Until then, sessions with a peer on such a branch make no progress (its blocks are refused as Evicted), which costs bandwidth, not safety.
- RPC tags: `finalized` = finalized_h; `safe` = max(head − k_safe, finalized_h), k_safe = 6 by default (owner decision 12, 2026-11-13: one minute at 10 s blocks; `--k-safe N`, or POCOL_K_SAFE for `run` and `pocol start` when the flag is absent; node::K_SAFE). `latest` and `pending` are the head. State queries (eth_getBalance, eth_getCode, eth_getStorageAt, eth_getTransactionCount, eth_call, eth_estimateGas) at a tag deeper than the trie window answer −32017 as for any height; the open refuses k_safe ≥ K_hist (K_hist = 32 comes from the network's local profile, not from a node flag), so the state at `safe` is inside the window and answered while safe = head − k_safe. When a heavier, shorter branch lowers the head to within k_safe of finalized_h, safe = finalized_h, and with R_keep ≥ K_hist (the production case) its state was pruned at the old head and is not restored: a state query at `safe` then answers −32017 as at `finalized`, and block queries still answer. Meanwhile `finalized` (head − R_keep or above it, below the window once the head passes R_keep) answers block queries (eth_getBlockByNumber) and −32017 for state, as before.
- Reorg alarm (owner decision 12): a separate setting k_alarm = 60 by default (ten minutes at 10 s blocks; `--k-alarm N`, or POCOL_K_ALARM for `run` and `pocol start` when the flag is absent; node::K_ALARM). The open refuses a pair outside 1 ≤ k_safe ≤ k_alarm (and k_safe < K_hist), naming the setting.
- pocol_finality → {head, safe, safeHash, finalized, finalizedHash, kSafe, kAlarm, rKeep, retentionFloor, reorgs, maxReorgDepth, reorgsGeKAlarm, networkHashrateEst, model: "probabilistic", scope: "local"}. Heavy class (unlisted method). maxReorgDepth is the most canonical blocks one reorganization abandoned (old head height − fork height) since this start, reorgsGeKAlarm the reorganizations that abandoned k_alarm or more (the alarm counter; it was reorgsGeKSafe, counted at k_safe, before owner decision 12); pocol_status.retention carries kSafe, kAlarm, finalized, floor, maxReorgDepth and reorgsGeKAlarm. networkHashrateEst is the same object as pocol_status.networkHashrateEst (one definition, V3-30 S3: the work of the last HASHRATE_WINDOW canonical blocks over their seconds; indicative only, timestamps are proposer-chosen).
- What it is not: not mathematical or committee finality, and no signed checkpoint. Another node (a new one, one restarted from a data directory without the file, or one that saw different branches) can hold a lower floor and follow a heavier branch below this node's finalized_h; this node then stays on its chain until an operator acts (clearing the floor above, or governance.md:200, manual checkpoint, for a fork deeper than R_keep). Lowering R_keep is not a finality mechanism (V3-R).
- Indicative confirmation depths (Nakamoto's model, Bitcoin paper §11: an attacker with hash share q catches up from z blocks behind; it assumes independent single-block PoW, not PoCol's template, share and fallback structure, and counts blocks where fork choice counts work). Indicative only, never a guarantee:

  | q | z = 6 (k_safe) | z = 60 (k_alarm) | z = 2048 (R_keep) |
  |---|---|---|---|
  | 0.1 | 2.4e-4 | 4.4e-35 | < 1e-300 |
  | 0.2 | 1.4e-2 | 1.4e-17 | < 1e-300 |
  | 0.3 | 1.3e-1 | 3.7e-8 | 2.2e-246 |
  | 0.4 | 5.0e-1 | 8.3e-3 | 3.7e-65 |

**Local RPC**
- 127.0.0.1; disallowed Host returns 403; Origin absent or chrome-extension://<ID>.
- Body ≤ 1 MiB, batch ≤ 20, eth_call ≤ 30M gas.
- Block tags: `latest`/`pending` the head, `safe` max(head − k_safe, finalized_h) with k_safe 6, `finalized` the local floor finalized_h ("Local finality", V3-26). A state query at `safe` is answered while safe = head − k_safe (the open refuses k_safe ≥ K_hist = 32); when a heavier, shorter branch makes safe = finalized_h, it answers −32017 as at `finalized`; at `finalized` it answers −32017 once the floor is below the trie window.
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
