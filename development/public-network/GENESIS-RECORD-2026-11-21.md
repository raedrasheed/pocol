# PoCol Mainnet genesis record, 21 November 2026

This record keeps the public facts of the real PoCol Mainnet genesis ceremony and of what followed it on its first
evening, **as reported by the owner** in the owner's post-genesis report of 2026-11-21 (in Arabic, to Claude). The owner
carried out every action on the servers and the Windows PC personally; nothing in this record was run by Claude, and
no value below was measured by Claude. Where the record explains a reported value, it cites the code at `main`
1bb11f4c; the consensus, node, wallet and ceremony sources it cites are the same in the ceremony commit cecb8e70, apart
from a rustfmt-only layout change in `prototypes/lp1-node`.

- **This is not a public launch.** A successful genesis is not approval of the public launch (the owner's instruction
  8). At the ceremony release `publicLaunchApproved` and `publicLaunchReady` are false and
  `technicalReadiness.complete` is false, as expected before the identity pin (step 7 of
  [the launch runbook](../../deploy/mainnet/README.md)): this release pins no network
  ([readiness.rs:140](../../prototypes/pocold/src/readiness.rs), `NETWORK_PROFILES` is empty), so a booted node reports
  the blockers `network-not-release-pinned` and `launch-not-approved`
  ([readiness.rs:309-322](../../prototypes/pocold/src/readiness.rs)).
- **The genesis is kept.** The owner's instructions: this is a real mainnet and its genesis is preserved; the
  ceremony script is not run again, no new genesis is made, no chain data or identity is deleted and no server is
  reinstalled. Services, mining, reboots, Netplan, UFW and P2P settings are changed only with the owner's approval.
- **No secrets.** This record holds only public values: chain and genesis identifiers, public miner IDs, public
  reward addresses, public server addresses and block hashes. It holds no private key, passphrase, backup file,
  token, channel name or chat identifier.

## Network identity

| Property | Value (as reported) |
| --- | --- |
| Network | PoCol Mainnet (real genesis, not a dry run) |
| Version | 0.5.0 |
| Chain ID | 2656089850 |
| Consensus rules version / protocol version | 2 / 2 |
| Genesis time | 2026-11-21 20:20:56 UTC (22:20:56 Gaza time) |
| Genesis hash | `0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e` |
| Invitation file SHA-256 (`network.pocol.json`) | `a74c1cf432e7fbca59aa6bd8250af660f1b1582d693b1b109316686f7241f233` |
| `networkFingerprint` | `0beba93b8904d6fe7df19c48577f370a4d77eda221f7df0d0a4239558a9dddb8` |
| Build `compatibilitySha256` | `220ca4e34e3942a71b1412534caac8d981adce6e3f79a90b19db07189fa62adb` |
| Ceremony packages | built from commit `cecb8e703018ea8d7fd6e6430945306ce1b281a5`; Windows and Linux packages compatible, both `consensusPinned=true` |
| Founders | 4 |
| Measured total hash rate (`-HashesPerSecond`) | 8,403,430 H/s |
| Seeds | 108.165.128.89:30300, 108.165.128.90:30300, 108.165.128.91:30300 |

The invitation file SHA-256 and the `networkFingerprint` differ by design: the file is written pretty-printed
([launcher.rs:410](../../prototypes/pocold/src/launcher.rs)) and the fingerprint is the SHA-256 of the compact
serialization ([launcher.rs:112-117](../../prototypes/pocold/src/launcher.rs); runbook step 5,
[deploy/mainnet/README.md:84-89](../../deploy/mainnet/README.md)). The file SHA-256 is the value the founding devices
compare and the owner announces. The owner reports that the invitation's SHA-256, chain ID, genesis hash,
`networkFingerprint` and three seeds were compared on all four devices
(`C:\PoCol-Mainnet-ceremony\network.pocol.json` on the PC, `/opt/pocol-standalone/network.pocol.json` on each server)
and match.

