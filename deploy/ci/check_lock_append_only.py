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

One exception, used once: against the retired network's last lock (RETIRED_BASE_LOCK_SHA256), the
tree may replace the whole history with a single kind=base line for the fresh v3 genesis (removal
plan §3), provided nothing but the history lines and the shas of existing wire keys change.
Run: python3 deploy/ci/check_lock_append_only.py [--base origin/main]
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
LOCK = 'prototypes/pocold/consensus-sources.lock'

# The one-time reset of the consensus lock to the fresh v3 network (removal plan §3): the sha256 of the
# raw bytes of the retired network's last lock (41 history lines, n=41 ref=Phase-9). Against exactly that
# base, and only there, reset_problem() decides instead of the append-only rule. It is spent once the
# reset merges: main's lock then has other bytes, so the exemption can never fire again. Phase 10
# deletes this constant, reset_problem() and their tests (deploy/ci only; no lock line).
RETIRED_BASE_LOCK_SHA256 = 'eecb9e772762d21e55e2d9c927589884fe87d46a056ac8c65967d07daefc6ae7'
# The only wire keys whose pinned sha the reset may move: their canonical samples carried the retired
# network's chain id and heights and now carry neutral values (net.rs canonical()). Every other key keeps
# its sha.
RESET_WIRE_KEYS = frozenset({'v2/hello', 'v2/get-range'})


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


def token(line, name):
    """The raw value of `name=` among a history line's fields (before ref= and note=), or None."""
    fields = line.split(' ref=', 1)[0]
    m = re.search(rf'(?:^| ){name}=(\S+)', fields)
    return m.group(1) if m else None


def files(text):
    """{path: sha} of the `file <sha> <path>` lines."""
    out = {}
    for line in text.splitlines():
        if line.startswith('file '):
            parts = line.rstrip('\r').split(' ', 2)
            if len(parts) != 3:
                raise ValueError(f'malformed file line {line!r}')
            out[parts[2]] = parts[1]
    return out


def reset_problem(base_text, head_text):
    """None when head is the one-time reset of the retired lock base_text (removal plan §3), else the reason.

    Strict: the head is one `history n=1 kind=base` line without a gate, at spec 2, that records the
    base's last line's rules, maxFork, spec, protocol, protocolMin, sources, vectors and deps; the file
    lines are the base's; the wire keys are the base's, and only the RESET_WIRE_KEYS samples may change sha. So the reset changes
    no consensus source, constant, vector or dependency, and only the history and pinned v2 samples move.
    """
    base, head = history(base_text), history(head_text)
    if len(head) != 1:
        return f'the reset keeps exactly one history line, this tree has {len(head)}'
    line, last = head[0], base[-1]
    if not line.startswith('history n=1 kind=base '):
        return f'the reset line must start with "history n=1 kind=base ":\n  here: {line}'
    if token(line, 'gate') is not None:
        return 'the reset line must carry no gate'
    if token(line, 'spec') != '2':
        return f'the reset line records spec={token(line, "spec")}, the fresh v3 network is spec 2'
    for name in ('rules', 'maxFork', 'spec', 'protocol', 'protocolMin', 'sources', 'vectors', 'deps'):
        if token(line, name) is None or token(line, name) != token(last, name):
            return f'the reset line records {name}={token(line, name)}, the base\'s last line {name}={token(last, name)}; a reset changes no consensus source, constant, vector or dependency'
    if files(head_text) != files(base_text):
        return 'the file lines differ from the base; a reset changes no consensus source'
    head_wire, base_wire = wire(head_text), wire(base_text)
    if set(head_wire) != set(base_wire):
        return 'the wire keys differ from the base; a reset may change only the pinned shas of existing keys'
    for key in sorted(base_wire):
        if key not in RESET_WIRE_KEYS and head_wire[key] != base_wire[key]:
            return f'wire key {key} changed; the reset may move only the samples of {", ".join(sorted(RESET_WIRE_KEYS))}'
    return None


def is_retired(base_bytes):
    return base_bytes is not None and hashlib.sha256(base_bytes).hexdigest() == RETIRED_BASE_LOCK_SHA256


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


def resets(base_text, head_text, base_bytes):
    """True when the one-time reset exemption decides: the base is the retired lock and head drops its history."""
    if base_text is None or head_text is None or not is_retired(base_bytes):
        return False
    base, head = history(base_text), history(head_text)
    return bool(base) and head[:len(base)] != base


def check(base_text, head_text, base_bytes=None):
    """None when head keeps base's history as a prefix (or is the one-time reset of the retired lock), else the reason.

    base_bytes are the raw bytes of the base lock as git stored them; they arm the reset exemption.
    """
    if base_text is None:
        return None
    if head_text is None:
        return 'the base has a consensus lock and this tree removed it'
    base, head = history(base_text), history(head_text)
    if not base:
        return 'the base lock has no history line'
    if resets(base_text, head_text, base_bytes):
        try:
            why = reset_problem(base_text, head_text)
        except ValueError as e:
            why = str(e)
        return None if why is None else f'not the one-time reset of the retired lock: {why}'
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


def git_bytes(*args):
    """stdout as the raw bytes git wrote (no locale decoding or newline translation)."""
    return subprocess.run(['git', '-C', str(ROOT), *args], capture_output=True).stdout


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--base', default='origin/main')
    args = parser.parse_args(argv)
    if git('cat-file', '-e', f'{args.base}^{{commit}}').returncode:
        print(f'base ref {args.base} is not available; fetch it (CI: actions/checkout with fetch-depth: 0)', file=sys.stderr)
        return 2
    exists = git('cat-file', '-e', f'{args.base}:{LOCK}').returncode == 0
    base_bytes = git_bytes('show', f'{args.base}:{LOCK}') if exists else None
    base_text = base_bytes.decode('utf-8') if exists else None
    path = ROOT / LOCK
    head_text = path.read_text(encoding='utf-8') if path.exists() else None
    why = check(base_text, head_text, base_bytes)
    if why:
        print(f'{LOCK}: {why}', file=sys.stderr)
        return 1
    if resets(base_text, head_text, base_bytes):
        print(f"{LOCK}: accepted against {args.base} as the one-time reset of the retired network's lock (RETIRED_BASE_LOCK_SHA256)")
        return 0
    print(f'{LOCK}: history is append-only against {args.base}' + ('' if exists else ' (the lock is new)'))
    return 0


if __name__ == '__main__':
    sys.exit(main())
