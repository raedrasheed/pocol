# LOG-CAP-AGGREGATE-ALTERNATIVES — avoiding permanent LOG_CAP exhaustion (design review)

> **PROPOSED for owner review — not approved, not implemented; LOG-CAP-AGGREGATE remains
> unapproved; this review does not block or change the R_keep (D-REORG-REDB) work.**

- Independent design review, 2026-10-23. No code, vector or specification file is changed, and the
  M1 reference is not edited. Every amendment below exists only in this file.
- Question: what LOG_CAP should be so that reaching it is not a permanent, chain-wide loss of logging.
  LOG-ACCT-01 (c)'s charge (`36 + 32·|topics| + |data|`, ≤ 546 logs per block) is taken as given.
- Relation to R_keep: no alternative touches fork choice (D133), R_keep, the block store or the
  metadata index. A2 and A3 read only the per-block `cum_log_bytes` that the D-REORG-REDB design
  freeze already keeps in offloaded entries (D-REORG-REDB-DESIGN-FREEZE.md:56–58).

## 1. Normative text relied on

| ref | Arabic (excerpt) | English |
|---|---|---|
| execution.md:82 | … وLOG_BLOCK_MAX=32 KiB، وLOG_CAP=1 GiB: توقف استثنائي. | "… LOG_BLOCK_MAX = 32 KiB, LOG_CAP = 1 GiB: exceptional halt." |
| execution.md:83 | TXBYTES_CAP وTX_CAP: الكتلة باطلة، والبنّاء يتخطى المعاملة. | "TXBYTES_CAP and TX_CAP: the block is invalid, and the builder skips the transaction." |
| execution.md:84 | LOG_BLOCK_MAX مع حد عدد السجلات المشتق في LG2 (546) أساس لِمّتي P1 وP3 (D89). | "LOG_BLOCK_MAX, with the log-count limit derived in LG2 (546), is the basis of lemmas P1 and P3." |
| execution.md:97 | GAS_LIMIT=8M بهدف 4M، وbaseFee بصيغة London. في الكتل الفارغة يستقر دون 8 wei | "GAS_LIMIT = 8M with a 4M target, baseFee by the London formula. In empty blocks it settles below 8 wei" |
| execution.md:106 | GeneralStateTests لـCancun عدا الاستثناءات المعلنة … | "Cancun GeneralStateTests, except the declared exceptions …" |
| storage.md:53 | … والسجل = 36+len. | "… and a log = 36 + len." |
| storage.md:57–58 | LG1 = d7 94 ‹20› c0 80، بمخزن 60 B. / LG2: 546 سجلًا تنجح، و547 تتوقف. | "LG1 = d7 94 ‹20› c0 80, stored in 60 B." / "LG2: 546 logs succeed, and 547 halt." |
| storage.md:70 | الكتل والإيصالات والسجلات والكود والفهارس حتى النهاية … | "[Retention:] blocks, receipts, logs, code and indexes until the end …" |
| storage.md:73 | عدد الكتل ≤ H_END=129600، وهو حد فعلي بـE1. المجموع نحو 19.65 GiB منطقيًا ونحو 29.5 GiB على القرص. | "The number of blocks ≤ H_END = 129600, an actual bound by E1. The total is about 19.65 GiB logical, 29.5 GiB on disk." |
| storage.md:77 | المخرجات: الكود 1.03، والسجلات 1.00، والإيصالات 0.50، والفهارس 0.63. | "Outputs [GiB]: code 1.03, logs 1.00, receipts 0.50, indexes 0.63." |
| governance.md:5, :9 | … T_blk=60s … / الاقتصاد [CP]: subsidy=2·10^18 … | "… T_blk = 60 s …" / "Economics [CP]: subsidy = 2·10^18 …" |
| governance.md:12–13 | الكتلة [CP]: GAS_LIMIT=8000000، وbaseFee0=10^9، … وBODY_MAX=196608. / السقوف [CP]: … وTXBYTES_CAP=2^33، وTX_CAP=10^7 … | "Block [CP]: GAS_LIMIT = 8,000,000, baseFee0 = 10^9, … BODY_MAX = 196608." / "Caps [CP]: … TXBYTES_CAP = 2^33, TX_CAP = 10^7 …" — LOG_CAP and LOG_BLOCK_MAX are not listed, nor among CP's 52 fields (consensus.md:18–25) |
| governance.md:15 | الاحتفاظ (محلي): K_hist=32، وHIST_TRIE_BUDGET=1717986918، وR_keep=2048. | "Retention (local): K_hist = 32, HIST_TRIE_BUDGET = 1717986918, R_keep = 2048." |
| governance.md:16 | النهاية [CP]: H_END=129600، وT_END=g_ts+10368000، وH_CLOSE=129440 … | "End [CP]: H_END = 129600, T_END = g_ts + 10368000, H_CLOSE = 129440 …" |
| governance.md:71 | … غير ok على العقدة الحقيقية يعني رفض الإقلاع بـ−32018. | "… a non-ok [ParamGate] result on the real node means refusing to boot with −32018." |
| governance.md:100 | R21: trie التاريخي، والاستثناء الوحيد simnet مع retentionStress. | "R21: the historical trie; the only exception is simnet with retentionStress." |
| governance.md:197, :200 | بلوغ سقف: تجميد وتنبيه. / تراجع أعمق من R_keep: checkpoint يدوي. | "Reaching a cap: freeze and alert." / "A rollback deeper than R_keep: manual checkpoint." |
| consensus.md:29 | المعلمات المحلية خارج CP لا تدخل genesisHash ولا الصلاحية: … | "Local parameters outside CP enter neither genesisHash nor validity: …" |
| consensus.md:46 | ForkSchedule = [(v, H_v)] … activeVersion(h) = آخر v بحيث H_v ≤ h. | "ForkSchedule = [(v, H_v)] …; activeVersion(h) = the last v with H_v ≤ h." |
| network.md:360 | … و−32017 خارج نافذة trie. | "… −32017: outside the trie window." |
| validation.md:985–986 | 50..1073 … كل منها معاملة … تستدعي LogBomb(1) (546 سجلًا، نحو 32 KiB). / B يتفرع من 999، وكتل 1000'..1080' تستدعي LogBomb(2) | "[RG:] blocks 50..1073 each hold a transaction calling LogBomb(1) (546 logs, about 32 KiB)." / "B forks from 999, and blocks 1000'..1080' call LogBomb(2)" |
| validation.md:2061, :2065 | sv-fix مع chainId=777903، وH_END=4000 … / ثوابت البناء caps-test (simnet فقط، غائبة عن الإصدار): LOG_CAP=2^24 … | "[caps/64:] sv-fix with chainId = 777903, H_END = 4000 …" / "caps-test build constants (simnet only, absent from the release): LOG_CAP = 2^24 …" |
| validation.md:2087–2088 | الاستجابة المعلنة … أو توقف استثنائي … أو تجميد. / بعدها 20 كتلة على الأقل تجتاز T17. | "[T18b:] exactly the declared response: … exceptional halt … or freeze." / "Then at least 20 blocks pass T17." |
| validation.md:2129 | تجميد المكوّن لا السلسلة ← T18b. | "Freezing the component, not the chain → T18b." |

