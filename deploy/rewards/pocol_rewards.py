"""Read-only latest-block reward projection for the pinned PoCol network.

Uses existing node RPC only. No transaction, node change or contract mutation.
The result is a projection from the captured block's state, not a settlement
receipt: evidence, registry changes or PAY eligibility can change before H+D.
Sources: PoColSystemV1.sol, apply.rs, lp1-node/{assignment,header,hashes}.rs.
A v3 (spec-2) network is read only with a trusted configuration (snapshot_v3, validated_network; V3-PR-G, dormant
until V3-PR-I); without one every function keeps the spec-1 Mainnet behaviour.
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


def _lower(value):
    """A node text field in lower case, or None when it is not text (so that a comparison with it fails closed)."""
    return value.lower() if isinstance(value, str) else None


# The highest consensus version these tools verify (pocold profile MAX_FORK_VERSION).
MAX_FORK_VERSION = 2


def validated_fork_schedule(value, max_version=MAX_FORK_VERSION):
    """Operator configuration, never an activation height learned from RPC.

    The generic runtime shape of pocold's configured_forks: (1, 0) first, then
    versions 2, 3, ... in order up to max_version at strictly increasing heights.
    max_version exists for tests of later versions; callers use the default."""
    require(isinstance(value, (tuple, list)) and 1 <= len(value) <= 16,
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
            all(v == i + 1 and v <= max_version for i, (v, _) in enumerate(entries)) and
            all(a[1] < b[1] for a, b in zip(entries, entries[1:])),
            'Unsupported trusted fork schedule')
    return tuple(entries)


def full_nonce_rule(version, nonce_mode):
    """pocold chain::full_nonce_rule: v2 introduced the full u64 nonce domain and
    every later version keeps it, so the test is >= 2, not == 2."""
    return version >= 2 or nonce_mode == 1


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


def rlp_decode(raw, limit=32768, max_items=1024):
    require(isinstance(raw, bytes) and 0 < len(raw) <= limit, 'Invalid RLP size')
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
            require(start <= end and len(values) < max_items, 'Invalid RLP list')
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


def decode_header(raw, fork_schedule=DEFAULT_FORK_SCHEDULE, network=None):
    """network is None for the pinned Mainnet (spec 1); a v3 network is a trusted validated_network value."""
    fork_schedule = validated_fork_schedule(fork_schedule)
    chain_id, genesis = (CHAIN_ID, GENESIS) if network is None else (network.chain_id, network.genesis)
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
    require(ut[1] and integer(1, 8) == chain_id and '0x'+ut[2].hex() == genesis, 'Header network mismatch')
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
            'gasLimit': integer(13, 8), 'gasUsed': integer(14, 8), 'baseFee': integer(15, 32),
            'txRoot': ut[10], 'evidenceRoot': ut[16]}


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
    # protocolVersion here is the header's fork version (item 6), not the wire version.
    full_range = full_nonce_rule(header['protocolVersion'], cp[4])
    maximum = 1 << 64 if full_range else min(1 << 64, ((1 << 256)//(header['target']+1))*cp[5])
    require(maximum > 0, 'Empty nonce space')
    owners = []
    for nonce in header['shares']+[header['nonce']]:
        require(0 <= nonce < maximum, 'Nonce outside assigned domain')
        matches = [m for i, (_, m) in enumerate(tickets) if i*maximum//len(tickets) <= nonce < (i+1)*maximum//len(tickets)]
        require(len(matches) == 1, 'Unassigned nonce')
        owners.append(matches[0])
    return owners


def snapshot(query, *, fork_schedule=DEFAULT_FORK_SCHEDULE, network=None):
    """Return a bounded, block-anchored projection, or raise RewardUnavailable.

    Without `network` this is the pinned Mainnet (spec 1) projection, schema pocol-block-reward-v1. A v3 (spec-2)
    network is read only with a trusted `network` configuration (validated_network) and returns snapshot_v3's
    pocol-block-reward-v3 projection."""
    if network is not None:
        return snapshot_v3(query, fork_schedule=fork_schedule, network=network)
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
    require(isinstance(params, dict) and params.get('chainId') == CHAIN_ID and _lower(params.get('genesisHash')) == GENESIS, 'Wrong PoCol network')
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
    require(header['height'] == h and header['hash'] == block_hash and header['parentHash'] == _lower(block.get('parentHash')), 'Block header mismatch')
    require(type(pocol.get('shares')) is int and pocol['shares'] == len(header['shares']), 'Share count mismatch')
    require(pocol.get('signed') is header['signed'] and _lower(pocol.get('proposer')) == header['proposer'], 'Proposer mismatch')
    require(pocol.get('attempt') == header['attempt'], 'Attempt mismatch')
    if 'templateId' in pocol:
        require(_lower(pocol['templateId']) == '0x'+header['templateId'].hex(), 'Template mismatch')
    winner = address(block.get('miner'))
    cp = {i: _call(query, CAPS, 'get(uint256)', [i], h, 1)[0] for i in (4, 5, 8, 9, 10, 11, 13, 14, 15, 16, 18, 24)}
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
    # Earn-in (EARN-IN-SPEC.md P1): a payment to an ACCRUING record (bondState 4) first fills its bond up
    # to B_reg (cp[16]) inside Escrow; only the excess reaches the reward address.
    bonds, accrued = {}, Counter()
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
        if (record[0] >> 16) & 255 == 4:
            bond = bonds.setdefault(miner, record[2])
            add = min(payment, max(cp[16] - bond, 0))
            bonds[miner] += add
            accrued[miner] += add
            rewards[miner] = reward
            payment -= add
            if payment == 0:
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
    if not isinstance(check, dict) or _lower(check.get('hash')) != block_hash:
        raise SnapshotChanged('Canonical block changed; reload the reward snapshot')
    beneficiaries = [dict({'minerId': m, 'rewardAddress': rewards[m], 'amountWei': str(amounts[m])},
                          **({'bondAccruedWei': str(accrued[m])} if accrued[m] else {}))
                     for m in sorted(set(amounts) | set(accrued)) if amounts[m] > 0 or accrued[m] > 0]
    paid = sum(amounts.values()) + sum(accrued.values())
    require(0 <= paid <= amount, 'Reward accounting mismatch')
    return {'schema': 'pocol-block-reward-v1', 'chainId': CHAIN_ID, 'genesisHash': GENESIS,
            'source': 'https://explorer.pocol.net',
            'fetchedAt': datetime.now(timezone.utc).isoformat(timespec='milliseconds').replace('+00:00', 'Z'),
            'blockHeight': h, 'blockHash': block_hash, 'settlementHeight': h+cp[11],
            'subsidyWei': str(cp[15]), 'grossRewardWei': str(amount),
            'projectedPaidWei': str(paid), 'projectedBurnedWei': str(amount-paid),
            'projectedBondWei': str(sum(accrued.values())),
            'beneficiaryCount': len(beneficiaries), 'beneficiaries': beneficiaries,
            'status': 'pending', 'shareCount': len(header['shares'])}


# ---------------------------------------------------------------------------------------------------------------------
# v3 (spec-2) networks: V3-PR-G. Dormant until V3-PR-I: a v3 genesis is read only with a trusted operator configuration
# (validated_network), exactly as a fork schedule is. Sources: PoColSystemV3.sol (RewardEscrow closeBlock/settleOne/
# distribute/payTo/burnBond/invalidate, MinerRegistry punish/payee, ShareLedger), lp4-execution apply.rs (phase order)
# and issuance.rs (M3), pocold chain.rs (membership, evidence_effects, late shares), V3-DECISIONS.md D-V3-01/02/10/14/
# 15/16/17/19/20, and the independent model prototypes/lp4-execution/tests/v3_model.py.
#
# The phase order of a v3 block h (apply.rs): phase 0 recordShares/recordResult(h-1), the body's evidence in order
# (kind 3 recordLate; kind 1/2 invalidate then punish with the block's signed proposer as reporter), markClose,
# pruneShares, settle(h-D), beginBlock; the user transactions; phase 2 issuance(h) and closeBlock(tips, issued).
# ---------------------------------------------------------------------------------------------------------------------
from collections import namedtuple

# The GenesisSpec version a release build decodes (genesis_spec.rs SPEC_VERSION); 2 is the v3 genesis (CP schema v2),
# decoded only by a v3-preview build until activation. CP item count of each version's schema.
SPEC_VERSION = 1
V3_SPEC_VERSION = 2
CP_ITEMS = {1: 52, 2: 64}
# sysCodeHash of the v3 program set: keccak256(RLP([C0C001..C0C005 runtimes of lp4-execution/tests/data/system-v3.json,
# 0x80 for PAY])) (apply.rs SystemPrograms::sys_code_hash); test_pocol_rewards recomputes it from that file.
V3_SYS_CODE_HASH = '0x70d4a3fd1b3ea222cef3e952b05f57c20c04cd2a2014f09472d14ccb0c0da5d7'
LEDGER = '0x0000000000000000000000000000000000c0c003'
MAX_MEMBERS_V3 = 256
MAX_CALLS_V3 = 8192
MAX_SECONDS_V3 = 60
MAX_SPEC_BYTES = 262144
MAX_BODY_BYTES = 4194304
MAX_LEDGER_OWNERS = 65536
# RewardEscrow pending words (V3-16): meta INVALID bit 168, winner word WIN_ACCRUING bit 160.
INVALID = 1 << 168
WIN_ACCRUING = 1 << 160
LOW64 = (1 << 64) - 1
LOW160 = (1 << 160) - 1

Network = namedtuple('Network', 'chain_id genesis spec_version source')


def validated_network(value):
    """Operator configuration of a v3 network, never learned from RPC: {"chainId": int, "genesisHash": "0x<64 hex>",
    "specVersion": 2, "v3": true} and optionally "source" (an https URL echoed in the answer). The v3 rules are dormant
    until V3-PR-I, so a spec-2 genesis is accepted only through this explicit configuration; the default (None) is the
    pinned spec-1 Mainnet. A Network value is validated again."""
    if isinstance(value, Network):
        value = {'chainId': value.chain_id, 'genesisHash': value.genesis, 'specVersion': value.spec_version, 'v3': True,
                 **({'source': value.source} if value.source is not None else {})}
    require(isinstance(value, dict) and {'chainId', 'genesisHash', 'specVersion', 'v3'} <= set(value) <=
            {'chainId', 'genesisHash', 'specVersion', 'v3', 'source'}, 'Invalid trusted network configuration')
    chain_id, genesis, version = value['chainId'], value['genesisHash'], value['specVersion']
    require(type(chain_id) is int and 0 < chain_id < 1 << 53, 'Invalid trusted network chain ID')
    require(isinstance(genesis, str) and re.fullmatch(r'0x[0-9a-f]{64}', genesis) is not None and genesis != GENESIS,
            'Invalid trusted network genesis hash')
    require(type(version) is int and version == V3_SPEC_VERSION and value['v3'] is True,
            'Only a spec-2 (v3) network is configured explicitly; spec 1 is the pinned Mainnet')
    source = value.get('source')
    require(source is None or (isinstance(source, str) and len(source) <= 200 and
                               re.fullmatch(r'https://[A-Za-z0-9.-]+(?::[0-9]{1,5})?(?:/[A-Za-z0-9._~/-]*)?', source)),
            'Invalid trusted network source')
    return Network(chain_id, genesis, version, source)


def v3_active(height, fork_schedule, spec_version):
    """pocold Spec::v3_active: active_version(h) >= 3 || genesis.spec_version >= 2 (V3-DECISIONS fork machinery)."""
    return active_version(height, fork_schedule) >= 3 or spec_version >= 2


def decode_genesis_spec(raw, versions=(V3_SPEC_VERSION,)):
    """[specVersion, chainId, CP[...], allocRoot, M_0List, sysCodeHash], canonical RLP, of one of `versions`, with that
    version's CP item count. Returns (version, chainId, cp list of ints, sysCodeHash hex)."""
    require(isinstance(raw, bytes) and 0 < len(raw) <= MAX_SPEC_BYTES, 'GenesisSpec size')
    top = rlp_decode(raw, MAX_SPEC_BYTES, 65536)
    require(isinstance(top, list) and len(top) == 6, 'A GenesisSpec has six top items')
    version, chain, cp, alloc_root, members, sys_hash = top
    def integer(b):
        require(isinstance(b, bytes) and len(b) <= 32 and (not b or b[0]), 'Noncanonical GenesisSpec integer')
        return int.from_bytes(b, 'big')
    require(isinstance(version, bytes) and len(version) == 1 and version[0] in versions,
            'Unsupported GenesisSpec version')
    require(isinstance(cp, list) and len(cp) == CP_ITEMS[version[0]], 'CP item count of the GenesisSpec version')
    require(isinstance(alloc_root, bytes) and len(alloc_root) == 32 and isinstance(members, list) and
            isinstance(sys_hash, bytes) and len(sys_hash) == 32, 'GenesisSpec structure')
    chain_id = integer(chain)
    require(0 < chain_id < 1 << 64, 'GenesisSpec chain ID')
    return version[0], chain_id, [integer(x) for x in cp], '0x' + sys_hash.hex()


