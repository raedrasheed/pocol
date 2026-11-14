import hashlib
import http.client
import json
from pathlib import Path
import subprocess
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch, MagicMock
import pocol_studio_gateway as g
import install_studio_gateway as installer
from install_studio_gateway import patch_caddy, ROUTE


def enc(value):
    if isinstance(value, list):
        raw = b''.join(map(enc, value)); offset = 0xc0
    else:
        raw = value; offset = 0x80
        if len(raw) == 1 and raw[0] < 128: return raw
    if len(raw) <= 55: return bytes([offset+len(raw)])+raw
    size = len(raw).to_bytes((len(raw).bit_length()+7)//8, 'big')
    return bytes([offset+55+len(size)])+size+raw


def minimal(value):
    return value.to_bytes((value.bit_length()+7)//8, 'big')


def tx(chain=g.CHAIN_ID, data=b'', chain_bytes=None):
    # The extension signs with minimal RLP integers (src/lib/wallet.js), so the chain ID has no leading zero bytes.
    field = minimal(chain) if chain_bytes is None else chain_bytes
    return '0x'+(b'\x02'+enc([field, b'', b'\x01', b'\x02', b'\x52\x08', b'a'*20, b'', data, [], b'', b'\x01', b'\x02'])).hex()


def request(method='eth_chainId', params=None, ident=1):
    return {'jsonrpc': '2.0', 'id': ident, 'method': method, 'params': params or []}


class ValidationTests(unittest.TestCase):
    def test_chunk_transaction_and_wrong_network(self):
        self.assertTrue(g.valid_raw_transaction(tx(data=b'x'*24643)))
        self.assertFalse(g.valid_raw_transaction(tx(chain=1)))
        self.assertFalse(g.valid_raw_transaction(tx()+'00'))
        self.assertFalse(g.valid_raw_transaction('0x01'+tx()[4:]))
        self.assertFalse(g.valid_raw_transaction(tx(data=b'x'*60000)))

    def test_privileged_calls_and_arbitrary_upstream_rejected(self):
        for method in ['eth_sendTransaction', 'personal_unlockAccount', 'admin_peers', 'miner_start', 'debug_traceTransaction']:
            self.assertFalse(g.valid_call(request(method)))
        self.assertFalse(g.valid_call({**request(), 'url': 'http://attacker'}))
        self.assertFalse(g.valid_call(request(ident=True)))
        self.assertFalse(g.valid_call(request('eth_getBlockByNumber', ['latest', True])))

    def test_batch_write_and_duplicate_ids_refused(self):
        self.assertFalse(g.valid_payload([request('eth_sendRawTransaction', [tx()])]))
        self.assertFalse(g.valid_payload([request(), request()]))
        self.assertTrue(g.valid_payload([request(), request('pocol_status', ident=2)]))

    def test_call_budget_and_pending_nonce(self):
        self.assertTrue(g.valid_call(request('eth_getTransactionCount', ['0x'+'1'*40, 'pending'])))
        self.assertTrue(g.valid_call(request('eth_call', [{'to': '0x'+'1'*40, 'data': '0x1234'}, 'latest'])))
        self.assertFalse(g.valid_call(request('eth_call', [{'to': '0x'+'1'*40, 'gas': '0xffffff'}, 'latest'])))
        self.assertFalse(g.valid_call(request('eth_call', [{'to': '0x'+'1'*40, 'accessList': []}, 'latest'])))

    def test_private_status_redacted(self):
        self.assertEqual(g.sanitize('pocol_status', {'chainId': g.CHAIN_ID, 'peers': ['private'], 'rpcAddr': 'secret'}), {'chainId': g.CHAIN_ID})

    def test_public_fork_and_network_metadata_survives_without_private_fields(self):
        fields = {'forkSchedule': [[1, 0], [2, 100]], 'effectiveNonceMode': 0,
                  'nextBlockNonceMode': 1, 'consensusCompatibilitySha256': '0x' + 'a' * 64,
                  'networkName': 'PoCol Mainnet', 'networkEnvironment': 'mainnet'}
        private = {'genesisPre': 'private', 'rpcLimits': {'private': True},
                   'peers': ['private'], 'forkScheduleFile': '/private/forks.json',
                   'environment': {'secret': 'private'}, 'rpcAddr': 'private'}
        for method in ('pocol_status', 'pocol_getParams'):
            with self.subTest(method=method):
                self.assertEqual(g.sanitize(method, {**fields, **private}), fields)

    def test_the_consensus_identity_and_wire_range_pass_the_sanitizer(self):
        fields = {'consensusIdentity': 'c' * 64, 'consensusRulesVersion': 2, 'protocolVersionMin': 2, 'maxForkVersion': 2}
        for method in ('pocol_status', 'pocol_getParams'):
            with self.subTest(method=method):
                self.assertEqual(g.sanitize(method, {**fields, 'peers': ['private'], 'lastIncompatibleReason': 'peer x'}), fields)

    def test_metadata_fields_do_not_expand_methods_or_accept_remote_activation(self):
        # FN-1 (C9) adds pocol_getCaps, read-only and parameterless; Sites W1 eth_maxPriorityFeePerGas and the two
        # locally answered methods; nothing else.
        self.assertEqual(g.NO_PARAMS, {'eth_chainId', 'pocol_status', 'pocol_getParams', 'eth_blockNumber', 'eth_gasPrice', 'pocol_getCaps',
                                       'eth_maxPriorityFeePerGas'})
        self.assertEqual(g.LOCAL, {'net_version', 'web3_clientVersion'})
        for method in ('pocol_setForkSchedule', 'pocol_setNetworkEnvironment', 'admin_setFork', 'miner_start'):
            self.assertFalse(g.valid_call(request(method, [[[1, 0], [2, 100]]])))
        for method in ('pocol_getParams', 'pocol_status'):
            self.assertTrue(g.valid_call(request(method)))
            self.assertFalse(g.valid_call(request(method, [{'forkSchedule': [[1, 0], [2, 100]]}])))


    def test_caps_are_relayed_without_params_and_only_their_documented_fields(self):
        self.assertTrue(g.valid_call(request('pocol_getCaps')))
        self.assertFalse(g.valid_call(request('pocol_getCaps', ['latest'])))
        self.assertTrue(g.valid_payload([request('pocol_getCaps'), request('eth_chainId', ident=2)]))
        caps = {'number': '0x2a', 'hash': '0x' + 'ab' * 32, 'v3': True, 'cumulative': False, 'alarmPercent': 80, 'alarm': False,
                'occupancy': {'accounts': {'percent': 1.5, 'remaining': '0x10', 'alarm': False}},
                'accounts': '0x1', 'accountsCap': '0x11', 'storageSlots': '0x2', 'storageSlotsCap': '0x22', 'codeBytes': '0x3', 'codeBytesCap': '0x33',
                'txCount': '0x0', 'txCap': '0x44', 'txBytes': '0x0', 'txBytesCap': '0x55', 'logBytes': '0x0', 'logCap': '0x66'}
        self.assertEqual(g.sanitize('pocol_getCaps', {**caps, 'peers': ['private'], 'rpcAddr': 'secret'}), caps)


class GatewayTests(unittest.TestCase):
    def test_identity_failure_prevents_broadcast(self):
        gateway = g.Gateway(); calls = []
        def query(method, params):
            calls.append(method)
            return {'result': {'chainId': g.CHAIN_ID, 'genesisHash': 'wrong'}} if method == 'pocol_status' else {'result': hex(g.CHAIN_ID)}
        gateway.query = query
        answer = gateway.call(request('eth_sendRawTransaction', [tx()]))
        self.assertIn('error', answer); self.assertNotIn('eth_sendRawTransaction', calls)

    def test_write_timeout_is_never_retried(self):
        gateway = g.Gateway(); calls = []
        def query(method, params):
            calls.append(method)
            if method == 'pocol_status': return {'result': {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}}
            if method == 'eth_chainId': return {'result': hex(g.CHAIN_ID)}
            raise TimeoutError()
        gateway.query = query
        self.assertIn('error', gateway.call(request('eth_sendRawTransaction', [tx()])))
        self.assertEqual(calls.count('eth_sendRawTransaction'), 1)

    def test_write_network_rechecked_even_after_read(self):
        gateway = g.Gateway(); checks = []
        gateway.verify_network = lambda **kw: checks.append(kw)
        gateway.query = lambda method, params: {'jsonrpc': '2.0', 'id': 1, 'result': '0x1'}
        gateway.call(request()); gateway.call(request('eth_sendRawTransaction', [tx()]))
        self.assertEqual(checks, [{'force': False}, {'force': True}])

    def test_read_and_write_rate_limits_are_bounded(self):
        gateway = g.Gateway()
        with patch('pocol_studio_gateway.time.monotonic', return_value=100):
            for _ in range(6): self.assertTrue(gateway.charge('client', write=True))
            self.assertFalse(gateway.charge('client', write=True))
            self.assertTrue(gateway.charge('different', write=True))
            self.assertTrue(gateway.charge('client', write=False))

    def test_public_listener_refused(self):
        with self.assertRaises(ValueError): g.Server(('0.0.0.0', 0), g.Gateway())


class HTTPTests(unittest.TestCase):
    def setUp(self):
        self.gateway = g.Gateway()
        self.gateway.verify_network = lambda **kw: None
        self.gateway.query = lambda method, params: {'jsonrpc': '2.0', 'id': 1, 'result': hex(g.CHAIN_ID)}
        self.server = g.Server(('127.0.0.1', 0), self.gateway)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True); self.thread.start()

    def tearDown(self):
        self.server.shutdown(); self.server.server_close(); self.thread.join()

    def call(self, method, path, body=None, headers=None):
        conn = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=3)
        conn.request(method, path, body, headers or {})
        response = conn.getresponse(); data = response.read(); status = response.status; conn.close()
        return status, data

    def test_actual_http_relay_health_and_allowlist(self):
        status, data = self.call('GET', '/studio/health'); self.assertEqual(status, 200); self.assertTrue(json.loads(data)['ready'])
        status, data = self.call('POST', '/studio/rpc', json.dumps(request()), {'Content-Type': 'application/json'})
        self.assertEqual(status, 200); self.assertEqual(json.loads(data)['result'], hex(g.CHAIN_ID))
        status, _ = self.call('POST', '/studio/rpc', json.dumps(request('admin_peers')), {'Content-Type': 'application/json'})
        self.assertEqual(status, 400)
        status, _ = self.call('POST', '/rpc', '{}', {'Content-Type': 'application/json'}); self.assertEqual(status, 404)

    def test_size_content_type_and_bad_json(self):
        status, _ = self.call('POST', '/studio/rpc', 'x'*(g.MAX_REQUEST+1), {'Content-Type': 'application/json'}); self.assertEqual(status, 413)
        status, _ = self.call('POST', '/studio/rpc', '{}'); self.assertEqual(status, 415)
        status, _ = self.call('POST', '/studio/rpc', '{', {'Content-Type': 'application/json'}); self.assertEqual(status, 400)


