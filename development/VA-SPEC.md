# VA-spec v1 — PoCol peer validation and synchronization (local devnet)

Status: specification for LP6 written before/with the peer code in
`prototypes/pocold/src/net.rs` and `node.rs`; reviewed against consensus.md
§4, §8 (envelope classes D129–D140), §9 (D133) and §10 (D118). Local devnet
transport only (loopback); it is not a production P2P design.

## 1. Transport and framing
- TCP, loopback addresses only (inbound non-loopback peers are dropped).
- Frame = `u32 BE length || RLP([type, ...fields])`, `1 <= length <= 8 MiB`.
- Decoding is strict: canonical RLP, exact arity, minimal integers, fixed
  32-byte hashes, 8-byte nonces; re-encoding must equal the received bytes.
- Any malformed frame closes the connection. Malformed input never reaches
  block state, is never stored and never marks a block invalid.

| type | message | fields |
|---|---|---|
| 0 | Hello | chainId, genesisHash, headHash, headHeight, totalWork (32 B), nodeName |
| 1 | Block | headerBytes, bodyBytes |
| 2 | Template | signed-template shell (header encoding with nonce 0), body |
| 3 | Share | TemplateID, nonce (8 bytes) |
| 4 | GetBlock | blockHash |
| 5 | GetRange | startHeight, count (<= 512) |
| 6 | Tx | raw signed envelope |
| 7 | GetFrom | locator (1..32 concatenated 32-byte hashes), count (<= 512) |

## 2. Network identity
`Hello.chainId` and `Hello.genesisHash` must equal cfg; otherwise the peer is
closed (wrong network). activeVersion is checked per header (H-pre 1b).

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
  work).
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
- Shares and templates never change validity rules; they only feed the
  honest builder (shareList) and the proposer flow.

## 6. Bounds (local service policy)
Frame 8 MiB; orphans 16/parent; future 64; held templates 64; GetRange 512;
rejects log 256; pool 64 MiB raw; RPC body 1 MiB, batch <= 20. Retention (D-REORG-REDB):
canonical blocks are kept whole; non-canonical blocks are metadata only (bytes in the journal),
at most R_keep unprotected plus 2·R_keep session-protected, none R_keep or more below the head;
states for K_hist canonical blocks and side_max side blocks.

## 7. Verified scenarios
`prototypes/pocold/tests/e2e_devnet.py`: 3 processes, >= 100 H-full blocks,
late join, partition/heavier branch/heal, SIGKILL crash/restart, garbage
frames, wrong-network Hello, invalid block over P2P, RPC transactions,
identical hash/stateRoot/receiptsRoot at every height, deterministic replay.
