"""Offline Windows/Linux controls for three-seed read-only collection."""
import contextlib
import copy
import io
import json
from pathlib import Path
import subprocess
import shlex
import tempfile
import unittest
from unittest.mock import patch

import collect_seeds as tool


def sample(index, height=200, common=None):
    return {'schema': 'pocol-mainnet-preflight-v1', 'readOnly': True,
            'deploymentPerformed': False, 'activationHeightSelected': False, 'result': 'PASS',
            'node': {'chainId': tool.CHAIN_ID, 'genesisHash': tool.GENESIS,
                     'compatibilitySha256': 'a' * 64, 'forkSchedule': [[1, '0'], [2, '15000']],
                     'head': {'height': str(height)},
                     'binding': {'node': f'seed-{index + 1}', 'miner': '0x' + f'{index + 1:040x}'}},
            'comparisonBlock': {'height': str(height if common is None else common),
                                'hash': '0x' + '1' * 64, 'stateRoot': '0x' + '2' * 64,
                                'receiptsRoot': '0x' + '3' * 64}}


class Fake:
    def __init__(self):
        self.calls = []
        self.mutate = lambda value, number: None

    def collect(self, host, height=None):
        index = tool.HOSTS.index(host)
        number = len(self.calls)
        self.calls.append((host, height))
        value = sample(index, height=200 + index + (10 if number >= 6 else 0), common=height)
        self.mutate(value, number)
        return value


class CollectionTests(unittest.TestCase):
    def run_collection(self, client):
        self.report = {}
        self.checkpoints = []
        self.waited = []
        with contextlib.redirect_stdout(io.StringIO()):
            tool.execute(client, self.report, lambda: self.checkpoints.append(copy.deepcopy(self.report)),
                         pause=self.waited.append, wait_seconds=15)

    def test_three_rounds_compare_common_roots_and_confirm_every_head_advances(self):
        client = Fake(); self.run_collection(client)
        self.assertEqual(len(client.calls), 9)
        self.assertEqual(client.calls[3:6], [(host, 140) for host in tool.HOSTS])
        self.assertEqual(self.waited, [15])
        self.assertEqual(self.report['phase'], 'complete')
        self.assertEqual(self.report['result'], 'PASS')
        self.assertTrue(self.report['all_three_heads_advanced'])

    def test_failed_seed_stops_before_common_round_and_preserves_reports(self):
        client = Fake()
        client.mutate = lambda value, number: value.update(result='FAIL') if number == 2 else None
        with self.assertRaises(tool.Blocked): self.run_collection(client)
        self.assertEqual(len(client.calls), 3)
        self.assertEqual(len(self.report['rounds']['initial']), 3)
        self.assertNotIn('common_block', self.report)

    def test_different_common_block_or_either_root_is_refused(self):
        for field in ['hash', 'stateRoot', 'receiptsRoot']:
            with self.subTest(field=field):
                client = Fake()
                client.mutate = lambda value, number: value['comparisonBlock'].update({field: '0x' + 'f' * 64}) if number == 4 else None
                with self.assertRaises(tool.Blocked): self.run_collection(client)
                self.assertEqual(len(client.calls), 6)

    def test_wrong_network_or_different_source_or_schedule_is_refused(self):
        for field, change in [('chainId', 7), ('genesisHash', '0x' + 'f' * 64),
                              ('compatibilitySha256', 'f' * 64), ('forkSchedule', [[1, '0']])]:
            with self.subTest(field=field):
                client = Fake()
                client.mutate = lambda value, number: value['node'].update({field: change}) if number == 1 else None
                with self.assertRaises(tool.Blocked): self.run_collection(client)
                self.assertEqual(len(client.calls), 3)

    def test_standalone_shared_display_name_does_not_replace_miner_independence(self):
        client = Fake()
        client.mutate = lambda value, number: value['node']['binding'].update(node='pocol')
        self.run_collection(client)
        self.assertEqual(self.report['result'], 'PASS')
        client = Fake()
        client.mutate = lambda value, number: value['node']['binding'].update(miner=sample(0)['node']['binding']['miner']) if number == 1 else None
        with self.assertRaises(tool.Blocked): self.run_collection(client)

    def test_installation_path_layout_or_invitation_cannot_change_between_rounds(self):
        for field, change in [('paths', {'state': '/a/different/state'}), ('layout', 'standalone'),
                              ('bundle', {'invitationSha256': 'b' * 64})]:
            with self.subTest(field=field):
                client = Fake()
                client.mutate = lambda value, number: value.update({field: change}) if number == 3 else None
                with self.assertRaisesRegex(tool.Blocked, 'installation or public invitation'): self.run_collection(client)
                self.assertEqual(len(client.calls), 6)

    def test_stalled_head_is_not_claimed_as_progress(self):
        client = Fake()
        client.mutate = lambda value, number: value['node']['head'].update(height='201') if number == 7 else None
        with self.assertRaises(tool.Blocked): self.run_collection(client)
        self.assertEqual(self.report['phase'], 'progress')
        self.assertNotIn('all_three_heads_advanced', self.report)

    def test_running_source_change_between_rounds_is_refused(self):
        client = Fake()
        client.mutate = lambda value, number: value['node'].update(compatibilitySha256='b' * 64) if number >= 3 else None
        with self.assertRaises(tool.Blocked): self.run_collection(client)

    def test_identity_change_between_rounds_is_refused(self):
        client = Fake()
        client.mutate = lambda value, number: value['node']['binding'].update(node='replaced') if number == 6 else None
        with self.assertRaises(tool.Blocked): self.run_collection(client)

    def test_identity_change_in_common_round_is_refused(self):
        client = Fake()
        client.mutate = lambda value, number: value['node']['binding'].update(node='replaced') if number == 3 else None
        with self.assertRaises(tool.Blocked): self.run_collection(client)
        self.assertEqual(len(client.calls), 6)

    def test_too_young_network_has_no_selected_freeze_height(self):
        client = Fake()
        client.mutate = lambda value, number: value['node']['head'].update(height='60')
        with self.assertRaises(tool.Blocked): self.run_collection(client)
        self.assertNotIn('historical_comparison_height', self.report)


