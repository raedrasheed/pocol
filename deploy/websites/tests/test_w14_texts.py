#!/usr/bin/env python3
"""Sites W14 texts: no served page, Studio file or the Studio quick start still tells a user to save the retired
activation schedule (Options -> "PoCol Mainnet activation", [[1,"0"],[2,"15000"]]) or to update the extension in
place (replace the files in the same folder and click Reload). The upgrade texts follow README-IDE.md's order in
English and Arabic, without version numbers: export the wallet, Remove the old version, verify the signature, Load
unpacked, compare the extension ID with pocol.net, import the wallet. DEPLOY-README-AR.txt sections 2 and 4 say that a
new network has a new chain ID and genesis and drop the old network's check and in-place rule.

    python3 -B -m unittest discover -s deploy/websites/tests -p 'test_*.py' -v
"""
import base64, hashlib, json, re, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAIN = ROOT / 'deploy' / 'websites' / 'pocol.net'
IDE = ROOT / 'deploy' / 'websites' / 'ide.pocol.net'
STUDIO = ROOT / 'prototypes' / 'pocol-studio'
SERVED = [ROOT / 'deploy' / 'websites' / site for site in ('explorer.pocol.net', 'ide.pocol.net', 'pocol.net')] + [STUDIO / 'dist']
QUICKSTART = STUDIO / 'QUICKSTART-AR.txt'
BINARY = {'.png', '.ico', '.zip', '.woff', '.woff2', '.ttf', '.webp', '.jpg', '.wasm'}

# The retired texts, in every spelling the files use (plain, JSON-escaped, HTML-escaped).
RETIRED = {
    'the retired activation setting': re.compile(r'PoCol Mainnet activation'),
    'the retired fork schedule': re.compile(r'\[2,(?:\\?"|&quot;)15000'),
    'an in-place update (same folder)': re.compile(r'(?i)same (?:extension |installation )?folder'),
    'an in-place update (replace the files)': re.compile(r'(?i)replace (?:the|its) (?:application )?files'),
    'an in-place update (Reload the extension)': re.compile(r'(?i)\b(?:select|click|choose|use|press)\s+\**reload\b(?! the page)'),
    'an in-place update (keep the installation)': re.compile(r'(?i)keep(?:ing)? the (?:existing|same) (?:extension )?installation'),
    'an in-place update (Arabic: Reload)': re.compile(r'(?:اختر|اضغط|استخدم|واختر|واضغط|واستخدم)\s+Reload|Reload\s*/\s*إعادة'),
    'an in-place update (Arabic: the same folder)': re.compile(r'مجلد(?:ها)? (?:الإضافة )?(?:الحالي|السابق)? ?نفسه|مجلدها الحالي'),
    'an in-place update (Arabic: replace the files)': re.compile(r'استبدل(?:\s+ال)?\s*(?:ال)?ملفات|مع استبدال الملفات'),
}

def texts():
    for base in SERVED:
        for path in sorted(base.rglob('*')):
            if path.is_file() and path.suffix.lower() not in BINARY:
                try:
                    yield path, path.read_text(encoding='utf-8')
                except UnicodeDecodeError:
                    pass
    yield QUICKSTART, QUICKSTART.read_text(encoding='utf-8')

def data(path, marker):
    text = path.read_text(encoding='utf-8')
    return json.JSONDecoder().raw_decode(text, text.index(marker) + len(marker))[0]

# README-IDE.md's upgrade order: each step's words, in order, per language (alternatives for each step).
ORDER = {
    'en': [r'export (?:your|the) wallet', r'\bRemove\b', r'signature', r'Load unpacked', r'extension ID', r'import (?:your|the) wallet'],
    'ar': [r'صدّر (?:محفظتك|المحفظة)', r'Remove|«إزالة»', r'توقيع', r'Load unpacked|«تحميل إضافة غير مضغوطة»',
           r'معرّف الإضافة', r'استورد (?:محفظتك|المحفظة|ملف المحفظة)'],
}

