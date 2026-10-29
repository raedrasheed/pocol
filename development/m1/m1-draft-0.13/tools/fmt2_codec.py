"""fmt-2 record codec reference for M1 draft 0.13 (annex row E4, D104).
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

Baseline (browser.md:534-535; governance.md:34, 140):
  value = {fmt: 2, epoch, seq, tomb, b64}
  b64   = base64 of the concatenation of [be32(len k) || k || be32(len v) || v], one per
          pair, ordered by the key BYTES (D104). k and v are UTF-8.
  RECORD_BYTES_MAX = 4*ceil((SITE_STORE_MAX + 8*SITE_ENTRIES_MAX)/3) + 256 = 1442048.

Two explicit assumptions / open items, not hidden:
  * RA-U64 (P-E4-1 / proposed CR-E4-01): epoch and seq are stored as JSON numbers. A JS
    Number holds integers exactly only up to 2^53-1, while the baseline range of epoch
    is [1, 2^64-1] (browser.md:539). This reference accepts only epochs <= 2^53-1 in the
    VALUE ('epochNotJsSafe' otherwise). The name still carries the full hex16. Python
    arbitrary-precision ints prove nothing about Chrome Number handling.
  * Unpaired surrogates (P-E4-2 / proposed CR-E4-02): the bridge accounting uses
    TextEncoder lengths (lone surrogate -> U+FFFD, 3 bytes; implementation.md:206), but
    persistence is UTF-8. Two distinct JS keys '\\ud800' and '\\ud801' encode to the same
    bytes EF BF BD, so a UTF-8 round trip cannot preserve them (root Node observation,
    task-011). encode() therefore REQUIRES Unicode scalar values and raises
    'unpairedSurrogate'. Nothing is normalized or dropped. encode_textencoder() shows the
    lossy alternative and the collision. Which layer rejects such input (bridge or
    persistence), and with which reply, needs a baseline decision.
Model metric only: enc(name, value) = UTF-8 bytes of the name plus the compact JSON value.
That is NOT chrome.storage's getBytesInUse accounting (P-E4-3).
"""

import base64
import json
import re

