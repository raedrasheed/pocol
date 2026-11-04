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

walk(json.loads((ROOT / 'prototypes/lp4-execution/tests/data/wire-envelopes.json').read_text()))
for kind in range(8):
    for data in (b'', b'\x80', b'\xc0', b'\xc1\x80'):
        seed(bytes([kind]) + data)
print(f'{len(list(out.iterdir()))} corpus entries in {out}')
