# Fresh PoCol network: the final consensus parameters and genesis contents (FN-0)

Status: **record of decided values, 2026-11-14.** Every value below comes from the owner decisions of 2026-11-13
([OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md](OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md), with both follow-ups and
the answers Q1-Q4 = a) and from the fresh-network consensus design (FRESH-NETWORK-DESIGN.md, the handoff design kept
outside this repository, §0-§1 and rows C1-C10). This record does not deploy anything. The genesis is made only at
the ceremony, after the rehearsal, and only with the owner's approval.

The values are committed as data in two network profiles. The genesis builder reads them (`pocold init-testnet`,
`prototypes/pocold/src/testnet.rs`, FN-4):

- `deploy/mainnet/v3-profile.json`: the mainnet profile.
- `deploy/mainnet/v3-rehearsal-profile.json`: the closed-rehearsal profile. It differs from mainnet only in its chain
  id and its launch second.

`prototypes/pocold/tests/fn4_profiles.rs` checks both profiles against this record and through the full ParamGate.

## 1. Decisions the design makes (design §0)

| Point | Value | Why |
|---|---|---|
| Inactivity removal | Θ_inact = 360 blocks (1 h), enforced only when a seat is needed: `removeInactive` is gated on `active == M_max`, and `registerReplacing(reward, victim)` atomically registers the sender and exits a removable victim. There is no automatic sweep. | Owner 10b; Q2 = a. Implemented in FN-2b (C4). |
| R_max | 4 | 1,440 registrations per hour. Honest throughput matters more when seats are contested. |
| Dead CP fields | 8 deleted: P_MAX, T_PROB, F0_acct, F0_slot, F0_code, E_epoch, theta_bp, k_sigma | No chain ran CP schema v2. Deleted in FN-2a and FN-2b; CP schema v2 has 59 fields. |
| Founders' bond | Each founder gets 999,980 PCOL. The genesis builder mints its 20 PCOL HELD bond into Escrow. | The initial supply is exactly 4,000,000 PCOL. |
| Caps | ACCT 1,000,000 / SLOT 4,000,000 / CODE 2^30, subject to A13 store mode. Fallback: CODE 2^29. | Base-spec values (`development/m1/reference/governance.md`). |
| Sites | explorer, studio and ide stay HTTPS-hosted. Any user site up to the publisher's 4 MiB / 1 MiB-per-file limit is published on chain. The explorer bundle is the rehearsal's reference site. | The IDE bundle (20.6 MB, one 10.8 MB file) is above the publisher limits. |
| Node-local timing | Declared (Δ 1, φ 1, t_exec 1), Δ_rel 7. Fallback (2, 1, 1), Δ_rel 8. | At dFbWait 8, (2, 2, 2) fails R3. The same CP admits both declarations. |
| Launch admission | Simnet profile, Simnet node, ExperimentalDevnet scope, as the retired network was admitted. R22 stays bound to 777001. `publicLaunchReady` stays false and its blockers are disclosed. | Q3 = a. R13 forces a continuous profile to Simnet/Simnet. R21 holds at these caps only through the simnet `retentionStress` exception. |

## 2. Consensus parameters: CP schema v2, 59 fields (in genesisHash; design §1a)

Fields 0-51 are the v1 fields, unchanged in order. Fields 52-58 are the v3 fields. "Ceremony" marks a launch input that
the genesis ceremony supplies; it is not a literal in the profile.

