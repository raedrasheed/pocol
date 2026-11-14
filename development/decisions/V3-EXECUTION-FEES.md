# V3-PR-C3 execution details

> **Superseded in part (FN-2a, owner decision 10d of 2026-11-13: no account-creation fee).** The creation burn
> described below ("Creation pricing", "The signed maximum fee", the CreationFeeBurn receipt annotation, the burn
> pricing of the pool, builder and eth_estimateGas, and `pocol_quoteCreationBurn`) is deleted, with the CP schema v2
> fields F0_acct, F0_slot and F0_code (CP schema v2 now has 64 fields; later indices moved down by three). New
> accounts, nonzero slots and code pay ordinary gas only. Kept: the base-fee floor, the v3 log caps, and ACCT_CAP
> and SLOT_CAP as A03 exceptional halts that charge the declared gas (owner answer 11 of 2026-11-13), with
> the pool's and builder's live-cap checks (`PoolMeter::admit`, `BlockMeter::check`); CODE_CAP still fails the CREATE
> (CreateContractSizeLimit, `code_caps.rs`). The burn sections are kept
> below as the record of D-V3-21; they no longer describe the code. Cap-fill risk: threat.md (v3 amendment), C10.

Implements D-V3-20/21/24. This is dormant host execution, selected only by
`v3_active(height)`. Release admission remains SPEC_VERSION=1, MAX_FORK_VERSION=2
and CONSENSUS_RULES_VERSION=1. No production caps, fee coefficients, live profile
or deployment are selected here. F still owns measured profile parameters.

## Base fee and logs

`next_base_fee` accepts an explicit floor. Every consensus, pool, RPC and wallet
prediction supplies baseFeeMin for the prospective child height under v3, zero
otherwise. The first child's baseFee0 is clamped too. London validation and wide
overflow errors precede the clamp; a floor cannot mask an invalid parent.

V3 block log caps are 262144 charged bytes and 4368 logs, versus 32768/546.
LOG-ACCT-01 remains `36 + 32*topics + data`; dirty-account/key caps are unchanged.
The existing finite-chain LOG_CAP rule and continuous-chain exception remain.
Execution, pool admission and template selection meter the active limits.

R26 reserves for the largest cap the admitted profile can activate, including
future heights. Preserve the existing response metadata allowance: RESP_MAX must
cover `2*active_log_bytes + active_log_count*512`, and the scan/count limits must
cover the active block limits. Defaults already cover v3. Both the complete gate
and the experimental-devnet partial evaluator use these limits. An unrelated
absent RPC field cannot waive an enforced log-limit predicate.

One v3 transaction may emit every block log (4368 logs / 256 KiB), so its JSON
receipt can reach `2*262144 + 4368*512 + 2048` bytes (about 2.63 MiB), above
LIGHT_RESP_MAX (512 KiB). In the v3 domain (`Limits.first_imports`, set at open
from `spec.v3_active(MAX)`) the node therefore serves eth_getTransactionReceipt
as heavy: heavy lane, reservation, deadline and RESP_MAX answer limit
(`rpc::Limits.heavy_receipts`). The v3 R26 adds 2048 bytes to its RESP_MAX
bound so that it covers that receipt. The cost is availability: every v3
receipt request, which wallets poll often, then takes a HEAVY_QUEUE place, a
RESP_MAX reservation from HEAVY_RESP_POOL and the heavy deadline, so receipt
polling competes with eth_getLogs and eth_call and gets -32021 busy when the
heavy lane or pool is full. V3-PR-G clients treat receipt polling as
heavy-class and back off on -32021 (V3-DESIGN.md PR-7);
`pocol_getParams.rpcLimits` does not yet report the flag, so that spec-1 output
stays byte-identical. Spec-1 profiles keep the light row
(<= 339 KiB) and every legacy lane, limit and R26 predicate byte-identical.

## Creation pricing and exceptional halts

After native execution, count persisted additions from the canonical transaction
diff. Accounts and nonzero slots exclude the five system programs, as their live
caps do; code is the same per-address logical inventory as CODE_CAP. An existing
prefunded destination is not a new account. Temporary/reverted creations are not
persisted additions. Identical runtime at different new addresses is priced per
logical byte at each address, even when its storage hash is shared. Preserve
LP4-CREATE-01: nonzero copied storage prevents a
CREATE collision from overwriting it, even with nonce zero and empty code.

