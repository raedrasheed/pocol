#!/usr/bin/env python3
"""Record testdata/explorer-php-vectors.json: the retired PHP relays of the explorer snapshot against the gateway's own
/api handlers, on the same fakes (explorer_fakes.py). The PHP files are read from Git at --rev (default e8974fa, the
last commit with the relays), served with `php -S` from a temporary copy whose api/config.php is generated from the
committed explorer-config.example.php (no host config.php is read), in front of this tree's gateway /rpc. Each case
is sent to both; a case whose answers differ must carry a note, and one with a note must differ. Loopback only; needs
php (8.1+, curl) and git.

    python3 -B deploy/rewards/testdata/record_explorer_php_vectors.py [--rev e8974fa] [--out FILE]
"""
import argparse, base64, importlib.util, json, os, shutil, subprocess, sys, tempfile, threading, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REWARDS = HERE.parent
REPO = REWARDS.parents[1]
sys.path.insert(0, str(REWARDS))
import explorer_fakes as F  # noqa: E402

# ---------------- the cases (each: name, scenario, request, optional note and ignored PHP-only keys) ----------------
ORIGIN = 'http://127.0.0.1:8661'
H = '0x' + 'ab' * 32
A = '0x' + 'cd' * 20
def call(method='eth_blockNumber', params=None, ident=1):
    return {'jsonrpc': '2.0', 'id': ident, 'method': method, 'params': [] if params is None else params}
def post(body, origin=None, ctype='application/json', path='/api/rpc.php', extra=(), length=True):
    raw = body if isinstance(body, (bytes, str)) else json.dumps(body)
    raw = raw.encode() if isinstance(raw, str) else raw
    headers = [['Content-Type', ctype]] if ctype is not None else []
    if length: headers.append(['Content-Length', str(len(raw))])
    if origin: headers.append(['Origin', origin])
    headers += [list(h) for h in extra]
    try: text = raw.decode('utf-8'); b = text if text.encode() == raw else {'b64': base64.b64encode(raw).decode()}
    except UnicodeDecodeError: b = {'b64': base64.b64encode(raw).decode()}
    return {'method': 'POST', 'path': path, 'headers': headers, 'body': b}
def get(path, method='GET'):
    return {'method': method, 'path': path, 'headers': []}
def nest(n): return '[' * n + '1' + ']' * n
CASES = []
def case(name, request, scenario=None, note=None, ignore=None):
    c = {'name': name, 'scenario': scenario or {}, 'request': request}
    if note: c['note'] = note
    if ignore: c['ignore'] = ignore
    CASES.append(c)

# ---- /api/rpc.php ----
case('rpc: one read without Origin (server-side caller)', post(call()))
case('rpc: the bundle overview batch with the site Origin', post([call('pocol_status', ident=1), call('pocol_getParams', ident=2), call('pocol_getMiners', ident=3)], ORIGIN))
case('rpc: pocol.net network.php block batch (20 calls)', post([call('eth_getBlockByNumber', [hex(100 - i), False], 4 + i) for i in range(20)]))
case('rpc: the bundle search reads', post([call('eth_getBalance', [A, 'latest'], 1), call('eth_getTransactionCount', [A, 'latest'], 2), call('eth_getCode', [A, '0x10'], 3), call('eth_getTransactionByHash', [H], 4), call('eth_getTransactionReceipt', [H], 5), call('eth_getBlockByHash', [H, True], 6)], ORIGIN))
case('rpc: v3 reads accepted (removable, finality, gas price, peer count)', post([call('pocol_getRemovable', ident=1), call('pocol_finality', ident=2), call('eth_gasPrice', ident=3), call('net_peerCount', ident=4)]))
case('rpc: membership page read (the fake node has no eth_call: a node error answer)', post(call('pocol_getMembers', [0, 256])))
case('rpc: member record read, mixed-case address', post(call('pocol_getMember', ['0x' + 'aB' * 20])))
case('rpc: batch of 21 refused', post([call(ident=i) for i in range(1, 22)]))
case('rpc: duplicate ids refused', post([call(ident=7), call('pocol_status', ident=7)]))
case('rpc: empty batch refused', post([]))
case('rpc: scalar body refused', post('1'))
case('rpc: invalid JSON refused', post('{"jsonrpc":'))
case('rpc: empty body refused', post(''))
for m in ('eth_sendRawTransaction', 'eth_call', 'pocol_getBlockReward', 'pocol_getNetworkGeography', 'pocol_getTransactions', 'pocol_submitShare'):
    case('rpc: method outside the explorer allow-list refused: ' + m, post(call(m, [] if m != 'eth_sendRawTransaction' else ['0x00'])))
