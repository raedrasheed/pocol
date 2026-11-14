"""Sites W5: the S1 snapshot helper (deploy/websites/tools/site_snapshot.py). Local only: git and temp directories."""
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'tools'))
import site_snapshot as S  # noqa: E402

ROOT = HERE.parents[2]
VECTORS = json.loads((ROOT / 'prototypes/pocol-extension/tests/data/publish-input-vectors.json').read_text())


def git(repo, *args):
    subprocess.run(['git', '-C', str(repo), *args], check=True, capture_output=True)


class Rules(unittest.TestCase):
    def test_hard_deny_follows_the_shared_vectors(self):
        for path, reason in VECTORS['deny']:
            self.assertEqual(S.deny_reason(path), reason, path)
        self.assertEqual(S.DENY_REASONS, ('dotfile', 'config', 'secret-key', 'database', 'backup', 'server-api', 'checksums'))

    def test_include_list_selection_follows_the_shared_vectors(self):
        v = VECTORS['include']
        s = S.select(v['paths'], S.parse_include_list(v['text']))
        self.assertEqual(s['kept'], v['kept'])
        self.assertEqual([list(d) for d in s['denied']], v['denied'])
        self.assertEqual(s['excluded'], v['excluded'])
        self.assertEqual(s['missing'], v['missing'])
        with self.assertRaises(ValueError):
            S.parse_include_list('# nothing\n')
        with self.assertRaises(ValueError):
            S.parse_include_list('a\n/a\n')


