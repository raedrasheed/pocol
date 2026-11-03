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
   head with CI, including the Windows wallet run. Then merge nothing else into `main` until step 3 is
   merged, so that the release which runs the new network (step 4) differs from the one that builds its
   genesis (step 2) only by the step-3 switch.

   Steps 0.2 and 2 need this release's `pocold`: the deployed `/opt/pocol-testnet/bin/pocold` is an
   older build without the `cutover` command, which it answers with a usage text and exit status 0.
   (Newer builds answer a command they lack with `error: unknown command` and exit status 2.)
   On each server, keep a checkout of the merged head that the `pocol-testnet` user can read (for
   example in `/var/tmp/pocol-checkout`) for the `deploy/mainnet` scripts, and extract that head's
   Linux package (its `source-commit.txt` names the commit) outside `/opt/pocol-testnet`. Never copy
   it into `/opt/pocol-testnet/bin`: the installed bundle then fails its own verification, and so
   does a rollback.
   ```sh
   mkdir /var/tmp/pocol-earnin-bin
   tar -xzf PoCol-Mainnet-<version>-linux-x86_64.tar.gz -C /var/tmp/pocol-earnin-bin
   cat /var/tmp/pocol-earnin-bin/source-commit.txt   # the merged head
   ```
2. **Operator rehearsal on a copy (A2)**, on one server, while the old network keeps running:
   ```sh
   sudo -u pocol-testnet cp -a /var/lib/pocol-testnet /var/tmp/pocol-rehearsal-data   # a copy; leave the original alone
   ```
   The copy must be taken while the service is stopped for a moment, or from the newest backup.
   A copy of a running store can be torn. Then, in the checkout, run the rehearsal as the service
   user, who owns the copy:
   ```sh
   python3 deploy/mainnet/new_network_config.py --old /opt/pocol-testnet/net/devnet.json \
       --genesis-time <a time 1 day ahead> --out /var/tmp/rehearsal-network.json
   sudo -u pocol-testnet env POCOLD_BIN_DIR=/var/tmp/pocol-earnin-bin \
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
   - wallets must be updated after the launch, as the new release notes describe: an existing wallet
     moves its old network files aside, keeps its key, and imports the new network, so its address
     stays the same (step 3);
   - after updating the extension, set Options > Trusted local fork schedule back to `[[1,"0"]]`
     (the H15000 value `[[1,"0"],[2,"15000"]]` belongs to the old network);
   - the extension's Viewer wallet derives its key from the network's chain ID and genesis, so after
     the update it shows a new address, and on-chain sites start with empty local storage. Move PCOL
     held at the Viewer wallet's address to a Studio, IDE or native wallet address (these keep their
     address) before block `S`;
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
sudo -u pocol-testnet /var/tmp/pocol-earnin-bin/pocold cutover --network /opt/pocol-testnet/net \
    --data /var/lib/pocol-testnet --height <S> --config /var/tmp/new-network.json \
    --out /var/tmp/pocol-earnin-genesis \
    --state-store --checkpoint-every 128 --delta 1 --phi 1 --t-exec 1 --delta-rel 8 --gamma 5
cd /var/tmp/pocol-earnin-genesis
sha256sum /var/tmp/new-network.json genesis.spec.hex alloc.json
python3 -c 'import json; d=json.load(open("devnet.json")); del d["timing"]; print(json.dumps(d, sort_keys=True))' | sha256sum
python3 -c 'import json; d=json.load(open("cutover-report.json")); del d["old"]["head"]; print(json.dumps(d, sort_keys=True))' | sha256sum
```

- The cutover opens the old data directory the way a node start does: it takes the lock and may
  rewrite the node's derived files. Run it only after the step-1 backup, as the service user, and
  check `--data`: a wrong path gets a new, empty store, and the cutover then refuses the height. It
  refuses to run while the old node holds the lock, refuses a height above the head, and never
  overwrites an output directory.
- All three servers must print the same five sums. `genesis.spec.hex` and `alloc.json` are the
  genesis: its hash and every balance. `devnet.json` also records the admission timing that server
  measured (`timing`), and `cutover-report.json` that server's own old head (`old.head`), so those two
  are compared without them. If any sum differs, stop and investigate. Do not pick one.
- `cutover-report.json` holds:
  - `S` and the old block hash;
  - the account count and the copied balances;
  - each credited bond and its total, and the miner bonds deducted;
  - the dropped pots by height (the unsettled pots of the last `D` blocks);
  - the excluded system addresses;
  - the total supply and the new genesis hash.

