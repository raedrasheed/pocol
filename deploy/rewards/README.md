# Reward projection of the v3 network

`pocol_rewards.py` is the read-only latest-block reward adapter of the public explorer gateway (`gateway.py`). It
reads a v3 (spec-2) network only: the v3 rules (V3-DESIGN.md, V3-DECISIONS.md) are the release rules (consensus rules
2, GenesisSpec version 2), and the answer has schema `pocol-block-reward-v3`.

The adapter does not submit transactions, modify state, change difficulty, or
prove that a projected payment has settled. It reads the captured block's state
and returns `status: "pending"`; evidence and payment eligibility can still
change before settlement.

## Trusted configuration

The network and the fork schedule are operator configuration, never RPC answers or public HTTP parameters. There is
no built-in network: `snapshot_v3` and `decode_header` need one.

```python
from pocol_rewards import snapshot_v3, validated_network

network = validated_network({'chainId': chain_id, 'genesisHash': genesis_hash, 'specVersion': 2, 'v3': True,
                             'source': 'https://explorer.pocol.net'})
result = snapshot_v3(node_query, fork_schedule=trusted_schedule, network=network)
```

`chain_id`, `genesis_hash` (lower-case `0x` hex), the optional `source` (an https URL the answer names) and
`trusted_schedule` come from the operator's deployment configuration. The adapter requires the node's
`pocol_getParams` to name that chain and genesis, its `genesisPre` to hash (keccak256) to the configured genesis and
to decode as a version-2 GenesisSpec with the 59 CP items of CP schema v2, nonceMode 1 (the full nonce domain, the
only mode a v3 genesis admits) and the v3 program set's sysCodeHash (`V3_SYS_CODE_HASH`, recomputed by the tests from
`system-v3.json`), and the node's advertised fork schedule to equal the configured one.

The trusted fork schedule defaults to `[[1, 0]]`: the v3 profiles name no fork. It only checks each header's version
against its height and the cap raise: fork version 2 is a kept no-op, and fork version 3 is the cap raise (FN-5,
`MAX_FORK_VERSION` 3, as in pocold's profile), whose node-advertised `capRaise` must sit at the schedule's fork-3
height. The moving-head `activeVersion` field is not used to choose rules for a previously captured block.

The answer has schema `pocol-block-reward-v3` and stays `status: "pending"`. For the captured block h it reproduces:

- **pot h**: issuance M3 (`issuance(h)`, V3-19) plus the block's tips (its receipts) less the signed proposer's tip
  share (V3-20) plus the carry into pot h, the base burn `R - A` of `settle(h-1-D)` lagged by one block (V3-15). The
  sum is checked against RewardEscrow's pending pot and the carry against `RewardEscrow.carry()`. Its projected
  settlement uses the header's owners under the all-live membership (D-V3-01: every live member of the parent's
  ActList owns a range of the full u64 nonce domain, without any activity filter); `projectedCarryWei` is its
  uncovered pool part, which goes to the next pot rather than burning. Late shares, evidence and registry changes before
  h + D can still change it. Once the registry is closing (`closing`), the final block may settle pot h
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

The gateway requires `--v3-network FILE` (a JSON object as above, at most 4096 bytes, read once at startup); the
public `pocol_getBlockReward` method stays a parameterless read. A reward read may call the node's block, header, state
and `eth_getTransactionReceipt` reads only, within `MAX_CALLS_V3` calls and `MAX_SECONDS_V3` seconds.

### v3 membership reads (Sites W6)

The gateway also serves the explorer's membership reads, each at one head block (every
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
  every member of A_h* is such a probationer all of them are (the all-probationer exception). A_h* is every live
  (non-tomb) member of the ActList: v3 has no activity window. When the answer depends on other members the gateway reads the ActList at the same
  height (`lengths`, whose heldTotal bounds every bond, and `activeSlice` pages, cached per head) and at most 32 other
  records; past that bound `proposerEligible` is `null`, never guessed. It also answers for records outside the
  active list, such as `registerReplacing` victims (status 2).
- `pocol_getRemovable` (cost 10) and `pocol_finality` (cost 1) are forwarded to the node. `pocol_getMiners` stays for
  compatibility.