| # | Field (profile name) | Value | Reason |
|---|---|---|---|
| 0 | g_ts (`genesisTime`) | ceremony: the synchronized launch second | ASERT is anchored absolutely. All 4 founders must be mining within D_att of g_ts (T8). |
| 1 | target_g (`hashRate`) | ceremony: `calibration_target(H_measured, 10)` | R25 with a **measured** class (`hashRate.class = "measurement"`); the builder and `pocold/src/lib.rs` carry the class to admission. |
| 2 | T_blk | 10 | Owner 10a. Gate-valid at measured Δ ≤ 2 s and φ ≤ 1 s. |
| 3 | tau | 600 (60 blocks) | R25 needs τ ≥ 100. Re-converges in about 75 blocks after a ±25 % step. |
| 4 | nonceMode | 1 | Unbounded ranges (D-V3-01). |
| 5 | c | 2 | The R7 term is 60-64 s, below D_att. |
| 6 | D_att | 90 | Empty a ≥ 1 blocks fall from about 0.96 % to about 0.06 %. Passes R5 and R7. |
| 7 | dFbWait | 8 | R3 holds up to (Δ2, φ2, t1) and (Δ2, φ1, t2). |
| 8-10 | m / kappa / S_max | 32 / 2 / 256 | κm = 64 and S_max ≥ 4κm (R9). These bound every payee loop: at most S_max + 3 payees per pot. |
| 11 | D | 4 | 40 s settlement and late-share window. |
| 12 | W_w | 8 | Draw window. |
| 13-14 | alpha_bp / gamma_bp | 1000 / 500 | Unchanged. |
| 15 | subsidy (S0) | 4·10^18 wei (4 PCOL) | Owner, second follow-up: 34,560 PCOL per day, 12,623,040 PCOL in year 1. |
| 16 | B_reg | 20·10^18 wei | Owner 10c (the bond earned from mining). |
| 17 | R_max | 4 | §1. The profile's `registrationPerBlock` must equal it. |
| 18 | M_max | 10,000 | Owner, second follow-up, answer 2. |
| 19 | M_min | 1 | Unchanged. |
| 20 | REG_RECORDS_MAX | 24,576 | R16: 10,000 + 4·(725 + 2048) = 21,092 ≤ 24,576. |
| 21-22 | SWEEP_MAX / SWEEP_SCAN | 16 / 64 | ≥ R_max + E_max (R17). |
| 23 | REG_OPS | 16 | `registerReplacing` uses 2 units and 1 R_max entry. |
| 24 | W_act | 32 | 5.3 min. |
| 25 | Theta_inact | 360 (1 h) | Owner 10b. Enforced only at a full set. |
| 26 | E_win | 360 | O-18 = b (one hour). |
| 27 | U | 380 | ≥ E_win + D = 364 and ≥ W_act + 2. |
| 28-29 | E_max / EVID_BYTES_MAX | 4 / 8192 | Unchanged. |
| 30 | GAS_LIMIT | 8,000,000 | One full site chunk per block; t_exec ≤ 1 s (T3). |
| 31 | baseFee0 | 10^9 wei | ≥ baseFeeMin. |
| 32-34 | TX_MAX / BODY_MAX / B_code_max | 131,072 / 196,608 / 32,768 | Unchanged. R10 holds. |
| 35 | CODE_CAP | 2^30 | Fallback 2^29 if A13 fails. |
| 36-37 | TXBYTES_CAP / TX_CAP | 2^24 / 20,000 | Per block when H_END = 0. |
| 38 | SLOT_CAP | 4,000,000 | Base spec. |
| 39 | ACCT_CAP | 1,000,000 | Base spec. |
| 40 | SYS_SLOT_MAX | 147,456 | slot_min ≈ 6 + 5·24,576 + 10,000 + 8·516 + 77 = 137,091 (the tail 77 uses the 59-field CP). |
| 41-44 | SYS_KEYS_BLOCK_MAX / SYS_KEYS_USER_MAX / SYS_ACCT_BLOCK_MAX / G_SYS | 4096 / 256 / 512 / 96,000,000 | Re-verify R18 after the envelope is re-measured (Phase 6a). |
| 45-48 | H_END / T_END / H_CLOSE / T_CLOSE | 0 | Continuous chain. A v3 profile may state these zeros; the builder refuses any other value. |
| 49 | Z_close | 256 | R20: Z_close ≥ ⌈REG_RECORDS_MAX / RET⌉ + P2 + D + 1 = 192 + 2 + 4 + 1 = 199. The old value 160 fails. Unused while H_END = 0. |
| 50-51 | RET / P2 | 128 / 2 | Unchanged. RET cannot rise above 128 (the gas domain). |
| 52 | H_half | 12,623,040 | 4 years × 8,640 blocks per day. |
| 53 | S_tail | 1.25·10^17 wei (S0/32) | The tail starts exactly at era 5. The builder checks S_tail = S0/32. |
| 54 | gamma_tip_bp | 5000 | O-20 = b. |
| 55 | baseFeeMin | 10^7 wei | O-20 = b; Q1 = a keeps the floor. |
| 56 | R_react | 4 | Unchanged. |
| 57 | C_react | 380 | ≥ U. |
| 58 | rho_bp | 2500 | O-17 = b. |

