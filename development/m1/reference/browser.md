# المتصفح والعزل

**العنوان**
- omnibox: 'pocol <profileName>/<0xaddr>[/path][@v<n>]'.
- web3:// إدخال مريح فقط. ERC-4804 وERC-6860 غير مدعومين في v1، ولم يُقرأ نصاهما.
- لا ندعي دعم Chrome الأصلي لأي مخطط.

**ملف الشبكة (D58، D77، D79)**
{profileId، name، chainId، genesisPre، genesisHash، forkSchedule، endpoint، trustLevel ∈ {DEV, LN, RP}}.

القبول:
1. فك genesisPre دون خطأ gs*.
2. keccak256(genesisPre) = genesisHash.
3. تطابق chainId.
4. networks.json موقّع، أو تأكيد يعرض الهاش كاملًا.
5. RecvFit (D100): يُحسب WorstLegit لكل طريقة exact تعتمد على CP، من قيم الملف نفسه: B_code_max، وBODY_MAX، وTX_MAX، وطول genesisPre. أي تجاوز لـRECV_LIMIT يرفض الملف بـ−32019 {rule: recvFit}، بدل قبول شبكة تُرفض ردودها الصادقة لاحقًا.

netKey = keccak256(RLP(['PoCol-net-v1', chainId, genesisHash])).
- الفحص عند الفتح، وكل 30s، وعند التوقيع. أي اختلاف يهدم الإطار ويلغي الطلبات بـ4901.
- chainId مكرر بـgenesis مختلف: قراءة فقط بوسم «متعارض»، و4100 للتوقيع.

**نافذة رؤوس RP (D77، D80–D84)**

الأعداد (D82):
- W = [b, h]، حيث b = max(1, h−12)، وn = h−b+1 ≤ 13.
- المرجع b−1 عند b > 1، ولا يُعد.
- المسترجع: from = max(1, b−1)، وcount = h−from+1 ≤ 14، وهو خفيف (count ≤ 64).

ثوابت العميل (D84):
- VIEW_SLACK_S = 4.
- ceilTarget = min(2^256−1, target_g·2^4).
- minWorkRP = floor(2^256/(ceilTarget+1)).
- C ساعة المتصفح، وΦ_view = 10s.

الخطوات:
0. h = eth_blockNumber.
   - عند h=0: لا pocol_getHeaders ولا إطار، وviewNoBlocks برسالة «الشبكة لم تنتج الكتلة 1 بعد؛ لا رأس للتحقق في RP».
   - eth_requestAccounts وeth_sendTransaction يعطيان 4901، وإعادة الفحص كل 30s.
1. احسب b وfrom وcount، ولا تخالف D81.
   - −32018 على طلب صحيح يعطي viewIncomplete.
   - −32021 يعطي إعادة بعد retryAfterMs حتى 3 مرات، ثم viewIncomplete.
2. الرد count رأسًا بارتفاعات from..h متتالية، والنقص أو الزيادة viewIncomplete.
3. المرجع (b > 1): البند 1، ثم 1ب، ثم viewFuture (ref.ts ≤ C+Φ_view). لا عمل ولا نافذة.
4. لكل x ∈ W تصاعديًا:
   - البند 1، ثم 1ب، ثم viewFuture.
   - البند 2: الأب هو المرجع، أو رأس genesis الافتراضي عند b=1، ويجب parentHash(1) = cfg.genesisHash وإلا viewGenesis.
   - البندان 3 و4.
   - البند 5 (ASERT بعرض ثابت).
   - viewTargetCeil: target_x ≤ ceilTarget، قبل PoW.
   - البنود 6–9.
5. الرسالة: «RP: مرتبط بالعمل والهوية لـn رأسًا [حتى genesis]، بعمل محتسب لا يقل عن <minWorkRP عشريًا> لكل رأس (work(ceilTarget))، دون تحقق تنفيذ أو عضوية».
   - عند minWorkRP = 1 تضاف «حد العمل غير فعال لهذه الشبكة».
   - «حتى genesis» عند b=1 فقط.
   - لا نسبة ولا تقدير زمني في الرسالة.
6. أي مخالفة: وسم، ورفض العرض، و4901، وعدم الاعتماد على أي جذر.
7. البنود 10–17 غير قابلة للتقييم، والكذب المتسق ممكن.

كلفة العميل: ≤ 14 فك رأس، و≤ 13 ASERT بوسائط ≤ 512 بتًا، و≤ 13 SHA-256.

أثر D84: شبكة صادقة متأخرة بأكثر من 4τ تعرض viewTargetCeil، أي 4 ساعات في network و40 دقيقة في P11. الحل LN.

أمثلة RW:
- h=3 → b=1، وfrom=1، وcount=3، وn=3، حتى genesis.
- h=13 → b=1، وfrom=1، وcount=13، وn=13.
- h=14 → b=2، وfrom=1، وcount=14، وn=13.
- h=20 → b=8، وfrom=7، وcount=14، وn=13.

أمثلة HC:
- target_g=2^240 → ceilTarget=2^244 → minWorkRP=4095.
- target_g=2^256−1 → minWorkRP=1.

LN عند h=0: العرض والتوقيع مسموحان، لأن العقدة تحققت من GenesisSpec وallocRoot.

**LogClient (D89، D91)**

المسار الوحيد لـeth_getLogs من الصفحة:
- BridgeAuth يتحقق من مخطط المرشح (D95).
- الجسر يحل الوسوم إلى أرقام.
- يرفض المدى > 1024 قبل الإرسال بـ−32020 {reason: range}.
- يمرر إلى LogClient.fetchAll، ولا يُرسل eth_getLogs خام.

تصنيف أسباب Limit:
- P1 = {resultBytes, resultCount, scanBytes}، بشرط a ≤ lc ≤ b−1.
- RES = {deadline, scratch}، بشرط a−1 ≤ lc ≤ b−1.
- أي مخالفة أو سبب آخر: serverViolation.

وسم withinLimit:
- [a, lc] الناتج عن P1 يُوسم، ويرثه كل مدى فرعي.
- P1 على مدى موسوم يعطي serverViolation (لِمّة P3)، أما RES عليه فمسموح.

LogClient.step(range{a, b, withinLimit, retries}, reply):
- Complete(logs): كل blockNumber ∈ [a, b]، وblockHash ثابت لكل ارتفاع، و(blockNumber، logIndex) تصاعدي بصرامة يلي آخر ملحق. ثم Append، والمخالفة serverViolation.
- Limit{r ∈ P1, lc}: Replace([a, lc]+withinLimit ثم [lc+1, b] بوسم الأصل). لا قفز قبل اكتمال [a, lc].
- Limit{r ∈ RES, lc}:
  - إن كان lc ≥ a: Replace([a, lc] ثم [lc+1, b]) بوسم الأصل.
  - وإلا، إن كان b > a: Replace([a, mid] ثم [mid+1, b])، حيث mid = a+floor((b−a)/2).
  - وإلا (a=b): Retry(500 ms) مع retries+1، وعند retries = 3 يعطي Error{r, block: a}.
- −32021: Retry(retryAfterMs) دون احتساب في retries، ويُحتسب في totalRequests.
- −32022 (anchorNotCanonical أو beyondAnchor): Restart.
- −32601: Error{unsupported}.
- أي خطأ آخر: Error صريح.

عدّ الطلبات:
- logRequests: طلبات pocol_getLogs في المحاولة، شاملة الإعادات.
- headRequests: طلبات eth_getBlockByNumber في المحاولة (المرساة وفحص ما قبل الاعتماد).
- totalRequests: كل طلبات RPC في كل محاولات التشغيل حتى لحظته.
- maxRequests يقيد totalRequests وحده، ويُفحص قبل الإرسال. الطلب الذي يجعله > maxRequests لا يُرسل، فيعطي abort(id, tooManyRequests) ثم Error{tooManyRequests}.

الإنهاء:
- Replace يستبدل مدى بمديين أقصر بصرامة.
- Retry ≤ 3 لكل كتلة، وRestart ≤ maxRestarts.
- maxRequests سقف صريح.
- خطوات المحاولة ≤ 5·(b−a+1) زائد إعادات −32021، والكل محدود بـmaxRequests.

لِمّة الصحة: مديات المحاولة الواحدة المكتملة تقسيم منفصل مرتب لـ[from, to]، وكل قطعة كاملة على فرع المرساة (P2). الدمج = نتيجة المدى لذلك الفرع.

عقد sink = {begin(attemptId, anchor), piece(attemptId, seq, range, logs), abort(attemptId, reason), commit(attemptId, summary)}:
- summary = {pieces, logs, logRequests, headRequests, totalRequests, anchor}، وtotalRequests يشمل المجهضة.
- attemptId يبدأ من 1 ويزيد بواحد.
- anchor = {hash, number} من eth_getBlockByNumber('latest') عند begin. إن كان to > anchor.number: abort(id, beyondHead) ثم Error.
- seq من 0 لكل محاولة، والقطع بترتيب المديات، وقد تكون فارغة.
- قبل commit: eth_getBlockByNumber(anchor.number).hash = anchor.hash، وعدم التطابق يعامل كـRestart.
- Restart: abort(id, reorg) ثم begin(id+1, anchor جديدة). maxRestarts = 3، أي ≤ 4 محاولات، ثم abort(id, reorgUnstable) وError.

الثوابت (يفحصها SinkChecker):
- (S-a) لكل محاولة: begin واحد، ثم ≥ 0 piece، ثم حدث نهائي واحد بالضبط (abort أو commit).
- (S-b) لا استدعاء بعد الحدث النهائي، ولا محاولة قبل انتهاء سابقتها.
- (S-c) commit مرة واحدة على الأكثر، وللمحاولة الأخيرة فقط.
- (S-d) مديات المحاولة المعتمدة تغطي [from, to] تمامًا.
- (S-e) pieces وlogs تطابق ما سُلم، وlogRequests وheadRequests تطابق سجل MockLogServer للمحاولة، وtotalRequests يطابق كل طلبات التشغيل.

دلالة المستهلك: بيانات piece مؤقتة حتى commit. عند abort تُسقط كل بيانات attemptId. الناتج صحيح لفرع anchor المعلن، وreorg بعد فحص ما قبل الاعتماد لا يُكشف.

الجامعان:
- fetchAll (الجسر): sink داخلي يفرغ عند abort.
  - maxBytes = 4 MiB لكل محاولة، ومهلة 30s، وmaxRequests = 4096.
  - التجاوز tooLarge أو timeout أو tooManyRequests دون نتيجة جزئية.
  - الصفحة تستلم بعد commit فقط.
- fetchEach (أداة الاختبار، Node.js): sink خارجي، ومهلة 600s، وmaxRequests = 65536، ولا يتاح للصفحات.

في RP: −32601 يعطي unsupported، والكذب المتسق ممكن (D13).

**الجسر وتفويضه (D95، D96، D97)**

