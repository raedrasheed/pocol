"""Reward snapshot tests against an independent deterministic RPC fixture.

The fixture uses the published v1 header layout and nonce-ticket formulas, not
the module's header decoder or owner reconstruction. No network is contacted.
Run: python3 -m unittest -v test_pocol_rewards.py
"""
import copy
import hashlib
import json
import os
import time
import unittest
import urllib.request
from collections import Counter
from pathlib import Path

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
                   11: 4, 13: 1000, 14: 500, 15: 2 * 10**18, 16: 100 * 10**18, 18: 64, 24: 32}
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

    def test_full_nonce_range_holds_from_version_two_onwards(self):
        for version, mode, full in ((3, 0, True), (7, 0, True), (2, 0, True), (1, 1, True), (1, 0, False)):
            with self.subTest(version=version, mode=mode):
                self.assertIs(rewards.full_nonce_rule(version, mode), full)

    def test_generic_schedule_rule_accepts_later_versions_only_under_a_higher_bound(self):
        three = ((1, 0), (2, 10), (3, 20))
        self.assertEqual(rewards.validated_fork_schedule(three, 3), three)
        self.assertEqual(rewards.active_version(25, three), 3)
        self.assertEqual(rewards.active_version(19, three), 2)
        self.assertEqual(rewards.MAX_FORK_VERSION, 2)
        for schedule, bound in ((three, 2), ([(1, 0), (3, 20)], 3), ([(1, 0), (2, 10), (3, 10)], 3),
                                ([(1, 0), (2, 10), (3, 9)], 3), ([(1, 0)] * 17, 3)):
            with self.subTest(schedule=schedule, bound=bound):
                with self.assertRaises(rewards.RewardUnavailable):
                    rewards.validated_fork_schedule(schedule, bound)

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

    def test_accruing_record_fills_its_bond_before_its_reward_address(self):
        node = Node()
        node.cp[16] = 20 * 10**18
        winner = node.records[node.winner]
        winner[0] = (winner[0] & ~(255 << 16)) | (4 << 16)
        winner[2] = 20 * 10**18 - 100_000_000_000_000_000   # 0.1 PCOL short of the bond
        result = rewards.snapshot(node)
        by = {b['minerId']: b for b in result['beneficiaries']}
        self.assertEqual(by[node.winner]['bondAccruedWei'], '100000000000000000')
        self.assertEqual(int(by[node.winner]['amountWei']), 326562500000000000 - 100_000_000_000_000_000)
        self.assertNotIn('bondAccruedWei', by[node.order[1]])
        self.assertEqual(result['projectedPaidWei'], '379687500000000000', 'accrual is paid value, not a burn')
        self.assertEqual(result['projectedBondWei'], '100000000000000000')

    def test_accruing_record_far_from_its_bond_receives_nothing_at_its_address(self):
        node = Node()
        node.cp[16] = 20 * 10**18
        winner = node.records[node.winner]
        winner[0] = (winner[0] & ~(255 << 16)) | (4 << 16)
        winner[2] = 0
        result = rewards.snapshot(node)
        by = {b['minerId']: b for b in result['beneficiaries']}
        self.assertEqual((by[node.winner]['amountWei'], by[node.winner]['bondAccruedWei']), ('0', '326562500000000000'))

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



