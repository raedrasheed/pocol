# الترقيات والتشغيل

**معلمات genesis (فرضيات، profileKind=network، و[CP] = مضمنة في CP)**
- الشبكة: chainId=777001، وForkSchedule=[(1,0)].
- الصعوبة [CP]: g_ts، وtarget_g، وT_blk=60s، وτ=3600s، وnonceMode=0، وc=2.
- المحاولات: D_att=300 وΔ_fb_wait=25 [CP]. A_fb=1 وΓ=30 محليان.
- التوقيت المحلي: Δ_rel=26، وΦ=10، وφ=5، وΔ=5، وt_exec=3. المشتقات: σ=14، وX^h=22، وX^W=36.
- الحصص [CP]: m=32، وκ=2، وS_max=256، وD=16، وW_w=64، وα_bp=1000، وγ_bp=500.
- الاقتصاد [CP]: subsidy=2·10^18، وB_reg=10^20 wei.
- العضوية [CP]: R_max=2، وM_max=256، وM_min=1، وREG_RECORDS_MAX=16384، وSWEEP_MAX=16، وSWEEP_SCAN=64، وREG_OPS=16، وW_act=128، وΘ_inact=1024.
- الأدلة [CP]: E_win=2048، وU=2200، وE_max=4، وEVID_BYTES_MAX=8192.
- الكتلة [CP]: GAS_LIMIT=8000000، وbaseFee0=10^9، وTX_MAX=131072، وBODY_MAX=196608.
- السقوف [CP]: B_code_max=32768، وCODE_CAP=2^30، وTXBYTES_CAP=2^33، وTX_CAP=10^7، وSLOT_CAP=4·10^6، وACCT_CAP=10^6.
- SYS [CP]: SYS_SLOT_MAX=131072، وSYS_KEYS_BLOCK_MAX=2048، وSYS_KEYS_USER_MAX=256، وSYS_ACCT_BLOCK_MAX=512، وG_SYS=96·10^6.
- الاحتفاظ (محلي): K_hist=32، وHIST_TRIE_BUDGET=1717986918، وR_keep=2048.
- النهاية [CP]: H_END=129600، وT_END=g_ts+10368000، وH_CLOSE=129440، وT_CLOSE=T_END−9600، وZ_close=160، وRET=128، وP2=2.
- التأكيد (محلي): k_safe=6، وk_final=60، وk_view=12.
- الإضافة (محلي):
  - VIEW_SLACK_S=4، وΦ_view=10s.
  - fetchAll: 4 MiB لكل محاولة، و30s، وmaxRequests=4096، و3 إعادات بدء.
  - إعادة الكتلة المنفردة عند RES: 3 مرات بفاصل 500 ms.
  - BridgeAuth: BRIDGE_MSG_MAX=64 KiB، وعمق JSON ≤ 8، واسم الطريقة ≤ 64 B، وcallObj.gas ≤ 30000000، وFEE_HISTORY_BLOCKS_MAX=128، وFEE_PERCENTILES_MAX=16 (D100)، وaddress ≤ 16، وtopics ≤ 4×16.
  - دلو الجسر (D97، أعداد صحيحة): BUCKET_CAP_MT=50000، وBUCKET_REFILL_MT_PER_MS=50، وMSG_COST_MT=1000، أي 50 رسالة/s بسعة 50.
  - التعليق: PENDING_READS_MAX=4، ومهلة ناقل RpcReadClient 10s، ومهلة fetchAll 30s. تأكيد خارجي واحد معلق.
  - التخزين والتنقل (D96): SITE_STORE_MAX=1048576 بايتًا بمحاسبة entry = utf8len(k)+utf8len(v)، والمفتاح ≤ 256 B، والقيمة ≤ 61440 B، وفترة التجميع 250 ms، وpathStr وhttpsUrl ≤ 2048 B.
  - BridgeTrace: 1 MiB، في بناء الاختبار فقط.
  - StoreQueue (D101):
    - لكل جلسة: STORE_SESSION_MSGS_MAX=8، وSTORE_SESSION_BYTES_MAX=262144.
    - لكل الإضافة: STORE_GLOBAL_MSGS_MAX=64، وSTORE_GLOBAL_BYTES_MAX=4194304، وSTORE_ACTIVE_GLOBAL=2.
    - المهل: STORE_QUEUE_WAIT=10s، وSTORE_WRITE_DEADLINE=5s.
    - الحجم: STORE_MSG_OVERHEAD=64، وSTORE_DICT_CACHE_MAX=8 MiB.
    - المحمّل: ≤ 8 رسائل و≤ 256 KiB مرسلة دون رد، وانتظار ≤ 2s قبل التنقل.
  - التخزين المستدام (D104):
    - القرص: STORE_DISK_HARD=256 MiB، وSTORE_RECORDS_MAX=1024، وSTORE_SITES_MAX=64، وSITE_ENTRIES_MAX=4096، وRECORD_BYTES_MAX=1442048.
    - الأجيال: GEN_WINDOW_MAX=4، وT_LATE=60s، وLATE_SITES_PER_GEN=4 مشتق (D112).
    - الاحتياطيات: LATE_RESERVE=23072768، وLATE_NAMES=24، وEPOCH_NAMES_MAX=8 (D134)، وMETA_RESERVE=1 MiB.
    - القبور (D108): ADMIN_SLOTS=2، وADMIN_SLOT_WAIT=5s، وTOMB_BYTES_MAX=512، وEPOCH_BYTES_MAX=256.
    - الاسترداد: STORE_RECOVERY_DEADLINE=5s، ومحاولتا تثبيت، وRECOVERY_SLOTS=2، وRECOVERY_SLOT_WAIT=5s (D109).
    - قراءة الاسترداد (D110): RECOVERY_READ_DEADLINE=5s، وRECOVERY_READ_SLOTS=2، وRECOVERY_GETS_MAX=4. المشتقات: حسم البوابة ≤ 25s، وAdminDelete ≤ 45s.
  - RecvGuard (D98):
    - جدول RECV_LIMIT في browser.
    - RESP_DEPTH_MAX=16، وERR_DATA_MAX=4 KiB، وانتظار الحجز 2s.
    - SITE_POOL=32 MiB، وSESSION_RECV_MAX=10 MiB، وTRUST_POOL=2 MiB، وCONTENT_POOL=8·68 KiB.
    - RECV_INFLIGHT_MAX=48.
  - SiteSession (D99):
    - NAV_CAP_MT=6000، وNAV_REFILL_MT_PER_MS=1، وNAV_COST_MT=2000.
    - GLOBAL_PENDING_READS_MAX=16.
    - SITE_CACHE_MAX=4 MiB+64 KiB، وCACHE_GLOBAL_MAX=16 MiB.
  - المصفوفة وأسماء طرق site_* ثابتة في الكود لا معلمات قابلة للضبط.
