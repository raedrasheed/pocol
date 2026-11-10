#!/usr/bin/env python3
"""The PoCol consensus identity (VA-SPEC §2.1), computed from public data only.

A second reference implementation of pocold/src/identity.rs consensus_id: two nodes
peer only when they send the same identity. Standard library only, so it can be
copied into single-file tools (deploy/mainnet/preflight.py keeps an inline copy)."""
import hashlib

DOMAIN = b'pocol-consensus-identity-v1\0'
FLAG_CAPS_TEST = 1
FLAG_UNPINNED = 2
# The highest consensus version these tools verify (pocold profile MAX_FORK_VERSION).
MAX_FORK_VERSION = 2
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
    (version u16 || height u64) x n || [sources, only with the unpinned bit]), big-endian.
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
    if bool(flags & FLAG_UNPINNED) != (sources is not None):
        raise ValueError('the unpinned bit and the aggregate go together')
    data = DOMAIN + chain_id.to_bytes(8, 'big') + raw(genesis) + rules.to_bytes(4, 'big') + bytes([flags, len(forks)])
    data += b''.join(v.to_bytes(2, 'big') + h.to_bytes(8, 'big') for v, h in forks)
    if sources is not None:
        data += raw(sources)
    return hashlib.sha256(data).hexdigest()


def build_flags(build):
    """The flags a build reports through `pocold build-info`."""
    return (FLAG_CAPS_TEST if 'caps-test' in build.get('features', []) else 0) | (0 if build['consensusPinned'] else FLAG_UNPINNED)


def for_network(info, build):
    """The identity a node of build-info `build` sends on the network of devnet.json `info`."""
    flags = build_flags(build)
    return consensus_id(info['chainId'], info['genesisHash'], build['consensusRulesVersion'], flags,
                        validated_schedule(info.get('forks', [[1, 0]]), build.get('maxForkVersion', MAX_FORK_VERSION)),
                        build['consensusSourcesSha256'] if flags & FLAG_UNPINNED else None)
