# قواعد الإجماع

**0. الإعداد المثبت (D72، D77، D79)**

cfg = {chainId، genesisHash، ForkSchedule، CP، M_0}.

GenesisSpec = RLP([specVersion, chainId, CP, allocRoot, M_0List, sysCodeHash]):
- specVersion: u8 = 1، وإلا gsVersion.
- chainId: u64 ∈ [1, 2^64−1].
- allocRoot وsysCodeHash: 32 بايتًا بالضبط.
- genesisHash = keccak256(GenesisSpec)، ولا يشمل ForkSchedule.

الفك:
- RLP صارم، والأعداد بأقل بايتات، والصفر = 0x80.
- الأخطاء: gsInt (بادئة صفرية أو عدد ≤ 0x7f بغير بايت مفرد)، وgsCount، وgsRange (خارج المدى أو تعداد مجهول)، وgsLen.
- ترتيب التقييم: البنية، ثم gsVersion، ثم gsCount، ثم gsInt، ثم gsLen، ثم gsRange، ثم gsOrder، ثم gsSys.

CP (الإصدار 1، 52 عنصرًا بالترتيب):
- 1–8: g_ts u64، وtarget_g u256 [1, 2^256−1]، وT_blk u32 ≥ 1، وtau u32 ≥ 1، وnonceMode u8 (0 bounded، 1 unbounded)، وc u8 ≥ 1، وD_att u32، وdFbWait u32.
- 9–15: m u16 ≥ 1، وkappa u8 ≥ 1، وS_max u16 ≤ 256، وD u16، وW_w u16، وalpha_bp u16 ≤ 10^4، وgamma_bp u16 ≤ 10^4−alpha_bp.
- 16–24: subsidy u256، وB_reg u256، وR_max u8، وM_max u16 ≥ 1، وM_min u16 ≥ 1، وREG_RECORDS_MAX u32، وSWEEP_MAX u16، وSWEEP_SCAN u16، وREG_OPS u16.
- 25–30: W_act u32، وTheta_inact u32، وE_win u32، وU u32، وE_max u8، وEVID_BYTES_MAX u32.
- 31–40: GAS_LIMIT u64، وbaseFee0 u256، وTX_MAX u32، وBODY_MAX u32، وB_code_max u32، وCODE_CAP u64، وTXBYTES_CAP u64، وTX_CAP u64، وSLOT_CAP u64، وACCT_CAP u64.
- 41–45: SYS_SLOT_MAX u32، وSYS_KEYS_BLOCK_MAX u32، وSYS_KEYS_USER_MAX u32، وSYS_ACCT_BLOCK_MAX u32، وG_SYS u64.
- 46–52: H_END u64، وT_END u64، وH_CLOSE u64، وT_CLOSE u64، وZ_close u32، وRET u32 ≥ 1، وP2 u32.

العلاقات بين الحقول تفحصها ParamGate لا الفك. R_max=0 صالحة وتمنع register وreactivate.

المعلمات المحلية خارج CP لا تدخل genesisHash ولا الصلاحية: Δ، وφ، وΦ، وt_exec، وΔ_rel، وΓ، وA_fb، وK_hist، وR_keep، وk_*، وVIEW_SLACK_S، ومعلمات RpcGuard وBridgeAuth.

M_0List = [[id 20 B, rewardAddr 20 B]…]:
- M_min ≤ |M_0List| ≤ M_max، و|M_0List| ≥ 1.
- id تصاعدي بايتيًا بصرامة، والتكرار gsOrder.
- id ∉ {0x00×20، SYSTEM_ADDRESS، 0x…C0C001–0x…C0C0FF}، وإلا gsSys.

اشتقاق حالة genesis:
1. alloc.
2. أكواد النظام.
3. لكل مدخل سجل MinerRegistry: active وHELD، وbond = B_reg، وlastShare = 0، وgraceStart = 0. ActList بترتيب M_0List، وRecLen = ActLen = |M_0|.
4. Escrow += |M_0|·B_reg، وheldTotal = |M_0|·B_reg.

allocRoot = جذر MPT للحالة الناتجة، وعدم المطابقة يخرق R24(أ).

sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80 لـPAY])).

ForkSchedule = [(v, H_v)] مرتبة، تبدأ بـ(1,0). activeVersion(h) = آخر v بحيث H_v ≤ h. لا يُقرأ cfg من رأس أو نظير أو RPC.

**1. الترميز**

RLP صارم: بلا أصفار بادئة، وبأقصر بادئة، وبعدد عناصر دقيق، وبلا بايتات زائدة.

