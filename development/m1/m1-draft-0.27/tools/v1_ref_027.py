"""HeaderNetCheck (RP header window) reference for M1 draft 0.27, row V1. Successor of
m1-draft-0.26/tools/v1_ref.py (which is unchanged). SPECIFICATION FIXTURE TOOLING ONLY.
Python standard library only. NOT executed by the author.

Changes from 0.26, each a reference/instrumentation repair, not a change of normative acceptance:
  C32  Replies are parsed from their literal JSON text. Every JSON-RPC envelope and every -32021
       error is validated (annex/C32-RPC-ERROR-ENVELOPE.md). A malformed reply ends the check with a
       controlled viewIncomplete and no frame; it never raises and never waits.
  V1-SHA-COST  Every SHA-256 evaluation goes through one counting function with a category
       (templateId, powHash, shareHash). The counters report logical evaluations, unique preimages, and
       the same run with the TemplateID cache disabled (annex/V1-SHA-COST.md). The 0.26 reading
       "SHA-256 = PoW only" (P-V1-3) is withdrawn; the source bound stays an unresolved discrepancy.
  Builder  Headers may carry share lists (search for valid share nonces, consensus.md:161).

Normative behaviour otherwise unchanged from 0.26: steps, items, rule codes, messages, ASERT.
"""

import hashlib
import hmac
import json
import math

K = None
RLP = None


def bind(keccak_mod, rlp_mod):
    global K, RLP
    K, RLP = keccak_mod, rlp_mod


TOP = 2 ** 256
U256_MAX = TOP - 1
VIEW_SLACK_S = 4
PHI_VIEW_S = 10
KVIEW = 12
TAG = b'PoCol-tpl-v1'
UT_MAX, ST_MAX, HDR_MAX = 613, 682, 3067
SHARES_MAX = 256
MAX_RETRIES = 3                                     # browser.md:41
RETRY_RANGE = {'busy': (250, 2000), 'rate': (0, 2000)}     # network.md:234 (busy); P-C32-2 (rate)
NO_BLOCKS = 'الشبكة لم تنتج الكتلة 1 بعد؛ لا رأس للتحقق في RP'
WARN_MINWORK1 = 'حد العمل غير فعال لهذه الشبكة'
INT_W = {'chainId': 8, 'h': 8, 'a': 4, 'protocolVersion': 2, 'ts': 8, 'target': 32, 'gasLimit': 8, 'gasUsed': 8, 'baseFee': 32}
FIX_W = {'genesisHash': 32, 'parentHash': 32, 'stateRoot': 32, 'txRoot': 32, 'receiptsRoot': 32, 'logsBloom': 256, 'evidenceRoot': 32}
UT_FIELDS = ('tag', 'chainId', 'genesisHash', 'parentHash', 'h', 'a', 'protocolVersion', 'ts', 'target', 'stateRoot', 'txRoot',
             'receiptsRoot', 'logsBloom', 'gasLimit', 'gasUsed', 'baseFee', 'evidenceRoot', 'proposer')


