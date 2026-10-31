"""Read-only latest-block reward projection for the pinned PoCol network.

Uses existing node RPC only. No transaction, node change or contract mutation.
The result is a projection from the captured block's state, not a settlement
receipt: evidence, registry changes or PAY eligibility can change before H+D.
Sources: PoColSystemV1.sol, apply.rs, lp1-node/{assignment,header,hashes}.rs.
"""
from collections import Counter
from datetime import datetime, timezone
import hashlib
import re
import time

CHAIN_ID = 1599846715
GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
REGISTRY = '0x0000000000000000000000000000000000c0c001'
ESCROW = '0x0000000000000000000000000000000000c0c002'
CAPS = '0x0000000000000000000000000000000000c0c004'
ZERO = '0x' + '00' * 20
MAX_U256 = (1 << 256) - 1
MAX_MEMBERS = 64
MAX_CALLS = 512
MAX_SECONDS = 20
DEFAULT_FORK_SCHEDULE = ((1, 0),)


class RewardUnavailable(RuntimeError):
    pass


class SnapshotChanged(RewardUnavailable):
    pass


def require(ok, message):
    if not ok:
        raise RewardUnavailable(message)


def validated_fork_schedule(value):
    """Operator configuration, never an activation height learned from RPC."""
    require(isinstance(value, (tuple, list)) and 1 <= len(value) <= 2,
            'Invalid trusted fork schedule')
    entries = []
    for entry in value:
        require(isinstance(entry, (tuple, list)) and len(entry) == 2,
                'Invalid trusted fork entry')
        version, height = entry
        require(type(version) is int and type(height) is int and
                0 <= height < 1 << 64, 'Invalid trusted fork values')
        entries.append((version, height))
    require(entries[0] == (1, 0) and
            (len(entries) == 1 or entries[1][0] == 2 and entries[1][1] > 0),
            'Unsupported trusted fork schedule')
    return tuple(entries)


def active_version(height, fork_schedule):
    return next(version for version, start in reversed(fork_schedule) if start <= height)


# Keccak-256 (Ethereum padding 0x01), not FIPS SHA3-256. Standard-library only.
_RC = (0x1, 0x8082, 0x800000000000808a, 0x8000000080008000,
       0x808b, 0x80000001, 0x8000000080008081, 0x8000000000008009,
       0x8a, 0x88, 0x80008009, 0x8000000a, 0x8000808b,
       0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
       0x8000000000008002, 0x8000000000000080, 0x800a,
       0x800000008000000a, 0x8000000080008081, 0x8000000000008080,
       0x80000001, 0x8000000080008008)
_ROT = (0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39,
        41, 45, 15, 21, 8, 18, 2, 61, 56, 14)
_MASK = (1 << 64) - 1


def keccak256(data):
    state = [0] * 25
    padded = bytearray(data)
    padded.append(1)
    padded.extend(b'\0' * ((-len(padded)) % 136))
    padded[-1] |= 128
    rot = lambda v, n: ((v << n) | (v >> ((64 - n) % 64))) & _MASK
    for start in range(0, len(padded), 136):
        for i in range(17):
            state[i] ^= int.from_bytes(padded[start + i*8:start + (i+1)*8], 'little')
        for rc in _RC:
            c = [state[x] ^ state[x+5] ^ state[x+10] ^ state[x+15] ^ state[x+20] for x in range(5)]
            d = [c[(x-1) % 5] ^ rot(c[(x+1) % 5], 1) for x in range(5)]
            b = [0] * 25
            for x in range(5):
                for y in range(5):
                    i = x + 5*y
                    b[y + 5*((2*x+3*y) % 5)] = rot(state[i] ^ d[x], _ROT[i])
            for x in range(5):
                for y in range(5):
                    state[x+5*y] = b[x+5*y] ^ ((~b[(x+1) % 5+5*y]) & b[(x+2) % 5+5*y])
            state[0] ^= rc
    return b''.join(v.to_bytes(8, 'little') for v in state)[:32]


