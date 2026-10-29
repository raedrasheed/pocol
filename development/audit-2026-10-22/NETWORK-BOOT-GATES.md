# Why the network profile does not boot (2026-10-27)

Measured by `pocold/tests/network_budget.rs::network_profile_boot_gates_itemized` (evidence/memory-r14/network-gates.log).
- **CP:** governance.md's genesis parameters (`network()` in that file). The chainId is 777920 only because the
  builder refuses 777001, and the chainId enters no gate except R22.
- **Local parameters:** governance.md's for a network node: Δ=5, φ=5, Φ=10, t_exec=3, Δ_rel=26, Γ=30, K_hist=32,
  no retentionStress.
- **RPC facts:** pocold's own.
- **Timing:** measured on this host (4 vCPU KVM).
- **Checked against the gate:** the R1–R26 table is re-derived from the rules' formulas
  (lp1-node/src/gate.rs `check`). The test checks it against the gate's own verdict, so the two cannot drift.

The refusals come in four layers. A network boot needs every layer cleared.

## 1. Structural: no code path boots a network profile

| what | where | why it refuses |
|---|---|---|
| The admission request is always a simnet profile on a simnet node, in experimental-devnet scope | `pocold::load_spec_for` (src/lib.rs) | R22 requires kind = Network, chainId = 777001 and node = Node. The experimental-devnet scope admits only simnet on simnet. A network node needs `Scope::Specification` |
| No network genesis can be built | `devnet::build` refuses chainId 777001 | There is no network genesis artifact in the repository: alloc, M0 list, g_ts, and a target_g from a *measured* H_cal |

## 2. ParamGate R1–R26 with the network values

Only **R21** fails. Every other rule the test re-derives passes. The rest of R1–R26 pass too in `admit`'s
verdict, except R26, which this scope waives.

