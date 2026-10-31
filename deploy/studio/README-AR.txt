PoCol Studio — تجهيز الاستضافة والاتصال
الإصدار الأول، شبكة الاختبار ذات Chain ID = 1599846715

هذه الملفات جاهزة للتثبيت ولم تُنفّذ على الخادم من داخل هذه الجلسة.
تثبيت البوابة لا ينشر أي موقع ولا يرسل معاملات ولا يقرأ مفتاح المحفظة.

1. نطاق Studio على استضافة Namecheap/cPanel

في cPanel > Domains > Create A New Domain:
- Domain: studio.pocol.net
- ألغِ Share document root إن ظهر.
- Document root: مجلد مستقل، مثل studio.pocol.net.
- ارفع محتويات حزمة PoCol-Studio-cPanel.zip إلى جذر هذا المجلد.
- فعّل شهادة SSL عبر AutoSSL إذا لم تتفعّل تلقائيًا.

إذا كانت DNS تُدار خارج cPanel، أضف سجل A باسم studio يشير إلى عنوان
خادم استضافة cPanel نفسه (لا تستخدم 108.165.128.89 لصفحة Studio في هذا الخيار).
استخدم عنوان Shared IP Address الظاهر في cPanel أو إعداد الاستضافة الموثّق؛
لا نحزر عنوان خادم Namecheap. لا تعدّل سجلات pocol.net أو explorer أو البريد.

2. تمكين الاتصال المباشر بالشبكة

المتطلبات: خادم Linux مع systemd وPython 3.10 أو أحدث وflock، وصلاحية sudo.
يجب أن تكون بوابة Explorer الحالية وخدمة Caddy فعّالتين، وأن تكون شهادة
gateway.pocol.net موثوقة، والعقدة المطلوبة متاحة محليًا على 127.0.0.1:8545.
يجب أن يكون المنفذ المحلي 8089 متاحًا أو مستخدمًا بنسخة Studio السابقة المُدارة.
لا يتطلب هذا المثبّت تنزيل حزم من الإنترنت؛ مكوّنات البوابة مضمّنة فيه.

يوجد بالفعل نطاق gateway.pocol.net على خادم العقدة الأولى 108.165.128.89.
سنضيف إليه مسارًا مستقلاً: https://gateway.pocol.net/studio/rpc
خدمة Explorer ومسارها /rpc يبقيان كما هما؛ RPC الخام على 8545 يبقى محليًا.

احفظ Install-PoCol-Studio-Gateway.sh في Downloads على Windows، ثم في PowerShell:

scp -F none -o IdentitiesOnly=yes -i "$env:USERPROFILE\.ssh\id_ed25519_pocol" "$env:USERPROFILE\Downloads\Install-PoCol-Studio-Gateway.sh" pocol-admin@108.165.128.89:~/

افتح اتصال SSH المعتاد بالخادم، ثم فحص دون تعديل:

sudo bash ~/Install-PoCol-Studio-Gateway.sh --check

إذا ظهر CHECK PASSED، ثبّت:

sudo bash ~/Install-PoCol-Studio-Gateway.sh

تحقّق من HTTPS الطبيعي دون تعطيل فحص الشهادة:

curl --fail --max-time 20 https://gateway.pocol.net/studio/health

يجب أن تعرض الاستجابة ready:true و chainId:1599846715.
أعد الاختبار من جهاز Windows أو cPanel أيضًا لإثبات الوصول الخارجي.

البرنامج يضيف خدمة pocol-studio-gateway.service على 127.0.0.1:8089
ويضيف مسارها إلى Caddy الموجود. لا يفتح منفذ 8089 للعامة ولا يحتاج سجل DNS جديدًا
للبوابة. يتأكد من هوية الشبكة قبل التثبيت وقبل كل بث معاملة موقّعة.
بعد إعادة تحميل Caddy يختبر شهادة HTTPS باسم gateway.pocol.net ومسار صحة Studio
ومسار Explorer السابق /rpc. إذا فشل أي اختبار، يسترجع الإعداد السابق والخدمة السابقة.
هذا الاختبار المحلي يثبت الشهادة والمسارات؛ يبقى الاختبار من Windows/cPanel مطلوبًا
لإثبات وصول المستخدم من خارج الخادم.
إذا لم يتعرّف على إعداد Caddy الموجود، يتوقف دون استبداله؛ تُراجع الرسالة أولًا.
تُحفظ نسخة Caddy السابقة في /var/backups/pocol-studio-gateway/.
إذا تعثّرت الاستعادة نفسها، يستمر بمحاولة بقية خطوات الاستعادة ويعرض
ROLLBACK INCOMPLETE مع الخطوات المتعثّرة ومسار النسخة الاحتياطية؛ لا يعلن النجاح.

3. إضافة Chrome

تتصل الإضافة المحدّثة بمسار Studio الجديد، ويُختار اتصال «عقدة بعيدة» بوضوح.
يتوقف احتياج SSH على جهاز المستخدم بعد تثبيت البوابة وتحديث الإضافة.
التثبيت من متجر Chrome يتطلب إرسال الإضافة وقبول المراجعة؛ ليس جزءًا من هذا المثبّت.
المفاتيح تبقى داخل الإضافة؛ البوابة لا تستقبل إلا معاملات موقّعة مسبقًا.

استكشاف الخطأ

sudo systemctl status pocol-studio-gateway.service --no-pager
sudo journalctl -u pocol-studio-gateway.service -n 40 --no-pager
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile

لا تُشغّل مثبّت HTTPS القديم مرة أخرى، لأنه يتوقع ملف Caddy السابق قبل إضافة Studio.
لا تحذف بيانات العقدة ولا تغيّر منفذ RPC الخام لحل مشكلة في واجهة Studio.

المصادر
https://github.com/palcertcom/pocol/blob/50c2f27ee67961dec7aaf0eb735c0b641810e0ea/prototypes/pocold/src/site.rs
https://caddyserver.com/docs/caddyfile/directives/reverse_proxy
https://caddyserver.com/docs/caddyfile/directives/request_body
https://docs.cpanel.net/cpanel/domains/domains/

حدود التحقق في هذه الحزمة
اجتازت البوابة اختبارات التحقق والاتصال HTTP المحلي مع عقدة بديلة للاختبار.
يظل اختبار اتصال الإنتاج وصدور معاملة حقيقية وعرض موقع من جهاز المستخدم
مطلوبًا بعد تثبيت البوابة والإضافة ورفع Studio.
