"""Initialize a disposable corpus from the repository's public test vectors."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)

def seed(data):
    (out / hashlib.sha256(data).hexdigest()).write_bytes(data)

def walk(value):
    if isinstance(value, dict):
        raw = value.get('raw')
        if isinstance(raw, str):
            try: data = bytes.fromhex(raw.removeprefix('0x'))
            except ValueError: pass
            else:
                if len(data) <= 16383: seed(b'\x05' + data)
        for child in value.values(): walk(child)
    elif isinstance(value, list):
        for child in value: walk(child)

def rlp(data, base=0x80):
    """An RLP byte string (base 0x80), or a list (base 0xc0) of already encoded items."""
    if base == 0x80 and len(data) == 1 and data[0] < 0x80: return data
    if len(data) < 56: return bytes([base + len(data)]) + data
    size = len(data).to_bytes((len(data).bit_length() + 7) // 8, 'big')
    return bytes([base + 55 + len(size)]) + size + data

def message(*items):
    """A P2P frame for selector 0: RLP([type, ...fields]) of byte-string items (integers minimal)."""
    return b'\x00' + rlp(b''.join(rlp(x) for x in items), 0xc0)

def uint(n): return n.to_bytes((n.bit_length() + 7) // 8, 'big')

walk(json.loads((ROOT / 'prototypes/lp4-execution/tests/data/wire-envelopes.json').read_text()))
for kind in range(8):
    for data in (b'', b'\x80', b'\xc0', b'\xc1\x80'):
        seed(bytes([kind]) + data)
# Peer discovery: the 11-item Hello (listenPort, nonce), GetAddr, and Addr with 0, 1 and 64 records of
# 18 bytes (IPv4 mapped into IPv6, and IPv6, each with a big-endian port).
seed(message(b'', uint(777900), bytes([1]) * 32, bytes([2]) * 32, uint(1), bytes([3]) * 32, b'fuzz', uint(2),
    bytes([4]) * 32, uint(30300), bytes(range(1, 9))))
seed(message(uint(8)))
records = [bytes(10) + b'\xff\xff' + bytes([81, 2, 69, i]) + (30300).to_bytes(2, 'big') if i % 2 == 0
    else bytes.fromhex('26064700') + bytes(11) + bytes([i]) + (1024 + i).to_bytes(2, 'big') for i in range(64)]
for n in (0, 1, 64):
    seed(message(uint(9), b''.join(records[:n])))
print(f'{len(list(out.iterdir()))} corpus entries in {out}')