def in_order(text, steps):
    at = 0
    for step in steps:
        m = re.compile(step).search(text, at)
        if not m:
            return False
        at = m.end()
    return True

def published_ids():
    """(extension ID, release key fingerprint), computed here from the committed keys: Chrome's ID of the manifest key
    (sha256 of its DER, first 32 hex digits as a-p) and ssh-keygen's SHA256 fingerprint of the one allowed_signers key."""
    key_lines = [line.strip() for line in (ROOT / 'prototypes/pocol-extension/release/manifest-key.pub').read_text(encoding='utf-8').splitlines()
                 if line.strip() and not line.strip().startswith('#')]
    assert len(key_lines) == 1 and not key_lines[0].startswith('PLACEHOLDER'), key_lines
    digest = hashlib.sha256(base64.b64decode(key_lines[0], validate=True)).hexdigest()[:32]
    ext_id = ''.join(chr(ord('a') + int(c, 16)) for c in digest)
    signers = [line.split() for line in (ROOT / 'deploy/release/allowed_signers').read_text(encoding='utf-8').splitlines()
               if line.strip() and not line.startswith('#')]
    assert len(signers) == 1 and signers[0][2] == 'ssh-ed25519', signers
    blob = base64.b64decode(signers[0][3], validate=True)
    fingerprint = 'SHA256:' + base64.b64encode(hashlib.sha256(blob).digest()).decode('ascii').rstrip('=')
    return ext_id, fingerprint

class PublishedReleaseIds(unittest.TestCase):
    """Runbook step 7: the pages that tell users to compare the extension ID with "the one published on pocol.net" and
    to verify the release signature publish both values: the extension ID of the committed manifest key and the
    fingerprint of the committed release key, in English and Arabic."""
    def test_the_extension_and_download_pages_publish_the_extension_id_and_the_release_key_fingerprint(self):
        ext_id, fingerprint = published_ids()
        # The values the runbook and the release README name.
        self.assertEqual(ext_id, 'gggcoflmedlhlkcdhebmoepmggdnafmp')
        self.assertIn(f'256 {fingerprint} release@pocol.net (ED25519)', (ROOT / 'deploy/release/README.md').read_text(encoding='utf-8'))
        guide = data(MAIN / 'guide-content.js', 'window.POCOL_GUIDE = ')
        install = next(s for t in guide['topics'] if t['id'] == 'extension' for s in t['sections'] if s['id'] == 'install')
        locales = data(MAIN / 'about-locales.js', 'const extra = ')
        for lang in ('en', 'ar'):
            bullets = [b[lang] for b in install['bullets'] if ext_id in b[lang]]
            self.assertEqual(len(bullets), 1, (lang, 'the guide names the extension ID in one install bullet'))
            self.assertIn(fingerprint, bullets[0], lang)
            self.assertIn(ext_id, locales[lang]['ideReleaseIds'], lang)
            self.assertIn(fingerprint, locales[lang]['ideReleaseIds'], lang)
        for page in ('chrome-extension.html', 'index.html'):
            text = (MAIN / page).read_text(encoding='utf-8')
            self.assertIn('published on pocol.net', text, page)
            self.assertIn(ext_id, text, page)
            self.assertIn(fingerprint, text, page)
        index = (MAIN / 'index.html').read_text(encoding='utf-8')
        self.assertIn(f'data-i18n="ideReleaseIds">{locales["en"]["ideReleaseIds"]}</p>', index)
        # No page names another extension ID or release key fingerprint.
        for path, text in texts():
            for other in set(re.findall(r'SHA256:[A-Za-z0-9+/]{43}\b', text)) - {fingerprint}:
                self.fail(f'{path.relative_to(ROOT).as_posix()} names another key fingerprint {other}')

