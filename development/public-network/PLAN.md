# Public PoCol network: launch gates and remaining work

The owner asked for a public network on 29 October 2026. On 2026-11-13 the owner decided to launch it as a brand-new
network from a fresh genesis, keeping nothing of the retired network
([owner decisions](../decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md)). The delivery target is **PoCol
Mainnet**, a public network that anyone can join, launched directly from its genesis ceremony. On 2026-11-20 the owner
decided to launch it publicly without a separate rehearsal, taking responsibility for the launch decision and
accepting the documented risks ([owner decisions 2026-11-20](../decisions/OWNER-DECISIONS-2026-11-20.md) D1). There is
no testnet, devnet or rehearsal stage before the launch.

## Current truthful status

The network runs the v3 release rule set: consensus rules 2 on a version-2 `GenesisSpec` (CP schema v2, nonceMode 1).
Its parameters and genesis contents are fixed in [FRESH-NETWORK-CP.md](../decisions/FRESH-NETWORK-CP.md): chain id
2656089850 for the mainnet and 1656550731 for the rehearsal profile, kept only as a test and devnet profile; four
founders (the three US servers and the owner's PC), each with 1,000,000 PCOL including its 20 PCOL bond; `M_max` 10,000; `T_blk` 10 s; no probation pool, no creation fee
and no seat contest. The genesis is continuous (`H_END` 0). The committed profiles are
`deploy/mainnet/v3-profile.json` and `v3-rehearsal-profile.json`, and the ceremony is described in
[deploy/mainnet/README.md](../../deploy/mainnet/README.md) and
[deploy/standalone/README.md](../../deploy/standalone/README.md).

The network launches publicly and permissionlessly **with the gates below still open**. The node reports launch
approval apart from technical readiness (V3-28 H1; OWNER-DECISIONS-2026-11-20 D4), every value computed from facts
(`prototypes/pocold/src/readiness.rs`):
- `publicLaunchApproved`, `launchApprovalReference`, `publicLaunchReady` and `launchBlockers`: the launch is approved
  only on a release that pins this network's (chainId, genesisHash) with the approval reference
  "OWNER-DECISIONS-2026-11-20 D1", on a build whose hard facts clear, and only by a node booted with exactly the pinned
  admission. Before the identity pin no release pins the genesis, so every node reports approval false with
  `network-not-release-pinned` and `launch-not-approved`.
- `technicalReadiness` `{complete, pending}`: every open gate below (`plan-gate-<id>`) and what the admission reports
  (`admission-scope` and each `admission-condition-*` and `admission-waiver-*`), never claimed passed. These items are
  reported and do not block the launch decision (D6 C).

None of this stops a node from running, mining or serving sites. PoCol Mainnet is admitted as the network profile on a
network node in its own scope, `public-network` (D5), on its chain only (`PUBLIC_NETWORK_CHAINS`, R22). The scope is
never specification compliance; it reports every condition and the owner's explicit R13, R21 and R26 waivers, and it
changes no consensus validity rule, allocRoot or genesis hash. The accepted risks of D1 and D2 (no rehearsal, 4 GB
founder servers, each waiver and pending item) are listed in the decision record, "Accepted risks (D1, D2)", and
recorded in [FINDINGS.md](FINDINGS.md) (F-10, F-92 and F-93, with the other entries the record cites). The
launch disclosures are drafted in the standalone README, "Launch disclosures" (owner decision pending:
launch-disclosure wording; owner decision 2026-11-18: they carry no "not public-launch-ready" item).

New miners register through an ordinary transaction. A newcomer may register with a 0 PCOL bond and earn the rest
from its rewards (ACCRUING). At a full set of `M_max` members a newcomer replaces an inactive member
(`registerReplacing`). This is a measured experiment, not a proof of Sybil resistance.

## Acceptance ledger

| Gate | Required deliverable/evidence | Present status |
| --- | --- | --- |
| K1: Independent identities | OS-random per-node keys, no secrets in genesis/bundles, permission failures and backup/restart tests | Unix keys, Windows DPAPI keys, service-local custody and the passphrase-encrypted key backup implemented; state recovery tested locally; the operator key recovery runbook is written ([RECOVERY.md](RECOVERY.md)) and its drill, planned for the rehearsal that OWNER-DECISIONS-2026-11-20 D1 removed, is not yet run; an opt-in encrypted-at-rest server key implemented: a systemd credential sealed to the host key or TPM2 (`pocol start --key-credential`, `deploy/standalone/encrypt-server-key.sh`), tested locally and against fakes, not yet run on the servers; the recovery drill, a server run with the encrypted credential, and an encrypted keystore or HSM for the other keys remain |
| J1: Open miner enrollment | A non-genesis identity funds a bond, registers through an ordinary transaction, joins mining; duplicate/invalid operations rejected | Zero-bond (ACCRUING) registration, top-up, rejoin and replacement at a full set implemented and tested on v3; first-fee funding is decided (owner decision 2026-11-18: the automated fee faucet runs on mainnet, 0.002 PCOL once per new address, 200 a day, 3 per client a day, from a dedicated key holding at most 10 PCOL; the limits are code-enforced ceilings); eligibility churn and load, and network-scale enrollment remain |
| I1: Stable network identity | Published genesis, versioned wire/constants identity, reproducible release and compatibility rejection | Wire v2 with the consensus identity and the consensus-source lock implemented and tested; chain ids drawn; the genesis is published at the ceremony and pinned in the clients afterwards; a native double build compares two builds of the Linux release binaries and package (other checkout paths, umasks, TZ and cargo homes) byte for byte (`deploy/release/double_build.py`, CI jobs `native-reproducible` and `native-reproducible-compare`; the Windows package and the APK are not covered): local runs on 2026-11-19 were byte-identical, the CI jobs have not yet run; the normative digest adoption and an independently reproduced binary release (someone other than the release builder rebuilding the signed release) remain |
| A1: Honest admission | Approved profile, measured H_cal, real worst-case execution calibration, R18/R21 obligations, RpcGuard gaps closed | Open; the profile is approved (FRESH-NETWORK-CP) and the ceremony records a measured hash rate (`pocol hashrate`, PR #108, on the four founder devices). PoCol Mainnet is admitted as the network profile on a network node in scope public-network (OWNER-DECISIONS-2026-11-20 D5), which is not specification compliance: it reports its seven admission conditions and the owner's explicit R13, R21 and R26 waivers; real worst-case execution calibration, the R18/R21 obligations and the RpcGuard gaps remain |
| S1: Bounded historical state | K_eff and historical RPC behavior within the resource budget; near-cap measurements and fault/recovery tests | Open; R21 fails at the mainnet caps (a retained-state bound of 59,702,459,358 B against HIST_TRIE_BUDGET 1,717,986,918 B). The public-network admission reports an explicit owner waiver of the budget term (OWNER-DECISIONS-2026-11-20 D5, D2), and the retained-blocks term stays enforced; S1 stays open until K_eff and the historical bound are measured; do not rely on average RSS |
| N1: Internet exposure | Seed/bootstrap operations, hostile-peer and RPC overload tests, ingress controls, time synchronization, public endpoint monitoring | Seeds, peer discovery, automatic outbound, inbound eviction, bounded handshakes, gateway configuration and monitoring implemented; actual Internet ingress acceptance and the remaining RpcGuard controls open |
| L1: Sustained operation | Multi-host soak, partition/heal, restart/replay, backup/restore, release/rollback drill and bounded resource growth | Local packaged processes passed restore and progress/root comparison; the evidence plan is [REHEARSAL.md](REHEARSAL.md). There is no rehearsal (OWNER-DECISIONS-2026-11-20 D1): the launch evidence (T1-T8, M1-M4, R-off, D1, S1, S2 and the L1 drills) has no pre-launch measurement and is measured on PoCol Mainnet after the launch, where a remedy that changes a consensus parameter is a fork with notice; that evidence, an actual multi-host soak and an upgrade drill remain |
| E1: Network lifetime/economics | Explicit approved choices for initial allocation, issuance/bonds, H_CLOSE/H_END and cumulative caps | Open; allocation, issuance (M3, S0 4 PCOL), the bond, H_END 0 and the caps with their raise path are approved in FRESH-NETWORK-CP; the founders' reward addresses are decided (three owner-held offline addresses, made 2026-11-18, given to the ceremony as `-ServerRewardAddresses`) and enter the genesis at the ceremony; the ceremony and the launch evidence, measured on PoCol Mainnet after the launch (no rehearsal, OWNER-DECISIONS-2026-11-20 D1), remain |
| Q1: Release review | Exact-commit CI, independent security review, unresolved-finding ledger and public limitations | Open; AI-authored tests do not substitute for external review; the owner commissions the independent review after the launch (owner decision 2026-11-18); the unresolved-finding ledger and public limitations are in [FINDINGS.md](FINDINGS.md) |

A gate closes only with its implementation and evidence, never by changing a status label. No dates or completion
percentages are asserted without that evidence. `prototypes/pocold/src/readiness.rs` (`PLAN_GATES_OPEN`) lists the
open gates in this order, with what remains of each, and reports them in `technicalReadiness.pending`; its test keeps
the gate ids equal. Its texts follow the rows above. `readiness.rs` is node-local reporting and not in the
consensus-source lock (`prototypes/pocold/consensus-boundary.txt`, `=pocold::readiness`), so a text change needs no
re-pin.

## Launch order

There is no rehearsal step (OWNER-DECISIONS-2026-11-20 D1; the rehearsal profile, chain 1656550731, is kept only as a
test and devnet profile).

1. **Release.** Build the release with `POCOL_RELEASE=1` from a pinned tree (the consensus-source lock was reset to a
   single `base` line at the release cut, Phase 8, and records the public-network admission as one `neutral` line)
   and run the full CI, with the mandatory pre-launch tests of D6 E. A test that cannot run is reported as not run.
2. **Mainnet ceremony.** On freshly provisioned devices (the three founder servers, also the seeds, and the owner's
   PC; D3), generate fresh keys and run the ceremony with the mainnet profile. Each node boots in scope
   public-network and reports approval false (`network-not-release-pinned`, `launch-not-approved`) until the pin.
   Publish the invitation and its fingerprint through a trusted channel. Once PoCol is installed on the three founder
   servers, set `POCOL_DATA_REQUIRED=1` in their disk monitor (with the owner's approval) and check that its `--status` reports the PoCol data
   directory present (OWNER-DECISIONS-2026-11-21 D7, the owner's requirement; step 6 of the launch runbook).
3. **Pin the identity** of the new genesis in the clients in one reviewed change: `set_network_identity.py` writes the
   release pin with the approval reference "OWNER-DECISIONS-2026-11-20 D1".
4. **Signed release.** Release the clients from the switched tree: the first signed release (release signing is
   owner-held, D6 F). A node of that release booted on PoCol Mainnet reports `publicLaunchApproved` and
   `publicLaunchReady` true, with its `technicalReadiness` still listing what is open.
5. **Public launch** with the disclosures, once the founder servers' disk monitor is active on all three servers
   and a real test notification from each has been received (OWNER-DECISIONS-2026-11-21 D7: the servers keep their
   35 GB disks, [deploy/monitoring](../../deploy/monitoring/README.md), "Acceptance test"). The owner installed the
   monitor on the three servers on 2026-11-21, reports its acceptance test passed on each and approves that D7's
   condition is met (D7, "Acceptance of D7's monitoring"). The launch evidence of [REHEARSAL.md](REHEARSAL.md) is measured on PoCol
   Mainnet after the launch; a remedy that changes a consensus parameter is then a fork (version 4 or later, announced
   at least 24 h ahead), with the owner's approval and recorded in FRESH-NETWORK-CP. The gates above stay open until
   their evidence exists; an independent security review (Q1) is commissioned after the launch.

Operators generate private keys on their own devices and provide only public addresses. Never put secret keys in
chat, Git or uploaded test artifacts.

## Source anchors

- [Owner decisions 2026-11-20: public launch, accepted risks](../decisions/OWNER-DECISIONS-2026-11-20.md)
- [Owner decisions 2026-11-21: the founder servers' 35 GB disks and their monitoring](../decisions/OWNER-DECISIONS-2026-11-21.md)
- [Final parameters and genesis contents](../decisions/FRESH-NETWORK-CP.md)
- [Production boot blockers](../audit-2026-10-22/BOOT-BLOCKERS-2026-10-28.md)
- [Admission implementation](../../prototypes/lp4-execution/src/admission.rs)
- [Registry and system contracts](../../prototypes/lp4-execution/contracts/system/PoColSystemV3.sol)
- [Genesis builder](../../prototypes/pocold/src/testnet.rs)
- [Launch readiness](../../prototypes/pocold/src/readiness.rs)
- [Launch runbook](../../deploy/mainnet/README.md)
- [Independent key operations](KEYS.md)
- [Key recovery runbook](RECOVERY.md)
- [The launch-evidence plan, measured after the launch](REHEARSAL.md)
- [Service, gateway, monitoring and recovery runbook](../../deploy/public-testnet/README.md)
- [Founder servers' disk monitor](../../deploy/monitoring/README.md)
- [Compatibility and operations](OPERATIONS.md)
- [Unresolved findings and public limitations](FINDINGS.md)
- [Key and enrollment results, 29 October 2026](RESULTS.md)