| rule | inputs | required | actual | result |
|---|---|---|---|---|
| R2 | Δ_rel, σ=14, Δ, t_exec | Δ_rel ≥ σ+Δ+2·t_exec = 25 | 26 | pass |
| R3 | dFbWait, Φ, Δ, t_exec | dFbWait ≥ Φ+Δ+2·t_exec = 21 | 25 | pass |
| R4 | Φ, dFbWait, Δ_rel, t_exec | Φ < dFbWait, Δ_rel ≥ t_exec | 10 < 25, 26 ≥ 3 | pass |
| R5 | dFbWait, D_att | 3·dFbWait ≤ D_att = 300 | 75 | pass |
| R7 | D_att, Γ, X^W=36, c, T_blk, Φ | D_att+Γ ≥ 2(X^W+c·T_blk)+Φ = 322 | 330 | pass |
| R8 | D, U, E_win, R_keep | D ≥ 2, U ≥ E_win+D = 2064, E_win ≤ R_keep | 16, 2200, 2048 | pass |
| R9 | S_max, κ, m, B_code_max | S_max ≥ 4κm = 256, B_code_max ≥ 24577 | 256, 32768 | pass |
| R10 | BODY_MAX, TX_MAX | ≥ TX_MAX+16384 = 147456 | 196608 | pass |
| R13 | H_END, H_CLOSE, Z_close, T_CLOSE, T_END | H_CLOSE+Z_close ≤ H_END, … | 129600 ≤ 129600 | pass |
| R14 | validate_p99 (the calibration workload's largest phase) | ≤ t_exec = 3000 ms | 16 ms | pass |
| R16 | REG_RECORDS_MAX and the registry inputs | ≥ records_required = 12578 | 16384 | pass |
| R17 | SWEEP_MAX, R_max, E_max | ≥ 6 | 16 | pass |
| R19 | each phase's measured time | ≤ 3000 ms | 9, 16, 14 ms | pass |
| R20 | Z_close and the registry inputs | ≥ close_blocks_required = 147 | 160 | pass |
| **R21** | (K_hist+side_max) whole snapshots at ACCT_CAP+5 accounts, SLOT_CAP+SYS_SLOT_MAX slots | ≤ HIST_TRIE_BUDGET = 1,717,986,918 B; the only exception is simnet with retentionStress | **59,619,697,068 B** | **fail** |
| R25 | H_cal, T_blk, target_g | target_g = 2^256/(H_cal·T_blk) | equal | pass, but H_cal is synthetic (layer 3) |

**R21 in store mode does not fit either.**
- The gate's input models whole-state mode, which keeps full states.
- In store mode the retained versions share nodes in state.redb. What the 32 versions behind the live one can
  add is still at most 32 block additions = **24,267,648,672 B**. The live version itself is 2,001,761,109 B.
  This is above the budget too.
- The spec's mechanism for this case is runtime, not a count. Validation scenario L12: "dense state change
  until HIST_TRIE_BUDGET is exceeded. K_eff falls, and what is outside it gives −32017". consensus.md:230 and
  storage.md:70 say the same.
- **pocold does not implement K_eff.** It always keeps K_hist versions. R21 cannot pass at the network
  profile until it does. Its admission fact would then be the enforced budget, not a count.
- Implementing K_eff is conformance to the spec, not a rule change. It is the largest piece of work this
  list implies, and it is not done here.

## 3. Specification-scope conditions

A network node is admitted in `Scope::Specification`. There, each of these reasons why an admission is not
specification evidence is a refusal, with its rules:

| condition | rules | what it means | what would clear it |
|---|---|---|---|
| rpc-absent | R26 | RpcGuard mechanisms pocold does not enforce: scratch_heavy, scratch_light, mem_total, cancel_max_ms (RPCGUARD.md) | enforce them (RpcGuard P1–P4 are decisions) |
| calibration-synthetic | R25 | H_cal is the devnet's `--work-bits` fixture | a measured hash rate for the network's PoW |
| timing-not-a-bound | R14, R19 | the timing is the calibration workload (one compute loop), not a p99 or worst case | a worst-case workload (decision row 2; MODEXP, A13-DERIVATION §3b) |
| gas-monotonicity-assumed | R18 | SYS gas holds at the fixture's maxima; monotonicity in the domain fields is assumed | a proof or the general derivation |
| gate-deferred-general-R18, -general-R21, -caps64 | R18, R21 | LP3 obligations deferred to LP4 (the general R18/R21 derivations, the caps64 minimum of SYS_SLOT_MAX) | those derivations |
| r21 (bound above the budget) | R21 | as in layer 2 | K_eff |

**Timing at the network profile.** t_exec is 3 s, not the devnet's 1 s. When this was written, the slowest
block measured on this host was MODEXP with an even modulus, at 1.2–1.6 s, which is within 3 s. After the two
MODEXP fast paths (`b46658a`, `0c2ad75`; A13-DERIVATION §3b-2) the slowest measured is about 0.51 s. The
condition stays, because the timing is still not a bound.

## 4. pocold's A13 memory check (`resources::check` at open)

| mode | model | budget required (model ÷ 7/8) | budget | result |
|---|---:|---:|---:|---|
| whole-state (the default) | 26,819,504,117 B | 30,650,861,848 B | 6,442,450,944 B | **fail** |
| store (`--state-store`, opt-in) | 5,581,321,485 B (5.198 GiB) | 6,378,653,126 B | 6,442,450,944 B | pass |

This is the computed bound, not measured memory. Store mode is opt-in, and making it the network default is
decision row 9. (2026-10-27: 5,460,970,545 B before `5abbaf3`'s two stored-version handles and the deferred
checkpoint release's owed checkpoint set and release rows; A13-DERIVATION §3c.)

## Devnet constraints, kept apart

None of these applies to the network profile. They are why the devnet boots only in experimental-devnet
scope:
- **Locals:** the single-host values (Δ=1, φ=0, t_exec=1 s, Δ_rel=5, Γ=5). At t_exec = 2 s the devnet fails R2,
  R3 and R7 unless dFbWait and D_att, which are CP fields, change (D-2, evidence/memory-r14/d2-gate.log).
- **R21:** passes only by the simnet retentionStress exception.
- **R25:** the synthetic calibration.
- **R26:** the waiver of the RpcGuard fields pocold does not enforce.