class RetiredTexts(unittest.TestCase):
    def test_no_served_file_names_the_activation_setting_or_an_in_place_update(self):
        seen = 0
        for path, text in texts():
            seen += 1
            for what, rx in RETIRED.items():
                m = rx.search(text)
                self.assertIsNone(m, f'{path.relative_to(ROOT).as_posix()} still has {what}: '
                                     f'{text[max(0, m.start() - 60):m.end() + 40]!r}' if m else '')
        self.assertGreater(seen, 40)

    def test_the_patterns_find_the_retired_texts(self):
        # Each pattern finds the text it was written for (the texts before W14), so a silent pattern cannot pass.
        for what, sample in (
                ('the retired activation setting', 'In extension Options → PoCol Mainnet activation, save'),
                ('the retired fork schedule', 'save [[1,\\"0\\"],[2,\\"15000\\"]]. Reopen'),
                ('the retired fork schedule', '[[1,&quot;0&quot;],[2,&quot;15000&quot;]]'),
                ('an in-place update (same folder)', 'replace the files in the same extension folder'),
                ('an in-place update (replace the files)', 'To update an existing installation, replace its files in'),
                ('an in-place update (Reload the extension)', 'and click Reload on its extension card'),
                ('an in-place update (keep the installation)', 'Keep the existing extension installation to retain'),
                ('an in-place update (Arabic: Reload)', 'ثم اضغط Reload من بطاقة الإضافة'),
                ('an in-place update (Arabic: Reload)', 'واضغط Reload/إعادة تحميل على إضافة PoCol'),
                ('an in-place update (Arabic: the same folder)', 'استبدل الملفات داخل مجلد الإضافة نفسه'),
                ('an in-place update (Arabic: the same folder)', 'استبدل الملفات في مجلدها الحالي'),
                ('an in-place update (Arabic: replace the files)', 'إلى المجلد السابق نفسه مع استبدال الملفات')):
            self.assertRegex(sample, RETIRED[what], what)
        # A page reload is not an extension update.
        for sample in ('Reload the page for a new snapshot.', 'The Solidity compiler could not load. Reload the page and try again.'):
            for what, rx in RETIRED.items():
                self.assertNotRegex(sample, rx, what)

