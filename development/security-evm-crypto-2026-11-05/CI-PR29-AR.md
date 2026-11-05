# نتيجة CI لمصدر إصلاحات PR29

المصدر المختبر: `1355b12a8b03e7fda32138cda2ee479ad39ddc21`.

**نجحت 26 مهمة CI**. بقي فحص CodeQL التجميعي فاشلًا بسبب 11 نتيجة راجعت مصادرها كـFalse positive. لم يُدمج PR29 ولم تُعدل حالات التنبيهات هنا. [المراجعة وأداة المالك](CODEQL-PR29-REVIEW-AR.md).

هذه النتائج من GitHub API، مع حفظ [حالات الفحوص](evidence/ci-source-1355b12-checks.json) و[تفاصيل المهمات](evidence/ci-source-1355b12-jobs.json). لم تُتحقق بايتات أرشيف artifacts المحجوب بسياسة الشبكة؛ لا تُستنتج مؤشرات جودة SARIF الخام من نجاح المهمة.

|المهمة|النتيجة|الدليل|
|---|---|---|
|Analyze (csharp)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677292)|
|Analyze (java-kotlin)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677531)|
|Analyze (actions)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677610)|
|Analyze (c-cpp)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677650)|
|Analyze (go)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677654)|
|Analyze (python)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677708)|
|Analyze (rust)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677747)|
|Analyze (javascript-typescript)|نجاح|[تشغيل 42125256600](https://github.com/palcertcom/pocol/actions/runs/42125256600/job/127587677804)|
|Windows Server 2022 (extension)|نجاح|[تشغيل 42125261552](https://github.com/palcertcom/pocol/actions/runs/42125261552/job/127587688574)|
|Windows Server 2022 (lp1)|نجاح|[تشغيل 42125261552](https://github.com/palcertcom/pocol/actions/runs/42125261552/job/127587688860)|
|Windows Server 2022 (pocold)|نجاح|[تشغيل 42125261552](https://github.com/palcertcom/pocol/actions/runs/42125261552/job/127587689057)|
|Windows Server 2022 (lp4)|نجاح|[تشغيل 42125261552](https://github.com/palcertcom/pocol/actions/runs/42125261552/job/127587689142)|
|Windows Server 2022 (pocold-io-fault)|نجاح|[تشغيل 42125261552](https://github.com/palcertcom/pocol/actions/runs/42125261552/job/127587689236)|
|browser (store)|نجاح|[تشغيل 42125261563](https://github.com/palcertcom/pocol/actions/runs/42125261563/job/127587688382)|
|browser (memory)|نجاح|[تشغيل 42125261563](https://github.com/palcertcom/pocol/actions/runs/42125261563/job/127587688643)|
|rehearsal|نجاح|[تشغيل 42125261589](https://github.com/palcertcom/pocol/actions/runs/42125261589/job/127587688563)|
|operations|نجاح|[تشغيل 42125261654](https://github.com/palcertcom/pocol/actions/runs/42125261654/job/127587688725)|
|browser|نجاح|[تشغيل 42125261684](https://github.com/palcertcom/pocol/actions/runs/42125261684/job/127587688891)|
|consensus (ubuntu-24.04)|نجاح|[تشغيل 42125261684](https://github.com/palcertcom/pocol/actions/runs/42125261684/job/127587689187)|
|consensus (windows-2022)|نجاح|[تشغيل 42125261684](https://github.com/palcertcom/pocol/actions/runs/42125261684/job/127587689206)|
|keys-and-enrollment|نجاح|[تشغيل 42125261752](https://github.com/palcertcom/pocol/actions/runs/42125261752/job/127587688786)|
|csharp|نجاح|[تشغيل 42125261816](https://github.com/palcertcom/pocol/actions/runs/42125261816/job/127587689528)|
|go|نجاح|[تشغيل 42125261816](https://github.com/palcertcom/pocol/actions/runs/42125261816/job/127587689995)|
|native (ubuntu-24.04)|نجاح|[تشغيل 42125261827](https://github.com/palcertcom/pocol/actions/runs/42125261827/job/127587689211)|
|android|نجاح|[تشغيل 42125261827](https://github.com/palcertcom/pocol/actions/runs/42125261827/job/127587689572)|
|native (windows-2022)|نجاح|[تشغيل 42125261827](https://github.com/palcertcom/pocol/actions/runs/42125261827/job/127587689959)|

يشمل ذلك بناء Windows وLinux وAndroid، فحص العقدة الكامل وio-fault على Windows، المتصفح الفعلي في وضعي memory/store مع الانقسام وإعادة التشغيل، فحوص التوافق والعمليات والتسجيل، وتحليل CodeQL للغات الثماني ومهمتي البناء التشخيصي C#/Go. APK غير موقّع ولم يختبر على هاتف فعلي.

متابعة التقرير والأدوات في PR30 لا تغير أي مدخل منتج مختبر. حُفظ PR29 عند رأس المصدر أعلاه حتى لا يُلغى فحص جارٍ. يجب معالجة بوابة CodeQL والتحقق من تحديثها قبل الدمج؛ وبعد دمج PR29 يُعاد توجيه PR30 إلى main.
