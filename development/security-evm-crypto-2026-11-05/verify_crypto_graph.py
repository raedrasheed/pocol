#!/usr/bin/env python3
"""Verify saved locked Cargo metadata for the complete active EVM crypto graph.

This reads metadata only; it performs no network access or package mutation.
It checks dependency routing and versions, not constant-time code generation.
"""
import argparse
import json
from pathlib import Path
import re
import sys


def version(value):
    if not re.fullmatch(r'\d+\.\d+\.\d+', value):
        raise ValueError('Unexpected cryptographic dependency version: ' + value)
    return tuple(map(int, value.split('.')))


def inspect(metadata, root):
    packages = metadata['packages']
    problems = []
    floors = {'crypto-bigint': (0, 7, 8), 'ctutils': (0, 4, 4),
              'k256': (0, 14, 0), 'elliptic-curve': (0, 14, 3)}
    crypto = {}
    for name, floor in floors.items():
        selected = sorted({p['version'] for p in packages if p['name'] == name})
        crypto[name] = selected
        if not selected or any(version(v) < floor for v in selected):
            problems.append(f'{name} missing or below the reviewed version floor')
    forks = {}
    for name, upstream in [('alloy-primitives', '0.8.26'), ('alloy-eip7702', '0.5.1'),
                           ('revm-precompile', '16.2.0')]:
        matches = [p for p in packages if p['name'] == name]
        expected = root / 'prototypes/third-party/evm-crypto' / f'{name}-{upstream}' / 'Cargo.toml'
        valid = (len(matches) == 1 and matches[0]['version'] == upstream + '+pocol.crypto.1'
                 and matches[0]['source'] is None
                 and Path(matches[0]['manifest_path']).resolve() == expected.resolve())
        forks[name] = valid
        if not valid:
            problems.append(name + ' does not resolve to its reviewed local compatibility patch')
    engine = [p for p in packages if p['name'] == 'revm']
    if len(engine) != 1 or engine[0]['version'] != '19.7.0':
        problems.append('The reviewed EVM engine version changed')
    return {'ok': not problems, 'cryptographic_versions': crypto,
            'local_patches_selected': forks, 'engine': [p['version'] for p in engine],
            'problems': problems,
            'scope': 'Complete active metadata graph; unrelated historical tools and prototypes are separate'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata', type=Path, required=True)
    parser.add_argument('--repo-root', type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    try:
        report = inspect(json.loads(args.metadata.read_text(encoding='utf-8')), args.repo_root.resolve())
    except (OSError, ValueError, KeyError, TypeError) as error:
        report = {'ok': False, 'error': str(error)}
    print(json.dumps(report, indent=2))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
