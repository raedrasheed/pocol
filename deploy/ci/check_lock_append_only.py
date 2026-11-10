#!/usr/bin/env python3
"""The consensus lock's history is append-only (VA-SPEC §2.4, plan §1.5).

The history lines of prototypes/pocold/consensus-sources.lock on the base ref
(origin/main by default) must be an exact prefix of this tree's. `file` and
`dep` lines are rewritten by every re-pin and are not compared. `wire` lines
(plan §1.6) are compared by their rules: a base key whose version is still
accepted (v >= this tree's protocolMin) is kept unchanged, and a key the base
does not have needs v == this tree's protocol and v above every base key's v.
A lock that the base does not have yet passes (PR5 creates it). A base ref that is not available
fails closed: CI checks out with fetch-depth 0 so that origin/main exists.
Run: python3 deploy/ci/check_lock_append_only.py [--base origin/main]
"""
import argparse
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
LOCK = 'prototypes/pocold/consensus-sources.lock'


def history(text):
    return [line.rstrip('\r') for line in text.splitlines() if line.startswith('history ')]


def wire(text):
    """{key: (v, sha)} of the `wire <sha> <key>` lines."""
    out = {}
    for line in text.splitlines():
        if line.startswith('wire '):
            parts = line.rstrip('\r').split(' ')
            m = re.fullmatch(r'v([1-9][0-9]*)/[a-z0-9-]+', parts[-1])
            if len(parts) != 3 or not m:
                raise ValueError(f'malformed wire line {line!r}')
            out[parts[2]] = (int(m.group(1)), parts[1])
    return out


def field(line, name):
    m = re.search(rf' {name}=([0-9]+)( |$)', line)
    return int(m.group(1)) if m else None


def wire_problem(base_text, head_text, last):
    """None when head's wire lines follow the §1.6 rules against base's, else the reason."""
    protocol, minimum = field(last, 'protocol'), field(last, 'protocolMin')
    if protocol is None or minimum is None:
        return 'the last history line records no protocol= and protocolMin='
    base, head = wire(base_text), wire(head_text)
    for key, (v, sha) in sorted(base.items()):
        if v >= minimum and head.get(key, (v, None))[1] != sha:
            return f'wire key {key} (v{v} >= protocolMin={minimum}) changed or was removed; only raising PROTOCOL_VERSION_MIN retires it'
    top = max((v for v, _ in base.values()), default=0)
    for key, (v, _) in sorted(head.items()):
        if key not in base and (v != protocol or v <= top):
            return f'new wire key {key} at v{v} needs v == protocol ({protocol}) and above every base key ({top})'
    return None


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
    try:
        return wire_problem(base_text, head_text, head[-1])
    except ValueError as e:
        return str(e)


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