**Not specified anywhere:** what LOG_CAP sums (the LOG-CAP-AGGREGATE question); what "freeze"
(governance.md:197) does operationally; any reset, decay or window for any cap; an RPC answer for logs a
node no longer holds; how the 1.00 GiB inventory line treats stored per-log overhead (LG1 stores an
empty LOG0, 24 B of RLP, in 60 B against a charge of 36 B; if len is the RLP length, a stored log
exceeds its charge by 24–33 B).

## 2. The problem, re-derived

Under "chain", the counter is the charged log bytes of the canonical ancestors plus this block's, and
it never decreases.
- Once the room left is smaller than a transaction's log charge, that transaction halts (status 0,
  whole gas limit charged, LOG-ACCT-01 Effects), on that branch and every descendant.
- Block production continues: halts never invalidate a block (fallback body empty, consensus.md:111,
  :166).
- No rule lowers the counter. governance.md:200 covers rollbacks deeper than R_keep, and a checkpoint
  on the same chain keeps the same ancestors. Only a new genesis would restart it; the text does not
  provide for one.

| profile (source) | H_END | H_END·LOG_BLOCK_MAX | vs cap | earliest LogCap halt |
|---|---:|---:|---|---|
| network (governance.md:16) | 129,600 | 4,246,732,800 B = 3.955 GiB | > 1 GiB: **reachable** | block 32,769 (2^30 / 2^15 = 32,768 full blocks) |
| prototype devnet (`lp4-execution/src/devnet.rs:38`, not normative) | 20,000 | 655,360,000 B = 625 MiB | < 1 GiB | never |
| end/full (storage.md:81) | 700 | 22,937,600 B = 21.9 MiB | < 1 GiB | never |
| caps/64, caps-test cap 2^24 (validation.md:2061, :2065) | 4,000 | 131,072,000 B = 125 MiB | > 16 MiB | 513th LogBomb block (2^24 / 32,760 = 512.1) |

