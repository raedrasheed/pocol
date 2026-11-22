# deploy/mainnet: launching the new PoCol network

PoCol launches a brand-new network from a fresh genesis
([owner decisions, 2026-11-13](../../development/decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)):
new keys, a new chain ID, four founding miners, and nothing carried over from the retired network. PoCol Mainnet
launches as a public network directly from its ceremony, without a separate rehearsal
([owner decisions, 2026-11-20](../../development/decisions/OWNER-DECISIONS-2026-11-20.md) D1). Nothing in this
directory deploys anything, and no step below runs on a server without the owner's approval.

## Launch runbook (stub)

This runbook is completed once the genesis parameters are fixed. The steps, in order:

1. **Ceremony packages.** Build the packages the ceremony runs on with `POCOL_RELEASE=1` from a pinned tree in CI
   (five-device-build). The mainnet ceremony runs on those CI artifacts, which the owner downloads
   from that CI run and checks against its `SHA256SUMS`: integrity only, not a release signature. No signed release
   exists yet: the assembler refuses every release until the identity switch (step 7), and the switch changes the
   native binaries (`network_display.rs`) as well as the extension version, so these packages are never published.
2. **Prepare the servers.** The three founder servers, `108.165.128.89`, `108.165.128.90` and `108.165.128.91`
   (D3; also the seeds), start from a clean install (`deploy/standalone/install-linux.sh` refuses a host that
   already has a PoCol install). Each has 4 GB of RAM, a risk the owner accepted (D2; 8 GB is recommended): that
   is below the unit's `MemoryMax=7G`, so if a node outgrows its host the kernel, not the unit, stops it and
   systemd restarts it, at most 5 times in 300 s ([FINDINGS.md](../../development/public-network/FINDINGS.md), F-23,
   F-93). Each keeps its 35 GB disk, a risk the owner accepted on the condition of tested monitoring
   ([owner decisions, 2026-11-21](../../development/decisions/OWNER-DECISIONS-2026-11-21.md) D7; F-94): the disk
   monitor of [deploy/monitoring](../monitoring/README.md) is installed on each server, with the owner's approval, at
   this step or at any time before step 8. It installs only its own files (`install-linux.sh` checks only
   `/opt/pocol-standalone`, its unit and its service user) and never touches PoCol's data, unit or firewall. The owner
   installed it on the three servers on 2026-11-21 and reports its acceptance test passed on each (D7, "Acceptance of
   D7's monitoring"); until PoCol is installed it reports the PoCol data directory as absent. The
   owner's computer runs the fourth founding miner; the phone is a wallet.
3. **Fresh keys.** Each founding miner generates its miner key on its own device: on each server `install-linux.sh`
   runs `pocol identity --home /var/lib/pocol-standalone` as the `pocol-standalone` user, and on the PC the ceremony
   runs `pocol.exe identity` in the wallet's home `%LOCALAPPDATA%\PoCol`. Only the public miner ID leaves the device.
   Founders 1-3 (the servers) are paid to owner-held reward addresses (owner decision 2026-11-16): the owner makes
   three of them offline with `pocol-key generate` and `pocol-key address` on a device that is never a server, and
   passes them to the ceremony as `-ServerRewardAddresses`. Each offline key is backed up encrypted with `pocol-key export --key-file FILE
   --out BACKUP` (passphrase read twice from stdin), and the backup checked without writing any file with
   `pocol-sign --chain-id ID --address --backup BACKUP`, which prints the same address; `pocol-sign --backup BACKUP`
   signs from it. Only the 20 PCOL bond is tied to each server. Founder 4 (the owner's PC) keeps its own
   address. Each reward account starts at nonce 1 (PAY-02): its first offline transaction uses `pocol-sign --nonce 1`.
4. **Genesis.** Start from the committed profile, `v3-profile.json` (`v3-rehearsal-profile.json` is only a test and
   devnet profile, never a launch step).
   It holds the chain ID and the consensus parameters recorded in
   [FRESH-NETWORK-CP.md](../../development/decisions/FRESH-NETWORK-CP.md), and the four founders' allocations
   (999,980 PCOL each, plus the 20 PCOL HELD bond minted into Escrow). Fill its `PLACEHOLDER` values: the launch
   second, the founders' measured aggregate hash rate (`pocol hashrate` on the four founder devices), each founder's
   public miner ID and reward address (the
   owner-held ones for founders 1-3). Then
   run `pocold init-testnet --config PROFILE.json --out NEW_DIR` once, and record the printed chain ID and genesis
   hash. Its summary reports the admission (scope public-network, the network profile on a network node, never
   specification compliance, with its conditions and the owner's waivers) and, since no release pins the new genesis
   yet, `publicLaunchApproved` false with `network-not-release-pinned` and `launch-not-approved`. While any
   placeholder remains, the builder refuses. `--test-placeholders` fills them with public test keys
   for a local test genesis only; that genesis runs on the local test chain ID 777990 (the profile's ID is recorded
   as `profileChainId`), is marked `testPlaceholders` and is never a launch genesis.
   The hash rate is measured before the genesis, on each founding device with its own copy of the release binary,
   since the rate depends on the CPU (the proof-of-work hash is one SHA-256 of 40 bytes):
   `pocol hashrate --seconds 60` on each server, run from the verified Linux package extracted into a new folder in the
   admin's home (check it with `sha256sum --check SHA256SUMS` there, and delete the folder afterwards:
   `install-linux.sh` checks only `/opt/pocol-standalone`, its unit and its service user), and `.\pocol.exe hashrate --seconds 60` on the PC with
   the wallet closed. Run it three times on each device and take the median. It measures the one miner thread that
   `pocol start` and the wallet mine with (neither sets `--mine-threads`), and opens no home, key or network. The
   ceremony's `-HashesPerSecond` is the sum of the four medians, since every founder hashes its own nonce range in
   parallel; the PC's share of the sum is the hashrate fraction T7 compares with
   ([REHEARSAL.md](../../development/public-network/REHEARSAL.md)). The value sets only the starting target (target_g
   = calibration_target(H, T_blk)): ASERT is anchored at genesis and corrects a wrong value with a time constant of
   about τ/(T_blk·ln 2) ≈ 87 blocks. An estimate from a simple model, not a test in this repository: with the value k
   times the real rate, the first 100 blocks average about 8.7 s at k = 0.8 and 11.4 s at k = 1.25, stay inside T6's
   5-20 s from about k = 0.5 to k = 4, and miss it at k = 8, whose first blocks take about 80 s, close to D_att. A
   benchmark slightly overstates the network's rate (nobody hashes while a template is built), which gives a few slow
   blocks at the start.
   The late-start check on the final CP (FRESH-NETWORK-CP.md §8, ceremony step 3) is
   `prototypes/pocold/tests/launch_delay_asert.rs`. CI runs it at eight rates spanning the ceremony's admissible range
   (1 to 2^53-1 H/s), not at every rate and not at the measured one. Its ignored `ceremony_filled_profile_record` runs
   it on the filled profile (`<setup folder>\v3-profile.json`, public values only),
   from a development checkout at the release commit, never on a founding device:
   `POCOL_FILLED_PROFILE=<absolute path of the filled profile> cargo +1.99.1 test --release --locked --manifest-path
   prototypes/pocold/Cargo.toml --test launch_delay_asert -- --ignored --nocapture` (cargo runs the test from
   `prototypes/pocold`, so a relative path is not found). Keep its printed collapse
   threshold and near-free burst with the ceremony record. `Initialize-PoColNetwork.ps1` fills the profile and runs
   `init-testnet` in one run, so this run comes after the genesis: it is a record, not a gate. Making it a gate needs a pause in the ceremony before `init-testnet`, which
   is the owner's decision (proposal, not decided).
5. **Invitation.** The ceremony packs the genesis files with the three seed addresses into the network invitation
   (`pocol network`) and prints its file SHA-256. Each server checks its copy against that value before installing it.
   That file SHA-256 (`Get-FileHash -Algorithm SHA256` on Windows, `sha256sum` on Linux) is the value every founding
   device compares and the owner announces. `pocol inspect --network FILE` prints `networkFingerprint`, a different
   value: the SHA-256 of the parsed invitation, which reformatting the JSON leaves unchanged. It is not the file's
   SHA-256.
6. **Founders boot on the genesis.** The four founding devices, the three servers and the owner's PC (founder 4) as
   `Initialize-PoColNetwork.ps1` makes them, start from the invitation; this genesis is the launch genesis (no
   rehearsal, no wipe and repeat). Each founder checks with `pocol_status` that its node reports
   the recorded chain ID and genesis hash, `networkScope` public-network and `networkEnvironment` mainnet, and the
   computed approval and readiness, unchanged by any label. Then, with the owner's approval, on each of the three
   servers once PoCol is installed there (D7, the owner's requirement): `sudoedit /etc/pocol-disk-monitor/monitor.conf`
   and set `POCOL_DATA_REQUIRED=1`, then check that
   `sudo python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py --status` reports the PoCol data directory
   `/var/lib/pocol-standalone` present, with its size (not `not present`), and that the next daily report shows it.
   From then on a missing or unreadable data directory raises a CRITICAL alert.
7. **Pin the identity in the clients.** Switch every client from the identity the tree pins today to the new
   one with `set_network_identity.py` (below), in one reviewed PR, and release the clients: rebuild the native
   packages and the 0.5.0 extension from the switched tree, then assemble, sign and verify them
   (`deploy/release/README.md`, owner procedure). This is the first signed release; no package is signed before it.
   The switch writes the release pin with the approval reference "OWNER-DECISIONS-2026-11-20 D1", so a node of this
   release booted on PoCol Mainnet reports `publicLaunchApproved` and `publicLaunchReady` true while its
   `technicalReadiness` still lists what is open. The same PR publishes on the
   pocol.net extension and download pages the two values users compare: the extension ID
   `gggcoflmedlhlkcdhebmoepmggdnafmp` (`prototypes/pocol-extension/release/manifest-key.pub`) and the release key
   fingerprint `SHA256:FlrCFc4LXIOOPMrT2g4dgV6CuOEiw1L30NWKzGPBZt8` (`deploy/release/allowed_signers`).
8. **Public launch.** Before the network is opened to the public, the disk monitor is active on the three founder
   servers and a real test notification from each has been received (D7 of 2026-11-21): on each server
   `--test-alert` delivered and received on every configured channel, a `--simulate-usage 96` alert received,
   `systemctl list-timers pocol-disk-monitor.timer` listing the timer and a forced failure reported
   ([deploy/monitoring/README.md](../monitoring/README.md), "Acceptance test"). The owner reports this passed on all
   three servers on 2026-11-21 and approves that D7's condition is met. The monitor's post-install step of step 6
   (`POCOL_DATA_REQUIRED=1`, the PoCol data directory reported present) remains: it follows PoCol's install by the
   ceremony (step 3) and the founders' boot (step 6). At the journal's worst case, about 1.7 GB a day, a 35 GB disk
   fills in about three weeks; the monitor warns 30, 7 and 2 days ahead, and a disk expansion is a separate owner
   decision. Then publish the invitation on pocol.net, signed with the release key like every download
   (`deploy/release`), and open the seeds. The launch evidence
   ([REHEARSAL.md](../../development/public-network/REHEARSAL.md)) is measured on PoCol Mainnet after the launch.

## Tools

- `set_network_identity.py --from OLD.json --to NEW.json [--expect-sha256 HEX --expect-chain-id N --expect-genesis 0xHEX] [--check]`:
  switches every client's pinned chain ID,
  genesis hash and GenesisSpec (`FILES` lists them) from one network's invitation to another's, with the
  derived content versions and the Studio gateway installer. It names no network itself: both identities come
  from the invitations, proven by their GenesisSpec. The invitation of the identity the tree pins today, which
  is no longer in the tree, is taken from git history:

  ```sh
  git show <commit>:deploy/mainnet/h15000/network.pocol.json > /tmp/OLD.json
  python3 deploy/mainnet/set_network_identity.py --from /tmp/OLD.json --to NEW_DIR/network.pocol.json --expect-...
  python3 deploy/mainnet/set_network_identity.py --from /tmp/OLD.json --to NEW_DIR/network.pocol.json --expect-... --check
  ```

  `<commit>` is any commit before that file was deleted (`git log --diff-filter=D -- deploy/mainnet/h15000`
  names the deleting commit; use its parent). The tool prints the identity it switches from and to before it
  writes anything. On a public network chain (`PUBLIC_NETWORK_CHAINS`, the chain of `v3-profile.json`,
  2656089850) `--to` needs the three published values, `--expect-sha256` (the invitation file's announced
  SHA-256), `--expect-chain-id` and `--expect-genesis`, and refuses, writing nothing, a file that does not match
  all three (a value given on another chain must match too); `--check` needs them as well, and
  `deploy/websites/tools/site_identity.py --to` takes the same three. A spec-2 genesis must have CP nonceMode 1,
  which the release node and the extension require. It also writes the release's pinned launch network, `NETWORK_PROFILES` in
  `prototypes/pocold/src/readiness.rs` (V3-28 H2): the `--to` entry replaces the `--from` one, unless `--to` is a
  retired network or the rehearsal network (the chain ID of `v3-rehearsal-profile.json`), which get no entry, and
  `--check` refuses a missing or extra entry, or an entry of the other chain's form. Each entry is written on the
  declaration's one line, entries separated by `,` (`HEX` the 64 lower-case hex digits of the genesis hash, decoded at
  compile time). On a public network chain (`PUBLIC_NETWORK_CHAINS` of the profile code, the mainnet profile's chain,
  which a test keeps equal to `v3-profile.json`) the entry is
  `NetworkProfile{chain_id:CHAIN,genesis_hash:genesis_hash("HEX"),admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")}`:
  the public network's admission (scope public-network, the network profile on a network node, with the conditions and
  the owner's R13, R21 and R26 waivers its boot reports; `pocold`'s `tests/fn4_profiles.rs` checks it against a boot)
  and the owner's launch approval (D1). On any other allowed network it is
  `NetworkProfile{chain_id:CHAIN,genesis_hash:genesis_hash("HEX"),admission:ExpectedAdmission::FRESH_NETWORK,approval:None}`
  (the experimental admission, which approves nothing and removes only `network-not-release-pinned`). The tool never
  writes `ExpectedAdmission::LAUNCH` (scope specification, no condition, no waiver). A booted node of the pinned
  network reports `publicLaunchApproved` true only when its booted admission matches the pin exactly (otherwise
  `admission-mismatch`, naming the fields that differ), and its `technicalReadiness` still lists `admission-scope`,
  each `admission-condition-*` and `admission-waiver-*`, and every open gate. The launcher boots nothing: it reports
  the pin's approval reference and keeps `admission-not-booted`. Since step 7 the tree pins PoCol Mainnet (chain
  2656089850; its genesis hash is in `prototypes/pocold/src/network_display.rs`, which `set_network_identity.py` lists),
  so the list holds its one public-network entry; every other network reports `network-not-release-pinned` and `launch-not-approved` (and, in
  the launcher, `admission-not-booted`). Live nodes report the pin only once they run a release built from the switched
  tree. The report is node-local: it never stops a node from running, mining or serving sites.
