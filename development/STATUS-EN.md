# PoCol development status — 16 November 2026

## The network

On 2026-11-13 the owner decided to launch **PoCol Mainnet** (currency PCOL) as a brand-new public network from a
fresh genesis, with new keys and a new chain id, keeping nothing of the retired network
([decisions](decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)). The network launches directly as a public
mainnet, after a closed rehearsal on the same devices.

The final parameters are in [FRESH-NETWORK-CP.md](decisions/FRESH-NETWORK-CP.md):

- the v3 rule set: consensus rules 2 (`CONSENSUS_RULES_VERSION` 2) on a version-2 `GenesisSpec` (CP schema v2,
  nonceMode 1), continuous (`H_END` 0);
- chain id 2656089850 for the mainnet and 1656550731 for the rehearsal;
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

## Next

1. **Release cut.** Reset the consensus-source lock once, to a single `base` line, in the
   last code change before the release; build with `POCOL_RELEASE=1` and run the full CI.
2. **Rehearsal.** Run the ceremony with the rehearsal profile on the three servers and the PC and measure the launch
   evidence (T1-T8, M1-M4).
3. **Mainnet ceremony.** Fresh keys again, the mainnet profile, one genesis and one invitation.
4. **Identity pin.** Switch the clients from the retired identity they still pin to the new genesis, in one reviewed
   change, and release them.
5. **Public launch** with the launch disclosures.

The network launches with `publicLaunchReady: false` and its launch blockers disclosed; the open gates are in
[public-network/PLAN.md](public-network/PLAN.md). No independent external security review has been done.

## Open owner items

Each of these is undecided; texts that need them say "owner decision pending".

- the native release version and the signing scheme;
- translations;
- the founders' reward address: each server's hot miner key, or a separate owner-held address;
- the launch-disclosure wording (draft in `deploy/standalone/README.md`);
- whether a fee faucet runs on mainnet;
- the explorer gateway host;
- the canonical public repository URL.

## Earlier work

M1, the specification stage, was accepted at Draft 0.32 for all 41 rows of its gate, and the coordinator improvement
was verified at Draft 0.33. Those were specification and evidence results, not node, browser or production results.
The full M1 cycle log (Drafts 0.1 to 0.33, 7 October 2026 onwards) is kept in git history:
`git show 0fccc3d:development/STATUS-EN.md`. Later stages are listed in the root README and reported in
[reports/](reports/).