OTHER_GENESIS = '0x' + '5a' * 32
CHUNK = b'\x00' + b'site chunk bytes' * 64
ADDR = '0x' + 'c1' * 20


class Clock:
    def __init__(self, t=1000.0): self.t = t
    def __call__(self): return self.t


class Node:
    """A mock upstream node: identity answers and code at `safe` and `latest` (any other tag reads `latest`)."""

    def __init__(self, chain=g.CHAIN_ID, genesis=g.GENESIS):
        self.chain, self.genesis = chain, genesis
        self.code = {'safe': {}, 'latest': {}}
        self.calls = []

    def put(self, address, code, *tags):
        for at in tags: self.code[at][address.lower()] = '0x' + code.hex()

    def drop(self, address, *tags):
        for at in tags: self.code[at].pop(address.lower(), None)

    def query(self, method, params):
        self.calls.append(method if method != 'eth_getCode' else (method, params[1]))
        if method == 'pocol_status': return {'jsonrpc': '2.0', 'id': 1, 'result': {'chainId': self.chain, 'genesisHash': self.genesis}}
        if method == 'eth_chainId': return {'jsonrpc': '2.0', 'id': 1, 'result': hex(self.chain)}
        if method == 'eth_getCode':
            table = self.code['safe' if params[1] == 'safe' else 'latest']
            return {'jsonrpc': '2.0', 'id': 1, 'result': table.get(params[0].lower(), '0x')}
        return {'jsonrpc': '2.0', 'id': 1, 'result': '0x1'}

    def code_calls(self):
        return [c for c in self.calls if isinstance(c, tuple)]


def gateway_with(node=None, clock=None, network=None):
    node = node or Node(*(network or ()))
    gateway = g.Gateway(network=network, clock=clock or Clock())
    gateway.query = node.query
    return gateway, node


def code(tag='latest', address=ADDR, ident=1):
    return request('eth_getCode', [address, tag], ident)


