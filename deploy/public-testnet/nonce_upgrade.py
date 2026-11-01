#!/usr/bin/env python3
"""Prepare a seed bundle and migrate its stopped state binding for full nonce v2.

Offline only: no service start, installation, key read, or chain-state mutation.
Use the same approved activation height on every node and browser verifier.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys

import node_operator as op

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MAX_RECORD = 4 * 1024 * 1024
INTENT = 'nonce-upgrade.intent.json'
BACKUP = 'network-binding.pre-full-nonce.json'
MUTABLE = {'manifest.json', 'build-info.json', 'net/devnet.json',
           'node_operator.py', 'nonce_upgrade.py'} | {'bin/' + n for n in op.BINARIES}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def regular(path):
    mode = path.lstat()
    require(stat.S_ISREG(mode.st_mode) and mode.st_nlink == 1,
            'Expected a regular single-link file: ' + str(path))
    return mode


def read_bytes(path, limit=4 * 1024 * 1024):
    regular(path)
    with path.open('rb') as stream:
        value = stream.read(limit + 1)
    require(len(value) <= limit, 'File is too large: ' + str(path))
    return value


def fork_transition(before, after):
    require(isinstance(before, dict) and isinstance(after, dict), 'Invalid network metadata')
    old = copy.deepcopy(before)
    new = copy.deepcopy(after)
    def v1_start(value):
        return (isinstance(value, list) and len(value) == 2
                and all(type(x) is int for x in value) and value == [1, 0])
    old_forks = old.pop('forks', [[1, 0]])
    require(isinstance(old_forks, list) and len(old_forks) == 1 and v1_start(old_forks[0]),
            'Existing network must be v1-only')
    forks = new.pop('forks', None)
    require(isinstance(forks, list) and len(forks) == 2 and v1_start(forks[0])
            and isinstance(forks[1], list) and len(forks[1]) == 2
            and type(forks[1][0]) is int and forks[1][0] == 2
            and type(forks[1][1]) is int and 0 < forks[1][1] < 2**64,
            'Expected one future full-nonce v2 activation')
    require(old == new and old.get('keyMode') == 'independent-key-files',
            'Only the fork schedule may change in network metadata')
    return forks[1][1]


def validate_build(build):
    require(isinstance(build, dict) and build.get('schema') == 'pocol-testnet-build-v1'
            and build.get('features') == [] and build.get('protocolVersion') == 2,
            'Use a feature-free wire-v2 native release')
    sources = build.get('sourceSha256')
    require(isinstance(sources, dict) and {'pocold/src/chain.rs', 'lp1-node/src/profile.rs'} <= sources.keys(),
            'Build source inventory is missing consensus inputs')
    digest = hashlib.sha256(b'pocol-testnet-source-compatibility-v1\0')
    for name, expected in sorted(sources.items()):
        require(isinstance(name, str) and not Path(name).is_absolute()
                and '..' not in Path(name).parts and '\\' not in name,
                'Unsafe build source path')
        content = read_bytes(ROOT / 'prototypes' / name)
        require(hashlib.sha256(content).hexdigest() == expected, 'Rebuild stale native release: ' + name)
        encoded = name.encode()
        digest.update(len(encoded).to_bytes(8, 'big')); digest.update(encoded)
        digest.update(len(content).to_bytes(8, 'big')); digest.update(content)
    require(build.get('compatibilitySha256') == digest.hexdigest(), 'Build compatibility digest mismatch')


def prepare_bundle(old_bundle, binary_dir, invitation_file, out, expected_old=None):
    """Create a new bundle; the existing installation and state remain untouched."""
    require(not out.resolve().is_relative_to(old_bundle.resolve()),
            'New bundle must be outside the existing bundle')
    old_manifest, _ = op.verify(old_bundle, expected_old)
    invitation = op.read_json(invitation_file)
    require(isinstance(invitation, dict) and set(invitation) == {'schema', 'compatibilitySha256', 'seeds', 'files'}
            and invitation['schema'] == 'pocol-network-v1', 'Invalid new invitation')
    public = invitation['files']
    require(isinstance(public, dict) and set(public) == {'alloc.json', 'devnet.json', 'genesis.spec.hex'}
            and all(isinstance(v, str) for v in public.values()), 'Invalid public network files')
    for name in ('genesis.spec.hex', 'alloc.json'):
        require(public[name].encode() == read_bytes(old_bundle / 'net' / name),
                'The existing genesis and allocations must remain byte-identical')
    # The invitation's discovery addresses are not installed here. The existing
    # node.json (including every configured peer and endpoint) is preserved.
    new_info = json.loads(public['devnet.json'])
    activation = fork_transition(op.read_json(old_bundle / 'net/devnet.json'), new_info)
    for name in op.BINARIES:
        regular(binary_dir / name)
    build = json.loads(subprocess.check_output([str(binary_dir / 'pocold'), 'build-info'],
                                              text=True, timeout=10))
    validate_build(build)
    require(invitation['compatibilitySha256'] == build['compatibilitySha256'],
            'Invitation and new native binaries disagree')
    require(not out.exists() and not out.is_symlink(), 'Never overwrite an existing seed bundle')
    out.mkdir(parents=True, mode=0o700)
    for name, source in op.files(old_bundle).items():
        if name == 'manifest.json':
            continue
        target = out / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    for name in op.BINARIES:
        shutil.copy2(binary_dir / name, out / 'bin' / name)
    for name in ('node_operator.py', 'nonce_upgrade.py'):
        shutil.copy2(HERE / name, out / name)
    (out / 'net/devnet.json').write_text(public['devnet.json'], encoding='utf-8')
    (out / 'build-info.json').write_text(json.dumps(build, indent=2) + '\n', encoding='utf-8')
    config = op.read_json(out / 'node.json')
    manifest = copy.deepcopy(old_manifest)
    manifest['identity'] = op.identity(config, new_info, build)
    manifest['sha256'] = {name: op.digest(path) for name, path in op.files(out).items()}
    op.write_json(out / 'manifest.json', manifest)
    expected_new = op.digest(out / 'manifest.json')
    verify_transition(old_bundle, out, expected_new)
    return {'bundle': str(out), 'bundleManifestSha256': expected_new,
            'activationHeight': str(activation), 'deploymentPerformed': False}


def verify_transition(old_bundle, new_bundle, expected_new):
    require(isinstance(expected_new, str) and re.fullmatch(r'[0-9a-f]{64}', expected_new),
            'A trusted new bundle manifest SHA256 is required')
    old, old_config = op.verify(old_bundle)
    new, new_config = op.verify(new_bundle, expected_new)
    for bundle in (old_bundle, new_bundle):
        build = op.read_json(bundle / 'build-info.json')
        require(isinstance(build.get('compatibilitySha256'), str)
                and re.fullmatch(r'[0-9a-f]{64}', build['compatibilitySha256']),
                'Invalid release compatibility digest')
    require(old_config == new_config, 'Node identity, miner, endpoints and peers must not change')
    for key in set(old) | set(new):
        if key not in {'identity', 'sha256'}:
            require(old.get(key) == new.get(key), 'Bundle metadata must remain unchanged')
    old_identity, new_identity = copy.deepcopy(old['identity']), copy.deepcopy(new['identity'])
    old_identity.pop('compatibilitySha256'); new_identity.pop('compatibilitySha256')
    require(old_identity == new_identity, 'Only the release compatibility binding may change')
    before, after = op.files(old_bundle), op.files(new_bundle)
    require(set(before) - MUTABLE == set(after) - MUTABLE, 'Unexpected bundle inventory change')
    for name in set(before) - MUTABLE:
        require(op.digest(before[name]) == op.digest(after[name]), 'Protected bundle file changed: ' + name)
    activation = fork_transition(op.read_json(old_bundle / 'net/devnet.json'),
                                 op.read_json(new_bundle / 'net/devnet.json'))
    return old, new, activation


def rlp(raw):
    """Canonical, depth-bounded journal/header decoding; no execution or signature claim."""
    def one(pos, depth):
        require(depth <= 6 and pos < len(raw), 'Malformed journal RLP')
        lead = raw[pos]
        if lead < 128:
            return raw[pos:pos+1], pos+1
        listed = lead >= 192
        short, long = (192, 247) if listed else (128, 183)
        if lead <= long:
            length, start = lead-short, pos+1
        else:
            width = lead-long
            require(width <= 4 and pos+1+width <= len(raw), 'Malformed RLP length')
            size = raw[pos+1:pos+1+width]
            require(size[0] != 0, 'Noncanonical RLP length')
            length, start = int.from_bytes(size, 'big'), pos+1+width
            require(length >= 56, 'Noncanonical long RLP')
        end = start+length
        require(end <= len(raw), 'Truncated journal RLP')
        if not listed:
            value = raw[start:end]
            require(not (length == 1 and value[0] < 128), 'Noncanonical RLP string')
            return value, end
        values = []
        while start < end:
            value, start = one(start, depth+1)
            require(start <= end and len(values) < 1024, 'Invalid journal RLP list')
            values.append(value)
        return values, end
    result, end = one(0, 0)
    require(end == len(raw), 'Trailing journal RLP')
    return result


def header_height(payload, genesis, chain_id):
    fields = rlp(payload)
    require(isinstance(fields, list) and len(fields) == 3 and all(isinstance(x, bytes) for x in fields),
            'Invalid journal record payload')
    require(0 < len(fields[0]) <= 3067, 'Invalid journal header length')
    header = rlp(fields[0])
    require(isinstance(header, list) and len(header) == 4 and isinstance(header[0], list)
            and len(header[0]) == 2, 'Invalid journal header')
    unsigned = header[0][0]
    require(isinstance(unsigned, list) and len(unsigned) == 18 and unsigned[0] == b'PoCol-tpl-v1',
            'Invalid unsigned journal header')
    def integer(index, width):
        value = unsigned[index]
        require(isinstance(value, bytes) and len(value) <= width and (not value or value[0]),
                'Invalid header integer')
        return int.from_bytes(value, 'big')
    require(unsigned[2] == genesis and integer(1, 8) == chain_id and integer(6, 2) == 1,
            'Journal header differs from the existing v1 network')
    height = integer(4, 8)
    require(height > 0, 'Invalid stored block height')
    return height


def highest_stored_height(data, info):
    """Read every complete branch without repairing journal or commit files."""
    genesis_hex = info.get('genesisHash')
    require(isinstance(genesis_hex, str) and re.fullmatch(r'0x[0-9a-fA-F]{64}', genesis_hex),
            'Invalid genesis identity')
    genesis = bytes.fromhex(genesis_hex[2:])
    require(type(info.get('chainId')) is int and 0 < info['chainId'] < 2**64,
            'Invalid chain identity')
    commit = read_bytes(data / 'blocks.commit', 4160)
    require(len(commit) in (128, 4160), 'Recover the commit file with the existing release first')
    slots = [commit[:64], commit[64:128] if len(commit) == 128 else commit[4096:4160]]
    valid = []
    for slot in slots:
        if slot == bytes(64):
            continue
        nums = tuple(int.from_bytes(slot[i:i+8], 'big') for i in (0, 8, 16))
        expected = hashlib.sha256(b'POCOL-COMMIT' + genesis + slot[:24]).digest()
        require(slot[24:56] == expected and slot[56:] == bytes(8),
                'Recover a damaged commit slot with the existing release first')
        valid.append(nums)
    require(valid, 'No valid journal commit slot')
    _, count, committed_end = max(valid, key=lambda value: value[0])
    require(committed_end >= 40, 'Invalid journal commit boundary')
    regular(data / 'blocks.journal')
    highest, records, position = 0, 0, 40
    matched_commit = count == 0 and committed_end == 40
    with (data / 'blocks.journal').open('rb') as stream:
        require(stream.read(40) == b'POCOLJ2\n' + genesis, 'Journal network/version mismatch')
        while True:
            frame = stream.read(8)
            if not frame:
                break
            require(len(frame) == 8, 'Recover a torn journal with the existing release first')
            size = int.from_bytes(frame[:4], 'big')
            require(size <= MAX_RECORD and frame[4:] == hashlib.sha256(b'POCOL-REC-LEN' + frame[:4]).digest()[:4],
                    'Invalid journal frame')
            content = stream.read(32 + size + 1)
            require(len(content) == 32 + size + 1 and content[-1] == 1,
                    'Recover a torn journal with the existing release first')
            payload = content[32:-1]
            require(hashlib.sha256(payload).digest() == content[:32], 'Invalid journal checksum')
            highest = max(highest, header_height(payload, genesis, info['chainId']))
            records += 1; position += size + 41
            if records == count:
                matched_commit = position == committed_end
            require(records <= count + 1, 'Journal has more than one uncommitted record')
    require(matched_commit and count <= records <= count + 1, 'Journal does not match its commit boundary')
    return highest


def atomic_public_write(path, content, owner):
    """Resume only this operation's expected temporary bytes, preserving ownership."""
    temporary = path.with_name(path.name + '.nonce-upgrade.tmp')
    # Validate the opened descriptor before truncation. A path-based precheck
    # cannot protect an existing file replaced by a hard link before open().
    flags = os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK
    descriptor = os.open(temporary, flags, 0o600)
    with os.fdopen(descriptor, 'r+b') as stream:
        meta = os.fstat(stream.fileno())
        require(stat.S_ISREG(meta.st_mode) and meta.st_nlink == 1, 'Invalid upgrade temporary file')
        saved = stream.read(len(content) + 1)
        require(content.startswith(saved), 'Unexpected upgrade temporary file; preserve it for review')
        os.fchmod(stream.fileno(), 0o600)
        if (meta.st_uid, meta.st_gid) != owner:
            os.fchown(stream.fileno(), *owner)
        stream.seek(0); stream.truncate()
        stream.write(content); stream.flush(); os.fsync(stream.fileno())
    os.replace(temporary, path)
    op.sync_dir(path.parent)