## Founders

| Founder | Device | Miner ID (public) | Rewards go to |
| --- | --- | --- | --- |
| 1 | server 108.165.128.89, Ubuntu 24.04.5, interface `ens3` | `0xc319eeced588afe03515508bab5f27805534b307` | `0x56bd8b1a42a6a3b09c04ebdd91e6fa3cc944b6bd` (owner-held) |
| 2 | server 108.165.128.90, Ubuntu 24.04.5, interface `ens3` | `0x4479d23cb5580b912a8402230071b8a7f0a701ec` | `0xbf335311d1a3ffc8c0efd1dc422f727f48e6de11` (owner-held) |
| 3 | server 108.165.128.91, Ubuntu 24.04.5, interface `eth0` | `0xe45343ea8d6d54fb693a6ad424fac11d85624b6e` | `0x4ce8b7625efc6898e59d7436c3a5a7569e91b3fc` (owner-held) |
| 4 | the owner's Windows PC, PoCol Wallet 0.5.0 | `0x15df38aaba05b83fb8c58d2dd9d804654278a96f` | its own miner ID (its id is its reward address) |

The reward keys of founders 1-3 are held by the owner with encrypted offline backups (as reported). The server miner
keys hold no balance; founder 4's key controls its membership, bond, allocation and rewards
([RECOVERY.md:25-26](RECOVERY.md)). At genesis all four founders are the owner's (F-34).

## The ceremony

**Pre-checks reported by the owner:** no earlier PoCol on the servers; NTP synchronized, sudo, SSH and host-key
fingerprints checked; the Windows and Linux packages compatible and the ceremony file ready; no
`%LOCALAPPDATA%\PoCol` on the PC; the sum of the hash-rate measurements 8,403,430; encrypted backups of the reward
keys made; an earlier dry run on chain 777990 passed; the Windows clock within 0.5 s; sleep and hibernate disabled
and the PC on mains power; the disk monitor's tests passed.

**Run:** from `C:\PoCol-Mainnet-ceremony`, `Initialize-PoColNetwork.ps1 -Network Mainnet` with the three servers,
their three reward addresses above, `-HashesPerSecond 8403430`, `-LaunchDelaySeconds 900` and the Linux archive
`PoCol-Mainnet-0.5.0-linux-x86_64.tar.gz`.

**What it did, as reported:** it installed PoCol on the three servers and made their identities; made the real
genesis and the invitation, checking the invitation's SHA-256 while distributing it; opened TCP 30300 in UFW; started
`pocol-standalone.service` on the three servers (all `active (running)`); and opened the Windows wallet. RPC on the
servers is `127.0.0.1:8545`. In the script these are: the invitation packed with one `--seed IP:30300` per server
([Initialize-PoColNetwork.ps1:349-351](../../deploy/standalone/Initialize-PoColNetwork.ps1)); on each server one SSH
command that checks the file's SHA-256, refuses an existing `/opt` copy, installs it, runs `sudo ufw allow 30300/tcp`
and starts the service ([Initialize-PoColNetwork.ps1:365](../../deploy/standalone/Initialize-PoColNetwork.ps1)); then
the wallet ([Initialize-PoColNetwork.ps1:380](../../deploy/standalone/Initialize-PoColNetwork.ps1)). The UFW rule
has no source restriction, and the script never enables UFW, so an inactive UFW filters nothing
([deploy/standalone/README.md:133-140](../../deploy/standalone/README.md)); whether UFW is active on the servers is
not reported. The node listens on 0.0.0.0:30300 with RPC on loopback only
([install-linux.sh:35](../../deploy/standalone/install-linux.sh)).

**Founders' bonds at genesis.** `init-testnet` refuses a genesis in which a founder is not a live HELD record with
the registration bond, or the Escrow does not hold all four bonds
([testnet.rs:448-467](../../prototypes/pocold/src/testnet.rs), called at
[testnet.rs:658](../../prototypes/pocold/src/testnet.rs)). So the four 20 PCOL bonds were in place at genesis; their
state after genesis has not been read on chain.

