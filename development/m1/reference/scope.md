# PoCol scope and scientific origin

**What the thesis states (provided excerpts only)**

Nature:
- PoCol derives from PoW. It retains the target condition and cumulative work, and changes template construction, work ownership, the round lifecycle and rewards [PDF:72 §4.1].

Template and ranges:
- A fixed block template, a nonce-free header template, and disjoint nonce ranges from a deterministic seed [PDF:84-85 §5.1, PDF:90-92 §5.6].
- The template includes the protocol version, a deterministic timestamp and a target based on the recent production rate [PDF:90 §5.5.2, PDF:99 §5.8.2].
- The template may be derived or received and then verified [PDF:99].

Validation and rewards:
- Validation includes the template, range membership, PoW and the precommitted reward [PDF:102 §5.8.5].
- The reward transaction is first in the block [PDF:104 §5.9.1].
- R_tot = subsidy+fees [PDF:103 (5.6)]. Small rounding error is acceptable, and outputs sum to R_tot [PDF:104 §5.9.2].

Limits acknowledged by the text itself:
- Gossip does not guarantee template agreement [PDF:100].
- The seed is biasable [PDF:91 §5.6.1].
- Equal rewards incentivize Sybil identities [PDF:103 §5.9].
- Activity is not proof of search [PDF:105 §5.9.3].
- Verification of non-winner contributions is not implemented [PDF:80 §4.5].
- Identity and contribution are future work [PDF:140 §8.5].

Configuration and lifecycle:
- Genesis uses a configuration file; M_0 is agreed before distribution [PDF:96 §5.7.2].
- A round closes on timeout [PDF:93 §5.6.5]; miners idle after exhausting their range [PDF:95 §5.6.6].
- Signed exit [PDF:88 §5.4.2] and inactivity removal [PDF:89 §5.4.4, PDF:111-112 §5.10.3].

**Unavailable in the provided excerpts**
- The difficulty-adjustment formula. λ_blk = H_tot·p_succ is only an analytical model [PDF:108].
- Version activation, miner-list structure, and genesis and P2P bytes.
- Rounding and burn rules, the final block and fee collection.
- RPC interfaces and limits, log retrieval and test fixtures.

Our rules for these items are our additions: D33, D59, D61, D62 and D65–D97.

**What the thesis implemented and tested**
- Three-stage BlockSim experiments [PDF:119 §6.3-6.4].
- Stages 2 and 3 assume a common template; they do not implement agreement [PDF:119].
- No distributed implementation [PDF:138 §8.1].

**Results limited to their configurations**
- Stage 1: energy remains 8.420833333 kWh under continuous operation; partitioning alone does not reduce it [PDF:128, PDF:131].
- Stage 2: calculated reduction of 55.05% with 90.39% of blocks, without jointly satisfying useful-floor and churn [PDF:134].
- Stage 3: 44.95% less energy, 67.70% of blocks, and 5.219 times longer median duration against W01 [PDF:136].
- Security, fairness and Sybil resistance are not established [PDF:4, PDF:81, PDF:140].

**Product additions (ours)**
- An EVM layer, website storage and a Chrome extension.
- RP header constraints: the RP window (D82), timestamp/target constraints with a work(ceilTarget) bound (D84), the h=0 branch and pocol_getHeaders.
- Network identity/configuration: GenesisSpec and CP, closing with a binding H_END limit (D85), and the end/full fixture (D86).
- Storage and difficulty: trie window, compressed journal, classified burns and resource-bounded ASERT (D74, D83).
- RPC resource policy (D87, D88, D90, D92, D93, D94) and branch-bound log retrieval (D89, D91).
- Extension-bridge authorization, its unified envelope, rate/pending model and local-storage quota accounting (D95, D96, D97).

RPC and bridge provisions are service policies, not consensus rules.

**Implementation alternatives**
- (A) Fork geth: LGPL/GPL licensing and ongoing maintenance.
- (B) Engine API: does not enforce a template before mining.
- (C) Rust pocold with revm.

The selected option (D1) is (C); PoCol remains the consensus protocol.

**Declared material changes to PoCol (modifications, not replacement)**
- TM1: a template signed by the attempt-0 proposer, with deterministic fallback and exclusive branching (D4, D22, D31, D35, D36, D43) [PDF:99].
- TM2: seed derived from the parent's seal (D5) [PDF:91].
- TM3: winner-signed shares, equivocation evidence, integer rounding and classified burns (D6, D29, D65–D68).
- TM4: deferred settlement, PAY and a bond state machine (D7, D51, D56, D78).
- TM5: attempts with time windows (D17) [PDF:93].
- TM6: PoW input is TemplateID‖nonce (D16).
- TM7: the proposer chooses the timestamp (D23), a declared deviation from the deterministic timestamp [PDF:90].
- TM8: genesis with a canonical schema (D28, D79) [PDF:96].
- TM9: caps, inventory, H_END-bounded closing (D85) and a trie window.
- TM10: eligibility for active miners and identity-owner-controlled reactivation (D21, D59, D62).
- TM11: identity and version are validity rules (D72).
- TM12: absolutely anchored, fixed-width ASERT (D74, D83), specifying what [PDF:90] leaves undetailed.
- TM13: a transitive lexicographic tie-break along the branch path (D133). [PDF:74 §4.2] states cumulative work only; the tie-break is unavailable in the provided excerpts.
- TM14: record the winner's identity in the child block (D141).
  - The precommitted reward [PDF:104 §5.9.1] does not include the winner.
  - Equality in [PDF:103 (5.9)] does not need the winner; winnerPart is our addition that makes deferral necessary.

**Stages (D2)**
Stage A runs on anvil; stage B in pocold. Success in A does not establish B.

**Requires visual review in the PDF**
- Equations: (5.4a-b) [PDF:92], 1/λ [PDF:73, PDF:76], H_tot and λ_eff [PDF:108-110], and (5.6)-(5.9) [PDF:103].
- Figures: 5.1–5.5 [PDF:94, 98, 107, 111, 114] and 7.1–7.4 [PDF:132-133].
- Tables: 6.1, 6.2, 7.1–7.4 and 8.1 [PDF:120, 124, 131, 134-135, 139].
