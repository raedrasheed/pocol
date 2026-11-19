#!/usr/bin/env python3
"""Release boundary checks: no invitation, wallet data or signing key in the package; one Android application id; the
network identity only from a v3 invitation of the mainnet profile's chain; one release version."""
import hashlib
import io
import json
import os
import re
import pathlib
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from unittest.mock import patch


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
GRADLE = 'prototypes/pocol-android/app/build.gradle'
MAINNET = ('retired-networks.json', 'placeholder-network.pocol.json', 'v3-profile.json')
# The release version, read from package.py (its only definition), so a version bump touches no test.
VERSION = re.search(r"(?m)^version='([0-9]+\.[0-9]+\.[0-9]+)'$", (HERE / 'package.py').read_text(encoding='utf-8'))[1]


def mainnet_file(name):
    return json.loads((ROOT / 'deploy/mainnet' / name).read_text(encoding='utf-8'))


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
        # package.py reads these at run time: the Android build file, and the retired, placeholder and mainnet identities.
        (self.root / GRADLE).parent.mkdir(parents=True)
        shutil.copyfile(ROOT / GRADLE, self.root / GRADLE)
        (self.root / 'deploy/mainnet').mkdir(parents=True)
        for name in MAINNET:
            shutil.copyfile(ROOT / 'deploy/mainnet' / name, self.root / 'deploy/mainnet' / name)
        self.build = {'compatibilitySha256': 'c' * 64, 'consensusPinned': True, 'consensusRulesVersion': 2,
                      'protocolVersionMin': 2, 'consensusSourcesSha256': 'e' * 64, 'features': [], 'version': VERSION,
                      'scope': 'mainnet'}
        self.commit = 'd' * 40
        self.commit_time = 1795029413
        for name in ('README.md', 'install-linux.sh',
                     'Start-PoCol.cmd', 'Initialize-PoColNetwork.ps1'):
            (self.script.parent / name).write_text('public fixture\n', encoding='utf-8')
        for name in ('network.pocol.json', 'wallet.key', 'debug.keystore'):
            (self.root / name).write_bytes(b'EXCLUDED FIXTURE')
        # FN-7: the committed profiles the Windows genesis ceremony fills, as committed: package.py also reads the
        # mainnet profile's chain ID at run time (MAINNET), so a fixture in its place would break --network.
        for name in ('v3-profile.json', 'v3-rehearsal-profile.json'):
            shutil.copyfile(ROOT / 'deploy/mainnet' / name, self.root / 'deploy/mainnet' / name)
        notice = self.root / 'prototypes/third-party/evm-crypto/NOTICE.txt'
        notice.parent.mkdir(parents=True)
        notice.write_bytes(b'public crypto license fixture\n')

    def fixture(self, path):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(b'public artifact fixture\n')
        return file

    def package(self, platform, *extra, source_date_epoch=None):
        out = self.root / 'dist'
        # The archive date comes from SOURCE_DATE_EPOCH when set (a reproducible-build job sets it), else the commit.
        env = {k: v for k, v in os.environ.items() if k != 'SOURCE_DATE_EPOCH'}
        if source_date_epoch is not None:
            env['SOURCE_DATE_EPOCH'] = source_date_epoch

        def command(args, **kwargs):
            if args == ['git', 'rev-parse', 'HEAD']:
                return self.commit + '\n'
            if args == ['git', 'log', '-1', '--format=%ct', 'HEAD']:
                return f'{self.commit_time}\n'
            self.assertEqual(args[-1], 'build-info')
            return json.dumps(self.build).encode('utf-8')

        with patch.object(sys, 'argv', ['package.py', '--platform', platform, '--out', str(out), *extra]), \
                patch('subprocess.check_output', command), patch.dict(os.environ, env, clear=True):
            runpy.run_path(str(self.script), run_name='__main__')
        for name in ('network.pocol.json', 'wallet.key', 'debug.keystore'):
            self.assertEqual((self.root / name).read_bytes(), b'EXCLUDED FIXTURE')
        return out

    def metadata(self, folder):
        self.assertEqual((folder / 'THIRD-PARTY-CRYPTO-NOTICES.txt').read_bytes(),
                         b'public crypto license fixture\n')
        release = json.loads((folder / 'release-info.json').read_text(encoding='utf-8'))
        self.assertEqual(release['version'], VERSION)
        self.assertEqual(release['compatibilitySha256'], self.build['compatibilitySha256'])
        self.assertEqual((release['consensusRulesVersion'], release['protocolVersionMin']), (2, 2))
        self.assertEqual(release['sourceCommit'], self.commit)
        self.assertEqual(release['networkName'], 'PoCol Mainnet')
        # A fresh network: no coordinated upgrade of an existing chain, and no activation height.
        self.assertNotIn('requiresCoordinatedUpgrade', release)
        self.assertNotIn('activationHeight', release)
        self.assertFalse(release['networkInvitationIncluded'])
        self.assertFalse(release['publicLaunchReady'])
        self.assertNotIn('network.pocol.json', {f.name for f in folder.iterdir()})
        for line in (folder / 'SHA256SUMS').read_text(encoding='utf-8').splitlines():
            expected, name = line.split('  ', 1)
            self.assertEqual(hashlib.sha256((folder / name).read_bytes()).hexdigest(), expected)
        return release

    def android_fixture(self):
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk')
        info = self.fixture('prototypes/pocold/target/aarch64-linux-android/release/build/pocold-fixture/out/build-info.json')
        info.write_text(json.dumps(self.build), encoding='utf-8')

    def test_android_packages_only_the_unsigned_org_pocol_wallet_release(self):
        self.android_fixture()
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/debug/app-debug.apk')
        self.fixture('prototypes/pocol-android/app/build/outputs/apk/release/wallet.key')
        out = self.package('Android')
        release = self.metadata(out)
        self.assertEqual({p.name for p in out.iterdir()}, {
            f'PoCol-Mainnet-{VERSION}-arm64-unsigned.apk', 'README.md',
            'build-info.json', 'release-info.json', 'source-commit.txt', 'SHA256SUMS',
            'THIRD-PARTY-CRYPTO-NOTICES.txt'})
        self.assertFalse(release['installable'])
        # The id and versionCode are build.gradle's, the only source (the CI badging check compares them with the APK).
        gradle = (ROOT / GRADLE).read_text(encoding='utf-8')
        self.assertEqual(release['applicationId'], 'org.pocol.wallet')
        self.assertEqual(re.findall(r"applicationId '([^']+)'", gradle), ['org.pocol.wallet'])
        self.assertEqual(release['versionCode'], int(re.search(r'versionCode ([0-9]+)', gradle)[1]))
        self.assertEqual(release['signingStatus'], 'requires-release-signer')
        for retired in ('lastDistributedSignerSha256', 'installsBeside'):
            self.assertNotIn(retired, release)
        self.assertEqual((release['chainId'], release['genesisHash'], release['consensusIdentity']), (None, None, None))
        # V3-28 H1: the raw identity plus the readiness keys the desktop build-info has, marked as not computed.
        packaged = json.loads((out / 'build-info.json').read_text(encoding='utf-8'))
        self.assertEqual({k: packaged[k] for k in self.build}, self.build)
        self.assertIs(packaged['publicLaunchReady'], False)
        self.assertEqual([b['id'] for b in packaged['launchBlockers']], ['readiness-not-computed'])
        self.assertEqual(set(packaged), set(self.build) | {'publicLaunchReady', 'launchBlockers'})

    def test_there_is_one_android_application_id_and_no_option_for_another(self):
        self.android_fixture()
        with self.assertRaises(SystemExit) as refused, patch('sys.stderr'):
            self.package('Android', '--android-application-id', 'org.pocol.wallet')
        self.assertEqual(refused.exception.code, 2, 'argparse refuses the retired option as unknown')
        self.assertFalse((self.root / 'dist').exists())
        self.assertNotIn('pocolApplicationId', (ROOT / GRADLE).read_text(encoding='utf-8'))
        self.assertNotIn('pocolAppLabel', (ROOT / 'prototypes/pocol-android/app/src/main/AndroidManifest.xml').read_text(encoding='utf-8'))

    def test_an_android_build_of_another_version_or_without_one_id_is_refused(self):
        self.android_fixture()
        gradle = self.root / GRADLE
        text = gradle.read_text(encoding='utf-8')
        for change, message in (((f"versionName '{VERSION}'", "versionName '9.9.9'"), 'not the release version'),
                                (("applicationId 'org.pocol.wallet'", "applicationId 'org.pocol.wallet'\napplicationId 'org.pocol.other'"), 'one applicationId'),
                                (('versionCode ', 'versionCode = '), 'one applicationId, versionCode')):
            with self.subTest(change=change):
                self.assertIn(change[0], text)
                gradle.write_text(text.replace(change[0], change[1], 1), encoding='utf-8')
                with self.assertRaisesRegex(SystemExit, message): self.package('Android')
                shutil.rmtree(self.root / 'dist')

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
                # Paths order as on a Windows runner (WindowsPath compares case-insensitively): the archive's entry
                # order, and so its bytes, must not depend on the OS that packs it.
                with patch.object(pathlib.PurePath, '__lt__', lambda a, b: str(a).lower() < str(b).lower()):
                    out = self.package(platform)
                if platform == 'Linux':
                    archive = out / f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz'
                    with tarfile.open(archive) as tar:
                        names = tar.getnames()
                        self.assertNotIn('network.pocol.json', names)
                        self.assertNotIn('wallet.key', names)
                        # The genesis ceremony runs from the Windows package only.
                        self.assertFalse({'v3-profile.json', 'v3-rehearsal-profile.json'} & set(names))
                        if os.name != 'nt':
                            self.assertEqual(tar.getmember('pocol').mode & 0o111, 0o111)
                        expanded = self.root / 'expanded'
                        expanded.mkdir()
                        for member in tar.getmembers():
                            self.assertEqual(Path(member.name).name, member.name)
                            self.assertTrue(member.isfile())
                            (expanded / member.name).write_bytes(tar.extractfile(member).read())
                    self.assertEqual(names, sorted(names))
                    release = self.metadata(expanded)
                else:
                    # The Windows download is one byte-stable archive with the package's SHA256SUMS inside.
                    archive = out / f'PoCol-Mainnet-{VERSION}-windows-x86_64.zip'
                    self.assertEqual({p.name for p in out.iterdir()}, {archive.name, 'SHA256SUMS'})
                    self.assertEqual((out / 'SHA256SUMS').read_text(encoding='utf-8'),
                                     f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n')
                    expanded = self.root / 'expanded'
                    expanded.mkdir()
                    with zipfile.ZipFile(archive) as zf:
                        for info in zf.infolist():
                            self.assertEqual(Path(info.filename).name, info.filename)
                            self.assertEqual(info.date_time, (1980, 1, 1, 0, 0, 0))
                            (expanded / info.filename).write_bytes(zf.read(info))
                        names = zf.namelist()
                    self.assertEqual(names, sorted(names))
                    self.assertNotIn('network.pocol.json', names)
                    self.assertNotIn('wallet.key', names)
                    release = self.metadata(expanded)
                    self.assertTrue(all((expanded / (name + suffix)).is_file() for name in bins))
                    self.assertTrue((expanded / 'PoCol-Wallet.exe').is_file())
                    for name in ('v3-profile.json', 'v3-rehearsal-profile.json'):
                        self.assertEqual((expanded / name).read_bytes(), (self.root / 'deploy/mainnet' / name).read_bytes())
                        self.assertIn('  ' + name, (expanded / 'SHA256SUMS').read_text(encoding='utf-8'))
                    # Byte-stable: the same inputs give the same archive, here packed in this OS's own path order.
                    first = archive.read_bytes()
                    shutil.rmtree(out); shutil.rmtree(expanded)
                    self.assertEqual((self.package(platform) / archive.name).read_bytes(), first)
                    expanded.mkdir()
                    with zipfile.ZipFile(io.BytesIO(first)) as zf:
                        zf.extract('SHA256SUMS', expanded)
                    inner = [l.split('  ', 1)[1] for l in (expanded / 'SHA256SUMS').read_text(encoding='utf-8').splitlines()]
                    self.assertEqual(inner, sorted(inner), 'the package SHA256SUMS lists its files in ordinal order')
                    shutil.rmtree(expanded)
                self.assertNotIn('applicationId', release)
                shutil.rmtree(out)


    def native_fixture(self):
        for name in ('pocol', 'pocold', 'pocol-key', 'pocol-sign', 'pocol-site'):
            self.fixture('prototypes/pocold/target/release/' + name).chmod(0o755)
        self.fixture('deploy/standalone/linux-desktop/pocol-desktop').chmod(0o755)

    def test_the_linux_archive_is_byte_stable(self):
        # deploy/release/double_build.py compares the archives of two builds byte for byte: the archive must not carry
        # the packing time, the inputs' mtimes or modes beyond the executable bit, the umask, the TZ or owner names.
        self.native_fixture()
        name = f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz'
        executables = {'pocol', 'pocold', 'pocol-key', 'pocol-sign', 'pocol-site', 'pocol-desktop'}
        out = self.package('Linux')
        first = (out / name).read_bytes()
        self.assertEqual(first[:4], b'\x1f\x8b\x08\x00', 'gzip without a file name or other optional fields')
        self.assertEqual(first[4:8], b'\0\0\0\0', 'no gzip timestamp')
        with tarfile.open(fileobj=io.BytesIO(first)) as tar:
            self.assertEqual(tar.pax_headers, {})
            members = tar.getmembers()
        self.assertEqual([m.name for m in members], sorted(m.name for m in members))
        for m in members:
            with self.subTest(member=m.name):
                self.assertEqual((m.mtime, m.uid, m.gid, m.uname, m.gname, m.pax_headers), (self.commit_time, 0, 0, '', '', {}))
                if os.name != 'nt':
                    self.assertEqual(m.mode, 0o755 if m.name in executables else 0o644)
        self.assertTrue(executables <= {m.name for m in members})
        shutil.rmtree(out)
        # Other input mtimes and modes, another umask and TZ: the same bytes.
        for path in (self.root / 'prototypes/pocold/target/release').iterdir():
            os.utime(path, (1_000_000_000, 1_000_000_000))
            if os.name != 'nt': path.chmod(0o700)
        for path in (self.script.parent / 'README.md', self.script.parent / 'install-linux.sh'):
            os.utime(path, (2_000_000_000, 2_000_000_000))
            if os.name != 'nt': path.chmod(0o600)
        old_umask = os.umask(0o077)
        try:
            with patch.dict(os.environ, {'TZ': 'Pacific/Kiritimati'}):
                again = (self.package('Linux') / name).read_bytes()
        finally:
            os.umask(old_umask)
        self.assertEqual(again, first)
        shutil.rmtree(out)
        # SOURCE_DATE_EPOCH, when a build sets it, is the one date; anything but whole seconds is refused.
        with tarfile.open(self.package('Linux', source_date_epoch='1234567890') / name) as tar:
            self.assertEqual({m.mtime for m in tar.getmembers()}, {1234567890})
        shutil.rmtree(out)
        for bad in ('', '12.5', '-1', 'now'):
            with self.subTest(source_date_epoch=bad):
                with self.assertRaisesRegex(SystemExit, 'SOURCE_DATE_EPOCH'):
                    self.package('Linux', source_date_epoch=bad)
                shutil.rmtree(out)

    def test_a_build_of_another_version_is_never_packaged(self):
        self.native_fixture()
        for change in ({'version': '0.4.0'}, {'version': None}):
            with self.subTest(change=change):
                self.build.update(change)
                if change['version'] is None: del self.build['version']
                with self.assertRaisesRegex(SystemExit, 'not the release version'):
                    self.package('Linux')
                shutil.rmtree(self.root / 'dist', ignore_errors=True)
                self.build['version'] = VERSION

    def test_an_unpinned_or_unlabelled_consensus_build_is_never_packaged(self):
        self.native_fixture()
        for change in ({'consensusPinned': False}, {'consensusPinned': None}):
            with self.subTest(change=change):
                self.build = {**self.build, **change}
                with self.assertRaisesRegex(SystemExit, 'unpinned consensus build'): self.package('Linux')
                self.assertEqual({p.name for p in (self.root / 'dist').iterdir()}, {'build-info.json'}, 'nothing else is staged')
                shutil.rmtree(self.root / 'dist')

    def test_a_pinned_rules_2_build_packages_on_every_platform_with_its_build_info(self):
        # The v3 rules are the release rules (consensus rules 2, GenesisSpec version 2 by default): pocold's build-info
        # has no v3Preview field and there is no preview build to refuse, so a pinned build packages on every platform
        # and its release-info carries the build's rules as reported.
        self.native_fixture()
        self.android_fixture()
        android_info = self.root / 'prototypes/pocold/target/aarch64-linux-android/release/build/pocold-fixture/out/build-info.json'
        self.assertNotIn('v3Preview', self.build)
        script = self.script.read_text(encoding='utf-8')
        self.assertNotIn('v3Preview', script); self.assertNotIn('v3-preview', script)
        for platform in ('Linux', 'Android'):
            with self.subTest(platform=platform):
                android_info.write_text(json.dumps(self.build), encoding='utf-8')
                out = self.package(platform)
                try:
                    folder = self.expanded(out) if platform == 'Linux' else out
                    release = json.loads((folder / 'release-info.json').read_text(encoding='utf-8'))
                    self.assertEqual((release['consensusRulesVersion'], release['protocolVersionMin']), (2, 2))
                    self.assertEqual(json.loads((folder / 'build-info.json').read_text(encoding='utf-8'))['consensusRulesVersion'], 2)
                finally:
                    shutil.rmtree(out); shutil.rmtree(self.root / 'expanded', ignore_errors=True)

    def invitation(self, info, rules=2):
        network = self.root / 'next.pocol.json'
        network.write_text(json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': rules, 'seeds': [],
                                       'files': {'devnet.json': json.dumps(info)}}), encoding='utf-8')
        return network

    def v3_info(self, **change):
        # The mainnet profile's chain ID (read, never written here) under a fixture genesis of no network.
        info = {'chainId': mainnet_file('v3-profile.json')['chainId'], 'genesisHash': '0x' + 'ab' * 32,
                'specVersion': 2, 'v3': True, 'forks': [[1, 0], [2, 1000]]}
        info.update(change)
        return {k: v for k, v in info.items() if v is not None}

    def refused(self, info, message, rules=2):
        with self.assertRaisesRegex(SystemExit, message): self.package('Linux', '--network', str(self.invitation(info, rules)))
        self.assertEqual({p.name for p in (self.root / 'dist').iterdir()}, {'build-info.json'}, 'nothing else is staged')
        shutil.rmtree(self.root / 'dist')

    def test_release_info_takes_the_network_identity_only_from_a_v3_invitation_of_the_mainnet_profile(self):
        self.native_fixture()
        info = self.v3_info()
        release = self.metadata(self.expanded(self.package('Linux', '--network', str(self.invitation(info)))))
        reference = runpy.run_path(str(self.identity))['consensus_id']
        self.assertEqual((release['chainId'], release['genesisHash']), (info['chainId'], info['genesisHash']))
        self.assertEqual(release['consensusIdentity'], reference(info['chainId'], info['genesisHash'], 2, 0, [(1, 0), (2, 1000)]))
        shutil.rmtree(self.root / 'dist'); shutil.rmtree(self.root / 'expanded')
        upper = self.v3_info(genesisHash='0x' + 'AB' * 32)
        release = self.metadata(self.expanded(self.package('Linux', '--network', str(self.invitation(upper)))))
        self.assertEqual(release['genesisHash'], info['genesisHash'], 'recorded in lowercase')
        shutil.rmtree(self.root / 'dist'); shutil.rmtree(self.root / 'expanded')
        release = self.metadata(self.expanded(self.package('Linux')))
        self.assertEqual((release['chainId'], release['genesisHash'], release['consensusIdentity']), (None, None, None),
                         'no invitation, no claim: no release compiles a network identity in')
        shutil.rmtree(self.root / 'dist')

    def test_an_invitation_of_another_network_rules_or_spec_is_refused(self):
        self.native_fixture()
        retired = mainnet_file('retired-networks.json')['retired'][0]
        placeholder = json.loads(mainnet_file('placeholder-network.pocol.json')['files']['devnet.json'])
        rehearsal = mainnet_file('v3-rehearsal-profile.json')['chainId']
        self.assertNotEqual(rehearsal, mainnet_file('v3-profile.json')['chainId'])
        for info, message, rules in (
                # The old network's rules (1) bind other consensus rules.
                (self.v3_info(), 'other consensus rules', 1),
                (self.v3_info(chainId=retired['chainId'], genesisHash=retired['genesisHash']), 'retired network', 2),
                (self.v3_info(chainId=retired['chainId']), 'retired network', 2),
                (self.v3_info(genesisHash=retired['genesisHash']), 'retired network', 2),
                (self.v3_info(chainId=placeholder['chainId'], genesisHash=placeholder['genesisHash']), 'network placeholder', 2),
                (self.v3_info(genesisHash=placeholder['genesisHash']), 'network placeholder', 2),
                # A spec-1 devnet.json (no specVersion, no v3) and an inconsistent declaration.
                (self.v3_info(specVersion=None, v3=None), 'spec-1 network', 2),
                (self.v3_info(v3=None), 'no valid GenesisSpec version', 2),
                (self.v3_info(specVersion=3), 'no valid GenesisSpec version', 2),
                # A rehearsal invitation is never stamped as a PoCol Mainnet release.
                (self.v3_info(chainId=rehearsal), 'mainnet profile', 2),
                (self.v3_info(chainId=True), 'no chain ID and genesis hash', 2),
                (self.v3_info(genesisHash='0x1234'), 'no chain ID and genesis hash', 2)):
            with self.subTest(info=info, rules=rules):
                self.refused(info, message, rules)

    def test_the_identity_checks_fail_closed_without_their_files(self):
        self.native_fixture()
        for name in MAINNET:
            with self.subTest(name=name):
                path = self.root / 'deploy/mainnet' / name
                saved = path.read_bytes()
                for broken in (None, b'{}'):
                    if broken is None: path.unlink()
                    else: path.write_bytes(broken)
                    self.refused(self.v3_info(), 'refusing to package')
                path.write_bytes(saved)

    def test_the_release_version_is_one_constant(self):
        # package.py holds the version; every other shipped copy must equal it, and the workflow reads it from there.
        out = subprocess.run([sys.executable, '-I', str(HERE / 'package.py'), '--version'], capture_output=True, text=True, check=True)
        self.assertEqual(out.stdout.strip(), VERSION)
        copies = {
            GRADLE: rf"versionName '{re.escape(VERSION)}'",
            # build-info's `version` is the pocold crate version (build.rs).
            'prototypes/pocold/Cargo.toml': rf'(?m)\A\[package\]\nname = "pocold"\nversion = "{re.escape(VERSION)}"$',
            'prototypes/pocol-windows/Wallet.cs': rf'AssemblyVersion\("{re.escape(VERSION)}\.0"\)\][\s\S]*AssemblyFileVersion\("{re.escape(VERSION)}\.0"\)',
            'prototypes/pocol-windows/Wallet.xaml': rf'PoCol Wallet · {re.escape(VERSION)}&',
            'deploy/standalone/linux-desktop/pocol-desktop': rf'PoCol Mainnet · {re.escape(VERSION)} ',
            'deploy/standalone/linux-desktop/install-desktop-icon.sh': rf'PoCol {re.escape(VERSION)} local node',
            'deploy/standalone/README.md': rf'(?m)^# PoCol Mainnet · Wallet {re.escape(VERSION)} [\s\S]*PoCol-Mainnet-{re.escape(VERSION)}-linux-x86_64\.tar\.gz',
        }
        for rel, pattern in copies.items():
            with self.subTest(file=rel):
                text = (ROOT / rel).read_text(encoding='utf-8')
                self.assertRegex(text, pattern)
        workflow = (ROOT / '.github/workflows/five-device-build.yml').read_text(encoding='utf-8')
        # Tool pins (Rust toolchain, Android build-tools and NDK, Gradle) are not release versions: drop them, then
        # match on token boundaries so a release version inside another number (5.0.0 in 35.0.0) does not count.
        unpinned = re.sub(r'(?:toolchain install |cargo \+|build-tools[;/]|ndk;|gradle-)[0-9][0-9.]*', '', workflow)
        self.assertNotRegex(unpinned, rf'(?<![0-9.]){re.escape(VERSION)}(?![0-9])')
        self.assertIn('deploy/standalone/package.py --version', workflow)

    def expanded(self, out):
        expanded = self.root / 'expanded'
        shutil.rmtree(expanded, ignore_errors=True); expanded.mkdir()
        with tarfile.open(out / f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz') as tar:
            for member in tar.getmembers(): (expanded / member.name).write_bytes(tar.extractfile(member).read())
        return expanded


if __name__ == '__main__':
    unittest.main()
