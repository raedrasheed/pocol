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
import pocol_identity

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

    def bundle(self, name, digest, activation=None, legacy=False):
        """A seed bundle of an R1 release (pinned, rules 1), or with legacy=True of a 0.4.0 one (no rules)."""
        bundle = self.root / name; bundle.mkdir(); (bundle / 'net').mkdir(); (bundle / 'bin').mkdir()
        config = {'name': 'seed1', 'minerAddress': '0x' + '11' * 20,
                  'listen': '0.0.0.0:30300', 'rpc': '127.0.0.1:8545', 'peers': ['192.0.2.5:30300']}
        info = {'chainId': 777915, 'genesisHash': '0x' + GENESIS.hex(), 'keyMode': 'independent-key-files',
                'admission': {'scope': 'experimental-devnet'}, 'forks': [[1, 0]] + ([[2, activation]] if activation else [])}
        build = {'schema': 'pocol-testnet-build-v1', 'features': [], 'protocolVersion': 2,
                 'compatibilitySha256': digest}
        if not legacy:
            build.update(consensusPinned=True, consensusRulesVersion=1, consensusSourcesSha256='cd' * 32,
                         maxForkVersion=2, protocolVersionMin=2)
        op.write_json(bundle / 'node.json', config); op.write_json(bundle / 'net/devnet.json', info)
        op.write_json(bundle / 'build-info.json', build)
        for filename in ['node_operator.py', 'pocol-testnet.service', 'nginx.conf', 'README.md',
                         'net/genesis.spec.hex', 'net/alloc.json'] + ['bin/' + n for n in op.BINARIES] + ([] if legacy else ['pocol_identity.py']):
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
        self.assertEqual(op.read_json(self.data / upgrade.backup_name(self.before)), self.before)
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
        with self.assertRaisesRegex(ValueError, 'existing network and its fork schedule'): self.migrate()

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
        backup = self.data / upgrade.backup_name(self.before)
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
        invitation = {'schema': 'pocol-network-v2', 'consensusRulesVersion': 1,
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

    def test_build_validation_rejects_changed_local_crypto_dependency(self):
        source = self.root / 'source-vendor'
        vendor = 'third-party/evm-crypto/alloy-primitives-0.8.26/src/bits/address.rs'
        sources = {'pocold/src/chain.rs': b'consensus fixture',
                   'lp1-node/src/profile.rs': b'profile fixture', vendor: b'public recovery fixture'}
        digest = hashlib.sha256(b'pocol-testnet-source-compatibility-v1\0')
        for name, content in sorted(sources.items()):
            path = source / 'prototypes' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            encoded = name.encode()
            digest.update(len(encoded).to_bytes(8, 'big')); digest.update(encoded)
            digest.update(len(content).to_bytes(8, 'big')); digest.update(content)
        build = {**self.build, 'compatibilitySha256': digest.hexdigest(),
                 'sourceSha256': {name: hashlib.sha256(content).hexdigest()
                                  for name, content in sources.items()}}
        with patch.object(upgrade, 'ROOT', source):
            upgrade.validate_build(build)
            (source / 'prototypes' / vendor).write_bytes(b'changed recovery code')
            with self.assertRaisesRegex(ValueError, 'Rebuild stale native release: third-party/'):
                upgrade.validate_build(build)


    def certified(self, digest):
        """A LEGACY_BUILDS mirror that certifies `digest` on this fixture's chain (shipped: nothing certified)."""
        entry = {'digest': digest, 'rules': 1, 'chainId': 777915, 'genesisHash': '0x' + GENESIS.hex(),
                 'sources': None, 'certified': 'development/decisions/LEGACY-CERTIFICATION-0.4.0.md'}
        return patch.object(pocol_identity, 'LEGACY_BUILDS', (entry,))

    def test_a_second_migration_keeps_the_full_nonce_backup_and_writes_its_own(self):
        # What the 0.4.0 tool left after its full-nonce migration: its fixed backup and a finished intent.
        legacy_backup = self.data / upgrade.LEGACY_BACKUP
        legacy_backup.write_text('{"left": "by the 0.4.0 migration"}\n')
        op.write_json(self.data / upgrade.INTENT, {'schema': 'pocol-full-nonce-seed-upgrade-v1', 'after': self.before})
        self.migrate()
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.after)
        # A later release of the same rules and schedule: a rebind, with its own backup of the replaced marker.
        newer = self.bundle('newer', 'c' * 64, activation=100)
        with patch.object(upgrade.subprocess, 'check_output', return_value=(newer / 'build-info.json').read_text()):
            upgrade.migrate_binding(self.new, newer, self.data, op.digest(newer / 'manifest.json'))
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), op.read_json(newer / 'manifest.json')['identity'])
        self.assertEqual(op.read_json(self.data / upgrade.backup_name(self.before)), self.before)
        self.assertEqual(op.read_json(self.data / upgrade.backup_name(self.after)), self.after)
        self.assertNotEqual(upgrade.backup_name(self.before), upgrade.backup_name(self.after))
        self.assertEqual(legacy_backup.read_text(), '{"left": "by the 0.4.0 migration"}\n')

    def test_a_zero_append_rebind_migrates_the_marker_and_skips_the_height_check(self):
        rebind = self.bundle('rebind', 'b' * 64)
        self.journal([1, 2, 500, 3])
        original = self.unchanged_state()
        report = upgrade.migrate_binding(self.old, rebind, self.data, op.digest(rebind / 'manifest.json'))
        self.assertEqual(report['appended'], [])
        after = op.read_json(rebind / 'manifest.json')['identity']
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), after)
        self.assertEqual(after['consensusIdentity'], self.before['consensusIdentity'], 'same rules and schedule')
        self.assertNotEqual(after['compatibilitySha256'], self.before['compatibilitySha256'])
        self.assertEqual(self.unchanged_state(), original)

    def test_fork_transition_appends_any_future_entries_and_never_changes_one(self):
        before = op.read_json(self.old / 'net/devnet.json')
        self.assertEqual(upgrade.fork_transition(before, before), [])
        two = {**before, 'forks': [[1, 0], [2, 100]]}
        self.assertEqual(upgrade.fork_transition(before, two), [[2, 100]])
        self.assertEqual(upgrade.fork_transition(two, two), [])
        with patch.object(pocol_identity, 'MAX_FORK_VERSION', 3):
            self.assertEqual(upgrade.fork_transition(two, {**two, 'forks': [[1, 0], [2, 100], [3, 200]]}), [[3, 200]])
            self.assertEqual(upgrade.fork_transition(before, {**two, 'forks': [[1, 0], [2, 100], [3, 200]]}), [[2, 100], [3, 200]])
        for forks in ([[1, 0]], [[1, 0], [2, 101]], [[1, 0], [2, 100], [3, 200]]):
            with self.subTest(forks=forks), self.assertRaises(ValueError):
                upgrade.fork_transition(two, {**two, 'forks': forks})

    def test_a_digest_only_bundle_swap_starts_on_the_existing_marker_without_migration(self):
        swap = self.bundle('swap', 'b' * 64)
        key = self.root / 'miner.pocol-key'; key.write_text('not read by this test')
        build = op.read_json(swap / 'build-info.json')
        def run(args, **kwargs):
            return '0x' + '11' * 20 + '\n' if args[1] == 'address' else json.dumps(build)
        marker = (self.data / 'network-binding.json').read_bytes()
        with patch.object(op.subprocess, 'check_output', side_effect=run), \
                patch.object(op.os, 'execv', side_effect=SystemExit('started')) as started:
            with self.assertRaisesRegex(SystemExit, 'started'): op.launch(swap, self.data, key)
        self.assertEqual(started.call_args.args[1][1], 'run')
        self.assertEqual((self.data / 'network-binding.json').read_bytes(), marker, 'the marker is never rewritten')
        # Another schedule is another consensus identity: refused.
        with patch.object(op.subprocess, 'check_output', side_effect=run), patch.object(op.os, 'execv') as started:
            with self.assertRaisesRegex(ValueError, 'another network, consensus rules'): op.launch(self.new, self.data, key)
            started.assert_not_called()

    def test_migration_after_a_digest_only_swap_starts_on_the_kept_marker(self):
        # The installed bundle is a digest-only swap: launch() accepted the marker and kept its older build digest.
        swap = self.bundle('swap', 'c' * 64)
        installed = op.read_json(swap / 'manifest.json')['identity']
        self.assertNotEqual(installed, self.before)
        self.assertTrue(op.bound(self.before, installed))
        marker = (self.data / 'network-binding.json').read_bytes()
        expected = op.digest(self.new / 'manifest.json')
        real_write = upgrade.atomic_public_write
        def interrupted(path, content, owner):
            if path.name == 'network-binding.json': raise OSError('simulated interruption')
            real_write(path, content, owner)
        with patch.object(upgrade, 'atomic_public_write', side_effect=interrupted):
            with self.assertRaises(OSError): upgrade.migrate_binding(swap, self.new, self.data, expected)
        self.assertEqual((self.data / 'network-binding.json').read_bytes(), marker)
        intent = op.read_json(self.data / upgrade.INTENT)
        self.assertEqual(intent['before'], self.before, 'the intent records the actual starting marker')
        self.assertEqual(intent['oldManifestSha256'], op.digest(swap / 'manifest.json'))
        upgrade.migrate_binding(swap, self.new, self.data, expected)
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.after)
        self.assertEqual((self.data / upgrade.backup_name(self.before)).read_bytes(), marker,
                         'the backup keeps the original marker bytes')
        self.assertFalse((self.data / upgrade.backup_name(installed)).exists())
        self.assertTrue(upgrade.migrate_binding(swap, self.new, self.data, expected)['bindingMigrated'])
        # A marker of another node is still refused from the same installed bundle.
        for name in ('network-binding.json', upgrade.INTENT): (self.data / name).unlink()
        op.write_json(self.data / 'network-binding.json', {**self.before, 'miner': None})
        with self.assertRaisesRegex(ValueError, 'does not match the old seed bundle'):
            upgrade.migrate_binding(swap, self.new, self.data, expected)

    def test_a_legacy_marker_is_accepted_only_through_a_certified_build(self):
        old = self.bundle('legacy', 'a' * 64, legacy=True)
        legacy = op.read_json(old / 'manifest.json')['identity']
        self.assertNotIn('consensusIdentity', legacy)
        op.write_json(self.data / 'legacy-marker.json', legacy)
        expected = op.read_json(self.new / 'manifest.json')['identity']
        self.assertTrue(op.bound(legacy, legacy), 'its own 0.4.0 release still starts')
        self.assertFalse(op.bound(legacy, expected), 'uncertified as shipped')
        (self.data / 'network-binding.json').unlink(); op.write_json(self.data / 'network-binding.json', legacy)
        with self.assertRaisesRegex(ValueError, 'not certified'):
            upgrade.migrate_binding(old, self.new, self.data, op.digest(self.new / 'manifest.json'))
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), legacy)
        with self.certified('a' * 64):
            self.assertTrue(op.bound(legacy, expected))
            self.assertFalse(op.bound({**legacy, 'miner': None}, expected), 'node and miner still bind')
            self.assertFalse(op.bound({**legacy, 'genesisHash': '0x' + '00' * 32}, expected))
            upgrade.migrate_binding(old, self.new, self.data, op.digest(self.new / 'manifest.json'))
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), expected)
        with self.certified('f' * 64): self.assertFalse(op.bound(legacy, expected), 'another digest')

    def test_prepare_refuses_a_v1_invitation_unless_certified_and_another_rules_revision(self):
        public = {name: (self.new / 'net' / name).read_text() for name in ('genesis.spec.hex', 'alloc.json', 'devnet.json')}
        for invitation in ({'schema': 'pocol-network-v1', 'compatibilitySha256': self.build['compatibilitySha256'], 'seeds': [], 'files': public},
                           {'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': [], 'files': public},
                           {'schema': 'pocol-network-v2', 'compatibilitySha256': 'a' * 64, 'seeds': [], 'files': public}):
            network = self.root / 'invitation.json'; network.write_text(json.dumps(invitation))
            with self.subTest(invitation=invitation['schema']), patch.object(upgrade, 'validate_build'), \
                    self.assertRaisesRegex(ValueError, 'consensus rules|Invalid new invitation'):
                upgrade.prepare_bundle(self.old, self.new / 'bin', network, self.root / 'refused', op.digest(self.old / 'manifest.json'))
            self.assertFalse((self.root / 'refused').exists())

    def test_build_validation_requires_a_pinned_release_with_rules(self):
        source = self.root / 'source-pinned'
        content = b'consensus fixture'
        for name in ('pocold/src/chain.rs', 'lp1-node/src/profile.rs'):
            (source / 'prototypes' / name).parent.mkdir(parents=True, exist_ok=True)
            (source / 'prototypes' / name).write_bytes(content)
        digest = hashlib.sha256(b'pocol-testnet-source-compatibility-v1\0')
        for name in sorted(('pocold/src/chain.rs', 'lp1-node/src/profile.rs')):
            digest.update(len(name).to_bytes(8, 'big')); digest.update(name.encode())
            digest.update(len(content).to_bytes(8, 'big')); digest.update(content)
        build = {**self.build, 'compatibilitySha256': digest.hexdigest(), 'sourceSha256': {
            name: hashlib.sha256(content).hexdigest() for name in ('pocold/src/chain.rs', 'lp1-node/src/profile.rs')}}
        with patch.object(upgrade, 'ROOT', source):
            upgrade.validate_build(build)
            for change in ({'consensusPinned': False}, {'consensusRulesVersion': None}, {'consensusRulesVersion': 0}):
                with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'pinned consensus'):
                    upgrade.validate_build({**build, **change})


if __name__ == '__main__':
    unittest.main()
