# Private PoCol testnet preparation

This is an operator-controlled, finite experimental network using the existing
`simnet` admission path, with real PoCol node processes on separate Linux servers.
It is not the production network profile, a public testnet, or independent security
acceptance. No consensus checks are disabled or changed by this package. The
experimental admission conditions and waivers remain visible in every node's status.

**Current deployment status: not deployed. Server addresses/access have not been supplied.**
The example addresses are placeholders. The evidence below is a single-host rehearsal.

## Pilot scope and trust

- Start with three dedicated Linux x86_64 servers on one isolated private LAN or VPN.
  Ubuntu 24.04 LTS with systemd and Python 3.10+ is the initial deployment target;
  build the binaries on a compatible OS/architecture. Docker is not required.
- Planning allowance per host: 4 vCPU, 8 GiB RAM and 40 GiB SSD. These are provisional
  pilot allocations, not measured minimums or a capacity guarantee. The unit has a
  7 GiB memory cap; exhaustion stops the service and must be investigated, not hidden.
- All machines and users with access must be trusted. **Miner and funded-account
  keys are deterministic from public labels**, inherited from `init-devnet`.
  Anyone with the labels can impersonate these miners or spend the test balances.
  A random chainId or genesis hash does not change this. Individual secure key
  custody and authenticated membership are prerequisites for inviting untrusted parties.
- The initial test accounts `alice` and `bob` each have 10^24 base units. There is no
  real value, public faucet, exchange bridge, or token sale. R_max=0 means miner
  registration/reactivation is disabled; the miner set starts from genesis.
- Timing uses the P11 values from `development/m1/reference/governance.md`: T_blk=10,
  tau=600, D_att=60, fallback wait=6 seconds; local delta=1, phi=1, t_exec=1,
  delta_rel=8, gamma=5. Other limits remain the existing small devnet defaults.
  This is **not** a claim that the complete P11 profile is implemented. `pocold`
  still inherits simnet retentionStress and its explicit admission waivers.
- H_CLOSE=19,000 and H_END=20,000 remain unchanged. At a nominal ten seconds per block,
  that is about 53/56 hours, not a perpetual chain or an observed deadline. Stop the
  pilot before closing; preserve evidence and prepare a new release/new chainId for
  a new experiment. Never silently rewrite genesis or delete data to restart it.
- `--state-store --checkpoint-every 128` is selected for this pilot. It does not
  change the application's default or prove production resource bounds.

## 1. Build and prepare once

From the repository root, on the build machine:

```sh
cargo +1.99.1 build --locked --release --bins --manifest-path prototypes/pocold/Cargo.toml
cp deploy/private-testnet/inventory.example.json /tmp/pocol-inventory.json
# Edit the copy with the actual private IPv4 addresses; keep it outside Git.
python3 deploy/private-testnet/private_testnet.py prepare \
  --inventory /tmp/pocol-inventory.json --out /tmp/pocol-private-release
```

Choose a chainId unique to this experiment, never 777001. The tool validates the
inventory, invokes the existing full ParamGate and copies the **same** genesis and
allocation to each bundle. It refuses an existing output directory. `prepare.log`
preserves admission output, including any refusal. No SSH, provisioning, firewall
change or remote start is performed.

Genesis time defaults to ten minutes after preparation. For a coordinated later
start, pass `--genesis-time UNIX_SECONDS`. Prepare near the planned launch time;
do not reuse an old idle genesis as a fresh performance experiment. Genesis files
must never be edited after a node starts.

Output includes `release.json`, `network/`, and a directory per node containing
the three binaries, network files, `node.json`, `runtime.py`, a service unit,
browser profile and checksums. Build provenance records the Git revision and
whether the source checkout was dirty; this is not a reproducible-build attestation.
Keep generated bundles, live inventories and chain data outside Git.

## 2. Host preflight and isolation

Before installing or starting anything, record on each host:

1. OS/CPU architecture and binary compatibility; `python3 --version`, `systemctl --version`.
2. Private interface address equals that node's inventory IP; routes to every peer
   use the private network. For hosts at different providers, establish an encrypted
   private VPN first; the package does not configure it.
3. Clock synchronization is active (`timedatectl status` and the time service's own
   offset report). Verify clock skew stays within phi=1 s and peer propagation meets
   delta=1 s under the pilot workload. A successful ping alone is not a bound.
4. Provider/host firewall: permit TCP P2P (30300 in the example) **only from the
   listed private peer IPs**. SSH is limited to the operator's authorized source.
   Deny public P2P ingress and all remote RPC ingress. Do not change SSH rules blindly.
5. Test the restrictions from an allowed peer and a disallowed host. RPC must listen
   only on 127.0.0.1, checked with `ss -ltnp` after start.
6. Disk space and memory availability; review admission on the actual hardware.
   A gate refusal blocks that node. Do not raise budgets or waive rules merely to start it.

