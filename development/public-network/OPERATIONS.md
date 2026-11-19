# Public-network compatibility and operations

This guide covers how PoCol Mainnet nodes peer, discover each other, take upgrades and bound their memory, and how
the operator seeds are run and stopped. The network is a fresh v3 network (consensus rules 2, a version-2
`GenesisSpec` with nonceMode 1; [FRESH-NETWORK-CP.md](../decisions/FRESH-NETWORK-CP.md)). Nothing in this guide
launches or changes a server by itself. The launch gates that are still open are in [PLAN.md](PLAN.md); servers alone
do not close them.

## Compatibility and hostile input

Wire v2 carries `protocolVersion` (the sender's maximum wire version) and a 32-byte
**consensus identity** in the Hello (VA-SPEC §2). Two nodes peer only when they run the
same consensus rules on the same chain with the same fork schedule: the identity is
SHA-256 over the chain id, genesis hash, `CONSENSUS_RULES_VERSION`, the caps-test and
unpinned flags and the full fork schedule, so anyone can compute it from devnet.json
(`deploy/public-testnet/pocol_identity.py`). A peer is accepted iff its chain and
genesis are equal, its maximum wire version is at least `PROTOCOL_VERSION_MIN` (2),
and its identity is equal. Old 9-item handshakes, other wire ranges and other
identities fail closed, on first contact and later status updates. A rejected
connection is removed immediately; already queued frames and a second valid-looking
Hello cannot reauthorize it. Only admitted peers receive gossip. The first frame is
limited to 512 bytes; peer names are 1..64 printable ASCII bytes without spaces or
control codes.

`pocold build-info` still exposes a reproducible source fingerprint
(`compatibilitySha256`), and `pocol_status` / `pocol_getParams` expose it. It is now
**informational**: install-integrity status for operators, never a peering condition.
It commits to the UTF-8 path and raw bytes of all Rust source files in the three
crates, their Cargo manifests, the pocold lockfile/build script, embedded
system/website artifacts and the caps/crash/I/O-fault feature flags, in bytewise path
order with 64-bit big-endian lengths, SHA-256 and domain
`pocol-testnet-source-compatibility-v1\0`. It excludes Git commit metadata, absolute
build paths, timestamps and secrets. Raw text requires a release checkout with
preserved line endings.

The consensus-critical sources are pinned instead by `pocold/consensus-sources.lock`
(VA-SPEC §2.4). A change to one of them needs a reviewed, labelled history line; a
build of a tree that differs from the lock is **unpinned**: its identity carries the
tree's source aggregate, so it peers only with byte-identical trees, and a
`POCOL_RELEASE=1` (packaging) build of it fails. Build-info reports `consensusPinned`,
`consensusSourcesSha256`, `consensusRulesVersion`, `protocolVersionMin` and
`maxForkVersion`. The identity is self-declared: it is not peer authentication or
remote attestation, and a malicious peer can advertise any value. Block validity,
economics, H_END and the existing production-admission refusals are unchanged; block
validation stays authoritative.

Inspection also found an unconditional loopback-source filter in the P2P listener.
It prevented the older private-server package from accepting inter-host connections;
its loopback rehearsal could not detect that. The listener now honors the explicitly
configured bind address and accepts remote TCP peers through the same bounded
handshake. Native RPC remains loopback-only. A required CI control connects through
the runner's own non-loopback interface, without changing routes or contacting an
external host. That is narrower evidence than an actual multi-host trial.

## Operations implementation

`deploy/public-testnet/` contains a public-address inventory validator, one-genesis
packager, independent-key Linux service, checksum-gated fresh installer, offline
backup/restore, cross-node progress/root monitoring and a dedicated TLS gateway
configuration. `--join` prepares a new observer or independent miner with published
seeds. Secret keys stay outside bundles/state; systemd credentials expose only the
host's key to its service user. State binds to network, release, node and miner.

Backups hold both the operator and Rust journal locks, reject active processes and
links, checksum/fsync the complete stopped state and restore only into a new path.
The gateway has per-IP and global request/connection limits, fixed body/time limits,
TLS verification prerequisites and preserved Origin. It does not fix unbounded
precompile cancellation or complete RpcGuard memory accounting. Monitoring checks
compatible peers, healthy history, finite-lifetime proximity and common confirmed
hash/state/receipt roots while all nodes advance.

## Peer discovery

A node learns peer addresses by asking, never by being told (VA-SPEC §1a; local
policy, which never changes validity, chain state or admission). Hello now has 11
items: `listenPort` and an 8-byte `nonce` follow the consensus identity (VA-SPEC §2.2). `PROTOCOL_VERSION` stays 2, and a 9-item
Hello fails closed. A node sends GetAddr (type 8) once on each connection it dialed, and
answers the first GetAddr of each inbound connection with one Addr (type 9) of at
most 64 addresses. It learns only the addresses it dialed and verified, the source
IP and `listenPort` of its inbound peers, and the answer to its own GetAddr. Nothing
is relayed further, and no peer is banned. With automatic outbound (on by default) a
node keeps 4 connections (2 without a listener) to book addresses in distinct address
groups, plus a spare slot: a short feeler every 2 or 10 minutes, or an extra connection
while the node is stalled.

| Setting | `pocold run` | Launcher (`pocol start`, wallet, mobile) |
|---|---|---|
| Discovery on (default) or off | `--no-discovery`; `POCOL_DISCOVERY=0` or `1` | `POCOL_DISCOVERY` |
| Local mode (off by default) | `--discover-local`; `POCOL_DISCOVER_LOCAL=0` or `1` | `POCOL_DISCOVER_LOCAL` |
| Automatic outbound (default `auto`: 4 with a listener, 2 without; 0..8) | `--outbound N\|auto`; `POCOL_OUTBOUND` | `POCOL_OUTBOUND` (wallet and mobile: auto = 2) |
| Seeds (at most 7) | `--seed ADDR`, repeated; a seed equal to a `--peer` stays static; refused with `--no-discovery` | the invitation's seeds, none once it sets `seedsRetired` |
| Static peers | `--peer ADDR`, repeated | `pocol start --peer IP:PORT`, repeated (at most 7) |
| Bootstrap addresses (at most 16, ports >= 1024) | `--bootstrap ADDR`, repeated | the invitation's optional `bootstrap` |

A flag wins over its variable, and an empty value counts as unset. Any other value
stops the command before the node starts (status 1), naming the variable. Local
mode also learns loopback and private addresses, but only from peers that have no
public address either. It is meant for single-host and LAN rehearsals.
`pocold status` reads the flags and ignores the variables.

**Escape hatch.** `POCOL_DISCOVERY=0` (or `--no-discovery`) turns discovery off. The
node then sends `listenPort` 0, asks nothing, answers nothing, and neither reads nor
writes the address book. A listening node that comes back this way withdraws its
advertisement: at its next connection, a peer forgets the self-advertised address at
its IP, unless another live connection from that IP still advertises it. Two protections stay on: the self-connection nonce check,
and clearing a configured peer's redial by the address that was dialed.

**The address book.** The book is the file `addrbook` in the node's data directory
(`pocold run --data DIR`, which is `/var/lib/pocol-testnet` for the Linux service,
or the launcher home's `state/`). It is bound to the chain id and genesis, and it is
checksummed. The node reads it when the network starts and writes it atomically 60
s after the start, every 15 minutes, and at a clean stop, each time only if it
changed, and at once when its anchors change. `pocold run` and the launcher stop cleanly on SIGINT and SIGTERM (so
`systemctl stop` saves the book and the anchors); a killed node keeps its last save.
The file is advisory: deleting it only makes the node start with an empty book. It
may be backed up with the rest of the data directory. In local mode, never copy it
to another node, because its local addresses mean something only on the host or LAN
where they were learned. A damaged, foreign or oversized file is ignored: the node
starts with an empty book, logs the reason, and shows it as
`discovery.book.file: "ignored: <reason>"`.

**Monitoring.** `pocol_status` gains:
- `outboundPeers`: live connections this node dialed;
- `discovery`: the settings, `listenPort`, and `selfDetection` (the node listens, so
  it sends a nonce);
- `discovery.book`: the size of the book (entries, verified, relayable, address
  groups, the maximum, and `file`), never its contents;
- `discovery.self`: this node's own addresses (at most 16);
- `discovery.counters`.

`compatiblePeers` no longer counts a connection to the node itself. A seed whose
only peer is itself therefore shows 0, and monitor.py can see that
it is isolated. Watch `compatiblePeers`, `discovery.self`, the `selfConnections`
counter and `bookSaveErrors`. Automatic outbound adds `discovery.outbound`:
`automatic`, `healthy` (useful peers in distinct groups; >= 2 on a listener), `stalled`
(the head's timestamp older than `windowSeconds`), `quiet`, `starved`, `seedTarget`,
`seeds[].state` (connected, dialing, suppressed, held, self), `slots[]` (role, kind,
state, address, group, uptime, failures), `anchors`, `familyDown`, `seedNode`,
`seedsRetired` and `bootstrap`; and the counters `dials`, `dialFailed`, `dialNoHello`,
`dialBusy`, `dialIncompatible`, `dialNoRoute`, `slotsStale`, `feelers`, `feelersOk`,
`feelersIncompatible`, `anchorsDialed`, `extras`, `extraSwaps`, `extraClosed`,
`extraStarved`, `seedWants`, `seedReleases`, `seedRotations`, `seedHolds`, `stalls`,
`quiets` and `bootstrapLoaded`. Watch `healthy`, `stalled`, `seedTarget` and
`dialFailed` against `dials`. A feeler is never in `peers` or `compatiblePeers`;
`outboundPeers` counts slot connections, the feeler's included.

**Privacy.** Only a listening node is advertised, and only by its own consent: a
non-zero `listenPort` in its Hello. A node that does not listen sends `listenPort` 0
and nonce 0, so nothing in its Hello names it or links it across addresses. A node
listening on loopback (`--listen 127.0.0.1:P`) sends both only to loopback peers, so
a remote seed it dials does not record its public IP as a listener; one listening on
a specific address sends them only on connections that use that address. An
inbound peer is recorded under its connection's source IP, never under an address it
claims. A peer that dials this node may receive up to 64 addresses that the node
verified or was advertised within the last 7 days. It receives at most 2 per address
group, never addresses the node only heard of, and never timestamps.

**Automatic outbound connections (VA-SPEC §1a.7-§1a.9).** Each slot holds one
connection at a time, at most one per address group, never to a configured peer, a
seed, this node, an inbound peer's advertised address or an address already
connected. Choice: verified or unverified entries 50/50 when both can be dialed, in
each by source first (all self-advertisements one source), then group, then fewest
failures; an unverified entry is dialed again after 10 minutes, a verified one after 1
minute, and a slot waits 1 s doubling to 60 s after failures. A connection that never
sends a Hello is a failure of an unverified entry, so squatted entries become
replaceable. A configured peer's dialer also backs off, to one connect per 30 s.
The feeler sends nothing, reads the peer's Hello, verifies or forgets the address and
closes, so the peer evicts nobody for it. Stalled (the head's timestamp older than W =
max(120 s, 12 blocks)) or quiet (no block from a peer moved the head for 4W), the spare
becomes an extra full connection to a new source (bootstrap addresses first once no
seed can be wanted); if it moves the head it replaces the slot useful longest ago (never
the sync peer). Anchors: up to 2 automatic peers connected 10 minutes and useful within
W, saved with the book and dialed first after a restart. Seeds bootstrap and refill
only: at most 2 wanted, a dead one held 1 minute doubling to 6 hours (while this host
itself has no route and no peer, 1 minute only; a failure doubles the hold only after this
node verified some other address since the last one, so an upstream outage behind a working
router leaves the holds short; holds clear once it is back online); a listening node
releases its seed 2 minutes after 2 useful peers hold, a node without a listener keeps
one (owner decision D7'a). A seed host (its exact `--listen`, or a local listener on the
seed's port) keeps all other seeds. Configured peers plus slots (the spare included)
are at most 18 (network.md:157, D14'): `auto` is lowered to fit and an explicit
`--outbound` past it is refused at boot. Cost at the defaults: +27,517,640 B of model
(5 slot places and one forwarder) and +16 tasks (144 of TasksMax 256).
`POCOL_OUTBOUND=0` restores PR2a behaviour (seeds are static peers, with the backoff).

**Upgrades (G2, G3).** G2 is closed: peering no longer depends on the build digest,
so a release that changes only the digest (no consensus rule, no schedule) interoperates
with the previous one, and seeds and wallets may take it in any order (staggered
rollouts). Launcher homes take it too: `pocol network` writes `pocol-network-v2`
invitations bound to `consensusRulesVersion`, not to a build digest, so their
fingerprint is stable across such releases, and `pocol upgrade` appends future fork
entries of any supported version (append-only, future-only) or rebinds to an
equivalent v2 file. `pocol inspect-upgrade` shows the effect first and writes nothing.
G3 (a fork activation) still needs a coordinated release, announced (signed) at least 24 h, 8,640 blocks at
T_blk 10, before its activation height (governance.md, Upgrades; owner decision 2026-11-16): a node without the new
schedule entry is refused by the others with an explicit reason. A seed bundle of
`deploy/public-testnet` (`node_operator.py` refuses a state marker whose `consensusIdentity`
differs) takes the activation with the seed fork-migration tool
`deploy/public-testnet/nonce_upgrade.py` (`deploy/public-testnet/README.md`): `prepare` builds a
new bundle with the published invitation's devnet.json (only the appended fork entries and, with a
fork-3 entry, its `capRaise` may differ) and the new release, and `migrate`, on the stopped seed,
moves only its state binding to the new consensus identity, with a backup and a resumable intent.
`migrate` refuses an appended activation that is not above every stored block, or that is fewer
than 8,640 blocks (24 h at T_blk 10) above the seed's highest stored block. The tool applies the
published schedule append (VA-SPEC §2.3) to the seed's configuration and changes no consensus rule
and no chain data; `test_nonce_upgrade.py` tests it (CI: public-testnet-operations). A seed run
from a launcher home takes the activation with `pocol upgrade` like any home.

**Activating the cap raise (FN-5, C10; rehearsal S2).** The raise of ACCT_CAP, SLOT_CAP, CODE_CAP and SYS_SLOT_MAX is
compiled into every release from FN-5 on (MAX_FORK_VERSION 3) and dormant until a schedule names fork version 3. It
exists only on a v3 (spec-2) genesis. To activate it:
1. **Build.** Any release with `maxForkVersion` 3 in `pocold build-info` (rehearsal S2: the `caps-test` build, whose
   lowered caps let the fixture fill ACCT_CAP quickly). The off-node verifiers must be releases that accept fork
   version 3 too, before the schedule is published: the extension (`forks.js` MAX_FORK_VERSION 3), the reward gateway
   (`pocol_rewards.py` MAX_FORK_VERSION 3) and the identity tool (`pocol_identity.py`, which takes the bound from
   build-info). A verifier with bound 2 fails closed as soon as the node advertises the new schedule, not only from H.
2. **devnet.json.** Append two entries to `forks` and add `capRaise` beside it, nothing else:
   `"forks": [[1,0],[2,H2],[3,H]]`, `"capRaise": {"ACCT_CAP": a, "SLOT_CAP": s, "CODE_CAP": c, "SYS_SLOT_MAX": m}`
   (JSON integers, all four; SYS_SLOT_MAX within u32). (2,H2) comes first because versions are never skipped; under
   nonceMode 1 it is a no-op. Choose H2 < H, both at least 17,280 blocks above every home's and seed's stored head when
   the signed announcement and the invitation are published: the 8,640-block minimum notice (24 h at T_blk 10) plus a
   day of blocks for every seed, home and gateway to update. H2 needs this margin as well as H, because a seed bundle's
   `nonce_upgrade.py migrate` refuses once fewer than 8,640 blocks remain before the first appended activation (H2). Each value must be at least the genesis value and at least one must be higher; ParamGate (R1-R26 at the
   raised CP) refuses anything else at boot, and `pocol network --from` refuses it before it writes.
3. **Publish.** `pocol network --from <network dir> --seed … [--bootstrap …] --out network.pocol.json`, then publish
   the file and its fingerprint through the same trusted channel as the genesis, with H2, H and the four values.
4. **Homes.** `pocol inspect-upgrade --network …` and compare with the published values: `change` "activation",
   `appended` `[[2,H2],[3,H]]`, `capRaise` (the four caps and `height` H), `forkSchedule`, `consensusIdentity` and
   `fingerprintAfter`. Then `pocol upgrade --network …`. A scheduled raise can never be changed or removed afterwards.
5. **Seeds and gateways.** A seed run from a launcher home upgrades as in step 4. A `deploy/public-testnet` seed
   bundle: `nonce_upgrade.py prepare` with the published file and the release, compare its `consensusIdentity` and
   `appended` with the published values, then stop the seed, run `nonce_upgrade.py migrate`, install the new bundle
   and start it (`deploy/public-testnet/README.md`). Run `migrate` while the seed's highest stored block is at most
   H2 - 8,640; above that it refuses (with the margin of step 2 that leaves about a day after publication). A seed
   that missed this point cannot use the tool, and a scheduled entry can never be moved: start the new bundle on a
   new, empty state directory (keeping the old one aside), so that it binds the new identity and syncs the chain from
   its peers. Set the reward gateway's trusted
   `--fork-schedule` file to
   `[[1,0],[2,H2],[3,H]]` when its node is upgraded (until both match it answers "differs from trusted
   configuration"). Wallets add the same schedule to their local profile (`[[1,"0"],[2,"H2"],[3,"H"]]`) before H;
   one left on the old schedule refuses headers from H on (`netVersion`).
6. **Check.** Before H, `pocol_getCaps` reports `capRaise` with `active` false and the genesis caps; from the block
   whose child is H (head H-1) it reports `active` true and `accountsCap`, `storageSlotsCap` and `codeBytesCap` equal
   to the raised values; `pocol_getParams` reports `capRaise` and `maxForkVersion` 3, and every node reports the same
   `consensusIdentity`. A node left without the raise agrees on every block below H and is refused from H.
7. **Rehearsal S2.** On the cap-test build: fill ACCT_CAP; a transfer that creates an account fails (the pool refuses
   it with `AcctCap`), so a newcomer cannot be funded to `register`; activate as above; from H both succeed; restart a node and
   reorg across H (`pocold/tests/fn5_cap_raise.rs` is the in-process version of these checks) and compare the heads,
   state roots and `consensusIdentity` on every node.

**Reading the status fields.** `pocol_status` (and wallet status) report:
- `consensusIdentity`, `consensusRulesVersion`, `maxForkVersion`, `protocolVersion`
  (maximum), `protocolVersionMin` and `consensusPinned` (false: a development build
  that peers only with byte-identical trees);
- `incompatiblePeers{network, wire, consensus}`: refused Hellos by reason. `network`
  is another chain or genesis (the address is forgotten); `wire` a wire version below
  the minimum; `consensus` another identity (both hold the address);
- `lastIncompatibleReason`: the last refusal, `peer <name>: <reason>`, at most 160
  characters, for example "peer has not scheduled activation (2,H): it must apply the
  published network upgrade" or "peer runs consensus rules 3; this node runs rules 2";
- `publicLaunchReady` and `launchBlockers` [{id, detail}] (V3-28 H1,
  `prototypes/pocold/src/readiness.rs`): computed from this build and the admission the
  node booted with, true only with no blocker. Today it is false, for example
  `plan-gate-Q1`, `network-not-release-pinned`, `admission-scope` and `profile-kind`
  (a booted node runs a v3 genesis, since admission refuses spec 1, so it never reports
  `v3-inactive`).
  `pocold build-info` reports the build's readiness alone (blocker `no-network`). The
  launcher's network description (`pocol network`, `import`, `upgrade`) reports these
  fields for the invitation's files, but it admits nothing, so it adds
  `admission-not-booted`: what devnet.json records about its admission is never
  evidence. The blocker clears only when this release pins the invitation's (chainId,
  genesisHash) in `NETWORK_PROFILES` with exactly the launch admission (scope
  specification, no condition, no waiver, the Network profile on a Node; V3-28 H2 (1)),
  and the launcher then reads that pin, not devnet.json. The release pins no network, so
  today every network keeps `admission-not-booted`. The identity switch
  (`deploy/mainnet/set_network_identity.py`) pins the new network with the decided launch
  admission (`ExpectedAdmission::FRESH_NETWORK`: experimental-devnet scope, Simnet
  profile on a Simnet node), not the launch admission, so that network loses only
  `network-not-release-pinned` and keeps `admission-not-booted`. Only a booted node's
  status reports the admission it booted with.
  An Android package's build-info.json is the build's raw identity, with
  `publicLaunchReady: false` and the single blocker `readiness-not-computed`.
`compatibilitySha256` stays for install integrity: `monitor.py` groups nodes by
`consensusIdentity` (and fork schedule) and warns about mixed digests.

**Upgrade rules (PR5).** Every consensus-relevant path stays fail-closed. Only D3 is an owner decision; D2 and D5
are the recommended defaults the code implements (owner decision pending: D2 and D5 of PR5):

- D2, "merged to main counts as released": gate values are spent on merge (VA-SPEC §2.3);
- D3 (owner decision), fork versions are sequential: a schedule appends version 2 before version 3, and the runtime
  schedule shape refuses a skipped version ([V3-DECISIONS.md](../decisions/V3-DECISIONS.md), "Release rules and
  lock", D3);
- D5, the activation-window identity (CONS-10) and a data-directory rules record, is deferred.

**Seeds behind NAT.** A seed that dials its own public address detects itself by its
nonce and stops dialing that address. This needs NAT hairpinning. A seed behind a NAT
without hairpinning cannot reach itself, so it cannot detect itself. Its dialer keeps
retrying its own address, as before this release, which costs nothing but a failed
connect. Such a seed host keeps its local or unspecified `--listen` and lists the other
seeds with `pocol start --peer` (or `pocold run --peer`: static peers, standalone README),
so that it keeps them all. Do not pass the public IP as `--listen` on a 1:1 NAT host (most
cloud VMs with an elastic or public IP): that IP is on no interface, so the bind fails
(`Cannot assign requested address`, unless `net.ipv4.ip_nonlocal_bind=1`) and the node
does not start.

## Stopping the seeds

The owner will stop the operator seeds. Nodes of this release keep finding, verifying and keeping community
connections without them: restarted nodes through their anchors and book, brand-new nodes through the bootstrap list
in the invitation, stalled nodes through the extra slot.

**Before the first seed stops** (each step checked):
1. **Release adoption.** Builds before automatic outbound dial only configured peers and are isolated once the seeds
   stop. Announce the stop with the release and keep the seeds up for an adoption window.
2. **Community listeners.** At least 8 volunteers in at least 4 /16s, each reachable from outside
   (deploy/public-testnet/test_remote_listener.py from another network).
3. **Capacity.** The listeners' `peers_max` sum is at least 1.5·(4·L + 2·N) for L listeners and N non-listening nodes.
   Phones behind one carrier NAT share a /16: each listener takes at most 5 of them (`per_group_max` + 1); listeners
   that serve wallets may raise `POCOL_PER_GROUP_MAX` within A13. Watch `refusedGroup`, `refusedRate` and
   `inboundEvicted` on sample listeners for a day.
4. **Bootstrap list.** Publish a v2 invitation with the same seeds and a `bootstrap` list
   (`pocol network --from … --seed … --bootstrap <volunteer> … --out network.pocol.json`) and its fingerprint through
   the same trusted channel as the genesis. Homes run `pocol inspect-upgrade --network …`, **compare the shown
   `fingerprintAfter` and `bootstrapAfter` with the published ones**, then `pocol upgrade --network …` (a rebind; a
   wrong list could eclipse new joiners). Older builds refuse unknown fields, so publish only for this release and later.
5. **Health without the seeds.** For a day on sample listening nodes: `healthy` >= 2, `seedTarget` 0, seeds
   "suppressed", `stalled` false; on non-listening nodes `healthy` >= 1 besides the held seed; on each seed
   `discovery.book.verified` >= 16 in `keys` >= 8.

**Staged stop.** Stop one seed with `systemctl stop` (a clean stop) and watch `stalled`, `healthy` and `extraSwaps` on
sample nodes for 24 h; then the second, and 24 h later the third. If the seeds were genesis miners, production goes on
with the other miners (fallback templates fill the stopped ones' drawn slots; expect slower blocks). Then publish an invitation with `seedsRetired: true`
(`--seeds-retired true`; same seeds and bootstrap list, fingerprint through the trusted channel) and let homes take it
with `pocol upgrade`; public-testnet join bundles drop their seeds (inventory `seedsRetired`).

**After the stop.** Keep the seed IPs allocated for as long as any invitation in circulation names them without
`seedsRetired`: a new holder of such an IP would be every brand-new joiner's first contact. Nodes without
`seedsRetired` want a seed only while needed and hold a dead one 1 minute to 6 hours (about 6 connects per seed per
6 h per node). An isolated node (`compatiblePeers` 0, `healthy` 0, `stalled`) recovers with `pocol start --peer <known
node>`, `pocold run --peer` or `--bootstrap`, or a newer invitation through `pocol upgrade`. Rollback: start any seed
again. Never copy an `addrbook` between nodes in local mode; prefer the bootstrap list.

## Inbound peers and eviction

A node keeps up to 32 established inbound peers (their Hello accepted) and, counted
apart, up to 8 connections still waiting for their Hello: at most 40 inbound
connections at once, besides one outbound connection per configured peer. When the
32 are full, the first Hello for this network and build from a newcomer evicts one
inbound peer instead of bouncing off (VA-SPEC §1b; local policy, which never changes
validity, chain state or admission). Protected are a keyed netgroup lottery, the
latest deliverers of new blocks, locked templates, pooled transactions and shares
(each kind its own quota), the longest connected, peers in their first 30 s, and as
many connections from a configured peer's address as it has configured peers. The
victim is the youngest of the rest in the address group with the most connections.
Evictions are limited to one per newcomer group per 30 s, and to 8 at once then one
per 2 s overall; past that, a newcomer is closed, as before. A public /16 or /32
holds at most 4 places plus one spare, and the newcomer in the spare place evicts
within its own group.

| Setting | `pocold run` | Launcher (`pocol start`) |
|---|---|---|
| Established inbound peers (default 32, 1..128) | `--peers-max N`; `POCOL_PEERS_MAX` | `POCOL_PEERS_MAX` |
| Inbound peers per public /16 or /32 (default 4, 0 off) | `--per-group-max N`; `POCOL_PER_GROUP_MAX` | `POCOL_PER_GROUP_MAX` |
| RPC `safe` depth k_safe (V3-26, default 6; 1 ≤ k_safe ≤ k_alarm, below K_hist = 32) | `--k-safe N`; `POCOL_K_SAFE` | `POCOL_K_SAFE` |
| Reorg alarm depth k_alarm (V3-26, default 60; `pocol_finality.reorgsGeKAlarm`) | `--k-alarm N`; `POCOL_K_ALARM` | `POCOL_K_ALARM` |

A flag wins over its variable, and an empty value counts as unset (except for
`POCOL_K_SAFE` and `POCOL_K_ALARM`, where an empty or non-numeric value is refused:
`run` names the variable, the launcher answers "Invalid POCOL_K_SAFE" or "Invalid
POCOL_K_ALARM"; a pair outside the k_safe bounds stops the node at open, naming it). For `pocold run`, a
value outside 1..128 (0 included) stops the command before the node starts (status
1), naming the flag or variable; the launcher refuses it as "Invalid
POCOL_PEERS_MAX". The seed and join units that `deploy/public-testnet/prepare.py`
writes set neither: **seeds run at the default 32** (see the halt figures below).

**Memory.** A13 counts every inbound place, the 8 waiting for a Hello included: one
more established place costs 3,424,620 B of model, about **3.73 MiB of budget** with
the 1/8 reserve. The raise from 8 to 32 adds 110,248,134 B of model (about 126 MB of
budget) to a node with 2 configured peers, on every profile, whether or not it
listens. An operator who lowers `POCOL_MEMORY_BUDGET_BYTES` (read by `pocol start`
and, since PR7, by `pocold run`, which also takes `--memory-budget-bytes`) should
lower `POCOL_PEERS_MAX` with it.

**The continuous chain's halt with eviction off.** A continuous chain (`H_END` 0) has no
finite lifetime, so a node with eviction off stops once its journal holds more records than the A13
budget admits (`a13_live_chain_halt_records`, store mode as the seeds boot; T_blk
10 s, so 8,640 records a day):

| Node | Before PR3 (main `e3d6f07`) | After PR3 | Difference |
|---|---:|---:|---:|
| Default node, 2 configured peers | 1,169,988 records (135.4 days) | 1,127,165 records (130.5 days) | −42,823 records (−5.0 days) |
| Seed, 7 configured peers, at the default 32 | 1,145,342 records (132.6 days) | 1,102,318 records (127.6 days) | −43,024 records (−5.0 days) |
| Seed at `POCOL_PEERS_MAX=64` (not shipped) | — | 1,059,778 records (122.7 days) | −85,564 records (−9.9 days) |

The plan allowed seeds 64 established places only if that moved their halt by at most
a week of records (60,480) against the same code at 8; it moves it by 85,093, so seeds
stay at the default. That halt was still finite (NODE-03 / SCALE-02). PR7 replaces it
(next section): with eviction on, the default, these record counts are only the
eviction-off reference.

## Memory bounds (PR7)

**What changed.** A node keeps only the last E canonical blocks in memory (E = 6,417 at
the defaults). Older canonical entries move to a new file in the data directory,
`canon.redb`, and come back from it if the head falls back. Checkpoints hold only that
window. The A13 bound no longer grows with the journal's record count. On the
continuous chain it grows only with the observed sizes of history.redb, canon.redb and
state.redb, at about 1 B per 4 KiB of file. A seed at the defaults (store mode, 7
configured peers) now stops at about **10.4 TiB of history.redb**
(`a13_live_chain_halt_disk_bytes`: 11,380,897,964,159 B; with 2 configured peers
11,642,050,895,999 B), where it stopped at 1,089,603 records (126.1 days) before (with
PR6's pool terms and automatic outbound; 1,100,218 records, 127.3 days, before automatic
outbound; 1,102,318 records, 127.6 days, before PR6). Nodes
on one chain hold almost the same files, so they would reach that size at almost the
same time; the disk usually fills first.

**The data directory.** `canon.redb` sits beside `journal`, `history.redb` and (store
mode) `state.redb`. It is derived data: a missing, damaged or foreign one (another
genesis) is set aside as `canon.redb.aside` and rebuilt by replaying from genesis.
Every checkpoint of a window above height 1 then needs the replay. It costs about
680 B a block on disk (about 2.1 GB a year at `T_blk` 10 s), and its first 34 MB
are allocated at once. Back it up with the rest of the data directory, or leave it
out and let the next open rebuild it.

**Settings** (local policy, never validity):
- `--memory-budget-bytes N`, or `POCOL_MEMORY_BUDGET_BYTES` when the flag is absent
  (`pocold run`; `pocol start` reads the variable). The default is 6 GiB. A value
  that is not a number, an empty one included, stops the command before the node
  starts.
- `--evict-depth N`: the canonical blocks kept in memory below the head. A value
  below the safe minimum is refused at open, naming the term it fails (whole-state
  mode 4,370 at the defaults, store mode 6,403). `--evict-depth 0` turns eviction
  off: the behaviour before PR7, including the record-count stop.

**LOCAL CAPACITY.** Every 256 records the node observes its files and rechecks the
bound. It logs `LOCAL CAPACITY warning: …` once the bound with its 1/8 reserve
reaches 15/16 of the budget. Past the budget it stops with `LOCAL CAPACITY: …; raise
--memory-budget-bytes or POCOL_MEMORY_BUDGET_BYTES`. With eviction on, either message
now means terabytes of history files, not days of blocks: raise the budget (each
extra GiB of budget admits about 3.5 TiB more history) or move to a host with more
memory. At open, `LOCAL CAPACITY at open: the journal's N records need … B of replay
tables …` means the journal's replay tables (about 160 B a record counted) do not fit
beside the bound; raise the budget. Status reports `canonicalBase`, `blocksIndexed`,
`anchors`, `evictStalls` (always 0 on a healthy node), `evictDepth`, `diskObserved`
and `resourceDiskBytes`. `resourceDiskBytes` replaces `resourceProjectionRecords`.

**Upgrade and downgrade.** The local formats that only the retired network's builds
wrote are not read (Phase 8a). A state store that holds a format-1 checkpoint
(`POCKPT01`) is deleted and started empty, and the node replays from genesis; there is
no one-time checkpoint resume and no migration headroom check. A state store of
format 2 and a history.redb that holds the old nonce ranges table are set aside
(`*.aside`) and rebuilt by replay. A journal in the v1 layout (`POCOLJ1`) or with the
128-byte commit file is refused by name and left untouched: start that node on a new
data directory. A whole-state node replays from genesis, as it always does, with
window-sized memory. An older binary refuses the current checkpoint format (bad
magic), drops it and replays from genesis, ignores canon.redb, and has the old memory
behaviour.

**Threads.** Every inbound place has a reader and a writer thread. The A13 thread
inventory is 2·(peers_max + 8) + 4·(configured peers) + 40 tasks with 32 RPC
connections: 128 at the defaults with 2 peers, 148 with 7, 340 at `--peers-max 128`
with 7. The rule for a unit is `TasksMax` ≥ 2·(peers_max + 8) + 4·peers + 48 (8 tasks
of margin). The public-testnet unit has `TasksMax=512`, the private testnet's and the
standalone unit 256. On Linux with cgroup v2, `pocold run` reads its cgroup's
`pids.max` when the network starts and refuses to start with "the A13 thread
inventory needs N tasks; this service allows M (raise TasksMax or lower
--peers-max)" when the inventory does not fit; when the limit cannot be read (the
root cgroup, cgroup v1, another OS) nothing is checked. A miner thread that cannot be
spawned is logged and skipped for that template instead of stopping the node.

**Redial backoff.** A configured peer whose connection ends within 60 s (a full peer
evicting or refusing this node, say) is redialed after 0.6 s, then 1.2 s, doubling to
at most 30 s; a connection that lasted 60 s or more resets the delay to 300 ms, and a
failed connect doubles it as well. A stable peer that is lost is redialed at once;
one that keeps closing quickly, or is down, is redialed at most every 30 s.

**Monitoring.** `pocol_status` gains:
- `inboundEstablished` (established inbound peers still open) and `inboundMax` (the
  setting, `peers_max`);
- `stats.inboundEvicted` and `stats.inboundEvictedInGroup` (evictions, and those within
  a newcomer's own full group);
- `stats.inboundRefusedFull` (no peer could be evicted) and `stats.inboundRefusedRate`
  (the eviction budget was spent), both also in the connection refusals;
- `stats.creditsBlock`, `creditsTemplate`, `creditsTx` and `creditsShare`: first
  deliveries from peers, which buy protection.

A steadily rising `inboundRefusedRate` means newcomers are arriving faster than the
budget lets them in, which is what a Hello flood looks like.

**Remaining risks.** Connections that never send a Hello, renewed every 5 s, can
still hold the 8 handshake places (and a group's spare place); with the group cap
this needs at least 2 /16s. Peers that connect first after a restart win uptime
protection for up to half of the unprotected places. Usefulness is cheap to forge,
so it buys only capped protection. Within one /16, an attacker and honest members
can evict each other's youngest members, at most once per 30 s per group, and the
group's oldest member is always kept.

## Evidence and limits (29 October 2026)

This is a dated record of the first operations increment, measured before the fresh network; the tools it
covers (`deploy/public-testnet/`) are current. Local runs (evidence under `evidence/operations/`):

| Check | Result | Scope |
| --- | --- | --- |
| Rust library | 51 PASS | Existing library regressions |
| Real-socket network/RPC | 55 PASS | Includes incompatible initial/repeated Hello, pipelined transaction fencing and bounded handshake/name checks |
| Key/genesis CLI | 5 PASS | Independent generation, file checks and admission controls |
| Final consensus and caps regression | 93 + 11 PASS | Seven existing expanded consensus measurements remain ignored; none added or disabled |
| Operations controls | 7 PASS | Inventory, links/checksums, both backup locks, restore tampering/identity, independent digest reproduction, URL and duplicate-JSON controls |
| Packaged four-node integration | 9 PASS | Independent transfers, late registration/mining, duplicate rejection, site publish/fetch, live-backup refusal, stopped-state restore and observation |
| systemd unit syntax | PASS locally | Unit verification only; actual service activation/credential ownership still needs a target host |
| Nginx TLS/rate test | Pending exact-head CI | Nginx is absent locally; the workflow installs it and runs the loopback test, without claiming a local pass |
| Separate systemd service UID and root restore | Pending exact-head CI | Runs the service from a restrictive-mode bundle and restores root-made state with service UID/GID; uses loopback fixtures only |

The packaged integration binary SHA-256 is
`33b3c8e23d4bfb762526853863f562f856daa36296a8015a099f5f6237d6296b`;
its source compatibility fingerprint is
`64e7fef3cbd46c7e7af6c3442962ecf3291678f233b45a60158f15f0528cf950`.
Restore compared the common block at height 27. The following 180-second observation
ended with all four heads at 59 and identical confirmed roots at height 53. These are
four processes on one machine, not four-host acceptance or a sustained soak.

The independent digest control initially failed: Rust Path component ordering differed
from bytewise string ordering. The failure is retained in evidence; the build script
was corrected and the independent control then passed. The 51/55 Rust run preceded
that ordering correction; the packaged integration and final controls used the corrected
build. Exact-head CI reruns the Rust suites on the final submitted source.

A wider consensus check then found a legacy fixture that injected messages from a
nonexistent peer. Its original failure is retained. Handler fixtures now establish a
real compatible transport connection before injecting deterministic events; their
original validity, penalty, pool and recovery assertions remain. The non-loopback
filter correction followed the recorded packaged run, so final-source integration
and the actual non-loopback-interface control are required in CI. Local interface
enumeration was unavailable (`netlink: Operation not permitted`); no route/security
configuration was changed to bypass that restriction.

Automated review of the first submitted operations commit found two real service
permission defects: a 077 preparation umask could leave public bundles unreadable
by the service UID, and root-created restored state was not owned by that UID. The
installer now normalizes the verified public tree to directories/binaries 0755 and
other files 0644. Root restore requires an explicit service owner and applies UID/GID
with 0700 directories/0600 files. The runbook also updates StateDirectory alongside
the restored data path under ProtectSystem=strict. The added CI drill starts the
actual separate-UID service, checks LoadCredential key access, restores and restarts.

Exact-head CI at `f2be434ff8a27aa10e813b0b05fbc482c952b9f4` passed all Rust,
Python, packaged integration, non-loopback listener and Nginx TLS/rate checks, but
the new service drill failed. The systemd credential's metadata did not satisfy
the strict Rust key-file checks. The original failure is retained in
`evidence/operations/initial-systemd-credential-failure.log` (workflow run
[40883255893](https://github.com/palcertcom/pocol/actions/runs/40883255893)).
The unit now materializes a service-owned 0600 copy in a private 0700
RuntimeDirectory, removed on stop, without relaxing the Rust checks or copying a
secret into persistent state. The drill additionally checks owner, mode, link
count and removal on stop; this correction still requires exact-head CI.
The [systemd credential/runtime directory semantics](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html)
are distinct from the application's own key-file metadata policy.

A subsequent final-core local integration passed all nine checks with binary
`7b1bc349966d8a42d501575e7de452d6baf9ecd96985b482d36ec27d59415e23`
and compatibility fingerprint
`bfdf1d05559199057c3dfb83969839fe3fd723a195d4e332ed17ec485b36e4ca`.
Its public evidence is retained separately in `evidence/operations/integration-final/`.
It includes the listener correction but does not exercise systemd credentials.

The operations integration used the recorded packaged files. Subsequent preparer-only
validation additionally rejects multicast/reserved/duplicate-host addresses and
noncanonical seed ports. Those controls are covered by the final unit tests and CI;
they do not change the node fingerprint. Public evidence contains no keys, state
backups, generated TLS private keys or complete release binaries.

The historical English-translation manifest is preserved. Its current-file hash
check reports five deliberately changed code/test files from this increment; that
is expected snapshot drift, not a new translation attestation. Its language scan
found no raw Arabic text outside the unchanged thesis, with original-source hashes,
reference numbers and translated archive checks unchanged. The complete result is
retained as `translation-snapshot-drift.json` rather than relabeled PASS.

## Remaining launch work

The open gates and the launch order are in [PLAN.md](PLAN.md). For operations:

1. **Rehearsal.** Run the genesis ceremony first with the rehearsal profile
   (`deploy/mainnet/v3-rehearsal-profile.json`) on the three servers and the PC, and measure the launch evidence
   in [REHEARSAL.md](REHEARSAL.md) (T1-T8, M1-M4, R-off, D1, S1, the cap-raise rehearsal S2 above; FRESH-NETWORK-CP
   §9): multi-host partition/heal, restart/replay, backup/restore ([RECOVERY.md](RECOVERY.md)), overload,
   release/rollback, the resource-growth bound and soak.
2. **Mainnet ceremony.** Only after the rehearsal evidence, re-provision the hosts and run the ceremony with the
   committed mainnet profile (`deploy/mainnet/v3-profile.json`, a continuous genesis with `H_END` 0) as described in
   [deploy/mainnet/README.md](../../deploy/mainnet/README.md) and
   [deploy/standalone/README.md](../../deploy/standalone/README.md): fresh keys again on every founder's device, one
   genesis, one invitation (PLAN.md, "Launch order").
3. **Bounded state and admission.** Resolve bounded historical-state/K_eff accounting and near-cap tests; complete the
   deferred resource/gas derivations and the missing RpcGuard mechanisms; measure worst-case execution.
4. **Review.** Obtain an independent security review (Q1, open: the software reports it as `plan-gate-Q1`) and
   publish the reviewed release with its limitations. Existing automated review is not that review.
5. **Fork tooling.** The seed fork-migration tool (`deploy/public-testnet/nonce_upgrade.py`, G3 above) refuses an
   H2 or H less than 8,640 blocks (the 24 h minimum notice) above the seed's highest stored block, with a test; it
   has not yet been run on real hosts (the upgrade drill of gate L1 remains open).
