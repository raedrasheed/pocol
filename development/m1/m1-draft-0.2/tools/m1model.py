"""Executable reference model of the M1 draft 0.2 chunk and manifest rules.

Specification tooling only: it exists to make the fixtures falsifiable.
It is not the production publisher, viewer, or contract, and it has not
been reviewed as such.

Rule IDs and their precedence are defined in M1-SPEC-0.2.md section 3.
"""

import re

from keccak import keccak256, keccak256_cached
import rlp_strict
from rlp_strict import RlpError

# --- Constants (B = baseline, P = proposal; see M1-SPEC-0.2.md) -----------
CHUNK_MAX = 24575            # B01/B07
RUNTIME_MAX = CHUNK_MAX + 1  # B02
FILE_MAX = 1048576           # B07
MANIFEST_MAX = 65536         # B07 / FD contract limit
FILES_MAX = 256              # B07 client limit
SITE_MAX = 4194304           # B07 client limit; definition = sum of file sizes (P-SITE)
PATH_MAX = 256               # B05
MIME_MAX = 10                # B06
MIME_HTML = 1
U8, U32 = 8, 32

# Proposed expansion of the elided baseline address 0x...C0C005 (U01).
FACTORY = bytes.fromhex('0000000000000000000000000000000000c0c005')
OTHER_FACTORY = bytes.fromhex('0000000000000000000000000000000000c0c006')  # mock only

INIT_PREFIX = bytes.fromhex('80600a5f395ff3')
SEG_RE = re.compile(rb'[A-Za-z0-9._~-]+')

STAGES = ('manifest', 'decode', 'structure', 'semantic', 'fetch', 'hash')


class Reject(Exception):
    def __init__(self, rule, stage, detail=''):
        super().__init__(rule, stage, detail)
        self.rule, self.stage, self.detail = rule, stage, detail


# --- Chunks ---------------------------------------------------------------
def runtime(data):
    return b'\x00' + data


def initcode(data):
    rt = runtime(data)
    return b'\x61' + len(rt).to_bytes(2, 'big') + INIT_PREFIX + rt


def chunk_address(data, factory=FACTORY):
    salt = keccak256_cached(data)
    return keccak256(b'\xff' + factory + salt + keccak256_cached(initcode(data)))[12:]


def split(data):
    """P04: original byte order, maximal chunks, last may be shorter; empty -> []."""
    return [data[i:i + CHUNK_MAX] for i in range(0, len(data), CHUNK_MAX)]


# --- Publisher-side manifest construction (canonical) ---------------------
def build_manifest(files, entry_index, factory=FACTORY):
    """files: list of (path:str|bytes, mime:int, data:bytes) already sorted.

    Returns (manifest_bytes, provider) where provider maps chunk address to
    runtime code for every referenced chunk.
    """
    provider, items = {}, []
    for path, mime, data in files:
        if isinstance(path, str):
            path = path.encode('ascii')
        chunks = []
        for part in split(data):
            a = chunk_address(part, factory)
            provider[a] = runtime(part)
            chunks.append([a, rlp_strict.uint(len(part))])
        items.append([path, rlp_strict.uint(mime), rlp_strict.uint(len(data)),
                      keccak256_cached(data), chunks])
    m = rlp_strict.encode([rlp_strict.uint(1), rlp_strict.uint(entry_index), items])
    return m, provider


def manifest_chunks(manifest, factory=FACTORY):
    """Canonical split of the manifest itself (P04 applied to manifest bytes)."""
    out, provider = [], {}
    for part in split(manifest):
        a = chunk_address(part, factory)
        provider[a] = runtime(part)
        out.append((a, len(part)))
    return out, provider


# --- Validation -----------------------------------------------------------
def _int(x, bits, disabled):
    if not isinstance(x, bytes):
        raise Reject('struct.shape', 'structure', 'integer field is a list')
    if x[:1] == b'\x00' and 'struct.int' not in disabled:
        raise Reject('struct.int', 'structure', 'leading zero')
    v = int.from_bytes(x, 'big') if x else 0
    if v >> bits and 'struct.int' not in disabled:
        raise Reject('struct.int', 'structure', 'exceeds u%d' % bits)
    return v


def _list(x, n):
    if not isinstance(x, list) or (n is not None and len(x) != n):
        raise Reject('struct.shape', 'structure', 'expected list of %s' % n)


def _bytes(x):
    if not isinstance(x, bytes):
        raise Reject('struct.shape', 'structure', 'expected byte string')


def _structure(m, disabled):
    """Pass 1: shapes, types, integer form/width, fixed byte widths."""
    _list(m, 3)
    version = _int(m[0], U8, disabled)
    entry = _int(m[1], U32, disabled)
    _list(m[2], None)
    files = []
    for f in m[2]:
        _list(f, 5)
        path, mime, size, h, chunks = f
        _bytes(path)
        mime = _int(mime, U8, disabled)
        size = _int(size, U32, disabled)
        _bytes(h)
        if len(h) != 32 and 'struct.width' not in disabled:
            raise Reject('struct.width', 'structure', 'contentHash length %d' % len(h))
        _list(chunks, None)
        cs = []
        for c in chunks:
            _list(c, 2)
            _bytes(c[0])
            if len(c[0]) != 20 and 'struct.width' not in disabled:
                raise Reject('struct.width', 'structure', 'address length %d' % len(c[0]))
            cs.append((c[0], _int(c[1], U32, disabled)))
        files.append({'path': path, 'mime': mime, 'size': size, 'hash': h, 'chunks': cs})
    return version, entry, files


