# أوامر الجولة الثانية ونتائجها

المسار `/workspace/pocol`؛ المخرجات المؤقتة خارج checkout في `/workspace/pocol-audit-round2`، والسجلات المحفوظة في `evidence/round2`. هذه الأوامر الأساسية بصيغ قابلة للإعادة. نطاق كل scan وتجزئات ملفاته محفوظة؛ إضافة اختبارات لاحقة قد تغير الأعداد.

```bash
git status --short
git fetch origin main
git rev-parse origin/main
# 0dda5056d2a78b2e135defb803df480e80422f8f
git ls-remote --heads origin
# 30 رأسًا وقت البحث؛ فُهرست أشجارها وتاريخ Git دون checkout.
export CARGO_HOME=/workspace/.pocol-cargo RUSTUP_HOME=/workspace/.pocol-rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_BUILD_JOBS=4

python3 -m unittest discover -s deploy/faucet -p 'test*.py' -v
# 15 passed نهائيًا؛ baseline الأول:8passed/6failed، وضابط status الإضافي فشل قبل إصلاحه ثم نجح.
POCOL_TEST_JAVA_HOME=/workspace/pocol-platform-tools/root/usr/lib/jvm/java-21-openjdk-amd64 \
  python3 -m unittest discover -s deploy/standalone -p test_android_jni.py -v
# 8 passed؛ baseline ذو6 حالات: 1 passed / 5 failed.
cargo +1.99.1 test --release --locked \
  --manifest-path prototypes/lp4-execution/Cargo.toml --no-fail-fast
# exit0، 221 passed / 0 failed / 1 ignored، 24 نتيجة هدف/doctest.
python3 deploy/standalone/test_wallet_earnin.py
# PASS، أربع عمليات محلية، height20، 3 كتل، لا أجهزة فعلية.

/lib64/ld-linux-x86-64.so.2 \
  --library-path /workspace/pocol-platform-tools/root/usr/lib/x86_64-linux-gnu \
  /workspace/pocol-platform-tools/root/usr/bin/php8.4 -n \
  -d extension_dir=/workspace/pocol-platform-tools/root/usr/lib/php/20240924 \
  -d extension=curl -d extension=posix deploy/websites/tests/api_security.php
# 41 PASS، networkRequests0؛ php -l للملفات الإنتاجية السبعة نجح أيضًا.
PLAYWRIGHT_MODULE=/workspace/pocol-browser-tools/node_modules/playwright/index.mjs \
  node deploy/websites/tests/browser_security.mjs
# النسخة النهائية:20 PASS، Chromium155.0.8059.79، حُظر طلبان خارجيان، لا إضافة/live RPC.
```

فحص `node --check` للملفات الخام132 نجح131 وفشل في `sitestore.js` ذي بدائل fault المتنافية. بُنيت بدائل الإضافة باستدعاء `build({fault,out})` من build.mjs لكل `null` و`FAULTS` السبعة، خارج checkout، ثم شُغّل `node --check` على34 وحدة JS لكل بديل:272/272. السجل والخريطة محفوظان. فشلَا الحزام في `websites-browser.log` و`websites-browser-final.log` ثم صُححا؛ `websites-browser-final2.log` هو التشغيل النهائي، دون تغيير شيفرة المواقع.

## Android

نُزلت JDK21 وPHP8.4 من Debian الرسمي بقوائم APT مستقلة تحت `/workspace/pocol-platform-tools/apt` وبمفتاح Debian المثبت، ثم `dpkg-deb -x` إلى root/ دون خدمات نظام. SDK metadata من `https://dl.google.com/android/repository/repository2-1.xml`. طابق الحجم وSHA1 المنشور لأرشيفات Android؛ وطابق SHA256 المنشور لـGradle8.11.1. البصمات الإضافية محفوظة في `android-download-provenance.json`. لم يُعطل TLS أو يُغير checksum متوقع.

