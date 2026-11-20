# deploy/mainnet: launching the new PoCol network

PoCol launches a brand-new network from a fresh genesis
([owner decisions, 2026-11-13](../../development/decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)):
new keys, a new chain ID, four founding miners, and nothing carried over from the retired network. PoCol Mainnet
launches as a public network directly from its ceremony, without a separate rehearsal
([owner decisions, 2026-11-20](../../development/decisions/OWNER-DECISIONS-2026-11-20.md) D1). Nothing in this
directory deploys anything, and no step below runs on a server without the owner's approval.

## Launch runbook (stub)

This runbook is completed once the genesis parameters are fixed. The steps, in order:

1. **Ceremony packages.** Build the packages the ceremony runs on with `POCOL_RELEASE=1` from a pinned tree in CI
   (five-device-build). The mainnet ceremony runs on those CI artifacts, which the owner downloads
   from that CI run and checks against its `SHA256SUMS`: integrity only, not a release signature. No signed release
   exists yet: the assembler refuses every release until the identity switch (step 7), and the switch changes the
   native binaries (`network_display.rs`) as well as the extension version, so these packages are never published.
2. **Prepare the servers.** The three founder servers, `108.165.128.89`, `108.165.128.90` and `108.165.128.91`
   (D3; also the seeds), start from a clean install (`deploy/standalone/install-linux.sh` refuses a host that
   already has a PoCol install). Each has 4 GB of RAM, a risk the owner accepted (D2; 8 GB is recommended). The
   owner's computer runs the fourth founding miner; the phone is a wallet.
