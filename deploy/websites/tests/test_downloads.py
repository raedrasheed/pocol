#!/usr/bin/env python3
"""pocol.net's download links (2026-11-22). No PoCol Mainnet native package is released before
the signed release (W16), so:
- every link to pocol.net's downloads/ names a file the snapshot holds, one that site-identity.json keeps out of Git
  (its SHA256SUMS.txt line is kept), or a retired package that site_identity.py's LEGACY_NATIVE exempts. The removed
  invitation (downloads/network.pocol.json, the retired network's, removed 2026-11-13) and PoCol-Apps-README.txt (never
  in the snapshot) are linked nowhere;
- the home page's "MAINNET EDITION" buttons (wallet-downloads.js) offer no package, so they show "No release available";
- a retired package is only ever linked with a label that says it is the retired network's, in English and Arabic;
- no site (pocol.net, ide.pocol.net, studio.pocol.net) names an extension package before its signed release: the
  retired network's 0.4.1 build (SITE-01) is not offered, and after the release only the release's own package,
  recorded as a kept-out download, may be linked;
- the extension ID and the release key fingerprint that pocol.net publishes (SITE-03) are the ones the repository's
  manifest key and allowed_signers give, by the documented derivations."""
import base64, hashlib, html, json, re, shutil, subprocess, sys, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAIN = ROOT / 'deploy' / 'websites' / 'pocol.net'
IDE = ROOT / 'deploy' / 'websites' / 'ide.pocol.net'
STUDIO = ROOT / 'prototypes' / 'pocol-studio' / 'dist'
sys.path.insert(0, str(ROOT / 'deploy' / 'websites' / 'tools'))
sys.path.insert(0, str(ROOT / 'deploy' / 'release'))
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
        # The snapshot may hold no download file at all (git keeps no empty folder).
        downloads = MAIN / 'downloads'
        tracked = {p.name for p in downloads.iterdir() if p.is_file()} if downloads.is_dir() else set()
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
        # The five platforms and the release record (tools/release_page.py fills them from a verified signed release).
        self.assertEqual(set(downloads), {'windows', 'linux', 'android', 'ios', 'macos', 'release'})
        self.assertEqual(downloads.pop('release'), '', 'no signed release is recorded before W16')
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

    def test_the_verify_text_does_not_deny_the_retired_package_the_same_section_links(self):
        # 2026-11-22 review: "this page offers no package" was false while the same section of the home page links the
        # retired network's 0.4.0 Windows package. The text says no PoCol Mainnet package, and names the retired one.
        page = (MAIN / 'index.html').read_text(encoding='utf-8')
        retired = [href for href, label in anchors(page) if 'downloads/' in href and any(w in label for w in RETIRED_LABEL)]
        about = (MAIN / 'about-locales.js').read_text(encoding='utf-8')
        start = about.index('const extra = ') + len('const extra = ')
        extra, _ = json.JSONDecoder().raw_decode(about, start)
        en, ar = extra['en']['appsVerifyBody'], extra['ar']['appsVerifyBody']
        self.assertNotIn('offers no package', en)
        self.assertNotIn('أي حزمة قبل', ar)
        self.assertIn('this page offers no PoCol Mainnet package', en)
        self.assertIn('أي حزمة لشبكة PoCol Mainnet', ar)
        if retired:
            self.assertIn('the only package it links is the retired network’s Windows package (0.4.0)', en)
            self.assertIn('حزمة Windows للشبكة السابقة (0.4.0)', ar)
            self.assertTrue(all('PoCol-Mainnet-0.4.0-Windows.zip' in href for href in retired), retired)
        self.assertIn(html.escape(en, quote=False), page)

    def test_no_text_tells_users_to_download_a_retired_package_for_pocol_mainnet(self):
        for path, text in served_text():
            self.assertNotIn('download the complete PoCol-Mainnet-0.4.0', text, path.name)
            self.assertNotIn('These applications connect to PoCol Mainnet', text, path.name)
            self.assertNotIn('نزّل حزمة PoCol-Mainnet-0.4.0', text, path.name)

EXTENSION_PACKAGE = re.compile(r'PoCol-Studio-(?:IDE-Viewer-[0-9]+\.[0-9]+\.[0-9]+|Chrome)\.zip')

def release_extension_name():
    """The extension package of the signed release (assemble_release.py), or None while no release can be assembled
    (before the identity switch the extension stays the retired network's 0.4.1 build)."""
    import assemble_release as release
    try:
        return next(n for n, kind in release.allowed_names(ROOT).items() if kind == 'extension')
    except release.Refused:
        return None

