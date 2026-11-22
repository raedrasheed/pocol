حدّث محتويات جذر pocol.net بعد أخذ نسخة احتياطية. احتفظ بملفات إعداد API وبياناتها الموجودة. ارفع ملفات HTML أخيرًا.
هذه النسخة تسبق الشبكة الجديدة: لا تتضمن ملف شبكة، وتُستبدل حزم التنزيل وملف الشبكة عند إطلاق الشبكة الجديدة. تنزيل Android موقوف لحين تجهيز الإصدار الموقّع.

لا تكفي الكتابة فوق الملفات: يبقى على الخادم كل ملف لم يعد في هذه النسخة ويظل متاحًا للزوار، مثل ملف الشبكة القديم downloads/network.pocol.json وحزمة الإضافة القديمة. بعد الرفع أنشئ قائمة بملفات الجذر وبصماتها من Terminal في cPanel داخل جذر الموقع:
find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ~/pocol.net-files.txt
ثم قارنها بهذه النسخة في نسخة من المستودع:
python3 deploy/websites/tools/host_files.py --site pocol.net --listing pocol.net-files.txt
احذف من الخادم كل ملف تذكره الأداة بعلامة DELETE، وأعد رفع كل ملف بعلامة DIFFERS أو MISSING، ثم أعد إنشاء القائمة وكرر حتى تطبع الأداة OK. ملفات الإعداد التي لا تكون في أي نسخة (api/config.php و.user.ini وphp.ini وerror_log و.well-known/ وcgi-bin/) تظهر بعلامة KEEP ولا تُحذف. لا تستبدل ملف .htaccess الموجود على الخادم: يكتب cPanel فيه إعداداته (كتلة "# php -- BEGIN cPanel-generated handler, do not edit" حتى "# php -- END cPanel-generated handler, do not edit" التي يضيفها MultiPHP Manager، وأي تحويل إلى HTTPS). إذا ظهر بعلامة MERGE فخذ منه نسخة احتياطية، وأبقِ تلك الكتل كما هي، وأضف قبلها أو بعدها محتوى ملف .htaccess من هذه النسخة دون تعديل، ثم نزّل نسخة من الملف الناتج وتحقق منها بإضافة --host-copy .htaccess=COPY إلى الأمر أعلاه.