- RpcGuard (محلي):
  - الاتصالات: RPC_CONN_MAX=32، وREQ_BUF=1 MiB، وCONN_BUF=64 KiB، وSEND_SOCK_BUF=32 KiB.
  - التصاريح: HEAVY_ACTIVE=4، وHEAVY_QUEUE=16، وLIGHT_ACTIVE=16، وLIGHT_QUEUE=32.
  - المعدل: RATE=20/s بسعة 20، والدفعة تستهلك k رموزًا.
  - الذاكرة: HEAVY_RESP_MEM=48 MiB، وLIGHT_RESP_MEM=16 MiB، وRESP_MAX=8 MiB، وLIGHT_RESP_MAX=512 KiB، وBATCH_RESP_MAX=16 MiB، وSCRATCH_HEAVY=16 MiB، وSCRATCH_LIGHT=1 MiB، وRPC_MEM_TOTAL=192 MiB.
  - الدفعات: BATCH_MAX=20، وERR_ITEM_MAX=256 B، وBATCH_BYTES_HARD=16782400 B.
  - السجلات: LOGS_RANGE_MAX=1024، وLOGS_COUNT_MAX=10000، وLOGS_SCAN_BYTES=32 MiB.
  - المهل: REQ_READ_DEADLINE=10s، وIDLE=30s (من sendDone)، وQUEUE_WAIT=2s، وCOMPUTE_DEADLINE_HEAVY=5s، وCOMPUTE_DEADLINE_LIGHT=500 ms، وSendPacer بـGRACE=2s وMIN_SEND_RATE=65536 B/s، وBATCH_WALL=420s، وCANCEL_MAX=100 ms.
  - الإنفاذ: ENFORCE_SLACK=200 ms، وTIMER_EPS=1 ms.
  - التتبع: rpc.pacerTrace (false افتراضيًا، وtrue في RG)، بمخزن 4 MiB.
  - حجب الأصل: ORIGIN_DENY = {pocol_submitShare، pocol_getTemplate}، ثابت في الكود.
  - المشتقات (لا تُضبط يدويًا، وكلها حتى sendDone): SEND_MAX(size) = GRACE+size/R+ENFORCE_SLACK، وBATCH_SEND_MAX = 258.0791015625s وحد مراقبته 258.2791015625s، والحد الأدنى لـBATCH_WALL = 404.2791015625s.
