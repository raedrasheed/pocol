# deploy/mainnet: launching the new PoCol network

PoCol launches a brand-new network from a fresh genesis
([owner decisions, 2026-11-13](../../development/decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)):
new keys, a new chain ID, four founding miners, and nothing carried over from the retired network. Nothing in
this directory deploys anything, and no step below runs on a server without the owner's approval.

## Launch runbook (stub)

This runbook is completed once the genesis parameters are fixed. The steps, in order:

1. **Release.** Build the release that will run the network with `POCOL_RELEASE=1` from a pinned tree, and
   publish its binaries and checksums.
2. **Prepare the servers.** The three US servers start from a clean install (`deploy/standalone/install-linux.sh`
   refuses a host that already has a PoCol install). The owner's computer runs the fourth founding miner; the
   phone is a wallet.
3. **Fresh keys.** Each founding miner generates its key on its own device with the independent-key flow
   (`pocol-key`); only the public miner ID and reward address leave the device.
4. **Genesis.** Write the public genesis config (chain ID, genesis time, the four founders, their genesis
   allocations and the consensus parameters the owner approved) and run
   `pocold init-testnet --config PUBLIC.json --out NEW_DIR` once. Record the printed chain ID and genesis hash.
5. **Invitation.** Pack the genesis files with the three seed addresses into the network invitation
   (`pocol network`), and check its fingerprint on every founding device.
6. **Closed rehearsal.** Run the network on the three US servers only, then wipe and repeat from step 4 for
   the public launch if anything changed.
7. **Pin the identity in the clients.** Switch every client from the identity the tree pins today to the new
   one with `set_network_identity.py` (below), in one reviewed PR, and release the clients.
8. **Public launch.** Publish the invitation on pocol.net and open the seeds.

## Tools

- `set_network_identity.py --from OLD.json --to NEW.json [--check]`: switches every client's pinned chain ID,
  genesis hash and GenesisSpec (`FILES` lists them) from one network's invitation to another's, with the
  derived content versions and the Studio gateway installer. It names no network itself: both identities come
  from the invitations, proven by their GenesisSpec. The invitation of the identity the tree pins today, which
  is no longer in the tree, is taken from git history:

  ```sh
  git show <commit>:deploy/mainnet/h15000/network.pocol.json > /tmp/OLD.json
  python3 deploy/mainnet/set_network_identity.py --from /tmp/OLD.json --to NEW_DIR/network.pocol.json
  python3 deploy/mainnet/set_network_identity.py --from /tmp/OLD.json --to NEW_DIR/network.pocol.json --check
  ```

  `<commit>` is any commit before that file was deleted (`git log --diff-filter=D -- deploy/mainnet/h15000`
  names the deleting commit; use its parent).

## Local checks

```sh
python3 -B -m unittest discover -s deploy/mainnet -v
```
