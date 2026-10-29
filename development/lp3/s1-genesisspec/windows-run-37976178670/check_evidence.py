"""Re-checks the preserved evidence of Windows run 37976178670. Python stdlib and git only.
    python -I development/lp3/s1-genesisspec/windows-run-37976178670/check_evidence.py --repo . \
        [--blocked-out development/lp3/s1-genesisspec/windows-run-37976178670/blocked-checks.json]

1. The artifact zip's SHA-256 equals the digest GitHub recorded for artifact 11641114416, and the job log reports
   the same upload digest, the tested commit and the verification script's SHA-256.
2. Every file under artifact/ equals the zip entry of the same name, byte for byte; nothing is missing or extra.
3. results.json names the tested commit, and its counts equal the counts recomputed from its rows.
4. source-hashes.json: every Windows working-tree hash equals the git blob at the tested commit, either unchanged
   or after LF -> CRLF checkout conversion.
5. The differential's pinned M1 inputs equal the git blobs unchanged, and its corpus hashes equal the Linux runs.
Then every BLOCKED BY PLATFORM row is mapped to the checks that ran on Windows for the same input, and to the
Linux RLIMIT_AS evidence already in the repository. Blocked rows are never counted as passed. Exit 1 if any
check fails.
"""

import argparse
import hashlib
import json
import subprocess
import sys
import zipfile
from pathlib import Path

RUN = 37976178670
ARTIFACT_ID = 11641114416
DIGEST = '63f65757d4879b09abeeccf8a88bf3d36ec7143fd0cb308dbd8a27b428871c22'
COMMIT = '4e26e4d3049731645ab1d85ccfb9972d29d63390'
SCRIPT_SHA = 'c2eb204deff90d4577575e473f49f3f483998364ecbd2faa2949839405e3d43f'
ZIP = 'lp3-s1-windows-%s.zip' % COMMIT
S1 = Path('development/lp3/s1-genesisspec')

# Per blocked case: the Windows checks that ran for the same or a larger input (heap-level allocation cases,
# CLI cases without a limit, differential ladder inputs). None of them applies an OS memory limit.
WINDOWS_COVER = {
    'largeOneLine192MiB.limit256MiB': {'alloc': ['oversizedLine256MiB']},
    'largeOneLine192MiB.limit64MiB': {'alloc': ['oversizedLine256MiB']},
    'limitPlusOneThenMaxValid': {'alloc': ['limitPlusOneThenMaxValid'], 'cli': ['limitPlusOneThenMaxValid.noLimit'],
                                 'diff': ['ladder.maxValid', 'ladder.maxValidUnordered']},
    'maxDeepAndFlat': {'alloc': ['maxDeepNest', 'maxFlatList'], 'diff': ['ladder.maxDeep', 'ladder.maxFlat']},
    'gsv1Times100000': {'alloc': ['gsv1Times30000']},
    'gsVersion262144Bytes': {'alloc': ['gsVersion256KiB'], 'diff': ['ladder.version262144.*']},
    'gsVersion2818466Bytes': {'alloc': ['gsVersionMax'], 'diff': ['ladder.version2818466.*']},
    'reservationFailure.limit8MiB': {},
}
# Linux RLIMIT_AS evidence for the same cases: (file, build, what ran).
LINUX = [
    ('author-r2/lp3-genesis-cli.json', 'debug', 'round 2 (26ee0a4), native Linux'),
    ('author-r2/lp3-genesis-cli-release-max.json', 'release-max', 'round 2 (26ee0a4), native Linux'),
    ('author-r4-encoding-fix/fixed/fixed-sim-lp3_genesis_cli.json', 'debug',
     'round 4 harness (086a3ea), Linux with simulated Windows text defaults'),
    ('author-r4-encoding-fix/fixed/fixed-sim-max-lp3_genesis_cli.json', 'release-max',
     'round 4 harness (086a3ea), Linux with simulated Windows text defaults'),
    ('author-r4-encoding-fix/fixed/fixed-strict-lp3_genesis_cli.json', 'debug',
     'round 4 harness (086a3ea), native Linux, EncodingWarning as error'),
]
SMOKE = 'author-r4-encoding-fix/linux-smoke-not-windows-evidence/results.json'


def sha(b):
    return hashlib.sha256(b).hexdigest()


def blob(repo, path):
    r = subprocess.run(['git', 'show', '%s:%s' % (COMMIT, path)], cwd=repo, capture_output=True)
    return r.stdout if r.returncode == 0 else None


