"""Offline regressions for the 19-ID scope and final open-main inventory."""
import base64
import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, unquote

import apply_remaining_codeql as tool

ROOT = Path(__file__).resolve().parents[2]
HEAD = 'c' * 40


def inventory_row(number):
    return {'number': number, 'state': 'open', 'rule': {'id': 'other/example'},
            'tool': {'name': 'Other scanner'}, 'most_recent_instance': {
                'ref': tool.MAIN, 'state': 'open', 'commit_sha': HEAD,
                'location': {'path': 'example.rs', 'start_line': 1, 'end_line': 1}}}


class Fake:
    def __init__(self):
        self.plan = tool.load_plan()
        self.rows = self.plan['alerts']
        self.alerts = {r['number']: {'number': r['number'], 'state': 'open',
                                  'rule': {'id': r['rule']}, 'tool': {'name': 'CodeQL'}} for r in self.rows}
        self.instances_by_id = {r['number']: [dict(ref=tool.MAIN, state='open',
                                  commit_sha=HEAD, location=copy.deepcopy(r['location']))] for r in self.rows}
        self.writes, self.calls, self.extra = [], [], []
        self.current_head = HEAD
        self.inventory_pages = None
        self.advance_on_inventory = False
        self.bad_current_source = False

    def head(self): return self.current_head
    def instances(self, number): return copy.deepcopy(self.instances_by_id[number])

    def request(self, endpoint, payload=None):
        self.calls.append(endpoint)
        if '/contents/' in endpoint:
            self.assert_read(payload)
            path = unquote(endpoint.split('/contents/', 1)[1].split('?ref=', 1)[0])
            data = (ROOT / path).read_bytes()
            if self.bad_current_source: data += b'changed current main'
            return {'type': 'file', 'encoding': 'base64', 'content': base64.b64encode(data).decode()}
        if '/code-scanning/alerts?' in endpoint:
            self.assert_read(payload)
            query = parse_qs(endpoint.split('?', 1)[1])
            assert query['ref'] == [tool.MAIN] and query['state'] == ['open']
            assert 'tool_name' not in query
            if self.advance_on_inventory: self.current_head = 'd' * 40
            if self.inventory_pages is not None:
                return self.inventory_pages[int(query['page'][0]) - 1]
            return [dict(copy.deepcopy(a), most_recent_instance=copy.deepcopy(self.instances_by_id[n][0]))
                    for n, a in self.alerts.items() if a['state'] == 'open'] + copy.deepcopy(self.extra)
        number = int(endpoint.rsplit('/', 1)[1])
        if payload is not None:
            self.writes.append((number, payload))
            self.alerts[number]['state'] = 'dismissed'
            for instance in self.instances_by_id[number]: instance['state'] = 'dismissed'
        return copy.deepcopy(self.alerts[number])

    @staticmethod
    def assert_read(payload):
        assert payload is None, 'Unexpected mutation outside a reviewed alert endpoint'


