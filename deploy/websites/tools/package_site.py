#!/usr/bin/env python3
"""Build the cPanel deployment package of pocol.net's document root (the owner's request of 2026-11-23).

    python3 -B deploy/websites/tools/package_site.py --site pocol.net --out DIR

DIR (new or empty) receives:

    PoCol-pocol.net-site-<commit date>-<12 hex of the commit>.zip   the document root: extract it into pocol.net's
                                     Document Root. Every file of the snapshot but .htaccess, SHA256SUMS.txt included.
    <zip>.sha256                     `sha256sum -c` line of the ZIP
    pocol.net.htaccess               the snapshot's .htaccess, merged by hand into the host's (cPanel writes its PHP
                                     handler block there, so the host's file is never replaced; tools/host_files.py MERGE)
    DEPLOY-pocol.net-AR.md, DEPLOY-pocol.net-EN.md
                                     the deployment, verification and rollback steps (deploy/websites/package/, filled
                                     with this package's name, digest, commit and network identity)
    MANIFEST.json                    commit, identity, the ZIP's files with their size and SHA-256, what was left out
    PACKAGE-SHA256SUMS.txt           `sha256sum -c` lines of every other file of DIR

The files are the committed blobs of the site at HEAD (deploy/websites/tools/site_snapshot.py's tracked_snapshot: no
working-tree, autocrlf or export filter, so an untracked or ignored file never ships), and the build refuses:
- a working tree with a tracked change anywhere, or an untracked file under deploy/websites (site_identity.py --check
  reads the working tree, so it must be the committed tree);
- a tree that `site_identity.py --check` does not pass unchanged (identity, ?v= content versions, SHA256SUMS.txt);
- a snapshot file that SHA256SUMS.txt does not list with its digest, or a line for a file that is neither in the
  snapshot nor a declared kept-out release file (site-identity.json keptOut; those are listed in MANIFEST.json and the
  instructions as files to upload from the signed release);
- any file that must never be in a document root package: config.php, .user.ini, php.ini, error_log, a dotfile other
  than .htaccess, keys (*.key, *.pem), databases, backups, archives, .git, tests/, tools/, node_modules/, and owner notes
  (README*, DEPLOY*, INSTALL*, NOTES*), which https://pocol.net/ would serve.

The ZIP is reproducible: entries in byte order of their path, a directory entry before its files, files 0644 and
directories 0755 (Unix), each entry dated the commit's committer time (UTC; a later commit gets a later time, so a
changed file of the same size is not taken for the old one by an If-Modified-Since check), deflated at level 9, no extra
fields and no comment. The same commit gives the same bytes (with the same zlib, as on one CI runner image). Nothing is uploaded or deployed and no network is
contacted.
"""
import argparse, hashlib, io, json, posixpath, re, stat, subprocess, sys, time, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import site_snapshot  # noqa: E402  (tracked_snapshot: the committed blobs exactly)

SITES = {'pocol.net': 'deploy/websites/pocol.net'}
DECLARED = 'deploy/websites/site-identity.json'
TEMPLATES = 'deploy/websites/package'
SUMS = 'SHA256SUMS.txt'
# Left out of the ZIP and shipped beside it.
BESIDE = {'.htaccess': 'pocol.net.htaccess'}
ABBREV = 12
PHP_MINIMUM, PHP_RECOMMENDED = '8.1', '8.3'
# Paths (relative to the document root) that a document root package must never hold.
DENY = re.compile(
    r'(?i)(?:^|/)(?:config\.php|\.user\.ini|php\.ini|error_log|\.git(?:/|$)|\.git[a-z]*|node_modules/|tests?/|tools/'
    r'|\.ds_store|thumbs\.db|(?:readme|deploy|install|notes?)[^/]*)$'
    r'|(?:^|/)(?:node_modules|tests?|tools|\.git|\.svn)/'
    r'|\.(?:key|pem|p12|pfx|sql|sqlite3?|db|bak|swp|orig|log|zip|tar|gz|tgz|bz2|xz|7z|rar|apk|exe|py|pyc|sh)$'
    r'|(?:^|/)\.(?!htaccess$)[^/]+(?:/|$)')
