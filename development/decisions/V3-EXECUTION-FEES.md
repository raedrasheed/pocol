# V3 execution details (V3-PR-C3)

Implements D-V3-20 (the base-fee floor), D-V3-24 (the block log caps) and the live-state caps of D-V3-21 on the v3
release rule set (specVersion 2, CONSENSUS_RULES_VERSION 2, MAX_FORK_VERSION 3). The rules are selected by
`v3_active(height)`, which is true on every admitted network. The values are the committed profiles'
([FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md) §2).

There is no account-creation fee (owner decision 10d of 2026-11-13; FN-2a): new accounts, nonzero slots and code pay
ordinary gas only. The creation burn that D-V3-21 first added, its signed-maximum bound (owner option A of 2026-11-12),
the CreationFeeBurn receipt annotation, the burn pricing of the pool, builder and eth_estimateGas,
`pocol_quoteCreationBurn` and the CP fields F0_acct, F0_slot and F0_code were deleted with it. Read the sections that
described them in this file's history, for example at commit `52b2507`.

## Base fee and logs

`next_base_fee` accepts an explicit floor. Every consensus, pool, RPC and wallet
prediction supplies baseFeeMin (CP 55) for the prospective child height. The first child's baseFee0 is clamped too. London validation and wide
overflow errors precede the clamp; a floor cannot mask an invalid parent.

The block log caps are 262144 charged bytes and 4368 logs (D-V3-24; the old network had 32768/546).
LOG-ACCT-01 remains `36 + 32*topics + data`; dirty-account/key caps are unchanged.
The existing finite-chain LOG_CAP rule and continuous-chain exception remain.
Execution, pool admission and template selection meter the active limits.

R26 reserves for the largest cap the admitted profile can activate, including
future heights. Preserve the existing response metadata allowance: RESP_MAX must
cover `2*active_log_bytes + active_log_count*512`, and the scan/count limits must
cover the active block limits. Defaults already cover v3. Both the complete gate
and the experimental-devnet partial evaluator use these limits. An unrelated
absent RPC field cannot waive an enforced log-limit predicate.

One transaction may emit every block log (4368 logs / 256 KiB), so its JSON
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
`pocol_getParams.rpcLimits` does not report the flag.

## Live-state caps and exceptional halts

ACCT_CAP and SLOT_CAP are A03 exceptional halts (ACCT 1,000,000 and SLOT 4,000,000 at launch; the cap-raise path,
fork version 3, can raise them, D-V3-23). Accounts and nonzero slots exclude the five system programs, as at genesis.
A user transaction whose persisted effects would pass a cap halts on the existing exceptional-halt path: its native
effects and logs are discarded, the sender's nonce is incremented, it pays its whole declared gas (owner answer 11 of
2026-11-13 keeps this rule for every A03 cap halt), and the block remains valid. CODE_CAP (2^30) is not a halt: a
CREATE that would pass it fails (CreateContractSizeLimit, `code_caps.rs`). Preserve LP4-CREATE-01: nonzero copied
storage prevents a CREATE collision from overwriting it, even with nonce zero and empty code. The pool
(`PoolMeter::admit`) and the builder (`BlockMeter::check`) check the live caps and refuse or skip a transaction that
would halt on one in their order. The owner accepted the cap-fill risk (Q1 option a); it is disclosed in threat.md.

**Cap halts and inclusion order.** A cap halt depends on the order of the block, so a transaction can meet one that
its sender did not foresee:

- Hostile order. A proposer's own transactions ahead fill the block's log count
  (`V3_LOG_COUNT_MAX = 4368`; at 375 gas per LOG0 about 1.6M gas) or the remaining account or slot units, so a later
  transaction that logs or creates halts and pays its declared gas; LogBytes, LogCap and the dirty-key/account caps
  are other routes.
- Any user against an honest builder. The honest selection checks the caps (B4) only in its own order and on its own
  simulation, and its append round (B5) adds the candidates it held back after the selected ones. Under v3 the append
  round drops an appended transaction that halts in its build, with every appended one after it (node-local,
  `build_signed_template`), so a user cannot push another user's appended transaction into a halt in a template an
  honest node signs. The transaction stays pooled.
- Honest-builder routes still open. The selection includes, by design, a transaction whose own logs pass a log cap in
  every block on its parent (for example logs that follow PREVRANDAO, which admission does not see), as that halt at
  its declared gas, so it cannot stay pooled unpaid; and caps the selection does not model (the dirty-key and
  dirty-account caps), or the difference between its simulation and the block (whose SYSTEM phase runs first), can
  make a selected transaction halt in the real build.

The guaranteed loss bound of a transaction is therefore its signed `max_fee*gas_limit`, not only its spent gas.
Charging only the spent gas for these halts would change A03's full-gas anti-spam semantics; it is not decided.

## Predictions and clients

Builder selection executes from its actual parent in its selection order.
Independent block execution must reproduce the selected body's gas, effects and
receipts.

`eth_estimateGas` models the active log limits of the next block (a call whose
own logs pass a cap gets -32000 {reason}, not a gas value) and its live-state
caps, and real gas/value affordability when `from` is supplied. A no-sender gas query
uses a funded virtual caller and the original parent's cap counts, so that query
control does not create a fictitious charged sender. `eth_call` remains an
unpriced read-only simulation. Neither query commits state or burns funds.
The gas search respects an explicit gas limit and the sender's real gas-price
budget; it does not require enough balance to buy a whole block of gas.
Estimates expose the actual next block's GASLIMIT to code.

ParamGate does not check an upper bound on baseFeeMin; the committed profiles set it to 10^7 wei.

## Resource model and validation

The node derives its v3-cap domain at open from `spec.v3_active(MAX)` (the existing
`Limits.first_imports` domain). All per-block retained/window/side/reload log terms
use the largest active log-byte cap. Finite aggregate LOG_CAP is unchanged.
There is no creation burn, so no receipt annotation, burn field or burn
arithmetic is counted (`resources.rs`, "v3-execution-inventory"). Other scalar amounts and wide arithmetic
use the reserved consensus stack. No state counter, lifetime metadata map or
per-unit annotation is retained.

Receipt building itself was under-counted before V3-PR-C3:
the unchanged legacy `receipts::encode` nests growing RLP/topic/log/envelope
buffers, `ordered_root` clones every encoded receipt for the receipts root, and
apply copies the USER receipt list into its report while the original is
alive. Count this peak explicitly: `16*active_log_bytes + 2*side_txs*per_tx +
65536` bytes, where `side_txs=min(BODY_MAX/MIN_TX_BYTES,GAS_LIMIT/21000)` and
`per_tx=380` is the existing receipt/entry allowance. Sixteen log-byte copies
cover the nested buffers/Vec growth and metadata (about 9.3 by inspection); the
two receipt buffers and 64 KiB cover the per-transaction base fields and fixed
bloom/short-list scratch. Retained history and native EVM log storage remain
separate, so this is conservative.

The live-state caps rest on A13 in store mode (FRESH-NETWORK-CP.md §1, "Caps"); no resource reserve or
execution-time limit is relaxed.

Tests live in `pocold/tests/v3_execution.rs`, LP4 fee and admission
tests, and LP1 param-gate tests. Original London and USER vector files remain
byte-identical. The consensus CI workflow runs the new execution suite explicitly.

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
