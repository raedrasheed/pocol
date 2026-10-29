# واجهات العقد والشبكة

**P2P**
libp2p عبر TCP+Noise، مع gossipsub، وطلب ورد على '/pocol/1/req'.

**الرسائل**
- Status = [chainId, genesisHash, protocolVersion, headHash, height, totalWork]. اختلاف الهوية أو إصدار غير مدعوم يقطع الاتصال. Status معلوماتي لا يغني عن 1ب.
- Request = [reqId, kind, body]: 1 GetHeaders [locator ≤ 32, count ≤ 512]، و2 GetBodies [hashes ≤ 64]، و3 GetTemplate، و4 GetEnvelope [blockHash, exclude ≤ 4 hdrId] (D131). رد GetEnvelope هو النسخة المخزنة الصالحة الغلاف إن لم يكن hdrId لها في exclude، وإلا NotFound.
- Response = [reqId, status, payload]. status: 0 نجاح، و1 غير موجود، و2 تجاوز، و3 غير صالح، و4 تقييد. الجسم المفقود 0x80، والبتر عند 8 MiB.

**gossip (الحد بالبايت)**
- tx: 131072.
- template: 197294. الفحص: الحجم، ثم الشكل الموقّع (وإلا templateForm)، ثم 1ب، ثم ecrecover، ثم البنود الممكنة.
- block: 3067 (الرأس وحده).
- share: 43.
- evidence1: 1368.
- evidence2: 243.

**استلام block**
1. الحجم، ثم 1ب دون الأب. الفشل عقوبة دون تخزين.
2. الأب المجهول: طابور unknownParent ≤ 64، دون عمل ولا ASERT. رفض الأب يُسقط الابن بـancestorInvalid.
3. بقية H-pre (ومنها 10أ)، ثم الجسم، ثم H-full، ثم البث.
4. (D129) envReject يُسقط النسخة ويعاقب مرسلها، دون حكم على blockHash. تُطلب نسخة أخرى من نظير أعلن الكتلة. البث يرسل النسخة المخزنة بايتيًا.

