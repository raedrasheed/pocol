"""Safety regressions for the PR29 owner helper; no network or real API writes."""
import base64
import copy
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlsplit

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


class DynamicHeadFake(Fake):
    """Model the observed service ref without deriving it from the helper."""
    def __init__(self):
        super().__init__()
        evidence = Path(__file__).with_name('evidence') / 'codeql-pr29-run-ref.json'
        self.service_ref = json.loads(evidence.read_text())['head_branch']
        self.rows[0]['ref'] = self.service_ref

    def request(self, endpoint, payload=None):
        if '/alerts?' in endpoint:
            requested = parse_qs(urlsplit(endpoint).query).get('ref')
            if requested != [self.service_ref]:
                return []
        return super().request(endpoint, payload)


class ReviewedOlderSourceFake(DynamicHeadFake):
    """Current PR source plus the actual independently reviewed older file."""
    def __init__(self):
        super().__init__()
        folder = Path(__file__).resolve().parent
        evidence = json.loads((folder / 'evidence/pr29-reviewed-older-source.json').read_text())
        self.reviewed = tool.load_plan()[0]
        self.alert['rule']['id'] = self.reviewed['rule']
        self.rows[0]['location'] = copy.deepcopy(self.reviewed['location'])
        self.older_commit = evidence['source_commit']
        self.rows.append({'state': 'open', 'ref': 'refs/heads/main',
                          'commit_sha': self.older_commit,
                          'location': {**self.reviewed['location'], 'start_line': 51, 'end_line': 51}})
        self.older_content = evidence['content'].encode()
        self.content = (folder.parents[1] / self.reviewed['location']['path']).read_bytes()

    def request(self, endpoint, payload=None):
        if '/contents/' in endpoint and payload is None:
            query = parse_qs(urlsplit(endpoint).query)
            data = self.older_content if query.get('ref') == [self.older_commit] else self.content
            return {'type': 'file', 'encoding': 'base64',
                    'content': base64.b64encode(data).decode()}
        return super().request(endpoint, payload)