```bash
export JAVA_HOME=/workspace/pocol-platform-tools/root/usr/lib/jvm/java-21-openjdk-amd64
gcc -Wall -Wextra -Werror -fanalyzer -fPIC \
  -I"$JAVA_HOME/include" -I"$JAVA_HOME/include/linux" \
  -c prototypes/pocol-android/app/src/main/cpp/bridge.c \
  -o /workspace/pocol-audit-round2/bridge-analyzed.o
# exit0 بلا diagnostics؛ javac للمصادر الثلاثة مع API35 نجح بتحذيرات موثقة.
rustup target add --toolchain 1.99.1 aarch64-linux-android
export ANDROID_HOME=/workspace/pocol-platform-tools/android-sdk
export GRADLE_USER_HOME=/workspace/pocol-platform-tools/gradle-cache
export PATH="$CARGO_HOME/bin:$JAVA_HOME/bin:/workspace/pocol-platform-tools/gradle-8.11.1/bin:$PATH"
export JAVA_TOOL_OPTIONS='-Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Djavax.net.ssl.trustStore=/etc/ssl/certs/java/cacerts'
export GRADLE_OPTS='-Dorg.gradle.workers.max=4 -Dorg.gradle.jvmargs=-Xmx2g'
bash deploy/standalone/build-android.sh
# BUILD SUCCESSFUL،45task؛ NDK27.2.12479018/SDK35، APK unsigned، لا توقيع/نشر.
"$ANDROID_HOME/build-tools/35.0.0/zipalign" -c -P 16 -v 4 \
  prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk
# Verification succesful
"$ANDROID_HOME/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf" \
  -lW prototypes/pocol-android/app/src/main/jniLibs/arm64-v8a/libpocol_bridge.so
```

proxy/CA أعلاه يطابق الجلسة المفحوصة؛ اتبع القيم الموروثة في جلسة أخرى. sdkmanager الرسمي أعاد أن --licenses لم يعد مطلوبًا في نسخته؛ سجله محفوظ. لا تعديل في سياسات المتصفح.

## Fuzzing

```bash
rustup toolchain install nightly --profile minimal --component rust-src
# nightly2026-11-04،1.101.0-nightly/708c94ef0
cargo +1.99.1 install cargo-fuzz --version 0.13.2 --locked
cargo +nightly fuzz run --fuzz-dir /workspace/pocol-audit-round2/fuzz \
  --target-dir /workspace/pocol-audit-round2/fuzz-target -j 4 parsers \
  /workspace/pocol-audit-round2/fuzz/corpus/parsers -- \
  -max_total_time=180 -max_len=16384 -rss_limit_mb=2048 -timeout=3 -seed=777900
# exit0،25,050,824 تنفيذًا،183s،OOM/timeout/crash0/0/0؛ -fork=4.
python3 development/security-audit-2026-11-04/fuzz/seed.py \
  /workspace/pocol-audit-round2/reusable-corpus
# 108 seed وقت الإنشاء؛ corpus يتغير أثناء الفحص.
CARGO_NET_OFFLINE=true cargo +nightly fuzz run \
  --fuzz-dir development/security-audit-2026-11-04/fuzz \
  --target-dir /workspace/pocol-audit-round2/fuzz-target -j 1 parsers \
  /workspace/pocol-audit-round2/reusable-corpus -- \
  -runs=5000 -max_len=16384 -rss_limit_mb=2048 -timeout=3 -seed=777900
# executor المسموح:exit0 معASan/LSan؛ shell المقيد أتم5000 ثم فشل LSan في process inspection.
```

حزمة fuzz في المستودع تناظر harness الخارجي، مع تعليق توضيحي ومسارات نسبية وقفل مستقل. لم يُعطل sanitizer أو يُغير ptrace؛ الـartifact الفارغ لفشل LSan ليس شاهد parser vulnerability.

## Semgrep والأسرار

```bash
export SEMGREP_SETTINGS_FILE=/workspace/pocol-audit-tools/semgrep-settings.yml
export SEMGREP_LOG_FILE=/workspace/pocol-audit-tools/semgrep.log
/workspace/pocol-audit-tools/bin/semgrep --version
# 1.183.0؛ قواعد المصدر عند16b9e6d45f10649f6a531ec4838ed9924372c3bc
/workspace/pocol-audit-tools/bin/semgrep scan \
  --config /tmp/pocol-semgrep-rules/javascript/browser/security \
  --config /tmp/pocol-semgrep-rules/javascript/lang/security \
  --config /tmp/pocol-semgrep-rules/python/lang/security \
  --config /tmp/pocol-semgrep-rules/rust --config /tmp/pocol-semgrep-rules/solidity \
  --config /tmp/pocol-semgrep-rules/php/lang/security \
  --config /tmp/pocol-semgrep-rules/java/lang/security \
  --config /tmp/pocol-semgrep-rules/java/android/security \
  --config /tmp/pocol-semgrep-rules/csharp/lang/security \
  --config /tmp/pocol-semgrep-rules/c/lang/security \
  --config /tmp/pocol-semgrep-rules/bash/lang/security \
  --config /tmp/pocol-semgrep-rules/yaml/github-actions/security \
  --jobs 2 --timeout 15 --timeout-threshold 0 --max-memory 2048 \
  --max-target-bytes 0 --no-git-ignore --metrics off --disable-version-check \
  --json --output /workspace/pocol-audit-round2/semgrep-wide.json \
  /workspace/pocol-audit-round2/scan-inputs
# 735 targets،368 rules run،637 findings/26 errors؛ exit0 ليس خلوًا من الثغرات.
```

