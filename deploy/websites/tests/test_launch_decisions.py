#!/usr/bin/env python3
"""Owner launch decisions of 2026-11-16 that the website snapshots carry: the site languages (English and Arabic only,
decision 3) and the explorer gateway host (explorer.pocol.net, decision 9). Decision 8 (no faucet on mainnet) was
superseded on 2026-11-18: the fee faucet runs on mainnet. Outside the tests and READMEs, the sites name it only in
pocol.net's claim page (faucet.html, faucet.js, faucet.css), the home page's Get started card link and its locales.js keys,
SHA256SUMS.txt and the privacy policy (privacy.html, privacy.js); the only faucet URLs are
https://explorer.pocol.net/faucet/claim and /faucet/status.
Gate Q1's unresolved-finding ledger and public limitations (development/public-network/FINDINGS.md, owner decisions
4 and 7 of 2026-11-18): every link and cited path exists, every F-NN id is unique."""
import html, json, re, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAIN = ROOT / 'deploy' / 'websites' / 'pocol.net'
LANGUAGES = {'ar', 'en'}
HOST = 'explorer.pocol.net'

def tracked_text(base):
    for path in sorted(base.rglob('*')):
        if path.is_file() and path.suffix.lower() not in {'.png', '.ico', '.zip', '.woff', '.woff2', '.ttf', '.webp', '.jpg'}:
            try:
                yield path, path.read_text(encoding='utf-8')
            except UnicodeDecodeError:
                pass

class Languages(unittest.TestCase):
    def test_every_page_offers_exactly_english_and_arabic(self):
        pages = sorted(MAIN.glob('*.html'))
        self.assertGreaterEqual(len(pages), 10)
        for page in pages:
            text = page.read_text(encoding='utf-8')
            selects = re.findall(r'<select id="language"[^>]*>(.*?)</select>', text, re.S)
            self.assertEqual(len(selects), 1, page.name)
            self.assertEqual(set(re.findall(r'<option value="([^"]+)"', selects[0])), LANGUAGES, page.name)
            self.assertEqual(len(re.findall(r'<option ', selects[0])), 2, page.name)

    def test_the_dictionaries_hold_exactly_english_and_arabic(self):
        text = (MAIN / 'locales.js').read_text(encoding='utf-8')
        prefix = 'window.POCOL_LOCALES = '
        self.assertTrue(text.startswith(prefix))
        locales = json.loads(text[len(prefix):].rstrip().rstrip(';'))
        self.assertEqual(set(locales), LANGUAGES)
        self.assertEqual(set(locales['en']), set(locales['ar']))
        about = (MAIN / 'about-locales.js').read_text(encoding='utf-8')
        start = about.index('const extra = ') + len('const extra = ')
        extra, _ = json.JSONDecoder().raw_decode(about, start)
        self.assertEqual(set(extra), LANGUAGES)
        self.assertEqual(set(extra['en']), set(extra['ar']))

    def test_the_guide_and_reward_texts_are_english_and_arabic(self):
        guide = (MAIN / 'guide-content.js').read_text(encoding='utf-8')
        prefix = 'window.POCOL_GUIDE = '
        self.assertTrue(guide.startswith(prefix))
        texts = []
        def walk(value):
            if isinstance(value, dict):
                if 'en' in value:
                    texts.append(set(value))
                for v in value.values():
                    walk(v)
            elif isinstance(value, list):
                for v in value:
                    walk(v)
        walk(json.loads(guide[len(prefix):].rstrip().rstrip(';')))
        self.assertTrue(texts)
        for keys in texts:
            self.assertEqual(keys, LANGUAGES)
        reward = (MAIN / 'reward.js').read_text(encoding='utf-8')
        self.assertRegex(reward, r'const translations = \{\s*en:')

    def test_the_site_script_accepts_only_english_and_arabic(self):
        app = (MAIN / 'app.js').read_text(encoding='utf-8')
        self.assertIn("const SUPPORTED = ['en', 'ar'];", app)
        self.assertIn("root.dir = lang === 'ar' ? 'rtl' : 'ltr';", app)

