# Owner decisions (2026-11-20): the public launch of PoCol Mainnet

Status: **decided by the owner on 2026-11-20.** Approving a decision is not approval to deploy: every launch step on
a server still needs its own approval.

## The owner's words (verbatim)

On the public launch:

> اعتماد إطلاق شبكة PoCol كشبكة عامة (Public Mainnet)، مع تحملي مسؤولية قرار الإطلاق وقبول المخاطر الموثقة المرتبطة به.

> قرار تجاوز Rehearsal واعتماد الإطلاق العام نهائي، والمطلوب تنفيذه هندسيًا بصورة متكاملة دون تزوير نتائج الجاهزية أو إضعاف قواعد الإجماع.

In the owner's further mandatory instructions of the same day (decision 6):

> إطلاق شبكة PoCol إطلاقًا عامًا كاملًا (Public Mainnet Launch)، بعد اجتياز الاختبارات الفنية والأمنية المطلوبة.

> لا أريد إطلاق Testnet أو Devnet أو Simnet أو Rehearsal، ولا أريد أن تكون الشبكة الرئيسية مجرد شبكة تجريبية أعيدت تسميتها.

> يجب أن تكون القيم ناتجة عن حالة الشبكة الفعلية وسياسة الإطلاق المعتمدة، وليست قيمًا ثابتة أو مزورة.

Decisions 2 to 5 were given in the same directive; this record states them as decided, without a verbatim quote of
their own.

## Decisions

1. **D1: public launch without a separate rehearsal.** PoCol Mainnet (`deploy/mainnet/v3-profile.json`, chain id
   2656089850) launches as a public network directly from its genesis ceremony. The owner takes responsibility for the
   launch decision and accepts the documented risks ("قبول المخاطر الموثقة المرتبطة به"; the section "Accepted risks
   (D1, D2)" below lists them). The decision to skip the rehearsal is final, and it is carried out without falsifying
   readiness results or weakening the consensus rules. The independent security review stays after the launch
   (decision 7 of [2026-11-18](OWNER-DECISIONS-2026-11-18.md)). D1 is the launch approval reference that the release
   pins for the mainnet's genesis (`launchApprovalReference` "OWNER-DECISIONS-2026-11-20 D1",
   `prototypes/pocold/src/readiness.rs` `PUBLIC_MAINNET_APPROVAL`, written by `deploy/mainnet/set_network_identity.py`
   at the identity pin). It supersedes:
   - the closed rehearsal before the public launch ([2026-11-13](OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md),
     summary item 6) and the rehearsal step of the launch order
     ([PLAN.md](../public-network/PLAN.md), "Launch order");
   - the launch admission of [FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md) §1 and §4 (Q3 = a: a Simnet profile on a
     Simnet node in scope experimental-devnet), for the mainnet chain only (D5 replaces it there);
   - the wording "stays as computed" and "refuses nothing" of decision 4 of
     [2026-11-18](OWNER-DECISIONS-2026-11-18.md), which is refined by D4: readiness stays computed and truthful, and
     that decision's truthfulness rule stays.
2. **D2: founder servers with 4 GB of RAM, risk accepted.** The three founder servers have 4 GB of RAM, and the owner
   accepts the risk: a node is OOM-killed and restarts if its real use passes about 3.5 GB, for example after the
   member set fills, and block production stops if all three restart at once. The recommended size is 8 GB. (The
   standalone service unit, `deploy/standalone/install-linux.sh`, sets `MemoryHigh=2560M` and `MemoryMax=3G`, so on
   these servers the unit's own limit is reached first; the A13 model's default uses 6 GiB.)
