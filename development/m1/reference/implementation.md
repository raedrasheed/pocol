# خطة التطوير

**البيئة**
Windows 10 بـ8 GB، دون Docker أو WSL أو صلاحيات مسؤول.
- منافذ ومجلدات خاصة، دون إيقاف بيئة العمل القائمة.
- Chrome اختباري بـuser-data-dir منفصل.
- نسختا anvil (نحو 200 MiB لكل منهما).
- RpcTap وكيل Node محلي ≤ 100 MiB.
- CanarySink محلي. جدار الحماية قد يطلب سماحًا، وهذا قرار المستخدم، والبديل جهاز ثانٍ.
- FixtureMiner أحادي الخيط ≤ 200 MiB.
- RpcAttacker ≤ 300 MiB، ويُقاس RSS العقدة وحدها.
- fetchEach في Node ≤ 300 MiB.
- PacerSim في Rust دون شبكة.
- BR19a وBR19b في Node دون متصفح ولا شبكة.
- Python محمول اختياري للمكتبة الثالثة والمرجع وSchemaRef وBridgeRef وSiteStorageRef، وإن تعذر يُستخدم جهاز آخر ويُعلن.
- BR18e وBR18f لا يحتاجان اتصالًا خارجيًا فعليًا: TestHooks يعترض chrome.tabs.create ويسجل الرابط دون تحميله.
- BR17e يكتب نحو 1 MiB إلى chrome.storage.local في ملف المستخدم الاختباري فقط.

**الترخيص**
Apache-2.0، مع اعتماديات MIT أو Apache أو BSD. Foundry وsolc أدوات بناء فقط.

**المراحل**
- M0: الإصدارات، والتراخيص (ومنها hyper وsocket2 وdependency-cruiser)، وanvil، والكناري، ومكتبات keccak الثلاث مع K1–K3، ومكتبة العرض الثابت، وتدقيق rpc-fault وأخطائه الثلاثة، وbridge-fault بأخطائه الاثنين والثلاثين المعدودة في governance (مخرج M0)، والتحقق من توفر chrome.storage.local.getKeys في Chrome الاختباري.

المرحلة A (anvil، لا تثبت شيئًا عن PoCol):
- M1: M1-spec، ثم ChunkFactory وWebsite وأداة الناشر. الاختبارات: T1، وT16.
- M2: إضافة MV3 تشمل:
  - DnrGuard، وProfileMutex، وRtcLockdown، والمحفظة.
  - BridgeAuth، وBridgeBucket، وRpcReadClient (مع pendingReads)، وWalletSubmit، وSiteStorage (مع محاسبة الحصة)، وNavigator، وBridgeTrace.
  - CanarySink، وHeaderNetCheck، وLogClient (مع SinkChecker والعدادات).
  - RpcTap (مع hold)، وBadSite، وGoodSite، وBridgeRef مع SiteStorageRef.
  - الاختبارات: BG1، وBR19a، وBR19b، وBR20a، وBR20b، وMR-unit أولًا، ثم وحدة SiteStorage على BR17e بقاموس وهمي، ثم T11a، وT12، وX8، وBR1–BR14، وBR16–BR18، وBR19c، وBR20c، وBR20d، وMR1–MR12، وBR21a–BR21f، وBR22a–BR22h وBR23 وBR24 (مع EpochFence وRecoveryGate وReadSlots وRecoverySlots وSeqAlloc وDiskGate وDiskLedger وAdminDelete وAdminSlots وSR-model)، وBR-neg، وBR-dep، وRW-unit، وHC-unit، وLC-unit (LC1–LC18).

المرحلة B (pocold، دمج PoCol الفعلي):
- M3: GenesisSpec، وParamGate (R13، وR16 بـLmax وLAG، وR20، وR26 مع المشتقات وTIMER_EPS)، وNetConfig، وCodec، وNetId، وTiming، وDifficulty.asert، وDraw، والتقسيم، ونموذج TS. الاختبارات: K1–K3، وL4، وT22، وT0، وAS1–AS19، وAS-eq، وASQ، وASB، وTJ، وPG-END1–5، وPG-L، وPG-RPC، وCF1–CF6، وSV1–SV7s.
- M4: revm مع PAY، وGenesisBuilder (حتى 1024)، والمراحل، وCapsTracker، وTipsAccumulator، والعقود النظامية، وBuild_fb، وExecFixture. الاختبارات: T2، وT2b، وCI1–CI3، وGB1، وGB-END، وL4s، وT2c، وT24، وT26.
  - (D141، D142) BlockMeta.parentResult(hash) → {blockHash، winner، shares[(n, owner)]}.
    - يشتقه ParentResult.derive(HeaderStore.header(hash), Assigner.tableFor(hash)) عند تحقق البندين 13 و14، ويُخزن بمفتاح hash.
    - Reexec.verifyParentResult(hash) يعيد الاشتقاق عند المزامنة وTrieRebuild وإعادة التنظيم، ويقارن بالمخزن، والاختلاف parentResultMismatch.
    - ShareRecorder.record(ctx.parentResult) يستدعيه recordShares(h−1) وحده.
  - Executor.apply يأخذ ctx.parentResult، ولا يقبل nonce الكتلة الحالية ولا مالكها، ويفرض ذلك نوع الواجهة.
  - الفحص الآلي في M4 يضيف شرطين: closeBlock لا يكتب k=2 لـpending[h]، وrecordResult يُستدعى من SYSTEM_ADDRESS فقط.
  - بناء exec-fault:winnerInState للاختبار فقط، غائب عن الإصدار.
- M4b: FixtureMiner وFixtureVerify. الاختبارات: FX0، وSV8، وSV8-fb، وSV8-cap، وFBX، وSV9، وSV9-final، وNX (أ)، وTW1–TW5 مع الضوابط winnerInState وsharesFromFirstSeen وsharesFromMiner وsharesDropped (D141، D142). FixtureMiner يقبل plan.shareOwners لبحث حصص في نطاقات محددة على UT مبني مسبقًا.
  - العتبة (D143) تُحسب في FixtureMiner.shareThreshold(UT) = min(2^256−1, UT.target·CP.m)، باستدعاء دالة الإجماع نفسها لا بثابت في plan.
  - plan.gapShare يطلب nonce بهاش في (T_share، حد علوي معطى] لضابط TW5-gap.
  - بناء fixture-fault:fixedShareThreshold للاختبار فقط، غائب عن الإصدار. FixtureMiner يقبل plan.searchOwners = [w, x] على UT واحد مبني مسبقًا.
