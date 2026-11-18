#!/usr/bin/env python3
"""Owner launch decisions of 2026-11-16 that the website snapshots carry: the site languages (English and Arabic only,
decision 3) and the explorer gateway host (explorer.pocol.net, decision 9). Decision 8 (no faucet on mainnet) was
superseded on 2026-11-18: the fee faucet runs on mainnet. Outside the tests and READMEs, the sites name it only in
pocol.net's claim page (faucet.html, faucet.js, faucet.css), the home page's Get started card link and its locales.js keys,
and SHA256SUMS.txt; the only faucet URLs are https://explorer.pocol.net/faucet/claim and /faucet/status."""
import json, re, unittest
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
                if path.name in self.PAGE_FILES:
                    continue
                if path.name == 'SHA256SUMS.txt':
                    lines = [line for line in text.splitlines() if re.search(r'faucet', line, re.I)]
                    self.assertTrue(all(re.fullmatch(r'[0-9a-f]{64}  faucet\.(?:html|js|css)', line) for line in lines), name)
                    self.assertEqual(len(lines), 3, name)
                    continue
                rest = {'index.html': re.sub(r'<a href="faucet\.html">|data-i18n="faucetCard(?:Title|Body)"', '', text),
                        'locales.js': re.sub(r'"faucetCard(?:Title|Body)": ', '', text)}.get(path.name)
                self.assertIsNotNone(rest, name)
                self.assertIsNone(re.search(r'faucet', rest, re.I), name)

if __name__ == '__main__':
    unittest.main()