الغلاف الموحد (D96):
- الرسالة نص JSON = {id, kind, payload} بهذه الحقول الثلاثة فقط.
- id عدد صحيح في [0, 2^32−1] يولده المحمّل، وهو مفتاح الرد فقط.
- kind ∈ {rpc_read, wallet_req, storage_set, nav}.
- payload = {method, params} بهذين الحقلين فقط، وparams مصفوفة دائمًا لكل kind، ولو فارغة. لا شكل بديل لأي kind.
- الرد: {id, result} أو {id, error: {code, message, data?}}. id = null حين لا يمكن استخراج id صالح.
- لا نتيجة جزئية بحالة نجاح.

القاعدة العامة:
- الموقع خصم (A7b). كل رسالة تمر بـBridgeAuth.check في عامل الإضافة قبل أي اتصال شبكي أو أثر محلي، والرفض افتراضي لكل ما ليس في المصفوفة.
- مصفوفة السماح Map مجمدة تُبنى عند تحميل الوحدة، لا كائن JS عادي، فلا تطابق لـ__proto__ أو constructor أو hasOwnProperty.
- التطابق بايتي حرفي لاسم الطريقة: حساس لحالة الأحرف، بلا مسافات أو محارف تحكم، و≤ 64 بايت ASCII.
- جدول خفيف/ثقيل في RpcGuard ليس جدول صلاحيات.

ختم الوصول (D97):
- العامل يختم كل رسالة لحظة استلامها من المنفذ بـarrivalMs = floor(performance.now()) في العامل، قبل أي فحص. الموقع لا يتحكم فيه.
- الرسائل تُعالج واحدة واحدة بترتيب الاستلام.

ترتيب الفحص (يتوقف عند أول خرق، وهو نفسه لكل kind):
- Bpre (دون تحليل):
  - الحجم > BRIDGE_MSG_MAX = 64 KiB: −32600 {reason: size} مع id = null، دون استهلاك رمز.
  - دلو المعدل لكل جلسة موقع (D97، D99)، حالته {tokens_mt، last_ms} بأعداد صحيحة:
    - عند إنشاء الجلسة (لا الإطار): tokens_mt = BUCKET_CAP_MT = 50000، وlast_ms = لحظة الإنشاء. التنقل لا يعيد ملء الدلو.
    - لكل رسالة اجتازت الحجم: tokens_mt = min(50000, tokens_mt + 50·(arrivalMs − last_ms))، ثم last_ms = arrivalMs.
    - إن كان tokens_mt ≥ 1000: يُخصم 1000 وتُتابع الرسالة. وإلا: −32005 {reason: rate} مع id = null، دون خصم.
    - الرسالة التي تجتاز فحص المعدل تستهلك رمزها ولو رُفضت لاحقًا في B0–B3.
- B0 البنية: parseStrict يرفض JSON غير الصالح، والمفاتيح المكررة في أي مستوى، والعمق > 8. ثم: الحقول الخارجية تمامًا {id, kind, payload}، وid صحيح ضمن المدى، وkind نص، وpayload كائن بالحقلين {method, params} تمامًا، وmethod نص، وparams مصفوفة. أي مخالفة: −32600، ويُعاد id إن كان صالحًا وإلا null.
- B1: kind ∈ المجموعة، وإلا −32600 {reason: kind}.
- B2: (kind, method) في المصفوفة، وإلا 4200 {method}، حيث method مقصوص إلى أول 64 وحدة UTF-16.
- B3: params تطابق مخطط الطريقة بالضبط في عدد العناصر والأنواع والحقول المسموحة والحدود، وإلا −32602 {path}. path بصيغة 'params' أو 'params[i]' أو 'params[i].field'.
- B4 التوجيه، ثم قيد الحالة الذي يطبقه المكوّن الموجّه إليه (D97)، لا BridgeAuth:
  - rpc_read إلى RpcReadClient، عدا eth_getLogs إلى LogClient.fetchAll. كلاهما يمر بعدّاد التعليق للجلسة:
    - إن كان pendingReads = 4، أو بلغ العدّاد العام 16: −32005 {reason: pending} دون اتصال.
    - ثم RecvGuard.reserve (D98). إن تعذر الحجز خلال 2s: −32005 {reason: busy}.
  - wallet_req إلى Wallet، بقيوده الخاصة (طلب معلق واحد، و≤ 5 في الدقيقة).
  - storage_set إلى StoreQueue (D101). الامتلاء يعطي −32005 {reason: store} دون إلحاق. بعد القبول ينفذ SiteStorage الرسالة بترتيبها، وتجاوز الحصة يعطي 4300 {reason: quota, limit: 1048576} وفق المحاسبة أدناه.
  - nav إلى Navigator، والتأكيد المعلق الثاني −32005 {reason: pending}.

عدّاد التعليق pendingReads (D97):
- لكل جلسة موقع (D99)، ويبدأ 0 عند إنشاء الجلسة.
- يزيد بواحد لحظة قبول RpcReadClient أو LogClient للرسالة وقبل أول اتصال.
- ينقص بواحد لحظة إرسال الرد النهائي للإطار: نجاح، أو خطأ بعيد، أو خطأ ناقل أو مهلة (10s لـRpcReadClient، و30s لـfetchAll) يُرد بـ−32603 {reason: transport}.
- هدم الإطار لا يصفّره. طلبات الإطار المهدوم تصير يتيمة:
  - تُلغى بـAbortController دون رد.
  - ينقص العدّاد بكل طلب منها عند استقراره فقط، أي رفض وعد الناقل وتحرير مخزن الاستقبال وحجزه.
  - أقصى زمن للاستقرار هو مهلة الناقل نفسها.
- لا يُصفّر إلا بانتهاء الجلسة، بعد استقرار كل طلباتها.
- إعادات LogClient الداخلية لا تزيده، فرسالة eth_getLogs واحدة = تعليق واحد.

الأنواع الأولية:
- addr = '0x'+40 hex.
- hash32 وslot32 = '0x'+64 hex.
- qty = hex بلا أصفار بادئة ضمن u64.
- tag ∈ {'latest', 'safe', 'finalized'} أو qty. pending وearliest وكائن EIP-1898 مرفوضة.
- data = '0x'+hex زوجي ≤ 64 KiB.
- str(n) = نص لا يحوي وحدات بديلة منفردة (lone surrogates)، وطول ترميزه UTF-8 بين 1 وn بايت.
- pathStr = نص ASCII مطبوع (0x21–0x7e) طوله 1..2048، أوله '/'.
- httpsUrl = نص ASCII مطبوع طوله ≤ 2048، يبدأ حرفيًا بـ'https://'، ويُحلل بـURL إلى protocol = 'https:' وhostname غير فارغ وusername وpassword فارغين.

مصفوفة rpc_read (الطريقة: المخطط):
- eth_chainId، وnet_version، وweb3_clientVersion، وeth_blockNumber، وeth_gasPrice، وeth_maxPriorityFeePerGas: [].
- eth_getBalance، وeth_getCode، وeth_getTransactionCount: [addr, tag].
- eth_getStorageAt: [addr, slot32, tag].
- eth_getBlockByNumber: [tag, false]. eth_getBlockByHash: [hash32, false]. الكتلة الكاملة (true) مرفوضة.
- eth_getTransactionByHash، وeth_getTransactionReceipt: [hash32].
- eth_call: [callObj, tag]. eth_estimateGas: [callObj] أو [callObj, tag]. معلمة ثالثة (state override) مرفوضة.
  - callObj حقول من {from, to, gas, gasPrice, maxFeePerGas, maxPriorityFeePerGas, value, data, input} فقط.
  - to إلزامي، فلا محاكاة نشر.
  - gas ≤ 30000000، وdata أو input لا كلاهما.
  - كلاهما محاكاة على لقطة بلا التزام حالة ولا بث.
- eth_feeHistory: [qty ≤ 128, tag, percentiles]، حيث percentiles ≤ 16 عددًا تصاعديًا في [0, 100]. هذا حد منتجي أضيق من حد العقدة (≤ 1024) حتى يبقى الرد ضمن RECV_LIMIT المشتق (D100).
- eth_getLogs: [filter]، والمرشح أحد شكلين:
  - {fromBlock, toBlock, address?, topics?}، حيث address addr أو ≤ 16 منها، وtopics ≤ 4 مواضع كل منها null أو hash32 أو ≤ 16 منها.
  - {blockHash, address?, topics?}.

مصفوفة wallet_req:
- eth_accounts وeth_requestAccounts وeth_chainId: [].
- eth_sendTransaction: [txObj]، حيث txObj حقول من {from, to, gas, value, data, maxFeePerGas, maxPriorityFeePerGas, nonce}. from هو الحساب الممنوح لـ(netKey, siteAddress)، وإلا 4100.

مصفوفة storage_set (D96، محلية، بلا اتصال شبكي):
- site_storageSet: [str(256), str(61440) أو null]. القيمة null تعني الحذف.
- site_storageClear: [].
- النتيجة null عند النجاح.

مصفوفة nav (D96، محلية، بلا اتصال شبكي):
- site_navigate: [pathStr]. العارض يطبق تطبيع storage، ثم يهدم الإطار ويبني إطارًا جديدًا بالمسار وفق D26. المسار غير الموجود أو المرفوض بالتطبيع يعرض صفحة 404 للعارض. لا رد للإطار المهدوم. الإطار الجديد ينضم إلى جلسة الموقع نفسها بدلوها وعدّاديها (D99).

دلو التنقل للجلسة، ويُخصم منه قبل الهدم:
- NAV_CAP_MT = 6000، وNAV_REFILL_MT_PER_MS = 1، وNAV_COST_MT = 2000. النتيجة ثلاثة تنقلات متتالية، ثم تنقل كل 2s.
- العجز يعطي −32005 {reason: 'nav'} دون هدم.
- site_openExternal: [httpsUrl]. يعرض تأكيدًا بالرابط كاملًا. النقر على «فتح» يفتح تبويبًا جديدًا دون opener ويرد null، والرفض أو الإغلاق يعطي 4001. تأكيد معلق ثانٍ من الإطار نفسه: −32005 {reason: pending}.

البادئة site_ محجوزة للطرق المحلية، ولا تظهر في مصفوفة rpc_read، فلا تصل أي طريقة site_ إلى RpcReadClient أو إلى عقدة.

المحظورات:
- القاعدة هي الرفض الافتراضي، والقائمة التالية أمثلة تُختبر صراحة.
- محظور من كل kind: eth_sendRawTransaction، وeth_sign، وpersonal_*، وeth_signTypedData*، وwallet_*، وeth_subscribe، وeth_newFilter وأخواتها، وdebug_*، وtxpool_*، وadmin_*، وanvil_*، وevm_*، وeth_getProof، وكل pocol_* (ومنها pocol_submitShare وpocol_getTemplate).
- eth_sendTransaction خارج wallet_req محظورة، وطرق القراءة داخل wallet_req محظورة، وطرق site_* خارج صنفها محظورة، وطرق RPC داخل storage_set وnav محظورة.

