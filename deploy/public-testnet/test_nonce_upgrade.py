"""Offline seed fork-migration tests with authenticated public bundle and journal fixtures.

The bundles run the v3 genesis of the spec-2 decoder corpus (as test_operations.py does); the fork entries they append
are fixtures of the published schedule append (VA-SPEC §2.3), never a network's real heights."""
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

CORPUS = Path(__file__).resolve().parents[2] / 'prototypes/pocol-extension/tests/data'
V3_HEX = '0x' + next(item['hex'] for item in json.loads((CORPUS / 'decoder-corpus-v3.json').read_text())['items']
                     if item['label'] == 'v3/base')
CHAIN = pocol_identity.decode_genesis_spec(bytes.fromhex(V3_HEX[2:]))['chainId']
GENESIS = bytes.fromhex('ab' * 32)
NOTICE = upgrade.MIN_NOTICE_BLOCKS
# The fixture activation: far enough above the fixture journal's highest block (20) for the minimum notice.
ACTIVATION = 20000
RAISE = {'ACCT_CAP': 2**40, 'SLOT_CAP': 2**40, 'CODE_CAP': 2**40, 'SYS_SLOT_MAX': 2**20}


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


def record(height, version=1, first_import=False):
    unsigned = [b'PoCol-tpl-v1', CHAIN, GENESIS, bytes(32), height, 0, version,
                1000 + height, 2**256 - 1, bytes(32), bytes(32), bytes(32), bytes(256),
                8000000, 0, 7, bytes(32), b'']
    header = encode([[unsigned, b''], bytes(8), [], bytes(65)])
    # store.rs: [header, body, result], with first-import metadata (version 1, 33 bytes) as a fourth item.
    items = [header, encode([[], []]), b''] + ([b'\1' + bytes(33)] if first_import else [])
    payload = encode(items)
    size = len(payload).to_bytes(4, 'big')
    return size + hashlib.sha256(b'POCOL-REC-LEN' + size).digest()[:4] + hashlib.sha256(payload).digest() + payload + b'\1'


class SeedUpgradeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.old = self.bundle('old', 'a' * 64)
        self.new = self.bundle('new', 'b' * 64, activation=ACTIVATION)
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

    def bundle(self, name, digest, activation=None, forks=None, raise_=None):
        """A seed bundle of a pinned release (rules 2, maxForkVersion 3) on the v3 fixture genesis."""
        bundle = self.root / name; bundle.mkdir(); (bundle / 'net').mkdir(); (bundle / 'bin').mkdir()
        config = {'name': 'seed1', 'minerAddress': '0x' + '11' * 20,
                  'listen': '0.0.0.0:30300', 'rpc': '127.0.0.1:8545', 'peers': ['192.0.2.5:30300']}
        info = {'chainId': CHAIN, 'genesisHash': '0x' + GENESIS.hex(), 'specVersion': 2, 'v3': True,
                'keyMode': 'independent-key-files', 'admission': {'scope': 'experimental-devnet'},
                'forks': forks or [[1, 0]] + ([[2, activation]] if activation else [])}
        if raise_ is not None: info['capRaise'] = raise_
        build = {'schema': 'pocol-testnet-build-v1', 'features': [], 'protocolVersion': 2,
                 'compatibilitySha256': digest, 'consensusPinned': True, 'consensusRulesVersion': 2,
                 'consensusSourcesSha256': 'cd' * 32, 'maxForkVersion': 3, 'protocolVersionMin': 2}
        op.write_json(bundle / 'node.json', config); op.write_json(bundle / 'net/devnet.json', info)
        op.write_json(bundle / 'build-info.json', build)
        (bundle / 'net/genesis.spec.hex').write_text(V3_HEX + '\n')
        for filename in ['node_operator.py', 'pocol_identity.py', 'install.sh', 'pocol-testnet.service', 'nginx.conf',
                         'README.md', 'net/alloc.json'] + ['bin/' + n for n in op.BINARIES]:
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
        content = b'POCOLJ2\n' + GENESIS + b''.join(record(h, version, first_import=i % 2 == 1) for i, h in enumerate(heights))
        values = (1).to_bytes(8, 'big') + len(heights).to_bytes(8, 'big') + len(content).to_bytes(8, 'big')
        slot = values + hashlib.sha256(b'POCOL-COMMIT' + GENESIS + values).digest() + bytes(8)
        (self.data / 'blocks.commit').write_bytes(bytes(4096) + slot)
        (self.data / 'blocks.journal').write_bytes(content + (record(extra) if extra is not None else b''))

    def migrate(self, new=None):
        new = new or self.new
        return upgrade.migrate_binding(self.old, new, self.data, op.digest(new / 'manifest.json'))

    def unchanged_state(self):
        return {name: (self.data / name).read_bytes() for name in
                ('blocks.journal', 'blocks.commit', 'history.redb', 'state.redb')}

    def test_migration_preserves_all_chain_bytes_and_binding_ownership(self):
        original = self.unchanged_state()
        metadata = (self.data / 'network-binding.json').stat()
        report = self.migrate()
        self.assertEqual(report['highestStoredBlock'], '20')
        self.assertEqual(report['appended'], [[2, str(ACTIVATION)]])
        self.assertEqual(report['consensusIdentity'], self.after['consensusIdentity'])
        self.assertNotEqual(self.after['consensusIdentity'], self.before['consensusIdentity'], 'a fork entry is another identity')
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
        for heights, extra in (([1, ACTIVATION, 3], None), ([1, 20], ACTIVATION)):
            with self.subTest(heights=heights, extra=extra):
                self.journal(heights, extra)
                original = self.unchanged_state()
                with self.assertRaisesRegex(ValueError, 'above every stored'): self.migrate()
                self.assertEqual(self.unchanged_state(), original)
                self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.before)

    def test_an_activation_closer_than_the_minimum_notice_is_refused(self):
        # 24 h at T_blk 10 (owner decision 2026-11-16): 8,640 blocks between the highest stored block and the height.
        self.assertEqual(NOTICE, 8640)
        for highest, extra in ((ACTIVATION - NOTICE + 1, None), (ACTIVATION - 1, None), (20, ACTIVATION - NOTICE + 1)):
            with self.subTest(highest=highest, extra=extra):
                self.journal([1, highest, 3], extra)
                original = self.unchanged_state()
                with self.assertRaisesRegex(ValueError, 'at least 8640'): self.migrate()
                self.assertEqual(self.unchanged_state(), original)
                self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.before)
                self.assertFalse((self.data / upgrade.INTENT).exists())
                self.assertFalse((self.data / upgrade.backup_name(self.before)).exists())
        # Exactly the minimum notice is accepted.
        self.journal([1, ACTIVATION - NOTICE, 3])
        self.assertEqual(self.migrate()['highestStoredBlock'], str(ACTIVATION - NOTICE))
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.after)
        with self.assertRaisesRegex(ValueError, 'at least 8640'): upgrade.require_notice([[2, 9000], [3, 30000]], 400)
        # The first appended entry of a two-entry append (H2) sets the last point a seed can migrate, however far H is.
        upgrade.require_notice([[2, 40000], [3, 10**9]], 40000 - NOTICE)
        with self.assertRaisesRegex(ValueError, r'\(2, 40000\).*at least 8640'):
            upgrade.require_notice([[2, 40000], [3, 10**9]], 40000 - NOTICE + 1)
        # Every appended entry is checked, not only the first.
        with self.assertRaisesRegex(ValueError, r'\(3, 30000\)'): upgrade.require_notice([[2, 40000], [3, 30000]], 30000 - NOTICE + 1)
        upgrade.require_notice([], 10**9)

    def test_fork_two_and_the_cap_raise_are_appended_together(self):
        h2, h3 = ACTIVATION, ACTIVATION + 2 * NOTICE
        raised = self.bundle('raised', 'b' * 64, forks=[[1, 0], [2, h2], [3, h3]], raise_=RAISE)
        after = op.read_json(raised / 'manifest.json')['identity']
        report = self.migrate(raised)
        self.assertEqual(report['appended'], [[2, str(h2)], [3, str(h3)]])
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), after)
        before = op.read_json(raised / 'net/devnet.json')
        for change in ({'capRaise': dict(RAISE, ACCT_CAP=RAISE['ACCT_CAP'] + 1)}, {'capRaise': None},
                       {'forks': before['forks'][:2]}):
            changed = dict(before, **change)
            if changed.get('capRaise') is None: changed.pop('capRaise')
            with self.subTest(change=change), self.assertRaises(ValueError):
                upgrade.fork_transition(before, changed)
        with self.assertRaisesRegex(ValueError, 'cap raise'):
            upgrade.fork_transition(op.read_json(self.old / 'net/devnet.json'), dict(before, capRaise=None))
        with self.assertRaisesRegex(ValueError, 'cap raise'):
            upgrade.fork_transition(op.read_json(self.old / 'net/devnet.json'), {k: v for k, v in before.items() if k != 'capRaise'})

    def test_bad_manifest_wrong_node_or_changed_genesis_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'manifest identity'):
            upgrade.migrate_binding(self.old, self.new, self.data, '0' * 64)
        for name in ('node.json', 'net/genesis.spec.hex', 'net/alloc.json', 'pocol-testnet.service', 'install.sh'):
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
            with self.subTest(name=name, size=len(content)):
                (self.data / name).write_bytes(content)
                before = self.unchanged_state()
                with self.assertRaises(ValueError): self.migrate()
                self.assertEqual(self.unchanged_state(), before)
                (self.data / name).write_bytes(original[name])
        # The retired 128-byte commit layout (both slots in one sector) is refused by size, as the release refuses it.
        (self.data / 'blocks.commit').write_bytes(original['blocks.commit'][4096:] + bytes(64))
        with self.assertRaisesRegex(ValueError, 'Recover the commit file'): self.migrate()
        (self.data / 'blocks.commit').write_bytes(original['blocks.commit'])
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
                         [[1, 0], [3, 100]], [[1, 0], [2, 2**64]], [[1, 0], [2, 100], [3, 200], [4, 300]],
                         [[1, 0], [2, 100], [3, 100]]):
            for raise_ in ({}, {'capRaise': RAISE}):
                with self.subTest(schedule=schedule, raise_=raise_), self.assertRaises(ValueError):
                    upgrade.fork_transition(before, {**after, 'forks': schedule, **raise_})
        for change in ({'chainId': 7}, {'genesisHash': '0x' + '00' * 32}, {'admission': {}}, {'v3': False}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                upgrade.fork_transition(before, {**after, **change})

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
        changed = op.read_json(self.new / 'net/devnet.json'); changed['forks'][1][1] = ACTIVATION + 1
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

    def source_tree(self, name, sources):
        """A fixture source checkout and the build-info digest of `sources` (build.rs, no features)."""
        source = self.root / name
        digest = hashlib.sha256(b'pocol-testnet-source-compatibility-v1\0')
        for path, content in sorted(sources.items()):
            (source / 'prototypes' / path).parent.mkdir(parents=True, exist_ok=True)
            (source / 'prototypes' / path).write_bytes(content); encoded = path.encode()
            digest.update(len(encoded).to_bytes(8, 'big')); digest.update(encoded)
            digest.update(len(content).to_bytes(8, 'big')); digest.update(content)
        build = {**self.build, 'compatibilitySha256': digest.hexdigest(),
                 'sourceSha256': {path: hashlib.sha256(content).hexdigest() for path, content in sources.items()}}
        return source, build

    def invitation(self, **change):
        value = {'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': ['192.0.2.5:30300'],
                 'files': {name: (self.new / 'net' / name).read_text()
                           for name in ('genesis.spec.hex', 'alloc.json', 'devnet.json')}}
        value.update(change)
        network = self.root / 'network.pocol.json'
        if network.exists(): network.unlink()
        network.write_text(json.dumps(value))
        return network

    def test_prepare_preserves_node_service_and_network_inputs(self):
        source, build = self.source_tree('source', {'pocold/src/chain.rs': b'new consensus', 'lp1-node/src/profile.rs': b'v2 schedule'})
        network = self.invitation(bootstrap=['198.51.100.7:30300'])
        target = self.root / 'prepared'
        original = {name: path.read_bytes() for name, path in op.files(self.old).items()}
        with patch.object(upgrade, 'ROOT', source), patch.object(upgrade.subprocess, 'check_output', return_value=json.dumps(build)):
            result = upgrade.prepare_bundle(self.old, self.new / 'bin', network, target, op.digest(self.old / 'manifest.json'))
        self.assertFalse(result['deploymentPerformed'])
        self.assertEqual(result['appended'], [[2, str(ACTIVATION)]])
        self.assertEqual(result['consensusIdentity'], self.after['consensusIdentity'], 'the identity the published schedule names')
        self.assertEqual({name: path.read_bytes() for name, path in op.files(self.old).items()}, original)
        for name in ('node.json', 'pocol-testnet.service', 'nginx.conf', 'install.sh', 'net/genesis.spec.hex', 'net/alloc.json'):
            self.assertEqual((target / name).read_bytes(), original[name])
        self.assertTrue((target / 'nonce_upgrade.py').is_file())
        op.verify(target, result['bundleManifestSha256'])
        with self.assertRaisesRegex(ValueError, 'outside'):
            upgrade.prepare_bundle(self.old, self.new / 'bin', network, self.old / 'nested')

    def test_prepare_refuses_other_schemas_rules_genesis_and_metadata(self):
        public = {name: (self.new / 'net' / name).read_text() for name in ('genesis.spec.hex', 'alloc.json', 'devnet.json')}
        info = json.loads(public['devnet.json'])
        cases = [({'schema': 'pocol-network-v1', 'compatibilitySha256': 'a' * 64, 'seeds': [], 'files': public}, 'Invalid new invitation'),
                 ({'schema': 'pocol-network-v2', 'consensusRulesVersion': 3, 'seeds': [], 'files': public}, 'consensus rules'),
                 ({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': [], 'files': public, 'extra': 1}, 'Invalid new invitation'),
                 ({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': [], 'files': dict(public, alloc='{}')}, 'public network files'),
                 ({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': [], 'files': dict(public, **{'alloc.json': '{"x":1}'})}, 'byte-identical'),
                 ({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': [],
                   'files': dict(public, **{'devnet.json': json.dumps(dict(info, forks=[[1, 0], [2, 5]], chainId=7))})}, 'Only the fork schedule')]
        for invitation, message in cases:
            network = self.root / 'invitation.json'
            if network.exists(): network.unlink()
            network.write_text(json.dumps(invitation))
            with self.subTest(message=message), patch.object(upgrade, 'validate_build'), \
                    self.assertRaisesRegex(ValueError, message):
                upgrade.prepare_bundle(self.old, self.new / 'bin', network, self.root / 'refused', op.digest(self.old / 'manifest.json'))
            self.assertFalse((self.root / 'refused').exists())

    def test_build_validation_rejects_changed_local_crypto_dependency(self):
        vendor = 'third-party/evm-crypto/alloy-primitives-0.8.26/src/bits/address.rs'
        source, build = self.source_tree('source-vendor', {'pocold/src/chain.rs': b'consensus fixture',
                                                           'lp1-node/src/profile.rs': b'profile fixture', vendor: b'public recovery fixture'})
        with patch.object(upgrade, 'ROOT', source):
            upgrade.validate_build(build)
            (source / 'prototypes' / vendor).write_bytes(b'changed recovery code')
            with self.assertRaisesRegex(ValueError, 'Rebuild stale native release: third-party/'):
                upgrade.validate_build(build)

    def test_build_validation_requires_a_pinned_feature_free_release_with_rules(self):
        source, build = self.source_tree('source-pinned', {'pocold/src/chain.rs': b'consensus fixture', 'lp1-node/src/profile.rs': b'consensus fixture'})
        with patch.object(upgrade, 'ROOT', source):
            upgrade.validate_build(build)
            for change in ({'consensusPinned': False}, {'consensusRulesVersion': None}, {'consensusRulesVersion': 0}):
                with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'pinned consensus'):
                    upgrade.validate_build({**build, **change})
            for change in ({'features': ['caps-test']}, {'protocolVersion': 1}, {'schema': 'other'}):
                with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'feature-free'):
                    upgrade.validate_build({**build, **change})
            with self.assertRaisesRegex(ValueError, 'compatibility digest'):
                upgrade.validate_build({**build, 'compatibilitySha256': 'f' * 64})

    def test_a_second_migration_writes_its_own_backup(self):
        self.migrate()
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), self.after)
        # A later release of the same rules and schedule: a rebind, with its own backup of the replaced marker.
        newer = self.bundle('newer', 'c' * 64, activation=ACTIVATION)
        with patch.object(upgrade.subprocess, 'check_output', return_value=(newer / 'build-info.json').read_text()):
            upgrade.migrate_binding(self.new, newer, self.data, op.digest(newer / 'manifest.json'))
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), op.read_json(newer / 'manifest.json')['identity'])
        self.assertEqual(op.read_json(self.data / upgrade.backup_name(self.before)), self.before)
        self.assertEqual(op.read_json(self.data / upgrade.backup_name(self.after)), self.after)
        self.assertNotEqual(upgrade.backup_name(self.before), upgrade.backup_name(self.after))

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
        three = {**two, 'forks': [[1, 0], [2, 100], [3, 200]], 'capRaise': RAISE}
        self.assertEqual(upgrade.fork_transition(before, two), [[2, 100]])
        self.assertEqual(upgrade.fork_transition(two, two), [])
        self.assertEqual(upgrade.fork_transition(two, three), [[3, 200]])
        self.assertEqual(upgrade.fork_transition(before, three), [[2, 100], [3, 200]])
        self.assertEqual(upgrade.fork_transition(three, three), [])
        for forks in ([[1, 0]], [[1, 0], [2, 101]]):
            with self.subTest(forks=forks), self.assertRaises(ValueError):
                upgrade.fork_transition(two, {**two, 'forks': forks})
        with self.assertRaises(ValueError): upgrade.fork_transition(three, two)

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
        # Another schedule is another consensus identity: refused until migrated.
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

    def test_a_marker_without_a_consensus_identity_is_refused(self):
        legacy = {k: v for k, v in self.before.items() if k not in ('consensusIdentity', 'consensusRulesVersion')}
        (self.data / 'network-binding.json').unlink(); op.write_json(self.data / 'network-binding.json', legacy)
        with self.assertRaisesRegex(ValueError, 'does not match the old seed bundle'): self.migrate()
        self.assertEqual(op.read_json(self.data / 'network-binding.json'), legacy)
        self.assertFalse((self.data / upgrade.INTENT).exists())


if __name__ == '__main__':
    unittest.main()
