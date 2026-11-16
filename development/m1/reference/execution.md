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
- GenesisBuilder(alloc, CP, M_0List) constructs genesis state and compares its root with allocRoot, supporting up to M_max members (10,000 on the fresh network).

**chainId**
- CHAINID = cfg.chainId.
- EIP-155, 2930 and 1559 transactions with another chainId: −32015 in the pool; inclusion invalidates the block under item 17.
- Legacy transactions without EIP-155 are rejected.

**Tips accumulator (D68)**
- Host-side u256, zeroed on entry.
- Add gasUsed·effectiveTip after every transaction.
- Pass in phase 2 as closeBlock(tips, issued) = selector‖be256(blockTips)‖be256(issuance(h)), length 68 bytes (finalize(tips) at F, 36 bytes), then destroy it.
- Only SYSTEM_ADDRESS can call closeBlock and finalize. No balance difference, SELFBALANCE or EIP-1153 is used.

**System contracts**
- MinerRegistry: 0x…C0C001.
- RewardEscrow: 0x…C0C002, with pending[j] = keccak256(be256(j)‖be256(3))+k, where k ∈ 0..2 (D141), and the carry in slots 4 and 5 (slot 4+(t & 1) holds the carry into pot t, D-V3-15):
  - k=0: amount.
  - k=1: kind (one byte) with proposer; INVALID at bit 168 (an ACCRUING accused winner, D-V3-16).
  - k=2: winner, with WIN_ACCRUING at bit 160; zero means not recorded. The zero address cannot be an identity: gsSys rejects it, and msg.sender cannot be zero.
- ShareLedger: 0x…C0C003.
- ChainCaps: 0x…C0C004.
- ChunkFactory: 0x…C0C005, without storage.
- PAY: 0x…C0C0FF, native.

MinerRegistry layout:
- Slots 0–4: RecLen, ActLen, sweepCur, block counters, heldTotal; slot 9: this block's maintenance and reactivation budgets (D-V3-08, D-V3-09).
- RecArr[i] = keccak256(be256(5))+i.
- ActList[i] = keccak256(be256(6))+i.
- Rec[id].w_k = keccak256(pad32(id)‖be256(7))+k, four words (incentives, Registry).

**Protection (D25)**
- SYSTEM_ADDRESS = 0xff…fe.
- ≤ 30M gas per call; ≤ G_SYS per block. A system-call revert invalidates the block.

**User functions** (calldata length / value / maximum writes; PoColSystemV3.sol, measured by tests/v3_contracts.rs):
- register(address): 36 / 0..B_reg / 10.
- registerReplacing(address reward, address victim): 68 / 0..B_reg / 13.
- topUp(): 4 / > 0 / 5 (4 without a promotion).
- deregister(): 4 / — / 7.
- removeInactive(address): 36 / — / 7.
- reactivate(): 4 / — / 6.
- setRewardAddress(address): 36 / — / 2.
- claimBond(): 4 / — / 10.

**Block phases**
- (0) recordShares(h−1) and recordResult(h−1) (skipped at h=1), then the body's evidence in body order (type 1: punish(accused, reporter); type 2: invalidate(j, accused) for a pot still pending, then punish; kind 3: recordLate(j, owners)), markClose (≤ RET records during closing, H_END ≠ 0), pruneShares, settle(h−D), then beginBlock (V3-02, V3-16, V3-17). The reporter is the block's signed proposer, zero on a fallback block; every SYSTEM call keeps tx.origin = that address.
  - recordShares(h−1) (D142): a system call writes count[h−1][m] in the existing ShareLedger layout for each owner m: its item count in ctx.parentResult.shares, plus 1 for winner (ν). It updates lastShare = h−1 for every such owner.
    - Owners ≤ S_max+1, keeping writes within the previously calculated bound.
  - recordResult(h−1) (D141): a system call writes pending[h−1].winner = ctx.parentResult.winner, with WIN_ACCRUING when the winner is a live ACCRUING record. It runs before the evidence, so evidence in block h cannot change what it records.
  - recordLate(j, owners) (D-V3-02): count[j][owner] and C_j rise by one per late share, and lastShare = max(lastShare, j); unlike recordShares it never reverts on lastShare > j.
  - At h ≥ 2, pending[h−1] always exists because h−1 is not final if it has a child.
  - Applies during closing because this is a system write, not a user write.