class ChainIdentityTests(unittest.TestCase):
    def test_minimal_chain_ids_of_one_three_and_four_bytes_are_accepted_when_configured(self):
        for chain in (1, 0x123456, g.CHAIN_ID):
            with self.subTest(chain=chain):
                self.assertTrue(g.valid_raw_transaction(tx(chain=chain), chain))
                self.assertTrue(g.valid_payload(request('eth_sendRawTransaction', [tx(chain=chain)]), chain))
        self.assertEqual(len(minimal(1)), 1); self.assertEqual(len(minimal(0x123456)), 3); self.assertEqual(len(minimal(g.CHAIN_ID)), 4)

    def test_padded_and_oversized_chain_ids_are_refused(self):
        # A leading zero is never minimal RLP, even when the value matches (the pre-W1 check demanded exactly four bytes).
        self.assertFalse(g.valid_raw_transaction(tx(chain_bytes=b'\x00' + minimal(g.CHAIN_ID))))
        self.assertFalse(g.valid_raw_transaction(tx(chain_bytes=(1).to_bytes(4, 'big')), 1))
        self.assertFalse(g.valid_raw_transaction(tx(chain_bytes=(g.CHAIN_ID + (1 << 32)).to_bytes(5, 'big'))))
        self.assertFalse(g.valid_raw_transaction(tx(chain_bytes=b'')))
        self.assertFalse(g.valid_raw_transaction(tx(chain_bytes=b'\x01' * 9), int.from_bytes(b'\x01' * 9, 'big')))

    def test_a_configured_chain_id_above_two_to_the_32_does_not_crash(self):
        big = (1 << 40) + 7
        gateway, _ = gateway_with(network=(big, OTHER_GENESIS))
        self.assertEqual(gateway.chain_id, big)
        self.assertTrue(g.valid_payload(request('eth_sendRawTransaction', [tx(chain=big)]), gateway.chain_id))
        self.assertFalse(g.valid_payload(request('eth_sendRawTransaction', [tx()]), gateway.chain_id))
        self.assertIn('result', gateway.call(request('eth_sendRawTransaction', [tx(chain=big)])))

    def test_identity_compare_is_canonical(self):
        upper = '0x' + g.GENESIS[2:].upper()
        for chain_answer, status in ((hex(g.CHAIN_ID).upper().replace('0X', '0x'), {'chainId': g.CHAIN_ID, 'genesisHash': upper}),
                                     (hex(g.CHAIN_ID), {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS})):
            gateway = g.Gateway(clock=Clock())
            gateway.query = lambda m, p, c=chain_answer, s=status: {'result': s if m == 'pocol_status' else c}
            with self.subTest(status=status): self.assertIsNotNone(gateway.verify_network(force=True))
        bad = [('0x0' + hex(g.CHAIN_ID)[2:], {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}),
               (hex(g.CHAIN_ID), {'chainId': str(g.CHAIN_ID), 'genesisHash': g.GENESIS}),
               (hex(g.CHAIN_ID), {'chainId': True, 'genesisHash': g.GENESIS}),
               (hex(g.CHAIN_ID), {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS[:-2]}),
               (hex(g.CHAIN_ID), {'chainId': g.CHAIN_ID, 'genesisHash': OTHER_GENESIS}),
               (g.CHAIN_ID, {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}),
               (hex(g.CHAIN_ID + 1), {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS})]
        for chain_answer, status in bad:
            gateway = g.Gateway(clock=Clock())
            gateway.query = lambda m, p, c=chain_answer, s=status: {'result': s if m == 'pocol_status' else c}
            with self.subTest(chain=chain_answer, status=status), self.assertRaises(RuntimeError):
                gateway.verify_network(force=True)

    def test_identity_is_rechecked_after_its_ttl_only(self):
        clock = Clock(); gateway, node = gateway_with(clock=clock)
        gateway.call(request('eth_blockNumber')); gateway.call(request('eth_blockNumber'))
        self.assertEqual(node.calls.count('pocol_status'), 1)
        clock.t += g.IDENTITY_TTL
        gateway.call(request('eth_blockNumber'))
        self.assertEqual(node.calls.count('pocol_status'), 2)


