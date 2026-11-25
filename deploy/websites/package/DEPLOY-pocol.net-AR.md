# نشر الموقع الرسمي pocol.net ({{NETWORK}}) على استضافة cPanel

| | |
| --- | --- |
| الحزمة | `{{ZIP}}` (‏{{ZIP_BYTES}} بايت، {{FILE_COUNT}} ملفًا) |
| بصمة SHA-256 للحزمة | `{{ZIP_SHA256}}` |
| مبنية من الإيداع | `{{COMMIT}}` ‏({{DATE}}) |
| الشبكة | {{NETWORK}}، معرّف السلسلة `{{CHAIN_ID}}`، بصمة genesis ‏`{{GENESIS}}` |

هذه الحزمة للموقع الرسمي pocol.net وحده (دون Explorer وStudio وIDE). لا يُرفع منها شيء ولا يتغير أي خادم أو سجل DNS
إلا حين تنفذ أنت الخطوات التالية. الدليل الإنجليزي `DEPLOY-pocol.net-EN.md` يضم الخطوات نفسها.

## محتوى الحزمة

- `{{ZIP}}`: محتويات جذر الموقع (Document Root). الملفات في أعلى الأرشيف مباشرة دون مجلد يحيط بها، ومعها
  `SHA256SUMS.txt` الذي يسرد كل ملف من ملفات الموقع مع بصمته.
- `{{ZIP}}.sha256`: سطر التحقق من بصمة الأرشيف.
- `{{HTACCESS}}`: ملف `.htaccess` الخاص بالموقع. **ليس** داخل الأرشيف، لأن ملف `.htaccess` على الخادم يحمل إعدادات
  cPanel نفسها، فيُدمج يدويًا (الخطوة 5) ولا يُستبدل أبدًا.
- `MANIFEST.json`: الإيداع وهوية الشبكة وكل ملف بحجمه وبصمته.
- `PACKAGE-SHA256SUMS.txt`: بصمة SHA-256 لكل ملف من ملفات الحزمة.

ملفات الإصدار المحفوظة خارج Git التي تُرفع من الإصدار الموقّع: {{RELEASE_FILES}}

يقدّم GitHub ملف CI ‏`{{ARTIFACT}}` أرشيفًا واحدًا يضم هذه الملفات: فك ضغطه مرة واحدة على جهازك ثم استخدم الملفات
أعلاه. الأرشيف الذي يُرفع هو `{{ZIP}}`، لا أرشيف GitHub المحيط به.

## 0. تحقق من الحزمة على جهازك

    sha256sum -c PACKAGE-SHA256SUMS.txt

يجب أن ينتهي كل سطر بـ `OK`. وعلى Windows ‏(PowerShell): يجب أن يطبع `Get-FileHash {{ZIP}} -Algorithm SHA256`
البصمة `{{ZIP_SHA256}}` (لا يهم حجم الأحرف).

## 1. قبل البدء (cPanel)

1. **جذر الموقع.** من cPanel ثم Domains: دوّن مجلد Document Root الخاص بـ pocol.net (غالبًا `public_html`).
   تسميه الأوامر التالية `DOCROOT`، فاضبطه أولًا في Terminal داخل cPanel:

        DOCROOT="$HOME/public_html"

2. **المواقع المتداخلة.** في القائمة نفسها تحقق هل لنطاق أو نطاق فرعي آخر (ide.pocol.net أو studio.pocol.net) جذر
   موقع *داخل* جذر pocol.net، مثل `public_html/ide`. إن وُجد فذلك المجلد ملك الموقع الآخر ولا يُحذف أبدًا، والأوامر
   التالية تبيّن أين يُذكر (`ide` في الأمثلة).
3. **PHP.** من cPanel ثم MultiPHP Manager: اختر PHP {{PHP_RECOMMENDED}} لـ pocol.net (و{{PHP_MINIMUM}} على الأقل؛ لا
   يعمل `api/network.php` و`api/reward.php` على PHP 8.0). ومن Select PHP Version ثم Extensions (أو MultiPHP INI
   Editor): يجب أن يكون الامتداد `curl` مفعّلًا.
4. **بوابة Explorer.** ملخص الشبكة (`/api/network.php`) ولوحة المكافآت (`/api/reward.php`) وصفحة طلب الرسوم الأولى تقرأ من
   `https://{{GATEWAY}}`. إلى أن تُثبَّت البوابة الجديدة هناك ويُنقل سجل DNS الخاص بها، يجيب الملخص ولوحة المكافآت
   بأن البيانات "غير متاحة مؤقتًا" (HTTP 503) وتفشل طلبات الرسوم لتعذّر الوصول. هذا متوقع وآمن: ترفض الصفحات أي
   بيانات لا يطابق معرّف سلسلتها أو بصمة genesis فيها شبكة {{NETWORK}}. ولا تغيّر هذه الحزمة Explorer.

