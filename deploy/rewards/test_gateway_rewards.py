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
        fields = {'consensusIdentity': 'c' * 64, 'consensusRulesVersion': 1, 'protocolVersionMin': 2, 'maxForkVersion': 2}
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


if __name__ == '__main__':
    unittest.main()
