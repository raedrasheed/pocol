"""Strict RLP encoder/decoder for M1 specification tooling (draft 0.3).

Decoding raises RlpError(rule) with a rule ID from M1-SPEC-0.2 section 3.2,
as amended by M1-SPEC-0.3-AMENDMENTS.md R3-01.
A byte string decodes to `bytes`; a list decodes to `list`.

Changes from draft 0.2 (C03):
- the decoder is iterative (explicit stack), so canonical input of any depth
  that fits in the byte limit decodes without native recursion;
- every item header and body is bounded by the extent of its innermost
  enclosing item, not by the end of the whole input, so a child that crosses
  its parent's end is rejected at the child's header before its body is read.
No depth limit is imposed: valid RLP is never rejected for depth alone.
"""


class RlpError(Exception):
    def __init__(self, rule, detail=''):
        super().__init__(rule, detail)
        self.rule = rule
        self.detail = detail


def _len_prefix(n, short, long_):
    if n < 56:
        return bytes([short + n])
    lb = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([long_ + len(lb)]) + lb


def encode(x):
    """Recursive encoder; used only for shallow fixture structures."""
    if isinstance(x, (bytes, bytearray)):
        x = bytes(x)
        if len(x) == 1 and x[0] < 0x80:
            return x
        return _len_prefix(len(x), 0x80, 0xb7) + x
    if isinstance(x, list):
        body = b''.join(encode(i) for i in x)
        return _len_prefix(len(body), 0xc0, 0xf7) + body
    raise TypeError(type(x))


def nest(inner, depth):
    """Wrap pre-encoded `inner` in `depth` canonical list headers, without recursion.

    nest(b'\\xc0', d - 1) is d nested empty lists.
    """
    headers, size = [], len(inner)
    for _ in range(depth):
        h = _len_prefix(size, 0xc0, 0xf7)
        headers.append(h)
        size += len(h)
    headers.reverse()
    return b''.join(headers) + bytes(inner)


def uint(n):
    """Minimal big-endian integer; zero is the empty string (P02)."""
    if n < 0:
        raise ValueError(n)
    return n.to_bytes((n.bit_length() + 7) // 8, 'big')


def _header(b, i, limit, allow):
    """Parse the header of the item at offset i, which must lie within [i, limit).

    Returns (kind, start, length) with kind in {'byte', 'str', 'list'}.
    Check order (R3-01): header present; length-of-length present; leading
    zero in length; long form below 56; body inside the enclosing extent.
    """
    where = 'enclosing item' if limit < len(b) else 'input'
    if i >= limit:
        raise RlpError('decode.truncated', 'offset %d: no item before end of %s' % (i, where))
    p = b[i]
    if p < 0x80:
        return 'byte', i, 1
    if p <= 0xb7:
        kind, n, st = 'str', p - 0x80, i + 1
    elif p <= 0xbf or p > 0xf7:
        kind = 'str' if p <= 0xbf else 'list'
        ll = p - (0xb7 if p <= 0xbf else 0xf7)
        if i + 1 + ll > limit:
            raise RlpError('decode.truncated', 'offset %d: length of length crosses %s' % (i, where))
        lb = b[i + 1:i + 1 + ll]
        if lb[0] == 0 and 'decode.noncanonical' not in allow:
            raise RlpError('decode.noncanonical', 'offset %d: leading zero in length' % i)
        n, st = int.from_bytes(lb, 'big'), i + 1 + ll
        if n < 56 and 'decode.noncanonical' not in allow:
            raise RlpError('decode.noncanonical', 'offset %d: long form for length %d' % (i, n))
    else:
        kind, n, st = 'list', p - 0xc0, i + 1
    if st + n > limit:
        raise RlpError('decode.truncated', 'offset %d: body of %d bytes crosses %s' % (i, n, where))
    return kind, st, n


def decode(b, allow=frozenset()):
    """Decode exactly one item; `allow` disables named rules (mutation harness only).

    Iterative: memory is O(depth) list frames, time O(len(b)).
    """
    b = bytes(b)
    root = []
    stack = [(root, len(b))]          # (container, end offset of its extent)
    i = 0
    while True:
        cont, end = stack[-1]
        if len(stack) == 1:
            if root:
                break
        elif i == end:
            stack.pop()
            continue
        kind, st, n = _header(b, i, end, allow)
        if kind == 'list':
            child = []
            cont.append(child)
            stack.append((child, st + n))
            i = st
            continue
        s = b[st:st + n]
        if kind == 'str' and n == 1 and s[0] < 0x80 and 'decode.noncanonical' not in allow:
            raise RlpError('decode.noncanonical', 'offset %d: single byte wrapped' % i)
        cont.append(s)
        i = st + n
    if i != len(b) and 'decode.trailing' not in allow:
        raise RlpError('decode.trailing', '%d trailing bytes' % (len(b) - i))
    return root[0]


def max_depth(b):
    """Nesting depth of a decoded-valid input, computed without recursion (lists only)."""
    best, stack, i = 0, [len(b)], 0
    while i < len(b) or len(stack) > 1:
        while len(stack) > 1 and i == stack[-1]:
            stack.pop()
        if i >= len(b):
            break
        kind, st, n = _header(b, i, stack[-1], frozenset())
        if kind == 'list':
            stack.append(st + n)
            best = max(best, len(stack) - 1)
            i = st
        else:
            i = st + n
    return best