مسار الكتابة الوحيد على الشبكة:
1. wallet_req/eth_sendTransaction.
2. موافقة مربوطة بالشبكة والبايتات (D14، D26).
3. Wallet يوقّع البايتات المعروضة.
4. WalletSubmit يرسل eth_sendRawTransaction بتلك البايتات فقط، بعد التحقق من keccak256(raw) = txHash المعتمد.

WalletSubmit وحده يملك ناقل الإرسال الخام.

العزل البرمجي:
- RpcReadClient هو الناقل الشبكي الوحيد المتاح لـBridge. يعيد فحص الاسم مقابل مصفوفة القراءة نفسها كدفاع ثانٍ، ويرفض غيرها بـ4200 دون اتصال.
- الطرق الداخلية تمر عبر HeaderNetCheck وLogClient وNetworkProfiles، لا عبر Bridge، ولا تستهلك دلو الإطار ولا تعليقه. منها pocol_getHeaders، وpocol_getParams، وpocol_getLogs، وeth_getBlockByNumber للمرساة، وeth_getCode لجلب القطع.
- قاعدة dependency-cruiser في CI: وحدة Bridge لا تستورد WalletSubmit ولا HttpTransport الخام.

BridgeTrace (D97، بناء الاختبار وتفعيل صريح فقط):
- سطر لكل رسالة: {frame, seq, id|null, arrivalMs, tokensBefore_mt, bpre ∈ {pass, size, rate}, stage ∈ {B0, B1, B2, B3, B4, ok}, code|null, route|null, pendingBefore|null, storeTotalBefore|null, storeTotalAfter|null, forwardedSeq|null, replyMs|null}.
- storeTotalBefore وstoreTotalAfter لرسائل storage_set وحدها، بالمحاسبة أدناه.
- لا يغير القرارات، ومخزنه ثابت 1 MiB، والفيض يجعل التشغيل غير حاسم.

الأخطاء:
- تُعاد للإطار بأشكال ثابتة: 4200 {method}، و−32600 {reason?}، و−32602 {path}، و−32005 {reason}، و4300 {reason, limit}، و−32603 {reason ∈ {transport, recvLimit, recvDepth, recvParse, storeTimeout}}، و4001، و4100، و4901. أسباب −32005 هي {rate, pending, nav, busy, store}.
- 4300 و−32005 رمزان محليان للجسر. −32005 مأخوذ من EIP-1474، و4300 غير قياسي ومعلن.
- نص الخطأ البعيد لا يُمرر إلا لطريقة مسموحة وصلت إلى الخادم.

في RP: المصفوفة نفسها، والفحص محلي قبل الإرسال إلى البعيد.

**استقبال الردود البعيدة: RecvGuard (D98)**

كل طلب شبكي من الإضافة يمر بـHttpTransport.request(method, body, {recvLimit, deadline, pool}). يشمل ذلك RpcReadClient وLogClient وHeaderNetCheck وNetworkProfiles وChunkFetcher وWalletSubmit. لا تُستدعى response.text() ولا response.json().

خطوات القراءة:
1. الحجز: يُحجز recvLimit من المجمع قبل الإرسال، بطابور FIFO. العجز خلال 2s يعطي busy.
2. إن كانت Content-Length > recvLimit يُلغى الطلب قبل قراءة أي بايت.
3. القراءة بقارئ تدفق على response.body:
   - البايتات تُعد بعد فك الضغط الذي يجريه Chrome، فقنبلة الضغط تتوقف بالعد نفسه.
   - كل قطعة تُفحص قبل إلحاقها. القطعة التي تجعل المجموع > recvLimit تُهمل، ويُلغى الطلب بـAbortController: recvLimit.
4. بعد النهاية:
   - TextDecoder بوضع fatal، وإلا recvParse.
   - ثم parseStrict بعمق ≤ 16 ورفض المفاتيح المكررة، وإلا recvDepth أو recvParse.
   - المحلل تكراري لا عودي، وكلفته خطية.
5. الرد الخاطئ (حالة HTTP غير 200 أو كائن error) يخضع للحدود نفسها:
   - message يُقص إلى 256 وحدة UTF-16.
   - data يُمرر فقط إن كان ترميزه ≤ 4 KiB، وإلا يُحذف.
6. أي إلغاء أو مهلة أو خطأ يهمل المخزن كله:
   - لا يصل محتوى جزئي إلى إطار أو sink أو عارض.
   - الحجز يُحرر عند الاستقرار.

RECV_LIMIT لكل طريقة (D100). لكل طريقة أحد صنفين:
- exact: RECV_LIMIT ≥ WorstLegit(method)، وهو أسوأ جسم HTTP مشروع من pocold، محسوبًا بمعلمات CP وبالمعلمات التي يرسلها العميل فعلًا (مخطط الجسر أو الاستدعاء الداخلي). هذا وعد توافق كامل في LN، ويفحصه BG1 بالملفات المعرّفة، وRecvFit لكل ملف.
- capped: حد منتجي أضيق من العقدة عمدًا، في قائمة مغلقة حرفيًا:
  - eth_call بحد 1 MiB، والعقدة تسمح حتى RESP_MAX = 8 MiB.
  - eth_estimateGas بحد 64 KiB. النجاح ≤ 4 KiB، أما خطأ revert ببيانات أكبر فيعطي recvLimit.
  - الرد المشروع الأكبر من الحد يعطي −32603 {recvLimit}، ويُسجل في T12.
  - إضافة طريقة إلى capped تتطلب سطرًا هنا ومراجعة.

WorstLegit بالبايت. الافتراضات: JSON مضغوط بلا مسافات، و69 B لكل كمية u256 نصية مع فاصلتها، و32 B لكل نسبة عشرية.
- 4096 [exact]: eth_chainId، وnet_version، وweb3_clientVersion، وeth_blockNumber، وeth_gasPrice، وeth_maxPriorityFeePerGas، وeth_getBalance، وeth_getStorageAt، وeth_getTransactionCount، وeth_sendRawTransaction. النتيجة ≤ 70 B، والغلاف ≤ 1024.
- 69632 (68 KiB) [exact]: eth_getCode. الأسوأ 2·B_code_max+1024 = 66560.
- 98304 (96 KiB) [exact]:
  - pocol_getHeaders بـcount ≤ 14: الأسوأ 14·(2·3067+16)+1024 = 87124.
  - pocol_getParams: الأسوأ 2·|genesisPre|+4096، فيلزم |genesisPre| ≤ 47104، ويفحصه RecvFit.
- 167936 (164 KiB) [exact]: eth_getBlockBy* بالهاشات. الأسوأ 2048+floor(BODY_MAX/85)·70 = 163958 في network. الحد السابق 160 KiB (163840) كان أقل منه بـ118 B.
- 180224 (176 KiB) [exact]: eth_feeHistory بمخطط الجسر n ≤ 128 وp ≤ 16. الأسوأ 325+2·(n+1)·69+2·n·32+n·(69p+3) = 168015، وحقلا blob محسوبان احتياطًا.
- 266240 (260 KiB) [exact]: eth_getTransactionByHash. الأسوأ 2·TX_MAX+2048 = 264192.
- 348160 (340 KiB) [exact]: eth_getTransactionReceipt. الأسوأ 2·32768+546·512+2048 = 347136.
- 1048576 [capped]: eth_call.
- 65536 [capped]: eth_estimateGas.
- 8 MiB [exact في LN]: pocol_getLogs، والأسوأ RESP_MAX. فوقه حد fetchAll التجميعي 4 MiB. في RP قد يتجاوز الحدَّ بعيدٌ لا يطبق D88، فيعطي Error{recvLimit} صريحًا.
- الطريقة غير المدرجة لا تُرسل.

في RP تُطبق الحدود نفسها، ووعد exact يخص pocold في LN وحده. الخادم غير المضغوط (بمسافات) قد يعطي recvLimit، وهذا معلن.

المجمعات (حدود عامة في العامل):
- SITE_POOL = 32 MiB لكل طلبات المواقع.
- SESSION_RECV_MAX = 10 MiB لكل جلسة.
- TRUST_POOL = 2 MiB مستقل لـHeaderNetCheck وNetworkProfiles، فلا يجوّعه موقع.
- CONTENT_POOL = 8·68 KiB لجلب القطع.
- RECV_INFLIGHT_MAX = 48 طلبًا لمجمعي المواقع والمحتوى معًا. مجمع الثقة خارج هذا العد ومحدود ببايتاته، فلا يجوّعه عدد الطلبات. عبر الجسر لا يتجاوز العدد 16+8، فهذا الحد دفاع ثانٍ يختبره MR10c مباشرة.

الأثر:
- الاستقبال المخزن ≤ مجموع المجمعات (نحو 34.6 MiB)، زائد قطعة تدفق لكل طلب.
- ذاكرة التحليل المؤقتة تُقاس في MR10.
- الطلب البطيء يحتجز حجزه حتى مهلته (10s أو 30s).

**جلسة الموقع: SiteSession (D99)**

الإنشاء والانتهاء:
- المفتاح (tabId, netKey, siteAddress).
- تُنشأ بفعل المستخدم فقط، عبر omnibox أو واجهة الإضافة.
- تنتهي بإغلاق التبويب، أو بانتقال المستخدم إلى عنوان أو شبكة أخرى.

ما تملكه الجلسة:
- دلو الرسائل، وpendingReads، ودلو التنقل.
- حجز SESSION_RECV_MAX.
- ChunkCache، وحصتها في StoreQueue (عداد رسائل وبايتات، D101).

الإطارات والتحميل:
- كل إطار ينشئه site_navigate أو إعادة التحميل ينضم إلى الجلسة نفسها، فلا يجدد أي حصة.
- LoadJob واحد لكل جلسة. التنقل يلغي LoadJob السابق، وطلبات قطعه اليتيمة تبقى في CONTENT_POOL حتى تستقر.
- روابط blob للتحميل السابق تُلغى عند الهدم.
- رسائل منفذ الإطار المهدوم تُهمل بعد إغلاقه، ولا تستهلك رموزًا.

ذاكرة القطع:
- ChunkCache بمفتاح عنوان القطعة وطولها. العنوان مشتق من keccak256 البيانات، فهو مفتاح محتوى.
- الحد 4 MiB+64 KiB لكل جلسة، و16 MiB عامًا بسياسة LRU على الجلسات غير النشطة.
- التنقل داخل إصدار مخزن لا يعيد جلب أي قطعة.

الحدود التراكمية لكل جلسة، مهما تكرر التنقل:
- ≤ 4 قراءات حية أو يتيمة.
- عدد الرسائل حتى الثانية t ≤ 50+50·t.
- عدد التنقلات حتى الثانية t ≤ 3+t/2.
- جلب كل قطعة مرة واحدة ما بقيت في الذاكرة.

معالجة الرسالة لا تنتظر الشبكة، والتحليل الكبير محدود بـRECV_LIMIT، فيبقى العامل مستجيبًا.

