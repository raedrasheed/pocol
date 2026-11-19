# Key recovery runbook (gate K1)

How to back up and restore the keys of the fresh network's founders and of the fee faucet, and what to do when a key
is lost. It belongs to gate K1 in [PLAN.md](PLAN.md), which stays open: the runbook is drilled in the rehearsal
([REHEARSAL.md](REHEARSAL.md), section 4.3), and an encrypted keystore or HSM remains. The commands are the ones the
tools implement: `pocol-key` (`prototypes/pocold/src/bin/pocol-key.rs`), `pocol-sign` (`pocol-sign.rs`), `pocol`
(`pocol.rs`: `identity`, `export-key`, `inspect-key-backup`, `import-key`) and the server layout of
`deploy/standalone/install-linux.sh`. The key formats and checks are in [KEYS.md](KEYS.md).

Rules for every key:
- A passphrase is read from stdin, never from an argument: on a terminal it is typed with echo off.
- Only the encrypted backup (98 bytes: PBKDF2-HMAC-SHA256 with 600,000 iterations, AES-256-GCM) leaves the device that
  made the key. A plaintext key file is never copied between hosts, and no secret goes into chat, Git or an uploaded
  artifact.
- Keep each backup apart from its passphrase. A backup is as strong as its passphrase.
- Never run two nodes with one miner key: two devices mining with one key can lose the bond.
- No command here deletes a key file or writes over one. `pocol-key generate`, `export` and `import` and `pocol
  export-key` refuse an existing path. `pocol import-key` moves the key file it replaces aside as
  `identity.pocol-key.replaced-<address>`. The only file written over is the public copy of the address,
  `/opt/pocol-standalone/identity.address`, where a step below says so.

| Key | Where it lives | What it controls | Backup |
|---|---|---|---|
| Server miner key (founders 1-3) | `/var/lib/pocol-standalone/identity.pocol-key`, owned by `pocol-standalone`, made by `install-linux.sh` (`pocol identity`) | the founder's membership and its 20 PCOL bond; the key holds no balance | encrypted backup, kept offline (section 1) |
| PC miner key (founder 4) | the Windows wallet home, `%LOCALAPPDATA%\PoCol`, DPAPI-protected | membership, bond, and the PC's own allocation and rewards (its id is its reward address) | Settings → Key backup, or `pocol.exe export-key --out FILE` (section 2) |
| Offline reward keys (founders 1-3) | the owner's offline device | the 999,980 PCOL allocation of each server founder, its rewards and any bond refund | an encrypted backup and paper copies, held offline by the owner (section 3) |
| Faucet operator key | `/var/lib/pocol-faucet/operator.pocol-key`, owned by `pocol-faucet` | the faucet's float only, at most 10 PCOL on mainnet | none needed: replace it (section 5) |

Chain ids for `pocol-sign`: 1656550731 on the rehearsal network, 2656089850 on mainnet. With `--address` it only
prints the key's address and signs nothing.

## 1. Server miner key: back up

On each server, as the admin account. The node may keep running: `pocol-key` reads the key file and takes no home
lock. The commands run as `pocol-standalone`, because a key file must be owned by the user that reads it.

```sh
sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol-key export \
  --key-file /var/lib/pocol-standalone/identity.pocol-key --out /var/lib/pocol-standalone/miner.pocol-backup
sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol-sign --chain-id CHAIN_ID --address \
  --backup /var/lib/pocol-standalone/miner.pocol-backup
cat /opt/pocol-standalone/identity.address
```

`export` asks for the passphrase twice and refuses one shorter than 12 characters or one that guessing tools try
first. It writes the backup with mode 0600, opens it again before it is written, and prints only the public address.
`pocol-sign --address --backup` decrypts the backup in memory and prints its address without writing any file. All
three lines must show the same address.

Move the ciphertext to offline storage with the admin's own access, then remove both copies from the server, for
example:

```sh
sudo install -m 0600 -o "$(id -un)" /var/lib/pocol-standalone/miner.pocol-backup "$HOME/miner.pocol-backup"
# copy "$HOME/miner.pocol-backup" off the server (for example scp from the PC), label it with the server and the
# address, and check the offline copy (pocol-sign --chain-id CHAIN_ID --address --backup FILE on the offline device
# prints the same address). Then remove the admin's copy and the server's:
rm "$HOME/miner.pocol-backup"
sudo rm /var/lib/pocol-standalone/miner.pocol-backup
```