**المزامنة (D38)**
1. Status واللوكيتور.
2. R: H-pre لكل رأس، والمخالف يُسقط مع نسله ويُحظر النظير. ≤ 4096 لكل فرع و≤ 16384 إجمالًا.
3. E: GetBodies، ثم H-full متتابعًا.
   - الجسم الذي لا يطابق txRoot وevidenceRoot خطأ رد (BodyMismatch، D125): يُسقط، ويُعاقب النظير، ويُعاد الطلب من نظير آخر، ويبقى الرأس.
   - الجسم المطابق الذي يخالف H-full بخرق blockReject يبطل الكتلة ونسلها. خرق envReject (ومنه الجزء التوقيعي من البند 13) يرفض النسخة وحدها، ويبقى الرأس ومدخله، وتُطلب نسخة أخرى (D130).
   - جدولة الجلب (D128، سلوك عقدة لا صلاحية) ثلاث آليات:
     - DiscRun: تشغيلة اكتشاف واحدة مع علم dirty، وtried لكل تشغيلة.
     - BodySched: طلب جسم جارٍ واحد لكل هاش، وtriedPeers.
       - إعادة الأهلية (D130، تعدّل D129) تضع النظير في صنف reeligible دون محوه من tried للدورة الجارية. أسبابها: Status منه، أو إعلانه الكتلة أو نسلها، أو انقضاء مهلة أسية min(1s·2^k, 60s).
       - الاختيار بصرامة: غير المجرَّب في الدورة بترتيب المعرّف، ثم reeligible بترتيب الإعلان. لا يُطلب نظير reeligible ما دام لـx نظير غير مجرَّب.
       - cycleReset(x) يبدأ دورة جديدة: يمحو tried وreeligible ومؤقتات المهلة لـx. في pocold يقع عند صيرورة x قابلة للطلب من جديد بعد إزالتها، وفي SimNet لكل الهاشات في خطوة t_avail الذرية (أمر تجهيز).
     - EnvFetch (D131، سلوك عقدة لا صلاحية): يُفعَّل لـx حين تكون كل نسخ رأسها لدى i مرفوضة بـenvReject، أو حين لا يحوز i نسخة envOk بعد توفر حالة الأب.
       - الطلب GetEnvelope [x, exclude]، حيث exclude أحدث ≤ 4 من hdrId المرفوضة لـx. الصادق لا يخزن ولا يخدم إلا نسخة envOk.
       - الجدولة كـBodySched: طلب جارٍ واحد لكل x، وtriedEnv وreeligible لكل دورة، واختيار صارم بترتيب المعرّف لغير المجرَّبين. صنف reeligible طابور FIFO بلحظة الدخول، وللنظير فيه موضع واحد، ودخوله ثانية بعد طلبه يضعه في الذيل (D139). cycleReset عند t_avail في SimNet، ومهلة أسية في pocold.
       - (D137) لا يُصدر طلب لـx قبل توفر حالة الأب. سبب ذلك أن envOwn يجب أن يكون قابلًا للتقييم، فلا يدخل رد مطلوب envPending.
       - EnvQueue: مدخل واحد لكل x يحتاج جلبًا، بترتيب FIFO لحظة الحاجة، و≤ 64 B لكل مدخل. عدد المداخل ≤ حد طوابير المزامنة (16384).
       - EnvSlots (D137): ENV_SOLICIT_SLOTS = 32 مقعدًا للطلبات المطلوبة، منفصلة عن ENV_GOSSIP_SLOTS = 32 لفحص النسخ غير المطلوبة. المجموع = ENV_INFLIGHT_GLOBAL = 64.
         - المقعد يملك فحصًا واحدًا ومخزن رد ≤ ENV_REPLY_MAX = 3067+64 = 3131 B.
         - المنح: لرأس EnvQueue فقط، في الخطوة نفسها التي يُختار فيها النظير ويُرسل الطلب. لا طلب دون مقعد. release لمقعد مطلوب يمنحه في الخطوة نفسها لرأس EnvQueue إن وُجد، فالمنح محافظ على العمل (D139).
         - الحالات (D138): reserved (الطلب مرسل، ولا بايت في المخزن)، ثم buffered (الرد يُقرأ داخل المخزن المحجوز)، ثم checking (الفحص يقرأ المخزن)، ثم released.
         - المحاولة: كل منح يأخذ attemptId من نوع u64، رتيبًا لكل عقدة، ولا يُعاد استخدامه.
           - كل حدث يحمل (slotIdx, attemptId): الرد، والمهلة، والانقطاع، ونتيجة الفحص.
           - الحدث الذي لا يطابق المحاولة الحالية للمقعد يُهمل، ولا يغيّر أي مقعد.
         - التسلسل: أحداث EnvFetch تُعالج في حلقة واحدة بترتيب الوقوع. عند تساوي اللحظة يسبق اكتمال الرد المهلة: الرد في وقته إن وصل آخر بايت منه عند t ≤ sendTime+ENV_REQ_TIMEOUT.
         - المهلة: ENV_REQ_TIMEOUT يقيس من الإرسال حتى آخر بايت من الرد فقط. الانتقال إلى checking يلغي مؤقتها، فلا مهلة أثناء الفحص.
         - الإلغاء منفصل عن الملكية. كل سبب إنهاء يضع cancelRequested، ثم:
           - في reserved أو buffered: يُعاد ضبط substream الطلب، وتُهمل البايتات الجزئية، ويقع release في الخطوة نفسها، لأن لا مهمة تقرأ المخزن.
           - في checking: لا تحرير. الفحص (envPre ثم envOwn، دون apply) حساب متزامن لا يُقاطع، يُنفذ في EnvCheckPool وحده (D139). يبقى مالكًا للمقعد والمخزن حتى يستقر مهما طال، ويقع release في خطوة الاستقرار نفسها.
             - EnvCheckPool: ENV_CHECK_WORKERS = W = 4 خيوط مخصصة لا تنفذ غير فحوص الغلاف، بطابور FIFO بلحظة الدخول في checking. لا يُنفذ فحص غلاف في ComputePool ولا في runtime الإجماع.
             - c_exec: زمن تنفيذ فحص واحد بساعة الجدار، وفرضيته ENV_EXEC_MAX = 50 ms.
             - لِمّة المدى: الفحوص الحية ≤ 64، فأمام أي فحص ≤ 63، ويتحرر له خيط خلال ≤ ⌈63/W⌉·c_exec.
             - إذن c_env = (⌈63/W⌉+1)·ENV_EXEC_MAX = 17·50 = 850 ms، ما دام c_exec ≤ ENV_EXEC_MAX.
             - تجاوز ENV_EXEC_MAX يزيد envCheckOverrun، ولا يحرر المقعد، ويبطل الحد الزمني لا السقف.
         - نتيجة الفحص الملغى:
           - حكم envReject يُحفظ ويُعاقب به المرسل إن بقي لـx مدخل محلي، لأن الحكم دالة على البايتات وحدها.
           - حكم envOk يُستعمل فقط إن بقيت x في envWait.
           - غير ذلك تُهمل النتيجة، ويزيد lateCheckDropped.
           - في كل الحالات: لا تُستورد كتلة مزالة أو مستوردة، ولا يُحرر مقعد محاولة أخرى.
         - لِمّة السقف (D138): كل مهمة فحص وكل بايت مستقبَل يملكه مقعد من لحظة المنح حتى release، وrelease لا يسبق استقرار الفحص. إذن في كل لحظة، مهما توالت الإلغاءات:
           - الفحوص الجارية ≤ 32 مطلوبًا + 32 gossip.
           - المخازن الحية ≤ 64·3131 B.
         - مقاعد gossip تتبع القواعد نفسها: acquireGossip يحجز المقعد قبل الفحص، وتنتقل بايتات النسخة إلى مخزن المقعد.
         - الرد الأكبر من ENV_REPLY_MAX يُقطع عند الحد، ويُعامل خطأ رد يعاقب المرسل.
         - release يقع مرة واحدة بالضبط، ويحرسه علم ومعرّف المحاولة. لا يقع في checking إلا عند استقرار الفحص (D138). أسباب إنهاء المحاولة ثمانية:
           - اكتمال الفحص (envOk أو envReject).
           - NotFound.
           - انقضاء ENV_REQ_TIMEOUT = 2s من الإرسال قبل اكتمال استقبال الرد. لا يسري في checking.
           - انقطاع النظير.
           - رد أكبر من الحد.
           - استيراد x.
           - تخزين نسخة envOk لـx من مسار آخر.
           - blockReject لـx أو إزالتها.
         - بعد release غير ناجح يعود x إلى ذيل EnvQueue إن بقي في envWait وله نظير غير مجرَّب أو reeligible. وإلا ينتظر حدث إعادة أهلية، ثم يدخل الذيل.
         - المهلة تضع النظير في triedEnv للدورة، مع مهلة أسية لإعادة الأهلية كـNotFound. لا استبعاد دائم، ولا عقوبة للبطء وحده.
         - بعد المهلة أو الإلغاء في reserved أو buffered يُعاد ضبط substream، فلا يصل رد مطلوب متأخر على الطلب نفسه. النسخة التي يرسلها النظير لاحقًا لا مقعد مطلوبًا لها، وتمر بمسار النسخة غير المطلوبة أدناه دون عقوبة على التأخر.
       - الرد بنسخة تفشل الغلاف خطأ رد يعاقب المرسل ويضعه في triedEnv. النسخة المكررة (hdrId في EnvCache) لا تُفحص ثانية.
       - النسخ envPending ≤ ENV_PENDING_MAX = 4 لكل x. الزائد يُسقط دون فحص ولا يطرد المخزن، وإسقاطه لا يمنع EnvFetch لاحقًا.
       - احتفاظ أحكام الغلاف (D136، سياسة عقدة محلية لا صلاحية):
         - الحكم المحفوظ لـenvReject = {hdrId، blockHash، rule، peer، seq} ≤ ENV_ENTRY_MAX = 128 B. لا تُحفظ بايتات النسخة المرفوضة.
         - لا يُحفظ حكم إلا لـblockHash له مدخل محلي: HeaderStore، أو طوابير المزامنة، أو unknownParent، أو الجانبية. خارج ذلك تُعاقب النسخة وتُسقط دون حفظ.
         - الحدود: rej(x) ≤ ENV_REJ_PER_BLOCK = 8، والمجموع ≤ ENV_REJ_GLOBAL = 16384. النسخ envPending عامًا ≤ ENV_PENDING_GLOBAL = 1024. الفحوص الجارية ≤ ENV_INFLIGHT_GLOBAL = 64، منها ≤ 32 مطلوبة و≤ 32 غير مطلوبة (D137).
         - النسخة غير المطلوبة (gossip أو إعلان) لـx بلا نسخة envOk تمر بالخطوات التالية:
           - hdrId محفوظ: تُسقط دون فحص.
           - rej(x) = 8، أو pending(x) = 4، أو امتلاء عام، أو لا مقعد gossip حر وحالة الأب متوفرة (D137): تُسقط دون فحص ودون عقوبة، ويزيد envFloodDropped. إن لم تتوفر حالة الأب فالنسخة envPending ضمن حديها، دون مقعد. تبقى x في envWait، وEnvFetch فعال.
           - غير ذلك: تُفحص.
         - النسخة المطلوبة (رد GetEnvelope من النظير المختار، على مقعد محجوز قبل الطلب، D137) تُفحص دائمًا ضمن ENV_SOLICIT_SLOTS، فلا تتجاوز سقف الفحوص. إن كان rej(x) = 8 يُطرد أقدم حكم لـx قبل الإدراج. عددها ≤ طلب واحد لكل (x، نظير) في الدورة، و≤ 32 جاريًا عامًا.
         - الامتلاء العام يطرد أقدم حكم إدراجًا، ويزيد envEvicted.
         - التنظيف: عند استيراد x، أو تخزين نسخة envOk لها، أو blockReject، أو إزالتها بحدود الطوابير أو D118، تُحذف كل أحكامها ونسخها المنتظرة.
         - الحكم المطرود قد يُعاد فحصه عند وروده ثانية، بالنتيجة الحتمية نفسها. «كل hdrId مرة واحدة» تسري ما بقي حكمه محفوظًا.
         - الامتلاء لا يكتب blockReject ولا pocol_getRejects، ولا يطلق إلغاء النسل.
         - لِمّة التعافي: لكل x في envWait لدى الصادق i حائز صادق h* لنسخة envOk (D139). التعريفات:
           - t_0 (D140): أول لحظة تتحقق فيها الشروط G1–G4 معًا، وتبقى متحققة حتى القبول:
             - G1: حالة أب x متوفرة لدى i (شرط الإرسال في D137)، وx باقية في envWait بمدخل محلي غير مزال بحدود الطوابير أو D118.
             - G2: h* متصل بـi، ويحوز نسخة envOk ويخدمها.
             - G3 (الاستجابة): كل طلب GetEnvelope من i إلى h* يصدر بعد t_0 يصل آخر بايت من رده خلال RTT_h* < ENV_REQ_TIMEOUT.
             - G4: ثبات مجموعة نظراء i في الفترة (تعريف P أدناه).
             - الزمن قبل t_0 (preCondWait) لا يدخل D_x، ويُسجل قياسًا دون أي ادعاء زمني.
             - انقطاع أي شرط قبل القبول يسقط الضمان. عند عودة الشروط كلها تبدأ لحظة جديدة t_0'، وتُحسب P وQ وB منها.
             - التعريف السابق في D139 (وجود x في envWait واتصال h* فقط) ملغى، لأن D137 يمنع الطلب قبل حالة الأب، ولأن الاتصال لا يضمن الرد ضمن المهلة.
           - P: عدد النظراء المتمايزين الذين يجوز لـEnvFetch اختيارهم لـx خلال الفترة، ومنهم h*.
             - بافتراض ثبات مجموعة نظراء i في الفترة يكون P ≤ 50 (حد P2P).
             - كل اتصال جديد يزيد P بواحد، فلا يُدّعى حد ثابت تحت تبدل غير محدود.
           - Q: أقصى طول لـEnvQueue خلال الفترة شاملًا x، وQ ≤ 16384.
           - B: صفر إن كان h* غير مجرَّب أو reeligible عند t_0، وإلا فالباقي من مهلته الأسية، وB ≤ 60s.
           - عدد الطلبات المطلوبة لـx حتى رد h*، شاملًا الطلب الناجح، ≤ 2P−1. السبب:
             - غير المجرَّبين ≤ P−1.
             - ثم ≤ P−1 نظيرًا سبقوا h* في طابور reeligible.
             - ثم h*.
           - رد كل طلب يُفحص على مقعده المحجوز.
           - (D137، D138) كل دور مقعد يتحرر خلال ≤ ENV_REQ_TIMEOUT + c_env، للأسباب التالية:
             - المهلة تقيس الاستقبال وحده.
             - الفحص بعدها، من الدخول في checking حتى الاستقرار شاملًا انتظار EnvCheckPool، ≤ c_env ولا يُقاطع (D139).
             - الإلغاء لا يقصّر الدور ولا يطيله فوق هذا الحد.
           - الانتظار حتى منح كل طلب ≤ ⌈Q/32⌉·T_role، حيث T_role = ENV_REQ_TIMEOUT + c_env. السبب:
             - المنح محافظ على العمل، فكل نافذة طولها T_role تضم ≥ 32 منحًا ما دام الطابور غير فارغ.
             - أمام x ≤ Q−1 مدخلًا.
           - ثم دور الطلب نفسه ≤ T_role.
           - إذن تأخر القبول بعد t_0 ≤ D_x = (2P−1)·(⌈Q/32⌉+1)·T_role + B.
             - عند P=1 وQ=1 وB=0: D_x = 2·T_role، فالحد يشمل الاستقبال والفحص.
             - الصيغة السابقة (P−1)·⌈Q/32⌉·T_role ملغاة، لأنها أغفلت الطلب الناجح ودوره.
           - فترات انتظار إعادة الأهلية، حين تكون x خارج الطابور بلا نظير قابل للاختيار، محصورة في [t_0, t_0+B]، لأن h* يبقى قابلًا للاختيار بعدها حتى يُطلب.
           - الحد يكبر مع Q، ويبقى محدودًا لأن Q ≤ 16384. الامتلاء لا يمنع القبول ما بقيت G1–G4.
           - الحائز البطيء (D140): إن كان رد h* أبطأ من ENV_REQ_TIMEOUT باستمرار، فكل محاولة تنقضي بـTimeout ويُعاد ضبط substream، فلا يُقرأ الرد المتأخر.
             - لا ضمان قبول عبر المسار المطلوب، وهذا ليس مجرد تأخير محدود بالمهلة الأسية.
             - يبقى القبول ممكنًا فقط عبر gossip إن وُجد مقعد gossip حر، أو عبر حائز آخر يحقق G3.
             - هذه سياسة خدمة: x تبقى في envWait دون blockReject ودون مدخل في pocol_getRejects، وتُعد المهل في envTimeouts.
         - الذاكرة (D137): أحكام 16384·128 + منتظرة 1024·3067 + مقاعد 64·3131 + EnvQueue 16384·64 = 2097152 + 3140608 + 200384 + 1048576 = 6486720 B ≤ ENV_MEM_MAX = 7 MiB (7340032). لا مخزن رد خارج المقاعد.
       - لِمّة الجلب في التصريف:
         - envOk حتمية، لأن envPre بلا حالة وenvOwn على حالة أب ملتزمة. فرد الصادق صالح لدى الطالب.
         - لكل x ∈ Rec حائز صادق لنسخة envOk، مثبت بـHaltPin (validEnv في validation).
         - الاختيار الصارم يبلغه خلال ≤ |H|−1 طلبًا، وكل زوج (x، نظير) ≤ طلب واحد.
         - إذن طلبات GetEnvelope بعد t_avail ≤ |Henv_i|·(|H|−1).
         - كل نسخة مستلمة موجودة قبل t_avail (لِمّة النسخ)، فـenvCheck ≤ Venv_i.
     - HfullDone وHdrCache: لا يُعاد H-full لكتلة محسومة. التخزين بمفتاحين (D129):
       - BlockCache بمفتاح blockHash: أحكام المحتوى الملتزم المكتملة فقط.
       - EnvCache بمفتاح hdrId: أحكام الغلاف.

     التفصيل في validation.
