# Upgrades and operation

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
