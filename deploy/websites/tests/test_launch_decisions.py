#!/usr/bin/env python3
"""Owner launch decisions of 2026-11-16 that the website snapshots carry: the site languages (English and Arabic only,
decision 3), the explorer gateway host (explorer.pocol.net, decision 9) and no faucet on the sites (decision 8)."""
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
        self.assertGreaterEqual(len(pages), 9)
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

class NoFaucet(unittest.TestCase):
    def test_no_site_links_a_faucet(self):
        for base in (ROOT / 'deploy' / 'websites', ROOT / 'prototypes' / 'pocol-studio' / 'dist'):
            for path, text in tracked_text(base):
                if 'tests' in path.parts or path.name == 'README.md':
                    continue
                # The IDE bundle is a third-party editor build; a faucet link would be a URL or a page text.
                self.assertIsNone(re.search(r'faucet', text, re.I) if path.suffix != '.js' or 'assets' not in path.parts
                                  else re.search(r'pocol[^`"\']{0,40}faucet|faucet\.pocol', text, re.I),
                                  path.relative_to(ROOT).as_posix())

if __name__ == '__main__':
    unittest.main()