4. التبني بعمق ≤ R_keep.

**قيود الإدخال**
لا RPC يقبل كتلًا أو رؤوسًا أو قوالب. pocol_submitShare يفحص TemplateID مقابل قالب محلي مقبول.

**المجمع**
- len > TX_MAX: −32014.
- chainId مخالف: −32015.
- ممتلئ: −32016، وبعد F: −32016 {final: true}.
- المعاد بعد reorg يُعاد فحصه.

**حدود موارد P2P والعقدة**
- ≤ 50 نظيرًا، والطابور ≤ 256، و≤ 16 لكل نظير.
- المجمع ≤ 64 MiB.
- ذاكرة العقدة دون RPC ≤ 750 MiB، تشمل ذاكرة redb المؤقتة وأحكام الأغلفة ومقاعدها وEnvQueue ≤ ENV_MEM_MAX = 7 MiB (D136، D137). ذاكرة RPC ≤ 192 MiB.
- ASERT ≤ 512 بتًا.
- الحظر بعد 3 مخالفات 1ب أو templateForm في 10 دقائق عبر gossip، وفورًا في المزامنة.

**JSON-RPC القياسية**
- eth_chainId، وnet_version، وweb3_clientVersion.
- eth_blockNumber، وeth_getBlockByNumber/ByHash، وeth_getTransactionByHash، وeth_getTransactionReceipt، وeth_sendRawTransaction.
- eth_call، وeth_estimateGas، وeth_gasPrice، وeth_maxPriorityFeePerGas، وeth_feeHistory (≤ 1024).
- eth_getBalance، وeth_getCode، وeth_getStorageAt، وeth_getTransactionCount.
- eth_getLogs (≤ 1024 وحدود D88)، وeth_getProof (ضمن K_eff، وإلا −32017).
- غير مدعوم: المرشحات وeth_subscribe (−32601). النقل HTTP/1.1 فقط.