3. **D3: founders and seeds.** The founders are three servers, `108.165.128.89`, `108.165.128.90` and
   `108.165.128.91` (also the invitation's seeds), and the owner's Windows PC (founder 4). The genesis is fresh: no
   old genesis and no dry-run identity is reused.
4. **D4: launch approval is separate from technical readiness.** The node reports, computed from facts and never
   written as a label:
   - `publicLaunchApproved`: the build's hard facts clear (spec version at least 2, consensus sources pinned, no
     test-only feature, `V3_LAUNCH_BLOCKERS` empty), v3 is active, (chainId, genesisHash) is a release pin with an
     approval reference and the expected admission `PUBLIC_MAINNET` (or `LAUNCH`), and the admission the node booted
     with matches that pin exactly;
   - `launchApprovalReference`: the pin's approval reference, or null;
   - `launchBlockers`: the hard blockers only (spec-version, consensus-unpinned, test-features, v3-launch-blocker,
     no-network or genesis-unreadable, v3-inactive, network-not-release-pinned, launch-not-approved,
     admission-unrecorded, admission-not-booted, admission-mismatch);
   - `publicLaunchReady`: `publicLaunchApproved` with no launch blocker;
   - `technicalReadiness` `{complete, pending}`: every open gate of the acceptance ledger (`plan-gate-<id>`) and what
     the admission reports (`admission-scope`, each `admission-condition-<id>` and `admission-waiver-<rule>`,
     `profile-kind`), never claimed passed.

   The launcher and the invitation description boot nothing, so they report `admission-not-booted` and are never
   approved; build-info reports `no-network`.
