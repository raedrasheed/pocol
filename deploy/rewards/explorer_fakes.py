"""Test doubles for the explorer /api tests (test_explorer_api.py): a loopback JSON-RPC node, the optional GeoIP and
transaction index modules, and an HTTP client that sends exactly the headers it is given. Loopback only."""
import base64
import copy
import http.client
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# A trusted network configuration (--v3-network), as in test_gateway_rewards.py.
NET = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32, 'specVersion': 2, 'v3': True}
HASH = '0x' + 'ab' * 32
ADDRESS = '0x' + 'cd' * 20


def default_answers():
    """{method: answer fields}: the node's answer to each read, without jsonrpc and id."""
    block = {'number': '0x10', 'hash': HASH, 'parentHash': '0x' + '01' * 32, 'timestamp': '0x65000000', 'transactions': []}
    return {
        'pocol_status': {'result': {'chainId': NET['chainId'], 'genesisHash': NET['genesisHash'],
                                    'head': {'height': 16, 'hash': HASH}, 'pool': 0, 'peers': [{}, {}],
                                    'stats': {'reorgs': 0, 'mined': 3, 'imported': 13}, 'private': 'dropped'}},
        'pocol_getParams': {'result': {'chainId': NET['chainId'], 'genesisHash': NET['genesisHash'], 'protocolVersion': 2,
                                       'compatibilitySha256': '0x' + '11' * 32}},
        'pocol_getMiners': {'result': [{'id': ADDRESS, 'reward': ADDRESS, 'status': 1, 'tomb': False, 'bondState': 1}]},
        'eth_blockNumber': {'result': '0x10'},
        'net_peerCount': {'result': '0x2'},
        'eth_gasPrice': {'result': '0x3b9aca00'},
        'pocol_getRemovable': {'result': []},
        'pocol_finality': {'result': {'height': 10}},
        'eth_getBalance': {'result': '0xde0b6b3a7640000'},
        'eth_getTransactionCount': {'result': '0x1'},
        'eth_getCode': {'result': '0x'},
        'eth_getBlockByNumber': {'result': block},
        'eth_getBlockByHash': {'result': block},
        'eth_getTransactionByHash': {'result': None},
        'eth_getTransactionReceipt': {'result': None},
    }


class FakeNode(ThreadingHTTPServer):
    """A pocold JSON-RPC endpoint on 127.0.0.1: `answers[method]` is the answer's fields ({'result': ..} or
    {'error': ..}), the whole envelope when it has 'envelope', the body bytes when it has 'raw', or 'fail' for an
    HTTP 500. `delay` seconds pass before each answer."""
    daemon_threads = True

    def __init__(self):
        self.answers = default_answers()
        self.calls = []
        self.delay = 0
        super().__init__(('127.0.0.1', 0), NodeHandler)
        self.thread = threading.Thread(target=self.serve_forever, daemon=True)
        self.thread.start()

    def reset(self):
        self.answers = default_answers()
        self.calls = []
        self.delay = 0

    def stop(self):
        self.shutdown(); self.server_close(); self.thread.join()


class NodeHandler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_args):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        self.server.calls.append((request['method'], request['params']))
        if self.server.delay:
            time.sleep(self.server.delay)
        answer = self.server.answers.get(request['method'], {'error': {'code': -32601, 'message': 'Method not found'}})
        if answer == 'fail':
            self.send_response(500); self.send_header('Content-Length', '0'); self.end_headers(); return
        answer = copy.deepcopy(answer)
        if 'raw' in answer:
            body = answer['raw']
        else:
            envelope = answer['envelope'] if 'envelope' in answer else {'jsonrpc': '2.0', 'id': request['id'], **answer}
            body = json.dumps(envelope).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def geography(**changes):
    """An answer of the GeoIP module (pocol-network-geography-v1) with private fields the relay drops."""
    value = {'schema': 'pocol-network-geography-v1', 'chainId': NET['chainId'], 'genesisHash': NET['genesisHash'],
             'generatedAt': 1700000000, 'source': 'observed-peer-ip', 'scope': 'one-node', 'peerConnections': 3,
             'excludedConnections': 1, 'observerAddress': '203.0.113.9',
             'database': {'path': '/var/lib/geoip/dbip.mmdb', 'provider': 'DB-IP Lite', 'release': '2026-11',
                          'updatedAt': 1699999999, 'licenseKey': 'private'},
             'nodes': [{'id': 'ip-' + 'a' * 24, 'ip': '203.0.113.9', 'country': 'US', 'connections': 0, 'observer': True,
                        'miner': '0x' + 'ef' * 20, 'asn': 64500},
                       {'country': None, 'connections': 2, 'observer': False, 'miner': None, 'id': 'ip-' + 'b' * 24,
                        'port': 30300}]}
    value.update(changes)
    return value


class FakeGeography:
    def __init__(self):
        self.value = geography()

    def snapshot(self, raw_status):
        if isinstance(self.value, Exception):
            raise self.value
        return copy.deepcopy(self.value)


