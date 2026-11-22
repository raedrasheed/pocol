#!/usr/bin/env python3
"""pocol.net's download links (2026-11-22). No PoCol Mainnet native package is released before
the signed release (W16), so:
- every link to pocol.net's downloads/ names a file the snapshot holds, one that site-identity.json keeps out of Git
  (its SHA256SUMS.txt line is kept), or a retired package that site_identity.py's LEGACY_NATIVE exempts. The removed
  invitation (downloads/network.pocol.json, the retired network's, removed 2026-11-13) and PoCol-Apps-README.txt (never
  in the snapshot) are linked nowhere;
- the home page's "MAINNET EDITION" buttons (wallet-downloads.js) offer no package, so they show "No release available";
- a retired package is only ever linked with a label that says it is the retired network's, in English and Arabic."""
import html, json, re, sys, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAIN = ROOT / 'deploy' / 'websites' / 'pocol.net'
sys.path.insert(0, str(ROOT / 'deploy' / 'websites' / 'tools'))
import site_identity as si  # noqa: E402

# An absolute https://pocol.net/ link, a root- or dot-relative one right after a quote, '=' or '(' ("/downloads/…",
# "./downloads/…"), or a bare relative "downloads/…".
LINK = re.compile(r'(?:https://pocol\.net/|(?<=["\'=(])\.?/|(?<![A-Za-z0-9_./:-]))downloads/([A-Za-z0-9][A-Za-z0-9._-]*)')
RETIRED_LABEL = ('Retired network', 'للشبكة السابقة')

def served_text():
    for path in sorted(MAIN.rglob('*')):
        if path.is_file() and path.suffix in ('.html', '.js'):
            yield path, path.read_text(encoding='utf-8')

def guide():
    text = (MAIN / 'guide-content.js').read_text(encoding='utf-8')
    return json.loads(text[len('window.POCOL_GUIDE = '):].rstrip().rstrip(';'))

def anchors(text):
    """(href, label text) of each <a> in an HTML text."""
    for m in re.finditer(r'<a\b([^>]*)>(.*?)</a>', text, re.S):
        href = re.search(r'\bhref="([^"]*)"', m.group(1))
        if href:
            yield html.unescape(href.group(1)), html.unescape(re.sub(r'<[^>]+>', '', m.group(2)))

class DownloadLinks(unittest.TestCase):
    def test_the_link_pattern_sees_every_link_form(self):
        sample = ('<a href="https://pocol.net/downloads/a.zip?v=1">x</a> <a href="downloads/b.zip">x</a> '
                  '<a href="/downloads/c.zip">x</a> <a href=\'./downloads/d.zip\'>x</a> url(/downloads/e.png) '
                  'https://example.org/downloads/f.zip other/downloads/g.zip')
        self.assertEqual(LINK.findall(sample), ['a.zip', 'b.zip', 'c.zip', 'd.zip', 'e.png'])

    def test_every_download_link_is_served_kept_out_or_an_exempt_retired_package(self):
        declared = json.loads((ROOT / si.DECLARED).read_text(encoding='utf-8'))
        kept_out = {name.split('/', 1)[1] for name in declared['keptOut']['pocol.net']}
        tracked = {p.name for p in (MAIN / 'downloads').iterdir() if p.is_file()}
        allowed = tracked | kept_out | set(si.LEGACY_NATIVE)
        found = set()
        for path, text in served_text():
            for name in LINK.findall(text):
                found.add(name)
                self.assertIn(name, allowed, f'{path.name} links downloads/{name}, which pocol.net does not serve')
        self.assertTrue(found)
        for gone in ('network.pocol.json', 'PoCol-Apps-README.txt', 'PoCol-Mainnet-0.4.0-Linux.zip'):
            self.assertNotIn(gone, found)

    def test_the_mainnet_edition_buttons_offer_no_package_before_the_signed_release(self):
        text = (MAIN / 'wallet-downloads.js').read_text(encoding='utf-8')
        prefix = 'window.POCOL_DOWNLOADS = '
        self.assertTrue(text.startswith(prefix))
        downloads = json.loads(text[len(prefix):].rstrip().rstrip(';'))
        self.assertEqual(set(downloads), {'windows', 'linux', 'android', 'ios', 'macos'})
        for platform, value in downloads.items():
            url = value if isinstance(value, str) else value.get('url', '')
            # Until W16 no native package is offered; the retired 0.4.0 packages never are.
            self.assertEqual(url, '', f'the {platform} MAINNET EDITION button offers {url}')

    def test_a_retired_package_is_linked_only_as_the_retired_networks(self):
        seen = 0
        for path, text in served_text():
            if path.suffix != '.html':
                continue
            for href, label in anchors(text):
                if 'PoCol-Mainnet-0.4.' in href:
                    seen += 1
                    self.assertTrue(any(word in label for word in RETIRED_LABEL), f'{path.name}: {label!r} -> {href}')
        for topic in guide()['topics']:
            for section in topic.get('sections', []):
                for link in section.get('links') or []:
                    if 'PoCol-Mainnet-0.4.' in link['url']:
                        seen += 1
                        self.assertIn(RETIRED_LABEL[0], link['label']['en'], link)
                        self.assertIn(RETIRED_LABEL[1], link['label']['ar'], link)
        self.assertGreater(seen, 0)

    def test_no_text_tells_users_to_download_a_retired_package_for_pocol_mainnet(self):
        for path, text in served_text():
            self.assertNotIn('download the complete PoCol-Mainnet-0.4.0', text, path.name)
            self.assertNotIn('These applications connect to PoCol Mainnet', text, path.name)
            self.assertNotIn('نزّل حزمة PoCol-Mainnet-0.4.0', text, path.name)

if __name__ == '__main__':
    unittest.main()
