# Contracts and EVM compatibility

**Separation of consensus and execution**

apply(S_parent, ctx, body, parentHeader) → (S, receipts, roots, B_old, settlements, blockTips).

ctx = {chainId=cfg.chainId, h, ts, baseFee, gasLimit, prevrandao, coinbase=Escrow, ancestors, closing, closeStart, gate2, final, parentResult}.
- parentResult (D141, D142) = complete ParentResult(p): {blockHash(p), winner = owner(p.nonce), shares[(n, owner(n))]}.
  - Sole source: Header(p) bytes stored in HeaderStore under blockHash(p) (nonce and shareList), plus p's assignment table computed over grandparent state.
  - Never read from mining memory, the share pool or a copy unbound to parentHash.
  - Computed when the parent passes items 13 and 14, and stored as a derived value in BlockMeta under blockHash(p).
  - On reexecution (synchronization, TrieRebuild and reorg), rederive it from Header(p) and grandparent state and compare with the stored value. A mismatch is an implementation error (parentResultMismatch) that stops import.
  - Independent of winnerSig bytes: every valid envelope yields the same owner.
  - Empty at h=1.
- ctx includes no block-h field outside UT and the body: no nonce, owner, shareList or either signature.

- Consensus sees EVM only through apply; target does not enter ctx.
- GenesisBuilder(alloc, CP, M_0List) constructs genesis state and compares its root with allocRoot, supporting up to M_max (1024 in end/full).

**chainId**
- CHAINID = cfg.chainId.
- EIP-155, 2930 and 1559 transactions with another chainId: −32015 in the pool; inclusion invalidates the block under item 17.
- Legacy transactions without EIP-155 are rejected.

**Tips accumulator (D68)**
- Host-side u256, zeroed on entry.
- Add gasUsed·effectiveTip after every transaction.
- Pass in phase 2 as selector‖be256(blockTips), length 36 bytes, then destroy it.
- Only SYSTEM_ADDRESS can call closeBlock and finalize.
- No balance difference, SELFBALANCE or EIP-1153 is used.

**System contracts**
- MinerRegistry: 0x…C0C001.
- RewardEscrow: 0x…C0C002, with pending[j] = keccak256(be256(j)‖be256(3))+k, where k ∈ 0..2 (D141):
  - k=0: amount.
  - k=1: kind (one byte) with proposer.
  - k=2: winner; zero means not recorded. The zero address cannot be an identity: gsSys rejects it, and msg.sender cannot be zero.
- ShareLedger: 0x…C0C003.
- ChainCaps: 0x…C0C004.
- ChunkFactory: 0x…C0C005, without storage.
- PAY: 0x…C0C0FF, native.

MinerRegistry layout:
- Slots 0–4: RecLen, ActLen, sweepCur, block counters, heldTotal.
- RecArr[i] = keccak256(be256(5))+i.
- ActList[i] = keccak256(be256(6))+i.
- Rec[id].w_k = keccak256(pad32(id)‖be256(7))+k.

**Protection (D25)**
- SYSTEM_ADDRESS = 0xff…fe.
- ≤ 30M gas per call; ≤ G_SYS per block.
- A system-call revert invalidates the block.

**User functions** (calldata length / value / maximum writes):
- register(address): 36 / B_reg / 9.
- deregister(): 4 / — / 7.
- removeInactive(address): 36 / — / 7.
- reactivate(): 4 / — / 5.
- setRewardAddress(address): 36 / — / 1.
- claimBond(): 4 / — / 9.

**Block phases**
- (0) applyEvidence, markClose (≤ RET records during closing), pruneShares, recordShares(h−1), recordResult(h−1), settle(h−D), then beginBlock.
  - recordShares(h−1) (D142): a system call writes count[h−1][m] in the existing ShareLedger layout for each owner m: its item count in ctx.parentResult.shares, plus 1 for winner (ν). It updates lastShare = h−1 for every such owner.
    - Owners ≤ S_max+1, keeping writes within the previously calculated bound.
  - recordResult(h−1) (D141): a system call writes pending[h−1].winner = ctx.parentResult.winner.
  - Skipped at h=1.
  - At h ≥ 2, pending[h−1] always exists because h−1 is not final if it has a child.
  - Applies during closing because this is a system write, not a user write.
