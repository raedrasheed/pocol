# Earn-in network: normative specification (P1)

Status: P1 draft, implementing the owner-approved [proposal](EARN-IN-MEMBERSHIP-PROPOSAL.md). The key words
MUST, MUST NOT and MAY are normative. Every rule here holds from genesis on the new network.
Wherever this text is silent, `PoColSystemV1.sol`, `REGISTRY-LAYOUT.md`, `consensus.md` and
`incentives.md` keep governing.

## 1. Constants and parameters

| Name | Value | Source |
|---|---|---|
| `B` | `20·10^18` wei | genesis CP field 16 (`B_reg`), so `cfg(16)` |
| `P_MAX` | 8 | system code constant (bound by `sysCodeHash`) |
| `T_PROB` | 60,480 | system code constant |
| `HELD` | 1 | `bondState` value, unchanged |
| `ACCRUING` | 4 | new `bondState` value |
| `nonceMode` | 1 | genesis CP field 4; the fork schedule is `[[1,0]]` |

Every other CP value equals the old network's genesis, except `chainId` and `g_ts`, which are new.

## 2. Record layout additions

- **`w0` bits 152..215:** `registeredAt` (u64). `register` writes it, and nothing else changes it.
  Bits 216..255 stay reserved zero.
- **Registry slot 8:** `accruing`, the number of records with `bondState = ACCRUING`, at most `P_MAX`.
  It is 0 at genesis.
- **Predicates:**
  - `live(w)` ⇔ `status(w) ≠ 0 ∧ tomb(w) = 0 ∧ bondState(w) ∈ {HELD, ACCRUING}`.
  - `held(w)` keeps its meaning (`bondState = HELD`).

## 3. User paths (MinerRegistry fallback)

The dispatcher order of checks is unchanged:
`BadCalldata → BadValue → Closing → RegOpsLimit → BadPhase → semantic`. The new selector `topUp()`
takes no arguments (length 4) and counts as a user operation, but not in the `R_max` counter.

**V1. Value check (replaces the single `msg.value == (registering ? B : 0)` rule):**
- `register` requires `msg.value ≤ B`;
- `topUp` requires `msg.value > 0`;
- every other selector requires `msg.value = 0`;
- otherwise the call reverts `BadValue`.

**U1. `register(address reward)`.** All existing preconditions hold (identity, capacity, `w = 0`,
`R_max`). In addition:
1. **Reward durability:** if `reward ≠ msg.sender`, the PAY-02 probe `Sys.pay(reward, 0)` MUST
   succeed, or the call reverts `RewardNotDurable`. If `reward = msg.sender`, the probe MUST NOT be
   made.
2. **Partial bond:** let `v = msg.value`. If `v < B` and `accruing ≥ P_MAX`, revert `Capacity`.
3. **Escrow transfer:** if `v > 0`, `Sys.pay(ESCROW, v)` MUST succeed, or the call reverts
   `PaymentFailed`.
4. **Record:** write the record as today, with `bond = v`, `bondState = (v = B ? HELD : ACCRUING)`
   and `registeredAt = h`. `heldTotal += v`. If ACCRUING, `accruing += 1`.

**U2. `topUp()`.**
1. **Preconditions:** the caller's record has `live`, `bondState = ACCRUING`, `status = 1`, and
   `h < registeredAt + T_PROB`; else revert `BadState`. `msg.value ≤ B − bond`; else revert
   `BadValue`.
2. **Escrow transfer:** `Sys.pay(ESCROW, msg.value)` MUST succeed, or the call reverts
   `PaymentFailed`.
3. **Update:** `bond += msg.value` and `heldTotal += msg.value`. If `bond = B`, then
   `bondState = HELD` and `accruing −= 1`.

**U3. `setRewardAddress`, `deregister`, `removeInactive`** use `live` in place of `held`. Every other
condition is unchanged.

**U4. `reactivate()`** uses `live` in place of `held`. If `bondState = ACCRUING`, it additionally
requires `h < registeredAt + T_PROB`; else revert `BadState`. It MUST NOT change `registeredAt`.

**U5. `claimBond()`** is unchanged, except that the refund applies to any `live` record (rule S2).

## 4. System paths

**S1. Sweep deadline (`beginBlock`).** For a scanned record with `live`, `bondState = ACCRUING`,
`status = 1` and `h ≥ registeredAt + T_PROB`:
- if `ActLen > M_min`, it is exited exactly as `deregister` exits it: `status = 2`, `exit = h`,
  `due = h + U`, removed from the ActList;
- otherwise it stays as it is until a later sweep.

This counts as a scan, not as a deletion. The existing scan and delete bounds are unchanged.

**S2. Refund.** `refund` and `Escrow.releaseBond` accept any `live` record, not only HELD ones. If
`bond = 0`, no PAY is made. If the record was ACCRUING, `accruing −= 1` when it is erased.

**S3. `markClose`** refunds every `live` record and marks it PAID. This is irrelevant on a
continuous network, but specified for completeness.

**S4. `punish(id)`.** The HELD branch applies to any `live` record. If the record was ACCRUING,
`accruing −= 1`. It burns `bond`, whatever its size.

**S5. `accrue(id, amount) → excess`:**
- only Escrow may call it; anyone else gets `Unauthorized`;
- if the record is `live` and ACCRUING:
  - `add = min(amount, B − bond)`;
  - `bond += add`, `heldTotal += add`;
  - if `bond = B`, then `bondState = HELD` and `accruing −= 1`;
  - return `amount − add`;
