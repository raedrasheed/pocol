#!/usr/bin/env python3
"""Build independent before/after graphs and compare public recovery and EVM results.

All manifests, locks, binaries and JSONL output belong to a new scratch output
directory. Existing checkouts and product fixtures are read only. Requires the
locked crates in the Cargo cache; this runner does not fetch from the network.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before-root', type=Path, required=True)
    parser.add_argument('--after-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        parser.error('--output must be new; previous evidence is never overwritten')
    output.mkdir(parents=True)
    sources = Path(__file__).resolve().parent
    env = os.environ.copy()
    env['CARGO_BUILD_JOBS'] = '2'
    cargo = shutil.which('cargo')
    if not cargo:
        parser.error('cargo must be on PATH')
    results = {'source_sha256': {name: sha(sources / name) for name in
                                ['equivalence.rs', 'fallback_equivalence.rs', 'run_equivalence.py']},
               'runs': {}, 'comparisons': {}}
    for phase, root in [('before', args.before_root.resolve()), ('after', args.after_root.resolve())]:
        for kind in ['full', 'fallback']:
            directory = output / f'{phase}-{kind}'
            directory.mkdir()
            source = sources / ('equivalence.rs' if kind == 'full' else 'fallback_equivalence.rs')
            name = 'pocol-evm-equivalence' if kind == 'full' else 'pocol-fallback-equivalence'
            deps = ['serde_json="=1.0.145"']
            if kind == 'full':
                deps += ['revm={version="=19.7.0",features=["serde"]}',
                         'revm-precompile="16.2.0"',
                         'pocold={path=' + json.dumps(str(root / 'prototypes/pocold')) + '}',
                         'pocol-lp4-execution={path=' + json.dumps(str(root / 'prototypes/lp4-execution')) + '}']
            else:
                deps += ['revm-precompile={version="16.2.0",default-features=false,features=["std"]}']
                if phase == 'after':
                    deps += ['elliptic-curve={version="=0.14.3",default-features=false}']
            manifest = ('[package]\nname=' + json.dumps(name) + '\nversion="0.0.0"\n'
                        'edition="2021"\npublish=false\n[dependencies]\n' + '\n'.join(deps) +
                        '\n[[bin]]\nname=' + json.dumps(name) + '\npath=' + json.dumps(str(source)) +
                        '\n[workspace]\n')
            if phase == 'after':
                manifest += '[patch.crates-io]\n'
                for package, version in [('alloy-primitives', '0.8.26'),
                                         ('alloy-eip7702', '0.5.1'), ('revm-precompile', '16.2.0')]:
                    path = root / 'prototypes/third-party/evm-crypto' / f'{package}-{version}'
                    manifest += package + '={path=' + json.dumps(str(path)) + '}\n'
            (directory / 'Cargo.toml').write_text(manifest)
            shutil.copyfile(root / 'prototypes/pocold/Cargo.lock', directory / 'Cargo.lock')
            command = [cargo, '+1.99.1', 'build', '--release', '--offline',
                       '--manifest-path', str(directory / 'Cargo.toml')]
            with (directory / 'build.log').open('w') as log:
                build = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=600)
            if build.returncode:
                print(f'{phase}-{kind}: build failed; see {directory / "build.log"}', flush=True)
                return build.returncode
            metadata = subprocess.check_output([cargo, '+1.99.1', 'metadata', '--locked', '--offline',
                                                '--format-version', '1', '--manifest-path',
                                                str(directory / 'Cargo.toml')], env=env, text=True)
            resolved = json.loads(metadata)
            target = Path(resolved['target_directory']) / 'release' / name
            if os.name == 'nt':
                target = target.with_suffix('.exe')
            binary = directory / target.name
            shutil.copyfile(target, binary)
            if os.name != 'nt':
                binary.chmod(0o755)
            selected = [{'name': p['name'], 'version': p['version'], 'source': p['source']}
                        for p in resolved['packages'] if p['name'] in
                        ['crypto-bigint', 'k256', 'revm', 'revm-precompile', 'alloy-primitives', 'alloy-eip7702']]
            if kind == 'fallback':
                ids = {p['id'] for p in resolved['packages'] if p['name'] == 'revm-precompile'}
                assert all('secp256k1' not in n['features'] for n in resolved['resolve']['nodes'] if n['id'] in ids)
            modes = ['crypto', 'evm'] if kind == 'full' else ['fallback']
            for mode in modes:
                arguments = [str(binary), mode] if kind == 'full' else [str(binary), str(output / f'{phase}-crypto.jsonl')]
                data = output / f'{phase}-{mode}.jsonl'
                with data.open('w') as stream, (directory / f'{mode}.log').open('w') as log:
                    run = subprocess.run(arguments, env=env, stdout=stream, stderr=log, timeout=600)
                count = sum(1 for _ in data.open())
                results['runs'][f'{phase}-{mode}'] = {'exit': run.returncode, 'cases': count,
                    'sha256': sha(data), 'graph': selected, 'binary_sha256': sha(binary)}
                print(f'{phase}-{mode}: exit {run.returncode}, {count} cases', flush=True)
                if run.returncode:
                    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
                    return run.returncode
    for mode in ['crypto', 'evm', 'fallback']:
        before, after = output / f'before-{mode}.jsonl', output / f'after-{mode}.jsonl'
        equal = sha(before) == sha(after)
        record = {'identical': equal, 'before_sha256': sha(before), 'after_sha256': sha(after)}
        if not equal:
            with before.open() as a, after.open() as b:
                for index, (old, new) in enumerate(zip(a, b)):
                    if old != new:
                        record['first_mismatch'] = {'line': index + 1, 'before': json.loads(old), 'after': json.loads(new)}
                        break
        results['comparisons'][mode] = record
    results['ok'] = all(item['identical'] for item in results['comparisons'].values())
    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps({'ok': results['ok'], 'comparisons': results['comparisons']}), flush=True)
    return 0 if results['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
