"""Deterministic fixture generator for M1 draft 0.2.

Usage: python gen_fixtures.py <output-dir>
Writes JSON fixtures; never touches draft 0.1 files. No network, no EVM,
no third-party packages. Specification tooling only.
"""

import json
import sys
from pathlib import Path

from keccak import keccak256
import rlp_strict
from rlp_strict import uint
import m1model as M
import m1abi as A

hx = lambda b: '0x' + bytes(b).hex()
HTML = (b'<!doctype html><html><head><meta charset="utf-8"><title>PoCol</title></head>'
        b'<body><h1>PoCol</h1></body></html>\n')
FACTORY_NOTE = ('Chunk addresses assume factory 0x0000000000000000000000000000000000c0c005, '
                'a proposed expansion of the elided baseline value (U01). Regenerate if U01 changes.')


class Raw(bytes):
    """Pre-encoded RLP inserted verbatim (for deliberately non-canonical fixtures)."""


def enc(x):
    if isinstance(x, Raw):
        return bytes(x)
    if isinstance(x, list):
        body = b''.join(enc(i) for i in x)
        return rlp_strict._len_prefix(len(body), 0xc0, 0xf7) + body
    return rlp_strict.encode(x)


def file_item(path, mime, data, factory=M.FACTORY):
    if isinstance(path, str):
        path = path.encode('latin-1') if all(ord(c) < 256 for c in path) else path.encode()
    return [path, uint(mime), uint(len(data)), keccak256(data),
            [[M.chunk_address(p, factory), uint(len(p))] for p in M.split(data)]]


def manifest(files, entry=0, version=1):
    return enc([uint(version), uint(entry), files])


CODE_TABLE = {}


def register(data, factory=M.FACTORY):
    for p in M.split(data):
        CODE_TABLE[hx(M.chunk_address(p, factory))] = hx(M.runtime(p))


def addrs_of(files):
    out = []
    for f in files:
        for c in f[4]:
            a = hx(c[0])
            if a not in out:
                out.append(a)
    return out


# ---------------------------------------------------------------------------
def draft01_reproduction():
    """Re-create the four draft 0.1 vector files with this independent tooling."""
    factory = M.FACTORY
    vectors = []
    for name, data in [('single-byte', b'a'), ('max-chunk', b'\xaa' * 24575), ('html', HTML)]:
        rt, init = M.runtime(data), M.initcode(data)
        vectors.append({'id': name, 'factory': hx(factory), 'data': hx(data), 'dataLen': len(data),
                        'salt': hx(keccak256(data)), 'initcode': hx(init),
                        'initcodeHash': hx(keccak256(init)), 'runtime': hx(rt), 'runtimeLen': len(rt),
                        'address': hx(M.chunk_address(data, factory))})
    entry = file_item(b'/index.html', 1, HTML)
    m = manifest([entry])
    man = {'encodingDecision': 'P02 pending review', 'entryIndex': 0, 'rlp': hx(m), 'length': len(m),
           'hash': hx(keccak256(m)), 'fileHash': hx(keccak256(HTML))}
    import copy
    neg = []

    def add(i, changed, reason):
        neg.append({'id': i, 'manifest': hx(rlp_strict.encode(changed)), 'expected': 'reject',
                    'reason': reason})
    for i, field, value, why in [('unknown-mime', 1, uint(11), 'unknown MIME'),
                                 ('entry-css', 1, uint(2), 'entry must be HTML'),
                                 ('wrong-size', 2, uint(len(HTML) + 1), 'sum of chunk lengths differs from size'),
                                 ('wrong-content-hash', 3, bytes(32), 'reconstructed content hash mismatch'),
                                 ('dot-segment', 0, b'/./index.html', 'dot segment'),
                                 ('non-ascii', 0, '/صفحة.html'.encode(), 'non ASCII path')]:
        e = copy.deepcopy(entry); e[field] = value
        add(i, [uint(1), uint(0), [e]], why)
    add('duplicate-path', [uint(1), uint(0), [entry, entry]], 'strict path ordering violated')
    e = copy.deepcopy(entry); e[0] = b'/a.html'
    add('unsorted-path', [uint(1), uint(0), [entry, e]], 'strict path ordering violated')
    e = copy.deepcopy(entry); e[4][0][1] = uint(0)
    add('zero-chunk-length', [uint(1), uint(0), [e]], 'chunk length lower bound')
    neg += [{'id': 'trailing-byte', 'manifest': hx(m + b'\0'), 'expected': 'reject', 'reason': 'trailing input'},
            {'id': 'noncanonical-rlp', 'manifest': '0x8101', 'expected': 'reject',
             'reason': 'non-minimal scalar and not manifest shape'}]
    dump = lambda o: json.dumps(o, indent=2) + '\n'
    return {'chunks.json': dump(vectors).encode(), 'index.html': HTML,
            'manifest.json': dump(man).encode(), 'negative-manifests.json': dump(neg).encode()}


