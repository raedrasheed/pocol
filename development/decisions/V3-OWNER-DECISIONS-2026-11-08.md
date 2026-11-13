# PoCol v3 — owner decisions (2026-11-08)

> **Superseded in part by the owner decisions of 2026-11-13**
> ([OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md](OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)): PoCol launches a brand-new network from a
> fresh genesis and keeps nothing related to the old network. This record stays as dated history. Affected items:
>
> - **O-0** (v3 rides the held earn-in re-genesis): superseded. v3 launches as a brand-new network from a fresh
>   genesis with new keys and a new chain ID; there is no re-genesis or cutover of the old chain (answers 1-5).
> - **O-6** (block time 30 s): changed. Difficulty adjustment stays, with a 10 s average block time (answer 10a).
> - **O-10** (contestable seats when the set is full): superseded. There is no seat contest; registrations are
>   rate-limited per block, and inactive members are removed after about an hour and may return (answers 10, 10b).
> - The member limit **M_max** (V3-07, 256): changed to **M_max = 10,000** (second follow-up, answer 2).
> - **O-14** (probation pool of M_max/4 slots with a contest inside the pool): superseded. There is no separate
>   probation pool: newcomers accruing their 20 PCOL bond count within M_max = 10,000. New miners register for free
>   and the first 20 PCOL of their rewards is held as their bond, forfeited on misbehaviour (answers 10, 10c; second
>   follow-up, answer 2).
> - **O-21** (raised state caps with a burned creation fee): the creation fee is removed; new accounts pay ordinary
>   gas only (answer 10d).
> - **O-32** (the 4,000,000 PCOL genesis allocation): superseded. The new genesis gives each of the four founding
>   miners 1,000,000 PCOL (answers 8, 9, 15).
> - **O-33** (how the re-genesis is run): moot. The old network is stopped and nothing carries over, so there is no
>   freeze height S, old-chain export or migration (answers 1-4).
> - **O-19** (issuance, M3): changed. S0 = 4 PCOL per block at the 10 s average block, halving every 4 years
>   (H_half = 12,623,040 blocks), with a tail of S0/32 = 0.125 PCOL (answer 7; second follow-up, answer 1).
>
> How these map onto the parameters and programs is implemented in later changes; until then the code still carries
> the 2026-11-08 values.

Status: **approved by the owner, Raed S. Rasheed, on 2026-11-08.** Approval of the design and its values is not
approval to deploy: the earn-in re-genesis and every live-network change remain on hold until the owner releases
them.

The full design, with the principle check of every item, is [V3-DESIGN.md](V3-DESIGN.md). It answers the
2026-11 readiness review of the consensus, economics, execution and decentralization findings.

## 1. The owner's rule for consensus changes (verbatim)

> أوافق على تعديل التي لا تغيير خوارزمية الاجماع بدون اي تحفظ. أما التي تحتاج تغيير في خوارزمية الاجماع فأوافق
> على التغيير الذي لا يغيير فكرة الاجماع بالتعاون ولا يغير فكرة الكتلة النموذج أو تغيير أي مبدأ من المبادئ
> الاساسية ل PoCol.

The rule has two parts:
- **Node-local fixes** do not change the consensus algorithm. They are approved without reservation.
- **Consensus changes** are approved only if they keep three things:
  - consensus by collaboration;
  - the template block, where one drawn proposer signs one template that all members mine on together;
  - every basic principle of PoCol (P1–P10 in V3-DESIGN.md §2).

Under this rule, items classed NODE-LOCAL and CONSENSUS-COMPLIANT in V3-DESIGN.md §1 are approved:
- V3-01, V3-02, V3-04, V3-07, V3-08, V3-09, V3-16, V3-23, V3-24 and V3-34;
- the node-local items.

V3-22 and V3-31 are deferred. V3-R is rejected.

## 2. Owner choices (the owner accepted every recommendation: «موافق على التوصيات»)

| Question | Item | Decision |
|---|---|---|
| O-0 Vehicle for v3 | — | **a**: v3 rides the held earn-in re-genesis, and the same release builds the in-place fork machinery for later changes |
| O-6 Block time | V3-06 | **b**: 30 s (D_att 150, dFbWait 10, Δ_rel 13), valid if the measured WAN Δ ≤ 2 s; otherwise 60 s (option c) |
| O-10 Contestable seats when the set is full | V3-10 | **b**: contest by work over one day with a statistical margin (θ=1/8, 3σ), only when all seats are full, with the displaced member refunded, and only together with V3-02 |
| O-11 Inactivity removal delay Θ_inact | V3-11 | **b**: 1 hour |
| O-12 Registration bond B_reg | V3-12 | **a**: 20 PCOL |
| O-13 Probationers as proposers | V3-13 | **b**: yes, once the accrued bond is at least B_reg/4 |
| O-14 Probation pool | V3-14 | **b**: M_max/4 slots (64), contest by work inside the pool, T_PROB = 30 days, both as CP fields (requires V3-09) |
| O-15 Uncovered share pot | V3-15 | **b**: carried into the next block's pot at closeBlock |
| O-17 Reward for reporting misbehaviour | V3-17 | **b**: 25% of the burned bond to the reporter, 75% burned |
| O-18 Evidence window and bond delay | V3-18 | **b**: 1 hour, with U = E_win + D + a margin |
| O-19 Issuance schedule | V3-19 | **M3**: M1's rate (2 PCOL per 60 s, scaled by T_blk) halving every 4 years, with a tail of S0/32 |
| O-20 Fee market | V3-20 | **b**: the proposer receives 50% of tips directly, the rest stays in the shared pot, base-fee floor 0.01 gwei; the pool becomes fee-ordered with replacement and eviction (node-local) |
| O-21 State caps | V3-21 | **b**: raise the caps (A13-verified) and add a burned creation fee that rises as the cap fills |
| O-32 The 4,000,000 PCOL genesis allocation | V3-32 | **b**: publish the holder addresses and their purpose, and move the development fund into a public time-locked account with published spending reports |
| O-33 How the re-genesis is run | V3-33 | **b**: no user transactions after S; publish the old-chain data; ship an Android key-export build first; and commit publicly that this is the last owner-only re-genesis, with later consensus changes going through the fork schedule |

## 3. Related owner statements (2026-11-08)

- **Seeds:** the three operator seeds will be stopped once peer-address discovery works.
- **Genesis allocation:** the 4,000,000 PCOL genesis allocation is meant to fund wider development of the network.
- **Android signing:** the owner will replace the Android signing key with a real key.
- **Audit:** an independent human security audit will take place after the real launch.

## 4. Implementation order

Each step is its own pull request, merged when CI passes:
1. this decision record;
2. the fork machinery;
3. the program-set registry and the parameter schema v2;
4. the host rules;
5. the v3 programs;
6. escrow and issuance;
7. the resource proofs;
8. the off-node verifiers;
9. simulations and test vectors;
10. the cutover rehearsal.

V3-DESIGN.md §3 gives the dependencies.
