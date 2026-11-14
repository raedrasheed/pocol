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


def v3_spec(chain_id=777963, version=2, items=59, sys_hash=None):
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
        for name in ('issuance-carry-vectors.json', 'evidence-penalty-vectors.json', 'late-share-vectors.json', 'registry-budget-vectors.json'):
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

    def test_the_final_block_settles_every_remaining_pot_and_burns_the_carry(self):
        """Every chain scenario with a final block, replayed through snapshot_v3's own settlement code (_settle, _Payments)
        to its end: settle(h-D) in each block's phase 0 with its base burn carried into pot h+1, the tip share, and at F
        finalize (the tips and both outstanding carries burn) then settleFinal for every pot in [F-D+1, F-1] with its
        base burn burned. Each owner's total payment and the burned total equal the model's final state."""
        checked = 0
        for name in ('issuance-carry-vectors.json', 'evidence-penalty-vectors.json', 'late-share-vectors.json', 'registry-budget-vectors.json'):
            for label, vector in json.loads((V3_VECTORS / name).read_text()).items():
                if not isinstance(vector, dict) or not any(s['op'][0] == 'final' for s in vector.get('steps', [])):
                    continue
                chain = VectorChain(vector)
                d = chain.cp[11]
                paid, burned, carried, settled = Counter(), 0, {}, []
                for step in vector['steps']:
                    op = step['op']
                    if op[0] == 'begin' and step['ok']:
                        h, proposer = int(op[1]), chain.id(op[2]) if op[2] else None
                        self.assertEqual(step['evidence'], [], label)
                        if h >= 2:
                            chain.owners[h - 1] = [chain.id(x) for x in step['shares']] + [chain.id(step['winner'])]
                        flows = rewards._Payments(chain, h - 1)
                        carry = rewards._settle(chain, flows, h - d, h, set(), [], False)[1] if h > d else 0
                    elif op[0] in ('close', 'final'):
                        self.assertEqual(int(op[1]), h)
                        tips = int(step['tips'])
                        if op[0] == 'close':
                            chain.proposers[h] = proposer
                            share = rewards.tip_share(tips, chain.gamma_tip, proposer is not None)
                            if share:
                                flows.pay_to(proposer, share)
                            chain.pots[h] = rewards.pot_amount(int(step['issued']), tips, share, carried.get(h - 1, 0))
                            self.assertEqual(chain.pots[h], int(step['amount']), (label, h))
                            carried[h] = carry
                        else:
                            self.assertEqual((step['issued'], step['amount']), ('0', '0'))
                            flows.burned += tips + carried.get(h - 1, 0) + carry
                            for j in range(max(1, h - d + 1), h):
                                settled.append(rewards._settle(chain, flows, j, h, set(), [], True)[0])
                        paid += flows.paid
                        burned += flows.burned
                    elif op[0] not in ('begin', 'register'):
                        self.fail((label, op))
                with self.subTest(vector=label):
                    self.assertEqual([s['potHeight'] for s in settled], list(range(h - d + 1, h)))
                    self.assertTrue(all(int(s['baseBurnWei']) > 0 and s['carryOutWei'] == '0' for s in settled))
                    names = {v: k for k, v in chain.ids.items()}
                    self.assertEqual({names[m]: str(v) for m, v in paid.items() if v}, vector['final']['paid'])
                    self.assertEqual(str(burned), vector['final']['burned'])
                    self.assertEqual((vector['final']['carry'], vector['final']['pots']), ({}, {}))
                    checked += 1
        self.assertGreaterEqual(checked, 1)

    def test_reporter_cut_and_split_follow_the_programs(self):
        self.assertEqual(rewards.reporter_cut(20 * 10**18, 2500, True), 5 * 10**18)
        self.assertEqual(rewards.reporter_cut(20 * 10**18, 2500, False), 0)
        self.assertEqual(rewards.reporter_cut(7, 2500, True), 1)
        self.assertEqual(rewards.split(2 * 10**18, 1000, 500), (2 * 10**17, 10**17, 17 * 10**17))
        r = 17 * 10**17
        self.assertEqual(rewards.base_burn(r, 3, 64), r - r * 3 // 64)
        self.assertEqual(rewards.base_burn(r, 64, 64), 0)
        self.assertEqual(rewards.base_burn(r, 70, 64), 0)


PCOL = 10**18
B_REG = 20 * PCOL
HELD, PAID, ACCRUING = 1 << 16, 3 << 16, 4 << 16


class FakeV3Chain:
    """The reads _Payments, _punish, _mark_close, _tip_share_record and _settle make of a _V3Chain, served from plain
    dictionaries: records[(id, height)] = recordWords, accounts[(address, height)] = (nonce, code, balance), contract
    words by (to, signature, args, height). Anything else is an unexpected read."""

    def __init__(self, cp=None):
        self.cp = {8: 32, 9: 2, 11: 4, 13: 1000, 14: 500, 16: B_REG, 45: 0, 46: 0, 47: 0, 48: 0, 50: 128, rewards.CP_RHO_BP: 2500}
        self.cp.update(cp or {})
        self.records, self.accounts, self.words = {}, {}, {}

    def result(self, method, params):
        address, tag = params
        nonce, code, balance = self.accounts.get((address, int(tag, 16)), (0, '0x', 0))
        return {'eth_getBalance': hex(balance), 'eth_getTransactionCount': hex(nonce), 'eth_getCode': code}[method]

    def record(self, miner, height):
        return tuple(self.records.get((miner, height), (0, 0, 0, 0)))

    def call(self, to, signature, args, height, words=None):
        return list(self.words[(to, signature, tuple(args), height)])


def ident(n):
    return '0x' + n.to_bytes(20, 'big').hex()


class PagedActList(rewards._V3Chain):
    """A _V3Chain whose MinerRegistry has `n` active members (ids 1..n, HELD), served page by page through activeSlice,
    and whose blocks carry `moves[h]` = (REG_OPS users, reactivations, evidence kinds) in their per-block counters."""

    def __init__(self, n, moves=None):
        super().__init__(None, rewards.validated_network(V3_NETWORK), ((1, 0), (2, 0)), {18: rewards.MAX_MEMBERS_V3})
        self.members = [i + 1 for i in range(n)]
        self.moves, self.calls = moves or {}, Counter()

    def body(self, k):
        return [], [encode([bytes([kind]), b'']) for kind in self.moves.get(k, (0, 0, ()))[2]]

    def call(self, to, signature, args, height, words=None):
        self.calls[signature] += 1
        ops, reacts, _ = self.moves.get(height, (0, 0, ()))
        if signature == 'lengths()':
            return [len(self.members), len(self.members), 0, 0]
        if signature == 'counters()':
            return [ops]
        if signature == 'budgets()':
            return [(reacts << 8) | (height << 16)]
        if signature == 'activeSlice(uint256,uint256)':
            start, count = args
            page = self.members[start:start + count]
            n = len(page)
            words_ = [64, 64 + 32 * (n + 1), n, *page, n, *[1 | HELD] * n]
            return b''.join(w.to_bytes(32, 'big') for w in words_)
        raise AssertionError('Unexpected contract read: ' + signature)


class V3PagedActListTests(unittest.TestCase):
    """FN-2b C5: the gateway reads the ActList of M_max 10,000 members in activeSlice pages and reuses it for the next
    height when the block cannot have moved a member (the delta cache)."""

    def test_ten_thousand_members_are_read_in_pages(self):
        chain = PagedActList(10000)
        members = chain.active_list(5)
        self.assertEqual([int(m, 16) for m, _ in members], list(range(1, 10001)))
        self.assertTrue(all(w == 1 | HELD for _, w in members))
        self.assertEqual(chain.calls['activeSlice(uint256,uint256)'], -(-10000 // rewards.ACTIVE_PAGE))
        self.assertEqual(rewards.MAX_MEMBERS_V3, 10000)

    def test_a_quiet_block_reuses_the_list_and_a_moving_block_reads_it_again(self):
        chain = PagedActList(1500, moves={7: (1, 0, ()), 8: (0, 1, ()), 9: (0, 0, (2,)), 10: (0, 0, (3,))})
        chain.active_list(5)
        pages = -(-1500 // rewards.ACTIVE_PAGE)
        self.assertEqual(chain.calls['activeSlice(uint256,uint256)'], pages)
        chain.active_list(6)
        self.assertEqual(chain.calls['activeSlice(uint256,uint256)'], pages, 'a quiet block: the delta cache')
        for h, reread in ((7, True), (8, True), (9, True), (10, False)):
            before = chain.calls['activeSlice(uint256,uint256)']
            chain.active_list(h)
            with self.subTest(h=h):
                self.assertEqual(chain.calls['activeSlice(uint256,uint256)'] - before, pages if reread else 0,
                                 'a REG_OPS user, a reactivation or kind-1/2 evidence moves members; late shares (kind 3) do not')

    def test_a_malformed_page_is_refused(self):
        chain = PagedActList(3)
        good = chain.call
        chain.call = lambda to, sig, args, h, words=None: good(to, sig, args, h, words)[:-32] if sig.startswith('activeSlice') else good(to, sig, args, h, words)
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'Malformed activeSlice answer'):
            chain.active_list(1)
        chain = PagedActList(3)
        chain.members = [1, 1, 2]
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'Duplicate active miner'):
            chain.active_list(1)


class V3FlowTests(unittest.TestCase):
    """snapshot_v3's own reward-flow code (_Payments, _punish, _mark_close, _tip_share_record, _settle) on a fake chain."""
    MINER, REWARD, OTHER = ident(0x11), ident(0x22), ident(0x33)

    def chain(self, w0=1 | ACCRUING, bond=5 * PCOL, at=99, durable=True, escrow=10**24):
        c = FakeV3Chain()
        c.records[(self.MINER, at)] = (w0, int(self.REWARD, 16), bond, 0)
        c.accounts[(rewards.ESCROW, at)] = (1, '0x6000', escrow)
        c.accounts[(self.REWARD, at)] = c.accounts[(self.REWARD, at + 1)] = (1 if durable else 0, '0x', 0)
        return c

    def test_pay_burns_for_a_non_durable_recipient_an_underfunded_escrow_and_an_overflow(self):
        """PAY-02 (Sys.pay): each failure burns the payment; nothing reaches the address or leaves Escrow."""
        flows = rewards._Payments(self.chain(durable=False), 99)
        self.assertEqual(flows.pay(self.REWARD, 7, self.MINER), 7)
        self.assertEqual((flows.burned, flows.paid[self.MINER], flows.escrow), (7, 0, 10**24))
        flows = rewards._Payments(self.chain(escrow=6), 99)
        self.assertEqual(flows.pay(self.REWARD, 7, self.MINER), 7)
        self.assertEqual((flows.burned, flows.escrow), (7, 6))
        c = self.chain()
        c.accounts[(self.REWARD, 99)] = (1, '0x', rewards.MAX_U256 - 6)
        flows = rewards._Payments(c, 99)
        self.assertEqual(flows.pay(self.REWARD, 7, self.MINER), 7)
        self.assertEqual(flows.pay(self.REWARD, 6, self.MINER), 0)
        self.assertEqual((flows.burned, flows.paid[self.MINER], flows.escrow), (7, 6, 10**24 - 6))
        # A contract without a nonce is durable; the durability read follows the `at` asked for (the tip share reads h).
        c = self.chain(durable=False)
        c.accounts[(self.REWARD, 100)] = (0, '0x6000', 0)    # code by block 100, none at 99
        flows = rewards._Payments(c, 99)
        self.assertEqual(flows.pay(self.REWARD, 7, self.MINER, at=100), 0)
        self.assertEqual(flows.pay(self.REWARD, 7, self.MINER), 7)

    def test_an_accruing_record_fills_its_bond_and_is_promoted_at_b_reg(self):
        flows = rewards._Payments(self.chain(bond=B_REG - 3), 99)
        flows.pay_to(self.MINER, 10)
        self.assertEqual((flows.accrued[self.MINER], flows.paid[self.MINER]), (3, 7))
        self.assertEqual((flows.record(self.MINER)[0] >> 16 & 255, flows.record(self.MINER)[2]), (1, B_REG))
        flows.pay_to(self.MINER, 10)
        self.assertEqual((flows.accrued[self.MINER], flows.paid[self.MINER]), (3, 17))

    def closing_chain(self, w0, bond, durable=True, counters=0, n=1):
        c = self.chain(w0=w0, bond=bond, durable=durable)
        c.cp.update({45: 120, 46: 1, 47: 100, 48: 1 << 40})
        c.words[(rewards.REGISTRY, 'counters()', (), 99)] = [counters]
        c.words[(rewards.REGISTRY, 'lengths()', (), 99)] = [n, n, 0, 0]
        c.words[(rewards.REGISTRY, 'recordAt(uint256)', (0,), 99)] = [int(self.MINER, 16)]
        return c

    def after(self, c, cursor, close_h=100):
        c.words[(rewards.REGISTRY, 'counters()', (), 100)] = [(1 << 89) | (1 << 122) | (cursor << 90) | (close_h << 187)]

    def test_mark_close_refunds_an_accruing_payee_before_the_settlement_pays_it(self):
        """Closing starts at H_CLOSE: markClose refunds the ACCRUING record's bond to its reward address and makes it PAID,
        so the settlement and the tip share of the same block pay its reward address directly, with no bond accrual."""
        c = self.closing_chain(1 | ACCRUING, 5 * PCOL)
        self.after(c, 1)
        flows = rewards._Payments(c, 99)
        closing, visited = rewards._mark_close(c, flows, 100, 0)
        self.assertEqual((closing, visited), ({'closeHeight': 100, 'cursorDone': True}, [self.MINER]))
        self.assertEqual(flows.refunds, [{'minerId': self.MINER, 'rewardAddress': self.REWARD, 'amountWei': str(5 * PCOL)}])
        self.assertEqual(flows.record(self.MINER)[0] >> 16 & 255, 3)
        flows.pay_to(self.MINER, 3 * PCOL)
        self.assertEqual((flows.paid[self.MINER], flows.accrued[self.MINER]), (3 * PCOL, 0))
        self.assertEqual(flows.escrow, 10**24 - 8 * PCOL)
        # Before H_CLOSE (and T_CLOSE) nothing happens, and state h must not be closing either.
        c = self.closing_chain(1 | ACCRUING, 5 * PCOL)
        c.cp[47] = 101
        c.words[(rewards.REGISTRY, 'counters()', (), 100)] = [0]
        flows = rewards._Payments(c, 99)
        self.assertEqual(rewards._mark_close(c, flows, 100, 0), (None, []))
        flows.pay_to(self.MINER, 3 * PCOL)
        self.assertEqual((flows.paid[self.MINER], flows.accrued[self.MINER]), (0, 3 * PCOL))
        # The closing timestamp starts it too, and a wrong cursor at h fails closed.
        c = self.closing_chain(1 | HELD, B_REG)
        c.cp[47] = 1000
        self.after(c, 0)
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'Closing cursor differs'):
            rewards._mark_close(c, rewards._Payments(c, 99), 100, 1 << 40)

    def test_mark_close_leaves_a_record_live_when_its_refund_cannot_be_paid(self):
        c = self.closing_chain(1 | HELD, B_REG, durable=False)
        self.after(c, 1)
        flows = rewards._Payments(c, 99)
        rewards._mark_close(c, flows, 100, 0)
        self.assertEqual((flows.refunds, flows.record(self.MINER)[0] >> 16 & 255, flows.escrow), ([], 1, 10**24))
        # A cursor already running continues from where it stopped; a tomb or a PAID record is not refunded.
        for w0 in (1 | PAID, 1 | HELD | (1 << 8)):
            c = self.closing_chain(w0, B_REG, counters=(1 << 89) | (1 << 122) | (0 << 90) | (90 << 187))
            self.after(c, 1, 90)
            flows = rewards._Payments(c, 99)
            self.assertEqual(rewards._mark_close(c, flows, 100, 0)[0], {'closeHeight': 90, 'cursorDone': True})
            self.assertEqual(flows.refunds, [])

    def test_the_reporter_cut_needs_a_reporter_record(self):
        """V3-17 burnBond: rho of the burned bond to the reward address of the reporter's record; a reporter without a
        record (or no reporter, a fallback block) gets nothing and the whole bond burns."""
        c = self.chain(w0=1 | HELD, bond=B_REG)
        c.records[(self.OTHER, 99)] = (1 | HELD | (1 << 8), int(self.OTHER, 16), 0, 0)    # a tombed reporter keeps its cut
        c.accounts[(self.OTHER, 99)] = (1, '0x', 0)
        c.records[(ident(0x44), 99)] = (0, int(self.OTHER, 16), 0, 0)    # payee(): no record (w0 = 0) is no reward
        for reporter, cut in ((self.OTHER, 5 * PCOL), (ident(0x44), 0), (ident(0x45), 0), (None, 0)):
            flows = rewards._Payments(c, 99)
            bond, reward, paid = rewards._punish(flows, self.MINER, reporter, 2500)
            self.assertEqual((bond, paid, flows.burned), (B_REG, cut, B_REG - cut))
            self.assertEqual(reward, self.OTHER if cut else rewards.ZERO)
            self.assertEqual(flows.record(self.MINER)[0] >> 8 & 255, 1)
        # A PAID record is only tombed: nothing burns.
        c = self.chain(w0=1 | PAID, bond=B_REG)
        flows = rewards._Payments(c, 99)
        self.assertEqual(rewards._punish(flows, self.MINER, None, 2500), (0, rewards.ZERO, 0))

    def own_call(self, signature, status=1, sender=None):
        """A transaction of the block as tips() returns it: sender and receipt status, destination and selector."""
        return {'from': sender or self.MINER, 'to': rewards.REGISTRY, 'status': status,
                'selector': rewards.keccak256(signature.encode())[:4]}

    def test_the_tip_share_uses_the_record_the_user_phase_left(self):
        """closeBlock pays the tip share on the proposer's record after the user phase: a proposer's own successful
        registry call in the block (setRewardAddress, topUp) is read back from state h; a promotion by topUp is ambiguous
        and refused."""
        set_reward, top_up = [self.own_call('setRewardAddress(address)')], [self.own_call('topUp()')]
        new = ident(0x55)
        # setRewardAddress: HELD before and after, a new reward address.
        c = self.chain(w0=1 | HELD, bond=B_REG)
        c.records[(self.MINER, 100)] = (1 | HELD, int(new, 16), B_REG, 0)
        c.accounts[(new, 100)] = (1, '0x', 0)
        flows = rewards._Payments(c, 99)
        after = rewards._tip_share_record(c, flows, self.MINER, 10, 100, set_reward)
        flows.pay_to(self.MINER, 10, at=100)
        self.assertEqual((flows.rewards[self.MINER], flows.paid[self.MINER]), (new, 10))
        self.assertTrue(rewards._same_record(flows.record(self.MINER), after))
        # Without its own successful registry call (another destination, another sender, a reverted call) the reproduced
        # record must equal state h.
        for calls in ([dict(set_reward[0], to=self.OTHER)], [self.own_call('setRewardAddress(address)', sender=self.OTHER)],
                      [self.own_call('setRewardAddress(address)', status=0)]):
            flows = rewards._Payments(c, 99)
            after = rewards._tip_share_record(c, flows, self.MINER, 10, 100, calls)
            flows.pay_to(self.MINER, 10, at=100)
            self.assertFalse(rewards._same_record(flows.record(self.MINER), after))
        # topUp below B_reg: ACCRUING at h with bond b + topUp + share; the share accrues on b + topUp.
        c = self.chain(w0=1 | ACCRUING, bond=5 * PCOL)
        c.records[(self.MINER, 100)] = (1 | ACCRUING, int(self.REWARD, 16), 6 * PCOL + 10, 0)
        flows = rewards._Payments(c, 99)
        after = rewards._tip_share_record(c, flows, self.MINER, 10, 100, top_up)
        flows.pay_to(self.MINER, 10, at=100)
        self.assertEqual((flows.accrued[self.MINER], flows.paid[self.MINER]), (10, 0))
        self.assertTrue(rewards._same_record(flows.record(self.MINER), after))
        # A topUp that may have promoted it before the share: refused.
        c.records[(self.MINER, 100)] = (1 | HELD, int(self.REWARD, 16), B_REG, 0)
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'ambiguous'):
            rewards._tip_share_record(c, rewards._Payments(c, 99), self.MINER, 10, 100, top_up)
        # The same promotion without a registry call of its own is the share's doing.
        c = self.chain(w0=1 | ACCRUING, bond=B_REG - 4)
        c.records[(self.MINER, 100)] = (1 | HELD, int(self.REWARD, 16), B_REG, 0)
        flows = rewards._Payments(c, 99)
        after = rewards._tip_share_record(c, flows, self.MINER, 10, 100, [])
        flows.pay_to(self.MINER, 10, at=100)
        self.assertEqual((flows.accrued[self.MINER], flows.paid[self.MINER]), (4, 6))
        self.assertTrue(rewards._same_record(flows.record(self.MINER), after))

    def test_only_a_successful_top_up_makes_the_share_promotion_ambiguous(self):
        """setRewardAddress (or any registry call but topUp) keeps the bond and its state, and a reverted topUp changes
        nothing: a record phase 0 left ACCRUING 4 wei below B_reg and HELD at h was promoted by the share of 10, which
        accrues 4 and pays 6 (to the new reward address after setRewardAddress)."""
        new = ident(0x55)
        for calls, reward in (([self.own_call('setRewardAddress(address)')], new),
                              ([self.own_call('topUp()', status=0)], self.REWARD),
                              ([self.own_call('topUp()', status=0), self.own_call('setRewardAddress(address)')], new)):
            c = self.chain(w0=1 | ACCRUING, bond=B_REG - 4)
            c.records[(self.MINER, 100)] = (1 | HELD, int(reward, 16), B_REG, 0)
            c.accounts[(new, 100)] = (1, '0x', 0)
            flows = rewards._Payments(c, 99)
            after = rewards._tip_share_record(c, flows, self.MINER, 10, 100, calls)
            flows.pay_to(self.MINER, 10, at=100)
            self.assertEqual((flows.accrued[self.MINER], flows.paid[self.MINER], flows.rewards[self.MINER]), (4, 6, reward))
            self.assertEqual(dict(flows.accrued_to), {(self.MINER, reward): 4})
            self.assertTrue(rewards._same_record(flows.record(self.MINER), after))
        # setRewardAddress on a record that stays ACCRUING: the share accrues on the phase-0 bond, and a state h that does
        # not hold that bond plus the share is caught.
        c = self.chain(w0=1 | ACCRUING, bond=5 * PCOL)
        c.records[(self.MINER, 100)] = (1 | ACCRUING, int(new, 16), 5 * PCOL + 10, 0)
        flows = rewards._Payments(c, 99)
        after = rewards._tip_share_record(c, flows, self.MINER, 10, 100, [self.own_call('setRewardAddress(address)')])
        flows.pay_to(self.MINER, 10, at=100)
        self.assertEqual((flows.accrued[self.MINER], flows.paid[self.MINER]), (10, 0))
        self.assertTrue(rewards._same_record(flows.record(self.MINER), after))
        c.records[(self.MINER, 100)] = (1 | ACCRUING, int(new, 16), 6 * PCOL + 10, 0)
        flows = rewards._Payments(c, 99)
        after = rewards._tip_share_record(c, flows, self.MINER, 10, 100, [self.own_call('setRewardAddress(address)')])
        flows.pay_to(self.MINER, 10, at=100)
        self.assertFalse(rewards._same_record(flows.record(self.MINER), after))
        # A successful topUp next to setRewardAddress still makes the promotion ambiguous.
        c = self.chain(w0=1 | ACCRUING, bond=B_REG - 4)
        c.records[(self.MINER, 100)] = (1 | HELD, int(self.REWARD, 16), B_REG, 0)
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'ambiguous'):
            rewards._tip_share_record(c, rewards._Payments(c, 99), self.MINER, 10, 100,
                                      [self.own_call('setRewardAddress(address)'), self.own_call('topUp()')])

    def test_transaction_destination_and_selector_come_from_the_committed_bytes(self):
        """_tx_call decodes the three forms lp4-execution accepts (legacy, 0x01, 0x02); a creation has no destination."""
        to, data = bytes.fromhex(rewards.REGISTRY[2:]), rewards.TOP_UP
        one, zero = b'\x01', b''
        legacy = rewards.rlp_encode([zero, one, one, to, one, data, one, one, one])
        eip2930 = b'\x01' + rewards.rlp_encode([one, zero, one, one, to, one, data, [], zero, one, one])
        eip1559 = b'\x02' + rewards.rlp_encode([one, zero, one, one, one, to, one, data, [[to, [b'\x00' * 32]]], zero, one, one])
        for raw in (legacy, eip2930, eip1559):
            self.assertEqual(rewards._tx_call(raw), (rewards.REGISTRY, data))
        self.assertEqual(rewards._tx_call(rewards.rlp_encode([zero, one, one, zero, one, b'\x60\x00', one, one, one])),
                         (None, b'\x60\x00'))
        for raw, message in ((b'\x03' + eip1559[1:], 'Unsupported transaction type'), (b'\x02' + legacy, 'Malformed transaction'),
                             (rewards.rlp_encode([zero, one, one, to[:19], one, data, one, one, one]), 'Malformed transaction')):
            with self.assertRaisesRegex(rewards.RewardUnavailable, message):
                rewards._tx_call(raw)

    def test_the_earliest_final_block_follows_the_host_final_predicate(self):
        """finality_core.rs final_predicate: F >= closeHeight + D and (F >= H_END or ts >= T_END), with either end
        nonzero. T_END = 0 makes the time condition always true, so H_END (only a ceiling) never delays F."""
        def final(h, ts, close_h, d, h_end, t_end):
            return (h_end != 0 or t_end != 0) and h >= close_h + d and (h >= h_end or ts >= t_end)
        # The counterexample: H_END 1000, T_END 0, closing at 100, D 4: block 104 is final at any timestamp, so pot 100
        # settles there with its base burn burned.
        self.assertTrue(final(104, 0, 100, 4, 1000, 0))
        self.assertEqual(rewards.earliest_final_height(100, 100, 4, 1000, 0), 104)
        for h_end, t_end in ((1000, 0), (0, 1 << 63), (1000, 1 << 63), (101, 5)):
            for h in range(100, 110):
                earliest = rewards.earliest_final_height(100, h, 4, h_end, t_end)
                # A lower bound that some timestamp attains.
                self.assertEqual(earliest, next(f for f in range(h + 1, h + 20) if final(f, 1 << 64, 100, 4, h_end, t_end)))
        self.assertIsNone(rewards.earliest_final_height(100, 100, 4, 0, 0))


