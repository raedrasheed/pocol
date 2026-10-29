"""Reference model of M1 section 6 (lookup, omnibox, navigation,
document-relative references), draft 0.3. Specification tooling only.

Change from draft 0.2 (C04, R3-02): the @v selector is validated lexically
before any integer conversion, so an arbitrarily long digit string is always
malformedSelector and never reaches an integer parser.

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
SELECTOR_RE = re.compile(r'(.*)@v([0-9]+)\Z', re.S)     # [0-9] is ASCII only
U32_MAX_DIGITS = '4294967295'                            # 2**32 - 1


def selector_value(digits):
    """R3-02 lexical check on an ASCII digit string. Returns the u32 value or None.

    Malformed: leading '0' (includes '0'); more than 10 digits; 10 digits
    comparing greater than '4294967295'. Conversion happens only afterwards,
    on at most 10 digits.
    """
    if digits[0] == '0' or len(digits) > len(U32_MAX_DIGITS):
        return None
    if len(digits) == len(U32_MAX_DIGITS) and digits > U32_MAX_DIGITS:
        return None
    return int(digits)


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
        version = selector_value(m.group(2))
        if version is None:
            return {'kind': 'malformedSelector'}
        comp = m.group(1)
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
