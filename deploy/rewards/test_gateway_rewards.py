import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path
import threading
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('reward_gateway_under_test', ROOT / 'gateway.py')
gateway = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gateway)

def request(method='pocol_getBlockReward', params=None, ident=1):
    return {'jsonrpc': '2.0', 'id': ident, 'method': method, 'params': [] if params is None else params}

class GatewayRewardTests(unittest.TestCase):
    def test_only_parameterless_summary_is_public(self):
        self.assertTrue(gateway.valid_call(request()))
        for params in ([1], ['latest'], [{}], [None], ['0x100'],
                       [[[1, 0], [2, 100]]], [{'forkSchedule': [[1, 0], [2, 100]]}]):
            self.assertFalse(gateway.valid_call(request(params=params)))
        for method, params in (
            ('eth_call', [{'to': '0x' + '0' * 40, 'data': '0x'}, 'latest']),
            ('eth_getStorageAt', ['0x' + '0' * 40, '0x0', 'latest']),
            ('pocol_getBlockRlp', ['latest']),
            ('eth_sendRawTransaction', ['0x00']),
            ('admin_peers', []),
        ):
            self.assertFalse(gateway.valid_call(request(method, params)))
        self.assertTrue(gateway.valid_call(request('eth_getBlockByNumber', ['latest', False])))
        self.assertTrue(gateway.valid_call(request('pocol_getNetworkGeography')))

    def test_schedule_file_is_numeric_validated_and_defaults_to_legacy(self):
        self.assertEqual(gateway.load_fork_schedule(), ((1, 0),))
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            filename = Path(directory) / 'forks.json'
            for value in ([[1, 0]], [[1, 0], [2, 100]]):
                filename.write_text(json.dumps(value))
                self.assertEqual(gateway.load_fork_schedule(filename), tuple(map(tuple, value)))
            invalid = ([], [[2, 0]], [[1, 1]], [[1, 0], [2, 0]],
                       [[1, 0], [3, 100]], [[1, 0], [2, 1 << 64]],
                       [[1, 0], [2, -1]], [[1, 0], [2, True]],
                       [[1, 0], [2, 1.0]], [[1, 0], [2, '100']],
                       [[1, 0], [2, 100], [3, 200]], {'forkSchedule': [[1, 0]]})
            for value in invalid:
                filename.write_text(json.dumps(value))
                with self.subTest(value=value), self.assertRaises((ValueError, RuntimeError)):
                    gateway.load_fork_schedule(filename)
            for raw in ('[', ' ' * 4097):
                filename.write_text(raw)
                with self.assertRaises((ValueError, RuntimeError)):
                    gateway.load_fork_schedule(filename)

    def test_schedule_is_local_frozen_and_forwarded_as_keyword(self):
        for configured in (None, [[1, 0], [2, 100]]):
            expected = ((1, 0),) if configured is None else ((1, 0), (2, 100))
            service = gateway.Gateway() if configured is None else gateway.Gateway(fork_schedule=configured)
            if configured is not None:
                configured[1][1] = 1
            reads = []
            def fake(query, *, fork_schedule):
                reads.append(fork_schedule)
                return {'blockHeight': 100}
            # Extra JSON-RPC fields cannot replace the operator's startup schedule.
            supplied = request()
            supplied['forkSchedule'] = [[1, 0], [2, 1]]
            with patch.object(gateway, 'reward_snapshot', fake):
                self.assertIn('result', service.call(supplied))
            self.assertEqual(reads, [expected])
            self.assertEqual(service.fork_schedule, expected)

    def test_file_is_not_reloaded_by_a_reward_request(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            filename = Path(directory) / 'forks.json'
            filename.write_text('[[1,0],[2,100]]')
            service = gateway.Gateway(fork_schedule=gateway.load_fork_schedule(filename))
            filename.write_text('[[1,0],[2,1]]')
            with patch.object(gateway, 'reward_snapshot', return_value={}) as snapshot:
                service.call(request())
            self.assertEqual(snapshot.call_args.kwargs['fork_schedule'], ((1, 0), (2, 100)))

    def test_cli_rejects_invalid_schedule_before_connecting_to_node(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            filename = Path(directory) / 'forks.json'
            filename.write_text('[[1,0],[2,0]]')
            result = subprocess.run([sys.executable, str(ROOT / 'gateway.py'),
                                     '--fork-schedule', str(filename)], capture_output=True,
                                    text=True, timeout=5)
            self.assertEqual(result.returncode, 2)
            self.assertIn('Unsupported trusted fork schedule', result.stderr)
            self.assertNotIn('ConnectionRefusedError', result.stderr)

    def test_params_expose_only_new_public_fork_fields(self):
        added = {'forkSchedule': [[1, 0], [2, 100]], 'effectiveNonceMode': 0,
                 'nextBlockNonceMode': 1, 'consensusCompatibilitySha256': '0x' + 'a' * 64}
        value = {'chainId': gateway.CHAIN_ID, **added,
                 'internalRpc': 'private', 'forkScheduleFile': '/private/forks.json'}
        result = gateway.sanitize('pocol_getParams', value)
        for key, expected in added.items():
            self.assertEqual(result[key], expected)
        self.assertNotIn('internalRpc', result)
        self.assertNotIn('forkScheduleFile', result)

    def test_public_network_designation_does_not_expose_private_metadata(self):
        labels = {'networkName': 'PoCol Mainnet', 'networkEnvironment': 'mainnet',
                  'forkSchedule': [[1, 0], [2, 100]], 'effectiveNonceMode': 0,
                  'nextBlockNonceMode': 1, 'consensusCompatibilitySha256': '0x' + 'a' * 64}
        for method in ('pocol_getParams', 'pocol_status'):
            result = gateway.sanitize(method, {**labels, 'genesisPre': 'private',
                                               'localPaths': ['/private'], 'environment': {'secret': 'private'}})
            self.assertEqual({name: result[name] for name in labels}, labels)
            self.assertNotIn('genesisPre', result)
            self.assertNotIn('localPaths', result)
            self.assertNotIn('environment', result)

    def test_the_consensus_identity_and_wire_range_pass_the_sanitizer(self):
        fields = {'consensusIdentity': 'c' * 64, 'consensusRulesVersion': 2, 'protocolVersionMin': 2, 'maxForkVersion': 2}
        for method in ('pocol_getParams', 'pocol_status'):
            with self.subTest(method=method):
                result = gateway.sanitize(method, {**fields, 'consensusSourcesSha256': 'private', 'peers': ['private']})
                self.assertEqual({name: result[name] for name in fields}, fields)
                self.assertNotIn('consensusSourcesSha256', result)

    def test_snapshot_is_cached_with_request_ids_preserved(self):
        service = gateway.Gateway()
        reads = []
        def fake(query, *, fork_schedule):
            reads.append(True)
            return {'blockHeight': 10, 'grossRewardWei': '2000000000000000000'}
        with patch.object(gateway, 'reward_snapshot', fake):
            first = service.call(request(ident=2))
            second = service.call(request(ident=3))
        self.assertEqual(first['result'], second['result'])
        self.assertEqual([first['id'], second['id']], [2, 3])
        self.assertEqual(len(reads), 1)

    def test_one_snapshot_at_a_time_does_not_block_normal_reads(self):
        entered, release = threading.Event(), threading.Event()
        service = gateway.Gateway()
        service.query = lambda method, params: {'jsonrpc': '2.0', 'id': 1, 'result': '0x10'}
        def fake(query, *, fork_schedule):
            entered.set()
            release.wait(2)
            return {'blockHeight': 16}
        with patch.object(gateway, 'reward_snapshot', fake):
            worker = threading.Thread(target=service.call, args=(request(),))
            worker.start()
            self.assertTrue(entered.wait(1))
            busy = service.call(request(ident=2))
            normal = service.call(request('eth_blockNumber', ident=3))
            release.set()
            worker.join(2)
        self.assertEqual(busy['error']['code'], -32021)
        self.assertEqual(normal['result'], '0x10')
        self.assertFalse(worker.is_alive())

    def test_failures_are_throttled_and_do_not_leak_details(self):
        service = gateway.Gateway()
        calls = []
        def fail(query, *, fork_schedule):
            calls.append(True)
            raise RuntimeError('internal endpoint details')
        with patch.object(gateway, 'reward_snapshot', fail):
            first = service.call(request())
            second = service.call(request())
        self.assertEqual(len(calls), 1)
        self.assertNotIn('internal', str(first))
        self.assertEqual(first['error']['code'], -32000)
        self.assertEqual(second['error']['code'], -32000)

    def test_an_unexpected_snapshot_exception_is_a_structured_error(self):
        """A malformed node answer of an unexpected type (AttributeError, TypeError, ...) must not escape Gateway.call:
        the client gets the generic -32000 error and the failure is throttled like any other."""
        for kind in (AttributeError, TypeError, IndexError, ZeroDivisionError):
            service = gateway.Gateway()
            calls = []
            def fail(query, *, fork_schedule):
                calls.append(True)
                raise kind("'int' object has no attribute 'lower'")
            with self.subTest(kind=kind.__name__), patch.object(gateway, 'reward_snapshot', fail):
                first = service.call(request())
                second = service.call(request())
                self.assertEqual((first['error']['code'], second['error']['code']), (-32000, -32000))
                self.assertNotIn('lower', str(first))
                self.assertEqual(len(calls), 1)

    def test_subcall_budget_and_read_only_filter(self):
        for prohibited in (True, False):
            service = gateway.Gateway()
            reads = []
            service.query = lambda method, params: reads.append(method) or {'result': '0x0'}
            def excessive(query, *, fork_schedule):
                for _ in range(513):
                    query('eth_sendRawTransaction' if prohibited else 'eth_call', [])
                return {}
            with patch.object(gateway, 'reward_snapshot', excessive):
                answer = service.call(request())
            self.assertIn('error', answer)
            self.assertEqual(len(reads), 0 if prohibited else 512)

class GatewayV3NetworkTests(unittest.TestCase):
    """V3-PR-G: a v3 (spec-2) network only from the operator's startup file; the public method stays parameterless."""
    NETWORK = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32, 'specVersion': 2, 'v3': True}

    def test_network_file_is_validated_and_absent_by_default(self):
        self.assertIsNone(gateway.load_v3_network())
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            filename = Path(directory) / 'network.json'
            filename.write_text(json.dumps(self.NETWORK))
            self.assertEqual(gateway.load_v3_network(filename).genesis, '0x' + '5a' * 32)
            for bad in (dict(self.NETWORK, specVersion=1), dict(self.NETWORK, v3=False), dict(self.NETWORK, chainId='1'),
                        dict(self.NETWORK, genesisHash=gateway.GENESIS), [[1, 0]]):
                filename.write_text(json.dumps(bad))
                with self.subTest(bad=bad), self.assertRaises((ValueError, RuntimeError)):
                    gateway.load_v3_network(filename)
            filename.write_text(' ' * 4097)
            with self.assertRaisesRegex(ValueError, '4096'):
                gateway.load_v3_network(filename)

    def test_v3_network_is_forwarded_with_its_read_budget_and_receipts(self):
        service = gateway.Gateway(fork_schedule=[[1, 0], [2, 2]], network=self.NETWORK)
        self.assertEqual(service.chain_id, 777963)
        seen = {}
        def fake(query, *, fork_schedule, network):
            seen.update(schedule=fork_schedule, network=network)
            with patch.object(service, 'query', return_value={'jsonrpc': '2.0', 'id': 1, 'result': {}}):
                query('eth_getTransactionReceipt', ['0x' + '00' * 32])
                with self.assertRaisesRegex(RuntimeError, 'Unsupported reward read'):
                    query('eth_sendRawTransaction', ['0x00'])
            return {'blockHeight': 3}
        supplied = request()
        supplied['network'] = {'chainId': 1}
        with patch.object(gateway, 'reward_snapshot', fake):
            self.assertIn('result', service.call(supplied))
        self.assertEqual((seen['schedule'], seen['network'].chain_id), (((1, 0), (2, 2)), 777963))

    def test_mainnet_gateway_keeps_its_receipt_free_reads(self):
        service = gateway.Gateway()
        self.assertIsNone(service.network)
        def fake(query, *, fork_schedule):
            with self.assertRaisesRegex(RuntimeError, 'Unsupported reward read'):
                query('eth_getTransactionReceipt', ['0x' + '00' * 32])
            return {}
        with patch.object(gateway, 'reward_snapshot', fake):
            self.assertIn('result', service.call(request()))

    def test_verify_network_uses_the_configured_identity(self):
        service = gateway.Gateway(network=self.NETWORK)
        answers = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32}
        with patch.object(service, 'query', return_value={'result': answers}):
            service.verify_network()
        with patch.object(service, 'query', return_value={'result': {'chainId': gateway.CHAIN_ID, 'genesisHash': gateway.GENESIS}}):
            with self.assertRaises(RuntimeError):
                service.verify_network()

    def test_cli_rejects_a_spec_1_network_file_before_connecting(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            filename = Path(directory) / 'network.json'
            filename.write_text(json.dumps(dict(self.NETWORK, specVersion=1)))
            result = subprocess.run([sys.executable, str(ROOT / 'gateway.py'), '--v3-network', str(filename)],
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 2)
            self.assertIn('spec-2', result.stderr)
            self.assertNotIn('ConnectionRefusedError', result.stderr)

def w0_word(status=1, tomb=0, bond_state=1, rec_pos=0, act_pos=0, last_share=0, registered_at=0):
    return status | tomb << 8 | bond_state << 16 | rec_pos << 24 | act_pos << 56 | last_share << 88 | registered_at << 152

def abi_two_arrays(ids, words):
    n = len(ids); word = lambda v: v.to_bytes(32, 'big')
    return '0x' + (word(64) + word(64 + 32 * (n + 1)) + word(n) + b''.join(map(word, ids)) + word(n) + b''.join(map(word, words))).hex()

def selector(signature):
    return '0x' + gateway.keccak256(signature.encode())[:4].hex()

B_REG = 20 * 10**18
LOW160 = (1 << 160) - 1
NET = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32, 'specVersion': 2, 'v3': True}

