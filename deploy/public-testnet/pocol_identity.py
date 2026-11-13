#!/usr/bin/env python3
"""The PoCol consensus identity (VA-SPEC §2.1), computed from public data only.

A second reference implementation of pocold/src/identity.rs consensus_id: two nodes
peer only when they send the same identity. Standard library only, so it can be
copied into single-file tools (deploy/mainnet/preflight.py keeps an inline copy)."""
import hashlib

DOMAIN = b'pocol-consensus-identity-v1\0'
FLAG_CAPS_TEST = 1
FLAG_UNPINNED = 2
# Test-only spec-2 preview (pocold identity::FLAG_V3_PREVIEW): isolated even when its sources are pinned.
FLAG_V3_PREVIEW = 4
# The flags that append the consensus-sources aggregate to the identity (identity.rs consensus_id).
FLAGS_WITH_SOURCES = FLAG_UNPINNED | FLAG_V3_PREVIEW
# The highest consensus version these tools verify (pocold profile MAX_FORK_VERSION).
MAX_FORK_VERSION = 2
# The GenesisSpec version a release build decodes (genesis_spec.rs SPEC_VERSION). A v3-preview build also decodes
# spec 2 (the v3 genesis, CP schema v2); a release build refuses it until activation (V3-PR-I).
SPEC_VERSION = 1
PREVIEW_SPEC_VERSION = 2
# The lowest wire version a peer may send (pocold identity::PROTOCOL_VERSION_MIN).
PROTOCOL_VERSION_MIN = 2
# Mirror of pocold launcher::LEGACY_BUILDS (test_pocol_identity compares it with launcher.rs). A 0.4.0 binding names
# only its build digest; it is accepted only through a certified entry bound to its chain and genesis. The H15000
# entry ships uncertified (owner decision D1 is pending; its recommended Option B needs no certification), so every
# legacy path fails closed until an owner records the replay evidence (plan §1.7, §6).
LEGACY_BUILDS = ({'digest': '6cf38a14aa01ca3d824b0309564942cc926ca7232e2a5b2672a17e8c99e35476', 'rules': 1,
                  'chainId': 1599846715, 'genesisHash': '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca',
                  'sources': None, 'certified': None},)


def validated_schedule(value, max_version=MAX_FORK_VERSION):
    """pocold configured_forks' runtime shape: (1, 0) first, then versions 2, 3, ...
    in order (each once) up to max_version, at strictly increasing heights, 1..16
    entries. max_version exists for tests of later versions; callers use the default."""
    if not isinstance(value, (tuple, list)) or not 1 <= len(value) <= 16:
        raise ValueError('invalid consensus fork schedule')
    entries = []
    for entry in value:
        if not isinstance(entry, (tuple, list)) or len(entry) != 2:
            raise ValueError('invalid consensus fork entry')
        version, height = entry
        if type(version) is not int or type(height) is not int or not 0 <= height < 1 << 64:
            raise ValueError('invalid consensus fork values')
        entries.append((version, height))
    if (entries[0] != (1, 0)
            or any(v != i + 1 or v > max_version for i, (v, _) in enumerate(entries))
            or any(a[1] >= b[1] for a, b in zip(entries, entries[1:]))):
        raise ValueError('unsupported consensus fork schedule')
    return tuple(entries)


def legacy_rules(digest, chain_id, genesis, table=None):
    """The rules revision a certified legacy build runs on this chain and genesis, or None (fail closed).
    table exists for tests; callers use LEGACY_BUILDS."""
    for entry in LEGACY_BUILDS if table is None else table:
        if (entry['certified'] and isinstance(digest, str) and digest.lower() == entry['digest']
                and type(chain_id) is int and chain_id == entry['chainId']
                and isinstance(genesis, str) and genesis.lower() == entry['genesisHash']):
            return entry['rules']
    return None


