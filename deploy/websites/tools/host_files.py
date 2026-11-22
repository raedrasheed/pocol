#!/usr/bin/env python3
"""Compare a website host's document root with its snapshot before and after an upload (SITE-02, 2026-11-22).

An upload by overwrite (cPanel File Manager, FTP) leaves every file that the snapshot no longer has on the host, and the
host keeps serving it: the retired network's invitation downloads/network.pocol.json and its extension package are such
files. This tool reads a listing of the host's files with their SHA-256 and says, for one site:

    DELETE   a served file that is not in the snapshot (delete it from the host)
    DIFFERS  a file whose bytes are not the snapshot's (upload it again)
    MERGE    a .htaccess whose bytes are not the snapshot's. cPanel writes its own directives into the document root's
             .htaccess (the MultiPHP Manager's "# php -- BEGIN cPanel-generated handler, do not edit" ... "# php -- END
             cPanel-generated handler, do not edit" block, an HTTPS redirect), so it is never simply replaced: back up
             the host's file, keep those blocks and add the snapshot's .htaccess unchanged before or after them, then
             download a copy and check it with --host-copy .htaccess=COPY (the copy must be the listed file, and the
             snapshot's directives must appear in it as one unchanged block; comments and blank lines are ignored)
    MISSING  a file of the snapshot that the host does not have (upload it; a kept-out release file is uploaded from the
             signed release)
    KEEP     host configuration that is never in a snapshot (api/config.php, the MultiPHP INI Editor's .user.ini and
             php.ini, PHP's error_log, .well-known/, cgi-bin/, --keep PATH), a .htaccess merged as above or where the
             snapshot has none, and on
             pocol.net the retired packages that site_identity.py's LEGACY_NATIVE exempts while a page links them

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
sys.path.insert(0, str(Path(__file__).resolve().parent))
import site_identity as si  # noqa: E402  (LEGACY_NATIVE: the retired packages the pages still link, labelled as such)
SITES = {
    'pocol.net': ROOT / 'deploy/websites/pocol.net',
    'ide.pocol.net': ROOT / 'deploy/websites/ide.pocol.net',
    'studio.pocol.net': ROOT / 'prototypes/pocol-studio/dist',
}
SUMS = 'SHA256SUMS.txt'
# Host configuration that no snapshot holds and an upload must keep (deploy/websites/README.md: preserve every hosting
# configuration and credential): the PHP configuration, cPanel's PHP settings (MultiPHP INI Editor: .user.ini, php.ini)
# and PHP's error_log at any depth, and the host's own folders (AutoSSL challenges, CGI).
KEEP = re.compile(r'(?:^|/)(?:config\.php|\.user\.ini|php\.ini|error_log)$|^\.well-known/|^cgi-bin/')
# A .htaccess at any depth: cPanel adds its own directives (PHP handler, redirects), so a host copy is merged, not replaced.
HTACCESS = re.compile(r'(?:^|/)\.htaccess$')
MERGE_HELP = ("back up the host's file, keep its cPanel-generated block and any redirect, add the snapshot's .htaccess "
              "unchanged, then check a copy with --host-copy {path}=COPY")
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

def directives(data):
    """The directive lines of a .htaccess (stripped; comments and blank lines left out)."""
    lines = (line.strip() for line in data.decode('utf-8', 'strict').splitlines())
    return [line for line in lines if line and not line.startswith('#')]

def merged(snapshot, host):
    """Whether the snapshot .htaccess's directives appear in the host's, unchanged and as one block."""
    want, have = directives(snapshot), directives(host)
    return bool(want) and any(have[i:i + len(want)] == want for i in range(len(have) - len(want) + 1))

def snapshot_file(site, path, root=None):
    base = Path(root) / SITES[site].relative_to(ROOT) if root else SITES[site]
    return (base / path).read_bytes()

def compare(site, listing, keep=(), root=None, host_copies=None):
    """The report rows (kind, path, detail) of a host listing against `site`'s snapshot, sorted by kind and path.
    `host_copies` maps a listed path to the bytes of a copy of the host's file (a merged .htaccess)."""
    want, kept_out = expected(site, root)
    host_copies = host_copies or {}
    for path in host_copies:
        if path not in listing:
            raise Refused(f'--host-copy {path}: the listing has no such file')
    rows = []
    for path, digest in listing.items():
        if path in want:
            if want[path] != digest and HTACCESS.search(path):
                copy = host_copies.get(path)
                if copy is None:
                    rows.append(('MERGE', path, f'host {digest[:12]}, snapshot {want[path][:12]}: ' + MERGE_HELP.format(path=path)))
                elif hashlib.sha256(copy).hexdigest() != digest:
                    rows.append(('MERGE', path, 'the --host-copy file is not the listed host file'))
                elif not merged(snapshot_file(site, path, root), copy):
                    rows.append(('MERGE', path, "the host's file does not hold the snapshot's directives as one unchanged block: "
                                 + MERGE_HELP.format(path=path)))
                else:
                    rows.append(('KEEP', path, "merged: the host's own directives and the snapshot's, unchanged"))
            elif want[path] != digest:
                rows.append(('DIFFERS', path, f'host {digest[:12]}, snapshot {want[path][:12]}'))
        elif KEEP.search(path) or path in keep:
            rows.append(('KEEP', path, 'host configuration, not in a snapshot'))
        elif HTACCESS.search(path):
            # A site whose snapshot has no .htaccess there (ide.pocol.net): the host's own (cPanel's handler) stays.
            rows.append(('KEEP', path, "the host's .htaccess; the snapshot has none here (check that it holds only host directives)"))
        elif site == 'pocol.net' and path.startswith('downloads/') and path[len('downloads/'):] in si.LEGACY_NATIVE:
            rows.append(('KEEP', path, "the retired network's package, linked as such until the signed release (LEGACY_NATIVE)"))
        else:
            rows.append(('DELETE', path, 'not in the snapshot'))
    for path in sorted(set(want) - set(listing)):
        rows.append(('MISSING', path, 'a release file kept out of Git: upload it from the signed release'
                     if path in kept_out else 'upload it'))
    order = {'DELETE': 0, 'DIFFERS': 1, 'MERGE': 2, 'MISSING': 3, 'KEEP': 4}
    return sorted(rows, key=lambda r: (order[r[0]], r[1]))

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--site', required=True, choices=sorted(SITES))
    p.add_argument('--listing', required=True, type=Path, help='the host listing (sha256sum format)')
    p.add_argument('--keep', action='append', default=[], metavar='PATH', help='another host file to keep (repeatable)')
    p.add_argument('--host-copy', action='append', default=[], metavar='PATH=FILE',
                   help="a local copy of the host's file PATH (a merged .htaccess; repeatable)")
    a = p.parse_args(argv)
    try:
        listing = read_listing(a.listing.read_text(encoding='utf-8'))
        copies = {}
        for item in a.host_copy:
            path, sep, file = item.partition('=')
            if not sep or not file:
                raise Refused(f'--host-copy {item!r}: PATH=FILE')
            copies[norm(path)] = Path(file).read_bytes()
        rows = compare(a.site, listing, {norm(k) for k in a.keep}, host_copies=copies)
    except (OSError, UnicodeDecodeError, Refused) as e:
        print('FAIL', e, file=sys.stderr)
        return 2
    for kind, path, detail in rows:
        print(f'{kind:8} {path}  ({detail})')
    bad = [r for r in rows if r[0] != 'KEEP']
    counts = {k: sum(1 for r in rows if r[0] == k) for k in ('DELETE', 'DIFFERS', 'MERGE', 'MISSING', 'KEEP')}
    print(('OK' if not bad else 'NOT IN STEP') + f' {a.site}: ' + ', '.join(f'{k.lower()} {v}' for k, v in counts.items()))
    return 1 if bad else 0

if __name__ == '__main__':
    sys.exit(main())