5. **D5: an honest protocol classification for the public network, before the final genesis.** The public network
   is admitted as the network profile (`ProfileKind::Network`) on a network node (`NodeKind::Node`) in its own scope,
   **public-network** (`Scope::PublicNetwork`, `prototypes/lp4-execution/src/admission/scope.rs`), only on a chain of
   `PUBLIC_NETWORK_CHAINS` (`[2656089850]`, the mainnet profile's chain). R22 allows the network profile on 777001 and
   on those chains and stays two-way: a Simnet profile is never relabelled as the network profile. The scope is never
   specification compliance (`specificationCompliant` false). It reports every condition, and it waives only at clause
   level, each waiver reported with its detail: R13's continuous-kind term, R21's history-budget term (the
   retained-blocks term stays enforced) and the R26 predicates that read RpcGuard fields the node does not enforce.
   The kind changes no allocRoot and no genesis hash (`prototypes/pocold/tests/fn4_profiles.rs` pins both from the
   Simnet build at 8b465b9), and no validity, root, reward, genesis decoding, wire or fork-choice rule; the
   consensus-source lock records it as one `neutral` line (ref `public-network-admission`).
6. **D6: a full public launch, not a renamed test network (the owner's further instructions).**
   - The launch is a full public launch of PoCol Mainnet, after passing the required technical and security tests
     (E lists them; a test that cannot run is reported as not run, never as passed). No testnet, devnet, simnet or
     rehearsal is launched, and the mainnet is not a test network renamed.
   - Every value is computed from the network's actual state and the decided launch policy, never fixed or forged.
   - A. The admission scope name is exactly `public-network`. The owner's waivers stay explicit as
     `admission-waiver-*` ids in `technicalReadiness`.
   - B. Every network producer (`pocol_status`, the init-testnet summary, the launcher description; build-info gives
     nulls) also reports `networkName`, `networkEnvironment` and `networkScope`, computed from facts. After a
     successful ceremony, identity pin and signed release, a node booted on the mainnet reports networkName
     "PoCol Mainnet", networkEnvironment "mainnet", networkScope "public-network", `publicLaunchApproved` true,
     `publicLaunchReady` true and a truthful `technicalReadiness`.
   - C. Answers to the open questions of the design: `publicLaunchReady` is approval with no hard blocker, and the
     technical gates are reported, not blocking; D1 accepts the documented risks, which this record lists under
     "Accepted risks (D1, D2)" and claims no more; the finding ledger (PR #107) is assumed to merge first, and the
     entries this change makes untrue are updated where that ledger is on the branch, otherwise this record carries the
     table; the ceremony hash rate comes from `pocol hashrate` (PR #108), which merges first; the launcher's display of
     approval (false, with the approval reference beside it) is accepted.
   - D. No mainnet-facing surface describes PoCol Mainnet as a testnet, devnet, simnet, experimental-devnet, private
     or rehearsal network, or as not public-launch-ready. Test modes, development tools, automated tests, the rehearsal
     profile as a test profile, historical records and every true statement of a test result or a risk are kept.
     pocol.io and pocol.org are owner-hosted and not in this repository.
   - E. The mandatory pre-launch tests: consensus rules, genesis validity, device compatibility (Linux, Windows and
     Android consensus identity), node sync, mining, transactions, rewards, external connectivity, RPC security
     controls, the proof that the move to public-network changes no consensus validity rule or node compatibility,
     and one check that every client copy carries the same chain id and genesis hash. A test that cannot run is
     reported as not run, never as passed.
   - F. Release signing stays owner-held: the first signed release follows the ceremony, at the identity pin, and no
     package is described as signed before it is.
7. **The rehearsal profile** (`deploy/mainnet/v3-rehearsal-profile.json`, chain 1656550731) is kept only as a test and
   devnet profile. It is not a launch step, and its chain is never pinned by a release.

Earlier records keep their text with the note "Superseded by owner decision 2026-11-20".

## Accepted risks (D1, D2)

D1 accepts "the documented risks" (the owner's words above). The finding ledger, `development/public-network/FINDINGS.md`,
is not on this branch: it is on branch `claude/finding-ledger` (PR #107, commit 64a642ff). This section therefore
lists, for every item the node reports as pending for PoCol Mainnet, the text on this branch that documents it and the
entries of that ledger (as of 64a642ff) that record it. Each item is covered by D1 by the owner's words; this record
claims no more for it. The waivers of D5 are the engineering form of that acceptance: they are reported, never
presented as passed. The R26 waiver skips only the predicates that read RpcGuard fields the node does not enforce. The
R13 and R21 waivers remove terms the parameter gate enforces, for the network profile on a public network chain in
scope public-network only: R13's continuous-kind term and R21's history-budget term (`retained-state bound <=
HIST_TRIE_BUDGET`). The Specification and ExperimentalDevnet scopes still refuse a network profile on those terms.
Every other term of R13 and R21 stays enforced: a finite schedule's checks, retained blocks >= min(K_hist, H_END) and
the resource facts.

| Pending item | What it is | Documented on this branch in | FINDINGS ids (PR #107) |
| --- | --- | --- | --- |
| No rehearsal (`plan-gate-L1`, `plan-gate-E1`) | The launch evidence (T1-T8, M1-M4, R-off, D1, S1, S2 and the L1 drills) has no pre-launch measurement; it is measured on PoCol Mainnet after the launch, and a remedy that changes a consensus parameter is then a fork with notice | [REHEARSAL.md](../public-network/REHEARSAL.md), [PLAN.md](../public-network/PLAN.md) L1 and E1 | F-77, F-20, F-79, F-80 |
| 4 GB founder servers (D2) | A node past its memory limit is killed and restarts; production stops if all three servers restart at once; the A13 margin is thin and the unit's `MemoryMax=3G` is below the A13 admitted bound | [A13-DERIVATION.md](../audit-2026-10-22/A13-DERIVATION.md) §4, [deploy/standalone/README.md](../../deploy/standalone/README.md) | F-21, F-22, F-23 |
| `admission-waiver-R21` | The retained-state bound at the mainnet caps, 59,702,459,358 B, exceeds `HIST_TRIE_BUDGET` 1,717,986,918 B; the owner waives the budget term (tied to D2); the retained-blocks term stays enforced; K_eff is not implemented | [FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md) §4, [governance.md](../m1/reference/governance.md) "Fresh v3 network profile" (R21 figure) | F-17, F-16 |
| `admission-waiver-R13` | A continuous protocol (no end height) on the network profile, which R13 admits only for a simnet profile on a simnet node | [FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md) §1, [governance.md](../m1/reference/governance.md) "Fresh v3 network profile" | F-10 (its text, "admitted as a Simnet profile in ExperimentalDevnet scope", is superseded by D5) |
| `admission-waiver-R26`, `admission-condition-rpc-absent` | Four RpcGuard fields are not enforced (scratch_heavy, scratch_light, mem_total, cancel_max_ms), so the R26 predicates that read them are not evaluated, on Internet-exposed nodes | [RPCGUARD.md](../audit-2026-10-22/RPCGUARD.md), `prototypes/pocold/src/lib.rs` (`RPC_ENFORCED`) | F-11, F-12, F-87 |
| `admission-condition-gas-monotonicity-assumed` | SYS gas monotonicity in the domain fields is assumed (R18) | [A13-DERIVATION.md](../audit-2026-10-22/A13-DERIVATION.md) §4 | F-15 |
| `admission-condition-gate-deferred-general-R18`, `admission-condition-gate-deferred-general-R21`, `admission-condition-gate-deferred-caps64` | The parameter gate's three deferred obligations (LP4:general-R18, LP4:general-R21, LP4:caps64-minimum-SYS_SLOT_MAX) | `prototypes/lp1-node/src/gate.rs` (`GateReport::deferred`) | F-16 |
| `admission-condition-timing-not-a-bound`, `admission-condition-continuous-protocol` | The measured timing is not a worst-case bound; the protocol is continuous | [A13-DERIVATION.md](../audit-2026-10-22/A13-DERIVATION.md) §4, [FRESH-NETWORK-CP.md](FRESH-NETWORK-CP.md) §1 | F-21 |
| `admission-scope` | The scope public-network is not specification compliance | [governance.md](../m1/reference/governance.md) "Fresh v3 network profile", `prototypes/lp4-execution/src/admission/scope.rs` | F-10 |
| `plan-gate-K1` | Independent identities: the key recovery drill, a server run with the encrypted credential, an encrypted keystore or HSM | [PLAN.md](../public-network/PLAN.md) K1, [RECOVERY.md](../public-network/RECOVERY.md) | F-42 to F-49 |
| `plan-gate-J1` | Open miner enrollment: eligibility churn and load, network-scale enrollment | [PLAN.md](../public-network/PLAN.md) J1 | F-29, F-31, F-32, F-71, F-72, F-73 |
| `plan-gate-I1` | Stable network identity: the genesis and its pin, the normative digest adoption, an independently reproduced release | [PLAN.md](../public-network/PLAN.md) I1 | F-51 to F-58, F-69 |
| `plan-gate-A1` | Honest admission: worst-case execution calibration, the R18/R21 obligations, the RpcGuard gaps | [PLAN.md](../public-network/PLAN.md) A1 | F-10, F-11, F-13, F-14, F-15, F-16, F-18, F-21, F-24 |
| `plan-gate-S1` | Bounded historical state: K_eff and historical RPC behaviour within the resource budget | [PLAN.md](../public-network/PLAN.md) S1 | F-16, F-17, F-21, F-22, F-26 |
| `plan-gate-N1` | Internet exposure: ingress acceptance, the remaining RpcGuard controls | [PLAN.md](../public-network/PLAN.md) N1 | F-11, F-12, F-25, F-35, F-36, F-38 to F-41 |
| `plan-gate-Q1` | Release review: the independent security review (after the launch, decision 7 of 2026-11-18) | [PLAN.md](../public-network/PLAN.md) Q1 | F-01 to F-04, F-50, F-62, F-63, F-67, F-68, F-70, F-74 to F-76, F-82 to F-87, F-89, F-91 |

`plan-gate-L1` also covers the L1 entries F-09, F-23, F-26, F-27, F-31, F-64, F-65, F-66, F-78 and F-81, and
`plan-gate-E1` the E1 entries F-19 and F-34. When the finding ledger is on the branch, its entries are cited instead of
this table, and F-10 gets the note "Superseded by owner decision 2026-11-20 D5".
`deploy/websites/tests/test_owner_decision_record.py` keeps this section listing every pending id the node reports for
PoCol Mainnet, and every approval reference naming a decision of an existing record.
