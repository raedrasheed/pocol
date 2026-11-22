"""The explorer snapshot's same-origin reads, served by the explorer gateway (Sites W8, owner decision 9).

The snapshot (deploy/websites/explorer.pocol.net) is a built bundle without sources; it reads /api/rpc.php (POST),
/api/miner-locations.php and /api/transactions.php (GET), operators read /api/health.php, and pocol.net's network
summary (pocol.net/api/network.php) POSTs to https://explorer.pocol.net/api/rpc.php. Those paths were PHP relays in
front of this gateway's /rpc (api/rpc.php, health.php, miner-locations.php, transactions.php and lib.php, retired with
the PHP site). They are answered here, in the gateway process, with the same request checks, method allow-list, size
limits, field filtering, response shapes and status codes; Caddy serves the snapshot's static files on the same host
(deploy/explorer).

Where the in-process answer cannot be the PHP one, the safer behaviour is kept (deploy/rewards/README.md, Explorer
reads): a refusal of the gateway's request budget is each path's busy answer (429, with Retry-After: 3 on
/api/rpc.php only as rpc.php sent it; 503 on transactions) where PHP answered 503 naming its config.php, a request
body without one Content-Length or with Transfer-Encoding is refused, repeated transaction query keys are refused,
health has no PHP-only keys (phpReady, curlReady), and an empty filtered object is {} (PHP encoded []).
The answers carry the security headers the explorer's .htaccess set; Caddy sets the same on the static files.
Standard library only; no network access besides the gateway's own node reads.
"""
import json
import re
import threading
import time
from urllib.parse import unquote_to_bytes, urlsplit

MAX_BODY = 16384                # lib.php POCOL_MAX_BODY
MAX_REPLY = 4 * 1024 * 1024     # lib.php POCOL_MAX_REPLY: the whole answer of one relayed request
MAX_ID = 9007199254740991       # lib.php POCOL_MAX_ID
MEMBERS_START_MAX = 10000       # lib.php POCOL_MEMBERS_START_MAX (the gateway's MAX_MEMBERS_V3)
MEMBERS_PAGE_MAX = 256          # lib.php POCOL_MEMBERS_PAGE_MAX (the gateway's MEMBER_PAGE_MAX)
MAX_BATCH = 20
REQUEST_DEPTH = 32              # rpc.php json_decode($body, false, 32, JSON_BIGINT_AS_STRING)
REPLY_DEPTH = 64                # lib.php json_decode($response, false, 64, JSON_BIGINT_AS_STRING)
FORWARD_SECONDS = 9             # lib.php CURLOPT_TIMEOUT: the deadline of a whole request, every node read included
RELAY_SLOTS = 4                 # lib.php pocol_slot: four non-blocking slots shared by every /api read
MAX_QUERY = 4096                # transactions.php: a query this long is refused (the bundle's are < 300 bytes)
SITE_ORIGIN = 'https://explorer.pocol.net'

# lib.php pocol_valid_call: the reads the explorer and pocol.net's summary may make, nothing else. Narrower than the
# gateway's /rpc (pocol_getBlockReward, pocol_getNetworkGeography and pocol_getTransactions are refused here).
NO_PARAMS = frozenset({'pocol_status', 'pocol_getParams', 'pocol_getMiners', 'eth_blockNumber', 'net_peerCount',
                       'eth_gasPrice', 'pocol_getRemovable', 'pocol_finality', 'pocol_getMemberCounts'})
HASH_METHODS = frozenset({'eth_getTransactionByHash', 'eth_getTransactionReceipt'})
ACCOUNT_METHODS = frozenset({'eth_getBalance', 'eth_getTransactionCount', 'eth_getCode'})
METHODS = NO_PARAMS | HASH_METHODS | ACCOUNT_METHODS | {'pocol_getMembers', 'pocol_getMember', 'eth_getBlockByNumber',
                                                         'eth_getBlockByHash'}

# The explorer's .htaccess (Header always set, on every response of the site, api/ included), verbatim.
CONTENT_SECURITY_POLICY = ("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' "
                           "https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:; "
                           "connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'")
SECURITY_HEADERS = (
    ('X-Content-Type-Options', 'nosniff'),
    ('Referrer-Policy', 'strict-origin-when-cross-origin'),
    ('X-Frame-Options', 'DENY'),
    ('Permissions-Policy', 'camera=(), microphone=(), geolocation=()'),
    ('Content-Security-Policy', CONTENT_SECURITY_POLICY),
)
JSON_TYPE = 'application/json; charset=utf-8'