**أهلية التوقيع**
- كل تعديل على الملفات تحت ProfileMutex، وeligibilityEpoch يزيد معه.
- signEligible: الملف مطابق للمجمَّد، ولا تعارض، وnetKey غير ملغى، والاتصال قائم، والمحفظة غير مقفلة، وepoch مطابق، و(LN أو DEV أو RP مع h ≥ 1 ونافذة ناجحة).
- داخل القفل: الأهلية، ثم الهوية، ثم فحص ثانٍ، ثم التوقيع بالبايتات المعروضة، ثم الإرسال عبر WalletSubmit، ثم التحرير.
- المفتاح لكل netKey = HKDF-SHA256(seed، salt=netKey، info='PoCol-wallet-v1').

**manifest.json (MV3)**
- sandbox.pages = [sandbox.html].
- CSP: 'sandbox allow-scripts allow-forms; default-src 'none'; script-src 'sha256-<H_loader>' blob:; style-src blob: 'unsafe-inline'; img-src blob: data:; font-src blob:; media-src blob:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none''.
- الأذونات: storage، وunlimitedStorage (D103)، وomnibox، وdeclarativeNetRequest، وhost 127.0.0.1، وoptional_host للبعيد.

**التحميل (D26)**
1. viewer يثبت السياق ويفحص الهوية وحراسة DNR.
2. العامل يجلب ويتحقق.
3. iframe بـsandbox='allow-scripts allow-forms'.
4. MessageChannel يُسلم مرة واحدة، ويُربط الإطار بجلسة الموقع (D99) دون إنشاء دلو أو عدّاد جديدين.
5. المحمّل يطبق RtcLockdown، ويجمد، ويعقّم، وينفذ السكربتات من blob بالترتيب.
6. حدث load ثانٍ يهدم الإطار.

**التعقيم**
- يُحذف: on*، وjavascript:، وiframe/frame/object/embed/base/portal، وmeta http-equiv، وscript داخل svg، وprefetch/preconnect/dns-prefetch/prerender.
- يُعاد كتابة: src، وhref، وsrcset، وurl()، و@import.
- يُعترض: fetch وXHR النسبيان.

غير مدعوم: eval، وimport النسبي، وWorkers، وcookies، وIndexedDB، وWebRTC.

**العزل الشبكي**
- الطبقة 1: CSP.
- الطبقة 2: DNR جلسي لكل تبويب:
  - حجب {id 2·tabId، priority 1}.
  - سماح {id 2·tabId+1، priority 2، urlFilter '|chrome-extension://<ID>/'}.
  - التثبيت مغلق عند الفشل.
- الطبقة 3: RtcLockdown، وحجته غير مثبتة، وX8 بوابة.
- الروابط (D96): المحمّل يعترض النقر على الروابط. الرابط النسبي أو المطلق داخل الموقع يرسل nav/site_navigate بالمسار، والرابط https الخارجي يرسل nav/site_openExternal. الروابط بمخططات أخرى لا تُتبع.

**التخزين المحلي للموقع (D34، D96)**
- مفتاح التخزين المنطقي هو 'site:'+netKey+':'+address، وحالته قاموس {key → value} محفوظ في سجلات نسخ غير قابلة للاستبدال (D103).
- المحاسبة الحرفية للحصة (SITE_STORE_MAX = 1048576 بايتًا):
  - entry(k, v) = utf8len(k) + utf8len(v)، بترميز UTF-8 للنصين كما وردا في params بعد فك JSON، دون علامات الاقتباس ولا بنية JSON ولا بادئة 'site:'.
  - total = Σ entry على كل الأزواج المخزنة للموقع.
  - site_storageSet(k, v) مع v نص: newTotal = total − (entry(k, old) إن وُجد k) + entry(k, v). القبول إن newTotal ≤ 1048576، فالمساواة مقبولة. وإلا 4300 {reason: 'quota', limit: 1048576}.
  - site_storageSet(k, null): حذف، newTotal = total − entry(k, old) أو total إن غاب k. لا يُرفض بالحصة أبدًا.
  - site_storageClear: newTotal = 0، ولا يُرفض بالحصة.
  - (D104) قيدان إضافيان:
    - الإضافة التي تجعل عدد الأزواج > SITE_ENTRIES_MAX = 4096 تُرفض بـ4300 {reason: 'entries', limit: 4096}.
    - كل كتابة، ومنها الحذف والمسح، تخضع لقبول القرص، فقد تُرفض بـ4300 {reason: 'disk'|'sites'} دون أي أثر.
- الذرية: SiteStorage يحسب newTotal، ثم يكتب الحالة كاملة بعملية chrome.storage.local.set واحدة لعنصر سجل جديد (D103). عند 4300 لا تُستدعى أي كتابة، فتبقى اللقطة بايتيًا كما كانت. فشل الكتابة نفسها يُرد بـ−32603 {reason: transport} ويُعاد تحميل اللقطة من التخزين.
- SiteStorage يسلسل الرسائل لكل مفتاح تخزين (netKey، عنوان) عبر StoreQueue، فلا تتداخل حسابات total.

**StoreQueue (D101)**

حجم الرسالة في الطابور:
- qbytes(msg) = 64 + utf8len(k) + utf8len(v)، وv = null تُحسب 0.
- site_storageClear تُحسب 64.
- أقصى رسالة = 64+256+61440 = 61760.

القبول عند B4، قبل الإلحاق ودون أي كتابة. تُقبل الرسالة إن تحققت الشروط الأربعة معًا:
- sessQ.n+1 ≤ STORE_SESSION_MSGS_MAX = 8.
- sessQ.bytes+qbytes ≤ STORE_SESSION_BYTES_MAX = 262144.
- globQ.n+1 ≤ STORE_GLOBAL_MSGS_MAX = 64.
- globQ.bytes+qbytes ≤ STORE_GLOBAL_BYTES_MAX = 4194304.

المساواة مقبولة. أي خرق يعطي −32005 {reason: store} فورًا. العدادات تشمل كل رسالة مقبولة لم تصل إلى ownState = released (انظر دورة الملكية أدناه)، ومنها النشطة التي رُد عليها بـstoreTimeout ولم تستقر بعد.

حقيقة حسابية (I55): بالقيم الافتراضية لا يمكن بلوغ الحد البايتي العام، لأن 64 رسالة قصوى = 64·61760 = 3952640 < 4194304. الحد العددي العام يحسم دائمًا قبله، فالحد البايتي العام دفاع زائد يحمي من تعديل لاحق للثوابت. يُختبر شرطه منفصلًا في BR21e بمعلمات اختبار.

الترتيب:
- FIFO لكل مفتاح تخزين بترتيب القبول (arrivalMs ثم seq)، حتى لو جاءت الرسائل من تبويبات متعددة للموقع نفسه.
- set وclear يُنفذان بالترتيب نفسه.
- لكل مفتاح كتابة نشطة واحدة على الأكثر.
- لكل الإضافة ≤ STORE_ACTIVE_GLOBAL = 2 كتابات نشطة. بقية المفاتيح تنتظر في FIFO عام.

التنفيذ:
- تقييم الرأس (D102، I56): حين تصير الرسالة أول رسالة غير محررة لمفتاحها ولا كتابة نشطة لذلك المفتاح، يُحسب newTotal على total المؤكد فورًا. لا يُنتظر مقعد عام، ولا يُستدعى Chrome.
  - التجاوز: الرد 4300 {reason: quota, limit: 1048576}، ثم release في الخطوة نفسها (queued → released). صفر استدعاء set، ولا مقعد ولا قفل محتجز. ثم يُقيَّم الرأس التالي للمفتاح فورًا.
  - القبول (ومنه الحذف والمسح دائمًا): ownState = ready. تنتظر الرسالة مقعدًا عامًا في FIFO عام بترتيب القبول (arrivalMs ثم seq).
- المقعد العام يُؤخذ لحظة بدء الكتابة فقط (ready → active)، فلا تحتجز رسالة مرفوضة بالحصة مقعدًا أبدًا.
- نتيجة التقييم تبقى صحيحة حتى البدء، لأن total المفتاح لا يتغير إلا بكتابات المفتاح نفسه، وهذه تقع خلف الرسالة في FIFO.
- STORE_QUEUE_WAIT يسري على queued وready معًا.
- كتابة واحدة بـchrome.storage.local.set للقاموس كاملًا. الذرية مفترضة لعنصر واحد، ويقيسها BR21c.
- النجاح يحدّث القاموس وtotal، ثم يُرد null.

المهل:
- STORE_QUEUE_WAIT = 10s من القبول حتى بدء الكتابة. انقضاؤها يعطي −32005 {store}، وتُزال الرسالة دون كتابة.
- STORE_WRITE_DEADLINE = 5s للكتابة النشطة. انقضاؤها يعطي −32603 {reason: storeTimeout}، أي نتيجة غير معروفة.
  - المفتاح يبقى محجوبًا، ومقعد الكتابة النشطة محتجزًا، حتى يستقر وعد Chrome.
  - عند الاستقرار يُعاد تحميل القاموس من التخزين ويُعاد حساب total.
  - لا تبدأ الكتابة التالية للمفتاح قبل ذلك، فلا تنعكس الأوامر.

دورة الملكية (D102، I54):
- لكل رسالة مقبولة حالتان منفصلتان:
  - حالة الرد: replyState ∈ {none, sent, suppressed}.
  - حالة الملكية: ownState ∈ {queued, ready, active, released}.
    - queued: لم تُقيَّم بعد.
    - ready: اجتازت الحصة وتنتظر مقعدًا.
- الرد النهائي يُرسل مرة واحدة على الأكثر، ولا يغير الملكية.
- الموارد المملوكة:
  - لكل رسالة: qbytes ورسالة واحدة في عداد الجلسة وفي العداد العام.
  - للنشطة أيضًا: مقعد الكتابة النشطة العام، وقفل مفتاح التخزين.
- release(msg) هو المكان الوحيد الذي يطرح العدادات ويحرر المقعد والقفل. يُنفذ مرة واحدة بالضبط لكل رسالة مقبولة. استدعاؤه مرة ثانية على الرسالة نفسها خطأ تنفيذ يكشفه BR21.
- الانتقالات:
  - queued → ready: تقييم الرأس بقبول.
  - ready → active: أخذ مقعد عام وبدء set، دون تحرير.
  - queued → released في ثلاث حالات:
    - تقييم الرأس برفض الحصة: الرد 4300، دون set ودون مقعد.
    - انقضاء STORE_QUEUE_WAIT عند acceptMs+10000: الرد −32005 {store}.
    - الإلغاء بالهدم أو بانتهاء الجلسة: suppressed، دون رد.
  - ready → released: انقضاء STORE_QUEUE_WAIT أو الإلغاء، بالردود نفسها.
  - active → released: عند استقرار وعد chrome.storage.local.set فقط، نجاحًا أو فشلًا، ولا شيء غيره.