- (1) User transactions.
- (2) closeBlock(tips): writes pending[h] = {amount, kind, proposer} with winner = 0, reading nothing from block h's result. At F, use finalize(tips); no pending[F] is created (D141).
- (3) Roots.

**EVM fields**
- NUMBER=h, TIMESTAMP=ts, COINBASE = Escrow.
- PREVRANDAO = SHA256('PoColRandao'‖seal_p); not secure randomness.
- BLOCKHASH in (h, n) notation: (10,0) → genesis, (257,0) → 0, (257,1) → hash(1), (300,44) → hash(44), (h,h) → 0.
- BLOBHASH=0, BLOBBASEFEE=1, without EIP-4788.

**Caps**
- CODE_CAP and B_code_max: code deposit fails.
- SLOT_CAP, ACCT_CAP, DIRTY_BLOCK_MAX=4096, ACCT_DIRTY_BLOCK_MAX=2048, LOG_BLOCK_MAX=32 KiB and LOG_CAP=1 GiB: exceptional halt.
- TXBYTES_CAP and TX_CAP: block invalid; the builder skips the transaction.
- LOG_BLOCK_MAX together with the LG2-derived log-count bound (546) underpins lemmas P1 and P3 (D89).

**System resources (D50, D54, D70)**
- User context: 256 keys and 16 accounts; zero during closing, otherwise SysWriteClosing.
- Derived bounds: KN=1752, KC=1482, KF=1395, AN=404, AC=388, AF=257.
- Estimated gas (measured by T14): about 26M for ordinary blocks and 37M for final blocks.
- (D141) Distinct keys per block do not change when the winner write moves:
  - Ordinary block: two words for pending[h], one for pending[h−1], instead of three for pending[h].
  - Block F: pending[F−1]'s winner key is already among those deleted by finalize.
  - Thus KN, KC and KF are unchanged. ParamGate.derive rechecks them in M4; T14 measures cost.

**Accounts and transactions**
- secp256k1 with EIP-155. Accepted: legacy-155, 2930 and 1559. Types 3 and 4 return −32015.
- GAS_LIMIT=8M with a 4M target; London baseFee formula. In empty blocks it stabilizes below 8 wei and is read from the header.

**RPC**
- hash = blockHash, miner = winner, nonce = be64, difficulty = work(target).
- eth_call and eth_estimateGas are heavy, bounded to 30M gas, on a snapshot in a compute thread with SCRATCH ≤ 16 MiB and a 5s deadline (D88).
- Both simulate: no state commitment, pool writes or broadcasting. This justifies inclusion in the read matrix (D95), with state override rejected and to required.
- EVM memory at 30M gas is ≤ about 3.9 MiB (3n+n²/512). Read-state memory uses the same counter.

**Testing**
- Cancun GeneralStateTests except declared exceptions, followed by T2b and T2c.
- CI1: CHAINID=777902.
- CI2: chainId=777001 returns −32015.
- CI3: a block containing CI2's transaction is rejected under item 17.
- GB1: heldTotal = 4·B_reg, ActList = M_0 in order, RecLen = 4.
- GB-END (D86): RecLen = ActLen = 1024, heldTotal = 1024·B_reg.

**Automated inspection (M4)**
onlyNotClosing, PAY for every transfer, no SSTORE in ChunkFactory, CALLER as the id source, and tips from calldata.

**Reused components**
- revm with PAY, alloy, k256, sha2/sha3, redb and rust-libp2p.
- hyper for HTTP/1.1 instead of jsonrpsee, because D88, D90, D92 and D94 need control of flow, reservation, sequencing, sending clock, socket buffer and sendDone.
- tokio (two runtimes) with a fixed compute-thread pool; socket2 for SO_SNDBUF.
- ruint or crypto-bigint for fixed 512-bit width (D83); no num-bigint in ASERT.
- In the extension: a strict JSON parser for BridgeAuth, preferably hand-implemented over JSON.parse with type checks rather than a general schema library. dependency-cruiser (MIT) checks imports.
- Foundry and solc are build tools only; no geth code. Licenses are checked in M0.

**Wallets**
MetaMask through custom RPC (T15) and the embedded wallet (D14). The embedded wallet is the extension's only source of eth_sendRawTransaction (D95).

**Declared deviations**
PAY does not notify the recipient, block-F tips are burned, and forced transfers are locked.

EVM compatibility does not confer Ethereum consensus security.