The systemd IP allow-list adds isolation, but may have no effect if the host lacks
eBPF support. It is not a substitute for the verified provider/host firewall.
Official references: [systemd IP filtering and its limitations](https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html#IPAddressAllow=),
[SSH forwarding](https://man.openbsd.org/ssh#L),
[ExitOnForwardFailure limitations](https://man.openbsd.org/ssh_config#ExitOnForwardFailure).

## 3. Transfer and install each bundle

Transfer only the matching node directory and `install.sh` using authenticated SSH
with verified host keys. Do not disable host-key checking. Obtain that node's manifest
SHA-256 from the trusted build machine's `release.json`, separately from the upload.
Checksums detect corruption; they are not signatures or authentication of the author.

On a fresh dedicated host, after completing preflight:

```sh
sudo bash install.sh /absolute/path/to/node1 EXPECTED_MANIFEST_SHA256
sudo systemctl start pocol-private
sudo systemctl status pocol-private --no-pager
sudo journalctl -u pocol-private -n 100 --no-pager
```

Repeat with the matching bundle on node2 and node3. Installation does not start or
enable the service, overwrite an existing installation, or alter firewalls. Binaries
and configuration are root-owned; the service runs as `pocol`. State lives in
`/var/lib/pocol-private`, with a network identity marker and an exclusive process lock.
The wrapper verifies bundled files before exec; a second wrapper cannot run admission
against a live data directory. Do not run `pocold status` directly against live data.

After a successful pilot start and verification, `sudo systemctl enable pocol-private`
can enable boot persistence. The unit retries failures only three times in five minutes.
An unchanged invalid profile is not retried indefinitely.

## 4. Operator access and convergence check

In three separate terminals, using SSH aliases configured for the actual hosts:

```sh
ssh -N -o ExitOnForwardFailure=yes -L 127.0.0.1:18541:127.0.0.1:8540 pocol-node1
ssh -N -o ExitOnForwardFailure=yes -L 127.0.0.1:18542:127.0.0.1:8540 pocol-node2
ssh -N -o ExitOnForwardFailure=yes -L 127.0.0.1:18543:127.0.0.1:8540 pocol-node3
```

After at least seven blocks, on the operator machine:

```sh
python3 deploy/private-testnet/private_testnet.py check \
  --release /tmp/pocol-private-release \
  --rpc node1=http://127.0.0.1:18541 \
  --rpc node2=http://127.0.0.1:18542 \
  --rpc node3=http://127.0.0.1:18543 \
  --observe 180 --report /tmp/pocol-private-health.json
```

The check requires network and node identity, store mode, the honest experimental
admission scope, connected peers, block-height progress, and matching block/state/
receipt roots six blocks below the lowest head. It rejects differing networks,
non-progress and divergent roots. It is an operational smoke test, **not** finality,
security acceptance or proof that the processes live on different machines.
Retain host evidence separately. `ExitOnForwardFailure` cannot prove RPC availability;
the check does that through the tunnels.

The default observation is 180 seconds: the configured attempt duration is 60
seconds, and exhausting a finite nonce range can leave a healthy miner waiting for
the next attempt. The earlier 30-second rehearsal interval was shorter than one
attempt. This change tests eventual operational progress, not a ten-second block
latency guarantee; no progress in the selected interval still fails the check.

For Chrome, import a copy of `browser-profile.json` after changing its endpoint to
the appropriate local tunnel port. It deliberately uses LN (trust in the operator's
local/tunnelled node), not a claim of full state verification by a light client.

## 5. Transfers, shutdown and recovery

`bin/pocol-sign` signs test transfers; `bin/pocol-site` publishes/fetches test sites.
Both use the existing disposable label-derived accounts. See the [LP10 runbook](../../development/lp10/LP10-RUNBOOK.md)
for usage; replace its chainId and RPC port with this release's values. The automated
rehearsal below actually transfers 12345 base units from alice to bob and checks the
receipt and balance on every node.

Stop one host with `sudo systemctl stop pocol-private`; keep the data directory.
Back up the **whole stopped data directory**, release metadata and matching bundle
before upgrades. Do not copy only a live redb file. Restart with the same bundle/data,
then run the convergence check again. Restore backups to a separate stopped test host
and verify identity/replay before relying on them. No backup or restore test on the
eventual servers has yet been performed.

The installer deliberately refuses upgrades. For a planned upgrade: stop the node,
back up, review the new binary's storage compatibility, stage a root-owned replacement
bundle with the same network identity, verify it, switch the installation and restart.
Rollback requires the matching binary and compatible data snapshot; never assume a
new binary's database writes can be opened by an older one.

## Local checks and remote acceptance still required

```sh
python3 -m unittest discover -s deploy/private-testnet -p 'test_*.py' -v
python3 deploy/private-testnet/rehearse.py --work /tmp/pocol-private-rehearsal-new
```

The rehearsal keeps logs, manifests, journals and results in its new work directory,
including on failure. It uses three separate loopback addresses and real packaged
executables, a test-currency transfer, positive/negative identity checks, a duplicate
process check and restart/rejoin. All started processes are stopped afterwards.

Before calling the **distributed** pilot accepted, run the above on the actual hosts,
verify network isolation and clocks, perform a fresh transfer and site round-trip,
restart each host, test one controlled peer disconnection/reconnection, and retain
common-chain evidence during an agreed observation period before H_CLOSE. Separate
failure/stress campaigns and the production boot blockers remain future work.

Implementation sources: [node CLI](../../prototypes/pocold/src/main.rs),
[admission scope](../../prototypes/pocold/src/lib.rs),
[disposable genesis keys](../../prototypes/lp4-execution/src/devnet.rs),
[production boot blockers](../../development/audit-2026-10-22/BOOT-BLOCKERS-2026-10-28.md).