البنى:
- UT (18 عنصرًا، ≤ 613 B): [tag 'PoCol-tpl-v1', chainId u64, genesisHash, parentHash, h u64, a u32, protocolVersion u16, ts u64, target ∈ [1, 2^256−1], stateRoot, txRoot, receiptsRoot, logsBloom 256, gasLimit, gasUsed, baseFee, evidenceRoot, proposer 20 B أو فارغ].
- SignedTemplate = [UT, sig 65 أو فارغ] ≤ 682 B.
- Header = [SignedTemplate, nonce 8, shareList ≤ 256, winnerSig 65] ≤ 3067 B.
- Body = [txs, evidence] ≤ BODY_MAX.

الاشتقاقات:
- TemplateID = SHA256(RLP(UT)).
- SigMsg = keccak256(0x19‖'PoCol template'‖0x0a‖TemplateID).
- powHash = SHA256(TemplateID‖be64(nonce)).
- seal = keccak256(TemplateID‖be64(nonce))، وseal_genesis = keccak256(genesisHash).
- shareRoot = keccak256(RLP(shareList)).
- blockHash = keccak256(TemplateID‖be64(nonce)‖shareRoot)، وهاش الكتلة 0 = genesisHash.
- WinMsg = keccak256(0x19‖'PoCol winner'‖0x0a‖TemplateID‖be64(nonce)‖shareRoot).

قواعد إضافية:
- txRoot وevidenceRoot شجرتا MPT بمفتاح RLP(i).
- التوقيع: s ≤ n/2، وv ∈ {0,1}، وRFC6979.
- shareList تصاعدية بصرامة، ولا تحوي nonce الفائز.
- رأس genesis الافتراضي = {h=0، ts=CP.g_ts، seal=seal_genesis، blockHash=genesisHash}.

**1ب. استقلال القالب عن نتيجة التعدين (D141)**
- كل حقل في UT، ومنه stateRoot وreceiptsRoot وlogsBloom وgasUsed، دالة على ما يلي فقط: حالة الأب، ورأس الأب، وcfg، وحقول UT غير الجذور، والجسم.
- apply للكتلة h لا يقرأ أيًا مما يلي من الكتلة h: nonce، أو مالكه، أو shareList، أو winnerSig، أو sig.
- نتيجة الكتلة h (فائزها وحصصها) تُسجل في الكتلة h+1 بـrecordShares(h) وrecordResult(h)، من ParentResult(h) (D142) لا من أي حالة محلية.
  - ParentResult(p) = {blockHash(p)، winner، shares}. shares قائمة [(n, owner(n))] لكل n في shareList(p) بترتيبها التصاعدي، وwinner = owner(p.nonce). nonce الفائز حصة ضمنية تُعد لـwinner.
  - shareList ملتزم بـshareRoot داخل blockHash، فكل نسخ غلاف p تعطي القائمة نفسها.
  - المالكون من تعيين p: البذرة من seal الجد، وA_p* من حالة الجد. إذن ParentResult دالة على المحتوى الملتزم وحده.
  - أبوان بالقالب والnonce نفسيهما وshareRoot مختلفين كتلتان مختلفتا blockHash، ولكل منهما ParentResult مستقل. فيختلف stateRoot لابنيهما إن اختلفت الحصص، ولا يُخلط بينهما.
- لِمّة: لقالب UT ثابت، كل nonce يجتاز البنود 6–9 و13 في نطاق أي مالك في A_h* يعطي الجذور نفسها. إذن كتلتان بالقالب نفسه تشتركان في TemplateID وstateRoot، وتختلفان في blockHash فقط.
- الكتلة النهائية F بلا ابن، فلا تُسجل نتيجتها، وهذا متسق مع غياب مكافأتها المباشرة.

**2. العضوية**
- M_h = سجلات MinerRegistry في حالة الأب [PDF:87]، وAct_h = ActList ≤ M_max.
- A_h = {m ∈ Act_h، tomb=0 : lastShare ≥ h−W_act أو graceStart ≥ h−W_act}.
- A_h* = A_h، أو Act_h دون القبور إن كانت A_h فارغة.
- deregister وremoveInactive يفشلان إن جعلا |Act| < M_min.
- register وreactivate ≤ R_max لكل كتلة، ويسريان من h+1.
- في الإغلاق تُرفض عمليات السجل، فلا يزيد RecLen بعد closeH.

**3. التوقيت**
- L(a) = p.ts+a·D_att+1، وU(a) = p.ts+(a+1)·D_att، بحساب u128. الكتلة باطلة إن كان U(a) ≥ 2^63.
- الصلاحية: L(a) ≤ ts ≤ U(a).
- طابع الاحتياطي: L(0)+Δ_fb_wait عند a=0، وL(a) عند a ≥ 1.
- ts > C+Φ يعني طابورًا ≤ 256.
- المقترح يبني بـmax(L(0), C)، والانتقال إلى a+1 عند C > U(a)+Γ.
- T_j بالمللي ثانية، والوصول عند T_j بالضبط يقفل.