case('rpc: eth_call with state overrides refused', post(call('eth_call', [{}, 'latest', {}])))
case('rpc: boolean id refused', post({'jsonrpc': '2.0', 'id': True, 'method': 'eth_blockNumber', 'params': []}))
case('rpc: float id refused', post('{"jsonrpc":"2.0","id":1.0,"method":"eth_blockNumber","params":[]}'))
case('rpc: id above 2^53-1 refused', post(call(ident=2 ** 53)))
case('rpc: id -(2^53-1) accepted', post(call(ident=-(2 ** 53 - 1))))
case('rpc: params object refused', post({'jsonrpc': '2.0', 'id': 1, 'method': 'eth_blockNumber', 'params': {}}))
case('rpc: oversized integer page bound refused', post('{"jsonrpc":"2.0","id":1,"method":"pocol_getMembers","params":[0,99999999999999999999]}'))
case('rpc: 30-digit integer in an ignored field accepted', post('{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[],"x":123456789012345678901234567890}'))
case('rpc: 31 nested arrays accepted', post('{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[],"x":' + nest(30) + '}'))
case('rpc: 32 nested arrays refused', post('{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[],"x":' + nest(31) + '}'))
case('rpc: 8000 nested arrays refused', post(nest(8000)))
case('rpc: NaN refused', post('{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[],"x":NaN}'))
case('rpc: lone surrogate escape refused', post('{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[],"x":"\\ud800"}'))
case('rpc: UTF-8 byte order mark refused', post(b'\xef\xbb\xbf' + json.dumps(call()).encode()))
case('rpc: malformed UTF-8 refused', post(b'{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[],"x":"\xff"}'))
case('rpc: foreign Origin refused', post(call(), 'https://evil.example'))
case('rpc: empty Origin refused', post(call(), extra=[('Origin', '')]))
case('rpc: GET refused with Allow: POST', get('/api/rpc.php'))
case('rpc: OPTIONS refused (no CORS)', get('/api/rpc.php', 'OPTIONS'))
case('rpc: PUT refused', {'method': 'PUT', 'path': '/api/rpc.php', 'headers': [['Content-Length', '0']], 'body': ''})
case('rpc: text/plain refused', post(call(), ctype='text/plain'))
case('rpc: no Content-Type refused', post(call(), ctype=None))
case('rpc: JSON media type with parameters accepted', post(call(), ctype=' Application/JSON ; charset=utf-8'))
pad = json.dumps(call())
case('rpc: 16384-byte body accepted', post(pad + ' ' * (16384 - len(pad))))
case('rpc: 16385-byte body refused', post(pad + ' ' * (16385 - len(pad))))
case('rpc: a node error answer is relayed', post(call('eth_getBalance', [A, 'latest'])), {'node': {'eth_getBalance': {'error': {'code': -32000, 'message': 'state pruned'}}}})
case('rpc: an unavailable node is an error answer', post(call()), {'node': {'eth_blockNumber': 'fail'}})
case('rpc: an answer without result or error is an invalid response', post(call()), {'node': {'eth_blockNumber': {'envelope': {'jsonrpc': '2.0', 'id': 1}}}})
case('rpc: an answer of another protocol version is an invalid response', post(call()), {'node': {'eth_blockNumber': {'envelope': {'jsonrpc': '1.0', 'id': 1, 'result': '0x1'}}}})
case('rpc: an untyped error is an invalid response', post(call()), {'node': {'eth_blockNumber': {'error': {'code': '1', 'message': 'x'}}}})
case('rpc: an answer with result and error is an invalid response', post(call()), {'node': {'eth_blockNumber': {'result': '0x1', 'error': {'code': 1, 'message': 'x'}}}})
case('rpc: answers above 4 MiB', post([call('eth_getCode', [A, 'latest'], 1), call('eth_getCode', [A, '0x1'], 2)]), {'node': {'eth_getCode': {'big': 2200000}}},
     note='PHP answered 503 with a message naming its api/config.php; the gateway answers 503 with a message naming its own service.')
