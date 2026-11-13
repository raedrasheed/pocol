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
- Template execution, waiting until X^W (36s in genesis, 10s in dev), and precomputed fallback.
- Shares ≤ 2.3 KiB; evidence ≤ 8 KiB.
- Header-only broadcast saves up to 192 KiB per block per peer.

Verification:
- Fixed-width ASERT ≤ 512 bits.
- RP ≤ 14 headers (approximately 43 KiB) and ≤ 13 ASERT calculations.
- genesisPre decoding: 341 B in GSV1, approximately 44 KiB in end/full.
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

**Income and issuance**
- pot = subsidy+blockTips; base fee burned.
- Subsidies ≤ subsidy·(H_END−1) = 2·10^18·129599 wei, approximately 259198 coins. R23 prevents overflow.
- Estimated baseBurn approximately 57% of R at κ=2 (T17). In sv-fix with fallback and C=1 it reaches 98.4%, a fixture effect.
- Identity requires a held bond; M_0 bonds are minted in genesis and included in R23.
- Closing takes 160 blocks within H_END.

**Storage**
Approximately 29.5 GiB × 3 replicas until ts_F+30 days.

**Fixtures**
- FixtureMiner approximately 2^16 hashes per block, bounded by 2^28.
- CL-END approximately 700 blocks in minutes.
- RG approximately 1100 dense blocks and two competing branches, in minutes to be measured.
- RG14 approximately 35 minutes.
- PacerSim seconds.
- BR16 (10^4 random cases) minutes; BR19a and BR19b fractions of a second; BR17e seconds.

**Experimental funding**
Publishing fees fund part of the cost. The project bears storage, retrieval and RPC costs, as disclosed. A permanent network requires S4, outside this specification.

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

---

## v3 amendment — issuance, tips, carry and fees (D-V3-15, 19–21)

Only when `v3_active(h)` is true, replace the finite-fixture issuance description with the approved M3 target below. Existing spec-1 issuance and rewards remain unchanged. This records rules to implement and validate, not deployed behavior. See [the decision record](../../decisions/V3-DECISIONS.md).

Let k=floor(h/H_half). Issuance is 0 at h=0 and otherwise `max(k>=256 ? 0 : S0>>k, S_tail)`. The approved initial rate is 2 PCOL per 60 seconds scaled by T_blk, with a four-year halving interval and tail S0/32. At the candidate 30-second block time, S0=1 PCOL, H_half=4,207,680 blocks and S_tail=0.03125 PCOL. Validate `2*S0*H_half + S_tail*2^64 < 2^127`. These are gross issuance amounts; classified burns still reduce circulating supply.

For a signed block j, pay gamma_tip=50% of tips directly to its proposer reward address; the remaining tips enter the shared pot. For fallback, all tips enter the pot. Phase-2 closeBlock(j) sets `pending[j].amount=issuance(j)+(1-gamma_tip)*tips+carry` for signed blocks, and `issuance(j)+tips+carry` for fallback, then zeroes carry. Phase-0 settlement precedes closeBlock. Uncovered base-burn share remainder goes to carry, while roundBurn and other classified burns remain burns. On a finite final block, burn residual carry.

Thus each pot amount is fixed by its own template and body before mining. Later share carriers do not retroactively change an already fixed pot. The Escrow accounting invariant is `balance >= SUM(pending)+held+burned+carry`. Preserve the existing nonnegative surplus: a forced transfer into Escrow may increase its balance without increasing these tracked amounts. Account for external base-fee burns separately (there is no creation-fee burn, FN-2a). Require T17, conservation at every step and old-chain differential tests.

The v3 base-fee floor is 10^7 wei (0.01 gwei). There is no creation fee (owner decision 10d of 2026-11-13, FN-2a; it superseded the earlier sender-paid creation burn `F0*CAP/(CAP-used)`): a new account, nonzero slot or code byte pays ordinary gas only, and the live-state caps stay A03 halts charging the declared gas. Apply the cap failure rule consistently in execution, builder and estimation. The fresh-network caps (ACCT 1,000,000, SLOT 4,000,000, CODE 2^30) are subject to A13; filling them costs only gas, a risk the owner accepted (Q1 option a, threat.md v3 amendment), with a cap-raise path (C10) required before launch. No new shared per-block growth ceiling is added.

The four public genesis allocations and owner-operated time-lock/reporting plan are disclosed in the decision record. This amendment does not move funds or change allocation, and it does not assert current balances.