**Cost and timing on the network profile:**
- One LOG0 of 32,732 zero bytes is charged exactly 32,768 B and costs about 288,000 gas under Cancun
  (execution.md:106): 375 + 8·32,732 = 262,231, memory 5,113, intrinsic 21,000. That is 3.6% of
  GAS_LIMIT and 7.2% of the 4M target, so under London (execution.md:97) filling does not raise baseFee.
- 32,768 such blocks use about 9.45·10^9 gas: 9.45·10^18 wei at 1 gwei (about 4.7 block subsidies,
  governance.md:9), or under 7.6·10^10 wei at the idle base fee (< 8 wei). Priority fees are not
  specified, and a proposer can include its own transactions.
- The fill takes about 22.8 days at T_blk = 60 s. After it, 96,832 blocks remain (74.7% of H_END,
  about 67 days), and every logging transaction in them halts.

This matches the spec's stated posture, "freeze the component, not the chain" (validation.md:2129) and
"freeze and alert" (governance.md:197). What follows the alert is not specified.

**Correction to INDEPENDENT-REVIEW-SIX-PROPOSALS.md:64–66.** Its "under 'block' (vacuous) the term
would be unbounded over H_END" is imprecise. A canonical chain has at most H_END blocks (storage.md:73),
so even without LOG_CAP the charged log bytes are ≤ H_END·LOG_BLOCK_MAX. LOG_CAP does not make the bound
finite; it lowers it, from 3.955 GiB to 1 GiB on network, and does nothing on the devnet (625 MiB).
A13's `LOG_CAP·33/32` = 1,056.0 MiB could be `min(LOG_CAP, H_END·LOG_BLOCK_MAX)·33/32` = 644.5 MiB on
the devnet under any reading (a local model tightening, noted here, not proposed).

Common to every alternative: a filler can take a block's whole 32 KiB or its 546 logs, and logging
transactions ordered after it in that block halt (LogBytes or LogCount). This per-block auction exists
today and is not changed.

## 3. Alternatives

### A1 — make LOG_CAP provably unreachable (gate rule)
- **(a) Normative change.**
  - Add a new ParamGate rule, R27: `H_END·LOG_BLOCK_MAX ≤ LOG_CAP` (u128). Its only exception is the
    simnet caps-test build, following the pattern of R21 (governance.md:100).
  - Then choose one of three variants:
    - **A1a:** LOG_CAP = 2^32 at execution.md:82 (and in its copy, FINAL_DESIGN.md:819). This
      admits H_END ≤ 131,072; network passes with a margin of 48,234,496 B (1,472 blocks).
    - **A1b:** LOG_BLOCK_MAX ≤ ⌊2^30 / 129,600⌋ = 8,285 B.
    - **A1c:** network H_END ≤ 32,768, with T_END, H_CLOSE and T_CLOSE re-derived under R13.
- **(b) Consensus.** Validity does not change. For an admitted profile LogCap never fires, so receipts,
  gas and roots equal those of the vacuous "block" reading. R27 is a boot gate: a violating profile is
  refused with −32018 (governance.md:71). Deterministic (a constant and CP); fork choice untouched.
  A1b also halts every block over 8,285 B, so receipts and roots differ for log-heavy blocks; A1c
  changes CP.
- **(c) Storage.**
  - A1a: network logs ≤ 3.955 GiB; storage.md:77 goes from 1.00 to 3.96 GiB and the storage.md:73
    total from 19.65 to about 22.6 GiB logical (+15%). The on-disk factor is not derivable from the text.
  - A1a, A13 term `min(…)·33/32`: devnet 644.5 MiB (−411.5); network 4,176.6 MiB. Network-scale caps
    are already refused by the 6 GiB node (A13-DERIVATION.md:143). A1b and A1c keep 1 GiB.
