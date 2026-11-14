# Reward projection and the full nonce range upgrade

`pocol_rewards.py` is the read-only latest-block reward adapter previously
packaged for the public explorer gateway. It is now tracked beside the node
upgrade so the gateway can use the same height-dependent nonce rules. Its
`pocol-block-reward-v1` response schema and payout formula are unchanged.

The adapter does not submit transactions, modify state, change difficulty, or
prove that a projected payment has settled. It reads the captured block's state
and returns `status: "pending"`; evidence and payment eligibility can still
change before settlement.

## Trusted activation configuration

Without an explicit schedule, only consensus version 1 is accepted:

```python
from pocol_rewards import snapshot

result = snapshot(node_query)
```

For a coordinated upgrade, pass the exact operator-approved schedule used by
the nodes. `activation_height` below is an integer loaded from the trusted
deployment configuration, not from an RPC response or a public HTTP parameter:

```python
trusted_schedule = ((1, 0), (2, activation_height))
result = snapshot(node_query, fork_schedule=trusted_schedule)
```

The adapter requires the node's `pocol_getParams.forkSchedule` to match that
configuration. It never adopts a schedule advertised by the node. A legacy
node without `forkSchedule` is accepted only with the default v1 schedule.
Use the gateway's direct node RPC connection: a proxy that strips this field
must be updated before enabling the upgrade projection.

Both the captured header and its parent are checked against the trusted
schedule at their individual heights. Before activation, immutable genesis
`nonceMode` still determines the range. At and after activation, version 2 uses
the full domain of `2^64` values, including nonce `2^64 - 1`. The genesis Caps
value is not rewritten. The moving-head `activeVersion` field is not used to
choose rules for a previously captured block.

This version also corrects membership reconstruction to use **CP24 (`W_act`,
blocks)**. The earlier adapter used CP7 (`dFbWait`, seconds), which could exclude
valid members when their last activity was older than 6 but within 32 blocks
under the current network configuration.

## v3 (spec-2) networks

The v3 rules (V3-DESIGN.md, V3-DECISIONS.md) are the release rules (consensus rules 2, GenesisSpec version 2), but no
v3 network identity is pinned yet (removal plan Phase 10). The adapter reads a v3 genesis only when the
operator configures it explicitly, in the same trusted way as a fork schedule; without it every call is the pinned
spec-1 Mainnet projection above, byte for byte, and a node on another genesis is refused:

```python
from pocol_rewards import snapshot, validated_network

network = validated_network({'chainId': chain_id, 'genesisHash': genesis_hash, 'specVersion': 2, 'v3': True})
result = snapshot(node_query, fork_schedule=trusted_schedule, network=network)
```

`chain_id`, `genesis_hash` and `trusted_schedule` come from the operator's deployment configuration, never from RPC.
The adapter requires the node's `pocol_getParams` to name that chain and genesis, its `genesisPre` to hash (keccak256)
to the configured genesis and to decode as a version-2 GenesisSpec with the 59 CP items of CP schema v2 and the v3
program set's sysCodeHash (`V3_SYS_CODE_HASH`, recomputed by the tests from `system-v3.json`), and the node's
advertised fork schedule to equal the configured one. `v3_active(h)` is `active_version(h) >= 3 || specVersion >= 2`.

The answer has schema `pocol-block-reward-v3` and stays `status: "pending"`. For the captured block h it reproduces:

