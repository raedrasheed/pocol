(() => {
  'use strict';
  // The privacy policy of the PoCol Chrome extension and the PoCol websites (2026-11-22). Every
  // statement describes what the code in this repository does; deploy/websites/tests/test_privacy_page.py checks that
  // privacy.html is this page's English text and that every English entry has its Arabic one. The page renders text
  // only (textContent), never HTML.
  const translations = {
    "en": {
      "title": "Privacy policy · PoCol",
      "description": "How the PoCol Chrome extension and the PoCol websites handle your information: keys stay on your device, no accounts, no analytics.",
      "eyebrow": "PRIVACY",
      "heading": "Privacy policy",
      "updated": "Last updated: 22 November 2026",
      "lead": "This policy covers the PoCol Chrome extension (PoCol Studio, IDE & Viewer) and the websites pocol.net, studio.pocol.net, ide.pocol.net and explorer.pocol.net. It describes what their software, published in the PoCol repository, does with your information. PoCol has no user accounts, shows no advertising and uses no analytics or tracking.",
      "sections": [
        {
          "heading": "What a public blockchain makes public",
          "items": [
            "Everything you send to the PoCol network becomes part of a public blockchain: your address, the transactions you sign and their amounts, and the files of the websites and contracts you publish. Anyone can read them, and they cannot be changed or deleted afterwards, by you or by PoCol.",
            "Do not publish personal information on the network."
          ]
        },
        {
          "heading": "The Chrome extension",
          "items": [
            "Your wallet keys are created in your browser, or imported from your encrypted wallet file, a private key or a recovery phrase, and stored only in this extension's local storage in Chrome, encrypted with your password (PBKDF2-SHA256 with 600,000 iterations and AES-256-GCM). Of a recovery phrase only the derived key of its first account is kept; the phrase itself is not stored. Your password and your private keys never leave the extension, except inside the encrypted backup file that the extension saves to your computer when you create your wallet or ask for a backup.",
            "To read the network and send transactions, the extension contacts the PoCol network gateway at https://explorer.pocol.net/studio/rpc, or a node on your own computer (127.0.0.1) if you add one. These requests carry your wallet address (to read its balance and transaction count), the transactions you approve, the hashes of the transfers you sent (to read their receipts), and the website or contract files you choose to publish. Like every internet request, they show your IP address to the gateway. Before the toolbar wallet makes its first such request (network status, balance, Send or transfer status), it shows what it sends and asks you to agree. A Studio publication or an IDE contract operation sends your address and its files only after you unlock the wallet in the extension's own confirmation window and start the operation there; until then that window reads only the network's identity and fees.",
            "The extension also stores on your device the network profiles you add, the progress of each publication (including its signed transactions until the publication completes, so that an interrupted publication can resume), and a list of the transfers you sent from it (hash, recipient, amount and status), so that it can show their status. Removing the extension deletes all of this, so keep an encrypted backup of your wallet (Settings, Back up).",
            "You send PCOL from the extension's own Send page: it shows the network, the recipient, the amount, the fee and the total, and the transfer is signed only after you confirm it there.",
            "The extension works with two websites only, https://studio.pocol.net and https://ide.pocol.net, and receives from them the publication or contract you ask it to handle. Nothing is signed or sent until you approve it in the extension's own window. The extension does not read other websites, your tabs or your browsing history.",
            "On-chain websites that you open in the extension's viewer are loaded from the PoCol network and run in an isolated sandbox with no access to your wallet or to the extension. On PoCol Mainnet this version gives such sites no account and no signature: their requests are refused without a dialog. Only on a local development network that you add yourself can a site ask for the viewer's local wallet; the extension asks you first, the grant is for that tab only, and you can end it in Settings.",
            "The extension contacts no other server, uses no analytics, and does not sell or share your information."
          ]
        },
        {
          "heading": "Chrome Web Store User Data Policy",
          "items": [
            "The PoCol extension's use of your information complies with the Chrome Web Store User Data Policy, including the Limited Use requirements: it uses your information only for the features this policy describes, transfers it only to the PoCol network gateway or the node you add, to carry out your requests, and never uses or transfers it for advertising, for creditworthiness or lending, or for any purpose unrelated to these features."
          ]
        },
        {
          "heading": "The websites",
          "items": [
            "pocol.net stores your language and appearance choices in your browser. Its network statistics and block reward are fetched by pocol.net's own server from explorer.pocol.net; your browser sends no personal information for them.",
            "The fee faucet page sends the address you enter to the faucet, which runs at explorer.pocol.net. The faucet keeps a record of each address it has paid, with the day and the transaction, so that it pays each address only once. To apply its daily limit per connection, it counts claims per IP address (for IPv6, per /64 network) in memory for the current day (UTC) only; it does not write IP addresses to disk.",
            "PoCol Studio (studio.pocol.net) keeps your drafts in your browser's local storage, checks the gateway's health and, before publishing, reads the network's limits at explorer.pocol.net, and publishes only through the extension, after your approval.",
            "PoCol IDE (ide.pocol.net) stores its appearance choice in your browser and reads public network data from explorer.pocol.net; a contract is deployed only through the extension, after your approval.",
            "Requests to explorer.pocol.net, including the extension's, are answered by the PoCol gateway on that server. The gateway software writes no request log, and the web server configuration published in the repository enables no access log. The Studio relay (/studio/rpc) counts requests per IP address in memory to limit abuse.",
            "The explorer page loads its font stylesheet from Google Fonts (fonts.googleapis.com), so Google receives your IP address and browser information when you open it, under Google's own privacy policy.",
            "pocol.net, studio.pocol.net and ide.pocol.net are served by a web hosting provider, whose servers may keep standard access logs (IP address, time, requested page and browser type) for the period the provider sets."
          ]
        },
        {
          "heading": "Cookies and tracking",
          "items": [
            "The PoCol websites and the extension set no cookies and use no tracking or advertising technology. Browser storage (local storage and the extension's storage) is used only for the purposes above and stays on your device."
          ]
        },
        {
          "heading": "Your choices",
          "items": [
            "You can delete what a PoCol website stores by clearing that site's data in your browser, and what the extension stores by removing the extension. Information already on the public blockchain cannot be deleted."
          ]
        },
        {
          "heading": "Changes and questions",
          "items": [
            "Changes to this policy are published on this page with a new date. Questions about privacy can be asked in the PoCol repository on GitHub: https://github.com/raedrasheed/pocol/issues. Report security problems privately, as the repository's SECURITY.md describes."
          ]
        }
      ]
    },
    "ar": {
      "title": "سياسة الخصوصية · PoCol",
      "description": "كيف تتعامل إضافة PoCol لمتصفح Chrome ومواقع PoCol مع معلوماتك: تبقى المفاتيح على جهازك، بلا حسابات ولا أدوات تحليل.",
      "eyebrow": "الخصوصية",
      "heading": "سياسة الخصوصية",
      "updated": "آخر تحديث: 22 نوفمبر 2026",
      "lead": "تغطي هذه السياسة إضافة PoCol لمتصفح Chrome ‏(PoCol Studio, IDE & Viewer) والمواقع pocol.net وstudio.pocol.net وide.pocol.net وexplorer.pocol.net، وتصف ما تفعله برمجياتها المنشورة في مستودع PoCol بمعلوماتك. لا توجد في PoCol حسابات مستخدمين ولا إعلانات، ولا تُستخدم أي أدوات تحليل أو تتبّع.",
      "sections": [
        {
          "heading": "ما تجعله سلسلة الكتل العامة علنيًا",
          "items": [
            "كل ما ترسله إلى شبكة PoCol يصبح جزءًا من سلسلة كتل عامة: عنوانك، والمعاملات التي توقّعها ومبالغها، وملفات المواقع والعقود التي تنشرها. يمكن لأي شخص قراءتها، ولا يمكن تعديلها أو حذفها بعد ذلك، لا منك ولا من PoCol.",
            "لا تنشر معلومات شخصية على الشبكة."
          ]
        },
        {
          "heading": "إضافة Chrome",
          "items": [
            "تُنشأ مفاتيح محفظتك داخل متصفحك، أو تُستورد من ملف محفظتك المشفّر أو من مفتاح خاص أو من عبارة استرداد، وتُخزَّن فقط في التخزين المحلي لهذه الإضافة في Chrome، مشفّرة بكلمة مرورك (PBKDF2-SHA256 بـ 600,000 تكرار وAES-256-GCM). من عبارة الاسترداد لا يُحفظ إلا المفتاح المشتق لحسابها الأول، ولا تُخزَّن العبارة نفسها. لا تغادر كلمة مرورك ولا مفاتيحك الخاصة الإضافة أبدًا، إلا داخل ملف النسخة الاحتياطية المشفّرة الذي تحفظه الإضافة على جهازك عند إنشاء محفظتك أو عندما تطلب نسخة احتياطية.",
            "لقراءة الشبكة وإرسال المعاملات تتصل الإضافة ببوابة شبكة PoCol على https://explorer.pocol.net/studio/rpc، أو بعقدة على جهازك (127.0.0.1) إن أضفتها. تحمل هذه الطلبات عنوان محفظتك (لقراءة رصيده وعدد معاملاته)، والمعاملات التي توافق عليها، وبصمات التحويلات التي أرسلتها (لقراءة إيصالاتها)، وملفات المواقع أو العقود التي تختار نشرها. وكأي طلب عبر الإنترنت، تكشف هذه الطلبات عنوان IP الخاص بك للبوابة. وقبل أول طلب من هذا النوع (حالة الشبكة أو الرصيد أو الإرسال أو حالة التحويلات) تعرض محفظة شريط الأدوات ما ترسله وتطلب موافقتك. أما النشر من Studio وعمليات العقود من IDE فلا ترسل عنوانك وملفاتها إلا بعد أن تفتح قفل المحفظة في نافذة التأكيد الخاصة بالإضافة وتبدأ العملية فيها؛ وقبل ذلك لا تقرأ تلك النافذة إلا هوية الشبكة ورسومها.",
            "تخزّن الإضافة على جهازك أيضًا ملفات تعريف الشبكات التي تضيفها، وتقدّم كل عملية نشر (بما في ذلك معاملاتها الموقّعة حتى يكتمل النشر، لكي يمكن استئناف عملية نشر انقطعت)، وقائمة بالتحويلات التي أرسلتها منها (البصمة والمستلم والمبلغ والحالة) لكي تعرض حالتها. إزالة الإضافة تحذف كل ذلك، لذا احتفظ بنسخة احتياطية مشفّرة من محفظتك (الإعدادات، النسخ الاحتياطي).",
            "ترسل PCOL من صفحة الإرسال الخاصة بالإضافة: تعرض الشبكة والمستلم والمبلغ والرسوم والإجمالي، ولا يُوقَّع التحويل إلا بعد أن تؤكده فيها.",
            "تعمل الإضافة مع موقعين فقط، https://studio.pocol.net وhttps://ide.pocol.net، وتتلقى منهما عملية النشر أو العقد الذي تطلب منها معالجته. لا يُوقَّع شيء ولا يُرسَل قبل موافقتك في نافذة الإضافة نفسها. لا تقرأ الإضافة مواقع أخرى ولا علامات التبويب لديك ولا سجل تصفحك.",
            "تُحمَّل المواقع المنشورة على السلسلة التي تفتحها في عارض الإضافة من شبكة PoCol، وتعمل في بيئة معزولة لا تصل إلى محفظتك ولا إلى الإضافة. وعلى PoCol Mainnet لا يمنح هذا الإصدار هذه المواقع أي حساب ولا توقيع: تُرفض طلباتها دون نافذة. وعلى شبكة تطوير محلية تضيفها أنت فقط يمكن لموقع أن يطلب المحفظة المحلية للعارض؛ فتسألك الإضافة أولًا، ويقتصر الإذن على علامة التبويب تلك، ويمكنك إنهاؤه من الإعدادات.",
            "لا تتصل الإضافة بأي خادم آخر، ولا تستخدم أدوات تحليل، ولا تبيع معلوماتك ولا تشاركها."
          ]
        },
        {
          "heading": "سياسة بيانات المستخدم في سوق Chrome الإلكتروني",
          "items": [
            "يلتزم استخدام إضافة PoCol لمعلوماتك بسياسة بيانات المستخدم في سوق Chrome الإلكتروني، بما في ذلك متطلبات الاستخدام المحدود (Limited Use): فهي لا تستخدم معلوماتك إلا للميزات التي تصفها هذه السياسة، ولا تنقلها إلا إلى بوابة شبكة PoCol أو العقدة التي تضيفها لتنفيذ طلباتك، ولا تستخدمها ولا تنقلها أبدًا للإعلانات أو لتقييم الجدارة الائتمانية أو الإقراض أو لأي غرض لا علاقة له بهذه الميزات."
          ]
        },
        {
          "heading": "المواقع",
          "items": [
            "يحفظ pocol.net اختياراتك للغة والمظهر في متصفحك. ويجلب خادم pocol.net نفسه إحصاءات الشبكة ومكافأة الكتلة من explorer.pocol.net، ولا يرسل متصفحك من أجلها أي معلومات شخصية.",
            "ترسل صفحة صنبور الرسوم العنوان الذي تُدخله إلى الصنبور الذي يعمل على explorer.pocol.net. يحتفظ الصنبور بسجل لكل عنوان دفع له، مع اليوم والمعاملة، لكي يدفع لكل عنوان مرة واحدة فقط. ولتطبيق حده اليومي لكل اتصال يعدّ الطلبات لكل عنوان IP (ولكل شبكة ‎/64 في IPv6) في الذاكرة لليوم الحالي (بتوقيت UTC) فقط، ولا يكتب عناوين IP على القرص.",
            "يحفظ PoCol Studio ‏(studio.pocol.net) مسوداتك في التخزين المحلي لمتصفحك، ويتحقق من حالة البوابة ويقرأ حدود الشبكة قبل النشر على explorer.pocol.net، ولا ينشر إلا عبر الإضافة بعد موافقتك.",
            "يحفظ PoCol IDE ‏(ide.pocol.net) اختيار المظهر في متصفحك ويقرأ بيانات الشبكة العامة من explorer.pocol.net، ولا يُنشر عقد إلا عبر الإضافة بعد موافقتك.",
            "تجيب بوابة PoCol على الخادم explorer.pocol.net عن الطلبات الموجهة إليه، بما فيها طلبات الإضافة. لا تكتب برمجيات البوابة سجلًا للطلبات، ولا يفعّل إعداد خادم الويب المنشور في المستودع أي سجل وصول. ويعدّ مرحِّل Studio ‏(/studio/rpc) الطلبات لكل عنوان IP في الذاكرة للحد من إساءة الاستخدام.",
            "تحمّل صفحة المستكشف ورقة أنماط الخطوط من Google Fonts ‏(fonts.googleapis.com)، فتتلقى Google عنوان IP الخاص بك ومعلومات متصفحك عند فتحها، وفق سياسة الخصوصية الخاصة بها.",
            "يستضيف pocol.net وstudio.pocol.net وide.pocol.net مزوّدُ استضافة ويب قد تحتفظ خوادمه بسجلات وصول معتادة (عنوان IP والوقت والصفحة المطلوبة ونوع المتصفح) للمدة التي يحددها المزوّد."
          ]
        },
        {
          "heading": "ملفات تعريف الارتباط والتتبع",
          "items": [
            "لا تضع مواقع PoCol ولا الإضافة أي ملفات تعريف ارتباط، ولا تستخدم أي تقنية تتبع أو إعلانات. يُستخدم تخزين المتصفح (التخزين المحلي وتخزين الإضافة) للأغراض المذكورة أعلاه فقط، ويبقى على جهازك."
          ]
        },
        {
          "heading": "خياراتك",
          "items": [
            "يمكنك حذف ما يخزّنه موقع من مواقع PoCol بمسح بيانات ذلك الموقع في متصفحك، وحذف ما تخزّنه الإضافة بإزالتها. أما المعلومات الموجودة على سلسلة الكتل العامة فلا يمكن حذفها."
          ]
        },
        {
          "heading": "التغييرات والأسئلة",
          "items": [
            "تُنشر التغييرات على هذه السياسة في هذه الصفحة مع تاريخ جديد. يمكن طرح الأسئلة المتعلقة بالخصوصية في مستودع PoCol على GitHub: https://github.com/raedrasheed/pocol/issues. أبلغ عن المشكلات الأمنية بشكل خاص، كما يصف ملف SECURITY.md في المستودع."
          ]
        }
      ]
    }
  };
  const language = () => (document.documentElement.lang === 'ar' ? 'ar' : 'en');
  const description = document.querySelector('meta[name="description"]');
  const body = document.querySelector('#privacy-body');
  function render() {
    const text = translations[language()];
    document.title = text.title;
    if (description) description.content = text.description;
    for (const key of ['eyebrow', 'heading', 'updated', 'lead']) document.querySelector(`#privacy-${key}`).textContent = text[key];
    body.replaceChildren(...text.sections.map(section => {
      const block = document.createElement('section');
      block.className = 'privacy-section';
      const heading = document.createElement('h2');
      heading.textContent = section.heading;
      const list = document.createElement('ul');
      for (const item of section.items) {
        const entry = document.createElement('li');
        entry.textContent = item;
        list.append(entry);
      }
      block.append(heading, list);
      return block;
    }));
  }
  window.addEventListener('pocol:language', render);
  render();
})();