For each class separately, release that transaction's removed units, then price
its newly persisted units at consecutive occupancies. Thus a slot replacement at
a full cap still pays for one new slot at the last-unit price, even with zero net
growth. This final-diff accounting preserves existing cap/commit semantics and
does not depend on hash-map iteration or the order of independent SSTOREs inside
a transaction. Subsequent transactions use the updated live counts. There is no
new shared growth budget.

For `n` additions, starting after removals at `used`, charge exactly
`SUM(i=0..n-1, floor(F0*CAP/(CAP-used-i)))`. Calculate each division with a U512
product, then check the payable sum. Do not approximate charged amounts with a
harmonic function or round only an aggregate. A zero F0 waives the price, never
the cap. The harmonic approximation is only an independent fill-cost test.

The sender pays from its actual post-execution balance, in addition to gas and
value. Deduct only after all classes are quoted and the total is payable, and
only within the signed maximum fee (D-V3-21, next section). A charge past that
maximum, unpaid or overflowing halts with `Halt::CreationFee` on the existing
exceptional-halt path: discard the native effects and logs, increment the
original sender nonce, charge the spent gas before refunds (not the declared
gas; see "The signed maximum fee") and credit only its gas tips to Escrow. The block remains
valid.
A later block-cap halt also discards the provisional creation charge. Native
reverts pay no creation fee. Pool admission refuses such unpayable effects;
selection re-executes and reprices in its own order and skips a cap-style halt.
Parking preserves the existing pre-execution/deferred-admission policy: parked
transactions are repriced before selection or actual pending-state admission.

The admitted R23 supply bound is below 2^127; aggregate paid burns fit U256.
Unadmitted control APIs retain checked aggregate arithmetic, as for their tips.
F0_slot and F0_code in tests are explicitly synthetic, not a calibration result.

## The signed maximum fee (D-V3-21, owner option A)

The owner chose option A on 2026-11-12
([V3-OWNER-DECISIONS-2026-11-12.md](V3-OWNER-DECISIONS-2026-11-12.md), second
decision). Under v3 only, a nonzero creation burn is charged only if

    gas_used * effective_price + burn <= max_fee * gas_limit

where `max_fee` is maxFeePerGas for type 2 and the gas price for types 0/1, and
`effective_price` is the block's base fee plus the paid tip (the whole gas price
for types 0/1). `gas_used` is the native execution's charged gas (after
refunds), the same quantity a successful transaction pays. The value is not in
the sum: it stays separate, as in the Ethereum balance pre-check
`balance >= max_fee*gas_limit + value`, which revm enforces before execution.
That pre-check makes the bound imply affordability; the post-execution balance
check stays as a defence. Otherwise the transaction halts with
`Halt::CreationFee`. A zero burn is never checked (its gas cost is always within
the maximum). Code: `user_phase::SignedFee`, `charge_creation`; every caller
(USER phase, pool admission, builder selection, eth_estimateGas) passes the
transaction's signed fields. Spec-1 is unchanged: `charge_creation` returns no
burn and no halt unless `v3_active(height)`.

**Halt charge (decided under the owner rule).** A `Halt::CreationFee` charges
the gas the execution spent, at the effective price (tips on that gas to Escrow;
the block's gas used and the receipt's cumulative gas count it), never the
declared gas_limit. The spent gas is taken before the EIP-3529 refund
(`gas_used + gas_refunded` of the successful native result, at most gas_limit):
the halt discards every effect, including the storage clears that earned the
refund, so it is priced like a revert of the same work (revm applies refunds
only on success) and opens no spam discount against a revert. Every other A03
cap halt keeps the declared gas. Reason: under option A a wallet raises
gas_limit above the execution's gas so that `max_fee*gas_limit` also carries
the burn. The burn's price is set by the occupancy at inclusion, which the
builder's order sets. Charging the declared gas when that order pushes the burn
past the maximum would turn the sender's unused burn allowance into a gas
charge, which a hostile proposer could cause on purpose. With the spent gas the
sender pays what the execution cost, gets no effects and pays no burn, so on
this route its loss is `spent*effective_price <= max_fee*gas_limit`. Principle
check: a function of the transaction and ancestor state (Q2.4); burns stay
classified (Q4.3); collaborative consensus, the template block and fork choice
are untouched; constants and identity are unchanged. No current balance-only
CreationFee halt remains reachable for an engine result, so this rule covers
every CreationFee halt.

