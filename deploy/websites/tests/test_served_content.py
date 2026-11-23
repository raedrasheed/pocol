#!/usr/bin/env python3
"""What pocol.net's document root serves (the owner's request of 2026-11-23 for the cPanel deployment of PoCol Mainnet):
- no served file names a test network ("Testnet", "test net", in English or Arabic), not even as a hidden i18n key, and
  none names a release below 0.5.0 (the retired network's 0.4.x and 0.3.x packages; PoCol Mainnet's first release is
  0.5.0). A version is a dotted triple that is not part of a longer dotted number, so 127.0.0.1 is not one;
- the retired network is only named where that is true and needed: the migration warnings (balances do not carry over,
  move the old wallet folder aside, the earlier Android app is not updated, an old wallet home cannot be used) and the
  404 page's note that an address may have belonged to it;
- sitemap.xml lists exactly the served pages, each a file of the snapshot, and robots.txt names it. 404.html is not a
  page to index, and the faucet claim page faucet.html is left out on purpose: test_launch_decisions.FaucetPage keeps
  the files that name the faucet to the claim page, the home page (which links it), locales.js, SHA256SUMS.txt and the
  privacy policy, and that rule stays;
- the document root holds no deployment instructions or other owner notes: they ship beside the ZIP
  (deploy/websites/tools/package_site.py), so https://pocol.net/ never serves them (2026-11-23: README-AR.txt said the
  copy came before the new network and named host internals).
The files are the tracked ones (git ls-files), so an ignored or untracked file in a checkout is not judged."""
import re, subprocess, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SITE = 'deploy/websites/pocol.net'
MAIN = ROOT / SITE
TEXT = ('.html', '.js', '.css', '.txt', '.xml', '.json', '.webmanifest', '.php', '.svg')
TESTNET = re.compile(r'(?i)test[\s_-]?net|الشبكة التجريبية|شبكة تجريبية|شبكة الاختبار|شبكة اختبار')
OLD_RELEASE = re.compile(r'(?<![0-9.])0\.[0-4]\.[0-9]+(?![0-9.])')
RETIRED = re.compile(r'(?i)retired network|الشبكة السابقة|للشبكة السابقة|earlier Android app')

def tracked():
    out = subprocess.run(['git', '-C', str(ROOT), 'ls-files', '-z', '--', SITE], capture_output=True, check=True, timeout=60)
    return sorted(p[len(SITE) + 1:] for p in out.stdout.decode('utf-8').split('\0') if p)

def served_text():
    for rel in tracked():
        if rel.endswith(TEXT) or rel == '.htaccess':
            yield rel, (MAIN / rel).read_text(encoding='utf-8')

class ServedContent(unittest.TestCase):
    def test_the_patterns_see_what_they_refuse(self):
        for sample in ('PoCol Testnet', 'data-i18n="networkTestnet"', 'the test net', 'test-net', 'على الشبكة التجريبية', 'شبكة الاختبار'):
            self.assertRegex(sample, TESTNET)
        for sample in ('PoCol-Mainnet-0.4.0-Windows.zip', 'version 0.3.9', '(0.4.1)', 'إصدار 0.4.0'):
            self.assertRegex(sample, OLD_RELEASE)
        for sample in ('127.0.0.1', '0.5.0', '10.0.4.1', '0.4.0.1', 'v=0.40.1'):
            self.assertNotRegex(sample, OLD_RELEASE)

    def test_no_served_file_names_a_test_network_or_a_release_before_0_5_0(self):
        seen = 0
        for rel, text in served_text():
            seen += 1
            m = TESTNET.search(text)
            self.assertIsNone(m, f'{rel}: {text[max(0, m.start() - 60):m.end() + 40]!r}' if m else '')
            m = OLD_RELEASE.search(text)
            self.assertIsNone(m, f'{rel}: {text[max(0, m.start() - 60):m.end() + 40]!r}' if m else '')
        self.assertGreater(seen, 30)

    # The served texts that may name the retired network, each true and needed (2026-11-23 review): the migration
    # warnings and the 404 note. Every sentence naming it must contain one of these, so a new mention fails until it is
    # judged here.
    KEPT = ('do not carry over', 'لا تنتقل', 'cannot be used on this network', 'لا يمكن استخدام',
            'belonged to the retired network', 'تخص الشبكة السابقة', 'does not update', 'لا يحدّث',
            'Coming from the', 'هل كنت تستخدم', 'links no package of the retired network', 'ولا تربط أي حزمة للشبكة السابقة')
    # A sentence ends at a full stop before a space, quote or tag (not one inside org.pocol.node), at ? ؟ or a newline,
    # and a text at a quote or a tag.
    END = re.compile(r'\.(?=[\s"<]|$)|[?؟\n"<>]')

    def test_the_retired_network_is_named_only_in_its_true_notices(self):
        seen = 0
        for rel, text in served_text():
            if not rel.endswith(('.html', '.js')):
                self.assertIsNone(RETIRED.search(text), rel)
                continue
            for m in RETIRED.finditer(text):
                seen += 1
                start = max((e.end() for e in self.END.finditer(text, 0, m.start())), default=0)
                end = self.END.search(text, m.end())
                sentence = text[start:end.start() + 1 if end else len(text)]
                self.assertTrue(any(k in sentence for k in self.KEPT), f'{rel}: {sentence.strip()[:200]!r}')
        self.assertGreater(seen, 10)

    def test_the_sitemap_lists_exactly_the_served_pages(self):
        pages = {rel for rel in tracked() if rel.endswith('.html') and rel not in ('404.html', 'faucet.html')}
        sitemap = (MAIN / 'sitemap.xml').read_text(encoding='utf-8')
        locs = re.findall(r'<loc>([^<]+)</loc>', sitemap)
        self.assertEqual(len(locs), len(set(locs)), 'a page is listed twice')
        listed = set()
        for loc in locs:
            self.assertTrue(loc.startswith('https://pocol.net/'), loc)
            rel = loc[len('https://pocol.net/'):] or 'index.html'
            self.assertTrue((MAIN / rel).is_file(), loc)
            listed.add(rel)
        self.assertEqual(listed, pages)
        self.assertIn('Sitemap: https://pocol.net/sitemap.xml', (MAIN / 'robots.txt').read_text(encoding='utf-8'))

    def test_the_document_root_holds_no_deployment_notes(self):
        notes = [rel for rel in tracked() if re.search(r'(?i)(^|/)(readme|deploy|install|notes?)[^/]*$', rel)]
        self.assertEqual(notes, [], 'deployment instructions ship beside the ZIP (tools/package_site.py), not in the '
                                    'document root, where https://pocol.net/ would serve them')

if __name__ == '__main__':
    unittest.main()
