"""LP2 process-level end-to-end check of the `store` CLI. Python standard library only.

Root runs it after building, for example:
    python tests/e2e_store.py --bin target/debug/lp1-node.exe --fixtures fixtures

Every store lives in a fresh temporary directory created here. Corrupted or torn journals are made
by copying bytes into NEW store directories; the stores written by the node are never edited.
Journal layout knowledge (metadata 128 bytes, record = 120 fixed bytes + header, header at
offset 84) comes from the documented experimental format in src/store.rs. Prints one JSON summary
line; exit status 0 only if every check passed. Concurrent-writer and crash checks need the Windows
share-mode handle and are in tests/store_process.rs.
"""

import argparse
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile

CHECKS = []
META = 128
REC_FIXED = 120
HDR_AT = 84
MAX_FILE = 128 + 1024 * 3187


def check(name, cond, detail=''):
    CHECKS.append({'check': name, 'ok': bool(cond), 'detail': '' if cond else str(detail)[:400]})


def run(binary, fixtures, *args):
    p = subprocess.run([binary, 'store'] + list(args) + ['--fixtures', fixtures], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300)
    lines = p.stdout.decode('utf-8').strip().splitlines()
    try:
        obj = json.loads(lines[-1]) if lines else {}
    except ValueError:
        obj = {'unparsed': lines[-1][:200]}
    return p.returncode, obj, lines


def jpath(store):
    return os.path.join(store, 'candidates.lp2j')


def read(store):
    with open(jpath(store), 'rb') as f:
        return f.read()


def raw_store(path, data):
    os.mkdir(path)
    with open(jpath(path), 'wb') as f:
        f.write(data)


def records(data):
    """Offsets and lengths of complete records (independent parser of the documented layout)."""
    out, o = [], META
    while o + 16 <= len(data):
        (n,) = struct.unpack('>I', data[o + 12:o + 16])
        end = o + REC_FIXED + n
        if end > len(data):
            break
        out.append((o, n))
        o = end
    return out