## 2. خذ نسخة احتياطية من الموقع الحالي

احفظ النسخة الاحتياطية خارج جذر الموقع: أي أرشيف داخله يستطيع أي زائر تنزيله، وهو يضم `api/config.php`.

**من Terminal (الطريقة المفضلة):**

    STAMP=$(date -u +%Y%m%dT%H%M%SZ)
    mkdir -p ~/pocol-backups && cd "$DOCROOT"
    find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ~/pocol-backups/pocol.net-before-$STAMP-files.txt
    tar -czf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz .
    cp -p .htaccess ~/pocol-backups/htaccess-$STAMP 2>/dev/null || echo "no .htaccess on the host"
    echo "$STAMP"

دوّن قيمة `STAMP` المطبوعة. وإن وُجد موقع متداخل (الخطوة 1.2) فأضف `--exclude=./ide` إلى أمر `tar`، واستخدم
`find . -path ./ide -prune -o -type f -print0` في الأمر الأول.

**من File Manager:** من Settings فعّل "Show Hidden Files (dotfiles)". أنشئ في المجلد الرئيسي (خارج جذر الموقع) المجلد
`pocol-backups` ‏(+ Folder). ثم افتح جذر الموقع، ثم Select All (وألغِ تحديد مجلد أي موقع متداخل، الخطوة 1.2)، ثم Compress
ثم Zip Archive. تقترح نافذة الضغط مسارًا للأرشيف داخل جذر الموقع: استبدل المسار كله بـ
`/pocol-backups/pocol.net-backup-YYYYMMDD.zip` قبل الضغط على Compress، حتى لا يُكتب الأرشيف أبدًا حيث يقدّمه الموقع
للزوار (مسارات File Manager تبدأ من المجلد الرئيسي: `/public_html/...` هو جذر الموقع، و`/pocol-backups/...` خارجه).
ثم نزّل إلى جهازك نسخة من الأرشيف ونسخة من `.htaccess`.

في الحالتين نزّل النسخة الاحتياطية إلى جهازك قبل المتابعة.

## 3. ارفع الأرشيف وتحقق منه

ارفع `{{ZIP}}` و`{{ZIP}}.sha256` إلى **المجلد الرئيسي** (لا إلى جذر الموقع) من File Manager ثم Upload. ثم في Terminal:

    cd ~ && sha256sum -c {{ZIP}}.sha256

يجب أن يطبع `{{ZIP}}: OK`.

## 4. فك الأرشيف داخل جذر الموقع

**من Terminal:**

    unzip -o -q ~/{{ZIP}} -d "$DOCROOT"

**من File Manager:** حدّد `{{ZIP}}` في المجلد الرئيسي، ثم Extract، واكتب مسار جذر الموقع (مثل `/public_html`)، ثم
Extract Files، واسمح باستبدال الملفات الموجودة.

لا يضم الأرشيف `.htaccess` ولا `api/config.php` ولا `.user.ini` ولا `php.ini` ولا `error_log` ولا `.well-known/` ولا
`cgi-bin/`، فتبقى ملفات الخادم هذه كما هي. احتفظ بها.

## 5. ادمج ملف `.htaccess`

1. من File Manager ثم Settings فعّل "Show Hidden Files"، وافتح جذر الموقع، ثم Edit على `.htaccess`.
2. أبقِ كتل cPanel كما هي: الكتلة من `# php -- BEGIN cPanel-generated handler, do not edit` حتى
   `# php -- END cPanel-generated handler, do not edit` (يكتبها MultiPHP Manager) وأي تحويل إلى HTTPS.
3. الصق محتوى `{{HTACCESS}}` كاملًا ودون تعديل قبل تلك الكتل أو بعدها، كتلةً واحدة. واحذف نسخة سابقة من توجيهات
   الموقع نفسه إن وُجدت، ثم احفظ.
4. إن لم يكن على الخادم ملف `.htaccess` فارفع `{{HTACCESS}}` إلى جذر الموقع وغيّر اسمه إلى `.htaccess`.
5. نزّل نسخة من الملف المحفوظ (للخطوة 7) وسمّها `host.htaccess`.

## 6. الصلاحيات

