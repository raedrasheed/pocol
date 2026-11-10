# VA-spec v1 — PoCol peer validation and synchronization (experimental TCP transport)

Status: specification for LP6 written before/with the peer code in
`prototypes/pocold/src/net.rs` and `node.rs`; reviewed against consensus.md
§4, §8 (envelope classes D129–D140), §9 (D133) and §10 (D118). An experimental
TCP transport (devnets and testnets, loopback or remote peers); it is not the
production P2P design of network.md (libp2p, Noise, gossipsub).

## 1. Transport and framing
- TCP on the operator-selected listener: a loopback bind stays local; a private or public bind accepts remote
  peers, each of which must pass the Hello identity check (§2). Outbound connections go to the configured peers
  (one dialer each, backing off to 30 s after failures; seeds among them follow §1a.8) and, with automatic
  outbound (§1a.7; on by default), to addresses from the book.
- Inbound limits (local policy, `net::Limits`): at most `peers_max` established connections (their
  first eligible Hello accepted, §1b; default 32, `--peers-max N` or `POCOL_PEERS_MAX`, 1..128; seeds
  run at the default) and, counted apart, at most `handshaking_max` (8) still waiting for their Hello,
  so the physical bound is their sum, `inbound_max` (40). A full established set evicts one peer for
  an eligible newcomer, or closes the newcomer (§1b). At most `per_group_max` (a fixed default 4;
  `--per-group-max`, or `POCOL_PER_GROUP_MAX` for `run`; 0 off) connections per public address
  group: IPv4 /16, IPv6 /32, an IPv6 address carrying an IPv4 address (6to4, Teredo, NAT64, mapped)
  counting as that address, and Yggdrasil 200::/7 (key-derived) as one group. A full group has one
  spare place (`GROUP_SPARE`): the newcomer that takes it must evict the youngest unprotected member
  of its own group at its first eligible Hello (§1b.6), or it is closed, so the group's established
  members stay within `per_group_max` and one idle host cannot lock the rest of its /16 out. A
  connection past the spare place is refused even while other places are free: nodes behind one NAT
  or one provider range share these places. Groups are counted per address, so one host that
  reaches the node from several groups (an IPv4 /16 and an IPv6 /32, say) holds a full set of places
  in each. Addresses that are not globally reachable (`net::group_of`: loopback, private, link-local,
  shared 100.64/10, benchmarking, documentation, reserved, unique-local fc00::/7 including cjdns,
  ...) are not grouped, and neither are the node's configured peers, by their exact address.
- A configured peer is redialed after a failed connect, or a connection that lasted less than 60 s, with a
  doubling delay, 0.6 s, 1.2 s, ... at most 30 s (`net::redial_delay`); a connection that lasted 60 s or more
  resets it to 300 ms. A node that evicts or refuses this one, and a peer that is down, therefore see its
  redials settle down.
- Frame = `u32 BE length || RLP([type, ...fields])`, `1 <= length <= MAX_FRAME` (256 KiB).
- Decoding is strict: canonical RLP, exact arity, minimal integers, fixed
  32-byte hashes, 8-byte nonces; re-encoding must equal the received bytes.
- Any malformed frame closes the connection. Malformed input never reaches
  block state, is never stored and never marks a block invalid.

| type | message | fields |
|---|---|---|
| 0 | Hello | chainId, genesisHash, headHash, headHeight, totalWork (32 B), nodeName, protocolVersion (the sender's maximum wire version, §2.2), consensusIdentity (32 B, §2.1), listenPort (0..65535; 0: none), nonce (8 bytes; 0: none) |
| 1 | Block | headerBytes, bodyBytes |
| 2 | Template | signed-template shell (header encoding with nonce 0), body |
| 3 | Share | TemplateID, nonce (8 bytes) |
| 4 | GetBlock | blockHash |
| 5 | GetRange | startHeight, count (<= 512) |
| 6 | Tx | raw signed envelope |
| 7 | GetFrom | locator (1..32 concatenated 32-byte hashes), count (<= 512) |
| 8 | GetAddr | (no fields) |
| 9 | Addr | records: one byte string of 0..64 records of 18 bytes, a 16-byte IPv6 address (IPv4 as `::ffff:a.b.c.d`) and a nonzero big-endian u16 port |

## 1a. Peer discovery (local policy)
Local node policy (`node::Limits::discovery`): it never changes validity, chain state, cfg or admission, and
GetAddr and Addr are not gossip (NodeLocal.gossip and R15 do not cover them). A malformed frame closes the
connection (§1); any other discovery fault drops an entry or a message and is counted. No peer is banned.
1. Advertising. A listening node with discovery on sends its listener's port in `Hello.listenPort` (0
   otherwise). listenPort and the nonce (§2) are sent only on a connection that can reach the listener:
   every connection for an unspecified bind, a loopback peer's for a loopback bind, otherwise a connection
   on the bound address; elsewhere both are 0. They are read from a connection's first Hello only.
