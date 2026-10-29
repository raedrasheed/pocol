# العقود وتوافق EVM

**الفصل بين الإجماع والتنفيذ**

apply(S_parent, ctx, body, parentHeader) → (S, receipts, roots, B_old, settlements, blockTips).

ctx = {chainId=cfg.chainId، h، ts، baseFee، gasLimit، prevrandao، coinbase=Escrow، ancestors، closing، closeStart، gate2، final، parentResult}.
- parentResult (D141، D142) = ParentResult(p) كاملًا: {blockHash(p)، winner = owner(p.nonce)، shares[(n, owner(n))]}.
  - المصدر الوحيد: بايتات Header(p) المخزنة في HeaderStore بمفتاح blockHash(p) (nonce وshareList)، مع جدول تعيين p المحسوب على حالة الجد.
  - لا يُقرأ من ذاكرة التعدين، ولا من مجمع الحصص، ولا من نسخة غير مرتبطة بـparentHash.
  - يُحسب عند تحقق البندين 13 و14 للأب، ويُخزن في BlockMeta بمفتاح blockHash(p) قيمةً مشتقة.
  - عند إعادة التنفيذ (المزامنة، وTrieRebuild، وإعادة التنظيم) يُعاد اشتقاقه من Header(p) وحالة الجد ويُقارن بالمخزن. الاختلاف خطأ تنفيذ (parentResultMismatch) يوقف الاستيراد.
  - لا يعتمد على بايتات winnerSig: كل غلاف صالح يعطي المالك نفسه.
  - عند h=1 يكون فارغًا.
- لا يدخل ctx أي حقل من الكتلة h خارج UT والجسم: لا nonce، ولا مالكه، ولا shareList، ولا التوقيعان.

- الإجماع لا يرى EVM إلا عبر apply، والهدف لا يدخل ctx.
- GenesisBuilder(alloc, CP, M_0List) يبني حالة genesis ويقارن الجذر بـallocRoot، ويدعم حتى M_max (1024 في end/full).

**chainId**
- CHAINID = cfg.chainId.
- معاملات EIP-155 و2930 و1559 بـchainId آخر: −32015 في المجمع، وإدراجها يبطل الكتلة بالبند 17.
- legacy دون EIP-155 مرفوضة.

**متراكم tips (D68)**
- u256 في المضيف، يُصفّر عند الدخول.
- يُضاف gasUsed·effectiveTip بعد كل معاملة.
- يُمرر في المرحلة 2 بالصيغة selector‖be256(blockTips) بطول 36 بايتًا، ثم يُتلف.
- closeBlock وfinalize لا يستدعيهما إلا SYSTEM_ADDRESS.
- لا يُستخدم فرق الرصيد ولا SELFBALANCE ولا EIP-1153.

**العقود النظامية**
- MinerRegistry: 0x…C0C001.
- RewardEscrow: 0x…C0C002، مع pending[j] = keccak256(be256(j)‖be256(3))+k، حيث k ∈ 0..2 (D141):
  - k=0: amount.
  - k=1: kind (بايت) مع proposer.
  - k=2: winner، والصفر يعني غير مسجل. العنوان الصفري لا يكون هوية، فهو مرفوض بـgsSys، وmsg.sender لا يكون صفرًا.
- ShareLedger: 0x…C0C003.
- ChainCaps: 0x…C0C004.
- ChunkFactory: 0x…C0C005، بلا تخزين.
- PAY: 0x…C0C0FF، أصلي.

تخطيط MinerRegistry:
- الخانات 0–4: RecLen، وActLen، وsweepCur، وعدادات الكتلة، وheldTotal.
- RecArr[i] = keccak256(be256(5))+i.
- ActList[i] = keccak256(be256(6))+i.
- Rec[id].w_k = keccak256(pad32(id)‖be256(7))+k.

**الحماية (D25)**
- SYSTEM_ADDRESS = 0xff…fe.
- ≤ 30M gas لكل نداء، و≤ G_SYS لكل كتلة.
- revert في نداء نظام يبطل الكتلة.

**دوال المستخدم** (طول calldata / القيمة / أقصى كتابات):
- register(address): 36 / B_reg / 9.
- deregister(): 4 / — / 7.
- removeInactive(address): 36 / — / 7.
- reactivate(): 4 / — / 5.
- setRewardAddress(address): 36 / — / 1.
- claimBond(): 4 / — / 9.

**مراحل الكتلة**
- (0) applyEvidence، ثم markClose (≤ RET سجلًا في الإغلاق)، ثم pruneShares، ثم recordShares(h−1)، ثم recordResult(h−1)، ثم settle(h−D)، ثم beginBlock.
  - recordShares(h−1) (D142): نداء نظام يكتب بتخطيط ShareLedger القائم count[h−1][m] لكل مالك m: عدد عناصره في ctx.parentResult.shares، زائد 1 لـwinner (ν). ويحدّث lastShare = h−1 لكل مالك منها.
    - المالكون ≤ S_max+1، فالكتابات ضمن الحد المحسوب سابقًا.
  - recordResult(h−1) (D141): نداء نظام يكتب pending[h−1].winner = ctx.parentResult.winner.
  - يُتخطى عند h=1.
  - عند h ≥ 2 تكون pending[h−1] موجودة دائمًا، لأن h−1 ليست نهائية ما دام لها ابن.
  - يسري في الإغلاق، لأنه كتابة نظام لا كتابة مستخدم.
