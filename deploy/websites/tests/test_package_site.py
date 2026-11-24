#!/usr/bin/env python3
"""deploy/websites/tools/package_site.py: the cPanel deployment package of pocol.net (the owner's request of
2026-11-23). The ZIP holds exactly the committed snapshot but .htaccess (shipped beside it for the manual merge), each
file byte for byte as SHA256SUMS.txt lists it; two builds of one commit are byte-identical; entries are sorted, 0644
files and 0755 directories, dated the commit time, with no extra field; untracked and ignored files never ship; a dirty
tree, a stale SHA256SUMS.txt, an unlisted file or a file that a document root package must never hold is refused; the
deployment guides are filled from the commit and the declared identity, and name what the host keeps and deletes.
Scratch repositories are built under TMPDIR; nothing is uploaded and no network is contacted."""
import hashlib, io, json, os, re, shutil, stat, subprocess, sys, tempfile, time, unittest, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent / 'tools'))
sys.path.insert(0, str(HERE))
import package_site as ps  # noqa: E402
import host_files as hf  # noqa: E402
import test_downloads  # noqa: E402  (the stale-file sentence of the owner's upload guides; its tests are not re-run here)

SITE = 'deploy/websites/pocol.net'
GIT_ENV = {'GIT_AUTHOR_NAME': 'test', 'GIT_AUTHOR_EMAIL': 'test@example.invalid', 'GIT_COMMITTER_NAME': 'test',
           'GIT_COMMITTER_EMAIL': 'test@example.invalid', 'GIT_CONFIG_NOSYSTEM': '1'}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def git(repo, *args, when='2026-11-23T10:00:00Z'):
    env = dict(os.environ, **GIT_ENV, GIT_AUTHOR_DATE=when, GIT_COMMITTER_DATE=when, HOME=str(repo))
    return subprocess.run(['git', '-c', 'core.autocrlf=false', '-C', str(repo), *args], check=True, capture_output=True,
                          env=env, timeout=60).stdout.decode()

def sums_of(files):
    return ''.join(f'{sha(d)}  {p}\n' for p, d in sorted(files.items()) if p != 'SHA256SUMS.txt').encode()

class ScratchRepo:
    """A repository with a small pocol.net snapshot, its declared identity and the real deployment guides."""
    FILES = {'index.html': b'<!doctype html><p>PoCol</p>\n', 'style.css': b'p{color:#000}\n',
             'api/network.php': b'<?php echo 1;\n', 'assets/a.png': b'\x89PNG\r\n\x1a\n\x00',
             '.htaccess': b'Options -Indexes\nErrorDocument 404 /404.html\n', '404.html': b'<p>404</p>\n'}

    def __init__(self, tmp):
        self.root = Path(tmp) / 'repo'
        self.site = self.root / SITE
        self.site.mkdir(parents=True)
        git(self.root, 'init', '-q')
        (self.root / '.gitignore').write_text('**/api/config.php\n', encoding='utf-8')
        for rel in ('deploy/websites/site-identity.json',):
            target = self.root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            doc = json.loads((ROOT / rel).read_text(encoding='utf-8'))
            target.write_text(json.dumps({k: doc[k] for k in ('chainId', 'genesisHash', 'name', 'gatewayHost', 'keptOut')}),
                              encoding='utf-8')
        shutil.copytree(ROOT / ps.TEMPLATES, self.root / ps.TEMPLATES)
        self.write(dict(self.FILES))

    def write(self, files, message='snapshot', when='2026-11-23T10:00:00Z'):
        for old in sorted(self.site.rglob('*'), reverse=True):
            if old.is_file():
                old.unlink()
        files = dict(files)
        files['SHA256SUMS.txt'] = sums_of(files)
        for rel, data in files.items():
            (self.site / rel).parent.mkdir(parents=True, exist_ok=True)
            (self.site / rel).write_bytes(data)
        git(self.root, 'add', '-A')
        git(self.root, 'commit', '-q', '--allow-empty', '-m', message, when=when)
        return files

