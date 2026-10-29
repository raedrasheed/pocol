"""Execute every check this package claims, and record machine-readable results.

Usage: python run_checks.py   (from the tools directory)
Writes ../results/run-results.json and ../results/fixture-hashes.json.
Exit status is non-zero if any check fails.
"""

import hashlib
import json
import platform
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

import keccak
import m1model as M
import m1paths
import m1abi
import gen_fixtures

HERE = Path(__file__).resolve().parent
PKG = HERE.parent
VEC = PKG / 'vectors'
DRAFT01 = PKG.parent / 'vectors'

RULES = [
    'decode.oversize', 'decode.truncated', 'decode.noncanonical', 'decode.trailing',
    'struct.shape', 'struct.int', 'struct.width',
    'manifest.version', 'manifest.fileCount', 'manifest.entryIndex',
    'path.length', 'path.grammar', 'path.dotSegment', 'path.order',
    'file.mime', 'entry.mime', 'file.size', 'chunk.len', 'file.sum', 'site.size',
    'fetch.missing', 'fetch.prefix', 'fetch.length', 'fetch.origin', 'file.hash',
    'manifest.length', 'manifest.split', 'mfetch.missing', 'manifest.hash',
]
RULES_WITHOUT_FIXTURE_BY_DESIGN = {
    'mfetch.prefix': 'same function as fetch.prefix (check_chunk); covered there',
    'mfetch.length': 'same function as fetch.length; covered there',
    'mfetch.origin': 'same function as fetch.origin; covered there',
}

results = []


def check(name, ok, **detail):
    results.append({'check': name, 'status': 'pass' if ok else 'FAIL', **detail})
    return ok


def load(name):
    return json.loads((VEC / name).read_text())


def provider_for(fx, table):
    codes = {bytes.fromhex(a[2:]): bytes.fromhex(table[a][2:]) for a in fx.get('provider', [])}
    for a, c in fx.get('providerOverrides', {}).items():
        codes[bytes.fromhex(a[2:])] = bytes.fromhex(c[2:])
    return codes


def sha256_tree(root):
    return {str(p.relative_to(root)).replace('\\', '/'): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file()}


