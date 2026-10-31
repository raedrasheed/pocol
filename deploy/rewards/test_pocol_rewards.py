"""Reward snapshot tests against an independent deterministic RPC fixture.

The fixture uses the published v1 header layout and nonce-ticket formulas, not
the module's header decoder or owner reconstruction. No network is contacted.
Run: python3 -m unittest -v test_pocol_rewards.py
"""
import copy
import hashlib
import time
import unittest

import pocol_rewards as rewards


CHAIN_ID = 1599846715
GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
IDS = ['0x' + x * 40 for x in ('1', '2', '3')]
REGISTRY = '0x0000000000000000000000000000000000c0c001'
ESCROW = '0x0000000000000000000000000000000000c0c002'
CAPS = '0x0000000000000000000000000000000000c0c004'
U256 = (1 << 256) - 1


def uint(value):
    return value.to_bytes((value.bit_length() + 7) // 8, 'big')


def encode(value):
    """Independent canonical RLP fixture builder."""
    if isinstance(value, int):
        value = uint(value)
    if isinstance(value, list):
        body = b''.join(encode(v) for v in value)
        base = 0xc0
    else:
        body, base = value, 0x80
        if len(body) == 1 and body[0] < 0x80:
            return body
    if len(body) < 56:
        return bytes([base + len(body)]) + body
    size = uint(len(body))
    return bytes([base + 55 + len(size)]) + size + body


def word(value):
    if isinstance(value, str):
        value = int(value, 16)
    return value.to_bytes(32, 'big')


def abi(*values):
    return '0x' + b''.join(word(v) for v in values).hex()


def selector(signature):
    return rewards.keccak256(signature.encode())[:4]


def header(height, parent, nonce, shares, proposer=None, *, version=1, target=None):
    target = (1 << 256) // 100 - 1 if target is None else target
    timestamp = int(time.time()) - (20 - height) * 8
    ut = [b'PoCol-tpl-v1', CHAIN_ID, bytes.fromhex(GENESIS[2:]),
          bytes.fromhex(parent[2:]), height, 0, version, timestamp, target,
          b'\x44' * 32, b'\x55' * 32, b'\x66' * 32, b'\0' * 256,
          8_000_000, 0, 7, b'\x77' * 32,
          bytes.fromhex(proposer[2:]) if proposer else b'']
    nonce_bytes = nonce.to_bytes(8, 'big')
    share_bytes = [n.to_bytes(8, 'big') for n in shares]
    raw = encode([[ut, b'\x11' * 65 if proposer else b''],
                  nonce_bytes, share_bytes, b'\x22' * 65])
    tid = hashlib.sha256(encode(ut)).digest()
    seal = rewards.keccak256(tid + nonce_bytes)
    block_hash = rewards.keccak256(tid + nonce_bytes + rewards.keccak256(encode(share_bytes)))
    return raw, '0x' + block_hash.hex(), seal, tid, timestamp


class Node:
    """Small retained-state node; every state-dependent read must be anchored."""
    def __init__(self, shares=None, signed=False, *, height=20, activation=None,
                 max_nonce=False, version=None, parent_version=None):
        self.height = height
        self.version = (2 if activation is not None and height >= activation else 1) if version is None else version
        parent_version = (2 if activation is not None and height-1 >= activation else 1) if parent_version is None else parent_version
        self.cp = {4: 0, 5: 2, 7: 6, 8: 32, 9: 2, 10: 256,
                   11: 4, 13: 1000, 14: 500, 15: 2 * 10**18, 18: 64, 24: 32}
        self.parent_raw, self.parent_hash, self.parent_seal, _, _ = header(
            height-1, '0x' + 'aa' * 32, 7, [], version=parent_version)
        seed = hashlib.sha256(b'PoColNonce' + self.parent_seal + word(height)[-8:] + word(0)[-4:]).digest()
        self.order = sorted(IDS, key=lambda a: hashlib.sha256(seed + bytes.fromhex(a[2:])).digest())
        domain = (1 << 64) if self.version == 2 else 200
        self.ranges = {a: (i * domain // 3, (i + 1) * domain // 3)
                       for i, a in enumerate(self.order)}
        self.winner = self.order[-1] if max_nonce else self.order[0]
        self.nonce = (1 << 64)-1 if max_nonce else self.ranges[self.winner][0] + 1
        self.proposer = self.order[2] if signed else None
        # Two committed shares owned by the second member, plus implicit winner.
        self.share_nonces = ([self.ranges[self.order[1]][0] + 1,
                              self.ranges[self.order[1]][0] + 2] if shares is None else shares)
        self.raw, self.hash, _, self.tid, self.timestamp = header(
            height, self.parent_hash, self.nonce, self.share_nonces, self.proposer, version=self.version)
        self.params = {'chainId': CHAIN_ID, 'genesisHash': GENESIS,
                       'protocolVersion': 2, 'activeVersion': self.version, 'nonceMode': 0,
                       'forkSchedule': [[1, 0]] + ([[2, activation]] if activation is not None else [])}
        self.records = {a: [1 | (1 << 16), int(a, 16), 100 * 10**18, (height-1) << 192] for a in IDS}
        self.parent_records = copy.deepcopy(self.records)
        self.gross = 2 * 10**18
        self.pending = [self.gross, int(self.proposer, 16) << 8 if signed else 1, 0]
        self.accounts = {a: {'nonce': 1, 'code': '0x', 'balance': 10**20} for a in IDS}
        self.accounts[ESCROW] = {'nonce': 1, 'code': '0x6000', 'balance': 10**25}
        self.block = {'number': hex(height), 'hash': self.hash, 'parentHash': self.parent_hash,
                      'miner': self.winner, 'nonce': '0x' + self.nonce.to_bytes(8, 'big').hex(),
                      'timestamp': hex(self.timestamp), 'transactions': [], 'baseFeePerGas': '0x7',
                      'gasUsed': '0x0', 'gasLimit': hex(8_000_000), 'stateRoot': '0x' + '44' * 32,
                      'pocol': {'attempt': 0, 'signed': signed, 'proposer': self.proposer or '0x',
                                'shares': len(self.share_nonces), 'final': False,
                                'templateId': '0x' + self.tid.hex()}}
        self.calls = []
        self.anchor_changed = False
        self.malformed = None

    def __call__(self, method, params):
        self.calls.append((method, copy.deepcopy(params)))
        if method == 'pocol_getParams':
            result = self.params
        elif method == 'eth_blockNumber':
            result = hex(self.height)
        elif method == 'eth_getBlockByNumber':
            if params[0] == hex(self.height-1):
                result = {'number': hex(self.height-1), 'hash': self.parent_hash}
            else:
                result = copy.deepcopy(self.block)
                if self.anchor_changed and params[0] != 'latest':
                    result['hash'] = '0x' + 'ff' * 32
        elif method == 'pocol_getBlockRlp':
            raw = self.parent_raw if params[0] == hex(self.height-1) else self.raw
            result = {'header': '0x' + raw.hex(), 'body': '0xc2c0c0'}
        elif method == 'eth_call':
            call, tag = params
            if tag not in (hex(self.height-1), hex(self.height)):
                raise AssertionError('Unanchored contract state read: ' + str(params))
            data = bytes.fromhex(call['data'][2:])
            sel, arg = data[:4], int.from_bytes(data[4:36], 'big')
            if self.malformed == sel:
                result = '0x01'
            elif call['to'].lower() == CAPS and sel == selector('get(uint256)'):
                result = abi(self.cp[arg])
            elif call['to'].lower() == ESCROW and sel == selector('pending(uint256)'):
                if arg != self.height:
                    raise AssertionError('Incorrect reward pot')
                result = abi(*self.pending)
            elif call['to'].lower() == REGISTRY and sel == selector('lengths()'):
                result = abi(len(IDS), len(IDS), 0, 0)
            elif call['to'].lower() == REGISTRY and sel == selector('activeAt(uint256)'):
                result = abi(IDS[arg])
            elif call['to'].lower() == REGISTRY and sel == selector('recordWords(address)'):
                address = '0x' + arg.to_bytes(20, 'big').hex()
                records = self.parent_records if tag == hex(self.height-1) else self.records
                result = abi(*records.get(address, [0, 0, 0, 0]))
            elif call['to'].lower() == REGISTRY and sel == selector('payInfos(address[])'):
                count = int.from_bytes(data[36:68], 'big')
                addresses = ['0x' + data[68 + 32*i:100 + 32*i][-20:].hex() for i in range(count)]
                recs = [self.records.get(a, [0, 0, 0, 0]) for a in addresses]
                result = abi(32, count, *[r[1] if r[0] and not ((r[0] >> 8) & 255) else 0 for r in recs])
            else:
                raise AssertionError('Unexpected contract read: ' + str(params))
        elif method in ('eth_getBalance', 'eth_getTransactionCount', 'eth_getCode'):
            address, tag = params
            if tag != hex(self.height):
                raise AssertionError('Unanchored payout state read')
            a = self.accounts.get(address.lower(), {'nonce': 0, 'code': '0x', 'balance': 0})
            result = (hex(a['balance']) if method == 'eth_getBalance' else
                      hex(a['nonce']) if method == 'eth_getTransactionCount' else a['code'])
        else:
            raise AssertionError('Unexpected RPC: ' + method)
        return {'jsonrpc': '2.0', 'id': 1, 'result': copy.deepcopy(result)}


class RewardTests(unittest.TestCase):
    def test_activation_boundary_uses_each_blocks_rules(self):
        schedule = ((1, 0), (2, 70))
        for height in (69, 70, 71):
            with self.subTest(height=height):
                node = Node(height=height, activation=70)
                result = rewards.snapshot(node, fork_schedule=schedule)
                payouts = {b['minerId']: int(b['amountWei']) for b in result['beneficiaries']}
                self.assertEqual(payouts, {node.winner: 326562500000000000,
                                          node.order[1]: 53125000000000000})
                self.assertEqual(result['blockHeight'], height)
                self.assertEqual(result['settlementHeight'], height + 4)
                self.assertEqual(node.cp[4], 0, 'The fork must not rewrite genesis CP')

    def test_maximum_u64_nonce_has_a_payable_owner_after_activation(self):
        node = Node(height=70, activation=70, max_nonce=True)
        result = rewards.snapshot(node, fork_schedule=((1, 0), (2, 70)))
        self.assertEqual(node.nonce, 18446744073709551615)
        payouts = {b['minerId']: int(b['amountWei']) for b in result['beneficiaries']}
        self.assertEqual(payouts, {node.order[-1]: 326562500000000000,
                                  node.order[1]: 53125000000000000})

    def test_maximum_nonce_is_rejected_before_activation(self):
        node = Node(height=69, activation=70, max_nonce=True)
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'outside assigned domain'):
            rewards.snapshot(node, fork_schedule=((1, 0), (2, 70)))

    def test_header_version_must_match_its_height(self):
        for height, version in ((69, 2), (70, 1), (71, 1)):
            with self.subTest(height=height, version=version):
                node = Node(height=height, activation=70, version=version)
                with self.assertRaisesRegex(rewards.RewardUnavailable, 'Header version'):
                    rewards.snapshot(node, fork_schedule=((1, 0), (2, 70)))

    def test_parent_version_is_checked_across_activation(self):
        node = Node(height=70, activation=70, parent_version=2)
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'Header version'):
            rewards.snapshot(node, fork_schedule=((1, 0), (2, 70)))

    def test_rpc_cannot_enable_or_reschedule_a_fork(self):
        node = Node(height=70, activation=70)
        with self.assertRaises(rewards.RewardUnavailable):
            rewards.snapshot(node)
        node.params['forkSchedule'] = [[1, 0], [2, 69]]
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'differs from trusted'):
            rewards.snapshot(node, fork_schedule=((1, 0), (2, 70)))
        del node.params['forkSchedule']
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'did not advertise'):
            rewards.snapshot(node, fork_schedule=((1, 0), (2, 70)))

    def test_legacy_rpc_without_schedule_remains_v1_only(self):
        node = Node()
        del node.params['forkSchedule']
        self.assertEqual(rewards.snapshot(node)['blockHeight'], 20)

    def test_moving_head_version_does_not_override_anchored_header(self):
        for height, advertised_version in ((69, 2), (70, 1)):
            with self.subTest(height=height):
                node = Node(height=height, activation=70)
                node.params['activeVersion'] = advertised_version
                self.assertEqual(rewards.snapshot(node, fork_schedule=((1, 0), (2, 70)))['blockHeight'], height)

    def test_unsupported_or_ambiguous_trusted_schedules_fail_before_rpc(self):
        invalid = ([], [(2, 0)], [(1, 0), (2, 0)], [(1, 0), (3, 70)],
                   [(1, 0), (2, 70), (1, 80)], [(1, 0), (2, 1 << 64)],
                   [(1, False)], [(True, 0)], [(1, 0), (2, '70')])
        for schedule in invalid:
            with self.subTest(schedule=schedule):
                node = Node()
                with self.assertRaises(rewards.RewardUnavailable):
                    rewards.snapshot(node, fork_schedule=schedule)
                self.assertEqual(node.calls, [])

    def test_liveness_uses_32_block_window_not_6_second_fallback_wait(self):
        node = Node(height=70)
        # This participant last became active 20 blocks ago: inside W_act=32,
        # outside the unrelated dFbWait=6. Its two proofs remain payable.
        node.parent_records[node.order[1]][3] = 50 << 192
        result = rewards.snapshot(node)
        payouts = {b['minerId']: int(b['amountWei']) for b in result['beneficiaries']}
        self.assertEqual(payouts[node.order[1]], 53125000000000000)
        queried_caps = [int(p[0]['data'][-64:], 16) for m, p in node.calls
                        if m == 'eth_call' and p[0]['to'] == CAPS]
        self.assertIn(24, queried_caps)
        self.assertNotIn(7, queried_caps)

    def test_liveness_falls_back_to_live_members_when_all_inactive(self):
        node = Node(height=70)
        for record in node.parent_records.values():
            record[3] = 0
        self.assertEqual(rewards.snapshot(node)['projectedPaidWei'], '379687500000000000')

    def test_invalid_activity_window_fails_closed(self):
        node = Node()
        node.cp[24] = 0
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'nonce configuration'):
            rewards.snapshot(node)

    def test_keccak_known_vectors(self):
        self.assertEqual(rewards.keccak256(b'').hex(),
                         'c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470')
        self.assertEqual(rewards.keccak256(b'abc').hex(),
                         '4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45')

    def test_nonzero_shares_follow_nonce_owner_and_exact_floor(self):
        node = Node()
        result = rewards.snapshot(node)
        payouts = {b['minerId']: int(b['amountWei']) for b in result['beneficiaries']}
        self.assertEqual(payouts, {node.winner: 326562500000000000,
                                  node.order[1]: 53125000000000000})
        self.assertEqual(result['beneficiaryCount'], 2)
        self.assertEqual(result['grossRewardWei'], '2000000000000000000')
        self.assertEqual(result['projectedPaidWei'], '379687500000000000')
        self.assertEqual(result['projectedBurnedWei'], '1620312500000000000')
        self.assertEqual(result['shareCount'], 2)
        self.assertEqual(result['blockHeight'], 20)
        self.assertEqual(result['blockHash'], node.hash)
        self.assertEqual(result['settlementHeight'], 24)
        self.assertEqual(result['status'], 'pending')

    def test_signed_distinct_proposer_is_separate_positive_beneficiary(self):
        node = Node(signed=True)
        result = rewards.snapshot(node)
        payouts = {b['minerId']: int(b['amountWei']) for b in result['beneficiaries']}
        self.assertEqual(payouts, {node.winner: 226562500000000000,
                                  node.order[1]: 53125000000000000,
                                  node.proposer: 100000000000000000})
        self.assertEqual(result['beneficiaryCount'], 3)

    def test_no_shares_implicit_winner_counted_once(self):
        node = Node(shares=[])
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 1)
        self.assertEqual(result['projectedPaidWei'], '326562500000000000')
        self.assertEqual(result['beneficiaries'][0]['minerId'], node.winner)

    def test_winner_with_two_extra_shares_is_one_beneficiary(self):
        node = Node()
        shares = [node.ranges[node.winner][0] + 2, node.ranges[node.winner][0] + 3]
        node.share_nonces = shares
        node.raw, node.hash, _, node.tid, _ = header(
            20, node.parent_hash, node.nonce, shares, node.proposer)
        node.block['hash'] = node.hash
        node.block['pocol']['templateId'] = '0x' + node.tid.hex()
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 1)
        self.assertEqual(result['projectedPaidWei'], '379687500000000000')
        self.assertEqual(result['beneficiaries'][0]['minerId'], node.winner)

    def test_zero_reward_address_is_burned(self):
        node = Node(shares=[])
        node.records[node.winner][1] = 0
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 0)
        self.assertEqual(result['projectedPaidWei'], '0')

    def test_tombed_share_owner_is_burned_not_counted(self):
        node = Node()
        node.records[node.order[1]][0] |= 1 << 8
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 1)
        self.assertEqual(result['projectedPaidWei'], '326562500000000000')
        self.assertEqual(result['projectedBurnedWei'], '1673437500000000000')

    def test_absent_winner_record_burns_winner_and_fallback_proposer_parts(self):
        node = Node()
        del node.records[node.winner]
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 1)
        self.assertEqual(result['projectedPaidWei'], '53125000000000000')

    def test_balance_only_account_is_not_durable_pay_recipient(self):
        node = Node(shares=[])
        node.accounts[node.winner]['nonce'] = 0
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 0)
        self.assertEqual(result['projectedPaidWei'], '0')
        self.assertEqual(result['projectedBurnedWei'], node.pending[0].__str__())

    def test_contract_recipient_is_durable_even_with_zero_nonce(self):
        node = Node(shares=[])
        node.accounts[node.winner]['nonce'] = 0
        node.accounts[node.winner]['code'] = '0x00'
        self.assertEqual(rewards.snapshot(node)['beneficiaryCount'], 1)

    def test_zero_after_integer_rounding_is_not_a_beneficiary(self):
        node = Node()
        node.cp[15] = 1
        node.pending[0] = 1
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 0)
        self.assertEqual(result['projectedPaidWei'], '0')
        self.assertEqual(result['projectedBurnedWei'], '1')

    def test_distinct_miners_sharing_reward_wallet_are_counted_by_miner(self):
        node = Node()
        node.records[node.order[1]][1] = int(node.winner, 16)
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 2)
        self.assertEqual({b['rewardAddress'] for b in result['beneficiaries']}, {node.winner})

    def test_destination_overflow_burns_the_payment(self):
        node = Node(shares=[])
        node.accounts[node.winner]['balance'] = U256
        result = rewards.snapshot(node)
        self.assertEqual(result['beneficiaryCount'], 0)
        self.assertEqual(result['projectedPaidWei'], '0')

    def test_wrong_network_fails_closed(self):
        node = Node()
        node.params['chainId'] = 1
        with self.assertRaises((ValueError, RuntimeError)):
            rewards.snapshot(node)

    def test_anchor_change_fails_closed(self):
        node = Node()
        node.anchor_changed = True
        with self.assertRaises((ValueError, RuntimeError)):
            rewards.snapshot(node)

    def test_malformed_fixed_abi_fails_closed(self):
        node = Node()
        node.malformed = selector('pending(uint256)')
        with self.assertRaises((ValueError, RuntimeError)):
            rewards.snapshot(node)

    def test_missing_pending_pot_fails_closed(self):
        node = Node()
        node.pending = [0, 0, 0]
        with self.assertRaises((ValueError, RuntimeError)):
            rewards.snapshot(node)

    def test_final_block_fails_closed(self):
        node = Node()
        node.block['pocol']['final'] = True
        with self.assertRaises((ValueError, RuntimeError)):
            rewards.snapshot(node)

    def test_header_hash_mismatch_fails_closed(self):
        node = Node()
        node.block['hash'] = '0x' + 'ee' * 32
        with self.assertRaises((ValueError, RuntimeError)):
            rewards.snapshot(node)

    def test_header_share_count_mismatch_fails_closed(self):
        node = Node()
        node.block['pocol']['shares'] += 1
        with self.assertRaises((ValueError, RuntimeError)):
            rewards.snapshot(node)


if __name__ == '__main__':
    unittest.main()