ما يتاح لموقع عبر الإضافة مجموعة جزئية تحددها مصفوفة BridgeAuth في browser، لا هذه القائمة. طرق site_* محلية في الإضافة، والعقدة لا تعرفها وترد عليها بـ−32601 لو وصلتها.

**pocol_getHeaders(from, count) (D80، D81)**
RLP list لرؤوس السلسلة القانونية from..from+count−1. الفحص بترتيب ثابت يتوقف عند أول خرق:
- F0: كميات JSON صحيحة ضمن u64 (hex بلا أصفار بادئة). المخالفة أو المعلمة الناقصة أو الزائدة: −32602.
- F1: from < 1: −32018 {fromZero}.
- F2: count < 1 أو > 512: −32018 {countRange}.
- F3: from > head: −32018 {fromAboveHead}.
- F4: from+count−1 > head (u128): −32018 {beyondHead}.

F0–F2 لا تقرأ الحالة. F3–F4 تقرأ head من لقطة الطلب. عند head=0 كل طلب اجتاز F0–F2 يعطي fromAboveHead. النجاح يعيد count رأسًا بالضبط، والعميل يرسل ≤ 14.

**RpcGuard (D87، D88، D90، D92، D93، D94): سياسة خدمة محلية لا قاعدة صلاحية**

