# Open decisions: table presented to the owner (2026-10-28)

**Nothing in this file is approved; the task assignment does not approve any of it.** The code currently runs each decision's present behavior. Where it implements the recommended alternative, that is **provisional**, not a decision. Technical details and file/line references are in [DECISION-TABLE-2026-10-28.md](DECISION-TABLE-2026-10-28.md).

**Previously approved (reference only, not open):**
- D-1 (a): storage conformance S1–S5.
- D-6: local checkpoints.
- D-7: pool gas limit 4×GAS_LIMIT.

**Note:** resolving every decision below still would not boot the network profile today, because:
- There is no approved network genesis;
- K_eff is unimplemented (R21);
- No code path requests network-profile admission (R22).

Classification by cause: [../BOOT-BLOCKERS-2026-10-28.md](../BOOT-BLOCKERS-2026-10-28.md).

## Summary

| ID | Decision | Type | Recorded recommendation | Work depending on it |
|---|---|---|---|---|
| LOG-ACCT-01 | How logs count against LOG_BLOCK_MAX; whether 546 is a limit | Normative | (c): 36 + 32·topics + data, at most 546 logs per block | execution.md amendment; adopting provisional code |
| D-A03-DIRTY | Meaning of dirty; a halt whose effects do not fit | Normative | (a): net changes; effects do not fit → invalid block and producer skips transaction | Amendment; accuracy of A13/RECV bounds based on ACCT_DIRTY_BLOCK_MAX |
| D-A03-CAPS | What SLOT_CAP/ACCT_CAP count and when SYS_SLOT_MAX is checked | Normative | (2): user state only; SYS_SLOT_MAX after system phases | Amendment; final genesis-check form |
| D-BODY-MAX | Encoded body or sum of elements | Normative | (a): full body RLP length on every path | Clarify consensus.md:56 |
| D-A03-SYSUSER | What the user-context SYS allowance counts | Normative | (a): program storage keys only; tips and ChunkFactory nonce do not count | Amendment; R18 user allowance |
| LOG-CAP-AGGREGATE | LOG_CAP scope | Normative | Cumulative on the chain (code: per branch) | Amendment; interpretation of A13 historical-log bound; N1 |
| D-2 | Worst case per gas (MODEXP) | Normative/profile | (0) no normative change, conditional on calibration row 2 (ii) | D-2 acceptance tests; new genesis if (a)/(a2) chosen |
| R14/R19 calibration | Blocks timed at boot | Local admission policy | (ii) calibrate slowest kinds | Remove `timing-not-a-bound`; safety of D-2 (0) |
| D-3 | Caps regime (O1/O2/O3) | Normative | O2 for network with both corrections; devnet caps unchanged | N1–N3 code, R27–R29, new genesis vectors, network genesis |
| N1 | R27 and LOG_CAP = 2^32 | Normative | Part of O2 | R27 gate; D-5 release |
| N2 | R28 and network TX_CAP = 5·10^7 | Normative (CP) | Part of O2 with both corrections | New network genesisHash; R28 test |
| N3 | R29 and network TXBYTES_CAP = 24·2^30 | Normative (CP) | Part of O2 | New genesisHash; R29 test |
| N4 | O3 linear allowance (alternative to N1–N3) | Normative | Not recommended unless 19.65 GiB inventory is a hard limit | Nothing unless selected |
| D-4 | N5–N8 interface items | Normative/interface | Approve all four | N5–N8 implementation and tests |
| N5 | Pool: −32016 {cap} and eviction | Interface | Approve | Pool eviction across reorg test |
| N6 | `pocol_getCaps` fields | Interface | Approve field list | Serving method and fields |
| N7 | eth_call/estimateGas report A03 halt | Interface | Approve | RPC test |
| N8 | Scope sentence in consensus.md:170 | Specification text | Approve | Text amendment |
| D-5 | LOG_CAP identity and constants outside CP | Normative/wire | (b) with release plan: protocolVersion and constants digest in Hello, R27 gate | Hello change; AT1–AT8; any binary booting network profile |
| P1 | RPC rate across connections | Local service policy | (c): keep-alive and per-connection bucket | Test; part of removing `rpc-absent` |
| P2 | CANCEL_MAX for precompile execution | Local service policy | (b): −32020 above per-precompile gas cap | Cap values; R26 cancel_max_ms |
| P3 | Effectful method completing after deadline | Local service policy | (a): deadline check before effect | Test pinning choice |
| P4 | Methods outside classification table | Local service policy | (b): rows for served methods, −32601 for others before admission | RPCGUARD.md classification rows |
| Key path | A13 counts 64 branches per key path | Bound assumption, not rule | Keep 65 nodes | Nothing; bound fits without reducing it |
| Default mode | Store mode default for network | Local default | Keep opt-in until measured at network scale | Network boot with defaults |
| Journal mitigations | JOURNAL-GROWTH-AND-REPLAY §7 options | Local storage (8 normative) | No recommendation beyond description | On-disk journal-size bound |