class ExplorerHost(unittest.TestCase):
    def test_the_retired_gateway_host_is_named_nowhere_served(self):
        for base in (ROOT / 'deploy' / 'websites', ROOT / 'prototypes' / 'pocol-studio' / 'dist',
                     ROOT / 'prototypes' / 'pocol-extension' / 'src', ROOT / 'deploy' / 'studio',
                     # Sites W8: the explorer gateway host's installer and Caddy site, and the gateway itself.
                     ROOT / 'deploy' / 'explorer', ROOT / 'deploy' / 'rewards'):
            for path, text in tracked_text(base):
                if 'tests' in path.parts or path.name == 'README.md':
                    continue
                if 'gateway.pocol.net' in text:
                    self.fail(f'{path.relative_to(ROOT).as_posix()} names the retired gateway host')

    def test_the_shipped_studio_release_notes_name_the_explorer_host(self):
        # build-release.py copies RELEASE-NOTES.md into every Studio release, so it is served too.
        notes = (ROOT / 'prototypes' / 'pocol-studio' / 'RELEASE-NOTES.md').read_text(encoding='utf-8')
        build = (ROOT / 'prototypes' / 'pocol-studio' / 'scripts' / 'build-release.py').read_text(encoding='utf-8')
        self.assertIn("(studio / 'RELEASE-NOTES.md', 'RELEASE-NOTES.md')", build)
        self.assertNotIn('gateway.pocol.net', notes)
        self.assertIn(f'`{HOST}/studio/rpc`', notes)

    def test_the_shipped_studio_release_notes_lead_with_the_package_version(self):
        # The notes are shipped with every Studio release: their first heading names the release they describe (they were
        # titled 0.1.1 after the step-7 release set package.json to 0.5.0).
        studio = ROOT / 'prototypes' / 'pocol-studio'
        version = json.loads((studio / 'package.json').read_text(encoding='utf-8'))['version']
        first = (studio / 'RELEASE-NOTES.md').read_text(encoding='utf-8').splitlines()[0]
        self.assertRegex(first, r'^# PoCol Studio [0-9]+\.[0-9]+\.[0-9]+ ')
        self.assertEqual(re.match(r'^# PoCol Studio ([0-9.]+) ', first).group(1), version, first)

    def test_the_gateway_readmes_say_the_explorer_dns_record_moves_at_launch(self):
        # The installer's TLS self-test needs explorer.pocol.net to point at the gateway server, which happens only
        # when the owner moves its record at launch; both language versions say so.
        studio = ROOT / 'deploy' / 'studio'
        english = (studio / 'README.md').read_text(encoding='utf-8')
        arabic = (studio / 'README-AR.txt').read_text(encoding='utf-8')
        self.assertIn('the owner moves its DNS record at launch', english)
        self.assertIn('ينقل المالك', arabic)
        self.assertIn('سجل DNS الخاص بـ explorer.pocol.net إلى خادم البوابة عند الإطلاق', arabic)
        self.assertNotIn('يشير نطاق explorer.pocol.net إلى خادم البوابة', arabic)
        # Sites W8: the explorer gateway installer (deploy/explorer) has the same TLS self-test.
        explorer = ROOT / 'deploy' / 'explorer'
        self.assertIn('the owner moves its DNS record at launch', (explorer / 'README.md').read_text(encoding='utf-8'))
        self.assertIn('سجل DNS الخاص بـ explorer.pocol.net إلى خادم البوابة عند الإطلاق',
                      (explorer / 'README-AR.txt').read_text(encoding='utf-8'))

    def test_the_studio_csp_connects_only_to_the_explorer_host(self):
        htaccess = (ROOT / 'prototypes' / 'pocol-studio' / 'dist' / '.htaccess').read_text(encoding='utf-8')
        csp = re.search(r'Content-Security-Policy "([^"]+)"', htaccess).group(1)
        connect = re.search(r"connect-src ([^;]+)", csp).group(1).split()
        self.assertEqual(connect, ["'self'", f'https://{HOST}'])
        app = (ROOT / 'prototypes' / 'pocol-studio' / 'dist' / 'app.js').read_text(encoding='utf-8')
        self.assertEqual(sorted(set(re.findall(r"fetch\('(https://[^']+)'", app))),
                         [f'https://{HOST}/studio/health', f'https://{HOST}/studio/rpc'])

    def test_the_declared_rpc_url_and_gateway_host(self):
        declared = json.loads((ROOT / 'deploy' / 'websites' / 'site-identity.json').read_text(encoding='utf-8'))
        self.assertEqual(declared['rpcUrl'], f'https://{HOST}/studio/rpc')
        self.assertEqual(declared['gatewayHost'], HOST)
        reward = (MAIN / 'api' / 'reward.php').read_text(encoding='utf-8')
        self.assertIn(f"const POCOL_REWARD_UPSTREAM = 'https://{HOST}/rpc';", reward)

    def test_the_extension_names_the_explorer_host(self):
        ext = ROOT / 'prototypes' / 'pocol-extension'
        self.assertIn(f"'https://{HOST}/*'", (ext / 'build.mjs').read_text(encoding='utf-8'))
        self.assertIn(f'"endpoint": "https://{HOST}/studio/rpc"', (ext / 'src' / 'lib' / 'studio-network.js').read_text(encoding='utf-8'))
        package = (ext / 'scripts' / 'package-release.py').read_text(encoding='utf-8')
        self.assertIn(f"RELEASE_ENDPOINT = 'https://{HOST}/studio/rpc'", package)

