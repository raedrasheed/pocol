# Adversary and trust model

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
