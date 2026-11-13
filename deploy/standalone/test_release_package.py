#!/usr/bin/env python3
"""Release boundary checks: no old invitation, wallet data or replacement signer."""
import hashlib
import json
import os
import re
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
        self.identity = self.root / 'deploy/public-testnet/pocol_identity.py'
        self.identity.parent.mkdir(parents=True)
        shutil.copyfile(HERE.parent / 'public-testnet/pocol_identity.py', self.identity)
        self.build = {'compatibilitySha256': 'c' * 64, 'consensusPinned': True, 'consensusRulesVersion': 1,
                      'protocolVersionMin': 2, 'consensusSourcesSha256': 'e' * 64, 'features': []}
        self.commit = 'd' * 40
        for name in ('README.md', 'install-linux.sh',
                     'Start-PoCol.cmd', 'Initialize-PoColNetwork.ps1'):
            (self.script.parent / name).write_text('public fixture\n', encoding='utf-8')
        for name in ('network.pocol.json', 'wallet.key', 'debug.keystore'):
            (self.root / name).write_bytes(b'EXCLUDED FIXTURE')
        notice = self.root / 'prototypes/third-party/evm-crypto/NOTICE.txt'
        notice.parent.mkdir(parents=True)
        notice.write_bytes(b'public crypto license fixture\n')

    def fixture(self, path):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(b'public artifact fixture\n')
        return file

    def package(self, platform, *extra):
        out = self.root / 'dist'

        def command(args, **kwargs):
            if args == ['git', 'rev-parse', 'HEAD']:
                return self.commit + '\n'
            self.assertEqual(args[-1], 'build-info')
            return json.dumps(self.build).encode('utf-8')

        with patch.object(sys, 'argv', ['package.py', '--platform', platform, '--out', str(out), *extra]), patch('subprocess.check_output', command):
            runpy.run_path(str(self.script), run_name='__main__')
        for name in ('network.pocol.json', 'wallet.key', 'debug.keystore'):
            self.assertEqual((self.root / name).read_bytes(), b'EXCLUDED FIXTURE')
        return out

    def metadata(self, folder):
        self.assertEqual((folder / 'THIRD-PARTY-CRYPTO-NOTICES.txt').read_bytes(),
                         b'public crypto license fixture\n')
        release = json.loads((folder / 'release-info.json').read_text(encoding='utf-8'))
        self.assertEqual(release['compatibilitySha256'], self.build['compatibilitySha256'])
        self.assertEqual((release['consensusRulesVersion'], release['protocolVersionMin']), (1, 2))
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
            'PoCol-Mainnet-0.4.0-arm64-unsigned.apk', 'README.md',
            'build-info.json', 'release-info.json', 'source-commit.txt', 'SHA256SUMS',
            'THIRD-PARTY-CRYPTO-NOTICES.txt'})
        self.assertFalse(release['installable'])
        self.assertEqual(release['applicationId'], 'org.pocol.node')
        self.assertEqual(release['versionCode'], 5)
        self.assertEqual(release['signingStatus'], 'blocked-requires-existing-signer')
        # V3-28 H1: the raw identity plus the readiness keys the desktop build-info has, marked as not computed.
        packaged = json.loads((out / 'build-info.json').read_text(encoding='utf-8'))
        self.assertEqual({k: packaged[k] for k in self.build}, self.build)
        self.assertIs(packaged['publicLaunchReady'], False)
        self.assertEqual([b['id'] for b in packaged['launchBlockers']], ['readiness-not-computed'])
        self.assertEqual(set(packaged), set(self.build) | {'publicLaunchReady', 'launchBlockers'})

    def test_android_new_key_app_is_packaged_under_its_own_application_id(self):
        # V3-33: the new-key build is another app (org.pocol.wallet) that installs beside org.pocol.node.
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk')
        info = self.fixture('prototypes/pocold/target/aarch64-linux-android/release/build/pocold-fixture/out/build-info.json')
        info.write_text(json.dumps(self.build), encoding='utf-8')
        out = self.package('Android', '--android-application-id', 'org.pocol.wallet')
        release = self.metadata(out)
        self.assertIn('PoCol-Mainnet-0.4.0-arm64-org.pocol.wallet-unsigned.apk', {p.name for p in out.iterdir()})
        self.assertNotIn('PoCol-Mainnet-0.4.0-arm64-unsigned.apk', {p.name for p in out.iterdir()})
        self.assertEqual((release['applicationId'], release['installsBeside']), ('org.pocol.wallet', 'org.pocol.node'))
        self.assertEqual(release['signingStatus'], 'requires-new-release-signer')
        self.assertNotIn('lastDistributedSignerSha256', release)
        self.assertFalse(release['installable'])
        with self.assertRaises(SystemExit):
            self.package('Android', '--android-application-id', 'org.example.other')

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


    def native_fixture(self):
        for name in ('pocol', 'pocold', 'pocol-key', 'pocol-sign', 'pocol-site'):
            self.fixture('prototypes/pocold/target/release/' + name).chmod(0o755)
        self.fixture('deploy/standalone/linux-desktop/pocol-desktop').chmod(0o755)

    def test_an_unpinned_or_unlabelled_consensus_build_is_never_packaged(self):
        self.native_fixture()
        for change in ({'consensusPinned': False}, {'consensusPinned': None}):
            with self.subTest(change=change):
                self.build = {**self.build, **change}
                with self.assertRaisesRegex(SystemExit, 'unpinned consensus build'): self.package('Linux')
                self.assertEqual({p.name for p in (self.root / 'dist').iterdir()}, {'build-info.json'}, 'nothing else is staged')
                shutil.rmtree(self.root / 'dist')

    def test_preview_artifacts_are_refused_even_when_consensus_is_pinned(self):
        self.native_fixture()
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk')
        android_info = self.fixture('prototypes/pocold/target/aarch64-linux-android/release/build/pocold-fixture/out/build-info.json')
        for platform in ('Linux', 'Android'):
            for change in ({'v3Preview': True}, {'v3Preview': False, 'features': ['v3-preview']}):
                with self.subTest(platform=platform, change=change):
                    self.build = {**self.build, 'v3Preview': False, 'features': [], **change}
                    android_info.write_text(json.dumps(self.build), encoding='utf-8')
                    with self.assertRaisesRegex(SystemExit, 'v3-preview build'):
                        self.package(platform)
                    self.assertEqual({p.name for p in (self.root / 'dist').iterdir()}, {'build-info.json'})
                    shutil.rmtree(self.root / 'dist')

    def test_a_certified_legacy_entry_must_name_the_packaged_consensus_sources(self):
        self.native_fixture()
        text = self.identity.read_text(encoding='utf-8')
        certified = text.replace("'sources': None, 'certified': None", "'sources': '%s', 'certified': 'evidence'")
        self.assertNotEqual(certified, text, 'the shipped mirror is uncertified')
        self.identity.write_text(certified % ('f' * 64), encoding='utf-8')
        with self.assertRaisesRegex(SystemExit, 'rerun the replay'): self.package('Linux')
        shutil.rmtree(self.root / 'dist')
        self.identity.write_text(certified % self.build['consensusSourcesSha256'], encoding='utf-8')
        self.metadata(self.expanded(self.package('Linux')))

    def test_release_info_names_the_consensus_identity_only_of_a_matching_v2_invitation(self):
        self.native_fixture()
        # The network package.py names (switched with it at the cutover), so no identity is pinned here.
        chain, genesis = re.search(r"chain,genesis=([0-9]+),'(0x[0-9a-f]{64})'", self.script.read_text(encoding='utf-8')).groups()
        info = {'chainId': int(chain), 'genesisHash': genesis, 'forks': [[1, 0], [2, 15000]]}
        network = self.root / 'next.pocol.json'
        network.write_text(json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': 1, 'seeds': [],
                                       'files': {'devnet.json': json.dumps(info)}}), encoding='utf-8')
        release = self.metadata(self.expanded(self.package('Linux', '--network', str(network))))
        reference = runpy.run_path(str(self.identity))['consensus_id']
        self.assertEqual(release['consensusIdentity'], reference(int(chain), genesis, 1, 0, [(1, 0), (2, 15000)]))
        shutil.rmtree(self.root / 'dist'); shutil.rmtree(self.root / 'expanded')
        self.assertIsNone(self.metadata(self.expanded(self.package('Linux')))['consensusIdentity'], 'no invitation, no claim')
        shutil.rmtree(self.root / 'dist')
        network.write_text(json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': [],
                                       'files': {'devnet.json': json.dumps(info)}}), encoding='utf-8')
        with self.assertRaisesRegex(SystemExit, 'other consensus rules'): self.package('Linux', '--network', str(network))

    def expanded(self, out):
        expanded = self.root / 'expanded'
        shutil.rmtree(expanded, ignore_errors=True); expanded.mkdir()
        with tarfile.open(out / 'PoCol-Mainnet-0.4.0-linux-x86_64.tar.gz') as tar:
            for member in tar.getmembers(): (expanded / member.name).write_bytes(tar.extractfile(member).read())
        return expanded


if __name__ == '__main__':
    unittest.main()
