"""install_observer_node.py: the invitation checks, the unit (no miner, RPC on 127.0.0.1:8545 only, the invitation's
seeds, the hardening) and the install, --check and rollback paths on temporary paths with every system command faked.
The unit's command line against a live devnet is deploy/explorer/e2e_explorer_devnet.py."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import install_observer_node as observer  # noqa: E402

GENESIS = '0x' + '5a' * 32
SEEDS = ['192.0.2.10:30300', '192.0.2.11:30300', '[2001:db8::1]:30300']


def invitation(**changes):
    files = {'alloc.json': '{"0x' + '11' * 20 + '": {"balance": "1"}}\n',
             'devnet.json': json.dumps({'chainId': 777963, 'genesisHash': GENESIS, 'specVersion': 2, 'v3': True}),
             'genesis.spec.hex': '0xf901bc02\n'}
    value = {'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': list(SEEDS), 'files': files}
    value.update(changes)
    return json.dumps(value).encode()


class InvitationTests(unittest.TestCase):
    def test_a_valid_invitation(self):
        chain, genesis, seeds, files = observer.parse_invitation(invitation())
        self.assertEqual((chain, genesis, seeds), (777963, GENESIS, SEEDS))
        self.assertEqual(sorted(files), ['alloc.json', 'devnet.json', 'genesis.spec.hex'])

    def test_refusals(self):
        good = json.loads(invitation())
        bad = [{'schema': 'pocol-network-v1'}, {'seeds': []}, {'seeds': ['seed.example:30300']}, {'seeds': ['192.0.2.1:0']},
               {'seeds': ['0.0.0.0:30300']}, {'seeds': ['224.0.0.1:30300']}, {'seeds': ['192.0.2.1:30300', '192.0.2.1:30300']},
               {'seeds': ['192.0.2.1:70000']}, {'files': {k: v for k, v in good['files'].items() if k != 'alloc.json'}},
               {'files': {**good['files'], 'devnet.json': json.dumps({'chainId': 'x', 'genesisHash': GENESIS})}},
               {'files': {**good['files'], 'devnet.json': json.dumps({'chainId': 1, 'genesisHash': GENESIS.upper()})}},
               {'files': {**good['files'], 'genesis.spec.hex': 'not hex'}},
               # More seeds than pocold takes (node::SEEDS_MAX 7), and seeds that are not one plain address: an IPv6
               # zone (which ipaddress accepts, spaces included) would split into more pocold flags in ExecStart.
               {'seeds': [f'192.0.2.{i}:30300' for i in range(1, 9)]},
               {'seeds': ['[fe80::1%a --listen 0.0.0.0 --x]:30300']}, {'seeds': ['[fe80::1%a --peer 203.0.113.9]:30300']},
               {'seeds': ['[fe80::1%eth0]:30300']}, {'seeds': ['192.0.2.1:30300 --miner x']}, {'seeds': [' 192.0.2.1:30300']},
               {'seeds': ['192.0.2.1:030300']}, {'seeds': ['192.0.2.01:30300']}, {'seeds': ['[192.0.2.1]:30300']},
               {'seeds': ['2001:db8::1:30300']}, {'seeds': ['[2001:db8::1]:30300%%n']}, {'seeds': ['192.0.2.1:30300\n']},
               {'seeds': [30300]}]
        for change in bad:
            with self.subTest(change=change), self.assertRaises(ValueError):
                observer.parse_invitation(invitation(**change))
        with self.assertRaises(ValueError):
            observer.parse_invitation(b'x' * (observer.INVITATION_MAX + 1))
        self.assertEqual(observer.SEEDS_MAX, 7, "pocold's node::SEEDS_MAX")
        seven = [f'192.0.2.{i}:30300' for i in range(1, 8)] + []
        self.assertEqual(observer.parse_invitation(invitation(seeds=seven))[2], seven)
        self.assertEqual(observer.parse_invitation(invitation(seeds=['[::ffff:192.0.2.1]:30300']))[2], ['[::ffff:192.0.2.1]:30300'])

    @unittest.skipUnless(os.environ.get('POCOL_INVITATION'), 'POCOL_INVITATION names an invitation file to parse')
    def test_an_operator_invitation_parses(self):
        chain, genesis, seeds, _ = observer.parse_invitation(Path(os.environ['POCOL_INVITATION']).read_bytes())
        self.assertGreater(chain, 0); self.assertTrue(seeds)


class UnitTests(unittest.TestCase):
    def test_the_unit_runs_no_miner_and_loopback_rpc_only(self):
        text = observer.unit_text(SEEDS)
        line = next(l for l in text.splitlines() if l.startswith('ExecStart='))
        self.assertEqual(line, 'ExecStart=/opt/pocol-observer/pocold run --network /etc/pocol-observer/network --data '
                               '/var/lib/pocol-observer/data --name explorer-observer --rpc 127.0.0.1:8545 '
                               '--seed 192.0.2.10:30300 --seed 192.0.2.11:30300 --seed [2001:db8::1]:30300')
        self.assertNotIn('--miner', text); self.assertNotIn('--listen', text)
        for setting in ('DynamicUser=yes', 'StateDirectory=pocol-observer', 'ProtectSystem=strict', 'NoNewPrivileges=yes',
                        'MemoryMax=7G', 'MemorySwapMax=0', 'RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX'):
            self.assertIn(setting + '\n', text)
        self.assertIn('--rpc 127.0.0.1:8545 --listen 0.0.0.0:30300 --seed', observer.unit_text(SEEDS, '0.0.0.0:30300'))

    def test_the_guard_refuses_a_miner_or_another_rpc_address(self):
        args = observer.exec_args(SEEDS)
        observer.check_args(args)
        for bad in (args + ['--miner', 'a'], args + ['--miner-key-file', '/k'], [a.replace('127.0.0.1:8545', '0.0.0.0:8545') for a in args],
                    args + ['--rpc', '127.0.0.1:8546']):
            with self.subTest(bad=bad[-2:]), self.assertRaises(RuntimeError):
                observer.check_args(bad)
        with patch.object(observer, 'RPC', '0.0.0.0:8545'), self.assertRaises(RuntimeError):
            observer.check_args(observer.exec_args(SEEDS)[:-6] + ['--rpc', '0.0.0.0:8545'])

    def test_no_argument_can_split_into_more_flags_or_carry_a_specifier(self):
        # The guard itself, for a seed that reached it some other way: whitespace, %, $, quotes, backslashes refused.
        for seed in ('[fe80::1%a --listen 0.0.0.0 --x]:30300', '192.0.2.1:30300 --peer 203.0.113.9', '192.0.2.1:%H',
                     '$HOME', '"192.0.2.1:1"', '192.0.2.1:1\\', '192.0.2.1:1;x', '192.0.2.1:1\tx', ''):
            with self.subTest(seed=seed), self.assertRaisesRegex(RuntimeError, 'Unexpected character'):
                observer.unit_text([seed])
        line = next(l for l in observer.unit_text(SEEDS, '0.0.0.0:30300').splitlines() if l.startswith('ExecStart='))
        self.assertRegex(line, r'^ExecStart=[A-Za-z0-9/._:\[\] -]+$')
        self.assertEqual(line.count(' --'), 5 + len(SEEDS), 'one flag per argument pair, nothing more')


class InstallTests(unittest.TestCase):
    def run_install(self, extra=(), raw=None, binary=b'\x7fELF fake pocold', identity=None, existing=None, gateway_network=None,
                    fail=None, digest=None, invitation_digest=None, pinned=None):
        """main() on temporary paths: (root, commands, printed, error). `identity` is what the node's RPC answers
        (default: the invitation's network, no miner); `existing(paths)` prepares a previous install."""
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        raw = invitation() if raw is None else raw
        (root / 'in').mkdir()
        (root / 'in' / 'network.pocol.json').write_bytes(raw); (root / 'in' / 'pocold').write_bytes(binary)
        paths = dict(BASE=root / 'opt', CONFIG=root / 'etc', NETWORK_DIR=root / 'etc' / 'network', INVITATION=root / 'etc' / 'network.pocol.json',
                     UNIT=root / 'observer.service', BACKUPS=root / 'backups', GATEWAY_NETWORK=root / 'gw' / 'network.json',
                     REPO_IDENTITY=root / 'repo' / 'site-identity.json')
        # The repository's pinned identity: the test invitation's network unless a test says otherwise.
        (root / 'repo').mkdir()
        if pinned is not False:
            paths['REPO_IDENTITY'].write_text(json.dumps(pinned or {'chainId': 777963, 'genesisHash': GENESIS}))
        if gateway_network is not None:
            (root / 'gw').mkdir(); paths['GATEWAY_NETWORK'].write_text(json.dumps(gateway_network))
        if existing:
            existing(paths)
        commands = []
        def command(*args, **kwargs):
            commands.append(args)
            if fail and args[:2] == ('systemctl', fail): raise RuntimeError('systemctl ' + fail + ' failed')
            absent = args[:2] in (('systemctl', 'is-active'), ('systemctl', 'is-enabled')) and not existing
            return SimpleNamespace(returncode=1 if absent else 0, stdout=b'', stderr=b'')
        answer = identity if identity is not None else (777963, GENESIS, None)
        argv = ['--invitation', str(root / 'in' / 'network.pocol.json'),
                '--invitation-sha256', invitation_digest or hashlib.sha256(raw).hexdigest(), '--pocold', str(root / 'in' / 'pocold'),
                '--pocold-sha256', digest or hashlib.sha256(binary).hexdigest(), '--wait', '10', *extra]
        error = None
        with patch.multiple(observer, **paths), \
             patch.object(observer.os, 'geteuid', return_value=0, create=True), \
             patch.object(observer.shutil, 'which', return_value='/bin/systemctl'), \
             patch.object(observer.socket, 'socket', return_value=MagicMock()), \
             patch.object(observer, 'command', side_effect=command), \
             patch.object(observer, 'rpc_identity', return_value=answer), \
             patch.object(observer.time, 'sleep'), \
             patch('builtins.print') as output:
            try:
                observer.main(argv)
            except RuntimeError as exc:
                error = str(exc)
        return root, commands, ' '.join(str(c) for c in output.call_args_list), error

    def test_install_writes_the_binary_network_files_and_unit_then_enables_after_the_identity(self):
        root, commands, printed, error = self.run_install()
        self.assertIsNone(error)
        self.assertEqual((root / 'opt' / 'pocold').read_bytes(), b'\x7fELF fake pocold')
        self.assertEqual((root / 'etc' / 'network.pocol.json').read_bytes(), invitation())
        files = json.loads(invitation())['files']
        for name in observer.FILES:
            self.assertEqual((root / 'etc' / 'network' / name).read_text(), files[name], name)
        self.assertEqual((root / 'observer.service').read_text(), observer.unit_text(SEEDS).replace('/etc/pocol-observer/network', str(root / 'etc' / 'network')))
        self.assertLess(commands.index(('systemctl', 'restart', observer.SERVICE)), commands.index(('systemctl', 'enable', observer.SERVICE)))
        self.assertIn('INSTALLED', printed)

    def test_check_changes_nothing(self):
        root, commands, printed, error = self.run_install(extra=['--check'])
        self.assertIsNone(error)
        self.assertIn('CHECK PASSED', printed)
        self.assertFalse((root / 'opt').exists()); self.assertFalse((root / 'etc').exists()); self.assertFalse((root / 'observer.service').exists())
        self.assertNotIn(('systemctl', 'restart', observer.SERVICE), commands)

    def test_refusals_change_nothing(self):
        cases = {
            'invitation digest': dict(invitation_digest='0' * 64),
            'gateway network': dict(gateway_network={'chainId': 1, 'genesisHash': GENESIS}),
            'another invitation': dict(existing=lambda p: (p['CONFIG'].mkdir(), p['BASE'].mkdir(), (p['BASE'] / observer.MARKER).write_text('x'),
                                                          p['INVITATION'].write_bytes(invitation(seeds=['192.0.2.99:30300'])))),
            'unmarked unit': dict(existing=lambda p: p['UNIT'].write_text('[Unit]\n')),
            'unrecognized base': dict(existing=lambda p: p['BASE'].mkdir()),
        }
        for name, kwargs in cases.items():
            with self.subTest(name):
                root, commands, _, error = self.run_install(**kwargs)
                self.assertIsNotNone(error)
                self.assertNotIn(('systemctl', 'restart', observer.SERVICE), commands)
                self.assertFalse((root / 'opt' / 'pocold').exists())
        root, commands, _, error = self.run_install(invitation_digest='0' * 64)
        self.assertRegex(error, '--invitation-sha256')
        root, commands, _, error = self.run_install(digest='0' * 64)
        self.assertRegex(error, '--pocold-sha256')
        self.assertEqual(commands, []); self.assertFalse((root / 'opt').exists())

    def test_a_node_that_names_another_network_or_a_miner_is_rolled_back(self):
        for identity, message in (((1, GENESIS, None), 'did not answer'), ((777963, GENESIS, '0x' + 'ab' * 20), 'must not mine')):
            with self.subTest(message):
                root, commands, printed, error = self.run_install(identity=identity)
                self.assertRegex(error, message)
                self.assertNotIn('INSTALLED', printed)
                for path in ('opt', 'etc', 'observer.service'):
                    self.assertFalse((root / path).exists(), path)
                self.assertIn(('systemctl', 'stop', observer.SERVICE), commands)
                self.assertNotIn(('systemctl', 'enable', observer.SERVICE), commands)

    def test_a_failed_update_restores_the_previous_install(self):
        old_unit = observer.unit_text(SEEDS[:2])
        def existing(p):
            p['BASE'].mkdir(); (p['BASE'] / observer.MARKER).write_text('PoCol observer node v1\n'); (p['BASE'] / 'pocold').write_bytes(b'old')
            p['CONFIG'].mkdir(); p['INVITATION'].write_bytes(invitation()); p['UNIT'].write_text(old_unit)
        root, commands, _, error = self.run_install(existing=existing, identity=(1, GENESIS, None))
        self.assertRegex(error, 'did not answer')
        self.assertEqual((root / 'opt' / 'pocold').read_bytes(), b'old')
        self.assertEqual((root / 'observer.service').read_text(), old_unit)
        self.assertEqual(commands[-1], ('systemctl', 'restart', observer.SERVICE))

    def test_an_invitation_of_another_network_than_the_repository_pins_is_refused(self):
        """Final review (2026-11-22): a valid invitation with a matching operator-supplied digest, but of another
        network than site-identity.json pins, installs nothing unless --other-network says so."""
        other = {'chainId': 4242, 'genesisHash': '0x' + '66' * 32}
        root, commands, _, error = self.run_install(pinned=other)
        self.assertIn('not the network this repository pins', error or '')
        self.assertFalse((root / 'opt').exists()); self.assertEqual(commands, [])
        root, commands, _, error = self.run_install(pinned=False)
        self.assertIn('is missing', error or ''); self.assertFalse((root / 'opt').exists())
        root, commands, _, error = self.run_install(extra=('--other-network',), pinned=other)
        self.assertIsNone(error); self.assertTrue((root / 'opt' / 'pocold').is_file())
        repo = Path(__file__).resolve().parents[1] / 'websites' / 'site-identity.json'
        declared = json.loads(repo.read_text())
        self.assertEqual(observer.pinned_identity(repo), (declared['chainId'], declared['genesisHash']), 'the installer reads the pin the websites use')

    def test_usage(self):
        full = ['--invitation', 'x', '--invitation-sha256', '1' * 64, '--pocold', 'y', '--pocold-sha256', '0' * 64]
        self.assertEqual(observer.usage(full)['--invitation-sha256'], '1' * 64)
        for argv in ([], ['--invitation', 'x'], ['--invitation', 'x', '--pocold', 'y'], ['--invitation', 'x', '--pocold', 'y', '--pocold-sha256', 'ab'],
                     # --invitation-sha256 is required: the invitation decides the node's seeds and network identity.
                     ['--invitation', 'x', '--pocold', 'y', '--pocold-sha256', '0' * 64], full[:2] + ['--invitation-sha256', 'AB' * 32] + full[4:],
                     ['--invitation', 'x', '--pocold', 'y', '--pocold-sha256', '0' * 64, '--listen', 'host:1'],
                     ['--invitation', 'x', '--pocold', 'y', '--pocold-sha256', '0' * 64, '--wait', '5'],
                     ['--invitation', 'x', '--pocold', 'y', '--pocold-sha256', '0' * 64, '--check', '--check']):
            with self.subTest(argv=argv), self.assertRaises(RuntimeError):
                observer.usage(argv)


if __name__ == '__main__':
    unittest.main()