def transactions_page(options, rows=None, **changes):
    value = {'schema': 'pocol-transactions-v1', 'chainId': NET['chainId'], 'genesisHash': NET['genesisHash'],
             'page': options['page'], 'size': options['size'], 'order': options['order'], 'q': options['q'], 'total': 1,
             'index': {'height': 16, 'head': 16, 'updatedAt': 1700000000, 'state': 'ready'},
             'snapshot': options.get('snapshot', {'height': 16, 'hash': HASH}),
             'rows': [{'hash': HASH, 'blockHash': HASH, 'blockNumber': '0x10', 'transactionIndex': '0x0',
                       'timestamp': '0x65000000', 'value': '0x1', 'kind': 'Transfer', 'from': ADDRESS, 'to': ADDRESS}]
             if rows is None else rows}
    value.update(changes)
    return value


class FakeTransactions:
    """The transaction index: `page(options)` answers `self.answer(options)`, or raises it when it is an exception
    class (gateway.SnapshotChanged for a moved chain)."""
    def __init__(self):
        self.answer = transactions_page
        self.asked = []

    def page(self, options):
        self.asked.append(options)
        if isinstance(self.answer, type) and issubclass(self.answer, Exception):
            raise self.answer('moved')
        return self.answer(options)


def send(port, method, path, headers=(), body=None, timeout=10):
    """(status, [(header, value)], body bytes) of one request with exactly `headers` (no automatic Content-Length)."""
    conn = http.client.HTTPConnection('127.0.0.1', port, timeout=timeout)
    try:
        conn.putrequest(method, path, skip_accept_encoding=True)
        for name, value in headers:
            conn.putheader(name, value)
        conn.endheaders(body)
        response = conn.getresponse()
        return response.status, response.getheaders(), response.read()
    finally:
        conn.close()


def post_json(port, path, value, origin=None, extra=()):
    body = value if isinstance(value, bytes) else json.dumps(value).encode()
    headers = [('Content-Type', 'application/json'), ('Content-Length', str(len(body))), *extra]
    if origin is not None:
        headers.append(('Origin', origin))
    return send(port, 'POST', path, headers, body)


# ---------------- recorded cases (testdata/explorer-php-vectors.json) ----------------
COMPARED_HEADERS = ('Content-Type', 'Cache-Control', 'X-Content-Type-Options', 'Allow', 'Retry-After')


def apply_scenario(gateway_module, gateway, node, geo, tx, scenario):
    """Set the fakes and the gateway's state for one recorded case. `scenario` keys: node ({method: answer}, an answer
    {'big': N} is a result of N hex digits), geography (an object, null for no GeoIP module, or changes to the default
    under 'changes'), transactions ('none' for no index, 'moved' for SnapshotChanged, or changes of the page with
    'rows' a row count), tokens (the request budget left), txOptionsValid (the index's option check, default true;
    the caller applies it to gateway.valid_options)."""
    node.reset()
    for method, answer in scenario.get('node', {}).items():
        if isinstance(answer, dict) and 'big' in answer:
            answer = {'result': '0x' + 'a' * answer['big']}
        node.answers[method] = answer
    gateway.geography = geo
    geo.value = geography()
    if 'geography' in scenario:
        value = scenario['geography']
        if value is None:
            gateway.geography = None
        elif 'changes' in value:
            geo.value = geography(**value['changes'])
            for key in value.get('remove', []):
                geo.value.pop(key, None)
        else:
            geo.value = value
    gateway.transactions = tx
    tx.answer = transactions_page
    value = scenario.get('transactions')
    if value == 'none':
        gateway.transactions = None
    elif value == 'moved':
        tx.answer = gateway_module.SnapshotChanged
    elif isinstance(value, dict):
        rows = value.get('rows')
        changes = {k: v for k, v in value.items() if not (k == 'rows' and type(v) is int)}
        tx.answer = lambda options: transactions_page(
            options, [dict(transactions_page(options)['rows'][0], hash='0x' + f'{i:064x}') for i in range(rows)]
            if type(rows) is int else changes.pop('rows', None), **changes)
    with gateway.lock:
        gateway.cache.clear(); gateway.cache_size = 0
        gateway.tokens = float(scenario.get('tokens', 80)); gateway.token_time = time.monotonic()
        gateway.member_cache.clear(); gateway.draw_cache.clear()


def request_body(request):
    body = request.get('body')
    if body is None:
        return None
    if isinstance(body, dict):
        return base64.b64decode(body['b64'])
    return body.encode('utf-8')


def run_case(port, request):
    """The normalized answer to a recorded request: status, the compared headers and the body (JSON, or text)."""
    status, headers, body = send(port, request['method'], request['path'], [tuple(h) for h in request.get('headers', [])],
                                 request_body(request))
    seen = {}
    for name, value in headers:
        for wanted in COMPARED_HEADERS:
            if name.lower() == wanted.lower():
                seen.setdefault(wanted, []).append(value)
    try:
        parsed = json.loads(body) if body else None
    except ValueError:
        parsed = body.decode('utf-8', 'replace')
    return {'status': status, 'headers': {k: seen[k] for k in COMPARED_HEADERS if k in seen}, 'body': parsed}