# ---------------------------------------------------------------------------------------------------------------------
# v3 (spec-2) networks, V3-PR-G. The model vectors are the independent model's (prototypes/lp4-execution/tests/v3_model.py,
# tests/v3/*.json); the differential runs against a v3 devnet chain from the preview build (pocold tests/v3_reward_gateway.rs).
REPO = Path(__file__).resolve().parents[2]
V3_VECTORS = REPO / 'prototypes' / 'lp4-execution' / 'tests' / 'v3'
V3_NETWORK = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32, 'specVersion': 2, 'v3': True}
# The CP schema v2 defaults of the model's chain scenarios (v3_model.py DEFAULT_CP and CHAIN_DEFAULT) by name.
MODEL_DEFAULT = {'D': 4, 'm': 32, 'kappa': 2, 'alpha_bp': 1000, 'gamma_bp': 500, 'S0': 2 * 10**18, 'H_half': 4_207_680,
                 'S_tail': 2 * 10**18 // 32, 'gamma_tip_bp': 5000}


def v3_spec(chain_id=777963, version=2, items=67, sys_hash=None):
    """A canonical GenesisSpec [version, chainId, CP, allocRoot, M_0List, sysCodeHash] for the refusal tests."""
    cp = [uint(1) for _ in range(items)]
    sys_hash = bytes.fromhex((sys_hash or rewards.V3_SYS_CODE_HASH)[2:])
    return encode([bytes([version]), chain_id, cp, b'\x33' * 32, [], sys_hash])


class V3ConfigurationTests(unittest.TestCase):
    def test_v3_sys_code_hash_is_the_bundled_v3_program_set(self):
        programs = json.loads((REPO / 'prototypes' / 'lp4-execution' / 'tests' / 'data' / 'system-v3.json').read_text())['programs']
        codes = [bytes.fromhex(p['runtime'].removeprefix('0x')) for p in programs]
        self.assertEqual(len(codes), 5)
        self.assertEqual('0x' + rewards.keccak256(encode(codes + [b''])).hex(), rewards.V3_SYS_CODE_HASH)

    def test_a_v3_network_is_only_an_explicit_spec_2_configuration(self):
        net = rewards.validated_network(V3_NETWORK)
        self.assertEqual((net.chain_id, net.genesis, net.spec_version, net.source), (777963, '0x' + '5a' * 32, 2, None))
        self.assertEqual(rewards.validated_network(net), net)
        invalid = [dict(V3_NETWORK, specVersion=1), dict(V3_NETWORK, v3=False), {k: v for k, v in V3_NETWORK.items() if k != 'v3'},
                   dict(V3_NETWORK, specVersion=True), dict(V3_NETWORK, chainId='777963'), dict(V3_NETWORK, chainId=0),
                   dict(V3_NETWORK, genesisHash=GENESIS), dict(V3_NETWORK, genesisHash='0x' + '5A' * 32),
                   dict(V3_NETWORK, extra=1), dict(V3_NETWORK, source='http://example.org'), [], None]
        for value in invalid:
            with self.subTest(value=value):
                with self.assertRaises(rewards.RewardUnavailable):
                    rewards.validated_network(value)

    def test_the_default_snapshot_refuses_a_v3_node(self):
        node = Node()
        node.params['chainId'], node.params['genesisHash'] = 777963, '0x' + '5a' * 32
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'Wrong PoCol network'):
            rewards.snapshot(node)

    def test_a_configured_v3_network_refuses_a_v1_node_before_any_state_read(self):
        node = Node()
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'Wrong PoCol network'):
            rewards.snapshot(node, network=V3_NETWORK)
        self.assertEqual([m for m, _ in node.calls], ['pocol_getParams'])

    def test_the_v3_genesis_spec_is_proven_by_its_trusted_hash(self):
        def params(spec, genesis=None):
            return {'chainId': 777963, 'genesisHash': genesis or '0x' + rewards.keccak256(spec).hex(), 'activeVersion': 2,
                    'forkSchedule': [[1, 0], [2, 2]], 'genesisPre': '0x' + spec.hex()}
        def run(p):
            calls = []
            def query(method, args):
                calls.append(method)
                if method != 'pocol_getParams':
                    raise AssertionError('read past the network check: ' + method)
                return {'jsonrpc': '2.0', 'id': 1, 'result': p}
            net = dict(V3_NETWORK, genesisHash=p['genesisHash'])
            with self.assertRaises(rewards.RewardUnavailable) as caught:
                rewards.snapshot_v3(query, fork_schedule=((1, 0), (2, 2)), network=net)
            return str(caught.exception)
        good = v3_spec()
        self.assertRegex(run(params(good)), 'Invalid (nonce|reward) configuration', 'a structurally valid v3 spec reaches the CP checks')
        self.assertIn('does not hash', run(params(good, '0x' + '5a' * 32)))
        self.assertIn('Unsupported GenesisSpec version', run(params(v3_spec(version=1, items=52))))
        self.assertIn('CP item count', run(params(v3_spec(items=52))))
        self.assertIn('chain ID differs', run(params(v3_spec(chain_id=777964))))
        self.assertIn('v3 program set', run(params(v3_spec(sys_hash='0x' + '11' * 32))))
        untrusted = params(good)
        untrusted['forkSchedule'] = [[1, 0], [2, 3]]
        self.assertIn('differs from trusted', run(untrusted))
        del untrusted['forkSchedule']
        self.assertIn('did not advertise', run(untrusted))

    def test_v3_active_follows_the_spec_version_or_a_third_fork(self):
        self.assertTrue(rewards.v3_active(0, ((1, 0),), 2))
        self.assertFalse(rewards.v3_active(10**9, ((1, 0), (2, 5)), 1))
        self.assertTrue(rewards.v3_active(20, ((1, 0), (2, 10), (3, 20)), 1))
        self.assertFalse(rewards.v3_active(19, ((1, 0), (2, 10), (3, 20)), 1))

    def test_ordered_root_matches_the_empty_and_single_item_tries(self):
        self.assertEqual(rewards.ordered_root([]).hex(), '56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421')
        # One item: a single leaf [hex-prefix(key 0x80 nibbles 8,0 as leaf) = 0x2080, value].
        leaf = encode([b'\x20\x80', b'\x07' * 40])
        self.assertEqual(rewards.ordered_root([b'\x07' * 40]), rewards.keccak256(leaf))


