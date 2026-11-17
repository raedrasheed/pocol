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
- مسارَي البوابة /rpc و /health، ومسار Studio /studio/* الذي يضيفه مثبّت deploy/studio إلى الكتلة نفسها.
الملفات المخفية وملفات PHP وقوائم التحقق والامتدادات التي كان .htaccess يمنعها تعطي 404، ولا يُعرض محتوى المجلدات.

2. المتطلبات

خادم Linux مع systemd وPython 3.10 أو أحدث وCaddy فعّال، وصلاحية sudo.
العقدة متاحة محليًا على 127.0.0.1:8545، والمنفذ المحلي 8088 متاح أو مستخدم بنسخة البوابة السابقة المُدارة.
ملف الشبكة /etc/pocol-explorer-gateway/network.json موجود (وملف forks.json اختياري)، كما في deploy/rewards/README.md.
شغّل الأوامر من نسخة Git من التزام الإصدار؛ المثبّت يأخذ منها gateway.py و pocol_rewards.py و explorer_api.py.

3. التثبيت

قبل ذلك لا تصدر شهادة HTTPS لهذا الاسم على خادم البوابة، فيفشل اختبار HTTPS في المثبّت ثم يسترجع الإعداد
السابق. لذلك شغّل الفحص (--check) قبل نقل السجل، والتثبيت الكامل بعد نقله مباشرة. لا يُعطَّل فحص الشهادة أبدًا.

python3 deploy/websites/tools/site_snapshot.py --out /tmp/explorer-site
sudo python3 deploy/explorer/install_explorer_gateway.py --site /tmp/explorer-site --check
sudo python3 deploy/explorer/install_explorer_gateway.py --site /tmp/explorer-site
curl --fail --max-time 20 https://explorer.pocol.net/api/health.php

يجب أن تعرض الاستجابة "result":"PASS". أعد الاختبار من جهاز خارجي أيضًا لإثبات الوصول من خارج الخادم.

الأمر الأول يبني النسخة الثابتة (اثنا عشر ملفًا) من الملفات المحفوظة في Git ويتحقق منها مقابل SHA256SUMS.txt.
الفحص (--check) لا يغيّر شيئًا: يتحقق من ملف الشبكة وهوية العقدة والنسخة الثابتة والمنفذ 8088 وصحة ملف Caddy الجديد.
التثبيت يحفظ نسخة احتياطية في /var/backups/pocol-explorer-gateway/، ويثبّت خدمة pocol-explorer-gateway.service
على 127.0.0.1:8088 فقط، ويكتب النسخة الثابتة في /srv/pocol-explorer/site-<معرّف>، ثم يعيد تحميل Caddy ويختبر عبر
HTTPS باسم explorer.pocol.net وشهادة موثوقة كل ملف ثابت والترويسات و /api/health.php و /api/rpc.php و /rpc
و /health والمسارات الممنوعة ومسار Studio إن وُجد. إذا فشل أي اختبار، يسترجع ملف Caddy والملفات والخدمة السابقة،
وإذا تعثّرت الاستعادة يعرض ROLLBACK INCOMPLETE مع الخطوات المتعثّرة ومسار النسخة الاحتياطية.

ثبّت هذه البوابة قبل بوابة Studio (deploy/studio)، لأن مثبّت Studio يضيف مساره إلى هذه الكتلة.
إذا وُجد موقع explorer.pocol.net غير مُدار أو كتلة معدّلة يدويًا، يتوقف المثبّت دون تغيير لتُراجع أولًا.

4. استكشاف الخطأ

sudo systemctl status pocol-explorer-gateway.service --no-pager
sudo journalctl -u pocol-explorer-gateway.service -n 40 --no-pager
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile

بدون وحدتَي GeoIP وفهرس المعاملات الاختياريتين تجيب /api/miner-locations.php و /api/transactions.php بالرمز 503
مع رسالتيهما، كما كانت ملفات PHP تجيب بدونهما.