class VectorChain(FakeV3Chain):
    """A model chain scenario (tests/v3/*.json) as the reads _settle makes: pots from its close steps, owners and winners
    from its begin steps, every record HELD with its own durable reward address (the scenario has no evidence)."""

    def __init__(self, vector):
        cp = dict(MODEL_DEFAULT, **{k: int(v) for k, v in vector['cp'].items()})
        super().__init__({8: cp['m'], 9: cp['kappa'], 11: cp['D'], 13: cp['alpha_bp'], 14: cp['gamma_bp'], 16: B_REG})
        self.gamma_tip = cp['gamma_tip_bp']
        self.ids = {}
        self.pots, self.owners, self.proposers = {}, {}, {}

    def id(self, name):
        return self.ids.setdefault(name, ident(int.from_bytes(name.encode(), 'big')))

    def result(self, method, params):
        return {'eth_getBalance': hex(10**30), 'eth_getTransactionCount': '0x1', 'eth_getCode': '0x'}[method]

    def record(self, miner, height):
        return (1 | HELD, int(miner, 16), B_REG, 0)

    def pending(self, j, at):
        winner = self.owners[j][-1] if j < at else None
        meta = int(self.proposers[j], 16) << 8 if self.proposers[j] else 1
        return self.pots[j], meta, int(winner, 16) if winner else 0

    def header(self, j):
        return {'nonce': j, 'signed': bool(self.proposers[j]), 'proposer': self.proposers[j] or '0x'}

    def owner(self, j, nonce):
        return self.owners[j][-1]

    def header_owners(self, j):
        return list(self.owners[j])

    def evidence(self, b, kinds):
        return []

    def ledger(self, j, h):
        counts = Counter(self.owners[j])
        return list(counts), list(counts.values()), sum(counts.values())


