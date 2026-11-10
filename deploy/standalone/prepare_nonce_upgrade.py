"""Prepare a new public invitation; never opens a wallet or contacts a node.

Activation must be agreed with every operator. The new binary's explicit `pocol upgrade` command validates and installs
this invitation on stopped homes. The output is always `pocol-network-v2`, bound to the consensus rules the new build
runs (plan §1.7). Each `--append V:H` adds one future fork entry (append-only); with none the output is a rebind.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'public-testnet'))
import pocol_identity  # noqa: E402  (the consensus identity and schedule reference, standard library only)


def prepare(invitation, build, appended, observed_head):
    if type(observed_head) is not int or not 0 <= observed_head < 2**64:
        raise ValueError('the observed head must be a uint64 height')
    if not isinstance(invitation, dict) or invitation.get('schema') not in ('pocol-network-v1', 'pocol-network-v2'):
        raise ValueError('unsupported network invitation')
    binding = 'compatibilitySha256' if invitation['schema'] == 'pocol-network-v1' else 'consensusRulesVersion'
    if set(invitation) != {'schema', binding, 'seeds', 'files'}:
        raise ValueError('unsupported network invitation')
    digest, rules = build.get('compatibilitySha256', ''), build.get('consensusRulesVersion')
    if (build.get('schema') != 'pocol-testnet-build-v1' or not re.fullmatch(r'[0-9a-f]{64}', digest)
            or type(rules) is not int or rules < 1):
        raise ValueError('use build-info from the exact new native release')
    if build.get('features') != []:
        raise ValueError('test-feature builds cannot prepare an operator release')
    if build.get('consensusPinned') is not True:
        raise ValueError('an unpinned consensus build cannot prepare an operator release')
    files = invitation['files']
    if set(files) != {'alloc.json', 'devnet.json', 'genesis.spec.hex'} or not all(isinstance(x, str) for x in files.values()):
        raise ValueError('expected exactly three public genesis files')
    info = json.loads(files['devnet.json'])
    if info.get('keyMode') != 'independent-key-files':
        raise ValueError('requires an independent-key network')
    # The new build must run the rules the existing file binds: v2 declares them; a v1 file has them only through a
    # certified LEGACY_BUILDS entry for its chain and genesis (none is certified as shipped: fail closed).
    current = (invitation['consensusRulesVersion'] if binding == 'consensusRulesVersion'
               else pocol_identity.legacy_rules(invitation['compatibilitySha256'], info.get('chainId'), info.get('genesisHash')))
    if type(current) is not int or current != rules:
        raise ValueError('the new build runs other consensus rules than this network invitation binds')
    old = [list(entry) for entry in pocol_identity.validated_schedule(info.get('forks', [[1, 0]]), build.get('maxForkVersion', 2))]
    entries = [list(entry) for entry in appended]
    if any(len(entry) != 2 or type(entry[1]) is not int or not observed_head < entry[1] < 2**64 for entry in entries):
        raise ValueError('each activation must be a uint64 height above the observed head')
    # Future-only and append-only: the runtime schedule rule over the whole result (versions in order, heights rising).
    forks = [list(entry) for entry in pocol_identity.validated_schedule(old + entries, build.get('maxForkVersion', 2))]
    # The operator selects only future rule changes, never a new genesis.
    updated = {'schema': 'pocol-network-v2', 'consensusRulesVersion': rules,
               'seeds': copy.deepcopy(invitation['seeds']), 'files': copy.deepcopy(files)}
    if entries:
        info['forks'] = forks
        updated['files']['devnet.json'] = json.dumps(info, indent=2) + '\n'
    return updated


def entry(value):
    match = re.fullmatch(r'([1-9][0-9]{0,4}):(0|[1-9][0-9]{0,19})', value)
    if not match:
        raise argparse.ArgumentTypeError('expected VERSION:HEIGHT, for example 2:150000')
    return [int(match[1]), int(match[2])]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--network', required=True, type=Path)
    parser.add_argument('--build-info', required=True, type=Path)
    parser.add_argument('--append', action='append', type=entry, default=[], metavar='VERSION:HEIGHT',
                        help='a future fork entry; repeat for several, omit for a rebind')
    parser.add_argument('--activation-height', type=int, help='shorthand for --append 2:HEIGHT')
    parser.add_argument('--observed-head', required=True, type=int)
    parser.add_argument('--out-dir', required=True, type=Path)
    args = parser.parse_args()
    appended = ([[2, args.activation_height]] if args.activation_height is not None else []) + args.append
    old_bytes = args.network.read_bytes()
    old = json.loads(old_bytes)
    new = prepare(old, json.loads(args.build_info.read_bytes()), appended, args.observed_head)
    # A new directory prevents replacing any previously approved release files.
    args.out_dir.mkdir(parents=True, exist_ok=False)
    encoded = (json.dumps(new, indent=2) + '\n').encode()
    (args.out_dir / 'network.pocol.json').write_bytes(encoded)
    # Browser profiles use decimal strings to avoid JavaScript integer rounding.
    forks = json.loads(new['files']['devnet.json']).get('forks', [[1, 0]])
    (args.out_dir / 'browser-fork-schedule.json').write_text(json.dumps([[v, str(h)] for v, h in forks], indent=2) + '\n')
    report = {
        'appended': [[v, str(h)] for v, h in appended], 'observedHead': str(args.observed_head),
        'consensusRulesVersion': new['consensusRulesVersion'],
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