def ib(n):
    return n.to_bytes((n.bit_length() + 7) // 8, 'big')


def be64(n):
    return n.to_bytes(8, 'big')


def ceil_target(target_g):
    return min(U256_MAX, target_g * 2 ** VIEW_SLACK_S)


def work(t):
    return TOP // (t + 1)


def min_work(ceil):
    return TOP // (ceil + 1)


def plan(h):
    b = max(1, h - KVIEW)
    frm = max(1, b - 1)
    return {'b': b, 'from': frm, 'count': h - frm + 1, 'n': h - b + 1, 'hasRef': b > 1, 'anchoredGenesis': b == 1}


def count_phrase(n):
    if n == 1:
        return 'رأس واحد'
    if n == 2:
        return 'رأسين'
    if 3 <= n <= 10:
        return '%d رؤوس' % n
    return '%d رأسًا' % n


def message(n, anchored_genesis, minwork):
    m = ('RP: مرتبط بالعمل والهوية لـ' + count_phrase(n) + (' حتى genesis' if anchored_genesis else '')
         + '، بعمل محتسب لا يقل عن ' + str(minwork) + ' لكل رأس (work(ceilTarget))، دون تحقق تنفيذ أو عضوية')
    if minwork == 1:
        m += '، ' + WARN_MINWORK1
    return m


# ------------------------------------------------------------------ counters and SHA-256 accounting

class Counters:
    def __init__(self):
        self.decodes = self.templateIds = self.asert = self.powHash = self.shareHash = self.ecrecover = 0
        self.maxAsertBits = 0
        self.sha = {'templateId': 0, 'powHash': 0, 'shareHash': 0}
        self.preimages = set()
        self.events = []
        self.sleptMs = 0

    def sha256(self, category, data):
        self.sha[category] += 1
        self.preimages.add(data)
        return hashlib.sha256(data).digest()

    def as_dict(self):
        return {'headerDecodes': self.decodes, 'templateIds': self.templateIds, 'asert': self.asert, 'powHash': self.powHash,
                'shareHash': self.shareHash, 'ecrecover': self.ecrecover, 'maxAsertBits': self.maxAsertBits,
                'sha256Total': sum(self.sha.values()), 'sha256ByCategory': dict(self.sha), 'sha256UniquePreimages': len(self.preimages),
                'sleptMs': self.sleptMs}


def asert(cp, p_ts, p_h, ctr=None):
    if ctr is not None:
        ctr.asert += 1
    dt = (p_ts - cp['g_ts']) - cp['T_blk'] * p_h
    if not abs(dt) < 2 ** 97:
        return None
    num = dt * 65536
    e = num // cp['tau']
    s = e // 65536
    f = e - 65536 * s
    bits = [num.bit_length(), e.bit_length()]
    if s >= 256:
        y = U256_MAX
    elif s <= -257:
        y = 1
    else:
        poly = 195766423245049 * f + 971821376 * f * f + 5127 * f ** 3 + 2 ** 47
        F = 65536 + poly // 2 ** 48
        X = cp['target_g'] * F
        k = s - 16
        Y = X << k if k >= 0 else X >> (-k)
        bits += [poly.bit_length(), X.bit_length(), Y.bit_length()]
        y = min(max(Y, 1), U256_MAX)
    if ctr is not None:
        ctr.maxAsertBits = max(ctr.maxAsertBits, max(bits))
    return y


# ------------------------------------------------------------------ secp256k1 (unchanged from 0.26)

P_FIELD = 2 ** 256 - 2 ** 32 - 977
N_ORDER = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)


def _add(a, b):
    if a is None:
        return b
    if b is None:
        return a
    if a[0] == b[0]:
        if (a[1] + b[1]) % P_FIELD == 0:
            return None
        lam = 3 * a[0] * a[0] * pow(2 * a[1], -1, P_FIELD) % P_FIELD
    else:
        lam = (b[1] - a[1]) * pow(b[0] - a[0], -1, P_FIELD) % P_FIELD
    x = (lam * lam - a[0] - b[0]) % P_FIELD
    return (x, (lam * (a[0] - x) - a[1]) % P_FIELD)


def ec_mul(k, pt):
    k %= N_ORDER
    out = None
    while k:
        if k & 1:
            out = _add(out, pt)
        pt = _add(pt, pt)
        k >>= 1
    return out


def pub_bytes(pt):
    return pt[0].to_bytes(32, 'big') + pt[1].to_bytes(32, 'big')


def address_of(pt):
    return K.keccak256(pub_bytes(pt))[12:]


def _rfc6979_k(x, z32):
    h = (int.from_bytes(z32, 'big') % N_ORDER).to_bytes(32, 'big')
    bx = x.to_bytes(32, 'big') + h
    v, kk = b'\x01' * 32, b'\x00' * 32
    kk = hmac.new(kk, v + b'\x00' + bx, hashlib.sha256).digest()
    v = hmac.new(kk, v, hashlib.sha256).digest()
    kk = hmac.new(kk, v + b'\x01' + bx, hashlib.sha256).digest()
    v = hmac.new(kk, v, hashlib.sha256).digest()
    while True:
        v = hmac.new(kk, v, hashlib.sha256).digest()
        k = int.from_bytes(v, 'big')
        if 1 <= k < N_ORDER:
            return k
        kk = hmac.new(kk, v + b'\x00', hashlib.sha256).digest()
        v = hmac.new(kk, v, hashlib.sha256).digest()


