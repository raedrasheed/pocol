# PoCol design — Ready to begin a prototype

Digest: e8a19ecb306c7b955cddc75b050fdda95f9829c0e2b5cc06563cb27dba851aa0

This is a proposed specification, not a security proof, test results or production approval.

Design cycle 83: addressing I77, the fixed share threshold in TW5.

**Textual fact**
- In sv-fix, blocks 1..199 are fallback blocks with timestamp +7, so the parent header at h=9 satisfies p.ts−g_ts = 63 and T_blk·p.h = 90.
- The ASERT path for block 10 is Δt = −27, e = floor_div(−1769472, 600) = −2950, s = −1 and f = 62586.
- Therefore target = F·2^223, where F ≈ 127041. The target is about 0.969·2^240, and T_share = 32·target = F·2^228, about 0.969·2^245 < 2^245.
- The 2^245 threshold written in TW5 is looser than item 9. I accept the objection; no test was executed.

**Alternatives**
- (A) Selected (D143): derive the threshold from the actual constructed UT: T_share = min(2^256−1, UT.target·CP.m). FixtureMiner obtains it from the consensus function itself, and a predicate checks item 9 for every share against the actual threshold. Add two negative controls, one deterministic.
- (B) Change fixture timestamps so s=0 and f=0 at h=10, yielding target = 2^240. This affects the shared sv-fix rule (ts +7) and every vector derived from it (SV8, SV9 and TW1–TW4).
- (C) Retain 2^245 and search again on rejection. This conceals the fixture error rather than correcting it.

**Effect**
- TW5 settlement values stay unchanged because share counts per owner have not changed.
- No validity rule changes.

**Conclusion**
- I confirm readiness for experimentation, subject to Codex's ordinary review followed by adversarial review. Readiness is not a security proof.
- The first reviewable and testable task is M1-spec. The block-10 ASERT path and derived threshold are transferred to the M4/M4b annex.

## PoCol scope and scientific origin

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

## Adversary and trust model

**From the thesis**
- A PPT adversary with share β. It withholds, publishes conflicting histories, ignores ranges, modifies the template and mounts eclipse attacks without breaking cryptography [PDF:74-75 §4.3].
- The network is partially synchronous with bound Δ after GST [PDF:73].
- The 0.5 threshold does not transfer automatically [PDF:74].
- Sybil attacks, grinding and contribution need analysis [PDF:75].

**Our assumptions**
- A1: SHA-256, keccak256 and secp256k1 are secure.
- A2: after GST, the message and body arrive within Δ; verification, construction and Build_fb finish within t_exec.
- A3: |C_j−t| ≤ φ, Φ = 2φ, and the clock is monotonic.
- A4a: the attempt-0 proposer is honest and online with probability q > 0.
- A4b: H_hon > 0, and an honest miner completes its range within c·T_blk.
- A5: no safe threshold for H_adv is assumed.
- A6: each identity costs a locked B_reg.
- A7: the browser, extension and local node are trusted; remote RPC is untrusted. Remote RPC may send success or error responses with any of these properties (D98):
  - Any size or depth.
  - Any compression encoding.
  - Any degree of slowness.
  - A false Content-Length.
- A7b (D95, D96, D97): site JavaScript is a fully adversarial actor inside the sandbox frame:
  - Sends any bridge message with any id, kind, method, params, structure and timing, including batches, malformed names, prototype keys, legacy local-message forms and successive bursts.
  - May hold valid presigned transactions.
  - Its capabilities are restricted by the BridgeAuth matrix. RPC access is only through RpcReadClient, LogClient or Wallet after approval; storage/navigation access is only through SiteStorage and Navigator.
  - It cannot see chrome.*, keys or the worker clock used to timestamp arrival.
- A15e (D134): no bound is assumed on boot count or the number of generations that disappear before an epoch item is confirmed.
  - The fence-name bound depends on a shared name per E and the name gate, not the disappearance rate.
  - A disappearance loop shorter than T_LATE after the limit is reached disables local storage fail-closed without exceeding the limit.
- A8: historical trie ≤ HIST_TRIE_BUDGET.
- A9: system payments do not execute recipient code.
- A10 (RPC): the adversary sends any request with any parameters/timing and can:
  - Open multiple connections and send batches.
  - Read slowly or intermittently, stop at the beginning of each item, or read quickly and then stop.
  - Leave accepted socket bytes unread, adjust its receive window and disconnect at any time.
  - Access local RPC from a local process or an Origin-allowed page.
  - With enough work, cause a reorg between two requests or before commitment.

  A malicious local process can omit Origin; node origin blocking (D95) does not protect against it.
- A11: nodeKind is truthful.
- A12: cfg is local; the adversary may be an authorized proposer or a colluding winner signing any UT.
- A13: the adversary chooses ts within [L(a), U(a)] and C+Φ.
- A14: the network profile carries genesisPre in schema D79 and is accepted only if decoded without error and its hash matches the signed or confirmed genesisHash. The extension does not see alloc; the node recomputes allocRoot.
- A15 (D103): an MV3 worker may disappear at any time. chrome.storage operations it issued before disappearing may apply later at any point, or never apply.
  - Assumed: atomic set of one item and no two simultaneously live worker instances.
  - No ordering is assumed between operations from a dead generation and those from a live generation.
  - BR22c measures this.
- A15c (D104): every operation from a dead generation applies or is discarded within T_LATE = 60s of the next generation's boot; wall-clock bootMs does not jump backward by more than that. BR22c measures this assumption. Violating it invalidates only the disk bound, not D103 ordering resolution.
- A15d (D112): a set or remove whose promise rejects in the live generation does not apply later. BR22c measures this assumption.
  - It was implicit in D105 because the ticket is dropped after a post-failure measurement.
  - Violation invalidates only disk, name and site bounds, not ordering resolution or seq uniqueness. The BR22a fail variant tests this.

**Timing**
- ts = max(L(0), C_P) ≤ U(0), σ = max(Δ, Φ+1)+t_exec, and T_j = max(b_j+Δ_rel, L(0)+Δ_fb_wait).
- Arrival theorem (conditional): after GST, with an honest proposer, the template arrives before T_j under two conditions:
  - C1: Δ_rel ≥ σ+Δ+2t_exec.
  - C2: Δ_fb_wait ≥ Φ+Δ+2t_exec.

  Sticky FB covers withholding.
- Pre-mining bounds:
  - X^h = max(Δ+t_exec, Φ+1)+Δ+2t_exec.
  - X^W = max(Φ+1+Δ_fb_wait, Δ+t_exec+Δ_rel).

  Values are 22/36 in genesis and 6/10 in P11. Collusion invalidates them (T10b(iv-v)).

**Timestamp and ASERT**
- One timestamp's effect on the next target is ≤ 2^((U−L)/τ), about 1.059 in genesis.
- T10c measures multi-block grinding; resistance is not claimed.
- ASERT resources (D83): bounded-width inputs and saturation at s ≥ 256 or s ≤ −257 before multiplication/shifting. No intermediate exceeds 512 bits; an extreme header has constant cost.

**RPC resources (D87, D88, D90, D92, D93, D94)**

Previous facts:
- A valid eth_getLogs over 10000 blocks permitted about 312.5 MiB raw.
- Aggregated batches held responses without a bound.
- The global rate limit permitted starvation.
- A startup grace period per item extended sending.

After the changes:
- Sequential HTTP/1.1 gives each connection at most one response reservation (S1).
- Heavy reservations ≤ 48 MiB; light reservations ≤ 16 MiB.
- SCRATCH ≤ 16 MiB per heavy permit and ≤ 1 MiB per light permit, tracked by a thread-local counter. Buffers ≤ 32·(1 MiB+64 KiB); total 178 MiB ≤ 192 MiB.
- Snapshot and permit are released before sending.
- Sending uses a cumulative rate, one clock per response/batch and one startup grace. Credit at any instant is A−R·sendTime; it does not raise the maximum because sendTime ≤ max(GRACE, A/R)+ENFORCE_SLACK.
- Batch sending ≤ 258.2791015625s; total ≤ BATCH_WALL = 420s conservatively. Disconnection releases all resources within ≤ 100 ms. RPC has a separate runtime.
- A measures server-socket acceptance, not client reads; their difference is generally unbounded. SO_SNDBUF = 32 KiB explicitly. Verification uses PacerSim and trace predicates (D93), not an assumed difference bound.
- (D94) The server's obligation ends at sendDone (A = P = size). Accepted unread bytes thereafter occupy kernel buffers outside the response reservation and do not extend any RpcGuard resource.

Declared residual limits:
- A local adversary occupying 32 connections denies new connections; six slow readers deny heavy methods (−32021).
- An existing honest connection continues receiving light service.
- Each adversarial connection is bounded to ≤ 420s per batch.
- A reader that first reads quickly and then stops holds the reservation until its credit expires, within the same bound.
- An adversary not reading after sendDone occupies only a connection until IDLE (30s) and its connection's kernel buffers, measured within RSS.
- Consensus and memory are unaffected. RG5, RG8, RG9 and RG14 measure this.

**Log retrieval (D89, D91)**
- pocol_getLogs rejects a noncanonical anchor, preventing chunks from two branches from mixing (P2).
- Chunks delivered before detecting a change are invalidated by abort; one attempt's result is committed.
- The anchor is rechecked before commitment. A later reorg is not detected; this is disclosed.
- SCRATCH or deadline exhaustion does not imply progress: the client splits/bisects, and explicitly fails at one block.
- Consistent lying remains possible in RP (D13).

**Identity, version and network**
- D72: stateless item 1b applies before storage and work accounting. Violation rejects the block and descendants and penalizes the peer. Residual cost is wasted adversarial work and attempt a=0, handled by FB.
- D79: two conforming implementations produce identical bytes; strict decoding precedes hashing.
- D75: the template channel accepts only the signed form at a=0.
- D136: an adversary rotating peers may send unbounded distinct signature envelopes for a fixed block.
  - Memory is bounded by per-block and global caps.
  - Saturation delays acceptance of a valid gossip envelope without preventing it (D137):
    - Before issuance, each GetEnvelope request reserves a checking slot and response buffer from a pool independent of gossip; its response is always checked without exceeding the cap.
    - An adversary holding requested slots with slow responses delays recovery up to ENV_REQ_TIMEOUT per turn. Its bound is D_x in the recovery lemma (network, D139), including the successful request and EnvCheckPool wait. The bound is conditional on network G1–G4 (D140): parent state available, an honest connected holder replying within ENV_REQ_TIMEOUT, and a stable peer set for i. Time before those conditions hold is outside the bound. Each new connection adds a potential request, so there is no fixed bound under unbounded identity churn. A holder consistently slower than the timeout has no acceptance guarantee on this path; an adversary controlling the holder's link can enforce that.
    - (D138) Cancellation does not release capacity before checking settles, so an adversary cannot multiply running checks through timeouts, disconnections or concurrent imports. At most it keeps the slot for c_env after cancellation.
  - CPU cost per unsolicited copy is ≤ c_env before the cap is reached and zero afterward.
  - The total checking-rate bound is governed by the gossip limit and peer count, and measured in T23 N11.

**Sybil attacks and identity splitting**
- Draw and shares are additive; the denominator is max(C, κm), and B_reg is the deterrent (T7b).
- Only the identity owner can reactivate, with a bond, after W_act and within R_max.
- Records ≤ M_max+R_max·(Lmax+LAG) ≤ REG_RECORDS_MAX, where Lmax = max(U, 2·E_win+D+1) and LAG = 2048 (D86).
- Exclusion (D116, D120):
  - C_full and c_w include the winner's implicit nonce; trimming to S_max applies only to explicit shares.
  - In the ownSharesOnly arm with fixed c_w: direct gain is zero when C_full ≤ κm, and less than R·c_w·(1/κm−1/C_full)+1 above it.
  - In emptyShares, gain is computed exactly and may be negative.
  - Indirect effects through Draw and lastShare have no analytical bound; T7c(S) measures them.
- Burns and fees are not a profit channel.
- Closing (D56, D85): deterrence is limited to a tombstone witness, burning unsettled rewards and loss of membership; no block exceeds H_END (E1).

**Browser**
- chainId collisions (D58) do not mix storage or approvals.
- RP window (D77, D82, D84):
  - n ≤ 13 headers under items 1, 1b and 2–9, with viewFuture and viewTargetCeil.
  - At b > 1, add a reference header under items 1 and 1b and viewFuture.
- ceilTarget = min(2^256−1, target_g·2^VIEW_SLACK_S); minWorkRP = work(ceilTarget). The value is 4095 at target_g = 2^240 and 1 at saturation, effectively no bound (HC3, HC5).
- Rough estimate, not a guarantee: about 49 seconds of H_cal for 13 headers in network.
- At h=0 (D80), RP permits neither display nor signing. Malformed requests are rejected in a fixed order without reading state (D81).
- Leakage is tested using canary sinks (D37, D63).

**Bridge (D95, D96, D97)**

Previous facts:
- Before D95, rpc_read had no allowlist; a site could pass eth_sendRawTransaction with a presigned transaction, or pocol_submitShare, without approval.
- Before D96, storage_set and nav envelopes violated B0.
- Before D97, the rate vector expected a fixed count not implied by a refilling bucket and an independent pending limit.
- Before this version, quota vector BR17e did not reach the limit, so overflow atomicity was not tested (I50).

After D95–D97:
- One envelope per kind and one checking order.
- Before any connection or local effect, BridgeAuth rejects every (kind, method) outside the matrix and every params value violating its schema.
- The only network write is through wallet_req and explicit approval. External navigation requires explicit confirmation.
- Rate limiting uses a deterministic integer bucket per site session; pending requests use a precisely bounded counter per session. Neither resets on navigation (D99).
- Orphaned requests from a torn-down frame stay counted until they settle.
- Every remote response passes RecvGuard (D98):
  - Per-method byte limit checked during reading.
  - Bounded reservation pools.
  - No partial content is committed.
- Local-storage quota is accounted literally, including replacement; overflow is atomic.
- The node rejects mining methods from any request carrying Origin.

Declared residual limits:
- Permitted read methods reveal public state and balances to the site; these are already public.
- eth_call and eth_estimateGas consume RpcGuard capacity within bridge limits (≤ 4 pending and ≤ 50 messages/s with capacity 50).
- A site can fill its local storage to 1 MiB per (netKey, address), but cannot access another site's storage.
- Pending storage_set writes are bounded (D101):
  - Per session: ≤ 8 messages and ≤ 256 KiB, counting the active write.
  - Extension-wide: ≤ 64 messages and ≤ 4 MiB, with ≤ 2 concurrently active writes.
  - Rejection occurs before enqueueing.
  - Slow chrome.storage delays writes and rejects them after timeout; they do not accumulate unboundedly.
  - Declared residual: a locally hanging chrome.storage operation holds an active-write slot until settling, potentially pausing other sites' writes temporarily.
- A site may request an external https link; it opens only when the user clicks a confirmation displaying the complete URL. Misleading users through link content is outside the protection scope.
- Network isolation (D37) prevents the frame from broadcasting a presigned transaction over a channel outside the extension.
- Different sites and tabs have separate sessions, so their aggregate may exceed 50 messages/s.
  - Extension-wide limits apply first: ≤ 16 pending reads and ≤ 32 MiB reserved site-receive memory.
  - Node RpcGuard applies afterward.
- A site cannot create its own session: opening a PoCol site in a tab requires a user action.
- Remaining D98 limits:
  - Chrome, not the extension, enforces HTTP-header limits.
  - Temporary parsing memory scales with the method bound (up to 8 MiB for pocol_getLogs) and is measured in MR10.
- BR16 detects defects in BridgeAuth itself; their absence is not assumed.

**Storage flooding**
Each component has a cap or derived bound. Reaching it freezes that component without stopping blocks (T18b). The time horizon is bounded by H_END.

**Fixtures**
- ExecFixture is not an adversary.
- ChainFixture (including end/full and logs/dense) passes every rule, without a ParamGate exemption except profileKind=simnet.
- NX is an authorized adversary; MaliciousRpc a remote-RPC adversary; RpcAttacker a local-RPC adversary; BadSite a bridge adversary.

**Out of scope**
Breaking cryptography, device compromise, Chrome vulnerabilities, modification of the victim's cfg and OS-level flooding.

**Not established**
- Common prefix and rollback probabilities [PDF:76-77].
- Incentives, selfish withholding and behavior before GST.
- Completeness of leakage-channel coverage, incentives to produce F and resistance to timestamp grinding.
- Adequacy of the RP work bound and numeric RPC limits.
- Timer accuracy on other devices and the bound between socket acceptance and client reading.
- Absence of JSON-parsing vulnerabilities in BridgeAuth's implementation.

## Consensus rules

**0. Pinned configuration (D72, D77, D79)**

cfg = {chainId, genesisHash, ForkSchedule, CP, M_0}.

GenesisSpec = RLP([specVersion, chainId, CP, allocRoot, M_0List, sysCodeHash]):
- specVersion: u8 = 1; otherwise gsVersion.
- chainId: u64 ∈ [1, 2^64−1].
- allocRoot and sysCodeHash: exactly 32 bytes.
- genesisHash = keccak256(GenesisSpec), excluding ForkSchedule.

Decoding:
- Strict RLP, minimal-byte integers, zero = 0x80.
- Errors: gsInt (leading zero or integer ≤ 0x7f not encoded as one byte), gsCount, gsRange (out of range or unknown enum), and gsLen.
- Evaluation order: structure, gsVersion, gsCount, gsInt, gsLen, gsRange, gsOrder, then gsSys.

CP (version 1, 52 ordered elements):
- 1–8: g_ts u64, target_g u256 [1, 2^256−1], T_blk u32 ≥ 1, tau u32 ≥ 1, nonceMode u8 (0 bounded, 1 unbounded), c u8 ≥ 1, D_att u32, dFbWait u32.
- 9–15: m u16 ≥ 1, kappa u8 ≥ 1, S_max u16 ≤ 256, D u16, W_w u16, alpha_bp u16 ≤ 10^4, gamma_bp u16 ≤ 10^4−alpha_bp.
- 16–24: subsidy u256, B_reg u256, R_max u8, M_max u16 ≥ 1, M_min u16 ≥ 1, REG_RECORDS_MAX u32, SWEEP_MAX u16, SWEEP_SCAN u16, REG_OPS u16.
- 25–30: W_act u32, Theta_inact u32, E_win u32, U u32, E_max u8, EVID_BYTES_MAX u32.
- 31–40: GAS_LIMIT u64, baseFee0 u256, TX_MAX u32, BODY_MAX u32, B_code_max u32, CODE_CAP u64, TXBYTES_CAP u64, TX_CAP u64, SLOT_CAP u64, ACCT_CAP u64.
- 41–45: SYS_SLOT_MAX u32, SYS_KEYS_BLOCK_MAX u32, SYS_KEYS_USER_MAX u32, SYS_ACCT_BLOCK_MAX u32, G_SYS u64.
- 46–52: H_END u64, T_END u64, H_CLOSE u64, T_CLOSE u64, Z_close u32, RET u32 ≥ 1, P2 u32.

ParamGate, not decoding, checks inter-field relationships. R_max=0 is valid and disables register and reactivate.

Local parameters outside CP enter neither genesisHash nor validity: Δ, φ, Φ, t_exec, Δ_rel, Γ, A_fb, K_hist, R_keep, k_*, VIEW_SLACK_S, and RpcGuard/BridgeAuth parameters.

M_0List = [[id 20 B, rewardAddr 20 B]…]:
- M_min ≤ |M_0List| ≤ M_max, and |M_0List| ≥ 1.
- id strictly increases bytewise; duplicates give gsOrder.
- id ∉ {0x00×20, SYSTEM_ADDRESS, 0x…C0C001–0x…C0C0FF}; otherwise gsSys.

Deriving genesis state:
1. alloc.
2. System code.
3. For each MinerRegistry entry: active and HELD, bond = B_reg, lastShare = 0, graceStart = 0. ActList follows M_0List order; RecLen = ActLen = |M_0|.
4. Escrow += |M_0|·B_reg; heldTotal = |M_0|·B_reg.

allocRoot = the resulting state's MPT root; a mismatch violates R24(A).

sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80 for PAY])).

ForkSchedule = ordered [(v, H_v)], starting with (1,0). activeVersion(h) = the last v with H_v ≤ h. cfg is never read from a header, peer or RPC.

**1. Encoding**

Strict RLP: no leading zeros, shortest prefixes, exact element counts and no trailing bytes.

Structures:
- UT (18 elements, ≤ 613 B): [tag 'PoCol-tpl-v1', chainId u64, genesisHash, parentHash, h u64, a u32, protocolVersion u16, ts u64, target ∈ [1, 2^256−1], stateRoot, txRoot, receiptsRoot, logsBloom 256, gasLimit, gasUsed, baseFee, evidenceRoot, proposer 20 B or empty].
- SignedTemplate = [UT, sig 65 or empty] ≤ 682 B.
- Header = [SignedTemplate, nonce 8, shareList ≤ 256, winnerSig 65] ≤ 3067 B.
- Body = [txs, evidence] ≤ BODY_MAX.

Derivations:
- TemplateID = SHA256(RLP(UT)).
- SigMsg = keccak256(0x19‖'PoCol template'‖0x0a‖TemplateID).
- powHash = SHA256(TemplateID‖be64(nonce)).
- seal = keccak256(TemplateID‖be64(nonce)); seal_genesis = keccak256(genesisHash).
- shareRoot = keccak256(RLP(shareList)).
- blockHash = keccak256(TemplateID‖be64(nonce)‖shareRoot); block-0 hash = genesisHash.
- WinMsg = keccak256(0x19‖'PoCol winner'‖0x0a‖TemplateID‖be64(nonce)‖shareRoot).

Additional rules:
- txRoot and evidenceRoot are MPTs keyed by RLP(i).
- Signature: s ≤ n/2, v ∈ {0,1}, RFC6979.
- shareList is strictly increasing and excludes the winning nonce.
- Virtual genesis header = {h=0, ts=CP.g_ts, seal=seal_genesis, blockHash=genesisHash}.

**1b. Template independence from the mining result (D141)**
- Every UT field, including stateRoot, receiptsRoot, logsBloom and gasUsed, depends only on parent state, parent header, cfg, non-root UT fields and the body.
- apply for block h reads none of these from block h: nonce, its owner, shareList, winnerSig or sig.
- Block h's result (winner and shares) is recorded in h+1 through recordShares(h) and recordResult(h), using ParentResult(h) (D142), not local state.
  - ParentResult(p) = {blockHash(p), winner, shares}. shares is [(n, owner(n))] for each n in shareList(p), in its ascending order; winner = owner(p.nonce). The winning nonce is an implicit share counted for winner.
  - shareList is committed by shareRoot inside blockHash, so every envelope copy of p yields the same list.
  - Owners come from p's assignment: the seed from the grandparent's seal and A_p* from grandparent state. ParentResult therefore depends only on committed content.
  - Two parents with the same template/nonce but different shareRoot values have different blockHash values and independent ParentResult values. Their children's stateRoot values therefore differ if the shares differ; they are not mixed.
- Lemma: for fixed UT, every nonce satisfying items 6–9 and 13 in any A_h* owner's range yields the same roots. Thus two blocks with the same template share TemplateID and stateRoot and differ only in blockHash.
- Final block F has no child, so its result is not recorded, consistently with its lack of a direct reward.

**2. Membership**
- M_h = MinerRegistry records in parent state [PDF:87]; Act_h = ActList ≤ M_max.
- A_h = {m ∈ Act_h, tomb=0 : lastShare ≥ h−W_act or graceStart ≥ h−W_act}.
- A_h* = A_h, or Act_h excluding tombstones if A_h is empty.
- deregister and removeInactive fail if they would make |Act| < M_min.
- register and reactivate ≤ R_max per block, effective from h+1.
- Registry operations are rejected during closing, so RecLen does not increase after closeH.

**3. Timing**
- L(a) = p.ts+a·D_att+1 and U(a) = p.ts+(a+1)·D_att, computed in u128. A block is invalid if U(a) ≥ 2^63.
- Validity: L(a) ≤ ts ≤ U(a).
- Fallback timestamp: L(0)+Δ_fb_wait at a=0; L(a) at a ≥ 1.
- ts > C+Φ means a queue bounded to ≤ 256.
- The proposer builds with max(L(0), C); transition to a+1 occurs when C > U(a)+Γ.
- T_j is in milliseconds; arrival exactly at T_j locks.

**4. State machine per (parentHash, h, a)**
- (A) a=0: WAIT until T_j. A valid signed template from Draw(h,0) arriving at time ≤ T_j locks, then PROP; otherwise sticky FB. An invalid template counts as absent and the peer is penalized.
- (B) Empty Act: stop.
- (C) a ≥ 1: FB when C ≥ L(a).
- A second template from the same proposer: type-1 evidence, then FB.
- A template preceding its parent is stored after form and 1b checks (≤ 4 per parent, ≤ 64 total), and removed on parentInvalid.

**4b. Build_fb(p, a)**
- chainId and genesisHash from cfg; protocolVersion = activeVersion(h).
- proposer and sig are empty; ts follows §3.
- target = ASERT(p), gasLimit = GAS_LIMIT, baseFee = nextBaseFee(p).
- Body = 0xc2c0c0; roots come from full apply.

**5. Seed and partition**
- seed = SHA256('PoColNonce'‖seal_p‖be64(h)‖be32(a)).
- ticket = SHA256(seed‖id20), sorted ascending over A_h* [PDF:91, modified].
- start(i) = floor(i·nMax/n); end(i) = floor((i+1)·nMax/n)−1 [PDF:92].
- work(t) = floor(2^256/(t+1)).
- nMax = min(2^64, c·work(target)) in nonceMode=0; 2^64 in nonceMode=1.

**6. Draw**
- w(m) = Σcount[j][m] over j ∈ [h−1−W_w, h−2], for A_h* members.
- P+ = {w > 0}; if empty, select uniformly.
- x_k = SHA256('PoColProposer'‖seal_p‖be64(h)‖be32(0)‖be32(k)), with rejection sampling to remove bias.
- Select the first member in address order whose cumulative sum exceeds x_k mod W.

**7. Fixed-width ASERT (D74, D83)**

floor_div rounds toward −∞.
1. Δt = (p.ts−g_ts) − T_blk·p.h, in i128, with |Δt| < 2^97.
2. e = floor_div(Δt·65536, τ).
3. s = floor_div(e, 65536); f = e−65536·s ∈ [0, 65535].
4. s ≥ 256 gives 2^256−1; s ≤ −257 gives 1; computation ends.
5. F = 65536 + floor_div(195766423245049·f + 971821376·f² + 5127·f³ + 2^47, 2^48), in u128, with F ∈ [65536, 131071].
6. X = target_g·F < 2^273; k = s−16 ∈ [−272, 239]. Y = X·2^k if k ≥ 0; otherwise Y = floor(X/2^(−k)).
7. target = min(max(Y, 1), 2^256−1).

Equivalence lemma: X ∈ [2^16, 2^273), so k ≥ 240 clamps to 2^256−1, while k ≤ −273 yields 0 then 1. Hence D83 = D74. At block h=1 the target is target_g.

**8. Closing and finalization (D57, D85)**
- closing is sticky, beginning when h ≥ H_CLOSE or ts ≥ T_CLOSE. closeH is the first closing block; closeH ≤ H_CLOSE.
- The cursor processes ≤ RET records per block starting at closeH. cursorDone is true in state closeH+⌈RecLen_closeH/RET⌉−1 at the latest.
- gate2: h ≥ H_END−P2 or ts ≥ T_END−P2·T_blk.
- final = closing_p and cursorDone_p and h ≥ closeH_p+D and (h ≥ H_END or ts ≥ T_END). The final block has no valid child.
- Item 10a: h ≤ H_END.

Theorem E1 (under R13 and R20):
- At h = H_END, p.h ≥ closeH+⌈REG_RECORDS_MAX/RET⌉+P2+D.
- Thus cursorDone_p is true and H_END is final; no block lies above it, and the non-genesis block count is ≤ H_END.
- E1 is an upper bound, not an arrival guarantee.

**H-pre** (stateless, with parent header or virtual genesis header):
1. Encoding and sizes; each transaction ≤ TX_MAX.
1b. NetID in order: chainId, genesisHash, activeVersion. Errors: netChain, netGenesis, netVersion.
2. h = p.h+1, parentHash = blockHash(p), h ≤ H_END (10a).
3. Form: either (proposer≠∅ and sig≠∅ and a=0 and ecrecover = proposer), or (proposer=∅ and sig=∅).
4. Time window and fallback timestamp.
5. target = ASERT(p).
6. nonce < nMax.
7. powHash ≤ target.
8. winnerSig is recoverable.
9. Every share < nMax and distinct from nonce; SHA256(TemplateID‖be64(n)) ≤ T_share.

**H-full** (with parent state):
10. final_p = false.
11. gasLimit and baseFee.
12. Exclusive branching: signed requires proposer = Draw(h,0). Fallback requires byte-identical RLP(UT) = Build_fb(p,a).UT and Body = 0xc2c0c0.
13. nonce owner ∈ A_h*; winnerSig is from that owner.
14. Share owners ∈ A_h*.
15. Body matches roots and limits.
16. txBytes and txCount are within caps.
17. apply matches stateRoot, receiptsRoot, logsBloom and gasUsed; system assertions hold.

Evaluation order (D75):
- The node stops at the first violation (firstReject); diagnostics evaluate every evaluable item.
- Without parent header: items 2–9 are notEvaluable. Without parent state: items 10–17 are notEvaluable.
- Additional messages: unknownParent, ancestorInvalid, parentInvalid, templateForm.
- Envelope classification (D129): blockHash commits to neither sig nor winnerSig; each violation belongs to one of two classes:
  - envReject: a violation depending only on sig or winnerSig bytes. Includes:
    - Failure to decode the header, or malformed encoding of the two signature fields (item 1).
    - Agreement between sig presence and proposer, and signature recovery (item 3, excluding committed condition a=0).
    - winnerSig recovery (item 8).
    - The winnerSig signer being the nonce owner (signature portion of item 13).

    Effect: reject header copy hdrId = keccak256(RLP(Header)), not the block. Penalize the sender; do not record a block in pocol_getRejects or trigger parentInvalid/ancestorInvalid. The block remains admissible through another copy.
  - blockReject: every other violation. It depends on committed content and rejects blockHash and its descendants.
- A block is valid if its committed content is valid and a valid envelope copy exists.
- The node stores the first envelope-valid copy and serves it through gossip, synchronization, pocol_getHeaders and GetEnvelope (D131).
- envWait (D131): a block whose known copies all received envReject remains undecided at blockHash level. Alternative-envelope fetching continues until a valid copy arrives or the block is removed under sync-queue limits or D118. Removal is local retention, not invalidity. Further copies of an accepted blockHash are dropped without checking.
- This is an implementation specification; it changes neither validity outcomes nor chain selection.
- Envelope-check stages (D130):
  - envPre: stateless; decoding both signature fields and recovering sig/winnerSig (signature portions of items 1, 3 and 8).
  - envOwn: parent state, no execution; checks that the winnerSig signer owns nonce in A_h* (signature portion of item 13). Cost is assignment computation from parent state, cached per parent.
  - Before parent state is available, the copy is envPending, with neither verdict nor rejection.
  - An envOwn verdict is final for each hdrId because the parent is committed in blockHash. Retaining settled verdicts is bounded by local policy D136 (network). Eviction allows rechecking hdrId with the same deterministic result and does not change block validity.
- Execution (apply) occurs once per blockHash after its first envOk copy; its verdict is cached in BlockCache.
- envReject does not invoke RejectPath.cancel, set HfullDone or record a block in pocol_getRejects. These three are restricted to blockReject or import.

**9. Chain selection (D133, amends D15)**

From the thesis [PDF:74 §4.2]: only the cumulative-work rule. The tie-break is unavailable in the provided excerpts; the following is our addition.

Definitions:
- Candidates: headers of blocks imported (passed H-full) by the node.
- W(t) = Σwork from genesis to t.
- importSeq(b): a local monotonic counter assigned to each blockHash at its first successful import. It remains unchanged while the block is stored. A block removed by D118 and imported again receives a new number.
- key(b) = (signed(b) ? 0 : 1, importSeq(b), blockHash(b)); lexicographically smaller is better.

The ordering t1 ≻ t2 holds in either case:
- W(t1) > W(t2).
- Or W is equal and key(c1) < key(c2), where c1 and c2 are children of the lowest common ancestor on the paths to t1 and t2.

Total-order lemma:
- work(t) ≥ 1 for every target, so a candidate's path cannot prefix another's with equal work. Thus distinct children c1 ≠ c2 always exist when W is equal.
- Comparison is lexicographic over paths from genesis; the key totally orders siblings, so ≻ is a transitive total order.
- Head = maximum under ≻, independent of examination order. No sequential pairwise comparison.

Transition lemma:
- W and key are fixed for each stored block.
- The set grows only through import; D118 does not remove canonical blocks.
- Hence head changes only on import, at most once per import.

Effect:
- Signed precedes fallback at the first divergence (D15).
- Same-type sibling ties use local import precedence and cannot be ground.
- The rule changes no block's validity.

**10. Recovery**
- Displaced transactions return to the pool after revalidation.
- Rollback uses inverse diffs for all state, including pending[j] (D78).
- Trie window: K_eff ≤ K_hist; requests outside it return −32017.
- Roll back up to R_keep, then TrieRebuild.
- Side blocks ≤ 2 per height (D118):
  - Retain the canonical block if present and one noncanonical block ranked by its branch's highest cumulative work, then first seen.
  - Compare another arrival with the stored noncanonical block, retain the higher-ranked one and delete the loser with its stored descendants.
  - A branch that later becomes heavier is fetched through synchronization via Status.
  - This is a local-retention rule, not validity.

**11. Progress (conditional)**
An honest miner starts within X^W; in bounded mode p0 ≥ 1−exp(−c·ρ_hon).

**12. Finality**
Probabilistic: safe = head−6, finalized = head−60 [PDF:76-77], without a mathematical guarantee.

**13. Consensus independence from RPC and the extension (D87–D97)**
- Validation/mining run in a separate prioritized runtime with one redb writer.
- RPC reads use snapshots released at computation completion.
- The following enter no validity item:
  - RpcGuard, SendPacer and PacerTrace.
  - The pocol_getLogs anchor and sendDone/receiveDone events.
  - BridgeAuth, bridge-message envelope, rate bucket, pending counter, SiteStorage quota and node origin blocking.
- pocol_getLogs reads the canonical chain without changing it.

## Contribution and rewards

**From the thesis**
- R_tot = subsidy+fees [PDF:103 (5.6)]; Σf = 1 [(5.7)].
- Equality incentivizes Sybil identities [PDF:103].
- Small rounding error is acceptable [PDF:104].
- Activity is not proof of search [PDF:105].
- Inactivity removal has no bond penalty [PDF:89, PDF:112].

**Share (D6)**
- A shareList item satisfies items 9 and 14, where T_share = min(2^256−1, target·m).
- The winning nonce is an implicit share.
- (D142) c_i, C_j and lastShare are derived in recordShares(j) from ParentResult(j) of block j on the executed branch. Settlement therefore depends on block j's own committed shareList, not the first copy seen by the miner or node.
- Shares sample work; they do not prove exhaustive search. κm = 64.
- pocol_submitShare is a local-only mining interface. A site cannot reach it through the bridge (D95), nor can any request carrying Origin.

**Evidence**

Type 2 (D29): RLP([2, TemplateID, nonce, shareRoot1, sig1, shareRoot2, sig2]).
- Valid if the roots differ, owner(nonce) signed both, an ancestor block exists at j ≥ h−E_win, and the accused has a record with tomb=0.
- Before settle(j), its effect is to invalidate and burn pot_j.

Type 1 is valid if:
- UT1 ≠ UT2, both signed at a=0 with the same (parentHash, h) and cfg identity.
- h ≥ h_incl−E_win.
- The signer is DrawIndex[h mod E_win].
- The accused has a record with tomb=0.

Evidence violating its conditions invalidates the containing block.

**Registry**
Rec[id] has four words:
- w0: status, tomb, bondState, recPos, actPos, lastShare.
- w1: rewardAddr.
- w2: bond.
- w3: exitHeight, dueHeight, burnHeight, graceStart.

M_0 members are written in genesis (D79).

**Authorization (D62)**
- id = msg.sender in all functions except permissionless removeInactive.
- calldata length is exactly 4 or 36 bytes; no fallback or receive.

**Bond state machine**
- register with value B_reg: active and HELD.
- deregister and removeInactive: active → exited, with dueHeight = h+U.
- reactivate: from exited, HELD and tomb=0, after W_act.
- claimBond or sweeping: after dueHeight and outside closing, through PAY, then delete the record.
- Closing cursor: HELD → PAID, at ≤ RET per block.
- Evidence:
  - Against an active record: BURNED and tomb=1; remove from the active set.
  - Against PAID: write tomb=1 only.
  - Against a tombstone or identity without a record: invalidate the block.
- Delete a tombstone after burnHeight+E_win+D+1.

Error order: BadCalldata, BadValue, Closing, RegOpsLimit. At R_max=0, register returns RegOpsLimit outside closing and Closing during it.

**Sweeping and registry bound (D61, D86)**
- ≤ 64 steps and ≤ 16 deletions per block.
- LAG = 2048: protocol constant and maximum sweeping delay.
- Lmax = max(U, 2·E_win+D+1): maximum inactive-record lifetime before deletion eligibility.
- R16: REG_RECORDS_MAX ≥ M_max+R_max·(Lmax+LAG).
- network: Lmax = 4113; bound 256+2·6161 = 12578 ≤ 16384.

**Fees (D68)**
- blockTips = Σ gasUsed·effectiveTip for phase-1 transactions.
- effectiveTip = min(maxPriority, maxFee−baseFee) for type 1559; gasPrice−baseFee otherwise.
- Base fee is burned outside Escrow and does not enter burnedTotal.

**Commitment and settlement (D78, COINBASE = Escrow)**
- pot_j = subsidy+blockTips_j, written in phase 2 of block j as pending[j] = {amount, kind ∈ {signed, fallback}, proposer}. All three values are known from UT and the body before mining.
- winner starts at zero; recordResult(j) writes it in phase 0 of block j+1 as the owner of block j's nonce (D141).
- Recording lemma: R8 requires D ≥ 2, so j+1 ≤ j+D−1 and every settle(j) reads a recorded winner.
  - Reading winner = 0 in settle is a system assertion failure invalidating the block; it cannot occur on a valid chain.
  - Fallback effect: propPart goes to the recorded winner.
- Block F: no pending[F] is created. recordResult(F−1) precedes finalize in F, so pending[F−1] is settled with its winner.
- Type-2 evidence for block j does not need the recorded winner because it invalidates and burns pot_j.
- settle(j) in block j+D reads and deletes pending[j]; a reorg restores it.

settle formulas:
- invalid: invalidBurn = pot.
- normal:
  - winnerPart = floor(pot·α_bp/10^4).
  - propPart = floor(pot·γ_bp/10^4), to the signed proposer or fallback winner.
  - R = pot−winnerPart−propPart; Q = max(C_j, κm).
  - p_i = floor(R·c_i/Q); A = floor(R·C_j/Q).
  - A tombstone's share goes to penaltyBurn; an empty account's share to payBurn.
  - baseBurn = R−A; roundBurn = A−Σp_i ≤ k_j−1.
  - Identity: pot = paidTotal+payBurn+penaltyBurn+roundBurn+baseBurn.
- F: subsidy_F = 0, finalBurn = blockTips_F; block F settles everything remaining.

Aggregation:
- burnedTotal is one slot; burned value remains as Escrow balance.
- Surplus = balance−(Σpending+heldTotal+burnedTotal) ≥ 0.

Issuance bound (D85): block count ≤ H_END; F has no subsidy, so subsidies ≤ subsidy·(H_END−1).

**ExecFixture (κm=64, α_bp=1000, γ_bp=500)**
- SV1: R=64000 and {w:1} → A=1000, baseBurn=63000.
- SV2: R=101 and {w:1, 64 identities with 1 share each} → A=101, Σp=65, roundBurn=36.
- SV3: R=1000 and {w:3, x:2} → p=46 and 31, roundBurn=1, baseBurn=922.
- SV4: R=6400 and t a tombstone → paidTotal=100, penaltyBurn=100, baseBurn=6200.
- SV5: as SV4, but t has no account → payBurn=100.
- SV6: invalid pot → invalidBurn=5000.
- SV7: pot=80000, fallback, C=1 → 8000 and 4000, R=68000, A=1062, baseBurn=66938, paidTotal=13062.
- SV7s: pot=164000, signed, C=1 → 16400 and 8200, R=139400, A=2178, baseBurn=137222, paidTotal=26778.
- SV10a (D116): R=64000, c_w=1, C_full=128 (w and 127 identities with 1 share each), C_kept=1. Full: p_w=500, A=64000, roundBurn=0, baseBurn=0. With exclusion: p_w=1000, A=1000, baseBurn=63000. g_dir=500 < 501.
- SV10b: C_full=64 → p_w=1000 in both cases, g_dir=0.
- SV10c: C_full=65 → 984 versus 1000, g_dir=16 < 16.39.
- In SV10a–c, w is represented only by ν (c_w=1); other identities' shares are explicit.
- SV10d (D120): R=64000; E_full = two explicit w shares and 127 others, so C_full=130, c_w^full=3, p_w=1476.
  - ownSharesOnly: C_kept=3, so p_w=3000 and g_dir=1524 < 1524.08.
  - emptyShares: C_kept=1, so p_w=1000 and g_dir=−476.
- SV10e (S_max boundary): R=64000; 300 explicit non-w shares arrive, trimmed to the 256 smallest nonces. Thus C_full=257, c_w=1, p_i=249 per share, Σp=63993 and roundBurn=7.
  - ownSharesOnly: p_w=1000, g_dir=751 < 751.98.
- SV10f: a 257-item shareList invalidates the block under item 1. Explicitly adding ν to the list is rejected under item 9.

**ChainFixture**
- SV8: Δburned(F)=1155008, w +208992.
- SV8-fb: 1071008.
- SV9: pot_30=253015.
- SV9-final: 1244023.
- SV8-reorg: 1208230 since 198.
- SV9-reorg: pending[30]=80000.

**Exclusion effect (T7c, D116)**

Notation:
Share sets (D120), for each height j won by w:
- ν: winning nonce. Always an implicit share outside shareList, counted in C_j and its owner's share count. Thus C_j = 1+|shareList_j|; c_i counts ν for its owner (consistent with SV1).
- E_full: valid explicit shares for height j (items 9 and 14, all ≠ ν) received by the winner before publishing its header.
  - Trim to S_max using the smallest nonces; this is honest construction policy.
  - ν does not enter the trimming or occupy a slot.
- E_kept: actual shareList content; length ≤ S_max under item 1.
- C_full = 1+|E_full| ≤ S_max+1 = 257; C_kept = 1+|E_kept|.
- c_w^full = 1 + w's explicit shares in E_full; c_w^kept = 1 + w's explicit shares in E_kept.
- Hence c_w ≥ 1 always; an empty list gives C_kept = c_w^kept = 1.

Effect:
- General direct gain: g_dir = floor(R·c_w^kept/max(C_kept, κm)) − floor(R·c_w^full/max(C_full, κm)).
  - winnerPart and propPart do not depend on shares.
  - g_dir may be negative.
- ownSharesOnly arm: E_kept = w's explicit shares in E_full. Thus c_w^kept = c_w^full = c_w, and C_kept = c_w ≤ C_full. Then:
  - g_dir = 0 if C_full ≤ κm because both denominators are κm.
  - Otherwise 0 ≤ g_dir < R·c_w·(1/max(C_kept, κm) − 1/C_full)+1 ≤ R·c_w·(1/κm − 1/C_full)+1.
  - The bound requires fixed c_w; it does not apply to a list adding w shares absent from E_full.
- emptyShares arm: E_kept = ∅, so c_w^kept = 1 and Q = κm.
  - g_dir = floor(R/κm) − floor(R·c_w^full/max(C_full, κm)), computed exactly without another bound.
  - It equals ownSharesOnly only when c_w^full = 1.
- Indirect effect: excluded shares enter neither w(m) in Draw nor lastShare for excluded miners, potentially increasing the adversary's later proposer share and propPart. There is no analytical bound; relative g in T7c(S) measures it, and the 2% bound is a falsifiable hypothesis.

**T17 per-block invariants**
- Conservation, roundBurn ≤ k−1, pot−subsidy = tips, and balanced Escrow.
- PAID at most once.
- pending[j] exists for every nonfinal j ∈ [h−D+1, h].
- (D141) pending[j].winner ≠ 0 for every j ∈ [h−D+1, h−1]; pending[h].winner = 0 at the end of block h.
- heldTotal(0) = |M_0|·B_reg; h ≤ H_END.

PAY: 5000 gas, cannot fail and executes no code.

RPC service has no reward; its cost is borne by the operator.

**Disclosed**
- No direct reward for F's winner.
- Selfish withholding is unresolved (T10).
- reward_mode=equal is for comparison only [PDF:103 (5.9)].

## Contracts and EVM compatibility

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

## Web-file storage

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

## Browser and isolation

**Address**
- omnibox: 'pocol <profileName>/<0xaddr>[/path][@v<n>]'.
- web3:// is only a convenient input form. ERC-4804 and ERC-6860 are not supported in v1, and their texts have not been read.
- We do not claim native Chrome support for any scheme.

**Network profile (D58, D77, D79)**
{profileId, name, chainId, genesisPre, genesisHash, forkSchedule, endpoint, trustLevel ∈ {DEV, LN, RP}}.

Admission:
1. Decode genesisPre without any gs* error.
2. keccak256(genesisPre) = genesisHash.
3. chainId matches.
4. Signed networks.json, or a confirmation displaying the complete hash.
5. RecvFit (D100): calculate WorstLegit for each CP-dependent exact method from the profile's own values: B_code_max, BODY_MAX, TX_MAX and genesisPre length. Any RECV_LIMIT overrun rejects the profile with −32019 {rule: recvFit}, rather than admitting a network whose honest responses would later be rejected.

netKey = keccak256(RLP(['PoCol-net-v1', chainId, genesisHash])).
- Check on opening, every 30s and on signing. Any mismatch tears down the frame and cancels requests with 4901.
- Duplicate chainId with different genesis: read-only with a “conflicting” label, and 4100 for signing.

**RP header window (D77, D80–D84)**

Counts (D82):
- W = [b, h], where b = max(1, h−12), and n = h−b+1 ≤ 13.
- Reference b−1 when b > 1; it is not counted.
- Retrieved: from = max(1, b−1), count = h−from+1 ≤ 14; this is light (count ≤ 64).

Client constants (D84):
- VIEW_SLACK_S = 4.
- ceilTarget = min(2^256−1, target_g·2^4).
- minWorkRP = floor(2^256/(ceilTarget+1)).
- C is the browser clock, and Φ_view = 10s.

Steps:
0. h = eth_blockNumber.
   - At h=0: no pocol_getHeaders or frame; viewNoBlocks with message “The network has not produced block 1 yet; no header is available for RP verification”.
   - eth_requestAccounts and eth_sendTransaction return 4901; recheck every 30s.
1. Calculate b, from and count without violating D81.
   - −32018 for a valid request gives viewIncomplete.
   - −32021 retries after retryAfterMs up to 3 times, then gives viewIncomplete.
2. The response contains count headers at consecutive heights from..h; too few or too many gives viewIncomplete.
3. Reference (b > 1): item 1, then 1b, then viewFuture (ref.ts ≤ C+Φ_view). No work or window check.
4. For each x ∈ W in ascending order:
   - Item 1, then 1b, then viewFuture.
   - Item 2: the parent is the reference, or the virtual genesis header when b=1; parentHash(1) must equal cfg.genesisHash, otherwise viewGenesis.
   - Items 3 and 4.
   - Item 5 (fixed-width ASERT).
   - viewTargetCeil: target_x ≤ ceilTarget, before PoW.
   - Items 6–9.
5. Message: “RP: work and identity bound for n headers [back to genesis], with computed work of at least <minWorkRP in decimal> per header (work(ceilTarget)), without execution or membership verification”.
   - At minWorkRP = 1 append “The work bound is ineffective for this network”.
   - “back to genesis” only when b=1.
   - No percentage or time estimate in the message.
6. Any violation: label, reject display, return 4901 and rely on no root.
7. Items 10–17 are not evaluable; consistent lying is possible.

Client cost: ≤ 14 header decodes, ≤ 13 ASERT operations with operands ≤ 512 bits, and ≤ 13 SHA-256 operations.

Effect of D84: an honest network more than 4τ behind shows viewTargetCeil: 4 hours in network and 40 minutes in P11. The remedy is LN.

RW examples:
- h=3 → b=1, from=1, count=3, n=3, back to genesis.
- h=13 → b=1, from=1, count=13, n=13.
- h=14 → b=2, from=1, count=14, n=13.
- h=20 → b=8, from=7, count=14, n=13.

HC examples:
- target_g=2^240 → ceilTarget=2^244 → minWorkRP=4095.
- target_g=2^256−1 → minWorkRP=1.

LN at h=0: display and signing are allowed because the node verified GenesisSpec and allocRoot.

**LogClient (D89, D91)**

The page's only eth_getLogs path:
- BridgeAuth checks the filter schema (D95).
- The bridge resolves tags to numbers.
- Reject a range > 1024 before sending with −32020 {reason: range}.
- Pass to LogClient.fetchAll; never send raw eth_getLogs.

Limit reason classes:
- P1 = {resultBytes, resultCount, scanBytes}, provided a ≤ lc ≤ b−1.
- RES = {deadline, scratch}, provided a−1 ≤ lc ≤ b−1.
- Any violation or other reason: serverViolation.

withinLimit marker:
- Mark [a, lc] produced by P1; every subrange inherits the marker.
- P1 on a marked range gives serverViolation (lemma P3); RES on it is allowed.

LogClient.step(range{a, b, withinLimit, retries}, reply):
- Complete(logs): every blockNumber ∈ [a, b], blockHash is constant per height, and (blockNumber, logIndex) is strictly ascending after the last append. Then Append; a violation gives serverViolation.
- Limit{r ∈ P1, lc}: Replace([a, lc]+withinLimit then [lc+1, b] with the original marker). Do not skip ahead before completing [a, lc].
- Limit{r ∈ RES, lc}:
  - If lc ≥ a: Replace([a, lc] then [lc+1, b]) with the original marker.
  - Otherwise, if b > a: Replace([a, mid] then [mid+1, b]), where mid = a+floor((b−a)/2).
  - Otherwise (a=b): Retry(500 ms) with retries+1; at retries = 3 return Error{r, block: a}.
- −32021: Retry(retryAfterMs), not counted in retries but counted in totalRequests.
- −32022 (anchorNotCanonical or beyondAnchor): Restart.
- −32601: Error{unsupported}.
- Any other error: explicit Error.

Request accounting:
- logRequests: pocol_getLogs requests in the attempt, including retries.
- headRequests: eth_getBlockByNumber requests in the attempt (anchor and pre-commit check).
- totalRequests: every RPC request across all attempts in the run so far.
- maxRequests bounds only totalRequests and is checked before sending. A request that would exceed maxRequests is not sent; emit abort(id, tooManyRequests), then Error{tooManyRequests}.

Termination:
- Replace replaces one range with two strictly shorter ranges.
- Retry ≤ 3 per block; Restart ≤ maxRestarts.
- maxRequests is an explicit ceiling.
- Attempt steps ≤ 5·(b−a+1) plus −32021 retries; all are bounded by maxRequests.

Correctness lemma: the ranges of one completed attempt form an ordered disjoint partition of [from, to], and every piece is complete on the anchor branch (P2). Concatenation equals the range result for that branch.

sink contract = {begin(attemptId, anchor), piece(attemptId, seq, range, logs), abort(attemptId, reason), commit(attemptId, summary)}:
- summary = {pieces, logs, logRequests, headRequests, totalRequests, anchor}; totalRequests includes aborted attempts.
- attemptId starts at 1 and increases by one.
- anchor = {hash, number} from eth_getBlockByNumber('latest') at begin. If to > anchor.number: abort(id, beyondHead), then Error.
- seq starts at 0 per attempt; pieces follow range order and may be empty.
- Before commit: eth_getBlockByNumber(anchor.number).hash = anchor.hash; a mismatch is treated as Restart.
- Restart: abort(id, reorg), then begin(id+1, new anchor). maxRestarts = 3, hence ≤ 4 attempts, then abort(id, reorgUnstable) and Error.

Invariants (checked by SinkChecker):
- (S-a) Per attempt: one begin, then ≥ 0 piece events, then exactly one terminal event (abort or commit).
- (S-b) No call after the terminal event; no attempt before the previous one ends.
- (S-c) At most one commit, only for the final attempt.
- (S-d) The committed attempt's ranges cover [from, to] exactly.
- (S-e) pieces and logs match what was delivered; logRequests and headRequests match the attempt's MockLogServer trace; totalRequests matches all run requests.

Consumer semantics: piece data is provisional until commit. On abort, discard all data for attemptId. The result is correct for the declared anchor branch; a reorg after the pre-commit check is not detected.

The two collectors:
- fetchAll (bridge): internal sink cleared on abort.
  - maxBytes = 4 MiB per attempt, timeout 30s, maxRequests = 4096.
  - Exceeding limits gives tooLarge, timeout or tooManyRequests without a partial result.
  - The page receives data only after commit.
- fetchEach (test tool, Node.js): external sink, timeout 600s, maxRequests = 65536; unavailable to pages.

In RP: −32601 gives unsupported; consistent lying is possible (D13).

**Bridge and its authorization (D95, D96, D97)**

Unified envelope (D96):
- A message is JSON text = {id, kind, payload}, with exactly these three fields.
- id is an integer in [0, 2^32−1] generated by the loader; it is only the response key.
- kind ∈ {rpc_read, wallet_req, storage_set, nav}.
- payload = {method, params}, with exactly these two fields; params is always an array for every kind, even when empty. No alternative form for any kind.
- Response: {id, result} or {id, error: {code, message, data?}}. id = null when no valid id can be extracted.
- No partial result with success status.

General rule:
- The site is an adversary (A7b). Every message passes through BridgeAuth.check in the extension worker before any network request or local effect; deny by default anything absent from the matrix.
- The allowlist is a frozen Map built at module load, not a plain JS object; __proto__, constructor and hasOwnProperty do not match.
- Method names match exact bytes: case-sensitive, no whitespace or control characters, ≤ 64 ASCII bytes.
- RpcGuard's light/heavy table is not a permission table.

Arrival stamp (D97):
- The worker stamps every message on port receipt with arrivalMs = floor(performance.now()) in the worker, before any check. The site cannot control it.
- Process messages one by one in arrival order.

Check order (stop at the first violation; identical for every kind):
- Bpre (without parsing):
  - Size > BRIDGE_MSG_MAX = 64 KiB: −32600 {reason: size}, id = null, without consuming a token.
  - Rate bucket per site session (D97, D99), with integer state {tokens_mt, last_ms}:
    - On session creation (not frame creation): tokens_mt = BUCKET_CAP_MT = 50000, last_ms = creation time. Navigation does not refill the bucket.
    - For each message passing the size check: tokens_mt = min(50000, tokens_mt + 50·(arrivalMs − last_ms)), then last_ms = arrivalMs.
    - If tokens_mt ≥ 1000: subtract 1000 and continue. Otherwise: −32005 {reason: rate}, id = null, without deduction.
    - A message passing the rate check consumes its token even if later rejected in B0–B3.
- B0 structure: parseStrict rejects invalid JSON, duplicate keys at any depth and depth > 8. Then: outer fields exactly {id, kind, payload}, valid in-range integer id, string kind, payload object with exactly {method, params}, string method and array params. Any violation: −32600; return id if valid, otherwise null.
- B1: kind ∈ the set, otherwise −32600 {reason: kind}.
- B2: (kind, method) appears in the matrix, otherwise 4200 {method}, with method truncated to its first 64 UTF-16 units.
- B3: params exactly matches the method schema in element count, types, permitted fields and limits, otherwise −32602 {path}. path is 'params', 'params[i]' or 'params[i].field'.
- B4 routing, then the state constraint enforced by the destination component (D97), not BridgeAuth:
  - rpc_read routes to RpcReadClient, except eth_getLogs to LogClient.fetchAll. Both pass through the session's pending counter:
    - If pendingReads = 4 or the global count reaches 16: −32005 {reason: pending}, without a request.
    - Then RecvGuard.reserve (D98). If reservation is unavailable within 2s: −32005 {reason: busy}.
  - wallet_req routes to Wallet with its own constraints (one pending request and ≤ 5 per minute).
  - storage_set routes to StoreQueue (D101). A full queue gives −32005 {reason: store} without appending. Once admitted, SiteStorage executes messages in order; exceeding quota gives 4300 {reason: quota, limit: 1048576} under the accounting below.
  - nav routes to Navigator; a second pending confirmation gives −32005 {reason: pending}.

pendingReads counter (D97):
- Per site session (D99), initially 0 when the session is created.
- Increment by one when RpcReadClient or LogClient admits the message, before the first request.
- Decrement by one when the final response is sent to the frame: success, remote error, or transport error/timeout (10s for RpcReadClient, 30s for fetchAll), returned as −32603 {reason: transport}.
- Tearing down the frame does not reset it. Requests from a torn-down frame become orphans:
  - Cancel through AbortController without a response.
  - Decrement for each request only when it settles: the transport promise rejects and the receive buffer and reservation are released.
  - Maximum settlement time is the transport timeout itself.
- Reset only when the session ends, after every request has settled.
- Internal LogClient retries do not increment it: one eth_getLogs message = one pending request.

Primitive types:
- addr = '0x'+40 hex.
- hash32 and slot32 = '0x'+64 hex.
- qty = hex without leading zeros within u64.
- tag ∈ {'latest', 'safe', 'finalized'} or qty. Reject pending, earliest and EIP-1898 objects.
- data = '0x'+even-length hex ≤ 64 KiB.
- str(n) = a string without lone surrogates, with UTF-8 encoded length between 1 and n bytes.
- pathStr = printable ASCII (0x21–0x7e), length 1..2048, starting with '/'.
- httpsUrl = printable ASCII of length ≤ 2048, beginning literally with 'https://', parsed by URL with protocol = 'https:', nonempty hostname, and empty username and password.

rpc_read matrix (method: schema):
- eth_chainId, net_version, web3_clientVersion, eth_blockNumber, eth_gasPrice, eth_maxPriorityFeePerGas: [].
- eth_getBalance, eth_getCode, eth_getTransactionCount: [addr, tag].
- eth_getStorageAt: [addr, slot32, tag].
- eth_getBlockByNumber: [tag, false]. eth_getBlockByHash: [hash32, false]. Full blocks (true) are rejected.
- eth_getTransactionByHash, eth_getTransactionReceipt: [hash32].
- eth_call: [callObj, tag]. eth_estimateGas: [callObj] or [callObj, tag]. A third parameter (state override) is rejected.
  - callObj fields may only be from {from, to, gas, gasPrice, maxFeePerGas, maxPriorityFeePerGas, value, data, input}.
  - to is mandatory: no deployment simulation.
  - gas ≤ 30000000; data or input, not both.
  - Both simulate on a snapshot without committing state or broadcasting.
- eth_feeHistory: [qty ≤ 128, tag, percentiles], where percentiles contains ≤ 16 ascending numbers in [0, 100]. This product limit is narrower than the node's (≤ 1024) to keep responses within the derived RECV_LIMIT (D100).
- eth_getLogs: [filter], in either form:
  - {fromBlock, toBlock, address?, topics?}, where address is addr or ≤ 16 addresses, and topics has ≤ 4 positions, each null, hash32 or ≤ 16 hashes.
  - {blockHash, address?, topics?}.

wallet_req matrix:
- eth_accounts, eth_requestAccounts, eth_chainId: [].
- eth_sendTransaction: [txObj], where txObj fields are from {from, to, gas, value, data, maxFeePerGas, maxPriorityFeePerGas, nonce}. from is the account granted to (netKey, siteAddress), otherwise 4100.

storage_set matrix (D96, local, no network request):
- site_storageSet: [str(256), str(61440) or null]. null means deletion.
- site_storageClear: [].
- Successful result is null.

nav matrix (D96, local, no network request):
- site_navigate: [pathStr]. The viewer applies storage normalization, tears down the frame and builds a new one at the path under D26. A missing or normalization-rejected path shows the viewer's 404 page. No response to the torn-down frame. The new frame joins the same site session with its bucket and two counters (D99).

Session navigation bucket, charged before teardown:
- NAV_CAP_MT = 6000, NAV_REFILL_MT_PER_MS = 1, NAV_COST_MT = 2000. This allows three consecutive navigations, then one every 2s.
- Insufficient balance gives −32005 {reason: 'nav'} without teardown.
- site_openExternal: [httpsUrl]. Show confirmation with the full URL. Clicking “Open” opens a new tab without opener and returns null; rejecting or closing gives 4001. A second pending confirmation from the same frame: −32005 {reason: pending}.

The site_ prefix is reserved for local methods and absent from rpc_read; no site_ method reaches RpcReadClient or a node.

Prohibitions:
- Deny by default; the following examples are tested explicitly.
- Prohibited in every kind: eth_sendRawTransaction, eth_sign, personal_*, eth_signTypedData*, wallet_*, eth_subscribe, eth_newFilter and related methods, debug_*, txpool_*, admin_*, anvil_*, evm_*, eth_getProof, and all pocol_* (including pocol_submitShare and pocol_getTemplate).
- eth_sendTransaction outside wallet_req is prohibited; read methods inside wallet_req, site_* methods outside their class, and RPC methods inside storage_set or nav are prohibited.

The only network-write path:
1. wallet_req/eth_sendTransaction.
2. Approval bound to the network and bytes (D14, D26).
3. Wallet signs the displayed bytes.
4. WalletSubmit sends eth_sendRawTransaction with only those bytes, after verifying keccak256(raw) = approved txHash.

Only WalletSubmit owns the raw-send transport.

Code isolation:
- RpcReadClient is the only network transport available to Bridge. It rechecks the name against the same read matrix as a second defense and rejects anything else with 4200 without a request.
- Internal methods pass through HeaderNetCheck, LogClient and NetworkProfiles, not Bridge, and consume neither the frame bucket nor its pending count. These include pocol_getHeaders, pocol_getParams, pocol_getLogs, eth_getBlockByNumber for the anchor, and eth_getCode for chunk fetching.
- CI dependency-cruiser rule: Bridge must not import WalletSubmit or raw HttpTransport.

BridgeTrace (D97, test build and explicit activation only):
- One row per message: {frame, seq, id|null, arrivalMs, tokensBefore_mt, bpre ∈ {pass, size, rate}, stage ∈ {B0, B1, B2, B3, B4, ok}, code|null, route|null, pendingBefore|null, storeTotalBefore|null, storeTotalAfter|null, forwardedSeq|null, replyMs|null}.
- storeTotalBefore and storeTotalAfter are only for storage_set messages, using the accounting below.
- Does not change decisions; fixed 1 MiB buffer; overflow makes the run inconclusive.

Errors:
- Return fixed forms to the frame: 4200 {method}, −32600 {reason?}, −32602 {path}, −32005 {reason}, 4300 {reason, limit}, −32603 {reason ∈ {transport, recvLimit, recvDepth, recvParse, storeTimeout}}, 4001, 4100, 4901. −32005 reasons are {rate, pending, nav, busy, store}.
- 4300 and −32005 are local bridge codes. −32005 comes from EIP-1474; 4300 is explicitly nonstandard.
- Remote error text is forwarded only for an allowed method that reached the server.

In RP: the same matrix, with local checks before sending remotely.

**Remote response reception: RecvGuard (D98)**

Every extension network request goes through HttpTransport.request(method, body, {recvLimit, deadline, pool}), including RpcReadClient, LogClient, HeaderNetCheck, NetworkProfiles, ChunkFetcher and WalletSubmit. Never call response.text() or response.json().

Read steps:
1. Reservation: reserve recvLimit from the pool before sending, using a FIFO queue. Failure within 2s gives busy.
2. If Content-Length > recvLimit, cancel before reading any byte.
3. Read response.body with a stream reader:
   - Count bytes after Chrome decompression, so the same count stops a decompression bomb.
   - Check each chunk before appending. Discard a chunk that would exceed recvLimit, and cancel through AbortController: recvLimit.
4. At end:
   - TextDecoder in fatal mode, otherwise recvParse.
   - Then parseStrict with depth ≤ 16 and duplicate-key rejection, otherwise recvDepth or recvParse.
   - The parser is iterative, not recursive, with linear cost.
5. An error response (non-200 HTTP status or error object) has the same limits:
   - Truncate message to 256 UTF-16 units.
   - Forward data only if its encoding is ≤ 4 KiB; otherwise remove it.
6. Any cancellation, timeout or error discards the entire buffer:
   - No partial content reaches a frame, sink or viewer.
   - Release the reservation on settlement.

RECV_LIMIT per method (D100). Each method has one of two classes:
- exact: RECV_LIMIT ≥ WorstLegit(method), the worst legitimate HTTP body from pocold, computed using CP and the parameters the client actually sends (bridge schema or internal call). This promises full LN compatibility, checked by BG1 for defined profiles and by RecvFit for each profile.
- capped: deliberately narrower product limit than the node, in this literal closed list:
  - eth_call capped at 1 MiB; the node permits RESP_MAX = 8 MiB.
  - eth_estimateGas capped at 64 KiB. Success ≤ 4 KiB; a revert error with larger data gives recvLimit.
  - A legitimate response above the limit gives −32603 {recvLimit} and is recorded in T12.
  - Adding a capped method requires a row here and review.

WorstLegit in bytes. Assumptions: compact JSON without spaces, 69 B per textual u256 quantity including its comma, 32 B per decimal percentage.
- 4096 [exact]: eth_chainId, net_version, web3_clientVersion, eth_blockNumber, eth_gasPrice, eth_maxPriorityFeePerGas, eth_getBalance, eth_getStorageAt, eth_getTransactionCount, eth_sendRawTransaction. Result ≤ 70 B; envelope ≤ 1024.
- 69632 (68 KiB) [exact]: eth_getCode. Worst case 2·B_code_max+1024 = 66560.
- 98304 (96 KiB) [exact]:
  - pocol_getHeaders with count ≤ 14: worst case 14·(2·3067+16)+1024 = 87124.
  - pocol_getParams: worst case 2·|genesisPre|+4096, requiring |genesisPre| ≤ 47104, checked by RecvFit.
- 167936 (164 KiB) [exact]: eth_getBlockBy* with hashes. Worst case 2048+floor(BODY_MAX/85)·70 = 163958 in network. The previous 160 KiB (163840) limit was 118 B too small.
- 180224 (176 KiB) [exact]: eth_feeHistory under bridge schema n ≤ 128, p ≤ 16. Worst case 325+2·(n+1)·69+2·n·32+n·(69p+3) = 168015, conservatively including both blob fields.
- 266240 (260 KiB) [exact]: eth_getTransactionByHash. Worst case 2·TX_MAX+2048 = 264192.
- 348160 (340 KiB) [exact]: eth_getTransactionReceipt. Worst case 2·32768+546·512+2048 = 347136.
- 1048576 [capped]: eth_call.
- 65536 [capped]: eth_estimateGas.
- 8 MiB [exact in LN]: pocol_getLogs; worst case RESP_MAX. Above it is fetchAll's aggregate 4 MiB limit. In RP, a remote server not implementing D88 may exceed it, producing explicit Error{recvLimit}.
- Unlisted methods are not sent.

RP applies the same bounds; the exact promise applies only to pocold in LN. A noncompact server (using spaces) may produce recvLimit; this is disclosed.

Pools (worker-global bounds):
- SITE_POOL = 32 MiB for all site requests.
- SESSION_RECV_MAX = 10 MiB per session.
- TRUST_POOL = 2 MiB, separate for HeaderNetCheck and NetworkProfiles, so a site cannot starve it.
- CONTENT_POOL = 8·68 KiB for chunk fetching.
- RECV_INFLIGHT_MAX = 48 requests across site and content pools. The trust pool is outside this count and bounded by bytes, so request count cannot starve it. Bridge traffic does not exceed 16+8; this bound is a second defense tested directly by MR10c.

Effect:
- Buffered reception ≤ sum of pools (approximately 34.6 MiB), plus one stream chunk per request.
- Temporary parsing memory is measured in MR10.
- A slow request holds its reservation until timeout (10s or 30s).

**Site session: SiteSession (D99)**

Creation and termination:
- Key (tabId, netKey, siteAddress).
- Created only by user action through the omnibox or extension UI.
- Ends on tab close or user navigation to another address or network.

Session-owned resources:
- Message bucket, pendingReads and navigation bucket.
- SESSION_RECV_MAX reservation.
- ChunkCache and its StoreQueue quota (message and byte counters, D101).

Frames and loading:
- Every frame created by site_navigate or reload joins the same session; no quota is renewed.
- One LoadJob per session. Navigation cancels the previous LoadJob; orphaned chunk requests stay in CONTENT_POOL until settlement.
- Revoke the previous load's blob URLs on teardown.
- Ignore torn-down-frame port messages after closing; they do not consume tokens.

Chunk memory:
- ChunkCache keyed by chunk address and length. The address derives from keccak256(data), making it a content key.
- Limit 4 MiB+64 KiB per session and 16 MiB globally, with LRU across inactive sessions.
- Navigation within a cached version does not refetch chunks.

Cumulative per-session limits regardless of repeated navigation:
- ≤ 4 live or orphaned reads.
- Messages by second t ≤ 50+50·t.
- Navigations by second t ≤ 3+t/2.
- Fetch each chunk once while it remains cached.

Message handling does not wait for the network; large parses are bounded by RECV_LIMIT, keeping the worker responsive.

**Signing eligibility**
- Every profile edit is under ProfileMutex and increments eligibilityEpoch.
- signEligible: the profile matches its frozen copy, no conflict, netKey not revoked, connection active, wallet unlocked, epoch matches, and (LN or DEV or RP with h ≥ 1 and a successful window).
- Inside the lock: eligibility, identity, second check, sign the displayed bytes, send through WalletSubmit, then release.
- Per-netKey key = HKDF-SHA256(seed, salt=netKey, info='PoCol-wallet-v1').

**manifest.json (MV3)**
- sandbox.pages = [sandbox.html].
- CSP: 'sandbox allow-scripts allow-forms; default-src 'none'; script-src 'sha256-<H_loader>' blob:; style-src blob: 'unsafe-inline'; img-src blob: data:; font-src blob:; media-src blob:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none''.
- Permissions: storage, unlimitedStorage (D103), omnibox, declarativeNetRequest, host 127.0.0.1 and optional_host for remote endpoints.

**Loading (D26)**
1. viewer freezes context and checks identity and the DNR guard.
2. The worker fetches and verifies.
3. iframe with sandbox='allow-scripts allow-forms'.
4. Deliver MessageChannel once; bind the frame to SiteSession (D99) without creating a new bucket or counter.
5. The loader applies RtcLockdown, freezes, sanitizes and executes blob scripts in order.
6. A second load event tears down the frame.

**Sanitization**
- Remove: on*, javascript:, iframe/frame/object/embed/base/portal, meta http-equiv, scripts inside svg, prefetch/preconnect/dns-prefetch/prerender.
- Rewrite: src, href, srcset, url(), @import.
- Intercept: relative fetch and XHR.

Unsupported: eval, relative import, Workers, cookies, IndexedDB and WebRTC.

**Network isolation**
- Layer 1: CSP.
- Layer 2: per-tab session DNR:
  - Block {id 2·tabId, priority 1}.
  - Allow {id 2·tabId+1, priority 2, urlFilter '|chrome-extension://<ID>/'}.
  - Installation fails closed.
- Layer 3: RtcLockdown; its argument is unproven, and X8 is a gate.
- Links (D96): the loader intercepts link clicks. A relative or absolute in-site link sends nav/site_navigate with the path; an external https link sends nav/site_openExternal. Links with other schemes are not followed.

**Site-local storage (D34, D96)**
- Logical storage key: 'site:'+netKey+':'+address; its state is a dictionary {key → value} stored in immutable versioned records (D103).
- Literal quota accounting (SITE_STORE_MAX = 1048576 bytes):
  - entry(k, v) = utf8len(k) + utf8len(v), UTF-8 encoding of the strings as present in params after JSON decoding, excluding quotes, JSON structure and the 'site:' prefix.
  - total = Σ entry over all pairs stored for the site.
  - site_storageSet(k, v) for string v: newTotal = total − (entry(k, old) if k exists) + entry(k, v). Accept if newTotal ≤ 1048576; equality is allowed. Otherwise 4300 {reason: 'quota', limit: 1048576}.
  - site_storageSet(k, null): delete; newTotal = total − entry(k, old), or total if k is absent. Never rejected for quota.
  - site_storageClear: newTotal = 0; never rejected for quota.
  - (D104) Two additional constraints:
    - An insertion making pair count > SITE_ENTRIES_MAX = 4096 is rejected with 4300 {reason: 'entries', limit: 4096}.
    - Every write, including deletion and clear, requires disk admission and may be rejected with 4300 {reason: 'disk'|'sites'} without any effect.
- Atomicity: SiteStorage computes newTotal, then writes the complete state using one chrome.storage.local.set operation for a new record (D103). At 4300 no write is called, so the snapshot remains byte-identical. A write failure returns −32603 {reason: transport} and reloads the snapshot from storage.
- SiteStorage serializes messages per storage key (netKey, address) through StoreQueue, preventing overlapping total calculations.

**StoreQueue (D101)**

Queued message size:
- qbytes(msg) = 64 + utf8len(k) + utf8len(v), counting null v as 0.
- site_storageClear counts as 64.
- Maximum message = 64+256+61440 = 61760.

Admission at B4, before appending and without any write. Admit a message only if all four conditions hold:
- sessQ.n+1 ≤ STORE_SESSION_MSGS_MAX = 8.
- sessQ.bytes+qbytes ≤ STORE_SESSION_BYTES_MAX = 262144.
- globQ.n+1 ≤ STORE_GLOBAL_MSGS_MAX = 64.
- globQ.bytes+qbytes ≤ STORE_GLOBAL_BYTES_MAX = 4194304.

Equality is allowed. Any violation immediately gives −32005 {reason: store}. Counters include every admitted message not yet in ownState = released (see ownership lifecycle below), including active writes answered with storeTimeout but not yet settled.

Arithmetic fact (I55): with defaults, the global byte limit is unreachable because 64 maximum messages = 64·61760 = 3952640 < 4194304. The global count limit always triggers first; the global byte limit is an additional defense against later constant changes. BR21e tests its condition separately using test parameters.

Ordering:
- FIFO per storage key in admission order (arrivalMs then seq), even across multiple tabs for the same site.
- set and clear execute in the same order.
- At most one active write per key.
- Extension-wide ≤ STORE_ACTIVE_GLOBAL = 2 active writes. Other keys wait in a global FIFO.

Execution:
- Head evaluation (D102, I56): when a message becomes its key's first unreleased message and the key has no active write, compute newTotal immediately from confirmed total. Do not wait for a global slot or call Chrome.
  - Overrun: reply 4300 {reason: quota, limit: 1048576}, then release in the same step (queued → released). Zero set calls; no slot or lock held. Immediately evaluate the key's next head.
  - Admission (always including deletion and clear): ownState = ready. Wait for a global slot in global FIFO admission order (arrivalMs then seq).
- Take the global slot only when starting the write (ready → active); a quota-rejected message never holds a slot.
- The evaluation remains valid until start because only that key's writes change its total, and they are behind the message in FIFO.
- STORE_QUEUE_WAIT applies to both queued and ready.
- One chrome.storage.local.set writes the complete dictionary. Single-item atomicity is assumed and measured by BR21c.
- Success updates dictionary and total, then replies null.

Timeouts:
- STORE_QUEUE_WAIT = 10s from admission until write start. Expiry gives −32005 {store}; remove the message without writing.
- STORE_WRITE_DEADLINE = 5s for an active write. Expiry gives −32603 {reason: storeTimeout}, meaning the outcome is unknown.
  - The key stays blocked and the active-write slot held until the Chrome promise settles.
  - On settlement, reload the dictionary from storage and recompute total.
  - Do not start the key's next write before this, preventing command reordering.

Ownership lifecycle (D102, I54):
- Every admitted message has two separate states:
  - Reply state: replyState ∈ {none, sent, suppressed}.
  - Ownership state: ownState ∈ {queued, ready, active, released}.
    - queued: not yet evaluated.
    - ready: passed quota, awaiting a slot.
- Send at most one final response; it does not change ownership.
- Owned resources:
  - Per message: qbytes and one message in session/global counters.
  - Active messages also own the global active-write slot and storage-key lock.
- release(msg) is the sole place that subtracts counters and frees slot/lock. Execute exactly once per admitted message. A second call for the same message is an implementation error detected by BR21.
- Transitions:
  - queued → ready: successful head evaluation.
  - ready → active: take a global slot and start set, without release.
  - queued → released in three cases:
    - Quota-rejected head evaluation: reply 4300, no set or slot.
    - STORE_QUEUE_WAIT expiry at acceptMs+10000: reply −32005 {store}.
    - Cancellation on teardown or session end: suppressed, no response.
  - ready → released: STORE_QUEUE_WAIT expiry or cancellation, with the same responses.
  - active → released: only when the chrome.storage.local.set promise settles, successfully or unsuccessfully; nothing else.
- Settlement before deadline: update dictionary and total (or reload on failure), reply (null or −32603 {transport}), then release in the same step.
- STORE_WRITE_DEADLINE expiry: reply −32603 {storeTimeout}; replyState = sent. Ownership stays active, with counters, slot and lock.
- Settlement after storeTimeout or frame teardown: reload dictionary, recompute total, then release without any response.
- Frame teardown after storeTimeout does not change counters: the message has been answered but remains active.
- Invariant: at every moment session/global counters equal the count and qbytes sum of unreleased messages.
  - New messages can therefore still be admitted while an active write is pending, but only within remaining limits.
  - Messages for the blocked key wait until settlement or STORE_QUEUE_WAIT.

Teardown and navigation (D99):
- Cancel unstarted messages from the torn-down frame without writing or replying; free their counters immediately.
- An active write completes because Chrome writes cannot be cancelled; count it until settlement and discard its response.
- Navigator waits for the key's active write to settle until STORE_WRITE_DEADLINE before taking the new frame's snapshot. After timeout, load the frame with confirmed state and an “Uncertain save state” badge.
- Session termination applies the same rule.

Memory:
- Queues ≤ 4 MiB.
- Active writes ≤ 2·(1048576+61760) in values plus serialized copy, estimated ≤ 5 MiB, measured in BR21c.
- Loaded key dictionaries: one per key with a live session; evict others by LRU above STORE_DICT_CACHE_MAX = 8 MiB.
- The loader builds localStorage from a local snapshot on load; reads are synchronous from the snapshot.
- The loader enforces quota locally with the same accounting: setItem causing total > 1048576 throws QuotaExceededError, keeping its snapshot ≤ 1 MiB.
- Coalesce writes every 250ms into a dirty-key set retaining the last value per key; clear() empties the set and marks clear. Local queue memory is therefore ≤ the local snapshot.
- The loader sends storage_set only if three conditions hold:
  - Sent but unanswered messages < 8.
  - Sent but unanswered bytes + qbytes ≤ 256 KiB.
  - Local estimate indicates a bucket token.
- A rate or store response makes the item dirty again unless a newer value was written.
- On intercepted internal links, the loader sends dirty keys, waits up to 2s for responses, then sends site_navigate. A site directly calling site_navigate accepts losing pending writes (D101).
- A loader write receiving 4300: restore the last confirmed value in the local snapshot and show a “Not saved” badge.
- No sharing between addresses or netKeys; a site cannot access another's key because SiteStorage derives the key from frame context, not the message.

**Worker-generation recovery (D103, I57)**

D101 and D102 apply within one worker generation. The following governs transitions between generations.

Alternatives:
- (A) Persistent owner: an offscreen document or port keeping the worker alive. MV3 does not guarantee their survival, leaving the same question on their disappearance.
- (B) Only reread the dictionary at boot: a late write from a dead generation overwrites a newer write (I57 scenario).
- (C) IndexedDB transactions: transaction outcome on context loss is unverified and introduces a second store.
- (D) Selected: immutable records versioned by (epoch, seq), with a generation fence and per-key checkpoint.

**1. Fence**
- Name (D134): fence item for E is only 'pocol:epoch:'+hex16(E), without nonce; value {bootMs: Date.now()} ≤ EPOCH_BYTES_MAX. Every generation calculating the same E writes the same name.
- At boot, list 'pocol:epoch:*' with getKeys. EN is the count, M the maximum, E = M+1. In order: name gate, generation window (section 6), then write. No set before both gates pass.
- Name gate (D134): EN + 1 ≤ EPOCH_NAMES_MAX = 8. On violation:
  - Issue nothing; increment epochGateBlocks.
  - Wait T_LATE from this generation's boot using its monotonic clock, sweep as below, then repeat enumeration and gate once.
  - If still violated: storage_set returns −32603 {storeRecovery, reason: 'epochNames'} throughout the generation; no snapshot; escalate action 25.
- Write 'pocol:epoch:'+hex16(E) with {bootMs} and wait for success. STORE_EPOCH_RETRY retries use the same name, adding no names.
- No data write or session before confirmation.
- Retry failure up to STORE_EPOCH_RETRY = 3 times. Then storage_set returns −32603 {reason: storeRecovery}; no snapshot.
- Sweep (D134): after T_LATE from the live generation's boot using its monotonic clock, delete every epoch item whose name is smaller than the largest visible at sweep time, regardless of stored bootMs.
  - Never delete the largest; the upper bound is nondecreasing.
  - Sweep in both confirmed and gate-blocked generations.
  - A failed remove leaves the item counted in EN.
- Writers lemma: let M_L be the largest visible item when live generation L boots. Every generation writing a name X ≤ M_L booted before L, because any later generation sees M_L and chooses a larger number. Thus each such writer booted ≤ bootMs(L), and all its operations settle before bootMs(L)+T_LATE (A15c). A swept name cannot return.
- Name-bound lemma:
  - Every name ≤ M has appeared before because each writer of X saw X−1.
  - By the writers lemma, a deleted name cannot return.
  - Thus names present or able to appear at any instant are limited to visible names plus {M+1}.
  - The gate at last issuance preserves EN+1 ≤ 8; names increase only through a boot passing the gate.
  - Therefore epoch items ≤ 8 at every instant, regardless of generations disappearing before confirmation.
- Fence lemma:
  - Every generation writing data first confirmed its fence; any later generation sees it and chooses a strictly larger E.
  - A generation that did not confirm its fence wrote no data.
  - Among generations sharing E (D134), only one confirms: the second booted after the first disappeared; if the first had confirmed, the second would see E and choose E+1.

**2. Record**
- Every set writes a new immutable item: 'site:'+netKey+':'+addr+':r:'+hex16(E)+':'+hex8(seq).
- Value {fmt: 2, epoch, seq, tomb, b64}. b64 is base64 of pairs serialized as [be32(len k)‖k‖be32(len v)‖v] in bytewise key order (D104).
- (D104) One SeqAlloc allocates seq per (key, generation):
  - Starts at 0; alloc() returns the current value and increments before issuing any set, whether a checkpoint, checkpoint retry or ordinary write.
  - No other allocator; seq is not derived from the checkpoint number.
- Range: seq ∈ [0, 2^32−1], epoch ∈ [1, 2^64−1]. Exhausted seq makes the key's writes in this generation return −32603 {storeRecovery} without set. Exhausted epoch disables local storage.
- Never reuse seq even if its promise fails or times out.
- Confirmed state = dict in the greatest present valid (epoch, seq) record.
- After a record succeeds, delete lower records best-effort. Failed deletions remain charged to disk (D104).

**3. Per-key recovery: RecoveryGate (D104)**
Each key has one state per generation ∈ {unrecovered, recovering, ready, failed} and one shared recovery operation. Any snapshot or storage_set from any tab waits for that same operation, never starting another.

On first access (unrecovered → recovering), start the read phase (D110, I63):
- t_s is transition time. Create a new gateId for the key and start total RECOVERY_READ_DEADLINE = 5s, including read-slot wait, getKeys, every get, record decoding and validation.
- Every getKeys or get takes a ReadSlots slot (RECOVERY_READ_SLOTS = 2 extension-wide) through global ReadFIFO. Release only when that read promise itself settles. No read without a slot.
- List key record names by prefix filtering getKeys; get only the greatest. If corrupt, descend to the next, up to RECOVERY_GETS_MAX = 4 get reads.
- Select the greatest valid record. Skip corrupt records and count them in recoveryCorrupt.
- No record for the key means dictionary {}.
- Corrupt records with no valid record within four reads make the key failed; messages return −32603 {storeRecovery, reason: corrupt}. No issuance or seq allocation. Records remain and can be administratively deleted.
- Deadline expiry before reads complete returns the key to unrecovered with readTimeout:
  - Pending messages return −32603 {storeRecovery, reason: readTimeout} and are released.
  - No issuance, seq allocation or disk reservation.
  - Next access starts a new gate with a new gateId.
- Accept a read result only if gateId matches the current gate and it is in the read phase. Otherwise ignore the result, release only its slot and increment lateReadsDropped. Thus no checkpoint is issued after resolution.
- Reads do not change disk; pending reads do not enter LATE_RESERVE or LATE_NAMES. Memory is bounded: ≤ 2 pending reads, each ≤ RECORD_BYTES_MAX or a name list ≤ STORE_RECORDS_MAX+LATE_NAMES under A15c.
- Request a RecoverySlots slot (D109), not STORE_ACTIVE_GLOBAL, with wait ≤ RECOVERY_SLOT_WAIT = 5s from the request.
- On obtaining the slot, in the same synchronous step: DiskLedger.reserve, then s_c = SeqAlloc.alloc(), then issue checkpoint (E, s_c) with the same dictionary. STORE_RECOVERY_DEADLINE = 5s starts at issuance.
- Disk rejection: no issuance; release the slot in the same step and count the attempt failed without a pending operation.
- First-attempt slot unavailable within 5s: no issuance or seq allocation. Return key to unrecovered; pending messages return −32603 {storeRecovery, reason: busy} and are released. Next access starts a new gate.
- On success: ready, confirmed = (E, s_c). Subsequent writes use the same SeqAlloc.
- Then delete lower records.

Before checkpoint success:
- Key messages remain queued, counted and subject to STORE_QUEUE_WAIT.
- No snapshot.

On checkpoint failure or timeout:
- One second attempt with fresh alloc() and the same dictionary. Request it when the first settles unsuccessfully or times out; request a fresh RecoverySlots slot, wait ≤ 5s. The first attempt remains pending, owning its slot and counted on disk until settlement.
- Second-attempt slot unavailable within 5s is treated as its timeout: key failed under D107, no issuance or seq allocation.
- If successful first-attempt settlement makes the key ready while waiting for the second slot, withdraw its request from RecoveryFIFO without issuance.
- Resolution bound (D110): read phase completes or expires by ≤ t_s+5s, so first slot request t0 ≤ t_s+5s. Slot by ≤ t0+5s, timeout by ≤ t0+10s, second slot by ≤ t0+15s, second timeout by ≤ t0+20s.
- The gate therefore resolves within ≤ 25s of t_s into one of four states: ready, failed, unrecovered due to busy, or unrecovered due to readTimeout.
- Messages awaiting recovery remain subject to STORE_QUEUE_WAIT = 10s and may receive store before resolution.

RecoverySlots (D109, I62):
- RECOVERY_SLOTS = 2 extension-wide, separate from STORE_ACTIVE_GLOBAL and AdminSlots. Slot state ∈ {free, held(op)}.
- Acquire through global RecoveryFIFO in request order, in the same synchronous step as DiskLedger.reserve and issuance.
- Release only on checkpoint-promise settlement, success or failure, or disk rejection before issuance. Neither timeout nor resolution releases it. Generation loss drops all state.
- RecoverySlots lemma: every issued checkpoint owns a slot from issuance until settlement and cannot issue without one. Thus unsettled checkpoints per generation ≤ 2 at every instant, including generation loss, regardless of keys or attempts.
- Resolution lemma: read phase ≤ 5s under a total deadline; each checkpoint-slot wait ≤ 5s; each checkpoint timeout ≤ 5s; attempts ≤ 2. Resolution is therefore ≤ 25s from t_s without assuming any read/write promise settles. Post-resolution read results are ignored through gateId, changing neither resolution nor issuing an operation.
- The two events (D107):
  - apply: the record appears in storage and becomes visible to listKeys.
  - settle: the set promise settles in the live generation.
  - Resolution depends only on settle; apply without settle does not change state.
- The first successful settle of any attempt before resolution immediately makes the key ready.
- confirmed = greatest successfully settled version. Both checkpoints carry the same dictionary, so later success of the other attempt does not change it.
- ready never returns to an earlier state.
- Resolve failed once every attempt has settled unsuccessfully or timed out. It remains terminal for that generation; a late successful settle cannot change it and increments lateRecordsSeen.
- Failure or timeout of both attempts makes the key failed: its messages return −32603 {storeRecovery} and are released; no snapshot in this generation.
- A late attempt, if applied, has a lower version than every post-ready write and carries the same recovered dictionary.

Resolution lemma:
- Every previous-generation write has version < (E, 0).
- If applied before recovery read, it becomes the checkpoint basis. Otherwise it remains below every subsequent record and never becomes visible.
- Thus its outcome resolves once without reversing or superseding a subsequent write.
- A write answered null committed before the response; no record is deleted before a greater record commits, so it is not lost.

Allocation lemma (D104):
- Within a generation, every set receives a fresh number before issuance; record names cannot repeat.
- Every late operation has a lower version than any operation allocated after it.
- The single gate prevents two parallel checkpoints.

Survival lemma:
- Deletion, whether sweeping or recovery, removes record r only after observing a present h > r for the same key.
- The greatest record is never removed.
- Inductively, the greatest applied record remains present even if deletion applies late.

**4. Memory**
- Worker loss drops queues, counters, locks, sessions and reservations.
- The new generation starts with zero counters and inherits no message IDs.
- No response reaches a message from a previous generation.

**5. Frames**
- viewer holds a runtime port to the worker; the worker announces genId in hello.
- On onDisconnect or a different genId:
  - viewer immediately tears down every site frame.
  - Show “The extension worker disconnected; recent writes may not have been saved” with a “Reload” button.
- No automatic reload. The new session requires user action (D99), preventing a site from renewing its bucket by crashing the worker.
- Dirty keys in the torn-down frame's snapshot are lost with notice and are not sent from an old snapshot.
- Reload takes a snapshot after key recovery.

**6. Disk**
Binding limits (D104):
- Record size: with SITE_ENTRIES_MAX = 4096, enc ≤ RECORD_BYTES_MAX = 4·ceil((1048576+8·4096)/3)+256 = 1442048.
- Measurement:
  - inUse = getBytesInUse(null), measured after boot sweep and after every set/remove settlement.
  - names = count returned by getKeys.
- Admission rule for every set (data, checkpoint, deletion or clear):
  - inUse + unres + enc(w) + LATE_RESERVE + META_RESERVE ≤ STORE_DISK_HARD = 256 MiB.
  - names + unresN + 1 + LATE_NAMES + EPOCH_NAMES_MAX ≤ STORE_RECORDS_MAX = 1024; reserve 24+8 = 32 (D134). Visible items count both in names and in reserve, conservatively.
  - unres and unresN are sums of live DiskLedger reservations (D105), in bytes and names, not separate estimates.
  - LATE_RESERVE = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS)·RECORD_BYTES_MAX = 4·4·1442048 = 23072768; LATE_NAMES = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS+ADMIN_SLOTS) = 24.
    - Six per generation: two global slots, two checkpoint slots (D109), two tombstone slots (D108).
    - Epoch items are separate, with total EPOCH_NAMES_MAX = 8 reserved (D134).
  - META_RESERVE = 1 MiB for epoch items and tombstones. Only tombstones are exempt from the byte condition and charged to META_RESERVE. Constraint (D108): META_RESERVE ≥ STORE_RECORDS_MAX·TOMB_BYTES_MAX + (GEN_WINDOW_MAX+1)·ADMIN_SLOTS·TOMB_BYTES_MAX + EPOCH_NAMES_MAX·EPOCH_BYTES_MAX = 524288+5120+2048 = 531456 ≤ 1048576 (D134). Components:
    - Retained tombstones ≤ STORE_RECORDS_MAX because tombstones are not exempt from the name condition.
    - Unsettled tombstones ≤ ADMIN_SLOTS for the live generation and each dead generation.
    - EPOCH_BYTES_MAX = 256; epoch items ≤ EPOCH_NAMES_MAX = 8 at every instant (D134).
- Violation: 4300 {reason: 'disk', limit} before any set; never delete a record to make room.
- DiskLedger (D105): admission and reservation are one synchronous worker step, with no await between check and addition. JavaScript is single-threaded, so admissions cannot overlap.
  - reserve(op): check conditions against current R_bytes, R_names and R_sites; in the same step add enc(op), one name and a site slot if new; return an operation-owned DiskTicket.
  - Includes deletion, clear, checkpoint and tombstone (its bytes are in META_RESERVE). remove has a zero ticket.
  - Ticket lifetime:
    - On settle, become settled with settleSeq; remain counted.
    - Drop only on completion of a refresh (getBytesInUse and getKeys) requested after settleSeq, adopting its measurement in that same step. There is no gap between reservation removal and measured effect.
  - Serialize refresh: one at a time, scheduled after every settle.
  - Site slot: after successful writing, it appears in measured sitesLive. On failure, release with that same measurement.
  - A ticket whose promise never settles remains until generation loss, then LATE_RESERVE covers it under A15c.
  - Subtract deletions only through measurement; delay overestimates usage rather than understating it.
  - Reservation lemma: every live-generation operation absent from the last measurement owns a live ticket. Chrome-measured disk ≤ inUse + R_bytes + dead-generation effects; each admission preserves the bound. Likewise for names and sites.
- Site slots (D105, D112, I66):
  - sitesLive = distinct (netKey, address) keys with at least one record name in the last adopted getKeys, regardless of content: data, {} or tombstone.
    - Never count a key twice.
    - Remove a key only after an adopted measurement sees no record for it and it has no live-generation pin (D113).
    - pin (D113): every non-remove DiskLedger ticket for an operation on a key counted at admission pins it in sitesLive until ticket removal (D105). The measurement dropping the ticket was requested after settle and sees its effect if applied. Thus no operation-created key remains uncounted.
    - A site-slot ticket drops in the same step as adopting the measurement that sees it (D105), with no gap or double count.
  - Two operations can create a new key: data set and checkpoint for a recordless key (dictionary {}).
    - A tombstone cannot create a key because it issues only after a record exists for the key in the live generation.
    - remove cannot create a key.
  - LATE_SITES_PER_GEN = STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS = 4. Every unsettled creating operation owns a slot until settle (D102, D109). A successfully settled operation is visible to the next generation; a rejected operation does not apply (A15d). Thus dead-generation operations able to create keys later ≤ 4.
  - lateGens(now) = number of visible epoch items X ≤ E, including the live generation, with bootMs(X) ≥ now−T_LATE (D134).
    - Each X represents its preceding generation, whose operations apply or are dropped before bootMs(X)+T_LATE (A15c).
    - Count ≤ GEN_WINDOW_MAX under the generation window.
    - These items are not swept before leaving the window.
    - (D134) An unconfirmed generation writes only a shared-name epoch item, not data. Older generations may write that item's value late, making stored bootMs older.
    - Coverage lemma: for every dead data-writing generation D, every writer of E_D+1 booted after D. Otherwise, seeing E_D before D booted would make D choose E_D+1, not E_D.
    - Thus stored bootMs ≥ boot time of D's successor, keeping the item in the window while D's operations may apply.
    - Visible items from unconfirmed generations also count, conservatively.
  - LATE_SITES(now) = LATE_SITES_PER_GEN·lateGens(now) ≤ 16.
    - X's contribution does not disappear on elapsed time alone, but on completing a refresh requested after bootMs(X)+T_LATE and adopting its measurement in the same step.
    - DiskLedger schedules refresh at every expiry.
  - Admission: every new-key-creating operation (not in sitesLive, no reserved slot) requires sitesLive + R_sites + LATE_SITES(now) + 1 ≤ STORE_SITES_MAX = 64.
    - Reserve the slot in the same admission step.
    - Violation gives 4300 {reason: 'sites', limit: 64, retryAfterMs?} without set.
    - retryAfterMs is time until the earliest expiry sufficient for admission; omit when sitesLive + R_sites alone prevent admission.
    - Operations on counted or reserved keys need no slot and are unaffected by LATE_SITES.
  - Removing a key's last record (sole tombstone) requires:
    - No session, running gate or live ticket for the key.
    - Every epoch item older than the live generation has left the window.
    - Release the slot only through subsequent measurement.
  - Counted key with retained tombstone (D113, I67):
    - A tombstone is a record, so the key is in sitesLive with its slot retained.
    - Every operation on it reserves no slot, does not increase R_sites, and pins it. Includes a checkpoint with dictionary {} and the first set after deletion resolves.
    - A new slot is needed only when all three hold: last record removed, an adopted measurement sees no key, and no pin. Then the first operation is creating and subject to admission.
  - TombReaper (D113):
    - Check removal conditions above at every adopted refresh and every epoch window exit.
    - If satisfied, issue remove for the sole tombstone with a zero ticket.
    - Opening a key session before issuance prevents it.
    - Issued remove is not cancelled. A later operation before the measurement that no longer sees the key is treated as acting on a counted key and pins it. Thus it is neither double-counted nor dropped before that operation's effect appears.
    - Late remove from a dead generation follows the same rule because pin follows live-generation operations.
  - Sites lemma (conditional on A15c and A15d): at every instant each on-disk key is in one of three cases:
    - (1) In the last adopted measurement.
    - (2) Has a live ticket in the live generation.
    - (3) Created by an unsettled operation from dead generation D: ≤ 4 per D, ending before bootMs(succ(D))+T_LATE; succ(D) stays in lateGens until a measurement requested after that time.
    - Admission preserves the sum of the three bounds ≤ 64. Therefore on-disk key count ≤ 64 at every instant.
    - No record is automatically deleted to repair an overrun.
  - Effect: after each boot, new-site admission capacity decreases by 4 per generation in the window, for up to 60s. Existing sites are unaffected; rejection occurs only if sitesLive + R_sites > 63 − LATE_SITES(now).
- Bound lemma (conditional on A15c): Chrome-measured disk ≤ inUse + unres + operations of ≤ GEN_WINDOW_MAX dead generations ≤ STORE_DISK_HARD. Each dead generation has:
  - ≤ 2 data sets, because there are 2 global slots.
  - ≤ RECOVERY_SLOTS = 2 checkpoints, by the RecoverySlots lemma (D109).
  - ≤ ADMIN_SLOTS = 2 tombstones, by the AdminSlots lemma (D108), within META_RESERVE.
  - Epoch items are not counted per generation; total EPOCH_NAMES_MAX = 8 follows from the name-bound lemma (D134), within META_RESERVE.
- Generation window: EpochFence does not issue its item if GEN_WINDOW_MAX = 4 visible epoch items have bootMs ≥ now−T_LATE (D134).
  - Lemma: a data-writing generation sees ≤ 3 items in the window before issuance.
  - After issuance, no new name other than its own appears (name-bound lemma); a late write does not make bootMs newer.
  - Thus lateGens ≤ 4 throughout its lifetime.
  - Wait until the oldest leaves the window; no write or snapshot before then.
  - A waiting generation has written no item and does not count in the window.
- Sweep: at boot and after every successful set, delete only records below the greatest present record per key. A failed remove leaves the record counted in inUse and names, so admission stops at the bound instead of growing.
- Entire-site deletion requires user action from “Storage management”, with confirmation showing site and size:
  - Execute AdminDelete (D106) below, then sweep lower records.
  - One AdminDelete(key) per key; any second request (from another page) shares its promise.
  - 1: key becomes deleting; viewer prevents new sessions and shows “Deleting data”.
  - 2: close every key session in every tab:
    - Tear down frames with notice “Site data deleted”, without automatic reload.
    - Their queued and ready messages become suppressed, then release.
    - Active writes retain ownership until settle (D102).
  - 3: wait for RecoveryGate resolution (≤ 25s, D109, D110). ready, failed and unrecovered (busy or readTimeout) all continue. The latter issued no write; late reads are ignored through gateId. If unrecovered with no running gate, continue immediately.
  - 4: allocate the tombstone version with SeqAlloc.alloc() after those two steps, with a DiskLedger ticket. Take an AdminSlots slot (D108), not STORE_ACTIVE_GLOBAL. No slot gives adminBusy without a tombstone.
    - Do not wait for active writes; its version is greater than every previously allocated version.
    - Allocate no other version for the key while deleting.
  - Successful tombstone settle: confirmed = tombstone, dictionary {}, total = 0. Late settlement of an older write does not change the dictionary because its version < confirmed.
  - Tombstone failure or timeout (5s):
    - Second attempt with a fresh version on an AdminSlots slot (D108).
    - First attempt retains its slot until settle.
  - The following outcome applies in three cases: both attempts fail, both time out, or no second-attempt slot:
    - Key becomes unrecovered; the first later session performs fresh recovery with a checkpoint.
    - Page shows “Deletion failed”, or “Deletion outcome uncertain” for timeout.
    - A tombstone applied before recovery read becomes its basis: successful deletion. One applied afterward is below the checkpoint and never becomes visible.
  - Recreation (D113): a user-created session after resolution sees {}; its first set version is greater than the tombstone.
    - While the tombstone remains counted, the session reserves no site slot and is unaffected by LATE_SITES.
    - If TombReaper removed it and an adopted measurement sees no key, the first checkpoint creates the key and reserves one slot under D105/D112.
  - Deletion lemma: each set in the generation was allocated before the tombstone or after resolution; earlier-generation versions < (E, 0). Therefore no record from an old snapshot supersedes the tombstone.
  - No-late-tombstone lemma (D111): each tombstone issues in a step passing the guard, op.state = running. Resolution changes state to settled before any later step; nothing returns it to running. No tombstone can issue for a resolved operation, so no such tombstone supersedes a set from a post-resolution session. A pre-resolution unsettled tombstone has a lower version than every post-resolution allocation and cannot supersede a new write.
  - Do not delete the tombstone before every older generation leaves the T_LATE window.
- AdminSlots (D108, I61): tombstone-slot ownership.
  - ADMIN_SLOTS = 2 extension-wide slots outside STORE_ACTIVE_GLOBAL. Slot state ∈ {free, held(op)}.
  - Acquire only at tombstone set issuance, through global AdminFIFO in request order, in the same synchronous step as DiskLedger.reserve.
  - Release only when the tombstone promise settles, success or failure. Neither timeout response, AdminDelete resolution nor notice releases it. Generation loss drops all state; a pending tombstone remains a late dead-generation operation.
  - Wait ≤ ADMIN_SLOT_WAIT = 5s per attempt; no tombstone without a slot.
  - Attempt 1 without a slot: adminBusy. No tombstone; key unrecovered; data retained. Page shows “Could not start deletion; data retained”.
  - Attempt 2 starts after first-attempt failure settlement or timeout. If no slot:
    - uncertain if the first is pending.
    - failed if the first has failed.
    - In either case, key becomes unrecovered.
  - Late successful tombstone settlement after resolution does not change resolution; subsequent recovery governs it as above.
  - Operation identity (D111, I65):
    - Each AdminDelete has a generation-unique opId and state ∈ {running, settled}.
    - deleting[key] holds the running operation's opId.
    - Every AdminFIFO request carries its operation's opId and requestId.
  - Resolution: the first successful tombstone-attempt settlement before resolution immediately yields deleted. Resolution with any outcome (deleted, failed, uncertain or adminBusy) performs the following in one synchronous step:
    - op.state = settled.
    - Withdraw every pending AdminFIFO request for the operation without seq allocation or reservation.
    - Remove the key from deleting state.
    - Release new sessions to proceed.
  - Issuance guard: in the synchronous slot-grant step, before DiskLedger.reserve and SeqAlloc.alloc, check:
    - Request not withdrawn.
    - op.state = running.
    - Key is deleting.
    - deleting[key].opId = op.opId.
  - Any failed check: immediately return the slot to the next AdminFIFO request in the same step, with no reserve, alloc or set; increment adminStaleDropped. Grant, guard, reserve, allocation and issuance form one step without await.
  - RecoverySlots (D109) uses the same guard: issue only if request gateId is current and gate unresolved. A mismatch returns the slot and increments recoveryStaleDropped.
  - AdminDelete time bound ≤ 25s (gate resolution including reads, D110) + 2·(5s wait + 5s deadline) = 45s.
  - AdminSlots lemma:
    - Every issued tombstone owns a slot from issuance until settle; none issues without a slot.
    - Therefore issued unsettled tombstones per generation ≤ ADMIN_SLOTS at every instant, including generation loss.
    - Each dead generation leaves ≤ 2 late tombstones, regardless of keys or attempts.
  - Different-key requests compete in AdminFIFO; same-key requests share one promise.
  - TOMB_BYTES_MAX = 512 for tombstone name plus encoded value. RecordCodec rejects larger values; nothing is issued.
- Enumeration: getKeys is mandatory; availability is checked in M0 and BR22c.
  - Absence disables local storage with storeRecovery; never use get(null).
  - Enumeration memory is proportional to names ≤ STORE_RECORDS_MAX+LATE_NAMES under A15c.

**7. Monitoring**
stats returns {epoch, recoveries, recoveryCorrupt, lateRecordsSeen, storeRecoveryFail, inUse, names, sitesLive, diskRejects, removeFailures, genWaitMs, seqMax, resvBytes, resvNames, resvSites, ticketsLive, adminDeletes, adminDeleteFailed, adminDeleteUncertain, adminBusy, adminSlotsHeld, adminTombsUnsettledMax, recoverySlotsHeld, recoveryUnsettledMax, recoveryBusy, readSlotsHeld, readTimeouts, lateReadsDropped, recoveryCorruptFail, adminStaleDropped, recoveryStaleDropped, lateGens, lateSites, sitesRejectsLate, pinnedKeys, tombReaps, epochNames, epochGateBlocks, epochResurrected}. lateRecordsSeen counts records appearing after the checkpoint with lower versions.

**Wallet (D14)**
- eth_requestAccounts requires approval per (netKey, siteAddress).
- eth_sendTransaction after connection, with approval bound to txHash.
- Cancel with 4001 on port disconnection, worker restart, more than one pending request or more than 5 per minute.
- Reject: switch/addEthereumChain, eth_sign, personal_sign, signTypedData, eth_sendRawTransaction.
- AES-GCM with PBKDF2 at 600k iterations; lock after 15 minutes.

Opening a site alone signs nothing, grants no account, broadcasts no transaction and opens no external tab. Store policies are unverified; the experiment uses an unpacked extension.

## Node and network interfaces

**P2P**
libp2p over TCP+Noise, with gossipsub and request/response on '/pocol/1/req'.

**Messages**
- Status = [chainId, genesisHash, protocolVersion, headHash, height, totalWork]. An identity mismatch or unsupported version disconnects the peer. Status is informational and does not replace 1b.
- Request = [reqId, kind, body]: 1 GetHeaders [locator ≤ 32, count ≤ 512], 2 GetBodies [hashes ≤ 64], 3 GetTemplate, and 4 GetEnvelope [blockHash, exclude ≤ 4 hdrId] (D131). GetEnvelope returns the stored envelope-valid copy if its hdrId is not in exclude, otherwise NotFound.
- Response = [reqId, status, payload]. status: 0 success, 1 not found, 2 limit exceeded, 3 invalid, 4 rate limited. A missing body is 0x80; truncate at 8 MiB.

**gossip (byte limit)**
- tx: 131072.
- template: 197294. Check: size, signed form (otherwise templateForm), 1b, ecrecover, then evaluable items.
- block: 3067 (header only).
- share: 43.
- evidence1: 1368.
- evidence2: 243.

**Receiving block**
1. Size, then 1b without the parent. Failure incurs a penalty without storage.
2. Unknown parent: unknownParent queue ≤ 64, without work or ASERT. Parent rejection drops the child as ancestorInvalid.
3. Remaining H-pre (including 10a), then body, then H-full, then broadcast.
4. (D129) envReject drops the copy and penalizes its sender without judging blockHash. Request another copy from a peer that announced the block. Broadcast sends the stored copy byte for byte.

**Synchronization (D38)**
1. Status and locator.
2. R: H-pre for every header; violations drop the header and descendants and ban the peer. ≤ 4096 per branch and ≤ 16384 overall.
3. E: GetBodies, then sequential H-full.
   - A body not matching txRoot and evidenceRoot is a response error (BodyMismatch, D125): drop it, penalize the peer and request it from another peer; retain the header.
   - A matching body violating H-full through blockReject invalidates the block and descendants. envReject (including the signature part of item 13) rejects only the copy; retain the header and its entry and request another copy (D130).
   - Fetch scheduling (D128, node behavior, not validity) uses three mechanisms:
     - DiscRun: one discovery run with a dirty flag and per-run tried set.
     - BodySched: one in-flight body request per hash, with triedPeers.
       - Reeligibility (D130, amending D129) places the peer in reeligible without removing it from tried in the current cycle. Triggers: its Status, announcement of the block or a descendant, or expiry of exponential backoff min(1s·2^k, 60s).
       - Strict selection: untried peers in ID order, then reeligible peers in announcement order. Never request a reeligible peer while x has an untried peer.
       - cycleReset(x) starts a new cycle: clears tried, reeligible and backoff timers for x. In pocold this occurs when x becomes requestable again after removal; in SimNet it occurs for all hashes in the atomic t_avail step (fixture command).
     - EnvFetch (D131, node behavior, not validity): activated for x when every header copy at i is rejected by envReject, or i has no envOk copy once parent state is available.
       - Request GetEnvelope [x, exclude], where exclude contains the latest ≤ 4 rejected hdrId values for x. An honest node stores and serves only an envOk copy.
       - Scheduling follows BodySched: one in-flight request per x, per-cycle triedEnv and reeligible, and strict ID-order selection of untried peers. reeligible is FIFO by entry time, with one position per peer; reentry after a request moves it to the tail (D139). cycleReset at t_avail in SimNet; exponential backoff in pocold.
       - (D137) No request for x before parent state is available. envOwn must be evaluable, so a solicited response never enters envPending.
       - EnvQueue: one entry per x needing a fetch, FIFO by need time, ≤ 64 B per entry. Entries ≤ synchronization queue bound (16384).
       - EnvSlots (D137): ENV_SOLICIT_SLOTS = 32 slots for solicited requests, separate from ENV_GOSSIP_SLOTS = 32 for unsolicited-copy checks. Total = ENV_INFLIGHT_GLOBAL = 64.
         - A slot owns one check and a response buffer ≤ ENV_REPLY_MAX = 3067+64 = 3131 B.
         - Grant: only to the EnvQueue head, in the same step that selects the peer and sends the request. No request without a slot. Releasing a solicited slot grants it to the EnvQueue head in the same step, if present; grants are work-conserving (D139).
         - States (D138): reserved (request sent, buffer empty), then buffered (response read into the reserved buffer), then checking (check reads the buffer), then released.
         - Attempt: each grant receives a node-local monotonic u64 attemptId, never reused.
           - Every event carries (slotIdx, attemptId): response, timeout, disconnection and check result.
           - An event not matching the slot's current attempt is ignored and changes no slot.
         - Ordering: EnvFetch events are processed in one event loop in occurrence order. At equal times, response completion precedes timeout: a response is on time if its final byte arrives at t ≤ sendTime+ENV_REQ_TIMEOUT.
         - Timeout: ENV_REQ_TIMEOUT measures only from send to the final response byte. Transition to checking cancels its timer; no timeout runs during checking.
         - Cancellation is separate from ownership. Every termination cause sets cancelRequested, then:
           - In reserved or buffered: reset the request substream, discard partial bytes and release in the same step, because no task is reading the buffer.
           - In checking: no release. The check (envPre then envOwn, without apply) is synchronous, uninterruptible computation run only in EnvCheckPool (D139). It owns the slot and buffer until settlement, however long that takes, and releases in the settlement step itself.
             - EnvCheckPool: ENV_CHECK_WORKERS = W = 4 dedicated threads running only envelope checks, with a FIFO queue by entry into checking. No envelope check runs in ComputePool or the consensus runtime.
             - c_exec: wall-clock execution time of one check; assumed ENV_EXEC_MAX = 50 ms.
             - Waiting-bound lemma: live checks ≤ 64, so at most 63 precede any check; a thread becomes available within ≤ ⌈63/W⌉·c_exec.
             - Therefore c_env = (⌈63/W⌉+1)·ENV_EXEC_MAX = 17·50 = 850 ms, provided c_exec ≤ ENV_EXEC_MAX.
             - Exceeding ENV_EXEC_MAX increments envCheckOverrun, does not release the slot, and invalidates the time bound, not the capacity bound.
         - Cancelled-check result:
           - Retain envReject and penalize the sender if x still has a local entry, because the judgment depends only on the bytes.
           - Use envOk only if x remains in envWait.
           - Otherwise discard the result and increment lateCheckDropped.
           - In all cases: never import a removed or already imported block, or release another attempt's slot.
         - Capacity lemma (D138): every checking task and received byte is owned by a slot from grant until release, and release never precedes check settlement. Thus at every instant, regardless of repeated cancellations:
           - In-flight checks ≤ 32 solicited + 32 gossip.
           - Live buffers ≤ 64·3131 B.
         - Gossip slots follow the same rules: acquireGossip reserves before checking, and copy bytes move into the slot buffer.
         - A response exceeding ENV_REPLY_MAX is cut off at the bound and treated as a response error penalizing the sender.
         - Release occurs exactly once, guarded by a flag and attempt ID. In checking it occurs only at check settlement (D138). There are eight attempt-termination causes:
           - Check completion (envOk or envReject).
           - NotFound.
           - ENV_REQ_TIMEOUT = 2s expires after send before full response reception. It does not apply in checking.
           - Peer disconnect.
           - Oversized response.
           - Import of x.
           - An envOk copy of x is stored through another path.
           - blockReject of x or removal of x.
         - After unsuccessful release, x returns to the EnvQueue tail if it remains in envWait and has an untried or reeligible peer. Otherwise it awaits a reeligibility event, then joins the tail.
         - Timeout places the peer in triedEnv for the cycle, with exponential reeligibility backoff as for NotFound. No permanent exclusion and no penalty for slowness alone.
         - After timeout or cancellation in reserved or buffered, reset the substream, so a late solicited response cannot arrive on that request. A later copy sent by the peer has no solicited slot and follows the unsolicited path below, without a lateness penalty.
       - A response whose copy fails envelope checks is a response error: penalize the sender and place it in triedEnv. A duplicate copy (hdrId in EnvCache) is not checked again.
       - envPending copies ≤ ENV_PENDING_MAX = 4 per x. Drop excess without checking or evicting stored copies; dropping does not prevent later EnvFetch.
       - Envelope-judgment retention (D136, local node policy, not validity):
         - Stored envReject judgment = {hdrId, blockHash, rule, peer, seq} ≤ ENV_ENTRY_MAX = 128 B. Rejected-copy bytes are not retained.
         - Retain judgments only for a blockHash with a local entry: HeaderStore, synchronization queues, unknownParent or side blocks. Otherwise penalize and discard the copy without retention.
         - Bounds: rej(x) ≤ ENV_REJ_PER_BLOCK = 8; total ≤ ENV_REJ_GLOBAL = 16384. Global envPending ≤ ENV_PENDING_GLOBAL = 1024. In-flight checks ≤ ENV_INFLIGHT_GLOBAL = 64, including ≤ 32 solicited and ≤ 32 unsolicited (D137).
         - An unsolicited copy (gossip or announcement) of x with no envOk copy follows:
           - Stored hdrId: drop without checking.
           - rej(x) = 8, pending(x) = 4, global capacity full, or no free gossip slot while parent state is available (D137): drop without checking or penalty, increment envFloodDropped. Without parent state, retain as envPending within both bounds, without a slot. x stays in envWait and EnvFetch remains active.
           - Otherwise: check it.
         - A solicited copy (GetEnvelope response from the selected peer, on a slot reserved before requesting, D137) is always checked within ENV_SOLICIT_SLOTS, never exceeding the check cap. If rej(x) = 8, evict x's oldest judgment before insertion. At most one request per (x, peer) per cycle; ≤ 32 globally in flight.
         - Global capacity pressure evicts the oldest inserted judgment and increments envEvicted.
         - Cleanup: when x is imported, an envOk copy is stored, blockReject occurs, or x is removed by queue bounds or D118, delete all its judgments and pending copies.
         - An evicted judgment may be recomputed on rearrival, with the same deterministic result. “Each hdrId once” applies while its judgment remains stored.
         - Capacity pressure never records blockReject or pocol_getRejects, or triggers descendant cancellation.
         - Recovery lemma: for each x in envWait at honest i, an honest holder h* has an envOk copy (D139). Definitions:
           - t_0 (D140): first instant when G1–G4 all hold and continue holding until acceptance:
             - G1: parent state of x is available at i (D137 send prerequisite); x remains in envWait with a local entry not removed by queue bounds or D118.
             - G2: h* is connected to i, holds an envOk copy and serves it.
             - G3 (responsiveness): every GetEnvelope request from i to h* sent after t_0 receives its final response byte within RTT_h* < ENV_REQ_TIMEOUT.
             - G4: i's peer set is stable during the interval (P defined below).
             - Time before t_0 (preCondWait) is excluded from D_x and recorded as a measurement without any timing claim.
             - Losing any prerequisite before acceptance voids the guarantee. When all return, start a new t_0' and compute P, Q and B from it.
             - The earlier D139 definition (only x in envWait and connected h*) is withdrawn: D137 forbids requesting before parent state, and connectivity does not guarantee a timely response.
           - P: number of distinct peers EnvFetch may select for x during the interval, including h*.
             - Assuming i's peer set is stable, P ≤ 50 (P2P bound).
             - Each new connection increments P; no fixed bound is claimed under unlimited churn.
           - Q: maximum EnvQueue length during the interval including x; Q ≤ 16384.
           - B: zero if h* is untried or reeligible at t_0; otherwise its remaining exponential backoff, B ≤ 60s.
           - Solicited requests for x through the h* response, including success, ≤ 2P−1. Reason:
             - At most P−1 untried peers.
             - Then at most P−1 peers preceding h* in reeligible.
             - Then h*.
           - Each response is checked on its reserved slot.
           - (D137, D138) Each slot turn releases within ≤ ENV_REQ_TIMEOUT + c_env, because:
             - The timeout measures reception alone.
             - Subsequent checking, from entering checking to settlement including EnvCheckPool wait, is ≤ c_env and uninterruptible (D139).
             - Cancellation neither shortens the turn nor extends it beyond this bound.
           - Wait until each request is granted ≤ ⌈Q/32⌉·T_role, where T_role = ENV_REQ_TIMEOUT + c_env. Reason:
             - Grants are work-conserving, so each T_role interval includes ≥ 32 grants while the queue stays nonempty.
             - At most Q−1 entries precede x.
           - The request's own turn is then ≤ T_role.
           - Hence acceptance delay after t_0 ≤ D_x = (2P−1)·(⌈Q/32⌉+1)·T_role + B.
             - For P=1, Q=1, B=0: D_x = 2·T_role, covering reception and checking.
             - The former formula (P−1)·⌈Q/32⌉·T_role is withdrawn because it omitted the successful request and its turn.
           - Reeligibility waits while x is outside the queue without a selectable peer are confined to [t_0, t_0+B], because h* remains selectable afterward until requested.
           - The bound grows with Q but stays finite because Q ≤ 16384. Capacity pressure does not prevent acceptance while G1–G4 hold.
           - Slow holder (D140): if h* consistently responds later than ENV_REQ_TIMEOUT, every attempt times out and resets the substream; the late response is unread.
             - No acceptance guarantee through the solicited path; this is not merely delay bounded by exponential backoff.
             - Acceptance remains possible only through gossip when a gossip slot is free, or through another holder satisfying G3.
             - This is service policy: x stays in envWait, without blockReject or an entry in pocol_getRejects; timeouts increment envTimeouts.
         - Memory (D137): judgments 16384·128 + pending copies 1024·3067 + slots 64·3131 + EnvQueue 16384·64 = 2097152 + 3140608 + 200384 + 1048576 = 6486720 B ≤ ENV_MEM_MAX = 7 MiB (7340032). No response buffer outside slots.
       - Drain-phase fetch lemma:
         - envOk is deterministic: envPre is stateless and envOwn uses committed parent state. An honest response is valid at the requester.
         - Each x ∈ Rec has an honest holder of an envOk copy, established by HaltPin (validEnv in validation).
         - Strict selection reaches it within ≤ |H|−1 requests, with ≤ one request per (x, peer).
         - Thus GetEnvelope requests after t_avail ≤ |Henv_i|·(|H|−1).
         - Every received copy existed before t_avail (copy lemma), so envCheck ≤ Venv_i.
     - HfullDone and HdrCache: H-full is not repeated for a settled block. Two-key storage (D129):
       - BlockCache keyed by blockHash: completed judgments of committed content only.
       - EnvCache keyed by hdrId: envelope judgments.

     Details in validation.
4. Adoption at depth ≤ R_keep.

**Input constraints**
No RPC accepts blocks, headers or templates. pocol_submitShare checks TemplateID against an accepted local template.

**Mempool**
- len > TX_MAX: −32014.
- chainId mismatch: −32015.
- Full: −32016; after F: −32016 {final: true}.
- Transactions returned after reorg are rechecked.

**P2P and node resource limits**
- ≤ 50 peers, queue ≤ 256, ≤ 16 per peer.
- Mempool ≤ 64 MiB.
- Node memory excluding RPC ≤ 750 MiB, including redb cache, envelope judgments, slots and EnvQueue ≤ ENV_MEM_MAX = 7 MiB (D136, D137). RPC memory ≤ 192 MiB.
- ASERT ≤ 512 bits.
- Ban after 3 violations of 1b or templateForm in 10 minutes through gossip; immediately during synchronization.

**Standard JSON-RPC**
- eth_chainId, net_version, web3_clientVersion.
- eth_blockNumber, eth_getBlockByNumber/ByHash, eth_getTransactionByHash, eth_getTransactionReceipt, eth_sendRawTransaction.
- eth_call, eth_estimateGas, eth_gasPrice, eth_maxPriorityFeePerGas, eth_feeHistory (≤ 1024).
- eth_getBalance, eth_getCode, eth_getStorageAt, eth_getTransactionCount.
- eth_getLogs (≤ 1024 and D88 limits), eth_getProof (within K_eff, otherwise −32017).
- Unsupported: filters and eth_subscribe (−32601). HTTP/1.1 transport only.

A site's extension-accessible methods are a subset defined by the BridgeAuth matrix in browser, not this list. site_* methods are extension-local; the node does not recognize them and returns −32601 if they reach it.

**pocol_getHeaders(from, count) (D80, D81)**
RLP list of canonical-chain headers from..from+count−1. Fixed-order checks stop at the first violation:
- F0: valid JSON quantities within u64 (hex without leading zeros). Invalid, missing or extra parameters: −32602.
- F1: from < 1: −32018 {fromZero}.
- F2: count < 1 or > 512: −32018 {countRange}.
- F3: from > head: −32018 {fromAboveHead}.
- F4: from+count−1 > head (u128): −32018 {beyondHead}.

F0–F2 do not read state. F3–F4 read head from the request snapshot. At head=0 every request passing F0–F2 returns fromAboveHead. Success returns exactly count headers; the client sends ≤ 14.

**RpcGuard (D87, D88, D90, D92, D93, D94): local service policy, not a validity rule**

Bounds are test assumptions, exposed in pocol_getParams.rpcLimits.

**(A) Transport and invariant S1**
- HTTP/1.1 on hyper, no HTTP/2 or pipelining, on 127.0.0.1 only.
- One request at a time; do not read the next before sendDone of the previous reply.
- S1: at most one response reservation and one waiting request per connection.
- Explicit SO_SNDBUF = SEND_SOCK_BUF = 32 KiB per connection disables autotuning. No bound is assumed on the acceptance/read gap; measure it in RG14(0) and tracing, without using it in any criterion (D93).

**(B) Classification table (worst-case encoded response)**

Light (LIGHT_RESP_MAX = 512 KiB):
- ≤ 4 KiB:
  - eth_chainId, net_version, web3_clientVersion, eth_blockNumber.
  - eth_gasPrice, eth_maxPriorityFeePerGas, eth_getBalance, eth_getStorageAt, eth_getTransactionCount, eth_sendRawTransaction.
  - pocol_getTiming, pocol_getTarget, pocol_getCaps, pocol_syncStatus, pocol_getPot, pocol_getSettlement.
  - pocol_getBurnTotals, pocol_getEscrow, pocol_getRetention, pocol_getClosing, pocol_getRpcStats, pocol_submitShare.
- eth_getCode: ≤ 2·B_code_max+1 KiB ≈ 65 KiB.
- eth_getTransactionByHash: ≤ 2·TX_MAX+2 KiB ≈ 258 KiB.
- eth_getTransactionReceipt: ≤ 2·LOG_BLOCK_MAX+546·512 B+2 KiB ≈ 339 KiB.
- eth_getBlockBy* with hashes: ≤ 2 KiB+(BODY_MAX/85)·70 B ≈ 160 KiB.
- pocol_getHeaders with count ≤ 64: ≤ 64·(2·3067+16) B ≈ 385 KiB.
- pocol_getParams: ≤ 2·|genesisPre|+4 KiB, thus ≤ 92 KiB in end/full.
- pocol_getAssignment, pocol_getMinerSet and pocol_getTemplate: ≤ 1024·160 B = 160 KiB.

Heavy (RESP_MAX = 8 MiB): eth_getLogs, pocol_getLogs, eth_call, eth_estimateGas, eth_getProof, eth_feeHistory, pocol_getHeaders with count > 64, eth_getBlockBy* with full transactions, pocol_getRejects.

Unlisted method: −32601 before admission. Adding one requires a table row and passing RG13. Overflow while encoding: −32020 {resultBytes}, with clean release (RG12).

**(C) Global budgets (node-wide)**
- RPC_CONN_MAX = 32; close excess connections without reading.
- Permits: HEAVY_ACTIVE = 4, HEAVY_QUEUE = 16, LIGHT_ACTIVE = 16, LIGHT_QUEUE = 32. FIFO queues; an honest request waits behind ≤ 31 requests.
- Per-connection token bucket: rate 20/s, capacity 20:
  - Single request consumes one token; a batch consumes k tokens at parsing.
  - Insufficient tokens return −32021 {rate, retryAfterMs} without execution. For a batch, write an array of k errors ≤ 256 B from CONN_BUF.
  - No waiting for tokens within a request or batch.
- Response memory: HEAVY_RESP_MEM = 48 MiB, LIGHT_RESP_MEM = 16 MiB = 32·512 KiB, so light reservations never run out (S1).
- SCRATCH: ≤ 16 MiB per heavy permit, ≤ 1 MiB per light permit. Computation is synchronous without await on a thread from a fixed pool (20 threads), with thread-local accounting; overflow returns −32020 {scratch}.
- REQ_BUF = 1 MiB, CONN_BUF = 64 KiB per connection.
- RPC_MEM_TOTAL = 48+16+4·16+16·1+32·(1+1/16) = 178 MiB ≤ 192 MiB.
- Encode into a list of 64 KiB segments inside the reservation, without doubling reallocations.
- Kernel buffers (32 KiB per connection) are outside accounting and measured within RSS, including accepted-but-unread bytes after sendDone.
- Snapshots ≤ 20: one per permit.

**(D) Request ownership lifecycle (D88)**
1. Reading:
   - Starts at the first byte. Headers and body ≤ REQ_BUF within REQ_READ_DEADLINE = 10s, otherwise close.
   - Close idle connections between requests after IDLE = 30s, measured from previous response sendDone or connection opening.
2. Parsing: F0, origin denial (H2), classification, rate bound and numeric log-range check. All within REQ_BUF, without reservation or snapshot.
3. Atomic admission: permit and class-maximum reservation together for the queue head.
   - Waiting requests hold nothing.
   - Within QUEUE_WAIT = 2s, otherwise −32021 {busy, retryAfterMs ∈ [250, 2000]}.
4. Computation owns {permit, reservation, snapshot, SCRATCH}; encode inside the reservation. COMPUTE_DEADLINE from admission includes encoding: 5s heavy, 500 ms light; expiry returns −32020 {deadline}.
5. End of computation: release snapshot, SCRATCH and permit; shrink reservation to actual size.
6. Sending under SendPacer (D90, D92, D93, D94):
   - P = bytes submitted by server to pacer (written); A = bytes accepted by server socket; A ≤ P.
   - pending is true when P > A.
   - sendTime advances only while pending, starting with the first submitted byte.
   - R = MIN_SEND_RATE = 65536 B/s; GRACE = 2s.
   - Sole binding rule: close if sendTime > GRACE and A < R·sendTime.
   - Credit = A − R·sendTime; violation instant s_v = max(GRACE, A/R).
   - Enforcement:
     - Timer at sendTime = s_v+TIMER_EPS, where TIMER_EPS = 1 ms resolves equality because the rule is strict.
     - Reset timer whenever A or pending changes; backstop check every 50 ms, and recheck exact values when it fires.
     - lateness ≤ ENFORCE_SLACK = 200 ms, recorded in pacerLateMaxMs.
   - Bound theorem:
     - While the connection remains open, sendTime ≤ max(GRACE, A/R)+ENFORCE_SLACK and A ≤ size.
     - Thus SEND_MAX(size) = GRACE+size/R+ENFORCE_SLACK. An 8 MiB reply → 130.2s; a 512 KiB reply → 10.2s.
     - Credit does not raise the bound.
   - CloseOK: every rate-triggered close satisfies sendTime > GRACE, A < R·sendTime, and sendTime − s_v ≤ ENFORCE_SLACK; record (sendTime, A) in RpcStats.
   - PacerTrace (D93):
     - rpc.pacerTrace=true, available in release builds.
     - Record a row on every P, A or pending change and at close/sendDone: {conn, t_ns (monotonic), sendTime_ns, P, A, pending, event ∈ {produce, accept, pendStart, pendStop, close, sendDone}}.
     - Does not change decisions or consensus.
     - Fixed 4 MiB buffer; overflow increments traceDropped; traceDropped > 0 makes a run inconclusive.
   - sendDone (D94): first instant A = P = size after all reply bytes (or the entire batch including ']') have been submitted. At this instant:
     - Pacer.write returns Done; release the reservation.
     - sendTime stops permanently, record sendDone, start IDLE.
   - receiveDone: instant Rc = size at the client.
     - Invisible to the server; does not delay reservation release or enter any timeout/rule.
     - Measured only in client logs and PacerSim.
     - Bytes between A and Rc at sendDone (unreadAtSendDone) are in kernel buffers, not the reservation.
   - Release the reservation at sendDone or close, whichever comes first.
7. Do not read the next request before sendDone.

The only independent deadlines are REQ_READ_DEADLINE, IDLE, QUEUE_WAIT, COMPUTE_DEADLINE, the SendPacer rule and BATCH_WALL. All measure server events, not client reception.

**(E) Batches (D88, D90, D92)**
- ≤ BATCH_MAX = 20 items, chunked streaming: write '[', then each item passes stages 3–5 and submits its reply to the pacer.
- Item k+1 starts admission as soon as the socket accepts the final byte of item k. The server can therefore precede client reading by up to socket-buffer capacity.
- Between items hold no permit, snapshot or reservation; each item reenters admission at the queue tail.
- One SendPacer per batch: one sendTime starting at '[', paused without pending (queue wait and computation), one GRACE, and the rule uses cumulative A; no per-item deadline or grace.
- Batch sendDone occurs when the final framing byte is accepted. Each item's reservation releases on acceptance of its final byte.
- batchSent = all submitted bytes. The item that makes batchSent > BATCH_RESP_MAX = 16 MiB is computed, then replaced with −32020 {batchBytes}; subsequent items receive the same error without execution. Each error ≤ ERR_ITEM_MAX = 256 B from CONN_BUF.
- BATCH_BYTES_HARD = 16 MiB+20·256+64 = 16782400 B.
- Derived bounds:
  - BATCH_SEND_MAX = 2+256.0791015625 = 258.0791015625s; monitoring bound 258.2791015625s.
  - Waiting ≤ 20·(2+0.2) = 44s; computation ≤ 20·(5+0.1) = 102s.
  - Total ≤ 404.2791015625s.
- BATCH_WALL = 420s backstop from end of reading to sendDone; R26(F) requires ≥ 404.2791015625. Reaching it closes/releases within ≤ 100 ms and increments batchWallHits; any nonzero value in release is a defect.
- For a busy or rate item, write its error and continue. Preserve order and copy IDs. Mid-batch disconnection produces incomplete JSON that is not parsed.

**(F) Cancellation**
Set a token on close or any timeout; poll every 16 scanned blocks, every 1 MiB encoded and every 10^6 gas. Release all resources within ≤ CANCEL_MAX = 100 ms.

**(G) eth_getLogs**
Order:
1. F0 (−32602).
2. Numeric range ≤ 1024, otherwise −32020 {range, maxRange: 1024} without admission.
3. Admission.
4. Resolve 'latest', 'safe' and 'finalized' from the snapshot, then check range.
5. Ascending scan.

P1 bound:
- Process block x completely.
- If adding it makes response > RESP_MAX, count > LOGS_COUNT_MAX = 10000, or scan > LOGS_SCAN_BYTES = 32 MiB, return −32020 {reason ∈ {resultBytes, resultCount, scanBytes}, fromBlock, lastCompleteBlock = x−1, nextFromBlock = x}.

Resource bound: deadline or SCRATCH returns −32020 {deadline|scratch, fromBlock, lastCompleteBlock ≥ fromBlock−1, nextFromBlock = lastCompleteBlock+1}, without assuming progress.

Discard the entire partial buffer.

P1 lemma: one block ≤ 32 KiB raw and ≤ 546 logs; its encoding < 400 KiB. Therefore at P1, lastCompleteBlock ≥ fromBlock.

P3 lemma: P1 is raised only at lastCompleteBlock+1 and encoding is deterministic, so no subrange of [fromBlock, lastCompleteBlock] raises P1 on the same chain. It may raise only a resource bound.

blockHash filter = one block.

**(H) pocol_getLogs(filter, anchorHash) (D89)**
- Heavy, with eth_getLogs output, bounds and the same −32020. fromBlock/toBlock are numeric only; tags return −32602.
- Snapshot checks:
  - canonicalHash(height(anchorHash)) ≠ anchorHash returns −32022 {anchorNotCanonical, head}.
  - toBlock > height(anchorHash) returns −32022 {beyondAnchor}.
- P2 lemma: blockHash recursively commits to its parent. If anchorHash is canonical in two snapshots, their canonical blocks ≤ height(anchor) match, so successful chunks with the same anchor belong to one branch.
- No session state; no consensus change.

**(H2) Node origin denial (D95, second defense)**
- ORIGIN_DENY = {pocol_submitShare, pocol_getTemplate}.
- If a request carries Origin with any allowed value and its method is in ORIGIN_DENY, return −32601 at stage 2 without admission or snapshot.
- Apply per item in a batch.
- Local mining programs send no Origin. eth_sendRawTransaction remains allowed for the extension origin because WalletSubmit needs it; BridgeAuth protects it.
- Does not protect against a local process removing Origin (A10); not a validity rule.

**(I) Separation from consensus**
- RPC uses a two-thread tokio runtime and a 20-thread pool. Validation, mining and synchronization use another runtime.
- redb snapshots do not hold the writer.
- Changing RpcGuard, PacerTrace or ORIGIN_DENY does not change validity or require a fork.

**Other custom interfaces**
- pocol_getTemplate, pocol_submitShare (local mining interfaces, denied with Origin).
- pocol_getAssignment(h, a) → {seed, nMax, nonceMode, members[{id, start, end}]}.
- pocol_getMinerSet, pocol_getCaps, pocol_syncStatus.
- pocol_getTiming → {L0, U0, Tj, bj, fsmState}.
- pocol_getTarget(h) → {deltaT, e, s, f, F, target, work, saturated ∈ {none, high, low}}.
- pocol_getParams → {profileKind, nodeKind, nonceMode, chainId, genesisHash, genesisPre, netKey, forkSchedule, activeVersion, rpcLimits}.
- pocol_getPot(j) → {status, amount, kind, winner|null, blockHash, settlesAt}, otherwise −32013. winner = null until block j+1 is imported on the canonical branch (D141).
- pocol_getSettlement(j): decimal wei strings; unsettled returns −32013 {settlesAt}.
- pocol_getBurnTotals.
- pocol_getEscrow → {balance, pendingSum, heldTotal, burnedTotal, surplus}.
- pocol_getRetention → {complete, fromHeight, replicasConfigured, replicasLive, checkedAt} (D119).
  - ReplicaMonitor checks a list of local replicas (including this node) every 10s.
  - A replica is live if it replies within 2s, complete=true, and head ≥ local head−6.
  - Informational, not validity. Trusted in LN, not RP.
- pocol_getClosing → {closing, closeH, cursor, recLen, hEnd, finalAt|null}.
- pocol_getRejects: latest 256 in form {hash, rule, peer, path}.
- pocol_getRpcStats → {conns, heavyActive, heavyQueued, lightActive, lightQueued, heavyResv, lightResv, scratchHeavyMax, scratchLightMax, snapshotsOpen, maxResPerConn, cancelled, cancelMaxMs, busyCount, rateCount, limitCount, anchorRejects, originDenied, pacerClosed, pacerLateMaxMs, lastPacerClose{sendTime, accepted}, sendDoneCount, sendOverrunMaxMs, batchSendMaxS, batchWallMaxS, batchWallHits, traceDropped}.
  - sendOverrunMaxMs = maximum (sendTime at sendDone − (GRACE+size/R)) in milliseconds.
  - originDenied counts H2 responses.

**Local RPC**
- 127.0.0.1; disallowed Host returns 403; Origin absent or chrome-extension://<ID>.
- Body ≤ 1 MiB, batch ≤ 20, eth_call ≤ 30M gas.
- All requests pass RpcGuard; ORIGIN_DENY methods are not served with Origin.

**Errors**
- Standard: −32700, −32600, −32601, −32602, −32603, −32000.
- Custom:
  - −32010 template, −32011 range, −32012 share, −32013 unknown or unsettled.
  - −32014 limit, −32015 type or chainId, −32016 cap or finalized, −32017 outside trie window.
  - −32018 parameters (boot and pocol_getHeaders reasons).
  - −32019 {rule}: client log. rule ∈ items 1–9, net*, viewIncomplete, viewGenesis, viewNoBlocks, viewFuture, viewTargetCeil, recvFit (D100), or gs*.
  - −32020 {reason ∈ {range, resultBytes, resultCount, scanBytes, deadline, scratch, batchBytes}, fromBlock?, lastCompleteBlock?, nextFromBlock?}.
  - −32021 {reason ∈ {busy, rate}, retryAfterMs}.
  - −32022 {reason ∈ {anchorNotCanonical, beyondAnchor}, head}.
- Extension to frame:
  - EIP-1193: 4001, 4100, 4200 {method} (disallowed method, D95), 4901.
  - Bridge-local (D96, D97): −32600 {reason ∈ {size, kind}?}, −32602 {path}, −32005 {reason ∈ {rate, pending, nav, busy, store}} (EIP-1474 code), 4300 {reason: quota, limit} (nonstandard, disclosed), −32603 {reason ∈ {transport, recvLimit, recvDepth, recvParse, storeTimeout}}.

**Trust levels (D13)**
- DEV: anvil.
- LN (default): full verification.
- RP: D77/D82 window with D80, D81 and D84. A remote server may not implement D88–D94 or H2.
  - The extension protects itself through the bridge, BridgeAuth, LogClient, 10s timeout, RecvGuard (D98) and site session (D99).
  - RpcGuard bounds protect the node, not the extension.

A profile and its hash from the same RPC do not establish correctness except in LN.

## Sustainability and value

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

## Upgrades and operation

**Genesis parameters (assumptions, profileKind=network; [CP] = included in CP)**
- Network: chainId=777001, ForkSchedule=[(1,0)].
- Difficulty [CP]: g_ts, target_g, T_blk=60s, τ=3600s, nonceMode=0, c=2.
- Attempts: D_att=300 and Δ_fb_wait=25 [CP]. A_fb=1 and Γ=30 are local.
- Local timing: Δ_rel=26, Φ=10, φ=5, Δ=5, t_exec=3. Derived: σ=14, X^h=22, X^W=36.
- Shares [CP]: m=32, κ=2, S_max=256, D=16, W_w=64, α_bp=1000, γ_bp=500.
- Economics [CP]: subsidy=2·10^18, B_reg=10^20 wei.
- Membership [CP]: R_max=2, M_max=256, M_min=1, REG_RECORDS_MAX=16384, SWEEP_MAX=16, SWEEP_SCAN=64, REG_OPS=16, W_act=128, Θ_inact=1024.
- Evidence [CP]: E_win=2048, U=2200, E_max=4, EVID_BYTES_MAX=8192.
- Block [CP]: GAS_LIMIT=8000000, baseFee0=10^9, TX_MAX=131072, BODY_MAX=196608.
- Caps [CP]: B_code_max=32768, CODE_CAP=2^30, TXBYTES_CAP=2^33, TX_CAP=10^7, SLOT_CAP=4·10^6, ACCT_CAP=10^6.
- SYS [CP]: SYS_SLOT_MAX=131072, SYS_KEYS_BLOCK_MAX=2048, SYS_KEYS_USER_MAX=256, SYS_ACCT_BLOCK_MAX=512, G_SYS=96·10^6.
- Retention (local): K_hist=32, HIST_TRIE_BUDGET=1717986918, R_keep=2048.
- End [CP]: H_END=129600, T_END=g_ts+10368000, H_CLOSE=129440, T_CLOSE=T_END−9600, Z_close=160, RET=128, P2=2.
- Confirmation (local): k_safe=6, k_final=60, k_view=12.
- Extension (local):
  - VIEW_SLACK_S=4, Φ_view=10s.
  - fetchAll: 4 MiB per attempt, 30s, maxRequests=4096, 3 restarts.
  - Single-block retries on RES: 3 times, 500 ms apart.
  - BridgeAuth: BRIDGE_MSG_MAX=64 KiB, JSON depth ≤ 8, method name ≤ 64 B, callObj.gas ≤ 30000000, FEE_HISTORY_BLOCKS_MAX=128, FEE_PERCENTILES_MAX=16 (D100), address ≤ 16, topics ≤ 4×16.
  - Bridge bucket (D97, integers): BUCKET_CAP_MT=50000, BUCKET_REFILL_MT_PER_MS=50, MSG_COST_MT=1000, or 50 messages/s with capacity 50.
  - Pending: PENDING_READS_MAX=4, RpcReadClient transport timeout 10s, fetchAll timeout 30s. One pending external confirmation.
  - Storage/navigation (D96): SITE_STORE_MAX=1048576 bytes, accounting entry = utf8len(k)+utf8len(v); key ≤ 256 B; value ≤ 61440 B; batch interval 250 ms; pathStr and httpsUrl ≤ 2048 B.
  - BridgeTrace: 1 MiB, test build only.
  - StoreQueue (D101):
    - Per session: STORE_SESSION_MSGS_MAX=8, STORE_SESSION_BYTES_MAX=262144.
    - Extension-wide: STORE_GLOBAL_MSGS_MAX=64, STORE_GLOBAL_BYTES_MAX=4194304, STORE_ACTIVE_GLOBAL=2.
    - Timeouts: STORE_QUEUE_WAIT=10s, STORE_WRITE_DEADLINE=5s.
    - Size: STORE_MSG_OVERHEAD=64, STORE_DICT_CACHE_MAX=8 MiB.
    - Loader: ≤ 8 messages and ≤ 256 KiB sent without replies; wait ≤ 2s before navigation.
  - Durable storage (D104):
    - Disk: STORE_DISK_HARD=256 MiB, STORE_RECORDS_MAX=1024, STORE_SITES_MAX=64, SITE_ENTRIES_MAX=4096, RECORD_BYTES_MAX=1442048.
    - Generations: GEN_WINDOW_MAX=4, T_LATE=60s, derived LATE_SITES_PER_GEN=4 (D112).
    - Reserves: LATE_RESERVE=23072768, LATE_NAMES=24, EPOCH_NAMES_MAX=8 (D134), META_RESERVE=1 MiB.
    - Tombstones (D108): ADMIN_SLOTS=2, ADMIN_SLOT_WAIT=5s, TOMB_BYTES_MAX=512, EPOCH_BYTES_MAX=256.
    - Recovery: STORE_RECOVERY_DEADLINE=5s, two anchor attempts, RECOVERY_SLOTS=2, RECOVERY_SLOT_WAIT=5s (D109).
    - Recovery reads (D110): RECOVERY_READ_DEADLINE=5s, RECOVERY_READ_SLOTS=2, RECOVERY_GETS_MAX=4. Derived: gate resolution ≤ 25s, AdminDelete ≤ 45s.
  - RecvGuard (D98):
    - RECV_LIMIT table in browser.
    - RESP_DEPTH_MAX=16, ERR_DATA_MAX=4 KiB, reservation wait 2s.
    - SITE_POOL=32 MiB, SESSION_RECV_MAX=10 MiB, TRUST_POOL=2 MiB, CONTENT_POOL=8·68 KiB.
    - RECV_INFLIGHT_MAX=48.
  - SiteSession (D99):
    - NAV_CAP_MT=6000, NAV_REFILL_MT_PER_MS=1, NAV_COST_MT=2000.
    - GLOBAL_PENDING_READS_MAX=16.
    - SITE_CACHE_MAX=4 MiB+64 KiB, CACHE_GLOBAL_MAX=16 MiB.
  - The matrix and site_* method names are code constants, not configurable parameters.
- RpcGuard (local):
  - Connections: RPC_CONN_MAX=32, REQ_BUF=1 MiB, CONN_BUF=64 KiB, SEND_SOCK_BUF=32 KiB.
  - Permits: HEAVY_ACTIVE=4, HEAVY_QUEUE=16, LIGHT_ACTIVE=16, LIGHT_QUEUE=32.
  - Rate: RATE=20/s, capacity 20; a batch consumes k tokens.
  - Memory: HEAVY_RESP_MEM=48 MiB, LIGHT_RESP_MEM=16 MiB, RESP_MAX=8 MiB, LIGHT_RESP_MAX=512 KiB, BATCH_RESP_MAX=16 MiB, SCRATCH_HEAVY=16 MiB, SCRATCH_LIGHT=1 MiB, RPC_MEM_TOTAL=192 MiB.
  - Batches: BATCH_MAX=20, ERR_ITEM_MAX=256 B, BATCH_BYTES_HARD=16782400 B.
  - Logs: LOGS_RANGE_MAX=1024, LOGS_COUNT_MAX=10000, LOGS_SCAN_BYTES=32 MiB.
  - Timeouts: REQ_READ_DEADLINE=10s, IDLE=30s (from sendDone), QUEUE_WAIT=2s, COMPUTE_DEADLINE_HEAVY=5s, COMPUTE_DEADLINE_LIGHT=500 ms, SendPacer with GRACE=2s and MIN_SEND_RATE=65536 B/s, BATCH_WALL=420s, CANCEL_MAX=100 ms.
  - Enforcement: ENFORCE_SLACK=200 ms, TIMER_EPS=1 ms.
  - Tracing: rpc.pacerTrace (false by default, true in RG), 4 MiB buffer.
  - Origin denial: ORIGIN_DENY = {pocol_submitShare, pocol_getTemplate}, fixed in code.
  - Derived (never manually configured; all until sendDone): SEND_MAX(size) = GRACE+size/R+ENFORCE_SLACK; BATCH_SEND_MAX = 258.0791015625s with monitoring bound 258.2791015625s; minimum BATCH_WALL = 404.2791015625s.
- Envelope retention (D136, local, nonconsensus): ENV_REJ_PER_BLOCK=8, ENV_REJ_GLOBAL=16384, ENV_ENTRY_MAX=128 B, ENV_PENDING_GLOBAL=1024, ENV_INFLIGHT_GLOBAL=64 = ENV_SOLICIT_SLOTS=32 + ENV_GOSSIP_SLOTS=32, ENV_REQ_TIMEOUT=2s, ENV_REPLY_MAX=3131 B, ENV_MEM_MAX=7 MiB (D137), ENV_CHECK_WORKERS=4, ENV_EXEC_MAX=50 ms (D139, replacing ENV_CHECK_MAX in D138). Derived c_env = (⌈63/ENV_CHECK_WORKERS⌉+1)·ENV_EXEC_MAX = 850 ms; build constraint ENV_CHECK_WORKERS ≥ 1.
  - ENV_EXEC_MAX is a measured assumption for one check's execution time; c_env derives from it (D139). Exceeding it increments envCheckOverrun and raises an alert without releasing the seat before settlement. Safety takes precedence over liveness.
  - Build constraint: ENV_SOLICIT_SLOTS+ENV_GOSSIP_SLOTS = ENV_INFLIGHT_GLOBAL; both ≥ 1.
  - Memory constraint: ENV_REJ_GLOBAL·ENV_ENTRY_MAX + ENV_PENDING_GLOBAL·3067 + ENV_INFLIGHT_GLOBAL·ENV_REPLY_MAX + 16384·64 ≤ ENV_MEM_MAX.
  - Monitoring adds solicitHeld, envTimeouts, liveChecks, liveBuffers, cancelPending, lateCheckDropped and envCheckOverrun (D138). Any liveChecks > 64 or liveBuffers > 64 is an implementation defect blocking release. Changing these requires a node release, not a fork. Monitor envFloodDropped, envEvicted, envRejEntries; alert on envEvicted > 1000 per minute.
- Derived constants (D86): LAG=2048 (outside CP), Lmax = 4113.

**Boot gate R1–R26**

ParamGate.check(profile, measured, nodeKind) returns ok, an ordered list of every violated rule, or UnknownField. A non-ok result on a real node refuses boot with −32018.

Timing:
- R1: Φ=2φ.
- R2: Δ_rel ≥ σ+Δ+2t_exec.
- R3: Δ_fb_wait ≥ Φ+Δ+2t_exec.
- R4: Φ < Δ_fb_wait, Δ_rel ≥ t_exec.
- R5: 3Δ_fb_wait ≤ D_att.
- R6: X^h and X^W.
- R7: D_att+Γ ≥ 2(X^W+c·T_blk)+Φ.

Evidence and sizes:
- R8: D ≥ 2, U ≥ E_win+D, E_win ≤ R_keep.
- R9: S_max ≥ 4κm, B_code_max ≥ 24577.
- R10: TX_MAX ≤ BODY_MAX−16 KiB.
- R11: k_final ≤ R_keep, k_view < K_hist, k_view+2 ≤ 512.

Membership and end:
- R12: M_max ≥ |M_0| ≥ M_min ≥ 1.
- R13: H_END > Z_close, H_CLOSE+Z_close ≤ H_END, T_CLOSE ≤ T_END.

Performance and resources:
- R14: t_exec ≥ validate_p99.
- R15: gossip limits.
- R16: REG_RECORDS_MAX ≥ M_max+R_max·(Lmax+LAG).
- R17: SWEEP_MAX ≥ R_max+E_max.
- R18: SYS, including the cost of RET records within G_SYS.
- R19: block times ≤ t_exec.
- R20: Z_close ≥ ⌈REG_RECORDS_MAX/RET⌉+P2+D+1.
- R21: historical trie; the only exception is simnet with retentionStress.

Profiles and identity:
- R22: profileKind, nodeKind and chainId separation.
- R23: Σalloc+|M_0|·B_reg+subsidy·H_END < 2^127.
- R24: (A) Decode GenesisSpec; match genesisHash, allocRoot and sysCodeHash. (B) ForkSchedule nonempty, starting at (1,0), ascending. (C) Every v supported. (D) TestFork-v2 only in simnet. (E) chainId ≠ 0, fits u64.
- R25: (A) 1 ≤ target_g ≤ 2^256−1. (B) τ ≥ 1 and T_blk ≥ 1. (C) In network: τ ≥ 10·T_blk. (D) target_g calibration recorded.

RPC service (local, not affecting consensus), R26:
- (A) HEAVY_RESP_MEM ≥ RESP_MAX, LIGHT_RESP_MEM ≥ RPC_CONN_MAX·LIGHT_RESP_MAX.
- (B) HEAVY_RESP_MEM+LIGHT_RESP_MEM+HEAVY_ACTIVE·SCRATCH_HEAVY+LIGHT_ACTIVE·SCRATCH_LIGHT+RPC_CONN_MAX·(REQ_BUF+CONN_BUF) ≤ RPC_MEM_TOTAL.
- (C) BATCH_RESP_MAX ≥ RESP_MAX, LIGHT_QUEUE ≥ RPC_CONN_MAX, BATCH_MAX·ERR_ITEM_MAX+64 B ≤ CONN_BUF.
- (D) RESP_MAX ≥ 2·LOG_BLOCK_MAX+546·512 B, LOGS_COUNT_MAX ≥ 546, LOGS_SCAN_BYTES ≥ LOG_BLOCK_MAX.
- (E) LIGHT_RESP_MAX ≥ greatest light value (385 KiB).
- (F) SEND_MAX and BATCH_SEND_MAX are formula-only; BATCH_WALL ≥ BATCH_MAX·(QUEUE_WAIT+ENFORCE_SLACK+COMPUTE_DEADLINE_HEAVY+CANCEL_MAX)+BATCH_SEND_MAX+ENFORCE_SLACK. RATE ≥ 1, capacity ≥ BATCH_MAX.
- (G) 0 < TIMER_EPS < ENFORCE_SLACK ≤ 500 ms, SEND_SOCK_BUF ≤ 64 KiB.

Default checks: (A) 48 ≥ 8, 16 ≥ 16. (B) 178 ≤ 192. (C) 16 ≥ 8, 32 ≥ 32, 5184 B ≤ 64 KiB. (D) 8 MiB ≥ 337 KiB. (E) 512 ≥ 385. (F) 420 ≥ 404.2791015625, 20 ≥ 20. (G) 1 < 200 ≤ 500, 32 ≤ 64.

Network checks: R16 = 12578 ≤ 16384, R20: 160 ≥ 147, R13: 129440+160 = 129600.

Bridge constants are outside ParamGate because they belong to the extension, not pocold. BG1 unit tests check:
- BUCKET_CAP_MT = 50·MSG_COST_MT, BUCKET_REFILL_MT_PER_MS·1000 = 50·MSG_COST_MT, PENDING_READS_MAX ≥ 1.
- SITE_STORE_MAX ≥ 17·(3+61440)+4045, exactly ≥ 1048576, preserving BR17e's numerical derivation. Literal value 1048576.
- BRIDGE_MSG_MAX ≥ 61440+256+128, accommodating the maximum storage_set key/value lengths and envelope.
- RECV (D100): each exact method requires RECV_LIMIT ≥ WorstLegit for CP values in network, sv-fix, end/full and logs/dense, with bridge-schema limits (FEE_HISTORY_BLOCKS_MAX, FEE_PERCENTILES_MAX, count ≤ 14).
- The capped set is literally {eth_call, eth_estimateGas}; each has a product-limit row in browser.
- RECV_LIMIT need not exceed RpcGuard's general heavy-method bound. This distinction is intentional and disclosed.
- SESSION_RECV_MAX ≥ 8 MiB, SITE_POOL ≥ SESSION_RECV_MAX, TRUST_POOL ≥ 2·96 KiB.
- NAV_CAP_MT = 3·NAV_COST_MT.
- GLOBAL_PENDING_READS_MAX ≥ PENDING_READS_MAX.
- SITE_CACHE_MAX ≥ 4 MiB+64 KiB.
- (D101) STORE_SESSION_BYTES_MAX ≥ 64+256+61440 = 61760, admitting the largest message into an empty session.
- (D101) STORE_GLOBAL_BYTES_MAX ≥ STORE_GLOBAL_MSGS_MAX·61760 (3952640 ≤ 4194304), STORE_GLOBAL_MSGS_MAX ≥ STORE_SESSION_MSGS_MAX ≥ 1, STORE_ACTIVE_GLOBAL ≥ 1, STORE_QUEUE_WAIT > STORE_WRITE_DEADLINE.
- (D104) Disk constraints:
  - STORE_DISK_HARD ≥ STORE_SITES_MAX·2·RECORD_BYTES_MAX + LATE_RESERVE + META_RESERVE, namely 184582144+23072768+1048576 = 208703488 ≤ 268435456.
  - STORE_RECORDS_MAX ≥ 2·STORE_SITES_MAX + LATE_NAMES + EPOCH_NAMES_MAX + 1, namely 128+24+8+1 = 161 ≤ 1024 (D134).
  - (D108, D109) LATE_NAMES = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS+ADMIN_SLOTS); EPOCH_NAMES_MAX ≥ GEN_WINDOW_MAX+2 so ordinary batches do not close storage (D134); ADMIN_SLOTS ≥ 2; RECOVERY_SLOTS ≥ 2; RECOVERY_SLOT_WAIT ≤ STORE_RECOVERY_DEADLINE.
  - (D110) RECOVERY_READ_SLOTS ≥ 1, RECOVERY_GETS_MAX ≥ 2, 0 < RECOVERY_READ_DEADLINE < STORE_QUEUE_WAIT. Derived AdminDelete bound = RECOVERY_READ_DEADLINE+2·(RECOVERY_SLOT_WAIT+STORE_RECOVERY_DEADLINE)+2·(ADMIN_SLOT_WAIT+STORE_RECOVERY_DEADLINE) = 45s, displayed literally by the build tool.
  - (D108) META_RESERVE ≥ STORE_RECORDS_MAX·TOMB_BYTES_MAX + (GEN_WINDOW_MAX+1)·ADMIN_SLOTS·TOMB_BYTES_MAX + EPOCH_NAMES_MAX·EPOCH_BYTES_MAX, namely 531456 ≤ 1048576 (D134).
  - RECORD_BYTES_MAX = 4·ceil((SITE_STORE_MAX+8·SITE_ENTRIES_MAX)/3)+256, LATE_RESERVE = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS)·RECORD_BYTES_MAX.
  - (D112) LATE_SITES_PER_GEN = STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS, STORE_SITES_MAX ≥ GEN_WINDOW_MAX·LATE_SITES_PER_GEN+1, namely 64 ≥ 17. The build tool literally displays the minimum immediately admissible sites after the worst generation window (64−16 = 48).
- (I55) The first condition makes the global byte limit defensively redundant under defaults, as disclosed. TQ parameters in BR21e intentionally violate it and are permitted only in test builds. Any release build with nondefault StoreQueue values fails.

Any violation fails the extension build.

**Experimental profiles**

P11 (dev): T_blk=10, τ=600, c=2, Δ=1, t_exec=1, φ=1, Φ=2, Δ_rel=8, Δ_fb_wait=6, Γ=5, D_att=60, chainId=777901. Derived: σ=4, X^h=6, X^W=10.

anvil-br (phase A, DEV): anvil with chainId=777910 behind RpcTap, keys 7–10 funded. Outside ParamGate because it is not pocold.

sv-fix (simnet):
- Values: P11 with nonceMode=1, subsidy=80000 wei, target_g=2^240, chainId=777902, g_ts=1700000000.
- M_0 = {w, x, y, z}, keys 3–6, sorted; rewardAddr = id.
- End: H_END=200, H_CLOSE=40, Z_close=160, T_END=g_ts+10^7, T_CLOSE=T_END. Remaining CP uses network values.
- alloc: s1..s4 (keys 7–10, 10^18 each), Rv (code 0x5f5ffd), Sd (code 0x73‹Escrow›ff, balance 5000).
- R16 = 12578, R13: 40+160 = 200, R20: 160 ≥ 147. On node, returns [R22]. CP matches GSV1.

end/full (simnet, D86):
- Values: sv-fix with chainId=777905, R_max=0, M_max=1024, M_min=1, REG_RECORDS_MAX=1024, RET=8, H_END=700, Z_close=147, H_CLOSE=553, T_END=g_ts+10^7, T_CLOSE=T_END.
- M_0: 1024 identities: w, x, y, z and 1020 identities idᵢ = last 20 bytes of keccak256('PoColEnd'‖be32(i)) for i ∈ [0, 1019], without private keys. rewardAddr = id; list sorted. Any duplicate or reserved-address match gives FixtureSetupError('ids').
- alloc: as sv-fix, with s1 = 3·10^20.
- Gate: R12: 1024 ≥ 1024 ≥ 1; R16: 1024 ≥ 1024; R17: 16 ≥ 4; R13: 553+147 = 700; R20: 147 ≥ 147; R23 holds. R8, R9 and R11 from network; R1–R7 from P11; R18 with 8 records/block; R21 with H_END=700; default R26. Result ok on simnet, [R22] on node.

logs/dense (simnet):
- Values: sv-fix with chainId=777906, H_END=1300, H_CLOSE=1140, Z_close=160.
- alloc adds:
  - LogBomb: 546 LOG0 logs per call, 24-byte data with final 8 bytes = be64(arg).
  - MemHog: expands memory to the 30M-gas limit and reads 10^4 cold slots.
  - Spin: loops until gas exhaustion.
  - Blob: blob(uint256 n) returns n bytes of 0xab, n ≤ 4 MiB; response 2n+fixed header.
  - s2 = 10^20.
- R13: 1140+160 = 1300, R20: 160 ≥ 147. Result ok on simnet, [R22] on node.

Others: sv-fix-v2 (ForkSchedule=[(1,0),(2,50)], chainId=777904, node returns [R22, R24]); caps/64 (777903, defined in the VA annex under T18b); P15; P51; P55.

**Genesis (TM8, [PDF:96])**
- M_0 = 4 HELD identities.
- target_g = min(2^256−1, max(1, floor(2^256/(H_cal·T_blk))−1)).
- The genesis tool produces genesisPre; three keccak libraries hash it; signing occurs only when all agree.

**Upgrades (D72)**
- Hard fork = a new binary adding (v, H_v), with a signed announcement ≥ 1440 blocks beforehand.
- Nonupdated nodes reject with netVersion, causing a deliberate, announced split.
- Changing CP or specVersion means a new network.
- Fork choice (D133) is a consensus rule:
  - It does not change validity, but differing versions may select different heads on ties.
  - Changes require a coordinated release and signed announcement as for an upgrade; they are not RPC or local-retention changes.
- Nonconsensus changes do not affect validity and are announced through web3_clientVersion and extension versions. These include RPC, extension, RpcGuard, LogClient, PacerTrace, BridgeAuth, bridge envelope/bucket/pending counter, SiteStorage quota accounting and ORIGIN_DENY (D81, D82, D84, D87–D97).
- D83 and D86–D97 do not change consensus outcomes.
- Adding a bridge-matrix method requires its schema row in the unified envelope, read/write/local classification, BR and Python-reference updates, and review.
- A method changing network state, broadcasting or signing never belongs to rpc_read. Any new local method carries the site_ prefix and does not belong to rpc_read.
- Changing bucket or pending constants requires joint updates to BR19, BridgeRef and BG1. Changing SITE_STORE_MAX or key/value bounds requires rederiving BR17e, SiteStorageRef and BG1 together.

**Procedures**
1. Closing and end, then a signed snapshot.
2. Cap reached: freeze and alert.
3. Emergency: signed announcement at height ≥ 1440.
4. Halt: signed restart statement (disclosed centralization).
5. Rollback deeper than R_keep: manual checkpoint.
6. Network revocation: update networks.json.
7. K_eff < k_view: alert.
8. RecLen > 0.9·REG_RECORDS_MAX: alert.
9. X8 failure: withdraw extension.
10. Burn-monitor violation: block release.
11. simnet on node: [R22].
12. FixtureSetupError is not a test result.
13. > 10 item-1b or templateForm rejections per hour: alert.
14. Mean interval over 100 blocks outside [0.5, 2]·T_blk: alert.
15. New network at h=0 longer than 10·T_blk: alert.
16. > 50 −32018 errors per minute for pocol_getHeaders from one connection: alert and close.
17. s ≥ VIEW_SLACK_S in a canonical header: alert that RP will not display.
18. closing without cursorDone before H_END−P2−D: alert (impossible under R20).
19. target_g·2^VIEW_SLACK_S ≥ 2^256 in a published profile: alert that the RP bound is ineffective.
20. RPC pressure: alert when −32021 > 20% for 5 minutes, heavyResv ≥ 0.9·HEAVY_RESP_MEM for 60s, conns = 32 for 60s, or pacerClosed > 100 per minute.
21. Release-blocking implementation defect on any of:
    - snapshotsOpen > 20, maxResPerConn > 1, scratch above its bound, lightResv > LIGHT_RESP_MEM.
    - cancelMaxMs > 100, pacerLateMaxMs > 200, sendOverrunMaxMs > 200.
    - batchSendMaxS > 258.2791015625, batchWallHits > 0.
    - Reservation not released at sendDone.
    - TC-send, CloseOK or Enforced violation in any recorded trace.

    Thresholds = exact values + ENFORCE_SLACK, without rounding.
22. anchorRejects > 10 per minute: frequent-reorganization alert.
23. Any BR or BG1 vector failure, forbidden request reaching RpcTap/RpcRecorder, local-storage change or tab opening from a rejected message, bridge/quota decision disagreeing with BridgeRef/SiteStorageRef, or dependency-cruiser rule violation: block extension release and withdraw distributed versions.
24. originDenied > 0 on a production node: alert, because the extension origin must not request mining methods.
25. (D103, D104) Local alert with a “Manage storage” link for any extension condition: storeRecoveryFail > 0, epochGateBlocks > 0 (D134), recoveryCorrupt > 0, diskRejects > 0, removeFailures > 0, inUse ≥ 0.9·STORE_DISK_HARD. Record values in T12. Any BR22a criterion violation blocks extension release.

**Keys**
- Genesis, checkpoint, publication, restart and networks.json use Shamir 2-of-3.
- Identity key separate from rewardAddr.
- Multisig for the site owner.
- Fixture keys 1–10 are test-only, including the presigned transaction in BadSite.

**Monitoring (Prometheus)**
FSM, T_j, rejections by rule, ASERT (s, saturated), Escrow, pendingSum, burns, RecLen, SYS, closing/cursor, K_eff, disk, memory, activeVersion, −32018 reasons, and all pocol_getRpcStats metrics including originDenied. Extension: local counters for BridgeAuth rejection by code/method, rate/pending/quota rejections, and accepted/rejected external-navigation confirmations, without external transmission.

**M0 output**
- versions.lock, cargo-deny, license-checker, Windows tools, canary.
- Three keccak libraries (Rust sha3, TS @noble/hashes, Python pycryptodome) with licenses.
- ruint or crypto-bigint, hyper (MIT), socket2 (MIT/Apache), dependency-cruiser (MIT).
- rpc-fault absent from release builds; three injected faults: lightInflate, pacerOff, perItemGrace. PacerTrace is not an injected fault.
- Test-only exec-fault build, absent from release, with four injected faults:
  - winnerInState (D141), detected by TW1.
  - sharesFromFirstSeen, sharesFromMiner, sharesDropped (D142), detected by TW5.
- Test-only fixture-fault build, absent from release, with one injected fault: fixedShareThreshold (D143). TW5's fixture assertion detects it deterministically.
- Test-only bridge-fault build, absent from release, with thirty-two injected faults:
  - allowAll disables B2, demonstrating BR detects it.
  - legacyLocal accepts legacy {key, value} and {path}, demonstrating BR18-L detects it.
  - noRefill disables bucket refill, demonstrating BR19a detects it.
  - quotaNoSubtract computes newTotal without subtracting the old entry on replacement, demonstrating BR17e(e7) detects it.
  - frameBudget creates the bucket and both counters per frame, ignoring orphaned requests, demonstrating BR20a and BR20c detect it.
  - recvNoLimit collects the whole body before checking, demonstrating MR1 and MR4 detect it.
  - storeUnbounded (D101) disables StoreQueue limits, timeouts and teardown cancellation, demonstrating BR21a, BR21b and BR21e detect it.
  - storeEarlyRelease (D102) releases active-message resources on the storeTimeout reply and again on settlement, demonstrating BR21a(q8) and BR21b(r6) detect it.
  - quotaNoRelease (I56) takes a seat before quota evaluation and replies 4300 without release, demonstrating BR21f and (f2) detect it.
  - singleItem (D103) overwrites a single item per key, demonstrating BR22a detects it.
  - noEpochConfirm writes data before epoch confirmation, demonstrating BR22a(P4) and BR22b detect it.
  - noCheckpoint serves snapshot without an anchor, demonstrating BR22a detects it.
  - reuseSeq reuses seq after promise failure, demonstrating BR22a(fail) detects it.
  - parallelRecovery (D104) creates a recovery gate per tab, demonstrating BR22d detects it.
  - seqFromCheckpoint derives write seq from the anchor's nominal 0, demonstrating BR22d detects it.
  - sweepMax permits deleting the greatest record, demonstrating BR22a and BR23 detect it.
  - noDiskGate disables disk admission, demonstrating BR23(d2, d6) detects it.
  - diskNoReserve (D105) checks without reserving and drops the ticket on settle, demonstrating BR23(d8–d11) detects it.
  - deleteUnordered (D106) issues the tombstone before closing sessions, demonstrating BR24(x1) detects it.
  - readyOnApply (D107) makes the key ready on apply, demonstrating BR22d(V-a) detects it.
  - adminEarlyRelease (D108) releases the tombstone seat on timeout, demonstrating BR24(x9) detects it.
  - recoveryNoSlot (D109) issues the second anchor attempt without a seat, demonstrating BR22f detects it.
  - lateReadCheckpoint (D110) accepts a read result after its gate expires and issues an anchor from it, demonstrating BR22g detects it.
  - adminNoCancel (D111) disables only seat-request withdrawal at resolution, demonstrating the guard suffices in BR24(x12b) and BR22f-f5. Both tests must pass with it.
  - adminStaleIssue (D111) disables both withdrawal and guard, demonstrating BR24(x12) detects it.
  - noLateSites (D112) sets LATE_SITES = 0, demonstrating BR23(d12) detects it.
  - seatOnCounted (D113) requests a site seat for an operation on a tombstone-counted key, demonstrating BR23(d15) detects it.
  - dropPinned (D113, I68) drops a pinned key when measurement does not see it, demonstrating BR23(d15c) detects it: direct assertion at 30050, X admission at 60100 after LATE_SITES=0, and count 65 at 60350.
  - epochNonceNames (D134, I74) restores D103's old per-boot nonce naming without the name gate, demonstrating BR22h(h1) detects it with 41 names.
  - epochNoGate (D134) uses the shared name without the name gate, demonstrating BR22h(h2) detects it with 41 names.
  - epochSweepEarly (D134) sweeps smaller names immediately after confirmation without waiting T_LATE, demonstrating BR22h(h5) detects it with epochResurrected > 0.
  - epochReserveOmitted (D135, I64) omits EPOCH_NAMES_MAX from name admission, demonstrating BR23(d11 and d11b) detects it.

## Validation plan

**Status**
All tests are proposed; none has been run, and no success is recorded before execution. Outputs are JSON traces with status ∈ {pass, fail, inconclusive, blocked}.

**Types**
- G: deterministic; any violation is a failure.
- S: 24 seeds bᵢ = SHA256('PoColSeed'‖be32(i)), with a 95% bootstrap CI, then 48 seeds, then “inconclusive”.
- T13: 12 seeds with an exact sign test and Holm correction [PDF:123].
- C (conditional): measurement on a real socket or browser; its condition is computed from the measured trace throughout the run, not one instant. If satisfied, a violation is a failure; otherwise the result is “inconclusive”, with values recorded after 3 repetitions.

**Gates**
All G, T4, T7, T7b, T7c, T7e, T8, T9, T10b(i, ii, iii, vi), T19, T18b, T27, X8, BR, BG1, MR, NX, RW, HC, AS, ASQ, ASB, L4, HG, PG-END, PG-RPC, CL-END, LC, PS and RG.

**Measurements**
- T10, T10b(iv, v), T10c, T13, baseBurn/R and time of F.
- viewTargetCeil rate, RG9, totalRequests in RG3, B_meas and pacerLateMaxMs.
- Closure times in RG14, and receiveDone−sendDone and unreadAtSendDone on sockets.
- Arrival-interval distribution in BR19c.

**ASERT (G)**

Input (p.ts−g_ts, p.h, T_blk, τ), then trace (Δt, e, s, f, F), then target. target_g=2^240 unless otherwise stated.
- AS1: (0,0,10,600) → (0,0,0,0,65536) → 2^240.
- AS2: (700,10,10,600) → (600,65536,1,0,65536) → 2^241.
- AS3: (400,100,10,600) → (−600,−65536,−1,0,65536) → 2^239.
- AS4: (400,10,10,600) → (300,32768,0,32768,92674) → 92674·2^224.
- AS5: (700,100,10,600) → (−300,−32768,−1,32768,92674) → 46337·2^224.
- AS6: (11,1,10,65536) → (1,1,0,1,65537) → 65537·2^224.
- AS7: (9,1,10,65536) → (−1,−1,−1,65535,131071) → 131071·2^223.
- AS8: (65545,1,10,65536) → (65535,65535,0,65535,131071) → 131071·2^224.
- AS9: (34463,10000,10,65536) → (−65537,−65537,−2,65535,131071) → 131071·2^222.
- AS10: (9,1,10,65537) → e=−1 → 131071·2^223. Truncation yields 2^240 and fails.
- AS11: (10300,10,10,600) → s=17 → 2^256−1.
- AS12: (55400,20000,10,600) → s=−241 → 1.
- AS13: target_g=2^256−1, (0,0,10,600) → 2^256−1 without overflow.
- AS14: (2^62, 7, 60, 3600) → s ≥ 256 → 2^256−1 through step 4.
- AS15: (0, 2^64−1, 2^32−1, 1) → 1 through step 4.
- AS16: target_g=1, (153000,0,10,600) → s=255 and f=0 → 2^255.
- AS17: target_g=1, (153600,0,10,600) → s=256 → 2^256−1.
- AS18: target_g=2^256−1, (0,15420,10,600) → s=−257 → 1.
- AS19: target_g=2^256−1, (0,15360,10,600) → s=−256 and k=−272 → 0 → 1.
- AS-eq: an unbounded reference (Python or TS BigInt) matches AS1–AS19 and 10^6 random inputs with s ∈ [−300, 300].
- ASB: Rust without heap allocation (allocator counter), TS with intermediates ≤ 512 bits; each of AS14–AS19 takes ≤ 1 ms and ≤ 10 times AS1.
- F values are derived manually and confirmed by two implementations against the reference; any discrepancy stops M3.
- ASQ: relative error ≤ 2·10^−4; F and target(e) are nondecreasing.

**T4-B (S)**: mean interval over 3000 blocks within [0.9, 1.1]·T_blk.

**T10c (measurement)**: extreme timestamps.

**T0 (D73)**
cfg_B = {777001, 0x11×32, [(1,0)], nonceMode=1}, with parent h=0 at ts=1700000000.
- L0: accept V1–V6 and V3a. Reject V7–V15: overlong h, trailing byte, high s, v=0x1b, seven-byte nonce, target=0x80, non-strict shareList or one containing nonce, 19-byte proposer, and 17 items.
- L1 (firstReject / diagSet): V3a netChain/{netChain}, V3b netChain/{netChain, 3}, V16–V19 3/{3}, V23 netGenesis, V24 and V25 netVersion, V26 netChain.
- L2: V1, V5 and V6 with a 2^28 search bound; V6 length 3067.
- L3: V20–V22 give item 12.

**L4 (D79)**
- K1: keccak256(empty) = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470.
- K2: keccak256(0xc0) = 1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347.
- K3: keccak256(0x80) = 56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421.

GSV1 (341 bytes, non-bootable):
- Header: f9 01 52 | 01 | 83 0b de ae.
- CP: f8 b1, then:
  - 1–10: 84 65 53 f1 00 | 9f 01 ‹00×30› | 0a | 82 02 58 | 01 | 02 | 3c | 06 | 20 | 02.
  - 11–17: 82 01 00 | 10 | 40 | 82 03 e8 | 82 01 f4 | 83 01 38 80 | 89 05 6b c7 5e 2d 63 10 00 00.
  - 18–30: 02 | 82 01 00 | 01 | 82 40 00 | 10 | 40 | 10 | 81 80 | 82 04 00 | 82 08 00 | 82 08 98 | 04 | 82 20 00.
  - 31–40: 83 7a 12 00 | 84 3b 9a ca 00 | 83 02 00 00 | 83 03 00 00 | 82 80 00 | 84 40 00 00 00 | 85 02 00 00 00 00 | 83 98 96 80 | 83 3d 09 00 | 83 0f 42 40.
  - 41–52: 83 02 00 00 | 82 08 00 | 82 01 00 | 82 02 00 | 84 05 b8 d8 00 | 81 c8 | 84 65 ec 87 80 | 28 | 84 65 ec 87 80 | 81 a0 | 81 80 | 02.
- Tail: a0 ‹22×32› | f8 56 ea 94 ‹a1×20› 94 ‹b1×20› ea 94 ‹a2×20› 94 ‹b2×20› | a0 ‹33×32›.
- Lengths: CP 179, M_0List 88, payload 338.

Acceptance: (i) literal match in Rust and TS. (ii) Decode then encode returns identical bytes. (iii) H_GSV1 is recorded by agreement of three libraries after K1–K3; any discrepancy stops M3. (iv) It is then frozen.

L4n: N1 remove item 52 → gsCount. N2 item 5 = 02 → gsRange. N3 item 3 = 81 0a → gsInt. N4 swap the two entries → gsOrder. N5 duplicate entry → gsOrder. N6 19-byte rewardAddr → gsLen. N7 specVersion=02 → gsVersion. N8 item 2 = 80 → gsRange. N9 trailing byte → L0. N10 id=…C0C001 → gsSys. N11 different genesisHash → profile rejection and R24(A).

L4s: CP in sv-fix matches GSV1 and GB1; allocRoot is recorded by Rust/anvil agreement.

**PG-END (G)**
- PG-END1: network with H_CLOSE=129584 → [R13].
- PG-END2: Z_close=146 and H_CLOSE=H_END−146 → [R20].
- PG-END3: network → ok.
- PG-END4: end/full on simnet → ok; on node → [R22] alone.
- PG-END5: end/full with R_max=2 → [R16]; with Z_close=146 and H_CLOSE=554 → [R20].
- PG-L: Lmax = 4113 in network and end/full; R16 right-hand side = 12578.

**PG-RPC (G)**
Each of the following gives [R26]: HEAVY_RESP_MEM=4 MiB, LIGHT_RESP_MEM=8 MiB, RPC_CONN_MAX=64 (violates A, B and C), LOGS_COUNT_MAX=500, LIGHT_RESP_MAX=256 KiB, BATCH_WALL=400s or 404s, bucket capacity 10, ERR_ITEM_MAX=4 KiB, ENFORCE_SLACK=0 or 600 ms, TIMER_EPS=0 or 200 ms, or SEND_SOCK_BUF=128 KiB.

BATCH_WALL=405s → ok. Defaults → ok; derive displays 258.0791015625, 258.2791015625 and 404.2791015625 literally.

**BG1 (G, bridge constants)**: defaults pass. BUCKET_CAP_MT=49999, MSG_COST_MT=0, PENDING_READS_MAX=0, SITE_STORE_MAX=1048575 or BRIDGE_MSG_MAX=61823 fail the build.

D100 vectors:
- Fail the build:
  - RECV(eth_getCode)=64 KiB, because 65536 < 66560.
  - RECV(eth_getBlockByHash)=160 KiB, because 163840 < 163958.
  - FEE_HISTORY_BLOCKS_MAX=256, because the worst case is 335567 > 180224.
  - Adding eth_getCode to capped without a producer-bound row.
- Pass: eth_call at 1 MiB, because it is capped.

**ExecFixture**
SV1–SV7s, CL1–CL4, CI1–CI3, GB1 and GB-END.
- SV9-exec: h=30 with [tx1, tx2, tx3] → statuses 1/0/1, gasUsed 21000, 21004 and 26003, blockTips=173015, surplus +5000.
- SV8-exec: blockTips=84000.

**FixtureMiner(profile, plan)**
For every h:
1. Build_fb or a template signed by Draw(h,0)'s key, otherwise 'draw'.
2. target ≥ 2^236, otherwise 'target'.
3. Scan R(winner) with bound 2^28, otherwise 'search'.
4. shareList=[] and winnerSig.
5. Submit; unexpected rejection is 'reject'.

sv-fix rule: h ∈ [1,199] are fallback blocks with empty bodies, ts +7, winner=w, pot=80000, closeH=40 and F=200.

Transactions:
- tx1: type-2 from s1 to s2, value=1, gas=21000, tip 4, maxFee=2·10^9, chainId=777902, nonce 0.
- tx2: s3 to Rv, gas=50000, tip 3.
- tx3: s4 to Sd, gas=50000, tip 1.

FixtureVerify: diagnose → {first, violated, notEvaluable}, and diagnoseChain. A match passes; otherwise “blocked”.

**Phase A (G)**
- T1: nonpublisher reverts. createVersion then publish then setCurrent succeeds. Revoking current reverts. Transfer without acceptance does not change owner. Duplicate store returns the same address.
- T11a: modifying one byte displays “tampering” without a frame.
- T16: manifest vectors.
- T12 (gate): 12 sites with X1–X7, DNR-d1–DNR-d5 and Q1–Q12; zero transactions without clicks. Sites: relative css, folders, classic scripts, modules with importmap, inline, localStorage (through BR17), fetch JSON, woff2, images, form, SPA with internal links (through BR18), and contract reads/writes, with BR14 as a legitimate path. Record bridge messages rejected by rate for each site; criterion: zero rate rejections for a site reading/writing at a normal load rate.
- X8: CanarySink on TCP 18080, 18443 and 13478, UDP 13478 and 19000; channels C1–C8. Flags: K0–K2 zero arrivals, K3 informational, K4 control, K5 zero for C5–C7.
- RW-unit (h → b, from, count, n, anchoredGenesis): RW1 1 → (1,1,1,1,yes). RW2 3 → (1,1,3,3,yes). RW3 13 → (1,1,13,13,yes). RW4 14 → (2,1,14,13,no). RW5 20 → (8,7,14,13,no). RW6 2^64−1 → (2^64−13, 2^64−14, 14, 13, no) without overflow. Exhaustive for h ∈ [1, 10^6]: count ≤ 14, n ≤ 13, count = n+[b>1].
- HC-unit: HC1 target_g=2^240 and target=2^244 → accepted. HC2 target=2^244+1 → viewTargetCeil. HC3 target_g=2^256−1 → ceilTarget=2^256−1 without overflow. HC4 target_g=2^240 → minWork=4095; message contains '4095', not '4095.9' or '/16'. HC5 target_g=2^256−1 → minWork=1 with “work bound ineffective”. HC6 target_g=2^252 → ceilTarget=2^256−1 and minWork=1. HC7 target_g=1 → ceilTarget=16 and minWork=floor(2^256/17). Exhaustive: 10^4 random values; for every target ≤ ceilTarget, work(target) ≥ minWork, checked with BigInt.

**BR (G, bridge authorization, envelope, rate and quota, D95, D96, D97)**

Setup:
- Phase A on anvil-br (chainId=777910) behind RpcTap, a local HTTP proxy recording each request forwarded to anvil in order, with source, arrival and response times; supports hold(method, ms) to delay responses.
- Extension installed in test Chrome using the test build with BridgeTrace; BadSite published through the factory at S_A; GoodSite is a second site at S_B.
- BadSite embeds txA: type-2 from key 7 (s1) to s2, value 1, nonce 0, chainId=777910, signed outside the browser by the test tool.
- TestHooks records approval/confirmation dialogs, tab openings and SiteStorage snapshots.
- Every message uses envelope {id, kind, payload}; shorthand kind {method, params} below means payload = {method, params}, with id increasing from 1. M1-spec contains each literal message.
- To isolate BR1–BR18 from rate effects: each case uses a fresh session opened by TestHooks (full bucket), fewer than 50 messages, and BadSite awaits each reply before the next message unless stated otherwise. Any unexpected rate/pending rejection makes the case “blocked”, not passed, and records a fixture error.

BridgeRef (D97): an independent Python reference written from the text, taking BridgeTrace (arrival times) and RpcTap logs (response times), returning each message's expected decision: bpre (pass/size/rate), stage/code, routing and pending decision. The bucket uses the same integer arithmetic; pendingReads is computed from routing time and the recorded final-response time. Includes SiteStorageRef: a reference dictionary per (netKey, address), applying the literal entry/newTotal accounting in browser, returning decision (null or 4300) and total after every message.

Common criterion for every rejection case:
- (a) Exact specified error (code, data and returned id), matching BridgeRef.
- (b) Zero RpcTap requests caused by the message, including zero eth_sendRawTransaction from any source except WalletSubmit.
- (c) After mining 3 blocks with the test tool, not the site: eth_getTransactionCount(s1, latest) = 0; s2 balance unchanged.
- (d) Zero open approval/confirmation dialogs and zero new tabs. Only BR18 is excepted, expecting exactly one confirmation.
- (e) SiteStorage snapshot for every key is bytewise unchanged.
- (f) The frame keeps working and a subsequent legitimate read succeeds.

Cases:
- BR1: rpc_read {method: eth_sendRawTransaction, params: [txA]} → 4200.
- BR2: rpc_read eth_sendTransaction → 4200, without approval.
- BR3: rpc_read pocol_submitShare then pocol_getTemplate → 4200 each.
- BR4: malformed names → 4200, including 'ETH_SENDRAWTRANSACTION', 'eth_call ' (space), 'eth_call\u0000', 'eth_getBalance' with a Cyrillic lookalike, a 65-byte name, and 'Site_storageSet'.
- BR5: array payload [{eth_blockNumber}, {eth_sendRawTransaction}] → −32600 for the whole message; zero requests even for the first member.
- BR6: extra payload fields {method: eth_call, params: [...], jsonrpc: '2.0', id: 7}, an extra outer field beside {id, kind, payload}, or duplicate method in raw text → −32600.
- BR7 (kind substitution and B0–B3 precedence):
  - BR7a: wallet_req {method: eth_getBalance, params: [addr, 'latest']} → 4200 (B2).
  - BR7b: wallet_req {method: eth_sendRawTransaction, params: [txA]} → 4200 (B2).
  - BR7c: storage_set {method: eth_call} without params → −32600 (B0).
  - BR7d: storage_set {method: eth_call, params: []} → 4200 {method: 'eth_call'} (B2).
  - BR7e: storage_set {method: site_storageSet, params: ['k', 5]} → −32602 {path: 'params[1]'} (B3).
  - BR7f: kind='rpc' with {method: eth_blockNumber, params: []} → −32600 {reason: kind} (B1).
  - BR7g: rpc_read {method: site_storageSet, params: ['k', 'v']} → 4200; rpc_read {method: site_navigate, params: ['/']} → 4200.
  - BR7h: nav {method: eth_call, params: [{to: addr}, 'latest']} → 4200.
- BR8: eth_call with third parameter {stateOverride} → −32602 {path: 'params[2]'}.
- BR9: eth_call with gas=30000001, no to, or both data and input → −32602.
- BR10: rpc_read eth_getLogs over 1..100: RpcTap sees only pocol_getLogs and eth_getBlockByNumber, zero raw eth_getLogs. Range 1..1025 → −32020 {range} without any request. A filter with 17 addresses → −32602. On anvil: −32601 for pocol_getLogs gives Error{unsupported}, recorded.
- BR11: method ∈ {'__proto__', 'constructor', 'hasOwnProperty', 'toString'} in every kind → 4200; zero worker exceptions.
- BR12: eth_getBlockByNumber [latest, true] → −32602. tag='pending', 'earliest' or {blockHash} → −32602.
- BR13: payload depth 9 → −32600. Message size 64 KiB+1 → −32600 {reason: size}, id = null, without parsing or token consumption (tokensBefore on the next BridgeTrace row equals tokensBefore on this row). Invalid JSON → −32600, id = null. Negative, 2^32 or string id → −32600, id = null. All without requests.
- BR14 (legitimate path): wallet_req eth_requestAccounts, click approval, then wallet_req eth_sendTransaction from the granted account to s2 for value 1, then click approval. Expected: exactly one eth_sendRawTransaction in RpcTap, source WalletSubmit, keccak256(raw) = displayed txHash, transaction included after mining. Rejection variant: 4001, zero eth_sendRawTransaction. Ungranted from variant: 4100.
- BR15 (phase B, pocold LN on sv-fix, in M7):
  - Repeat BR1, BR3 and BR5 with RpcRecorder instead of RpcTap: zero requests; mempool, ShareLedger and stateRoot bytewise unchanged.
  - Direct HTTP request from the test tool, Origin: chrome-extension://<ID>, to pocol_submitShare and pocol_getTemplate → −32601 from h2; originDenied increases, zero acceptance.
  - Same request without Origin → served normally (positive control).
  - BR14 on pocold → transaction inclusion.
- BR16 (fuzz):
  - 10^4 random messages from a generator seeded with bᵢ: names from the complete Ethereum/PoCol/anvil/site_* method lists with mutations, random kind inside/outside the set, random params, structural envelope mutations, including storage_set values near quota boundaries.
  - BadSite sends at intervals ≥ 25 ms in batches of 20, awaiting replies so rate does not dominate; the criterion does not assume absence of rate rejections.
  - Criterion:
    - Every response code matches BridgeRef for the same message and recorded arrival times, including Bpre–B4 order and SiteStorageRef quota decision.
    - Every RpcTap request name belongs to the read array and its parameters pass SchemaRef; zero prohibited requests; request set = messages BridgeRef expects to route.
    - SiteStorage changes arise only from BridgeRef-accepted messages; final dictionary matches SiteStorageRef byte for byte.
    - Zero opened tabs (TestHooks automatically rejects every confirmation); every schema-valid site_openExternal gives 4001.
    - No BridgeTrace overflow; otherwise inconclusive.
- BR17 (local storage, D96). Initial state for each vector: empty S_A storage unless stated otherwise:
  - BR17a success: storage_set {method: site_storageSet, params: ['k', 'v']} → result null. Within ≤ 500 ms, the highest version record for 'site:'+netKey+':'+S_A (D103) contains k='v'. After site reload, localStorage.getItem('k') returns 'v'. Zero RpcTap requests.
  - BR17b delete: after BR17a, {method: site_storageSet, params: ['k', null]} → null; k absent.
  - BR17c clear: keys a and b, then {method: site_storageClear, params: []} → null; storage empty.
  - BR17d schema: params [''] or ['', 'v'] → −32602 (path 'params' for the first, 'params[0]' for the second). 257-byte UTF-8 key → −32602 {path: 'params[0]'}. 61441-byte value → −32602 {path: 'params[1]'}. ['k', 'v', 'x'] → −32602 {path: 'params'}. [1, 'v'] → −32602 {path: 'params[0]'}. ['\ud800', 'v'] (lone surrogate) → −32602 {path: 'params[0]'}. site_storageClear with [1] → −32602 {path: 'params'}.
  - BR17e quota (I50). Literal accounting: entry(k, v) = utf8len(k)+utf8len(v), total = Σentry; accept when newTotal ≤ 1048576; rejection is 4300 {reason: 'quota', limit: 1048576}. Fresh session, 24 messages (< 50), awaiting each reply. Each value is ASCII 'a', 'b' or 'x', so JSON value length = its length + 2; largest message = envelope ≤ 128 bytes + 61440 + 3 ≤ 61571 bytes < BRIDGE_MSG_MAX = 65536. After each message, record total from BridgeTrace (storeTotalBefore/storeTotalAfter), matching SiteStorageRef. Table (message → newTotal → reply → subsequent total):
    - (e1) 17 site_storageSet messages with keys k00..k16 (3 bytes each) and 61440-byte 'a' values. Message i ∈ [1, 17] → newTotal = 61443·i ≤ 1044531 → null, total = 61443·i. After the seventeenth message, total = 1044531, remaining 4045.
    - (e2) Message 18: ['k17', 61440×'a'] → newTotal = 1044531+61443 = 1105974 > 1048576 → 4300. total = 1044531; k17 absent; dictionary byte snapshot equals its pre-message snapshot.
    - (e3) Message 19: ['k17', 4043×'a'] → newTotal = 1044531+4046 = 1048577 → 4300, no bytewise change.
    - (e4) Message 20: ['k17', 4042×'a'] → newTotal = 1044531+4045 = 1048576 → null. total = exactly 1048576.
    - (e5) Message 21: ['k18', 'x'] → newTotal = 1048576+4 = 1048580 → 4300, unchanged, k18 absent.
    - (e6) Message 22: ['k17', 4043×'a'] (replacement) → newTotal = 1048576 − 4045 + 4046 = 1048577 → 4300. k17 remains 4042 bytes, bytewise unchanged.
    - (e7) Message 23: ['k00', 'b'] (shrinking replacement) → newTotal = 1048576 − 61443 + 4 = 987137 → null.
    - (e8) Message 24: ['k01', null] (deletion) → newTotal = 987137 − 61443 = 925694 → null. Deletion is not rejected by quota.
    - (e9) After site reload: localStorage contains k00 = 'b', k02..k16 with 61440-byte 'a' values, k17 with 4042-byte 'a', and neither k01 nor k18. Accounted total from the dictionary read from chrome.storage.local = 925694.
    - Criterion for every 4300: zero RpcTap requests, zero chrome.storage.local.set calls (recorded by TestHooks), byte-identical pre/post-message dictionary snapshots; BridgeTrace record has storeTotalAfter = storeTotalBefore.
    - Discrimination: (e2), (e3), (e5) and (e6) detect missing atomicity. (e4) detects strict (<) instead of (≤). (e7) detects failure to subtract old entry; bridge-fault:quotaNoSubtract must fail it (returns 4300 because 1048576+4 > limit). (e6) detects replacement accounting using the value difference without the key or vice versa.
    - M1-spec contains this literal table with totals before/after every message and the literal twenty-four messages.
  - BR17f isolation: GoodSite (S_B) on the same network cannot see S_A's k in localStorage; S_A on another network profile (different netKey) cannot see it either. S_B quota is independent: after S_A fills at BR17e(e4), ['k', 'v'] succeeds on S_B.
  - BR17g legacy form: storage_set payload {key: 'k', value: 'v'} → −32600, unchanged.
- BR18 (navigation, D96):
  - BR18a internal: nav {method: site_navigate, params: ['/about.html']} on a site containing about.html → old frame destroyed, new frame displays about.html under D26; zero tabs, zero RPC requests except worker chunk fetching.
  - BR18b: ['/x/'] → displays /x/index.html; ['/'] → entry.
  - BR18c: ['/a/../b.html'], ['/a%2fb'] and ['//x'] pass schema; normalization gives a viewer 404 page without replying to the frame.
  - BR18d: ['about.html'] (no '/') → −32602 {path: 'params[0]'}. ['/a b'] (space) → −32602. Path length 2049 → −32602.
  - BR18e allowed external: {method: site_openExternal, params: ['https://example.org/']} → one confirmation displaying the full URL. TestHooks clicks “Open” → one new tab without opener; response null. No network request from site context.
  - BR18f rejected external: same message, TestHooks clicks “Cancel” → 4001, zero tabs.
  - BR18g: ['http://example.org/'], ['javascript:alert(1)'], ['chrome-extension://<ID>/x'], ['https://user:pw@example.org/'] and ['https://'] → −32602 {path: 'params[0]'}, without confirmation.
  - BR18h: pending confirmation, then second site_openExternal → −32005 {reason: 'pending'} for the second; one confirmation remains.
  - BR18-L legacy form: nav payload {path: '/a'} or {external: 'https://example.org/'} → −32600, zero confirmations.
- BR19 (rate and pending, D97). Three separate layers:
  - BR19a (G, unit, Bpre bucket alone): BridgeAuth.check in Node with FakeClock, fresh session created at 1000 ms; m = {id: i, kind: 'rpc_read', payload: {method: 'eth_blockNumber', params: []}}. check runs Bpre–B3 only, with no routing/connection; result is “check passed”, not RPC service. Vectors in order (time: messages → result, subsequent tokens_mt):
    - (a1) 1000: messages 1..50 → ok, 0. Messages 51..60 → −32005 {reason: 'rate'}, id = null, 0.
    - (a2) 1019: message 61 → rate, tokens_mt = 950.
    - (a3) 1020: message 62 → ok, 0.
    - (a4) 3000: tokens = min(50000, 0+50·1980) = 50000. Messages 63..112 → ok; message 113 → rate.
    - (a5) 5000: 50 invalid-JSON messages → −32600, id = null each; next valid message → rate, because B0 rejections consumed their tokens.
    - (a6) 7000: 50 messages of size 64 KiB+1 → −32600 {size}, then 50 valid messages → ok, then the fifty-first → rate, because size rejection consumes nothing.
    - (a7) Each BridgeTrace record has tokensBefore matching the table.
    - Negative control bridge-fault:noRefill: must fail (a3).
  - BR19b (G, unit, pending alone): RpcReadClient with FakeTransport holding responses until manually released, FakeClock, a frame at pendingReads = 0, inputs that passed check.
    - (b1) Five valid eth_call messages simultaneously → first four reach FakeTransport, fifth −32005 {reason: 'pending'} without sending. pendingReads = 4.
    - (b2) Release first reply → frame receives it, pendingReads = 3. Sixth message → reaches transport, pendingReads = 4.
    - (b3) Advance clock 10000 ms without responses → four pending requests receive −32603 {reason: 'transport'}, pendingReads = 0.
    - (b4) eth_getLogs through LogClient.fetchAll with a holding MockLogServer → one pending read regardless of internal LogClient requests; three following eth_call reach transport, fourth pending.
    - (b5) Destroy frame with 4 pending requests (D99):
      - Zero replies to the destroyed frame.
      - New frame in the same session sees pendingReads = 4, so its first read gets −32005 {pending}.
      - FakeTransport.settle for one orphan makes pendingReads = 3, so the next read reaches transport.
      - No orphan response reaches any frame.
  - BR19c (C with G matching, integrated Chrome on anvil-br):
    - BadSite sends 60 eth_blockNumber messages in one synchronous loop without waiting, then after 2000 ms sends 5 eth_call messages to s2 with RpcTap.hold('eth_call', 3000).
    - Matching (always G): each BridgeTrace decision equals BridgeRef computed from arrivalMs and recorded response times; RpcTap request set exactly equals routed messages, with zero requests for rejected messages.
    - Decisiveness condition: if BridgeRef expects at least one rate rejection among the sixty and at least one pending rejection among the five, the case is decisive. Otherwise, after 3 repetitions it is “inconclusive”, with recorded arrival intervals; logic is covered by BR19a and BR19b.
    - No fixed count such as 50/10 is expected, because refill depends on actual arrival times.
- BR-neg (negative controls): bridge-fault:allowAll disables B2 and RpcReadClient checking and must fail BR1 (eth_sendRawTransaction appears in RpcTap and txA is included). bridge-fault:legacyLocal must fail BR17g and BR18-L. bridge-fault:noRefill must fail BR19a(a3). bridge-fault:quotaNoSubtract must fail BR17e(e7). bridge-fault:storeUnbounded must fail BR21a(q3, q5), BR21b(r2) and BR21e(t=5). bridge-fault:storeEarlyRelease must fail BR21a(q8) and BR21b(r6). bridge-fault:quotaNoRelease must fail BR21f at t=100 and in (f2). D103 controls (singleItem, noEpochConfirm, noCheckpoint, reuseSeq) must fail BR22a in its specified cases. These builds are absent from release.
- BR20 (self-navigation and site session, D99):
  - BR20a (G, unit, FakeClock/FakeTransport; session created at 1000 with tokens_mt=50000, nav=6000, pending=0).
    - Each message has two separate columns:
      - check: Bpre–B3 result (pass or rate).
      - route: B4 result, namely forwarded, −32005 {pending}, or nav.
    - Table (messages → check / route, then subsequent tokens_mt and pending):
      - 1000, F1, m1..m4 eth_call → pass / forwarded. Then 46000, pending=4.
      - 1000, F1, m5..m49 eth_blockNumber → pass / pending. Then 1000, pending=4.
      - 1000, F1, m50 site_navigate('/') → pass / nav. Then 0, nav=4000. Four requests become orphaned; F2 joins the session.
      - 1001, F2, m51 eth_blockNumber → rate (id=null) / —. Then 50.
      - 1020, F2, m52 eth_blockNumber → pass / pending. Then 0.
      - 1040, F2, m53 eth_call → pass / pending. Then 0.
      - 1041, FakeTransport.settle(orphan 1) → pending=3, zero replies to any frame.
      - 1060, F2, m54 eth_call → pass / forwarded. Then 0, pending=4.
      - 1061, F2, m55 eth_blockNumber → rate / —. Then 50.
    - Negative control bridge-fault:frameBudget must fail at m51: it gives F2 a new bucket and pending=0, making m51 pass / forwarded.
    - BridgeRef matches every table cell.
  - BR20b (G, unit):
    - Three site_navigate calls at 0 → three navigations.
    - Fourth at 0 → −32005 {nav}, without teardown.
    - At 2000 → navigation. At 2001 → nav.
  - BR20c (integrated Chrome on anvil-br, RpcTap.hold('eth_call', 3000)):
    - Scenario: BadSite startup page sends 4 eth_call then site_navigate('/') immediately on load, for 60s.
    - G criteria, from RpcTap/BridgeTrace against session-based BridgeRef:
      - Maximum overlapping eth_call requests from the session ≤ 4.
      - Messages accepted by t ≤ 50+50·t.
      - Navigations performed by t ≤ 3+t/2.
      - Chunk eth_getCode requests equal the site's chunk count, once only.
      - Every decision matches BridgeRef.
      - Session reservation ≤ 10 MiB in TestHooks.recvStats.
    - C criteria:
      - Worker ping p99 ≤ 100 ms.
      - Closing the tab ends the session and releases reservations within ≤ 1s after orphan requests settle.
      - Extension-process memory ≤ baseline+64 MiB by Windows counters.
      - If measurement is unavailable, result is “inconclusive”, with recorded values.
  - BR20d: five tabs for the same site, each sending 4 held eth_call:
    - 16 requests arrive; others get −32005 {pending}.
    - Trust checking in a sixth tab succeeds through TRUST_POOL.
- BR21 (storage queue, D101). BridgeTrace adds these fields for each storage_set:
  - storeQBefore{sessN, sessBytes, globN, globBytes}.
  - storeRoute ∈ {queued, store, cancelled, active, done}.
  - storeViolated ⊆ {sessN, sessBytes, globN, globBytes} on rejection.
  - reply/release events for each message with subsequent counters (D102).

  Common G criterion for all BR21:
  - Each accepted message has exactly one release and at most one reply.
  - Counters on every row equal the sum of unreleased messages in BridgeRef.
  - No negative counter.
  - globQ ≥ sessQ for every session.
  - BR21a (G, Node unit with FakeClock and FakeStorage holding every set until release(i) or fail(i), session S1 with a full bucket and empty key K). Maximum message M(kNN) = ['kNN', 61440×'a'], qbytes = 61507; small m(kNN) = ['kNN', 'x'], qbytes = 68. The writer does not await replies.
    - (q1) t=0: M(k00) becomes active, one set call, sess = (1, 61507).
    - (q2) t=1..3: M(k01..k03) accepted waiting, sess = (4, 246028), zero additional set.
    - (q3) t=4: M(k04) gives 307535 > 262144, hence −32005 {store}, not enqueued.
    - (q4) t=5..8: m(k90..k93) accepted, sess = (8, 246300).
    - (q5) t=9: m(k94) gives n = 9 > 8, hence −32005 {store}.
    - (q6) release(1), then successive releases: set order = k00, k01, k02, k03, k90..k93; one call each with cumulative dictionary. Replies null in order. Counters return to (0, 0). Final dictionary = SiteStorageRef.
    - (q7) fail(1): k00 gives −32603 {transport}, followed by reload, then k01 starts. Zero bytewise change from k00.
    - (q8) Holding beyond timeout (I54, D102). Start at the end of (q5), without (q6)/(q7): k00 active, k01..k03 accepted at 1..3, k90..k93 at 5..8, sess = (8, 246300). Table (time: event → reply, then sess (n, bytes)):
      - 5000: k00 write expires → −32603 {storeTimeout} for k00. Counters unchanged (8, 246300); k00 remains active.
      - 6000: m(k95) → −32005 {store}, storeViolated = {sessN}, because 9 > 8 and k00 is still counted. Counters (8, 246300).
      - 10001, 10002, 10003: waits for k01, k02, k03 expire → −32005 {store} each. Counters (5, 61779).
      - 10005..10008: waits for k90..k93 expire → store each. Counters (1, 61507).
      - 10010: m(k96) → accepted waiting, because the key is blocked. Counters (2, 61575), zero second set call.
      - 12000: release(1), i.e. k00 settles → zero reply because already answered, then reload and total = 61443, then one release. Counters (1, 68). k96 then becomes active; second set is called.
      - 12100: release(2) → null for k96, total = 61447, counters (0, 0).
      - Criterion: k00 has exactly one reply and one release.
      - Negative control bridge-fault:storeEarlyRelease releases on timeout reply, yielding (7, 184793) at 5000. It then accepts m(k95) at 6000 and subtracts k00 again at 12000. Must fail at 6000.
    - (q9) Global: eight sessions with different keys, 8 small messages each, giving globN = 64. First message from a ninth session → −32005 {store}. Concurrent set calls never exceed exactly 2 at any instant.
    - (q10) Resolving the count boundary with defaults (I55).
      - Setup: sixteen sessions S1..S16 for different sites, each with a storage key. FakeStorage holds every set. Clock does not advance past t=64 ms, so STORE_QUEUE_WAIT/STORE_WRITE_DEADLINE do not expire.
      - Sending: at t = 4(j−1)+i, j ∈ 1..16 and i ∈ 0..3, Sj sends M(k_j_i), qbytes = 61507.
      - Session bounds: after every message Sj ≤ (4, 246028), within (8, 262144); no violation.
      - States: first messages of S1/S2 active, occupying both slots. First messages of S3..S16 ready. Others queued.
      - At t=63: glob = (64, 3936448).
      - At t=64: S17 sends M(k) → globN = 65 > 64, while globBytes = 3997955 ≤ 4194304 and S17 = (1, 61507), within its bounds. Result −32005 {store}, storeViolated = {globN} only, without enqueueing. glob stays (64, 3936448); exactly 2 set calls.
      - Negative control: storeUnbounded accepts S17's message and fails.
      - Global byte boundary is tested separately in BR21e.
  - BR21b (G, unit, teardown/navigation):
    - (r1) F1 has active held k00 and waiting k01..k03.
    - (r2) site_navigate from F1 at 100: k01..k03 cancelled without reply/set; sess immediately becomes (1, 61507). New frame receives no snapshot until k00 settles or 5100.
    - (r3) release(1) at 300: zero reply to any frame; F2 snapshot contains k00; counters (0, 0).
    - (r4) Variant without release: at 5100 F2 loads confirmed state with the badge; k00 remains counted until settlement.
    - (r5) Tab closure applies the same rule; session deleted after k00 settles.
    - (r6) Teardown after timeout (D102). State: F1 has active held k00, waiting k01, sess = (2, 123014).
      - At 5000: storeTimeout for k00 without release; counters (2, 123014).
      - At 5100: site_navigate. k01 cancelled (suppressed), counters (1, 61507), k00 unchanged. Navigator waits until 10100.
      - At 7000: release(1). Zero reply, one release → (0, 0). F2 snapshot at 7000 contains k00.
      - Variant: no release by 10100 → F2 loads confirmed state with the badge. k00 remains counted (1, 61507) until settlement, then one release.
      - Negative control storeEarlyRelease gives (1, 61507) at 5000, then (0, 0) at 5100, and fails.
  - BR21c (C with G matching, Chrome on anvil-br): BadSite sends M(k·) for 60s at the bucket's maximum allowed rate without awaiting replies.
    - G: every decision matches BridgeRef extended with StoreQueue; sess.n ≤ 8 and sess.bytes ≤ 262144 on every row; zero RpcTap requests; final dictionary = SiteStorageRef.
    - C: worker ping p99 ≤ 100 ms, extension-process memory ≤ baseline+32 MiB, each set completes within 5s. If measurement unavailable, “inconclusive” with recorded values.
  - BR21d (G, loader): a site calls setItem 10^4 times over 50 keys within 10s.
    - Unanswered loader messages ≤ 8 at every instant.
    - Zero unrecovered store/rate rejections.
    - Confirmed dictionary after settlement = latest value for every key.
    - setItem exceeding 1 MiB locally throws QuotaExceededError without any message.
    - An internal link following a write preserves that write before navigation.
  - BR21e (G, unit, global byte boundary, I55):
    - Setup: StoreQueue built with test parameters TQ = {sessMsgs 8, sessBytes 262144, globMsgs 64, globBytes 300000, active 2, wait 10000, write 5000}.
    - TQ deliberately violates BG1 (globBytes ≥ globMsgs·61760) to make the byte boundary reachable. Accepted only in test builds; BG1 rejects release builds with nondefault values.
    - FakeStorage holds every set. S1, S2 and S3 are sessions for different sites.
    - Table (time: message → reply, then glob (n, bytes), then storeViolated):
      - t=0..3: S1 M(k00..k03) → accepted, glob (4, 246028), S1 (4, 246028).
      - t=4: S2 ['k50', 53905×'a'], qbytes = 64+3+53905 = 53972 → accepted, glob (5, 300000) = exact limit.
      - t=5: S2 ['k51', 'x'], qbytes = 68 → 300068 > 300000 → −32005 {store}, storeViolated = {globBytes} only. S2 (2, 54040) within its bounds, globN = 6 ≤ 64.
      - t=6: S3 ['k60', null], qbytes = 67 → 300067 → store with {globBytes} only.
      - t=100: release(1), i.e. k00 settles → null, glob (4, 238493).
      - t=101: S2 ['k51', 'x'] → accepted, glob (5, 238561).
    - Criterion: BridgeRef match in every cell; literal storeViolated match.
    - Negative control storeUnbounded must accept the message at t=5 and fail.
  - BR21f (G, unit, quota rejection before set, I56):
    - Setup: defaults. FakeStorage holds every set until settle(n).
      - S_A dictionary loaded with BR17e(e4) state, total = 1048576.
      - S_B and S_C empty.
      - Message qbytes: m0 = ['k17', 4042×'a'] → 4109. m1 = ['k18', 'x'] → 68. m2 = ['k01', null] → 67. m3 = S_B ['k', 'v'] → 66. m4 = S_C ['k', 'v'] → 66.
    - Table (time: event → reply, then S_A and glob as (n, bytes), then set count):
      - t=0: m0 → newTotal = 1048576 (same-size replacement, equality accepted) → ready then active. S_A (1, 4109), glob (1, 4109), set=1.
      - t=1: m1 → queued behind active. S_A (2, 4177).
      - t=2: m2 → queued. S_A (3, 4244), glob (3, 4244).
      - t=3: m3 → active in second slot. glob (4, 4310), set=2.
      - t=4: m4 → ready without a slot. glob (5, 4376).
      - t=100: settle(1):
        - m0 replies null and releases. S_A (2, 135).
        - evalHead(m1): 1048580 > 1048576 → 4300, then release, zero set. S_A (1, 67).
        - evalHead(m2): deletion → ready.
        - Free slot goes to m2 before m4 because accepted earlier. set=3, glob (3, 199).
      - t=200: settle(2) → null for m3, then m4 active, set=4, glob (2, 133).
      - t=300: settle(3) → null for m2. total(S_A) = 987133, S_A (0, 0).
      - t=400: settle(4) → null for m4. glob (0, 0).
      - t=500: S_A ['k18', 'x'] → newTotal = 987137 → active, set=5. At t=600 it settles → null, total = 987137.
    - Criterion:
      - m1: one reply (4300), one release.
      - k18 appears in no set before set=5.
      - releaseCount = 6 = accepted count; doubleReleaseCount = 0; quotaRecheckMismatch = 0.
      - Counters return to (0, 0).
      - Final dictionary = SiteStorageRef.
    - (f2) Rejection does not wait for a slot:
      - Setup: S_B/S_C active and held, occupying both slots. S_A has no active write, total = 1048576.
      - t=0: S_A ['k18', 'x'] → immediate 4300, release, zero set.
      - t=1: S_A ['k01', null] → ready, waiting until a slot releases.
    - Negative control: bridge-fault:quotaNoRelease leaves S_A = (2, 135) after t=100 and blocks m2, failing. In (f2), it waits for a slot and fails.
- BR22 (worker-generation recovery, D103):
  - BR22a (G, exhaustive model). SR-model is a Python reference, matched by TS StoreQueue/StoreRecovery over FakeBackend. FakeBackend holds every generation's operations, applying them at scheduler-selected points or dropping them.
    - Initial state: tabs T1/T2 for S_A; k0='a' confirmed with null. Generation 1 accepts A = ['k1', 'x'] from T1.
    - Disappearance point P:
      - P0: before set.
      - P1: after set call, before application.
      - P2: after application, before reply.
      - P3: after storeTimeout reply.
      - P4: second disappearance during generation 2 epoch write.
      - P5: second disappearance during anchor.
    - Late-application point L for A and other pending operations:
      - Not applied.
      - Before epoch enumeration.
      - Between confirmation and key read.
      - Between read and anchor.
      - After anchor.
      - After write B.
    - After disappearance: reload T1/T2 by clicking, then T2 writes B = ['k2', 'y'], then T1 writes C = ['k1', 'z']; each gets null.
    - Enumerate all combinations, including a third disappearance before any new-generation write. Add a fail variant: a rejected promise that applies later.
    - Criteria:
      - (1) k0='a' in every snapshot and final state.
      - (2) Final state always contains k1='z' and k2='y'; A never supersedes B/C.
      - (3) k1 in the new generation's first snapshot is either 'x' or absent; it stays identical in every later snapshot until C, across any further disappearances.
      - (4) Zero replies to previous-generation messages, zero duplicate replies.
      - (5) Epochs of generations that wrote data are strictly increasing.
      - (6) No snapshot before anchor commitment.
      - (7) Per key after sweeping: largest record always exists; disk by FakeBackend's metric ≤ D104 limit at every instant.
    - Negative controls:
      - singleItem fails at P1 with L = after write B.
      - noEpochConfirm fails at P4.
      - noCheckpoint fails at P1 with L = between read/anchor, then third disappearance, because k1='x' appears after a snapshot lacking it.
      - reuseSeq fails in the fail variant.
  - BR22b (G, unit, fence):
    - Generation 1 reads max=5, writes pending 6, then disappears.
    - Generation 2 reads 5, writes 6' and confirms it.
    - Generation 1's item applies afterward.
    - Generation 3 reads 6 and writes 7.
    - Criterion: zero data records with generation 1's epoch; sweeping does not delete the largest epoch item.
  - BR22c (C with G matching, test Chrome):
    - Setup: DevTools port on 127.0.0.1 in the test profile. Tool stops the worker through CDP at times targeting P0–P3 during BadSite writes from two tabs.
    - G:
      - Storage record read through CDP matches SR-model for the classified point.
      - Frames are destroyed and notice appears within ≤ 1s of onDisconnect.
      - Zero automatic reloads.
      - Dictionary after clicking = SR-model.
    - C: if P1/P2 is not hit within 20 attempts, result is “inconclusive” for that point, with recorded distribution. Logic covered by BR22a.
    - Also measure whether Chrome applies dead-worker operations late and their delay (A15c), recovery time, getKeys availability, and getBytesInUse versus storage-file size.
  - BR22d (G, unit, D104): overlapping recoveries and re-anchoring.
    - Setup: FakeBackend holds every operation until settle or late apply. Generation 2. Key K for S_A has record (1,3), dictionary {k0:'a'}.
    - Table (time: event → effect, set count):
      - 0: T1 requests snapshot → recovering, set C0 = (2,0). set=1.
      - 1: T2 sends ['k1','x'] and T2 requests snapshot → both await the same gate. set=1, zero second anchor.
      - 5000: C0 timeout → issue C1 = (2,1); C0 remains pending and counted. set=2.
      - 5100: settle(C1) succeeds → ready at version (2,1). T1/T2 snapshots = {k0:'a'}. Then W = (2,2), dictionary {k0:'a', k1:'x'}. set=3.
      - 5200: settle(W) → null to T2, sweep (1,3)/(2,1).
      - 5300: C0 applies late → lateRecordsSeen=1; next sweep deletes it.
      - 5400: snapshot for new frame = {k0:'a', k1:'x'}.
    - Criterion: distinct set names, no snapshot before 5100, k1='x' in every state after 5200.
    - Variants (D107, apply ≠ settle):
  - V-a: apply(C0) at 5050 without settle → same base table; ready at 5100 with (2,1).
  - V-b: settle(C0) succeeds at 5050:
    - ready at 5050, confirmed=(2,0), T1/T2 snapshots = {k0:'a'} at 5050.
    - W=(2,2) issued at 5050.
    - settle(C1) succeeds at 5100 → confirmed=(2,1), dictionary unchanged.
    - settle(W) at 5200 → null.
  - V-c: base table, then settle(C0) succeeds at 5250 → unchanged, lateRecordsSeen=1.
  - V-d: settle(C1) fails at 5100 with C0 timed out:
    - failed at 5100; T2 message gets −32603 {storeRecovery}.
    - Successful settle(C0) at 5150 does not change failed, increments lateRecordsSeen. Record (2,0) remains largest and is recovered by the next generation.
  - V-e: no attempt settles → failed at 10000 when C1 times out.
  - Every variant: no snapshot before first successful settle; k1='x' after settle(W).
    - Failure variant: C1 fails at 5100 → failed; T2 message gets −32603 {storeRecovery} and releases; zero snapshots.
    - Negative controls:
      - parallelRecovery issues (2,0) twice at 1 and fails.
      - seqFromCheckpoint gives W = (2,1), colliding with C1, and fails.
  - BR22e (G, seq exhaustion): SeqAlloc starts at 2^32−2 through a test parameter.
    - Anchor takes 0xfffffffe; first write takes 0xffffffff and replies null.
    - Second write → −32603 {storeRecovery}, no set.
  - BR22f (G, unit, RecoverySlots contention, D109, I62):
    - Setup: generation 2; keys K1/K2/K3 for different sites, each with record (1,3) and nonempty dictionary. FakeBackend holds every operation. Each key has a waiting storage_set message.
    - Table (time: event → effect, then held/set count):
      - 0: K1 snapshot → C0_K1 = (2,0) in slot A. held=1, set=1.
      - 1: K2 snapshot → C0_K2 = (2,0) in slot B. held=2, set=2.
      - 2: K3 snapshot → waits in RecoveryFIFO, zero alloc.
      - 5000: C0_K1 timeout → request C1_K1 behind K3.
      - 5001: C0_K2 timeout → request C1_K2.
      - 5002: K3 wait expires → unrecovered; message −32603 {storeRecovery, busy}, release; zero set/alloc for K3. recoveryBusy=1.
      - 10000: C1_K1 wait expires → K1 failed. 10001: K2 failed. held=2, set=2 unchanged.
      - 13000: generation disappears with C0_K1/C0_K2 pending, then their late application at all L points defined in BR22a.
    - Variant (f2): settle(C0_K1) fails at 7000 → slot A goes to C1_K1 = (2,1) at 7000, deadline 12000. K2 failed at 10001. K1 resolves ≤ 12000.
    - Variant (f3): settle(C0_K1) succeeds at 6000 → K1 ready, C1_K1 request withdrawn; slot A goes to C1_K2 at 6000.
    - Variant (f4): deleteSite(K1) at 3000 → tombstone not issued before K1 resolves at 10000, then uses AdminSlots.
    - Criteria: recoveryUnsettledMax ≤ 2 on every row; no anchor without a slot; every slot wait ≤ 5000 ms; every resolution ≤ 20s from first request; no snapshot before successful settle. After disappearance: disk/names within bounds at every instant; next recovery = SR-model.
    - Negative control recoveryNoSlot issues C1_K1/C1_K2 at 5000/5001, producing 4 unsettled operations and failing.
  - BR22g (G, unit, read phase, D110, I63):
    - Setup: generation 2. FakeBackend holds getKeys/get until settleRead(i). Keys K1/K2/K3 for different sites, each with a waiting storage_set message. t_s = 0 unless stated otherwise.
    - (g1) Held getKeys:
      - 0: K1 snapshot causes getKeys to take slot R1.
      - 5000: readTimeout. K1 returns to unrecovered; message gets −32603 {storeRecovery, readTimeout} and releases. Zero alloc, zero set, zero disk ticket.
      - 6000: settleRead(getKeys). Result ignored, lateReadsDropped=1, R1 releases, zero set.
      - 6100: new K1 arrival opens a gate with a new gateId.
    - (g2) getKeys immediately succeeds; get of largest record held. At 5000 readTimeout. get result at 7000 issues no anchor.
    - (g3) K1/K2 reads held in R1/R2. K3 waits in ReadFIFO, then readTimeout at 5000 without any read. readSlotsHeld = 2 on every row.
    - (g4) Corrupt records:
      - Five records for the key, four largest corrupt: failed {corrupt} after exactly 4 get calls; fifth not read.
      - Variant: three records, all corrupt → failed {corrupt}.
      - Variant: zero records → anchor dictionary {}, then ready.
    - (g5) Deletion during reading:
      - 1000: deleteSite(K1), getKeys held.
      - 5000: gate resolves readTimeout; tombstone issued with an AdminSlots slot.
      - 6000: getKeys result ignored.
      - settle(tombstone) makes the dictionary {}. Deletion resolves ≤ 45s in every tombstone-timeout variant.
    - (g6) Boundary: reads finish at 4900, so slot requested at 4900. With slots held and timeouts as in BR22f, key resolves at ≤ 24900.
    - Criteria:
      - No set for a gate after resolution; no set before a valid read completes.
      - readSlotsHeld ≤ 2 on every row.
      - Every resolution ≤ 25000 ms from t_s; every AdminDelete ≤ 45000 ms.
      - One release per message.
      - Final dictionary = SR-model extended with the read phase.
    - Negative control lateReadCheckpoint issues a anchor at 6000 in (g1) and fails.
  - BR22h (G, unit, epoch fence, D134, I74):
    - Setup:
      - FakeBackend holds every set/remove until scheduler apply, settle or drop, under A15c/A15d.
      - Each generation has a monotonic clock.
      - Initial state: only F(5) visible, bootMs = −200000; S_A record version (5, 3).
    - (h1) Forty generations G1..G40 at t = k·1000. Each issues set F(6) and disappears after 10 ms without settle. All operations apply after G40 boots, in fixed-seed order.
      - Expected: all generations choose E = 6; names {F(5), F(6)} at every instant.
    - (h2) Generations at t = k·70000. Each generation's operation applies before the next boots; disappearance after 100 ms, before T_LATE, so no sweep.
      - G1..G7 write F(6)..F(12); pre-write EN values 1..7.
      - G8..G40: EN = 8, so no issue.
      - Expected: names ≤ 8 at every instant; epochGateBlocks = 33.
    - (h3) Continue h2; G41 at 41·70000 stays alive:
      - EN = 8; wait 60 monotonic seconds, then delete F(5)..F(11).
      - EN = 1; issue F(13) and confirm it.
      - Then anchor (13, 0), then ['k','v'] at version (13, 1), reply null.
      - No data set before F(13) confirmation.
    - (h4) As h3, but remove always fails:
      - After sweep EN = 8; every storage_set gives −32603 {storeRecovery, reason: 'epochNames'}.
      - Zero F/data set; record (5, 3) remains readable unchanged; action 25 is raised.
    - (h5) Exhaustive:
      - Six generations; three placements for each F operation: before next boot, after it within A15c, or dropped. Total 3^6 = 729 combinations.
      - Two boot intervals per combination: 1000 ms and 70000 ms.
      - Seventh generation stays alive and writes data.
      - Reference: SR-model extended with D134.
    - (h6) Coverage:
      - G1 at 0 issues F(6) and disappears.
      - G2 at 70000 confirms F(6), bootMs = 70000, issues a creating set for new site A without settle, then disappears at 70500.
      - G1's F(6) applies at 70600, setting bootMs(F(6)) = 0.
      - G3 at 71000 confirms F(7), sitesLive = 60.
      - At 71100: lateGens = 1 (only F(7)), hence LATE_SITES = 4. New site B: 60+0+4+1 = 65 > 64 → 4300 {sites}.
      - apply(A) at 100000.
      - Expected: keys ≤ 64 at every instant; G2 operations covered by G3's item as stated in the coverage lemma.
    - Criteria at every instant of every vector:
      - Epoch items ≤ EPOCH_NAMES_MAX.
      - Largest visible item nondecreasing; epochResurrected = 0.
      - Data-writing generations' epochs strictly increase; no repeated data-record name.
      - No confirmed dictionary lost; names ≤ STORE_RECORDS_MAX.
      - Every decision matches SR-model.
    - Negative controls:
      - epochNonceNames gives 41 names in (h1) and fails.
      - epochNoGate gives 41 names in (h2) and fails.
      - epochSweepEarly fails (h5). Detecting combination: G1 issues pending F(6), G2 confirms F(6), G3 confirms F(7) and immediately deletes F(6), then G1's F(6) applies, resurrecting the name.
- BR23 (G, unit, disk, D104). FakeBackend computes bytesInUse from summed encoded name/value lengths.
  - (d1) Setup: live generation E booted at −120000; no other epoch item in the window, so lateGens=0 and LATE_SITES=0. There are 64 sites with one live record each; last refresh completed before 0.
    - 0: first set for sixty-fifth site X → 4300 {sites, 64}, no retryAfterMs, zero set.
    - 100: delete site 1 through “Storage management”. Tombstone settles at 200; refresh completes at 250, dropping its ticket. sitesLive=64 including tombstone; X rejected at 300.
    - 300: TombReaper issues remove (no session or older generation in window); settles at 350; subsequently issued refresh completes at 400, sitesLive=63.
    - 450: X → ok, R_sites=1.
  - (d2) remove always fails, with successive maximum-size writes to one key:
    - Every accepted set satisfies admission.
    - First 4300 {disk} occurs at the first computed violation.
    - Disk ≤ STORE_DISK_HARD at every instant.
    - Last confirmed dictionary remains readable after reboot.
  - (d3) Five boots within 60s: fifth writes no epoch item until first leaves the window, then continues.
  - (d4) Four dead generations, each with two pending maximum-size sets, apply after their successor boots: disk ≤ limit at every instant.
  - (d5) Missing getKeys → storeRecovery, zero get(null) calls.
  - (d6) Tombstones and undeleted records accumulate to the full name-condition boundary (D134): names+R_names+1+LATE_NAMES+EPOCH_NAMES_MAX ≤ 1024.
    - Writes accepted while names+R_names ≤ 991 before the operation.
    - First 4300 {disk} at names+R_names = 992, because 992+1+24+8 = 1025.
    - No set for the rejected operation.
  - (d7) Dictionary reaching 4097 pairs → 4300 {entries, 4096}.
  - (d8, D105) Two admissions for different keys in the same event-loop turn, inUse set to fit only one enc → first ok, second 4300 {disk}; R_bytes = first enc.
  - (d9) Setup: lateGens=0 as d1, sitesLive=63, R_sites=0.
    - First messages for two new sites in the same turn → first ok, second 4300 {sites}, no retryAfterMs.
    - Variant: first set fails → once a refresh issued after failure settle completes, R_sites returns to 0 and second request succeeds on retry.
  - (d10) Gap: settle at 100; refresh issued at 90 completes at 110. Ticket remains after 110; admission at 120 sees it. Dropped only when a refresh issued after 100 completes, whose measurement includes its effect.
  - (d11, I64) Name boundary under the full admission rule: names+R_names+1+LATE_NAMES+EPOCH_NAMES_MAX ≤ STORE_RECORDS_MAX, i.e. names+R_names+1+24+8 ≤ 1024.
    - Setup: bytes/sites far from limits. names+R_names = 991; no refresh between admissions.
    - Two admissions for counted keys in the same event-loop turn:
      - First: 991+1+24+8 = 1024 → ok, R_names increments by one.
      - Second: 992+1+24+8 = 1025 → 4300 {disk}, no set.
  - (d11b, I64) Detect omitted epoch reserve:
    - Setup as d11, but names+R_names = 999. This is the old formula's boundary because 999+1+24 = 1024.
    - One operation: 999+1+24+8 = 1032 > 1024 → 4300 {disk}, no set, unchanged R_names.
    - Negative control bridge-fault:epochReserveOmitted removes EPOCH_NAMES_MAX from the name condition:
      - Accepts d11b because 1024 ≤ 1024.
      - Accepts d11's second operation because 1017 ≤ 1024.
      - Fails both vectors.
    - SR-model carries d11/d11b with correct and defective paths.
  - (d12, D112, I66) Site slots across generations. SR-model extended with lateGens/LATE_SITES.
    - Setup:
      - 63 counted keys; disk/names far from limits.
      - Generation E booted at −120000, outside the window.
      - FakeBackend holds operations.
    - Table (time: event → effect):
      - −10: E issues first nonempty-data set for new site A, ticket R_sites=1.
      - 0: E disappears before apply.
      - 100: E+1 boots and confirms epoch. sitesLive = 63, lateGens = 1, LATE_SITES = 4.
      - 200: first set for new B → 63+0+4+1 = 68 > 64 → 4300 {sites, 64, retryAfterMs ≈ 60000}, zero set.
      - 30000: late apply(A).
      - 60100: E+1 window expires; refresh issued afterward completes at 60150. sitesLive = 64, lateGens = 0.
      - 60200: retry B → 64+0+0+1 > 64 → 4300 {sites}, no retryAfterMs.
    - Criterion:
      - Distinct on-disk keys ≤ 64 at every instant.
      - Every decision matches SR-model.
      - Zero automatic deletion of any record.
    - (d12a) A never applies → sitesLive = 63 at 60150; B accepted at 60200; count 64.
    - (d12b) Repeated site: at 200, a write to a counted key → accepted without a slot despite LATE_SITES.
    - Negative control bridge-fault:noLateSites sets LATE_SITES = 0:
      - Accepts B at 200.
      - apply(A) at 30000 makes count 65 and fails.
  - (d13) Multiple generations:
    - Setup: sitesLive = 50; E+1/E+2/E+3 boot at 100/20100/40100.
    - Each dead generation leaves two pending creating operations for new sites: a data set and an empty-dictionary anchor {} for a recordless key.
    - Scheduler applies/drops them at every allowed position under A15c.
    - Live generation attempts a new site every 1000 ms until 120000.
    - Criterion:
      - Count ≤ 64 at every instant.
      - LATE_SITES ≤ 12; does not decrease before the measurement issued after expiry.
      - Admissions/rejections match SR-model.
  - (d14) Deletion and recreation:
    - Setup: 64 keys; user deleted K, leaving a single tombstone.
    - Tombstone removal cannot precede older generations leaving the window or occur with a session/ticket for the key.
    - A new site is rejected until a completed measurement does not see K, then accepted.
    - Recreating K afterward requires one new slot.
  - (d15, D113, I67) Capacity with a retained tombstone.
    - Setup:
      - 64 counted keys including K with one confirmed/measured tombstone, R_sites=0, no K session.
      - E−1 booted at −50000, live E at −40000. At 0: lateGens=2, LATE_SITES=8.
    - 100: first set for different site X → 4300 {sites, 64}, no retryAfterMs.
    - 200: K session opened by clicking. Recovery sees tombstone, dictionary {}; anchor accepted without a slot, R_sites=0 despite LATE_SITES=8. TombReaper does not remove tombstone because a session exists.
    - 300: K ['k','v'] → null, R_sites=0.
    - 400: X → 4300 {sites}.
    - (d15b) Without a session:
      - 10000: E−1 leaves window.
      - 10050: TombReaper issues remove, settles at 10100. Later-issued refresh completes at 10150 → sitesLive=63.
      - 20000: E leaves; refresh completes at 20100 → lateGens=0.
      - 20200: K session → creating anchor {} reserves a slot: 63+0+0+1 = 64 → ok, R_sites=1.
      - 20250: X → 63+1+0+1 = 65 → 4300.
      - 20300: settle. 20350: refresh → sitesLive=64, R_sites=0.
      - 20400: K set without a slot.
    - (d15c, I68) Pin with direct assertion, then overflow after contributions leave the window.
      - Setup:
        - 64 counted keys including K with one confirmed/measured tombstone, no K session.
        - E booted at −120000; no other epoch item in window.
        - FakeBackend holds operations.
        - DiskLedger.refreshNow() is a test-build-only hook issuing a normal serialized refresh.
      - Table (time: event → effect in correct implementation):
        - −100: TombReaper in E issues remove R for K's tombstone. D112 conditions hold: no session or older generations in window. R is a zero ticket, unsettled.
        - −50: E disappears before apply(R).
        - 0: E+1 boots and confirms epoch. lateGens=1 (E+1's item represents E), LATE_SITES=4.
        - 50: refresh sees K tombstone → sitesLive=64.
        - 100: K session by clicking. Recovery sees tombstone, dictionary {}. Anchor C0 = (E+1, 0) in RecoverySlots A, no site slot because K counted, adding a pin. C0 held, no settle.
        - 5100: C0 timeout → C1 = (E+1, 1) in slot B, no site slot, adding a pin. Held.
        - 10100: C1 timeout → K failed in this generation under D107. Expected side effect outside the discriminator. Both tickets live, pinnedKeys=1.
        - 30000: late apply(R) within T_LATE of E+1 boot (A15c). Tombstone disappears; no K record on disk.
        - 30010: refreshNow completes at 30050 without seeing K. Direct assertion: K stays in sitesLive; stats.sitesLive=64, pinnedKeys=1, lateSites=4.
        - 30100: new X → literal 4300 {reason: 'sites', limit: 64}, no retryAfterMs, zero set. Reason: sitesLive+R_sites = 64+0; 64+0+0+1 > 64 prevents admission even after LATE_SITES disappears, and D112 requires field absence here. Same rejection code as dropPinned, but different data shape.
        - 60000: E+1 window expires; onWindowExpiry schedules refresh completing at 60050 → lateGens=0, LATE_SITES=0, sitesLive=64 through pin.
        - 60100: X → 64+0+0+1 = 65 > 64 → 4300 {sites, 64}, no retryAfterMs, zero set.
        - 60300: apply(C0), then successful settle. K remains failed, lateRecordsSeen=1.
        - 60310: refresh completes at 60350, sees K, drops C0 ticket. K counted by measurement, count 64.
        - 60400: settle(C1) fails → ticket dropped by next refresh, pinnedKeys=0.
      - Defective dropPinned path, derived from the same table:
        - 30050: sitesLive=63.
        - 30100: 4300 {reason: 'sites', limit: 64, retryAfterMs: 29900}, zero set. Reason: 63+0+4+1 = 68 > 64 while 63+0+0+1 ≤ 64; only reserve blocks admission. retryAfterMs = bootMs(E+1)+T_LATE−30100 = 60000−30100. Actual admission also requires completion of a refresh issued after 60000 (D112).
        - 60100: 63+0+0+1 = 64 ≤ 64 → accept X, reserve slot. set(X) settles at 60200; refresh at 60250 → sitesLive=64.
        - 60350: K/X both seen → 65 distinct on-disk keys.
    - Criterion:
      - Distinct on-disk keys ≤ 64 at every instant.
      - Zero slots for any K operation while counted.
      - Exactly one slot in d15b.
      - Every decision matches SR-model.
    - Negative controls:
      - bridge-fault:seatOnCounted requests a slot for a counted key, rejecting anchor at 200 (64+0+8+1 > 64), and fails.
      - bridge-fault:dropPinned fails d15c at three points:
        - Direct assertion at 30050: sitesLive=63, K excluded despite pin.
        - Accepting X at 60100.
        - Key count 65 at 60350.
        - At 30100 both implementations agree on rejection code, but retryAfterMs in dropPinned violates the correct literal reply, failing that SR-model cell. Secondary evidence; primary discrimination is the three points above.
      - SR-model carries both paths; any cell discrepancy fails.
  - Negative control diskNoReserve accepts the second operation in (d8), (d9), (d11), and drops the ticket at settle in (d10).
  - Negative controls: noDiskGate fails (d2)/(d6); sweepMax fails BR22a at P1 with late delete application.
- BR24 (G, unit, administrative deletion, D106). FakeBackend/SR-model extended with deletion. Setup: S_A dictionary {k0:'a'}, tabs T1/T2.
  - (x1) W1=['k1','x'] from T1 active/held, W2 from T2 waiting, then deleteSite:
    - Both frames destroyed; W2 suppressed with release.
    - Tombstone takes seq greater than W1.
    - settle(tombstone), then successful settle(W1) → dictionary {}, largest record = tombstone; new session sees {}.
  - (x2) W1 applies after tombstone following storeTimeout → lower than tombstone, swept; result {}.
  - (x3) Deletion while recovering: tombstone not issued before gate resolution; late C0 lower than tombstone.
  - (x4) Tombstone fails twice → unrecovered and “Deletion failed”. Next recovery = largest valid record under SR-model.
  - (x5) Tombstone timeout, then application after new recovery anchor → does not appear; notice “Unconfirmed”. Application before read → {}.
  - (x6) Recreation after resolution (D113): snapshot {}; first set version greater than tombstone.
    - While tombstone remains: R_sites unchanged; no ticket carries a site slot.
    - After TombReaper removes it and a measurement not seeing S_A is adopted: first anchor reserves one slot.
    - At-capacity case covered by BR23(d15).
  - (x7) deleteSite from two pages simultaneously → one tombstone, one promise.
  - (x8) Attempt to open session while deleting → blocked until resolution.
  - (x9, D108) Tombstone-slot ownership.
    - Setup: K1/K2 with nonempty dictionaries, FakeBackend holds every operation, both gates ready.
    - Table (time: event → effect):
      - 0: deleteSite(K1) → tombstone T1a takes slot A. held=1.
      - 5000: T1a timeout → T1b takes B; T1a retains A. held=2.
      - 6000: deleteSite(K2) → waits in AdminFIFO, zero set.
      - 10000: T1b timeout → K1 uncertain then unrecovered; both slots remain held.
      - 11000: K2 wait expires → adminBusy, zero K2 tombstone; K2 dictionary bytewise unchanged.
      - 13000: generation disappears with T1a/T1b pending.
    - After disappearance: apply T1a/T1b late at every L point in BR22a.
    - Criteria: 2 late tombstones; disk/names bounded at every instant; K1 follows SR-model; K2 intact.
  - (x10) x9 variant: settle(T1a) fails at 8000.
    - A releases; K2 tombstone takes it at 8000. held=2.
    - K1 does not resolve before 10000.
  - (x11) x9 variant: settle(T1b) succeeds at 7000.
    - K1 deleted; B releases; K2 tombstone takes it at 7000.
    - T1a owns A until settle; late successful settle does not alter resolution.
  - (x12, D111, I65) First succeeds while second waits, followed by recreation.
    - Setup: K1/K2/K3 with nonempty dictionaries, gates ready; FakeBackend holds every operation.
    - Table (time: event → effect):
      - 0: deleteSite(K1), op1 → tombstone T1a in A.
      - 1: deleteSite(K2), op2 → T2a in B. held=2.
      - 4000: deleteSite(K3), op3 → T3a request in AdminFIFO.
      - 5000: T1a timeout → T1b request for op1 behind T3a. T1a retains A.
      - 6000: settle(T1a) succeeds; same-step events:
        - K1 deleted, op1 settled.
        - Withdraw T1b.
        - A goes to T3a.
      - 6100: new K1 session by click → snapshot {}. Then ['k9','n'] → null, version greater than T1a.
      - 7000: settle(T2a) succeeds → K2 deleted, B free. held=1.
      - 8000: settle(T3a) succeeds → K3 deleted. held=0.
    - Criterion:
      - For op1 after 6000: zero alloc, zero DiskLedger ticket, zero set.
      - Final K1 dictionary {k9:'n'} after reboot/recovery.
      - adminStaleDropped = 0.
      - One release per message.
      - set calls = 3 tombstones + k9 write.
    - (x12b) bridge-fault:adminNoCancel disables withdrawal only:
      - T1b remains in FIFO, receives B at 7000.
      - Guard rejects it; B immediately returned.
      - Criterion: adminStaleDropped = 1, zero alloc/set for op1, K1 = {k9:'n'}.
    - Negative control bridge-fault:adminStaleIssue disables withdrawal and guard:
      - Issues T1b at 7000 with version greater than k9.
      - K1 becomes {}, failing x12.
    - RecoverySlots variant (BR22f-f5): settle(C0_K1) succeeds while C1_K1 waits, withdrawal disabled by same build → recoveryStaleDropped = 1, zero C1_K1 set.
  - Criterion: zero k0/k1 appearances after a confirmed tombstone, one release per message, disk bounded at every instant. adminTombsUnsettledMax ≤ 2 on every row; no tombstone without a slot.
  - Negative control adminEarlyRelease (D108) releases A at 5000 and B at 10000.
    - Issues K2 tombstone at 6000, making unsettled count 3, failing x9.
    - After disappearance, late tombstones exceed generation reserve.
  - Negative control deleteUnordered issues tombstone before closing sessions, allocating W2 afterward, failing (x1) by k1 appearing.
- BR-dep: two dependency-cruiser rules in CI; violation of either fails:
  - Bridge module imports neither WalletSubmit nor HttpTransport.
  - No module calls fetch except HttpTransport.

**MR (G with C measurements, remote-response receiving, D98)**

Setup:
- MaliciousHttp is a local Node server with an RP profile. Replies literally according to script; records sent bytes and client-disconnect time.
- MR-unit tests HttpTransport in Node with a fake stream (G).
- MR1–MR12 run in Chrome.

Common criterion:
- Exact specified code.
- Zero partial content in any frame, sink or viewer.
- Per-request TestHooks.recvStats.maxBuffered ≤ recorded recvLimit+maxChunk.
- Reservation released within ≤ 100 ms of settlement.
- Worker ping p99 ≤ 100 ms (C).

Cases:
- MR1: eth_blockNumber replies 300 MiB at maximum speed → −32603 {recvLimit} after exceeding 4096 bytes. Record actual bytes sent before cutoff.
- MR2: error object with 300 MiB data → recvLimit.
- MR3: Content-Length = 1 GiB → recvLimit, zero buffered bytes.
- MR4: Content-Encoding gzip; 1 MiB decompresses to 1 GiB → recvLimit using post-decompression count.
- MR5: response ≤ 4 KiB, depth 2000 → recvDepth.
- MR6 (boundary): eth_getCode valid JSON response exactly 69632 bytes → accepted; 69633 → recvLimit.
- MR6b (capped): eth_call body 1048576 bytes → accepted; 1048577 → recvLimit.
- MR6c (exact): eth_getBlockByHash body 163958 bytes, network worst case → accepted; 167937 → recvLimit.
- MR6d (exact): eth_feeHistory n=128, p=16, body 168015 bytes → accepted.
- CAP1 (G, anvil-br, Blob contract deployed by test tool): eth_call to Blob(262144), then Blob(1048576).
  - Expected result derived from body length B recorded by RpcTap: if B ≤ 1048576, success and byte match; otherwise recvLimit.
  - Estimate: first approximately 512 KiB, second approximately 2 MiB.
  - Record in T12 as a product limit.
- RF (G, NetworkProfiles.validate):
  - RF1: network → accepted.
  - RF2: end/full → accepted.
  - RF3: profile with 1100 M_0 identities, |genesisPre| > 47104 → recvFit.
  - RF4: BODY_MAX=262144; worst case 217928 > 167936 → recvFit.
- MR7: eth_call dripping 1 B/s → −32603 {transport} at 10s±0.2.
- MR8: pocol_getHeaders replies 20 MiB → viewIncomplete and 4901, no frame.
- MR9: eth_getCode response for a 1 MiB chunk → load failure without partial display.
- MR10 (I53): six separate cases. Unlike the general MR rule, MR10a–MR10c and MR10f run in Node, calling HttpTransport or Bridge with FakeClock/FakeStream holding replies. Reservation uses recvLimit, not actual size; equality with limit accepted.
  - MR10a (session bound):
    - At 0, S1 sends ten eth_call at 1 MiB limit → all reserved, S1 reservation = 10 MiB.
    - Eleventh request at 0 → waits, then {busy} at 2000.
    - Twelfth at 2100 → waits. Releasing request 1 at 2500 reserves it at 2500.
  - MR10b (SITE_POOL):
    - S1/S2/S3 each 10×1 MiB at 0, then two S4 requests → 32 MiB.
    - Third S4 request → {busy} at 2000.
    - At 100: pocol_getHeaders from TRUST_POOL (96 KiB) and chunk from CONTENT_POOL (68 KiB) → immediate reservation.
  - MR10c (RECV_INFLIGHT_MAX):
    - 48 eth_blockNumber requests (4 KiB) from 12 sessions → all reserved.
    - Request 49 from a thirteenth session → {busy} at 2000 despite available bytes.
    - Trust request at the same instant → immediate reservation.
  - MR10d (Chrome, G with C measurement):
    - Three tabs, 5 eth_call each. MaliciousHttp holds replies for 5s; each reply is 900 KiB.
    - G: per session, 4 messages routed and 1 −32005 {pending}; total reservation 12 MiB; zero busy.
    - G: HeaderNetCheck in fourth tab succeeds. After release, replies arrive complete; reservation returns to 0 within ≤ 100 ms.
    - C: maximum process memory measured and disclosed.
  - MR10e: global pending bound (16) covered by BR20d.
  - MR10f (SESSION_RECV_MAX through bridge), session with full bucket:
    - t=0: m1 eth_getLogs over 0x1..0x10. H(latest) replies immediately and releases reservation, then pocol_getLogs held → 8 MiB reservation, pendingReads=1.
    - t=10: m2 eth_call → 9 MiB, pending=2.
    - t=20: m3 eth_call → 10 MiB, pending=3.
    - t=30: m4 eth_call → passes pending gate (pending=4), waits for reservation; at 2030 gets −32005 {busy}, pending=3.
    - t=2100: release pocol_getLogs → H(16) reserved, then commit; pending=2, reservation 2 MiB.
    - t=2200: m5 eth_call → immediate reservation (3 MiB).
- MR11: invalid UTF-8 or duplicate key within bound → recvParse.
- MR12: 512 KiB error.message for eth_call → delivered to frame truncated to 256 units; data above 4 KiB removed.
- MR-neg: bridge-fault:recvNoLimit must fail MR1/MR4.

**LC-unit (G, LogClient with deterministic MockLogServer and SinkChecker)**

Branches, all with head at 3000:
- A: two logs at each of 10, 499, 500, 1000, 2500 (tag 1), total 10.
- B: matches A at 1..999, differs from 1000. Two logs at each of 1000', 2600' (tag 2), total 10.
- A': matches A at 1..999, differs from 1000. Two logs at 1000'', 2500'' (tag 3), total 10.

Logs per window (1,1024)/(1025,2048)/(2049,3000): A: 8 (10, 499, 500, 1000), then 0, then 2 (2500). B: 8 (10, 499, 500, 1000'), then 0, then 2 (2600'). A': 8 (… 1000''), then 0, then 2 (2500'').

RefX(a,b) is branch X's complete list; each committed result is compared bytewise after canonical ordering. Header requests recorded as H(·). Counters follow browser's definition; SinkChecker checks (S-a)–(S-e).

Cases:
- LC1: 1..3000 without limits. Requests: H(latest), (1,1024), (1025,2048), (2049,3000), H(3000). Trace: begin(1), piece×3 (8, 0, 2), commit(1, {pieces:3, logs:10, logRequests:3, headRequests:2, totalRequests:5}). Result RefA(1,3000).
- LC2: (1,1024) gives Limit{resultBytes, lc=499}. Log requests: (1,1024), (1,499), (500,1024). Result RefA(1,1024) = 8; logs at 10/499 once only. Any request starting at 500 before (1,499) completes fails.
- LC3: −32021 {retryAfterMs=500} → wait ≥ 500 ms, then same range; both requests count in totalRequests/logRequests.
- LC4: fetchAll exceeding 4 MiB → tooLarge, zero partial result.
- LC5: (500,500) gives Limit{deadline, lc=499} four times. Requests: H(latest), (500,500)×4, totalRequests=5; no precommit H. Result {deadline, block: 500}, abort(1, deadline).
- LC6: (1,1024) gives Limit{deadline, lc=0} → (1,512), then (513,1024).
- LC7: serverViolation in four cases: resultBytes with lc=b=1024, resultBytes with lc=a−1, deadline with lc=a−2, unknown reason.
- LC8: first (1,1024) gives −32022 anchorNotCanonical, then server serves B. Trace: begin(1,A), abort(1,reorg), begin(2,B), piece×3 (8, 0, 2), commit(2, {pieces:3, logs:10, logRequests:3, headRequests:2, totalRequests:7}). Result RefB.
- LC9: log outside [a,b], non-strict order, or changing blockHash at one height → serverViolation.
- LC10: maxRequests=4 on LC1. Four requests succeed, three pieces. H(3000) would make totalRequests=5, so not sent → abort(1, tooManyRequests), Error, zero commit. MockLogServer records exactly 4 requests.
- LC11: Limit{scratch, lc=0} on (1,1024) → (1,512), then (513,1024). Result RefA(1,1024).
- LC12: (1,1024) gives scratch, lc=300; (1,300) gives scratch, lc=150. Sequence: (1,1024), (1,300), (1,150), (151,300), (301,1024). Result RefA(1,1024).
- LC13: (500,500) gives scratch, lc=499 four times, intervals ≥ 500 ms; then Error{scratch, block: 500}, abort(1, scratch).
- LC14: (1,1024) gives resultBytes, lc=499; then (1,499) gives resultBytes, lc=200 → serverViolation. Variant: deadline, lc=200 → (1,200), (201,499), (500,1024).
- LC15: scratch with lc=b → serverViolation. scratch with lc=a−1 and a=b → Retry, not bisection.
- LC16 (range 1..3000):
  1. H(latest) → A, then begin(1, {hashA3000, 3000}).
  2. (1,1024) → −32020 {resultBytes, fromBlock: 1, lastCompleteBlock: 499, nextFromBlock: 500}, no piece.
  3. (1,499) [withinLimit] → 4 logs (10, 499) → piece(1,0).
  4. (500,1024) → 4 (500, 1000 from A) → piece(1,1).
  5. (1025,2048) → 0 → piece(1,2).
  6. Server switches to B. (2049,3000) → −32022 {anchorNotCanonical, head: 3000} → abort(1, reorg).
  7. H(latest) → B, then begin(2, {hashB3000, 3000}).
  8. (1,1024) → 8 (10, 499, 500, 1000') → piece(2,0).
  9. (1025,2048) → 0 → piece(2,1).
  10. (2049,3000) → 2 (2600') → piece(2,2).
  11. Matching H(3000) → commit(2, {pieces:3, logs:10, logRequests:3, headRequests:2, totalRequests:11, anchor: hashB3000}).
  - Accounting: attempt 1 = 6, attempt 2 = 5. Committed RefB(1,3000) = 10; zero tag-1 logs from 1000. Reference consumer drops attempt-1 pieces. Defective consumer gets 18 logs, including A's two logs at 1000 with blockHash outside B, detected by the test.
- LC17 (A→B→A'):
  1. H → A, begin(1). 2. (1,1024) → 8 → piece(1,0). 3. (1025,2048) → 0 → piece(1,1). 4. B: (2049,3000) → −32022 → abort(1). 5. H → B, begin(2). 6. (1,1024) → 8 → piece(2,0). 7. A': (1025,2048) → −32022 → abort(2). 8. H → A', begin(3). 9. Three windows (8, 0, 2) → piece(3,0..2). 10. Matching H(3000) → commit(3, {pieces:3, logs:10, logRequests:3, headRequests:2, totalRequests:12}).
  - Accounting 4+3+5. Committed RefA'. Variant: −32022 on first log request in 4 attempts → abort(4, reorgUnstable), totalRequests=8, zero commit.
- LC18: three windows succeed on A (8, 0, 2), then server switches to B. H(3000) ≠ hashA3000 → abort(1,reorg), begin(2,B), three pieces, matching H, commit(2, {…, logRequests:3, headRequests:2, totalRequests:10}). Result RefB.

**PS-unit (G, SendPacer with PacerSim using virtual clock/socket)**

Literal PacerSim model (D93, D94):
- P = bytes delivered to the pacer (written), A = socket-accepted bytes, Rc = client-read bytes. pending = [P > A].
- sendTime advances by the virtual clock only while pending.
- Acceptance modes: (a) acceptPlan: scripted A(sendTime), A ≤ P; client unmodeled. (b) sockBuf=SB: immediate greedy writing, A = min(P, Rc+SB).
- Batch: item k+1 is produced (P += S) immediately when A = P at item k's end; computation time zero unless computeGaps specified. Server thus leads client by up to SB.
- Client in PS5/PS5c: when its read pointer reaches an available first byte of item k, pause 1.8s, then read all of k at infinite speed as bytes become available, then move to k+1.
- At one instant: repeat (read, accept, produce) to a fixed point, then check. Recorded state at each instant is the check-point state.
- Events (D94): sendDone is first check point with A = P = size after all items produced; record {t, sendTime, Rc, receivedItems, unreadAtSendDone = A−Rc}. receiveDone is first check point with Rc = size. Always receiveDone ≥ sendDone; equality possible.
- Constants: R = 65536 B/s, GRACE = 2s, TIMER_EPS = 1 ms, ENFORCE_SLACK = 200 ms. No JSON framing bytes in PS.
- Common criterion: every trace has A ≤ P and, in sockBuf mode, Rc ≤ A. Every closure satisfies CloseOK. No sendDone preceded by a violation lasting beyond TIMER_EPS. Rc = A is not required at sendDone.

Cases:
- PS1 (credit covers pause): 4 MiB reply, P = 4194304 from start. acceptPlan: 2 MiB at constant rate over sendTime ∈ [0, 1.0], zero for 10s, then 1 MiB/s. At pause: A0 = 2097152, sendTime0 = 1.0, credit 2031616. Minimum credit at sendTime = 11: 1376256 > 0. Expected: sendDone without closure at sendTime = 13, A = 4194304 ≥ 851968.
- PS2: PS1 with 60s pause → closure at sendTime ∈ (32.000, 32.002], because s_v = 32.
- PS3: 2 MiB at 2097152/31 B/s over [0, 31.0]. A = 67650.06·t ≥ R·t, no violation; then 10s pause → closure at (32.000, 32.002]. Bytes alone do not determine credit.
- PS4 (stepwise A): PS1 until sendTime 1.0, then socket accepts 1024 B at each sendTime = 1+n/16 (n = 1, 2, …). After step n: A_n = 2097152+1024n, s_v(n) = 32+n/64. First violation at smallest n with s_v(n) < 1+(n+1)/16, i.e. n > 660. Thus n = 661, A = 2774016, s_v = 42.328125, inside [42.3125, 42.375). At n = 660, s_v = 42.3125 equals step 661's instant; acceptance runs before checking, no violation. Expected closure at sendTime ∈ (42.328125, 42.330125].
- PS5 (SB = 30000, 20 items, S = 98304, hence SB < S):
  - Table (t, Rc, P, A, pending, sendTime, s_v): t=0: (0, S, 30000, yes, 0, 2.0), client pauses at item 1. t=1.8: reads item 1, A = S = P, then item 2 produced. State (S, 2S=196608, S+30000=128304, yes, 1.8, max(2, 1.9578)=2.0), client pauses at item 2.
  - Conforming: closure at sendTime ∈ (2.000, 2.002], t ≈ 2.001, Rc = S. Neither sendDone nor receiveDone.
  - perItemGrace: at 1.8k client reads item k (Rc = kS), acceptance of k completes, k+1 produced, A = kS+30000. Since SB < S, k's acceptance cannot finish before it is read. Item k pending over [1.8(k−1), 1.8k], its clock ≤ 1.8 < 2, no closure. At t=34.2: Rc = 19S, item 20 produced, A = 19S+30000 < 20S = P. At t=36: client reads item 20; fixed-point loop accepts final byte before reading it; at check point A = P = Rc = 20S. Result: sendDone = receiveDone = 36, receivedItems = 20 at both events, unreadAtSendDone = 0.
  - Passing requires both outcomes.
- PS5c (SB = 204800 > 2S; server leads client):
  - Table (t, Rc, P, A, sendTime, s_v), pending true throughout [0, 10.625]: t=0: items 1/2 fully accepted, 8192 bytes of item 3 accepted. (0, 3S=294912, 204800, 0, 3.125). t=1.8: (S, 4S=393216, 303104, 1.8, 4.625). t=3.6: (2S, 5S=491520, 401408, 3.6, 6.125). t=5.4: (3S, 6S=589824, 499712, 5.4, 7.625). t=7.2: (4S, 7S=688128, 598016, 7.2, 9.125). t=9.0: (5S, 8S=786432, 696320, 9.0, 10.625).
  - Each row's read consumes the previous item, completing acceptance of an item and producing the next. A ≤ P on every row. sendTime = t because pending becomes zero only instantaneously. Client paused at item 6 over [9.0, 10.8], A constant.
  - Conforming: closure at sendTime ∈ (10.625, 10.627], Rc = 5S. No sendDone.
  - perItemGrace: at t = 1.8k, Rc = kS, A = min(P, kS+204800). Items 1/2 never become pending. Item j ≥ 3 produced at 1.8(j−3), acceptance completes at 1.8(j−2), pending ≤ 1.8 < 2, no closure. t=30.6 (k=17): item 20 produced, P = 20S, A = 19S+8192. t=32.4 (k=18): Rc = 18S, A = 20S = P. This is sendDone; receivedItems = 18, unreadAtSendDone = 2S = 196608. t=34.2: reads item 19. t=36: reads item 20, receiveDone, receivedItems = 20.
  - This vector establishes accepted unread bytes at sendDone. Reservation released at 32.4, not 36.
- PS6: PS2 with injected timer delays. 150 ms: closure within ENFORCE_SLACK, pacerLateMaxMs ≤ 200. 250 ms on all timers including backstop: pacerLateMaxMs = 250; monitor raises action 21.
- PS7: two-item batch with computeGap 100s, no pending. sendTime does not advance, no closure; batchSendMaxS equals actual send time through sendDone.
- PS8: zero acceptPlan with P > 0 → closure at (2.000, 2.002], A = 0.

**Phase B (G)**
T22 (PK1–PK18), T0, AS, ASQ, ASB, PG-END, PG-RPC, T2, T2b, T2c, T24, T26 and T27.
- FX0: repeat build with bytewise match; Draw(h,0)=w for each h ∈ [3,200].
- FX0-neg: nonce from another range → 13; powHash > target → 7.
- TJ (P11, p.ts=1000): TJ1 1001→1009, TJ2 998→1007, TJ3 1030→1038; TJ4: 1009.000s locks, 1009.001s means FB.

Chain vectors:
- SV8: 1..199, then F=200 signed by w with [tx1] → Δburned(F)=1155008, w +208992, zero HELD, zero pending.
- SV8-fb: fallback F → 1071008, w +208992.
- SV8-cap: well-formed fallback h=201 → H-pre item 2 (10a), without work.
- FBX: SV8-fb then SV8 on same parent → adopt SV8, state byte-identical.
- TW (D141, G, ChainFixture on sv-fix):
  - TW1 (one template, two owners):
    - At h=10 build one UT with Build_fb (fallback, body 0xc2c0c0) before any search, without winner input.
    - FixtureMiner searches same UT in w's range then x's, bound 2^28 each; produces B_w/B_x with winnerSig from each range owner.
    - Expected: RLP(UT), TemplateID, stateRoot, receiptsRoot and gasUsed byte-identical; different blockHash; both pass H-full on same parent.
    - State at 10 byte-identical on both branches; pending[10].winner = 0 in both.
  - TW2 (recording, settlement and rollback):
    - W = B_w then fallback 11..26 with winner w; X = B_x then 11..26 by same rule.
    - After block 11: pending[10].winner = w in W, x in X; stateRoot(11) differs.
    - At 26 settle fallback pot_10 = 80000, C=1 as SV7; beneficiary receives 13062.
    - Expected: x balance in X minus W = +13062; w = −13062; no other account differs.
    - Extend W with block 27 after importing X → adoption; state byte-identical to a node that saw W alone.
  - TW3 (finalization): sv-fix through F=200.
    - pending[199].winner = w written in F before finalize.
    - No pending[200]; SV8 values unchanged.
  - TW4 (RPC): pocol_getPot(10) returns winner = null at head 10, w after importing 11.
  - TW5 (D142, nonempty shares):
    - Setup:
      - FixtureMiner builds one fallback UT at h=10 and finds winning nonce ν in w's range.
      - Then searches shares in x/y ranges with bound 2^28, threshold derived from same UT (D143): T_share = min(2^256−1, UT.target·CP.m).
        - ASERT trace for sv-fix block 10: parent h=9, p.ts−g_ts = 63, Δt = 63−90 = −27, e = floor_div(−1769472, 600) = −2950, s = −1, f = 62586.
        - Thus target = F·2^223, T_share = F·2^228 with m = 32.
        - F computed by Difficulty.asert, matches AsertRef, recorded literally in M4b annex. Approximate F ≈ 127041, T_share ≈ 0.969·2^245 < 2^245.
        - Earlier 2^245 value withdrawn because it violates item 9.
      - Fixture assertion (G): each share in B_a/B_b satisfies SHA256(TemplateID‖be64(n)) ≤ derived T_share and passes Consensus.headerPrecheck item 9. FixtureMiner.shareThreshold(UT) equals consensus value bytewise. Any discrepancy gives FixtureSetupError('shareThreshold').
      - Settlement values below depend only on per-owner share counts, unchanged by threshold variation.
      - B_a = [UT, ν, shareList {s_x1, s_x2}], B_b = [UT, ν, shareList {s_y1}].
      - TemplateID/roots identical; shareRoot/blockHash differ; both pass H-full.
    - Branches: B_a then fallback 11..26, winner w; B_b then 11..26 by same rule.
    - Expected after 11:
      - count[10] = {w:1, x:2} in A, {w:1, y:1} in B.
      - lastShare(x) = 10 only in A.
      - stateRoot(11) differs between branches.
    - Settlement at 26 (fallback pot=80000, R=68000, Q=64):
      - A: p_w=1062, p_x=2125, A=3187, roundBurn=0, baseBurn=64813.
      - B: p_w=1062, p_y=1062, A=2125, roundBurn=1, baseBurn=65875.
      - Both branches also pay w winnerPart+propPart = 12000, total 13062 as SV7.
    - Four arms yield byte-identical state for each branch:
      - (i) Direct import.
      - (ii) Fresh-node synchronization from genesis through a peer, no mining memory.
      - (iii) TrieRebuild from snapshot at 9.
      - (iv) Node imports A through 26, then B through 27 and reorganizes, then A through 28 and returns.
    - Ordering variant: B_b reaches second node before B_a, without affecting either branch's state.
    - parentResultMismatch = 0 in every arm.
  - TW5 negative controls:
    - exec-fault:sharesFromFirstSeen takes shares from first-seen version of (TemplateID, nonce). Fails (iv) and ordering variant because B's stateRoot(11) uses A's shares.
    - exec-fault:sharesFromMiner uses local FixtureMiner memory, failing (ii)/(iii).
    - exec-fault:sharesDropped passes an empty list, failing A settlement values (p_x = 0).
    - TW5-gap (validity control, D143):
      - FixtureMiner searches x's range for nonce n_gap with hash in (derived T_share, 2^245]. Probability per hash ≈ 1.5·10^−5; bound 2^28 sufficient, otherwise FixtureSetupError('gap').
      - B_gap = [UT, ν, shareList {s_x1, n_gap}] with valid winnerSig.
      - Expected: item 9 rejection (blockReject), diagSet = {9}, state unchanged.
    - fixture-fault:fixedShareThreshold makes FixtureMiner.shareThreshold return constant 2^245. Fixture assertion fails deterministically because value differs from consensus. It does not rely on a share landing in the gap during search, which is probabilistic.
  - Negative control exec-fault:winnerInState passes current-block nonce owner to closeBlock:
    - Verification stateRoot differs from Build_fb's pre-search root; B_w/B_x rejected by item 17.
    - If template built with guessed winner w, only B_x rejected.
    - TW1 fails both cases.
- SV8-reorg: A = fallback 199/200_A. B = signed 199' at ts_198+1 with [tx1], then fallback 200_B. Require work(B) > work(A), otherwise 'tie'. A through gossip, then B through sync. Expected head 200_B, Δburned since 198 = 1208230, w +235770, tx1 in 199'.
- SV8-tie: ts_199' = ts_198+7 → B through signed preference.
- SV9: block 30 signed with [tx1, tx2, tx3] → getPot(30) = {pending, 253015, signed, hash(30), 46}, surplus +5000. winner (D141) = null at head 30, w after import 31.
- SV9-final: Δburned=1244023.
- SV9-reorg: A = 30/31. B = fallback 30'/31'/32'. Head 32'; state as a node seeing B alone. getPot(30..32) = {pending, 80000, fallback, hash(·'), 46..48}; winner = w for 30'/31', null for head 32' (D141). getSettlement(30) → −32013 {46}; getSettlement(14..16) = {13062, 66938}. Δbalance = +200814 = ΔburnedTotal, surplus 0, Sd = 5000. tx1..tx3 in mempool; reintroducing A does not change head.

**CL-END (G, end/full)**
- Filled only in genesis. h ∈ [1,699] empty fallback, ts +7, winner=w except 10/560; F=700.
- GB-END: RecLen=ActLen=1024, heldTotal=1024·10^20.
- CL-OPS1: h=10 signed by w, body [register(s1) value B_reg, gas=200000, chainId=777905]. Expected status=0 with RegOpsLimit, RecLen=1024, s1 loses fees only. If Draw(10,0)≠w, FixtureSetupError('draw').
- CL-OPS2: same at 560 → Closing.
- closeH=553, 8 records/block. cursorDone true at 680, not 679; F = 700.
- At F: all 1024 PAID or deleted, each rewardAddr received B_reg once, heldTotal=0, Σpending=0.
- T17 every block; Σsubsidy = 80000·699.
- 701 rejected by item 2 (10a); RetentionStore ≤ derived inventory.
- CL-END-neg: PG-END5.
- Disclosed limit: does not exercise registration churn; covered by R16 with T21/T18b.

**RG (D88–D94, logs/dense, prerequisite for M7)**

Fixture:
- 1100 blocks. 50..1073 signed by w, each with s2 transaction calling LogBomb(1) (546 logs, about 32 KiB). Others empty fallback.
- B forks from 999; blocks 1000'..1080' call LogBomb(2), work(B) > work(A), otherwise 'tie'.
- A' forks from 999 with LogBomb(1), work(A') > work(B).
- Node: pocold on Windows, 8 GB, default RpcGuard, rpc.pacerTrace=true. Second node produces at T_blk=10.
- ReferenceLogs directly from RetentionStore for a specified branch.
- RpcAttacker (Rust): {conns, batch, method, range, readRateBps, pauseAtItemMs, pauseAfterBytes, pauseMs, rcvBuf (default 8 KiB), dropAfterBytes, durationS, reconnect}. Records each received byte's time using the same monotonic clock (QueryPerformanceCounter), and receiveDone if reached.
- ResourceProbe every 100 ms. RSS_0 after sync without requests.

Trace assertions, gates for every RG4/RG5/RG14 run:
- TC-send (server trace only): P/A nondecreasing, A ≤ P. sendTime advances only during pending by elapsed monotonic time in those periods. Every completed response has exactly one sendDone, A = P = size, followed by no produce/accept for it. Corresponding reservation released at sendDone: ResourceProbe heldBytes drops by response size in first subsequent sample. traceDropped = 0, otherwise inconclusive.
- TC-recv (client log versus trace): Rc ≤ A at every corresponding instant. Rc = A not required at sendDone; unreadAtSendDone is a measurement. If client continues reading without disconnect after sendDone: receiveDone must occur with Rc = size, bytes matching reference, receiveDone ≥ sendDone; difference measured. If client intentionally disconnects/stops after sendDone: receiveDone requirement inapplicable, recorded, not server failure.
- CloseOK: every rate closure satisfies sendTime > 2, A < 65536·sendTime, sendTime − max(2, A/65536) ≤ 0.2, and matches lastPacerClose.
- Enforced: no interval with sendTime > 2 and A < 65536·sendTime lasts over 0.2s of sendTime without closure. Includes violations arising during read pauses, regardless of A progress or pending interruption. After sendDone, sendTime no longer advances, so no new violation can arise.
- NoEarly: included in CloseOK.
- Breach(trace): first violation instant computed from measured A/pending trace; may not exist.

Cases:
- RG1: eth_getLogs 1..1025 → −32020 {range, maxRange: 1024}, zero admission/snapshot/reservation.
- RG2: eth_getLogs 50..1073 → −32020 {resultBytes|resultCount}, lastCompleteBlock ≥ 50 (P1), no result field.
- RG2b: pocol_getLogs(50..1073, anchor) gives Limit with lc; (50..lc, anchor) gives Complete matching ReferenceLogs bytewise.
- RG3: fetchEach in Node over 50..1073 with head anchor. Sink compares each piece then discards it, passes SinkChecker. Committed 1024·546 = 559104 without loss/duplication; every response ≤ 8 MiB. Counters match RpcRecorder and are recorded.
- RG3-bridge: fetchAll → tooLarge, no partial result.
- RG3b: hook after third piece pauses fetchEach, sends B and awaits adoption. Expected −32022 anchorNotCanonical. Trace: begin(1,A), piece(1,0..2), abort(1,reorg), begin(2,B), pieces, commit(2). Attempt 2 = ReferenceLogs for B (1000'..1073' with LogBomb(2) data); zero logs outside B. totalRequests = RpcRecorder log. Variants: A→B→A' (two aborts, commit(3) = A' ReferenceLogs); pre-anchor-check hook (abort, new attempt).
- RG4: (i) 31 connections, each batching 20 pocol_getLogs over 24-block windows, reading 1 KiB/s, reconnecting for 300s. Every sample: maxResPerConn ≤ 1, heavyResv ≤ 48 MiB, lightResv ≤ 16 MiB, scratchHeavyMax ≤ 16 MiB, snapshotsOpen ≤ 20, RSS ≤ RSS_0+256 MiB. Each connection: TC-send, TC-recv, CloseOK, Enforced. (ii) Fast client, batch 20 over 4-block windows (about 1.2 MiB/member). First 13 complete/match; member exceeding 16 MiB and successors get batchBytes. Order preserved; sendDone then receiveDone.
- RG5: 64 connection attempts, each batching 20 heavy requests, reading 1 KiB/s for 300s with reopening. RSS_max − RSS_0 ≤ 256 MiB; bounds as RG4; conns ≤ 32, excess closed without reading. Every slow connection closes and meets assertions; closure time measured. Control receiver (C) reads 70 KiB/s for an 8 MiB response: log condition for every t > t_first+2 is received ≥ 65536·(t−t_first), t_first = first delivered byte. Then A ≥ received ≥ R·sendTime, no violation. sendDone mandatory with sendTime ≤ 130.2s; receiveDone mandatory because reader continues; difference measured. Zero crashes/OOM.
- RG6: pocol_getLogs over 1024, then TCP disconnect after 1 ms, 1000 times. snapshotsOpen, heavyResv, scratch return to 0 within ≤ 100 ms (p100); same for eth_call with 30M-gas loop.
- RG7 (during RG5): each block imported ≤ 2s, validate_p99 ≤ 3s, zero unexplained FB versus same-seed reference run; imported = produced.
- RG8: honest client with connection established before RG5 sends 10 eth_blockNumber/s. p99 through receiveDone ≤ 200 ms, ≤ 1% −32021. Its new-connection attempts are measured.
- RG9 (measurement): fetchEach on an honest existing connection during RG5; matching commit or explicit error, never partial; record proportion.
- RG10: every successful RG2–RG9 response matches ReferenceLogs at receiveDone.
- RG11: eth_call to MemHog with 30M gas, 100 times. Complete or −32020 {scratch}; counter ≤ 16 MiB; RSS ≤ RSS_0+256 MiB.
- RG12: rpc-fault:lightInflate (600 KiB eth_blockNumber reply) → −32020 {resultBytes}, lightResv = 0, zero panic. Feature absent from release.
- RG13: each light method 1000 times with worst parameters. Computation p100 ≤ 50 ms, size ≤ table; otherwise reclassify before M7.
- RG14 (real sockets): TC-send, TC-recv, CloseOK and Enforced gate every run. Case results:
  - (0) rcvBuf=8 KiB, eth_call to Blob(4 MiB), no reading. G: CloseOK closure. B_meas = A at closure measured, used in no criterion.
  - (0b) sendDone before receiveDone: rcvBuf=64 KiB, eth_call to Blob(n=16384), reply about 32 KiB < SO_SNDBUF. Reader waits 5s then reads all. G: sendDone with sendTime < 2s, no closure, reservation released before first read, recorded unreadAtSendDone > 0; then receiveDone with matching bytes. Real-socket counterpart of PS5c.
  - (i) Request read at 1 B/s → closure at 10s±0.2.
  - (ii) Idle connection → closure at 30s±0.2 from previous sendDone or opening.
  - (iii) Queue wait > 2s → −32021 {busy}.
  - (iv) eth_call to Spin → −32020 {deadline} at 5s±0.2.
  - (v) Blob sizes 512 KiB/8 MiB: read 70 KiB/s (C under RG5 condition): sendDone/receiveDone, sendOverrunMaxMs ≤ 200. Read 32 KiB/s: CloseOK/Enforced closure, time measured.
  - (vi) Batch 20 eth_call to Spin, fast reader → 20 deadline errors, duration ≈ 100s < 420s, batchSendMaxS < 2s, zero closure.
  - (vii) Per-item grace: batch 20 eth_call to Blob(n=49152), S ≈ 96 KiB (record exact size), rcvBuf=8 KiB. Reader pauses 1.8s at each item's first byte, then reads item at maximum speed. Run conforming and rpc-fault:perItemGrace builds with same reader. Conforming: TC-send/TC-recv/CloseOK/Enforced gates; closure/time measured. perItemGrace: record sendDone/receiveDone/unreadAtSendDone if reached. DiscriminationCheck (C) on perItemGrace trace under shared-batch rule: condition is Breach(trace) exists and persists > 0.2s of shared sendTime, computed over whole trace. If met: Enforced must report violation and perItemGrace must not have closed there. If unmet after 3 repetitions: “inconclusive” with trace; logic covered by PS5/PS5c.
  - (vii-b) Credit covers pause (C): 4 MiB Blob response. Reader reads 2 MiB at maximum speed, pauses 10s, then continues. Condition throughout measured pause: A(t) ≥ 65536·sendTime(t) at every instant. If met: any pause-time closure fails; sendDone/receiveDone required. Otherwise record Breach(trace)/closure time; only Enforced gates. Record A0/sendTime0 measurements.
  - (vii-c) Credit exhaustion: (vii-b) with 60s pause. Closure time measured. Enforced: if Breach(trace) persists > 0.2s, close within ≤ 0.2s of it; otherwise must not close. Record breach-to-closure difference.
  - (viii) Near-16 MiB Blob batch, read 65 KiB/s (C under RG5): sendDone/receiveDone, batchSendMaxS ≤ 258.2791015625, total through sendDone ≤ 420s, sendOverrunMaxMs ≤ 200.
  - (ix) rpc-fault:pacerOff, reader 1 KiB/s → BATCH_WALL closure at 420s±0.2, batchWallHits=1. Release build has batchWallHits = 0 throughout RG.
  - (x) Two 20-member batches within one second on one connection → second gives 20 −32021 {rate} without execution; first completes.
  - (xi) In RG14/RG4/RG5: pacerLateMaxMs ≤ 200, sendOverrunMaxMs ≤ 200.
  - (xii) Repeat (v), (vii-c), (0b) with rpc.pacerTrace=false: same qualitative outcome, CloseOK from lastPacerClose, matching sendDoneCount. Any difference recorded as tracing defect.
- RG15: pocol_getLogs on each dense block separately (50..1073). scratch ≤ 4 MiB, zero Limit{scratch}; otherwise reduce LogCursor consumption before M7.

Failure:
- Any of these blocks M7: violation of any G in RG1–RG8/RG10–RG15, violation of TC-send/TC-recv/CloseOK/Enforced in any run, or a C case whose condition holds but expected result fails.
- RG9 is measurement.
- Inconclusive C cases disclosed with values, do not block M7 because logic covered by PS.
- Timing measurements repeated 3 times; criterion applies to worst.

**NX (G)**

Baseline:
- ChainFixture on sv-fix through 9, vector at h=10. Signed NX0–NX4 from w with [tx1]; fallback NX5/NX5c with 0xc2c0c0.
- sv-fix-v2: baseline through 49; NX6/NX8 at 50, NX7 at 49.
- Deep baseline through 19; NX0d/NX1d at 20.

Paths: (a) gossip, (b) sync, (c) template, (d) MaliciousRpc in RP.
- h=10: b=1, from=1, count=10. h=20: b=8, from=7, count=14. h=50: b=38, from=37, count=14.
- Clock in (d) = head ts+5s; target_g=2^240, message mentions 4095.

Table (firstReject / diagSet / notEvaluable):
- NX0: (a)/(b) accept. (d) display / ∅ / {10–17}, label “10 headers back to genesis”, bound 4095.
- NX0d: (d) request (7, 14), “13 headers”.
- NX-T: (c) lock / ∅ / {6–9, 13–17}.
- NX1 (chainId=777001): (a)/(b) netChain/{netChain}. (c) netChain with {6–9, 13–17}. (d) netChain and 4901.
- NX1d: (d) netChain and 4901.
- NX2: NX1 with netGenesis.
- NX3 (v=2)/NX4 (v=0): NX1 with netVersion.
- NX5: (a)/(b) netChain/{netChain, 12}. (c) templateForm. (d) netChain.
- NX5c: (a)/(b) accept. (c) templateForm. (d) display.
- NX6/NX7: netVersion on all paths.
- NX8: accepted; (d) request (37, 14), “13 headers”.
- NX9 (h=11 on NX1): (a) unknownParent then ancestorInvalid, diagnoseChain = [NX1: {netChain}, NX9: ∅], notEvaluable = {2–17}. (b) ancestorInvalid and ban. (c) parentInvalid. (d) reject at NX1, 4901.

RP window:
- NX-V1: remove header 1 at h=10 → viewIncomplete, 4901.
- NX-V2: parentHash(1)=keccak256('other') → viewGenesis, 4901; (a) unknownParent.
- NX-V3: remove reference 7 at h=20 → viewIncomplete.
- NX-V3b: reference 7, chainId=777001 → netChain.
- NX-V3c: reference 7 with correct identity/timestamp, powHash > target → display “13 headers”.
- NX-V3d: 15 headers (6..20) → viewIncomplete.
- NX-V3e: reference timestamp g_ts+2^62 → viewFuture, zero ASERT, no exponent-proportional allocation; worker ping response ≤ 100 ms.
- NX-V3f: clock g_ts+10^6, reference timestamp C−200, headers 8..20 spaced 10s with trivial work → viewTargetCeil on 8 without powHash, 4901.
- NX-V3g: headers 8..20 with honest timestamps, targets exactly 2^244, powHash ≤ 2^244 → display “13 headers”, bound 4095.
- NX-V4: honest node at h=0 → viewNoBlocks, zero pocol_getHeaders, no frame, 4901, zero transactions. After block 1 within ≤ 30s: n=1 “back to genesis”, “No site”.
- NX-V5: false eth_blockNumber=0 → NX-V4 outcome.
- NX-V6: −32021 four times → 3 retries, then viewIncomplete, 4901.
- LN0: LN at h=0 → “No site” without viewNoBlocks; tx1 enters mempool.

Common expectation for every rejection: head, work, stateRoot, mempool and templates bytewise unchanged. pocol_getRejects records it; peer penalized. In (c), FB at T_j; valid result accepted by second node. Rust/TS agree.

**HG (G)**
- head=0: HG1 (0,1) fromZero, HG2 (1,0) countRange, HG3 (1,513) countRange, HG4 (1,1) fromAboveHead, HG5 (0,0) fromZero, HG6 (0,513) fromZero, HG7 (5,0) countRange, HG8 (1,512) fromAboveHead.
- head=10: HG9 (5,10) beyondHead, HG10 (10,1) one header, HG11 (1,10) ten, HG12 (11,0) countRange, HG13 (18446744073709551615, 512) fromAboveHead without overflow.
- Syntax: HG14 from='0x01', HG15 '0x10000000000000000', HG16 one parameter; all −32602.
- Criterion: zero state reads/admissions in HG1–HG3, HG5–HG7, HG14–HG16; count ≤ 14 in every client request.

**Remaining phase B**
- T3: 4 nodes, 10k blocks, matching roots.
- T20 (FSM1–FSM10), T20-NX (FB at T_j), T23 (N1–N8), defined in VA annex.
- T5: zero duplicate nonces among honest miners [PDF:73].
- T6: burning the last active miner stops the chain.
- T17 monitor, T21 synchronization through closing, T25 (VA annex).
- T14 (8 GB): RSS ≤ 3 GB, 10k synchronization ≤ 30 minutes, validate_p99 ≤ 3s.
- T11b: LN rejects tampering. T11c: RP labels n/minWorkRP.
- T15: manual MetaMask; record any failure caused by eth_getLogs limit.
- BR15 (see BR).

**Statistical (va/base without adversary, window 201..3200, VA framework)**
- T4-H (truncation, S): for each (h, a, i), honest node i owning a range in attempt a:
  - trunc = 1 if attempt closes by transitioning to a+1 (C > U(a)+Γ) before i scans its whole range.
  - T4-H = Σtrunc ÷ triple count. Accept: CI upper bound ≤ 0.02.
- T4-W (S): for each (h, i), W = 1 if i's first-hash time on h minus parent-arrival time at i exceeds X^W.
  - T4-W = mean W. Accept: CI upper bound ≤ 0.05.
- T4-L (S, D121): proportion of seeds reaching quiescence height before T_RUN_MAX.
  - Accept: CI lower bound ≥ 0.95.
  - Every stall disclosed with its TraceDB trace.
- T4-B as defined above.
- T8 (S): seed-paired arms: va/base (nonceMode=0, c=2), and va/base with nonceMode=1.
  - Metric: p95 intervals of blocks 201..3200 at N0. T8 = p95_bounded ÷ p95_unbounded.
  - Accept: CI upper bound ≤ 1.5. Fail: lower bound > 1.5, requiring larger c before M8.
  - Post-range idle time recorded as measurement.
- T7, T7b, T7c, T7e, T9, T10, T10b, T19, T25, T18b: definitions/thresholds in VA annex.
- T8 (above), T13 [PDF:117], T18 ≤ 33 GiB.
- T4-V measures proportion s ≥ 4.

**VA annex: consensus, incentive and retention tests (D115)**

Status: proposed, not executed. Scripts/literal message tables are in VA-spec; the following is binding on them.

Common framework:
- SimNet (D44) runs real pocold modules with a virtual clock over NetSim.
  - Each link delay U[50 ms, Δ], seed derived from bᵢ.
  - Script commands: partition(A, B, t0, t1), drop(link, kind), delay(link, ms), kill(node), restart(node), wipe(node).
- va/base profile (simnet): P11 with nonceMode=0, c=2, chainId=777920, H_END=4000, H_CLOSE=3840, Z_close=160.
  - target_g = floor(2^256/14720)−1, i.e. H_tot·T_blk = 14720 honest hashes.
- Honest miners: 16 identities, one node each, rates {8,8,4,4,4,4,2,2,2,2,1,1,1,1,1,1} units (total 46). One unit = 32 real SHA-256 hashes per virtual second.
- Adversary: Adv nodes, rate H_adv, β = H_adv/(46+H_adv). Values: 5 → 0.098, 12 → 0.207, 16 → 0.258, 20 → 0.303.
  - Controls its node links, signs any message with its keys, chooses what/to whom/when to send.
  - Cannot break cryptography (A1) or control honest-to-honest links except declared partition commands.
- Window and termination (D117):
  - Warmup 200 blocks; measurement on N0 canonical blocks 201..3200.
  - Quiescence: first instant N0 reaches quiescence height (3260, or 3600 in T7e):
    - Apply measurement barrier (D122) below: stop adversary, fault injection and honest mining, drain to Quiet, then HeadCheck.
    - Post-quiescence measurements use state after Quiet, not a fixed-time instant.
    - No fixed 300s window with ongoing mining.
  - Run limit: T_RUN_MAX = 3·(quiescence height)·T_blk virtual seconds, 97800s at 3260. Seed where N0 does not reach height before limit is “stalled”.
  - Stalled seed is not discarded:
    - In G gates (D121): stalled seed not evaluable for quiescence-conditional G parts; counted/disclosed.
      - Gate fails if G violated in any evaluated seed.
      - Also fails if stalled count exceeds 2 of 24 or 4 of 48, because T_RUN_MAX is three times expected duration; repeated stalling is a progress defect.
      - Otherwise judge evaluated seeds.
    - In S gates: censored value from CEN below before any between-arm arithmetic.
    - In measurements: separate disclosed category.
  - Zero denominator: any metric with zero denominator in a nonstalled seed takes its CEN censored value, counted as clipped.
    - underpowered label only for an arm whose test explicitly requires a minimum event count.
    - Such arm judged “inconclusive”, not passed.
  - CEN (D121): every per-seed S metric clipped before aggregation to finite [lo, hi]; censored value is worst endpoint:
    - Ratios (T9, T19, T10b(i), T10b(iii)(b), convergence ratios, T4-L): [0, 1], worst 0.
    - T4-H/T4-W: [0, 1], worst 1.
    - Δ_k/g: [−1, 1], worst 1.
    - ρ_i: [0, 2], worst 2, outside TOST range.
    - T8/T4-B (mean ÷ T_blk): [0, 10], worst 10.
    - Computed out-of-range values clipped and counted as clipped.
    - Paired arms computed within seed first. If either stalls or any denominator is zero, paired metric directly takes censored value, with no arithmetic on nonfinite values. Thus no ∞, ∞−∞ or NaN.
  - Excitation: event-conditional tests (T7e, T10b(iii)) mark a seed “unexcited” if event absent within specified range.
    - Pass only if ≥ 20 of 24 or ≥ 40 of 48 seeds excited; otherwise inconclusive.
    - Any violation in an excited seed fails.
  - Convergence: every VA “single head within x” or “adoption within x” criterion has two parts:
    - (G) HeadCheck after measurement barrier (D122). Neither matching heads at a fixed instant nor a tie-resolving block required.
    - (S) t_conv = first instant ≥ reference-event time when all honest heads match, during active run before barrier.
      - Equal work with different heads is not convergence.
      - Per-seed indicator 1 if t_conv − event time ≤ x.
      - No t_conv before barrier → 0 from CEN.
      - Accept if CI lower bound ≥ 0.95; time itself measured.
    - For event-conditional tests, ratio computed on excited seeds. No probabilistic timeout classified G.
  - Timing-criterion classification (D121):
    - Convergence by above rule: T7e(a), x = 30·T_blk; T10b(ii), x = 30·T_blk; T10b(iii)(c), x = 30·T_blk; T10b(vi), x = 60s.
    - T23 N6: (G) fetch from another peer completes before script end. (S) completes within 3Δ of rejection.
    - T4-L: (S) as below.
    - Only deterministic VA timings under G:
      - T20 with fake clock.
      - T25 with fixed timers: ReplicaMonitor check every 10s, timeout 2s; extension polls pocol_getRetention every 10s; worst 22s ≤ 30s.
      - Queue/rollback bounds (≤ R_keep), because state constraints, not time.
- Nodes and halves:
  - N_i for identity i in rate order above, including N0=8/N1=8.
  - A = even nodes {N0, N2, …, N14}, B = odd nodes; 23 units each.
  - This partition used in T10b(ii, iii) and T7e.
- Statistics (type S, D121):
  - Seed is the unit: 24 seeds, then 48 by adding b25..b48.
  - Estimator: mean per-seed values after CEN. For metrics defined within seed as a ratio or p95 (T8, T10b(i)), that metric is the per-seed value.
  - CI: percentile bootstrap, 10^4 resamples.
    - Each resample draws n seed indices with replacement and computes mean.
    - Compared arms share indices.
    - Linearly interpolated percentiles (numpy type 7): two-sided 95% CI = [2.5, 97.5], T7 TOST = [5, 95].
  - Generator: PCG64 seeded by first 8 bytes of SHA256('PoColBoot'‖testId‖armId‖be32(n)), big-endian integer.
    - Python reference BootRef is authoritative.
    - TS implementation matches bytewise on VA-spec vectors.
  - Acceptance requires both:
    - Relevant CI bound meets criterion.
    - Seeds clipped due to stalling/zero denominator ≤ 2 of 24 or ≤ 4 of 48.
  - Failure: opposite bound outside criterion.
  - Otherwise inconclusive after 48 seeds, including excess clipped seeds despite relevant bound meeting criterion; disclose reason.
- Source of truth: NetSim log of every message/fault, N0 state and per-seed TraceDB.
- Measurement barrier and HeadCheck (D122):
  - Barrier: at quiescence height, or script end for tests without one, execute within one virtual instant:
    - halt: disconnect adversary nodes, stop fault injection, heal partitions, set all links to 50 ms.
      - (D124) Drop every adversary message not delivered before barrier; no adversary byte arrives afterward. Honest messages scheduled before barrier delivered on schedule (≤ t_halt+Δ).
      - Every honest node killed without wipe restarted at same instant, completes within VA-spec's fixed RESTART_MAX. Wiped node starts empty.
    - stopMining: each honest node stops hashing, sending new templates, building fallback blocks and shares.
      - Receiving, verification, import, broadcast and sync continue.
      - SimNet fixture command, not protocol rule; no validity change.
    - A block discovered before stopMining and being broadcast delivered normally.
  - Drain (D123): run until Quiet, 2 consecutive virtual seconds satisfying both:
    - ChainWork (below) empty at every honest node.
    - None of the following:
    - In-flight message between honest nodes.
    - Pending sync/GetBodies request between honest nodes.
    - Block whose parent is known to an honest node but verification incomplete.
    - Header in unknownParent whose parent is known to some honest node.
  - ChainWork(i) (D123): all deferred work at honest i that might change its head/state without a new message. Includes:
    - Future-queue entries (ts > C+Φ, consensus §3).
    - Block/header in unknownParent whose parent is known to some honest node.
    - Running/waiting verification, import, reorganization or TrieRebuild.
    - Sync/retry/body-fetch timers scheduled toward honest nodes.
    - Honest Status advertising a heavier head with no ongoing fetch.
  - Registration: every deferred timer/queue in consensus/sync/import scheduled through ChainTimer, tag ∈ {chain, nonChain}.
    - No other timer used in those modules; enforced by M5 clippy disallowed_methods.
    - SimNet reads ChainWork from ChainTimer/node queues, not guesses.
  - statusSweep: fixture command using existing Status without protocol changes. At barrier, at t_avail within atomic U (D129), and after every head change during drain, every honest node sends Status to every honest node, leaving no heavier head unknown.
  - Future-queue policy, fixture policy rather than validity rule:
    - (F1) Wait for an entry imported by any honest node.
      - Lemma: SimNet clocks satisfy |C_j−t| ≤ φ (A3). If A imported it at t_A, ts ≤ C_A(t_A)+Φ ≤ t_A+φ+Φ.
      - Every honest B has C_B(t) ≥ t−φ, so entry becomes eligible there by t ≤ t_A+2φ = t_A+Φ.
    - (F2) Wait for an entry imported by no honest node whose earliest eligibility at any honest node is ≤ t_halt+2Φ.
    - (F3) Otherwise purgeFuture removes it from every honest queue atomically with pending descendants.
      - Record futurePurged and (ts−C) as measurements.
      - Equivalent to stopped adversary not sending it. Execute only when ChainWork contains nothing except F3 entries.
    - Eligibility (D124 correcting prior wording): F1/F2 bound temporal eligibility, not import.
      - F2 entry eligible at first honest node ≤ t_halt+2Φ, at all ≤ t_halt+3Φ after one imports it (F1).
      - Entries imported before barrier eligible at all by ≤ t_halt+Φ.
      - Import still requires body fetching/H-full and may take until DRAIN_MAX(run) below, or resolve through U/V.
  - Body availability (D124, fixture command, not validity rule):
    - Possession: holds(i, b) true if block b's body bytes are in honest i's persistent or pending-body store, matching txRoot, evidenceRoot and BODY_MAX. Stateless check.
      - Implementation retains a matching body for a known header until import/rejection/removal, within unknownParent/side-queue bounds.
    - t_avail: last delivery of an honest message scheduled before barrier, after restart completion. Thus t_avail ≤ t_halt+max(Δ, RESTART_MAX).
    - Closure lemma: no new body source after t_avail because adversary disconnected, undelivered messages dropped, honest nodes not producing.
      - Body set therefore cannot grow after t_avail. Nondecrease does not follow from closure alone; ensured by DrainPin.
    - DrainPin (D125, SimNet fixture command, not protocol rule):
      - Phase 1 (D126, HaltPin): in halt's same atomic step before subsequent events, each honest node pins PinSet: every known header, held body, header version (hdrId) envOk/envPending in HeaderStore/EnvCache (D132), and every EnvCache envReject judgment (D136). D136 limits do not evict pinned judgments after barrier, so hdrId not rechecked during drain and envCheck ≤ Venv_i. Honest nodes do not forward rejected versions; no new rejection arrives after t_avail. Includes every structure: persistent store, pending-body store, unknownParent, side queue, future queue.
        - Every header/body arriving after t_halt added to PinSet on arrival, including pre-barrier scheduled honest messages and restart disk reloads.
        - No pinned element evicted: neither D118, queue bounds nor pending-body eviction. Arrival to a full structure accepted over limit and counted in pinBytes, without evicting others.
        - PinSet bounded: adversary disconnected/messages dropped; arrivals only from honest nodes. Thus |PinSet| ≤ structure contents at t_halt + honest in-flight messages at t_halt + restart-loaded contents. Record pinBytes/pinEntries; PIN_MEM_MAX unchanged.
      - Phase 2 (D126, narrow): at t_avail compute Rec, apply U and release pins in one step. Release all outside Rec, already removed by U; retain Rec pins until Quiet.
      - Only exceptions: V/F3, both atomically remove block from Rec at every honest node.
      - Each refused eviction increments pinRefusals. Pinned memory recorded as pinBytes.
      - Exceeding VA-spec's fixed PIN_MEM_MAX makes seed unevaluable within D121 quota; disclosed.
    - Possession lemma (D126, replaces D125 wording): no PinSet eviction between t_halt/t_avail; no Rec eviction between t_avail/Quiet except V/F3.
      - Thus HeldAtHalt(b) = {i : holds(i, b) at t_halt} ⊆ holders(b, t_avail).
      - Likewise (D132): EnvHeldAtHalt(b) ⊆ holders of a validEnv version of b at t_avail, because envelopes are in PinSet.
      - Every block meeting three conditions lies in Rec: an honest node held its body at barrier, an honest node (possibly another) held its validEnv envelope, and parent in Rec or genesis.
      - Body-held block without validEnv at any honest node is outside Rec due to envUnavailable; not loss of possession.
      - Rec stable after t_avail except V/F3 removals.
    - pinBreach (D126, G): HeadCheckRef computes HeldAtHalt from the NetSim log independently of DrainPin. Any bodyPurged block due to bodyUnavailable with HeldAtHalt(b) ≠ ∅ fails G.
      - (D132) EnvHeldAtHalt(b) = {i : i holds version h of b at t_halt with successful envPre(h)/envOwn(h, S(parent))}, computed by HeadCheckRef from log/RefReplay. Any bodyPurged block due to envUnavailable with EnvHeldAtHalt(b) ≠ ∅ fails G. U cannot conceal eviction between barrier and t_avail.
    - Rec (D125): recoverable block set computed by HeadCheckRef from NetSim log at t_avail; smallest set satisfying:
      - b ∈ Rec if any honest node imported it.
      - Or if all four hold:
        - An honest node knew its header.
        - An honest node (possibly another) held its body.
        - An honest node (possibly a third) held a valid-envelope header version, validEnv(b) (D131).
        - Parent in Rec or genesis.
      - validEnv(b) (D131): HeadCheckRef computes from log. Requires version h held by an honest node at t_avail with successful envPre(h)/envOwn(h, S(parent)), parent state from RefReplay. An honest-imported block satisfies it by construction; version holder pinned by HaltPin.
      - Remove every V-rejected or F3-removed block and descendants from Rec.
    - Path U (unrecoverable): at t_avail apply purgeUnavailable to every honest ChainWork entry for b ∉ Rec, including future/unknownParent queues and fetch requests.
      - Reason bodyUnavailable if no honest node holds b's body.
      - Or envUnavailable (D131) if body held but no honest node holds validEnv.
        - Fixture exclusion, not blockReject: no pocol_getRejects, parentInvalid or honest-peer penalty; descendants removed as depUnavailable.
        - pinBreach covers every envUnavailable whose block had a validEnv holder at t_halt.
        - Live pocold counterpart is only envWait (consensus).
      - Otherwise depUnavailable, missingAncestor = nearest ancestor outside Rec.
      - Atomically remove entry at all honest nodes, with pending descendants, requests and timers.
      - Record bodyPurged and hash of each removed entry.
      - Equivalent to adversary not sending header, because header work uncounted before H-full (D38).
      - Never remove a Rec entry or any honest-imported block.
      - Entry with honest-held body but ancestor outside Rec removed as depUnavailable because import needs parent state.
      - Fixture exclusion recorded in bodyPurged{hash, reason, missingAncestor?}, closed reason ∈ {bodyUnavailable, envUnavailable, depUnavailable} (D132); missingAncestor required only for depUnavailable.
        - None of three reasons is consensus invalidity: no pocol_getRejects, parentInvalid, ancestorInvalid or honest-peer penalty.
        - Penalty for sender of corrupt envelope at envReject is separate, imposed at checking, not exclusion.
    - Path A (recoverable): Rec entry remains until import/rejection. Failure to import before t_halt+DRAIN_MAX(run) is a sync defect and G failure, not blocking.
    - Path V (rejected, D125, restricted to D130 blockReject): header H-pre rejection or H-full rejection of a body matching header commitment, through blockReject violation, before/after eligibility, removes in the rejection step:
      - Entry.
      - Descendants through parentInvalid/ancestorInvalid.
      - All its timers/requests from ChainWork.

      Node implementation behavior, not fixture command; recorded in pocol_getRejects. Each ChainWork entry keyed by its block hash, followed by cancellation.
    - EnvReject (D130): envelope violation (envPre/envOwn) drops version and penalizes sender. Header, entry, descendants and requests remain, without RejectPath.cancel/HfullDone; fetch alternate through EnvFetch (network, D131). Without any honest-held valid version, block outside Rec as envUnavailable.
    - BodyMismatch (D125): body response mismatching header txRoot/evidenceRoot is a response error, not block invalidity:
      - Drop response, penalize/ban source under P2P limits.
      - Retain header, entry and request; immediately retry another holder (N5).
      - Nothing for the block in pocol_getRejects.
      - Body matching roots but violating item 15 (e.g. BODY_MAX) or 17 is block invalidity through V.
    - NotFound immediately moves request to another honest peer, without waiting SYNC_RETRY_MAX.
  - Orphan headers whose parent is unknown to every honest node do not prevent Quiet.
  - DRAIN_MAX(run) (D124) computed per run from TraceDB. Failure to reach Quiet before t_halt+DRAIN_MAX(run) fails G.
    - Formula (D125, definitions amended in D126): DRAIN_MAX(run) = max(Δ, RESTART_MAX) + 3Φ + SYNC_RETRY_MAX + Σ_{i∈H} (|Need(i)|·c_b(i) + HR_i + RB_i) + 2s. Summing serially across nodes is conservative. Definitions:
      - HR_i (D131, replaces D128/D129 formulas) = (|Kmiss_i| + (1+S_i)·(|H|−1))·RTT_i + (|Kmiss_i| + |Kpend_i|)·c_h + Venv_i·c_env + |Henv_i|·(|H|−1)·RTT_i.
        - Henv_i (D131) = headers x ∈ Kmiss_i ∪ Kpend_i ∪ Need(i) for which i holds no envOk at t_avail. Upper bound, because GetHeaders arrivals need no EnvFetch.
        - First term: productive/nonproductive FetchSched requests.
        - Second: block-level header checking.
        - Third: envelope checking, each hdrId once.
        - Fourth: productive/nonproductive GetEnvelope requests (network, EnvFetch, D131).
        - Venv_i = Σ V_x over x ∈ Kmiss_i ∪ Kpend_i ∪ Need(i). V_x = distinct header versions (hdrId) of x at honest nodes at t_avail, read from log.
        - Version lemma: no new version after t_avail because:
          - Adversary disconnected.
          - Honest nodes do not sign (stopMining); forward received bytes unchanged.
          - Each hdrId checked once.
        - c_env = c_rec + c_own (D130): c_rec recovers both signatures (envPre), c_own computes nonce owner from parent state without execution (envOwn). Both in SimParams. Earlier ⌈|Kmiss_i|/512⌉ bound withdrawn because overlapping peer responses do not ensure it. Definitions:
        - Kpend_i (D127): headers known by i at t_avail with incomplete H-pre, in sync, unknownParent or receive queues. Kpend_i ∩ Kmiss_i = ∅.
        - HdrCache (D128, amends D127, node behavior not validity rule):
          - H-pre has two phases:
            - Hs: items 1/1b, stateless.
            - Hp: items 2–9, requiring parent header.
          - (D129, amends D128) Two-key caching:
            - BlockCache keyed by blockHash: only completed committed-content judgments, valid or blockReject{rule}.
            - EnvCache keyed by hdrId: envelope judgments envOk/envReject{rule}, under consensus classification.
            - Check distribution:
              - Hs includes stateless envelope checks (signature parts of 3/8).
              - Hp retains committed parts of 2–9.
              - Signature part of 13 is envOwn (D130): checked with parent state without execution, final judgment cached by hdrId in EnvCache. Before parent state, envPending without judgment.
              - Execution once per blockHash after first envOk, result in BlockCache.
            - envReject writes no BlockCache entry and does not cancel descendants. Block awaits another version.
          - notEvaluable/unknownParent are not cached judgments. Header waits in unknownParent with Hs result; Hp evaluated once parent header available.
          - inFlightHdr (D129), two keys: hdrId for envelopes, blockHash for content.
            - Different-envelope versions not coalesced.
            - Byte-identical versions coalesced.
          - Hence per blockHash ≤ one committed evaluation per phase, combined cost c_h; per hdrId ≤ one envelope check, cost c_env (D129).
        - FetchSched (D128, node behavior not validity rule) enforces fetch bounds:
          - DiscRun: at most one header-discovery run per node.
            - Status advertising unknown/heavier head during a run sets dirty, starts no new run.
            - Run finishing dirty starts a new run recomputing unknown advertised heads.
            - Thus run count R_i ≤ 1+S_i.
          - Within run: peers ordered by ID. For each j advertising unknown head, send GetHeaders; anchor first request at locator, then at last header received from j.
            - Productive response contains ≥ 1 hash unknown at receipt.
            - Nonproductive response (empty, NotFound, all known) adds j to run's tried; do not request tried peers.
            - Each new post-t_avail hash lies in Kmiss_i; subsequent productive requests ≤ |Kmiss_i|, nonproductive ≤ R_i·(|H|−1).
          - BodySched: at most one ongoing body request per block hash, with triedPeers.
            - NotFound/BodyMismatch adds peer to triedPeers and immediately moves request.
            - New discovery does not re-request a hash with an active request or visit triedPeers.
            - Reeligibility (D129/D130): triedPeers not permanent exclusion, but reeligibility does not clear it.
              - Status from j marks it reeligible for all hashes, retaining tried for current cycle.
              - j announcing x/descendant, or expiry of min(1s·2^k, 60s) after j's kth NotFound for x, marks j reeligible for x only. Expiry alone sends no request.
            - Strict selection: untried this cycle by ID, then reeligible by announcement order. No reeligible request while x has an untried peer this cycle.
            - Attempts end when header removed by sync-queue limits or D118.
            - Drain lemma:
              - BodySched.cycleReset in atomic t_avail step clears tried/reeligible/timeout timers for all hashes before any request. Subsequent statusSweep does not clear tried.
              - Every x ∈ Need(i) has holder h* at t_avail, pinned until Quiet, replying Ok within RTT_halt.
              - h* untried in new cycle. Each x request visits an untried peer; reaches h* within ≤ |H|−1 requests. Honest nonholder replies NotFound, never BodyMismatch. No request after Ok.
              - A hash learned by i after t_avail starts with empty cycle, same argument.
              - Thus after t_avail: each (hash, peer) ≤ one request, each hash ≤ |H|−1 requests, zero reeligible-class requests. Status/announcements during drain may mark reeligible without requesting (D130).
            - Bound applies to closed drain. On live network, peer count/exponential backoff bound load, tested by T23 N9.
          - HfullDone (D130): settled content (import/blockReject) not reevaluated. envReject does not set HfullDone; next version checked. Reorganization reexecution classed reexec, not hfull.
          - Fetch ordering: greatest advertised-work branch, then oldest announcement. Ordering irrelevant to bound, which holds for every order.
          - SimNet evicts no entry before Quiet. pocold bound is sync-queue limit (16384).
          - Thus hdrCheck per i after t_avail ≤ |Kmiss_i|+|Kpend_i| distinct hashes, each once.
        - K = all headers known by any honest node at t_avail after U. No new header afterward: production stopped, adversary disconnected.
        - Kmiss_i = K headers unknown to i. F3-entry headers remain in K if known before t_avail; counted, not excluded.
        - S_i = discovery triggers at i: one Status from each other honest node in first statusSweep, and one after each other honest head change. Add one Status from each honest node in t_avail statusSweep (D129). Thus S_i ≤ 2(|H|−1) + Σ_{j≠i} T_j.
        - (D128) Triggers coalesced into DiscRun runs, R_i ≤ 1+S_i; each costs ≤ |H|−1 nonproductive requests plus productive ones.
        - c_h: SimNet H-pre time per header.
      - RB_i (D126) = T_i·(d*_i·(c_rd + t_exec_sim) + [d*_i > K_eff]·T_REBUILD_MAX). Definitions:
        - T_i = |Need(i)| bounds i's head transitions.
          - Transition lemma: work uncounted before H-full (D38), so head changes only on new-block import.
          - Rejection changes no head because rejected block was never adopted.
          - Head is maximum under total order ≻ (D133). Keys fixed, so only newly imported block changes head (consensus §9 transition lemma).
          - Thus each import changes head at most once; transitions ≤ imports ≤ |Need(i)| regardless of branch-extension alternation.
        - d*_i = min(R_keep, Hmax_i − Lmin_i). Hmax_i highest height in Rec ∪ imported_i; Lmin_i lowest common-ancestor height of any two blocks there.
          - Each transition rolls back ≤ d*_i, then reapplies ≤ d*_i previously imported blocks. Hence t_exec_sim in factor.
        - Bound independent of “distinct branch” definition/count.
      - c_rd: SimNet reverse-diff rollback time per block.
      - T_REBUILD_MAX: measured TrieRebuild bound for simnet state, fixed in VA-spec. Exceeding it in a run fails G (rebuildOverrun), not a wait.
      - Actual transition requiring depth > R_keep not executed: HeadCheck reports Q5, deepReorgRequired, governance action 5. Therefore d*_i clipped to R_keep.
      - SYNC_RETRY_MAX: maximum timeout of an unanswered pre-barrier sync request.
        - Included once only.
        - After barrier, every honest-to-honest request answered within RTT_halt (default 0.1s in SimParams). SimParams (D127) is a single record jointly read by NetSim/drainBound/HeadCheckRef. Values (RTT_halt, c_h, c_rd, t_exec_sim, t_halt) may be overridden only by an explicitly declared QV vector, without validity changes. No waiting after NotFound; no other timeout reaches SYNC_RETRY_MAX.
        - Value fixed in VA-spec.
      - c_b(i) = (|H|−1)·(RTT_i+t_exec_sim) + 2·t_exec_sim (D131): worst per-block cost for honest i.
        - RTT(i,j) (D131): upper bound on post-barrier link request/response duration. Defaults to RTT_halt; per-link overrides in SimParams permitted; NetSim guarantees it for every honest-to-honest request.
        - RTT_i = max_j RTT(i,j). Only RTT value used in HR_i, c_b(i) and i's BudgetCheck bounds; never use RTT_halt directly in a bound. Covers each-peer attempts, verification and possible rollback.
        - t_exec_sim is SimNet virtual verification time, t_exec_sim ≤ t_exec.
      - N_miss = Σ_{i∈H} |Need(i)| (D126). Need(i) = Rec(t_avail) \ imported_i(t_avail): every Rec block not imported by i.
        - No path/work restriction; D126 removed D125's “work ≥ work(head_i)” restriction.
        - Includes lighter/side branches in ChainWork(i) and those entering after statusSweep.
        - Closure lemma: every i import after t_avail lies in Rec. By induction:
          - Body comes from honest holder, no other source.
          - Valid envelope comes from honest holder, hence validEnv (D132); no new version after t_avail.
          - Parent imported.
          - No new blocks. Thus Need(i) covers every possible import/rejection by i.
        - Set intentionally broader than necessary; conservative bound.
        - Includes restarted node's entire missing history even if its ChainWork empty.
        - Also satisfies three conditions:
        - Some honest node holds b's body or imported it.
        - i has not imported it.
        - It was not rejected.

        Bound sums pairs serially, accounting for chain dependencies.
      - DRAIN_CAP = 3600s. If DRAIN_MAX(run) exceeds it, seed “unevaluable” for this item, counted in stalled quota (D121). Disclose bound components: Need(i), HR_i, RB_i per i.
      - (D125) SimNet.drainBound must literally match HeadCheckRef in integer milliseconds every run; mismatch fails G. Detects implementation/formula disagreement only, not completeness.
      - BudgetCheck (D126, G, independent of formula): simnet-build ActLog records each honest node's post-t_avail activity and virtual duration. Classes: hdrFetch, hdrCheck, bodyFetch, hfull (import/reject), reorg, reexec, rebuild, eligibilityWait, preHaltRetry, notFoundHop. Every ChainTimer timer has class. Criteria:
        - Budget table (D127) per honest i. Each class: count, serial duration, DRAIN_MAX component:
          - hdrFetch (D128): productive GetHeaders ≤ |Kmiss_i|, nonproductive ≤ (1+S_i)·(|H|−1). Zero requests to tried peers in run; one active run at any instant. Duration ≤ request Σ RTT(i,j) ≤ count·RTT_i (D131); first HR_i term.
          - hdrCheck (including envCheck, D129):
            - Content: ≤ |Kmiss_i|+|Kpend_i| distinct blockHash, once each at c_h.
            - Envelope: ≤ Venv_i checks, each hdrId once at c_env.
            - envFetch (D131, ActLog hdrFetch subclass):
              - GetEnvelope requests ≤ |Henv_i|·(|H|−1).
              - Each (hash, peer) ≤ one request; zero reeligible requests.
              - Duration ≤ count·RTT_i; fourth HR_i term.
              - (D137) EnvSlots local per node. Thus while waiting for a slot, each occupied slot belongs to that node's own request/check. Wait ≤ sum of those durations, already serialized in third/fourth terms; no extra component.
              - After t_avail honest peer replies within RTT_i < ENV_REQ_TIMEOUT. SimNet requires RTT_i < ENV_REQ_TIMEOUT for every i, otherwise FixtureSetupError. Thus no drain timeout: timer measures receive only through last byte; later checking excluded (D138).
              - Cancelled checks hold slots until settlement. Each is one hdrId check at c_env, counted in third term; slot waiting adds no component.
              - Additional G assertion: envTimeouts = 0 after t_avail.
              - (D139) SimParams c_env bounds checking interval derived from EnvCheckPool. SimNet models W/c_exec; total pool occupancy per i ≤ Venv_i·c_env, counted in third term.
              - G assertion: zero envFetch requests without slots, solicitHeld ≤ 32.
            - Component: second/third HR_i terms.
          - bodyFetch/notFoundHop: requests/root checks ≤ |Need(i)|·(|H|−1), duration ≤ |Need(i)|·(|H|−1)·(RTT_i+t_exec_sim); first part of |Need(i)|·c_b(i).
          - (D128/D129) Additional post-t_avail bodyFetch constraints:
            - Each (hash, peer) ≤ one request.
            - One active request per hash at any instant.
            - Zero BodySched reeligible requests; zero requests for pairs already requested in cycle (D130).
          - hfull: ≤ |Need(i)| executions, each blockHash once (HfullDone), duration ≤ |Need(i)|·2·t_exec_sim; remaining |Need(i)|·c_b. envOwn not hfull; envCheck at c_env (D130).
          - reorg: transitions ≤ T_i, each depth ≤ d*_i, duration ≤ T_i·d*_i·c_rd.
          - reexec: ≤ T_i·d*_i blocks, duration ≤ T_i·d*_i·t_exec_sim.
          - rebuild: ≤ T_i·[d*_i > K_eff], each ≤ T_REBUILD_MAX. Last three classes together = RB_i.
          - Discovery triggers: ≤ S_i (count, not duration).
        - Parallel waits not summed; each interval measured on virtual clock:
          - restart: completes ≤ t_halt+RESTART_MAX; component max(Δ, RESTART_MAX).
          - eligibilityWait: last F1/F2 eligibility ≤ t_halt+3Φ; component 3Φ.
          - preHaltRetry: every pre-barrier request answered/timed out ≤ t_avail+SYNC_RETRY_MAX; component SYNC_RETRY_MAX once. Retries then counted in bodyFetch/hdrFetch.
        - Bound lemma (D127): t_halt-to-Quiet ≤ sum of three wait components + Σ_i serial durations + 2s. This is DRAIN_MAX(run); every component has one table row.
        - Zero unknown activity classes (unknownWorkClass), zero time unattributed to ChainWork.
        - (D131) Every honest-to-honest request after t_avail: measured duration ≤ RTT(i,j) ≤ RTT_i used in drainBound; otherwise G failure.
        - Every serial class: sum durations ≤ table duration bound. Every parallel wait: interval ≤ bound, without summing durations.
        - Any violation fails G even if Quiet reached within bound. Exposes missing formula terms even when parallelism conceals them.
      - Coverage lemma (D126, conditional on complete ActLog classification enforced by ChainTimer/BudgetCheck): every post-t_avail drain interval belongs to one of ten classes, each covered by formula. Verification, waiting and each-peer attempt serialized, so sum is upper bound.
      - DRAIN_CAP does not apply to QV vectors. Exceeding it there is FixtureSetupError stopping M5.
      - HeadCheckRef computes N_miss/DRAIN_MAX(run) through independent Python reference.
    - Rationale: network entirely honest, no production/fault/adversary; incomplete sync is a progress defect, not probabilistic event.
  - Quiescence lemma (D123, conditional):
    - At Quiet, ChainWork empty at every honest node, no post-barrier messages/production/faults.
    - Hence no event changes head/state; heads remain fixed.
    - Requires complete ChainTimer registration and cancellation of path-V entries upon rejection. Missing registration/cancellation detected by QV7–QV14/PostQuiet, not assumed absent.
  - Reachability lemma (D124, conditional on quiescence lemma and correct sync):
    - After t_avail/U, every remaining entry in Rec. Thus:
      - An honest node holds its body and each missing ancestor's body.
      - An honest node holds its validEnv and each missing ancestor's (D132).
      - Both possessions retained by DrainPin.

      Therefore imported by fetching/BodySched/EnvFetch within HR_i/c_b(i) bounds.
    - Thus every entry fetched/imported or rejected through V within DRAIN_MAX(run).
    - Failure to reach implies progress defect, not withholding.
  - PostQuiet (G): after Quiet run for QUIET_HOLD = 3Φ+60s, or per-QV duration. Any honest head/stateRoot change fails.
  - Evaluate HeadCheck at Quiet, repeat at PostQuiet end; results must match.
  - HeadCheck on honest set H after Quiet, all items G:
    - (Q1) W* = highest cumulative work among H heads. Every i ∈ H must have work(head_i) = W*.
    - (Q2, D133) If two honest heads differ: children of their lowest common ancestor along both branches have same type, signed or fallback.
      - Signed versus fallback violates D15, since signed precedes importSeq in key.
      - Legitimate difference recorded as tie measurement, not violation.
    - (Q2b, D133) Every honest i: head_i = ForkChoiceRef(imported_i, importSeq_i), computed from TraceDB.
      - Independent Python ForkChoiceRef computes maximum in three scan orders: insertion, reverse, blockHash.
      - Differing reference results stop M5 as reference error. head_i mismatch fails G.
    - (Q3) Every block held by two honest nodes has identical stateRoot/receiptsRoot/gasUsed. G at every instant, not just after Quiet.
    - (Q4) Every honest node's head state byte-matches RefReplay, a reference node importing only that head branch from genesis in order.
    - (Q5) Every honest rollback during run ≤ R_keep.
  - HeadCheck requires neither new block nor tie resolution; no PoW event placed under G.
  - Deterministic HeadCheckRef vectors gate VA; run by miniature SimNet script, not seeds:
    - QV1: N3 produces valid block 10 ms before barrier, link 50 ms. Heads differ at barrier. After Quiet all heads N3 block, HeadCheck passes, zero post-stopMining blocks. Reference records no G violation at barrier instant.
    - QV2: N0/N1 simultaneously produce equal-work fallback siblings, then barrier.
      - After Quiet heads may differ under first-seen rule.
      - Result: Q1/Q2 pass, tie = 1, zero G violations.
    - QV3: equal-work siblings, one signed, one fallback. After Quiet all heads signed.
      - Negative control simnet-fault:noSignedTiebreak leaves some on fallback, fails Q2.
    - QV4: simnet-fault:noReorgOnHeavier leaves a node on lighter branch, fails Q1.
    - QV5: simnet-fault:execDrift changes one transaction's gasUsed at one node, fails Q3.
    - QV6: adversary orphan header whose parent withheld from everyone does not prevent Quiet; queue remains bounded.
    - qv/net profile (simnet):
      - va/base with network timing block: T_blk=60, τ=3600, D_att=300, Δ_fb_wait=25 [CP], Δ_rel=26, Φ=10, φ=5, Δ=5, t_exec=3, Γ=30.
      - chainId=777922, target_g = floor(2^256/88320)−1. Passes R1–R7 with network values.
      - Rationale: Φ = 10 > Quiet window (2s), exposing queue defect hidden by va/base's Φ = 2.
    - QV7–QV10 setup:
      - Honest mining disabled throughout script; blocks from FixtureMiner.
      - P head at all honest nodes, ts=988, t_halt=1000.
      - Clocks: N3 = t+5, N4 = t−5, others = t.
    - QV7 (future block imported by some): fallback X on P at a=0, ts = P.ts+1+25 = 1014, delivered to all at 999.5.
      - N3: C+Φ = 1014.5 ≥ 1014, imports.
      - Others except N4: 1009.5 < 1014, queued, eligible t=1004.
      - N4: eligible at C_N4 ≥ 1004, i.e. t=1009.
      - Expected:
        - No Quiet before N4 import completes (≥ 1009).
        - HeadCheck passes, all heads X, futurePurged=0.
        - PostQuiet through 1100 unchanged.
      - simnet-fault:quietIgnoresFuture declares Quiet before 1009, fails Q1 (N3 on X, others P) and ChainWork-empty check.
      - simnet-fault:purgeImported removes X from queues despite N3 import, fails Q1.
    - QV8 (far future, imported by none): fallback X' on P at a=1, ts = L(1) = 988+301 = 1289, delivered to all at 999.5.
      - Maximum C+Φ = 1014.5, none imports. Earliest eligibility 1274 > t_halt+2Φ = 1020.
      - Expected:
        - purgeFuture when ChainWork first contains nothing else; futurePurged=1.
        - Quiet, all heads P.
        - PostQuiet through 1400 unchanged.
      - quietIgnoresFuture declares Quiet without removal; heads change to X' between 1274/1284 during PostQuiet, fails.
    - QV9 (child of future block): QV7 plus Y child of X, reaches all at 999.6.
      - Except N3, Y stays unknownParent, parent known by N3; prevents Quiet until importing X then Y.
      - Expected: all heads Y, HeadCheck passes.
    - QV10 (sync timer):
      - N5 on branch lighter by one block. Its first GetBodies to N6 dropped by script before t_halt; sync schedules 5s retry, value fixed in VA-spec.
      - Expected: no Quiet before retry/adoption completion, then HeadCheck passes.
      - simnet-fault:untaggedTimer schedules retry outside ChainTimer, declares Quiet early, fails Q1 or PostQuiet.
    - QV11–QV14 setup (D124):
      - QV7 setup plus AdvPeer: scripted adversarial peer connected to all honest nodes, disconnected at barrier t_halt=1000.
      - t_avail = 1005 because Δ=5.
      - FixtureMiner blocks with fixture keys, all H-pre valid. AdvPeer delivers only scripted data.
    - QV11 (complete withholding):
      - AdvPeer sends header Hx at 999.0, signed on P at ts=990, body [tx1]; at 999.1 sends child header Hy.
      - Replies NotFound to GetBodies; no honest node holds Hx/Hy bodies.
      - Expected:
        - purgeUnavailable at 1005 removes Hx/Hy from every honest node with requests/timers; bodyPurged=2.
        - Quiet by ≤ 1007, all heads P, HeadCheck passes.
        - PostQuiet through 1100 unchanged.
      - simnet-fault:keepUnavailable disables U; Quiet not reached before t_halt+DRAIN_MAX(run), fails G. Establishes gate discrimination.
    - QV12 (one honest holder): QV11, but AdvPeer delivers Hx body only to N7 at 999.2, withholds Hy.
      - Expected:
        - N7 imports Hx.
        - U removes only Hy (bodyPurged=1), leaves Hx.
        - Others fetch Hx body from N7 after statusSweep.
        - All heads Hx, Q4 passes.
      - simnet-fault:purgeHeld removes every entry not imported by its local node, dropping Hx outside N7, fails Q1.
      - simnet-fault:skipHolder prevents body requests to N7; Quiet not reached before t_halt+DRAIN_MAX(run), G failure as sync defect. U cannot conceal it because N7 holds body.
    - QV13 (H-full rejection):
      - AdvPeer sends Hz header/body to all at 999.0. Deliberately wrong stateRoot, valid signatures. Then sends child Hw header without body.
      - Expected:
        - Every honest node rejects Hz by item 17, Hw by ancestorInvalid.
        - ChainWork empty without purge, bodyPurged=0.
        - All heads P, pocol_getRejects records rejection.
      - QV13b: Hz signed at ts=1014, future as QV7.
        - Eligible on arrival at N3, 1004 at most nodes, 1009 at N4; then item 17 rejection.
        - Quiet not before 1009; bodyPurged=0, futurePurged=0.
      - simnet-fault:noCancelOnReject retains Hz/Hw fetch timers after rejection; Quiet unreachable, fails G.
    - QV14 (future without body):
      - AdvPeer sends Xf header at 999.5, signed on P at ts=1014, body [tx1], but delivers body to no honest node.
      - Expected:
        - U removes at 1005; bodyPurged=1, futurePurged=0.
        - Requests issued on N3 eligibility cancelled with it.
        - Then Quiet, all heads P.
    - QV15 (wrong body then correct, D125): QV11 setup.
      - AdvPeer sends Hb header to all at 999.0, signed on P at ts=990, body [tx1]. Correct body delivered to N7 at 999.2.
      - Replies to N3 GetBodies at 999.3 with [tx2], mismatching txRoot.
      - Expected:
        - N3 records BodyMismatch, penalizes AdvPeer, retains Hb, fetches body from N7.
        - All heads Hb; zero Hb pocol_getRejects entries; bodyPurged=0.
      - simnet-fault:rejectOnBadBody cancels Hb through V at N3, fails Q1/rejection-log criterion.
    - QV16 (unrecoverable dependency):
      - AdvPeer sends Pa header (on P, ts=990) at 999.0, then child Ca header.
      - Withholds Pa body from all; delivers Ca body only to N7 at 999.2.
      - Expected:
        - Pa ∉ Rec, hence Ca ∉ Rec. purgeUnavailable removes both at 1005.
        - bodyPurged = [{Pa, bodyUnavailable}, {Ca, depUnavailable, missingAncestor: Pa}].
        - Zero pocol_getRejects entries; zero honest-peer penalties.
        - All heads P, Quiet ≤ t_halt+DRAIN_MAX(run).
      - simnet-fault:excludeDepAsInvalid records Ca as parentInvalid, fails.
    - QV16b: QV16 with Pa body delivered only to N8.
      - Expected: Pa/Ca in Rec; fetch from N8 then N7; all heads Ca, bodyPurged=0.
      - purgeHeld/skipHolder fail.
    - QV17 (evict last holder):
      - QV12 setup: only N7 holds Hx body, in Rec and on heaviest branch.
      - At 1002 script executes evictProbe(N7, Hx), simnet-only hook invoking D118/pending-body eviction, simulating memory pressure.
      - Expected: eviction refused, pinRefusals=1, all heads Hx.
      - Additional criterion (D126): NetSim records N7 pinEntries including Hx from t_halt=1000, before evictProbe at 1002.
      - simnet-fault:noDrainPin evicts body; U classifies Hx bodyUnavailable despite HeldAtHalt(Hx) = {N7}, fails pinBreach. Does not rely on Quiet failure: U permits Quiet (D125 wording withdrawn for this reason).
      - simnet-fault:pinLate (D126) protects at t_avail rather than t_halt; permits 1002 eviction, same pinBreach failure.
    - QV18 (late restart):
      - N9 killed without wipe at P0. FixtureMiner then builds L = R_keep+100 = 2148 empty fallback blocks. Barrier at their end, N9 restarted there.
      - Expected:
        - |Need(N9)| = 2148; HR_N9 using D128 formula with D129 Venv bound is binding.
        - Productive GetHeaders response count measured, not criterion; approximately 5 batches of 512 expected.
        - Zero rollback; all heads chain tip; Q4/Q5 pass.
        - Quiet ≤ t_halt+DRAIN_MAX(run); drainBound = HeadCheckRef.
      - simnet-fault:drainUndercount computes Need from ChainWork alone, gives 0 for N9, fails match.
    - QV18b (rollback deeper than R_keep): N9 on private fallback branch R_keep+1 deep below fork, lighter than main.
      - Expected: no rollback. HeadCheck reports Q1/Q5, deepReorgRequired; expected vector outcome. Raise governance action 5.
      - simnet-fault:silentDeepReorg silently rolls back beyond R_keep, fails.
    - QV18c (TrieRebuild): N9 on private branch depth 40 > K_eff = 32, lighter than main.
      - Expected: rollback then TrieRebuild, rebuildOverrun=0, Quiet ≤ t_halt+DRAIN_MAX(run), bound includes RB_N9; Q4 passes.
    - QV19 (deferred lighter branch, D126): QV11 setup, t_exec_sim = 2s for this vector.
      - Main branch (D127): F then M1..M41 = P then S1/S2; all signed with fixture keys, ts = p.ts+1 (F.ts = 947, P.ts = 988), empty bodies, delivered to all.
      - AdvPeer at 999.0 delivers only N2 headers/bodies for side chain Ls1..Ls40 forking from F, same timestamps/bodies.
        - Ls1 shares M1's signed template but differs in nonce/winner; no equivocation evidence.
        - Weight lemma: ASERT depends only on (p.ts, p.h); equal timestamps per height imply equal work per height. Thus side branch (40 blocks) strictly lighter than main (43).
        - N2 verifies serially at t_exec_sim = 2s, imports at most 3 before t_avail = 1005; post-t_avail hfull_N2 ≥ 37 from TraceDB.
      - Expected:
        - No Quiet before N2 verifies all forty and stores them as side blocks under D118.
        - Honest heads unchanged.
        - Need for every honest node includes Ls1..Ls40; Quiet ≤ t_halt+DRAIN_MAX(run); BudgetCheck passes.
      - simnet-fault:needHeadsOnly computes Need only from heaviest-head paths, excludes Ls* from Need'(N2). Then hfull_N2 ≥ 37 > |Need'(N2) ∩ Ls| = 0, deterministic BudgetCheck failure. Quiet exceeding defective bound not required, measured.
    - QV20 (D128, late adoption of competing branches without D118 violation): qv/net, honest mining disabled, SimParams = {t_halt = 1000, t_exec_sim = 2s}.
      - Setup:
        - FixtureMiner builds chain through A0 for all honest nodes.
        - Then three partitions after A0 until t_halt: partition({N10}, rest), partition({N11}, rest), partition({N12}, rest).
        - Builds main A0→P with five blocks for others; feeds only N10 Fa (6 blocks), N11 Fb (7), N12 Fc (8).
        - Blocks signed by Draw keys, ts = p.ts+1, empty bodies; A0 children share one signed template. Equal work per height (QV19 lemma), weight proportional to length.
      - Retention: each node's own branch canonical, at most one side block per height; no pre-barrier D118 eviction. HaltPin preserves all afterward.
      - At barrier heal partitions; statusSweep advertises Fa/Fb/Fc. Rec includes all three; |Need(N4)| = 21, d*_N4 = 8.
      - Derivation:
        - Adopting a competitor requires serial H-full import of ≥ 6 of its blocks, ≥ 12s after first body arriving ≥ t_halt+RTT_halt.
        - Thus no N4 transition before t_avail = 1005; no completed heavier branch there then.
        - After Quiet N4 head Fc, uniquely heaviest (Q1). Thus post-t_avail transitions ≥ 1, depth ≥ 5.
      - Expected:
        - All heads Fc.
        - Each honest node's transitions ≤ T_i, every rollback ≤ d*_i; reorg/reexec/rebuild durations within RB_i.
        - Quiet ≤ t_halt+DRAIN_MAX(run); BudgetCheck passes.
        - Transition count (1–3)/order follow FetchSched, measured. No fixed timeline because each transition adds reorg/reexec counted in RB.
      - simnet-fault:rbPerBranch (bound 2·b, D125 wording): b = 0 at N4 by derivation, transitions ≥ 1 > 0, deterministic BudgetCheck failure.
      - Previous version (D127) below through QV21 withdrawn: do not execute/count as criterion. Violated pre-barrier D118 and omitted reorg/reexec from timing; remove from VA-spec. Text retained for traceability only:
      - Setup (D127): SimParams.t_exec_sim = 10s. A0 ancestor of P at depth 5. Main A0→P has five blocks.
        - Fa (6), Fb (7), Fc (8) fork from A0. All signed, ts = p.ts+1, empty bodies; four A0 children share one signed template with different nonces. Equal work per height (QV19), weight proportional to length.
        - AdvPeer delivers prefixes Fa1–5/Fb1–5/Fc1–5 only to N4 at 800. Verification completes ≤ 950; all equal main work; N4 stays P by first seen.
        - Only N4 holds tails Fa6, Fb6–7, Fc6–8.
      - AdvPeer delivers tails to N4 at 999.0 (Fa6), 999.1 (Fb6, Fb7), 999.2 (Fc6–8). Serial verification imports at 1009, 1019, 1029, 1039, 1049, 1059, all after t_avail = 1005:
        - 1009: Fa6 (6 > 5), first transition with rollback 5.
        - 1019: Fb6 ties, no transition.
        - 1029: Fb7, second transition with rollback 6.
        - 1049: Fc7 ties, no transition.
        - 1059: Fc8, third transition with rollback 7.
        - The other honest nodes follow via statusSweep from N4.
        - d*_N4 = 8, and T_N4 ≥ |Need(N4)| ≥ 6.
      - Expected:
        - N4 adopts Fa, then Fb, then Fc: three transitions after t_avail with rollbacks 5, 6, and 7.
        - Actual N4 transitions ≤ T_N4, and every rollback ≤ d*_N4.
        - Quiet within the bound, BudgetCheck passes, and all heads are Fc.
      - Negative control simnet-fault:rbPerBranch calculates the transition bound as 2·b (D125 formulation), where b is the number of complete branches at N4 at t_avail with work exceeding its head, and b = 0 here. Thus N4 transitions = 3 > 0, deterministically failing the BudgetCheck predicate. Exceeding Quiet is not required.
    - QV21 (many deferred headers, D126):
      - SimParams (D127, D128): t_halt = 100000, RTT_halt = 1s, c_h = 0.01s, and P.ts = 27800.
        - At 99999.0 AdvPeer delivers to N3 alone the headers and bodies of 1200 empty signed blocks above P, with timestamp spacing 60s = T_blk (within [L(0), U(0)]), making the last 99800. Δt is constant, so the target is constant and FixtureMiner costs about 1200·88320 hashes.
      - The other honest nodes learn of them only via statusSweep. RTT_halt = 1s is used in NetSim, the formula, and the reference together.
      - Expected (D128):
        - Kmiss_i and Kpend_i are read from the state log at t_avail, not assumed.
        - Lower bound: no honest node knows the headers before 99999.0, and c_h = 0.01s, so each honest node completes H-pre for ≤ 600 of them before t_avail = 100005. Thus |Kmiss_i|+|Kpend_i| ≥ 600 for every i.
        - hdrCheck after t_avail for each i = |Kmiss_i|+|Kpend_i| distinct hashes.
        - Quiet within the bound, and BudgetCheck passes.
        - If the measured sum is below 600 for any i, the result is FixtureSetupError.
      - Negative control simnet-fault:hdrUndercount calculates HR as ⌈|Kmiss_i|/512⌉·(|H|−1) requests, without the c_h term or Kpend. Thus the hdrCheck bound = 0 and the measurement ≥ 600, deterministically failing the BudgetCheck predicate. The former start predicate (≥ 15 > 3) was withdrawn because DiscRun merges starts (D128).
    - QV22 (known headers under examination, D127): qv/net profile, honest mining disabled. SimParams = {t_halt = 10000, c_h = 0.1s, c_h(N0) = 0.05s (D128), RTT_halt = 0.1s}, and P is everyone's head with ts = 2700.
      - (D128) The holder is honest, and this vector has no AdvPeer:
        - partition({N15}, rest, t_P, t_halt).
        - FixtureMiner feeds N15 alone 120 blocks signed with Draw keys above P, spaced 60s apart (last 9900, constant target).
        - N15 imports them canonically without a side branch, so there is no D118 eviction or unknownParent, and the bodies are held (Rec).
        - At the barrier the partition heals, and statusSweep announces N15's head.
      - (D128) Values are read from the state log at t_avail = 10005. Derivation for each i ≠ N15:
        - The 120 headers arrive in one GetHeaders response at ≤ 10000.2.
        - H-pre then runs at ≤ 20 headers/s for N0, and ≤ 10/s for the others.
        - Thus |Kmiss_i| = 0, and |Kpend_i| ≥ 24 for N0 and ≥ 72 for the others.
        - If measured Σ_{i≠N15}|Kpend_i| = 0, the result is FixtureSetupError.
      - Duplicate arrival is guaranteed: each honest node broadcasts the header of every block it imports (network, block receipt), so each header reaches every honest node from at least two sources.
      - N0 is faster and announces heavier heads during other nodes' ongoing body requests. This excitation is counted from TraceDB; its absence makes the noFetchDedup control FixtureSetupError.
      - Expected:
        - hdrCheck for each i = exactly 120 distinct hashes over the entire run, and after t_avail ≤ |Kpend_i|.
        - All heads are block 120, and BudgetCheck passes.
      - Negative control simnet-fault:kpendIgnored calculates HR using Kmiss alone, so the hdrCheck bound = 0 < |Kpend_i|, failing the BudgetCheck predicate.
      - Negative control simnet-fault:noHdrCache repeats H-pre on each arrival, repeating a hash in hdrCheck and failing the "each hash once" predicate.
      - Negative control simnet-fault:noFetchDedup (D128) requests each missing body again at each discovery start, disregarding in-flight requests or triedPeers. Under the excitation above, the pair (hash, N15) repeats, failing the bodyFetch predicate.
    - QV22b (header before its parent, D128): qv/net profile, honest mining disabled, t_halt = 1000.
      - Setup:
        - partition({N15}, rest) until t_halt, and N15 imports B1 then B2 above P.
        - AdvPeer sends the B2 header to N7 alone at 999.0; N7 performs Hs and B2 enters unknownParent.
      - After the barrier N7 fetches B1 from N15, performs Hp for B2 once, then imports B2.
      - Expected:
        - All heads are B2.
        - For B2 at N7, one Hs evaluation (envelope and items 1 and 1b) cached since 999.0, and one Hp evaluation after B1 arrives.
        - Zero Hp verdicts and zero blockReject for B2 in BlockCache before B1 arrives.
        - D129 corrects the "zero entries" wording, since caching the Hs result is allowed.
        - B2 is in Rec because its header is known, its body is at N15, and its parent is in Rec; bodyPurged = 0.
      - Negative control simnet-fault:cacheNotEvaluable caches unknownParent as a rejection. N7 then ignores B2's subsequent arrival and remains on B1, failing Q1.
      - Negative control simnet-fault:skipHp treats the Hs result as a complete H-pre verdict. ActLog then records zero Hp evaluations for B2, failing the predicate.
    - QV23 (D129, signature poisoning): qv/net profile, honest mining disabled, t_halt = 1000.
      - Setup:
        - partition({N15}, rest) until t_halt.
        - FixtureMiner feeds N15 block Bs above P, signed by Draw(h,0) with the nonce owner as winner, then its fallback child Cs. N15 imports both.
      - At 999.0 AdvPeer sends the other honest nodes a copy of the Bs header sharing its blockHash, with one of three corrupt envelopes:
        - (a) Unrecoverable winnerSig (item 8).
        - (b) sig omitted with a nonempty proposer (item 3).
        - (c) Valid winnerSig with AdvPeer's key (item 13 in H-full).

        It then sends the correct Cs header at 999.05.
      - After the barrier N15's correct copy arrives via statusSweep and GetHeaders.
      - Variant (t): the N15–N7 link heals at 998.9, so the corrupt and correct copies reach N7 at 999.0 and are checked concurrently.
      - Expected:
        - All heads are Cs, and BlockCache(Bs) = valid.
        - One envReject per corrupt copy, and an AdvPeer penalty.
        - Zero Bs or Cs entries in pocol_getRejects, and zero parentInvalid and ancestorInvalid.
        - In (c): envCheck fails in H-full before execution, then one envRefetch.
        - Bs and Cs are in Rec, and bodyPurged = 0.
        - Measured Venv = number of copies at t_avail, and BudgetCheck passes.
      - Negative controls:
        - simnet-fault:cacheByBlockHash caches envelope rejection under blockHash. The correct copy is rejected and nodes remain on P, failing Q1.
    - QV23d (D130, recoverable copies from nonowners): QV23 setup.
      - At 999.0, 999.01, and 999.02 AdvPeer sends three copies of the Bs header with the same blockHash, each with a valid winnerSig from a different adversarial key (a1, a2, a3), then the correct Cs header.
      - Variant (t): the three copies and the correct copy reach N7 together at 999.0 after the N15–N7 link heals at 998.9.
      - Expected for every honest node except N15:
        - Three envOwn = envReject verdicts, each hdrId once, and zero execution for any of them.
        - Execute Bs once after the correct copy, then import Bs and Cs.
        - Zero RejectPath.cancel, zero parentInvalid and ancestorInvalid, and zero entries in pocol_getRejects.
        - GetEnvelope requests ≤ |Henv_i|·(|H|−1), BudgetCheck passes, and all heads are Cs.
      - Negative controls:
        - simnet-fault:envRejectCancels applies path V to envReject, canceling Bs and rejecting Cs, failing Q1.
        - simnet-fault:hfullDoneOnEnv sets HfullDone on envReject, so the correct copy is not checked, failing Q1.
        - simnet-fault:execPerCopy executes the block before envOwn for each copy, making hfull for Bs = 4 > 1, failing BudgetCheck.
    - QV24 (D130, reeligibility during fetching): qv/net profile, honest mining disabled, t_halt = 1000.
      - SimParams (D131): RTT(link) = 0.1s by default, RTT(N3, j) = 1s for each j, and t_exec_sim = 0.05s. Thus RTT_N3 = 1s, and RTT_i = 1s for every other honest node too, because it has a link to N3. drainBound, HeadCheckRef, and BudgetCheck use only these values.
      - Setup: partition({N15}, rest) until t_halt, and N15 imports X1..X5 above P.
      - After cycleReset, N3 requests body X1 from N0, then N1, then N2, then N4..N14 (NotFound, 1s per request), then N15.
      - N2 fetches X1 over 0.1s links and imports it around t_avail+1.6, sending Status and announcing X1 before N3 reaches holder N15 (around t_avail+14).
      - Expected:
        - At N3: each pair (Xk, peer) ≤ one request, zero requests of class reeligible, and N2 in reeligible for X1 without a request.
        - Exponential timeout timers may expire, with zero resulting requests.
        - All heads are X5, BudgetCheck passes, and Quiet ≤ t_halt+DRAIN_MAX(run).
      - Negative control simnet-fault:globalRtt (D131) calculates drainBound with RTT_halt = 0.1s for every link. N3's first request has measured duration 1s > 0.1s, deterministically failing the request predicate in BudgetCheck, without requiring Quiet to exceed the bound.
      - Negative control simnet-fault:statusResetsTried (the literal D129 rule) clears N2 from tried upon Status. N2 becomes the smallest untried peer, so X1 is requested from it again, failing the single-pair predicate.
        - simnet-fault:mergeInFlight merges checks by blockHash, failing variant (t) in the same way.
    - QV25 (D131, invalid envelopes and alternative envelope fetching):
      - QV25a (no valid copy): QV11 setup.
        - FixtureMiner builds Bz on P with a nonce in N2's range, without N2's key.
        - At 999.0 AdvPeer sends all honest nodes the Bz body and three header copies, with winnerSig from a1, a2, and a3.
        - Expected:
          - Three envReject per honest node, and zero execution.
          - validEnv(Bz) is false, so Bz ∉ Rec. U removes it at 1005, and bodyPurged = [{Bz, envUnavailable}].
          - Zero entries in pocol_getRejects, and zero GetEnvelope requests after t_avail.
          - All heads are P, and Quiet ≤ t_halt+DRAIN_MAX(run).
        - Negative control simnet-fault:envAsBlockReject records exhausted copies as blockReject, creating an entry in pocol_getRejects and failing.
        - Negative control simnet-fault:noEnvUnavailable keeps Bz in Rec, so its entry remains after EnvFetch is exhausted, preventing Quiet within the bound and failing G.
      - QV25c (body and valid envelope at different honest nodes): qv/net profile, honest mining disabled, t_halt = 1000.
        - partition({N15}, rest) until t_halt.
        - FixtureMiner feeds N15 two siblings on P with equal work: signed Bm, and fallback Bv with winner N15. N15 imports both, its head is Bm (D15), and Bv is on a side branch.
        - AdvPeer, connected to everyone, sends N7 the Bv body and copy h_bad with winnerSig from a2 at 999.0.
        - Bv ∈ Rec because its header, body, and valid copy are at N15. N7 has a Bv entry with an envReject copy.
        - Expected:
          - N7 requests GetEnvelope [Bv, [h_bad]] in strict order, reaches N15 within ≤ |H|−1 requests, and receives the valid copy.
          - Execute Bv once, and store it on a side branch.
          - All heads are Bm, each pair (Bv, peer) ≤ one request, BudgetCheck passes, and Quiet ≤ t_halt+DRAIN_MAX(run).
        - Negative control simnet-fault:noEnvFetch does not request an alternative envelope. N15 does not announce Bv because it is not a head, and its pre-barrier broadcast is blocked by the partition. N7's entry remains, preventing Quiet within the bound and failing G.
        - Variant QV25c-dup: AdvPeer and AdvPeer2 send the same h_bad to N7 at 999.0 and 999.01. Expected: one envCheck for h_bad, penalties for both senders, and all other results as above.
    - QV26 (D133, transitive preference): qv/net profile, honest mining disabled, t_halt = 1000, and P is everyone's head.
      - Blocks (FixtureMiner, empty bodies):
        - X and Y are children of P sharing one signed template from Draw(h,0), with two nonces in different owners' ranges. One UT, so no type 1 evidence.
        - On X: fallback A2, and signed C2 with the same timestamp L(0)+Δ_fb_wait.
        - On Y: fallback B2 with the same timestamp.
        - Equality lemma: X and Y have the same timestamp and height, so the targets at h+1 are equal, hence W(A2) = W(B2) = W(C2).
      - Arms: every parent-compatible linear import order for the five blocks, numbering 5!/(3·2) = 20. Delivered to N0 in order, and each arm runs with SimParams.forkScanOrder ∈ {insertion, reverse, byHash}, an implementation parameter, not a rule.
      - Expected for each arm:
        - The head is C2 if importSeq(X) < importSeq(Y), otherwise B2. Never A2.
        - The head matches across all three scan orders.
        - Head transitions ≤ number of imports.
        - Q1, Q2, and Q2b pass.
      - Variant QV26m: N0 imports X before Y, and N1 imports Y before X. The heads are C2 and B2, and the X versus Y tie is between signed blocks, so tie = 1 without violating G.
      - Negative control simnet-fault:pairwiseTiebreak applies the old rule (signed at the first difference, then head importSeq) with sequential pairwise scanning. For order X, A2, Y, B2, C2:
        - insertion scan: A2 versus B2 → A2, then versus C2 → C2.
        - reverse scan: C2 versus B2 → B2, then versus A2 → A2.
        - The head varies with scan order, and head A2 violates ForkChoiceRef, deterministically failing Q2b.
    - Common criterion for QV11–QV25 with QV23d, according to the NetSim log:
      - Every entry removed by path U is outside Rec at t_avail, for only one of these three reasons (D132):
        - bodyUnavailable.
        - envUnavailable: the body is held, and no honest node has a validEnv copy.
        - depUnavailable with a correct missingAncestor.

        Any other reason, or a reason inconsistent with HeadCheckRef's classification of that block, fails G.
      - No entry in Rec is removed except by path V on a matching body or by F3, and the last holder of a block in Rec is never lost.
      - (D126) Zero pinBreach, and BudgetCheck passes.
      - Every Quiet occurs at ≤ t_halt+DRAIN_MAX(run) as calculated by HeadCheckRef.
  - simnet-fault builds are for testing only and are absent from the release.
- Immediate G failure in any run upon any of the following:
  - An honest node crashes.
  - A T17 violation.
  - A Q3 violation at any moment.
  - Failure to reach Quiet before t_halt+DRAIN_MAX(run), for a seed where DRAIN_MAX(run) did not exceed DRAIN_CAP.
  - (D125) SimNet.drainBound differs from HeadCheckRef.
  - rebuildOverrun > 0.
  - Loss of the last holder of a block in Rec before Quiet, or pinBreach (D126).
  - BudgetCheck failure (D126).
  - (D132) An entry excluded for any of the three reasons (bodyUnavailable, envUnavailable, or depUnavailable) experiences any of the following:
    - Appears in pocol_getRejects.
    - Triggers parentInvalid or ancestorInvalid for its descendants.
    - Sets HfullDone or a blockReject verdict in BlockCache.
    - Causes an honest peer to be penalized.

    This makes control simnet-fault:envAsBlockReject fail under an explicit G rule.
  - An honest node's head or stateRoot changes during PostQuiet (D123).

**T7 (reward proportionality, S, gate)**
- Setup: no adversary.
- Metric: r_i = what settle paid identity i, including winnerPart and propPart, ÷ total paid to all identities in the window. h_i = its rate/46. ρ_i = r_i/h_i.
- Acceptance: TOST with 90% CI places every ρ_i within [0.98, 1.02].
- Failure: the CI of any ρ_i lies entirely outside the range.

**T7b (identity splitting at constant work, S, gate)**
- Setup: adversary with β = 16/62.
  - Baseline arm: one identity at rate 16.
  - Arms k ∈ {2, 4, 8}: k identities at rate 16/k each, registered in genesis with bonds k·B_reg.
  - Churn arm (k=4): each identity performs deregister then reactivate after W_act, with staggered offsets within R_max, reconnecting to new peers after each return.
- Metric: Δ_k = (R_k − R_1)/R_1, where R is total paid to adversarial identities in the window, paired by seed. The adversary's Draw share is recorded as a separate measurement.
- Acceptance: CI upper bound ≤ 0.01 for every arm.
- Failure: lower bound > 0.01.
- Δ < 0 is not a failure.

**T7c (withholding others' shares, G and S, gate)**
- Setup (D120): arms paired by seed per the incentives definitions, with adversary β = 16/62. In ownSharesOnly the adversarial winner puts only its own shares from E_full in shareList, and in a second arm an empty list. The reference arm with the same seed includes all shares.
- (G) For every block won by the adversary:
  - E_full from the NetSim log: valid explicit shares for the height that reached the adversarial node before it published its header, excluding ν, truncated to S_max by smallest nonce. C_full = 1+|E_full|.
  - Reference settlement is calculated by ExecFixture from {ν}∪E_full, and actual settlement from the header.
  - g_dir matches the general formula in D120 exactly in both arms.
  - ownSharesOnly must also have c_w^kept = c_w^full and satisfy the bound.
  - emptyShares has no bound other than the formula.
  - Record (C_full, C_kept, c_w^full, c_w^kept, g_dir) for each block.
  - SV10a–f is a pre-run gate, deterministically covering both threshold branches, the empty list, and the S_max limit.
  - Therefore (G) does not require C_full > κm to occur in SimNet, which may be rare at m=32. Record the count of these blocks as a measurement, without an underpowered label.
- (S) For each adversarial arm:
  - g_seed = (R_arm − R_ref)/R_ref, where R is total paid to adversarial identities in the window, including winnerPart and propPart.
  - R_ref = 0, or a stall in either arm, yields the censored value from CEN.
  - Acceptance for each arm: CI upper bound ≤ 0.02.
  - T7c is not event-conditioned, so the excitation quota does not apply. A seed where the adversary never wins is valid, since R includes propPart and indirect effects.

**T7e (winner equivocation, G and S, gate)**
- Setup: at the adversary's first win at height j ∈ [1000, 1500], running through quiescence at 3600, so j+E_win ≤ 3548 < H_CLOSE.
  - No win in the range means an unexcited seed.
  - No honest signed block whose proposer saw both headers within [j+1, j+E_win] makes the seed unexcited for items (b) and (c) only. Items (a) and (d) still apply.
  - Sign two headers with the same nonce and shareRoot1 ≠ shareRoot2.
  - Send the first to half the honest nodes and the second to the other half, with a 20s partition, then heal.
- Criteria:
  - (a) (G) HeadCheck after the barrier (Q1–Q5). (S) Convergence within 30·T_blk after healing, using the convergence rule on excited seeds.
  - (b) If an honest signed block exists in [j+1, j+D−1] after its proposer saw both headers: type 2 evidence is included in it or earlier, settle(j) gives invalidBurn = pot_j, and the accused has tomb=1 and bond BURNED.
  - (c) If that block does not exist: including evidence within E_win gives tomb=1 and zero effect on settled pot_j. Record both cases.
  - (d) Evidence violating its conditions invalidates the carrying block (negative control in the same script).

**T9 (progress with silent proposers, S, gate)**
- Setup: four honest identities with weights (8, 4, 2, 1, totaling 15/46) are silent proposers: they send no templates and mine on fallback.
- Metric: for each h in the window at N0, I_h = 1 if block h arrives within X^W+5·T_blk of its parent's arrival. T9 = ΣI_h/3000.
- Acceptance: CI lower bound ≥ 0.95.

**T10 (measurement only)**
- Setup: β ∈ {0.098, 0.207, 0.303} with two policies:
  - Selfish mining: withhold blocks and reveal them when honest nodes advance.
  - Double spending: pay, then build a private branch excluding the payment, revealed if its work exceeds the other branch.
- Metrics:
  - Adversarial share of canonical blocks versus β.
  - Reorganization depth distribution.
  - Double-spend success rate after k ∈ {1, 6, 60} confirmations per 1000 attempts.
- No security threshold is inferred from this [PDF:74].

**T10b (gate: i, ii, iii, vi. Measurement: iv, v)**
- (i) No adversary: sample every virtual second, passing if all honest prefixes through head_min−6 match. Acceptance: CI lower bound for the fraction of successful samples ≥ 0.99.
- (ii) Partition 23/23 by weight for 600s at block 1000, then heal. G for every evaluated seed after the barrier:
  - HeadCheck (Q1–Q5).
  - This includes Q4: each honest node's state is byte-identical to a node that saw only its head's branch, covering the winning branch without assuming tie resolution.
  - (S) Convergence within 30·T_blk of healing, using the convergence rule.
  - Losing-branch transactions return to the mempool after revalidation.
  - Depth is a measurement.
- (iii) Two templates with body withholding:
  - Excitation: the first h ∈ [1000, 2500] where Draw(h,0) is the adversarial identity and AdvOracle finds a nonce.
    - AdvOracle searches outside virtual time in the adversary's range R(adv) for UT1, up to 2^28 hashes.
    - If unsuccessful, try the next eligible h; if the range is exhausted, the seed is unexcited.
    - AdvOracle gives the adversary unrealistic search power, so this arm tests rule handling, not adversarial success probability.
  - t0 = L(0) for height h. partition(A, B, t0−1s, t0+X^W+120s), with the adversary connected to both halves.
  - At t0: send UT2 with its body to B only, and no template to A.
  - At t0+X^W: publish the UT1 header with the nonce to A only, and refuse GetBodies until t0+X^W+120s.
  - Then publish the body to everyone and heal the partition.
  - Item (d) is conditional as in T7e(c).
  - Criteria (G):
    - (a) No honest node counts work or changes its head for a block whose body it has not verified (D38).
    - (b) (S, not G): fraction of excited seeds where each half produced at least one valid block during [t0, t0+X^W+120s], via fallback or UT2. Acceptance requires CI lower bound ≥ 0.9. Record a seed where one half produced no block; it is not a G violation.
    - (c) (G) HeadCheck after the barrier (Q1–Q5). (S) Convergence within 30·T_blk after body publication and healing, using the convergence rule on excited seeds.
    - (d) Type 1 evidence is included in the first honest signed block whose proposer saw both templates within E_win, and the accused has tomb=1.
    - (e) unknownParent and side-branch queues remain within their bounds.
- (iv) Proposer–winner collusion for premining: measure the head start time.
- (v) Collusive extreme timestamps: measurement with T10c.
- (vi) Isolation and reconnection: the adversary isolates N5 for 300s, feeds it a private branch at rate 5 containing a header with an invalid identity, then reconnects it. G after the barrier:
  - HeadCheck includes N5: its head has work W*, is not on the private branch, and its state matches RefReplay for its head's branch (Q4).
  - (S) It adopts the heavier branch within 60s virtual time after reconnection, using the convergence rule.
  - Rollback ≤ R_keep (Q5).
  - The peer sending the invalid header is banned.

**T19 (inclusion under partial censorship, S, gate)**
- Setup: adversarial proposers with weights (8, 4, 2, 1) exclude tagged target transactions. A generator sends a target transaction every 5 blocks to honest nodes.
- Metric (per height, D47): eligible heights are those with a target transaction that reached the honest mempool more than X^W before its parent's L(0). I_h = 1 if all are included by h+10. T19 = ΣI_h ÷ number of eligible heights.
- Acceptance: CI lower bound ≥ 0.8.

**T20 (FSM1–FSM10, G, fake clock)**
- FSM1: a valid template before T_j locks.
- FSM2: arrival exactly at T_j locks; 1 ms later gives FB.
- FSM3: absence gives sticky FB with no return.
- FSM4: an invalid template is treated as absence, with a peer penalty.
- FSM5: a second template from the proposer gives type 1 evidence, then FB.
- FSM6: a template preceding its parent is stored, then applied when the parent arrives, limited to 4 per parent and 64 total.
- FSM7: parentInvalid deletes it.
- FSM8: when a ≥ 1, FB occurs at L(a).
- FSM9: transition to a+1 when C > U(a)+Γ.
- FSM10: empty Act means a halt.
- Criterion: state and events match the VA-spec table.

**T23 (N1–N10, G, NetSim)**
- N1: full Δ delay on every link.
- N2: 10% gossip message loss.
- N3: three item 1b violations from a peer cause its ban.
- N4: a peer sends 100 headers with unknown parents: queue ≤ 64, and no ASERT.
- N5: a body not matching the roots in GetBodies: reject the response, not the block (BodyMismatch, D125), penalize the peer, and fetch the body from another peer.
- N6: a peer always refuses GetBodies.
  - (G) Fetching from another peer completes before the script ends.
  - (S) Completes within 3Δ of refusal, with CI lower bound ≥ 0.95.
- N7: synchronize 3000 blocks from two peers, one sending invalid headers: ban it and complete.
- N8: restart during synchronization: resume without corruption.
- N9 (D129, late availability on the live network):
  - t=10: N3 knows header X, and no peer holds its body. All N3 peers reply NotFound, so all enter triedPeers.
  - t=20: FixtureMiner delivers body X to N5, which imports it and broadcasts its header.
  - (G) N3 imports X before the script ends (t=200), and the first request after broadcast targets N5.
  - (S) Import within 2s of broadcast.
  - Variant: drop(N5→N3, block) without Status. N3 requests N5 again via exponential backoff within ≤ 60s. (G) Import before t=200.
  - Negative control simnet-fault:noReeligible keeps N5 excluded, failing both cases.
- N10 (D131, late valid envelope on the live network):
  - t=10: AdvPeer sends N3 body X and three header copies with winnerSig from nonowners. N3 has three envReject, and X is in envWait without a verdict on blockHash.
  - t=40: FixtureMiner delivers X with its correct copy to N5, which imports and broadcasts it.
  - (G) Zero blockReject and zero X entries in pocol_getRejects at N3, and N3 imports X before t=200.
  - (S) Import within 2s of broadcast.
  - Negative control simnet-fault:envAsBlockReject makes N3 reject X permanently, failing.
- N11 (D136, envelope flooding on the live network, I75):
  - Setup: sv-fix in SimNet. N3 knows header X with its body and holds no envOk copy.
  - AdvPool: 300 peer identities connect successively, ≤ 20 concurrently. Each sends N3 two different X header copies with winnerSig from nonowners via gossip, then disconnects before being banned. Total 600 hdrId.
  - Meanwhile every adversarial peer responds to GetEnvelope with a new corrupt copy.
  - t=60: FixtureMiner delivers X with its correct copy to N5 alone.
  - (G) At every sample: rej(X) ≤ 8, pending(X) ≤ 4, envRejEntries ≤ 16384, and envelope memory ≤ ENV_MEM_MAX.
  - (G) Checks of unsolicited copies for X ≤ 8 + number of evicted verdicts for X. Zero blockReject for X, and zero entries for it in pocol_getRejects.
  - (G) N3 imports X before t=200, and the request that obtained the correct copy has a solicited response.
  - (S) Import within 30s of t=60; time is a measurement.
  - envFloodDropped and envEvicted are measurements.
- N11b (G, EnvCache unit with a fake clock):
  - 20000 verdicts for 5000 blockHash values with local entries.
  - Expected: total ≤ 16384 at every step, eviction strictly in FIFO order, and rej per block ≤ 8.
  - Removing a blockHash deletes all its verdicts. A verdict for a blockHash without an entry is not retained.
- N11c: as N11, with the correct copy arriving via gossip while full.
  - Expected: dropped without checking and without penalizing N5, then fetched by EnvFetch within the N11 bound.
- Negative controls:
  - simnet-fault:envCacheUnbounded disables caps. Fails N11b (20000 > 16384), and fails N11 (rej(X) > 8).
  - simnet-fault:envFullAsBlockReject records blockReject when the cap is reached, failing N11.
  - simnet-fault:envDropSolicited drops solicited responses when full. X is not imported before t=200, failing N11.
  - simnet-fault:envSolicitUnbounded (D137) issues requests and checks every solicited response without slots. In-flight checks exceed 64 in N12, failing.
  - simnet-fault:envReplyUnreserved issues requests without reservation and queues responses. In-flight requests exceed 32 or envMemBytes > ENV_MEM_MAX in N12, failing.
  - simnet-fault:envTimeoutExclude permanently excludes the peer after Timeout. X100 is not imported in N12t, failing.
  - simnet-fault:envReleaseOnCancel (D138) releases the slot when cancellation is requested before the check settles. In N12c it grants 32 new slots at t=1, making liveChecks = 96 > 64, failing.
  - simnet-fault:envTimeoutCoversCheck (D138) keeps the request timeout active during checking and releases the slot when it expires.
  - simnet-fault:envCheckSharedPool (D139) runs envelope checks in the shared ComputePool, failing N12w.
    - In N12c the timeout expires at 2.5s while checking continues, failing the liveChecks predicate.
    - In N12r(r1) it releases the slot after entering checking, failing.
    - In QV25c, with c_env set in SimParams so RTT_i+c_env > 2s, a timeout occurs during draining, failing the envTimeouts = 0 predicate.
- N12 (D137, checks saturated by solicited responses for multiple blocks):
  - Setup: sv-fix in SimNet, and ENV_REQ_TIMEOUT = 2s.
    - N3 has one hundred entries X1..X100 with their bodies and parent states, and no envOk copy for any.
    - N3's peers: twenty adversarial peers, and N5.
  - Adversaries reply to every GetEnvelope after 1.5s with a new corrupt copy. They send corrupt copies for each Xk via gossip at the maximum rate.
  - t=30: FixtureMiner delivers X100 with its correct copy to N5 alone, with drop(N5→N3, block).
  - (G) At every transition:
    - solicitHeld ≤ 32, gossipHeld ≤ 32, in-flight checks ≤ 64, and envMemBytes ≤ ENV_MEM_MAX.
    - Zero solicited requests or checks without a slot.
    - One release per slot.
    - Grant order matches EnvQueue FIFO in ActLog.
  - (G) Zero blockReject, and zero entries in pocol_getRejects for any Xk.
  - (G) N3 imports X100 before t=600 with a copy from a solicited response from N5.
  - (S) Import time is a measurement, compared with the recovery lemma's bound.
  - N12t: N5→N3 link delay 3s until t=40, then 50 ms.
    - The first request to N5 after t=30 expires with Timeout, then becomes reeligible via exponential backoff.
    - (G) Import before t=600, and the late response follows the gossip path without a penalty.
- N12b (G, EnvSlots unit with a fake clock): for each of the eight outcomes in network:
  - The exact state transition, one release, and x returning to the tail or awaiting reeligibility.
  - A response larger than ENV_REPLY_MAX is cut off at 3131 B and penalized.
  - A response after release does not reserve a solicited slot.
- N12c (G, EnvSlots unit, D138, cancellation during checking):
  - Setup:
    - Fake clock.
    - CheckGate is a hook in simnet builds only, holding completion of every check until settle(i).
    - 64 blocks X1..X64 in envWait, with parent states available.
  - t=0:
    - 32 solicited requests for X1..X32, whose responses complete at 0.5s and enter checking.
    - 32 gossip copies for X33..X64 enter checking.
    - liveChecks = 64, and liveBuffers = 64.
  - t=1, termination reasons during checking:
    - X1..X8 peers disconnect.
    - X9..X16 imported via another path.
    - envOk for X17..X24 via another path.
    - X25..X32 removed by D118.
    - X33..X64 imported.
  - From t=1 until t=3, advancing the clock past each request's timeout:
    - solicitHeld = 32, and gossipHeld = 32.
    - Zero new grants, and zero release.
    - liveChecks = 64, and liveBuffers = 64.
  - t=4: settle all checks, half envOk and half envReject. Expected:
    - Exactly 64 release.
    - Zero imports of an imported or removed block.
    - lateCheckDropped = number of discarded results under the network rule.
    - New grants begin only after settlement.
  - Predicate at every transition:
    - liveChecks ≤ 64, solicitChecks ≤ 32, and liveBuffers·3131 ≤ 200384 B.
    - No event with an old attempt ID changes a slot.
- N12p (G, EnvFetch unit with EnvSlots and EnvCheckPool, fake clock and FakeExec, D139, minimal case):
  - Constants: ENV_REQ_TIMEOUT = 2000 ms, W = 4, and c_exec = 50 ms per check. Thus c_env = 850 ms, and T_role = 2850 ms.
  - t=0: Y1..Y32 granted to peer A. x enters EnvQueue alone (Q=1), and its only peer h* = N5 is untried (P=1, B=0).
  - A completes the last byte of every response at 1999, so 32 checks enter EnvCheckPool and settle in batches of 4 at 2049, 2099, …, 2399.
  - 2049: the first slot is released and granted to x in the same step. N5 responds with the last byte at 3549 (RTT 1500), the check settles at 3599, then import.
  - Expected: measured D_x = 3599 ≤ calculated D_x = 5700, and every span ≤ 850.
  - (p-c) Churn: a new peer C with a smaller ID connects at 2000, so P = 2.
    - x is granted to C at 2049, which replies NotFound at 3549.
    - Granted to N5 in the same step, which replies at 5049, with settlement at 5099.
    - Expected: 5099 ≤ 3·2·2850 = 17100.
  - Negative control ref-fault:recoveryOldBound in RecoveryRef calculates (P−1)·⌈Q/32⌉·T_role = 0 < 3599, failing.
- N12h (G, unit, reaching the holder after P−1 failures, D139):
  - Setup: P = 5, with A1..A4 having IDs smaller than N5 and not holding x. Q=1, and no other load.
  - (h-a) Each Ai replies NotFound after 1900 ms. Requests at 0, 1900, 3800, 5700, then N5 at 7600, replying at 9100, with settlement at 9150.
  - (h-b) Ai do not respond, so each times out at 2000. N5 is requested at 8000, with settlement at 9550.
  - (h-c) N5 was previously tried with NotFound at −1000 (k=1), becomes reeligible at 1000, and B = 1000. A1..A4 are untried, so requested first at (h-a) times, then N5.
  - Expected: request count = 5 ≤ 2P−1 = 9, every time ≤ 9·2·2850+B, and order matches RecoveryRef exactly.
- N12a (G, EnvFetch unit with RecoveryRef, D140, late parent):
  - Constants as N12p: T_role = 2850 ms, W = 4, c_exec = 50 ms, and RTT(N5, N3) = 1500 ms.
  - t=0: x is in envWait at N3 with its header and body, N5 is the sole holder (P=1, Q=1, B=0), and x's parent state is unavailable.
  - t=20000: the parent state becomes available through late import of the parent body.
  - Expected:
    - Zero GetEnvelope for x before 20000.
    - First request at 20000, response at 21500, settlement at 21550, then import.
    - RecoveryRef: t_0 = 20000, preCondWait = 20000, delay after t_0 = 1550 ≤ D_x = 5700, and verdict withinBound.
  - Negative controls:
    - simnet-fault:envRequestBeforeParent sends the request before the parent state, failing the "zero requests before 20000" predicate.
    - ref-fault:t0Unconditioned calculates t_0 = 0, declaring boundViolated (21550 > 5700) for a conforming execution, failing reference agreement with the expected verdict.
- N12s (G, SimNet, D140, persistently slow holder):
  - Setup: N3 has x in envWait with its parent state, N5 is the sole holder with RTT(N5, N3) = 3000 ms permanently, ENV_REQ_TIMEOUT = 2000 ms, drop(N5→N3, block), and the script runs until t=200000.
  - Expected:
    - Every request to N5 expires at +2000 with substream reset, zero late bytes read into a buffer, and envTimeouts ≥ 1.
    - Zero x imports, zero blockReject, zero x entries in pocol_getRejects, and x remains in envWait.
    - RecoveryRef: G3 is not satisfied, so the verdict is noGuarantee; neither timing success nor timing failure is recorded.
  - Variant N12s-i: from t=60000, RTT(N5, N3) = 500 ms.
    - t_0' = 60000, and B = remaining N5 exponential backoff at 60000 from the trace (≤ 60000).
    - Expected: import at ≤ 60000 + 2·2850 + B, and verdict withinBound.
  - ref-fault:t0Unconditioned declares boundViolated in N12s, failing.
- N12w (G, EnvCheckPool unit, D139):
  - t=0: 64 live checks (32 solicited, 32 gossip) with c_exec = 50 ms.
  - Expected: settlement in 16 batches through 800, maximum span = 800 ≤ c_env = 850, liveChecks ≤ 64, and zero checks outside EnvCheckPool.
  - (w-o) One check with c_exec = 900:
    - envCheckOverrun = 1, no release before it settles, and the cap is preserved.
    - No claim that this check passed the time bound.
  - Negative control simnet-fault:envCheckSharedPool runs checks in the shared ComputePool. The test fills the pool with twenty tasks lasting 5s each, making span ≥ 5050 > 850, failing.
- N12r (G, unit, response–timeout race, D138):
  - (r1) Last byte exactly at sendTime+2000 ms → checking, and no timeout.
  - (r2) Last byte at +2001 ms:
    - Timeout at +2000 resets the substream and releases the slot.
    - Late bytes are not read into any buffer.
    - The slot is granted to a new attempt with a larger attemptId.
  - (r3) The hook injects a check result with a previous attempt ID after regrant → discarded, with no release for the new attempt.
  - (r4) Cancellation in buffered after 1500 bytes → immediate release, and liveBuffers decreases in the same step.
- Criterion:
  - HeadCheck after the barrier, with the barrier applied at the end of each script.
  - Zero crashes.
  - P2P limits preserved.

**T18b (L1–L13, G, caps/64)**

Profile caps/64 (simnet):
- Values: sv-fix with chainId=777903, H_END=4000, H_CLOSE=3840, and Z_close=160.
- Caps: CODE_CAP=2^21, TXBYTES_CAP=2^24, TX_CAP=20000, SLOT_CAP=50000, and ACCT_CAP=20000.
- Membership and evidence: R_max=2, M_max=64, E_win=64, U=96, and D=16. Thus Lmax=145, and REG_RECORDS_MAX = 64+2·(145+2048) = 4450, exactly the R16 bound.
- SYS_SLOT_MAX = smallest value passing R18 through ParamGate.derive, fixed in VA-spec.
- caps-test build constants (simnet only, absent from release): LOG_CAP=2^24, HIST_TRIE_BUDGET=64 MiB (R21 exception).
- Any ParamGate violation gives FixtureSetupError.

Loads: each Lk is an independent run from genesis with seed b1.
- L1: store distinct 24575-byte chunks, one per block.
- L2: transactions with 131000-byte calldata to Rv, filling BODY_MAX.
- L3: transfers with 21000 gas.
- L4: a SlotFill contract writes 300 new slots per transaction.
- L5: transfer 1 wei to new addresses.
- L6: LogBomb.
- L7 and L8 (bound gates, not reachability gates, since R16 makes reaching the limits impossible by construction): maximum churn for 2000 blocks, meaning R_max registrations or reactivations per block with deregister and claimBond using funded test keys. Criterion:
  - RecLen ≤ 4450 and SYS usage ≤ SYS_SLOT_MAX in every block.
  - RegOpsLimit above R_max.
  - Reaching either is a failure of the lemma.
- L9: 600 transactions of 131072 bytes to the mempool.
- L10: a peer sends 100 headers with unknown parents. Queue ≤ 64; arrivals when full are dropped without storage, and without ASERT.
- L11: three blocks at the same height with different branch work, arriving in order canonical, lower, then higher. Retain canonical and higher under D118, and reorganize if higher becomes heavier than canonical.
- L12: intensive state changes until HIST_TRIE_BUDGET is exceeded. K_eff decreases, and anything outside it gives −32017.
- L13: K_eff < k_view triggers procedure 7.

L9 response: −32016 after 64 MiB, and RSS within T14.
- Criterion:
  - Exactly the response declared in execution and storage: deposit failure, exceptional halt, or block invalidity and transaction skipping, or freezing.
  - At least 20 subsequent blocks pass T17.
  - Zero crashes, and RSS within T14.

**T25 (replica loss during update and recovery, G)**
- Setup (ReplicaHarness):
  - Machines: M1 runs R1 and R2, and M2 runs R3. On one machine, declare the result "not representative of availability".
  - Profile: sv-fix with chainId=777921. Identities: w on R1, x on R2, y and z on R3. Every pocold replica retains everything.
  - Site T25Site:
    - v1 = {index.html 4096 B, style.css 8192 B, app.js 614400 B}.
    - v2 = {index.html 4100 B, style.css with v1 bytes (same chunk address), app.js 1032192 B}.
    - n = 44 new chunks for v2.
  - Snapshot S0: complete copy of R3's RetentionStore directory (state, all blocks, bodies, and receipts through H_s), where H_s is the height of setCurrent(v1). Signed with test key 1, with keccak256 for every file.
  - wipe: kill, then delete the entire data directory, retaining the identity key and cfg.
  - Counting: ReplicaMonitor (D119) on R1 with list {R1, R2, R3}.
  - Source of truth: publisher's local files and keccak256 for each file, and R3's stateRoot at the same height.
- Script:
  - (a) After n/2 chunks are included: kill and wipe R1.
  - (b) Complete publication, then setCurrent(v2).
  - (c) Restore R1 from S0 after verifying its signature, then synchronize from R2 and R3. When R1's head reaches H_s+⌊(head_R3−H_s)/2⌋: kill and wipe R2.
  - (d) R1 completes synchronization from R3 alone.
  - (e) The extension in LN on R1 fetches v2 and v1@v1.
  - (f) Separate arm: kill and wipe all three replicas without recovery.
  - (f2) Separate arm: restore R1 alone from S0 without peers.
- Criteria:
  - R1 stateRoot = R3 stateRoot.
  - v1 and v2 bytes match publisher hashes.
  - "Retention requirement unmet" appears within ≤ 30s of replicasLive falling below 2, and disappears within ≤ 30s of its return to ≥ 2.
  - In (f): explicit "Local node unavailable" and "Content unavailable", without partial display or mismatching bytes.
  - In (f2): head = H_s, v1 displays identically, and v2 gives "Version does not exist at this height". The warning remains (replicasLive = 1), and loss after H_s is declared.
- Failure: any mismatch, silent display, or partial display.
- Recovery time is a measurement. (f) proves correct failure, not availability.

**Mapping conditional claims**
- No profit from identity splitting ← T7b. Failure means D7 and κm are insufficient, requiring incentive changes before any public network.
- Bound on exclusion gain ← T7c.
  - (G) failure means an implementation error, since the formula and bound are arithmetic (D116, D120).
  - (S) failure means the indirect effect through Draw exceeds 2%, requiring incentive changes before any public network, such as uniform Draw or γ_bp=0.
- Equivocation deterrence ← T7e. Failure invalidates D29 as implemented.
- Progress with absent proposers ← T9 with C1 and C2. Failure means D22 and D40 do not achieve the declared progress.
- Convergence after partition, isolation, and body withholding ← T10b(ii, iii, vi). Failure blocks M5.
- Inclusion under partial censorship ← T19.
- Freezing the component, not the chain ← T18b.
- Recovery and truthful reporting of insufficient replicas ← T25.
- T10 and T10b(iv, v) are measurements and do not prove security.
- Passing any VA test does not prove general security, and does not transfer to a real network without T3 and T14.

**Separation of evidence**
- Phase A is not used for any PoCol claim, and ExecFixture does not prove block validity.
- ChainFixture, NX, RW, HG, HC, CL-END, LC, and RG do not prove general behavior under an adversary.
- RG proves resource bounds for one node on a specified machine.
- PS proves the control's logic and the distinction between sendDone and receiveDone on the defined virtual model.
- Trace predicates prove rule application to the execution's own values, not the accuracy of A against the kernel.
- DiscriminationCheck proves the gate's ability to detect the injected fault only when the measured path exhibits a violation.
- LC proves client logic against a fake server, and RG3 and RG3b against pocold in specific scenarios.
- BR proves rejection of the enumerated and tested random cases and success of the enumerated legitimate paths, not freedom of the JSON parser from all vulnerabilities. BR17e proves quota accounting and atomicity only on the specified schedule. BR19a and BR19b prove bucket and pending-request logic on a fake clock and transport, and BR19c proves consistency of the Chrome implementation with the reference using its own recorded arrival times, not the accuracy of those times.
- SimNet does not transfer to a real network without T3 and T14.
- No success here is proof of security.

## Development plan

**Environment**
Windows 10 with 8 GB, without Docker, WSL or administrator privileges.
- Dedicated ports/directories, without stopping the existing work environment.
- Test Chrome with a separate user-data-dir.
- Two anvil instances (approximately 200 MiB each).
- RpcTap local Node proxy ≤ 100 MiB.
- Local CanarySink. Firewall permission may be requested; the user decides, with a second machine as the alternative.
- Single-threaded FixtureMiner ≤ 200 MiB.
- RpcAttacker ≤ 300 MiB; measure the node's RSS separately.
- fetchEach in Node ≤ 300 MiB.
- PacerSim in Rust, without networking.
- BR19a and BR19b in Node, without browser or networking.
- Optional portable Python for the third library, reference, SchemaRef, BridgeRef and SiteStorageRef; if unavailable, use another machine and disclose it.
- BR18e and BR18f need no actual external connection: TestHooks intercepts chrome.tabs.create and records the URL without loading it.
- BR17e writes approximately 1 MiB to chrome.storage.local only in the test user profile.

**License**
Apache-2.0, with MIT, Apache or BSD dependencies. Foundry and solc are build tools only.

**Stages**
- M0: versions, licenses (including hyper, socket2 and dependency-cruiser), anvil, canary, three keccak libraries with K1–K3, fixed-width library, audit rpc-fault and its three faults, bridge-fault and its thirty-two faults listed in governance (M0 output), and verify chrome.storage.local.getKeys availability in test Chrome.

Phase A (anvil; establishes nothing about PoCol):
- M1: M1-spec, then ChunkFactory, Website and publisher tool. Tests: T1, T16.
- M2: MV3 extension including:
  - DnrGuard, ProfileMutex, RtcLockdown, wallet.
  - BridgeAuth, BridgeBucket, RpcReadClient (with pendingReads), WalletSubmit, SiteStorage (with quota accounting), Navigator, BridgeTrace.
  - CanarySink, HeaderNetCheck, LogClient (with SinkChecker and counters).
  - RpcTap (with hold), BadSite, GoodSite, BridgeRef with SiteStorageRef.
  - Tests: BG1, BR19a, BR19b, BR20a, BR20b and MR-unit first; then SiteStorage unit tests on BR17e with a fake dictionary; then T11a, T12, X8, BR1–BR14, BR16–BR18, BR19c, BR20c, BR20d, MR1–MR12, BR21a–BR21f, BR22a–BR22h, BR23 and BR24 (with EpochFence, RecoveryGate, ReadSlots, RecoverySlots, SeqAlloc, DiskGate, DiskLedger, AdminDelete, AdminSlots and SR-model), BR-neg, BR-dep, RW-unit, HC-unit, LC-unit (LC1–LC18).

Phase B (pocold; actual PoCol integration):
- M3: GenesisSpec, ParamGate (R13, R16 with Lmax/LAG, R20, R26 with derived values/TIMER_EPS), NetConfig, Codec, NetId, Timing, Difficulty.asert, Draw, partitioning, TS model. Tests: K1–K3, L4, T22, T0, AS1–AS19, AS-eq, ASQ, ASB, TJ, PG-END1–5, PG-L, PG-RPC, CF1–CF6, SV1–SV7s.
- M4: revm with PAY, GenesisBuilder (up to 1024), phases, CapsTracker, TipsAccumulator, system contracts, Build_fb, ExecFixture. Tests: T2, T2b, CI1–CI3, GB1, GB-END, L4s, T2c, T24, T26.
  - (D141, D142) BlockMeta.parentResult(hash) → {blockHash, winner, shares[(n, owner)]}.
    - Derived by ParentResult.derive(HeaderStore.header(hash), Assigner.tableFor(hash)) after items 13 and 14 pass, stored by hash.
    - Reexec.verifyParentResult(hash) rederives during sync, TrieRebuild and reorg, comparing stored data; mismatch is parentResultMismatch.
    - Only recordShares(h−1) calls ShareRecorder.record(ctx.parentResult).
  - Executor.apply takes ctx.parentResult and cannot accept the current block's nonce or owner; its interface type enforces this.
  - M4 automated checks add two conditions: closeBlock does not write k=2 for pending[h]; recordResult is called only from SYSTEM_ADDRESS.
  - Test-only exec-fault:winnerInState build, absent from release.
- M4b: FixtureMiner, FixtureVerify. Tests: FX0, SV8, SV8-fb, SV8-cap, FBX, SV9, SV9-final, NX (a), TW1–TW5 with winnerInState, sharesFromFirstSeen, sharesFromMiner and sharesDropped controls (D141, D142). FixtureMiner accepts plan.shareOwners to search specified ranges for shares on a prebuilt UT.
  - Threshold (D143) computed by FixtureMiner.shareThreshold(UT) = min(2^256−1, UT.target·CP.m), calling the consensus function itself, not a plan constant.
  - plan.gapShare requests a nonce whose hash lies in (T_share, supplied upper bound] for TW5-gap.
  - Test-only fixture-fault:fixedShareThreshold build, absent from release. FixtureMiner accepts plan.searchOwners = [w, x] on one prebuilt UT.
- M5 (code must not start before VA-spec review, D115): P2P, TemplateFSM, sync, SimNet with NetSim, AdvNode and TraceDB.
  - ChainTimer (D123): schedule(tag ∈ {chain, nonChain}, at, job), pending(tag). The sole scheduler in consensus, synchronization and import modules.
  - A clippy disallowed_methods rule prohibits direct tokio::time::sleep, interval and timeout use in those modules; violations fail CI.
  - SimNet.chainWork(node), SimNet.purgeFuture(entry), SimNet.statusSweep(), simnet build only.
  - (D124) SimNet.bodyHolders(hash), SimNet.purgeUnavailable(tAvail), SimNet.drainBound(run) → {N_miss, c_b, DRAIN_MAX}, simnet build only.
  - (D125) In simnet builds only:
    - SimNet.recSet(tAvail) → {rec, excluded[{hash, reason, missingAncestor?}]}.
    - SimNet.need(i) → {blocks, branches[{forkDepth}]}.
    - (D126) DrainPin.haltPin() within the atomic halt step; DrainPin.add(item) for every subsequent honest arrival; DrainPin.narrow(rec) at t_avail in the same U step; DrainPin.stats → {pinRefusals, pinBytes, pinEntries}.
    - (D126) ActLog.record(node, class, durMs) through ChainTimer; BudgetCheck.run(actLog, bounds) → ok | {class, node, actual, bound}; HeadCheckRef.heldAtHalt(log).
    - evictProbe(node, hash) test hook.
    - drainBound returns {need, K, Kmiss, Kpend, S, T, dStar, HR, RB, waits{restart, eligibility, preHalt}, DRAIN_MAX}, matching HeadCheckRef (D126, D127).
    - (D127) SimParams.load(vector) is one record read by NetSim, drainBound and HeadCheckRef; BudgetCheck.table(run) returns each class's row with count, duration and component.
  - (D127, D128) In pocold:
    - HdrCache (D129), bounded by sync queues, containing:
      - BlockCache keyed by blockHash for completed committed-content verdicts.
      - EnvCache keyed by hdrId for envelope verdicts.
      - inFlightHdr with both keys.
      - HeaderStore.accept(blockHash, hdrId) retains the first envelope-valid copy.
      - EnvClass.classify(rule, part) → env | block.
      - EnvCheck.pre(hdr) → envOk | envReject, stateless. EnvCheck.own(hdr, parentState) → envOk | envReject, without apply, caching assignment per parent (D130).
      - EnvCache states {envPending, envOk, envReject}; the verdict after own is final while retained (D136). Interfaces:
        - admit(hdr, solicited, peer) → check | dropFlood | dropKnown.
        - record(x, hdrId, rule, peer) evicts x's oldest verdict or the globally oldest verdict.
        - purge(x) on import, envOk, blockReject or removal.
        - stats → {envRejEntries, envFloodDropped, envEvicted, envMemBytes}.
        - pin() in simnet builds as part of HaltPin.
    - FetchSched interfaces:
      - DiscRun.onStatus(peer, head) → start | markDirty.
      - DiscRun.next() → {peer, locator} | done, marking tried.
      - BodySched.request(hash) → peer | waiting, holding inFlight and triedPeers.
      - BodySched.onReply(hash, peer, NotFound | Mismatch | Ok).
      - BodySched.onStatus(peer) and BodySched.onAnnounce(peer, hash) put the peer in reeligible without removing it from tried (D130).
      - BodySched.cycleReset(hash | all) clears tried, reeligible and timers; called by SimNet at t_avail.
      - BodySched.pick(hash) → untried in ID order | reeligible | waiting.
      - Backoff with BODY_RETRY_BASE = 1s and BODY_RETRY_MAX = 60s; untried preferred (D129).
      - HfullDone.has(hash).
      - (D131) EnvFetch:
        - request(hash) → peer | waiting.
        - onReply(hash, peer, Copy(hdr) | NotFound | Timeout | Oversize | Disconnect); every outcome calls EnvSlots.release once (D137).
        - EnvQueue (D137): enqueue(hash) once per x, head(), requeueTail(hash), remove(hash).
        - EnvSlots (D137):
          - grantSolicit(hash) → Slot{reqId, peer, buf 3131 B} | none, in the sending step itself.
          - acquireGossip(hdr) → Slot | none.
          - cancel(slot, attemptId, reason) (D138): marks cancelRequested. In reserved/buffered, resets the substream and releases immediately; does not release in checking.
          - onCheckSettled(slot, attemptId, verdict): sole event releasing a checking seat, applying network's cancelled-check-result rule.
          - release(slot, attemptId, reason): internal, with a duplicate-prevention flag. No-op unless attemptId matches the current attempt.
          - Timeout timer cancelled on entering checking. Timer and response events use one loop; ties favor the response.
          - stats adds {liveChecks, liveBuffers, cancelPending, lateCheckDropped, envCheckOverrun}.
          - CheckGate.hold/settle is a simnet-only hook for N12c and N12r.
          - EnvCheckPool (D139): submit(slot, attemptId, hdr) with FIFO queue and W dedicated threads; stats → {queueLen, spanMaxMs, envCheckOverrun}. FakeExec in simnet sets c_exec per check (N12p, N12h, N12w).
          - RecoveryRef (Python): conditions(trace) → {t0 | none, preCondWait, unmet ⊆ {G1, G2, G3, G4}, verdict ∈ {withinBound, boundViolated, noGuarantee}} (D140), with ref-fault:t0Unconditioned. bound(P, Q, B, T_role) → D_x; sequence(peers, replies) → expected request order, with ref-fault:recoveryOldBound.
          - onLateReply(reqId) → unsolicited admit path.
          - stats → {solicitHeld, gossipHeld, releaseCount, doubleReleaseCount, timeouts}. Any nonzero doubleReleaseCount fails.
        - onStatus, onAnnounce and cycleReset as in BodySched.
      - GetEnvelope.serve(hash, exclude) → HeaderStore.validCopy(hash) | NotFound.
      - EnvWait.has(hash), ENV_PENDING_MAX = 4; D136 constants (ENV_REJ_PER_BLOCK, ENV_REJ_GLOBAL, ENV_PENDING_GLOBAL, ENV_INFLIGHT_GLOBAL); D137 constants (ENV_SOLICIT_SLOTS, ENV_GOSSIP_SLOTS, ENV_REQ_TIMEOUT, ENV_REPLY_MAX). T23 N11–N11c, N12, N12t and N12b tests in M5.
      - SimParams.rtt(i, j), rttMax(i) (D131); ActLog records envFetch as a hdrFetch subclass with each request's duration.
    - ActLog records (hash, peer) per request and the run ID per run.
  - In pocold: ChainWork keyed by block hash; RejectPath.cancel(hash) cancels the entry, descendants and timers in the rejection step, called only for H-pre/H-full blockReject on a commitment-matching body. envReject uses EnvRejectPath.drop(hdrId, peer) without cancellation (D130). BodyFetch.onMismatch(peer, hash) penalizes the peer and retries without cancellation (D125). NotFound immediately moves fetching to another peer. Tests: T3, T4, T5, T6, T9, T20, T21, T23, SV8-reorg, SV8-tie, SV9-reorg, NX (b, c).
- M6 (requires VA-spec, D115): shares, settlement, evidence, closing with End.bound, BurnMonitor, EndFixture, adversarial policies in T7b, T7c and T7e. Tests: T7, T7b, T7c, T8, T17, T27, CL-END, CL-OPS1–2.
- M7: hyper RPC server:
  - First SendPacer with PacerSim; pass PS1–PS8 before any socket test.
  - Then RpcGuard, ComputePool, CountingAlloc, RateBucket, LogsQuery, pocol_getLogs, BatchStreamer, SO_SNDBUF, PacerTrace, TraceCheck (with DiscriminationCheck), OriginDeny.
  - LN and RP window, logs/dense, DenseFixture (B, A', Blob), RpcAttacker, ReferenceLogs.
  - Tests: PS1–PS8, HG1–HG16, RG1–RG15 with RG2b, RG3-bridge, RG3b and variants; RG14(0)–(xii) with (0b) and perItemGrace control; T11b, T11c, T15, BR15, NX (d), NX-V1–V6 with V3b–V3g, NX0d, NX1d, LN0.
  - M7 cannot pass without PS, RG and BR15.
- M8 (requires VA-spec, D115): RetentionStore, TrieRebuild, LoadGen, ReplicaHarness for T25. Tests: T10, T10b, T10c, T13, T14, T18, T18b, T19, T25, T4-V.

**Interfaces — Consensus**
- GenesisSpec.encode/decode/verify → GsError{code, path}.
- ParamGate.check/derive: derive exposes Lmax, LAG, RPC_MEM, BATCH_SEND_MAX, its monitoring bound and the minimum BATCH_WALL as exact rationals.
- NetConfig.load, activeVersion, NetId.check.
- Difficulty.asert(pTs u64, pH u64, cp) → {target U256, trace{deltaT i128, e i128, s i128, f u16, F u32|null, saturated}}, without heap allocation, intermediates ≤ U512.
- Codec, Timing, Assigner, Proposer.draw, Membership, TemplateFSM, Fallback.build.
- End: isClosing, cursorDone, isFinal, bound(h).
- Consensus.headerPrecheck/validateFull.
- ForkChoice (D133):
  - importSeq(hash).
  - key(b) = (signedFlag, importSeq, blockHash).
  - better(t1, t2): by work, then compare the children of the lowest common ancestor.
  - best(candidates): maximum, independent of examination order.
  - onImport(b) → headChanged.
- Independent ForkChoiceRef (Python), used by HeadCheckRef in Q2b.

**Interfaces — Execution**
GenesisBuilder.build, Executor.apply, TipsAccumulator, SystemCall, Pay.credit, CapsTracker, Rewards.payInt, Escrow.pending(j), SettlementIndex, BurnMonitor.

**Interfaces — RPC (D88–D95)**
- HeadersParams.parse(json) → {from u64, count u64} | InvalidParams.
- HeadersParams.check(from, count, headFn) → ok | {fromZero | countRange | fromAboveHead | beyondHead}; headFn is not called before F3.
- MethodTable.classify(method, params) → {class ∈ {heavy, light}, maxResp} | Unknown. Fixed table matching network (B).
- OriginDeny.check(method, hasOrigin) → ok | Deny, fixed ORIGIN_DENY, called in stage 2 before admission.
- RateBucket.take(conn, k) → ok | Rate{retryAfterMs}, consuming k tokens without waiting.
- RpcGuard:
  - accept(conn) → ConnSlot | Reject: above 32, reject without reading; set SO_SNDBUF = SEND_SOCK_BUF through socket2 before any write.
  - admit(class, deadline) → Admission{permit, reservation} | Busy{retryAfterMs}, atomic FIFO admission.
  - Reservation: 64 KiB segments; write rejects overflow with Overflow; shrink(), release() in Drop.
  - Admission.snapshot() → ReadTxn, released with the permit in endCompute().
  - endCompute(admission) → OwnedResponse{reservation}.
- ComputePool.run(admission, job) → Result | ScratchExceeded | Deadline | Cancelled, with 20 threads and no await in job.
- CountingAlloc: thread-local counter, bounded per job.
- CancelToken: check() in scan/execution loops.
- LogsQuery.run(filter, snap, anchor?, budget{bytes, count, scan, deadline}, cancel) → Complete(Reservation) | LimitP1{reason, fromBlock, lastCompleteBlock ≥ fromBlock, nextFromBlock} | LimitRes{reason ∈ {deadline, scratch}, fromBlock, lastCompleteBlock ≥ fromBlock−1, nextFromBlock} | AnchorReject{reason, head} | Cancelled. LimitP1 and LimitRes encode as −32020.
- SendPacer (D90, D92, D93, D94):
  - Built on abstract Clock and SocketSink; same implementation in production and PacerSim. Does not directly import tokio.
  - SendPacer.new(clock, grace=2s, minRate=65536 B/s, slack=200 ms, timerEps=1 ms, trace?) → Pacer, state {sendTime, produced P, accepted A, pending, deadlineTimer, finished}.
  - Pacer.produce(n): P += n. Pacer.finish(): declares production complete; first subsequent A = P is sendDone.
  - Pacer.onWrite(result): A += accepted count, or WouldBlock; then update pending.
  - Pacer.violationAt() → s_v = max(grace, A/minRate); timer at s_v+timerEps with a backstop check every 50 ms.
  - Pacer.check() → Ok | Close{sendTime_ns, accepted, lateMs}.
  - Pacer.write(sink, bytes) → Done{sendDone_ns, sendTime_ns} | Closed{…}. Done means only A = P = size; it does not wait for client reads. Caller immediately releases reservation on Done.
  - A new Pacer for a single request, one Pacer per batch; no grace-reset interface.
  - rpc-fault:perItemGrace (simnet only) creates a Pacer per item, a negative control in PS5, PS5c and RG14(vii).
- PacerSim.run(script, variant ∈ {conforming, perItemGrace}) → {outcome ∈ {sendDone, close}, closeAt{sendTime, A}?, sendDoneAt{t, sendTime, Rc, receivedItems, unreadAtSendDone}?, receiveDoneAt{t, receivedItems}?, lateMs, rows[{t, Rc, P, A, pending, sendTime, s_v}], trace}. script = {mode ∈ {acceptPlan[(sendTime_ns, bytes)], sockBuf SB}, items[S…], clientPauses (pauseAtItemFirstByte=1.8s), computeGaps?, timerJitterMs?}.
- PacerTrace: 4 MiB buffer; overflow counted in traceDropped. Structure {conn, t_ns, sendTime_ns, P, A, pending, event ∈ {produce, accept, pendStart, pendStop, close, sendDone}}.
- TraceCheck.check(trace, clientLog, probeLog, params, rule ∈ {batchShared}) → {TCsend, TCrecv, CloseOK, Enforced, breachAt?, closeAt?, sendDoneAt?, receiveDoneAt?, unreadAtSendDone?, gap}. Matching Rust and TS versions.
- DiscriminationCheck(tracePerItem, clientLog) → {condition: met|unmet, enforcedReportsBreach}.
- BatchStreamer.run(conn, items, pacer, wallDeadline): writes '[', then per item: admit, ComputePool, endCompute, pacer.produce/pacer.write until A = P, release the item's reservation, then next item. After ']', calls finish and waits for sendDone. batchBytes/rate errors use CONN_BUF. wallDeadline = end of reading + BATCH_WALL, measured until sendDone. maxResPerConn ≤ 1 is debug_asserted and monitored.
- ConnLoop: after sendDone, start IDLE timer and permit reading the next request, without waiting for client reads.
- RpcStats.snapshot(), Mempool.admit, RejectLog.

**Interfaces — Registry and storage**
Registry, RetentionStore (with LogCursor and (height, blockHash) index), TrieRebuild, SyncPipeline, OrphanQueue.

**Interfaces — Fixtures**
- ExecFixture.run, FixtureMiner.build, Plan.override, FixtureVerify, PeerHarness (with a hook between client requests).
- MaliciousRpc.serve(chain, {drop?, replace?, fakeHeight?, extra?, refTs?, clock?, targets?, busyCount?}).
- MockLogServer.serve(branches{A, B, A'}, script): literal responses, branch switching at a specified step, eth_getBlockByNumber answers from the current branch, logging every request's order/type (H or logs)/attempt, and a response-hold option (for BR19b(b4)).
- SinkChecker.wrap(sink, serverLog): checks (S-a)–(S-e), records trace.
- EndFixture.ids(n=1020) → sorted list | FixtureSetupError('ids').
- DenseFixture.build(profile='logs/dense', branches=['B', 'A'']).
- RpcAttacker.run({…}) → monotonic timeline per byte/item, receiveDone or cutoff time, and close time.
- ReferenceLogs.read(from, to, branchHash).
- ResourceProbe.sample(pid, 100 ms) → {rss, heavyResv, lightResv, scratchHeavyMax, snapshotsOpen, conns, maxResPerConn, pacerClosed, pacerLateMaxMs, batchWallHits, traceDropped, sendDoneCount}.
- RpcRecorder, SimNet.run, LoadGen, KeccakTriad.check, AsertRef.
- RpcTap.serve(upstream, port) → log {seq, tIn, tOut, origin, method, params, source ∈ {readClient, walletSubmit, logClient, headerNet, other}}; RpcTap.hold(method, ms) delays responses. Source comes from a test header added by modules only in test builds.
- BadSite.build(vectors=BR1–BR13 and BR16–BR19c, txA): published site sending literal bridge messages and recording responses. GoodSite is a second site for BR17f isolation testing.
- TestHooks: approvals(), confirmations(), autoConfirm(accept|reject), tabsCreated() intercepting chrome.tabs.create, bytewise siteStorageSnapshot(netKey, addr), storageSetCalls() intercepting chrome.storage.local.set, bridgeTrace(frame).
- BridgeFuzz.run(seed, n=10^4) → messages and responses.
- FakeClock (configurable nowMs), FakeTransport (holds responses until release(i) or timeout) for BR19a/BR19b.
- SchemaRef (Python): schema and expected B0–B3 code. BridgeRef (Python): adds Bpre with integer bucket and B4 limits using BridgeTrace/RpcTap times, returning the expected decision per message. SiteStorageRef (Python, within BridgeRef): dictionary per (netKey, address), apply(msg) → {decision ∈ {null, 4300}, totalBefore, totalAfter} with literal entry/newTotal accounting.

**Interfaces — Extension**
- HeaderNetCheck.plan(h, kView=12) → {b, from, count, n, hasRef, anchoredGenesis}.
- HeaderNetCheck.ceil(targetG, slackS=4), minWork(ceilTarget) using BigInt.
- HeaderNetCheck.window(head, fetch, cfg, clock) → {ok, n, anchoredGenesis, minWork} | {rule}.
- TrustMessage.render.
- BridgeBucket (D97): new(createdMs) → {tokens_mt: 50000, last_ms: createdMs}. take(nowMs) → ok | rate, using browser's integer formula, no floating point.
- BridgeAuth (D95, D96, D97):
  - ALLOW: frozen Map from (kind, method) to Schema, built at module load, including site_storageSet/site_storageClear under storage_set and site_navigate/site_openExternal under nav.
  - check(rawText, frameCtx, arrivalMs) → {ok, id, kind, route ∈ {readClient, logClient, wallet, siteStorage, navigator}, method, params} | {id|null, code ∈ {−32600, −32005, 4200, −32602}, data}. Performs Bpre–B3 only. Pure except for frameCtx.bucket consumption; no networking or local effects. Does not read pendingReads or quota.
  - parseStrict(rawText) → rejects duplicate keys and depth > 8. Size checked earlier in Bpre.
  - Schema.validate(params) → ok | {path}.
- Bridge.onMessage(raw): stamps arrivalMs, runs check, routes to the component, writes a BridgeTrace row in test builds.
- RpcReadClient.send(frameCtx, method, params): rechecks method ∈ READ_METHODS; otherwise 4200 without networking. Checks pendingReads < 4; otherwise −32005 {pending} without networking. Increments before send; decrements on final response or 10s timeout (−32603 {transport}). Sole network transport exported to Bridge.
- SiteStorage (D96):
  - State: dictionary {key → value} per (frameCtx.netKey, frameCtx.siteAddress), with total calculated/cached in memory and verified by recomputation on load.
  - entry(k, v) = utf8ByteLength(k)+utf8ByteLength(v) through TextEncoder, not String.length.
  - set(frameCtx, key, value|null) → ok | Quota{limit}: computes newTotal = total − (old entry if present) + (new entry unless value = null). If newTotal > 1048576, returns Quota without writing. Otherwise builds and writes the new dictionary with one chrome.storage.local.set, then updates total. Write failure → −32603 {transport}, reload from storage.
  - clear(frameCtx) → ok, writing an empty dictionary.
  - snapshot(frameCtx) → loader dictionary on load.
  - Executed through StoreQueue (D101); no overlapping messages for one key.
  - check(frameCtx, msg) → ok | Quota{limit}: computes only newTotal without writing; called by StoreQueue.evalHead.
  - set is called only for a ready message that passed check, defensively recomputing newTotal. Any discrepancy is an implementation error counted in stats.quotaRecheckMismatch; nonzero fails.
- StoreQueue (D101):
  - admit(session, msg, arrivalMs) → queued | Store, before appending, checking all four counters.
  - evalHead(key) (I56): called when the key's head is released, or a message is accepted for an empty key. Calls SiteStorage.check:
    - Quota: reply 4300, then release.
    - ok: message becomes ready.
    - Takes no seat and does not call StorageBackend.
  - pump(): if a seat is available, takes the oldest ready message in the global FIFO by admission order, makes it active, calls set. One active write per key, ≤ 2 extension-wide.
  - onSettle(reqId): only event moving an active message to released.
  - expire(nowMs): releases expired waiting messages with store replies; replies storeTimeout for expired active writes without releasing.
  - release(msg) internal, guarded by released flag. debug_assert prevents repetition; stats() counts releaseCount/doubleReleaseCount. Nonzero doubleReleaseCount fails.
  - Per-message state {replyState, ownState} under D102, recorded in BridgeTrace.
  - cancelFrame(frame): cancels waiting work, leaves active work untouched.
  - awaitActive(key, ≤ 5000 ms) for Navigator.
  - stats() for TestHooks.
  - Runs over abstract StorageBackend: chrome.storage.local in production, FakeStorage in tests.
  - Key derived from frame context, not message.
- EpochFence.boot(backend, monoClock) → {epoch} | Disabled{reason ∈ {epochNames, retry}} (D134):
  - Lists epoch items, applies name gate then generation window.
  - Writes 'pocol:epoch:'+hex16(E), waits for confirmation.
  - No data write is issued before success.
- EpochFence.sweep(): scheduled after T_LATE monotonic time from boot; removes names lower than the greatest visible name.
- EpochFence.stats → {epochNames, epochGateBlocks, epochResurrected}.
- RecoveryGate.await(key) → {dict, total, confirmed} | Failed{reason ∈ {corrupt, checkpoint}} | Busy | ReadTimeout: one shared promise per key per generation. Carries gateId; starts total read deadline on entering recovering (D110). Issues only the anchor and its second attempt, each with a RecoverySlots seat. Busy/ReadTimeout return the key to unrecovered without issuance. Every read response's gateId is checked before use.
- ReadSlots (D110): acquire(op, deadlineMs) → Slot | Timeout through ReadFIFO. slot.settle() is the only release event. stats → {held, timeouts, lateDropped}. StoreQueue does not evaluate a head or provide a snapshot before ready.
- RecoverySlots (D109): acquire(op, waitMs=5000) → Slot | Busy through RecoveryFIFO, in the same synchronous step as DiskLedger.reserve and issuance. cancel(request) on ready. slot.settle() is the only release event after issuance; no timeout-release interface. stats → {held, unsettledMax, busy}.
- SeqAlloc.alloc(key) → u32 | Exhausted: sole allocator of set sequence numbers in the generation.
- DiskLedger (D105):
  - reserve(op) → DiskTicket | {disk|sites|entries, limit}: synchronous, no await, checks and reserves in one step.
  - ticket.settled(settleSeq).
  - Serialized refresh() calls getBytesInUse/getKeys; upon completion, drops tickets whose settleSeq is below its issue number.
  - StoreQueue, RecoveryGate and AdminDelete never issue set without a ticket.
  - (D112) lateGens(now) from epoch items listed at boot and the live-generation item. lateSites(now) = 4·lateGens.
  - reserve(op) determines whether the operation creates a key: data set or anchor for an uncounted, unreserved key. Applies the site condition; returns {sites, limit, retryAfterMs?} on violation.
  - onWindowExpiry(X) schedules refresh, removing X's contribution after a refresh issued beyond bootMs(X)+T_LATE completes.
  - removeLastRecord(key) allowed only under D112; called only by TombReaper.
  - (D113) reserve(op) on a counted key takes no seat, adding a pin unless the operation is remove. refresh drops a key from sitesLive only if unseen by measurement and unpinned; drops pin in the same step as ticket removal.
  - TombReaper.scan(): called after every adopted refresh and every epoch-window exit; issues remove for eligible sole tombstones. stats → {tombReaps, pinnedKeys}.
- StorageManager (extension page): list(), deleteSite(netKey, addr) with user confirmation, returning the shared AdminDelete promise → deleted | failed | uncertain.
- AdminDelete (D106, D108): four-step run(key), generation-unique opId (D111), SessionRegistry.closeAll(key), Viewer.blockOpen(key). Result → deleted | failed | uncertain | adminBusy.
- AdminSlots (D108):
  - acquire(op, waitMs=5000) → Slot | Busy through AdminFIFO, in the same synchronous step as DiskLedger.reserve.
  - slot.settle(): sole release event. No timeout-release interface.
  - (D111) acquire(op{opId, key}, waitMs) → requestId; cancel(requestId) called in the resolution step.
  - grant(request) calls guard(op, key) before DiskLedger.reserve/SeqAlloc.alloc, without intervening await. Failure returns the seat to the next FIFO head and increments adminStaleDropped.
  - DeleteOp: {opId, key, state ∈ {running, settled}}; settle(result) changes state and withdraws requests in one step.
  - stats → {held, unsettledMax, busy}.
- RecordCodec: name(key, epoch, seq), strict fmt 2 encode/decode with RECORD_BYTES_MAX.
- StorageBackend adds three functions:
  - listKeys(): through getKeys only.
  - bytesInUse().
  - remove(keys).
- ViewerLink: hello{genId}, onLost() → teardownAll() with notification.
- SR-model (Python), FakeBackend(pending, scheduler) for BR22a/BR22b.
- Navigator (D96, D99): navigate(frameCtx, path), in order:
  1. Debits the session's navigation bucket, otherwise −32005 {nav}.
  2. Applies storage normalization.
  3. Orphans frame requests, cancels the previous LoadJob, calls StoreQueue.cancelFrame, then awaitActive before snapshot (D101).
  4. Asks viewer to tear down the frame and build a new one joining the same session, or displays a 404 page.
- SiteSession (D99):
  - Interfaces: open(tabId, netKey, siteAddress) by user action only, attach(frame), orphan(frame), settle(reqId), close().
  - Owns BridgeBucket, NavBucket, pendingReads, LoadJob, ChunkCache and SESSION_RECV_MAX reservation. openExternal(frameCtx, url) → confirm → null | 4001 | −32005 {pending}; opens a tab without opener.
- WalletSubmit.submit(rawTx, approvedTxHash): verifies keccak256(rawTx) = approvedTxHash, then sends eth_sendRawTransaction. Called only by Wallet after approval.
- HttpTransport (D98): internal; imported only by RpcReadClient, WalletSubmit, LogClient, HeaderNetCheck, NetworkProfiles and ChunkFetcher.
  - request(method, body, {recvLimit, deadline, pool, session?}) → {json} | {error ∈ {recvLimit, recvDepth, recvParse, transport, busy}}.
  - Does not call text() or json().
- RecvGuard:
  - reserve(pool, session, bytes, waitMs=2000).
  - read(stream, limit): counts each chunk before appending.
  - release(): on settlement.
  - stats(): for TestHooks.
- ChunkFetcher.fetch(addr, len, session): checks ChunkCache first, then fetches through HttpTransport from CONTENT_POOL.
- MaliciousHttp (Node): scripted MR server. A dependency-cruiser rule prohibits Bridge from importing it or WalletSubmit.
- LogClient: plan(from, to) → windows ≤ 1024 marked withinLimit=false. Pure step(range, reply) → Append | Replace | Retry | Restart | Error. Sink and RequestCounter (rejects before sending if totalRequests would exceed maxRequests). run(rpc, filter, sink, {timeout, maxRequests, maxRestarts=3}). fetchAll(rpc, filter, {maxBytes=4 MiB, timeout=30s, maxRequests=4096}) → Complete | Error{tooLarge, timeout, tooManyRequests, deadline, scratch, serverViolation, reorgUnstable, beyondHead, unsupported}, counting as one pending frame request. fetchEach(rpc, filter, sink, {timeout=600s, maxRequests=65536}).
- Viewer, Loader (builds localStorage from snapshot, batches writes every 250ms with a bucket-respecting queue, restores confirmed values on 4300, intercepts links), DnrGuard, NetworkProfiles.validate, Wallet, TestHooks, CanarySink.

**Dependencies**
- M2 after M1; M4 after M3; M4b after M4; M5/M6 after M4; M7 after M5; M8 after M5/M6.
- Within M2: BridgeBucket, BridgeAuth and RpcReadClient with BG1, BR19a/BR19b first; then SiteStorage with BR17e on a fake dictionary; WalletSubmit, Navigator and remaining BR; then T12.
- Within M7: SendPacer, PacerSim and PS first; then PacerTrace, TraceCheck and DiscriminationCheck; then RG and BR15.
- HeaderNetCheck and NetworkProfiles.validate import asert and the M3-TS model.
- LogClient tested with MockLogServer, then against pocold in RG3, RG3b and RG9.
- Phases A and B run in parallel, except shared asert delivered first in the second task.

**First reviewable, testable task: M1-spec**
A text document containing:
- Initcode bytes and address formula, with three vectors: 0x61, 24575 bytes of 0xaa, and a specified html file.
- Manifest encoding/normalization, Website slots/permissions, T1 and T16 with literal inputs.
- Extension annex: netKey vector (777001, 0x11×32), signEligible, Q8–Q12, X1–X7, DNR-d1–DNR-d5.
- Bridge-authorization annex (D95, D96, D97):
  - Literal envelope {id, kind, payload} and payload {method, params}, Bpre/B0–B4 order, literal error codes/data, and returned-id rule (value or null).
  - Integer bucket formula (capacity, refill, cost, consumption rules), arrivalMs definition, pendingReads definition and increment/decrement events.
  - Complete (kind, method) matrix for all four classes, machine-readable method schemas, primitive types with regexes including str, pathStr and httpsUrl.
  - SiteStorage semantics: entry/total/newTotal for insertion, replacement, deletion and clearing; ≤ 1048576 condition; single-write atomicity; key derivation. Navigator semantics: normalization, confirmation, no reply to torn-down frames.
  - Explicitly tested prohibition list.
  - Flowchart of the sole write path through WalletSubmit.
  - Literal dependency-cruiser rule.
  - Byte-literal BR1–BR14 and BR17–BR18 messages with expected responses, including BR7a–BR7h in violation order; literal txA/hash; RpcTap commands and “zero requests” criterion.
  - Complete BR17e table: all twenty-four literal messages and byte lengths; total before each, newTotal, total after, response: (e1) 61443·i through 1044531; (e2) 1105974 → 4300; (e3) 1048577 → 4300; (e4) 1048576 → null; (e5) 1048580 → 4300; (e6) 1048577 → 4300; (e7) 987137 → null; (e8) 925694 → null; (e9) final dictionary and total 925694.
  - BR19a (a1)–(a7) tables with arrivalMs/tokens_mt before and after each message; stepwise BR19b (b1)–(b5); BR19c script; BridgeRef specification and resolution criterion.
  - BR16 generator/seed; SchemaRef, BridgeRef, SiteStorageRef; BridgeTrace format.
  - D98 annex:
    - RECV_LIMIT table/derivation, exact/capped classes, WorstLegit/RecvFit formulas (D100).
    - MR6b–MR6d, CAP1, RF1–RF4 vectors; MR10a–MR10f timed scenarios.
    - Read steps 1–6 and pools.
    - Bytewise MaliciousHttp scripts for MR1–MR12.
  - D99 annex:
    - SiteSession lifecycle and orphan-request states.
    - Navigation-bucket formula.
    - Corrected BR20a table with check/route columns, tokens_mt/pending after each message, orphan settlement time; the same table transferred to BridgeRef.
    - BR20b table; BR20c script and criteria.
    - BridgeRef session-semantics extension.
  - D101 annex:
    - qbytes formula, four counters, per-key FIFO order, global active bound and both timeouts.
    - Teardown rule and waiting before snapshot.
    - BR21a (q1)–(q10) (q10 corrected to sixteen sessions), BR21b (r1)–(r6), BR21e, BR21f and (f2) rejecting quota before set, with times/counters/responses/reply/release/storeViolated events; TQ parameters; BR21c/BR21d scripts; replyState × ownState transitions (D102).
    - BridgeRef/SiteStorageRef extended with StoreQueue model.
  - D103 annex:
    - Epoch-item and record name/value formats; greatest-selection rule.
    - (D134, first review before storage-recovery implementation):
      - Shared name per E, name gate, post-T_LATE sweep rule.
      - Literal writer, name-bound and coverage lemmas.
      - BR22h (h1–h6) tables with three controls; SR-model extension.
    - Fence/recovery steps and both lemmas literally.
    - Viewer behavior on disappearance.
    - Full BR22a combination table with expected state per combination; BR22b/BR22c scripts.
  - D104 annex:
    - RecoveryGate state machine, SeqAlloc and bounds.
    - fmt 2 format; RECORD_BYTES_MAX derivation.
    - Disk-admission rule, generation window, sweeping, tombstone.
    - BR22d with V-a..V-e variants; BR22e; BR22f with f2–f4 variants and RecoverySlots state machine.
    - BR22g (g1–g6) with read phase, ReadSlots, gateId (D110), deriving the 25s resolution bound from t_s.
    - Literal BR23 (d1–d11, including d11b corrected with the complete name condition and epochReserveOmitted control path, D135) and BR24, with AdminDelete/DiskLedger/AdminSlots state machines (D105–D108).
    - Derivation of LATE_NAMES = 24 excluding epoch items and EPOCH_NAMES_MAX = 8 (D109, D134; 28 superseded), META_RESERVE constraint, AdminDelete = 45s bound, BR24(x9–x12) with x12b and BR22f-f5 variant.
    - DeleteOp state machine (D111), AdminSlots/RecoverySlots grant guard, literal no-late-tombstone lemma.
    - (D112) sitesLive, creating operations, lateGens/LATE_SITES formulas and expiry rule, admission condition, final-tombstone removal, literal site lemma, A15d.
    - Timed BR23(d1, d9, d12–d15) with lateGens/LATE_SITES/measurement values; BR24(x6); SR-model extension.
    - (D113) Literal tombstone-counted-key rule, pin, TombReaper and new-seat requirement.
    - (D114, I68) Corrected BR23(d15c), both correct/dropPinned paths, including:
      - Boot and expiry times.
      - lateGens, LATE_SITES, sitesLive, pinnedKeys after every refresh.
      - Test-only DiskLedger.refreshNow() hook; table transferred to SR-model.
      - Literal responses at 30100 in both paths, transferred literally to SR-model:
        - Correct: 4300 {reason: 'sites', limit: 64}, without retryAfterMs.
        - dropPinned: 4300 {reason: 'sites', limit: 64, retryAfterMs: 29900}.
        - Both values derived from D112's condition.
- HeaderNetCheck vectors: chainId=777002, RW1–RW6, h=3 anchored at genesis, viewIncomplete for missing header/reference at h=20, viewGenesis, h=0 → viewNoBlocks, HC1–HC7 with literal decimal minWork and message texts, NX-V3e, NX-V3f, NX-V3g.
- LogClient vectors: A/B/A' with literal logs/hashes, 8/0/2 table, LC1–LC18 literal JSON responses including −32020 (P1/RES), −32021 (busy/rate), −32022; literal H(·) request sequences; sink trace with each commit's summary; totalRequests table: LC1=5, LC5=5, LC8=7, LC10=4 sent, LC16=11, LC17=12 (variant 8), LC18=10; RefA/RefB/RefA'; complete step transitions for every (reason, lc, a=b, withinLimit) combination.
- NetworkProfiles.validate: GSV1, N1, N6, N11.
- X8: C1–C8 pages, K0–K5 flags.

Reviewed by Codex, then implemented outside this session.

**Second task (M3, TS)**
- Encoding/identity: K1–K3, KeccakTriad, GenesisSpec, NetConfig, NetId.
- Difficulty: asert with AS1–AS19, AS-eq, ASQ, ASB.
- Gate: ParamGate with PG-END1–5, PG-L, PG-RPC.
- Headers: Codec (L0), structural H-pre (L1), search (L2), Timing (TJ), Assigner.
- Execution: RegistryLayout, payInt, BurnMonitor.
- RPC/extension: HeadersParams (HG1–HG8, HG14–HG16), HeaderNetCheck.plan (RW1–RW6), ceil/minWork (HC1–HC7).

**VA-spec annex (D115)**
Written before M5 code and reviewed by Codex. Contains:
- Literal va/base profile with genesisPre bytes and identity rates.
- NetSim interface/commands; AdvNode interface (policies: silentProposer, splitTemplates, headerOnlyWithhold, selfish, doubleSpend, ownSharesOnly, emptyShares, winnerEquivocate, splitIdentity(k), churn, eclipse, censor).
- Every VA-annex test script with times, messages and faults.
- TraceDB format; each metric and denominator computed by an independent Python reference.
- Measurement barrier (D122, D123):
  - halt, stopMining and statusSweep commands.
  - Machine-readable Quiet definition from NetSim state and each node's ChainWork.
  - F1–F3 policy/purgeFuture; U/A/V paths with purgeUnavailable, holding definition, t_avail (D124); closed U-reason set {bodyUnavailable, envUnavailable, depUnavailable}, validEnv and EnvHeldAtHalt with one classification shared by SimNet.recSet, HeadCheckRef and BudgetCheck (D131, D132); Rec, DrainPin, Need(i), HR, RB, c_rd, T_REBUILD_MAX, PIN_MEM_MAX (D125); scoped SYNC_RETRY_MAX and RESTART_MAX; DRAIN_MAX(run) formula with N_miss, c_b, DRAIN_CAP; QUIET_HOLD.
- HeadCheckRef Python reference, including:
  - RefReplay.
  - Literal QV1–QV26, QV23d, QV25c-dup, QV26m and T23 N9/N10, including QV19–QV23 and revised QV20 (D128), removing its predecessor, with:
    - BudgetCheck table and ActLog classes.
    - Default SimParams and per-vector overrides.
    - Timestamps, targets and equal-work-per-height lemma.

    Alongside qv/net with genesisPre bytes and clock/eligibility tables.
  - Forty-six simnet-fault builds: envRequestBeforeParent (D140), envCheckSharedPool (D139), envReleaseOnCancel, envTimeoutCoversCheck (D138), envSolicitUnbounded, envReplyUnreserved, envTimeoutExclude (D137), envCacheUnbounded, envFullAsBlockReject, envDropSolicited (D136), pairwiseTiebreak (D133), globalRtt, envAsBlockReject, noEnvUnavailable, noEnvFetch (D131), statusResetsTried, envRejectCancels, hfullDoneOnEnv, execPerCopy (D130), cacheByBlockHash, mergeInFlight, skipHp, noReeligible (D129), noFetchDedup, cacheNotEvaluable (D128), kpendIgnored, noHdrCache (D127), pinLate, needHeadsOnly, rbPerBranch, hdrUndercount (D126), rejectOnBadBody, excludeDepAsInvalid, noDrainPin, drainUndercount, silentDeepReorg (D125), noSignedTiebreak, noReorgOnHeavier, execDrift, quietIgnoresFuture, purgeImported, untaggedTimer, keepUnavailable, purgeHeld, skipHolder, noCancelOnReject (D124).
- CEN table; BootRef (D121) with fixed-seed vectors/literal results for TS comparison; SV10a–f vectors (D120).
- FSM1–FSM10, N1–N8 and L1–L13 tables with expected states.
- ReplicaHarness for T25: literal T25Site files; kill, wipe, S0 creation/signing/retrieval commands; machine map; ReplicaMonitor list.
- Byte-literal caps/64 profile, SYS_SLOT_MAX derived from R18, SlotFill contract, L1–L13 loads, caps-test constants.
- AdvOracle and its bound, excitation definition for each conditional test, T_RUN_MAX per test.
- Runtime estimate on Windows with 8 GB; schedule arms in batches without changing criteria.

**M7-spec annex**
Written before M7 code and reviewed by Codex. Contains:
- Classification table with worst-size derivation per method; ORIGIN_DENY table.
- Resource-ownership lifecycle (1–7) state diagram, with release at sendDone.
- SendPacer diagram: P, A, pending, clock start/stop, s_v, TIMER_EPS, timer reset, backstop check, CloseOK, finish, sendDone.
- Literal PacerSim model: both acceptance modes, next-item production rule, client behavior, within-instant fixed-point iteration, sendDone/receiveDone/unreadAtSendDone definitions.
- Literal PS1–PS8 scripts with (t, Rc, P, A, pending, sendTime, s_v, event) tables for conforming/perItemGrace: PS5/PS5c through receiveDone (PS5: sendDone = receiveDone = 36. PS5c: sendDone 32.4, 18 items read, unread = 196608, then receiveDone 36), and staircase PS4 derivation.
- PacerTrace format; mathematical TC-send, TC-recv, CloseOK, Enforced, Breach, DiscriminationCheck definitions; RG14 C-case conditions as functions of complete traces; RG14(0b).
- Table naming each timeout's measured event, all server events.
- Literal JSON shapes for both −32020 types, both −32021 reasons, and −32022.
- Bytewise streaming-batch example, RG plan with RpcAttacker commands/criteria, BR15 plan.

## Decisions

- D1: A Rust pocold node using revm.
  Rationale: Compared with a geth fork (LGPL/GPL licensing and ongoing maintenance) and Engine API (does not enforce the template before mining). The chosen option controls determinism and fits 8 GB.
- D2: Two stages with separate evidence: A on anvil, B in PoCol.
  Rationale: Success in A does not establish B.
- D3: A modified Cancun profile with disclosed exceptions.
  Rationale: Preserves tool compatibility.
- D4: A template signed by the attempt-0 proposer, with deterministic fallback.
  Rationale: Mempool derivation does not establish agreement [PDF:100]; a quorum would turn the protocol into BFT.
- D5: Seed and Draw derive from the parent seal.
  Rationale: Prevents grinding through template content [PDF:91].
- D6: Shares in shareList signed by the winner.
  Rationale: Compared with a subsequent share window and heartbeats [PDF:105].
- D7: Settlement deferred D blocks, using denominator max(C, κm) and burning.
  Rationale: Denominator C alone makes exclusion profitable.
- D8: B_reg bond held throughout membership.
  Rationale: [PDF:89 §5.4.4].
- D9: uint64 nonce and bounded mode with c=2.
  Rationale: [PDF:95]; T8 settles the value.
- D10: S3 for the experiment; S4 required for the public network.
  Rationale: SSTORE costs more; calldata does not put bytes in state.
- D11: Immutable versions, rollback through setCurrent, no deletion until termination.
  Rationale: EIP-6780.
- D12: Render in an opaque-origin sandbox.
  Rationale: Extension pages expose chrome.*.
- D13: DEV, LN and RP trust levels.
  Rationale: An MPT proof does not establish root validity.
- D14: An EIP-1193 provider with two network-bound approvals.
  Rationale: MetaMask is unavailable from the sandbox.
- D15: Cumulative work, then lexicographic branch-path order at the first difference using (signed, importSeq, blockHash). Details in D133.
  Rationale: [PDF:74, PDF:76] specify cumulative work. The earlier formulation (signed at first difference, then first-seen tip) is not transitive (I73).
- D16: powHash over TemplateID‖nonce.
  Rationale: Removes circularity.
- D17: [L(a), U(a)] windows using u128 arithmetic.
  Rationale: Historically verifiable.
- D18: Storage budget approximately 29.5 GiB with N_rep ≥ 3.
  Rationale: Derived from the inventory; D85 makes its time bound effective.
- D19: A MessageChannel bridge handed over once.
  Rationale: The sandbox origin is null.
- D20: Limits with lengths calculated from structure.
  Rationale: Consistent worst cases.
- D21: A_h* from active, non-burned members; an empty set means an announced halt.
  Rationale: Falling back to M_h assigns a range to a nonproducing member.
- D22: Sticky FB; fallback timestamp L(0)+Δ_fb_wait.
  Rationale: Prevents splitting honest miners.
- D23: ts = max(L(0), C), chosen by the proposer.
  Rationale: ASERT needs actual time; this is a disclosed deviation from [PDF:90, PDF:99].
- D24: Consensus caps, then closing, then termination.
  Rationale: Shared verifiable rules.
- D25: System calls protected by SYSTEM_ADDRESS.
  Rationale: Prevents manipulation.
- D26: An inline loader with a CSP hash, and approval bound to txHash.
  Rationale: Trusted startup.
- D27: Byte-specified factory initcode; a stateless factory.
  Rationale: Literal, testable derivation.
- D28: Genesis through a configuration file.
  Rationale: [PDF:96 §5.7.2].
- D29: Winner-equivocation evidence (type 2).
  Rationale: Including shareRoot in PoW turns it into an extranonce.
- D30: Trie with K_eff ≤ 32, 2048-block diffs and TrieRebuild.
  Rationale: The value 128 exceeds 6 GiB.
- D31: A template state machine and a queue for templates arriving before their parent.
  Rationale: Resolves late, early and second templates.
- D32: Draw over A_h* weighted by shares.
  Rationale: Historically verifiable validity.
- D33: Byte-level P2P schemas and aggregate resource bounds.
  Rationale: A message limit alone does not bound memory.
- D34: Synchronous snapshot storage keyed by netKey, with writes through storage_set using the unified envelope (D96).
  Rationale: Provides localStorage semantics without exposing chrome.storage.
- D35: Composite WAIT trigger.
  Rationale: An absolute timer failed when the parent was late.
- D36: Build_fb with full execution.
  Rationale: Skipping system calls breaks state.
- D37: Three isolation layers: CSP, DNR and RtcLockdown.
  Rationale: CSP and DNR do not cover STUN.
- D38: Two-stage synchronization.
  Rationale: Eligibility requires parent state.
- D39: Compute closing, gate2 and final before execution.
  Rationale: Compared with manual settlement.
- D40: Derived timing parameters with a boot gate.
  Rationale: The previous relation omitted parent search time.
- D41: Registry functions fail with Closing.
  Rationale: Compared with partial prohibition.
- D42: The block message broadcasts only the header.
  Rationale: The complete block is 65 times larger.
- D43: Exclusive branching between signed and fallback templates.
  Rationale: Compared with signed templates for every a.
- D44: SimNet runs actual pocold modules.
  Rationale: BlockSim does not run the rule implementation.
- D45: P11, P15, P51 and P55 profiles derived from the gate.
  Rationale: One dev profile violates the gate.
- D46: T18b with a caps/64 profile.
  Rationale: Full caps cannot be reached within test time.
- D47: T19 uses cursor progress per height as its unit.
  Rationale: Avoids mixing two units.
- D48: A 14-component inventory.
  Rationale: Compared with a single byte ceiling.
- D49: TXBYTES_CAP applies to len(rawTx).
  Rationale: Covers the envelope.
- D50: SYS_KEYS and SYS_ACCT units, and sticky closing.
  Rationale: Compared with raising the cap.
- D51: Every system transfer goes through PAY.
  Rationale: A value-bearing CALL permits invalidating the block.
- D52: Hash-keyed trie nodes with reference counts.
  Rationale: Path keys do not support historical proofs.
- D53: A single R1–R26 gate, with only one exception in R21.
  Rationale: A parallel gate opens an exemption leak.
- D54: The user's system-resource budget is zero during closing.
  Rationale: Makes KF and AF true by construction.
- D55: Compressed receipts; logs stored once.
  Rationale: Compared with full storage.
- D56: bondState separate from the tombstone; evidence precedes the cursor.
  Rationale: The cost is weaker deterrence during closing.
- D57: final requires h ≥ closeH+D with membership frozen.
  Rationale: Compared with a union bound including rotation.
- D58: netKey identity and signEligible inside ProfileMutex.
  Rationale: chainId alone is insufficient.
- D59: reactivate for an exited, HELD record after W_act.
  Rationale: Compared with free reentry.
- D60: A deterministic K_eff trie window, as a retention rule.
  Rationale: Compared with a grindable consensus cap.
- D61: A compact registry with swap-and-pop and rotating sweep.
  Rationale: Compared with an index containing holes.
- D62: Authorization through msg.sender; only removeInactive is public.
  Rationale: Compared with reactivate(id) plus a check.
- D63: X8 with TCP/UDP sinks and control K4.
  Rationale: SOCKS does not observe UDP.
- D64: The gate computes derived values and returns every violated rule.
  Rationale: Compared with returning only the first rule.
- D65: payInt = floor(R·c/max(C, κm)).
  Rationale: [PDF:104 §5.9.2].
- D66: Classified burns, with one burnedTotal slot.
  Rationale: Compared with per-kind counters.
- D67: finalBurn = blockTips_F; block F settles everything remaining.
  Rationale: Shares of F are not recorded.
- D68: A host tips accumulator passed through protected calldata.
  Rationale: Implements the fees component in R_tot [PDF:103]. Compared with an Escrow balance difference and EIP-1153.
- D69: R22 for profile policy; R23 for the overflow bound.
  Rationale: A minimum-subsidy constraint is arbitrary.
- D70: Forced transfers and adding tips to COINBASE are not system writes.
  Rationale: Counting them as writes would invalidate every tipped transaction during closing.
- D71: ExecFixture and ChainFixture layers, with a deterministic search-bounded FixtureMiner.
  Rationale: Fallback grinding is prohibited.
- D72: Stateless item 1b: identity and version from cfg.
  Rationale: Compared with Status alone, removing fields from UT, and checking only in H-full.
- D73: T0 with L0–L4 layers.
  Rationale: Old V1 meets the target only with probability 2^−16.
- D74: Absolute-anchor ASERT with AS, ASQ and R25; binding arithmetic in D83.
  Rationale: Compared with a moving-average window and parent-relative adjustment. Absolute anchoring computes from the parent header alone, enabling H-pre and RP checks.
- D75: Three-way rejection expectations with diagnoseChain; template channel for signed form only.
  Rationale: firstReject alone hides fixture defects.
- D76: Fully self-describing chain vectors, with T_j in milliseconds.
  Rationale: Referring to a previous revision does not permit independent execution.
- D77: GenesisSpec includes CP; the extension carries genesisPre; RP window [max(1, h−12), h] checks items 1, 1b and 2–9 with a reference or genesis anchor. Details in D82/D84.
  Rationale: Compared with uploading fixtures and rejecting h < 13. Anchoring to genesis is stronger than a disconnected window.
- D78: pending[j] is a reversible consensus slot keyed by height. closeBlock(j) writes amount, kind and proposer; recordResult(j) writes winner in block j+1 (D141).
  Rationale: Compared with a blockHash key and recomputation at settle. Writing winner in block j itself makes the UT-committed stateRoot depend on the mining outcome (I76).
- D79: GenesisSpec version 1 schema: 52-item CP, M_0List, allocRoot, sysCodeHash, seven decoding errors and GSV1 vector.
  Rationale: Compared with an RLP dictionary and canonical JSON. The positional list is smaller and deterministic; the hash is recorded through agreement among three libraries.
- D80: At h=0 in RP: no header request, viewNoBlocks and 4901. In LN: normal viewing and signing.
  Rationale: A zero-count request falsely suggests verification; eth_getProof against allocRoot is more complex. Refusal until block 1 is simple and honest.
- D81: Check pocol_getHeaders in fixed first-violation order: F0 type (−32602), F1 fromZero, F2 countRange, F3 fromAboveHead, F4 beyondHead (u128). The last four return −32018.
  Rationale: Checking state first is unnecessarily costly; returning all reasons complicates clients. The choice is deterministic and consistent with firstReject.
- D82: Three RP counts: window W = [b, h] with n ≤ 13; uncounted reference b−1 when b > 1 checked by 1, 1b and viewFuture; retrieval from = max(1, b−1), count ≤ 14. Too few or too many gives viewIncomplete.
  Rationale: Shrinking the window weakens verification; fully checking the reference recurses; omitting it leaves parentHash(b) without a comparison.
- D83: Fixed-width ASERT: Δt, e, s and f in i128; early saturation at s ≥ 256 or s ≤ −257; k ∈ [−272, 239]; operands ≤ 512 bits. Output equals D74 by the equivalence lemma.
  Rationale: Allocation precedes an external timeout; a consensus rule for |Δt| changes validity; floating point is nondeterministic.
- D84: Two RP-client constraints before PoW: viewFuture for ts > C+Φ_view, and viewTargetCeil for target > min(2^256−1, target_g·2^4). The only advertised bound is numeric minWorkRP = work(ceilTarget), labeled ineffective at 1.
  Rationale: Checking the reference timestamp recurses; a signed checkpoint adds centralization; schedule comparison rejects honest networks. work is nonincreasing, making the bound exact.
- D85: Binding height bound: R13, R20, item 10a (h ≤ H_END) and theorem E1.
  Rationale: Derived H_MAX is complicated; liquidation in one block exceeds G_SYS; burning remaining bonds confiscates honest members' bonds.
- D86: R16 symbolically defines LAG = 2048 and Lmax = max(U, 2·E_win+D+1). end/full profile: R_max=0, M_max = REG_RECORDS_MAX = |M_0| = 1024 at genesis, RET=8, Z_close=147, H_CLOSE=553, H_END=700.
  Rationale: Filling through registration takes thousands of blocks; exemption is prohibited (D53); state injection bypasses the rules. Cost: registration turnover is not exercised.
- D87: RpcGuard is a local service policy binding on implementation but outside validity: heavy/light classification, global bounds, eth_getLogs range ≤ 1024, ≤ 10000 logs and ≤ 32 MiB scanned, explicit rejection without truncation, cancellation ≤ 100 ms, separate runtime and R26. PS and RG gates are required for M7. Details in D88–D94.
  Rationale: Reducing range does not bound concurrency; streaming without global limits does not bound snapshots; a session cursor needs state; per-source limits can be bypassed with multiple connections.
- D88: RPC resource ownership lifecycle: sequential HTTP/1.1 (S1), seven phases with start points/deadlines, streamed batches without resources between members, 48 MiB heavy and 16 MiB light budgets, thread-local SCRATCH accounting in a 20-thread pool, 178 MiB ≤ 192 MiB, derived classification table and per-connection token bucket with FIFO.
  Rationale: Per-member reservation in an accumulated batch can reach 512 MiB. HTTP/2 breaks S1. A shared budget starves light traffic. Estimated accounting does not bound revm. Cost: new connections and heavy traffic can be locally denied service.
- D89: pocol_getLogs(filter, anchorHash) with anchor/P2 and exact P1 bound (P3). LogClient retries [a, lc], then [lc+1, b]; fetchAll capped at 4 MiB; fetchEach for tests.
  Rationale: Resuming from nextFromBlock loses data. A partial result with success status misleads. Later blockHash verification does not detect empty blocks. Per-block filtering is costly. A session cursor can be exhausted. Cost: double scanning and a nonstandard method.
- D90: A send controller with one clock per response and batch, advancing only while pending: one-time GRACE, cumulative-rate rule, no per-member grace, derived BATCH_SEND_MAX = 258.0791015625s and reserve BATCH_WALL = 420s ≥ 404.2791015625s.
  Rationale: Per-member deadlines preserve repeated initial grace. A wall clock punishes honest readers for server slowness. Low BATCH_WALL interrupts honest batches. Cost: an intermittent reader without credit may be closed.
- D91: Complete LogClient transitions: P1 versus RES, splitting or bisection, 3 per-block retries, withinLimit marker, transactional sink with attemptId, anchor check before commit, S-a..S-e invariants and three counters with maxRequests on totalRequests checked before sending.
  Rationale: Treating scratch as P1 assumes unguaranteed progress. Immediate rejection turns temporary pressure into failure. Deferring all pieces recreates accumulation. Invalidation without an ID cannot distinguish late pieces. A per-attempt counter does not bound restarts.
- D92: Credit semantics and controller enforcement: credit = A−R·sendTime without a cap; timer at s_v+TIMER_EPS (1 ms), backup check every 50 ms, delay ≤ 200 ms; strict bounds = exact values + ENFORCE_SLACK; SO_SNDBUF = 32 KiB; accepted-versus-read difference is measurement only; CloseOK governs every closure.
  Rationale: Sliding windows and buckets do not reduce maximum occupancy and penalize intermittent readers. Periodic checking with a bound equal to the theoretical value is contradictory. sendTime ≤ max(GRACE, A/R)+ENFORCE_SLACK holds regardless of pause distribution.
- D93: Two-layer controller verification: (1) PacerSim runs the same implementation over abstract Clock and SocketSink, with a literal model distinguishing P, A and Rc, two acceptance modes, next-member production immediately at A = P, specified client behavior, iteration to quiescence within each instant, and PS1–PS8 as a deterministic gate. (2) Real sockets: PacerTrace and TC-send, TC-recv, CloseOK and Enforced predicates gate every run; conditional-case conditions are functions of the complete trace; DiscriminationCheck is a conditional negative control; B_meas is measurement only.
  Rationale: Prediction from bytes alone is wrong (I44); pause-start values do not imply the result (I45). Mock-only tests miss Windows faults; ETW needs privileges; a model without next-member production leaves PS5c underived. Cost: A's accuracy against the kernel is supported only by “received ≤ A” and A matching size at sendDone.
- D94: Separate completion events: sendDone (server), the first A = P = size after finish, releases reservation, stops sendTime and starts IDLE. receiveDone (client), Rc = size, is invisible to the server and enters no deadline. All server deadlines measure through sendDone. TC-send checks only the server; TC-recv checks Rc ≤ A and receiveDone if the client continues; unreadAtSendDone is a measurement. Decisive vector: PS5c and RG14(0b).
  Rationale: Holding reservation until client reading makes it hostage to a nonreader. Application acknowledgments change standard JSON-RPC. Requiring Rc = A at sendDone rejects a conforming implementation (I46). Cost: server completion does not mean client receipt; this is disclosed.
- D95: Default-deny extension bridge authorization, with a second defense in the node. BridgeAuth runs in the worker before any request or local effect, using D96's unified envelope and Bpre then B0–B4. Explicit rpc_read matrix separate from light/heavy classification; eth_call/eth_estimateGas simulate with mandatory to, gas ≤ 30M and no state override; eth_getLogs exclusively through LogClient. wallet_req is limited to eth_accounts, eth_requestAccounts, eth_chainId and eth_sendTransaction. Prohibited in every kind: eth_sendRawTransaction, signing, wallet_*, filters, debug/txpool/admin/anvil/evm, eth_getProof and all pocol_*. Only write path: wallet_req, approval, Wallet, WalletSubmit with keccak256(raw) = approved txHash. RpcReadClient is the sole transport exported to Bridge and rechecks the allowlist; dependency-cruiser forbids Bridge importing WalletSubmit or HttpTransport. Node: ORIGIN_DENY = {pocol_submitShare, pocol_getTemplate} returns −32601 for any request carrying Origin. Verification: BR1–BR19, BR-neg and BR-dep.
  Rationale: A denylist leaves new methods allowed. Cost classification is not authorization. Origin alone cannot distinguish site from wallet at the node. A separate mining port isolates more strongly but doubles RpcGuard budgets, remaining a public-network option. A general schema library has a larger surface. Cost: reads expose public state; eth_call consumes RpcGuard quota; sites requiring eth_getProof or filters do not work in v1; parser correctness is supported by BR16, not assumed.
- D96: Unified envelope for all bridge messages: {id, kind, payload}, integer id in [0, 2^32−1], payload = {method, params}, params always an array. storage_set: site_storageSet [str(256), str(61440)|null], site_storageClear []. nav: site_navigate [pathStr], site_openExternal [httpsUrl] with confirmation. site_ is reserved for local methods and absent from rpc_read. Same check order for every kind. Disclosed local errors: −32005 {rate|pending}, 4300 {quota, limit}. SiteStorage derives its key from frame context and accounts quota literally: entry = utf8len(k)+utf8len(v); newTotal = total − old entry + new entry; admit at newTotal ≤ 1048576; deletion/clear not rejected. Atomic application with one chrome.storage.local.set or none. Verification: BR7a–BR7h, BR17 (including boundary table BR17e), BR18, bridge-fault:legacyLocal and quotaNoSubtract.
  Rationale: A structural exception for each local kind needs two parsers and error orders, causing I48. Removing kind loses the double (kind, method) check. Unprefixed local names can collide.

Quota-accounting alternatives:
(A) String.length (UTF-16 units): differs from actual stored bytes and is awkward to test in Python.
(B) Serialized dictionary JSON size: depends on key ordering and escaping, preventing literal accounting.
(C) chrome.storage.local.getBytesInUse: includes unspecified Chrome overhead and prevents deterministic unit tests.
Selected: UTF-8 bytes of key/value, deterministic and manually derivable as in BR17e.

Cost: nonstandard code 4300, localStorage values limited to 61440 bytes per key, actual disk quota differs due to Chrome overhead, and every admitted write rewrites the entire site dictionary.
- D97: Deterministic bridge rate/pending model:
- Stamp: arrivalMs = floor(performance.now()), set by the worker at receipt before checks; process in arrival order.
- Bucket: per site session (D99), integer arithmetic. tokens_mt ∈ [0, 50000], refill 50 per millisecond, cost 1000.
  - Size rejection consumes no token.
  - Every rate-admitted message consumes its token even if rejected later.
- Checks: BridgeAuth.check implements only Bpre–B3; destination components enforce B4 constraints.
- Per-session pendingReads:
  - Increment at admission.
  - Decrement at final response, timeout, or orphan settlement after frame teardown.
  - Navigation does not reset it.
  - One eth_getLogs message = one pending request.
- Verification:
  - BR19a/BR19b are deterministic units.
  - BR19c/BR16 against BridgeRef using recorded times.
  - BR20 for navigation.
  - Negative controls noRefill and frameBudget.
  Rationale: Alternatives:
(A) Expect a fixed count in Chrome: not implied by a refilling bucket (I49).
(B) Fixed window: permits double rate around window boundaries.
(C) Floating point: nondeterministic boundary results.
(D) Remove rate limiting: does not bound rejected-message parsing cost.
(E) Per-frame ownership: navigation renews it (I52), so ownership moved to the session.

Disclosed cost:
- BR19c proves implementation consistency with its own arrival times, not their accuracy.
- BR19c may remain “inconclusive” on a slow device.
- −32603 {transport} is a local code.
- D98: Extension RecvGuard: all network reception passes only through HttpTransport.
- Stream reads; count after decompression and before append; cancel immediately above the method's RECV_LIMIT. Reject oversized Content-Length before reading.
- Iterative parsing at depth ≤ 16, rejecting invalid UTF-8 and duplicate keys.
- Errors have the same bounds: truncated message, data ≤ 4 KiB.
- Advance reservation from separate pools: SITE_POOL = 32 MiB, SESSION_RECV_MAX = 10 MiB, protected TRUST_POOL = 2 MiB, CONTENT_POOL, RECV_INFLIGHT_MAX = 48. Unavailable within 2s gives busy.
- Never adopt partial content. Verification: MR1–MR12 with recvNoLimit control.
  Rationale: Alternatives:
(A) Accumulate with text()/json(), then check: exhausts memory before checking (I51).
(B) Trust Content-Length: may be absent or false with chunking/compression.
(C) Isolate each request in an offscreen document: MV3 permits one document, costs more and does not bound total memory.
(D) One global bound without per-method limits: lets a false light response occupy the budget.

Selected: streaming accounting, per-method bounds and separate pools; deterministic and boundary-testable (MR6). A separate trust pool prevents site starvation of network checks.

Cost:
- eth_call above 1 MiB does not work for sites.
- Temporary parsing memory is measured, not proven.
- Chrome enforces header limits.
- D99: SiteSession keyed by (tabId, netKey, siteAddress):
- Created by user action; ends at tab close or navigation to another address.
- Owns message bucket, pendingReads, navigation bucket (capacity 3, one per 2s), receive reservation, ChunkCache (≤ 4 MiB+64 KiB) and one LoadJob.
- site_navigate creates a frame joining the same session without renewal.
- Torn-down-frame requests become orphans: cancelled without reply, counted until settlement.
- Global bounds: 16 pending reads, 16 MiB chunk cache.
- Verification: BR20a–BR20d with frameBudget control.
  Rationale: Alternatives:
(A) Per-frame ownership: self-navigation renews it (I52).
(B) Wait for cancellation completion before the new frame: slows honest navigation and does not prevent bucket renewal.
(C) Prohibit site_navigate and full reload: breaks multipage sites.
(D) Site-wide budget across tabs: lets one tab block another for the same site.

Selected: (E) session-level inheritance with orphan accounting. Gains B's benefit without blocking the UI; cumulative limits do not depend on navigation count.

Cost:
- 0.5 navigation/s after three consecutive navigations may slow fast-clicking users, recorded in T12.
- User-created tabs multiply quota up to the global bound.
- D100: Two extension receive-bound classes:
- exact: RECV_LIMIT ≥ WorstLegit derived from CP and actual client parameters.
- capped: closed list {eth_call at 1 MiB, eth_estimateGas at 64 KiB}, with disclosed product limits narrower than the node.

Associated changes:
- BG1 checks both classes instead of RpcGuard's general bound.
- RecvFit rejects profiles whose legitimate responses exceed client bounds.
- eth_getBlockBy* bound increases to 164 KiB.
- Bridge eth_feeHistory schema restricts n ≤ 128, p ≤ 16.
- RECV_INFLIGHT_MAX excludes the trust pool.
  Rationale: Alternatives:
(A) Raise all client limits to server limits (8 MiB for heavy methods): the site pool then fits only four responses, weakening D98.
(B) Keep old BG1: contradicts the selected eth_call limit, preventing any build.
(C) One general bound: fails to distinguish compatibility guarantees from product constraints.

The choice makes each method either derived and boundary-tested or explicitly capped and recorded in T12. Derivation exposed the 160 KiB error.

Cost:
- eth_call above 1 MiB, large eth_estimateGas reverts and feeHistory above 128 blocks do not work for sites.
- Bounds assume compact JSON.
- D101: Bounded StoreQueue for SiteStorage writes; qbytes = 64+utf8len(k)+utf8len(v):
- Limits: per session ≤ 8 messages and ≤ 262144 bytes; extension-wide ≤ 64 messages, ≤ 4 MiB, ≤ 2 active writes. Counters include every unreleased message.
- Admit/reject (−32005 {store}) before append.
- Order: FIFO per storage key, one active write per key.
- Evaluate quota when the message becomes its key's head, without a slot (D102).
- Timeouts: 10s queue wait, 5s write. Write timeout gives storeTimeout; block key until the Chrome promise settles, then reread state.
- Teardown/session end: cancel waiting messages without replies, finish active writes and wait ≤ 5s for active writes before snapshot.
- Loader: enforce quota locally, coalesce writes per key, bound unanswered sends to 8 messages/256 KiB, and send writes before navigation.
- Verification: BR21a–BR21f with storeUnbounded and quotaNoRelease controls.
  Rationale: Alternatives:
(A) Queue depth 1: rejects legitimate consecutive writes.
(B) Coalesce last values in the worker: changes per-message response semantics.
(C) Count limit only: does not bound memory.
(D) Cancel active writes on teardown: impossible with chrome.storage.

Selected: count/byte-bounded queue rejecting before append, with loader coalescing. Bounds memory, preserves order and atomicity, and is testable with mock clock/storage.

The global byte limit is defensively redundant under defaults (I55), tested in BR21e.

Cost:
- Sites exceeding limits or navigating without waiting lose pending writes.
- A hung Chrome write holds its slot until settlement.
- D102: Separate response from StoreQueue resource ownership:
- Two states per admitted message:
  - replyState ∈ {none, sent, suppressed}.
  - ownState ∈ {queued, ready, active, released}.
- Reply at most once; it releases nothing.
- Only release subtracts qbytes/count from session/global counters and frees active slot/key lock if owned. Execute exactly once.
- Transitions:
  - queued → ready or released on head evaluation; 4300 rejection without calling Chrome.
  - ready → active on taking a global slot and starting set.
  - queued/ready → released on wait expiry or cancellation.
  - active → released only on chrome.storage promise settlement, regardless of prior storeTimeout or teardown. Post-response settlement reloads without another response.
- Verification: BR21a(q8), BR21b(r6), BR21f, (f2), with storeEarlyRelease and quotaNoRelease controls.
  Rationale: Write-timeout alternatives:
(A) Release on timeout response: admits beyond bounds and double-subtracts (I54).
(B) Separate hung-write counter: creates parallel semantics.
(C) Withhold timeout response until settlement: leaves the site unanswered.

Quota-rejection alternatives (I56):
(D) Evaluate after taking a slot; define active release without a promise: rejection waits behind other sites' hung writes and adds a second active-release event.
(E) Evaluate at admission: earlier queued writes may change total.

Selected: evaluate at key head, where total is confirmed and cannot change before start. Rejection is immediate and slot-independent; onSettle remains the sole active-release event; counters literally equal held resources.

Cost: a hung Chrome write holds the session quota and slot until settlement.
- D103: Local-storage recovery across MV3 worker generations:
- Confirmed generation fence before any data write.
- Immutable records versioned (epoch, seq).
- Per-key checkpoint recovery before any snapshot or head evaluation.
- Worker loss drops memory state; viewer tears frames down with notice, without automatic reload.
- D101/D102 apply within one generation.
- D104 governs seq allocation, concurrent recovery and disk bounds.
  Rationale: Compared with a persistent offscreen owner, boot-time dictionary rereading, IndexedDB transactions and reconnecting a live frame with snapshot reconciliation. The choice assumes no ordering across generations and requires only single-item set atomicity.

Cost:
- JavaScript state and unsent writes are lost with notice.
- One checkpoint write per key per generation.
- D104 disk bounds do not follow from D103 alone.
- D104: Single recovery, unified allocation and a binding local-storage disk budget:
- Recovery: one RecoveryGate per key/generation, shared by every snapshot and storage_set.
- Allocation: one SeqAlloc assigns a number at every set issuance, whether checkpoint, retry or write. Never reuse numbers; exhaustion gives storeRecovery.
- Two checkpoint attempts using RecoverySlots (D109), then failed, or unrecovered/busy if the first slot is unavailable.
- Encoding: fmt 2 with base64 and SITE_ENTRIES_MAX = 4096, so RECORD_BYTES_MAX = 1442048.
- Admission: each set uses getBytesInUse plus unsettled operations, LATE_RESERVE and META_RESERVE within 256 MiB, ≤ 1024 names, ≤ 64 sites.
- Generation window: 4 generations within 60s.
- Sweep: never delete the greatest record; failed deletions remain counted.
- Entire-site deletion: user action through a tombstone only.
- Enumeration: getKeys mandatory; no get(null).
- Verification: BR22d, BR22e, BR22f and BR23.
  Rationale: I57 — alternatives:
- Per-tab lock: issues two checkpoints with the same name.
- Optimistic recovery with reconciliation: exposes a snapshot before resolution.
- Derive write seq from checkpoint number: collides with retries.

I58 — alternatives:
- Unbounded retention: violates the advertised bound.
- Automatic eviction: silently deletes confirmed state.
- get(null): unbounded memory.
- Assume successful deletion: does not hold on failure.

The choice makes record names unique by construction, preserves every confirmed write by the survival lemma and turns failed sweeping into write rejection rather than growth. Moving checkpoints to RecoverySlots addresses I62 (D109).

Cost:
- Disk bound conditional on A15c.
- Limits of 4096 pairs and 64 sites.
- Up to 60s waiting after repeated generation loss.
- Requires getKeys availability.
- D105: Local-storage DiskLedger:
- Admission/reservation is one synchronous step, covering bytes, names and new-site slot, returning an operation-owned ticket.
- Ticket remains counted after settle until a refresh requested afterward completes; remove it while adopting that measurement.
- Serialized refresh.
- Subtract deletion only through measurement.
- Verification: BR23(d8–d11) and diskNoReserve control.
  Rationale: Alternatives:
(A) Async lock around checking: serializes all writes behind slow measurement.
(B) Check without reservation, then recheck at issuance: retains the same gap.
(C) Drop reservation at settle: creates a gap before measurement.

The choice makes the bound a construction lemma in a single-threaded worker.

Cost:
- Temporary usage overestimation.
- A never-settling operation ticket remains until worker loss.
- D106: Per-key serialized AdminDelete:
1. deleting; block new sessions.
2. Close every key session; cancel waiting messages.
3. Wait for RecoveryGate resolution (≤ 25s including reads, D109/D110).
4. Tombstone with a SeqAlloc version on an AdminSlots slot (D108); no other allocation until resolution.

Two tombstone attempts, then unrecovered with notice of failure, uncertainty or adminBusy. Total time ≤ 45s. Recreate through a new session. LATE_NAMES = 28 (D109).
  Rationale: Alternatives:
(A) Ordinary tombstone in key FIFO: waits for a write that may never settle; surviving sessions allow old-snapshot writes after it.
(B) Direct record deletion: violates the survival lemma.
(C) Retain sessions while invalidating their snapshots: needs reconciliation.

The choice makes the tombstone the highest version by construction. Tombstone-slot ownership is in D108; gate-resolution bound in D109/D110.

Cost: site tabs are torn down and their unsent writes lost.
- D107: Separate apply and settle during recovery:
- Resolution depends only on settle.
- First success immediately makes the key ready; confirmed = greatest successfully settled version.
- failed resolves after every attempt fails or times out and remains terminal for the generation.
- BR22d variants V-a..V-e, with readyOnApply control.
  Rationale: Alternatives:
(A) ready at apply: the worker does not necessarily observe it, and it may come from a dead generation.
(B) Fixed readiness time: contradicts first-success semantics, causing I60.

The choice makes every variant derivable.
- D108: AdminSlots for tombstone-slot ownership:
- Two extension-wide slots outside STORE_ACTIVE_GLOBAL.
- Acquire only at tombstone set issuance through AdminFIFO, waiting ≤ 5s.
- Release only at settle; neither timeout nor resolution releases it.
- No tombstone without a slot.
- No first-attempt slot gives adminBusy without a tombstone. No second-attempt slot gives uncertain or failed.
- Bound lemma: ≤ 2 unsettled tombstones per generation.
- Current derivations:
  - AdminSlots contributes GEN_WINDOW_MAX·ADMIN_SLOTS = 8 to LATE_NAMES, out of total 28 (D109).
  - META_RESERVE constraint with TOMB_BYTES_MAX = 512.
  - BG1 gates.
  - Total AdminDelete bound 45s (D110).
  - Earlier values 20 and 30s are historical and superseded.
- Verification: BR24(x9–x11), adminEarlyRelease control.
  Rationale: Alternatives:
(A) One slot held until settle: prevents retry while the first is pending, breaking the resolution bound.
(B) Release on timeout: unbounded unsettled tombstones invalidate reserves (I61).
(C) Unlimited slots with a counter: no fixed dead-generation bound.
(D) Per-key slot: bound scales with site count.

The choice makes the bound a construction lemma and permits one retry. Tombstone phase alone ≤ 20s, plus gate resolution ≤ 25s.

Cost: two pending tombstones prevent further deletion until settlement or worker loss (adminBusy).
- D109: RecoverySlots for checkpoint ownership:
- Two extension-wide slots, separate from STORE_ACTIVE_GLOBAL, AdminSlots and ReadSlots (D110).
- Acquire only at issuance through RecoveryFIFO, waiting ≤ 5s from request, in the same synchronous step as DiskLedger.reserve and SeqAlloc.alloc.
- Release only at settle or disk rejection before issuance; neither timeout nor resolution releases it.
- No checkpoint without a slot.
- No first slot: unrecovered/busy without issuance. No second slot: treat as its timeout, making key failed under D107. Becoming ready withdraws the pending slot request.
- Lemmas:
  - ≤ 2 unsettled checkpoints per generation.
  - Checkpoint phase ≤ 20s from t0; total resolution ≤ 25s from t_s including reads (D110).
- Derivations: LATE_RESERVE = 23072768, LATE_NAMES = 28, AdminDelete bound 45s, BG1 gates.
- Verification: BR22f, recoveryNoSlot control.
  Rationale: Alternatives:
(A) Checkpoints on the two global slots with unlimited wait: two pending attempts for different keys prevent resolution and block all sites' writes (I62).
(B) Issue a second attempt without a slot: breaks the late-operation bound.
(C) Release on timeout: unbounded unsettled operations.
(D) Per-key slot: bound scales with site count.

The choice makes both bound and resolution construction lemmas without assuming promise settlement, following D108.

Cost:
- Two pending checkpoints prevent recovery of other keys until settlement or worker loss.
- Disk reserve nearly doubles (approximately 22 MiB).
- D110: Bounded RecoveryGate read phase:
- t_s is unrecovered → recovering time. Create gateId and start total RECOVERY_READ_DEADLINE = 5s.
- Deadline includes read-slot waits, getKeys, every get, decoding and validation.
- ReadSlots: two extension-wide slots, released only by read-promise settlement.
- RECOVERY_GETS_MAX = 4. Corrupt records with no valid record within that limit give failed {corrupt}.
- Deadline expiry gives unrecovered/readTimeout without writing or reservation.
- Ignore reads tied to a noncurrent gateId, preventing post-resolution checkpoints.
- Derived bounds: gate resolution ≤ 25s from t_s; AdminDelete ≤ 45s.
- Verification: BR22g, lateReadCheckpoint control.
  Rationale: Alternatives:
(A) Checkpoint-only deadline: hung reads prevent resolution (I63).
(B) Per-call deadline without a pending-read bound: hung promises accumulate with every new gate.
(C) Terminal failed on read timeout: disables the site for the generation due to a read that wrote nothing.
(D) Unbounded corrupt-record reads: no resolution time bound.

The choice makes resolution a construction lemma without assuming promise settlement. Reads do not change disk, so LATE_RESERVE is unaffected. Corruption failure is explicit rather than silently selecting an older record.

Cost: two pending reads deny recovery to other sites (readTimeout) until settlement or worker loss. Corruption in the greatest four records disables the site until administrative deletion.
- D111: Deletion/recovery operation identity at slot grant:
- Each AdminDelete has opId and state {running, settled}.
- Any resolution synchronously transitions to settled, withdraws all pending AdminFIFO requests for the operation and leaves deleting.
- Slot grant passes a guard in the same step as DiskLedger.reserve, SeqAlloc.alloc and issuance. Require an unwithdrawn request, op.state = running and matching deleting[key].opId.
- Failure immediately returns the slot without reserve, alloc or set; count adminStaleDropped.
- RecoverySlots uses the same guard with gateId.
- Verification: BR24(x12), x12b, BR22f-f5; adminNoCancel and adminStaleIssue controls.
  Rationale: Alternatives:
(A) Withdrawal alone: correctness depends on covering every resolution path.
(B) Guard alone: dead requests remain blocking FIFO and cause false adminBusy.
(C) Dedicated slot per operation until resolution: breaks ADMIN_SLOTS bound (I61).

The choice makes “no tombstone after resolution” a construction lemma in a single-threaded worker. Withdrawal preserves FIFO fairness; the guard does not depend on complete withdrawal.

Cost: extra per-operation state/ID and a strict implementation constraint against await between grant and issuance.
- D112: Site-slot accounting across worker generations:
- sitesLive counts distinct keys with any record in the last measurement, including {} and tombstones.
- Key-creating operations: data set and checkpoint for a recordless key; ≤ LATE_SITES_PER_GEN = 4 unsettled per generation.
- Dynamic reserve LATE_SITES(now) = 4·lateGens(now) ≤ 16; lateGens counts confirmed epoch items in the T_LATE window.
- Drop a generation's contribution only when a refresh requested after bootMs+T_LATE completes.
- Creating-operation admission requires sitesLive + R_sites + LATE_SITES + 1 ≤ 64; otherwise 4300 {sites}, with retryAfterMs when reserve causes rejection.
- Remove the last tombstone only without a session, gate or ticket, and after older generations leave the window.
- Explicit A15d assumption: a rejected promise never applies later.
- Verification: BR23(d12–d14), noLateSites control.
  Rationale: Alternatives:
(A) Fixed reserve 16: permanently reduces capacity to 48 sites.
(B) Block new sites for 60s after every boot: correct but disrupts opening new sites after repeated MV3 worker loss.
(C) Durable intent marker per new site: that marker can itself apply late, so reserve remains and an extra write/name is added.

The choice makes the bound a construction lemma under the same A15c/A15d assumptions underlying disk bounds. Impact is time-limited and near capacity. Dropping contributions through measurement rather than elapsed time prevents a gap between window exit and visibility of effects.

Cost:
- Temporary rejection of new sites for up to 60s at sitesLive ≥ 48 with repeated loss.
- lateGens depends on wall-clock bootMs.
- Exposes previously implicit A15d.
- D113: Site-slot accounting for counted keys:
- A retained tombstone is a record preserving the key's sitesLive slot. Operations on counted keys, including recreation after deletion, reserve no slot and are unaffected by LATE_SITES.
- Every non-remove ticket on a counted key pins it until dropped through a post-settle-requested measurement.
- Remove the key from sitesLive only through an adopted measurement not seeing it, with no pin.
- TombReaper removes the sole tombstone under D112 at every refresh/epoch-window exit. Opening a session before issuance prevents it.
- A new slot is required only after last-record removal and adoption of a measurement not seeing the key.
- Verification: BR23(d1, d9, d15), BR24(x6); seatOnCounted and dropPinned controls.
  Rationale: Alternatives:
(A) Always reserve a new slot on recreation: double-counts and rejects recreating K at 64 despite no increase in keys (I67).
(B) Release the slot at tombstone issuance: late writes restore the key uncounted.
(C) Block sessions only during tombstone removal: misses a dead generation's late remove deleting the tombstone before settlement of a slotless operation.

The choice aligns accounting with on-disk keys. pin closes the gap between slotless admission and effect visibility, following D105 tickets. TombReaper specifies who removes the last tombstone and when, previously unspecified.

Cost:
- A deleted site remains counted until tombstone removal, up to 60s after deletion if older generations remain in-window, delaying new-site admission at capacity.
- Per-key pin state.
- D114: BR23(d15c) tests pin in two phases:
- Within T_LATE: direct assertion after a measurement not seeing K requires sitesLive=64 and pinnedKeys=1. X's response at 30100 is literal per path:
  - Correct: 4300 {reason: 'sites', limit: 64}, no retryAfterMs, because sitesLive+R_sites alone prevent admission.
  - dropPinned: same code with retryAfterMs: 29900, because reserve alone prevents admission.
  - Same code, different data shape.
- After all lateGens contributions expire (LATE_SITES=0): an overrun vector keeps a live pending checkpoint carrying pin. Correct implementation rejects X; dropPinned admits it and reaches 65 after apply(C0).

Dead remove applies within T_LATE under A15c. DiskLedger.refreshNow() is a test-build-only hook and does not change decisions.
  Rationale: The first table did not distinguish implementations because LATE_SITES rejected X in both. The second expected retryAfterMs in the correct implementation, contrary to D112 (I68).

Alternatives:
(A) Direct assertion alone: detects the defect without demonstrating its effect on the 64 bound.
(B) Boot without a contribution: violates D112.
(C) Data ticket instead of checkpoint: requires ready; checkpoint success sweeps the tombstone.
(D) Always include retryAfterMs: changes a correct rule to simplify agreement.

The choice derives every cell, including error data shape, from existing D105/D107/D112/D113 rules.

Cost: K becomes failed in the test generation, a recorded side effect.
- D115: The VA annex in validation defines self-contained consensus, incentive and retention gates:
- Common framework: SimNet/NetSim, va/base profile, honest rates, adversarial β, measurement window, seeds, CI rule and source of truth.
- T7, T7b, T7c, T7e, T9, T10, T10b(i–vi), T19, T20, T23, T18b and T25, with scenarios, metrics, denominators and pass/fail/inconclusive criteria.
- Table mapping each conditional claim to its measuring test.
- Literal cases in VA-spec, reviewed before M5/M6/M8 code.
  Rationale: Alternatives:
(A) Define while writing M5 code: leaves acceptance gates undefined at approval (I69).
(B) Convert gates to measurements: lowers standards to simplify agreement.
(C) Complete literal definitions in the main specification: doubles its length.

The choice puts binding rules/criteria in the specification and literal scripts in an annex reviewed before code, following M1-spec/M7-spec.

Cost:
- Arms/seeds may exceed available runtime on 8 GB; schedule batches and disclose duration.
- Some items, such as T7e(b) and T10b(iii)(d), require an honest signed block in a specified window; otherwise record the case without a claim.
- D116: Exclusion effect defined through C_full and C_kept:
- g_dir = floor(R·c_w/max(C_kept, κm)) − floor(R·c_w/max(C_full, κm)).
- Zero at C_full ≤ κm; above it, bound strictly below R·c_w·(1/κm−1/C_full)+1.
- Indirect Draw/lastShare effects disclosed without a bound; T7c(S) measures them with a 2% threshold.
- Vectors SV10a–c.
  Rationale: The earlier C_incl formula incorrectly gave zero (I70).

Alternatives:
(A) Redefine C_incl as the full count: leaves the bound ambiguous.
(B) Measure only: hides a provable arithmetic defect.

The choice follows directly from payInt (D65) and is tested across the threshold.
- D117: VA termination framework:
- T_RUN_MAX.
- Quiescence height triggers the measurement barrier (D122), replacing the fixed 300s window.
- Stalled seeds and zero denominators take the worst value and are not dropped.
- Excitation quorum 20/24.
- Convergence: G through HeadCheck after the barrier; temporal S (t_conv) before it.
- Literal halves 23/23.
- Explicit AdvOracle in T10b(iii); production during withholding is an S criterion.
- Arithmetic definitions of T4-H/W/L and T8.
  Rationale: Alternatives:
(A) Drop stalled seeds: silent bias.
(B) G deadline for a probabilistic event: false failure.
(C) Wait for the adversary to find a nonce using its own power: excitation may not occur.

The choice separates rule correctness (G) from probability (S) without lowering criteria. A 300s window with continuing mining was removed because tip equality remains probabilistic (I69), replaced by D122.
- D118: Side-block policy: at each height retain the canonical block and one noncanonical block with the greatest cumulative work for its branch, then earliest-seen. Delete the loser with descendants; refetch through sync if it later becomes heavier. Local retention, not validity.
  Rationale: Alternatives:
(A) Earliest-seen only: adversary can occupy the slot with a weak branch.
(B) Delete all noncanonical blocks: slows honest reorganization.

The choice aligns with fork choice and bounds memory.
- D119: Local ReplicaMonitor in pocol_getRetention: replicas listed in node configuration, checked every 10s. A replica is live if it responds within 2s, complete=true and head ≥ local head−6. The extension warns at replicasLive < 2.
  Rationale: Alternatives:
(A) Retention field in Status: changes P2P messages.
(B) Manual declaration: not a measurement.

The choice is outside consensus and testable in T25. Trusted only in LN.
- D120: Exclusion share sets:
- ν (winning nonce) is always an implicit share outside shareList, counted in C and c_w.
- E_full: received explicit shares, trimmed to S_max smallest nonces excluding ν; C_full ≤ 257.
- E_kept: shareList contents.
- General g_dir = floor(R·c_w^kept/max(C_kept, κm)) − floor(R·c_w^full/max(C_full, κm)).
- D116 bound applies only to ownSharesOnly with fixed c_w. emptyShares is computed exactly and can be negative.
- Vectors SV10d–f; T7c(G) does not require observing C_full > κm in SimNet.
  Rationale: Alternatives:
(A) Include ν in shareList: changes item 9, encoding and S_max.
(B) Exclude ν from C: contradicts SV1 and existing payInt.
(C) One bound for all arms: wrong when c_w differs (I71).

The choice preserves consensus rules and derives every arm from payInt (D65).
- D121: VA statistical rules:
- Every probabilistic deadline is S; post-quiescence equality is G, with an explicit classification table.
- CEN clips each seed metric to a finite range before aggregation and computes paired metrics within each seed.
- Estimator is the mean; CI uses paired percentile bootstrap (10^4, numpy type 7, PCG64 seeded from SHA256).
- Passing requires ≤ 2/24 or ≤ 4/48 clipped seeds.
- Stalled G seeds count; exceeding the same quorum fails.
- T4-L becomes S.
- underpowered only with an explicit event threshold; removed from T7c.
  Rationale: Alternatives:
(A) +∞ with median: resolves ordering, not paired differences.
(B) Drop stalled seeds: silent bias.
(C) G bounds for probabilistic events: false failures and conflict with D117 (I69).

The choice is literally reproducible and treats stalls as worst cases, not undefined cases.
- D122: VA measurement barrier:
- halt: disconnect adversary, stop faults, heal partition.
- stopMining for every honest node: fixture command, not protocol rule.
- Drain to Quiet: 2s without transfers or pending requests between honest nodes, DRAIN_MAX = 900s; exceeding it fails G.
- All HeadCheck is G:
  - Q1: equal cumulative work at honest tips.
  - Q2: differing tips are legitimate ties after signed preference.
  - Q3: equal stateRoot, receiptsRoot and gasUsed for every shared block at every instant.
  - Q4: state matches RefReplay of the tip branch.
  - Q5: rollback ≤ R_keep.
- Record ties as tie, not violations.
- Temporal S convergence is measured before the barrier; a tie is not convergence.
- QV1–QV6 with three negative controls.
  Rationale: Alternatives:
(A) Fixed window with continuing mining: an in-flight block or tie causes false failure (I69).
(B) Make all tip equality S: loses deterministic detection of fork-choice/execution errors.
(C) Stop mining but require one tip: rejects a legitimate tie resolved locally by “first seen” [PDF:74].
(D) Wait for a tie-breaking block: reintroduces PoW inside a deadline.

The choice makes each G item a deterministic function of fixed post-Quiet state. Quiet is deterministically reachable in an honest network without production/faults. Probability remains entirely in S.

Cost:
- stopMining is not real operation; HeadCheck establishes rule correctness, not convergence speed.
- Legitimate ties mean honest nodes may retain different tips until the next block, a disclosed protocol behavior.
- D123: Measurement-barrier quiescence includes every deferred action capable of changing the chain:
- ChainWork: future queue, unknownParent with known parent, running validation/import/reorg, sync/fetch timers and Status advertising an unfetched heavier tip.
- Mandatory registration through ChainTimer, with a clippy rule.
- statusSweep at barrier and after every tip change.
- Future-queue policy:
  - F1: wait for anything imported by an honest node, eligible for all within Φ under A3.
  - F2: wait for anything eligible within 2Φ.
  - F3: purgeFuture otherwise, a disclosed fixture command with futurePurged measurement.
- Quiescence lemma conditional on complete registration; PostQuiet G gate; QUIET_HOLD = 3Φ+60s.
- QV7–QV10 on qv/net, with three negative controls.
- Replaces D122's Quiet definition and instant-independence statement; retains the rest of D122.
  Rationale: Alternatives:
(A) Wait for every future entry: unbounded, as large-a fallback has distant ts = L(a).
(B) Drop the whole queue: drops something another honest node imported, recreating Q1 failure.
(C) Change the protocol queue rule: changes validity to simplify tests.
(D) Manually enumerate queues without timer registration: cannot detect a forgotten timer.

The choice makes quiescence deterministic and time-bounded (future queue ≤ t_halt+3Φ), preserving every entry that can reach an honest tip to avoid false failure. Negative controls and PostQuiet can detect missing registration.

Cost:
- purgeFuture removes headers the real protocol would later import. Distant timestamps remain a T10c measurement, not HeadCheck.
- The lemma depends on complete ChainTimer registration.
- D124: Resolve unavailable bodies and rejected entries at the measurement barrier:
- Drop undelivered adversary messages; restart stopped honest nodes without wiping.
- Possession: a root-matching body in honest storage, durable or pending.
- t_avail ≤ t_halt+max(Δ, RESTART_MAX), with a possession-closure lemma afterward.
- Path U (fixture command): at t_avail purgeUnavailable removes every entry whose body no honest node holds, together with descendants, requests and timers.
- Path A: wait for entries held by honest nodes; failure to import is a G sync failure.
- Path V (node implementation behavior): rejection cancels the entry, descendants and timers in the same step. NotFound immediately moves the request to another peer.
- Correct F1/F2 to eligibility, not import, bounds.
- DRAIN_MAX(run) = max(Δ, RESTART_MAX)+3Φ+SYNC_RETRY_MAX+N_miss·c_b+2s; SYNC_RETRY_MAX enters once. DRAIN_CAP = 3600s; exceeding it makes the seed unevaluable under D121's quorum.
- QV11–QV14; keepUnavailable, purgeHeld, skipHolder and noCancelOnReject negative controls.
- Replaces D123's termination statement and D122's fixed DRAIN_MAX.
  Rationale: Alternatives:
(A) Wait for a fixed ceiling, then “inconclusive”: confuses legitimate withholding with sync defects.
(B) Remove everything not imported by its holder: drops data retrievable from another honest node, falsely failing Q1.
(C) A protocol fetch timeout deleting headers: changes node behavior to simplify tests and may discard temporary withholding that the adversary later reveals.
(D) Fixed drain time: does not follow after long partitions because missing-block count has no prior bound.

The choice makes removal deterministic from fixed post-t_avail possession, preserving everything recoverable. The bound derives from measured state; sync faults remain detectable (skipHolder).

Cost:
- Path U is not a real network where bodies may appear later.
- The sequential bound is conservative and potentially large.
- D125: Complete measurement-barrier resolution:
- (1) BodyMismatch: a non-root-matching body is a response error; drop it, penalize its source and refetch. Path V only covers H-pre rejection or H-full rejection on a matching body.
- (2) Rec: recoverable-dependency closure at t_avail. U removes entries outside it as bodyUnavailable or depUnavailable; fixture exclusions do not enter pocol_getRejects.
- (3) DrainPin: a SimNet fixture command prevents evicting any Rec header/body from t_halt through Quiet, except V/F3. Possession lemma replaces stability lemma.
- (4) Bound: Need(i) includes each honest node's missing history. DRAIN_MAX sums fetching, headers, rollback and TrieRebuild. drainBound literally matches HeadCheckRef; DRAIN_CAP does not apply to QV. Rollback beyond R_keep reports Q5 with deepReorgRequired.
- QV15–QV18c; rejectOnBadBody, excludeDepAsInvalid, noDrainPin, drainUndercount and silentDeepReorg negative controls.
  Rationale: Alternatives:
(A) Classify every ambiguous case “unevaluable”: hides sync faults.
(B) Permanent protocol retention: changes D118/memory to simplify tests.
(C) Large fixed bound: underived and hides undercounting.
(D) Treat BodyMismatch as invalidity: lets a malicious peer discard a valid block and contradicts N5.

The choice makes every removal depend on a preserved reference set, separates response errors from invalid blocks, and derives a bound matched against an independent reference so the ceiling cannot conceal undercounting.

Cost: DrainPin/U are synthetic, not real eviction under memory pressure; the sequential bound is conservative.
- D126: Complete the measurement barrier:
- (1) Two-phase protection:
  - Atomic HaltPin at t_halt pins every header/body known or held by an honest node, then adds late honest arrivals and restart data. Bounds do not evict pinned data; above-bound arrivals are admitted and counted.
  - narrow at t_avail releases entries outside Rec in the same U step.
  - pinBreach (G), computed from NetSim trace: a block removed by U as bodyUnavailable despite a holder at the barrier.
- (2) Need(i) = Rec \ imported_i without a work condition, with closure ensuring every post-t_avail import belongs to Rec.
- (3) Bound:
  - T_i = |Need(i)| bounds transitions by the lemma that every tip change occurs on import.
  - d*_i = min(R_keep, Hmax−Lmin) per rollback, with replay.
  - HR includes Kmiss, discovery starts S_i and H-pre cost.
  - Formula-independent BudgetCheck compares each activity class's count/duration against its bound.
- QV19–QV21; pinLate, needHeadsOnly, rbPerBranch, hdrUndercount controls; correction of QV17 noDrainPin control.
- Replaces D125's Need/HR/RB definitions, possession lemma and DrainPin activation; retains the rest of D125.
  Rationale: Alternatives:
(A) Compute Rec at t_halt: misses late honest messages and restart-loaded data.
(B) Permanent protocol protection: changes D118 to simplify tests.
(C) Count distinct branches with a constant factor: does not bound sequential adoptions.
(D) Match two references using the same formula only: does not prove completeness.

The choice:
- Makes protection active before every post-barrier event.
- Makes concealed eviction detectable by an independent predicate.
- Builds bounds on closed sets and proven transition counts.
- Adds independent BudgetCheck, failing when a class is omitted.

Cost: much looser bound, including all Rec per node and d*_i per transition, approaching DRAIN_CAP in long partitions. Above-limit admission during draining does not model real memory pressure.
- D127: Complete drain-bound accounting and test discrimination:
- (1) Kpend_i: known unchecked headers at t_avail, separate from Kmiss_i, included in HR_i.
- (2) HdrCache node behavior prevents repeated H-pre for the same header.
- (3) BudgetCheck table: each sequential class has a count, duration and one DRAIN_MAX component. Parallel waits (restart, eligibilityWait, preHaltRetry) are measured as intervals against a single fixed bound, not summed.
- (4) One SimParams record supplies test overrides to simulator, formula and reference.
- (5) needHeadsOnly, rbPerBranch, hdrUndercount, kpendIgnored and noHdrCache controls fail deterministic BudgetCheck predicates, not Quiet overruns.
- (6) QV19–QV21 derived with signed timestamps and explicit targets, equal-work-per-height lemma, and new QV22.
- Replaces D126's HR formula and BudgetCheck criterion wording; retains the rest of D126.
  Rationale: Alternatives:
(A) Keep controls based on Quiet overrun: not implied by a conservative bound; defects may pass.
(B) Sum all class durations: incorrectly compares parallel waits to one bound.
(C) Remove c_h: conceals real cost.
(D) Inflate RTT outside the definition: makes reference and simulator inconsistent.

The choice maps every bound component to a countable row and makes each negative control fail a numerically derived predicate, without changing validity.

Cost: HdrCache needs memory bounded by sync queues. Vectors need offset clocks and greater fixture-mining cost in QV21.
- D128: Enforce drain-fetch bounds and rederive vectors:
- (1) FetchSched is node behavior, not validity:
  - DiscRun: one discovery run plus dirty flag; runs R_i ≤ 1+S_i.
  - tried per run: productive requests reveal a new hash; unproductive ones exclude the peer.
  - BodySched: one in-flight body request per hash and triedPeers; every (hash, peer) pair gets ≤ one request.
  - HfullDone: do not repeat H-full for a resolved block.
- (2) HR_i = (|Kmiss_i| + (1+S_i)·(|H|−1))·RTT_halt + (|Kmiss_i|+|Kpend_i|)·c_h, a proven conservative bound.
- (3) H-pre has Hs/Hp stages. HdrCache stores only completed results, never unknownParent/notEvaluable, with inFlightHdr.
- (4) QV20 uses two isolated honest holders without violating D118; derived negative predicate (b = 0, transitions ≥ 1), no fixed timing table.
- (5) QV21/QV22 values come from the trace, with derived lower bounds and FixtureSetupError. QV22 has honest holder N15.
- (6) QV22b; noFetchDedup and cacheNotEvaluable controls.
- Replaces D127's HR formula, hdrFetch row and HdrCache definition; retains the rest of D127.
  Rationale: Alternatives:
(A) Bound ⌈Kmiss/512⌉ with a standard locator: overlapping peer responses invalidate it.
(B) Fixed fetch ordering to force timed multiple transitions: fragile, not implied by protocol rules.
(C) Pre-barrier fixture exception to D118: changes retention to simplify tests.
(D) Retain AdvPeer as QV22 body source: its messages drop at the barrier, turning the vector into a U-path test.

The choice proves each numeric bound from explicit scheduling rules, makes vectors reach their states using honest sources without retention violations, and makes every negative control fail a measured-value-derived predicate.

Cost:
- Looser bound: productive requests counted per header rather than batch.
- Deterministic demonstration of multiple transitions is no longer available; their count is measured.
- D129: Separate header copy from block, and restore fetch-peer eligibility.

(1) Violation classes:
- envReject: depends only on sig/winnerSig bytes, namely items 1, 3, 8 and the signature part of 13. Reject hdrId, not blockHash: penalize sender without block-reject record or descendant cancellation.
- blockReject: every other violation; reject blockHash and descendants.

(2) Caching:
- BlockCache keyed by blockHash, EnvCache by hdrId, inFlightHdr by both.
- Store and serve the first envelope-valid copy.

(3) Drain bound:
- HR_i += Venv_i·(c_env+RTT_halt), where Venv_i is copies measured at t_avail.
- Lemma: no new copy arises after t_avail.

(4) Reeligibility:
- Reset triedPeers on Status, block/descendant announcement or exponential backoff up to 60s, preferring untried peers.
- statusSweep at t_avail in the atomic U step. Lemma: afterward each (hash, peer) pair ≤ 1 request, zero backoff waits.

(5) Vectors/controls:
- QV23, T23 N9, corrections to QV18/QV22b.
- cacheByBlockHash, mergeInFlight, skipHp and noReeligible.

Replaces D128's HdrCache definition and triedPeers rule; retains the rest of D128.
  Rationale: Cache-key alternatives:
(A) Include both signatures in blockHash: changes consensus encoding. RFC6979 is unverifiable, so a signer can make different blockHashes for identical content, fragmenting descendants.
(B) Full-byte key only: repeats content checks for every adversarial copy.
(C) Cache no signature verdict: repeats recovery on every duplicate arrival.

triedPeers alternatives:
(D) Permanent exclusion: prevents recovery from temporary NotFound.
(E) Periodically reset everyone: unbounded load.

The choice preserves validity rules, prevents a malformed-copy verdict from invalidating a valid block, and retains provable, BudgetCheck-countable drain bounds.

Cost:
- A second cache key's memory.
- A connected adversary forces an envelope check per copy, bounded by banning after three violations and the peer limit.
- Venv bound depends on measured counts.
- D130: Complete D129 fetching and envelope handling.

(1) Fetching:
- Reeligibility (Status, announcement or backoff) adds the peer to reeligible without removing it from the cycle's tried set.
- Strict selection: untried in ID order, then reeligible.
- cycleReset starts a new cycle; in SimNet, atomic at t_avail for every hash.
- Lemma: afterward every pair ≤ one request; zero reeligible requests.

(2) Envelope:
- Two stages: stateless envPre, and parent-state envOwn without execution, with final verdict per hdrId. Before parent state: envPending.
- c_env = c_rec + c_own.
- Execute once per blockHash after first envOk copy.
- Path V, RejectPath.cancel, HfullDone and block-rejection records apply only to blockReject or import.

(3) Vectors/controls:
- QV23d, QV24.
- statusResetsTried, envRejectCancels, hfullDoneOnEnv, execPerCopy.

Replaces D129's triedPeers removal rule; retains the rest of D129.
  Rationale: Fetch alternatives:
(A) Bound all reeligibilities: Status message count has no prior bound.
(B) Remove reeligibility: breaks live recovery (N9).
(C) Prefer evidenced eligible peers over untried ones: sometimes faster, but recreates repeated requests.

Envelope alternatives:
(D) Move ownership checks to H-pre: requires parent state.
(E) Execute each copy: apply cost per adversarial copy.
(F) Cache envOwn by blockHash: reintroduces poisoning.

The choice makes the bound a construction lemma without restricting the live network; adversarial copies incur only nonexecution checks; validity rules remain unchanged.

Cost:
- A newly holding peer is not queried until untried peers are exhausted, potentially delaying fetch.
- cycleReset is a SimNet fixture command.
- Per-parent c_own cost/state.
- D131: Complete D129/D130 RTT accounting and envelope fetching.

(1) RTT:
- RTT(i,j) is a per-link upper bound in SimParams, default RTT_halt.
- RTT_i = max_j RTT(i,j) is the sole value used in HR_i, c_b(i) and BudgetCheck bounds.
- Per-request G predicate: measured duration ≤ RTT(i,j).

(2) Fetching:
- New P2P request GetEnvelope [blockHash, exclude ≤ 4] returns only the stored valid copy.
- EnvFetch state machine with strict BodySched scheduling.
- ENV_PENDING_MAX = 4 per block.
- Live envWait without a blockHash verdict.

(3) Rec:
- Requires validEnv, computed by HeadCheckRef.
- U adds envUnavailable, a fixture exclusion, not blockReject.

(4) Bound:
- HR_i = (|Kmiss_i|+(1+S_i)·(|H|−1))·RTT_i + (|Kmiss_i|+|Kpend_i|)·c_h + Venv_i·c_env + |Henv_i|·(|H|−1)·RTT_i, by a fetch lemma.

(5) Vectors/controls:
- QV25a, QV25c, QV25c-dup, T23 N10.
- globalRtt, envAsBlockReject, noEnvUnavailable, noEnvFetch.

Replaces HR_i/c_b formulas and envRefetch ≤ Venv; retains the rest of D129/D130.
  Rationale: RTT alternatives:
(A) Global maximum-link RTT: correct but inflates every bound.
(B) Sum actual request RTTs in the formula: makes the bound depend on the run's own trace.
Selected: a prior per-node bound, with a per-request predicate detecting overruns.

Envelope alternatives:
(C) Rely on GetHeaders: cannot reach side blocks, as in QV25c.
(D) blockReject after exhausting copies: permits poisoning a valid block whose envelope appears later.
(E) Keep an envelope-less block in Rec: prevents Quiet, causing I72.
Selected: preserves validity, separates missing envelope from invalid content, retains live recovery via envWait, and derives bounds from explicit scheduling.

Cost:
- Another P2P message type.
- envWait occupies sync queues up to their bounds.
- envUnavailable is synthetic SimNet exclusion; the live network only waits.
- D132: Unify envelope exclusion in verification rules:
- (1) Closed U-reason set: bodyUnavailable, envUnavailable, depUnavailable (with missingAncestor). Same set in common QV11–QV25 criterion, bodyPurged, SimNet.recSet, HeadCheckRef, BudgetCheck and VA-spec.
- (2) None means consensus invalidity. Appearance in pocol_getRejects, triggering parentInvalid/ancestorInvalid, HfullDone/blockReject, or penalizing an honest peer for it fails G.
- (3) PinSet includes envOk and envPending copies.
- (4) EnvHeldAtHalt extends pinBreach.
- (5) Possession, closure and reachability lemmas explicitly require validEnv.
- No validity change; rest of D131 retained.
  Rationale: Alternatives:
(A) Rely on implicit D131 precedence: leaves textual conflict between QV25a and the common criterion (I72).
(B) Merge envUnavailable into bodyUnavailable: misclassifies a block with a held body, falsely failing pinBreach.
(C) blockReject on envelope exhaustion: restores poisoning prevented by D129.

The choice derives all vectors/controls from one classification. Correct QV25a passes; envAsBlockReject fails an explicit rule.

Cost: extra reason field and EnvHeldAtHalt reference definition; pinning envelopes increases pinBytes.
- D133: A transitive total fork-choice order:
- t1 ≻ t2 by cumulative work first. On ties compare children of the lowest common ancestor using key = (signed ? 0 : 1, importSeq, blockHash).
- importSeq is a local counter assigned at first successful import and constant while retained.
- Tip = maximum, independent of enumeration order.
- Transition lemma: tip changes only on import.
- Verification: amended Q2, Q2b, ForkChoiceRef, QV26 with twenty import orders and three enumeration orders, QV26m, pairwiseTiebreak control.
- Classification: consensus fork-choice rule, not a validity change.
  Rationale: Alternatives:
(A) Selected: lexicographic path order. Transitive by construction, preserves signed preference, cannot be ground.
(B) Minimum blockHash at difference: consistent across nodes, but winner can grind shareRoot to win every tie.
(C) Tip first-seen alone: transitive but removes D15 signed preference.
(D) Retain pairwise comparison with fixed enumeration: conceals rather than removes the cycle; results still depend inconsistently on arrival order.

Cost:
- Same-kind sibling ties stay local; honest nodes may retain two tips until the next block.
- Delivery-order-controlling adversaries affect local ties as with first-seen, measured in T10.
- D134: Bounded-name epoch fence:
- Name only 'pocol:epoch:'+hex16(E), no nonce. Generations computing the same E write the same name; retries add no names.
- Before issuance: EN+1 ≤ EPOCH_NAMES_MAX = 8 name gate, then generation window over visible items.
- On violation, wait monotonic T_LATE and sweep. Persistent violation disables local storage for the generation with storeRecovery {epochNames}.
- Sweep: after T_LATE from live-generation boot, delete every name below the greatest visible, regardless of stored bootMs.
- Lemmas: writers, name bound (≤ 8 always), lateGens coverage despite late value writes.
- Rederivation: LATE_NAMES = 24 excluding epochs; name condition adds EPOCH_NAMES_MAX; META_RESERVE reserves 8·EPOCH_BYTES_MAX = 2048.
- Supersedes D103 nonce naming/sweeping and every LATE_NAMES = 28 mention in D106/D108/D109. Now 24 plus 8 epoch items.
- Verification: BR22h, epochNonceNames/epochNoGate/epochSweepEarly controls, within M1-spec.
  Rationale: Alternatives:
(A) Fixed ring of overwriteable names: late writes may lower the maximum, repeating E across two data-writing generations and colliding seq.
(B) Two keys: fence plus generation item written only by confirmed generations. Correct, but coverage proves the shared name suffices; the second key adds an unnecessary write.
(C) Limit worker-loss rate: unjustified assumption.
(D) Sweep only after items appear: no instantaneous bound; sweeping may fail.

The choice makes the bound independent of disappearing-generation count: each possible name is visible or M+1; gate precedes issuance. Safe sweeping T_LATE after live boot prevents resurrection without trusting stored bootMs.

Cost: a loss loop shorter than 60s after reaching the limit, or persistent delete failure, disables local storage fail-closed with notice.
- D135: Correct name-boundary vectors after D134 without changing the rule:
- Every BR23 name-bound vector follows names+R_names+1+LATE_NAMES+EPOCH_NAMES_MAX ≤ 1024.
- d11: boundary 991; first admitted, second disk.
- d11b: old-formula boundary 999; expect rejection.
- Negative control bridge-fault:epochReserveOmitted fails d11/d11b.
- Explicit d6 derivation: first rejection at 992.
- M1-spec carries both paths of both vectors and corrects LATE_NAMES = 28 to 24 plus 8 epochs.
  Rationale: I64.

Alternatives:
(A) Change rule to fit old vector: removes the epoch reserve underlying the name-bound lemma.
(B) Delete the vector: leaves the boundary untested.
(C) Correct only d11: fails to detect omission of eight, since corrected d11 admits the first item in both paths.

The choice aligns vectors with the current rule and adds a deterministic discriminator for defective implementations.

Cost: one additional vector and negative control.
- D136: Bounded envelope-verdict retention (local node policy):
- Verdict without copy bytes ≤ 128 B; retain only for a blockHash with a local entry.
- Bounds: 8 rejection verdicts per block, 16384 globally, 1024 pending copies globally, 64 checks in progress. ENV_MEM_MAX = 6 MiB.
- At capacity, drop unsolicited copies without checking or penalty.
- Always check solicited EnvFetch responses, with per-block then global FIFO eviction.
- Clean up on resolution/removal.
- Eviction permits rechecking; fullness never writes blockReject.
- HaltPin pins rejection verdicts during draining.
- Verification: T23 N11–N11c, three negative controls.
  Rationale: I75.

Alternatives:
(A) Permanent deduplication set: unbounded memory.
(B) Time-to-live alone: cannot bound peaks under rapid flooding.
(C) Per-peer quota: cheap peer identities bypass it.
(D) Drop every copy at capacity, including requested ones: flooding permanently blocks the valid envelope.

The choice gives a fixed memory bound independent of peer count, while preserving recovery: solicited requests are bounded by strict EnvFetch scheduling and reach an honest holder. Validity is unchanged.

Cost:
- Flooding delays acceptance of gossip-arriving valid envelopes until EnvFetch retrieves them.
- Rechecking evicted hdrId costs c_env.
- D137: Envelope-fetch resource ownership, amending D136 checks/memory:
- Slot reservation: every GetEnvelope reserves one of ENV_SOLICIT_SLOTS = 32 before issuance. The slot owns one check and reply buffer ≤ 3131 B, granted to the head of EnvQueue (FIFO per block) in the send step.
- Gossip slots: separate ENV_GOSSIP_SLOTS = 32 for unsolicited copies; total ENV_INFLIGHT_GLOBAL = 64.
- Request condition: no request before parent state is available.
- Release exactly once on one of eight exits: check, NotFound, ENV_REQ_TIMEOUT = 2s, disconnect, over-limit, import, envOk through another path, or removal.
- Timeout gives reeligible with exponential backoff, not permanent exclusion.
- Late replies follow gossip handling.
- ENV_MEM_MAX = 7 MiB includes slots and EnvQueue.
- Verification: N12, N12t, N12b; envSolicitUnbounded, envReplyUnreserved, envTimeoutExclude controls.
  Rationale: I75.

Alternatives:
(A) Check above the ceiling: unbounded resources.
(B) Queue responses after receipt: unreserved bytes; eviction recreates starvation.
(C) Drop requested replies at capacity: fullness becomes denial of service.
(D) Per-block slot: bound scales with entry count.

The choice makes “solicited replies are always checked” true by construction within a fixed ceiling. Separate pools prevent gossip starvation of recovery. FIFO plus timeout provides an explicit recovery bound. Slots are local, so waiting adds no drain-bound component. Validity is unchanged.

Cost:
- An adversary holding slots with slow replies delays recovery up to (P−1)·⌈Q/32⌉·(2s+c_env).
- Honest peers slower than 2s are retried after exponential backoff.
- D138: EnvSlots cancellation ownership, amending D137:
- Slot states: reserved, buffered, checking, released.
- Cancellation request is separate from release:
  - reserved/buffered: reset substream, release in the same step.
  - checking: no release before check settlement. envPre/envOwn without apply is synchronous, uninterruptible computation ≤ c_env.
- ENV_REQ_TIMEOUT measures from send to last response byte; cancel its timer on entering checking. Response wins equality.
- Each grant has a never-reused u64 attemptId; ignore events not matching the current attempt.
- Cancelled-check result:
  - Retain envReject and penalize sender if x still has an entry.
  - Use envOk only if x remains in envWait.
  - Otherwise lateCheckDropped.
- Ceiling lemma: running checks ≤ 64, buffers ≤ 64·3131 B at every instant.
- Slot role duration ≤ ENV_REQ_TIMEOUT + c_env.
- Drain lemma: no timeout because RTT_i < ENV_REQ_TIMEOUT; checking is outside the timeout.
- Verification: N12c/N12r; envReleaseOnCancel/envTimeoutCoversCheck controls.
  Rationale: I75.

Alternatives:
(A) Interrupt checks on cancellation: interruption points inside signature recovery/assignment complicate implementation; the bound still depends on response latency, while checks are already short.
(B) Synchronous checking inside EnvFetch loop: prevents concurrent release by construction, but blocks all fetch/timeout events during each check.
(C) Timeout includes checking: breaks the drain lemma; RTT_i < 2s is insufficient.
(D) Decrement on cancellation while work continues: the D137 defect itself.

The choice makes check/buffer ceilings construction lemmas: ownership ends at work settlement, not cancellation request. Timeout measures reception alone, preserving the drain bound under existing RTT_i assumptions. Validity is unchanged.

Cost:
- A cancelled slot remains held up to c_env.
- Exceeding assumed ENV_CHECK_MAX delays recovery without breaking the ceiling and is monitored.
- D139: Corrected recovery bound and specified check scheduler, amending D137/D138:
- EnvCheckPool: four dedicated envelope-check threads with FIFO queue.
  - c_env = (⌈63/W⌉+1)·ENV_EXEC_MAX = 850 ms, bounding checking including wait.
  - Monitor ENV_EXEC_MAX overruns; do not release the slot.
- Work-conserving grants: release grants the EnvQueue head in the same step.
- EnvFetch reeligible is FIFO with one position per peer.
- D_x = (2P−1)·(⌈Q/32⌉+1)·T_role + B, T_role = ENV_REQ_TIMEOUT + c_env. P counts distinct peers over the interval, including holder; Q is maximum queue length; B ≤ 60s.
- Bound requires a stable peer set; churn increases P and is not hidden.
- Verification: N12p/N12h/N12w; envCheckSharedPool and ref-fault:recoveryOldBound controls.
  Rationale: I75: previous formula omitted the successful request and did not bound processor waiting.

Scheduler alternatives:
(A) Shared ComputePool: unbounded waiting.
(B) Consensus runtime: blocks validation.
(C) One thread per slot: costly on 8 GB.
Selected: derives duration from the ceiling of 64 live checks.

Bound alternatives:
(A) Unconditional fixed bound: false under churn.
(B) Connection-time ordering: breaks the one-pair drain lemma.
Selected: explicitly conditional, derived from existing scheduling after clarifying reeligible ordering.

Cost:
- Conservative, large bound for large Q.
- Unlimited peer churn delays recovery without a fixed bound.
- Four additional threads.
- D140: Narrow envelope-recovery theorem, amending D139 start point/conditions:
- t_0 is the first instant G1–G4 hold together and persist until acceptance: G1 parent state available, x retained in envWait with a local entry; G2 connected honest holder serves an envOk copy; G3 every post-t_0 request to it completes within RTT_h* < ENV_REQ_TIMEOUT; G4 stable peers.
- Time before t_0 is preCondWait measurement, outside D_x. Losing a condition voids the guarantee; restoration starts fresh t_0'.
- A consistently slower-than-timeout holder has no solicited-path acceptance guarantee. Acceptance is possible only through gossip or another holder, without blockReject.
- D_x formula, slot ownership and EnvCheckPool unchanged.
- Verification: N12a, N12s, N12s-i; envRequestBeforeParent/t0Unconditioned controls; three-valued RecoveryRef.conditions verdict.
  Rationale: I75: D139's bound exceeded its conditions.

Alternatives:
(A) Longer/adaptive timeout: increases slot occupancy under flooding without bounding arbitrary slowness.
(B) Add parent-availability time: no prior bound, since it follows sync/execution.
(C) Admit a post-timeout reply on the slot: recreates unbounded occupancy and breaks D138.

The choice makes the guarantee correct under explicit trace-checkable conditions and treats everything outside them as disclosed measurement, not timed success/failure. Validity is unchanged.

Cost: no acceptance guarantee when the only holder is persistently slow or its link adversarially controlled; the block remains in envWait up to queue limits.
- D141: Record mining results in the child block.
- apply(h) reads neither block h's nonce, owner, shareList nor either signature.
- closeBlock(h) leaves pending[h].winner = 0.
- recordResult(h−1) in block h phase 0 writes winner from ctx.parentResult: the parent nonce owner computed by item 13 validation and stored in BlockMeta.
- Guarantee: D ≥ 2 (R8) places recording before every settle.
- F: no pending[F] is created; recordResult(F−1) precedes finalize.
- Distinct keys per block unchanged.
- Verification: TW1–TW4, exec-fault:winnerInState control.
- Consensus rule applied before any network exists, so no fork.
  Rationale: I76.

Alternatives:
(A) Selected: defer to child. Reuses recordShares(h−1), changing neither encoding, timing nor settlement amounts.
(B) Post-close root outside TemplateID: changes header/blockHash encoding and L0/T0/RP vectors, doubles roots.
(C) Read winner from ancestor header at settle: extends ctx dependence to D blocks; pending is no longer self-contained for RPC, evidence and rollback.

Cost:
- pocol_getPot shows winner = null for the tip.
- F's winner is not recorded in state; it already has no direct reward.
- parentResult correctness depends on retaining nonce owner with parent; TW/Q4 detect errors.
- D142: Complete parent result is an input to child execution:
- ctx.parentResult = ParentResult(p) = {blockHash(p), winner = owner(p.nonce), shares[(n, owner(n))] for each n in shareList(p)}.
- Sole source: Header(p) bytes keyed by blockHash(p), committing shareRoot, with p's assignment over grandparent state.
- recordShares(h−1) writes count[h−1] (including ν for winner) and lastShare only from shares. recordResult(h−1) writes winner.
- BlockMeta's stored value is derived; rederive/compare on every replay (parentResultMismatch).
- No mining memory, share pool or another copy of (TemplateID, nonce).
- Verification: TW5 with four arms and an ordering variant; sharesFromFirstSeen, sharesFromMiner and sharesDropped controls.
  Rationale: I76 (reopened).

Alternatives:
(A) Selected: explicit parent result derived from committed content. All envelope copies yield the same result; parents with different shareRoots are distinguished by blockHash.
(B) Pass full Header(p) to apply: brings assignment computation and grandparent state into the host, duplicating item 13 work.
(C) Record shares in the same block: recreates circularity because shareList is outside UT.

No validity rule changes beyond specifying the input. SV1–SV10 and empty-list TW1–TW4 values are unchanged.

Cost: retain a derived value per block and rederive it on replay.
- D143: Fixture-vector share thresholds derive from the built UT:
- T_share = min(2^256−1, UT.target·CP.m), through FixtureMiner.shareThreshold calling the consensus function.
- Record block-10 ASERT trace in sv-fix literally: Δt = −27, e = −2950, s = −1, f = 62586, target = F·2^223.
- Item-9 predicate for every share.
- Controls: TW5-gap (gap share rejected by item 9) and fixture-fault:fixedShareThreshold (deterministically fails threshold equality).
- TW5 settlement values unchanged.
  Rationale: I77.

Alternatives:
(A) Change sv-fix timestamps to make target = 2^240 at h=10: changes the shared fixture rule and requires rederiving SV8/SV9/TW1–TW4.
(B) Retain the constant and search again: hides the contradiction.

The choice makes fixtures use the rule itself, preventing divergence. The deterministic control detects the constant without relying on a probabilistic event.

Cost: literal F is not written here; two implementations and a reference establish it in annex M4b.

## Risks requiring later verification

- The β threshold and common prefix are unproven [PDF:74-77, PDF:140]. Passing T9, T10 and T10b does not establish security.
- The arrival theorem is conditional on Δ ≤ 5s, t_exec ≤ 3s and φ ≤ 5s, measured by T14, T4-H and T23. Genesis C1 margin is one second; T22 prevents startup if it fails.
- R7 is nominal; T4-H fails if truncation exceeds 2%.
- D74: F values and AS targets are manually derived, verified by two implementations and an unbounded reference. ASQ measures approximation quality; T4-B measures interval oscillation.
- D83: the equivalence lemma is textual and compared by AS-eq. ASB measures possible library heap allocation.
- D84: RP's work bound is weak: 4095 per header, 1 at saturation. A network lagging ≥ 4τ is not displayed; an inaccurate browser clock may give viewFuture. LN is the baseline.
- D85: E1 assumes closing creates no records and the cursor processes RET within G_SYS, measured by CL-END and T17.
- D86: CL-END does not test long registration turnover; T21 and T18b cover it. GB-END measures GenesisBuilder with 1024 identities.
- D88: numeric memory bounds are assumptions; RG4, RG5 and RG11 fail above RSS_0+256 MiB.
- D88: classification table is manual, measured by RG13. SCRATCH accounting assumes allocation on the computation thread.
- D88: local starvation of new connections and heavy methods is disclosed (RG8, RG9).
- D93: A measures socket acceptance, not client reads; assertions establish rule application only to measured A.
- D93: PacerSim is a simplification; RG14 covers Windows socket behavior only through assertions.
- D93: DiscriminationCheck and C cases may remain inconclusive; report values without claiming success.
- D93: PacerTrace may affect timing; traceDropped > 0 makes the run inconclusive.
- D94: server completion does not mean client receipt. A nonreading client occupies a connection until IDLE.
- D94: a cutoff absent from the RpcAttacker log makes the run inconclusive.
- D92: ENFORCE_SLACK=200 ms is a Windows-timer assumption. Credit permits retaining reservations within the derived bound.
- D90: an honest intermittent reader without credit may be closed; batchWallHits > 0 in release is a defect.
- D91/D89: P3 assumes deterministic encoding. A reorg after the check is not detected. pocol_getLogs is nonstandard; double scanning increases cost.
- D95: the manual JSON parser may have vulnerabilities; passing BR does not establish their absence. Sites requiring eth_getProof, filters or eth_sign do not work in v1. ORIGIN_DENY does not protect against a local process.
- D96: quota and value bounds are tighter than native localStorage. Atomic writes rewrite the entire dictionary. Navigation loses JavaScript state; external confirmation depends on the user.
- D97: BR19c does not detect arrivalMs timestamping defects. A write-heavy site may reach the rate limit.
- D98/D100: parsing memory is measured, not proven. Bounds assume compact JSON. Capped methods do not work above their bounds; RecvFit rejects large profiles.
- D99: malicious tabs may starve other sites' reads; the navigation bucket may slow users.
- D101/D102: set atomicity is assumed. Two hung seats stop saving for every site.
- D103/D104: resolution assumes atomic set and one live worker. Disk bound is conditional on A15c; getKeys is required. Persistent removal failure stops saving.
- D134: an MV3-worker disappearance loop shorter than 60s after EPOCH_NAMES_MAX is reached, or persistent remove failure, disables local storage fail-closed. The name-bound lemma is textual and conditional on A15c; BR22h and SR-model detect violations, rather than providing machine-checked proof. Literal BR22h(h5) combination times are written in M1-spec.
- D135: BR23 name-boundary vectors must remain derived from the full admission formula. Any later LATE_NAMES or EPOCH_NAMES_MAX change requires rederiving d6, d11 and d11b together.
- D136–D140: envelope flooding delays admission of a valid gossiped envelope until EnvFetch retrieves it.
- Bound: D_x = (2P−1)·(⌈Q/32⌉+1)·(ENV_REQ_TIMEOUT+c_env) + B, measured from t_0 under G1–G4 (D140). Large Q makes it large.
- Time before parent state availability and before the conditions hold is outside the bound; it is only measured.
- Unbounded peer churn increases P without a fixed ceiling.
- An honest holder consistently slower than 2s has no solicited-path admission guarantee; an adversary controlling its link can enforce this. The block stays in envWait until queue limits.
- Eviction permits hdrId rechecking at c_env cost; a seat cancelled during checking stays held until check settlement.
- ENV_EXEC_MAX = 50 ms is a measured assumption. c_env = 850 ms derives assuming EnvCheckPool threads receive CPU; exceeding it delays recovery without breaking the capacity bound.
- Aggregate checking rate is measured in N11/N12, not proven. Numeric values are assumptions.
- D105–D111: a pending ticket, tombstone, anchor or read holds seats until worker disappearance. Deletion may take 45s. Lemmas assume no await between grant and issuance.
- D112–D114: site bound is conditional on A15c/A15d. New sites may be temporarily rejected near capacity; a deleted site remains counted until its tombstone is removed. d15c uses a test hook.
- D115: VA on SimNet does not transfer to a real network without T3/T14 and may exceed the runtime budget on 8 GB. T25 on one machine does not represent availability.
- D116/D120: analytical bound covers only ownSharesOnly. Indirect effects through Draw are not analytically bounded; T7c(S) failure requires incentive changes.
- D121/D117: CEN and quorum values are assumptions; 24-seed bootstrap may fall below nominal coverage. AdvOracle is an unrealistic capability.
- D122/D123: stopMining/purgeFuture are fixtures; HeadCheck establishes rule correctness, not convergence speed. The lemma is conditional on ChainTimer completeness.
- D124/D125: U path and BodyMismatch are synthetic. depUnavailable may drop a valid child. T_REBUILD_MAX is measured. Rollback deeper than R_keep is manual.
- D126: HaltPin/narrow suspend eviction, so they do not represent real memory pressure. The broad bound may exceed DRAIN_CAP after long partitions.
- D127/D128: the bound lemma depends on complete ActLog classification and correct HdrCache/FetchSched. Bounding productive requests by header count is highly conservative; DRAIN_MAX may approach DRAIN_CAP. SimParams permits test overrides, rejected by the gate outside QV. QV21 is expensive to prepare (approximately 10^8 hashes). Excitations in QV21/QV22 are measured and may give FixtureSetupError with differently timed implementations. QV20 transition count is a measurement, not a criterion. c_h, c_rd, T_REBUILD_MAX and PIN_MEM_MAX are fixed in VA-spec; old QV20 text is removed there.
- D129: classifying items as envelope or committed content is manual; mistakes may restore poisoning or accept defective copies, detected by QV23/QV23d rather than proof. A connected adversary forces envelope checks per copy until D136 limits or banning. Venv is measured and may grow after many pre-barrier copies. Exponential timeout (up to 60s) delays recovery when announcements are lost.
- D130: strict untried-first selection delays a newly holding peer until untried peers are exhausted, up to (|H|−1)·RTT in the live network. cycleReset at t_avail is a fixture command; the one-pair lemma concerns closed draining. envOwn adds assignment computation cached per parent. An adversary sending many copies before parent state can fill envPending to ENV_PENDING_GLOBAL.
- D131: RTT_i is a conservative prior bound; one slow link inflates every connected honest node's bound. envUnavailable is fixture exclusion, not later envelope arrival in a real network; pocold keeps the block in envWait until queue limits. ENV_PENDING_MAX may discard a valid copy during flooding; recovery depends on EnvFetch from an honest holder. validEnv in HeadCheckRef depends on RefReplay of parent state.
- D132: pinning envelope copies and rejection verdicts in PinSet raises pinBytes after pre-barrier flooding and may reach PIN_MEM_MAX, making the seed unevaluable. Correct three-way classification depends on SimNet/HeadCheckRef agreement, checked by equality rather than proof.
- D133: same-type sibling ties use local import order, so honest nodes may retain two heads until the next block. Delivery-order control influences ties as with first-seen ordering; T10 measures the effect. D118 deletion and reimport change importSeq, altering local preference without changing work. QV26/ForkChoiceRef detect implementation errors, not machine-checked proof.
- D141/D142: recording the parent's result (winner/shares) in its child makes state validity depend on ParentResult derived from Header(p) and p's assignment over grandparent state. Derivation errors or a share copy unbound to the parent's blockHash change the child's stateRoot, detected by item 17, TW1–TW5, Q4 and parentResultMismatch, not machine-checked proof. TrieRebuild needs sequential grandparent state for rederivation. pocol_getPot shows winner = null for the head. F's winner is not recorded in state. System-key counting (KN/KC/KF unchanged) is textual reasoning rechecked by ParamGate.derive and T14.
- D143: TW5's share threshold derives from actual ASERT. The literal F value for block 10 is not written in the specification; two implementations and a reference establish it in M4b. Any other share-search fixture must use FixtureMiner.shareThreshold, not a constant.
- D118: dropping the lower side branch relies on synchronization to recover it.
- D119: ReplicaMonitor trusts listed replicas and does not prove availability; warning can lag by 30s.
- MetaMask and other tools may request eth_getLogs ranges > 1024 and fail; recorded in T15.
- A13: resistance to multiblock timestamp grinding is unproven; T10c measures it.
- D72: an adversarial proposer wastes attempt a=0. A partial upgrade produces an announced split.
- D77/D82/D80: RP's window does not verify execution; an RPC falsely claiming h=0 denies service.
- D79: GSV1 bytes are manual; the extension does not verify allocRoot.
- D78/D75/D76: pending restoration depends on rollback. NX depends on diagnostic isolation; SV8-reorg/RG3b depend on ASERT divergence.
- baseFee in empty blocks is below 8 wei; SV9 gas values are corrected by agreement of three implementations.
- D43 depends on bytewise Build_fb agreement; SYS bounds assume matching Solidity keys.
- D65/D67: Δ_path may dominate; F's winner has no direct reward; large baseBurn reduces income.
- SettlementIndex is nonconsensus. Grinding may lower K_eff. Writes from another context are detected after occurrence.
- D37/D63: RtcLockdown may be bypassed; X8 required before any public release.
- R19: a full final block may exceed 3s on 8 GB.
- Deterrence is reduced during closing; PAY does not notify the recipient; collusion invalidates X^h and X^W.
- Burning the last active miners halts the chain; E1 is an upper bound, not a guarantee.
- DNR, CSP and MV3-worker behavior is unverified (T12).
- Illegal content is not deleted before the end.
- ERC-4804/ERC-6860 texts and store policies have not been read.
- Replica count is not proof of availability (T25).
- Energy reduction may reduce attack cost [PDF:79].
- Windows tools may violate constraints (M0).
- Specification length exceeds the target; proposed movement to annexes is conditional on Codex confirming nothing is omitted.

History of superseded values:
- LATE_NAMES: D106/D108/D109 mention 28; binding value 24, plus 8 epoch items (D134).
- ENV_MEM_MAX: D136 says 6 MiB; binding value 7 MiB (D137).
- Timeout release: unconditional in D137, qualified by D138.
- Recovery bound: D138's ENV_CHECK_MAX and (P−1) formula replaced by ENV_EXEC_MAX and D_x in D139.
- t_0: D139 definition replaced by D140's G1–G4-conditional definition.
- winner: earlier D78 wording (phase 2) replaced by D141.
- parentResult: D141's {winner} expanded to full ParentResult by D142.
- T_share = 2^245 in TW5 superseded by D143.

## Final review risks

- Persistent partitions, selfish mining or timestamp grinding may weaken liveness and confirmation security. T10/T10c are measurements; T10b tests specified scenarios; no security threshold is proven, consistent with [PDF:74–77, PDF:140 §8.5].
- Identity splitting and withholding others' shares may change rewards and Draw. T7b accepts increases no greater than 1%, T7c no greater than 2%, under specified CI; gate failure requires incentive changes. Passing does not establish general incentive compatibility.
- Envelope recovery is conditional on G1–G4 and measured EnvCheckPool timing. N12a/N12s/N12w distinguish parent availability, slowness and check overruns; no guarantee with a sole holder consistently slower than timeout or unbounded peer churn.
- A remote RPC can provide consistent but false history, roots and content passing the RP window; this is not full verification. LN is primary; NX/MR test inconsistent tampering and receive limits without proving RPC honesty.
- Chrome storage bounds depend on item atomicity, one live worker, A15c and A15d. BR22–BR24 test ordering, delay and bounds; violated assumptions prevent adopting disk/name/site bounds and cannot merely be described as experimental success.
- Sandbox network leakage, RtcLockdown bypass or an unprotected wallet path has not been tested in practice. Any forbidden canary access, unapproved send or rejected-message effect in T12, X8 or BR blocks extension release.
- Windows memory/timeout bounds are assumptions. Exceeding resource, enforcement or cancellation limits in T14, RG or MR fails the dependent stage; runtime separation alone does not establish CPU/disk independence under pressure.
- Losing all three replicas truly loses data. T25 requires byte-identical retrieval, warning within 30s and explicit failure on loss; ReplicaMonitor is informational, not proof of availability or physical independence.
- DrainPin, stopMining and U path are synthetic test instruments. HeadCheck/BudgetCheck do not establish recovery under a continuing adversary or live eviction pressure; limitations on transferring SimNet results to real operation remain disclosed.
- Accepted prototype decisions: pocold with revm, website bytes in chunk contracts, LN as trust baseline, separate anvil experiments and PoCol integration. No approval for a permanent public network or its funding; S4 requires a separate specification.
- Visual review was not performed: equations, figures and tables named in scope, especially [PDF:92 §5.6.3, PDF:103–104 §5.9, PDF:108–110 §5.10]. Extracted text is insufficient to claim inspection of their images.
- No open objection in this textual adversarial review. First reviewable/testable task is M1-spec, then fixing M4b vectors including F and reviewing VA-spec/M7-spec before dependent implementation. All named tests remain proposed.
