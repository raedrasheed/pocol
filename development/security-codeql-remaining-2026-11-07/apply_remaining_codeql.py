#!/usr/bin/env python3
"""Verify/apply only the independently reviewed 19 remaining CodeQL findings.

Default is read-only. Reuse the full-batch and current-source guards of the
historical helper without changing its 197-ID plan. Afterward, collect a compact
read-only inventory of every open finding on main, including findings outside
this plan. Use only the operator's existing gh authentication.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

REVIEW = Path(__file__).resolve().parents[1] / 'security-evm-crypto-2026-11-05'
sys.path.insert(0, str(REVIEW))
from apply_reviewed_codeql import (Blocked, GitHub, REPOSITORY, UnconfirmedMainDismissal,
                                   run_batch, validate_dismissed_comment)

PLAN_SHA256 = '9badea929b55e5185d243361671969465443e9ba294529c4913cd09f9280c46f'
EXPECTED_IDS = {236, 237, 238, 239, 240, 241, 242, 244, 249, 250, 251, 252,
                253, 254, 255, 256, 257, 258, 259}
MAIN = 'refs/heads/main'


def load_plan(path=None):
    path = path or Path(__file__).with_name('reviewed-remaining-19.json')
    text = path.read_text(encoding='utf-8-sig').replace('\r\n', '\n')
    if hashlib.sha256(text.encode('utf-8')).hexdigest() != PLAN_SHA256:
        raise Blocked('The independently reviewed remaining-19 plan changed')
    value = json.loads(text)
    rows = value['alerts']
    if (value['repository'] != REPOSITORY or len(rows) != 19
            or {row['number'] for row in rows} != EXPECTED_IDS
            or any(type(row['number']) is not int
                   or row['rule'] != 'rust/hard-coded-cryptographic-value'
                   or row['reason'] != 'false positive' for row in rows)):
        raise Blocked('Unexpected remaining-19 review scope or decision')
    for row in rows:
        validate_dismissed_comment(row['comment'])
    return value


def open_main_inventory(client):
    result, seen = [], set()
    for page in range(1, 1001):
        endpoint = (f'repos/{REPOSITORY}/code-scanning/alerts?state=open&ref={MAIN}'
                    f'&per_page=100&page={page}')
        rows = client.request(endpoint)
        if not isinstance(rows, list):
            raise Blocked('Unexpected open-main inventory response; inventory is unconfirmed')
        for alert in rows:
            if not isinstance(alert, dict):
                raise Blocked('Invalid inventory row; inventory is unconfirmed')
            number, rule, tool = alert.get('number'), alert.get('rule'), alert.get('tool')
            instance = alert.get('most_recent_instance')
            if (type(number) is not int or number <= 0 or number in seen
                    or not isinstance(rule, dict) or not isinstance(rule.get('id'), str)
                    or not isinstance(tool, dict) or not isinstance(tool.get('name'), str)
                    or not isinstance(instance, dict) or instance.get('ref') != MAIN
                    or instance.get('state') != 'open'):
                raise Blocked('Inconsistent or duplicate open-main row; inventory is unconfirmed')
            seen.add(number)
            # Do not filter out plan IDs, test classifications or other tools.
            # Any returned open finding must remain visible to the reviewer.
            result.append({'number': number, 'rule': rule['id'], 'tool': tool['name'],
                           'global_state': alert.get('state'),
                           'instance': {key: instance.get(key) for key in
                                        ['ref', 'state', 'commit_sha', 'location']}})
        if len(rows) < 100:
            return result
    raise Blocked('Too many inventory pages; no complete inventory was established')


def execute(client, rows, apply, report, checkpoint):
    run_batch(client, rows, apply, report, checkpoint)
    report['application_complete'] = apply
    report['phase'] = 'postcheck'
    checkpoint()
    inventory = open_main_inventory(client)
    report['remaining_open_main'] = inventory
    final_head = client.head()
    report['main_at_postcheck'] = final_head
    report['postcheck_stable_main'] = final_head == report['validated_main_head']
    if not report['postcheck_stable_main']:
        raise Blocked('Main advanced during the final inventory; inspect the confirmed decisions')
    report['remaining_open_main_count'] = len(inventory)
    report['phase'] = 'complete'
    report['main_clear_at_postcheck'] = len(inventory) == 0
    checkpoint()
    print(f'Open main findings after read-only inventory: {len(inventory)}', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists() or not args.out.parent.is_dir():
        parser.error('--out must be a new file and its parent directory must exist')
    report = {'repository': REPOSITORY, 'scope': '19 independently reviewed remaining findings',
              'apply': args.apply, 'phase': 'preflight', 'preflight': [], 'decisions': []}
    try:
        with args.out.open('x', encoding='utf-8') as output:
            output.write(json.dumps(report) + '\n')
    except OSError:
        parser.error('Unable to create the fresh report file')
    def checkpoint():
        args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    code = 0
    try:
        plan = load_plan()
        report['review_source_commit'] = plan['review_source_commit']
        report['review_plan_sha256'] = PLAN_SHA256
        execute(GitHub(), plan['alerts'], args.apply, report, checkpoint)
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
