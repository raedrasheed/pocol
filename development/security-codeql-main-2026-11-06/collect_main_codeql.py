#!/usr/bin/env python3
"""Collect current evidence for the 197 previously reviewed alerts; GET only.

Use the owner's existing gh session. This program cannot dismiss an alert and
does not request credentials. Reports contain metadata and source hashes, not
source text, tokens or signed download URLs. Changed rows are collected together
instead of stopping at the first source difference.
"""
import argparse
import base64
import binascii
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import quote

REVIEW = Path(__file__).resolve().parents[1] / 'security-evm-crypto-2026-11-05'
sys.path.insert(0, str(REVIEW))
from apply_reviewed_codeql import Blocked, GitHub, REPOSITORY, load_plan
from apply_pr29_codeql import load_plan as load_pr29_plan

MAIN = 'refs/heads/main'
FIELDS = ('path', 'start_line', 'end_line', 'start_column', 'end_column')
STATES = ('open', 'fixed', 'dismissed', None)
INSTANCE_FIELDS = ('ref', 'state', 'commit_sha', 'analysis_key', 'category', 'environment')


class ApiError(Blocked):
    def __init__(self, endpoint, status=None):
        self.endpoint, self.status = endpoint, status
        # Never forward gh stderr, authentication data or arbitrary response text.
        super().__init__(f'GitHub GET failed (HTTP {status or "unknown"}): {endpoint}')


class ReadOnlyGitHub(GitHub):
    def request(self, endpoint, payload=None):
        if payload is not None or not endpoint.startswith(f'repos/{REPOSITORY}/'):
            raise Blocked('Only read-only requests to the reviewed repository are allowed')
        command = ['gh', 'api', '--method', 'GET', '-H',
                   'Accept: application/vnd.github+json', endpoint]
        response = subprocess.run(command, capture_output=True, timeout=60)
        try:
            value = json.loads(response.stdout.decode('utf-8-sig'))
        except (ValueError, UnicodeError):
            raise ApiError(endpoint) from None
        if response.returncode:
            status = str(value.get('status', '')) if isinstance(value, dict) else ''
            raise ApiError(endpoint, int(status) if re.fullmatch(r'[1-5][0-9]{2}', status) else None)
        return value


def utc_now():
    return datetime.now(timezone.utc).isoformat()


def load_reviews():
    historical = load_plan(REVIEW / 'reviewed-codeql-alerts.json')
    variants = historical['alerts'] + load_pr29_plan()
    return historical, variants


def safe_location(value):
    if not isinstance(value, dict):
        return None
    return {field: value.get(field) for field in FIELDS}


def valid_source(instance):
    commit = instance.get('commit_sha')
    location = instance.get('location')
    if not isinstance(location, dict):
        return False
    path = location.get('path')
    return (isinstance(commit, str) and re.fullmatch(r'[0-9a-f]{40}', commit) is not None
            and isinstance(path, str) and bool(path) and not path.startswith('/')
            and '\\' not in path and not any(part in ('', '.', '..') for part in path.split('/'))
            and all(type(location.get(field)) is int and location[field] > 0
                    for field in FIELDS[1:])
            and location['end_line'] >= location['start_line'])


def source_hash(client, instance, cache):
    identity = (instance['commit_sha'], instance['location']['path'])
    if identity not in cache:
        commit, path = identity
        content = client.request(f'repos/{REPOSITORY}/contents/{quote(path, safe="/")}?ref={commit}')
        if (not isinstance(content, dict) or content.get('type') != 'file'
                or content.get('encoding') != 'base64' or not isinstance(content.get('content'), str)):
            raise Blocked('Immutable source is not an available base64 regular file')
        try:
            data = base64.b64decode(''.join(content['content'].split()), validate=True)
        except (ValueError, binascii.Error):
            raise Blocked('Invalid source encoding') from None
        cache[identity] = hashlib.sha256(data).hexdigest()
    return cache[identity]


def matches(row, instance, digest, variants):
    return any(variant['rule'] == row['rule'] and variant['reason'] == row['reason']
               and variant['source_sha256'] == digest
               and all(variant['location'].get(field) == instance['location'].get(field) for field in FIELDS)
               for variant in variants)