class TransportTests(unittest.TestCase):
    def client(self, runner):
        directory = tempfile.TemporaryDirectory(); self.addCleanup(directory.cleanup)
        key = Path(directory.name) / 'key'; key.write_text('never read key contents')
        return tool.SSH(key, runner=runner)

    def test_ssh_streams_only_pinned_program_to_fixed_read_only_command(self):
        calls = []
        def run(command, **kwargs):
            calls.append((command, kwargs))
            return subprocess.CompletedProcess(command, 0, json.dumps(sample(0)).encode(), b'')
        client = self.client(run)
        self.assertEqual(client.collect(tool.HOSTS[0], 140)['result'], 'PASS')
        command, options = calls[0]
        self.assertIn('StrictHostKeyChecking=yes', command)
        self.assertIn('BatchMode=yes', command)
        self.assertIn('IdentitiesOnly=yes', command)
        self.assertEqual(command[-2], 'pocol-admin@108.165.128.89')
        self.assertEqual(command[-1], 'sudo -n python3 -B - --rpc http://127.0.0.1:8545 --layout auto --check-height 140')
        self.assertEqual(options['input'], tool.load_program())
        self.assertNotIn(b'never read key contents', options['input'])
        self.assertNotIn('shell', options)

    def test_wrong_host_or_height_never_starts_ssh(self):
        calls = []
        client = self.client(lambda *args, **kwargs: calls.append(args))
        for host, height in [('other.example', None), (tool.HOSTS[0], True), (tool.HOSTS[0], -1)]:
            with self.assertRaises(tool.Blocked): client.collect(host, height)
        self.assertEqual(calls, [])

    def test_interactive_sudo_preserves_ssh_guards_and_uses_no_password_pipe(self):
        client = self.client(lambda *args, **kwargs: self.fail('Noninteractive runner must not run'))
        client.interactive_sudo = True
        with patch.object(tool.ssh_terminal, 'run', return_value=subprocess.CompletedProcess([], 0, json.dumps(sample(0)).encode(), b'')) as run, \
             contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(client.collect(tool.HOSTS[0], 140)['result'], 'PASS')
        command, begin, end = run.call_args.args
        self.assertIn('-tt', command); self.assertNotIn('-T', command)
        for guard in ['BatchMode=yes','IdentitiesOnly=yes','StrictHostKeyChecking=yes']:
            self.assertIn(guard, command)
        remote = shlex.split(command[-1])
        self.assertEqual(remote[:2], ['sudo','-p'])
        self.assertNotIn('-S', remote); self.assertNotIn('-n', remote)
        self.assertIn('--check-height', remote)
        self.assertEqual(remote[-1], '140')
        self.assertNotEqual(begin, end)

    def test_interactive_sudo_error_is_checkpoint_safe_and_never_contains_terminal_data(self):
        client = self.client(lambda *args, **kwargs: None); client.interactive_sudo = True
        with patch.object(tool.ssh_terminal, 'run', side_effect=tool.ssh_terminal.TerminalError('No complete public report')), \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(tool.Blocked, 'No complete public report'):
                client.collect(tool.HOSTS[0])

    def test_interactive_mode_requires_owner_terminal_before_any_ssh_or_report(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'report.json'
            with patch.object(tool.sys.stdin, 'isatty', return_value=False), patch.object(tool, 'SSH') as ssh, \
                 contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit): tool.main(['--interactive-sudo','--out',str(output)])
                ssh.assert_not_called()
            self.assertFalse(output.exists())

    def test_raw_ssh_diagnostic_is_not_copied_to_the_error(self):
        client = self.client(lambda *args, **kwargs: subprocess.CompletedProcess([], 255, b'', b'PRIVATE STDERR MATERIAL'))
        with self.assertRaises(tool.Blocked) as error: client.collect(tool.HOSTS[0])
        self.assertNotIn('PRIVATE', str(error.exception))

    def test_duplicate_json_invalid_json_or_changed_operation_is_refused(self):
        for content in [b'{"schema":"a","schema":"b"}', b'not JSON',
                        json.dumps({**sample(0), 'deploymentPerformed': True}).encode()]:
            client = self.client(lambda *args, **kwargs: subprocess.CompletedProcess([], 0, content, b''))
            with self.assertRaises(tool.Blocked): client.collect(tool.HOSTS[0])

    def test_hash_mismatch_is_refused_before_any_ssh_call(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'preflight.py'; path.write_text('changed code')
            with self.assertRaises(tool.Blocked): tool.load_program(path)

    def test_program_preserves_reviewed_meaning_with_windows_newlines(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'preflight.py'
            path.write_bytes(tool.load_program().replace(b'\n', b'\r\n'))
            self.assertEqual(tool.load_program(path), tool.load_program())

    def test_existing_report_is_preserved_before_ssh(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'report.json'; output.write_text('keep')
            with patch.object(tool, 'SSH') as ssh, contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit): tool.main(['--out', str(output)])
                ssh.assert_not_called()
            self.assertEqual(output.read_text(), 'keep')

    def test_cli_produces_one_completed_owner_report(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'report.json'
            with patch.object(tool, 'SSH', return_value=Fake()), \
                 contextlib.redirect_stdout(io.StringIO()):
                # The explicitly injected execute avoids a real observation delay in this unit test.
                original = tool.execute
                with patch.object(tool, 'execute', side_effect=lambda client, report, checkpoint, **kwargs:
                                  original(client, report, checkpoint, pause=lambda seconds: None, **kwargs)):
                    self.assertEqual(tool.main(['--out', str(output)]), 0)
            value = json.loads(output.read_text())
            self.assertEqual(value['result'], 'PASS')
            self.assertTrue(value['read_only'])
            self.assertFalse(value['deployment_performed'])
            self.assertFalse(value['freeze_height_selected'])


if __name__ == '__main__':
    unittest.main()
