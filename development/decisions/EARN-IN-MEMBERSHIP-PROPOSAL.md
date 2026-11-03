# New PoCol network with earn-in membership and a 20 PCOL bond

Status: **approved by the owner on 2026-11-01.** Nothing is implemented or launched yet. The live
network keeps running until the new one is built, tested and rehearsed (N4).

Request (owner, 2026-11-01): let a miner without PCOL join, through probationary membership funded by
its own rewards, with the bond lowered to **20 PCOL**. Then the owner chose to launch these rules as
a **new network from a new genesis** and to stop the current live network, rather than upgrade it in
place.

## 0. Approved decisions

| ID | Decision |
|---|---|
| E1 | Bond `B_reg` = 20 PCOL |
| E2 | Probation seats `P_max` = 8 |
| E3 | Probation deadline `T_prob` = 60,480 blocks (7 days at `T_blk` = 10 s) |
| E4 | At the deadline, the accrued bond is refunded to the miner |
| E5 | Probationers are left out of the proposer draw |
| E6 | `topUp()` allowed, by the identity itself |
| E7 | Old bonds of 100 PCOL return to their owners (section 4) |
| E8 | `reward == msg.sender` is durable for `register` |
| N1 | Balances move to the new network (section 4) |
| N2 | New chain ID |
| N3 | Genesis miners: the three current server identities, each with a 20 PCOL bond |
| N4 | The live network stops only after the new one is ready; the owner or operator stops it |
| A1 | Work goes in phases, each in its own pull request; PR #16 and #17 are reviewed first |
| A2 | A small fee faucet; an operator rehearsal before launch |
| A3 | Old Android installations cannot follow the new network (Android stays deferred) |
| A4 | Server, wallet and extension deployments are done by the owner, one approval each |

E9 (an activation height `H3`) no longer applies: the rules hold from block 0. The code-replacement
mechanism the in-place upgrade needed is also dropped.

## 1. Problem

On the current network a new identity must lock `B_reg` = 100 PCOL when it calls
`register(address)` (`MinerRegistry` fallback in
`prototypes/lp4-execution/contracts/system/PoColSystemV1.sol`). Without a record an identity is not
in `A_h`, gets no nonce range, and earns nothing. A miner who holds no PCOL has no way in.

The bond does three things (threat model A6, `development/m1/reference/threat.md`):
- it prices each identity, so the active seats (`M_max` = 64) and registry capacity cannot be filled
  for free;
- `punish` burns it on evidence;
- with `R_max` = 1 registration or reactivation per block, it slows registration churn.

The rules below keep a cost for all three.

## 2. What stays as on the current network

- The reward formula and issuance:
  - subsidy 2 PCOL per block;
  - winner `alpha` = 10%;
  - proposer `gamma` = 5%;
  - shares `p_i = R·c_i/Q` with `Q = max(C, κ·m)` = max(C, 64);
  - the uncovered part is burned.
- Nonce assignment and the full 64-bit nonce domain. The new genesis sets `nonceMode` = 1 with the
  fork schedule `[[1,0]]`, so the domain is full from block 1.
