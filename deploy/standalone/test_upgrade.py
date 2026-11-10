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

from test_launcher import BIN, EXE, ROOT, call, identity, network

H15000 = ROOT / 'deploy/mainnet/h15000/network.pocol.json'
# launcher::legacy_refusal() under SWITCH = B, owner decision D1's recommended default (plan §5.4).
LEGACY_REFUSAL = 'This network was replaced by a new genesis. Follow the release notes to move the old network files aside; keep your wallet key.'


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
        # The upgrade moves a v1 bounded-range network to v2, so it starts without fullNonce.
        self.invitation = network(self.root, [self.home], ['127.0.0.1:30300'], full_nonce=False)
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

    def test_keeps_identity_and_chain_files_and_backs_up_the_binding_by_fingerprint(self):
        # A v2 binding names the consensus rules, not a build digest, so no "previous build" case remains here; a
        # v1 file of an uncertified build is refused (launcher.rs unit tests; the CLI refusal moves to plan T32).
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
        backups = sorted(p.name for p in self.home.glob('network.pocol.json.before-*'))
        self.assertEqual(len(backups), 1, backups)
        self.assertRegex(backups[0], r'^network\.pocol\.json\.before-[0-9a-f]{16}$')
        self.assertEqual(read_json(self.home / backups[0]), self.before)
        self.assertFalse((self.home / 'network.pocol.json.before-full-nonce').exists(), 'the 0.4.0 name is never written')
        self.assertFalse((self.home / 'network-upgrade.pending.json').exists())
        self.upgrade()  # Safe idempotent retry.
        for name in ('alloc.json', 'genesis.spec.hex'):
            self.assertEqual((self.home / 'network' / name).read_bytes(), self.before['files'][name].encode('utf-8'))

    def test_a_home_bound_to_the_uncertified_h15000_file_is_refused_with_the_release_notes_text(self):
        # Plan T32 and §5.4: LEGACY_BUILDS ships uncertified (owner decision D1, Option B by default), so neither an
        # upgrade nor a plain start of an H15000 home proceeds, and nothing in the home changes.
        h15000 = read_json(H15000)
        write_json(self.home / 'network.pocol.json', h15000)
        for name, content in h15000['files'].items():
            (self.home / 'network' / name).write_bytes(content.encode('utf-8'))
        before = {p.relative_to(self.home): p.read_bytes() for p in self.home.rglob('*') if p.is_file()}
        for args in (('upgrade', '--home', self.home, '--network', self.target),
                     ('inspect-upgrade', '--home', self.home, '--network', self.target),
                     ('import', '--home', self.home)):
            with self.subTest(command=args[0]):
                refused = call('pocol', *args, ok=False)
                self.assertNotEqual(refused.returncode, 0)
                self.assertIn(LEGACY_REFUSAL, refused.stderr)
                self.assertEqual({p.relative_to(self.home): p.read_bytes() for p in self.home.rglob('*') if p.is_file()}, before)
        self.assertFalse((self.home / 'network-upgrade.pending.json').exists())

    def test_rejects_changed_network_metadata_seeds_and_schedule(self):
        candidates = []
        for name in ('alloc.json', 'genesis.spec.hex'):
            changed = copy.deepcopy(self.after)
            changed['files'][name] += '\n'
            candidates.append(changed)
        changed = copy.deepcopy(self.after)
        changed['seeds'] = ['127.0.0.1:30301']
        candidates.append(changed)
        # The same schedule ([[1, 0]]) in a reformatted devnet.json is a rebind, not a refusal (plan §1.7).
        same = copy.deepcopy(self.after)
        metadata = json.loads(same['files']['devnet.json']); metadata['forks'] = [[1, 0]]
        same['files']['devnet.json'] = json.dumps(metadata)
        write_json(self.target, same)
        self.assertEqual(json.loads(call('pocol', 'inspect-upgrade', '--home', self.home, '--network', self.target).stdout)['change'], 'rebind')
        for forks in ([[1, 0], [2, 0]], [[1, 0], [3, 100000]]):
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