- احتفاظ الأغلفة (D136، محلي لا إجماعي): ENV_REJ_PER_BLOCK=8، وENV_REJ_GLOBAL=16384، وENV_ENTRY_MAX=128 B، وENV_PENDING_GLOBAL=1024، وENV_INFLIGHT_GLOBAL=64 = ENV_SOLICIT_SLOTS=32 + ENV_GOSSIP_SLOTS=32، وENV_REQ_TIMEOUT=2s، وENV_REPLY_MAX=3131 B، وENV_MEM_MAX=7 MiB (D137)، وENV_CHECK_WORKERS=4، وENV_EXEC_MAX=50 ms (D139، ويحل محل ENV_CHECK_MAX في D138). المشتق c_env = (⌈63/ENV_CHECK_WORKERS⌉+1)·ENV_EXEC_MAX = 850 ms، وقيد البناء ENV_CHECK_WORKERS ≥ 1.
  - ENV_EXEC_MAX فرضية مقيسة لزمن تنفيذ الفحص الواحد، وc_env مشتق منها (D139). تجاوزه يزيد envCheckOverrun ويطلق تنبيهًا، ولا يحرر المقعد قبل الاستقرار. السلامة مقدمة على التقدم.
  - قيد البناء: ENV_SOLICIT_SLOTS+ENV_GOSSIP_SLOTS = ENV_INFLIGHT_GLOBAL، وكلاهما ≥ 1.
  - قيد الذاكرة: ENV_REJ_GLOBAL·ENV_ENTRY_MAX + ENV_PENDING_GLOBAL·3067 + ENV_INFLIGHT_GLOBAL·ENV_REPLY_MAX + 16384·64 ≤ ENV_MEM_MAX.
  - المراقبة تضيف solicitHeld وenvTimeouts، وliveChecks وliveBuffers وcancelPending وlateCheckDropped وenvCheckOverrun (D138). أي liveChecks > 64 أو liveBuffers > 64 عيب تنفيذ يوقف الإصدار. تغييرها إصدار عقدة لا fork. المراقبة: envFloodDropped وenvEvicted وenvRejEntries، وتنبيه عند envEvicted > 1000 في الدقيقة.
- ثوابت مشتقة (D86): LAG=2048 (خارج CP)، وLmax = 4113.

**بوابة الإقلاع R1–R26**

ParamGate.check(profile, measured, nodeKind) تعيد ok، أو قائمة مرتبة بكل القواعد المخروقة، أو UnknownField. غير ok على العقدة الحقيقية يعني رفض الإقلاع بـ−32018.

التوقيت:
- R1: Φ=2φ.
- R2: Δ_rel ≥ σ+Δ+2t_exec.
- R3: Δ_fb_wait ≥ Φ+Δ+2t_exec.
- R4: Φ < Δ_fb_wait، وΔ_rel ≥ t_exec.
- R5: 3Δ_fb_wait ≤ D_att.
- R6: X^h وX^W.
- R7: D_att+Γ ≥ 2(X^W+c·T_blk)+Φ.

الأدلة والأحجام:
- R8: D ≥ 2، وU ≥ E_win+D، وE_win ≤ R_keep.
- R9: S_max ≥ 4κm، وB_code_max ≥ 24577.
- R10: TX_MAX ≤ BODY_MAX−16 KiB.
- R11: k_final ≤ R_keep، وk_view < K_hist، وk_view+2 ≤ 512.

العضوية والنهاية:
- R12: M_max ≥ |M_0| ≥ M_min ≥ 1.
- R13: H_END > Z_close، وH_CLOSE+Z_close ≤ H_END، وT_CLOSE ≤ T_END.

الأداء والموارد:
- R14: t_exec ≥ validate_p99.
- R15: حدود gossip.
- R16: REG_RECORDS_MAX ≥ M_max+R_max·(Lmax+LAG).
- R17: SWEEP_MAX ≥ R_max+E_max.
- R18: SYS، ويشمل كلفة RET سجلًا ضمن G_SYS.
- R19: أزمنة الكتل ≤ t_exec.
- R20: Z_close ≥ ⌈REG_RECORDS_MAX/RET⌉+P2+D+1.
- R21: trie التاريخي، والاستثناء الوحيد simnet مع retentionStress.

الملفات والهوية:
- R22: profileKind وnodeKind وفصل chainId.
- R23: Σalloc+|M_0|·B_reg+subsidy·H_END < 2^127.
- R24: (أ) فك GenesisSpec ومطابقة genesisHash وallocRoot وsysCodeHash. (ب) ForkSchedule غير فارغ، يبدأ بـ(1,0)، وتصاعدي. (ج) كل v مدعوم. (د) TestFork-v2 في simnet فقط. (هـ) chainId ≠ 0 ويتسع في u64.
- R25: (أ) 1 ≤ target_g ≤ 2^256−1. (ب) τ ≥ 1 وT_blk ≥ 1. (ج) في network: τ ≥ 10·T_blk. (د) معايرة target_g مسجلة.