- Share and block targets, `W_act`, `Theta_inact`, `U`, `R_max`, `M_max`, and the inactivity, exit
  and reactivation rules (including the wallet's rejoin flow from PR #17).
- Every other consensus parameter of the current genesis, unless section 3 says otherwise.

## 3. Rules of the new network (from genesis)

**R1. Bond.** Genesis CP field 16 `B_reg` = 20 PCOL (`20·10^18` wei).

**R2. Registration with a partial bond.** `register(address reward)` accepts any `msg.value` `v`
with `0 ≤ v ≤ B_reg`:
- `v = B_reg`: the record is full (`bondState` HELD = 1), as today.
- `v < B_reg`: the record is on probation, with a new `bondState` ACCRUING (4) and `bond = v`.

Both share the `R_max` slot and the `M_max` capacity check. `registeredAt` (the registration height)
is stored in a new record field (section 7).

**R3. Probation seats.** At most `P_max` = 8 records may be ACCRUING at once. Beyond that,
`register` with `v < B_reg` reverts `Capacity`.

**R4. Earning into the bond.** While a record is ACCRUING, every payment owed to it goes into its
bond instead of to its reward address: the winner share, and its share payments (R7 removes the
proposer share). When the bond reaches `B_reg`:
- the record becomes HELD in the same settlement;
- the excess is paid to the reward address.

The funds stay in Escrow and `heldTotal` grows by the same amount, so the escrow invariant
`balance ≥ pending + heldTotal + burned` keeps holding.

**R5. Top-up.** `topUp()`, called by the identity itself with `msg.value > 0`, adds
`min(msg.value, B_reg − bond)` to an ACCRUING bond and returns the rest to the caller. The record is
promoted when the bond reaches `B_reg`. (P4 change: the first draft reverted a value above
`B_reg − bond`; see EARN-IN-SPEC.md U2.)

**R6. Probation deadline.** An ACCRUING record still ACCRUING at
`registeredAt + T_prob` (`T_prob` = 60,480) is exited by the sweep, with `exit = h` and
`due = h + U`. At `due` the accrued bond is refunded to the reward address and the record is erased,
which frees the seat.

**R7. Proposer draw.** ACCRUING members keep a nonce range and earn wins and shares, but get
weight 0 in the proposer draw. If every member is ACCRUING, the draw falls back to all members.

**R8. Exit, inactivity, punishment.**
- `deregister`, `removeInactive` and `reactivate` work for ACCRUING records as for HELD ones; the
  accrued bond is refunded at `due`.
- `reactivate` does not reset `registeredAt`.
- `punish` burns the accrued bond and tombs the record.

**R9. Durable self-reward.** `register` accepts `reward == msg.sender` without the PAY-02 durability
probe, because the sender's nonce is at least 1 once the transaction runs. Other reward addresses
still need PAY-02.

**Constants.** `P_max` and `T_prob` are constants of the system contract code, which genesis binds
through `sysCodeHash`. They are not new CP fields, so the 52-field GenesisSpec schema stays as it is.

## 4. Moving balances to the new network (N1)

The old chain is stopped at an agreed final height `S`, and the new genesis allocation is built from
its canonical state at `S`. The construction is deterministic and reproducible by anyone holding the
old chain.

1. **Every user account moves unchanged:** nonce, balance, code and storage. This covers contracts
   users deployed, for example Studio sites. With a new chain ID, old signed transactions cannot
   replay.
2. **Not copied:** the five system program addresses (Caps, Ledger, Escrow, Registry, factory).
   Their state is rebuilt by the new genesis.
3. **Held bonds of old records go to their reward addresses' balances:** 100 PCOL for every registered
   identity (7 of them on 2026-11-01).
4. **The three genesis miners fund their 20 PCOL bonds from their own moved balances.** The new
   supply then equals the old supply.
5. **Unsettled reward pots** of the last `D` = 4 blocks before `S` (at most 8 PCOL) are not
   reproduced. Their value is not issued on the new network. Each one is listed in the cutover report.
6. **The cutover report:**
   - lists every moved account and the sums before and after;
   - publishes the old final block hash at `S`;
   - publishes the new genesis hash, so anyone can check the result.

This reads "balances move" as the whole user state moving, so no user loses coins or contracts. If
only plain balances should move (no contract code or storage), that is a one-line change to the
construction and has to be decided before launch.

## 5. A new miner still needs a little PCOL for fees

The bond can be earned, but a transaction fee cannot: gas is never free.
- With R9, one transaction is enough. The wallet's fee cap for a registry transaction is about
  0.001 PCOL; the fee actually charged is lower.
- The fee faucet (A2) sends about 0.002 PCOL per address, once per address, within a daily limit.
- The faucet is funded from an operator account's moved balance. No coins are created for it.

## 6. Time to earn the 20 PCOL bond

Paid value per block ≈ 2 PCOL × (0.10 + 0.85 × C/64), with about C ≈ 33 shares per block, which gives
about 1.08 PCOL (the proposer share is excluded, per R7). A probationer with hash share `f` needs
about `18.6 / f` blocks:

| `f` | Blocks | Time |
|---|---:|---:|
| 1/2 | 37 | about 6 min |
| 1/8 | 149 | about 25 min |
| 1/32 | 594 | about 1.7 h |
| 1/100 | 1,857 | about 5.2 h |
| 1/1000 | 18,570 | about 52 h |

With `T_prob` = 7 days, a miner needs `f` ≥ about 0.03% to finish probation.

## 7. Storage layout additions (MinerRegistry)

Everything below is additive. Existing words keep their meaning.

| Where | New content |
|---|---|
| `w0.bondState` | value 4 = ACCRUING |
| `w0` bits 152..215 | `registeredAt` (u64), previously reserved zero |
| slot 8 | ACCRUING count (u32) |

The Escrow gains an `accrue` path that only Escrow uses internally: `payTo` and `distribute` call
Registry `accrue(id, amount)`, which is callable only by Escrow. The Registry gains `topUp()`.

## 8. Security analysis

| Concern | Old network (100 PCOL) | New network |
|---|---|---|
| Filling the 64 full seats | 6,400 PCOL locked | 1,280 PCOL locked, or the hash work to earn it (about 1,190 blocks of the whole network's paid reward) |
| Filling probation seats | not applicable | 8 identities that keep sharing: cheap, but it only delays newcomers by up to 7 days, never block production (R7) |
| Penalty for misbehaviour | burns 100 PCOL | burns up to 20 PCOL |
| Taking a sponsor's money | possible with plain sponsorship | not applicable: a probationer's bond is its own reward |
| Slowing blocks with absent proposers | full members only | full members only (R7) |
| Supply | not applicable | moved supply = old supply minus at most 8 PCOL of unsettled pots |

## 9. Implementation phases

Each phase is its own pull request and needs no network action. The owner reviews each one.

| Phase | Content |
|---|---|
| P1 | Normative text for R1–R9 and section 4. An independent reference model, and test vectors from it |
| P2 | System contracts: MinerRegistry and RewardEscrow with R2–R9, against the P1 vectors |
| P3 | Node: R7 in the proposer draw. Genesis tooling for the new chain ID, `B_reg` = 20 and `nonceMode` = 1. The cutover tool (section 4) with its report |
| P4 | Clients: wallet engine and Windows wallet (register with 0–20 PCOL, probation progress, top-up); extension, Explorer, gateway reward accounting, websites; fee faucet |
| P5 | Tests: full suites, multi-process devnets, a cutover rehearsal from a copy of the old chain, mutation checks, Windows CI |
| P6 | Release, and a rollout runbook for the owner: freeze `S`, stop the old services, build genesis, start the three servers, publish clients |
