PoCol Explorer — بوابة Explorer وموقعها على خادم البوابة (Sites W8)

هذه الملفات جاهزة للتثبيت ولم تُنفّذ على أي خادم من داخل هذه الجلسة.
التثبيت لا يعدّل بيانات العقدة ولا المفاتيح ولا سجلات DNS.

1. ما الذي يتغيّر

يحلّ موقع البوابة محلّ موقع PHP السابق على explorer.pocol.net (قرار المالك 9، 2026-11-16)، وينقل المالك
سجل DNS الخاص بـ explorer.pocol.net إلى خادم البوابة عند الإطلاق.
موقع Caddy واحد على خادم البوابة يقدّم:
- ملفات Explorer الثابتة (deploy/websites/explorer.pocol.net) عبر file_server، مع ترويسات الأمان التي كان
  يضعها ملف .htaccess السابق (Content-Security-Policy و X-Frame-Options DENY و Referrer-Policy و
  Permissions-Policy و nosniff) و Cache-Control: no-store؛
- مسارات القراءة نفسها التي يستدعيها Explorer: /api/rpc.php (POST) و /api/health.php و
  /api/miner-locations.php و /api/transactions.php (GET)، تجيب عنها البوابة نفسها (deploy/rewards/explorer_api.py)
  بالقيود نفسها التي كانت في ملفات PHP؛ ملخص الشبكة في pocol.net يقرأ /api/rpc.php نفسه؛
