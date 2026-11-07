#!/usr/bin/env python3
"""Verify the 11 reviewed PR29 findings; --apply uses the owner's normal gh session.

No alert IDs are guessed. Discover only exact reviewed rule/location matches on
PR29, then verify every open instance and immutable source across all refs.
The older main-branch batch and its guard remain unchanged.
"""
import argparse
import base64
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import quote

from apply_reviewed_codeql import (Blocked, GitHub, REPOSITORY,
                                   load_plan as load_historical_plan, validate_dismissed_comment)

PLAN_SHA256 = '2caff84219b7b409206b1c1adc43b9ec33f95e4385ce77ececddceb47c41e33c'
# GitHub's dynamic default-setup run analyzes this PR head, not its merge ref.
# Evidence: evidence/codeql-pr29-run-ref.json (run 42125256600).
REF = 'refs/pull/29/head'
FIELDS = ['path', 'start_line', 'end_line', 'start_column', 'end_column']


class InstanceBlocked(Blocked):
    def __init__(self, message, number, instance, source_sha256=None):
        self.details = {'number': number, 'ref': instance.get('ref'),
                        'commit_sha': instance.get('commit_sha'),
                        'location': instance.get('location'), 'state': instance.get('state'),
                        'source_sha256': source_sha256}
        super().__init__(f'{message}: alert #{number}, ref {instance.get("ref")}, '
                         f'location {json.dumps(instance.get("location"), sort_keys=True)}')


class AlertBlocked(Blocked):
    def __init__(self, number, row, alert):
        self.details = {'number': number,
                        'expected': {'number': number, 'rule': row['rule'], 'tool': 'CodeQL'},
                        'observed': {'number': alert.get('number'), 'state': alert.get('state'),
                                     'state_present': 'state' in alert,
                                     'rule': alert.get('rule', {}).get('id'),
                                     'tool': alert.get('tool', {}).get('name')}}
        super().__init__(f'Alert metadata changed for #{number}: '
                         f'{json.dumps(self.details["observed"], sort_keys=True)}')


class DismissalUnconfirmed(Blocked):
    def __init__(self, number, response, instances):
        self.details = {'number': number, 'patch_sent': True,
                        'response_state': response.get('state'),
                        'response_state_present': 'state' in response,
                        'observed_instances': [{field: instance.get(field) for field in
                                                ['ref', 'state', 'commit_sha', 'location']}
                                               for instance in instances]}
        super().__init__(f'PATCH sent for #{number}, but GitHub did not confirm dismissal '
                         'of all previously active instances; inspect unconfirmed_patch')


def instance_identity(instance):
    # Preserve the exact verified analysis/source snapshot when the global
    # response state is null and confirmation must come from instance states.
    fields = ['ref', 'analysis_key', 'environment', 'category', 'commit_sha']
    value = {field: instance.get(field) for field in fields}
    value['location'] = {field: (instance.get('location') or {}).get(field) for field in FIELDS}
    return json.dumps(value, sort_keys=True)


@lru_cache(maxsize=1)
def historical_rows():
    # The previous review has its own fixed SHA256 and scope validation.
    # Its data is evidence for exact older source versions, not an ID allowlist.
    path = Path(__file__).with_name('reviewed-codeql-alerts.json')
    return tuple(load_historical_plan(path)['alerts'])


def reviewed_variants(row, instance):
    # The PR target itself must match its exact current review. Other refs can
    # have an older, independently reviewed location in a known source version.
    if instance.get('ref') == REF:
        return [row]
    return [row] + [old for old in historical_rows()
                    if old['rule'] == row['rule'] and old['reason'] == row['reason']
                    and old['location']['path'] == row['location']['path']]


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
            or any(r['reason'] != 'false positive' for r in rows)):
        raise Blocked('Unexpected reviewed PR29 scope')
    for row in rows:
        validate_dismissed_comment(row['comment'])
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
        raise Blocked(f'Not all {len(wanted)} reviewed locations were found on {REF} '
                      f'({len(found)} matched); inspect the current PR findings')
    return found


