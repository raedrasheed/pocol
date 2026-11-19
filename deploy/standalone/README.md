# PoCol Mainnet · Wallet 0.5.0 and native node tools

The Android artifact is an **unsigned release APK**: it is not installable until the
release operator signs it (see `prototypes/pocol-android/README.md`).

Run three Linux server miners, one native Windows wallet, and one native Android
wallet. Both wallets contain the same full validating Rust/P2P engine and sign
locally. Each wallet is **wallet only** by default: its local node validates and
synchronizes but never mines. **Settings → Device role → Wallet and miner** adds the
Mining tab, whose mining switch starts/stops actual block production while validation
and synchronization continue. No assistant or central signing/control service is
needed after the one-time network setup.

The network is displayed as **PoCol Mainnet** only after its chain ID and
genesis are verified. The designation does not change its experimental admission
conditions or `publicLaunchReady: false`; financial qualification and independent
audit are not claimed. The wallet validates locally and signs on the device.
Its primary color is #1aa9e4; it has Wallet, Activity and Settings screens (plus a
Mining screen in the wallet-and-miner role), account transfers, receive/copy/share
(Android), transaction review, and explicit bonded miner registration. No invented fiat price, reward yield or
external assets are displayed. Activity lists transactions submitted by this
installation, with inclusion/revert status and confirmation counts.

## Continuous protocol

Every network this release builds is a v3 genesis (`specVersion` 2), and every genesis
`pocold init-testnet` builds is continuous (only a local `pocold init-devnet --h-end`
devnet is finite). The bootstrap passes no option for it: `pocold init-testnet`
refuses the retired options `continuous`, `earnin` and `fullNonce` by name. The
genesis explicitly encodes all four lifetime fields (`H_END`, `T_END`, `H_CLOSE`,
`T_CLOSE`) as zero, meaning **no scheduled closing block, final block or expiration
date**. This is not a larger hidden end height. The release bundles one compiled
system-program set (v3), which keeps the registry open; a spec-1 genesis is refused
at start.

A continuous chain applies TX_CAP and TXBYTES_CAP per block, and resets the historical
log allowance at each block. Per-transaction, per-block, live-state, gas, peer and
queue protections remain, and no cumulative transaction/log limit can gradually
exhaust the ability to send. Every device runs the same release with one shared
genesis; never erase an existing identity or regenerate different genesis files per device.

No fixed machine can promise infinite storage. History, replay indexes and some
metadata still grow. The local memory model explicitly projects from observed
journal records plus 2048 records of headroom, rechecks as records grow, and refuses
inadequate local capacity without marking valid blocks invalid. It does not claim
an infinite-history resource bound. `POCOL_MEMORY_BUDGET_BYTES` can set the launcher's
local declared budget after provisioning resources; it does not change consensus.
`POCOL_PEERS_MAX` (or `pocold run --peers-max N`, 1..128) sets how many established
inbound peers the node keeps, default 32; up to 8 more connections may wait for their
Hello. When the 32 are full, a newcomer evicts one inbound peer (the youngest of the
most crowded address range; a few recent useful peers, the longest connected and the
configured peers are protected) or is closed. `POCOL_OUTBOUND` (`auto`, the default: 4 automatic outbound connections with `--listen`, 2 without; or 0..8, 0
restoring the earlier behaviour) sets how many connections the node opens itself to addresses it learned, plus one
spare slot; `pocol start --peer IP:PORT` (repeated, at most 7) adds static peers besides the invitation's seeds. A
seed host recognises its own seed address from a local listener on the seed's port (or by its nonce, with NAT
hairpinning); one behind 1:1 NAT lists the other seeds with `--peer` to keep them all, and never passes its public IP
as `--listen`, which is on no interface there (the bind fails and the node does not start).
`POCOL_PER_GROUP_MAX` (or `pocold run --per-group-max
N`) sets how many inbound connections one public address range (an IPv4 /16 or IPv6
/32) may hold, default 4, plus one spare place whose newcomer must
evict within its own range; `0` turns the limit off. Both are local P2P policy and do
not change consensus. Every inbound place counts against the memory budget, about
3.73 MiB each: an operator who lowers `POCOL_MEMORY_BUDGET_BYTES` should lower
`POCOL_PEERS_MAX` with it.
The default model uses 6 GiB; a 4 GiB server with a 3 GiB service cgroup remains a
small pilot, not a worst-case capacity proof. The service's `MemoryMax=3G` was already
below the model's bound, and the 32 established inbound places add about 126 MB of
budget to it (110 MB of model) over the earlier 8. The unit's `TasksMax=256` holds the
thread inventory (164 tasks with 7 configured peers and automatic outbound, 144 with 2). Disk sizing, compaction and long-term
load qualification remain operational work. Numeric heights retain the format's
u64 representation; zero disables scheduled expiry, not hardware arithmetic.