## Details per decision

Each decision has seven fields:
1. Current behavior;
2. Proposed change;
3. Alternatives;
4. Security, performance and compatibility effects;
5. Recommendation;
6. Exact dependencies;
7. Type.

### Six consensus proposals

**LOG-ACCT-01**
1. Provisionally implements (c) in `user_phase.rs`: charge 36+32·topics+data, count limit 546.
2. Amendment: charge each log 36 + 32·|topics| + |data|; block total ≤ LOG_BLOCK_MAX and at most 546 logs. A transaction exceeding either halts exceptionally and contributes nothing.
3. Alternatives: (a) 36+data, no count bound; (b) data only; (c) recommended; (d) 36+encoded log.
4. Effects:
   - Security: (a)/(b) break the receipt bound (≤ 546 logs, < 400 KiB).
   - Performance: not stated in sources.
   - Compatibility: no accepted vector or genesisHash changes.
5. Recommendation: (c).
6. Dependencies: specification amendment and adoption of provisional code. No gate or boot depends on it.
7. Type: normative.

**D-A03-DIRTY**
1. Provisionally implements (a): net changes. The producer skips a transaction whose halt effects do not fit.
2. Amendment: both limits bound the union of net-changed keys/accounts. Exceeding them halts. If halt effects do not fit, the block is invalid.
3. Alternatives:
   - (a) recommended;
   - (b) exempt effects, making the effective bound larger;
   - (a-touched) count every touched account.
4. Effects:
   - Security: low liveness risk, appearing only at GAS_LIMIT ≥ ~120M.
   - Performance: neither limit is reachable at 8M.
   - Compatibility: no vector or genesisHash change.
5. Recommendation: (a).
6. Dependencies: amendment. Choosing (b) reopens A13/RECV bounds.
7. Type: normative.

**D-A03-CAPS**
1. Provisionally implements (2): user state only; check SYS_SLOT_MAX after system phases.
2. Amendment: SLOT_CAP/ACCT_CAP bound state outside the five system programs; SYS_SLOT_MAX is checked after system phases. Genesis must satisfy both limits.
3. Alternatives: (1) all accounts/slots; (2) recommended.
4. Effects:
   - Security: (1) has medium risk, potentially stopping production near the cap.
   - Performance: not stated.
   - Compatibility: no genesisHash change.
5. Recommendation: (2).
6. Dependency: amendment.
7. Type: normative.

**D-BODY-MAX**
1. Provisionally implements (a): full encoded-body length on every path.
2. Clarification of consensus.md:56: BODY_MAX bounds the RLP encoding of [txs, evidence]; validation/decoding apply the same limit.
3. Alternatives: (a); (b) sum of elements; (c) sum with margin.
4. Effects:
   - Security: removes inconsistency where one path accepted a block and another rejected it.
   - Performance: not stated.
   - Compatibility: no valid block becomes invalid.
5. Recommendation: (a).
6. Dependency: clarification text.
7. Type: normative.

**D-A03-SYSUSER**
1. Provisionally implements (a), with no effect for v1 programs.
2. Proposal: SYS allowance counts storage keys of the five programs and the programs themselves; zero during closing. D70 balances and ChunkFactory nonce do not count.
3. Alternatives: (a); (b) count Escrow tips, halting every tipped transaction during closing.
4. Effects:
   - Security: low.
   - Performance: not stated.
   - Compatibility: no validity change.
5. Recommendation: (a).
6. Dependencies: amendment and R18 user allowance.
7. Type: normative.

**LOG-CAP-AGGREGATE**
1. Provisional behavior: LOG_CAP = 2^30, cumulative from the parent counter, hence per branch.
2. Amendment says “on the canonical chain”. Code works per branch; text does not specify the counter for a side-branch block.
3. Alternatives:
   - Chain-wide (recommended);
   - Per block (ineffective);
   - Retention-window-wide (consensus-unsafe);
   - A1–A6 family, incorporated into D-3/N1.
4. Effects:
   - Security: high in principle. Reaching the cap stops all chain logging; network profile can reach it from block 32,769.
   - Performance: A13 historical-log bound.
   - Compatibility: no validity or genesisHash change.
