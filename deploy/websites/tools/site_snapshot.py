#!/usr/bin/env python3
"""Sites W5: build the publish directory of a site's on-chain copy (rehearsal S1) from the git-tracked snapshot.

    python3 deploy/websites/tools/site_snapshot.py --out DIR \\
        [--site deploy/websites/explorer.pocol.net] [--include deploy/websites/tools/explorer-s1-include.txt] [--rev HEAD]

The files are the committed blobs of SITE at REV, read with `git ls-tree` and `git cat-file` so that no autocrlf, eol
or export filter changes a byte (never a host docroot or a test copy), and each file the snapshot's SHA256SUMS.txt lists
must match its entry. Only the paths of the include list are written to DIR, which must be new or empty. The hard-deny list (hidden files, config.*, *.key,
*.pem, *.sql, *.bak*, api/**, SHA256SUMS*) is applied whatever the list says, and every refused or left-out path is
reported. The rules are those of the extension (lib/publish-input.js), Studio (model.js) and pocol-site (site.rs);
pocol-extension/tests/data/publish-input-vectors.json is their shared vector file. Publish DIR afterwards with
`pocol-site publish --dir DIR` or PoCol Studio's folder import. Nothing here contacts a network.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
DENY_REASONS = ('dotfile', 'config', 'secret-key', 'database', 'backup', 'server-api', 'checksums')


def deny_reason(path):
    """The hard-deny reason for a site path ('/a/b.txt'), or None. Case-insensitive."""
    segs = path.lstrip('/').split('/')
    base = segs[-1].lower()
    if any(s.startswith('.') for s in segs):
        return 'dotfile'
    if len(segs) > 1 and segs[0].lower() == 'api':
        return 'server-api'
    if base.startswith('config.'):
        return 'config'
    if base.endswith(('.key', '.pem')):
        return 'secret-key'
    if base.endswith('.sql'):
        return 'database'
    if '.bak' in base:
        return 'backup'
    if base.startswith('sha256sums'):
        return 'checksums'
    return None


def parse_include_list(text):
    out = []
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        path = '/' + line.lstrip('/')
        if path in out:
            raise ValueError(f'duplicate include-list path {path}')
        out.append(path)
    if not out:
        raise ValueError('the include list is empty')
    return out


def select(paths, include):
    """{kept, denied:[(path, reason)], excluded, missing} in the order of `paths`, as the other implementations."""
    wanted, kept, denied, excluded = set(include), [], [], []
    for path in paths:
        reason = deny_reason(path)
        if reason:
            denied.append((path, reason))
        elif path not in wanted:
            excluded.append(path)
        else:
            kept.append(path)
    present = set(paths)
    return {'kept': kept, 'denied': denied, 'excluded': excluded, 'missing': [p for p in include if p not in present]}


def git_bytes(repo, *args, stdin=None):
    """A git command's raw stdout. Line-ending and export settings never apply: the blobs are read, not checked out or
    archived, and core.autocrlf/core.eol are pinned off anyway (Git for Windows defaults core.autocrlf to true)."""
    return subprocess.run(['git', '-c', 'core.autocrlf=false', '-c', 'core.eol=lf', '-C', str(repo), *args],
                          check=True, capture_output=True, input=stdin).stdout


def tracked_snapshot(repo, rev, site):
    """{site path: bytes} of the regular files of `site` at `rev`: the committed blobs exactly, read with `git ls-tree`
    and `git cat-file --batch` (no working-tree, autocrlf, eol or export filter). Links and submodules are refused."""
    site = site.strip('/')
    prefix = site + '/'
    listing = git_bytes(repo, 'ls-tree', '-r', '-z', '--full-tree', rev, '--', site)
    entries = []
    for record in listing.split(b'\0'):
        if not record:
            continue
        meta, name = record.split(b'\t', 1)
        mode, kind, oid = meta.decode('ascii').split(' ')
        name = name.decode('utf-8')
        if kind != 'blob' or mode not in ('100644', '100755'):
            raise ValueError(f'{name}: not a regular file in the tracked snapshot')
        if not name.startswith(prefix):
            raise ValueError(f'{name}: outside {site}')
        entries.append(('/' + name[len(prefix):], oid))
    if not entries:
        raise ValueError(f'{site} has no tracked files at {rev}')
    batch = git_bytes(repo, 'cat-file', '--batch', stdin=''.join(oid + '\n' for _, oid in entries).encode('ascii'))
    files, at = {}, 0
    for path, oid in entries:
        header_end = batch.index(b'\n', at)
        got, kind, size = batch[at:header_end].decode('ascii').split(' ')
        if got != oid or kind != 'blob':
            raise ValueError(f'{path}: git cat-file answered {got} {kind}')
        start = header_end + 1
        files[path] = batch[start:start + int(size)]
        at = start + int(size) + 1
    return files


def checksum_entries(files):
    """{site path: sha256 hex} from the snapshot's own SHA256SUMS.txt ('<hex>  <path>' lines), if it has one."""
    text = files.get('/SHA256SUMS.txt')
    if text is None:
        return {}
    sums = {}
    for line in text.decode('utf-8').splitlines():
        if not line.strip():
            continue
        parts = line.split(None, 1)
        if len(parts) != 2 or len(parts[0]) != 64 or any(c not in '0123456789abcdefABCDEF' for c in parts[0]):
            raise ValueError(f'SHA256SUMS.txt: malformed line {line!r}')
        sums['/' + parts[1].strip().lstrip('*').lstrip('/')] = parts[0].lower()
    return sums


def build(out, site, include_file, rev='HEAD', repo=ROOT):
    include = parse_include_list(pathlib.Path(include_file).read_text(encoding='utf-8'))
    files = tracked_snapshot(repo, rev, site)
    sel = select(sorted(files), include)
    refused = [p for p, _ in sel['denied'] if p in include]
    if sel['missing'] or refused:
        raise ValueError(f'the include list names files that are not tracked ({sel["missing"]}) or are always refused ({refused})')
    sums = checksum_entries(files)
    wrong = [p for p in sel['kept'] if p in sums and hashlib.sha256(files[p]).hexdigest() != sums[p]]
    if wrong:
        raise ValueError(f'these files differ from the snapshot\'s SHA256SUMS.txt: {wrong}')
    out = pathlib.Path(out)
    if out.exists() and any(out.iterdir()):
        raise ValueError(f'{out} is not empty')
    for path in sel['kept']:
        target = out / path.lstrip('/')
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(files[path])
    return {'site': site, 'rev': rev, 'files': [{'path': p, 'bytes': len(files[p]), 'sha256': hashlib.sha256(files[p]).hexdigest()} for p in sel['kept']],
            'bytes': sum(len(files[p]) for p in sel['kept']),
            'refused': [{'path': p, 'reason': r} for p, r in sel['denied']], 'excluded': sel['excluded']}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--out', required=True)
    ap.add_argument('--site', default='deploy/websites/explorer.pocol.net')
    ap.add_argument('--include', default=str(ROOT / 'deploy/websites/tools/explorer-s1-include.txt'))
    ap.add_argument('--rev', default='HEAD')
    a = ap.parse_args(argv)
    try:
        report = build(a.out, a.site, a.include, a.rev)
    except (ValueError, subprocess.CalledProcessError) as e:
        print(f'error: {e}', file=sys.stderr)
        return 1
    for r in report['refused']:
        print(f"refused {r['path']}: {r['reason']} (never published)", file=sys.stderr)
    for p in report['excluded']:
        print(f'not listed {p}', file=sys.stderr)
    print(json.dumps(report, indent=2))
    return 0


if __name__ == '__main__':
    sys.exit(main())