**Not yet recorded:** runbook step 6 asks each founder to check with `pocol_status` the chain ID, genesis hash,
`networkScope` public-network, `networkEnvironment` mainnet and the computed approval and readiness
([deploy/mainnet/README.md:90-94](../../deploy/mainnet/README.md)). The report quotes the ceremony release's values,
not each booted node's `pocol_status`. The Windows wallet runs its node without RPC
([wallet.rs:858](../../prototypes/pocold/src/wallet.rs)) and its status has no readiness or scope fields
([wallet.rs:583-592](../../prototypes/pocold/src/wallet.rs)), so founder 4's step-6 check is limited to the chain ID
and genesis hash the wallet shows. The step 4 record test on the filled profile
([deploy/mainnet/README.md:73-83](../../deploy/mainnet/README.md)) and the next daily disk report showing the PoCol
data directory ([deploy/mainnet/README.md:95-99](../../deploy/mainnet/README.md)) are not mentioned in the report.

## Post-genesis partition incident

**Symptoms (as reported).** After the start the servers could not reach each other: `compatiblePeers: 0` on some
servers, `[Errno 113] No route to host`, and ARP entries `108.165.128.89 FAILED` and `108.165.128.90 FAILED` on
founder 3, and the same on the others. Chain ID and genesis matched, but the servers grew three different branches.

**Cause (as diagnosed by the owner).** ARP fails between the servers inside `108.165.128.0/24`, while the gateway
`108.165.128.1` is reachable and its MAC address known: direct layer-2 reach between the servers is not available.
The likely explanation is isolation by the provider between its customers' servers in one subnet, with the gateway
still routing between them; the provider has not confirmed this.

**Three branches.** Block 16 before the fix:

| Server | Hash of block 16 |
| --- | --- |
| Founder 1 | `0x0f58ce235e39b00fea6656ed3976342734cb847459a2c3deb32facaa8254ab28` |
| Founder 2 | `0x15aab5dd636229f3a4e149c0f00cebabda1425baee632bb119bcea14e74ed275` |
| Founder 3 | `0x1e0b273888bc128d2bb84b5cdfff6dfa2edc20a4cb7f869b646e43018b4ab052` |

No branch was deleted by hand, no node's data was wiped and the genesis was not made again.

**Fix: /32 routes via the gateway.** Live `/32` routes via `108.165.128.1` were added, in the order F1-F3, F1-F2,
F2-F3. The final live set: on F1 (`ens3`) `108.165.128.90` and `108.165.128.91` via `108.165.128.1 dev ens3`; on F2
(`ens3`) `.89` and `.91` via `108.165.128.1`; on F3 (`eth0`) `.89` and `.90` via `108.165.128.1`. Checked with
`ip -4 route get` and `ip -4 route show table main`, with TCP tests passing. On F3 `RTNETLINK answers: File exists`
appeared and the route was replaced with `ip route replace`; an attempt with `eth0` on F1 failed because its
interface is `ens3`. The default route and UFW were not changed and no node was restarted. All six routes were
correct at the final checks.

**Reorganization.** After F3 was connected, block 16 on F1 changed to F3's branch hash; after F2 was connected, it
changed to F2's. The owner lists these as the matching reference hashes:

| Height | Hash |
| --- | --- |
| 16 | `0x15aab5dd636229f3a4e149c0f00cebabda1425baee632bb119bcea14e74ed275` |
| 40 | `0xfc2adf7fe03cfde8e18bae710ed59151c60096552d1119cf3c1b5642957d776b` |
| 60 | `0xdab7103bcde4e60c87c7a4c69ba02821c6ad0714d791a605e1c91eb8691a3c7d` |
| 120 | `0xd915ad257f1d1f2f6abc9d11981a6822dd893f8f7d9ca566e74f76a6816358cb` |
| 150 | `0x70a9710a738417f2d8d97a7dc740d99c0b9cbc2d497bf47c6f6e569cf14baf9d` |
| 240 | `0x63b14f60320de1bf9181f0e8e8794c2d056c58fa89ff8017820573ef35c76fb3` |

