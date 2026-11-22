#!/usr/bin/env python3
"""Sites W7: the explorer's refresh stays under the relay's 4 MiB answer limit at the mainnet profile's bounds.

The bundle's 15 s refresh (read from the served bundle, so a change there fails here) is three /api/rpc.php requests:
pocol_status + pocol_getParams + one pocol_getMembers page of 256, then 20 blocks with full=false, then
eth_getTransactionByHash for the 5 newest transaction hashes. Each is sent through the gateway's real relay
(deploy/rewards/explorer_api.py Relay.forward, its answer accounting and MAX_REPLY) over a stub gateway that answers
with the widest values the node can give: block and transaction fields as prototypes/pocold/src/rpc.rs block_json /
tx_json write them, at the profile's GAS_LIMIT, BODY_MAX and TX_CAP (deploy/mainnet/v3-profile.json), member rows as
deploy/rewards/gateway.py member_w0 writes them with every field at its widest. One full block (the account activity
and the block view read one per request) is about 393 KB and fits; 20 full blocks, what the refresh asked for before
W7, are over the limit and the relay refuses them (503): the regression guard for the light refresh.

    python3 -B -m unittest discover -s deploy/websites/tests -p 'test_*.py' -v
"""
import importlib.util, json, re, sys, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REWARDS = ROOT / 'deploy' / 'rewards'
sys.path.insert(0, str(REWARDS))
for extra in ('deploy/mainnet', 'deploy/public-testnet', 'deploy/studio'):
    sys.path.insert(0, str(ROOT / extra))
import explorer_api as api  # noqa: E402
_spec = importlib.util.spec_from_file_location('explorer_gateway_for_w7_size', REWARDS / 'gateway.py')
gateway = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gateway)