def sign(x, z32):
    z = int.from_bytes(z32, 'big')
    k = _rfc6979_k(x, z32)
    R = ec_mul(k, G)
    if R[0] >= N_ORDER:
        raise ValueError('R.x >= n')
    r = R[0]
    s = pow(k, -1, N_ORDER) * (z + r * x) % N_ORDER
    v = R[1] & 1
    if s > N_ORDER // 2:
        s, v = N_ORDER - s, v ^ 1
    return r.to_bytes(32, 'big') + s.to_bytes(32, 'big') + bytes([v])


def recover(z32, sig, ctr=None):
    if ctr is not None:
        ctr.ecrecover += 1
    if len(sig) != 65:
        return None
    r, s, v = int.from_bytes(sig[:32], 'big'), int.from_bytes(sig[32:64], 'big'), sig[64]
    if not (1 <= r < N_ORDER and 1 <= s <= N_ORDER // 2 and v in (0, 1)):
        return None
    alpha = (pow(r, 3, P_FIELD) + 7) % P_FIELD
    beta = pow(alpha, (P_FIELD + 1) // 4, P_FIELD)
    if beta * beta % P_FIELD != alpha:
        return None
    y = beta if beta & 1 == v else P_FIELD - beta
    z = int.from_bytes(z32, 'big')
    rinv = pow(r, -1, N_ORDER)
    return _add(ec_mul(-z * rinv % N_ORDER, G), ec_mul(s * rinv % N_ORDER, (r, y)))


# ------------------------------------------------------------------ derivations and builder

def sig_msg(tid):
    return K.keccak256(b'\x19' + b'PoCol template' + b'\x0a' + tid)


def win_msg(tid, nonce, share_root):
    return K.keccak256(b'\x19' + b'PoCol winner' + b'\x0a' + tid + be64(nonce) + share_root)


def ut_list(f):
    return [ib(f[name]) if name in INT_W else f[name] for name in UT_FIELDS]


def build_header(f, signer_x, nonce_rule, share_count=0, m=32, invalid_after=None):
    """Fixture builder (not the checker). nonce_rule: ('pow'|'fail'|'fixed', n). share_count: number of
    valid shares to search: smallest nonces n != nonce with SHA256(tid || be64(n)) <= min(2^256-1, target*m).
    invalid_after: if set, append the smallest nonce greater than the last valid share that FAILS T_share."""
    ut = ut_list(f)
    ut_raw = RLP.encode(ut)
    tid = hashlib.sha256(ut_raw).digest()
    sig = sign(signer_x, sig_msg(tid)) if f['proposer'] else b''
    kind, val = nonce_rule
    if kind == 'fixed':
        nonce = val
    else:
        nonce = 0
        while True:
            ok = int.from_bytes(hashlib.sha256(tid + be64(nonce)).digest(), 'big') <= f['target']
            if ok == (kind == 'pow'):
                break
            nonce += 1
    shares = []
    t_share = min(U256_MAX, f['target'] * m)
    n = 0
    while len(shares) < share_count:
        if n != nonce and int.from_bytes(hashlib.sha256(tid + be64(n)).digest(), 'big') <= t_share:
            shares.append(n)
        n += 1
    if invalid_after:
        while n == nonce or int.from_bytes(hashlib.sha256(tid + be64(n)).digest(), 'big') <= t_share:
            n += 1
        shares.append(n)
    share_items = [be64(x) for x in shares]
    share_root = K.keccak256(RLP.encode(share_items))
    wsig = sign(signer_x, win_msg(tid, nonce, share_root))
    item = [[ut, sig], be64(nonce), share_items, wsig]
    return {'item': item, 'utRaw': ut_raw, 'tid': tid, 'nonce': nonce, 'shares': shares, 'shareRoot': share_root,
            'blockHash': K.keccak256(tid + be64(nonce) + share_root), 'powHash': hashlib.sha256(tid + be64(nonce)).digest(),
            'sigMsg': sig_msg(tid), 'winMsg': win_msg(tid, nonce, share_root), 'sig': sig, 'winnerSig': wsig, 'encoded': RLP.encode(item)}


# ------------------------------------------------------------------ C32: reply parsing and error envelope

class _Reject(Exception):
    pass


def _no_constants(name):
    raise _Reject('non-standard JSON constant ' + name)


def integral(v):
    """Value semantics of the accepted C19 (m1-draft-0.6/tools/bridge_ref_06.py integral_value): a JSON number
    whose value is finite and integral; booleans and strings are never numbers."""
    if type(v) is bool:
        return None
    if type(v) is int:
        return v
    if type(v) is float and math.isfinite(v) and v.is_integer():
        return int(v)
    return None


def classify_reply(text):
    """Returns ('result', value) | ('retry', ms, reason) | ('error', code) | ('malformed', why). Never raises."""
    try:
        obj = json.loads(text, parse_constant=_no_constants)
    except (ValueError, _Reject, TypeError, RecursionError) as e:
        return ('malformed', 'json: %s' % type(e).__name__)
    if type(obj) is not dict:
        return ('malformed', 'envelope not an object')
    if obj.get('jsonrpc') != '2.0':
        return ('malformed', 'jsonrpc')
    has_r, has_e = 'result' in obj, 'error' in obj
    if has_r == has_e:
        return ('malformed', 'result xor error')
    if has_r:
        return ('result', obj['result'])
    e = obj['error']
    if type(e) is not dict:
        return ('malformed', 'error not an object')
    code = integral(e.get('code'))
    if code is None:
        return ('malformed', 'error.code')
    if type(e.get('message')) is not str:
        return ('malformed', 'error.message')
    if code != -32021:
        return ('error', code)
    d = e.get('data')
    if type(d) is not dict:
        return ('malformed', 'busy data not an object')
    reason = d.get('reason')
    if type(reason) is not str or reason not in RETRY_RANGE:
        return ('malformed', 'busy reason')
    ms = integral(d.get('retryAfterMs'))
    lo, hi = RETRY_RANGE[reason]
    if ms is None or not lo <= ms <= hi:
        return ('malformed', 'retryAfterMs')
    return ('retry', ms, reason)


# ------------------------------------------------------------------ the checker

class RuleFail(Exception):
    def __init__(self, rule, detail=None):
        super().__init__(rule)
        self.rule, self.detail = rule, detail


def _uint(b, width):
    if type(b) is not bytes or len(b) > width or (len(b) > 0 and b[0] == 0):
        raise RuleFail('1', 'integer')
    return int.from_bytes(b, 'big')


def parse_header(item):
    if not (type(item) is list and len(item) == 4):
        raise RuleFail('1', 'header arity')
    st, nonce, shares, wsig = item
    if not (type(st) is list and len(st) == 2 and type(st[0]) is list and len(st[0]) == 18):
        raise RuleFail('1', 'template arity')
    ut, sig = st
    f = {}
    for name, v in zip(UT_FIELDS, ut):
        if name == 'tag':
            if v != TAG:
                raise RuleFail('1', 'tag')
            f[name] = v
        elif name in INT_W:
            f[name] = _uint(v, INT_W[name])
        elif name in FIX_W:
            if type(v) is not bytes or len(v) != FIX_W[name]:
                raise RuleFail('1', name)
            f[name] = v
        else:
            if type(v) is not bytes or len(v) not in (0, 20):
                raise RuleFail('1', 'proposer')
            f[name] = v
    if not 1 <= f['target'] <= U256_MAX:
        raise RuleFail('1', 'target range')
    if type(sig) is not bytes or len(sig) not in (0, 65):
        raise RuleFail('1', 'sig')
    if type(nonce) is not bytes or len(nonce) != 8:
        raise RuleFail('1', 'nonce')
    if type(shares) is not list or len(shares) > SHARES_MAX or any(type(x) is not bytes or len(x) != 8 for x in shares):
        raise RuleFail('1', 'shareList')
    if type(wsig) is not bytes or len(wsig) != 65:
        raise RuleFail('1', 'winnerSig')
    ut_raw = RLP.encode(ut)
    if len(ut_raw) > UT_MAX or len(RLP.encode(st)) > ST_MAX or len(RLP.encode(item)) > HDR_MAX:
        raise RuleFail('1', 'size')
    f.update(sig=sig, nonce=int.from_bytes(nonce, 'big'), shares=[int.from_bytes(x, 'big') for x in shares],
             shareItems=shares, winnerSig=wsig, utRaw=ut_raw)
    return f


def _qty(v):
    if type(v) is not str or not v.startswith('0x') or len(v) < 3:
        return None
    body = v[2:]
    if any(c not in '0123456789abcdef' for c in body) or (len(body) > 1 and body[0] == '0'):
        return None
    return int(body, 16)


class Window:
    def __init__(self, cfg, ctr, cache=True):
        self.cfg, self.cp, self.ctr, self.cache = cfg, cfg['cp'], ctr, cache
        self._tid = {}

    def tid(self, hd):
        """TemplateID = SHA256(RLP(UT)) (consensus.md:59). With cache=False every use recomputes it."""
        key = id(hd)
        if self.cache and key in self._tid:
            return self._tid[key]
        self.ctr.templateIds += 1
        t = self.ctr.sha256('templateId', hd['utRaw'])
        self._tid[key] = t
        return t

    def block_hash(self, p):
        if p.get('genesis'):
            return self.cfg['genesisHash']
        return K.keccak256(self.tid(p) + be64(p['nonce']) + K.keccak256(RLP.encode(p['shareItems'])))

    def active_version(self, h):
        v = None
        for ver, start in self.cfg['forkSchedule']:
            if start <= h:
                v = ver
        return v

    def netid(self, hd):
        self.ctr.events.append(['netid', hd['h']])
        if hd['chainId'] != self.cfg['chainId']:
            raise RuleFail('netChain')
        if hd['genesisHash'] != self.cfg['genesisHash']:
            raise RuleFail('netGenesis')
        if hd['protocolVersion'] != self.active_version(hd['h']):
            raise RuleFail('netVersion')

    def future(self, hd, clock_s):
        self.ctr.events.append(['viewFuture', hd['h']])
        if hd['ts'] > clock_s + PHI_VIEW_S:
            raise RuleFail('viewFuture')

    def window_header(self, x, p, clock_s, ceil):
        cp, ctr = self.cp, self.ctr
        self.netid(x)
        self.future(x, clock_s)
        ctr.events.append(['item2', x['h']])
        if x['h'] != p['h'] + 1:
            raise RuleFail('2', 'height')
        if p.get('genesis'):
            if x['parentHash'] != self.cfg['genesisHash']:
                raise RuleFail('viewGenesis')
        elif x['parentHash'] != self.block_hash(p):
            raise RuleFail('2', 'parentHash')
        if x['h'] > cp['H_END']:
            raise RuleFail('2', 'H_END (10a)')
        ctr.events.append(['item3', x['h']])
        signed = bool(x['proposer']) or bool(x['sig'])
        if signed:
            if not (x['proposer'] and x['sig'] and x['a'] == 0):
                raise RuleFail('3', 'form')
            q = recover(sig_msg(self.tid(x)), x['sig'], ctr)
            if q is None or address_of(q) != x['proposer']:
                raise RuleFail('3', 'ecrecover')
        ctr.events.append(['item4', x['h']])
        a = x['a']
        lo = p['ts'] + a * cp['D_att'] + 1
        hi = p['ts'] + (a + 1) * cp['D_att']
        if hi >= 2 ** 63 or not lo <= x['ts'] <= hi:
            raise RuleFail('4', 'window')
        if not signed and x['ts'] != (lo + cp['dFbWait'] if a == 0 else lo):
            raise RuleFail('4', 'fallback stamp')
        ctr.events.append(['asert', x['h']])
        t = asert(cp, p['ts'], p['h'], ctr)
        if t is None or x['target'] != t:
            raise RuleFail('5', {'expected': t})
        ctr.events.append(['viewTargetCeil', x['h']])
        if x['target'] > ceil:
            raise RuleFail('viewTargetCeil')
        n_max = 2 ** 64 if cp['nonceMode'] == 1 else min(2 ** 64, cp['c'] * work(x['target']))
        ctr.events.append(['item6', x['h']])
        if not x['nonce'] < n_max:
            raise RuleFail('6')
        ctr.events.append(['powHash', x['h']])
        ctr.powHash += 1
        if int.from_bytes(ctr.sha256('powHash', self.tid(x) + be64(x['nonce'])), 'big') > x['target']:
            raise RuleFail('7')
        ctr.events.append(['item8', x['h']])
        share_root = K.keccak256(RLP.encode(x['shareItems']))
        if recover(win_msg(self.tid(x), x['nonce'], share_root), x['winnerSig'], ctr) is None:
            raise RuleFail('8')
        ctr.events.append(['item9', x['h']])
        t_share = min(U256_MAX, x['target'] * cp['m'])
        prev = -1
        for n in x['shares']:
            if n <= prev or n >= n_max or n == x['nonce']:
                raise RuleFail('9', 'order/range')
            prev = n
            ctr.shareHash += 1
            if int.from_bytes(ctr.sha256('shareHash', self.tid(x) + be64(n)), 'big') > t_share:
                raise RuleFail('9', 'T_share')


def check_window(cfg, rpc, clock_s, ctr, cache=True):
    """browser.md:35-56. rpc.call(method, params) -> reply TEXT; rpc.sleep(ms). Never raises on reply content."""
    def fail(rule, at, detail=None):
        return {'ok': False, 'rule': rule, 'at': at, 'detail': detail, 'frame': False, 'cancel': 4901,
                'log': {'code': -32019, 'data': {'rule': rule}}, 'rootReliance': False}

    kind = classify_reply(rpc.call('eth_blockNumber', []))
    h = _qty(kind[1]) if kind[0] == 'result' else None
    if h is None:
        return fail('viewIncomplete', None, {'eth_blockNumber': kind[0], 'why': kind[1] if kind[0] == 'malformed' else None})
    if h == 0:
        return {'ok': False, 'rule': 'viewNoBlocks', 'at': 0, 'message': NO_BLOCKS, 'frame': False, 'headersRequested': False,
                'deny': {'eth_requestAccounts': 4901, 'eth_sendTransaction': 4901}, 'recheckMs': 30000}
    pl = plan(h)
    params = [hex(pl['from']), hex(pl['count'])]
    retries = 0
    while True:
        kind = classify_reply(rpc.call('pocol_getHeaders', params))
        if kind[0] == 'result':
            res = kind[1]
            break
        if kind[0] == 'retry':
            if retries < MAX_RETRIES:
                retries += 1
                ctr.sleptMs += kind[1]
                rpc.sleep(kind[1])
                continue
            return fail('viewIncomplete', None, {'retriesExhausted': retries, 'last': kind[2]})
        if kind[0] == 'malformed':
            return fail('viewIncomplete', None, {'malformed': kind[1], 'retries': retries})
        return fail('viewIncomplete', None, {'error': kind[1], 'retries': retries})
    try:
        if type(res) is not str or not res.startswith('0x'):
            raise ValueError('result')
        outer = RLP.decode(bytes.fromhex(res[2:]))
    except Exception:
        return fail('viewIncomplete', None, 'reply is not an RLP list')
    if type(outer) is not list or len(outer) != pl['count']:
        return fail('viewIncomplete', None, {'items': len(outer) if type(outer) is list else None, 'count': pl['count']})
    hdrs = []
    for i, item in enumerate(outer):
        ctr.decodes += 1
        ctr.events.append(['decode', pl['from'] + i])
        try:
            hd = parse_header(item)
        except RuleFail as e:
            return fail('1', pl['from'] + i, e.detail)
        if hd['h'] != pl['from'] + i:
            return fail('viewIncomplete', pl['from'] + i, {'height': hd['h'], 'expected': pl['from'] + i})
        hdrs.append(hd)
    w = Window(cfg, ctr, cache)
    cp = cfg['cp']
    ceil = ceil_target(cp['target_g'])
    if pl['hasRef']:
        ref = hdrs[0]
        try:
            w.netid(ref)
            w.future(ref, clock_s)
        except RuleFail as e:
            return fail(e.rule, ref['h'], e.detail)
        parent, win = ref, hdrs[1:]
    else:
        parent, win = {'genesis': True, 'h': 0, 'ts': cp['g_ts']}, hdrs
    for x in win:
        try:
            w.window_header(x, parent, clock_s, ceil)
        except RuleFail as e:
            return fail(e.rule, x['h'], e.detail)
        parent = x
    mw = min_work(ceil)
    return {'ok': True, 'n': pl['n'], 'anchoredGenesis': pl['anchoredGenesis'], 'minWork': mw, 'frame': True,
            'message': message(pl['n'], pl['anchoredGenesis'], mw), 'rootReliance': False}


class ScriptedRpc:
    """Serves literal reply TEXTS in order; records method, params and the client time of every call."""

    def __init__(self, replies, t0=0):
        self.replies, self.t, self.calls = list(replies), t0, []

    def call(self, method, params):
        self.calls.append({'t': self.t, 'method': method, 'params': list(params)})
        if not self.replies:
            return '{"jsonrpc":"2.0","id":0,"error":{"code":-32603,"message":"unscripted"}}'
        return self.replies.pop(0)

    def sleep(self, ms):
        self.t += ms
