"""V3 NetworkProfiles.validate reference for M1 draft 0.21 (D58, D77, D79, D100). Stdlib only.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author. Not extension code.

Sources: browser.md:8-19 (profile, acceptance steps 1-5, netKey), 290-318 (RECV_LIMIT, WorstLegit,
TRUST_POOL); consensus.md:5-44 (GenesisSpec, decode errors and their evaluation order, CP widths,
M_0List rules, SYSTEM_ADDRESS from FINAL_DESIGN.md:787); validation.md:64-79 (GSV1, L4n), 780-784 (RF1-RF4);
governance.md:105 (R24), 3-16 / 152-163 (network, sv-fix, end/full CP values).

Reused read-only: m1-draft-0.2/tools/keccak.py (pure Keccak-256) and rlp_strict.py (canonical encoder).
The decoder here has its own framing parser because integer canonicality must be reported as gsInt
(consensus.md:15), not as a framing error (L0).
"""

import importlib.util
import json
import re
from pathlib import Path

_ROOT = Path(__file__).resolve().parent.parent.parent


def _load(rel, name):
    spec = importlib.util.spec_from_file_location(name, str(_ROOT / rel))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


K = _load('m1-draft-0.2/tools/keccak.py', 'keccak_v3')
RLP = _load('m1-draft-0.2/tools/rlp_strict.py', 'rlp_strict_v3')
keccak256 = K.keccak256

SYSTEM_ADDRESS = b'\xff' * 19 + b'\xfe'                      # FINAL_DESIGN.md:787
RESERVED_LOW, RESERVED_HIGH = 0xC0C001, 0xC0C0FF             # 0x...C0C001 - 0x...C0C0FF (consensus.md:34)

# CP version 1: (name, width bits, lower, upper) in order (consensus.md:18-25)
CP_FIELDS = [
    ('g_ts', 64, 0, None), ('target_g', 256, 1, None), ('T_blk', 32, 1, None), ('tau', 32, 1, None),
    ('nonceMode', 8, 0, 1), ('c', 8, 1, None), ('D_att', 32, 0, None), ('dFbWait', 32, 0, None),
    ('m', 16, 1, None), ('kappa', 8, 1, None), ('S_max', 16, 0, 256), ('D', 16, 0, None), ('W_w', 16, 0, None),
    ('alpha_bp', 16, 0, 10000), ('gamma_bp', 16, 0, 'gamma'),
    ('subsidy', 256, 0, None), ('B_reg', 256, 0, None), ('R_max', 8, 0, None), ('M_max', 16, 1, None), ('M_min', 16, 1, None),
    ('REG_RECORDS_MAX', 32, 0, None), ('SWEEP_MAX', 16, 0, None), ('SWEEP_SCAN', 16, 0, None), ('REG_OPS', 16, 0, None),
    ('W_act', 32, 0, None), ('Theta_inact', 32, 0, None), ('E_win', 32, 0, None), ('U', 32, 0, None), ('E_max', 8, 0, None),
    ('EVID_BYTES_MAX', 32, 0, None),
    ('GAS_LIMIT', 64, 0, None), ('baseFee0', 256, 0, None), ('TX_MAX', 32, 0, None), ('BODY_MAX', 32, 0, None),
    ('B_code_max', 32, 0, None), ('CODE_CAP', 64, 0, None), ('TXBYTES_CAP', 64, 0, None), ('TX_CAP', 64, 0, None),
    ('SLOT_CAP', 64, 0, None), ('ACCT_CAP', 64, 0, None),
    ('SYS_SLOT_MAX', 32, 0, None), ('SYS_KEYS_BLOCK_MAX', 32, 0, None), ('SYS_KEYS_USER_MAX', 32, 0, None),
    ('SYS_ACCT_BLOCK_MAX', 32, 0, None), ('G_SYS', 64, 0, None),
    ('H_END', 64, 0, None), ('T_END', 64, 0, None), ('H_CLOSE', 64, 0, None), ('T_CLOSE', 64, 0, None),
    ('Z_close', 32, 0, None), ('RET', 32, 1, None), ('P2', 32, 0, None),
]
assert len(CP_FIELDS) == 52