- **pot h**: issuance M3 (`issuance(h)`, V3-19) plus the block's tips (its receipts) less the signed proposer's tip
  share (V3-20) plus the carry into pot h, the base burn `R - A` of `settle(h-1-D)` lagged by one block (V3-15). The
  sum is checked against RewardEscrow's pending pot and the carry against `RewardEscrow.carry()`. Its projected
  settlement uses the header's owners under the all-live membership (D-V3-01: every live member of the parent's
  ActList owns a nonce range on unbounded ranges, without the `W_act` activity filter); `projectedCarryWei` is its
  uncovered pool part, which goes to the next pot rather than burning. Late shares, evidence and registry changes before
  h + D can still change it, as on spec 1. Once the registry is closing (`closing`), the final block may settle pot h
  before h + D and burn its base burn instead of carrying it: `closing.settlementMayBeFinal` says when that can happen
  (`earliestFinalHeight <= h + D`). As in the host's final predicate, `earliestFinalHeight` is `max(closeHeight + D,
  h + 1)`: H_END is only a ceiling, and with T_END = 0 (or H_END = 0) the end condition always holds.
- **block h's own flows** (`block`): these ran in block h and are not projections. They are its evidence in body order:
  kind-3 late shares with one owner per nonce under template j's assignment (D-V3-02); type-1/2 evidence burning
  only the offender's bond (and its parts of pots settled later) with `rho_bp` of that bond paid to the reward address
  of the block's signed proposer (V3-17); the whole pot burning only for an accused winner recorded ACCRUING
  (V3-16). They also include `settle(h-D)`, with its header and late owners (checked against the ShareLedger) and its
  carry, and the tip share. ACCRUING records, including a registerReplacing victim that stays exited until its due
  height (FN-2b C4), fill their bond before their reward address is paid. Registrations, exits and replacements change
  the ActList and so the nonce owners of later blocks. The ActList is read in pages of 512 through
  MinerRegistry.activeSlice (FN-2b C5; up to MAX_MEMBERS_V3 = 10,000 members), each page sent with ACTIVE_SLICE_GAS =
  30,000,000 (pocold's eth_call maximum; a page of 512 reads two cold slots per member, about 2.74M gas, more than the
  2,000,000 CALL_GAS of the other reads), and reused for the next height when a
  block has no kind-1/2 evidence and its registry counters show no registration, exit, replacement or reactivation. Once closing starts (H_CLOSE or T_CLOSE), `markClose` runs after the evidence
  and before `settle(h-D)`: it refunds up to CP 50 live records per block (`bondRefunds`), and each refunded record is
  PAID from then on, so that block's settlement and tip share pay its reward address directly instead of its bond.
- **the final block F** (`final: true`): no pot F and no issuance. `settle(F-D)` runs as in any block, then `finalize`
  burns the tips and both outstanding carries (`block.finalize.carryBurnedWei`) and `settleFinal` settles every pot in
  [F-D+1, F-1] with its base burn burned (`block.finalize.settlements`).

Every reproduced burn is checked against RewardEscrow's `burnedTotal`, the carries against `carry()`, the closing cursor
against the registry's counters, and the refunded and proposer records against state h; any difference refuses the
snapshot rather than report it. The tip share is paid after the user phase, out of an Escrow that then also holds the
block's tips and issuance, with the recipient's PAY durability of state h and on the proposer's record as the user phase
left it: when the proposer's own MinerRegistry call succeeded in that block (setRewardAddress, topUp, ...; the
destination and selector read from the committed transaction, the status from its receipt), its reward address is read
back from state h, its bond and bond state kept from phase 0 unless the call was a topUp, and a topUp that may have
promoted it to HELD before the share (ambiguous from state h) is refused. A
fallback block commits the empty body (no tips, no evidence). Bond refunds outside closing (sweep, claimBond) and
user-transaction transfers are not reward flows and are not reported. Block bodies are bound to their headers by the
transaction and evidence roots.

State reads use block heights: pocold has no EIP-1898 `{blockHash}` form for `eth_call`/`eth_getBalance` (a non-string
tag is read as `latest`), so a reorganisation away from the anchor and back within one snapshot (A to B to A) is not
detected by the final anchor re-read. A read of another branch's state would generally break the cross-checks above,
but this is not a guarantee.

The gateway enables it with `--v3-network FILE` (a JSON object as above, at most 4096 bytes, read once at startup);
the public `pocol_getBlockReward` method stays a parameterless read. A v3 reward read may also call
`eth_getTransactionReceipt`, and it has a larger read budget (`MAX_CALLS_V3`, `MAX_SECONDS_V3`).

### v3 membership reads (Sites W6)

With `--v3-network`, the gateway also serves the explorer's membership reads, each at one head block (every
`eth_call` at that height, then a re-read of the block; a changed block is error -32022) and cached per head block:

- `pocol_getMembers(start, count)` (integers, `0 <= start < 10000`, `1 <= count <= 256`, token cost 5): one
  `MinerRegistry.activeSlice` page of the active list with the record word w0 only, as `id`, `status`, `tomb`,
  `bondState`, `lastShare` and `registeredAt`, each status and bond state labelled (`HELD` 1, `BURNED` 2, `PAID` 3,
  `ACCRUING` 4). Bond and reward address are not in a page: reading w1-w3 for 10,000 members would exceed the v3
  read budget.
- `pocol_getMember(address)` (token cost 20, like `pocol_getBlockReward`): `recordWords` (all four words: w0 fields,
  the reward address, `bondWei`, exitHeight, dueHeight, burnHeight, graceStart), `recordClocks` (`lastReactHeight`
  and `lastMaintHeightMod32`; `recordWords` returns w1 as the reward address alone, so the clocks are read only
  here) and `removalStatus` (the removal verdict, labelled), plus `proposerEligible`: whether the next block's draw
  (pocold `membership_one_pass`, `Membership::from_parent`, `DrawWeights::new`) counts the record as a candidate. A
  live member of the proposer membership A_h* is one unless it is ACCRUING with a bond below B_reg/4 (Caps), and when
  every member of A_h* is such a probationer all of them are (the all-probationer exception); on a bounded nonce
  domain (fork version 1 at the next height and CP nonceMode 0) A_h* is the members with lastShare or graceStart
  within W_act unless none is. When the answer depends on other members the gateway reads the ActList at the same
  height (`lengths`, whose heldTotal bounds every bond, and `activeSlice` pages, cached per head) and at most 32 other
  records; past that bound `proposerEligible` is `null`, never guessed. It also answers for records outside the
  active list, such as `registerReplacing` victims (status 2).
- `pocol_getRemovable` (cost 10) and `pocol_finality` (cost 1) are forwarded to the node. `pocol_getMiners` stays for
  compatibility.

The explorer's PHP allowlist (`deploy/websites/explorer.pocol.net/api/lib.php`) accepts the same four methods with
the same bounds.

## Integration boundary

The gateway installer and running service are not changed by adding this
directory. Its caller must replace the old module and pass the trusted schedule
as shown above during the coordinated release. Keep the schedule out of the
public `pocol_getBlockReward` request; that method remains a no-argument read.
Do not enable version 2 by accepting arbitrary browser-supplied heights.

## Validation

```sh
python3 -m unittest discover -s deploy/rewards -v
```

Tests use a deterministic independent RLP/RPC fixture with block-anchored state
reads. They cover activation boundaries, a v1 parent of the first v2 block,
maximum-nonce ownership, incorrect versions and untrusted schedules, moving
head races, the activity-window regression, reward arithmetic and PAY burns.

The v3 tests check the network configuration and the GenesisSpec proof, and the reward arithmetic against the
independent model's vectors (`prototypes/lp4-execution/tests/v3/*.json`: the M3 schedule, for every closed pot of
every chain scenario issuance + tips - share + the lagged base burn, and the scenario with a final block replayed
through the adapter's own settlement code to its end). `V3FlowTests` run the payment, closing-refund, reporter-cut and
tip-share code on a fake chain. `V3RecordedTapeTests` replay RPC answers recorded from the devnet chain below
(`testdata/v3-devnet-tape.json`): the honest answers give the recorded snapshots, and each tampered answer (a body
or an ancestor its header does not commit, a receipt or transaction list of another block, a ShareLedger, pot, carry,
burned total, closing cursor or record that differs from the reconstruction, a forged late-share window, winner or
fallback body, a changed anchor, and every answer field replaced by a value of another JSON type) fails closed.
`V3DevnetDifferential` runs only with `POCOL_V3_DEVNET` set. `prototypes/pocold/tests/v3_reward_gateway.rs`
(a default pocold test since the v3 rules became the release rules) sets it: it builds a 50-block v3 devnet chain with tips, late shares (one carried by the
block that settles their pot), two registerReplacing calls at the full set (a HELD and an ACCRUING victim, the second
payable and punishable after its exit), type-2 evidence against an ACCRUING and a HELD winner,
against a share owner who is not the winner, against the parent block's ACCRUING winner and against pot h-D's
ACCRUING winner in block h, a fallback block, the proposer's own registry call in its block, closing with its bond
refunds and the final block, serves it over pocold's JSON-RPC, and compares every block's reproduced payments, refunds,
bonds, burns, pots, carry and owners with the node's state (and the recorded tape with the chain's answers):

```sh
cargo +1.99.1 test --release --locked --manifest-path prototypes/pocold/Cargo.toml --test v3_reward_gateway
```

The node remains the source of truth for canonical execution. This adapter is
not a replacement for full block validation.

## Gateway startup integration

`gateway.py` is the explorer gateway with this adapter integration. The public
`pocol_getBlockReward` method still accepts exactly `params: []`; clients cannot
supply an activation height. The gateway passes an immutable, locally validated
schedule to the adapter on every reward read. No HTTP/RPC input can replace it.

For an operator-approved deployment, install `gateway.py` and `pocol_rewards.py`
together in the existing `/opt/pocol-explorer-gateway` service directory. Keep
its other installed modules and existing service arguments. With no new argument,
the gateway uses only `[[1, 0]]` and does not enable consensus version 2.

To configure the coordinated upgrade, create an operator-controlled JSON file
at `/etc/pocol-explorer-gateway/forks.json`. Its contents must be numeric pairs
`[[1, 0], [2, H]]`, replacing `H` with the exact approved activation height used
by the nodes. `H` must be a positive integer below `2^64`. Strings, booleans,
zero-height version 2, unknown versions, and extra entries are rejected. Keep
this configuration outside the website's document root and writable only by the
operator; it is not a downloadable public configuration endpoint.

Add this argument to the existing service's gateway command:

```sh
--fork-schedule /etc/pocol-explorer-gateway/forks.json
```

For a service using the default local ports, the complete command is:

```sh
python3 /opt/pocol-explorer-gateway/gateway.py \
  --bind 127.0.0.1 --port 8088 --rpc-port 8545 \
  --fork-schedule /etc/pocol-explorer-gateway/forks.json
```

The file is loaded and validated once, before the node connection is opened.
Changing the file has no effect on a running gateway; an operator restart is
required. The adapter then requires the node's advertised schedule to match the
configured schedule, rather than adopting the node's response. These files do
not alter the installed service or choose an activation height automatically.

The explorer's existing `pocol_getParams` sanitizer now retains the public fields
`forkSchedule`, `effectiveNonceMode`, `nextBlockNonceMode`, and
`consensusCompatibilitySha256`. The public RPC method allowlist is unchanged.

The Studio gateway (`deploy/studio/pocol_studio_gateway.py`) retains the same public
fields in its own sanitizer. Neither gateway adds transaction or read methods, and no
live deployment is performed by the repository tests.
