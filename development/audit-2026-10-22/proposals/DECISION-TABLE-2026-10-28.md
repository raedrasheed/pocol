# Open decisions: the table for the owner (2026-10-28)

**Nothing in this file is approved or adopted, and this request is not read as approving any of it.** Every entry below is a decision the owner
has not taken. The code runs each one's current behaviour. Where the code carries a recommended
alternative, it does so *provisionally* (proposals/README.md, "Implemented provisionally"), which is not a
decision.

Paths: `AUD/` = `development/audit-2026-10-22/`, `P/` = `AUD/proposals/`, code paths are under `prototypes/`.
Code line numbers were checked on the working tree on 2026-10-28 (before this round's commits, which move some of them). Several line numbers cited in the source
documents have since moved; see "Source inconsistencies" at the end.

**Already approved, not open (for reference only):**
- D-1 option (a): storage conformance S1–S5 (P/DECISION-PACKAGE.md §1 "D-1"; FINDING-STATUS.json `decisions.approvedByOwner`).
- D-6 (a): local state checkpoints that keep the block history the spec requires (DECISION-PACKAGE.md §4 "D-6").
- D-7 at 4·GAS_LIMIT: the local pool gas bound (DECISION-PACKAGE.md §2 "D-7"; FINDING-STATUS.json).

## Summary

| id | decision | kind | recommendation recorded in the docs | blocks |
|---|---|---|---|---|
| LOG-ACCT-01 | how a log is charged against LOG_BLOCK_MAX, and whether 546 is a limit | NORMATIVE | (c): 36 + 32·\|topics\| + \|data\|, ≤ 546 logs per block | the amendment to execution.md §السقوف; the final status of its vectors and of the provisional code |
| D-A03-DIRTY | what "dirty" counts; a halt whose residue does not fit | NORMATIVE | A: net change; residue that does not fit → block invalid, builder skips | the amendment; the exactness of A13/RECV bounds that use ACCT_DIRTY_BLOCK_MAX (rely on A or A-touched) |
| D-A03-CAPS | what SLOT_CAP/ACCT_CAP count; when SYS_SLOT_MAX is checked | NORMATIVE | 2: user state only; SYS_SLOT_MAX after the system phases | the amendment; genesis acceptance rule (`check_caps`) final form |
| D-BODY-MAX | encoded body or item sum | NORMATIVE | (a): \|RLP([txs, evidence])\| ≤ BODY_MAX on every path | the clarification to consensus.md:56 |
| D-A03-SYSUSER | what the user-context SYS allowance counts | NORMATIVE | (a): program storage keys/programs; tips and ChunkFactory nonce do not count | the amendment; R18's user allowance relies on it |
| LOG-CAP-AGGREGATE | the scope of LOG_CAP | NORMATIVE | chain (OPEN-DECISIONS row 6 words it "per branch") | the amendment; the A13 history term's reading; N1 builds on it |
| D-2 | the per-gas worst case (MODEXP) | NORMATIVE / profile | (0) no normative change, gated by row 2 (ii) — **conflicts with DECISION-PACKAGE.md's (a)** | if (a)/(a2): a new devnet genesisHash; D-2 acceptance tests |
| R14/R19 workload | which blocks the boot calibration times | local admission policy | (ii) calibrate on the slowest kinds | clearing condition `timing-not-a-bound` (network boot in Specification scope); D-2 (0) relies on it |
| D-3 | the caps regime (O1/O2/O3) | NORMATIVE | O2 on the network profile, with R28/R29 scoped to network and the 21,000-gas lemma stated (or TX_CAP ≥ 61,560,000); devnet caps kept | N1–N3 code, gates R27–R29, new genesis vectors, the network genesis |
| N1 | R27 + LOG_CAP = 2^32 | NORMATIVE | part of O2 (= A1a) | R27 boot gate; G1/G2 vectors rebase; D-5 release |
| N2 | R28 + network TX_CAP = 5·10^7 | NORMATIVE (CP) | part of O2, with the two corrections | network genesisHash; R28 gate test |
| N3 | R29 + network TXBYTES_CAP = 24·2^30 | NORMATIVE (CP) | part of O2 | network genesisHash; R29 gate test |
| N4 | O3 linear allowance (alternative to N1–N3) | NORMATIVE | not recommended unless the 19.65 GiB inventory is a hard limit | — (only if chosen) |
| D-4 | the interface items N5–N8 | NORMATIVE / interface | approve all four | N5–N8 implementation and their RPC/pool tests |
| N5 | pool −32016 {cap} and eviction | NORMATIVE / interface | approve | pool eviction test across a reorg |
| N6 | `pocol_getCaps` fields | NORMATIVE / interface | approve the field list | serving `pocol_getCaps` with defined fields |
| N7 | eth_call/eth_estimateGas report an A03 halt | NORMATIVE / interface | approve | the RPC test for it |
| N8 | scope sentence in consensus.md:170 | NORMATIVE (spec text) | approve | the spec edit |
| D-5 | the identity of LOG_CAP (and other out-of-CP constants) | NORMATIVE / wire | (b) with the release plan (protocolVersion + constants digest in Hello, R27 boot gate, same release as N1) | Hello wire change; AT1–AT8; any binary able to boot the network profile |
| P1 | RPC rate across connections | local service policy | C: keep-alive, bucket per connection | R26 `rpc-absent` clearing (indirectly); its RPC test |
| P2 | CANCEL_MAX for precompile runs | local service policy | B: −32020 above a per-precompile gas cap | `cancel_max_ms` (R26 `rpc-absent`); deciding vectors |
| P3 | effectful method finishing after its deadline | local service policy | A: deadline checked before the effect | a test pinning the chosen option |
| P4 | methods outside the classification table | local service policy | B: table lines for served methods; −32601 before admission otherwise | RPCGUARD.md classification rows (PARTIAL) |
| key path | A13 counts 64 branches (65 nodes) per key path | local (an assumption in the A13 bound, not a rule) | keep 65 | nothing (the store-mode bound fits without it) |
| store default | store mode as the network profile's default | local default | keep opt-in until a near-cap network-scale state is run and measured | a network boot with default flags (whole-state A13 fails) |

Network boot overall: even with every entry above decided, the network profile does not boot today. Its
other refusals are structural (no code path admits a network profile; no network genesis), R21 (pocold does
not implement K_eff), and Specification-scope conditions (AUD/NETWORK-BOOT-GATES.md §1–§3).

---

## Consensus proposals (P/README.md; all "PROPOSED, not approved")

### LOG-ACCT-01

| field | content |
|---|---|
| 1. Current behaviour | Alternative (c) implemented provisionally (P/LOG-ACCT-01.md "Implementation status"). Charge per log: `lp4-execution/src/user_phase.rs:37` (`36+32*topics+data`); count limit `LOG_COUNT_MAX=546` at `:21`; checks `:216` (LogCount) and `:217` (LogBytes) in `BlockMeter::check`. A halted transaction adds nothing. |
| 2. Proposed change | Amendment to execution.md §السقوف (P/LOG-ACCT-01.md "Proposed amendment"): "One log is charged 36 + 32·\|topics\| + \|data\| bytes. The charges of a block's logs sum to at most LOG_BLOCK_MAX (equality allowed), and a block holds at most 546 logs. The first transaction that would exceed either limit halts exceptionally. A halted transaction adds nothing to either sum." |
| 3. Alternatives | (a) 36 + \|data\|, no count limit (violates P1: 910 empty LOG0 fit). (b) \|data\|, no count limit (violates LG2). (c) 36 + 32·\|topics\| + \|data\|, ≤ 546 per block (recommended). (d) 36 + \|RLP(log)\|, no count limit (violates LG2). (P/LOG-ACCT-01.md "Alternatives".) |
| 4a. Security | Under (a), (b) the P1 / receipt bound (network.md:202, :304, ≤ 546 logs, < 400 KiB) is violated (P/LOG-ACCT-01.md table). Liveness risk "low per block" (P/INDEPENDENT-REVIEW-SIX-PROPOSALS.md row 1). |
| 4b. Performance | Not stated in the sources beyond the receipt-size bound above. |
| 4c. Compatibility | Every accepted M1 vector unchanged under (c) (LG1, LG2 hold). Blocks valid under 090eae4 with an over-limit transaction stay valid but get different receipts and roots; no such chain outside local test runs. genesisHash unchanged (the six keep every genesisHash, P/OPEN-DECISIONS.md row 6). Another alternative changes code, vectors and the validity of test-devnet blocks (ibid.). The A13 history bound depends on LOG_CAP with (c)'s charge. Wire/RPC: not stated. |
| 5. Recommendation | "(c) recommended" (P/LOG-ACCT-01.md); "(c): 36 + 32·topics + data, ≤ 546 logs per block. The only reading consistent with every quoted text" (P/DECISION-PACKAGE.md §5a). |
| 6. Blocked / not blocked | Blocked: the amendment entering the spec text (it "exists only in its proposal file until the owner approves it", P/README.md "Rules"); the code's status changes from provisional to adopted. Not blocked: testing the other repairs on the provisional code; the model vectors and Rust replays already pass (P/DECISION-PACKAGE.md §5a). No source names a gate, boot step or release blocked by it. |
| 7. Kind | NORMATIVE (receipts, roots). |

### D-A03-DIRTY

| field | content |
|---|---|
| 1. Current behaviour | Alternative A implemented provisionally (P/D-A03-DIRTY.md). Net dirty sets checked as halts at `lp4-execution/src/user_phase.rs:219` (DirtyKeys) and `:220` (DirtyAccounts); `apply_halt` (`:238`) returns `UserError::DirtyAccounts(i)` (`:247`) or `UserError::AcctCap(i)` (`:248`) when the residue does not fit; pocold's builder drops index `i` and retries (`pocold/src/node.rs:943` `build_signed_template`). |
| 2. Proposed change | Amendment (P/D-A03-DIRTY.md "Proposed amendment"): "DIRTY_BLOCK_MAX and ACCT_DIRTY_BLOCK_MAX bound the union, over a block's USER transactions, of the storage keys and accounts whose value a transaction changes (net, per transaction). A transaction whose changes would exceed either bound halts exceptionally. If the halt's own effects (nonce and fee of the sender, the coinbase tip) would exceed ACCT_DIRTY_BLOCK_MAX, or create an account beyond ACCT_CAP, the block is invalid, and the builder skips the transaction." |
| 3. Alternatives | A: net change; non-fitting residue → block invalid, builder skips (recommended). B: as A, residue exempt; block always valid; effective bound ACCT_DIRTY_BLOCK_MAX + halted senders + 1. A-touched: every touched account counts. Rejected: drop the halted transaction without effect (replay; contradicts body commitment). |
| 4a. Security | Liveness "low and only at GAS_LIMIT ≥ ~120M"; A's invalid case mirrors TX_CAP (P/INDEPENDENT-REVIEW-SIX-PROPOSALS.md row 2). Under B every bound derived from ACCT_DIRTY_BLOCK_MAX (incl. A13, RECV) must use the larger figure (P/D-A03-DIRTY.md "Effects"). |
| 4b. Performance | At devnet GAS_LIMIT 8M the limits are unaffordable (≈ 9,300 gas per account change, ≥ 2,900 per key), so they bind only at higher GAS_LIMIT profiles (ibid.). No timing figure stated. |
| 4c. Compatibility | No accepted M1 vector affected. A adds one block-invalid condition of the same kind as TX_CAP. Receipts/gas/roots differ in disagreeing cases. genesisHash unchanged (P/OPEN-DECISIONS.md row 6). Wire/RPC: not stated. |
| 5. Recommendation | "A (recommended)" (P/D-A03-DIRTY.md); "A: a net change counts; a halt whose residue would exceed the limit makes the block invalid, and the builder skips it" (P/DECISION-PACKAGE.md §5a). |
| 6. Blocked / not blocked | Blocked: the amendment; the A13/RECV derivations stay exact only under A or A-touched (P/D-A03-DIRTY.md "Effects"), so choosing B would reopen them. Not blocked: other repairs; vectors replay (`apply::tests::a03::proposals::d_a03_dirty_vectors`). No gate or boot step named. |
| 7. Kind | NORMATIVE (validity, roots). |

### D-A03-CAPS

| field | content |
|---|---|
| 1. Current behaviour | Alternative 2 implemented provisionally (P/D-A03-CAPS.md). User-state halts at `lp4-execution/src/user_phase.rs:222` (SlotCap) and `:223` (AcctCap); genesis `check_caps` at `lp4-execution/src/genesis_builder.rs:51`; SYS_SLOT_MAX after the system phases in `GenesisImage::verify_candidate` (`genesis_builder.rs:28`). |
| 2. Proposed change | Amendment (P/D-A03-CAPS.md): "SLOT_CAP and ACCT_CAP bound the nonzero storage slots and existing accounts outside the five system programs. A USER transaction that would exceed either halts exceptionally. The system programs' nonzero storage slots are bounded by SYS_SLOT_MAX after each block's system phases. A block that exceeds it is invalid. The genesis must satisfy both bounds." |
| 3. Alternatives | 1: caps count every account and slot incl. programs; a system write past the cap invalidates the block. 2: user-state caps; SYS_SLOT_MAX after system phases (recommended). |
| 4a. Security | "medium under alt 1": the builder must predict the system phase or produce invalid blocks, which can stall production near the cap; alt 2 removes the coupling (P/INDEPENDENT-REVIEW-SIX-PROPOSALS.md row 3). Under 2 the post-phase check is unreachable for an admitted profile (R18). |
| 4b. Performance | Not stated. |
| 4c. Compatibility | Genesis acceptance changes for a genesis whose total slots exceed SLOT_CAP while its user slots do not; unreachable at devnet and network values. The LP4 post-phase-2 atomicity test uses SYS_SLOT_MAX. genesisHash unchanged under the recommendation (P/OPEN-DECISIONS.md row 6). |
| 5. Recommendation | "2 (recommended)" (P/D-A03-CAPS.md); "2: user state only; SYS_SLOT_MAX checked after the system phases" (P/DECISION-PACKAGE.md §5a). |
| 6. Blocked / not blocked | Blocked: the amendment. Not blocked: other repairs; replay tests pass. No gate or boot step named. |
| 7. Kind | NORMATIVE (validity, genesis acceptance). |

### D-BODY-MAX

| field | content |
|---|---|
| 1. Current behaviour | Alternative (a) implemented provisionally: `pocold/src/chain.rs:548–550` refuses `b.body.encode().len() > BODY_MAX` ("1 BODY_MAX"), as `Body::decode` does; the producer drops transactions until the encoded body fits (P/D-BODY-MAX.md). |
| 2. Proposed change | Clarification to consensus.md:56 (P/D-BODY-MAX.md): "BODY_MAX bounds the length of the RLP encoding of Body = [txs, evidence], including every list and string header; validation and decoding apply the same bound." |
| 3. Alternatives | (a) encoded length on every path (recommended). (b) item sum; decode must then accept up to BODY_MAX + list headers. (c) item sum + 1,024 B margin; stricter than (a) for many transactions. |
| 4a. Security | Removes a self-inconsistency (a block one path accepted and another rejected; peers dropped it at decode, own journal replay failed); "improving liveness" (P/INDEPENDENT-REVIEW-SIX-PROPOSALS.md row 4). |
| 4b. Performance | Not stated. |
| 4c. Compatibility | No spec-valid block becomes invalid. Under (b), decode changes and RECV bounds need list-header slack. genesisHash unchanged. |
| 5. Recommendation | "(a) recommended" (P/D-BODY-MAX.md); "(a): the encoded body, the literal reading and what decode does" (P/DECISION-PACKAGE.md §5a). |
| 6. Blocked / not blocked | Blocked: the clarification text. Not blocked: other repairs; `consensus::d_body_max_vectors` passes. No gate named. |
| 7. Kind | NORMATIVE (validity). |

### D-A03-SYSUSER

| field | content |
|---|---|
| 1. Current behaviour | Alternative (a) implemented provisionally; vacuous for the v1 programs (P/D-A03-SYSUSER.md). Checks at `lp4-execution/src/user_phase.rs:224` (SysWriteClosing), `:225` (SysKeys vs SYS_KEYS_USER_MAX), `:226` (SysAccounts vs `USER_SYS_ACCOUNTS=16`, `:31`). |
| 2. Proposed change | (P/D-A03-SYSUSER.md) "The user-context SYS allowance counts storage keys of the five system programs changed by USER transactions (≤ SYS_KEYS_USER_MAX per block) and the programs whose storage they change (≤ 16), and is zero while closing (SysWriteClosing). Balance changes (D70) and the ChunkFactory nonce are not system writes." |
| 3. Alternatives | (a) as above (recommended). (b) an Escrow tip credit also counts; every tipping transaction during closing would halt (contradicts D70). |
| 4a. Security | Low; (b) would halt every tipping transaction during closing (P/INDEPENDENT-REVIEW-SIX-PROPOSALS.md row 5). |
| 4b. Performance | Not stated. |
| 4c. Compatibility | No validity change (halts only). With v1 programs (a) never triggers. Receipts/roots differ only in S2-like cases under (b). genesisHash unchanged. |
| 5. Recommendation | "(a) recommended" (P/D-A03-SYSUSER.md); "(a): storage writes to the programs only; balance credits do not count" (P/DECISION-PACKAGE.md §5a). |
| 6. Blocked / not blocked | Blocked: the amendment; R18's user allowance "relies on it" (P/D-A03-SYSUSER.md). Not blocked: everything with v1 programs (vacuous). |
| 7. Kind | NORMATIVE (halts). |

### LOG-CAP-AGGREGATE

| field | content |
|---|---|
| 1. Current behaviour | "chain" implemented provisionally (P/LOG-CAP-AGGREGATE.md). `LOG_CAP=1<<30` (`1<<24` under feature caps-test) at `lp4-execution/src/user_phase.rs:25–26`; halt at `:218`; the counter is the parent's `cum_log_bytes` (`pocold/src/chain.rs:422`) plus this block's (`:579`), so it is per branch (P/CAPS-SUSTAINABILITY.md §1 "scope"). |
| 2. Proposed change | (P/LOG-CAP-AGGREGATE.md) "LOG_CAP bounds the sum of charged log bytes over the canonical chain, this block included; the first transaction that would exceed it halts exceptionally." |
| 3. Alternatives | chain: cumulative over ancestors + this block (recommended). block: per block (vacuous, LOG_BLOCK_MAX ≪ LOG_CAP). retained: logs in the local RetentionStore window (not consensus-safe). A further family (A1–A6, avoiding a permanent cliff) is in P/LOG-CAP-AGGREGATE-ALTERNATIVES.md and is folded into D-3/N1. |
| 4a. Security | "HIGH in principle": a permanent, chain-wide halt of every logging transaction once reached; network H_END·LOG_BLOCK_MAX ≈ 3.955 GiB > 1 GiB so reachable from block 32,769 (P/INDEPENDENT-REVIEW-SIX-PROPOSALS.md; P/LOG-CAP-AGGREGATE-ALTERNATIVES.md §2), for about 6.6·10^10 wei at the idle base fee (P/CAPS-SUSTAINABILITY.md §2). |
| 4b. Performance | Not stated beyond the A13 history term (LOG_CAP·33/32 = 1,056.0 MiB, P/LOG-CAP-AGGREGATE-ALTERNATIVES.md §2). |
| 4c. Compatibility | No validity change. Unreachable at 1 GiB for a short devnet (devnet max 625 MiB). Every node computes the same value under "chain"; not under "retained". genesisHash unchanged (LOG_CAP is not a CP field). |
| 5. Recommendation | "chain (recommended)" (P/LOG-CAP-AGGREGATE.md). P/OPEN-DECISIONS.md row 6 and P/DECISION-PACKAGE.md §5a word it "per branch" / "cumulative per branch", while the amendment text says "canonical chain" (see inconsistencies). The independent review flags the cliff as a separate open question, not decided (P/INDEPENDENT-REVIEW-SIX-PROPOSALS.md). |
| 6. Blocked / not blocked | Blocked: the amendment; the reading behind A13's history term; N1 (LOG_CAP = 2^32 + R27) is defined on top of the "chain" reading (P/LOG-CAP-AGGREGATE-ALTERNATIVES.md §5). Not blocked: replay `log_cap_aggregate_vectors`; caps-test exhaustion test. |
| 7. Kind | NORMATIVE (halts, receipts, roots). |

---

## Execution time

### D-2 — the per-gas worst case (MODEXP)

| field | content |
|---|---|
| 1. Current behaviour | Cancun pricing with two result-preserving MODEXP fast paths (`b46658a`, `0c2ad75`): address 5 is `lp4-execution/src/modexp.rs:24–26` (`berlin_run` falls back to revm's), installed at `lp4-execution/src/native.rs:144–147`. Slowest measured block: MODEXP 9–16-byte modulus on revm's path, 0.47 s validate / 0.51 s import, ≈ 60 ns per gas; margin to devnet t_exec 1 s ≈ 2×; host variance up to 2×; the 384-shape grid is not a proof of the maximum (P/OPEN-DECISIONS.md row 1; AUD/A13-DERIVATION.md §3b-2). Devnet CP: d_att 25, d_fb_wait 3, gas_limit 8M (`lp4-execution/src/devnet.rs:37–39`). |
| 2. Proposed change | (0): no normative change; rely on row 2 (ii) as the boot gate that refuses a too-slow host (P/OPEN-DECISIONS.md row 1). |
| 3. Alternatives | (0) no change, Cancun + fast paths. (a) devnet t_exec 2 s with dFbWait 3 → 5 and D_att 25 → 35. (a2) devnet GAS_LIMIT 8M → 6M. (b) reprice MODEXP. (c) cap precompile inputs. (d) not normative: a further fast path for 9–16-byte moduli. (P/OPEN-DECISIONS.md row 1.) |
| 4a. Security | The residual risk under (0) is slower hosts and shapes outside the grid, which row 2 (ii) would catch at boot (P/OPEN-DECISIONS.md row 1). (d): "more code to verify", consensus-critical arithmetic for about 2× on one class (ibid.). |
| 4b. Performance | (0): ≤ 0.51 s per block on this host. (a2): worst MODEXP block ≈ 0.72 s at 6M (P/DECISION-PACKAGE.md §2 "D-2", pre-fast-path figure). (a): longer windows. |
| 4c. Compatibility | (0): no change to consensus, genesisHash or vectors. (a): a new devnet genesisHash (dFbWait, D_att are CP fields; AUD/evidence/memory-r14/d2-gate.log also changes local Δ_rel 5 → 8, Γ 5 → 8). (a2): new devnet genesisHash, smaller blocks. (b)/(c): hard fork of every vector. (d): no consensus change if exact. (P/OPEN-DECISIONS.md row 1; AUD/NETWORK-BOOT-GATES.md "Devnet constraints".) |
| 5. Recommendation | P/OPEN-DECISIONS.md row 1 (2026-10-27): "**(0)**, with row 2's calibration on the slowest kinds as the gate that refuses a host too slow for them. (a) stays the fallback if a target host fails that gate. (d) only if a target host needs it". **Conflict:** P/DECISION-PACKAGE.md §2 "D-2" still reads "Recommendation: (a)" and calls (a) "a local timing value … genesisHash none", which d2-gate.log and FINDING-STATUS.json contradict. |
| 6. Blocked / not blocked | Blocked: D-2 acceptance tests ("admission passes with the new devnet timing; the R14/R19 calibration on a MODEXP block stays ≤ t_exec; the vectors are unchanged", P/DECISION-PACKAGE.md §2) — the second depends on row 2; under (a)/(a2), regenerated devnet genesis files. Not blocked: the network profile's timing (t_exec 3 s covers 0.51 s, AUD/NETWORK-BOOT-GATES.md §3); devnet boot as today. |
| 7. Kind | NORMATIVE / profile ((0) and (d) need no normative change). |

### R14/R19 calibration workload

| field | content |
|---|---|
| 1. Current behaviour | The boot calibration is one GAS_LIMIT compute loop (`JUMPDEST; PUSH1 0; JUMP`, `lp4-execution/src/admission.rs:329`), timed in memory and, for a store-mode node, through a scratch store (`measure_on_store`, `:343`); the larger per phase is used. 8–19 ms per phase, ≈ 1.3–2.5 ns per gas. The admission records condition `timing-not-a-bound` (`admission.rs:320`). (P/OPEN-DECISIONS.md row 2; AUD/A13-DERIVATION.md §3, §3b.) |
| 2. Proposed change | (ii): calibrate on the slowest kinds — MODEXP 9–16-byte and 1,024-byte moduli on revm's path, BN pairing and mul, keccak over 32 KiB, ecrecover, many transfers — and gate on the largest per phase (P/OPEN-DECISIONS.md row 2). No exact rule text is recorded. |
| 3. Alternatives | (i) keep the loop and the condition. (ii) slowest kinds, gate on the largest per phase. (iii) a mix with a percentile. |
| 4a. Security | Under (i) R14/R19 pass without measuring the blocks they bound, on this or a slower host (row 2). (ii) would refuse a host about twice as slow as this one. |
| 4b. Performance | (ii): boot takes "tens of seconds longer (a slow block per calibration sample, in each mode)" (row 2). |
| 4c. Compatibility | "No consensus effect" (row 2). No genesisHash, vector, wire or RPC change stated. It changes what "validate_p99" means in the admission report (row 2 heading). |
| 5. Recommendation | "**(ii)**: the gate then measures the blocks R14/R19 bound, and row 1 (0) relies on it" (P/OPEN-DECISIONS.md row 2). |
| 6. Blocked / not blocked | Blocked: clearing `timing-not-a-bound`, a Specification-scope refusal for a network node ("a worst-case workload (decision row 2 …)", AUD/NETWORK-BOOT-GATES.md §3); D-2 option (0)'s safety argument. Not blocked: devnet boot in experimental-devnet scope (the condition is reported, not refused there); the other network-boot layers are independent. |
| 7. Kind | Local admission policy. |

---

## Caps sustainability (P/CAPS-SUSTAINABILITY.md §5, §5a; D-3/D-4 group N1–N8 as shown)

Grouping in the sources: D-3 = the regime, O2 = N1 + N2 + N3, O3 = N4 (P/DECISION-PACKAGE.md §3 "D-3");
D-4 = N5–N8 (ibid. "D-4"). Current code for all: TX_CAP/TXBYTES_CAP item 16 at `pocold/src/chain.rs:568`
via `Chain::within_caps` `:602–605` (per branch, from the parent's counters); LOG_CAP as in
LOG-CAP-AGGREGATE above. Conformance items already implemented (builder skip, template locking,
caps-test LOG_CAP) are not open (P/CAPS-SUSTAINABILITY.md §6).

### D-3 — the caps regime

| field | content |
|---|---|
| 1. Current behaviour | O1: network TX_CAP 10^7, TXBYTES_CAP 2^33, LOG_CAP 2^30, all cumulative per branch and reachable before H_END on the network profile (26,316 / 43,695 / 32,768 blocks); after them user activity on that branch freezes (P/CAPS-SUSTAINABILITY.md §1–§3; P/D3-D5-HEND-ANALYSIS.md §2.3). |
| 2. Proposed change | O2 on the network profile: gates R27 `H_END·LOG_BLOCK_MAX ≤ LOG_CAP`, R28 `H_END·⌊GAS_LIMIT/21000⌋ ≤ TX_CAP`, R29 `H_END·TXB_BLOCK ≤ TXBYTES_CAP` (TXB_BLOCK = 196,592); LOG_CAP 2^32, TX_CAP 5·10^7, TXBYTES_CAP 24·2^30; only exception the simnet caps-test (P/CAPS-SUSTAINABILITY.md §5). With two corrections: R28/R29 scoped to the network profile, and the 21,000-gas lemma stated in R28 or TX_CAP ≥ 61,560,000 (P/DECISION-PACKAGE.md §3; P/D3-D5-HEND-ANALYSIS.md §2.5 X3, X4). |
| 3. Alternatives | O1 keep, freeze as specified. O2 unreachable by construction (N1–N3). O3 linear allowance (N4). O4 sliding window. O5 remove caps from validity (CP schema change). (P/CAPS-SUSTAINABILITY.md §5; DECISION-PACKAGE.md lists O1–O3 only.) |
| 4a. Security | O1: permanent freeze reachable organically (20–34% average use) or by griefing for 6.6·10^10–1.5·10^12 wei at the idle base fee. O2: no cumulative freeze; proofs of unreachability with margins 1.12% / 1.13% / 1.50%; TX_CAP's rests on the 21,000-gas lemma (checked over 2,178 SSTORE sequences, not a formal proof). O3: a griefable rate limit. (P/CAPS-SUSTAINABILITY.md §5; P/D3-D5-HEND-ANALYSIS.md §2.3.) |
| 4b. Performance | O2: ≈ 43.2 GiB logical inventory (+23), ≈ 65 GiB on disk at 1.5×. Resident memory: whole-state devnet O2 model 11.12–14.62 GiB, refused by the 6 GiB gate in every case; "needs canonical history off-heap first" (P/CAPS-SUSTAINABILITY.md §5, §5b). Network **store-mode** A13 model at the O2 caps, computed 2026-10-28 (`pocold/tests/network_budget.rs::network_store_model_at_the_o2_caps`): **5,616,872,049 B (5.231 GiB), 5.978 GiB required against the 6 GiB budget**, against 5,581,321,485 B (5.198 GiB, 5.941 GiB required) today. It fits, with about 22 MB of headroom on the required budget. LOG_CAP's share is added from the history term's formula, since LOG_CAP is a build constant. |
| 4c. Compatibility | O2: a new network genesisHash (TX caps are CP); no network chain exists; GSV1 unchanged; devnet genesis files regenerated; networks.json gets the new hash; no RPC change; LOG_CAP change needs no devnet re-genesis (H_END·32,768 ≤ both values). O1: none. O3: new vectors; genesisHash changes only if BURST is CP. |
| 5. Recommendation | "**O2**, scoped to the network profile, with the 21,000-gas lemma stated in R28 (or TX_CAP ≥ 61,560,000)" (P/OPEN-DECISIONS.md row 3); "O2 on the network profile, and keep the approved devnet caps" (P/DECISION-PACKAGE.md §3); "approve D-3 with the two corrections and D-5 (b) together with the release plan" (ibid.). |
| 6. Blocked / not blocked | Blocked: N1–N3 code; D-3 acceptance tests (gate tests for R27–R29; new genesis vectors from `caps_model.py` replayed by `caps_identity.rs`; unreachability at H_END; caps-test still exhausts — P/DECISION-PACKAGE.md §3); creating the network genesis (P/D3-D5-HEND-ANALYSIS.md §4.3 step 5). Not blocked: the devnet (keeps the approved D-A13-PROFILE caps); the implemented conformance items. |
| 7. Kind | NORMATIVE. |

### N1 — R27 + LOG_CAP = 2^32 (= A1a)

| field | content |
|---|---|
| 1. Current behaviour | LOG_CAP = 2^30 (`user_phase.rs:25`); no R27. |
| 2. Proposed change | "LOG_CAP = 2^32 bounds the sum of charged log bytes over the canonical chain, this block included; the first transaction that would exceed it halts exceptionally. R27: H_END·LOG_BLOCK_MAX ≤ LOG_CAP; the only exception is the simnet caps-test build." Texts: execution.md:82 (and FINAL_DESIGN.md:819), governance.md gate, validation.md:2065 ("LOG_CAP=2^24 (استثناء R27)"), storage.md:73/77 (P/LOG-CAP-AGGREGATE-ALTERNATIVES.md §5; P/CAPS-SUSTAINABILITY.md §5a). |
| 3. Alternatives | A1b (LOG_BLOCK_MAX ≤ 8,285 B; breaks LG2, RG); A1c (network H_END ≤ 32,768; new genesisHash); A2 sliding window; A3 epoch reset; A4 remove LOG_CAP; A5 log gas pricing; A6 status quo (P/LOG-CAP-AGGREGATE-ALTERNATIVES.md §3). |
| 4a. Security | Removes the permanent logging cliff for every admitted profile (ibid. §5). |
| 4b. Performance | +2.955 GiB logical logs on the network inventory (+15%); A13 term `min(LOG_CAP, H_END·LOG_BLOCK_MAX)·33/32` (devnet 644.5 MiB) only if resources.rs implements the tightening — as coded it uses LOG_CAP (2^32) (P/CAPS-SUSTAINABILITY.md §5b). |
| 4c. Compatibility | genesisHash unchanged (LOG_CAP not in CP); LG1, LG2, RG, T18b L6 intact; G1/G2 vectors rebased to 2^32 − 100; no existing chain affected. Two nodes with different LOG_CAP would split silently on a long profile → D-5. |
| 5. Recommendation | "Adopt 'chain' together with A1a" (P/LOG-CAP-AGGREGATE-ALTERNATIVES.md §5); part of O2 (P/OPEN-DECISIONS.md row 3). |
| 6. Blocked / not blocked | Blocked: R27 boot gate and AT3, AT4 (P/D3-D5-HEND-ANALYSIS.md §4.4); the D-5 release (ships with N1). Not blocked: devnet operation (already unreachable at 2^30). |
| 7. Kind | NORMATIVE (protocol constant + gate). |

### N2 — R28 + network TX_CAP = 5·10^7

| field | content |
|---|---|
| 1. Current behaviour | Network TX_CAP = 10^7 (governance.md:13); item 16 at `chain.rs:568`, `:602–605`. |
| 2. Proposed change | "R28: H_END·⌊GAS_LIMIT/21000⌋ ≤ TX_CAP; network TX_CAP = 5·10^7" (P/CAPS-SUSTAINABILITY.md §5a), scoped to the network profile and with the 21,000-gas lemma stated, or TX_CAP ≥ 61,560,000 (P/DECISION-PACKAGE.md §3 corrections). |
| 3. Alternatives | Keep 10^7 (O1); N4 (O3); lemma-free sizing ≥ 61,560,000 (e.g. 6.2·10^7, P/D3-D5-HEND-ANALYSIS.md X4). |
| 4a. Security | Unreachable under lemma L1(b), margin 752,000 (1.50%); without L1 a block holds up to 475 transactions and 5·10^7 is not enough. |
| 4b. Performance | Receipts 0.50 → 2.5 GiB, indexes 0.63 → 3.15 GiB of inventory; history term +380·TX_CAP (P/CAPS-SUSTAINABILITY.md §5). |
| 4c. Compatibility | CP field 38: a new network genesisHash and netKey; `store::open` refuses old data dirs (P/CAPS-SUSTAINABILITY.md §4); no network chain exists. As worded unscoped, R28 refuses devnet and caps/64 (X3). |
| 5. Recommendation | Part of O2, with the two corrections (P/OPEN-DECISIONS.md row 3). |
| 6. Blocked / not blocked | Blocked: R28 gate test; network genesis vectors. Not blocked: devnet. |
| 7. Kind | NORMATIVE (CP value + gate). |

### N3 — R29 + network TXBYTES_CAP = 24·2^30

| field | content |
|---|---|
| 1. Current behaviour | Network TXBYTES_CAP = 2^33 (governance.md:13); item 16 as N2. |
| 2. Proposed change | "R29: H_END·TXB_BLOCK(BODY_MAX, TX_MAX) ≤ TXBYTES_CAP; network TXBYTES_CAP = 24·2^30" (P/CAPS-SUSTAINABILITY.md §5a); network-scoped (X3). Optional R29′ `H_END·BODY_MAX ≤ TXBYTES_CAP` (P/D3-D5-HEND-ANALYSIS.md X5). |
| 3. Alternatives | Keep 2^33; N4; R29′ simplification. |
| 4a. Security | Unreachable by proof, margin 291,480,576 B (1.13%). |
| 4b. Performance | Transactions 8.0 → 24.0 GiB of inventory; history term TXBYTES_CAP (P/CAPS-SUSTAINABILITY.md §5). |
| 4c. Compatibility | CP field 37: new network genesisHash; same as N2. |
| 5. Recommendation | Part of O2 (P/OPEN-DECISIONS.md row 3). |
| 6. Blocked / not blocked | Blocked: R29 gate test; network genesis vectors. Not blocked: devnet. |
| 7. Kind | NORMATIVE (CP value + gate). |

### N4 — O3 linear allowance (alternative to N1–N3)

| field | content |
|---|---|
| 1. Current behaviour | Not implemented. |
| 2. Proposed change | "cum_X(block h) ≤ ⌊h·CAP_X/H_END⌋ + BURST_X for txs, tx bytes and log bytes (a log halt, a tx block invalid)"; texts execution.md:82–83, consensus.md:170 (P/CAPS-SUSTAINABILITY.md §5, §5a). |
| 3. Alternatives | N1–N3 (O2); O4 window; O1. |
| 4a. Security | Never permanent; a griefer can take each block's allowance; honest users must outbid on tips. |
| 4b. Performance | Inventory and resident memory unchanged; throughput capped at the lifetime average plus the burst (network 77 tx, 66 KB, 8.3 KB logs per block). |
| 4c. Compatibility | A new validity/halt rule; new vectors at h boundaries; genesisHash changes only if BURST is CP; `pocol_getCaps` should expose the allowance. |
| 5. Recommendation | Only "if the 19.65 GiB inventory is a hard requirement" (P/CAPS-SUSTAINABILITY.md §5 item 3; P/D3-D5-HEND-ANALYSIS.md §5). |
| 6. Blocked / not blocked | Nothing is blocked on it unless chosen. |
| 7. Kind | NORMATIVE. |

### D-4 — the interface items N5–N8 (umbrella)

| field | content |
|---|---|
| 1. Current behaviour | Pool, RPC and spec text around the caps are unspecified or silent (P/OPEN-DECISIONS.md row 4); details per item below. |
| 2. Proposed change | Approve or reject each of N5–N8. |
| 3. Alternatives | Per item: approve / reject. |
| 4a. Security | N5 ends pool squatting near the byte boundary (P/CAPS-SUSTAINABILITY.md §3a). |
| 4b. Performance | Not stated. |
| 4c. Compatibility | "RPC and pool behaviour; spec text; no validity change" (P/OPEN-DECISIONS.md row 4). |
| 5. Recommendation | "**approve all four**" (P/OPEN-DECISIONS.md row 4; P/DECISION-PACKAGE.md §3 "D-4"). |
| 6. Blocked / not blocked | Blocked: "an RPC test for each; a pool eviction test across a reorg" (P/DECISION-PACKAGE.md §3). Not blocked: "the existing caps tests unchanged" (ibid.); conformance items E-1 (pool after F) and E-2 (`pocol_getClosing` etc.), queued separately (P/DECISION-PACKAGE.md §3 "Spec gaps"). |
| 7. Kind | NORMATIVE / interface. |

### N5 — pool −32016 {cap} and eviction

| field | content |
|---|---|
| 1. Current behaviour | The pool accepts transactions that can never be included; they stay until POOL_MAX, then −32016 "pool full" (`pocold/src/node.rs:126`); `PoolMeter` checks only SLOT_CAP, ACCT_CAP, SYS_SLOT_MAX (`lp4-execution/src/user_phase.rs:187`, `:205`); no expiry or eviction (P/CAPS-SUSTAINABILITY.md §3a, §3b). |
| 2. Proposed change | "pool: −32016 {cap} for a transaction that does not fit the head branch's remaining TX_CAP/TXBYTES_CAP room …, and such transactions leave the pool when a new head leaves them no room" (P/CAPS-SUSTAINABILITY.md §5a); text network.md:149–153. |
| 3. Alternatives | Approve; reject (status quo). |
| 4a. Security | Ends free pool squatting (≈ 256 × 131,072 B transactions fill POOL_MAX_BYTES) (§3a). |
| 4b. Performance | Not stated. |
| 4c. Compatibility | New RPC error detail {cap}; re-checking after a reorg is already approved (network.md:153). No validity change. |
| 5. Recommendation | "approve" (P/DECISION-PACKAGE.md §3 "D-4"). |
| 6. Blocked / not blocked | Blocked: pool eviction test across a reorg; RPC test. Not blocked: consensus. |
| 7. Kind | NORMATIVE / interface (pool, RPC). |

### N6 — `pocol_getCaps` fields

| field | content |
|---|---|
| 1. Current behaviour | Listed in the light table (`pocold/src/rpc.rs:523`) but not served: no handler, so −32601 (`rpc.rs:761`) (P/CAPS-SUSTAINABILITY.md §3a). |
| 2. Proposed change | "pocol_getCaps → {txCount, txBytes, logBytes, txCap, txBytesCap, logCap} of the head branch, plus an alert for governance.md:197's 'freeze and alert'" (P/CAPS-SUSTAINABILITY.md §5a). |
| 3. Alternatives | Approve the field list; reject. |
| 4a. Security | Not stated. |
| 4b. Performance | Not stated. |
| 4c. Compatibility | RPC addition; serving the method is conformance (network.md:333), its fields are not specified (P/DECISION-PACKAGE.md §3). |
| 5. Recommendation | "approve the field list" (P/DECISION-PACKAGE.md §3). |
| 6. Blocked / not blocked | Blocked: serving `pocol_getCaps` with defined fields and its RPC test. Not blocked: other RPC conformance. |
| 7. Kind | NORMATIVE / interface (RPC). |

### N7 — eth_call / eth_estimateGas report an A03 halt

| field | content |
|---|---|
| 1. Current behaviour | `simulate` (`pocold/src/rpc.rs:1044`) runs without the A03 meter and reports success for a transaction that will halt and cost its whole gas limit (P/CAPS-SUSTAINABILITY.md §3b). |
| 2. Proposed change | "eth_call and eth_estimateGas report an A03 halt (or apply the meter) instead of success"; text execution.md:101–102 (P/CAPS-SUSTAINABILITY.md §5a). |
| 3. Alternatives | Approve; reject. |
| 4a. Security | Not stated beyond users being misled about halts. |
| 4b. Performance | Not stated. |
| 4c. Compatibility | RPC-visible result change; no validity change. |
| 5. Recommendation | "approve" (P/DECISION-PACKAGE.md §3). |
| 6. Blocked / not blocked | Blocked: its RPC test. Not blocked: consensus. |
| 7. Kind | NORMATIVE / interface (RPC). |

### N8 — scope sentence in consensus.md:170

| field | content |
|---|---|
| 1. Current behaviour | consensus.md:170 reads "16. txBytes وtxCount ضمن السقوف" with no scope; the code reads the parent's branch counters (`chain.rs:602–605`). |
| 2. Proposed change | "state the scope explicitly: 'the counts of the branch up to the parent plus this block' (item 16; LOG-CAP-AGGREGATE for logs)" (P/CAPS-SUSTAINABILITY.md §5a). |
| 3. Alternatives | Approve; reject. |
| 4a. Security | Not stated. |
| 4b. Performance | None. |
| 4c. Compatibility | Spec text only; "what the code does" (P/DECISION-PACKAGE.md §3). |
| 5. Recommendation | "approve" (P/DECISION-PACKAGE.md §3). |
| 6. Blocked / not blocked | Blocked: the spec edit. Not blocked: code (already per branch, tested in both directions since `e985ce1`, P/CAPS-SUSTAINABILITY.md §7d). |
| 7. Kind | NORMATIVE (spec text). |

### D-5 — the identity of LOG_CAP

| field | content |
|---|---|
| 1. Current behaviour | LOG_CAP is outside the CP and genesisHash. pocold's Hello is `{chain_id, genesis, head, height, work, node}` (`pocold/src/net.rs:65`; arity 7 at `:101`), with no protocolVersion although network.md:7's Status has it. Two nodes differing in LOG_CAP split silently once a branch crosses the smaller cap (P/CAPS-SUSTAINABILITY.md §4). |
| 2. Proposed change | (b) with the release plan: R27 boot gate on every profile but caps-test (PA); protocolVersion in Hello (PB); a consensus-constants digest `constsDigest(v)` in Hello/Status and `pocol_getParams`, refuse on mismatch (PC); ship with N1, no later than the first binary able to boot the network profile; every later constant change takes a new protocolVersion and ForkSchedule entry (PD). Hello/Status = [chainId, genesisHash, protocolVersion, constsDigest, headHash, height, totalWork] (P/D3-D5-HEND-ANALYSIS.md §4.2–§4.3). |
| 3. Alternatives | (a) nothing. (b) version in ForkSchedule and Hello. (c) LOG_CAP into the CP (schema 2, 52 → 53 fields). Finer: PA–PF (P/D3-D5-HEND-ANALYSIS.md §4.2). |
| 4a. Security | (b+) catches every out-of-CP constant mismatch at handshake; (b) alone misses same-version binaries with different constants (P/D3-D5-HEND-ANALYSIS.md §4.3). |
| 4b. Performance | None stated. |
| 4c. Compatibility | Hello wire format changes (arity 7 → 8 or more); old and new pocold stop peering (loud refusal); GSV1 unchanged; no devnet re-genesis (2^30 → 2^32 neutral for H_END ≤ 32,768); a data directory failing R27 is refused (−32018). (c): every vector, refdec.js, rp.js, devnet re-genesis. |
| 5. Recommendation | "**(b)**, with the release plan: protocolVersion and a constants digest in Hello, R27 as a boot gate, in the same release as N1" (P/OPEN-DECISIONS.md row 5); "D-5 (b+) … recommended" (P/D3-D5-HEND-ANALYSIS.md §5). P/DECISION-PACKAGE.md §3 "D-5" says only "(b)" in its table and adds the digest in its later release-plan paragraph. |
| 6. Blocked / not blocked | Blocked: the Hello change (also an open conformance gap, but it "changes the wire format, so it goes with the release plan", P/DECISION-PACKAGE.md §3); AT1–AT8 (P/D3-D5-HEND-ANALYSIS.md §4.4); the N1 release; by constraint, any binary able to boot the network profile (ibid. §4.3 step 5). Not blocked: current devnets (neutral). |
| 7. Kind | NORMATIVE / wire. |

---

## RpcGuard (P/RPCGUARD-R2-PROPOSALS.md: "PROPOSED — not approved"; local service policy, no consensus effect)

Note: P/RPCGUARD-R2-PROPOSALS.md itself records no recommendation for any of P1–P4. The recommendations
below come only from P/DECISION-PACKAGE.md §5b and P/OPEN-DECISIONS.md row 7.

### P1 — RPC request rate across connections (network.md:215–218)

| field | content |
|---|---|
| 1. Current behaviour | Option A: pocold closes every connection after one response (`pocold/src/rpc.rs:259`, `Connection: close`) and applies the per-connection bucket to that one request (`rate_refused`, `rpc.rs:237`); no aggregate bound beyond RPC_CONN_MAX and the queues. |
| 2. Proposed change | C: HTTP keep-alive (IDLE 30 s, next request read after sendDone), bucket per connection. No proposed text is recorded for C (the source drafts text only for B). |
| 3. Alternatives | A (current) per connection, one request. B one bucket per server (640/s, 640). C keep-alive + per-connection bucket. D per source IP (stage A, rejected). |
| 4a. Security | A: no aggregate bound when clients reconnect. D evadable through 127/8. C: aggregate as the spec assumes (≤ RPC_CONN_MAX·rate). |
| 4b. Performance | Not stated. |
| 4c. Compatibility | C: transport change; clients that read to EOF must honour Content-Length. No consensus effect. |
| 5. Recommendation | "**C**: HTTP keep-alive with the bucket per connection" (P/DECISION-PACKAGE.md §5b). |
| 6. Blocked / not blocked | Blocked: "an RPC test for each chosen option" (P/DECISION-PACKAGE.md §5b). AUD/NETWORK-BOOT-GATES.md §3 says R26 `rpc-absent` clears by enforcing mechanisms and that "RpcGuard P1–P4 are decisions"; which of P1–P4 maps to which R26 field is not stated except P2 ↔ cancel_max_ms. Not blocked: consensus; devnet (R26 waived in its scope). |
| 7. Kind | Local service policy. |

### P2 — CANCEL_MAX for precompile runs (network.md:286)

| field | content |
|---|---|
| 1. Current behaviour | Option A: the stop hook is polled every `STOP_GAS = 1_000_000` gas of an interpreter frame (`lp4-execution/src/native.rs:69`); a precompile runs to its end (BLAKE2F 29.9M rounds ≈ 0.5 s; −32020 480–569 ms late); pocold reports `cancel_max_ms` absent. |
| 2. Proposed change | B: eth_call/eth_estimateGas refuse (−32020 or halt) a single precompile call whose gas exceeds a per-precompile cap from a measured worst time per gas. No cap values or text are recorded. |
| 3. Alternatives | A (current). B per-precompile gas cap. C RPC-only re-implementations that poll. D worker thread abandoned at deadline. |
| 4a. Security | B bounds cancellation ≤ 100 ms by construction on the measured host. |
| 4b. Performance | C duplicates cryptography; D breaks the one-thread A13 inventory. |
| 4c. Compatibility | B: RPC results differ from on-chain execution for large precompile calls ("an explicit error, never a different result", P/DECISION-PACKAGE.md §5b). No consensus effect. |
| 5. Recommendation | "**B**: eth_call/eth_estimateGas refuse, with −32020, a single precompile call whose gas exceeds a measured per-precompile cap" (P/DECISION-PACKAGE.md §5b). |
| 6. Blocked / not blocked | Blocked: deciding vectors (BLAKE2F 29.9M rounds, MODEXP worst EIP-2565 within 30M, BN254 880 pairs, on the reference and Windows CI hosts); `cancel_max_ms` in R26 `rpc-absent` (AUD/NETWORK-BOOT-GATES.md §3). Not blocked: consensus. |
| 7. Kind | Local service policy. |

### P3 — effectful method completing after its COMPUTE_DEADLINE (network.md:235)

| field | content |
|---|---|
| 1. Current behaviour | Option A: `EFFECTFUL = [eth_sendRawTransaction, pocol_submitShare]` (`pocold/src/rpc.rs:837`) is exempt from the post-encoding deadline check (`:857`); the deadline is checked before the effect. No committed test pins A (mutant R3-P3-NO-EFFECTFUL-EXEMPTION survives deliberately). |
| 2. Proposed change | A, text: "For a method whose effect is applied when it runs (eth_sendRawTransaction, pocol_submitShare), the COMPUTE_DEADLINE is checked before the effect; once the effect is applied, its result is answered." |
| 3. Alternatives | A (current): result answered. B (literal): −32020 {deadline} after a pooled effect; a retry is rejected as a duplicate. |
| 4a. Security | Not stated. |
| 4b. Performance | Not stated. |
| 4c. Compatibility | A: none (pre-RpcGuard behaviour). B: removes the exemption; RPC-visible change. |
| 5. Recommendation | "**A**: the deadline is checked before the effect; once the effect is applied, its result is answered" (P/DECISION-PACKAGE.md §5b). |
| 6. Blocked / not blocked | Blocked: "a test for the chosen option belongs with the decision" (P/RPCGUARD-R2-PROPOSALS.md P3). Not blocked: running behaviour. |
| 7. Kind | Local service policy. |

### P4 — methods outside the classification table (network.md:210)

| field | content |
|---|---|
| 1. Current behaviour | Option A: nine unlisted methods served on the heavy lane (e.g. `net_listening` `pocold/src/rpc.rs:917`, `pocol_status` `:977`, `pocold_rpcCounters` `:979`, `pocol_getBlockRlp` `:992`, `pocol_getMiners` `:994`); unknown and listed-but-unimplemented methods get −32601 after admission. |
| 2. Proposed change | B: table lines — light (≤ 4 KiB): net_listening, net_peerCount, eth_syncing, eth_mining, eth_accounts; heavy: pocol_status, pocold_rpcCounters, pocol_getBlockRlp (≤ 2 × block encoding + 1 KiB), pocol_getMiners; −32601 before admission for every other method. |
| 3. Alternatives | A (current). B table lines. C pocold-local inspection class without Origin. D refuse every unlisted method (breaks extension, scripts, tests). |
| 4a. Security | C: sites lose pocol_status and pocold_* methods. |
| 4b. Performance | B: an unknown method is answered at once, never busy. |
| 4c. Compatibility | B: none for pocold's tools once lines exist; D breaks pocol-extension sw.js, scripts/devnet.py, tests/e2e_devnet.py, tests/e2e_site.py. |
| 5. Recommendation | "**B**: add table lines for the methods pocold serves; −32601 before admission for every other method" (P/DECISION-PACKAGE.md §5b). |
| 6. Blocked / not blocked | Blocked: RG13 measurement of the light lines; the tests listed in P/RPCGUARD-R2-PROPOSALS.md P4; RPCGUARD.md's two classification rows stay PARTIAL. Not blocked: current tooling. |
| 7. Kind | Local service policy. |

---

## Memory model and defaults

### The key-path assumption (A13 counts 64 branches per key path)

| field | content |
|---|---|
| 1. Current behaviour | `TRIE_PATH_NODES = 65` (`pocold/src/resources.rs:148`): a branch at each of 64 nibbles plus the leaf; since `892dca6` the batch counts 64 branches and two leaves (incl. the displaced leaf) per changed key (AUD/A13-DERIVATION.md §2e item 3). Network store-mode bound 5.198 GiB, gate 5.94 GiB (§2e "Updated R3"; §3c). |
| 2. Proposed change | The assumption: every key path has at most 21 nodes (P/DECISION-PACKAGE.md §1 "The key-path assumption"). |
| 3. Alternatives | Keep 65 nodes (conservative, current). Count 21 (assumes an adversary cannot grind ≈ 2^76–2^80 hashes per path). |
| 4a. Security | If the assumption fails, the batch, whole-state trie paths and state.redb file terms undercount; each longer key adds up to 44 nodes; the node exceeds its bound in proportion to the paths an adversary paid for. |
| 4b. Performance | With 21: about 0.7 GiB less (batch 0.92 → ≈ 0.33, allocator → ≈ 0.15) (P/OPEN-DECISIONS.md row 8; AUD/A13-DERIVATION.md §2e). P/DECISION-PACKAGE.md says "0.82 GiB" (older figure). |
| 4c. Compatibility | None on consensus, genesisHash, vectors, wire or RPC (an A13 modelling assumption). |
| 5. Recommendation | "**keep 65**: the network store bound now fits without it (5.20 GiB, gate 5.94, with the deferred checkpoint release; 5.09 before it)" (P/OPEN-DECISIONS.md row 8). |
| 6. Blocked / not blocked | Blocked: nothing named. Not blocked: the A13 store-mode check passes without it (AUD/NETWORK-BOOT-GATES.md §4). |
| 7. Kind | Local (an assumption in the A13 bound, not a rule). |

### Store mode as the network profile's default

| field | content |
|---|---|
| 1. Current behaviour | Store mode is opt-in: `Limits::state_store` defaults to false (`pocold/src/node.rs:67`, `:70`); enabled by `--state-store` (`pocold/src/main.rs:39`, "experimental"). Whole-state A13 at network: 26,819,504,117 B model, fails; store mode 5,581,321,485 B, passes (AUD/NETWORK-BOOT-GATES.md §4). |
| 2. Proposed change | Make store mode the default when profileKind = network (P/OPEN-DECISIONS.md row 9). |
| 3. Alternatives | Keep opt-in; default when profileKind = network. |
| 4a. Security | Not stated. Store mode's bound is computed, not measured RSS; rests on conditions `store-inventory`, `transient-snapshot-inventory`, `store-file-factor`, `revm-loading` (AUD/A13-DERIVATION.md §2e). |
| 4b. Performance | Store mode imports 1.8–2.4× slower on a small fsynced store, ≈ equal on larger; reorg 1.5–2.7× faster; open from checkpoint 1.8–4.0× faster (P/DECISION-PACKAGE.md §1 "Disk and time"); near-cap open 262 s, checkpoint stalls (AUD/A13-DERIVATION.md §3c). |
| 4c. Compatibility | "none on consensus; a local default" (P/OPEN-DECISIONS.md row 9). A data directory must be opened in the mode it was made in (`main.rs` usage text). |
| 5. Recommendation | "**keep opt-in** until a near-cap network-scale state has been run and measured (RSS, open, reorg, crash)" (P/OPEN-DECISIONS.md row 9). |
| 6. Blocked / not blocked | Blocked: a network boot with default flags (whole-state A13 refuses); DECISION-PACKAGE acceptance test 5 (measured RSS on a synthetic near-cap network state). Not blocked: opt-in store-mode use; devnet. |
| 7. Kind | Local default (not normative). |


### Journal mitigations (JOURNAL-GROWTH-AND-REPLAY.md §7; FINDING-STATUS.json lists them as not approved)

| field | content |
|---|---|
| 1. Current behaviour | §7's eight options were all PROPOSED in round 3. Since then: option 3's local direction exists as the approved D-6 checkpoints (open replays the records after the newest checkpoint, not from genesis); option 7 is implemented (C-1 E1 incremental root, C-1 E2 summaries). **2026-10-28, local and not one of the eight:** open streams the journal and keeps only each record's offset, so the open transient no longer grows with the journal (evidence/r4/README.md § Open memory). |
| 2. Proposed change | Unchanged for options 1 (headers-first sync, a conformance gap against approved text), 2 (dedupe markers for re-imports), 4 (reverse diffs, storage.md:70), 5 (O(window) retain), 6 (keep the fork-point state) and 8 (normative). |
| 3. Alternatives | Each option as in §7; or keep the journal as it is. |
| 4a. Security | Options 1 and 2 bound the re-journal of a losing branch, which today has no bound (an attacker's side blocks are journaled once valid). |
| 4b. Performance | Option 4 makes a deep-fork rebuild scale with depth, not height; option 5 removes the b·H² term of open. |
| 4c. Compatibility | 1–7: local storage; 8: normative. |
| 5. Recommendation | None beyond §7's descriptions. |
| 6. Blocked / not blocked | Blocked: a bound on the journal's disk size (side blocks and re-fetched blocks are journaled again; evidence/r4/README.md § Journal to H_END); open time after a deep reorg. Not blocked: the canonical chain's journal bound, which follows from the caps. |
| 7. Kind | Local storage (1–7); NORMATIVE (8). |

---

## Source inconsistencies found

1. **D-2 recommendation.** P/OPEN-DECISIONS.md row 1 (2026-10-27) recommends (0); P/DECISION-PACKAGE.md §2 "D-2" still recommends (a) and calls it local with "genesisHash none". AUD/evidence/memory-r14/d2-gate.log, AUD/NETWORK-BOOT-GATES.md and FINDING-STATUS.json show (a) needs dFbWait and D_att (CP) changes, so a new devnet genesisHash.
2. **D-2 option (a) parameters.** OPEN-DECISIONS lists dFbWait 3 → 5 and D_att 25 → 35 only; the admitted d2-gate.log line also changes the local Δ_rel 5 → 8 and Γ 5 → 8.
3. **FINDING-STATUS.json is stale for R14/R19.** Its entry says "only MODEXP with an even modulus exceeds t_exec = 1 s, at 1.2-1.6 s"; after the fast paths no measured block exceeds 1 s (0.51 s max; P/OPEN-DECISIONS.md row 2, AUD/A13-DERIVATION.md §3b-2).
4. **FINDING-STATUS.json lists an open item not in OPEN-DECISIONS.md:** "every journal mitigation (JOURNAL-GROWTH-AND-REPLAY.md §7)". It is not in the 9-row table (which claims to list every open decision) nor in this task's list.
5. **LOG-CAP-AGGREGATE wording.** The proposal and README say "chain" / "the canonical chain"; OPEN-DECISIONS row 6 and DECISION-PACKAGE §5a say "per branch". The code is per branch (parent's counter). The amendment text "canonical chain" does not say what a side-branch block uses; N8 would fix that for TX caps only.
6. **Key-path saving.** P/DECISION-PACKAGE.md says the assumption "costs 0.82 GiB"; its later paragraph and OPEN-DECISIONS row 8 say about 0.7 GiB. OPEN-DECISIONS row 8 pairs the 5.20 GiB total with "allocator 0.28 → ≈ 0.15", but after the R3 update the allocator term is 0.391 GiB (AUD/A13-DERIVATION.md §2e, §3c), so the 0.15 estimate was not recomputed for 5.20 GiB.
7. **protocolVersion citation.** P/DECISION-PACKAGE.md §3 cites network.md:6; the Status line is network.md:7 (checked), as P/CAPS-SUSTAINABILITY.md and P/D3-D5-HEND-ANALYSIS.md cite.
8. **D-3 option set.** P/CAPS-SUSTAINABILITY.md §5 has O1–O5; DECISION-PACKAGE §3 and OPEN-DECISIONS row 3 list O1–O3 only.
9. **D-5 recommendation wording.** DECISION-PACKAGE §3 table: "(b)"; D3-D5-HEND-ANALYSIS: "(b+)" = PA + PB + PC, described as a refinement; OPEN-DECISIONS row 5 says "(b)" but its text is the (b+) content.
10. **RpcGuard recommendations.** P/RPCGUARD-R2-PROPOSALS.md records no recommendation and drafts text only for P1-B, P3-A and P4-B; the recommendations (P1 C, P2 B, P3 A, P4 B) exist only in DECISION-PACKAGE §5b / OPEN-DECISIONS row 7. No text is drafted for the recommended P1 C or P2 B.
11. **Missing fact for D-3 O2 (now computed, see D-3 4b).** No source recomputed the network store-mode A13 bound (5.198 GiB) at the O2 cap values; P/CAPS-SUSTAINABILITY.md §5b covers whole-state devnet only. DECISION-PACKAGE says O2's memory side depends on C-1 S1 (now done), while CAPS-SUSTAINABILITY §5 says it "needs canonical history off-heap first".
12. **Stale code line numbers in the sources** (checked 2026-10-28): LOG_CAP constant `user_phase.rs:24–25` → `:25–26`; LogCap check `:197` → `:218`; `apply.rs:250`/`:261` TX_MAX label → `apply.rs:318`; `chain.rs:319` BODY_MAX → `:550`; `chain.rs:335`/`:351` item 16 / `within_caps` → `:568` / `:602`; `resources.rs:103–107` history term → around `:407–418`; `rpc.rs:859` simulate → `:1044`.