خدمة RPC (محلية، لا تمس الإجماع)، R26:
- (أ) HEAVY_RESP_MEM ≥ RESP_MAX، وLIGHT_RESP_MEM ≥ RPC_CONN_MAX·LIGHT_RESP_MAX.
- (ب) HEAVY_RESP_MEM+LIGHT_RESP_MEM+HEAVY_ACTIVE·SCRATCH_HEAVY+LIGHT_ACTIVE·SCRATCH_LIGHT+RPC_CONN_MAX·(REQ_BUF+CONN_BUF) ≤ RPC_MEM_TOTAL.
- (ج) BATCH_RESP_MAX ≥ RESP_MAX، وLIGHT_QUEUE ≥ RPC_CONN_MAX، وBATCH_MAX·ERR_ITEM_MAX+64 B ≤ CONN_BUF.
- (د) RESP_MAX ≥ 2·LOG_BLOCK_MAX+546·512 B، وLOGS_COUNT_MAX ≥ 546، وLOGS_SCAN_BYTES ≥ LOG_BLOCK_MAX.
- (هـ) LIGHT_RESP_MAX ≥ أكبر قيمة خفيفة (385 KiB).
- (و) SEND_MAX وBATCH_SEND_MAX بالصيغة وحدها، وBATCH_WALL ≥ BATCH_MAX·(QUEUE_WAIT+ENFORCE_SLACK+COMPUTE_DEADLINE_HEAVY+CANCEL_MAX)+BATCH_SEND_MAX+ENFORCE_SLACK. RATE ≥ 1، والسعة ≥ BATCH_MAX.
- (ز) 0 < TIMER_EPS < ENFORCE_SLACK ≤ 500 ms، وSEND_SOCK_BUF ≤ 64 KiB.

فحص القيم الافتراضية: (أ) 48 ≥ 8، و16 ≥ 16. (ب) 178 ≤ 192. (ج) 16 ≥ 8، و32 ≥ 32، و5184 B ≤ 64 KiB. (د) 8 MiB ≥ 337 KiB. (هـ) 512 ≥ 385. (و) 420 ≥ 404.2791015625، و20 ≥ 20. (ز) 1 < 200 ≤ 500، و32 ≤ 64.

فحوص network: R16 = 12578 ≤ 16384، وR20: 160 ≥ 147، وR13: 129440+160 = 129600.

ثوابت الجسر لا تدخل ParamGate لأنها في الإضافة لا في pocold. تُفحص في اختبار وحدة BG1:
- BUCKET_CAP_MT = 50·MSG_COST_MT، وBUCKET_REFILL_MT_PER_MS·1000 = 50·MSG_COST_MT، وPENDING_READS_MAX ≥ 1.
- SITE_STORE_MAX ≥ 17·(3+61440)+4045، أي ≥ 1048576 بالضبط، حتى يبقى BR17e مشتقًا بأرقامه. القيمة الحرفية 1048576.
- BRIDGE_MSG_MAX ≥ 61440+256+128، حتى تتسع رسالة storage_set القصوى لمفتاح وقيمة بأقصى طول مع الغلاف.
- RECV (D100): لكل طريقة exact يجب RECV_LIMIT ≥ WorstLegit بقيم CP في network وsv-fix وend/full وlogs/dense، وبحدود مخطط الجسر (FEE_HISTORY_BLOCKS_MAX، وFEE_PERCENTILES_MAX، وcount ≤ 14).
- مجموعة capped تساوي حرفيًا {eth_call، eth_estimateGas}، ولكل منها سطر حد منتجي في browser.
- لا يُشترط RECV_LIMIT ≥ حد RpcGuard العام للطرق الثقيلة. هذا الفرق مقصود ومعلن.
- SESSION_RECV_MAX ≥ 8 MiB، وSITE_POOL ≥ SESSION_RECV_MAX، وTRUST_POOL ≥ 2·96 KiB.
- NAV_CAP_MT = 3·NAV_COST_MT.
- GLOBAL_PENDING_READS_MAX ≥ PENDING_READS_MAX.
- SITE_CACHE_MAX ≥ 4 MiB+64 KiB.
- (D101) STORE_SESSION_BYTES_MAX ≥ 64+256+61440 = 61760، حتى تُقبل أقصى رسالة في جلسة فارغة.
- (D101) STORE_GLOBAL_BYTES_MAX ≥ STORE_GLOBAL_MSGS_MAX·61760 (3952640 ≤ 4194304)، وSTORE_GLOBAL_MSGS_MAX ≥ STORE_SESSION_MSGS_MAX ≥ 1، وSTORE_ACTIVE_GLOBAL ≥ 1، وSTORE_QUEUE_WAIT > STORE_WRITE_DEADLINE.
- (D104) قيود القرص:
  - STORE_DISK_HARD ≥ STORE_SITES_MAX·2·RECORD_BYTES_MAX + LATE_RESERVE + META_RESERVE، أي 184582144+23072768+1048576 = 208703488 ≤ 268435456.
  - STORE_RECORDS_MAX ≥ 2·STORE_SITES_MAX + LATE_NAMES + EPOCH_NAMES_MAX + 1، أي 128+24+8+1 = 161 ≤ 1024 (D134).
  - (D108، D109) LATE_NAMES = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS+ADMIN_SLOTS)، وEPOCH_NAMES_MAX ≥ GEN_WINDOW_MAX+2 حتى لا تُغلق الدفعات العادية التخزين (D134)، وADMIN_SLOTS ≥ 2، وRECOVERY_SLOTS ≥ 2، وRECOVERY_SLOT_WAIT ≤ STORE_RECOVERY_DEADLINE.
  - (D110) RECOVERY_READ_SLOTS ≥ 1، وRECOVERY_GETS_MAX ≥ 2، و0 < RECOVERY_READ_DEADLINE < STORE_QUEUE_WAIT. الحد المشتق لـAdminDelete = RECOVERY_READ_DEADLINE+2·(RECOVERY_SLOT_WAIT+STORE_RECOVERY_DEADLINE)+2·(ADMIN_SLOT_WAIT+STORE_RECOVERY_DEADLINE) = 45s، وتعرضه أداة البناء حرفيًا.
  - (D108) META_RESERVE ≥ STORE_RECORDS_MAX·TOMB_BYTES_MAX + (GEN_WINDOW_MAX+1)·ADMIN_SLOTS·TOMB_BYTES_MAX + EPOCH_NAMES_MAX·EPOCH_BYTES_MAX، أي 531456 ≤ 1048576 (D134).
  - RECORD_BYTES_MAX = 4·ceil((SITE_STORE_MAX+8·SITE_ENTRIES_MAX)/3)+256، وLATE_RESERVE = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS)·RECORD_BYTES_MAX.
  - (D112) LATE_SITES_PER_GEN = STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS، وSTORE_SITES_MAX ≥ GEN_WINDOW_MAX·LATE_SITES_PER_GEN+1، أي 64 ≥ 17. تعرض أداة البناء الحد الأدنى للمواقع المقبولة فورًا بعد أسوأ نافذة أجيال حرفيًا (64−16 = 48).
