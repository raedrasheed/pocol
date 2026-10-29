"""BR16 message generator, Python implementation (R6-06, annex row B11). Specification tooling.

Implements annex/br16-generator.json exactly; tools/br16_gen.cjs implements the same
algorithm in JavaScript (BadSite's language). Equal stream hashes show the two agree.
"""

import hashlib
import json
from pathlib import Path

SPEC = json.loads((Path(__file__).resolve().parent.parent / 'annex' / 'br16-generator.json').read_text(encoding='utf-8'))


def seed(i):
    return hashlib.sha256(b'PoColSeed' + i.to_bytes(4, 'big')).digest()          # FD:L2522


class Prng:
    def __init__(self, s):
        self.s, self.k, self.buf = s, 0, b''

    def u32(self):
        if not self.buf:
            self.buf = hashlib.sha256(self.s + self.k.to_bytes(8, 'big')).digest()
            self.k += 1
        v, self.buf = int.from_bytes(self.buf[:4], 'big'), self.buf[4:]
        return v

    def r(self, n):
        return self.u32() % n


class Pairs(list):
    """Ordered key/value pairs (duplicates allowed)."""


def esc(s):
    out = []
    for ch in s:
        if ch == '"':
            out.append('\\"')
        elif ch == '\\':
            out.append('\\\\')
        elif ord(ch) < 0x20:
            out.append('\\u%04x' % ord(ch))
        else:
            out.append(ch)
    return '"' + ''.join(out) + '"'


def ser(v):
    if v is None:
        return 'null'
    if v is True:
        return 'true'
    if v is False:
        return 'false'
    if isinstance(v, int):
        return str(v)
    if isinstance(v, str):
        return esc(v)
    if isinstance(v, Pairs):
        return '{' + ','.join(esc(k) + ':' + ser(x) for k, x in v) + '}'
    if isinstance(v, dict):
        return '{' + ','.join(esc(k) + ':' + ser(x) for k, x in v.items()) + '}'
    if isinstance(v, list):
        return '[' + ','.join(ser(x) for x in v) + ']'
    raise TypeError(type(v))


def msg(i, kind, method, params):
    return ser(Pairs([('id', i), ('kind', kind), ('payload', Pairs([('method', method), ('params', params)]))]))


def gen(p, depth):
    c = p.r(8)
    if c == 0:
        return p.r(1000)
    if c == 1:
        return SPEC['strs'][p.r(len(SPEC['strs']))]
    if c == 2:
        return p.r(2) == 0
    if c == 3:
        return None
    if c == 4:
        return '0x' + ''.join('%08x' % p.u32() for _ in range(5))
    if c == 5:
        return '0x' + ''.join('%08x' % p.u32() for _ in range(8))
    if c == 6:
        if depth >= 3:
            return None
        n = p.r(3)
        return [gen(p, depth + 1) for _ in range(n)]
    if depth >= 3:
        return None
    n = p.r(3)
    out = Pairs()
    for _ in range(n):
        k = SPEC['keys'][p.r(len(SPEC['keys']))]
        out.append((k, gen(p, depth + 1)))
    return out


def mutate(p, name, m):
    if m == 0:
        return ''.join(ch.upper() if 'a' <= ch <= 'z' else ch for ch in name)
    if m == 1:
        return name + ' '
    if m == 2:
        return name + '\u0000'
    if m == 3:
        i = name.find('a')
        return name if i < 0 else name[:i] + 'а' + name[i + 1:]
    if m == 4:
        return name[:-1]
    return SPEC['proto'][p.r(len(SPEC['proto']))]


def template(j, m):
    i = j + 1
    bp = '{"method":"eth_blockNumber","params":[]}'
    if m == 0:
        t = msg(i, 'rpc_read', 'eth_blockNumber', [])
        return t[:len(t) // 2]
    if m == 1:
        return '{"id":%d,"kind":"rpc_read","payload":[%s,{"method":"eth_sendRawTransaction","params":[]}]}' % (i, bp)
    if m == 2:
        return '{"id":%d,"kind":"rpc_read","payload":%s,"x":0}' % (i, bp)
    if m == 3:
        return '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_blockNumber","params":[],"jsonrpc":"2.0"}}' % i
    if m == 4:
        return '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_blockNumber"}}' % i
    if m == 5:
        return '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_blockNumber","method":"eth_sendRawTransaction","params":[]}}' % i
    if m == 6:
        return '{"id":"%d","kind":"rpc_read","payload":%s}' % (i, bp)
    if m == 7:
        return '{"id":-1,"kind":"rpc_read","payload":%s}' % bp
    if m == 8:
        return '{"id":4294967296,"kind":"rpc_read","payload":%s}' % bp
    if m == 9:
        return '{"id":1.5,"kind":"rpc_read","payload":%s}' % bp
    if m == 10:
        return '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_call","params":[[[[[[[]]]]]]]}}' % i
    head = '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_call","params":["' % i
    tail = '"]}}'
    return head + 'a' * (65537 - len(head) - len(tail)) + tail


def generate(seed_index=None, count=None):
    """Yield (j, text, openExternalParam0 or None)."""
    p = Prng(seed(SPEC['seedIndex'] if seed_index is None else seed_index))
    for j in range(SPEC['count'] if count is None else count):
        if j % 50 == 49:
            kl = [1, 255, 256, 257][p.r(4)]
            vl = [0, 1, 61439, 61440, 61441][p.r(5)]
            yield j, msg(j + 1, 'storage_set', 'site_storageSet', ['k' * kl, 'a' * vl]), None
            continue
        if p.r(10) == 0:
            yield j, template(j, p.r(12)), None
            continue
        r5 = p.r(5)
        mid = 0 if r5 == 0 else (4294967295 if r5 == 1 else j + 1)
        kind = SPEC['validKinds'][p.r(4)] if p.r(10) < 8 else SPEC['badKinds'][p.r(6)]
        if kind in SPEC['validKinds'] and p.r(2) == 0:
            lst = SPEC['matrixMethods'][kind]
            method = lst[p.r(len(lst))]
        else:
            method = SPEC['names'][p.r(len(SPEC['names']))]
        if p.r(4) == 0:
            method = mutate(p, method, p.r(6))
        if method in SPEC['samples'] and p.r(2) == 0:
            params = SPEC['samples'][method]
        else:
            n = p.r(4)
            params = [gen(p, 1) for _ in range(n)]
        p0 = params[0] if method == 'site_openExternal' and params and isinstance(params[0], str) else None
        yield j, msg(mid, kind, method, params), p0


def stream_hash(seed_index=None, count=None):
    h = hashlib.sha256()
    n = 0
    for _, text, _ in generate(seed_index, count):
        h.update(text.encode('utf-8'))
        h.update(b'\n')
        n += 1
    return h.hexdigest(), n
