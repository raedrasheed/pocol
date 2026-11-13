# Fresh v3-only PoCol network: consensus design (corrected after review)

I read `origin/main` d24843d in `/home/user/pocol-import` and made no edits. This design merges four analyses (timing, membership, economics, sites) with:
- the owner decisions of 2026-11-13 (`OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md`, including both follow-ups);
- the removal plan (`/tmp/claude-0/-home-user-Proof-of-Collaboration/5c42c059-8cb3-5c8b-83e8-431ffbd082d1/scratchpad/removal-plan.md`);
- the two reviews. I checked every review finding against the code. The appendix lists how each was resolved.

## 0. Decisions this design makes

| Point | Inputs said | Chosen | Why |
|---|---|---|---|
| Inactivity removal | timing: Θ_inact 360 (1 h). membership: 6 h plus a first-share deadline and an automatic sweep. Review 1: the `Capacity` path is not atomic, and `removeInactive` is not gated. | **Θ_inact = 360 (1 h), enforced only when a seat is needed:** (i) `removeInactive` is gated on `active == M_max`; (ii) a new atomic `registerReplacing(reward, victim)` registers the sender and exits a victim that `removeInactive` could remove at that height. No automatic sweep. | `register` fails with `Capacity` at a full set (`PoColSystemV3.sol:513`). Today `removeInactive` (`:490-494`) only exits the victim, so a seat it frees can be taken in the same block by a higher-tip registrant or by the proposer. An EOA cannot bundle the two calls: a contract caller registers itself (`id = msg.sender`, `:476`) and its self-reward is not durable (`:514-519`). Ungated, a zero-work griefer can remove up to REG_OPS = 16 members per block even when no seat is needed. An inactive member costs nothing: its draw weight is zero, because weights are share counts in W_w (`chain.rs:556-561`), and SYS gas does not depend on M. Owner confirmation: Q2. |
| R_max | timing: 2. membership: 4. | **4** | Draft V3-09 value: 1,440 registrations per hour. Honest throughput matters more when seats are contested. |
| Dead CP fields | delete, or reserve at 0 | **Delete 8 fields:** P_MAX, T_PROB, F0_acct, F0_slot, F0_code, E_epoch, theta_bp, k_sigma | No chain runs CP schema v2, so D-V3-34's "immutable wire order" binds nothing yet. This costs one renumbering in FN-2b. |
| T_PROB | delete, or keep | **Delete** | Owner 10c names no deadline. The deadline existed only to recycle pool seats. |
| Founders' bond | — | **Allocate 999,980 PCOL to each founder. `genesis_builder` mints the 20 PCOL HELD bond into Escrow.** | Initial supply is exactly 4,000,000 PCOL (`genesis_builder.rs:64-87`). |
| Caps | draft 200k / 500k / 32 MiB, or the base spec | **ACCT 1,000,000 / SLOT 4,000,000 / CODE 2^30 (1 GiB)**, subject to A13 store mode; fallback CODE 2^29 | Base-spec values (`development/m1/reference/governance.md:13`). They give lifetime room for about 256 sites at the 4 MiB publisher maximum, or about 40k sites of 25 KB. The three operator sites stay hosted (row "Sites"), so they are not part of this argument. |
| Sites | the IDE bundle on-chain? | **explorer/studio/ide stay HTTPS-hosted tools of the network. The on-chain requirement is that any user site (HTML/CSS/JS) up to the publisher's 4 MiB / 1 MiB-per-file limit is published and viewed through explorer, studio, ide and the extension. The explorer bundle (1.66 MB, 22 files, largest 687 KB) is published on-chain in the rehearsal as the reference site.** | The IDE bundle is 20.6 MB and its `compiler/soljson.js` alone is 10.8 MB, above `FILE_MAX` 1 MiB and `SITE_MAX` 4 MiB (`publisher.js:45-46`). Raising those would be a client project on its own and is not needed for owner requirement 2. |
| Node-local timing | (2,2,2) default, or (1,1,1) | **Declared (Δ1, φ1, t1), Δ_rel 7; fallback (2,1,1), Δ_rel 8** | At dFbWait 8, (2,2,2) fails R3. The same CP admits both declarations. |
| Launch admission | design: rebind R22 and admit as Network/Node. Review 2: that cannot work. | **Admit the profile as the old network was: `Simnet` kind, `Simnet` node, `ExperimentalDevnet` scope. R22 stays bound to 777001; the fresh id is any other value. `publicLaunchReady` stays false and its blockers are disclosed.** | R13 forces a continuous profile (H_END = H_CLOSE = T_END = T_CLOSE = 0) to be Simnet/Simnet (`gate.rs:315-317`). ExperimentalDevnet admits only Simnet/Simnet (`admission.rs:497-500`). Specification scope refuses any open condition (`:507-515`), and `continuous-protocol` and the deferred LP3 items are always present. R21 at these caps holds only through the simnet `retentionStress` exception (`gate.rs:352-355`). `pocold/src/lib.rs:248` hard-codes Simnet/Simnet/ExperimentalDevnet. The network still runs publicly and permissionlessly. Owner confirmation: Q3. |

## 1. CP and constant table for the new genesis

After the deletions, CP schema v2 has **59 fields**: the v1 fields 0-51 unchanged, then 7 v3 fields.

Every value must pass ParamGate R1-R26 and A13 store mode at the final values. I re-checked these by hand: R2, R3, R5, R7, R8, R9, R10, R16 and **R20**. R18 needs FN-2b's gas domain. R21 needs the simnet exception (§1c). "Measure" means the value is a launch input, not a literal.

### 1a. Consensus parameters (in genesisHash)