المجلدات 755 والملفات 644، لمسارات الحزمة وحدها:

    cd "$DOCROOT"
    unzip -Z1 ~/{{ZIP}} | grep '/$' | tr '\n' '\0' | xargs -0 -r chmod 755
    unzip -Z1 ~/{{ZIP}} | grep -v '/$' | tr '\n' '\0' | xargs -0 -r chmod 644

## 7. تحقق من الملفات واحذف القديمة

لا يكفي فك الأرشيف فوق الموقع القديم؛ فقد تبقى ملفات الشبكة السابقة متاحة للزوار. يجب استبدال ملف دعوة الشبكة القديم بالملف الموقّع للشبكة الرئيسية والتحقق من بصمته وتوقيعه، وليس حذف الملف الجديد. الملفات التي تحمل الحالة DIFFERS تُستبدل بنسختها الصحيحة، أما DELETE فتُحذف بعد المراجعة، وتبقى ملفات KEEP محفوظة.

مسار ملف دعوة الشبكة الذي يجب استبداله، وليس حذفه: `downloads/network.pocol.json`. تحقّق من مطابقة بصمته وتوقيعه للإصدار الموقّع قبل نشر الموقع.

1. كل ملف من ملفات الحزمة في مكانه (لا يُطبع شيء ثم `FILES OK`):

        cd "$DOCROOT"
        grep -v '  \.htaccess$' SHA256SUMS.txt | sha256sum -c --quiet - && echo "FILES OK"

2. أنشئ قائمة بملفات الخادم:

        cd "$DOCROOT"
        find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ~/pocol.net-files.txt

   (مع موقع متداخل: `find . -path ./ide -prune -o -type f -print0 | ...`). ثم نزّل `~/pocol.net-files.txt`.
3. على جهاز فيه نسخة من المستودع عند الإيداع `{{COMMIT}}` (مستنسخة على Windows بالأمر `git -c core.autocrlf=false`)
   نفّذ:

        python3 deploy/websites/tools/host_files.py --site pocol.net --listing pocol.net-files.txt --host-copy .htaccess=host.htaccess

   (مع موقع متداخل أضف `--keep-dir ide`). احذف من الخادم كل ملف بعلامة `DELETE`، وأعد رفع كل ملف بعلامة `DIFFERS` أو
   `MISSING` (من الأرشيف)، وأصلح `.htaccess` إن ظهر بعلامة `MERGE`، ثم أعد إنشاء القائمة وكرر حتى تطبع الأداة
   `OK pocol.net`. الملفات بعلامة `KEEP` (`api/config.php` و`.user.ini` و`php.ini` و`error_log` و`.well-known/`
   و`cgi-bin/` وملف `.htaccess` المدموج) تبقى.

   الملفات المتوقع ظهورها بعلامة `DELETE`، وقد تركها موقع الشبكة السابقة: `README-AR.txt` (ملاحظة الرفع القديمة)،
   و`NETWORK-UPDATE-15000.txt`، و`RETAINED-NATIVE-DOWNLOADS.json`، و`README-MAINNET-UPDATE-AR.txt`،
   و`downloads/PoCol-Apps-SHA256SUMS.txt`، وحزمتا التطبيقات
   السابقتان `downloads/PoCol-Mainnet-0.4.0-Windows.zip` و`downloads/PoCol-Mainnet-0.4.0-Linux.zip`، وحزم الإضافة
   السابقة `downloads/PoCol-Studio-IDE-Viewer-0.4.0.zip` و`downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip`
   و`downloads/PoCol-Studio-Chrome.zip`، وملفات `.sha256` الخاصة بها. والمرجع هو قائمة الأداة.

   **دون المستودع** يسرد هذا الأمر ملفات الخادم التي ليست في الحزمة (راجع القائمة ثم احذفها، عدا ملفات `KEEP` أعلاه
   ومجلد أي موقع متداخل):

        cd "$DOCROOT"
        LC_ALL=C comm -23 <(find . -type f | LC_ALL=C sort) \
          <( (sed 's/^[0-9a-f]\{64\}  /.\//' SHA256SUMS.txt; echo ./SHA256SUMS.txt) | LC_ALL=C sort)

## 8. الإنهاء

- احذف `~/{{ZIP}}` و`~/{{ZIP}}.sha256` من المجلد الرئيسي حين لا تحتاج إليهما. ولا تترك أي أرشيف داخل جذر الموقع.
- إن كانت الاستضافة أو شبكة توزيع المحتوى تخزن الصفحات مؤقتًا (LiteSpeed Cache أو Cloudflare) فامسح ذلك التخزين.

## 9. فحوص الموقع الحي (من المتصفح)