**Residual: other cap halts (not closed).** The spent-gas rule closes the
CreationFee route only. Every other order-dependent A03 cap halt charges the
declared gas, and a transaction can meet one in a block built by a hostile
proposer or, through another user, in a block an honest node builds. Such a
block is valid, and the sender pays its whole declared
`gas_limit*effective_price`, which under option A includes the gas it added to
carry the burn, for no effect. The guaranteed loss bound, whoever orders the
block, is therefore the owner's bound itself, the signed `max_fee*gas_limit`:
never more than the sender signed, but not only its spent gas. Routes:

- Hostile order. A proposer's own transactions ahead fill the block's log count
  (`V3_LOG_COUNT_MAX = 4368`; at 375 gas per LOG0 about 1.6M gas), and every
  transaction with a nonzero burn emits at least its CreationFeeBurn log, so it
  halts with LogCount; filling the remaining account or slot units ahead
  (AcctCap/SlotCap) is another, costlier route, and so are LogBytes, LogCap and
  the dirty-key/account caps.
- Any user against an honest builder. The honest selection checks the caps (B4)
  only in its own order and on its own simulation. Its append round (B5) adds
  the candidates it held back for gas or for the execution budget after the
  selected ones, and `build_signed` accepts halts. Review reproduction: a user
  fills the log count with one included call (4368 LOG0) and exhausts the
  selection's execution budget with heavy one-log calls that B4 then leaves out
  (they stay pooled, unpaid, and can be reused every block); an estimated
  creating transfer held for the append round was appended after the filler and
  halted with LogCount, charged its whole declared gas, in a template the honest
  node signed. Since this review, under v3, the append round drops an appended
  transaction that halts in its build with every appended one after it (as one
  the build reports by index; node-local, `build_signed_template`), so this
  route is closed for honest builders. The transaction stays pooled.
- Honest-builder routes still open. The selection includes, by design, a
  transaction whose own logs pass a log cap in every block on its parent (for
  example logs that follow PREVRANDAO, which admission does not see) as that
  halt at its declared gas, so it cannot stay pooled unpaid; and caps the
  selection does not model (the dirty-key and dirty-account caps), or the
  difference between its simulation and the block (whose SYSTEM phase runs
  first), can make a selected transaction halt in the real build.

This was already true of A03 cap halts before option A (any transaction that
logs can be pushed into a LogCount halt); option A raises the declared gas that
such a halt charges. Extending the spent-gas charge to these halts would change
A03's full-gas anti-spam semantics and is not part of option A; it is an open
question for the owner.

**Per-transaction burn ceiling.** Since gas_limit <= GAS_LIMIT, one transaction
can burn at most `max_fee*GAS_LIMIT - gas_used*effective_price`. Creating a unit
whose price is p therefore needs `max_fee*gas_limit >= p + gas cost`, so near a
full cap the sender must sign, and hold, a large maximum: the last account at
the candidate F0_acct=10^15 wei and ACCT_CAP=200000 costs F0·CAP = 2·10^20 wei
(200 PCOL), which at an 8M GAS_LIMIT needs max_fee >= 2.5·10^13 wei
(25,000 gwei) with gas_limit = GAS_LIMIT. Such a transaction also reserves its
whole declared gas against the block (Cancun) and the pool's per-sender gas
quota. A unit priced above `max_fee*GAS_LIMIT` for every fee the sender is
willing to sign cannot be created by that sender; it is never charged more than
it signed. The anti-fill cost of the pricing rule is unchanged.

**Estimation and quote.** eth_estimateGas finds a gas limit whose
`max_fee*gas_limit` covers the burn: its first run, at the largest affordable
limit, must pass the bound at the current occupancy (otherwise the error names
CreationFee); its binary search requires the bound with the burn priced as if a
few more creations were included ahead (`ESTIMATE_DRIFT_UNITS = 4`): 4 more
occupied accounts, 4 more slots and `4*B_code_max` more code bytes (4
deployments of the largest size), each clamped so the additions still fit the
cap. The margin absorbs a few creations included ahead of the transaction; it
is not a guarantee, and the consensus bound limits the loss when it is
exceeded. The margin applies only when a limit below the first run's passes
it: when none does, or when the funds or the request's gas cap leave less than
1000 gas to search, the estimate is the first run's limit, which is checked
only at the current occupancy. The price used for the gas term is the request's
maxFeePerGas/gasPrice and maxPriorityFeePerGas, or the predicted base fee; a
wallet that then signs a higher max_fee is still covered. `pocol_quoteCreationBurn`
(node-local, v3 profiles only, network.md "Creation burn quote") returns the
burn split by class, its total and the gas, at the head's occupancy, priced
like the estimate's first run.