| # | Field | Value | Reason |
|---|---|---|---|
| 0 | g_ts | synchronized launch second | ASERT is anchored absolutely (`asert.rs:57`). All 4 founders must be mining within D_att of g_ts. |
| 1 | target_g | `calibration_target(H_measured, 10)` | R25 with a **measured** class. Today `pocold/src/lib.rs:245` labels the configured rate `SyntheticFixture` (C7). |
| 2 | T_blk | 10 | Owner 10a. Gate-valid at measured Δ ≤ 2 s and φ ≤ 1 s. |
| 3 | τ | 600 (60 blocks) | R25 needs τ ≥ 100. Re-converges in about 75 blocks after a ±25 % step. Changing it later needs a fork. |
| 4 | nonceMode | 1 | Unbounded ranges (D-V3-01). |
| 5 | c | 2 | The R7 term is 60-64 s, below D_att. |
| 6 | D_att | **90** | a≥1 empty blocks fall from about 0.96 % to about 0.06 %. Passes R5 and R7. |
| 7 | dFbWait | **8** | R3 holds up to (Δ2, φ2, t1) and (Δ2, φ1, t2). |
| 8-10 | m / κ / S_max | 32 / 2 / 256 | κm = 64 and S_max ≥ 4κm (R8). These bound every payee loop: ≤ S_max+3 payees per pot. |
| 11 | D | 4 | 40 s settlement and late-share window. |
| 12 | W_w | 8 | Draw window. |
| 13-14 | α_bp / γ_bp | 1000 / 500 | Unchanged. |
| 15 | subsidy S0 | **4·10^18** (4 PCOL) | Owner, second follow-up. 34,560 PCOL per day; 12,623,040 PCOL in year 1. |
| 16 | B_reg | 20·10^18 | Owner 10c (earned bond). |
| 17 | R_max | **4** | §0. |
| 18 | M_max | **10,000** | Owner. u16. |
| 19 | M_min | 1 | Unchanged. |
| 20 | REG_RECORDS_MAX | **24,576** | R16: 10,000 + 4·(725+2048) = 21,092 ≤ 24,576. |
| 21-22 | SWEEP_MAX / SWEEP_SCAN | 16 / 64 | ≥ R_max + E_max. |
| 23 | REG_OPS | 16 | `registerReplacing` uses 2 units and 1 R_max entry. |
| 24 | W_act | 32 | 5.3 min. |
| 25 | Θ_inact | **360 (1 h)** | Owner 10b. Enforced only at a full set (§0, C4). |
| 26 | E_win | **360** | O-18=b. |
| 27 | U | **380** | ≥ E_win + D = 364 and ≥ W_act + 2. |
| 28-29 | E_max / EVID_BYTES_MAX | 4 / 8192 | Unchanged. |
| 30 | GAS_LIMIT | 8,000,000 | One full site chunk per block. t_exec ≤ 1 s (T3). |
| 31 | baseFee0 | 10^9 | ≥ baseFeeMin. |
| 32-34 | TX_MAX / BODY_MAX / B_code_max | 131,072 / 196,608 / 32,768 | Unchanged. R9 holds. |
| 35 | CODE_CAP | **2^30** | Fallback 2^29 if A13 fails. |
| 36-37 | TXBYTES_CAP / TX_CAP | 2^24 / 20,000 | Per block when H_END = 0 (`chain.rs:820-825`). |
| 38 | SLOT_CAP | **4,000,000** | Base spec. |
| 39 | ACCT_CAP | **1,000,000** | Base spec. |
| 40 | SYS_SLOT_MAX | **147,456** | slot_min ≈ 6 + 5·24,576 + 10,000 + 8·516 + 77 = **137,091**. The tail 2+3(D+1)+1+59 = 77 uses the 59-field CP. Recompute with `lp4 resources.rs` after FN-2b. |
| 41-44 | SYS_KEYS_BLOCK_MAX / _USER_MAX / SYS_ACCT_BLOCK_MAX / G_SYS | 4096 / 256 / 512 / 96M | Re-verify R18 after the envelope is re-measured (Phase 6a). |
| 45-48 | H_END / T_END / H_CLOSE / T_CLOSE | 0 | Continuous chain. |
| 49 | Z_close | **256** | **R20:** Z_close ≥ ⌈REG_RECORDS_MAX/RET⌉ + P2 + D + 1 = 192 + 2 + 4 + 1 = 199 (`param_gate.rs:83`, `gate.rs:351`). The old value 160 fails. RET cannot rise above 128 (`admission.rs`, `in_gas_domain`). Unused while H_END = 0. |
| 50-51 | RET / P2 | 128 / 2 | Unchanged. |
| 52 | H_half | **12,623,040** | 4 years × 8,640 blocks per day. |
| 53 | S_tail | **1.25·10^17** (S0/32) | Tail exactly from era 5. Builder check: S_tail = S0/32. |
| 54 | γ_tip_bp | 5000 | O-20=b. Was index 59. |
| 55 | baseFeeMin | 10^7 wei, unless Q1(b) | O-20=b. Was index 60. |
| 56 | R_react | 4 | Was index 61. |
| 57 | C_react | **380** (≥ U) | Was index 62. |
| 58 | ρ_bp | 2500 | O-17=b. Was index 66. |
| — | **deleted** | P_MAX, T_PROB, F0_acct, F0_slot, F0_code, E_epoch, theta_bp, k_sigma | C1-C3. |

### 1b. Genesis contents (not CP fields)

- **Chain ids.** Draw two fresh random values in [2^24, 2^32) before the rehearsal and record them in FN-0's decision record:
  - one for the rehearsal and one for mainnet, which must differ so that rehearsal transactions cannot be replayed on mainnet (EIP-155 binds only the chain id);
  - exclude 777001 (R22), 1599846715, 261597237 and 7779xx;
  - no release compiles either id in (§0, Launch admission), so both go through the same admission path.