class NetworkFileTests(unittest.TestCase):
    GOOD = {'chainId': 777, 'genesisHash': OTHER_GENESIS, 'specVersion': 2, 'v3': True}

    def write(self, directory, text):
        path = Path(directory) / 'network.json'; path.write_text(text); return str(path)

    def test_the_explorer_format_is_accepted_and_served(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(g.load_network(self.write(directory, json.dumps(self.GOOD))), (777, OTHER_GENESIS))
            source = {**self.GOOD, 'source': 'https://example.org/net'}
            self.assertEqual(g.load_network(self.write(directory, json.dumps(source))), (777, OTHER_GENESIS))
        self.assertIsNone(g.load_network(None))
        gateway, _ = gateway_with(network=(777, OTHER_GENESIS))
        self.assertEqual((gateway.chain_id, gateway.genesis), (777, OTHER_GENESIS))
        self.assertEqual(gateway.call(request('net_version'))['result'], '777')
        # Without a file the pinned constants stay the default.
        self.assertEqual((g.Gateway().chain_id, g.Gateway().genesis), (g.CHAIN_ID, g.GENESIS))

    def test_invalid_network_files_are_refused(self):
        bad = [{**self.GOOD, 'extra': 1}, {k: v for k, v in self.GOOD.items() if k != 'v3'}, {**self.GOOD, 'specVersion': 1},
               {**self.GOOD, 'specVersion': True}, {**self.GOOD, 'specVersion': 2.0}, {**self.GOOD, 'v3': 1},
               {**self.GOOD, 'chainId': True}, {**self.GOOD, 'chainId': 777.0}, {**self.GOOD, 'chainId': '777'},
               {**self.GOOD, 'chainId': 0}, {**self.GOOD, 'chainId': 1 << 53}, {**self.GOOD, 'genesisHash': OTHER_GENESIS.upper()},
               {**self.GOOD, 'genesisHash': OTHER_GENESIS[:-2]}, {**self.GOOD, 'genesisHash': g.GENESIS},
               {**self.GOOD, 'source': 'http://example.org'}, [self.GOOD]]
        with tempfile.TemporaryDirectory() as directory:
            for value in bad:
                with self.subTest(value=value), self.assertRaises(ValueError):
                    g.load_network(self.write(directory, json.dumps(value)))
            for text in ('{', '{"chainId": 777, "chainId": 778, "genesisHash": "%s", "specVersion": 2, "v3": true}' % OTHER_GENESIS,
                         '{"chainId": NaN, "genesisHash": "%s", "specVersion": 2, "v3": true}' % OTHER_GENESIS, ' ' * 4097):
                with self.subTest(text=text[:40]), self.assertRaises(ValueError):
                    g.load_network(self.write(directory, text))

    def test_cli_refuses_a_bad_file_or_bind_before_connecting(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(g, 'Gateway') as gateway, \
                patch('sys.stderr'):
            for argv in (['--network', self.write(directory, '{}')], ['--network', directory + '/missing.json'],
                         ['--bind', '0.0.0.0'], ['--port', '0']):
                with self.subTest(argv=argv), self.assertRaises(SystemExit):
                    g.main(argv)
            gateway.assert_not_called()

    def test_cli_passes_the_file_identity_to_the_gateway(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(g, 'Gateway') as gateway, \
                patch.object(g, 'Server') as server, patch('builtins.print'):
            g.main(['--network', self.write(directory, json.dumps(self.GOOD)), '--rpc-port', '9545'])
            gateway.assert_called_once_with(9545, (777, OTHER_GENESIS))
            gateway.return_value.verify_network.assert_called_once_with(force=True)
            self.assertEqual(server.call_args.args[0], ('127.0.0.1', 8089))


class ChunkCacheTests(unittest.TestCase):
    def test_a_hit_makes_no_upstream_call(self):
        gateway, node = gateway_with(); node.put(ADDR, CHUNK, 'safe', 'latest')
        self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex())
        self.assertEqual(node.code_calls(), [('eth_getCode', 'safe')])
        before = list(node.calls)
        answer = gateway.call(code('latest', ADDR.upper().replace('0X', '0x'), ident=9))
        self.assertEqual(answer, {'jsonrpc': '2.0', 'id': 9, 'result': '0x' + CHUNK.hex()})
        self.assertEqual(gateway.call(code('safe'))['result'], '0x' + CHUNK.hex())
        self.assertEqual(node.calls, before)
        self.assertEqual(gateway.cache.hits, 2)

    def test_empty_code_is_never_cached(self):
        gateway, node = gateway_with()
        for _ in range(2): self.assertEqual(gateway.call(code())['result'], '0x')
        self.assertEqual(node.code_calls(), [('eth_getCode', 'safe'), ('eth_getCode', 'latest')] * 2)
        self.assertEqual(len(gateway.cache.entries), 0)

    def test_code_that_is_not_chunk_shaped_is_answered_but_not_cached(self):
        for raw in (b'\x60\x80\x60\x40', b'\x00' * (g.CHUNK_CODE_MAX + 1)):
            gateway, node = gateway_with(); node.put(ADDR, raw, 'safe', 'latest')
            with self.subTest(size=len(raw)):
                for _ in range(2): self.assertEqual(gateway.call(code())['result'], '0x' + raw.hex())
                self.assertEqual(len(gateway.cache.entries), 0)
                self.assertEqual(node.code_calls().count(('eth_getCode', 'latest')), 2)
        largest = b'\x00' + b'z' * (g.CHUNK_CODE_MAX - 1)
        gateway, node = gateway_with(); node.put(ADDR, largest, 'safe')
        self.assertEqual(gateway.call(code())['result'], '0x' + largest.hex()); self.assertEqual(len(gateway.cache.entries), 1)

    def test_a_chunk_at_latest_but_not_at_safe_is_answered_but_not_cached(self):
        gateway, node = gateway_with(); node.put(ADDR, CHUNK, 'latest')
        for _ in range(2): self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex())
        self.assertEqual(node.code_calls(), [('eth_getCode', 'safe'), ('eth_getCode', 'latest')] * 2)
        self.assertEqual(len(gateway.cache.entries), 0)
        # Asked at `safe`, the node's `safe` answer is the answer.
        self.assertEqual(gateway.call(code('safe'))['result'], '0x')

    def test_a_safe_request_fills_the_cache_for_latest(self):
        gateway, node = gateway_with(); node.put(ADDR, CHUNK, 'safe')
        self.assertEqual(gateway.call(code('safe'))['result'], '0x' + CHUNK.hex())
        self.assertEqual(gateway.call(code('latest'))['result'], '0x' + CHUNK.hex())
        self.assertEqual(node.code_calls(), [('eth_getCode', 'safe')])

    def test_finalized_pending_and_numbered_requests_bypass_the_cache(self):
        gateway, node = gateway_with(); node.put(ADDR, CHUNK, 'safe', 'latest')
        gateway.call(code())
        for at in ('finalized', 'pending', 'earliest', '0x10'):
            with self.subTest(tag=at):
                self.assertEqual(g.classify(code(at)), 'read')
                before = len(node.code_calls())
                gateway.call(code(at))
                self.assertEqual(node.code_calls()[before:], [('eth_getCode', at)])

    def test_the_lru_bound_holds(self):
        cache = g.ChunkCache(limit=3 * (1000 + g.CHUNK_ENTRY_OVERHEAD))
        epoch = cache.bind(g.GENESIS)
        for i in range(3): self.assertTrue(cache.put(epoch, f'a{i}', b'\x00' * 1000))
        self.assertIsNotNone(cache.get(g.GENESIS, 'a0'))   # a0 becomes the most recently used
        self.assertTrue(cache.put(epoch, 'a3', b'\x00' * 1000))
        self.assertEqual(list(cache.entries), ['a2', 'a0', 'a3'])
        self.assertLessEqual(cache.size, cache.limit)
        self.assertEqual(cache.size, sum(len(v) + g.CHUNK_ENTRY_OVERHEAD for v in cache.entries.values()))
        self.assertFalse(cache.put(epoch, 'huge', b'\x00' * cache.limit))
        # Tiny entries still pay the per-entry overhead, so the entry count is bounded too.
        tiny = g.ChunkCache(limit=10 * (1 + g.CHUNK_ENTRY_OVERHEAD)); e = tiny.bind(g.GENESIS)
        for i in range(50): tiny.put(e, f't{i}', b'\x00')
        self.assertEqual(len(tiny.entries), 10)
        self.assertEqual(g.CHUNK_CACHE_BYTES, 24 * 1024 * 1024)

    def test_the_cache_is_bound_to_the_verified_genesis_and_flushed_on_an_identity_change(self):
        clock = Clock(); gateway, node = gateway_with(clock=clock); node.put(ADDR, CHUNK, 'safe', 'latest')
        gateway.call(code()); self.assertEqual(len(gateway.cache.entries), 1)
        self.assertIsNone(gateway.cache.get(OTHER_GENESIS, ADDR))
        # The node behind the gateway is replaced by another network: after the TTL the hit needs a fresh identity.
        node.genesis = OTHER_GENESIS; clock.t += g.IDENTITY_TTL
        answer = gateway.call(code())
        self.assertEqual(answer['error']['code'], -32000)
        self.assertEqual(len(gateway.cache.entries), 0); self.assertIsNone(gateway.cache.identity)
        node.genesis = g.GENESIS
        before = len(node.code_calls())
        self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex())
        self.assertEqual(node.code_calls()[before:], [('eth_getCode', 'safe')], 'refilled from the node, not served stale')

    def test_a_fill_started_before_a_flush_is_discarded(self):
        cache = g.ChunkCache(); epoch = cache.bind(g.GENESIS)
        cache.flush(); cache.bind(g.GENESIS)
        self.assertFalse(cache.put(epoch, ADDR, CHUNK)); self.assertEqual(len(cache.entries), 0)
        unbound = g.ChunkCache()
        self.assertFalse(unbound.put(unbound.epoch, ADDR, CHUNK))

    def test_a_chunk_reorged_out_before_safe_is_reported_missing_and_stored_again(self):
        gateway, node = gateway_with(); node.put(ADDR, CHUNK, 'latest')
        self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex())
        node.drop(ADDR, 'latest')                              # a reorg above `safe` removes it
        self.assertEqual(gateway.call(code())['result'], '0x')  # the publisher sees it missing ...
        node.put(ADDR, CHUNK, 'latest')                         # ... and stores it again
        self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex())
        self.assertEqual(len(gateway.cache.entries), 0)
        node.put(ADDR, CHUNK, 'safe')                           # once at `safe` it is cached
        gateway.call(code()); self.assertEqual(len(gateway.cache.entries), 1)

    def test_hits_are_served_while_every_general_slot_is_busy_and_misses_use_reserved_slots(self):
        gateway, node = gateway_with(); node.put(ADDR, CHUNK, 'safe')
        other = '0x' + 'c2' * 20; node.put(other, CHUNK, 'safe')
        gateway.call(code())
        for _ in range(g.GENERAL_SLOTS): gateway.rpc_slots.acquire()
        try:
            self.assertEqual(gateway.call(request('eth_blockNumber'))['error']['code'], -32021)
            self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex())
            self.assertEqual(gateway.call(code('latest', other))['result'], '0x' + CHUNK.hex(), 'a miss uses a chunk slot')
            for _ in range(g.CHUNK_SLOTS): gateway.chunk_slots.acquire()
            try:
                self.assertEqual(gateway.call(code('latest', '0x' + 'c3' * 20))['error']['code'], -32021)
                self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex(), 'hits need no slot')
            finally:
                for _ in range(g.CHUNK_SLOTS): gateway.chunk_slots.release()
        finally:
            for _ in range(g.GENERAL_SLOTS): gateway.rpc_slots.release()

    def test_a_hit_after_the_identity_ttl_rechecks_the_identity_first(self):
        clock = Clock(); gateway, node = gateway_with(clock=clock); node.put(ADDR, CHUNK, 'safe')
        gateway.call(code()); clock.t += g.IDENTITY_TTL
        before = len(node.calls)
        self.assertEqual(gateway.call(code())['result'], '0x' + CHUNK.hex())
        self.assertEqual(node.calls[before:], ['pocol_status', 'eth_chainId'])

    def test_upstream_failure_is_a_structured_error(self):
        gateway, node = gateway_with()
        def broken(method, params):
            if method == 'eth_getCode': raise TimeoutError()
            return node.query(method, params)
        gateway.query = broken
        self.assertEqual(gateway.call(code())['error']['code'], -32000)