class Registry:
    """A mock v3 node: blocks, MinerRegistry lengths/activeSlice/recordWords/recordClocks/removalStatus and Caps.get(16).
    Records hold the stored words; recordWords masks w1 to the reward address and recordClocks returns its clocks, as
    PoColSystemV3.sol does."""
    def __init__(self, members, records=None, removal=None, height=40):
        self.members = members                  # [(id int, w0)]
        self.records = records or {}            # address -> (w0, w1, w2, w3)
        self.removal = removal or {}
        self.height, self.hashes = height, {}
        self.calls = []
    def head_hash(self, h):
        return self.hashes.get(h, '0x' + format(h, '064x'))
    def query(self, method, params):
        self.calls.append((method, params))
        ok = lambda v: {'jsonrpc': '2.0', 'id': 1, 'result': v}
        if method == 'eth_getBlockByNumber':
            h = self.height if params[0] == 'latest' else int(params[0], 16)
            return ok({'number': hex(h), 'hash': self.head_hash(h)})
        if method == 'eth_call':
            to, data = params[0]['to'], params[0]['data']
            assert params[1] == hex(self.height), 'every read at the head height'
            sel, args = data[:10], data[10:]
            if to == gateway.CAPS and sel == selector('get(uint256)'):
                assert int(args, 16) == 16
                return ok('0x' + B_REG.to_bytes(32, 'big').hex())
            assert to == gateway.REGISTRY
            if sel == selector('lengths()'):
                return ok('0x' + b''.join(v.to_bytes(32, 'big') for v in (len(self.members) + 3, len(self.members), 0, 0)).hex())
            if sel == selector('activeSlice(uint256,uint256)'):
                start, count = int(args[:64], 16), int(args[64:], 16)
                page = self.members[start:start + count]
                return ok(abi_two_arrays([i for i, _ in page], [w for _, w in page]))
            member = '0x' + args[-40:]
            w0, w1, w2, w3 = self.records.get(member, (0, 0, 0, 0))
            if sel == selector('recordWords(address)'):
                return ok('0x' + b''.join(v.to_bytes(32, 'big') for v in (w0, w1 & LOW160, w2, w3)).hex())
            if sel == selector('recordClocks(address)'):
                return ok('0x' + b''.join(v.to_bytes(32, 'big') for v in (w1 >> 224, (w1 >> 160) & (1 << 64) - 1)).hex())
            if sel == selector('removalStatus(address)'):
                return ok('0x' + self.removal.get(member, 1).to_bytes(32, 'big').hex())
        raise AssertionError(method)
    def reads(self, method):
        return [c for c in self.calls if c[0] == method]