class FaucetPage(unittest.TestCase):
    STATUS = f'https://{HOST}/faucet/status'
    CLAIM = f'https://{HOST}/faucet/claim'
    CODES = {'invalid_address', 'reserved_address', 'contract_account', 'bad_request', 'too_large', 'unsupported_media',
             'not_found', 'deadline', 'already_claimed', 'not_new_account', 'already_funded', 'busy', 'daily_limit',
             'client_limit', 'signer_failed', 'node_refused', 'node_unavailable', 'unknown_outcome', 'wrong_network',
             'over_float', 'empty'}
    PAGE_FILES = {'faucet.html', 'faucet.js', 'faucet.css'}
    # The privacy policy (privacy.html and its text in privacy.js, 2026-11-22) describes what the faucet records; it
    # names no faucet URL or route (the checks above still apply to it).
    POLICY_FILES = {'privacy.html', 'privacy.js'}

    def script(self):
        return (MAIN / 'faucet.js').read_text(encoding='utf-8')

    def test_the_page_offers_english_and_arabic_and_loads_the_site_scripts(self):
        page = (MAIN / 'faucet.html').read_text(encoding='utf-8')
        select = re.findall(r'<select id="language"[^>]*>(.*?)</select>', page, re.S)
        self.assertEqual(len(select), 1)
        self.assertEqual(set(re.findall(r'<option value="([^"]+)"', select[0])), LANGUAGES)
        scripts = [s.split('?')[0] for s in re.findall(r'<script src="([^"]+)"', page)]
        self.assertEqual(scripts, ['locales.js', 'about-locales.js', 'language-picker.js', 'app.js', 'about-nav.js',
                                   'faucet.js'])
        for element in ('id="theme"', 'id="menu"', 'id="nav"', 'id="dialog"', 'id="language-dialog"', 'id="year"'):
            self.assertIn(element, page)
        index = (MAIN / 'index.html').read_text(encoding='utf-8')
        self.assertEqual(index.count('<a href="faucet.html">'), 1)

    def test_the_script_calls_only_the_two_faucet_routes(self):
        script = self.script()
        self.assertEqual(set(re.findall(r"'(https://[^']*)'", script)), {f'https://{HOST}', self.STATUS, self.CLAIM})
        self.assertEqual(re.findall(r'fetch\(([^,]+),', script), ['url'])
        self.assertEqual(sorted(re.findall(r'request\((STATUS_URL|CLAIM_URL),', script)), ['CLAIM_URL', 'STATUS_URL'])
        self.assertIn("body: JSON.stringify({ address })", script)
        self.assertIn("'Content-Type': 'application/json'", script)
        self.assertIn('const ADDRESS = /^0x[0-9a-fA-F]{40}$/;', script)
        self.assertIn('const CLAIM_TIMEOUT_MS = 60000;', script)
        self.assertNotIn('keepalive', script)
        # The faucet pins its own network; the page pins no chain ID.
        self.assertNotIn('chainId', script)
        self.assertNotIn('CHAIN_ID', script)
        # Server text is never parsed as HTML: the script writes text only.
        for sink in ('innerHTML', 'outerHTML', 'insertAdjacentHTML', 'document.write', 'eval(', 'new Function'):
            self.assertNotIn(sink, script)

    def test_every_text_and_every_error_code_is_english_and_arabic(self):
        script = self.script()
        start = script.index('const translations = ') + len('const translations = ')
        translations, _ = json.JSONDecoder().raw_decode(script, start)
        self.assertEqual(set(translations), LANGUAGES)
        self.assertEqual(set(translations['en']), set(translations['ar']))
        codes = re.search(r'const CODES = \[(.*?)\];', script, re.S).group(1)
        self.assertEqual(set(re.findall(r"'([a-z_]+)'", codes)), self.CODES)
        self.assertEqual(len(self.CODES), 21)
        for lang in LANGUAGES:
            self.assertEqual(set(translations[lang]['errors']), self.CODES, lang)
            texts = [v for k, v in translations[lang].items() if k != 'errors'] + list(translations[lang]['errors'].values())
            for text in texts:
                self.assertTrue(isinstance(text, str) and text.strip(), lang)
                if lang == 'ar':
                    self.assertRegex(text, r'[\u0600-\u06ff]')
        self.assertIn('check the address in the explorer before', translations['en']['errors']['unknown_outcome'].lower())
        self.assertIn('ربما أُرسل التحويل بالفعل؛ تحقّق من العنوان في المستكشف قبل إعادة المحاولة',
                      translations['ar']['errors']['unknown_outcome'])
        for lang in LANGUAGES:
            self.assertNotRegex(translations[lang]['errors']['unknown_outcome'], r'(?i)failed|فشل')

    def test_only_the_claim_page_names_the_faucet(self):
        endpoints = {self.STATUS, self.CLAIM}
        for base in (ROOT / 'deploy' / 'websites', ROOT / 'prototypes' / 'pocol-studio' / 'dist'):
            for path, text in tracked_text(base):
                if 'tests' in path.parts or path.name == 'README.md':
                    continue
                name = path.relative_to(ROOT).as_posix()
                if path.suffix == '.js' and 'assets' in path.parts:
                    # The IDE bundle is a third-party editor build; a faucet link would be a URL or a page text.
                    self.assertIsNone(re.search(r'pocol[^`"\']{0,40}faucet|faucet\.pocol', text, re.I), name)
                    continue
                if not re.search(r'faucet', text, re.I):
                    continue
                self.assertEqual(path.parent, MAIN, name)
                for url in re.findall(r'https?://[^\s"\'<>`)]*faucet[^\s"\'<>`)]*', text, re.I):
                    self.assertIn(url, endpoints, name)
                for route in re.findall(r'/faucet[\w/.-]*', text, re.I):
                    self.assertIn(route, {'/faucet/status', '/faucet/claim'}, name)
                if path.name in self.PAGE_FILES | self.POLICY_FILES:
                    continue
                if path.name == 'SHA256SUMS.txt':
                    lines = [line for line in text.splitlines() if re.search(r'faucet', line, re.I)]
                    self.assertTrue(all(re.fullmatch(r'[0-9a-f]{64}  faucet\.(?:html|js|css)', line) for line in lines), name)
                    self.assertEqual(len(lines), 3, name)
                    continue
                if path.name == 'about-locales.js':
                    text = re.sub(r'"walletFaucet(?:Title|Body|Steps|Limits|Button|Eyebrow)"\s*:\s*"(?:\\.|[^"\\])*"', '', text)
                    self.assertIsNone(re.search(r'faucet', text, re.I), name)
                    continue
                if path.name == 'index.html':
                    self.assertEqual(text.count('id="wallet-fee-faucet"'), 1, name)
                    text, count = re.subn(r'<article id="wallet-fee-faucet"[^>]*>.*?</article>', '', text, flags=re.S)
                    self.assertEqual(count, 1, name)
                rest = {'index.html': re.sub(r'<a href="faucet\.html">|data-i18n="faucetCard(?:Title|Body)"', '', text),
                        'locales.js': re.sub(r'"faucetCard(?:Title|Body)": ', '', text)}.get(path.name)
                self.assertIsNotNone(rest, name)
                self.assertIsNone(re.search(r'faucet', rest, re.I), name)