def issuance(h, s0, h_half, s_tail):
    """V3-19 M3 (lp4-execution issuance.rs): h == 0 ? 0 : max(k >= 256 ? 0 : S0 >> k, S_tail), k = h // H_half."""
    if h == 0:
        return 0
    k = h // h_half if h_half else LOW64
    return max(0 if k >= 256 else s0 >> k, s_tail)


def tip_share(tips, gamma_tip_bp, signed):
    """V3-20 closeBlock: gamma_tip_bp/10^4 of the tips (floor) to the signed proposer; a fallback block keeps them all."""
    return tips * gamma_tip_bp // 10000 if signed else 0


def pot_amount(issued, tips, share, carry):
    """V3-15/19/20 closeBlock: pot h = issued + (tips - share) + the carry into pot h."""
    return issued + tips - share + carry


def split(amount, alpha_bp, gamma_bp):
    """M1 settleOne: winnerPart, propPart and R."""
    wp, pp = amount * alpha_bp // 10000, amount * gamma_bp // 10000
    return wp, pp, amount - wp - pp


def base_burn(r, total, kappa_m):
    """RewardEscrow.distribute: Q = max(C, kappa*m), A = floor(R*C/Q); the base burn R - A goes to the carry into the
    next pot (V3-15, lagged), except at a final block."""
    q = max(total, kappa_m)
    return r - r * total // q


def reporter_cut(bond, rho_bp, has_payee):
    """V3-17 burnBond: rho_bp/10^4 of the burned bond (floor) to the reporter's reward address, none without a payee."""
    return bond * rho_bp // 10000 if has_payee else 0


