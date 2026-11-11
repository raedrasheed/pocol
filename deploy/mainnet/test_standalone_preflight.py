"""Installer-layout regression controls using disposable public files and a real /proc child.

The child is a copied Python interpreter, with test-only expected binary hashes.
It holds the fixture's LOCK/state.redb open without touching a node or service.
The old public GenesisSpec comes from the already published invitation.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import preflight as tool


class LayoutTests(unittest.TestCase):
    def test_auto_recognizes_each_known_active_service_without_guessing_paths(self):
        for kind, paths in tool.LAYOUTS.items():
            with self.subTest(kind=kind):
                services = [{'unit': paths[2], 'activeState': 'active'}]
                self.assertEqual(tool.choose_layout('auto', *tool.LAYOUTS['manifest'], services), (kind, *paths))

    def test_missing_ambiguous_or_custom_auto_layout_is_not_accepted(self):
        both = [{'unit': paths[2], 'activeState': 'active'} for paths in tool.LAYOUTS.values()]
        for services in ([], both, [{'unit': 'custom.service', 'activeState': 'active'}]):
            with self.subTest(services=services), self.assertRaises(tool.PreflightError):
                tool.choose_layout('auto', *tool.LAYOUTS['manifest'], services)
        with self.assertRaises(tool.PreflightError):
            tool.choose_layout('auto', Path('/custom'), Path('/state'), 'custom.service', [])

    def test_unknown_service_discovery_keeps_names_only_and_remains_bounded(self):
        response = subprocess.CompletedProcess([], 0, 'pocol-custom.service loaded active running PRIVATE_DESCRIPTION\n')
        with patch.object(tool.subprocess, 'run', return_value=response) as run:
            self.assertEqual(tool.additional_node_units(), ['pocol-custom.service'])
        self.assertEqual(run.call_args.args[0][-1], 'pocol*.service')
        response.stdout *= 25
        with patch.object(tool.subprocess, 'run', return_value=response): self.assertEqual(tool.additional_node_units(), [])


@unittest.skipUnless(sys.platform == 'linux' and os.getuid() > 0, 'Linux non-root /proc fixture')
class StandaloneTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.bundle, self.home = self.root / 'bundle', self.root / 'home'
        self.bundle.mkdir(); self.home.mkdir()
        (self.home / 'network').mkdir(); (self.home / 'state').mkdir()
        self.invitation = json.loads((Path(__file__).parent / 'h15000/network.pocol.json').read_text())
        self.write_invitations()
        for name, content in self.invitation['files'].items():
            (self.home / 'network' / name).write_bytes(content.encode())
        self.miner = '0x' + '11' * 20
        for directory in (self.bundle, self.home):
            (directory / 'identity.address').write_text(self.miner + '\n')
        # Empty private-path sentinels: no key/password value is ever written.
        # The open guard below proves the inventory does not inspect them.
        (self.home / 'identity.pocol-key').touch()
        (self.bundle / 'operator-private.env').touch()
        self.marker = 'UNREQUESTED_PUBLIC_TEST_MARKER'
        for name in tool.LEGACY_BINARIES:
            (self.bundle / name).write_bytes(('public test fixture ' + name).encode())
        shutil.copyfile(Path(sys.executable).resolve(), self.bundle / 'pocol')
        (self.bundle / 'pocol').chmod(0o755)
        hashes = {name: hashlib.sha256((self.bundle / name).read_bytes()).hexdigest() for name in tool.LEGACY_BINARIES}
        guard = patch.object(tool, 'LEGACY_BINARIES', hashes); guard.start(); self.addCleanup(guard.stop)
        (self.home / 'state/blocks.journal').write_bytes(b'POCOLJ2\n' + bytes.fromhex(tool.GENESIS[2:]) + b'untouched history')
        (self.home / 'state/LOCK').write_bytes(b'')
        (self.home / 'state/state.redb').write_bytes(b'never open this as a database')
        child = ('import sys\nfrom pathlib import Path\n'
                 'state=Path(sys.argv[1])\n'
                 'held=[(state/n).open("rb") for n in ("LOCK","state.redb")]\n'
                 'print("ready",flush=True)\nsys.stdin.buffer.read(1)\n')
        self.child = subprocess.Popen([str(self.bundle / 'pocol'), '-B', '-c', child, str(self.home / 'state')],
                                      stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                      env={**os.environ, 'PYTHONHOME': sys.base_prefix})
        self.addCleanup(self.close_child)
        self.assertEqual(self.child.stdout.readline(), b'ready\n')
        self.service = {'unit': 'pocol-standalone.service', 'activeState': 'active', 'available': True,
                        'user': 'pocol-standalone', 'stateDirectory': 'pocol-standalone', 'mainPID': self.child.pid}
        guard = patch.object(tool, 'service_report', side_effect=lambda unit: dict(self.service))
        guard.start(); self.addCleanup(guard.stop)
        self.params = {'chainId': tool.CHAIN_ID, 'genesisHash': tool.GENESIS,
                       'compatibilitySha256': tool.LEGACY_SOURCE, 'protocolVersion': 2,
                       'forkSchedule': [[1, 0], [2, 15000]], 'activeVersion': 2}
        self.status = {**self.params, 'name': 'pocol', 'miner': self.miner, 'stateStore': True,
                       'head': {'height': 77346, 'hash': '0x' + '22' * 32},
                       'compatiblePeers': 6, 'stats': {'historyDamaged': False},
                       'unrequestedField': self.marker}
        self.block = {'number': hex(77346), 'hash': self.status['head']['hash'],
                      'stateRoot': '0x' + '33' * 32, 'receiptsRoot': '0x' + '44' * 32}

    def close_child(self):
        if self.child.poll() is None:
            self.child.stdin.close()
            try: self.child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.child.terminate(); self.child.wait(timeout=5)
        for stream in (self.child.stdout, self.child.stderr): stream.close()

    def write_invitations(self):
        for directory in (self.bundle, self.home):
            (directory / 'network.pocol.json').write_text(json.dumps(self.invitation))

    def rpc(self, method, params):
        return copy.deepcopy({'pocol_getParams': self.params, 'pocol_status': self.status,
                              'eth_getBlockByNumber': self.block}[method])

    def collect(self):
        return tool.collect(self.rpc, self.bundle, self.home, self.service['unit'],
                            services=[self.service], layout='standalone')

    def test_legacy_installer_without_manifest_has_a_verified_public_binding_and_no_writes(self):
        before = {p: p.read_bytes() for p in self.home.rglob('*') if p.is_file()}
        opened, original = [], tool.open_public
        def public_only(path, limit=None):
            self.assertNotIn(path.name, ('identity.pocol-key', 'operator-private.env'))
            opened.append(path)
            return original(path, limit)
        with patch.object(tool, 'open_public', side_effect=public_only): result = self.collect()
        self.assertEqual(result['result'], 'PASS', result['issues'])
        self.assertEqual(result['layout'], 'standalone')
        self.assertEqual(result['paths']['state'], str(self.home / 'state'))
        self.assertEqual(result['stateBinding']['miner'], self.miner)
        self.assertEqual(result['bindingEvidence']['openStateFilesVerified'], ['LOCK', 'state.redb'])
        self.assertEqual(result['bindingEvidence']['mainPID'], self.child.pid)
        self.assertFalse(result['deploymentPerformed']); self.assertFalse(result['activationHeightSelected'])
        self.assertNotIn(self.marker, json.dumps(result))
        self.assertEqual({p: p.read_bytes() for p in self.home.rglob('*') if p.is_file()}, before)
        self.assertNotIn(self.home / 'state/state.redb', opened)  # Only stat/fd metadata, no DB read.

    def test_auto_uses_the_standalone_home_and_state_instead_of_the_manifest_defaults(self):
        with patch.dict(tool.LAYOUTS, {'standalone': (self.bundle, self.home, self.service['unit'])}):
            result = tool.collect(self.rpc, *tool.LAYOUTS['manifest'][:2], services=[self.service], layout='auto')
        self.assertEqual(result['result'], 'PASS', result['issues'])
        self.assertEqual(result['paths']['home'], str(self.home))

    def test_unreviewed_binary_is_not_trusted_from_its_own_hash(self):
        (self.bundle / 'pocol-sign').write_bytes(b'changed native binary')
        result = self.collect()
        self.assertFalse(result['checks']['bundle'])
        self.assertNotIn('stateBinding', result)

    def test_different_running_source_cannot_inherit_published_binary_binding(self):
        # Another source with no consensus identity is not the published 0.4.0 build: refused at the node.
        self.params['compatibilitySha256'] = self.status['compatibilitySha256'] = 'f' * 64
        result = self.collect()
        self.assertFalse(result['checks']['node']); self.assertEqual(result['result'], 'FAIL')
        # An R1 node on the 0.4.0 installation runs another release: refused at the bundle.
        self.r1_status()
        result = self.collect()
        self.assertFalse(result['checks']['bundle'])
        self.assertIn('running consensus identity differs', json.dumps(result['issues']))

    def r1_status(self, source='c' * 64):
        golden = 'bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110'  # the T11 H15000 identity
        for value in (self.params, self.status):
            value.update(compatibilitySha256=source, consensusRulesVersion=1, consensusIdentity=golden, consensusPinned=True)

    def r1_release(self, saved_v1=True):
        """Pin this fixture's binaries as the R1 package, with its v2 twin installed and the 0.4.0 file (or the twin)
        saved in the home."""
        twin = {'schema': 'pocol-network-v2', 'consensusRulesVersion': 1,
                'seeds': self.invitation['seeds'], 'files': self.invitation['files']}
        (self.bundle / 'network.pocol.json').write_text(json.dumps(twin))
        if not saved_v1: (self.home / 'network.pocol.json').write_text(json.dumps(twin))
        hashes = dict(tool.LEGACY_BINARIES)
        self.r1_status()
        return [patch.object(tool, 'LEGACY_BINARIES', {name: '0' * 64 for name in hashes}),
                patch.object(tool, 'R1_BINARIES', hashes), patch.object(tool, 'R1_SOURCE', 'c' * 64)]

    def test_the_r1_profile_fails_closed_while_its_release_is_not_pinned(self):
        patches = self.r1_release()
        for guard in patches[:1]: guard.start(); self.addCleanup(guard.stop)
        result = self.collect()
        self.assertFalse(result['checks']['bundle'])
        self.assertIn('differs from the published legacy release', json.dumps(result['issues']))

    def test_the_pinned_r1_profile_accepts_a_saved_0_4_0_binding_with_its_v2_twin_and_equal_v2_bindings(self):
        for saved_v1 in (True, False):
            with self.subTest(saved_v1=saved_v1):
                patches = self.r1_release(saved_v1)
                for guard in patches: guard.start()
                try: result = self.collect()
                finally:
                    for guard in patches: guard.stop()
                self.assertEqual(result['result'], 'PASS', result['issues'])
                self.assertEqual(result['bundle']['release'], 'R1')
                self.assertEqual(result['bundle']['consensusIdentity'], self.params['consensusIdentity'])
                self.write_invitations()

    def test_the_r1_profile_accepts_the_advisory_v2_fields_and_nothing_else(self):
        """PR2b (D17, D23): a v2 invitation may carry `bootstrap` and `seedsRetired`, outside the network's identity;
        any other extra field is still refused."""
        patches = self.r1_release()
        for guard in patches: guard.start(); self.addCleanup(guard.stop)
        twin = json.loads((self.bundle / 'network.pocol.json').read_text())
        for extra, ok in (({'bootstrap': ['192.0.2.7:30300']}, True), ({'seedsRetired': False}, True),
                          ({'bootstrap': [], 'seedsRetired': True}, True), ({'relay': True}, False)):
            with self.subTest(extra=extra):
                (self.bundle / 'network.pocol.json').write_text(json.dumps({**twin, **extra}))
                result = self.collect()
                self.assertEqual(result['checks']['bundle'], ok, result['issues'])
        (self.bundle / 'network.pocol.json').write_text(json.dumps(twin))

    def test_the_r1_profile_refuses_a_twin_with_other_content_or_another_saved_file(self):
        patches = self.r1_release()
        for guard in patches: guard.start(); self.addCleanup(guard.stop)
        twin = json.loads((self.bundle / 'network.pocol.json').read_text())
        for change in ({'seeds': ['192.0.2.1:30300']}, {'consensusRulesVersion': 2}):
            with self.subTest(change=change):
                (self.bundle / 'network.pocol.json').write_text(json.dumps({**twin, **change}))
                self.assertFalse(self.collect()['checks']['bundle'])
        (self.bundle / 'network.pocol.json').write_text(json.dumps(twin))
        (self.home / 'network.pocol.json').write_text(json.dumps({**self.invitation, 'compatibilitySha256': 'a' * 64}))
        self.assertFalse(self.collect()['checks']['bundle'])

    def test_other_public_address_cannot_claim_the_running_miner(self):
        for directory in (self.bundle, self.home):
            (directory / 'identity.address').write_text('0x' + '22' * 20)
        self.assertFalse(self.collect()['checks']['bundle'])

    def test_mismatching_install_and_home_invitations_are_refused(self):
        (self.home / 'network.pocol.json').write_text(json.dumps({**self.invitation, 'compatibilitySha256': 'a' * 64}))
        self.assertFalse(self.collect()['checks']['bundle'])

    def test_network_file_change_is_not_hidden_by_matching_invitation(self):
        (self.home / 'network/alloc.json').write_text('{}')
        self.assertFalse(self.collect()['checks']['bundle'])

    def test_host_selected_genesis_spec_cannot_replace_the_independent_pin(self):
        self.invitation['files']['genesis.spec.hex'] = '00'
        self.write_invitations(); (self.home / 'network/genesis.spec.hex').write_text('00')
        self.assertFalse(self.collect()['checks']['bundle'])

    def test_wrong_journal_identity_does_not_open_or_repair_live_state(self):
        path = self.home / 'state/blocks.journal'; wrong = b'POCOLJ2\n' + b'\0' * 32
        path.write_bytes(wrong)
        self.assertFalse(self.collect()['checks']['stateBinding'])
        self.assertEqual(path.read_bytes(), wrong)

    def test_other_executable_pid_or_service_account_cannot_supply_storage_proof(self):
        for field, value in [('mainPID', os.getpid()), ('mainPID', 0), ('user', 'root'),
                             ('stateDirectory', 'elsewhere'), ('activeState', 'inactive')]:
            with self.subTest(field=field, value=value):
                original = dict(self.service); self.service[field] = value
                self.assertFalse(self.collect()['checks']['stateBinding'])
                self.service.clear(); self.service.update(original)

    def test_copy_of_unheld_database_is_not_the_running_database(self):
        database = self.home / 'state/state.redb'
        database.rename(database.with_suffix('.aside'))
        database.write_bytes(b'a different inode at the same path')
        self.assertFalse(self.collect()['checks']['stateBinding'])

    def test_process_change_during_capture_is_not_reported_as_a_stable_binding(self):
        with patch.object(tool, 'service_report', return_value={**self.service, 'mainPID': self.child.pid + 1}):
            self.assertFalse(self.collect()['checks']['stateBinding'])

    def test_public_symlink_hardlink_and_fifo_are_refused_before_reading(self):
        path, private_path = self.home / 'identity.address', self.home / 'identity.pocol-key'
        for kind in ('symlink', 'hardlink', 'fifo'):
            with self.subTest(kind=kind):
                path.unlink()
                if kind == 'symlink': path.symlink_to(private_path)
                elif kind == 'hardlink': os.link(private_path, path)
                else: os.mkfifo(path)
                with patch.object(tool.os, 'open', wraps=os.open) as opened:
                    self.assertFalse(self.collect()['checks']['bundle'])
                    self.assertNotIn(path, [call.args[0] for call in opened.call_args_list])
                path.unlink(); path.write_text(self.miner)

    def test_symlinked_parent_of_public_files_is_not_followed(self):
        directory = self.home / 'network'; other = self.home / 'network-original'
        directory.rename(other); directory.symlink_to(other, target_is_directory=True)
        self.assertFalse(self.collect()['checks']['bundle'])


if __name__ == '__main__':
    unittest.main()
