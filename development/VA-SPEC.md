# VA-spec v1 — PoCol peer validation and synchronization (experimental TCP transport)

Status: specification for LP6 written before/with the peer code in
`prototypes/pocold/src/net.rs` and `node.rs`; reviewed against consensus.md
§4, §8 (envelope classes D129–D140), §9 (D133) and §10 (D118). An experimental
TCP transport (devnets and testnets, loopback or remote peers); it is not the
production P2P design of network.md (libp2p, Noise, gossipsub).

## 1. Transport and framing
- TCP on the operator-selected listener: a loopback bind stays local; a private or public bind accepts remote
  peers, each of which must pass the Hello identity check (§2). Outbound connections go to the configured peers
  (one dialer each; seeds among them follow §1a.8) and, with automatic outbound on (§1a.7; default off), to
  learned addresses.
- Inbound limits (local policy, `net::Limits`): at most `peers_max` connections, and at most
  `per_group_max` (default 4, half of `peers_max`; `--per-group-max`, or `POCOL_PER_GROUP_MAX` for
  `run`; 0 off) per public address group: IPv4 /16, IPv6 /32, an IPv6 address carrying an IPv4
  address (6to4, Teredo, NAT64, mapped) counting as that address, and Yggdrasil 200::/7 (key-derived)
  as one group. So one public /16 or /32 never holds more than half of the inbound slots. A
  connection past the limit is refused even while other slots are free: nodes behind one NAT or one
  provider range share these places until inbound eviction exists. Groups are counted per
  address, so one host that reaches the node from several groups (an IPv4 /16 and an IPv6 /32, say)
  holds a full set of places in each. Addresses that are not globally reachable (`net::group_of`:
  loopback, private, link-local, shared 100.64/10, benchmarking, documentation, reserved,
  unique-local fc00::/7 including cjdns, ...) are not grouped, and neither are the node's configured
  peers, by their exact address.
- Frame = `u32 BE length || RLP([type, ...fields])`, `1 <= length <= MAX_FRAME` (256 KiB).
- Decoding is strict: canonical RLP, exact arity, minimal integers, fixed
  32-byte hashes, 8-byte nonces; re-encoding must equal the received bytes.
- Any malformed frame closes the connection. Malformed input never reaches
  block state, is never stored and never marks a block invalid.

| type | message | fields |
|---|---|---|
| 0 | Hello | chainId, genesisHash, headHash, headHeight, totalWork (32 B), nodeName, protocolVersion, compatibility (32 B), listenPort (0..65535; 0: none), nonce (8 bytes; 0: none) |
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
   since the entry's last counted failure. Closed before any Hello (a full listener, say) is no failure.
   Another release holds an entry back min(10 min · 2^(n-1), 6 h); another chain or genesis removes it.
5. Answers: GetAddr is answered once per inbound connection and never on a connection this node dialed, with
   an Addr of at most 64 entries (possibly none): entries verified or self-advertised within 7 days, verified
   first, at most 2 per address group, never the requester's own advertised address, local entries only to a
   local requester, from a list rebuilt at most once a minute (an entry that leaves the book leaves the list
   at once); it is queued only while the requester's queue
   is below half (§4). A node sends GetAddr once per connection it dialed (not feelers).
6. Persistence: `<data dir>/addrbook`, bound to chainId and genesis and checksummed, written atomically 60 s
   after start, every 15 min, when the anchors change and at a clean stop, read with a 1 MiB bound. Every
   entry read passes 3 and 4 again; a damaged, foreign or oversized file is ignored (empty book). These
   periods, the refresh of live entries and the 120 s of 2(c) run on the wall clock: when it steps back, no
   deadline stays more than its period ahead, and a GetAddr sent "in the future" expires.
7. Automatic outbound, default off (`--outbound N`, 0..8; 0 until inbound eviction exists): N slots, one
   connection each, at most one per address group, never to a configured peer, a seed, this node or an
   address already connected. Choice: verified or unverified entries 50/50 when both exist; verified by
   group, unverified by answering peer (all self-advertisements one source), then group; fewest failures
   first. An address is retried 10 min after a failed or short (< 60 s) attempt; a slot waits
   min(1 s · 2^k, 60 s) after k failures; an unreachable address family is skipped for 1 h. With all N slots
   up, one feeler every 2 min (10 min if not listening) connects to an unverified entry, sends a Hello
   announcing genesis, verifies the address and closes; it carries no gossip, sync or GetAddr. Up to 2
   anchors (automatic peers connected 10 min that delivered a block, distinct groups) are dialed first after
   a restart. Inbound + configured + automatic + feeler <= 50 (network.md:156), checked at boot.
