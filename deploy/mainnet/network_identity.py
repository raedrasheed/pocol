#!/usr/bin/env python3
"""Moves every pinned PoCol Mainnet identity (chain ID and genesis hash) to the new network's.

The earn-in network's genesis hash exists only after `pocold cutover` has run at the agreed final height S
(development/decisions/EARN-IN-SPEC.md section 7), so the clients are switched in one reviewed step:

    python deploy/mainnet/network_identity.py --genesis-hash 0x<64 hex from cutover-report.json>
    python deploy/mainnet/network_identity.py --genesis-hash 0x... --check     # verify only

The chain ID defaults to the approved new value. Only the code and test files listed below change;
documents that describe the old network's history are left as they are. Nothing is deployed.
"""
import argparse, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OLD_CHAIN_ID = 1599846715
OLD_GENESIS = 'be137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
# sha256(b'PoCol earn-in network 2026')[:4] mod (2^31 - 2^20) + 2^20; not listed on chainid.network.
NEW_CHAIN_ID = 261597237
FILES = [
    'prototypes/pocold/src/network_display.rs',
    'prototypes/pocol-extension/src/lib/studio-network.js',
    'prototypes/pocol-extension/tests/studio_test.mjs',
    'prototypes/pocol-extension/tests/publisher_test.mjs',
    'prototypes/pocol-extension/tests/contract_ui_test.mjs',
    'prototypes/pocol-studio/dist/app.js',
    'prototypes/pocol-windows/WalletUiTests.cs',
    'deploy/mainnet/preflight.py',
    'deploy/rewards/pocol_rewards.py',
    'deploy/rewards/gateway.py',
    'deploy/rewards/test_pocol_rewards.py',
    'deploy/standalone/package.py',
    'deploy/studio/install_studio_gateway.py',
    'deploy/studio/pocol_studio_gateway.py',
]

def tokens(chain_id, genesis):
    """Every spelling the files use: decimal, Rust digit groups, 0x hex, and the hash with or without 0x."""
    return [str(chain_id), f'{chain_id:_}', hex(chain_id), genesis]

def token(t):
    """A whole token, also as a JavaScript BigInt literal (`1599846715n`)."""
    return rf'(?<![0-9A-Za-z_]){re.escape(t)}(?=n?(?![0-9A-Za-z_]))'

def rewrite(text, old, new):
    out = text
    # The genesis hash first (case-insensitive, keeps an existing 0x prefix), then the chain ID spellings.
    out = re.sub(old[3], new[3], out, flags=re.IGNORECASE)
    for o, n in zip(old[:3], new[:3]):
        out = re.sub(token(o), n, out)
    return out

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--genesis-hash', required=True, help='new genesis hash, 0x + 64 hex digits')
    p.add_argument('--chain-id', type=int, default=NEW_CHAIN_ID)
    p.add_argument('--check', action='store_true', help='verify that every file already carries the new identity')
    p.add_argument('--root', type=Path, default=ROOT)
    a = p.parse_args(argv)
    if not re.fullmatch(r'0x[0-9a-fA-F]{64}', a.genesis_hash):
        p.error('--genesis-hash must be 0x followed by 64 hex digits')
    genesis = a.genesis_hash[2:].lower()
    if a.chain_id in (0, OLD_CHAIN_ID, 777001) or not 0 < a.chain_id < 2**53 or genesis == OLD_GENESIS:
        p.error('the new identity must differ from the old network and be a valid chain ID')
    old, new = tokens(OLD_CHAIN_ID, OLD_GENESIS), tokens(a.chain_id, genesis)
    problems, changed = [], []
    for rel in FILES:
        path = a.root / rel
        text = path.read_text(encoding='utf-8')
        after = rewrite(text, old, new)
        left = [t for t in old if re.search(token(t), after, re.IGNORECASE)]
        if left:
            problems.append(f'{rel}: old identity still present ({", ".join(left)})')
        if not (str(a.chain_id) in after or f'{a.chain_id:_}' in after or genesis in after.lower()):
            problems.append(f'{rel}: no identity found to update')
        if after != text:
            changed.append(rel)
            if not a.check:
                path.write_text(after, encoding='utf-8')
    if a.check and changed:
        problems += [f'{rel}: still carries the old identity' for rel in changed]
    for line in problems:
        print('FAIL', line, file=sys.stderr)
    if problems:
        return 2
    print(('verified' if a.check else 'updated'), len(FILES), 'files;', 'changed:', len(changed),
          f'chainId={a.chain_id} genesisHash=0x{genesis}')
    return 0

if __name__ == '__main__':
    sys.exit(main())