## 3. Switch the clients to the new identity

In a checkout of `main` that keeps the repository's LF line endings (on Windows, clone with
`git clone -c core.autocrlf=false`: the tool refuses hashed files whose line endings were converted, and
its message says how to fix an existing checkout), with the new genesis hash from `cutover-report.json`
(`newGenesis.genesisHash`, which starts with `0x`) and a copy of the public `genesis.spec.hex` from step
2's output:

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
and no copy of the old Mainnet identity. Nothing is written unless every file passes: the tool writes
every new file beside its original first, then replaces the originals, the node file last. If a
replacement fails, it lists the files it replaced and says whether to restore them (`git checkout --`
with those paths) or simply to run the same command again.
`--check` passes only on a tree that carries exactly this identity. If the launch later needs a new
genesis (for example a new launch time), run the same commands again with the new hash and spec: the
tool switches from the identity the tree carries. Then put the new genesis hash in the release notes
too. Documents about the old network's history stay as they are, and so do the website snapshots in
`deploy/websites` (step 5). `test_network_identity.py` passes on the switched tree and fails if any
other tracked file pins the old identity.

In the same PR, change what the tool cannot, because it ships with the packages:
- replace `deploy/standalone/RELEASE-UPGRADE.md`, the old network's 0.4.0 update notes, with the new
  network's notes. They must name the chain ID `261597237` and the new genesis hash, which users
  check, and may name the old network as history. They must also give existing wallets their way
  across, because Import and Upgrade refuse a home bound to the old genesis, and a new home would
  make a new, empty address:
  1. close the wallet, or stop the service as its user;
  2. put the new release's files and the new `network.pocol.json` in the application folder, replacing
     the old invitation there (the 0.4.0 notes put it beside `PoCol-Wallet.exe`; the standalone Linux
     service keeps it in `/opt/pocol-standalone`). The wallet starts its engine as soon as it opens,
     and an old invitation beside it binds the home to the old genesis again;
  3. in the wallet's home (on Windows `%LOCALAPPDATA%\PoCol`; the standalone Linux service uses
     `/var/lib/pocol-standalone`), make a folder such as `old-network` and move into it
     `network.pocol.json`, `network`, `state`, `wallet-activity.json` and, if present,
     `network-upgrade.pending.json` and `network.pocol.json.before-full-nonce`. Delete nothing;
  4. keep `identity.pocol-key` and `identity.address` where they are;
  5. open the wallet, or start the service: it imports the new invitation beside it (Settings →
     Import, or `pocol import --home <home> --network network.pocol.json`, does the same). The address
     stays the same.

  `deploy/standalone/test_upgrade.py` (`NewGenesisRejoin`) runs these steps, and shows that an old
  invitation left beside the application binds the home to the old genesis again;
- give the packages new version numbers: `deploy/standalone/package.py` and
  `deploy/standalone/test_release_package.py`; the Windows wallet's assembly versions in `Wallet.cs`
  and its label in `Wallet.xaml`; the files in `deploy/standalone/linux-desktop`; the extension's
  `package.json` and `package-lock.json`; Studio's `package.json`; the Android app's `versionName` (and
  `versionCode`, if a signed update must install over 5) in `prototypes/pocol-android/app/build.gradle`;
  and `.github/workflows/five-device-build.yml`, whose name, artifact names and APK check carry 0.4.0;
- rewrite the texts that are true only of the old network: the title and the same-chain upgrade
  text of `deploy/standalone/README.md`, and the titles and release paragraphs of the extension's
  `README-IDE.md`, `README-STUDIO.md` and `README-AR.md`, which say that the chain ID and genesis are
  unchanged. Name the new network in words there: each of these files must keep its listed number of
  copies of the identity (or the PR updates its counts in `FILES`);
- point Studio's extension download in `prototypes/pocol-studio/dist/app.js` and
  `prototypes/pocol-studio/QUICKSTART-AR.txt` at the new extension package;
- then run the first command again, which refreshes Studio's content version and the gateway
  installer after these edits, and `--check` last.

`test_network_identity.py` fails on the switched tree until all of this is done, and lists what is
left. Commit, open a PR, let CI build the clients, and merge.

## 4. Start the new network (approval: start the new servers)

