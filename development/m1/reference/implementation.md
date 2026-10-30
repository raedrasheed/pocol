# Development plan

**Environment**
Windows 10 with 8 GB, without Docker, WSL or administrator privileges.
- Dedicated ports/directories, without stopping the existing work environment.
- Test Chrome with a separate user-data-dir.
- Two anvil instances (approximately 200 MiB each).
- RpcTap local Node proxy ≤ 100 MiB.
- Local CanarySink. Firewall permission may be requested; the user decides, with a second machine as the alternative.
- Single-threaded FixtureMiner ≤ 200 MiB.
- RpcAttacker ≤ 300 MiB; measure the node's RSS separately.
- fetchEach in Node ≤ 300 MiB.
- PacerSim in Rust, without networking.
- BR19a and BR19b in Node, without browser or networking.
- Optional portable Python for the third library, reference, SchemaRef, BridgeRef and SiteStorageRef; if unavailable, use another machine and disclose it.
- BR18e and BR18f need no actual external connection: TestHooks intercepts chrome.tabs.create and records the URL without loading it.
- BR17e writes approximately 1 MiB to chrome.storage.local only in the test user profile.

**License**
Apache-2.0, with MIT, Apache or BSD dependencies. Foundry and solc are build tools only.

**Stages**
- M0: versions, licenses (including hyper, socket2 and dependency-cruiser), anvil, canary, three keccak libraries with K1–K3, fixed-width library, audit rpc-fault and its three faults, bridge-fault and its thirty-two faults listed in governance (M0 output), and verify chrome.storage.local.getKeys availability in test Chrome.

Phase A (anvil; establishes nothing about PoCol):
- M1: M1-spec, then ChunkFactory, Website and publisher tool. Tests: T1, T16.
- M2: MV3 extension including:
  - DnrGuard, ProfileMutex, RtcLockdown, wallet.
  - BridgeAuth, BridgeBucket, RpcReadClient (with pendingReads), WalletSubmit, SiteStorage (with quota accounting), Navigator, BridgeTrace.
  - CanarySink, HeaderNetCheck, LogClient (with SinkChecker and counters).
  - RpcTap (with hold), BadSite, GoodSite, BridgeRef with SiteStorageRef.
  - Tests: BG1, BR19a, BR19b, BR20a, BR20b and MR-unit first; then SiteStorage unit tests on BR17e with a fake dictionary; then T11a, T12, X8, BR1–BR14, BR16–BR18, BR19c, BR20c, BR20d, MR1–MR12, BR21a–BR21f, BR22a–BR22h, BR23 and BR24 (with EpochFence, RecoveryGate, ReadSlots, RecoverySlots, SeqAlloc, DiskGate, DiskLedger, AdminDelete, AdminSlots and SR-model), BR-neg, BR-dep, RW-unit, HC-unit, LC-unit (LC1–LC18).

Phase B (pocold; actual PoCol integration):
- M3: GenesisSpec, ParamGate (R13, R16 with Lmax/LAG, R20, R26 with derived values/TIMER_EPS), NetConfig, Codec, NetId, Timing, Difficulty.asert, Draw, partitioning, TS model. Tests: K1–K3, L4, T22, T0, AS1–AS19, AS-eq, ASQ, ASB, TJ, PG-END1–5, PG-L, PG-RPC, CF1–CF6, SV1–SV7s.
- M4: revm with PAY, GenesisBuilder (up to 1024), phases, CapsTracker, TipsAccumulator, system contracts, Build_fb, ExecFixture. Tests: T2, T2b, CI1–CI3, GB1, GB-END, L4s, T2c, T24, T26.
  - (D141, D142) BlockMeta.parentResult(hash) → {blockHash, winner, shares[(n, owner)]}.
    - Derived by ParentResult.derive(HeaderStore.header(hash), Assigner.tableFor(hash)) after items 13 and 14 pass, stored by hash.
    - Reexec.verifyParentResult(hash) rederives during sync, TrieRebuild and reorg, comparing stored data; mismatch is parentResultMismatch.
    - Only recordShares(h−1) calls ShareRecorder.record(ctx.parentResult).
  - Executor.apply takes ctx.parentResult and cannot accept the current block's nonce or owner; its interface type enforces this.
  - M4 automated checks add two conditions: closeBlock does not write k=2 for pending[h]; recordResult is called only from SYSTEM_ADDRESS.
  - Test-only exec-fault:winnerInState build, absent from release.
- M4b: FixtureMiner, FixtureVerify. Tests: FX0, SV8, SV8-fb, SV8-cap, FBX, SV9, SV9-final, NX (a), TW1–TW5 with winnerInState, sharesFromFirstSeen, sharesFromMiner and sharesDropped controls (D141, D142). FixtureMiner accepts plan.shareOwners to search specified ranges for shares on a prebuilt UT.
  - Threshold (D143) computed by FixtureMiner.shareThreshold(UT) = min(2^256−1, UT.target·CP.m), calling the consensus function itself, not a plan constant.
  - plan.gapShare requests a nonce whose hash lies in (T_share, supplied upper bound] for TW5-gap.
  - Test-only fixture-fault:fixedShareThreshold build, absent from release. FixtureMiner accepts plan.searchOwners = [w, x] on one prebuilt UT.
