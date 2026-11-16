# Sustainability and value

**Facts from the thesis**
- Partitioning alone does not reduce energy [PDF:128, PDF:131].
- Reductions come from lower-power states with a service loss [PDF:134, PDF:136].
- Results are limited to simulation [PDF:125].

**Unproven inference**
- Reducing active work may reduce attack cost; read T13 together with T10 [PDF:79].
- ASERT steers the target back toward T_blk after the rate decreases, proportionally lowering rewrite cost; no bound is claimed.
- The RP ceiling (D84) lower-bounds forgery cost by work(ceilTarget) per header; it does not make forgery expensive.
  - At target_g=2^240 this is 4095 per header, or 53235 expected hashes for 13 headers.
  - Rough estimate, not a guarantee: approximately 49 seconds at H_cal; the bound collapses at saturation.

**Costs**

Coordination:
- Template execution, waiting until X^W (11 s on the fresh network at both declared local timings, (1, 1, 1) and (2, 1, 1); threat.md, Timing), and precomputed fallback.
- Shares ≤ 2.3 KiB; evidence ≤ 8 KiB.
- Header-only broadcast saves up to 192 KiB per block per peer.

Verification:
- Fixed-width ASERT ≤ 512 bits.
- RP ≤ 14 headers (approximately 43 KiB) and ≤ 13 ASERT calculations.
- genesisPre decoding: linear in its size, which MAX_SPEC_BYTES bounds (genesis_spec.rs); a fresh-network GenesisSpec holds 59 CP fields and four founders.
- BridgeAuth: schema checking per message ≤ 64 KiB, linear in message size, without networking on rejection. Over-rate messages are rejected without parsing; bucket cost is two integer operations.
- Site-local storage ≤ 1 MiB per (netKey, address) in chrome.storage.local on the user's machine. Quota accounting uses integer addition/subtraction per message; an atomic write rewrites the entire site dictionary (≤ 1 MiB) per accepted message. Batching every 250ms reduces writes.

RPC service (D88–D94):
- Memory ≤ 192 MiB (computed 178 MiB), with 32 KiB kernel buffers per connection.
- Two asynchronous threads and 20 computation threads, mostly idle.
- Snapshots ≤ 20, each ≤ 5s.
- Worst-case send memory: 48 MiB heavy and 16 MiB light, released at sendDone rather than client receipt.
- Sending times until sendDone:
  - Heavy response ≤ 130.2s; light ≤ 10.2s.
  - Batch ≤ 258.2791015625s sending and ≤ 404.2791015625s total; BATCH_WALL = 420s with reserve.
- Close at the first instant sendTime > 2s and A < 64 KiB/s·sendTime, with delay ≤ 200 ms.
- Credit delays closure until sendTime = A/R without extending the maximum:
  - 2 MiB accepted at sendTime = 1s permits stalling until 32s (PS1, PS2).
  - At sendTime = 31s, approximately 1s remains (PS3).
- Attacker connection ≤ 420s per batch, one reservation at any moment (S1), ≤ 20 items/s. After sendDone, only the connection and kernel buffer remain held until IDLE.
- PacerTrace: 4 MiB when enabled; disabled by default, enabled in RG. BridgeTrace: 1 MiB in the test build.
- Retrieval cost (P3, D91):
  - Worst case: each range scanned twice, up to 10 bisection levels added at resource limits, and two header requests per attempt.
  - RG3 (1024 dense blocks) is expected to need approximately 2–3 requests per approximately 25 blocks; this estimate must be measured.
- Restarting after reorg multiplies cost up to 4 times, then gives an explicit error.
- Starvation under a local attack is possible and disclosed; it does not affect consensus.
- A malicious site remains limited by its session despite self-navigation, consuming approximately no more than one connection's RpcGuard share. Limits:
  - ≤ 4 live or orphaned reads.
  - 50 messages/s, capacity 50.
  - 0.5 navigations/s, capacity 3.
  - 10 MiB reserved receive memory.
- Extension receive memory ≤ approximately 34.6 MiB reserved, plus parsing memory measured in MR10. ChunkCache ≤ 16 MiB.

**Extension-local storage (D104)**
- Total chrome.storage.local disk ≤ STORE_DISK_HARD = 256 MiB by Chrome's measurement, conditional on A15c.
- Steady state ≤ 64 sites × two records × 1442048 bytes, approximately 176 MiB, with approximately 22 MiB late-write reserve (D109). Site reserve is dynamic (D112): ≤ 16 seats for ≤ 60s after boots, affecting only new-site admission near capacity.
- Reaching the limit stops saving (4300 disk), without automatically deleting anything.
- Epoch items ≤ 8 names and ≤ 2 KiB at every moment (D134).
- Cost of this bound: a disappearance loop shorter than 60s after reaching it disables local storage until a generation survives 60s.

**Energy**
SHA-256 mining; losing work is unrewarded; D_att extends idling (T8).

