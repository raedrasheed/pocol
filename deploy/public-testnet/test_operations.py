#!/usr/bin/env python3
"""Failure-oriented controls for key-free releases and offline recovery."""
import copy
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import unittest.mock
import node_operator as op
import prepare
import monitor


def inventory():
    return {'name': 'test', 'nodes': [{'name': f'n{i}', 'ip': f'127.0.0.{i+2}', 'p2pPort': 30300,
        'rpcPort': 8540+i, 'rpcHost': f'rpc{i}.example.test', 'minerAddress': '0x' + f'{i+1:040x}', 'seeds': []} for i in range(3)]}


class Operations(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup); self.root = Path(self.tmp.name)

    def test_inventory_rejects_exposure_and_injection_errors(self):
        good = inventory(); self.assertEqual(prepare.validate_inventory(good, True), good)
        for field, value in [('ip', '0.0.0.0'), ('ip', '169.254.169.254'), ('ip', '8.8.8.8'),
            ('name', '../bad'), ('rpcHost', 'x;\ninclude /tmp/evil;'), ('rpcHost', '-x.example'),
            ('minerAddress', '0x'+'0'*40), ('p2pPort', True), ('p2pPort', 8540), ('seeds', ['127.0.0.2:30300'])]:
            bad = copy.deepcopy(good); bad['nodes'][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises((ValueError, TypeError)):
                prepare.validate_inventory(bad, True)
        with self.assertRaises(ValueError): prepare.validate_inventory(good)
        for address in ['224.0.0.1', '239.1.2.3', '255.255.255.255', '192.0.2.1', '10.0.0.1']:
            self.assertFalse(prepare.allowed_ip(prepare.ipaddress.IPv4Address(address), False))
        bad = copy.deepcopy(good); bad['nodes'][1]['minerAddress'] = bad['nodes'][0]['minerAddress']
        with self.assertRaises(ValueError): prepare.validate_inventory(bad, True)
        single = {'name': 'join', 'nodes': [good['nodes'][0]]}
        with self.assertRaises(ValueError): prepare.validate_inventory(single, True, True)
        single['nodes'][0]['seeds'] = ['127.0.0.8:30300']; prepare.validate_inventory(single, True, True)

    def test_inventory_refuses_the_removed_freeze_fields(self):
        for field in ['freezeUserTxsAfter', 'freezeGenesisHash']:
            bad = inventory(); bad['nodes'][0][field] = 1000 if field == 'freezeUserTxsAfter' else '0x' + 'ab' * 32
            with self.assertRaisesRegex(ValueError, 'Invalid node fields', msg=field): prepare.validate_inventory(bad, True)

    def test_checksums_reject_links_extras_and_traversal(self):
        d = self.root / 'files'; d.mkdir(); (d / 'file').write_bytes(b'public')
        hashes = {'file': op.digest(d / 'file')}; op.verify_files(d, hashes, set())
        (d / 'extra').write_bytes(b'extra')
        with self.assertRaises(ValueError): op.verify_files(d, hashes, set())
        (d / 'extra').unlink(); (d / 'link').symlink_to(d / 'file')
        with self.assertRaises(ValueError): op.files(d)
        (d / 'link').unlink(); os.link(d / 'file', d / 'hard')
        with self.assertRaises(ValueError): op.files(d)
        (d / 'hard').unlink()
        with self.assertRaises(ValueError): op.verify_files(d, {'../file': hashes['file']}, set())
        (d / 'file').write_bytes(b'changed')
        with self.assertRaises(ValueError): op.verify_files(d, hashes, set())

    def state(self):
        data = self.root / 'state'; data.mkdir()
        op.write_json(data / 'network-binding.json', {'chainId': 123, 'miner': None})
        for name in ('blocks.journal', 'blocks.commit'): (data / name).write_bytes(b'fixture\0\xff')
        return data

    def test_backup_respects_both_locks_and_never_includes_keys(self):
        data = self.state()
        for name in ('.operator.lock', 'LOCK'):
            with (data / name).open('a') as handle:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                with self.assertRaises(BlockingIOError): op.backup(data, self.root / 'backup')
                self.assertFalse((self.root / 'backup').exists())
        (data / 'secret.pocol-key').write_bytes(b'not-a-real-key')
        with self.assertRaises(ValueError): op.backup(data, self.root / 'backup')
        (data / 'secret.pocol-key').unlink()
        report = op.backup(data, self.root / 'backup')
        self.assertFalse(report['keysIncluded'])
        self.assertEqual((self.root / 'backup/state/blocks.journal').read_bytes(), b'fixture\0\xff')
        self.assertEqual(set(op.files(self.root / 'backup/state')), {'network-binding.json','blocks.journal','blocks.commit'})
        with self.assertRaises(ValueError): op.backup(data, self.root / 'backup')
        with self.assertRaises(ValueError): op.backup(data, data / 'child')

    def test_public_build_digest_recomputed_independently(self):
        binary = prepare.ROOT / 'prototypes/pocold/target/release/pocold'
        info = json.loads(subprocess.check_output([str(binary), 'build-info'], text=True))
        h = hashlib.sha256(b'pocol-testnet-source-compatibility-v1\0')
        for name, sha in sorted(info['sourceSha256'].items()):
            data = (prepare.ROOT / 'prototypes' / name).read_bytes(); path = name.encode()
            self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
            h.update(len(path).to_bytes(8,'big')); h.update(path)
            h.update(len(data).to_bytes(8,'big')); h.update(data)
        h.update(','.join(info['features']).encode())
        self.assertEqual(h.hexdigest(), info['compatibilitySha256'])
        self.assertEqual(info['features'], []); self.assertEqual(info['protocolVersion'], 2)
        self.assertIn('lp4-execution/tests/data/system-v1.json', info['sourceSha256'])
        # The fields node_operator and prepare bind a bundle to (VA-SPEC §2): a pinned release build of rules 2 (the v3
        # release rules).
        self.assertEqual((info['consensusRulesVersion'], info['protocolVersionMin'], info['maxForkVersion']), (2, 2, 2))
        self.assertIs(info['consensusPinned'], True, 're-pin the consensus lock (pocold/tests/consensus_guard.rs)')

    def bundle(self, name='bundle', digest='b'*64, forks=None):
        bundle = self.root / name; bundle.mkdir(); (bundle / 'net').mkdir(); (bundle / 'bin').mkdir()
        config = {'name':'n1', 'minerAddress':None, 'listen':'0.0.0.0:30300', 'rpc':'127.0.0.1:8545', 'peers':[]}
        info = {'chainId':777915, 'genesisHash':'0x'+'a'*64, 'admission':{'scope':'experimental-devnet'}}
        if forks is not None: info['forks'] = forks
        build = {'features':[], 'protocolVersion':2, 'compatibilitySha256':digest, 'consensusRulesVersion':2,
                 'consensusPinned':True, 'consensusSourcesSha256':'c'*64}
        op.write_json(bundle / 'node.json', config); op.write_json(bundle / 'net/devnet.json', info)
        op.write_json(bundle / 'build-info.json', build)
        for name in ['net/genesis.spec.hex','net/alloc.json','node_operator.py','pocol_identity.py','pocol-testnet.service'] + ['bin/'+n for n in op.BINARIES]:
            (bundle / name).write_bytes(b'public fixture')
        binding = op.identity(config, info, build)
        op.write_json(bundle / 'manifest.json', {'schema':op.SCHEMA, 'identity':binding,
            'sha256':{name:op.digest(path) for name,path in op.files(bundle).items()}})
        return bundle, binding

    def test_restore_rejects_tampering_wrong_identity_and_existing_target(self):
        bundle, binding = self.bundle(); data = self.state()
        (data / 'network-binding.json').unlink(); op.write_json(data / 'network-binding.json', binding)
        saved = self.root / 'backup'; report = op.backup(data, saved)
        target = self.root / 'restored'; expected = report['backupManifestSha256']
        with self.assertRaises(ValueError): op.restore(bundle, saved, target, '0'*64)
        (saved / 'state/blocks.journal').write_bytes(b'tampered')
        with self.assertRaises(ValueError): op.restore(bundle, saved, target, expected)
        (saved / 'state/blocks.journal').write_bytes(b'fixture\0\xff')
        manifest = op.read_json(saved / 'backup.json'); manifest['identity']['miner'] = '0x'+'1'*40
        (saved / 'backup.json').unlink(); op.write_json(saved / 'backup.json', manifest)
        with self.assertRaises(ValueError): op.restore(bundle, saved, target, op.digest(saved / 'backup.json'))
        manifest['identity'] = binding; (saved / 'backup.json').unlink(); op.write_json(saved / 'backup.json', manifest)
        op.restore(bundle, saved, target, op.digest(saved / 'backup.json'))
        with self.assertRaises(ValueError): op.restore(bundle, saved, target, op.digest(saved / 'backup.json'))
        self.assertEqual((target / 'blocks.journal').read_bytes(), b'fixture\0\xff')

    def test_a_digest_only_bundle_swap_starts_on_the_existing_marker_without_migration(self):
        old, before = self.bundle('old', 'b' * 64)
        swap, swapped = self.bundle('swap', 'd' * 64)
        fork, forked = self.bundle('fork', 'b' * 64, forks=[[1, 0], [2, 100]])
        self.assertNotEqual(swapped['compatibilitySha256'], before['compatibilitySha256'])
        self.assertEqual(swapped['consensusIdentity'], before['consensusIdentity'], 'same rules and schedule')
        self.assertNotEqual(forked['consensusIdentity'], before['consensusIdentity'], 'a fork entry is another identity')
        self.assertTrue(op.bound(before, swapped), 'a digest-only release binds the existing marker')
        self.assertFalse(op.bound(before, forked))
        data = self.root / 'state'; data.mkdir(mode=0o700)
        op.write_json(data / 'network-binding.json', before)
        marker = (data / 'network-binding.json').read_bytes()
        def build_info(bundle):
            return lambda args, **kwargs: json.dumps(op.read_json(bundle / 'build-info.json'))
        with unittest.mock.patch.object(op.subprocess, 'check_output', side_effect=build_info(swap)), \
                unittest.mock.patch.object(op.os, 'execv', side_effect=SystemExit('started')) as started:
            with self.assertRaisesRegex(SystemExit, 'started'): op.launch(swap, data, None)
        self.assertEqual(started.call_args.args[1][1], 'run')
        self.assertEqual(started.call_args.args[1][0], str(swap / 'bin/pocold'))
        self.assertEqual((data / 'network-binding.json').read_bytes(), marker, 'the marker is never rewritten')
        # Another fork schedule is another consensus identity: refused, and nothing starts.
        with unittest.mock.patch.object(op.subprocess, 'check_output', side_effect=build_info(fork)), \
                unittest.mock.patch.object(op.os, 'execv') as started:
            with self.assertRaisesRegex(ValueError, 'another network, consensus rules'): op.launch(fork, data, None)
            started.assert_not_called()
        self.assertEqual((data / 'network-binding.json').read_bytes(), marker)

    def test_monitor_rejects_unsafe_urls_before_any_io(self):
        for url in ('http://example.test', 'http://169.254.169.254:80', 'file:///etc/passwd',
                    'https://user:password@example.test', 'https://example.test/redirect', 'https://example.test/?key=secret'):
            with self.subTest(url=url), self.assertRaises(ValueError): monitor.rpc(url, 'pocol_status')

    def test_service_units_hold_the_thread_inventory_at_the_default_inbound_settings(self):
        # PR3: 512 tasks hold pocold's thread inventory up to --peers-max 128 with 7 peers (340); seeds stay at the
        # default 32 established inbound (the live-chain halt rule), so neither a seed's nor a joining node's unit sets
        # the inbound settings.
        seed = inventory()['nodes'][0]
        join = dict(seed, seeds=['127.0.0.8:30300'])
        for name, node in (('seed', seed), ('join', join)):
            unit = prepare.service(node)
            with self.subTest(name):
                self.assertEqual([l for l in unit.splitlines() if l.startswith('TasksMax=')], ['TasksMax=512'])
                self.assertNotIn('POCOL_PEERS_MAX', unit); self.assertNotIn('POCOL_PER_GROUP_MAX', unit)

    def test_monitor_requires_one_consensus_identity_and_only_warns_on_a_mixed_build_digest(self):
        release = {'chainId': 777915, 'genesisHash': '0x' + 'a' * 64, 'consensusIdentity': 'c' * 64,
                   'protocolVersion': 2, 'protocolVersionMin': 2, 'compatibilitySha256': 'b' * 64}
        def status(name, **change):
            return {**{k: release[k] for k in ('chainId', 'genesisHash', 'consensusIdentity', 'compatibilitySha256')},
                    'protocolVersion': 2, 'protocolVersionMin': 2, 'name': name, 'stateStore': True,
                    'admission': {'scope': 'experimental-devnet'}, 'compatiblePeers': 1,
                    'head': {'height': 10}, 'lifetime': {'closingHeight': 10**6}, 'stats': {'historyDamaged': False}, **change}
        urls = {'n1': 'https://n1.example.test', 'n2': 'https://n2.example.test'}
        def observe(**change):
            nodes = {'https://n1.example.test': status('n1'), 'https://n2.example.test': status('n2', **change)}
            warnings = []
            with unittest.mock.patch.object(monitor, 'rpc', side_effect=lambda url, method: nodes[url]):
                return monitor.snapshot(release, urls, warnings), warnings
        self.assertEqual(observe()[1], [])
        values, warnings = observe(compatibilitySha256='d' * 64)
        self.assertEqual(set(values), {'n1', 'n2'})
        self.assertEqual(len(warnings), 1); self.assertIn('n2', warnings[0])
        for change in ({'consensusIdentity': 'e' * 64}, {'chainId': 7}, {'protocolVersion': 1}, {'protocolVersionMin': 3}):
            with self.subTest(change=change), self.assertRaises(ValueError): observe(**change)
        # A continuous chain reports closingHeight null (pocold node.rs, H_END = 0) and is not near any closing height.
        continuous = {'continuous': True, 'closingHeight': None, 'finalHeight': None, 'closingTime': None, 'finalTime': None}
        self.assertEqual(set(observe(lifetime=continuous)[0]), {'n1', 'n2'})
        for lifetime in ({'continuous': False, 'closingHeight': 105, 'finalHeight': 200}, {'continuous': False, 'closingHeight': None},
                         {'closingHeight': None}, {'continuous': True, 'closingHeight': 10**6}):
            with self.subTest(lifetime=lifetime), self.assertRaises(ValueError): observe(lifetime=lifetime)

    def test_prepared_release_names_the_consensus_identity_of_its_network(self):
        build = {'consensusRulesVersion': 2, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': 'aa' * 32}
        info = {'chainId': 777915, 'genesisHash': '0x' + '01' * 32, 'forks': [[1, 0], [2, 50]]}
        manifest = op.identity({'name': 'n1', 'minerAddress': None}, info, build | {'protocolVersion': 2, 'compatibilitySha256': 'b' * 64})
        self.assertEqual(manifest['consensusIdentity'], prepare.pocol_identity.consensus_id(777915, info['genesisHash'], 2, 0, [(1, 0), (2, 50)]))
        self.assertEqual(manifest['consensusRulesVersion'], 2)
        self.assertTrue(op.bound(dict(manifest), manifest))
        # A marker without a consensus identity (an older state binding) never binds a node.
        self.assertFalse(op.bound({k: v for k, v in manifest.items() if k not in ('consensusIdentity', 'consensusRulesVersion')}, manifest))

    def test_duplicate_json_keys_are_refused(self):
        path = self.root / 'bad.json'; path.write_text('{"chainId":1,"chainId":2}')
        with self.assertRaises(ValueError): op.read_json(path)


if __name__ == '__main__': unittest.main()