def consensus_id(chain_id, genesis, rules, flags, forks, sources=None):
    """SHA-256(domain || chainId u64 || genesisHash || rules u32 || flags u8 || n u8 ||
    (version u16 || height u64) x n || [sources, exactly with the unpinned or preview bit]), big-endian.
    genesis and sources are 32 bytes or 64 hex characters (an 0x prefix is allowed);
    returns lowercase hex without a prefix, as pocol_status consensusIdentity."""
    def raw(value):
        if isinstance(value, str):
            value = bytes.fromhex(value[2:] if value.startswith('0x') else value)
        if len(value) != 32:
            raise ValueError('expected 32 bytes')
        return value
    if not 1 <= len(forks) <= 16:
        raise ValueError('a consensus identity covers a validated schedule')
    if bool(flags & FLAGS_WITH_SOURCES) != (sources is not None):
        raise ValueError('the unpinned or preview bit and the aggregate go together')
    data = DOMAIN + chain_id.to_bytes(8, 'big') + raw(genesis) + rules.to_bytes(4, 'big') + bytes([flags, len(forks)])
    data += b''.join(v.to_bytes(2, 'big') + h.to_bytes(8, 'big') for v, h in forks)
    if sources is not None:
        data += raw(sources)
    return hashlib.sha256(data).hexdigest()


def build_flags(build):
    """The flags a build reports through `pocold build-info` (identity.rs flags(): bit0 caps-test, bit1 unpinned,
    bit2 v3-preview)."""
    features = build.get('features', [])
    return ((FLAG_CAPS_TEST if 'caps-test' in features else 0) | (0 if build['consensusPinned'] else FLAG_UNPINNED)
            | (FLAG_V3_PREVIEW if build.get('v3Preview') is True or 'v3-preview' in features else 0))


def spec_version(info):
    """The GenesisSpec version devnet.json `info` declares (pocold testnet writes `specVersion: 2` and `v3: true`
    together for a v3 genesis and neither for spec 1). Anything else is refused."""
    version = info.get('specVersion', 1)
    if type(version) is not int or version not in (1, PREVIEW_SPEC_VERSION) or info.get('v3', False) is not (version == 2):
        raise ValueError('unsupported or inconsistent GenesisSpec version')
    return version


def for_network(info, build):
    """The identity a node of build-info `build` sends on the network of devnet.json `info`. The spec version is not
    an identity input (genesisHash already commits the whole GenesisSpec), but a node opens a spec-2 (v3) genesis only
    when its build decodes it: a v3-preview build, never a release build before activation."""
    flags = build_flags(build)
    if spec_version(info) > SPEC_VERSION and not flags & FLAG_V3_PREVIEW:
        raise ValueError('a release build refuses a spec-2 (v3) genesis until activation; only a v3-preview build opens it')
    return consensus_id(info['chainId'], info['genesisHash'], build['consensusRulesVersion'], flags,
                        validated_schedule(info.get('forks', [[1, 0]]), build.get('maxForkVersion', MAX_FORK_VERSION)),
                        build['consensusSourcesSha256'] if flags & FLAGS_WITH_SOURCES else None)


# ---------------------------------------------------------------------------------------------------------------------
# GenesisSpec reference decoder: a port of lp1-node/src/genesis_spec.rs decode_bounded (framing L0, then gsStructure,
# gsVersion, gsCount, gsInt, gsLen, gsRange, gsOrder, gsSys, each over the whole input) with the reference's error codes
# and details, for CP schema v1 and v2. test_pocol_identity checks it against the Rust verdicts of the whole decoder
# corpus (prototypes/pocol-extension/tests/data, the release and the v3-preview verdict files), as refdec.js is checked.
# ---------------------------------------------------------------------------------------------------------------------
class GenesisSpecError(ValueError):
    """A GenesisSpec refusal: `code` (L0, gsStructure, gsVersion, ...) and `detail`, exactly as genesis_spec.rs GsError."""
    def __init__(self, code, detail):
        super().__init__(f'{code}:{detail}')
        self.code, self.detail = code, detail


