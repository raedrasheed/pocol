# Earn-in network: cutover runbook (owner)

Arabic: [EARN-IN-CUTOVER-RUNBOOK-AR.md](EARN-IN-CUTOVER-RUNBOOK-AR.md). Design and rules:
[proposal](../../development/decisions/EARN-IN-MEMBERSHIP-PROPOSAL.md),
[specification](../../development/decisions/EARN-IN-SPEC.md) section 7.

This moves PoCol Mainnet (chain ID `1599846715`, genesis `0xbe137bfd…09ca`) to a new network built
from a new genesis. The new network uses the earn-in rules, chain ID `261597237`, and the three
current servers as genesis miners. Balances, nonces, contracts and their storage move over unchanged.
Each old HELD bond (100 PCOL) returns to its reward address. Each server then funds a 20 PCOL bond
from its own moved balance.

Nothing in this repository deploys, stops a service, or touches a key. Every step below that changes
a server is the owner's or the operator's action. A4 asks for one approval for each of:
- stopping the live network;
- starting the new servers;
- publishing the wallet, the extension and the websites;
- starting the faucet.

Never print, copy off the host, or publish the contents of `/etc/pocol-testnet`, the miner key
files, `/var/backups/pocol-pre-0.4.0.*` or `api/config.php`.

## 0. Before the day

1. Merge the pull requests in order (A1): #16, then #17, then #18. Build the release from the merged
   head with CI, including the Windows wallet run.
2. **Operator rehearsal on a copy (A2)**, on one server, while the old network keeps running:
   ```sh
   sudo -u pocol-testnet cp -a /var/lib/pocol-testnet /var/tmp/pocol-rehearsal-data   # a copy; leave the original alone
   ```
   The copy must be taken while the service is stopped for a moment, or from the newest backup.
   A copy of a running store can be torn. Then:
   ```sh
   python3 deploy/mainnet/new_network_config.py --old /opt/pocol-testnet/net/devnet.json \
       --genesis-time <a time 1 day ahead> --out /var/tmp/rehearsal-network.json
   python3 deploy/mainnet/rehearse_cutover.py --copy --network /opt/pocol-testnet/net \
       --data /var/tmp/pocol-rehearsal-data --height <its head - 60> \
       --config /var/tmp/rehearsal-network.json --work /var/tmp/pocol-rehearsal -- \
       --state-store --checkpoint-every 128 --delta 1 --phi 1 --t-exec 1 --delta-rel 8 --gamma 5
   ```
   It must print `"result": "PASS"` and `"deterministic": true`. Read the summary:
   - accounts copied;
   - bonds credited (one per HELD record);
   - miner bonds deducted (`60000000000000000000`);
   - pots dropped;
   - total supply.

   Delete `/var/tmp/pocol-rehearsal*` afterwards.
3. Choose the final height `S` and the launch time. Publish both, with this text:
   - transactions included after block `S` do not move to the new network;
   - wallets must be updated after the launch;
   - old Android installations cannot follow the new network (A3).

## 1. Freeze (approval: stop the live network)

1. Wait until every server's head is at least `S + 60`. Check that all three servers report the same
   hash for block `S`:
   ```sh
   curl -s -H 'Content-Type: application/json' --data \
     '{"jsonrpc":"2.0","id":1,"method":"eth_getBlockByNumber","params":["<S in hex>",false]}' http://127.0.0.1:8545
   ```
2. Stop the old service on all three servers: `sudo systemctl stop pocol-testnet.service`.
3. Take the normal backup (`node_operator.py backup`). Keep the old data directory. **Never delete
   it**: it is the only rollback.

## 2. Build the new genesis (on each server, independently)

```sh
python3 deploy/mainnet/new_network_config.py --old /opt/pocol-testnet/net/devnet.json \
    --genesis-time <launch time> --out /var/tmp/new-network.json
sudo -u pocol-testnet /opt/pocol-testnet/bin/pocold cutover --network /opt/pocol-testnet/net \
    --data /var/lib/pocol-testnet --height <S> --config /var/tmp/new-network.json \
    --out /var/tmp/pocol-earnin-genesis \
    --state-store --checkpoint-every 128 --delta 1 --phi 1 --t-exec 1 --delta-rel 8 --gamma 5
sha256sum /var/tmp/pocol-earnin-genesis/*
```

