"""Offline seed upgrade tests with authenticated public bundle and journal fixtures."""
import copy
import fcntl
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import node_operator as op
import nonce_upgrade as upgrade

GENESIS = bytes.fromhex('ab' * 32)


def encode(value):
    if isinstance(value, int):
        value = value.to_bytes((value.bit_length() + 7) // 8, 'big')
    listed = isinstance(value, list)
    body = b''.join(encode(x) for x in value) if listed else value
    short, long = (192, 247) if listed else (128, 183)
    if not listed and len(body) == 1 and body[0] < 128:
        return body
    if len(body) < 56:
        return bytes([short + len(body)]) + body
    size = len(body).to_bytes((len(body).bit_length() + 7) // 8, 'big')
    return bytes([long + len(size)]) + size + body


def record(height, version=1):
    unsigned = [b'PoCol-tpl-v1', 777915, GENESIS, bytes(32), height, 0, version,
                1000 + height, 2**256 - 1, bytes(32), bytes(32), bytes(32), bytes(256),
                8000000, 0, 7, bytes(32), b'']
    header = encode([[unsigned, b''], bytes(8), [], bytes(65)])
    payload = encode([header, encode([[], []]), b''])
    size = len(payload).to_bytes(4, 'big')
    return size + hashlib.sha256(b'POCOL-REC-LEN' + size).digest()[:4] + hashlib.sha256(payload).digest() + payload + b'\1'


class SeedUpgradeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.old = self.bundle('old', 'a' * 64)
        self.new = self.bundle('new', 'b' * 64, activation=100)
        self.data = self.root / 'state'; self.data.mkdir(mode=0o700)
        self.before = op.read_json(self.old / 'manifest.json')['identity']
        self.after = op.read_json(self.new / 'manifest.json')['identity']
        op.write_json(self.data / 'network-binding.json', self.before)
        (self.data / 'history.redb').write_bytes(b'unchanged history')
        (self.data / 'state.redb').write_bytes(b'unchanged state')
        self.journal([1, 2, 20, 3])
        self.build = op.read_json(self.new / 'build-info.json')
        self.binary = patch.object(upgrade.subprocess, 'check_output', return_value=json.dumps(self.build))
        self.binary.start(); self.addCleanup(self.binary.stop)

    def bundle(self, name, digest, activation=None):
        bundle = self.root / name; bundle.mkdir(); (bundle / 'net').mkdir(); (bundle / 'bin').mkdir()
        config = {'name': 'seed1', 'minerAddress': '0x' + '11' * 20,
                  'listen': '0.0.0.0:30300', 'rpc': '127.0.0.1:8545', 'peers': ['192.0.2.5:30300']}
        info = {'chainId': 777915, 'genesisHash': '0x' + GENESIS.hex(), 'keyMode': 'independent-key-files',
                'admission': {'scope': 'experimental-devnet'}, 'forks': [[1, 0]] + ([[2, activation]] if activation else [])}
        build = {'schema': 'pocol-testnet-build-v1', 'features': [], 'protocolVersion': 2,
                 'compatibilitySha256': digest}
        op.write_json(bundle / 'node.json', config); op.write_json(bundle / 'net/devnet.json', info)
        op.write_json(bundle / 'build-info.json', build)
        for filename in ['node_operator.py', 'pocol-testnet.service', 'nginx.conf', 'README.md',
                         'net/genesis.spec.hex', 'net/alloc.json'] + ['bin/' + n for n in op.BINARIES]:
            (bundle / filename).write_bytes(b'public fixture\n')
        self.manifest(bundle)
        return bundle

    def manifest(self, bundle):
        path = bundle / 'manifest.json'
        if path.exists(): path.unlink()
        info = op.read_json(bundle / 'net/devnet.json')
        identity = op.identity(op.read_json(bundle / 'node.json'), info, op.read_json(bundle / 'build-info.json'))
        op.write_json(path, {'schema': op.SCHEMA, 'rehearsalOnly': True, 'identity': identity,
                            'sha256': {name: op.digest(path) for name, path in op.files(bundle).items()}})
        return op.digest(path)

    def journal(self, heights, extra=None, version=1):
        content = b'POCOLJ2\n' + GENESIS + b''.join(record(h, version) for h in heights)
        values = (1).to_bytes(8, 'big') + len(heights).to_bytes(8, 'big') + len(content).to_bytes(8, 'big')
        slot = values + hashlib.sha256(b'POCOL-COMMIT' + GENESIS + values).digest() + bytes(8)
        (self.data / 'blocks.commit').write_bytes(bytes(4096) + slot)
        (self.data / 'blocks.journal').write_bytes(content + (record(extra) if extra is not None else b''))

    def migrate(self):
        return upgrade.migrate_binding(self.old, self.new, self.data, op.digest(self.new / 'manifest.json'))

    def unchanged_state(self):
        return {name: (self.data / name).read_bytes() for name in
                ('blocks.journal', 'blocks.commit', 'history.redb', 'state.redb')}

    def test_migration_preserves_all_chain_bytes_and_binding_ownership(self):
        original = self.unchanged_state()
        metadata = (self.data / 'network-binding.json').stat()
        report = self.migrate()
        self.assertEqual(report['highestStoredBlock'], '20')
        self.assertFalse(report['chainStateChanged']); self.assertFalse(report['serviceStarted'])
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.after)
        self.assertEqual(op.read_json(self.data / upgrade.BACKUP), self.before)
        self.assertEqual(self.unchanged_state(), original)
        actual = (self.data / 'network-binding.json').stat()
        self.assertEqual((actual.st_uid, actual.st_gid), (metadata.st_uid, metadata.st_gid))
        self.assertEqual(actual.st_mode & 0o777, 0o600)
        self.assertTrue(self.migrate()['bindingMigrated'])

    def test_locks_prevent_migration_while_either_owner_is_running(self):
        for name in ('.operator.lock', 'LOCK'):
            with (self.data / name).open('a') as handle:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                with self.assertRaises(BlockingIOError): self.migrate()
            self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.before)

    def test_activation_must_follow_every_branch_and_uncommitted_complete_frame(self):
        for heights, extra in (([1, 100, 3], None), ([1, 20], 100)):
            with self.subTest(heights=heights, extra=extra):
                self.journal(heights, extra)
                original = self.unchanged_state()
                with self.assertRaisesRegex(ValueError, 'above every stored'): self.migrate()
                self.assertEqual(self.unchanged_state(), original)
                self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.before)

    def test_bad_manifest_wrong_node_or_changed_genesis_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'manifest identity'):
            upgrade.migrate_binding(self.old, self.new, self.data, '0' * 64)
        for name in ('node.json', 'net/genesis.spec.hex', 'net/alloc.json', 'pocol-testnet.service'):
            path = self.new / name; original = path.read_bytes()
            path.write_bytes(json.dumps({'name': 'other', 'minerAddress': None}).encode() if name == 'node.json' else b'changed')
            self.manifest(self.new)
            with self.subTest(name=name), self.assertRaises(ValueError): self.migrate()
            path.write_bytes(original); self.manifest(self.new)
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.before)

    def test_corrupt_torn_or_wrong_version_journal_and_commit_are_not_repaired(self):
        original = self.unchanged_state()
        mutations = [('blocks.journal', original['blocks.journal'][:-1]),
                     ('blocks.journal', original['blocks.journal'][:-2] + b'xx'),
                     ('blocks.commit', bytes(4160)),
                     ('blocks.commit', original['blocks.commit'][:-10] + b'x' * 10)]
        for name, content in mutations:
            with self.subTest(name=name):
                (self.data / name).write_bytes(content)
                before = self.unchanged_state()
                with self.assertRaises(ValueError): self.migrate()
                self.assertEqual(self.unchanged_state(), before)
                (self.data / name).write_bytes(original[name])
        self.journal([1, 2], version=2)
        with self.assertRaisesRegex(ValueError, 'existing v1 network'): self.migrate()

    def test_new_binary_must_match_verified_build_metadata(self):
        original = self.unchanged_state()
        with patch.object(upgrade.subprocess, 'check_output', return_value=json.dumps({**self.build, 'compatibilitySha256': 'c' * 64})):
            with self.assertRaisesRegex(ValueError, 'build identity differ'): self.migrate()
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.before)
        self.assertEqual(self.unchanged_state(), original)
        self.assertFalse((self.data / upgrade.INTENT).exists())

    def test_fork_metadata_rejects_boolean_versions_downgrades_and_other_changes(self):
        before = op.read_json(self.old / 'net/devnet.json')
        after = op.read_json(self.new / 'net/devnet.json')
        for schedule in ([[True, 0], [2, 100]], [[1, False], [2, 100]],
                         [[1, 0], [2, True]], [[1, 0], [2, 100], [1, 101]],
                         [[1, 0], [3, 100]], [[1, 0], [2, 2**64]]):
            with self.subTest(schedule=schedule), self.assertRaises(ValueError):
                upgrade.fork_transition(before, {**after, 'forks': schedule})
        with self.assertRaises(ValueError):
            upgrade.fork_transition(before, {**after, 'chainId': 7})

    def test_interrupted_binding_write_resumes_only_the_same_plan(self):
        real_write = upgrade.atomic_public_write
        def interrupted(path, content, owner):
            if path.name == 'network-binding.json':
                path.with_name(path.name + '.nonce-upgrade.tmp').write_bytes(content[:12])
                raise OSError('simulated interruption')
            real_write(path, content, owner)
        with patch.object(upgrade, 'atomic_public_write', side_effect=interrupted):
            with self.assertRaises(OSError): self.migrate()
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.before)
        self.assertTrue((self.data / upgrade.INTENT).is_file())
        self.assertTrue(self.migrate()['bindingMigrated'])
        changed = op.read_json(self.new / 'net/devnet.json'); changed['forks'][1][1] = 101
        (self.new / 'net/devnet.json').write_text(json.dumps(changed)); self.manifest(self.new)
        with self.assertRaisesRegex(ValueError, 'exactly the same'): self.migrate()

    def test_symlinks_and_unexpected_binding_backup_are_preserved(self):
        backup = self.data / upgrade.BACKUP
        backup.write_text('{"unexpected":true}')
        with self.assertRaisesRegex(ValueError, 'backup differs'): self.migrate()
        self.assertEqual(backup.read_text(), '{"unexpected":true}')
        backup.unlink(); backup.symlink_to(self.data / 'network-binding.json')
        with self.assertRaisesRegex(ValueError, 'single-link'): self.migrate()

    def test_hardlinked_upgrade_temp_is_not_truncated(self):
        protected = self.data / 'state.redb'
        original = protected.read_bytes()
        target = self.data / 'network-binding.json'
        temporary = target.with_name(target.name + '.nonce-upgrade.tmp')
        os.link(protected, temporary)
        owner = (target.stat().st_uid, target.stat().st_gid)
        with self.assertRaisesRegex(ValueError, 'Invalid upgrade temporary'):
            upgrade.atomic_public_write(target, b'new public binding', owner)
        self.assertEqual(protected.read_bytes(), original)
        self.assertEqual(temporary.read_bytes(), original)
        self.assertEqual(op.read_json(target), self.before)

    def test_prepare_preserves_node_service_and_network_inputs(self):
        source = self.root / 'source'; (source / 'prototypes/pocold/src').mkdir(parents=True)
        (source / 'prototypes/lp1-node/src').mkdir(parents=True)
        sources = {'pocold/src/chain.rs': b'new consensus', 'lp1-node/src/profile.rs': b'v2 schedule'}
        digest = hashlib.sha256(b'pocol-testnet-source-compatibility-v1\0')
        for name, content in sorted(sources.items()):
            (source / 'prototypes' / name).write_bytes(content); encoded = name.encode()
            digest.update(len(encoded).to_bytes(8, 'big')); digest.update(encoded)
            digest.update(len(content).to_bytes(8, 'big')); digest.update(content)
        build = {**self.build, 'sourceSha256': {name: hashlib.sha256(content).hexdigest() for name, content in sources.items()},
                 'compatibilitySha256': digest.hexdigest()}
        invitation = {'schema': 'pocol-network-v1', 'compatibilitySha256': digest.hexdigest(),
                      'seeds': ['192.0.2.5:30300'], 'files': {
                          name: (self.new / 'net' / name).read_text()
                          for name in ('genesis.spec.hex', 'alloc.json', 'devnet.json')}}
        network = self.root / 'network.pocol.json'; network.write_text(json.dumps(invitation))
        target = self.root / 'prepared'
        original = {name: path.read_bytes() for name, path in op.files(self.old).items()}
        with patch.object(upgrade, 'ROOT', source), patch.object(upgrade.subprocess, 'check_output', return_value=json.dumps(build)):
            result = upgrade.prepare_bundle(self.old, self.new / 'bin', network, target, op.digest(self.old / 'manifest.json'))
        self.assertFalse(result['deploymentPerformed'])
        self.assertEqual({name: path.read_bytes() for name, path in op.files(self.old).items()}, original)
        for name in ('node.json', 'pocol-testnet.service', 'nginx.conf', 'net/genesis.spec.hex', 'net/alloc.json'):
            self.assertEqual((target / name).read_bytes(), original[name])
        op.verify(target, result['bundleManifestSha256'])
        with self.assertRaisesRegex(ValueError, 'outside'):
            upgrade.prepare_bundle(self.old, self.new / 'bin', network, self.old / 'nested')


if __name__ == '__main__':
    unittest.main()