class BudgetTests(unittest.TestCase):
    def test_every_item_is_classified_before_charging(self):
        self.assertEqual([g.classify(r) for r in (code(), code('safe'), code('finalized'), request('net_version'),
                                                  request('eth_sendRawTransaction', [tx()]), request('eth_blockNumber'))],
                         ['chunk', 'chunk', 'read', 'local', 'write', 'read'])
        self.assertEqual(g.read_cost(request('pocol_getLogs', [{'blockHash': '0x' + '1' * 64}])), g.LOGS_COST)
        self.assertEqual(g.read_cost(request('eth_blockNumber')), 1)

    def test_chunk_bucket_exhaustion_leaves_the_read_bucket_untouched(self):
        gateway = g.Gateway(clock=Clock())
        capacity = dict((k, c) for k, c, _ in g.CHUNK_BUCKETS)['chunk']
        for _ in range(capacity): self.assertTrue(gateway.charge('viewer', 0, chunks=1))
        self.assertFalse(gateway.charge('viewer', 0, chunks=1))
        self.assertNotIn(('client', 'viewer'), gateway.buckets)
        self.assertNotIn('global-read', gateway.buckets)
        self.assertTrue(gateway.charge('viewer', 1))
        self.assertTrue(gateway.charge('another', 0, chunks=1))
        # A mixed request is all or nothing: its read part is not taken when the chunk part fails.
        read_before = gateway.buckets[('client', 'viewer')]
        self.assertFalse(gateway.charge('viewer', 1, chunks=1))
        self.assertEqual(gateway.buckets[('client', 'viewer')], read_before)

    def test_buckets_refill_with_the_injected_clock(self):
        clock = Clock(); gateway = g.Gateway(clock=clock)
        capacity, rate = [(c, r) for k, c, r in g.CHUNK_BUCKETS if k == 'chunk'][0]
        for _ in range(capacity): gateway.charge('viewer', 0, chunks=1)
        self.assertFalse(gateway.charge('viewer', 0, chunks=1))
        clock.t += 1   # exact in binary floating point: one second refills `rate` tokens
        for _ in range(rate): self.assertTrue(gateway.charge('viewer', 0, chunks=1))
        self.assertFalse(gateway.charge('viewer', 0, chunks=1))

    def test_one_four_mib_site_fits_one_clients_chunk_burst(self):
        # About 171 chunks plus the manifest (SITES-PLAN W1 sizing; Q8 tunes it).
        capacity = dict((k, c) for k, c, _ in g.CHUNK_BUCKETS)
        self.assertGreaterEqual(capacity['chunk'], 175); self.assertGreaterEqual(capacity['global-chunk'], 4 * 175)


