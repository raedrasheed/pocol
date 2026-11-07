"""Mock API tests: no network and no actual alert state writes."""
import base64
import copy
import hashlib
import contextlib
import io
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
        self.alert = {'number': ROW['number'], 'state': 'open', 'rule': {'id': ROW['rule']}, 'tool': {'name': 'CodeQL'}}
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
    def test_comment_limit_matches_observed_github_contract(self):
        self.assertEqual(tool.MAX_DISMISSED_COMMENT_CHARS, 280)

    def test_exact_280_character_comment_is_accepted(self):
        client = Fake(); row = {**ROW, 'comment': 'x' * 280}
        self.assertEqual(tool.verify_and_apply(client, row, HEAD, True, {})['result'], 'dismissed')
        self.assertEqual(client.writes[0]['dismissed_comment'], 'x' * 280)

    def test_281_character_comment_is_rejected_before_metadata_requests(self):
        client = Fake(); row = {**ROW, 'comment': 'x' * 281}
        with patch.object(client, 'request', wraps=client.request) as request:
            with self.assertRaisesRegex(tool.Blocked, '280'):
                tool.verify_and_apply(client, row, HEAD, True, {})
            request.assert_not_called()
        self.assertEqual(client.writes, [])

    def test_transport_blocks_oversized_comment_without_invoking_gh(self):
        with patch.object(tool.subprocess, 'run') as request:
            with self.assertRaisesRegex(tool.Blocked, '280'):
                tool.GitHub().request('repos/palcertcom/pocol/code-scanning/alerts/237',
                                      {'state': 'dismissed', 'dismissed_comment': 'x' * 281})
            request.assert_not_called()

    def test_transport_preserves_a_280_character_unicode_comment_without_truncation(self):
        comment = '\u0645' * 280
        response = type('Response', (), {'returncode': 0, 'stdout': b'{"state":"dismissed"}'})()
        with patch.object(tool.subprocess, 'run', return_value=response) as request:
            tool.GitHub().request('repos/palcertcom/pocol/code-scanning/alerts/237',
                                  {'state': 'dismissed', 'dismissed_comment': comment})
            payload = json.loads(request.call_args.kwargs['input'].decode('utf-8'))
            self.assertEqual(payload['dismissed_comment'], comment)

    def test_wrong_returned_number_blocks_even_a_closed_alert(self):
        client = Fake(); client.alert.update(number=13, state='dismissed'); self.blocked(client)

    def test_unknown_or_nullable_global_state_blocks(self):
        for state in [None, 'unexpected']:
            client = Fake(); client.alert['state'] = state; self.blocked(client)

    def test_nullable_or_missing_state_on_other_ref_cannot_be_ignored(self):
        for missing in [True, False]:
            client = Fake(); other = copy.deepcopy(client.rows[0]); other['ref'] = 'refs/pull/9/head'
            if missing: del other['state']
            else: other['state'] = None
            client.rows.append(other); self.blocked(client)

    def test_missing_ref_blocks_before_write(self):
        client = Fake(); other = copy.deepcopy(client.rows[0]); del other['ref']
        client.rows.append(other); self.blocked(client)

    def test_changed_current_main_blocks_when_analysis_still_uses_reviewed_old_commit(self):
        class LaggingAnalysis(Fake):
            def request(self, endpoint, payload=None):
                if '/contents/' in endpoint and endpoint.endswith('ref=' + HEAD):
                    return {'type': 'file', 'encoding': 'base64',
                            'content': base64.b64encode(b'unreviewed current main').decode()}
                return super().request(endpoint, payload)
        client = LaggingAnalysis(); client.rows[0]['commit_sha'] = 'b' * 40
        self.blocked(client)

    def test_unconfirmed_patch_is_identified_as_a_write_that_needs_inspection(self):
        class Unconfirmed(Fake):
            def request(self, endpoint, payload=None):
                if payload is not None:
                    self.writes.append(payload)
                    return {'state': None}
                return super().request(endpoint, payload)
        client = Unconfirmed()
        with self.assertRaises(tool.UnconfirmedMainDismissal) as error:
            tool.verify_and_apply(client, ROW, HEAD, True, {})
        self.assertTrue(error.exception.details['patch_sent'])
        self.assertEqual(error.exception.details['number'], ROW['number'])
        self.assertEqual(len(client.writes), 1)

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


