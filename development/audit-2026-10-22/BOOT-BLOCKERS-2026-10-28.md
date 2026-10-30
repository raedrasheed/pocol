# What keeps the network profile from booting, by cause (2026-10-28)

[NETWORK-BOOT-GATES.md](NETWORK-BOOT-GATES.md) lists every refusal by layer. This file sorts the same refusals by
**cause**, so that a missing input is not read as a defect and a defect is not read as a resource limit:

- **A. No approved genesis or measured input.** Nothing in pocold is wrong. The network genesis and its measured
  calibration do not exist, and no code can supply them.
- **B. Implementation gaps.** The specification says what to do and pocold does not do it yet. Fixing them is
  conformance work, with no rule change.
- **C. Decisions not taken.** The owner's decision decides what the code must do.
- **D. Resource limits.** The computed bound or a budget refuses, for the profile as specified.

The values are re-measured on `1074b47` by `pocold/tests/network_budget.rs::network_profile_boot_gates_itemized`
(evidence/r4/boot/boot-gates.log). The CP is governance.md's network values. The chainId is 777920, because
`devnet::build` refuses 777001. The local values are governance.md's for a network node: Δ=5, φ=5, Φ=10,
t_exec=3 s, Δ_rel=26, Γ=30, K_hist=32.

## R22 and R21: the two rules that refuse, with their actual values

| rule | condition (source) | required | actual | cause |
|---|---|---|---|---|
| R22, profile layer | `ProfileIdentity::validate` (lp1-node/src/profile.rs:273): a node of kind Node needs a profile of kind Network, and kind Network ⇔ chainId 777001 | kind Network, node kind Node, chainId 777001 | pocold always requests kind Simnet, node kind Simnet (`pocold::load_spec_for`, src/lib.rs:188); no genesis with chainId 777001 can be built | **A** (no network genesis) and **B** (no code path asks for a network admission) |
| R22, admission layer | `admission::admit`, experimental-devnet scope (lp4-execution/src/admission.rs:283): only a simnet profile on a simnet node | Scope::Specification for a network node | Scope::ExperimentalDevnet, hard-coded in `load_spec_for` | **B** |
| R21 | (K_hist + side_max) whole snapshots at ACCT_CAP+5 accounts and SLOT_CAP+SYS_SLOT_MAX slots ≤ HIST_TRIE_BUDGET; the only exception is a simnet profile with retentionStress (governance.md R21; gate.rs) | ≤ 1,717,986,918 B | 59,619,697,068 B (34.7×) | **D**, for whole-state snapshots |
| R21, store mode | what the 32 versions behind the live one can add, at most 32 block additions | ≤ 1,717,986,918 B | 26,269,409,781 B: the 32 versions' additions 24,267,648,672 B and the live version 2,001,761,109 B (the live version alone exceeds the budget) | **D** as counted; **B**: the spec's runtime mechanism for this case is K_eff (validation L12, consensus.md:230, storage.md:70), which pocold does not implement |

R14 and R19 do not refuse: the calibration's phases took 7, 9 and 11 ms against t_exec = 3,000 ms on this run (the
H_END measurement ran beside it). They time one compute loop, not the slowest valid blocks, so the pass is not a
bound (condition `timing-not-a-bound`, decision row 2); the slowest measured blocks are in evidence/r4/README.md § 6.

## Every refusal, by cause

### A. No approved genesis or measured input (no code defect)

| refusal | what is missing | who supplies it |
|---|---|---|
| R22: no network genesis | the network genesis artifact: alloc, M0 list, g_ts, and target_g from a measured H_cal | the owner (governance); then `devnet::build` or a network builder can produce chainId 777001 |
| R25 `calibration-synthetic` | a measured hash rate for the network's PoW (H_cal). The devnet's `--work-bits` fixture is synthetic | a measurement on the target hardware, approved as an input |

### B. Implementation gaps (conformance; no rule change)

| refusal | gap | size |
|---|---|---|
| R22 | `load_spec_for` has no network path (kind, node kind and scope are hard-coded) | small once A exists |
| R21 | K_eff: when the retained versions exceed HIST_TRIE_BUDGET, K_eff falls and what is outside it answers −32017 | the largest piece of work the list implies; not started |
| R26 `rpc-absent` | the RpcGuard mechanisms pocold does not enforce: scratch_heavy, scratch_light, mem_total, cancel_max_ms (RPCGUARD.md) | medium; cancel_max_ms depends on decision P2 |
| R18, R21 `gate-deferred-*`, `gas-monotonicity-assumed` | the general R18/R21 derivations, the caps64 minimum of SYS_SLOT_MAX, and gas monotonicity in the domain fields: LP3 obligations deferred to LP4 | proofs, not code |
| (not a gate) the journal at open | open held every journal payload (condition `open-replay-transient`, about 1.0× the journal). At H_END the network journal reaches about TXBYTES_CAP (8 GiB) plus headers, so this transient alone could exceed the budget | **fixed in this round** (the journal is streamed at open); evidence/r4/README.md § Open memory |

### C. Decisions not taken (proposals/OPEN-DECISIONS.md)

| refusal | decision | what it unblocks |
|---|---|---|
| R14/R19 `timing-not-a-bound` | the R14/R19 calibration workload (row 2) | the Specification-scope condition: the gate would time the slowest blocks, not one compute loop |
| R26 (cancel_max_ms and rate) | RpcGuard P1–P4 (row 7) | the mechanisms B lists, with their chosen behaviour |
| no admissible network caps regime | D-3 / D-5 (rows 3, 5) | O2 needs R27–R29 and new caps in the network genesis; D-5 changes Hello |

### D. Resource limits (computed bounds against budgets)

| check | computed | budget | result |
|---|---:|---:|---|
| A13, whole-state mode (the default) | 26,819,504,117 B model; 30,650,861,848 B required | 6,442,450,944 B (6 GiB) | **refused** |
| A13, store mode (`--state-store`) | 5,581,321,485 B model (5.198 GiB); 6,378,653,126 B required | 6,442,450,944 B | passes, computed |
| R21 (above) | 59,619,697,068 B / 26,269,409,781 B | 1,717,986,918 B | refused |

After 719da04 (the chain's side set, counted at 32 + 16 B per side entry, evidence/r4 § 4) the store-mode model is
5,581,616,397 B (5.198 GiB), 6,378,990,168 B required, and the whole-state model 26,819,799,029 B, 30,651,198,891 B
required; the verdicts are unchanged.

The model cap and the budget are one condition: A13 requires model × 8/7 ≤ budget, that is model ≤ budget × 7/8 =
5.25 GiB for a 6 GiB budget. The 1/8 headroom is for what the model does not count (allocator fragmentation,
per-allocation headers). The store-mode model is 5.198 GiB against that 5.25 GiB cap (0.05 GiB to spare; at the O2 caps of decision row 3 it is 5.231 GiB, evidence/r4/boot/store-model-o2.log). What a node measured
is a different quantity: evidence/r4/README.md § Memory.

## What booting the network profile would take

1. A: the network genesis and a measured H_cal, approved by the owner.
2. C: decisions rows 2, 3, 5 and 7.
3. B: a network admission path in `load_spec_for`, K_eff (R21), the RpcGuard mechanisms (R26), and the deferred
   derivations.
4. D: store mode (the whole-state model does not fit), which is decision row 9 if it is to be the default.

None of these is done by this round except the journal streaming in B. The network profile does not boot.