class AllowlistTests(unittest.TestCase):
    H = '0x' + 'ab' * 32

    def test_dapp_read_methods_and_their_bounds(self):
        ok = [request('eth_maxPriorityFeePerGas'),
              request('eth_feeHistory', ['0x80', 'latest', [10, 50.5, 90]]), request('eth_feeHistory', ['0x1', 'safe', []]),
              request('eth_getStorageAt', [ADDR, self.H, 'latest']), request('eth_getStorageAt', [ADDR, '0x0', 'finalized']),
              request('eth_getBlockByHash', [self.H, False]),
              request('eth_getBalance', [ADDR, 'safe']), request('eth_call', [{'to': ADDR}, 'finalized'])]
        bad = [request('eth_maxPriorityFeePerGas', ['latest']),
               request('eth_feeHistory', ['0x81', 'latest', []]), request('eth_feeHistory', [5, 'latest', []]),
               request('eth_feeHistory', ['0x01', 'latest', []]), request('eth_feeHistory', ['0x1', 'latest', list(range(17))]),
               request('eth_feeHistory', ['0x1', 'latest', [50, 10]]), request('eth_feeHistory', ['0x1', 'latest', [101]]),
               request('eth_feeHistory', ['0x1', 'latest', [True]]), request('eth_feeHistory', ['0x1', 'latest', ['5']]),
               request('eth_feeHistory', ['0x1', 'latest', [float('nan')]]), request('eth_feeHistory', ['0x1', 'latest']),
               request('eth_getStorageAt', [ADDR, '0x' + '1' * 65, 'latest']), request('eth_getStorageAt', [ADDR, self.H]),
               request('eth_getStorageAt', [ADDR, self.H, 'newest']),
               request('eth_getBlockByHash', [self.H, True]), request('eth_getBlockByHash', [self.H[:-2], False]),
               request('eth_getLogs', [{'blockHash': self.H}]), request('web3_sha3', ['0x00'])]
        for r in ok:
            with self.subTest(ok=r): self.assertTrue(g.valid_call(r))
        for r in bad:
            with self.subTest(bad=r): self.assertFalse(g.valid_call(r))

    def test_log_filters_mirror_the_extension_log_client(self):
        A2 = '0x' + 'c2' * 20
        ok = [{'blockHash': self.H}, {'blockHash': self.H, 'address': ADDR, 'topics': [self.H, None, [self.H, self.H]]},
              {'fromBlock': '0x0', 'toBlock': '0x3ff'}, {'fromBlock': '0x10', 'toBlock': '0x10', 'address': [ADDR, A2]},
              {'fromBlock': '0x1', 'toBlock': '0x2', 'topics': []}]
        bad = [{'blockHash': self.H, 'fromBlock': '0x0'}, {'fromBlock': '0x0', 'toBlock': '0x400'}, {'fromBlock': '0x2', 'toBlock': '0x1'},
               {'fromBlock': 'latest', 'toBlock': 'latest'}, {'fromBlock': '0x01', 'toBlock': '0x2'}, {'fromBlock': '0x1'},
               {'fromBlock': '0x1', 'toBlock': '0x2', 'address': [ADDR] * 17}, {'blockHash': self.H, 'topics': [None] * 5},
               {'blockHash': self.H, 'topics': [[self.H] * 17]}, {'blockHash': self.H, 'topics': [1]},
               {'blockHash': self.H, 'address': '0x1'}, {'blockHash': self.H, 'limit': 1}, {'blockHash': self.H[:-1]}, []]
        for f in ok:
            with self.subTest(ok=f): self.assertTrue(g.valid_call(request('pocol_getLogs', [f])))
        for f in bad:
            with self.subTest(bad=f): self.assertFalse(g.valid_call(request('pocol_getLogs', [f])))
        self.assertFalse(g.valid_call(request('pocol_getLogs', [ok[0], ok[0]])))

    def test_net_version_and_client_version_are_answered_without_the_node(self):
        gateway = g.Gateway()
        gateway.query = MagicMock(side_effect=AssertionError('no upstream call'))
        gateway.verify_network = MagicMock(side_effect=AssertionError('no upstream call'))
        self.assertEqual(gateway.call(request('net_version', ident=4)), {'jsonrpc': '2.0', 'id': 4, 'result': str(g.CHAIN_ID)})
        self.assertEqual(gateway.call(request('web3_clientVersion'))['result'], g.CLIENT_VERSION)
        self.assertFalse(g.valid_call(request('net_version', ['x'])))

    def test_non_finite_numbers_and_duplicate_keys_are_refused_on_the_wire(self):
        for raw in (b'{"jsonrpc":"2.0","id":1,"method":"eth_feeHistory","params":["0x1","latest",[NaN]]}',
                    b'{"jsonrpc":"2.0","id":1,"id":2,"method":"eth_chainId","params":[]}'):
            with self.assertRaises(ValueError): g.strict_json(raw)


class HTTPChunkTests(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.gateway, self.node = gateway_with(clock=self.clock)
        self.node.put(ADDR, CHUNK, 'safe')
        self.server = g.Server(('127.0.0.1', 0), self.gateway)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True); self.thread.start()

    def tearDown(self):
        self.server.shutdown(); self.server.server_close(); self.thread.join()

    def post(self, value, client='203.0.113.5'):
        conn = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=3)
        conn.request('POST', '/studio/rpc', json.dumps(value), {'Content-Type': 'application/json', 'X-Pocol-Client-IP': client})
        response = conn.getresponse(); data = response.read(); conn.close()
        return response.status, json.loads(data)

    def test_a_batch_is_charged_per_class_and_answered_in_order(self):
        charges = []; charge = self.gateway.charge
        self.gateway.charge = lambda *a, **kw: charges.append((a, kw)) or charge(*a, **kw)
        status, answer = self.post([code(ident=1), request('net_version', ident=2), request('pocol_getLogs', [{'blockHash': '0x' + '1' * 64}], 3)])
        self.assertEqual(status, 200)
        self.assertEqual([a['id'] for a in answer], [1, 2, 3])
        self.assertEqual(answer[0]['result'], '0x' + CHUNK.hex()); self.assertEqual(answer[1]['result'], str(g.CHAIN_ID))
        self.assertEqual(charges, [(('203.0.113.5', 1 + g.LOGS_COST), {'write': False, 'chunks': 1})])

    def test_chunk_exhaustion_is_429_while_reads_still_pass(self):
        capacity = dict((k, c) for k, c, _ in g.CHUNK_BUCKETS)['chunk']
        batch = [code(ident=i) for i in range(g.MAX_BATCH)]
        for _ in range(capacity // g.MAX_BATCH): self.assertEqual(self.post(batch)[0], 200)
        status, _ = self.post(batch)
        self.assertEqual(status, 429)
        self.assertEqual(self.post(request('eth_chainId'))[0], 200)
        self.assertEqual(self.post(code(), client='203.0.113.6')[0], 200, 'another client has its own chunk bucket')

    def test_health_reports_the_configured_identity(self):
        conn = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=3)
        conn.request('GET', '/studio/health'); value = json.loads(conn.getresponse().read()); conn.close()
        self.assertEqual((value['chainId'], value['genesisHash'], value['ready']), (g.CHAIN_ID, g.GENESIS, True))


