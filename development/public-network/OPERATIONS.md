# Public-network compatibility and operations, 29 October 2026

The owner requested completion of a functioning public network. This increment
implements additional prerequisites; **the complete launch goal is not fulfilled**.
No actual server addresses/access were supplied. No Internet service, paid hosting,
real-value currency or production profile was launched. Remaining software/protocol
obligations are still open in [PLAN.md](PLAN.md); servers alone would not close them.

## Compatibility and hostile input

Experimental wire v2 appends `protocolVersion` and a 32-byte compatibility digest to
the seven-field Hello, making nine fields. Old handshakes and different versions or
digests fail closed, on first contact and later status updates. A rejected connection
is removed immediately; already queued frames and a second valid-looking Hello
cannot reauthorize it. Only admitted peers receive gossip. The first frame is limited
to 512 bytes; peer names are 1..64 printable ASCII bytes without spaces/control codes.

`pocold build-info` exposes a reproducible source fingerprint, and `pocol_status` /
`pocol_getParams` expose it with protocolVersion. The fingerprint commits to the UTF-8
path and raw bytes of all Rust source files in the three crates, their Cargo manifests,
the pocold lockfile/build script, embedded system/website artifacts and the caps/crash/
I/O-fault feature flags. Paths use bytewise lexical order with 64-bit big-endian
lengths, SHA-256 and domain `pocol-testnet-source-compatibility-v1\0`. It excludes Git
commit metadata, absolute build paths, timestamps and secrets. Raw text requires a
release checkout with preserved line endings.

This conservative identity intentionally changes even for some non-consensus edits.
It is not the unresolved D-5 normative constants digest, a binary reproducibility
proof, peer authentication or remote attestation. A malicious peer can advertise any
digest. Strict coordination is required for upgrades; no implicit wire downgrade or
state-binding rewrite is provided. Block validity, economics, H_END and the existing
production-admission refusals are unchanged.

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
items: `listenPort` and an 8-byte `nonce` follow the compatibility digest.
`PROTOCOL_VERSION` stays 2, so the compatibility digest changes and a 9-item Hello
fails closed. A node sends GetAddr (type 8) once on each connection it dialed, and
answers the first GetAddr of each inbound connection with one Addr (type 9) of at
most 64 addresses. It learns only the addresses it dialed and verified, the source
IP and `listenPort` of its inbound peers, and the answer to its own GetAddr. Nothing
is relayed further, and no peer is banned. This release makes no new connections:
it dials only its configured peers, as before.

| Setting | `pocold run` | Launcher (`pocol start`) |
|---|---|---|
| Discovery on (default) or off | `--no-discovery`; `POCOL_DISCOVERY=0` or `1` | `POCOL_DISCOVERY` |
| Local mode (off by default) | `--discover-local`; `POCOL_DISCOVER_LOCAL=0` or `1` | `POCOL_DISCOVER_LOCAL` |

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
changed. `pocold run` has no signal handler, so a killed node keeps its last save.
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
only peer is itself therefore shows 0, and monitor.py and preflight.py can see that
it is isolated. Watch `compatiblePeers`, `discovery.self`, the `selfConnections`
counter and `bookSaveErrors`. The automatic-outbound keys (`discovery.outbound`:
`healthy`, `stalled`, `seeds[].state`) arrive with automatic outbound connections.

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

**Automatic outbound connections.** Dialing learned addresses (`--outbound`,
`--seed`, the feeler and anchors) is not part of this release. When it is added, its
default stays 0 until inbound eviction exists (PR3). Until then, a peer that fills
the inbound slots could turn automatic dialing into an eclipse.

**Upgrades (G2, G3).** This release changes the compatibility digest, so seeds and
users must upgrade together. Launcher homes cannot take a release that changes only
the digest: `upgrade_transition` (launcher.rs) accepts nothing but appending one v2
fork. Before this release reaches launcher homes, either issue a new invitation or
merge a separate change that accepts a transition where only `compatibility_sha256`
differs.

**Seeds behind NAT.** A seed that dials its own public address detects itself by its
nonce and stops dialing that address. This needs NAT hairpinning. A seed behind a NAT
without hairpinning cannot reach itself, so it cannot detect itself. Its dialer keeps
retrying its own address, as before this release, which costs nothing but a failed
connect. Once `pocol start --peer` exists (with automatic outbound), such a seed
should list the other seeds with it instead (standalone README).

## Evidence and limits

Local runs (evidence under `evidence/operations/`):

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

1. Resolve bounded historical-state/K_eff accounting and near-cap tests; complete
   deferred resource/gas derivations and the missing RpcGuard mechanisms.
2. Adopt a concrete public profile and normative decisions, measure target hardware
   and worst-case execution, and define lifetime/cumulative-cap behavior. The present
   profile still closes/ends at 19,000/20,000; this is not a perpetual network.
3. Supply operator-controlled hosts/endpoints, independent public keys/allocations,
   clock/network measurements and a shared genesis schedule; perform multi-host
   partition/heal, recovery, overload, release/rollback and soak acceptance.
4. Obtain an independent security review, publish a reviewed release and limitations,
   then activate/verify the public endpoints. Existing automated review is not that review.