- otherwise return `amount` and change nothing.

## 5. Settlement (RewardEscrow)

**P1. Payment to an ACCRUING record.** When `payTo(id, x)` or `distribute` owes `x > 0` to a record
that is `live` and ACCRUING:
1. Call `excess = Registry.accrue(id, x)`.
2. `x − excess` stays in Escrow and is now held, so it is neither burned nor paid.
3. Pay `excess` to the reward address. A failed PAY burns it, as today.

Payments to HELD records are unchanged.

**P2. Escrow invariant.** `check_escrow_invariant` is unchanged: settled value that moves into
`heldTotal` leaves the Escrow balance unchanged, so `balance ≥ pending + heldTotal + burned` still
holds.

## 6. Proposer draw (node, `DrawWeights`)

**D1. Candidates.** `ParentMember` gains `accruing: bool` (`bondState = ACCRUING` in the parent
state). The candidates are the members of `A_h` with `accruing = false`. If there are none, every
member of `A_h` is a candidate.

**D2. Weights.** Candidates are weighted by their share counts over the existing window. A member
that is not a candidate has weight 0. If every candidate's weight is 0, each candidate has weight 1.
The sampler, seed and order are unchanged.

## 7. Cutover genesis (from the old chain at final height `S`)

**C1. Input.** The canonical state after block `S` of the old network, chain ID `1599846715`,
genesis `0xbe137bfd…09ca`. `S` and that block's hash are published before the new genesis.

**C2. Accounts.** Every account in that state, except the system program addresses
`0xc0c001`–`0xc0c005`, is copied with its nonce, balance, code and storage unchanged.

**C3. Old bonds.** For every old record that is HELD and not a tomb, its `bond` is added to the
balance of its reward address. Burned totals, pending pots and any Escrow surplus are not copied.

**C4. Genesis miners.**
- `M_0` is the three server identities, in a stated order. Each gets a HELD record with `bond = B`.
- Each reward address's copied balance is reduced by `B`. The genesis MUST fail if any balance is
  below `B`.
- Each reward address MUST be durable (nonce > 0 or code), as PAY-02 already requires.

**C5. Identity.** The new chain ID is not `1599846715` and not a chain ID in public use.
`g_ts` is the launch time. The CP is as in section 1.

**C6. Report.** The cutover tool MUST output, and the release MUST publish:
- `S` and the old block hash;
- the account count and the sums of copied balances;
- the bonds credited, the pots dropped (by height and amount) and the miner bonds deducted;
- the resulting total supply;
- the new genesis hash.

Anyone with the old chain MUST be able to rebuild the same genesis bytes.

## 8. Required tests for P2/P3

The P1 reference model gives an independent expected result for each of these:
- registration with `v = 0`, `0 < v < B`, `v = B` and `v > B`;
- the `P_MAX` limit;
- both sides of R9 (`reward = msg.sender`, and another reward address);
- `topUp`: exact fill, over fill, and while not ACCRUING;
- accrual: under the bond, exactly reaching it, and with excess; winner, shares and the all-ACCRUING
  proposer fallback;
- the deadline exactly at `registeredAt + T_PROB − 1` and at `+ T_PROB`; refunding 0 and non-zero
  bonds;
- `reactivate` after the deadline;
- `punish` while ACCRUING;
- the escrow invariant after every step;
- the draw candidates and weights for D1–D2;
- cutover: a small synthetic old state yields the expected allocation, report and failure cases
  (a balance below `B`, a non-durable reward).

## 9. P2 implementation

The current network's programs (`PoColSystemV1.sol`, `system-v1.json`, `system-continuous.json`) are
unchanged. The old chain keeps validating with them, and the cutover tool reads it with them.

**Source and build**
- `prototypes/lp4-execution/contracts/system/PoColSystemEarnIn.sol` is V1 plus its continuous
  `markClose`, with every change marked `EARN-IN`.
- `tests/compile_system_earnin.cjs` builds it with the same locked compiler and settings as V1:
  `solc` 0.8.28, optimizer 200, Cancun, no metadata. Command:
  `NODE_PATH=<solc@0.8.28> node tests/compile_system_earnin.cjs tests/data/system-earnin.json`.

**Program selection**
- `SystemPrograms::earnin()` loads the new programs.
- `SystemPrograms::for_spec(cp, sysCodeHash)` picks the bundled program set that the GenesisSpec
  names. Admission uses it.
- `DevnetConfig::earnin` builds a devnet with these programs and `B_reg` = 20 PCOL.

**Tests**
- `lp4-execution/tests/earnin_contracts.rs` replays all 8 model scenarios against the compiled
  MinerRegistry. It compares reverts, records, `heldTotal`, the accruing count, the active set,
  burns and payments.
- Four planted mutants (`P_MAX` 9, R9 removed, no promotion at `B`, `T_PROB` + 1) each fail it.
- `pocold/tests/earnin.rs` runs a full chain:
  - R9 for a first-transaction registration;
  - a 0 PCOL bond filled by real Escrow settlement and promoted at exactly 20 PCOL;
  - normal payment afterwards;
  - top-up that is too large (reverted) and exact (promoted);
  - the escrow invariant after every block.

**Write counts of the new or changed paths**, measured by `earn_in_write_ceilings` (SSTOREs; the V1
ceilings stay as they are):
- `register` with a partial bond: 11 (V1 `register` is 10);
- `topUp`: 3, or 5 when it promotes;
- `accrue`: 2, or 4 when it promotes.

The proposer draw (D1–D2) and the cutover tool are P3.