class CaddyTests(unittest.TestCase):
    BASE = '''gateway.pocol.net {
        header {
            Cache-Control "no-store"
            X-Content-Type-Options "nosniff"
            -Server
        }
        request_body { max_size 16384 }
        @rpc { path /rpc }
        handle @rpc {
            @oversized expression `{http.request.header.Content-Length} != "" && int({http.request.header.Content-Length}) > 16384`
            respond @oversized "Request body exceeds 16 KiB" 413
            reverse_proxy 127.0.0.1:8088
        }
        handle { respond "Not found" 404 }
    }'''

    def test_preserve_original_explorer_guard_and_idempotence(self):
        patched = patch_caddy(self.BASE)
        self.assertIn(ROUTE, patched); self.assertIn('max_size 131072', patched)
        self.assertIn('int({http.request.header.Content-Length}) > 16384', patched)
        self.assertIn('X-Content-Type-Options "nosniff"', patched)
        self.assertIn('Cache-Control "no-store"', patched)
        self.assertEqual(patch_caddy(patched), patched)
        self.assertEqual(patched.replace(ROUTE, '').replace('max_size 131072', 'max_size 16384'), self.BASE)

    def test_the_relay_port_is_reached_only_through_the_header_setting_route(self):
        self.assertIn(installer.CLIENT_IP_HEADER, installer.ROUTE)
        other = self.BASE.replace('handle { respond "Not found" 404 }',
                                  'handle /other { reverse_proxy 127.0.0.1:8089 }\n        handle { respond "Not found" 404 }')
        with self.assertRaisesRegex(RuntimeError, 'reaches the Studio relay port'): patch_caddy(other)
        patched = patch_caddy(self.BASE)
        with self.assertRaisesRegex(RuntimeError, 'reaches the Studio relay port'):
            patch_caddy(patched.replace('handle { respond "Not found" 404 }', 'handle /x { reverse_proxy localhost:8089 }'))
        for route in (installer.ROUTE.replace('            ' + installer.CLIENT_IP_HEADER + '\n', ''),
                      installer.ROUTE.replace('127.0.0.1:8089', '0.0.0.0:8089')):
            with patch.object(installer, 'ROUTE', route), self.assertRaisesRegex(RuntimeError, 'X-Pocol-Client-IP'):
                patch_caddy(self.BASE)
        with patch.object(installer, 'UNIT_TEXT', installer.UNIT_TEXT.replace(' --bind 127.0.0.1', '')), \
                self.assertRaisesRegex(RuntimeError, 'bind the relay'):
            patch_caddy(self.BASE)

    def test_unrecognized_configs_refused(self):
        for text in ('example.com {}', self.BASE.replace('max_size 16384', 'max_size 20000'), self.BASE.replace('Request body exceeds 16 KiB', 'changed')):
            with self.assertRaises(RuntimeError): patch_caddy(text)
        with self.assertRaises(RuntimeError): patch_caddy(patch_caddy(self.BASE).replace('127.0.0.1:8089', '0.0.0.0:8089'))


