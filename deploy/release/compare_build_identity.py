#!/usr/bin/env python3
"""The consensus identity of the Linux, Windows and Android builds, compared (final design §6 test 5, OWNER-DECISIONS-
2026-11-20 D6 E: device compatibility). Two steps, run by .github/workflows/five-device-build.yml:

  genesis --bin-dir DIR --out OUT
      On a platform whose binaries run (Linux, Windows): fills deploy/mainnet/v3-profile.json with FIXED throwaway
      values, keeping its chain id 2656089850 (genesisTime 1800000000, hash rate 1000000 H/s, and founder ids and reward
      addresses that are SHA-256 digests of public labels: addresses no one holds a key for, so the genesis can never be
      mined or launched), runs `pocold init-testnet` and `pocol network` + `pocol inspect` on it and writes OUT/devnet.json,
      OUT/summary.json (init-testnet's), OUT/described.json (the launcher's) and OUT/build-info.json (`pocol build-info`).
      The inputs are fixed, so every platform must produce the same genesis bytes.
  compare --genesis LABEL=OUT ... --build LABEL=BUILD_INFO_JSON ...
      Every genesis directory must hold the same chain, genesis hash and admission (scope public-network, the network
      profile on a network node); every build-info must agree on each field the consensus identity or peering reads
      (consensus rules version, protocol versions, highest fork version, pinned flag, consensus-sources aggregate,
      test-only features) and on the build digest; and the identity each build would send on the reference genesis
      (pocol_identity.for_network, the reference implementation of identity.rs) must be one value, equal to the one
      the binaries themselves computed (described.json) where they ran. The Android engine cannot run on the CI host, so
      its identity is computed from its raw build-info (the build.rs output the APK's engine was compiled with).
Exit 0 only if everything agrees; a JSON report is printed either way.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'deploy/public-testnet'))
import pocol_identity  # noqa: E402

PROFILE = ROOT / 'deploy/mainnet/v3-profile.json'
MAINNET_CHAIN_ID = 2656089850
# Every build-info field the consensus identity or the wire handshake reads, and the build digest.
IDENTITY_FIELDS = ('consensusRulesVersion', 'protocolVersion', 'protocolVersionMin', 'maxForkVersion', 'consensusPinned',
                   'consensusSourcesSha256', 'features')
DIGEST_FIELDS = ('compatibilitySha256', 'version')


def throwaway(label):
    """A 20-byte address from a public label: no one holds its key."""
    return '0x' + hashlib.sha256(b'pocol-ci-build-identity-throwaway\0' + label.encode()).hexdigest()[:40]


def filled_profile():
    p = json.loads(PROFILE.read_text(encoding='utf-8'))
    if p['chainId'] != MAINNET_CHAIN_ID:
        raise SystemExit(f'the mainnet profile names chain {p["chainId"]}, not {MAINNET_CHAIN_ID}')
    p['genesisTime'] = 1_800_000_000
    p['hashRate']['hashesPerSecond'] = 1_000_000
    for i, f in enumerate(p['founders'], 1):
        f['id'], f['reward'] = throwaway(f'founder-{i}-id'), throwaway(f'founder-{i}-reward')
    if 'PLACEHOLDER' in json.dumps(p):
        raise SystemExit('a ceremony placeholder is left')
    return p


def genesis(bin_dir, out):
    exe = '.exe' if os.name == 'nt' else ''
    call = lambda name, *args: subprocess.run([str(Path(bin_dir) / (name + exe)), *map(str, args)], check=True, capture_output=True, text=True, encoding='utf-8').stdout
    out = Path(out)
    out.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='pocol-build-identity-') as tmp:
        tmp = Path(tmp)
        (tmp / 'v3-profile.json').write_text(json.dumps(filled_profile()), encoding='utf-8')
        summary = json.loads(call('pocold', 'init-testnet', '--config', tmp / 'v3-profile.json', '--out', tmp / 'net'))
        call('pocol', 'network', '--from', tmp / 'net', '--seed', '127.0.0.1:30300', '--out', tmp / 'network.pocol.json')
        described = json.loads(call('pocol', 'inspect', '--network', tmp / 'network.pocol.json'))
        (out / 'devnet.json').write_bytes((tmp / 'net/devnet.json').read_bytes())
    (out / 'summary.json').write_text(json.dumps(summary, indent=1), encoding='utf-8')
    (out / 'described.json').write_text(json.dumps(described, indent=1), encoding='utf-8')
    (out / 'build-info.json').write_text(call('pocol', 'build-info'), encoding='utf-8')
    print(json.dumps({'chainId': summary['chainId'], 'genesisHash': summary['genesisHash'], 'consensusIdentity': described['consensusIdentity']}))


def labelled(values):
    out = {}
    for v in values:
        label, sep, path = v.partition('=')
        if not sep or not label or label in out:
            raise SystemExit(f'expected a distinct LABEL=PATH, got {v!r}')
        out[label] = Path(path)
    return out


def compare(genesis_dirs, builds):
    """The comparison report and the list of disagreements (empty: every build agrees)."""
    problems = []
    load = lambda p: json.loads(Path(p).read_text(encoding='utf-8'))
    geneses = {k: {'info': load(d / 'devnet.json'), 'summary': load(d / 'summary.json'), 'described': load(d / 'described.json'),
                   'build': load(d / 'build-info.json')} for k, d in genesis_dirs.items()}
    if not geneses:
        problems.append('no genesis directory: the reference genesis comes from a platform whose binaries run')
    if not builds:
        problems.append('no build-info to compare')
    infos = {k: g['info'] for k, g in geneses.items()}
    for k, g in geneses.items():
        s = g['summary']
        got = (s.get('chainId'), s.get('scope'), s.get('profileKind'), s.get('nodeKind'), s.get('specificationCompliant'))
        if got != (MAINNET_CHAIN_ID, 'public-network', 'network', 'node', False):
            problems.append(f'{k}: init-testnet admitted {got}, not the public network admission on chain {MAINNET_CHAIN_ID}')
        if g['info'].get('genesisHash') != s.get('genesisHash'):
            problems.append(f'{k}: devnet.json and the summary name different genesis hashes')
    hashes = {k: i.get('genesisHash') for k, i in infos.items()}
    if len(set(hashes.values())) > 1:
        problems.append(f'the fixed inputs gave different genesis bytes: {hashes}')
    reference = next(iter(infos.values()), None)
    builds = {k: load(p) for k, p in builds.items()}
    # The builds whose binaries also made a genesis are compared through their own build-info as well.
    for k, g in geneses.items():
        if k in builds and builds[k] != g['build']:
            problems.append(f'{k}: the build-info given differs from the one its genesis step recorded')
    report = {'chainId': MAINNET_CHAIN_ID, 'genesisHash': hashes, 'builds': {}}
    for field in IDENTITY_FIELDS + DIGEST_FIELDS:
        values = {k: b.get(field) for k, b in builds.items()}
        if any(v is None for v in values.values()):
            problems.append(f'{field} missing: {values}')
        if len({json.dumps(v, sort_keys=True) for v in values.values()}) > 1:
            problems.append(f'{field} differs: {values}')
    for k, b in builds.items():
        if b.get('consensusPinned') is not True:
            problems.append(f'{k}: an unpinned consensus build')
        if b.get('features'):
            problems.append(f'{k}: test-only features {b.get("features")}')
    identities = {}
    if reference is not None:
        for k, b in builds.items():
            try:
                identities[k] = pocol_identity.for_network(reference, b)
            except (KeyError, ValueError, TypeError) as e:
                problems.append(f'{k}: no consensus identity: {e}')
        for k, g in geneses.items():
            if g['described'].get('consensusIdentity') != identities.get(k, pocol_identity.for_network(reference, g['build'])):
                problems.append(f'{k}: the binary computed identity {g["described"].get("consensusIdentity")}, the reference implementation another')
        if len(set(identities.values())) > 1:
            problems.append(f'the builds would send different consensus identities: {identities}')
    for k, b in builds.items():
        report['builds'][k] = {**{f: b.get(f) for f in IDENTITY_FIELDS + DIGEST_FIELDS}, 'consensusIdentity': identities.get(k)}
    report['agree'] = not problems
    report['problems'] = problems
    return report


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='command', required=True)
    g = sub.add_parser('genesis')
    g.add_argument('--bin-dir', required=True)
    g.add_argument('--out', required=True)
    c = sub.add_parser('compare')
    c.add_argument('--genesis', action='append', default=[])
    c.add_argument('--build', action='append', default=[])
    a = ap.parse_args()
    if a.command == 'genesis':
        genesis(a.bin_dir, a.out)
        return 0
    report = compare(labelled(a.genesis), labelled(a.build))
    print(json.dumps(report, indent=1))
    return 0 if report['agree'] else 1


if __name__ == '__main__':
    sys.exit(main())
