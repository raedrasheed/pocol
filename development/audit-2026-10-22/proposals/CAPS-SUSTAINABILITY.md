# CAPS-SUSTAINABILITY — LOG_CAP, TX_CAP and TXBYTES_CAP over a chain's life

> **Status: PROPOSED — not approved.** Every normative change below exists only in this file. The M1
> reference is not edited. LOG-CAP-AGGREGATE and the other five consensus proposals stay unapproved.
> Only conformance items are implemented (§6): the builder's skip (execution.md:83), template locking
> (consensus.md:101, §4(a)), and the caps-test build constant LOG_CAP (validation.md:2065; its
> HIST_TRIE_BUDGET is not, §6).
>
> Revised after the independent review (round 2, branch `wf/caps-r2`; corrections in §7), after the
> second review (round 3, branch `wf/caps-r3`; corrections in §7b) and after the third review (round 3b,
> branch `wf/caps-r3b`; corrections in §7d).

This study builds on [LOG-CAP-AGGREGATE.md](LOG-CAP-AGGREGATE.md) and
[LOG-CAP-AGGREGATE-ALTERNATIVES.md](LOG-CAP-AGGREGATE-ALTERNATIVES.md). The alternatives file
recommends A1a for LOG_CAP (2^32 plus gate R27), and its §6 flags TX_CAP and TXBYTES_CAP without
analysing them. This file analyses all three caps together, measures what happens at exhaustion, and
gives one integrated recommendation.

Numbers come from the independent model `model/caps_model.py`. It uses Python's standard library only,
runs with `python3 -I`, and writes deterministic output to `vectors/caps-sustainability.json`. Code
references are to `wf/caps` at the commits named in §6. At the integrated head fa97594 the chain.rs,
user_phase.rs, apply.rs and resources.rs lines cited here are unchanged. Those that moved are:
- node.rs: `submit_tx` at :400, its pool-full check at :406 (cited as :404), `PoolMeter::admit` at
  :410, and the caps check of `try_lock_signed` at :559 (cited as :557);
- rpc.rs: `simulate` (eth_call, eth_estimateGas) at :859 (cited as :439), and `find_tx` (the linear
  scan behind eth_getTransactionByHash) at :887 (cited as :455).

## 1. The three caps

| | LOG_CAP | TX_CAP | TXBYTES_CAP |
|---|---|---|---|
| text | execution.md:82: "LOG_CAP=1 GiB: exceptional halt" | execution.md:83: "the block is invalid, and the builder skips the transaction"; consensus.md:170, item 16: "txBytes and txCount within the caps" | the same, plus D49 (FINAL_DESIGN.md:5168): "TXBYTES_CAP on len(rawTx)" |
| kind | protocol constant, **not in CP** | CP field 38 (u64), consensus.md:23 | CP field 37 (u64), consensus.md:23 |
| scope | cumulative per branch: the charged log bytes of the parent's canonical ancestors plus this block (the provisional "chain" reading of LOG-CAP-AGGREGATE) | cumulative per branch: the parent's count plus this block's (code reading, A13-DERIVATION.md:42; storage.md:76–77 budgets receipts and indexes for 10^7 transactions) | cumulative per branch: the parent's bytes plus this block's (storage.md:76 "transactions 8.00 GiB" = 2^33) |
| unit | 36 + 32·topics + \|data\| per log, logs of successful frames only (LOG-ACCT-01 (c), provisional) | 1 per transaction in the body | raw transaction bytes, without the RLP item header |
| response | the transaction halts (status 0, whole gas limit charged, nonce and fee kept) | the block is invalid; the builder skips | the same |
| network | 2^30 | 10^7 | 2^33 |
| devnet (devnet.rs:36–42, not normative) | 2^30 | 20,000 | 2^24 |
| caps/64 (validation.md:2061–2062) | 2^30 | 20,000 | 2^24 |
| caps-test (validation.md:2065) | **2^24** | 20,000 | 2^24 |
| code | `user_phase.rs:24–25` (constant), `:197` (check), `:217` (halt); `chain.rs:344` (`cum_log_bytes`) | `chain.rs:335` → `Chain::within_caps` `:351`; `apply.rs:250` checks only one block's count against TX_CAP (labelled "TX_MAX") | `chain.rs:335`, `:351` |

The spec never says "cumulative" in so many words. That reading follows from three things: the
inventory, the fact that the caps-test build makes LOG_CAP reachable, and the vacuity of a per-block
reading (LOG_BLOCK_MAX ≤ 32 KiB < LOG_CAP; BODY_MAX < TXBYTES_CAP).

The counters are per branch because each one is derived from the parent block (`Entry.cum_*`, set in
`validate`).

## 2. Reachability before H_END

**Per-block maxima.** All four profiles share GAS_LIMIT = 8M, BODY_MAX = 196,608, TX_MAX = 131,072 and
LOG_BLOCK_MAX = 32 KiB.

| quantity | per block | how it is reached | gas |
|---|---:|---|---:|
| transactions | 380 = ⌊8,000,000 / 21,000⌋ | gas binds; bytes would allow 1,927 of the 100-byte minimal EIP-155 transfer | 7,980,000 (190 at the 4M target: 3,990,000) |
| raw tx bytes | 196,592 = 131,072 + 65,520 | two transactions whose encoded body is exactly BODY_MAX | 827,528 (zero calldata, 4 gas/byte); nonzero bytes still stay under 4M |
| charged log bytes | 32,768 | one LOG0 of 32,732 zero bytes (36 + 32,732) | 288,349 |

**Base-fee rule.** London (execution.md:97). With t = GAS_LIMIT/2 = 4M:
- above the target: b' = b + max(1, ⌊b·(u−t)/t/8⌋);
- below it: b' = b − ⌊b·(t−u)/t/8⌋.

The base fee is burned (economics.md:66, incentives.md:68).

At u ≤ t it never rises, and it falls to its floor: below 8 wei for empty blocks, and below 8t/(t−u) in
general. Filling TXBYTES_CAP or LOG_CAP needs 21% and 7% of the target, so the attacker pays the idle
base fee.

Filling TX_CAP faster than the target needs blocks above it, and those compound the base fee by about
×1.124 per full block. Starting from 7 wei, after 212 full blocks a single block costs one block subsidy
(2·10^18 wei, governance.md:9). The sustainable rate is therefore the target, about 190 transactions per
block.

Empty blocks are free and cut the base fee by ⌊b/8⌋ each: from baseFee0 = 10^9 it reaches the 7 wei
floor after **145** empty blocks (exact integer simulation; 2.4 h on the network profile, 580 s on the
devnet). A patient attacker therefore always pays the idle-base-fee column below. The "from 1 gwei"
column is the cost of one particular strategy, filling at the target from baseFee0 without waiting
first. At 3.99M gas the base fee falls only ×(1 − 1/3,200) per block, so that strategy is the
expensive one, not a bound.

Priority fees are not specified. The model assumes 0, because pocold's pool is FIFO and a proposer may
include its own transactions. Costs are the burned base fee, simulated exactly in integers. The
starting base fee is either the idle 7 wei or baseFee0 = 10^9. 1 coin = 10^18 wei.