الحدود فرضيات للاختبار، وتُعرض في pocol_getParams.rpcLimits.

**(أ) النقل والثابت S1**
- HTTP/1.1 على hyper، دون HTTP/2 ودون pipelining، على 127.0.0.1 فقط.
- طلب واحد في كل لحظة، ولا يُقرأ التالي قبل sendDone للرد السابق.
- S1: لكل اتصال حجز رد واحد وطلب منتظر واحد على الأكثر.
- SO_SNDBUF = SEND_SOCK_BUF = 32 KiB صريح لكل اتصال، فيتعطل الضبط التلقائي. فرق القبول عن القراءة لا يُفترض له حد، ويُقاس في RG14(0) والتتبع، ولا يدخل أي معيار (D93).

**(ب) جدول التصنيف (أسوأ رد مرمّز)**

خفيف (LIGHT_RESP_MAX = 512 KiB):
- ≤ 4 KiB:
  - eth_chainId، وnet_version، وweb3_clientVersion، وeth_blockNumber.
  - eth_gasPrice، وeth_maxPriorityFeePerGas، وeth_getBalance، وeth_getStorageAt، وeth_getTransactionCount، وeth_sendRawTransaction.
  - pocol_getTiming، وpocol_getTarget، وpocol_getCaps، وpocol_syncStatus، وpocol_getPot، وpocol_getSettlement.
  - pocol_getBurnTotals، وpocol_getEscrow، وpocol_getRetention، وpocol_getClosing، وpocol_getRpcStats، وpocol_submitShare.
- eth_getCode: ≤ 2·B_code_max+1 KiB ≈ 65 KiB.
- eth_getTransactionByHash: ≤ 2·TX_MAX+2 KiB ≈ 258 KiB.
- eth_getTransactionReceipt: ≤ 2·LOG_BLOCK_MAX+546·512 B+2 KiB ≈ 339 KiB.
- eth_getBlockBy* بالهاشات: ≤ 2 KiB+(BODY_MAX/85)·70 B ≈ 160 KiB.
- pocol_getHeaders بـcount ≤ 64: ≤ 64·(2·3067+16) B ≈ 385 KiB.
- pocol_getParams: ≤ 2·|genesisPre|+4 KiB، أي ≤ 92 KiB في end/full.
- pocol_getAssignment وpocol_getMinerSet وpocol_getTemplate: ≤ 1024·160 B = 160 KiB.

ثقيل (RESP_MAX = 8 MiB): eth_getLogs، وpocol_getLogs، وeth_call، وeth_estimateGas، وeth_getProof، وeth_feeHistory، وpocol_getHeaders بـcount > 64، وeth_getBlockBy* بمعاملات كاملة، وpocol_getRejects.

الطريقة غير المدرجة: −32601 قبل القبول، وإضافتها تتطلب سطرًا في الجدول واجتياز RG13. تجاوز الحد أثناء الترميز: −32020 {resultBytes} مع تحرير سليم (RG12).

**(ج) الميزانيات العالمية (لكل العقدة)**
- RPC_CONN_MAX = 32، والزائد يُغلق دون قراءة.
- التصاريح: HEAVY_ACTIVE = 4، وHEAVY_QUEUE = 16، وLIGHT_ACTIVE = 16، وLIGHT_QUEUE = 32. الطوابير FIFO، والصادق ينتظر خلف ≤ 31 طلبًا.
- دلو رموز لكل اتصال بمعدل 20/s وسعة 20:
  - المنفرد يستهلك رمزًا، والدفعة k رموزًا عند التحليل.
  - النقص يعطي −32021 {rate, retryAfterMs} دون تنفيذ. للدفعة مصفوفة k أخطاء ≤ 256 B تُكتب من CONN_BUF.
  - لا انتظار للرموز داخل طلب أو دفعة.
