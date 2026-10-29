"""CR-M1-01 rev 2 reference: response envelope, EIP-1186 shape and decision table,
and the per-load request/restart/retry budget (R5-04, C17, C18).

Specification tooling only. IMPORTANT: this module does NOT verify Merkle-Patricia
proofs. Proof verification is represented by an explicit, fixture-supplied
`verifier` outcome ('present' with the proven leaf, 'absent', or 'invalid').
The fixtures therefore test the decision order around a verifier, not the
verifier. Real MPT verification is the MPT-1..12 tests of CR-M1-01 (Phase A).
"""

import json

import bridge_ref as B04                  # strict JSON parsing helpers (0.4)
from keccak import keccak256              # 0.2

EMPTY_TRIE_ROOT = '0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421'   # keccak256(0x80)
KECCAK_EMPTY = '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470'      # keccak256('')
RESPONSE_DEPTH_MAX = 16                   # FD:L1244 parseStrict for remote replies
RETRY_PER_REQUEST_MAX = 3                 # FD:L1004 precedent for -32021 (P for proofs)
RETRY_AFTER_MAX_MS = 10000                # P
LOAD_DELAY_MAX_MS = 20000                 # P
MAX_ATTEMPTS = 4                          # 0.2 section 4.2 step 4
PROOF_NODES_MAX, NODE_BYTES_MAX = 65, 564  # CR-M1-01 section 4 (PR-1)


def account_path(address_hex):
    """State-trie key: keccak256 of the 20-byte address."""
    a = bytes.fromhex(address_hex[2:])
    assert len(a) == 20
    return '0x' + keccak256(a).hex()


def storage_path(slot):
    """Storage-trie key: keccak256 of the 32-byte big-endian slot."""
    return '0x' + keccak256(int(slot).to_bytes(32, 'big')).hex()


# --- envelope (strict JSON-RPC 2.0, single response) ------------------------
def envelope(raw, request_id):
    """Return ('result', value) | ('error', obj) | ('fail', reason)."""
    if B04.json_depth(raw) > RESPONSE_DEPTH_MAX:
        return 'fail', 'recvDepth'
    try:
        msg = json.loads(raw, object_pairs_hook=B04._pairs, parse_int=B04._parse_int,
                         parse_float=B04.FloatTok, parse_constant=B04._no_constants)
    except (B04._Reject, ValueError):
        return 'fail', 'recvParse'
    if not isinstance(msg, dict):
        return 'fail', 'envelope.notObject'            # includes batch arrays
    if msg.get('jsonrpc') != '2.0':
        return 'fail', 'envelope.jsonrpc'
    rid = msg.get('id')
    if not (isinstance(rid, B04.IntTok) and int(rid) == request_id):
        return 'fail', 'envelope.id'
    has_r, has_e = 'result' in msg, 'error' in msg
    if has_r == has_e:
        return 'fail', 'envelope.resultXorError'
    if set(msg) - {'jsonrpc', 'id', 'result', 'error'}:
        return 'fail', 'envelope.extraField'
    if has_e:
        e = msg['error']
        if not (isinstance(e, dict) and isinstance(e.get('code'), B04.IntTok) and isinstance(e.get('message'), str)):
            return 'fail', 'envelope.errorShape'
        return 'error', e
    return 'result', msg['result']


def classify_error(e):
    """-32021 -> ('retry', ms) when retryAfterMs is valid; otherwise the attempt fails."""
    code = int(e['code'])
    if code == -32021:
        d = e.get('data')
        ms = d.get('retryAfterMs') if isinstance(d, dict) else None
        if isinstance(ms, B04.IntTok) and 0 <= int(ms) <= RETRY_AFTER_MAX_MS:
            return 'retry', int(ms)
        return 'attemptFail', 'rate.badRetryAfter'
    if code == -32017:
        return 'attemptFail', 'outsideKeff'
    return 'attemptFail', 'rpcError %d' % code


# --- EIP-1186 result shape --------------------------------------------------
def _hexq(v, max_bits):
    return isinstance(v, str) and B04.re.fullmatch(r'0x(0|[1-9a-fA-F][0-9a-fA-F]*)', v) is not None \
        and int(v, 16).bit_length() <= max_bits


def _h32(v):
    return isinstance(v, str) and B04.re.fullmatch(r'0x[0-9a-fA-F]{64}', v) is not None


def _proof_list(v):
    return isinstance(v, list) and len(v) <= PROOF_NODES_MAX and all(
        isinstance(x, str) and B04.re.fullmatch(r'0x([0-9a-fA-F]{2})+', x) and (len(x) - 2) // 2 <= NODE_BYTES_MAX
        for x in v)


