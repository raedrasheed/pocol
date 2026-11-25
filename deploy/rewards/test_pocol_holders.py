"""The explorer gateway's holders index (pocol_holders.py) against an in-process fake v3 node (holders_fakes.py): the
issuance port, the genesis supply rule, the options, ranking, order, ties, type filters, search, paging, the Total
rule (bonded-only miner IDs, the RewardEscrow decomposition, no double counting), the supply identity and its
verification (an undiscoverable internal transfer, the account count), reorganizations (shallow, across snapshots,
during a snapshot, deeper than the kept window), restarts, a file or node of another network, rewards since the index
start and after a gap, new24h, the statistics history, big integers, query plans and a large synthetic set.
Loopback only (no socket at all); every SQLite connection is closed before the directory is removed (Windows)."""
import hashlib
import json
import os
import sqlite3
import sys
import tempfile
import time
import unittest
import unittest.mock
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pocol_holders as H  # noqa: E402
import holders_fakes as HF  # noqa: E402
import pocol_rewards as R  # noqa: E402

PCOL = HF.PCOL
FEE = 21000 * (10 ** 9 + 1)          # one fake transfer's fee: gas x (base fee + tip)
ID0, ID1, ID2, ID3 = (m for m, _ in HF.FOUNDERS)
RW0, RW1, RW2, RW3 = (r for _, r in HF.FOUNDERS)
TIME0 = HF.TIME0


def wallet(i):
    return '0x' + f'a{i:039x}'


def tx(sender, to, value, **kw):
    return dict({'from': sender, 'to': to, 'value': value}, **kw)


def options(**changes):
    value = {'page': 1, 'size': 100, 'order': 'balance_desc', 'type': 'all', 'q': ''}
    value.update(changes)
    return value