class RemainingTests(unittest.TestCase):
    def execute(self, client, apply=True):
        self.report = {'repository': tool.REPOSITORY, 'apply': apply}
        with contextlib.redirect_stdout(io.StringIO()):
            tool.execute(client, client.rows, apply, self.report, lambda: None)

    def test_review_plan_is_pinned_and_separate_from_the_197_id_plan(self):
        value = tool.load_plan()
        self.assertEqual(len(value['alerts']), 19)
        self.assertEqual({row['number'] for row in value['alerts']}, tool.EXPECTED_IDS)
        sys.path.insert(0, str(tool.REVIEW))
        from apply_reviewed_codeql import load_plan
        historical = load_plan(tool.REVIEW / 'reviewed-codeql-alerts.json')
        self.assertEqual(len(historical['alerts']), 197)
        self.assertTrue(tool.EXPECTED_IDS.isdisjoint({row['number'] for row in historical['alerts']}))

    def test_all_nine_source_files_and_exact_literals_match_the_review(self):
        rows = tool.load_plan()['alerts']
        paths = set()
        for row in rows:
            path = ROOT / row['location']['path']; paths.add(path)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), row['source_sha256'])
            line = path.read_text(encoding='utf-8').splitlines()[row['location']['start_line'] - 1]
            self.assertEqual(line, row['source_line'])
            self.assertEqual(line[row['location']['start_column'] - 1:row['location']['end_column'] - 1],
                             row['reviewed_literal'])
        self.assertEqual(len(paths), 9)
        self.assertTrue(any('examples' in path.parts for path in paths))

    def test_changed_plan_cannot_expand_mutation_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'plan.json'; path.write_text('{}')
            with self.assertRaises(tool.Blocked): tool.load_plan(path)

    def test_dry_run_keeps_every_alert_open_and_counts_the_inventory(self):
        client = Fake(); self.execute(client, apply=False)
        self.assertEqual(client.writes, [])
        self.assertFalse(self.report['application_complete'])
        self.assertEqual(self.report['remaining_open_main_count'], 19)
        self.assertFalse(self.report['main_clear_at_postcheck'])

    def test_apply_reaches_only_the_19_ids_then_proves_empty_main_inventory(self):
        client = Fake(); self.execute(client)
        self.assertEqual({number for number, _ in client.writes}, tool.EXPECTED_IDS)
        self.assertEqual(len(client.writes), 19)
        self.assertTrue(all(payload['dismissed_reason'] == 'false positive' for _, payload in client.writes))
        self.assertEqual(self.report['phase'], 'complete')
        self.assertTrue(self.report['application_complete'])
        self.assertTrue(self.report['postcheck_stable_main'])
        self.assertEqual(self.report['remaining_open_main_count'], 0)
        self.assertTrue(self.report['main_clear_at_postcheck'])

    def test_unrelated_open_finding_stays_visible_and_is_never_dismissed(self):
        client = Fake(); client.extra = [inventory_row(999)]
        self.execute(client)
        self.assertEqual(self.report['remaining_open_main_count'], 1)
        self.assertEqual(self.report['remaining_open_main'][0]['number'], 999)
        self.assertFalse(self.report['main_clear_at_postcheck'])
        self.assertNotIn(999, {number for number, _ in client.writes})

    def test_late_unreviewed_instance_blocks_the_entire_batch_before_writes(self):
        client = Fake(); last = max(tool.EXPECTED_IDS)
        client.instances_by_id[last].append({'ref': 'refs/pull/99/head', 'state': 'open',
            'commit_sha': HEAD, 'location': {**client.rows[-1]['location'], 'start_line': 1}})
        with self.assertRaises(tool.Blocked): self.execute(client)
        self.assertEqual(client.writes, [])
        self.assertEqual(self.report['phase'], 'preflight')

    def test_unknown_instance_state_blocks_all_writes(self):
        client = Fake(); client.instances_by_id[max(tool.EXPECTED_IDS)][0]['state'] = None
        with self.assertRaises(tool.Blocked): self.execute(client)
        self.assertEqual(client.writes, [])

    def test_unreviewed_current_main_source_blocks_all_writes(self):
        client = Fake(); client.bad_current_source = True
        with self.assertRaises(tool.Blocked): self.execute(client)
        self.assertEqual(client.writes, [])

    def test_repeated_application_only_skips_the_closed_reviewed_ids(self):
        client = Fake()
        for alert in client.alerts.values(): alert['state'] = 'dismissed'
        self.execute(client)
        self.assertEqual(client.writes, [])
        self.assertTrue(all(row['result'] == 'already closed' for row in self.report['decisions']))
        self.assertTrue(self.report['main_clear_at_postcheck'])

    def test_inventory_failure_preserves_confirmed_application_without_claiming_zero(self):
        client = Fake(); client.inventory_pages = [{'message': 'not a list'}]
        with self.assertRaises(tool.Blocked): self.execute(client)
        self.assertEqual(len(client.writes), 19)
        self.assertTrue(self.report['application_complete'])
        self.assertEqual(self.report['phase'], 'postcheck')
        self.assertNotIn('remaining_open_main_count', self.report)

    def test_main_advancement_prevents_a_clean_snapshot_claim(self):
        client = Fake(); client.advance_on_inventory = True
        with self.assertRaises(tool.Blocked): self.execute(client)
        self.assertTrue(self.report['application_complete'])
        self.assertFalse(self.report['postcheck_stable_main'])
        self.assertNotIn('remaining_open_main_count', self.report)
        self.assertNotIn('main_clear_at_postcheck', self.report)

    def test_inventory_paginates_all_tools_and_keeps_every_returned_id(self):
        client = Fake(); client.inventory_pages = [[inventory_row(n) for n in range(1000, 1100)], [inventory_row(1100)]]
        rows = tool.open_main_inventory(client)
        self.assertEqual(len(rows), 101)
        self.assertEqual(rows[-1]['number'], 1100)
        self.assertEqual(client.writes, [])

    def test_duplicate_inventory_ids_cannot_establish_a_complete_snapshot(self):
        client = Fake(); client.inventory_pages = [[inventory_row(n) for n in range(1000, 1100)], [inventory_row(1000)]]
        with self.assertRaises(tool.Blocked): tool.open_main_inventory(client)

    def test_malformed_or_wrong_ref_inventory_is_not_silently_filtered(self):
        for item in [None, inventory_row(True), {**inventory_row(999), 'most_recent_instance': {}}]:
            client = Fake(); client.inventory_pages = [[item]]
            with self.assertRaises(tool.Blocked): tool.open_main_inventory(client)

    def test_cli_keeps_an_existing_report_intact(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'report.json'; output.write_text('keep original')
            result = subprocess.run([sys.executable, str(Path(tool.__file__)), '--apply', '--out', str(output)],
                                    capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(output.read_text(), 'keep original')

    def test_cli_records_final_inventory_in_the_single_application_report(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'report.json'
            with patch.object(sys, 'argv', ['apply_remaining_codeql.py', '--apply', '--out', str(output)]), \
                 patch.object(tool, 'GitHub', return_value=Fake()), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(tool.main(), 0)
            value = json.loads(output.read_text(encoding='utf-8'))
            self.assertEqual(len(value['decisions']), 19)
            self.assertEqual(value['remaining_open_main_count'], 0)
            self.assertEqual(value['phase'], 'complete')


if __name__ == '__main__':
    unittest.main()