def shape(result, W, keys):
    """Required (EIP-1186): balance, codeHash, nonce, storageHash, accountProof, storageProof.
    address is optional; if present it must equal W."""
    if not isinstance(result, dict):
        return 'shape.notObject'
    for f in ('balance', 'codeHash', 'nonce', 'storageHash', 'accountProof', 'storageProof'):
        if f not in result:
            return 'shape.missing.' + f
    if 'address' in result and (not isinstance(result['address'], str) or result['address'].lower() != W.lower()):
        return 'bind.address'
    if not (_hexq(result['balance'], 256) and _hexq(result['nonce'], 64) and _h32(result['codeHash'])
            and _h32(result['storageHash']) and _proof_list(result['accountProof'])):
        return 'shape.types'
    sp = result['storageProof']
    if not isinstance(sp, list) or len(sp) != len(keys):
        return 'bind.storageProofCount'
    for i, p in enumerate(sp):
        if not (isinstance(p, dict) and {'key', 'value', 'proof'} <= set(p)):
            return 'shape.storageProof[%d]' % i
        if not (_h32(p['key']) or _hexq(p['key'], 256)):     # padded or compact form (clients differ)
            return 'shape.storageProof[%d].key' % i
        if int(p['key'], 16) != keys[i]:
            return 'bind.key[%d]' % i
        if not (_hexq(p['value'], 256) and _proof_list(p['proof'])):
            return 'shape.storageProof[%d].types' % i
    return None


def decide(result, W, keys, state_root, verifier):
    """Decision order of CR-M1-01 rev 2 section 3 for one verified-envelope result.

    verifier = {'account': {'outcome': 'present'|'absent'|'invalid', 'leaf': {...}},
                'storage': [{'outcome': ..., 'value': '0x..'} per key]}
    Returns ('ok', values) | ('noWebsite', reason) | ('attemptFail', reason).
    """
    s = shape(result, W, keys)
    if s:
        return 'attemptFail', s
    acct = verifier['account']
    if not result['accountProof']:
        if state_root != EMPTY_TRIE_ROOT:
            return 'attemptFail', 'shape.emptyAccountProof'
        acct = {'outcome': 'absent'}                    # authenticated: the whole state trie is empty
    if acct['outcome'] == 'invalid':
        return 'attemptFail', 'bind.accountProof'
    if acct['outcome'] == 'absent':
        return 'noWebsite', 'accountAbsent'             # reported field values are not used
    leaf = acct['leaf']
    for f in ('nonce', 'balance', 'storageHash', 'codeHash'):
        if int(leaf[f], 16) != int(result[f], 16):
            return 'attemptFail', 'bind.leaf.' + f
    if result['codeHash'].lower() == KECCAK_EMPTY:
        return 'noWebsite', 'noCode'
    values = []
    for i, (p, v) in enumerate(zip(result['storageProof'], verifier['storage'])):
        if not p['proof']:
            if result['storageHash'].lower() != EMPTY_TRIE_ROOT:
                return 'attemptFail', 'shape.emptyStorageProof[%d]' % i
            v = {'outcome': 'absent'}
        if v['outcome'] == 'invalid':
            return 'attemptFail', 'bind.storageProof[%d]' % i
        proven = 0 if v['outcome'] == 'absent' else int(v['value'], 16)
        if proven != int(p['value'], 16):
            return 'attemptFail', 'bind.storageValue[%d]' % i
        values.append(proven)
    return 'ok', values


# --- per-load budget --------------------------------------------------------
def simulate(script):
    """script: list of per-send outcomes, consumed in order, for requests
    anchor, proof1, proof2 of each attempt. Outcome forms:
      {'ok': true} | {'retry': ms} | {'fail': reason} | {'noWebsite': true} | {'invariant': rule}
    Returns totals and the load result. A -32021 retry repeats the same request
    with the same anchor; it is not a restart."""
    pos, sends, delay, restarts = 0, 0, 0, -1
    log = []
    for _ in range(MAX_ATTEMPTS):
        restarts += 1
        failed = False
        for req in ('anchor', 'proof1', 'proof2'):
            tries = 0
            while True:
                if pos >= len(script):
                    raise ValueError('script exhausted')
                o = script[pos]
                pos += 1
                sends += 1
                log.append(req)
                if 'retry' in o:
                    tries += 1
                    if tries > RETRY_PER_REQUEST_MAX:          # 4th -32021 for this request: attempt fails
                        failed = True
                        break
                    if delay + o['retry'] > LOAD_DELAY_MAX_MS:  # cumulative wait cap for the load
                        return _sim(pos, script, 'unavailable', sends, delay, restarts, log)
                    delay += o['retry']
                    continue
                if 'fail' in o:
                    failed = True
                elif 'noWebsite' in o:
                    return _sim(pos, script, 'noWebsite', sends, delay, restarts, log)
                elif 'invariant' in o:
                    return _sim(pos, script, 'stateInvariant', sends, delay, restarts, log)
                break
            if failed:
                break
        if not failed:
            return _sim(pos, script, 'render', sends, delay, restarts, log)
    return _sim(pos, script, 'inconsistent', sends, delay, restarts, log)


def _sim(pos, script, result, sends, delay, restarts, log):
    if pos != len(script):
        raise ValueError('%d unused script steps' % (len(script) - pos))
    return {'result': result, 'sends': sends, 'delayMs': delay, 'restarts': restarts, 'log': log}


def worst_case_sends():
    """Upper bound: 4 attempts x 3 requests x (1 + 3 retries)."""
    return MAX_ATTEMPTS * 3 * (1 + RETRY_PER_REQUEST_MAX)