- **Founders:** the 3 US servers and the owner's PC, each with fresh keys. The phone is a wallet only.
  - Each founder gets a HELD record with a 20 PCOL bond, minted into Escrow.
  - Each founder's reward address gets 999,980 PCOL, with nonce ≥ 1 (PAY-02).
  - Initial supply is 4,000,000 PCOL. A tooling check confirms allocation + B_reg = 10^24 wei per founder.
  - A founder that loses its seat (C4) can register again as HELD at once by sending 20 PCOL with `register`/`registerReplacing` (`enroll`: probation only when msg.value < B_reg).
- **Supply bound:** about 2.3058·10^36 wei. That is 121 bits against the 127-bit limit.
- **Programs:** the V3 program set only, with ChunkFactory at `0xC0C005`. ForkSchedule is `[(1,0)]` and MAX_FORK_VERSION = 1. The cap-raise path (C10) is compiled in and dormant.

### 1c. Node-local values and admission (outside genesisHash)

- Δ / φ / t_exec = 1 / 1 / 1, so Δ_rel = 7. If T1 measures more than 1 s, declare (2,1,1) and Δ_rel 8. Γ = 5.
- R_KEEP 2,048 (5.7 h); K_HIST 32; K_FINAL / K_VIEW 60 / 12.
- `safe` at 6 blocks and the reorg alarm at 60 (owner answer 12; N2 rework, §3).
- NTP offset ≤ 0.5 s on every founder, including the Windows PC through a configured w32time peer.
- Admission:
  - Simnet profile, Simnet node, ExperimentalDevnet scope, with `retentionStress` set.
  - R21's `historical_bytes` is far above `HIST_TRIE_BUDGET` at these caps. The actual resident bound is A13 store mode, measured in M1.
  - The calibration class is measured, not `SyntheticFixture`.

## 2. Consensus changes

The owner rule passes if a change leaves intact:
- collaborative consensus (shared shares and rewards);
- the template block (proposer-signed immutable template, header-only solve);
- P1-P10.

**No change below touches the template, the share and draw mechanics, fork choice, or reward splitting.**