class InstallerTests(unittest.TestCase):
    def test_rollback_continues_after_caddy_command_timeout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); base = root/'opt'; unit = root/'gateway.service'; caddy = root/'Caddyfile'
            base.mkdir(); source = base/'pocol_studio_gateway.py'; marker = base/'managed-by-studio-v1'
            for path in (source, marker, unit): path.write_bytes(b'new')
            caddy.write_bytes(b'new caddy')
            commands = []
            def command(*args):
                commands.append(args)
                if args == ('systemctl', 'reload', 'caddy'):
                    raise subprocess.TimeoutExpired(args, 45)
                return SimpleNamespace(returncode=0)
            with patch.multiple(installer, BASE=base, UNIT=unit, CADDYFILE=caddy), \
                 patch.object(installer, 'command', side_effect=command):
                failures = installer.rollback_install(b'old caddy', {source: None, marker: None, unit: None}, False, False, False)
            self.assertEqual(caddy.read_bytes(), b'old caddy')
            self.assertFalse(base.exists()); self.assertFalse(unit.exists())
            self.assertEqual(len(failures), 1)
            self.assertIn('reload previous Caddy configuration', failures[0])
            self.assertIn(('systemctl', 'stop', installer.SERVICE), commands)
            self.assertIn(('systemctl', 'disable', installer.SERVICE), commands)
            self.assertEqual(commands[-1], ('systemctl', 'daemon-reload'))

    def test_the_unit_binds_loopback_with_room_for_the_chunk_cache(self):
        self.assertIn('ExecStart=/usr/bin/python3 /opt/pocol-studio-gateway/pocol_studio_gateway.py --bind 127.0.0.1 --port 8089 --rpc-port 8545\n',
                      installer.UNIT_TEXT)
        self.assertIn('MemoryMax=256M\n', installer.UNIT_TEXT); self.assertNotIn('MemoryMax=128M', installer.UNIT_TEXT)
        # The unit text the previous installer wrote, byte for byte: an update may replace exactly that.
        self.assertEqual([hashlib.sha256(t.encode()).hexdigest() for t in installer.PRIOR_UNIT_TEXTS],
                         ['591e2ecaf31978a8b8385bf0563ae344b26b6f57da4b5d3647b325ef149ee8cc'])
        # The arguments the unit passes are ones the gateway's own parser accepts.
        with patch.object(g, 'Gateway') as gateway, patch.object(g, 'Server'), patch('builtins.print'):
            g.main(installer.UNIT_TEXT.split('pocol_studio_gateway.py ', 1)[1].split('\n', 1)[0].split())
            gateway.assert_called_once_with(8545, None)

    def test_identity_compare_is_canonical(self):
        upper = '0x' + g.GENESIS[2:].upper()
        self.assertTrue(installer.same_identity({'chainId': g.CHAIN_ID, 'genesisHash': upper}, g.CHAIN_ID, g.GENESIS))
        self.assertTrue(installer.same_identity({'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}, g.CHAIN_ID, upper))
        for value in ({'chainId': str(g.CHAIN_ID), 'genesisHash': g.GENESIS}, {'chainId': True, 'genesisHash': g.GENESIS},
                      {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS[2:]}, {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS + '00'},
                      {'chainId': g.CHAIN_ID, 'genesisHash': '0x' + 'g' * 64}, {'chainId': g.CHAIN_ID}, None, []):
            with self.subTest(value=value): self.assertFalse(installer.same_identity(value, g.CHAIN_ID, g.GENESIS))
        health = {'service': 'pocol-studio-gateway', 'ready': True, 'chainId': g.CHAIN_ID, 'genesisHash': upper}
        explorer = {'jsonrpc': '2.0', 'id': 1, 'result': {'chainId': g.CHAIN_ID, 'genesisHash': upper}}
        with patch.object(installer, 'tls_json', side_effect=[health, explorer]): installer.verify_https_routes()
        with patch.object(installer, 'local_json', return_value=health): installer.verify_health()
        with patch.object(installer, 'local_json', return_value={**health, 'chainId': float(g.CHAIN_ID)}), \
                self.assertRaises(RuntimeError):
            installer.verify_health()

    def test_https_checks_both_routes_and_both_identities(self):
        health = {'service': 'pocol-studio-gateway', 'ready': True, 'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}
        explorer = {'jsonrpc': '2.0', 'id': 1, 'result': {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}}
        with patch.object(installer, 'tls_json', side_effect=[health, explorer]) as get:
            installer.verify_https_routes()
            self.assertEqual(get.call_args_list[0].args, ('/studio/health',))
            self.assertEqual(get.call_args_list[1].args, ('/rpc', request('pocol_getParams')))
        with patch.object(installer, 'tls_json', side_effect=[health, {**explorer, 'result': {'chainId': 1, 'genesisHash': g.GENESIS}}]):
            with self.assertRaisesRegex(RuntimeError, 'Existing Explorer HTTPS'):
                installer.verify_https_routes()
        with patch.object(installer, 'tls_json', return_value={**health, 'ready': False}):
            with self.assertRaisesRegex(RuntimeError, 'Studio HTTPS'):
                installer.verify_https_routes()

    def test_tls_uses_public_hostname_trust_and_loopback_transport(self):
        context = MagicMock()
        connection = MagicMock()
        connection.getresponse.return_value.status = 200
        connection.getresponse.return_value.read.return_value = b'{"ready":true}'
        with patch.object(installer.ssl, 'create_default_context', return_value=context) as trust, \
             patch.object(installer.socket, 'create_connection', return_value='local-socket') as sock, \
             patch.object(installer.http.client, 'HTTPSConnection', return_value=connection) as https:
            self.assertEqual(installer.tls_json('/studio/health'), {'ready': True})
            trust.assert_called_once_with()
            sock.assert_called_once_with(('127.0.0.1', 443), timeout=12)
            context.wrap_socket.assert_called_once_with('local-socket', server_hostname='gateway.pocol.net')
            https.assert_called_once_with('gateway.pocol.net', 443, timeout=12, context=context)
            connection.close.assert_called_once()

    def run_install(self, fail_https=False, existing=False, unit_text=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); base = root/'opt'; unit = root/'gateway.service'; caddy = root/'Caddyfile'; backups = root/'backups'
            original = CaddyTests.BASE.encode(); caddy.write_bytes(original)
            prior_source = b'# existing source preserved\n'
            if existing:
                base.mkdir(); (base/'managed-by-studio-v1').write_bytes(b'PoCol Studio gateway v1\n')
                (base/'pocol_studio_gateway.py').write_bytes(prior_source); unit.write_text(unit_text or installer.UNIT_TEXT)
            commands = []
            def command(*args, **kwargs):
                commands.append(args)
                absent = args[:3] in [('systemctl', 'is-active', '--quiet'), ('systemctl', 'is-enabled', '--quiet')] and args[-1] == installer.SERVICE and not existing
                return SimpleNamespace(returncode=1 if absent else 0, stdout=b'', stderr=b'')
            gateway = MagicMock()
            with patch.multiple(installer, BASE=base, UNIT=unit, CADDYFILE=caddy, BACKUPS=backups), \
                 patch.object(installer.os, 'geteuid', return_value=0), \
                 patch.object(installer.shutil, 'which', return_value='/test/tool'), \
                 patch.object(installer.sys, 'argv', ['installer']), \
                 patch.object(installer.socket, 'socket', return_value=MagicMock()), \
                 patch.object(installer, 'command', side_effect=command), \
                 patch.object(g, 'Gateway', return_value=gateway), \
                 patch.object(installer, 'verify_health'), \
                 patch.object(installer, 'verify_https_routes', side_effect=RuntimeError('TLS route failed') if fail_https else None) as verify, \
                 patch('builtins.print') as output:
                if fail_https:
                    with self.assertRaisesRegex(RuntimeError, 'TLS route failed'): installer.main()
                    self.assertEqual(caddy.read_bytes(), original)
                    if existing:
                        self.assertEqual((base/'pocol_studio_gateway.py').read_bytes(), prior_source)
                        self.assertTrue(unit.exists())
                        self.assertEqual(commands[-1], ('systemctl', 'restart', installer.SERVICE))
                    else:
                        self.assertFalse(base.exists()); self.assertFalse(unit.exists())
                    self.assertFalse(any('INSTALLED:' in str(c) for c in output.call_args_list))
                else:
                    installer.main(); verify.assert_called_once_with()
                    self.assertEqual(caddy.read_text(), patch_caddy(original.decode()))
                    self.assertEqual(unit.read_text(), installer.UNIT_TEXT)
                    self.assertIn(('systemctl', 'enable', installer.SERVICE), commands)
                    self.assertTrue(any('both Studio and existing Explorer HTTPS' in str(c) for c in output.call_args_list))
                gateway.verify_network.assert_called_once_with(force=True)
                self.assertEqual(len(list(backups.glob('*/Caddyfile.before'))), 1)

    def test_install_reports_success_only_after_https_routes_pass(self): self.run_install()
    def test_first_install_restores_caddy_and_removes_new_service_on_tls_failure(self): self.run_install(fail_https=True)
    def test_update_restores_prior_service_source_and_active_state_on_tls_failure(self): self.run_install(fail_https=True, existing=True)
    def test_update_replaces_the_previous_installers_unit(self): self.run_install(existing=True, unit_text=installer.PRIOR_UNIT_TEXTS[0])

    def test_update_refuses_a_hand_edited_unit(self):
        with self.assertRaisesRegex(RuntimeError, 'custom edits'):
            self.run_install(existing=True, unit_text=installer.UNIT_TEXT.replace('MemoryMax=256M', 'MemoryMax=1G'))


if __name__ == '__main__': unittest.main()
