# Independent keys and public-input genesis — experimental foundation

These commands remove the dependence on public label-derived signing secrets.
They do not close the [public launch gates](PLAN.md). Run the integration on loopback
until network acceptance is complete. Linux/Unix key files retain strict ownership and mode checks. The later
[standalone launcher](../../deploy/standalone/README.md) also supports Windows
through user-bound DPAPI ciphertext; it never treats a raw Unix key as a safe
Windows file. Android uses its app-private Unix storage with the same key checks.

## Generate keys at their owner

Build the release binaries from the exact reviewed source:

```sh
cargo +1.99.1 build --locked --release --bins --manifest-path prototypes/pocold/Cargo.toml
mkdir -m 700 /absolute/private/key-directory
prototypes/pocold/target/release/pocol-key generate \
  --out /absolute/private/key-directory/miner.pocol-key
prototypes/pocold/target/release/pocol-key generate \
  --out /absolute/private/key-directory/reward.pocol-key
```

On Unix, each command prints **only the public address**. The secret is 32 binary bytes,
generated through OS randomness, created exclusively with mode 0600 and fsynced.
Existing paths are never overwritten. Read the address again with:

```sh
prototypes/pocold/target/release/pocol-key address \
  --key-file /absolute/private/key-directory/miner.pocol-key
```

Files must be regular, have one link, be owned by the running user, have no group/
other permissions and contain a valid scalar. Final-component symlinks are refused.
The parent directory and host must also be protected. These files are **plaintext**,
not encrypted keystores; they are not intended for real-value assets. Temporary
secret input buffers are zeroized; no claim of complete process-memory erasure or
protection against a compromised operating system is made. Keep a controlled offline
backup, test recovery to the same address and retain access to reward keys.

For a service, generate the miner key as the service user in a separate private key
directory. The node must run as that same user. Do not copy every miner's secret to
every host. No shared private-key list is needed to construct genesis. `*.pocol-key`
is ignored by Git, but that does not replace checking release and artifact contents.

## Construct a candidate using public information only

Create `public-genesis.json` with actual public addresses:

```json
{
  "chainId": 777912,
  "genesisTime": 1793275200,
  "registrationPerBlock": 1,
  "miners": [{"id": "0x1111111111111111111111111111111111111111", "reward": "0x2222222222222222222222222222222222222222"}],
  "accounts": [{"address": "0x3333333333333333333333333333333333333333", "wei": "1000000000000000000000000"}]
}
```

The addresses and time above are placeholders, **not usable signing identities or
a launch schedule**. Use fresh unique chainId/time and distribute one generated
network directory; do not independently regenerate genesis on different hosts.

```sh
prototypes/pocold/target/release/pocold init-testnet \
  --config public-genesis.json --out new-network-directory
```

The schema rejects unknown/duplicate fields, duplicate allocation/miner addresses,
reserved identities, invalid amounts and the production chainId. It preserves an
explicit reward allocation while setting its required durable nonce. It then derives
the roots and runs the existing full experimental admission before creating output.
Output contains `genesis.spec.hex`, `alloc.json`, `devnet.json` and public identities;
no private keys. R_max is explicitly bounded by the requested value and the gate.
No miner count/caps/gas policy is inferred from a hostname.

Candidate timing is P11's timing with the existing small-state experimental limits;
synthetic difficulty, retentionStress and admission waivers remain visible. This
does not constitute a production calibration or the complete normative P11 profile.

## Run/sign without labels

For a local test, with each process using a different data directory and ports:

```sh
prototypes/pocold/target/release/pocold run --network new-network-directory \
  --data node-data --name node1 --miner-key-file /absolute/private/key-directory/miner.pocol-key \
  --listen 127.0.0.1:30300 --rpc 127.0.0.1:8540 --state-store \
  --delta 1 --phi 1 --t-exec 1 --delta-rel 8 --gamma 5
```

Use `--peer IP:PORT` for known peers. `pocol-sign --key-file FILE` and
`pocol-site publish --key-file FILE` accept independent account keys instead of
`--account LABEL`. Never supply both. `--miner-key-file` is mutually exclusive with
`--miner LABEL`; missing or duplicate key selectors fail. The old label switches
remain solely for existing disposable devnet fixtures.

The first private-testnet bundler in `deploy/private-testnet/` still uses disposable
labels. The later [public operations candidate](../../deploy/public-testnet/README.md)
uses independent public addresses and per-service local keys. It has its own launch
limitations and does not turn the private label-based bundle into a public release.

## Enrollment exercise

The local integration starts three genesis miners and a fourth key that is absent
from genesis. A funded reward account first sends a transaction so it has a durable
nonce. The fourth key signs `register(address)` to registry
`0x0000000000000000000000000000000000c0c001`, deposits the configured bond, enters
membership through normal execution and mines. A repeated registration must revert.
This proves the exercised path only, not unrestricted capacity, economic incentives
or resistance to Sybil/denial-of-service attacks.

```sh
python3 prototypes/pocold/tests/independent_keys.py -v
python3 prototypes/pocold/tests/e2e_independent_keys.py --work /tmp/pocol-key-test-new
```

The work directory contains real test secrets under `secrets/`; **never upload it**.
Only `public-evidence/` is eligible for review/artifacts. Tests check generation,
permissions, symlink/hardlink rejection, conflicting options, genesis validation,
signed transfers, enrollment, duplicate rejection, site publication/fetch and restart.
