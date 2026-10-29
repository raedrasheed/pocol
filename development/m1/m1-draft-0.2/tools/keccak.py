"""Pure-Python Keccak-256 (Ethereum variant, padding 0x01).

Specification tooling only. Independent of pycryptodome; NOT one of the
three M0 hash libraries (K1-K3), which remain outstanding.
"""

_RC = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
]
_ROT = [0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14]
_M = (1 << 64) - 1
# pi: B[y + 5*((2x+3y)%5)] = rot(A[x+5y])
_PI = [0] * 25
for _x in range(5):
    for _y in range(5):
        _PI[_x + 5 * _y] = _y + 5 * ((2 * _x + 3 * _y) % 5)


def _f(A):
    M = _M
    for rc in _RC:
        C0 = A[0] ^ A[5] ^ A[10] ^ A[15] ^ A[20]
        C1 = A[1] ^ A[6] ^ A[11] ^ A[16] ^ A[21]
        C2 = A[2] ^ A[7] ^ A[12] ^ A[17] ^ A[22]
        C3 = A[3] ^ A[8] ^ A[13] ^ A[18] ^ A[23]
        C4 = A[4] ^ A[9] ^ A[14] ^ A[19] ^ A[24]
        D = (C4 ^ (((C1 << 1) | (C1 >> 63)) & M),
             C0 ^ (((C2 << 1) | (C2 >> 63)) & M),
             C1 ^ (((C3 << 1) | (C3 >> 63)) & M),
             C2 ^ (((C4 << 1) | (C4 >> 63)) & M),
             C3 ^ (((C0 << 1) | (C0 >> 63)) & M))
        B = [0] * 25
        for i in range(25):
            v = A[i] ^ D[i % 5]
            r = _ROT[i]
            B[_PI[i]] = (((v << r) | (v >> (64 - r))) & M) if r else v
        for y in range(0, 25, 5):
            b0, b1, b2, b3, b4 = B[y], B[y + 1], B[y + 2], B[y + 3], B[y + 4]
            A[y] = b0 ^ (~b1 & b2)
            A[y + 1] = b1 ^ (~b2 & b3)
            A[y + 2] = b2 ^ (~b3 & b4)
            A[y + 3] = b3 ^ (~b4 & b0)
            A[y + 4] = b4 ^ (~b0 & b1)
        A[0] ^= rc
    return A


def keccak256(data: bytes) -> bytes:
    rate = 136
    buf = bytearray(data)
    buf.append(0x01)
    buf.extend(b'\x00' * ((-len(buf)) % rate))
    buf[-1] |= 0x80
    A = [0] * 25
    fb = int.from_bytes
    for off in range(0, len(buf), rate):
        for j in range(17):
            A[j] ^= fb(buf[off + 8 * j: off + 8 * j + 8], 'little')
        _f(A)
    return b''.join(A[j].to_bytes(8, 'little') for j in range(4))


_cache = {}


def keccak256_cached(data: bytes) -> bytes:
    """Memoised for large repeated fixture chunks."""
    if len(data) < 4096:
        return keccak256(data)
    h = _cache.get(data)
    if h is None:
        h = _cache[data] = keccak256(data)
    return h


def selftest():
    assert keccak256(b'').hex() == 'c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470'
    assert keccak256(b'abc').hex() == '4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45'
    # EIP-1014 example 0
    a = keccak256(b'\xff' + bytes(20) + bytes(32) + keccak256(b'\x00'))[12:]
    assert a.hex() == '4d1a2e2bb4f88f0250f26ffff098b0b30b26bf38'
    # multi-block boundary lengths
    for n in (135, 136, 137, 272):
        keccak256(b'\x61' * n)
    return True


if __name__ == '__main__':
    import time
    selftest()
    t = time.time(); keccak256(b'\xaa' * 1048576); print('1 MiB in %.1fs' % (time.time() - t))
