#!/usr/bin/env python3
"""Check the private signer's resolved dependency closure, without network or writes.

The legacy recovery/EVM graph is reported separately and is not suppressed.
Use --metadata with a saved `cargo metadata --format-version 1` document, or run
after fetching the locked dependencies. This is a routing/version check, not a
proof of constant-time code generation on every compiler or CPU.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys


def version(text):
    if '-' in text:
        raise ValueError('Pre-release versions need explicit review')
    return tuple(int(part) for part in text.split('.'))


def inspect(metadata):
    packages = {package['id']: package for package in metadata['packages']}
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    roots = [package for package in packages.values()
             if package['name'] == 'pocol-lp4-execution']
    if len(roots) != 1:
        raise ValueError('Expected exactly one execution package')
    dependencies = {dep['name']: dep['pkg'] for dep in nodes[roots[0]['id']]['deps']}
    name = 'k256_signer' if 'k256_signer' in dependencies else 'k256'
    entry = dependencies[name]
    pending, reached = [entry], set()
    while pending:
        identifier = pending.pop()
        if identifier in reached:
            continue
        reached.add(identifier)
        pending.extend(dep['pkg'] for dep in nodes[identifier]['deps'])
    closure = [packages[identifier] for identifier in reached]
    bigint = [package for package in closure if package['name'] == 'crypto-bigint']
    ctutils = [package for package in closure if package['name'] == 'ctutils']
    problems = []
    if not bigint or any(version(p['version']) < (0, 7, 8) for p in bigint):
        problems.append('Private signing reaches an uncorrected crypto-bigint stack')
    if not ctutils or any(version(p['version']) < (0, 4, 4) for p in ctutils):
        problems.append('Private signing lacks the corrected ctutils stack')
    if version(packages[entry]['version']) < (0, 14, 0):
        problems.append('Private signing still resolves to the legacy k256 backend')
    selected = sorted(({'name': p['name'], 'version': p['version']}
                       for p in closure if p['name'] in
                       ['k256', 'ecdsa', 'elliptic-curve', 'crypto-bigint', 'ctutils']),
                      key=lambda p: (p['name'], p['version']))
    remaining = sorted({p['version'] for p in packages.values()
                        if p['name'] == 'crypto-bigint' and
                        version(p['version']) < (0, 7, 6)})
    return {
        'ok': not problems,
        'private_dependency_name': name,
        'private_signer_stack': selected,
        'private_closure_packages': len(reached),
        'legacy_crypto_bigint_elsewhere': remaining,
        'problems': problems,
        'scope': 'Private signer routing and version floors only; legacy audit findings remain visible',
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata', type=Path)
    args = parser.parse_args()
    try:
        if args.metadata:
            data = json.loads(args.metadata.read_text(encoding='utf-8'))
        else:
            repo = Path(__file__).resolve().parents[2]
            command = ['cargo', 'metadata', '--locked', '--offline', '--format-version', '1',
                       '--manifest-path', str(repo / 'prototypes/lp4-execution/Cargo.toml')]
            result = subprocess.run(command, capture_output=True, text=True, timeout=120)
            if result.returncode:
                raise ValueError('Locked offline Cargo metadata failed; fetch dependencies first')
            data = json.loads(result.stdout)
        report = inspect(data)
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print(json.dumps({'ok': False, 'error': str(error)}))
        return 1
    print(json.dumps(report, indent=2))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