ParamGate R1-R26 admits this CP at both declared locals, (1, 1, 1) and (2, 1, 1), for both chain ids. The design
checked R2, R3, R5, R7, R8, R9, R10, R16 and R20 by hand. `fn4_profiles.rs` asserts R2, R3, R5, R7, R16 and R20, and
the admission itself. R18 rests on FN-2b's V3-only gas domain. R21 rests on the simnet exception (§4).

Issuance (M3, `lp4-execution/src/issuance.rs`): issuance(h) = max(S0 >> ⌊h / H_half⌋, S_tail) for h ≥ 1.

| h | issuance(h) |
|---|---|
| H_half − 1 = 12,623,039 | 4 PCOL |
| H_half = 12,623,040 | 2 PCOL |
| 5·H_half − 1 = 63,115,199 | 0.25 PCOL |
| 5·H_half = 63,115,200 and after | 0.125 PCOL (the tail) |

The future bound is 2·S0·H_half + S_tail·2^64 = 2,305,843,009,314,678,272·10^18 wei.

## 3. Genesis contents (not CP fields; design §1b)

- **Chain ids.** Section 5 gives the two ids. No release compiles either id in, so both go through the same admission
  path.
- **Founders.** Four (owner answers 9 and 15): the three US servers and the owner's PC. Each generates fresh keys on its
  own device at the ceremony. The phone is a wallet only.
  - Each founder has a live HELD record with a 20 PCOL bond, minted into Escrow.
  - Each founder's reward address gets 999,980 PCOL and nonce ≥ 1 (PAY-02).
  - The builder checks allocation + B_reg = 10^24 wei for every founder. It also checks that the founders are four
    and distinct, and that they are the only members and the only allocation.
  - In the committed profiles, each founder's `id` and `reward` are `PLACEHOLDER: ...` strings.
- **Initial supply.** 4,000,000 PCOL.
- **Supply bound.** 4·10^24 + future bound = 2,305,843,009,318,678,272·10^18 wei, about 2.3058·10^36. That is 121
  bits, below the 127-bit limit (genesis_builder, R23). The builder records it in devnet.json under `supply`.
- **Programs.** The V3 program set only, with ChunkFactory at `0xC0C005`. ForkSchedule is `[(1,0)]` and
  MAX_FORK_VERSION is 3 (fork 2 a no-op, fork 3 the dormant cap raise). The cap-raise path (C10, FN-5) is compiled in
  and dormant.

**Placeholders.** Every value the ceremony supplies is a string that starts with `PLACEHOLDER`: `genesisTime`,
`hashRate.hashesPerSecond` and each founder's `id` and `reward`. The builder lists every placeholder and refuses to
write a genesis while any remains. The single exception is the explicit test flag
`pocold init-testnet ... --test-placeholders`, which fills them with:

- the current second;
- a synthetic hash rate, whose class is set to `synthetic-fixture`;
- public deterministic test keys.

The flag also replaces the profile's chain id with the local test id 777990 (`TEST_CHAIN_ID`, a 7779xx devnet id that
the FN-0 draws exclude) and records the profile's id as `profileChainId`. A test chain therefore never signs under the
mainnet or rehearsal id, which keeps the §1b separation (EIP-155 binds only the chain id). A genesis built with the flag
is marked `testPlaceholders: true` and "TEST GENESIS" in its devnet.json and summary. It is never a launch genesis.

## 4. Node-local values and admission (outside genesisHash; design §1c)

- Δ / φ / t_exec = 1 / 1 / 1, so Δ_rel = 7 and Γ = 5. This is the profiles' `localTiming` and the builder's v3 default
  (`TimingAssumptions::V3_DEFAULT`). If T1 measures more than 1 s, declare (2, 1, 1): Δ_rel 8, Γ 5, the same CP.
