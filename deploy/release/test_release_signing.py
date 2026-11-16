#!/usr/bin/env python3
"""Release signing (owner decision 2026-11-16): the verifier, the owner-run signer and the assembler, with throwaway
ssh-ed25519 keys made in temporary directories (never committed). Needs ssh-keygen (OpenSSH 8.2 or later) and bash.

    python3 -B deploy/release/test_release_signing.py -v
"""
import hashlib, json, os, re, shutil, subprocess, sys, tempfile, unittest, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import verify_release as vr  # noqa: E402
import assemble_release as ar  # noqa: E402

KEYGEN = shutil.which('ssh-keygen')
VERSION = ar.native_version()
EXT_VERSION = ar.extension_version()


def keygen(*args, **kw):
    return subprocess.run([KEYGEN, *args], check=True, capture_output=True, **kw)


class Keys:
    """Throwaway keys in a temporary directory."""
    def __init__(self, base):
        self.base = Path(base)

    def make(self, name):
        path = self.base / name
        keygen('-q', '-t', 'ed25519', '-N', '', '-C', f'{name}@test.invalid', '-f', str(path))
        return path

    def allowed(self, key, principal=vr.PRINCIPAL, namespace=vr.NAMESPACE, name='allowed_signers'):
        kind, blob = (key.with_suffix('.pub') if key.suffix != '.pub' else key).read_text().split()[:2]
        path = self.base / name
        path.write_text(f'# test\n{principal} namespaces="{namespace}" {kind} {blob}\n', encoding='ascii')
        return path


def sign(key, path, namespace=vr.NAMESPACE):
    keygen('-q', '-Y', 'sign', '-f', str(key), '-n', namespace, str(path))