- M5 (لا يبدأ كوده قبل مراجعة VA-spec، D115): P2P، وTemplateFSM، والمزامنة، وSimNet مع NetSim وAdvNode وTraceDB.
  - ChainTimer (D123): schedule(tag ∈ {chain, nonChain}, at, job) وpending(tag). هو المجدول الوحيد في وحدات الإجماع والمزامنة والاستيراد.
  - تمنع قاعدة clippy disallowed_methods استعمال tokio::time::sleep وinterval وtimeout مباشرة في هذه الوحدات، وخرقها يفشل CI.
  - SimNet.chainWork(node) وSimNet.purgeFuture(entry) وSimNet.statusSweep()، وتعمل في بناء simnet وحده.
  - (D124) SimNet.bodyHolders(hash) وSimNet.purgeUnavailable(tAvail) وSimNet.drainBound(run) → {N_miss, c_b, DRAIN_MAX}، وتعمل في بناء simnet وحده.
  - (D125) في بناء simnet وحده:
    - SimNet.recSet(tAvail) → {rec, excluded[{hash, reason, missingAncestor?}]}.
    - SimNet.need(i) → {blocks, branches[{forkDepth}]}.
    - (D126) DrainPin.haltPin() داخل خطوة halt الذرية، وDrainPin.add(item) لكل وارد صادق بعدها، وDrainPin.narrow(rec) عند t_avail في خطوة U نفسها، وDrainPin.stats → {pinRefusals, pinBytes, pinEntries}.
    - (D126) ActLog.record(node, class, durMs) عبر ChainTimer، وBudgetCheck.run(actLog, bounds) → ok | {class, node, actual, bound}، وHeadCheckRef.heldAtHalt(log).
    - evictProbe(node, hash) خطاف اختبار.
    - drainBound يعيد {need, K, Kmiss, Kpend, S, T, dStar, HR, RB, waits{restart, eligibility, preHalt}, DRAIN_MAX} ويُطابق HeadCheckRef (D126، D127).
    - (D127) SimParams.load(vector) سجل واحد يقرؤه NetSim وdrainBound وHeadCheckRef، وBudgetCheck.table(run) يعيد سطر كل صنف بعدده ومدته ومكوّنه.
  - (D127، D128) في pocold:
    - HdrCache (D129) بحد طوابير المزامنة، ومكوناته:
      - BlockCache بمفتاح blockHash لأحكام المحتوى الملتزم المكتملة.
      - EnvCache بمفتاح hdrId لأحكام الغلاف.
      - inFlightHdr بالمفتاحين.
      - HeaderStore.accept(blockHash, hdrId) يحفظ أول نسخة صالحة الغلاف.
      - EnvClass.classify(rule, part) → env | block.
      - EnvCheck.pre(hdr) → envOk | envReject، بلا حالة. EnvCheck.own(hdr, parentState) → envOk | envReject، دون apply، مع تخزين التعيين لكل أب (D130).
      - EnvCache بحالات {envPending, envOk, envReject}، والحكم بعد own نهائي ما بقي محفوظًا (D136). الواجهات:
        - admit(hdr, solicited, peer) → check | dropFlood | dropKnown.
        - record(x, hdrId, rule, peer) يطرد أقدم حكم لـx أو أقدم حكم عامًا.
        - purge(x) عند الاستيراد أو envOk أو blockReject أو الإزالة.
        - stats → {envRejEntries, envFloodDropped, envEvicted, envMemBytes}.
        - pin() في بناء simnet ضمن HaltPin.
    - FetchSched، بالواجهات التالية:
      - DiscRun.onStatus(peer, head) → start | markDirty.
      - DiscRun.next() → {peer, locator} | done، ويضع tried.
      - BodySched.request(hash) → peer | waiting، ويحمل inFlight وtriedPeers.
      - BodySched.onReply(hash, peer, NotFound | Mismatch | Ok).
      - BodySched.onStatus(peer) وBodySched.onAnnounce(peer, hash) يضعان النظير في reeligible دون محوه من tried (D130).
      - BodySched.cycleReset(hash | all) يمحو tried وreeligible والمؤقتات، ويستدعيه SimNet عند t_avail.
      - BodySched.pick(hash) → untried بترتيب المعرّف | reeligible | waiting.
      - Backoff بـBODY_RETRY_BASE = 1s وBODY_RETRY_MAX = 60s، مع تفضيل غير المجرَّب (D129).
      - HfullDone.has(hash).
      - (D131) EnvFetch:
        - request(hash) → peer | waiting.
        - onReply(hash, peer, Copy(hdr) | NotFound | Timeout | Oversize | Disconnect)، وكل نتيجة تستدعي EnvSlots.release مرة واحدة (D137).
        - EnvQueue (D137): enqueue(hash) مرة لكل x، وhead()، وrequeueTail(hash)، وremove(hash).
        - EnvSlots (D137):
          - grantSolicit(hash) → Slot{reqId, peer, buf 3131 B} | none، في خطوة الإرسال نفسها.
          - acquireGossip(hdr) → Slot | none.
          - cancel(slot, attemptId, reason) (D138): يضع cancelRequested. في reserved أو buffered يعيد ضبط substream ثم يحرر فورًا، ولا يحرر في checking.
          - onCheckSettled(slot, attemptId, verdict): الحدث الوحيد الذي يحرر مقعدًا في checking، ويطبق قاعدة نتيجة الفحص الملغى في network.
          - release(slot, attemptId, reason): داخلي، بعلم يمنع التكرار. لا يفعل شيئًا إن لم يطابق attemptId المحاولة الحالية.
          - مؤقت المهلة يُلغى عند الانتقال إلى checking. أحداث المؤقت والرد تمر بحلقة واحدة، والتساوي للرد.
          - stats تضيف {liveChecks, liveBuffers, cancelPending, lateCheckDropped, envCheckOverrun}.
          - CheckGate.hold/settle خطاف في بناء simnet فقط، لـN12c وN12r.
          - EnvCheckPool (D139): submit(slot, attemptId, hdr) بطابور FIFO وW خيوط مخصصة، وstats → {queueLen, spanMaxMs, envCheckOverrun}. FakeExec في بناء simnet يحدد c_exec لكل فحص (N12p وN12h وN12w).
          - RecoveryRef (Python): conditions(trace) → {t0 | none, preCondWait, unmet ⊆ {G1, G2, G3, G4}, verdict ∈ {withinBound, boundViolated, noGuarantee}} (D140)، مع ref-fault:t0Unconditioned. bound(P, Q, B, T_role) → D_x، وsequence(peers, replies) → ترتيب الطلبات المتوقع، مع ref-fault:recoveryOldBound.
          - onLateReply(reqId) → مسار admit غير المطلوب.
          - stats → {solicitHeld, gossipHeld, releaseCount, doubleReleaseCount, timeouts}. أي قيمة غير صفرية لـdoubleReleaseCount فشل.
        - onStatus وonAnnounce وcycleReset كـBodySched.
      - GetEnvelope.serve(hash, exclude) → HeaderStore.validCopy(hash) | NotFound.
      - EnvWait.has(hash)، وENV_PENDING_MAX = 4، وثوابت D136 (ENV_REJ_PER_BLOCK وENV_REJ_GLOBAL وENV_PENDING_GLOBAL وENV_INFLIGHT_GLOBAL). وثوابت D137 (ENV_SOLICIT_SLOTS وENV_GOSSIP_SLOTS وENV_REQ_TIMEOUT وENV_REPLY_MAX). اختبارات T23 N11–N11c وN12 وN12t وN12b في M5.
      - SimParams.rtt(i, j) وrttMax(i) (D131)، وActLog يسجل envFetch صنفًا فرعيًا من hdrFetch مع مدة كل طلب.
    - ActLog يسجل لكل طلب (hash، peer) ولكل تشغيلة معرّفها.
  - في pocold: ChainWork بمفتاح هاش الكتلة، وRejectPath.cancel(hash) يلغي المدخل ونسله ومؤقتاته في خطوة الرفض، ويُستدعى فقط لرفض H-pre أو H-full على جسم مطابق للالتزام بصنف blockReject. envReject يمر عبر EnvRejectPath.drop(hdrId, peer) دون إلغاء (D130). BodyFetch.onMismatch(peer, hash) يعاقب النظير ويعيد الطلب دون إلغاء (D125). NotFound ينقل طلب الجلب فورًا إلى نظير آخر. الاختبارات: T3، وT4، وT5، وT6، وT9، وT20، وT21، وT23، وSV8-reorg، وSV8-tie، وSV9-reorg، وNX (ب، ج).
