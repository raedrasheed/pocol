# المساهمة والمكافآت

**من الأطروحة**
- R_tot = subsidy+fees [PDF:103 (5.6)]، وΣf = 1 [(5.7)].
- المساواة تحفز Sybil [PDF:103].
- التقريب الضئيل مقبول [PDF:104].
- النشاط ليس إثبات بحث [PDF:105].
- الإزالة بالخمول دون عقوبة سند [PDF:89، PDF:112].

**الحصة (D6)**
- عنصر shareList يحقق البندين 9 و14، حيث T_share = min(2^256−1, target·m).
- nonce الفائز حصة ضمنية.
- (D142) c_i وC_j وlastShare تُشتق في recordShares(j) من ParentResult(j) للكتلة j على الفرع المنفذ. إذن التسوية دالة على shareList الملتزمة للكتلة j نفسها، لا على أول نسخة رآها المعدّن أو العقدة.
- الحصص عينة عمل لا إثبات مسح، وκm = 64.
- pocol_submitShare واجهة تعدين محلية فقط. لا يصل إليها موقع عبر الجسر (D95)، ولا طلب يحمل Origin.

**الأدلة**

النوع 2 (D29): RLP([2, TemplateID, nonce, shareRoot1, sig1, shareRoot2, sig2]).
- يصح بشروط: اختلاف الجذرين، وتوقيع owner(nonce) على الاثنين، ووجود كتلة سلف بـj ≥ h−E_win، وللمتهم سجل مع tomb=0.
- أثره قبل settle(j): إبطال pot_j وحرقه.

النوع 1: يصح بشروط:
- UT1 ≠ UT2، وكلاهما موقّع عند a=0 بنفس (parentHash, h) وهوية cfg.
- h ≥ h_incl−E_win.
- الموقّع هو DrawIndex[h mod E_win].
- للمتهم سجل مع tomb=0.

الدليل المخالف لشروطه يبطل الكتلة الحاملة.

**السجل**
Rec[id] أربع كلمات:
- w0: status، وtomb، وbondState، وrecPos، وactPos، وlastShare.
- w1: rewardAddr.
- w2: bond.
- w3: exitHeight، وdueHeight، وburnHeight، وgraceStart.

أعضاء M_0 يُكتبون في genesis (D79).

**التفويض (D62)**
- id = msg.sender في كل الدوال، عدا removeInactive المفتوحة.
- calldata بطول 4 أو 36 بايتًا بالضبط، بلا fallback ولا receive.

**آلة السند**
- register بقيمة B_reg: active وHELD.
- deregister وremoveInactive: من active إلى exited، مع dueHeight = h+U.
- reactivate: من exited وHELD وtomb=0، بعد W_act.
- claimBond أو الكنس: بعد dueHeight وخارج الإغلاق، عبر PAY، ثم حذف السجل.
- مؤشر الإغلاق: من HELD إلى PAID، بمعدل ≤ RET لكل كتلة.
- الدليل:
  - ضد سجل نشط: BURNED وtomb=1، ويُخرج النشط.
  - ضد PAID: يكتب tomb=1 فقط.
  - ضد قبر أو هوية بلا سجل: يبطل الكتلة.
- حذف القبر بعد burnHeight+E_win+D+1.

ترتيب الأخطاء: BadCalldata، ثم BadValue، ثم Closing، ثم RegOpsLimit. عند R_max=0 يعطي register خارج الإغلاق RegOpsLimit، وداخله Closing.

**الكنس وحد السجل (D61، D86)**
- ≤ 64 خطوة و≤ 16 حذفًا لكل كتلة.
- LAG = 2048: ثابت بروتوكول، وأقصى تأخر للكنس.
- Lmax = max(U, 2·E_win+D+1): أقصى عمر لسجل غير نشط قبل أهلية حذفه.
- R16: REG_RECORDS_MAX ≥ M_max+R_max·(Lmax+LAG).
- network: Lmax = 4113، والحد 256+2·6161 = 12578 ≤ 16384.

**الرسوم (D68)**
- blockTips = Σ gasUsed·effectiveTip لمعاملات المرحلة 1.
- effectiveTip = min(maxPriority, maxFee−baseFee) للنوع 1559، وgasPrice−baseFee لغيره.
- base fee يُحرق خارج Escrow، ولا يدخل burnedTotal.

