#!/usr/bin/env python3
"""Explicit same-chain upgrade: no private-key replacement or state mutation."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from test_launcher import call, network


def rlp(value):
    if isinstance(value, int):
        value = value.to_bytes((value.bit_length() + 7) // 8, 'big')
    if isinstance(value, list):
        raw = b''.join(map(rlp, value)); short, long = 0xc0, 0xf7
    else:
        raw = value; short, long = 0x80, 0xb7
        if len(raw) == 1 and raw[0] < 128:
            return raw
    if len(raw) < 56:
        return bytes([short + len(raw)]) + raw
    size = len(raw).to_bytes((len(raw).bit_length() + 7) // 8, 'big')
    return bytes([long + len(size)]) + size + raw


class FullNonceUpgrade(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.home = self.root / 'home'
        self.invitation = network(self.root, [self.home], ['127.0.0.1:30300'])
        call('pocol', 'import', '--home', self.home, '--network', self.invitation)
        self.before = json.loads(self.invitation.read_text())
        self.after = copy.deepcopy(self.before)
        metadata = json.loads(self.after['files']['devnet.json'])
        metadata['forks'] = [[1, 0], [2, 100000]]
        self.after['files']['devnet.json'] = json.dumps(metadata)
        self.target = self.root / 'upgraded-network.pocol.json'
        self.target.write_text(json.dumps(self.after))
        self.key_before = (self.home / 'identity.pocol-key').read_bytes()
        self.address_before = (self.home / 'identity.address').read_bytes()

    def upgrade(self, ok=True):
        return call('pocol', 'upgrade', '--home', self.home, '--network', self.target, ok=ok)

    def assert_identity_preserved(self):
        self.assertEqual((self.home / 'identity.pocol-key').read_bytes(), self.key_before)
        self.assertEqual((self.home / 'identity.address').read_bytes(), self.address_before)

    def test_accepts_previous_build_and_keeps_identity_and_chain_files(self):
        self.before['compatibilitySha256'] = 'f' * 64
        (self.home / 'network.pocol.json').write_text(json.dumps(self.before))
        state = self.home / 'state'
        state.mkdir()
        (state / 'LOCK').write_bytes(b'')
        genesis = bytes.fromhex(json.loads(self.before['files']['devnet.json'])['genesisHash'][2:])
        (state / 'blocks.journal').write_bytes(b'POCOLJ2\n' + genesis)
        (state / 'state.redb').write_bytes(b'unchanged database sentinel')
        original = {p.name: p.read_bytes() for p in state.iterdir()}
        self.upgrade()
        self.assertEqual({p.name: p.read_bytes() for p in state.iterdir()}, original)
        self.assert_identity_preserved()
        self.assertEqual(json.loads((self.home / 'network.pocol.json').read_text()), self.after)
        self.assertEqual(json.loads((self.home / 'network.pocol.json.before-full-nonce').read_text()), self.before)
        self.assertFalse((self.home / 'network-upgrade.pending.json').exists())
        self.upgrade()  # Safe idempotent retry.
        for name in ('alloc.json', 'genesis.spec.hex'):
            self.assertEqual((self.home / 'network' / name).read_text(), self.before['files'][name])

    def test_rejects_changed_network_metadata_seeds_and_schedule(self):
        candidates = []
        for name in ('alloc.json', 'genesis.spec.hex'):
            changed = copy.deepcopy(self.after)
            changed['files'][name] += '\n'
            candidates.append(changed)
        changed = copy.deepcopy(self.after)
        changed['seeds'] = ['127.0.0.1:30301']
        candidates.append(changed)
        for forks in ([[1, 0]], [[1, 0], [2, 0]], [[1, 0], [3, 100000]]):
            changed = copy.deepcopy(self.after)
            metadata = json.loads(changed['files']['devnet.json'])
            metadata['forks'] = forks
            changed['files']['devnet.json'] = json.dumps(metadata)
            candidates.append(changed)
        changed = copy.deepcopy(self.after)
        metadata = json.loads(changed['files']['devnet.json'])
        metadata['genesisTime'] += 1
        changed['files']['devnet.json'] = json.dumps(metadata)
        candidates.append(changed)
        binding = (self.home / 'network.pocol.json').read_bytes()
        for candidate in candidates:
            self.target.write_text(json.dumps(candidate))
            self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
            self.assertEqual((self.home / 'network.pocol.json').read_bytes(), binding)
            self.assert_identity_preserved()
            self.assertFalse((self.home / 'network-upgrade.pending.json').exists())

    def test_cannot_reschedule_or_remove_an_existing_activation(self):
        self.upgrade()
        current = (self.home / 'network.pocol.json').read_bytes()
        for forks in ([[1, 0]], [[1, 0], [2, 100001]]):
            changed = copy.deepcopy(self.after)
            metadata = json.loads(changed['files']['devnet.json'])
            metadata['forks'] = forks
            changed['files']['devnet.json'] = json.dumps(metadata)
            self.target.write_text(json.dumps(changed))
            self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
            self.assertEqual((self.home / 'network.pocol.json').read_bytes(), current)

    def test_resume_after_each_public_configuration_write(self):
        # Materialize each durable crash boundary without killing a real wallet.
        for stage in ('intent', 'metadata', 'binding'):
            (self.home / 'network.pocol.json').write_text(json.dumps(self.before))
            (self.home / 'network' / 'devnet.json').write_text(self.before['files']['devnet.json'])
            marker = self.home / 'network-upgrade.pending.json'
            marker.write_text(json.dumps({'before': self.before, 'after': self.after}))
            if stage in ('metadata', 'binding'):
                (self.home / 'network' / 'devnet.json').write_text(self.after['files']['devnet.json'])
            if stage == 'binding':
                (self.home / 'network.pocol.json').write_text(json.dumps(self.after))
            result = call('pocol', 'import', '--home', self.home, '--network', self.target, ok=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('incomplete', result.stderr)
            self.upgrade()
            self.assertFalse(marker.exists())
            self.assert_identity_preserved()
            self.assertEqual(json.loads((self.home / 'network.pocol.json').read_text()), self.after)

    def test_rejects_unknown_resume_target_and_changed_local_file(self):
        marker = self.home / 'network-upgrade.pending.json'
        marker.write_text(json.dumps({'before': self.before, 'after': self.after}))
        changed = copy.deepcopy(self.after)
        metadata = json.loads(changed['files']['devnet.json'])
        metadata['forks'][-1][1] += 1
        changed['files']['devnet.json'] = json.dumps(metadata)
        self.target.write_text(json.dumps(changed))
        self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
        self.assertTrue(marker.exists())
        marker.unlink()
        self.target.write_text(json.dumps(self.after))
        alloc = self.home / 'network' / 'alloc.json'
        alloc.write_text('unexpected')
        self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
        self.assertEqual(alloc.read_text(), 'unexpected')
        self.assert_identity_preserved()

    def test_upgrade_never_creates_a_missing_home_or_key(self):
        absent = self.root / 'absent'
        result = call('pocol', 'upgrade', '--home', absent, '--network', self.target, ok=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(absent.exists())
        (self.home / 'identity.pocol-key').unlink()
        self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
        self.assertFalse((self.home / 'identity.pocol-key').exists())

    def test_rejects_activation_at_or_below_a_stored_branch_height(self):
        # A framed header fixture tests the read-only safety scan, not block validity.
        info = json.loads(self.before['files']['devnet.json'])
        genesis = bytes.fromhex(info['genesisHash'][2:])
        ut = [b'PoCol-tpl-v1', info['chainId'], genesis, bytes(32), 100000, 0, 1, 0,
              1, bytes(32), bytes(32), bytes(32), bytes(256), 0, 0, 0, bytes(32), b'']
        header = rlp([[ut, b''], bytes(8), [], bytes(65)])
        payload = rlp([header, b'\xc2\xc0\xc0', b''])
        size = len(payload).to_bytes(4, 'big')
        frame = size + hashlib.sha256(b'POCOL-REC-LEN' + size).digest()[:4]
        frame += hashlib.sha256(payload).digest() + payload + b'\x01'
        state = self.home / 'state'; state.mkdir()
        (state / 'LOCK').write_bytes(b'')
        journal = state / 'blocks.journal'
        journal.write_bytes(b'POCOLJ2\n' + genesis + frame)
        original = journal.read_bytes()
        result = self.upgrade(ok=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('above every stored block', result.stderr)
        self.assertEqual(journal.read_bytes(), original)
        self.assertFalse((self.home / 'network-upgrade.pending.json').exists())
        self.assert_identity_preserved()


if __name__ == '__main__':
    unittest.main()
