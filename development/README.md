# PoCol development

This directory holds the specification, the owner's decisions, the evidence of every stage and the working rules.
The current status is in [STATUS-EN.md](STATUS-EN.md).

## Where things are

| Location | What it holds |
|---|---|
| [decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md](decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md) | The owner's decision to launch a brand-new network from a fresh genesis, keeping nothing of the retired network |
| [decisions/FRESH-NETWORK-CP.md](decisions/FRESH-NETWORK-CP.md) | The final consensus parameters, genesis contents, chain ids and what is left to later steps |
| [decisions/V3-DECISIONS.md](decisions/V3-DECISIONS.md) | The v3 rule-set decisions (D-V3-01 to D-V3-34), the release rules and lock, and the founders' allocation |
| [decisions/V3-DESIGN.md](decisions/V3-DESIGN.md), [decisions/V3-EXECUTION-FEES.md](decisions/V3-EXECUTION-FEES.md) | The v3 design and its execution fees |
| [decisions/](decisions/) | The other owner decisions and evidence records; dated records are kept with their "superseded" notes |
| [VA-SPEC.md](VA-SPEC.md) | The validation specification: wire, consensus identity, the consensus-source lock and its history kinds |
| [m1/reference/](m1/reference/) | The normative reference specification: consensus, execution, network, storage, browser, governance, incentives, economics, threat model |
| [public-network/PLAN.md](public-network/PLAN.md) | The public-network launch gates (the acceptance ledger) and the launch order |
| [public-network/OPERATIONS.md](public-network/OPERATIONS.md) | Peering, discovery, upgrades, the cap-raise activation, seeds, eviction and memory bounds |
| [public-network/KEYS.md](public-network/KEYS.md) | Independent key operations |
| [audit-2026-10-22/](audit-2026-10-22/) | The audit of 2026-10-22, its repair rounds, evidence and proposals |
| `lp1/` … `lp10/`, [reports/](reports/) | Evidence and reports for each implementation stage |

The deployment side is in `deploy/`: the launch runbook in [deploy/mainnet/README.md](../deploy/mainnet/README.md)
and the native clients and the genesis ceremony in [deploy/standalone/README.md](../deploy/standalone/README.md).

## Workflow

1. Start from current `main` on a separate branch. Preserve the thesis, the specifications and their evidence.
2. Claude authors scoped changes. Codex independently reviews and runs applicable checks. Use sequential
   author/reviewer turns to avoid overlapping writes.
3. Link each finding to a requirement, affected revision, proposed fix and evidence. Distinguish open, proposed fix,
   verified, deferred and owner decision.
4. Record commands, tool versions, exit codes and the exact tested revision in the pull request. A successful test run
   does not establish complete coverage.
5. Do not close findings by elapsed time, fixed round count or agreement alone. Missing tests remain missing.
   Experimental code is not production approval.
6. The owner takes every consensus and normative decision. A decision counts only once the owner approves it
   explicitly; until then a text that needs it says "owner decision pending".
7. A change to a consensus-critical source needs a labelled history line in `prototypes/pocold/consensus-sources.lock`
   (VA-SPEC §2.3, §2.4). After launch, a rule change goes behind a new fork version; fork versions are sequential (D3).

Every status update explains what changed, why, what was actually tested, what remains and whether the owner needs to
decide anything. Code, technical specifications and progress summaries are in English.

## Execution boundary

GitHub is the shared record. Pull requests carry the proposed changes, their reviews and their CI results; nothing is
merged, deployed or launched automatically. CI workflows run on the paths they list; never count a cancelled or
unexecuted check as passing, and do not add a permanently green placeholder check. Never put secret keys, tokens or
private data in the repository, chat or uploaded artifacts.

## References

- [GitHub Issues](https://docs.github.com/en/issues/tracking-your-work-with-issues/learning-about-issues/about-issues)
- [Pull requests](https://docs.github.com/en/pull-requests/reference/pull-requests)
- [GitHub Actions](https://docs.github.com/en/actions/get-started/understand-github-actions)
