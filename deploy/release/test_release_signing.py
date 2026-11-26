#!/usr/bin/env python3
"""Release signing (owner decision 2026-11-16): the verifier, the owner-run signer and the assembler, with throwaway
ssh-ed25519 keys made in temporary directories (never committed). Needs ssh-keygen (OpenSSH 8.2 or later) and bash.

    python3 -B deploy/release/test_release_signing.py -v
"""
import hashlib, io, json, os, re, shutil, struct, subprocess, sys, tempfile, unittest, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import verify_release as vr  # noqa: E402
# The owner's release key (allowed_signers, committed 2026-11-18) and its published fingerprint.
OWNER_RELEASE_KEY = 'AAAAC3NzaC1lZDI1NTE5AAAAICfqlk8IMpc0B0XDYNM2k2OrbeZuLehFEz0wOXplO82c'
OWNER_RELEASE_FINGERPRINT = 'SHA256:r/IMxkMr+wv3OyAbiX3XYhSV4pqy3Lv5/z/VV06DJyc'
import assemble_release as ar  # noqa: E402

KEYGEN = shutil.which('ssh-keygen')
VERSION = ar.native_version()
# The documented key-line count of a downloaded allowed_signers (README.md, user procedure step 1): must print 1.
KEY_COUNT = "LC_ALL=C grep -Ecv '^[[:blank:]]*(#|$)' allowed_signers"
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


def key_line(key, principal=vr.PRINCIPAL):
    kind, blob = key.with_suffix('.pub').read_text().split()[:2]
    return f'{principal} namespaces="{vr.NAMESPACE}" {kind} {blob}'


def apk(pairs=((0x7109871a, b'v2 signer'),), block=True, entries=(('AndroidManifest.xml', b'manifest'), ('classes.dex', b'dex'))):
    """A ZIP with an APK Signing Block of these ID-value pairs inserted just before its central directory (the layout
    apksigner writes), or a plain ZIP (block=False)."""
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w') as z:
        for name, data in entries:
            z.writestr(name, data)
    data = buf.getvalue()
    if not block:
        return data
    eocd = data.rfind(b'PK\x05\x06')
    cd_size, cd_offset = struct.unpack_from('<II', data, eocd + 12)
    body = b''.join(struct.pack('<QI', 4 + len(v), i) + v for i, v in pairs)
    size = len(body) + 8 + 16
    signing = struct.pack('<Q', size) + body + struct.pack('<Q', size) + ar.APK_SIG_BLOCK
    return (data[:cd_offset] + signing + data[cd_offset:eocd + 16] + struct.pack('<I', cd_offset + len(signing))
            + data[eocd + 20:])


def fake_root(base, ext_version=VERSION, native=VERSION):
    """A temporary tree with only the files assemble_release reads, the committed network files copied."""
    root = Path(base)
    files = {'deploy/standalone/package.py': f"version='{native}'\n",
             'prototypes/pocol-extension/package.json': json.dumps({'version': ext_version}),
             'prototypes/pocol-extension/scripts/package-release.py': f"RELEASE_NAME = '{ar.release_name()}'\n",
             'prototypes/pocold/src/identity.rs': f'pub const CONSENSUS_RULES_VERSION: u32 = {ar.consensus_rules_version()};\n'}
    for rel, text in files.items():
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        (root / rel).write_text(text, encoding='utf-8')
    for rel in ('deploy/mainnet/placeholder-network.pocol.json', 'deploy/mainnet/retired-networks.json',
                'deploy/mainnet/v3-profile.json', 'deploy/public-testnet/pocol_identity.py'):
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / rel, root / rel)
    return root


MAINNET_CHAIN = json.loads((ROOT / 'deploy/mainnet/v3-profile.json').read_text(encoding='utf-8'))['chainId']
REHEARSAL_CHAIN = json.loads((ROOT / 'deploy/mainnet/v3-rehearsal-profile.json').read_text(encoding='utf-8'))['chainId']
RETIRED = json.loads((ROOT / 'deploy/mainnet/retired-networks.json').read_text(encoding='utf-8'))['retired'][0]