- R_KEEP 2,048 (5.7 h); K_HIST 32; K_FINAL / K_VIEW 60 / 12.
- `safe` at 6 blocks and the reorg alarm at 60 (owner answer 12).
- NTP offset ≤ 0.5 s on every founder, including the Windows PC through a configured w32time peer.
- Admission: a Simnet profile on a Simnet node in ExperimentalDevnet scope, with `retentionStress`. R21's input is far
  above `HIST_TRIE_BUDGET` at these caps; the resident bound is A13 store mode, to be measured in M1. The calibration class
  is measured.

## 5. Chain ids (drawn 2026-11-14)

| Network | Chain id | Hex |
|---|---|---|
| Rehearsal | **1656550731** | 0x62bcf54b |
| Mainnet | **2656089850** | 0x9e50b6fa |

Both ids were drawn on 2026-11-14 with Python's CSPRNG (`secrets`), uniformly in [2^24, 2^32). The command was:

```sh
python3 -c "import secrets; print(2**24 + secrets.randbelow(2**32 - 2**24))"
```

It gives one value per id. The rule is to draw again when a value hits the excluded set:

- 777001 (R22);
- 1599846715 (the retired network);
- 261597237 (the earlier public testnet);
- any 7779xx (local devnets);
- 1234567891 (the extension's placeholder);
- the other id.

Both values were re-verified on 2026-11-14:

- each is in [16,777,216, 4,294,967,296);
- neither is in the excluded set;
- they differ, so rehearsal transactions cannot be replayed on mainnet (EIP-155 binds only the chain id).

`fn4_profiles.rs` re-checks all three on every run and reads the retired id from `deploy/mainnet/retired-networks.json`.

## 6. Owner-rule checks

The owner rule passes if a change leaves intact collaborative consensus (shared shares and rewards), the template block
(a proposer-signed immutable template and a header-only solve) and P1-P10. None of C1-C10 touches the template, the
share and draw mechanics, fork choice or reward splitting.

| Change | Check |
|---|---|
| C1: delete the creation burn and F0_* (FN-2a) | Owner 10d; a fee rule only. **Pass.** |
| C2: delete the seat contest and the probation pool (FN-2b) | Owner 10b. Removal reasons shrink to exit, inactivity and punishment. ACCRUING, the earned bond and the B_reg/4 draw exclusion stay. **Pass.** The V3-16 exposure without P_MAX is disclosed and measured (M4). |
| C3: delete T_PROB (FN-2b) | Owner 10c. **Pass.** |
| C4: inactivity only at a full set, atomically (FN-2b) | Owner 10b; Q2 = a. Not a seat contest: no performance measure and no threshold. **Pass.** |
| C5: `activeSlice` view (FN-2b) | View only. **Pass.** |
| C6: node memory and CPU at 10,000 members (FN-1, FN-3) | No rule change. **Pass.** |
| C7: parameter plumbing and builder (FN-4) | Parameters and tooling: the profiles, the builder's checks, the measured calibration class and the v3 local-timing default. No rule, program or verdict changes. **Pass.** |
| C8: launch admission and identity (FN-7) | Governance labels only; R22 unchanged. **Pass.** |
| C9: `pocol_getCaps` (FN-1) | Node-local. **Pass.** |
| C10: cap-raise path (FN-5) | D-V3-23, already approved within the owner rule. **Pass.** |

## 7. Superseded and changed owner items

These are the O-items of [V3-DECISIONS.md](V3-DECISIONS.md) and
[V3-OWNER-DECISIONS-2026-11-08.md](V3-OWNER-DECISIONS-2026-11-08.md) that the 2026-11-13 answers replace. The final
values above take their place.

| Item | Was | Now |
|---|---|---|
| O-6 (block time) | b: 30 s (D_att 150, dFbWait 10, Δ_rel 13) | Changed (answer 10a): T_blk 10, D_att 90, dFbWait 8, declared (1, 1, 1) with Δ_rel 7, fallback (2, 1, 1). |
| O-10 (contestable seats) | b: a contest by work over one day | Superseded (answers 10, 10b): no seat contest. Θ_inact 360 applies only at a full set, through `registerReplacing`. |
| O-14 (probation pool) | b: M_max/4 slots, T_PROB 30 days | Superseded (answers 10, 10c; second follow-up, answer 2): no pool and no deadline. ACCRUING members count within M_max = 10,000. |
| O-19 (issuance) | M3 at 2 PCOL per 60 s, scaled by T_blk | Changed (answer 7; second follow-up, answer 1): S0 4 PCOL per 10 s block, H_half 12,623,040, S_tail S0/32. |
| O-21 (state caps) | b: ACCT 200k / SLOT 500k / CODE 32 MiB with a burned creation fee | Changed (answer 10d): no creation fee. Caps ACCT 1,000,000 / SLOT 4,000,000 / CODE 2^30 (Q1 = a: the cap-raise path and an occupancy alarm instead). |
| O-32 (the 4,000,000 PCOL allocation) | b: four public holders and a development fund | Superseded (answers 8, 9, 15): 1,000,000 PCOL to each of the four founders, including its 20 PCOL bond. |
| O-33 (how the re-genesis is run) | b: freeze height S, export and migration | Moot (answers 1-4): a brand-new network, with nothing carried over. |

## 8. Removal-plan amendments

The removal plan is a handoff document that is not in this repository, so its text is not edited here. The design asks
for two amendments, recorded here:

- **Genesis ceremony.** The line "Run `pocold init-testnet` with the v3 config: 4 founding miners, `accounts` of 10^24
  wei each, `F0_*` per decision, `P_MAX`/`M_max`/`T_blk` per decision, and the new chain id" goes. The replacement:
  1. Fill the ceremony placeholders of `deploy/mainnet/v3-profile.json`: the launch second, the measured aggregate hash
     rate, and each founder's public miner id and reward address.
  2. Run `pocold init-testnet --config v3-profile.json --out NEW_DIR`, without `--test-placeholders`.
  3. Run `launch_delay_asert.rs` on the final CP.

  There are no F0_* or P_MAX fields any more. The founders' allocation is 999,980 PCOL plus the 20 PCOL HELD bond, not
  `accounts` of 10^24 wei.
- **Phase 7.** `registrationPerBlock` is kept. It is R_max (`testnet.rs`), and a profile must state it equal to the
  CP's R_max.

## 9. Left to later steps

- **FN-7 (C8): done.** `deploy/standalone/Initialize-PoColNetwork.ps1 -Network Rehearsal|Mainnet` fills the
  committed profile (shipped and checksummed in the Windows package) instead of building a config: the launch second,
  the measured hash rate (`-HashesPerSecond`) and the four founders (the three servers given by `-Servers`, then the
  owner's PC, each a fresh key on its own device; founders 1-3 have reward = the owner-held -ServerRewardAddresses, founder 4 id = reward). The chain id (§5) and `registrationPerBlock` = R_max
  are the profile's; the script refuses a profile whose id is not the one of `-Network`, never passes
  `--test-placeholders` outside its `-DryRun`, parses init-testnet's summary (experimental-devnet scope,
  `publicLaunchReady` false with `admission-scope` and `profile-kind`) and prints every launch blocker. The seeds are
  the servers on TCP 30300. The launch disclosures are drafted in `deploy/standalone/README.md` for owner review.
  Tests: `deploy/standalone/test_initialize_network.py` and, on Windows CI, `test_initialize_network.ps1` (the dry
  run through pocold init-testnet --test-placeholders, and the refusals). Decided 2026-11-16 (owner): each server founder's
  reward is a separate owner-held offline address, so no allocation sits on an internet-facing server.
- **The launch evidence** (design §4b: T1-T8, M1-M4, R-off, D1, S1, S2) is measured in the rehearsal, on the release
  candidate, at this CP with the rehearsal chain id. The thresholds, the risks they read (design §4a) and the L1
  drills are now in the repository: [REHEARSAL.md](../public-network/REHEARSAL.md). A remedy that changes a consensus
  parameter changes only profile values (such as D_att, dFbWait, GAS_LIMIT, BODY_MAX, τ, S_max or κm, and T_blk
  20-30 s as a last resort; REHEARSAL.md names them per measure), with the owner's approval, recorded here, and both
  profiles change together.
- **Phase 10.** The ceremony's genesis hash is pinned in the clients.