**4. آلة الحالات لكل (parentHash, h, a)**
- (أ) a=0: WAIT حتى T_j. القالب الموقّع الصالح من Draw(h,0) في لحظة ≤ T_j يقفل ثم PROP، وإلا FB لاصق. القالب المخالف كغياب، ويُعاقب النظير.
- (ب) Act فارغة: توقف.
- (ج) a ≥ 1: FB عند C ≥ L(a).
- قالب ثانٍ من المقترح نفسه: دليل من النوع 1، ثم FB.
- القالب السابق لأبيه يُخزن بعد فحص الشكل و1ب (≤ 4 لكل أب، و≤ 64 إجمالًا)، ويُحذف بـparentInvalid.

**4ب. Build_fb(p, a)**
- chainId وgenesisHash من cfg، وprotocolVersion = activeVersion(h).
- proposer وsig فارغان، وts وفق §3.
- target = ASERT(p)، وgasLimit = GAS_LIMIT، وbaseFee = nextBaseFee(p).
- Body = 0xc2c0c0، والجذور من apply الكامل.

**5. البذرة والتقسيم**
- seed = SHA256('PoColNonce'‖seal_p‖be64(h)‖be32(a)).
- ticket = SHA256(seed‖id20)، مرتبة تصاعديًا على A_h* [PDF:91، معدّل].
- start(i) = floor(i·nMax/n)، وend(i) = floor((i+1)·nMax/n)−1 [PDF:92].
- work(t) = floor(2^256/(t+1)).
- nMax = min(2^64, c·work(target)) في nonceMode=0، و2^64 في nonceMode=1.

**6. Draw**
- w(m) = Σcount[j][m] على j ∈ [h−1−W_w, h−2] لأعضاء A_h*.
- P+ = {w > 0}، وفراغها يعني اختيارًا موحدًا.
- x_k = SHA256('PoColProposer'‖seal_p‖be64(h)‖be32(0)‖be32(k))، مع رفض الانحياز.
- يُختار أول عضو بترتيب العنوان يتجاوز مجموعه التراكمي x_k mod W.

**7. ASERT بعرض ثابت (D74، D83)**

floor_div تقرب نحو −∞.
1. Δt = (p.ts−g_ts) − T_blk·p.h، في i128، و|Δt| < 2^97.
2. e = floor_div(Δt·65536, τ).
3. s = floor_div(e, 65536)، وf = e−65536·s ∈ [0, 65535].
4. s ≥ 256 يعطي 2^256−1، وs ≤ −257 يعطي 1، وينتهي الحساب.
5. F = 65536 + floor_div(195766423245049·f + 971821376·f² + 5127·f³ + 2^47, 2^48)، في u128، وF ∈ [65536, 131071].
6. X = target_g·F < 2^273، وk = s−16 ∈ [−272, 239]. Y = X·2^k إن كان k ≥ 0، وإلا Y = floor(X/2^(−k)).
7. target = min(max(Y, 1), 2^256−1).

لِمّة التكافؤ: X ∈ [2^16, 2^273)، فـk ≥ 240 يُقص إلى 2^256−1، وk ≤ −273 يعطي 0 ثم 1. إذن D83 = D74. للكتلة h=1 يكون الهدف target_g.

**8. الإغلاق والنهاية (D57، D85)**
- closing لاصقة، تبدأ عند h ≥ H_CLOSE أو ts ≥ T_CLOSE. closeH في أول كتلة closing، وcloseH ≤ H_CLOSE.
- المؤشر يعالج ≤ RET سجلًا لكل كتلة من closeH. cursorDone صحيح في حالة closeH+⌈RecLen_closeH/RET⌉−1 على الأكثر.
- gate2: h ≥ H_END−P2 أو ts ≥ T_END−P2·T_blk.
- final = closing_p و cursorDone_p و h ≥ closeH_p+D و (h ≥ H_END أو ts ≥ T_END). الكتلة النهائية بلا ابن صالح.
- البند 10أ: h ≤ H_END.

المبرهنة E1 (تحت R13 وR20):
- عند h = H_END يكون p.h ≥ closeH+⌈REG_RECORDS_MAX/RET⌉+P2+D.
- إذن cursorDone_p صحيح والكتلة H_END نهائية، ولا كتلة فوقها، وعدد الكتل غير genesis ≤ H_END.
- E1 حد أعلى لا ضمان وصول.