**الالتزام والتسوية (D78، COINBASE = Escrow)**
- pot_j = subsidy+blockTips_j، ويُكتب في المرحلة 2 من الكتلة j في pending[j] = {amount، kind ∈ {signed, fallback}، proposer}. القيم الثلاث معروفة من UT والجسم قبل التعدين.
- الحقل winner يبدأ صفرًا، ويكتبه recordResult(j) في المرحلة 0 من الكتلة j+1 بمالك nonce الكتلة j (D141).
- لِمّة التسجيل: R8 يفرض D ≥ 2، فـj+1 ≤ j+D−1، وكل settle(j) يقرأ winner مسجلًا.
  - قراءة winner = 0 في settle تأكيد نظام يبطل الكتلة، ولا تقع على سلسلة صالحة.
  - الأثر في الاحتياطي: propPart يذهب إلى winner المسجل.
- الكتلة F: لا تُنشأ pending[F]. recordResult(F−1) يسبق finalize في F، فتُسوى pending[F−1] بفائزها.
- دليل النوع 2 على الكتلة j لا يحتاج winner المسجل، لأنه يبطل pot_j ويحرقه.
- settle(j) في الكتلة j+D يقرأ pending[j] ويحذفها، وإعادة التنظيم تعيدها.

صيغ settle:
- invalid: invalidBurn = pot.
- normal:
  - winnerPart = floor(pot·α_bp/10^4).
  - propPart = floor(pot·γ_bp/10^4)، للمقترح الموقّع، أو للفائز في الاحتياطي.
  - R = pot−winnerPart−propPart، وQ = max(C_j, κm).
  - p_i = floor(R·c_i/Q)، وA = floor(R·C_j/Q).
  - حصة القبر إلى penaltyBurn، وحصة الحساب الفارغ إلى payBurn.
  - baseBurn = R−A، وroundBurn = A−Σp_i ≤ k_j−1.
  - الهوية: pot = paidTotal+payBurn+penaltyBurn+roundBurn+baseBurn.
- F: subsidy_F = 0، وfinalBurn = blockTips_F، والكتلة F تسوي كل المتبقي.

التجميع:
- burnedTotal خانة واحدة، والحرق يبقى رصيدًا في Escrow.
- الفائض = balance−(Σpending+heldTotal+burnedTotal) ≥ 0.

حد الإصدار (D85): عدد الكتل ≤ H_END، وF بلا إعانة، فالإعانات ≤ subsidy·(H_END−1).

**ExecFixture (κm=64، α_bp=1000، γ_bp=500)**
- SV1: R=64000 و{w:1} → A=1000، وbaseBurn=63000.
- SV2: R=101 و{w:1، و64 هوية بحصة 1} → A=101، وΣp=65، وroundBurn=36.
- SV3: R=1000 و{w:3، x:2} → p=46 و31، وroundBurn=1، وbaseBurn=922.
- SV4: R=6400 وt قبر → paidTotal=100، وpenaltyBurn=100، وbaseBurn=6200.
- SV5: مثل SV4 لكن t بلا حساب → payBurn=100.
- SV6: pot باطل → invalidBurn=5000.
- SV7: pot=80000 احتياطي وC=1 → 8000 و4000، وR=68000، وA=1062، وbaseBurn=66938، وpaidTotal=13062.
- SV7s: pot=164000 موقّع وC=1 → 16400 و8200، وR=139400، وA=2178، وbaseBurn=137222، وpaidTotal=26778.
- SV10a (D116): R=64000، وc_w=1، وC_full=128 (w و127 هوية بحصة 1)، وC_kept=1. الكامل: p_w=500، وA=64000، وroundBurn=0، وbaseBurn=0. المستبعد: p_w=1000، وA=1000، وbaseBurn=63000. g_dir=500 < 501.
- SV10b: C_full=64 → p_w=1000 في الحالتين، وg_dir=0.
- SV10c: C_full=65 → 984 مقابل 1000، وg_dir=16 < 16.39.
- في SV10a–c يكون w ممثلًا بـν وحده (c_w=1)، والهويات الأخرى حصصها صريحة.
- SV10d (D120): R=64000، وE_full = حصتان صريحتان لـw و127 حصة لغيره، فـC_full=130، وc_w^full=3، وp_w=1476.
  - ownSharesOnly: C_kept=3، فـp_w=3000، وg_dir=1524 < 1524.08.
  - emptyShares: C_kept=1، فـp_w=1000، وg_dir=−476.
