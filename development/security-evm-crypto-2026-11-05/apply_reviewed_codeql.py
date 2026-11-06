#!/usr/bin/env python3
"""Verify/apply only the approved CodeQL decisions using the operator's gh session.

Default is read-only. --apply can dismiss a reviewed alert only if every open
instance, across all refs, still matches its reviewed source hash and location.
No credentials are read or requested. Existing gh authentication is preserved.
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

REPOSITORY = 'palcertcom/pocol'
PLAN_SHA256 = 'ae3aab68e25a51b0331f46947dca2cb4ff75acf79e11c68b6852654cde2c43fe'


class Blocked(Exception):
    pass


def load_plan(path):
    text = path.read_text(encoding='utf-8-sig').replace('\r\n', '\n')
    if hashlib.sha256(text.encode('utf-8')).hexdigest() != PLAN_SHA256:
        raise Blocked('The reviewed plan changed; a new source review is required')
    plan = json.loads(text)
    if plan['repository'] != REPOSITORY or len(plan['alerts']) != 197:
        raise Blocked('Unexpected repository or review scope')
    ids = [row['number'] for row in plan['alerts']]
    if len(set(ids)) != 197 or any(type(number) is not int or number <= 0 for number in ids):
        raise Blocked('Invalid reviewed alert identifiers')
    for row in plan['alerts']:
        if row['reason'] not in ['false positive', 'used in tests'] or not row['comment'] or len(row['comment']) > 512:
            raise Blocked('Invalid approved dismissal decision')
    return plan


class GitHub:
    def request(self, endpoint, payload=None):
        command = ['gh', 'api', '-H', 'Accept: application/vnd.github+json',
                   '--method', 'PATCH' if payload is not None else 'GET', endpoint]
        if payload is not None:
            command += ['--input', '-']
        response = subprocess.run(command, input=None if payload is None else
                                  json.dumps(payload, ensure_ascii=False).encode('utf-8'),
                                  capture_output=True, timeout=60)
        if response.returncode:
            try:
                error = json.loads(response.stdout.decode('utf-8-sig'))
                detail = str(error.get('status', '')) + ' ' + str(error.get('message', 'GitHub API failed'))
            except (ValueError, UnicodeError):
                detail = 'gh could not complete the API request'
            raise Blocked(detail.strip())
        return json.loads(response.stdout.decode('utf-8-sig'))

    def head(self):
        value = self.request(f'repos/{REPOSITORY}/git/ref/heads/main')['object']['sha']
        if not re.fullmatch(r'[0-9a-f]{40}', value):
            raise Blocked('Invalid main commit identifier')
        return value

    def instances(self, number):
        result = []
        for page in range(1, 1001):
            rows = self.request(f'repos/{REPOSITORY}/code-scanning/alerts/{number}/instances?per_page=100&page={page}')
            if not isinstance(rows, list):
                raise Blocked('Unexpected alert instance response')
            result.extend(rows)
            if len(rows) < 100:
                return result
        raise Blocked('Too many instances to validate safely')


def verify_and_apply(client, row, expected_head, apply, source_cache):
    endpoint = f'repos/{REPOSITORY}/code-scanning/alerts/{row["number"]}'
    alert = client.request(endpoint)
    if alert['state'] in ['fixed', 'dismissed']:
        return {'number': row['number'], 'result': 'already closed'}
    if alert['state'] != 'open' or alert['rule']['id'] != row['rule'] or alert['tool']['name'] != 'CodeQL':
        raise Blocked('Alert state, tool or rule no longer matches the review')
    instances = [item for item in client.instances(row['number']) if item.get('state') == 'open']
    if not instances or not any(item.get('ref') == 'refs/heads/main' for item in instances):
        raise Blocked('No reviewed open main instance remains')
    for instance in instances:
        location = instance.get('location', {})
        if any(location.get(key) != row['location'].get(key) for key in
               ['path', 'start_line', 'end_line', 'start_column', 'end_column']):
            raise Blocked('An open instance has a different location; review it separately')
        commit = instance.get('commit_sha', '')
        path = location.get('path', '')
        if not re.fullmatch(r'[0-9a-f]{40}', commit) or not path or path.startswith('/') or '..' in path.split('/'):
            raise Blocked('Unsafe or missing source identity')
        key = (commit, path)
        if key not in source_cache:
            content = client.request(f'repos/{REPOSITORY}/contents/{quote(path, safe="/")}?ref={commit}')
            if content.get('type') != 'file' or content.get('encoding') != 'base64':
                raise Blocked('Reviewed source is unavailable as a regular file')
            data = base64.b64decode(content['content'])
            source_cache[key] = hashlib.sha256(data).hexdigest()
        if source_cache[key] != row['source_sha256']:
            raise Blocked('An open instance uses changed source; review it separately')
    if client.head() != expected_head:
        raise Blocked('Main advanced during the run; restart source validation')
    if not apply:
        return {'number': row['number'], 'result': 'verified; no write', 'open_instances': len(instances)}
    response = client.request(endpoint, {'state': 'dismissed', 'dismissed_reason': row['reason'],
                                         'dismissed_comment': row['comment']})
    if response.get('state') != 'dismissed':
        raise Blocked('GitHub did not confirm dismissal; inspect the server policy')
    return {'number': row['number'], 'result': 'dismissed', 'reason': row['reason']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out must be a new file; previous records are never overwritten')
    if not args.out.parent.is_dir():
        parser.error('--out parent directory must already exist')
    report = {'repository': REPOSITORY, 'apply': args.apply, 'decisions': []}
    code = 0
    try:
        plan = load_plan(Path(__file__).with_name('reviewed-codeql-alerts.json'))
        client = GitHub()
        head = client.head()
        report['validated_main_head'] = head
        cache = {}
        for row in plan['alerts']:
            decision = verify_and_apply(client, row, head, args.apply, cache)
            report['decisions'].append(decision)
            args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
            print(f'#{row["number"]}: {decision["result"]}', flush=True)
    except (Blocked, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report['blocked'] = str(error)
        print('Stopped: ' + str(error), file=sys.stderr)
        code = 1
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return code


if __name__ == '__main__':
    sys.exit(main())
