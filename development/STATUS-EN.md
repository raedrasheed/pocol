# PoCol development status — 20 November 2026

## The network

On 2026-11-13 the owner decided to launch **PoCol Mainnet** (currency PCOL) as a brand-new public network from a
fresh genesis, with new keys and a new chain id, keeping nothing of the retired network
([decisions](decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)). On 2026-11-20 the owner decided that it
launches directly as a public mainnet, without a separate rehearsal, and accepted the documented risks
([decisions](decisions/OWNER-DECISIONS-2026-11-20.md) D1).

The final parameters are in [FRESH-NETWORK-CP.md](decisions/FRESH-NETWORK-CP.md):

- the v3 rule set: consensus rules 2 (`CONSENSUS_RULES_VERSION` 2) on a version-2 `GenesisSpec` (CP schema v2,
  nonceMode 1), continuous (`H_END` 0);
- chain id 2656089850 for the mainnet; 1656550731 is the rehearsal profile's, kept only as a test and devnet profile;
- four founders (the three US servers and the owner's PC), each with 1,000,000 PCOL including its 20 PCOL bond;
- `M_max` 10,000, `T_blk` 10 s, RPC `safe` depth 6 and reorg alarm depth 60;
- no probation pool, no creation fee and no seat contest; a newcomer may start with a 0 PCOL bond and earn it;
- `MAX_FORK_VERSION` 3: fork version 2 is a no-op, fork version 3 is the cap raise (FN-5), dormant until a schedule
  names it.

The Android and Windows wallets are **wallet only** by default; **Settings → Device role → Wallet and miner** adds
mining. The browser extension moves to 0.5.0 with a fixed key.

## Done

- The retired network's code, tooling and packages are removed, and every network this release builds or admits is a
  v3 genesis; admission refuses spec 1.
- The v3 rule set is the release: earn-in without a probation pool or creation fee (FN-2a, FN-2b), membership storage
  for 10,000 members (FN-3), the committed mainnet and rehearsal profiles (FN-4), the cap raise (FN-5) and the
  genesis ceremony script (FN-7).
- Devnets, tests, websites, the extension, the reward gateway and the wallets run on v3.
- The specification and decision records describe the v3 network only; dated records keep "superseded" notes.
- The consensus-source lock was reset once for the fresh genesis (Phase 8): one `kind=base` line recording rules 2,
  maxFork 3 and spec 2, with the same consensus sources; the retired network's history lines are gone.

## Next

1. **Release cut.** The consensus-source lock has been reset once, to a single `base` line (Phase 8; the one-time
   exemption of the append-only check is spent). Build with `POCOL_RELEASE=1` and run the full CI.
2. **Mainnet ceremony.** No rehearsal (D1): fresh keys on the three founder servers (also the seeds) and the owner's
   PC (D3), the mainnet profile, one genesis and one invitation.
3. **Identity pin.** Switch the clients from the retired identity they still pin to the new genesis, in one reviewed
   change; it pins the genesis with the approval reference "OWNER-DECISIONS-2026-11-20 D1".
4. **Signed release** of the switched clients (the first signed release).
5. **Public launch** with the launch disclosures. The launch evidence (T1-T8, M1-M4) is measured on the network after
   the launch.

The node computes launch approval apart from technical readiness (D4): `publicLaunchApproved` and
`publicLaunchReady` become true only on a release that pins the genesis with its approval reference and only on a
node booted with exactly the pinned admission, while `technicalReadiness` keeps listing the open gates of
[public-network/PLAN.md](public-network/PLAN.md) and the admission's conditions and waivers. PoCol Mainnet is
admitted as the network profile on a network node in scope public-network (D5), never specification compliance, with
the owner's R13, R21 and R26 waivers reported. No independent external security review has been done.

## Open owner items

Each of these is undecided; texts that need them say "owner decision pending".

- the launch-disclosure wording (draft in `deploy/standalone/README.md`);
- the reward page S1 wording.

Decided by the owner on 2026-11-20
([decisions/OWNER-DECISIONS-2026-11-20.md](decisions/OWNER-DECISIONS-2026-11-20.md)):

- the public launch of PoCol Mainnet without a separate rehearsal, with the owner's responsibility for the launch
  decision and acceptance of the documented risks (D1; it supersedes the closed rehearsal of 2026-11-13);
- the three founder servers have 4 GB of RAM, risk accepted, 8 GB recommended (D2);
- the founders and seeds: `108.165.128.89`, `108.165.128.90`, `108.165.128.91` and the owner's Windows PC, from a
  fresh genesis (D3);
- launch approval separate from technical readiness (D4);
- the public network's own admission scope, public-network, with explicit owner waivers (D5);
- a full public launch, never a renamed test network, every value computed from facts (D6);
- the rehearsal profile is kept only as a test and devnet profile.

Decided by the owner on 2026-11-18:

- the extension manifest public key (`prototypes/pocol-extension/release/manifest-key.pub`), made offline by the
  owner: extension ID `gggcoflmedlhlkcdhebmoepmggdnafmp`;
- the release signing public key (`deploy/release/allowed_signers`), made offline by the owner: fingerprint
  `SHA256:FlrCFc4LXIOOPMrT2g4dgV6CuOEiw1L30NWKzGPBZt8` (ED25519);
- the three owner-held server reward addresses, made offline with `pocol-key` and given to the ceremony as
  `-ServerRewardAddresses` (not committed: the ceremony takes them as a parameter), and held offline as an
  encrypted backup and paper copies;
- no text may state anything untrue or present the public launch as prevented; the launch disclosures carry no
  "not public-launch-ready" item and no list of ongoing engineering work;
- newcomers' first registration fee: the automated fee faucet runs on mainnet (`deploy/faucet`), with the owner's
  limits as code-enforced ceilings: 0.002 PCOL per new address once, 200 transfers a day, 3 per client a day, from a
  dedicated operator key holding at most 10 PCOL (this supersedes the 2026-11-16 "no faucet on mainnet");
- the independent security review (Q1) is commissioned after the launch.

The decisions and the owner's words are in
[decisions/OWNER-DECISIONS-2026-11-18.md](decisions/OWNER-DECISIONS-2026-11-18.md). Its decision 4 ("stays as
computed", "refuses nothing") is refined by the decision of 2026-11-20 D4; its truthfulness rule stays.

Decided by the owner on 2026-11-16:

- native release version 0.5.0 (pocold, Windows, Android; the extension's `package.json` moves with its signed 0.5.0
  release);
- release signing: every download and `SHA256SUMS` signed with ssh-keygen ed25519, namespace `pocol-release`
  (`deploy/release`);
- site languages at launch: English and Arabic;
- the server founders' rewards go to owner-held offline addresses (`-ServerRewardAddresses`); only the 20 PCOL bond
  stays on each server;
- the canonical repository: `raedrasheed/pocol`;
- a fork activation is announced at least 24 h (8,640 blocks at T_blk 10) ahead;
- the build-info scope label is `mainnet`;
- no faucet on mainnet (the rehearsal network only; superseded on 2026-11-18: the fee faucet runs on mainnet);
- the explorer gateway host is `explorer.pocol.net`, replacing the old PHP site.

## Earlier work

M1, the specification stage, was accepted at Draft 0.32 for all 41 rows of its gate, and the coordinator improvement
was verified at Draft 0.33. Those were specification and evidence results, not node, browser or production results.
The full M1 cycle log (Drafts 0.1 to 0.33, 7 October 2026 onwards) is kept in git history:
`git show 0fccc3d:development/STATUS-EN.md`. Later stages are listed in the root README and reported in
[reports/](reports/).