- SV10e (حد S_max): R=64000، ووصلت 300 حصة صريحة لغير w، فتُقص إلى 256 بأصغر nonce. إذن C_full=257، وc_w=1، وp_i=249 لكل حصة، وΣp=63993، وroundBurn=7.
  - ownSharesOnly: p_w=1000، وg_dir=751 < 751.98.
- SV10f: shareList بـ257 عنصرًا تبطل الكتلة بالبند 1. وإضافة ν صراحة إلى القائمة يرفضها البند 9.

**ChainFixture**
- SV8: Δburned(F)=1155008، وw +208992.
- SV8-fb: 1071008.
- SV9: pot_30=253015.
- SV9-final: 1244023.
- SV8-reorg: 1208230 منذ 198.
- SV9-reorg: pending[30]=80000.

**أثر الاستبعاد (T7c، D116)**

الرموز:
مجموعات الحصص (D120)، لكل ارتفاع j فاز به w:
- ν: nonce الفائز. حصة ضمنية دائمًا خارج shareList، تُحسب في C_j وفي حصة مالكها. إذن C_j = 1+|shareList_j|، وc_i يعد ν لمالكه (متسق مع SV1).
- E_full: الحصص الصريحة الصالحة للارتفاع j (البندان 9 و14، وكلها ≠ ν) التي وصلت الفائز قبل نشر رأسه.
  - تُقص إلى S_max بأصغر nonce، وهذه سياسة البناء الصادقة.
  - ν لا يدخل القص ولا يشغل مقعدًا فيه.
- E_kept: محتوى shareList الفعلي، وطوله ≤ S_max بالبند 1.
- C_full = 1+|E_full| ≤ S_max+1 = 257، وC_kept = 1+|E_kept|.
- c_w^full = 1 + حصص w الصريحة في E_full، وc_w^kept = 1 + حصص w الصريحة في E_kept.
- إذن c_w ≥ 1 دائمًا، والقائمة الفارغة تعطي C_kept = c_w^kept = 1.

الأثر:
- المكسب المباشر العام: g_dir = floor(R·c_w^kept/max(C_kept, κm)) − floor(R·c_w^full/max(C_full, κm)).
  - winnerPart وpropPart لا يعتمدان على الحصص.
  - قد يكون g_dir سالبًا.
- الذراع ownSharesOnly: E_kept = حصص w الصريحة الواردة في E_full. إذن c_w^kept = c_w^full = c_w، وC_kept = c_w ≤ C_full. عندئذ:
  - g_dir = 0 إن كان C_full ≤ κm، لأن المقامين κm.
  - وإلا 0 ≤ g_dir < R·c_w·(1/max(C_kept, κm) − 1/C_full)+1 ≤ R·c_w·(1/κm − 1/C_full)+1.
  - الحد مقصور على ثبات c_w، ولا يُطبق على قائمة تضيف حصصًا لـw غير واردة في E_full.
- الذراع emptyShares: E_kept = ∅، فـc_w^kept = 1 وQ = κm.
  - g_dir = floor(R/κm) − floor(R·c_w^full/max(C_full, κm)) يُحسب بالضبط، دون حد آخر.
  - يساوي ذراع ownSharesOnly فقط عند c_w^full = 1.
- الأثر غير المباشر: الحصص المستبعدة لا تدخل w(m) في Draw، ولا تحدّث lastShare للمستبعدين، فقد ترتفع حصة الخصم من الاقتراح وpropPart لاحقًا. لا حد تحليلي له، ويقيسه g النسبي في T7c(S)، وحد 2% فرضية قابلة للفشل.

**ثوابت T17 لكل كتلة**
- الحفظ، وroundBurn ≤ k−1، وpot−subsidy = tips، وEscrow متوازن.
- PAID مرة واحدة على الأكثر.
- pending[j] موجودة لكل j ∈ [h−D+1, h] غير نهائي.
- (D141) pending[j].winner ≠ 0 لكل j ∈ [h−D+1, h−1]، وpending[h].winner = 0 في نهاية الكتلة h.
- heldTotal(0) = |M_0|·B_reg، وh ≤ H_END.

PAY: 5000 gas، ولا يفشل، ولا ينفذ كودًا.

خدمة RPC بلا مكافأة، وكلفتها على المشغل.

**معلن**
- لا مكافأة مباشرة لفائز F.
- الحجب الأناني غير محلول (T10).
- reward_mode=equal للمقارنة فقط [PDF:103 (5.9)].