- ذاكرة الرد: HEAVY_RESP_MEM = 48 MiB، وLIGHT_RESP_MEM = 16 MiB = 32·512 KiB، فحجوزات الخفيف لا تنفد (S1).
- SCRATCH: ≤ 16 MiB لكل تصريح ثقيل و≤ 1 MiB لكل خفيف. الحساب متزامن دون await في خيط من مجمع ثابت (20 خيطًا) بعداد خيطي، والتجاوز −32020 {scratch}.
- REQ_BUF = 1 MiB، وCONN_BUF = 64 KiB لكل اتصال.
- RPC_MEM_TOTAL = 48+16+4·16+16·1+32·(1+1/16) = 178 MiB ≤ 192 MiB.
- الترميز في قائمة مقاطع 64 KiB داخل الحجز، دون إعادة تخصيص بالمضاعفة.
- مخازن النواة (32 KiB لكل اتصال) خارج العد، وتُقاس ضمن RSS، ومنها البايتات المقبولة غير المقروءة بعد sendDone.
- اللقطات ≤ 20، أي لقطة لكل تصريح.

**(د) دورة ملكية الطلب (D88)**
1. القراءة:
   - تبدأ من أول بايت. الرأس والجسم ≤ REQ_BUF خلال REQ_READ_DEADLINE = 10s، وإلا إغلاق.
   - الخامل بين الطلبات يُغلق بعد IDLE = 30s، محسوبة من sendDone للرد السابق أو من فتح الاتصال.
2. التحليل: F0، وحجب الأصل (ح2)، والتصنيف، وحد المعدل، وفحص المدى الرقمي للسجلات. كلها داخل REQ_BUF بلا حجز ولا لقطة.
3. القبول الذري: التصريح وحجز الحد الأعلى للفئة معًا لرأس الطابور.
   - المنتظر لا يحتجز شيئًا.
   - خلال QUEUE_WAIT = 2s، وإلا −32021 {busy, retryAfterMs ∈ [250, 2000]}.
4. الحساب: يملك {تصريحًا، وحجزًا، ولقطة، وSCRATCH}، والترميز في الحجز. COMPUTE_DEADLINE من القبول شاملًا الترميز: 5s ثقيل و500 ms خفيف، وانتهاؤها −32020 {deadline}.
5. نهاية الحساب: تحرير اللقطة وSCRATCH والتصريح، وتقليص الحجز إلى الحجم الفعلي.
6. الإرسال بضابط SendPacer (D90، D92، D93، D94):
   - P = البايتات التي سلّمها الخادم للضابط (written)، وA = البايتات التي قبلها مقبس الخادم، وA ≤ P.
   - pending صحيح عند P > A.
   - sendTime ساعة تتقدم فقط عند pending، وتبدأ مع أول بايت مُسلّم.
   - R = MIN_SEND_RATE = 65536 B/s، وGRACE = 2s.
   - القاعدة الملزمة الوحيدة: إغلاق إن كان sendTime > GRACE وA < R·sendTime.
   - الرصيد = A − R·sendTime، ولحظة الخرق s_v = max(GRACE, A/R).
   - الإنفاذ:
     - مؤقت على sendTime = s_v+TIMER_EPS، حيث TIMER_EPS = 1 ms يحسم التساوي لأن القاعدة صارمة.
     - يُعاد ضبط المؤقت مع كل تغير في A أو pending، مع فحص احتياطي كل 50 ms، وإعادة فحص بالقيم الدقيقة عند الإطلاق.
     - lateness ≤ ENFORCE_SLACK = 200 ms، ويُسجل في pacerLateMaxMs.
   - مبرهنة الحد:
     - طوال بقاء الاتصال sendTime ≤ max(GRACE, A/R)+ENFORCE_SLACK وA ≤ size.
     - إذن SEND_MAX(size) = GRACE+size/R+ENFORCE_SLACK. رد 8 MiB → 130.2s، ورد 512 KiB → 10.2s.
     - الرصيد لا يرفع الحد.
   - CloseOK: كل إغلاق بالمعدل يحقق sendTime > GRACE، وA < R·sendTime، وsendTime − s_v ≤ ENFORCE_SLACK، ويُسجل (sendTime، A) في RpcStats.
   - PacerTrace (D93):
     - rpc.pacerTrace=true، متاح في بناء الإصدار.
     - يسجل سطرًا لكل تغير في P أو A أو pending وعند الإغلاق وsendDone: {conn، t_ns (رتيبة)، sendTime_ns، P، A، pending، event ∈ {produce, accept, pendStart, pendStop, close, sendDone}}.
     - لا يغير القرارات ولا الإجماع.
     - مخزن ثابت 4 MiB، والفيض في traceDropped، وtraceDropped > 0 يجعل التشغيل غير حاسم.
   - sendDone (D94): أول لحظة يكون فيها A = P = size بعد تسليم كل بايتات الرد (أو الدفعة كاملة مع ']'). عنده:
     - ينتهي Pacer.write بـDone، ويُحرر الحجز.
     - تتوقف sendTime نهائيًا، ويُسجل حدث sendDone، وتبدأ IDLE.
   - receiveDone: لحظة Rc = size لدى العميل.
     - لا يراها الخادم، ولا تؤخر تحرير الحجز، ولا تدخل أي مهلة أو قاعدة.
     - تُقاس في سجل العميل وPacerSim فقط.
     - البايتات بين A وRc عند sendDone (unreadAtSendDone) في مخازن النواة لا في الحجز.
   - الحجز يُحرر عند sendDone أو عند الإغلاق، أيهما أسبق.
7. الطلب التالي لا يُقرأ قبل sendDone.

المهل المستقلة الوحيدة: REQ_READ_DEADLINE، وIDLE، وQUEUE_WAIT، وCOMPUTE_DEADLINE، وقاعدة SendPacer، وBATCH_WALL. كلها تقيس أحداث الخادم، ولا تقيس استلام العميل.

