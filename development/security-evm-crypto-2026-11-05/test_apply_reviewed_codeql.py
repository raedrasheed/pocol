"""Mock API tests: no network and no actual alert state writes."""
import base64
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import apply_reviewed_codeql as tool

HEAD = 'a' * 40
CONTENT = b'public synthetic test source\n'
LOCATION = {'path': 'tests/public.rs', 'start_line': 4, 'end_line': 4,
            'start_column': 2, 'end_column': 8}
ROW = {'number': 12, 'rule': 'rust/hard-coded-cryptographic-value', 'location': LOCATION,
       'reason': 'false positive', 'comment': 'Reviewed public protocol nonce.',
       'source_sha256': hashlib.sha256(CONTENT).hexdigest()}


class Fake:
    def __init__(self):
        self.alert = {'state': 'open', 'rule': {'id': ROW['rule']}, 'tool': {'name': 'CodeQL'}}
        self.rows = [{'state': 'open', 'ref': 'refs/heads/main', 'commit_sha': HEAD,
                      'location': copy.deepcopy(LOCATION)}]
        self.content = CONTENT
        self.current_head = HEAD
        self.writes = []

    def request(self, endpoint, payload=None):
        if payload is not None:
            self.writes.append(payload)
            return {'state': 'dismissed'}
        if '/contents/' in endpoint:
            return {'type': 'file', 'encoding': 'base64',
                    'content': base64.b64encode(self.content).decode()}
        return self.alert

    def instances(self, number):
        return self.rows

    def head(self):
        return self.current_head


class ApplyTests(unittest.TestCase):
    def test_dry_run_never_writes(self):
        client = Fake()
        self.assertEqual(tool.verify_and_apply(client, ROW, HEAD, False, {})['result'], 'verified; no write')
        self.assertEqual(client.writes, [])

    def test_explicit_apply_uses_only_the_reviewed_fields(self):
        client = Fake()
        self.assertEqual(tool.verify_and_apply(client, ROW, HEAD, True, {})['result'], 'dismissed')
        self.assertEqual(client.writes, [{'state': 'dismissed', 'dismissed_reason': ROW['reason'],
                                          'dismissed_comment': ROW['comment']}])

    def blocked(self, client):
        with self.assertRaises(tool.Blocked):
            tool.verify_and_apply(client, ROW, HEAD, True, {})
        self.assertEqual(client.writes, [])

    def test_changed_rule_blocks_write(self):
        client = Fake(); client.alert['rule']['id'] = 'different/rule'; self.blocked(client)

    def test_changed_location_blocks_write(self):
        client = Fake(); client.rows[0]['location']['start_line'] += 1; self.blocked(client)

    def test_changed_source_blocks_write(self):
        client = Fake(); client.content = b'new source'; self.blocked(client)

    def test_every_open_ref_must_match(self):
        client = Fake(); other = copy.deepcopy(client.rows[0]); other['ref'] = 'refs/pull/9/merge'
        other['location']['path'] = 'product/private.rs'; client.rows.append(other); self.blocked(client)

    def test_no_main_instance_blocks_write(self):
        client = Fake(); client.rows[0]['ref'] = 'refs/heads/other'; self.blocked(client)

    def test_missing_commit_blocks_write(self):
        client = Fake(); del client.rows[0]['commit_sha']; self.blocked(client)

    def test_concurrent_main_change_blocks_write(self):
        client = Fake(); client.current_head = 'b' * 40; self.blocked(client)

    def test_closed_alert_is_idempotent(self):
        client = Fake(); client.alert['state'] = 'fixed'
        self.assertEqual(tool.verify_and_apply(client, ROW, HEAD, True, {})['result'], 'already closed')
        self.assertEqual(client.writes, [])

    def test_plan_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'plan.json'; path.write_text('{}')
            with self.assertRaises(tool.Blocked): tool.load_plan(path)

    def test_repository_plan_hash_and_scope_are_valid(self):
        plan = tool.load_plan(Path(__file__).with_name('reviewed-codeql-alerts.json'))
        self.assertEqual(plan['counts'], {'false positive': 193, 'used in tests': 4})

    def test_denied_api_is_reported_without_writes_or_credentials(self):
        response = type('Response', (), {'returncode': 1,
            'stdout': json.dumps({'status': '403', 'message': 'Resource not accessible by integration'}).encode()})()
        with patch.object(tool.subprocess, 'run', return_value=response) as run:
            with self.assertRaisesRegex(tool.Blocked, '403 Resource not accessible'):
                tool.GitHub().request('repos/palcertcom/pocol/code-scanning/alerts/12')
            self.assertNotIn('PATCH', run.call_args.args[0])


if __name__ == '__main__':
    unittest.main()
