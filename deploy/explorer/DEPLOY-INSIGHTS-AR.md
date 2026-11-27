# نشر قسم الحائزين وخريطة المعدّنين على خادم بوابة المستكشف

النسخة الإنجليزية: [DEPLOY-INSIGHTS.md](DEPLOY-INSIGHTS.md)، وهي المرجع عند أي اختلاف في الأوامر. التصميم وعقد الواجهة:
[MINERS-MAP-HOLDERS.md](../../development/explorer/MINERS-MAP-HOLDERS.md). السلوك والحدود: [README.md](README.md)
وملخصه العربي [README-AR.txt](README-AR.txt).

**الحالة: خطة فقط؛ لم يُنفَّذ أي شيء هنا على خادم.** النشر، وتنزيل قاعدة GeoIP، وأداة تقارير المشغّلين، كلٌّ منها يحتاج
موافقة المالك أولًا ([OWNER-DECISIONS-2026-11-25.md](../../development/decisions/OWNER-DECISIONS-2026-11-25.md)، قسم
"Still the owner's to decide"). كل الأوامر تُشغَّل على خادم البوابة الحالي (الذي يقدّم `explorer.pocol.net`، والمثبَّت
بـ `install_explorer_gateway.py` من main عند 04f77806)، بمستخدم لديه sudo، ومن نسخة Git لالتزام الإصدار المدموج. لا
شيء هنا يمسّ خوادم المؤسسين ولا إجماع PoCol Mainnet ولا بيانات العقدة ولا المفاتيح ولا DNS؛ البوابة تقرأ فقط عقدة الخادم
نفسه على `127.0.0.1:8545`.

## خطة إصدار PR #120

الأقسام التالية (من 0 إلى 7) هي خطوات كل خادم على حدة؛ وهذه الخطة ترتّبها في إصدار إنتاجي واحد، لكل مرحلة فيه بوابة
قرار (متابعة أو توقف) وطريقة استرجاع. لا تُنفَّذ أي مرحلة إلا بموافقة صريحة من المالك، ولم يُنفَّذ شيء من هذه الخطة على
خادم إنتاجي. وعبارة «لم تُقَس» تعني ذلك حرفيًا: لا توجد مدة مخمَّنة في هذه الخطة.

ما يقدّمه الإنتاج اليوم (مقيس في 2026-11-26):

- يقدّم Caddy على خادم البوابة `explorer.pocol.net` مباشرة (بلا Cloudflare): الصفحة `/` وملف الحزمة البرمجية (bundle)
  `assets/index-e46a96e9d375.js` و `/api/health.php` تُرسَل مع `cache-control: no-store`، فلا حاجة هناك أبدًا إلى مسح
  تخزين مؤقت. والعنوان `https://explorer.pocol.net/SHA256SUMS.txt` يعطي 404 (النسخة المقدَّمة لا تتضمنه أبدًا، والمثبّت
  يتحقق من هذا الرمز).
- الموقع `pocol.net` خلف Cloudflare؛ و `privacy.html` و `SHA256SUMS.txt` يُقدَّمان مع `cf-cache-status: DYNAMIC` (بلا
  تخزين مؤقت). الملف `pocol.net/SHA256SUMS.txt` في الإنتاج مطابق بايتًا ببايت لملف الالتزام f8379198 ‏(PR #119)، وبصمته
  `20b3783e841dfc30f9a604718bd1fce6f9c280aed04c82392354778cc120f08b`: ما زال يذكر حزمة الإضافة الأولى. أما الملفات
  الأربعة المنشورة الآن في `pocol.net/downloads` فهي الحزمة المعاد توقيعها في PR #121 التي يذكرها main. وكل ملف آخر في
  `SHA256SUMS.txt` الخاص بـ main يطابق الإنتاج بايتًا ببايت، ما عدا `api/network.php` و `api/reward.php` (ينفّذهما PHP
  فلا تصح مقارنتهما) و `chrome-extension.html`، الذي تختلف بايتاته المقدَّمة فقط لأن خاصية Email Address Obfuscation في
  Cloudflare تحوّل `release@pocol.net` إلى روابط `/cdn-cgi/l/email-protection` وتحقن
  `/cdn-cgi/scripts/.../email-decode.min.js` (إعداد في Cloudflare وليس الملف الأصلي؛ وهذا الإصدار لا يغيّره).
- البوابة مثبّتة بالأداة `install_explorer_gateway.py` من main عند 04f77806. ولم يغيّر PR #119 ولا PR #121 شيئًا في
  `deploy/explorer` ولا `deploy/rewards` ولا ملفات موقع المستكشف، فالفرع main عند 4b17da2c له المثبّت نفسه والوحدة نفسها
  ونسخة المستكشف نفسها `site-1c9ec5374487` (12 ملفًا؛ محسوبة من الالتزام، وتُؤكَّد على الخادم بالأمر
  `grep -n 'root \*' /etc/caddy/Caddyfile` من القسم 0).

الترتيب: المرحلة 0 (مستقلة)، ثم بعد دمج PR #120 المرحلة 1 (صفحة الخصوصية)، فالمرحلة 2 (بوابة المستكشف)، ثم المرحلتان
الاختياريتان 3 و4. تأتي صفحة الخصوصية أولًا لأن البوابة الجديدة تقرأ عناوين IP العامة لأقران عقدتها (أدناه)، والسياسة
المنشورة في 22 نوفمبر لا تصف ذلك.

### المرحلة 0: مزامنة `pocol.net/SHA256SUMS.txt` مع ملف main (مستقلة عن PR #120)

لا تعتمد هذه المرحلة على PR #120، ويمكن تنفيذها قبل دمجه بعد موافقة المالك على هذا الرفع (لا يغطّيها أي بند في
OWNER-DECISIONS-2026-11-25.md، فسجّل الموافقة بتاريخها). لا تتغيّر إلا أسطر التنزيلات الأربعة الخاصة بـ PR #121:

| الملف | SHA-256 |
|---|---|
| `pocol.net/SHA256SUMS.txt` في الإنتاج (= f8379198) | `20b3783e841dfc30f9a604718bd1fce6f9c280aed04c82392354778cc120f08b` |
| `deploy/websites/pocol.net/SHA256SUMS.txt` في main ‏(4b17da2c) | `14c386700ab2ac8d4edde416bbd04fa8717fcacbf9944d2782de9cc524842e72` |

| السطر | الإنتاج | main |
|---|---|---|
| `downloads/PoCol-Studio-IDE-Viewer-0.5.0.zip` | `f947c019...` | `6e38e72f331a52ca4de0cc701aad9e026ea0e08422468041fbc5bb5c24b71070` |
| `downloads/PoCol-Studio-IDE-Viewer-0.5.0.zip.sig` | `829e7bdc...` | `85f692b2316c09c6b11fe9591e65bebb2868aa821eefed3d9c2f1bf4a9042bdf` |
| `downloads/SHA256SUMS` | `470c39c9...` | `352e2231017abfdf30f0bf5558dcf71ed142f78c2f876aefb54bf5c59bc8f6eb` |
| `downloads/SHA256SUMS.sig` | `6cda09ce...` | `4cf40dca808b8efdc468dff3d2ed025115a8e7ea1e67c309d2efac2ed313d8fb` |

