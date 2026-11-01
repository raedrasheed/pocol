#!/usr/bin/env python3
"""Release boundary checks: no old invitation, wallet data or replacement signer."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch


HERE = Path(__file__).resolve().parent


class ReleasePackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='pocol-package-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.script = self.root / 'deploy/standalone/package.py'
        self.script.parent.mkdir(parents=True)
        shutil.copyfile(HERE / 'package.py', self.script)
        self.build = {'compatibilitySha256': 'c' * 64}
        self.commit = 'd' * 40
        for name in ('README.md', 'RELEASE-UPGRADE.md', 'install-linux.sh',
                     'Start-PoCol.cmd', 'Initialize-PoColNetwork.ps1'):
            (self.script.parent / name).write_text('public fixture\n', encoding='utf-8')
        for name in ('network.pocol.json', 'wallet.key', 'debug.keystore'):
            (self.root / name).write_bytes(b'EXCLUDED FIXTURE')

    def fixture(self, path):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(b'public artifact fixture\n')
        return file

    def package(self, platform):
        out = self.root / 'dist'

        def command(args, **kwargs):
            if args == ['git', 'rev-parse', 'HEAD']:
                return self.commit + '\n'
            self.assertEqual(args[-1], 'build-info')
            return json.dumps(self.build).encode('utf-8')

        with patch.object(sys, 'argv', ['package.py', '--platform', platform, '--out', str(out)]), patch('subprocess.check_output', command):
            runpy.run_path(str(self.script), run_name='__main__')
        for name in ('network.pocol.json', 'wallet.key', 'debug.keystore'):
            self.assertEqual((self.root / name).read_bytes(), b'EXCLUDED FIXTURE')
        return out

    def metadata(self, folder):
        release = json.loads((folder / 'release-info.json').read_text(encoding='utf-8'))
        self.assertEqual(release['compatibilitySha256'], self.build['compatibilitySha256'])
        self.assertEqual(release['sourceCommit'], self.commit)
        self.assertEqual(release['networkName'], 'PoCol Mainnet')
        self.assertTrue(release['requiresCoordinatedUpgrade'])
        self.assertIsNone(release['activationHeight'])
        self.assertFalse(release['networkInvitationIncluded'])
        self.assertFalse(release['publicLaunchReady'])
        self.assertNotIn('network.pocol.json', {f.name for f in folder.iterdir()})
        for line in (folder / 'SHA256SUMS').read_text(encoding='utf-8').splitlines():
            expected, name = line.split('  ', 1)
            self.assertEqual(hashlib.sha256((folder / name).read_bytes()).hexdigest(), expected)
        return release

    def test_android_packages_only_unsigned_release_without_old_signer_or_invitation(self):
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk')
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/debug/app-debug.apk')
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/release/wallet.key')
        info = self.fixture('prototypes/pocold/target/aarch64-linux-android/release/build/pocold-fixture/out/build-info.json')
        info.write_text(json.dumps(self.build), encoding='utf-8')
        out = self.package('Android')
        release = self.metadata(out)
        self.assertEqual({p.name for p in out.iterdir()}, {
            'PoCol-Mainnet-0.4.0-arm64-unsigned.apk', 'README.md', 'RELEASE-UPGRADE.md',
            'build-info.json', 'release-info.json', 'source-commit.txt', 'SHA256SUMS'})
        self.assertFalse(release['installable'])
        self.assertEqual(release['applicationId'], 'org.pocol.node')
        self.assertEqual(release['versionCode'], 5)
        self.assertEqual(release['signingStatus'], 'blocked-requires-existing-signer')

    def test_native_complete_packages_preserve_modes_and_exclude_old_network(self):
        bins = ('pocol', 'pocold', 'pocol-key', 'pocol-sign', 'pocol-site')
        for platform in ('Windows', 'Linux'):
            with self.subTest(platform=platform):
                suffix = '.exe' if platform == 'Windows' else ''
                for name in bins:
                    self.fixture('prototypes/pocold/target/release/' + name + suffix).chmod(0o755)
                if platform == 'Windows':
                    self.fixture('prototypes/pocold/target/release/PoCol-Wallet.exe')
                    self.fixture('wallet-preview.png')
                else:
                    self.fixture('deploy/standalone/linux-desktop/pocol-desktop').chmod(0o755)
                self.fixture('prototypes/pocold/target/release/network.pocol.json')
                self.fixture('prototypes/pocold/target/release/wallet.key')
                out = self.package(platform)
                if platform == 'Linux':
                    archive = out / 'PoCol-Mainnet-0.4.0-linux-x86_64.tar.gz'
                    with tarfile.open(archive) as tar:
                        names = tar.getnames()
                        self.assertNotIn('network.pocol.json', names)
                        self.assertNotIn('wallet.key', names)
                        if os.name != 'nt':
                            self.assertEqual(tar.getmember('pocol').mode & 0o111, 0o111)
                        expanded = self.root / 'expanded'
                        expanded.mkdir()
                        for member in tar.getmembers():
                            self.assertEqual(Path(member.name).name, member.name)
                            self.assertTrue(member.isfile())
                            (expanded / member.name).write_bytes(tar.extractfile(member).read())
                    self.metadata(expanded)
                else:
                    self.metadata(out)
                    self.assertTrue(all((out / (name + suffix)).is_file() for name in bins))
                    self.assertTrue((out / 'PoCol-Wallet.exe').is_file())
                shutil.rmtree(out)


if __name__ == '__main__':
    unittest.main()