# ---------------------------------------------------------------------------
def big_manifest(target):
    """Valid manifest of exactly `target` bytes (<=256 files, paths <=256 B)."""
    a, h = M.chunk_address(b'a'), keccak256(b'a')

    def build(n, k_prev, k_last):
        ks = [240] * (n - 2) + [k_prev, k_last]
        files = [[('/p%03d-' % i + 'x' * ks[i]).encode(),
                  uint(1 if i == 0 else 10), uint(1), h, [[a, uint(1)]]] for i in range(n)]
        return manifest(files, 0), files
    for n in range(150, 257):
        if len(build(n, 250, 250)[0]) < target:
            continue
        for k_prev in range(250, -1, -1):
            for k in range(250, -1, -1):
                m, files = build(n, k_prev, k)
                if len(m) == target:
                    return m, files
                if len(m) < target:
                    break
    raise RuntimeError('no exact manifest for %d' % target)


def fixtures():
    register(HTML); register(b'a'); register(b'z\n'); register(b'')
    register(b'\xaa' * 24575)
    base = [file_item('/index.html', 1, HTML)]
    two = base + [file_item('/z.txt', 10, b'z\n')]
    pos, neg, ver = [], [], []

    def P(i, files, entry=0, desc='', boundary=None, raw=None):
        m = raw if raw is not None else manifest(files, entry)
        pos.append({'id': i, 'description': desc, 'boundary': boundary, 'manifest': hx(m),
                    'manifestLength': len(m), 'manifestKeccak256': hx(keccak256(m)),
                    'provider': addrs_of(files), 'mock': False,
                    'expected': {'result': 'accept'}})

    def N(i, rule, stage, files=None, entry=0, raw=None, isolation='full', next_rule=None,
          desc='', provider=None, overrides=None, mock=False, fetch_calls=None, boundary=None,
          baseline=None):
        m = raw if raw is not None else manifest(files, entry)
        neg.append({'id': i, 'rule': rule, 'stage': stage, 'description': desc,
                    'baselineCase': baseline, 'boundary': boundary,
                    'isolation': isolation, 'nextRuleWhenDisabled': next_rule,
                    'manifest': hx(m), 'manifestLength': len(m),
                    'provider': provider if provider is not None else (addrs_of(files) if files else []),
                    'providerOverrides': overrides or {}, 'mock': mock,
                    'expected': {'result': 'reject', 'firstRule': rule, 'stage': stage,
                                 'fetchCalls': fetch_calls if fetch_calls is not None
                                 else (0 if stage in ('decode', 'structure', 'semantic') else None)}})

    # ----- positive -----
    P('pos-minimal', base, desc='Draft 0.1 minimal site (byte-identical manifest).')
    P('pos-empty-file', [file_item('/empty.css', 2, b''), base[0]], entry=1,
      desc='F01: empty CSS file: size 0, chunks [], contentHash keccak256(empty).')
    P('pos-shared-chunk', [file_item('/a.html', 1, HTML), file_item('/b.html', 1, HTML)],
      desc='Two files reference the same chunk address (duplicate references allowed, U17).')
    P('pos-case-distinct', [file_item('/INDEX.html', 1, HTML), base[0]], entry=1,
      desc='Case-sensitive paths; "/INDEX.html" < "/index.html" bytewise.')
    P('pos-path-256', [file_item('/' + 'a' * 255, 1, HTML)], boundary='path 256 B (limit)')
    P('pos-path-special-segments', [file_item('/...', 1, HTML), file_item('/~a/b-c_d.e', 10, b'z\n')],
      desc='"..." and "~" segments are valid; "." and ".." are not.')
    P('pos-chunk-24575', [base[0], file_item('/max.bin', 10, b'\xaa' * 24575)],
      boundary='chunk 24575 B (limit)')
    f1m = b'\xaa' * 1048576
    register(f1m)
    P('pos-file-1048576', [file_item('/big.txt', 10, f1m), base[0]], entry=1,
      boundary='file 1048576 B = 42 x 24575 + 16426 (43 chunks)')
    many = [file_item('/f%03d' % i, 1 if i == 0 else 10, b'a') for i in range(256)]
    P('pos-files-256', many, boundary='256 files (limit)')
    site = [file_item('/f%d.txt' % i, 1 if i == 0 else 10, f1m) for i in range(4)]
    P('pos-site-4194304', site,
      boundary='site 4194304 B = 4 x 1048576 logical bytes; chunks shared, so unique bytes are ~1 MiB')
    m65536, files65536 = big_manifest(65536)
    P('pos-manifest-65536', files65536, raw=m65536, boundary='manifest 65536 B (limit)')

    # ----- decode -----
    m0 = manifest(base)
    N('neg-trailing-byte', 'decode.trailing', 'decode', base, raw=m0 + b'\x00', baseline='extra byte')
    body = b'\x81\x01' + enc(uint(0)) + enc([base[0]])
    N('neg-noncanonical-version', 'decode.noncanonical', 'decode', base,
      raw=bytes([0xf8, len(body)]) + body, desc='version 1 encoded as 0x8101 inside a valid manifest',
      baseline='non-canonical RLP')
    e = list(base[0]); e[0] = Raw(b'\xb8\x0b/index.html')
    N('neg-noncanonical-long-string', 'decode.noncanonical', 'decode', base, raw=manifest([e]),
      desc='11-byte path in long form 0xb8 0x0b')
    e = list(base[0]); c = base[0][4][0]
    e[4] = [Raw(b'\xf8\x16' + enc(c[0]) + enc(c[1]))]
    N('neg-noncanonical-long-list', 'decode.noncanonical', 'decode', base, raw=manifest([e]),
      desc='22-byte chunk pair in long list form 0xf8 0x16')
    N('neg-length-leading-zero', 'decode.noncanonical', 'decode', base,
      raw=b'\xf9\x00' + m0[1:], desc='top-level length 0x004d with a leading zero byte')
    N('neg-truncated', 'decode.truncated', 'decode', base, raw=m0[:-1], isolation='none')
    m65537, f65537 = big_manifest(65537)
    N('neg-manifest-65537', 'decode.oversize', 'decode', f65537, raw=m65537,
      boundary='manifest 65537 B')
    N('neg-draft01-noncanonical-rlp', 'decode.noncanonical', 'decode', raw=bytes.fromhex('8101'),
      isolation='next', next_rule='struct.shape',
      desc='Draft 0.1 fixture kept for traceability; also not a list (two faults).')

    # ----- structure -----
    N('neg-top-arity-2', 'struct.shape', 'structure', raw=enc([uint(1), uint(0)]), isolation='none')
    N('neg-top-arity-4', 'struct.shape', 'structure', base,
      raw=enc([uint(1), uint(0), [base[0]], b'']), isolation='none')
    N('neg-file-arity-4', 'struct.shape', 'structure', base, raw=manifest([base[0][:4]]), isolation='none')
    N('neg-file-arity-6', 'struct.shape', 'structure', base, raw=manifest([base[0] + [b'']]),
      isolation='none')
    N('neg-files-not-list', 'struct.shape', 'structure', raw=enc([uint(1), uint(0), b'x']),
      isolation='none')
    e = list(base[0]); e[1] = [uint(1)]
    N('neg-int-is-list', 'struct.shape', 'structure', base, raw=manifest([e]), isolation='none')
    e = list(base[0]); e[2] = b'\x00' + uint(len(HTML))
    N('neg-int-leading-zero', 'struct.int', 'structure', base, raw=manifest([e]),
      desc='size 111 encoded as 0x00 0x6f')
    N('neg-zero-as-00', 'struct.int', 'structure', base,
      raw=enc([uint(1), b'\x00', [base[0]]]), desc='entryIndex 0 encoded as byte 0x00 instead of 0x80')
    e = list(two[1]); e[1] = uint(256)
    N('neg-mime-u8-overflow', 'struct.int', 'structure', two, raw=manifest([two[0], e]),
      isolation='next', next_rule='file.mime')
    e = list(base[0]); e[2] = uint(2 ** 32)
    N('neg-size-u32-overflow', 'struct.int', 'structure', base, raw=manifest([e]),
      isolation='next', next_rule='file.size')
    for n in (31, 33):
        e = list(base[0]); e[3] = (base[0][3] + b'\x00')[:n]
        N('neg-hash-%d' % n, 'struct.width', 'structure', base, raw=manifest([e]),
          isolation='next', next_rule='file.hash')
    for n in (19, 21):
        e = list(base[0]); a = base[0][4][0][0]
        e[4] = [[(a + b'\x00')[:n], base[0][4][0][1]]]
        N('neg-addr-%d' % n, 'struct.width', 'structure', base, raw=manifest([e]),
          isolation='next', next_rule='fetch.missing')

    # ----- semantic -----
    N('neg-version-0', 'manifest.version', 'semantic', base, raw=enc([uint(0), uint(0), [base[0]]]))
    N('neg-version-2', 'manifest.version', 'semantic', base, raw=enc([uint(2), uint(0), [base[0]]]))
    N('neg-files-empty', 'manifest.fileCount', 'semantic', raw=enc([uint(1), uint(0), []]),
      isolation='next', next_rule='manifest.entryIndex')
    many257 = [file_item('/f%03d' % i, 1 if i == 0 else 10, b'a') for i in range(257)]
    N('neg-files-257', 'manifest.fileCount', 'semantic', many257, boundary='257 files')
    N('neg-entry-index-range', 'manifest.entryIndex', 'semantic', base, entry=1)
    for i, p, rule in [('no-slash', 'index.html', 'path.grammar'), ('root', '/', 'path.grammar'),
                       ('trailing-slash', '/a/', 'path.grammar'), ('leading-double-slash', '//a', 'path.grammar'),
                       ('empty-segment', '/a//b', 'path.grammar'), ('space', '/a b', 'path.grammar'),
                       ('percent', '/a%20b', 'path.grammar'), ('backslash', '/a\\b', 'path.grammar'),
                       ('non-ascii', '/صفحة.html', 'path.grammar'),
                       ('dot', '/./index.html', 'path.dotSegment'), ('dotdot-tail', '/a/..', 'path.dotSegment'),
                       ('dotdot-lead', '/..', 'path.dotSegment'),
                       ('length-257', '/' + 'a' * 256, 'path.length'), ('empty', '', 'path.length')]:
        N('neg-path-' + i, rule, 'semantic', [file_item(p, 1, HTML)],
          boundary='path 257 B' if i == 'length-257' else None, baseline='path grammar (FD:L895)')
    N('neg-duplicate-path', 'path.order', 'semantic', base + base, baseline='duplicate path')
    N('neg-unsorted-path', 'path.order', 'semantic', [base[0], file_item('/a.html', 1, HTML)],
      baseline='unsorted paths')
    N('neg-case-unsorted', 'path.order', 'semantic', [file_item('/a.html', 1, HTML), file_item('/B.html', 1, HTML)],
      desc='0x42 "B" sorts before 0x61 "a"')
    e = list(two[1]); e[1] = uint(11)
    N('neg-unknown-mime', 'file.mime', 'semantic', two, raw=manifest([two[0], e]),
      desc='MIME 11 on a non-entry file; entry stays valid', baseline='unknown mime')
    e = list(two[1]); e[1] = uint(0)
    N('neg-mime-zero', 'file.mime', 'semantic', two, raw=manifest([two[0], e]))
    N('neg-entry-css', 'entry.mime', 'semantic', [file_item('/index.html', 2, HTML)],
      baseline='entry not html')
    N('neg-draft01-unknown-mime-entry', 'file.mime', 'semantic', [file_item('/index.html', 11, HTML)],
      isolation='next', next_rule='entry.mime', desc='Draft 0.1 fixture; MIME 11 on the entry (two faults).')
    f1m1 = f1m + b'\xaa'
    register(f1m1)
    N('neg-file-1048577', 'file.size', 'semantic', [file_item('/big.txt', 10, f1m1), base[0]], entry=1,
      boundary='file 1048577 B = 42 x 24575 + 16427 (still 43 chunks)')
    e = list(base[0]); e[4] = [base[0][4][0], [base[0][4][0][0], uint(0)]]
    N('neg-chunk-len-0', 'chunk.len', 'semantic', base, raw=manifest([e]), isolation='next',
      next_rule='fetch.length', desc='extra [addr, 0] keeps the sum equal to size',
      baseline='len mismatch / lower bound')
    big = b'\xbb' * 24576
    e = [b'/index.html', uint(1), uint(24576), keccak256(big), [[M.chunk_address(big), uint(24576)]]]
    N('neg-chunk-len-24576', 'chunk.len', 'semantic', provider=[], raw=manifest([e]), isolation='next',
      next_rule='fetch.missing', boundary='chunk 24576 B')
    e = list(base[0]); e[4] = [[base[0][4][0][0], uint(0)]]
    N('neg-draft01-zero-chunk-length', 'chunk.len', 'semantic', base, raw=manifest([e]),
      isolation='next', next_rule='file.sum', desc='Draft 0.1 fixture (three faults).')
    e = list(base[0]); e[2] = uint(len(HTML) + 1)
    N('neg-wrong-size', 'file.sum', 'semantic', base, raw=manifest([e]), baseline='size mismatch')
    a1 = M.chunk_address(b'a')
    e = [b'/empty.css', uint(2), uint(0), keccak256(b''), [[a1, uint(1)]]]
    N('neg-empty-with-chunk', 'file.sum', 'semantic', provider=[hx(a1)] + addrs_of(base),
      raw=manifest([e, base[0]], 1), isolation='next', next_rule='file.hash')
    e = [b'/a.css', uint(2), uint(1), keccak256(b'a'), []]
    N('neg-nonempty-no-chunks', 'file.sum', 'semantic', provider=addrs_of(base),
      raw=manifest([e, base[0]], 1), isolation='next', next_rule='file.hash')
    N('neg-site-4194305', 'site.size', 'semantic', site + [file_item('/z.txt', 10, b'a')],
      boundary='site 4194305 B')

    # ----- fetch / hash (mock providers) -----
    ha = hx(M.chunk_address(HTML))
    N('neg-fetch-missing', 'fetch.missing', 'fetch', base, provider=[], isolation='next',
      next_rule='fetch.prefix', mock=True, fetch_calls=1, desc='eth_getCode returns 0x')
    N('neg-fetch-prefix', 'fetch.prefix', 'fetch', base, provider=[],
      overrides={ha: hx(b'\x01' + HTML)}, mock=True, fetch_calls=1, baseline='code[0] != 0')
    N('neg-fetch-length', 'fetch.length', 'fetch', base, provider=[],
      overrides={ha: hx(b'\x00' + HTML + b'\x00')}, mock=True, isolation='next',
      next_rule='fetch.origin', fetch_calls=1, baseline='len mismatch')
    other = file_item('/index.html', 1, HTML, M.OTHER_FACTORY)
    N('neg-fetch-other-factory', 'fetch.origin', 'fetch', [other], provider=[],
      overrides={hx(other[4][0][0]): hx(b'\x00' + HTML)}, mock=True, fetch_calls=1,
      desc='identical bytes deployed by a factory at 0x...c0c006', baseline='another factory')
    mod = bytearray(HTML); mod[20] ^= 0x01
    N('neg-fetch-modified-byte', 'fetch.origin', 'fetch', base, provider=[],
      overrides={ha: hx(b'\x00' + bytes(mod))}, mock=True, isolation='next', next_rule='file.hash',
      fetch_calls=1, desc='address re-derivation rejects before the file hash (Codex correction)')
    e = list(base[0]); e[3] = bytes(32)
    N('neg-file-hash', 'file.hash', 'hash', base, raw=manifest([e]), fetch_calls=1,
      desc='valid chunk at its correct address; only the declared hash is wrong',
      baseline='hash mismatch')

    # ----- manifest retrieval stage (Website record) -----
    def rec(m, chunks=None):
        cs, prov = M.manifest_chunks(m)
        for a, code in prov.items():
            CODE_TABLE[hx(a)] = hx(code)
        cs = chunks or cs
        return {'manifestHash': hx(keccak256(m)), 'manifestLen': len(m),
                'chunks': [[hx(a), ln] for a, ln in cs]}

    r = rec(m0)
    ver.append({'id': 'ver-minimal', 'record': r, 'provider': [c[0] for c in r['chunks']] + addrs_of(base),
                'expected': {'result': 'accept'}, 'note': 'createVersion inputs for T1-02'})
    r = rec(m65536)
    ver.append({'id': 'ver-manifest-65536', 'record': r,
                'provider': [c[0] for c in r['chunks']] + addrs_of(files65536),
                'expected': {'result': 'accept'}, 'note': 'canonical split [24575, 24575, 16386]'})
    r = rec(m0); r['manifestHash'] = hx(bytes(32))
    ver.append({'id': 'ver-manifest-hash', 'record': r,
                'provider': [c[0] for c in r['chunks']] + addrs_of(base),
                'expected': {'result': 'reject', 'firstRule': 'manifest.hash', 'stage': 'manifest'},
                'isolation': 'full'})
    r = rec(m0)
    ver.append({'id': 'ver-manifest-chunk-missing', 'record': r, 'provider': [],
                'expected': {'result': 'reject', 'firstRule': 'mfetch.missing', 'stage': 'manifest'},
                'isolation': 'next', 'nextRuleWhenDisabled': 'mfetch.prefix', 'mock': True})
    p1, p2 = m0[:40], m0[40:]
    for p in (p1, p2):
        CODE_TABLE[hx(M.chunk_address(p))] = hx(M.runtime(p))
    r = rec(m0, [(M.chunk_address(p1), 40), (M.chunk_address(p2), len(p2))])
    ver.append({'id': 'ver-manifest-noncanonical-split', 'record': r,
                'provider': [c[0] for c in r['chunks']] + addrs_of(base),
                'expected': {'result': 'reject', 'firstRule': 'manifest.split', 'stage': 'manifest'},
                'isolation': 'full', 'note': 'proposed restriction U08'})
    r = rec(m0); r['manifestLen'] = 0; r['chunks'] = []
    ver.append({'id': 'ver-manifest-length-0', 'record': r, 'provider': [],
                'expected': {'result': 'reject', 'firstRule': 'manifest.length', 'stage': 'manifest'},
                'isolation': 'next', 'nextRuleWhenDisabled': 'manifest.hash'})
    return pos, neg, ver


