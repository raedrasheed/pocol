# Public PoCol network: launch gates and implementation work

The owner requested a deployable real network and explicitly selected **a public
network** on 29 October 2026. The private three-server package is preparation
infrastructure, not completion of that request. The delivery target is an openly
joinable public testnet with test currency first. An economic mainnet is a separate
acceptance decision; no real funds, infrastructure purchases or launch are implied.

## Current truthful status

Public launch is **blocked**. Independent key custody and transaction-driven miner
registration passed exact-head CI. A second increment adds strict wire compatibility,
independent-key service bundles, offline recovery and cross-node monitoring. The candidate
genesis still goes through `ExperimentalDevnet` admission, with all existing
conditions visible. It does not admit chainId 777001 or silently reinterpret a
simnet as the production profile.

The existing registry contracts already implement registration, reactivation,
deregistration and bond claims. The old devnet builder's R_max=0 disabled new
registration; the contract functionality was not absent. The new public-input
candidate can explicitly set registrationPerBlock to 0..4 and must still pass the
existing admission checks. This is a bounded experiment, not an economic policy
approval or proof of Sybil resistance.

## Acceptance ledger

| Gate | Required deliverable/evidence | Present status |
| --- | --- | --- |
| K1: Independent identities | OS-random per-node keys, no secrets in genesis/bundles, permission failures and backup/restart tests | Unix keys and service-local custody implemented; state recovery tested locally; operator secret-key recovery and encrypted keystore/HSM remain |
| J1: Open miner enrollment | A non-genesis identity funds a bond, registers through an ordinary transaction, joins mining; duplicate/invalid operations rejected | Local four-node integration implemented; public faucet, eligibility churn/load and network-scale enrollment remain |
| I1: Stable network identity | Published genesis, versioned wire/constants identity, reproducible release and compatibility rejection | Experimental wire v2 and conservative source fingerprint implemented and tested; D-5 normative digest/adoption and independently reproduced binary release remain open |
| A1: Honest admission | Approved profile, measured H_cal, real worst-case execution calibration, R18/R21 obligations, RpcGuard gaps closed | Open; synthetic calibration and seven existing conditions remain |
| S1: Bounded historical state | K_eff and historical RPC behavior within the resource budget; near-cap measurements and fault/recovery tests | Open; do not bypass R21 or rely on average RSS |
| N1: Internet exposure | Seed/bootstrap operations, hostile-peer and RPC overload tests, ingress controls, time synchronization, public endpoint monitoring | Seed/join packaging, bounded handshakes, gateway configuration and monitoring implemented; actual Internet ingress acceptance and remaining RpcGuard controls open |
| L1: Sustained operation | Multi-host soak, partition/heal, restart/replay, backup/restore, release/rollback drill and bounded resource growth | Four packaged processes passed local restore and 180-second progress/root comparison; physical servers, actual multi-host soak and upgrade drill remain open |
| E1: Network lifetime/economics | Explicit approved choices for initial allocation, issuance/bonds, H_CLOSE/H_END and cumulative caps | Open; experimental defaults close at 19,000/20,000 blocks (~53/56 nominal hours) |
| Q1: Release review | Exact-commit CI, independent security review, unresolved-finding ledger and public limitations | Open; AI-authored tests do not substitute for external review |

The implementation order is K1/J1, then identity/admission/storage and hostile-network
controls, then the actual multi-host public-testnet acceptance. Gates must be closed
with implementation and evidence, not by changing a status label. No dates or
completion percentages are asserted without that evidence.

## Decisions and inputs still needed for launch

The [existing decision table](../audit-2026-10-22/proposals/DECISION-TABLE-2026-10-28.md)
has unresolved normative alternatives, including conflicting recommendations for
D-2. The new public-network request establishes the goal; it does not resolve
those contradictions or supply genesis allocations, target hardware measurements,
hosting access or public endpoints. Historical approvals and proposals remain intact.

Engineering recommendation: first publish a bounded public **testnet** with valueless
test currency after its gates pass; keep the economic mainnet unlaunched. A perpetual
network requires an explicit design for the finite H_END and cumulative caps. Do not
delete those rules or inflate their constants merely to keep a process running.

The public testnet will need operator-approved server inventory and SSH access,
network/monitoring endpoints, an initial public-address allocation list and a
scheduled shared genesis. Operators generate private keys locally and provide only
their public addresses. Never put secret keys in chat, Git or uploaded test artifacts.

## Source anchors

- [Production boot blockers](../audit-2026-10-22/BOOT-BLOCKERS-2026-10-28.md)
- [Current admission implementation](../../prototypes/pocold/src/lib.rs)
- [Registry contract](../../prototypes/lp4-execution/contracts/system/PoColSystemV1.sol)
- [Genesis and resource defaults](../../prototypes/lp4-execution/src/devnet.rs)
- [Independent key operations](KEYS.md)
- [Service, gateway, monitoring and recovery runbook](../../deploy/public-testnet/README.md)
- [Compatibility and operations evidence](OPERATIONS.md)

## Five-device standalone increment (29 October 2026)

The owner requested ordinary independent operation on three prepared Linux VPS,
one Windows PC and one Android phone. `deploy/standalone/` adds a common native
launcher, fixed local identities, a bounded shared-genesis invitation and native
release builds. The Android app hosts the Rust node itself, not a remote RPC-only
wallet. Windows uses user-bound DPAPI key protection. These additions do not
change any consensus/admission/lifetime/resource launch gate above.

One-time genesis publication still requires public addresses generated on the
devices. New identities can join P2P without an operator allowlist; mining still
requires the existing bonded on-chain enrollment. No automatic funding authority
or approval service is embedded. Public network deployment, cross-platform
physical-device acceptance, mobile capacity/lifecycle testing and a stable private
APK signing identity remain outstanding. The standalone README records the
precise scope and commands.


## Wallet and continuous-genesis increment (2026-10-29)

The owner requested native Android/Windows wallet interfaces (#1aa9e4), actual
local mining controls and no scheduled network expiry. The new bootstrap creates
a separate continuous genesis: all four lifetime fields zero, separate compiled
registry programs, per-block transaction/log allowances, and explicit growing
local resource projections. Existing finite networks are not migrated or erased.
Both wallets sign locally after a one-use expiring review; bonded registration
is explicit and mining is off by default. See deploy/standalone/README.md.

This removes the earlier 19000/20000 shutdown from **new continuous profiles**;
it does not close the existing memory, RpcGuard, calibration, external review,
physical Android or real multi-host qualification gates. Long-term metadata,
replay and disk growth require capacity work; infinite fixed-resource operation
is not established. No live servers were changed by this increment.