- (I55) الشرط الأول يجعل الحد البايتي العام زائدًا دفاعيًا بالقيم الافتراضية، وهذا معلن. معلمات TQ في BR21e تخالفه عمدًا، وهي مسموحة في بناء الاختبار فقط. أي بناء إصدار بقيم StoreQueue غير الافتراضية يفشل.

أي خرق يفشل بناء الإضافة.

**ملفات التجربة**

P11 (dev): T_blk=10، وτ=600، وc=2، وΔ=1، وt_exec=1، وφ=1، وΦ=2، وΔ_rel=8، وΔ_fb_wait=6، وΓ=5، وD_att=60، وchainId=777901. المشتقات: σ=4، وX^h=6، وX^W=10.

anvil-br (المرحلة A، DEV): anvil بـchainId=777910 خلف RpcTap، مع المفاتيح 7–10 ممولة. لا يدخل ParamGate لأنه ليس pocold.

sv-fix (simnet):
- القيم: P11 مع nonceMode=1، وsubsidy=80000 wei، وtarget_g=2^240، وchainId=777902، وg_ts=1700000000.
- M_0 = {w, x, y, z} بالمفاتيح 3–6، مرتبة، وrewardAddr = id.
- النهاية: H_END=200، وH_CLOSE=40، وZ_close=160، وT_END=g_ts+10^7، وT_CLOSE=T_END. بقية CP بقيم network.
- alloc: s1..s4 (المفاتيح 7–10، بـ10^18 لكل منها)، وRv (كود 0x5f5ffd)، وSd (كود 0x73‹Escrow›ff ورصيد 5000).
- R16 = 12578، وR13: 40+160 = 200، وR20: 160 ≥ 147. على node يعطي [R22]. CP مطابق لـGSV1.

end/full (simnet، D86):
- القيم: sv-fix مع chainId=777905، وR_max=0، وM_max=1024، وM_min=1، وREG_RECORDS_MAX=1024، وRET=8، وH_END=700، وZ_close=147، وH_CLOSE=553، وT_END=g_ts+10^7، وT_CLOSE=T_END.
- M_0: 1024 هوية، هي w وx وy وz، و1020 هوية idᵢ = آخر 20 بايتًا من keccak256('PoColEnd'‖be32(i)) لـi ∈ [0, 1019]، بلا مفاتيح خاصة. rewardAddr = id، والقائمة مرتبة. أي تكرار أو تطابق مع عنوان محجوز يعطي FixtureSetupError('ids').
- alloc: كـsv-fix، مع s1 = 3·10^20.
- البوابة: R12: 1024 ≥ 1024 ≥ 1، وR16: 1024 ≥ 1024، وR17: 16 ≥ 4، وR13: 553+147 = 700، وR20: 147 ≥ 147، وR23 متحقق. R8 وR9 وR11 من network، وR1–R7 من P11، وR18 بـ8 سجلات/كتلة، وR21 بـH_END=700، وR26 افتراضيًا. النتيجة ok على simnet و[R22] على node.