def verify(client, row, number, expected_heads, apply, cache):
    validate_dismissed_comment(row['comment'])
    endpoint = f'repos/{REPOSITORY}/code-scanning/alerts/{number}'
    alert = client.request(endpoint)
    if ('state' not in alert or alert['state'] not in ['open', 'fixed', 'dismissed', None]
            or alert.get('number') != number or alert.get('rule', {}).get('id') != row['rule']
            or alert.get('tool', {}).get('name') != 'CodeQL'):
        raise AlertBlocked(number, row, alert)
    if alert['state'] in ['fixed', 'dismissed']:
        return {'number': number, 'result': 'already closed'}
    all_instances = client.instances(number)
    for instance in all_instances:
        if instance.get('state') not in ['open', 'fixed', 'dismissed', None]:
            raise InstanceBlocked('Unknown instance state', number, instance)
    target = [instance for instance in all_instances if instance.get('ref') == REF]
    if alert['state'] is None and target and all(r.get('state') in ['fixed', 'dismissed'] for r in target):
        return {'number': number, 'result': 'already closed on reviewed PR'}
    # Null is allowed by GitHub's schema; it does not itself establish openness.
    if not any(r.get('state') == 'open' for r in target):
        raise Blocked(f'No explicitly open reviewed PR29 instance remains for #{number}')
    # A nullable state on another ref is not evidence that it is inactive:
    # validate its source too before any global dismissal.
    instances = [r for r in all_instances if r.get('state') not in ['fixed', 'dismissed']]
    for instance in instances:
        location = instance.get('location') or {}
        commit, path = instance.get('commit_sha', ''), location.get('path', '')
        if not re.fullmatch(r'[0-9a-f]{40}', commit) or not path or path.startswith('/') or '..' in path.split('/'):
            raise InstanceBlocked('Unsafe or missing immutable source identity', number, instance)
        if path != row['location']['path']:
            raise InstanceBlocked('Unreviewed source path on an open ref', number, instance)
        identity = (commit, path)
        if identity not in cache:
            content = client.request(f'repos/{REPOSITORY}/contents/{quote(path, safe="/")}?ref={commit}')
            if content.get('type') != 'file' or content.get('encoding') != 'base64':
                raise Blocked('Reviewed source is not an available regular file')
            cache[identity] = hashlib.sha256(base64.b64decode(content['content'])).hexdigest()
        candidates = [variant for variant in reviewed_variants(row, instance)
                      if all(location.get(field) == variant['location'].get(field) for field in FIELDS)]
        if not candidates:
            raise InstanceBlocked('Unreviewed location on an open ref', number, instance, cache[identity])
        if not any(cache[identity] == variant['source_sha256'] for variant in candidates):
            raise InstanceBlocked('Unreviewed source version on an open ref', number, instance, cache[identity])
    if heads(client) != expected_heads:
        raise Blocked('Main or the PR advanced during the batch; restart validation')
    if not apply:
        return {'number': number, 'result': 'verified; no write'}
    required_instances = {instance_identity(instance) for instance in instances}
    response = client.request(endpoint, {'state': 'dismissed', 'dismissed_reason': row['reason'],
                                         'dismissed_comment': row['comment']})
    if response.get('state') != 'dismissed':
        after = client.instances(number) if 'state' in response and response['state'] is None else []
        dismissed = {instance_identity(instance) for instance in after if instance.get('state') == 'dismissed'}
        if (not required_instances <= dismissed
                or any(instance.get('state') not in ['dismissed', 'fixed'] for instance in after)):
            raise DismissalUnconfirmed(number, response, after)
    return {'number': number, 'result': 'dismissed', 'reason': row['reason']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists() or not args.out.parent.is_dir():
        parser.error('--out must be new and its parent directory must exist')
    report = {'repository': REPOSITORY, 'pull_request': 29, 'apply': args.apply,
              'phase': 'preflight', 'preflight': [], 'decisions': []}
    code = 0
    try:
        rows, client = load_plan(), GitHub()
        historical_rows()
        expected = heads(client)
        report['validated_main_and_pr_heads'] = list(expected)
        matches, cache = discover(client, rows), {}
        for row in rows:
            decision = verify(client, row, matches[key(row['rule'], row['location'])], expected, False, cache)
            report['preflight'].append(decision)
            if not args.apply:
                report['decisions'].append(decision)
            args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
            print(f'Preflight #{decision["number"]}: {decision["result"]}', flush=True)
        if args.apply:
            report['phase'] = 'apply'
            for row in rows:
                # Re-read each live instance and the heads immediately before
                # its PATCH; only immutable source-file hashes are cached.
                decision = verify(client, row, matches[key(row['rule'], row['location'])], expected, True, cache)
                report['decisions'].append(decision)
                args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
                print(f'#{decision["number"]}: {decision["result"]}', flush=True)
        report['phase'] = 'complete'
    except (Blocked, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report['blocked'] = str(error)
        if isinstance(error, InstanceBlocked):
            report['blocked_instance'] = error.details
        elif isinstance(error, AlertBlocked):
            report['blocked_alert'] = error.details
        elif isinstance(error, DismissalUnconfirmed):
            report['unconfirmed_patch'] = error.details
        print('Stopped: ' + str(error), file=sys.stderr)
        code = 1
    args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    return code


if __name__ == '__main__':
    sys.exit(main())
