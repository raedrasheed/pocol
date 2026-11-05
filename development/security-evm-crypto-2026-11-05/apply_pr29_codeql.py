#!/usr/bin/env python3
"""Verify the 11 reviewed PR29 findings; --apply uses the owner's normal gh session.

No alert IDs are guessed. Discover only exact reviewed rule/location matches on
PR29, then verify every open instance and immutable source across all refs.
The older main-branch batch and its guard remain unchanged.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import quote

from apply_reviewed_codeql import Blocked, GitHub, REPOSITORY

PLAN_SHA256 = '2caff84219b7b409206b1c1adc43b9ec33f95e4385ce77ececddceb47c41e33c'
REF = 'refs/pull/29/merge'
FIELDS = ['path', 'start_line', 'end_line', 'start_column', 'end_column']


def key(rule, location):
    return (rule, *(location.get(field) for field in FIELDS))


def load_plan():
    text = Path(__file__).with_name('reviewed-pr29-codeql.json').read_text(
        encoding='utf-8-sig').replace('\r\n', '\n')
    if hashlib.sha256(text.encode()).hexdigest() != PLAN_SHA256:
        raise Blocked('Reviewed PR29 plan changed')
    plan = json.loads(text)
    rows = plan['alerts']
    if (plan['repository'] != REPOSITORY or plan['pull_request'] != 29
            or len(rows) != 11 or len({key(r['rule'], r['location']) for r in rows}) != 11
            or any(r['reason'] != 'false positive' or not r['comment']
                   or len(r['comment']) > 512 for r in rows)):
        raise Blocked('Unexpected reviewed PR29 scope')
    return rows


def heads(client):
    pr = client.request(f'repos/{REPOSITORY}/pulls/29')
    if (pr['state'] != 'open' or pr['base']['ref'] != 'main'
            or pr['head']['repo']['full_name'] != REPOSITORY
            or pr['head']['ref'] != 'security/evm-crypto-2026-11-05'):
        raise Blocked('PR29 source or target changed, or the PR is closed')
    current = (client.head(), pr['head']['sha'])
    if any(not re.fullmatch(r'[0-9a-f]{40}', value) for value in current):
        raise Blocked('Missing source commit identity')
    return current


def discover(client, rows):
    wanted = {key(r['rule'], r['location']) for r in rows}
    found = {}
    # Closed findings make repeated owner runs idempotent. Missing or duplicate
    # matches stop the batch before any writes; unrelated findings are untouched.
    for state in ['open', 'dismissed', 'fixed']:
        for page in range(1, 1001):
            endpoint = (f'repos/{REPOSITORY}/code-scanning/alerts?ref={REF}'
                        f'&tool_name=CodeQL&state={state}&per_page=100&page={page}')
            alerts = client.request(endpoint)
            if not isinstance(alerts, list):
                raise Blocked('Unexpected alert list response')
            for alert in alerts:
                identity = key(alert['rule']['id'], alert.get('most_recent_instance', {}).get('location', {}))
                if identity not in wanted or alert['tool']['name'] != 'CodeQL':
                    continue
                number = alert['number']
                if type(number) is not int or number <= 0:
                    raise Blocked('Invalid server alert identifier')
                if identity in found and found[identity] != number:
                    raise Blocked('Ambiguous reviewed alert match')
                found[identity] = number
            if len(alerts) < 100:
                break
        else:
            raise Blocked('Too many alerts to validate safely')
    if set(found) != wanted:
        raise Blocked('Not all 11 reviewed locations were found; inspect the current PR findings')
    return found


def verify(client, row, number, expected_heads, apply, cache):
    endpoint = f'repos/{REPOSITORY}/code-scanning/alerts/{number}'
    alert = client.request(endpoint)
    if alert['state'] in ['fixed', 'dismissed']:
        return {'number': number, 'result': 'already closed'}
    if alert['state'] != 'open' or alert['rule']['id'] != row['rule'] or alert['tool']['name'] != 'CodeQL':
        raise Blocked('Rule, tool or state changed')
    instances = [r for r in client.instances(number) if r.get('state') == 'open']
    if not any(r.get('ref') == REF for r in instances):
        raise Blocked('No reviewed open PR29 instance remains')
    for instance in instances:
        location = instance.get('location', {})
        if any(location.get(field) != row['location'].get(field) for field in FIELDS):
            raise Blocked('Another open ref has a different location')
        commit, path = instance.get('commit_sha', ''), location.get('path', '')
        if not re.fullmatch(r'[0-9a-f]{40}', commit) or not path or path.startswith('/') or '..' in path.split('/'):
            raise Blocked('Unsafe or missing immutable source identity')
        identity = (commit, path)
        if identity not in cache:
            content = client.request(f'repos/{REPOSITORY}/contents/{quote(path, safe="/")}?ref={commit}')
            if content.get('type') != 'file' or content.get('encoding') != 'base64':
                raise Blocked('Reviewed source is not an available regular file')
            cache[identity] = hashlib.sha256(base64.b64decode(content['content'])).hexdigest()
        if cache[identity] != row['source_sha256']:
            raise Blocked('Source on an open ref differs from the review')
    if heads(client) != expected_heads:
        raise Blocked('Main or the PR advanced during the batch; restart validation')
    if not apply:
        return {'number': number, 'result': 'verified; no write'}
    response = client.request(endpoint, {'state': 'dismissed', 'dismissed_reason': row['reason'],
                                         'dismissed_comment': row['comment']})
    if response.get('state') != 'dismissed':
        raise Blocked('GitHub did not confirm dismissal')
    return {'number': number, 'result': 'dismissed', 'reason': row['reason']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists() or not args.out.parent.is_dir():
        parser.error('--out must be new and its parent directory must exist')
    report = {'repository': REPOSITORY, 'pull_request': 29, 'apply': args.apply, 'decisions': []}
    code = 0
    try:
        rows, client = load_plan(), GitHub()
        expected = heads(client)
        report['validated_main_and_pr_heads'] = list(expected)
        matches, cache = discover(client, rows), {}
        for row in rows:
            decision = verify(client, row, matches[key(row['rule'], row['location'])], expected, args.apply, cache)
            report['decisions'].append(decision)
            args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
            print(f'#{decision["number"]}: {decision["result"]}', flush=True)
    except (Blocked, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report['blocked'] = str(error)
        print('Stopped: ' + str(error), file=sys.stderr)
        code = 1
    args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    return code


if __name__ == '__main__':
    sys.exit(main())
