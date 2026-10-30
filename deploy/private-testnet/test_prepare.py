"""Negative controls for deployment boundaries; no servers required."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

from private_testnet import validate_inventory, rpc, check
from runtime import digest, verify


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.inventory = json.loads(Path(__file__).with_name('inventory.example.json').read_text())

    def test_private_inventory(self):
        self.assertEqual(validate_inventory(self.inventory)['chainId'], 777910)

    def test_reject_unsafe_addresses(self):
        for ip in ['0.0.0.0', '8.8.8.8', '127.0.0.1', '169.254.169.254', '::1', 'localhost']:
            with self.subTest(ip=ip), self.assertRaises(ValueError):
                inventory = copy.deepcopy(self.inventory)
                inventory['nodes'][0]['ip'] = ip
                validate_inventory(inventory)

    def test_reject_duplicate_or_injected_names_and_unknown_options(self):
        for name in ['node2', '../node', 'node;touch-x', 'node\nExecStart=x']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                inventory = copy.deepcopy(self.inventory)
                inventory['nodes'][0]['name'] = name
                validate_inventory(inventory)
        self.inventory['nodes'][0]['extraArgs'] = ['--unsafe']
        with self.assertRaises(ValueError):
            validate_inventory(self.inventory)

    def test_reject_network_chain_and_invalid_ports(self):
        for cid in [777001, True, 0, 2**53]:
            with self.subTest(cid=cid), self.assertRaises(ValueError):
                validate_inventory(dict(self.inventory, chainId=cid))
        for port in [True, 22, 65536, '30300']:
            with self.subTest(port=port), self.assertRaises(ValueError):
                inventory = copy.deepcopy(self.inventory)
                inventory['nodes'][0]['p2pPort'] = port
                validate_inventory(inventory)

    def test_reject_rehearsal_as_deployment(self):
        for i, node in enumerate(self.inventory['nodes']):
            node.update(ip='127.0.0.' + str(i + 2), rpcPort=18540 + i)
        validate_inventory(self.inventory, rehearsal=True)
        with self.assertRaises(ValueError):
            validate_inventory(self.inventory)
        self.inventory['nodes'][1]['rpcPort'] = 18540
        with self.assertRaises(ValueError):
            validate_inventory(self.inventory, rehearsal=True)

    def test_rpc_rejects_remote_or_authenticated_urls_before_network(self):
        for url in ['http://10.0.0.1:8540', 'https://127.0.0.1:8540', 'http://user@127.0.0.1:8540']:
            with self.subTest(url=url), self.assertRaises(ValueError):
                rpc(url, 'pocol_status')

    def test_manifest_tamper_missing_and_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            required = ['node.json', 'runtime.py', 'bin/pocold', 'net/devnet.json',
                        'net/genesis.spec.hex', 'net/alloc.json']
            for name in required:
                p = root / name
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text('{}')
            manifest = {'schema': 'pocol-private-bundle-v1',
                        'sha256': {name: digest(root / name) for name in required}}
            def save():
                (root / 'manifest.json').write_text(json.dumps(manifest))
            save()
            self.assertEqual(verify(root), {})
            (root / 'net/alloc.json').write_text('{"tampered":true}')
            with self.assertRaises(ValueError):
                verify(root)
            del manifest['sha256']['net/alloc.json']
            save()
            with self.assertRaises(ValueError):
                verify(root)
            manifest['sha256']['../outside'] = 'irrelevant'
            save()
            with self.assertRaises(ValueError):
                verify(root)

    def test_health_rejects_false_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            release = {'chainId': 7, 'genesisHash': 'expected',
                       'bundleManifestSha256': {'a': 'x', 'b': 'y', 'c': 'z'}}
            (root / 'release.json').write_text(json.dumps(release))
            args = SimpleNamespace(release=root, rpc=[f'{n}=http://127.0.0.1:{9000+i}'
                                    for i, n in enumerate('abc')], observe=1, report=None)
            def status(name):
                return {'chainId': 7, 'genesisHash': 'expected', 'name': name,
                        'stateStore': True, 'peers': ['peer'], 'head': {'height': 10},
                        'admission': {'scope': 'experimental-devnet', 'specificationCompliant': False}}
            for mutation in [{'chainId': 8}, {'peers': []}, {'stateStore': False}, {}]:
                def fake_rpc(url, method, params=None):
                    name = 'abc'[int(url.rsplit(':', 1)[1]) - 9000]
                    return dict(status(name), **mutation)
                with self.subTest(mutation=mutation), patch('private_testnet.rpc', fake_rpc), \
                     patch('private_testnet.time.sleep'), self.assertRaises(ValueError):
                    # The empty mutation still must fail: all heights are stationary.
                    check(args)


if __name__ == '__main__':
    unittest.main()