2. Learning. (a) An inbound connection's first compatible Hello with listenPort != 0: the address (the
   connection's source IP, listenPort), never another IP, as a self-advertisement; with listenPort 0 and a
   nonzero nonce (a listening node that no longer advertises), the unverified self-advertisements at that
   public source IP are forgotten, except those another live connection advertises. (b) The first compatible
   Hello of a connection this node dialed: the dialed address is verified when listenPort != 0 and forgotten
   when it is 0. (c) The one Addr that answers this node's GetAddr on a connection it dialed, within 120 s:
   at most 64 addresses, heard from that peer. Nothing else is learned.
3. Never stored: ports below 1024 (configured addresses exempt); IPv6 outside 2000::/3; unspecified,
   multicast, broadcast, link-local and reserved addresses; addresses without an address group (§1) unless
   `--discover-local` is on and the address came from a peer that has none either; this node's own
   addresses. Learned addresses are never exempt from the inbound group limit.
4. Address book: at most 1024 entries, 8 per address group (local mode: each local endpoint is its own
   group), 2 ports per IP, 64 unverified per answering peer and 256 self-advertised. A newcomer replaces only
   an entry that is terrible, except that a peer's newcomer may replace that peer's oldest unverified entry,
   a self-advertisement one of the most crowded group's, and a verified newcomer an unverified one; a
   verified entry, and a live entry of a full IP, are never replaced. Most crowded (netgroup-largest): among
   the unverified self-advertisements, by address group when the 256 or the book is full, by subnet (IPv4
   address, IPv6 /64) inside the newcomer's full group; that group gives up its entry advertised longest ago
   if it holds at least 2 more than the newcomer's. Terrible: never verified and 3 counted failures; 10
   failures and no verification for 7 days; or nothing heard, advertised or verified for 30 days; terrible
   entries are dropped at load and at each save. A failure counts only if some dialed connection was verified
   since the entry's last counted failure. Closed before any Hello (a full listener, say) is no failure for a
   verified entry and a failure for an unverified one, as is a remote unreachable error; a connect that this host
   could not hand over (a local failure) is neither. An entry is dialed again 10 min after a failed or short
   attempt if never verified, 1 min if verified (at once after a connection that verified it), and a verified
   entry at once while the node has no compatible peer.
   Another release holds an entry back min(10 min · 2^(n-1), 6 h); another chain or genesis removes it.
5. Answers: GetAddr is answered once per inbound connection and never on a connection this node dialed, with
   an Addr of at most 64 entries (possibly none): entries verified or self-advertised within 7 days, verified
   entries not learned from a self-advertisement first, at most 2 per address group, never the requester's own advertised address, local entries only to a
   local requester, from a list rebuilt at most once a minute (an entry that leaves the book leaves the list
   at once); it is queued only while the requester's queue
   is below half (§4). A node sends GetAddr once per connection it dialed (not feelers).
6. Persistence: `<data dir>/addrbook`, bound to chainId and genesis and checksummed, written atomically 60 s
   after start, every 15 min, when the anchors change and at a clean stop, read with a 1 MiB bound. Every
   entry read passes 3 and 4 again; a damaged, foreign or oversized file is ignored (empty book). These
   periods, the refresh of live entries and the 120 s of 2(c) run on the wall clock: when it steps back, no
   deadline stays more than its period ahead, and a GetAddr sent "in the future" expires.
