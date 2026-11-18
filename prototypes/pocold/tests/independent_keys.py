#!/usr/bin/env python3
"""Independent-key and public-input genesis controls using release binaries (Unix)."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

BIN = Path(os.environ.get('POCOLD_BIN_DIR', str(Path(__file__).resolve().parents[1] / 'target/release')))


def rlp(data, i=0):
    """Decodes one RLP item at data[i:]: (bytes or list, next index). Enough to read a GenesisSpec."""
    b = data[i]
    if b < 0x80: return data[i:i + 1], i + 1
    if b < 0xb8: n = b - 0x80; return data[i + 1:i + 1 + n], i + 1 + n
    if b < 0xc0: k = b - 0xb7; n = int.from_bytes(data[i + 1:i + 1 + k], 'big'); return data[i + 1 + k:i + 1 + k + n], i + 1 + k + n
    if b < 0xf8: n, start = b - 0xc0, i + 1
    else: k = b - 0xf7; n, start = int.from_bytes(data[i + 1:i + 1 + k], 'big'), i + 1 + k
    items, j = [], start
    while j < start + n:
        item, j = rlp(data, j); items.append(item)
    return items, start + n


def nonce_mode(net):
    """CP field 4 (nonceMode) of the genesis written by init-testnet or init-devnet."""
    spec, _ = rlp(bytes.fromhex((net / 'genesis.spec.hex').read_text().strip().removeprefix('0x')))
    return int.from_bytes(spec[2][4], 'big')


class IndependentKeys(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def call(self, binary, *args):
        return subprocess.run([str(BIN / binary), *map(str, args)], capture_output=True, text=True)

    def key(self, name):
        path = self.root / (name + '.pocol-key')
        result = self.call('pocol-key', 'generate', '--out', path)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertRegex(result.stdout.strip(), r'^0x[0-9a-f]{40}$')
        return path, result.stdout.strip()

    def config(self):
        _, miner = self.key('miner')
        _, reward = self.key('reward')
        return {'chainId': 777913, 'genesisTime': 1793275000, 'registrationPerBlock': 1,
                'miners': [{'id': miner, 'reward': reward}],
                'accounts': [{'address': reward, 'wei': '1000000000000000000000000'}]}

    def init(self, config, out='net'):
        path = self.root / 'public.json'
        path.write_text(json.dumps(config))
        return self.call('pocold', 'init-testnet', '--config', path, '--out', self.root / out)

    def test_independent_generation_and_no_overwrite(self):
        first, address = self.key('a')
        second, other = self.key('b')
        self.assertNotEqual(address, other)
        self.assertEqual(first.stat().st_mode & 0o777, 0o600)
        self.assertEqual(len(first.read_bytes()), 32)
        saved = first.read_bytes()
        self.assertNotEqual(self.call('pocol-key', 'generate', '--out', first).returncode, 0)
        self.assertEqual(first.read_bytes(), saved)
        self.assertEqual(self.call('pocol-key', 'address', '--key-file', first).stdout.strip(), address)
        self.assertNotIn(saved.hex(), address)

    def test_unsafe_secret_files_fail_without_disclosure(self):
        path, _ = self.key('a')
        saved = path.read_bytes()
        for data in [b'', bytes(32), b'x' * 33]:
            path.write_bytes(data)
            result = self.call('pocol-key', 'address', '--key-file', path)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn(saved.hex(), result.stdout + result.stderr)
        path.write_bytes(saved)
        for mode in [0o644, 0o700, 0o000]:
            path.chmod(mode)
            self.assertNotEqual(self.call('pocol-key', 'address', '--key-file', path).returncode, 0)
        path.chmod(0o400)
        self.assertEqual(self.call('pocol-key', 'address', '--key-file', path).returncode, 0)
        path.chmod(0o600)
        link = self.root / 'link'
        link.symlink_to(path)
        self.assertNotEqual(self.call('pocol-key', 'address', '--key-file', link).returncode, 0)
        link.unlink()
        os.link(path, link)
        self.assertNotEqual(self.call('pocol-key', 'address', '--key-file', path).returncode, 0)

    def feed(self, binary, *args, stdin):
        """A tool run whose passphrase lines arrive on stdin (a pipe), as the Windows wallet feeds `pocol`."""
        return subprocess.run([str(BIN / binary), *map(str, args)], input=stdin, capture_output=True, text=True,
                              encoding='utf-8', timeout=120)

    def test_a_tool_waiting_on_a_pipe_for_its_passphrase_ends_on_sigterm(self):
        # The Ctrl-C handlers of the prompt are installed for a terminal only (echo off): with stdin a pipe that is
        # still open, SIGTERM ends pocol-key at once instead of waiting for the pipe to deliver a line or close.
        import signal, time
        key, _ = self.key('waiting')
        out = self.root / 'waiting.pocol-backup'
        p = subprocess.Popen([str(BIN / 'pocol-key'), 'export', '--key-file', str(key), '--out', str(out)],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            time.sleep(1)
            self.assertIsNone(p.poll(), 'still waiting for the passphrase')
            p.send_signal(signal.SIGTERM)
            p.wait(timeout=5)
        finally:
            if p.poll() is None:
                p.kill()
            p.stdin.close()
            p.wait()
        self.assertNotEqual(p.returncode, 0)
        self.assertFalse(out.exists(), 'nothing was written')

    def test_encrypted_backup_restores_the_same_address(self):
        # CRY-02 for raw key files: `pocol-key export` writes the 98-byte passphrase-encrypted backup of
        # `pocol export-key`, `pocol-key import` restores it into a new key file. Passphrases come on stdin.
        key, address = self.key('reward')
        secret = key.read_bytes()
        backup = self.root / 'reward.pocol-backup'
        phrase = 'a long backup passphrase ✓'
        both = f'{phrase}\n{phrase}\n'
        def export(stdin, out=backup):
            return self.feed('pocol-key', 'export', '--key-file', key, '--out', out, stdin=stdin)
        self.assertIn('differ', export(f'{phrase}\nanother long passphrase\n').stderr)
        self.assertIn('at least 12', export('short\nshort\n').stderr)
        self.assertIn('too easy to guess', export('Password2024!\nPassword2024!\n').stderr)
        self.assertIn('no passphrase', export('').stderr)
        self.assertFalse(backup.exists(), 'a refused passphrase writes no backup')
        done = export(both)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout.strip(), address)
        data = backup.read_bytes()
        self.assertEqual((len(data), data[:17]), (98, b'POCOL-KEY-BACKUP\x01'))
        self.assertEqual(backup.stat().st_mode & 0o777, 0o600)
        self.assertNotIn(secret, data)
        self.assertNotIn(secret.hex(), done.stdout + done.stderr)
        # An existing output, a symlink to one or another file, is never replaced.
        again = export(both)
        self.assertNotEqual(again.returncode, 0)
        self.assertIn('never replaced', again.stderr)
        self.assertEqual(backup.read_bytes(), data)
        link = self.root / 'link.pocol-backup'
        link.symlink_to(self.root / 'absent')
        self.assertNotEqual(export(both, link).returncode, 0)
        self.assertFalse((self.root / 'absent').exists())

        restored = self.root / 'restored.pocol-key'
        def restore(stdin, out=restored, source=backup):
            return self.feed('pocol-key', 'import', '--backup', source, '--out', out, stdin=stdin)
        wrong = restore('a wrong passphrase\n')
        self.assertNotEqual(wrong.returncode, 0)
        self.assertIn('Wrong passphrase', wrong.stderr)
        self.assertFalse(restored.exists(), 'a wrong passphrase writes nothing')
        damaged = self.root / 'damaged.pocol-backup'
        damaged.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
        self.assertIn('Wrong passphrase or damaged', restore(phrase + '\n', source=damaged).stderr)
        self.assertFalse(restored.exists())
        ok = restore(phrase + '\r\n')
        self.assertEqual(ok.returncode, 0, ok.stderr)
        self.assertEqual(ok.stdout.strip(), address)
        self.assertEqual(restored.read_bytes(), secret)
        self.assertEqual(restored.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.call('pocol-key', 'address', '--key-file', restored).stdout.strip(), address)
        # The restored key file, or any other existing path, is never overwritten by an import.
        other, _ = self.key('other')
        saved = other.read_bytes()
        for target in (restored, other):
            refused = restore(phrase + '\n', out=target)
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn('never replaced', refused.stderr)
        self.assertEqual((restored.read_bytes(), other.read_bytes()), (secret, saved))
        for args in [('export', '--key-file', key), ('import', '--backup', backup), ('export', '--out', backup, '--key-file', key)]:
            self.assertIn('usage', self.feed('pocol-key', *args, stdin=both).stderr)

    def test_signing_from_a_backup_matches_the_key_file(self):
        key, address = self.key('alice')
        backup = self.root / 'alice.pocol-backup'
        phrase = 'a long backup passphrase ✓'
        self.assertEqual(self.feed('pocol-key', 'export', '--key-file', key, '--out', backup,
                                   stdin=f'{phrase}\n{phrase}\n').returncode, 0)
        tx = ('--chain-id', 777913, '--to', '0x' + '22' * 20, '--value', 7, '--nonce', 1, '--count', 2)
        plain = self.call('pocol-sign', *tx, '--key-file', key)
        self.assertEqual(plain.returncode, 0, plain.stderr)
        self.assertEqual(len(plain.stdout.split()), 2)
        sealed = self.feed('pocol-sign', *tx, '--backup', backup, stdin=phrase + '\n')
        self.assertEqual(sealed.returncode, 0, sealed.stderr)
        self.assertEqual(sealed.stdout, plain.stdout, 'the same signed transactions')
        self.assertEqual(self.feed('pocol-sign', '--chain-id', 777913, '--address', '--backup', backup,
                                   stdin=phrase + '\n').stdout.strip(), address)
        wrong = self.feed('pocol-sign', *tx, '--backup', backup, stdin='a wrong passphrase\n')
        self.assertNotEqual(wrong.returncode, 0)
        self.assertIn('Wrong passphrase', wrong.stderr)
        self.assertEqual(wrong.stdout, '')
        # Exactly one key source: a backup with a key file or a devnet label, a repeated or valueless --backup.
        for args in [('--backup', backup, '--key-file', key), ('--backup', backup, '--account', 'alice'),
                     ('--backup', backup, '--backup', backup), ('--backup',)]:
            refused = self.feed('pocol-sign', '--chain-id', 777913, '--address', *args, stdin=phrase + '\n')
            self.assertNotEqual(refused.returncode, 0, args)
            self.assertEqual(refused.stdout, '', args)

    def test_ambiguous_and_missing_key_selectors_fail(self):
        path, _ = self.key('a')
        for args in [('--account', 'alice', '--key-file', path), ('--key-file',),
                     ('--key-file', path, '--key-file', path)]:
            self.assertNotEqual(self.call('pocol-sign', '--chain-id', 777913, '--address', *args).returncode, 0)
        for args in [('--miner-key-file',), ('--miner-key-file', path, '--miner', 'alice')]:
            self.assertNotEqual(self.call('pocold', 'run', *args).returncode, 0)

    def test_genesis_uses_only_public_inputs_and_keeps_admission(self):
        config = self.config()
        result = self.init(config)
        self.assertEqual(result.returncode, 0, result.stderr)
        info = json.loads((self.root / 'net/devnet.json').read_text())
        self.assertFalse(info['admission']['specificationCompliant'])
        self.assertEqual(info['admission']['scope'], 'experimental-devnet')
        self.assertEqual(info['registrationPerBlock'], 1)
        self.assertEqual(info['miners'][0]['id'], config['miners'][0]['id'])
        alloc = json.loads((self.root / 'net/alloc.json').read_text())
        self.assertEqual(alloc[config['miners'][0]['reward']]['nonce'], 1)
        self.assertEqual(alloc[config['miners'][0]['reward']]['balance'], config['accounts'][0]['wei'])
        for key in self.root.glob('*.pocol-key'):
            for file in (self.root / 'net').iterdir():
                self.assertNotIn(key.read_bytes().hex(), file.read_text())
        original = (self.root / 'net/genesis.spec.hex').read_bytes()
        self.assertNotEqual(self.init(config).returncode, 0)
        self.assertEqual(original, (self.root / 'net/genesis.spec.hex').read_bytes())

    def test_every_genesis_is_v3_and_the_retired_options_are_refused(self):
        # Phase 6a: init-testnet builds only v3 genesis files (nonceMode 1, the full 64-bit nonce domain, from genesis);
        # "v3": true builds the same bytes, and the retired spec-1 options (fullNonce, continuous, earnin) and "v3": false
        # are refused before any output.
        config = self.config()
        self.assertEqual(self.init(config, 'plain').returncode, 0)
        self.assertEqual(self.init({**config, 'v3': True}, 'stated').returncode, 0)
        for name in ('genesis.spec.hex', 'alloc.json'):
            self.assertEqual((self.root / 'plain' / name).read_bytes(), (self.root / 'stated' / name).read_bytes(), name)
        self.assertEqual(nonce_mode(self.root / 'plain'), 1, 'the full 64-bit nonce domain from genesis')
        info = json.loads((self.root / 'plain/devnet.json').read_text())
        self.assertEqual((info['specVersion'], info['v3']), (2, True))
        for retired in ('fullNonce', 'continuous', 'earnin'):
            self.assertNotIn(retired, info, retired)
        for name, extra in (('full', {'fullNonce': True}), ('bounded', {'fullNonce': False}), ('cont', {'continuous': True}),
                            ('earn', {'earnin': True}), ('spec1', {'v3': False})):
            result = self.init({**config, **extra}, name)
            self.assertNotEqual(result.returncode, 0, name)
            self.assertFalse((self.root / name).exists(), name)
        devnet = ['init-devnet', '--miners', 'a,b,c', '--chain-id', '777912', '--genesis-time', '1793275000']
        self.assertEqual(self.call('pocold', *devnet, '--out', self.root / 'dev').returncode, 0)
        # A devnet is v3 (Phase 5a): nonceMode 1 from genesis.
        self.assertEqual(nonce_mode(self.root / 'dev'), 1)
        info = json.loads((self.root / 'dev' / 'devnet.json').read_text())
        self.assertEqual((info['specVersion'], info['v3']), (2, True))
        self.assertNotIn('fullNonce', info)

    def test_bad_genesis_inputs_never_create_output(self):
        config = self.config()
        cases = [dict(config, chainId=777001), dict(config, registrationPerBlock=255),
                 dict(config, secret='not-permitted'), dict(config, miners=config['miners'] * 2),
                 dict(config, accounts=config['accounts'] * 2), dict(config, miners=[]),
                 dict(config, miners=[dict(config['miners'][0], id='0x0000000000000000000000000000000000c0c006')])]
        for case in cases:
            with self.subTest(case=case):
                result = self.init(case)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.root / 'net').exists())
        # Serde struct parsing must reject duplicate keys, not silently choose the last.
        public = self.root / 'duplicate.json'
        public.write_text(json.dumps(config).replace('"chainId":', '"chainId": 777001, "chainId":'))
        self.assertNotEqual(self.call('pocold', 'init-testnet', '--config', public, '--out', self.root / 'net').returncode, 0)


if __name__ == '__main__':
    unittest.main()