With the server's copy removed, the next backup can be written to the same path (`export` refuses an existing one).

## 2. Server miner key: restore

Stop the node that used the key first, wherever it runs. The restore commands run as `pocol-standalone`; put the
backup where that user can read it (`install` writes over a file, so the first line checks that none is there):

```sh
sudo systemctl stop pocol-standalone
sudo test ! -e /var/lib/pocol-standalone/miner.pocol-backup && \
  sudo install -m 0600 -o pocol-standalone -g pocol-standalone miner.pocol-backup /var/lib/pocol-standalone/miner.pocol-backup
```

Once the restored node is checked (end of this section), remove the server's copy of the backup again:
`sudo rm /var/lib/pocol-standalone/miner.pocol-backup`.

**The key file is missing and the home is intact.** The node then refuses to start with "existing identity key is
missing; restore its backup, never silently replace it". Restore the file with `pocol-key import`, which never
replaces an existing path:

```sh
sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol-key import \
  --backup /var/lib/pocol-standalone/miner.pocol-backup --out /var/lib/pocol-standalone/identity.pocol-key
sudo systemctl start pocol-standalone
```

It prints the restored address. At start the node checks the key against the home's `identity.address` and refuses a
different one ("key does not match this home's original public identity").

**The key file is damaged, or the server was re-provisioned.** A re-provisioned server already holds a new identity:
`install-linux.sh` made one. Use the home-aware restore of `pocol`, which takes the home lock (the node must be
stopped) and never deletes a key:

```sh
sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol inspect-key-backup \
  --home /var/lib/pocol-standalone --backup /var/lib/pocol-standalone/miner.pocol-backup
sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol import-key \
  --home /var/lib/pocol-standalone --backup /var/lib/pocol-standalone/miner.pocol-backup --replace CURRENT_ADDRESS
sudo install -m 0644 /var/lib/pocol-standalone/identity.address /opt/pocol-standalone/identity.address
```

- `inspect-key-backup` writes nothing. It prints the backup's `address`, the home's `current` account,
  `currentKeyReadable` and `replaces`.
- A damaged key of the same account is restored without `--replace`. The damaged file is kept as
  `identity.pocol-key.replaced-<address>`.
- On a re-provisioned server, `--replace` names the new address that `install-linux.sh` printed (the `current`
  above). The new key is kept as `identity.pocol-key.replaced-<new address>`, and the last line refreshes the public
  copy of the address that `install-linux.sh` put in `/opt/pocol-standalone`.
- A re-provisioned server has no chain state: once the shared `network.pocol.json` is installed beside the binaries
  and the service starts, the node syncs from its peers.

Check the restored node: `pocol_status` on `127.0.0.1:8545` reports `miner` equal to the restored address, and its
`stats.mined` grows.