class LaunchTexts(unittest.TestCase):
    """Owner decision 4 (2026-11-18): no served text states anything untrue or presents the public launch as prevented
    or conditional; the node's readiness report is node-local and refuses nothing. The texts that did (and the retired
    network's upgrade and migration steps, which are untrue on the new network) were replaced; none may come back.
    OWNER-DECISIONS-2026-11-20 (D1, D4, D5, D6): PoCol Mainnet is the public network, launched without a separate
    rehearsal, admitted in scope public-network, with launch approval reported apart from technical readiness and every
    value computed by the node. No served text or client user interface states the old experimental admission, a fixed
    readiness value or a launch after a closed rehearsal (CLAIMS)."""
    SERVED = [ROOT / 'deploy' / 'websites' / site for site in ('explorer.pocol.net', 'ide.pocol.net', 'pocol.net')] + \
             [ROOT / 'prototypes' / 'pocol-studio' / 'dist']
    REMOVED = {
        'production readiness': re.compile(r'(?i)production[- ]read(?:y|iness)'),
        'simnet-only': re.compile(r'(?i)simnet-only'),
        'production qualification': re.compile(r'(?i)production qualification'),
        'production admission': re.compile(r'(?i)production admission'),
        'production blockers': re.compile(r'(?i)production blockers'),
        'a pending Chrome Web Store release': re.compile(r'(?i)until Chrome Web Store|Web Store installation is not available yet'),
        'the retired upgrade path': re.compile(r'Upgrade existing network|migration support|block 15000|Install-PoCol-Explorer'),
        'a pending HTTPS domain': re.compile(r'(?i)when a domain is available'),
        'Arabic: production readiness': re.compile(r'الجاهزية للإنتاج|جاهز للإنتاج|للاستخدام الإنتاجي'),
        'Arabic: production qualification': re.compile(r'(?:التأهيل|تأهيلها) للإنتاج'),
        'Arabic: production operation': re.compile(r'التشغيل الإنتاجي|عوائق الإنتاج|مراجعة أمنية إنتاجية'),
        'Arabic: a required release step': re.compile(r'خطوة إصدار مطلوبة'),
        'Arabic: a pending Chrome store release': re.compile(r'إلى حين نشرها في متجر Chrome|متجر Chrome لم يُتح بعد'),
        'Arabic: the retired upgrade path': re.compile(r'مساعدة الترحيل|الكتلة 15000'),
        'a pending Android update': re.compile(r'(?i)Android update is being prepared|Android update in preparation'),
        'Arabic: a pending Android update': re.compile(r'تحديث Android قيد التجهيز|يجري تجهيز تحديث Android'),
        'a key backup the retired wallet lacks': re.compile(r'back it up first \(Settings → Back up key'),
        'Arabic: a key backup the retired wallet lacks': re.compile(r'فاحفظ نسخة احتياطية منه أولًا \(Settings'),
    }
    # Checked in the served files and in the clients' user interface sources (the extension pages and scripts, the
    # Windows wallet and the Android app).
    CLIENTS = [ROOT / 'prototypes' / 'pocol-extension' / 'src', ROOT / 'prototypes' / 'pocol-windows',
               ROOT / 'prototypes' / 'pocol-android' / 'app' / 'src' / 'main']
    CLAIMS = {
        'a hard-coded experimental admission': re.compile(r'(?i)Simnet profile in ExperimentalDevnet|(?:ExperimentalDevnet|experimental-devnet) scope'),
        'a hard-coded readiness value': re.compile(r'(?i)publicLaunch(?:Ready|Approved)"?\s*:\s*(?:false|true)\b'),
        'the superseded R22 and R21 status': re.compile(r'(?i)binds the separately defined Network profile to chain ID 777001|only through the simnet retentionStress exception'),
        'a launch after a closed rehearsal': re.compile(r'(?i)after (?:a |the )?(?:closed )?rehearsal|rehearsal (?:comes |runs )?(?:first|before the (?:public )?launch)'),
        'Arabic: a hard-coded experimental admission': re.compile(r'بملف Simnet في نطاق|في نطاق (?:ExperimentalDevnet|experimental-devnet)'),
        'Arabic: the superseded R22 and R21 status': re.compile(r'المعرَّف على حدة بمعرّف السلسلة 777001|استثناء retentionStress'),
        'Arabic: a launch after a closed rehearsal': re.compile(r'بعد (?:ال)?تجربة (?:ال)?مغلقة|بعد (?:ال)?(?:بروفة|تجربة الإطلاق)'),
    }

    def test_no_served_file_has_a_removed_launch_text(self):
        seen = 0
        for base in self.SERVED:
            for path, text in tracked_text(base):
                seen += 1
                for what, rx in self.REMOVED.items():
                    m = rx.search(text)
                    self.assertIsNone(m, f'{path.relative_to(ROOT).as_posix()} has {what}: '
                                         f'{text[max(0, m.start() - 60):m.end() + 40]!r}' if m else '')
        self.assertGreater(seen, 40)

    # The current operator and reference documents that describe PoCol Mainnet (the bundle README is copied into every
    # seed and join bundle, deploy/public-testnet/prepare.py). Decision records, audit reports and results keep their
    # text and are not listed.
    DOCUMENTS = ['README.md', 'development/m1/reference/threat.md', 'development/public-network/PLAN.md',
                 'deploy/public-testnet/README.md', 'deploy/mainnet/README.md', 'deploy/standalone/README.md']

    def test_no_current_document_has_a_superseded_launch_claim(self):
        for name in self.DOCUMENTS:
            text = (ROOT / name).read_text(encoding='utf-8')
            for what, rx in self.CLAIMS.items():
                m = rx.search(text)
                self.assertIsNone(m, f'{name} has {what}: {text[max(0, m.start() - 60):m.end() + 40]!r}' if m else '')
        # The bundle README names the fields the node computes (readiness.rs json()) and where the open gates and the
        # admission's conditions and waivers are reported, and calls PCOL a test currency only off the public chain.
        bundle = ' '.join((ROOT / 'deploy' / 'public-testnet' / 'README.md').read_text(encoding='utf-8').split())
        for name in ('`publicLaunchApproved`', '`launchApprovalReference`', '`publicLaunchReady`', '`launchBlockers`',
                     '`technicalReadiness.pending`', '`public-network`', '`experimental-devnet`', '`testCurrency`'):
            self.assertIn(name, bundle)
        self.assertNotIn('request test coins', bundle)
        self.assertIn("on PoCol Mainnet's chain it is not", bundle)

    def test_no_served_file_or_client_has_a_superseded_launch_claim(self):
        seen = 0
        for base in self.SERVED + self.CLIENTS:
            for path, text in tracked_text(base):
                seen += 1
                for what, rx in self.CLAIMS.items():
                    m = rx.search(text)
                    self.assertIsNone(m, f'{path.relative_to(ROOT).as_posix()} has {what}: '
                                         f'{text[max(0, m.start() - 60):m.end() + 40]!r}' if m else '')
        self.assertGreater(seen, 80)

    def test_the_claim_patterns_find_the_superseded_texts(self):
        for what, sample in (
                ('a hard-coded experimental admission', 'The node admits it as a Simnet profile in ExperimentalDevnet scope'),
                ('a hard-coded experimental admission', 'Every founder node runs in experimental-devnet scope.'),
                ('a hard-coded readiness value', 'and reports publicLaunchReady: false with its launchBlockers'),
                ('a hard-coded readiness value', '{"publicLaunchApproved":true}'),
                ('the superseded R22 and R21 status', 'Rule R22 binds the separately defined Network profile to chain ID 777001'),
                ('the superseded R22 and R21 status', "rule R21 holds at the network's caps only through the simnet retentionStress exception"),
                ('a launch after a closed rehearsal', "The public launch follows after a closed rehearsal on the founders' devices."),
                ('a launch after a closed rehearsal', 'PoCol Mainnet opens to the public after the rehearsal.'),
                ('a launch after a closed rehearsal', 'The rehearsal runs before the public launch.'),
                ('Arabic: a hard-coded experimental admission', 'وتقبلها العقدة بملف Simnet في نطاق ExperimentalDevnet'),
                ('Arabic: a hard-coded experimental admission', 'تعمل العقد في نطاق experimental-devnet'),
                ('Arabic: the superseded R22 and R21 status', 'تربط القاعدة R22 ملف الشبكة Network المعرَّف على حدة بمعرّف السلسلة 777001'),
                ('Arabic: the superseded R22 and R21 status', 'إلا عبر استثناء retentionStress الخاص بشبكات simnet'),
                ('Arabic: a launch after a closed rehearsal', 'يأتي الإطلاق العام بعد تجربة مغلقة على أجهزة المؤسسين.'),
                ('Arabic: a launch after a closed rehearsal', 'تُفتح الشبكة للجمهور بعد البروفة.')):
            self.assertRegex(sample, self.CLAIMS[what], what)
        # The status texts that replaced them are not refused.
        for sample in ('It launches from a fresh genesis under the v3 rule set by the owner\'s decision of 2026-11-20, without a separate rehearsal,',
                       'دون تجربة إطلاق منفصلة (rehearsal)، بينما ما زالت بواباتها التقنية مفتوحة.',
                       # The status page's wording since the launch on 2026-11-21 (2026-11-23).
                       "It launched from a fresh genesis on 2026-11-21 (genesis time 20:20:56 UTC) under the v3 rule set, by the owner's decision of 2026-11-20 and without a separate rehearsal;",
                       'بقرار المالك المؤرخ 2026-11-20 ودون تجربة إطلاق منفصلة (rehearsal)، وما زالت بواباتها التقنية مفتوحة.'):
            for what, rx in self.CLAIMS.items():
                self.assertNotRegex(sample, rx, what)

    def test_the_guide_cites_no_superseded_launch_text(self):
        # "Repository links pin the source revision they cite": each repository source the guide renders is read at its
        # pinned revision and anchor (git show; websites.yml keeps the history). Outside the decision records, the
        # cited text has no superseded launch claim (CLAIMS); a decision record keeps its text, so the cited lines of one
        # that does carry the owner's superseded note of 2026-11-20. The status page's README source is the "Current
        # status and limits" section.
        import subprocess
        if subprocess.run(['git', '-C', str(ROOT), 'rev-parse', '--git-dir'], capture_output=True).returncode != 0:
            self.skipTest('not a git checkout: the pinned revisions cannot be read')
        guide = json.loads((MAIN / 'guide-content.js').read_text(encoding='utf-8')[len('window.POCOL_GUIDE = '):].rstrip().rstrip(';'))
        checked = 0
        for topic in guide['topics']:
            for source in topic['sources']:
                if 'url' in source:
                    continue
                ref = source.get('ref', guide['sha'])
                shown = subprocess.run(['git', '-C', str(ROOT), 'show', f"{ref}:{source['path']}"], capture_output=True)
                self.assertEqual(shown.returncode, 0, f"{topic['id']}: {source['path']} at {ref} is not in this checkout "
                                                      f"(fetch the history): {shown.stderr.decode(errors='replace')}")
                text = shown.stdout.decode('utf-8')
                if source.get('anchor'):
                    m = re.fullmatch(r'L(\d+)(?:-L(\d+))?', source['anchor'])
                    lines = text.splitlines()
                    lo, hi = int(m.group(1)), int(m.group(2) or m.group(1))
                    self.assertLessEqual(hi, len(lines), (topic['id'], source['path'], source['anchor']))
                    text = '\n'.join(lines[lo - 1:hi])
                hits = [what for what, rx in self.CLAIMS.items() if rx.search(text)]
                where = (topic['id'], source['path'], source.get('anchor'), ref[:7], hits)
                if source['path'].startswith('development/decisions/'):
                    if hits:
                        self.assertIn('Superseded by owner decision 2026-11-20', text, where)
                else:
                    self.assertEqual(hits, [], where)
                if topic['id'] == 'status' and source['path'] == 'README.md' and source['label']['en'] == 'Current status and limits':
                    self.assertTrue(text.startswith('## Current status and limits\n'), where)
                    self.assertIn('public-network', text, where)
                checked += 1
        self.assertGreater(checked, 30)

    def test_the_patterns_find_the_removed_texts(self):
        # Each pattern finds the text it was written for, so a silent pattern cannot pass.
        for what, sample in (
                ('production readiness', 'Production readiness is not claimed'),
                ('production readiness', 'The current README labels the code simnet-only and not production-ready.'),
                ('simnet-only', 'labels the code simnet-only'),
                ('production qualification', 'it does not imply completed production qualification.'),
                ('production admission', 'while production admission, open security questions and independent audit remain'),
                ('production blockers', 'Current status and production blockers'),
                ('a pending Chrome Web Store release', 'Manual extension installation is needed until Chrome Web Store publication.'),
                ('a pending Chrome Web Store release', 'Chrome Web Store installation is not available yet.'),
                ('the retired upgrade path', 'Use Settings → Upgrade existing network while disconnected.'),
                ('the retired upgrade path', 'retain all data and obtain migration support before proceeding.'),
                ('the retired upgrade path', 'Use the supplied Install-PoCol-Explorer.sh installer'),
                ('a pending HTTPS domain', 'A verified HTTPS gateway can be configured when a domain is available.'),
                ('Arabic: production readiness', 'الجاهزية للإنتاج غير مثبتة'),
                ('Arabic: production readiness', 'بأنه مخصص لشبكات simnet وغير جاهز للإنتاج'),
                ('Arabic: production qualification', 'ولا يعني اكتمال تأهيلها للإنتاج.'),
                ('Arabic: production qualification', 'أو اكتمال التأهيل للإنتاج.'),
                ('Arabic: production operation', 'بينما يظل التشغيل الإنتاجي والمسائل الأمنية المفتوحة'),
                ('Arabic: production operation', 'الحالة الحالية وعوائق الإنتاج'),
                ('Arabic: a required release step', 'وظهور إيصال النشر على الشبكة خطوة إصدار مطلوبة.'),
                ('Arabic: a pending Chrome store release', 'والإضافة تحتاج تثبيتًا يدويًا إلى حين نشرها في متجر Chrome.'),
                ('Arabic: the retired upgrade path', 'احتفظ بجميع البيانات واطلب مساعدة الترحيل قبل المتابعة.'),
                ('a pending Android update', 'The Android update is being prepared with release signing.'),
                ('a pending Android update', 'Android update in preparation'),
                ('Arabic: a pending Android update', 'تحديث Android قيد التجهيز'),
                ('Arabic: a pending Android update', 'يجري تجهيز تحديث Android بتوقيع الإصدار.'),
                ('a key backup the retired wallet lacks',
                 'If you want to keep your old key, back it up first (Settings → Back up key to encrypted file, or pocol.exe export-key).'),
                ('Arabic: a key backup the retired wallet lacks',
                 'إن أردت الاحتفاظ بمفتاحك القديم فاحفظ نسخة احتياطية منه أولًا (Settings ثم Back up key to encrypted file، أو الأمر pocol.exe export-key).')):
            self.assertRegex(sample, self.REMOVED[what], what)

    def test_the_home_page_fallbacks_are_the_english_locale(self):
        # The static English of the wallet-download section (no JavaScript, crawlers, the first paint) is the locale's
        # English, so a text replaced in about-locales.js cannot stay behind in index.html.
        locales = (MAIN / 'about-locales.js').read_text(encoding='utf-8')
        english = json.loads(re.search(r'const extra = (\{.*?\}\}); for', locales, re.S).group(1))['en']
        page = (MAIN / 'index.html').read_text(encoding='utf-8')
        checked = 0
        for tag, key, text in re.findall(r'<(\w+)[^>]*\bdata-i18n="(apps\w+)"[^>]*>(.*?)</\1>', page, re.S):
            self.assertIn(key, english, key)
            self.assertEqual(html.unescape(text), english[key], key)
            checked += 1
        self.assertGreater(checked, 25)
        # The retired network's old key stays in the folder moved aside; the 0.4.0 wallet has no encrypted key backup.
        self.assertIn('your old key stays in that folder', english['appsWindowsUpdateBody'])
        self.assertIn('\u200e%LOCALAPPDATA%\\PoCol', json.loads(re.search(r'const extra = (\{.*?\}\}); for', locales, re.S).group(1))['ar']['appsWindowsUpdateBody'])

    def test_the_status_page_reports_the_gates_without_blocking_the_launch(self):
        guide = json.loads((MAIN / 'guide-content.js').read_text(encoding='utf-8')[len('window.POCOL_GUIDE = '):].rstrip().rstrip(';'))
        status = next(t for t in guide['topics'] if t['id'] == 'status')
        launch = next(s for s in status['sections'] if s['id'] == 'production')
        self.assertEqual(launch['title'], {'en': 'Launch status and open gates', 'ar': 'حالة الإطلاق والبوابات التي ما زالت مفتوحة'})
        self.assertIn('This report is local to the node and stops nothing', launch['paragraphs'][0]['en'])
        self.assertIn('وهذا التقرير محلي في العقدة ولا يمنع شيئًا', launch['paragraphs'][0]['ar'])
        page = (MAIN / 'pocol-status.html').read_text(encoding='utf-8')
        self.assertEqual(page.count('Launch status and open gates'), 2)  # the heading and the page's table of contents
        # OWNER-DECISIONS-2026-11-20 D4, D5, D6: the text names the public network, its admission scope and waivers, and
        # the approval and technical readiness fields as the node computes them (readiness.rs json(), scope.rs and
        # ExpectedAdmission::PUBLIC_MAINNET), in both languages and on the static page.
        readiness = (ROOT / 'prototypes' / 'pocold' / 'src' / 'readiness.rs').read_text(encoding='utf-8')
        emitted = re.search(r'pub fn json\(r:&Readiness\)->serde_json::Value\{(.*?)\n\}', readiness, re.S).group(1)
        fields = ['publicLaunchApproved', 'launchApprovalReference', 'publicLaunchReady', 'launchBlockers', 'technicalReadiness']
        for field in fields:
            self.assertIn(f'"{field}"', emitted, field)
        scope = re.search(r'Scope::PublicNetwork=>"([a-z-]+)"', (ROOT / 'prototypes' / 'lp4-execution' / 'src' / 'admission' / 'scope.rs').read_text(encoding='utf-8')).group(1)
        self.assertEqual(scope, 'public-network')
        waivers = re.findall(r'"(R\d+)"', re.search(r'const PUBLIC_MAINNET:Self=Self\{.*?waivers:&\[([^\]]*)\]', readiness, re.S).group(1))
        self.assertEqual(waivers, ['R13', 'R21', 'R26'])
        static = html.unescape(re.search(r'<section class="doc-section" id="production">(.*?)</section>', page, re.S).group(1))
        for lang, text in (('en', launch['paragraphs'][0]['en']), ('ar', launch['paragraphs'][0]['ar']), ('static', static)):
            for name in fields + [scope] + waivers:
                self.assertIn(name, text, (lang, name))
        self.assertTrue(launch['paragraphs'][0]['en'].startswith('PoCol Mainnet is the public network.'))
        self.assertTrue(launch['paragraphs'][0]['ar'].startswith('PoCol Mainnet هي الشبكة العامة.'))
        self.assertIn(launch['paragraphs'][0]['en'], static)
        # Every repository source link the guide renders pins a full commit, and the static list shows the same ones.
        for topic in guide['topics']:
            static = (MAIN / guide['files'][topic['id']]).read_text(encoding='utf-8')
            listed = re.search(r'<details class="doc-sources" open>.*?</details>', static, re.S).group(0)
            for source in topic['sources']:
                if 'url' in source:
                    continue
                ref = source.get('ref', guide['sha'])
                self.assertRegex(ref, r'^[0-9a-f]{40}$')
                href = f"https://github.com/raedrasheed/pocol/blob/{ref}/{source['path']}" + (f"#{source['anchor']}" if source.get('anchor') else '')
                self.assertIn(f'<a href="{href}">', listed, topic['id'])
            self.assertEqual(len(re.findall(r'<li>', listed)), len(topic['sources']), topic['id'])
            for code, href_ref in re.findall(r'href="https://github\.com/raedrasheed/pocol/blob/([0-9a-f]{40})/[^"]*">[^<]*</a> · <code>([0-9a-f]+)</code>', listed):
                self.assertEqual(code[:7], href_ref, topic['id'])