- الاستقرار قبل المهلة: تحديث القاموس وtotal (أو إعادة التحميل عند الفشل)، ثم الرد (null أو −32603 {transport})، ثم release في الخطوة نفسها.
- انقضاء STORE_WRITE_DEADLINE: الرد −32603 {storeTimeout} ويصير replyState = sent. الملكية تبقى active بعداداتها ومقعدها وقفلها.
- الاستقرار بعد storeTimeout أو بعد هدم الإطار: إعادة تحميل القاموس من التخزين، وإعادة حساب total، ثم release، دون أي رد.
- هدم الإطار بعد رد storeTimeout لا يغير العدادات: الرسالة رُد عليها ولا تزال نشطة.
- الثابت: في كل لحظة يساوي عداد الجلسة والعداد العام عدد الرسائل غير المحررة ومجموع qbytes لها.
  - لذلك يستمر قبول رسائل جديدة أثناء احتجاز نشطة معلقة، لكن ضمن ما تبقى من الحدود فقط.
  - ورسائل المفتاح المحجوب تنتظر حتى الاستقرار أو حتى STORE_QUEUE_WAIT.

الهدم والتنقل (D99):
- رسائل الإطار المهدوم التي لم تبدأ تُلغى دون كتابة ودون رد، وتُحرر عداداتها فورًا.
- الكتابة النشطة تُكمل لأن كتابة Chrome لا تُلغى، وتُحتسب حتى استقرارها، وردها يُهمل.
- Navigator ينتظر استقرار الكتابة النشطة للمفتاح حتى STORE_WRITE_DEADLINE قبل أخذ snapshot للإطار الجديد. بعد المهلة يُحمّل الإطار بالحالة المؤكدة مع شارة «حالة الحفظ غير مؤكدة».
- انتهاء الجلسة يطبق القاعدة نفسها.

الذاكرة:
- الطوابير ≤ 4 MiB.
- الكتابات النشطة ≤ 2·(1048576+61760) قيمًا مع نسخة التسلسل، أي تقديرًا ≤ 5 MiB، وتُقاس في BR21c.
- قواميس المفاتيح المحملة: قاموس لكل مفتاح له جلسة حية، والبقية تُطرد بـLRU عند تجاوز STORE_DICT_CACHE_MAX = 8 MiB.
- المحمّل يبني localStorage من لقطة محلية عند التحميل. القراءة متزامنة من اللقطة.
- المحمّل يفرض الحصة محليًا بالمحاسبة نفسها: setItem الذي يجعل المجموع > 1048576 يرمي QuotaExceededError، فلا تتجاوز لقطته المحلية 1 MiB.
- الكتابة تُجمع كل 250ms في مجموعة مفاتيح متسخة تدمج القيمة الأخيرة لكل مفتاح، وclear() يفرغ المجموعة ويضع علامة مسح. ذاكرة الطابور المحلي إذن ≤ اللقطة المحلية.
- المحمّل يرسل storage_set فقط إن تحققت ثلاثة شروط:
  - المرسل ولم يُرد عليه < 8 رسائل.
  - المرسل ولم يُرد عليه + qbytes ≤ 256 KiB.
  - تقدير محلي بوجود رمز في الدلو.
- ما يُرد عليه بـrate أو store يعود متسخًا ما لم تُكتب له قيمة أحدث.
- عند اعتراض رابط داخلي يرسل المحمّل المفاتيح المتسخة، وينتظر ردودها حتى 2s، ثم يرسل site_navigate. الموقع الذي يستدعي site_navigate مباشرة يقبل فقد كتاباته المنتظرة (D101).
- رد 4300 على كتابة من المحمّل: يعيد المحمّل القيمة في لقطته المحلية إلى آخر قيمة مؤكدة، ويعرض شارة «لم يُحفظ».
- لا مشاركة بين عنوانين أو بين netKey مختلفين، ولا يصل موقع إلى مفتاح غيره، لأن SiteStorage يشتق المفتاح من سياق الإطار لا من الرسالة.

**تعافي جيل العامل (D103، I57)**

D101 وD102 تسريان داخل جيل عامل واحد. ما يلي يحكم الانتقال بين الأجيال.

البدائل:
- (أ) مالك مستمر: مستند offscreen أو منفذ يبقي العامل حيًا. لا يضمن MV3 بقاءهما، فيبقى السؤال نفسه عند زوالهما.
- (ب) إعادة قراءة القاموس عند الإقلاع وحدها: كتابة متأخرة من جيل ميت تمحو كتابة أحدث (سيناريو I57).
- (ج) معاملات IndexedDB: مصير المعاملة عند زوال السياق غير متحقق، وتضيف مخزنًا ثانيًا.
- (د) المختار: سجلات لا تُستبدل بنسخة (epoch, seq)، مع سياج جيل ونقطة تثبيت لكل مفتاح.

**1. السياج**
- الاسم (D134): عنصر السياج للعدد E اسمه 'pocol:epoch:'+hex16(E) وحده، دون nonce، وقيمته {bootMs: Date.now()} ≤ EPOCH_BYTES_MAX. كل جيل يحسب E نفسه يكتب الاسم نفسه.
- عند الإقلاع يسرد العامل عناصر 'pocol:epoch:*' بـgetKeys. EN عددها، وM أكبرها، وE = M+1. ثم بالترتيب: بوابة الأسماء، ثم نافذة الأجيال (القسم 6)، ثم الكتابة. لا set قبل اجتياز البوابتين.
- بوابة الأسماء (D134): الشرط EN + 1 ≤ EPOCH_NAMES_MAX = 8. عند الخرق:
  - لا إصدار، ويزيد epochGateBlocks.
  - ينتظر الجيل حتى مضي T_LATE على إقلاعه بساعته الرتيبة، ثم يكنس كما أدناه، ثم يعيد السرد والبوابة مرة واحدة.
  - إن استمر الخرق: storage_set يعطي −32603 {storeRecovery, reason: 'epochNames'} طوال الجيل، ولا snapshot، ويُرفع الإجراء 25.
- يكتب 'pocol:epoch:'+hex16(E) بقيمة {bootMs}، وينتظر نجاح الكتابة. إعادات STORE_EPOCH_RETRY بالاسم نفسه، فلا تضيف أسماء.
- لا كتابة بيانات ولا جلسة قبل التأكيد.
- الفشل يُعاد حتى STORE_EPOCH_RETRY = 3 مرات. بعدها يعطي storage_set الرد −32603 {reason: storeRecovery}، ولا snapshot.
- الكنس (D134): بعد مضي T_LATE على إقلاع الجيل الحي بساعته الرتيبة، يحذف كل عنصر epoch اسمه أصغر من أكبر عنصر ظاهر عند الكنس، أيًا كان bootMs المخزن.
  - الأكبر لا يُحذف أبدًا، فالحد الأعلى غير متناقص.
  - يجري الكنس في الجيل المؤكد وفي الجيل المحجوب بالبوابة.
  - فشل remove يبقي العنصر محتسبًا في EN.
- لِمّة الكتّاب: ليكن M_L أكبر عنصر ظاهر عند إقلاع الجيل الحي L. كل جيل يكتب اسمًا X ≤ M_L أقلع قبل L، لأن أي جيل لاحق يرى M_L فيختار عددًا أكبر. إذن خلف كل كاتب أقلع ≤ bootMs(L)، وكل عملياته تستقر قبل bootMs(L)+T_LATE (A15c). فالاسم المحذوف بالكنس لا يعود.
- لِمّة حد الأسماء:
  - كل اسم ≤ M ظهر من قبل، لأن كل كاتب لـX رأى X−1 ظاهرًا.
  - بلِمّة الكتّاب لا يعود اسم محذوف.
  - إذن الأسماء الحاضرة، أو التي قد تظهر في أي لحظة، محصورة في الظاهرة مع {M+1}.
  - البوابة عند آخر إصدار تحفظ EN+1 ≤ 8، ولا تزيد الأسماء بعدها إلا بإقلاع يجتاز البوابة.
  - فعناصر epoch ≤ 8 في كل لحظة، مهما تعددت الأجيال التي زالت قبل التأكيد.
- لِمّة السياج:
  - كل جيل كتب بيانات أكّد عنصره قبل ذلك، فيراه أي جيل لاحق، فيكون E اللاحق أكبر بصرامة.
  - الجيل الذي لم يؤكد عنصره لم يكتب بيانات.
  - من الأجيال التي تتشارك E (D134) يؤكد جيل واحد فقط: الثاني أقلع بعد زوال الأول، فإن كان الأول قد أكد رأى الثاني E واختار E+1.

**2. السجل**
- كل set يكتب عنصرًا جديدًا لا يُستبدل: 'site:'+netKey+':'+addr+':r:'+hex16(E)+':'+hex8(seq).
- قيمة العنصر {fmt: 2, epoch, seq, tomb, b64}. b64 هو base64 لتسلسل الأزواج [be32(len k)‖k‖be32(len v)‖v] بترتيب بايتي للمفاتيح (D104).
- (D104) يخصص seq مخصص واحد SeqAlloc لكل (مفتاح، جيل):
  - يبدأ من 0، وalloc() يعيد القيمة الحالية ويزيدها قبل إصدار أي set، سواء كانت نقطة تثبيت أو إعادة محاولتها أو كتابة عادية.
  - لا مخصص آخر، ولا يُشتق seq من رقم نقطة التثبيت.
- المدى: seq ∈ [0, 2^32−1]، وepoch ∈ [1, 2^64−1]. نفاد seq يجعل كتابات المفتاح في الجيل −32603 {storeRecovery} دون set. نفاد epoch يعطل التخزين المحلي.
- لا يُعاد استخدام seq ولو فشل الوعد أو انقضت مهلته.
- الحالة المؤكدة = dict في أكبر سجل (epoch, seq) موجود وصالح.
- بعد نجاح سجل تُحذف السجلات الأقل منه بأفضل جهد. ما فشل حذفه يبقى محتسبًا في القرص (D104).

**3. الاسترداد لكل مفتاح: RecoveryGate (D104)**
لكل مفتاح في الجيل حالة واحدة ∈ {unrecovered, recovering, ready, failed} وعملية استرداد مشتركة واحدة. أي snapshot أو storage_set، من أي تبويب، ينتظر العملية نفسها ولا يبدأ غيرها.

عند أول وصول (unrecovered → recovering) تبدأ مرحلة القراءة (D110، I63):
- t_s هي لحظة الانتقال. عندها يُنشأ gateId جديد للمفتاح، وتبدأ مهلة كلية RECOVERY_READ_DEADLINE = 5s. المهلة تشمل انتظار مقاعد القراءة، وgetKeys، وكل get، وفك السجلات وفحصها.
- كل نداء getKeys أو get يأخذ مقعدًا من ReadSlots (RECOVERY_READ_SLOTS = 2 لكل الإضافة) عبر ReadFIFO عام. لا يحرر المقعد إلا settle وعد القراءة نفسه. لا قراءة دون مقعد.
- يسرد العامل أسماء سجلات المفتاح من getKeys بالتصفية على البادئة، ثم يقرأ الأكبر وحده بـget. إن كان تالفًا يقرأ الذي يليه نزولًا، حتى RECOVERY_GETS_MAX = 4 قراءات get.
- يختار أكبر سجل صالح. السجل التالف يُتخطى ويُعد في recoveryCorrupt.
- غياب أي سجل للمفتاح يعني القاموس {}.
- وجود سجل تالف دون سجل صالح ضمن القراءات الأربع يجعل المفتاح failed، ورسائله تُرد بـ−32603 {storeRecovery, reason: corrupt}. لا إصدار ولا تخصيص seq. السجلات تبقى، ويمكن حذفها إداريًا.
- انقضاء المهلة قبل اكتمال القراءة يعيد المفتاح إلى unrecovered بسبب readTimeout:
  - رسائله المنتظرة تُرد بـ−32603 {storeRecovery, reason: readTimeout} وتُحرر.
  - لا إصدار، ولا تخصيص seq، ولا حجز قرص.
  - الوصول التالي يبدأ بوابة جديدة بـgateId جديد.
