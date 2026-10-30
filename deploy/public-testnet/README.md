# Public testnet operations candidate

This package advances public-network preparation. It is **not a public launch
clearance**: the existing experimental admission conditions, resource/proof gaps,
finite lifetime and missing real-host acceptance remain. Test coins have no monetary
value. No infrastructure, public endpoint or economic mainnet is created by these
scripts. See `development/public-network/PLAN.md` in the source repository.

Supported deployment target: dedicated Linux x86-64 hosts with systemd, Python 3.10+
and the release binaries built on a compatible distribution. Use three separate
hosts initially. Reserve at least 8 GiB RAM per host, with a 7 GiB service limit;
measure disk needs and free space on the target profile before launch. These are
operating prerequisites, not proof that the unresolved near-cap resource bounds fit.

## 1. Independent operator keys and one genesis

Each miner operator generates their own key on their own host using the reviewed
`pocol-key generate --out /etc/pocol-testnet/miner.pocol-key` command, in a root-owned
0700 directory. The tool creates a 0600 plaintext key; protect it and make an encrypted
offline backup under the operator's custody. Only its printed public address is
collected. Never submit secret bytes in chat, Git, inventory or public bundles.
Reward and faucet accounts should have separate operator-held keys.

Prepare one public allocation/configuration using `development/public-network/KEYS.md`.
Use `pocold init-testnet --config public-genesis.json --out network` once. Distribute
exactly those three genesis files to everyone; never regenerate genesis per host.
The current builder still uses a synthetic difficulty fixture. Actual hardware
calibration and an adopted public profile remain launch requirements.

Build all four binaries together, with Rust 1.99.1 and the checked-in lockfile:

```sh
cargo +1.99.1 build --locked --release --bins --manifest-path prototypes/pocold/Cargo.toml
python3 -B deploy/public-testnet/prepare.py --inventory inventory.json --network network --out /tmp/pocol-release
```

Replace every example inventory placeholder with an actual operator-controlled
endpoint/public address. The preparer validates public IPv4 addresses, safe hostnames,
unique miners, distinct endpoints and a maximum of seven configured outbound peers.
It rejects stale binaries, test-feature builds and label-derived genesis. It copies
no private keys. `--rehearsal` permits loopback only and marks bundles non-installable.
`release.json` records per-node manifest hashes and the strict implementation identity.
Distribute these hashes through an authenticated channel separate from the bundles.
SHA-256 checks are integrity checks, not a release-signing trust system.

## 2. Fresh host installation and exposure

Inspect the installer from the trusted source checkout before running it as root:

```sh
sudo bash deploy/public-testnet/install.sh /absolute/path/to/seed-1 TRUSTED_MANIFEST_SHA256
```

The installer verifies a root-owned snapshot before running bundled code, refuses
existing installations/state, and installs an inactive service. It does not generate
keys, enable services, install a TLS certificate or change a firewall. The unit uses
systemd `LoadCredential` to deliver the local miner key. Before launch, the service
copies that credential to its own mode-0600 file in the mode-0700
`/run/pocol-testnet` runtime directory. This satisfies the unchanged strict key-file
checks; systemd removes the runtime directory on stop. The original root-owned key
and its temporary runtime copy stay outside both the bundle and the state backup.
The runtime copy is plaintext and is not an encrypted keystore or HSM.

Before activation on actual hosts:

1. Confirm clock synchronization (`timedatectl show -p NTPSynchronized --value`),
   clock error, RTT/jitter and execution timing against the admitted local assumptions
   (delta=1 s, phi=1 s, t_exec=1 s, delta_rel=8 s, gamma=5 s). A synchronized flag alone
   does not establish these bounds. Do not activate if they fail.
2. Restrict SSH to operator IPs. Permit P2P TCP 30300 and TLS TCP 443 only as needed;
   the native RPC binds to 127.0.0.1. Check cloud security groups and host firewall.
   Do not flush an existing firewall or expose native RPC on 0.0.0.0.
3. Resolve your RPC DNS name to the correct host and install a valid certificate at
   `/etc/pocol-testnet/tls/{fullchain.pem,privkey.pem}`. Test the dedicated gateway
   config with `nginx -t -c /opt/pocol-testnet/nginx.conf` before enabling it.
4. Review and close the source launch ledger before Internet activation. The Nginx
   template limits per-IP and total HTTP requests/connections, but does not close
   the node's unresolved RpcGuard/precompile or consensus execution obligations.
   Run the hostile traffic, partition, recovery and soak acceptance on actual hosts.
