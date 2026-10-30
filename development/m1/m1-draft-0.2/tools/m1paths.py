"""Reference model of M1 draft 0.2 section 4 (lookup, omnibox, navigation,
document-relative references). Specification tooling only.

Results are dicts:
  {'kind': 'lookup', 'path': '/x.html' | None (entry), 'version': n | None}
  {'kind': '404', 'rule': ...}
  {'kind': 'malformedSelector'} | {'kind': 'schemaError'}     (no RPC, no frame)
  {'kind': 'fragmentOnly'} | {'kind': 'scheme', 'scheme': s} | {'kind': 'networkPath'}
"""

import re

PATH_MAX = 256
SEG_RE = re.compile(r'[A-Za-z0-9._~-]+\Z')
SCHEME_RE = re.compile(r'[A-Za-z][A-Za-z0-9+.\-]*:')
SELECTOR_RE = re.compile(r'(.*)@v([0-9]+)\Z', re.S)
U32_MAX = 2 ** 32 - 1


def split_query_fragment(s):
    """P08 step 1: cut at the first '?' or '#', whichever comes first."""
    cut = min([i for i in (s.find('?'), s.find('#')) if i >= 0], default=len(s))
    return s[:cut]


def lookup_path(p):
    """B10 stage N applied to a component with query/fragment already removed.

    Returns {'kind': 'lookup', 'path': None} for the entry file.
    """
    if p in ('', '/'):
        return {'kind': 'lookup', 'path': None}
    if '%' in p:
        return {'kind': '404', 'rule': 'nav.percent'}
    if '//' in p:
        return {'kind': '404', 'rule': 'nav.emptySegment'}
    if not p.startswith('/'):
        return {'kind': '404', 'rule': 'nav.grammar'}
    segs = p[1:].split('/')
    if any(s in ('.', '..') for s in segs):
        return {'kind': '404', 'rule': 'nav.dotSegment'}
    if p.endswith('/'):
        p += 'index.html'
    if len(p.encode('ascii', 'replace')) > PATH_MAX:
        return {'kind': '404', 'rule': 'nav.length'}
    for s in p[1:].split('/'):
        if not SEG_RE.match(s):
            return {'kind': '404', 'rule': 'nav.grammar'}
    return {'kind': 'lookup', 'path': p}


def omnibox(rest):
    """`rest` is the input after '<0xaddr>' in 'pocol <profile>/<0xaddr>[/path][@v<n>]'.

    Order (F18, Codex correction): split query/fragment first, then parse a
    trailing @v<digits> in the remaining component only, then normalize.
    """
    comp = split_query_fragment(rest)
    version = None
    m = SELECTOR_RE.match(comp)
    if m:
        digits = m.group(2)
        if digits[0] == '0' or int(digits) > U32_MAX:   # zero, leading zero, overflow
            return {'kind': 'malformedSelector'}
        version, comp = int(digits), m.group(1)
    elif comp.endswith('@v'):
        return {'kind': 'malformedSelector'}
    if comp and not comp.startswith('/'):
        return {'kind': '404', 'rule': 'nav.grammar'}
    r = lookup_path(comp)
    if r['kind'] == 'lookup':
        r['version'] = version
    return r


def navigate(path_str):
    """site_navigate [pathStr]: baseline schema, then lookup. No version suffix."""
    if not (1 <= len(path_str) <= 2048) or not path_str.startswith('/') or \
            any(not 0x21 <= ord(c) <= 0x7e for c in path_str):
        return {'kind': 'schemaError'}                 # -32602 {path: 'params[0]'}
    r = lookup_path(split_query_fragment(path_str))
    if r['kind'] == 'lookup':
        r['version'] = 'session'
    return r


def _remove_dot_segments(base_dir, rel):
    """RFC 3986 merge + remove_dot_segments, except climbing above root is an error."""
    out = [s for s in base_dir.split('/')[1:-1]]
    parts = rel.split('/')
    for k, s in enumerate(parts):
        last = k == len(parts) - 1
        if s == '.':
            if last:
                out.append('')
            continue
        if s == '..':
            if not out:
                return None
            out.pop()
            if last:
                out.append('')
            continue
        out.append(s)
    return '/' + '/'.join(out)


def reference(ref, base_path):
    """Stage R: classify and resolve a reference found in HTML/CSS (F05).

    base_path is the manifest path of the document containing the reference.
    """
    if ref == '':
        return {'kind': 'lookup', 'path': base_path, 'version': 'session'}
    if ref.startswith('#'):
        return {'kind': 'fragmentOnly'}
    if SCHEME_RE.match(ref):
        return {'kind': 'scheme', 'scheme': ref.split(':', 1)[0].lower()}
    if ref.startswith('//'):
        return {'kind': 'networkPath'}
    if ref.startswith('?'):
        return {'kind': 'lookup', 'path': base_path, 'version': 'session'}
    comp = split_query_fragment(ref)
    if comp.startswith('/'):
        r = lookup_path(comp)                    # absolute-path: literal, no dot removal
    else:
        resolved = _remove_dot_segments(base_path, comp)
        if resolved is None:
            return {'kind': '404', 'rule': 'ref.aboveRoot'}
        r = lookup_path(resolved)
    if r['kind'] == 'lookup':
        r['version'] = 'session'
    return r
