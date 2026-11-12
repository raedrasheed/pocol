# V3-PR-C3 execution details

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
value. Deduct only after all classes are quoted and the total is payable. An
unpaid or overflowing charge follows the existing exceptional-halt path: discard
the native effects and logs, increment the original sender nonce, charge the full
declared gas, and credit only its gas tips to Escrow. The block remains valid.
A later block-cap halt also discards the provisional creation charge. Native
reverts pay no creation fee. Pool admission refuses such unpayable effects;
selection re-executes and reprices in its own order and skips a cap-style halt.
Parking preserves the existing pre-execution/deferred-admission policy: parked
transactions are repriced before selection or actual pending-state admission.

The admitted R23 supply bound is below 2^127; aggregate paid burns fit U256.
Unadmitted control APIs retain checked aggregate arithmetic, as for their tips.
F0_slot and F0_code in tests are explicitly synthetic, not a calibration result.

## Receipt and accounting class

A nonzero paid charge appends one standard receipt log from ChainCaps (C0C004),
whose immutable program emits no EVM logs. Topics are
`keccak256("CreationFeeBurn(address,uint256,uint256,uint256)")` and the sender
address, zero-padded to 32 bytes. Data is three big-endian U256 words in order:
account burn, slot burn, code burn. Its LOG-ACCT-01 charge is 196 bytes. Meter this
log's bytes and count, include it in the receipt bloom/root and ordinary receipt
history, and discard it on a halt. Zero charges add no log. Ethereum's four-field
receipt RLP encoding and transaction wire format remain unchanged.

`BlockOutput.facts.creation_burn` and `BlockRun.facts.creation_burn` report these
three external burn classes for that block. They are not Escrow's internal held
`burned_total`, and are never added to Escrow's accounting need. A state-only
`chain_facts(db)` query has no block/receipt context and leaves the per-block class
zero; it is not a cumulative external-burn query. Receipts preserve the class on
restart, checkpoints, stored-state replay and history offload. No new system slot,
SYSTEM write or template/header field is introduced.

Conservation tests check at every block that live balances plus external gas-base
and creation burns equal initial balances plus issuance, alongside the separate
Escrow invariant. They compare copy, summary/overlay and stored trie execution.

## Predictions and clients

Pool pending effects include the same fee deduction and annotation. Builder
selection reprices from its actual parent and selection order; a big-first
candidate can change whether another sender can afford its charge. Independent
block execution must reproduce the selected body's gas, effects and receipts.

V3 `eth_estimateGas` models creation charges, active log limits and real
gas/value/creation affordability when `from` is supplied. A no-sender gas query
uses a funded virtual caller and the original parent's cap counts, so that query
control does not create a fictitious charged sender. `eth_call` remains an
unpriced read-only simulation. Neither query commits state or burns funds.
The gas search respects an explicit gas limit and the sender's real gas-price
budget; it does not require enough balance to buy a whole block of gas.
Priced estimates expose the actual next block's GASLIMIT to code, so the
separate read-only call ceiling cannot select a different creation-fee branch.

Gas estimates remain gas quantities, not coin-denominated fee quotes. Before I,
the G client work must show the additional creation-burn estimate separately
from the maximum gas fee: creation pricing depends on inclusion-time occupancy,
so the existing maximum gas fee does not bound it. The ordinary sender balance
check prevents an unpaid inclusion; this PR adds no new signed fee-limit field.
The release cannot admit this dormant v3 behavior before that client stage.

## Resource model and validation

The node derives its v3-cap domain at open from `spec.v3_active(MAX)` (the existing
`Limits.first_imports` domain). All per-block retained/window/side/reload log terms
use the largest active log-byte cap. Finite aggregate LOG_CAP is unchanged.
Encoded annotations are already inside these
log bounds. Add 4096 bytes per execution for the extra native-result annotation
allocation/growth beyond EVM-gas-purchased logs. Count the 96-byte CreationBurn
fields in two boxed BlockRun reports even on legacy profiles: 192 added model
bytes. Other scalar amounts and wide arithmetic use the reserved consensus stack.
No state counter,
lifetime metadata map or per-unit annotation is retained.

Receipt encoding itself nests growing RLP/topic/log/envelope buffers, and apply
copies the USER receipt list into its report. Count this previously implicit
peak explicitly: `16*active_log_bytes + 2*side_txs*per_tx + 65536` bytes, where
`side_txs=min(BODY_MAX/MIN_TX_BYTES,GAS_LIMIT/21000)` and `per_tx=380` is the
existing receipt/entry allowance. Sixteen log-byte copies cover the nested
buffers/Vec growth and metadata; the two receipt buffers and 64 KiB cover the
per-transaction base fields and fixed bloom/short-list scratch. Retained history
and native EVM log storage remain separate, so this is conservative. It grows
with the active v3 cap. The default legacy 8M-gas profile adds 878624 B here,
plus 192 B for the reports: 878816 B in total.

Exact pricing work is bounded by newly persisted state under existing gas,
runtime and dirty limits: deployed bytes cost 200 gas each, slots/accounts have
their existing dirty ceilings. No resource reserve or execution-time limit is
relaxed. Raising candidate live-state caps still requires F's A13 evidence.

The six legacy continuous non-evicting record stops, ordered by peers 8/32/64
and dials 2/7, are now 1,145,894 / 1,121,036 / 1,113,874 / 1,089,243 / 1,071,332 /
1,046,701 (formerly 1,146,268 / 1,121,410 / 1,114,248 / 1,089,603 / 1,071,706 /
1,046,848). All original R10 baselines and owner seed-rule assertions remain;
the default 32-peer guard still passes. Every windowed file stop moves down by
`4096*878816=3599630336` bytes; (32 peers, 2 dials) is now 11,590,056,575,103 B.
Exact pins and one-byte boundary tests remain enforced. Default Linux x86-64
devnet model: 3,397,791,426 bytes with eviction, 3,388,329,805 without. These are
capacity models, not elapsed-life or RSS claims.

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