القيم الأربع في عمود main هي الملفات المنشورة الآن في `pocol.net/downloads` (تحقّقت منها الأداة `verify_release.py` مقابل
`allowed_signers`، بالمفتاح `SHA256:r/IMxkMr+wv3OyAbiX3XYhSV4pqy3Lv5/z/VV06DJyc`).

1. على جهاز فيه المستودع: `git show 4b17da2c:deploy/websites/pocol.net/SHA256SUMS.txt > SHA256SUMS.txt`، ثم يجب أن يطبع
   `sha256sum SHA256SUMS.txt` البصمة `14c38670...2e72` كما أعلاه. لا تفتح الملف في محرر ولا تحفظه منه (يجب أن تبقى نهايات
   أسطره LF).
2. في cPanel ثم File Manager (من Settings فعّل «Show Hidden Files»): انسخ `SHA256SUMS.txt` من جذر الموقع إلى المجلد
   `/pocol-backups` وأعد تسمية النسخة `SHA256SUMS.txt.before` (أنشئ المجلد `pocol-backups` في مجلدك الرئيسي إن لم يكن
   موجودًا)، أي خارج جذر الموقع كما في الخطوة 2 من `DEPLOY-pocol.net-AR.md`، حتى لا يُقدَّم أبدًا ولا تسرده
   `host_files.py` بعلامة `DELETE`.
3. ارفع ملف `SHA256SUMS.txt` من الخطوة 1 إلى جذر الموقع مع السماح بالاستبدال. لا يتغيّر شيء آخر.

**البوابة:** من جهاز آخر يطبع `curl -s https://pocol.net/SHA256SUMS.txt | sha256sum` البصمة `14c38670...2e72` (الملف
DYNAMIC فلا حاجة إلى مسح التخزين المؤقت). ومن cPanel ثم Terminal يُتوقَّع أن تطبع الخطوة 7.1 من `DEPLOY-pocol.net-AR.md`
(الأمر `grep -v '  \.htaccess$' SHA256SUMS.txt | sha256sum -c --quiet - && echo "FILES OK"` داخل جذر الموقع) العبارة
`FILES OK`؛ ولم يُشغَّل هذا على الخادم بعد.

**الاسترجاع:** احذف `SHA256SUMS.txt` الجديد من جذر الموقع، وانسخ `/pocol-backups/SHA256SUMS.txt.before` إلى جذر الموقع،
وأعد تسمية النسخة `SHA256SUMS.txt`؛ عندها يطبع أمر curl نفسه `20b3783e...f08b`. وهذا يعيد ملفًا يذكر حزمة الإضافة الأولى
المسحوبة، التي لم تعد تطابق التنزيلات المنشورة.

### قبل المرحلة 1: الشروط المسبقة

1. دمج المالك PR #120 في main **بالتزام دمج** (يُسمّى أدناه *التزام الدمج*؛ وأول 12 حرفًا من بصمته تسمّي حزمة المرحلة 1).
2. فحوص CI ناجحة على التزام الدمج: كل سير عمل يشغّله دفعه إلى main، ومنها «Websites» و «Consensus rules, profiles and
   extension release» (ووظيفتها «Extension release (Studio, IDE, Viewer)») و «pocol.net cPanel package» (الذي ينتج
   تشغيله حزمة المرحلة 1).
3. موافقات المالك، كلٌّ منها مسجّلة بتاريخها (يُطلب من المالك أن يوافق على كل مرحلة مع استرجاعها، حتى يُسترجع أي توقف
   فورًا ثم يُبلَّغ عنه):
   - نص سياسة الخصوصية في المرحلة 1، يراجعه المالك قبل نشره، ونشره في pocol.net (OWNER-DECISIONS-2026-11-25.md، قسم
     "Still the owner's to decide"، البند 3، ونشر pocol.net هذا)؛
   - نشر البوابة في المرحلة 2 (البند 4)؛
   - كل إعادة تشغيل لـ `pocol-explorer-gateway.service` وكل إعادة تحميل لـ Caddy: كل تشغيل للمثبّت (المرحلة 2، والمرحلة 3،
     وخطوة البوابة في المرحلة 4، والتحديث الشهري لقاعدة GeoIP، والإصلاح في «الاسترجاع بعد مرحلة لاحقة»)، والاسترجاع
     التلقائي في 6.1، والاسترجاع اليدوي في 6.2، والإيقاف والتشغيل في استرجاع المرحلة 3؛
   - اختياريًا للمرحلة 3: تنزيل قاعدة GeoIP وإسنادها وفق CC BY 4.0 (البند 2)؛
   - اختياريًا للمرحلة 4: أداة التقارير على كل خادم مؤسس، بموافقة مستقلة لكل خادم (البند 1).
4. على خادم البوابة نسخة Git من التزام الدمج (يطبعها `git rev-parse HEAD` في القسم 0)، بلا أي تعديل محلي.

### المرحلة 1: صفحة الخصوصية في pocol.net

يجب أن تكون منشورة قبل المرحلة 2: فالبوابة الجديدة تقرأ قائمة أقران عقدتها وتستبدل في الذاكرة كل عنوان IP عام لقرين
برمز مشتق بمفتاح (`pocol_geoip.observe_peers`، بملح عشوائي، مرة كل 10 ثوانٍ على الأكثر ما دامت الخريطة تُقرأ)، حتى بلا
قاعدة GeoIP؛ والسياسة المعدّلة هي التي تصف ذلك، وتصف البحث عن الدولة في المرحلة 3. وحتى المرحلة 3 تصف السياسة بحثًا لا
تجريه البوابة بعد (بلا قاعدة لا يُحدَّد أي موقع): فهي لا تصف أبدًا أقل مما تفعله البوابة. لا يغيّر PR #120 في pocol.net
إلا هذه الملفات الثلاثة؛ وبعد تنفيذ المرحلة 0 تكون هي وحدها المختلفة عن الإنتاج:

