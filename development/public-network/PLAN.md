# Public PoCol network: launch gates and remaining work

The owner asked for a public network on 29 October 2026. On 2026-11-13 the owner decided to launch it as a brand-new
network from a fresh genesis, keeping nothing of the retired network
([owner decisions](../decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)). The delivery target is **PoCol
Mainnet**, a public network that anyone can join, launched directly from its genesis ceremony after a closed
rehearsal on the same devices. There is no separate test-currency testnet stage.

## Current truthful status

The network runs the v3 release rule set: consensus rules 2 on a version-2 `GenesisSpec` (CP schema v2, nonceMode 1).
Its parameters and genesis contents are fixed in [FRESH-NETWORK-CP.md](../decisions/FRESH-NETWORK-CP.md): chain id
2656089850 for the mainnet and 1656550731 for the rehearsal; four founders (the three US servers and the owner's PC),
each with 1,000,000 PCOL including its 20 PCOL bond; `M_max` 10,000; `T_blk` 10 s; no probation pool, no creation fee
and no seat contest. The genesis is continuous (`H_END` 0). The committed profiles are
`deploy/mainnet/v3-profile.json` and `v3-rehearsal-profile.json`, and the ceremony is described in
[deploy/mainnet/README.md](../../deploy/mainnet/README.md) and
[deploy/standalone/README.md](../../deploy/standalone/README.md).

The network launches publicly and permissionlessly **with the gates below still open**. The
software reports `publicLaunchReady: false` with its `launchBlockers` (V3-28 H1). That never stops a node from
running, mining or serving sites; it is a statement of what is not yet established. Two blockers cannot clear on
this network as designed: R22 binds the `Network` profile to chainId 777001, so the mainnet is admitted as a Simnet
profile in `ExperimentalDevnet` scope (`profile-kind`, `admission-scope`). The launch disclosures are drafted in the
standalone README, "Launch disclosures" (owner decision pending: launch-disclosure wording; owner decision
2026-11-18: they carry no "not public-launch-ready" item).

New miners register through an ordinary transaction. A newcomer may register with a 0 PCOL bond and earn the rest
from its rewards (ACCRUING). At a full set of `M_max` members a newcomer replaces an inactive member
(`registerReplacing`). This is a measured experiment, not a proof of Sybil resistance.

## Acceptance ledger

| Gate | Required deliverable/evidence | Present status |
| --- | --- | --- |
| K1: Independent identities | OS-random per-node keys, no secrets in genesis/bundles, permission failures and backup/restart tests | Unix keys, Windows DPAPI keys, service-local custody and the passphrase-encrypted key backup implemented; state recovery tested locally; operator secret-key recovery procedures and an encrypted keystore or HSM remain |
| J1: Open miner enrollment | A non-genesis identity funds a bond, registers through an ordinary transaction, joins mining; duplicate/invalid operations rejected | Zero-bond (ACCRUING) registration, top-up, rejoin and replacement at a full set implemented and tested on v3; first-fee funding is decided (owner decision 2026-11-18: the automated fee faucet runs on mainnet, 0.002 PCOL once per new address, 200 a day, 3 per client a day, from a dedicated key holding at most 10 PCOL; the limits are code-enforced ceilings); eligibility churn and load, and network-scale enrollment remain |
| I1: Stable network identity | Published genesis, versioned wire/constants identity, reproducible release and compatibility rejection | Wire v2 with the consensus identity and the consensus-source lock implemented and tested; chain ids drawn; the genesis is published at the ceremony and pinned in the clients afterwards; a native double build compares two builds of the Linux release binaries and package (other checkout paths, umasks, TZ and cargo homes) byte for byte (`deploy/release/double_build.py`, CI jobs `native-reproducible` and `native-reproducible-compare`; the Windows package and the APK are not covered): local runs on 2026-11-19 were byte-identical, the CI jobs have not yet run; the normative digest adoption and an independently reproduced binary release (someone other than the release builder rebuilding the signed release) remain |
| A1: Honest admission | Approved profile, measured H_cal, real worst-case execution calibration, R18/R21 obligations, RpcGuard gaps closed | Open; the profile is approved (FRESH-NETWORK-CP) and the ceremony will record a measured hash rate, but the network is admitted in ExperimentalDevnet scope with its admission conditions; real worst-case execution calibration, the R18/R21 obligations and the RpcGuard gaps remain |
| S1: Bounded historical state | K_eff and historical RPC behavior within the resource budget; near-cap measurements and fault/recovery tests | Open; R21 holds only through the simnet `retentionStress` exception; do not bypass R21 or rely on average RSS |
| N1: Internet exposure | Seed/bootstrap operations, hostile-peer and RPC overload tests, ingress controls, time synchronization, public endpoint monitoring | Seeds, peer discovery, automatic outbound, inbound eviction, bounded handshakes, gateway configuration and monitoring implemented; actual Internet ingress acceptance and the remaining RpcGuard controls open |
| L1: Sustained operation | Multi-host soak, partition/heal, restart/replay, backup/restore, release/rollback drill and bounded resource growth | Local packaged processes passed restore and progress/root comparison; the rehearsal on the three servers and the PC (T1-T8, M1-M4), an actual multi-host soak and an upgrade drill remain |
| E1: Network lifetime/economics | Explicit approved choices for initial allocation, issuance/bonds, H_CLOSE/H_END and cumulative caps | Open; allocation, issuance (M3, S0 4 PCOL), the bond, H_END 0 and the caps with their raise path are approved in FRESH-NETWORK-CP; the founders' reward addresses are decided (three owner-held offline addresses, made 2026-11-18, given to the ceremony as `-ServerRewardAddresses`) and enter the genesis at the ceremony; the ceremony and the rehearsal evidence remain |
| Q1: Release review | Exact-commit CI, independent security review, unresolved-finding ledger and public limitations | Open; AI-authored tests do not substitute for external review; the owner commissions the independent review after the launch (owner decision 2026-11-18) |

A gate closes only with its implementation and evidence, never by changing a status label. No dates or completion
percentages are asserted without that evidence. `prototypes/pocold/src/readiness.rs` (`PLAN_GATES_OPEN`) lists the
open gates in this order; its test keeps the two equal.

## Launch order

1. **Release.** Build the release with `POCOL_RELEASE=1` from a pinned tree (the consensus-source lock was reset to a
   single `base` line at the release cut, Phase 8) and run the full CI.
2. **Rehearsal.** Run the ceremony with the rehearsal profile on the three servers and the PC, with fresh keys, and
   measure the launch evidence (T1-T8, M1-M4, FRESH-NETWORK-CP §9). A failed threshold changes only profile values,
   recorded in FRESH-NETWORK-CP, and both profiles change together.
3. **Mainnet ceremony.** Re-provision the devices, generate fresh keys again and run the ceremony with the mainnet
   profile. Publish the invitation and its fingerprint through a trusted channel.
4. **Pin the identity** of the new genesis in the clients in one reviewed change, and release them.
5. **Public launch** with the disclosures. The gates above stay open until their evidence exists; an independent
   security review (Q1) is still outstanding.

Operators generate private keys on their own devices and provide only public addresses. Never put secret keys in
chat, Git or uploaded test artifacts.

## Source anchors

- [Final parameters and genesis contents](../decisions/FRESH-NETWORK-CP.md)
- [Production boot blockers](../audit-2026-10-22/BOOT-BLOCKERS-2026-10-28.md)
- [Admission implementation](../../prototypes/lp4-execution/src/admission.rs)
- [Registry and system contracts](../../prototypes/lp4-execution/contracts/system/PoColSystemV3.sol)
- [Genesis builder](../../prototypes/pocold/src/testnet.rs)
- [Launch readiness](../../prototypes/pocold/src/readiness.rs)
- [Launch runbook](../../deploy/mainnet/README.md)
- [Independent key operations](KEYS.md)
- [Service, gateway, monitoring and recovery runbook](../../deploy/public-testnet/README.md)
- [Compatibility and operations](OPERATIONS.md)
- [Key and enrollment results, 29 October 2026](RESULTS.md)
