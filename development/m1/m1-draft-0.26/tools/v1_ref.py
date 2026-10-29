"""HeaderNetCheck (RP header window) reference for M1 draft 0.26, row V1 (D77, D80-D84).
SPECIFICATION FIXTURE TOOLING ONLY. Python standard library only. NOT executed by the author.

Sources (Arabic baseline):
  reference/browser.md:22-72      window counts, client constants, steps 0-7, messages, RW/HC examples
  reference/consensus.md:48-71    encoding (UT, SignedTemplate, Header) and derivations
  reference/consensus.md:92-98    timing L(a), U(a), fallback stamp
  reference/consensus.md:113-118  work(t), nMax
  reference/consensus.md:126-137  fixed-width ASERT
  reference/consensus.md:151-161  H-pre items 1-9
  reference/validation.md:132-133 RW-unit, HC-unit;  :926 T_share = min(2^256-1, target*m) (D143)
  reference/validation.md:1051-1052, 1069-1081  NX window vectors

This is a model of the specification text. It is not the M3-TS model, not the extension, and not a
consensus implementation. Items 10-17 (execution and membership) are NOT evaluated, as browser.md:56 says.
Every hash, signature and nonce of the fixtures is computed here; nothing is copied from a node.
Conventions that the baseline does not fix are labelled P-V1-n in annex/V1-HEADERNETCHECK.md.
"""

import hashlib
import hmac

K = None        # Keccak-256 module (m1-draft-0.2/tools/keccak.py), bound by bind()
RLP = None      # strict RLP module (m1-draft-0.3/tools/rlp_strict.py), bound by bind()


def bind(keccak_mod, rlp_mod):
    global K, RLP
    K, RLP = keccak_mod, rlp_mod


TOP = 2 ** 256
U256_MAX = TOP - 1
VIEW_SLACK_S = 4                  # browser.md:30
PHI_VIEW_S = 10                   # browser.md:33
KVIEW = 12                        # browser.md:25 (b = max(1, h-12))
TAG = b'PoCol-tpl-v1'
UT_MAX, ST_MAX, HDR_MAX = 613, 682, 3067
SHARES_MAX = 256
NO_BLOCKS = 'الشبكة لم تنتج الكتلة 1 بعد؛ لا رأس للتحقق في RP'
WARN_MINWORK1 = 'حد العمل غير فعال لهذه الشبكة'
INT_W = {'chainId': 8, 'h': 8, 'a': 4, 'protocolVersion': 2, 'ts': 8, 'target': 32, 'gasLimit': 8, 'gasUsed': 8, 'baseFee': 32}
FIX_W = {'genesisHash': 32, 'parentHash': 32, 'stateRoot': 32, 'txRoot': 32, 'receiptsRoot': 32, 'logsBloom': 256, 'evidenceRoot': 32}
UT_FIELDS = ('tag', 'chainId', 'genesisHash', 'parentHash', 'h', 'a', 'protocolVersion', 'ts', 'target', 'stateRoot', 'txRoot',
             'receiptsRoot', 'logsBloom', 'gasLimit', 'gasUsed', 'baseFee', 'evidenceRoot', 'proposer')


# ------------------------------------------------------------------ integers and units

def ib(n):
    """Minimal big-endian bytes; 0 -> b'' (RLP 0x80)."""
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
    """P-V1-4: Arabic number agreement; source examples '10 رؤوس' and '13 رأسًا' (validation.md:1055-1056)."""
    if n == 1:
        return 'رأس واحد'
    if n == 2:
        return 'رأسين'
    if 3 <= n <= 10:
        return '%d رؤوس' % n
    return '%d رأسًا' % n


def message(n, anchored_genesis, minwork):
    """browser.md:51-54. The fixed parts are copied from the source; n, [حتى genesis] and the decimal fill it."""
    m = ('RP: مرتبط بالعمل والهوية لـ' + count_phrase(n) + (' حتى genesis' if anchored_genesis else '')
         + '، بعمل محتسب لا يقل عن ' + str(minwork) + ' لكل رأس (work(ceilTarget))، دون تحقق تنفيذ أو عضوية')
    if minwork == 1:
        m += '، ' + WARN_MINWORK1
    return m


# ------------------------------------------------------------------ fixed-width ASERT (consensus.md:126-137)

class Counters:
    def __init__(self):
        self.decodes = self.templateIds = self.asert = self.powHash = self.shareHash = self.ecrecover = 0
        self.maxAsertBits = 0
        self.events = []

    def as_dict(self):
        return {'headerDecodes': self.decodes, 'templateIds': self.templateIds, 'asert': self.asert, 'powHash': self.powHash,
                'shareHash': self.shareHash, 'ecrecover': self.ecrecover, 'maxAsertBits': self.maxAsertBits}


def asert(cp, p_ts, p_h, ctr=None):
    """target for the child of p. Returns None if |dt| >= 2^97 (consensus.md:129 precondition)."""
    if ctr is not None:
        ctr.asert += 1
    dt = (p_ts - cp['g_ts']) - cp['T_blk'] * p_h
    if not abs(dt) < 2 ** 97:
        return None
    num = dt * 65536
    e = num // cp['tau']                       # floor toward -inf
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