- `https://pocol.net/` بالإنجليزية وبالعربية (من اختيار اللغة)، وكل صفحة في `https://pocol.net/sitemap.xml`: لا صفحة
  ولا صورة مفقودة، ولا تُظهر وحدة تحكم المتصفح (F12) أي خطأ 404 أو خطأ Content-Security-Policy.
- `https://pocol.net/no-such-page` تعرض صفحة 404 الخاصة بالموقع.
- `https://pocol.net/api/config.php` تجيب 403 (أو 404) ولا تعرض أي ملف.
- `https://pocol.net/api/network.php` تجيب بـ JSON فيه `"chainId":{{CHAIN_ID}}` و
  `"genesisHash":"{{GENESIS}}"` بعد تشغيل بوابة Explorer، وقبل ذلك تجيب 503 بالنص
  "Network data is temporarily unavailable." (الخطوة 1.4). و`https://pocol.net/api/reward.php` تجيب بـ JSON، أو 503
  قبل تشغيل البوابة.
- من أدوات المطور ثم Network: ترويسات استجابة الصفحة تضم `Content-Security-Policy` و`X-Content-Type-Options: nosniff`
  و`X-Frame-Options: DENY` (مصدرها ملف `.htaccess` المدموج).
- `https://pocol.net/SHA256SUMS.txt` هو ملف الحزمة: بصمته هي البصمة التي يذكرها `MANIFEST.json` للملف `SHA256SUMS.txt`.

## 10. الاستعادة (الرجوع إلى الموقع السابق)

**من Terminal**، بقيمة `STAMP` من الخطوة 2:

    cd "$DOCROOT"
    tar -xzf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz
    LC_ALL=C comm -13 <(cut -c67- ~/pocol-backups/pocol.net-before-$STAMP-files.txt | LC_ALL=C sort) <(find . -type f | LC_ALL=C sort) > ~/pocol-backups/added-$STAMP.txt
    cat ~/pocol-backups/added-$STAMP.txt

يعيد `tar` كل ملف سابق، ومنه ملف `.htaccess` السابق. ويسرد الأمران الأخيران، في الملف
`~/pocol-backups/added-$STAMP.txt`، الملفات التي أضافتها هذه الحزمة ولم تكن في الموقع السابق. راجع القائمة واحذف منها
كل سطر يجب إبقاؤه (ملف `KEEP`، مثل `error_log` الذي كتبه PHP بعد ذلك). ومع موقع متداخل (الخطوة 1.2) استخدم
`find . -path ./ide -prune -o -type f -print` في الأمر الثالث، حتى لا يُسرد أي ملف من ملفات ذلك الموقع. ثم احذف الملفات
المسرودة، والمجلدات التي أنشأتها هذه الحزمة وصارت فارغة (تبقى مجلدات الموقع السابق، ومنها مجلد `cgi-bin/` الفارغ):

    tr '\n' '\0' < ~/pocol-backups/added-$STAMP.txt | xargs -0 -r rm -f --
    LC_ALL=C comm -13 <(tar -tzf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz | sed -n 's#/$##p' | LC_ALL=C sort) <(find . -mindepth 1 -type d | LC_ALL=C sort) | LC_ALL=C sort -r | tr '\n' '\0' | xargs -0 -r rmdir --ignore-fail-on-non-empty --

(ومع موقع متداخل: `find . -mindepth 1 -path ./ide -prune -o -type d -print` في الأمر الأخير). ثم تحقق:

    sha256sum -c --quiet ~/pocol-backups/pocol.net-before-$STAMP-files.txt && echo "RESTORED"

لا يُطبع إلا `RESTORED` (قد يختلف ملف سجل كتبه PHP بعد النسخ الاحتياطي). ثم امسح التخزين المؤقت كما في الخطوة 8.

**من File Manager:** حدّد أرشيف النسخة الاحتياطية من الخطوة 2 (`/pocol-backups/pocol.net-backup-YYYYMMDD.zip`، وارفعه
إلى هناك من جهازك إن لم يعد موجودًا)، وفكّه داخل جذر الموقع (مع السماح بالاستبدال)، وأعد ملف `.htaccess` المحفوظ إن
لزم، واحذف الملفات والمجلدات الفارغة التي أضافتها هذه الحزمة (بمقارنتها بمحتوى الأرشيف، مع إبقاء مجلد أي موقع متداخل)،
ثم امسح التخزين المؤقت.

بعد الاستعادة لا تنطبق أداة `host_files.py` (فهي تقارن بهذه الحزمة): المرجع هو قائمة ملفات النسخة الاحتياطية نفسها.
