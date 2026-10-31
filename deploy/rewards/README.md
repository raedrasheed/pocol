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

`studio-fork-fields.patch` makes the corresponding sanitizer-only change to the
separately installed `pocol_studio_gateway.py`. Review it against that service's
current source and apply it from the directory containing the file:

```sh
patch --dry-run -p1 < /path/to/studio-fork-fields.patch
patch -p1 < /path/to/studio-fork-fields.patch
```

That patch adds no transaction or read methods and changes no authentication,
request validation, or limits. Restart the Studio gateway through its existing
service management after applying the reviewed patch. Both gateway changes must
be included in the coordinated rollout; no live deployment is performed by the
repository tests.