**Income and issuance (D-V3-15, D-V3-19, D-V3-20)**
- pot_j = issuance(j) + (1−γ_tip)·tips_j + carry_j on a signed block, issuance(j) + tips_j + carry_j on a fallback block (incentives, Commitment and settlement). The signed proposer receives γ_tip = 50 % of its block's tips directly. The base fee is burned; it never falls below baseFeeMin = 10^7 wei (0.01 gwei).
- M3 issuance: issuance(h) = max(S0 >> ⌊h/H_half⌋, S_tail) for h ≥ 1 (0 at h = 0, and the S0 term is 0 after 256 halvings). Fresh network: S0 = 4 PCOL per 10 s block (34,560 PCOL per day, 12,623,040 PCOL in year 1), H_half = 12,623,040 blocks (4 years), S_tail = S0/32 = 0.125 PCOL from era 5 (h ≥ 63,115,200), about 394,200 PCOL per year after that. These are gross amounts; burns reduce the circulating supply.
- Supply bound (R23): the initial supply plus 2·S0·H_half + S_tail·2^64 = 4·10^24 + 2,305,843,009,314,678,272·10^18 wei = 2,305,843,009,318,678,272·10^18 wei, about 2.3058·10^36 (121 bits, below the 127-bit limit; the genesis builder records it in devnet.json). The tail term covers the whole u64 height domain: it is a representability bound, not a supply projection.
- Each pot's amount is fixed by its own template and body before mining; later carriers of late shares do not change it. The base burn of a settlement (about 57 % of R at κ=2 in the T17 estimate; 98.4 % in sv-fix with fallback and C=1, a fixture effect) goes to the carry into the next pot instead of being burned. roundBurn, payBurn, penaltyBurn and evidence burns remain burns.
- Escrow accounting: balance ≥ Σpending + heldTotal + burnedTotal + carry. A forced transfer into Escrow may raise its balance without raising these tracked amounts, so the surplus is nonnegative. Base-fee burns are accounted outside Escrow.
- Membership needs a bond: 20 PCOL HELD, or a bond still being earned from rewards (ACCRUING; incentives, Earn-in rules). The M_0 bonds are minted in genesis and included in R23.
- No creation fee (FN-2a, owner decision 10d of 2026-11-13): a new account, nonzero slot or code byte pays ordinary gas only. The live-state caps (ACCT 1,000,000, SLOT 4,000,000, CODE 2^30, subject to A13) stop further creations when full; filling them costs only gas, a risk the owner accepted (Q1 option a; threat.md, "Live-state caps"). The cap-raise path (fork version 3, FN-5) is implemented and activated only by a release that schedules it; no shared per-block growth ceiling exists.
- The continuous chain (H_END = 0) has no closing and no final block.

**Genesis allocation (FRESH-NETWORK-CP.md §3)**
- Four founders (the three US servers and the owner's PC), each with 1,000,000 PCOL: 999,980 PCOL on its reward address and its 20 PCOL HELD bond minted into Escrow. The initial supply is exactly 4,000,000 PCOL, and nothing else is allocated. (Owner decision pending: whether each server founder's reward address is its own hot miner key, which puts 999,980 PCOL on an internet-facing server, or a separate owner-held address.)
- This text moves no funds and does not assert current balances.

**Storage**
- The block journal is never compacted: in the worst case BODY_MAX × 8,640 blocks per day ≈ 1.7 GB per day (about 620 GB per year) per node. Operators should size disks for that or measure and alarm (the drafted launch disclosures in deploy/standalone/README.md carry this figure).
- The resident state bound is A13 store mode, measured in M1 (FN-3C); the live-state caps bound the state, and R_KEEP (2,048 blocks) bounds the reorganization depth a node can follow without a resync.

**Fixtures**
- FixtureMiner approximately 2^16 hashes per block, bounded by 2^28.
- CL-END approximately 700 blocks in minutes.
- RG approximately 1100 dense blocks and two competing branches, in minutes to be measured.
- RG14 approximately 35 minutes.
- PacerSim seconds.
- BR16 (10^4 random cases) minutes; BR19a and BR19b fractions of a second; BR17e seconds.

**Experimental funding**
Block rewards (M3 issuance, the pots' tips and the proposer's tip share) pay miners. Nothing pays storage, retrieval or RPC service: node operators bear those costs, as disclosed. Sustained operator funding (S4) is outside this specification.

**Target customer (hypothesis)**
dApp-interface publishers, researchers and educators needing an interface bound to their contract, verifiable, versioned and recoverable, less exposed to DNS or hosting replacement.

**Measurable value**
- Fraction displayed through LN; time to first display.
- Gas and publishing time per KiB; retrieval time.
- Detection of 100% of inconsistent tampering.
- Zero network confusion, ineligible signatures or accepted identity-violating blocks.
- Zero site writes without approval; zero external tabs without confirmation (BR).
- Interval deviation (T4-B); viewTargetCeil rate on an honest network.
- Light p99 under pressure (RG8); LogClient requests per dense block (RG3).

**Volunteer-publisher trial criterion (their existence is not claimed)**
- Support: ≥ 5 independent publishers for 30 days.
- Failure: fewer than two publishers, or cost per MiB exceeding a publisher-declared alternative.
- Inconclusive: 2–4 publishers.

No demand, revenue or performance is claimed.