- مسارَي البوابة /rpc و /health، ومسار Studio /studio/* الذي يضيفه مثبّت deploy/studio إلى الكتلة نفسها،
  ومسارَي صنبور الرسوم /faucet/claim و /faucet/status اللذين يضيفهما deploy/faucet/faucet_route.py إليها
  (يُمرَّران إلى الصنبور على 127.0.0.1:8547 بعد حذف البادئة /faucet؛ انظر deploy/faucet/README.md).
الملفات المخفية وملفات PHP وقوائم التحقق والامتدادات التي كان .htaccess يمنعها تعطي 404، ولا يُعرض محتوى المجلدات.

2. المتطلبات

خادم Linux مع systemd وPython 3.10 أو أحدث وCaddy فعّال، وصلاحية sudo.
العقدة متاحة محليًا على 127.0.0.1:8545، والمنفذ المحلي 8088 متاح أو مستخدم بنسخة البوابة السابقة المُدارة.
ملف الشبكة /etc/pocol-explorer-gateway/network.json موجود (وملف forks.json اختياري)، كما في deploy/rewards/README.md.
شغّل الأوامر من نسخة Git من التزام الإصدار؛ المثبّت يأخذ منها gateway.py و pocol_rewards.py و explorer_api.py.

3. التثبيت

قبل ذلك لا تصدر شهادة HTTPS لهذا الاسم على خادم البوابة، فيفشل اختبار HTTPS في المثبّت ثم يسترجع الإعداد
السابق. لذلك شغّل الفحص (--check) قبل نقل السجل، والتثبيت الكامل بعد نقله حين تُرجع خوادم DNS العامة عنوان
هذا الخادم للاسم:

dig +short explorer.pocol.net @1.1.1.1; dig +short explorer.pocol.net @8.8.8.8

ينتظر التثبيت الشهادة الأولى حتى --https-wait ثانية (600 افتراضيًا، من 60 إلى 3600). إذا فشل اختبار HTTPS مع ذلك،
انتظر انتشار السجل قبل إعادة التشغيل، لأن كل إعادة تبدأ طلب شهادة جديدًا، وتكرار الفشل قد يبلغ حد الجهة المصدرة
لهذا الاسم مدة ساعة. لا يُعطَّل فحص الشهادة أبدًا.

D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" \
  && sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --check
sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D"
curl --fail --max-time 20 https://explorer.pocol.net/api/health.php

يجب أن تعرض الاستجابة "result":"PASS". أعد الاختبار من جهاز خارجي أيضًا لإثبات الوصول من خارج الخادم.

الأمر الأول يبني النسخة الثابتة (اثنا عشر ملفًا) من الملفات المحفوظة في Git في مجلد جديد ويتحقق منها مقابل
SHA256SUMS.txt، ويجب أن ينجح؛ إذا فشل لا يُشغَّل الفحص، ولا تبقى نسخة قديمة تُثبَّت.
يقارن المثبّت النسخة أيضًا بنسخة Explorer في هذا الالتزام، ويرفض نسخة مختلفة (قديمة أو معدّلة يدويًا) ويعرض
المعرّفين، إلا إذا سُمّي معرّفها صراحة بالخيار --accept-site site-<المعرّف> (لنسخة مقصود اختلافها، مثل نسخة
مُحوّلة لشبكة تجريبية).
الفحص (--check) لا يغيّر شيئًا: يتحقق من ملف الشبكة وهوية العقدة والنسخة الثابتة والمنفذ 8088 وصحة ملف Caddy الجديد.
ويرفض ملف network.json أو forks.json غير مقروء للجميع (0644) أو في مجلد لا يمكن دخوله، ومجلدَي /srv/pocol-explorer
و /opt/pocol-explorer-gateway إن لم يكونا مملوكين لـ root بالصلاحية 0755، لأن Caddy والخدمة يقرآنها بمستخدمين
غير مميّزين. المجلدات التي ينشئها المثبّت تكون 0755 مهما كان umask.
التثبيت يحفظ نسخة احتياطية في /var/backups/pocol-explorer-gateway/، ويثبّت خدمة pocol-explorer-gateway.service
على 127.0.0.1:8088 فقط، ويكتب النسخة الثابتة في /srv/pocol-explorer/site-<معرّف>، ثم يعيد تحميل Caddy ويختبر عبر
HTTPS باسم explorer.pocol.net وشهادة موثوقة كل ملف ثابت والترويسات و /api/health.php و /api/rpc.php و /rpc
و /health والمسارات الممنوعة ومسار Studio إن وُجد، ومسار الصنبور إن وُجد (GET /faucet/status برقم سلسلة الشبكة؛
ولا يُرسل أي طلب إلى /faucet/claim). إذا فشل أي اختبار، يسترجع ملف Caddy والملفات والخدمة السابقة،
وإذا تعثّرت الاستعادة يعرض ROLLBACK INCOMPLETE مع الخطوات المتعثّرة ومسار النسخة الاحتياطية.

ثبّت هذه البوابة قبل بوابة Studio (deploy/studio) ومسار الصنبور (deploy/faucet/faucet_route.py)، لأن كلًّا منهما
يضيف مساره إلى هذه الكتلة؛ أي ترتيب لتثبيتهما يعطي الملف نفسه، والتحديث يحتفظ بالمسارين كما هما ويرفض أي تعديل عليهما.
إذا وُجد موقع explorer.pocol.net غير مُدار أو كتلة معدّلة يدويًا، يتوقف المثبّت دون تغيير لتُراجع أولًا.

4. استكشاف الخطأ

sudo systemctl status pocol-explorer-gateway.service --no-pager
sudo journalctl -u pocol-explorer-gateway.service -n 40 --no-pager
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile

فهرس المعاملات (deploy/rewards/pocol_transactions.py) يُثبَّت مع البوابة ويعمل داخلها، وملفه في
/var/lib/pocol-explorer-gateway/transactions.sqlite3 (مجلد الحالة الخاص بالخدمة). يتبع العقدة المحلية، ويعيد الفهرسة
من نقطة التفرّع عند إعادة التنظيم، ويحتفظ بخمسمئة ألف معاملة على الأكثر.
لا توجد وحدة GeoIP: الشبكة لا تسجّل أماكن المعدّنين، ولا تُستنتج الأماكن من عناوين IP للأقران (الخصوصية). لذلك تجيب
/api/miner-locations.php بالرمز 503، ويقول Explorer إن أماكن المعدّنين غير متاحة. تغيير ذلك يحتاج قرارًا صريحًا من المالك.

5. عقدة المراقبة (Observer)

البوابة تقرأ 127.0.0.1:8545 فقط، وواجهة RPC لدى المؤسسين محلية فقط، لذلك يحتاج خادم البوابة عقدته الخاصة.
deploy/explorer/install_observer_node.py يثبّت عقدة لا تعدّن ولا تحمل مفتاحًا (pocold run بلا --miner)، وواجهة RPC
على 127.0.0.1:8545 فقط، وتتصل ببذور الدعوة. يتحقق المثبّت من بصمة SHA-256 لملف pocold ومن بصمة الدعوة، ثم ينتظر أن
تجيب العقدة باسم الشبكة الصحيحة دون معدِّن، وإلا يسترجع الحالة السابقة. خطوات النشر الكاملة بالترتيب في README.md
(Production deployment of the gateway host). لم يُجرَّب أي من ذلك على خادم حقيقي.