# ---------------------------------------------------------------------------
def path_cases():
    """Hand-written expectations (M1-SPEC-0.2 section 4). The model is checked
    against these; they are not generated by the model."""
    L = lambda p, v=None: {'kind': 'lookup', 'path': p, 'version': v}
    F = lambda r: {'kind': '404', 'rule': r}
    S = 'session'
    om = [
        ('', L(None)), ('/', L(None)), ('/index.html?x=1#part', L('/index.html')),
        ('/docs/', L('/docs/index.html')), ('/a//b', F('nav.emptySegment')), ('//x', F('nav.emptySegment')),
        ('/./a', F('nav.dotSegment')), ('/a/../b', F('nav.dotSegment')), ('/%61', F('nav.percent')),
        ('/a%2fb', F('nav.percent')), ('/INDEX.html', L('/INDEX.html')), ('/a#b?c', L('/a')),
        ('/index.html?q=%20', L('/index.html')), ('/a\\b', F('nav.grammar')), ('/a;b', F('nav.grammar')),
        ('/docs', L('/docs')), ('/' + 'x' * 245 + '/', F('nav.length')),
        ('/' + 'x' * 244 + '/', L('/' + 'x' * 244 + '/index.html')),
        ('/a.html@v2', L('/a.html', 2)), ('/a.html@v2?x=1', L('/a.html', 2)),
        ('/a.html?x=@v2', L('/a.html')), ('@v3', L(None, 3)), ('/@v3', L(None, 3)),
        ('/a.html@v0', {'kind': 'malformedSelector'}), ('/a.html@v01', {'kind': 'malformedSelector'}),
        ('/a.html@v4294967295', L('/a.html', 4294967295)),
        ('/a.html@v4294967296', {'kind': 'malformedSelector'}), ('/a.html@v', {'kind': 'malformedSelector'}),
        ('/a@vx', F('nav.grammar')), ('/a@v1@v2', F('nav.grammar')),
    ]
    nav = [
        ('/', L(None, S)), ('/x/', L('/x/index.html', S)), ('/about.html', L('/about.html', S)),
        ('/a/../b.html', F('nav.dotSegment')), ('/a%2fb', F('nav.percent')), ('//x', F('nav.emptySegment')),
        ('about.html', {'kind': 'schemaError'}), ('/a b', {'kind': 'schemaError'}),
        ('/' + 'a' * 2048, {'kind': 'schemaError'}), ('/a.html@v2', F('nav.grammar')),
        ('/a.html?x#y', L('/a.html', S)),
    ]
    ref = [
        ('../style.css', '/docs/page.html', L('/style.css', S)),
        ('img/a.png', '/docs/page.html', L('/docs/img/a.png', S)),
        ('./', '/docs/page.html', L('/docs/index.html', S)),
        ('..', '/docs/page.html', L(None, S)),
        ('../../x.html', '/a.html', F('ref.aboveRoot')),
        ('/a/../b.html', '/docs/page.html', F('nav.dotSegment')),
        ('a//b.html', '/page.html', F('nav.emptySegment')),
        ('x%20y.png', '/page.html', F('nav.percent')),
        ('style.css?v=2#k', '/page.html', L('/style.css', S)),
        ('?q=1', '/docs/page.html', L('/docs/page.html', S)),
        ('#top', '/page.html', {'kind': 'fragmentOnly'}),
        ('', '/page.html', L('/page.html', S)),
        ('https://example.org/', '/page.html', {'kind': 'scheme', 'scheme': 'https'}),
        ('data:image/png;base64,AA==', '/page.html', {'kind': 'scheme', 'scheme': 'data'}),
        ('JavaScript:alert(1)', '/page.html', {'kind': 'scheme', 'scheme': 'javascript'}),
        ('//evil.example/x.js', '/page.html', {'kind': 'networkPath'}),
        ('a\\b.css', '/page.html', F('nav.grammar')),
    ]
    return {'omnibox': [{'input': i, 'expected': e} for i, e in om],
            'navigate': [{'input': i, 'expected': e} for i, e in nav],
            'reference': [{'input': i, 'base': b, 'expected': e} for i, b, e in ref]}


