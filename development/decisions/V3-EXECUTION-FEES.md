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

One v3 transaction may emit every block log (4368 logs / 256 KiB), so its JSON
receipt can reach `2*262144 + 4368*512 + 2048` bytes (about 2.63 MiB), above
LIGHT_RESP_MAX (512 KiB). In the v3 domain (`Limits.first_imports`, set at open
from `spec.v3_active(MAX)`) the node therefore serves eth_getTransactionReceipt
as heavy: heavy lane, reservation, deadline and RESP_MAX answer limit
(`rpc::Limits.heavy_receipts`). The v3 R26 adds 2048 bytes to its RESP_MAX
bound so that it covers that receipt. Spec-1 profiles keep the light row
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

The amount charged depends on inclusion order. Within one block, every new unit
is priced at the occupancy left by the transactions before it, and the builder
chooses that order (its big-first slot included). A transaction's quote, from
eth_estimateGas or the pool, is therefore a quote at the state it was computed
on, not a consensus limit: an earlier transaction in the same block can raise
the price, up to F0*CAP for the last free unit. A sender who wants a ceiling has
none today. This is the open pre-launch item of D-V3-21 (V3-DECISIONS.md): the
owner chooses a node-local quoted-burn limit in the pool and eth_estimateGas
(with V3-PR-G client support) or a sender-signed cap before V3-PR-I. Until then
`V3_LAUNCH_BLOCKERS` in `lp4-execution/src/admission.rs` holds
`D-V3-21 creation-burn user bound`, and admission refuses every profile that
can activate v3 outside a simnet profile on a simnet node, in any build.

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
