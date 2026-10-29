"""LP3 S1 differential check: Rust `lp1-node genesis-decode` against the accepted M1 reference decoder.
Python standard library only. Run after building, for example:
    python tests/lp3_genesis_diff.py --bin target/debug/lp1-node --m1 ../../development/m1

The oracle is loaded read-only from the accepted M1 tree and checked by SHA-256 first:
  m1-draft-0.21/tools/netprofile_ref.py   decode_genesis / encode_genesis / keccak256 (pure Python)
  m1-draft-0.22/tools/iterative_parse.py  C30 explicit-stack framing, installed as NP.parse as the
                                          0.22 runner does (decode_genesis resolves parse at call time)
  m1-draft-0.2/tools/keccak.py, rlp_strict.py  loaded by netprofile_ref
Corpus: GSV1, the 32 GSV1 negatives, the 13 C30 cases (deep and shallow), seeded structured edits
of the GSV1 item tree, seeded byte mutations, and a size ladder: gsVersion values of 17 bytes to
256 KiB, the largest valid spec (65535 members, MAX_SPEC_BYTES), and framing / structure inputs of
that size (`--max-version` adds a gsVersion item of that size, slow in a debug build). For every
input both sides must agree on the code and the detail string of a rejection, or, for an accepted
input, on genesisHash, chainId, member count and exact re-encoding. Every detail is compared as a
string with no exception, gsVersion included (round 1 masked values >= 2^128). Python >= 3.11
refuses str() of integers above 4300 digits by default; the reference reports the integer itself,
so that conversion limit is lifted here. No input exceeds MAX_SPEC_BYTES, so none may be refused.

Prints one JSON summary line (also written to --out if given); exit 0 only if every input agrees.
"""

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import random
import subprocess
import sys
import tempfile
from pathlib import Path

if hasattr(sys, 'set_int_max_str_digits'):
    sys.set_int_max_str_digits(0)

PINNED = {
    'm1-draft-0.21/tools/netprofile_ref.py': 'ca94ef3423222016a63808685e56869e5ed732a3cce732b9c29fc022221690e5',
    'm1-draft-0.22/tools/iterative_parse.py': '742e1936ead8fda8521db7a3ae01baa94e335dd8295ec77a30b09b8ddeb7034a',
    'm1-draft-0.2/tools/keccak.py': '99aa7770d2966ccbc804eb913cd97d54e15bba627c4ee7f71dc3f248a2f91a1d',
    'm1-draft-0.2/tools/rlp_strict.py': 'abd9ba9accacdefff7edc1fc18eb7513402e2ef21528ebac64c6e84cde6a236a',
    'm1-draft-0.21/vectors/v3-gsv1.json': '88097ee293aeab70c2089c2b61da8ce624503a59dfafd34657e9c1a465056f32',
    'm1-draft-0.22/vectors/c30-depth.json': '728d8b4ac88c0212d8b1cb6224c5f834f923619e1c32320dcf1eae120fb72464',
}