**Client consequence: no fixed 21000-gas creating transfers.** Under v3 a
transaction whose effects create an account, slot or code needs a gas_limit
above its executed gas, at least
`ceil((gas_used*effective_price + burn) / max_fee)`. With gas_limit = 21000 =
gas_used a type 0/1 transfer has no room at all (its effective price is its
max_fee), and a type 2 transfer has only `21000*(max_fee - effective_price)`,
about 2·10^13 wei at a 1 gwei base fee with max_fee = 2·base fee, below the
first account's price F0_acct = 10^15 wei. So a plain transfer to a new address
signed with the hard-coded 21000 (common in scripts, library defaults and
wallets that skip eth_estimateGas for code-less recipients) is refused by the
pool (`-32000 creation charge would halt (CreationFee)`) and skipped by the
builder. Clients must use eth_estimateGas, or add the burn from
`pocol_quoteCreationBurn`, to set the gas limit.

The pool refuses a submission whose burn would pass its signed maximum on the
pending state; the builder reprices in its own order and skips one that would
then pass it, and its append round drops an appended transaction that halts
(under v3). Another proposer may include it; the transaction then halts with
CreationFee as above.

## Receipt and accounting class

A nonzero paid charge appends one standard receipt log from ChainCaps (C0C004),
whose immutable program emits no EVM logs. Topics are
`keccak256("CreationFeeBurn(address,uint256,uint256,uint256)")` and the sender
address, zero-padded to 32 bytes. Data is three big-endian U256 words in order:
account burn, slot burn, code burn. Its LOG-ACCT-01 charge is 196 bytes. Meter this
log's bytes and count, include it in the receipt bloom/root and ordinary receipt
history, and discard it on a halt. Zero charges add no log. Ethereum's four-field
receipt RLP encoding and transaction wire format remain unchanged.

The receipts are the record of these three external burn classes: the block's
burn is the sum of its CreationFeeBurn annotations (the USER phase result also
exposes the running total while the block executes). No ChainFacts, BlockRun,
BlockOutput or Entry field carries a burn, so legacy report layouts and the
legacy memory model are unchanged; `chain_facts(db)` stays a pure state query.
The burns are not Escrow's internal held `burned_total`, and are never added to
Escrow's accounting need. Receipts preserve the class on restart, checkpoints,
stored-state replay and history offload. No new system slot, SYSTEM write or
template/header field is introduced.

Conservation tests check at every block that live balances plus external gas-base
and creation burns equal initial balances plus issuance, alongside the separate
Escrow invariant. They compare copy, summary/overlay and stored trie execution.

## Predictions and clients

Pool pending effects include the same fee deduction and annotation. Builder
selection reprices from its actual parent and selection order; a big-first
candidate can change whether another sender can afford its charge. Independent
block execution must reproduce the selected body's gas, effects and receipts.

V3 `eth_estimateGas` models creation charges (within the signed maximum, with
the occupancy margin above), active log limits and real gas/value/creation
affordability when `from` is supplied. A no-sender gas query
uses a funded virtual caller and the original parent's cap counts, so that query
control does not create a fictitious charged sender. `eth_call` remains an
unpriced read-only simulation. Neither query commits state or burns funds.
The gas search respects an explicit gas limit and the sender's real gas-price
budget; it does not require enough balance to buy a whole block of gas.
Priced estimates expose the actual next block's GASLIMIT to code, so the
separate read-only call ceiling cannot select a different creation-fee branch.

Gas estimates remain gas quantities, not coin-denominated fee quotes; under
D-V3-21 option A the estimated gas limit is large enough that the signed
maximum fee also carries the burn, so an unmodified wallet (MetaMask) that
signs the estimated gas and its own max_fee is bounded by what it shows the
user. Wallets that want to show the burn separately read
`pocol_quoteCreationBurn` (V3-PR-G client work); no signed field or transaction
type is added.

The amount charged still depends on inclusion order. Within one block, every new
unit is priced at the occupancy left by the transactions before it, and the
builder chooses that order (its big-first slot included); an earlier
transaction can raise the price, up to F0*CAP for the last free unit. Since
option A that order can no longer charge a sender more than it signed: if the
burn would pass `max_fee*gas_limit`, the transaction halts with CreationFee and
pays only its spent gas (other order-dependent cap halts still charge the
declared gas; see "Residual: other cap halts"). This closes the open pre-launch item of D-V3-21;
the `D-V3-21 creation-burn user bound` entry has left `V3_LAUNCH_BLOCKERS`
(`lp4-execution/src/admission.rs`), which is now empty. The gate and its call in
`admit` stay so that a later blocker is enforced again. Admission of a v3
profile outside simnet is still refused by the v3-preview build restriction,
and a release build cannot decode spec 2 (V3-PR-I).