- **(d) Compatibility.**
  - A1a: LG1, LG2 and RG unchanged. The caps-test needs the R27 exception (125 MiB > 2^24); with it,
    T18b L6 still reaches the halt. No existing chain holds more than 1 GiB (the devnet at most
    625 MiB), so no outcome changes. genesisHash is unchanged, since neither constant is a CP field:
    not a hard fork for any existing chain. Proposal vectors G1 and G2 must be rebased to 2^32 − 100.
  - A1b: breaks LG2 (546·60 = 32,760 > 8,285), LogBomb (governance.md:168), the basis of P1 and P3
    (execution.md:84) and RG. Incompatible with accepted vectors.
  - A1c: a new network genesisHash, and a network shortened from 90 days of blocks to at most 22.75.
- **(e) Liveness.** There is no cumulative stall for an admitted profile. Only the per-block limit
  remains.
- **(f) Complexity.** For A1a: one constant, one gate predicate, one exception and the text edits.
  Lowest risk. The halt path stays as defence in depth and is still exercised by the caps-test.

### A2 — sliding window
- **(a) Normative change.**
  - Replace LOG_CAP at execution.md:82 with two protocol constants, LOG_WIN and LOG_WIN_CAP. They
    are outside CP, so genesisHash is unchanged.
  - Rule: "the charged log bytes of this block and its LOG_WIN−1 canonical ancestors ≤ LOG_WIN_CAP;
    the first transaction that would exceed it halts exceptionally." Gate: LOG_WIN_CAP ≥ LOG_BLOCK_MAX.
  - Illustrative values: W = 8,192, cap = 2^26 (64 MiB: 2,048 full blocks, 8 KiB per block on
    average). Smaller pairs with the same 1 GiB lifetime bound break RG: W = 2,048 with 2^24, or
    4,096 with 2^25, halts RG branch B (1,031 LogBomb blocks · 32,760 = 33,775,560 B > 2^25).
- **(b) Consensus.** Halts only; receipts, gas and roots differ from the status quo wherever a window
  binds. The window sum is a pure function of the branch, cum(h−1) − cum(h−W) from the existing
  per-block counter: node-consistent, independent of fork choice, recomputed per branch on a reorg with
  nothing to undo. A snapshot restore needs cum at h−W (the status quo needs cum at h); neither is
  specified for snapshots.
- **(c) Storage.** Lifetime bytes ≤ min(⌈H_END/W⌉·cap, H_END·LOG_BLOCK_MAX). Network: 16 · 64 MiB =
  1 GiB, so the inventory and the A13 term (1,056.0 MiB) are unchanged. Devnet: 3 · 64 MiB, so the A13
  term is 198.0 MiB.
- **(d) Compatibility.** LG1 and LG2 unchanged. RG fits (at most 1,031 consecutive LogBomb blocks,
  under 2,048). caps/64 reaches the release cap at its 2,049th LogBomb block, so the 2^24 build
  constant can be dropped; recovery (h > 8,192) is beyond caps/64's H_END and needs a unit test. An
  existing chain is affected only if it carries more than 64 MiB in some 8,192-block window; none
  exists outside local test runs (proposals/README.md). Deployment on an existing chain would go
  through ForkSchedule (consensus.md:46), genesisHash unchanged.
- **(e) Liveness.** A filler saturates the window in 2,048 blocks. Logging then halts for at most
  W − 2,048 = 6,144 blocks (about 4.3 days), until old blocks slide out. A persistent filler can hold
  throughput at 8 KiB per block and must outbid others on priority fee for each freed byte, because
  log bytes have no price of their own. The stall is not permanent.
- **(f) Complexity.** Low to moderate: one ancestor lookup at depth W. The risks are off-by-one
  errors at the window edges and at h < W, so vectors are needed.

### A3 — epoch reset
- **(a) Normative change.** "The charged log bytes of the blocks of epoch ⌊(h−1)/E⌋ up to this block
  ≤ LOG_EPOCH_CAP." Illustrative values: E = 8,192, cap = 2^26.
- **(b) Consensus.** As A2. The sum is cum(h−1) − cum(E·⌊(h−1)/E⌋).
- **(c) Storage.** As A2: 16 epochs on network give 1 GiB; the devnet term is 198.0 MiB.
- **(d) Compatibility.** As A2. The reset at h = 8,193 is not reachable in caps/64.
- **(e) Liveness.** A stall lasts at most E − 2,048 = 6,144 blocks and ends at a predictable
  boundary. Every reset invites a rush: one filler can take the whole budget in an epoch's first
  2,048 blocks, every epoch.
- **(f) Complexity.** Slightly simpler than A2. The boundary rush is the cost.