class BatchFake(Fake):
    def __init__(self):
        super().__init__()
        second = copy.deepcopy(ROW)
        second['location']['start_line'] = second['location']['end_line'] = 9
        self.reviewed = [copy.deepcopy(ROW), second]
        self.alerts = {}
        self.all_instances = {}
        self.events = []
        for number, row in zip([401, 402], self.reviewed):
            alert = copy.deepcopy(self.alert)
            alert['number'] = number
            alert['most_recent_instance']['location'] = copy.deepcopy(row['location'])
            self.alerts[number] = alert
            self.all_instances[number] = [{**self.rows[0], 'location': copy.deepcopy(row['location'])}]
        self.listed = list(self.alerts.values())

    def request(self, endpoint, payload=None):
        if '/alerts/' in endpoint:
            number = int(endpoint.rsplit('/', 1)[-1])
            if payload is None:
                return self.alerts[number]
            self.events.append(('write', number))
        return super().request(endpoint, payload)

    def instances(self, number):
        self.events.append(('read_instances', number))
        return self.all_instances[number]


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

    def test_discovery_uses_the_observed_dynamic_head_ref(self):
        client = DynamicHeadFake()
        self.assertEqual(client.service_ref, 'refs/pull/29/head')
        self.assertEqual(tool.discover(client, [ROW]),
                         {tool.key(ROW['rule'], LOCATION): 401})
        self.assertEqual(client.writes, [])

    def test_verification_accepts_the_observed_dynamic_head_instance(self):
        client = DynamicHeadFake()
        self.assertEqual(tool.verify(client, ROW, 401, (HEAD, PR_HEAD), False, {})['result'],
                         'verified; no write')
        self.assertEqual(client.writes, [])

    def test_merge_ref_alone_does_not_match_the_reviewed_head_analysis(self):
        client = DynamicHeadFake()
        client.rows[0]['ref'] = 'refs/pull/29/merge'
        self.blocked(client)

    def test_reviewed_older_main_location_is_accepted_with_its_full_source_hash(self):
        client = ReviewedOlderSourceFake()
        result = tool.verify(client, client.reviewed, 401, (HEAD, PR_HEAD), False, {})
        self.assertEqual(result['result'], 'verified; no write')
        self.assertEqual(client.writes, [])

    def test_current_and_reviewed_older_source_allow_only_the_reviewed_payload(self):
        client = ReviewedOlderSourceFake()
        tool.verify(client, client.reviewed, 401, (HEAD, PR_HEAD), True, {})
        self.assertEqual(client.writes, [{'state': 'dismissed',
                                         'dismissed_reason': client.reviewed['reason'],
                                         'dismissed_comment': client.reviewed['comment']}])

    def test_changed_older_source_is_rejected_despite_a_reviewed_location(self):
        client = ReviewedOlderSourceFake(); client.older_content += b'changed'
        with self.assertRaises(tool.Blocked):
            tool.verify(client, client.reviewed, 401, (HEAD, PR_HEAD), True, {})
        self.assertEqual(client.writes, [])

    def test_unreviewed_older_location_is_rejected_despite_a_known_file_hash(self):
        client = ReviewedOlderSourceFake()
        client.rows[-1]['location']['start_line'] += 1
        client.rows[-1]['location']['end_line'] += 1
        with self.assertRaises(tool.Blocked):
            tool.verify(client, client.reviewed, 401, (HEAD, PR_HEAD), True, {})
        self.assertEqual(client.writes, [])

    def test_pr_target_must_match_its_own_reviewed_location(self):
        client = ReviewedOlderSourceFake()
        client.rows[0]['location'] = copy.deepcopy(client.rows[-1]['location'])
        client.rows[0]['commit_sha'] = client.older_commit
        with self.assertRaises(tool.Blocked):
            tool.verify(client, client.reviewed, 401, (HEAD, PR_HEAD), True, {})
        self.assertEqual(client.writes, [])

    def run_batch(self, client, apply):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'result.json'
            argv = ['apply_pr29_codeql.py', '--out', str(output)] + (['--apply'] if apply else [])
            with patch.object(tool, 'GitHub', return_value=client), \
                    patch.object(tool, 'load_plan', return_value=client.reviewed), \
                    patch('sys.argv', argv), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                code = tool.main()
            return code, json.loads(output.read_text())

    def test_full_batch_preflight_blocks_all_writes_if_a_later_alert_is_unreviewed(self):
        client = BatchFake()
        client.all_instances[402][0]['location']['start_line'] += 1
        code, report = self.run_batch(client, True)
        self.assertEqual(code, 1)
        self.assertEqual(client.writes, [])
        self.assertEqual(report['phase'], 'preflight')
        self.assertEqual(report['decisions'], [])
        self.assertEqual(len(report['preflight']), 1)
        self.assertEqual(report['blocked_instance']['number'], 402)
        self.assertEqual(report['blocked_instance']['ref'], tool.REF)
        self.assertEqual(report['blocked_instance']['source_sha256'], ROW['source_sha256'])

    def test_apply_revalidates_after_all_alerts_pass_preflight(self):
        client = BatchFake()
        code, report = self.run_batch(client, True)
        self.assertEqual(code, 0)
        self.assertEqual(report['phase'], 'complete')
        self.assertEqual(len(report['preflight']), 2)
        self.assertEqual(len(report['decisions']), 2)
        self.assertEqual(len(client.writes), 2)
        first_write = client.events.index(('write', 401))
        self.assertIn(('read_instances', 402), client.events[:first_write])
        self.assertEqual(client.events.count(('read_instances', 401)), 2)
        self.assertEqual(client.events.count(('read_instances', 402)), 2)

    def test_read_only_batch_never_enters_apply_phase(self):
        client = BatchFake()
        code, report = self.run_batch(client, False)
        self.assertEqual(code, 0)
        self.assertEqual(client.writes, [])
        self.assertEqual(report['phase'], 'complete')
        self.assertEqual(len(report['decisions']), 2)
        self.assertTrue(all(row['result'] == 'verified; no write' for row in report['decisions']))

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
