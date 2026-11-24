#!/usr/bin/env python3
"""The websites' download links and listed downloads (pocol.net, ide.pocol.net, studio.pocol.net's
prototypes/pocol-studio/dist, explorer.pocol.net; 2026-11-22). No PoCol Mainnet native package is released before the
signed release (W16), so:
- no site file names the content version (the first 12 hex digits of the SHA-256, the `?v=` value site_identity.py
  writes) of an invitation of a retired network;
- every downloads/ link of a site names a file that site's snapshot holds, one that site-identity.json keeps out of Git
  (its SHA256SUMS.txt line is kept), or, on pocol.net, a retired package that site_identity.py's LEGACY_NATIVE exempts
  (none since the owner's request of 2026-11-23). The removed invitation (downloads/network.pocol.json, the retired
  network's, removed 2026-11-13), PoCol-Apps-README.txt (never in the snapshot) and the retired Linux and Windows
  packages are linked nowhere;
- the home page's "MAINNET EDITION" buttons (wallet-downloads.js) offer no package, so they show "No release available";
- no page links or names a retired package (PoCol-Mainnet-0.4.x or older): the owner's request of 2026-11-23 removed the
  last one, the retired network's 0.4.0 Windows package that was linked, labelled as such, beside "Coming from the
  retired network?";
- no site names an extension package before its signed release: the retired network's 0.4.1 build (SITE-01) and the
  unsigned 0.5.0 zip step 7 listed are not offered, and after the release only the release's own package, recorded as
  a kept-out download, may be linked. Until tools/release_page.py records a signed release, no site lists an extension
  package digest (SHA256SUMS.txt, a tracked .sha256, keptOut), and no text tells a user to download an extension
  package now or says one ships with a site copy;
- the extension ID and the release key fingerprint that pocol.net publishes (SITE-03) are the ones the repository's
  manifest key and allowed_signers give, by the documented derivations."""
import base64, hashlib, html, json, re, shutil, subprocess, sys, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
WEB = ROOT / 'deploy' / 'websites'
MAIN = WEB / 'pocol.net'
IDE = WEB / 'ide.pocol.net'
EXPLORER = WEB / 'explorer.pocol.net'
STUDIO = ROOT / 'prototypes' / 'pocol-studio' / 'dist'
sys.path.insert(0, str(WEB / 'tools'))
sys.path.insert(0, str(ROOT / 'deploy' / 'release'))
import site_identity as si  # noqa: E402

# The invitations (network.pocol.json) of the networks in deploy/mainnet/retired-networks.json. That file records chain
# ID and genesis hash only, and the retired invitation is no longer in the tree, so its SHA-256 is recorded here: it is
# the SHA-256 of `git show 882eeaee^:deploy/mainnet/h15000/network.pocol.json` (the invitation of chain 1599846715,
# deleted from the tree by 882eeaee), whose content version f4ed13730195 the pages linked as
# https://pocol.net/downloads/network.pocol.json?v=f4ed13730195 (withdrawn 2026-11-22).
RETIRED_INVITATIONS = {1599846715: 'f4ed1373019563106a4ba0af9f063c874c667a5be50a2b42f32f16e2dcbeaf3b'}

SITES = {'pocol.net': MAIN, 'ide.pocol.net': IDE, 'studio.pocol.net': STUDIO, 'explorer.pocol.net': EXPLORER}
# An absolute https:// link to a PoCol site (group 1, its host), a root- or dot-relative one right after a quote, a
# backtick, '=' or '(' ("/downloads/…", "./downloads/…"), or a bare relative "downloads/…" (group 2, the file name).
LINK = re.compile(r'(?:https://((?:ide\.|studio\.|explorer\.)?pocol\.net)/|(?<=["\'`=(])\.?/|(?<![A-Za-z0-9_./:-]))'
                  r'downloads/([A-Za-z0-9][A-Za-z0-9._-]*)')
RETIRED_LABEL = ('Retired network', 'للشبكة السابقة')
# A native package of the retired network (0.4.x and older; PoCol Mainnet's first release is 0.5.0).
RETIRED_PACKAGE = re.compile(r'PoCol-Mainnet-0\.[0-4]\.[0-9]+-[A-Za-z0-9_.-]+')
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

def declared():
    return json.loads((ROOT / si.DECLARED).read_text(encoding='utf-8'))