# ------------------------------------------------------------------ secp256k1 (recovery, RFC 6979 signing)

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
    """Deterministic ECDSA (RFC 6979, SHA-256), low s, v in {0, 1}; layout r || s || v (P-V1-5)."""
    z = int.from_bytes(z32, 'big')
    k = _rfc6979_k(x, z32)
    R = ec_mul(k, G)
    if R[0] >= N_ORDER:
        raise ValueError('R.x >= n: not representable with v in {0,1}')
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
    if not (1 <= r < N_ORDER and 1 <= s <= N_ORDER // 2 and v in (0, 1)):     # consensus.md:69
        return None
    alpha = (pow(r, 3, P_FIELD) + 7) % P_FIELD
    beta = pow(alpha, (P_FIELD + 1) // 4, P_FIELD)
    if beta * beta % P_FIELD != alpha:
        return None
    y = beta if beta & 1 == v else P_FIELD - beta
    z = int.from_bytes(z32, 'big')
    rinv = pow(r, -1, N_ORDER)
    return _add(ec_mul(-z * rinv % N_ORDER, G), ec_mul(s * rinv % N_ORDER, (r, y)))


# ------------------------------------------------------------------ derivations (consensus.md:58-65)

def sha256(b):
    return hashlib.sha256(b).digest()


def sig_msg(tid):
    return K.keccak256(b'\x19' + b'PoCol template' + b'\x0a' + tid)


def win_msg(tid, nonce, share_root):
    return K.keccak256(b'\x19' + b'PoCol winner' + b'\x0a' + tid + be64(nonce) + share_root)


def ut_list(f):
    out = []
    for name in UT_FIELDS:
        v = f[name]
        out.append(ib(v) if name in INT_W else v)
    return out


def build_header(f, shares, signer_x, nonce_rule):
    """f: UT field dict (proposer filled by the caller); nonce_rule: ('pow', None) smallest nonce with
    powHash <= target; ('fail', None) smallest nonce with powHash > target; ('fixed', n)."""
    ut = ut_list(f)
    ut_raw = RLP.encode(ut)
    tid = sha256(ut_raw)
    sig = sign(signer_x, sig_msg(tid)) if f['proposer'] else b''
    kind, val = nonce_rule
    if kind == 'fixed':
        nonce = val
    else:
        nonce = 0
        while True:
            ok = int.from_bytes(sha256(tid + be64(nonce)), 'big') <= f['target']
            if ok == (kind == 'pow'):
                break
            nonce += 1
    share_items = [be64(x) for x in shares]
    share_root = K.keccak256(RLP.encode(share_items))
    wsig = sign(signer_x, win_msg(tid, nonce, share_root))
    item = [[ut, sig], be64(nonce), share_items, wsig]
    return {'item': item, 'utRaw': ut_raw, 'tid': tid, 'nonce': nonce, 'shareRoot': share_root,
            'blockHash': K.keccak256(tid + be64(nonce) + share_root), 'powHash': sha256(tid + be64(nonce)),
            'sigMsg': sig_msg(tid), 'winMsg': win_msg(tid, nonce, share_root), 'sig': sig, 'winnerSig': wsig,
            'encoded': RLP.encode(item)}


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
    """Item 1 (encoding, widths, sizes). Raises RuleFail('1')."""
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
        else:                                                    # proposer: 20 bytes or empty
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
    def __init__(self, cfg, ctr):
        self.cfg, self.cp, self.ctr = cfg, cfg['cp'], ctr
        self._tid = {}

    def tid(self, hd):
        key = id(hd)
        if key not in self._tid:
            self.ctr.templateIds += 1
            self._tid[key] = sha256(hd['utRaw'])
        return self._tid[key]

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
        if int.from_bytes(sha256(self.tid(x) + be64(x['nonce'])), 'big') > x['target']:
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
            if int.from_bytes(sha256(self.tid(x) + be64(n)), 'big') > t_share:
                raise RuleFail('9', 'T_share')


def check_window(cfg, rpc, clock_s, ctr):
    """browser.md:35-56. rpc.call(method, params) -> reply dict; rpc.sleep(ms). Returns the outcome."""
    def fail(rule, at, detail=None):
        return {'ok': False, 'rule': rule, 'at': at, 'detail': detail, 'frame': False, 'cancel': 4901,
                'log': {'code': -32019, 'data': {'rule': rule}}, 'rootReliance': False}

    r = rpc.call('eth_blockNumber', [])
    h = _qty(r.get('result')) if type(r) is dict else None
    if h is None:
        return fail('viewIncomplete', None, 'eth_blockNumber reply')
    if h == 0:
        return {'ok': False, 'rule': 'viewNoBlocks', 'at': 0, 'message': NO_BLOCKS, 'frame': False, 'headersRequested': False,
                'deny': {'eth_requestAccounts': 4901, 'eth_sendTransaction': 4901}, 'recheckMs': 30000}
    pl = plan(h)
    params = [hex(pl['from']), hex(pl['count'])]
    retries = 0
    while True:
        r = rpc.call('pocol_getHeaders', params)
        err = r.get('error') if type(r) is dict else {'code': None}
        if err is None:
            break
        if err.get('code') == -32021 and retries < 3:
            retries += 1
            rpc.sleep(err['data']['retryAfterMs'])
            continue
        return fail('viewIncomplete', None, {'error': err.get('code'), 'retries': retries})
    res = r.get('result')
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
    w = Window(cfg, ctr)
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
    """Serves literal replies in order; records method, params and the client time of every call."""

    def __init__(self, replies, t0=0):
        self.replies, self.t, self.calls = list(replies), t0, []

    def call(self, method, params):
        self.calls.append({'t': self.t, 'method': method, 'params': list(params)})
        if not self.replies:
            return {'error': {'code': -32603, 'message': 'unscripted'}}
        return self.replies.pop(0)

    def sleep(self, ms):
        self.t += ms
