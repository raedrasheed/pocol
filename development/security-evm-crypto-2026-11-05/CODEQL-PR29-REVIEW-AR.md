# مراجعة تنبيهات PR29 الجديدة

> صُحّح مرجع أداة المالك في 6 نوفمبر 2026 إلى `refs/pull/29/head` المطابق
> لتشغيل GitHub الفعلي. [سبب التوقف واختبارات التصحيح](CODEQL-PR29-REF-FIX-AR.md).

بعد رفع المصدر `1355b12a8b03e7fda32138cda2ee479ad39ddc21`، أنهى CodeQL
تحليل اللغات الثماني وأبلغ فحص الطلب عن **11 تنبيهًا جديدًا: 10 critical و1 high**.
أُبقي الفحص الفاشل ولم تُعدل نتيجته أو تُحذف اختبارات أو تُضف suppressions.
السجل هو [فحص CodeQL](https://github.com/palcertcom/pocol/runs/127587951047)،
والـannotations الأصلية محفوظة في
[`evidence/codeql-new-alert-annotations.json`](evidence/codeql-new-alert-annotations.json).

|الملف والسطر/العمود|القاعدة|مراجعة المسار والقرار|
|---|---|---|
|earnin_halt_proof.rs 39:43، 41:40، 50:37، 58:62|rust/hard-coded-cryptographic-value|القيمة 1 تدخل `insert -> AccountInfo::new(..., nonce, ...)` لتكوين حسابات genesis داخل InMemoryDB اختبارية. هي حالة عامة لحساب EVM، لا nonce توقيع ECDSA. **False positive** لكل موضع.|
|earnin_halt_proof.rs 126:56، 150:56، 177:56|القاعدة نفسها|القيمة 0 تهيئ عداد معاملات حساب المهاجم الاصطناعي. `nonce_of` يقرأ العداد، وTxEnv يستخدمه للتحقق العام من ترتيب معاملات CREATE. لا سر أو عشوائية تشفيرية. **False positive** لكل موضع.|
|earnin_halt_proof.rs 127:29، 151:29، 178:29|القاعدة نفسها|`attacker.create(0)` يحسب عنوان CREATE العام المطابق لمعاملة العداد 0 في قاعدة الاختبار. تغيير الثابت عشوائيًا سيكسر شاهد الانحدار. **False positive** لكل موضع.|
|signing_compat.rs 83:14–71|rust/cleartext-logging|الوسيط الوحيد في println هو `secrets.len() * digests.len()`؛ يطبع عدد الحالات، وليس بايتات مفتاح أو قيمة مشتقة من محتواه. تعداد المفاتيح يُضبط بعدد fixtures عام 1..4096، وعدد digests ثابت. كل المفاتيح أصلًا fixtures عامة مشتقة من وسوم ثابتة. **False positive**.|

المسارات الكاملة تحت `prototypes/lp4-execution/tests/`. لكل واحدة من الحالات
الـ11 صف مستقل مع القاعدة والموقع الدقيق ونص السطر وSHA256 **للملف الكامل**
وتعليق الإغلاق في [`reviewed-pr29-codeql.json`](reviewed-pr29-codeql.json).
ليست العبرة بوسم Test وحده: العداد بروتوكولي عام، والمخرج الوحيد في التنبيه
الأخير عدد عناصر، كما يبين المصدر الفعلي. اختبارات earnin الثلاثة والتوقيع
مرت في مجموعة التنفيذ المحلية، وكذلك تحقق التنفيذ على Windows في PR29.

التكامل الحالي لا يملك صلاحية تعديل التنبيهات ويعيد 403. لذلك هذه **مراجعة
مكتملة للمصدر، وليست إثبات إغلاق على GitHub**؛ بقي الدمج متوقفًا على نتيجة
CodeQL. لا تتطلب معالجة هذه النتائج تعديل كود التشفير أو سلوك nonce العام.

## تطبيق قرارات المراجعة بجلسة المالك

أداة [`apply_pr29_codeql.py`](apply_pr29_codeql.py) منفصلة عن دفعة main القديمة
ذات 197 تنبيهًا. تكتشف أرقام التنبيهات من PR29 فقط بالمطابقة الدقيقة للقاعدة
والموقع؛ لا تخمن IDs ولا تستخدم credentials بديلة. قبل أي PATCH تتحقق من
المصدر الثابت في **كل instance مفتوحة على أي ref** وثبات main وHEAD الطلب.
تتوقف على التطابق الناقص/الملتبس أو المصدر المختلف أو 403 وتحفظ سجلًا جزئيًا.
وضع القراءة لا يغير GitHub، والإغلاق يحتاج `--apply` صراحةً.

من جذر هذا الفرع على جهاز يملك جلسة `gh` العادية بصلاحية code-scanning:

```text
python development/security-evm-crypto-2026-11-05/apply_pr29_codeql.py --out codeql-pr29-verify-v2.json
python development/security-evm-crypto-2026-11-05/apply_pr29_codeql.py --apply --out codeql-pr29-apply-v2.json
```

يلزم أن يكون اسما النتيجتين جديدين. الحالات المغلقة تُتخطى. بعد التطبيق يلزم
التحقق من تحديث فحص GitHub نفسه قبل الدمج؛ لا تغيّر الأداة نتيجة CheckRun.
اختبارات API المحاكية لا تغلق أي تنبيه فعلي.