class Base(unittest.TestCase):
    def setUp(self):
        # Cleanups run last-in first-out: the directory goes after every index (and its writer) is closed, since
        # Windows cannot delete an open SQLite file.
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.path = os.path.join(self.dir.name, 'holders.sqlite3')
        self.now = [float(TIME0)]
        self.opened = []
        self.addCleanup(self.close_all)

    def close_all(self):
        for index in self.opened:
            index.close()

    def index(self, chain, path=None, **kw):
        kw.setdefault('pace', 0)
        kw.setdefault('slowdown', 0)
        kw.setdefault('rewards', chain.rewards)
        index = H.HolderIndex(path or self.path, chain.query, chain.network, clock=lambda: self.now[0], **kw)
        self.opened.append(index)
        return index

    @staticmethod
    def rows(index, **kw):
        out, page = [], 1
        while True:
            answer = index.page(options(page=page, **kw))
            out += answer['rows']
            if len(answer['rows']) < answer['size']:
                return out
            page += 1

    @staticmethod
    def settle(index, rounds=4):
        """Steps until a snapshot is published at the history tip (a rescan takes one more round)."""
        for _ in range(rounds):
            index.step()
            status = index.status()
            if status['heights']['snapshot'] == status['heights']['history']:
                stats = index.stats()
                return stats
        raise AssertionError('no snapshot at the tip')

    @staticmethod
    def activity():
        """A chain with transfers, a contract creation and a protocol burn."""
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(1), 5 * PCOL), tx(RW2, wallet(2), 3 * PCOL)])
        chain.mine(txs=[tx(wallet(1), None, PCOL, creates='0x' + 'cc' * 20), tx(RW3, wallet(3), 3 * PCOL)])
        chain.mine(burn=PCOL // 2)
        chain.mine(txs=[tx(wallet(1), wallet(1), 0)])
        return chain


class IssuanceTests(unittest.TestCase):
    """issuance.rs `issuance` and `cumulative`, ported to Python integers."""
    S0, H4, TAIL = PCOL, 4_207_680, PCOL // 32

    def test_the_rust_era_boundary_vectors(self):
        i = lambda h: H.issuance(h, self.S0, self.H4, self.TAIL)
        h = self.H4
        self.assertEqual([i(0), i(1), i(h - 1), i(h), i(2 * h - 1), i(2 * h), i(5 * h - 1)],
                         [0, self.S0, self.S0, self.S0 // 2, self.S0 // 2, self.S0 // 4, self.S0 // 16])
        for height in (5 * h, 6 * h, 255 * h, 256 * h, H.U64_MAX):
            self.assertEqual(i(height), self.TAIL)
        z = lambda height: H.issuance(height, self.S0, h, 0)
        self.assertEqual(z(60 * h), 0)
        self.assertEqual(z(59 * h), self.S0 >> 59)
        self.assertEqual(H.issuance(256 * h, (1 << 256) - 1, h, 0), 0)
        self.assertEqual(H.issuance(256 * h - 1, (1 << 256) - 1, h, 0), 1)
        self.assertEqual(H.issuance(7, self.S0, 0, self.TAIL), self.TAIL)

    def test_the_closed_form_is_the_per_height_sum(self):
        # The Rust test's compressed fixtures, against the port and against the fake's own per-block formula.
        for s, hh, t in ((1000, 4, 31), (1 << 20, 3, 0), (7, 1, 1)):
            direct = 0
            for n in range(0, 400):
                if n:
                    direct += HF.fake_issuance(n, s, hh, t)
                    self.assertEqual(H.issuance(n, s, hh, t), HF.fake_issuance(n, s, hh, t))
                self.assertEqual(H.cumulative(n, s, hh, t), direct, (s, hh, t, n))

    def test_mainnet_edge_heights(self):
        s0, hh, tail = 4 * PCOL, 12_623_040, PCOL // 8
        c = lambda n: H.cumulative(n, s0, hh, tail)
        era0 = (hh - 1) * s0
        self.assertEqual(c(0), 0)
        self.assertEqual(c(1), s0)
        self.assertEqual(c(hh - 1), era0)
        self.assertEqual(c(hh), era0 + s0 // 2)
        self.assertEqual(c(2 * hh), era0 + hh * s0 // 2 + s0 // 4)
        # From era 5 on S0 >> k <= S_tail (4 PCOL / 32 = 0.125 PCOL): every later height issues the tail.
        tail_from = era0 + hh * (s0 // 2 + s0 // 4 + s0 // 8 + s0 // 16)
        for n in (5 * hh, 5 * hh + 1, 6 * hh, 300 * hh, H.U64_MAX):
            self.assertEqual(c(n), tail_from + (n - 5 * hh + 1) * tail, n)
        # testnet.rs supply_bound: 4e24 + 2 S0 H_half + S_tail 2^64, below 2^127, holds every cumulative(h).
        bound = 2 * s0 * hh + tail * (1 << 64)
        self.assertLessEqual(c(H.U64_MAX), bound)
        self.assertLess(4 * 10 ** 24 + bound, 1 << 127)
        self.assertEqual(H.cumulative(9, s0, 0, tail), 9 * tail)


class GenesisTests(unittest.TestCase):
    def test_the_mainnet_spec_gives_four_founders_and_four_million_pcol(self):
        identity = json.loads((HERE.parent / 'websites' / 'site-identity.json').read_text())
        spec = R.hex_bytes(identity['genesisSpec'])
        self.assertEqual('0x' + R.keccak256(spec).hex(),
                         '0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e')
        members = H.genesis_members(spec)
        self.assertEqual(members, [
            ('0x15df38aaba05b83fb8c58d2dd9d804654278a96f', '0x15df38aaba05b83fb8c58d2dd9d804654278a96f'),
            ('0x4479d23cb5580b912a8402230071b8a7f0a701ec', '0xbf335311d1a3ffc8c0efd1dc422f727f48e6de11'),
            ('0xc319eeced588afe03515508bab5f27805534b307', '0x56bd8b1a42a6a3b09c04ebdd91e6fa3cc944b6bd'),
            ('0xe45343ea8d6d54fb693a6ad424fac11d85624b6e', '0x4ce8b7625efc6898e59d7436c3a5a7569e91b3fc')])
        network = R.validated_network({'chainId': 2656089850, 'genesisHash': '0x' + R.keccak256(spec).hex(),
                                       'specVersion': 2, 'v3': True})
        params = {'chainId': 2656089850, 'genesisHash': network.genesis, 'genesisPre': identity['genesisSpec']}
        query = lambda method, p, deadline=None: {'jsonrpc': '2.0', 'id': 1, 'result': params}
        with tempfile.TemporaryDirectory() as folder:
            index = H.HolderIndex(os.path.join(folder, 'h.sqlite3'), query, network, rewards=lambda h: None)
            try:
                index._check_identity()
                self.assertEqual(index.genesis_wei, 4_000_000 * PCOL)
                self.assertEqual((index.cp[15], index.cp[52], index.cp[53], index.cp[25], index.cp[11]),
                                 (4 * PCOL, 12_623_040, PCOL // 8, 360, 4))
            finally:
                index.close()

    def test_record_slot_is_keccak_of_the_padded_id_and_seven(self):
        miner = '0x4479d23cb5580b912a8402230071b8a7f0a701ec'
        expected = R.keccak256(bytes(12) + bytes.fromhex(miner[2:]) + bytes(31) + b'\x07')
        self.assertEqual(H.record_slot(miner), int.from_bytes(expected, 'big'))

    def test_a_spec_without_members_is_refused(self):
        spec = R.rlp_encode([b'\x02', b'\x01', [], b'\x33' * 32, [], b'\x00' * 32])
        with self.assertRaises(H.NodeAnswerInvalid):
            H.genesis_members(spec)


class OptionsTests(unittest.TestCase):
    def test_query_strings(self):
        self.assertEqual(H.holder_options(''), {'page': 1, 'size': 25, 'order': 'balance_desc', 'type': 'all', 'q': ''})
        self.assertEqual(H.holder_options('page=3&size=100&order=balance_asc&type=miner&q=0xAbC&other=1'),
                         {'page': 3, 'size': 100, 'order': 'balance_asc', 'type': 'miner', 'q': '0xabc'})
        pinned = H.holder_options('height=12&hash=0x' + 'AB' * 32)
        self.assertEqual(pinned['snapshot'], {'height': 12, 'hash': '0x' + 'ab' * 32})
        for bad in ('page=0', 'page=1000000001', 'page=x', 'size=10', 'order=newest', 'type=holder', 'q=abc',
                    'q=0x', 'q=0x' + 'a' * 41, 'page=1&page=2', 'page[]=1', 'height=1', 'hash=0x' + 'ab' * 32,
                    'height=-1&hash=0x' + 'ab' * 32, 'height=1&hash=0x12', 'q=' + 'a' * 5000):
            self.assertIsNone(H.holder_options(bad), bad)
        self.assertTrue(H.valid_options(H.holder_options('q=0x12&height=5&hash=0x' + '0' * 64)))

    def test_valid_options_is_exactly_the_shape(self):
        good = options(size=25)
        self.assertTrue(H.valid_options(good))
        for bad in (None, [], dict(good, page=True), dict(good, page=0), dict(good, size=30), dict(good, type='x'),
                    dict(good, order='newest'), dict(good, q='0xABC'), dict(good, extra=1),
                    {k: v for k, v in good.items() if k != 'type'},
                    dict(good, snapshot={'height': 1}), dict(good, snapshot={'height': -1, 'hash': '0x' + '0' * 64}),
                    dict(good, snapshot={'height': 1, 'hash': '0x' + 'A' * 64})):
            self.assertFalse(H.valid_options(bad), bad)


class ContentTests(Base):
    def test_rows_types_links_and_the_total_rule(self):
        chain = self.activity()
        index = self.index(chain)
        stats = self.settle(index)
        rows = {r['address']: r for r in self.rows(index)}
        state = chain.states[chain.head]
        # Founders 1-3: the ID holds no PCOL, only its HELD 20 PCOL bond, and names its distinct reward address.
        for miner, reward in HF.FOUNDERS[1:]:
            self.assertEqual((rows[miner]['type'], rows[miner]['availableWei'], rows[miner]['bondedWei'],
                              rows[miner]['totalWei'], rows[miner]['rewardAddress'], rows[miner]['minerIds']),
                             ('miner', '0', str(HF.B_REG), str(HF.B_REG), reward, []))
            self.assertEqual((rows[reward]['type'], rows[reward]['bondedWei'], rows[reward]['minerIds'],
                              rows[reward]['blocksMined'], rows[reward]['rewardAddress']),
                             ('reward', '0', [miner], None, None))
            self.assertEqual(rows[reward]['availableWei'], str(state.accounts[reward][0]))
        # Founder 0 is its own reward address: one miner row with its balance and its bond.
        self.assertEqual((rows[ID0]['type'], rows[ID0]['totalWei']),
                         ('miner', str(state.accounts[ID0][0] + HF.B_REG)))
        escrow = rows[H.ESCROW]
        held, burned = chain.held_total(state), state.burned
        self.assertEqual((escrow['type'], escrow['label'], escrow['bondedWei']), ('system', 'RewardEscrow', '0'))
        self.assertEqual(int(escrow['totalWei']), state.accounts[H.ESCROW][0] - held - burned)
        self.assertEqual(escrow['availableWei'], str(state.accounts[H.ESCROW][0]))
        self.assertEqual(rows['0x' + 'cc' * 20]['type'], 'contract')
        self.assertEqual(rows[wallet(1)]['type'], 'wallet')
        # Transactions: sent or received once each (the self-transfer counts once), a creation counts for its contract.
        self.assertEqual(rows[wallet(1)]['transactions'], 3)
        self.assertEqual(rows['0x' + 'cc' * 20]['transactions'], 1)
        # No double counting: the sum of every Total is the current supply G - F - P.
        supply = int(stats['supply']['currentSupplyWei'])
        self.assertEqual(sum(int(r['totalWei']) for r in rows.values()), supply)
        self.assertEqual([r['rank'] for r in self.rows(index)], list(range(1, len(rows) + 1)))
        for r in rows.values():
            self.assertEqual(int(r['sharePercentE10']), int(r['totalWei']) * 10 ** 12 // supply)
        # Blocks mined count the miner ID (the block's `miner`), never the reward address.
        mined = {}
        for b in chain.blocks[1:]:
            mined[b['miner']] = mined.get(b['miner'], 0) + 1
        for miner in (ID1, ID2, ID3):
            self.assertEqual(rows[miner]['blocksMined'], mined.get(miner, 0))

    def test_supply_identity_and_statistics(self):
        chain = self.activity()
        index = self.index(chain)
        stats = self.settle(index)
        state = chain.states[chain.head]
        issued = sum(HF.fake_issuance(n, 4 * PCOL, 12_623_040, PCOL // 8) for n in range(1, chain.head + 1))
        fees = sum(int(b['baseFeePerGas'], 16) * int(b['gasUsed'], 16) for b in chain.blocks)
        s = stats['supply']
        self.assertEqual(int(s['genesisWei']), 4_000_000 * PCOL)
        self.assertEqual(int(s['issuedWei']), issued)
        self.assertEqual(int(s['mintedWei']), 4_000_000 * PCOL + issued)
        self.assertEqual(int(s['baseFeeBurnedWei']), fees)
        self.assertEqual(int(s['protocolBurnedWei']), PCOL // 2)
        self.assertEqual(int(s['totalBurnedWei']), fees + PCOL // 2)
        self.assertEqual(int(s['currentSupplyWei']), 4_000_000 * PCOL + issued - fees - PCOL // 2)
        self.assertEqual(int(s['availableWei']), chain.balances_sum())
        self.assertEqual(int(s['bondedWei']), chain.held_total(state))
        self.assertEqual(int(s['pendingRewardsWei']), state.accounts[H.ESCROW][0] - chain.held_total(state) - PCOL // 2)
        system = sum(int(r['totalWei']) for r in self.rows(index, type='system'))
        self.assertEqual(int(s['systemWei']), system)
        self.assertEqual(int(s['circulatingWei']),
                         sum(v[0] for a, v in state.accounts.items() if a not in H.LABELS))
        self.assertEqual(int(s['circulatingWei']) + int(s['bondedWei']) + int(s['systemWei']), int(s['currentSupplyWei']))
        v = stats['verification']
        self.assertEqual(v, {'complete': True, 'balancesSumWei': str(chain.balances_sum()),
                             'expectedSumWei': str(4_000_000 * PCOL + issued - fees), 'unlocatedWei': '0',
                             'accountsIndexed': chain.existing_accounts(state),
                             'accountsOnChain': chain.existing_accounts(state), 'accountsMissing': 0})
        self.assertEqual(set(stats), {'schema', 'chainId', 'genesisHash', 'snapshot', 'index', 'supply', 'verification',
                                      'holders', 'concentration', 'distribution', 'history', 'rewards'})
        self.assertEqual((stats['schema'], stats['chainId'], stats['genesisHash']),
                         (H.STATS_SCHEMA, chain.chain_id, chain.network['genesisHash']))
        # Concentration and holder counts leave out system accounts.
        people = [r for r in self.rows(index) if r['type'] != 'system']
        self.assertEqual(stats['holders']['count'], len(people))
        self.assertEqual(stats['holders']['withBonded'], 4)
        self.assertEqual(stats['holders']['withAvailable'], sum(1 for r in people if r['availableWei'] != '0'))
        for n in (1, 10, 50, 100):
            total = sum(int(r['totalWei']) for r in people[:n])
            self.assertEqual(stats['concentration']['top%d' % n],
                             {'totalWei': str(total), 'sharePercentE10': str(total * 10 ** 12 // int(s['currentSupplyWei']))})
        buckets = stats['distribution']
        self.assertEqual([b['minWei'] for b in buckets], [str(x * PCOL) for x in (0, 1, 10, 100, 1000, 10 ** 4, 10 ** 5, 10 ** 6)])
        self.assertIsNone(buckets[-1]['maxWei'])
        self.assertEqual(sum(b['accounts'] for b in buckets), len(people))
        self.assertEqual(sum(int(b['totalWei']) for b in buckets), sum(int(r['totalWei']) for r in people))
        # Founder 0's account holds its 999,980 PCOL, its 20 PCOL bond and rewards (at least 1M); founders 1-3's
        # reward accounts hold 999,980 PCOL less what they sent, plus rewards (below 1M).
        self.assertEqual((buckets[6]['accounts'], buckets[7]['accounts']), (3, 1))
        self.assertEqual(stats['holders']['new24h'], None)
        self.assertEqual(stats['history']['from'], chain.head)

    def test_ranking_ties_order_and_reverse(self):
        chain = HF.FakeHolderChain()
        # Equal totals: ranked by address ascending in the descending order, the exact reverse in the ascending one.
        chain.mine(txs=[tx(RW1, wallet(9), 2 * PCOL), tx(RW1, wallet(4), 2 * PCOL), tx(RW2, wallet(7), 2 * PCOL)])
        index = self.index(chain)
        self.settle(index)
        desc = self.rows(index)
        ties = [r['address'] for r in desc if r['totalWei'] == str(2 * PCOL)]
        self.assertEqual(ties, [wallet(4), wallet(7), wallet(9)])
        asc = self.rows(index, order='balance_asc')
        self.assertEqual(asc, list(reversed(desc)))
        totals = [int(r['totalWei']) for r in desc]
        self.assertEqual(totals, sorted(totals, reverse=True))

    def test_type_filters_search_and_pages_keep_global_ranks(self):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(i), (i + 1) * PCOL) for i in range(60)])
        chain.mine(txs=[tx(RW2, None, PCOL, creates='0x' + 'cd' * 20)])
        index = self.index(chain)
        self.settle(index)
        everything = self.rows(index)
        for kind in H.TYPES[1:]:
            want = [r for r in everything if r['type'] == kind]
            for order in H.ORDERS:
                got = self.rows(index, type=kind, order=order)
                self.assertEqual(got, want if order == 'balance_desc' else list(reversed(want)), (kind, order))
            self.assertEqual(index.page(options(type=kind))['total'], len(want))
        self.assertEqual(index.page(options())['total'], len(everything))
        # Pages of 25: totals, a last partial page and an empty page past the end.
        pages = [index.page(options(size=25, page=p)) for p in (1, 2, 3, 4)]
        self.assertEqual([len(p['rows']) for p in pages[:3]], [25, 25, len(everything) - 50])
        self.assertEqual(pages[3]['rows'], [])
        self.assertEqual(sum((p['rows'] for p in pages), []), everything)
        asc = [index.page(options(size=25, page=p, order='balance_asc'))['rows'] for p in (1, 2, 3)]
        self.assertEqual(sum(asc, []), list(reversed(everything)))
        # Search: an address prefix, lower-cased by the parser, with the global ranks; with a type; pages of it.
        found = index.page(options(q='0xa0000000000000000000000000000000000000'))
        expected = [r for r in everything if r['address'].startswith('0xa0000000000000000000000000000000000000')]
        self.assertEqual(found['rows'], expected)
        self.assertEqual(found['total'], len(expected))
        self.assertEqual(index.page(options(q='0xa', size=25, page=2, order='balance_asc'))['rows'],
                         list(reversed([r for r in everything if r['address'].startswith('0xa')]))[25:50])
        self.assertEqual(index.page(options(q='0xa', type='miner'))['total'], 0)
        self.assertEqual(index.page(options(q=wallet(3)))['rows'], [r for r in everything if r['address'] == wallet(3)])
        self.assertEqual(index.page(options(q='0xcd', type='contract'))['total'], 1)
        self.assertEqual(index.page(options(q='0xff'))['total'], 0)

    def test_zero_balance_accounts_are_not_holders(self):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(1), PCOL + FEE)])
        chain.mine(txs=[tx(wallet(1), wallet(2), PCOL)])
        index = self.index(chain)
        stats = self.settle(index)
        addresses = {r['address'] for r in self.rows(index)}
        self.assertNotIn(wallet(1), addresses)
        self.assertIn(wallet(2), addresses)
        self.assertNotIn(H.REGISTRY, addresses)  # a system account with no PCOL is not a holder either
        # wallet(1) still exists (nonce 1): the account count compares like with like.
        self.assertEqual(stats['verification']['accountsMissing'], 0)
        self.assertTrue(stats['verification']['complete'])

    def test_undecoded_transactions_have_no_parties(self):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[{'from': RW1, 'to': wallet(5), 'undecoded': True, 'value': 0}])
        index = self.index(chain)
        stats = self.settle(index)
        # The node could not decode it, so its parties are unknown: the sender's balance change is still found (a
        # reward account is refreshed every snapshot) and the identity holds; the recipient got nothing.
        self.assertTrue(stats['verification']['complete'])
        self.assertEqual({r['address']: r['transactions'] for r in self.rows(index)}[RW1], 0)

    def test_bond_accrual_counts_for_the_miner_id(self):
        chain = HF.FakeHolderChain()
        def accruing(state, n):
            state.records[ID1].update(bondState=4, bond=5 * PCOL)
        chain.mine(mutate=accruing, flows=[])
        index = self.index(chain)
        self.settle(index)
        chain.mine(miner=ID1, flows=[(ID1, RW1, 3 * PCOL, 2 * PCOL)])
        self.settle(index)
        rows = {r['address']: r for r in self.rows(index)}
        self.assertEqual((rows[ID1]['bondedWei'], rows[ID1]['rewardsWei'], rows[RW1]['rewardsWei']),
                         (str(7 * PCOL), str(2 * PCOL), str(3 * PCOL)))
        rewards = index.rewards_by_address()
        self.assertEqual(rewards['miners'][ID1], {'paidWei': str(3 * PCOL), 'bondAccruedWei': str(2 * PCOL)})
        self.assertTrue(index.stats()['verification']['complete'])

    def test_big_integers_are_exact(self):
        whale, second = '0x' + 'f1' * 20, '0x' + 'f2' * 20
        chain = HF.FakeHolderChain()
        chain.states[0].accounts[whale] = [2 ** 130, 0, '0x']
        chain.states[0].accounts[second] = [2 ** 129 + 1, 0, '0x']
        chain.mine(txs=[tx(whale, second, 2 ** 64 + 7)])
        genesis = 4_000_000 * PCOL + 2 ** 130 + 2 ** 129 + 1
        index = self.index(chain, genesis_wei=genesis)
        stats = self.settle(index)
        rows = self.rows(index)
        self.assertEqual([r['address'] for r in rows[:2]], [whale, second])
        self.assertEqual(rows[0]['totalWei'], str(2 ** 130 - 2 ** 64 - 7 - FEE))
        self.assertEqual(rows[1]['totalWei'], str(2 ** 129 + 1 + 2 ** 64 + 7))
        supply = int(stats['supply']['currentSupplyWei'])
        self.assertGreater(supply, 2 ** 130)
        self.assertEqual(rows[0]['sharePercentE10'], str((2 ** 130 - 2 ** 64 - 7 - FEE) * 10 ** 12 // supply))
        self.assertEqual(sum(int(r['totalWei']) for r in rows), supply)
        self.assertTrue(stats['verification']['complete'])


class VerificationTests(Base):
    def chain_with_contract(self, wallets=30):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(i), PCOL) for i in range(wallets)])
        chain.mine(txs=[tx(RW2, None, 2 * PCOL, creates='0x' + 'cc' * 20)])
        return chain

    def test_an_undiscoverable_internal_transfer_is_published_as_incomplete(self):
        chain = self.chain_with_contract()
        index = self.index(chain)
        self.assertTrue(self.settle(index)['verification']['complete'])
        hidden = '0x' + '99' * 20
        chain.mine(internal=[('0x' + 'cc' * 20, hidden, PCOL // 4)])
        index.step()   # the difference is new: a full rescan is due, nothing is published at this height
        self.assertEqual(index.status()['heights']['snapshot'], chain.head - 1)
        before = chain.calls.get('eth_getBalance', 0)
        stats = self.settle(index)
        self.assertGreater(chain.calls['eth_getBalance'] - before, 30)   # every known address was read again
        v = stats['verification']
        self.assertEqual((v['complete'], v['unlocatedWei']), (False, str(PCOL // 4)))
        self.assertEqual(int(v['expectedSumWei']) - int(v['balancesSumWei']), PCOL // 4)
        self.assertEqual((v['accountsOnChain'], v['accountsMissing']), (v['accountsIndexed'] + 1, 1))
        self.assertNotIn(hidden, {r['address'] for r in self.rows(index)})
        # The same difference again: published as it is, without another full rescan.
        chain.mine()
        before = chain.calls['eth_getBalance']
        stats = self.settle(index)
        self.assertLess(chain.calls['eth_getBalance'] - before, 30)
        self.assertEqual(stats['verification']['unlocatedWei'], str(PCOL // 4))

    def test_an_internal_transfer_to_a_known_wallet_is_found_by_the_rescan(self):
        chain = self.chain_with_contract()
        index = self.index(chain)
        self.settle(index)
        chain.mine(internal=[('0x' + 'cc' * 20, wallet(3), PCOL // 4)])
        stats = self.settle(index)
        self.assertTrue(stats['verification']['complete'])
        rows = {r['address']: r for r in self.rows(index)}
        self.assertEqual(rows[wallet(3)]['availableWei'], str(PCOL + PCOL // 4))

    def test_the_account_count_is_compared_only_at_the_snapshot_height(self):
        chain = self.chain_with_contract(3)
        chain.caps_extra = 2
        index = self.index(chain)
        v = self.settle(index)['verification']
        self.assertEqual((v['complete'], v['accountsMissing'], v['unlocatedWei']), (False, 2, '0'))
        # pocol_getCaps answering the next head: not compared, and the balance identity alone decides.
        def moved(method, params):
            if method == 'pocol_getCaps':
                chain.on_read = None
                chain.mine()
        chain.mine()
        chain.on_read = moved
        index.step()
        v = index.stats()['verification']
        self.assertEqual((v['complete'], v['accountsOnChain'], v['accountsMissing']), (True, None, None))

    def test_a_periodic_full_rescan(self):
        chain = self.chain_with_contract()
        index = self.index(chain, rescan_seconds=600)
        self.settle(index)
        chain.mine()
        before = chain.calls['eth_getBalance']
        self.settle(index)
        self.assertLess(chain.calls['eth_getBalance'] - before, 30)
        self.now[0] += 601
        chain.mine()
        before = chain.calls['eth_getBalance']
        self.settle(index)
        self.assertGreater(chain.calls['eth_getBalance'] - before, 30)

    def test_batched_reads_give_the_same_answers(self):
        chain = self.chain_with_contract(45)
        single = self.index(chain)
        self.settle(single)
        batched = self.index(chain, path=os.path.join(self.dir.name, 'batched.sqlite3'), batch=chain.batch)
        self.settle(batched)
        self.assertEqual(self.rows(single), self.rows(batched))
        self.assertEqual(single.stats()['supply'], batched.stats()['supply'])


class SnapshotDisciplineTests(Base):
    def test_a_moving_head_around_the_registry_read_discards_the_snapshot(self):
        chain = HF.FakeHolderChain()
        chain.mine_many(3)
        index = self.index(chain)
        def move(method, params):
            if method == 'pocol_getMiners':
                chain.on_read = None
                chain.mine()
        chain.on_read = move
        index.step()
        self.assertIsNone(index.status()['heights']['snapshot'])
        self.settle(index)
        self.assertEqual(index.stats()['snapshot']['height'], 4)

    def test_a_reorganization_during_the_reads_discards_the_snapshot(self):
        chain = HF.FakeHolderChain()
        chain.mine_many(5)
        index = self.index(chain)
        old = chain.blocks[5]['hash']
        def reorg(method, params):
            if method == 'eth_getBalance':
                chain.on_read = None
                chain.reorg(4)
                chain.mine(txs=[tx(RW1, wallet(8), PCOL)])
                chain.mine()
        chain.on_read = reorg
        index.step()
        self.assertIsNone(index.status()['heights']['snapshot'])
        stats = self.settle(index)
        self.assertEqual(stats['snapshot']['height'], 6)
        self.assertNotEqual(chain.blocks[5]['hash'], old)
        self.assertTrue(stats['verification']['complete'])
        self.assertIn(wallet(8), {r['address'] for r in self.rows(index)})

    def test_before_the_first_snapshot_answers_are_unavailable(self):
        chain = HF.FakeHolderChain()
        index = self.index(chain)
        for call in (lambda: index.page(options()), index.stats, index.registry, index.winners):
            with self.assertRaises(H.Unavailable):
                call()
        self.assertIsNone(index.rewards_by_address())
        with self.assertRaises(ValueError):
            index.page(options(size=7))


class ReorgTests(Base):
    def compare_with_fresh(self, chain, index):
        fresh = self.index(chain, path=os.path.join(self.dir.name, 'fresh-%d.sqlite3' % len(self.opened)))
        self.settle(fresh)
        strip = lambda rows: [{k: v for k, v in r.items() if k != 'rewardsWei'} for r in rows]
        self.assertEqual(strip(self.rows(index)), strip(self.rows(fresh)))
        a, b = index.stats(), fresh.stats()
        self.assertEqual((a['supply'], a['verification'], a['snapshot']), (b['supply'], b['verification'], b['snapshot']))

    def test_a_shallow_reorganization_is_rolled_back(self):
        chain = HF.FakeHolderChain()
        chain.mine_many(2)
        index = self.index(chain)
        self.settle(index)
        rewards_from = index.status()['heights']['rewardsFrom']
        chain.mine(txs=[tx(RW1, wallet(1), 4 * PCOL)])
        chain.mine(txs=[tx(wallet(1), wallet(2), PCOL)])
        self.settle(index)
        pinned = index.stats()['snapshot']
        self.assertIn(wallet(2), {r['address'] for r in self.rows(index)})
        chain.reorg(3)
        chain.mine(txs=[tx(RW2, wallet(7), PCOL)], miner=ID3)
        chain.mine(miner=ID3)
        self.settle(index)
        addresses = {r['address']: r for r in self.rows(index)}
        self.assertNotIn(wallet(2), addresses)
        self.assertIn(wallet(7), addresses)
        self.assertEqual(addresses[wallet(1)]['transactions'], 1)
        with self.assertRaises(H.SnapshotChanged):
            index.page(options(snapshot={'height': pinned['height'], 'hash': pinned['hash']}))
        # Rewards: the removed blocks' flows are undone; the kept ones and the new branch's are counted once.
        expected = {}
        for n in range(rewards_from, chain.head + 1):
            for miner, reward, paid, _ in chain.flows[n]:
                expected[reward] = expected.get(reward, 0) + paid
        self.assertEqual({a: int(w) for a, w in index.rewards_by_address()['addresses'].items() if int(w)}, expected)
        self.compare_with_fresh(chain, index)

    def test_a_reorganization_across_snapshots(self):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(0), PCOL)])
        index = self.index(chain)
        snapshots = [self.settle(index)['snapshot']]
        for i in range(1, 6):
            chain.mine(txs=[tx(RW1, wallet(i), PCOL)])
            snapshots.append(self.settle(index)['snapshot'])
        self.assertEqual([s['height'] for s in snapshots], [1, 2, 3, 4, 5, 6])
        chain.reorg(4)
        chain.mine_many(3)
        self.settle(index)
        kept = index.page(options(snapshot={'height': snapshots[3]['height'], 'hash': snapshots[3]['hash']}))
        self.assertEqual(kept['snapshot'], snapshots[3])
        for gone in snapshots[4:]:
            with self.assertRaises(H.SnapshotChanged):
                index.page(options(snapshot={'height': gone['height'], 'hash': gone['hash']}))
        self.compare_with_fresh(chain, index)

    def test_a_reorganization_deeper_than_the_kept_window_indexes_again(self):
        chain = HF.FakeHolderChain()
        chain.mine_many(20, txs=[])
        index = self.index(chain, recent=8)
        self.settle(index)
        chain.reorg(5)
        chain.mine(txs=[tx(RW1, wallet(3), PCOL)])
        chain.mine_many(18)
        with self.assertLogs('pocol_holders', 'WARNING') as logs:
            self.settle(index)
        self.assertTrue(any('indexing again' in line for line in logs.output))
        self.compare_with_fresh(chain, index)

    def test_a_node_behind_the_index_rolls_it_back(self):
        chain = HF.FakeHolderChain()
        chain.mine_many(6)
        index = self.index(chain)
        self.settle(index)
        chain.reorg(3)
        chain.mine()
        self.settle(index)
        self.assertEqual(index.status()['heights']['history'], 4)
        self.compare_with_fresh(chain, index)


class RestartAndIdentityTests(Base):
    def test_a_restart_resumes_from_the_stored_tip(self):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(1), PCOL)])
        chain.mine_many(4)
        first = self.index(chain)
        self.settle(first)
        rewards_from = first.status()['heights']['rewardsFrom']
        rows = self.rows(first)
        first.close()
        self.opened.remove(first)
        chain.mine_many(2)
        before = chain.calls['eth_getBlockByNumber']
        second = self.index(chain)
        self.settle(second)
        # Only the new blocks (and the head and fork checks) are read: nothing from genesis again.
        self.assertLess(chain.calls['eth_getBlockByNumber'] - before, 12)
        self.assertEqual(second.status()['heights']['rewardsFrom'], rewards_from)
        self.assertLessEqual({r['address'] for r in rows}, {r['address'] for r in self.rows(second)})
        self.assertTrue(second.stats()['verification']['complete'])

    def test_a_file_of_another_network_is_refused_and_left_closed(self):
        chain = HF.FakeHolderChain()
        index = self.index(chain)
        index.close()
        self.opened.remove(index)
        other = HF.FakeHolderChain(chain_id=777965)
        with self.assertRaisesRegex(RuntimeError, 'another network'):
            H.HolderIndex(self.path, other.query, other.network)
        # The refused open left nothing open: the file can be removed (Windows) and opened again for its network.
        again = self.index(chain)
        again.close()
        self.opened.remove(again)
        for suffix in ('', '-wal', '-shm'):
            if os.path.exists(self.path + suffix):
                os.remove(self.path + suffix)

    def test_a_node_of_another_network_is_refused(self):
        chain = HF.FakeHolderChain()
        other = HF.FakeHolderChain(chain_id=777965)
        index = H.HolderIndex(self.path, other.query, chain.network, clock=lambda: self.now[0], pace=0)
        self.opened.append(index)
        with self.assertRaisesRegex(RuntimeError, 'does not belong'):
            index.step()

    def test_the_follower_thread_publishes_and_stops(self):
        chain = HF.FakeHolderChain()
        chain.mine_many(3)
        index = self.index(chain, poll=0.01)
        index.start()
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline and index.status()['heights']['snapshot'] != 3:
            time.sleep(0.01)
        index.stop()
        self.assertEqual(index.status()['heights']['snapshot'], 3)
        self.assertEqual(index.page(options())['index']['state'], 'ready')
        self.now[0] += H.STALE_SECONDS + 1
        self.assertEqual(index.page(options())['index']['state'], 'reconnecting')


class RewardsTests(Base):
    def test_rewards_since_the_index_start_then_null_after_a_gap(self):
        chain = HF.FakeHolderChain()
        chain.mine_many(10)
        index = self.index(chain)
        self.settle(index)
        self.assertEqual(index.status()['heights']['rewardsFrom'], 10)
        for _ in range(3):
            chain.mine()
            self.settle(index)
        expected = {}
        for n in range(10, 14):
            for miner, reward, paid, _ in chain.flows[n]:
                expected[reward] = expected.get(reward, 0) + paid
        got = index.rewards_by_address()
        self.assertEqual((got['from'], got['to']), (10, 13))
        self.assertEqual({a: int(w) for a, w in got['addresses'].items()}, expected)
        rows = {r['address']: r for r in self.rows(index)}
        for address, amount in expected.items():
            self.assertEqual(rows[address]['rewardsWei'], str(amount))
        self.assertEqual(rows[ID0]['rewardsWei'], str(expected.get(ID0, 0)))
        self.assertEqual(index.stats()['rewards'], {'from': 10, 'to': 13})
        # A failed read is retried while the block is still inside the window.
        chain.rewards_fail = True
        chain.mine()
        index.step()
        chain.rewards_fail = False
        self.settle(index)
        self.assertEqual(index.rewards_by_address()['to'], 14)
        # A gap longer than the state window: rewards are null from then on.
        chain.mine_many(40)
        self.settle(index)
        self.assertIsNone(index.rewards_by_address())
        self.assertTrue(all(r['rewardsWei'] is None for r in self.rows(index)))
        self.assertEqual(index.stats()['rewards'], {'from': None, 'to': None})
        chain.mine()
        self.settle(index)
        self.assertIsNone(index.rewards_by_address())
        # The operator's reset: rewards count again from the next followed block.
        index.reset_rewards()
        chain.mine()
        self.settle(index)
        self.assertEqual(index.rewards_by_address()['from'], chain.head)

    def test_the_default_rewards_reader_is_snapshot_v3_of_one_block(self):
        chain = HF.FakeHolderChain()
        with unittest.mock.patch.object(H, 'snapshot_v3', return_value={'x': 1}) as snapshot:
            reader = H.default_rewards(chain.query, R.validated_network(chain.network))
            self.assertEqual(reader(7), {'x': 1})
        snapshot.assert_called_once_with(chain.query, fork_schedule=R.DEFAULT_FORK_SCHEDULE,
                                         network=R.validated_network(chain.network), height=7)

    def test_reward_flows_are_checked(self):
        good = {'blockHeight': 5, 'blockHash': '0x' + '11' * 32,
                'block': {'beneficiaries': [{'minerId': ID1, 'rewardAddress': RW1, 'amountWei': '10',
                                             'bondAccruedWei': '2'}]}}
        self.assertEqual(H.reward_flows(good, 5, '0x' + '11' * 32), [(ID1, RW1, 10, 2)])
        for bad in (dict(good, blockHeight=6), dict(good, blockHash='0x' + '22' * 32), dict(good, block={}),
                    dict(good, block={'beneficiaries': [{'minerId': ID1, 'rewardAddress': RW1, 'amountWei': '-1'}]}),
                    dict(good, block={'beneficiaries': [{'minerId': ID1, 'rewardAddress': RW1, 'amountWei': '01'}]}),
                    dict(good, block={'beneficiaries': [{'minerId': 'x', 'rewardAddress': RW1, 'amountWei': '1'}]})):
            with self.assertRaises(H.NodeAnswerInvalid):
                H.reward_flows(bad, 5, '0x' + '11' * 32)


class TimeSeriesTests(Base):
    HOUR = 3600

    def test_new24h_only_after_the_index_covered_the_whole_day(self):
        chain = HF.FakeHolderChain()
        chain.mine(ts=TIME0 + self.HOUR)
        index = self.index(chain)
        self.settle(index)                                  # covered from height 1
        for n in range(2, 30):
            txs = [tx(RW1, wallet(n), PCOL)] if n in (3, 26) else []
            chain.mine(txs=txs, ts=TIME0 + n * self.HOUR)
            holders = self.settle(index)['holders']
            start = n - 24                                  # the first block of the last 24 h of block time
            if start < 1:
                self.assertEqual((holders['new24h'], holders['new24hFrom']), (None, None), n)
            else:
                expected = sum(1 for k in (3, 26) if start <= k <= n)
                self.assertEqual((holders['new24h'], holders['new24hFrom']), (expected, start), n)
        # A gap in the snapshots longer than the state window: not covered again until a whole day has passed.
        chain.mine_many(40, ts=None)
        self.assertEqual(self.settle(index)['holders']['new24h'], None)

    def test_history_points_are_hourly_and_thinned(self):
        chain = HF.FakeHolderChain()
        chain.mine()
        index = self.index(chain)
        with unittest.mock.patch.object(H, 'HISTORY_POINTS_MAX', 10):
            self.settle(index)
            chain.mine()                                     # ten seconds later: no new point
            self.assertEqual(len(self.settle(index)['history']['points']), 1)
            for n in range(1, 30):
                chain.mine(ts=int(chain.blocks[-1]['timestamp'], 16) + self.HOUR)
                history = self.settle(index)['history']
                self.assertLessEqual(len(history['points']), 10)
            heights = [p['height'] for p in history['points']]
            self.assertEqual(heights[0], 1)
            self.assertEqual(history['from'], 1)
            self.assertEqual(heights, sorted(heights))
            self.assertEqual(heights[-1], chain.head)
            point = history['points'][-1]
            stats = index.stats()
            self.assertEqual((point['holders'], point['currentSupplyWei'], point['top10SharePercentE10']),
                             (stats['holders']['count'], stats['supply']['currentSupplyWei'],
                              stats['concentration']['top10']['sharePercentE10']))

    def test_snapshots_are_kept_three_and_three_minutes(self):
        chain = HF.FakeHolderChain()
        index = self.index(chain)
        snapshots = []
        for _ in range(5):
            chain.mine(txs=[tx(RW1, wallet(len(snapshots)), PCOL)])
            snapshots.append(self.settle(index)['snapshot'])
            self.now[0] += 10
        for s in snapshots:  # all younger than 180 s
            index.page(options(snapshot={'height': s['height'], 'hash': s['hash']}))
        pinned = index.page(options(size=25, page=1, snapshot={'height': snapshots[0]['height'],
                                                                'hash': snapshots[0]['hash']}))
        self.assertEqual(pinned['snapshot'], snapshots[0])
        self.assertEqual(len(pinned['rows']), pinned['total'])
        self.now[0] += 200
        chain.mine()
        latest = self.settle(index)['snapshot']
        kept = snapshots[3:] + [latest]
        for s in kept:
            index.page(options(snapshot={'height': s['height'], 'hash': s['hash']}))
        for s in snapshots[:3]:
            with self.assertRaises(H.SnapshotChanged):
                index.page(options(snapshot={'height': s['height'], 'hash': s['hash']}))

    def test_a_pinned_page_does_not_change(self):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(i), (i + 1) * PCOL) for i in range(40)])
        index = self.index(chain)
        self.settle(index)
        first = index.page(options(size=25, page=2))
        chain.mine(txs=[tx(RW2, wallet(i), 50 * PCOL) for i in range(0, 40, 3)])
        self.settle(index)
        snap = first['snapshot']
        again = index.page(options(size=25, page=2, snapshot={'height': snap['height'], 'hash': snap['hash']}))
        self.assertEqual(again['rows'], first['rows'])
        self.assertNotEqual(index.page(options(size=25, page=2))['rows'], first['rows'])


class RegistryTests(Base):
    def test_registry_active_inactive_and_winners(self):
        chain = HF.FakeHolderChain(cp={25: 5})
        exited, punished = '0x' + '71' * 20, '0x' + '72' * 20
        def setup(state, n):
            state.records[ID1]['lastShare'] = 7
            state.records[ID3]['graceStart'] = 6
            state.records[exited] = {'status': 2, 'tomb': False, 'bondState': 3, 'lastShare': 1, 'bond': HF.B_REG,
                                     'registeredAt': 1, 'reward': exited, 'graceStart': 0}
            state.records[punished] = {'status': 1, 'tomb': True, 'bondState': 2, 'lastShare': 1, 'bond': HF.B_REG,
                                       'registeredAt': 1, 'reward': punished, 'graceStart': 0}
        chain.mine_many(6)
        chain.mine(mutate=setup)
        chain.mine()
        index = self.index(chain)
        self.settle(index)
        registry = index.registry()
        self.assertEqual((registry['height'], registry['blockHash'], registry['thetaInact']),
                         (8, chain.blocks[8]['hash'], 5))
        records = {r['id']: r for r in registry['records']}
        self.assertEqual(set(records[ID0]), {'id', 'reward', 'status', 'tomb', 'bondState', 'lastShare', 'graceStart',
                                             'registeredAt', 'active', 'inactive'})
        # h + 1 = 9 >= max(lastShare, graceStart) + 5 is inactive.
        self.assertEqual((records[ID0]['active'], records[ID0]['inactive']), (True, True))
        self.assertEqual((records[ID1]['active'], records[ID1]['inactive']), (True, False))   # share at 7
        self.assertEqual((records[ID3]['active'], records[ID3]['inactive'], records[ID3]['graceStart']), (True, False, 6))
        self.assertEqual((records[exited]['active'], records[exited]['inactive'], records[exited]['graceStart']),
                         (False, False, None))
        self.assertEqual((records[punished]['active'], records[punished]['tomb']), (False, True))
        rows = {r['address']: r for r in self.rows(index)}
        # A tomb's bond (BURNED) and a refunded one (PAID) are not held: those IDs hold nothing and are not holders.
        self.assertNotIn(punished, rows)
        self.assertNotIn(exited, rows)
        self.assertTrue(index.stats()['verification']['complete'])
        winners = index.winners()
        self.assertEqual((winners['from'], winners['to'], winners['blocks']), (1, 8, 8))
        self.assertEqual(sum(winners['perMiner'].values()), 8)
        self.assertEqual(winners['perMiner'], {m: sum(1 for b in chain.blocks[1:] if b['miner'] == m)
                                               for m in sorted({b['miner'] for b in chain.blocks[1:]})})
        last2 = index.winners(window=2)
        self.assertEqual((last2['from'], last2['to'], last2['blocks']), (7, 8, 2))
        with self.assertRaises(ValueError):
            index.winners(window=0)


class ScaleTests(Base):
    def test_page_queries_use_indexes_and_never_sort(self):
        chain = HF.FakeHolderChain()
        chain.mine(txs=[tx(RW1, wallet(i), PCOL) for i in range(30)])
        index = self.index(chain)
        self.settle(index)
        conn = sqlite3.connect(self.path)
        try:
            args = {'all_desc': (1, 1, 25), 'all_asc': (1, 1, 25), 'type_desc': (1, 'wallet', 1, 25),
                    'type_asc': (1, 'wallet', 1, 25), 'search': (1, '0xa', '0xag'),
                    'search_type': (1, '0xa', '0xag', 'wallet'), 'row': (1, 3), 'newest': (),
                    'pinned': (1, '0x' + '00' * 32), 'canonical': (1,)}
            self.assertEqual(set(args), set(H.PAGE_SQL))
            for name, sql in H.PAGE_SQL.items():
                plan = ' | '.join(r[-1] for r in conn.execute('EXPLAIN QUERY PLAN ' + sql, args[name]))
                self.assertNotIn('TEMP B-TREE', plan, name)
                if 'FROM holders' in sql:
                    self.assertIn('SEARCH holders USING', plan, name)
                    self.assertNotIn('SCAN holders', plan, name)
        finally:
            conn.close()

    def test_a_large_synthetic_set(self):
        count = 100_000
        static = {'0x' + hashlib.sha256(i.to_bytes(4, 'big')).hexdigest()[:40]: (i % 997 + 1) * 10 ** 15
                  for i in range(count)}
        self.assertEqual(len(static), count)
        chain = HF.FakeHolderChain(static=static)
        chain.mine()
        index = self.index(chain, genesis_wei=4_000_000 * PCOL + sum(static.values()))
        index.step()   # history, rewards and a first (full) snapshot of the founders' chain
        # The large set seeded as known accounts already read at this height (as after earlier snapshots), so the
        # next snapshot reads only its dirty set and the test measures the materialization and the pages.
        conn = index.writer
        conn.execute('BEGIN IMMEDIATE')
        conn.executemany('INSERT INTO accounts VALUES(?,0,1,0,?,1,0,?)',
                         [(a, str(v), chain.head) for a, v in static.items()])
        conn.execute("INSERT OR REPLACE INTO meta VALUES('rescan_at', ?)", (str(self.now[0] + 10 ** 6),))
        conn.execute("INSERT OR REPLACE INTO meta VALUES('full', '0')")
        conn.execute('COMMIT')
        chain.mine(txs=[tx(RW1, wallet(1), PCOL)])
        started = time.monotonic()
        stats = self.settle(index)
        publish = time.monotonic() - started
        self.assertTrue(stats['verification']['complete'], stats['verification'])
        self.assertEqual(stats['holders']['count'], count + 4 + 3 + 1)
        timings = {}
        def timed(name, **kw):
            started = time.monotonic()
            answer = index.page(options(**kw))
            timings[name] = time.monotonic() - started
            return answer
        first = timed('page 1', size=100)
        deep = timed('deep page', size=100, page=900)
        last = timed('last page asc', size=100, order='balance_asc')
        wallets = timed('wallet filter page 500', size=50, type='wallet', page=500)
        search1 = timed('search 0xb (one digit)', q='0xb', size=25, page=100)
        search3 = timed('search 0xb12 (three digits)', q='0xb12', size=25)
        self.assertEqual(first['total'], stats['holders']['count'] + 1)
        self.assertEqual([r['rank'] for r in deep['rows']], list(range(89_901, 90_001)))
        self.assertEqual(last['rows'][0]['rank'], first['total'])
        self.assertEqual(len(wallets['rows']), 50)
        self.assertGreater(search1['total'], 5000)
        self.assertTrue(search3['rows'] and all(r['address'].startswith('0xb12') for r in search3['rows']))
        self.assertEqual(search3['total'], sum(1 for a in static if a.startswith('0xb12')))
        sys.stderr.write('\n  holders scale (%d accounts): publish %.2f s; ' % (count, publish)
                         + '; '.join('%s %.4f s' % kv for kv in timings.items()) + '\n')
        self.assertLess(publish, 120)
        for name, seconds in timings.items():
            self.assertLess(seconds, 1.0, name)


if __name__ == '__main__':
    unittest.main()