def v3_service(node):
    service = gateway.Gateway(network=NET)
    service.query = node.query
    return service

class GatewayMembershipTests(unittest.TestCase):
    A = '0x' + '11' * 20

    def test_membership_methods_and_their_bounds(self):
        for params in ([0, 1], [0, 256], [9999, 256]):
            self.assertTrue(gateway.valid_call(request('pocol_getMembers', params)))
        for params in ([0, 257], [0, 0], [-1, 1], [10000, 1], [True, 1], [0, True], [0, 1.0], ['0x0', 1], [0], [0, 1, 2], [None, 1]):
            with self.subTest(params=params): self.assertFalse(gateway.valid_call(request('pocol_getMembers', params)))
        self.assertTrue(gateway.valid_call(request('pocol_getMember', [self.A])))
        for params in ([], [self.A, 'latest'], ['0x11'], [self.A + '00'], [1]):
            with self.subTest(params=params): self.assertFalse(gateway.valid_call(request('pocol_getMember', params)))
        for method in ('pocol_getRemovable', 'pocol_finality'):
            self.assertTrue(gateway.valid_call(request(method)))
            self.assertFalse(gateway.valid_call(request(method, ['latest'])))
        self.assertEqual(gateway.COSTS['pocol_getMember'], gateway.COSTS['pocol_getBlockReward'])
        self.assertFalse(gateway.valid_call(request('eth_call', [{'to': gateway.REGISTRY, 'data': selector('lengths()')}, 'latest'])))

    def test_every_bond_state_and_status_is_labelled(self):
        expected = {0: 'NONE', 1: 'HELD', 2: 'BURNED', 3: 'PAID', 4: 'ACCRUING'}
        for state, name in expected.items():
            self.assertEqual(gateway.member_w0(w0_word(bond_state=state))['bondStateLabel'], name)
        self.assertEqual([gateway.member_w0(w0_word(status=s))['statusLabel'] for s in (0, 1, 2)], ['NONE', 'ACTIVE', 'EXITED'])
        for word in (w0_word(bond_state=5), w0_word(status=3)):
            with self.assertRaises(RuntimeError): gateway.member_w0(word)

    def test_members_page_carries_w0_fields_only_at_one_height(self):
        members = [(0x11, w0_word(bond_state=1, last_share=39, registered_at=2, rec_pos=0, act_pos=0)),
                   (0x22, w0_word(bond_state=4, last_share=30, registered_at=7, rec_pos=1, act_pos=1)),
                   (0x33, w0_word(tomb=1, bond_state=2, last_share=5, registered_at=3, rec_pos=2, act_pos=2))]
        node = Registry(members); service = v3_service(node)
        answer = service.call(request('pocol_getMembers', [0, 256], ident=7))
        self.assertEqual(answer['id'], 7)
        result = answer['result']
        self.assertEqual({k: result[k] for k in ('height', 'blockHash', 'active', 'start', 'count')},
                         {'height': 40, 'blockHash': node.head_hash(40), 'active': 3, 'start': 0, 'count': 3})
        self.assertEqual(result['members'][1], {'id': '0x' + format(0x22, '040x'), 'status': 1, 'statusLabel': 'ACTIVE', 'tomb': False,
                                                'bondState': 4, 'bondStateLabel': 'ACCRUING', 'lastShare': 30, 'registeredAt': 7})
        self.assertEqual([m['bondStateLabel'] for m in result['members']], ['HELD', 'ACCRUING', 'BURNED'])
        self.assertTrue(result['members'][2]['tomb'])
        for record in result['members']:
            self.assertFalse({'bond', 'bondWei', 'rewardAddress', 'exitHeight'} & set(record))
        self.assertEqual(len(node.reads('eth_call')), 2)

    def test_paging_reads_only_the_requested_slice(self):
        members = [(i + 1, w0_word(act_pos=i, rec_pos=i)) for i in range(300)]
        node = Registry(members); service = v3_service(node)
        first = service.call(request('pocol_getMembers', [0, 256]))['result']
        self.assertEqual(first['count'], 256)
        self.assertLess(len(json.dumps(first)), 64 * 1024, 'a full page stays far below the 4 MiB limits')
        second = service.call(request('pocol_getMembers', [256, 256]))['result']
        self.assertEqual((second['count'], second['members'][0]['id']), (44, '0x' + format(257, '040x')))
        node.calls.clear()
        empty = service.call(request('pocol_getMembers', [300, 10]))['result']
        self.assertEqual((empty['count'], empty['members']), (0, []))
        self.assertEqual([c[1][0]['data'][:10] for c in node.reads('eth_call')], [selector('lengths()')], 'no slice past the end')

    def test_members_are_cached_per_head(self):
        node = Registry([(0x11, w0_word())]); service = v3_service(node)
        first = service.call(request('pocol_getMembers', [0, 1]))
        node.calls.clear()
        self.assertEqual(service.call(request('pocol_getMembers', [0, 1], ident=2))['result'], first['result'])
        self.assertEqual(node.calls, [('eth_getBlockByNumber', ['latest', False])], 'one head read on a hit')
        node.height = 41; node.members = [(0x11, w0_word(last_share=41))]
        self.assertEqual(service.call(request('pocol_getMembers', [0, 1]))['result']['members'][0]['lastShare'], 41)

    def test_a_block_replaced_while_reading_is_refused(self):
        node = Registry([(0x11, w0_word())]); service = v3_service(node)
        query = node.query
        def moving(method, params):
            answer = query(method, params)
            if method == 'eth_call': node.hashes[40] = '0x' + 'ee' * 32
            return answer
        service.query = moving
        answer = service.call(request('pocol_getMembers', [0, 1]))
        self.assertEqual(answer['error']['code'], -32022)
        self.assertEqual(service.member_cache, {})

    def test_malformed_active_slices_are_refused(self):
        for members in ([(0, w0_word())], [(0x11, w0_word(status=2))], [(0x11, w0_word(bond_state=9))]):
            node = Registry(members); service = v3_service(node)
            with self.subTest(members=members):
                self.assertEqual(service.call(request('pocol_getMembers', [0, 1]))['error']['code'], -32000)
        node = Registry([(0x11, w0_word())]); service = v3_service(node)
        query = node.query
        service.query = lambda m, p: ({'jsonrpc': '2.0', 'id': 1, 'result': '0x' + '00' * 64}
                                      if m == 'eth_call' and p[0]['data'].startswith(selector('activeSlice(uint256,uint256)')) else query(m, p))
        self.assertEqual(service.call(request('pocol_getMembers', [0, 1]))['error']['code'], -32000)

    def member(self, w0, w1=0, w2=0, w3=0, removal=1):
        node = Registry([], {self.A: (w0, w1, w2, w3)}, {self.A: removal}); service = v3_service(node)
        answer = service.call(request('pocol_getMember', [self.A.upper().replace('0X', '0x')]))
        return answer.get('result', answer), node

    def test_member_decodes_all_four_words_and_the_removal_verdict(self):
        reward = 0x99 * (1 << 152) + 0x1234
        w1 = reward | 17 << 160 | 0xfffffff0 << 224
        w3 = 30 | 30 + 720 << 64 | 0 << 128 | 12 << 192
        record, node = self.member(w0_word(status=1, bond_state=1, rec_pos=4, act_pos=3, last_share=39, registered_at=2),
                                   w1, B_REG, w3, removal=4)
        self.assertEqual({k: record[k] for k in ('address', 'exists', 'status', 'statusLabel', 'bondStateLabel', 'recPos', 'actPos',
                                                 'lastShare', 'registeredAt', 'rewardAddress', 'lastReactHeight', 'lastMaintHeightMod32',
                                                 'bondWei', 'bondTargetWei', 'exitHeight', 'dueHeight', 'burnHeight', 'graceStart',
                                                 'removal', 'removalLabel', 'proposerEligible', 'height')},
                         {'address': self.A, 'exists': True, 'status': 1, 'statusLabel': 'ACTIVE', 'bondStateLabel': 'HELD', 'recPos': 4,
                          'actPos': 3, 'lastShare': 39, 'registeredAt': 2, 'rewardAddress': '0x' + reward.to_bytes(20, 'big').hex(),
                          'lastReactHeight': 17, 'lastMaintHeightMod32': 0xfffffff0, 'bondWei': str(B_REG), 'bondTargetWei': str(B_REG),
                          'exitHeight': 30, 'dueHeight': 750, 'burnHeight': 0, 'graceStart': 12, 'removal': 4,
                          'removalLabel': 'RECENTLY_ACTIVE', 'proposerEligible': True, 'height': 40})
        self.assertEqual([c[1][0]['to'] for c in node.reads('eth_call')], [gateway.REGISTRY] * 3 + [gateway.CAPS])
        self.assertEqual([c[1][0]['data'][:10] for c in node.reads('eth_call')][:3],
                         [selector('recordWords(address)'), selector('recordClocks(address)'), selector('removalStatus(address)')])
        self.assertEqual(len(node.reads('eth_getBlockByNumber')), 2, 'the head and its re-check')
        self.assertLessEqual(len(node.calls), gateway.MEMBER_READS_MAX)

    def test_member_clocks_come_from_record_clocks_not_the_masked_reward_word(self):
        reward = 0x42 * (1 << 152) + 7
        record, node = self.member(w0_word(), reward | 123456789 << 160 | 99 << 224)
        self.assertEqual((record['rewardAddress'], record['lastReactHeight'], record['lastMaintHeightMod32']),
                         ('0x' + reward.to_bytes(20, 'big').hex(), 123456789, 99))
        self.assertEqual(self.member(w0_word(), reward)[0]['lastReactHeight'], 0, 'a record never reactivated')
        # A node answering recordWords with an unmasked w1, or recordClocks out of range, is not the v3 registry.
        node = Registry([], {self.A: (w0_word(), reward, 0, 0)}, {self.A: 1}); service = v3_service(node)
        query = node.query
        def wide(field):
            def answer(m, p):
                if m == 'eth_call' and p[0]['data'].startswith(selector('recordWords(address)')) and field == 'w1':
                    return {'jsonrpc': '2.0', 'id': 1, 'result': '0x' + b''.join(v.to_bytes(32, 'big') for v in (w0_word(), reward | 1 << 160, 0, 0)).hex()}
                if m == 'eth_call' and p[0]['data'].startswith(selector('recordClocks(address)')) and field != 'w1':
                    values = (1 << 32, 0) if field == 'maint' else (0, 1 << 64)
                    return {'jsonrpc': '2.0', 'id': 1, 'result': '0x' + b''.join(v.to_bytes(32, 'big') for v in values).hex()}
                return query(m, p)
            return answer
        for field in ('w1', 'maint', 'react'):
            service.query = wide(field); service.member_cache.clear()
            with self.subTest(field=field):
                self.assertEqual(service.call(request('pocol_getMember', [self.A]))['error']['code'], -32000)

    def test_accruing_proposer_eligibility_follows_the_quarter_bond_rule(self):
        for bond, eligible in ((B_REG // 4, True), (B_REG // 4 - 1, False), (B_REG - 1, True), (0, False)):
            record, _ = self.member(w0_word(bond_state=4), w2=bond)
            with self.subTest(bond=bond):
                self.assertEqual((record['bondStateLabel'], record['proposerEligible']), ('ACCRUING', eligible))
        record, _ = self.member(w0_word(bond_state=4, tomb=1), w2=B_REG)
        self.assertFalse(record['proposerEligible'], 'a tomb is never drawn')

    def test_exited_victims_burned_paid_and_absent_records(self):
        record, _ = self.member(w0_word(status=2, bond_state=1), w2=B_REG, w3=50 | 770 << 64)
        self.assertEqual((record['statusLabel'], record['exitHeight'], record['dueHeight'], record['proposerEligible'], record['removalLabel']),
                         ('EXITED', 50, 770, False, 'NOT_LIVE_ACTIVE'))
        self.assertEqual(self.member(w0_word(status=2, tomb=1, bond_state=2), w3=9 << 128)[0]['bondStateLabel'], 'BURNED')
        self.assertEqual(self.member(w0_word(status=2, bond_state=3))[0]['bondStateLabel'], 'PAID')
        record, _ = self.member(0)
        self.assertEqual((record['exists'], record['statusLabel'], record['bondStateLabel'], record['bondWei']), (False, 'NONE', 'NONE', '0'))
        for code, name in gateway.REMOVAL_LABELS.items():
            self.assertEqual(self.member(w0_word(), removal=code)[0]['removalLabel'], name)
        self.assertEqual(self.member(w0_word(), removal=5)[0]['error']['code'], -32000)

    def test_membership_needs_a_configured_v3_network(self):
        service = gateway.Gateway()
        service.query = lambda m, p: (_ for _ in ()).throw(AssertionError('no node read'))
        for r in (request('pocol_getMembers', [0, 1]), request('pocol_getMember', [self.A])):
            self.assertEqual(service.call(r)['error']['code'], -32000)

    def test_removable_and_finality_are_forwarded(self):
        seen = []
        service = gateway.Gateway(network=NET)
        service.query = lambda m, p: seen.append((m, p)) or {'jsonrpc': '2.0', 'id': 1, 'result': {'method': m}}
        for method in ('pocol_getRemovable', 'pocol_finality'):
            self.assertEqual(service.call(request(method))['result'], {'method': method})
        self.assertEqual(seen, [('pocol_getRemovable', []), ('pocol_finality', [])])

    def test_requests_are_charged_by_method_cost(self):
        service = gateway.Gateway(network=NET); costs = []
        service.charge = lambda cost: costs.append(cost) or False
        server = gateway.Server(('127.0.0.1', 0), service)
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        try:
            import http.client
            body = json.dumps([request('pocol_getMember', [self.A], 1), request('pocol_getMembers', [0, 256], 2),
                               request('pocol_getRemovable', ident=3), request('pocol_finality', ident=4)])
            conn = http.client.HTTPConnection('127.0.0.1', server.server_port, timeout=3)
            conn.request('POST', '/rpc', body, {'Content-Type': 'application/json'})
            self.assertEqual(conn.getresponse().status, 429); conn.close()
        finally:
            server.shutdown(); server.server_close(); thread.join()
        self.assertEqual(costs, [20 + 5 + 10 + 1])


if __name__ == '__main__':
    unittest.main()