### A4 — remove LOG_CAP from consensus; total log storage becomes a local retention matter
- **(a) Normative change.**
  - Delete "وLOG_CAP=1 GiB" at execution.md:82 (and FINAL_DESIGN.md:819) and "LOG_CAP=2^24" at
    validation.md:2065; storage.md:77 becomes H_END·LOG_BLOCK_MAX.
  - Optional pruning also needs: storage.md:70 amended (logs are kept "until the end"); a local
    parameter at governance.md:15 (outside CP, consensus.md:29); an RPC answer for pruned ranges
    (−32017 means only "outside the trie window", network.md:360, so widen it or add a code); and
    archival replicas (storage.md:85, T25 full-retention nodes) keeping every log.
- **(b) Consensus.** For every admitted profile the outcomes are those of A1a: no LogCap halt, ever.
  LOG_BLOCK_MAX and the 546-log limit stay consensus. Pruning is local (no rule reads old logs, and
  re-execution regenerates them); deterministic, fork choice untouched. This is the consensus-safe form
  of the "retained" idea: retention bounds storage without being a consensus input.
- **(c) Storage.** Without pruning, as A1a. With pruning, the resident logs are
  min(H_END, LOG_RETAIN)·LOG_BLOCK_MAX·33/32.
- **(d) Compatibility.** LG1, LG2 and RG are unchanged. T18b L6 loses its LogCap case and still tests
  LOG_BLOCK_MAX and LG2. G1–G3 become ok. No existing chain is affected, and genesisHash is unchanged.
- **(e) Liveness.** There is no cliff. Pruning narrows only RPC history.
- **(f) Complexity.** The consensus change is the smallest of all, a deletion. Pruning would be local
  code of moderate size. The declared halt and its test are lost.

### A5 — economic pressure instead of a cliff
- **(a) Normative change.** Change the gas schedule; the Cancun conformance (execution.md:106) then
  needs a declared exception for LOG0–LOG4. Examples: data gas of 8·2^⌊C/2^28⌋ per byte, where C is
  the counter of the ancestors; or a separate log base fee, which means a new header field and update
  rule.
- **(b) Consensus.** gasUsed, receipts and roots change for every logging transaction past the first
  step. gasUsed feeds the London baseFee, which is a header check (consensus.md:165), so headers
  differ as well. It is deterministic only if the gas is implemented exactly. Fork choice is
  independent.
- **(c) Storage.** Pricing bounds nothing by itself; the bound stays H_END·LOG_BLOCK_MAX. If LOG_CAP
  is kept, the cliff remains and is only costlier to reach.
- **(d) Compatibility.** LogBomb's gas rises with C, so LG2 holds only if its gas limit covers the
  rise. Any chain whose counter passes the first step forks; the devnet can reach 625 MiB, beyond
  256 MiB. Wallets and eth_estimateGas change.
- **(e) Liveness.** It is a soft ceiling. Without decay, late users pay for early users' logs.
- **(f) Complexity.** High: custom LOG gas in revm and a departure from the Cancun tests. Highest risk.

### A6 — status quo, the "chain" reading (for comparison)
- **(a)** No change beyond LOG-CAP-AGGREGATE's own amendment. **(b)** Halts once the counter nears
  2^30. Deterministic and independent of fork choice.
- **(c)** At most 1 GiB: the inventory's 1.00 GiB and the A13 term of 1,056.0 MiB.
- **(d)** Everything stays as accepted: LG1, LG2, RG, and the caps-test halting at the 513th LogBomb
  block.
- **(e)** A permanent freeze of logging on network, possible from block 32,769. It costs 9.45·10^18 wei
  at 1 gwei, or far less at the idle base fee. Recovery is only out of band, through a new genesis.
- **(f)** Already implemented provisionally (`Halt::LogCap`).

## 4. Comparison