EXPLORER = ROOT / 'deploy' / 'websites' / 'explorer.pocol.net'
PROFILE = json.loads((ROOT / 'deploy' / 'mainnet' / 'v3-profile.json').read_text(encoding='utf-8'))['cp']
GAS_LIMIT, BODY_MAX, TX_CAP = PROFILE['GAS_LIMIT'], PROFILE['BODY_MAX'], PROFILE['TX_CAP']
TX_GAS_MIN, MIN_TX_BYTES = 21_000, 85            # prototypes/pocold/src/resources.rs
MAX_TXS = min(TX_CAP, GAS_LIMIT // TX_GAS_MIN, BODY_MAX // MIN_TX_BYTES)
U64, U256 = (1 << 64) - 1, (1 << 256) - 1
qty = hex
word = lambda b: '0x' + b * 32

def bundle_text():
    page = (EXPLORER / 'index.html').read_text(encoding='utf-8')
    found = re.findall(r'<script\b[^>]*\bsrc="/(assets/index-[A-Za-z0-9_-]+\.js)"', page)
    assert len(found) == 1, found
    return (EXPLORER / found[0]).read_text(encoding='utf-8')

def tx(i, input_bytes=0):
    """tx_json: every field at its widest; `input_bytes` of calldata."""
    return {'hash': '0x%064x' % (U256 - i), 'blockHash': word('ff'), 'blockNumber': qty(U64), 'transactionIndex': qty(TX_CAP),
            'from': '0x' + 'ff' * 20, 'to': '0x' + 'ff' * 20, 'nonce': qty(U64), 'gas': qty(GAS_LIMIT), 'value': qty(U256),
            'input': '0x' + 'ff' * input_bytes, 'type': '0x2', 'gasPrice': qty(U256), 'maxPriorityFeePerGas': qty(U256),
            'maxFeePerGas': qty(U256), 'chainId': qty((1 << 53) - 1)}

def block(transactions):
    """block_json: every header field at its widest, with `transactions` (hashes, or tx_json objects)."""
    return {'number': qty(U64), 'hash': word('ff'), 'parentHash': word('ff'), 'nonce': '0x' + 'ff' * 8,
            'sha3Uncles': '0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347', 'logsBloom': '0x' + 'ff' * 256,
            'transactionsRoot': word('ff'), 'stateRoot': word('ff'), 'receiptsRoot': word('ff'), 'miner': '0x' + 'ff' * 20,
            'difficulty': qty(U256), 'totalDifficulty': qty(U256), 'extraData': '0x', 'size': qty(U64), 'gasLimit': qty(GAS_LIMIT),
            'gasUsed': qty(GAS_LIMIT), 'timestamp': qty(U64), 'transactions': transactions, 'uncles': [], 'baseFeePerGas': qty(U256),
            'mixHash': word('00'), 'pocol': {'attempt': U64, 'signed': True, 'proposer': '0x' + 'ff' * 20, 'shares': 4096,
                                             'templateId': word('ff'), 'final': True}}

def light_block():
    return block(['0x%064x' % (U256 - i) for i in range(MAX_TXS)])

def full_block():
    """The largest single-transaction body: one transaction carrying BODY_MAX less its envelope as calldata
    (about 393 KB of JSON)."""
    return block([tx(0, BODY_MAX - 200)])

def members_page():
    # status 1 (the active list holds only ACTIVE records), tomb, ACCRUING, and the widest lastShare and registeredAt.
    w0 = 1 | (1 << 8) | (4 << 16) | (0xffffffff << 24) | (0xffffffff << 56) | (U64 << 88) | (U64 << 152)
    rows = [{'id': '0x' + 'ff' * 20, **gateway.member_w0(w0)} for _ in range(api.MEMBERS_PAGE_MAX)]
    return {'height': U64, 'blockHash': word('ff'), 'active': gateway.MAX_MEMBERS_V3, 'start': 0, 'count': len(rows), 'members': rows}

STATUS = {'chainId': (1 << 53) - 1, 'genesisHash': word('ff'), 'head': {'height': U64, 'hash': word('ff')}, 'pool': 100000,
          'lifetime': {'continuous': True}, 'protocolVersion': 99, 'compatibilitySha256': word('ff'), 'forkSchedule': [[1, '0'], [2, str(U64)], [3, str(U64)]],
          'effectiveNonceMode': 1, 'nextBlockNonceMode': 1, 'consensusCompatibilitySha256': word('ff'), 'networkName': 'x' * 64,
          'networkEnvironment': 'x' * 64, 'peerCount': 128, 'stats': {'reorgs': U64, 'mined': U64, 'imported': U64},
          # The readiness booleans the gateway relays (OWNER-DECISIONS-2026-11-20 D4), at their longest.
          'publicLaunchApproved': False, 'publicLaunchReady': False, 'technicalReadiness': {'complete': False}}

class StubGateway:
    """Gateway.charge and Gateway.call as the relay uses them, answering each call from `answer(method, params)`."""
    def __init__(self, answer):
        self.answer = answer
    def charge(self, cost, client=None, reserve=0.0):
        return True
    def call(self, request, deadline=None):
        return {'jsonrpc': '2.0', 'id': request['id'], 'result': self.answer(request['method'], request['params'])}

def answer(method, params):
    if method in ('pocol_status', 'pocol_getParams'):
        return STATUS
    if method == 'pocol_getMembers':
        return members_page()
    if method == 'eth_getBlockByNumber':
        return full_block() if params[1] else light_block()
    if method == 'eth_getTransactionByHash':
        return tx(1, BODY_MAX - 200)
    raise AssertionError(method)

def forward(calls):
    """The relay's (status, body bytes) for one /api/rpc.php batch."""
    relay = api.Relay(StubGateway(answer), 'http://127.0.0.1:1')
    payload = [{'jsonrpc': '2.0', 'id': i + 1, 'method': m, 'params': p} for i, (m, p) in enumerate(calls)]
    assert api.valid_payload(payload), calls
    status, value, _ = relay.forward(payload)
    return status, (value[1] if value else b'')

class RefreshSize(unittest.TestCase):
    def test_the_bundle_refreshes_with_one_members_page_light_blocks_and_five_transactions(self):
        text = bundle_text()
        self.assertIn('it([[`pocol_status`,[]],[`pocol_getParams`,[]],[`pocol_getMembers`,[0,256]]]),r=Number(e.head.height),'
                      'i=Math.min(20,r+1),a=await it(Array.from({length:i},(e,t)=>[`eth_getBlockByNumber`,[ot(r-t),!1]]))', text)
        self.assertIn('PocolW7H=PocolW7Hashes(a,5),', text)
        self.assertIn('PocolW7H.map(([e])=>[`eth_getTransactionByHash`,[e]])', text)
        self.assertNotIn('pocol_getMiners', text)
        self.assertIsNone(re.search(r'\[ot\([A-Za-z]+-t\),!0\]', text), 'no block list asks for full blocks')

    def test_each_refresh_request_and_all_three_together_stay_under_the_limit(self):
        batches = {'status, params and 256 members': [('pocol_status', []), ('pocol_getParams', []), ('pocol_getMembers', [0, 256])],
                   f'20 light blocks of {MAX_TXS} transaction hashes': [('eth_getBlockByNumber', [hex(U64 - i), False]) for i in range(20)],
                   'the 5 newest transactions, each the largest a block holds': [('eth_getTransactionByHash', ['0x%064x' % i]) for i in range(5)]}
        total = 0
        for name, calls in batches.items():
            with self.subTest(name):
                status, body = forward(calls)
                self.assertEqual(status, 200, name)
                self.assertLess(len(body), api.MAX_REPLY, name)
                total += len(body)
        self.assertLess(total, api.MAX_REPLY, f'all three requests together: {total} bytes')
        self.assertGreater(MAX_TXS, 300, 'the light blocks carry the most hashes the profile allows')

    def test_one_full_block_fits_and_twenty_do_not(self):
        status, body = forward([('eth_getBlockByNumber', ['latest', True])])
        self.assertEqual(status, 200)
        self.assertTrue(380_000 < len(body) < 410_000, f'one full block is about 393 KB of JSON: {len(body)}')
        self.assertLess(len(body), api.MAX_REPLY)
        # Many small transactions instead (the most the gas limit admits, all of BODY_MAX as calldata) fit as well.
        many = block([tx(i, (BODY_MAX - 200) // MAX_TXS) for i in range(MAX_TXS)])
        self.assertLess(len(api.encode(many)), api.MAX_REPLY)
        # The refresh before W7: 20 full blocks in one request. Over the limit, so the relay answers 503 (no data).
        status, body = forward([('eth_getBlockByNumber', [hex(U64 - i), True]) for i in range(20)])
        self.assertEqual(status, 503)
        self.assertGreater(20 * len(api.encode(full_block())), api.MAX_REPLY)

if __name__ == '__main__':
    unittest.main()