def rlp_encode(value):
    def prefix(size, short, long):
        if size < 56:
            return bytes([short + size])
        raw = size.to_bytes((size.bit_length() + 7)//8, 'big')
        return bytes([long + len(raw)]) + raw
    if isinstance(value, list):
        raw = b''.join(rlp_encode(v) for v in value)
        return prefix(len(raw), 192, 247) + raw
    require(isinstance(value, bytes), 'Invalid RLP value')
    if len(value) == 1 and value[0] < 128:
        return value
    return prefix(len(value), 128, 183) + value


def rlp_decode(raw):
    require(isinstance(raw, bytes) and 0 < len(raw) <= 32768, 'Invalid RLP size')
    def one(pos, depth=0):
        require(depth <= 6 and pos < len(raw), 'Malformed RLP')
        first = raw[pos]
        if first < 128:
            return raw[pos:pos+1], pos+1
        is_list = first >= 192
        short, long = (192, 247) if is_list else (128, 183)
        if first <= long:
            size, start = first-short, pos+1
        else:
            width = first-long
            require(width <= 4 and pos+1+width <= len(raw), 'Malformed RLP length')
            length = raw[pos+1:pos+1+width]
            require(length[0] != 0, 'Noncanonical RLP length')
            size, start = int.from_bytes(length, 'big'), pos+1+width
            require(size >= 56, 'Noncanonical long RLP')
        end = start+size
        require(end <= len(raw), 'Truncated RLP')
        if not is_list:
            value = raw[start:end]
            require(not (len(value) == 1 and value[0] < 128), 'Noncanonical RLP string')
            return value, end
        values = []
        while start < end:
            value, start = one(start, depth+1)
            require(start <= end and len(values) < 1024, 'Invalid RLP list')
            values.append(value)
        return values, end
    value, end = one(0)
    require(end == len(raw) and rlp_encode(value) == raw, 'Trailing or noncanonical RLP')
    return value


def hex_bytes(value, size=None, maximum=4194304):
    require(isinstance(value, str) and re.fullmatch(r'0x(?:[0-9a-fA-F]{2})*', value) is not None, 'Malformed hex data')
    require(len(value) <= 2 + 2*maximum, 'Oversized hex data')
    raw = bytes.fromhex(value[2:])
    require(size is None or len(raw) == size, 'Invalid hex width')
    return raw


def address(value, allow_zero=False):
    raw = hex_bytes(value, 20)
    require(allow_zero or any(raw), 'Zero address')
    return '0x' + raw.hex()


def quantity(value):
    require(isinstance(value, str) and re.fullmatch(r'0x(?:0|[1-9a-fA-F][0-9a-fA-F]*)', value) is not None, 'Malformed quantity')
    number = int(value, 16)
    require(number <= MAX_U256, 'Quantity overflow')
    return number


def decode_header(raw, fork_schedule=DEFAULT_FORK_SCHEDULE):
    fork_schedule = validated_fork_schedule(fork_schedule)
    require(len(raw) <= 3067, 'Header size limit')
    top = rlp_decode(raw)
    require(isinstance(top, list) and len(top) == 4, 'Header arity')
    st = top[0]
    require(isinstance(st, list) and len(st) == 2, 'Template arity')
    ut = st[0]
    require(isinstance(ut, list) and len(ut) == 18 and ut[0] == b'PoCol-tpl-v1', 'Unsupported header')
    def integer(index, width):
        value = ut[index]
        require(isinstance(value, bytes) and len(value) <= width and (not value or value[0]), 'Noncanonical header integer')
        return int.from_bytes(value, 'big')
    for i in (2, 3, 9, 10, 11, 16):
        require(isinstance(ut[i], bytes) and len(ut[i]) == 32, 'Invalid header hash')
    require(ut[1] and integer(1, 8) == CHAIN_ID and '0x'+ut[2].hex() == GENESIS, 'Header network mismatch')
    height, version = integer(4, 8), integer(6, 2)
    require(version == active_version(height, fork_schedule),
            'Header version differs from trusted fork schedule')
    require(isinstance(ut[12], bytes) and len(ut[12]) == 256, 'Invalid bloom')
    require(isinstance(ut[17], bytes) and len(ut[17]) in (0, 20), 'Invalid proposer')
    require(isinstance(st[1], bytes) and len(st[1]) == (65 if ut[17] else 0), 'Invalid template signature shape')
    require(isinstance(top[1], bytes) and len(top[1]) == 8, 'Invalid nonce')
    require(isinstance(top[3], bytes) and len(top[3]) == 65, 'Invalid winner signature')
    require(isinstance(top[2], list) and len(top[2]) <= 256, 'Invalid shares')
    require(all(isinstance(x, bytes) and len(x) == 8 for x in top[2]), 'Invalid share nonce')
    shares = [int.from_bytes(x, 'big') for x in top[2]]
    nonce = int.from_bytes(top[1], 'big')
    require(shares == sorted(set(shares)) and nonce not in shares, 'Duplicate or unordered shares')
    tid = hashlib.sha256(rlp_encode(ut)).digest()
    share_root = keccak256(rlp_encode(top[2]))
    target = integer(8, 32)
    require(target > 0, 'Zero target')
    return {'height': height, 'protocolVersion': version, 'attempt': integer(5, 4), 'target': target,
            'nonce': nonce, 'shares': shares, 'templateId': tid,
            'hash': '0x'+keccak256(tid+top[1]+share_root).hex(),
            'parentHash': '0x'+ut[3].hex(), 'proposer': '0x'+ut[17].hex(),
            'signed': bool(ut[17]), 'seal': keccak256(tid+top[1]),
            'stateRoot': '0x'+ut[9].hex(), 'timestamp': integer(7, 8),
            'gasLimit': integer(13, 8), 'gasUsed': integer(14, 8), 'baseFee': integer(15, 32)}


def _result(query, method, params):
    result = query(method, params)
    require(isinstance(result, dict) and result.get('jsonrpc') == '2.0' and 'result' in result and 'error' not in result, 'Node read failed: '+method)
    return result['result']


def _word(value):
    require(type(value) is int and 0 <= value <= MAX_U256, 'ABI argument overflow')
    return value.to_bytes(32, 'big')


def _call(query, to, signature, args, height, words):
    data = keccak256(signature.encode())[:4] + b''.join(_word(x) for x in args)
    raw = hex_bytes(_result(query, 'eth_call', [{'to': to, 'data': '0x'+data.hex(), 'gas': '0x1e8480'}, hex(height)]), words*32)
    return [int.from_bytes(raw[i:i+32], 'big') for i in range(0, len(raw), 32)]


def _record(query, miner, height):
    values = _call(query, REGISTRY, 'recordWords(address)', [int(miner, 16)], height, 4)
    require(values[1] < 1 << 160, 'Malformed reward address')
    return values


def reconstruct_owners(query, header, cp, fork_schedule=DEFAULT_FORK_SCHEDULE):
    """Match parent-state Membership and the rewarded block's effective mode."""
    fork_schedule = validated_fork_schedule(fork_schedule)
    h = header['height']
    require(h > 0, 'Genesis has no reward')
    require(header['protocolVersion'] == active_version(h, fork_schedule),
            'Header version differs from trusted fork schedule')
    parent_height = h-1
    if parent_height == 0:
        require(header['parentHash'] == GENESIS, 'Wrong genesis parent')
        parent_seal = keccak256(bytes.fromhex(GENESIS[2:]))
    else:
        parent = _result(query, 'pocol_getBlockRlp', [hex(parent_height)])
        require(isinstance(parent, dict), 'Missing parent header')
        decoded = decode_header(hex_bytes(parent.get('header'), maximum=3067), fork_schedule)
        require(decoded['height'] == parent_height and decoded['hash'] == header['parentHash'], 'Parent header mismatch')
        parent_seal = decoded['seal']
    lengths = _call(query, REGISTRY, 'lengths()', [], parent_height, 4)
    n = lengths[1]
    require(0 < n <= cp[18] <= MAX_MEMBERS, 'Unsupported active membership size')
    # CP24 is W_act in blocks. CP7 is dFbWait in seconds, not liveness.
    cutoff = max(0, h-cp[24])
    live, active, seen = [], [], set()
    for i in range(n):
        value = _call(query, REGISTRY, 'activeAt(uint256)', [i], parent_height, 1)[0]
        require(0 < value < 1 << 160 and value != (1 << 160)-2 and not 0xc0c001 <= value <= 0xc0c0ff, 'Invalid active miner')
        miner = '0x'+value.to_bytes(20, 'big').hex()
        require(miner not in seen, 'Duplicate active miner')
        seen.add(miner)
        record = _record(query, miner, parent_height)
        require(record[0] != 0 and record[0] & 255 == 1, 'Invalid active record')
        if (record[0] >> 8) & 255:
            continue
        live.append(miner)
        if ((record[0] >> 88) & ((1 << 64)-1)) >= cutoff or record[3] >> 192 >= cutoff:
            active.append(miner)
    members = active or live
    require(bool(members), 'No nonce owners')
    seed = hashlib.sha256(b'PoColNonce'+parent_seal+h.to_bytes(8, 'big')+header['attempt'].to_bytes(4, 'big')).digest()
    tickets = sorted((hashlib.sha256(seed+bytes.fromhex(m[2:])).digest(), m) for m in members)
    require(len(set(t[0] for t in tickets)) == len(tickets), 'Nonce ticket collision')
    full_range = header['protocolVersion'] == 2 or cp[4] == 1
    maximum = 1 << 64 if full_range else min(1 << 64, ((1 << 256)//(header['target']+1))*cp[5])
    require(maximum > 0, 'Empty nonce space')
    owners = []
    for nonce in header['shares']+[header['nonce']]:
        require(0 <= nonce < maximum, 'Nonce outside assigned domain')
        matches = [m for i, (_, m) in enumerate(tickets) if i*maximum//len(tickets) <= nonce < (i+1)*maximum//len(tickets)]
        require(len(matches) == 1, 'Unassigned nonce')
        owners.append(matches[0])
    return owners


def snapshot(query, *, fork_schedule=DEFAULT_FORK_SCHEDULE):
    """Return a bounded, block-anchored projection, or raise RewardUnavailable."""
    fork_schedule = validated_fork_schedule(fork_schedule)
    started = time.monotonic()
    calls = 0
    raw_query = query
    def bounded(method, params):
        nonlocal calls
        calls += 1
        require(calls <= MAX_CALLS and time.monotonic()-started <= MAX_SECONDS, 'Reward read budget exceeded')
        answer = raw_query(method, params)
        require(time.monotonic()-started <= MAX_SECONDS, 'Reward read timed out')
        return answer
    query = bounded
    params = _result(query, 'pocol_getParams', [])
    require(isinstance(params, dict) and params.get('chainId') == CHAIN_ID and params.get('genesisHash', '').lower() == GENESIS, 'Wrong PoCol network')
    # activeVersion describes the moving head, not necessarily the block captured
    # below. A head advancing across H must not change that block's rules.
    require(type(params.get('activeVersion')) is int and
            params['activeVersion'] in {v for v, _ in fork_schedule},
            'Unsupported active consensus version')
    advertised = params.get('forkSchedule')
    if advertised is None:
        require(fork_schedule == DEFAULT_FORK_SCHEDULE,
                'Node did not advertise the configured fork schedule')
    else:
        require(validated_fork_schedule(advertised) == fork_schedule,
                'Node fork schedule differs from trusted configuration')
    block = _result(query, 'eth_getBlockByNumber', ['latest', False])
    require(isinstance(block, dict), 'No latest block')
    h = quantity(block.get('number'))
    require(0 < h < (1 << 64)-65536, 'Invalid block height')
    block_hash = '0x'+hex_bytes(block.get('hash'), 32).hex()
    pocol = block.get('pocol')
    require(isinstance(pocol, dict) and pocol.get('final') is False, 'Final block has no new reward pot')
    encoded = _result(query, 'pocol_getBlockRlp', [hex(h)])
    require(isinstance(encoded, dict), 'Missing block header')
    header = decode_header(hex_bytes(encoded.get('header'), maximum=3067), fork_schedule)
    require(header['height'] == h and header['hash'] == block_hash and header['parentHash'] == block.get('parentHash', '').lower(), 'Block header mismatch')
    require(type(pocol.get('shares')) is int and pocol['shares'] == len(header['shares']), 'Share count mismatch')
    require(pocol.get('signed') is header['signed'] and pocol.get('proposer', '').lower() == header['proposer'], 'Proposer mismatch')
    require(pocol.get('attempt') == header['attempt'], 'Attempt mismatch')
    if 'templateId' in pocol:
        require(pocol['templateId'].lower() == '0x'+header['templateId'].hex(), 'Template mismatch')
    winner = address(block.get('miner'))
    cp = {i: _call(query, CAPS, 'get(uint256)', [i], h, 1)[0] for i in (4, 5, 8, 9, 10, 11, 13, 14, 15, 18, 24)}
    require(cp[4] in (0, 1) and 0 < cp[5] <= 255 and 0 < cp[24] < 1 << 32, 'Invalid nonce configuration')
    require(0 < cp[8] <= 65535 and 0 < cp[9] <= 255 and 0 < cp[10] <= 256 and 1 < cp[11] <= 65535, 'Invalid reward configuration')
    require(cp[13]+cp[14] <= 10000 and 0 < cp[15] <= MAX_U256 and len(header['shares']) <= cp[10], 'Invalid reward caps')
    amount, meta, stored_winner = _call(query, ESCROW, 'pending(uint256)', [h], h, 3)
    expected_meta = int(header['proposer'], 16) << 8 if header['signed'] else 1
    require(meta == expected_meta and stored_winner == 0 and amount >= cp[15], 'Missing or inconsistent pending pot')
    require(amount * max(cp[13], cp[14], cp[10]+1) <= MAX_U256, 'Reward arithmetic overflow')
    owners = reconstruct_owners(query, header, cp, fork_schedule)
    require(owners[-1] == winner, 'Winner assignment mismatch')
    counts = Counter(owners)
    total = len(owners)
    wp, pp = amount*cp[13]//10000, amount*cp[14]//10000
    rest = amount-wp-pp
    denominator = max(total, cp[8]*cp[9])
    proposer = header['proposer'] if header['signed'] else winner
    payments = [(winner, wp), (proposer, pp)] + [(m, rest*c//denominator) for m, c in counts.items()]
    records, accounts, amounts, rewards = {}, {}, Counter(), {}
    escrow_balance = quantity(_result(query, 'eth_getBalance', [ESCROW, hex(h)]))
    require(escrow_balance >= amount, 'Reward escrow underfunded')
    for miner, payment in payments:
        if payment == 0:
            continue
        if miner not in records:
            records[miner] = _record(query, miner, h)
        record = records[miner]
        if record[0] == 0 or (record[0] >> 8) & 255:
            continue
        reward = '0x'+record[1].to_bytes(20, 'big').hex()
        if reward == ZERO:
            continue
        if reward not in accounts:
            nonce = quantity(_result(query, 'eth_getTransactionCount', [reward, hex(h)]))
            code = b'' if nonce else hex_bytes(_result(query, 'eth_getCode', [reward, hex(h)]), maximum=65536)
            balance = escrow_balance if reward == ESCROW else quantity(_result(query, 'eth_getBalance', [reward, hex(h)]))
            accounts[reward] = [bool(nonce or code), balance]
        durable, balance = accounts[reward]
        if not durable or escrow_balance < payment or (reward != ESCROW and balance+payment > MAX_U256):
            continue
        if reward != ESCROW:
            escrow_balance -= payment
            accounts[reward][1] += payment
        amounts[miner] += payment
        rewards[miner] = reward
    check = _result(query, 'eth_getBlockByNumber', [hex(h), False])
    if not isinstance(check, dict) or check.get('hash', '').lower() != block_hash:
        raise SnapshotChanged('Canonical block changed; reload the reward snapshot')
    beneficiaries = [{'minerId': m, 'rewardAddress': rewards[m], 'amountWei': str(amounts[m])} for m in sorted(amounts) if amounts[m] > 0]
    paid = sum(amounts.values())
    require(0 <= paid <= amount, 'Reward accounting mismatch')
    return {'schema': 'pocol-block-reward-v1', 'chainId': CHAIN_ID, 'genesisHash': GENESIS,
            'source': 'https://explorer.pocol.net',
            'fetchedAt': datetime.now(timezone.utc).isoformat(timespec='milliseconds').replace('+00:00', 'Z'),
            'blockHeight': h, 'blockHash': block_hash, 'settlementHeight': h+cp[11],
            'subsidyWei': str(cp[15]), 'grossRewardWei': str(amount),
            'projectedPaidWei': str(paid), 'projectedBurnedWei': str(amount-paid),
            'beneficiaryCount': len(beneficiaries), 'beneficiaries': beneficiaries,
            'status': 'pending', 'shareCount': len(header['shares'])}