- كل نتيجة قراءة تُقبل فقط إن طابق gateId الحالي وكانت البوابة في مرحلة القراءة. غير ذلك تُهمل النتيجة، ويُحرر مقعدها فقط، ويزيد lateReadsDropped. لذلك لا تُصدر نقطة تثبيت بعد الحسم.
- القراءة لا تغير القرص، فالقراءة المعلقة لا تدخل LATE_RESERVE ولا LATE_NAMES. ذاكرتها محدودة: ≤ 2 قراءة معلقة، كل منها ≤ RECORD_BYTES_MAX أو قائمة أسماء ≤ STORE_RECORDS_MAX+LATE_NAMES تحت A15c.
- يطلب مقعدًا من RecoverySlots (D109)، لا من STORE_ACTIVE_GLOBAL، بانتظار ≤ RECOVERY_SLOT_WAIT = 5s من لحظة الطلب.
- عند الحصول على المقعد، وفي الخطوة المتزامنة نفسها: DiskLedger.reserve، ثم s_c = SeqAlloc.alloc()، ثم إصدار نقطة تثبيت (E, s_c) بالقاموس نفسه. مهلة STORE_RECOVERY_DEADLINE = 5s تبدأ لحظة الإصدار.
- رفض القرص: لا إصدار، ويُحرر المقعد في الخطوة نفسها، وتُعد المحاولة فاشلة دون عملية معلقة.
- تعذر المقعد للمحاولة الأولى خلال 5s: لا إصدار ولا تخصيص seq. يعود المفتاح إلى unrecovered، ورسائله المنتظرة تُرد بـ−32603 {storeRecovery, reason: busy} وتُحرر. الوصول التالي يبدأ بوابة جديدة.
- عند النجاح: ready، وconfirmed = (E, s_c). الكتابات اللاحقة تأخذ أرقامها من SeqAlloc نفسه.
- ثم يحذف ما دونها.

قبل نجاح نقطة التثبيت:
- رسائل المفتاح تبقى queued، محسوبة، وخاضعة لـSTORE_QUEUE_WAIT.
- لا snapshot.

عند فشل نقطة التثبيت أو انقضاء مهلتها:
- محاولة ثانية واحدة بـalloc() جديد وبالقاموس نفسه. تُطلب لحظة فشل settle الأولى أو انقضاء مهلتها، وتطلب مقعدًا جديدًا من RecoverySlots بانتظار ≤ 5s. المحاولة الأولى تبقى معلقة ومالكة مقعدها ومحتسبة في القرص حتى تستقر.
- تعذر مقعد المحاولة الثانية خلال 5s يُعامل كانقضاء مهلتها: المفتاح failed وفق D107، دون إصدار ودون تخصيص seq.
- إن صار المفتاح ready بـsettle ناجح للأولى أثناء انتظار مقعد الثانية، يُسحب طلب المقعد من RecoveryFIFO دون إصدار.
- حد الحسم (D110): مرحلة القراءة تنتهي أو تنقضي عند ≤ t_s+5s، فطلب مقعد المحاولة الأولى t0 ≤ t_s+5s. ثم مقعد ≤ t0+5s، ثم مهلة ≤ t0+10s، ثم مقعد الثانية ≤ t0+15s، ثم مهلتها ≤ t0+20s.
- إذن تُحسم البوابة خلال ≤ 25s من t_s. الحسم أحد أربعة: ready، أو failed، أو unrecovered بسبب busy، أو unrecovered بسبب readTimeout.
- رسائل المفتاح المنتظرة خلال الاسترداد تبقى خاضعة لـSTORE_QUEUE_WAIT = 10s، فقد تُرد بـstore قبل الحسم.

RecoverySlots (D109، I62):
- RECOVERY_SLOTS = 2 لكل الإضافة، منفصلة عن STORE_ACTIVE_GLOBAL وعن AdminSlots. حالة المقعد ∈ {free, held(op)}.
- الأخذ عبر RecoveryFIFO عام بترتيب الطلب، في الخطوة المتزامنة نفسها مع DiskLedger.reserve والإصدار.
- التحرير عند settle وعد نقطة التثبيت فقط، نجاحًا أو فشلًا، أو عند رفض القرص قبل الإصدار. لا تحرره المهلة ولا الحسم. زوال الجيل يسقط الحالة كلها.
- لِمّة RecoverySlots: كل نقطة تثبيت صادرة تملك مقعدًا من الإصدار حتى settle، ولا تُصدر دونه. إذن نقاط التثبيت غير المستقرة في الجيل ≤ 2 في كل لحظة، ومنها لحظة زواله، مهما تعددت المفاتيح والمحاولات.
- لِمّة الحسم: مرحلة القراءة ≤ 5s بمهلة كلية، وكل انتظار مقعد تثبيت ≤ 5s، وكل مهلة تثبيت ≤ 5s، والمحاولات ≤ 2. فالحسم ≤ 25s من t_s دون افتراض استقرار أي وعد قراءة أو كتابة. نتائج القراءة بعد الحسم تُهمل بـgateId، فلا تغير الحسم ولا تصدر عملية.
- الحدثان (D107):
  - apply: ظهور السجل في التخزين، فيراه listKeys.
  - settle: استقرار وعد set في الجيل الحي.
  - الحسم يعتمد settle وحده، وapply دون settle لا يغير الحالة.
- أول settle ناجح لأي محاولة قبل الحسم يجعل المفتاح ready فورًا.
- confirmed = أكبر نسخة استقرت بنجاح. نسختا التثبيت تحملان القاموس نفسه، فلا يتغير القاموس بنجاح المحاولة الأخرى لاحقًا.
- ready لا يعود إلى حالة سابقة.
- failed يُحسم حين تكون كل محاولة قد فشل settle لها أو انقضت مهلتها. بعدها يبقى نهائيًا في الجيل، وأي settle ناجح متأخر لا يغيره ويُعد في lateRecordsSeen.
- فشل المحاولتين أو انقضاء مهلتيهما يجعل المفتاح failed: رسائل المفتاح تُرد بـ−32603 {storeRecovery} وتُحرر، ولا snapshot في هذا الجيل.
- المحاولة المتأخرة، إن طُبقت، نسختها أقل من كل كتابة بعد ready، وقاموسها هو القاموس المسترد نفسه.

لِمّة الحسم:
- كل كتابة من جيل سابق نسختها < (E, 0).
- إن طُبقت قبل قراءة الاسترداد صارت أساس نقطة التثبيت. وإلا بقيت أقل من كل سجل لاحق، فلا تظهر أبدًا.
- إذن نتيجتها تُحسم مرة واحدة ولا تنقلب، ولا تتجاوز كتابة لاحقة.
- الكتابة التي رُد عليها null التزمت قبل الرد، ولا يُحذف سجل قبل التزام سجل أكبر منه، فلا تُفقد.

لِمّة التخصيص (D104):
- داخل الجيل يأخذ كل set رقمًا جديدًا قبل إصداره، فلا يتكرر اسم سجل.
- كل عملية متأخرة من الجيل نسختها أقل من كل عملية خُصص لها رقم بعدها.
- البوابة الواحدة تمنع نقطتي تثبيت متوازيتين.

لِمّة البقاء:
- الحذف، في الكنس أو الاسترداد، لا يزيل سجلًا r إلا بعد رؤية سجل موجود h > r للمفتاح نفسه.
- لا يُزال أكبر سجل أبدًا.
- بالاستقراء يبقى أكبر سجل مطبق موجودًا، حتى لو طُبق الحذف متأخرًا.

**4. الذاكرة**
- زوال العامل يُسقط الطوابير والعدادات والأقفال والجلسات والحجوزات.
- الجيل الجديد يبدأ بعدادات صفرية، ولا يرث معرفات الرسائل.
- لا يصل رد لرسالة من جيل سابق.

**5. الإطارات**
- viewer يحمل منفذ runtime مع العامل، والعامل يعلن genId في رسالة hello.
- عند onDisconnect أو ظهور genId مختلف:
  - يهدم viewer كل إطارات المواقع فورًا.
  - يعرض «انقطع عامل الإضافة؛ قد تكون آخر الكتابات غير محفوظة» مع زر «إعادة التحميل».
- لا إعادة تحميل تلقائية. الجلسة الجديدة بفعل المستخدم (D99)، فلا يجدد موقع دلوه بإسقاط العامل.
- المفاتيح المتسخة في لقطة الإطار المهدوم تُفقد بإعلان، ولا تُرسل من لقطة قديمة.
- إعادة التحميل تأخذ snapshot بعد استرداد المفتاح.

**6. القرص**
الحدود الملزمة (D104):
- حجم السجل: مع SITE_ENTRIES_MAX = 4096 يكون enc ≤ RECORD_BYTES_MAX = 4·ceil((1048576+8·4096)/3)+256 = 1442048.
- القياس:
  - inUse = getBytesInUse(null)، يُقاس بعد كنس الإقلاع وبعد استقرار كل set أو remove.
  - names = عدد الأسماء التي يعيدها getKeys.
- قاعدة القبول لكل set (بيانات، أو تثبيت، أو حذف، أو مسح):
  - inUse + unres + enc(w) + LATE_RESERVE + META_RESERVE ≤ STORE_DISK_HARD = 256 MiB.
  - names + unresN + 1 + LATE_NAMES + EPOCH_NAMES_MAX ≤ STORE_RECORDS_MAX = 1024، أي احتياطي 24+8 = 32 (D134). العناصر الظاهرة تُحسب في names وفي الاحتياطي معًا، وهذا محافظ.
  - unres وunresN: مجموع حجوزات DiskLedger الحية (D105) بالبايت وبالأسماء، لا تقدير منفصل.
  - LATE_RESERVE = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS)·RECORD_BYTES_MAX = 4·4·1442048 = 23072768، وLATE_NAMES = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS+ADMIN_SLOTS) = 24.
    - الستة لكل جيل: مقعدان عامان، ومقعدا تثبيت (D109)، ومقعدا القبر (D108).
    - عناصر epoch خارجها، ويحجزها EPOCH_NAMES_MAX = 8 كليًا (D134).
  - META_RESERVE = 1 MiB لعناصر epoch والقبور. القبر وحده معفى من شرط البايت ومحسوب في META_RESERVE. القيد (D108): META_RESERVE ≥ STORE_RECORDS_MAX·TOMB_BYTES_MAX + (GEN_WINDOW_MAX+1)·ADMIN_SLOTS·TOMB_BYTES_MAX + EPOCH_NAMES_MAX·EPOCH_BYTES_MAX = 524288+5120+2048 = 531456 ≤ 1048576 (D134). مكونات القيد:
    - القبور الباقية ≤ STORE_RECORDS_MAX، لأن القبر غير معفى من شرط الأسماء.
    - القبور غير المستقرة ≤ ADMIN_SLOTS للجيل الحي ولكل جيل ميت.
    - EPOCH_BYTES_MAX = 256، وعناصر epoch ≤ EPOCH_NAMES_MAX = 8 في كل لحظة (D134).
