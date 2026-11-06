#!/usr/bin/env python3
"""Preserve CodeQL diagnostic metadata without treating job success as full quality.

The original SARIF remains the authoritative artifact. This summary never writes
GitHub alert states, suppresses results, or invents extraction coverage metrics.
"""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sarif-dir', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    files = sorted(args.sarif_dir.rglob('*.sarif'))
    if not files:
        parser.error('No SARIF files were generated')
    summaries = []
    for path in files:
        document = json.loads(path.read_text(encoding='utf-8'))
        for run in document.get('runs', []):
            invocations = [{key: invocation.get(key) for key in
                            ['executionSuccessful', 'toolExecutionNotifications',
                             'toolConfigurationNotifications', 'properties'] if key in invocation}
                           for invocation in run.get('invocations', [])]
            summaries.append({'sarif': str(path.relative_to(args.sarif_dir)),
                              'driver': run.get('tool', {}).get('driver', {}).get('name'),
                              'version': run.get('tool', {}).get('driver', {}).get('version'),
                              'results': len(run.get('results', [])),
                              'invocations': invocations, 'run_properties': run.get('properties')})
    report = {'scope': 'Built C#/Go diagnostic configuration; existing default setup is unchanged',
              'default_setup_quality_improvement_verified': False, 'runs': summaries}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'sarif_files': len(files), 'runs': len(summaries),
                      'raw_diagnostics_preserved': True}))


if __name__ == '__main__':
    main()