class FindingsLedger(unittest.TestCase):
    """development/public-network/FINDINGS.md, gate Q1's unresolved-finding ledger and public limitations. Every relative
    link and cited repository path exists (a cited line number is at most its file's line count; the content at that
    line is not checked), every F-NN id is unique and in order, and, for owner decision 4, none of a fixed list of
    phrases appears that call the launch not ready, blocked, prevented or conditional (other wording is not judged).
    .github/workflows/findings-ledger.yml runs this class on every push and pull request to main."""
    PATH = ROOT / 'development' / 'public-network' / 'FINDINGS.md'
    KINDS = {'open-finding', 'limitation', 'accepted-risk'}
    SEVERITIES = {'high', 'medium', 'low', 'info'}
    GATES = {'K1', 'J1', 'I1', 'A1', 'S1', 'N1', 'L1', 'E1', 'Q1'}
    LINK = re.compile(r'\[([^\]]*)\]\(([^)\s]+)\)')
    CITED = re.compile(r'`((?:prototypes|development|deploy|\.github)/[\w./-]*?)(?::([\d,-]+))?`')
    REMOVED = re.compile(r'(?i)not (?:yet )?ready (?:for|to) (?:a |the )?(?:full )?(?:public )?launch|'
                         r'not (?:yet )?public-launch-ready|ليست جاهزة للاطلاق|'
                         r'\blaunch (?:is|stays|remains) (?:blocked|prevented|conditional|on hold)|'
                         r'\b(?:blocks?|prevents?|stops?|holds? back) (?:the )?(?:public )?launch\b|'
                         r'\bbefore (?:the )?(?:public )?launch (?:can|may)\b|'
                         r'(?<!not a )\b(?:public )?launch (?:condition|prerequisite)s?\b')
    # Sentences the pattern must catch (and one it must leave: the ledger says severity "is not a launch condition").
    CAUGHT = ('The network is not ready for a public launch.', 'The launch is blocked until the review.',
              'This blocks the public launch.', 'It must be fixed before the launch can go ahead.',
              'A launch condition remains.', 'The node is not public-launch-ready.')

    def text(self):
        return self.PATH.read_text(encoding='utf-8')

    def rows(self):
        rows = []
        for line in self.text().splitlines():
            if line.startswith('| F-'):
                cells = [c.strip() for c in re.split(r'(?<!\\)\|', line)[1:-1]]
                self.assertEqual(len(cells), 7, line[:80])
                rows.append(cells)
        return rows

    def test_every_relative_link_exists_and_cited_lines_lie_inside_their_file(self):
        checked = 0
        for label, target in self.LINK.findall(self.text()):
            if re.match(r'(?i)(?:https?:|mailto:|#)', target):
                continue
            path = (self.PATH.parent / target.split('#')[0]).resolve()
            self.assertTrue(path.exists(), f'{label} -> {target}')
            checked += 1
            m = re.fullmatch(r'([^:\s]+):([\d,-]+)', label)
            if m:
                self.assertEqual((ROOT / m.group(1)).resolve(), path, label)
                lines = self.line_count(path)
                for number in re.findall(r'\d+', m.group(2)):
                    self.assertLessEqual(int(number), lines, label)
                spans = [tuple(map(int, r.split('-'))) for r in m.group(2).split(',') if '-' in r]
                self.assertTrue(all(a <= b for a, b in spans), label)
            elif re.fullmatch(r'[\w.-]+/[\w./-]*', label):
                # A source shown as a bare repository path links to that path.
                self.assertEqual((ROOT / label).resolve(), path, label)
        self.assertGreater(checked, 500)

    @staticmethod
    def line_count(path):
        return len(path.read_text(encoding='utf-8', errors='replace').splitlines())

    def test_every_cited_repository_path_exists(self):
        cited = set(self.CITED.findall(self.text()))
        self.assertTrue(cited)
        for path, numbers in sorted(cited):
            self.assertTrue((ROOT / path).exists(), path)
            if numbers:
                self.assertTrue((ROOT / path).is_file(), path)
                lines = self.line_count(ROOT / path)
                for number in re.findall(r'\d+', numbers):
                    self.assertLessEqual(int(number), lines, f'{path}:{numbers}')

    def test_ids_are_unique_and_in_order_and_every_row_is_complete(self):
        rows = self.rows()
        ids = [r[0] for r in rows]
        self.assertGreater(len(ids), 50)
        self.assertEqual(len(ids), len(set(ids)), 'duplicate F-NN id')
        self.assertEqual(ids, [f'F-{n:02d}' for n in range(1, len(ids) + 1)])
        for fid, area, kind, severity, statement, gate, sources in rows:
            self.assertTrue(area, fid)
            self.assertIn(kind, self.KINDS, fid)
            self.assertIn(severity, self.SEVERITIES, fid)
            self.assertRegex(statement, r'^\*\*[^*]+\.\*\* \S', fid)
            self.assertTrue(gate == '-' or set(g.strip() for g in gate.split(',')) <= self.GATES, fid)
            self.assertTrue(self.LINK.search(sources), fid)
        # Every F-NN reference anywhere in the file names an entry of the table.
        self.assertLessEqual(set(re.findall(r'\bF-\d{2}\b', self.text())), set(ids))
        # The count stated in the header is the table's.
        self.assertEqual(re.search(r'The ledger has\s+(\d+) entries', self.text()).group(1), str(len(ids)))

    def test_the_text_keeps_q1_open_and_never_presents_the_launch_as_prevented(self):
        text = self.text()
        self.assertIsNone(self.REMOVED.search(text), self.REMOVED.search(text) and self.REMOVED.search(text).group(0))
        for sentence in self.CAUGHT:
            self.assertRegex(sentence, self.REMOVED)
        self.assertNotRegex('It is not a launch condition.', self.REMOVED)
        self.assertIn('Q1 stays open', text)
        self.assertIn('the owner commissions an independent security review\n  after the launch', text)
        self.assertIn('## Public limitations', text)
        self.assertIn('## Adding and closing entries', text)
        plan = (ROOT / 'development' / 'public-network' / 'PLAN.md').read_text(encoding='utf-8')
        q1 = next(line for line in plan.splitlines() if line.startswith('| Q1:'))
        cells = [c.strip() for c in q1.split('|')]
        self.assertTrue(cells[3].startswith('Open;'), q1)
        self.assertIn('[FINDINGS.md](FINDINGS.md)', cells[3])

if __name__ == '__main__':
    unittest.main()