- M6 (يشترط VA-spec، D115): الحصص، والتسوية، والأدلة، والإغلاق مع End.bound، وBurnMonitor، وEndFixture، وسياسات الخصم في T7b وT7c وT7e. الاختبارات: T7، وT7b، وT7c، وT8، وT17، وT27، وCL-END، وCL-OPS1–2.
- M7: خادم RPC على hyper:
  - أولًا SendPacer مع PacerSim واجتياز PS1–PS8 قبل أي اختبار مقبس.
  - ثم RpcGuard، وComputePool، وCountingAlloc، وRateBucket، وLogsQuery، وpocol_getLogs، وBatchStreamer، وSO_SNDBUF، وPacerTrace، وTraceCheck (مع DiscriminationCheck)، وOriginDeny.
  - LN ونافذة RP، وlogs/dense، وDenseFixture (B وA' وBlob)، وRpcAttacker، وReferenceLogs.
  - الاختبارات: PS1–PS8، وHG1–HG16، وRG1–RG15 مع RG2b وRG3-bridge وRG3b ومتغيراتها، وRG14(0)–(xii) مع (0b) وضابط perItemGrace، وT11b، وT11c، وT15، وBR15، وNX (د)، وNX-V1–V6 مع V3b–V3g، وNX0d، وNX1d، وLN0.
  - M7 لا يُجتاز دون PS وRG وBR15.
- M8 (يشترط VA-spec، D115): RetentionStore، وTrieRebuild، وLoadGen، وReplicaHarness لـT25. الاختبارات: T10، وT10b، وT10c، وT13، وT14، وT18، وT18b، وT19، وT25، وT4-V.

**الواجهات — الإجماع**
- GenesisSpec.encode/decode/verify → GsError{code, path}.
- ParamGate.check/derive: derive تعرض Lmax وLAG وRPC_MEM وBATCH_SEND_MAX وحد مراقبته والحد الأدنى لـBATCH_WALL كأعداد نسبية دقيقة.
- NetConfig.load وactiveVersion، وNetId.check.
- Difficulty.asert(pTs u64, pH u64, cp) → {target U256, trace{deltaT i128, e i128, s i128, f u16, F u32|null, saturated}}، دون تخصيص كومة، وبوسائط ≤ U512.
- Codec، وTiming، وAssigner، وProposer.draw، وMembership، وTemplateFSM، وFallback.build.
- End: isClosing، وcursorDone، وisFinal، وbound(h).
- Consensus.headerPrecheck/validateFull.
- ForkChoice (D133):
  - importSeq(hash).
  - key(b) = (signedFlag, importSeq, blockHash).
  - better(t1, t2): بالعمل، ثم مقارنة ابني السلف المشترك الأدنى.
  - best(candidates): الأعظم مستقلًا عن ترتيب الفحص.
  - onImport(b) → headChanged.
- ForkChoiceRef (Python) مستقل، ويستعمله HeadCheckRef في Q2b.

**الواجهات — التنفيذ**
GenesisBuilder.build، وExecutor.apply، وTipsAccumulator، وSystemCall، وPay.credit، وCapsTracker، وRewards.payInt، وEscrow.pending(j)، وSettlementIndex، وBurnMonitor.

**الواجهات — RPC (D88–D95)**
- HeadersParams.parse(json) → {from u64, count u64} | InvalidParams.
- HeadersParams.check(from, count, headFn) → ok | {fromZero | countRange | fromAboveHead | beyondHead}، وheadFn لا تُستدعى قبل F3.
- MethodTable.classify(method, params) → {class ∈ {heavy, light}, maxResp} | Unknown. جدول ثابت مطابق لـnetwork (ب).
- OriginDeny.check(method, hasOrigin) → ok | Deny، مع ORIGIN_DENY ثابت، ويُستدعى في المرحلة 2 قبل القبول.
- RateBucket.take(conn, k) → ok | Rate{retryAfterMs}، يستهلك k رموزًا دون انتظار.
- RpcGuard:
  - accept(conn) → ConnSlot | Reject: فوق 32 دون قراءة، ويضبط SO_SNDBUF = SEND_SOCK_BUF عبر socket2 قبل أي كتابة.
  - admit(class, deadline) → Admission{permit, reservation} | Busy{retryAfterMs}، قبول ذري FIFO.
  - Reservation: مقاطع 64 KiB، وwrite يرفض التجاوز بـOverflow، وshrink() وrelease() في Drop.
  - Admission.snapshot() → ReadTxn، يُحرر مع التصريح في endCompute().
  - endCompute(admission) → OwnedResponse{reservation}.
- ComputePool.run(admission, job) → Result | ScratchExceeded | Deadline | Cancelled، بـ20 خيطًا ودون await في job.
- CountingAlloc: عداد خيطي بحد لكل job.
- CancelToken: check() في حلقات المسح والتنفيذ.
- LogsQuery.run(filter, snap, anchor?, budget{bytes, count, scan, deadline}, cancel) → Complete(Reservation) | LimitP1{reason, fromBlock, lastCompleteBlock ≥ fromBlock, nextFromBlock} | LimitRes{reason ∈ {deadline, scratch}, fromBlock, lastCompleteBlock ≥ fromBlock−1, nextFromBlock} | AnchorReject{reason, head} | Cancelled. LimitP1 وLimitRes يُرمزان بـ−32020.
- SendPacer (D90، D92، D93، D94):
  - مبني على Clock وSocketSink مجردين، والتنفيذ نفسه في الإنتاج وPacerSim. لا يستورد tokio مباشرة.
  - SendPacer.new(clock, grace=2s, minRate=65536 B/s, slack=200 ms, timerEps=1 ms, trace?) → Pacer، بحالة {sendTime, produced P, accepted A, pending, deadlineTimer, finished}.
  - Pacer.produce(n): P += n. Pacer.finish(): يعلن انتهاء الإنتاج، وأول A = P بعده هو sendDone.
  - Pacer.onWrite(result): A += عدد مقبول، أو WouldBlock، ثم تحديث pending.
  - Pacer.violationAt() → s_v = max(grace, A/minRate)، والمؤقت على s_v+timerEps مع فحص احتياطي كل 50 ms.
  - Pacer.check() → Ok | Close{sendTime_ns, accepted, lateMs}.
  - Pacer.write(sink, bytes) → Done{sendDone_ns, sendTime_ns} | Closed{…}. Done يعني A = P = size فقط، ولا ينتظر قراءة العميل. المستدعي يحرر الحجز فور Done.
  - الطلب المنفرد بـPacer جديد، والدفعة بـPacer واحد، ولا واجهة لإعادة السماح.
  - rpc-fault:perItemGrace (simnet فقط) ينشئ Pacer لكل عنصر، ضابطًا سالبًا في PS5 وPS5c وRG14(vii).
- PacerSim.run(script, variant ∈ {conforming, perItemGrace}) → {outcome ∈ {sendDone, close}, closeAt{sendTime, A}?, sendDoneAt{t, sendTime, Rc, receivedItems, unreadAtSendDone}?, receiveDoneAt{t, receivedItems}?, lateMs, rows[{t, Rc, P, A, pending, sendTime, s_v}], trace}. script = {mode ∈ {acceptPlan[(sendTime_ns, bytes)], sockBuf SB}, items[S…], clientPauses (pauseAtItemFirstByte=1.8s), computeGaps?, timerJitterMs?}.
- PacerTrace: مخزن 4 MiB، والفيض في traceDropped. بنيته {conn, t_ns, sendTime_ns, P, A, pending, event ∈ {produce, accept, pendStart, pendStop, close, sendDone}}.
- TraceCheck.check(trace, clientLog, probeLog, params, rule ∈ {batchShared}) → {TCsend, TCrecv, CloseOK, Enforced, breachAt?, closeAt?, sendDoneAt?, receiveDoneAt?, unreadAtSendDone?, gap}. نسختان Rust وTS متطابقتان.
- DiscriminationCheck(tracePerItem, clientLog) → {condition: met|unmet, enforcedReportsBreach}.
- BatchStreamer.run(conn, items, pacer, wallDeadline): يكتب '['، ثم لكل عنصر: admit، ثم ComputePool، ثم endCompute، ثم pacer.produce وpacer.write حتى A = P، ثم تحرير حجز العنصر، ثم العنصر التالي. بعد ']' يستدعي finish وينتظر sendDone. أخطاء batchBytes وrate من CONN_BUF. wallDeadline = نهاية القراءة + BATCH_WALL، ويقيس حتى sendDone. maxResPerConn ≤ 1 مؤكد بـdebug_assert ومراقب.
- ConnLoop: بعد sendDone يبدأ مؤقت IDLE ويُسمح بقراءة الطلب التالي، دون أي انتظار لقراءة العميل.
- RpcStats.snapshot()، وMempool.admit، وRejectLog.

**الواجهات — السجل والتخزين**
Registry، وRetentionStore (مع LogCursor وفهرس (height, blockHash))، وTrieRebuild، وSyncPipeline، وOrphanQueue.

**الواجهات — التجهيزات**
- ExecFixture.run، وFixtureMiner.build، وPlan.override، وFixtureVerify، وPeerHarness (مع خطاف بين طلبات العميل).
- MaliciousRpc.serve(chain, {drop?, replace?, fakeHeight?, extra?, refTs?, clock?, targets?, busyCount?}).
- MockLogServer.serve(branches{A, B, A'}, script): ردود حرفية، وتبديل الفرع عند خطوة محددة، وإجابة eth_getBlockByNumber من الفرع الحالي، وتسجيل كل طلب بترتيبه ونوعه (H أو سجلات) ومحاولته، وخيار احتجاز الرد (لـBR19b(b4)).
- SinkChecker.wrap(sink, serverLog): يفحص (S-a)–(S-e) ويسجل التتبع.
- EndFixture.ids(n=1020) → قائمة مرتبة أو FixtureSetupError('ids').
- DenseFixture.build(profile='logs/dense', branches=['B', 'A'']).
- RpcAttacker.run({…}) → سجل زمني رتيب لكل بايت ولكل عنصر، ولحظة receiveDone أو القطع، ولحظة الإغلاق.
- ReferenceLogs.read(from, to, branchHash).
- ResourceProbe.sample(pid, 100 ms) → {rss, heavyResv, lightResv, scratchHeavyMax, snapshotsOpen, conns, maxResPerConn, pacerClosed, pacerLateMaxMs, batchWallHits, traceDropped, sendDoneCount}.
- RpcRecorder، وSimNet.run، وLoadGen، وKeccakTriad.check، وAsertRef.
- RpcTap.serve(upstream, port) → سجل {seq, tIn, tOut, origin, method, params, source ∈ {readClient, walletSubmit, logClient, headerNet, other}}، وRpcTap.hold(method, ms) يؤخر الرد. المصدر من ترويسة اختبار تضيفها الوحدات في بناء الاختبار فقط.
- BadSite.build(vectors=BR1–BR13 وBR16–BR19c, txA): موقع منشور يرسل الرسائل الحرفية عبر الجسر ويسجل الردود. GoodSite موقع ثانٍ لاختبار العزل في BR17f.
- TestHooks: approvals()، وconfirmations()، وautoConfirm(accept|reject)، وtabsCreated() باعتراض chrome.tabs.create، وsiteStorageSnapshot(netKey, addr) بايتيًا، وstorageSetCalls() باعتراض chrome.storage.local.set، وbridgeTrace(frame).
- BridgeFuzz.run(seed, n=10^4) → رسائل وردود.
- FakeClock (nowMs قابل للضبط) وFakeTransport (يحتجز الردود حتى release(i) أو timeout) لـBR19a وBR19b.
- SchemaRef (Python): المخطط والرمز المتوقع لـB0–B3. BridgeRef (Python): يضيف Bpre بالدلو الصحيح وقيود B4 من أوقات BridgeTrace وRpcTap، ويعيد القرار المتوقع لكل رسالة. SiteStorageRef (Python، ضمن BridgeRef): قاموس لكل (netKey، عنوان)، وapply(msg) → {decision ∈ {null, 4300}, totalBefore, totalAfter} بمحاسبة entry وnewTotal الحرفية.

**الواجهات — الإضافة**
- HeaderNetCheck.plan(h, kView=12) → {b, from, count, n, hasRef, anchoredGenesis}.
- HeaderNetCheck.ceil(targetG, slackS=4)، وminWork(ceilTarget) بـBigInt.
- HeaderNetCheck.window(head, fetch, cfg, clock) → {ok, n, anchoredGenesis, minWork} | {rule}.
- TrustMessage.render.
- BridgeBucket (D97): new(createdMs) → {tokens_mt: 50000, last_ms: createdMs}. take(nowMs) → ok | rate، بالصيغة الصحيحة في browser، دون فاصلة عائمة.
- BridgeAuth (D95، D96، D97):
  - ALLOW: Map مجمدة من (kind، method) إلى Schema، تُبنى عند تحميل الوحدة، وتشمل site_storageSet وsite_storageClear تحت storage_set، وsite_navigate وsite_openExternal تحت nav.
  - check(rawText, frameCtx, arrivalMs) → {ok, id, kind, route ∈ {readClient, logClient, wallet, siteStorage, navigator}, method, params} | {id|null, code ∈ {−32600, −32005, 4200, −32602}, data}. تنفذ Bpre–B3 فقط. دالة نقية بلا اتصال شبكي ولا أثر محلي، عدا استهلاك frameCtx.bucket. لا تقرأ pendingReads ولا الحصة.
  - parseStrict(rawText) → يرفض المفاتيح المكررة، والعمق > 8. الحجم يُفحص قبله في Bpre.
  - Schema.validate(params) → ok | {path}.
- Bridge.onMessage(raw): يختم arrivalMs، ثم check، ثم يوجّه إلى المكوّن، ويكتب سطر BridgeTrace في بناء الاختبار.
- RpcReadClient.send(frameCtx, method, params): يعيد فحص method ∈ READ_METHODS، وإلا 4200 دون اتصال. يفحص pendingReads < 4، وإلا −32005 {pending} دون اتصال. يزيد العداد قبل الإرسال وينقصه عند الرد النهائي أو مهلة 10s (−32603 {transport}). الناقل الشبكي الوحيد المصدَّر لـBridge.
- SiteStorage (D96):
  - الحالة: قاموس {key → value} لكل (frameCtx.netKey، frameCtx.siteAddress)، وtotal محسوب ومحفوظ في الذاكرة ومُتحقق منه عند التحميل بإعادة الحساب من القاموس.
  - entry(k, v) = utf8ByteLength(k)+utf8ByteLength(v) عبر TextEncoder، لا String.length.
  - set(frameCtx, key, value|null) → ok | Quota{limit}: يحسب newTotal = total − (entry القديم إن وُجد) + (entry الجديد إن لم تكن value = null). إن newTotal > 1048576 يعيد Quota دون أي كتابة. وإلا يبني القاموس الجديد ويكتبه بـchrome.storage.local.set واحدة، ثم يحدّث total. فشل الكتابة → −32603 {transport} وإعادة تحميل من التخزين.
  - clear(frameCtx) → ok، بكتابة قاموس فارغ.
  - snapshot(frameCtx) → قاموس للمحمّل عند التحميل.
  - التنفيذ عبر StoreQueue (D101)، فلا تتداخل رسالتان لمفتاح واحد.
  - check(frameCtx, msg) → ok | Quota{limit}: يحسب newTotal وحده دون كتابة، ويستدعيه StoreQueue.evalHead.
  - set لا يُستدعى إلا لرسالة ready اجتازت check، ويعيد حساب newTotal دفاعيًا. أي اختلاف عن check خطأ تنفيذ يُعد في stats.quotaRecheckMismatch، وقيمته غير الصفرية فشل.
- StoreQueue (D101):
  - admit(session, msg, arrivalMs) → queued | Store، قبل الإلحاق، بفحص العدادات الأربعة.
  - evalHead(key) (I56): يُستدعى عند تحرر رأس المفتاح، أو عند قبول رسالة لمفتاح خالٍ. يستدعي SiteStorage.check:
    - Quota: الرد 4300 ثم release.
    - ok: الرسالة تصير ready.
    - لا يأخذ مقعدًا ولا يستدعي StorageBackend.
  - pump(): إن توفر مقعد، يأخذ أقدم رسالة ready في FIFO العام بترتيب القبول، فيجعلها active ويستدعي set. كتابة نشطة واحدة لكل مفتاح، و≤ 2 لكل الإضافة.
  - onSettle(reqId): الحدث الوحيد الذي ينقل رسالة نشطة إلى released.
  - expire(nowMs): يحرر المنتظرة التي انقضى انتظارها مع رد store. ويرد storeTimeout للنشطة التي انقضت كتابتها دون تحرير.
  - release(msg) داخلي، يحرسه علم released. يؤكد debug_assert عدم التكرار، ويُعد releaseCount وdoubleReleaseCount في stats(). أي قيمة غير صفرية لـdoubleReleaseCount فشل.
  - الحالة لكل رسالة {replyState, ownState} وفق D102، وتُكتب في BridgeTrace.
  - cancelFrame(frame): يلغي المنتظر ولا يمس النشط.
  - awaitActive(key, ≤ 5000 ms) لـNavigator.
  - stats() لـTestHooks.
  - يعمل فوق StorageBackend مجرد، والإنتاج chrome.storage.local والاختبار FakeStorage.
  - المفتاح مشتق من سياق الإطار لا من الرسالة.
- EpochFence.boot(backend, monoClock) → {epoch} | Disabled{reason ∈ {epochNames, retry}} (D134):
  - يسرد عناصر epoch، ويطبق بوابة الأسماء ثم نافذة الأجيال.
  - ثم يكتب 'pocol:epoch:'+hex16(E) وينتظر تأكيده.
  - لا تُصدَّر أي كتابة بيانات قبل نجاحه.
- EpochFence.sweep(): يُجدول عند مضي T_LATE رتيبًا من الإقلاع، ويحذف الأسماء الأصغر من الأكبر الظاهر.
- EpochFence.stats → {epochNames, epochGateBlocks, epochResurrected}.
- RecoveryGate.await(key) → {dict, total, confirmed} | Failed{reason ∈ {corrupt, checkpoint}} | Busy | ReadTimeout: وعد مشترك واحد لكل مفتاح في الجيل. يحمل gateId، ويبدأ مهلة القراءة الكلية عند الانتقال إلى recovering (D110). يصدر نقطة التثبيت ومحاولتها الثانية فقط، كل منهما بمقعد من RecoverySlots. Busy وReadTimeout يعيدان المفتاح إلى unrecovered دون إصدار. كل رد قراءة يُفحص gateId له قبل استعماله.
- ReadSlots (D110): acquire(op, deadlineMs) → Slot | Timeout عبر ReadFIFO. slot.settle() هو الحدث الوحيد للتحرير. stats → {held, timeouts, lateDropped}. StoreQueue لا يقيّم رأسًا ولا يعطي snapshot قبل ready.
- RecoverySlots (D109): acquire(op, waitMs=5000) → Slot | Busy عبر RecoveryFIFO، في الخطوة المتزامنة نفسها مع DiskLedger.reserve والإصدار. cancel(request) عند ready. slot.settle() الحدث الوحيد للتحرير بعد الإصدار، ولا واجهة تحرير عند المهلة. stats → {held, unsettledMax, busy}.
- SeqAlloc.alloc(key) → u32 | Exhausted: المخصص الوحيد لأرقام set في الجيل.
- DiskLedger (D105):
  - reserve(op) → DiskTicket | {disk|sites|entries, limit}: دالة متزامنة بلا await، تفحص وتحجز في خطوة واحدة.
  - ticket.settled(settleSeq).
  - refresh() متسلسل، يستدعي getBytesInUse وgetKeys، ويسقط عند اكتماله التذاكر التي settleSeq لها أقل من رقم صدوره.
  - StoreQueue وRecoveryGate وAdminDelete لا تصدر set دون تذكرة.
  - (D112) lateGens(now) من عناصر epoch المسرودة عند الإقلاع وعنصر الجيل الحي. lateSites(now) = 4·lateGens.
  - reserve(op) يحدد كون العملية منشئة لمفتاح: set بيانات أو تثبيت لمفتاح غير محتسب وغير محجوز. ثم يطبق شرط المواقع ويعيد {sites, limit, retryAfterMs?} عند الخرق.
  - onWindowExpiry(X) يجدول refresh، ويسقط إسهام X عند اكتمال refresh صدر بعد bootMs(X)+T_LATE.
  - removeLastRecord(key) يُسمح به فقط بشروط D112، ويستدعيه TombReaper وحده.
  - (D113) reserve(op) على مفتاح محتسب لا يحجز مقعدًا، ويضيف pin للمفتاح ما لم تكن العملية remove. refresh يسقط المفتاح من sitesLive فقط إن لم يره القياس ولم يكن له pin، ويسقط pin في خطوة إسقاط التذكرة نفسها.
  - TombReaper.scan(): يُستدعى بعد كل refresh معتمد وعند كل خروج epoch من النافذة، ويصدر remove للقبور الوحيدة المؤهلة. stats → {tombReaps, pinnedKeys}.
- StorageManager (صفحة الإضافة): list()، وdeleteSite(netKey, addr) بتأكيد المستخدم، وتعيد وعد AdminDelete المشترك → deleted | failed | uncertain.
- AdminDelete (D106، D108): run(key) بخطواته الأربع وopId فريد في الجيل (D111)، وSessionRegistry.closeAll(key)، وViewer.blockOpen(key). النتيجة → deleted | failed | uncertain | adminBusy.
- AdminSlots (D108):
  - acquire(op, waitMs=5000) → Slot | Busy، عبر AdminFIFO، ويتم في الخطوة المتزامنة نفسها مع DiskLedger.reserve.
  - slot.settle(): الحدث الوحيد للتحرير. لا واجهة تحرير عند المهلة.
  - (D111) acquire(op{opId, key}, waitMs) → requestId، وcancel(requestId) يُستدعى في خطوة الحسم.
  - grant(request) يستدعي guard(op, key) قبل DiskLedger.reserve وSeqAlloc.alloc، بلا await بينها. الفشل يعيد المقعد إلى رأس FIFO التالي ويزيد adminStaleDropped.
  - DeleteOp: {opId, key, state ∈ {running, settled}}، وsettle(result) ينقل الحالة ويسحب الطلبات في الخطوة نفسها.
  - stats → {held, unsettledMax, busy}.
- RecordCodec: name(key, epoch, seq)، وencode/decode لصيغة fmt 2 بفحص صارم وحد RECORD_BYTES_MAX.
- StorageBackend يضيف ثلاث دوال:
  - listKeys(): عبر getKeys فقط.
  - bytesInUse().
  - remove(keys).
- ViewerLink: hello{genId}، وonLost() → teardownAll() مع الإعلان.
- SR-model (Python)، وFakeBackend(pending, scheduler) لـBR22a وBR22b.
- Navigator (D96، D99): navigate(frameCtx, path) ينفذ بالترتيب:
  1. يخصم من دلو تنقل الجلسة، وإلا −32005 {nav}.
  2. يطبق تطبيع storage.
  3. يحول طلبات الإطار إلى يتيمة، ويلغي LoadJob السابق، ويستدعي StoreQueue.cancelFrame، ثم awaitActive قبل snapshot (D101).
  4. يطلب من viewer هدم الإطار وبناء إطار جديد ينضم إلى الجلسة نفسها، أو يعرض صفحة 404.
- SiteSession (D99):
  - الواجهات: open(tabId, netKey, siteAddress) بفعل المستخدم فقط، وattach(frame)، وorphan(frame)، وsettle(reqId)، وclose().
  - تملك BridgeBucket وNavBucket وpendingReads وLoadJob وChunkCache وحجز SESSION_RECV_MAX. openExternal(frameCtx, url) → confirm → null | 4001 | −32005 {pending}، ويفتح التبويب دون opener.
- WalletSubmit.submit(rawTx, approvedTxHash): يتحقق من keccak256(rawTx) = approvedTxHash ثم يرسل eth_sendRawTransaction. لا يُستدعى إلا من Wallet بعد الموافقة.
- HttpTransport (D98): داخلي، تستورده RpcReadClient وWalletSubmit وLogClient وHeaderNetCheck وNetworkProfiles وChunkFetcher فقط.
  - request(method, body, {recvLimit, deadline, pool, session?}) → {json} | {error ∈ {recvLimit, recvDepth, recvParse, transport, busy}}.
  - لا يستدعي text() ولا json().
- RecvGuard:
  - reserve(pool, session, bytes, waitMs=2000).
  - read(stream, limit): يعد كل قطعة قبل إلحاقها.
  - release(): عند الاستقرار.
  - stats(): لـTestHooks.
- ChunkFetcher.fetch(addr, len, session): يبحث في ChunkCache أولًا، ثم يجلب عبر HttpTransport من CONTENT_POOL.
- MaliciousHttp (Node): خادم سكربتات لـMR. قاعدة dependency-cruiser تمنع Bridge من استيراده أو استيراد WalletSubmit.
- LogClient: plan(from, to) → نوافذ ≤ 1024 بوسم withinLimit=false. step(range, reply) → Append | Replace | Retry | Restart | Error، دالة نقية. Sink وRequestCounter (يرفض قبل الإرسال ما يجعل totalRequests > maxRequests). run(rpc, filter, sink, {timeout, maxRequests, maxRestarts=3}). fetchAll(rpc, filter, {maxBytes=4 MiB, timeout=30s, maxRequests=4096}) → Complete | Error{tooLarge, timeout, tooManyRequests, deadline, scratch, serverViolation, reorgUnstable, beyondHead, unsupported}، ويحتسب تعليقًا واحدًا للإطار. fetchEach(rpc, filter, sink, {timeout=600s, maxRequests=65536}).
- Viewer، وLoader (يبني localStorage من snapshot، ويجمع الكتابات كل 250ms مع طابور يحترم الدلو، ويعيد القيمة المؤكدة عند 4300، ويعترض الروابط)، وDnrGuard، وNetworkProfiles.validate، وWallet، وTestHooks، وCanarySink.

**الاعتماديات**
- M2 بعد M1، وM4 بعد M3، وM4b بعد M4، وM5 وM6 بعد M4، وM7 بعد M5، وM8 بعد M5 وM6.
- داخل M2: BridgeBucket وBridgeAuth وRpcReadClient مع BG1 وBR19a وBR19b أولًا، ثم SiteStorage مع جدول BR17e على قاموس وهمي، ثم WalletSubmit وNavigator وبقية BR، ثم T12.
- داخل M7: SendPacer وPacerSim وPS أولًا، ثم PacerTrace وTraceCheck وDiscriminationCheck، ثم RG وBR15.
- HeaderNetCheck وNetworkProfiles.validate يستوردان asert ونموذج M3-TS.
- LogClient يُختبر بـMockLogServer ثم مقابل pocold في RG3 وRG3b وRG9.
- المرحلتان A وB متوازيتان، عدا asert المشتركة التي تُسلم أولًا ضمن المهمة الثانية.

**أول مهمة قابلة للمراجعة والاختبار: M1-spec**
وثيقة نصية تتضمن:
- بايتات initcode وصيغة العنوان، بثلاثة متجهات: 0x61، و24575 بايتًا من 0xaa، وملف html محدد.
- ترميز manifest والتطبيع، وخانات Website والصلاحيات، وT1 وT16 بمدخلات حرفية.
- ملحق الإضافة: netKey بالمتجه (777001, 0x11×32)، وsignEligible، وQ8–Q12، وX1–X7، ود1–د5.
- ملحق تفويض الجسر (D95، D96، D97):
  - الغلاف {id, kind, payload} وpayload {method, params} نصًا، وترتيب Bpre وB0–B4 ورموز الأخطاء وبياناتها الحرفية، وقاعدة id المعاد (القيمة أو null).
  - صيغة الدلو بالأعداد الصحيحة (السعة والتجدد والكلفة وقواعد الاستهلاك)، وتعريف arrivalMs، وتعريف pendingReads وأحداث زيادته ونقصه.
  - مصفوفة (kind، method) كاملة لكل الأصناف الأربعة مع مخطط كل طريقة بصيغة قابلة للآلة، والأنواع الأولية بتعبيراتها النمطية ومنها str وpathStr وhttpsUrl.
  - دلالة SiteStorage: صيغة entry وtotal وnewTotal للإضافة والاستبدال والحذف والمسح، وشرط ≤ 1048576، والذرية بكتابة واحدة، واشتقاق المفتاح. ودلالة Navigator (التطبيع، والتأكيد، وعدم الرد للإطار المهدوم).
  - قائمة المحظورات المختبرة صراحة.
  - مخطط تدفق مسار الكتابة الوحيد حتى WalletSubmit.
  - قاعدة dependency-cruiser نصًا.
  - رسائل BR1–BR14 وBR17–BR18 حرفية بايتيًا مع الرد المتوقع، ومنها BR7a–BR7h بترتيب الخرق. وtxA الحرفية وهاشها، وأوامر RpcTap ومعيار «صفر طلب».
  - جدول BR17e كاملًا: الرسائل الأربع والعشرون بنصها الحرفي وطولها البايتي، وtotal قبل كل رسالة وnewTotal وtotal بعدها والرد: (e1) 61443·i حتى 1044531، و(e2) 1105974 → 4300، و(e3) 1048577 → 4300، و(e4) 1048576 → null، و(e5) 1048580 → 4300، و(e6) 1048577 → 4300، و(e7) 987137 → null، و(e8) 925694 → null، و(e9) القاموس النهائي ومجموعه 925694.
  - جداول BR19a (a1)–(a7) بقيم arrivalMs وtokens_mt قبل وبعد كل رسالة، وسيناريوهات BR19b (b1)–(b5) خطوة بخطوة، وسكربت BR19c ومواصفة BridgeRef وشرط الحسم.
  - مولد BR16 وبذرته، وSchemaRef وBridgeRef وSiteStorageRef، وصيغة BridgeTrace.
  - ملحق D98:
    - جدول RECV_LIMIT واشتقاقه، مع صنفي exact وcapped وصيغ WorstLegit وRecvFit (D100).
    - متجهات MR6b–MR6d وCAP1 وRF1–RF4، وسيناريوهات MR10a–MR10f بأزمنتها.
    - خطوات القراءة 1–6، والمجمعات.
    - سكربتات MaliciousHttp لـMR1–MR12 بالبايتات.
  - ملحق D99:
    - دورة حياة SiteSession، وحالات الطلب اليتيم.
    - صيغة دلو التنقل.
    - جدول BR20a المصحح بعمودي check وroute، وtokens_mt وpending بعد كل رسالة، ولحظة استقرار اليتيم. ويُنقل الجدول نفسه إلى BridgeRef.
    - جدول BR20b، وسكربت BR20c ومعاييره.
    - امتداد BridgeRef بدلالة الجلسة.
  - ملحق D101:
    - صيغة qbytes، والعدادات الأربعة، وترتيب FIFO لكل مفتاح، وحد النشط العام، والمهلتان.
    - قاعدة الهدم والانتظار قبل snapshot.
    - جداول BR21a (q1)–(q10) (بـq10 المصحح بست عشرة جلسة)، وBR21b (r1)–(r6)، وBR21e، وBR21f و(f2) لرفض الحصة قبل set، بالأزمنة والعدادات والردود وأحداث reply وrelease وstoreViolated، ومعلمات TQ، وسكربتا BR21c وBR21d، ومخطط الانتقالات replyState × ownState (D102).
    - امتداد BridgeRef وSiteStorageRef بنموذج StoreQueue.
  - ملحق D103:
    - صيغة أسماء عناصر epoch والسجلات وقيمها، وقاعدة اختيار الأكبر.
    - (D134، أول ما يُراجع قبل تنفيذ تعافي التخزين):
      - الاسم المشترك لكل E، وبوابة الأسماء، وقاعدة الكنس بعد T_LATE.
      - لِمّات الكتّاب وحد الأسماء والتغطية نصًا.
      - جداول BR22h (h1–h6) مع الضوابط الثلاثة، وامتداد SR-model.
    - خطوات السياج والاسترداد، واللِّمّتان نصًا.
    - سلوك viewer عند الزوال.
    - جدول تركيبات BR22a كاملًا مع الحالة المتوقعة لكل تركيبة، وسكربتا BR22b وBR22c.
  - ملحق D104:
    - آلة حالات RecoveryGate، وSeqAlloc وحدوده.
    - صيغة fmt 2 واشتقاق RECORD_BYTES_MAX.
    - قاعدة قبول القرص، ونافذة الأجيال، والكنس، والقبر.
    - جداول BR22d بمتغيراتها V-a..V-e، وBR22e، وBR22f بمتغيراتها f2–f4 مع آلة حالات RecoverySlots.
    - BR22g (g1–g6) مع مرحلة القراءة وReadSlots وgateId (D110)، واشتقاق حد الحسم 25s من t_s.
    - BR23 (d1–d11 مع d11b المصحح بالصيغة الكاملة لشرط الأسماء ومسار الضابط epochReserveOmitted، D135)، وBR24 حرفيًا، مع آلات حالات AdminDelete وDiskLedger وAdminSlots (D105–D108).
    - اشتقاق LATE_NAMES = 24 دون عناصر epoch وEPOCH_NAMES_MAX = 8 (D109، D134؛ القيمة 28 ملغاة)، وقيد META_RESERVE، وحد AdminDelete = 45s، وجدول BR24(x9–x12) مع x12b ومتغير BR22f-f5.
    - آلة حالات DeleteOp (D111) وحارس المنح لـAdminSlots وRecoverySlots، ولِمّة عدم القبر المتأخر نصًا.
    - (D112) تعريف sitesLive، والعمليات المنشئة، وصيغة lateGens وLATE_SITES وقاعدة إسقاطها، وشرط القبول، وإزالة القبر الأخير، ولِمّة المواقع نصًا، مع A15d.
    - جداول BR23(d1، d9، d12–d15) بأزمنتها وقيم lateGens وLATE_SITES والقياس، وBR24(x6)، وامتداد SR-model.
    - (D113) قاعدة المفتاح المحتسب بقبر، وpin، وTombReaper، وشرط الحاجة إلى مقعد جديد، نصًا.
    - (D114، I68) جدول BR23(d15c) المصحح بالمسارين الصحيح وdropPinned، يشمل:
      - أزمنة الإقلاع والانقضاء.
      - قيم lateGens وLATE_SITES وsitesLive وpinnedKeys بعد كل refresh.
      - خطاف DiskLedger.refreshNow() لبناء الاختبار فقط، ونقل الجدول إلى SR-model.
      - الرد الحرفي عند 30100 في المسارين، وينقل إلى SR-model نصًا:
        - الصحيح: 4300 {reason: 'sites', limit: 64} دون retryAfterMs.
        - dropPinned: 4300 {reason: 'sites', limit: 64, retryAfterMs: 29900}.
        - اشتقاق القيمتين من شرط D112.
- متجهات HeaderNetCheck: chainId=777002، وRW1–RW6، وh=3 مرساة على genesis، وviewIncomplete لنقص رأس ولنقص المرجع عند h=20، وviewGenesis، وh=0 → viewNoBlocks، وHC1–HC7 بقيم minWork العشرية الحرفية ونصوص الرسالة، وNX-V3e وNX-V3f وNX-V3g.
- متجهات LogClient: الفروع A وB وA' بسجلاتها وهاشاتها الحرفية، وجدول 8/0/2، وLC1–LC18 بردود JSON حرفية، ومنها أشكال −32020 (P1 وRES) و−32021 (busy وrate) و−32022، وتسلسل الطلبات الحرفي مع H(·)، وتتبع sink مع summary لكل commit، وجدول totalRequests: LC1=5، وLC5=5، وLC8=7، وLC10=4 مرسلة، وLC16=11، وLC17=12 (متغيره 8)، وLC18=10، وRefA وRefB وRefA'، وجدول انتقالات step كاملًا لكل تركيب (سبب، lc، a=b، withinLimit).
- NetworkProfiles.validate: GSV1، وN1، وN6، وN11.
- X8: صفحات C1–C8 والأعلام K0–K5.

يراجعها Codex، ثم تُنفذ خارج هذه الجلسة.

**المهمة الثانية (M3، TS)**
- الترميز والهوية: K1–K3، وKeccakTriad، وGenesisSpec، وNetConfig، وNetId.
- الصعوبة: asert مع AS1–AS19 وAS-eq وASQ وASB.
- البوابة: ParamGate مع PG-END1–5 وPG-L وPG-RPC.
- الرؤوس: Codec (L0)، وH-pre البنيوي (L1)، والبحث (L2)، وTiming (TJ)، وAssigner.
- التنفيذ: RegistryLayout، وpayInt، وBurnMonitor.
- RPC والإضافة: HeadersParams (HG1–HG8 وHG14–HG16)، وHeaderNetCheck.plan (RW1–RW6)، وceil وminWork (HC1–HC7).

**ملحق VA-spec (D115)**
يُكتب قبل كود M5 ويراجعه Codex. يحتوي:
- الملف va/base حرفيًا ببايتات genesisPre ومعدلات الهويات.
- واجهة NetSim وأوامرها، وواجهة AdvNode (سياسات: silentProposer، وsplitTemplates، وheaderOnlyWithhold، وselfish، وdoubleSpend، وownSharesOnly، وemptyShares، وwinnerEquivocate، وsplitIdentity(k)، وchurn، وeclipse، وcensor).
- سكربت كل اختبار في ملحق VA بأزمنته ورسائله وأعطاله.
- صيغة TraceDB، وحساب كل مقياس ومقامه بمرجع Python مستقل.
- حاجز القياس (D122، D123):
  - أوامر halt وstopMining وstatusSweep.
  - تعريف Quiet آليًا من حالة NetSim ومن ChainWork لكل عقدة.
  - سياسة F1–F3 وpurgeFuture، والمسارات U وA وV مع purgeUnavailable وتعريف الحيازة وt_avail (D124)، ومجموعة أسباب U المغلقة {bodyUnavailable, envUnavailable, depUnavailable} وvalidEnv وEnvHeldAtHalt، بتصنيف واحد يقرؤه SimNet.recSet وHeadCheckRef وBudgetCheck (D131، D132)، وRec وDrainPin وNeed(i) وHR وRB وc_rd وT_REBUILD_MAX وPIN_MEM_MAX (D125)، وSYNC_RETRY_MAX بنطاقه وRESTART_MAX، وصيغة DRAIN_MAX(run) مع N_miss وc_b وDRAIN_CAP، وQUIET_HOLD.
- HeadCheckRef بمرجع Python، يشمل:
  - RefReplay.
  - متجهات QV1–QV26 مع QV23d وQV25c-dup وQV26m وT23 N9 وN10 حرفيًا، ومنها QV19–QV23 ونسخة QV20 المعدلة (D128) مع حذف نسختها السابقة، مع:
    - جدول BudgetCheck وأصناف ActLog.
    - SimParams الافتراضية وتجاوزات كل متجه.
    - الطوابع والأهداف ولِمّة تساوي العمل لكل ارتفاع.

    ومعها ملف qv/net ببايتات genesisPre وجداول الساعات والأهلية.
  - بناءات simnet-fault الستة والأربعون: envRequestBeforeParent (D140)، وenvCheckSharedPool (D139)، وenvReleaseOnCancel، وenvTimeoutCoversCheck (D138)، وenvSolicitUnbounded، وenvReplyUnreserved، وenvTimeoutExclude (D137)، وenvCacheUnbounded، وenvFullAsBlockReject، وenvDropSolicited (D136)، وpairwiseTiebreak (D133)، وglobalRtt، وenvAsBlockReject، وnoEnvUnavailable، وnoEnvFetch (D131)، وstatusResetsTried، وenvRejectCancels، وhfullDoneOnEnv، وexecPerCopy (D130)، وcacheByBlockHash، وmergeInFlight، وskipHp، وnoReeligible (D129)، وnoFetchDedup، وcacheNotEvaluable (D128)، وkpendIgnored، وnoHdrCache (D127)، وpinLate، وneedHeadsOnly، وrbPerBranch، وhdrUndercount (D126)، وrejectOnBadBody، وexcludeDepAsInvalid، وnoDrainPin، وdrainUndercount، وsilentDeepReorg (D125)، و noSignedTiebreak، وnoReorgOnHeavier، وexecDrift، وquietIgnoresFuture، وpurgeImported، وuntaggedTimer، وkeepUnavailable، وpurgeHeld، وskipHolder، وnoCancelOnReject (D124).
- جدول CEN، وBootRef (D121) مع متجهات بذور ثابتة ونتائجها الحرفية لمطابقة TS، ومتجهات SV10a–f (D120).
- جداول FSM1–FSM10 وN1–N8 وL1–L13 بالحالة المتوقعة.
- ReplicaHarness لـT25: ملفات T25Site حرفيًا، وأوامر kill وwipe وإنشاء S0 وتوقيعها واسترجاعها، وخريطة الأجهزة، وقائمة ReplicaMonitor.
- ملف caps/64 ببايتاته، وSYS_SLOT_MAX المشتق من R18، وعقد SlotFill، وحمولات L1–L13، وثوابت caps-test.
- AdvOracle وحده، وتعريف الاستثارة لكل اختبار مشروط، وT_RUN_MAX لكل اختبار.
- تقدير زمن التشغيل على Windows بـ8 GB، مع جدولة الأذرع على دفعات دون تغيير المعايير.

**ملحق M7-spec**
يُكتب قبل كود M7 ويراجعه Codex. يحتوي:
- جدول التصنيف مع اشتقاق أسوأ حجم لكل طريقة، وجدول ORIGIN_DENY.
- مخطط حالات دورة الملكية (1–7) لكل مورد، مع نقطة التحرير عند sendDone.
- مخطط SendPacer: P وA وpending، وبدء الساعة وإيقافها، وs_v وTIMER_EPS، وإعادة ضبط المؤقت، والفحص الاحتياطي، وCloseOK، وfinish وsendDone.
- نموذج PacerSim الحرفي: نمطا القبول، وقاعدة إنتاج العنصر التالي، وسلوك العميل، والتكرار حتى الثبات داخل اللحظة، وتعريف sendDone وreceiveDone وunreadAtSendDone.
- سكربتات PS1–PS8 حرفية مع جداول (t، Rc، P، A، pending، sendTime، s_v، الحدث) للمطابق ولـperItemGrace: جدولا PS5 وPS5c حتى receiveDone (PS5: sendDone = receiveDone = 36. PS5c: sendDone 32.4 بـ18 عنصرًا مقروءًا وunread = 196608، ثم receiveDone 36)، واشتقاق PS4 الدرجي.
- صيغة PacerTrace، وتعريف TC-send وTC-recv وCloseOK وEnforced وBreach وDiscriminationCheck حسابيًا، وشروط حالات C في RG14 كدوال على مسار التتبع الكامل، وحالة RG14(0b).
- جدول يحدد لكل مهلة الحدث الذي تقيسه، وكلها أحداث خادم.
- أشكال JSON الحرفية لـ−32020 بنوعيه و−32021 بسببيه و−32022.
- مثال دفعة متدفقة بايتيًا، وخطة RG بأوامر RpcAttacker ومعاييرها، وخطة BR15.