GAMMA = 'gamma'   # gamma_bp <= 10^4 - alpha_bp
# CP schema v1 in order: name, width in bits, lower bound, upper bound (None: none).
CP_SCHEMA = (
    ('g_ts', 64, 0, None), ('target_g', 256, 1, None), ('T_blk', 32, 1, None), ('tau', 32, 1, None), ('nonceMode', 8, 0, 1),
    ('c', 8, 1, None), ('D_att', 32, 0, None), ('dFbWait', 32, 0, None), ('m', 16, 1, None), ('kappa', 8, 1, None),
    ('S_max', 16, 0, 256), ('D', 16, 0, None), ('W_w', 16, 0, None), ('alpha_bp', 16, 0, 10000), ('gamma_bp', 16, 0, GAMMA),
    ('subsidy', 256, 0, None), ('B_reg', 256, 0, None), ('R_max', 8, 0, None), ('M_max', 16, 1, None), ('M_min', 16, 1, None),
    ('REG_RECORDS_MAX', 32, 0, None), ('SWEEP_MAX', 16, 0, None), ('SWEEP_SCAN', 16, 0, None), ('REG_OPS', 16, 0, None),
    ('W_act', 32, 0, None), ('Theta_inact', 32, 0, None), ('E_win', 32, 0, None), ('U', 32, 0, None), ('E_max', 8, 0, None),
    ('EVID_BYTES_MAX', 32, 0, None), ('GAS_LIMIT', 64, 0, None), ('baseFee0', 256, 0, None), ('TX_MAX', 32, 0, None),
    ('BODY_MAX', 32, 0, None), ('B_code_max', 32, 0, None), ('CODE_CAP', 64, 0, None), ('TXBYTES_CAP', 64, 0, None),
    ('TX_CAP', 64, 0, None), ('SLOT_CAP', 64, 0, None), ('ACCT_CAP', 64, 0, None), ('SYS_SLOT_MAX', 32, 0, None),
    ('SYS_KEYS_BLOCK_MAX', 32, 0, None), ('SYS_KEYS_USER_MAX', 32, 0, None), ('SYS_ACCT_BLOCK_MAX', 32, 0, None),
    ('G_SYS', 64, 0, None), ('H_END', 64, 0, None), ('T_END', 64, 0, None), ('H_CLOSE', 64, 0, None), ('T_CLOSE', 64, 0, None),
    ('Z_close', 32, 0, None), ('RET', 32, 1, None), ('P2', 32, 0, None),
)
# CP schema v2 (genesis_spec.rs CP_V2_EXTRA): the complete v1 prefix, then these fields in immutable wire order.
CP_V2_EXTRA = (
    ('H_half', 64, 1, None), ('S_tail', 256, 0, None), ('P_MAX', 16, 0, None), ('T_PROB', 32, 1, None),
    ('F0_acct', 256, 0, None), ('F0_slot', 256, 0, None), ('F0_code', 256, 0, None), ('gamma_tip_bp', 16, 0, 10000),
    ('baseFeeMin', 256, 0, None), ('R_react', 8, 0, None), ('C_react', 32, 0, None), ('E_epoch', 32, 1, None),
    ('theta_bp', 16, 0, 10000), ('k_sigma', 16, 0, None), ('rho_bp', 16, 0, 10000),
)
CP_SCHEMA_V2 = CP_SCHEMA + CP_V2_EXTRA
_ALPHA_BP = 13
# sysCodeHash of the v3 program set (apply.rs SystemPrograms::v3, the set a spec-2 genesis selects):
# keccak256(RLP([C0C001..C0C005 runtimes of lp4-execution/tests/data/system-v3.json, 0x80 for PAY])); equal to
# deploy/rewards/pocol_rewards.V3_SYS_CODE_HASH, and test_pocol_identity recomputes it from that file.
V3_SYS_CODE_HASH = '72b5af64a2023c2c1662e011a4dbac454bc06b02b4fa321c8ba1c305461ca849'


def cp_schema(version):
    """Known schemas only (genesis_spec.rs cp_schema); a future wire version never inherits v2's layout."""
    return {1: CP_SCHEMA, 2: CP_SCHEMA_V2}.get(version)


