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

The holders and miner map reads (/api/holders.php, /api/holders-stats.php, /api/miner-geography.php, GET and HEAD)
and the operator node reports (POST /api/geo-report.php) never had a PHP relay; they follow
development/explorer/MINERS-MAP-HOLDERS.md §4.1 and §5 (strict queries, every answer checked to the contract's exact
fields, 429 with Retry-After, 409 reset, 503 without data).
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
    # The holders and miner map reads and the operator node reports (MINERS-MAP-HOLDERS.md §4.1, §5): no PHP relay
    # ever answered these paths; their answers follow the contract (429 with Retry-After, 409 reset, 503 without data).
    '/api/holders.php': (('GET', 'HEAD'), 'Use GET to read PCOL holders.'),
    '/api/holders-stats.php': (('GET', 'HEAD'), 'Use GET to read the PCOL holder statistics.'),
    '/api/miner-geography.php': (('GET', 'HEAD'), 'Use GET to read the miner map.'),
    '/api/geo-report.php': (('POST',), 'Use POST to send a signed operator node report.'),
}
BUSY = 'The data service is busy. Please retry shortly.'
REFUSED = 'Only supported, bounded explorer reads are accepted.'
FAILED = 'The PoCol gateway request failed. Check the explorer gateway service and its node.'
INVALID = 'The PoCol gateway returned an invalid response.'
# miner-locations.php named a GeoIP gateway update that does not exist (deploy/explorer/README.md, Miner map); the
# gateway says what is true whether or not a geography module is installed (a documented difference from PHP).
GEOIP_UNAVAILABLE = 'Miner locations are unavailable: the PoCol network does not record where miners are.'
TRANSACTIONS_UNAVAILABLE = ('Transaction history is unavailable. Install or check the Transactions gateway update on '
                            'the Linux node.')
GEOGRAPHY_KEYS = ('schema', 'chainId', 'genesisHash', 'generatedAt', 'source', 'scope', 'peerConnections',
                  'excludedConnections')
DATABASE_KEYS = ('provider', 'release', 'updatedAt')
NODE_KEYS = ('id', 'country', 'connections', 'observer', 'miner')
TRANSACTION_KEYS = ('page', 'size', 'order', 'q', 'height', 'hash')
ORDERS = ('newest', 'oldest', 'value_desc', 'value_asc')
# GET /api/holders.php (§5.1): the query keys, orders and account types (pocol_holders.QUERY_KEYS, ORDERS, TYPES).
HOLDER_KEYS = ('page', 'size', 'order', 'type', 'q', 'height', 'hash')
HOLDER_ORDERS = ('balance_desc', 'balance_asc')
HOLDER_TYPES = ('all', 'wallet', 'miner', 'reward', 'contract', 'system')
HOLDER_LABELS = ('MinerRegistry', 'RewardEscrow', 'ShareLedger', 'ChainCaps', 'ChunkFactory', 'PAY', 'SYSTEM')
HOLDERS_UNAVAILABLE = 'PCOL holders are unavailable: the holders index has no snapshot yet or is not installed.'
HOLDERS_RESET = 'The holder snapshot changed. Refreshing.'
MAP_UNAVAILABLE = 'The miner map is unavailable: the holders index has no registry snapshot yet or is not installed.'
# POST /api/geo-report.php (§4.1; pocol_geoip.REPORT_ERRORS, MAX_REPORT_BYTES).
MAX_REPORT = 16384
REPORT_ERRORS = {400: 'The report is malformed.', 401: 'Unknown reporter or bad signature.',
                 409: 'The report is stale or replayed.', 413: 'Request size limit exceeded.',
                 415: 'Content-Type must be application/json.', 503: 'Operator reports are not enabled.'}
# The exact keys of each answer: a field the backend added (an address, a keyed id) never reaches a public answer.
HOLDERS_PAGE_KEYS = frozenset({'schema', 'chainId', 'genesisHash', 'page', 'size', 'order', 'type', 'q', 'total',
                               'snapshot', 'index', 'supply', 'verification', 'rows'})
HOLDER_ROW_KEYS = frozenset({'rank', 'address', 'type', 'label', 'minerIds', 'rewardAddress', 'totalWei', 'availableWei',
                             'bondedWei', 'sharePercentE10', 'transactions', 'blocksMined', 'rewardsWei'})
