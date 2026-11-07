#!/usr/bin/env python3
"""Verify/apply only the approved CodeQL decisions using the operator's gh session.

Default is read-only. --apply can dismiss a reviewed alert only if every open
instance, across all refs, still matches its reviewed source hash and location.
The complete batch is checked before its first PATCH. The current main source
is also verified, and live metadata/instances are re-read before each dismissal.
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
MAX_DISMISSED_COMMENT_CHARS = 280


class Blocked(Exception):
    pass


def validate_dismissed_comment(comment):
    if not isinstance(comment, str) or not comment or len(comment) > MAX_DISMISSED_COMMENT_CHARS:
        length = len(comment) if isinstance(comment, str) else 'invalid type'
        raise Blocked(f'Dismissal comment must contain 1..{MAX_DISMISSED_COMMENT_CHARS} characters '
                      f'(received {length}); no truncation is performed')


class UnconfirmedMainDismissal(Blocked):
    def __init__(self, number, response):
        self.details = {'number': number, 'patch_sent': True,
                        'response_state': response.get('state') if isinstance(response, dict) else None,
                        'state_present': isinstance(response, dict) and 'state' in response}
        super().__init__(f'PATCH sent for #{number}, but dismissal was not confirmed; inspect unconfirmed_patch')


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
        if row['reason'] not in ['false positive', 'used in tests']:
            raise Blocked('Invalid approved dismissal decision')
        validate_dismissed_comment(row['comment'])
    return plan


class GitHub:
    def request(self, endpoint, payload=None):
        if payload is not None and 'dismissed_comment' in payload:
            validate_dismissed_comment(payload['dismissed_comment'])
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


def source_sha256(client, commit, path, cache):
    key = (commit, path)
    if key not in cache:
        content = client.request(f'repos/{REPOSITORY}/contents/{quote(path, safe="/")}?ref={commit}')
        if (not isinstance(content, dict) or content.get('type') != 'file'
                or content.get('encoding') != 'base64' or not isinstance(content.get('content'), str)):
            raise Blocked('Reviewed source is unavailable as a regular file')
        data = base64.b64decode(''.join(content['content'].split()), validate=True)
        cache[key] = hashlib.sha256(data).hexdigest()
    return cache[key]


def verify_and_apply(client, row, expected_head, apply, source_cache):
    validate_dismissed_comment(row['comment'])
    endpoint = f'repos/{REPOSITORY}/code-scanning/alerts/{row["number"]}'
    alert = client.request(endpoint)
    if not isinstance(alert, dict):
        raise Blocked(f'Unexpected metadata response for #{row["number"]}')
    rule, engine = alert.get('rule'), alert.get('tool')
    if (type(alert.get('number')) is not int or alert['number'] != row['number']
            or not isinstance(rule, dict) or rule.get('id') != row['rule']
            or not isinstance(engine, dict) or engine.get('name') != 'CodeQL'
            or alert.get('state') not in ['open', 'fixed', 'dismissed']):
        raise Blocked(f'Alert #{row["number"]}: number, state, tool or rule no longer matches the review')
    if alert['state'] in ['fixed', 'dismissed']:
        return {'number': row['number'], 'result': 'already closed'}
    all_instances = client.instances(row['number'])
    if any(not isinstance(item, dict) or item.get('state') not in ['open', 'fixed', 'dismissed']
           for item in all_instances):
        # A nullable or missing state is not evidence of inactivity. This main
        # batch requires explicitly known states; it does not guess PR-only state.
        raise Blocked(f'Alert #{row["number"]}: an instance has a missing or unknown state')
    instances = [item for item in all_instances if item['state'] == 'open']
    if not instances or not any(item.get('ref') == 'refs/heads/main' for item in instances):
        raise Blocked('No reviewed open main instance remains')
    for instance in instances:
        location = instance.get('location', {})
        ref = instance.get('ref')
        if not isinstance(ref, str) or not ref.startswith('refs/') or not isinstance(location, dict):
            raise Blocked(f'Missing ref or source location for #{row["number"]}')
        if any(location.get(key) != row['location'].get(key) for key in
               ['path', 'start_line', 'end_line', 'start_column', 'end_column']):
            raise Blocked('An open instance has a different location; review it separately')
        commit = instance.get('commit_sha', '')
        path = location.get('path', '')
        if (not isinstance(commit, str) or not re.fullmatch(r'[0-9a-f]{40}', commit)
                or not isinstance(path, str) or not path or path.startswith('/')
                or '\\' in path or '..' in path.split('/')):
            raise Blocked('Unsafe or missing source identity')
        if source_sha256(client, commit, path, source_cache) != row['source_sha256']:
            raise Blocked('An open instance uses changed source; review it separately')
    # Analysis may lag behind a push. A reviewed old instance alone cannot
    # authorize dismissing changed, not-yet-analyzed code on current main.
    if source_sha256(client, expected_head, row['location']['path'], source_cache) != row['source_sha256']:
        raise Blocked(f'Current main source changed for #{row["number"]}; review it separately')
    if client.head() != expected_head:
        raise Blocked('Main advanced during the run; restart source validation')
    if not apply:
        return {'number': row['number'], 'result': 'verified; no write', 'open_instances': len(instances)}
    response = client.request(endpoint, {'state': 'dismissed', 'dismissed_reason': row['reason'],
                                         'dismissed_comment': row['comment']})
    if not isinstance(response, dict) or response.get('state') != 'dismissed':
        raise UnconfirmedMainDismissal(row['number'], response)
    return {'number': row['number'], 'result': 'dismissed', 'reason': row['reason']}


def run_batch(client, rows, apply, report, checkpoint):
    report.update(phase='preflight', preflight=[], decisions=[])
    # Validate every API payload before any network request. A later oversized
    # comment must not produce a partially applied batch, even for direct callers.
    for row in rows:
        report['active_alert'] = row['number']
        validate_dismissed_comment(row['comment'])
    report.pop('active_alert', None)
    head = client.head()
    report['validated_main_head'] = head
    cache = {}
    for row in rows:
        report['active_alert'] = row['number']
        decision = verify_and_apply(client, row, head, False, cache)
        report['preflight'].append(decision)
        if not apply:
            report['decisions'].append(decision)
        checkpoint()
        print(f'Preflight #{row["number"]}: {decision["result"]}', flush=True)
    if client.head() != head:
        raise Blocked('Main advanced during preflight; no writes were performed')
    if apply:
        report['phase'] = 'apply'
        for row in rows:
            report['active_alert'] = row['number']
            # Cache only immutable source bytes. Re-read live alert metadata and
            # all instance states, and check HEAD immediately before each PATCH.
            decision = verify_and_apply(client, row, head, True, cache)
            report['decisions'].append(decision)
            checkpoint()
            print(f'#{row["number"]}: {decision["result"]}', flush=True)
    if client.head() != head:
        raise Blocked('Main advanced before final confirmation; inspect the saved decisions')
    report['phase'] = 'complete'
    report.pop('active_alert', None)


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
    def checkpoint():
        args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    code = 0
    try:
        plan = load_plan(Path(__file__).with_name('reviewed-codeql-alerts.json'))
        client = GitHub()
        run_batch(client, plan['alerts'], args.apply, report, checkpoint)
    except (Blocked, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report['blocked'] = str(error)
        if isinstance(error, UnconfirmedMainDismissal):
            report['unconfirmed_patch'] = error.details
        print('Stopped: ' + str(error), file=sys.stderr)
        code = 1
    checkpoint()
    return code


if __name__ == '__main__':
    sys.exit(main())