logs/dense (simnet):
- القيم: sv-fix مع chainId=777906، وH_END=1300، وH_CLOSE=1140، وZ_close=160.
- alloc يضيف:
  - LogBomb: 546 سجل LOG0 لكل نداء، والبيانات 24 بايتًا آخر 8 منها = be64(arg).
  - MemHog: يوسع الذاكرة إلى حد 30M gas ويقرأ 10^4 خانة باردة.
  - Spin: حلقة حتى نفاد gas.
  - Blob: blob(uint256 n) يعيد n بايتًا 0xab، حيث n ≤ 4 MiB، والرد 2n+ترويسة ثابتة.
  - s2 = 10^20.
- R13: 1140+160 = 1300، وR20: 160 ≥ 147. النتيجة ok على simnet و[R22] على node.

أخرى: sv-fix-v2 (ForkSchedule=[(1,0),(2,50)]، وchainId=777904، وعلى node [R22, R24])، وcaps/64 (777903، معرّف في ملحق VA تحت T18b)، وP15، وP51، وP55.

**genesis (TM8، [PDF:96])**
- M_0 = 4 هويات HELD.
- target_g = min(2^256−1, max(1, floor(2^256/(H_cal·T_blk))−1)).
- أداة genesis تنتج genesisPre، ويُحسب الهاش بثلاث مكتبات keccak، ولا يُوقع إلا عند تطابقها.

**الترقيات (D72)**
- hard fork = ثنائي جديد يضيف (v, H_v)، بإعلان موقّع قبل ≥ 1440 كتلة.
- غير المحدثة ترفض بـnetVersion، فيقع انقسام مقصود معلن.
- تغيير CP أو specVersion يعني شبكة جديدة.
- قاعدة اختيار السلسلة (D133) إجماعية:
  - لا تغير الصلاحية، لكن اختلافها بين الإصدارات قد يفرق الرؤوس عند التعادل.
  - تغييرها يتطلب إصدارًا منسقًا وإعلانًا موقّعًا كالترقية، ولا يُعامل كتغيير RPC أو احتفاظ محلي.
- تغييرات خارج الإجماع لا تمس الصلاحية، وتُعلن بإصدار web3_clientVersion والإضافة. تشمل RPC والإضافة وRpcGuard وLogClient وPacerTrace وBridgeAuth وغلاف الجسر ودلوه وعدّاد تعليقه ومحاسبة حصة SiteStorage وORIGIN_DENY (D81، وD82، وD84، وD87–D97).
- D83 وD86–D97 لا تغير ناتجًا إجماعيًا.
- إضافة طريقة إلى مصفوفة الجسر تتطلب: سطرًا بمخططها داخل الغلاف الموحد، وتصنيفها قراءة أو كتابة أو محلية، وتحديث BR ومرجع Python، ومراجعة.
- أي طريقة تغير حالة الشبكة أو تبث أو توقع لا تدخل rpc_read أبدًا. أي طريقة محلية جديدة تحمل البادئة site_ ولا تدخل rpc_read.
- تغيير ثوابت الدلو أو التعليق يتطلب تحديث BR19 وBridgeRef وBG1 معًا. تغيير SITE_STORE_MAX أو حدي المفتاح والقيمة يتطلب إعادة اشتقاق جدول BR17e وSiteStorageRef وBG1 معًا.