**(هـ) الدفعات (D88، D90، D92)**
- ≤ BATCH_MAX = 20 عنصرًا، متدفقة بـchunked: يُكتب '['، ثم كل عنصر يمر بالمراحل 3–5 ثم يُسلّم رده للضابط.
- العنصر k+1 يبدأ القبول فور قبول المقبس آخر بايت من العنصر k. لذلك قد يسبق الخادم قراءة العميل بما يصل إلى سعة مخزن المقبس.
- بين العناصر لا تصريح ولا لقطة ولا حجز، وكل عنصر يعيد القبول من آخر الطابور.
- SendPacer واحد للدفعة: sendTime واحدة تبدأ مع '['، وتتوقف دون pending (انتظار الطابور والحساب)، وGRACE مرة واحدة، والقاعدة على A التراكمية، ولا مهلة ولا سماح لكل عنصر.
- sendDone للدفعة عند قبول آخر بايت من الإطار الختامي. حجز كل عنصر يُحرر عند قبول آخر بايت منه.
- batchSent = كل البايتات المسلّمة. العنصر الذي يجعل batchSent > BATCH_RESP_MAX = 16 MiB يُحسب ثم يُستبدل بـ−32020 {batchBytes}، وما بعده كذلك دون تنفيذ. كل خطأ ≤ ERR_ITEM_MAX = 256 B من CONN_BUF.
- BATCH_BYTES_HARD = 16 MiB+20·256+64 = 16782400 B.
- الحدود المشتقة:
  - BATCH_SEND_MAX = 2+256.0791015625 = 258.0791015625s، وحد المراقبة 258.2791015625s.
  - الانتظار ≤ 20·(2+0.2) = 44s، والحساب ≤ 20·(5+0.1) = 102s.
  - المجموع ≤ 404.2791015625s.
- BATCH_WALL = 420s حد احتياطي من نهاية القراءة حتى sendDone، وR26(و) تشترط ≥ 404.2791015625. بلوغه يغلق ويحرر خلال ≤ 100 ms ويزيد batchWallHits، وأي قيمة غير صفرية في الإصدار عيب.
- عنصر busy أو rate يُكتب خطؤه ويُتابع. الترتيب محفوظ، والمعرفات منسوخة. الانقطاع وسط الدفعة يعطي JSON ناقصًا لا يُحلل.

**(و) الإلغاء**
رمز يُضبط عند الإغلاق أو أي مهلة، ويُفحص كل 16 كتلة ممسوحة وكل 1 MiB مرمّز وكل 10^6 gas. يحرر كل الموارد خلال ≤ CANCEL_MAX = 100 ms.

**(ز) eth_getLogs**
الترتيب:
1. F0 (−32602).
2. المدى الرقمي ≤ 1024، وإلا −32020 {range, maxRange: 1024} دون قبول.
3. القبول.
4. حل 'latest' و'safe' و'finalized' من اللقطة ثم فحص المدى.
5. مسح تصاعدي.

حد P1:
- تُعالج الكتلة x كاملة.
- إن جعلت إضافتها الرد > RESP_MAX، أو العدد > LOGS_COUNT_MAX = 10000، أو المسح > LOGS_SCAN_BYTES = 32 MiB، فالنتيجة −32020 {reason ∈ {resultBytes, resultCount, scanBytes}, fromBlock, lastCompleteBlock = x−1, nextFromBlock = x}.

حد الموارد: المهلة أو SCRATCH تعطي −32020 {deadline|scratch, fromBlock, lastCompleteBlock ≥ fromBlock−1, nextFromBlock = lastCompleteBlock+1}، دون افتراض تقدم.

المخزن الجزئي يُهمل كله.

لِمّة P1: كتلة واحدة ≤ 32 KiB خامًا و≤ 546 سجلًا، وترميزها < 400 KiB. إذن عند P1 يكون lastCompleteBlock ≥ fromBlock.

لِمّة P3: حد P1 يُرفع عند lastCompleteBlock+1 فقط، والترميز حتمي، فكل مدى جزئي من [fromBlock, lastCompleteBlock] لا يرفع P1 على السلسلة نفسها. قد يرفع حد موارد فقط.

مرشح blockHash = كتلة واحدة.

**(ح) pocol_getLogs(filter, anchorHash) (D89)**
- ثقيلة، بخرج eth_getLogs وحدوده و−32020 نفسها. fromBlock وtoBlock رقميان فقط، والوسوم −32602.
- فحوص اللقطة:
  - canonicalHash(height(anchorHash)) ≠ anchorHash يعطي −32022 {anchorNotCanonical, head}.
  - toBlock > height(anchorHash) يعطي −32022 {beyondAnchor}.
- لِمّة P2: blockHash يلتزم بالأب تعاقبيًا. فإن بقي anchorHash قانونيًا في لقطتين، تطابقت الكتل القانونية ≤ height(anchor) فيهما، فكل القطع الناجحة بالمرساة نفسها من فرع واحد.
- بلا حالة جلسة، ولا تغير الإجماع.

**(ح2) حجب الأصل في العقدة (D95، دفاع ثانٍ)**
- ORIGIN_DENY = {pocol_submitShare، pocol_getTemplate}.
- إن حمل الطلب ترويسة Origin بأي قيمة مسموحة، وكانت الطريقة في ORIGIN_DENY، يُرد −32601 في المرحلة 2 دون قبول ولا لقطة.
- في الدفعة يُطبق لكل عنصر.
- برامج التعدين المحلية لا ترسل Origin. eth_sendRawTransaction تبقى مسموحة لأصل الإضافة لأن WalletSubmit يحتاجها، وحمايتها في BridgeAuth.
- لا يحمي من عملية محلية تحذف Origin (A10)، ولا يدخل الصلاحية.