case('rpc: the request budget is spent', post(call()), {'tokens': 0},
     note='PHP turned the gateway\'s 429 into 503 with its config.php message; the in-process relay answers 429 with Retry-After: 3 and the busy message PHP used for its own slots.')
case('rpc: two Origin headers', post(call(), ORIGIN, extra=[('Origin', 'https://evil.example')]),
     note='php -S kept one of the two; the gateway refuses more than one Origin header (403).')
case('rpc: chunked request body', {'method': 'POST', 'path': '/api/rpc.php', 'headers': [['Content-Type', 'application/json'], ['Transfer-Encoding', 'chunked']],
     'body': '%x\r\n%s\r\n0\r\n\r\n' % (len(pad), pad)},
     note='PHP read a chunked body; the gateway requires one Content-Length (400), as its /rpc does. Every client sends Content-Length.')
case('api: config.php is not served', get('/api/config.php'),
     note='php -S executes the host file (Apache denied it with api/.htaccess); on the gateway host it does not exist and /api/* has only the four paths (404).')
case('api: lib.php is not served', get('/api/lib.php'),
     note='php -S executes it (Apache denied it); the gateway answers 404.')
case('api: another /api path is not found', get('/api/index.php'), note='php -S answers its HTML 404 page; the gateway answers a JSON 404.')

# ---- /api/health.php ----
IGN = ['phpReady', 'curlReady']
HN = 'health keeps service, gatewayReachable, networkMatch, result and message; the PHP-only phpReady and curlReady are not sent.'
case('health: the configured network', get('/api/health.php'), ignore=IGN)
case('health: the network hash in upper case still matches', get('/api/health.php'), {'node': {'pocol_getParams': {'result': {'chainId': 777963, 'genesisHash': '0x' + '5A' * 32}}}}, ignore=IGN)
case('health: another chain ID', get('/api/health.php'), {'node': {'pocol_getParams': {'result': {'chainId': 1, 'genesisHash': '0x' + '5a' * 32}}}}, ignore=IGN)
case('health: a string chain ID', get('/api/health.php'), {'node': {'pocol_getParams': {'result': {'chainId': '777963', 'genesisHash': '0x' + '5a' * 32}}}}, ignore=IGN)
case('health: another genesis', get('/api/health.php'), {'node': {'pocol_getParams': {'result': {'chainId': 777963, 'genesisHash': '0x' + '5b' * 32}}}}, ignore=IGN)
case('health: node unavailable (the gateway answers an error)', get('/api/health.php'), {'node': {'pocol_getParams': 'fail'}}, ignore=IGN)
case('health: POST refused with Allow: GET', post(call(), path='/api/health.php'))
case('health: HEAD refused', get('/api/health.php', 'HEAD'))
case('health: query string ignored', get('/api/health.php?x=1'), ignore=IGN)
case('health: the request budget is spent', get('/api/health.php'), {'tokens': 0}, ignore=IGN,
     note='PHP answered 503 (gatewayReachable false, its config.php message); the relay answers 429 with the busy message.')

# ---- /api/miner-locations.php ----
G = '/api/miner-locations.php'
case('geography: the public fields only', get(G))
case('geography: HEAD', get(G, 'HEAD'), note='The gateway also sends nosniff on a bodiless HEAD answer (php -S ignores .htaccess, whose Header always set added it on Apache).')
case('geography: query string ignored', get(G + '?refresh=1'))
case('geography: POST refused with Allow: GET, HEAD', post(call(), path=G))
case('geography: another schema', get(G), {'geography': {'changes': {'schema': 'pocol-network-geography-v2'}}})
case('geography: another chain ID', get(G), {'geography': {'changes': {'chainId': 1}}})
case('geography: a string chain ID', get(G), {'geography': {'changes': {'chainId': '777963'}}})
case('geography: the genesis hash in upper case', get(G), {'geography': {'changes': {'genesisHash': '0x' + '5A' * 32}}})
case('geography: another source', get(G), {'geography': {'changes': {'source': 'configured'}}})
case('geography: another scope', get(G), {'geography': {'changes': {'scope': 'network'}}})
case('geography: nodes not a list', get(G), {'geography': {'changes': {'nodes': {'a': 1}}}})
case('geography: database not an object', get(G), {'geography': {'changes': {'database': ['DB-IP Lite']}}})
case('geography: a node that is not an object', get(G), {'geography': {'changes': {'nodes': [{'id': 'ip-' + 'a' * 24}, 5]}}})
case('geography: missing excludedConnections and peerConnections are null', get(G), {'geography': {'changes': {}, 'remove': ['excludedConnections', 'peerConnections']}})
case('geography: no nodes', get(G), {'geography': {'changes': {'nodes': []}}})
case('geography: database without public fields', get(G), {'geography': {'changes': {'database': {'path': '/x'}}}},
     note='PHP encoded the empty filtered object as []; the gateway sends {}.')
