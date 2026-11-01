#!/usr/bin/env python3
"""Explicit same-chain upgrade: no private-key replacement or state mutation."""
import copy
import hashlib
import json
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time
import unittest

from test_launcher import BIN, EXE, call, network


def read_json(path):
    return json.loads(path.read_bytes().decode('utf-8'))


def write_json(path, value):
    # Fixtures must use the native UTF-8 bytes even on Windows locales/newlines.
    path.write_bytes(json.dumps(value, ensure_ascii=False).encode('utf-8'))


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
        self.before = read_json(self.invitation)
        metadata = json.loads(self.before['files']['devnet.json'])
        metadata['upgradeTestNote'] = 'Unicode fixture: Δφ / شبكة PoCol / 日本語'
        self.before['files']['devnet.json'] = json.dumps(metadata, ensure_ascii=False, indent=2) + '\n'
        write_json(self.invitation, self.before)
        call('pocol', 'import', '--home', self.home, '--network', self.invitation)
        self.after = copy.deepcopy(self.before)
        metadata = json.loads(self.after['files']['devnet.json'])
        metadata['forks'] = [[1, 0], [2, 100000]]
        self.after['files']['devnet.json'] = json.dumps(metadata, ensure_ascii=False, indent=2) + '\n'
        self.target = self.root / 'upgraded-network.pocol.json'
        write_json(self.target, self.after)
        self.key_before = (self.home / 'identity.pocol-key').read_bytes()
        self.address_before = (self.home / 'identity.address').read_bytes()

    def upgrade(self, ok=True):
        return call('pocol', 'upgrade', '--home', self.home, '--network', self.target, ok=ok)

    def assert_identity_preserved(self):
        self.assertEqual((self.home / 'identity.pocol-key').read_bytes(), self.key_before)
        self.assertEqual((self.home / 'identity.address').read_bytes(), self.address_before)

    def wallet_status(self, executable):
        """Connect through the real GUI CLI path, then stop without enabling mining."""
        messages = queue.Queue()
        with tempfile.TemporaryFile(mode='w+t', encoding='utf-8') as errors:
            process = subprocess.Popen([str(executable), 'wallet', '--home', str(self.home)],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors,
                text=True, encoding='utf-8', bufsize=1)
            def read():
                for line in process.stdout:
                    try: messages.put(json.loads(line))
                    except ValueError: pass
            reader = threading.Thread(target=read, daemon=True); reader.start()
            try:
                deadline = time.monotonic() + 90
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        errors.seek(0)
                        self.fail('wallet exited before status: ' + errors.read())
                    try: message = messages.get(timeout=1)
                    except queue.Empty: continue
                    if message.get('type') == 'status': return message
                self.fail('wallet status timeout')
            finally:
                if process.poll() is None:
                    try:
                        process.stdin.write('{"method":"stop"}\n'); process.stdin.flush()
                    except (BrokenPipeError, OSError): pass
                    try: process.wait(timeout=15)
                    except subprocess.TimeoutExpired: process.kill(); process.wait()
                reader.join(timeout=2)
                process.stdin.close(); process.stdout.close()

    def test_reconnect_uses_upgraded_home_despite_stale_adjacent_invitation(self):
        app = self.root / 'app'; app.mkdir()
        executable = app / ('pocol' + EXE)
        shutil.copy2(BIN / executable.name, executable)
        adjacent = app / 'network.pocol.json'
        write_json(adjacent, self.before)
        before = self.wallet_status(executable)
        state = self.home / 'state'
        journal = (state / 'blocks.journal').read_bytes()
        self.assertTrue((state / 'state.redb').is_file())
        self.upgrade()
        bound = (self.home / 'network.pocol.json').read_bytes()
        after = self.wallet_status(executable)
        self.assertEqual(read_json(adjacent), self.before, 'the packaged invitation remains stale')
        self.assertEqual((self.home / 'network.pocol.json').read_bytes(), bound)
        self.assertEqual(read_json(self.home / 'network.pocol.json'), self.after)
        self.assertEqual((state / 'blocks.journal').read_bytes(), journal)
        self.assertTrue((state / 'state.redb').is_file())
        for field in ('address', 'chainId', 'genesisHash', 'height', 'balanceWei', 'registered', 'bond'):
            self.assertEqual(after[field], before[field], field)
        self.assertFalse(after['miningEnabled'])
        self.assert_identity_preserved()
        # Explicit selection remains authoritative and cannot silently rebind a home.
        refused = subprocess.run([str(executable), 'wallet', '--home', str(self.home),
                                  '--network', str(adjacent)], capture_output=True,
                                 text=True, encoding='utf-8', timeout=30)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn('bound to another invitation', refused.stderr)
        self.assertEqual((self.home / 'network.pocol.json').read_bytes(), bound)
        self.assertEqual((state / 'blocks.journal').read_bytes(), journal)
        self.assert_identity_preserved()

    def test_accepts_previous_build_and_keeps_identity_and_chain_files(self):
        self.before['compatibilitySha256'] = 'f' * 64
        write_json(self.home / 'network.pocol.json', self.before)
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
        self.assertEqual(read_json(self.home / 'network.pocol.json'), self.after)
        self.assertEqual(read_json(self.home / 'network.pocol.json.before-full-nonce'), self.before)
        self.assertFalse((self.home / 'network-upgrade.pending.json').exists())
        self.upgrade()  # Safe idempotent retry.
        for name in ('alloc.json', 'genesis.spec.hex'):
            self.assertEqual((self.home / 'network' / name).read_bytes(), self.before['files'][name].encode('utf-8'))

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
            write_json(self.target, candidate)
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
            write_json(self.target, changed)
            self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
            self.assertEqual((self.home / 'network.pocol.json').read_bytes(), current)

    def test_resume_after_each_public_configuration_write(self):
        # Materialize each durable crash boundary without killing a real wallet.
        for stage in ('intent', 'metadata', 'binding'):
            write_json(self.home / 'network.pocol.json', self.before)
            (self.home / 'network' / 'devnet.json').write_bytes(self.before['files']['devnet.json'].encode('utf-8'))
            marker = self.home / 'network-upgrade.pending.json'
            write_json(marker, {'before': self.before, 'after': self.after})
            if stage in ('metadata', 'binding'):
                (self.home / 'network' / 'devnet.json').write_bytes(self.after['files']['devnet.json'].encode('utf-8'))
            if stage == 'binding':
                write_json(self.home / 'network.pocol.json', self.after)
            result = call('pocol', 'import', '--home', self.home, '--network', self.target, ok=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('incomplete', result.stderr)
            self.upgrade()
            self.assertFalse(marker.exists())
            self.assert_identity_preserved()
            self.assertEqual(read_json(self.home / 'network.pocol.json'), self.after)

    def test_rejects_unknown_resume_target_and_changed_local_file(self):
        marker = self.home / 'network-upgrade.pending.json'
        write_json(marker, {'before': self.before, 'after': self.after})
        changed = copy.deepcopy(self.after)
        metadata = json.loads(changed['files']['devnet.json'])
        metadata['forks'][-1][1] += 1
        changed['files']['devnet.json'] = json.dumps(metadata)
        write_json(self.target, changed)
        self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
        self.assertTrue(marker.exists())
        marker.unlink()
        write_json(self.target, self.after)
        alloc = self.home / 'network' / 'alloc.json'
        alloc.write_bytes(b'unexpected')
        self.assertNotEqual(self.upgrade(ok=False).returncode, 0)
        self.assertEqual(alloc.read_bytes(), b'unexpected')
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
