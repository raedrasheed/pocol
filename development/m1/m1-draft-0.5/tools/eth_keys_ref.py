"""Test keys and txA (R5-08, annex row B8). Specification tooling only; no network.

Pure-Python secp256k1 (affine arithmetic), BIP-39 seed, BIP-32 derivation,
RFC 6979 deterministic ECDSA with low-s normalization, and EIP-1559 (type 2)
transaction encoding. Used only to produce the literal txA fixture and test
addresses. It is cross-checked in a different language by tools/txa_check.cjs
(Node: OpenSSL secp256k1 public keys, @noble/hashes keccak, BigInt verify).

TK-1 (PROPOSAL, P): the baseline names "fixture keys 1-10" (FD:L2467) and funds
keys 7-10 on anvil-br (FD:L2384) but defines no derivation. Proposed: fixture
key i is anvil's default development account i-1, i.e. BIP-32 path
m/44'/60'/0'/0/(i-1) of the BIP-39 mnemonic
'[LOCAL TEST SEED NOT PUBLISHED]' with an empty
passphrase. anvil funds these accounts by default.
"""

import os
import hashlib
import hmac

from keccak import keccak256
import rlp_strict as R

P = 2 ** 256 - 2 ** 32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)
MNEMONIC = os.environ.get('POCOL_TEST_MNEMONIC')
HARD = 2 ** 31


def _add(p, q):
    if p is None:
        return q
    if q is None:
        return p
    if p[0] == q[0]:
        if (p[1] + q[1]) % P == 0:
            return None
        lam = 3 * p[0] * p[0] * pow(2 * p[1], -1, P) % P
    else:
        lam = (q[1] - p[1]) * pow(q[0] - p[0], -1, P) % P
    x = (lam * lam - p[0] - q[0]) % P
    return x, (lam * (p[0] - x) - p[1]) % P


def mul(k, pt=G):
    acc = None
    while k:
        if k & 1:
            acc = _add(acc, pt)
        pt = _add(pt, pt)
        k >>= 1
    return acc


def address(d):
    x, y = mul(d)
    return '0x' + keccak256(x.to_bytes(32, 'big') + y.to_bytes(32, 'big'))[12:].hex()


def _compressed(pt):
    return bytes([2 + (pt[1] & 1)]) + pt[0].to_bytes(32, 'big')


def bip39_seed(mnemonic=MNEMONIC, passphrase=''):
    if not mnemonic:
        raise ValueError('POCOL_TEST_MNEMONIC is required locally; no key material is published')
    return hashlib.pbkdf2_hmac('sha512', mnemonic.encode(), ('mnemonic' + passphrase).encode(), 2048)


def bip32_path(seed, path):
    i = hmac.new(b'Bitcoin seed', seed, hashlib.sha512).digest()
    k, c = int.from_bytes(i[:32], 'big'), i[32:]
    for idx in path:
        data = (b'\x00' + k.to_bytes(32, 'big') if idx >= HARD else _compressed(mul(k))) + idx.to_bytes(4, 'big')
        i = hmac.new(c, data, hashlib.sha512).digest()
        il = int.from_bytes(i[:32], 'big')
        if il >= N:
            raise ValueError('invalid child (il >= n)')
        k, c = (il + k) % N, i[32:]
        if k == 0:
            raise ValueError('invalid child (zero key)')
    return k


def fixture_key(i, seed=None):
    """TK-1: fixture key i (1..10) = anvil default account i-1."""
    seed = seed or bip39_seed()
    return bip32_path(seed, [44 + HARD, 60 + HARD, 0 + HARD, 0, i - 1])


def rfc6979_k(d, h):
    x = d.to_bytes(32, 'big')
    h1 = (int.from_bytes(h, 'big') % N).to_bytes(32, 'big')
    v, k = b'\x01' * 32, b'\x00' * 32
    k = hmac.new(k, v + b'\x00' + x + h1, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    k = hmac.new(k, v + b'\x01' + x + h1, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    while True:
        v = hmac.new(k, v, hashlib.sha256).digest()
        cand = int.from_bytes(v, 'big')
        if 1 <= cand < N:
            return cand
        k = hmac.new(k, v + b'\x00', hashlib.sha256).digest()
        v = hmac.new(k, v, hashlib.sha256).digest()


def sign(d, h):
    """Returns (r, s, yParity) with s <= n/2 (Ethereum low-s)."""
    k = rfc6979_k(d, h)
    rp = mul(k)
    r = rp[0] % N
    z = int.from_bytes(h, 'big')
    s = pow(k, -1, N) * (z + r * d) % N
    rec = rp[1] & 1
    if s > N // 2:
        s, rec = N - s, rec ^ 1
    return r, s, rec


def recover(h, r, s, rec):
    y2 = (pow(r, 3, P) + 7) % P
    y = pow(y2, (P + 1) // 4, P)
    if y & 1 != rec:
        y = P - y
    z = int.from_bytes(h, 'big')
    rinv = pow(r, -1, N)
    q = _add(mul(s * rinv % N, (r, y)), mul((-z * rinv) % N))
    return '0x' + keccak256(q[0].to_bytes(32, 'big') + q[1].to_bytes(32, 'big'))[12:].hex()


def type2_tx(d, chain_id, nonce, tip, max_fee, gas, to_hex, value, data=b''):
    fields = [R.uint(chain_id), R.uint(nonce), R.uint(tip), R.uint(max_fee), R.uint(gas),
              bytes.fromhex(to_hex[2:]), R.uint(value), data, []]
    sighash = keccak256(b'\x02' + R.encode(fields))
    r, s, rec = sign(d, sighash)
    raw = b'\x02' + R.encode(fields + [R.uint(rec), R.uint(r), R.uint(s)])
    return {'signingHash': '0x' + sighash.hex(), 'r': hex(r), 's': hex(s), 'yParity': rec,
            'raw': '0x' + raw.hex(), 'txHash': '0x' + keccak256(raw).hex()}