## Fresh network setup from Windows (the genesis ceremony, FN-7)

This section creates the network: one genesis, its invitation and the server
installations. Every device then joins with that same invitation.

`Initialize-PoColNetwork.ps1` creates the fresh v3 network: first the **rehearsal**, then
**PoCol Mainnet**, each in its own ceremony with fresh keys. It fills the committed network
profile shipped beside it (`v3-rehearsal-profile.json` or `v3-profile.json`, both covered by
`SHA256SUMS`; the values are decided in `development/decisions/FRESH-NETWORK-CP.md`) and
admits its genesis with `pocold init-testnet`, without `--test-placeholders`. The script
fills only the ceremony inputs: the launch second, the founders' measured hash rate and the
four founders' public addresses. Every other value, including the chain id (rehearsal
1656550731, mainnet 2656089850, CP §5) and `registrationPerBlock` (equal to `R_max`, 4), is
the committed profile's, and the script refuses a profile whose chain id is not the one of
`-Network`.

The four founders are the three servers (founders 1-3, in the order given) and this PC
(founder 4). Each generates a fresh key on its own device; only public addresses are
collected. Each founder is a HELD member from genesis with a 20 PCOL bond in Escrow. Founders
1-3 are paid to owner-held reward addresses (owner decision 2026-11-16): the 999,980 PCOL
allocation and every mining reward of each server founder go to an address the owner holds
offline, given to the script as `-ServerRewardAddresses`; only the 20 PCOL bond is tied to each
server, whose key holds no balance (send it PCOL before a server transaction such as
reactivate, deregister or claimBond). Founder 4 (this PC) is its own reward address. The phone is not a founder: it joins with the invitation, wallet only by
default, and can register as a miner later (see *Mining* below).

Before the run:
- Provision three fresh Ubuntu servers with the `pocol-admin` account, the local key
  `%USERPROFILE%\.ssh\id_ed25519_pocol` and NTP-synchronized clocks. Configure a w32time
  NTP peer on this PC (every founder within 0.5 s). The script refuses a local or
  free-running time source and measures this PC's offset against that peer.
- This PC needs a fresh wallet home: if `%LOCALAPPDATA%\PoCol` exists, back up its key
  (`pocol.exe export-key --out FILE`), close PoCol and move the folder aside (never delete
  it). The script refuses to start otherwise.
- Measure the founders' aggregate hash rate (the rehearsal's `networkHashrateEst` in
  `pocold status` feeds the mainnet run), and choose the launch second, or a delay.