| ID | Change | Files | Tests | Owner-rule check |
|---|---|---|---|---|
| **C1** | **Delete the D-V3-21 creation burn and the F0 fields.** Caps stay as A03 halts. | lp4: `creation_fees.rs` (delete), `lib.rs:125`, `user_phase.rs` (Halt::CreationFee, the burn field and split, `admit_result` burn path, `SignedFee`, `quote_creation`, `charge_creation`, the `halt_gas` special case `:358-360`). Keep `PoolMeter::admit` and `BlockMeter::check`. Both CP copies (`genesis_spec.rs`, `p1_genesis_spec.rs`); `profile.rs`/`p1_profile.rs`; `devnet.rs`; `refdec.js`. `admission.rs` blocker text. pocold: `rpc.rs` (`pocol_quoteCreationBurn`, drift and Margin/Quote, the simulate burn), `node.rs` burn sites, `resources.rs`, `readiness.rs`. | Rewrite `v3_execution.rs`, keeping the cap-halt cases. Also `v3_membership.rs`, `memory_bounds.rs`, `consensus.rs`, `v3_scaffold.rs`, `admission/tests.rs`. New: a chunk or deploy burns nothing beyond gas (supply invariant); the RPC method is gone. | Owner 10d. Fee rule only. **Pass.** Cap-fill risk: Q1 and C10. |
| **C2** | **Delete the seat contest (V3-10/E4) and the probation pool (V3-14).** | **`PoColSystemV3.sol`:** <br>• CP_P_MAX, ACC_COUNT, the epoch constants and counters, POOL_OUT, `pooled`. <br>• The ACC_COUNT writes in promote/refund/punish/enroll/reactivate (394, 500-501, 520-521, 525, 666, 755), `accruingCount`, the `extra` parameter of `checkBudgets`. <br>• `measured`/`threshold`/`isqrt` and the views (contest-specific; `contest()`'s skeleton is reused by C4). <br>• `credit`/`credited` become `lastShare = max(lastShare, j)`; `recordShares` returns to one pass. <br>• **Renumber literal and named CP indices:** `Sys.cfg(66)` → `cfg(58)` (`:190`), `Sys.cfg(59)` → `cfg(54)` (`:207`), `CP_R_REACT` 61 → 56, `CP_C_REACT` 62 → 57 (`:354-355`); ChainCaps comment (`:54`) and `require(n<67)` → `n<59` (`:57`). A missed index reverts `closeBlock`/`burnBond`, so every block would be invalid. <br>**Host:** `apply.rs` loses `Probation.seats`; `chain.rs:68-70` `Spec::probation()`. <br>**`admission.rs` `in_gas_domain`:** for `ProgramSet::V3` only, drop the M_max and P_MAX terms (no V3 SYS loop iterates members) and require the payee bound S_max+3 with all payees ACCRUING (the basis of the re-measured settle). Keep `m_max ≤ 256` for spec-1 sets until Phase 6a deletes them (`GAS_DOMAIN` covers a V1 loop over M_max records, `admission.rs:184-188`). Delete `GAS_DOMAIN_V3_P_MAX`; keep κm 64. <br>**`lp4 resources.rs`:** `registry_fixed` 8 → 6. <br>**Wallet:** every `apply::Probation` and slot-8 (ACC_COUNT) use in `wallet.rs` (around 83-87, 177-184, 327, 587, 637, 650); the contest plan, quote and UI become the C4 path. | Delete the contest blocks in `v3_contracts.rs`, `v3_model.py`, `contest-vectors.json`, `earnin-vectors-p64.json`. <br>**Delete or edit the source-text mutants** in `compile_system_v3.cjs` (31, 34, 92; re-point 51 and 60 at `cfg(58)`/`cfg(54)`). Regenerating them alone fails. <br>Regenerate `system-v3.json`, earn-in-on-v3, registry-budget, late-share and issuance-carry vectors. <br>CP-length consumers: `forks_test.mjs:82` (67 → 59), `pocol_rewards.py:458` `CP_ITEMS {2: 59}` plus its README, `lp1-node/tests/gate.rs:447-449`, `pocold/tests/v3_late_shares.rs:78`, `testnet.rs:286-287` (`t_prob`, `e_epoch`). <br>New tests: `payees(j) ≤ S_max+3` asserted in the model and on host chains; **settle with ≥ 256 payees all ACCRUING at bond 0, all promoted in one call, with carry**; EIP-170 runtime size. | Owner 10b. Removal reasons shrink to exit, inactivity and punishment (P8). ACCRUING, the earned bond and the B_reg/4 draw exclusion stay. **Pass.** <br>**Disclosed exposure (rule unchanged):** with P_MAX gone, up to 10,000 members may be ACCRUING. An ACCRUING winner accused with type-2 evidence makes the whole pot INVALID (`RewardEscrow.invalidate`, `:232-237`, `WIN_ACCRUING`), and members at bond ≥ B_reg/4 = 5 PCOL are drawable proposers (`chain.rs:552`). Add to `threat.md` and measure (M4). |
| **C3** | **Delete the probation deadline (T_PROB).** | Solidity: `pastProbation`/`probationExit` (483, 498, 644-653). The CP field. `apply.rs` `Probation.blocks`. `v3_earnin.rs`. Wallet probation display and its deadline text (`wallet.rs:83-87, 327`). | ACCRUING never exits by time. topUp and reactivate work without the deadline. Port the `earnin_refund_fix`/`earnin_halt_proof` regressions. | Owner 10c. **Pass.** |
| **C4** | **Inactivity only when a seat is needed, atomically.** (a) `removeInactive` reverts unless `active == M_max` (one condition in the `removing` branch, `:491-492`). (b) New `registerReplacing(address reward, address victim)`, payable 0..B_reg: the existing `contest()` skeleton (`:533-559`: calldata, BadValue, Closing, 2 REG_OPS + 1 R_max, BadPhase, sender without a record). Its check is: victim live and active; `active == M_max`; `height ≥ max(lastShare, grace) + Θ_inact` (removeInactive's rule, codes 4-7 gone); `active > M_min`. The victim exits through `exitAt` (refund after U, no burn, punishable until then). The sender then enrolls exactly as `register`. | Solidity, model, wallet (`Capacity` → choose a removable victim **at random** among the eligible ones to avoid collisions, then `registerReplacing`), gateway. A node RPC listing members removable at h+1 (needed by wallets). | At a full set: a newcomer and a competing registrant in the same block both succeed only on distinct victims. `register` alone still fails with `Capacity`. `removeInactive` below a full set reverts. A victim within Θ reverts. A removed member can register again after its refund at U (paying 20 PCOL gives HELD at once). R_max/REG_OPS budgets hold. Model and vectors. | Owner 10b ("removed after about an hour, may return"). **This is not a seat contest:** no performance measure, no threshold. It only makes atomic two operations that anyone may already do (remove an inactive member, register). **Pass.** Owner confirmation in Q2. |
| **C5** | **Registry view `activeSlice(start,count) → (ids, w0)`.** | `PoColSystemV3.sol` (same PR as C2, since sysCodeHash changes). `pocol_rewards.py` (`MAX_MEMBERS_V3` 10,000, paged reads plus a delta cache). | Gateway differential against the host at 10,000 members. | View only. **Pass.** |
| **C6** | **Node memory and CPU at 10,000 members** (node-local; `chain.rs` takes a `neutral` line). | `chain.rs`: replace `Entry.ranges` (64 B·M per entry: 64 × 10,000 × 4,713 ≈ 3.0 GB = 2.8 GiB at slim depth) with a per-block ActList delta plus a snapshot every 256 heights; Assignment recomputed on demand behind a small LRU. One-pass `membership()` (`:544-555`) with a cache keyed by parent and (parent, attempt). `history.rs`, `checkpoint.rs`, `node.rs`. `assignment.rs:213` `owner()` becomes a binary search. A13 terms in `pocold resources.rs`. | Identical verdicts on every v3 test. Kind-3 and type-2 evidence across slim/unslim, restart and reorg. A13 store mode at M_max 10,000, E_win 360, REG_RECORDS_MAX 24,576. `history.redb` growth per day ≪ 100 MB. | No rule change. **Pass.** |
| **C7** | **Parameter plumbing and builder** (tooling). | `testnet.rs`: a committed `deploy/mainnet/v3-profile.json` with the §1a CP (Z_close 256 included), plus a rehearsal profile that differs only in chain id and g_ts. Default localTiming (1,1,1). `hashRate` with a measured calibration class (`pocold/src/lib.rs:245`). Θ/E_win/U/C_react from the profile. Founder allocation check. `devnet.rs`: a `subsidy` field (no hard-coded 2e18), S_tail = S0/32 checked. Remove the `t_prob`/`e_epoch` defaults. `launch_delay_asert.rs` at the final CP. | ParamGate R1-R26 on the final CP and locals at (1,1,1) and (2,1,1), including R20 = 199 ≤ 256. Issuance vectors at h = H−1, H, 5H−1, 5H, plus `future_bound`. | Parameters. **Pass.** |
| **C8** | **Launch admission and identity (no R22 change).** Keep R22 bound to 777001. Admit through ExperimentalDevnet as the old network did. `Initialize-PoColNetwork.ps1` emits the v3 config with the chain id from FN-0's record. Readiness reports `publicLaunchReady = false` with `profile-kind`, `admission-scope` and the `admission-condition-*` blockers, disclosed in the launch notes. The Network/Specification path is a separate governance track: R13 for continuous Network profiles, R21 at the base-spec caps, the Scope and deferred-LP3 model, the `NETWORK_PROFILES` pin. It is offered as Q3(b). | `Initialize-PoColNetwork.ps1`, `testnet.rs` (id ≠ 777001 is already checked at `:255`). | The final and rehearsal profiles are admitted as Simnet/Simnet/ExperimentalDevnet. Readiness lists exactly the expected blockers. | Governance labels only. **Pass.** |
| **C9** | **`pocol_getCaps`** (live accounts/slots/code against the caps). Listed at `rpc.rs:528` with no handler. | `rpc.rs`; publisher and Studio pre-checks; occupancy alarm at 80 %. | RPC test; a publisher refuses a site that cannot fit before signing. | Node-local. **Pass.** |
| **C10** | **Cap-raise path, a launch requirement.** The D-V3-23 machinery restricted to the cap fields (ACCT_CAP, SLOT_CAP, CODE_CAP, SYS_SLOT_MAX): height-aware `cp_at(h)` for those fields, compiled in and dormant. Activation then needs only a signed release that sets the new values and a height. | lp4 `profile.rs`/CP access, `chain.rs`, gate (R-rules evaluated at both CPs), fork config. | Replay and reorg across the switch height. A filled account cap followed by a raise lets `register` and new accounts succeed again. Rehearsal step S2. | D-V3-23, already approved within the owner rule. **Pass.** |

Unchanged and verified:
- the issuance arithmetic (`issuance.rs:23-38`), ASERT and timing;
- ChunkFactory and Website, fork choice, the share ledger, late credit and the settle split;
- γ_tip and ρ (indices renumbered only).

## 3. Implementation order, merged with the removal phases

```
Ph0 ─ Ph1 ─ [in-flight: N5, N2', N3, G2, G3a] ─ Ph2 ─ Ph3 ─ FN-2a ─ FN-2b ─ G3b' ─ Ph4 ─ FN-3 ─ FN-4 ─ FN-5 ─ Ph5(a,b,c) ─ Ph6a ─ Ph6b ─ Ph7+FN-6 ─ FN-7 ─ Ph8 ─ rehearsal ─ ceremony ─ Ph10
FN-0 with Ph0;  FN-1 any time after Ph2;  Ph9 docs in parallel, finished before Ph10
```

| PR | Content | Placement and reason | Lock line (pre-reset) |
|---|---|---|---|
| **FN-0** | `development/decisions/FRESH-NETWORK-CP.md`: §1, the owner-rule checks, the superseded O-items (O-6, O-10, O-14, O-19, O-21, O-32, O-33), and both chain ids. **It also amends the stale removal-plan text:** the ceremony line "`F0_*` per decision, `P_MAX`/`M_max`/`T_blk` per decision" goes; in Phase 7, `registrationPerBlock` is kept (it is R_max, `testnet.rs:262`). | With Phase 0. Docs only. | none |
| **In-flight** | **N5** (`v3-n5-launch-ready`, #40). **N2** (`v3-n2-finality`), reworked from a single `K_SAFE = 60` to `safe` 6 plus an alarm at 60 (owner 12); T5's `reorgs_ge_k_safe` comes from here. **N3** (`v3-n3-mining-threads`: the non-mining node mode, owner 14, the T1 fallback). **G2** (`v3-pr-g2-reward-gateway`, #41). **G3a** (`v3-pr-g3a-verifiers`). | After Phase 1, before FN-2a, so FN-2 and the rehearsal build on them. | as each branch's own line |
| **FN-1** | `pocol_getCaps` (C9), one-pass membership and cache, `owner()` binary search (part of C6). | After Phase 2; parallel to Phase 3. | `neutral` |
| **FN-2a** | C1. | After Phase 3, before Phase 4, so Phase 4's spec bump covers the final schema. | one `kind=neutral-vector` line, **no gate** (the guard allows a gate only on `neutral`, `consensus_boundary.rs:626`) |
| **FN-2b** | C2 + C3 + C4 + C5, plus the CP renumbering: Solidity named and literal indices, `profile.rs`/`p1_profile.rs`, `devnet.rs`, `refdec.js`, `pocol_rewards.py`, `v3_model.py` CP_INDEX, and the consumers listed in C2. Regenerate the decoder verdicts from Rust. V3-only gas domain. Wallet and gateway C4 path. | After FN-2a, before Phase 4 and before Phase 6a, which re-measures `GAS_ENVELOPE_V3`. | one `kind=neutral-vector` line, no gate |
| **G3b'** | `v3-pr-g3b-wallets` rebased onto FN-2b: burn removed, contest UI replaced by `registerReplacing`. | After FN-2b. | none or `neutral` |
| Phase 4 | spec 2 by default, `v3-preview` removed, rules 2. | — | `bump` |
| **FN-3** | C6 storage redesign. | After Phase 4, before Phase 5c: the launch evidence must be measured on the final format. | `neutral` |
| **FN-4** | C7 builder, mainnet and rehearsal profiles. | With or just before Phase 5a. | `neutral` |
| **FN-5** | C10 cap-raise path. | After Phase 4, before Phase 5c, so Phase 5 tests it. | `neutral gate=fork:V`, with V above the spent maxFork |
| Phase 5 | devnets v3 by default, test migration. **Add:** <br>• a CI full-set test: a host-level test genesis with 10,000 records, bypassing the 256-miner limit of `testnet.rs:257`, measuring child_ctx, RSS, A13 and history growth, with the CI runtime budgeted; <br>• `init-devnet --v3`; <br>• `e2e_site.py` ported and **added to CI**. | — | `neutral`/`neutral-vector` |
| Phase 6a/6b | Drop spec-1 (which removes the spec-1 `m_max` gas-domain term), collapse the branches, re-measure the envelope. | — | `neutral-vector` |
| **FN-6** with Phase 7 | Clients: <br>• gateway `activeSlice`; <br>• extension: `SPEC_VERSION` 2, built-in profile placeholder, publisher fees from the node, chatgpt.site origins removed; <br>• Studio and gateway: canonical chain-id compare; an `eth_getCode` cache or a separate budget for immutable chunks; <br>• IDE and explorer constant patches; <br>• wallets' `registerReplacing` path. | — | none or `neutral` |
| **FN-7** | C8: the `Initialize-PoColNetwork.ps1` v3 config and the readiness disclosure text. No R22 change. | Last code PR before Phase 8. | `neutral` |
| Phase 8 | Lock reset to one `base` line; release candidate. | — | `base` |
| Rehearsal | §4b on the RC: 3 US servers (wiped by the owner) plus the home PC, at the final CP with the rehearsal chain id. | A failed T-threshold changes only profile values (D_att, dFbWait, T_blk 20-30 s as a last resort). | — |
| Ceremony, Phase 10 | Genesis with the mainnet chain id. Identity pinning: `studio-network.js` spec-2 `genesisPre`, Studio, gateway, explorer, IDE, extension. Under Q3(b) only, also `readiness.rs` `NETWORK_PROFILES`. | — | first new `neutral` |

## 4. Risks and what the rehearsal must measure

### 4a. Risks

1. **Node memory at 10,000 members.** Without FN-3, entry ranges take about 2.8 GiB of the 6 GiB `MEM_BUDGET`, and history grows about 4.5 GB per day. This is a launch blocker until C6 lands.
2. **Cap fill with no creation fee.**
   - The account cap fills first: 10^6 new-account transfers at 21,000 gas = 2.1·10^10 gas. At the 10^7 wei floor that is about **0.21 PCOL**. At about 190 per block (the 4M target, so the base fee stays at the floor) it takes about **14.6 h**.
   - After that, nobody without an existing account can get one. A newcomer then cannot register (the registrant is an EOA, `enroll` plus PAY-02), so permissionless entry stops. This conflicts with owner answer 10 ("anyone who wants to mine can").
   - All three caps fill for about 3.2-3.3 PCOL over about 9.5 days. Consensus and mining continue.
   - Mitigations: C10 is a launch requirement; C9's occupancy alarm; Q1(b). Disclose in `threat.md` A6.
3. **10 s latency.** These need measured Δ ≤ 2 s and φ ≤ 1 s.
   - The withholding gain is 1.6× the 30 s value (+0.105 at f = 0.2).
   - Natural forks run at about 0.8-1.4 %.
   - The home PC pays a latency tax of about 1-2.5 %.
   - Disclose all three.
4. **Calibration.** A wrong target_g gives either a burst of 1 s blocks or slow blocks for 75+ blocks. Measure the founders' aggregate hashrate.
5. **Launch synchronization.** A founder starting more than D_att (90 s) after g_ts produces fallback-only blocks.
6. **Sybil seats and eviction at a full set.**
   - A zero-work seat costs only gas (about 2·10^-6 PCOL). M_max fills in about 6.9 h at R_max 4.
   - With C4, an honest newcomer can always take the seat of a member with no credited share in the last hour, atomically. R_max slots are contested by priority fee.
   - The other side: at a full set of equal small miners (11,520 shares/h over 10,000 members, about 1.15 each), **about 31.6 % are removable at any moment**. Newcomers, honest or Sybil, can evict up to 1,440 per hour.
   - Evicted members cannot reactivate while the set is full. They register again after their refund at U.
   - Below a full set nobody can be removed.
   - Disclose this. Q2 offers the alternative.
7. **Founders' seats.** At a full set, a founder offline for more than 1 h can lose its seat to `registerReplacing`. It returns by registering again with 20 PCOL (HELD at once) after U, if a removable seat exists. Disclose this; exercised in R-off.
8. **V3-16 exposure** (C2). ACCRUING winners can make a whole pot INVALID under type-2 evidence, and low-bond (≥ 5 PCOL) members can propose. Without P_MAX their share of pots and proposals is no longer bounded. Measure it (M4) and disclose it.
9. **Gas envelope basis.** settle now has up to 259 payees per pot, all possibly ACCRUING. At about 32.7k gas per promoted payee (6.27M for 192, `admission.rs:209-210`), that is about 8.5-13M, which must be re-measured below the 30M per-call cap. R18 must be re-verified against the V3-only gas domain.
10. **Site throughput and lifetime.**
    - One full chunk per block is about 2.4 KiB/s.
    - A 4 MiB site takes about 29 min (175 blocks) and pushes the base fee from 0.01 to about 15 gwei (about 1.9 PCOL burned).
    - The explorer bundle (1.66 MB) takes about 70 blocks.
    - The IDE bundle (20.6 MB, one 10.8 MB file) cannot be published under the client limits and stays hosted.
    - CODE_CAP, not throughput, is the lifetime limit.
11. **Disk.** The journal is never compacted (`history.rs:5`). The worst case is BODY_MAX × 8,640 ≈ 1.7 GB per day (about 620 GB per year). Size the servers' disks for at least 2 years of worst case, or measure and alarm.
12. **Process.**
    - The decoder verdicts, both copies of `p1_*`, the Solidity named and literal CP indices, and the source-text mutants change in the same PR (FN-2b).
    - The lock-reset exemption is a one-shot.
13. **R_KEEP 5.7 h.** A longer partition forces a resync. Disclose it.
14. **Admission label.** The release reports `publicLaunchReady = false` (Q3). Disclose this in the launch notes.

### 4b. Rehearsal thresholds

**Setup:**
- Run on the final CP with the rehearsal chain id, for at least 24 h plus the M1 fill, with all 4 founders mining.
- Include a home-PC off/on step of 10 min and a near-BODY_MAX site publish while the PC is the proposer.
- If the PC cannot join, use `tc netem` with 150 ± 30 ms and a 1-5 Mbit/s upload limit.

| # | Measure | Pass | If it fails |
|---|---|---|---|
| T1 | `pocol_templateLatency` per pair | late 0; sinceParentMs p99 ≤ 1,000 ms (Δ1) or ≤ 2,000 ms (Δ2); max < Δ_rel − 2 s, including full-body templates from the PC | Peer the PC to the US servers only, declare Δ2 (same CP), require ≥ 5 Mbit/s upload, or run the PC as a non-mining node (N3). Last resort: T_blk 20-30 s before genesis. |
| T2 | Clock offsets | pairwise ≤ 500 ms; zero future-queued templates | Fix NTP. φ = 2 still passes at dFbWait 8. |
| T3 | t_exec | worst block (GAS_LIMIT, site publish): validate+commit p99 ≤ 1 s on every founder; R14/R19 admitted | Lower GAS_LIMIT or BODY_MAX, or replace the slow founder. |
| T4 | Empty blocks | a=0 ≤ 0.5 %, a≥1 ≤ 0.2 % (outside off/on windows) | Raise D_att. |
| T5 | Forks | siblings ≤ 2 %, max reorg ≤ 2, `reorgs_ge_k_safe` 0 at `safe` 6, no alarm at 60 | Fix relay, then T_blk 20-30 s. |
| T6 | Intervals | 24 h mean 10 s ± 5 %; difficulty noise ≤ ±10 %; re-converges ≤ 150 blocks after a 25 % step; first 100 blocks' mean in 5-20 s | Re-measure hashRate; τ 1,200-1,800. |
| T7 | Home-PC shares | ≥ 99 % credited; credited fraction ≥ 0.95 × hashrate fraction | As T1. |
| T8 | Launch drill | all founders mining within D_att of g_ts | Scripted start. |
| M1 | **Full set:** a registration harness registers up to 10,000 identities (about 2,500 blocks ≈ 7 h), mostly idle, with some ACCRUING miners | child_ctx p99 < 100 ms; RSS within the A13 bound; history growth per day within C6's target; gateway reward computation < 1 s per block; T4 and T5 unchanged | FN-3 or the caching is not done; do not launch. |
| M2 | SYS gas at the full set | settle (all-ACCRUING payees), recordShares, recordLate, beginBlock and the phase-0 total within `GAS_ENVELOPE_V3` and G_SYS; no SYS call grows with M | Re-measure; lower S_max or κm only through ParamGate. |
| M3 | Capacity path | at a full set, two registrants in the same block with `registerReplacing`: both succeed on distinct victims; `register` alone gets `Capacity`; `removeInactive` reverts below a full set; a victim re-registers after U | Fix the wallet or program. |
| M4 | ACCRUING exposure | fraction of pots won and blocks proposed by ACCRUING members is recorded; any type-2 INVALID pot is logged with its burned amount | Disclose; owner review if material. |
| R-off | **Founder offline > Θ + U (about 2 h) at the full set** | the home PC is evicted by `registerReplacing`; it is refunded at U; it registers again (with 20 PCOL → HELD, and once with 0 → ACCRUING, re-earning the bond) | Fix the wallet or program. |
| D1 | Disk per day | journal + history.redb growth recorded; projected 2-year worst case fits each server's disk | Size the disks or add an alarm. |
| S1 | Sites | **the real explorer bundle** and a synthetic 4 MiB site published (≤ 70 + 10 and ≤ 175 + 10 blocks); base-fee drift recorded; `pocol-site fetch` digests identical on all 4 nodes before and after restart; the Chrome viewer, Studio and IDE open both through the gateway within the rate budget; `pocol_getCaps` matches the state; the publisher refuses before signing at a near-full cap (cap-test build) | Fix the gateway caching or publisher fees. |
| S2 | Cap raise (cap-test build) | fill ACCT_CAP; `register` and new accounts fail; activate C10 at a height; both succeed; replay and reorg across the height are consistent | Fix C10; do not launch without it. |

## 5. Open owner questions

**Q1. Cap filling without a creation fee.**
- Under "ordinary gas only" (10d), one person can fill the **account** cap in about 14.6 h for about 0.21 PCOL.
- After that, nobody new can get an account, so nobody new can start mining, until a fork raises the caps. All caps fill in about 9.5 days for about 3.3 PCOL. The chain keeps running.
- Options (neither adds an account fee):
  - **(a) Recommended:** accept the risk. A ready-made cap-raise path (C10) ships at launch, an occupancy alarm warns at 80 %, and the risk is disclosed.
  - **(b)** Also raise the base-fee floor from 0.01 to 1 gwei. Filling the account cap would then cost about 21 PCOL and all caps about 330 PCOL. A transfer would cost about 0.00002 PCOL and a 4 MiB site about 1 PCOL plus drift.

**Q2. "Inactive members are removed after about one hour".**
- **(a) Recommended:** only when the network is full. A newcomer then takes, in one step, the seat of a member with no credited share in the last hour. The removed member gets its bond back after about an hour and may register again. Below a full set nobody is removed.
  - Note: at a full set of equal small miners, about a third of them have no share in a given hour, so each newcomer can replace one of them (up to 1,440 per hour).
  - This replace-in-one-step operation is not a seat contest: it measures no performance and only combines "remove an inactive member" and "register".
- **(b)** Automatic removal by the chain every block after 1 h. Rejected: at 10,000 small miners it would remove about a third of honest small miners every hour.
- **(c)** Like (a), but 1 h only for members that have **never** mined, and about 6 h for members that have. At a full set, about 0.1 % of active small miners would then be replaceable. This costs one extra parameter.

**Q3. The launch label.**
- **(a) Recommended:** launch the public network admitted the same way the old network was (the "experimental" admission). The software reports "not public-launch-ready" with its reasons, and we disclose them. Mining, sites and rewards are unaffected.
- **(b)** Before launch, rework the governance gates (continuous network profiles, retention bound at the large caps, specification scope) so the software admits it as a "network" profile. This is a large separate project and delays launch.

Everything else is decided by the owner's answers or is technical: chain ids, T_PROB and CP field deletion, R_max 4, Z_close 256, the cap values subject to A13, node-local timing, and the sites remaining hosted.

## Appendix: review findings and resolution

| # | Finding | Verified | Resolution |
|---|---|---|---|
| R1-1 | `Capacity` path not atomic (blocking) | Yes: `:490-494`, `:513`, `:476`, `:514-519` | C4(b) `registerReplacing`, in FN-2b before genesis; M3 tests same-block competition. |
| R1-2 | Θ 360 at M_max 10,000; `removeInactive` ungated | Yes: no capacity check at `:491-492`; about 31.6 % removable | C4(a) gate on `active == M_max`; numbers in §4a.6 and Q2; Q2(c) added. |
| R1-3 | Q1 understates the account fill | Yes: 0.21 PCOL, 14.6 h, blocks new registrants | Q1 leads with the account figure; C10 is a launch requirement; Q1(b) shows 21 PCOL. |
| R1-4 | V3-16 exposure grows with C2 | Yes: `invalidate` `:232-237`, `chain.rs:552` | C2 owner-rule note, `threat.md`, M4. |
| R1-5 | Gas envelope basis changes; `in_gas_domain` must be rewritten | Yes: `admission.rs:188, 217-220`; M_max > 256 fails R18 | C2: V3-only rewrite with the payee bound; M2; risk 9. |
| R1-6 | Literal CP indices missed | Yes: `:190 cfg(66)`, `:207 cfg(59)`, `:354-355`, `:54/57` | Listed in C2. |
| R1-7 | Wallet `Probation` uses | Yes (line numbers differ on d24843d) | C2/C3 list `wallet.rs` uses and `chain.rs:68-70`. |
| R1-8 | Arithmetic slips | Yes | SYS_SLOT_MAX tail 77 (137,091); Entry.ranges 2.8 GiB; `governance.md:13`. |
| R2-1 | R20 fails at Z_close 160 (blocking) | Yes: 199 > 160 (`param_gate.rs:83`, `gate.rs:351`) | Z_close 256; R20 in the checked list; C7 test. |
| R2-2 | C8 cannot admit Network/Node (blocking) | Yes: R13 `gate.rs:315-317`, ExperimentalDevnet `admission.rs:497-500`, Specification `:507-515`, R21 `gate.rs:352-355`, `pocold/src/lib.rs:248`, `readiness.rs:43` | C8 rescoped to option (a); (b) offered as Q3. |
| R2-3 | Dropping `m_max` must be V3-only | Yes: `admission.rs:184-188` | C2: V3 only; the spec-1 term goes in Phase 6a. |
| R2-4 | Lock notation | Yes: gate only on `neutral` (`consensus_boundary.rs:626`) | FN-2a/2b: one `neutral-vector` line, no gate. |
| R2-5 | Chain id timing and rehearsal replay | Yes | Two ids drawn before the rehearsal and recorded in FN-0. No compiled id under C8(a), so the rehearsal uses the same admission path with a distinct id. |
| R2-6 | Q2(a) misdescribes `removeInactive` | Yes | C4(a) gate makes it true, with its own owner-rule row. |
| R2-7 | In-flight branches missing | Yes: branches exist; N2 has a single K_SAFE | §3 "In-flight" row (N2 rework) and G3b'. |
| R2-8 | Rehearsal: offline > Θ+U; journal growth | Yes: `history.rs:5` | R-off and D1 steps; risks 7 and 11. |
| R2-9 | Site sizes inconsistent | Yes: IDE 20.6 MB with a 10.8 MB file; limits at `publisher.js:45-46` | The three sites stay hosted; S1 publishes the real explorer bundle; the CODE_CAP reasoning is fixed. |
| R2-10 | FN-2b misses CI consumers | Yes: `compile_system_v3.cjs:31,34,51,60,92`, `forks_test.mjs:82`, `pocol_rewards.py:458`, `gate.rs:447-449`, `v3_late_shares.rs:78`, `testnet.rs:286-287`, `chain.rs:68-70` | All listed in C2. |
| R2-11 | Entry.ranges overstated | Yes | 2.8 GiB. |
| R2-12 | Stale removal-plan text | Yes: `testnet.rs:262` | FN-0 amends it. |
| R2-13 | Readiness pin missing from Phase 10 | Yes | Added under Q3(b) only. |
| R2-14 | Full-set test infeasible via `testnet.rs` | Yes: 256 limit at `:257` | CI host-level 10,000-record genesis; rehearsal registration harness (M1). |