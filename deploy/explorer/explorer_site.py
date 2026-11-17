#!/usr/bin/env python3
"""The explorer.pocol.net Caddy site of the gateway host (Sites W8, owner decision 9) and the static copy it serves.

One Caddy site block, between the markers below, serves the explorer snapshot's static files with Caddy's file_server
and routes the gateway's paths to deploy/rewards/gateway.py on 127.0.0.1:8088: /rpc (with its 16 KiB guard, as the
lost host block had), /health, and /api/* (the snapshot's same-origin reads, explorer_api.py, under the same guard).
The static files and /api/* carry the headers the explorer's .htaccess set (explorer_api.SECURITY_HEADERS); dotfiles,
server code, checksum lists and the extensions .htaccess denied are 404. The Studio installer
(deploy/studio/install_studio_gateway.py) adds its /studio/* route to this block, and an update keeps it.

The static copy is a directory of the snapshot's public files, as `deploy/websites/tools/site_snapshot.py --out DIR`
builds it from the committed blobs: regular files only, no hidden or server files, a bounded size, an index.html whose
scripts and stylesheets exist. It is installed under /srv/pocol-explorer/site-<first 12 hex of its manifest's
SHA-256>, so a new copy is a new root and a Caddy reload switches it at once.

    python3 deploy/explorer/explorer_site.py manifest --site DIR
    python3 deploy/explorer/explorer_site.py test-config --site DIR --listen 127.0.0.1:PORT --gateway 127.0.0.1:PORT --out FILE

The second writes a loopback copy of the block (http://, no admin endpoint) for a local test (explorer_smoke.mjs).
Nothing here contacts a network or changes a host.
"""
import argparse
import hashlib
import os
from pathlib import Path, PurePosixPath
import re
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'rewards'))
sys.path.insert(0, str(HERE.parent / 'websites' / 'tools'))
import explorer_api  # noqa: E402  (the header set and the request limits of the gateway's /api reads)
from site_snapshot import deny_reason  # noqa: E402  (the publish hard-deny list: dotfiles, api/**, config.*, checksums)

HOST = 'explorer.pocol.net'
GATEWAY = '127.0.0.1:8088'
SITE_PARENT = PurePosixPath('/srv/pocol-explorer')
BEGIN = '# BEGIN POCOL EXPLORER GATEWAY v1'
END = '# END POCOL EXPLORER GATEWAY v1'
# The static copy: what the explorer snapshot holds, and room for a rebuilt bundle (W7) or added fonts.
EXTENSIONS = {'.html', '.js', '.css', '.png', '.ico', '.webmanifest', '.svg', '.woff2'}
MAX_FILES = 64
MAX_FILE = 4 * 1024 * 1024
MAX_TOTAL = 8 * 1024 * 1024
SEGMENT = re.compile(r'[A-Za-z0-9][A-Za-z0-9._-]{0,127}')
SITE_ROOT = re.compile(r'/srv/pocol-explorer/site-[0-9a-f]{12}')
# The .htaccess deny list (dotfiles; zip|tar|gz|bak|log|sh|md|json|map), server code and checksum lists: 404 even if
# such a file were in the root. The copy never holds one (site_manifest refuses it).
HIDDEN = r'(?i)(^|/)\.|\.(php[0-9]?|phtml|zip|tar|gz|bak|log|sh|md|json|map)$|(^|/)sha256sums'
GUARD = ('expression `{http.request.header.Content-Length} != "" && '
         'int({http.request.header.Content-Length}) > ' + str(explorer_api.MAX_BODY) + '`')


def _headers(indent):
    pad = ' ' * indent
    # nosniff is set for the whole site; Referrer-Policy, X-Frame-Options, Permissions-Policy and the CSP are the
    # explorer's (.htaccess), so they apply to its files and its /api reads, not to /rpc, /health or /studio/*.
    lines = [f'{pad}{name} "{value}"' for name, value in explorer_api.SECURITY_HEADERS if name != 'X-Content-Type-Options']
    return f'{pad[:-4]}header {{\n' + '\n'.join(lines) + f'\n{pad}defer\n{pad[:-4]}}}'


