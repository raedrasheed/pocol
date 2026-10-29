# PoCol — Proof-of-Collaboration (إثبات التعاون)

> **English summary.** PoCol is a coordinated proof-of-work consensus protocol proposed by **Raed S. Rasheed** in the PhD thesis
> *"Proof-of-Collaboration: A Novel Consensus Algorithm with Highly Efficient Power Consumption"* (Islamic University of Gaza,
> Faculty of Engineering, supervised by Prof. Aiman AbuSamra, 2026; the thesis PDF is in this repository).
> Miners keep PoW's hash-target puzzle and cumulative-work chain, but every round uses one canonical block template and a
> nonce-free header template. A deterministic seed splits the nonce domain into disjoint ranges among the registered
> miners, so honest miners never hash the same candidate header in a round.
> This repository holds the specification and the experimental implementation:
> - `pocold`, a Rust node with an EVM (revm, Cancun), P2P, mining, persistence and JSON-RPC;
> - an execution layer with system contracts;
> - on-chain website storage;
> - a Chrome MV3 extension that verifies and displays on-chain sites.
>
> **Status:** the network runs **for experiments and local devnets only (simnet)**. It is not production-ready, and it is not
> a security audit. See [Current status](#الحالة-الحالية-وحدود-ما-ثبت) below.

---

## المحتويات

1. [ما هو PoCol؟](#ما-هو-pocol)
2. [صاحب الفكرة والأصل العلمي](#صاحب-الفكرة-والأصل-العلمي)
3. [كيف يعمل PoCol](#كيف-يعمل-pocol)
4. [كيف طُوّر المشروع](#كيف-طوّر-المشروع)
5. [هيكلية المستودع والكود](#هيكلية-المستودع-والكود)
6. [المتطلبات](#المتطلبات)
7. [التشغيل السريع: شبكة تطوير محلية](#التشغيل-السريع-شبكة-تطوير-محلية)
8. [نشر موقع وقراءته من المتصفح](#نشر-موقع-وقراءته-من-المتصفح)
9. [واجهة JSON-RPC](#واجهة-json-rpc)
10. [الاختبارات](#الاختبارات)
11. [التطوير والمساهمة](#التطوير-والمساهمة)
12. [الحالة الحالية وحدود ما ثبت](#الحالة-الحالية-وحدود-ما-ثبت)
13. [الترخيص](#الترخيص)

---

## ما هو PoCol؟

PoCol بروتوكول إجماع لسلاسل الكتل، مشتق من إثبات العمل (PoW). في PoW التقليدي يجزّئ كل معدّن رؤوس كتل خاصة به في تنافس
مستمر، فيتكرر الجهد ويُستهلك قدر كبير من الطاقة. PoCol يبقي من PoW ما يعطيه أمانه، أي شرط الهدف (hash ≤ target)
والسلسلة ذات العمل التراكمي الأكبر. ويغيّر طريقة تنظيم العمل:

- **قالب واحد للجولة:** يعمل كل المعدّنين على قالب كتلة قانوني واحد وقالب رأس بلا nonce، ولا يتغير بين المرشحين إلا
  حقل الـnonce.
- **نطاقات منفصلة:** إجراء حتمي قائم على بذرة يقسّم نطاق الـnonce إلى أجزاء متباينة بين المعدّنين المسجلين. لا يجزّئ
  معدّنان نزيهان المدخل نفسه في الجولة نفسها.
- **مكافأة تعاونية:** تُوزَّع المكافأة على الفائز وعلى من قدّم «حصصًا» (shares)، وهي nonces تحقق هدفًا أسهل، تدل على
  العمل المبذول.
- **دورة حياة للعضوية:** تسجيل المعدّنين بسند، وخروج موقّع، وإزالة بالخمول، ومعدّنون احتياطيون.

فوق الإجماع بنى المشروع طبقة تنفيذ متوافقة مع EVM، وتخزينًا لمواقع الويب على السلسلة، وإضافة Chrome تعرض هذه المواقع
بعد التحقق منها. هذه الطبقات **إضافات منتج** على فكرة الأطروحة، وليست جزءًا منها.

## صاحب الفكرة والأصل العلمي

- **صاحب الفكرة:** المهندس **رائد سعيد خليل رشيد (Raed S. Rasheed)**.
- **الأصل:** أطروحة دكتوراه في هندسة الحاسوب، الجامعة الإسلامية بغزة، كلية الهندسة، بإشراف **أ. د. أيمن أبو سمرة
  (Prof. Aiman AbuSamra)**، 2026:
  *Proof-of-Collaboration: A Novel Consensus Algorithm with Highly Efficient Power Consumption*
  (إثبات التعاون: خوارزمية توافق جديدة ذات كفاءة عالية في استهلاك الطاقة).
- **نص الأطروحة:** [PhD-thesis-Raed-Rasheed-120211566-61.pdf](PhD-thesis-Raed-Rasheed-120211566-61.pdf).

ما قيس في الأطروحة كان بالمحاكاة (BlockSim موسّع بنمذجة الطاقة)، وليس على شبكة موزعة:

- استُخدم 20 معدّنًا منطقيًا: 16 نشطًا و4 احتياطيين، ونطاق nonce من 1,600 قيمة، لكل معدّن 100 قيمة.
- احتفظت أفضل سياسة موجهة للإنتاجية بنسبة **90.39%** من الكتل المقبولة، مع خفض محسوب للطاقة بنسبة **55.05%**.
- في مقارنة مطابقة مع PoW انخفضت الطاقة أيضًا، لكن مع إنتاج كتل أقل وزمن جولة أطول. النتيجة مقايضة بين الطاقة ومستوى
  الخدمة، ضمن الإعدادات التي قُيّمت.
- تنص الأطروحة نفسها على أن العدالة، وتوافق الحوافز، ومقاومة Sybil، وأمانًا يكافئ PoW **أهداف تصميمية لم تثبت تجريبيًا**.

ملخص لما أخذه التنفيذ من الأطروحة وما أضافه إليها في
[development/m1/reference/scope.md](development/m1/reference/scope.md).

## كيف يعمل PoCol

الوصف أدناه مبسّط. القواعد الملزمة في المواصفة: [development/m1/reference/](development/m1/reference/)
(`consensus.md` و`execution.md` و`network.md` و`storage.md` و`browser.md` و`governance.md` و`incentives.md` و`validation.md`).

### هوية الشبكة (genesis)
- كل شبكة معرّفة بـ`GenesisSpec`: ترميز RLP صارم لنسخة المواصفة، وchainId، ومعلمات الإجماع `CP`، وجذر الأرصدة
  الأولية، وقائمة المعدّنين الأولى `M_0`، وبصمة كود عقود النظام.
- `genesisHash = keccak256(GenesisSpec)`. أي اختلاف في الهوية أو الإصدار يقطع الاتصال بين العقد.
- قبل الإقلاع يقيّم **ParamGate** القواعد R1–R26 على المعلمات المحلية والإجماعية. الملف الذي يخالف قاعدة يُرفض مع
  قائمة القواعد المخالفة.

### الجولة والقالب
1. **القالب:** في كل محاولة يوقّع مقترح المحاولة 0 قالب الكتلة: المعاملات، والطابع الزمني، والهدف. إن تأخر المقترح
   تُبنى **كتلة احتياطية (fallback)** حتمية غير موقّعة بعد مهلة `Δ_fb_wait`.
2. **البذرة والتعيين:** تُشتق البذرة من ختم (seal) الكتلة الأب، وتحدد لكل معدّن نشط نطاقًا منفصلًا من الـnonce
   (`pocol_getAssignment`).
3. **العمل:** مدخل إثبات العمل هو `TemplateID ‖ nonce`. الكتلة صالحة إذا كانت تجزئته دون الهدف.
4. **الحصص:** كل nonce تحقق الهدف الأسهل `T_share = target · m` حصة. يدرج الفائز قائمة الحصص في رأس الكتلة.
5. **الصعوبة:** تضبط ASERT الهدف (مرساة مطلقة، عرض ثابت) ليقترب زمن الكتلة من `T_blk`.

### اختيار السلسلة
- تُختار السلسلة ذات **العمل التراكمي الأكبر**، وعند التعادل يحسم كاسر تعادل معجمي على مسار الفرع (D133).
- إعادة التنظيم محدودة بعمق `R_keep`: الفرع الأعمق يُرفض.

### المكافآت والتسوية
- `R_tot = subsidy + fees`. يُسجَّل الفائز وحصص الكتلة في الكتلة الابن (`ParentResult`).
- تتم التسوية مؤجلة عبر عقود النظام: Registry وEscrow وPAY. التقريب الصحيح والحرق مصنّفان.
- التسجيل يتطلب سندًا (`B_reg`). الخمول بعد عدد محدد من الكتل يُخرج المعدّن من المجموعة النشطة.

### التنفيذ (EVM)
- تُنفذ معاملات المستخدمين على revm 19.7 بقواعد Cancun، مع فصل واضح بين الإجماع والتنفيذ:
  `apply(S_parent, ctx, body, parentHeader) → (S, receipts, roots, …)`.
- للشبكة سقوف تراكمية على الحسابات والخانات والكود وبايتات المعاملات، ولها ارتفاع إغلاق نهائي `H_END`.

### تخزين المواقع والمتصفح
- **التخزين:** يُقسَّم الموقع إلى قطع تُنشر عقودًا بعناوين CREATE2 عبر `ChunkFactory`، مع manifest يحمل بصمات الملفات.
  لكل موقع إصدارات و`current()`.
- **التحقق:** تجلب إضافة Chrome الموقع، وتتحقق من عنوان كل قطعة وبصمة الـmanifest وكل ملف. ثم تعرضه في إطار معزول بلا
  شبكة (CSP، وقواعد DNR لكل تبويب، وRtcLockdown)، وكل طلب من الموقع يمر عبر BridgeAuth.
- **العميل الخفيف (RP):** يتحقق من نافذة من 13 رأسًا.
- **المحفظة:** محلية لشبكة التطوير، وكل توقيع يتطلب تأكيدًا يعرض البايتات الكاملة.

## كيف طوّر المشروع

طُوّر المشروع بعد الأطروحة، بتوجيه صاحب الفكرة واعتماداته، وبمساعدة وكلاء برمجة بالذكاء الاصطناعي في أدوار منفصلة:

- **Claude** (Anthropic) يؤلف التغييرات: المواصفة والكود والاختبارات والأدلة.
- **Codex** (OpenAI) يراجعها مستقلًا ويشغّل الفحوص، وأحيانًا ينفذ جولات إصلاح.
- **المالك** يتخذ القرارات الإجماعية والمعيارية. لا يُعدّ أي قرار معتمدًا ما لم يعتمده المالك صراحة، والقرارات المفتوحة
  مسجلة في `development/audit-2026-10-22/proposals/`.

قواعد العمل في [development/README.md](development/README.md): كل نتيجة تُربط بمتطلب ودليل، ولا يُغلق بند بمرور الوقت أو
بالاتفاق وحده، ونجاح إعادة التشغيل لا يفسر فشلًا سابقًا.

المراحل بالترتيب:

| المرحلة | ما أنجزته | أين |
|---|---|---|
| **M1** | المواصفة: من خط الأساس العربي إلى مسودات إنجليزية 0.1–0.33، مع متجهات بايتات ومراجعات مستقلة | `development/m1/` |
| **LP1–LP2** | عقدة Rust أولى (ترميز RLP، GenesisSpec، ParamGate، الرؤوس، ASERT، التعيين) وسجل كتل مرشحة | `prototypes/lp1-node/`، `development/lp1`، `lp2` |
| **LP3** | نموذج مرجعي بـTypeScript للحساب والتوقيت والعضوية والتعيين، ومقارنته مع Rust | `prototypes/lp3-model/`، `development/lp3` |
| **LP4–LP8** | طبقة التنفيذ: revm، وعقود النظام، وmempool، وPAY، والتسوية، والسقوف، وحدود الموارد | `prototypes/lp4-execution/`، `development/lp4` |
| **LP5–LP7** | العقدة `pocold`: الإنتاج والتعدين، واختيار السلسلة، والمزامنة بين العقد، وP2P، وRPC | `prototypes/pocold/` |
| **LP9** | إضافة Chrome MV3 | `prototypes/pocol-extension/`، `development/lp9` |
| **LP10** | شبكة تطوير متكاملة: نشر مواقع، وإعادة تنظيم، وإعادة تشغيل، ومتصفح | `prototypes/lp10/`، `development/lp10` |
| **تدقيق 2026-10-22 وجولاته** | إصلاح A01–A14، وحدود الذاكرة A13، ومخزن الحالة على القرص (redb)، ونقاط الحفظ، وRpcGuard، وأعطال I/O، والتحقق على Windows وChrome | `development/audit-2026-10-22/` |

ملخصات المراحل بالعربية في [development/reports/](development/reports/) و[development/STATUS-AR.md](development/STATUS-AR.md).
وحالة كل بند من التدقيق في `development/audit-2026-10-22/FINDING-STATUS.json` و`DASHBOARD-AR.md`.

## هيكلية المستودع والكود

```
.
├── PhD-thesis-Raed-Rasheed-120211566-61.pdf   الأطروحة (أصل الفكرة)
├── development/                               المواصفة، المتابعة، الأدلة، القرارات
│   ├── m1/reference/                          المواصفة المرجعية (بالعربية): consensus, execution, network, storage, browser, …
│   ├── m1/M1-SPEC.md, m1-draft-*/            مسودات المواصفة الإنجليزية ومتجهاتها
│   ├── VA-SPEC.md                             مواصفة التحقق
│   ├── lp1 … lp10/                            أدلة كل مرحلة
│   ├── audit-2026-10-22/                      التدقيق وجولات الإصلاح وأدلتها وجدول القرارات المفتوحة
│   ├── reports/, STATUS-AR.md                 ملخصات عربية
│   └── *-APPROVAL.md, OWNER-STANDING-AUTHORIZATION.md   اعتمادات المالك
├── prototypes/
│   ├── pocold/                                العقدة (Rust 1.99.1)
│   ├── lp4-execution/                         طبقة التنفيذ وعقود النظام (Rust 1.99.1 + Solidity)
│   ├── pocol-extension/                       إضافة Chrome MV3 (JavaScript)
│   ├── lp1-node/                              العقدة الأولى: الترميز والتحقق (Rust 1.58)
│   ├── lp3-model/                             النموذج المرجعي (TypeScript)
│   └── lp10/run_lp10.py                       مشغّل كل الحزم
└── .github/workflows/                         فحوص Windows وChrome وLP3 وLP4
```

### `prototypes/pocold` — العقدة

| الملف | المسؤولية |
|---|---|
| `src/main.rs` | سطر الأوامر: `init-devnet` و`run` و`status` |
| `src/node.rs` | حلقة الأحداث: الاستيراد، والتحقق، والـmempool، والقوالب، والاستعادة عند الفتح |
| `src/chain.rs` | شجرة الكتل، واختيار السلسلة بالعمل، وإعادة التنظيم، والاحتفاظ، والكتل الجانبية |
| `src/miner.rs` | التعدين ضمن النطاق المعيّن والحصص |
| `src/net.rs` | P2P عبر TCP: المصافحة، والمزامنة، والبث، وحدود الاتصالات |
| `src/rpc.rs` | خادم JSON-RPC مع RpcGuard: الطوابير، والمهل، ومعدل الطلبات، وحدود الذاكرة |
| `src/store.rs` | سجل الكتل `blocks.journal` (إلحاقي، مع تعافٍ من الذيل الممزق) |
| `src/history.rs` | `history.redb`: الإيصالات وفهرس المعاملات |
| `src/state_store.rs` | `state.redb`: نسخ الحالة كأشجار trie على القرص (وضع `--state-store` التجريبي) |
| `src/checkpoint.rs` | نقاط حفظ الحالة المختومة والمرتبطة بالسجل |
| `src/statediff.rs` | فروق الحالة بين الكتل |
| `src/resources.rs` | نموذج A13: الحد المحسوب للذاكرة وفحصه عند الإقلاع |
| `src/site.rs`، `src/bin/pocol-site.rs` | نشر المواقع وجلبها والتحقق منها |
| `src/bin/pocol-sign.rs` | توقيع معاملة لشبكة التطوير |
| `scripts/devnet.py` | إنشاء شبكة محلية وتشغيلها وإيقافها |
| `tests/` | اختبارات الإجماع والشبكة والتخزين والسقوف والميزانية، واختبارات e2e بـPython |

### `prototypes/lp4-execution` — التنفيذ
- **الملفات:** `apply.rs` و`apply/` (تطبيق الكتلة)، و`system.rs` و`system/` (مراحل النظام)، و`pay.rs` و`native.rs`،
  و`pool.rs`، و`state_trie.rs` و`node_trie.rs` (الجذور)، و`admission.rs` و`admission/` (ParamGate)، و`resources.rs`،
  و`modexp.rs` (مسارات MODEXP السريعة مع مطابقة revm)، و`devnet.rs` (بناء genesis للتطوير).
- **العقود (`contracts/`):** `RegistryPayment.sol` و`PendingEscrow.sol` و`ChunkFactory.sol` وعقود الشهود، وتخطيط
  التخزين في `REGISTRY-LAYOUT.md`.

### `prototypes/pocol-extension` — إضافة Chrome
- `src/sw.js`: الـservice worker، وفيه ملفات الشبكة وقواعد DNR والتوجيه.
- `src/lib/`:
  - `rp.js`: نافذة الرؤوس؛
  - `site.js`: جلب الموقع والتحقق منه؛
  - `bridge.js`: BridgeAuth؛
  - `sitestore.js`: التخزين المحلي للموقع؛
  - `wallet.js`: المحفظة.
- `src/sandbox-loader.js` و`src/viewer.js`: الإطار المعزول.
- `build.mjs`: بناء `dist/`، وبناء نسخ أعطال مقصودة لضوابط الاختبار السلبية.

## المتطلبات

| الأداة | النسخة | لماذا |
|---|---|---|
| Rust | 1.99.1، مثبّتة في `rust-toolchain.toml` | `pocold` و`lp4-execution` |
| Rust | 1.58.1 | `lp1-node` فقط |
| Python | 3.10 أو أحدث | `devnet.py` واختبارات e2e |
| Node.js | 22 أو أحدث | بناء الإضافة واختباراتها |
| Chromium/Chrome + Playwright 1.62.1 | — | اختبارات المتصفح فقط |

يعمل البناء على Linux وmacOS وWindows. تُفحص Windows وChrome آليًا في `.github/workflows/`. تعمل الشبكة المحلية على
`127.0.0.1` فقط، بمفاتيح تطوير حتمية لا قيمة لها.

## التشغيل السريع: شبكة تطوير محلية

```sh
git clone https://github.com/palcertcom/pocol.git && cd pocol

# 1) بناء العقدة والأدوات
cd prototypes/pocold && cargo build --release --locked && cd ../..
# الناتج: prototypes/pocold/target/release/{pocold,pocol-site,pocol-sign}

# 2) شبكة من ثلاث عقد (a, b, c)
D=prototypes/pocold/scripts/devnet.py
python3 $D init  /tmp/devnet --miners a,b,c --t-blk 2   # genesis, devnet.json, profile.json
python3 $D start /tmp/devnet --log                      # P2P على 30300+i و RPC على 8540+i
python3 $D status /tmp/devnet                           # الارتفاع والرأس والأقران والإنتاج
python3 $D rpc /tmp/devnet a eth_blockNumber
python3 $D stop /tmp/devnet                             # أو --only b لإيقاف عقدة واحدة
```

ملاحظات:
- **التوقيت:** اجعل `T_blk ≥ 1 + dFbWait`. القيم الافتراضية `--t-blk 2 --fb-wait 1` مستدامة.
- **إعادة التشغيل:** العقدة التي تُعاد بنفس مجلد `--data` تعيد تشغيل سجلها، وتُستعاد الذيول الممزقة.
  `pocold status --network DIR --data DIR` يطبع الرأس دون تشغيل الشبكة.
- **وضع التخزين على القرص (تجريبي):** أضف `--state-store` لأوامر `pocold run`، أو `POCOLD_RUN_ARGS=--state-store`
  مع `devnet.py`. افتح كل مجلد بيانات بالوضع الذي أُنشئ به.
- **تشغيل `pocold` مباشرة:**
  ```sh
  pocold init-devnet --out NET --miners a,b,c [--chain-id 777900] [--t-blk S] ...
  pocold run --network NET --data DATA --name a --miner a --listen 127.0.0.1:30300 --rpc 127.0.0.1:8540 [--peer ADDR]...
  ```
  `pocold help` يعرض كل الخيارات.
- **الشبكات المسموحة:** chainId الافتراضي 777900 (simnet). ملف الشبكة الإنتاجي (chainId 777001) لا يُقلع عمدًا.

## نشر موقع وقراءته من المتصفح

```sh
CID=$(python3 -c "import json;print(json.load(open('/tmp/devnet/net/devnet.json'))['chainId'])")
B=prototypes/pocold/target/release
$B/pocol-site publish --rpc 127.0.0.1:8540 --chain-id $CID --account alice --dir ./mysite
#   -> {"site":"0x…","version":0,...}   للتحديث أضف --site 0x…
$B/pocol-site fetch --rpc 127.0.0.1:8541 --site 0x… --out ./fetched   # يتحقق من كل قطعة وملف
```

إضافة Chrome:
1. ابنِ الإضافة:
   ```sh
   cd prototypes/pocol-extension
   npm ci
   node build.mjs    # الناتج في dist/
   ```
2. افتح `chrome://extensions`، وفعّل وضع المطوّر، واختر Load unpacked ثم مجلد `prototypes/pocol-extension/dist`.
3. في صفحة خيارات الإضافة، الصق `/tmp/devnet/net/profile.json` ثم اضغط Add. يُقبل الملف فقط إذا طابقت بصمة
   `genesisPre` قيمة `genesisHash`.
4. لفتح موقع، اكتب في شريط العنوان: `pocol devnet/0x<site>/`.
5. المحفظة لشبكة التطوير فقط. لتمويلها استخدم `pocol-sign`، ثم أرسل الناتج بـ`eth_sendRawTransaction`.

التفاصيل الكاملة في [development/lp10/LP10-RUNBOOK.md](development/lp10/LP10-RUNBOOK.md)
و[development/lp9/LP9-EXTENSION.md](development/lp9/LP9-EXTENSION.md).

## واجهة JSON-RPC

العقدة تقدم مجموعة من `eth_*` المعيارية:
- `eth_blockNumber`، `eth_chainId`، `eth_getBalance`، `eth_getCode`، `eth_getStorageAt`؛
- `eth_call`، `eth_estimateGas`، `eth_sendRawTransaction`، `eth_getTransactionReceipt`؛
- `eth_getLogs`، `eth_getBlockByNumber`، `eth_feeHistory`، وغيرها.

وتضيف `net_*` و`web3_clientVersion`، وطرقًا خاصة بـPoCol:

| الطريقة | الغرض |
|---|---|
| `pocol_status`، `pocol_syncStatus` | حالة العقدة والمزامنة |
| `pocol_getTemplate`، `pocol_getAssignment`، `pocol_getTarget`، `pocol_submitShare` | القالب ونطاق الـnonce والهدف، وتقديم الحصص (محلي فقط) |
| `pocol_getMiners`، `pocol_getMinerSet` | المعدّنون والمجموعة النشطة |
| `pocol_getSettlement`، `pocol_getEscrow`، `pocol_getPot`، `pocol_getBurnTotals` | المكافآت والتسوية والحرق |
| `pocol_getHeaders`، `pocol_getBlockRlp`، `pocol_getLogs` | الرؤوس للعميل الخفيف، والكتل، والسجلات المرتبطة بالفرع |
| `pocol_getParams`، `pocol_getCaps`، `pocol_getTiming`، `pocol_getClosing`، `pocol_getRetention` | المعلمات والسقوف والتوقيت والإغلاق والاحتفاظ |
| `pocol_getRejects`، `pocol_getRpcStats` | التشخيص |

حدود RPC، مثل الاتصالات والأحجام والمهل، موصوفة في `development/m1/reference/network.md`
و`development/audit-2026-10-22/RPCGUARD.md`.

## الاختبارات

```sh
# كل الحزم دفعة واحدة (الأدلة في development/lp10/LP10-EVIDENCE.json)
python3 prototypes/lp10/run_lp10.py
python3 prototypes/lp10/run_lp10.py --only e2e-browser

# العقدة
cd prototypes/pocold
cargo test --release --locked
cargo test --release --locked --features io-fault \
  --test state_store --test consensus --test checkpoint_source --test eviction_memory   # أعطال I/O حتمية
cargo test --release --locked --features crash-test --test consensus                     # انهيار في خطوات محددة
python3 tests/e2e_devnet.py && python3 tests/e2e_site.py

# طبقة التنفيذ
cargo test --release --locked --manifest-path prototypes/lp4-execution/Cargo.toml

# الإضافة
cd prototypes/pocol-extension && npm ci && npm test
node tests/e2e_browser.mjs       # يحتاج Chromium وPlaywright وعقدًا محلية
node tests/e2e_integrated.mjs    # تقسيم الشبكة، وإعادة التنظيم، وإعادة التشغيل
```

- **القياسات الطويلة:** حجم السجل حتى `H_END`، والتشغيل عند سقوف الشبكة، وقياسات الأداء موجودة كاختبارات `#[ignore]`
  في `tests/journal_measure.rs`. شغّلها بـ`-- --ignored`.
- **المنافذ:** تستخدم الحزم 30300–30302 و8540–8542 و8600–8602. شغّل حزمة واحدة في كل مرة.
- **حفظ الأدلة:** عند الفشل تحفظ أدوات e2e سجلات العقد والمدخلات والبذور في `POCOL_E2E_KEEP` أو `tests/failures/`.

## التطوير والمساهمة

- **الإجماع خط أحمر:** أي تغيير يمس قاعدة إجماع أو معلمة في `CP` أو ترميزًا على السلك يحتاج قرارًا من المالك. اكتب
  مقترحًا في `development/audit-2026-10-22/proposals/` بالبدائل والأثر، ولا تنفذه قبل اعتماده.
- **الحدود المحلية:** لا تُرفع حدود الموارد إلا بقرار. كل بنية بيانات جديدة في العقدة يجب أن تُحسب في نموذج A13
  (`src/resources.rs`)، والاختبار `a13_bound_breakdown` يثبّت الإجمالي.
- **الاختبار قبل الادعاء:** أضف اختبارًا يفشل بدون الإصلاح، وسجّل الأمر والنسخة والنتيجة. احفظ الإخفاقات في الأدلة، ولا
  تعدّ الاختبار المتجاهل أو الملغى نجاحًا.
- **الأسلوب:**
  - الكود والمواصفات التقنية بالإنجليزية، وملخصات المتابعة بالعربية؛
  - اتبع أسلوب الملف الذي تعدّله؛
  - الاعتماديات مثبّتة بنسخ دقيقة في `Cargo.lock` و`package-lock.json`.
- **CI:**
  - `audit-r2-windows-verify.yml`: Windows Server 2022؛
  - `pocol-browser-verify.yml`: Chrome على Linux بالوضعين؛
  - `lp3-*.yml` و`lp4-execution-controls.yml`.
- **الأمان:**
  - الاختبارات محصورة في عقد وRPC محلية وملفات متصفح اختبارية؛
  - لا تستخدم أموالًا أو مفاتيح حقيقية؛
  - لا تنشر برامج استغلال.

## الحالة الحالية وحدود ما ثبت

**ثبت بالاختبار (محليًا وفي CI):**
- شبكة تطوير من ثلاث عقد تنتج الكتل وتتزامن.
- إعادة التنظيم عند التئام الشبكة في الاتجاهين.
- التعافي بعد `SIGKILL`.
- نشر المواقع وجلبها مع التحقق.
- عرضها في Chrome بعزل مُختبَر بضوابط سلبية.
- استئناف الحالة من نقاط الحفظ.
- إيقاف العقدة عند أعطال القرص دون الحكم على كتل صالحة بأنها غير صالحة.
- النجاح على Linux وWindows Server 2022 وChrome.

**لم يثبت أو ما زال مفتوحًا:**
- **ملف الشبكة الإنتاجي لا يُقلع على `pocold`:** لا يوجد genesis معتمد للشبكة. وضع الحالة الكاملة لا يمر بشرط الذاكرة
  A13، وR21 يرفض، ولا قبول شبكة بخلاف simnet (R22).
- **حد الذاكرة:** في وضع التخزين حد محسوب بنحو 5.2 GiB ضمن ميزانية 6 GiB، والشرط أن يكون النموذج × 8/7 ≤ 6 GiB. لم
  تُقس كل أسوأ الحالات.
- **قرارات معلّقة عند المالك:** قرارات إجماعية ومعيارية عديدة، منها المقترحات الإجماعية الستة وسقوف الشبكة وهوية
  الثوابت ومعايرة التوقيت، مسجلة في `development/audit-2026-10-22/proposals/DECISION-TABLE-2026-10-28.md`.
- **أهداف تصميمية:** الأمان المكافئ لـPoW، والعدالة، ومقاومة Sybil لم تثبت.
- **لا تدقيق خارجي:** لم يُجرَ تدقيق أمني بشري خارجي مستقل.

**لا تستخدم هذا الكود لشبكة حقيقية أو بأموال حقيقية.**

## الترخيص

الكود البرمجي في `prototypes/` مرخص بـ**Apache-2.0**، كما هو مذكور في ملفات `Cargo.toml` و`package.json`. التراخيص الخارجية
المضمّنة في `prototypes/lp4-execution/licenses/` ومع المكتبات المضمّنة في الإضافة. حقوق الأطروحة لمؤلفها.

للتواصل حول الفكرة أو المشروع: افتح Issue في هذا المستودع.