case('geography: a node without public fields', get(G), {'geography': {'changes': {'nodes': [{'ip': '203.0.113.1'}]}}},
     note='PHP encoded the empty filtered row as []; the gateway sends {}.')
case('geography: no GeoIP module', get(G), {'geography': None})
case('geography: no GeoIP module, HEAD', get(G, 'HEAD'), {'geography': None},
     note='php -S added its default text/html Content-Type to the bodiless 503; the gateway sends no Content-Type.')
case('geography: the request budget is spent', get(G), {'tokens': 0},
     note='PHP answered 503 (the gateway\'s 429 became its 503); the relay answers 429, as PHP did for its own busy slots.')

# ---- /api/transactions.php ----
T = '/api/transactions.php'
case('transactions: defaults', get(T))
case('transactions: the bundle query', get(T + '?page=2&size=50&order=oldest&q=0xABcd'))
case('transactions: an empty q', get(T + '?page=1&size=100&order=value_desc&q='))
case('transactions: a snapshot', get(T + '?page=1&size=25&order=newest&q=&height=16&hash=0x' + 'AB' * 32))
case('transactions: the largest snapshot height', get(T + '?height=9007199254740991&hash=' + H))
case('transactions: leading zeros', get(T + '?page=007&height=0016&hash=' + H))
case('transactions: the largest page', get(T + '?page=1000000000'))
case('transactions: a key with a leading space', get(T + '?+page=3'))
case('transactions: an unknown key ignored', get(T + '?page=2&lang=en'))
for q in ('page=0', 'page=1000000001', 'page=-1', 'page=1.5', 'page=', 'page=1%0A', 'size=30', 'size=', 'order=random', 'q=0x', 'q=ab', 'q=0x' + 'a' * 65,
          'q=0x+ab', 'height=16', 'hash=' + H, 'height=&hash=' + H, 'height=9007199254740992&hash=' + H, 'height=16&hash=0x12', 'page[]=1', 'q[0]=0x1', 'height[]=1&hash=' + H):
    case('transactions: refused ' + q, get(T + '?' + q))
case('transactions: a repeated key', get(T + '?page=1&page=2'),
     note='PHP took the last value; the gateway refuses a repeated key of the query (400). The bundle sends each key once.')
case('transactions: more rows than the page size', get(T + '?size=25'), {'transactions': {'rows': 26}})
case('transactions: exactly the page size', get(T + '?size=25'), {'transactions': {'rows': 25}})
case('transactions: another schema', get(T), {'transactions': {'schema': 'pocol-transactions-v2'}})
case('transactions: another chain ID', get(T), {'transactions': {'chainId': 1}})
case('transactions: the genesis hash in upper case', get(T), {'transactions': {'genesisHash': '0x' + '5A' * 32}})
case('transactions: rows not a list', get(T), {'transactions': {'rows': {'not': 'a list'}}})
case('transactions: the chain moved', get(T), {'transactions': 'moved'})
case('transactions: the chain moved, HEAD', get(T, 'HEAD'), {'transactions': 'moved'})
case('transactions: HEAD', get(T, 'HEAD'), note='The gateway also sends nosniff on a bodiless HEAD answer (php -S ignores .htaccess, whose Header always set added it on Apache).')
case('transactions: no transaction index', get(T), {'transactions': 'none'})
case('transactions: options the index refuses', get(T), {'txOptionsValid': False})
case('transactions: POST refused with Allow: GET, HEAD', post(call(), path=T))
case('transactions: the request budget is spent', get(T), {'tokens': 0})


