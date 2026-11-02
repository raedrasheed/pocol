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
   - after updating the extension, set Options > Trusted local fork schedule back to `[[1,"0"]]`
     (the H15000 value `[[1,"0"],[2,"15000"]]` belongs to the old network);
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

In the repository, with the new genesis hash from `cutover-report.json` (`newGenesis.genesisHash`,
which starts with `0x`) and a copy of the public `genesis.spec.hex` from step 2's output:

```sh
python3 deploy/mainnet/network_identity.py --genesis-hash <newGenesis.genesisHash> \
    --genesis-spec <copy of /var/tmp/pocol-earnin-genesis/genesis.spec.hex>
python3 deploy/mainnet/network_identity.py --genesis-hash <newGenesis.genesisHash> \
    --genesis-spec <copy of /var/tmp/pocol-earnin-genesis/genesis.spec.hex> --check
```

The tool refuses a GenesisSpec that is not canonical RLP, does not hash (keccak256) to the genesis
hash, or does not carry the new chain ID. It moves every chain ID, genesis hash and GenesisSpec pinned
in the repository's clients; the extension and the node pin the whole GenesisSpec, which proves the
other two. Chain IDs are matched in decimal, Rust digit groups, hex and JavaScript BigInt form. The
files are:
- the node display and its published-genesis test;
- the extension's pages, network file, tests and READMEs;
- Studio, its README, and the content version (`app.js?v=`) its page loads it with;
- the Studio gateway, its READMEs, and its installer `Install-PoCol-Studio-Gateway.sh`, rebuilt from
  the switched sources;
- the Windows wallet tests, the preflight, rewards, gateway, packaging and the repository README.

Each file must hold exactly its listed number of copies of the identity before and after the switch,
and no copy of the old Mainnet identity; nothing is written unless every file passes. `--check` passes
only on a tree that carries exactly this identity. If the launch later needs a new genesis (for example a new launch time), run the same
commands again with the new hash and spec: the tool switches from the identity the tree carries.
Documents about the old network's history stay as they are, and so do the website snapshots in
`deploy/websites` (step 5). `test_network_identity.py` passes on the switched tree and fails if any
other tracked file pins the old identity.

In the same PR, change what the tool cannot, because it ships with every package:
- replace `deploy/standalone/RELEASE-UPGRADE.md`, the old network's 0.4.0 update notes, with the new
  network's notes;
- give the packages new version numbers: `deploy/standalone/package.py` and
  `deploy/standalone/test_release_package.py`, the extension's `package.json` and
  `package-lock.json`, and the titles of `README-IDE.md` and `README-STUDIO.md`.

`test_network_identity.py` fails on the switched tree until both are done. Commit, open a PR, let CI
build the clients, and merge.

## 4. Start the new network (approval: start the new servers)

1. Put the three genesis files in a network directory. The report is published, but it is not part
   of the bundle.
   ```sh
   mkdir /var/tmp/pocol-earnin-net
   cp /var/tmp/pocol-earnin-genesis/{genesis.spec.hex,alloc.json,devnet.json} /var/tmp/pocol-earnin-net/
   ```
2. Build the release from the merged head of step 3, after its CI passes, and build the server
   bundles from it with `deploy/public-testnet/prepare.py --network /var/tmp/pocol-earnin-net`, from
   the same inventory as the old release. `prepare.py` refuses binaries built from other sources than
   the checkout.
3. On each server:
   - move the old installation and data aside, **without deleting them**: `/opt/pocol-testnet` to
     `/opt/pocol-testnet-1599846715`, and `/var/lib/pocol-testnet` to
     `/var/lib/pocol-testnet-1599846715`;
   - keep `/etc/pocol-testnet` (the same miner keys) and every website and gateway configuration
     file, including `api/config.php`; step 5 changes only their network values;
   - install the new bundle with its `install.sh`, as before.
4. Start the service at the launch time. Check:
   - `eth_chainId` is `0xf97a835`;
   - the genesis hash is the published one;
   - the head advances;
   - `pocol_getMiners` lists the three servers with `bond` `0x1158e460913d00000` (20 PCOL) and
     `bondState` 1;
   - a few accounts from the report hold the reported balances.

## 5. Gateway, Explorer, websites and wallets (approval: publish)

Everything below comes from the merged head of step 3.

- **Reward gateway.** Deploy `deploy/rewards`, the gateway that pocol.net and the Explorer use. The
  reward accounting reads CP 16, and an accruing record's payment first fills its bond.
  - The new network's fork schedule is `[[1,0]]`. Set the gateway's trusted schedule (its
    `--fork-schedule` file, for example `/etc/pocol-explorer-gateway/forks.json`) to `[[1,0]]`, or
    remove the option: version 1 alone is the default. With the H15000 schedule `[[1,0],[2,15000]]` it
    refuses every reward request.
  - The gateway also loads two host-only modules, `pocol_transactions.py` and `pocol_geoip.py`, and a
    transaction index (`POCOL_TX_DB`). Check the modules for a pinned chain ID or genesis hash, and
    move the old index aside so that it is rebuilt from the new genesis.
  - Then check that `pocol_getBlockReward` through the gateway returns a reward.
