#!/usr/bin/env python3
"""The consensus lock's history is append-only (VA-SPEC §2.4, plan §1.5).

The history lines of prototypes/pocold/consensus-sources.lock on the base ref
(origin/main by default) must be an exact prefix of this tree's. `file` and
`dep` lines are rewritten by every re-pin and are not compared. A lock that the
base does not have yet passes (PR5 creates it). A base ref that is not available
fails closed: CI checks out with fetch-depth 0 so that origin/main exists.
Run: python3 deploy/ci/check_lock_append_only.py [--base origin/main]
"""
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
LOCK = 'prototypes/pocold/consensus-sources.lock'


def history(text):
    return [line.rstrip('\r') for line in text.splitlines() if line.startswith('history ')]


def check(base_text, head_text):
    """None when head keeps base's history as a prefix, else the reason."""
    if base_text is None:
        return None
    if head_text is None:
        return 'the base has a consensus lock and this tree removed it'
    base, head = history(base_text), history(head_text)
    if not base:
        return 'the base lock has no history line'
    for i, line in enumerate(base):
        if i >= len(head):
            return f'history line {i + 1} of the base was removed'
        if head[i] != line:
            return f'history line {i + 1} differs from the base (never edit history lines; append one):\n  base: {line}\n  here: {head[i]}'
    return None


def git(*args):
    return subprocess.run(['git', '-C', str(ROOT), *args], capture_output=True, text=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--base', default='origin/main')
    args = parser.parse_args(argv)
    if git('cat-file', '-e', f'{args.base}^{{commit}}').returncode:
        print(f'base ref {args.base} is not available; fetch it (CI: actions/checkout with fetch-depth: 0)', file=sys.stderr)
        return 2
    exists = git('cat-file', '-e', f'{args.base}:{LOCK}').returncode == 0
    base_text = git('show', f'{args.base}:{LOCK}').stdout if exists else None
    path = ROOT / LOCK
    head_text = path.read_text(encoding='utf-8') if path.exists() else None
    why = check(base_text, head_text)
    if why:
        print(f'{LOCK}: {why}', file=sys.stderr)
        return 1
    print(f'{LOCK}: history is append-only against {args.base}' + ('' if exists else ' (the lock is new)'))
    return 0


if __name__ == '__main__':
    sys.exit(main())