def render_block(site_root, host=HOST, gateway=GATEWAY):
    """The managed site block. Its /rpc route, `max_size 16384` and 16 KiB guard text are the ones the Studio installer
    requires (install_studio_gateway.patch_caddy), and nothing in it names the Studio relay port. Every block opens
    with `{` at the end of its line and closes on a line of its own (Caddy 2.7 and later refuse a token after `{` on
    the same line, such as `request_body { max_size 16384 }`); one-path matchers use the one-line form."""
    if not SITE_ROOT.fullmatch(str(site_root)):
        raise ValueError('The site root is /srv/pocol-explorer/site-<12 hex>')
    if not re.fullmatch(r'127\.0\.0\.1:[0-9]{1,5}', gateway):
        raise ValueError('The gateway is on loopback')
    return f'''{BEGIN}
{host} {{
    header {{
        Cache-Control "no-store"
        X-Content-Type-Options "nosniff"
        -Server
    }}
    request_body {{
        max_size {explorer_api.MAX_BODY}
    }}
    @rpc path /rpc
    handle @rpc {{
        @oversized {GUARD}
        respond @oversized "Request body exceeds 16 KiB" 413
        reverse_proxy {gateway}
    }}
    @health path /health
    handle @health {{
        reverse_proxy {gateway}
    }}
    @explorer_api path /api/*
    handle @explorer_api {{
{_headers(12)}
        @api_oversized {GUARD}
        respond @api_oversized "Request body exceeds 16 KiB" 413
        reverse_proxy {gateway} {{
            transport http {{
                dial_timeout 3s
                response_header_timeout 15s
            }}
        }}
    }}
    handle {{
{_headers(12)}
        @explorer_hidden path_regexp explorer_hidden `{HIDDEN}`
        respond @explorer_hidden "Not found" 404
        root * {site_root}
        file_server
    }}
}}
{END}
'''


def loopback_config(site_root, listen, gateway):
    """A local test copy of the block: plain HTTP on `listen`, no admin endpoint, the copy's root (any path)."""
    if not re.fullmatch(r'127\.0\.0\.1:[0-9]{1,5}', listen):
        raise ValueError('A test listens on loopback')
    block = render_block('/srv/pocol-explorer/site-000000000000', 'http://' + listen, gateway)
    root = str(Path(site_root).resolve()).replace('\\', '/')
    if any(c in root for c in ' "`{}\n'):
        raise ValueError('The test site root needs a plain path')
    return '{\n    admin off\n    persist_config off\n}\n\n' + block.replace('/srv/pocol-explorer/site-000000000000', root, 1)


def is_stock_caddyfile(text):
    """The Caddyfile the Debian/Ubuntu caddy package installs (`:80` serving /usr/share/caddy), comments aside."""
    tokens = ' '.join(line.split('#', 1)[0] for line in text.splitlines()).split()
    return tokens == [':80', '{', 'root', '*', '/usr/share/caddy', 'file_server', '}']


def plan_caddyfile(text, site_root, studio=None):
    """(new Caddyfile, previous site root or None) with the managed block serving `site_root`. `studio` is the Studio
    installer module (ROUTE, MARKER, patch_caddy): its route in the managed block is kept. A fresh server's stock file
    is replaced, other sites are kept, and anything else that names this host or the gateway port is refused."""
    block = render_block(site_root)
    if BEGIN not in text and END not in text:
        if HOST in text:
            raise RuntimeError(f'An unmanaged {HOST} Caddy site exists; review it first (no changes made)')
        if '8088' in text:
            raise RuntimeError('Another Caddy route reaches the explorer gateway port; review it first')
        if not text.strip() or is_stock_caddyfile(text):
            return block, None
        return text.rstrip('\n') + '\n\n' + block, None
    if text.count(BEGIN) != 1 or text.count(END) != 1 or text.index(BEGIN) > text.index(END):
        raise RuntimeError('The managed explorer Caddy block markers are damaged; review it first')
    start, stop = text.index(BEGIN), text.index(END) + len(END) + 1
    before, region, after = text[:start], text[start:stop], text[stop:]
    if not region.endswith('\n'):
        region, after = region + '\n', after
    outside = before + after
    if HOST in outside or '8088' in outside:
        raise RuntimeError(f'Another Caddy site or route uses {HOST} or the gateway port; review it first')
    has_studio = studio is not None and studio.MARKER in region
    normalized = region
    if has_studio:
        if region.count(studio.ROUTE) != 1:
            raise RuntimeError('The Studio route in the explorer block has custom edits; review it first')
        normalized = region.replace(studio.ROUTE, '', 1).replace('max_size 131072', 'max_size 16384', 1)
    roots = SITE_ROOT.findall(normalized)
    if len(roots) != 1 or normalized != render_block(roots[0]):
        raise RuntimeError('The managed explorer Caddy block has custom edits; review it first (no changes made)')
    new = before + block + after
    if has_studio:
        new = studio.patch_caddy(new)
    return new, roots[0]


