#!/usr/bin/env python3
"""Compile the existing fixtures with two locked tool installs and compare bytes.

Outputs go only to --output, never to tracked contract fixtures. No network,
package installation, or Rust build is performed by this helper.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before-tools', type=Path, required=True)
    parser.add_argument('--after-tools', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--jobs', type=int, choices=[1, 2], default=2)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    installs = {'before': args.before_tools.resolve(), 'after': args.after_tools.resolve()}
    for tools in installs.values():
        if not (tools / 'node_modules/solc/soljson.js').is_file():
            parser.error('Each tools directory must have its locked npm ci completed')
    if output.exists():
        parser.error('--output must be a new directory so evidence cannot be overwritten')
    output.mkdir(parents=True)
    scripts = sorted((repo / 'prototypes/lp4-execution/tests').glob('compile_*.cjs'))
    cases = [(script.stem + '.json', script, []) for script in scripts]
    cases.append(('continuous.json', repo / 'prototypes/lp4-execution/tests/compile_system_v1.cjs',
                  ['--continuous']))

    def compare(case):
        name, script, extra = case
        result = {'fixture': name, 'runs': {}}
        contents = {}
        for phase, tools in installs.items():
            directory = output / phase
            directory.mkdir(exist_ok=True)
            target = directory / name
            env = os.environ.copy()
            env['NODE_PATH'] = str(tools / 'node_modules')
            command = ['node', str(script), str(target), *extra]
            start = time.monotonic()
            try:
                run = subprocess.run(command, cwd=repo, env=env, capture_output=True,
                                     text=True, timeout=300)
                (directory / (name + '.log')).write_text(run.stdout + run.stderr)
                record = {'exit': run.returncode, 'seconds': round(time.monotonic() - start, 2)}
                if run.returncode == 0:
                    contents[phase] = target.read_bytes()
                    record['sha256'] = hashlib.sha256(contents[phase]).hexdigest()
                    record['compiler'] = json.loads(contents[phase])['compiler']
            except (OSError, ValueError, subprocess.TimeoutExpired) as error:
                record = {'exit': -1, 'error': str(error)}
            result['runs'][phase] = record
            print(f'{phase}: {name}: exit {record["exit"]}', flush=True)
        result['identical'] = (len(contents) == 2 and contents['before'] == contents['after'])
        return result

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(pool.map(compare, cases))
    report = {'cases': results, 'all_identical': all(item['identical'] for item in results)}
    (output / 'comparison.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'fixtures': len(results), 'all_identical': report['all_identical']}))
    return 0 if report['all_identical'] else 1


if __name__ == '__main__':
    sys.exit(main())