STATS_KEYS = frozenset({'schema', 'chainId', 'genesisHash', 'snapshot', 'index', 'supply', 'verification', 'holders',
                        'concentration', 'distribution', 'history', 'rewards'})
SUPPLY_KEYS = ('genesisWei', 'issuedWei', 'mintedWei', 'baseFeeBurnedWei', 'protocolBurnedWei', 'totalBurnedWei',
               'currentSupplyWei', 'circulatingWei', 'availableWei', 'bondedWei', 'systemWei', 'pendingRewardsWei')
GEOGRAPHY_V2_KEYS = frozenset({'schema', 'chainId', 'genesisHash', 'generatedAt', 'height', 'blockHash', 'database',
                               'miners', 'nodes', 'sources', 'blocksWindow', 'countries', 'unknown'})
MAP_MINER_KEYS = frozenset({'registered', 'active', 'inactive', 'exited', 'punished', 'located', 'unknown', 'connected',
                            'thetaInact'})
COUNTRY_KEYS = frozenset({'code', 'miners', 'nodes', 'blocks', 'blocksSharePercentE10', 'rewardsWei'})


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


# ---------------- holders.php, holders-stats.php and miner-geography.php (§5) ----------------
_DECIMAL = re.compile(r'0|[1-9][0-9]{0,77}')
_SIGNED = re.compile(r'0|-?[1-9][0-9]{0,77}')
_LOWER_ADDRESS = re.compile(r'0x[0-9a-f]{40}')
_LOWER_HASH = re.compile(r'0x[0-9a-f]{64}')
_REPORTER = re.compile(r'[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?')   # pocol_geoip.REPORTER_NAME


def holder_options(query):
    """holders.php: the pocol_getHolders options of a query string, or None (400). Defaults page 1, size 25,
    balance_desc, type all, no search; page 1..10^9, size 25/50/100, q a 0x address prefix of 1..40 hex digits
    (lower-cased), height and hash only together. A key of ours given twice or as an array (page[]=1) is refused, other
    keys are ignored (the transaction_options rules; pocol_holders.holder_options is the same function)."""
    if len(query) > MAX_QUERY:
        return None
    found = {}
    for pair in query.split('&') if query else []:
        if not pair:
            continue
        raw_key, _, raw_value = pair.partition('=')
        key = unquote_to_bytes(raw_key.replace('+', ' ')).decode('utf-8', 'replace').lstrip(' ')
        base = key.split('[', 1)[0]
        if base not in HOLDER_KEYS:
            continue
        if base != key or base in found:
            return None
        found[base] = unquote_to_bytes(raw_value.replace('+', ' ')).decode('utf-8', 'replace')
    page, size, order, kind, q = (found.get('page', '1'), found.get('size', '25'), found.get('order', 'balance_desc'),
                                  found.get('type', 'all'), found.get('q', ''))
    if (not re.fullmatch(r'[0-9]+', page) or not 1 <= int(page) <= 1000000000 or size not in ('25', '50', '100')
            or order not in HOLDER_ORDERS or kind not in HOLDER_TYPES or (q != '' and not re.fullmatch(r'0x[0-9a-fA-F]{1,40}', q))):
        return None
    options = {'page': int(page), 'size': int(size), 'order': order, 'type': kind, 'q': q.lower()}
    if 'height' in found or 'hash' in found:
        height, block = found.get('height'), found.get('hash')
        if height is None or not re.fullmatch(r'[0-9]+', height) or int(height) > MAX_ID or not _hash(block):
            return None
        options['snapshot'] = {'height': int(height), 'hash': block.lower()}
    return options


def plain_query(query):
    """holders-stats.php and miner-geography.php take no parameter: any query of at most MAX_QUERY bytes is ignored
    (a cache-busting key), a longer one is refused (400)."""
    return len(query) <= MAX_QUERY


def _n(value, low=0, high=MAX_ID):
    return type(value) is int and low <= value <= high


def _n_or_null(value, low=0):
    return value is None or _n(value, low)


def _wei(value, pattern=_DECIMAL):
    return isinstance(value, str) and pattern.fullmatch(value) is not None


def _keys(value, keys):
    return isinstance(value, dict) and set(value) == set(keys)


