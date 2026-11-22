#!/usr/bin/env python3
"""deploy/websites/tools/release_page.py: pocol.net's download buttons are filled only from a release directory that
verifies with the repository's allowed_signers, and every file, its .sig, SHA256SUMS and SHA256SUMS.sig become
pocol.net kept-out download lines. A throwaway ssh-ed25519 key, made in a temporary directory, signs a synthetic
release in a temporary copy of the tree; no committed key, release or host is used."""
import contextlib, hashlib, io, json, shutil, subprocess, sys, tempfile, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'tools'))
import release_page as rp  # noqa: E402
import site_identity as si  # noqa: E402

ROOT = si.ROOT
NATIVE = si.release.native_version(ROOT)
COPIED = ['deploy/websites', si.STUDIO]
COPIED_FILES = [si.RETIRED, si.EXTENSION_PACKAGE, si.NATIVE_PACKAGE]

def snapshot(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for top in COPIED for p in sorted((root / top).rglob('*')) if p.is_file() and '__pycache__' not in p.parts}

@unittest.skipUnless(shutil.which('ssh-keygen'), 'needs ssh-keygen (OpenSSH 8.2 or later)')
class ReleasePage(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix='pocol-release-page-'))
        self.addCleanup(shutil.rmtree, self.tmp)
        root = self.root = self.tmp / 'tree'
        for top in COPIED:
            shutil.copytree(ROOT / top, root / top, ignore=shutil.ignore_patterns('__pycache__', 'config.php'))
        for rel in COPIED_FILES:
            (root / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / rel, root / rel)
        # The switched tree: the extension at the native release version (owner decision 2026-11-16), so a release
        # can be named.
        pkg = json.loads((root / si.EXTENSION_PACKAGE).read_text(encoding='utf-8'))
        pkg['version'] = NATIVE
        (root / si.EXTENSION_PACKAGE).write_text(json.dumps(pkg, indent=2) + '\n', encoding='utf-8')
        key = self.tmp / 'throwaway'
        subprocess.run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-C', 'test', '-f', str(key)], check=True, timeout=60,
                       stdin=subprocess.DEVNULL)
        kind, blob = key.with_suffix('.pub').read_text().split()[:2]
        (root / 'deploy/release').mkdir(parents=True, exist_ok=True)
        (root / 'deploy/release/allowed_signers').write_text(f'release@pocol.net namespaces="pocol-release" {kind} {blob}\n')
        self.key = key
        self.release = self.tmp / 'release'
        self.release.mkdir()
        self.files = {f'PoCol-Mainnet-{NATIVE}-linux-x86_64.tar.gz': b'linux' * 300, f'PoCol-Mainnet-{NATIVE}-windows-x86_64.zip': b'win' * 999,
                      f'PoCol-Mainnet-{NATIVE}-arm64.apk': b'apk' * 5000, f'PoCol-Studio-IDE-Viewer-{NATIVE}.zip': b'ext' * 10}
        for name, data in self.files.items():
            (self.release / name).write_bytes(data)
        self.sign_all()

    def sign(self, path):
        sig = path.with_name(path.name + '.sig')
        sig.unlink(missing_ok=True)
        subprocess.run(['ssh-keygen', '-q', '-Y', 'sign', '-f', str(self.key), '-n', 'pocol-release', str(path)],
                       check=True, timeout=60, stdin=subprocess.DEVNULL, capture_output=True)

    def sign_all(self):
        sums = ''.join(f'{hashlib.sha256((self.release / n).read_bytes()).hexdigest()}  {n}\n' for n in sorted(self.files))
        (self.release / 'SHA256SUMS').write_text(sums, encoding='ascii')
        for name in [*self.files, 'SHA256SUMS']:
            self.sign(self.release / name)

    def run_tool(self, *args):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            rc = rp.main([str(self.release), '--root', str(self.root), *args])
        return rc, out.getvalue() + err.getvalue()

    def test_a_verified_release_fills_the_buttons_and_the_kept_out_lines(self):
        rc, out = self.run_tool('--check')
        self.assertEqual(rc, 0, out)
        before = snapshot(self.root)
        self.assertEqual(snapshot(self.root), before, '--check writes nothing')
        rc, out = self.run_tool()
        self.assertEqual(rc, 0, out)
        downloads = rp.read_downloads(self.root)
        base = 'https://pocol.net/downloads/'
        for platform, name in (('linux', f'PoCol-Mainnet-{NATIVE}-linux-x86_64.tar.gz'), ('windows', f'PoCol-Mainnet-{NATIVE}-windows-x86_64.zip'),
                               ('android', f'PoCol-Mainnet-{NATIVE}-arm64.apk')):
            entry = downloads[platform]
            self.assertEqual(entry['url'], base + name)
            self.assertEqual(entry['sig'], base + name + '.sig')
            self.assertEqual(entry['sha256'], hashlib.sha256(self.files[name]).hexdigest())
        self.assertEqual(downloads['android']['size'], '14.6 KB')
        self.assertEqual((downloads['ios'], downloads['macos']), ('', ''))
        self.assertEqual(downloads['release'], {'version': NATIVE, 'sums': base + 'SHA256SUMS', 'sumsSig': base + 'SHA256SUMS.sig'})
        sums = dict(reversed(line.split('  ', 1)) for line in (self.root / si.MAIN / si.SUMS).read_text(encoding='utf-8').splitlines())
        for name in [*self.files, 'SHA256SUMS']:
            for listed in (name, name + '.sig'):
                self.assertEqual(sums[f'downloads/{listed}'], hashlib.sha256((self.release / listed).read_bytes()).hexdigest(), listed)
        kept = json.loads((self.root / si.DECLARED).read_text(encoding='utf-8'))['keptOut']['pocol.net']
        self.assertEqual(len(kept), 2 * (len(self.files) + 1))
        self.assertEqual(si.main(['--check', '--root', str(self.root)]), 0)

    def test_the_invitation_tracked_in_the_snapshot_must_be_the_signed_one(self):
        invitation = b'{"schema": "pocol-network-v2"}\n'
        self.files['network.pocol.json'] = invitation
        (self.release / 'network.pocol.json').write_bytes(invitation)
        self.sign_all()
        tracked = self.root / si.MAIN / 'downloads' / 'network.pocol.json'
        tracked.parent.mkdir(exist_ok=True)
        tracked.write_bytes(b'{"schema": "other"}\n')
        self.assertEqual(si.main(['--root', str(self.root)]), 0)
        before = snapshot(self.root)
        rc, out = self.run_tool()
        self.assertEqual(rc, 2)
        self.assertIn("is not the signed release's network.pocol.json", out)
        self.assertEqual(snapshot(self.root), before)
        tracked.write_bytes(invitation)
        self.assertEqual(si.main(['--root', str(self.root)]), 0)
        rc, out = self.run_tool()
        self.assertEqual(rc, 0, out)
        sums = (self.root / si.MAIN / si.SUMS).read_text(encoding='utf-8')
        self.assertIn(f'{hashlib.sha256(invitation).hexdigest()}  downloads/network.pocol.json\n', sums)
        self.assertIn('downloads/network.pocol.json.sig\n', sums)
        kept = json.loads((self.root / si.DECLARED).read_text(encoding='utf-8'))['keptOut']['pocol.net']
        self.assertNotIn('downloads/network.pocol.json', kept)
        self.assertIn('downloads/network.pocol.json.sig', kept)

    def test_a_release_that_does_not_verify_changes_nothing(self):
        before = snapshot(self.root)
        name = f'PoCol-Mainnet-{NATIVE}-arm64.apk'
        (self.release / name).write_bytes(b'tampered')
        rc, out = self.run_tool()
        self.assertEqual(rc, 2)
        self.assertIn('does not verify', out)
        self.assertEqual(snapshot(self.root), before)
        # Signed with another key: refused too.
        (self.release / name).write_bytes(self.files[name])
        other = self.tmp / 'other'
        subprocess.run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-f', str(other)], check=True, timeout=60, stdin=subprocess.DEVNULL)
        self.key = other
        self.sign_all()
        rc, out = self.run_tool()
        self.assertEqual(rc, 2)
        self.assertIn('does not verify', out)
        self.assertEqual(snapshot(self.root), before)

    def test_a_file_that_is_not_a_release_name_is_refused(self):
        self.files['PoCol-Mainnet-0.4.0-Windows.zip'] = b'retired'
        (self.release / 'PoCol-Mainnet-0.4.0-Windows.zip').write_bytes(b'retired')
        self.sign_all()
        before = snapshot(self.root)
        rc, out = self.run_tool()
        self.assertEqual(rc, 2)
        self.assertIn('not release file names', out)
        self.assertEqual(snapshot(self.root), before)

    def test_no_release_can_be_named_before_the_identity_switch(self):
        shutil.copy(ROOT / si.EXTENSION_PACKAGE, self.root / si.EXTENSION_PACKAGE)
        if json.loads((ROOT / si.EXTENSION_PACKAGE).read_text(encoding='utf-8'))['version'] == NATIVE:
            self.skipTest('the repository is already switched')
        before = snapshot(self.root)
        rc, out = self.run_tool()
        self.assertEqual(rc, 2)
        self.assertIn('no release can be named yet', out)
        self.assertEqual(snapshot(self.root), before)

if __name__ == '__main__':
    unittest.main()