3. **Fresh keys.** Each founding miner generates its miner key on its own device with the independent-key flow
   (`pocol-key`); only the public miner ID leaves the device. Founders 1-3 (the servers) are paid to owner-held
   reward addresses (owner decision 2026-11-16): the owner makes three of them offline with `pocol-key generate` and
   `pocol-key address` on a device that is never a server, and passes them to the ceremony as
   `-ServerRewardAddresses`. Each offline key is backed up encrypted with `pocol-key export --key-file FILE
   --out BACKUP` (passphrase read twice from stdin), and the backup checked without writing any file with
   `pocol-sign --chain-id ID --address --backup BACKUP`, which prints the same address; `pocol-sign --backup BACKUP`
   signs from it. Only the 20 PCOL bond is tied to each server. Founder 4 (the owner's PC) keeps its own
   address. Each reward account starts at nonce 1 (PAY-02): its first offline transaction uses `pocol-sign --nonce 1`.
4. **Genesis.** Start from the committed profile, `v3-profile.json` (`v3-rehearsal-profile.json` is only a test and
   devnet profile, never a launch step).
   It holds the chain ID and the consensus parameters recorded in
   [FRESH-NETWORK-CP.md](../../development/decisions/FRESH-NETWORK-CP.md), and the four founders' allocations
   (999,980 PCOL each, plus the 20 PCOL HELD bond minted into Escrow). Fill its `PLACEHOLDER` values: the launch
   second, the founders' measured aggregate hash rate (`pocol hashrate` on the four founder devices), each founder's
   public miner ID and reward address (the
   owner-held ones for founders 1-3). Then
   run `pocold init-testnet --config PROFILE.json --out NEW_DIR` once, and record the printed chain ID and genesis
   hash. Its summary reports the admission (scope public-network, the network profile on a network node, never
   specification compliance, with its conditions and the owner's waivers) and, since no release pins the new genesis
   yet, `publicLaunchApproved` false with `network-not-release-pinned` and `launch-not-approved`. While any
   placeholder remains, the builder refuses. `--test-placeholders` fills them with public test keys
   for a local test genesis only; that genesis runs on the local test chain ID 777990 (the profile's ID is recorded
   as `profileChainId`), is marked `testPlaceholders` and is never a launch genesis.
5. **Invitation.** Pack the genesis files with the three seed addresses into the network invitation
   (`pocol network`), and check its fingerprint on every founding device.
6. **Founders boot on the genesis.** The three servers and the PC start from the invitation; this genesis is the
   launch genesis (no rehearsal, no wipe and repeat). Each founder checks with `pocol_status` that its node reports
   the recorded chain ID and genesis hash, `networkScope` public-network and `networkEnvironment` mainnet, and the
   computed approval and readiness, unchanged by any label.
7. **Pin the identity in the clients.** Switch every client from the identity the tree pins today to the new
   one with `set_network_identity.py` (below), in one reviewed PR, and release the clients: rebuild the native
   packages and the 0.5.0 extension from the switched tree, then assemble, sign and verify them
   (`deploy/release/README.md`, owner procedure). This is the first signed release; no package is signed before it.
   The switch writes the release pin with the approval reference "OWNER-DECISIONS-2026-11-20 D1", so a node of this
   release booted on PoCol Mainnet reports `publicLaunchApproved` and `publicLaunchReady` true while its
   `technicalReadiness` still lists what is open. The same PR publishes on the
   pocol.net extension and download pages the two values users compare: the extension ID
   `gggcoflmedlhlkcdhebmoepmggdnafmp` (`prototypes/pocol-extension/release/manifest-key.pub`) and the release key
   fingerprint `SHA256:FlrCFc4LXIOOPMrT2g4dgV6CuOEiw1L30NWKzGPBZt8` (`deploy/release/allowed_signers`).
8. **Public launch.** Publish the invitation on pocol.net, signed with the release key like every download
   (`deploy/release`), and open the seeds. The launch evidence ([REHEARSAL.md](../../development/public-network/REHEARSAL.md))
   is measured on PoCol Mainnet after the launch.

## Tools

- `set_network_identity.py --from OLD.json --to NEW.json [--check]`: switches every client's pinned chain ID,
  genesis hash and GenesisSpec (`FILES` lists them) from one network's invitation to another's, with the
  derived content versions and the Studio gateway installer. It names no network itself: both identities come
  from the invitations, proven by their GenesisSpec. The invitation of the identity the tree pins today, which
  is no longer in the tree, is taken from git history:

  ```sh
  git show <commit>:deploy/mainnet/h15000/network.pocol.json > /tmp/OLD.json
  python3 deploy/mainnet/set_network_identity.py --from /tmp/OLD.json --to NEW_DIR/network.pocol.json
  python3 deploy/mainnet/set_network_identity.py --from /tmp/OLD.json --to NEW_DIR/network.pocol.json --check
  ```

  `<commit>` is any commit before that file was deleted (`git log --diff-filter=D -- deploy/mainnet/h15000`
  names the deleting commit; use its parent). The tool prints the identity it switches from and to before it
  writes anything. It also writes the release's pinned launch network, `NETWORK_PROFILES` in
  `prototypes/pocold/src/readiness.rs` (V3-28 H2): the `--to` entry replaces the `--from` one, unless `--to` is a
  retired network or the rehearsal network (the chain ID of `v3-rehearsal-profile.json`), which get no entry, and
  `--check` refuses a missing or extra entry, or an entry of the other chain's form. Each entry is written on the
  declaration's one line, entries separated by `,` (`HEX` the 64 lower-case hex digits of the genesis hash, decoded at
  compile time). On a public network chain (`PUBLIC_NETWORK_CHAINS` of the profile code, the mainnet profile's chain,
  which a test keeps equal to `v3-profile.json`) the entry is
  `NetworkProfile{chain_id:CHAIN,genesis_hash:genesis_hash("HEX"),admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")}`:
  the public network's admission (scope public-network, the network profile on a network node, with the conditions and
  the owner's R13, R21 and R26 waivers its boot reports; `pocold`'s `tests/fn4_profiles.rs` checks it against a boot)
  and the owner's launch approval (D1). On any other allowed network it is
  `NetworkProfile{chain_id:CHAIN,genesis_hash:genesis_hash("HEX"),admission:ExpectedAdmission::FRESH_NETWORK,approval:None}`
  (the experimental admission, which approves nothing and removes only `network-not-release-pinned`). The tool never
  writes `ExpectedAdmission::LAUNCH` (scope specification, no condition, no waiver). A booted node of the pinned
  network reports `publicLaunchApproved` true only when its booted admission matches the pin exactly (otherwise
  `admission-mismatch`, naming the fields that differ), and its `technicalReadiness` still lists `admission-scope`,
  each `admission-condition-*` and `admission-waiver-*`, and every open gate. The launcher boots nothing: it reports
  the pin's approval reference and keeps `admission-not-booted`. The tree pins a retired network today, so the list is
  empty and every network reports `network-not-release-pinned` and `launch-not-approved` (and, in the launcher,
  `admission-not-booted`). The report is node-local: it never stops a node from running, mining or serving sites.