def _identity(value, schema, network):
    return (isinstance(value, dict) and value.get('schema') == schema and type(value.get('chainId')) is int
            and value['chainId'] == network.chain_id and value.get('genesisHash') == network.genesis)


def _snapshot(value):
    return (_keys(value, ('height', 'hash', 'timestamp')) and _n(value['height']) and _n(value['timestamp'])
            and isinstance(value['hash'], str) and _LOWER_HASH.fullmatch(value['hash']) is not None)


def _index(value):
    return (_keys(value, ('state', 'head', 'historyHeight', 'updatedAt')) and value['state'] in ('ready', 'syncing', 'reconnecting')
            and _n(value['head'], -1) and _n(value['historyHeight'], -1) and _n(value['updatedAt']))


def _holder_row(row, kind):
    if not _keys(row, HOLDER_ROW_KEYS) or not _n(row['rank'], 1) or not isinstance(row['address'], str) \
            or _LOWER_ADDRESS.fullmatch(row['address']) is None:
        return False
    if row['type'] not in HOLDER_TYPES[1:] or (kind != 'all' and row['type'] != kind):
        return False
    if not (row['label'] is None or row['label'] in HOLDER_LABELS) or not isinstance(row['minerIds'], list):
        return False
    if not all(isinstance(m, str) and _LOWER_ADDRESS.fullmatch(m) for m in row['minerIds']):
        return False
    if not (row['rewardAddress'] is None or (isinstance(row['rewardAddress'], str) and _LOWER_ADDRESS.fullmatch(row['rewardAddress']))):
        return False
    return (all(_wei(row[k]) for k in ('totalWei', 'availableWei', 'bondedWei', 'sharePercentE10'))
            and _n(row['transactions']) and _n_or_null(row['blocksMined'])
            and (row['rewardsWei'] is None or _wei(row['rewardsWei'])))


def holders_page(value, network, options):
    """holders.php: the holders index page (pocol-holders-v1) of this network for exactly `options` (its echo, its
    pinned snapshot) with at most `size` rows of the contract's fields, or None."""
    if not _identity(value, 'pocol-holders-v1', network) or set(value) != HOLDERS_PAGE_KEYS:
        return None
    if any(value[k] != options[k] or type(value[k]) is not type(options[k]) for k in ('page', 'size', 'order', 'type', 'q')):
        return None
    if not _n(value['total']) or not _snapshot(value['snapshot']) or not _index(value['index']):
        return None
    pinned = options.get('snapshot')
    if pinned is not None and (value['snapshot']['height'], value['snapshot']['hash']) != (pinned['height'], pinned['hash']):
        return None
    if not _keys(value['supply'], ('currentSupplyWei',)) or not _wei(value['supply']['currentSupplyWei']):
        return None
    if not _keys(value['verification'], ('complete',)) or type(value['verification']['complete']) is not bool:
        return None
    rows = value['rows']
    if not isinstance(rows, list) or len(rows) > options['size'] or not all(_holder_row(r, options['type']) for r in rows):
        return None
    return value