- (1) معاملات المستخدمين.
- (2) closeBlock(tips): يكتب pending[h] = {amount، kind، proposer} مع winner = 0، ولا يقرأ شيئًا من نتيجة الكتلة h. أو finalize(tips) عند F، ولا تُنشأ pending[F] (D141).
- (3) الجذور.

**حقول EVM**
- NUMBER=h، وTIMESTAMP=ts، وCOINBASE = Escrow.
- PREVRANDAO = SHA256('PoColRandao'‖seal_p)، وليست عشوائية آمنة.
- BLOCKHASH بالصيغة (h, n): (10,0) → genesis، و(257,0) → 0، و(257,1) → hash(1)، و(300,44) → hash(44)، و(h,h) → 0.
- BLOBHASH=0، وBLOBBASEFEE=1، دون EIP-4788.

**السقوف**
- CODE_CAP وB_code_max: يفشل الإيداع.
- SLOT_CAP، وACCT_CAP، وDIRTY_BLOCK_MAX=4096، وACCT_DIRTY_BLOCK_MAX=2048، وLOG_BLOCK_MAX=32 KiB، وLOG_CAP=1 GiB: توقف استثنائي.
- TXBYTES_CAP وTX_CAP: الكتلة باطلة، والبنّاء يتخطى المعاملة.
- LOG_BLOCK_MAX مع حد عدد السجلات المشتق في LG2 (546) أساس لِمّتي P1 وP3 (D89).

**موارد النظام (D50، D54، D70)**
- سياق المستخدم: 256 مفتاحًا و16 حسابًا، وصفر في الإغلاق، وإلا SysWriteClosing.
- المشتقات: KN=1752، وKC=1482، وKF=1395، وAN=404، وAC=388، وAF=257.
- gas مقدر (يقيسه T14): نحو 26M للعادية، ونحو 37M للنهائية.
- (D141) المفاتيح المتمايزة لكل كتلة لا تتغير بنقل كتابة winner:
  - الكتلة العادية: كلمتان لـpending[h]، وكلمة لـpending[h−1]، بدل ثلاث كلمات لـpending[h].
  - الكتلة F: مفتاح winner لـpending[F−1] ضمن ما يحذفه finalize أصلًا.
  - إذن KN وKC وKF كما هي. يعيد ParamGate.derive فحصها في M4، ويقيس T14 الكلفة.

**الحسابات والمعاملات**
- secp256k1 مع EIP-155. المقبول: legacy-155 و2930 و1559. النوعان 3 و4 يعطيان −32015.
- GAS_LIMIT=8M بهدف 4M، وbaseFee بصيغة London. في الكتل الفارغة يستقر دون 8 wei، ويُقرأ من الرأس.

**RPC**
- hash = blockHash، وminer = الفائز، وnonce = be64، وdifficulty = work(target).
- eth_call وeth_estimateGas ثقيلتان بحد 30M gas، على لقطة في خيط حساب بـSCRATCH ≤ 16 MiB ومهلة 5s (D88).
- كلاهما محاكاة: لا تلتزمان حالة، ولا تكتبان في المجمع، ولا تبثان شيئًا. هذا أساس إدراجهما في مصفوفة القراءة (D95)، مع رفض state override وإلزام to.
- ذاكرة EVM عند 30M gas ≤ نحو 3.9 MiB (3n+n²/512). ذاكرة الحالة المقروءة في العداد نفسه.

**الاختبار**
- GeneralStateTests لـCancun عدا الاستثناءات المعلنة، ثم T2b وT2c.
- CI1: CHAINID=777902.
- CI2: chainId=777001 يعطي −32015.
- CI3: الكتلة الحاملة لمعاملة CI2 تُرفض بالبند 17.
- GB1: heldTotal = 4·B_reg، وActList = M_0 بترتيبها، وRecLen = 4.
- GB-END (D86): RecLen = ActLen = 1024، وheldTotal = 1024·B_reg.

**الفحص الآلي (M4)**
onlyNotClosing، وPAY لكل نقل، وChunkFactory بلا SSTORE، وCALLER مصدر id، وtips من calldata.

**المكونات المعاد استخدامها**
- revm مع PAY، وalloy، وk256، وsha2/sha3، وredb، وrust-libp2p.
- hyper لـHTTP/1.1، بدل jsonrpsee، لأن D88 وD90 وD92 وD94 تحتاج تحكمًا في التدفق والحجز والتسلسل وساعة الإرسال ومخزن المقبس وحدث sendDone.
- tokio (runtimeان) مع مجمع خيوط حساب ثابت، وsocket2 لـSO_SNDBUF.
- ruint أو crypto-bigint لعرض 512 بتًا ثابت (D83)، ولا num-bigint في ASERT.
- في الإضافة: محلل JSON صارم لـBridgeAuth، ويُفضل التنفيذ اليدوي فوق JSON.parse مع فحص الأنواع لا مكتبة مخططات عامة. وdependency-cruiser (MIT) لفحص الاستيراد.
- Foundry وsolc أدوات بناء فقط، ولا كود من geth. التراخيص تُفحص في M0.

**المحافظ**
MetaMask عبر RPC مخصص (T15)، والمحفظة المدمجة (D14). المحفظة المدمجة هي المصدر الوحيد لـeth_sendRawTransaction من الإضافة (D95).

**الانحرافات المعلنة**
PAY لا يخطر المستلم، وtips الكتلة F تُحرق، والنقل القسري يُقفل.

توافق EVM لا يمنح أمان إجماع Ethereum.
