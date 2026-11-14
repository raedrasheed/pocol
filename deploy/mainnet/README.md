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
  names the deleting commit; use its parent). The tool prints the identity it switches from and to before it
  writes anything.
- `retired-networks.json`: the identities the clients were switched away from, append-only. Every switch adds
  its `--from` network, and the tool refuses a listed network as `--to` (swapped `--from`/`--to` included):
  a rollback is a `git revert` of the switch commit, not a reversed run. `--allow-revert` switches back to a
  retired network on purpose. Its first entry is the network retired on 2026-11-13, which the clients still pin
  until the new genesis exists.

The tests (`test_set_network_identity.py`) also guard the tree: every tracked file that pins the current
identity is in `FILES` or kept on purpose; once the tree pins a network that is not retired, no tracked file
outside `development/` and `deploy/websites/` may hold a retired identity (`FILES` included); and the shipped
release texts that are true only on the retired network (`STEP3`: the 0.4.x versions and "chain ID and genesis
unchanged" notes) must be gone by then, so the version bump (removal plan Phase 7) lands before the switch.

## A spec-2 (v3) genesis

Since phase 4 of the fresh network the v3 rules are the release rules (consensus rules 2):
release nodes and clients decode GenesisSpec version 2 (CP schema v2) by default, and
version 1 still decodes. `set_network_identity.py` switches the clients to a spec-2
genesis because the tree's node decodes it (`SPEC_VERSION` 2 in `genesis_spec.rs`), and
only after the whole release decoder
(`deploy/public-testnet/pocol_identity.decode_genesis_spec` at `SPEC_VERSION` 2, checked
against the Rust verdicts of the decoder corpus: framing, structure, counts, canonical
integers, widths and bounds of every field of the final CP schema v2 of 59 fields, roots,
M_0List order and reserved ids) accepts it and its `sysCodeHash` names the v3 program
set. A refused `--to` writes nothing. Spec-1 switches are unchanged.

## The extension's built-in profile placeholder (decision D1)

Until the new genesis exists, the extension's built-in profile (`prototypes/pocol-extension/src/lib/studio-network.js`)
pins no retired network: it holds a placeholder, the spec-2 (v3) devnet GenesisSpec of the decoder corpus
(`tests/data/decoder-corpus-v3.json`, item `v3/base`) under a chain ID of no network, whose invitation is
`placeholder-network.pocol.json`. The file carries the `NETWORK_PLACEHOLDER` mark, its `FILES` counts are then counts
of the placeholder, and `prototypes/pocol-extension/scripts/package-release.py` refuses a build that carries the mark,
so no release ships it (CI checks the refusal instead of packaging).

```sh
python3 deploy/mainnet/set_network_identity.py --placeholder deploy/mainnet/placeholder-network.pocol.json [--check]
```

`--placeholder` writes a spec-2 placeholder (never a retired network or the tree's own) into `PLACEHOLDER_FILES` only.
The identity switch (`--from`/`--to`) replaces the placeholder there with the `--to` identity and removes the mark, so
the switch stays one run. The extension pages (`publisher.html`, `contract.html`) and READMEs keep the tree's identity
until that switch.

## Local checks

```sh
python3 -B -m unittest discover -s deploy/mainnet -v
```