class V3ModelVectorTests(unittest.TestCase):
    """The reward arithmetic of snapshot_v3 against the independent model's vectors (tests/v3/*.json)."""

    def test_issuance_schedule_vectors(self):
        cases = json.loads((V3_VECTORS / 'issuance-carry-vectors.json').read_text())['schedule']['cases']
        self.assertGreater(len(cases), 10)
        for c in cases:
            with self.subTest(case=c):
                self.assertEqual(rewards.issuance(int(c['h']), int(c['S0']), int(c['H_half']), int(c['S_tail'])), int(c['issued']))

    def test_every_pot_is_issuance_plus_tips_less_the_share_plus_the_lagged_base_burn(self):
        """Every closed pot of every chain scenario: issued = M3, amount = issued + tips - share + carry, and the carry
        into pot h is the base burn R - A of settle(h-1-D) (Q = max(C, kappa*m), C the header and late shares of
        h-1-D). A pot that evidence names is skipped for the carry (its settlement may burn whole)."""
        checked, carried, tipped = 0, 0, 0
        for name in ('issuance-carry-vectors.json', 'evidence-penalty-vectors.json', 'late-share-vectors.json', 'contest-vectors.json'):
            for label, vector in json.loads((V3_VECTORS / name).read_text()).items():
                if not isinstance(vector, dict) or 'steps' not in vector or not any(s['op'][0] == 'begin' for s in vector['steps']):
                    continue
                cp = dict(MODEL_DEFAULT, **{k: int(v) for k, v in vector['cp'].items()})
                d, kappa_m = cp['D'], cp['kappa'] * cp['m']
                pots, proposers, counts, named, winners = {}, {}, Counter(), set(), {}
                for step in vector['steps']:
                    op = step['op']
                    if op[0] == 'final':
                        break
                    if op[0] == 'begin' and step['ok']:
                        h = int(op[1])
                        proposers[h] = op[2]
                        if h >= 2:
                            counts[h - 1] += len(step['shares']) + 1
                            winners[h - 1] = step['winner']
                        for e in step['evidence']:
                            if e[0] == 'late':
                                counts[e[1]] += len(e[2])
                            elif e[1] is not None:
                                named.add(e[1])
                    if op[0] == 'close':
                        h = int(op[1])
                        issued, tips, amount = int(step['issued']), int(step['tips']), int(step['amount'])
                        with self.subTest(vector=label, h=h):
                            self.assertEqual(rewards.issuance(h, cp['S0'], cp['H_half'], cp['S_tail']), issued)
                            share = rewards.tip_share(tips, cp['gamma_tip_bp'], proposers[h] != '')
                            carry = amount - rewards.pot_amount(issued, tips, share, 0)
                            self.assertGreaterEqual(carry, 0)
                            j0 = h - 1 - d
                            if j0 >= 1 and j0 not in named:
                                r = rewards.split(pots[j0], cp['alpha_bp'], cp['gamma_bp'])[2]
                                self.assertEqual(carry, rewards.base_burn(r, counts[j0], kappa_m))
                                carried += carry > 0
                            elif j0 < 1:
                                self.assertEqual(carry, 0)
                            checked += 1
                            tipped += share > 0
                        pots[h] = amount
        self.assertGreater(checked, 500)
        self.assertGreater(carried, 100)
        self.assertGreater(tipped, 5)

    def test_reporter_cut_and_split_follow_the_programs(self):
        self.assertEqual(rewards.reporter_cut(20 * 10**18, 2500, True), 5 * 10**18)
        self.assertEqual(rewards.reporter_cut(20 * 10**18, 2500, False), 0)
        self.assertEqual(rewards.reporter_cut(7, 2500, True), 1)
        self.assertEqual(rewards.split(2 * 10**18, 1000, 500), (2 * 10**17, 10**17, 17 * 10**17))
        r = 17 * 10**17
        self.assertEqual(rewards.base_burn(r, 3, 64), r - r * 3 // 64)
        self.assertEqual(rewards.base_burn(r, 64, 64), 0)
        self.assertEqual(rewards.base_burn(r, 70, 64), 0)


def _devnet():
    raw = os.environ.get('POCOL_V3_DEVNET')
    return json.loads(raw) if raw else None


@unittest.skipUnless(_devnet(), 'needs POCOL_V3_DEVNET: run pocold tests/v3_reward_gateway.rs (--features v3-preview)')
class V3DevnetDifferential(unittest.TestCase):
    """snapshot_v3 of every block of a v3 devnet chain from the preview build (pocold tests/v3_reward_gateway.rs), over
    the node's JSON-RPC, against the node's own state: what each reward address and bond gained in the block, the
    Escrow burned total, the pending pot and carry, and the ShareLedger's owners."""

    @classmethod
    def setUpClass(cls):
        cls.fx = _devnet()
        cls.schedule = tuple(map(tuple, cls.fx['forkSchedule']))
        cls.network = {'chainId': cls.fx['chainId'], 'genesisHash': cls.fx['genesisHash'], 'specVersion': 2, 'v3': True}
        cls.names = {v: k for k, v in cls.fx['members'].items()}
        cls.snaps = {h: rewards.snapshot_v3(cls.rpc, fork_schedule=cls.schedule, network=cls.network, height=h)
                     for h in range(1, cls.fx['head'] + 1)}

    @classmethod
    def rpc(cls, method, params):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}).encode()
        request = urllib.request.Request(cls.fx['rpc'], data=body, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=30) as reply:
            return json.loads(reply.read())

    def read(self, method, params):
        answer = self.rpc(method, params)
        self.assertNotIn('error', answer, (method, params))
        return answer['result']

    def call(self, to, signature, args, h):
        data = rewards.keccak256(signature.encode())[:4] + b''.join(word(a) for a in args)
        raw = bytes.fromhex(self.read('eth_call', [{'to': to, 'data': '0x' + data.hex()}, hex(h)])[2:])
        return [int.from_bytes(raw[i:i+32], 'big') for i in range(0, len(raw), 32)]

    def records(self, h):
        """Every registry record at h: id -> recordWords."""
        n = self.call(REGISTRY, 'lengths()', [], h)[0]
        ids = ['0x' + self.call(REGISTRY, 'recordAt(uint256)', [i], h)[0].to_bytes(20, 'big').hex() for i in range(n)]
        return {i: self.call(REGISTRY, 'recordWords(address)', [int(i, 16)], h) for i in ids}

    def senders(self, h):
        block = self.read('eth_getBlockByNumber', [hex(h), False])
        return {self.read('eth_getTransactionByHash', [t])['from'].lower() for t in block['transactions']}

    def balance(self, a, h):
        return int(self.read('eth_getBalance', [a, hex(h)]), 16)

    def ledger(self, j, h):
        raw = bytes.fromhex(self.read('eth_call', [{'to': rewards.LEDGER, 'data': '0x' + (selector('entriesOf(uint64)') + word(j)).hex()}, hex(h)])[2:])
        w = [int.from_bytes(raw[i:i+32], 'big') for i in range(0, len(raw), 32)]
        n = w[3]
        return Counter({'0x' + w[4 + i].to_bytes(20, 'big').hex(): w[5 + n + i] for i in range(n)})

    def test_every_block_moves_exactly_the_reproduced_payments(self):
        for h, snap in self.snaps.items():
            with self.subTest(h=h):
                block = snap['block']
                before, after, senders = self.records(h - 1), self.records(h), self.senders(h)
                by_reward, by_bond = Counter(), Counter()
                for b in block['beneficiaries']:
                    by_reward[b['rewardAddress']] += int(b['amountWei'])
                    by_bond[b['minerId']] += int(b.get('bondAccruedWei', 0))
                rewards_seen = {'0x' + r[1].to_bytes(20, 'big').hex() for r in list(before.values()) + list(after.values())}
                self.assertLessEqual(set(by_reward), rewards_seen)
                for reward in sorted(rewards_seen - senders):
                    self.assertEqual(self.balance(reward, h) - self.balance(reward, h - 1), by_reward[reward], reward)
                for ident in sorted(set(before) & set(after) - senders):
                    self.assertEqual(after[ident][2] - before[ident][2], by_bond[ident], self.names.get(ident, ident))
                burned = self.call(ESCROW, 'burnedTotal()', [], h)[0] - self.call(ESCROW, 'burnedTotal()', [], h - 1)[0]
                self.assertEqual(burned, int(block['burnedWei']))
                self.assertEqual(self.call(ESCROW, 'pending(uint256)', [h], h)[0], int(snap['grossRewardWei']))
                carry = self.call(ESCROW, 'carry()', [], h)[0]
                self.assertEqual(carry, int(block['settlement']['carryOutWei']) if block['settlement'] else 0)
                self.assertEqual(int(snap['grossRewardWei']), int(snap['issuedWei']) + int(snap['tipsWei'])
                                 - int(snap['tipShareWei']) + int(snap['carryInWei']))

    def test_the_owners_of_every_pot_are_the_ledger_owners(self):
        d = 4
        for h, snap in self.snaps.items():
            with self.subTest(h=h):
                if h + 1 in self.snaps:
                    late = Counter(o for e in self.snaps[h + 1]['block']['evidence'] if e['kind'] == 3 and e['potHeight'] == h
                                   for o in e['owners'])
                    header = Counter({o['minerId']: o['shares'] for o in snap['owners']})
                    self.assertEqual(self.ledger(h, h + 1), header + late)
                settled = snap['block']['settlement']
                if settled:
                    self.assertEqual(settled['potHeight'], h - d)
                    self.assertEqual(self.ledger(h - d, h), Counter({o['minerId']: o['shares'] for o in settled['owners']}))

    def test_an_undisturbed_projection_is_its_settlement(self):
        """A pot nothing changes between its block and h + D (no late share, no evidence, no bond filled) settles exactly
        as the block's own projection says."""
        compared = 0
        for j, snap in self.snaps.items():
            if j + 4 not in self.snaps:
                continue
            settled = self.snaps[j + 4]['block']['settlement']
            quiet = (settled['lateShareCount'] == 0 and not settled['invalid'] and
                     not any(self.snaps[b]['block']['evidence'] for b in range(j + 1, j + 5)) and
                     not any('bondAccruedWei' in b for b in snap['beneficiaries'] + settled['beneficiaries']))
            if not quiet:
                continue
            with self.subTest(j=j):
                self.assertEqual(settled['beneficiaries'], snap['beneficiaries'])
                self.assertEqual(settled['carryOutWei'], snap['projectedCarryWei'])
                self.assertEqual(settled['grossRewardWei'], snap['grossRewardWei'])
                compared += 1
        self.assertGreater(compared, 5)

    def test_the_chain_exercises_every_v3_reward_rule(self):
        fx, m = self.fx, self.fx['members']
        rho = 2500
        # Tips and the tip share (V3-20), issuance M3 (V3-19), the lagged carry (V3-15).
        self.assertTrue(any(int(s['tipShareWei']) > 0 and int(s['tipShareWei']) * 2 == int(s['tipsWei']) for s in self.snaps.values()))
        self.assertTrue(any(int(s['carryInWei']) > 0 for s in self.snaps.values()))
        self.assertTrue(all(s['issuedWei'] == '2000000000000000000' for s in self.snaps.values()))
        # All-live membership (D-V3-01): the idle three owns a range in a block past W_act (its nonces are assigned).
        chain = rewards._V3Chain(self.rpc, rewards.validated_network(self.network), self.schedule,
                                 rewards._v3_cp(self.read('pocol_getParams', []), rewards.validated_network(self.network)))
        encoded = self.read('pocol_getBlockRlp', [hex(10)])
        chain.set_anchor(rewards.decode_header(bytes.fromhex(encoded['header'][2:]), self.schedule, rewards.validated_network(self.network)), encoded['body'])
        self.assertIn(m['three'], [owner for _, _, owner in chain.assignment(10)[1]])
        self.assertEqual(len(chain.assignment(10)[1]), 4)
        # Late shares (V3-02): block 5 credits two's omitted shares of template 4, paid at settle(4) in block 8.
        late = [e for e in self.snaps[fx['lateBlock']]['block']['evidence'] if e['kind'] == 3]
        self.assertEqual([(e['potHeight'], set(e['owners'])) for e in late], [(4, {m['two']})])
        settled = self.snaps[8]['block']['settlement']
        self.assertEqual((settled['potHeight'], settled['lateShareCount']), (4, 2))
        self.assertIn(m['two'], {b['minerId'] for b in settled['beneficiaries']})
        # Contests (V3-10/V3-14): three exited at the seat contest; alice exited, still ACCRUING, POOL_OUT (w0 bit 87).
        three = self.records(fx['seatContest'])[m['three']]
        self.assertEqual((three[0] & 255, (three[0] >> 8) & 255), (2, 0))
        alice = self.records(fx['poolContest'])[m['alice']]
        self.assertEqual((alice[0] & 255, (alice[0] >> 16) & 255, (alice[0] >> 87) & 1), (2, 4, 1))
        self.assertNotIn(m['alice'], {o['minerId'] for o in self.snaps[fx['poolContest'] + 1]['owners']})
        # The exited pool victim is payable: its share of a pending pot fills its accrued bond.
        paid = {b['minerId']: b for b in self.snaps[fx['poolVictimPaid']]['block']['settlement']['beneficiaries']}
        self.assertGreater(int(paid[m['alice']]['bondAccruedWei']), 0)
        # And punishable: type-2 evidence against it (pot 18, recorded ACCRUING) burns its bond less rho to the reporter
        # and pot 18 whole (V3-16/V3-17).
        burn = fx['wholePotBurn']
        ev = [e for s in self.snaps.values() for e in s['block']['evidence'] if e['kind'] == 2 and e['potHeight'] == burn]
        self.assertEqual(len(ev), 1)
        self.assertEqual((ev[0]['accused'], ev[0]['potInvalidated']), (m['alice'], True))
        self.assertEqual(int(ev[0]['reporterCutWei']), int(ev[0]['bondBurnedWei']) * rho // 10000)
        self.assertGreater(int(ev[0]['reporterCutWei']), 0)
        whole = self.snaps[burn + 4]['block']['settlement']
        self.assertEqual((whole['potHeight'], whole['invalid'], whole['burnedWei'], whole['beneficiaries']), (burn, True, whole['grossRewardWei'], []))
        # Offender-only (V3-16): two, a HELD winner, is accused before settle(22); pot 22 settles normally, two's winner
        # part and shares burn, one is paid, the reporter receives rho of two's 20 PCOL bond.
        pot = fx['offenderPot']
        ev = [e for s in self.snaps.values() for e in s['block']['evidence'] if e['kind'] == 2 and e['potHeight'] == pot]
        self.assertEqual(len(ev), 1)
        self.assertEqual((ev[0]['accused'], ev[0]['potInvalidated'], ev[0]['bondBurnedWei'], ev[0]['reporterCutWei']),
                         (m['two'], False, str(20 * 10**18), str(5 * 10**18)))
        offender = self.snaps[pot + 4]['block']['settlement']
        self.assertFalse(offender['invalid'])
        payees = {b['minerId'] for b in offender['beneficiaries']}
        self.assertNotIn(m['two'], payees)
        self.assertIn(m['one'], payees)
        self.assertGreater(int(offender['burnedWei']), 0)


if __name__ == '__main__':
    unittest.main()