# ---------------------------------------------------------------------------
def abi_fixture(ver_min):
    ex = {'A': bytes.fromhex('00000000000000000000000000000000000000a1'),
          'B': bytes.fromhex('00000000000000000000000000000000000000b2')}
    addr_m = bytes.fromhex(ver_min['record']['chunks'][0][0][2:])
    vs = A.version_slots(1, 1)
    return {
        'status': 'PROPOSED identifiers computed from proposed signatures and layout P13; '
                  'not compiler output (experiment E-04).',
        'factory': {'functions': {s: A.selector(s) for s in A.FACTORY_FUNCTIONS},
                    'errors': {s: A.selector(s) for s in A.FACTORY_ERRORS},
                    'revertData': {'ChunkLength(0)': A.revert_data('ChunkLength(uint256)', 0),
                                   'ChunkLength(24576)': A.revert_data('ChunkLength(uint256)', 24576)}},
        'website': {'functions': {s: A.selector(s) for s in A.WEBSITE_FUNCTIONS},
                    'errors': {s: A.selector(s) for s in A.WEBSITE_ERRORS},
                    'eventTopic0': {s: A.topic(s) for s in A.WEBSITE_EVENTS}},
        'slots': {
            'fixed': {'owner': 0, 'pendingOwner': 1, 'versionCount|currentVersion': 2,
                      'versions(mapping)': 3, 'publishers(mapping)': 4, 'publisherCount': 5},
            'formulas': {
                'Version[id].base': 'keccak256(be256(id) || be256(3))',
                'Version[id] word base+1': 'manifestLen bits 0-31 | status bits 32-39 | publishedBlock bits 40-103',
                'Version[id].chunks.length': 'base + 2',
                'Version[id].chunks[i]': 'keccak256(be256(base + 2)) + i; addr bits 0-159 | len bits 160-191',
                'publishers[a]': 'keccak256(pad32(a) || be256(4)); value 1 = enabled',
                'slot 2': 'versionCount bits 0-31 | currentVersion bits 32-63'},
            'version': {str(v): {k: (A.h32(x) if isinstance(x, int) else [A.h32(y) for y in x])
                                 for k, x in A.version_slots(v, 3).items()} for v in (1, 2, 1024)},
            'examplePublisherSlots(placeholder addresses, not test accounts)':
                {k + '=' + hx(a): A.h32(A.publisher_slot(a)) for k, a in ex.items()},
        },
        't1_02_expectedState': {
            'description': 'After: deploy(owner=A); setPublisher(B,true); B: createVersion(ver-minimal); '
                           'publish(1); setCurrent(1). <A>, <B>, <publishBlock> are bound at run time.',
            'slot0': 'pad32(<A>)', 'slot1': A.h32(0),
            'slot2': A.h32(A.pack_counts(1, 1)),
            'publishers[<A>]': A.h32(1), 'publishers[<B>]': A.h32(1), 'slot5': A.h32(2),
            'Version[1].base': A.h32(vs['manifestHash']),
            'value@Version[1].base': ver_min['record']['manifestHash'],
            'value@base+1': '0x' + '00' * 19 + '<publishBlock as 8 bytes>' + '01' + '0000004f',
            'value@base+1 if publishBlock=1': A.h32(A.pack_version_word(79, 1, 1)),
            'value@base+2': A.h32(1),
            'slot of chunks[0]': A.h32(vs['chunks[i]'][0]),
            'value@chunks[0]': A.h32(A.pack_chunk(addr_m, 79)),
        },
    }


def main(out):
    out = Path(out)
    (out / 'draft01-repro').mkdir(parents=True, exist_ok=True)
    for name, data in draft01_reproduction().items():
        (out / 'draft01-repro' / name).write_bytes(data)
    pos, neg, ver = fixtures()
    hdr = {'specVersion': 'M1 draft 0.2', 'factoryAssumption': FACTORY_NOTE}
    dump = lambda o: json.dumps(o, indent=2, ensure_ascii=True) + '\n'
    (out / 'manifest-positive.json').write_text(dump({**hdr, 'fixtures': pos}))
    (out / 'manifest-negative.json').write_text(dump({**hdr, 'fixtures': neg}))
    (out / 'version-records.json').write_text(dump({**hdr, 'fixtures': ver}))
    (out / 'code-table.json').write_text(dump({**hdr, 'note': 'Honest chunk code: runtime 0x00||data at '
                                               'its factory-derived address. Mock overrides live in the fixtures.',
                                               'codes': dict(sorted(CODE_TABLE.items()))}))
    (out / 'path-cases.json').write_text(dump({'specVersion': 'M1 draft 0.2', **path_cases()}))
    (out / 'abi-and-slots.json').write_text(dump(abi_fixture(ver[0])))


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else 'vectors')