- الخرق: 4300 {reason: 'disk', limit} قبل أي set، ولا يُحذف أي سجل لإفساح مكان.
- DiskLedger (D105): القبول والحجز خطوة متزامنة واحدة في العامل، بلا await بين الفحص والإضافة. JavaScript أحادي الخيط، فلا يتداخل قبولان.
  - reserve(op): يفحص الشروط بقيم R_bytes وR_names وR_sites الحالية، ثم يضيف إليها في الخطوة نفسها enc(op) واسمًا واحدًا ومقعد موقع إن كان الموقع جديدًا، ويعيد DiskTicket مملوكة للعملية.
  - يشمل ذلك الحذف والمسح ونقطة التثبيت والقبر (بايتاته في META_RESERVE). أما remove فبتذكرة صفرية.
  - عمر التذكرة:
    - عند settle تصير settled برقم settleSeq، وتبقى محسوبة.
    - تُسقط فقط عند اكتمال refresh (getBytesInUse وgetKeys) صدر طلبه بعد settleSeq، مع اعتماد قياسه في الخطوة نفسها. إذن لا فجوة بين خروجها من الحجز ودخول أثرها في القياس.
  - refresh متسلسل، واحد في كل لحظة، ويُجدول بعد كل settle.
  - مقعد الموقع: بعد نجاح الكتابة يظهر في sitesLive المقيس. بعد فشلها يُحرر مع ذلك القياس نفسه.
  - التذكرة التي لا يستقر وعدها تبقى حتى زوال الجيل، ثم يغطيها LATE_RESERVE تحت A15c.
  - الحذف لا يُطرح من الاستهلاك إلا بالقياس، فالتأخر يبالغ في الاستهلاك ولا ينقصه.
  - لِمّة الحجز: كل عملية للجيل الحي غير مشمولة في آخر قياس تملك تذكرة حية. فالقرص بمقياس Chrome ≤ inUse + R_bytes + أثر الأجيال الميتة، والشرط عند كل قبول يحفظ الحد. والمثل للأسماء والمواقع.
- مقاعد المواقع (D105، D112، I66):
  - sitesLive = عدد المفاتيح (netKey، عنوان) المتمايزة التي لها اسم سجل واحد على الأقل في آخر getKeys معتمد، أيًا كان محتواه: بيانات، أو {}، أو قبر.
    - لا يُحتسب مفتاح مرتين.
    - لا يخرج مفتاح منه إلا بقياس معتمد لا يرى له أي سجل، بشرط ألا يكون له pin في الجيل الحي (D113).
    - pin (D113): كل تذكرة DiskLedger، عدا remove، لعملية على مفتاح محتسب لحظة قبولها تثبّت المفتاح في sitesLive حتى إسقاط التذكرة (D105). القياس الذي يسقط التذكرة صدر بعد settle، فيرى أثرها إن طُبقت. لذلك لا يبقى مفتاح أنشأته العملية دون عد.
    - التذكرة التي تحمل مقعد موقع تُسقط في خطوة اعتماد القياس الذي يراه (D105)، فلا فجوة ولا عدّ مزدوج.
  - العمليات التي قد تنشئ مفتاحًا جديدًا اثنتان: set بيانات، ونقطة تثبيت لمفتاح بلا سجل (القاموس {}).
    - القبر لا ينشئ مفتاحًا، لأنه لا يصدر إلا بعد سجل للمفتاح في الجيل الحي.
    - remove لا ينشئ مفتاحًا.
  - LATE_SITES_PER_GEN = STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS = 4. كل عملية منشئة غير مستقرة تملك مقعدًا حتى settle (D102، D109). والمستقرة بنجاح يراها الجيل التالي، والمرفوضة لا تُطبق (A15d). فعمليات الجيل الميت التي قد تنشئ مفتاحًا لاحقًا ≤ 4.
  - lateGens(now) = عدد عناصر epoch الظاهرة X ≤ E، ومنها الجيل الحي، بحيث bootMs(X) ≥ now−T_LATE (D134).
    - كل X يمثل الجيل الذي سبقه، وعملياته تُطبق أو تُسقط قبل bootMs(X)+T_LATE (A15c).
    - العدد ≤ GEN_WINDOW_MAX بنافذة الأجيال.
    - هذه العناصر لا تُكنس قبل خروجها من النافذة.
    - (D134) الجيل غير المؤكد لا يكتب إلا عنصر epoch باسم مشترك، دون بيانات. قد تكتب أجيال أقدم قيمة العنصر نفسه متأخرة، فيصير bootMs المخزن أقدم.
    - لِمّة التغطية: لكل جيل ميت D كتب بيانات، كل كاتب للاسم E_D+1 أقلع بعد D. السبب: لو أقلع قبله ورأى E_D لاختار D العدد E_D+1 لا E_D.
    - إذن bootMs المخزن ≥ إقلاع خلف D، فيبقى العنصر في النافذة ما دامت عمليات D قد تُطبق.
    - العناصر الظاهرة من أجيال غير مؤكدة تُعد أيضًا، وهذا محافظ.
  - LATE_SITES(now) = LATE_SITES_PER_GEN·lateGens(now) ≤ 16.
    - إسهام X لا يسقط بانقضاء الزمن وحده، بل عند اكتمال refresh صدر بعد bootMs(X)+T_LATE، مع اعتماد قياسه في الخطوة نفسها.
    - DiskLedger يجدول refresh عند كل انقضاء.
  - القبول: كل عملية منشئة لمفتاح جديد (غير محتسب في sitesLive وبلا مقعد محجوز) تتطلب sitesLive + R_sites + LATE_SITES(now) + 1 ≤ STORE_SITES_MAX = 64.
    - يُحجز المقعد في خطوة القبول نفسها.
    - الخرق يعطي 4300 {reason: 'sites', limit: 64, retryAfterMs?} دون set.
    - retryAfterMs هو الزمن حتى أقرب انقضاء يكفي للقبول، ويغيب إن كان sitesLive + R_sites وحدهما يمنعان القبول.
    - العمليات على مفتاح محتسب أو محجوز لا تحتاج مقعدًا، ولا تتأثر بـLATE_SITES.
  - إزالة آخر سجل لمفتاح (قبر وحيد) تتطلب ما يلي:
    - لا جلسة، ولا بوابة جارية، ولا تذكرة حية للمفتاح.
    - خروج كل عناصر epoch الأقدم من الجيل الحي من النافذة.
    - المقعد لا يتحرر إلا بالقياس اللاحق.
  - مفتاح محتسب بقبر باقٍ (D113، I67):
    - القبر سجل، فالمفتاح في sitesLive ومقعده قائم.
    - كل عملية على المفتاح لا تحجز مقعدًا ولا تزيد R_sites، وتثبّته بـpin. يشمل ذلك نقطة تثبيت بالقاموس {}، وأول set بعد حسم الحذف.
    - المقعد الجديد يلزم فقط إن تحققت ثلاثة شروط معًا: أُزيل آخر سجل، واعتُمد قياس لا يرى المفتاح، ولا pin له. عندها تكون العملية الأولى عملية منشئة تخضع لشرط القبول.
  - TombReaper (D113):
    - يفحص شروط الإزالة أعلاه عند كل refresh معتمد وعند كل خروج epoch من النافذة.
    - إن تحققت يصدر remove للقبر الوحيد بتذكرة صفرية.
    - فتح جلسة للمفتاح قبل الإصدار يمنعه.
    - remove الصادر لا يُلغى. أي عملية لاحقة على المفتاح قبل القياس الذي لا يراه تُعامل كعملية على مفتاح محتسب، فتثبّته بـpin. لذلك لا يُحسب مرتين، ولا يسقط من العد قبل ظهور أثرها.
    - remove متأخر من جيل ميت يُعامل بالقاعدة نفسها، لأن pin يتبع عمليات الجيل الحي.
  - لِمّة المواقع (مشروطة بـA15c وA15d): في كل لحظة، كل مفتاح على القرص يقع في إحدى ثلاث حالات:
    - (1) في آخر قياس معتمد.
    - (2) له تذكرة حية في الجيل الحي.
    - (3) أنشأته عملية غير مستقرة لجيل ميت D. هذه ≤ 4 لكل D، وتنتهي قبل bootMs(succ(D))+T_LATE، وsucc(D) محتسب في lateGens حتى قياس صادر بعد ذلك.
    - شرط القبول يحفظ مجموع الحدود الثلاثة ≤ 64. إذن عدد المفاتيح على القرص ≤ 64 في كل لحظة.
    - لا يُحذف أي سجل آليًا لإصلاح تجاوز.
  - الأثر: بعد كل إقلاع يقل عدد المواقع الجديدة الممكن قبولها بمقدار 4 لكل جيل في النافذة، حتى 60s. المواقع القائمة لا تتأثر، والرفض يقع فقط إن كان sitesLive + R_sites > 63 − LATE_SITES(now).
- لِمّة الحد (مشروطة بـA15c): القرص بمقياس Chrome ≤ inUse + unres + عمليات ≤ GEN_WINDOW_MAX جيلًا ميتًا ≤ STORE_DISK_HARD. كل جيل ميت له ما يلي:
  - ≤ 2 set بيانات، لأن المقاعد العامة 2.
  - ≤ RECOVERY_SLOTS = 2 نقطة تثبيت، بلِمّة RecoverySlots (D109).
  - ≤ ADMIN_SLOTS = 2 قبر بلِمّة AdminSlots (D108)، ضمن META_RESERVE.
  - عناصر epoch لا تُحسب لكل جيل، بل يحدها EPOCH_NAMES_MAX = 8 كليًا بلِمّة حد الأسماء (D134)، ضمن META_RESERVE.
- نافذة الأجيال: لا يصدر EpochFence عنصره إن وُجدت GEN_WINDOW_MAX = 4 عناصر epoch ظاهرة بـbootMs ≥ now−T_LATE (D134).
  - لِمّة: الجيل الذي يكتب بيانات يرى ≤ 3 عناصر في النافذة قبل إصداره.
  - بعد إصداره لا يظهر اسم جديد غير اسمه (لِمّة حد الأسماء)، والكتابة المتأخرة لا تجعل bootMs أحدث.
  - إذن lateGens ≤ 4 طوال حياته.
  - ينتظر حتى يخرج أقدمها من النافذة، ولا كتابة ولا snapshot قبل ذلك.
  - الجيل المنتظر لم يكتب عنصرًا، فلا يُعد ضمن النافذة.
