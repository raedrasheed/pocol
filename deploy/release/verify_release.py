#!/usr/bin/env python3
"""Verify a PoCol release directory: every file, SHA256SUMS and their ssh-ed25519 signatures (owner decision
2026-11-16). Python 3.8+, standard library only; the signatures are checked by OpenSSH's ssh-keygen (8.2 or later).

    python3 verify_release.py DIR [--allowed-signers FILE] [--ssh-keygen PATH]

DIR holds the release as it is published: the downloadable files, SHA256SUMS (one `<sha256>  <name>` line per file,
LF only, sorted names of letters, digits, `.`, `_` and `-`), SHA256SUMS.sig, and `<name>.sig` beside every listed
file. The signer is the principal release@pocol.net in namespace pocol-release of the allowed_signers file (default:
the committed deploy/release/allowed_signers, exactly one ssh-ed25519 key line, printable ASCII and LF only).

It prints `VERIFIED: ...` with the release key's SHA256 fingerprint (compare it with the one published on pocol.net)
and exits 0 only when SHA256SUMS.sig, every listed file's hash and every `<name>.sig` verify,
and nothing else is in DIR. Otherwise it prints `NOT VERIFIED: <reason>` and exits 1 (2 for a usage error). There is
no checksum-only success: a release without its signatures is never reported verified. A SHA-256 match alone is
integrity, not the publisher's authentication.

One download by hand: a FILE.sig binds only the file's bytes, not its name or version, so check the signed SHA256SUMS
and the file's line in it (deploy/release/README.md, user procedure):

    ssh-keygen -Y verify -f allowed_signers -I release@pocol.net -n pocol-release -s SHA256SUMS.sig < SHA256SUMS
    grep '  FILE$' SHA256SUMS | sha256sum --check
"""
import argparse, base64, binascii, hashlib, os, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
PRINCIPAL = 'release@pocol.net'
NAMESPACE = 'pocol-release'
SUMS = 'SHA256SUMS'
SIG = '.sig'
ALLOWED_SIGNERS = HERE / 'allowed_signers'
PROBE = HERE / 'testdata'
PROBE_PRINCIPAL = 'probe@pocol.invalid'
# The same strict line as Initialize-PoColNetwork.ps1's package check: lower-case hex, two spaces, a bare name.
LINE = re.compile(r'([0-9a-f]{64})  ([A-Za-z0-9._-]+)')
KEY_LINE = re.compile(r'(?P<principal>\S+) namespaces="(?P<ns>[^"]+)" (?P<type>\S+) (?P<key>\S+)(?: .*)?')
ARMOR = re.compile(rb'-----BEGIN SSH SIGNATURE-----\n[A-Za-z0-9+/=\n]+-----END SSH SIGNATURE-----\n?')
MAX_SIG = 4096


class NotVerified(Exception):
    pass


def read_allowed(path, principal=PRINCIPAL):
    """The one key line of an allowed_signers file: (principal, key type, base64). Refuses the placeholder.

    Only printable ASCII and LF are allowed, so this reading and ssh-keygen's agree on every line: ssh-keygen skips
    only spaces and tabs before a `#`, while Python's str.strip and .NET's Trim also remove VT, FF, NEL, NBSP and other
    Unicode spaces. A line `\\v#x,release@pocol.net namespaces=... KEY` would otherwise be a comment here and a
    second key line (whose principal pattern list matches release@pocol.net) to ssh-keygen."""
    try:
        raw = Path(path).read_bytes()
    except OSError as e:
        raise NotVerified(f'cannot read the allowed signers file {path} ({e})') from None
    if b'\r' in raw:
        raise NotVerified(f'{path}: CR line endings')
    bad = re.search(rb'[^\x20-\x7e\n]', raw)
    if bad:
        raise NotVerified(f'{path}: byte 0x{bad[0][0]:02x} at offset {bad.start()} is not printable ASCII (only printable '
                          'ASCII and LF are allowed)')
    text = raw.decode('ascii')
    lines = [l for l in text.split('\n') if l.strip(' ') and not l.lstrip(' ').startswith('#')]
    if len(lines) != 1:
        raise NotVerified(f'{path}: exactly one key line is allowed, found {len(lines)}')
    if 'PLACEHOLDER' in lines[0]:
        raise NotVerified(f'the owner release key is not committed ({path} holds the PLACEHOLDER line)')
    m = KEY_LINE.fullmatch(lines[0].strip(' '))
    if not m:
        raise NotVerified(f'{path}: the key line is not `{principal} namespaces="{NAMESPACE}" ssh-ed25519 KEY`')
    if m['principal'] != principal:
        raise NotVerified(f'{path}: the principal is {m["principal"]!r}, not {principal}')
    if m['ns'] != NAMESPACE:
        raise NotVerified(f'{path}: the namespace is {m["ns"]!r}, not {NAMESPACE}')
    if m['type'] != 'ssh-ed25519' or not re.fullmatch(r'AAAAC3NzaC1lZDI1NTE5[A-Za-z0-9+/]{48}', m['key']):
        raise NotVerified(f'{path}: the key is not an ssh-ed25519 public key')
    return m['principal'], m['type'], m['key']


def fingerprint(key):
    """The SHA256 fingerprint ssh-keygen -l prints for a public key's base64 blob: SHA256:<unpadded base64>."""
    try:
        blob = base64.b64decode(key, validate=True)
    except (binascii.Error, ValueError):
        raise NotVerified('the key is not base64') from None
    return 'SHA256:' + base64.b64encode(hashlib.sha256(blob).digest()).decode('ascii').rstrip('=')