- `v3-profile.json`, `v3-rehearsal-profile.json`: the fresh network's mainnet profile and the rehearsal profile,
  which is kept only as a test and devnet profile (FN-4; OWNER-DECISIONS-2026-11-20). They differ only in the chain ID
  and the launch second. `test_v3_profiles.py` checks them; `pocold`'s
  `tests/fn4_profiles.rs` builds and admits them.
- `retired-networks.json`: the identities the clients were switched away from, append-only. Every switch adds
  its `--from` network, and the tool refuses a listed network as `--to` (swapped `--from`/`--to` included):
  a rollback is a `git revert` of the switch commit, not a reversed run. `--allow-revert` switches back to a
  retired network on purpose (step 7 never uses it). Its first entry is the network retired on 2026-11-13, which the
  clients pinned until step 7 switched them to PoCol Mainnet.

The tests (`test_set_network_identity.py`) also guard the tree: every tracked file that pins the current
identity is in `FILES` or kept on purpose; once the tree pins a network that is not retired, no tracked file
outside `development/` and `deploy/websites/` may hold a retired identity (`FILES` included); and the shipped
release texts that are true only on the retired network (`STEP3`: the 0.4.x versions and "chain ID and genesis
unchanged" notes) must be gone by then. The native 0.5.0 bump (owner decision 2026-11-16) landed before the switch;
what `STEP3` still lists (the extension and Studio versions and the 0.4.x extension download names) moves with the
signed 0.5.0 extension release and its download links. Files outside `FILES` that name the public network's chain ID
on purpose (its profile code, the ceremony, the faucet, the release build check, their tests and this runbook) are
listed in `MAINNET_NAMED` of the test with a reason; a test requires exactly those, so a new mention is reviewed there,
and once the tree pins that chain their mentions are not taken for unlisted client pins (a retired identity in them is
still refused). `deploy/release/assemble_release.py` refuses a release whose
extension `package.json` version is not the native version, so no release is assembled before the switch sets it to
0.5.0: the ceremony runs on unsigned CI artifacts (step 1), and the first signed release follows the switch (step 7).

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
the switch stays one run. The extension pages (`publisher.html`, `contract.html`) name no network: their chain ID chip
is filled from the built-in profile at run time. The extension READMEs keep the tree's identity until the switch.

## Local checks

```sh
python3 -B -m unittest discover -s deploy/mainnet -v
```