- `v3-profile.json`, `v3-rehearsal-profile.json`: the fresh network's mainnet profile and the rehearsal profile,
  which is kept only as a test and devnet profile (FN-4; OWNER-DECISIONS-2026-11-20). They differ only in the chain ID
  and the launch second. `test_v3_profiles.py` checks them; `pocold`'s
  `tests/fn4_profiles.rs` builds and admits them.
- `retired-networks.json`: the identities the clients were switched away from, append-only. Every switch adds
  its `--from` network, and the tool refuses a listed network as `--to` (swapped `--from`/`--to` included):
  a rollback is a `git revert` of the switch commit, not a reversed run. `--allow-revert` switches back to a
  retired network on purpose. Its first entry is the network retired on 2026-11-13, which the clients still pin
  until the new genesis exists.

The tests (`test_set_network_identity.py`) also guard the tree: every tracked file that pins the current
identity is in `FILES` or kept on purpose; once the tree pins a network that is not retired, no tracked file
outside `development/` and `deploy/websites/` may hold a retired identity (`FILES` included); and the shipped
release texts that are true only on the retired network (`STEP3`: the 0.4.x versions and "chain ID and genesis
unchanged" notes) must be gone by then. The native 0.5.0 bump (owner decision 2026-11-16) landed before the switch;
what `STEP3` still lists (the extension and Studio versions and the 0.4.x extension download names) moves with the
signed 0.5.0 extension release and its download links. `deploy/release/assemble_release.py` refuses a release whose
extension `package.json` version is not the native version, so no release is assembled before the switch sets it to
0.5.0: the ceremony runs on unsigned CI artifacts (step 1), and the first signed release follows the switch (step 7).

## A spec-2 (v3) genesis

Since phase 4 of the fresh network the v3 rules are the release rules (consensus rules 2):
release nodes and clients decode GenesisSpec version 2 (CP schema v2) by default, and
version 1 still decodes. `set_network_identity.py` switches the clients to a spec-2
genesis because the tree's node decodes it (`SPEC_VERSION` 2 in `genesis_spec.rs`), and
only after the whole release decoder
(`deploy/public-testnet/pocol_identity.decode_genesis_spec` at `SPEC_VERSION` 2, checked
against the Rust verdicts of the decoder corpus: framing, structure, counts, canonical
integers, widths and bounds of every field of the final CP schema v2 of 59 fields, roots,
M_0List order and reserved ids) accepts it and its `sysCodeHash` names the v3 program
set. A refused `--to` writes nothing. Spec-1 switches are unchanged.

## The extension's built-in profile placeholder (decision D1)

Until the new genesis exists, the extension's built-in profile (`prototypes/pocol-extension/src/lib/studio-network.js`)
pins no retired network: it holds a placeholder, the spec-2 (v3) devnet GenesisSpec of the decoder corpus
(`tests/data/decoder-corpus-v3.json`, item `v3/base`) under a chain ID of no network, whose invitation is
`placeholder-network.pocol.json`. The file carries the `NETWORK_PLACEHOLDER` mark, its `FILES` counts are then counts
of the placeholder, and `prototypes/pocol-extension/scripts/package-release.py` refuses a build that carries the mark,
so no release ships it (CI checks the refusal instead of packaging).

```sh
python3 deploy/mainnet/set_network_identity.py --placeholder deploy/mainnet/placeholder-network.pocol.json [--check]
```

`--placeholder` writes a spec-2 placeholder (never a retired network or the tree's own) into `PLACEHOLDER_FILES` only.
The identity switch (`--from`/`--to`) replaces the placeholder there with the `--to` identity and removes the mark, so
the switch stays one run. The extension pages (`publisher.html`, `contract.html`) and READMEs keep the tree's identity
until that switch.

## Local checks

```sh
python3 -B -m unittest discover -s deploy/mainnet -v
```
