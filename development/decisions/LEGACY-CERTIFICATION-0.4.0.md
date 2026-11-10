# Legacy certification: PoCol Mainnet 0.4.0 (H15000)

**Status: NOT CERTIFIED.** This is a template. Nothing below has been run or recorded.

A release after 0.4.0 runs a 0.4.0 (`pocol-network-v1`) launcher home only through a
certified entry of `launcher::LEGACY_BUILDS` (`prototypes/pocold/src/launcher.rs`) and its
mirror in `deploy/public-testnet/pocol_identity.py` (VA-SPEC §2; PR5 plan §1.7). The one
entry ships uncertified:

| field | value |
|---|---|
| digest (0.4.0 `compatibilitySha256`) | `6cf38a14aa01ca3d824b0309564942cc926ca7232e2a5b2672a17e8c99e35476` |
| rules | 1 |
| chain_id | `1599846715` |
| genesis_hash | `0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca` |
| sources | none |
| certified | none |

With the entry uncertified, every H15000 home is refused and nothing is written (fail
closed). Owner decision D1 is pending, and the code implements its recommended default,
**Option B**: the network moves to the earn-in re-genesis, and the offline replay that builds
the new genesis serves as the evidence of the old history. Under Option B this document stays
a template, and no legacy entry needs certifying (D4, at its default: no other legacy build is
certified either). It is needed only if the owner chooses **Option A** (R1 on the live H15000
chain).

## What certification shows

That a given release (R1) accepts the stored H15000 history exactly as 0.4.0 did: the same
block hash, `stateRoot` and `receiptsRoot` at every height. After the switch no 0.4.0 node can
peer, so equivalence with 0.4.0 on adversarial input does not matter for peering safety.
Certification covers one consensus-sources aggregate: any later re-pin of
`consensus-sources.lock` voids it until the replay is rerun and `sources` updated
(`package.py` refuses to package a build whose `consensusSourcesSha256` differs from a
certified entry's `sources`).

## Evidence to record (Option A only)

1. **Release under test.** Commit, `pocol build-info` of the Linux build (`consensusPinned`
   must be true), and its `consensusSourcesSha256`:
   - commit: `…`
   - `compatibilitySha256`: `…`
   - `consensusSourcesSha256` (becomes the entry's `sources`): `…`
2. **Consensus identity.** `consensusIdentity` for chain 1599846715, the H15000 genesis,
   rules 1, flags 0 and `[(1,0),(2,15000)]`. The pinned value is
   `bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110`
   (`identity::tests::the_h15000_identity_is_pinned`, `test_pocol_identity.py`); record what
   R1 reports on the replayed home: `…`
3. **Replay source.** A stopped copy of one seed's state (which seed, at which head, its
   journal SHA-256): `…`
4. **Per-block comparison.** R1 replays the copy from genesis; for every height 0..head
   record whether the canonical hash, `stateRoot` and `receiptsRoot` equal the source's.
   Attach the comparison file (heights, the three values from each side, and the verdict), and
   state explicitly the results at the head and at H15000−1, H15000 and H15000+1
   (`deploy/mainnet/preflight.py` checks the head):
   - comparison file and its SHA-256: `…`
   - mismatches: `…` (must be none)
5. **Rollback test.** R1 run on a copy of a home, then the pinned
   `PoCol-Mainnet-0.4.0-Linux.zip` (SHA-256 in `deploy/mainnet/h15000/README.md`) started on the
   same home, reaching the same head: `…`
6. **Sign-off.** Owner, date, and the commit that sets `sources` and `certified` (this file's
   path) in `LEGACY_BUILDS` and in the `pocol_identity.py` mirror: `…`

Every release that re-pins the lock repeats steps 1, 2 and 4 and updates `sources`, or the
entry must go back to uncertified.