- M5 (code must not start before VA-spec review, D115): P2P, TemplateFSM, sync, SimNet with NetSim, AdvNode and TraceDB.
  - ChainTimer (D123): schedule(tag ∈ {chain, nonChain}, at, job), pending(tag). The sole scheduler in consensus, synchronization and import modules.
  - A clippy disallowed_methods rule prohibits direct tokio::time::sleep, interval and timeout use in those modules; violations fail CI.
  - SimNet.chainWork(node), SimNet.purgeFuture(entry), SimNet.statusSweep(), simnet build only.
  - (D124) SimNet.bodyHolders(hash), SimNet.purgeUnavailable(tAvail), SimNet.drainBound(run) → {N_miss, c_b, DRAIN_MAX}, simnet build only.
  - (D125) In simnet builds only:
    - SimNet.recSet(tAvail) → {rec, excluded[{hash, reason, missingAncestor?}]}.
    - SimNet.need(i) → {blocks, branches[{forkDepth}]}.
    - (D126) DrainPin.haltPin() within the atomic halt step; DrainPin.add(item) for every subsequent honest arrival; DrainPin.narrow(rec) at t_avail in the same U step; DrainPin.stats → {pinRefusals, pinBytes, pinEntries}.
    - (D126) ActLog.record(node, class, durMs) through ChainTimer; BudgetCheck.run(actLog, bounds) → ok | {class, node, actual, bound}; HeadCheckRef.heldAtHalt(log).
    - evictProbe(node, hash) test hook.
    - drainBound returns {need, K, Kmiss, Kpend, S, T, dStar, HR, RB, waits{restart, eligibility, preHalt}, DRAIN_MAX}, matching HeadCheckRef (D126, D127).
    - (D127) SimParams.load(vector) is one record read by NetSim, drainBound and HeadCheckRef; BudgetCheck.table(run) returns each class's row with count, duration and component.
  - (D127, D128) In pocold:
    - HdrCache (D129), bounded by sync queues, containing:
      - BlockCache keyed by blockHash for completed committed-content verdicts.
      - EnvCache keyed by hdrId for envelope verdicts.
      - inFlightHdr with both keys.
      - HeaderStore.accept(blockHash, hdrId) retains the first envelope-valid copy.
      - EnvClass.classify(rule, part) → env | block.
      - EnvCheck.pre(hdr) → envOk | envReject, stateless. EnvCheck.own(hdr, parentState) → envOk | envReject, without apply, caching assignment per parent (D130).
      - EnvCache states {envPending, envOk, envReject}; the verdict after own is final while retained (D136). Interfaces:
        - admit(hdr, solicited, peer) → check | dropFlood | dropKnown.
        - record(x, hdrId, rule, peer) evicts x's oldest verdict or the globally oldest verdict.
        - purge(x) on import, envOk, blockReject or removal.
        - stats → {envRejEntries, envFloodDropped, envEvicted, envMemBytes}.
        - pin() in simnet builds as part of HaltPin.
    - FetchSched interfaces:
      - DiscRun.onStatus(peer, head) → start | markDirty.
      - DiscRun.next() → {peer, locator} | done, marking tried.
      - BodySched.request(hash) → peer | waiting, holding inFlight and triedPeers.
      - BodySched.onReply(hash, peer, NotFound | Mismatch | Ok).
      - BodySched.onStatus(peer) and BodySched.onAnnounce(peer, hash) put the peer in reeligible without removing it from tried (D130).
      - BodySched.cycleReset(hash | all) clears tried, reeligible and timers; called by SimNet at t_avail.
      - BodySched.pick(hash) → untried in ID order | reeligible | waiting.
      - Backoff with BODY_RETRY_BASE = 1s and BODY_RETRY_MAX = 60s; untried preferred (D129).
      - HfullDone.has(hash).
      - (D131) EnvFetch:
        - request(hash) → peer | waiting.
        - onReply(hash, peer, Copy(hdr) | NotFound | Timeout | Oversize | Disconnect); every outcome calls EnvSlots.release once (D137).
        - EnvQueue (D137): enqueue(hash) once per x, head(), requeueTail(hash), remove(hash).
        - EnvSlots (D137):
          - grantSolicit(hash) → Slot{reqId, peer, buf 3131 B} | none, in the sending step itself.
          - acquireGossip(hdr) → Slot | none.
          - cancel(slot, attemptId, reason) (D138): marks cancelRequested. In reserved/buffered, resets the substream and releases immediately; does not release in checking.
          - onCheckSettled(slot, attemptId, verdict): sole event releasing a checking seat, applying network's cancelled-check-result rule.
          - release(slot, attemptId, reason): internal, with a duplicate-prevention flag. No-op unless attemptId matches the current attempt.
          - Timeout timer cancelled on entering checking. Timer and response events use one loop; ties favor the response.
          - stats adds {liveChecks, liveBuffers, cancelPending, lateCheckDropped, envCheckOverrun}.
          - CheckGate.hold/settle is a simnet-only hook for N12c and N12r.
          - EnvCheckPool (D139): submit(slot, attemptId, hdr) with FIFO queue and W dedicated threads; stats → {queueLen, spanMaxMs, envCheckOverrun}. FakeExec in simnet sets c_exec per check (N12p, N12h, N12w).
          - RecoveryRef (Python): conditions(trace) → {t0 | none, preCondWait, unmet ⊆ {G1, G2, G3, G4}, verdict ∈ {withinBound, boundViolated, noGuarantee}} (D140), with ref-fault:t0Unconditioned. bound(P, Q, B, T_role) → D_x; sequence(peers, replies) → expected request order, with ref-fault:recoveryOldBound.
          - onLateReply(reqId) → unsolicited admit path.
          - stats → {solicitHeld, gossipHeld, releaseCount, doubleReleaseCount, timeouts}. Any nonzero doubleReleaseCount fails.
        - onStatus, onAnnounce and cycleReset as in BodySched.
      - GetEnvelope.serve(hash, exclude) → HeaderStore.validCopy(hash) | NotFound.
      - EnvWait.has(hash), ENV_PENDING_MAX = 4; D136 constants (ENV_REJ_PER_BLOCK, ENV_REJ_GLOBAL, ENV_PENDING_GLOBAL, ENV_INFLIGHT_GLOBAL); D137 constants (ENV_SOLICIT_SLOTS, ENV_GOSSIP_SLOTS, ENV_REQ_TIMEOUT, ENV_REPLY_MAX). T23 N11–N11c, N12, N12t and N12b tests in M5.
      - SimParams.rtt(i, j), rttMax(i) (D131); ActLog records envFetch as a hdrFetch subclass with each request's duration.
    - ActLog records (hash, peer) per request and the run ID per run.
  - In pocold: ChainWork keyed by block hash; RejectPath.cancel(hash) cancels the entry, descendants and timers in the rejection step, called only for H-pre/H-full blockReject on a commitment-matching body. envReject uses EnvRejectPath.drop(hdrId, peer) without cancellation (D130). BodyFetch.onMismatch(peer, hash) penalizes the peer and retries without cancellation (D125). NotFound immediately moves fetching to another peer. Tests: T3, T4, T5, T6, T9, T20, T21, T23, SV8-reorg, SV8-tie, SV9-reorg, NX (b, c).