def err_kind(obj):
    return obj.get('error', {}).get('kind')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', required=True)
    ap.add_argument('--fixtures', required=True)
    a = ap.parse_args()
    binary, fixtures = os.path.abspath(a.bin), os.path.abspath(a.fixtures)
    tmp = tempfile.mkdtemp(prefix='lp2-e2e-')
    with open(os.path.join(fixtures, 'chain.json'), 'rb') as f:
        chain = [h['rlp_hex'] for h in json.load(f)['headers']]
    try:
        s = os.path.join(tmp, 'main')
        code, o, _ = run(binary, fixtures, 'status', '--store', s)
        check('missing store is noStore', code == 2 and err_kind(o) == 'noStore', o)
        code, o, _ = run(binary, fixtures, 'init', '--store', s)
        check('init empty', code == 0 and o.get('candidateHeight') == 0 and o.get('executedHeight') == 0 and o.get('consensus') is False
              and o.get('awaiting') == 'H-full', o)
        code, o, _ = run(binary, fixtures, 'init', '--store', s)
        check('init never overwrites', code == 2 and err_kind(o) == 'exists', o)
        code, o, _ = run(binary, fixtures, 'append', '--store', s, '--range', '1:7')
        check('append batch 1..7', code == 0 and o.get('appended') == list(range(1, 8)), o)
        code, o, _ = run(binary, fixtures, 'append', '--store', s, '--range', '8:20')
        check('append batch 8..20 in a new process', code == 0 and o.get('candidateHeight') == 20, o)
        code, st, _ = run(binary, fixtures, 'status', '--store', s)
        good = read(s)
        check('status after restart', code == 0 and st.get('candidateHeight') == 20 and st.get('tail') == 'clean'
              and st.get('journalSha256') == hashlib.sha256(good).hexdigest() and st.get('canonical') is False, st)
        recs = records(good)
        check('independent record parse', len(recs) == 20 and all(good[o + HDR_AT:o + HDR_AT + n].hex() == chain[i] for i, (o, n) in enumerate(recs)), len(recs))
        code, o, _ = run(binary, fixtures, 'append', '--store', s, '--hex', chain[19])
        check('idempotent tip replay', code == 0 and o.get('appended') == [] and o.get('idempotentTipReplays') == 1 and read(s) == good, o)
        code, o, _ = run(binary, fixtures, 'append', '--store', s, '--hex', chain[4])
        check('conflicting height is notLinear', code == 1 and err_kind(o) == 'notLinear' and read(s) == good, o)

        s7 = os.path.join(tmp, 'seven')
        run(binary, fixtures, 'init', '--store', s7)
        run(binary, fixtures, 'append', '--store', s7, '--range', '1:7')
        before = read(s7)
        bad = chain[7][:-2] + '%02x' % (int(chain[7][-2:], 16) ^ 2)  # winnerSig v -> 2 or 3
        code, o, _ = run(binary, fixtures, 'append', '--store', s7, '--hex', bad)
        check('invalid winner signature rejected', code == 1 and err_kind(o) == 'rejected' and o['error'].get('rule') == '8' and read(s7) == before, o)
        code, o, _ = run(binary, fixtures, 'append', '--store', s7, '--hex', 'c0')
        check('malformed header rejected', code == 1 and o['error'].get('rule') == '1' and read(s7) == before, o)
        code, o, _ = run(binary, fixtures, 'append', '--store', s7, '--hex', 'ab' * 3068)
        check('oversized header bounded', code == 2 and err_kind(o) == 'bound' and read(s7) == before, o)

        fx2 = os.path.join(tmp, 'fixtures-mismatch')
        os.mkdir(fx2)
        with open(os.path.join(fixtures, 'profile.json'), 'rb') as f:
            prof = f.read()
        with open(os.path.join(fx2, 'profile.json'), 'wb') as f:
            f.write(prof + b'\n')
        shutil.copyfile(os.path.join(fixtures, 'chain.json'), os.path.join(fx2, 'chain.json'))
        code, o, _ = run(binary, fx2, 'status', '--store', s)
        check('profile byte mismatch', code == 2 and err_kind(o) == 'profileMismatch', o)

        last_o, last_n = recs[-1]
        torn = os.path.join(tmp, 'torn')
        raw_store(torn, good[:-10])
        src_sha = hashlib.sha256(read(torn)).hexdigest()
        code, o, lines = run(binary, fixtures, 'status', '--store', torn)
        first = json.loads(lines[0]) if lines else {}
        check('torn tail needs recovery', code == 3 and first.get('tail') == 'recoveryRequired' and first.get('candidateHeight') == 19
              and first.get('verifiedBytes') == last_o, lines)
        code, o, _ = run(binary, fixtures, 'append', '--store', torn, '--range', '20:20')
        check('writer refuses torn store', code == 3 and err_kind(o) == 'recoveryRequired', o)
        dest = os.path.join(tmp, 'recovered')
        code, o, _ = run(binary, fixtures, 'recover', '--store', torn, '--dest', dest)
        check('recover into new destination', code == 0 and o.get('candidateHeight') == 19 and o.get('discardedTailBytes') == len(good) - 10 - last_o
              and hashlib.sha256(read(torn)).hexdigest() == src_sha and read(dest) == good[:last_o], o)
        code, o, _ = run(binary, fixtures, 'recover', '--store', torn, '--dest', dest)
        check('existing destination refused', code == 2 and err_kind(o) == 'exists' and read(dest) == good[:last_o], o)
        code, o, _ = run(binary, fixtures, 'append', '--store', dest, '--range', '20:20')
        check('recovered store continues', code == 0 and o.get('candidateHeight') == 20, o)

        corrupt = os.path.join(tmp, 'corrupt')
        m = bytearray(good)
        m[recs[9][0] + 60] ^= 1  # blockHash field of record 10
        raw_store(corrupt, bytes(m))
        code, o, _ = run(binary, fixtures, 'status', '--store', corrupt)
        check('complete corrupt record', code == 2 and err_kind(o) == 'corrupt' and o['error'].get('seq') == 10, o)
        cdest = os.path.join(tmp, 'corrupt-dest')
        code, o, _ = run(binary, fixtures, 'recover', '--store', corrupt, '--dest', cdest)
        check('corrupt source not recovered', code == 2 and err_kind(o) == 'corrupt' and not os.path.exists(cdest), o)

        for name, data, kind in (('empty-file', b'', 'metadataIncomplete'), ('half-meta', good[:50], 'metadataIncomplete'),
                                 ('bad-meta', b'X' + good[1:], 'metadataCorrupt')):
            p = os.path.join(tmp, name)
            raw_store(p, data)
            code, o, _ = run(binary, fixtures, 'status', '--store', p)
            check('metadata ' + name, code == 2 and err_kind(o) == kind, o)

        big = os.path.join(tmp, 'big')
        os.mkdir(big)
        with open(jpath(big), 'wb') as f:
            f.truncate(MAX_FILE + 1)
        code, o, _ = run(binary, fixtures, 'status', '--store', big)
        check('file bound before reading', code == 2 and err_kind(o) == 'bound', o)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    failed = [c for c in CHECKS if not c['ok']]
    print(json.dumps({'e2e': 'lp2-store', 'checks': len(CHECKS), 'failed': len(failed), 'failures': failed}))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