**الإجراءات**
1. الإغلاق والنهاية، ثم لقطة موقعة.
2. بلوغ سقف: تجميد وتنبيه.
3. الطوارئ: إعلان موقّع بارتفاع ≥ 1440.
4. التوقف: بيان إعادة تشغيل موقّع (مركزية معلنة).
5. تراجع أعمق من R_keep: checkpoint يدوي.
6. إلغاء شبكة: تحديث networks.json.
7. K_eff < k_view: تنبيه.
8. RecLen > 0.9·REG_RECORDS_MAX: تنبيه.
9. فشل X8: سحب الإضافة.
10. خرق مراقب الحرق: يوقف الإصدار.
11. simnet على node: [R22].
12. FixtureSetupError ليس نتيجة اختبار.
13. > 10 رفضات 1ب أو templateForm في الساعة: تنبيه.
14. متوسط فاصل 100 كتلة خارج [0.5, 2]·T_blk: تنبيه.
15. شبكة جديدة عند h=0 أكثر من 10·T_blk: تنبيه.
16. > 50 خطأ −32018 في الدقيقة لـpocol_getHeaders من اتصال: تنبيه وإغلاق.
17. s ≥ VIEW_SLACK_S في رأس قانوني: تنبيه بأن RP لن تعرض.
18. closing دون cursorDone قبل H_END−P2−D: تنبيه (مستحيل تحت R20).
19. target_g·2^VIEW_SLACK_S ≥ 2^256 في ملف منشور: تنبيه بعدم فعالية حد RP.
20. ضغط RPC: تنبيه عند −32021 > 20% لـ5 دقائق، أو heavyResv ≥ 0.9·HEAVY_RESP_MEM لـ60s، أو conns = 32 لـ60s، أو pacerClosed > 100 في الدقيقة.
21. عيب تنفيذ يوقف الإصدار عند أي مما يلي:
    - snapshotsOpen > 20، أو maxResPerConn > 1، أو scratch فوق حده، أو lightResv > LIGHT_RESP_MEM.
    - cancelMaxMs > 100، أو pacerLateMaxMs > 200، أو sendOverrunMaxMs > 200.
    - batchSendMaxS > 258.2791015625، أو batchWallHits > 0.
    - حجز لم يُحرر عند sendDone.
    - خرق TC-send أو CloseOK أو Enforced في أي تتبع مسجل.

    العتبات = القيم الدقيقة + ENFORCE_SLACK دون تقريب.
22. anchorRejects > 10 في الدقيقة: تنبيه بإعادة تنظيم متكررة.
23. فشل أي متجه BR أو BG1، أو وصول طلب محظور إلى RpcTap أو RpcRecorder، أو تغير تخزين محلي أو فتح تبويب من رسالة مرفوضة، أو قرار جسر أو حصة يخالف BridgeRef أو SiteStorageRef، أو خرق قاعدة dependency-cruiser: يوقف إصدار الإضافة ويسحب أي نسخة موزعة.
24. originDenied > 0 على عقدة إنتاج: تنبيه، لأن أصل الإضافة لا يجب أن يطلب طرق التعدين.
25. (D103، D104) تنبيه محلي يعرض رابط «إدارة التخزين» عند أي مما يلي في الإضافة: storeRecoveryFail > 0، أو epochGateBlocks > 0 (D134)، أو recoveryCorrupt > 0، أو diskRejects > 0، أو removeFailures > 0، أو inUse ≥ 0.9·STORE_DISK_HARD. تُسجل القيم في T12. أي خرق لمعايير BR22a يوقف إصدار الإضافة.

**المفاتيح**
- genesis وcheckpoint والنشر وإعادة التشغيل وnetworks.json بنظام Shamir 2-من-3.
- مفتاح الهوية منفصل عن rewardAddr.
- multisig لمالك الموقع.
- مفاتيح التجهيز 1–10 للاختبار فقط، ومنها المعاملة الموقعة مسبقًا في BadSite.

**المراقبة (Prometheus)**
FSM وT_j، والرفض حسب القاعدة، وASERT (s وsaturated)، وEscrow، وpendingSum، والحرق، وRecLen، وSYS، وclosing والمؤشر، وK_eff، والقرص، والذاكرة، وactiveVersion، وأسباب −32018، وكل مقاييس pocol_getRpcStats ومنها originDenied. في الإضافة: عداد محلي لرفض BridgeAuth حسب الرمز والطريقة، ولرفض المعدل والتعليق والحصة، ولتأكيدات التنقل الخارجي المقبولة والمرفوضة، دون إرسال خارجي.

**مخرج M0**
- versions.lock، وcargo-deny، وlicense-checker، وأدوات Windows، والكناري.
- مكتبات keccak الثلاث (Rust sha3، وTS @noble/hashes، وPython pycryptodome) مع تراخيصها.
- ruint أو crypto-bigint، وhyper (MIT)، وsocket2 (MIT/Apache)، وdependency-cruiser (MIT).
- ميزة rpc-fault غائبة عن بناء الإصدار، وأخطاؤها المحقونة ثلاثة: lightInflate، وpacerOff، وperItemGrace. PacerTrace ليس خطأً محقونًا.
- بناء exec-fault للاختبار فقط، غائب عن الإصدار، بأربعة أخطاء محقونة:
  - winnerInState (D141) يكشفه TW1.
  - sharesFromFirstSeen وsharesFromMiner وsharesDropped (D142) يكشفها TW5.