def parse_sums(data):
    """SHA256SUMS bytes -> [(sha256, name)]: strict, LF only, distinct bare names, sorted."""
    try:
        text = data.decode('ascii')
    except UnicodeDecodeError:
        raise NotVerified(f'{SUMS} is not ASCII') from None
    if '\r' in text:
        raise NotVerified(f'{SUMS} has CR line endings (LF only)')
    if not text or not text.endswith('\n'):
        raise NotVerified(f'{SUMS} is empty or does not end with a newline')
    rows = []
    for line in text[:-1].split('\n'):
        m = LINE.fullmatch(line)
        if not m:
            raise NotVerified(f'{SUMS}: not a `<sha256>  <name>` line: {line[:120]!r}')
        name = m[2]
        if name in ('.', '..') or name.startswith('.') or name == SUMS or name.endswith(SIG):
            raise NotVerified(f'{SUMS}: {name!r} is not a release file name')
        rows.append((m[1], name))
    names = [n for _, n in rows]
    if len(set(names)) != len(names):
        raise NotVerified(f'{SUMS}: a name is listed twice')
    if names != sorted(names):
        raise NotVerified(f'{SUMS}: the names are not sorted')
    return rows


def find_keygen(given=None):
    keygen = given or shutil.which('ssh-keygen')
    if not keygen:
        raise NotVerified('ssh-keygen was not found (install OpenSSH 8.2 or later, or pass --ssh-keygen)')
    return keygen


def keygen_verify(keygen, allowed, principal, sig, data):
    """ssh-keygen -Y verify with the message on stdin as raw bytes."""
    try:
        r = subprocess.run([keygen, '-Y', 'verify', '-f', str(allowed), '-I', principal, '-n', NAMESPACE, '-s', str(sig)],
                           input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as e:
        raise NotVerified(f'cannot run {keygen} ({e})') from None
    return r.returncode == 0


def probe(keygen):
    """ssh-keygen must implement -Y verify (OpenSSH 8.2 or later): it must accept the committed known-good vector and
    refuse the same vector with one byte changed."""
    data = (PROBE / 'probe.txt').read_bytes()
    allowed, sig = PROBE / 'probe.allowed_signers', PROBE / 'probe.txt.sig'
    if not keygen_verify(keygen, allowed, PROBE_PRINCIPAL, sig, data) or \
            keygen_verify(keygen, allowed, PROBE_PRINCIPAL, sig, data + b'x'):
        raise NotVerified(f'{keygen} cannot check SSH signatures (ssh-keygen -Y verify needs OpenSSH 8.2 or later; on '
                          'Windows use Git for Windows usr\\bin\\ssh-keygen.exe or a current Win32-OpenSSH)')


def check_sig_file(path):
    try:
        raw = path.read_bytes() if path.is_file() and not path.is_symlink() else None
    except OSError:
        raw = None
    if raw is None:
        raise NotVerified(f'not signed: {path.name} is missing')
    if len(raw) > MAX_SIG or not ARMOR.fullmatch(raw):
        raise NotVerified(f'{path.name} is not an SSH signature')


def verify(directory, allowed=ALLOWED_SIGNERS, keygen=None, principal=PRINCIPAL):
    """Returns the number of signed files, or raises NotVerified."""
    directory = Path(directory)
    if not directory.is_dir():
        raise NotVerified(f'{directory} is not a directory')
    read_allowed(allowed, principal)
    keygen = find_keygen(keygen)
    probe(keygen)
    sums_path = directory / SUMS
    if not sums_path.is_file() or sums_path.is_symlink():
        raise NotVerified(f'no {SUMS} in {directory}')
    sums = sums_path.read_bytes()
    rows = parse_sums(sums)
    if not rows:
        raise NotVerified(f'{SUMS} lists no file')
    expected = {SUMS, SUMS + SIG} | {n for _, n in rows} | {n + SIG for _, n in rows}
    for entry in sorted(os.listdir(directory)):
        path = directory / entry
        if entry not in expected:
            raise NotVerified(f'{entry} is in the release directory but not in {SUMS}')
        if path.is_symlink() or not path.is_file():
            raise NotVerified(f'{entry} is not a regular file')
    check_sig_file(directory / (SUMS + SIG))
    if not keygen_verify(keygen, allowed, principal, directory / (SUMS + SIG), sums):
        raise NotVerified(f'{SUMS}.sig does not verify for {principal} in namespace {NAMESPACE}')
    for digest, name in rows:
        path = directory / name
        if not path.is_file() or path.is_symlink():
            raise NotVerified(f'{name} is listed in {SUMS} but missing')
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != digest:
            raise NotVerified(f'{name}: SHA-256 mismatch')
        check_sig_file(directory / (name + SIG))
        if not keygen_verify(keygen, allowed, principal, directory / (name + SIG), data):
            raise NotVerified(f'{name}.sig does not verify for {principal} in namespace {NAMESPACE}')
    return len(rows)


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('directory', type=Path)
    p.add_argument('--allowed-signers', type=Path, default=ALLOWED_SIGNERS)
    p.add_argument('--ssh-keygen', help='the ssh-keygen to use (default: the one on PATH)')
    a = p.parse_args(argv)
    try:
        count = verify(a.directory, a.allowed_signers, a.ssh_keygen)
        key = fingerprint(read_allowed(a.allowed_signers)[2])
    except NotVerified as e:
        print(f'NOT VERIFIED: {e}')
        return 1
    print(f'VERIFIED: {count} files, {SUMS} and every signature ({PRINCIPAL}, namespace {NAMESPACE}, key {key} (ED25519), '
          f'{a.allowed_signers})')
    print(f'Compare the key fingerprint {key} with the release key fingerprint published on pocol.net.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