**H-pre** (بلا حالة، مع رأس الأب أو رأس genesis الافتراضي):
1. الترميز والأحجام، وكل معاملة ≤ TX_MAX.
1ب. NetID بالترتيب: chainId، ثم genesisHash، ثم activeVersion. الأخطاء: netChain، ثم netGenesis، ثم netVersion.
2. h = p.h+1، وparentHash = blockHash(p)، وh ≤ H_END (10أ).
3. الشكل: إما (proposer≠∅ وsig≠∅ وa=0 وecrecover = proposer)، وإما (proposer=∅ وsig=∅).
4. النافذة الزمنية وطابع الاحتياطي.
5. target = ASERT(p).
6. nonce < nMax.
7. powHash ≤ target.
8. winnerSig قابل للاسترداد.
9. كل حصة < nMax ومختلفة عن nonce، وSHA256(TemplateID‖be64(n)) ≤ T_share.

**H-full** (بحالة الأب):
10. final_p = false.
11. gasLimit وbaseFee.
12. التفرع الحصري: الموقّع يتطلب proposer = Draw(h,0). الاحتياطي يتطلب RLP(UT) = Build_fb(p,a).UT بايتيًا، وBody = 0xc2c0c0.
13. مالك nonce ∈ A_h*، وwinnerSig منه.
14. مالكو الحصص ∈ A_h*.
15. الجسم يطابق الجذور والحدود.
16. txBytes وtxCount ضمن السقوف.
17. apply يطابق stateRoot وreceiptsRoot وlogsBloom وgasUsed، وتأكيدات النظام محققة.

ترتيب التقييم (D75):
- العقدة تتوقف عند أول خرق (firstReject)، والتشخيص يقيّم كل بند قابل للتقييم.
- دون رأس الأب: البنود 2–9 notEvaluable. دون حالة الأب: البنود 10–17 notEvaluable.
- رسائل إضافية: unknownParent، وancestorInvalid، وparentInvalid، وtemplateForm.
- صنف الغلاف (D129): blockHash لا يلتزم بـsig ولا بـwinnerSig، فيُقسم كل خرق إلى صنفين:
  - envReject: خرق يتوقف على بايتات sig أو winnerSig وحدها. يشمل:
    - تعذر فك الرأس، أو ترميز حقلي التوقيع (البند 1).
    - تطابق وجود sig مع proposer واسترداده (البند 3، عدا شرط a=0 الملتزم).
    - استرداد winnerSig (البند 8).
    - كون موقّع winnerSig مالك nonce (الجزء التوقيعي من البند 13).

    أثره: يرفض نسخة الرأس hdrId = keccak256(RLP(Header)) لا الكتلة. يُعاقب المرسل، ولا يُسجل في pocol_getRejects ككتلة، ولا يطلق parentInvalid ولا ancestorInvalid، وتبقى الكتلة قابلة للقبول بنسخة أخرى.
  - blockReject: كل خرق آخر. هو دالة على المحتوى الملتزم، فيرفض blockHash ونسله.
- الكتلة صالحة إن صح محتواها الملتزم ووُجدت لها نسخة غلاف صالحة.
- العقدة تخزن أول نسخة صالحة الغلاف، وتخدمها في البث والمزامنة وpocol_getHeaders وGetEnvelope (D131).
- envWait (D131): الكتلة التي رُفضت كل نسخها المعروفة بـenvReject تبقى بلا حكم على blockHash. يستمر جلب غلاف بديل حتى تصل نسخة صالحة، أو تُزال الكتلة بحدود طوابير المزامنة أو D118. هذه الإزالة احتفاظ محلي لا بطلان. بقية النسخ لـblockHash مقبول تُسقط دون فحص.
- هذا تحديد تنفيذي، لا يغير ناتج الصلاحية ولا اختيار السلسلة.
- مراحل فحص الغلاف (D130):
  - envPre: بلا حالة، ويشمل فك حقلي التوقيع واسترداد sig وwinnerSig (البنود 1 و3 و8 في جزئها التوقيعي).
  - envOwn: بحالة الأب دون تنفيذ، ويشمل كون موقّع winnerSig مالك nonce في A_h* (الجزء التوقيعي من البند 13). كلفته حساب التعيين من حالة الأب، ويُخزن لكل أب.
  - قبل توفر حالة الأب تكون النسخة envPending، فلا حكم ولا رفض.
  - حكم envOwn نهائي لكل hdrId، لأن الأب ملتزم في blockHash. الاحتفاظ بالحكم المحسوم محدود بسياسة D136 المحلية (network). الطرد يسمح بإعادة فحص hdrId، والنتيجة الحتمية نفسها، ولا يغير صلاحية الكتلة.