def _long_len(b, i, ll, short):
    if len(b) - (i + 1) < ll:
        raise GenesisSpecError('L0', 'truncated length')
    if b[i + 1] == 0:
        raise GenesisSpecError('L0', 'leading zero in length')
    n = int.from_bytes(b[i + 1:i + 1 + ll], 'big')
    if n < 56:
        raise GenesisSpecError('L0', short)
    return n, i + 1 + ll


def _head(b, i):
    """('byte'|'str'|'list', payload length, payload start) of the item at i."""
    if i >= len(b):
        raise GenesisSpecError('L0', 'truncated')
    p = b[i]
    if p < 0x80:
        return 'byte', 1, i
    if p <= 0xb7:
        return 'str', p - 0x80, i + 1
    if p <= 0xbf:
        return ('str',) + _long_len(b, i, p - 0xb7, 'long form for short string')
    if p <= 0xf7:
        return 'list', p - 0xc0, i + 1
    return ('list',) + _long_len(b, i, p - 0xf7, 'long form for short list')


def _check_framing(b):
    ends, i = [], 0
    while True:
        kind, n, st = _head(b, i)
        if kind == 'byte':
            i += 1
        elif kind == 'str':
            if n > len(b) - st:
                raise GenesisSpecError('L0', 'truncated string')
            i = st + n
        else:
            if n > len(b) - st:
                raise GenesisSpecError('L0', 'truncated list')
            i = st
            if n > 0:
                ends.append(st + n)
                continue
        while True:
            if not ends:
                if i != len(b):
                    raise GenesisSpecError('L0', f'{len(b) - i} trailing bytes')
                return
            if i > ends[-1]:
                raise GenesisSpecError('L0', 'item crosses list end')
            if i < ends[-1]:
                break
            ends.pop()


def _item(b, i):
    """(is list, payload start, payload length, wrapped single byte) of the framing-checked item at i."""
    kind, n, st = _head(b, i)
    return kind == 'list', st, n, kind == 'str' and n == 1 and b[st] < 0x80


def _children(b, it):
    out, i = [], it[1]
    while it[0] and i < it[1] + it[2]:
        child = _item(b, i)
        out.append(child)
        i = child[1] + child[2]
    return out


def _is_reserved(ident):
    if ident == bytes(20) or ident == b'\xff' * 19 + b'\xfe':
        return True
    return ident[:17] == bytes(17) and 0xc0c001 <= int.from_bytes(ident[17:], 'big') <= 0xc0c0ff