PLACEHOLDER = re.compile(r'\{\{([A-Z][A-Z0-9_]*)\}\}')


class Refused(Exception):
    pass


def git(repo, *args, timeout=60):
    out = subprocess.run(['git', '-c', 'core.autocrlf=false', '-c', 'core.eol=lf', '-C', str(repo), *args],
                         capture_output=True, timeout=timeout)
    if out.returncode != 0:
        raise Refused(f'git {" ".join(args)}: {out.stderr.decode("utf-8", "replace").strip()}')
    return out.stdout.decode('utf-8')


def require_clean(repo):
    """Refused unless the working tree is the committed tree: no tracked change anywhere and no untracked (not ignored)
    file under deploy/websites, which site_identity.py --check would read."""
    changed = git(repo, 'status', '--porcelain=v1', '--untracked-files=no')
    untracked = git(repo, 'status', '--porcelain=v1', '--untracked-files=all', '--', 'deploy/websites')
    untracked = [line for line in untracked.splitlines() if line.startswith('??')]
    if changed.strip() or untracked:
        lines = changed.splitlines() + untracked
        raise Refused('the working tree is not the committed tree (commit or remove these first): '
                      + '; '.join(lines[:10]) + (' ...' if len(lines) > 10 else ''))


def identity_check(repo):
    """site_identity.py --check must pass and change nothing."""
    out = subprocess.run([sys.executable, '-B', str(Path(repo) / 'deploy/websites/tools/site_identity.py'), '--check'],
                         capture_output=True, text=True, timeout=900, cwd=str(repo))
    if out.returncode != 0 or 'changed: 0 ' not in out.stdout:
        raise Refused(f'site_identity.py --check did not pass unchanged: {(out.stdout + out.stderr).strip()[-2000:]}')
    return out.stdout.strip()


def commit_info(repo):
    sha = git(repo, 'rev-parse', 'HEAD').strip()
    short = git(repo, '-c', f'core.abbrev={ABBREV}', 'log', '-1', '--format=%h', sha).strip()[:ABBREV]
    date = git(repo, 'log', '-1', '--format=%cs', sha).strip()
    epoch = int(git(repo, 'log', '-1', '--format=%ct', sha).strip())
    if not re.fullmatch(r'[0-9a-f]{40,64}', sha) or len(short) != ABBREV or not re.fullmatch(r'\d{4}-\d{2}-\d{2}', date):
        raise Refused(f'unexpected commit data {sha!r} {short!r} {date!r}')
    return {'commit': sha, 'short': short, 'date': date, 'epoch': epoch}


def parse_sums(data):
    sums = {}
    for n, line in enumerate(data.decode('utf-8').splitlines(), 1):
        m = re.fullmatch(r'([0-9a-f]{64})  (\S.*)', line)
        if not m:
            raise Refused(f'{SUMS} line {n}: not "SHA256  PATH": {line[:100]!r}')
        if m.group(2) in sums:
            raise Refused(f'{SUMS} line {n}: {m.group(2)} listed twice')
        sums[m.group(2)] = m.group(1)
    return sums