def _rlp_list_raw(encoded):
    raw = b''.join(encoded)
    if len(raw) < 56:
        return bytes([192 + len(raw)]) + raw
    size = len(raw).to_bytes((len(raw).bit_length() + 7)//8, 'big')
    return bytes([247 + len(size)]) + size + raw


def _hex_prefix(nibbles, leaf):
    flag = 2 if leaf else 0
    if len(nibbles) % 2:
        head, rest = bytes([((flag + 1) << 4) | nibbles[0]]), nibbles[1:]
    else:
        head, rest = bytes([flag << 4]), nibbles
    return head + bytes((rest[i] << 4) | rest[i+1] for i in range(0, len(rest), 2))


def _trie_node(items, depth):
    if len(items) == 1:
        key, value = items[0]
        return _rlp_list_raw([rlp_encode(_hex_prefix(key[depth:], True)), rlp_encode(value)])
    first, last = items[0][0], items[-1][0]
    common = depth
    while common < min(len(first), len(last)) and first[common] == last[common]:
        common += 1
    ref = lambda node: node if len(node) < 32 else rlp_encode(keccak256(node))
    if common > depth:
        return _rlp_list_raw([rlp_encode(_hex_prefix(first[depth:common], False)), ref(_trie_node(items, common))])
    branches = [[] for _ in range(16)]
    for key, value in items:
        require(len(key) > depth, 'Trie key is a prefix')   # RLP index keys are prefix-free
        branches[key[depth]].append((key, value))
    return _rlp_list_raw([ref(_trie_node(b, depth + 1)) if b else b'\x80' for b in branches] + [b'\x80'])


def ordered_root(values):
    """The ordered Merkle-Patricia root of a block's transactions or evidence items (key RLP(i)), lp4 receipts.rs."""
    if not values:
        return keccak256(b'\x80')
    def nibbles(raw):
        return [n for b in raw for n in (b >> 4, b & 15)]
    items = sorted((nibbles(rlp_encode(i.to_bytes((i.bit_length() + 7)//8, 'big'))), v) for i, v in enumerate(values))
    return keccak256(_trie_node(items, 0))


def _ident(value):
    return '0x' + value.to_bytes(20, 'big').hex()


def _live(w0):
    """MinerRegistry.live: a record (status != 0), not a tomb, HELD (1) or ACCRUING (4)."""
    return w0 & 255 != 0 and not (w0 >> 8) & 255 and (w0 >> 16) & 255 in (1, 4)


class _Payments:
    """RewardEscrow payments out of Escrow over one state: payTo/distribute (an absent record or a tomb burns; an ACCRUING
    record, exited included, fills its bond up to B_reg first and is promoted to HELD when it reaches it, EARN-IN P1),
    Sys.pay (PAY-02: a non-durable recipient, an underfunded Escrow or a balance overflow burns) and markClose's bond
    refunds (releaseBond: a failed PAY leaves the record live)."""

    def __init__(self, chain, at):
        self.chain, self.at = chain, at
        self.records, self.accounts, self.durable = {}, {}, {}
        self.escrow = quantity(chain.result('eth_getBalance', [ESCROW, hex(at)]))
        self.paid, self.accrued, self.rewards = Counter(), Counter(), {}
        # By (miner, reward address): a miner whose reward address changes within the block (its own setRewardAddress
        # before the tip share) has one beneficiary entry per address.
        self.paid_to, self.accrued_to = Counter(), Counter()
        self.burned = 0
        self.refunds = []

    def record(self, miner):
        if miner not in self.records:
            self.records[miner] = list(self.chain.record(miner, self.at))
        return self.records[miner]

    def _durable(self, reward, at):
        key = (reward, at)
        if key not in self.durable:
            nonce = quantity(self.chain.result('eth_getTransactionCount', [reward, hex(at)]))
            code = b'' if nonce else hex_bytes(self.chain.result('eth_getCode', [reward, hex(at)]), maximum=65536)
            self.durable[key] = bool(nonce or code)
        return self.durable[key]

    def transfer(self, reward, amount, at=None):
        """Sys.pay of `amount` from Escrow to `reward`: False (nothing moves) for the zero address, a recipient without a
        nonce or code (read at `at`, by default the state these payments start from), an Escrow below the amount or a
        recipient balance overflow (PAY-02)."""
        if reward == ZERO:
            return False
        if reward not in self.accounts:
            durable = self._durable(reward, self.at)
            balance = self.escrow if reward == ESCROW else quantity(self.chain.result('eth_getBalance', [reward, hex(self.at)]))
            self.accounts[reward] = [durable, balance]
        durable = self.accounts[reward][0] if at is None else self._durable(reward, at)
        if not durable or self.escrow < amount or (reward != ESCROW and self.accounts[reward][1] + amount > MAX_U256):
            return False
        if reward != ESCROW:
            self.escrow -= amount
            self.accounts[reward][1] += amount
        return True

    def pay(self, reward, amount, miner, at=None):
        """Sys.pay from Escrow; returns the amount burned (0 or amount)."""
        if amount == 0:
            return 0
        if not self.transfer(reward, amount, at):
            self.burned += amount
            return amount
        self.paid[miner] += amount
        self.paid_to[(miner, reward)] += amount
        self.rewards[miner] = reward
        return 0

    def pay_to(self, miner, amount, at=None):
        if amount == 0:
            return
        w0, reward, bond, _ = self.record(miner)
        if w0 == 0 or (w0 >> 8) & 255:
            self.burned += amount
            return
        reward = _ident(reward)
        if (w0 >> 16) & 255 == 4:
            b_reg = self.chain.cp[16]
            add = min(amount, max(b_reg - bond, 0))
            self.records[miner][2] = bond + add
            if bond + add == b_reg:
                self.records[miner][0] = (w0 & ~(255 << 16)) | (1 << 16)    # promote: HELD
            self.accrued[miner] += add
            self.accrued_to[(miner, reward)] += add
            self.rewards[miner] = reward
            amount -= add
        self.pay(reward, amount, miner, at)

    def refund(self, miner):
        """markClose on a live record: releaseBond pays its whole bond to its reward address (none to pay for a zero
        bond); on success the record is PAID (3), on a failed PAY it stays as it is. Returns whether it was refunded."""
        w0, reward, bond, _ = self.record(miner)
        reward = _ident(reward)
        if bond and not self.transfer(reward, bond):
            return False
        self.records[miner][0] = (w0 & ~(255 << 16)) | (3 << 16)
        if bond:
            self.refunds.append({'minerId': miner, 'rewardAddress': reward, 'amountWei': str(bond)})
        return True

    def punish(self, accused):
        """MinerRegistry.punish's record effect: tomb; a HELD or ACCRUING bond burns (returned), a PAID record keeps none."""
        w0, reward, bond, w3 = self.record(accused)
        require(w0 != 0 and not (w0 >> 8) & 255, 'Evidence accuses a record that is absent or a tomb')
        state = (w0 >> 16) & 255
        require(state in (1, 3, 4), 'Evidence accuses a record in an unknown bond state')
        if state == 3:
            self.records[accused][0] = w0 | (1 << 8)
            return 0
        self.records[accused][0] = (w0 & ~(255 << 16)) | (2 << 16) | (1 << 8)
        return bond

    def distribute(self, owners, counts, total, r, kappa_m):
        """RewardEscrow.distribute: p_i = floor(R*c_i/Q); returns (A - sum p_i roundBurn added to burned, base burn)."""
        q = max(total, kappa_m)
        paid = 0
        for miner in owners:
            p = r * counts[miner] // q
            paid += p
            self.pay_to(miner, p)
        a = r * total // q
        self.burned += a - paid
        return r - a

    def beneficiaries(self):
        return _entries(self.paid_to, self.accrued_to)


def _entries(paid, accrued):
    """Beneficiary entries by (miner, reward address): amountWei paid there, bondAccruedWei filled into the bond."""
    return [dict({'minerId': m, 'rewardAddress': r, 'amountWei': str(paid.get((m, r), 0))},
                 **({'bondAccruedWei': str(accrued[(m, r)])} if accrued.get((m, r)) else {}))
            for m, r in sorted(set(paid) | set(accrued)) if paid.get((m, r), 0) > 0 or accrued.get((m, r), 0) > 0]


def _punish(flows, accused, proposer, rho_bp):
    """MinerRegistry.punish(accused, reporter) with RewardEscrow.burnBond (V3-16/V3-17): the accused's HELD or ACCRUING
    bond burns, less rho_bp/10^4 of it paid by Sys.pay to the reward address of the reporter's record (tomb or not;
    `proposer` is the block's signed proposer). No reporter (a fallback block) or no reporter record: no cut. A failed
    PAY burns the cut too. Returns (bond, reporter reward address or ZERO, cut paid)."""
    before = flows.burned
    bond = flows.punish(accused)
    reward = ZERO
    if proposer is not None:
        payee = flows.record(proposer)
        reward = _ident(payee[1]) if payee[0] != 0 else ZERO
    cut = reporter_cut(bond, rho_bp, reward != ZERO)
    if cut and flows.pay(reward, cut, proposer) != 0:
        cut = 0
    flows.burned = before + bond - cut
    return bond, reward, cut


def earliest_final_height(close_height, h, d, h_end, t_end):
    """The lowest height above h that can be the final block F, or None when the chain has no scheduled end. The host's
    final_predicate (lp4-execution finality_core.rs) holds at F when (h_end != 0 or t_end != 0), the parent is closing
    with its cursor done, F >= closeHeight + D and (F >= h_end or ts >= t_end). That time condition gives no height
    bound: with t_end == 0 it always holds (ts >= 0), with h_end == 0 likewise (F >= 0), and with both set ts >= t_end
    can hold below h_end. H_END only caps the chain (child_allowed: F <= h_end), it never delays F."""
    if h_end == 0 and t_end == 0:
        return None
    return max(close_height + d, h + 1)


TOP_UP = keccak256(b'topUp()')[:4]


def _tx_call(raw):
    """(to, input) of a committed transaction, as lp4-execution wire.rs decodes the three accepted forms: legacy RLP
    (9 fields), EIP-2930 0x01 (11) and EIP-1559 0x02 (12). `to` is None for a contract creation."""
    kind = raw[0] if raw[0] in (1, 2) else 0
    require(kind or raw[0] >= 0xc0, 'Unsupported transaction type')
    fields = rlp_decode(raw[1:] if kind else raw, MAX_BODY_BYTES, 1 << 20)
    require(isinstance(fields, list) and len(fields) == (9, 11, 12)[kind], 'Malformed transaction')
    to, data = fields[(3, 4, 5)[kind]], fields[(5, 6, 7)[kind]]
    require(isinstance(to, bytes) and len(to) in (0, 20) and isinstance(data, bytes), 'Malformed transaction')
    return ('0x' + to.hex() if to else None), data


def _tip_share_record(chain, flows, proposer, share, h, calls):
    """The proposer's record as closeBlock(h) reads it, after the user phase. Phase 0 is reproduced in `flows`; the user
    phase changes the payment fields of `proposer`'s record (reward address, bond state, bond) only by its own successful
    transactions to MinerRegistry (setRewardAddress, topUp, deregister, claimBond, reactivate are keyed by msg.sender;
    another sender's removeInactive or registerContest only exits it), and the record after closeBlock (state h) differs
    from the one closeBlock read only by the tip share's accrual. `calls` are the block's transactions (tips()): sender,
    destination, receipt status and selector. Without a successful registry call of its own the reproduced record is
    used. With one, the record before closeBlock is recovered from state h: its reward address, and its bond state and
    bond from phase 0, which only topUp (EARN-IN U2) changes in the user phase. After a topUp, an ACCRUING record's bond
    is its bond at h less the share; a record phase 0 left ACCRUING that is HELD at h is ambiguous (the topUp may have
    promoted it before the share, or the share did): refused."""
    after = list(chain.record(proposer, h))
    state = lambda w: (w >> 16) & 255
    own = [c for c in calls if c['from'] == proposer and c['to'] == REGISTRY and c['status'] == 1]
    if own:
        pre = list(after)
        if share and after[0] != 0 and not (after[0] >> 8) & 255:
            phase0 = flows.record(proposer)
            if any(c['selector'] == TOP_UP for c in own):
                if state(after[0]) == 4:
                    require(after[2] >= share, 'Proposer bond below its tip share')
                    pre[2] = after[2] - share
                elif state(after[0]) == 1 and state(phase0[0]) == 4:
                    raise RewardUnavailable('The proposer topped up its bond in its own block; its tip share split is ambiguous')
            else:
                pre[0] = (after[0] & ~(255 << 16)) | (phase0[0] & (255 << 16))
                pre[2] = phase0[2]
        flows.records[proposer] = pre
    return after


def _same_record(a, b):
    """The fields reward payments read: status presence, tomb, bond state, reward address, bond."""
    return (a[0] != 0, (a[0] >> 8) & 255, (a[0] >> 16) & 255, a[1], a[2]) == (b[0] != 0, (b[0] >> 8) & 255, (b[0] >> 16) & 255, b[1], b[2])


class _Delta:
    """The part of a _Payments object's counters one step adds (a settlement, a tip share)."""
    def __init__(self, pay):
        self.pay, self.paid, self.accrued, self.burned = pay, Counter(pay.paid_to), Counter(pay.accrued_to), pay.burned

    def close(self):
        paid = {k: v - self.paid[k] for k, v in self.pay.paid_to.items() if v - self.paid[k]}
        accrued = {k: v - self.accrued[k] for k, v in self.pay.accrued_to.items() if v - self.accrued[k]}
        return sum(paid.values()), sum(accrued.values()), self.pay.burned - self.burned, _entries(paid, accrued)


class _V3Chain:
    """Block-anchored reads of a v3 chain at and below an anchor block (headers linked by parentHash from the anchor)."""

    def __init__(self, query, network, fork_schedule, cp):
        self.query, self.network, self.schedule, self.cp = query, network, fork_schedule, cp
        self.anchor = None
        self.headers, self.bodies, self.assignments, self.record_cache = {}, {}, {}, {}

    def result(self, method, params):
        return _result(self.query, method, params)

    def call(self, to, signature, args, height, words=None):
        data = keccak256(signature.encode())[:4] + b''.join(_word(x) for x in args)
        raw = hex_bytes(self.result('eth_call', [{'to': to, 'data': '0x'+data.hex(), 'gas': '0x1e8480'}, hex(height)]),
                        None if words is None else words*32)
        return raw if words is None else [int.from_bytes(raw[i:i+32], 'big') for i in range(0, len(raw), 32)]

    def record(self, miner, height):
        key = (miner, height)
        if key not in self.record_cache:
            values = self.call(REGISTRY, 'recordWords(address)', [int(miner, 16)], height, 4)
            require(values[1] < 1 << 160, 'Malformed reward address')
            self.record_cache[key] = tuple(values)
        return self.record_cache[key]

    def set_anchor(self, header, raw_body):
        self.anchor = header['height']
        self.headers[self.anchor] = header
        self.bodies[self.anchor] = self._body(header, raw_body)

    def header(self, k):
        require(self.anchor is not None and 0 <= k <= self.anchor, 'Header outside the anchored range')
        if k == 0:
            return {'height': 0, 'hash': self.network.genesis, 'seal': keccak256(bytes.fromhex(self.network.genesis[2:]))}
        # Every ancestor a snapshot reads lies within E_win + D + 2 blocks of the anchor (type-2 evidence of the parent
        # block, and the assignment of the template it names).
        require(self.anchor - k <= self.cp[26] + self.cp[11] + 2, 'Ancestor outside the reward window')
        while k not in self.headers:
            below = min(x for x in self.headers if x > k) - 1
            encoded = self.result('pocol_getBlockRlp', [hex(below)])
            require(isinstance(encoded, dict), 'Missing ancestor block')
            decoded = decode_header(hex_bytes(encoded.get('header'), maximum=3067), self.schedule, self.network)
            require(decoded['height'] == below and decoded['hash'] == self.headers[below + 1]['parentHash'],
                    'Ancestor header mismatch')
            self.headers[below] = decoded
            self.bodies[below] = self._body(decoded, encoded.get('body'))
        return self.headers[k]

    def _body(self, header, raw):
        """(txs, evidence) committed by the header's txRoot and evidenceRoot."""
        top = rlp_decode(hex_bytes(raw, maximum=MAX_BODY_BYTES), MAX_BODY_BYTES, 1 << 20)
        require(isinstance(top, list) and len(top) == 2 and all(isinstance(x, list) for x in top) and
                all(isinstance(i, bytes) and i for x in top for i in x), 'Malformed block body')
        txs, evidence = top
        require(ordered_root(txs) == header['txRoot'] and ordered_root(evidence) == header['evidenceRoot'],
                'Block body is not the one its header commits')
        return txs, evidence

    def body(self, k):
        self.header(k)
        return self.bodies[k]

    def assignment(self, k):
        """Block k's nonce ranges from its parent's state (consensus.md §5, chain.rs membership): on v3 with unbounded
        ranges every live (non-tomb) member of the parent's ActList owns one, regardless of recent shares (D-V3-01)."""
        if k in self.assignments:
            return self.assignments[k]
        header, parent = self.header(k), self.header(k - 1)
        require(k > 0, 'Genesis has no assignment')
        cp = self.cp
        n = self.call(REGISTRY, 'lengths()', [], k - 1, 4)[1]
        require(0 < n <= cp[18] <= MAX_MEMBERS_V3, 'Unsupported active membership size')
        full = full_nonce_rule(header['protocolVersion'], cp[4])
        all_live = v3_active(k, self.schedule, self.network.spec_version) and full
        cutoff = max(0, k - cp[24])
        live, active, seen = [], [], set()
        for i in range(n):
            value = self.call(REGISTRY, 'activeAt(uint256)', [i], k - 1, 1)[0]
            require(0 < value < 1 << 160 and value != (1 << 160)-2 and not 0xc0c001 <= value <= 0xc0c0ff, 'Invalid active miner')
            miner = _ident(value)
            require(miner not in seen, 'Duplicate active miner')
            seen.add(miner)
            record = self.record(miner, k - 1)
            require(record[0] != 0 and record[0] & 255 == 1, 'Invalid active record')
            if (record[0] >> 8) & 255:
                continue
            live.append(miner)
            if ((record[0] >> 88) & LOW64) >= cutoff or record[3] >> 192 >= cutoff:
                active.append(miner)
        members = live if all_live or not active else active
        require(bool(members), 'No nonce owners')
        seed = hashlib.sha256(b'PoColNonce' + parent['seal'] + k.to_bytes(8, 'big') + header['attempt'].to_bytes(4, 'big')).digest()
        tickets = sorted((hashlib.sha256(seed + bytes.fromhex(m[2:])).digest(), m) for m in members)
        require(len(set(t[0] for t in tickets)) == len(tickets), 'Nonce ticket collision')
        maximum = 1 << 64 if full else min(1 << 64, ((1 << 256)//(header['target']+1))*cp[5])
        require(maximum > 0, 'Empty nonce space')
        ranges = [(i*maximum//len(tickets), (i+1)*maximum//len(tickets), m) for i, (_, m) in enumerate(tickets)]
        self.assignments[k] = (maximum, ranges)
        return self.assignments[k]

    def owner(self, k, nonce):
        maximum, ranges = self.assignment(k)
        require(0 <= nonce < maximum, 'Nonce outside assigned domain')
        matches = [m for start, end, m in ranges if start <= nonce < end]
        require(len(matches) == 1, 'Unassigned nonce')
        return matches[0]

    def header_owners(self, k):
        """recordShares(k) sequence: the header shares in nonce order, then the implicit winner once."""
        header = self.header(k)
        return [self.owner(k, n) for n in header['shares']] + [self.owner(k, header['nonce'])]

    def evidence(self, k, kinds=(1, 2, 3)):
        """Block k's evidence effects in body order (chain.rs evidence_effects): kind 3 (late shares of template j,
        D-V3-02) gives j and one owner per nonce under block j's assignment; kind 1 (two signed templates) accuses
        their proposer; kind 2 (a winner's two share roots) accuses owner(nonce) of the ancestor template j within
        E_win that it names. The node validated the block; this reproduces the effects, not the validity."""
        out = []
        for raw in self.body(k)[1]:
            item = rlp_decode(raw, 65536)
            require(isinstance(item, list) and item and isinstance(item[0], bytes), 'Malformed evidence item')
            kind = item[0]
            if kind == b'\x03' and len(item) == 3:
                if 3 not in kinds:
                    out.append({'kind': 3})
                    continue
                require(isinstance(item[1], bytes) and len(item[1]) == 8 and isinstance(item[2], list) and
                        all(isinstance(n, bytes) and len(n) == 8 for n in item[2]), 'Malformed late shares')
                j = int.from_bytes(item[1], 'big')
                nonces = [int.from_bytes(n, 'big') for n in item[2]]
                require(max(1, k - self.cp[11]) <= j < k and nonces and nonces == sorted(set(nonces)), 'Late shares outside the window')
                out.append({'kind': 3, 'j': j, 'nonces': nonces, 'owners': [self.owner(j, n) for n in nonces]})
            elif kind == b'\x01' and len(item) == 3:
                st = item[1]
                require(isinstance(st, list) and len(st) == 2 and isinstance(st[0], list) and len(st[0]) == 18 and
                        isinstance(st[0][17], bytes) and len(st[0][17]) == 20, 'Malformed type-1 evidence')
                out.append({'kind': 1, 'accused': '0x' + st[0][17].hex(), 'j': None})
            elif kind == b'\x02' and len(item) == 7:
                if 2 not in kinds:
                    out.append({'kind': 2, 'unparsed': True})
                    continue
                require(isinstance(item[1], bytes) and len(item[1]) == 32 and isinstance(item[2], bytes) and len(item[2]) == 8,
                        'Malformed type-2 evidence')
                nonce = int.from_bytes(item[2], 'big')
                j = next((e for e in range(k - 1, max(0, k - self.cp[26] - 1), -1)
                          if self.header(e)['templateId'] == item[1]), None)
                require(j is not None, 'Type-2 evidence names no ancestor template')
                out.append({'kind': 2, 'accused': self.owner(j, nonce), 'j': j})
            else:
                raise RewardUnavailable('Unknown evidence form')
        return out

    def tips(self, k, block):
        """V3-20: the tips block k's user transactions paid to Escrow, sum gasUsed*(effectiveGasPrice - baseFee) over
        the receipts of the transactions its body commits; returns (tips, calls): each transaction's sender and receipt
        status (from its receipt), destination and selector (from the committed transaction)."""
        header = self.header(k)
        txs = self.body(k)[0]
        hashes = block.get('transactions')
        require(isinstance(hashes, list) and [_lower(h) for h in hashes] == ['0x' + keccak256(t).hex() for t in txs],
                'Block transactions differ from its body')
        total, calls = 0, []
        for raw, tx in zip(txs, hashes):
            receipt = self.result('eth_getTransactionReceipt', [tx])
            require(isinstance(receipt, dict) and _lower(receipt.get('transactionHash')) == _lower(tx) and
                    _lower(receipt.get('blockHash')) == header['hash'] and quantity(receipt.get('blockNumber')) == k,
                    'Receipt of another block')
            used, price = quantity(receipt.get('gasUsed')), quantity(receipt.get('effectiveGasPrice'))
            require(price >= header['baseFee'] and used <= header['gasUsed'], 'Receipt below the base fee')
            total += used * (price - header['baseFee'])
            to, data = _tx_call(raw)
            status = quantity(receipt.get('status'))
            require(_lower(receipt.get('to')) == to and status in (0, 1), 'Receipt of another transaction')
            calls.append({'from': _lower(receipt.get('from')), 'to': to, 'status': status, 'selector': data[:4]})
        return total, calls

    def pending(self, j, height):
        return self.call(ESCROW, 'pending(uint256)', [j], height, 3)

    def ledger(self, j, height):
        """ShareLedger.entriesOf(j): (owners in first-seen order, counts, total)."""
        raw = self.call(LEDGER, 'entriesOf(uint64)', [j], height)
        words = lambda at, n: [int.from_bytes(raw[at+32*i:at+32*i+32], 'big') for i in range(n)]
        require(len(raw) >= 96 and len(raw) % 32 == 0, 'Malformed ledger entries')
        a, c, total = words(0, 3)
        require(a == 96 and len(raw) >= a + 32, 'Malformed ledger entries')
        n = words(a, 1)[0]
        require(n <= MAX_LEDGER_OWNERS and c == a + 32 + 32*n and len(raw) == c + 32 + 32*n and words(c, 1)[0] == n,
                'Malformed ledger entries')
        owners = words(a + 32, n)
        require(all(0 < x < 1 << 160 for x in owners), 'Malformed ledger owner')
        return [_ident(x) for x in owners], words(c + 32, n), total


def _v3_cp(params, network):
    """The CP of the network's GenesisSpec (pocol_getParams.genesisPre), proven by the trusted genesis hash: specVersion 2,
    CP schema v2, the v3 program set."""
    spec = hex_bytes(params.get('genesisPre'), maximum=MAX_SPEC_BYTES)
    require('0x' + keccak256(spec).hex() == network.genesis, 'GenesisSpec does not hash to the trusted genesis')
    version, chain_id, values, sys_hash = decode_genesis_spec(spec, (network.spec_version,))
    require(chain_id == network.chain_id, 'GenesisSpec chain ID differs from the trusted network')
    require(sys_hash == V3_SYS_CODE_HASH, 'GenesisSpec does not name the v3 program set')
    cp = dict(enumerate(values))
    require(cp[4] in (0, 1) and 0 < cp[5] <= 255 and 0 < cp[24] < 1 << 32, 'Invalid nonce configuration')
    require(0 < cp[8] <= 65535 and 0 < cp[9] <= 255 and 0 < cp[10] <= 256 and 1 < cp[11] <= 65535 and 0 < cp[26] < 1 << 16,
            'Invalid reward configuration')
    require(cp[13] + cp[14] <= 10000 and 0 < cp[15] <= MAX_U256 and 0 < cp[16] <= MAX_U256 and 0 < cp[18] <= MAX_MEMBERS_V3,
            'Invalid reward caps')
    require(0 < cp[52] < 1 << 64 and cp[53] <= MAX_U256 and cp[56] <= 10000 and cp[63] <= 10000, 'Invalid v3 reward fields')
    return cp


def _result_word(chain, j, h):
    """pending(j) as block h's phase 0 reads it after recordResult(h-1): for j = h-1 the winner word recordResult writes,
    WIN_ACCRUING from the parent state (V3-16: committed when the result is recorded, before any evidence of h)."""
    amount, meta, word = chain.pending(j, h - 1)
    if j == h - 1:
        require(meta != 0 and word == 0, 'Parent pot already has a result')
        won = chain.owner(j, chain.header(j)['nonce'])
        record = chain.record(won, h - 1)
        word = int(won, 16) | (WIN_ACCRUING if record[0] != 0 and not (record[0] >> 8) & 255 and
                               (record[0] >> 16) & 255 == 4 else 0)
    return amount, meta, word


def _evidence_flows(chain, flows, h, proposer):
    """Block h's evidence in body order (phase 0): kind 3 is recordLate (reported); kind 1/2 invalidate a pending pot j
    only when the accused is its winner recorded ACCRUING (V3-16), then punish with the reporter's cut (V3-17).
    Returns (reported items, the pots invalidated in h)."""
    cp, d = chain.cp, chain.cp[11]
    evidence, invalidated = [], set()
    for effect in chain.evidence(h):
        if effect['kind'] == 3:
            evidence.append({'kind': 3, 'potHeight': effect['j'], 'nonces': [str(n) for n in effect['nonces']],
                             'owners': effect['owners']})
            continue
        accused, j = effect['accused'], effect['j']
        invalid = False
        if j is not None and j >= max(1, h - d):           # the host invalidates only a pending pot
            _, meta, word = _result_word(chain, j, h)
            require(meta != 0 and word != 0, 'Evidence names a pot without a result')
            invalid = _ident(word & LOW160) == accused and bool(word & WIN_ACCRUING)
            if invalid:
                invalidated.add(j)
        bond, reward, cut = _punish(flows, accused, proposer, cp[63])
        evidence.append({'kind': effect['kind'], 'accused': accused, 'potHeight': j, 'potInvalidated': invalid,
                         'bondBurnedWei': str(bond), 'reporter': proposer,
                         'reporterRewardAddress': reward if reward != ZERO else None, 'reporterCutWei': str(cut)})
    return evidence, invalidated


def _mark_close(chain, flows, h, timestamp):
    """MinerRegistry.markClose in block h's phase 0 (after the evidence, before settle(h-D)). Closing starts at the first
    block with h >= H_CLOSE (CP 47) or timestamp >= T_CLOSE (CP 48), unless both are zero; from then on each block refunds
    the bonds of up to CP 50 records of the record list from its cursor, each live record becoming PAID (3) when its
    PAY succeeds, so that the settlement and the tip share of that block pay it directly. Returns (closing facts or
    None, the records the cursor visited); the cursor and the closing height are checked against state h."""
    cp = chain.cp
    counters = lambda at: chain.call(REGISTRY, 'counters()', [], at, 1)[0]
    c = counters(h - 1)
    if not (c >> 89) & 1 and ((cp[47] == 0 and cp[48] == 0) or (h < cp[47] and timestamp < cp[48])):
        require(not (counters(h) >> 89) & 1, 'Closing state differs from its reconstruction')
        return None, []
    cursor, close_h = ((c >> 90) & 0xffffffff, (c >> 187) & LOW64) if (c >> 122) & 1 else (0, h)
    n = chain.call(REGISTRY, 'lengths()', [], h - 1, 4)[0]
    end = min(cursor + cp[50], n)
    visited = []
    for i in range(cursor, end):
        miner = _ident(chain.call(REGISTRY, 'recordAt(uint256)', [i], h - 1, 1)[0])
        visited.append(miner)
        if _live(flows.record(miner)[0]):
            flows.refund(miner)
    after = counters(h)
    require((after >> 89) & 1 and (after >> 122) & 1 and (after >> 90) & 0xffffffff == max(cursor, end) and
            (after >> 187) & LOW64 == close_h, 'Closing cursor differs from its reconstruction')
    return {'closeHeight': close_h, 'cursorDone': max(cursor, end) >= n}, visited


def _settle(chain, flows, j, h, invalidated, now, final):
    """settleOne(j) in block h (settle(h-D) in phase 0, or settleFinal(j) at the final block): the header owners of j
    and the late shares credited for j in blocks j+1..h (checked against the ShareLedger at h); INVALID (or invalidated
    in h) burns the whole pot; otherwise the winner part, the proposer part (the winner's for a fallback template) and
    distribute. The base burn R - A goes to the carry into pot h+1, or burns at the final block (slot1 set).
    Returns (the settlement, the carried base burn)."""
    cp = chain.cp
    amount, meta, word = _result_word(chain, j, h)
    require(meta != 0 and word != 0, 'Settled pot has no result')
    jh = chain.header(j)
    owners = chain.header_owners(j)
    require(_ident(word & LOW160) == owners[-1], 'Settled pot winner differs from its nonce owner')
    require(meta & ~INVALID == (int(jh['proposer'], 16) << 8 if jh['signed'] else 1), 'Settled pot proposer mismatch')
    late = [o for b in range(j + 1, h) for e in chain.evidence(b, (3,)) if e.get('j') == j for o in e['owners']]
    late += [o for e in now if e['kind'] == 3 and e['potHeight'] == j for o in e['owners']]
    counts = Counter(owners + late)
    listed, counts_listed, total_listed = chain.ledger(j, h)
    require(listed == list(counts) and counts_listed == [counts[m] for m in listed] and total_listed == sum(counts.values()),
            'ShareLedger differs from the reconstructed owners')
    total = sum(counts.values())
    invalid = bool(meta & INVALID) or j in invalidated
    delta = _Delta(flows)
    base = 0
    if invalid:
        flows.burned += amount
    else:
        wp, pp, r = split(amount, cp[13], cp[14])
        flows.pay_to(owners[-1], wp)
        flows.pay_to(jh['proposer'] if jh['signed'] else owners[-1], pp)
        base = flows.distribute(list(counts), counts, total, r, cp[8] * cp[9])
        if final:
            flows.burned += base
    paid, bonded, burned, beneficiaries = delta.close()
    return {'potHeight': j, 'grossRewardWei': str(amount), 'invalid': invalid, 'winner': owners[-1],
            'winnerAccruing': bool(word & WIN_ACCRUING), 'signed': jh['signed'], 'shareCount': total,
            'lateShareCount': len(late), 'owners': [{'minerId': m, 'shares': c} for m, c in counts.items()],
            'paidWei': str(paid), 'bondWei': str(bonded), 'burnedWei': str(burned),
            'carryOutWei': str(0 if final else base), 'baseBurnWei': str(base),
            'beneficiaryCount': len(beneficiaries), 'beneficiaries': beneficiaries}, (0 if final else base)


def _carry_in(chain, h):
    """The carry into pot h (and, at the final block, the carry it burns): the base burn of settle(h-1-D) in block h-1,
    zero for a pot burned whole (INVALID in h-2 or invalidated by block h-1's evidence) and before any settlement."""
    cp, d = chain.cp, chain.cp[11]
    if h - 1 <= d:
        return 0
    j0 = h - 1 - d
    amount0, meta0, word0 = chain.pending(j0, h - 2)
    require(meta0 != 0 and word0 != 0, 'Carried pot has no result')
    invalid0 = bool(meta0 & INVALID) or any(
        e['kind'] == 2 and e['j'] == j0 and e['accused'] == _ident(word0 & LOW160) and word0 & WIN_ACCRUING
        for e in chain.evidence(h - 1, (1, 2)))
    if invalid0:
        return 0
    total0 = chain.call(LEDGER, 'totalOf(uint64)', [j0], h - 1, 1)[0]
    require(total0 > 0, 'Carried pot has no shares')
    return base_burn(split(amount0, cp[13], cp[14])[2], total0, cp[8] * cp[9])


def snapshot_v3(query, *, fork_schedule=DEFAULT_FORK_SCHEDULE, network, height=None):
    """The v3 block projection (schema pocol-block-reward-v3) of the latest block, or of the canonical block `height`
    (operator and test use; the public gateway method stays a no-argument read of the latest block).

    For block h it reproduces, from the anchored state and the committed blocks:
    - pot h: issued(h) (M3) + tips - the proposer's tip share + the carry into pot h (the base burn of settle(h-1-D),
      lagged one block), checked against RewardEscrow's pending pot; and its projected settlement at h + D from the
      header's owners under the all-live membership (D-V3-01), as the v1 projection does (pending: late shares,
      evidence and registry changes before h + D can still change it; once the registry is closing, the final block
      may settle it earlier and burn its base burn instead of carrying it: `closing.settlementMayBeFinal`);
    - block h's own reward flows, which are executed, not projected: its evidence (late shares and their owners;
      offender-only burns with the reporter's cut rho to the signed proposer, D-V3-16/17; the whole pot only for an
      accused winner recorded ACCRUING), markClose's bond refunds once closing (each refunded record is PAID from then
      on), settle(h-D) with its header and late owners (cross-checked with the ShareLedger) and its base burn into the
      carry (cross-checked with RewardEscrow.carry), and the tip share; every burn is cross-checked with burnedTotal.
    - at the final block F (`final`): no pot F; settle(F-D), then finalize burns the tips and both outstanding carries,
      and settleFinal settles every pot in [F-D+1, F-1] with its base burn burned (`block.finalize`).
    ACCRUING records, a pool-contest victim exited until its due height included, fill their bond before their
    reward address is paid (EARN-IN P1; V3-14 owner decision 2026-11-13). Not reported: the sweep's and claimBond's
    bond refunds and user-transaction transfers. The tip share uses the proposer's record before closeBlock, recovered
    from state h when the proposer itself called MinerRegistry in that block (_tip_share_record)."""
    fork_schedule = validated_fork_schedule(fork_schedule)
    network = validated_network(network)
    require(height is None or type(height) is int, 'Invalid block height')
    started = time.monotonic()
    calls = 0
    raw_query = query
    def bounded(method, params):
        nonlocal calls
        calls += 1
        require(calls <= MAX_CALLS_V3 and time.monotonic()-started <= MAX_SECONDS_V3, 'Reward read budget exceeded')
        answer = raw_query(method, params)
        require(time.monotonic()-started <= MAX_SECONDS_V3, 'Reward read timed out')
        return answer
    query = bounded
    params = _result(query, 'pocol_getParams', [])
    require(isinstance(params, dict) and params.get('chainId') == network.chain_id and
            _lower(params.get('genesisHash')) == network.genesis, 'Wrong PoCol network')
    require(type(params.get('activeVersion')) is int and params['activeVersion'] in {v for v, _ in fork_schedule},
            'Unsupported active consensus version')
    advertised = params.get('forkSchedule')
    require(advertised is not None, 'Node did not advertise the configured fork schedule')
    require(validated_fork_schedule(advertised) == fork_schedule, 'Node fork schedule differs from trusted configuration')
    cp = _v3_cp(params, network)
    chain = _V3Chain(query, network, fork_schedule, cp)
    block = _result(query, 'eth_getBlockByNumber', ['latest' if height is None else hex(height), False])
    require(isinstance(block, dict), 'No such block')
    h = quantity(block.get('number'))
    require(0 < h < (1 << 64)-65536 and (height is None or h == height), 'Invalid block height')
    block_hash = '0x'+hex_bytes(block.get('hash'), 32).hex()
    pocol = block.get('pocol')
    require(isinstance(pocol, dict) and type(pocol.get('final')) is bool, 'Missing final flag')
    final = pocol['final']
    encoded = _result(query, 'pocol_getBlockRlp', [hex(h)])
    require(isinstance(encoded, dict), 'Missing block header')
    header = decode_header(hex_bytes(encoded.get('header'), maximum=3067), fork_schedule, network)
    require(header['height'] == h and header['hash'] == block_hash and header['parentHash'] == _lower(block.get('parentHash')),
            'Block header mismatch')
    require(type(pocol.get('shares')) is int and pocol['shares'] == len(header['shares']), 'Share count mismatch')
    require(pocol.get('signed') is header['signed'] and _lower(pocol.get('proposer')) == header['proposer'], 'Proposer mismatch')
    require(pocol.get('attempt') == header['attempt'], 'Attempt mismatch')
    if 'templateId' in pocol:
        require(_lower(pocol['templateId']) == '0x'+header['templateId'].hex(), 'Template mismatch')
    require(len(header['shares']) <= cp[10], 'Invalid reward caps')
    chain.set_anchor(header, encoded.get('body'))
    # A fallback block commits the empty body (pocold chain.rs validation item 12): no tips, no evidence, no reporter.
    require(header['signed'] or chain.body(h) == ([], []), 'A fallback block carries a body')
    winner = address(block.get('miner'))
    d, kappa_m = cp[11], cp[8]*cp[9]
    proposer = header['proposer'] if header['signed'] else None

    # Block h's executed flows over the parent state h-1: phase 0 (evidence, markClose, settle(h-D)), then phase 2.
    flows = _Payments(chain, h - 1)
    evidence, invalidated = _evidence_flows(chain, flows, h, proposer)
    closing, visited = _mark_close(chain, flows, h, header['timestamp'])
    settlement, carry_out = None, 0
    if h > d:
        settlement, carry_out = _settle(chain, flows, h - d, h, invalidated, evidence, False)
    tips, user_calls = chain.tips(h, block)
    issued = 0 if final else issuance(h, cp[15], cp[52], cp[53])
    # The user phase credits the tips to Escrow (coinbase) and the host the issuance, before phase 2 pays out of it.
    flows.escrow += tips + issued
    carry_in = _carry_in(chain, h)
    require(chain.call(ESCROW, 'carry()', [], h - 1, 1)[0] == carry_in, 'Carry into the pot differs from its reconstruction')
    share, tip_paid, tip_bonded, tip_burned, finals = 0, 0, 0, 0, []
    if final:
        # finalize(tips): the tips and both outstanding carries burn; then settleFinal(j) for every pending pot.
        flows.burned += tips + carry_in + carry_out
        for j in range(max(1, h - d + 1), h):
            if chain.pending(j, h - 1)[1] != 0:
                finals.append(_settle(chain, flows, j, h, invalidated, evidence, True)[0])
        require(chain.call(ESCROW, 'carry()', [], h, 1)[0] == 0 and
                all(chain.pending(j, h)[1] == 0 for j in range(max(1, h - d + 1), h + 1)), 'Pending pots or carry after the final block')
    else:
        require(chain.call(ESCROW, 'carry()', [], h, 1)[0] == carry_out, 'Carry differs from the reconstructed base burn')
        share = tip_share(tips, cp[56], header['signed'])
        if proposer is not None:
            after = _tip_share_record(chain, flows, proposer, share, h, user_calls)
            delta = _Delta(flows)
            flows.pay_to(proposer, share, at=h)       # PAY-02 durability as closeBlock sees it, after the user phase
            tip_paid, tip_bonded, tip_burned, _ = delta.close()
            require(_same_record(flows.record(proposer), after), 'Proposer record differs from its reconstruction')
    for miner in visited:
        require((chain.record(miner, h)[0] >> 16) & 255 == (flows.record(miner)[0] >> 16) & 255,
                'Closing refund differs from its reconstruction')
    burned_total = chain.call(ESCROW, 'burnedTotal()', [], h, 1)[0] - chain.call(ESCROW, 'burnedTotal()', [], h - 1, 1)[0]
    require(burned_total == flows.burned, 'Burned total differs from the reconstructed burns')

    owners, beneficiaries, paid, carry_projected, amount, winner_accruing, projection = [], [], 0, 0, 0, False, None
    may_be_final = None
    if not final:
        # Pot h: issued + tips - share + the carry into h (the base burn of settle(h-1-D) in block h-1).
        amount, meta, stored_winner = chain.pending(h, h)
        expected_meta = int(header['proposer'], 16) << 8 if header['signed'] else 1
        require(meta == expected_meta and stored_winner == 0, 'Missing or inconsistent pending pot')
        require(amount == pot_amount(issued, tips, share, carry_in), 'Pending pot differs from issuance, tips and carry')
        require(amount * max(cp[13], cp[14], kappa_m + 1) <= MAX_U256, 'Reward arithmetic overflow')
        owners = chain.header_owners(h)
        require(owners[-1] == winner, 'Winner assignment mismatch')
        counts = Counter(owners)
        projection = _Payments(chain, h)
        require(projection.escrow >= amount, 'Reward escrow underfunded')
        wp, pp, r = split(amount, cp[13], cp[14])
        projection.pay_to(winner, wp)
        projection.pay_to(proposer or winner, pp)
        carry_projected = projection.distribute(list(counts), counts, len(owners), r, kappa_m)
        winner_record = chain.record(winner, h)
        winner_accruing = winner_record[0] != 0 and not (winner_record[0] >> 8) & 255 and (winner_record[0] >> 16) & 255 == 4
        beneficiaries = projection.beneficiaries()
        paid = sum(projection.paid.values()) + sum(projection.accrued.values())
        require(0 <= paid <= amount - carry_projected, 'Reward accounting mismatch')
        earliest = None if closing is None else earliest_final_height(closing['closeHeight'], h, d, cp[45], cp[46])
        if earliest is not None:
            # Pot h settles at the final block F, its base burn burned, whenever F <= h + D.
            may_be_final = earliest <= h + d
            closing = dict(closing, earliestFinalHeight=earliest, settlementMayBeFinal=may_be_final)

    check = _result(query, 'eth_getBlockByNumber', [hex(h), False])
    if not isinstance(check, dict) or _lower(check.get('hash')) != block_hash:
        raise SnapshotChanged('Canonical block changed; reload the reward snapshot')
    out = {'schema': 'pocol-block-reward-v3', 'chainId': network.chain_id, 'genesisHash': network.genesis,
           'specVersion': network.spec_version}
    if network.source is not None:
        out['source'] = network.source
    block_out = {'evidence': evidence, 'bondRefunds': flows.refunds, 'settlement': settlement,
                 'tipShare': {'proposer': proposer, 'shareWei': str(share), 'paidWei': str(tip_paid),
                              'bondWei': str(tip_bonded), 'burnedWei': str(tip_burned)},
                 'paidWei': str(sum(flows.paid.values())), 'bondWei': str(sum(flows.accrued.values())),
                 'burnedWei': str(flows.burned), 'beneficiaries': flows.beneficiaries()}
    if final:
        block_out['finalize'] = {'tipsBurnedWei': str(tips), 'carryBurnedWei': str(carry_in + carry_out), 'settlements': finals}
    out.update({
        'fetchedAt': datetime.now(timezone.utc).isoformat(timespec='milliseconds').replace('+00:00', 'Z'),
        'blockHeight': h, 'blockHash': block_hash, 'final': final, 'settlementHeight': None if final else h + d,
        'closing': closing,
        'issuedWei': str(issued), 'tipsWei': str(tips), 'tipShareWei': str(share), 'carryInWei': str(carry_in),
        'grossRewardWei': str(amount), 'winnerAccruing': winner_accruing,
        'owners': [{'minerId': m, 'shares': c} for m, c in Counter(owners).items()],
        'projectedPaidWei': str(paid), 'projectedBurnedWei': str(amount - paid - carry_projected),
        'projectedBondWei': str(sum(projection.accrued.values()) if projection else 0), 'projectedCarryWei': str(carry_projected),
        'beneficiaryCount': len(beneficiaries), 'beneficiaries': beneficiaries,
        'block': block_out,
        'status': 'pending', 'shareCount': len(header['shares'])})
    return out