| profile | cap | fastest (blocks, time) | at the 4M target | before H_END | organic: per-block average that exhausts it exactly at H_END | attack cost from 7 wei / from 1 gwei |
|---|---|---|---|---|---|---|
| network (T_blk 60 s, H_END 129,600) | TX_CAP 10^7 | 26,316 (18.3 d), unsustainable | 52,632 (36.6 d) | **yes** | 77.2 tx (20.3% of max; 1.29 tx/s) | 1.47·10^12 wei / 12.8 coins |
| | TXBYTES_CAP 2^33 | 43,695 (30.3 d) | same | **yes** | 66,280 B (33.7%) | 2.5·10^11 wei / 0.0083 coins |
| | LOG_CAP 2^30 | 32,768 (22.8 d) | same | **yes** | 8,285 B (25.3%) | 6.6·10^10 wei / 0.0025 coins |
| devnet (4 s, 20,000) | TX_CAP 20,000 | 53 (212 s) | 106 (424 s) | **yes** | 1.0 tx (0.26%) | 3.0·10^9 wei / 0.42 coins |
| | TXBYTES_CAP 2^24 | 86 (344 s) | same | **yes** | 839 B (0.43%) | 5.0·10^8 wei / 0.0083 coins |
| | LOG_CAP 2^30 | 32,768 > H_END | — | no | 53,687 B > the 32,768 maximum | — |
| caps/64 (10 s, 4,000) | TX_CAP | 53 (530 s) | 106 (1,060 s) | **yes** | 5.0 tx (1.3%) | as devnet |
| | TXBYTES_CAP | 86 (860 s) | same | **yes** | 4,194 B (2.1%) | as devnet |
| | LOG_CAP 2^30 | 32,768 > H_END | — | no | — | — |
| caps-test (10 s, 4,000) | LOG_CAP 2^24 | 512 (5,120 s) | same | **yes** | 4,194 B (12.8%) | 1.0·10^9 wei / 0.0025 coins |

**Reading.**
- On the network profile, each cap is exhausted organically before H_END by a sustained average of
  1.29 transactions per second, 66 KB of transactions per block, or 8.3 KB of logs per block.
- An attacker exhausts each within 23–37 days, for under 10^-5 coins (about 1.5·10^12 wei for TX_CAP)
  at the idle base fee. From baseFee0 = 10^9, 145 empty blocks reach that fee. A higher starting base
  fee needs about 5.2 more empty blocks per doubling (exact integer simulation: 150 blocks from 2·10^9,
  162 from 10^10, 196 from 10^12).
- A target-rate attacker who starts at baseFee0 = 10^9 without waiting pays 12.8 coins (6.4 block
  subsidies) for TX_CAP. That is the cost of a worse strategy, not an upper bound on the attack.
- On the devnet and caps/64, the TX caps fall to 53 and 86 blocks of full load.

**Correction to LOG-CAP-AGGREGATE-ALTERNATIVES.md §6.**
- TXBYTES_CAP needs at least **43,695** blocks, not 43,691. A block carries at most 196,592 raw bytes,
  not BODY_MAX.
- At 190 transactions per block (3.99M gas, just under the target) the base fee falls slowly
  (×(1 − 1/3,200)); it is not flat.

## 3. Effects at exhaustion

### 3a. TX_CAP or TXBYTES_CAP reached on a branch

This section covers a branch whose remaining room fits no pooled transaction.

