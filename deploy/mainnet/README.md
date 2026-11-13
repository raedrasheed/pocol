# deploy/mainnet

PoCol launches a brand-new network from a fresh genesis
([owner decisions, 2026-11-13](../../development/decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)):
new keys, a new chain ID, four founding miners, and nothing carried over from the old network. The old
network's coordinated-upgrade and earn-in cutover tooling has been removed from this directory.

The launch runbook for the new network (a closed rehearsal on the three US servers, then the public launch)
is written once its genesis parameters are fixed. Nothing here deploys anything.

What remains until the last consumers of the old identity are rewritten:

- `h15000/`: the old network's invitation and fork schedules. Node, extension and Python tests still read
  `h15000/network.pocol.json`; it is deleted once they run on generated fixtures.
- `network_identity.py` (and `test_network_identity.py`): switches every client's pinned chain ID, genesis
  hash and GenesisSpec in one reviewed step. It is to be replaced by a generic tool that takes the old and
  new invitations, with no built-in old identity.

## Local checks

```sh
python3 -B -m unittest discover -s deploy/mainnet -v
```
