"""Explicit-stack RLP framing parser for M1 draft 0.22 (C30 repair). Python standard library only.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

C30 (coordination/review-001/REVIEW-0.21.md): m1-draft-0.21/tools/netprofile_ref.py parse() recursed once
per nested list. 1500 canonically framed nested lists (4291 bytes, inside the 47104-byte RecvFit bound)
raised an uncaught RecursionError instead of the controlled gsStructure rejection.

make_parse(GsError) returns parse(b) with exactly the 0.21 contract:
  node = ('b', bytes, wrapped) | ('l', [nodes]); framing errors GsError('L0', <same detail strings>);
  a wrapped single byte (81 xx, xx < 0x80) is valid framing with wrapped=True (judged later as gsInt).
Errors are raised at the same position and in the same order as the recursive 0.21 parser:
  header problems of an item, then (for strings) 'truncated string', (for lists) 'truncated list' at the
  list header; a child that ends beyond its parent's end raises 'item crosses list end' when the child is
  complete (the recursive parser checked after the child returned); 'N trailing bytes' after the root.
No depth limit and no input restriction are introduced: memory is linear in the input length.
"""


def make_parse(GsError):
    def header(b, i):
        """Returns (kind, n, st) for the item at i, or raises L0 exactly as 0.21 _parse did."""
        if i >= len(b):
            raise GsError('L0', 'truncated')
        p = b[i]
        if p < 0x80:
            return 'byte', 1, i
        if p <= 0xbf:
            if p <= 0xb7:
                return 'str', p - 0x80, i + 1
            ll = p - 0xb7
            lb = b[i + 1:i + 1 + ll]
            if len(lb) != ll:
                raise GsError('L0', 'truncated length')
            if lb[0] == 0:
                raise GsError('L0', 'leading zero in length')
            n = int.from_bytes(lb, 'big')
            if n < 56:
                raise GsError('L0', 'long form for short string')
            return 'str', n, i + 1 + ll
        if p <= 0xf7:
            return 'list', p - 0xc0, i + 1
        ll = p - 0xf7
        lb = b[i + 1:i + 1 + ll]
        if len(lb) != ll:
            raise GsError('L0', 'truncated length')
        if lb[0] == 0:
            raise GsError('L0', 'leading zero in length')
        n = int.from_bytes(lb, 'big')
        if n < 56:
            raise GsError('L0', 'long form for short list')
        return 'list', n, i + 1 + ll

    def parse(data):
        b = bytes(data)
        i = 0
        stack = []                                    # open lists: [children, end]
        while True:
            kind, n, st = header(b, i)
            if kind == 'byte':
                node, i = ('b', b[i:i + 1], False), i + 1
            elif kind == 'str':
                if st + n > len(b):
                    raise GsError('L0', 'truncated string')
                s = b[st:st + n]
                node, i = ('b', s, n == 1 and s[0] < 0x80), st + n
            else:
                end = st + n
                if end > len(b):
                    raise GsError('L0', 'truncated list')
                if st < end:
                    stack.append([[], end])
                    i = st
                    continue
                node, i = ('l', []), st
            # attach the completed node; close every list that ends here
            while True:
                if not stack:
                    if i != len(b):
                        raise GsError('L0', '%d trailing bytes' % (len(b) - i))
                    return node
                children, end = stack[-1]
                if i > end:
                    raise GsError('L0', 'item crosses list end')
                children.append(node)
                if i < end:
                    break
                stack.pop()
                node = ('l', children)

    parse.c30_iterative = True
    return parse