# path: (methods, the 405 answer's message) as the PHP relays answered.
ROUTES = {
    '/api/rpc.php': (('POST',), 'Use POST for read-only explorer queries.'),
    '/api/health.php': (('GET',), 'Use GET for the connection check.'),
    '/api/miner-locations.php': (('GET', 'HEAD'), 'Use GET to read automatic network geography.'),
    '/api/transactions.php': (('GET', 'HEAD'), 'Use GET to read transaction history.'),
}
BUSY = 'The data service is busy. Please retry shortly.'
REFUSED = 'Only supported, bounded explorer reads are accepted.'
FAILED = 'The PoCol gateway request failed. Check the explorer gateway service and its node.'
INVALID = 'The PoCol gateway returned an invalid response.'
GEOIP_UNAVAILABLE = 'Automatic GeoIP is unavailable. Install or check the GeoIP gateway update on the Linux node.'
TRANSACTIONS_UNAVAILABLE = ('Transaction history is unavailable. Install or check the Transactions gateway update on '
                            'the Linux node.')
GEOGRAPHY_KEYS = ('schema', 'chainId', 'genesisHash', 'generatedAt', 'source', 'scope', 'peerConnections',
                  'excludedConnections')
DATABASE_KEYS = ('provider', 'release', 'updatedAt')
NODE_KEYS = ('id', 'country', 'connections', 'observer', 'miner')
TRANSACTION_KEYS = ('page', 'size', 'order', 'q', 'height', 'hash')
ORDERS = ('newest', 'oldest', 'value_desc', 'value_asc')


def valid_origin(origin):
    """A --site-origin value: https://HOST[:PORT], or http:// on loopback only (a test copy)."""
    m = re.fullmatch(r'(https?)://([a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?)(?::([0-9]{1,5}))?', origin or '')
    if not m or (m.group(3) is not None and not 0 < int(m.group(3)) < 65536):
        return False
    return m.group(1) == 'https' or m.group(2) in ('127.0.0.1', 'localhost')


# ---------------- request decoding (PHP json_decode semantics) ----------------
def _refuse_constant(name):
    raise ValueError('JSON constant ' + name)


def _php_int(text):
    """JSON_BIGINT_AS_STRING: an integer outside the 64-bit range is a string, so a bound check refuses it."""
    if len(text.lstrip('-')) > 19:
        return text
    value = int(text)
    return value if -(1 << 63) <= value < (1 << 63) else text


def nesting_ok(value, depth):
    """Every container nested fewer than `depth` levels (PHP's json_decode depth), every string valid UTF-8 (PHP
    refuses a lone surrogate escape) and no object key starting with NUL (PHP refuses that property name)."""
    stack = [(value, 1)]
    while stack:
        item, level = stack.pop()
        if isinstance(item, (dict, list)):
            if level >= depth:
                return False
            children = item.values() if isinstance(item, dict) else item
            if isinstance(item, dict):
                for key in item:
                    if key.startswith('\x00') or not _utf8(key):
                        return False
            stack.extend((child, level + 1) for child in children)
        elif isinstance(item, str) and not _utf8(item):
            return False
    return True


def _utf8(text):
    try:
        text.encode('utf-8')
        return True
    except UnicodeEncodeError:
        return False


def decode_request(raw):
    """The request body as rpc.php decoded it (json_decode depth 32, JSON_BIGINT_AS_STRING), or ValueError. The depth
    is checked on the text before decoding, so deep nesting is refused without recursion."""
    text = raw.decode('utf-8')  # strict: PHP refuses malformed UTF-8
    level, in_string, escaped = 0, False, False
    for ch in text:
        if in_string:
            if escaped:
                escaped = False
            elif ch == '\\':
                escaped = True
            elif ch == '"':
                in_string = False
        elif ch == '"':
            in_string = True
        elif ch in '[{':
            level += 1
            if level >= REQUEST_DEPTH:
                raise ValueError('JSON nesting')
        elif ch in ']}':
            level -= 1
    value = json.loads(text, parse_constant=_refuse_constant, parse_int=_php_int)
    if not nesting_ok(value, REQUEST_DEPTH):
        raise ValueError('JSON nesting or string')
    return value


# ---------------- lib.php validators ----------------
def _match(pattern, value):
    return isinstance(value, str) and re.fullmatch(pattern, value) is not None


def _hash(value):
    return _match(r'0x[0-9a-fA-F]{64}', value)


def _address(value):
    return _match(r'0x[0-9a-fA-F]{40}', value)


