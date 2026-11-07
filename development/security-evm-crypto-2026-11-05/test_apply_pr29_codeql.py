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


class CommentContractTests(unittest.TestCase):
    def test_pr29_rejects_oversized_comment_before_network(self):
        client = Fake(); row = {**copy.deepcopy(ROW), 'comment': 'x' * 281}
        with patch.object(client, 'request', wraps=client.request) as request:
            with self.assertRaisesRegex(tool.Blocked, '280'):
                tool.verify(client, row, 401, (HEAD, PR_HEAD), True, {})
            request.assert_not_called()
        self.assertEqual(client.writes, [])


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


class ObservedNullStateFake(DynamicHeadFake):
    """Use the owner's actual #243 metadata/instance; PATCH replies are modeled."""
    def __init__(self):
        super().__init__()
        folder = Path(__file__).resolve().parent
        fixture = json.loads((folder / 'evidence/codeql-owner-null-state-observation.json').read_text())
        self.reviewed = tool.load_plan()[4]
        self.alert = copy.deepcopy(fixture['alert_243'])
        self.alert['rule'] = {'id': self.reviewed['rule']}
        self.alert['tool'] = {'name': 'CodeQL'}
        self.rows = copy.deepcopy(fixture['instances_243'])
        self.pr['head']['sha'] = self.rows[0]['commit_sha']
        self.content = (folder.parents[1] / self.reviewed['location']['path']).read_bytes()
        self.response_state = None
        self.confirm_after_patch = True
        self.instance_reads = 0

    def instances(self, number):
        self.instance_reads += 1
        return copy.deepcopy(self.rows)

    def request(self, endpoint, payload=None):
        if payload is not None:
            self.writes.append(payload)
            if self.confirm_after_patch:
                for instance in self.rows:
                    instance['state'] = 'dismissed'
            return {'state': self.response_state}
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