Download the CI artifacts of the pinned release commit and check them against that CI run's `SHA256SUMS`
(integrity only: the ceremony runs before the identity switch, so there is no signed release yet; the first one,
checked as in *Verify the download* below, follows the switch). Then extract
`PoCol-Mainnet-0.5.0-windows-x86_64.zip` into a new directory. Review `Initialize-PoColNetwork.ps1`,
open PowerShell inside that directory, and run (the addresses are placeholders for the
servers' public IPv4 addresses):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "& .\Initialize-PoColNetwork.ps1 -Network Rehearsal -Servers SERVER1,SERVER2,SERVER3 -ServerRewardAddresses OWNER_ADDR1,OWNER_ADDR2,OWNER_ADDR3 -HashesPerSecond RATE -LaunchDelaySeconds 900 -LinuxArchive .\PoCol-Mainnet-0.5.0-linux-x86_64.tar.gz"
```

`OWNER_ADDR1..3` are placeholders for three distinct owner-held addresses, one per server in
`-Servers` order. Make them on another device that is never a server, ideally offline:
`pocol-key generate --out FILE`, then `pocol-key address --key-file FILE` (lower-case `0x` hex,
as the script requires); keep each key file and its backup offline. Back the key up encrypted
with `pocol-key export --key-file FILE --out BACKUP` (the `export-key` format; the passphrase is
read twice from stdin) and check it, without writing any file, with
`pocol-sign --chain-id ID --address --backup BACKUP`, which prints the same address;
`pocol-sign --backup BACKUP` signs from the backup directly. The script refuses a
missing, repeated, mixed-case, zero or system address before it contacts any server, and an
address equal to a founder's miner identity. Each reward account starts at nonce 1 (PAY-02), so
its first offline-signed transaction needs `pocol-sign --nonce 1` (pocol-sign defaults to 0).
The rehearsal takes the same parameter (throwaway or separate addresses are fine there).

`-GenesisTime UNIX_SECONDS` replaces `-LaunchDelaySeconds` for an announced launch second; the
genesis is created at least 300 seconds before it. A founder that starts more than `D_att`
(90 s) after the launch second produces fallback-only blocks, so allow time for the three
sudo prompts. P2P is TCP 30300 on every server (the port `install-linux.sh` configures).
`-DryRun` runs only the local steps (four throwaway identities in the setup folder, a TEST
GENESIS on the local test chain id, an invitation in that folder) and contacts nothing; CI
runs it (`test_initialize_network.ps1`).

The process-scoped policy exception is for this script only. It uses `-F none` to avoid the
previously broken SSH config permissions. Enter passphrases and sudo passwords only into
your local terminal. Host-key verification remains enabled; verify new fingerprints through
the provider console. No key is forwarded or copied. The script contacts only the three
servers given. It verifies the package checksums, the build identity and the Linux/Windows
source compatibility (equal build digests across the packages it installs; peering itself
depends on the consensus identity, VA-SPEC §2), installs the servers, collects their public
addresses, creates this PC's identity, admits the genesis, distributes its invitation, opens
P2P TCP 30300, starts the server services and opens **PoCol-Wallet.exe**. Server RPC remains
loopback-only. Existing state and identities are never overwritten. On a partial failure,
retain the setup folder and the SAME genesis; do not blindly rerun or reset. The remote
deployment path has not been exercised by CI.

The genesis is admitted as an experimental network (below), and the script prints
`publicLaunchReady: false` with every launch blocker verbatim; it refuses a genesis that is
not in experimental-devnet scope or that comes from a non-release build.

After the run, on PoCol-Wallet: mining is off by default. Choose **Settings → Device role →
Wallet and miner**, turn mining on at once and keep it on (founder 4 mines from genesis).

**Rehearsal, then mainnet.** The mainnet ceremony runs on the same servers and PC with the
mainnet profile, after the rehearsal: re-provision the three servers (`install-linux.sh`
refuses an existing installation), extract the package into a new directory, and move this
PC's rehearsal home aside as above. Every founder key is then fresh for the mainnet.

Transfer the generated **network.pocol.json** to the phone. After the signing and
update checks in the release guide (`deploy/release/README.md`), install the correctly signed ARM64 APK,
open **PoCol Wallet → Settings → Import network file**, select that file, and
connect. Windows imports the adjacent shared file automatically; Settings also
supports explicit import while disconnected. All devices use the same invitation.
Publish the invitation fingerprint through a trusted channel, and sign the invitation with the release key
(`deploy/release`); a checksum is not publisher authentication, the release signature is. The websites take their identity and the explorer's seed list
from this invitation, in the repository (W15 for the rehearsal, W16 for the mainnet):
`python3 deploy/websites/tools/site_identity.py --to network.pocol.json --commit RELEASE_COMMIT --gateway-host explorer.pocol.net`.
On the gateway server, the explorer gateway and the explorer site at `explorer.pocol.net` (its static files, `/api`
reads, `/rpc`) are installed from that commit by `deploy/explorer/install_explorer_gateway.py`
(`deploy/explorer/README.md`): `--check` before the owner moves the `explorer.pocol.net` DNS record to that server, the
full install (whose HTTPS self-test needs the record) right after, then the Studio gateway (`deploy/studio`).

### Verify the download

Every published file has an ssh-ed25519 release signature `FILE.sig`, and the release's `SHA256SUMS` has
`SHA256SUMS.sig` (owner decision 2026-11-16; `deploy/release/README.md`, user procedure). Take `allowed_signers` from
the repository or pocol.net and compare its fingerprint with the published one
(`LC_ALL=C grep -Ecv '^[[:blank:]]*(#|$)' allowed_signers` must print `1`, then
`grep -v '^ *#' allowed_signers | cut -d' ' -f3,4 | ssh-keygen -lf -`). That only checks the file: the fingerprint that
decides is the signing key's, printed by the signature check below or the `VERIFIED` line. A `FILE.sig` binds no
file name or version, so verify the signed `SHA256SUMS` and the download's line in it, with OpenSSH 8.2 or later:

```sh
ssh-keygen -Y verify -f allowed_signers -I release@pocol.net -n pocol-release -s SHA256SUMS.sig < SHA256SUMS
awk -v f=FILE '$2 == f' SHA256SUMS | sha256sum --check
```

(the first ends `with ED25519 key SHA256:...`, the published fingerprint; the second must print `FILE: OK`), or the
whole directory with `deploy/release/verify_release.py DIR` (Windows: `Verify-PoColRelease.ps1 -Directory DIR`), which
prints the key fingerprint in its `VERIFIED` line.
Only `VERIFIED` is a verified release: the `SHA256SUMS` inside a package checks its integrity, not its publisher.

### Launch disclosures (DRAFT: wording for owner review)

> **DRAFT — not yet reviewed by the owner** (owner decision pending: launch-disclosure wording). Drafted from the
> fresh-network design's §4a and C8, which is kept outside this repository; the decided values are in
> `development/decisions/FRESH-NETWORK-CP.md`. Owner decision 2026-11-18: the disclosures carry no "not
> public-launch-ready" item and no list of ongoing engineering work; the other items below await the owner's review.

- **Founders.** Four founders (three servers and the owner's PC) hold 1,000,000 PCOL each
  from genesis, including each one's 20 PCOL bond: 4,000,000 PCOL of initial supply. The
  999,980 PCOL of each server founder sits on an owner-held reward address, not on the server.
- **Founders' seats.** At a full set (10,000 members), a member with no credited share for
  `Theta_inact` (360 blocks, 1 h), a founder included, can be replaced by a newcomer. It
  registers again with 20 PCOL after its refund.
- **Seats at a full set.** At a full set of equal small miners about a third of the members
  are replaceable at any moment, and newcomers can replace up to 1,440 per hour.
- **State caps.** No creation fee: the account cap (1,000,000) can be filled cheaply, after
  which new accounts cannot be created (and newcomers cannot register) until the dormant
  cap-raise path is used. `pocol_getCaps` reports the occupancy.
- **Low-bond members.** Members that are still earning their bond (ACCRUING) can win pots
  and, from 5 PCOL of bond, propose blocks; type-2 evidence against an ACCRUING winner makes
  its whole pot invalid.
- **10 s blocks.** Natural forks of about 1 %; a home PC pays a latency tax of about 1-2.5 %.
- **Retention.** A node partitioned for more than about 5.7 hours (R_KEEP 2,048 blocks)
  resyncs.
- **Disk.** The block journal is never compacted: up to about 1.7 GB per day in the worst
  case.

## Everyday use

- Windows 10 22H2 x64 / Windows 11: open **PoCol-Wallet.exe** or **Start-PoCol.cmd**.
  Native WPF uses Windows' .NET Framework 4.8; no WSL, Docker, browser or Python.
  The wallet starts the engine through private anonymous process pipes, not an
  HTTP admin endpoint. Closing the application checkpoints and stops the engine.
  Automatic startup after Windows reboot is not configured.
- Android 8+ ARM64: open **PoCol Wallet** and connect. The full engine runs in a
  separate foreground-service process with a notification and Stop control. Android
  may suspend/kill it because of battery, heat or memory policy. Keep server miners
  online; mobile 24/7 availability is not promised. After a reboot, reconnect manually.
- Linux: `pocol-standalone.service` starts on reboot and restarts after failure. Its
  installer is fresh-install only and preserves Unix executable modes in the tarball.

**Send:** enter the full recipient and a decimal PCOL amount, review network,
recipient, amount and maximum fee, then explicitly confirm/sign. The Rust engine
parses up to 18 decimal places without floats. Quotes expire after 60 seconds and
can be consumed once. A quote does not broadcast. Zero/system/precompile recipients,
contract calls from the simple send screen, stale/offline states and insufficient
available balance are refused. Inclusion is not advertised as irreversible finality.

**Device role:** both wallets are **wallet only** by default, including a home from a
release without the choice (no `app-mode.json`): the local node validates every block
but never mines, the Mining tab and **Manage mining** are hidden, and the engine refuses
mining, registration, top-up and rejoin with "This device is set to wallet only". In
**Settings → Device role** the user chooses **Wallet and miner** (the engine's
`set_mode` command; the choice is stored in the home's `app-mode.json` and survives
restarts and key restores) and can switch back at any time; switching back stops
mining at once (it is refused only while a confirmed rejoin is under way), even when
the choice cannot be saved (full or read-only storage: the engine then reports the
error, stays wallet only until restart, and the app may reopen as wallet and miner
with mining off). A registered identity that switches to wallet only earns no mining
rewards and, once every seat is taken, can be replaced as an inactive member; the
engine's `roleWarning` says so on the wallet card and in Settings → Device role,
since wallet only has no Mining tab.
Server miners (`pocol start`) are not affected.

**Mining:** off by default in the wallet-and-miner role too, including after restart. The four
founders (the three server keys and the ceremony PC's key) are genesis miners. Any other
device must hold an eligible registered identity to enable mining. A new identity can use **Review mining registration**,
review the current bond and maximum fee, and explicitly confirm its transaction.
Registration does not automatically enable mining. It needs the fee, a free or
replaceable seat (see *Full set* below) and network inclusion. It can be the identity's first
transaction: the Registry accepts the identity itself as its reward address. The ceremony PC holds its founder allocation; send PCOL to the phone's public address to fund its fees and, if wanted, its
bond. Turning mining off does not deregister, release the bond or stop the wallet.
Inactive/ejected identities follow existing registry rules; registration is never
silently repeated and a toggle does not bypass eligibility.

**Bond:** the bond is 20 PCOL, and a new identity does not need it up front.
**Review mining registration** offers a 0 PCOL bond by default and can lock 0 to 20 PCOL; the
rest is earned from mining rewards, with no deadline (owner decision 10c).
1. Get the first fee: the fee faucet (`deploy/faucet/README.md`; on mainnet and on the rehearsal network, owner
   decision 2026-11-18) sends 0.002 PCOL once to an account that has never sent a transaction. Any PCOL holder can
   also send it with an ordinary transfer.
2. The registration can be that account's first transaction (see *Mining* above).
3. The record is *accruing*: it mines like any member, but each reward fills the bond until it
   holds 20 PCOL. Only then are rewards paid to the balance. Accruing records count within the
   member limit (`M_max`), and an accruing member can propose blocks once its accrued bond is at
   least a quarter of the bond (5 PCOL).
4. The status carries `bondRemaining` for an active record (an exited record earns nothing more:
   its bond is refunded at its due height), and both wallets show it next to the bond held.
5. **Top up bond** (or `quote_top_up`) fills it at once while the record is active; otherwise the
   review says why (the bond is already held in full, the record has exited, or there is no
   record) before anything is signed. If rewards filled part of it before the top-up was
   included, the extra returns to the identity. The Registry takes one bond top-up or
   reward-address change per identity per block, and eight such operations per block across the
   network (V3-08): while a top-up or reward-address change of this account is still pending,
   the wallet hides **Top up bond** and refuses a second top-up, which would revert and cost its
   fee.

**Rejoin mining:** every live member keeps a nonce range whether or not it has shared recently
(D-V3-01; the network builder always sets nonceMode 1), so an active member is never shut out for
idling. **Rejoin mining** applies only after a voluntary exit (`deregister`); a member replaced at a
full set waits for its refund (below). When the engine reports `rejoinAvailable`, the Windows wallet
shows **Rejoin mining**. One review and confirmation then cover the whole sequence:
1. `deregister()` is signed immediately. The bond stays held.
2. `reactivate()` is signed by the engine once the next block is in the window. The Registry allows
   one reactivation per `C_react` blocks per identity (V3-09), and the engine reads the record's own
   clock: the window is `[max(exit + W_act, lastReactivation + C_react), exit + U)`, and the rejoin
   review states it in block heights. An identity that has never reactivated has no clock, so its
   window is `[exit + W_act, exit + U)`, with `W_act` (32 on PoCol Mainnet) and `U` (380).
   On Mainnet that is blocks exit+32 to exit+379. The engine makes at most three attempts, one
   per new head, because other registrations can take the block's registration slots
   (`R_max = 4`).
3. Mining is turned on once the identity is back in the active set.

The review shows the maximum fee for all four signatures. No PCOL is transferred.
- If the clock keeps the window shut until the refund at `exit + U`, the wallet says so at once
  (rejoin step `missed` with `cause: "cooldown"`, the refund height and the next allowed height)
  instead of trying.
- If the window passes, the Registry refunds the bond to the reward address at `exit + U` and erases
  the record. You then register again.
- If the wallet restarts while the identity is exited, **Rejoin mining** reviews the reactivation
  alone.
- The mining card shows the last reactivation, and the next allowed height while it is still ahead.
- Nothing about consensus, the Registry, keys or balances changes.
- The Android app offers the same action (**Rejoin mining** on its Mining tab, in the wallet-and-miner role), and **Top up bond**. Its engine
  loop advances a confirmed rejoin every 2 seconds, as the desktop wallet does (it signs `reactivate()` when the window
  opens), and shows each step (scheduled, reactivating, retrying, rejoined, stopped) as a notice; keep the app
  running until the account is back in the active set.

**Full set** (owner decisions 10b and Q2(a)): when all `M_max` seats are taken, **Review mining
registration** registers by replacement (`registerReplacing`): the engine picks at random a member
with no credited share and no (re)activation in the last `Theta_inact` blocks
(`pocol_getRemovable`), and the review names it and says that its bond is refunded after `U` blocks
and that it may register again. The status fields `seatsFull`, `removableSeats` and
`registerReplaces` drive the mining card's line; with no removable member the registration is refused
until one is. A member out of the active set while the set is full (replaced, or exited) cannot
reactivate (the Registry refuses it): the wallet shows "wait for the bond refund at block
`dueHeight`, then register again" and offers no rejoin.

There is no creation fee (FN-2a): a transfer to a new address is an ordinary 21,000-gas transfer.

## Keys and updates

Windows uses authenticated user-bound DPAPI ciphertext and rejects raw Unix keys,
reparse points, hard links and altered ciphertext. Linux/Android retain strict
0600/0400, same-user, single-link key checks. Keys never cross the wallet UI bridge,
HTTP RPC or P2P. A home lock prevents concurrent engines. Missing, corrupt or
substituted keys fail instead of silently rotating identity. A Linux or Android key
file is plaintext at rest; a Linux server can hold its key in an encrypted systemd
credential instead (*Encrypted server key* below).

### Encrypted key backup

Both wallets and the `pocol` command can export the key to a **passphrase-encrypted
backup file**, and restore it on another device or platform. The format is one
98-byte file: magic `POCOL-KEY-BACKUP`, version 1, PBKDF2-HMAC-SHA256 with 600,000
iterations, and AES-256-GCM. It is defined once in
`prototypes/pocold/src/keyfile/backup.rs`.

- Android and Windows: **Settings → Key backup → Back up key to encrypted file** /
  **Restore key from backup**, while the wallet is disconnected.
- Command line (the passphrase is read from stdin, with terminal echo off; export
  reads it twice):

  ```sh
  pocol export-key --home HOME --out wallet.pocol-backup        # never overwrites a file
  pocol inspect-key-backup --home HOME --backup wallet.pocol-backup   # read-only
  pocol import-key --home HOME --backup wallet.pocol-backup [--replace CURRENT_ADDRESS]
  ```

A new backup's passphrase must have at least 12 characters and is refused when it is
easy to guess: fewer than 5 different characters, a repeated part, a sequence such as
`abcd` or `1234`, or a common password or keyboard run (`keyfile::backup::check_new_passphrase`).
`inspect-key-backup` and `export-key` need an existing home and never create one. A
terminal whose echo cannot be switched off is refused instead of showing the
passphrase, and Ctrl-C cancels the prompt.

A restore that would replace the home's current account needs that account's address
(`--replace`, or the confirmation dialog in the wallets). The old key file is kept as
`identity.pocol-key.replaced-<address>`, and its `wallet-activity.json` moves aside.
A lost or unreadable key (for example after a Windows profile reset) is restored from
its own backup without `--replace`. The plaintext key is never written outside the
home's key file. A restore interrupted by a crash or power loss either changed
nothing or is finished the next time the wallet opens its key. Keep the backup file
and its passphrase apart. Stop mining on any other device that uses the same key: two
devices mining with one key can lose the bond.

Android cloud backup and device-to-device transfer copy nothing of the app
(`allowBackup="false"`, `fullBackupContent="false"` for Android 8 to 11, and
`dataExtractionRules` excluding every domain on Android 12 and later), and uninstalling
erases identity and state, so make an encrypted backup first. The app is
`org.pocol.wallet`. CI builds only unsigned release APKs; the owner signs each release
with the Android release key, and every later release must be signed with the same key
to update an installed app (`prototypes/pocol-android/README.md`). No keystore/private
key belongs in the release bundle or repository.

### Encrypted server key (Linux, opt-in)

`install-linux.sh` leaves the server's miner key in `/var/lib/pocol-standalone/identity.pocol-key`:
plaintext at rest, protected only by its owner and mode (the service user, 0600). A server can instead
hold it in a **systemd credential encrypted at rest** (gate K1). `encrypt-server-key.sh`, in the Linux
package, makes the move on a server whose service already runs:

```sh
sudo bash encrypt-server-key.sh --backup FILE   # --with-key host|tpm2|host+tpm2
sudo bash encrypt-server-key.sh --keep-plaintext  # seals and switches, but removes nothing
```

It needs systemd 250 or later (`LoadCredentialEncrypted=`). Make the passphrase-encrypted backup first
(`pocol export-key`, above) and give its path, readable by `pocol-standalone`, as `--backup FILE`: the
plaintext key file is removed only after that backup was checked to open to the same address. The script
refuses, changing nothing, unless it runs as root and finds `systemd-creds`, this installation's unit, a
running service, the plaintext key file, a `pocol` with `--key-credential`, and `--backup FILE` or
`--keep-plaintext`. It also refuses when a credential or drop-in already exists. Then it:

1. stops the service and reads the key file's address with `pocol identity`;
2. with `--backup`, checks that the backup opens to that address (`pocol inspect-key-backup`; it asks for the
   backup's passphrase);
3. seals the key file with
   `systemd-creds encrypt --name=pocol-identity identity.pocol-key /etc/credstore.encrypted/pocol-identity`.
   The default key is `host+tpm2` when systemd finds a usable TPM2, otherwise `host`; a null key is refused;
4. checks that the credential decrypts to the same bytes (through a pipe, never a file);
5. writes the drop-in `/etc/systemd/system/pocol-standalone.service.d/50-encrypted-key.conf`. The drop-in sets
   `LoadCredentialEncrypted=pocol-identity:/etc/credstore.encrypted/pocol-identity` and the unit's
   `ExecStart` plus `--key-credential pocol-identity`;
6. starts the service and waits until it reports the same address and keeps running. It stops waiting when
   the node exits first;
7. removes the plaintext key file with `shred --remove`, run as the service user, unless `--keep-plaintext` is
   given. It first checks the path again and removes nothing if it is no longer a regular file.

If anything fails before step 7, or the script is interrupted (Ctrl-C, a hang-up, TERM), it rolls back: it
removes the credential and the drop-in, clears the unit's start limit (`systemctl reset-failed`), and starts
the service again from the key file, which stays in place. It then checks that the service runs and reports
the key file's address, and says `ROLLBACK FAILED` when it does not; then start the service yourself
(`systemctl reset-failed` and `systemctl start`, see `journalctl -u pocol-standalone.service`).

With `--key-credential NAME`, `pocol start` reads the key only from `$CREDENTIALS_DIRECTORY/NAME`. systemd
decrypts the credential there, into memory-backed storage of the service. `pocol` never opens
`identity.pocol-key`, even when the credential is missing or `$CREDENTIALS_DIRECTORY` is unset. It refuses
to start then, and it refuses a credential whose account is not the home's `identity.address`. The
credential holds a key file's 32 bytes and gets a key file's checks: one link, a regular file, a valid key.
Only its owner may differ. It may be owned by the service user with mode 0400 or 0600, as a key file may,
or by root with mode 0400 or 0440, as systemd can place it. `identity`, `upgrade`, `inspect-upgrade` and
`export-key` take the same option. `import-key` and `inspect-key-backup` work on the key file and do not.

What this protects, and what it does not:
- Once the script removes it, no plaintext key file remains in the live state directory, and backups taken
  afterwards hold only the credential. Earlier backups, and old blocks of the disk, may still hold the
  plaintext (see below). With `host`, the credential is decrypted with `/var/lib/systemd/credential.secret`,
  a root-only file on the same disk, so a full copy of the disk still yields the key. With `tpm2` or
  `host+tpm2`, the copy also needs this machine's TPM2.
- Root, and the running service, can still read the key, and so can anyone who controls the running host.
  This is no HSM: the key is in the service's memory while it runs.
- `shred` cannot reach copies kept by a journaling or copy-on-write file system, an SSD's spare blocks or an
  earlier backup. On a fresh server, run the script before the first backup; full-disk encryption covers the
  rest. Older plaintext files left by key restores (`identity.pocol-key.replaced-*`) are listed, not removed.

Recovery with an encrypted server key (these commands are documented, not yet run on the servers; the
rehearsal exercises them):
- The credential is bound to this machine. A reinstalled host, a new host secret, a cleared or replaced TPM2,
  or a move to another server cannot decrypt it. With `tpm2` or `host+tpm2`, the script keeps
  `systemd-creds`' default TPM2 policy. systemd-creds(1) documents that default as binding to PCR 7, the
  Secure Boot state, or to a signed PCR 11 policy when `/etc/systemd/tpm2-pcr-public-key.pem` exists. So a
  firmware update, a Secure Boot key or revocation update, or switching Secure Boot on or off can also make the
  credential undecryptable, and the service then fails to start after the next boot. `--with-key host` binds to
  no PCR (and to no TPM2). The account then comes back only from the passphrase-encrypted backup. Restore it
  with the plaintext key file, then run the script again:

  ```sh
  sudo systemctl stop pocol-standalone.service
  sudo rm /etc/systemd/system/pocol-standalone.service.d/50-encrypted-key.conf /etc/credstore.encrypted/pocol-identity
  sudo systemctl daemon-reload
  sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol import-key --home /var/lib/pocol-standalone --backup FILE
  sudo systemctl reset-failed pocol-standalone.service
  sudo systemctl start pocol-standalone.service
  sudo journalctl -u pocol-standalone.service -n 20   # it reports the address in /opt/pocol-standalone/identity.address
  sudo bash encrypt-server-key.sh --backup FILE
  ```

  `FILE` must be readable by `pocol-standalone`. `import-key` restores the home's own account into the missing
  key file; it asks for the backup's passphrase.
- Commands that need the key run with the credential, the service stopped (they lock the home):

  ```sh
  sudo systemctl stop pocol-standalone.service
  sudo systemd-run --pipe --wait -p User=pocol-standalone \
    -p LoadCredentialEncrypted=pocol-identity:/etc/credstore.encrypted/pocol-identity \
    /opt/pocol-standalone/pocol upgrade --home /var/lib/pocol-standalone --network FILE --key-credential pocol-identity
  sudo systemctl start pocol-standalone.service
  ```

  The same form runs `pocol export-key --out FILE --key-credential pocol-identity`. Without the option, a
  command finds no key file and is refused ("existing identity key is missing"). It never makes a new identity.
- Back to a plaintext key file, while the credential still decrypts: stop the service and decrypt the
  credential into the key file, as the service user with mode 0600, then check its address:

  ```sh
  sudo systemctl stop pocol-standalone.service
  sudo bash -c 'set -euo pipefail; systemd-creds decrypt --name=pocol-identity /etc/credstore.encrypted/pocol-identity - \
    | runuser -u pocol-standalone -- sh -c "umask 077; set -C; cat > /var/lib/pocol-standalone/identity.pocol-key"'
  sudo runuser -u pocol-standalone -- /opt/pocol-standalone/pocol identity --home /var/lib/pocol-standalone
  ```

  `set -C` never overwrites an existing key file. The last command must report the address in
  `/opt/pocol-standalone/identity.address`. If a command fails or the address differs, the key file may be
  empty or partial: remove it (`sudo rm /var/lib/pocol-standalone/identity.pocol-key`) before you try again.
  Only then remove the drop-in and the credential (both, or a later run of the script refuses), run
  `sudo systemctl daemon-reload` and start the service.

## Validation and remaining qualification

CI builds native Linux/Windows and the real ARM64 Android engine/JNI/APK. It runs
portable identity/invitation controls, real-socket tests, five local validating
processes, and a five-process wallet test with real transfer, one-shot review,
bonded registration, the wallet-only default and the device-role switch (mining
refused in wallet only, stopped by a switch back, the role kept across restarts),
mining on/off, continued synchronization and identity/history recovery. A compiled-contract execution test applies 20,002 blocks with real signed
transfers on a continuous chain (no closing or ending height, no time expiry). A continuous
consensus test retains per-block caps and checks that local resource projections
grow with history.

The Windows build renders the actual WPF interface to a preview image with no
mock account balance. Android physical lifecycle/UX/performance and a real five-host
cross-platform soak remain unproven. Existing admission/RpcGuard/calibration/review
conditions remain in `development/public-network/PLAN.md`; no audit or financial
mainnet readiness is claimed.

```sh
cargo build --locked --release --bins --manifest-path prototypes/pocold/Cargo.toml
python deploy/standalone/test_launcher.py -v
python deploy/standalone/test_launcher.py --integration
python deploy/standalone/test_wallet.py
cargo test --locked --release --test consensus continuous_admission --manifest-path prototypes/pocold/Cargo.toml
cargo test --locked --release --lib continuous_executes --manifest-path prototypes/lp4-execution/Cargo.toml
```

Android: SDK35 / NDK27.2.12479018 / JDK17 / Gradle8.11.1,
`bash deploy/standalone/build-android.sh`. Windows wallet:
`powershell -File deploy/standalone/build-windows-wallet.ps1`.
Reproduce the system contracts (the V3 programs, the only bundled set since
phase 6a of the fresh network) with solc **0.8.28** and
`node prototypes/lp4-execution/tests/compile_system_v3.cjs out/system-v3.json`,
then compare it byte for byte with `prototypes/lp4-execution/tests/data/system-v3.json`
(as `.github/workflows/system-programs.yml` does).

Platform references:
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/overview/
- https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
- https://developer.android.com/develop/background-work/services/fgs/service-types