| subject | spec-intended | code before `9094f98` | code now |
|---|---|---|---|
| user transactions of every kind (transfers, contract calls and creations, the registry's register / deregister / claimBond) | none can be included on this branch (item 16) | same validity | same |
| builder (`template_txs`) | skips the transaction (execution.md:83) | the template carried every pooled transaction (see the two rows below) | skips what passes the room, with that sender's later transactions; signed empty blocks at the normal pace. Exact at the boundary in every position tested: the first, second and third transaction of a block, on a zero and a non-zero parent counter, for both counters (round 3b, §7d) |
| …past TXBYTES_CAP, or past TX_CAP while one block's count still fits it (the case on every real profile) | — | own block invalid ("16 caps"); every height waited until U(0)+Γ, i.e. D_att+Γ after the parent's timestamp (consensus.md:93, :97), for a fallback. Measured: 74–78 s per block on the tests/caps.rs test devnet (D_att 73, single-host Γ 5). Derived, not run: 30 s on the default devnet profile (D_att 25 + Γ 5, devnet.rs:37), about 330 s on the network profile (D_att 300 + Γ 30, governance.md:6) | — |
| …more pooled transactions than TX_CAP | — | `apply_block` refused the block (`Budget("TX_MAX")`, apply.rs:250: one block's count against the whole TX_CAP); the builder emptied it; **nothing was ever included, although room existed**. Reach: only when TX_CAP is below one block's transaction count, which gas bounds at 380 (the pool at 4,096). It cannot occur on the network (10^7), devnet or caps/64 (20,000) profiles; it was reproduced with TX_CAP 3 and 5 | — |
| received signed templates | a violating template counts as absent, then sticky FB at T_j (consensus.md:101, §4(a)) | locked and mined, wasting attempt 0 | not locked (`node.rs:557`) |
| validation, sync, journal replay | item 16 against the parent's counters | same | same, with one definition (`Chain::within_caps`) |
| system phases, rewards, settlement | not transactions, so unaffected | continue; pots are the subsidy (no tips) | same |
| funds | — | reward balances, and the bonds refunded at closing (`markClose`), can no longer be spent on this chain | same |
| mempool | not specified: network.md:149–153 lists −32014, −32015, and −32016 for "full" and "after F" | accepts; transactions that can never be included stay until POOL_MAX (4,096 or 32 MiB), then −32016 "pool full" | same; proposal N5 |
| …free pool squatting near the TXBYTES boundary (before full exhaustion) | not specified | — | Once the room left is below TX_MAX (131,072 B), a transaction larger than the room is skipped but stays pooled (`template_txs`). It is never included, so never charged, and the pool has no expiry and no eviction (`submit_tx`, node.rs:404). About 256 such 131,072-byte transactions fill POOL_MAX_BYTES (32 MiB; or 4,096 by count). The pool then answers −32016 to small transactions that would still fit the room, and the squatters are relayed to peers. The cost is only the balance needed to pass execution on the pending state, never spent. Every new head also re-executes the stuck pool (`rebuild_pool`), bounded by POOL_MAX. Code inspection; not run. Proposal N5 |
| RPC | — | eth_sendRawTransaction returns a hash whose receipt stays null; `pocol_getCaps` (network.md:333) answers −32601 | same; proposal N6 |
| reorg at the boundary | the counters follow the branch | adoption by work alone (D133); detached transactions return to the pool (A11) and are includable on a branch with room | a block on the branch that is not the head is checked against its own branch's counter (`Chain::within_caps` reads the parent's), in both directions: a block that fits its branch's room but not the head's is valid, and one that fits the head's room but not its branch's is "16 caps". Tested by `counters_are_per_branch_across_a_reorg_at_the_exhaustion_boundary`, per counter and direction. TXBYTES_CAP: neither direction up to round 2 (W12, the head's counter, survived; second review R2C-1); both since round 3 (5965b8d; X9 and X10 are each killed by one direction). TX_CAP: neither direction up to round 2 either, since X1 (the count read from the head) survives round 2's tests (third review C3R-1; round 3 wrote "up to round 2, only the TX_CAP side was tested", which was wrong); round 3 pinned only the "16 caps" direction (A's next block after the reorg), and R1 (max(head, parent)) survived it; both since round 3b (e985ce1: B's one-transaction block imports while A, at TX_CAP, is the head) |

### 3b. LOG_CAP reached

This section covers the case where the remaining room is smaller than a transaction's charge.

| subject | spec | code |
|---|---|---|
| a transaction whose logs (successful frames) would pass the room | exceptional halt | `Halt::LogCap`: status 0, no logs, gas limit charged, nonce + 1, fee = gas limit × price, tip to Escrow, all other state reverted |
| ERC-20/721 transfers, approvals, mints (they emit events) | halt | halt: tokens are frozen |
| plain transfers, calls that emit nothing, registry operations | unaffected | unaffected (tested) |
| contract creation | the same rule: a CREATE halts if and only if its constructor's logs pass the room | tested: a CREATE without logs deploys; one whose constructor logs creates nothing, is charged its gas limit, and consumes the creator's nonce (so that address is never used) |
| logs of a reverted sub-call | not charged (LOG-ACCT-01) | not counted |
| partial room | the first transaction that would pass it halts; a later, smaller one may fit | as specified (LOG-CAP-AGGREGATE G1/G2) |
| system phases and programs | — | a system call cannot log: the v1 programs have no LOG opcode (model scan), and any system log is `ApplyError::Logs` (apply.rs:217). They neither count nor halt |
| rewards, settlement | unaffected | unaffected; halted transactions still pay tip × gas limit into the pot |
| base fee | an exceptional halt consumes all gas | a halted transaction adds its whole gas limit to the block's gasUsed (`user_phase.rs:253`), which drives the next base fee (`fees::next_base_fee`, chain.rs:217). After exhaustion, logging transactions with generous gas limits raise the London base fee as if they used their full limit, and their senders pay that limit. Code inspection; the log_cap test measures the charge, not the base-fee effect |
| builder | halts are valid blocks | includes them (tested through the node) |
| mempool | no rule | accepts. The pending state runs without the log (and dirty) part of the A03 meter. `PoolMeter::admit` (node.rs:410) checks only SLOT_CAP, ACCT_CAP and SYS_SLOT_MAX (`PoolMeter`, user_phase.rs:169). LOG_COUNT_MAX, LOG_BLOCK_MAX, LOG_CAP and the dirty limits are checked only in `BlockMeter::check` (user_phase.rs:193–207). So the pool believes that a transaction that will halt at LOG_CAP succeeds |
| RPC | — | eth_call and eth_estimateGas simulate without the meter (rpc.rs:439). They report success for a transaction that will halt and cost its whole gas limit (proposal N7) |
| reorg | per branch | `Entry.cum_log_bytes` comes from the parent (chain.rs:344) |

## 4. Identity: genesisHash, and why "unchanged" is not "compatible"

genesisHash = keccak256(GenesisSpec), where GenesisSpec = RLP([specVersion, chainId, CP, allocRoot,
M_0List, sysCodeHash]). ForkSchedule is not included (consensus.md:7, :11). Results from the model,
replayed by `lp4-execution/tests/caps_identity.rs`:

| item | result |
|---|---|
| Keccak-256 | K1–K3 (validation.md:60–62), "", "abc" and a 43-byte vector; the permutation matches hashlib SHA3-256 at every length 0–700 (up to six 136-byte rate blocks with padding) |
| GSV1 | rebuilt from the validation.md:65–72 segments and, separately, from its field values: 341 bytes, CP 179, M_0List 88, payload 338; SHA-256 = the E05 inputSha256 |
| H_GSV1 | `de518e30…eb2277`, the E05 three-library record (it is not in the spec text itself) |
| GSV1, TX_CAP + 1 / TXBYTES_CAP + 1 | `bf9aeabe…56aa39` / `90e4a907…c25c8c` (both change) |
| devnet (777900, g_ts 1,700,000,000, miners a, b, c; 375 bytes from the Rust builder) | `4db6aa5c…d8e638`, identical in the release build (LOG_CAP 2^30) and the caps-test build (2^24) |
| devnet, CP-only TX_CAP + 1 / TXBYTES_CAP + 1 | `979fda9c…e75eed` / `52cbbb44…c0fc58`. A rebuilt genesis also changes allocRoot, because ChainCaps (C0C004) stores all 52 CP values at slots 0..51 (genesis_builder.rs:62) |
| schemas | the 6 GenesisSpec fields and 52 CP names of consensus.md:7, :19–25 equal the Rust table; LOG_CAP and LOG_BLOCK_MAX are in neither. Every one of the 52 fields round-trips and enters the hash; a 53rd CP item is `gsCount` |
| other inputs | sysCodeHash recomputed from `tests/data/system-v1.json` (`4f8712dd…`), with no LOG opcode in any program; ForkSchedule is not hashed; LOG_CAP enters neither CP nor allocRoot |

**"genesisHash unchanged" does not mean "compatible".**

Two nodes with different LOG_CAP share genesisHash, netKey and the handshake. pocold's Hello carries
chainId, genesisHash, head, height and work, but no protocolVersion, although network.md:7's Status
includes it. They agree block for block until the branch's charged log bytes cross the smaller cap.

From that block on, a logging transaction gets status 1 on one node and a halt on the other. Its
receipts, gasUsed, logsBloom and stateRoot differ, so each node rejects the other's blocks at item 17:
a silent chain split.

Only a ForkSchedule entry makes the change detectable, through protocolVersion in the header (item 1b
netVersion) and in Status. The code supports v1 only (`ForkSchedule::new`), and the extension's light
client accepts version 1 only (rp.js:40).

A TX_CAP or TXBYTES_CAP change is a CP change, and so a different chain:
- a new genesisHash and netKey;
- `store::open` refuses the old data directory, whose journal is bound to genesisHash;
- networks.json must change.

## 5. Options

All five options keep the conformance of §6.

| | consensus | storage at the caps | resident memory (pocold) | compatibility | liveness and griefing | complexity |
|---|---|---|---|---|---|---|
| **O1** keep the caps, freeze as specified (status quo + §6) | none | inventory 19.65 GiB logical (storage.md:73); pocold journal ≈ TXBYTES_CAP + 4 B per tx + 3.3 KB per block (network 8.4 GiB; receipts and logs are not stored, replay recomputes them) | history term TXBYTES_CAP + 380·TX_CAP + LOG_CAP·33/32 (resources.rs:103–107): devnet 1,079 MiB; network 12.6 GiB (already refused by the 6 GiB A13 gate) | none | the chain continues, but user activity is frozen permanently; network: organically at 20–34% average use, by griefing for 6.6·10^10 (LOG_CAP) to 1.5·10^12 wei (TX_CAP) at the idle base fee (§2) | done |
| **O2** unreachable by construction (A1a extended to all three) | gate R27: H_END·LOG_BLOCK_MAX ≤ LOG_CAP; R28: H_END·⌊GAS_LIMIT/21000⌋ ≤ TX_CAP; R29: H_END·TXB_BLOCK ≤ TXBYTES_CAP, where TXB_BLOCK is the largest raw-byte sum of a body ≤ BODY_MAX (196,592). Only exception: the simnet caps-test build. TX caps are CP, so a new genesisHash; LOG_CAP is a constant (same hash; a hard fork via ForkSchedule on any existing chain, but none holds > 1 GiB of logs) | network at LOG_CAP 2^32, TX_CAP 5·10^7, TXBYTES_CAP 24 GiB: inventory ≈ 43.2 GiB logical (txs 8.0 → 24.0, receipts 0.50 → 2.5, indexes 0.63 → 3.15, logs 1.0 → 4.0; 42.8 at the exact R27–R29 minima), ≈ 65 GiB on disk at the spec's 1.5× ratio | history term TXBYTES_CAP + 380·TX_CAP + L·33/32 (resources.rs:103–107), L the log bytes assumed. Network at the O2 sizes (L = 2^32): 45.8 GiB. Devnet at the R28/R29 minima (TX_CAP 7.6·10^6, TXBYTES_CAP 3,931,840,000): 6.98 GiB (7,495,680,000 B) with L = min(LOG_CAP, H_END·LOG_BLOCK_MAX) = 655,360,000, A1a's tightening, which resources.rs:107 does not have; 7.38 GiB with L = 2^30 as coded; 10.48 GiB with N1's 2^32. These replace the 1,131,673,472 B of the same three items at the devnet caps. At the integrated head fa97594 (model 5,577,343,580 B, 68,343,995 B unused), the whole A13 model becomes 11.12 GiB tightened, 11.52 GiB with L = 2^30 and 14.62 GiB with N1's 2^32 as coded. Those need budgets of 12.71, 13.17 and 16.70 GiB (ceil(8·total/7)), and the 6 GiB gate refuses boot in every case. §5b gives the exact figures, also on the c3a78e7 base that rounds 1 and 2 called "today". **Needs canonical history off-heap first** (bodies and receipts read from the journal beyond K_hist, as D-REORG-REDB did for side blocks: a local storage decision). Also, eth_getTransactionByHash scans the canonical chain linearly (rpc.rs:455), and `Node::open` holds the whole journal (A13 condition open-replay-transient) | GSV1 unchanged (a frozen vector; gate rules are not decode rules); devnet genesis files regenerated; networks.json gets the new network genesisHash; no RPC change | no cumulative freeze; only the per-block limits remain | low: three values, three gate predicates, the inventory re-derived |
| **O3** linear allowance: cum_X(block h) ≤ ⌊h·CAP_X/H_END⌋ + BURST_X for txs, tx bytes and log bytes (a log halt, a tx block invalid) | a new validity/halt rule, a protocol change; BURST is new CP or derived | lifetime ≤ CAP + BURST: inventory unchanged | unchanged | new vectors at the h boundaries; pocol_getCaps should expose the allowance | never permanent. Throughput is capped at the lifetime average plus the burst: network 77 tx, 66 KB, 8.3 KB of logs per block. A griefer can take each block's allowance; the base fee does not price it below the target, so honest users must outbid on tips | moderate |
| **O4** sliding window (A2 generalized) | as O3 | ≤ ⌈H_END/W⌉·cap | unchanged | as O3 | a stall of up to W minus the fill time per saturation | moderate |
| **O5** remove the caps from validity (A4 generalized) | for TX caps a CP schema change (version 2, 50 fields: GSV1, refdec.js and every vector change) | as O2 | as O2 | heavy | as O2 | high for the schema |

**Integrated recommendation** (the owner decides):
1. **Keep the §6 conformance in every case.** It removes the stall and the starvation, and makes the
   spec's "freeze the component, not the chain" (validation.md:2129) what the code does.
2. **Network profile: O2 for all three caps**, under one principle: "no cumulative cap may be reachable
   before H_END". This means R27–R29 and LOG_CAP = 2^32, TX_CAP = 5·10^7, TXBYTES_CAP = 24·2^30.
   - The LOG_CAP part is exactly A1a.
   - It adds no consensus mechanism, keeps the halts and invalidity as defence in depth, and keeps T18b
     reachable through the caps-test exception.
   - The price is about +23 GiB of logical storage. The network genesisHash changes, but no network chain
     exists: devnet.rs refuses 777001.
3. **If the 19.65 GiB inventory is a hard requirement: O3 for all three.** It is one mechanism, and it is
   preferred over O4 because it has no window-edge stall. It turns the cliff into a griefable rate
   limit.
4. **Devnet: keep the approved D-A13-PROFILE caps.** With §6, exhaustion at 53 blocks (TX_CAP) or 86
   blocks (TXBYTES_CAP) of full load is the declared freeze, not a stall. Resize only after a local
   canonical-history offload, because O2 sizes break the A13 bound.

## 5a. Normative proposals (PROPOSED, not approved)

| id | proposal | text touched |
|---|---|---|
| N1 | R27 + LOG_CAP = 2^32 (= A1a; already proposed) | execution.md:82, governance.md (gate), validation.md:2065, storage.md:73/77 |
| N2 | R28: H_END·⌊GAS_LIMIT/21000⌋ ≤ TX_CAP; network TX_CAP = 5·10^7 | governance.md:13 (CP), gate, storage.md:76–77 |
| N3 | R29: H_END·TXB_BLOCK(BODY_MAX, TX_MAX) ≤ TXBYTES_CAP; network TXBYTES_CAP = 24·2^30 | governance.md:13 (CP), gate, storage.md:76 |
| N4 | the alternative to N1–N3: the O3 linear allowance | execution.md:82–83, consensus.md:170 |
| N5 | pool: −32016 {cap} for a transaction that does not fit the head branch's remaining TX_CAP/TXBYTES_CAP room (not only after full exhaustion: this also ends the squatting of §3a), and such transactions leave the pool when a new head leaves them no room. Only the {cap} code and the eviction are new: re-checking after a reorg is already approved (network.md:153) | network.md:149–153 |
| N6 | pocol_getCaps → {txCount, txBytes, logBytes, txCap, txBytesCap, logCap} of the head branch, plus an alert for governance.md:197's "freeze and alert" | network.md:333, governance.md:197 |
| N7 | eth_call and eth_estimateGas report an A03 halt (or apply the meter) instead of success | execution.md:101–102 |
| N8 | state the scope explicitly: "the counts of the branch up to the parent plus this block" (item 16; LOG-CAP-AGGREGATE for logs) | consensus.md:170 |

## 5b. O2 resident memory, by commit

The A13 model (`pocold::resources::derive`, printed by `tests/consensus.rs` `a13_bound_breakdown`)
changes with the code, so each figure below names its commit.

- In rounds 1 and 2, "today" meant the D-REORG-REDB final model, 5,627,687,516 B. It was measured at
  9ce5a7c (evidence/a13-breakdown-redb.json, A13-DERIVATION.md:187–189), and the second review
  confirmed it at c3a78e7, the caps round-2 head.
- RpcGuard round 2 (RPCGUARD.md:158, merged at ba7d294) reduced the model to 5,577,343,580 B. That is
  the model at 273e785 (second review) and at fa97594, the integrated head this round starts from
  (evidence/caps-r3/a13-fa97594.log).

O2 replaces the three history items at the devnet caps with the history term at the R28/R29 minima:
- at the devnet caps, the items for TXBYTES_CAP (16,777,216 B), TX_CAP (7,600,000 B) and LOG_CAP
  (1,107,296,256 B) are 1,131,673,472 B together;
- the O2 term is TXBYTES_CAP + 380·TX_CAP + L·33/32 = 3,931,840,000 + 2,888,000,000 + L·33/32.

The gate needs ceil(8·total/7) ≤ 6 GiB = 6,442,450,944 B.

| base | model at that commit | unused at that commit | L (log bytes assumed) | history term | O2 model | O2 budget needed |
|---|---:|---:|---|---:|---:|---:|
| fa97594 | 5,577,343,580 B | 68,343,995 B (65.2 MiB) | 655,360,000 = H_END·LOG_BLOCK_MAX (A1a's tightening; not in resources.rs:107) | 7,495,680,000 B (6.98 GiB) | 11,941,350,108 B (11.12 GiB) | 13,647,257,267 B (12.71 GiB) |
| | | | 2^30, as coded at fa97594 | 7,927,136,256 B (7.38 GiB) | 12,372,806,364 B (11.52 GiB) | 14,140,350,131 B (13.17 GiB) |
| | | | 2^32, as coded with N1 | 11,249,025,024 B (10.48 GiB) | 15,694,695,132 B (14.62 GiB) | 17,936,794,437 B (16.70 GiB) |
| c3a78e7 (9ce5a7c's model) | 5,627,687,516 B | 10,808,068 B (10.3 MiB) | 655,360,000, tightened | 7,495,680,000 B | 11,991,694,044 B (11.17 GiB) | 13,704,793,194 B (12.76 GiB) |
| | | | 2^30 | 7,927,136,256 B | 12,423,150,300 B (11.57 GiB) | 14,197,886,058 B (13.22 GiB) |
| | | | 2^32 | 11,249,025,024 B | 15,745,039,068 B (14.66 GiB) | 17,994,330,364 B (16.76 GiB) |

Reading:
- The 6 GiB gate refuses boot in every row.
- resources.rs:107 counts LOG_CAP itself. So N1 + O2 as coded uses L = 2^32; the tightened rows need
  A1a's tightening to be implemented there as well. Even then, the model needs about twice the budget.
- The conclusion of §5 is unchanged: O2 on the devnet first needs canonical history off-heap.
- The arithmetic, integers only, is recorded in evidence/caps-r3/o2-memory-fa97594.json.

## 6. What is implemented (conformance), and the evidence

| change | spec | commit | evidence (development/audit-2026-10-22/evidence/caps/) |
|---|---|---|---|
| `Chain::within_caps` and `caps_room`: the one definition of item 16; validate calls it (same arithmetic, checked, so an overflow cannot wrap) | consensus.md:170, D49 | bdb14bb | `unfixed-bdb14bb.log`; the full suites in `suites-release-a0dd771.log` |
| `Node::template_txs` skips past the room (and the sender's later transactions) | execution.md:83 | 9094f98 | fails without it: `unfixed-bdb14bb.log` (6/6), `unfixed-final-tests-reverted-builder.log` (the final tests on the unfixed builder, 6/6), `unfixed-ddf023a-live-trace.log` (the stall traced); passes: `caps-tests-a0dd771.log` |
| `try_lock_signed` treats an over-cap template as absent | consensus.md:101, §4(a) | 9094f98 | `a_received_template_past_the_caps_is_treated_as_absent` in the same logs |
| cargo feature caps-test: LOG_CAP = 2^24; admission refuses anything but a simnet profile on a simnet node | validation.md:2065 | a797c83, a0dd771 | `caps-test-build-a0dd771.log`; both suites in that build: `suites-caps-test-a0dd771.log` |
| model, vectors, identity test | — | bc53984 | `caps-model-a0dd771.log`, `caps-tests-a0dd771.log` |
| round 2: the byte-exact TXBYTES_CAP boundary test (review CV-1); comment citations | consensus.md:170, D49, execution.md:83 | c635594 | `r2-caps-tests-0b9b4d0.log` (7 pass), `r2-mutations-c635594.log` (16 of 16 killed, §7a) |
| round 2: model check label and `emptyBlocksFromBaseFee0ToIdle` (review CV-7, CV-12) | — | 0b9b4d0 | `r2-caps-model-0b9b4d0.log` (25 checks; byte-identical to the committed vectors), `r2-caps-tests-0b9b4d0.log` (caps_identity 3 pass on the new vectors) |
| round 3: tests pin which block and which branch item 16 reads the byte counter from, and the builder's exact fit across blocks (second review R2C-1, R2C-2); test-only | consensus.md:170, D49, execution.md:83 | 5965b8d | in `evidence/caps-r3/`: `caps-tests-5965b8d.log` (7 pass), `mutations-5965b8d.log` (§7c) |
| round 3: the A13 model at the integrated head, and O2 recomputed on it (R2C-3, R2C-5) | — | measured at fa97594 | in `evidence/caps-r3/`: `a13-fa97594.log`, `o2-memory-fa97594.json` (§5b) |
| round 3b: tests pin TX_CAP's branch scope in both directions, and both counters' boundary at every position and block shape (third review C3R-1 to C3R-3); test-only | consensus.md:170, D49, execution.md:83 | e985ce1 | in `evidence/caps-r3b/`: `caps-tests-e985ce1.log` (9 pass), `mutations-e985ce1.log` (§7e) |

Round 3 ran at 5965b8d, in release, on Linux. The later round-3 commits change only `development/`.
The logs are in `evidence/caps-r3/`:
- `tests/caps.rs`: 7 pass;
- lp4-execution `caps_identity`: 3 pass;
- in the caps-test build, `tests/log_cap.rs` 2 pass and `tests/caps.rs` 7 pass;
- the whole pocold suite, once: 153 pass, 0 fail, 8 ignored (the 2 spec-depth tests and the 6 journal
  measurements, not run), no compiler warnings;
- `caps_model.py`: 25 checks, byte-identical to the committed vectors.

The full lp4-execution suite was not rerun, because round 3 changes no lp4 code.

Round 3b ran at e985ce1, in release, on Linux. The later round-3b commit changes only `development/`.
The logs are in `evidence/caps-r3b/`:
- `tests/caps.rs`: 9 pass (7 before, plus the two boundary matrices), no compiler warnings;
- lp4-execution `caps_identity`: 3 pass;
- in the caps-test build, `tests/log_cap.rs` 2 pass (exhaustedAt 513) and `tests/caps.rs` 9 pass;
- the whole pocold suite, once: 155 pass, 0 fail, 8 ignored (the 2 spec-depth tests and the 6 journal
  measurements, not run), no compiler warnings;
- the mutation check (§7e) and the R7 probe.

`caps_model.py` was not rerun, because round 3b changes neither the model nor its vectors.

The full lp4-execution suite was not rerun, because round 3b changes no lp4 code.

Round 2, at 0b9b4d0 (its `prototypes/` tree equals c635594's), release: lp4-execution 165 pass, 0 fail;
pocold 130 pass, 0 fail, 2 ignored (the spec-depth tests, not run); no compiler warnings
(`r2-suites-release-0b9b4d0.log`). In the caps-test build only the tests the round touches were run:
`tests/caps.rs` 7 pass and `caps_identity` 3 pass (`r2-caps-test-build-0b9b4d0.log`). The caps-test
full suites below are stage A's.

Suites at a0dd771, which carries stage A's final code (later stage A commits add documents only):

| build | lp4-execution | pocold |
|---|---|---|
| release | 165 pass, 0 fail | 129 pass, 0 fail, 2 ignored (the spec-depth tests, not run) |
| caps-test | 165 pass | 131 pass, 2 ignored |

No compiler warnings. These runs do not establish full acceptance. The network-profile figures come from
arithmetic and the model; no network-profile chain was run.

Not implemented, and why:
- **HIST_TRIE_BUDGET = 64 MiB** of the same caps-test line, so the caps-test item is implemented only in
  part (LOG_CAP). The spec's 64 MiB serves T18b L12/L13 (validation.md:2082–2083): the historical trie
  passes its budget, K_eff falls and −32017 answers beyond it. pocold has no historical trie and no
  K_eff. In lp4 the constant (`admission.rs:56`, not feature-gated) only bounds R21's retained-state
  figure. At 64 MiB, every caps-test admission would take the R21 simnet retentionStress exception
  (single-host local parameters set it) or be refused R21 without it, and nothing would be exercised.
- **The pool rule:** the spec states none (N5).
- **protocolVersion in pocold's Hello** (network.md:7): an open conformance gap of the wire format,
  outside this track.
- **Punishing the peer** that sent a violating template (consensus.md:101): pocold has no peer scoring
  for templates.

## 7. Round-2 corrections (after the independent review)

The review of `wf/caps` at f64ed4d found no blocker and one major issue. Every finding and its
resolution is listed below. Code changes are conformance tests and comments only; nothing normative
is implemented.

| finding | resolution |
|---|---|
| CV-1 (major): no test pinned the TXBYTES_CAP byte boundary. Every room left at least 56 B of slack, so six mutants survived: the builder off by one byte either way or counting RLP headers, and item 16 off by one byte either way or counting headers | fixed in c635594: `txbytes_cap_boundary_is_byte_exact_in_the_builder_and_in_item_16` sets TXBYTES_CAP to exactly a0+b0 (filled, mined, valid; room left 0) and to a0+b0−1 (b0 skipped; a block carrying it is "16 caps", both hand-built on genesis and through the node). The six survivors are killed (§7a) |
| CV-2: "a violating template counts as absent" was cited as consensus.md:136 | it is consensus.md:101 (§4(a)); fixed in this file, the proposals README, `node.rs:556` and `tests/caps.rs`. Commit message 9094f98 keeps the old line number (history is not rewritten) |
| CV-3: O2 headroom 9,457,060 B | 10,808,068 B (A13-DERIVATION.md:188–189) |
| CV-4: O2's "≈ 7.0 GiB" was the history term only, with the A1a tightening, and was quoted as resident memory | §5 now gives the formula, the log bytes assumed (6.98 / 7.38 / 10.48 GiB) and the whole A13 model (11.17 GiB, needing 12.76 GiB) |
| CV-5: the measured 74–78 s stall is the tests/caps.rs profile (D_att 73), not the default devnet | §3a: 74–78 s measured on that test devnet; D_att+Γ in general, 30 s on the default devnet and about 330 s on the network profile (derived, not run) |
| CV-6: the starvation mode was given without its reach | §3a: only when TX_CAP is below one block's count (≤ 380); not on any real profile |
| CV-7: "at most 12.8 coins" (§2; "≤ 12.8 coins" in §5 O1) | 12.8 coins is one strategy. 145 free empty blocks reach the 7 wei floor, after which TX_CAP costs about 1.5·10^12 wei. The model now records `emptyBlocksFromBaseFee0ToIdle` = 145 |
| CV-8: free pool squatting near the TXBYTES boundary was missed | §3a row added (code inspection); N5 now covers transactions that do not fit the remaining room |
| CV-9: halted transactions charge their whole gas limit to gasUsed | §3b base-fee row added (code inspection) |
| CV-10: N5's "re-checked after a reorg" is already approved (network.md:153) | N5 is limited to the −32016 {cap} code and the eviction |
| CV-11: the caps-test item is only partly implemented | §6: LOG_CAP implemented; HIST_TRIE_BUDGET = 64 MiB not, with the reason |
| CV-12: "five rate blocks"; "lower bound" for the caps-test history term | up to six 136-byte rate blocks (701 B with padding; the model's check label is corrected too); with LOG_CAP = 2^24 the history term is a smaller, still valid upper bound for that build |

### 7a. Mutation check after the fix

The review's mutants of the builder, item 16 and template locking, rerun against the round-2
`tests/caps.rs` at c635594 (`r2-mutations-c635594.log`; the runner and the edits stay in the scratch
directory). All 16 are killed, the six former survivors by the new boundary test:

| mutant | killed by |
|---|---|
| M2a builder `>=` room; M3a builder counts the RLP header | the exact fit (template carries only a0) |
| M2b builder 1 byte over the room | one byte short (template carries b0) |
| M2f item 16 `<` TXBYTES_CAP; M3b item 16 counts 2 extra bytes per transaction | the exact fit (the hand-built block on genesis is refused) |
| M2h item 16 accepts TXBYTES_CAP + 1 | one byte short (the hand-built [a0, b0] block is accepted) |
| M1, M2c–M2e, M2g, M3c, M6–M9 (killed before as well) | the existing tests, plus the boundary test where it applies |

The review's other notes need no code change:
- **M5tb** removes every status assertion from `log_cap.rs` together with the halt status. Its
  survival only shows that the assertions are what detect M5, and M5 itself is killed.
- **M4** (caps-test LOG_CAP back to 2^30) is killed by the constant-equality tests. The exhaustion test
  is written in terms of `caps::LOG_CAP` on purpose, so that it tests the halt at whatever the build's
  constant is; the equality tests pin the constant to validation.md:2065.
- **M1's first variant** did not compile and counted as incomplete. The corrected variant is killed,
  here and in the review.

## 7b. Round-3 corrections (after the second review)

The second review of `wf/caps` at 273e785 found no blocker, one major issue and five minor ones. Every
finding and its resolution is listed below. The only code change is to tests (`tests/caps.rs`,
5965b8d); nothing normative is implemented, and development/m1/reference is not edited.

| finding | resolution |
|---|---|
| R2C-1 (major): no test pinned which entry's counter item 16 reads for TXBYTES. W11 (the grandparent's counter) and W12 (the head's) survived the whole pocold suite. The reorg test pinned the branch scope for TX_CAP only, and every "over" block followed two empty blocks | fixed in 5965b8d, in three parts. (1) The boundary test checks the over block on the block that used the room, with no empty block between, so the parent's and the grandparent's counters differ (W11). (2) The reorg test has a TXBYTES fork case, with TXBYTES_CAP = big + b0 − 1 (828 B). Branch A carries big (716 B) and is left with 112 B of room. Branch B (fallback, b0 113 B, fallback) forks at genesis and is heavier. While A is the head, B's block carrying b0 fits B's room but not A's, and it is valid. Once B is the head, A's next block (113 B) fits B's room (715 B) but not A's, and it is "16 caps". This kills W12, and each direction is pinned on its own (X9, X10 in §7c). (3) The TX_CAP part also imports A's next block after the reorg, so the count is read from the parent, not from the head (X1). §3a's "tested" now covers both counters. *Corrected in round 3b (third review C3R-1, §7d): part (3) pins only the "16 caps" direction for TX_CAP, and R1 (max(head, parent)) survived it; nor was the TX_CAP side tested up to round 2, as §3a then said (X1 survives round 2's tests). Both directions are pinned since e985ce1* |
| R2C-2: the builder's exact fit was pinned in one position only, the second transaction of a block on genesis. B10, B11 and B12 survived tests/caps.rs | fixed in 5965b8d: a cross-block exact fit with TXBYTES_CAP = a0 + b0. a0 is alone in block 1. Then `template_txs` returns [b0], with b1 skipped. b0 is block 2's first transaction and fills the room on a non-zero parent counter. Block 2 reaches cum_tx_bytes = TXBYTES_CAP and is a valid head (the positive item-16 exact fit across blocks), and b1 on block 2 is "16 caps" |
| R2C-3: the "today" A13 figures (5,627,687,516 B, 10,808,068 B unused) were stale at the integrated head | §5 O2 cell and the new §5b name the commit of each figure. 5,627,687,516 B is 9ce5a7c's model, confirmed at c3a78e7. At fa97594 the model is 5,577,343,580 B (measured, `evidence/caps-r3/a13-fa97594.log`, = RPCGUARD.md:158), with 68,343,995 B unused. O2 is recomputed at fa97594: 11,941,350,108 B tightened, needing 13,647,257,267 B. A13-DERIVATION.md's "9ce5a7c and later" is not this track's file; replacement text is handed to the orchestrator |
| R2C-4: "waiting 145 empty blocks reaches that fee from any starting base fee" | §2: from baseFee0 = 10^9. A higher start needs about 5.2 more empty blocks per doubling: 150, 162 and 196 blocks from 2·10^9, 10^10 and 10^12 (recomputed by exact integer simulation) |
| R2C-5: the O2 whole model was given only with A1a's tightened log term | §5b gives the as-coded totals (L = 2^30, and N1's 2^32) next to the tightened one, on both bases. It also states that N1 + O2 as coded uses 2^32 unless resources.rs:107 implements the tightening, and that the gate refuses boot in every case |
| R2C-6: "the pending state runs without the A03 meter" was imprecise | §3b mempool row: `PoolMeter::admit` (node.rs:410) checks SLOT_CAP, ACCT_CAP and SYS_SLOT_MAX. LOG_COUNT_MAX, LOG_BLOCK_MAX, LOG_CAP and the dirty limits are only in `BlockMeter::check` (user_phase.rs:193–207). The LOG_CAP conclusion is unchanged |

The review's line-reference note is applied too: the intro now lists the node.rs and rpc.rs lines that
moved by fa97594.

### 7c. Mutation check after round 3

Three sets were rerun against the round-3 `tests/caps.rs` at 5965b8d, one mutant at a time, in release:
- the second review's 24 mutants (B1–B12, W1–W12), with its edits unchanged;
- the first review's 21 (M1–M11), with its edits unchanged;
- 10 new ones (X1–X10).

Results are in `evidence/caps-r3/mutations-5965b8d.log` and `.json`. The runner and the edits stay in
the scratch directory. **55 of 55 are killed.** None was incomplete and none timed out. The log_cap
and caps_identity mutants (M4, M4b, M5, M10, M11) ran the tests they target, as in the first review.

| set | mutants | killed by |
|---|---|---|
| second review, builder | B1–B12 | B10 (exact fit refused on a non-zero parent counter), B11 (list header counted for a block's first transaction) and B12 (off by one for a block's first transaction): the cross-block exact fit, where `template_txs` must return [b0] on block 1's counter. B1–B9 as in round 2 (the boundary test, and for B2 the fork case too) |
| second review, item 16 and the stored counter | W1–W12 | W11 (the grandparent's counter): the over block checked on the block that used the room. W12 (the head's counter): the fork case, B's block while A is the head. W1–W10 as in round 2 |
| first review | M1–M11 (with M2a–M2h, M3a–M3c, M4b) | the existing tests, as in round 2 |
| new: branch and block scope of item 16 | X1 TX count from the head; X2 TX count from the grandparent; X3 bytes from the canonical block at the parent's height; X4 a non-canonical parent's byte counter read as 0; X9 min(head, parent), the lenient direction only; X10 max(head, parent), the strict direction only | X1: A's next block after the reorg (TX_CAP part). X2: the template-locking test. X3 and X10: B's block while A is the head. X4 and X9: A's block while B is the head. So each direction of the fork case is load-bearing on its own |
| new: builder room and stored counter | X5 builder room from the grandparent's counter; X6 stored byte counter not cumulative; X7 stored byte counter from the head; X8 a block's first transaction not checked against the byte room | X5: the cross-block fit, plus two other tests. X6: six tests. X7: the stored-counter check (`assert_within_caps`) in the reorg test. X8: four tests |

The review's whole-suite survivors (W11, W12, B11) are now killed by `tests/caps.rs`, which is part of
the whole suite.

The third review found survivors of these tests that this set did not include (R1, R6, R9, T1–T4, T8;
§7d). "Each direction of the fork case is load-bearing on its own" holds for the byte counter only; for
TX_CAP, round 3 pinned one direction.

## 7d. Round-3b corrections (after the third review)

The third review of `wf/caps` at 938cb94 found no blocker, one major issue and three minor ones. Every
finding and its resolution is listed below. The only code change is to tests (`tests/caps.rs`,
e985ce1); nothing normative is implemented, and development/m1/reference is not edited.

| finding | resolution |
|---|---|
| C3R-1 (major): for TX_CAP only one direction of item 16's branch scope was pinned. R1 (the count read from max(head, parent)) survived tests/caps.rs and the whole pocold suite. §3a's "in both directions" was wrong for TX_CAP, and so was "up to round 2, only the TX_CAP side was tested": X1 (the count read from the head) survives round 2's tests | fixed in e985ce1. The TX_CAP part of the reorg test now builds B as (fallback, bob's transfer, fallback); B forks at genesis and is heavier than A. While A is the head at cum_txs = TX_CAP = 3, B's block carrying one transaction fits B's room (3) but not A's (0), and it imports (this kills R1). After the reorg (B at 1), A's next block fits B's room (2) but not A's, and it is "16 caps" (X1, R2, R3). The builder then fills B's room of 2 (templates [2, 0, 0]). §3a now states, per counter and direction, what was tested when; §7b carries a correction note |
| C3R-2: TX_CAP had no fill across blocks from a non-zero count and no "over" block carrying more than one transaction. T4, T8 and R6 survived the whole suite | fixed in e985ce1 by `tx_cap_boundary_is_exact_at_every_position_and_block_shape` (below) |
| C3R-3: the byte boundary was not pinned for a third or later transaction, for a non-first transaction on a non-zero parent, or for a multi-transaction "over" block on a non-zero parent. T1, T2, T3 and R9 survived the whole suite | fixed in e985ce1 by `txbytes_cap_boundary_is_exact_at_every_position_and_block_shape` (below) |
| C3R-4: round 3's doc_updates missed the second stale 5.241 GiB figure (FINDING-STATUS.json:647, make_finding_status.py:71) | these are not this track's files. The corrected text for both lines (5.194 GiB at fa97594; 5.241 GiB at 9ce5a7c) is handed to the orchestrator |

Instead of one case per survivor, the two new tests share one helper (`boundary_matrix`) that runs a
counter over every combination of:
- the parent's counter: zero (genesis), or two transactions (a0 and a1 in block 1, 226 B);
- k = 1, 2 or 3 of bob's transfers (113 B each) in the block;
- the cap at the exact fit (parent + k), or one short of it (one transaction, or one byte).

Each case checks three things:
- item 16 on a hand-built block of all k: valid at the exact fit; "16 caps" when one over, both on the
  harness and through the node;
- the builder, with bob's four transfers pooled: `template_txs` returns exactly his first k − short;
- the builder's block imports as a valid head (at the cap when the fit is exact), and the stored counters
  equal the bodies.

Two things are skipped:
- the case TX_CAP 0 (zero parent, k = 1, one short), which leaves nothing to fit;
- in the TX_CAP cases one over on a zero-count parent (k = 2, 3), the hand-built block; the builder is
  still checked there. That block, of more than TX_CAP transactions, cannot be built: build_signed
  applies the body, and apply_block refuses more than TX_CAP transactions in one block (apply.rs:250).
  It is the case R7 changes (§7e).

So 11 TX_CAP cases and 12 TXBYTES_CAP cases run; their `CAPS` lines list them.

`live()` now uses its own data directory (`live-<tag>`). Its tags `txcap` and `txbytes` were also used by
two in-process tests of the same binary. The two nodes of each pair have the same PID, so they used the
same data directory while running in parallel. Depending on timing, the second open could fail as
`Locked` (store.rs:117), or one test's `node()` could remove the directory under the other's running
node. No failure was observed.

### 7e. Mutation check after round 3b

Three sets were run against the round-3b `tests/caps.rs` at e985ce1, one mutant at a time, in release:
- round 3's 55 (§7c), with their edits unchanged;
- the third review's 20, with its edits unchanged. Eight of them (B10–B12, W11, W12, X1, X9, X10)
  have the same edits as round 3's and ran once, so 12 are new: R1–R3, R6–R9, T1–T4 and T8;
- 17 new ones (Y1–Y17), aimed at the positions and shapes the matrix covers.

The runner loads the earlier runners' edit lists from their files, so the edits are not retyped. Results
are in `evidence/caps-r3b/mutations-e985ce1.log` and `.json`; the runner and the edits stay in the
scratch directory. **83 of the 84 are killed. R7 survives, and it is equivalent (below).** No run hit
the harness timeout. Y5's first edit had one parenthesis too many and did not compile. It is recorded as
incomplete in run 1, and the corrected edit is killed in run 2. All 55 of round 3's kills still hold,
including the eight the third review reran (B10–B12, W11, W12, X1, X9, X10).

| mutant | killed by (tests/caps.rs line) |
|---|---|
| R1 TX count from max(head, parent) | the reorg test: B's one-transaction block while A is the head (:184) |
| R2 TX count from min(head, parent) | the reorg test: A's next block after the reorg (:187) |
| R3 TX count from the canonical block at p's height | the reorg test (:184) |
| R6 TX count +1 slack for multi-transaction blocks | the TX_CAP matrix: the block one over is "16 caps" (:156) |
| R8 bytes +1 slack for a single transaction on a zero-byte parent | the TXBYTES_CAP matrix (:156): one 113 B transfer on genesis with TXBYTES_CAP 112 |
| R9 bytes +1 slack for a multi-transaction block on a non-zero parent | the TXBYTES_CAP matrix (:156) |
| T1, T2, T3 (builder byte room after the first transaction; one byte over from the third; exact fit refused for a non-first transaction on a non-zero parent) | the TXBYTES_CAP matrix: the template (:160) |
| T4, T8 (builder TX room one short on a non-zero count; a block's first transaction not using TX room there) | the TX_CAP matrix (:160) and the reorg test (:190) |
| Y1–Y4 builder byte position (first transaction on a zero parent, exact and one over; exact fit from the third; one over for a non-first transaction on a non-zero parent) | the TXBYTES_CAP matrix (:160) |
| Y5–Y8 builder TX room (one long on a non-zero count; not decremented from the third; checked for the first two only; one short on a zero count) | the TX_CAP matrix (:160), and other tests |
| Y9–Y12 item 16 bytes (exact fit refused for 3+ transactions; +1 slack for 3+; only the first two counted; +1 slack for one transaction on a non-zero parent) | the TXBYTES_CAP matrix (:155, :156), Y12 also the boundary and reorg tests |
| Y13–Y17 item 16 count (+1 slack for one transaction on a non-zero parent; exact fit refused for multi-transaction blocks on a non-zero or a zero count; +1 slack for 3+; a block counted as at most 2) | the TX_CAP matrix (:155, :156), Y13–Y15 also other tests |

**R8** is not equivalent on the profiles these tests use. The third review found it equivalent on every
real profile, where TXBYTES_CAP (at least 16 MiB) exceeds TX_MAX (131,072 B), so a single transaction,
already bounded by TX_MAX (chain.rs:314), can never pass TXBYTES_CAP by one byte. The test devnets set
TXBYTES_CAP below TX_MAX, which admission accepts. There R8 accepts a block one byte over, and the matrix
kills it.

**R7** (TX count +1 slack on a zero-count parent) survives, and it is equivalent. On a zero-count parent,
item 16's count is txs.len() ≤ TX_CAP; R7 makes it txs.len() ≤ TX_CAP + 1. Each use of `within_caps`
either is followed by applying the body, or has no effect in release:
- `validate` (chain.rs:335): the next step applies the body (chain.rs:337), and `apply_block` refuses more
  than TX_CAP transactions in one block (apply.rs:250). The block is still `Reject::Block`, with the reason
  `apply Budget("TX_MAX")` instead of `16 caps`.
- `try_lock_signed` (node.rs:559): the next steps call `rebuild_signed` (node.rs:561, chain.rs:253),
  which applies the body as well, so the template is still not locked.
- `template_txs`: only a `debug_assert` (node.rs:465), compiled out in release.

So only the reason string changes, for a block that the builder never produces and that build_signed
cannot produce (it applies the body). A scratch probe forges such a block and confirms this
(`evidence/caps-r3b/r7-probe-e985ce1.log`; the probe program is gitignored and not committed). It uses
TX_CAP 2 and a zero-count parent. A sealed block carrying 3 transfers, its header built for 2 and then
committing to all 3, with the template re-signed by the drawn proposer, gives:
- without R7, `Block("16 caps")`;
- with R7, `Block("apply Budget(\"TX_MAX\")")`.

As a received template it is not locked in either case, while the valid 2-transfer control template
locks in both.