def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, str(path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# ------------------------------------------------------------------ literal construction (M1 runner rules)

def leaf(v):
    if isinstance(v, str):
        return bytes.fromhex(v)
    if 'repeat' in v:
        return bytes.fromhex(v['repeat']) * v['count']
    return b''.join(leaf(x) for x in v['concat'])


def list_header(n):
    if n < 56:
        return bytes([0xc0 + n])
    lb = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([0xf7 + len(lb)]) + lb


def serialize(t):
    if isinstance(t, list):
        body = b''.join(serialize(x) for x in t)
        return list_header(len(body)) + body
    return leaf(t)


def at(tree, path):
    node = tree
    for i in path[:-1]:
        node = node[i]
    return node, path[-1]


def apply_edits(tree, edits):
    t = json.loads(json.dumps(tree))
    post = []
    for e in edits:
        if e['op'] == 'delete':
            p, i = at(t, e['path'])
            del p[i]
        elif e['op'] == 'set':
            p, i = at(t, e['path'])
            p[i] = json.loads(json.dumps(e['value']))
        elif e['op'] == 'swap':
            pa, ia = at(t, e['a'])
            pb, ib = at(t, e['b'])
            pa[ia], pb[ib] = pb[ib], pa[ia]
        elif e['op'] == 'copy':
            pf, i_f = at(t, e['from'])
            pt, it = at(t, e['to'])
            pt[it] = json.loads(json.dumps(pf[i_f]))
        else:
            post.append(e)
    b = serialize(t)
    for e in post:
        h = bytes.fromhex(e['hex'])
        b = b + h if e['op'] == 'appendBytes' else h + b[len(h):]
    return b


def wrap(inner, depth):
    heads, n = [], len(inner)
    for _ in range(depth):
        h = list_header(n)
        n += len(h)
        heads.append(h)
    return b''.join(reversed(heads)) + bytes(inner)


def depth_case(gtree, case, depth):
    inner = bytes.fromhex(case['inner'])
    if case['place'] == 'top':
        b = wrap(inner, depth)
    else:
        t = json.loads(json.dumps(gtree))
        nested = wrap(inner, depth).hex()
        if case['place'] == 'gsv1Append':
            t.append(nested)
        else:
            node, i = at(t, case['path'])
            node[i] = nested
        b = apply_edits(t, case.get('edits', []))
    if case.get('dropLast'):
        b = b[:-1]
    if 'suffix' in case:
        b = b + bytes.fromhex(case['suffix'])
    return b


# ------------------------------------------------------------------ seeded corpus

def item_bytes(raw):
    """Canonical RLP string item for raw bytes (hex), regardless of whether it is a valid integer."""
    if len(raw) == 1 and raw[0] < 0x80:
        return raw.hex()
    if len(raw) < 56:
        return (bytes([0x80 + len(raw)]) + raw).hex()
    lb = len(raw).to_bytes((len(raw).bit_length() + 7) // 8, 'big')
    return (bytes([0xb7 + len(lb)]) + lb + raw).hex()


def random_item(rng):
    k = rng.randrange(12)
    if k <= 3:                                                  # integer-like values near width edges
        width = rng.choice([8, 16, 32, 64, 256])
        v = rng.choice([0, 1, 2, (1 << width) - 1, 1 << width, rng.getrandbits(width), 10000, 10001, 256, 257, 9000, 9001])
        raw = v.to_bytes((v.bit_length() + 7) // 8, 'big')
        return item_bytes(raw)
    if k == 4:                                                  # leading zero
        return item_bytes(b'\x00' + bytes(rng.randrange(256) for _ in range(rng.randrange(3))))
    if k == 5:                                                  # wrapped single byte
        return '81' + '%02x' % rng.randrange(0x80)
    if k == 6:                                                  # address- and root-sized strings
        n = rng.choice([0, 1, 19, 20, 21, 31, 32, 33])
        return item_bytes(bytes(rng.randrange(256) for _ in range(n)))
    if k == 7:                                                  # special member ids
        low = rng.choice([0, 0xC0C000, 0xC0C001, 0xC0C080, 0xC0C0FF, 0xC0C100])
        ident = rng.choice([bytes(17) + low.to_bytes(3, 'big'), b'\xff' * 19 + b'\xfe', bytes(20)])
        return item_bytes(ident)
    if k == 8:                                                  # nested list
        return [random_item(rng) for _ in range(rng.randrange(3))]
    if k == 9:
        return wrap(bytes.fromhex(item_bytes(b'\x01')), rng.randrange(1, 40)).hex()
    if k == 10:
        return '80'
    return '%02x' % rng.randrange(0x80)


def random_member(rng, pool):
    return [item_bytes(rng.choice(pool)), item_bytes(bytes(rng.randrange(256) for _ in range(20)))]


def structured(rng, gtree):
    t = json.loads(json.dumps(gtree))
    pool = [bytes([0xa0 + i]) * 20 for i in range(1, 6)] + [bytes(17) + b'\xc0\xc0\x01', b'\xff' * 19 + b'\xfe', bytes(20), bytes(17) + b'\xc0\xc1\x00']
    for _ in range(1 + rng.randrange(3)):
        op = rng.randrange(9)
        cp = t[2] if len(t) > 2 and isinstance(t[2], list) else None
        m0 = t[4] if len(t) > 4 and isinstance(t[4], list) else None
        if op <= 2 and cp:
            cp[rng.randrange(len(cp))] = random_item(rng)
        elif op == 3 and t:
            t[rng.randrange(len(t))] = random_item(rng)
        elif op == 4 and len(t) > 4:
            members = [random_member(rng, pool) for _ in range(rng.randrange(6))]
            if rng.randrange(2):
                members.sort(key=lambda m: m[0])
            t[4] = members
        elif op == 5 and cp:
            del cp[rng.randrange(len(cp))]
        elif op == 6 and cp is not None:
            cp.insert(rng.randrange(len(cp) + 1), random_item(rng))
        elif op == 7:
            if rng.randrange(2) and t:
                del t[rng.randrange(len(t))]
            else:
                t.insert(rng.randrange(len(t) + 1), random_item(rng))
        elif op == 8 and m0 and isinstance(m0[0], list) and m0[0]:
            m0[0][rng.randrange(len(m0[0]))] = random_item(rng)
    return serialize(t)


def byte_mutant(rng, base):
    v = bytearray(base)
    for _ in range(1 + rng.randrange(4)):
        n = len(v)
        op = rng.randrange(6)
        if op == 0 and n:
            v[rng.randrange(n)] ^= 1 << rng.randrange(8)
        elif op == 1 and n:
            v[rng.randrange(n)] = rng.randrange(256)
        elif op == 2:
            del v[rng.randrange(n + 1):]
        elif op == 3:
            v.insert(rng.randrange(n + 1), rng.randrange(256))
        elif op == 4 and n:
            del v[rng.randrange(n)]
        else:
            i = rng.randrange(n + 1)
            v[i:i] = v[i:i + rng.randrange(4)]
    return bytes(v)


# ------------------------------------------------------------------ comparison

def reference(NP, b):
    try:
        spec = NP.decode_genesis(b)
    except NP.GsError as e:
        return {'ok': False, 'code': e.code, 'detail': e.detail}
    return {'ok': True, 'genesisHash': '0x' + NP.keccak256(b).hex(), 'chainId': spec['chainId'], 'm0': len(spec['M_0List']),
            'reencodeEqual': NP.encode_genesis(spec) == b}


def agrees(ref, got):
    if ref['ok'] != got.get('ok'):
        return False
    if ref['ok']:
        return all(ref[k] == got.get(k) for k in ('genesisHash', 'chainId', 'm0', 'reencodeEqual')) and got.get('bootable') is False
    return 'refused' not in got and ref['code'] == got.get('code') and str(ref['detail']) == got.get('detail')


MAX_SPEC_BYTES = 2818474          # genesis_spec::MAX_SPEC_BYTES; the Rust summary row must agree


def rlp_item(raw):
    if len(raw) == 1 and raw[0] < 0x80:
        return raw
    if len(raw) < 56:
        return bytes([0x80 + len(raw)]) + raw
    lb = len(raw).to_bytes((len(raw).bit_length() + 7) // 8, 'big')
    return bytes([0xb7 + len(lb)]) + lb + raw


def ladder(NP, gtree, rng, max_version):
    out = []
    sizes = [17, 32, 33, 1024, 65536, 262144] + ([MAX_SPEC_BYTES - 8] if max_version else [])
    for n in sizes:
        for fill in ('ff', 'rand'):
            raw = b'\xff' * n if fill == 'ff' else bytes([rng.randrange(1, 256)]) + bytes(rng.randrange(256) for _ in range(n - 1))
            t = json.loads(json.dumps(gtree))
            t[0] = rlp_item(raw).hex()
            b = serialize(t) if n < MAX_SPEC_BYTES - 8 else list_header(len(rlp_item(raw))) + rlp_item(raw)
            out.append(('ladder.version%d.%s' % (n, fill), b))
    spec = NP.decode_genesis(serialize(gtree))
    cp = {}
    for name, width, lo, hi in NP.CP_FIELDS:
        cp[name] = (1 << width) - 1 if hi is None else (5000 if hi == 'gamma' else hi)
    cp['alpha_bp'] = 5000
    spec.update({'chainId': 2 ** 64 - 1, 'CP': cp, 'M_0List': [[b'\x10' * 16 + i.to_bytes(4, 'big'), b'\xb1' * 20] for i in range(1, 65536)]})
    big = NP.encode_genesis(spec)
    assert len(big) == MAX_SPEC_BYTES, len(big)
    out.append(('ladder.maxValid', big))
    out.append(('ladder.maxValidUnordered', big[:-33 - 86] + big[-33 - 43:-33] + big[-33 - 86:-33 - 43] + big[-33:]))
    depth, n = 0, 1
    while True:
        h = len(list_header(n))
        if n + h > MAX_SPEC_BYTES:
            break
        n, depth = n + h, depth + 1
    out.append(('ladder.maxDeep', wrap(b'\x80', depth)))
    out.append(('ladder.maxFlat', list_header(MAX_SPEC_BYTES - 4) + b'\x01' * (MAX_SPEC_BYTES - 4)))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', required=True)
    ap.add_argument('--m1', required=True)
    ap.add_argument('--seed', type=int, default=0x4c503301)
    ap.add_argument('--structured', type=int, default=20000)
    ap.add_argument('--bytes', type=int, default=10000)
    ap.add_argument('--out')
    ap.add_argument('--max-version', action='store_true')
    a = ap.parse_args()
    m1 = Path(a.m1).resolve()
    tool_sha = {rel: sha(m1 / rel) for rel in PINNED}
    bad = sorted(rel for rel, h in tool_sha.items() if h != PINNED[rel])
    if bad:
        print(json.dumps({'check': 'lp3-genesis-diff', 'ok': False, 'error': 'pinned M1 input changed', 'files': bad}))
        return 2
    NP = load(m1 / 'm1-draft-0.21/tools/netprofile_ref.py', 'netprofile_ref_lp3')
    IP = load(m1 / 'm1-draft-0.22/tools/iterative_parse.py', 'iterative_parse_lp3')
    NP.parse = IP.make_parse(NP.GsError)
    assert getattr(NP.parse, 'c30_iterative', False)

    # The M1 vector files are UTF-8 (they quote Arabic source literals). Every text read and write here
    # names its encoding: without one, Python on Windows uses the ANSI code page (cp1252 on the
    # windows-2022 runner) and fails on these files (Windows run 37972422364).
    gdoc = json.loads((m1 / 'm1-draft-0.21/vectors/v3-gsv1.json').read_text(encoding='utf-8'))
    cdoc = json.loads((m1 / 'm1-draft-0.22/vectors/c30-depth.json').read_text(encoding='utf-8'))
    gtree = gdoc['tree']
    corpus = [('gsv1', serialize(gtree))]
    corpus += [('neg.' + n['id'], apply_edits(gtree, n['edits'])) for n in gdoc['negatives']]
    for c in cdoc['cases']:
        for twin in ('deep', 'shallow'):
            corpus.append(('c30.%s.%s' % (c['id'], twin), depth_case(gtree, c, c[twin]['depth'])))
    rng = random.Random(a.seed)
    corpus += [('structured.%d' % i, structured(rng, gtree)) for i in range(a.structured)]
    base = corpus[0][1]
    corpus += [('bytes.%d' % i, byte_mutant(rng, base)) for i in range(a.bytes)]
    corpus += ladder(NP, gtree, rng, a.max_version)
    assert max(len(b) for _, b in corpus) <= MAX_SPEC_BYTES

    with tempfile.TemporaryDirectory(prefix='lp3-genesis-diff-') as tmp:
        inp, outp = os.path.join(tmp, 'in.txt'), os.path.join(tmp, 'out.jsonl')
        text = ''.join(b.hex() + '\n' for _, b in corpus)
        Path(inp).write_bytes(text.encode('ascii'))          # exact bytes; text mode would add \r on Windows
        proc = subprocess.run([a.bin, 'genesis-decode', '--in', inp, '--out', outp], capture_output=True, encoding='utf-8')
        rows = [json.loads(x) for x in Path(outp).read_text(encoding='utf-8').splitlines()] if os.path.exists(outp) else []
    summary_row = rows[-1] if rows else {}
    rows = rows[:-1]
    mismatches, ref_codes, label_codes = [], {}, {}
    for idx, (label, b) in enumerate(corpus):
        ref = reference(NP, b)
        got = rows[idx] if idx < len(rows) else {}
        code = 'ok' if ref['ok'] else ref['code']
        ref_codes[code] = ref_codes.get(code, 0) + 1
        group = label.split('.')[0]
        label_codes.setdefault(group, {})
        label_codes[group][code] = label_codes[group].get(code, 0) + 1
        if got.get('line') != idx + 1 or not agrees(ref, got):
            mismatches.append({'label': label, 'input': b.hex()[:200], 'reference': {k: str(v)[:200] for k, v in ref.items()},
                               'rust': {k: v[:200] if isinstance(v, str) else v for k, v in got.items()}})
    ok = (proc.returncode == 0 and len(rows) == len(corpus) and not mismatches and summary_row.get('inputs') == len(corpus)
          and summary_row.get('refused') == 0 and summary_row.get('maxSpecBytes') == MAX_SPEC_BYTES)
    summary = {
        'check': 'lp3-genesis-diff',
        'ok': ok,
        'inputs': len(corpus),
        'agree': len(corpus) - len(mismatches),
        'mismatches': len(mismatches),
        'firstMismatches': mismatches[:10],
        'rustExit': proc.returncode,
        'rustSummary': summary_row,
        'referenceCodes': dict(sorted(ref_codes.items())),
        'byGroup': {k: dict(sorted(v.items())) for k, v in sorted(label_codes.items())},
        'seed': a.seed,
        'maxVersion': a.max_version,
        'largestInput': max(len(b) for _, b in corpus),
        'corpusSha256': hashlib.sha256(text.encode()).hexdigest(),
        'pinnedInputs': tool_sha,
        'python': platform.python_version(),
    }
    line = json.dumps(summary, sort_keys=False)
    print(line)
    if a.out:
        Path(a.out).write_text(json.dumps(summary, indent=1) + '\n', encoding='utf-8')
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main())