def invitation(chain=MAINNET_CHAIN, genesis='0x' + 'ab' * 32, schema='pocol-network-v2', rules=None, spec=2):
    info = {'chainId': chain, 'genesisHash': genesis}
    if spec == 2:
        info.update(specVersion=2, v3=True)
    doc = {'schema': schema, 'consensusRulesVersion': ar.consensus_rules_version() if rules is None else rules, 'seeds': [],
           'files': {'genesis.spec.hex': '00', 'devnet.json': json.dumps(info)}}
    return (json.dumps(doc, indent=2) + '\n').encode()


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
        # The committed owner key did not sign this throwaway release; a file that still holds the placeholder line
        # verifies nothing at all.
        self.refused('SHA256SUMS.sig does not verify', allowed=HERE / 'allowed_signers')
        placeholder = self.base / 'placeholder_allowed_signers'
        placeholder.write_text(f'{vr.PRINCIPAL} namespaces="{vr.NAMESPACE}" ssh-ed25519 PLACEHOLDER-OWNER-RELEASE-PUBLIC-KEY\n')
        self.refused('owner release key is not committed', allowed=placeholder)
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

    def test_a_key_line_hidden_behind_unicode_or_control_whitespace_is_refused(self):
        # ssh-keygen skips only spaces and tabs before '#': '\v#x,release@pocol.net ...' is a key line to it (its
        # principal pattern list matches), while str.strip / .NET Trim would make it a comment. A release signed by
        # that hidden key must never verify, and every byte outside printable ASCII and LF is refused.
        evil = self.keys.make('evil')
        self.sign_all(key=evil)
        good = key_line(self.key)
        for prefix in (b'\x0b', b'\x0c', b'\x1c', b'\x85', b'\xc2\x85', b'\xc2\xa0', b'\xef\xbb\xbf', b'\t', b'\x00'):
            with self.subTest(prefix=prefix):
                path = self.base / f'allowed-hidden-{prefix.hex()}'
                hidden = prefix + b'#example,' + key_line(evil).encode()
                path.write_bytes(b'# release key\n' + hidden + b'\n' + good.encode() + b'\n')
                self.refused('not printable ASCII', allowed=path)
                with self.assertRaises(vr.NotVerified):
                    vr.read_allowed(path)
        # Spaces before a comment stay a comment for both readers.
        path = self.base / 'allowed-spaces'
        path.write_text(f'   # a comment\n{good}\n', encoding='ascii')
        self.assertEqual(vr.read_allowed(path)[2], good.split()[3])

    def test_verified_names_the_key_fingerprint(self):
        self.sign_all()
        r = self.run_verify()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        printed = keygen('-lf', str(self.key.with_suffix('.pub')), text=True).stdout.split()[1]
        self.assertTrue(printed.startswith('SHA256:'), printed)
        self.assertEqual(vr.fingerprint(vr.read_allowed(self.allowed)[2]), printed)
        self.assertIn(f'key {printed} (ED25519)', r.stdout.splitlines()[0])
        self.assertIn('published on pocol.net', r.stdout)
        # The documented fingerprint command for an allowed_signers file prints that one fingerprint.
        r = subprocess.run(f"grep -v '^ *#' '{self.allowed}' | cut -d' ' -f3,4 | '{KEYGEN}' -lf -", shell=True,
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual([l.split()[1] for l in r.stdout.splitlines()], [printed])

    def test_the_documented_key_check_sees_a_second_key_line(self):
        # The fingerprint command alone prints one correct line for a file that also holds a key line it cannot cut;
        # ssh-keygen accepts that second key. The documented count is 1 only for a one-key file.
        readme = (HERE / 'README.md').read_text(encoding='utf-8')
        standalone = (ROOT / 'deploy/standalone/README.md').read_text(encoding='utf-8')
        self.assertIn(KEY_COUNT, readme)
        self.assertIn(KEY_COUNT, standalone)
        good = key_line(self.key)
        evil = self.keys.make('evil')
        kind, blob = evil.with_suffix('.pub').read_text().split()[:2]
        printed = keygen('-lf', str(self.key.with_suffix('.pub')), text=True).stdout.split()[1]
        def documented(text):
            work = self.base / 'check'
            shutil.rmtree(work, ignore_errors=True)
            work.mkdir()
            (work / 'allowed_signers').write_text(text, encoding='utf-8')
            # A UTF-8 locale, as a user's terminal has: the documented LC_ALL=C must still count a Unicode-space line.
            env = dict(os.environ, LANG='C.UTF-8', LC_CTYPE='C.UTF-8')
            count = subprocess.run(KEY_COUNT, shell=True, cwd=work, capture_output=True, text=True, env=env).stdout.strip()
            fp = subprocess.run(f"grep -v '^ *#' allowed_signers | cut -d' ' -f3,4 | '{KEYGEN}' -lf -", shell=True, cwd=work,
                                capture_output=True, text=True).stdout
            return count, [l.split()[1] for l in fp.splitlines()]
        self.assertEqual(documented(f'# the release key\n\n{good}\n'), ('1', [printed]))
        for extra in (f'{vr.PRINCIPAL} {kind} {blob}', f'{vr.PRINCIPAL}  namespaces="{vr.NAMESPACE}" {kind} {blob}',
                      f'{vr.PRINCIPAL}\tnamespaces="{vr.NAMESPACE}" {kind} {blob}', f'\x0b#x,{vr.PRINCIPAL} {kind} {blob}',
                      f'\u2000#x,{vr.PRINCIPAL} {kind} {blob}'):
            with self.subTest(extra=extra):
                count, fingerprints = documented(f'{good}\n{extra}\n')
                self.assertEqual(count, '2')
                self.assertNotIn(vr.fingerprint(blob), fingerprints)

    def test_the_documented_one_download_check_binds_the_name(self):
        # A FILE.sig alone binds no name: an older signed file under a newer name keeps a good signature. The
        # documented check (SHA256SUMS.sig, then the file's own line) refuses it.
        self.sign_all()
        new = self.dir / f'PoCol-Mainnet-{VERSION}9-linux-x86_64.tar.gz'
        shutil.copyfile(self.dir / f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz', new)
        shutil.copyfile(self.dir / f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz.sig', new.with_name(new.name + '.sig'))
        alone = subprocess.run([KEYGEN, '-Y', 'verify', '-f', str(self.allowed), '-I', vr.PRINCIPAL, '-n', vr.NAMESPACE,
                                '-s', str(new) + '.sig'], input=new.read_bytes(), capture_output=True)
        self.assertEqual(alone.returncode, 0, 'the per-file signature binds the bytes only')
        def documented(name):
            sums = subprocess.run([KEYGEN, '-Y', 'verify', '-f', str(self.allowed), '-I', vr.PRINCIPAL, '-n', vr.NAMESPACE,
                                   '-s', 'SHA256SUMS.sig'], input=(self.dir / 'SHA256SUMS').read_bytes(), cwd=self.dir,
                                  capture_output=True)
            self.assertEqual(sums.returncode, 0, sums.stderr)
            self.assertIn(f'with ED25519 key {vr.fingerprint(vr.read_allowed(self.allowed)[2])}'.encode(), sums.stdout + sums.stderr)
            return subprocess.run(f"awk -v f={name} '$2 == f' SHA256SUMS | sha256sum --check", shell=True, cwd=self.dir,
                                  capture_output=True, text=True)
        ok = documented(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz')
        self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)
        self.assertIn(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz: OK', ok.stdout)
        self.assertNotEqual(documented(new.name).returncode, 0, 'a name SHA256SUMS does not list never checks')

    def test_the_committed_allowed_signers_is_the_owner_release_key(self):
        # The owner's release key (committed 2026-11-18): exactly this one line, and the fingerprint published on
        # pocol.net and in README.md is the one ssh-keygen -lf prints for it.
        text = (HERE / 'allowed_signers').read_text(encoding='ascii')
        lines = [l for l in text.splitlines() if l.strip() and not l.startswith('#')]
        self.assertEqual(lines, [f'{vr.PRINCIPAL} namespaces="{vr.NAMESPACE}" ssh-ed25519 {OWNER_RELEASE_KEY}'])
        self.assertNotIn('PLACEHOLDER', text)
        principal, kind, key = vr.read_allowed(HERE / 'allowed_signers')[:3]
        self.assertEqual((principal, kind, key), (vr.PRINCIPAL, 'ssh-ed25519', OWNER_RELEASE_KEY))
        self.assertEqual(vr.fingerprint(key), OWNER_RELEASE_FINGERPRINT)
        for doc in (HERE / 'allowed_signers', HERE / 'README.md'):
            self.assertIn(OWNER_RELEASE_FINGERPRINT, doc.read_text(encoding='utf-8'), doc)
        # A release signed by any other key (here the test's throwaway key) never verifies against it.
        self.sign_all()
        self.refused('SHA256SUMS.sig does not verify', allowed=HERE / 'allowed_signers')

    def test_a_placeholder_allowed_signers_verifies_nothing(self):
        # The refusal stays for any file that still holds the placeholder line (an old checkout, a test copy).
        path = self.base / 'placeholder_allowed_signers'
        path.write_text(f'{vr.PRINCIPAL} namespaces="{vr.NAMESPACE}" ssh-ed25519 PLACEHOLDER-OWNER-RELEASE-PUBLIC-KEY\n',
                        encoding='ascii')
        with self.assertRaises(vr.NotVerified) as e:
            vr.read_allowed(path)
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
        (self.dir / 'extra.txt').unlink()
        # Hidden entries count: Finder litter, a hidden signature.
        (self.dir / '.DS_Store').write_bytes(b'x')
        r = self.sign()
        self.assertEqual(r.returncode, 1); self.assertIn('.DS_Store is not listed', r.stderr); no_sig()
        (self.dir / '.DS_Store').unlink()
        (self.dir / '.x.sig').write_bytes(b'x')
        r = self.sign()
        self.assertEqual(r.returncode, 1); self.assertIn('never overwrite a signature', r.stderr)
        (self.dir / '.x.sig').unlink(); no_sig()
        # An allowed_signers with a key line hidden behind a VT.
        hidden = self.base / 'allowed-hidden'
        hidden.write_bytes(b'\x0b#x,' + key_line(other).encode() + b'\n' + self.allowed.read_bytes())
        r = self.sign(allowed=hidden)
        self.assertEqual(r.returncode, 1); self.assertIn('not printable ASCII', r.stderr); no_sig()

    def test_the_signer_needs_no_bash_4_or_gnu_tools(self):
        # Stock macOS: /bin/bash 3.2 (no mapfile or readarray) and no GNU sha256sum. Run with those builtins disabled
        # and a PATH holding only ssh-keygen, python3, dirname and basename.
        bin_dir = self.base / 'bin'
        bin_dir.mkdir()
        for tool in ('ssh-keygen', 'python3', 'dirname', 'basename'):
            (bin_dir / tool).symlink_to(shutil.which(tool))
        bash = shutil.which('bash')
        r = subprocess.run([bash, '-c', 'enable -n mapfile readarray; . "$0" "$@"', str(HERE / 'sign-release.sh'),
                            '--key', str(self.key), '--allowed-signers', str(self.allowed), str(self.dir)],
                           env={'PATH': str(bin_dir), 'HOME': str(self.base)}, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn('VERIFIED: 2 files', r.stdout)
        self.assertEqual(vr.verify(self.dir, self.allowed), 2)


class Assembler(unittest.TestCase):
    """Against a temporary tree whose extension version is the native one, as after the identity switch."""
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='pocol-release-assemble-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.root = fake_root(self.base / 'root')

    def file(self, name, data=b'x\n'):
        self.count = getattr(self, 'count', 0) + 1
        path = self.base / f'in{self.count}' / name
        path.parent.mkdir()
        path.write_bytes(data)
        return path

    def extension(self, name=None, version=None, members=()):
        self.count = getattr(self, 'count', 0) + 1
        path = self.base / f'ext{self.count}' / f'PoCol-Studio-IDE-Viewer-{VERSION}.zip'
        path.parent.mkdir()
        with zipfile.ZipFile(path, 'w') as z:
            z.writestr('manifest.json', json.dumps({'name': name or ar.release_name(), 'version': version or VERSION}))
            for member, data in members:
                z.writestr(member, data)
        return path

    def test_an_extension_package_from_a_crlf_checkout_is_refused(self):
        # The first 0.5.0 package published (f947c019...) was the CI build with CRLF in 52 text files.
        for member in ('sw.js', 'LICENSE', 'tools/Export-Studio-Wallet.ps1', 'lib/rlp.js'):
            with self.subTest(member=member):
                ext = self.extension(members=[(member, b'export const a = 1;\r\nexport const b = 2;\r\n')])
                self.refuses([ext], f'have carriage returns (a CRLF checkout), e.g. {member}')
        # A lone CR is refused too; a 0x0D byte in a PNG icon is binary, not a line ending.
        self.refuses([self.extension(members=[('lib/rlp.js', b'x\r')])], 'have carriage returns')
        names, _ = self.assemble([self.extension(members=[('icons/pocol-16.png', b'\x89PNG\r\n\x1a\n'), ('sw.js', b'ok\n')])])
        self.assertEqual(names, [f'PoCol-Studio-IDE-Viewer-{VERSION}.zip'])

    def assemble(self, files, root=None):
        out = self.base / f'out{getattr(self, "count", 0)}-{len(files)}'
        return ar.assemble(out, files, root or self.root), out

    def refuses(self, files, reason, root=None):
        out = self.base / 'refused-out'
        with self.assertRaises(ar.Refused) as e:
            ar.assemble(out, files, root or self.root)
        self.assertIn(reason, str(e.exception))
        self.assertFalse(out.exists())

    def test_a_release_set_is_assembled_and_signed(self):
        files = [self.file(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz'), self.file(f'PoCol-Mainnet-{VERSION}-windows-x86_64.zip'),
                 self.file(f'PoCol-Mainnet-{VERSION}-arm64.apk', apk()), self.extension(),
                 self.file('network.pocol.json', invitation())]
        names, out = self.assemble(files)
        self.assertEqual(names, sorted(f.name for f in files))
        rows = vr.parse_sums((out / 'SHA256SUMS').read_bytes())
        self.assertEqual([n for _, n in rows], sorted(f.name for f in files))
        for digest, name in rows:
            self.assertEqual(hashlib.sha256((out / name).read_bytes()).hexdigest(), digest)
        self.assertIsNotNone(KEYGEN, 'ssh-keygen is required')
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

    def test_the_extension_version_must_be_the_native_version(self):
        linux = self.file(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz')
        for other in ('0.4.1', '0.5.1', VERSION + '.1'):
            with self.subTest(extension=other):
                root = fake_root(self.base / f'root-{other}', ext_version=other)
                # Whatever the set: a native-only set is refused too, and so is the old extension's own zip name.
                self.refuses([linux], 'is not the native release version', root)
                old = self.file(f'PoCol-Studio-IDE-Viewer-{other}.zip')
                self.refuses([old], 'is not the native release version', root)
        names, _ = self.assemble([linux, self.extension()])
        self.assertEqual(names, sorted([linux.name, f'PoCol-Studio-IDE-Viewer-{VERSION}.zip']))
        self.assertIn(f'PoCol-Studio-IDE-Viewer-{VERSION}.zip', ar.allowed_names(self.root))

    def test_the_real_tree_assembles_only_with_matching_versions(self):
        # Before the identity switch package.json still says 0.4.1: nothing is assembled from this tree, not even the
        # retired network's published 0.4.1 extension beside the 0.5.0 native files.
        if EXT_VERSION != VERSION:
            with self.assertRaises(ar.Refused) as e:
                ar.allowed_names()
            self.assertIn('is not the native release version', str(e.exception))
            ext = self.base / 'retired' / f'PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip'
            ext.parent.mkdir()
            with zipfile.ZipFile(ext, 'w') as z:
                z.writestr('manifest.json', json.dumps({'name': ar.release_name(), 'version': EXT_VERSION}))
            r = subprocess.run([sys.executable, '-B', str(HERE / 'assemble_release.py'), '--out', str(self.base / 'o'), str(ext),
                                str(self.file(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz'))], capture_output=True, text=True)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn('REFUSED', r.stderr)
            self.assertFalse((self.base / 'o').exists())
        else:
            self.assertIn(f'PoCol-Studio-IDE-Viewer-{VERSION}.zip', ar.allowed_names())

    def test_the_apk_signing_block_must_be_at_its_place(self):
        name = f'PoCol-Mainnet-{VERSION}-arm64.apk'
        signed = apk()
        with zipfile.ZipFile(io.BytesIO(signed)) as z:
            self.assertEqual(z.read('classes.dex'), b'dex')
        self.assertEqual(self.assemble([self.file(name, signed)])[0], [name])
        self.assertEqual(self.assemble([self.file(name, apk(((0xf05368c0, b'v3'), (0x42726577, b'\0' * 8))))])[0], [name])
        # The magic inside an entry (the reported false accept), a plain ZIP, a block of padding only, inconsistent
        # size fields, and bytes that are not a ZIP.
        in_entry = apk(block=False, entries=(('res/raw/note.txt', ar.APK_SIG_BLOCK),))
        broken = bytearray(signed)
        eocd = broken.rfind(b'PK\x05\x06')
        cd_offset = struct.unpack_from('<I', broken, eocd + 16)[0]
        size = struct.unpack_from('<Q', broken, cd_offset - 24)[0]
        struct.pack_into('<Q', broken, cd_offset - 8 - size, size + 1)
        moved = bytearray(signed)
        struct.pack_into('<I', moved, eocd + 16, cd_offset + 1)
        for data, what in ((in_entry, 'magic in an entry'), (apk(block=False), 'plain zip'),
                           (apk(((0x42726577, b'\0' * 8),)), 'padding only'), (bytes(broken), 'size fields'),
                           (bytes(moved), 'central directory offset'), (b'PK no signing block APK Sig Block 42', 'not a zip')):
            with self.subTest(what=what):
                self.refuses([self.file(name, data)], 'no APK Signing Block')

    def test_the_invitation_must_be_the_mainnet_one(self):
        self.assertEqual(self.assemble([self.file('network.pocol.json', invitation())])[0], ['network.pocol.json'])
        placeholder = (ROOT / 'deploy/mainnet/placeholder-network.pocol.json').read_bytes()
        cases = [
            (placeholder, 'the network placeholder'),
            (invitation(chain=REHEARSAL_CHAIN), 'not the mainnet'),
            (invitation(chain=RETIRED['chainId']), 'retired network'),
            (invitation(genesis=RETIRED['genesisHash']), 'retired network'),
            (invitation(spec=1), 'spec-1 network'),
            (invitation(schema='pocol-network-v1'), 'other consensus rules'),
            (invitation(rules=ar.consensus_rules_version() + 1), 'other consensus rules'),
            (invitation(genesis='0x12'), 'no chain ID and genesis hash'),
            (b'{"schema": "pocol-network-v2", "note": "any JSON"}\n', 'not a network invitation'),
            (b'not json', 'not a network invitation'),
        ]
        for data, reason in cases:
            with self.subTest(reason=reason):
                self.refuses([self.file('network.pocol.json', data)], reason)

    def test_refusals(self):
        apk_name = f'PoCol-Mainnet-{VERSION}-arm64.apk'
        cases = [
            ([self.file(f'PoCol-Mainnet-{VERSION}-arm64-unsigned.apk', apk())], 'unsigned APK'),
            ([self.file(apk_name, b'PK no signing block')], 'no APK Signing Block'),
            ([self.file('PoCol-Mainnet-0.0.1-linux-x86_64.tar.gz')], 'not a release file name'),
            ([self.file('notes.txt')], 'not a release file name'),
            ([self.extension(name='[REHEARSAL] PoCol Studio, IDE & Viewer')], 'not the release build'),
            ([self.extension(version='9.9.9')], "not package.json's"),
        ]
        dup = self.file(f'PoCol-Mainnet-{VERSION}-linux-x86_64.tar.gz')
        cases.append(([dup, dup], 'given twice'))
        for files, reason in cases:
            with self.subTest(reason=reason):
                self.refuses(files, reason)
        busy = self.base / 'busy'; busy.mkdir(); (busy / 'x').write_bytes(b'x')
        with self.assertRaises(ar.Refused):
            ar.assemble(busy, [dup], self.root)


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
                     'never overwrite a signature', 'verify_release.py', 'os.listdir'):
            self.assertIn(text, shell)
        # Portable to stock macOS (bash 3.2, no GNU coreutils), and no glob that skips hidden entries.
        for text in ('mapfile', 'readarray', 'sha256sum', 'compgen', 'declare -A', 'for entry in *'):
            self.assertNotIn(text, shell)
        # Both PowerShell twins read allowed_signers as bytes, printable ASCII and LF only, and trim spaces only.
        for script in (ps, sign):
            self.assertIn('[IO.File]::ReadAllBytes($AllowedSigners)', script)
            self.assertIn('$Byte -lt 32 -or $Byte -gt 126', script)
            self.assertIn('.TrimStart([char]32).StartsWith', script)
            self.assertNotRegex(script, r'(\$_|KeyLines\[0\])\.Trim(Start)?\(\)')
        self.assertIn('key $Fingerprint (ED25519)', ps)

    def test_the_powershell_signer_checks_strictly_before_it_signs(self):
        # Verify-PoColRelease.ps1's SHA256SUMS rules run before the first signature, as sign-release.sh runs parse_sums:
        # otherwise an unsorted or duplicate list is signed, then refused, and its .sig files stay behind.
        sign = (HERE / 'Sign-PoColRelease.ps1').read_text(encoding='ascii')
        first_signature = sign.index('-Y sign -f $Key')
        for text in ("$Name.StartsWith('.') -or $Name -ceq $Sums -or $Name.EndsWith('.sig')",
                     'Sort-Object -Unique -CaseSensitive', '[Array]::Sort($Sorted, [StringComparer]::Ordinal)',
                     'the names are not sorted', 'a name is listed twice', '[IO.FileAttributes]::ReparsePoint',
                     'is not listed in SHA256SUMS', 'SHA256SUMS does not match the files'):
            self.assertIn(text, sign)
            self.assertLess(sign.index(text), first_signature, text)

    def test_the_powershell_test_can_pass_on_windows(self):
        # test_verify_release.ps1 runs on Windows CI only (no PowerShell here); these are the traps it must avoid.
        test = (HERE / 'test_verify_release.ps1').read_text(encoding='ascii')
        # Sort-Object compares by culture and ignores case: network.pocol.json would precede PoCol-..., which the
        # verifiers (ordinal, like Python sorted) refuse as unsorted.
        self.assertNotIn('Sort-Object Name', test)
        self.assertIn('[Array]::Sort($Names, [StringComparer]::Ordinal)', test)
        names = ['PoCol-Mainnet-test-windows-x86_64.zip', 'network.pocol.json']
        self.assertNotEqual(sorted(names), sorted(names, key=str.lower), 'the fixture names differ in the two orders')
        # Assert takes a [bool]: a -match over the verifier's two output lines is an array, which does not bind.
        self.assertNotRegex(test, r"Assert \(\(& [^\n]*\) -match ")
        # GitHub's powershell step exits with $LASTEXITCODE, which the last refusing verifier run left at 1.
        self.assertEqual(test.rstrip().splitlines()[-1], 'exit 0')

    def test_the_readme_commands_use_the_tool_constants(self):
        readme = (HERE / 'README.md').read_text(encoding='utf-8')
        sums_check = f'ssh-keygen -Y verify -f allowed_signers -I {vr.PRINCIPAL} -n {vr.NAMESPACE} -s SHA256SUMS.sig < SHA256SUMS'
        self.assertIn(sums_check, readme)
        self.assertIn("awk -v f=FILE '$2 == f' SHA256SUMS | sha256sum --check", readme)
        self.assertIn("grep -v '^ *#' allowed_signers | cut -d' ' -f3,4 | ssh-keygen -lf -", readme)
        self.assertIn('with ED25519 key SHA256:', readme)
        # The per-file signature binds no name: no document offers it as the check of a download.
        for doc in (readme, (ROOT / 'deploy/standalone/README.md').read_text(encoding='utf-8'),
                    (ROOT / 'prototypes/pocol-extension/README-IDE.md').read_text(encoding='utf-8'),
                    (ROOT / 'prototypes/pocol-extension/README-AR.md').read_text(encoding='utf-8'),
                    vr.__doc__):
            self.assertNotRegex(doc, r'-s \S+\.sig < (?!SHA256SUMS)')
            self.assertIn('-s SHA256SUMS.sig < SHA256SUMS', doc)
        self.assertIn('ssh-keygen -t ed25519 -C release@pocol.net', readme)
        self.assertIn('integrity', readme)
        for name in ('assemble_release.py', 'sign-release.sh', 'Sign-PoColRelease.ps1', 'verify_release.py', 'Verify-PoColRelease.ps1'):
            self.assertIn(name, readme)
            self.assertTrue((HERE / name).is_file(), name)
        for other in (ROOT / 'deploy/standalone/README.md', ROOT / 'prototypes/pocol-extension/README-IDE.md'):
            self.assertIn(f'-I {vr.PRINCIPAL} -n {vr.NAMESPACE}', other.read_text(encoding='utf-8'), other)

    def test_the_runbook_signs_only_after_the_identity_switch(self):
        # The assembler refuses every release until the switch sets the extension to the native version, and the
        # switch changes the native binaries too: no runbook step may ask for a signed or verified release before it.
        runbook = (ROOT / 'deploy/mainnet/README.md').read_text(encoding='utf-8')
        steps = dict(re.findall(r'(?ms)^(\d)\. \*\*(.*?)(?=^\d\. \*\*|^## )', runbook))
        self.assertEqual(sorted(steps), [str(n) for n in range(1, 9)])
        self.assertIn('Pin the identity', steps['7'])
        for n in '123456':
            self.assertNotIn('verify_release', steps[n], f'step {n}')
            self.assertNotRegex(steps[n], r'(?i)publish[^.]*signature', f'step {n}')
        self.assertIn('not a release signature', steps['1'])
        self.assertIn('assemble, sign and verify', steps['7'])
        standalone = (ROOT / 'deploy/standalone/README.md').read_text(encoding='utf-8')
        ceremony = standalone[:standalone.index('### Verify the download')]
        self.assertNotIn('Verify the release first', ceremony)
        self.assertIn('no signed release yet', ceremony)

    def test_the_byte_checked_files_keep_lf_on_every_checkout(self):
        # Git for Windows defaults to core.autocrlf=true: without -text, a checkout turns allowed_signers into CRLF
        # (both signers and verifiers refuse CR) and breaks the probe vector's signature.
        if subprocess.run(['git', 'rev-parse', '--git-dir'], cwd=ROOT, capture_output=True).returncode != 0:
            self.skipTest('not a git checkout')
        paths = ['deploy/release/allowed_signers'] + sorted(f'deploy/release/testdata/{p.name}' for p in (HERE / 'testdata').iterdir())
        self.assertGreaterEqual(len(paths), 4)
        out = subprocess.run(['git', 'check-attr', 'text', '--', *paths], cwd=ROOT, capture_output=True, text=True, check=True).stdout
        self.assertEqual(out.splitlines(), [f'{p}: text: unset' for p in paths])

    def test_the_scripts_are_ascii(self):
        for name in ('Verify-PoColRelease.ps1', 'Sign-PoColRelease.ps1', 'test_verify_release.ps1', 'sign-release.sh'):
            (HERE / name).read_bytes().decode('ascii')


if __name__ == '__main__':
    unittest.main()
