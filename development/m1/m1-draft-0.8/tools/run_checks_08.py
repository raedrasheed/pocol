"""Reference-only runner for M1 draft 0.8 (R8-01, C23). Python standard library only.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.8\\tools\\run_checks_08.py
Writes only m1-draft-0.8/results/run-results-0.8.json.

What it does:
1. Verifies the lineage hashes of the inherited files (values recorded by the
   coordinator in coordination/shared-repo/development/m1/PUBLICATION-MANIFEST.json,
   field localSha256). A mismatch is a FAIL.
2. Imports m1-draft-0.7/tools/run_checks_07.py as a module (its main() is NOT
   called, so nothing is written into 0.7) and runs every 0.7 step unchanged,
   except e_checks.
3. Runs e_checks_08: identical to 0.7 e_checks except that the U3 state is read
   from m1-draft-0.4/vectors/snapshot-invariants.json, where that fixture lives
   (0.7 looked in m1-draft-0.6, which has no such file).
"""

import hashlib
import json
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
sys.path.insert(0, str(ROOT / 'm1-draft-0.7' / 'tools'))

import run_checks_07 as R7                     # noqa: E402  (module import only; main() not called)

LINEAGE = {   # path -> localSha256 recorded by the coordinator (PUBLICATION-MANIFEST.json)
    'm1-draft-0.4/vectors/snapshot-invariants.json': '760ef12403607d628149999be3f0a5adb5908608c35d03d8525f8c591418f957',
    'm1-draft-0.3/vectors/snapshot-mock-scenarios.json': 'b1602db9256512ad2f40ca4d374c40ee55b39aa34be8758a9a426869a86a484a',
    'm1-draft-0.7/vectors/snapshot-e-cases.json': '1b875889f6044f0732be744fd390e850751cd8d3149e201006eefdebb634f47e',
    'm1-draft-0.7/tools/run_checks_07.py': 'badd6e346db21b0ff6463a9ae3e670e8b0f896c73d1db286bc41f7b782cbc8fa',
}
SNAPSHOT_INVARIANTS = ROOT / 'm1-draft-0.4' / 'vectors' / 'snapshot-invariants.json'   # C23 fix


def lineage_checks():
    for rel, want in LINEAGE.items():
        p = ROOT / rel
        got = hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None
        R7.check('lineage.sha256 ' + rel, got == want, expected=want, got=got)
    R7.check('c23.wrongPathAbsent m1-draft-0.6/vectors/snapshot-invariants.json',
             not (ROOT / 'm1-draft-0.6' / 'vectors' / 'snapshot-invariants.json').exists())
    R7.check('c23.sourceHasU3', 'U3' in R7.load(SNAPSHOT_INVARIANTS)['states'])
    # the other 0.6 fixture that 0.7 reads (epoch_checks) must still resolve
    R7.check('c23.other06ReferenceResolves profile-race-cases.json',
             (ROOT / 'm1-draft-0.6' / 'vectors' / 'profile-race-cases.json').exists())


def e_checks_08():
    """0.7 e_checks with the corrected U3 source; logic otherwise unchanged."""
    states = R7.S4.upgrade_03_states(R7.load(ROOT / 'm1-draft-0.3' / 'vectors' / 'snapshot-mock-scenarios.json')['states'])
    states['U3'] = R7.load(SNAPSHOT_INVARIANTS)['states']['U3']
    R7.record('snapshotE.selector', signature=R7.SE.GETTER_SIGNATURE,
              selector='0x' + R7.keccak256(R7.SE.GETTER_SIGNATURE.encode())[:4].hex())
    for c in R7.load(ROOT / 'm1-draft-0.7' / 'vectors' / 'snapshot-e-cases.json')['cases']:
        if 'sequence' in c:
            outs = [R7.SE.load_e(states, {'from': st}, c['request']) for st in c['sequence']]
            ok = [o.get('manifestHash') for o in outs] == c['expected']['renderedHashes'] and not any(o.get('mixed') for o in outs)
            got = [o.get('manifestHash') for o in outs]
        else:
            o = R7.SE.load_e(states, c['response'], c['request'])
            ok = all(o.get(k) == v for k, v in c['expected'].items())
            got = o
        R7.check('snapshotE.' + c['id'], ok, got=got)


def main():
    t0 = time.time()
    R7.check('keccak.selftest', R7.keccak.selftest())
    steps = [lineage_checks, R7.c20_checks, R7.epoch_checks, R7.br19_checks, R7.mr_checks,
             R7.session_checks, e_checks_08, R7.fetch_rule_checks]
    for step in steps:
        try:
            step()
        except Exception as e:                                      # record, never hide
            R7.check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    results = R7.results
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.8 (C23 repair of 0.7)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'inheritance': '0.7 steps run unchanged from m1-draft-0.7/tools/run_checks_07.py; e_checks replaced by e_checks_08 (U3 source path)',
           'evidenceKind': 'reference models of the specification text only',
           'notExecuted': ['browser / MV3 extension, DNR', 'MaliciousHttp over sockets, TestHooks', 'anvil / pocold / eth_call getter / MPT',
                           'ESLint / dependency-cruiser', 'real transactions'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.8.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:300])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