def _tag(value):
    return value == 'latest' or _match(r'0x[0-9a-fA-F]{1,16}', value)


def valid_call(request):
    """lib.php pocol_valid_call: one bounded read of the explorer allow-list."""
    if not isinstance(request, dict) or request.get('jsonrpc') != '2.0':
        return False
    ident, method, params = request.get('id'), request.get('method'), request.get('params')
    if type(ident) is not int or abs(ident) > MAX_ID or not isinstance(method, str) or not isinstance(params, list):
        return False
    if method in NO_PARAMS:
        return len(params) == 0
    if method == 'pocol_getMembers':
        return (len(params) == 2 and type(params[0]) is int and type(params[1]) is int
                and 0 <= params[0] < MEMBERS_START_MAX and 1 <= params[1] <= MEMBERS_PAGE_MAX)
    if method == 'pocol_getMember':
        return len(params) == 1 and _address(params[0])
    if method in HASH_METHODS:
        return len(params) == 1 and _hash(params[0])
    if method in ACCOUNT_METHODS:
        return len(params) == 2 and _address(params[0]) and _tag(params[1])
    if method == 'eth_getBlockByNumber':
        return len(params) == 2 and _tag(params[0]) and type(params[1]) is bool
    if method == 'eth_getBlockByHash':
        return len(params) == 2 and _hash(params[0]) and type(params[1]) is bool
    return False


def valid_payload(value):
    """lib.php pocol_valid_payload: one call or a batch of 1..20 with distinct ids."""
    calls = value if isinstance(value, list) else [value]
    if not 1 <= len(calls) <= MAX_BATCH:
        return False
    seen = set()
    for call in calls:
        if not valid_call(call) or call['id'] in seen:
            return False
        seen.add(call['id'])
    return True


def valid_answers(answers, calls):
    """lib.php pocol_valid_response, for the answers built in process: one per call with its id, jsonrpc 2.0, exactly
    one of result and error, and an error of a 64-bit integer code and a string message."""
    if len(answers) != len(calls):
        return False
    pending = {call['id'] for call in calls}
    for answer in answers:
        if not isinstance(answer, dict) or answer.get('jsonrpc') != '2.0' or type(answer.get('id')) is not int:
            return False
        if answer['id'] not in pending or ('result' in answer) == ('error' in answer):
            return False
        if 'error' in answer:
            error = answer['error']
            if (not isinstance(error, dict) or type(error.get('code')) is not int
                    or not -(1 << 63) <= error['code'] < (1 << 63) or not isinstance(error.get('message'), str)):
                return False
        pending.discard(answer['id'])
    return not pending


# ---------------- miner-locations.php and transactions.php ----------------
def public_geography(value, network):
    """miner-locations.php: the observed-peer geography of this network, reduced to its public fields (in their own
    order); None when it is not that answer."""
    if not isinstance(value, dict):
        return None
    if (value.get('schema') != 'pocol-network-geography-v1' or type(value.get('chainId')) is not int
            or value['chainId'] != network.chain_id or value.get('genesisHash') != network.genesis
            or value.get('source') != 'observed-peer-ip' or value.get('scope') != 'one-node'
            or not isinstance(value.get('nodes'), list) or not isinstance(value.get('database'), dict)):
        return None
    if not all(isinstance(row, dict) for row in value['nodes']):
        return None
    public = {key: value.get(key) for key in GEOGRAPHY_KEYS}
    public['database'] = {k: v for k, v in value['database'].items() if k in DATABASE_KEYS}
    public['nodes'] = [{k: v for k, v in row.items() if k in NODE_KEYS} for row in value['nodes']]
    return public


def transaction_options(query):
    """transactions.php: the pocol_getTransactions options of a query string, or None (400). PHP's defaults, patterns
    and ranges; a key of ours given twice or as an array (page[]=1) is refused, other keys are ignored."""
    if len(query) > MAX_QUERY:
        return None
    found = {}
    for pair in query.split('&') if query else []:
        if not pair:
            continue
        raw_key, _, raw_value = pair.partition('=')
        key = unquote_to_bytes(raw_key.replace('+', ' ')).decode('utf-8', 'replace').lstrip(' ')
        base = key.split('[', 1)[0]
        if base not in TRANSACTION_KEYS:
            continue
        if base != key or base in found:
            return None
        found[base] = unquote_to_bytes(raw_value.replace('+', ' ')).decode('utf-8', 'replace')
    page, size, order, q = (found.get('page', '1'), found.get('size', '25'), found.get('order', 'newest'),
                            found.get('q', ''))
    if (not re.fullmatch(r'[0-9]+', page) or not 1 <= int(page) <= 1000000000 or size not in ('25', '50', '100')
            or order not in ORDERS or (q != '' and not re.fullmatch(r'0x[0-9a-fA-F]{1,64}', q))):
        return None
    options = {'page': int(page), 'size': int(size), 'order': order, 'q': q.lower()}
    if 'height' in found or 'hash' in found:
        height, block = found.get('height'), found.get('hash')
        if height is None or not re.fullmatch(r'[0-9]+', height) or int(height) > MAX_ID or not _hash(block):
            return None
        options['snapshot'] = {'height': int(height), 'hash': block.lower()}
    return options


