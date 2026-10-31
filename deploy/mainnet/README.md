# PoCol Mainnet: coordinated same-chain rollout

The operator-designated network is **PoCol Mainnet**. Its existing identity is:

| Field | Value |
|---|---|
| Chain ID | `1599846715` |
| Genesis hash | `0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca` |
| Currency | `PCOL` |
| Mining range after the approved v2 height | `0` through `18,446,744,073,709,551,615` |

The name/environment metadata identifies this network. Technical admission fields,
`profileKind`, and `publicLaunchReady: false` retain their existing meaning. This
rollout preserves the genesis, balances, contract addresses, wallets and history.
It does not rewrite the reward formula or difficulty policy.

Keep the existing service account, `/opt/pocol-testnet`, `/var/lib/pocol-testnet`,
`/etc/pocol-testnet`, and `pocol-testnet.service` names. Their historical names are
installation paths; renaming them is unnecessary and could separate a node from its
existing state or credentials.

## 1. Collect the actual server state

Run this source-reviewed command **locally on each Linux seed** through the
operator's existing authenticated server session. It requires Python 3.10+ and
permission to read the installed public bundle and public state binding:

```sh
python3 -B /path/to/pocol/deploy/mainnet/preflight.py \
  --rpc http://127.0.0.1:8545 \
  --bundle /opt/pocol-testnet \
  --data /var/lib/pocol-testnet \
  --service pocol-testnet.service \
  --out /tmp/pocol-mainnet-preflight.json
```

Use the actual existing paths/ports if the installation differs. The output path
must be new. Exit status `0` means the bounded checks passed; `2` means a mismatch
or unavailable prerequisite must be resolved. A report is still produced for
normal observation failures. A `PASS` is an inventory result, not a deployment or
an independent production-readiness assessment.

The manifest check establishes bundle consistency, not publisher authentication;
verify the release checksum through the operator's trusted release channel. Run
against a controlled public bundle with no concurrent local file changes. The
inventory scan is not designed to resist another local process swapping files.
The exact persisted node/miner/protocol binding must match the installed bundle
and live node. If the head reorganizes or crosses activation during collection,
the report may fail its consistency check; collect a fresh report.

The script reads only three RPC methods: `pocol_getParams`, `pocol_status`, and
`eth_getBlockByNumber`. RPC is restricted to explicit loopback HTTP addresses. It
reports the current head, chain/genesis, source compatibility, installed and
running schedules, public bundle checksum, public state binding, disk availability,
and selected systemd unit states/paths. It never reads wallet keys, credential
files, process arguments, service environments, balances, or account histories.
It does not install files, repair the chain, stop/start services, or choose a fork
height. The only optional write is the sanitized report requested with `--out`.

Compare reports from all seeds. Their chain/genesis, source hash and schedules
must match. Re-run at one common existing block height to compare canonical hash,
state root and receipts root precisely:

```sh
python3 -B /path/to/pocol/deploy/mainnet/preflight.py \
  --check-height COMMON_EXISTING_HEIGHT \
  --out /tmp/pocol-mainnet-common-block.json
```

Replace `COMMON_EXISTING_HEIGHT` with an integer below every reported head. This
parameter only selects a historical read; it is **not** the upgrade activation
height. Use the same installation options as the first command if non-default.
Collect a second fresh report to confirm heads are advancing. The earlier finite-
testnet monitor assumes a numeric closing height and should not be used unchanged
for a continuous network.

## 2. Build one compatible release and prepare the public plan

Use the merged release revision and its successful checks for all native binaries.
Collect `pocol build-info` from the resulting Linux and Windows packages and verify
matching compatibility hashes. Keep existing Android application signing identity;
an unrelated signing key cannot update an installed application.

Only after collecting fresh seed reports, agree one future activation height `H`
that leaves time for the coordinated rollout. No height is preselected by this
source change. Prepare the reviewed invitation using the existing helper:

```sh
python3 -B deploy/standalone/prepare_nonce_upgrade.py \
  --network CURRENT_NETWORK_JSON \
  --build-info NEW_BUILD_INFO_JSON \
  --activation-height H \
  --observed-head CURRENT_CANONICAL_HEAD \
  --out-dir full-nonce-plan
```

The resulting invitation keeps the genesis and allocation bytes unchanged. It adds
`[[1,0],[2,H]]` to the trusted schedule and binds the new native release. The output
also includes a browser schedule with decimal-string heights and a checksum report.

## 3. Stage each seed without changing its live installation

On the build/operations workstation, prepare a new bundle for each existing seed:

```sh
python3 -B deploy/public-testnet/nonce_upgrade.py prepare \
  --old-bundle OLD_VERIFIED_BUNDLE \
  --expected-old-manifest OLD_MANIFEST_SHA256 \
  --bin-dir NEW_NATIVE_BINARIES \
  --network full-nonce-plan/network.pocol.json \
  --out NEW_SEED_BUNDLE
```

The preparation command checks source/build identity and preserves the seed's node
name, miner address, peers, endpoints, service definition, genesis and allocation.
Record each new manifest SHA256 through the authenticated release channel. Keep
the previous verified bundle available separately for binding verification and
recovery. Preparing a bundle does not install it on a server.

## 4. Perform the coordinated stopped upgrade

Follow the full [nonce upgrade procedure](../standalone/FULL-NONCE-UPGRADE.md).
It describes existing-home upgrades, Android's explicit upgrade action, and the
Chrome verifier's locally trusted fork schedule. The service-bound seed operation
is:

```sh
python3 -B deploy/public-testnet/nonce_upgrade.py migrate \
  --old-bundle OLD_VERIFIED_BUNDLE \
  --new-bundle NEW_VERIFIED_BUNDLE \
  --expected-new-manifest NEW_MANIFEST_SHA256 \
  --data /var/lib/pocol-testnet
```

Run it only while that seed is stopped and its existing recoverable state backup
is available. It acquires both existing locks, validates the journal and commit
records without repairing them, refuses a height at/below any stored branch, and
updates only the release binding with a backup and resumable intent. Repeating the
same interrupted operation is supported before activation. It does not install the
bundle or start the service. Keep the original installation paths and credentials
when switching to the reviewed new bundle.

All participating nodes must run the same source and schedule. Different schedules
or builds fail the peer handshake. Once v2 blocks exist, recovery must use a
compatible v2 release; reverting to a v1-only schedule is not a recovery method.

## 5. Update public integrations and verify

- Install the [reward gateway and adapter](../rewards/README.md) together, preserving
  its other modules and service arguments. Pass the approved numeric fork schedule
  through its local `--fork-schedule` file. Public requests cannot select `H`.
- Public parameter/status filtering may expose `networkName` and
  `networkEnvironment`, plus the reviewed fork/effective-mode fields. Keep private
  node metadata, local paths and RPC internals excluded.
- Update the Chrome extension and its locally approved schedule, and publish the
  corresponding main site, Studio, IDE and Explorer packages through their existing
  hosting workflows. These static packages cannot activate a node consensus fork.
- After restarting the coordinated seeds, collect fresh preflight reports and
  compare one common block's hash and roots. At `H`, the active consensus version
  must become `2`, and the effective nonce mode must become `1` (full 64-bit range).
  Genesis/chain ID and original wallet identities must remain unchanged.

Remote server access and actual installed paths are established by the operator's
server sessions and reports. Source changes and locally prepared packages alone
do not establish that any remote service or website has been updated.

## Local checks

```sh
python3 -B -m unittest discover -s deploy/mainnet -p test_preflight.py -v
python3 -B -m unittest discover -s deploy/public-testnet -p test_nonce_upgrade.py -v
python3 -B -m unittest discover -s deploy/rewards -v
```

Sources: [`identity.rs`](../../prototypes/pocold/src/identity.rs),
[`chain.rs`](../../prototypes/pocold/src/chain.rs),
[`node_operator.py`](../public-testnet/node_operator.py), and
[`nonce_upgrade.py`](../public-testnet/nonce_upgrade.py).
