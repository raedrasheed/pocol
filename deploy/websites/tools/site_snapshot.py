#!/usr/bin/env python3
"""Sites W5: build the publish directory of a site's on-chain copy (rehearsal S1) from the git-tracked snapshot.

    python3 deploy/websites/tools/site_snapshot.py --out DIR \\
        [--site deploy/websites/explorer.pocol.net] [--include deploy/websites/tools/explorer-s1-include.txt] [--rev HEAD]

The files come from `git archive REV -- SITE` (what is committed, never a host docroot or a test copy). Only the paths
of the include list are written to DIR, which must be new or empty. The hard-deny list (hidden files, config.*, *.key,
*.pem, *.sql, *.bak*, api/**, SHA256SUMS*) is applied whatever the list says, and every refused or left-out path is
reported. The rules are those of the extension (lib/publish-input.js), Studio (model.js) and pocol-site (site.rs);
pocol-extension/tests/data/publish-input-vectors.json is their shared vector file. Publish DIR afterwards with
`pocol-site publish --dir DIR` or PoCol Studio's folder import. Nothing here contacts a network.
"""
import argparse
import hashlib
import io
import json
import pathlib
import subprocess
import sys
import tarfile

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


def tracked_snapshot(repo, rev, site):
    """{site path: bytes} of the regular files of `site` at `rev`, read from `git archive`. Links are refused."""
    prefix = site.strip('/') + '/'
    data = subprocess.run(['git', '-C', str(repo), 'archive', '--format=tar', rev, '--', site.strip('/')],
                          check=True, capture_output=True).stdout
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data)) as tar:
        for member in tar.getmembers():
            if member.isdir() or member.name == 'pax_global_header':
                continue
            if not member.isfile():
                raise ValueError(f'{member.name}: not a regular file in the tracked snapshot')
            if not member.name.startswith(prefix):
                raise ValueError(f'{member.name}: outside {site}')
            files['/' + member.name[len(prefix):]] = tar.extractfile(member).read()
    if not files:
        raise ValueError(f'{site} has no tracked files at {rev}')
    return files


def build(out, site, include_file, rev='HEAD', repo=ROOT):
    include = parse_include_list(pathlib.Path(include_file).read_text(encoding='utf-8'))
    files = tracked_snapshot(repo, rev, site)
    sel = select(sorted(files), include)
    refused = [p for p, _ in sel['denied'] if p in include]
    if sel['missing'] or refused:
        raise ValueError(f'the include list names files that are not tracked ({sel["missing"]}) or are always refused ({refused})')
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