def main():
    t0 = time.time()
    check('keccak.selftest', keccak.selftest())

    # Well-known 4-byte selectors (independent public values) exercise the selector path.
    known = {'owner()': '0x8da5cb5b', 'transferOwnership(address)': '0xf2fde38b',
             'acceptOwnership()': '0x79ba5097', 'pendingOwner()': '0xe30c3978'}
    for sig, sel in known.items():
        check('abi.knownSelector ' + sig, m1abi.selector(sig) == sel, got=m1abi.selector(sig))

    # Draft 0.1 reproduction and determinism.
    with tempfile.TemporaryDirectory() as d1, tempfile.TemporaryDirectory() as d2:
        gen_fixtures.CODE_TABLE.clear(); gen_fixtures.main(d1)
        gen_fixtures.CODE_TABLE.clear(); gen_fixtures.main(d2)
        for f in ('chunks.json', 'index.html', 'manifest.json', 'negative-manifests.json'):
            same = (Path(d1) / 'draft01-repro' / f).read_bytes() == (DRAFT01 / f).read_bytes()
            check('draft01.byteIdentical ' + f, same)
        h1, h2, hv = sha256_tree(Path(d1)), sha256_tree(Path(d2)), sha256_tree(VEC)
        check('generator.deterministic (two runs)', h1 == h2)
        check('generator.committedFixturesCurrent', h1 == hv,
              differing=sorted(k for k in set(h1) | set(hv) if h1.get(k) != hv.get(k)))

    table = load('code-table.json')['codes']

    # Honest code table: every entry is 0x00||data at its factory-derived address.
    bad = [a for a, c in table.items()
           if M.chunk_address(bytes.fromhex(c[4:])) != bytes.fromhex(a[2:]) or c[:4] != '0x00']
    check('codeTable.allEntriesFactoryDerived', not bad, bad=bad)

    # Positive fixtures.
    for fx in load('manifest-positive.json')['fixtures']:
        m = bytes.fromhex(fx['manifest'][2:])
        r = M.validate_manifest(m, provider_for(fx, table))
        check('positive ' + fx['id'], r['result'] == 'accept' and len(m) == fx['manifestLength'],
              got=r.get('rule'), boundary=fx.get('boundary'))

    # Negative fixtures: expected first rule, stage, fetch count, then isolation.
    neg = load('manifest-negative.json')['fixtures']
    covered = set()
    for fx in neg:
        m = bytes.fromhex(fx['manifest'][2:])
        prov = provider_for(fx, table)
        r = M.validate_manifest(m, prov)
        exp = fx['expected']
        ok = r['result'] == 'reject' and r['rule'] == exp['firstRule'] and r['stage'] == exp['stage']
        if exp.get('fetchCalls') is not None:
            ok = ok and r['fetchCalls'] == exp['fetchCalls']
        check('negative ' + fx['id'], ok, expected=exp['firstRule'], got=r.get('rule'),
              stage=r.get('stage'), fetchCalls=r['fetchCalls'])
        covered.add(fx['rule'])
        iso = fx['isolation']
        if iso == 'none':
            continue
        d = M.validate_manifest(m, prov, disabled=frozenset([fx['rule']]))
        if iso == 'full':
            check('isolation.full ' + fx['id'], d['result'] == 'accept', got=d.get('rule'))
        else:
            check('isolation.next ' + fx['id'], d.get('rule') == fx['nextRuleWhenDisabled'],
                  expected=fx['nextRuleWhenDisabled'], got=d.get('rule'))

    # Version-record (manifest retrieval) fixtures.
    for fx in load('version-records.json')['fixtures']:
        rec = dict(fx['record'])
        rec['manifestHash'] = bytes.fromhex(rec['manifestHash'][2:])
        rec['chunks'] = [(bytes.fromhex(a[2:]), ln) for a, ln in rec['chunks']]
        prov = provider_for(fx, table)
        r = M.validate_version(rec, prov)
        exp = fx['expected']
        if exp['result'] == 'accept':
            check('version ' + fx['id'], r['result'] == 'accept', got=r.get('rule'))
            continue
        covered.add(exp['firstRule'])
        check('version ' + fx['id'], r.get('rule') == exp['firstRule'], expected=exp['firstRule'],
              got=r.get('rule'))
        iso = fx.get('isolation', 'none')
        if iso != 'none':
            d = M.validate_version(rec, prov, disabled=frozenset([exp['firstRule']]))
            want = 'accept' if iso == 'full' else fx['nextRuleWhenDisabled']
            got = d['result'] if iso == 'full' else d.get('rule')
            check('isolation.%s %s' % (iso, fx['id']), got == want, expected=want, got=got)

    missing = [r for r in RULES if r not in covered]
    check('coverage.everyRuleHasRejectFixture', not missing, missing=missing,
          byDesign=RULES_WITHOUT_FIXTURE_BY_DESIGN)

    # Boundary arithmetic quoted in the spec.
    for n, chunks, last in ((1048576, 43, 16426), (1048577, 43, 16427)):
        parts = M.split(b'\x00' * n)
        check('boundary.split %d' % n, len(parts) == chunks and len(parts[-1]) == last,
              got=[len(parts), len(parts[-1])])
    check('boundary.T25 v2 content chunks = 44',
          len(M.split(b'\x00' * 4100)) + len(M.split(b'\x00' * 1032192)) == 44)

    # Path model against hand-written expectations.
    pc = load('path-cases.json')
    for c in pc['omnibox']:
        got = m1paths.omnibox(c['input'])
        check('path.omnibox %r' % c['input'][:40], got == c['expected'], got=got)
    for c in pc['navigate']:
        got = m1paths.navigate(c['input'])
        check('path.navigate %r' % c['input'][:40], got == c['expected'], got=got)
    for c in pc['reference']:
        got = m1paths.reference(c['input'], c['base'])
        check('path.reference %r @ %s' % (c['input'][:40], c['base']), got == c['expected'], got=got)

    failed = [r for r in results if r['status'] != 'pass']
    out = {
        'package': 'M1 draft 0.2',
        'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
        'environment': {'python': sys.version.split()[0], 'platform': platform.platform(),
                        'thirdPartyPackages': 'none'},
        'notExecuted': ['EVM / anvil', 'Solidity compilation', 'gas measurement',
                        'browser / extension', 'pocold', 'M0 K1-K3 hash libraries',
                        'pycryptodome / pyrlp cross-check (not installed here)'],
        'summary': {'checks': len(results), 'passed': len(results) - len(failed),
                    'failed': len(failed), 'seconds': round(time.time() - t0, 1)},
        'results': results,
    }
    res = PKG / 'results'
    res.mkdir(exist_ok=True)
    (res / 'run-results.json').write_text(json.dumps(out, indent=2) + '\n')
    (res / 'fixture-hashes.json').write_text(json.dumps(
        {'algorithm': 'sha256', 'files': sha256_tree(VEC)}, indent=2) + '\n')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f)[:300])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