- The cutover only reads the old data directory. It refuses to run while the old node holds the
  lock, refuses a height above the head, and never overwrites an output directory.
- All three servers must produce the same four SHA-256 sums, and the same `new-network.json`.
  If they differ, stop and investigate. Do not pick one.
- `cutover-report.json` holds:
  - `S` and the old block hash;
  - the account count and the copied balances;
  - each credited bond and its total, and the miner bonds deducted;
  - the dropped pots by height (the unsettled pots of the last `D` blocks);
  - the excluded system addresses;
  - the total supply and the new genesis hash.

## 3. Switch the clients to the new identity

In the repository, with the new genesis hash from `cutover-report.json`:

```sh
python3 deploy/mainnet/network_identity.py --genesis-hash 0x<new genesis hash>
python3 deploy/mainnet/network_identity.py --genesis-hash 0x<new genesis hash> --check
```

This rewrites every pinned chain ID and genesis hash, in decimal, Rust digit groups, hex and
JavaScript BigInt form. The files covered are the node display, the extension and its tests, Studio,
the Windows wallet tests, the preflight, rewards, gateway, packaging and Studio gateway. Documents
about the old network's history stay as they are. Commit, open a PR, let CI build the clients, and
merge.

## 4. Start the new network (approval: start the new servers)

1. Put the three genesis files in a network directory. The report is published, but it is not part
   of the bundle.
   ```sh
   mkdir /var/tmp/pocol-earnin-net
   cp /var/tmp/pocol-earnin-genesis/{genesis.spec.hex,alloc.json,devnet.json} /var/tmp/pocol-earnin-net/
   ```
2. Build the server bundles with `deploy/public-testnet/prepare.py --network /var/tmp/pocol-earnin-net`,
   from the same inventory as the old release.
3. On each server:
   - move the old installation and data aside, **without deleting them**: `/opt/pocol-testnet` to
     `/opt/pocol-testnet-1599846715`, and `/var/lib/pocol-testnet` to
     `/var/lib/pocol-testnet-1599846715`;
   - keep `/etc/pocol-testnet` (the same miner keys) and every website and gateway configuration
     file, including `api/config.php`;
   - install the new bundle with its `install.sh`, as before.
4. Start the service at the launch time. Check:
   - `eth_chainId` is `0xf97a835`;
   - the genesis hash is the published one;
   - the head advances;
   - `pocol_getMiners` lists the three servers with `bond` `0x1158e460913d00000` (20 PCOL) and
     `bondState` 1;
   - a few accounts from the report hold the reported balances.

## 5. Gateway, Explorer, websites and wallets (approval: publish)

- Deploy the gateway and the Explorer with the identity commit from step 3. The reward accounting
  reads CP 16, and an accruing record's payment first fills its bond.
- Publish the Windows wallet, the extension, Studio and the standalone packages built from the merged
  head. Keep large files in GitHub Releases.
- Publish `cutover-report.json` beside the release notes.

## 6. Fee faucet (approval: start the faucet)

1. Create a dedicated operator key on one server:
   `pocol-key generate --out /var/lib/pocol-faucet/operator.pocol-key`.
   Fund it with an ordinary transfer from an operator account, for example 1 PCOL, which pays for
   500 claims.
2. Install `deploy/faucet` as in its [README](../faucet/README.md), with `config.example.json`
   (chain ID `261597237`) and `pocol-faucet.service`. Put it behind the existing HTTPS proxy.

## Rollback

- **Before anyone transacts on the new network:** stop the new services. Move the old directories
  back (`/opt/pocol-testnet-1599846715`, `/var/lib/pocol-testnet-1599846715`) and start
  `pocol-testnet.service`. The old chain continues from where it stopped.
- **After that:** do not roll back. Fix forward on the new network.

## What this does not cover

- Old Android installations cannot follow the new network (A3). Android stays deferred.
- `Initialize-PoColNetwork.ps1` must not be used to join either network.