5. Once accepted, start `pocol-testnet.service` and the reviewed gateway configuration.
   Observe logs (`journalctl -u pocol-testnet`) and the cross-node checker below.

The gateway is a standalone config for a dedicated Nginx instance, not an arbitrary
drop-in for a shared web server. It retains `Origin` so it cannot bypass the node's
origin restrictions. The supplied configuration assumes direct client connections;
do not add trusted-proxy/real-IP settings without the upstream trust configuration.
Nginx's connection limit counts processed requests, not every partial TCP handshake;
use host/provider ingress protections as well.

## 3. Joining and funding

Anyone can generate an independent key and request test coins from the operator.
The initial distribution remains an operator-signed transfer: an unauthenticated
automatic faucet is not included. A joining operator uses the same release source
and published genesis with a one-node inventory and `prepare.py --join`; set one or
more published seeds. A `null` minerAddress produces an observer with no key input.
An independent miner can start before registration but earns no mining membership
until its ordinary registry transaction succeeds. Follow the registration/bond
commands in `development/public-network/KEYS.md`; the reward account must be durable.
There is no allowlist of miner addresses in P2P. A source fingerprint is a compatibility
check, not identity authentication or Sybil resistance.

## 4. Monitoring

Supply every node's own HTTPS endpoint, or distinct loopback SSH tunnels:

```sh
python3 -B deploy/public-testnet/monitor.py --release /tmp/pocol-release/release.json \
  --rpc seed-1=http://127.0.0.1:18540 --rpc seed-2=http://127.0.0.1:28540 \
  --rpc seed-3=http://127.0.0.1:38540 --observe 180 --out observation-001.json
```

The checker requires matching genesis/build identity, compatible peers, healthy
history, advancement on every node, and identical confirmed hash/state/receipt roots.
It refuses redirects and oversized responses. The 180-second window covers three
60-second mining attempts; it does not promise ten-second block latency. It writes
immutable JSON and Prometheus text. Alert on checker failure, an observation older
than 360 seconds, no peers, low disk, repeated service restarts and closing-height
proximity. Archive each sample; do not feed duplicate node series from many old
`.prom` files into one textfile collector.

## 5. Offline backup, restore and rollback

Stop the service first. A backup takes both the operator lock and Rust journal lock;
an active process makes it fail. It copies the complete state with checksums and
fsync, and copies no keys. Keep the manifest hash separately:

```sh
sudo systemctl stop pocol-testnet
sudo python3 -B /opt/pocol-testnet/node_operator.py backup --out /backups/pocol-001
sudo python3 -B /opt/pocol-testnet/node_operator.py restore --backup /backups/pocol-001 \
  --expected-sha256 TRUSTED_BACKUP_MANIFEST_SHA256 --data /var/lib/pocol-testnet-restored --owner pocol-testnet
```

Restore requires an absent destination and matching chain, genesis, implementation,
node and miner address. Root must name the service account; restored files/directories
receive its UID/GID with modes 0600/0700. It never overwrites or starts existing state.
When selecting the restored path in the unit, update BOTH `ExecStart` (add
`--data /var/lib/pocol-testnet-restored`) and `StateDirectory=pocol-testnet-restored`.
The latter grants the correct writable path under `ProtectSystem=strict`; changing
only the command is insufficient. Reload systemd, start the restored instance, and
compare confirmed roots through RPC. Keep the original directory until the recovery
drill passes, and keep the original unit for rollback. Never run both instances.

Wire v2 refuses old handshakes or any different compatibility fingerprint. A code
change therefore needs a coordinated testnet upgrade and an explicit state migration
decision; these scripts do not rewrite state bindings to make an upgrade appear
compatible. For a failed same-release deployment, stop the service and restore its
matching prior snapshot and binaries. Never run two copies of one miner key at once.

## Lifetime and unresolved work

The current profile closes at height 19,000 and ends at 20,000: approximately 53/56
hours at nominal ten-second blocks, with actual elapsed time varying. This package
does not remove those consensus rules or inflate cumulative caps. A perpetual network
requires the remaining protocol/resource/economic design. Public launch still needs
approved public parameters, target-hardware calibration, admission/storage/RPC work,
real multi-host/Internet acceptance, and independent security review.

References for service behavior:
- https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#Credentials
- https://nginx.org/en/docs/http/ngx_http_limit_req_module.html
- https://nginx.org/en/docs/http/ngx_http_limit_conn_module.html
- https://nginx.org/en/docs/http/ngx_http_ssl_module.html