def migrate_binding(old_bundle, new_bundle, data, expected_new):
    """Migrate only the release marker under both stopped-node locks; resumable."""
    old, new, activation = verify_transition(old_bundle, new_bundle, expected_new)
    actual_build = json.loads(subprocess.check_output([str(new_bundle / 'bin/pocold'), 'build-info'],
                                                     text=True, timeout=10))
    require(actual_build == op.read_json(new_bundle / 'build-info.json'),
            'New binary and bundle build identity differ')
    require(data.is_dir() and not data.is_symlink(), 'Use the existing real state directory')
    require(not data.resolve().is_relative_to(old_bundle.resolve())
            and not data.resolve().is_relative_to(new_bundle.resolve()),
            'State must remain separate from both public seed bundles')
    marker = data / 'network-binding.json'
    metadata = regular(marker)
    owner = (metadata.st_uid, metadata.st_gid)
    old_info = op.read_json(old_bundle / 'net/devnet.json')
    with op.locked(data):
        current = op.read_json(marker)
        before, after = old['identity'], new['identity']
        intent = {'schema': 'pocol-full-nonce-seed-upgrade-v1',
                  'oldManifestSha256': op.digest(old_bundle / 'manifest.json'),
                  'newManifestSha256': expected_new, 'before': before, 'after': after,
                  'activationHeight': str(activation)}
        intent_path = data / INTENT
        if intent_path.exists() or intent_path.is_symlink():
            regular(intent_path)
            require(op.read_json(intent_path) == intent, 'Resume with exactly the same approved seed bundles')
        else:
            require(current == before, 'State binding does not match the old seed bundle')
        require(current in (before, after), 'Unexpected state release binding')
        # Check again on retry, including all side branches and a complete but
        # uncommitted frame. Never alter journal/checkpoint/history data here.
        highest = highest_stored_height(data, old_info)
        require(activation > highest, 'Activation must be above every stored block: ' + str(highest))
        backup = data / BACKUP
        if backup.exists() or backup.is_symlink():
            regular(backup)
            require(op.read_json(backup) == before, 'Existing binding backup differs; preserve it for review')
        else:
            require(current == before, 'Missing original binding backup')
            atomic_public_write(backup, read_bytes(marker), owner)
        encode = lambda value: (json.dumps(value, indent=2) + '\n').encode()
        if not intent_path.exists():
            atomic_public_write(intent_path, encode(intent), owner)
        if current != after:
            atomic_public_write(marker, encode(after), owner)
        return {'bindingMigrated': True, 'highestStoredBlock': str(highest),
                'activationHeight': str(activation), 'backup': str(backup),
                'newBundleManifestSha256': expected_new, 'serviceStarted': False,
                'chainStateChanged': False, 'keysAccessed': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    prepare = sub.add_parser('prepare')
    prepare.add_argument('--old-bundle', required=True, type=Path)
    prepare.add_argument('--expected-old-manifest', required=True)
    prepare.add_argument('--bin-dir', required=True, type=Path)
    prepare.add_argument('--network', required=True, type=Path)
    prepare.add_argument('--out', required=True, type=Path)
    migrate = sub.add_parser('migrate')
    migrate.add_argument('--old-bundle', required=True, type=Path)
    migrate.add_argument('--new-bundle', required=True, type=Path)
    migrate.add_argument('--expected-new-manifest', required=True)
    migrate.add_argument('--data', required=True, type=Path)
    args = parser.parse_args()
    if args.command == 'prepare':
        result = prepare_bundle(args.old_bundle.absolute(), args.bin_dir.absolute(), args.network,
                                args.out.absolute(), args.expected_old_manifest)
    else:
        result = migrate_binding(args.old_bundle.absolute(), args.new_bundle.absolute(),
                                 args.data.absolute(), args.expected_new_manifest)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print('full nonce seed upgrade: ' + str(error), file=sys.stderr)
        sys.exit(1)