The explorer's PHP allowlist (`deploy/websites/explorer.pocol.net/api/lib.php`) accepts the same four methods with
the same bounds.

## Integration boundary

The repository tests change no installed service. Keep the network and the schedule out of the public
`pocol_getBlockReward` request; that method remains a no-argument read.

## Validation

```sh
python3 -m unittest discover -s deploy/rewards -v
```

The tests check the network configuration (required, no default) and the GenesisSpec proof (nonceMode 1 only), and the reward arithmetic against the
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

Public host (owner decision 9, 2026-11-16): the explorer gateway is served at `https://explorer.pocol.net` (the Caddy
site `explorer.pocol.net`: `/rpc` and `/health`, with the Studio relay route `/studio/*` in the same block), replacing
the old PHP site there; the pocol.net reward page reads `https://explorer.pocol.net/rpc`. The owner moves the DNS record
at launch; nothing here touches DNS or servers. Serving the explorer snapshot and its same-origin `/api/*.php` reads from
that host is open work (deploy/websites/README.md, Explorer host).

`gateway.py` is the explorer gateway with this adapter integrated (a required import, installed beside it; the GeoIP
and transaction index modules stay optional). The public `pocol_getBlockReward` method still accepts exactly
`params: []`; clients cannot supply a network or a schedule. The gateway passes its immutable, locally validated
configuration to the adapter on every reward read. No HTTP/RPC input can replace it.

For an operator-approved deployment, install `gateway.py` and `pocol_rewards.py` together in the
`/opt/pocol-explorer-gateway` service directory, and create operator-controlled files outside the website's document
root, writable only by the operator (they are not downloadable public configuration):

- `/etc/pocol-explorer-gateway/network.json` (required): the network, taken from the network's invitation
  (`network.pocol.json`: its GenesisSpec's chain ID and keccak256 hash), for example
  `{"chainId": N, "genesisHash": "0x...", "specVersion": 2, "v3": true, "source": "https://explorer.pocol.net"}`.
  Keep `"source"` while the pocol.net reward page checks it.
- `/etc/pocol-explorer-gateway/forks.json` (optional): numeric pairs of the nodes' exact schedule, `[[1, 0]]` by
  default; `[[1, 0], [2, H2], [3, H3]]` for a cap raise at H3. Heights are integers below `2^64`, strictly increasing;
  strings, booleans, unknown versions and extra entries are rejected.

For a service using the default local ports, the complete command is:

```sh
python3 /opt/pocol-explorer-gateway/gateway.py \
  --bind 127.0.0.1 --port 8088 --rpc-port 8545 \
  --v3-network /etc/pocol-explorer-gateway/network.json
```

and with a scheduled cap raise:

```sh
python3 /opt/pocol-explorer-gateway/gateway.py \
  --bind 127.0.0.1 --port 8088 --rpc-port 8545 \
  --v3-network /etc/pocol-explorer-gateway/network.json \
  --fork-schedule /etc/pocol-explorer-gateway/forks.json
```

Without `--v3-network` the gateway exits (status 2) before it connects to the node, as it does for an invalid file.
Both files are loaded and validated once, before the node connection is opened; changing them has no effect on a
running gateway, and an operator restart is required. At startup the gateway refuses a node that does not name the
configured chain ID and genesis hash.

**Operator step at deployment.** The live explorer service unit is not in this repository. Its `ExecStart` must gain
`--v3-network` (and the network file must exist) when this gateway is installed: an unchanged unit, with only the old
arguments, fails at startup. The pocol.net reward page (`deploy/websites/pocol.net/api/reward.php`, `reward.js`) still
validates the retired `pocol-block-reward-v1` answer and is switched to `pocol-block-reward-v3` separately.

The explorer's `pocol_getParams` sanitizer retains the public fields `forkSchedule`, `capRaise`,
`effectiveNonceMode`, `nextBlockNonceMode` and `consensusCompatibilitySha256`. The public RPC method allowlist is
unchanged.

The Studio gateway (`deploy/studio/pocol_studio_gateway.py`) retains the same public
fields in its own sanitizer. Neither gateway adds transaction or read methods, and no
live deployment is performed by the repository tests.