def materialize(rev, target):
    """The explorer snapshot and the config example at `rev`, from Git blobs (never a host docroot)."""
    names = subprocess.run(['git', '-C', str(REPO), 'ls-tree', '-r', '--name-only', rev, '--', 'deploy/websites/explorer.pocol.net'],
                           check=True, capture_output=True, text=True).stdout.split()
    if 'deploy/websites/explorer.pocol.net/api/rpc.php' not in names:
        raise SystemExit(f'{rev} has no PHP relays')
    for name in names + ['deploy/websites/explorer-config.example.php']:
        if name.endswith('/config.php'):
            raise SystemExit('a config.php is tracked at ' + rev)
        path = target / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(subprocess.run(['git', '-C', str(REPO), 'show', f'{rev}:{name}'], check=True, capture_output=True).stdout)
    return target / 'deploy/websites/explorer.pocol.net', (target / 'deploy/websites/explorer-config.example.php').read_text()


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--rev', default='e8974fa')
    ap.add_argument('--out', default=str(HERE / 'explorer-php-vectors.json'))
    args = ap.parse_args()
    spec = importlib.util.spec_from_file_location('recorded_gateway', REWARDS / 'gateway.py')
    g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
    node = F.FakeNode(); geo = F.FakeGeography(); tx = F.FakeTransactions()
    gw = g.Gateway(node.server_address[1], geo, tx, network=F.NET)
    server = g.Server(('127.0.0.1', 0), gw, ORIGIN); threading.Thread(target=server.serve_forever, daemon=True).start()
    gport = server.server_address[1]
    work = Path(tempfile.mkdtemp(prefix='pocol-explorer-php-'))
    php = None
    try:
        site, example = materialize(args.rev, work / 'tree')
        config = (example.replace("'http://127.0.0.1:8088/rpc'", f"'http://127.0.0.1:{gport}/rpc'").replace("'https://explorer.pocol.net'", f"'{ORIGIN}'")
                  .replace("'chain_id' => 0", f"'chain_id' => {F.NET['chainId']}")
                  .replace("'genesis_hash' => '0x" + '0' * 64 + "'", f"'genesis_hash' => '{F.NET['genesisHash']}'"))
        if not all(s in config for s in (f'127.0.0.1:{gport}/rpc', ORIGIN, str(F.NET['chainId']), F.NET['genesisHash'])):
            raise SystemExit('the config example changed: not every key was replaced')
        (site / 'api' / 'config.php').write_text(config)
        (work / 'tmp').mkdir()
        php = subprocess.Popen(['php', '-S', ORIGIN.split('//')[1], '-t', str(site)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               env={**os.environ, 'PHP_CLI_SERVER_WORKERS': '4', 'TMPDIR': str(work / 'tmp')})
        php_port = int(ORIGIN.rsplit(':', 1)[1])
        for _ in range(100):
            try: F.send(php_port, 'GET', '/index.html'); break
            except OSError: time.sleep(0.1)
        vectors, unexpected = [], 0
        for c in CASES:
            out = {}
            for side, port in (('php', php_port), ('python', gport)):
                F.apply_scenario(g, gw, node, geo, tx, c['scenario'])
                g.valid_options = (lambda flag: (lambda o: flag))(c['scenario'].get('txOptionsValid', True))
                out[side] = F.run_case(port, c['request'])
            php_cmp = json.loads(json.dumps(out['php']))
            if isinstance(php_cmp['body'], dict):
                for k in c.get('ignore', []): php_cmp['body'].pop(k, None)
            if (php_cmp == out['python']) == ('note' in c):
                unexpected += 1
                print('UNEXPECTED', c['name'], json.dumps(out)[:800], file=sys.stderr)
            vectors.append({**c, 'php': out['php'], 'python': out['python']})
        if unexpected:
            raise SystemExit(f'{unexpected} cases differ from their notes; nothing written')
        Path(args.out).write_text(json.dumps({'about': 'Recorded by record_explorer_php_vectors.py from the PHP relays at ' + args.rev
                                              + '; replayed by test_explorer_api.py', 'rev': args.rev, 'origin': ORIGIN,
                                              'cases': vectors}, indent=1, ensure_ascii=False) + '\n')
        print(f'{len(vectors)} cases recorded, every difference noted')
    finally:
        if php: php.terminate(); php.wait()
        server.shutdown(); server.server_close(); node.stop(); shutil.rmtree(work)


if __name__ == '__main__':
    main()