def load(p):
    return json.loads(Path(p).read_text(encoding='utf-8-sig'))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--repo', required=True)
    ap.add_argument('--blocked-out')
    a = ap.parse_args()
    repo = Path(a.repo).resolve()
    ev = repo / S1 / ('windows-run-%d' % RUN)
    art = ev / 'artifact'
    problems = []

    def need(ok, what):
        print(('ok    ' if ok else 'FAIL  ') + what)
        if not ok:
            problems.append(what)

    # 1. Zip digest and job log.
    zbytes = (ev / ZIP).read_bytes()
    need(sha(zbytes) == DIGEST, 'artifact zip SHA-256 %s equals GitHub digest of artifact %d' % (sha(zbytes), ARTIFACT_ID))
    joblog = (ev / 'github-job-log' / '0_Windows Server 2022, Rust 1.58.1.txt').read_text(encoding='utf-8-sig')
    for s in ['SHA256 digest of uploaded artifact is ' + DIGEST, 'HEAD is now at ' + COMMIT[:7],
              'commit %s, verification script SHA-256 %s' % (COMMIT, SCRIPT_SHA), 'INCLUDE_SLOW: true',
              'PASS 20, FAIL 0, BLOCKED BY PLATFORM 16, NOT RUN 0']:
        need(s in joblog, 'job log contains %r' % s)

    # 2. Extracted files equal the zip entries.
    with zipfile.ZipFile(ev / ZIP) as z:
        names = sorted(i.filename for i in z.infolist() if not i.is_dir())
        same = all((art / n).read_bytes() == z.read(n) for n in names)
    on_disk = sorted(p.relative_to(art).as_posix() for p in art.rglob('*') if p.is_file())
    need(same and on_disk == names, '%d files under artifact/ equal the %d zip entries byte for byte' % (len(on_disk), len(names)))

    # 3. results.json.
    res = load(art / 'results.json')
    rows = res['results']
    counts = {}
    for r in rows:
        counts[r['status']] = counts.get(r['status'], 0) + 1
    need(res['commit'] == COMMIT and res['trackedFilesModified'] is False, 'results.json commit %s, trackedFilesModified false' % res['commit'])
    need(all(counts.get(k, 0) == v for k, v in res['counts'].items()) and sum(res['counts'].values()) == len(rows),
         'counts recomputed from %d rows: %s' % (len(rows), counts))
    need(counts.get('FAIL', 0) == 0 and counts.get('NOT RUN', 0) == 0, 'no FAIL and no NOT RUN row')

    # 4. Working-tree hashes against the commit.
    src = load(art / 'source-hashes.json')
    need(src['commit'] == COMMIT, 'source-hashes.json commit')
    kinds = {'unchanged': [], 'crlf': [], 'unexplained': []}
    for f in sorted(src['files'], key=lambda f: f['path']):
        path, h = f['path'], f['sha256']
        b = blob(repo, path)
        if b is not None and sha(b) == h:
            kinds['unchanged'].append(path)
        elif b is not None and sha(b.replace(b'\r\n', b'\n').replace(b'\n', b'\r\n')) == h:
            kinds['crlf'].append(path)
        else:
            kinds['unexplained'].append(path)
    need(not kinds['unexplained'], 'source-hashes: %d files equal the git blob, %d equal it after LF->CRLF checkout conversion, %d unexplained'
         % (len(kinds['unchanged']), len(kinds['crlf']), len(kinds['unexplained'])))

    # 5. Differential inputs and corpus.
    diffs = {'debug': load(art / 'lp3-genesis-diff.json'), 'release-max': load(art / 'lp3-genesis-diff-release-max.json')}
    linux_corpus = {'debug': load(repo / S1 / 'author-r4-encoding-fix/fixed/fixed-sim-lp3_genesis_diff.json')['corpusSha256'],
                    'release-max': load(repo / S1 / 'author-r4-encoding-fix/fixed/fixed-sim-max-lp3_genesis_diff.json')['corpusSha256']}
    for build, d in diffs.items():
        pins = all(sha(blob(repo, 'development/m1/' + p) or b'') == h for p, h in d['pinnedInputs'].items())
        need(pins, '%s differential: %d pinned M1 inputs equal the git blobs unchanged' % (build, len(d['pinnedInputs'])))
        need(d['corpusSha256'] == linux_corpus[build], '%s differential corpus %s equals the Linux corpus' % (build, d['corpusSha256'][:16]))
        need(d['ok'] and d['mismatches'] == 0 and d['agree'] == d['inputs'], '%s differential %d/%d agree' % (build, d['agree'], d['inputs']))

    # Blocked rows: overlap and coverage.
    alloc = {}
    for build, log in (('debug', 'genesis-batch-alloc-debug.log'), ('release-max', 'genesis-batch-alloc-release-max.log')):
        alloc[build] = {}
        for line in (art / 'logs' / log).read_text(encoding='utf-8').splitlines():
            if line.startswith('{"case"'):
                c = json.loads(line)
                alloc[build][c['case']] = c
    cli = {'debug': load(art / 'lp3-genesis-cli.json'), 'release-max': load(art / 'lp3-genesis-cli-release-max.json')}
    smoke = {r['check']: r for r in load(repo / S1 / SMOKE)['results']}
    linux = [(f, b, what, {r['check']: r for r in load(repo / S1 / f)['results']}) for f, b, what in LINUX]

    blocked = [r for r in rows if r['status'] == 'BLOCKED BY PLATFORM']
    out = []
    for r in blocked:
        parent, _, case = r['check'].partition(': ')
        if not case:
            out.append({'row': r['check'], 'reason': r['detail'], 'kind': 'summary row',
                        'overlap': 'restates the other %d per-case rows; no separate test' % (len(blocked) - 1)})
            continue
        build = 'release-max' if parent.endswith('release-max') else 'debug'
        other = [x['check'] for x in blocked if x['check'].endswith(': ' + case) and x['check'] != r['check']]
        cover = WINDOWS_COVER[case]
        win = []
        for n in cover.get('alloc', []):
            c = alloc[build].get(n)
            need(c is not None and c['ok'], 'Windows coverage for %s: allocation case %s (%s) ran and passed' % (r['check'], n, build))
            win.append({'check': 'genesis-batch-alloc-%s: %s' % (build, n), 'status': 'PASS' if c and c['ok'] else 'missing',
                        'peakHeapBytes': c and c['peakHeapBytes'], 'boundBytes': c and c['boundBytes']})
        for n in cover.get('cli', []):
            c = next((x for x in cli[build]['results'] if x['check'] == n), None)
            need(c is not None and c['status'] == 'PASS', 'Windows coverage for %s: CLI case %s ran and passed' % (r['check'], n))
            win.append({'check': '%s: %s' % (parent, n), 'status': c and c['status'], 'exit': c and c.get('exit')})
        if cover.get('diff'):
            d = diffs[build]
            win.append({'check': 'lp3-genesis-diff%s ladder inputs %s' % ('' if build == 'debug' else '-release-max', ', '.join(cover['diff'])),
                        'status': 'PASS' if d['mismatches'] == 0 else 'FAIL', 'ladderCodes': d['byGroup']['ladder']})
        lin = []
        for f, b, what, res_ in linux:
            x = res_.get(case)
            if b == build and x:
                need(x['status'] == 'PASS' and x.get('limitMiB'), 'Linux RLIMIT_AS evidence for %s in %s: PASS at %s MiB' % (r['check'], f, x.get('limitMiB')))
                lin.append({'file': str(S1 / f), 'ran': what, 'status': x['status'], 'limitMiB': x.get('limitMiB'), 'exit': x.get('exit'),
                            'signal': x.get('signal'), 'maxRssKiB': x.get('maxRssKiB'), 'seconds': x.get('seconds')})
        s = smoke.get(r['check'])
        if s:
            need(s['status'] == 'PASS', 'Linux CI-script smoke run (086a3ea) for %s: PASS' % r['check'])
            lin.append({'file': str(S1 / SMOKE), 'ran': 'CI script on Linux at 086a3ea', 'status': s['status'], 'detail': s['detail']})
        need(bool(lin), 'Linux evidence exists for %s' % r['check'])
        out.append({'row': r['check'], 'build': build, 'case': case, 'reason': r['detail'], 'kind': 'per-case row',
                    'overlap': ('same case also blocked as ' + ', '.join(other)) if other else 'only in the %s run' % build,
                    'windowsRanForSameInput': win or 'nothing: no Windows check exercises a failed memory reservation',
                    'linuxEvidence': lin})
    cases = sorted({o['case'] for o in out if 'case' in o})
    print('blocked rows %d = 1 summary row + %d per-case rows; distinct cases %d; distinct reasons %s'
          % (len(out), len(out) - 1, len(cases), sorted({o['reason'] for o in out})))
    if a.blocked_out:
        Path(a.blocked_out).write_text(json.dumps({'run': RUN, 'commit': COMMIT, 'blockedRows': len(out), 'distinctCases': cases,
                                                   'note': 'BLOCKED BY PLATFORM rows are not passes. Windows coverage below is heap-level or '
                                                           'without a memory limit; OS memory-limit behaviour is evidenced on Linux only.',
                                                   'rows': out}, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')
    print('problems: %d' % len(problems))
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