class UpgradeOrder(unittest.TestCase):
    def test_the_guide_and_the_pages_give_the_readme_ide_order_in_both_languages(self):
        guide = data(MAIN / 'guide-content.js', 'window.POCOL_GUIDE = ')
        extension = next(t for t in guide['topics'] if t['id'] == 'extension')
        sections = {s['id']: s for s in extension['sections']}
        install = sections['install']['bullets']
        update = sections['update']['paragraphs'][0]
        locales = data(MAIN / 'about-locales.js', 'const extra = ')
        for lang in ('en', 'ar'):
            for where, text in (('guide install', [b[lang] for b in install if re.search(ORDER[lang][1], b[lang])]),
                                ('guide update', [update[lang]]), ('pocol.net IDE help', [locales[lang]['ideInstallBody']])):
                self.assertEqual(len(text), 1, (lang, where))
                self.assertTrue(in_order(text[0], ORDER[lang]), (lang, where, text[0]))
                self.assertIsNone(re.search(r'[0-9]+\.[0-9]+\.[0-9]+', text[0]), (lang, where, 'no version number'))
        # The English pages are the guide's and the dictionary's text.
        html = lambda s: s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;').replace('"', '&quot;')
        chrome = (MAIN / 'chrome-extension.html').read_text(encoding='utf-8')
        for bullet in install:
            if 'activation' not in bullet['en']:
                self.assertIn(f'<li>{html(bullet["en"])}</li>', chrome)
        self.assertIn(f'<p>{html(update["en"])}</p>', chrome)
        self.assertIn(f'data-i18n="ideInstallBody">{html(locales["en"]["ideInstallBody"])}</p>',
                      (MAIN / 'index.html').read_text(encoding='utf-8'))

    def test_the_native_paragraph_says_the_release_pins_the_network(self):
        guide = data(MAIN / 'guide-content.js', 'window.POCOL_GUIDE = ')
        ecosystem = next(t for t in guide['topics'] if t['id'] == 'ecosystem')
        native = next(s for s in ecosystem['sections'] if s['id'] == 'native-apps')['paragraphs'][0]
        locales = data(MAIN / 'about-locales.js', 'const extra = ')
        self.assertEqual(native['en'], locales['en']['appsUpgradeNotice'])
        self.assertEqual(native['ar'], locales['ar']['appsUpgradeNotice'])
        self.assertIn('pins the network', native['en'])
        self.assertIn('there is no activation setting', native['en'])
        self.assertIn('يثبّت كل إصدار الشبكة', native['ar'])
        self.assertIn('لا يوجد إعداد للتفعيل', native['ar'])
        for page in ('index.html', 'pocol-tools.html'):
            self.assertIn(native['en'], (MAIN / page).read_text(encoding='utf-8'), page)
        install = next(t for t in guide['topics'] if t['id'] == 'extension')['sections'][0]['bullets']
        self.assertTrue(any('there is no activation setting' in b['en'] and 'لا يوجد إعداد للتفعيل' in b['ar'] for b in install))

    def test_the_studio_install_dialog_gives_the_order_in_both_languages(self):
        app = (STUDIO / 'dist' / 'app.js').read_text(encoding='utf-8')
        steps = re.findall(r"install3:'([^']*)'", app)
        self.assertEqual(len(steps), 2)
        by_lang = {('ar' if re.search(r'[؀-ۿ]', s) else 'en'): s for s in steps}
        self.assertEqual(set(by_lang), {'en', 'ar'})
        for lang, text in by_lang.items():
            self.assertTrue(in_order(text, ORDER[lang]), (lang, text))
            self.assertIsNone(re.search(r'[0-9]+\.[0-9]+\.[0-9]+', text), lang)

    def test_the_studio_quick_start_gives_the_order_without_a_version(self):
        text = QUICKSTART.read_text(encoding='utf-8')
        section = text[text.index('\n3) '):text.index('\n4) ')]
        self.assertTrue(in_order(section, ORDER['ar']), section)
        numbered = re.findall(r'(?m)^([1-6])\. ', section)
        self.assertEqual(numbered, ['1', '2', '3', '4', '5', '6'])
        self.assertIsNone(re.search(r'[0-9]+\.[0-9]+\.[0-9]+', section), 'no version number in the upgrade section')

class DeployReadme(unittest.TestCase):
    def test_sections_2_and_4_are_for_a_fresh_network(self):
        text = (IDE / 'DEPLOY-README-AR.txt').read_text(encoding='utf-8')
        # The section headings, then each section's body after its heading line.
        two = text[text.index('\n2. إعداد النطاق'):text.index('\n3. التحقق بعد الرفع')].split('\n', 2)[2]
        four = text[text.index('\n4. الاتصال بمحفظة PoCol'):text.index('\n5. الملفات والمشاريع')].split('\n', 2)[2]
        # A new network has a new chain ID and genesis; the copy for it does not mix with an earlier one.
        self.assertIn('لكل شبكة جديدة Chain ID وGenesis جديدان', two)
        self.assertIn('لا تخلط ملفات النسختين', two)
        self.assertNotIn('اترك الأصول المطابقة متاحة', two)
        # The extension upgrade is README-IDE.md's order, numbered; no in-place rule, no old block check.
        self.assertTrue(in_order(four, ORDER['ar']), four)
        self.assertEqual(re.findall(r'(?m)^([1-6])\. ', four), ['1', '2', '3', '4', '5', '6'])
        self.assertNotIn('10922', text)
        self.assertNotIn('لا تحذف الإضافة ثم تعيد تثبيتها', text)
        self.assertNotIn('للإصدار 0.4.0', text)
        # The anchored network block stays (deploy/websites/tools/site_identity.py moves it at the identity switch).
        for label in ('Chain ID: ', 'Chain ID (hex): ', 'RPC: ', 'Genesis: '):
            self.assertEqual(len(re.findall(rf'(?m)^{re.escape(label)}\S+$', four)), 1, label)

if __name__ == '__main__':
    unittest.main()