def holders_stats(value, network):
    """holders-stats.php: the holder statistics (pocol-holders-stats-v1) of this network with the contract's fields
    (and the index's `rewards` range), or None."""
    if not _identity(value, 'pocol-holders-stats-v1', network) or set(value) not in (STATS_KEYS, STATS_KEYS - {'rewards'}):
        return None
    if not _snapshot(value['snapshot']) or not _index(value['index']):
        return None
    if not _keys(value['supply'], SUPPLY_KEYS) or not all(_wei(value['supply'][k]) for k in SUPPLY_KEYS):
        return None
    v = value['verification']
    if (not _keys(v, ('complete', 'balancesSumWei', 'expectedSumWei', 'unlocatedWei', 'accountsIndexed', 'accountsOnChain',
                      'accountsMissing')) or type(v['complete']) is not bool or not _wei(v['balancesSumWei'])
            or not _wei(v['expectedSumWei']) or not _wei(v['unlocatedWei'], _SIGNED) or not _n(v['accountsIndexed'])
            or not _n_or_null(v['accountsOnChain']) or not _n_or_null(v['accountsMissing'], -MAX_ID)):
        return None
    h = value['holders']
    if (not _keys(h, ('count', 'withAvailable', 'withBonded', 'new24h', 'new24hFrom')) or not _n(h['count'])
            or not _n(h['withAvailable']) or not _n(h['withBonded']) or not _n_or_null(h['new24h']) or not _n_or_null(h['new24hFrom'])):
        return None
    c = value['concentration']
    if not _keys(c, ('top1', 'top10', 'top50', 'top100')) or not all(
            _keys(c[k], ('totalWei', 'sharePercentE10')) and _wei(c[k]['totalWei']) and _wei(c[k]['sharePercentE10']) for k in c):
        return None
    d = value['distribution']
    if not isinstance(d, list) or len(d) != 8 or not all(
            _keys(b, ('minWei', 'maxWei', 'accounts', 'totalWei')) and _wei(b['minWei']) and (b['maxWei'] is None or _wei(b['maxWei']))
            and _n(b['accounts']) and _wei(b['totalWei']) for b in d):
        return None
    history = value['history']
    if not _keys(history, ('from', 'points')) or not _n_or_null(history['from']) or not isinstance(history['points'], list) \
            or len(history['points']) > 2000 or not all(
                _keys(p, ('height', 'timestamp', 'holders', 'currentSupplyWei', 'top10SharePercentE10')) and _n(p['height'])
                and _n(p['timestamp']) and _n(p['holders']) and _wei(p['currentSupplyWei']) and _wei(p['top10SharePercentE10'])
                for p in history['points']):
        return None
    if 'rewards' in value and (not _keys(value['rewards'], ('from', 'to')) or not _n_or_null(value['rewards']['from'])
                               or not _n_or_null(value['rewards']['to'])):
        return None
    return value