TAPE_PATH = Path(__file__).resolve().parent / 'testdata' / 'v3-devnet-tape.json'
# Block and receipt fields snapshot_v3 never reads, and the pocol_getParams build-identity fields (they change with any
# node source change, not with the chain), left out of the tape.
UNREAD_FIELDS = ('logsBloom', 'logs', 'compatibilitySha256', 'consensusCompatibilitySha256', 'consensusIdentity',
                 'consensusPinned', 'consensusRulesVersion', 'rpcLimits')


def _tape_key(method, params):
    return json.dumps([method, params], separators=(',', ':'))


def record_v3_tape(rpc, fixture, path, heights):
    """Records the RPC answers snapshot_v3 reads for `heights` of the v3 devnet fixture (pocold tests/v3_reward_gateway.rs,
    served with POCOL_V3_DEVNET_SERVE) into `path`, with the snapshots they give. Usage, from deploy/rewards:
    POCOL_V3_DEVNET="$(cat FILE)" python3 -B -c 'import json, os, test_pocol_rewards as t; fx = json.loads(os.environ["POCOL_V3_DEVNET"]);
    t.V3DevnetDifferential.fx = fx; t.record_v3_tape(t.V3DevnetDifferential.rpc, fx, t.TAPE_PATH, [31, 32, 34, 36, 46, 50])'"""
    tape, snaps = {}, {}
    def record(method, params):
        answer = rpc(method, params)
        result = answer['result']
        if isinstance(result, dict):
            result = {k: v for k, v in result.items() if k not in UNREAD_FIELDS}
        assert tape.setdefault(_tape_key(method, params), result) == result, (method, params)
        return answer
    network = {'chainId': fixture['chainId'], 'genesisHash': fixture['genesisHash'], 'specVersion': 2, 'v3': True}
    for h in heights:
        snap = rewards.snapshot_v3(record, fork_schedule=tuple(map(tuple, fixture['forkSchedule'])), network=network, height=h)
        snap.pop('fetchedAt')
        snaps[str(h)] = snap
    data = {'fixture': {k: v for k, v in fixture.items() if k not in ('rpc', 'head')}, 'snapshots': snaps, 'tape': dict(sorted(tape.items()))}
    Path(path).write_text(json.dumps(data, separators=(',', ':'), sort_keys=True) + '\n')