| الملف | الإنتاج (= main) | الجديد (PR #120) |
|---|---|---|
| `privacy.html` | `370e76bfc7c359c8bf0bf9926fa80106231a2785076a35621bbdf3bf70d2dbce` | `25c1a2731a1ebe1651126b3742d9fb905810773748a4f3fa42e0c6ce0d13c63e` |
| `privacy.js` | `e12dff2468bdbe8e9ff9695e244761e6cac29d8b29b3f5ce8a08a1ff330aaa94` | `c490c987472c971a8d124c396a037aeecb91806be4af3ad6d2705a5948ffd8ed` |
| `SHA256SUMS.txt` | `14c386700ab2ac8d4edde416bbd04fa8717fcacbf9944d2782de9cc524842e72` (بعد المرحلة 0؛ و `20b3783e...f08b` بدونها) | `b867e95a0ba5fe2c72a6e642f60d9377720dd0a0e8df0d44e9e5939f48f84a1c` (عند decdb5d9؛ ويبقى كذلك في التزام الدمج ما دام الموقع لم يتغيّر) |

يختلف `SHA256SUMS.txt` الجديد عن ملف main في سطرَي الخصوصية فقط (وهو يحمل أصلًا أسطر التنزيلات الأربعة الخاصة بـ PR #121،
فإن لم تُنفَّذ المرحلة 0 أجرت هذه المرحلة تغييرها أيضًا).

1. في GitHub ثم Actions ثم «pocol.net cPanel package»، افتح التشغيل على main لالتزام الدمج (حدث push) ثم Summary ثم
   Artifacts: نزّل `pocol.net-cpanel-<أول 12 حرفًا من التزام الدمج>` وفكّه مرة واحدة على جهازك. يُحتفظ بهذا الملف 30 يومًا
   (`retention-days: 30`): نزّله خلال 30 يومًا من الدمج؛ وإن انتهت مدته فاسأل المالك قبل أي `workflow_dispatch` (التشغيل
   الجديد بناء جديد). فيه `PoCol-pocol.net-site-<تاريخ الالتزام>-<12 حرفًا>.zip` وملف `.sha256` الخاص به، و
   `pocol.net.htaccess` و `MANIFEST.json` و `PACKAGE-SHA256SUMS.txt`، والدليلان المعبّآن `DEPLOY-pocol.net-EN.md` و
   `-AR.md`. البناء نفسه عند decdb5d9 أعطى `PoCol-pocol.net-site-2026-11-26-decdb5d96d60.zip`: يذكر `SHA256SUMS.txt` فيه
   58 ملفًا، هي ملفات الأرشيف الخمسة والأربعون الأخرى، و `.htaccess` (المرفق بجانب الأرشيف باسم `pocol.net.htaccess`)،
   و12 تنزيلًا لا يتضمنها الأرشيف؛ وقد طابقت الملفات الخمسة والأربعون و `.htaccess` بصماتها، وطابقت أسطر التنزيلات الاثنا
   عشر التنزيلات المنشورة.
2. اتبع `DEPLOY-pocol.net-AR.md` (أو `-EN.md`) المرفق بالحزمة من الخطوة 0 إلى الخطوة 9: افحص الحزمة، وخذ النسخة الاحتياطية
   (ودوّن `STAMP`)، وارفع، وفكّ الأرشيف، وادمج `.htaccess`، واضبط الصلاحيات، ثم الخطوة 7 حتى تطبع `host_files.py`
   العبارة `OK pocol.net`. لم تُشغَّل `host_files.py` على خادم الإنتاج بعد: قائمتها، لا هذه الخطة، هي التي تحدد ما يُحذف
   أو يُرفع من جديد.

وإلى جانب الملفات الثلاثة أعلاه، تعيد خطوات الدليل كتابة كل ملفات الأرشيف (بعد المرحلة 0 تُكتب الملفات الأربعة والأربعون
الأخرى بالبايتات التي يقدّمها الإنتاج أصلًا)، وقد تغيّر `.htaccess` (الخطوة 5)، وتضبط الصلاحيات (الخطوة 6)، وتحذف الملفات
التي تعلّمها `host_files.py` بعلامة `DELETE` (الخطوة 7). دوّن ما غيّرته الخطوات 5 إلى 7: فالاسترجاع يتوقف عليه.

**البوابة:** الخطوة 9 من ذلك الدليل، ومن جهاز آخر تطبع الأوامر `curl -s https://pocol.net/privacy.html | sha256sum` و
`curl -s 'https://pocol.net/privacy.js?v=c490c987472c' | sha256sum` و `curl -s https://pocol.net/SHA256SUMS.txt |
sha256sum` القيم الثلاث الجديدة أعلاه؛ وتعرض الصفحة «Last updated: 25 November 2026» بالإنجليزية و«آخر تحديث: 25 نوفمبر
2026» بالعربية، وبند خريطة المعدّنين في اللغتين. ويبقى `chrome-extension.html` مختلفًا عبر HTTP بسبب إعادة كتابة Cloudflare
للبريد فقط (أعلاه)، وهذا ليس فشلًا لهذه المرحلة.

**Cloudflare:** الملفان `privacy.html` و `SHA256SUMS.txt` من النوع DYNAMIC (بلا تخزين مؤقت)، و `privacy.html` يربط
`privacy.js?v=c490c987472c` (إصدار بحسب المحتوى)، فلا حاجة إلى مسح التخزين المؤقت. وإن أظهر أحد الفحوص البايتات القديمة،
فامسح هذه العناوين من لوحة Cloudflare (منطقة pocol.net ثم Caching ثم Configuration ثم Purge Cache ثم Custom Purge):
`https://pocol.net/privacy.html` و `https://pocol.net/privacy.js?v=c490c987472c` و `https://pocol.net/SHA256SUMS.txt`.

**الاسترجاع:** استرجع أولًا المرحلة 4 ثم المرحلة 3 ثم المرحلة 2 إن كانت مثبّتة: يجب أن تصف السياسة المنشورة ما تفعله
البوابة. ثم الخطوة 10 من `DEPLOY-pocol.net-AR.md` بقيمة `STAMP` من خطوته 2. ولا تكفي إعادة الملفات الثلاثة السابقة وحدها
إلا إذا لم تحذف الخطوة 7 شيئًا (لا `DELETE`) وأبقت الخطوة 5 الملف `.htaccess` كما كان بايتًا ببايت (`privacy.html`
‏`370e76bf...` و `privacy.js` ‏`e12dff24...` و `SHA256SUMS.txt` ‏`14c38670...`، أو `20b3783e...f08b` إن لم تُنفَّذ
المرحلة 0؛ من النسخة الاحتياطية في الخطوة 2 أو من main عند 4b17da2c)، مع التحقق منها بأوامر curl نفسها.

### المرحلة 2: بوابة المستكشف، بلا قاعدة GeoIP وبلا أدوات تقارير

فقط بعد نشر المرحلة 1. الأقسام 0 و2 و3 و4، **بلا `--geoip-db`** (يُتخطّى القسم 1):

```sh
D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" \
  && sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --check
sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D"
```

النسخة الجديدة من 14 ملفًا: مقارنةً بـ main تستبدل `assets/index-e46a96e9d375.js` بالملف `assets/index-0ce9ca52c312.js`
(`0ce9ca52c312c1fa542d62dde06b6f0017022f90c1e1c60f681104467defff35`)، وتضيف `assets/pocol-insights.js`
(`e37a622126508836a62d4b4fe6373ac0716ef46064ae81a4b4c83622c04c7efa`) و `assets/pocol-insights.css`
(`dca8ac3be3dc45393b1c125ed487a44ec8be1c9738b1d08b594fb988ab3b2280`)، وتغيّر `index.html` (`89335937...` ←
`4b85f4b76b19d87dfd0d6e40a5ddf7bf4eb6a9b38c53f6874efaf571892dce86`). لقطة بنتها `site_snapshot.py` عند decdb5d9 (رأس
PR #120) طابقت الأسطر الأربعة عشر كلها في `deploy/websites/explorer.pocol.net/SHA256SUMS.txt`، ومعرّفها
`site-c29adcf97b30`، ويعطي التزام الدمج المعرّف نفسه ما دامت هذه الملفات الأربعة عشر لم تتغيّر.

**بوابة القرار** (أي فشل يعني التوقف والاسترجاع):

- يطبع القسم 2 السطر `CHECK PASSED: network <chain ID> <genesis hash> on the node, site-c29adcf97b30 (14 files, this
  checkout's snapshot), ...; the GeoIP database not installed (nothing is located), operator node reports disabled (no
  credential).`
- يطبع القسم 3 الأسطر `INSTALLED: ...` و `PASS: ...` و `Miner map: the GeoIP database not installed (nothing is
  located), operator node reports disabled (no credential).` و `Backup: ...` (دوّن هذا المسار: إنه استرجاع هذه المرحلة).
- فحوص القسم 4 كما هي: `/api/health.php` يعطي `"result":"PASS"`؛ و `/api/holders.php` يعطي 503 بالرسالة «PCOL holders
  are unavailable: ...» حتى أول لقطة، ثم `"state":"syncing"` ثم `"ready"`؛ و `/api/holders-stats.php` و
  `/api/miner-geography.php` يجيبان للشبكة (أو، مثل `holders.php`، يعطيان رد 503 بصيغة JSON حتى أول لقطة، وهذا ليس
  توقفًا)؛ و `POST /api/geo-report.php` يعطي 503؛ وبعد أول لقطة، في `/api/miner-geography.php` يساوي `miners.registered`
  عدد سجلات سجلّ المعدّنين (registry)، و `located` صفر، و `unknown` يساوي `registered`، و `database` تساوي null؛ ولا
  يظهر في أي رد عنوان IP أو رمز مشتق بمفتاح أو عنوان معدّن؛ وفحوص المتصفح بالإنجليزية وبالعربية؛ وسجل الخدمة.
- الأقسام الموجودة من قبل (Overview، ما عدا بطاقة الخريطة فيه، وTransactions) تبدو وتعمل كما كانت.

لم تُقَس مدة المرور الكامل الأول لفهرس الحائزين من التكوين على PoCol Mainnet: تكتمل المرحلة 2 حين يصبح
`/api/holders.php` في الحالة `ready` ويُظهر `holders-stats.php` القيمة `verification.complete` صحيحة (القسم 4)، ثم تُراقَب
كما في القسم 5.

**التغيير المرئي المتوقع:** قسم Holders جديد (بند واحد جديد في القائمة الجانبية للمستكشف)؛ وقسم Miner map الموجود وبطاقة
الخريطة في Overview يعرضان الآن الخريطة الجديدة، التي تعرض كل معدّن مسجّل على أنه «غير معروف» (Unknown) وتقول إنه لا توجد
قاعدة GeoIP مثبّتة (ولا يظهر إسناد DB-IP بلا قاعدة).

**الاسترجاع:** 6.1 (التثبيت الفاشل يسترجع نفسه) أو 6.2 بالنسخة الاحتياطية لهذه المرحلة.

**قبل المرحلة 3 أو 4:** احتفظ بنسخة من نسخة المستكشف التي يعيدها 6.2 بالنسخة الاحتياطية لهذه المرحلة، خارج
`/srv/pocol-explorer` (تنظيف المثبّت لا يحذف إلا مجلدات `site-*` هناك):

```sh
sudo cp -a /srv/pocol-explorer/site-1c9ec5374487 /var/backups/pocol-explorer-gateway/site-1c9ec5374487.keep
```

### المرحلة 3 (اختيارية): قاعدة GeoIP

فقط بعد موافقة المالك على التنزيل وعلى الإسناد وفق CC BY 4.0، وبعد نشر المرحلة 1. القسم 1 (التنزيل والبناء)، ثم الأقسام 2
إلى 4 **مع** `--geoip-db ~/geoip/dbip-country-YYYY-MM.bin`، من نسخة التزام الدمج نفسها. لا تتغيّر نسخة الموقع ولا الوحدة
(المعرّف نفسه، والمثبّت يعيد استخدام النسخة)؛ يضيف المثبّت `/opt/pocol-explorer-gateway/geoip/dbip-country.bin` ويعيد تشغيل
البوابة، التي تقرأ ملف فهرس الحائزين الموجود لديها، ويعيد تحميل Caddy.

**البوابة:** سطر `BUILT ...` في القسم 1 مع بصمة SHA-256 المسجّلة عند التنزيل؛ وسطر `CHECK PASSED` في القسم 2 يذكر القاعدة
(release YYYY-MM مع عدد نطاقات IPv4 وIPv6)؛ وسطر `Miner map:` في القسم 3 يذكرها؛ والقسم 4 مع `database.release` بالشهر
المبني، وظهور «IP Geolocation by DB-IP» في الخريطة مع رابط https://db-ip.com، و«CC BY 4.0» مع رابط https://creativecommons.org/licenses/by/4.0/، وإشعار تحويل القاعدة إلى صيغة PoCol الداخلية، وبقاء المعدّنين «غير معروفين» (`located`
صفر بلا أداة تقارير).

**الاسترجاع:** ليس في المثبّت خيار يحذف قاعدة GeoIP: التثبيت بلا `--geoip-db` يُبقي القاعدة المثبّتة (`Miner map: the GeoIP
database kept`). والبوابة لا تفتح الملف إلا عند بدء تشغيلها (في `gateway.py` تعيد `pocol_geoip.load_database` القيمة None
حين لا يوجد ملف)، لذلك:

```sh
sudo systemctl stop pocol-explorer-gateway.service
sudo rm -f /opt/pocol-explorer-gateway/geoip/dbip-country.bin
sudo rmdir /opt/pocol-explorer-gateway/geoip 2>/dev/null || true               # فقط إن أصبح فارغًا
sudo systemctl start pocol-explorer-gateway.service
```

بعدها يجيب `/api/miner-geography.php` بأن `database` تساوي null، وتقول الخريطة إنه لا توجد قاعدة GeoIP مثبّتة ويختفي
الإسناد، ويطبع أي تثبيت لاحق بلا `--geoip-db` العبارة `not installed (nothing is located)`. ولا يُمسّ فهرس الحائزين ولا
الوحدة ولا ملف Caddy ولا نسخة الموقع. (القسم 6.2 بالنسخة الاحتياطية لهذه المرحلة يحذف القاعدة أيضًا، لأن نسختها الاحتياطية
تذكر الملف بلا نسخة سابقة، لكن خطواته الأخيرة تحذف كذلك ملف فهرس الحائزين، فيعيد قراءة السلسلة من التكوين.) وللرجوع من
تحديث شهري إلى قاعدة الشهر السابق بدلًا من ذلك، شغّل الأقسام 2 إلى 4 بملف `--geoip-db` الخاص بالشهر السابق.

### المرحلة 4 (اختيارية، لكل خادم مؤسس على حدة): أدوات تقارير المشغّلين

فقط بعد المرحلة 3 (تُعطى أداة التقارير تلك القاعدة: يرفض `install_geo_reporter.py` العمل بلا `--database`) ومع نشر
المرحلة 1. القسم 7، بموافقة المالك لكل خادم مؤسس؛ والجزء الخاص بالبوابة (القسم 7، الخطوة 2) يعيد الأقسام 2 إلى 4 (بلا
`--geoip-db`، وهذا يُبقي القاعدة). **البوابة:** القسم 7، الخطوة 4، لذلك الخادم. **الاسترجاع:** القسم 7، الخطوة 5 (على
البوابة احذف `geo-reporters.json` ثم أعد تشغيل المثبّت؛ ولا تحذف الملف وحده أبدًا). ثم على الخادم المؤسس ومع المالك احذف
`/etc/pocol-geo-reporter/credential.json` (الذي يُبقيه `--uninstall`؛ والمجلد إن أصبح فارغًا) والقاعدة المنسوخة
`~/geoip/dbip-country-YYYY-MM.bin` والنسخ الاحتياطية للمثبّت `/var/backups/pocol-geo-reporter`؛ وعلى البوابة تحقّق من أن
`/etc/systemd/system/pocol-explorer-gateway.service.d/geo-reporters.conf` قد حُذف وأن `POST /api/geo-report.php` يعطي 503
من جديد.

### الاسترجاع بعد مرحلة لاحقة

استرجع بالترتيب العكسي (المرحلة 4، ثم 3، ثم 2، ثم 1). كل تشغيل للمثبّت بعد المرحلة 2 (المرحلة 3، وخطوة البوابة في المرحلة 4،
والتحديث الشهري) لا يُبقي إلا النسخة الجديدة والنسخة التي كان ملف Caddy يقدّمها قبله، فيحذف
`/srv/pocol-explorer/site-1c9ec5374487`، وهي النسخة التي يعيدها القسم 6.2 إلى ملف Caddy بالنسخة الاحتياطية للمرحلة 2. وقبل
إعادة تحميل Caddy في 6.2 (سطر `sudo caddy validate ... && sudo systemctl reload caddy`) أعد النسخة المحفوظة قبل المرحلة 3
أو 4:

```sh
sudo cp -a /var/backups/pocol-explorer-gateway/site-1c9ec5374487.keep /srv/pocol-explorer/site-1c9ec5374487
```

وبلا تلك النسخة المحفوظة، تعطي ملفات المستكشف الثابتة 404 بعد 6.2 حتى تعود النسخة: من نسخة Git لـ main عند 4b17da2c
(ومثبّتها يقبل وحدة 04f77806 التي أعادها 6.2)، شغّل أوامر القسمين 2 و3 بلا `--geoip-db`، فيعيد بناء `site-1c9ec5374487`
ويفحصها عبر HTTPS. هذا المسار مستنتج من الشيفرة ولم يُختبر.

### الملخص

| المرحلة | ما يتغيّر | من ينفّذها | المدة | البوابة | الاسترجاع |
|---|---|---|---|---|---|
| 0 | `pocol.net/SHA256SUMS.txt` (أسطر التنزيلات الأربعة) | المالك، من File Manager في cPanel | لم تُقَس (رفع ملف واحد) | بصمة curl ‏`14c38670...2e72` | إعادة `SHA256SUMS.txt.before` ‏(`20b3783e...f08b`) |
| 1 | في pocol.net: `privacy.html` و `privacy.js` و `SHA256SUMS.txt`؛ وإعادة كتابة باقي ملفات الأرشيف، و `.htaccess` (الخطوة 5)، والصلاحيات، وملفات `DELETE` في الخطوة 7 | المالك، من cPanel | لم تُقَس | الخطوة 9 من دليل cPanel، والبصمات الثلاث | الخطوة 10 من دليل cPanel (والملفات الثلاثة السابقة وحدها فقط إن لم تحذف الخطوة 7 شيئًا ولم يتغيّر `.htaccess`) |
| 2 | ملفات البوابة ووحدتها، ونسخة المستكشف `site-c29adcf97b30`، وجذر موقع Caddy، وملف فهرس الحائزين؛ إعادة تشغيل البوابة وإعادة تحميل Caddy | مشغّل لديه sudo على خادم البوابة | لم تُقَس؛ يعيد فحص المثبّت المحلي المحاولة نحو 10 ثوانٍ (20 محاولة بفاصل 0.5 ثانية) وفحص HTTPS حتى `--https-wait` (الافتراضي 600 ثانية)؛ ومدة المرور الكامل الأول لفهرس الحائزين لم تُقَس | مخرجات القسمين 2 و3، والقسم 4 | 6.1 أو 6.2 |
| 3 (اختيارية) | `/opt/pocol-explorer-gateway/geoip/dbip-country.bin`؛ إعادة تشغيل البوابة وإعادة تحميل Caddy | مشغّل لديه sudo على خادم البوابة | مدة بناء GeoIP لم تُقَس؛ والتثبيت كما في المرحلة 2 | `BUILT` في القسم 1، والأقسام 2 إلى 4 مع القاعدة | الإيقاف وحذف الملف والتشغيل (أعلاه) |
| 4 (اختيارية) | لكل خادم مؤسس: خدمة أداة التقارير ومؤقّتها وملف الاعتماد؛ وعلى البوابة `geo-reporters.json` وملف الإضافة للوحدة؛ إعادة تشغيل البوابة وإعادة تحميل Caddy | المالك لكل خادم؛ ومشغّل على البوابة | لم تُقَس (المؤقّت كل 60 ثانية) | القسم 7، الخطوة 4 | القسم 7، الخطوة 5، ثم ملف الاعتماد والقاعدة المنسوخة |

### ما لا يحدث أبدًا في هذا الإصدار

- لا تغيير في الإجماع ولا في التكوين (genesis) ولا في إعدادات PoCol Mainnet (`network.json` و `forks.json` ومعرّف السلسلة
  وبصمة التكوين).
- لا إعادة تشغيل للعقدة: البوابة تقرأ عقدة الخادم فقط، ولا يُوقَف `pocold` ولا يُعاد تشغيله.
- لا تغيير على أي معدّن أو خادم مؤسس خارج المرحلة 4.
- لا تغيير في الهوية البصرية: الشعار واللون `#1AA9E4` والخطوط وبنود القوائم الموجودة تبقى كما هي؛ والإضافة الوحيدة هي بند
  Holders في المستكشف (تعليمة المالك 2026-11-25 ‏D3)، بالأسلوب الموجود (D4).
- لا إعادة بناء ولا إعادة توقيع لأي حزمة إصدار: التنزيلات المنشورة تبقى كما هي بايتًا ببايت.
- لا تغيير في DNS.

## ما الذي يتغيّر على الخادم

| المكان | التغيير |
|---|---|
| `/opt/pocol-explorer-gateway/` | تحديث `gateway.py` و `explorer_api.py`، وإضافة `pocol_holders.py` و `pocol_geoip.py` (المثبّت يكتب الملفات الستة) |
| `/opt/pocol-explorer-gateway/geoip/dbip-country.bin` | قاعدة GeoIP، مع الخيار `--geoip-db` فقط |
| `/etc/systemd/system/pocol-explorer-gateway.service` | يضيف `Environment=POCOL_HOLDERS_DB=/var/lib/pocol-explorer-gateway/holders.sqlite3` والوصف الجديد؛ والباقي، ومنه `MemoryMax=256M`، كما هو |
| `pocol-explorer-gateway.service.d/geo-reporters.conf` | فقط ما دام الملف `/etc/pocol-explorer-gateway/geo-reporters.json` موجودًا (تقارير المشغّلين؛ ليست جزءًا من هذا النشر) |
| `/srv/pocol-explorer/site-<id>` | نسخة ثابتة جديدة من 14 ملفًا؛ ويُحتفظ بالنسخة السابقة للاسترجاع حتى التشغيل التالي للمثبّت، الذي يحذفها (انظر «الاسترجاع بعد مرحلة لاحقة») |
| `/etc/caddy/Caddyfile` | جذر الموقع في الكتلة المُعلَّمة فقط: مسارات `/api/*` تذهب أصلًا إلى البوابة ضمن حد 16 كيلوبايت |
| `/var/lib/private/pocol-explorer-gateway/holders.sqlite3` (مع `-wal` و `-shm`) | تُنشئه الخدمة عند أول تشغيل |

المسارات الجديدة: `/api/holders.php` و `/api/holders-stats.php` و `/api/miner-geography.php` (GET و HEAD) و
`POST /api/geo-report.php` (يعطي 503 حتى تُفعَّل تقارير المشغّلين).

## 0. تسجيل الحالة الحالية (من أجل الاسترجاع)

```sh
git rev-parse HEAD
sudo systemctl status pocol-explorer-gateway.service --no-pager
sudo cat /etc/systemd/system/pocol-explorer-gateway.service
grep -n 'root \*' /etc/caddy/Caddyfile
ls /srv/pocol-explorer/
curl -s --max-time 20 https://explorer.pocol.net/api/health.php
df -h / /var/lib /srv /opt; free -m
systemctl show pocol-explorer-gateway.service -p MemoryCurrent -p CPUUsageNSec
ps -o pid,rss,pcpu,etime,cmd -C pocold
```

احفظ هذه المخرجات مع التاريخ؛ فهي خط الأساس لاستهلاك البوابة والعقدة في الخطوة 5.

## 1. قاعدة GeoIP (اختيارية؛ فقط بعد موافقة المالك على التنزيل والإسناد)

بدونها يعمل التحديث أيضًا: لا يُحدَّد أي موقع، وتقول الخريطة إنه لا توجد قاعدة GeoIP مثبّتة.

1. على جهاز يختاره المالك، نزّل ملف CSV لقاعدة DB-IP «IP to Country Lite» للشهر الحالي من صفحة التنزيل
   (https://db-ip.com/db/download/ip-to-country-lite). ترخيصها CC BY 4.0: تعرض الخريطة «IP Geolocation by DB-IP» مع
   رابط https://db-ip.com، و«CC BY 4.0» مع رابط الترخيص https://creativecommons.org/licenses/by/4.0/، وإشعار تحويل القاعدة إلى
   صيغة PoCol الداخلية، كلما كانت القاعدة مثبّتة. سجّل بصمة SHA-256 للملف (`sha256sum`).
2. انسخه إلى خادم البوابة وابنِ القاعدة الثنائية من نسخة الإصدار (دون اتصال بالشبكة؛ يُفحص الملف سطرًا سطرًا ويتحقق
   `build` من النتيجة):

   ```sh
   install -d -m 0700 ~/geoip
   python3 -I -B deploy/rewards/pocol_geoip.py build --input ~/geoip/dbip-country-lite-YYYY-MM.csv.gz \
       --release YYYY-MM --output ~/geoip/dbip-country-YYYY-MM.bin
   python3 -I -B deploy/rewards/pocol_geoip.py info ~/geoip/dbip-country-YYYY-MM.bin
   ```

   يطبع `build` السطر `BUILT ...: release YYYY-MM, N IPv4 and M IPv6 ranges, source SHA-256 ...`، ويجب أن تطابق تلك
   البصمة ما سجّلته في 1.

**التحديث الشهري.** كرّر 1 و2 بملف الشهر الجديد، ثم الخطوات 2 إلى 4 أدناه بالخيار `--geoip-db` الجديد (يفحص المثبّت
القاعدة سجلًا سجلًا ويستبدلها ويعيد تشغيل البوابة). التحديث بلا `--geoip-db` يُبقي القاعدة المثبّتة، والحقل
`database.release` في الرد يذكر الشهر المستخدم.

## 2. الفحص المسبق (لا يغيّر شيئًا)

```sh
D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" \
  && sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --geoip-db ~/geoip/dbip-country-YYYY-MM.bin --check
```

(احذف `--geoip-db` إن لم تكن هناك قاعدة.) يجب أن يطبع `CHECK PASSED: network <chain ID> <genesis hash>` (المعرّف والبصمة كما يعلنهما `deploy/websites/site-identity.json`) مع
`site-<المعرّف الجديد> (14 files, this checkout's snapshot)` وسطر قاعدة GeoIP و
`operator node reports disabled (no credential)`. ويرفض دون أي تغيير: ملف وحدة معدّلًا يدويًا (يستبدل فقط وحدات هذا
الإصدار و04f77806 و1bb11f4c)، ومجلد `/opt/pocol-explorer-gateway` غير معروف، وكتلة Caddy معدّلة، وقاعدة تفشل فحصها،
وعقدة لشبكة أخرى، ونسخة ليست نسخة هذا الالتزام.

## 3. التثبيت

```sh
sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --geoip-db ~/geoip/dbip-country-YYYY-MM.bin
```

يحفظ نسخة احتياطية مما يستبدله في `/var/backups/pocol-explorer-gateway/<التاريخ>-<الوقت>-<pid>/`
(`Caddyfile.before` و `file-<n>.before` و `state.json`)، ويكتب الملفات والقاعدة والوحدة، ويعيد تشغيل البوابة ويفحصها
محليًا، ثم يعيد تحميل Caddy ويفحص عبر HTTPS بالاسم العام وشهادة موثوقة كل ملف ثابت والترويسات و `/api/health.php`
و `/api/rpc.php` و `/rpc` و `/health` و `/api/holders.php` و `/api/miner-geography.php` (بياناتهما أو 503 ما دام الفهرس
بلا لقطة) والمسارات المخفية ومسارَي Studio والصنبور إن وُجدا. يطبع `INSTALLED` و `PASS` و `Miner map` و
`Backup: /var/backups/pocol-explorer-gateway/...`. **دوّن مسار النسخة الاحتياطية.** عند أي فشل يسترجع الحالة السابقة
وحده (`INSTALL FAILED`)؛ وإذا تعثّرت خطوة استرجاع يطبع `ROLLBACK INCOMPLETE`، فاتبع عندها 6.2.

## 4. التحقق

من جهاز آخر:

```sh
curl -s --max-time 20 https://explorer.pocol.net/api/health.php
curl -s --max-time 20 'https://explorer.pocol.net/api/holders.php?size=25' | head -c 600
curl -s --max-time 20 https://explorer.pocol.net/api/holders-stats.php | head -c 900
curl -s --max-time 20 https://explorer.pocol.net/api/miner-geography.php | head -c 900
curl -s -o /dev/null -w '%{http_code}\n' -X POST -H 'Content-Type: application/json' --data '{}' \
  https://explorer.pocol.net/api/geo-report.php
```

- بعد التثبيت مباشرة يقرأ فهرس الحائزين السلسلة من التكوين: يجيب `/api/holders.php` بالرمز 503 حتى أول لقطة، ثم
  `"state":"syncing"` ثم `"ready"`. مدة ذلك على PoCol Mainnet لم تُقَس.
- عند الجاهزية: `"chainId"` بمعرّف السلسلة المعلن، وارتفاع اللقطة قريب من رأس العقدة، و `verification.complete` صحيحة مع
  `unlocatedWei` تساوي "0". الحسابات الوحيدة في تكوين PoCol Mainnet هي المؤسسون الأربعة، فأي فرق يعني حسابًا لا يمكن
  اكتشافه (F-104): يُنشر ولا يُخفى؛ سجّله مع ارتفاعه.
- `/api/miner-geography.php`: `miners.registered` يساوي عدد سجلات المعدّنين، و `located` صفر و `unknown` يساوي
  `registered` (لا مبلّغين)، والمصدر `explorer` حديث، و `database.release` هو الشهر الذي بنيته (أو `database` تساوي
  null بلا قاعدة). لا يظهر في الرد أي عنوان IP أو رمز مشتق بمفتاح أو عنوان معدّن.
- رمز الاستجابة الأخير 503 (التقارير غير مفعّلة).
- في المتصفح: قسم Holders وقسم Miner map وبطاقة الخريطة في Overview، بالإنجليزية وبالعربية (مفتاح EN | العربية).
- على الخادم: `sudo journalctl -u pocol-explorer-gateway.service -n 50 --no-pager` (تحذيرات `holders index: ...`، ولا
  يُسجَّل أي طلب أو عنوان).

## 5. ما يجب مراقبته بعد النشر (الأيام الأولى)

| ماذا | كيف | المتوقع؛ ومتى تتصرف |
|---|---|---|
| حالة الفهرس | `curl -s https://explorer.pocol.net/api/holders.php \| head -c 400` | `ready`، وارتفاع اللقطة يتقدّم خلف الرأس ببضع كتل؛ بقاء `reconnecting` أكثر من بضع دقائق يعني أن العقدة لا تجيب |
| الاكتمال | `verification` في `holders-stats.php` | `complete` صحيحة؛ أي فرق يُنشر (F-104)، وما دام منشورًا تُقرأ كل الأرصدة المعروفة كل 60 ثانية، فيتأخر الرصيد المنشور نحو دقيقة واحدة مضافًا إليها مدة لقطة واحدة |
| المسح الطويل | `sudo journalctl -u pocol-explorer-gateway.service --no-pager \| grep 'longer than the state window'` | لا شيء، أو أسطر قليلة أثناء المسح الأول (الكامل): كل توقف يحتفظ بما قرأه ويكمل عند كتلة أحدث، ويبقى ارتفاع اللقطة يتقدّم. إذا لم يتقدّم أبدًا فالمحاولة الواحدة لا تستطيع قراءة كل العقود ومعرّفات السجل وعناوين مكافآتها وعناوين النظام والعناوين التي ظهرت منذ المحاولة السابقة خلال 28 كتلة (نحو 4.7 دقائق؛ README، قسم Holders index): أبلغ عنه مع أسطر السجل |
| المكافآت | `rewards.from` في `holders-stats.php` | ارتفاع بدء الفهرس؛ null تعني فجوة تجاوزت نافذة 32 كتلة (F-103): أعد ضبطها بالأمر `pocol_holders.py reset-rewards` (README، قسم Holders index)، ولا يُمسّ شيء غيرها |
| ذاكرة البوابة ومعالجها | `systemctl show pocol-explorer-gateway.service -p MemoryCurrent -p CPUUsageNSec` و `systemd-cgtop -b -n 1` | أقل بكثير من `MemoryMax=256M` (المقيس: لقطة 100,000 حساب أقل من 50 ميغابايت)؛ استرجع إن اقتربت من الحد أو أُعيد تشغيل الخدمة |
| حمل العقدة | `ps -o rss,pcpu,etime -C pocold` ورأس `pocol_status` وأقرانها | تبقى عقدة المراقبة متزامنة مع أقرانها؛ كل كتلة جديدة تضيف قراءة مكافآت واحدة (`snapshot_v3`)، وكل لقطة تقرأ الأرصدة التي تغيّرت |
| القرص | `sudo du -sh /var/lib/private/pocol-explorer-gateway/` | نحو 23 ميغابايت لكل لقطة محفوظة عند 100,000 حساب؛ ومع نحو 19 لقطة محفوظة عند كتل كل 10 ثوانٍ (آخر 3 وكل لقطة عمرها أقل من 180 ثانية) المقيس 484 ميغابايت؛ حسابات PoCol Mainnet اليوم أقل بكثير (بضعة ميغابايت) |
| ردود الانشغال | تنبيهات «busy» في المستكشف | 429 مع `Retry-After: 3` تحت الحمل فقط |

## 6. الاسترجاع

### 6.1 تلقائي

التثبيت الفاشل يسترجع الحالة السابقة وحده (الخطوة 3): يعيد تحميل إعداد Caddy السابق ويعيد تشغيل الخدمة السابقة. لا يلزم
غير حذف ملف فهرس الحائزين الذي قد تكون البوابة الجديدة أنشأته قبل الفشل، والبوابة السابقة تتجاهله:
`sudo rm -f /var/lib/private/pocol-explorer-gateway/holders.sqlite3{,-wal,-shm}`.

### 6.2 بعد تثبيت ناجح (يدوي)، بالنسخة الاحتياطية من الخطوة 3

جُرِّبت هذه الخطوات في بيئة معزولة: المثبّت الحقيقي على مسارات مؤقتة مع تزييف كل أوامر النظام (أداة اختبارات
`test_explorer_gateway.py`)، تحديثًا لخادم يشغّل وحدة 04f77806 مع `--geoip-db`، ثم هذه الخطوات؛ فعادت كل المسارات إلى
بايتاتها السابقة وعاد ملف Caddy إلى النسخة السابقة من الموقع. لم تُشغَّل على خادم حقيقي.

```sh
B=/var/backups/pocol-explorer-gateway/<النسخة الاحتياطية من الخطوة 3>
sudo cat "$B/state.json"
sudo systemctl stop pocol-explorer-gateway.service
sudo python3 -I -B - "$B" <<'PY'
import json, os, pathlib, sys
backup, caddyfile = pathlib.Path(sys.argv[1]), pathlib.Path('/etc/caddy/Caddyfile')
state = json.loads((backup / 'state.json').read_text())
for i, name in enumerate(state['paths']):
    saved, target = backup / f'file-{i}.before', pathlib.Path(name)
    if saved.exists():
        target.write_bytes(saved.read_bytes()); os.chmod(target, 0o644); print('restored', target)
    elif target.exists():
        target.unlink(); print('removed new', target)
caddyfile.write_bytes((backup / 'Caddyfile.before').read_bytes()); os.chmod(caddyfile, 0o644)
print('restored', caddyfile, '- previous site:', state['previousSite'], '- new site:', state['site'])
PY
sudo rmdir /opt/pocol-explorer-gateway/geoip 2>/dev/null || true
sudo rmdir /etc/systemd/system/pocol-explorer-gateway.service.d 2>/dev/null || true
sudo rm -f /var/lib/private/pocol-explorer-gateway/holders.sqlite3 \
  /var/lib/private/pocol-explorer-gateway/holders.sqlite3-wal /var/lib/private/pocol-explorer-gateway/holders.sqlite3-shm
sudo systemctl daemon-reload
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile && sudo systemctl reload caddy
sudo systemctl start pocol-explorer-gateway.service
```

ثم تحقّق: `grep -n 'root \*' /etc/caddy/Caddyfile` يذكر `site-<المعرّف القديم>` (يحتفظ المثبّت بتلك النسخة، لكن حتى تشغيله التالي فقط: بعد تشغيل
لاحق للمثبّت أعدها أولًا كما في «الاسترجاع بعد مرحلة لاحقة»)، وملف
الوحدة هو المسجّل في الخطوة 0، و `/api/health.php` يعطي PASS، والمستكشف يعرض Overview وTransactions كما كان. بعدها يمكن
حذف النسخة الجديدة `/srv/pocol-explorer/site-<المعرّف الجديد>` وحدها. ملف فهرس المعاملات لا يُمسّ. ولا يصلح مثبّت
الإصدار السابق لهذا: فهو يرفض الوحدة الجديدة كتعديل يدوي حتى تعود الوحدة القديمة.

## 7. أداة تقارير المشغّلين على خوادم المؤسسين (فقط بعد موافقة المالك)

ليست جزءًا من هذا النشر. لا تُغيَّر خدمات خوادم المؤسسين إلا بموافقة المالك
([GENESIS-RECORD-2026-11-21.md](../../development/public-network/GENESIS-RECORD-2026-11-21.md)). إن وافق المالك:

1. ينشئ المالك ملحًا سريًا واحدًا للشبكة ومفتاحًا لكل مبلّغ (32 بايتًا عشوائيًا لكل منها بصيغة hex بأحرف صغيرة، مثلًا
   `python3 -c "import secrets; print(secrets.token_hex(32))"`) على الخوادم نفسها، ولا توضع أبدًا في المستودع أو في
   محادثة أو تذكرة.
2. خادم البوابة: الملف `/etc/pocol-explorer-gateway/geo-reporters.json` بالصيغة
   `{"salt": "<الملح>", "reporters": {"founder-1": "<المفتاح 1>", "founder-2": "<المفتاح 2>", "founder-3": "<المفتاح 3>"}}`،
   مملوكًا لـ root بالصلاحية 0600؛ ثم الخطوات 2 إلى 4 من جديد (يضيف المثبّت ملف الاعتماد، ويذكر سطر `Miner map` أن
   التقارير مفعّلة).
3. كل خادم مؤسس (بموافقة المالك لذلك الخادم): الملف `/etc/pocol-geo-reporter/credential.json` بالصيغة
   `{"reporter": "founder-N", "key": "<المفتاح N>", "salt": "<الملح>"}`، مملوكًا لـ root بالصلاحية 0600؛ انسخ القاعدة
   المبنية في الخطوة 1؛ ومن نسخة التزام الإصدار:
   `sudo python3 deploy/explorer/install_geo_reporter.py --database ~/geoip/dbip-country-YYYY-MM.bin --check` ثم الأمر
   نفسه بلا `--check`. يثبّت خدمة الأداة ومؤقّتها (كل 60 ثانية؛ DynamicUser و Nice=19 و MemoryMax=64M وقراءة محلية
   واحدة للعقدة في كل تشغيل) ويشغّل قبل بدء المؤقّت تجربة جافة يطبع تقريرها للمراجعة: رأس سلسلة العقدة وعنوان التعدين الخاص بها ورموز أقرانها ودولهم، بلا أي عنوان IP، ولا يُرسل منها شيء. يحتاج الخادم اتصال HTTPS
   صادرًا إلى explorer.pocol.net، وأي تغيير في الجدار الناري قرار المالك.
4. تحقّق: `systemctl list-timers pocol-geo-reporter.timer` و `journalctl -u pocol-geo-reporter.service -n 5`
   (`SENT: ...`)، وفي `/api/miner-geography.php` يظهر المصدر `founder-N` حديثًا ويُحتسب معدّن ذلك الخادم في
   `miners.located`. المؤسس 4 (حاسوب المالك بنظام Windows) بلا أداة تقارير، فيبقى معدّنه «غير معروف».
5. الإزالة: `sudo python3 deploy/explorer/install_geo_reporter.py --uninstall` على الخادم (يبقى ملف الاعتماد)؛ وعلى
   البوابة احذف `geo-reporters.json` ثم أعد الخطوات 2 إلى 4 (يُحذف ملف الإضافة أيضًا؛ وحذف الملف دون إعادة تشغيل المثبّت
   يمنع البوابة من البدء لأن `LoadCredential=` يحتاجه).