class Package(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = ScratchRepo(self.tmp.name)

    def build(self, name='out', **kw):
        out = Path(self.tmp.name) / name
        return out, ps.build(out, repo=self.repo.root, check=False, **kw)

    def test_the_zip_is_the_snapshot_but_htaccess_and_two_builds_are_identical(self):
        out1, manifest = self.build('one')
        out2, _ = self.build('two')
        names = sorted(p.name for p in out1.iterdir())
        self.assertEqual(names, sorted(p.name for p in out2.iterdir()))
        for name in names:
            self.assertEqual((out1 / name).read_bytes(), (out2 / name).read_bytes(), name)
        short = git(self.repo.root, '-c', 'core.abbrev=12', 'log', '-1', '--format=%h').strip()
        zip_name = f'PoCol-pocol.net-site-2026-11-23-{short}.zip'
        self.assertEqual(names, sorted([zip_name, zip_name + '.sha256', 'pocol.net.htaccess', 'DEPLOY-pocol.net-AR.md',
                                        'DEPLOY-pocol.net-EN.md', 'MANIFEST.json', 'PACKAGE-SHA256SUMS.txt']))
        data = (out1 / zip_name).read_bytes()
        self.assertEqual((out1 / (zip_name + '.sha256')).read_text(), f'{sha(data)}  {zip_name}\n')
        self.assertEqual((out1 / 'pocol.net.htaccess').read_bytes(), self.repo.FILES['.htaccess'])
        sums = dict((p, d) for d, p in (line.split('  ', 1) for line in (self.repo.site / 'SHA256SUMS.txt').read_text().splitlines()))
        epoch = int(git(self.repo.root, 'log', '-1', '--format=%ct'))
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            infos = z.infolist()
            files = [i for i in infos if not i.is_dir()]
            self.assertEqual(sorted(i.filename for i in files), sorted(set(self.repo.FILES) - {'.htaccess'} | {'SHA256SUMS.txt'}))
            self.assertEqual([i.filename for i in infos], sorted(i.filename for i in infos), 'entries in path order')
            self.assertEqual([i.filename for i in infos if i.is_dir()], ['api/', 'assets/'])
            for info in infos:
                self.assertEqual(info.date_time, time.gmtime(epoch)[:6], info.filename)
                self.assertEqual(info.extra, b'', info.filename)
                self.assertEqual(info.create_system, 3)
                mode = info.external_attr >> 16
                self.assertEqual(mode, (stat.S_IFDIR | 0o755) if info.is_dir() else (stat.S_IFREG | 0o644), info.filename)
                if not info.is_dir():
                    body = z.read(info)
                    if info.filename != 'SHA256SUMS.txt':
                        self.assertEqual(sha(body), sums[info.filename], info.filename)
                    self.assertEqual(body, (self.repo.site / info.filename).read_bytes())
            self.assertEqual(z.comment, b'')
        self.assertEqual(manifest['zip'], {'name': zip_name, 'bytes': len(data), 'sha256': sha(data), 'files': len(files)})
        self.assertEqual(manifest['beside'], {'.htaccess': 'pocol.net.htaccess'})
        self.assertEqual(manifest['commit'], git(self.repo.root, 'rev-parse', 'HEAD').strip())
        listed = (out1 / 'PACKAGE-SHA256SUMS.txt').read_text().splitlines()
        self.assertEqual([line.split('  ')[1] for line in listed], sorted(set(names) - {'PACKAGE-SHA256SUMS.txt'}))
        for line in listed:
            digest, name = line.split('  ')
            self.assertEqual(sha((out1 / name).read_bytes()), digest, name)

    def test_untracked_and_ignored_files_never_ship_and_a_dirty_tree_is_refused(self):
        (self.repo.site / 'api' / 'config.php').write_bytes(b'<?php $secret = 1;\n')  # ignored (.gitignore)
        (self.repo.site / 'stray.html').write_bytes(b'untracked')
        out, manifest = self.build()
        self.assertNotIn('api/config.php', [f['path'] for f in manifest['files']])
        self.assertNotIn('stray.html', [f['path'] for f in manifest['files']])
        with self.assertRaisesRegex(ps.Refused, 'stray.html'):
            ps.require_clean(self.repo.root)
        (self.repo.site / 'stray.html').unlink()
        ps.require_clean(self.repo.root)  # an ignored file does not make the tree dirty
        (self.repo.site / 'style.css').write_bytes(b'p{}\n')
        with self.assertRaisesRegex(ps.Refused, 'style.css'):
            ps.require_clean(self.repo.root)
        with self.assertRaisesRegex(ps.Refused, 'not the committed tree'):
            ps.build(Path(self.tmp.name) / 'dirty', repo=self.repo.root)

    def test_a_stale_or_incomplete_sha256sums_is_refused(self):
        files = self.repo.write(dict(self.repo.FILES))
        (self.repo.site / 'SHA256SUMS.txt').write_bytes(files['SHA256SUMS.txt'].replace(sha(self.repo.FILES['style.css']).encode(), b'0' * 64))
        git(self.repo.root, 'commit', '-qam', 'stale')
        with self.assertRaisesRegex(ps.Refused, 'style.css is not the committed file'):
            self.build()
        self.repo.write(dict(self.repo.FILES))
        (self.repo.site / 'extra.js').write_bytes(b'1')
        git(self.repo.root, 'add', '-A')
        git(self.repo.root, 'commit', '-qm', 'unlisted')
        with self.assertRaisesRegex(ps.Refused, 'does not list extra.js'):
            self.build('out2')
        self.repo.write(dict(self.repo.FILES))
        with open(self.repo.site / 'SHA256SUMS.txt', 'ab') as f:
            f.write(f'{"1" * 64}  downloads/gone.zip\n'.encode())
        git(self.repo.root, 'commit', '-qam', 'a line for no file')
        with self.assertRaisesRegex(ps.Refused, 'neither in the snapshot nor declared'):
            self.build('out3')

    def test_files_a_document_root_package_must_never_hold_are_refused(self):
        for bad in ('README-AR.txt', 'DEPLOY-README-AR.txt', 'api/config.php', '.user.ini', 'php.ini', 'api/error_log',
                    'tests/x.py', 'tools/site_identity.py', 'node_modules/a/index.js', 'downloads/old.zip', 'a.tar.gz',
                    'key.pem', 'site.key', '.git/config', '.env', 'assets/.DS_Store', 'db.sqlite', 'index.html.bak'):
            with self.subTest(bad=bad):
                self.assertRegex(bad, ps.DENY)
        for good in ('index.html', '.htaccess', 'api/network.php', 'assets/pocol-icon.png', 'site.webmanifest',
                     'SHA256SUMS.txt', 'robots.txt', 'sitemap.xml', 'favicon.ico', 'about-locales.js', 'privacy.css'):
            self.assertNotRegex(good, ps.DENY, good)
        self.repo.write(dict(self.repo.FILES, **{'README-AR.txt': b'notes'}))
        with self.assertRaisesRegex(ps.Refused, 'must never hold.*README-AR.txt'):
            self.build()

    def test_the_output_folder_must_be_new_or_empty(self):
        out = Path(self.tmp.name) / 'used'
        out.mkdir()
        (out / 'x').write_bytes(b'')
        with self.assertRaisesRegex(ps.Refused, 'not a new or empty directory'):
            ps.build(out, repo=self.repo.root, check=False)
        self.assertEqual([p.name for p in out.iterdir()], ['x'])

    def test_a_later_commit_gets_a_later_zip_time_and_name(self):
        _, first = self.build('a')
        self.repo.write(dict(self.repo.FILES, **{'style.css': b'p{color:#111}\n'}), when='2026-11-24T08:30:01Z')
        _, second = self.build('b')
        self.assertNotEqual(first['zip']['name'], second['zip']['name'])
        self.assertIn('-2026-11-24-', second['zip']['name'])
        with zipfile.ZipFile(Path(self.tmp.name) / 'b' / second['zip']['name']) as z:
            self.assertEqual(z.getinfo('style.css').date_time, (2026, 11, 24, 8, 30, 0))
        self.assertGreater(second['commitTime'], first['commitTime'])

    def test_the_guides_are_filled_and_an_unknown_placeholder_is_refused(self):
        out, manifest = self.build()
        doc = json.loads((ROOT / 'deploy/websites/site-identity.json').read_text(encoding='utf-8'))
        for lang in ('AR', 'EN'):
            text = (out / f'DEPLOY-pocol.net-{lang}.md').read_text(encoding='utf-8')
            self.assertNotIn('{{', text, lang)
            for value in (manifest['zip']['name'], manifest['zip']['sha256'], manifest['commit'], str(doc['chainId']),
                          doc['genesisHash'], 'pocol.net.htaccess', f"pocol.net-cpanel-{manifest['commit'][:12]}"):
                self.assertIn(value, text, lang)
        with self.assertRaisesRegex(ps.Refused, 'unknown placeholders'):
            ps.render('{{NOT_A_VALUE}}', {})

class Guides(unittest.TestCase):
    """The committed guides (deploy/websites/package) say what the host keeps, merges and deletes, and how to restore."""
    def text(self, lang):
        return (ROOT / ps.TEMPLATES / f'DEPLOY-pocol.net-{lang}.md').read_text(encoding='utf-8')

    def test_both_guides_have_the_same_steps_and_placeholders(self):
        ar, en = self.text('AR'), self.text('EN')
        self.assertEqual(sorted(set(ps.PLACEHOLDER.findall(ar))), sorted(set(ps.PLACEHOLDER.findall(en))))
        self.assertEqual(re.findall(r'^## (\d+)\.', ar, re.M), re.findall(r'^## (\d+)\.', en, re.M))
        commands = lambda t: [line.strip() for line in t.splitlines() if line.startswith('    ') and not line.strip().startswith('#')]
        self.assertEqual(commands(ar), commands(en), 'the two guides run the same commands')
        for text in (ar, en):
            for needed in ('sha256sum -c PACKAGE-SHA256SUMS.txt', 'sha256sum -c {{ZIP}}.sha256', 'tar -czf ~/pocol-backups/',
                           'unzip -o -q ~/{{ZIP}} -d "$DOCROOT"', "grep -v '  \\.htaccess$' SHA256SUMS.txt | sha256sum -c",
                           'host_files.py --site pocol.net --listing pocol.net-files.txt --host-copy .htaccess=host.htaccess',
                           '--keep-dir ide', '# php -- BEGIN cPanel-generated handler, do not edit', 'MultiPHP Manager',
                           'Show Hidden Files', 'api/config.php', '.user.ini', 'php.ini', 'error_log', '.well-known/',
                           'cgi-bin/', 'tar -xzf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz',
                           'sha256sum -c --quiet ~/pocol-backups/pocol.net-before-$STAMP-files.txt', '"chainId":{{CHAIN_ID}}',
                           '"genesisHash":"{{GENESIS}}"', 'PHP {{PHP_RECOMMENDED}}', '`curl`', '{{GATEWAY}}'):
                self.assertIn(needed, text)
        # The stale-file sentence of the earlier pocol.net/README-AR.txt (test_downloads.STALE_FILE_NOTE) moved here.
        self.assertIn(test_downloads.DownloadLinks.STALE_FILE_NOTE, ' '.join(ar.split()))

    def test_no_backup_archive_is_written_inside_the_document_root(self):
        # An archive in the Document Root is served to anyone and holds api/config.php (2026-11-23 review): the File
        # Manager backup names its full path outside it, in the Compress dialog itself, and every backup path the
        # guides name is under the home folder's pocol-backups/ (never a bare name, which File Manager would write
        # into the open folder, the Document Root).
        for lang, replace in (('EN', 'replace the\nwhole path with'), ('AR', 'استبدل المسار كله بـ')):
            text = self.text(lang)
            self.assertIn(replace, text, lang)
            for m in re.finditer(r'pocol\.net-(?:backup|docroot|before)-', text):
                self.assertEqual(text[m.start() - len('/pocol-backups/'):m.start()], '/pocol-backups/',
                                 f'{lang}: {text[m.start() - 40:m.end() + 20]!r}')
            self.assertGreaterEqual(len(re.findall(r'/pocol-backups/pocol\.net-backup-YYYYMMDD\.zip', text)), 2, lang)

    def commands(self, text, start, stop):
        part = text[text.index(start):]
        part = part[:part.index(stop)]
        return [line[4:] for line in part.splitlines() if line.startswith('    ') and not line.startswith('     ')]

    def test_the_terminal_backup_and_rollback_restore_the_previous_site(self):
        # Steps 2 and 10 of the guide, run as written in a scratch home folder: the rollback puts back every earlier
        # file byte for byte, deletes the files the package added and the folders it created, keeps the earlier site's
        # empty folders (cgi-bin/), and prints RESTORED. The guides run the same commands (the test above).
        en = self.text('EN')
        backup = self.commands(en, '## 2.', '**With File Manager:**')
        rollback = self.commands(en, '## 10.', '**With File Manager:**')
        self.assertIn('tar -czf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz .', backup)
        self.assertTrue(any('rmdir' in c for c in rollback) and any('rm -f' in c for c in rollback), rollback)
        old = {'index.html': b'old site\n', 'downloads/PoCol-Mainnet-0.4.0-Windows.zip': b'PK old', '.htaccess': b'# cPanel\n',
               'api/config.php': b'<?php $k = 1;\n', '.well-known/acme-challenge/t': b't', 'name with space.txt': b's'}
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / 'home'
            docroot = home / 'public_html'
            for rel, data in old.items():
                (docroot / rel).parent.mkdir(parents=True, exist_ok=True)
                (docroot / rel).write_bytes(data)
            (docroot / 'cgi-bin').mkdir()
            dirs_before = sorted(str(p.relative_to(docroot)) for p in docroot.rglob('*') if p.is_dir())
            # The package's extraction (step 4): a changed page, new files, a new folder and a nested new folder.
            deploy = ["printf 'new site\\n' > index.html", 'mkdir -p assets/icons api', "printf 'png' > assets/icons/a.png",
                      "printf 'php' > api/network.php", "printf 'sums' > SHA256SUMS.txt"]
            script = '\n'.join(['set -e', *backup, *deploy, *rollback])
            env = dict(os.environ, HOME=str(home), DOCROOT=str(docroot), LC_ALL='C')
            run = subprocess.run(['bash', '-c', script], capture_output=True, text=True, env=env, cwd=tmp, timeout=120)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(run.stdout.strip().splitlines()[-1], 'RESTORED', run.stdout)
            after = {str(p.relative_to(docroot)): p.read_bytes() for p in docroot.rglob('*') if p.is_file()}
            self.assertEqual(after, old)
            self.assertEqual(sorted(str(p.relative_to(docroot)) for p in docroot.rglob('*') if p.is_dir()), dirs_before)
        for lang in ('AR', 'EN'):
            text = self.text(lang)
            section = text[text.index('## 10.'):]
            # A nested site's files and folders never enter the rollback's lists.
            self.assertIn('`find . -path ./ide -prune -o -type f -print`', section, lang)
            self.assertIn('`find . -mindepth 1 -path ./ide -prune -o -type d -print`', section, lang)

    def test_the_files_the_guides_name_for_deletion_are_delete_rows(self):
        # Each downloads/ or top-level file the guides expect host_files.py to mark DELETE is not in the snapshot, and
        # the tool says DELETE for it; the retired network's 0.4.0 Windows package among them (2026-11-23).
        en = self.text('EN')
        section = en[en.index('Expected `DELETE` files'):en.index('The tool\'s list is the one to follow')]
        named = re.findall(r'`((?:downloads/)?[A-Za-z0-9][A-Za-z0-9._-]+\.(?:txt|json|zip))`', section)
        self.assertIn('downloads/PoCol-Mainnet-0.4.0-Windows.zip', named)
        self.assertIn('README-AR.txt', named)
        want, _ = hf.expected('pocol.net')
        listing = dict(want, **{p: '0' * 64 for p in named})
        rows = {path: kind for kind, path, _ in hf.compare('pocol.net', listing)}
        for path in named:
            self.assertNotIn(path, want)
            self.assertEqual(rows[path], 'DELETE', path)
        ar = self.text('AR')
        for path in named:
            self.assertIn(path, ar)

class RepositoryPackage(unittest.TestCase):
    """The repository's own pocol.net snapshot at HEAD."""
    def test_the_committed_snapshot_packages_and_equals_git_ls_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest = ps.build(Path(tmp) / 'out', repo=ROOT, check=False)
            tracked = subprocess.run(['git', '-C', str(ROOT), 'ls-tree', '-r', '--name-only', 'HEAD', '--', SITE],
                                     capture_output=True, text=True, check=True, timeout=60).stdout.split()
            tracked = sorted(p[len(SITE) + 1:] for p in tracked)
            self.assertEqual(sorted(f['path'] for f in manifest['files']), sorted(set(tracked) - {'.htaccess'}))
            sums = {}
            blob = subprocess.run(['git', '-C', str(ROOT), 'show', f'HEAD:{SITE}/SHA256SUMS.txt'], capture_output=True,
                                  check=True, timeout=60).stdout.decode()
            for line in blob.splitlines():
                digest, path = line.split('  ', 1)
                sums[path] = digest
            for f in manifest['files']:
                if f['path'] != 'SHA256SUMS.txt':
                    self.assertEqual(f['sha256'], sums[f['path']], f['path'])
            self.assertEqual(manifest['releaseFilesNotInGit'], [{'path': p, 'sha256': sums[p]} for p in sorted(json.loads((ROOT / 'deploy/websites/site-identity.json').read_text(encoding='utf-8'))['keptOut']['pocol.net'])])
            doc = json.loads((ROOT / 'deploy/websites/site-identity.json').read_text(encoding='utf-8'))
            self.assertEqual((manifest['chainId'], manifest['genesisHash']), (doc['chainId'], doc['genesisHash']))
            self.assertRegex(manifest['zip']['name'], r'^PoCol-pocol\.net-site-\d{4}-\d{2}-\d{2}-[0-9a-f]{12}\.zip$')
            with zipfile.ZipFile(Path(tmp) / 'out' / manifest['zip']['name']) as z:
                names = z.namelist()
            self.assertFalse([n for n in names if ps.DENY.search(n) or n.startswith('.')], names)

    def test_a_clean_checkout_passes_every_refusal(self):
        status = subprocess.run(['git', '-C', str(ROOT), 'status', '--porcelain=v1', '--untracked-files=no'],
                                capture_output=True, text=True, timeout=60)
        untracked = subprocess.run(['git', '-C', str(ROOT), 'status', '--porcelain=v1', '--untracked-files=all', '--',
                                    'deploy/websites'], capture_output=True, text=True, timeout=60)
        if status.stdout.strip() or '??' in untracked.stdout:
            self.skipTest('the checkout has uncommitted changes; CI builds from a clean checkout')
        with tempfile.TemporaryDirectory() as tmp:
            manifest = ps.build(Path(tmp) / 'out', repo=ROOT)
            self.assertIn('changed: 0 ', manifest['identityCheck'])

if __name__ == '__main__':
    unittest.main()