**The PC (founder 4).** Back up and restore with the wallet (Settings → Key backup → Back up key to encrypted file /
Restore key from backup, while disconnected) or with `pocol.exe export-key --out FILE`, `inspect-key-backup` and
`import-key`, with PoCol closed ([deploy/standalone/README.md](../../deploy/standalone/README.md), "Encrypted key
backup"). The same rules apply: `--replace` names the current account, and the old key file is kept.

## 3. Offline reward keys (owner-held)

The three reward keys of the server founders were made offline with `pocol-key generate` on a device that is never a
server. They are stored offline as an encrypted backup and paper copies, held by the owner
([owner decisions 2026-11-18](../decisions/OWNER-DECISIONS-2026-11-18.md), item 3).

- A further encrypted backup: `pocol-key export --key-file FILE --out BACKUP`. Check it without writing any file:
  `pocol-sign --chain-id CHAIN_ID --address --backup BACKUP` prints the same address.
- Sign straight from the backup, so no plaintext key needs to stay on the signing device: `pocol-sign --backup
  BACKUP ...`. Each reward account starts at nonce 1 (PAY-02), so its first transaction needs `--nonce 1`.
- `pocol-key import --backup BACKUP --out FILE` writes a plaintext key file again; delete it when it is no longer
  needed ([KEYS.md](KEYS.md)).
- These tools restore a key only from the encrypted backup; none of them reads a key from text. The paper copy is the
  owner's own record.

A lost reward key loses what its address holds: the allocation, the rewards already paid there and any bond refund
paid there. The Registry lets a member change its reward address with `setRewardAddress(address)`, a call that acts on
its sender, so the founder's miner key signs it (`prototypes/lp4-execution/contracts/system/PoColSystemV3.sol`). The
new address must already be durable, or the call reverts `RewardNotDurable`: it has sent a transaction (nonce at
least 1, PAY-02) or holds code; a balance alone is not enough (`registry_payment.rs`,
`zero_bond_durable_code_only_and_balance_only_admission`). A fresh offline replacement address therefore first sends
one transaction of its own. The Registry takes at most one maintenance operation (`setRewardAddress`, `topUp`) per
identity per block, and at most `REG_MAINT_MAX` (8) in a block (V3-08). The member's later rewards and its bond refund then go to the new address. What the
lost address already holds cannot be moved.

The wallets offer no screen for this call. It is an ordinary transaction to the Registry, signed by the miner key:

1. The server founder's miner key holds no balance, so first send it enough PCOL for the fee from an owner-held
   account. With `pocol-sign`'s default maximum fee of 100 gwei, a 1,000,000-gas transaction costs at most 0.1 PCOL.
2. Take the nonce from `eth_getTransactionCount` of the miner address (`"latest"`) and pass the count it returns as
   `--nonce`; `pocol-sign`'s default of 0 is wrong for a key that has already sent a transaction.
3. Sign and send it. The gas limit, 1,000,000, is the wallets' value for Registry maintenance calls
   (`REGISTRY_CALL_GAS`, `wallet.rs`); `pocol-sign`'s default of 21,000 is too low for a contract call. The data is
   `0x5e00e679` followed by the new address as a 32-byte word (lower-case, without `0x`, left-padded with 24
   zeros):

   ```sh
   sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol-sign --chain-id CHAIN_ID \
     --key-file /var/lib/pocol-standalone/identity.pocol-key \
     --to 0x0000000000000000000000000000000000c0c001 --nonce NONCE --gas 1000000 \
     --data 0x5e00e679000000000000000000000000NEW_REWARD_ADDRESS_HEX
   ```

   Send the printed line with `eth_sendRawTransaction` as in section 4, step 3, and check its receipt.

## 4. A lost miner key with no backup

What the Registry rules give (`PoColSystemV3.sol`; [FRESH-NETWORK-CP.md](../decisions/FRESH-NETWORK-CP.md), C4;
[deploy/standalone/README.md](../../deploy/standalone/README.md), "Full set"):

- Without the key nobody can act as that member. `deregister`, `claimBond`, `topUp`, `reactivate` and
  `setRewardAddress` all act on their sender.
- **Below a full set** the old record stays active and its bond stays held in Escrow. Nobody can remove it: below a
  full set `removeInactive` and `registerReplacing` refuse every victim. It shares nothing, so it earns nothing. The
  network is continuous (`H_END` 0), so no closing refund applies.
- **At a full set** (10,000 active members), once the record has had no credited share and no (re)activation for
  `Theta_inact` (360 blocks, 1 h), anyone may remove it with `removeInactive(id)` or take its seat with
  `registerReplacing(reward, id)`, while more than `M_min` (1) members are active. It then exits at that height h, with
  its due height h + `U` (380 blocks). From the due height the Registry's per-block sweep refunds the bond to the
  record's reward address and erases the record. Until then the record can still be punished.

To bring the founder back, make a new identity and register it again:

1. Stop the node and move the old home aside, never deleting it; then make a new identity:

   ```sh
   sudo systemctl stop pocol-standalone
   sudo mv /var/lib/pocol-standalone /var/lib/pocol-standalone.lost-OLD_ADDRESS
   sudo install -d -m 0700 -o pocol-standalone -g pocol-standalone /var/lib/pocol-standalone
   sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol identity --home /var/lib/pocol-standalone
   sudo install -m 0644 /var/lib/pocol-standalone/identity.address /opt/pocol-standalone/identity.address
   ```

   `pocol identity` prints the new address. If the old home had taken a newer invitation with `pocol upgrade`, import
   that one into the new home (`pocol import --home /var/lib/pocol-standalone --network FILE`, as `pocol-standalone`),
   otherwise the node uses `/opt/pocol-standalone/network.pocol.json`. Back the new key up at once (section 1), start
   the service and let it sync.
2. Fund the new address with the bond you will lock plus the fee. With `pocol-sign`'s default maximum fee of 100 gwei,
   a 400,000-gas transaction can cost at most 0.04 PCOL. A server founder's key holds no balance, so the owner sends
   it from an owner-held account. The faucet's 0.002 PCOL is sized for the wallets' registration, not for this
   default maximum fee.
3. Register: `register(address)` to the Registry, with a bond from 0 (ACCRUING: rewards fill the bond) to 20 PCOL
   (HELD). The reward word is the founder's owner-held reward address (it is durable from genesis), lower-case
   without `0x`, left-padded with 24 zeros. The gas limit, 400,000, is the wallets' registration reservation
   (`EARNIN_REGISTER_GAS`). A new key's first transaction has nonce 0.

   ```sh
   sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol-sign --chain-id CHAIN_ID \
     --key-file /var/lib/pocol-standalone/identity.pocol-key \
     --to 0x0000000000000000000000000000000000c0c001 --value BOND_WEI --nonce 0 --gas 400000 \
     --data 0x4420e486000000000000000000000000REWARD_ADDRESS_HEX
   curl -s -H 'Content-Type: application/json' \
     --data '{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":["SIGNED_TRANSACTION"]}' \
     http://127.0.0.1:8545
   ```

   `SIGNED_TRANSACTION` is the line `pocol-sign` printed. `eth_getTransactionReceipt` shows its status; `pocol_getMiners`
   then lists the new id with status 1.
4. **At a full set** `register` reverts with `Capacity`. Register by replacement instead, as the wallets do:
   `pocol_getRemovable` lists the removable members (`full`, `removable`), and `registerReplacing(reward, victim)` takes
   calldata `0x5326ca0b` followed by the reward word and the victim word. The wallets reserve 500,000 gas for it
   (`REPLACE_GAS`).

A lost PC key (founder 4) loses the PC's own balance as well, since its id is its reward address. The wallet refuses
a home whose key is missing, so close PoCol, move `%LOCALAPPDATA%\PoCol` aside (never delete it), open the wallet with
the network file to make a new identity, and register it from the wallet (**Review mining registration**).

A key that may have been **copied** rather than lost is a different case: its holder can sign everything the member
can, including `setRewardAddress` and `deregister`, and the Registry has no key rotation. Stop the node and bring the
founder back with a new identity as above; the old record then follows the rules above, except that the key's holder
can also act on it.

## 5. Faucet operator key

The operator key is a hot key generated on the faucet's server and funded with the float only
([deploy/faucet/README.md](../../deploy/faucet/README.md)). A lost or stolen operator key loses only what it holds: at
most the float (10 PCOL on mainnet). Nothing else depends on it, so it is replaced, not restored:

```sh
sudo systemctl stop pocol-faucet
# only if a damaged file is still there: keep it, never delete it
sudo mv /var/lib/pocol-faucet/operator.pocol-key /var/lib/pocol-faucet/operator.pocol-key.lost
sudo -u pocol-faucet /opt/pocol-standalone/pocol-key generate --out /var/lib/pocol-faucet/operator.pocol-key
```

`pocol-key generate` prints the new operator address. Before restarting, compare any ledger entry left as
`"state": "unknown"` with the old operator account's nonce (`eth_getTransactionCount` of the old address), as the
faucet README says. Keep the ledger: it records claims by claimant address, so it stays valid with the new key. Fund
the new address with at most the float, then `sudo systemctl start pocol-faucet`. The faucet reads the operator
address from the signer at start. On a rehearsal server use `/opt/pocol-testnet/bin/pocol-key` instead, with
`config.example.json` (signer `/opt/pocol-testnet/bin/pocol-sign`), as deploy/faucet/README.md says.