def select(files, kept_out):
    """(zip_files, beside) from the snapshot {path: bytes}: every file is checked against SHA256SUMS.txt and the deny
    list; .htaccess goes beside the ZIP."""
    if SUMS not in files:
        raise Refused(f'the snapshot has no {SUMS}')
    sums = parse_sums(files[SUMS])
    denied = sorted(p for p in files if DENY.search(p))
    if denied:
        raise Refused(f'files that a document root package must never hold: {denied}')
    for path, data in sorted(files.items()):
        if path == SUMS:
            continue
        if path not in sums:
            raise Refused(f'{SUMS} does not list {path} (run deploy/websites/tools/site_identity.py)')
        if hashlib.sha256(data).hexdigest() != sums[path]:
            raise Refused(f'{SUMS}: {path} is not the committed file (run deploy/websites/tools/site_identity.py)')
    extra = sorted(set(sums) - set(files))
    unknown = [p for p in extra if p not in kept_out]
    if unknown:
        raise Refused(f'{SUMS} lists files that are neither in the snapshot nor declared kept-out downloads: {unknown}')
    beside = {BESIDE[p]: files[p] for p in BESIDE if p in files}
    return {p: d for p, d in files.items() if p not in BESIDE}, beside, {p: sums[p] for p in extra}


def zip_time(epoch):
    t = time.gmtime(epoch)
    if t.tm_year < 1980:
        raise Refused('a commit time before 1980 cannot be a ZIP time')
    return (t.tm_year, t.tm_mon, t.tm_mday, t.tm_hour, t.tm_min, min(t.tm_sec - t.tm_sec % 2, 58))


def write_zip(files, epoch):
    """The ZIP bytes of {path: bytes}: sorted entries, directories first within their parent, fixed modes and times."""
    when = zip_time(epoch)
    dirs = sorted({posixpath.dirname(p) for p in files} - {''})
    dirs = sorted({'/'.join(d.split('/')[:i]) for d in dirs for i in range(1, d.count('/') + 2)})
    entries = sorted([(d + '/', None) for d in dirs] + list(files.items()), key=lambda e: e[0].encode('utf-8'))
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w') as z:
        for name, data in entries:
            info = zipfile.ZipInfo(name, date_time=when)
            info.create_system = 3
            if data is None:
                info.external_attr = ((stat.S_IFDIR | 0o755) << 16) | 0x10
                info.compress_type = zipfile.ZIP_STORED
                z.writestr(info, b'')
            else:
                info.external_attr = (stat.S_IFREG | 0o644) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                z.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    return buf.getvalue()


def verify_zip(data, files):
    """Reopen the ZIP: exactly `files`, byte for byte, with the fixed modes, no extra field and no comment."""
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        if z.testzip() is not None or z.comment:
            raise Refused('the written ZIP does not read back')
        names = [i.filename for i in z.infolist() if not i.is_dir()]
        if sorted(names) != sorted(files) or len(names) != len(set(names)):
            raise Refused('the written ZIP does not hold exactly the package files')
        for info in z.infolist():
            mode = info.external_attr >> 16
            if info.extra or (info.is_dir() and mode != stat.S_IFDIR | 0o755) or (not info.is_dir() and mode != stat.S_IFREG | 0o644):
                raise Refused(f'{info.filename}: unexpected ZIP entry attributes')
            if not info.is_dir() and z.read(info) != files[info.filename]:
                raise Refused(f'{info.filename}: the ZIP entry is not the committed file')


def render(template, values):
    missing = sorted(set(PLACEHOLDER.findall(template)) - set(values))
    if missing:
        raise Refused(f'unknown placeholders {missing}')
    return PLACEHOLDER.sub(lambda m: str(values[m.group(1)]), template)