1. On the first server, put its three genesis files in a network directory. Step 2 showed that all
   three servers built the same genesis; the `timing` in this server's `devnet.json` is informational,
   because every node measures its own at boot. The report is published, but it is not part of the
   bundle.
   ```sh
   mkdir /var/tmp/pocol-earnin-net
   cp /var/tmp/pocol-earnin-genesis/{genesis.spec.hex,alloc.json,devnet.json} /var/tmp/pocol-earnin-net/
   ```
2. Build the release from the merged head of step 3, after its CI passes, and build the server
   bundles from it with `deploy/public-testnet/prepare.py --network /var/tmp/pocol-earnin-net`, from
   the same inventory as the old release. `prepare.py` refuses binaries built from other sources than
   the checkout.
3. On each server:
   - move the old installation, data and service unit aside, **without deleting them**.
     `install.sh` refuses to install while any of the three exists. The script stops at the first
     failure and never moves onto an existing name:
     ```sh
     sudo bash -e <<'EOF'
     old=1599846715
     for path in /opt/pocol-testnet /var/lib/pocol-testnet; do
       test ! -e "$path-$old"; mv -T "$path" "$path-$old"
     done
     test ! -e "/etc/systemd/pocol-testnet.service-$old"
     mv -T /etc/systemd/system/pocol-testnet.service "/etc/systemd/pocol-testnet.service-$old"
     systemctl daemon-reload
     EOF
     ```
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
       `assets/index-CJbrNXr_.js`, which no page loads; also raise the extension version the IDE asks
       for ("0.4.0 or later") to the new one, because older extensions pin the old network;
     - the IDE guide `DEPLOY-README-AR.txt`, which lists the network settings.
  2. Remove the old network's H15000 upgrade instructions from pocol.net. They tell users to save the
     schedule `[[1,"0"],[2,"15000"]]`, to use the complete 0.4.0 package and to run Settings → Upgrade
     existing network, which refuses a new genesis. They are in:
     - `about-locales.js`, in all 45 locales: `appsUpgradeNotice`, `appsConnectionBody`,
       `appsWindowsUpdateBody`, `appsWindowsUpdateDownload`, `studioRelease` and `ideExtensionBody`.
       `app.js` writes these over the page text when it loads, so change them here, not only in the
       HTML;
     - the same texts in `index.html`, and the H15000 sections of `guide-content.js` and
       `pocol-tools.html`;
     - `README-AR.txt`, which sends users to `NETWORK-UPDATE-15000.txt` to join;
     - `RETAINED-NATIVE-DOWNLOADS.json`: list the new packages and the new invitation, without the
       H15000 `activationHeight`.

     Replace them with the new network's instructions from the release notes of step 3, including the
     schedule `[[1,"0"]]`. Do not quote the old schedule, the old chain ID or the H15000 name in them:
     the closing checks refuse all three.
  3. Make the reward panel accept earn-in records. In `reward.js` and `api/reward.php`, a beneficiary's
     `amountWei` may be `0` when its `bondAccruedWei` is above 0, and `sum(amountWei) +
     sum(bondAccruedWei)` must equal `projectedPaidWei`. Show the bond part as well. Before publishing,
     test both files with records from the reward gateway above, including an accruing one.
  4. Point every download link at the new packages, with their names, sizes and versions:
     `wallet-downloads.js`, `index.html`, `guide-content.js`, `pocol-tools.html`,
     `chrome-extension.html` and `smart-contracts.html` on pocol.net, and the extension link in the IDE
     bundle and in `DEPLOY-README-AR.txt`.
  5. Replace `pocol.net/downloads/network.pocol.json` with the new network's invitation. Build it
     outside the snapshot, with the new release's `pocol` and the old invitation's three seeds, one
     `--seed` each. `pocol network` writes the file with mode 600 and refuses an existing file, so
     copy it into place with mode 644:
     ```sh
     pocol network --from /var/tmp/pocol-earnin-net --seed <seed 1> --seed <seed 2> --seed <seed 3> \
         --out /var/tmp/network.pocol.json
     ```
  6. Give every changed file a new URL, so that no cache serves its old content:
     - pocol.net pages load files as `name?v=` with the first 12 hex digits of the file's SHA-256. Set
       the new value in every page that loads a changed file. The invitation is loaded by
       `index.html`, `guide-content.js` and `pocol-tools.html`, and a changed `guide-content.js` then
       needs its own new value in the pages that load it. `about-locales.js` is loaded by nine pages:
       `about`, `chrome-extension`, `collaborative-mining`, `how-pocol-works`, `index`,
       `onchain-websites`, `pocol-status`, `pocol-tools` and `smart-contracts`.
     - The Explorer and the IDE load their bundles by file name. Rename each changed bundle (for
       example to `index-<first 12 hex digits of its SHA-256>.js`) and update the `index.html` that
       loads it.
  7. Last, update the hash lists with the new hashes and sizes: each site's `SHA256SUMS.txt`, the
     `.sha256` files beside downloads, and `pocol.net/RETAINED-NATIVE-DOWNLOADS.json`. Cover the
     invitation and every published package.
  8. Afterwards, over the files you will publish (renamed bundles included, so `grep`, not
     `git grep`), the first command lists only each site's `NETWORK-UPDATE-15000.txt` history note, and
     the second lists nothing:
     ```sh
     grep -rnIE '1599846715|0x5f5bb93b|be137bfd|f4ed1373' deploy/websites
     grep -rnIE --include='*.html' --include='*.js' --include='*.json' --include='*.php' --include='*.txt' \
         --exclude='NETWORK-UPDATE-15000.txt' --exclude='SHA256SUMS.txt' \
         '0\.4\.[01]|15000(\\"|&quot;|")|block 15000|H15000|Upgrade existing network|NETWORK-UPDATE-15000' \
         deploy/websites
     ```