ParamGate does not yet check the new CP fields (F0_acct, F0_slot, F0_code > 0;
an upper bound on baseFeeMin). Zero coefficients are a tested fixture domain
(a zero F0 waives the price, never the cap), and the values are calibrated by F,
so these sanity rules belong with F's v3 profile and ParamGate R1-R26 run at the
v3 CP, before V3-PR-I.

## Resource model and validation

The node derives its v3-cap domain at open from `spec.v3_active(MAX)` (the existing
`Limits.first_imports` domain). All per-block retained/window/side/reload log terms
use the largest active log-byte cap. Finite aggregate LOG_CAP is unchanged.
Encoded annotations are already inside these
log bounds. Add 4096 bytes per execution (v3 domain only) for the extra
native-result annotation allocation/growth beyond EVM-gas-purchased logs. No
block report, ChainFacts or Entry field carries the burn, so the legacy report
layouts and model are unchanged by it. Other scalar amounts and wide arithmetic
use the reserved consensus stack. No state counter, lifetime metadata map or
per-unit annotation is retained.

Receipt building itself was under-counted on every profile, spec-1 included:
the unchanged legacy `receipts::encode` nests growing RLP/topic/log/envelope
buffers, `ordered_root` clones every encoded receipt for the receipts root, and
apply copies the USER receipt list into its report while the original is
alive. Count this peak explicitly: `16*active_log_bytes + 2*side_txs*per_tx +
65536` bytes, where `side_txs=min(BODY_MAX/MIN_TX_BYTES,GAS_LIMIT/21000)` and
`per_tx=380` is the existing receipt/entry allowance. Sixteen log-byte copies
cover the nested buffers/Vec growth and metadata (about 9.3 by inspection); the
two receipt buffers and 64 KiB cover the per-transaction base fields and fixed
bloom/short-list scratch. Retained history and native EVM log storage remain
separate, so this is conservative. Only its log cap is v3-gated. The default
legacy 8M-gas profile adds 878624 B.

Exact pricing work is bounded by newly persisted state under existing gas,
runtime and dirty limits: deployed bytes cost 200 gas each, slots/accounts have
their existing dirty ceilings. No resource reserve or execution-time limit is
relaxed. Raising candidate live-state caps still requires F's A13 evidence.

Spec-1 impact of that legacy correction: the six continuous non-evicting record
stops, ordered by peers 8/32/64 and dials 2/7, are now 1,145,894 / 1,121,036 /
1,113,874 / 1,089,243 / 1,071,332 / 1,046,701 (formerly 1,146,268 / 1,121,410 /
1,114,248 / 1,089,603 / 1,071,706 / 1,046,848). All original R10 baselines and
owner seed-rule assertions remain; the default 32-peer guard still passes. Every
windowed file stop moves down by `4096*878624=3598843904` bytes; (32 peers, 2
dials) is now 11,590,057,361,535 B. Exact pins and one-byte boundary tests remain
enforced. Default Linux x86-64 devnet model: 3,397,791,234 bytes with eviction,
3,388,329,613 without (3,396,912,610 / 3,387,450,989 before). These are capacity
models, not elapsed-life or RSS claims; verdicts, roots, fork choice and journal
and checkpoint bytes are unchanged.

Tests live in `pocold/tests/v3_execution.rs`, LP4 fee/creation-fee and admission
tests, and LP1 param-gate tests. Original London and USER vector files remain
byte-identical. The preview CI job runs the new execution suite explicitly.

The RPC deadline regression also tests ordering directly: for a single request
and a batch, hold admitted backend work and its body until the real HTTP client
receives 504, then release it. A transport waiting for backend completion cannot
pass. The real-node flood test retains expired-work, batch-member and resource
assertions. Its old client-wall-time upper bound compared an unloaded ~29 ms VM
sample with a client competing against eight flood threads; CI observed ~48 ms
despite HTTP 504. The causal probe replaces that scheduler-dependent comparison;
production deadlines and cancellation behavior are unchanged.

The inbound-flood regression holds the accepted replacement connection until
both its reserved outbound slot and the still-full inbound handshake set are
checked. Previously the temporary `accept(...).is_some()` dropped the remote
socket before checking its live count; Windows could release the slot first.
This fixes the test's socket lifetime, without changing production connection
limits, reconnect behavior, or test timeouts.