5. Recommendation: chain-wide, with A1a in N1.
6. Dependencies: amendment; A13-bound interpretation; N1.
7. Type: normative.

### Execution time

**D-2**
1. Current: Cancun pricing with two result-preserving MODEXP fast paths. Slowest measured block: 0.47 s validation, 0.51 s import, about 2× margin to t_exec = 1 s. The measured grid does not prove a maximum; final-tree measurements will be repeated in evidence/r4.
2. Proposal: (0) no normative change.
3. Alternatives:
   - (0) no change;
   - (a) t_exec = 2 s, dFbWait 3 → 5, D_att 25 → 35; measurement shows Δ_rel and Γ also change 5 → 8;
   - (a2) GAS_LIMIT = 6M;
   - (b) reprice MODEXP;
   - (c) precompile input cap;
   - (d) another non-normative fast path.
4. Effects:
   - Security: (0) retains risk from slower hosts and shapes outside the grid; calibration (ii) addresses it.
   - Performance: ≤ 0.51 s per block on this host.
   - Compatibility: (0) unchanged; (a)/(a2) new devnet genesisHash; (b)/(c) hard fork of every vector.
5. Recommendation: (0). Note: DECISION-PACKAGE §2 still recommends (a) and says “genesisHash unchanged”, contradicting measurement.
6. Dependencies: D-2 acceptance tests, one depending on calibration decision.
7. Type: normative/profile.

**R14/R19 calibration**
1. Current: one GAS_LIMIT compute loop, timed in memory and through a temporary store; 8–19 ms per phase. `timing-not-a-bound` is recorded.
2. Proposal: (ii) calibrate slowest kinds and gate on the maximum per phase. No written rule yet.
3. Alternatives: (i) retain loop; (ii); (iii) mix with percentile.
4. Effects:
   - Security: (i) passes R14/R19 without measuring the blocks they bound.
   - Performance: (ii) adds tens of seconds to boot.
   - Compatibility: no consensus effect.
5. Recommendation: (ii).
6. Dependencies: remove `timing-not-a-bound` within specification scope; safety argument for D-2 (0). Final-tree measurements per alternative are in evidence/r4.
7. Type: local admission policy.

### Cap sustainability

**D-3**
1. Current O1: network TX_CAP 10^7, TXBYTES_CAP 2^33, LOG_CAP 2^30, cumulative per branch. Reachable before H_END: 26,316, 43,695 and 32,768 blocks respectively, after which user activity freezes.
2. Proposed O2 for network profile:
   - Gates: R27 H_END·LOG_BLOCK_MAX ≤ LOG_CAP; R28 H_END·⌊GAS_LIMIT/21000⌋ ≤ TX_CAP; R29 H_END·TXB_BLOCK ≤ TXBYTES_CAP;
   - Values: LOG_CAP 2^32, TX_CAP 5·10^7, TXBYTES_CAP 24·2^30;
   - Two corrections: restrict R28/R29 to network and prove the 21,000-gas lemma, or TX_CAP ≥ 61,560,000.
3. Alternatives: O1–O5 (DECISION-PACKAGE lists only O1–O3).
4. Effects:
   - Security: O1 permits low-cost permanent freezing; O2 makes it provably unreachable, with 1.12–1.50% margins.
   - Performance: about 65 GiB on disk. A13 store-mode model at O2 caps, computed today: **5,616,872,049 B (5.231 GiB), requiring 5.978 GiB out of 6 GiB**. It fits with about 22 MB margin.
   - Compatibility: new network genesisHash; no network chain yet.
5. Recommendation: O2 with both corrections, devnet caps unchanged.
6. Dependencies: N1–N3 code, R27–R29 gates, new genesis vectors and network genesis creation.
7. Type: normative.

**N1** (R27 and LOG_CAP = 2^32)
- Current: 2^30, no R27.
- Alternatives: A1b–A6.
- Effects:
  - Security: removes the log cliff.
  - Performance: about 3 GiB more logical storage.
  - Compatibility: genesisHash unchanged. Nodes with differing values split silently, motivating D-5.
- Recommendation: part of O2.
- Dependencies: R27 gate, AT3/AT4 and D-5 release.
- Type: normative.

**N2** (R28 and TX_CAP = 5·10^7)
- Current: 10^7.
- Effects:
  - Security: unreachable conditional on lemma L1(b).
  - Performance: larger receipt inventory.
  - Compatibility: CP field, so genesisHash changes.