- (1) User transactions.
- (2) The host credits issuance(h) (M3, issuance.rs) to Escrow, then calls closeBlock(tips, issued): it pays the signed proposer floor(tips·gamma_tip_bp/10^4), drains the carry into pot h and writes pending[h] = {issued + tips − tipShare + carry, kind, proposer} with winner = 0, reading nothing from block h's result. At F (H_END ≠ 0 only), use finalize(tips): no issuance and no pending[F]; it burns the tips and both carries, then settleFinal settles the remaining pots (D141).
- (3) Roots.

**EVM fields**
- NUMBER=h, TIMESTAMP=ts, COINBASE = Escrow.
- PREVRANDAO = SHA256('PoColRandao'‖seal_p); not secure randomness. The block winner can bias it (seal and timestamp grinding persist; threat.md, Timestamp and ASERT; D-V3-27), so it is unsuitable for applications, lotteries or fair selection.
- BLOCKHASH in (h, n) notation: (10,0) → genesis, (257,0) → 0, (257,1) → hash(1), (300,44) → hash(44), (h,h) → 0.
- BLOBHASH=0, BLOBBASEFEE=1, without EIP-4788.

**Caps**
- CODE_CAP and B_code_max: code deposit fails.
- SLOT_CAP, ACCT_CAP, DIRTY_BLOCK_MAX=4096, ACCT_DIRTY_BLOCK_MAX=2048, the per-block log caps (256 KiB of charged log bytes and 4,368 logs, V3-24) and LOG_CAP=1 GiB of charged log bytes (over the branch when H_END ≠ 0; with H_END = 0 it counts each block from zero): exceptional halt.
- TXBYTES_CAP and TX_CAP (item 16: over the branch when H_END ≠ 0, per block when H_END = 0): block invalid; the builder skips the transaction.
- Node-local (PR6, not a validity rule): the honest builder counts each transaction's gas used and skips one
  whose gas_limit exceeds GAS_LIMIT minus the gas used so far (the reservation rule above it in the USER
  phase), with its sender's later ones; it also leaves out a transaction that would halt on the log caps or on
  SLOT_CAP/ACCT_CAP, except one whose own logs pass a log cap, which halts in any block and is included as that
  halt (the pool refuses such a transaction when admission sees it).
- The per-block log caps (V3_LOG_BLOCK_MAX = 256 KiB, V3_LOG_COUNT_MAX = 4,368; user_phase.rs) bound the receipts and logs that lemmas P1 and P3 rest on (D89); one transaction may emit all of them, which is why a receipt is served heavy (network.md).

**System resources (D50, D54, D70)**
- User context: SYS_KEYS_USER_MAX keys and 16 accounts; zero during closing, otherwise SysWriteClosing.
- Distinct SYS keys and accounts per block are derived from the CP by resources::derive on CP schema v2 and checked by ParamGate R18 against SYS_KEYS_BLOCK_MAX and SYS_ACCT_BLOCK_MAX (4,096 and 512 on the fresh network). The derivation counts each settle payee's accrual writes (its bond and state word, and heldTotal), the tip share's payTo and the carry.
- SYS gas: the worst call sequence of each phase class, composed from GAS_ENVELOPE_V3 (admission.rs: per-call maxima measured on the v3 programs, for example settle 11,957,000, recordShares 14,200,000 and recordLate 3,458,000, each below the 30M per-call cap), counted against G_SYS (96,000,000 on the fresh network). The envelope holds only inside the measured gas domain (S_max+3 ≤ 259 payees per pot, κm ≤ 64, RET ≤ 128, D ≤ 16, E_max ≤ 4); outside it the facts are absent and admission records an R18 condition. No v3 SYSTEM call iterates the members, so M_max is not a domain field.
- (D141) A pending pot keeps three words, so moving the winner write to recordResult(h−1) adds no key: two words for pending[h] and one for pending[h−1] in an ordinary block.

**Accounts and transactions**
- secp256k1 with EIP-155. Accepted: legacy-155, 2930 and 1559. Types 3 and 4 return −32015.
- GAS_LIMIT=8M with a 4M target; London baseFee formula with the floor baseFeeMin (10^7 wei on the fresh network), which empty blocks reach; the base fee is read from the header.

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
PAY does not notify the recipient, block-F tips are burned (H_END ≠ 0 only), and forced transfers are locked.

EVM compatibility does not confer Ethereum consensus security.