- الكنس: عند الإقلاع وبعد كل set ناجح، يحذف لكل مفتاح ما دون أكبر سجل موجود فقط. فشل remove يبقي السجل محتسبًا في inUse وnames، فيتوقف القبول عند الحد بدل النمو.
- الحذف الكلي لموقع يتم بفعل المستخدم فقط، من صفحة «إدارة التخزين» بتأكيد يعرض الموقع والحجم:
  - ينفذ AdminDelete (D106) أدناه، ثم يكنس ما دونه.
  - AdminDelete(key) عملية واحدة لكل مفتاح، وأي طلب ثانٍ (من صفحة أخرى) يشترك في وعدها.
  - 1: المفتاح يصير deleting، وviewer يمنع فتح جلسة جديدة له ويعرض «جارٍ حذف البيانات».
  - 2: تُغلق كل جلسات المفتاح في كل التبويبات:
    - تُهدم الإطارات بإعلان «حُذفت بيانات الموقع»، دون إعادة تحميل تلقائية.
    - رسائلها queued وready تصير suppressed ثم release.
    - الكتابات active تبقى بملكيتها حتى settle (D102).
  - 3: انتظار حسم RecoveryGate (≤ 25s، D109، D110). الحالات ready وfailed وunrecovered (busy أو readTimeout) كلها تتابع. الأخيرة لم تصدر أي عملية كتابة، ونتائج قراءتها المتأخرة تُهمل بـgateId. إن كان المفتاح unrecovered دون بوابة جارية يتابع فورًا.
  - 4: القبر بنسخة SeqAlloc.alloc() بعد الخطوتين، وبتذكرة DiskLedger. يأخذ مقعدًا من AdminSlots (D108)، لا من STORE_ACTIVE_GLOBAL. تعذر المقعد يعطي adminBusy دون قبر.
    - لا ينتظر الكتابات النشطة، لأن نسخته أكبر من كل نسخة خُصصت قبله.
    - لا تُخصص نسخة أخرى للمفتاح أثناء deleting.
  - settle ناجح للقبر: confirmed = القبر، والقاموس {}، وtotal = 0. settle متأخر لكتابة أقدم لا يغير القاموس، لأن نسختها < confirmed.
  - فشل القبر أو انقضاء مهلته (5s):
    - محاولة ثانية بنسخة جديدة، على مقعد وفق AdminSlots (D108).
    - الأولى تبقى مالكة مقعدها حتى settle.
  - النتيجة التالية تقع في ثلاث حالات: فشل المحاولتين، أو انقضاء مهلتيهما، أو تعذر مقعد للثانية:
    - المفتاح يصير unrecovered، وأول جلسة لاحقة تجري استردادًا جديدًا بنقطة تثبيت.
    - الصفحة تعرض «فشل الحذف»، أو «نتيجة الحذف غير مؤكدة» عند المهلة.
    - القبر المطبق قبل قراءة الاسترداد يصير أساسه، أي حذف ناجح. والمطبق بعدها أقل من نقطة التثبيت فلا يظهر.
  - إعادة الإنشاء (D113): جلسة جديدة بفعل المستخدم بعد الحسم ترى {}، ونسخة أول set لها أكبر من القبر.
    - ما دام القبر باقيًا ومحتسبًا، لا تحجز الجلسة مقعدًا، ولا تتأثر بـLATE_SITES.
    - إن أزال TombReaper القبر واعتُمد قياس لا يرى المفتاح، فنقطة التثبيت الأولى منشئة وتحجز مقعدًا واحدًا وفق D105 وD112.
  - لِمّة الحذف: كل set للمفتاح في الجيل خُصصت قبل القبر أو بعد حسمه، والأجيال السابقة نسخها < (E, 0). إذن لا يعلو القبر أي سجل من لقطة قديمة.
  - لِمّة عدم القبر المتأخر (D111): كل قبر يُصدر في خطوة يجتاز فيها الحارس، أي op.state = running. والحسم ينقل الحالة إلى settled قبل أي خطوة لاحقة، ولا ينقلها شيء إلى running مجددًا. إذن لا يُصدر قبر لعملية محسومة. فكل set لجلسة أُنشئت بعد الحسم لا يعلوها قبر من تلك العملية. القبر الصادر قبل الحسم ولم يستقر بعدُ نسخته أقل من كل تخصيص بعد الحسم، فلا يعلو الكتابة الجديدة.
  - لا يُحذف القبر قبل خروج كل الأجيال الأقدم منه من نافذة T_LATE.
- AdminSlots (D108، I61): ملكية مقاعد القبر.
  - ADMIN_SLOTS = 2 مقعدان لكل الإضافة، خارج STORE_ACTIVE_GLOBAL. حالة المقعد ∈ {free, held(op)}.
  - الأخذ: لحظة إصدار set القبر فقط، من AdminFIFO عام بترتيب الطلب. يتم في الخطوة المتزامنة نفسها مع DiskLedger.reserve.
  - التحرير: عند settle وعد القبر فقط، نجاحًا أو فشلًا. لا يحرره رد المهلة، ولا حسم AdminDelete، ولا الإعلان. زوال الجيل يسقط الحالة كلها، والقبر المعلق يبقى عملية متأخرة لجيل ميت.
  - الانتظار: ≤ ADMIN_SLOT_WAIT = 5s لكل محاولة، ولا قبر دون مقعد.
  - المحاولة 1، عند تعذر المقعد: adminBusy. لا قبر، والمفتاح unrecovered، والبيانات باقية. الصفحة تعرض «تعذر بدء الحذف؛ البيانات باقية».
  - المحاولة 2 تبدأ بعد فشل settle الأولى أو انقضاء مهلتها. عند تعذر مقعدها:
    - uncertain إن كانت الأولى معلقة.
    - failed إن كانت الأولى قد فشلت.
    - في الحالتين يصير المفتاح unrecovered.
  - settle ناجح متأخر لقبر بعد الحسم: لا يغير الحسم، ويحكمه الاسترداد التالي كما سبق.
  - هوية العملية (D111، I65):
    - كل AdminDelete يملك opId فريدًا في الجيل، وحالة ∈ {running, settled}.
    - deleting[key] يحمل opId العملية الجارية.
    - كل طلب AdminFIFO يحمل opId عمليته وrequestId.
  - الحسم: أول settle ناجح لأي محاولة قبر قبل الحسم يجعل النتيجة deleted فورًا. الحسم بأي نتيجة (deleted أو failed أو uncertain أو adminBusy) ينفذ في خطوة متزامنة واحدة ما يلي:
    - op.state = settled.
    - سحب كل طلب AdminFIFO معلق للعملية دون تخصيص seq ولا حجز.
    - مغادرة المفتاح حالة deleting.
    - تحرير الجلسات الجديدة.
  - حارس الإصدار: في الخطوة المتزامنة التي يُمنح فيها مقعد لطلب، وقبل DiskLedger.reserve وSeqAlloc.alloc، يُفحص ما يلي:
    - الطلب غير مسحوب.
    - op.state = running.
    - المفتاح deleting.
    - deleting[key].opId = op.opId.
  - فشل أي فحص: يُعاد المقعد فورًا في الخطوة نفسها إلى الطلب التالي في AdminFIFO، دون reserve ولا alloc ولا set، ويزيد adminStaleDropped. المنح والحارس والحجز والتخصيص والإصدار خطوة واحدة بلا await بينها.
  - RecoverySlots (D109) يطبق الحارس نفسه: لا إصدار إلا إن كان gateId الطلب هو الجاري، وكانت البوابة غير محسومة. والتعارض يعيد المقعد ويزيد recoveryStaleDropped.
  - الحد الزمني لـAdminDelete ≤ 25s (حسم البوابة شاملًا القراءة، D110) + 2·(5s انتظار + 5s مهلة) = 45s.
  - لِمّة AdminSlots:
    - كل قبر صادر يملك مقعدًا من لحظة الإصدار حتى settle، ولا يُصدر قبر دون مقعد.
    - إذن عدد القبور الصادرة غير المستقرة في الجيل ≤ ADMIN_SLOTS في كل لحظة، ومنها لحظة زواله.
    - كل جيل ميت يترك ≤ 2 قبر متأخر، مهما تعددت المفاتيح أو المحاولات.
  - الطلبات لمفاتيح مختلفة تتنافس في AdminFIFO، وطلبات المفتاح نفسه تشترك في وعد واحد.
  - TOMB_BYTES_MAX = 512 لاسم القبر وقيمته المرمزة معًا. RecordCodec يرفض ما يتجاوزه، فلا يُصدر.
- التعداد: getKeys إلزامي، ويُتحقق من توفره في M0 وBR22c.
  - غيابه يعطل التخزين المحلي بـstoreRecovery، ولا يُستعمل get(null).
  - ذاكرة التعداد تتناسب مع names ≤ STORE_RECORDS_MAX+LATE_NAMES تحت A15c.

**7. المراقبة**
stats تعيد {epoch, recoveries, recoveryCorrupt, lateRecordsSeen, storeRecoveryFail, inUse, names, sitesLive, diskRejects, removeFailures, genWaitMs, seqMax, resvBytes, resvNames, resvSites, ticketsLive, adminDeletes, adminDeleteFailed, adminDeleteUncertain, adminBusy, adminSlotsHeld, adminTombsUnsettledMax, recoverySlotsHeld, recoveryUnsettledMax, recoveryBusy, readSlotsHeld, readTimeouts, lateReadsDropped, recoveryCorruptFail, adminStaleDropped, recoveryStaleDropped, lateGens, lateSites, sitesRejectsLate, pinnedKeys, tombReaps, epochNames, epochGateBlocks, epochResurrected}. lateRecordsSeen عدد السجلات التي ظهرت بعد نقطة التثبيت بنسخة أقل منها.

**المحفظة (D14)**
- eth_requestAccounts بموافقة لكل (netKey, siteAddress).
- eth_sendTransaction بعد الاتصال، وبموافقة مربوطة بـtxHash.
- الإلغاء بـ4001 عند انقطاع المنفذ، أو إعادة تشغيل العامل، أو أكثر من طلب معلق، أو أكثر من 5 في الدقيقة.
- مرفوض: switch/addEthereumChain، وeth_sign، وpersonal_sign، وsignTypedData، وeth_sendRawTransaction.
- AES-GCM مع PBKDF2 بـ600k تكرار، وقفل بعد 15 دقيقة.

فتح الموقع وحده لا يوقع شيئًا ولا يمنح حسابًا ولا يبث معاملة ولا يفتح تبويبًا خارجيًا. سياسات المتجر غير متحققة، والتجربة بإضافة غير محزومة.
