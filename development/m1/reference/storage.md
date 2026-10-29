# Web-file storage

**Alternatives**
- S1 (SSTORE): approximately 640 gas/B.
- S2 (calldata with a hash): does not meet the requirement because bytes are outside state.
- S3 (chunk-contract bytecode): approximately 220 gas/B.
- S4 (native layer with rent): permits deletion; does not run on anvil.

Selected (D10): S3 for the experiment; S4 is required for a permanent public network.

**Where data resides and how it is retrieved**
- Chunk-contract code resides in every full node's state.
- Retrieval uses eth_getCode, a light method: response ≤ 2·24576+64 bytes, approximately 48 KiB in hexadecimal. The extension worker calls it through ChunkFetcher, not the site bridge, so it consumes neither the bridge bucket nor its pending counter. It passes through RecvGuard with a 68 KiB per-response bound in CONTENT_POOL and the site session's ChunkCache (D98, D99).
- eth_getProof is available within K_eff; it is heavy and prohibited for sites (D95).

**Factory (D27)**
store(bytes data), where 1 ≤ |data| ≤ 24575:
- salt = keccak256(data).
- initcode = 61 LL LL 80 60 0a 5f 39 5f f3 ‖ 00 ‖ data, where LLLL = be16(1+|data|).
- runtime = 0x00‖data; address through CREATE2; operation idempotent.
- The client checks code[0]=0x00 and length.

**Manifest**
RLP([1, entryIndex, files]), where file = [path, mimeId u8, size u32, contentHash keccak256, chunks [[addr20, len u32]…]].
- 1 ≤ len ≤ 24575, Σlen = size ≤ 1 MiB.
- entry is html; files are strictly bytewise ordered.
- Path: ('/' seg)+, where seg = [A-Za-z0-9._~-]+, neither '.' nor '..', length ≤ 256, case-sensitive.
- MIME: 1 html, 2 css, 3 js, 4 json, 5 png, 6 jpeg, 7 webp, 8 svg, 9 woff2, 10 text.
- Normalization (used by the viewer and site_navigate, D96): query and fragment excluded from lookup. '%', '//', '.' and '..' give 404. Empty path or '/' = entry; '/x/' = '/x/index.html'.

**Website contract**
- Slots: 0 owner, 1 pendingOwner, 2 versionCount|currentVersion, 3 Version{manifestHash, manifestLen|status|publishedBlock, manifest chunks}, 4 publishers, 5 publisherCount.
- States: 0 draft, 1 published, 2 revoked.
- PUBLISHER: createVersion, publish, setCurrent.
- OWNER: publishers (≤ 16), revoke for noncurrent versions, two-step ownership transfer.
- Contract limits: manifest ≤ 64 KiB; versions ≤ 1024.
- Client limits: site ≤ 4 MiB, ≤ 256 files, parallel fetching ≤ 8 light connections to the local node, 10s timeout. −32021 responses are retried after retryAfterMs.

**Rejection vectors (T16)**
Duplicate, unordered, '.', or non-ASCII path; unknown mime; non-html entry; mismatched size, len or hash; another factory; noncanonical RLP; trailing byte; code[0] ≠ 0.

**Fees and time (assumptions)**
A chunk costs approximately 5.4M gas, or one chunk per block. A 1 MiB site needs approximately 47 blocks and 0.25 coins at 1 gwei.

**Versions and deletion**
- An update is a new version; rollback uses setCurrent; revoke prevents default display.
- No byte deletion before the end (EIP-6780); every version remains in history.

**Site-local storage (off-network, D34, D96)**
A site's localStorage state exists only in the browser, outside network state. Details and quota accounting are in browser. D103 defines immutable versioned records and recovery after worker disappearance.

**RetentionStore**
- Sizes: account 120 B; slot 150 B; trie node = 32+4+RLP; compressed receipt ≤ 54 B; log = 36+len.
- Indexes: txHash ≤ 64 B; height 40 B; SettlementIndex ≤ 242 B per block.
- Vectors:
  - RC1 = c7 01 82 52 08 80 80 80.
  - LG1 = d7 94 ‹20› c0 80, with 60 B storage.
  - LG2: 546 logs succeed; 547 halt.
  - LG3 = 193 B.
- GenesisSpec and alloc are preserved with the snapshot.
- Logs use (height, blockHash) keys for the canonical chain.
- LogCursor scans ascending over one snapshot without loading the range, encoding each matching log into the reserved buffer (D88). Per-block scanning memory ≤ 4 MiB is assumed (RG15).
- The redb snapshot is released at computation completion (≤ 5s). The redb cache is shared and fixed.
- SettlementIndex (D78): nonconsensus, keyed by (j, blockHash), served for the canonical branch and rebuilt on reorg.

**trie**
Live ≤ 447 B per entry; historical ≤ 1.6 GiB; P(d) = 568d+186+32(d+1).

**Retention**
Blocks, receipts, logs, code and indexes until the end, with a live trie, K_eff ≤ 32 and diffs for 2048 blocks.

**Inventory (worst case)**
Block count ≤ H_END=129600, an actual bound under E1. Total approximately 19.65 GiB logical and 29.5 GiB on disk.

Breakdown in GiB:
- Blocks: headers 0.37, evidence 0.99, transactions 8.00.
- Outputs: code 1.03, logs 1.00, receipts 0.50, indexes 0.63.
- State: accounts 0.11, slots 0.56, system 0.02.
- Trie and history: live trie 2.14, historical 1.60, diffs 1.92, side branches 0.76.

Closing (≤ Z_close=160) is within H_END. In end/full, inventory is derived with H_END=700 (CL-END).

**Duration and replicas**
- Retain until ts_F+30 days, then a signed snapshot.
- N_rep ≥ 3 on at least two physical machines. Fewer than two live replicas counted by ReplicaMonitor in pocol_getRetention (D119) displays “Retention requirement unmet” (T25).
- Rejected headers are not stored.

**Flood prevention**
- Write flooding: gas, caps, derived bounds, pool, faucet and item 10a.
- Read flooding: D88, D90, D92, D94.
- Site flooding through the bridge: D95, D96, D97 (integer rate bucket, pending counter and literal local-quota accounting).

**Funding**
One-time publishing fees. Operators bear fetching costs within D88 limits.