7. Automatic outbound (`--outbound N`, 0..8, or `auto`, the default: 4 with a listener, 2 without; POCOL_OUTBOUND;
   0 with discovery off): N slots, one connection each, at most one per address group (local mode: per endpoint),
   never to a configured peer, a seed, this node, an inbound peer's advertised address or an address already
   connected; plus one spare slot. Choice: verified or unverified entries 50/50 when both can be dialed; in each, by
   source (all self-advertisements one source), then group, then fewest failures. A slot waits min(1 s · 2^k, 60 s)
   after k failures. An address family is skipped only while this host has no route for it (a local probe).
   Spare slot: with every slot up or without candidates, one feeler every 2 min (10 min without a listener, plus up
   to 30 s) connects to an unverified entry, sends nothing, reads the peer's Hello, verifies the address
   (listenPort != 0) or forgets it, and closes; it is never a peer, and the peer, which never receives its Hello,
   evicts nobody for it (§1b). While stalled (the head's timestamp older than W = max(120 s, 12·T_blk)) or quiet (no
   block from a peer moved the head within 4W), the spare is an extra full slot to a new source (bootstrap
   addresses first while no seed can be wanted); if it moves the head it replaces the slot that was useful longest
   ago (never the sync peer), else it closes after W. Useful: a new block within 2 of the head delivered first, a
   block of the peer's sync session, or a template that locked. Anchors: up to 2 automatic peers connected 10 min,
   useful within W, in distinct groups, saved with the book (not while stalled) and dialed first after a restart
   (once, within 24 h of their save). Configured peers + slots <= 18 (network.md:157), checked at boot; `auto` is
   lowered to fit.
