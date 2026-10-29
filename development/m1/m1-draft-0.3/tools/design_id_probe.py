"""C08 probe: test candidate derivations of the approved design ID. Read-only.

Usage: <python> m1-draft-0.3/tools/design_id_probe.py
Writes m1-draft-0.3/results/design-id-probe.json. Never modifies reference/.

The design ID appears inside FINAL_DESIGN.md itself (line 3), so it cannot be
the plain SHA-256 of that file's bytes (that would be a SHA-256 fixed point).
A mismatch between the two is therefore expected and is not evidence of
corruption. This script only tests whether some simple, stated preimage
reproduces the ID. A negative result leaves the derivation open; it does not
show that the ID is wrong.
"""

import hashlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
REF = ROOT / 'reference'
sys.path.append(str(ROOT / 'm1-draft-0.2' / 'tools'))
from keccak import keccak256          # noqa: E402

DESIGN_ID = 'e8a19ecb306c7b955cddc75b050fdda95f9829c0e2b5cc06563cb27dba851aa0'
RECORDED_RAW_SHA256 = 'd54b5aee37a052a053198c43a0278ae8561331f58e3f1148faa6ed4a194a480e'


def variants(raw):
    """Yield (name, bytes) candidate preimages derived from FINAL_DESIGN.md."""
    yield 'raw', raw
    text = raw.decode('utf-8')
    lf = text.replace('\r\n', '\n')
    yield 'lf', lf.encode()
    yield 'crlf', lf.replace('\n', '\r\n').encode()
    yield 'lf-no-bom', lf.lstrip('﻿').encode()
    lines = lf.split('\n')
    if len(lines) > 2 and DESIGN_ID in lines[2]:
        yield 'lf-without-line3', '\n'.join(lines[:2] + lines[3:]).encode()
        yield 'lf-without-lines2-4', '\n'.join(lines[:1] + lines[4:]).encode()
        for ph in ('', '0' * 64, 'TBD'):
            yield 'lf-id-replaced-by-%r' % ph, lf.replace(DESIGN_ID, ph).encode()
    yield 'lf-strip-trailing-newlines', lf.rstrip('\n').encode()


def main():
    raw = (REF / 'FINAL_DESIGN.md').read_bytes()
    out = {'designId': DESIGN_ID, 'recordedRawSha256': RECORDED_RAW_SHA256,
           'rawSha256Now': hashlib.sha256(raw).hexdigest(), 'candidates': [], 'matches': []}
    out['rawUnchangedSinceRecorded'] = out['rawSha256Now'] == RECORDED_RAW_SHA256
    cands = list(variants(raw))
    for p in sorted(REF.iterdir()):
        if p.is_file() and p.name != 'FINAL_DESIGN.md':
            cands.append(('file:' + p.name, p.read_bytes()))
    sections = ['scope.md', 'consensus.md', 'execution.md', 'storage.md', 'network.md', 'browser.md',
                'threat.md', 'incentives.md', 'economics.md', 'governance.md', 'validation.md',
                'implementation.md']
    if all((REF / s).exists() for s in sections):
        cands.append(('concat:sections(listed order)', b''.join((REF / s).read_bytes() for s in sections)))
    for name, data in cands:
        row = {'candidate': name, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
               'keccak256': keccak256(data).hex(),
               'sha3_256': hashlib.sha3_256(data).hexdigest() if hasattr(hashlib, 'sha3_256') else None}
        out['candidates'].append(row)
        for alg in ('sha256', 'keccak256', 'sha3_256'):
            if row[alg] == DESIGN_ID:
                out['matches'].append({'candidate': name, 'algorithm': alg})
    out['conclusion'] = ('derivation reproduced by: %s' % out['matches']) if out['matches'] else \
        'no tested candidate reproduces the design ID; derivation remains open (owner question Q-C08)'
    res = HERE.parent / 'results'
    res.mkdir(exist_ok=True)
    (res / 'design-id-probe.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
    print(out['conclusion'])
    return 0


if __name__ == '__main__':
    sys.exit(main())
