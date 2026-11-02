#!/usr/bin/env python3
"""new_network_config.py keeps the old miners and registration limit, sets the earn-in identity, and refuses bad input."""
import json, shutil, tempfile, time, unittest
from pathlib import Path
import new_network_config as nn

OLD = {'chainId': 1599846715, 'registrationPerBlock': 1, 'continuous': True,
       'miners': [{'id': '0x' + c * 40, 'reward': '0x' + c * 40} for c in 'abc']}

class NewNetworkConfig(unittest.TestCase):
    def test_build(self):
        cfg = nn.build(OLD, 1795000000)
        self.assertEqual(cfg, {'continuous': True, 'earnin': True, 'chainId': 261597237, 'genesisTime': 1795000000,
                               'registrationPerBlock': 1, 'miners': OLD['miners'], 'accounts': []})

    def test_refusals(self):
        for bad in ({**OLD, 'miners': OLD['miners'][:2]}, {**OLD, 'miners': [{'id': '0x1', 'reward': '0x2'}] * 3},
                    {**OLD, 'registrationPerBlock': 0}, {k: v for k, v in OLD.items() if k != 'registrationPerBlock'}):
            with self.assertRaises(ValueError): nn.build(bad, 1795000000)
        with self.assertRaises(ValueError): nn.build(OLD, 1795000000, chain_id=1599846715)

    def test_cli_writes_a_new_file_only(self):
        d = Path(tempfile.mkdtemp()); self.addCleanup(shutil.rmtree, d)
        (d / 'devnet.json').write_text(json.dumps(OLD))
        out = d / 'new.json'; future = int(time.time()) + 3600
        self.assertEqual(nn.main(['--old', str(d / 'devnet.json'), '--genesis-time', str(future), '--out', str(out)]), 0)
        self.assertEqual(json.loads(out.read_text())['genesisTime'], future)
        with self.assertRaises(SystemExit): nn.main(['--old', str(d / 'devnet.json'), '--genesis-time', str(future), '--out', str(out)])
        with self.assertRaises(SystemExit): nn.main(['--old', str(d / 'devnet.json'), '--genesis-time', '1', '--out', str(d / 'x.json')])

if __name__ == '__main__':
    unittest.main()
