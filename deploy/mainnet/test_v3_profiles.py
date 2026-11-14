"""The fresh network's committed profiles (FN-4; development/decisions/FRESH-NETWORK-CP.md).

v3-profile.json (mainnet) and v3-rehearsal-profile.json differ only in their chain id and launch second; the two chain
ids are the FN-0 draws, in [2^24, 2^32), distinct and outside the excluded set; the ceremony inputs are placeholders
and nothing else is; every founder's allocation plus the 20 PCOL bond is 10^24 wei. The builder's own checks and the
ParamGate admission are in prototypes/pocold/tests/fn4_profiles.rs.
"""
import json
import pathlib
import unittest

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
PCOL = 10**18


def load(name):
    return json.loads((HERE / name).read_text(encoding='utf-8'))


def placeholders(value, path=''):
    if isinstance(value, str):
        return [path or '/'] if value.startswith('PLACEHOLDER') else []
    if isinstance(value, list):
        return [p for i, v in enumerate(value) for p in placeholders(v, f'{path}/{i}')]
    if isinstance(value, dict):
        return [p for k, v in value.items() for p in placeholders(v, f'{path}/{k}')]
    return []


class V3Profiles(unittest.TestCase):
    def setUp(self):
        self.mainnet = load('v3-profile.json')
        self.rehearsal = load('v3-rehearsal-profile.json')

    def test_the_profiles_differ_only_in_chain_id_and_launch_second(self):
        strip = lambda p: {k: v for k, v in p.items() if k not in ('chainId', 'genesisTime')}
        self.assertEqual(strip(self.mainnet), strip(self.rehearsal))
        self.assertTrue(self.mainnet['v3'])
        self.assertEqual(self.mainnet['record'], 'development/decisions/FRESH-NETWORK-CP.md')
        self.assertTrue((ROOT / self.mainnet['record']).is_file())

    def test_the_chain_ids_are_the_fn0_draws(self):
        ids = (self.mainnet['chainId'], self.rehearsal['chainId'])
        self.assertEqual(ids, (2656089850, 1656550731))
        self.assertNotEqual(*ids)
        retired = {e['chainId'] for e in json.loads((HERE / 'retired-networks.json').read_text(encoding='utf-8'))['retired']}
        self.assertTrue(retired)
        record = (ROOT / self.mainnet['record']).read_text(encoding='utf-8')
        for chain_id in ids:
            self.assertTrue(2**24 <= chain_id < 2**32, chain_id)
            self.assertNotIn(chain_id, {777001, 261597237, 1234567891} | retired)
            self.assertFalse(777900 <= chain_id <= 777999, chain_id)
            self.assertIn(f'**{chain_id}**', record)

    def test_only_the_ceremony_inputs_are_placeholders(self):
        for profile in (self.mainnet, self.rehearsal):
            want = ['/genesisTime', '/hashRate/hashesPerSecond'] + [f'/founders/{i}/{f}' for i in range(4) for f in ('id', 'reward')]
            self.assertEqual(sorted(placeholders(profile)), sorted(want))
            self.assertEqual(profile['hashRate']['class'], 'measurement')

    def test_four_founders_with_the_bond_from_their_million(self):
        cp = self.mainnet['cp']
        self.assertEqual(int(cp['B_reg']), 20 * PCOL)
        self.assertEqual(len(self.mainnet['founders']), 4)
        for founder in self.mainnet['founders']:
            self.assertEqual(int(founder['allocationWei']) + int(cp['B_reg']), 10**24)
        self.assertEqual(self.mainnet['registrationPerBlock'], cp['R_max'])

    def test_the_cp_values_that_changed_from_the_draft(self):
        cp = self.mainnet['cp']
        self.assertEqual(len(cp), 57, 'CP schema v2 has 59 fields; g_ts and target_g come from the ceremony')
        self.assertEqual((cp['T_blk'], cp['D_att'], cp['dFbWait'], cp['M_max'], cp['R_max'], cp['Z_close']), (10, 90, 8, 10000, 4, 256))
        self.assertEqual((int(cp['subsidy']), int(cp['S_tail']), cp['H_half']), (4 * PCOL, 4 * PCOL // 32, 12623040))
        self.assertEqual((cp['Theta_inact'], cp['E_win'], cp['U'], cp['C_react']), (360, 360, 380, 380))
        self.assertGreaterEqual(cp['Z_close'], -(-cp['REG_RECORDS_MAX'] // cp['RET']) + cp['P2'] + cp['D'] + 1)
        self.assertEqual((cp['ACCT_CAP'], cp['SLOT_CAP'], cp['CODE_CAP']), (1000000, 4000000, 2**30))
        for gone in ('P_MAX', 'T_PROB', 'F0_acct', 'F0_slot', 'F0_code', 'E_epoch', 'theta_bp', 'k_sigma'):
            self.assertNotIn(gone, cp)


if __name__ == '__main__':
    unittest.main()