- التنفيذ (apply) يجري مرة واحدة لكل blockHash، بعد أول نسخة envOk، ويُخزن حكمه في BlockCache.
- envReject لا يطلق RejectPath.cancel، ولا يضع HfullDone، ولا يُسجل في pocol_getRejects ككتلة. هذه الثلاثة مقصورة على blockReject أو الاستيراد.

**9. اختيار السلسلة (D133، يعدّل D15)**

من الأطروحة [PDF:74 §4.2]: قاعدة العمل التراكمي فقط. كاسر التعادل غير متاح في المقاطع المرسلة، فما يلي إضافة منا.

التعريفات:
- المرشحون: رؤوس الكتل المستوردة (المجتازة لـH-full) لدى العقدة.
- W(t) = Σwork من genesis حتى t.
- importSeq(b): عداد محلي رتيب يُسند لكل blockHash لحظة أول استيراد ناجح. لا يتغير ما بقيت الكتلة مخزنة. الكتلة المحذوفة بـD118 ثم المستوردة ثانية تأخذ رقمًا جديدًا.
- key(b) = (signed(b) ? 0 : 1, importSeq(b), blockHash(b))، والأصغر معجميًا أفضل.

الترتيب t1 ≻ t2 يتحقق في إحدى حالتين:
- W(t1) > W(t2).
- أو تساوى W، وkey(c1) < key(c2)، حيث c1 وc2 ابنا السلف المشترك الأدنى على مساري t1 وt2.

لِمّة الترتيب الكلي:
- work(t) ≥ 1 لكل هدف، فلا يكون مسار مرشح بادئة لآخر بالعمل نفسه. إذن الابنان c1 ≠ c2 موجودان دائمًا عند تساوي W.
- المقارنة معجمية لمسارات من genesis، والمفتاح كلي على الأشقاء، فـ≻ ترتيب كلي متعدٍ.
- الرأس = الأعظم بـ≻، مستقلًا عن ترتيب الفحص. لا مقارنة زوجية تسلسلية.

لِمّة الانتقال:
- W وkey ثابتان لكل كتلة مخزنة.
- المجموعة تنمو بالاستيراد فقط، وD118 لا يحذف القانونية.
- إذن يتغير الرأس عند الاستيراد فقط، ومرة واحدة على الأكثر لكل استيراد.

الأثر:
- الموقّع يسبق الاحتياطي عند أول اختلاف (D15).
- التعادل بين شقيقين من النوع نفسه محلي بأسبقية الاستيراد، وغير قابل للطحن.
- القاعدة لا تغير صلاحية أي كتلة.

**10. التعافي**
- المعاملات المستبعدة تعود إلى المجمع بعد إعادة فحصها.
- التراجع بفروق عكسية لكل الحالة، ومنها pending[j] (D78).
- نافذة trie: K_eff ≤ K_hist، وما خارجها يعطي −32017.
- التراجع حتى R_keep، ثم TrieRebuild.
- الكتل الجانبية ≤ 2 لكل ارتفاع (D118):
  - يُحتفظ بالقانونية إن وُجدت، وبغير قانونية واحدة هي الأعلى عملًا تراكميًا لفرعها، ثم الأسبق رؤية.
  - عند وصول أخرى تُقارن بالمخزنة غير القانونية، وتُبقى الأعلى، ويُحذف الساقط مع نسله المخزن.
  - الفرع الذي يصير أثقل لاحقًا يُجلب بالمزامنة عبر Status.
  - القاعدة احتفاظ محلي لا صلاحية.

**11. التقدم (مشروط)**
الصادق يبدأ خلال X^W، وفي النمط المحدود p0 ≥ 1−exp(−c·ρ_hon).

**12. النهائية**
احتمالية: safe = head−6، وfinalized = head−60 [PDF:76-77]، دون ضمان رياضي.

**13. استقلال الإجماع عن RPC والإضافة (D87–D97)**
- التحقق والتعدين في runtime مستقل ذي أولوية، بكاتب redb واحد.
- قراءات RPC من لقطات تُحرر عند نهاية الحساب.
- العناصر التالية لا تدخل أي بند صلاحية:
  - RpcGuard وSendPacer وPacerTrace.
  - مرساة pocol_getLogs، وحدثا sendDone وreceiveDone.
  - BridgeAuth وغلاف رسائل الجسر ودلو معدله وعدّاد تعليقه وحصة SiteStorage، وحجب الأصل في العقدة.
- pocol_getLogs يقرأ السلسلة القانونية ولا يغيرها.
