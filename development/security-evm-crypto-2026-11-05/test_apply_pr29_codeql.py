"""Safety regressions for the PR29 owner helper; no network or real API writes."""
import base64
import copy
import hashlib
from pathlib import Path
import unittest

import apply_pr29_codeql as tool

HEAD, PR_HEAD = 'a' * 40, 'b' * 40
CONTENT = b'public synthetic EVM counter\n'
LOCATION = {'path': 'tests/public.rs', 'start_line': 4, 'end_line': 4,
            'start_column': 2, 'end_column': 8}
ROW = {'rule': 'rust/hard-coded-cryptographic-value', 'location': LOCATION,
       'source_sha256': hashlib.sha256(CONTENT).hexdigest(),
       'reason': 'false positive', 'comment': 'Reviewed public EVM account counter.'}


class Fake:
    def __init__(self):
        self.pr = {'state': 'open', 'base': {'ref': 'main'},
                   'head': {'sha': PR_HEAD, 'ref': 'security/evm-crypto-2026-11-05',
                            'repo': {'full_name': tool.REPOSITORY}}}
        self.main = HEAD
        self.alert = {'number': 401, 'state': 'open', 'rule': {'id': ROW['rule']},
                      'tool': {'name': 'CodeQL'},
                      'most_recent_instance': {'location': copy.deepcopy(LOCATION)}}
        self.rows = [{'state': 'open', 'ref': tool.REF, 'commit_sha': PR_HEAD,
                      'location': copy.deepcopy(LOCATION)}]
        self.content, self.writes = CONTENT, []
        self.listed = [self.alert]

    def request(self, endpoint, payload=None):
        if payload is not None:
            self.writes.append(payload)
            return {'state': 'dismissed'}
        if '/pulls/29' in endpoint:
            return self.pr
        if '/contents/' in endpoint:
            return {'type': 'file', 'encoding': 'base64',
                    'content': base64.b64encode(self.content).decode()}
        if '/alerts?' in endpoint:
            return self.listed if '&state=open&' in endpoint else []
        return self.alert

    def head(self):
        return self.main

    def instances(self, number):
        return self.rows


class ApplyPRTests(unittest.TestCase):
    def blocked(self, client, expected=(HEAD, PR_HEAD)):
        with self.assertRaises(tool.Blocked):
            tool.verify(client, ROW, 401, expected, True, {})
        self.assertEqual(client.writes, [])

    def test_read_only_never_writes(self):
        client = Fake()
        self.assertEqual(tool.verify(client, ROW, 401, (HEAD, PR_HEAD), False, {})['result'],
                         'verified; no write')
        self.assertEqual(client.writes, [])

    def test_explicit_apply_payload_is_limited_to_reviewed_decision(self):
        client = Fake()
        tool.verify(client, ROW, 401, (HEAD, PR_HEAD), True, {})
        self.assertEqual(client.writes, [{'state': 'dismissed',
                                         'dismissed_reason': 'false positive',
                                         'dismissed_comment': ROW['comment']}])

    def test_main_movement_blocks_write(self):
        client = Fake(); client.main = 'c' * 40; self.blocked(client)

    def test_pr_movement_blocks_write(self):
        client = Fake(); client.pr['head']['sha'] = 'c' * 40; self.blocked(client)

    def test_wrong_source_repository_blocks_write(self):
        client = Fake(); client.pr['head']['repo']['full_name'] = 'other/repo'; self.blocked(client)

    def test_other_ref_with_changed_source_blocks_write(self):
        client = Fake()
        other = copy.deepcopy(client.rows[0]); other['ref'] = 'refs/heads/other'
        other['location']['start_line'] += 1; client.rows.append(other)
        self.blocked(client)

    def test_source_content_change_blocks_write(self):
        client = Fake(); client.content += b'changed'; self.blocked(client)

    def test_missing_pr_instance_blocks_write(self):
        client = Fake(); client.rows[0]['ref'] = 'refs/heads/main'; self.blocked(client)

    def test_missing_commit_blocks_write(self):
        client = Fake(); client.rows[0]['commit_sha'] = ''; self.blocked(client)

    def test_changed_rule_blocks_write(self):
        client = Fake(); client.alert['rule']['id'] = 'different/rule'; self.blocked(client)

    def test_closed_alert_is_skipped(self):
        client = Fake(); client.alert['state'] = 'dismissed'
        self.assertEqual(tool.verify(client, ROW, 401, (HEAD, PR_HEAD), True, {})['result'],
                         'already closed')
        self.assertEqual(client.writes, [])

    def test_discovery_requires_exact_unambiguous_matches(self):
        client = Fake()
        self.assertEqual(tool.discover(client, [ROW]), {tool.key(ROW['rule'], LOCATION): 401})
        duplicate = copy.deepcopy(client.alert); duplicate['number'] = 402
        client.listed.append(duplicate)
        with self.assertRaises(tool.Blocked): tool.discover(client, [ROW])
        self.assertEqual(client.writes, [])

    def test_missing_reviewed_match_stops_discovery(self):
        client = Fake(); client.listed = []
        with self.assertRaises(tool.Blocked): tool.discover(client, [ROW])
        self.assertEqual(client.writes, [])

    def test_pinned_review_matches_all_11_actual_source_locations(self):
        rows = tool.load_plan()
        root = Path(__file__).resolve().parents[2]
        self.assertEqual(len(rows), 11)
        for row in rows:
            source = root / row['location']['path']
            self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), row['source_sha256'])
            self.assertEqual(source.read_text().splitlines()[row['location']['start_line'] - 1],
                             row['source_line'])


if __name__ == '__main__':
    unittest.main()
