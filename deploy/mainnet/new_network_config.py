#!/usr/bin/env python3
"""Writes the earn-in network's genesis config from the old network's public devnet.json (N2, N3).

The new network keeps the old genesis miners (the three server identities, in the old order, each with
its own reward address) and the old per-block registration limit; it takes the approved new chain ID,
`continuous` and `earnin`, and the launch time. Balances come from the cutover, so `accounts` is empty.

    python3 deploy/mainnet/new_network_config.py --old /opt/pocol-testnet/net/devnet.json \\
        --genesis-time 1795000000 --out new-network.json

Reads public files only; writes one new file.
"""
import argparse, json, re, sys, time
from pathlib import Path

NEW_CHAIN_ID = 261597237
OLD_CHAIN_ID = 1599846715
ADDRESS = re.compile(r'^0x[0-9a-fA-F]{40}$')

def build(old, genesis_time, chain_id=NEW_CHAIN_ID):
    miners = old.get('miners')
    if not isinstance(miners, list) or len(miners) != 3:
        raise ValueError('the old devnet.json must list the three genesis miners')
    for m in miners:
        if not (isinstance(m, dict) and ADDRESS.match(str(m.get('id', ''))) and ADDRESS.match(str(m.get('reward', '')))):
            raise ValueError('every miner needs an id and a reward address')
    if chain_id in (0, OLD_CHAIN_ID, old.get('chainId')):
        raise ValueError('the new chain ID must differ from the old network')
    per_block = old.get('registrationPerBlock')
    if not isinstance(per_block, int) or not 1 <= per_block <= 4:
        raise ValueError('the old registrationPerBlock is missing or out of range')
    return {'continuous': True, 'earnin': True, 'chainId': chain_id, 'genesisTime': int(genesis_time),
            'registrationPerBlock': per_block, 'miners': [{'id': m['id'], 'reward': m['reward']} for m in miners], 'accounts': []}

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--old', required=True, type=Path, help="the old network's devnet.json")
    p.add_argument('--genesis-time', required=True, type=int, help='launch time, Unix seconds (in the future)')
    p.add_argument('--chain-id', type=int, default=NEW_CHAIN_ID)
    p.add_argument('--out', required=True, type=Path)
    a = p.parse_args(argv)
    if a.out.exists(): p.error('--out must be a new file')
    if a.genesis_time <= int(time.time()): p.error('--genesis-time must be in the future')
    try:
        cfg = build(json.loads(a.old.read_text(encoding='utf-8')), a.genesis_time, a.chain_id)
    except ValueError as e:
        p.error(str(e))
    with open(a.out, 'x', encoding='utf-8') as f: json.dump(cfg, f, indent=1)
    print(json.dumps(cfg))
    return 0

if __name__ == '__main__':
    sys.exit(main())