SITE_STORE_MAX = 1048576          # browser.md:395
SITE_ENTRIES_MAX = 4096           # browser.md:402
KEY_MAX = 256                     # governance.md:25
VALUE_MAX = 61440
RECORD_BYTES_MAX = 4 * (-(-(SITE_STORE_MAX + 8 * SITE_ENTRIES_MAX) // 3)) + 256     # 1442048
SEQ_MAX = 2 ** 32 - 1
EPOCH_MAX = 2 ** 64 - 1
JS_SAFE_MAX = 2 ** 53 - 1
VALUE_KEYS = frozenset({'fmt', 'epoch', 'seq', 'tomb', 'b64'})
_B64_RE = re.compile(r'(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?')


class CodecError(ValueError):
    def __init__(self, reason, **detail):
        super().__init__(reason)
        self.reason = reason
        self.detail = detail


def _is_int(x):
    return type(x) is int


def has_unpaired_surrogate(s):
    return any(0xD800 <= ord(ch) <= 0xDFFF for ch in s)


def utf8_scalar(s):
    if type(s) is not str:
        raise CodecError('notString')
    if has_unpaired_surrogate(s):
        raise CodecError('unpairedSurrogate')
    return s.encode('utf-8')


def textencoder_bytes(s):
    """TextEncoder-compatible (lossy): each lone surrogate becomes U+FFFD."""
    return ''.join('�' if 0xD800 <= ord(ch) <= 0xDFFF else ch for ch in s).encode('utf-8')


def _be32(n):
    return n.to_bytes(4, 'big')


def serialize_pairs(d):
    """Strict canonical pair serialization. Raises CodecError on any limit breach."""
    if type(d) is not dict:
        raise CodecError('notDict')
    pairs = sorted(((utf8_scalar(k), utf8_scalar(v)) for k, v in d.items()), key=lambda p: p[0])
    if len(pairs) > SITE_ENTRIES_MAX:
        raise CodecError('entries', limit=SITE_ENTRIES_MAX)
    total = 0
    for k, v in pairs:
        if len(k) > KEY_MAX:
            raise CodecError('keyTooLong')
        if len(v) > VALUE_MAX:
            raise CodecError('valueTooLong')
        total += len(k) + len(v)
    if total > SITE_STORE_MAX:
        raise CodecError('quota', limit=SITE_STORE_MAX)
    return b''.join(_be32(len(k)) + k + _be32(len(v)) + v for k, v in pairs)


def serialize_pairs_textencoder(d):
    """The lossy alternative, for demonstration only: distinct keys may collide."""
    pairs = sorted(((textencoder_bytes(k), textencoder_bytes(v)) for k, v in d.items()), key=lambda p: p[0])
    return b''.join(_be32(len(k)) + k + _be32(len(v)) + v for k, v in pairs)


def parse_pairs(b):
    """Strict inverse: exact lengths, no trailing bytes, strict UTF-8, keys strictly
    ascending by bytes (so no duplicates), and the limits."""
    out, pos, prev, total = {}, 0, None, 0
    n = len(b)
    while pos < n:
        if pos + 4 > n:
            raise CodecError('truncated')
        lk = int.from_bytes(b[pos:pos + 4], 'big')
        pos += 4
        if pos + lk + 4 > n:
            raise CodecError('truncated')
        k = b[pos:pos + lk]
        pos += lk
        lv = int.from_bytes(b[pos:pos + 4], 'big')
        pos += 4
        if pos + lv > n:
            raise CodecError('truncated')
        v = b[pos:pos + lv]
        pos += lv
        if prev is not None:
            if k == prev:
                raise CodecError('duplicateKey')
            if k < prev:
                raise CodecError('unsortedKeys')
        prev = k
        if lk > KEY_MAX or lv > VALUE_MAX:
            raise CodecError('lengthLimit')
        try:
            ks, vs = k.decode('utf-8'), v.decode('utf-8')        # Python strict: rejects surrogates and overlongs
        except UnicodeDecodeError:
            raise CodecError('badUtf8')
        total += lk + lv
        out[ks] = vs
    if len(out) > SITE_ENTRIES_MAX:
        raise CodecError('entries')
    if total > SITE_STORE_MAX:
        raise CodecError('quota')
    return out


def b64_encode(b):
    return base64.b64encode(b).decode('ascii')


def b64_decode_strict(s):
    if type(s) is not str or not _B64_RE.fullmatch(s):
        raise CodecError('badBase64')
    b = base64.b64decode(s, validate=True)
    if b64_encode(b) != s:
        raise CodecError('nonCanonicalBase64')
    return b


def encode_value(E, seq, d, tomb=False):
    if not _is_int(E) or not 1 <= E <= EPOCH_MAX:
        raise CodecError('epochRange')
    if E > JS_SAFE_MAX:
        raise CodecError('epochNotJsSafe')                       # RA-U64
    if not _is_int(seq) or not 0 <= seq <= SEQ_MAX:
        raise CodecError('seqRange')
    if type(tomb) is not bool:
        raise CodecError('tombType')
    if tomb and d:
        raise CodecError('tombWithPairs')
    return {'fmt': 2, 'epoch': E, 'seq': seq, 'tomb': tomb, 'b64': b64_encode(serialize_pairs(d))}


def compact_json(v):
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False, sort_keys=True)


def enc_size(name, value):
    """Model metric (P-E4-3): UTF-8 bytes of name plus compact JSON value."""
    return len(name.encode('utf-8')) + len(compact_json(value).encode('utf-8'))


def decode_value(ver, value):
    """Validate a stored value for record version ver = (E, seq). Returns (dict, tomb)."""
    if type(value) is not dict:
        raise CodecError('notObject')
    if set(value) != VALUE_KEYS:
        raise CodecError('fields')
    if not (_is_int(value['fmt']) and value['fmt'] == 2):
        raise CodecError('fmt')
    ep, sq = value['epoch'], value['seq']
    if not (_is_int(ep) and 1 <= ep <= JS_SAFE_MAX and ep == ver[0]):
        raise CodecError('epoch')
    if not (_is_int(sq) and 0 <= sq <= SEQ_MAX and sq == ver[1]):
        raise CodecError('seq')
    if type(value['tomb']) is not bool:
        raise CodecError('tomb')
    d = parse_pairs(b64_decode_strict(value['b64']))
    if value['tomb'] and d:
        raise CodecError('tombWithPairs')
    return d, value['tomb']


def lookup_fmt2(items, net, addr, parse_record_name, gets_max=4):
    """Largest valid record (browser.md:541, 550-553) with the strict fmt-2 validator.
    parse_record_name is the strict 0.12 helper, passed in so the wiring is explicit."""
    vers = sorted(((parse_record_name(n, net, addr), n) for n in items if parse_record_name(n, net, addr)), reverse=True)
    gets = corrupt = 0
    reasons = []
    for ver, n in vers:
        if gets == gets_max:
            break
        gets += 1
        try:
            d, tomb = decode_value(ver, items[n])
            return {'failed': False, 'version': ver, 'dict': d, 'tomb': tomb, 'gets': gets, 'corrupt': corrupt, 'reasons': reasons}
        except CodecError as e:
            corrupt += 1
            reasons.append(e.reason)
    if corrupt:
        return {'failed': True, 'reason': 'corrupt', 'gets': gets, 'corrupt': corrupt, 'reasons': reasons}
    return {'failed': False, 'version': None, 'dict': {}, 'tomb': False, 'gets': 0, 'corrupt': 0, 'reasons': []}