def transactions_page(value, network, size):
    """transactions.php: the transaction index page of this network with at most `size` rows, or None."""
    if (not isinstance(value, dict) or value.get('schema') != 'pocol-transactions-v1'
            or type(value.get('chainId')) is not int or value['chainId'] != network.chain_id
            or value.get('genesisHash') != network.genesis or not isinstance(value.get('rows'), list)
            or len(value['rows']) > size):
        return None
    return value


# ---------------- the relay ----------------
class Response:
    __slots__ = ('status', 'headers', 'body')

    def __init__(self, status, headers, body=None):
        self.status, self.headers, self.body = status, headers, body


def encode(value):
    """Strict JSON: NaN or Infinity (a node answer json.loads accepted) raise ValueError, as PHP's json_decode of the
    gateway's reply failed on them."""
    return json.dumps(value, separators=(',', ':'), allow_nan=False).encode()


def json_response(status, value, extra=()):
    """lib.php pocol_json: the status, a JSON body and the no-store, nosniff and charset headers (with the site's
    security headers)."""
    return Response(status, [('Content-Type', JSON_TYPE), ('Cache-Control', 'no-store'), *SECURITY_HEADERS, *extra],
                    encode(value))


class Relay:
    """The four /api paths over one Gateway. `transaction_options_valid` is the transaction index's own option check
    (gateway.valid_options) and `cost` the token cost of one call (gateway.COSTS)."""

    def __init__(self, gateway, site_origin=SITE_ORIGIN, *, cost=lambda call: 1, transaction_options_valid=lambda _: False):
        if not valid_origin(site_origin):
            raise ValueError('Invalid site origin')
        self.gateway = gateway
        self.site_origin = site_origin
        self.cost = cost
        self.transaction_options_valid = transaction_options_valid
        self.slots = threading.BoundedSemaphore(RELAY_SLOTS)

    def handle(self, method, target, headers, rfile):
        """The Response to `method target` for an /api path (404 for one not in ROUTES)."""
        parts = urlsplit(target)
        path = parts.path
        if path not in ROUTES:
            return json_response(404, {'error': 'Not found'})
        methods, message = ROUTES[path]
        if method not in methods:
            return json_response(405, {'error': message}, [('Allow', ', '.join(methods))])
        if path == '/api/rpc.php':
            return self.rpc(headers, rfile)
        if path == '/api/health.php':
            return self.health()
        if path == '/api/miner-locations.php':
            return self.miner_locations(method)
        return self.transactions(method, parts.query)

    def forward(self, payload):
        """lib.php pocol_forward, in process: (status, answer, error). A free relay slot, the gateway's token budget,
        then each call through Gateway.call (its cache, upstream slots, sanitizer and error mapping)."""
        calls = payload if isinstance(payload, list) else [payload]
        if not self.slots.acquire(blocking=False):
            return 429, None, BUSY
        try:
            if not self.gateway.charge(sum(self.cost(call) for call in calls)):
                return 429, None, BUSY
            # lib.php's CURLOPT_TIMEOUT was a total deadline: every node read of every call keeps to it (Gateway.query
            # refuses a read once it has passed and bounds each socket wait by the time left), and a request that
            # reaches it is the relay's 503.
            deadline, size, answers = time.monotonic() + FORWARD_SECONDS, 2, []
            for call in calls:
                if time.monotonic() >= deadline:
                    return 503, None, FAILED
                try:
                    answer = self.gateway.call(call, deadline=deadline)
                except Exception:  # an unexpected node answer: the relay's 503, never a dropped connection
                    return 503, None, FAILED
                if time.monotonic() >= deadline:
                    return 503, None, FAILED
                try:
                    size += len(encode(answer)) + 1
                except ValueError:  # NaN or Infinity: lib.php's json_decode failed, pocol_forward answered 502
                    return 502, None, INVALID
                except Exception:
                    return 503, None, FAILED
                if size > MAX_REPLY:
                    return 503, None, FAILED
                answers.append(answer)
            value = answers if isinstance(payload, list) else answers[0]
            if not valid_answers(answers, calls) or not nesting_ok(value, REPLY_DEPTH):
                return 502, None, INVALID
            body = encode(value)
            if len(body) > MAX_REPLY:
                return 503, None, FAILED
            return 200, (value, body), None
        finally:
            self.slots.release()

    def rpc(self, headers, rfile):
        origins = headers.get_all('Origin') or []
        if len(origins) > 1 or (origins and origins[0] != self.site_origin):
            return json_response(403, {'error': 'Cross-origin requests are not enabled.'})
        types = headers.get_all('Content-Type') or ['']
        if len(types) != 1 or types[0].split(';')[0].strip().lower() != 'application/json':
            return json_response(415, {'error': 'Content-Type must be application/json.'})
        lengths = headers.get_all('Content-Length') or []
        if headers.get('Transfer-Encoding') is not None or len(lengths) != 1 or not re.fullmatch(r'[0-9]{1,12}', lengths[0].strip()):
            return json_response(400, {'error': REFUSED})
        size = int(lengths[0])
        if size > MAX_BODY:
            return json_response(413, {'error': 'Request size limit exceeded.'})
        try:
            raw = rfile.read(size)
            if len(raw) != size:
                raise ValueError('short body')
            value = decode_request(raw)
            if not valid_payload(value):
                raise ValueError('not an explorer read')
        except (ValueError, OSError, RecursionError):
            return json_response(400, {'error': REFUSED})
        status, answer, error = self.forward(value)
        if status != 200:
            return json_response(status, {'error': error}, [('Retry-After', '3')] if status == 429 else [])
        return Response(200, [('Content-Type', JSON_TYPE), ('Cache-Control', 'no-store'), *SECURITY_HEADERS], answer[1])

    def health(self):
        status, answer, error = self.forward({'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getParams', 'params': []})
        params = answer[0].get('result') if status == 200 else None
        network = self.gateway.network
        match = (isinstance(params, dict) and type(params.get('chainId')) is int and params['chainId'] == network.chain_id
                 and isinstance(params.get('genesisHash'), str) and params['genesisHash'].lower() == network.genesis)
        return json_response(200 if match else 429 if status == 429 else 503, {
            'service': 'PoCol Explorer',
            'gatewayReachable': status == 200,
            'networkMatch': match,
            'result': 'PASS' if match else 'NOT_CONNECTED',
            'message': ('Expected PoCol network reached. Check head freshness in Explorer separately.' if match
                        else error or 'The gateway does not report the expected PoCol network.'),
        })

    def miner_locations(self, method):
        call = {'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getNetworkGeography', 'params': []}
        status, answer, _ = self.forward(call)
        public = public_geography(answer[0].get('result'), self.gateway.network) if status == 200 else None
        if public is None:
            if method == 'HEAD':
                return Response(503, [('Cache-Control', 'no-store'), *SECURITY_HEADERS])
            return json_response(429 if status == 429 else 503, {'error': GEOIP_UNAVAILABLE})
        if method == 'HEAD':
            return Response(200, [('Content-Type', JSON_TYPE), ('Cache-Control', 'no-store'), *SECURITY_HEADERS])
        return json_response(200, {'data': public})

    def transactions(self, method, query):
        options = transaction_options(query)
        if options is None:
            return json_response(400, {'error': 'Invalid transaction query.'})
        unavailable = json_response(503, {'error': TRANSACTIONS_UNAVAILABLE})
        # The gateway refuses options its transaction index does not take (and every option without the index).
        if not self.transaction_options_valid(options):
            return unavailable
        status, answer, _ = self.forward({'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getTransactions',
                                          'params': [options]})
        reply = answer[0] if status == 200 else {}
        error = reply.get('error')
        if isinstance(error, dict) and type(error.get('code')) is int and error['code'] == -32022:
            return json_response(409, {'error': 'The chain changed. Refreshing transaction history.', 'reset': True})
        data = transactions_page(reply.get('result'), self.gateway.network, options['size'])
        if data is None:
            return unavailable
        if method == 'HEAD':
            return Response(200, [('Content-Type', JSON_TYPE), ('Cache-Control', 'no-store'), *SECURITY_HEADERS])
        return json_response(200, {'data': data})