- Recommendation: part of O2 with both corrections.
- Dependency: R28 test.
- Type: normative.

**N3** (R29 and TXBYTES_CAP = 24·2^30)
- Current: 2^33.
- Effects:
  - Security: provably unreachable.
  - Performance: transaction inventory increases 8 → 24 GiB.
  - Compatibility: new genesisHash.
- Recommendation: part of O2.
- Dependency: R29 test.
- Type: normative.

**N4** (O3, linear allowance)
- Current: unimplemented.
- Effects:
  - Security: no permanent freeze, but an attacker can consume the allowance.
  - Performance: inventory unchanged.
- Recommendation: only if 19.65 GiB inventory is a hard limit.
- Dependency: none unless selected.

**D-4** (N5–N8)
- Current: pool, RPC and text are silent on caps.
- Recommendation: approve all four.
- Dependencies: RPC test per item, pool-eviction test across reorg.
- Type: interface; no validity change.

**N5**
- Current: a transaction that can never be included remains until the pool fills.
- Proposal: −32016 {cap}, and eviction on a new head.
- Security effect: ends free pool occupation.

**N6**
- Current: method listed but not served (−32601).
- Proposal: {txCount, txBytes, logBytes, txCap, txBytesCap, logCap}, with a freeze alert.

**N7**
- Current: simulation reports success for a transaction that will halt.
- Proposal: report A03 halt.

**N8**
- Current: consensus.md:170 has no scope; code is per branch.
- Proposal: explicit scope text.

**D-5**
1. Current: LOG_CAP outside CP, Hello without protocolVersion; differing LOG_CAP values split silently.
2. Proposed (b), with release plan:
   - R27 gate;
   - protocolVersion in Hello;
   - Constants digest in Hello, rejecting mismatch;
   - Ships with N1.
3. Alternatives:
   - (a) nothing;
   - (b);
   - (c) put LOG_CAP in CP (schema 2).
4. Effects:
   - Security: (b+) catches any constants mismatch during handshake.
   - Performance: not stated.
   - Compatibility: Hello wire format changes; old and new versions cannot connect.
5. Recommendation: (b), with (b+) content.
6. Dependencies: Hello change, AT1–AT8, N1 release and any binary booting network profile.
7. Type: normative/wire.

### RpcGuard (no consensus effect)

Note: RPCGUARD-R2-PROPOSALS records no recommendation. Recommendations below come from DECISION-PACKAGE §5b; the recommended P1/P2 alternatives have no written text.

**P1**
- Current: connection closes after one response, bucket is per connection, no aggregate reconnect limit.
- Proposal: (c) keep-alive with per-connection bucket.
- Alternatives: (a); (b) server-wide bucket; (d) per-IP bucket.
- Security effect: (a) has no aggregate limit.
- Dependency: test.

**P2**
- Current: precompile runs to completion, delaying cancellation 480–569 ms.
- Proposal: (b) −32020 above per-precompile gas cap, values/text not yet written.
- Dependencies: measure values; R26 cancel_max_ms.

**P3**
- Current: (a), deadline checked before effect.
- Proposal: pin (a) in text.
- Alternative: (b) −32020 after an applied effect.
- Dependency: test pinning choice.

**P4**
- Current: nine unlisted methods served on heavy lane.
- Proposal: (b) table rows for them; −32601 before admission for others.
- Alternative (d) breaks extension/tools.

### Memory model and defaults

**Key-path assumption**
- Current: 65 nodes per path.
- Alternative: 21 nodes, assuming an adversary cannot grind about 2^76–2^80 hashes.
- Effects:
  - Security: if the assumption fails, the node exceeds its bound according to the attacker's spending.
  - Performance: about 0.7 GiB less. The allocator figure 0.15 is outdated after R3.
- Recommendation: keep 65.
- Dependency: none.

**Store mode as network default**
- Current: opt-in (`--state-store`).
- Performance effects:
  - Whole-state model 26.8 GB: rejected;
  - Store model 5.198 GiB: passes arithmetically.
- Recommendation: keep opt-in until network-scale RSS, opening, reorg and crash measurements.
- Dependency: network boot with defaults.

**Journal mitigations (§7)**
- Current:
  - Option 3 achieved locally through approved D-6 checkpoints;
  - Option 7 implemented (incremental root);
  - **New in this round, local:** opening scans the journal without retaining payloads.
- Options 1, 2, 4, 5, 6 proposed; 8 normative.
- Dependency: an on-disk journal-size bound, since refetched/side blocks are written again.