and states that the hashes at 150 and 240 match on F1, F2 and F3. The heights at the time of the check, read at
different times: F1 300, F2 282, F3 306.

**What this shows.** The kept block 16 is the one F2 had before the fix, so from the hashes F2's branch is the one
kept and the branches F1 and F3 had grown since a fork at or below block 16 were abandoned. Fork choice keeps the
branch with the most total work ([chain.rs:2251-2258](../../prototypes/pocold/src/chain.rs)); the reorganizations
were far inside the 2,048-block retention limit (R_KEEP,
[chain.rs:15](../../prototypes/pocold/src/chain.rs)), since every head was below 2,048. Up to block 240 at least,
the three servers were on one chain when the owner checked. Rewards and shares of the abandoned blocks exist only on
those branches; supply and the founders' bonds are not affected by a reorganization.

**What this does not show.**

- That the servers' later heads agree: the report says so itself. Height alone is not agreement (the owner's
  instruction 9); only hashes at a common height are.
- That founder 4 (the PC) is on the servers' chain: no block hash was compared on the PC, and its wallet shows no
  hash. Which server the PC's one connection goes to, and which branch the PC followed during the split, is not
  reported.
- That the routes survive a reboot: this is untested (below).
- How deep each server's reorganization was: `maxReorgDepth` and the reorg counters were not reported, and they
  count only since the node's last start ([REHEARSAL.md:86](REHEARSAL.md)).
- Any limit for a future split: a node whose own branch grows 2,048 blocks (about 5.7 h) past the fork no longer
  adopts the other branch by itself and needs operator recovery (F-09;
  [chain.rs:736-741](../../prototypes/pocold/src/chain.rs), [node.rs:1494](../../prototypes/pocold/src/node.rs)).

## Netplan persistence state

As reported: a new file `/etc/netplan/90-pocol-routes.yaml` was prepared on each server (mode 600, owner root); the
provider's `/etc/netplan/50-cloud-init.yaml` was not modified. On F1:

```yaml
network: {version: 2, ethernets: {ens3: {routes: [{to: 108.165.128.90/32, via: 108.165.128.1}, {to: 108.165.128.91/32, via: 108.165.128.1}]}}}
```

