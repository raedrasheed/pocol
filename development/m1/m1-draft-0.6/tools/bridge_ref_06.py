"""Bridge and proof-envelope harness, draft 0.6 (R6-02, C19). Specification tooling only.

Builds on m1-draft-0.4/tools/bridge_ref.py and m1-draft-0.5/tools/bridge_ref_05.py
(imported, not copied). Change: integer fields (bridge id, JSON-RPC response id,
error.code, retryAfterMs) use VALUE semantics: a JSON number whose double value
is finite and integral (and in range where a range applies). Booleans are never
numbers. The httpsUrl oracle is read from m1-draft-0.6/results/url-oracle-0.6.json.
"""

import json
import math
from pathlib import Path

import bridge_ref as B04
import bridge_ref_05 as B05
import proof_response_ref as PR

ORACLE_06 = Path(__file__).resolve().parent.parent / 'results' / 'url-oracle-0.6.json'
ID_MAX = 2 ** 32 - 1


def integral_value(v):
    """Return the integral value of a parsed JSON number, or None (bool, non-number,
    non-finite, non-integral)."""
    if isinstance(v, bool) or not isinstance(v, (B04.IntTok, B04.FloatTok)):
        return None
    if isinstance(v, B04.FloatTok):
        if not math.isfinite(v) or not float(v).is_integer():
            return None
        return int(v)                       # -0.0 -> 0
    return int(v)


def valid_id(v):
    n = integral_value(v)
    return n is not None and 0 <= n <= ID_MAX


B04.valid_id = valid_id                     # check() resolves valid_id at call time
check = B04.check
Bucket = B04.Bucket
Pending = B05.Pending
OracleMissing = B05.OracleMissing


def install_oracle(path=ORACLE_06):
    loaded = B05.load_oracle(path)
    B05.install_oracle(loaded[0] if loaded else {})
    return loaded


def envelope(raw, request_id):
    """CR-M1-01 rev 2 section 3.2 with value-semantics id and error.code."""
    if B04.json_depth(raw) > PR.RESPONSE_DEPTH_MAX:
        return 'fail', 'recvDepth'
    try:
        msg = json.loads(raw, object_pairs_hook=B04._pairs, parse_int=B04._parse_int,
                         parse_float=B04.FloatTok, parse_constant=B04._no_constants)
    except (B04._Reject, ValueError):
        return 'fail', 'recvParse'
    if not isinstance(msg, dict):
        return 'fail', 'envelope.notObject'
    if msg.get('jsonrpc') != '2.0':
        return 'fail', 'envelope.jsonrpc'
    if integral_value(msg.get('id')) != request_id:
        return 'fail', 'envelope.id'
    has_r, has_e = 'result' in msg, 'error' in msg
    if has_r == has_e:
        return 'fail', 'envelope.resultXorError'
    if set(msg) - {'jsonrpc', 'id', 'result', 'error'}:
        return 'fail', 'envelope.extraField'
    if has_e:
        e = msg['error']
        if not (isinstance(e, dict) and integral_value(e.get('code')) is not None and isinstance(e.get('message'), str)):
            return 'fail', 'envelope.errorShape'
        return 'error', e
    return 'result', msg['result']


def classify_error(e):
    code = integral_value(e['code'])
    if code == -32021:
        d = e.get('data')
        ms = integral_value(d.get('retryAfterMs')) if isinstance(d, dict) else None
        if ms is not None and 0 <= ms <= PR.RETRY_AFTER_MAX_MS:
            return 'retry', ms
        return 'attemptFail', 'rate.badRetryAfter'
    if code == -32017:
        return 'attemptFail', 'outsideKeff'
    return 'attemptFail', 'rpcError %d' % code