def miner_geography(value, network):
    """miner-geography.php: the miner map (pocol-miners-geography-v2) of this network, or None. Every key is the
    contract's and every value a count, a flag, a country code, a decimal, a block hash or a reporter name, so no IP
    address, keyed id or miner address can be in the public answer."""
    if not _identity(value, 'pocol-miners-geography-v2', network) or set(value) != GEOGRAPHY_V2_KEYS:
        return None
    if not _n(value['generatedAt']) or not _n(value['height']) or not isinstance(value['blockHash'], str) \
            or _LOWER_HASH.fullmatch(value['blockHash']) is None:
        return None
    database = value['database']
    if database is not None and (not _keys(database, ('provider', 'release', 'license'))
                                 or not all(isinstance(database[k], str) and len(database[k]) <= 64 for k in database)):
        return None
    if not _keys(value['miners'], MAP_MINER_KEYS) or not all(_n(x) for x in value['miners'].values()):
        return None
    if not _keys(value['nodes'], ('observed', 'located', 'unlocated')) or not all(_n(x) for x in value['nodes'].values()):
        return None
    sources = value['sources']
    if not isinstance(sources, list) or len(sources) > 64 or not all(
            _keys(s, ('kind', 'name', 'fresh', 'reportedAt')) and s['kind'] in ('observer', 'operator')
            and isinstance(s['name'], str) and _REPORTER.fullmatch(s['name']) and type(s['fresh']) is bool
            and _n_or_null(s['reportedAt']) for s in sources):
        return None
    if not _keys(value['blocksWindow'], ('from', 'to', 'blocks')) or not all(_n(x) for x in value['blocksWindow'].values()):
        return None
    countries = value['countries']
    if not isinstance(countries, list) or len(countries) > 512 or not all(
            _keys(c, COUNTRY_KEYS) and isinstance(c['code'], str) and re.fullmatch(r'[A-Z]{2}', c['code'])
            and _keys(c['miners'], ('located', 'active', 'inactive', 'connected')) and all(_n(x) for x in c['miners'].values())
            and _n(c['nodes']) and _n(c['blocks']) and _wei(c['blocksSharePercentE10'])
            and (c['rewardsWei'] is None or _wei(c['rewardsWei'])) for c in countries):
        return None
    if not _keys(value['unknown'], ('miners', 'blocks')) or not all(_n(x) for x in value['unknown'].values()):
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
    """The /api paths over one Gateway. `transaction_options_valid` and `holder_options_valid` are the transaction and
    holders indexes' own option checks (gateway.valid_options, gateway.valid_holder_options) and `cost` the token cost
    of one call (gateway.COSTS)."""

    def __init__(self, gateway, site_origin=SITE_ORIGIN, *, cost=lambda call: 1, transaction_options_valid=lambda _: False,
                 holder_options_valid=lambda _: False):
        if not valid_origin(site_origin):
            raise ValueError('Invalid site origin')
        self.gateway = gateway
        self.site_origin = site_origin
        self.cost = cost
        self.transaction_options_valid = transaction_options_valid
        self.holder_options_valid = holder_options_valid
        self.slots = threading.BoundedSemaphore(RELAY_SLOTS)

    def handle(self, method, target, headers, rfile, client=None, source=None):
        """The Response to `method target` for an /api path (404 for one not in ROUTES). `client` is the key the
        gateway charges the request to (gateway.client_key); `source` the sender's own address (gateway.client_address),
        given only to an operator node report."""
        parts = urlsplit(target)
        path = parts.path
        if path not in ROUTES:
            return json_response(404, {'error': 'Not found'})
        methods, message = ROUTES[path]
        if method not in methods:
            return json_response(405, {'error': message}, [('Allow', ', '.join(methods))])
        if path == '/api/rpc.php':
            return self.rpc(headers, rfile, client)
        if path == '/api/health.php':
            return self.health(client)
        if path == '/api/miner-locations.php':
            return self.miner_locations(method, client)
        if path == '/api/transactions.php':
            return self.transactions(method, parts.query, client)
        if path == '/api/holders.php':
            return self.holders(method, parts.query, client)
        if path == '/api/holders-stats.php':
            return self.holder_stats(method, parts.query, client)
        if path == '/api/miner-geography.php':
            return self.miner_map(method, parts.query, client)
        if path == '/api/geo-report.php':
            return self.geo_report(headers, rfile, client, source)
        return json_response(404, {'error': 'Not found'})  # a ROUTES entry without a branch

    def forward(self, payload, client=None):
        """lib.php pocol_forward, in process: (status, answer, error). A free relay slot, the gateway's token budget,
        then each call through Gateway.call (its cache, upstream slots, sanitizer and error mapping)."""
        calls = payload if isinstance(payload, list) else [payload]
        if not self.slots.acquire(blocking=False):
            return 429, None, BUSY
        try:
            if not self.gateway.charge(sum(self.cost(call) for call in calls), client):
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

    def rpc(self, headers, rfile, client=None):
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
        status, answer, error = self.forward(value, client)
        if status != 200:
            return json_response(status, {'error': error}, [('Retry-After', '3')] if status == 429 else [])
        return Response(200, [('Content-Type', JSON_TYPE), ('Cache-Control', 'no-store'), *SECURITY_HEADERS], answer[1])

    def health(self, client=None):
        status, answer, error = self.forward({'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getParams', 'params': []}, client)
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

    def miner_locations(self, method, client=None):
        call = {'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getNetworkGeography', 'params': []}
        status, answer, _ = self.forward(call, client)
        public = public_geography(answer[0].get('result'), self.gateway.network) if status == 200 else None
        if public is None:
            if method == 'HEAD':
                return Response(503, [('Cache-Control', 'no-store'), *SECURITY_HEADERS])
            return json_response(429 if status == 429 else 503, {'error': GEOIP_UNAVAILABLE})
        if method == 'HEAD':
            return Response(200, [('Content-Type', JSON_TYPE), ('Cache-Control', 'no-store'), *SECURITY_HEADERS])
        return json_response(200, {'data': public})

    def transactions(self, method, query, client=None):
        options = transaction_options(query)
        if options is None:
            return json_response(400, {'error': 'Invalid transaction query.'})
        unavailable = json_response(503, {'error': TRANSACTIONS_UNAVAILABLE})
        # The gateway refuses options its transaction index does not take (and every option without the index).
        if not self.transaction_options_valid(options):
            return unavailable
        status, answer, _ = self.forward({'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getTransactions',
                                          'params': [options]}, client)
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

    def index_read(self, method, call, client, check, unavailable, reset=None):
        """One holders or miner map read through forward (the relay slots, the budget, the deadline, the gateway's
        HOLDER_SLOTS): 200 {"data": ..} when `check` accepts the result, 429 with Retry-After when the relay, the
        budget or the gateway's slots are busy, 409 `reset` when the pinned snapshot changed (-32022), 503
        `unavailable` otherwise (no index, no snapshot yet, an answer of another shape)."""
        status, answer, _ = self.forward(call, client)
        if status == 429:
            return json_response(429, {'error': BUSY}, [('Retry-After', '3')])
        reply = answer[0] if status == 200 else {}
        error = reply.get('error')
        code = error.get('code') if isinstance(error, dict) and type(error.get('code')) is int else None
        if code == -32021:
            return json_response(429, {'error': BUSY}, [('Retry-After', '3')])
        if code == -32022 and reset is not None:
            return json_response(409, {'error': reset, 'reset': True})
        data = check(reply.get('result')) if error is None and 'result' in reply else None
        if data is None:
            return json_response(503, {'error': unavailable})
        if method == 'HEAD':
            return Response(200, [('Content-Type', JSON_TYPE), ('Cache-Control', 'no-store'), *SECURITY_HEADERS])
        return json_response(200, {'data': data})

    def holders(self, method, query, client=None):
        options = holder_options(query)
        if options is None:
            return json_response(400, {'error': 'Invalid holder query.'})
        # The gateway refuses options its holders index does not take (and every option without the index).
        if not self.holder_options_valid(options):
            return json_response(503, {'error': HOLDERS_UNAVAILABLE})
        network = self.gateway.network
        return self.index_read(method, {'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getHolders', 'params': [options]},
                               client, lambda value: holders_page(value, network, options), HOLDERS_UNAVAILABLE,
                               reset=HOLDERS_RESET)

    def holder_stats(self, method, query, client=None):
        if not plain_query(query):
            return json_response(400, {'error': 'Invalid holder statistics query.'})
        network = self.gateway.network
        return self.index_read(method, {'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getHolderStats', 'params': []},
                               client, lambda value: holders_stats(value, network), HOLDERS_UNAVAILABLE)

    def miner_map(self, method, query, client=None):
        if not plain_query(query):
            return json_response(400, {'error': 'Invalid miner map query.'})
        network = self.gateway.network
        return self.index_read(method, {'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getMinerGeography', 'params': []},
                               client, lambda value: miner_geography(value, network), MAP_UNAVAILABLE)

    def geo_report(self, headers, rfile, client=None, source=None):
        """POST /api/geo-report.php (§4.1): an operator node report, signed with its reporter's key. The request checks
        of rpc.php (one Content-Type application/json: 415; one Content-Length of digits and no Transfer-Encoding: 400;
        at most 16384 bytes: 413, refused before the body is read), and no Origin at all (a reporter never sends one,
        a browser always does on a cross-site POST): 400. Then 503 without the geo-reporters credential, the request
        budget (429 with Retry-After) and the gateway's verify_report (204 accepted; 400, 401, 409). The source
        address goes to the gateway's country lookup only; nothing here logs or keeps it. The relay slots are not
        taken: a report reads no node and is never queued behind the explorer's reads."""
        if headers.get_all('Origin'):
            return json_response(400, {'error': REPORT_ERRORS[400]})
        types = headers.get_all('Content-Type') or ['']
        if len(types) != 1 or types[0].split(';')[0].strip().lower() != 'application/json':
            return json_response(415, {'error': REPORT_ERRORS[415]})
        lengths = headers.get_all('Content-Length') or []
        if headers.get('Transfer-Encoding') is not None or len(lengths) != 1 or not re.fullmatch(r'[0-9]{1,12}', lengths[0].strip()):
            return json_response(400, {'error': REPORT_ERRORS[400]})
        size = int(lengths[0])
        if size > MAX_REPORT:
            return json_response(413, {'error': REPORT_ERRORS[413]})
        if not self.gateway.reports_enabled():
            return json_response(503, {'error': REPORT_ERRORS[503]})
        try:
            body = rfile.read(size)
        except OSError:
            return json_response(400, {'error': REPORT_ERRORS[400]})
        if len(body) != size:
            return json_response(400, {'error': REPORT_ERRORS[400]})
        if not self.gateway.charge(self.cost({'method': 'pocol_geoReport'}), client):
            return json_response(429, {'error': BUSY}, [('Retry-After', '3')])
        status = self.gateway.geo_report(headers, body, source)
        if status == 204:
            return Response(204, [('Cache-Control', 'no-store'), *SECURITY_HEADERS])
        return json_response(status if status in REPORT_ERRORS else 400, {'error': REPORT_ERRORS.get(status, REPORT_ERRORS[400])})