حُفظت نسخ بايتية من نطاق scan-map.json بأسماء SHA256 والامتداد، دون استبعاد dist/assets/evidence أو الملفات الكبيرة. الفحص الموسع وقت767 مسارًا؛ الجرد الحالي772. Prettier3.6.2 بـparser:babel نسق نسخ الحزم الخمسة؛ Babel7.28.5 بـsourceType:unambiguous أثبت تطابق AST بعد حذف metadata المواقع/comments/extra. أعيدت قاعدتا JS أعلاه على formatted/ بمهلة30s:20finding/57error. قواعد yaml على .github/workflows الأصلية:3finding/20error. لا تغيير في ملفات التوزيع.

استُخرج مصدر الصنبور وbridge.c من المرجع المثبت بـ`git show SHA:path` إلى changed-main/؛ Python/C/PHP مع حدود15s أعادت2finding/1partial. الفحص النهائي أضاف JS/Rust إلى هذه المجموعات، على الملفات العشرة التالية:8finding/1partial،192قاعدة. الملاحظتان في مصدر التطبيق بقيتا؛ دليل الإصلاح الاختبارات، وليس انخفاض العدد.

```text
deploy/faucet/pocol_faucet.py
deploy/faucet/test_pocol_faucet.py
deploy/faucet/test_faucet_security.py
deploy/standalone/test_android_jni.py
prototypes/pocol-android/app/src/main/cpp/bridge.c
deploy/websites/tests/api_security.php
deploy/websites/tests/browser_security.mjs
development/security-audit-2026-11-04/fuzz/fuzz_targets/parsers.rs
development/security-audit-2026-11-04/fuzz/seed.py
development/security-audit-2026-11-04/tools/scan_secrets_bounded.py
```

ثلاث محاولات قبل ضبط SEMGREP_LOG_FILE فشلت في مسار log الافتراضي للقراءة فقط؛ ليست scans مكتملة. المشاكل في parsing بقيت مسجلة دون ignore.

```bash
# من scan-inputs نفسه، لا من جذر Git الآخر:735ملفًا،115مرشحًا.
/workspace/pocol-audit-tools/bin/detect-secrets -c 2 scan --no-verify --all-files .
# من جذر المستودع: محاولة غير مقيدة أوقفت دون JSON مكتمل؛ لا تُحسب نتيجة.
/workspace/pocol-audit-tools/bin/detect-secrets -c 2 scan --no-verify
# الإعادة المكتملة، offline وhashes فقط ومهلة لكل ملف:
/workspace/pocol-audit-tools/bin/python \
  development/security-audit-2026-11-04/tools/scan_secrets_bounded.py \
  /workspace/pocol-audit-round2/secrets-bounded.jsonl --jobs 2 --seconds-per-file 5
# exit0،5953/5953،صفر مهلات،positive control5،29258candidate في811ملفًا.
```

محاولتان سابقتان من جذر غير ملائم عددتا صفر ملفات ولم تعتمدا. no-verify يمنع التحقق من credential عبر خدمة؛ البيانات المنشورة hashes/metadata فقط، ومرشحات entropy لم تُحسم كلها يدويًا.

## حزمة التسليم

```bash
curl --fail --silent --show-error --location --max-time 120 --max-filesize 300000000 \
  https://github.com/palcertcom/pocol/releases/download/handoff-2026-11-01/PoCol-Complete-Handoff-2026-11-01.zip \
  -o /workspace/pocol-audit-round2/handoff.zip
sha256sum /workspace/pocol-audit-round2/handoff.zip
# b9a3885a630bd7f32270488f741b68a5c923e3fc48c1478499ceb887c520d2f3
```

فهرسة zipfile دون تشغيل:280entry و7ZIP مختلفة داخلية، depth<=3 وحد70MB لكلZIP. لم توجد ملفات ts/tsx/jsx/map/vue/svelte في المواضع المفهرسة. فُحصت خمسة أرشيفات متتبعة أيضًا. سجلات الرفع والتحقق تضاف إلى التسليم؛ لا force push أو تشغيل workflows. API يمنع إنشاء PR وقراءة Actions/CodeQL؛ رابط pull/new لا يثبت إنشاء PR.