class Tape:
    """A JSON-RPC endpoint replaying a recorded tape; `edit(method, params, result, nth)` may change the nth answer to a
    call (1-based). A call the tape does not hold is a node error."""

    def __init__(self, data, edit=None):
        self.answers, self.edit, self.seen = data['tape'], edit, Counter()

    def __call__(self, method, params):
        key = _tape_key(method, params)
        self.seen[key] += 1
        result = copy.deepcopy(self.answers.get(key))
        if self.edit is not None:
            result = self.edit(method, params, result, self.seen[key])
        if key not in self.answers and result is None:
            return {'jsonrpc': '2.0', 'id': 1, 'error': {'code': -32000, 'message': 'not recorded'}}
        return {'jsonrpc': '2.0', 'id': 1, 'result': result}


class V3RecordedTapeTests(unittest.TestCase):
    """snapshot_v3 over RPC answers recorded from the v3 devnet chain (testdata/v3-devnet-tape.json, blocks 31, 32, 34, 36,
    46 and 50): the honest tape reproduces the recorded snapshots, and each tampered answer fails closed."""

    @classmethod
    def setUpClass(cls):
        cls.data = json.loads(TAPE_PATH.read_text())
        fx = cls.data['fixture']
        cls.fx = fx
        cls.network = {'chainId': fx['chainId'], 'genesisHash': fx['genesisHash'], 'specVersion': 2, 'v3': True}
        cls.schedule = tuple(map(tuple, fx['forkSchedule']))

    def snap(self, h, edit=None):
        out = rewards.snapshot_v3(Tape(self.data, edit), fork_schedule=self.schedule, network=self.network, height=h)
        out.pop('fetchedAt')
        return out

    def refused(self, h, edit, message, kind=rewards.RewardUnavailable):
        with self.assertRaisesRegex(kind, message):
            self.snap(h, edit)

    @staticmethod
    def on(method, match, change):
        """An edit applying change(result) to every answer of `method` whose params satisfy match(params)."""
        def edit(m, params, result, nth):
            return change(result) if m == method and match(params) else result
        return edit

    @staticmethod
    def contract(to, signature, height, args=None):
        sel = '0x' + rewards.keccak256(signature.encode())[:4].hex()
        def match(params):
            call, tag = params
            data = call['data']
            return (call['to'] == to and data.startswith(sel) and tag == hex(height) and
                    (args is None or data[10:] == ''.join(a.to_bytes(32, 'big').hex() for a in args)))
        return match

    @staticmethod
    def add_word(index, delta):
        def change(result):
            words = [result[2 + 64*i:66 + 64*i] for i in range((len(result) - 2) // 64)]
            words[index] = '%064x' % (int(words[index], 16) + delta)
            return '0x' + ''.join(words)
        return change

    def forge_anchor(self, h, change_body=None, change_ut=None):
        """An edit rewriting block h consistently (header roots, template, hash) after change_body(txs, evidence) or
        change_ut(ut): the snapshot then sees a different block whose RPC answers agree with each other."""
        encoded = self.data['tape'][_tape_key('pocol_getBlockRlp', [hex(h)])]
        top = rewards.rlp_decode(bytes.fromhex(encoded['header'][2:]))
        body = rewards.rlp_decode(bytes.fromhex(encoded['body'][2:]), 1 << 22, 1 << 20)
        if change_body:
            body = change_body(*body)
            top[0][0][10] = rewards.ordered_root(body[0])
            top[0][0][16] = rewards.ordered_root(body[1])
        if change_ut:
            change_ut(top)
        ut = top[0][0]
        tid = hashlib.sha256(rewards.rlp_encode(ut)).digest()
        block_hash = '0x' + rewards.keccak256(tid + top[1] + rewards.keccak256(rewards.rlp_encode(top[2]))).hex()
        header = '0x' + rewards.rlp_encode(top).hex()
        def edit(method, params, result, nth):
            if method == 'pocol_getBlockRlp' and params == [hex(h)]:
                return {'header': header, 'body': '0x' + rewards.rlp_encode(body).hex()}
            if method == 'eth_getBlockByNumber' and params == [hex(h), False]:
                result.update(hash=block_hash, transactions=['0x' + rewards.keccak256(t).hex() for t in body[0]])
                result['pocol'].update(templateId='0x' + tid.hex(), signed=bool(ut[17]), proposer='0x' + ut[17].hex())
            return result
        return edit

    def test_the_recorded_answers_reproduce_the_recorded_snapshots(self):
        self.assertEqual(sorted(map(int, self.data['snapshots'])), [31, 32, 34, 36, 46, 50])
        for h, expected in self.data['snapshots'].items():
            with self.subTest(h=h):
                self.assertEqual(self.snap(int(h)), expected)

    def test_a_body_that_its_header_does_not_commit_is_refused(self):
        def drop_evidence(result):
            body = rewards.rlp_decode(bytes.fromhex(result['body'][2:]), 1 << 22, 1 << 20)
            return dict(result, body='0x' + rewards.rlp_encode([body[0], []]).hex())
        self.refused(32, self.on('pocol_getBlockRlp', lambda p: p == [hex(32)], drop_evidence), 'not the one its header commits')
        # An ancestor's body too (block 31's evidence, read for the carry into pot 32).
        self.refused(32, self.on('pocol_getBlockRlp', lambda p: p == [hex(31)], drop_evidence), 'not the one its header commits')

    def test_an_ancestor_must_link_to_its_child(self):
        def other_nonce(result):
            top = rewards.rlp_decode(bytes.fromhex(result['header'][2:]))
            top[1] = (int.from_bytes(top[1], 'big') ^ 1).to_bytes(8, 'big')
            return dict(result, header='0x' + rewards.rlp_encode(top).hex())
        self.refused(32, self.on('pocol_getBlockRlp', lambda p: p == [hex(28)], other_nonce), 'Ancestor header mismatch')

    def test_receipts_and_the_transaction_list_are_bound_to_the_block(self):
        receipt = lambda p: True
        self.refused(32, self.on('eth_getTransactionReceipt', receipt, lambda r: dict(r, blockHash='0x' + '00' * 32)), 'Receipt of another block')
        self.refused(32, self.on('eth_getTransactionReceipt', receipt, lambda r: dict(r, blockNumber='0x1f')), 'Receipt of another block')
        self.refused(32, self.on('eth_getBlockByNumber', lambda p: p == [hex(32), False],
                                 lambda b: dict(b, transactions=['0x' + '11' * 32])), 'Block transactions differ from its body')
        self.refused(32, self.on('eth_getBlockByNumber', lambda p: p == [hex(32), False],
                                 lambda b: dict(b, transactions=[])), 'Block transactions differ from its body')
        # The destination is the committed transaction's, the status 0 or 1.
        own = self.fx['ownRegistryCallBlock']
        self.refused(own, self.on('eth_getTransactionReceipt', receipt, lambda r: dict(r, to=self.fx['members']['erin'])),
                     'Receipt of another transaction')
        self.refused(own, self.on('eth_getTransactionReceipt', receipt, lambda r: dict(r, status='0x2')), 'Receipt of another transaction')
        self.refused(own, self.on('eth_getTransactionReceipt', receipt, lambda r: dict(r, status=None)), 'Malformed quantity')
        # The proposer's setRewardAddress claimed reverted: the record phase 0 left (the old reward address) is not state h's.
        # (The old reward address's durability at h, which the tape did not record, is served as durable.)
        def reverted(method, params, result, nth):
            if method == 'eth_getTransactionReceipt':
                return dict(result, status='0x0')
            if result is None and method in ('eth_getTransactionCount', 'eth_getBalance'):
                return '0x1' if method == 'eth_getTransactionCount' else '0x0'
            return result
        self.refused(own, reverted, 'Proposer record differs')

    def test_the_share_ledger_must_hold_the_reconstructed_owners(self):
        # Block 32 settles pot 28 with its header owners and the two late shares block 32 itself credits.
        self.refused(32, self.on('eth_call', self.contract(rewards.LEDGER, 'entriesOf(uint64)', 32, [28]), self.add_word(2, 1)),
                     'ShareLedger differs')
        self.refused(32, self.on('eth_call', self.contract(rewards.LEDGER, 'entriesOf(uint64)', 32, [28]), self.add_word(-1, -1)),
                     'ShareLedger differs')

    def test_pots_carries_and_burns_must_match_their_reconstruction(self):
        escrow = lambda signature, height, args=None: self.contract(rewards.ESCROW, signature, height, args)
        self.refused(32, self.on('eth_call', escrow('pending(uint256)', 32, [32]), self.add_word(0, 1)), 'Pending pot differs')
        self.refused(32, self.on('eth_call', escrow('pending(uint256)', 32, [32]), self.add_word(1, 1)), 'inconsistent pending pot')
        self.refused(32, self.on('eth_call', escrow('carry()', 32), self.add_word(0, 1)), 'Carry differs from the reconstructed base burn')
        self.refused(32, self.on('eth_call', escrow('carry()', 31), self.add_word(0, 1)), 'Carry into the pot differs')
        self.refused(32, self.on('eth_call', escrow('burnedTotal()', 32), self.add_word(0, 1)), 'Burned total differs')
        # Block 31's evidence invalidates pot 27 in the block that settles it: that pot burns whole, no carry follows.
        self.refused(31, self.on('eth_call', escrow('carry()', 31), self.add_word(0, 1)), 'Carry differs')

    def test_the_proposer_record_and_the_closing_cursor_must_match_their_reconstruction(self):
        proposer = int(self.data['snapshots']['32']['block']['tipShare']['proposer'], 16)
        record = self.contract(rewards.REGISTRY, 'recordWords(address)', 32, [proposer])
        self.refused(32, self.on('eth_call', record, self.add_word(2, 1)), 'Proposer record differs')
        self.refused(32, self.on('eth_call', record, self.add_word(1, 1)), 'Proposer record differs')
        close = self.fx['closeHeight']
        self.refused(close, self.on('eth_call', self.contract(rewards.REGISTRY, 'counters()', close), self.add_word(0, 1 << 90)),
                     'Closing cursor differs')
        self.refused(close, self.on('eth_call', self.contract(rewards.REGISTRY, 'counters()', close), self.add_word(0, -(1 << 89))),
                     'Closing cursor differs')
        three = int(self.fx['members']['three'], 16)
        self.refused(close, self.on('eth_call', self.contract(rewards.REGISTRY, 'recordWords(address)', close, [three]),
                                    self.add_word(0, -(2 << 16))), 'Closing refund differs')
        # Not closing yet at 32: a closing flag at h is a contradiction.
        self.refused(32, self.on('eth_call', self.contract(rewards.REGISTRY, 'counters()', 32), self.add_word(0, 1 << 89)),
                     'Closing state differs')

    def test_the_final_block_must_leave_no_pot_and_no_carry(self):
        final = self.fx['finalHeight']
        escrow = lambda signature, height, args=None: self.contract(rewards.ESCROW, signature, height, args)
        self.refused(final, self.on('eth_call', escrow('carry()', final), self.add_word(0, 1)), 'Pending pots or carry after the final block')
        self.refused(final, self.on('eth_call', escrow('pending(uint256)', final, [final - 1]), self.add_word(1, 1)),
                     'Pending pots or carry after the final block')
        self.refused(final, self.on('eth_call', escrow('burnedTotal()', final), self.add_word(0, -1)), 'Burned total differs')
        # The final flag decides which phase 2 ran: claiming the other one fails closed either way.
        flip = lambda flag: self.on('eth_getBlockByNumber', lambda p: True, lambda b: dict(b, pocol=dict(b['pocol'], final=flag)))
        self.refused(final, flip(False), 'Carry differs')
        self.refused(32, flip(None), 'Missing final flag')
        # Block 32 claimed final, with the node answering the reads of the final path: no pot of [29, 31] pending at 31
        # (so no settleFinal), then also none at 32 and no carry left.
        pending = rewards.keccak256(b'pending(uint256)')[:4].hex()
        def claim_final(empty_after):
            def edit(method, params, result, nth):
                if method == 'eth_getBlockByNumber':
                    return dict(result, pocol=dict(result['pocol'], final=True))
                if method != 'eth_call' or params[0]['to'] != rewards.ESCROW:
                    return result
                data, tag = params[0]['data'], params[1]
                if data[2:10] == pending and int(data[10:], 16) >= 29 and tag in ([hex(31), hex(32)] if empty_after else [hex(31)]):
                    return '0x' + '00' * 96
                return '0x' + '00' * 32 if empty_after and escrow('carry()', 32)(params) else result
            return edit
        self.refused(32, claim_final(False), 'Pending pots or carry after the final block')
        self.refused(32, claim_final(True), 'Burned total differs')

    def test_the_tip_share_is_paid_after_the_user_phase(self):
        """closeBlock runs after the user phase: Escrow then also holds the block's tips and issuance, and the PAY-02
        durability of the proposer's reward address is that of state h."""
        snap = self.data['snapshots']['32']
        reward = snap['block']['beneficiaries'][0]['rewardAddress']
        phase0 = int(snap['block']['paidWei']) - int(snap['block']['tipShare']['paidWei'])
        self.assertGreater(int(snap['block']['tipShare']['paidWei']), 0)
        # Escrow at h-1 holds exactly what phase 0 pays out: the tip share comes out of the tips and the issuance.
        escrow = self.on('eth_getBalance', lambda p: p == [rewards.ESCROW, hex(31)], lambda r: hex(phase0))
        self.assertEqual(self.snap(32, escrow)['block'], snap['block'])
        self.refused(32, self.on('eth_getBalance', lambda p: p == [rewards.ESCROW, hex(31)], lambda r: hex(phase0 - 1)),
                     'Burned total differs')
        # The reward address without a nonce or code at h (where closeBlock pays) burns the share; at h-1 it is not read.
        def gone(method, params, result, nth):
            if method == 'eth_getTransactionCount' and params == [reward, hex(32)]:
                return '0x0'
            return '0x' if method == 'eth_getCode' and params == [reward, hex(32)] else result
        self.refused(32, gone, 'Burned total differs')

    def test_a_changed_anchor_is_reported_as_a_changed_snapshot(self):
        def edit(method, params, result, nth):
            if method == 'eth_getBlockByNumber' and nth == 2:
                return dict(result, hash='0x' + '00' * 32)
            return result
        self.refused(32, edit, 'Canonical block changed', rewards.SnapshotChanged)

    def test_forged_blocks_fail_closed(self):
        # Late shares for a template outside [h - D, h - 1]: block 32 claiming them for template 27.
        def late_for(j):
            def change(txs, evidence):
                items = [rewards.rlp_decode(e, 65536) for e in evidence]
                for item in items:
                    if item[0] == b'\x03':
                        item[1] = j.to_bytes(8, 'big')
                return [txs, [rewards.rlp_encode(item) for item in items]]
            return change
        self.refused(32, self.forge_anchor(32, late_for(27)), 'Late shares outside the window')
        self.refused(32, self.forge_anchor(32, late_for(32)), 'Late shares outside the window')
        # A fallback block (no proposer) carrying a body.
        def unsigned(top):
            top[0][0][17] = b''
            top[0][1] = b''
        self.refused(32, self.forge_anchor(32, change_ut=unsigned), 'A fallback block carries a body')
        # The winner named by the block must own the winning nonce.
        other = self.fx['members']['hank']
        self.refused(32, self.on('eth_getBlockByNumber', lambda p: True, lambda b: dict(b, miner=other)), 'Winner assignment mismatch')

    def test_an_ancestor_outside_the_reward_window_is_never_read(self):
        query = Tape(self.data)
        net = rewards.validated_network(self.network)
        cp = rewards._v3_cp(query('pocol_getParams', [])['result'], net)
        chain = rewards._V3Chain(query, net, self.schedule, cp)
        encoded = query('pocol_getBlockRlp', [hex(32)])['result']
        chain.set_anchor(rewards.decode_header(bytes.fromhex(encoded['header'][2:]), self.schedule, net), encoded['body'])
        window = cp[26] + cp[11] + 2
        with self.assertRaisesRegex(rewards.RewardUnavailable, 'Ancestor outside the reward window'):
            chain.header(32 - window - 1)

    def test_malformed_answer_types_fail_closed(self):
        """Every field of every answer the snapshot of block 32 reads, replaced by a value of another JSON type, gives
        RewardUnavailable (or SnapshotChanged), never another exception."""
        values = (None, 7, True, [], 'x')
        def paths(value, prefix=()):
            yield prefix
            if isinstance(value, dict):
                for k, v in value.items():
                    yield from paths(v, prefix + (k,))
            elif isinstance(value, list) and len(value) < 8:
                for i, v in enumerate(value):
                    yield from paths(v, prefix + (i,))
        def replace(value, path, new):
            if not path:
                return new
            value = copy.deepcopy(value)
            cursor = value
            for k in path[:-1]:
                cursor = cursor[k]
            cursor[path[-1]] = new
            return value
        honest = Tape(self.data)
        rewards.snapshot_v3(honest, fork_schedule=self.schedule, network=self.network, height=32)
        cases = 0
        for key in sorted(honest.seen):
            if key not in self.data['tape']:
                continue
            for path in paths(self.data['tape'][key]):
                for new in values if not key.startswith('["eth_call"') else (None, 7):
                    def edit(method, params, result, nth, key=key, path=path, new=new):
                        return replace(result, path, new) if _tape_key(method, params) == key and nth == 1 else result
                    try:
                        self.snap(32, edit)
                    except rewards.RewardUnavailable:
                        pass
                    except Exception as error:      # noqa: BLE001 - the point of the test
                        self.fail('%s at %s %s: %r' % (key[:60], path, new, error))
                    cases += 1
        self.assertGreater(cases, 450)     # every read field of every answer; the build-identity fields are not on the tape


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
        net = rewards.validated_network(cls.network)
        cls.d = rewards._v3_cp(cls.rpc('pocol_getParams', [])['result'], net)[11]
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
                for b in block['bondRefunds']:          # markClose's refunds (closing)
                    by_reward[b['rewardAddress']] += int(b['amountWei'])
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
                self.assertEqual(carry, int(block['settlement']['carryOutWei']) if block['settlement'] and not snap['final'] else 0)
                if snap['final']:
                    # finalize: no pot F, the tips and both carries burn, no pending pot is left.
                    self.assertEqual((snap['grossRewardWei'], snap['issuedWei'], snap['tipShareWei']), ('0', '0', '0'))
                    self.assertEqual(int(block['finalize']['carryBurnedWei']),
                                     int(snap['carryInWei']) + int(block['settlement']['carryOutWei']))
                    self.assertTrue(all(self.call(ESCROW, 'pending(uint256)', [j], h)[1] == 0 for j in range(h - self.d + 1, h + 1)))
                else:
                    self.assertEqual(int(snap['grossRewardWei']), int(snap['issuedWei']) + int(snap['tipsWei'])
                                     - int(snap['tipShareWei']) + int(snap['carryInWei']))

    def test_the_recorded_tape_is_this_chain(self):
        """testdata/v3-devnet-tape.json (V3RecordedTapeTests) holds this fixture chain's own answers and snapshots; after a
        change to the fixture chain it must be recorded again (record_v3_tape)."""
        data = json.loads(TAPE_PATH.read_text())
        self.assertEqual(data['fixture'], {k: v for k, v in self.fx.items() if k not in ('rpc', 'head')},
                         'the fixture chain changed: record testdata/v3-devnet-tape.json again (record_v3_tape)')
        for key, result in data['tape'].items():
            method, params = json.loads(key)
            live = self.read(method, params)
            if isinstance(live, dict):
                live = {k: v for k, v in live.items() if k not in UNREAD_FIELDS}
            self.assertEqual(live, result, key)
        for h, recorded in data['snapshots'].items():
            self.assertEqual({k: v for k, v in self.snaps[int(h)].items() if k != 'fetchedAt'}, recorded)

    def test_the_owners_of_every_pot_are_the_ledger_owners(self):
        d = self.d
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
                for settled in snap['block'].get('finalize', {}).get('settlements', []):
                    self.assertEqual(self.ledger(settled['potHeight'], h), Counter({o['minerId']: o['shares'] for o in settled['owners']}))

    def test_an_undisturbed_projection_is_its_settlement(self):
        """A pot nothing changes between its block and its settlement (no late share, no evidence, no bond filled, no
        reward address changed) settles exactly as the block's own projection says, at j + D or at the final block."""
        compared, final = 0, 0
        d, head = self.d, self.fx['head']
        for j, snap in self.snaps.items():
            if snap['final']:
                continue
            # Pot j settles at j + D, or at the final block F (settleFinal, its base burn burned) when F < j + D.
            at = min(j + d, head)
            if at == head and not self.snaps[head]['final'] and j + d > head:
                continue
            block = self.snaps[at]['block']
            settled = block['settlement'] if at == j + d else next(s for s in block['finalize']['settlements'] if s['potHeight'] == j)
            quiet = (settled['lateShareCount'] == 0 and not settled['invalid'] and
                     not any(self.snaps[b]['block']['evidence'] for b in range(j + 1, at + 1)) and
                     not j < self.fx['ownRegistryCallBlock'] <= at and          # a reward address changed in between
                     not any('bondAccruedWei' in b for b in snap['beneficiaries'] + settled['beneficiaries']))
            if not quiet:
                continue
            with self.subTest(j=j):
                self.assertEqual(settled['beneficiaries'], snap['beneficiaries'])
                self.assertEqual(settled['baseBurnWei'], snap['projectedCarryWei'])
                self.assertEqual(settled['carryOutWei'], '0' if at < j + d else snap['projectedCarryWei'])
                self.assertEqual(settled['grossRewardWei'], snap['grossRewardWei'])
                # The closing projection says when the final block may settle it instead.
                self.assertEqual(bool(snap['closing'] and snap['closing']['settlementMayBeFinal']), snap['closing'] is not None and
                                 snap['closing']['earliestFinalHeight'] <= j + d)
                compared += 1
                final += at < j + d
        self.assertGreater(compared, 5)
        self.assertGreater(final, 0)

    def test_the_chain_exercises_every_v3_reward_rule(self):
        fx, m = self.fx, self.fx['members']
        rho = 2500
        # Tips and the tip share (V3-20), issuance M3 (V3-19), the lagged carry (V3-15).
        self.assertTrue(any(int(s['tipShareWei']) > 0 and int(s['tipShareWei']) * 2 == int(s['tipsWei']) for s in self.snaps.values()))
        self.assertTrue(any(int(s['carryInWei']) > 0 for s in self.snaps.values()))
        self.assertTrue(all(s['issuedWei'] == ('0' if s['final'] else '2000000000000000000') for s in self.snaps.values()))
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
        # registerReplacing at the full set (FN-2b C4): three exited as by deregister; alice exited, still ACCRUING (no
        # POOL_OUT, w0 bit 87, since FN-2b), out of the next block's owners.
        three = self.records(fx['replacingHeld'])[m['three']]
        self.assertEqual((three[0] & 255, (three[0] >> 8) & 255), (2, 0))
        alice = self.records(fx['replacingAccruing'])[m['alice']]
        self.assertEqual((alice[0] & 255, (alice[0] >> 16) & 255, (alice[0] >> 87) & 1), (2, 4, 0))
        self.assertNotIn(m['alice'], {o['minerId'] for o in self.snaps[fx['replacingAccruing'] + 1]['owners']})
        # The exited victim is payable: its share of a pending pot fills its accrued bond.
        paid = {b['minerId']: b for b in self.snaps[fx['victimPaid']]['block']['settlement']['beneficiaries']}
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

    def test_the_chain_exercises_the_later_v3_reward_paths(self):
        fx, m, d, snaps = self.fx, self.fx['members'], self.d, self.snaps
        def evidence(pot, kind=2):
            return [e for s in snaps.values() for e in s['block']['evidence'] if e['kind'] == kind and e['potHeight'] == pot]
        # Type-2 evidence against a share owner who is not the winner (bob, for his share nonce of pot 25, won by carol
        # ACCRUING): bob burns, pot 25 is not invalidated and settles at 29, carol's parts fill her bond, bob's burn.
        pot = fx['shareOwnerAccusedPot']
        ev = evidence(pot)
        self.assertEqual([(e['accused'], e['potInvalidated']) for e in ev], [(m['bob'], False)])
        settled = snaps[pot + d]['block']['settlement']
        self.assertEqual((settled['invalid'], settled['winner'], settled['winnerAccruing']), (False, m['carol'], True))
        payees = {b['minerId']: b for b in settled['beneficiaries']}
        self.assertNotIn(m['bob'], payees)
        self.assertGreater(int(payees[m['carol']]['bondAccruedWei']), 0)
        self.assertGreater(int(settled['burnedWei']), 0)
        # Evidence in block h against pot h - D's ACCRUING winner: invalidate runs before settle(h - D), so the pot burns
        # whole in the same block and no carry follows it.
        pot = fx['sameBlockBurnPot']
        ev = evidence(pot)
        self.assertEqual([(e['accused'], e['potInvalidated']) for e in ev], [(m['carol'], True)])
        self.assertEqual(ev[0], snaps[pot + d]['block']['evidence'][0])
        settled = snaps[pot + d]['block']['settlement']
        self.assertEqual((settled['potHeight'], settled['invalid'], settled['burnedWei'], settled['carryOutWei']),
                         (pot, True, settled['grossRewardWei'], '0'))
        self.assertEqual(snaps[pot + d + 1]['carryInWei'], '0')
        # Late shares carried by the block that settles their pot (kind 3 for template h - D) are paid in that settlement.
        pot = fx['lateNowPot']
        late = [e for e in snaps[pot + d]['block']['evidence'] if e['kind'] == 3]
        self.assertEqual([(e['potHeight'], len(e['owners'])) for e in late], [(pot, 2)])
        self.assertEqual(snaps[pot + d]['block']['settlement']['lateShareCount'], 2)
        # Evidence against the parent block's ACCRUING winner: recordResult marks it in the same phase 0, so pot 33 is
        # invalidated and burns whole at 37.
        pot = fx['parentEvidencePot']
        ev = evidence(pot)
        self.assertEqual([(e['accused'], e['potInvalidated']) for e in ev], [(m['dave'], True)])
        self.assertEqual(ev[0], snaps[pot + 1]['block']['evidence'][0])
        self.assertEqual(snaps[pot + d]['block']['settlement']['invalid'], True)
        # A fallback block: no proposer, no tip share; at its settlement the winner takes the proposer part.
        fb = fx['fallbackBlock']
        self.assertEqual((snaps[fb]['block']['tipShare']['proposer'], snaps[fb]['tipShareWei']), (None, '0'))
        settled = snaps[fb + d]['block']['settlement']
        self.assertEqual((settled['signed'], settled['winner']), (False, m['one']))
        wp, pp, r = rewards.split(int(settled['grossRewardWei']), 1000, 500)
        shares = {o['minerId']: o['shares'] for o in settled['owners']}
        paid_one = sum(int(b['amountWei']) for b in settled['beneficiaries'] if b['minerId'] == m['one'])
        self.assertEqual(paid_one, wp + pp + r * shares[m['one']] // max(settled['shareCount'], 64))
        # The proposer's own registry call in its block (one: setRewardAddress to erin; or hank: a topUp): the tip share
        # follows the record the user phase left.
        own = snaps[fx['ownRegistryCallBlock']]['block']
        proposer = own['tipShare']['proposer']
        self.assertIn(proposer, (m['one'], m['hank']))
        self.assertGreater(int(own['tipShare']['shareWei']), 0)
        if proposer == m['one']:
            self.assertEqual([b['rewardAddress'] for b in own['beneficiaries'] if b['minerId'] == m['one'] and b['amountWei'] ==
                              own['tipShare']['paidWei']], [m['erin']])
        else:
            self.assertEqual(own['tipShare']['bondWei'], own['tipShare']['shareWei'])
        # Closing: markClose refunds one, hank (ACCRUING) and the exited three before settle(closeHeight - D), which
        # pays hank's share to his reward address directly (PAID), not into a bond.
        close = fx['closeHeight']
        refunds = {b['minerId']: int(b['amountWei']) for b in snaps[close]['block']['bondRefunds']}
        self.assertEqual(set(refunds), {m['one'], m['hank'], m['three']})
        self.assertEqual((refunds[m['one']], refunds[m['three']]), (20 * 10**18, 20 * 10**18))
        self.assertTrue(0 < refunds[m['hank']] < 20 * 10**18)
        hank = [b for b in snaps[close]['block']['settlement']['beneficiaries'] if b['minerId'] == m['hank']]
        self.assertTrue(len(hank) == 1 and int(hank[0]['amountWei']) > 0 and 'bondAccruedWei' not in hank[0])
        self.assertTrue(all('bondAccruedWei' in b for b in snaps[close - 1]['block']['beneficiaries'] if b['minerId'] == m['hank']))
        self.assertEqual(snaps[close]['closing']['closeHeight'], close)
        self.assertIsNone(snaps[close - 1]['closing'])
        # The final block: no pot, the tips and both carries burn, settleFinal settles pots F-D+1..F-1.
        final = fx['finalHeight']
        self.assertEqual((final, fx['head']), (close + d, final))
        self.assertTrue(snaps[final]['final'] and not any(s['final'] for h, s in snaps.items() if h != final))
        fin = snaps[final]['block']['finalize']
        self.assertEqual([s['potHeight'] for s in fin['settlements']], list(range(final - d + 1, final)))
        self.assertGreater(int(fin['tipsBurnedWei']), 0)
        self.assertGreater(int(fin['carryBurnedWei']), 0)
        self.assertTrue(all(s['carryOutWei'] == '0' and int(s['baseBurnWei']) > 0 for s in fin['settlements']))
        self.assertTrue(all(snaps[j]['closing']['settlementMayBeFinal'] for j in range(close, final)))


if __name__ == '__main__':
    unittest.main()