class NewGenesisRejoin(unittest.TestCase):
    """A home bound to one genesis joins a network with another genesis (the earn-in cutover, owner decision D1
    Option B) the way the release notes describe (runbook step 3; RELEASE-UPGRADE.md "Resync a launcher home"): the
    new invitation goes beside the application, the old network's files (state, wallet activity, every
    network.pocol.json.before-* backup) go to a folder in the home, the key stays, and the wallet imports the new
    invitation. Both invitations are `pocol network` output, so pocol-network-v2 bound to the rules revision. Import
    and upgrade refuse the new genesis while the old binding is in place, so nothing rebinds a home silently, and the
    published H15000 (v1, uncertified) invitation is refused with the Option B text and writes nothing."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.home = self.root / 'home'
        self.address = identity(self.home)
        self.old = self.invitation('old', 777915)
        self.new = self.invitation('new', 777916)
        # The application folder: the engine and, as the 0.4.0 notes asked, the old invitation beside it.
        self.app = self.root / 'app'
        self.app.mkdir()
        shutil.copy2(BIN / ('pocol' + EXE), self.app / ('pocol' + EXE))
        shutil.copy2(self.old, self.app / 'network.pocol.json')
        self.engine('import', '--home', self.home)

    def invitation(self, name, chain_id):
        root = self.root / name
        root.mkdir()
        config = {'chainId': chain_id, 'genesisTime': int(time.time()) + 5, 'registrationPerBlock': 1,
                  'miners': [{'id': self.address, 'reward': self.address}],
                  'accounts': [{'address': self.address, 'wei': str(10**24)}], 'fullNonce': True}
        (root / 'public.json').write_text(json.dumps(config), encoding='utf-8')
        call('pocold', 'init-testnet', '--config', root / 'public.json', '--out', root / 'genesis')
        call('pocol', 'network', '--from', root / 'genesis', '--out', root / 'network.pocol.json',
             '--seed', '127.0.0.1:30300')
        return root / 'network.pocol.json'

    def engine(self, *args, ok=True):
        """The application folder's engine without --network, as the Windows wallet runs it when it opens."""
        p = subprocess.run([str(self.app / ('pocol' + EXE)), *map(str, args)], capture_output=True, text=True,
                           encoding='utf-8', timeout=90)
        if ok and p.returncode:
            raise AssertionError(p.stderr)
        return p

    def bound_chain(self):
        return json.loads(read_json(self.home / 'network.pocol.json')['files']['devnet.json'])['chainId']

    def move_aside(self):
        # What a used home also holds: chain data, wallet activity, the H15000 upgrade's backup and a later
        # release's fingerprint-named backup.
        (self.home / 'state').mkdir(exist_ok=True)
        (self.home / 'state' / 'blocks.journal').write_bytes(b'old chain')
        (self.home / 'wallet-activity.json').write_text('[]', encoding='utf-8')
        (self.home / 'network.pocol.json.before-full-nonce').write_text('{}', encoding='utf-8')
        (self.home / 'network.pocol.json.before-0123456789abcdef').write_text('{}', encoding='utf-8')
        aside = self.home / 'old-network'
        aside.mkdir()
        moved = ['network.pocol.json', 'network', 'state', 'wallet-activity.json',
                 'network.pocol.json.before-full-nonce', 'network.pocol.json.before-0123456789abcdef']
        for name in moved:
            shutil.move(str(self.home / name), str(aside / name))
        return aside, moved

    def test_the_new_invitation_beside_the_application_and_the_old_files_aside_keep_the_address(self):
        self.assertEqual(self.bound_chain(), 777915)
        key = (self.home / 'identity.pocol-key').read_bytes()
        refused = call('pocol', 'import', '--home', self.home, '--network', self.new, ok=False)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn('bound to another invitation', refused.stderr)
        refused = call('pocol', 'upgrade', '--home', self.home, '--network', self.new, ok=False)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn('must preserve the genesis', refused.stderr)
        shutil.copy2(self.new, self.app / 'network.pocol.json')
        aside, moved = self.move_aside()
        self.engine('import', '--home', self.home)
        self.assertEqual(identity(self.home), self.address)
        self.assertEqual((self.home / 'identity.pocol-key').read_bytes(), key)
        self.assertEqual(self.bound_chain(), 777916)
        self.assertEqual(sorted(p.name for p in aside.iterdir()), sorted(moved))
        self.assertEqual((aside / 'state' / 'blocks.journal').read_bytes(), b'old chain')

    def test_the_real_h15000_invitation_beside_the_application_is_refused_and_writes_nothing(self):
        # Plan T32: a fresh home with only its key, and the published 0.4.0 invitation beside the application.
        home = self.root / 'fresh'
        self.assertEqual(len(identity(home)), 42)
        shutil.copy2(H15000, self.app / 'network.pocol.json')
        before = sorted(p.name for p in home.iterdir())
        refused = self.engine('import', '--home', home, ok=False)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn(LEGACY_REFUSAL, refused.stderr)
        self.assertEqual(sorted(p.name for p in home.iterdir()), before, 'no binding and no network files')
        self.assertFalse((home / 'network.pocol.json').exists())

    def test_an_old_invitation_left_beside_the_application_binds_the_home_again(self):
        # Why the notes replace the file beside the application first: otherwise opening the wallet rebinds
        # the cleared home to the old genesis, and importing the new invitation is then refused.
        self.move_aside()
        self.engine('import', '--home', self.home)
        self.assertEqual(self.bound_chain(), 777915)
        refused = call('pocol', 'import', '--home', self.home, '--network', self.new, ok=False)
        self.assertIn('bound to another invitation', refused.stderr)


if __name__ == '__main__':
    unittest.main()
