# فرز CodeQL — 5 نوفمبر 2026

مرجع القراءة `46d688b701e165211c36a5a1447fb029a3199302`. هذه مراجعة سياقات المصدر مع لقطات وصفحات المالك وصادرات CSV؛ صلاحية التكامل لا تتيح Code scanning API. لم يُصدّر SARIF الكامل، ولا يُدّعى فحص المسار الأصلي الفردي لكل تنبيه مدرج أدناه.

## القرارات التي لها دليل

| التنبيه | المصدر/المصب المبلغ عنه | أصل القيم والحكم المحدود | الحالة المرصودة |
| --- | --- | --- | --- |
| #7 | `deploy/studio/install_studio_gateway.py`، probe HTTPS | حد TLS 1.2 الصريح؛ مُثبت مولد مطابق واختبارات سياق SSL وInstallerTests ناجحة | **Fixed** عبر PR24 بحسب صفحة المالك على main |
| #200 | LP1 `fixed.rs:198` إلى `tests/assignment.rs:175` | مسار CodeQL من 28 خطوة يبدأ بمصفوفة صفرية لعملية ضرب، وينتهي عند `map.owner(map.nonce_max() as u64)` في اختبار حدود تقسيم مجال nonce للتعدين. ليس nonce لتشفير مفتاح. **False positive** في هذا المسار | أكد المالك تنفيذه؛ أصبحت القائمة 219 Open و2 Closed |
| #221 | `pocold/tests/state_store.rs:34` إلى `:41` | مسار CodeQL من 4 خطوات: `db.accounts` إلى تشخيص `assert_eq!` لقيم slots. helper خاص بحالات EVM مصطنعة يولدها الاختبار. **False positive** | انخفضت قاعدة التسجيل من21 إلى20 Open؛ لا قراءة لسبب الإغلاق عبر API |
| #220، #219، #218، #217 | الملف نفسه، الأسطر40،38،37،35 على الترتيب | مقارنة slots وcode وnonce/balance/code_hash ووجود الحساب، في helper نفسه وبمدخلات مولدات الاختبار المصطنعة فقط. **False positive** لكل موضع | أظهرت اللقطة بعد تصنيف الستة #215–#220:14 Open و7 Closed للقاعدة، و212 Open إجمالًا |
| #216، #215 | `pocold/tests/consensus.rs:1035` و`:1034` | storage وcode لحسابات devnet، وRPC محلي داخل العملية في اختبار pending؛ لا peers أو خادم RPC أو listen. **False positive** | ضمن الستة السابقة |
| #214، #213 | الملف نفسه، الأسطر1032 و1031 | nonce ورصيد حسابات devnet في الاختبار نفسه. **False positive** | راجعهما المساعد؛ لم يغيّر حالتهما |
| #212، #211 | `lp4-execution/src/view/tests.rs:102` و`:39` | code وحالة حساب/storage من Rng ببذرتين ثابتتين وبيانات صغيرة داخل اختبارات overlay. **False positive** | راجعهما المساعد؛ لم يغيّر حالتهما |
| #210، #209، #208 | `lp4-execution/src/apply/tests.rs:504` و`:501` و`:500` | slots وقيمها وcode لعقد fixture منشأ من bytes داخل اختبار devnet في الذاكرة، عبر30 كتلة مصطنعة. **False positive** | راجعها المساعد؛ لم يغيّر حالتها |

تخص هذه الأحكام المصب المبلغ عنه ومصادره في commit المرجعي. وسم Test وحده لا يثبت أن البيانات غير حساسة؛ تمت مراجعة كل موضع ومدخلاته. لم يُغلق المساعد تنبيهات، ولم تُعطل قاعدة Rust أو تُحذف اختبارات للتخلص من النتائج. السبعة الأخرى من قاعدة التسجيل لم تظهر مواضعها بعد، فلا حكم عليها.

## أدلة الاختبارات ومطابقة المصدر

- نجح اختبار LP1 `seeded_partition_properties_and_permutation_invariance` في تحقق معزول؛ [السجل المحفوظ](evidence/codeql-review-2026-11-05/assignment-boundary-test.log) هو المرجع لاسم الاختبار وأمره الفعليين.
- نجح `stored_versions_have_their_roots_and_states_and_survive_reopen` و`e3_pending_answers_are_the_pool_layer` باختبارين مستقلين، release/locked/offline، وبناء محدود بوظيفتين وخيط اختبار واحد. [الأوامر والنتائج](evidence/codeql-review-2026-11-05/logging-test-results.json)، [مطابقة المصدر](evidence/codeql-review-2026-11-05/logging-test-source-equivalence.json).
- يحفظ [سجل التنفيذ السابق](evidence/round2/execution.log) نجاح اختبارات `apply::tests::e4_a_node_trie_kept_from_the_write_sets_reads_as_every_block_state` و`view::tests::an_overlay_reads_and_merges_as_the_copy_it_replaces` و`view::tests::a_layer_without_its_code_copies_reads_and_merges_as_before`. قورنت مصادر LP4 وملفات Cargo/build ومصادر LP1 مع commit السجل ولم تختلف. [دليل إعادة استخدام السجل](evidence/codeql-review-2026-11-05/existing-execution-test-evidence.json). لم تُعد هذه الاختبارات لمجرد التوثيق.

## المصدر الدقيق للاستعلام

صدّر المالك [قائمة قواعد Rust](evidence/codeql-review-2026-11-05/code-scanning-rules-used.csv). ثبتت حزمة `codeql/rust-queries 0.1.46+1be292a1e54696ca543b4ad0f9b4537b9687caa7` من commit المستودع العام وqlpack.yml؛ [بيانات الأصل](evidence/codeql-review-2026-11-05/verified-query-pack-provenance.json).

تعامل heuristics العامة أسماءً مثل accounts على أنها بيانات حساب حساسة، وتصل نماذج logging إلى تشخيصات assert. يفسر هذا إمكان الإنذار على قيم اختبارية؛ لا يثبت أن كل إنذار من هذه القاعدة غير صحيح. تصدير القواعد لا يضم معرفات التنبيهات أو مواضعها أو حالتها الحالية.

## أدلة التغطية والجودة

[CSV الملفات الأصلي](evidence/codeql-review-2026-11-05/code-scanning-files-extracted.csv) يضم693 مسارًا فريدًا ناجح الاستخراج. جرد main كشف29 مسارًا غير مدرج من الفئات الممثلة؛ [التفصيل](evidence/codeql-review-2026-11-05/coverage-comparison.json)، [قائمة المسارات](evidence/codeql-review-2026-11-05/paths-not-listed.csv)، [مراجع scripts في HTML](evidence/codeql-review-2026-11-05/html-script-reference-review.json). ليست المقارنة نسبة تغطية عامة، ولا يضم CSV SHA أو وقت analysis.

نجاح استخراج كل الملفات المدرجة لا يغلق تحذير جودة C# أو فقد تبعيات Go. نجح تشغيل Windows WPF المستقل، وبقي بناء Go متوقفًا بسبب تنزيل الحزم؛ لم يُعد CodeQL بسياق بناء مصحح، ولم تتحسن مقاييسه المثبتة. [الحالة الكاملة والعمل المتبقي](LATEST-STATUS-2026-11-05-AR.md).