class ExtensionPackage(unittest.TestCase):
    """SITE-01 and SITE-03."""
    def served(self):
        for site in (MAIN, IDE, STUDIO):
            for path in sorted(site.rglob('*')):
                if path.is_file() and path.suffix in ('.html', '.js', '.txt') and path.name != 'SHA256SUMS.txt':
                    yield site, path, path.read_text(encoding='utf-8')

    def test_the_package_pattern_sees_every_extension_package_name(self):
        sample = 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip /downloads/PoCol-Studio-Chrome.zip PoCol-Studio-IDE-Viewer-12.0.3.zip'
        self.assertEqual(EXTENSION_PACKAGE.findall(sample),
                         ['PoCol-Studio-IDE-Viewer-0.4.1.zip', 'PoCol-Studio-Chrome.zip', 'PoCol-Studio-IDE-Viewer-12.0.3.zip'])

    def test_no_site_offers_an_extension_package_before_its_signed_release(self):
        declared = json.loads((ROOT / si.DECLARED).read_text(encoding='utf-8'))['keptOut']
        allowed = release_extension_name()
        for site, path, text in self.served():
            kept = {n.split('/', 1)[1] for n in declared.get(site.name, [])}
            for name in sorted(set(EXTENSION_PACKAGE.findall(text))):
                self.assertEqual(name, allowed, f'{path.relative_to(ROOT)} names the extension package {name}; only the '
                                 f'signed release package ({allowed or "none before the identity switch"}) may be named')
                self.assertIn(name, kept, f'{path.relative_to(ROOT)}: {name} is not a recorded download of {site.name}')
        # A kept-out extension package is the release's (its line in SHA256SUMS.txt is the signed file's digest).
        for key, names in declared.items():
            for name in names:
                if EXTENSION_PACKAGE.fullmatch(name.split('/', 1)[1]):
                    self.assertEqual(name.split('/', 1)[1], allowed, f'{key} keeps out {name}')

    def test_the_published_extension_id_and_key_fingerprint_are_the_repositorys(self):
        # The id: the first 128 bits of sha256 of the manifest key's DER, as build.mjs extensionId and
        # package-release.py --extension-id derive it.
        lines = (ROOT / 'prototypes/pocol-extension/release/manifest-key.pub').read_text(encoding='utf-8').splitlines()
        key = [line for line in lines if line and not line.startswith('#')]
        self.assertEqual(len(key), 1)
        digest = hashlib.sha256(base64.b64decode(key[0])).hexdigest()[:32]
        ext_id = ''.join(chr(ord('a') + int(c, 16)) for c in digest)
        self.assertEqual(ext_id, 'gggcoflmedlhlkcdhebmoepmggdnafmp')
        # The fingerprint: SHA256 of the key blob, unpadded base64, as ssh-keygen -lf prints it.
        signers = [l for l in (ROOT / 'deploy/release/allowed_signers').read_text(encoding='utf-8').splitlines()
                   if l.strip() and not l.lstrip().startswith('#')]
        self.assertEqual(len(signers), 1)
        blob = base64.b64decode(signers[0].split()[3])
        fingerprint = 'SHA256:' + base64.b64encode(hashlib.sha256(blob).digest()).decode().rstrip('=')
        if shutil.which('ssh-keygen'):
            # The documented command (deploy/release/README.md, user procedure step 1).
            out = subprocess.run(['ssh-keygen', '-lf', '-'], input=' '.join(signers[0].split()[2:4]) + '\n',
                                 capture_output=True, text=True, timeout=30)
            self.assertEqual(out.returncode, 0, out.stderr)
            self.assertEqual(out.stdout.split()[1], fingerprint)
        self.assertIn(f'256 {fingerprint} release@pocol.net (ED25519)', (ROOT / 'deploy/release/README.md').read_text(encoding='utf-8'))
        topic = next(t for t in guide()['topics'] if t['id'] == 'extension')
        section = next(s for s in topic['sections'] if s['id'] == 'verify-release')
        for lang in ('en', 'ar'):
            text = ' '.join(p[lang] for p in section['paragraphs'])
            self.assertIn(ext_id, text, lang)
            self.assertIn(fingerprint, text, lang)
            steps = ' '.join(b[lang] for b in section['bullets'])
            for command in ("grep -v '^ *#' allowed_signers | cut -d' ' -f3,4 | ssh-keygen -lf -",
                            'ssh-keygen -Y verify -f allowed_signers -I release@pocol.net -n pocol-release -s SHA256SUMS.sig < SHA256SUMS',
                            "awk -v f=FILE '$2 == f' SHA256SUMS | sha256sum --check", 'verify_release.py DIR'):
                self.assertIn(command, steps, lang)
        page = html.unescape((MAIN / 'chrome-extension.html').read_text(encoding='utf-8'))
        self.assertIn(f'Extension ID: {ext_id}', page)
        self.assertIn(fingerprint, page)
        # The home page's "Verify a download" names the same fingerprint, in both languages.
        about = (MAIN / 'about-locales.js').read_text(encoding='utf-8')
        start = about.index('const extra = ') + len('const extra = ')
        extra, _ = json.JSONDecoder().raw_decode(about, start)
        for lang in ('en', 'ar'):
            self.assertIn(fingerprint, extra[lang]['appsVerifyBody'], lang)
        self.assertIn(html.escape(extra['en']['appsVerifyBody'], quote=False), (MAIN / 'index.html').read_text(encoding='utf-8'))
        # The page says what is true today: no signed release is offered yet.
        self.assertIn('until then pocol.net offers no extension package', section['paragraphs'][0]['en'])

if __name__ == '__main__':
    unittest.main()
