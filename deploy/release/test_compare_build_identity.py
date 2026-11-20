#!/usr/bin/env python3
"""compare_build_identity.py: agreement passes and every kind of disagreement is named (no binaries needed; the
workflow runs the genesis step itself on Linux and Windows)."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import compare_build_identity as cbi  # noqa: E402
import pocol_identity  # noqa: E402  (compare_build_identity put deploy/public-testnet on the path)

BUILD = {'schema': 'pocol-testnet-build-v1', 'compatibilitySha256': 'ab' * 32, 'consensusPinned': True, 'consensusRulesVersion': 2,
         'consensusSourcesSha256': 'cd' * 32, 'features': [], 'maxForkVersion': 3, 'protocolVersion': 2, 'protocolVersionMin': 2,
         'scope': 'mainnet', 'version': '0.5.0'}
INFO = {'chainId': cbi.MAINNET_CHAIN_ID, 'genesisHash': '0x' + '11' * 32, 'forks': [[1, 0]], 'specVersion': 2, 'v3': True}
SUMMARY = {'chainId': cbi.MAINNET_CHAIN_ID, 'genesisHash': INFO['genesisHash'], 'scope': 'public-network', 'profileKind': 'network',
           'nodeKind': 'node', 'specificationCompliant': False}


class Compare(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def fresh(self):
        """A new empty directory for one sub-test's files."""
        self.root = Path(tempfile.mkdtemp(dir=self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def genesis(self, label, info=INFO, summary=SUMMARY, build=BUILD, identity=None):
        d = self.root / label
        d.mkdir()
        (d / 'devnet.json').write_text(json.dumps(info))
        (d / 'summary.json').write_text(json.dumps(summary))
        (d / 'described.json').write_text(json.dumps({'consensusIdentity': identity or pocol_identity.for_network(info, build)}))
        (d / 'build-info.json').write_text(json.dumps(build))
        return d

    def build(self, label, build=BUILD):
        p = self.root / f'{label}.json'
        p.write_text(json.dumps(build))
        return p

    def run_compare(self, geneses, builds):
        return cbi.compare(geneses, builds)

    def three(self, android=BUILD):
        g = {'Linux': self.genesis('Linux'), 'Windows': self.genesis('Windows')}
        b = {'Linux': self.build('Linux'), 'Windows': self.build('Windows'), 'Android': self.build('Android', android)}
        return g, b

    def test_identical_builds_agree_and_send_one_identity(self):
        r = self.run_compare(*self.three())
        self.assertTrue(r['agree'], r['problems'])
        self.assertEqual(len({b['consensusIdentity'] for b in r['builds'].values()}), 1)
        self.assertEqual(set(r['builds']), {'Linux', 'Windows', 'Android'})

    def test_each_identity_or_digest_field_that_differs_is_named(self):
        for field, value in [('consensusRulesVersion', 3), ('protocolVersionMin', 1), ('maxForkVersion', 2), ('consensusSourcesSha256', 'ef' * 32),
                             ('compatibilitySha256', '00' * 32), ('version', '0.5.1')]:
            with self.subTest(field=field):
                self.fresh()
                android = dict(BUILD, **{field: value})
                r = self.run_compare(*self.three(android))
                self.assertFalse(r['agree'])
                self.assertTrue(any(p.startswith(f'{field} differs') for p in r['problems']), r['problems'])

    def test_a_test_feature_or_an_unpinned_build_never_agrees(self):
        for android, word in [(dict(BUILD, features=['caps-test']), 'test-only features'), (dict(BUILD, consensusPinned=False), 'unpinned')]:
            with self.subTest(word=word):
                self.fresh()
                r = self.run_compare(*self.three(android))
                self.assertFalse(r['agree'])
                self.assertTrue(any(word in p for p in r['problems']), r['problems'])

    def test_a_missing_field_never_agrees(self):
        android = {k: v for k, v in BUILD.items() if k != 'consensusSourcesSha256'}
        r = self.run_compare(*self.three(android))
        self.assertFalse(r['agree'])
        self.assertTrue(any(p.startswith('consensusSourcesSha256 missing') for p in r['problems']), r['problems'])

    def test_platforms_that_built_different_genesis_bytes_disagree(self):
        other = dict(INFO, genesisHash='0x' + '22' * 32)
        g = {'Linux': self.genesis('Linux'), 'Windows': self.genesis('Windows', other, dict(SUMMARY, genesisHash=other['genesisHash']))}
        r = self.run_compare(g, {'Linux': self.build('Linux')})
        self.assertFalse(r['agree'])
        self.assertTrue(any('different genesis bytes' in p for p in r['problems']), r['problems'])

    def test_another_admission_or_chain_disagrees(self):
        for summary in [dict(SUMMARY, scope='experimental-devnet'), dict(SUMMARY, profileKind='simnet', nodeKind='simnet'),
                        dict(SUMMARY, chainId=1656550731), dict(SUMMARY, specificationCompliant=True)]:
            with self.subTest(summary=summary):
                self.fresh()
                r = self.run_compare({'Linux': self.genesis('Linux', summary=summary)}, {'Linux': self.build('Linux')})
                self.assertFalse(r['agree'])
                self.assertTrue(any('not the public network admission' in p for p in r['problems']), r['problems'])

    def test_a_binary_identity_unlike_the_reference_implementation_disagrees(self):
        r = self.run_compare({'Linux': self.genesis('Linux', identity='00' * 32)}, {'Linux': self.build('Linux')})
        self.assertFalse(r['agree'])
        self.assertTrue(any('reference implementation another' in p for p in r['problems']), r['problems'])

    def test_a_build_info_unlike_the_one_its_genesis_step_recorded_disagrees(self):
        g = {'Linux': self.genesis('Linux')}
        r = self.run_compare(g, {'Linux': self.build('Linux', dict(BUILD, compatibilitySha256='99' * 32))})
        self.assertFalse(r['agree'])
        self.assertTrue(any('differs from the one its genesis step recorded' in p for p in r['problems']), r['problems'])

    def test_nothing_to_compare_never_agrees(self):
        self.assertFalse(self.run_compare({}, {'Android': self.build('Android')})['agree'])
        self.assertFalse(self.run_compare({'Linux': self.genesis('Linux')}, {})['agree'])

    def test_the_filled_profile_keeps_the_mainnet_chain_with_keyless_throwaway_founders(self):
        p = cbi.filled_profile()
        self.assertEqual(p['chainId'], cbi.MAINNET_CHAIN_ID)
        self.assertNotIn('PLACEHOLDER', json.dumps(p))
        ids = [f['id'] for f in p['founders']] + [f['reward'] for f in p['founders']]
        self.assertEqual(len(set(ids)), 8)
        self.assertTrue(all(len(a) == 42 and a.startswith('0x') for a in ids))
        self.assertEqual(cbi.filled_profile(), copy.deepcopy(p), 'fixed inputs: every platform builds the same genesis')

    def test_labels_must_be_distinct(self):
        with self.assertRaises(SystemExit):
            cbi.labelled(['Linux=a', 'Linux=b'])
        with self.assertRaises(SystemExit):
            cbi.labelled(['no-label'])


if __name__ == '__main__':
    unittest.main()