class BatchFake(Fake):
    def __init__(self):
        super().__init__()
        self.alerts = {number: {**copy.deepcopy(self.alert), 'number': number} for number in [12, 13]}
        self.instances_by_number = {number: copy.deepcopy(self.rows) for number in self.alerts}
        self.write_numbers = []

    def request(self, endpoint, payload=None):
        if '/code-scanning/alerts/' in endpoint:
            number = int(endpoint.rsplit('/', 1)[1])
            if payload is not None: self.write_numbers.append(number)
            else: return self.alerts[number]
        return super().request(endpoint, payload)

    def instances(self, number):
        return self.instances_by_number[number]


class BatchTests(unittest.TestCase):
    def run_batch(self, client, apply=True, checkpoint=None, rows=None):
        rows = rows or [copy.deepcopy(ROW), {**copy.deepcopy(ROW), 'number': 13}]
        self.report = {}
        with contextlib.redirect_stdout(io.StringIO()):
            tool.run_batch(client, rows, apply, self.report, checkpoint or (lambda: None))

    def test_later_oversized_comment_blocks_all_network_and_writes(self):
        client = BatchFake(); rows = [copy.deepcopy(ROW), {**copy.deepcopy(ROW), 'number': 13, 'comment': 'x' * 281}]
        with patch.object(client, 'request', wraps=client.request) as request, \
             patch.object(client, 'head', wraps=client.head) as head:
            with self.assertRaisesRegex(tool.Blocked, '280'): self.run_batch(client, rows=rows)
            request.assert_not_called(); head.assert_not_called()
        self.assertEqual(client.writes, [])
        self.assertEqual(self.report['phase'], 'preflight')
        self.assertEqual(self.report['active_alert'], 13)
        self.assertEqual(self.report['preflight'], [])

    def test_later_preflight_failure_prevents_every_write(self):
        client = BatchFake(); client.instances_by_number[13][0]['location']['start_line'] += 1
        with self.assertRaises(tool.Blocked): self.run_batch(client)
        self.assertEqual(client.writes, [])
        self.assertEqual(self.report['phase'], 'preflight')
        self.assertEqual(self.report['active_alert'], 13)
        self.assertEqual(len(self.report['preflight']), 1)

    def test_only_a_complete_preflight_can_start_dismissals(self):
        client = BatchFake(); observed = []
        self.run_batch(client, checkpoint=lambda: observed.append((len(self.report['preflight']), len(client.writes))))
        self.assertEqual(observed[:2], [(1, 0), (2, 0)])
        self.assertEqual(client.write_numbers, [12, 13])
        self.assertEqual(self.report['phase'], 'complete')

    def test_dry_run_keeps_the_complete_batch_read_only(self):
        client = BatchFake(); self.run_batch(client, apply=False)
        self.assertEqual(client.writes, [])
        self.assertEqual(len(self.report['decisions']), 2)
        self.assertEqual(self.report['phase'], 'complete')

    def test_main_change_at_preflight_barrier_prevents_all_writes(self):
        client = BatchFake()
        def checkpoint():
            if len(self.report['preflight']) == 2: client.current_head = 'b' * 40
        with self.assertRaises(tool.Blocked): self.run_batch(client, checkpoint=checkpoint)
        self.assertEqual(client.writes, [])

    def test_live_metadata_is_rechecked_after_preflight(self):
        client = BatchFake()
        def checkpoint():
            if len(client.writes) == 1: client.alerts[13]['rule']['id'] = 'changed/rule'
        with self.assertRaises(tool.Blocked): self.run_batch(client, checkpoint=checkpoint)
        self.assertEqual(client.write_numbers, [12])
        self.assertEqual(self.report['phase'], 'apply')
        self.assertEqual(self.report['active_alert'], 13)


if __name__ == '__main__':
    unittest.main()