class TrackedSnapshot(unittest.TestCase):
    def make_repo(self, tmp):
        repo = pathlib.Path(tmp) / 'repo'
        site = repo / 'site'
        (site / 'api').mkdir(parents=True)
        (site / 'assets').mkdir()
        for rel, data in {'index.html': '<p>', 'assets/app.js': '1', '.htaccess': 'deny', 'api/rpc.php': '<?php',
                          'SHA256SUMS.txt': hashlib.sha256(b'<p>').hexdigest() + '  index.html\n', 'extra.txt': 'e'}.items():
            (site / rel).write_text(data)
        git(repo, 'init', '-q')
        git(repo, 'add', '-A')
        git(repo, '-c', 'user.name=t', '-c', 'user.email=t@example.invalid', 'commit', '-qm', 'site')
        (site / 'index.html').write_text('<p>uncommitted edit')   # never published: the snapshot is the commit
        (site / 'untracked.html').write_text('x')
        return repo

    def test_only_the_listed_tracked_files_are_written_and_every_refusal_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = self.make_repo(tmp)
            inc = pathlib.Path(tmp) / 'inc.txt'
            inc.write_text('index.html\nassets/app.js\n')
            out = pathlib.Path(tmp) / 'out'
            r = S.build(out, 'site', inc, repo=repo)
            self.assertEqual(sorted(str(p.relative_to(out)) for p in out.rglob('*') if p.is_file()), ['assets/app.js', 'index.html'])
            self.assertEqual((out / 'index.html').read_text(), '<p>', 'the committed bytes, not the working tree')
            self.assertEqual({d['path']: d['reason'] for d in r['refused']}, {'/.htaccess': 'dotfile', '/api/rpc.php': 'server-api', '/SHA256SUMS.txt': 'checksums'})
            self.assertEqual(r['excluded'], ['/extra.txt'])
            with self.assertRaisesRegex(ValueError, 'not empty'):
                S.build(out, 'site', inc, repo=repo)

    def test_the_committed_bytes_are_published_even_with_core_autocrlf_true(self):
        # Git for Windows sets core.autocrlf=true: `git archive` would then write CRLF text that matches neither the
        # commit nor SHA256SUMS.txt. The snapshot reads the blobs, so the bytes are the committed LF ones.
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / 'repo'
            (repo / 'site').mkdir(parents=True)
            files = {'index.html': b'<p>\na\nb\n', 'site.webmanifest': b'{\n"name": "x"\n}\n', 'app.js': b'let a = 1;\r\nlet b = 2;\n'}
            for rel, data in files.items():
                (repo / 'site' / rel).write_bytes(data)
            (repo / 'site' / 'SHA256SUMS.txt').write_bytes(''.join(f'{hashlib.sha256(d).hexdigest()}  {r}\n' for r, d in sorted(files.items())).encode())
            git(repo, 'init', '-q')
            git(repo, 'config', 'core.autocrlf', 'false')
            git(repo, 'add', '-A')
            git(repo, '-c', 'user.name=t', '-c', 'user.email=t@example.invalid', 'commit', '-qm', 'site')
            git(repo, 'config', 'core.autocrlf', 'true')
            git(repo, 'config', 'core.eol', 'crlf')
            archived = subprocess.run(['git', '-C', str(repo), 'archive', '--format=tar', 'HEAD', '--', 'site/index.html'], check=True, capture_output=True).stdout
            self.assertIn(b'<p>\r\na', archived, 'this git converts on archive with these settings (the case the snapshot avoids)')
            inc = pathlib.Path(tmp) / 'inc.txt'
            inc.write_text('index.html\nsite.webmanifest\napp.js\n')
            out = pathlib.Path(tmp) / 'out'
            r = S.build(out, 'site', inc, repo=repo)
            for rel, data in files.items():
                self.assertEqual((out / rel).read_bytes(), data, rel)
            self.assertEqual({f['path']: f['sha256'] for f in r['files']}, {'/' + k: hashlib.sha256(v).hexdigest() for k, v in files.items()})

    def test_a_file_that_differs_from_the_snapshot_sha256sums_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / 'repo'
            (repo / 'site').mkdir(parents=True)
            (repo / 'site' / 'index.html').write_bytes(b'<p>')
            (repo / 'site' / 'SHA256SUMS.txt').write_text('0' * 64 + '  index.html\n')
            git(repo, 'init', '-q')
            git(repo, 'add', '-A')
            git(repo, '-c', 'user.name=t', '-c', 'user.email=t@example.invalid', 'commit', '-qm', 'site')
            inc = pathlib.Path(tmp) / 'inc.txt'
            inc.write_text('index.html\n')
            with self.assertRaisesRegex(ValueError, 'SHA256SUMS'):
                S.build(pathlib.Path(tmp) / 'out', 'site', inc, repo=repo)
            self.assertFalse((pathlib.Path(tmp) / 'out').exists(), 'nothing is written')
            (repo / 'site' / 'SHA256SUMS.txt').write_text('x\n')
            git(repo, '-c', 'user.name=t', '-c', 'user.email=t@example.invalid', 'commit', '-qam', 'malformed')
            with self.assertRaisesRegex(ValueError, 'malformed'):
                S.build(pathlib.Path(tmp) / 'out2', 'site', inc, repo=repo)

    def test_a_tracked_symlink_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / 'repo'
            (repo / 'site').mkdir(parents=True)
            (repo / 'site' / 'index.html').write_bytes(b'<p>')
            (repo / 'site' / 'link.html').symlink_to('index.html')
            git(repo, 'init', '-q')
            git(repo, 'add', '-A')
            git(repo, '-c', 'user.name=t', '-c', 'user.email=t@example.invalid', 'commit', '-qm', 'site')
            with self.assertRaisesRegex(ValueError, 'not a regular file'):
                S.tracked_snapshot(repo, 'HEAD', 'site')

    def test_a_listed_file_that_is_untracked_or_refused_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = self.make_repo(tmp)
            for listed in ('untracked.html', 'api/rpc.php', '.htaccess'):
                inc = pathlib.Path(tmp) / 'inc.txt'
                inc.write_text(f'index.html\n{listed}\n')
                with self.assertRaisesRegex(ValueError, 'include list'):
                    S.build(pathlib.Path(tmp) / ('o-' + listed.replace('/', '_')), 'site', inc, repo=repo)

    def test_the_explorer_include_list_gives_twelve_files_of_the_tracked_explorer(self):
        with tempfile.TemporaryDirectory() as tmp:
            r = S.build(pathlib.Path(tmp) / 'explorer', 'deploy/websites/explorer.pocol.net', ROOT / 'deploy/websites/tools/explorer-s1-include.txt')
            paths = [f['path'] for f in r['files']]
            self.assertEqual(len(paths), 12)
            self.assertIn('/favicon.ico', paths)
            self.assertIn('/site.webmanifest', paths)
            refused = {d['path'] for d in r['refused']}
            self.assertTrue({'/.htaccess', '/SHA256SUMS.txt', '/api/rpc.php', '/api/lib.php', '/api/.htaccess'} <= refused)
            self.assertFalse(any(p.startswith('/api/') or p.endswith('.php') for p in paths))
            self.assertLess(r['bytes'], 4 * 1024 * 1024)


if __name__ == '__main__':
    unittest.main()
