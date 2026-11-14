"""Sites W5: the S1 snapshot helper (deploy/websites/tools/site_snapshot.py). Local only: git and temp directories."""
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
                          'SHA256SUMS.txt': 'x', 'extra.txt': 'e'}.items():
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
