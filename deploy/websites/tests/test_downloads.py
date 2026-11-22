#!/usr/bin/env python3
"""The download links and listed downloads of the websites (pocol.net, ide.pocol.net, studio.pocol.net's
prototypes/pocol-studio/dist, explorer.pocol.net) while no signed PoCol Mainnet release is published (W16):
- no site file names the content version (the first 12 hex digits of the SHA-256, the `?v=` value site_identity.py
  writes) of an invitation of a retired network, nor its full SHA-256;
- every downloads/ link names a file the site's snapshot holds, one that site-identity.json keeps out of Git (its
  SHA256SUMS.txt line is kept), or, on pocol.net, a retired package that site_identity.py's LEGACY_NATIVE exempts and
  that is labelled as the retired network's, in English and Arabic. The removed invitation (downloads/network.pocol.json,
  the retired network's, removed 2026-11-13), PoCol-Apps-README.txt (never in the snapshot) and the retired Linux
  package are linked nowhere;
- the home page's "MAINNET EDITION" buttons (wallet-downloads.js) offer no package, so they show "No release available";
- no site names, links or lists an extension package (owner policy kept on the integration branch, 2026-11-22: no
  extension zip link or listed digest until the signed release). Step 7 listed and linked an unsigned 0.5.0 zip: a
  digest of an unsigned build, which goes stale with any change to the extension and was not a signed release. CI
  (full-nonce-upgrade.yml) packages the extension twice and requires the same bytes, and refuses a listed extension
  line that is not that build's."""
import hashlib, html, json, re, sys, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
WEB = ROOT / 'deploy' / 'websites'
MAIN = WEB / 'pocol.net'
IDE = WEB / 'ide.pocol.net'
EXPLORER = WEB / 'explorer.pocol.net'
STUDIO = ROOT / 'prototypes' / 'pocol-studio' / 'dist'
sys.path.insert(0, str(WEB / 'tools'))
import site_identity as si  # noqa: E402

# The invitations (network.pocol.json) of the networks in deploy/mainnet/retired-networks.json. That file records chain
# ID and genesis hash only, and the retired invitation is no longer in the tree, so its SHA-256 is recorded here: it is
# the SHA-256 of `git show 882eeaee^:deploy/mainnet/h15000/network.pocol.json` (the invitation of chain 1599846715,
# genesis 0xbe137bfd...e509ca, deleted from the tree by 882eeaee), whose content version f4ed13730195 the pages linked
# as https://pocol.net/downloads/network.pocol.json?v=f4ed13730195.
RETIRED_INVITATIONS = {1599846715: 'f4ed1373019563106a4ba0af9f063c874c667a5be50a2b42f32f16e2dcbeaf3b'}

SITES = {'pocol.net': MAIN, 'ide.pocol.net': IDE, 'studio.pocol.net': STUDIO, 'explorer.pocol.net': EXPLORER}
# An absolute https:// link to a PoCol site (group 1, its host), a root- or dot-relative one right after a quote, a
# backtick, '=' or '(' ("/downloads/…", "./downloads/…"), or a bare relative "downloads/…" (group 2, the file name).
LINK = re.compile(r'(?:https://((?:ide\.|studio\.|explorer\.)?pocol\.net)/|(?<=["\'`=(])\.?/|(?<![A-Za-z0-9_./:-]))'
                  r'downloads/([A-Za-z0-9][A-Za-z0-9._-]*)')
RETIRED_LABEL = ('Retired network', 'للشبكة السابقة')
EXTENSION_PACKAGE = re.compile(r'PoCol-Studio-(?:IDE-Viewer-[0-9]+\.[0-9]+\.[0-9]+|Chrome)\.zip')
TEXT = ('.html', '.js', '.txt', '.json', '.css', '.webmanifest', '.svg', '.md', '.php')

def site_files(site):
    for path in sorted(site.rglob('*')):
        if path.is_file() and path.suffix in TEXT:
            yield path, path.read_text(encoding='utf-8', errors='replace')

def served_text(site=MAIN):
    """The site's pages, scripts and texts (not SHA256SUMS.txt, which lists files rather than linking them)."""
    for path, text in site_files(site):
        if path.suffix in ('.html', '.js', '.txt') and path.name != si.SUMS:
            yield path, text

def guide():
    text = (MAIN / 'guide-content.js').read_text(encoding='utf-8')
    return json.loads(text[len('window.POCOL_GUIDE = '):].rstrip().rstrip(';'))

def anchors(text):
    """(href, label text) of each <a> in an HTML text."""
    for m in re.finditer(r'<a\b([^>]*)>(.*?)</a>', text, re.S):
        href = re.search(r'\bhref="([^"]*)"', m.group(1))
        if href:
            yield html.unescape(href.group(1)), html.unescape(re.sub(r'<[^>]+>', '', m.group(2)))

def declared():
    return json.loads((ROOT / si.DECLARED).read_text(encoding='utf-8'))

