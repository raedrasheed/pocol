"""Strict RLP encoder/decoder for M1 specification tooling.

Decoding raises RlpError(rule) with a rule ID from M1-SPEC-0.2 section 3.
A byte string decodes to `bytes`; a list decodes to `list`.
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
    if isinstance(x, (bytes, bytearray)):
        x = bytes(x)
        if len(x) == 1 and x[0] < 0x80:
            return x
        return _len_prefix(len(x), 0x80, 0xb7) + x
    if isinstance(x, list):
        body = b''.join(encode(i) for i in x)
        return _len_prefix(len(body), 0xc0, 0xf7) + body
    raise TypeError(type(x))


def uint(n):
    """Minimal big-endian integer; zero is the empty string (P02)."""
    if n < 0:
        raise ValueError(n)
    return n.to_bytes((n.bit_length() + 7) // 8, 'big')


def _item(b, i, allow=frozenset()):
    if i >= len(b):
        raise RlpError('decode.truncated', 'offset %d' % i)
    p = b[i]
    if p < 0x80:
        return b[i:i + 1], i + 1
    if p <= 0xbf:
        if p <= 0xb7:
            n, st = p - 0x80, i + 1
        else:
            ll = p - 0xb7
            lb = b[i + 1:i + 1 + ll]
            if len(lb) != ll:
                raise RlpError('decode.truncated', 'length of length')
            if lb[0] == 0 and 'decode.noncanonical' not in allow:
                raise RlpError('decode.noncanonical', 'leading zero in length')
            n, st = int.from_bytes(lb, 'big'), i + 1 + ll
            if n < 56 and 'decode.noncanonical' not in allow:
                raise RlpError('decode.noncanonical', 'long form for short string')
        if st + n > len(b):
            raise RlpError('decode.truncated', 'string body')
        s = bytes(b[st:st + n])
        if n == 1 and s[0] < 0x80 and 'decode.noncanonical' not in allow:
            raise RlpError('decode.noncanonical', 'single byte wrapped')
        return s, st + n
    if p <= 0xf7:
        n, st = p - 0xc0, i + 1
    else:
        ll = p - 0xf7
        lb = b[i + 1:i + 1 + ll]
        if len(lb) != ll:
            raise RlpError('decode.truncated', 'length of length')
        if lb[0] == 0 and 'decode.noncanonical' not in allow:
            raise RlpError('decode.noncanonical', 'leading zero in length')
        n, st = int.from_bytes(lb, 'big'), i + 1 + ll
        if n < 56 and 'decode.noncanonical' not in allow:
            raise RlpError('decode.noncanonical', 'long form for short list')
    end = st + n
    if end > len(b):
        raise RlpError('decode.truncated', 'list body')
    out, j = [], st
    while j < end:
        x, j = _item(b, j, allow)
        if j > end:
            raise RlpError('decode.truncated', 'item crosses list end')
        out.append(x)
    return out, end


def decode(b, allow=frozenset()):
    """`allow` disables named rules; used only by the mutation harness."""
    x, j = _item(bytes(b), 0, allow)
    if j != len(b) and 'decode.trailing' not in allow:
        raise RlpError('decode.trailing', '%d trailing bytes' % (len(b) - j))
    return x
