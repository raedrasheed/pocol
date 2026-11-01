#!/usr/bin/env python3
"""Independent-key and public-input genesis controls using release binaries (Unix)."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

BIN = Path(os.environ.get('POCOLD_BIN_DIR', str(Path(__file__).resolve().parents[1] / 'target/release')))


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
