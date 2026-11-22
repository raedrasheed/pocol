#!/usr/bin/env python3
"""Compare a website host's document root with its snapshot before and after an upload (SITE-02, 2026-11-22).

An upload by overwrite (cPanel File Manager, FTP) leaves every file that the snapshot no longer has on the host, and the
host keeps serving it: the retired network's invitation downloads/network.pocol.json and its extension package are such
files. This tool reads a listing of the host's files with their SHA-256 and says, for one site:

    DELETE   a served file that is not in the snapshot (delete it from the host)
    DIFFERS  a file whose bytes are not the snapshot's (upload it again)
    MISSING  a file of the snapshot that the host does not have (upload it; a kept-out release file is uploaded from the
             signed release)
    KEEP     host configuration that is never in a snapshot (api/config.php, .well-known/, cgi-bin/, --keep PATH)

The exit status is 0 only when nothing is to delete, differs or is missing. Nothing on the host is read or changed: the
listing is made on the host by the owner, for example in cPanel's Terminal from the site's document root:

    find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ~/SITE-files.txt

(on macOS `shasum -a 256`), and then checked here:

    python3 deploy/websites/tools/host_files.py --site pocol.net --listing pocol.net-files.txt

The snapshot of a site with a SHA256SUMS.txt (pocol.net, ide.pocol.net) is that file's lines plus SHA256SUMS.txt
itself; studio.pocol.net (prototypes/pocol-studio/dist) is the hash of every file there. Each SHA256SUMS
line is checked against the tracked file first (deploy/websites/tools/site_identity.py --check keeps them equal).
explorer.pocol.net is not uploaded by hand: deploy/explorer installs its static copy.
"""
import argparse, hashlib, posixpath, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SITES = {
    'pocol.net': ROOT / 'deploy/websites/pocol.net',
    'ide.pocol.net': ROOT / 'deploy/websites/ide.pocol.net',
    'studio.pocol.net': ROOT / 'prototypes/pocol-studio/dist',
}
SUMS = 'SHA256SUMS.txt'
# Host configuration that no snapshot holds and an upload must keep (deploy/websites/README.md: preserve every hosting
# configuration and credential): the PHP configuration and the host's own folders (AutoSSL challenges, CGI).
KEEP = re.compile(r'(?:^|/)config\.php$|^\.well-known/|^cgi-bin/')
LINE = re.compile(r'([0-9a-fA-F]{64}) [ *](.+)')

class Refused(Exception):
    pass

def norm(path):
    """A listing path as a relative POSIX path of the document root, or Refused."""
    path = path.strip().replace('\\', '/')
    while path.startswith('./'):
        path = path[2:]
    clean = posixpath.normpath(path)
    if not path or path.startswith('/') or clean != path or clean.split('/')[0] == '..':
        raise Refused(f'listing path {path!r}: a relative path inside the document root')
    return clean

def read_listing(text):
    """{path: sha256} of a `sha256sum` listing (`HASH  PATH` or `HASH *PATH`); blank lines are skipped."""
    out = {}
    for n, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        m = LINE.fullmatch(line.rstrip('\r'))
        if not m:
            raise Refused(f'listing line {n}: not "SHA256  PATH": {line[:120]!r}')
        path = norm(m.group(2))
        if path in out:
            raise Refused(f'listing line {n}: {path} listed twice')
        out[path] = m.group(1).lower()
    return out

def expected(site, root=None):
    """{path: sha256} that the host must serve for `site`, and the kept-out names among them (release files that are
    not in Git: they are checked by the digest SHA256SUMS.txt records)."""
    base = Path(root) / SITES[site].relative_to(ROOT) if root else SITES[site]
    sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sums = base / SUMS
    if not sums.exists():
        return {p.relative_to(base).as_posix(): sha(p) for p in sorted(base.rglob('*')) if p.is_file()}, set()
    out, kept_out = {SUMS: sha(sums)}, set()
    for n, line in enumerate(sums.read_text(encoding='utf-8').splitlines(), 1):
        m = LINE.fullmatch(line)
        if not m:
            raise Refused(f'{sums}: line {n} is not "SHA256  PATH"')
        path, digest = norm(m.group(2)), m.group(1)
        local = base / path
        if local.is_file():
            if sha(local) != digest:
                raise Refused(f'{sums}: {path} is not the tracked file (run deploy/websites/tools/site_identity.py)')
        else:
            kept_out.add(path)
        out[path] = digest
    tracked = {p.relative_to(base).as_posix() for p in base.rglob('*') if p.is_file()}
    unlisted = sorted(tracked - set(out))
    if unlisted:
        raise Refused(f'{sums}: does not list {unlisted} (run deploy/websites/tools/site_identity.py)')
    return out, kept_out

def compare(site, listing, keep=(), root=None):
    """The report rows (kind, path, detail) of a host listing against `site`'s snapshot, sorted by kind and path."""
    want, kept_out = expected(site, root)
    rows = []
    for path, digest in listing.items():
        if path in want:
            if want[path] != digest:
                rows.append(('DIFFERS', path, f'host {digest[:12]}, snapshot {want[path][:12]}'))
        elif KEEP.search(path) or path in keep:
            rows.append(('KEEP', path, 'host configuration, not in a snapshot'))
        else:
            rows.append(('DELETE', path, 'not in the snapshot'))
    for path in sorted(set(want) - set(listing)):
        rows.append(('MISSING', path, 'a release file kept out of Git: upload it from the signed release'
                     if path in kept_out else 'upload it'))
    order = {'DELETE': 0, 'DIFFERS': 1, 'MISSING': 2, 'KEEP': 3}
    return sorted(rows, key=lambda r: (order[r[0]], r[1]))

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--site', required=True, choices=sorted(SITES))
    p.add_argument('--listing', required=True, type=Path, help='the host listing (sha256sum format)')
    p.add_argument('--keep', action='append', default=[], metavar='PATH', help='another host file to keep (repeatable)')
    a = p.parse_args(argv)
    try:
        listing = read_listing(a.listing.read_text(encoding='utf-8'))
        rows = compare(a.site, listing, {norm(k) for k in a.keep})
    except (OSError, UnicodeDecodeError, Refused) as e:
        print('FAIL', e, file=sys.stderr)
        return 2
    for kind, path, detail in rows:
        print(f'{kind:8} {path}  ({detail})')
    bad = [r for r in rows if r[0] != 'KEEP']
    counts = {k: sum(1 for r in rows if r[0] == k) for k in ('DELETE', 'DIFFERS', 'MISSING', 'KEEP')}
    print(('OK' if not bad else 'NOT IN STEP') + f' {a.site}: ' + ', '.join(f'{k.lower()} {v}' for k, v in counts.items()))
    return 1 if bad else 0

if __name__ == '__main__':
    sys.exit(main())