# RECV_LIMIT rows that depend on the profile (browser.md:300-306); RecvFit checks these.
RECV_FIT_ROWS = [
    ('eth_getCode', 69632, lambda cp, gp: 2 * cp['B_code_max'] + 1024),
    ('pocol_getParams', 98304, lambda cp, gp: 2 * gp + 4096),
    ('eth_getBlockByHash/Number', 167936, lambda cp, gp: 2048 + (cp['BODY_MAX'] // 85) * 70),
    ('eth_getTransactionByHash', 266240, lambda cp, gp: 2 * cp['TX_MAX'] + 2048),
]
TRUST_POOL = 2 * 1024 * 1024                                  # browser.md:318
PROFILE_FETCH_CAP = 1048576                                   # P-V3-6 (proposal: not in the source)
PROFILE_FETCH_DEADLINE_MS = 10000                             # P-V3-6 (proposal: not in the source)
PROFILE_KEYS = ('profileId', 'name', 'chainId', 'genesisPre', 'genesisHash', 'forkSchedule', 'endpoint', 'trustLevel')
_HEX = re.compile(r'0x(?:[0-9a-f]{2})+')
_H32 = re.compile(r'0x[0-9a-f]{64}')


class GsError(Exception):
    def __init__(self, code, detail=''):
        super().__init__(code, detail)
        self.code, self.detail = code, detail


# ------------------------------------------------------------------ framing (L0)

def _parse(b, i):
    """Returns (node, next). node = ('b', bytes, wrapped) or ('l', [nodes]). Framing errors -> L0."""
    if i >= len(b):
        raise GsError('L0', 'truncated')
    p = b[i]
    if p < 0x80:
        return ('b', b[i:i + 1], False), i + 1
    if p <= 0xbf:
        if p <= 0xb7:
            n, st = p - 0x80, i + 1
        else:
            ll = p - 0xb7
            lb = b[i + 1:i + 1 + ll]
            if len(lb) != ll:
                raise GsError('L0', 'truncated length')
            if lb[0] == 0:
                raise GsError('L0', 'leading zero in length')
            n, st = int.from_bytes(lb, 'big'), i + 1 + ll
            if n < 56:
                raise GsError('L0', 'long form for short string')
        if st + n > len(b):
            raise GsError('L0', 'truncated string')
        s = bytes(b[st:st + n])
        return ('b', s, n == 1 and s[0] < 0x80), st + n            # wrapped single byte: integer rule, judged as gsInt
    if p <= 0xf7:
        n, st = p - 0xc0, i + 1
    else:
        ll = p - 0xf7
        lb = b[i + 1:i + 1 + ll]
        if len(lb) != ll:
            raise GsError('L0', 'truncated length')
        if lb[0] == 0:
            raise GsError('L0', 'leading zero in length')
        n, st = int.from_bytes(lb, 'big'), i + 1 + ll
        if n < 56:
            raise GsError('L0', 'long form for short list')
    end = st + n
    if end > len(b):
        raise GsError('L0', 'truncated list')
    out, j = [], st
    while j < end:
        x, j = _parse(b, j)
        if j > end:
            raise GsError('L0', 'item crosses list end')
        out.append(x)
    return ('l', out), end


def parse(b):
    node, j = _parse(bytes(b), 0)
    if j != len(b):
        raise GsError('L0', '%d trailing bytes' % (len(b) - j))
    return node


# ------------------------------------------------------------------ GenesisSpec decode (consensus.md:7-34)

def _isb(n):
    return n[0] == 'b'


def _isl(n):
    return n[0] == 'l'


def _val(n):
    return int.from_bytes(n[1], 'big')


def decode_genesis(b):
    """Returns the decoded spec or raises GsError. Evaluation order (consensus.md:16): structure, gsVersion,
    gsCount, gsInt, gsLen, gsRange, gsOrder, gsSys. Framing errors are L0 (validation.md:77, N9)."""
    top = parse(b)
    # structure: the shapes that exist must be of the right kind (P-V3-2: reported as gsStructure)
    if not _isl(top):
        raise GsError('gsStructure', 'top not a list')
    kinds = ['b', 'b', 'l', 'b', 'l', 'b']
    for idx, node in enumerate(top[1][:6]):
        if node[0] != kinds[idx]:
            raise GsError('gsStructure', 'top[%d]' % idx)
    items = top[1]
    cp = items[2][1] if len(items) > 2 else []
    m0 = items[4][1] if len(items) > 4 else []
    if any(not _isb(x) for x in cp):
        raise GsError('gsStructure', 'CP item is a list')
    for e in m0:
        if not _isl(e) or any(not _isb(x) for x in e[1]):
            raise GsError('gsStructure', 'M_0List entry')
    # gsVersion
    if items and _val(items[0]) != 1:
        raise GsError('gsVersion', _val(items[0]))
    # gsCount
    if len(items) != 6:
        raise GsError('gsCount', 'top %d' % len(items))
    if len(cp) != 52:
        raise GsError('gsCount', 'CP %d' % len(cp))
    if len(m0) < 1:
        raise GsError('gsCount', 'M_0List empty')
    for e in m0:
        if len(e[1]) != 2:
            raise GsError('gsCount', 'M_0List entry %d' % len(e[1]))
    # gsInt: integers are specVersion, chainId and every CP item
    ints = [('specVersion', items[0]), ('chainId', items[1])] + [(CP_FIELDS[k][0], cp[k]) for k in range(52)]
    for name, n in ints:
        if n[2] or (len(n[1]) > 0 and n[1][0] == 0):
            raise GsError('gsInt', name)
    # gsLen
    if len(items[3][1]) != 32 or len(items[5][1]) != 32:
        raise GsError('gsLen', 'root')
    for e in m0:
        if len(e[1][0][1]) != 20 or len(e[1][1][1]) != 20:
            raise GsError('gsLen', 'M_0List address')
    # gsRange
    chain_id = _val(items[1])
    if not 1 <= chain_id <= 2 ** 64 - 1:
        raise GsError('gsRange', 'chainId')
    vals = {}
    for k, (name, width, lo, hi) in enumerate(CP_FIELDS):
        v = _val(cp[k])
        vals[name] = v
        if v >= 2 ** width or v < lo:
            raise GsError('gsRange', name)
        if hi == 'gamma':
            if v > 10000 - vals['alpha_bp']:
                raise GsError('gsRange', name)
        elif hi is not None and v > hi:
            raise GsError('gsRange', name)
    # gsOrder
    ids = [e[1][0][1] for e in m0]
    for a, c in zip(ids, ids[1:]):
        if not a < c:
            raise GsError('gsOrder', 'ids')
    # gsSys
    for i in ids:
        if i == bytes(20) or i == SYSTEM_ADDRESS or (i[:17] == bytes(17) and RESERVED_LOW <= int.from_bytes(i[17:], 'big') <= RESERVED_HIGH):
            raise GsError('gsSys', '0x' + i.hex())
    return {'specVersion': 1, 'chainId': chain_id, 'CP': vals, 'allocRoot': items[3][1], 'sysCodeHash': items[5][1],
            'M_0List': [[e[1][0][1], e[1][1][1]] for e in m0]}


def encode_genesis(spec):
    """Canonical re-encoding with the 0.2 encoder (used for the decode -> re-encode identity)."""
    cp = [RLP.uint(spec['CP'][n]) for n, _, _, _ in CP_FIELDS]
    return RLP.encode([RLP.uint(spec['specVersion']), RLP.uint(spec['chainId']), cp, spec['allocRoot'],
                       [[i, r] for i, r in spec['M_0List']], spec['sysCodeHash']])


# ------------------------------------------------------------------ profile validation (browser.md:8-17)

def _shape(p):
    """P-V3-1: exact types, no normalization (no case folding, trimming or URL rewriting)."""
    if type(p) is not dict:
        return 'notObject'
    if set(p) != set(PROFILE_KEYS):
        return 'keys'
    if type(p['profileId']) is not str or not p['profileId']:
        return 'profileId'
    if type(p['name']) is not str or not p['name']:
        return 'name'
    if type(p['chainId']) is not int:
        return 'chainId'
    if type(p['genesisPre']) is not str or not _HEX.fullmatch(p['genesisPre']):
        return 'genesisPre'
    if type(p['genesisHash']) is not str or not _H32.fullmatch(p['genesisHash']):
        return 'genesisHash'
    fs = p['forkSchedule']
    if type(fs) is not list or not all(type(x) is list and len(x) == 2 and all(type(y) is int for y in x) for x in fs):
        return 'forkSchedule'
    if type(p['endpoint']) is not str or not p['endpoint']:
        return 'endpoint'
    if p['trustLevel'] not in ('DEV', 'LN', 'RP'):
        return 'trustLevel'
    return None


def identity_line(p):
    """P-V3-4: the confirmation shows name, chainId and the FULL genesisHash (browser.md:15 step 4)."""
    return '%s · chainId %d · genesisHash %s' % (p['name'], p['chainId'], p['genesisHash'])


def net_key(chain_id, genesis_hash_hex):
    return '0x' + keccak256(RLP.encode([b'PoCol-net-v1', RLP.uint(chain_id), bytes.fromhex(genesis_hash_hex[2:])])).hex()


def recv_fit(cp, gp_len):
    rows = []
    for method, limit, f in RECV_FIT_ROWS:
        w = f(cp, gp_len)
        rows.append({'method': method, 'worst': w, 'limit': limit, 'fits': w <= limit})
    return rows


def validate(profile, trust):
    """trust = {'signed': bool, 'confirmedHash': str|None}. Returns {ok, error|None, trace, netKey?}.
    Order: shape, decode (step 1), hash (2), chainId (3), signed/confirmed (4), RecvFit (5)."""
    trace = []

    def fail(err):
        return {'ok': False, 'error': err, 'trace': trace}

    bad = _shape(profile)
    if bad:
        trace.append(['shape', 'fail', bad])
        return fail({'code': 'profileShape', 'field': bad})
    trace.append(['shape', 'ok'])
    gp = bytes.fromhex(profile['genesisPre'][2:])
    try:
        spec = decode_genesis(gp)
    except GsError as e:
        trace.append(['decode', 'fail', e.code])
        return fail({'code': e.code, 'detail': str(e.detail)})
    trace.append(['decode', 'ok', len(gp)])
    h = '0x' + keccak256(gp).hex()
    if h != profile['genesisHash']:
        trace.append(['hash', 'fail'])
        return fail({'code': 'genesisHash', 'rule': 'R24(a)'})
    trace.append(['hash', 'ok'])
    if spec['chainId'] != profile['chainId']:
        trace.append(['chainId', 'fail'])
        return fail({'code': 'chainId'})
    trace.append(['chainId', 'ok'])
    if trust.get('signed') is True:
        trace.append(['trust', 'signed'])
    elif trust.get('confirmedHash') == profile['genesisHash']:
        trace.append(['trust', 'confirmed', identity_line(profile)])
    else:
        trace.append(['trust', 'fail', identity_line(profile)])
        return fail({'code': 'unconfirmed'})
    rows = recv_fit(spec['CP'], len(gp))
    over = [r for r in rows if not r['fits']]
    trace.append(['recvFit', 'ok' if not over else 'fail', rows])
    if over:
        return fail({'code': -32019, 'data': {'rule': 'recvFit', 'violations': [{k: r[k] for k in ('method', 'worst', 'limit')} for r in over]}})
    return {'ok': True, 'error': None, 'trace': trace, 'netKey': net_key(profile['chainId'], profile['genesisHash'])}


def may_start_node(profile, result, known_nonbootable):
    """A validated profile is a view/RPC configuration only. Starting a node also needs node-side R24(a)
    (allocRoot/sysCodeHash recomputed from alloc), which the extension cannot do (threat.md:43)."""
    if not result.get('ok'):
        return False, 'notValidated'
    if profile['genesisHash'] in known_nonbootable:
        return False, 'knownNonBootable'
    return False, 'nodeSideR24aRequired'


# ------------------------------------------------------------------ FakeTransport (no network)

class FakeTransport:
    """Scripted HttpTransport stand-in for the trust pool: request(url, recvLimit, deadlineMs) reserves recvLimit
    from TRUST_POOL (P-V3-7), answers from the script and releases. Script entry: {body|bodyBytes, delayMs}."""

    def __init__(self, script, pool=TRUST_POOL, reserved=0):
        self.script, self.pool, self.reserved, self.log = list(script), pool, reserved, []

    def request(self, url, recv_limit, deadline_ms):
        entry = self.script.pop(0) if self.script else {'error': 'transport'}
        if self.pool - self.reserved < recv_limit:
            self.log.append([url, 'busy'])
            return {'error': 'busy'}
        self.reserved += recv_limit
        try:
            if entry.get('delayMs', 0) > deadline_ms:
                self.log.append([url, 'transport'])
                return {'error': 'transport'}
            body = entry['body'] if 'body' in entry else 'x' * entry['bodyBytes']
            if len(body.encode('utf-8')) > recv_limit:
                self.log.append([url, 'recvLimit'])
                return {'error': 'recvLimit'}
            self.log.append([url, 'ok', len(body.encode('utf-8'))])
            return {'body': body}
        finally:
            self.reserved -= recv_limit


def fetch_and_validate(transport, url, trust):
    """networks.json fetch through the trust pool, then validate each profile; nothing is accepted on a
    transport error and no partially parsed list is used."""
    r = transport.request(url, PROFILE_FETCH_CAP, PROFILE_FETCH_DEADLINE_MS)
    if 'error' in r:
        return {'ok': False, 'error': {'code': r['error']}, 'profiles': []}
    try:
        doc = json.loads(r['body'])
    except ValueError:
        return {'ok': False, 'error': {'code': 'networksJson'}, 'profiles': []}
    if type(doc) is not list:
        return {'ok': False, 'error': {'code': 'networksJson'}, 'profiles': []}
    return {'ok': True, 'error': None, 'profiles': [validate(p, trust) for p in doc]}