- **Studio gateway.** On the first server, install the Studio relay with the merged head's
  `deploy/studio/Install-PoCol-Studio-Gateway.sh`: first `sudo bash Install-PoCol-Studio-Gateway.sh
  --check`, then without `--check`, as in [its README](../studio/README.md). Check that
  `https://gateway.pocol.net/studio/health` reports chain `261597237` and the new genesis hash.
- **Explorer configuration.** `explorer.pocol.net/api/config.php` lives only on the host. Keep the
  file and change only two values: `chain_id` to the integer `261597237`, and `genesis_hash` to the new
  hash in lower case with `0x`. Do not print or copy the file. Check that
  `https://explorer.pocol.net/api/health.php` returns `PASS`, and that `api/transactions.php` and
  `api/miner-locations.php` return data.
- **Packages.** Build the Windows wallet, the extension, Studio and the standalone packages from the
  merged head, with the new version numbers from step 3, and publish them. Keep large files in GitHub
  Releases. The website steps below need their names, sizes and hashes.
- **Websites.** `deploy/websites` holds snapshots of the deployed pocol.net, ide.pocol.net and
  explorer.pocol.net. They are build output, and their checks refuse data from any other network, so
  never publish them unchanged. In this order:
  1. Update or rebuild the files that pin the identity:
     - pocol.net: `network.js`, `reward.js`, `api/network.php`, `api/reward.php`;
     - the Explorer bundle `assets/index-Cj8htKRO.js`;
     - the IDE bundle `assets/index-extension041-02af63944c2b.js`, and the superseded
       `assets/index-CJbrNXr_.js`, which no page loads;
     - the IDE guide `DEPLOY-README-AR.txt`, which lists the network settings.
  2. Make the reward panel accept earn-in records. In `reward.js` and `api/reward.php`, a beneficiary's
     `amountWei` may be `0` when its `bondAccruedWei` is above 0, and `sum(amountWei) +
     sum(bondAccruedWei)` must equal `projectedPaidWei`. Show the bond part as well. Before publishing,
     test both files with records from the reward gateway above, including an accruing one.
  3. Point every download link at the new packages, with their names, sizes and versions:
     `wallet-downloads.js`, `index.html`, `guide-content.js`, `pocol-tools.html` and
     `chrome-extension.html` on pocol.net, and the extension link in the IDE bundle and in
     `DEPLOY-README-AR.txt`.
  4. Replace `pocol.net/downloads/network.pocol.json` with the new network's invitation. Build it
     outside the snapshot, with the new release's `pocol` and the old invitation's three seeds, one
     `--seed` each. `pocol network` writes the file with mode 600 and refuses an existing file, so
     copy it into place with mode 644:
     ```sh
     pocol network --from /var/tmp/pocol-earnin-net --seed <seed 1> --seed <seed 2> --seed <seed 3> \
         --out /var/tmp/network.pocol.json
     ```
  5. Give every changed file a new URL, so that no cache serves its old content:
     - pocol.net pages load files as `name?v=` with the first 12 hex digits of the file's SHA-256. Set
       the new value in every page that loads a changed file. The invitation is loaded by
       `index.html`, `guide-content.js` and `pocol-tools.html`, and a changed `guide-content.js` then
       needs its own new value in the pages that load it.
     - The Explorer and the IDE load their bundles by file name. Rename each changed bundle (for
       example to `index-<first 12 hex digits of its SHA-256>.js`) and update the `index.html` that
       loads it.
  6. Last, update the hash lists with the new hashes and sizes: each site's `SHA256SUMS.txt`, the
     `.sha256` files beside downloads, and `pocol.net/RETAINED-NATIVE-DOWNLOADS.json`. Cover the
     invitation and every published package.
  7. Afterwards the first command lists only each site's `NETWORK-UPDATE-15000.txt` history note, and
     the second lists nothing:
     ```sh
     git grep -nE '1599846715|0x5f5bb93b|be137bfd|f4ed1373' -- deploy/websites
     git grep -nE 'PoCol-Mainnet-0\.4\.0|IDE-Viewer-0\.4\.[01]' -- 'deploy/websites/*.html' 'deploy/websites/*.js'
     ```
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
- The clients from step 3 work only on the new network. To serve the old network again, publish
  clients built from a commit before step 3, or from a revert of it.
- **If step 5 has started:** redeploy the reward gateway and the Studio relay from a commit before
  step 3, and set the gateway's trusted schedule back to `[[1,0],[2,15000]]`. Set `chain_id` and
  `genesis_hash` in the Explorer's `config.php` back to the old values, and republish the website
  snapshots and the invitation from before step 5.

## What this does not cover

- Old Android installations cannot follow the new network (A3). Android stays deferred.
- `Initialize-PoColNetwork.ps1` must not be used to join either network.