- M6 (requires VA-spec, D115): shares, settlement, evidence, closing with End.bound, BurnMonitor, EndFixture, adversarial policies in T7b, T7c and T7e. Tests: T7, T7b, T7c, T8, T17, T27, CL-END, CL-OPS1–2.
- M7: hyper RPC server:
  - First SendPacer with PacerSim; pass PS1–PS8 before any socket test.
  - Then RpcGuard, ComputePool, CountingAlloc, RateBucket, LogsQuery, pocol_getLogs, BatchStreamer, SO_SNDBUF, PacerTrace, TraceCheck (with DiscriminationCheck), OriginDeny.
  - LN and RP window, logs/dense, DenseFixture (B, A', Blob), RpcAttacker, ReferenceLogs.
  - Tests: PS1–PS8, HG1–HG16, RG1–RG15 with RG2b, RG3-bridge, RG3b and variants; RG14(0)–(xii) with (0b) and perItemGrace control; T11b, T11c, T15, BR15, NX (d), NX-V1–V6 with V3b–V3g, NX0d, NX1d, LN0.
  - M7 cannot pass without PS, RG and BR15.
- M8 (requires VA-spec, D115): RetentionStore, TrieRebuild, LoadGen, ReplicaHarness for T25. Tests: T10, T10b, T10c, T13, T14, T18, T18b, T19, T25, T4-V.

**Interfaces — Consensus**
- GenesisSpec.encode/decode/verify → GsError{code, path}.
- ParamGate.check/derive: derive exposes Lmax, LAG, RPC_MEM, BATCH_SEND_MAX, its monitoring bound and the minimum BATCH_WALL as exact rationals.
- NetConfig.load, activeVersion, NetId.check.
- Difficulty.asert(pTs u64, pH u64, cp) → {target U256, trace{deltaT i128, e i128, s i128, f u16, F u32|null, saturated}}, without heap allocation, intermediates ≤ U512.
- Codec, Timing, Assigner, Proposer.draw, Membership, TemplateFSM, Fallback.build.
- End: isClosing, cursorDone, isFinal, bound(h).
- Consensus.headerPrecheck/validateFull.
- ForkChoice (D133):
  - importSeq(hash).
  - key(b) = (signedFlag, importSeq, blockHash).
  - better(t1, t2): by work, then compare the children of the lowest common ancestor.
  - best(candidates): maximum, independent of examination order.
  - onImport(b) → headChanged.
- Independent ForkChoiceRef (Python), used by HeadCheckRef in Q2b.

**Interfaces — Execution**
GenesisBuilder.build, Executor.apply, TipsAccumulator, SystemCall, Pay.credit, CapsTracker, Rewards.payInt, Escrow.pending(j), SettlementIndex, BurnMonitor.

**Interfaces — RPC (D88–D95)**
- HeadersParams.parse(json) → {from u64, count u64} | InvalidParams.
- HeadersParams.check(from, count, headFn) → ok | {fromZero | countRange | fromAboveHead | beyondHead}; headFn is not called before F3.
- MethodTable.classify(method, params) → {class ∈ {heavy, light}, maxResp} | Unknown. Fixed table matching network (B).
- OriginDeny.check(method, hasOrigin) → ok | Deny, fixed ORIGIN_DENY, called in stage 2 before admission.
- RateBucket.take(conn, k) → ok | Rate{retryAfterMs}, consuming k tokens without waiting.
- RpcGuard:
  - accept(conn) → ConnSlot | Reject: above 32, reject without reading; set SO_SNDBUF = SEND_SOCK_BUF through socket2 before any write.
  - admit(class, deadline) → Admission{permit, reservation} | Busy{retryAfterMs}, atomic FIFO admission.
  - Reservation: 64 KiB segments; write rejects overflow with Overflow; shrink(), release() in Drop.
  - Admission.snapshot() → ReadTxn, released with the permit in endCompute().
  - endCompute(admission) → OwnedResponse{reservation}.
- ComputePool.run(admission, job) → Result | ScratchExceeded | Deadline | Cancelled, with 20 threads and no await in job.
- CountingAlloc: thread-local counter, bounded per job.
- CancelToken: check() in scan/execution loops.
- LogsQuery.run(filter, snap, anchor?, budget{bytes, count, scan, deadline}, cancel) → Complete(Reservation) | LimitP1{reason, fromBlock, lastCompleteBlock ≥ fromBlock, nextFromBlock} | LimitRes{reason ∈ {deadline, scratch}, fromBlock, lastCompleteBlock ≥ fromBlock−1, nextFromBlock} | AnchorReject{reason, head} | Cancelled. LimitP1 and LimitRes encode as −32020.
- SendPacer (D90, D92, D93, D94):
  - Built on abstract Clock and SocketSink; same implementation in production and PacerSim. Does not directly import tokio.
  - SendPacer.new(clock, grace=2s, minRate=65536 B/s, slack=200 ms, timerEps=1 ms, trace?) → Pacer, state {sendTime, produced P, accepted A, pending, deadlineTimer, finished}.
  - Pacer.produce(n): P += n. Pacer.finish(): declares production complete; first subsequent A = P is sendDone.
  - Pacer.onWrite(result): A += accepted count, or WouldBlock; then update pending.
  - Pacer.violationAt() → s_v = max(grace, A/minRate); timer at s_v+timerEps with a backstop check every 50 ms.
  - Pacer.check() → Ok | Close{sendTime_ns, accepted, lateMs}.
  - Pacer.write(sink, bytes) → Done{sendDone_ns, sendTime_ns} | Closed{…}. Done means only A = P = size; it does not wait for client reads. Caller immediately releases reservation on Done.
  - A new Pacer for a single request, one Pacer per batch; no grace-reset interface.
  - rpc-fault:perItemGrace (simnet only) creates a Pacer per item, a negative control in PS5, PS5c and RG14(vii).
- PacerSim.run(script, variant ∈ {conforming, perItemGrace}) → {outcome ∈ {sendDone, close}, closeAt{sendTime, A}?, sendDoneAt{t, sendTime, Rc, receivedItems, unreadAtSendDone}?, receiveDoneAt{t, receivedItems}?, lateMs, rows[{t, Rc, P, A, pending, sendTime, s_v}], trace}. script = {mode ∈ {acceptPlan[(sendTime_ns, bytes)], sockBuf SB}, items[S…], clientPauses (pauseAtItemFirstByte=1.8s), computeGaps?, timerJitterMs?}.
- PacerTrace: 4 MiB buffer; overflow counted in traceDropped. Structure {conn, t_ns, sendTime_ns, P, A, pending, event ∈ {produce, accept, pendStart, pendStop, close, sendDone}}.
- TraceCheck.check(trace, clientLog, probeLog, params, rule ∈ {batchShared}) → {TCsend, TCrecv, CloseOK, Enforced, breachAt?, closeAt?, sendDoneAt?, receiveDoneAt?, unreadAtSendDone?, gap}. Matching Rust and TS versions.
- DiscriminationCheck(tracePerItem, clientLog) → {condition: met|unmet, enforcedReportsBreach}.
- BatchStreamer.run(conn, items, pacer, wallDeadline): writes '[', then per item: admit, ComputePool, endCompute, pacer.produce/pacer.write until A = P, release the item's reservation, then next item. After ']', calls finish and waits for sendDone. batchBytes/rate errors use CONN_BUF. wallDeadline = end of reading + BATCH_WALL, measured until sendDone. maxResPerConn ≤ 1 is debug_asserted and monitored.
- ConnLoop: after sendDone, start IDLE timer and permit reading the next request, without waiting for client reads.
- RpcStats.snapshot(), Mempool.admit, RejectLog.