class Verifier(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(KEYGEN, 'ssh-keygen (OpenSSH 8.2 or later) is required: apt-get install openssh-client')
        self.tmp = tempfile.TemporaryDirectory(prefix='pocol-release-test-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.keys = Keys(self.base)
        self.key = self.keys.make('owner')
        self.allowed = self.keys.allowed(self.key)
        self.dir = self.base / 'release'
        self.dir.mkdir()
        self.files = {f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz': b'linux package\n',
                      f'PoCol-Mainnet-{VERSION}-arm64.apk': b'PK apk bytes APK Sig Block 42 more\n'}
        for name, data in self.files.items():
            (self.dir / name).write_bytes(data)
        self.write_sums()

    def write_sums(self, rows=None):
        rows = rows if rows is not None else sorted((hashlib.sha256(d).hexdigest(), n) for n, d in self.files.items())
        rows = sorted(rows, key=lambda r: r[1])
        (self.dir / vr.SUMS).write_bytes(''.join(f'{h}  {n}\n' for h, n in rows).encode())

    def sign_all(self, key=None, namespace=vr.NAMESPACE):
        for name in list(self.files) + [vr.SUMS]:
            sig = self.dir / (name + '.sig')
            if sig.exists():
                sig.unlink()
            sign(key or self.key, self.dir / name, namespace)

    def run_verify(self, directory=None, allowed=None):
        r = subprocess.run([sys.executable, '-B', str(HERE / 'verify_release.py'), str(directory or self.dir),
                            '--allowed-signers', str(allowed or self.allowed)], capture_output=True, text=True)
        return r

    def refused(self, reason, **kw):
        r = self.run_verify(**kw)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertNotIn('\nVERIFIED', '\n' + r.stdout)
        self.assertTrue(r.stdout.startswith('NOT VERIFIED: '), r.stdout)
        self.assertIn(reason, r.stdout)

    def test_a_signed_release_verifies(self):
        self.sign_all()
        r = self.run_verify()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertTrue(r.stdout.startswith('VERIFIED: 2 files'), r.stdout)

    def test_an_unsigned_release_never_verifies(self):
        self.refused('not signed: SHA256SUMS.sig is missing')

    def test_tampering_is_refused(self):
        self.sign_all()
        name = next(iter(self.files))
        data = bytearray((self.dir / name).read_bytes()); data[0] ^= 1
        (self.dir / name).write_bytes(bytes(data))
        self.refused('SHA-256 mismatch')

    def test_an_edited_sums_line_is_refused(self):
        self.sign_all()
        text = (self.dir / vr.SUMS).read_text().replace('0', '1', 1)
        (self.dir / vr.SUMS).write_text(text)
        self.refused('SHA256SUMS.sig does not verify')

    def test_a_file_whose_sums_line_and_hash_were_both_changed_is_refused(self):
        # The SHA-256 matches again, but SHA256SUMS.sig covers the old line.
        self.sign_all()
        name = next(iter(self.files))
        self.files[name] = b'replaced\n'
        (self.dir / name).write_bytes(self.files[name])
        self.write_sums()
        self.refused('SHA256SUMS.sig does not verify')

    def test_a_missing_file_signature_is_refused(self):
        self.sign_all()
        name = sorted(self.files)[0]
        (self.dir / (name + '.sig')).unlink()
        self.refused(f'not signed: {name}.sig is missing')

    def test_another_key_or_namespace_or_principal_is_refused(self):
        other = self.keys.make('other')
        self.sign_all(key=other)
        self.refused('SHA256SUMS.sig does not verify')
        self.sign_all(namespace='other-namespace')
        self.refused('SHA256SUMS.sig does not verify')
        self.sign_all()
        self.refused('principal', allowed=self.keys.allowed(self.key, principal='someone@test.invalid', name='a2'))

    def test_a_file_signed_by_another_key_is_refused(self):
        self.sign_all()
        other = self.keys.make('other')
        name = sorted(self.files)[0]
        (self.dir / (name + '.sig')).unlink()
        sign(other, self.dir / name)
        self.refused(f'{name}.sig does not verify')

    def test_an_unlisted_file_is_refused(self):
        self.sign_all()
        (self.dir / 'extra.txt').write_bytes(b'x')
        self.refused('extra.txt is in the release directory but not in SHA256SUMS')

    def test_malformed_sums_are_refused(self):
        h = 'ab' * 32
        cases = {f'{h}  ../x\n': 'not a `<sha256>  <name>` line', f'{h}  a/b\n': 'not a `<sha256>  <name>` line',
                 f'{h} *bin\n': 'not a `<sha256>  <name>` line', f'{h}  a\r\n': 'CR line endings', '': 'empty',
                 f'{h}  b\n{h}  a\n': 'not sorted', f'{h}  a\n{h}  a\n': 'listed twice', f'{h}  a': 'newline',
                 f'{h}  a.sig\n': 'not a release file name', f'{h}  SHA256SUMS\n': 'not a release file name',
                 f'{h.upper()}  a\n': 'not a `<sha256>  <name>` line'}
        for text, reason in cases.items():
            with self.subTest(text=text):
                with self.assertRaises(vr.NotVerified) as e:
                    vr.parse_sums(text.encode())
                self.assertIn(reason, str(e.exception))

    def test_a_crlf_sums_file_is_refused_end_to_end(self):
        text = (self.dir / vr.SUMS).read_text().replace('\n', '\r\n')
        (self.dir / vr.SUMS).write_bytes(text.encode())
        self.sign_all()
        self.refused('CR line endings')

    def test_the_allowed_signers_file_is_checked(self):
        self.sign_all()
        self.refused('owner release key is not committed', allowed=HERE / 'allowed_signers')
        pub = self.key.with_suffix('.pub').read_text().split()
        cases = {
            'two': (f'{vr.PRINCIPAL} namespaces="{vr.NAMESPACE}" {pub[0]} {pub[1]}\n' * 2, 'exactly one key line'),
            'none': ('# nothing\n', 'exactly one key line'),
            'rsa': (f'{vr.PRINCIPAL} namespaces="{vr.NAMESPACE}" ssh-rsa {pub[1]}\n', 'not an ssh-ed25519'),
            'nons': (f'{vr.PRINCIPAL} {pub[0]} {pub[1]}\n', 'the key line is not'),
            'otherns': (f'{vr.PRINCIPAL} namespaces="git" {pub[0]} {pub[1]}\n', 'namespace'),
            'crlf': (f'{vr.PRINCIPAL} namespaces="{vr.NAMESPACE}" {pub[0]} {pub[1]}\r\n', 'CR line endings'),
        }
        for name, (text, reason) in cases.items():
            with self.subTest(name=name):
                path = self.base / f'allowed-{name}'
                path.write_bytes(text.encode())
                self.refused(reason, allowed=path)

    def test_the_committed_allowed_signers_is_the_marked_placeholder(self):
        text = (HERE / 'allowed_signers').read_text(encoding='ascii')
        lines = [l for l in text.splitlines() if l.strip() and not l.startswith('#')]
        self.assertEqual(lines, [f'{vr.PRINCIPAL} namespaces="{vr.NAMESPACE}" ssh-ed25519 PLACEHOLDER-OWNER-RELEASE-PUBLIC-KEY'])
        self.assertIn('OWNER INPUT', text)
        with self.assertRaises(vr.NotVerified) as e:
            vr.read_allowed(HERE / 'allowed_signers')
        self.assertIn('not committed', str(e.exception))

    def test_no_private_key_is_committed(self):
        tracked = subprocess.run(['git', 'ls-files', '-z'], cwd=ROOT, capture_output=True, check=True).stdout.split(b'\0')
        for rel in filter(None, tracked):
            path = ROOT / rel.decode()
            if path.is_file() and path.stat().st_size < 1 << 20 and path.suffix not in ('.py', '.md', '.sh', '.ps1'):
                self.assertNotIn(b'-----BEGIN OPENSSH PRIVATE KEY-----', path.read_bytes(), rel.decode())

    def test_the_version_probe_needs_y_verify(self):
        vr.probe(KEYGEN)
        fake = self.base / 'old-ssh-keygen'
        fake.write_text('#!/bin/sh\necho "unknown option -- Y" >&2\nexit 1\n')
        fake.chmod(0o755)
        with self.assertRaises(vr.NotVerified) as e:
            vr.probe(str(fake))
        self.assertIn('OpenSSH 8.2', str(e.exception))
        # A tool that accepts everything is refused too: the probe also checks a changed message.
        yes = self.base / 'yes-ssh-keygen'
        yes.write_text('#!/bin/sh\ncat >/dev/null\nexit 0\n')
        yes.chmod(0o755)
        with self.assertRaises(vr.NotVerified):
            vr.probe(str(yes))
        self.sign_all()
        r = subprocess.run([sys.executable, '-B', str(HERE / 'verify_release.py'), str(self.dir), '--allowed-signers',
                            str(self.allowed), '--ssh-keygen', str(yes)], capture_output=True, text=True)
        self.assertEqual(r.returncode, 1)
        self.assertIn('NOT VERIFIED', r.stdout)


class Signer(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(KEYGEN, 'ssh-keygen is required')
        self.tmp = tempfile.TemporaryDirectory(prefix='pocol-release-sign-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.keys = Keys(self.base)
        self.key = self.keys.make('owner')
        self.allowed = self.keys.allowed(self.key)
        self.dir = self.base / 'release'
        self.dir.mkdir()
        for name in (f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz', 'network.pocol.json'):
            (self.dir / name).write_bytes(name.encode() + b'\n')
        (self.dir / 'SHA256SUMS').write_bytes(''.join(
            f'{hashlib.sha256((self.dir / n).read_bytes()).hexdigest()}  {n}\n' for n in sorted(os.listdir(self.dir))).encode())

    def sign(self, key=None, allowed=None):
        return subprocess.run(['bash', str(HERE / 'sign-release.sh'), '--key', str(key or self.key),
                               '--allowed-signers', str(allowed or self.allowed), str(self.dir)], capture_output=True, text=True)

    def test_sign_then_verify(self):
        r = self.sign()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn('VERIFIED: 2 files', r.stdout)
        self.assertIn('ED25519', r.stdout)
        self.assertEqual(sorted(p.name for p in self.dir.iterdir() if p.suffix == '.sig'),
                         sorted(['SHA256SUMS.sig', 'network.pocol.json.sig', f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz.sig']))
        self.assertEqual(vr.verify(self.dir, self.allowed), 2)
        # A second run never overwrites a signature.
        again = self.sign()
        self.assertEqual(again.returncode, 1)
        self.assertIn('never overwrite a signature', again.stderr)

    def test_the_signer_refuses_before_writing(self):
        def no_sig():
            self.assertEqual([p.name for p in self.dir.iterdir() if p.suffix == '.sig'], [])
        # The committed placeholder, or a key that is not the committed one.
        r = self.sign(allowed=HERE / 'allowed_signers')
        self.assertEqual(r.returncode, 1); self.assertIn('not the one in', r.stderr); no_sig()
        other = self.keys.make('other')
        r = self.sign(key=other)
        self.assertEqual(r.returncode, 1); self.assertIn('not the one in', r.stderr); no_sig()
        # A key inside the repository.
        inside = HERE / 'testdata' / 'tmp-test-key'
        shutil.copyfile(self.key, inside); shutil.copyfile(self.key.with_suffix('.pub'), inside.with_name(inside.name + '.pub'))
        try:
            r = self.sign(key=inside)
        finally:
            inside.unlink(); inside.with_name(inside.name + '.pub').unlink()
        self.assertEqual(r.returncode, 1); self.assertIn('inside the repository', r.stderr); no_sig()
        # SHA256SUMS that does not match, and an unlisted file.
        name = f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz'
        saved = (self.dir / name).read_bytes()
        (self.dir / name).write_bytes(b'changed\n')
        r = self.sign()
        self.assertEqual(r.returncode, 1); self.assertIn('does not match', r.stderr); no_sig()
        (self.dir / name).write_bytes(saved)
        (self.dir / 'extra.txt').write_bytes(b'x')
        r = self.sign()
        self.assertEqual(r.returncode, 1); self.assertIn('not listed', r.stderr); no_sig()


class Assembler(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='pocol-release-assemble-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)

    def file(self, name, data=b'x\n'):
        path = self.base / 'in' / name
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(data)
        return path

    def extension(self, name=None, version=None):
        self.count = getattr(self, 'count', 0) + 1
        path = self.base / f'ext{self.count}' / f'PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip'
        path.parent.mkdir()
        with zipfile.ZipFile(path, 'w') as z:
            z.writestr('manifest.json', json.dumps({'name': name or ar.release_name(), 'version': version or EXT_VERSION}))
        return path

    def test_a_release_set_is_assembled_and_signed(self):
        files = [self.file(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz'), self.file(f'PoCol-Mainnet-{VERSION}-windows-x86_64.zip'),
                 self.file(f'PoCol-Mainnet-{VERSION}-arm64.apk', b'PK\x03\x04 ... APK Sig Block 42 ...'), self.extension(),
                 self.file('network.pocol.json', b'{"schema": "pocol-network-v2"}\n')]
        out = self.base / 'out'
        self.assertEqual(ar.main(['--out', str(out), *map(str, files)]), 0)
        rows = vr.parse_sums((out / 'SHA256SUMS').read_bytes())
        self.assertEqual([n for _, n in rows], sorted(f.name for f in files))
        for digest, name in rows:
            self.assertEqual(hashlib.sha256((out / name).read_bytes()).hexdigest(), digest)
        if KEYGEN:
            keys = Keys(self.base)
            key = keys.make('owner')
            allowed = keys.allowed(key)
            with self.assertRaises(vr.NotVerified) as e:
                vr.verify(out, allowed)
            self.assertIn('not signed', str(e.exception))
            r = subprocess.run(['bash', str(HERE / 'sign-release.sh'), '--key', str(key), '--allowed-signers', str(allowed), str(out)],
                               capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(vr.verify(out, allowed), 5)

    def test_refusals(self):
        apk = f'PoCol-Mainnet-{VERSION}-arm64.apk'
        cases = [
            ([self.file(f'PoCol-Mainnet-{VERSION}-arm64-unsigned.apk', b'PK APK Sig Block 42')], 'unsigned APK'),
            ([self.file(apk, b'PK no signing block')], 'no APK Signing Block'),
            ([self.file('PoCol-Mainnet-0.0.1-linux-x86_64.tar.gz')], 'not a release file name'),
            ([self.file('notes.txt')], 'not a release file name'),
            ([self.extension(name='[REHEARSAL] PoCol Studio, IDE & Viewer')], 'not the release build'),
            ([self.extension(version='9.9.9')], "not package.json's"),
        ]
        dup = self.file(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz')
        cases.append(([dup, dup], 'given twice'))
        for files, reason in cases:
            with self.subTest(reason=reason):
                out = self.base / ('out-' + re.sub(r'\W', '', reason))
                with self.assertRaises(ar.Refused) as e:
                    ar.assemble(out, files)
                self.assertIn(reason, str(e.exception))
                self.assertFalse(out.exists())
        busy = self.base / 'busy'; busy.mkdir(); (busy / 'x').write_bytes(b'x')
        with self.assertRaises(ar.Refused):
            ar.assemble(busy, [dup])


class Consistency(unittest.TestCase):
    """The PowerShell twins (run on Windows CI by test_verify_release.ps1) keep the Python rules."""
    def test_the_powershell_verifier_keeps_the_rules(self):
        ps = (HERE / 'Verify-PoColRelease.ps1').read_text(encoding='ascii')
        for text in ("$Namespace = 'pocol-release'", "[string]$Principal = 'release@pocol.net'",
                     r"'\A([0-9a-f]{64})  ([A-Za-z0-9._-]+)\z'", '-RedirectStandardInput $Message',
                     "Contains('PLACEHOLDER')", "$Sums has CR line endings (LF only)", 'not signed: ',
                     "is in the release directory but not in $Sums", 'OpenSSH 8.2', r'\AAAAAC3NzaC1lZDI1NTE5[A-Za-z0-9+/]{48}\z',
                     "Write-Output \"VERIFIED: "):
            self.assertIn(text, ps)
        self.assertEqual(vr.LINE.pattern, r'([0-9a-f]{64})  ([A-Za-z0-9._-]+)')
        # No PowerShell pipe feeds ssh-keygen.
        self.assertNotRegex(ps, r'\|\s*&\s*\$SshKeygen')
        self.assertEqual(ps.count('Write-Output "VERIFIED: '), 1, 'one success line, after every check')
        self.assertLess(ps.index("Fail \"$($Row[1]).sig does not verify"), ps.index('Write-Output "VERIFIED: '))
        sign = (HERE / 'Sign-PoColRelease.ps1').read_text(encoding='ascii')
        for text in ("$Namespace = 'pocol-release'", 'inside the repository', 'never overwrite a signature',
                     "-Y sign -f $Key -n $Namespace", 'Verify-PoColRelease.ps1', "-lf $Pub"):
            self.assertIn(text, sign)
        shell = (HERE / 'sign-release.sh').read_text(encoding='ascii')
        for text in ('NAMESPACE=pocol-release', '-Y sign -f "$KEY_ABS" -n "$NAMESPACE"', 'inside the repository',
                     'never overwrite a signature', 'verify_release.py'):
            self.assertIn(text, shell)

    def test_the_readme_commands_use_the_tool_constants(self):
        readme = (HERE / 'README.md').read_text(encoding='utf-8')
        self.assertIn(f'ssh-keygen -Y verify -f allowed_signers -I {vr.PRINCIPAL} -n {vr.NAMESPACE} -s FILE.sig < FILE', readme)
        self.assertIn('ssh-keygen -t ed25519 -C release@pocol.net', readme)
        self.assertIn('integrity', readme)
        for name in ('assemble_release.py', 'sign-release.sh', 'Sign-PoColRelease.ps1', 'verify_release.py', 'Verify-PoColRelease.ps1'):
            self.assertIn(name, readme)
            self.assertTrue((HERE / name).is_file(), name)
        for other in (ROOT / 'deploy/standalone/README.md', ROOT / 'prototypes/pocol-extension/README-IDE.md'):
            self.assertIn(f'-I {vr.PRINCIPAL} -n {vr.NAMESPACE}', other.read_text(encoding='utf-8'), other)

    def test_the_scripts_are_ascii(self):
        for name in ('Verify-PoColRelease.ps1', 'Sign-PoColRelease.ps1', 'test_verify_release.ps1', 'sign-release.sh'):
            (HERE / name).read_bytes().decode('ascii')


if __name__ == '__main__':
    unittest.main()
