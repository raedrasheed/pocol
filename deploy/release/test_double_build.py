#!/usr/bin/env python3
"""double_build.py without cargo: the recipe's environment, the comparison and its diff summary. The builds themselves
run in CI (five-device-build.yml, native-reproducible) and locally (`python3 deploy/release/double_build.py`)."""
import contextlib
import fcntl
import hashlib
import io
import os
import runpy
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
DB = runpy.run_path(str(HERE / 'double_build.py'), run_name='double_build')


def git_repo(path, when=1795029413):
    path.mkdir(parents=True)
    git = ['git', '-c', 'user.name=t', '-c', 'user.email=t@invalid', '-c', 'commit.gpgsign=false', '-c', 'init.defaultBranch=t']
    subprocess.run(git + ['init', '-q'], cwd=path, check=True)
    (path / 'f').write_text('x\n', encoding='utf-8')
    subprocess.run(git + ['add', 'f'], cwd=path, check=True)
    stamp = f'@{when} +0000'
    subprocess.run(git + ['commit', '-q', '-m', 'm'], cwd=path, check=True,
                   env={**os.environ, 'GIT_AUTHOR_DATE': stamp, 'GIT_COMMITTER_DATE': stamp})
    return path


def leg(root, name, members):
    """A leg's output directory as `build` writes it, for an archive of `members` ({name: bytes})."""
    d = root / name
    (d / 'dist').mkdir(parents=True)
    archive = d / 'dist' / 'PoCol-Mainnet-0.0.0-linux-x86_64.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        for member, data in sorted(members.items()):
            info = tarfile.TarInfo(member)
            info.size, info.mtime, info.mode = len(data), 1, 0o755
            tar.addfile(info, io.BytesIO(data))
    DB['write_package_lists'](archive, d)
    # package.py's Linux output: the archive and its SHA256SUMS line.
    (d / 'dist' / 'SHA256SUMS').write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n',
                                           encoding='utf-8', newline='\n')
    (d / 'binaries.sha256').write_text(''.join(f'{hashlib.sha256(data).hexdigest()}  {n}\n' for n, data in sorted(members.items())),
                                       encoding='utf-8')
    (d / 'env.txt').write_text(f'leg {name}\n', encoding='utf-8')
    return d


class DoubleBuildTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='pocol-double-build-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_the_recipe_environment(self):
        checkout = git_repo(self.root / 'checkout')
        base = {'PATH': os.environ.get('PATH', ''), 'HOME': str(self.root), 'CARGO_HOME': str(self.root / 'cargo'),
                'RUSTUP_HOME': str(self.root / 'rustup'), 'RUSTFLAGS': '-C target-cpu=native', 'CFLAGS': '-march=native',
                'CARGO_TARGET_DIR': '/elsewhere', 'CARGO_PROFILE_RELEASE_DEBUG': '0', 'SOURCE_DATE_EPOCH': '1',
                'POCOL_REPIN_KIND': 'x', 'CC': 'clang'}
        env = DB['reproducible_env'](checkout, base)
        # Nothing of the caller's build flags survives; the recipe's own are set.
        for key in ('RUSTFLAGS', 'CARGO_PROFILE_RELEASE_DEBUG', 'POCOL_REPIN_KIND', 'CC'):
            self.assertNotIn(key, env)
        self.assertEqual(env['SOURCE_DATE_EPOCH'], '1795029413', "the commit's committer time")
        self.assertEqual((env['POCOL_RELEASE'], env['CARGO_INCREMENTAL']), ('1', '0'))
        self.assertEqual(env['CARGO_TARGET_DIR'], str(DB['BUILD_LINK'] / 'prototypes/pocold/target'))
        flags = env['CARGO_ENCODED_RUSTFLAGS'].split('\x1f')
        self.assertEqual(flags[0].split('=', 1)[0], '--remap-path-prefix')
        maps = dict(f.split('=', 1)[1].rsplit('=', 1) for f in flags)
        self.assertEqual(maps[str(DB['BUILD_LINK'])], '/pocol/src')
        self.assertEqual(maps[str(checkout.resolve())], '/pocol/src')
        self.assertEqual(maps[str(self.root / 'cargo')], '/pocol/cargo-home')
        self.assertEqual(maps[str(self.root / 'cargo/registry')], '/pocol/cargo-home/registry')
        self.assertEqual(maps[str(self.root / 'rustup')], '/pocol/rustup')
        froms = [f.split('=', 1)[1].rsplit('=', 1)[0] for f in flags]
        self.assertEqual([len(f) for f in froms], sorted(len(f) for f in froms), 'the more specific mapping comes last')
        # The C compilers (cc crate: blst, c-kzg, secp256k1-sys) get the same mappings.
        self.assertEqual(env['CFLAGS'].split(), [f'-ffile-prefix-map={f.split("=", 1)[1]}' for f in flags])
        self.assertEqual(env['CFLAGS'], env['ASMFLAGS'])
        # A second checkout elsewhere: other remap sources, the same targets and build path.
        other = DB['reproducible_env'](git_repo(self.root / 'deeper/other'), base)
        self.assertEqual(other['CARGO_TARGET_DIR'], env['CARGO_TARGET_DIR'])
        self.assertEqual(other['SOURCE_DATE_EPOCH'], env['SOURCE_DATE_EPOCH'])

    def test_paths_that_cannot_be_remapped_are_refused(self):
        for bad in ('/a=b', 'relative', '/a\nb', '/a b', '/a\tb', '/a\x1fb'):
            with self.subTest(path=bad), self.assertRaises(DB['Refused']):
                DB['checked_path'](Path(bad) if bad != 'relative' else bad)

    def test_identical_legs_compare_equal(self):
        members = {'pocold': b'\x7fELF same', 'release-info.json': b'{}'}
        a, b = leg(self.root, 'a', members), leg(self.root, 'b', members)
        with contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(DB['compare'](a, b, False), 0)
        self.assertIn('IDENTICAL', output.getvalue())

    def test_differing_legs_fail_with_a_summary(self):
        a = leg(self.root, 'a', {'pocold': b'\x7fELF one', 'README.md': b'same'})
        b = leg(self.root, 'b', {'pocold': b'\x7fELF two!', 'README.md': b'same', 'extra': b'x'})
        with contextlib.redirect_stdout(io.StringIO()) as output, contextlib.redirect_stderr(io.StringIO()) as errors:
            self.assertEqual(DB['compare'](a, b, False), 1)
        text = output.getvalue()
        self.assertIn('DIFFERENT', errors.getvalue())
        self.assertIn('pocold: sizes 8 / 9, first differing byte at offset 5 (0x5)', text)
        self.assertIn('extra: only in leg b', text)
        self.assertNotIn('README.md: sizes', text)
        self.assertIn('members.txt:', text)

    def test_an_unfinished_leg_never_compares_equal(self):
        members = {'pocold': b'same'}
        a, b = leg(self.root, 'a', members), leg(self.root, 'b', members)
        for name in DB['LISTS']:
            with self.subTest(missing=name):
                saved = (b / name).read_bytes()
                (b / name).write_bytes(b'')
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(DB['compare'](a, b, False), 1)
                (b / name).unlink()
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(DB['compare'](a, b, False), 1)
                (b / name).write_bytes(saved)

    def test_the_compared_archive_is_the_one_listed(self):
        """The archive CI uploads for signing (a leg's dist/) must be the one whose SHA-256 both legs agreed on."""
        members = {'pocold': b'\x7fELF same'}
        cases = {
            'tampered': lambda d: (d / 'dist' / 'PoCol-Mainnet-0.0.0-linux-x86_64.tar.gz').write_bytes(b'not the archive'),
            'extra file': lambda d: (d / 'dist' / 'stray.txt').write_bytes(b'x'),
            'missing': lambda d: (d / 'dist' / 'PoCol-Mainnet-0.0.0-linux-x86_64.tar.gz').unlink(),
            'no dist': lambda d: __import__('shutil').rmtree(d / 'dist'),
            'no SHA256SUMS': lambda d: (d / 'dist' / 'SHA256SUMS').unlink(),
            'other SHA256SUMS': lambda d: (d / 'dist' / 'SHA256SUMS').write_text('0' * 64 + '  x.tar.gz\n', encoding='utf-8'),
        }
        for case, spoil in cases.items():
            for which in 'ab':
                with self.subTest(case=case, leg=which):
                    root = self.root / f'{case}-{which}'
                    a, b = leg(root, 'a', members), leg(root, 'b', members)
                    spoil(a if which == 'a' else b)
                    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()) as errors:
                        self.assertEqual(DB['compare'](a, b, False), 1)
                    self.assertIn(f'leg {which}:', errors.getvalue())

    def use_build_link(self):
        """Points the script's BUILD_LINK into this test's directory (the functions read their own module globals)."""
        g = DB['build_dir'].__globals__
        saved = g['BUILD_LINK']
        g['BUILD_LINK'] = self.root / 'native-build' / 'src'
        self.addCleanup(g.__setitem__, 'BUILD_LINK', saved)
        return g['BUILD_LINK']

    def test_one_build_at_a_time(self):
        link = self.use_build_link()
        first, second = self.root / 'first', self.root / 'second'
        first.mkdir(); second.mkdir()
        with DB['build_link'](first):
            self.assertEqual(os.readlink(link), str(first))
            # A second build while the first holds the lock is refused and leaves the first build's link alone.
            with self.assertRaises(DB['Refused']):
                with DB['build_link'](second):
                    self.fail('a second build must not get the build path')
            self.assertEqual(os.readlink(link), str(first))
        self.assertFalse(link.is_symlink(), 'the link goes when the build ends')
        # Another process holding the lock (flock is per open file description) is refused the same way.
        fd = os.open(link.parent / 'lock', os.O_RDWR)
        self.addCleanup(os.close, fd)
        fcntl.flock(fd, fcntl.LOCK_EX)
        with self.assertRaises(DB['Refused']):
            with DB['build_link'](second):
                pass
        fcntl.flock(fd, fcntl.LOCK_UN)
        # A link left by a build that died is replaced; a link someone else put there meanwhile is not removed.
        os.symlink(self.root / 'dead', link)
        with DB['build_link'](first):
            self.assertEqual(os.readlink(link), str(first))
            link.unlink(); os.symlink(second, link)
        self.assertEqual(os.readlink(link), str(second))
        link.unlink()
        # A build directory others can write to is refused.
        os.chmod(link.parent, 0o777)
        with self.assertRaises(DB['Refused']):
            with DB['build_link'](first):
                pass
        os.chmod(link.parent, 0o755)

    def test_cargo_configuration_outside_the_commit_is_refused(self):
        outer = self.root / 'outer'
        checkout = git_repo(outer / 'deep' / 'checkout')
        home = self.root / 'cargo'
        (home).mkdir()
        (home / 'config.toml').write_text('[net]\nretry = 3\n', encoding='utf-8')
        env = {'CARGO_HOME': str(home)}
        DB['check_cargo_config'](checkout, env)  # the cargo home's own configuration is the builder's
        # A .cargo/config.toml in a directory above the checkout (as /tmp/.cargo would be): refused.
        for where, name in ((outer, 'config.toml'), (outer / 'deep', 'config')):
            with self.subTest(where=str(where), name=name):
                (where / '.cargo').mkdir()
                (where / '.cargo' / name).write_text('[build]\nrustc-wrapper = "/bin/false"\n', encoding='utf-8')
                with self.assertRaises(DB['Refused']) as caught:
                    DB['check_cargo_config'](checkout, env)
                self.assertIn(str(where / '.cargo' / name), str(caught.exception))
                (where / '.cargo' / name).unlink(); (where / '.cargo').rmdir()
        # A symlink above the checkout to the cargo home's file is the same file: allowed.
        (outer / '.cargo').mkdir()
        (outer / '.cargo' / 'config.toml').symlink_to(home / 'config.toml')
        DB['check_cargo_config'](checkout, env)
        (outer / '.cargo' / 'config.toml').unlink(); (outer / '.cargo').rmdir()
        # In the checkout: an untracked (or ignored) configuration is refused, a committed one is the commit's.
        (checkout / '.cargo').mkdir()
        (checkout / '.cargo' / 'config.toml').write_text('[build]\njobs = 1\n', encoding='utf-8')
        with self.assertRaises(DB['Refused']):
            DB['check_cargo_config'](checkout, env)
        git = ['git', '-c', 'user.name=t', '-c', 'user.email=t@invalid', '-c', 'commit.gpgsign=false']
        subprocess.run(git + ['add', '.cargo/config.toml'], cwd=checkout, check=True)
        subprocess.run(git + ['commit', '-q', '-m', 'cfg'], cwd=checkout, check=True)
        DB['check_cargo_config'](checkout, env)

    def test_first_difference(self):
        pa, pb = self.root / 'x', self.root / 'y'
        for one, two, want in ((b'abc', b'abc', None), (b'abc', b'abd', 2), (b'ab', b'abc', 2),
                               (b'a' * 70000 + b'b', b'a' * 70000 + b'c', 70000)):
            pa.write_bytes(one); pb.write_bytes(two)
            self.assertEqual(DB['first_difference'](pa, pb), want)


if __name__ == '__main__':
    unittest.main()