def collect_alert(client, row, variants, cache):
    number = row['number']
    result = {'number': number, 'expected_rule': row['rule'], 'approved_reason': row['reason'],
              'result': 'needs review', 'issues': [], 'instances': []}
    alert = client.request(f'repos/{REPOSITORY}/code-scanning/alerts/{number}')
    if not isinstance(alert, dict):
        raise Blocked('Invalid alert response')
    rule, engine = alert.get('rule'), alert.get('tool')
    result['observed'] = {'number': alert.get('number'), 'state': alert.get('state'),
                          'state_present': 'state' in alert,
                          'rule': rule.get('id') if isinstance(rule, dict) else None,
                          'tool': engine.get('name') if isinstance(engine, dict) else None}
    if (type(alert.get('number')) is not int or alert.get('number') != number
            or result['observed']['rule'] != row['rule']
            or result['observed']['tool'] != 'CodeQL'):
        result['issues'].append('alert number, rule or tool changed')
        return result
    if 'state' not in alert or alert['state'] not in STATES:
        result['issues'].append('missing or unknown global state')
    elif alert['state'] in ('fixed', 'dismissed'):
        result['result'] = 'already closed'
        return result
    instances = client.instances(number)
    for instance in instances:
        if not isinstance(instance, dict):
            result['issues'].append('invalid instance response')
            continue
        item = {field: instance.get(field) for field in INSTANCE_FIELDS}
        item['state_present'] = 'state' in instance
        item['location'] = safe_location(instance.get('location'))
        result['instances'].append(item)
        if 'state' not in instance or instance['state'] not in STATES:
            item['issue'] = 'missing or unknown instance state'
        elif instance['state'] in ('fixed', 'dismissed'):
            item['review'] = 'inactive; source not fetched'
            continue
        ref = instance.get('ref')
        if not isinstance(ref, str) or not ref.startswith('refs/') or any(ord(c) < 32 for c in ref):
            item['issue'] = 'missing or invalid instance ref'
        # Null is not proof of inactivity. Validate that source as well.
        if not valid_source(instance):
            item['issue'] = 'unsafe or missing immutable source identity'
        else:
            try:
                digest = source_hash(client, instance, cache)
                item['source_sha256'] = digest
                reviewed = matches(row, instance, digest, variants)
                item['review'] = 'matches pinned source review' if reviewed else 'unreviewed source or location'
                if not reviewed:
                    item['issue'] = item['review']
            except ApiError as error:
                if error.status != 404:
                    raise
                item['issue'] = 'immutable source unavailable (HTTP 404)'
            except Blocked as error:
                item['issue'] = str(error)
        if 'issue' in item:
            result['issues'].append({'instance': len(result['instances']) - 1, 'issue': item['issue']})
    target = [item for item in result['instances'] if item.get('ref') == MAIN]
    if not any(item.get('state') == 'open' for item in target):
        if target and all(item.get('state') in ('fixed', 'dismissed') for item in target):
            result['result'] = 'closed on main; other refs retained'
        else:
            result['issues'].append('no explicitly open main instance')
    elif not result['issues']:
        result['result'] = 'matches pinned review; no write'
    return result


def collect(client, plan, variants, report, checkpoint):
    report['main_at_start'] = client.head()
    cache = {}
    for row in plan['alerts']:
        try:
            result = collect_alert(client, row, variants, cache)
        except ApiError as error:
            if error.status != 404:
                raise
            result = {'number': row['number'], 'result': 'needs review',
                      'issues': ['reviewed alert unavailable (HTTP 404)']}
        report['alerts'].append(result)
        report['summary'] = dict(Counter(item['result'] for item in report['alerts']))
        checkpoint()
        print(f'#{row["number"]}: {result["result"]}', flush=True)
    report['main_at_end'] = client.head()
    report['stable_main_snapshot'] = report['main_at_start'] == report['main_at_end']
    report['unique_source_files_read'] = len(cache)
    report['phase'] = 'complete' if report['stable_main_snapshot'] else 'main advanced; collect again'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists() or not args.out.parent.is_dir():
        parser.error('--out must be a new file and its parent directory must exist')
    report = {'repository': REPOSITORY, 'read_only': True, 'scope': '197 previously reviewed alert IDs',
              'started_at': utc_now(), 'phase': 'collecting', 'alerts': []}
    def checkpoint():
        args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    # Exclusive creation preserves previous reports even when two invocations race.
    try:
        with args.out.open('x', encoding='utf-8') as output:
            output.write(json.dumps(report) + '\n')
    except OSError:
        parser.error('Unable to create the new report file')
    code = 0
    try:
        plan, variants = load_reviews()
        report['review_source_commit'] = plan['review_source_commit']
        collect(ReadOnlyGitHub(), plan, variants, report, checkpoint)
        code = 0 if report.get('stable_main_snapshot') else 2
    except (Blocked, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report['phase'] = 'stopped'
        # Exception strings from the API transport are deliberately sanitized.
        report['blocked'] = str(error) if isinstance(error, Blocked) else type(error).__name__
        if isinstance(error, ApiError):
            report['http_status'] = error.status
        print('Stopped: ' + report['blocked'], file=sys.stderr)
        code = 1
    report['finished_at'] = utc_now()
    checkpoint()
    return code


if __name__ == '__main__':
    sys.exit(main())