def release_record():
    """wallet-downloads.js's "release" record: "" until tools/release_page.py records a verified signed release."""
    text = (MAIN / 'wallet-downloads.js').read_text(encoding='utf-8')
    return json.loads(text[len('window.POCOL_DOWNLOADS = '):].rstrip().rstrip(';')).get('release', '')

def guide():
    text = (MAIN / 'guide-content.js').read_text(encoding='utf-8')
    return json.loads(text[len('window.POCOL_GUIDE = '):].rstrip().rstrip(';'))

def anchors(text):
    """(href, label text) of each <a> in an HTML text."""
    for m in re.finditer(r'<a\b([^>]*)>(.*?)</a>', text, re.S):
        href = re.search(r'\bhref="([^"]*)"', m.group(1))
        if href:
            yield html.unescape(href.group(1)), html.unescape(re.sub(r'<[^>]+>', '', m.group(2)))

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

    # The owner's upload guides served from a document root (ide.pocol.net/DEPLOY-README-AR.txt, SITE-02; pocol.net's
    # README-AR.txt left the document root on 2026-11-23 for deploy/websites/package/DEPLOY-pocol.net-AR.md, which
    # keeps the sentence and is checked by test_package_site.py) and the sentence
    # in them that names the removed invitation as an old file that stays on the host until it is deleted.
    HOST_GUIDES = ('README-AR.txt', 'DEPLOY-README-AR.txt')
    STALE_FILE_NOTE = 'يبقى على الخادم كل ملف لم يعد في هذه النسخة ويظل متاحًا للزوار، مثل ملف الشبكة القديم downloads/network.pocol.json'

    def test_every_download_link_is_served_kept_out_or_an_exempt_retired_package(self):
        kept = declared()['keptOut']
        allowed = {}
        for name, site in SITES.items():
            downloads = site / 'downloads'  # the snapshot may hold no download file at all (git keeps no empty folder)
            tracked = {p.name for p in downloads.iterdir() if p.is_file()} if downloads.is_dir() else set()
            allowed[name] = tracked | {n.split('/', 1)[1] for n in kept.get(name, [])}
        allowed['pocol.net'] |= set(si.LEGACY_NATIVE)
        found, scanned = set(), 0
        for name, site in SITES.items():
            for path, text in served_text(site):
                scanned += 1
                if path.name in self.HOST_GUIDES:
                    # The upload step names the removed invitation as a stale file to delete from the host; it links
                    # nothing. Only that sentence is set aside, and it must say so.
                    self.assertLessEqual(text.count('downloads/network.pocol.json'), text.count(self.STALE_FILE_NOTE),
                                         path.relative_to(ROOT))
                    text = text.replace(self.STALE_FILE_NOTE, '')
                for host, file in LINK.findall(text):
                    target = host or name
                    found.add((target, file))
                    self.assertIn(file, allowed[target], f'{path.relative_to(ROOT)} links {target}/downloads/{file}, '
                                  'which is not in that snapshot, not kept out (site-identity.json) and not an exempt '
                                  'retired package')
        # Since the owner's request of 2026-11-23 removed the retired Windows package's links, no site links a downloads/
        # file before the signed release (until then this checked `found` was not empty); the pattern itself is checked
        # by test_the_link_pattern_sees_every_link_form, and every served text of the four sites was read.
        self.assertGreater(scanned, 40)
        for gone in ('network.pocol.json', 'PoCol-Apps-README.txt', 'PoCol-Mainnet-0.4.0-Linux.zip',
                     'PoCol-Mainnet-0.4.0-Windows.zip'):
            self.assertNotIn(('pocol.net', gone), found)

    def test_the_mainnet_edition_buttons_match_signed_release(self):
        text = (MAIN / 'wallet-downloads.js').read_text(encoding='utf-8')
        prefix = 'window.POCOL_DOWNLOADS = '
        self.assertTrue(text.startswith(prefix))
        downloads = json.loads(text[len(prefix):].rstrip().rstrip(';'))
        self.assertEqual(set(downloads), {'windows', 'linux', 'android', 'ios', 'macos', 'release'})
        version = si.release.native_version(ROOT)
        record = downloads['release']
        self.assertEqual(record['version'], version)
        self.assertEqual(record['sums'], 'https://pocol.net/downloads/SHA256SUMS')
        self.assertEqual(record['sumsSig'], 'https://pocol.net/downloads/SHA256SUMS.sig')
        for platform in ('ios', 'macos'):
            self.assertEqual(downloads[platform], '')
        sums = dict(line.split('  ', 1)[::-1] for line in (MAIN / si.SUMS).read_text(encoding='utf-8').splitlines())
        kept = set(declared()['keptOut']['pocol.net'])
        for name in ('SHA256SUMS', 'SHA256SUMS.sig'):
            self.assertIn('downloads/' + name, kept)
            self.assertIn('downloads/' + name, sums)
        packages = {'windows': f'PoCol-Mainnet-{version}-windows-x86_64.zip', 'linux': f'PoCol-Mainnet-{version}-linux-x86_64.tar.gz', 'android': f'PoCol-Mainnet-{version}-arm64.apk'}
        for platform, name in packages.items():
            item = downloads[platform]
            url = 'https://pocol.net/downloads/' + name
            self.assertEqual(item['url'], url)
            self.assertEqual(item['sig'], url + '.sig')
            self.assertRegex(item['sha256'], r'^[0-9a-f]{64}$')
            self.assertEqual(sums['downloads/' + name], item['sha256'])
            self.assertIn('downloads/' + name, kept)
            self.assertIn('downloads/' + name + '.sig', kept)
            self.assertIn('downloads/' + name + '.sig', sums)

    def test_no_page_links_a_retired_package(self):
        # Until 2026-11-23 the home page, the tools page and the guide linked the retired network's 0.4.0 Windows
        # package, labelled as such; this test required that label and at least one such link. The owner's request of
        # 2026-11-23 (item 4: no Testnet and no 0.4.x release in the deployment content) removed the links, their
        # labels and the LEGACY_NATIVE exemption, so no served file may link or name a retired package any more, with
        # or without a label.
        self.assertEqual(si.LEGACY_NATIVE, frozenset())
        for path, text in served_text():
            for href, label in anchors(text) if path.suffix == '.html' else ():
                self.assertNotIn('PoCol-Mainnet-0.', href, f'{path.name}: {label!r} -> {href}')
            self.assertIsNone(RETIRED_PACKAGE.search(text), f'{path.name} names a retired package')
            for word in RETIRED_LABEL:
                self.assertNotIn(word + '’s Windows package', text, path.name)
            self.assertNotIn('حزمة Windows للشبكة السابقة (', text, path.name)
        for topic in guide()['topics']:
            for section in topic.get('sections', []):
                for link in section.get('links') or []:
                    self.assertNotIn('PoCol-Mainnet-0.', link['url'], link)
        self.assertEqual(RETIRED_PACKAGE.findall('downloads/PoCol-Mainnet-0.4.0-Windows.zip PoCol-Mainnet-0.3.2-Linux.zip '
                                                 'PoCol-Mainnet-0.5.0-arm64.apk'),
                         ['PoCol-Mainnet-0.4.0-Windows.zip', 'PoCol-Mainnet-0.3.2-Linux.zip'])

    def test_the_verify_text_says_what_the_page_offers(self):
        # 2026-11-22 review: "this page offers no package" was false while the same section of the home page linked the
        # retired network's 0.4.0 Windows package, so the text said no PoCol Mainnet package and named the retired one.
        # Since the owner's request of 2026-11-23 removed that link, the text says that the page links no package of the
        # retired network either, and no package link is left beside it.
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
        self.assertEqual(retired, [])
        self.assertEqual([href for href, _ in anchors(page) if 'downloads/' in href], [])
        self.assertIn('this page offers no PoCol Mainnet package and links no package of the retired network', en)
        self.assertIn('ولا تربط أي حزمة للشبكة السابقة', ar)
        self.assertNotIn('0.4.0', en + ar)
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
        for site in (MAIN, IDE, STUDIO, EXPLORER):
            for path in sorted(site.rglob('*')):
                if path.is_file() and path.suffix in ('.html', '.js', '.txt') and path.name != 'SHA256SUMS.txt':
                    yield site, path, path.read_text(encoding='utf-8')

    def test_the_package_pattern_sees_every_extension_package_name(self):
        sample = 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip /downloads/PoCol-Studio-Chrome.zip PoCol-Studio-IDE-Viewer-12.0.3.zip.sha256'
        self.assertEqual(EXTENSION_PACKAGE.findall(sample),
                         ['PoCol-Studio-IDE-Viewer-0.4.1.zip', 'PoCol-Studio-Chrome.zip', 'PoCol-Studio-IDE-Viewer-12.0.3.zip'])

    # Texts that tell a user to download, or say a copy ships with, an extension package that no site offers before the
    # signed release (the home page's Studio and IDE help, the IDE's deploy guide), in English and Arabic.
    DOWNLOAD_NOW = ('Download and extract the ZIP', 'extract the ZIP', 'The download includes',
                    'نزّل ZIP', 'فك ضغط ZIP', 'يتضمن التنزيل', 'المرفقة مع هذه النسخة')

    def test_no_text_offers_an_extension_package_before_the_signed_release(self):
        for name, site in SITES.items():
            for path, text in served_text(site):
                for phrase in self.DOWNLOAD_NOW:
                    self.assertNotIn(phrase, text, f'{path.relative_to(ROOT)}: {phrase}')
        # The home page's install step says when a package is offered, as the guide's install bullet does.
        locales = (MAIN / 'about-locales.js').read_text(encoding='utf-8')
        extra = json.JSONDecoder().raw_decode(locales, locales.index('const extra = ') + len('const extra = '))[0]
        self.assertIn('until then pocol.net offers no extension package', extra['en']['studioStep1Body'])
        self.assertIn('لا يعرض pocol.net أي حزمة للإضافة قبل ذلك', extra['ar']['studioStep1Body'])
        self.assertIn(f'data-i18n="studioStep1Body">{html.escape(extra["en"]["studioStep1Body"], quote=False)}</p>',
                      (MAIN / 'index.html').read_text(encoding='utf-8'))
        readme = (IDE / 'DEPLOY-README-AR.txt').read_text(encoding='utf-8')
        self.assertIn('المسار الأساسي هو إضافة PoCol التي تُنشر مع الإصدار الموقّع، ولا ترافق هذه النسخة.', readme)

    def test_no_site_lists_an_extension_package_digest_before_the_signed_release(self):
        # Step 7 listed the digest of an unsigned 0.5.0 build (withdrawn 2026-11-22). Only after tools/release_page.py
        # records a verified signed release may pocol.net list the release's own package, as a kept-out line.
        allowed = {release_extension_name()} - {None} if release_record() else set()
        for name, site in SITES.items():
            sums = site / si.SUMS
            if sums.exists():
                listed = {m.group(0) for line in sums.read_text(encoding='utf-8').splitlines()
                          for m in [EXTENSION_PACKAGE.search(line)] if m}
                self.assertLessEqual(listed, allowed if name == 'pocol.net' else set(),
                                     f'{sums.relative_to(ROOT)} lists an extension package')
            downloads = site / 'downloads'
            if downloads.is_dir():
                self.assertEqual(sorted(p.name for p in downloads.iterdir() if EXTENSION_PACKAGE.search(p.name)), [],
                                 f'{downloads.relative_to(ROOT)} tracks an extension package checksum')
        for key, names in declared()['keptOut'].items():
            kept = {m.group(0) for n in names for m in [EXTENSION_PACKAGE.search(n)] if m}
            self.assertLessEqual(kept, allowed if key == 'pocol.net' else set(), f'{si.DECLARED} keeps out for {key}')

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

    def test_the_sites_readme_describes_no_extension_download_that_no_site_offers(self):
        # deploy/websites/README.md is not served; its "Site truth" paragraph once said "The 0.5.0 download links and
        # names ... change with the signed release" after the 0.5.0 links were withdrawn. A version it calls a download
        # link must be one a served file still names.
        offered = {m.group(1) for _, _, text in self.served()
                   for m in re.finditer(r'PoCol-Studio-IDE-Viewer-([0-9]+\.[0-9]+\.[0-9]+)\.zip', text)}
        readme = ' '.join((WEB / 'README.md').read_text(encoding='utf-8').split())
        described = set(re.findall(r'\b([0-9]+\.[0-9]+\.[0-9]+) download links?\b', readme))
        self.assertLessEqual(described, offered, 'deploy/websites/README.md describes extension download links no site has')

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