8. Seeds (`--seed`, the invitation's seeds unless it retires them; with N > 0; with N = 0 they are configured
   peers): they bootstrap and refill, and at most 2 are wanted at once; a seed not wanted is not dialed. For the
   first minute seeds are wanted only while the book cannot fill the N slots (slots dialing or up, groups of
   dialable verified entries, anchors). Then a node wants one seed while it does not listen, has fewer than 2 useful
   peers (either direction, at least one outbound, outside the seeds' groups), has a slot without candidates and
   fewer than N peers, or is stalled; a listening node releases it 2 min after 2 useful peers hold. A released seed
   has been compatible 30 s, its GetAddr answered or expired, and is not the sync peer. A wanted seed not compatible
   within 30 s is held 1 min, doubling to 6 h, and the next is wanted. While stalled, a seed held for W without
   session progress is replaced by the next. A node that finds a seed is itself (§2, its exact `--listen`, or a local
   listener on the seed's port) keeps all other seeds.
9. Bootstrap addresses (`--bootstrap`, at most 16, ports >= 1024; the invitation's optional `bootstrap`, not part
   of the network's identity): at each network start each is an unverified entry of its own source, dialed by 7
   and never answered (5) before verified. A node whose seeds are gone reaches the network through its anchors, its
   book, its bootstrap addresses or its configured peers.

## 1b. Inbound eviction (local policy)
Local node policy (`net::Net::establish`, `net::Limits::evict`), after Bitcoin Core's SelectNodeToEvict without
its latency and relay-type steps (there is no Ping message). It never changes validity, chain state, cfg or
admission, and no message or wire field is added.
1. When. An inbound connection's first eligible Hello while `peers_max` connections are held, or the first
   eligible Hello of a connection that took its full group's spare place (§1) while that group still holds
   more than `per_group_max` places; a group that shrank meanwhile makes it an ordinary newcomer. Held:
   established and still open; a victim already closed but not yet released does not count.
2. Eligible: the Hello passes the one acceptance predicate of §2 (`identity::accepts`: chainId and
   genesisHash equal, protocolVersion >= PROTOCOL_VERSION_MIN, consensusIdentity equal), and its nonce is not
   this node's own. An ineligible Hello is never established, never evicts and
   never counts as held; the node then closes it (§2) or finishes it as a self-connection.
3. Candidates: the other established, open, non-exempt inbound connections. A connection established less
   than `grace` (30 s) ago is never chosen, so a flood of newcomers cannot evict the honest peers that joined
   just before it; it still counts in its key's size (step 4e).
4. Victim (`net::victim`, every order ends in the connection id, so the result does not depend on the input
   order); a candidate is a connection past its grace that no step protected:
   a. a keyed netgroup lottery: the eviction keys ranked by a hash keyed at random per process, and the
      oldest candidate of each of the first 4 keys protected;
   b. per kind of delivery (a new block, a template that locked, a transaction that entered the pool, a
      valid new share), the `max(4, peers_max/16)` candidates that delivered one most recently, each kind
      its own quota, so cheap shares cannot take a template relayer's place;
   c. the older half of the remaining candidates (by connection order, rounded down);
   d. with no candidate left, no victim;
   e. the eviction key with the most established non-exempt connections, protected and graced ones
      included (credits cannot shrink a key's apparent size), among the keys that still have a candidate,
      ties to the key with the youngest candidate; its youngest candidate is the victim.
5. Credits (step 4b, `Node::credit`): a block new to this node (a duplicate, an orphan or an invalid one
   earns nothing), a template that locked, a transaction admitted to the pool at once (a duplicate earns
   nothing, and so does one queued behind the D-7 backlog, which does not record its sender), and a valid
   new share. Each is the time of the connection's last delivery of that kind; a peer already gone earns
   nothing.
6. A full group (`net::victim_in`): the newcomer in the spare place evicts within its own group only: among
   the connections that hold a place in that group (not a configured address's further connections, which
   share its key but hold none) past their grace, the oldest and the latest deliverers of a new block and of a
   locked template are kept, and the youngest of the rest goes. Other groups are untouched. With none, the
   newcomer is refused by group.
7. Budget. A newcomer's eviction key evicts at most once per `key_interval` (30 s; a ring of the latest 64
   charges), and all newcomers together at most `burst` (8) at once, then one per `refill` (2 s), a GCRA.
   A newcomer the budget refuses is closed, as every newcomer to a full set was before eviction existed, so
   a spent budget fails safe.
8. Configured peers. As many open connections from a configured peer's exact address as there are
   configured peers at it are exempt: never candidates, never charged, and when no victim exists such a
   newcomer may exceed `peers_max` by at most min(configured peers, `handshaking_max`/2), so at least half
   of the handshake places remain. Further connections from that address are ordinary candidates.
9. Outcome. A refused newcomer is closed before its Hello reaches the node. A victim is closed (its queued
   frames dropped) and keeps its inbound and group places until its reader releases them, so physical
   inbound connections stay within `inbound_max` even while consensus is stalled.
10. Eviction keys (`net::EvictKey`), from the canonical address: a public address's group (§1, computed
    also with `per_group_max` 0); all of fc00::/7 (cjdns and ULA overlays, where addresses are cheap) one
    key; another ungrouped IPv6 address its /32 (loopback, link-local, ...); an ungrouped IPv4 address
    (LAN, loopback) itself. Only eviction uses these keys; the per-group cap stays on §1's groups.

The group cap and eviction from the largest key are the real limit on how many places one host, one /16 or
one flood can hold. Usefulness buys only the capped per-kind protection of step 4b, since forging it is
nearly free (relaying honest data first, shares below `nonce_max`, cheap transactions). Peers that connect
first after a restart win uptime protection (step 4c) for up to half of the unprotected places, each /16
within its 4 places plus 1 spare; this is Core's trade-off. `handshaking_max` connections that never send a
Hello, renewed every 5 s, still keep newcomers out (as a group's spare place does for its group); per-source
accept rate limiting is future work.

Status (`pocol_status`): `inboundEstablished`, `inboundMax` (`peers_max`), `stats.inboundEvicted`,
`stats.inboundEvictedInGroup`, `stats.inboundRefusedFull`, `stats.inboundRefusedRate`,
`stats.inboundRefusedByGroup`, and `stats.creditsBlock`, `creditsTemplate`, `creditsTx`, `creditsShare`.

## 2. Network identity
A Hello is accepted iff `identity::accepts(local, chainId, genesisHash, protocolVersion, consensusIdentity)` is
Ok: chainId and genesisHash equal cfg (else `Network`), protocolVersion >= PROTOCOL_VERSION_MIN (else
`Wire(v)`), and consensusIdentity equals this node's (else `Consensus`). Inbound eviction (`net::Net::eligible`,
§1b.2) and the node's close decision use this one predicate. A refused peer is closed; `Network` forgets an address
this node dialed, any other refusal holds it (§1a). The build digest (`compatibilitySha256`) is never a peering
condition: it is informational install-integrity status. activeVersion is checked per header (H-pre 1b); the
handshake is self-declared, not remote attestation, and block validation stays authoritative.

### 2.1 Consensus identity
```
id = SHA-256( "pocol-consensus-identity-v1\0" ‖ chainId u64 ‖ genesisHash 32 B ‖ CONSENSUS_RULES_VERSION u32
            ‖ flags u8 ‖ n u8 (1..=16) ‖ (version u16 ‖ height u64) × n ‖ [sourcesAggregate 32 B, only if bit1] )
```
Integers are big-endian; the schedule is the full configured fork schedule (`ForkSchedule::entries`). A node runs
only a schedule of versions 1, 2, ... in order, each once, starting `(1,0)`, at most MAX_FORK_VERSION = 2
(`configured_forks`; today exactly `[(1,0)]` or `[(1,0),(2,H)]`). The profile itself would accept a skipped version;
the runtime keeps refusing one (owner decision D3, implemented at its recommended default; the v3 PR decides).
genesisHash
commits CP, sysCodeHash and the alloc. Flags: bit0 caps-test, taken from `pocol_lp4_execution::CAPS_TEST` (the
crate whose LOG_CAP it changes; a build enabling lp4's feature without pocold's does not compile); bit1 unpinned,
set when the build's consensus-boundary aggregate differs from the lock (§2.4), in which case the 32-byte aggregate
is appended, so an unpinned tree peers only with byte-identical unpinned trees. Other bits are 0. Left out: the build
digest, the crash-test and io-fault features, `r_keep`, admission and other boot policy. A pinned build's identity
is computable by anyone from public data (devnet.json and `consensusRulesVersion`): `identity.rs`,
`deploy/public-testnet/pocol_identity.py` and the inline copy in `deploy/mainnet/preflight.py` compute it, and the
H15000 value (chain 1599846715, genesis `0xbe137b…`, rules 1, flags 0, `[(1,0),(2,15000)]`) is pinned in tests:
`bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110`.

### 2.2 Wire range
PROTOCOL_VERSION = 2 is this build's maximum and PROTOCOL_VERSION_MIN = 2 its minimum. Hello item 7 is the sender's
maximum; a peer is accepted iff its maximum >= MIN, and the session runs at min(peer's maximum, ours). The 11-item
Hello layout is frozen across every accepted range: changing it requires raising MIN. A sender never emits a message
type or field above the negotiated version, and decoders stay strict (§1). Item 8 was redefined from the build digest
to the consensus identity while keeping wire 2: deployed 0.4.0 sends 9 items and fails on arity, and earlier main
builds send a build digest, which fails the 32-byte comparison. A first frame that fails to decode and is a type-0
list of 9 items is counted as a 0.4.0 handshake (`legacyHellos`, §2.5).

Wire golden vectors (`pocold/tests/data/wire-golden.json`): one canonical frame per message type 0..9 and a
`v2/limits` entry (MAX_FRAME, the Hello arity and the field bounds), each keyed `v<version>/<name>`. The unit test
`net::tests::wire_golden` encodes each to its pinned bytes and decodes it back. The lock holds one `wire <sha256>
<key>` line per key, and the guard enforces: a key with v >= PROTOCOL_VERSION_MIN never changes and is never removed
(changing a frame needs a raised MIN, which retires the key); a new key needs v == PROTOCOL_VERSION and v greater
than every existing key's, since old decoders are strict. A wire change is re-pinned `kind=wire`.

### 2.3 Bump policy
Every change to a consensus-boundary file (§2.4) needs one appended history line in `consensus-sources.lock`:

| kind | when | requirement |
|---|---|---|
| `bump` | validity, state or receipt roots, rewards, genesis decoding or fork choice can differ for some input under a schedule or genesis an accepting build allows | CONSENSUS_RULES_VERSION += 1, **only together with a new genesis**; a rule change on a live chain is always a new fork version and a schedule entry |
| `neutral` | refactors with identical verdicts; logs and comments | note required |
| `neutral` + `gate=fork:V` | new rules reachable only through `active_version(h) >= V` | V greater than every `maxFork` on an earlier history line |
| `neutral` + `gate=spec:S` | new rules only under genesis specVersion >= S | S greater than every `spec` on an earlier history line |
| `neutral-vector` | a pinned vector file changed | note required; reviewers presume `bump` |
| `neutral-dep` | the dependency closure or a `rust-toolchain.toml` changed | note required; presumed `bump`; the release replay gate is mandatory |
| `wire` | only the wire golden file changed | the §2.2 rules |

"Released" means merged to main: every history line records `maxFork` (MAX_FORK_VERSION) and `spec`
(genesis_spec::SPEC_VERSION), so a gate value is spent as soon as the gated code lands on main, because public-testnet
nodes run main builds (owner decision D2, implemented at its recommended default). Consequently a second v3 change
after the first has merged needs a new gate or a bump with a new genesis.

### 2.4 Boundary, lock and enforcement
`pocold/consensus-boundary.txt` lists the consensus sources (paths relative to `prototypes/`): `+` includes (all of
`lp4-execution/src/` by default, so new modules are covered, minus the pool, RPC, bin and resources modules and
`*tests.rs` files that their parent declares only under `#[cfg(test)]` and that no `+` file loads without it),
the shared lp1 sources, the pocold validation, state,
history, signature-recovery, crate-root and identity sources, `build.rs`, the boundary code and file, the three
Cargo manifests, both `rust-toolchain.toml` files, the vector directories and the EVM-crypto path patches. Cargo.lock
is not hashed whole: the dependency closure of `root` crates (pocol-lp4-execution, k256, sha2, sha3) becomes one
`dep` line per package. Every other crate-root module is classified node-local (`=`), and a `+` file may reference an
`=` or `-` module (by `crate::`, `$crate::` in a macro, another crate's name, or a `self::`/`super::` path that climbs
to the crate root) only through a reviewed `~` line with its reason; other `$`-prefixed paths are macro metavariables
and are not resolved. The aggregate is
`SHA-256("pocol-consensus-sources-v1\0" ‖ (u64 len name ‖ name ‖ u64 len bytes ‖ bytes)*)`, with the `dep` lines as
one entry named `Cargo.lock#closure`.

`pocold/consensus-sources.lock` holds append-only `history` lines (n, kind, rules, maxFork, spec, protocol,
protocolMin, sources, vectors, deps, wire digests, ref, note) and the `file`, `dep` and `wire` manifest, which re-pin
mode rewrites: `POCOL_REPIN_CONSENSUS=<kind> POCOL_REPIN_REF=<PR> POCOL_REPIN_NOTE=<why> [POCOL_REPIN_GATE=fork:V]
cargo test --release --locked --test consensus_guard`. Re-pin refuses `neutral` over a changed vector or dependency
closure, `bump` without the incremented constant, and a spent gate. Enforcement:
- `build.rs` computes the aggregate. Equal to the last history line's `sources`: pinned. Otherwise the build is
  unpinned (bit1 and the aggregate in its identity, §2.1), and with `POCOL_RELEASE=1` (every packaging build) it fails
  with the instruction text. Build-info reports `consensusPinned`, `consensusSourcesSha256`, `consensusRulesVersion`,
  `protocolVersionMin` and `maxForkVersion`; `deploy/standalone/package.py` refuses a build whose `consensusPinned`
  is not true.
- The guard (`pocold/tests/consensus_guard.rs`, CI) checks the manifest, the aggregate build.rs saw, history
  consistency, the constants, module classification, the reference closure, `*tests.rs` declarations, the wire rules
  and the legacy launcher table (`launcher::LEGACY_BUILDS`: every entry names the H15000 chain, genesis and rules 1).
- `deploy/ci/check_lock_append_only.py` requires main's history lines to be a prefix of the branch's, and main's wire
  keys to obey §2.2.

The reference closure is a lexical scan, not a parser: an unusual construction errs towards an unclassified or
unannounced reference (fail closed). A consensus effect through a node-local module that no `+` file references is
outside what the guard can see; `chain.rs` validates everything it imports.

### 2.5 Diagnostics
A refusal is logged as `peer <name>: <reason>; closing` and counted in status `incompatiblePeers{network, wire,
consensus}`; `lastIncompatibleReason` holds the last `peer <name>: <reason>`, at most 160 characters (null when there
has been none), and `legacyHellos` counts 0.4.0 handshakes. The reasons: "another network (chain id or genesis)",
"wire protocol v < MIN", or for a consensus mismatch (`identity::explain`, at most FORKS_MAX + RULES + 3 hashes, close
path only): "peer has not scheduled activation (v,H): it must apply the published network upgrade" (the peer's
identity is a prefix of our schedule); "peer runs consensus rules r; this node runs rules R"; "peer runs a caps-test
build" (or the reverse); for an unpinned node, "this node runs an unpinned (development) build; the peer runs the
pinned rules: re-pin consensus-sources.lock or use a released build" or "... which peers only with byte-identical
trees"; otherwise "different consensus rules, fork schedule or test build (ours X…, theirs Y…). If this node has not
applied a published network upgrade, apply it (Settings → Upgrade or `pocol upgrade`) or update PoCol". A pinned node
cannot recognise an unpinned peer (it does not know the peer's aggregate), so it reports the generic text.
These three fields are in `pocol_status` and wallet status. `pocol_status` and `pocol_getParams` also report
`protocolVersion`, `protocolVersionMin`, `consensusRulesVersion`, `maxForkVersion`, `consensusIdentity` and
`consensusPinned` beside the informational `compatibilitySha256`; `consensusCompatibilitySha256` (pocol_getParams)
is a deprecated alias of `consensusIdentity`.

Self-connections. A listening node draws a random nonzero 64-bit nonce per process and sends it in every Hello on a connection that
can reach its listener (§1a.1; others send 0). A
first Hello carrying the receiver's own nonce is a self-connection: it is closed before any Status is kept, and
an address the receiver dialed for it is not dialed again by that process.

## 3. Block import pipeline (H-pre, H-full, envelope classes)
1. Decode header (strict, <= 3067 B) and body (<= BODY_MAX, canonical).
2. Duplicate blockHash → ignored. Known-invalid blockHash or invalid parent →
   blockReject (ancestorInvalid).
3. Unknown parent → orphan pool (<= 16 per parent) and `GetBlock(parent)` to
   the sender; orphans are retried when the parent imports.
4. H-pre on the parent's header metadata only (height, timestamp, base fee, gas
   used, gas limit, finality: pruning, slimming and offloading keep them; consensus
   D75): 2 linkage and 10a H_END; Future (ts > C+Φ) → future queue (<= 64), retried
   on every tick; 3 form and signature; 4 window; 5 target = ASERT(parent); 6 nonce
   < nMax; 7 PoW; 8 winnerSig recovery; 9 shares; 10 parent not final; 11 gas limit
   and base fee; the body binding (envReject) and body limits; a fallback's empty
   body (12). A verdict already reached with the parent's state for this TemplateID
   and nonce (`seal`, which every copy of one template and nonce shares, whatever
   its sig, winnerSig, body or share list) is returned from a local cache (256,
   FIFO): an H-full blockReject, or owner(nonce), so a copy whose winnerSig signer
   differs is envReject. The cache is never a validity input.
5. Only then the parent's state: outside the K_hist window → TrieRebuild
   (re-execution with root checks), then retry; a retained state held as diffs is
   materialized. A block refused after a rebuild leaves no rebuilt state behind.
   Deep rebuilds are rate-bounded (local policy): an unsolicited block draws on a
   token bucket (burst 8, one token back per second), a solicited one on its sync
   session's allowance (4; past it the session ends as not reached and its peer's
   claim is set aside). Exhaustion drops the block without a verdict: it is not
   recorded invalid, not journaled and costs its sender nothing; a real branch
   returns through a sync session (§4).
6. H-full 12–17 on the parent's state: proposer = Draw(h,0); winnerSig signer =
   owner(nonce) (envOwn, D130) before any execution; then a fallback's UT equal
   to Build_fb, evidence, caps, apply (chain.rs). Failures depending only on
   `sig` or `winnerSig` bytes are **envReject**: this copy is dropped, the
   blockHash stays acceptable from another copy and is not recorded as invalid.
   Every other failure is **blockReject**: the blockHash is recorded invalid
   and so are its descendants (ancestorInvalid).
7. Accepted block → fsynced journal record → in-memory insert → D133 head
   update → relay to all peers except the sender.

## 4. Synchronization
(D-REORG-REDB revision; network.md D38: Status, locator <= 32, count <= 512, <= 4096 per branch.)
- On connect, and on every new head, a node sends Hello (its Status, with the head's cumulative
  work). A feeler (§1a.7) sends no Hello.
- A peer announcing work >= ours with a head we do not store starts a sync session at its first Hello,
  unless one started within SESSION_GAP (1 s): then it is asked in its turn (one session at a time; later Hellos only update its Status, and claimants are asked in turn). `GetFrom(locator, 512)`: the locator is our head and the 9
  blocks below it, doubling steps, our canonical block at depth R_keep, and genesis. The responder
  answers its canonical blocks after the highest locator hash on its canonical chain, in height order,
  stopping when the requester's queue is half full. The next round puts the last delivered block first
  in the locator. Only a stored block that is new, or a stored copy at a new height inside the R_keep window, is
  progress; the next round follows a round with progress. The session ends when the peer's head is
  stored or our work exceeds its claim, after 3 s without progress, or at the protected-branch bound.
  The last two set the peer's claim aside until its announced head changes.
- A new block at or below head-R_keep is refused before validation (local retention; never invalid).
- Blocks a session delivers are solicited: they are taken even if retention removed them before (an
  unsolicited copy of such a block is refused before validation), and while non-canonical they are
  protected from the side cap (at most 2·R_keep).
- `GetRange` (start height, count) is still answered (canonical blocks in height order). `GetBlock`
  answers any stored block except genesis, an offloaded one from its journal record (byte-equal).
- Forks: blocks whose parents are unknown also recurse through `GetBlock(parent)` (short gaps); deep
  forks are fetched by the locator session; the heavier branch then wins by D133.

## 5. Templates and shares (consensus §4)
- Template: accepted only for the current parent at a=0 while in WAIT; checks
  signature, proposer = Draw(h,0), window, Φ, and that the UT equals the
  template rebuilt from apply(parent, body) (1b independence). Otherwise held
  (<= 64) for a parent not yet imported.
- Equivocation (type-1 evidence): a received template is observed only when its
  signature recovers to its proposer and Draw(h,0) is known without state: from
  the canonical block at that height (its `drawn`, the source evidence type 1 is
  checked against; this includes a template on a side parent whose signer is the
  canonically drawn proposer), or from a parent whose state is resident: the
  tip, or a side parent whose own Draw signed it (the pair is includable on that
  branch if it wins). It is observed only while its evidence can still be included (height +
  E_win >= head + 1, as evidence type 1 requires). Otherwise it is dropped (no
  record, no evidence, no penalty); observing a template never rebuilds or
  materializes state. An imported signed block's template is always recorded.
  The observations are kept per (parent, height, signer), so one signer's
  template never hides another's pair, and bounded (4096 keys); an overflow drops first the keys
  whose evidence can no longer be included, then all but the tip's (height above
  the head), so a flood of templates cannot hide a live equivocation.
- Share: accepted only for the locked TemplateID, `nonce < nMax`,
  `SHA256(TemplateID||be64(n)) <= T_share`; ownership follows from the range.
- Gossip (network.md:13, :15): a template is relayed to every peer except its
  sender, once per TemplateID, and only after it locked here (every check above
  passed). Those checks are deterministic on the parent state, so an honest relay
  never forwards a template that would be punished, and a punished sender is the
  peer that sent an invalid one. A new valid share for the locked template is
  relayed the same way, at most 4·S_max per template; an invalid, duplicate or
  unlocked one is dropped and never relayed. Fallback templates are built locally
  and never sent. A node that sees a second template from the same proposer
  (type-1 evidence, then the sticky fallback) relays neither that template nor the
  evidence yet, so each of an equivocator's templates spreads through the region
  that locked it, and only nodes that receive both switch to the fallback (evidence
  gossip, network.md, is future work).
- Shares and templates never change validity rules; they only feed the
  honest builder (shareList) and the proposer flow.

## 6. Bounds (local service policy)
Frame 256 KiB; inbound peers 32 established + 8 waiting for a Hello (seeds at the default, `--peers-max`
1..128), at most 4 per public address group plus one spare; inbound eviction (§1b) grace 30 s, one per key per
30 s, burst 8 then one per 2 s; relayed TemplateIDs
remembered 256; relayed shares 4·S_max per template; orphans 16/parent; future 64; held templates 64; GetRange 512;
rejects log 256; post-state verdicts per TemplateID+nonce 256; deep-rebuild budget (burst 8, one back per second, a
wall-clock step back delaying a refill by at most one second / session cap 4, for blocks on the session's branch; it
charges rebuilds of a missing parent state only, not slim restores of an entry's header and ranges); pool 32 MiB raw and <= 4096 transactions; RPC body 1 MiB, batch <= 20. Retention (D-REORG-REDB):
canonical blocks are kept whole; non-canonical blocks are metadata only (bytes in the journal),
at most R_keep unprotected plus 2·R_keep session-protected, none R_keep or more below the head;
states for K_hist canonical blocks and side_max side blocks. Peer discovery (§1a): book 1024 (8 per group,
2 ports per IP, 64 per answering peer, 256 self-advertised); Addr <= 64 records, one answer per inbound
connection, one Addr accepted per GetAddr within 120 s; automatic outbound <= 8 (auto 4 or 2) plus 1 spare;
configured + automatic <= 18; at most 2 seeds wanted; bootstrap addresses <= 16.

## 7. Verified scenarios
`prototypes/pocold/tests/e2e_devnet.py`: 3 processes, >= 100 H-full blocks,
late join, partition/heavier branch/heal, SIGKILL crash/restart, garbage
frames, wrong-network Hello, invalid block over P2P, RPC transactions,
identical hash/stateRoot/receiptsRoot at every height, deterministic replay.

`prototypes/pocold/tests/discovery.rs`, `prototypes/pocold/tests/e2e_discovery.py`: learning, answers,
self-connections, the book across restarts, outbound slots, seed release and loss, anchors across restarts,
the feeler without eviction, stall recovery, bootstrap with the seeds down.