| | permanent cliff | worst logging stall (network) | lifetime logs (network) | A13 logs term devnet / network (MiB) | texts touched | LG1/LG2, RG | caps-test | existing chains / genesisHash | complexity |
|---|---|---|---|---|---|---|---|---|---|
| A1a | no | none (per block only) | ≤ 3.955 GiB | 644.5 / 4,176.6 | ex:82, new R27, val:2065, sto:73/77 | unchanged | R27 exception | none / unchanged | very low |
| A1b | no | none | ≤ 1 GiB | ≤ 1,056 | ex:82, :84 | **LG2, RG broken** | exception | log-heavy blocks differ | low, incompatible |
| A1c | no | none | ≤ 1 GiB | ≤ 1,056 | gov:16 (CP) | unchanged | exception | network genesisHash changes | low; network life −75% |
| A2 | no | ≤ 6,144 blocks per saturation | ≤ 1 GiB | 198.0 / 1,056.0 | ex:82, val:2065 | unchanged at 8192 / 2^26 | release value reachable | none in practice / unchanged | low–moderate |
| A3 | no | ≤ 6,144 blocks, at known boundaries | ≤ 1 GiB | 198.0 / 1,056.0 | ex:82, val:2065 | unchanged at 8192 / 2^26 | release value reachable | none in practice / unchanged | low–moderate |
| A4 | no | none | ≤ 3.955 GiB (less if pruned locally) | 644.5 / 4,176.6 without pruning | ex:82, val:2065, sto:77 (+ sto:70, net:360 if pruning) | unchanged | LogCap case dropped | none / unchanged | lowest (consensus) |
| A5 | only if the cap is kept | soft | H_END-bounded, or 1 GiB | 644.5 / 4,176.6, or 1,056 | gas schedule, Cancun exception | LG2 depends on gas | changes | devnet chains fork | high |
| A6 | **yes** | permanent, from block 32,769 | ≤ 1 GiB | 1,056.0 / 1,056.0 | none | unchanged | as is | none | done |

## 5. Recommendation (a recommendation only; the owner decides)

**1. Adopt "chain" together with A1a.** Keep LOG-CAP-AGGREGATE's chain reading and make the cap
provably unreachable: LOG_CAP = 2^32, with the gate R27 `H_END·LOG_BLOCK_MAX ≤ LOG_CAP` and the simnet
caps-test as its only exception.
It removes the cliff for every admitted profile, changes no outcome on any existing chain, adds no
consensus mechanism, and leaves genesisHash, LG1, LG2, RG and T18b L6 intact. The price is +2.955 GiB
of logical log storage on the network inventory (+15%).

Draft text for the owner:
- English: "LOG_CAP = 2^32 bounds the sum of charged log bytes over the canonical chain, this block
  included; the first transaction that would exceed it halts exceptionally. R27: H_END·LOG_BLOCK_MAX
  ≤ LOG_CAP; the only exception is the simnet caps-test build."
- Arabic:
  > يحد LOG_CAP=2^32 مجموع بايتات السجلات المحسوبة على السلسلة القانونية شاملة الكتلة الحالية، وأول معاملة تتجاوزه تتوقف استثنائيًا. R27: H_END·LOG_BLOCK_MAX ≤ LOG_CAP، والاستثناء الوحيد بناء caps-test في simnet.
- Follow-on edits: validation.md:2065 reads "LOG_CAP=2^24 (استثناء R27)"; storage.md:77 logs → 3.96,
  with the storage.md:73 total re-derived; A13 uses `min(LOG_CAP, H_END·LOG_BLOCK_MAX)·33/32`, which is
  local.

**2. Equivalent simplification: A4 without pruning.** If the owner prefers deleting a rule that can
never fire, A4 gives the same consensus outcome with fewer words. Pruning can be added later as a local
matter.

**3. If the 1.00 GiB lifetime inventory is a hard requirement: A2**, at W = 8,192 and cap = 2^26 (both
illustrative). The cost is a new consensus mechanism and a griefable throttle of up to 6,144 blocks.
Prefer it over A3, which has a boundary rush.

**Not recommended:** A1b (breaks accepted LG2 and the RG fixture); A1c (shortens the network by 75%
and changes its genesisHash); A5 (changes the gas rules and does not remove the cliff on its own); A6,
unless the owner explicitly accepts a permanent "component freeze" (validation.md:2129) that is cheaply
reachable at about 25% of the network's life.

## 6. Related, not analysed here: TX_CAP and TXBYTES_CAP

The implementation reads both as chain-cumulative (A13-DERIVATION.md:42), and reaching either makes the
block invalid (execution.md:83). Both are reachable before H_END on network:
- TX_CAP = 10^7 against at most 380 transactions per block: 26,316 full-gas blocks, or 52,632 blocks
  at the 4M target, where baseFee stays flat.
- TXBYTES_CAP = 2^33 against BODY_MAX = 196,608: at least 43,691 blocks of about 0.8M gas with
  zero-byte calldata.

Exhausting either stops every user transaction, not only logging. The same A1–A4 shapes apply. They
are flagged for the owner and not decided here.
