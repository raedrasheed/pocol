"""Prepare a new public invitation; never opens a wallet or contacts a node.

Activation must be agreed with every operator. The new binary's explicit
`pocol upgrade` command validates and installs this invitation on stopped homes.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re


def prepare(invitation, build, height, observed_head):
    if (type(height) is not int or type(observed_head) is not int
            or not 0 <= observed_head < height < 2**64):
        raise ValueError('activation must be a uint64 height above the observed head')
    if set(invitation) != {'schema', 'compatibilitySha256', 'seeds', 'files'} or invitation['schema'] != 'pocol-network-v1':
        raise ValueError('unsupported network invitation')
    digest = build.get('compatibilitySha256', '')
    if build.get('schema') != 'pocol-testnet-build-v1' or not re.fullmatch(r'[0-9a-f]{64}', digest):
        raise ValueError('use build-info from the exact new native release')
    if build.get('features') != []:
        raise ValueError('test-feature builds cannot prepare an operator release')
    files = invitation['files']
    if set(files) != {'alloc.json', 'devnet.json', 'genesis.spec.hex'} or not all(isinstance(x, str) for x in files.values()):
        raise ValueError('expected exactly three public genesis files')
    info = json.loads(files['devnet.json'])
    if info.get('keyMode') != 'independent-key-files' or info.get('forks', [[1, 0]]) != [[1, 0]]:
        raise ValueError('requires an independent-key v1-only network')
    # The operator selects only a future rule change, never a new genesis.
    updated = copy.deepcopy(invitation)
    updated['compatibilitySha256'] = digest
    info['forks'] = [[1, 0], [2, height]]
    updated['files']['devnet.json'] = json.dumps(info, indent=2) + '\n'
    return updated


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--network', required=True, type=Path)
    parser.add_argument('--build-info', required=True, type=Path)
    parser.add_argument('--activation-height', required=True, type=int)
    parser.add_argument('--observed-head', required=True, type=int)
    parser.add_argument('--out-dir', required=True, type=Path)
    args = parser.parse_args()
    old_bytes = args.network.read_bytes()
    old = json.loads(old_bytes)
    new = prepare(old, json.loads(args.build_info.read_bytes()), args.activation_height, args.observed_head)
    # A new directory prevents replacing any previously approved release files.
    args.out_dir.mkdir(parents=True, exist_ok=False)
    encoded = (json.dumps(new, indent=2) + '\n').encode()
    (args.out_dir / 'network.pocol.json').write_bytes(encoded)
    # Browser profiles use decimal strings to avoid JavaScript integer rounding.
    (args.out_dir / 'browser-fork-schedule.json').write_text(json.dumps([[1, '0'], [2, str(args.activation_height)]], indent=2) + '\n')
    report = {
        'activationHeight': str(args.activation_height), 'observedHead': str(args.observed_head),
        'oldInvitationSha256': hashlib.sha256(old_bytes).hexdigest(),
        'newInvitationSha256': hashlib.sha256(encoded).hexdigest(),
        'genesisFileUnchanged': old['files']['genesis.spec.hex'] == new['files']['genesis.spec.hex'],
        'allocationFileUnchanged': old['files']['alloc.json'] == new['files']['alloc.json'],
        'seedsUnchanged': old['seeds'] == new['seeds'],
        'deployed': False,
    }
    (args.out_dir / 'upgrade-plan.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