def path_rule(path):
    """B05 manifest path grammar. Returns None or the rule ID."""
    if len(path) == 0 or len(path) > PATH_MAX:
        return 'path.length'
    if path[:1] != b'/':
        return 'path.grammar'
    for seg in path[1:].split(b'/'):
        if seg in (b'.', b'..'):
            return 'path.dotSegment'
        if not SEG_RE.fullmatch(seg):          # also covers empty and non-ASCII
            return 'path.grammar'
    return None


def _semantic(version, entry, files, disabled):
    """Pass 2: everything decidable without fetching any chunk."""
    def chk(cond, rule, detail=''):
        if cond and rule not in disabled:
            raise Reject(rule, 'semantic', detail)

    chk(version != 1, 'manifest.version', str(version))
    chk(len(files) == 0 or len(files) > FILES_MAX, 'manifest.fileCount', str(len(files)))
    chk(entry >= len(files), 'manifest.entryIndex', str(entry))
    prev, total = None, 0
    for i, f in enumerate(files):
        r = path_rule(f['path'])
        if r:
            chk(True, r, repr(f['path'][:40]))
        chk(prev is not None and not f['path'] > prev, 'path.order', 'file %d' % i)
        prev = f['path']
        chk(not 1 <= f['mime'] <= MIME_MAX, 'file.mime', 'file %d' % i)
        chk(i == entry and f['mime'] != MIME_HTML, 'entry.mime', 'file %d' % i)
        chk(f['size'] > FILE_MAX, 'file.size', 'file %d' % i)
        s = 0
        for j, (_, ln) in enumerate(f['chunks']):
            chk(not 1 <= ln <= CHUNK_MAX, 'chunk.len', 'file %d chunk %d' % (i, j))
            s += ln
        chk(s != f['size'], 'file.sum', 'file %d' % i)
        total += f['size']
    chk(total > SITE_MAX, 'site.size', str(total))


class Provider:
    """Mock eth_getCode provider; counts distinct fetches."""

    def __init__(self, codes):
        self.codes = {bytes(k): bytes(v) for k, v in (codes or {}).items()}
        self.calls = []

    def get(self, addr):
        if addr not in self.calls:
            self.calls.append(addr)
        return self.codes.get(addr, b'')


def check_chunk(addr, ln, code, factory, disabled, stage_prefix='fetch'):
    """B09 + P06 for one chunk. Precedence: missing, prefix, length, origin."""
    st = 'fetch' if stage_prefix == 'fetch' else 'manifest'
    if len(code) == 0 and stage_prefix + '.missing' not in disabled:
        raise Reject(stage_prefix + '.missing', st, addr.hex())
    if code[:1] != b'\x00' and stage_prefix + '.prefix' not in disabled:
        raise Reject(stage_prefix + '.prefix', st, addr.hex())
    if len(code) != ln + 1 and stage_prefix + '.length' not in disabled:
        raise Reject(stage_prefix + '.length', st, addr.hex())
    data = code[1:]
    if chunk_address(data, factory) != addr and stage_prefix + '.origin' not in disabled:
        raise Reject(stage_prefix + '.origin', st, addr.hex())
    return data


def validate_manifest(mbytes, provider=None, factory=FACTORY, disabled=frozenset(),
                      fetch=True):
    """Viewer-side validation (P05/B09). Returns a result dict."""
    prov = provider if isinstance(provider, Provider) else Provider(provider)
    try:
        if len(mbytes) > MANIFEST_MAX and 'decode.oversize' not in disabled:
            raise Reject('decode.oversize', 'decode', str(len(mbytes)))
        try:
            m = rlp_strict.decode(mbytes, frozenset(disabled))
        except RlpError as e:
            raise Reject(e.rule, 'decode', e.detail)
        version, entry, files = _structure(m, disabled)
        _semantic(version, entry, files, disabled)
        if not fetch:
            return {'result': 'accept', 'stagesRun': 'pre-fetch', 'fetchCalls': len(prov.calls)}
        out = []
        for i, f in enumerate(files):          # file-major order
            data = b''
            for addr, ln in f['chunks']:
                data += check_chunk(addr, ln, prov.get(addr), factory, disabled)
            if keccak256_cached(data) != f['hash'] and 'file.hash' not in disabled:
                raise Reject('file.hash', 'hash', 'file %d' % i)
            out.append({'path': f['path'].decode('ascii', 'replace'), 'size': f['size'],
                        'keccak256': '0x' + keccak256_cached(data).hex()})
        return {'result': 'accept', 'entryIndex': entry, 'files': out,
                'fetchCalls': len(prov.calls)}
    except Reject as r:
        return {'result': 'reject', 'rule': r.rule, 'stage': r.stage, 'detail': r.detail,
                'fetchCalls': len(prov.calls)}


def validate_version(record, provider=None, factory=FACTORY, disabled=frozenset()):
    """Manifest retrieval stage: record = {manifestHash, manifestLen, chunks:[(addr,len)]}."""
    prov = provider if isinstance(provider, Provider) else Provider(provider)
    try:
        def chk(cond, rule, detail=''):
            if cond and rule not in disabled:
                raise Reject(rule, 'manifest', detail)
        n = record['manifestLen']
        chk(not 1 <= n <= MANIFEST_MAX, 'manifest.length', 'manifestLen %d' % n)
        lens = [ln for _, ln in record['chunks']]
        canon = [len(p) for p in split(b'\x00' * n)]
        chk(lens != canon, 'manifest.split', str(lens))
        data = b''
        for addr, ln in record['chunks']:
            data += check_chunk(addr, ln, prov.get(addr), factory, disabled, 'mfetch')
        chk(keccak256(data) != record['manifestHash'], 'manifest.hash')
    except Reject as r:
        return {'result': 'reject', 'rule': r.rule, 'stage': r.stage, 'detail': r.detail,
                'fetchCalls': len(prov.calls)}
    res = validate_manifest(data, prov, factory, disabled)
    return res
