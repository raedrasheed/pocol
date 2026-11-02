#!/usr/bin/env python3
"""network_identity.py moves every listed pin, is idempotent and verifiable, and refuses bad input."""
import shutil, tempfile, unittest
from pathlib import Path
import network_identity as ni

NEW = '0x' + 'ab' * 32

class NetworkIdentity(unittest.TestCase):
    def copy(self):
        tmp = Path(tempfile.mkdtemp(prefix='pocol-identity-'))
        self.addCleanup(shutil.rmtree, tmp)
        for rel in ni.FILES:
            (tmp / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ni.ROOT / rel, tmp / rel)
        return tmp

    def test_every_listed_file_carries_the_old_identity_today(self):
        for rel in ni.FILES:
            text = (ni.ROOT / rel).read_text(encoding='utf-8').lower()
            self.assertTrue(ni.OLD_GENESIS in text or str(ni.OLD_CHAIN_ID) in text or f'{ni.OLD_CHAIN_ID:_}' in text, rel)

    def test_update_check_and_idempotence(self):
        root = self.copy()
        self.assertEqual(ni.main(['--genesis-hash', NEW, '--check', '--root', str(root)]), 2, 'check fails before the update')
        self.assertEqual(ni.main(['--genesis-hash', NEW, '--root', str(root)]), 0)
        for rel in ni.FILES:
            text = (root / rel).read_text(encoding='utf-8')
            self.assertNotIn(ni.OLD_GENESIS, text.lower(), rel)
            self.assertNotIn(str(ni.OLD_CHAIN_ID), text, rel)
            self.assertNotIn(f'{ni.OLD_CHAIN_ID:_}', text, rel)
        rust = (root / 'prototypes/pocold/src/network_display.rs').read_text()
        self.assertIn('const MAINNET_CHAIN_ID: u64 = 261_597_237;', rust)
        self.assertIn('ab' * 32, rust)
        self.assertIn('"chainId": 261597237', (root / 'prototypes/pocol-extension/src/lib/studio-network.js').read_text())
        self.assertEqual(ni.main(['--genesis-hash', NEW, '--check', '--root', str(root)]), 0)
        before = {rel: (root / rel).read_bytes() for rel in ni.FILES}
        self.assertEqual(ni.main(['--genesis-hash', NEW, '--root', str(root)]), 0)
        self.assertEqual(before, {rel: (root / rel).read_bytes() for rel in ni.FILES}, 'a second run changes nothing')

    def test_bad_input_is_refused_and_nothing_is_written(self):
        root = self.copy()
        before = {rel: (root / rel).read_bytes() for rel in ni.FILES}
        for args in (['--genesis-hash', '0x1234'], ['--genesis-hash', '0x' + ni.OLD_GENESIS],
                     ['--genesis-hash', NEW, '--chain-id', str(ni.OLD_CHAIN_ID)]):
            with self.assertRaises(SystemExit):
                ni.main(args + ['--root', str(root)])
        self.assertEqual(before, {rel: (root / rel).read_bytes() for rel in ni.FILES})

if __name__ == '__main__':
    unittest.main()