- **Studio.** Upload the merged head's Studio site (`prototypes/pocol-studio/dist`, the
  `PoCol-Studio-cPanel.zip` package) to the `studio.pocol.net` document root, as in
  [its README](../../prototypes/pocol-studio/README.md). Check that the page loads the new `app.js?v=`
  and reports chain `261597237` as connected.
- Publish `cutover-report.json` beside the release notes.

## 6. Fee faucet (approval: start the faucet)

1. On one server, create the faucet's user and its state directory, then a dedicated operator key
   owned by that user (`pocol-sign` refuses a key owned by another user or readable by others):
   ```sh
   sudo useradd --system --home-dir /var/lib/pocol-faucet --shell /usr/sbin/nologin pocol-faucet
   sudo install -d -m 0700 -o pocol-faucet -g pocol-faucet /var/lib/pocol-faucet
   sudo -u pocol-faucet /opt/pocol-testnet/bin/pocol-key generate --out /var/lib/pocol-faucet/operator.pocol-key
   ```
   Fund the address it prints with an ordinary transfer from an operator account. Each claim sends
   0.002 PCOL and pays its own fee, so 1 PCOL pays for a little under 490 claims.
2. Install `deploy/faucet` from the merged head as in its [README](../faucet/README.md):
   `pocol_faucet.py` in `/opt/pocol-faucet`, `config.example.json` as `/etc/pocol-faucet/config.json`
   (chain ID `261597237`, the key and ledger above, the signer `/opt/pocol-testnet/bin/pocol-sign`),
   and `pocol-faucet.service`. Put it behind the existing HTTPS proxy. Behind a proxy, also set
   `"trustProxy": true` and make that proxy location set `X-Forwarded-For`; otherwise every user
   shares the proxy's address and one daily allowance (`perClientDaily`). Set `"allowOrigin"` to the
   one website origin that calls it. Check `GET /status`, then one `POST /claim` through the proxy.

## Rollback

- **Before anyone transacts on the new network:** stop the new services. Move the new installation,
  state and unit aside under a dated name, never deleting them, then move the old ones back. The
  script stops at the first failure, before anything starts, and never moves onto an existing name,
  so it is safe to run again after a later launch:
  ```sh
  sudo bash -e <<'EOF'
  old=1599846715 new=261597237-$(date -u +%Y%m%dT%H%M%SZ)
  unit=/etc/systemd/system/pocol-testnet.service saved=/etc/systemd/pocol-testnet.service
  # The old installation must be waiting under its -1599846715 names before anything moves.
  test -e "/opt/pocol-testnet-$old"; test -e "/var/lib/pocol-testnet-$old"; test -e "$saved-$old"
  systemctl stop pocol-testnet.service
  for path in /opt/pocol-testnet /var/lib/pocol-testnet; do
    test ! -e "$path-$new"; mv -T "$path" "$path-$new"
    mv -T "$path-$old" "$path"
  done
  test ! -e "$saved-$new"; mv -T "$unit" "$saved-$new"
  mv -T "$saved-$old" "$unit"
  systemctl daemon-reload
  systemctl start pocol-testnet.service
  EOF
  ```
  The old chain continues from where it stopped.
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