- بناء fixture-fault للاختبار فقط، غائب عن الإصدار، بخطأ محقون واحد هو fixedShareThreshold (D143). يكشفه مسند تجهيز TW5 حتميًا.
- بناء bridge-fault للاختبار فقط، غائب عن الإصدار، باثنين وثلاثين خطأً محقونًا:
  - allowAll يعطل B2 لإثبات أن BR يكشفه.
  - legacyLocal يقبل الشكل القديم {key, value} و{path} لإثبات أن BR18-L يكشفه.
  - noRefill يعطل تجدد الدلو لإثبات أن BR19a يكشفه.
  - quotaNoSubtract يحسب newTotal دون طرح entry القديمة عند الاستبدال، لإثبات أن BR17e(e7) يكشفه.
  - frameBudget ينشئ الدلو والعدّادين لكل إطار ويتجاهل الطلبات اليتيمة، لإثبات أن BR20a وBR20c يكشفانه.
  - recvNoLimit يجمع الجسم كاملًا ثم يفحصه، لإثبات أن MR1 وMR4 يكشفانه.
  - storeUnbounded (D101) يعطل حدود StoreQueue ومهلها وإلغاء الهدم، لإثبات أن BR21a وBR21b وBR21e تكشفه.
  - storeEarlyRelease (D102) يحرر موارد الرسالة النشطة عند رد storeTimeout، ثم مرة ثانية عند الاستقرار، لإثبات أن BR21a(q8) وBR21b(r6) يكشفانه.
  - quotaNoRelease (I56) يأخذ المقعد قبل تقييم الحصة، ويرد 4300 دون release، لإثبات أن BR21f و(f2) يكشفانه.
  - singleItem (D103) يكتب فوق عنصر واحد للمفتاح، لإثبات أن BR22a يكشفه.
  - noEpochConfirm يكتب البيانات قبل تأكيد epoch، لإثبات أن BR22a(P4) وBR22b يكشفانه.
  - noCheckpoint يخدم snapshot دون نقطة تثبيت، لإثبات أن BR22a يكشفه.
  - reuseSeq يعيد seq بعد فشل الوعد، لإثبات أن BR22a(fail) يكشفه.
  - parallelRecovery (D104) ينشئ بوابة استرداد لكل تبويب، لإثبات أن BR22d يكشفه.
  - seqFromCheckpoint يشتق seq الكتابة من الرقم الاسمي 0 لنقطة التثبيت، لإثبات أن BR22d يكشفه.
  - sweepMax يسمح بحذف أكبر سجل، لإثبات أن BR22a وBR23 يكشفانه.
  - noDiskGate يعطل قبول القرص، لإثبات أن BR23(d2، d6) يكشفه.
  - diskNoReserve (D105) يفحص دون حجز ويسقط التذكرة عند settle، لإثبات أن BR23(d8–d11) يكشفه.
  - deleteUnordered (D106) يصدر القبر قبل إغلاق الجلسات، لإثبات أن BR24(x1) يكشفه.
  - readyOnApply (D107) يجعل المفتاح ready عند apply، لإثبات أن BR22d(V-a) يكشفه.
  - adminEarlyRelease (D108) يحرر مقعد القبر عند المهلة، لإثبات أن BR24(x9) يكشفه.
  - recoveryNoSlot (D109) يصدر المحاولة الثانية لنقطة التثبيت دون مقعد، لإثبات أن BR22f يكشفه.
  - lateReadCheckpoint (D110) يقبل نتيجة قراءة بعد انتهاء بوابتها ويصدر بها نقطة تثبيت، لإثبات أن BR22g يكشفه.
  - adminNoCancel (D111) يعطل سحب طلبات المقعد عند الحسم وحده، لإثبات أن الحارس يكفي في BR24(x12b) وBR22f-f5. يجب أن ينجح الاختباران فيه.
  - adminStaleIssue (D111) يعطل السحب والحارس معًا، لإثبات أن BR24(x12) يكشفه.
  - noLateSites (D112) يجعل LATE_SITES = 0، لإثبات أن BR23(d12) يكشفه.
  - seatOnCounted (D113) يطلب مقعد موقع لعملية على مفتاح محتسب بقبر، لإثبات أن BR23(d15) يكشفه.
  - dropPinned (D113، I68) يسقط مفتاحًا مثبّتًا بـpin عند قياس لا يراه، لإثبات أن BR23(d15c) يكشفه. يكشفه بالمسند المباشر عند 30050، وبقبول X عند 60100 بعد LATE_SITES=0، وبالعدد 65 عند 60350.
  - epochNonceNames (D134، I74) يعيد تسمية D103 القديمة بـnonce لكل إقلاع دون بوابة الأسماء، لإثبات أن BR22h(h1) يكشفه بـ41 اسمًا.
  - epochNoGate (D134) يستعمل الاسم المشترك دون بوابة الأسماء، لإثبات أن BR22h(h2) يكشفه بـ41 اسمًا.
  - epochSweepEarly (D134) يكنس الأسماء الأصغر فور التأكيد دون انتظار T_LATE، لإثبات أن BR22h(h5) يكشفه بـepochResurrected > 0.
  - epochReserveOmitted (D135، I64) يحذف EPOCH_NAMES_MAX من شرط قبول الأسماء، لإثبات أن BR23(d11 وd11b) يكشفه.
