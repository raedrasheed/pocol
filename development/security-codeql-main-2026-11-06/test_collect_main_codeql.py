"""Offline regressions for the owner evidence collector; no live alert writes."""
import base64
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import collect_main_codeql as tool

HEAD = 'a' * 40
CONTENT = b'public test counter\n'
LOCATION = {'path': 'tests/public.rs', 'start_line': 4, 'end_line': 4,
            'start_column': 2, 'end_column': 8}
ROW = {'number': 12, 'rule': 'rust/hard-coded-cryptographic-value', 'location': LOCATION,
       'reason': 'false positive', 'comment': 'Reviewed public account counter.',
       'source_sha256': hashlib.sha256(CONTENT).hexdigest()}


class Fake:
    def __init__(self):
        self.alert = {'number': ROW['number'], 'state': 'open', 'rule': {'id': ROW['rule']},
                      'tool': {'name': 'CodeQL'}}
        self.rows = [{'state': 'open', 'ref': tool.MAIN, 'commit_sha': HEAD,
                      'location': copy.deepcopy(LOCATION)}]
        self.content, self.calls, self.heads = CONTENT, [], [HEAD, HEAD]

    def request(self, endpoint, payload=None):
        if payload is not None:
            raise AssertionError('Collector attempted a write')
        self.calls.append(endpoint)
        if '/contents/' in endpoint:
            return {'type': 'file', 'encoding': 'base64',
                    'content': base64.b64encode(self.content).decode()}
        return copy.deepcopy(self.alert)

    def instances(self, number):
        return copy.deepcopy(self.rows)

    def head(self):
        return self.heads.pop(0)