8. Seeds (automatic outbound, default off: `--seed`, the invitation's seeds, with N > 0): all are dialed for
   the first minute. Then a node keeps one seed connection while it does not listen, has fewer than 2 useful
   automatic peers, or is stalled; a listening node closes it after 2 useful peers have held 2 min. Useful:
   an automatic peer outside every seed's address group that delivered a valid block at height >= head-2 or a
   template that locked within W = max(120 s, 12·T_blk); stalled: no new head from a peer within W. A seed
   not connected within 30 s, or held while stalled for W, is replaced by the next. A node that finds a seed
   is itself (§2, or its exact `--listen` address) keeps all other seeds. With N = 0 seeds are configured
   peers.

## 2. Network identity
`Hello.chainId` and `Hello.genesisHash` must equal cfg, and `Hello.protocolVersion` and
`Hello.compatibility` must match this build for the profile's fork schedule
(`identity::matches_for`); otherwise the peer is closed (wrong network or implementation).
activeVersion is checked per header (H-pre 1b).

A listening node draws a random nonzero 64-bit nonce per process and sends it in every Hello on a connection that
can reach its listener (§1a.1; others send 0). A
first Hello carrying the receiver's own nonce is a self-connection: it is closed before any Status is kept, and
an address the receiver dialed for it is not dialed again by that process.

## 3. Block import pipeline (H-pre, H-full, envelope classes)
1. Decode header (strict, <= 3067 B) and body (<= BODY_MAX, canonical).
2. Duplicate blockHash → ignored. Known-invalid blockHash or invalid parent →
   blockReject (ancestorInvalid).
3. Unknown parent → orphan pool (<= 16 per parent) and `GetBlock(parent)` to
   the sender; orphans are retried when the parent imports.
4. Future (ts > C+Φ) → future queue (<= 64), retried on every tick.
5. Parent state outside the K_hist window → TrieRebuild (re-execution with
   root checks), then retry.
6. H-pre 1–9 and H-full 10–17 (chain.rs). Failures depending only on `sig`
   or `winnerSig` bytes are **envReject**: this copy is dropped, the blockHash
   stays acceptable from another copy and is not recorded as invalid.
   Every other failure is **blockReject**: the blockHash is recorded invalid
   and so are its descendants (ancestorInvalid).
7. Accepted block → fsynced journal record → in-memory insert → D133 head
   update → relay to all peers except the sender.

## 4. Synchronization
(D-REORG-REDB revision; network.md D38: Status, locator <= 32, count <= 512, <= 4096 per branch.)
- On connect, and on every new head, a node sends Hello (its Status, with the head's cumulative
  work). A feeler (§1a.7, automatic outbound, default off) sends one Hello announcing genesis
  (height 0, work 0) and no refresh.
- A peer announcing work >= ours with a head we do not store starts a sync session at its first Hello
  (one session at a time; later Hellos only update its Status, and claimants are asked in turn). `GetFrom(locator, 512)`: the locator is our head and the 9
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
Frame 256 KiB; inbound peers 8, at most 4 per public address group; relayed TemplateIDs
remembered 256; relayed shares 4·S_max per template; orphans 16/parent; future 64; held templates 64; GetRange 512;
rejects log 256; pool 32 MiB raw and <= 4096 transactions; RPC body 1 MiB, batch <= 20. Retention (D-REORG-REDB):
canonical blocks are kept whole; non-canonical blocks are metadata only (bytes in the journal),
at most R_keep unprotected plus 2·R_keep session-protected, none R_keep or more below the head;
states for K_hist canonical blocks and side_max side blocks. Peer discovery (§1a): book 1024 (8 per group,
2 ports per IP, 64 per answering peer, 256 self-advertised); Addr <= 64 records, one answer per inbound
connection, one Addr accepted per GetAddr within 120 s; automatic outbound (default off) <= 8 plus 1 feeler;
all connections <= 50.

## 7. Verified scenarios
`prototypes/pocold/tests/e2e_devnet.py`: 3 processes, >= 100 H-full blocks,
late join, partition/heavier branch/heal, SIGKILL crash/restart, garbage
frames, wrong-network Hello, invalid block over P2P, RPC transactions,
identical hash/stateRoot/receiptsRoot at every height, deterministic replay.

`prototypes/pocold/tests/discovery.rs`, `prototypes/pocold/tests/e2e_discovery.py`: learning, answers,
self-connections, the book across restarts, and (automatic outbound, default off) outbound slots, seed
release and loss.