def decode_genesis_spec(b, max_version=SPEC_VERSION):
    """genesis_spec.rs decode_bounded: a GenesisSpec of version 1..max_version, or GenesisSpecError with the reference's
    code and detail. The default ceiling is the release decoder (SPEC_VERSION 1, which refuses spec 2 with gsVersion until
    activation, V3-PR-I); 2 is the v3-preview decoder (CP schema v2). Returns {specVersion, chainId, cp, allocRoot,
    m0 [(id, reward)], sysCodeHash} with byte values."""
    b = bytes(b)
    _check_framing(b)
    top = _item(b, 0)
    if not top[0]:
        raise GenesisSpecError('gsStructure', 'top not a list')
    kinds, slot, top_len = (False, False, True, False, True, False), [None] * 6, 0
    for idx, it in enumerate(_children(b, top)):
        if idx < 6:
            if it[0] != kinds[idx]:
                raise GenesisSpecError('gsStructure', f'top[{idx}]')
            slot[idx] = it
        top_len += 1
    none = (True, 0, 0, False)
    cp, m0 = slot[2] or none, slot[4] or none
    if any(x[0] for x in _children(b, cp)):
        raise GenesisSpecError('gsStructure', 'CP item is a list')
    for e in _children(b, m0):
        if not e[0] or any(x[0] for x in _children(b, e)):
            raise GenesisSpecError('gsStructure', 'M_0List entry')
    payload = lambda it: b[it[1]:it[1] + it[2]]
    spec_version = 1
    if slot[0] is not None:
        raw = payload(slot[0])
        value = raw.lstrip(b'\0')
        if len(value) != 1 or value[0] > max_version or cp_schema(value[0]) is None:
            raise GenesisSpecError('gsVersion', str(int.from_bytes(raw, 'big')))
        spec_version = value[0]
    schema = cp_schema(spec_version)
    if not (all(x is not None for x in slot) and top_len == 6):
        raise GenesisSpecError('gsCount', f'top {top_len}')
    version, chain, _, alloc_root, _, sys_code_hash = slot
    cps, members = _children(b, cp), _children(b, m0)
    if len(cps) != len(schema):
        raise GenesisSpecError('gsCount', f'CP {len(cps)}')
    for e in members:
        n = len(_children(b, e))
        if n != 2:
            raise GenesisSpecError('gsCount', f'M_0List entry {n}')
    if not members:
        raise GenesisSpecError('gsCount', 'M_0List empty')
    for name, it in [('specVersion', version), ('chainId', chain)] + [(f[0], c) for f, c in zip(schema, cps)]:
        if it[3] or (it[2] > 0 and b[it[1]] == 0):
            raise GenesisSpecError('gsInt', name)
    if alloc_root[2] != 32 or sys_code_hash[2] != 32:
        raise GenesisSpecError('gsLen', 'root')
    pairs = [(_children(b, e) + [none, none])[:2] for e in members]
    if any(ident[2] != 20 or reward[2] != 20 for ident, reward in pairs):
        raise GenesisSpecError('gsLen', 'M_0List address')
    chain_bytes = payload(chain)
    if not 1 <= len(chain_bytes) <= 8:
        raise GenesisSpecError('gsRange', 'chainId')
    values = []
    for (name, width, lo, hi), it in zip(schema, cps):
        x = int.from_bytes(payload(it), 'big')
        if x.bit_length() > width or x < lo:
            raise GenesisSpecError('gsRange', name)
        values.append(x)
        bound = max(0, 10000 - values[_ALPHA_BP]) if hi == GAMMA else hi
        if bound is not None and x > bound:
            raise GenesisSpecError('gsRange', name)
    ids = [payload(ident) for ident, _ in pairs]
    if any(a >= b_ for a, b_ in zip(ids, ids[1:])):
        raise GenesisSpecError('gsOrder', 'ids')
    for ident in ids:
        if _is_reserved(ident):
            raise GenesisSpecError('gsSys', '0x' + ident.hex())
    return {'specVersion': spec_version, 'chainId': int.from_bytes(chain_bytes, 'big'), 'cp': values,
            'allocRoot': payload(alloc_root), 'm0': [(payload(i), payload(r)) for i, r in pairs],
            'sysCodeHash': payload(sys_code_hash)}


def public_spec(info, spec, max_version=SPEC_VERSION):
    """The decoded public GenesisSpec `spec` (bytes) of the network devnet.json `info` declares: the reference decoder at
    `max_version` (a release build's SPEC_VERSION by default; PREVIEW_SPEC_VERSION only for a v3-preview build), whose
    version must equal the declaration (spec_version) and whose chainId must be the network's. A spec-2 genesis must name
    the v3 program set (V3_SYS_CODE_HASH): pocold selects that set for it, so any other sysCodeHash cannot start. The
    genesis hash (keccak256 of the spec) is not recomputed here; the node checks it against the profile."""
    declared = spec_version(info)
    decoded = decode_genesis_spec(spec, max_version)
    if decoded['specVersion'] != declared:
        raise ValueError('GenesisSpec version differs from the network metadata')
    if decoded['chainId'] != info.get('chainId'):
        raise ValueError('GenesisSpec chain ID differs from the network metadata')
    if declared == PREVIEW_SPEC_VERSION and decoded['sysCodeHash'].hex() != V3_SYS_CODE_HASH:
        raise ValueError('a spec-2 (v3) GenesisSpec must name the v3 program set (sysCodeHash)')
    return decoded