class CollectorTests(unittest.TestCase):
    def collect(self, client, variants=None):
        return tool.collect_alert(client, ROW, variants or [ROW], {})

    def test_exact_review_records_hash_without_source_text(self):
        result = self.collect(Fake())
        self.assertEqual(result['result'], 'matches pinned review; no write')
        self.assertEqual(result['instances'][0]['source_sha256'], ROW['source_sha256'])
        self.assertNotIn(CONTENT.decode().strip(), json.dumps(result))

    def test_null_global_state_requires_explicit_open_main(self):
        client = Fake(); client.alert['state'] = None
        self.assertEqual(self.collect(client)['result'], 'matches pinned review; no write')
        client.rows[0]['state'] = None
        self.assertIn('no explicitly open main instance', self.collect(client)['issues'])

    def test_nullable_other_ref_is_source_checked(self):
        client = Fake(); other = copy.deepcopy(client.rows[0])
        other.update(ref='refs/pull/8/head', state=None)
        other['location']['start_line'] += 10; other['location']['end_line'] += 10
        client.rows.append(other)
        result = self.collect(client)
        self.assertEqual(result['result'], 'needs review')
        self.assertEqual(result['instances'][1]['review'], 'unreviewed source or location')

    def test_all_changed_locations_are_retained(self):
        client = Fake(); client.rows.append(copy.deepcopy(client.rows[0]))
        for offset, row in enumerate(client.rows):
            row['location']['start_line'] += offset + 10
            row['location']['end_line'] += offset + 10
        self.assertEqual(len(self.collect(client)['issues']), 2)

    def test_changed_source_is_not_silently_accepted(self):
        client = Fake(); client.content = b'different public source'
        self.assertEqual(self.collect(client)['result'], 'needs review')

    def test_pinned_independently_reviewed_variant_is_recognized(self):
        client = Fake(); variant = copy.deepcopy(ROW)
        variant['location'].update(start_line=5, end_line=5)
        client.rows[0]['location'].update(start_line=5, end_line=5)
        self.assertEqual(self.collect(client, [ROW, variant])['result'], 'matches pinned review; no write')
        variant['reason'] = 'used in tests'
        self.assertEqual(self.collect(client, [ROW, variant])['result'], 'needs review')

    def test_global_metadata_checked_even_when_closed(self):
        client = Fake(); client.alert.update(state='dismissed', number=13)
        self.assertEqual(self.collect(client)['result'], 'needs review')

    def test_malformed_tool_and_rule_metadata_is_collected_as_a_difference(self):
        client = Fake(); client.alert['tool'] = 'unexpected'
        self.assertEqual(self.collect(client)['result'], 'needs review')
        client = Fake(); client.alert['rule'] = []
        self.assertEqual(self.collect(client)['result'], 'needs review')

    def test_missing_and_unknown_states_are_not_closure(self):
        for state in ['missing', 'unexpected']:
            client = Fake()
            if state == 'missing': del client.alert['state']
            else: client.alert['state'] = state
            self.assertIn('missing or unknown global state', self.collect(client)['issues'])
        client = Fake(); del client.rows[0]['state']
        self.assertEqual(self.collect(client)['instances'][0]['issue'], 'missing or unknown instance state')

    def test_closed_alert_does_not_fetch_source(self):
        client = Fake(); client.alert['state'] = 'fixed'
        self.assertEqual(self.collect(client)['result'], 'already closed')
        self.assertEqual(len(client.calls), 1)

    def test_no_open_main_does_not_label_other_refs_eligible(self):
        client = Fake(); client.rows[0]['ref'] = 'refs/pull/8/head'
        self.assertEqual(self.collect(client)['result'], 'needs review')
        closed = copy.deepcopy(client.rows[0]); closed.update(ref=tool.MAIN, state='fixed')
        client.rows.append(closed)
        self.assertEqual(self.collect(client)['result'], 'closed on main; other refs retained')

    def test_unsafe_source_paths_are_never_requested(self):
        for path in ['../private', '/absolute', 'tests/../x', 'C:\\x', 'tests//x', '']:
            client = Fake(); client.rows[0]['location']['path'] = path
            self.assertEqual(self.collect(client)['result'], 'needs review')
            self.assertFalse(any('/contents/' in endpoint for endpoint in client.calls))

    def test_source_cache_uses_commit_and_path(self):
        client = Fake(); other = copy.deepcopy(client.rows[0]); other['ref'] = 'refs/pull/8/head'
        client.rows.append(other); self.collect(client)
        self.assertEqual(sum('/contents/' in endpoint for endpoint in client.calls), 1)

    def test_batch_collects_differences_and_reports_moving_main(self):
        client = Fake(); client.content = b'changed'; client.heads[-1] = 'b' * 40
        report = {'alerts': []}; checkpoints = []
        tool.collect(client, {'alerts': [ROW, ROW]}, [ROW], report,
                     lambda: checkpoints.append(len(report['alerts'])))
        self.assertEqual(checkpoints, [1, 2])
        self.assertEqual(report['summary'], {'needs review': 2})
        self.assertFalse(report['stable_main_snapshot'])

    def test_first_permission_error_stops_without_retries(self):
        client = Fake(); client.request = lambda endpoint: (_ for _ in ()).throw(tool.ApiError(endpoint, 403))
        report = {'alerts': []}
        with self.assertRaises(tool.ApiError):
            tool.collect(client, {'alerts': [ROW, ROW]}, [ROW], report, lambda: None)
        self.assertEqual(report['alerts'], [])

    def test_transport_has_no_mutating_method_and_sanitizes_failures(self):
        response = subprocess.CompletedProcess([], 1,
            stdout=b'{"status":"403","message":"secret signed URL"}', stderr=b'credential material')
        with patch.object(tool.subprocess, 'run', return_value=response) as run:
            with self.assertRaises(tool.ApiError) as caught:
                tool.ReadOnlyGitHub().request('repos/palcertcom/pocol/code-scanning/alerts/12')
            self.assertEqual(caught.exception.status, 403)
            self.assertNotIn('secret', str(caught.exception))
            self.assertNotIn('credential', str(caught.exception))
            command = run.call_args.args[0]
            self.assertEqual(command[command.index('--method') + 1], 'GET')
            with self.assertRaises(tool.Blocked):
                tool.ReadOnlyGitHub().request('repos/palcertcom/pocol/code-scanning/alerts/12', {})
            self.assertEqual(run.call_count, 1)

    def test_cli_preserves_existing_report_and_rejects_apply(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'evidence.json'; output.write_text('keep me')
            command = [sys.executable, str(Path(tool.__file__)), '--out', str(output)]
            result = subprocess.run(command, capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(output.read_text(), 'keep me')
            output.unlink()
            result = subprocess.run(command + ['--apply'], capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertFalse(output.exists())

    def test_repository_reviews_are_pinned_and_current_files_match_190_rows(self):
        plan, variants = tool.load_reviews()
        self.assertEqual(len(plan['alerts']), 197)
        self.assertEqual(len(variants), 208)
        root = Path(tool.__file__).resolve().parents[2]
        identical = [row for row in plan['alerts'] if hashlib.sha256(
            (root / row['location']['path']).read_bytes()).hexdigest() == row['source_sha256']]
        self.assertEqual(len(identical), 190)


if __name__ == '__main__':
    unittest.main()