**(ط) الفصل عن الإجماع**
- RPC في runtime tokio بخيطين مع مجمع 20 خيطًا. التحقق والتعدين والمزامنة في runtime آخر.
- لقطات redb لا تحجز الكاتب.
- تغيير RpcGuard أو PacerTrace أو ORIGIN_DENY لا يغير الصلاحية ولا يتطلب fork.

**واجهات خاصة أخرى**
- pocol_getTemplate، وpocol_submitShare (واجهتا تعدين محليتان، محجوبتان مع Origin).
- pocol_getAssignment(h, a) → {seed, nMax, nonceMode, members[{id, start, end}]}.
- pocol_getMinerSet، وpocol_getCaps، وpocol_syncStatus.
- pocol_getTiming → {L0, U0, Tj, bj, fsmState}.
- pocol_getTarget(h) → {deltaT, e, s, f, F, target, work, saturated ∈ {none, high, low}}.
- pocol_getParams → {profileKind, nodeKind, nonceMode, chainId, genesisHash, genesisPre, netKey, forkSchedule, activeVersion, rpcLimits}.
- pocol_getPot(j) → {status, amount, kind, winner|null, blockHash, settlesAt}، وإلا −32013. winner = null حتى تُستورد الكتلة j+1 على الفرع القانوني (D141).
- pocol_getSettlement(j): سلاسل wei عشرية، وغير المسوى −32013 {settlesAt}.
- pocol_getBurnTotals.
- pocol_getEscrow → {balance, pendingSum, heldTotal, burnedTotal, surplus}.
- pocol_getRetention → {complete, fromHeight, replicasConfigured, replicasLive, checkedAt} (D119).
  - ReplicaMonitor يفحص كل 10s قائمة نسخ محلية (منها العقدة نفسها).
  - النسخة حية إن ردت خلال 2s، وcomplete=true، وhead ≥ head المحلي−6.
  - معلوماتي لا يدخل الصلاحية. يُعتمد في LN، ولا يُعتمد في RP.
- pocol_getClosing → {closing, closeH, cursor, recLen, hEnd, finalAt|null}.
- pocol_getRejects: آخر 256 بالشكل {hash, rule, peer, path}.
- pocol_getRpcStats → {conns, heavyActive, heavyQueued, lightActive, lightQueued, heavyResv, lightResv, scratchHeavyMax, scratchLightMax, snapshotsOpen, maxResPerConn, cancelled, cancelMaxMs, busyCount, rateCount, limitCount, anchorRejects, originDenied, pacerClosed, pacerLateMaxMs, lastPacerClose{sendTime, accepted}, sendDoneCount, sendOverrunMaxMs, batchSendMaxS, batchWallMaxS, batchWallHits, traceDropped}.
  - sendOverrunMaxMs = أقصى (sendTime عند sendDone − (GRACE+size/R)) بالمللي ثانية.
  - originDenied عدد ردود ح2.

**RPC المحلي**
- 127.0.0.1، وHost غير مسموح يعطي 403، وOrigin غائب أو chrome-extension://<ID>.
- الجسم ≤ 1 MiB، والدفعة ≤ 20، وeth_call ≤ 30M gas.
- كل الطلبات عبر RpcGuard، وطرق ORIGIN_DENY لا تُخدم مع Origin.

**الأخطاء**
- القياسية: −32700، و−32600، و−32601، و−32602، و−32603، و−32000.
- الخاصة:
  - −32010 قالب، و−32011 نطاق، و−32012 حصة، و−32013 مجهولة أو غير مسواة.
  - −32014 حد، و−32015 نوع أو chainId، و−32016 سقف أو منتهية، و−32017 خارج نافذة trie.
  - −32018 معلمات (إقلاع وأسباب pocol_getHeaders).
  - −32019 {rule}: سجل العميل. rule ∈ البنود 1–9، أو net*، أو viewIncomplete، أو viewGenesis، أو viewNoBlocks، أو viewFuture، أو viewTargetCeil، أو recvFit (D100)، أو gs*.
  - −32020 {reason ∈ {range, resultBytes, resultCount, scanBytes, deadline, scratch, batchBytes}, fromBlock?, lastCompleteBlock?, nextFromBlock?}.
  - −32021 {reason ∈ {busy, rate}, retryAfterMs}.
  - −32022 {reason ∈ {anchorNotCanonical, beyondAnchor}, head}.
- الإضافة نحو الإطار:
  - EIP-1193: 4001، و4100، و4200 {method} (طريقة غير مسموحة، D95)، و4901.
  - محلية للجسر (D96، D97): −32600 {reason ∈ {size, kind}?}، و−32602 {path}، و−32005 {reason ∈ {rate, pending, nav, busy, store}} (رمز EIP-1474)، و4300 {reason: quota, limit} (غير قياسي، معلن)، و−32603 {reason ∈ {transport, recvLimit, recvDepth, recvParse, storeTimeout}}.

**مستويات الثقة (D13)**
- DEV: anvil.
- LN (الأساسي): تحقق كامل.
- RP: نافذة D77 وD82 مع D80 وD81 وD84. البعيد قد لا يطبق D88–D94 ولا ح2.
  - الإضافة تحمي نفسها بالجسر وBridgeAuth وLogClient ومهلة 10s وRecvGuard (D98) وجلسة الموقع (D99).
  - حدود RpcGuard تحمي العقدة لا الإضافة.

الملف وبصمته من RPC نفسه لا يثبتان الصحة إلا في LN.