class RetiredInvitation(unittest.TestCase):
    def test_the_recorded_invitations_are_of_retired_networks(self):
        retired = {n['chainId'] for n in json.loads((ROOT / si.RETIRED).read_text(encoding='utf-8'))['retired']}
        self.assertTrue(set(RETIRED_INVITATIONS) <= retired, RETIRED_INVITATIONS)
        self.assertNotIn(declared()['chainId'], RETIRED_INVITATIONS)

    def test_no_site_file_names_a_retired_invitation_or_its_content_version(self):
        for chain, digest in RETIRED_INVITATIONS.items():
            for name, site in SITES.items():
                for path, text in site_files(site):
                    self.assertFalse(digest[:12] in text, f'{path.relative_to(ROOT)} names the content version of chain '
                                     f"{chain}'s invitation (retired)")
            self.assertFalse(digest[:12] in (ROOT / si.DECLARED).read_text(encoding='utf-8'), si.DECLARED)

class DownloadLinks(unittest.TestCase):
    def test_the_link_pattern_sees_every_link_form(self):
        sample = ('<a href="https://pocol.net/downloads/a.zip?v=1">x</a> <a href="downloads/b.zip">x</a> '
                  '<a href="/downloads/c.zip">x</a> <a href=\'./downloads/d.zip\'>x</a> url(/downloads/e.png) '
                  'href:`/downloads/f.zip` https://ide.pocol.net/downloads/g.zip\n- downloads/h.zip '
                  'https://example.org/downloads/x.zip other/downloads/y.zip')
        self.assertEqual(LINK.findall(sample), [('pocol.net', 'a.zip'), ('', 'b.zip'), ('', 'c.zip'), ('', 'd.zip'),
                                                ('', 'e.png'), ('', 'f.zip'), ('ide.pocol.net', 'g.zip'), ('', 'h.zip')])

    def test_every_download_link_is_served_kept_out_or_an_exempt_retired_package(self):
        kept = declared()['keptOut']
        allowed = {}
        for name, site in SITES.items():
            downloads = site / 'downloads'  # git keeps no empty folder
            tracked = {p.name for p in downloads.iterdir() if p.is_file()} if downloads.is_dir() else set()
            allowed[name] = tracked | {n.split('/', 1)[1] for n in kept.get(name, [])}
        allowed['pocol.net'] |= set(si.LEGACY_NATIVE)
        found = set()
        for name, site in SITES.items():
            for path, text in served_text(site):
                for host, file in LINK.findall(text):
                    target = host or name
                    found.add((target, file))
                    self.assertIn(file, allowed[target], f'{path.relative_to(ROOT)} links {target}/downloads/{file}, '
                                  'which is not in that snapshot, not kept out (site-identity.json) and not an exempt '
                                  'retired package')
        self.assertTrue(found)
        for gone in ('network.pocol.json', 'PoCol-Apps-README.txt', 'PoCol-Mainnet-0.4.0-Linux.zip'):
            self.assertNotIn(('pocol.net', gone), found)

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
        # The labelled links beside "Coming from the retired network?" stay until W16 (deploy/websites/README.md).
        self.assertGreater(seen, 0)

    def test_no_text_tells_users_to_download_a_retired_package_for_pocol_mainnet(self):
        for path, text in served_text():
            self.assertFalse('download the complete PoCol-Mainnet-0.4.0' in text, f'{path.name}: ' + 'download the complete PoCol-Mainnet-0.4.0')
            self.assertFalse('These applications connect to PoCol Mainnet' in text, f'{path.name}: ' + 'These applications connect to PoCol Mainnet')
            self.assertFalse('نزّل حزمة PoCol-Mainnet-0.4.0' in text, f'{path.name}: ' + 'نزّل حزمة PoCol-Mainnet-0.4.0')

class ExtensionPackage(unittest.TestCase):
    def test_the_package_pattern_sees_every_extension_package_name(self):
        sample = 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip /downloads/PoCol-Studio-Chrome.zip PoCol-Studio-IDE-Viewer-12.0.3.zip.sha256'
        self.assertEqual(EXTENSION_PACKAGE.findall(sample),
                         ['PoCol-Studio-IDE-Viewer-0.4.1.zip', 'PoCol-Studio-Chrome.zip', 'PoCol-Studio-IDE-Viewer-12.0.3.zip'])

    def test_no_site_names_or_links_an_extension_package_before_the_signed_release(self):
        for name, site in SITES.items():
            for path, text in served_text(site):
                self.assertEqual(sorted(set(EXTENSION_PACKAGE.findall(text))), [],
                                 f'{path.relative_to(ROOT)} names an extension package before the signed release')

    def test_no_site_lists_an_extension_package_digest(self):
        for name, site in SITES.items():
            sums = site / si.SUMS
            if sums.exists():
                listed = [line for line in sums.read_text(encoding='utf-8').splitlines() if EXTENSION_PACKAGE.search(line)]
                self.assertEqual(listed, [], f'{sums.relative_to(ROOT)} lists an extension package')
            downloads = site / 'downloads'
            if downloads.is_dir():
                self.assertEqual(sorted(p.name for p in downloads.iterdir() if EXTENSION_PACKAGE.search(p.name)), [],
                                 f'{downloads.relative_to(ROOT)} tracks an extension package checksum')
        for key, names in declared()['keptOut'].items():
            self.assertEqual([n for n in names if EXTENSION_PACKAGE.search(n)], [], f'{si.DECLARED} keeps out for {key}')

if __name__ == '__main__':
    unittest.main()