**Interfaces — Registry and storage**
Registry, RetentionStore (with LogCursor and (height, blockHash) index), TrieRebuild, SyncPipeline, OrphanQueue.

**Interfaces — Fixtures**
- ExecFixture.run, FixtureMiner.build, Plan.override, FixtureVerify, PeerHarness (with a hook between client requests).
- MaliciousRpc.serve(chain, {drop?, replace?, fakeHeight?, extra?, refTs?, clock?, targets?, busyCount?}).
- MockLogServer.serve(branches{A, B, A'}, script): literal responses, branch switching at a specified step, eth_getBlockByNumber answers from the current branch, logging every request's order/type (H or logs)/attempt, and a response-hold option (for BR19b(b4)).
- SinkChecker.wrap(sink, serverLog): checks (S-a)–(S-e), records trace.
- EndFixture.ids(n=1020) → sorted list | FixtureSetupError('ids').
- DenseFixture.build(profile='logs/dense', branches=['B', 'A'']).
- RpcAttacker.run({…}) → monotonic timeline per byte/item, receiveDone or cutoff time, and close time.
- ReferenceLogs.read(from, to, branchHash).
- ResourceProbe.sample(pid, 100 ms) → {rss, heavyResv, lightResv, scratchHeavyMax, snapshotsOpen, conns, maxResPerConn, pacerClosed, pacerLateMaxMs, batchWallHits, traceDropped, sendDoneCount}.
- RpcRecorder, SimNet.run, LoadGen, KeccakTriad.check, AsertRef.
- RpcTap.serve(upstream, port) → log {seq, tIn, tOut, origin, method, params, source ∈ {readClient, walletSubmit, logClient, headerNet, other}}; RpcTap.hold(method, ms) delays responses. Source comes from a test header added by modules only in test builds.
- BadSite.build(vectors=BR1–BR13 and BR16–BR19c, txA): published site sending literal bridge messages and recording responses. GoodSite is a second site for BR17f isolation testing.
- TestHooks: approvals(), confirmations(), autoConfirm(accept|reject), tabsCreated() intercepting chrome.tabs.create, bytewise siteStorageSnapshot(netKey, addr), storageSetCalls() intercepting chrome.storage.local.set, bridgeTrace(frame).
- BridgeFuzz.run(seed, n=10^4) → messages and responses.
- FakeClock (configurable nowMs), FakeTransport (holds responses until release(i) or timeout) for BR19a/BR19b.
- SchemaRef (Python): schema and expected B0–B3 code. BridgeRef (Python): adds Bpre with integer bucket and B4 limits using BridgeTrace/RpcTap times, returning the expected decision per message. SiteStorageRef (Python, within BridgeRef): dictionary per (netKey, address), apply(msg) → {decision ∈ {null, 4300}, totalBefore, totalAfter} with literal entry/newTotal accounting.

**Interfaces — Extension**
- HeaderNetCheck.plan(h, kView=12) → {b, from, count, n, hasRef, anchoredGenesis}.
- HeaderNetCheck.ceil(targetG, slackS=4), minWork(ceilTarget) using BigInt.
- HeaderNetCheck.window(head, fetch, cfg, clock) → {ok, n, anchoredGenesis, minWork} | {rule}.
- TrustMessage.render.
- BridgeBucket (D97): new(createdMs) → {tokens_mt: 50000, last_ms: createdMs}. take(nowMs) → ok | rate, using browser's integer formula, no floating point.
- BridgeAuth (D95, D96, D97):
  - ALLOW: frozen Map from (kind, method) to Schema, built at module load, including site_storageSet/site_storageClear under storage_set and site_navigate/site_openExternal under nav.
  - check(rawText, frameCtx, arrivalMs) → {ok, id, kind, route ∈ {readClient, logClient, wallet, siteStorage, navigator}, method, params} | {id|null, code ∈ {−32600, −32005, 4200, −32602}, data}. Performs Bpre–B3 only. Pure except for frameCtx.bucket consumption; no networking or local effects. Does not read pendingReads or quota.
  - parseStrict(rawText) → rejects duplicate keys and depth > 8. Size checked earlier in Bpre.
  - Schema.validate(params) → ok | {path}.
- Bridge.onMessage(raw): stamps arrivalMs, runs check, routes to the component, writes a BridgeTrace row in test builds.
- RpcReadClient.send(frameCtx, method, params): rechecks method ∈ READ_METHODS; otherwise 4200 without networking. Checks pendingReads < 4; otherwise −32005 {pending} without networking. Increments before send; decrements on final response or 10s timeout (−32603 {transport}). Sole network transport exported to Bridge.
- SiteStorage (D96):
  - State: dictionary {key → value} per (frameCtx.netKey, frameCtx.siteAddress), with total calculated/cached in memory and verified by recomputation on load.
  - entry(k, v) = utf8ByteLength(k)+utf8ByteLength(v) through TextEncoder, not String.length.
  - set(frameCtx, key, value|null) → ok | Quota{limit}: computes newTotal = total − (old entry if present) + (new entry unless value = null). If newTotal > 1048576, returns Quota without writing. Otherwise builds and writes the new dictionary with one chrome.storage.local.set, then updates total. Write failure → −32603 {transport}, reload from storage.
  - clear(frameCtx) → ok, writing an empty dictionary.
  - snapshot(frameCtx) → loader dictionary on load.
  - Executed through StoreQueue (D101); no overlapping messages for one key.
  - check(frameCtx, msg) → ok | Quota{limit}: computes only newTotal without writing; called by StoreQueue.evalHead.
  - set is called only for a ready message that passed check, defensively recomputing newTotal. Any discrepancy is an implementation error counted in stats.quotaRecheckMismatch; nonzero fails.
- StoreQueue (D101):
  - admit(session, msg, arrivalMs) → queued | Store, before appending, checking all four counters.
  - evalHead(key) (I56): called when the key's head is released, or a message is accepted for an empty key. Calls SiteStorage.check:
    - Quota: reply 4300, then release.
    - ok: message becomes ready.
    - Takes no seat and does not call StorageBackend.
  - pump(): if a seat is available, takes the oldest ready message in the global FIFO by admission order, makes it active, calls set. One active write per key, ≤ 2 extension-wide.
  - onSettle(reqId): only event moving an active message to released.
  - expire(nowMs): releases expired waiting messages with store replies; replies storeTimeout for expired active writes without releasing.
  - release(msg) internal, guarded by released flag. debug_assert prevents repetition; stats() counts releaseCount/doubleReleaseCount. Nonzero doubleReleaseCount fails.
  - Per-message state {replyState, ownState} under D102, recorded in BridgeTrace.
  - cancelFrame(frame): cancels waiting work, leaves active work untouched.
  - awaitActive(key, ≤ 5000 ms) for Navigator.
  - stats() for TestHooks.
  - Runs over abstract StorageBackend: chrome.storage.local in production, FakeStorage in tests.
  - Key derived from frame context, not message.
- EpochFence.boot(backend, monoClock) → {epoch} | Disabled{reason ∈ {epochNames, retry}} (D134):
  - Lists epoch items, applies name gate then generation window.
  - Writes 'pocol:epoch:'+hex16(E), waits for confirmation.
  - No data write is issued before success.
- EpochFence.sweep(): scheduled after T_LATE monotonic time from boot; removes names lower than the greatest visible name.
- EpochFence.stats → {epochNames, epochGateBlocks, epochResurrected}.
- RecoveryGate.await(key) → {dict, total, confirmed} | Failed{reason ∈ {corrupt, checkpoint}} | Busy | ReadTimeout: one shared promise per key per generation. Carries gateId; starts total read deadline on entering recovering (D110). Issues only the anchor and its second attempt, each with a RecoverySlots seat. Busy/ReadTimeout return the key to unrecovered without issuance. Every read response's gateId is checked before use.
- ReadSlots (D110): acquire(op, deadlineMs) → Slot | Timeout through ReadFIFO. slot.settle() is the only release event. stats → {held, timeouts, lateDropped}. StoreQueue does not evaluate a head or provide a snapshot before ready.
- RecoverySlots (D109): acquire(op, waitMs=5000) → Slot | Busy through RecoveryFIFO, in the same synchronous step as DiskLedger.reserve and issuance. cancel(request) on ready. slot.settle() is the only release event after issuance; no timeout-release interface. stats → {held, unsettledMax, busy}.
- SeqAlloc.alloc(key) → u32 | Exhausted: sole allocator of set sequence numbers in the generation.
- DiskLedger (D105):
  - reserve(op) → DiskTicket | {disk|sites|entries, limit}: synchronous, no await, checks and reserves in one step.
  - ticket.settled(settleSeq).
  - Serialized refresh() calls getBytesInUse/getKeys; upon completion, drops tickets whose settleSeq is below its issue number.
  - StoreQueue, RecoveryGate and AdminDelete never issue set without a ticket.
  - (D112) lateGens(now) from epoch items listed at boot and the live-generation item. lateSites(now) = 4·lateGens.
  - reserve(op) determines whether the operation creates a key: data set or anchor for an uncounted, unreserved key. Applies the site condition; returns {sites, limit, retryAfterMs?} on violation.
  - onWindowExpiry(X) schedules refresh, removing X's contribution after a refresh issued beyond bootMs(X)+T_LATE completes.
  - removeLastRecord(key) allowed only under D112; called only by TombReaper.
  - (D113) reserve(op) on a counted key takes no seat, adding a pin unless the operation is remove. refresh drops a key from sitesLive only if unseen by measurement and unpinned; drops pin in the same step as ticket removal.
  - TombReaper.scan(): called after every adopted refresh and every epoch-window exit; issues remove for eligible sole tombstones. stats → {tombReaps, pinnedKeys}.
- StorageManager (extension page): list(), deleteSite(netKey, addr) with user confirmation, returning the shared AdminDelete promise → deleted | failed | uncertain.
- AdminDelete (D106, D108): four-step run(key), generation-unique opId (D111), SessionRegistry.closeAll(key), Viewer.blockOpen(key). Result → deleted | failed | uncertain | adminBusy.
- AdminSlots (D108):
  - acquire(op, waitMs=5000) → Slot | Busy through AdminFIFO, in the same synchronous step as DiskLedger.reserve.
  - slot.settle(): sole release event. No timeout-release interface.
  - (D111) acquire(op{opId, key}, waitMs) → requestId; cancel(requestId) called in the resolution step.
  - grant(request) calls guard(op, key) before DiskLedger.reserve/SeqAlloc.alloc, without intervening await. Failure returns the seat to the next FIFO head and increments adminStaleDropped.
  - DeleteOp: {opId, key, state ∈ {running, settled}}; settle(result) changes state and withdraws requests in one step.
  - stats → {held, unsettledMax, busy}.
- RecordCodec: name(key, epoch, seq), strict fmt 2 encode/decode with RECORD_BYTES_MAX.
- StorageBackend adds three functions:
  - listKeys(): through getKeys only.
  - bytesInUse().
  - remove(keys).
- ViewerLink: hello{genId}, onLost() → teardownAll() with notification.
- SR-model (Python), FakeBackend(pending, scheduler) for BR22a/BR22b.
- Navigator (D96, D99): navigate(frameCtx, path), in order:
  1. Debits the session's navigation bucket, otherwise −32005 {nav}.
  2. Applies storage normalization.
  3. Orphans frame requests, cancels the previous LoadJob, calls StoreQueue.cancelFrame, then awaitActive before snapshot (D101).
  4. Asks viewer to tear down the frame and build a new one joining the same session, or displays a 404 page.
- SiteSession (D99):
  - Interfaces: open(tabId, netKey, siteAddress) by user action only, attach(frame), orphan(frame), settle(reqId), close().
  - Owns BridgeBucket, NavBucket, pendingReads, LoadJob, ChunkCache and SESSION_RECV_MAX reservation. openExternal(frameCtx, url) → confirm → null | 4001 | −32005 {pending}; opens a tab without opener.
- WalletSubmit.submit(rawTx, approvedTxHash): verifies keccak256(rawTx) = approvedTxHash, then sends eth_sendRawTransaction. Called only by Wallet after approval.
- HttpTransport (D98): internal; imported only by RpcReadClient, WalletSubmit, LogClient, HeaderNetCheck, NetworkProfiles and ChunkFetcher.
  - request(method, body, {recvLimit, deadline, pool, session?}) → {json} | {error ∈ {recvLimit, recvDepth, recvParse, transport, busy}}.
  - Does not call text() or json().
- RecvGuard:
  - reserve(pool, session, bytes, waitMs=2000).
  - read(stream, limit): counts each chunk before appending.
  - release(): on settlement.
  - stats(): for TestHooks.
- ChunkFetcher.fetch(addr, len, session): checks ChunkCache first, then fetches through HttpTransport from CONTENT_POOL.
- MaliciousHttp (Node): scripted MR server. A dependency-cruiser rule prohibits Bridge from importing it or WalletSubmit.
- LogClient: plan(from, to) → windows ≤ 1024 marked withinLimit=false. Pure step(range, reply) → Append | Replace | Retry | Restart | Error. Sink and RequestCounter (rejects before sending if totalRequests would exceed maxRequests). run(rpc, filter, sink, {timeout, maxRequests, maxRestarts=3}). fetchAll(rpc, filter, {maxBytes=4 MiB, timeout=30s, maxRequests=4096}) → Complete | Error{tooLarge, timeout, tooManyRequests, deadline, scratch, serverViolation, reorgUnstable, beyondHead, unsupported}, counting as one pending frame request. fetchEach(rpc, filter, sink, {timeout=600s, maxRequests=65536}).
- Viewer, Loader (builds localStorage from snapshot, batches writes every 250ms with a bucket-respecting queue, restores confirmed values on 4300, intercepts links), DnrGuard, NetworkProfiles.validate, Wallet, TestHooks, CanarySink.

**Dependencies**
- M2 after M1; M4 after M3; M4b after M4; M5/M6 after M4; M7 after M5; M8 after M5/M6.
- Within M2: BridgeBucket, BridgeAuth and RpcReadClient with BG1, BR19a/BR19b first; then SiteStorage with BR17e on a fake dictionary; WalletSubmit, Navigator and remaining BR; then T12.
- Within M7: SendPacer, PacerSim and PS first; then PacerTrace, TraceCheck and DiscriminationCheck; then RG and BR15.
- HeaderNetCheck and NetworkProfiles.validate import asert and the M3-TS model.
- LogClient tested with MockLogServer, then against pocold in RG3, RG3b and RG9.
- Phases A and B run in parallel, except shared asert delivered first in the second task.

**First reviewable, testable task: M1-spec**
A text document containing:
- Initcode bytes and address formula, with three vectors: 0x61, 24575 bytes of 0xaa, and a specified html file.
- Manifest encoding/normalization, Website slots/permissions, T1 and T16 with literal inputs.
- Extension annex: netKey vector (777001, 0x11×32), signEligible, Q8–Q12, X1–X7, DNR-d1–DNR-d5.
- Bridge-authorization annex (D95, D96, D97):
  - Literal envelope {id, kind, payload} and payload {method, params}, Bpre/B0–B4 order, literal error codes/data, and returned-id rule (value or null).
  - Integer bucket formula (capacity, refill, cost, consumption rules), arrivalMs definition, pendingReads definition and increment/decrement events.
  - Complete (kind, method) matrix for all four classes, machine-readable method schemas, primitive types with regexes including str, pathStr and httpsUrl.
  - SiteStorage semantics: entry/total/newTotal for insertion, replacement, deletion and clearing; ≤ 1048576 condition; single-write atomicity; key derivation. Navigator semantics: normalization, confirmation, no reply to torn-down frames.
  - Explicitly tested prohibition list.
  - Flowchart of the sole write path through WalletSubmit.
  - Literal dependency-cruiser rule.
  - Byte-literal BR1–BR14 and BR17–BR18 messages with expected responses, including BR7a–BR7h in violation order; literal txA/hash; RpcTap commands and “zero requests” criterion.
  - Complete BR17e table: all twenty-four literal messages and byte lengths; total before each, newTotal, total after, response: (e1) 61443·i through 1044531; (e2) 1105974 → 4300; (e3) 1048577 → 4300; (e4) 1048576 → null; (e5) 1048580 → 4300; (e6) 1048577 → 4300; (e7) 987137 → null; (e8) 925694 → null; (e9) final dictionary and total 925694.
  - BR19a (a1)–(a7) tables with arrivalMs/tokens_mt before and after each message; stepwise BR19b (b1)–(b5); BR19c script; BridgeRef specification and resolution criterion.
  - BR16 generator/seed; SchemaRef, BridgeRef, SiteStorageRef; BridgeTrace format.
  - D98 annex:
    - RECV_LIMIT table/derivation, exact/capped classes, WorstLegit/RecvFit formulas (D100).
    - MR6b–MR6d, CAP1, RF1–RF4 vectors; MR10a–MR10f timed scenarios.
    - Read steps 1–6 and pools.
    - Bytewise MaliciousHttp scripts for MR1–MR12.
  - D99 annex:
    - SiteSession lifecycle and orphan-request states.
    - Navigation-bucket formula.
    - Corrected BR20a table with check/route columns, tokens_mt/pending after each message, orphan settlement time; the same table transferred to BridgeRef.
    - BR20b table; BR20c script and criteria.
    - BridgeRef session-semantics extension.
  - D101 annex:
    - qbytes formula, four counters, per-key FIFO order, global active bound and both timeouts.
    - Teardown rule and waiting before snapshot.
    - BR21a (q1)–(q10) (q10 corrected to sixteen sessions), BR21b (r1)–(r6), BR21e, BR21f and (f2) rejecting quota before set, with times/counters/responses/reply/release/storeViolated events; TQ parameters; BR21c/BR21d scripts; replyState × ownState transitions (D102).
    - BridgeRef/SiteStorageRef extended with StoreQueue model.
  - D103 annex:
    - Epoch-item and record name/value formats; greatest-selection rule.
    - (D134, first review before storage-recovery implementation):
      - Shared name per E, name gate, post-T_LATE sweep rule.
      - Literal writer, name-bound and coverage lemmas.
      - BR22h (h1–h6) tables with three controls; SR-model extension.
    - Fence/recovery steps and both lemmas literally.
    - Viewer behavior on disappearance.
    - Full BR22a combination table with expected state per combination; BR22b/BR22c scripts.
  - D104 annex:
    - RecoveryGate state machine, SeqAlloc and bounds.
    - fmt 2 format; RECORD_BYTES_MAX derivation.
    - Disk-admission rule, generation window, sweeping, tombstone.
    - BR22d with V-a..V-e variants; BR22e; BR22f with f2–f4 variants and RecoverySlots state machine.
    - BR22g (g1–g6) with read phase, ReadSlots, gateId (D110), deriving the 25s resolution bound from t_s.
    - Literal BR23 (d1–d11, including d11b corrected with the complete name condition and epochReserveOmitted control path, D135) and BR24, with AdminDelete/DiskLedger/AdminSlots state machines (D105–D108).
    - Derivation of LATE_NAMES = 24 excluding epoch items and EPOCH_NAMES_MAX = 8 (D109, D134; 28 superseded), META_RESERVE constraint, AdminDelete = 45s bound, BR24(x9–x12) with x12b and BR22f-f5 variant.
    - DeleteOp state machine (D111), AdminSlots/RecoverySlots grant guard, literal no-late-tombstone lemma.
    - (D112) sitesLive, creating operations, lateGens/LATE_SITES formulas and expiry rule, admission condition, final-tombstone removal, literal site lemma, A15d.
    - Timed BR23(d1, d9, d12–d15) with lateGens/LATE_SITES/measurement values; BR24(x6); SR-model extension.
    - (D113) Literal tombstone-counted-key rule, pin, TombReaper and new-seat requirement.
    - (D114, I68) Corrected BR23(d15c), both correct/dropPinned paths, including:
      - Boot and expiry times.
      - lateGens, LATE_SITES, sitesLive, pinnedKeys after every refresh.
      - Test-only DiskLedger.refreshNow() hook; table transferred to SR-model.
      - Literal responses at 30100 in both paths, transferred literally to SR-model:
        - Correct: 4300 {reason: 'sites', limit: 64}, without retryAfterMs.
        - dropPinned: 4300 {reason: 'sites', limit: 64, retryAfterMs: 29900}.
        - Both values derived from D112's condition.
- HeaderNetCheck vectors: chainId=777002, RW1–RW6, h=3 anchored at genesis, viewIncomplete for missing header/reference at h=20, viewGenesis, h=0 → viewNoBlocks, HC1–HC7 with literal decimal minWork and message texts, NX-V3e, NX-V3f, NX-V3g.
- LogClient vectors: A/B/A' with literal logs/hashes, 8/0/2 table, LC1–LC18 literal JSON responses including −32020 (P1/RES), −32021 (busy/rate), −32022; literal H(·) request sequences; sink trace with each commit's summary; totalRequests table: LC1=5, LC5=5, LC8=7, LC10=4 sent, LC16=11, LC17=12 (variant 8), LC18=10; RefA/RefB/RefA'; complete step transitions for every (reason, lc, a=b, withinLimit) combination.
- NetworkProfiles.validate: GSV1, N1, N6, N11.
- X8: C1–C8 pages, K0–K5 flags.

Reviewed by Codex, then implemented outside this session.

**Second task (M3, TS)**
- Encoding/identity: K1–K3, KeccakTriad, GenesisSpec, NetConfig, NetId.
- Difficulty: asert with AS1–AS19, AS-eq, ASQ, ASB.
- Gate: ParamGate with PG-END1–5, PG-L, PG-RPC.
- Headers: Codec (L0), structural H-pre (L1), search (L2), Timing (TJ), Assigner.
- Execution: RegistryLayout, payInt, BurnMonitor.
- RPC/extension: HeadersParams (HG1–HG8, HG14–HG16), HeaderNetCheck.plan (RW1–RW6), ceil/minWork (HC1–HC7).

**VA-spec annex (D115)**
Written before M5 code and reviewed by Codex. Contains:
- Literal va/base profile with genesisPre bytes and identity rates.
- NetSim interface/commands; AdvNode interface (policies: silentProposer, splitTemplates, headerOnlyWithhold, selfish, doubleSpend, ownSharesOnly, emptyShares, winnerEquivocate, splitIdentity(k), churn, eclipse, censor).
- Every VA-annex test script with times, messages and faults.
- TraceDB format; each metric and denominator computed by an independent Python reference.
- Measurement barrier (D122, D123):
  - halt, stopMining and statusSweep commands.
  - Machine-readable Quiet definition from NetSim state and each node's ChainWork.
  - F1–F3 policy/purgeFuture; U/A/V paths with purgeUnavailable, holding definition, t_avail (D124); closed U-reason set {bodyUnavailable, envUnavailable, depUnavailable}, validEnv and EnvHeldAtHalt with one classification shared by SimNet.recSet, HeadCheckRef and BudgetCheck (D131, D132); Rec, DrainPin, Need(i), HR, RB, c_rd, T_REBUILD_MAX, PIN_MEM_MAX (D125); scoped SYNC_RETRY_MAX and RESTART_MAX; DRAIN_MAX(run) formula with N_miss, c_b, DRAIN_CAP; QUIET_HOLD.
- HeadCheckRef Python reference, including:
  - RefReplay.
  - Literal QV1–QV26, QV23d, QV25c-dup, QV26m and T23 N9/N10, including QV19–QV23 and revised QV20 (D128), removing its predecessor, with:
    - BudgetCheck table and ActLog classes.
    - Default SimParams and per-vector overrides.
    - Timestamps, targets and equal-work-per-height lemma.

    Alongside qv/net with genesisPre bytes and clock/eligibility tables.
  - Forty-six simnet-fault builds: envRequestBeforeParent (D140), envCheckSharedPool (D139), envReleaseOnCancel, envTimeoutCoversCheck (D138), envSolicitUnbounded, envReplyUnreserved, envTimeoutExclude (D137), envCacheUnbounded, envFullAsBlockReject, envDropSolicited (D136), pairwiseTiebreak (D133), globalRtt, envAsBlockReject, noEnvUnavailable, noEnvFetch (D131), statusResetsTried, envRejectCancels, hfullDoneOnEnv, execPerCopy (D130), cacheByBlockHash, mergeInFlight, skipHp, noReeligible (D129), noFetchDedup, cacheNotEvaluable (D128), kpendIgnored, noHdrCache (D127), pinLate, needHeadsOnly, rbPerBranch, hdrUndercount (D126), rejectOnBadBody, excludeDepAsInvalid, noDrainPin, drainUndercount, silentDeepReorg (D125), noSignedTiebreak, noReorgOnHeavier, execDrift, quietIgnoresFuture, purgeImported, untaggedTimer, keepUnavailable, purgeHeld, skipHolder, noCancelOnReject (D124).
- CEN table; BootRef (D121) with fixed-seed vectors/literal results for TS comparison; SV10a–f vectors (D120).
- FSM1–FSM10, N1–N8 and L1–L13 tables with expected states.
- ReplicaHarness for T25: literal T25Site files; kill, wipe, S0 creation/signing/retrieval commands; machine map; ReplicaMonitor list.
- Byte-literal caps/64 profile, SYS_SLOT_MAX derived from R18, SlotFill contract, L1–L13 loads, caps-test constants.
- AdvOracle and its bound, excitation definition for each conditional test, T_RUN_MAX per test.
- Runtime estimate on Windows with 8 GB; schedule arms in batches without changing criteria.

**M7-spec annex**
Written before M7 code and reviewed by Codex. Contains:
- Classification table with worst-size derivation per method; ORIGIN_DENY table.
- Resource-ownership lifecycle (1–7) state diagram, with release at sendDone.
- SendPacer diagram: P, A, pending, clock start/stop, s_v, TIMER_EPS, timer reset, backstop check, CloseOK, finish, sendDone.
- Literal PacerSim model: both acceptance modes, next-item production rule, client behavior, within-instant fixed-point iteration, sendDone/receiveDone/unreadAtSendDone definitions.
- Literal PS1–PS8 scripts with (t, Rc, P, A, pending, sendTime, s_v, event) tables for conforming/perItemGrace: PS5/PS5c through receiveDone (PS5: sendDone = receiveDone = 36. PS5c: sendDone 32.4, 18 items read, unread = 196608, then receiveDone 36), and staircase PS4 derivation.
- PacerTrace format; mathematical TC-send, TC-recv, CloseOK, Enforced, Breach, DiscriminationCheck definitions; RG14 C-case conditions as functions of complete traces; RG14(0b).
- Table naming each timeout's measured event, all server events.
- Literal JSON shapes for both −32020 types, both −32021 reasons, and −32022.
- Bytewise streaming-batch example, RG plan with RpcAttacker commands/criteria, BR15 plan.
