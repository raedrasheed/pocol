#!/usr/bin/env python3
"""Fill pocol.net's download buttons from a signed release directory (2026-11-22, the download page structure).

Until the signed release exists, pocol.net/wallet-downloads.js offers nothing: every platform is "" and the home page
shows "No release available". This tool is the only step that fills it, from a release directory that
deploy/release/verify_release.py verifies (SHA256SUMS.sig, every file's hash and FILE.sig, with the committed
deploy/release/allowed_signers), so the page can never name a file, size or hash that is not the signed release's:

    python3 deploy/websites/tools/release_page.py RELEASE_DIR            # verify, then write
    python3 deploy/websites/tools/release_page.py RELEASE_DIR --check    # verify and show, write nothing

It writes, for each native package that deploy/release/assemble_release.py names (linux, windows, apk -> android):
{"url": "https://pocol.net/downloads/NAME", "size": "12.3 MB", "sha256": HASH, "sig": URL.sig}, and under "release"
the version and the URLs of the signed SHA256SUMS and SHA256SUMS.sig. The extension package is listed in the release's
SHA256SUMS (users verify it with the steps on chrome-extension.html); the store listing, once it exists, is linked
separately. Then site_identity.py's --download records every release file, its .sig, SHA256SUMS and SHA256SUMS.sig as
pocol.net kept-out download lines, so SHA256SUMS.txt names exactly the files the owner uploads to pocol.net/downloads/.
Nothing is uploaded or deployed.
"""
import argparse, hashlib, json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'deploy' / 'websites' / 'tools'))
sys.path.insert(0, str(ROOT / 'deploy' / 'release'))
import site_identity as si  # noqa: E402
import verify_release as vr  # noqa: E402
import assemble_release as release  # noqa: E402

DOWNLOADS_JS = 'deploy/websites/pocol.net/wallet-downloads.js'
PREFIX = 'window.POCOL_DOWNLOADS = '
BASE = 'https://pocol.net/downloads/'
PLATFORMS = ('windows', 'linux', 'android', 'ios', 'macos')
KIND_PLATFORM = {'windows': 'windows', 'linux': 'linux', 'apk': 'android'}

class Refused(Exception):
    pass

def size_text(n):
    for unit, scale in (('GB', 1 << 30), ('MB', 1 << 20), ('KB', 1 << 10)):
        if n >= scale:
            return f'{n / scale:.1f} {unit}'
    return f'{n} B'

def read_downloads(root):
    text = (root / DOWNLOADS_JS).read_text(encoding='utf-8')
    if not text.startswith(PREFIX):
        raise Refused(f'{DOWNLOADS_JS}: does not start with {PREFIX!r}')
    return json.loads(text[len(PREFIX):].rstrip().rstrip(';'))

def plan(directory, root=ROOT):
    """(new wallet-downloads.js text, {pocol.net download path: sha256}) for a verified release directory."""
    directory = Path(directory)
    allowed = root / 'deploy' / 'release' / 'allowed_signers'
    try:
        vr.verify(directory, allowed)
    except vr.NotVerified as e:
        raise Refused(f'{directory}: the release does not verify ({e}); nothing is written') from None
    try:
        names = release.allowed_names(root)
    except release.Refused as e:
        raise Refused(f'no release can be named yet: {e}') from None
    rows = vr.parse_sums((directory / vr.SUMS).read_bytes())
    unknown = sorted(n for _, n in rows if n not in names)
    if unknown:
        raise Refused(f'{vr.SUMS} lists {unknown}, which are not release file names {sorted(names)}')
    version = release.native_version(root)
    downloads = {p: '' for p in PLATFORMS}
    lines = {}
    for digest, name in rows:
        sig = directory / (name + vr.SIG)
        tracked = root / si.MAIN / 'downloads' / name
        if tracked.is_file():
            # A release file the snapshot already holds (the invitation, published with the identity switch): its
            # SHA256SUMS.txt line is its own digest, which must be the signed one.
            if hashlib.sha256(tracked.read_bytes()).hexdigest() != digest:
                raise Refused(f'{tracked.relative_to(root)} is not the signed release\'s {name}')
        else:
            lines[f'{si.MAIN}/downloads/{name}'] = digest
        lines[f'{si.MAIN}/downloads/{name}{vr.SIG}'] = hashlib.sha256(sig.read_bytes()).hexdigest()
        platform = KIND_PLATFORM.get(names[name])
        if platform:
            downloads[platform] = {'url': BASE + name, 'size': size_text((directory / name).stat().st_size),
                                   'sha256': digest, 'sig': BASE + name + vr.SIG}
    for name in (vr.SUMS, vr.SUMS + vr.SIG):
        lines[f'{si.MAIN}/downloads/{name}'] = hashlib.sha256((directory / name).read_bytes()).hexdigest()
    downloads['release'] = {'version': version, 'sums': BASE + vr.SUMS, 'sumsSig': BASE + vr.SUMS + vr.SIG}
    current = read_downloads(root)
    if set(current) != set(downloads):
        raise Refused(f'{DOWNLOADS_JS}: names {sorted(current)}, expected {sorted(downloads)}')
    text = PREFIX + json.dumps(downloads, indent=2, ensure_ascii=False) + ';\n'
    return text, lines

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('directory', type=Path, help='the signed release directory (after verify_release.py prints VERIFIED)')
    p.add_argument('--check', action='store_true', help='verify and show only; write nothing')
    p.add_argument('--root', type=Path, default=ROOT)
    a = p.parse_args(argv)
    try:
        text, lines = plan(a.directory, a.root)
    except (OSError, ValueError, Refused, si.Refused) as e:
        print('FAIL', e, file=sys.stderr)
        return 2
    print(text, end='')
    if a.check:
        return 0
    target = a.root / DOWNLOADS_JS
    before = target.read_bytes()
    target.write_text(text, encoding='utf-8')
    args = [x for path, digest in sorted(lines.items()) for x in ('--download', f'{path}={digest}')]
    rc = si.main([*args, '--root', str(a.root)])
    if rc != 0:
        target.write_bytes(before)  # site_identity.py wrote nothing; neither does this run
        print('FAIL site_identity.py refused the release lines; nothing was changed', file=sys.stderr)
    return rc

if __name__ == '__main__':
    sys.exit(main())