F2 has the same on `ens3` to `.89/32` and `.91/32`; F3 on `eth0` to `.89/32` and `.90/32`. `netplan get` and
`netplan generate` succeeded, the routes appeared in the systemd-networkd files, and the IP, gateway and DNS did not
change. A non-blocking warning about `gateway4` came from the provider's file. **Not done:** `netplan apply`,
`netplan try`, a restart of systemd-networkd, a reboot. **Whether the routes come back after a reboot is untested.**
Not established from the report: the ethernets id that `50-cloud-init.yaml` uses (the 90 file merges into the
provider's definition only if it is the same id, `ens3` or `eth0`), and whether exactly one generated networkd file
holds both the address and gateway and the two routes. Recorded as F-95.

## Monitoring state

As reported: the disk monitor's seven acceptance tests passed before the ceremony (#112, `1bb11f4c`; F-94). After
the install the owner set `POCOL_DATA_REQUIRED=1` in `/etc/pocol-disk-monitor/monitor.conf` on the three servers and
verified it with `--status`, a `systemctl start` of the service, `journalctl -n 12` and `systemctl is-active` of the
timer: on all three the data directory reads, the timer is active, with zero delivery errors and zero queued
messages, and no active alert. Data directory: F1 33.0 MB, F2 21.0 MB, F3 31.9 MB. Free space: F1 31.0 GB, F2
31.0 GB, F3 32.5 GB. One hour of growth is not enough for a rate; 24 h and 7 d measurements are needed.

`--status` shows `heartbeat: not configured` and `peer check: off`. The disk monitor measures disk; it does not see
a P2P outage, a stalled chain or a reorganization. No monitoring of node health (height progress, peers,
reorganizations) is in place. Recorded as F-97.

## Windows wallet state (founder 4)

As reported, the last view of the wallet: PoCol Wallet 0.5.0, chain ID 2656089850, block height 567, Direct Peers 1,
Mining Enabled Yes, Accepted Blocks This Session 107, Registration Bond 20 PCOL. What these fields are, from the code:

- **Direct Peers** is the number of open P2P connections ([Wallet.cs:63](../../prototypes/pocol-windows/Wallet.cs);
  [wallet.rs:585](../../prototypes/pocold/src/wallet.rs);
  [node.rs:3278](../../prototypes/pocold/src/node.rs)), not `compatiblePeers`. One connection is the designed steady
  state of a node without a listener, which keeps one seed after the first minute
  ([outbound.rs:535-537](../../prototypes/pocold/src/node/discovery/outbound.rs); OPERATIONS.md D7'a,
  [OPERATIONS.md:187-189](OPERATIONS.md)); the seed target has no /16 logic, so the owner's explanation (outbound
  IP-group diversity) is not the cause. Recorded as F-98.
- **Accepted Blocks This Session** counts blocks this node mined and imported locally
  ([node.rs:2913-2916](../../prototypes/pocold/src/node.rs); [wallet.rs:590](../../prototypes/pocold/src/wallet.rs);
  [Wallet.cs:139](../../prototypes/pocol-windows/Wallet.cs)). Nothing lowers it after a reorganization, so it is not a
  count of canonical blocks or rewards.
- **Registration Bond 20 PCOL** is the registration-bond target (`bond` in
  [wallet.rs:590](../../prototypes/pocold/src/wallet.rs)), not a reading of the held bond; the held bond is shown only
  while it is still being earned ([Wallet.cs:139-140](../../prototypes/pocol-windows/Wallet.cs)).

Founder 4's key has no encrypted backup yet (the owner's P0). Recorded as F-96.

## Open items

Every step below needs the owner's explicit approval; the owner runs it. In the owner's order of priority:

1. **P0, founder 4's key backup (F-96).** Check first that the chain progresses without the PC; with approval,
   disconnect the wallet, make the encrypted backup, check it and its address, store it offline, reconnect and switch
   mining back on (it starts off).
2. **P0, read-only stability checks.** On each server's `127.0.0.1:8545`: `pocol_status` (head height and hash,
   `compatiblePeers`, peers, the readiness fields of runbook step 6) and `pocol_finality`; block hashes at common
   recent heights on all three servers, read within the same minute; blocks mined by founder 4's ID in the servers'
   chain, which shows the PC's blocks are adopted; service, memory and disk state.
3. **P1, route persistence after a reboot (F-95).** First read-only: the provider's netplan id and the generated
   networkd files. Then a controlled test: provider console ready, one server at a time, state saved, checks after,
   and a rollback plan.
4. **P1, network health monitoring (F-97).**
5. **P1, the founders' keys** (server miner key backups are an owner decision; RECOVERY.md).
6. **P2, identity pin and signed release** (runbook step 7), updates of the clients and sites, publication of the
   invitation (step 8) and the remaining tests.
7. **P2, less dependence on the founder servers (F-98):** volunteer listeners in other /16s and a bootstrap list
   ([OPERATIONS.md:335-349](OPERATIONS.md)); stopping the seeds needs those prerequisites first, and the servers have
   not been stopped to test production without them.

The new ledger entries are F-95 to F-98 in [FINDINGS.md](FINDINGS.md).
