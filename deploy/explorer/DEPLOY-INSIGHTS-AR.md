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

## ما الذي يتغيّر على الخادم

| المكان | التغيير |
|---|---|
| `/opt/pocol-explorer-gateway/` | تحديث `gateway.py` و `explorer_api.py`، وإضافة `pocol_holders.py` و `pocol_geoip.py` (المثبّت يكتب الملفات الستة) |
| `/opt/pocol-explorer-gateway/geoip/dbip-country.bin` | قاعدة GeoIP، مع الخيار `--geoip-db` فقط |
| `/etc/systemd/system/pocol-explorer-gateway.service` | يضيف `Environment=POCOL_HOLDERS_DB=/var/lib/pocol-explorer-gateway/holders.sqlite3` والوصف الجديد؛ والباقي، ومنه `MemoryMax=256M`، كما هو |
| `pocol-explorer-gateway.service.d/geo-reporters.conf` | فقط ما دام الملف `/etc/pocol-explorer-gateway/geo-reporters.json` موجودًا (تقارير المشغّلين؛ ليست جزءًا من هذا النشر) |
| `/srv/pocol-explorer/site-<id>` | نسخة ثابتة جديدة من 14 ملفًا؛ ويُحتفظ بالنسخة السابقة للاسترجاع |
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
   رابط https://db-ip.com كلما كانت القاعدة مثبّتة. سجّل بصمة SHA-256 للملف (`sha256sum`).
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

(احذف `--geoip-db` إن لم تكن هناك قاعدة.) يجب أن يطبع `CHECK PASSED: network 2656089850 0x664dc53a...` مع
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
- عند الجاهزية: `"chainId":2656089850`، وارتفاع اللقطة قريب من رأس العقدة، و `verification.complete` صحيحة مع
  `unlocatedWei` تساوي "0". الحسابات الوحيدة في تكوين PoCol Mainnet هي المؤسسون الأربعة، فأي فرق يعني حسابًا لا يمكن
  اكتشافه (F-104): يُنشر ولا يُخفى؛ سجّله مع ارتفاعه.
- `/api/miner-geography.php`: `miners.registered` يساوي عدد سجلات المعدّنين، و `located` صفر و `unknown` يساوي
  `registered` (لا مبلّغين)، والمصدر `explorer` حديث، و `database.release` هو الشهر الذي بنيته (أو `database` تساوي
  null بلا قاعدة). لا يظهر في الرد أي عنوان IP أو رمز أو عنوان معدّن.
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

التثبيت الفاشل يسترجع الحالة السابقة وحده (الخطوة 3). لا يلزم غير حذف ملف فهرس الحائزين الذي قد تكون البوابة الجديدة
أنشأته قبل الفشل (الخطوة 5 في 6.2)، والبوابة السابقة تتجاهله.

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

ثم تحقّق: `grep -n 'root \*' /etc/caddy/Caddyfile` يذكر `site-<المعرّف القديم>` (يحتفظ المثبّت بتلك النسخة)، وملف
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