def build(out, site='pocol.net', repo=ROOT, check=True):
    """Write the package into `out` and return its manifest. `check` runs the clean-tree and site_identity.py --check
    refusals (tests of the packaging itself on a scratch repository turn it off)."""
    if site not in SITES:
        raise Refused(f'--site {site}: only {sorted(SITES)}')
    repo = Path(repo)
    out = Path(out)
    if out.exists() and (not out.is_dir() or any(out.iterdir())):
        raise Refused(f'{out} is not a new or empty directory')
    info = commit_info(repo)
    identity_report = None
    if check:
        require_clean(repo)
        identity_report = identity_check(repo)
    declared = json.loads(git(repo, 'show', f'{info["commit"]}:{DECLARED}'))
    kept_out = {n.split('/', 1)[1] for n in declared.get('keptOut', {}).get(site, [])}
    blobs = site_snapshot.tracked_snapshot(repo, info['commit'], SITES[site])
    files = {p.lstrip('/'): d for p, d in blobs.items()}
    zip_files, beside, release_files = select(files, kept_out)
    data = write_zip(zip_files, info['epoch'])
    verify_zip(data, zip_files)
    name = f'PoCol-{site}-site-{info["date"]}-{info["short"]}.zip'
    digest = hashlib.sha256(data).hexdigest()
    values = {
        'ZIP': name, 'ZIP_SHA256': digest, 'ZIP_BYTES': len(data), 'COMMIT': info['commit'], 'SHORT': info['short'],
        'DATE': info['date'], 'CHAIN_ID': declared['chainId'], 'GENESIS': declared['genesisHash'],
        'NETWORK': declared['name'], 'FILE_COUNT': len(zip_files), 'ARTIFACT': f'{site}-cpanel-{info["short"]}',
        'HTACCESS': BESIDE['.htaccess'], 'PHP_MINIMUM': PHP_MINIMUM, 'PHP_RECOMMENDED': PHP_RECOMMENDED,
        'GATEWAY': declared.get('gatewayHost', 'explorer.pocol.net'),
        'RELEASE_FILES': '\n'.join(f'- `{p}`' for p in sorted(release_files)) or '-',
    }
    docs = {}
    for lang in ('AR', 'EN'):
        template = git(repo, 'show', f'{info["commit"]}:{TEMPLATES}/DEPLOY-{site}-{lang}.md')
        docs[f'DEPLOY-{site}-{lang}.md'] = render(template, values).encode('utf-8')
    manifest = {
        'site': site, 'commit': info['commit'], 'commitDate': info['date'], 'commitTime': info['epoch'],
        'network': declared['name'], 'chainId': declared['chainId'], 'genesisHash': declared['genesisHash'],
        'zip': {'name': name, 'bytes': len(data), 'sha256': digest, 'files': len(zip_files)},
        'files': [{'path': p, 'bytes': len(d), 'sha256': hashlib.sha256(d).hexdigest()}
                  for p, d in sorted(zip_files.items(), key=lambda e: e[0].encode('utf-8'))],
        'beside': {p: BESIDE[p] for p in BESIDE if p in files},
        'releaseFilesNotInGit': [{'path': p, 'sha256': d} for p, d in sorted(release_files.items())],
        'php': {'minimum': PHP_MINIMUM, 'recommended': PHP_RECOMMENDED, 'extensions': ['curl', 'json']},
        'identityCheck': identity_report,
        'artifact': values['ARTIFACT'],
    }
    outputs = {name: data, name + '.sha256': f'{digest}  {name}\n'.encode('ascii'), **beside, **docs,
               'MANIFEST.json': (json.dumps(manifest, indent=2, ensure_ascii=False) + '\n').encode('utf-8')}
    outputs['PACKAGE-SHA256SUMS.txt'] = ''.join(f'{hashlib.sha256(outputs[n]).hexdigest()}  {n}\n'
                                                for n in sorted(outputs)).encode('ascii')
    out.mkdir(parents=True, exist_ok=True)
    for n, d in outputs.items():
        (out / n).write_bytes(d)
    return manifest


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    p.add_argument('--site', required=True, choices=sorted(SITES))
    p.add_argument('--out', required=True, type=Path)
    a = p.parse_args(argv)
    try:
        manifest = build(a.out, a.site)
    except (Refused, ValueError, OSError, subprocess.SubprocessError) as e:
        print(f'FAIL {e}', file=sys.stderr)
        return 2
    z = manifest['zip']
    print(f'OK {z["name"]} {z["bytes"]} bytes sha256 {z["sha256"]} files {z["files"]} commit {manifest["commit"]} '
          f'chainId {manifest["chainId"]} genesisHash {manifest["genesisHash"]} -> {a.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