def _refs(page):
    return re.findall(r'<script\b[^>]*\bsrc="([^"]+)"', page) + re.findall(r'<link\b[^>]*\bhref="([^"]+)"', page)


def site_files(directory):
    """{posix path: bytes} of a static copy, or ValueError naming what is refused."""
    root = Path(directory)
    if root.is_symlink() or not root.is_dir():
        raise ValueError(f'{directory}: not a directory')
    files, total = {}, 0
    stack = [(root, PurePosixPath())]
    while stack:
        folder, rel_folder = stack.pop()
        with os.scandir(folder) as entries:
            for entry in sorted(entries, key=lambda e: e.name):
                rel = rel_folder / entry.name
                if not SEGMENT.fullmatch(entry.name):
                    raise ValueError(f'{rel}: a hidden or unexpected name')
                if entry.is_symlink():
                    raise ValueError(f'{rel}: a symbolic link')
                if entry.is_dir(follow_symlinks=False):
                    if len(rel.parts) >= 3:
                        raise ValueError(f'{rel}: nested too deep')
                    stack.append((Path(entry.path), rel))
                    continue
                if not entry.is_file(follow_symlinks=False):
                    raise ValueError(f'{rel}: not a regular file')
                reason = deny_reason('/' + str(rel))
                if reason or PurePosixPath(entry.name).suffix.lower() not in EXTENSIONS:
                    raise ValueError(f'{rel}: never served ({reason or "file type"})')
                size = entry.stat(follow_symlinks=False).st_size
                total += size
                if size > MAX_FILE or total > MAX_TOTAL or len(files) >= MAX_FILES:
                    raise ValueError(f'{rel}: the copy exceeds {MAX_FILES} files, {MAX_FILE} bytes a file or {MAX_TOTAL} in all')
                files[str(rel)] = Path(entry.path).read_bytes()
    if 'index.html' not in files:
        raise ValueError('the copy has no index.html')
    for ref in _refs(files['index.html'].decode('utf-8', 'replace')):
        if re.match(r'(?:[a-z]+:)?//', ref):
            continue
        target = ref.split('?')[0].split('#')[0].lstrip('/')
        if target not in files:
            raise ValueError(f'index.html references {ref}, which is not in the copy')
    return files


def manifest(files):
    """The copy's SHA256SUMS-style manifest (sorted) and its site id (the first 12 hex of the manifest's SHA-256)."""
    text = ''.join(f'{hashlib.sha256(files[name]).hexdigest()}  {name}\n' for name in sorted(files))
    return text, hashlib.sha256(text.encode()).hexdigest()[:12]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    sub = ap.add_subparsers(dest='command', required=True)
    m = sub.add_parser('manifest'); m.add_argument('--site', required=True)
    t = sub.add_parser('test-config')
    for name in ('--site', '--listen', '--gateway', '--out'):
        t.add_argument(name, required=True)
    a = ap.parse_args(argv)
    try:
        files = site_files(a.site)
        text, ident = manifest(files)
        if a.command == 'manifest':
            print(text + f'site-{ident}', end='\n')
        else:
            Path(a.out).write_text(loopback_config(a.site, a.listen, a.gateway), encoding='utf-8')
            print(f'wrote {a.out} for site-{ident} ({len(files)} files)')
    except (ValueError, OSError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