class ObservedBatchFake(BatchFake):
    """Replay actual owner IDs/global states; model other branch instances.

    #243's PR instance is captured. Other target locations and the older main
    source are modeled from the two pinned source reviews, not claimed captured.
    """
    def __init__(self):
        super().__init__()
        folder = Path(__file__).resolve().parent
        captured = json.loads((folder / 'evidence/codeql-owner-null-state-observation.json').read_text())
        older = json.loads((folder / 'evidence/pr29-reviewed-older-source.json').read_text())
        self.reviewed = tool.load_plan()
        self.main = older['source_commit']
        self.pr['head']['sha'] = captured['instances_243'][0]['commit_sha']
        history = {row['number']: row for row in tool.historical_rows()}
        self.alerts, self.all_instances, self.sources = {}, {}, {}
        self.unconfirmed_number = None
        for row, metadata in zip(self.reviewed, captured['alert_metadata']):
            number = metadata['requested']
            assert metadata['actual_rule'] == row['rule']
            instance = {**copy.deepcopy(captured['instances_243'][0]),
                        'location': copy.deepcopy(row['location'])}
            self.all_instances[number] = [instance]
            self.sources[(self.pr['head']['sha'], row['location']['path'])] = (
                folder.parents[1] / row['location']['path']).read_bytes()
            if number in history and history[number]['location']['path'] == row['location']['path']:
                self.all_instances[number].append({**copy.deepcopy(instance), 'ref': 'refs/heads/main',
                    'commit_sha': self.main, 'location': copy.deepcopy(history[number]['location'])})
                self.sources[(self.main, older['path'])] = older['content'].encode()
            self.alerts[number] = {'number': number, 'state': metadata['state'],
                'rule': {'id': metadata['actual_rule']}, 'tool': {'name': metadata['tool']},
                'most_recent_instance': instance}

    def request(self, endpoint, payload=None):
        if '/alerts?' in endpoint:
            state = parse_qs(urlsplit(endpoint).query)['state'][0]
            return [alert for number, alert in self.alerts.items()
                    if self.all_instances[number][0]['state'] == state]
        if '/alerts/' in endpoint:
            number = int(endpoint.rsplit('/', 1)[-1])
            if payload is None:
                return self.alerts[number]
            self.writes.append(payload); self.events.append(('write', number))
            if number != self.unconfirmed_number:
                for instance in self.all_instances[number]:
                    instance['state'] = 'dismissed'
                if self.alerts[number]['state'] is not None:
                    self.alerts[number]['state'] = 'dismissed'
            return {'state': self.alerts[number]['state']}
        if '/contents/' in endpoint:
            url = urlsplit(endpoint)
            path = url.path.split('/contents/', 1)[1]
            content = self.sources[(parse_qs(url.query)['ref'][0], path)]
            return {'type': 'file', 'encoding': 'base64',
                    'content': base64.b64encode(content).decode()}
        return Fake.request(self, endpoint, payload)

    def instances(self, number):
        return copy.deepcopy(super().instances(number))


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

    def test_actual_null_global_state_uses_explicit_open_pr_instance(self):
        client = ObservedNullStateFake()
        result = tool.verify(client, client.reviewed, 243, tool.heads(client), False, {})
        self.assertEqual(result['result'], 'verified; no write')
        self.assertEqual(client.writes, [])

    def test_null_patch_reply_needs_fresh_confirmation_of_all_dismissed_instances(self):
        client = ObservedNullStateFake()
        result = tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
        self.assertEqual(result['result'], 'dismissed')
        self.assertEqual(len(client.writes), 1)
        self.assertEqual(client.instance_reads, 2)

    def test_null_global_with_closed_pr_instance_is_idempotent(self):
        client = ObservedNullStateFake(); client.rows[0]['state'] = 'dismissed'
        result = tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
        self.assertEqual(result['result'], 'already closed on reviewed PR')
        self.assertEqual(client.writes, [])

    def test_null_global_does_not_make_an_unknown_pr_instance_open(self):
        client = ObservedNullStateFake(); client.rows[0]['state'] = None
        with self.assertRaises(tool.Blocked):
            tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
        self.assertEqual(client.writes, [])

    def test_null_state_on_another_ref_does_not_skip_source_validation(self):
        client = ObservedNullStateFake()
        other = copy.deepcopy(client.rows[0]); other['ref'] = 'refs/heads/other'
        other['state'] = None; other['location']['start_line'] += 1
        client.rows.append(other)
        with self.assertRaises(tool.Blocked):
            tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
        self.assertEqual(client.writes, [])

    def test_null_patch_reply_with_still_open_instances_is_not_reported_closed(self):
        client = ObservedNullStateFake(); client.confirm_after_patch = False
        with self.assertRaises(tool.Blocked):
            tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
        self.assertEqual(len(client.writes), 1)

    def test_null_patch_reply_cannot_be_confirmed_by_missing_instances(self):
        for incomplete in ['missing_instance', 'missing_location']:
            with self.subTest(incomplete=incomplete):
                client = ObservedNullStateFake()
                original = client.instances
                def read(number):
                    result = original(number)
                    if client.instance_reads > 1:
                        if incomplete == 'missing_instance':
                            return []
                        result[0]['location'] = None
                    return result
                client.instances = read
                with self.assertRaises(tool.DismissalUnconfirmed):
                    tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
                self.assertEqual(len(client.writes), 1)

    def test_unknown_global_state_remains_blocked_with_metadata_details(self):
        client = ObservedNullStateFake(); client.alert['state'] = 'pending'
        with self.assertRaises(tool.AlertBlocked) as error:
            tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
        self.assertEqual(error.exception.details['observed']['state'], 'pending')
        self.assertEqual(error.exception.details['number'], 243)
        self.assertEqual(client.writes, [])

    def test_absent_state_is_not_the_same_as_an_explicit_null_state(self):
        client = ObservedNullStateFake(); del client.alert['state']
        with self.assertRaises(tool.AlertBlocked) as error:
            tool.verify(client, client.reviewed, 243, tool.heads(client), True, {})
        self.assertFalse(error.exception.details['observed']['state_present'])
        self.assertEqual(client.writes, [])

    def test_all_11_actual_metadata_rows_pass_preflight_and_apply_in_the_service_model(self):
        client = ObservedBatchFake()
        code, report = self.run_batch(client, True)
        self.assertEqual(code, 0)
        self.assertEqual(report['phase'], 'complete')
        self.assertEqual(len(report['preflight']), 11)
        self.assertEqual(len(report['decisions']), 11)
        self.assertEqual(len(client.writes), 11)
        self.assertEqual([r['number'] for r in report['decisions']],
                         [47, 48, 49, 52, 243, 245, 58, 246, 247, 248, 260])
        first_write = next(i for i, event in enumerate(client.events) if event[0] == 'write')
        self.assertIn(('read_instances', 260), client.events[:first_write])

    def test_all_11_are_idempotent_even_when_six_global_states_remain_null(self):
        client = ObservedBatchFake()
        self.assertEqual(self.run_batch(client, True)[0], 0)
        code, report = self.run_batch(client, True)
        self.assertEqual(code, 0)
        self.assertEqual(len(client.writes), 11)
        self.assertTrue(all(row['result'].startswith('already closed') for row in report['decisions']))

    def test_unconfirmed_patch_is_recorded_as_sent_and_stops_later_writes(self):
        client = ObservedBatchFake(); client.unconfirmed_number = 243
        code, report = self.run_batch(client, True)
        self.assertEqual(code, 1)
        self.assertEqual(report['phase'], 'apply')
        self.assertEqual(len(report['preflight']), 11)
        self.assertEqual(len(report['decisions']), 4)
        self.assertEqual(len(client.writes), 5)
        self.assertEqual(report['unconfirmed_patch']['number'], 243)
        self.assertTrue(report['unconfirmed_patch']['patch_sent'])

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
